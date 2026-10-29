//! Compte de l'utilisateur (D-37) : connexion, inscription, déconnexion,
//! mot de passe oublié. Toutes les requêtes au backend partent de Rust ;
//! l'interface ne voit jamais les jetons. Chaque changement émet
//! [`ACCOUNT_CHANGED`].

use minim_core::{AccountService, AccountStatus, SignUpResult};
use tauri::{AppHandle, Emitter as _, Runtime, State};

use crate::error::CommandError;

/// Événement émis quand l'état du compte change (sans contenu).
pub const ACCOUNT_CHANGED: &str = "account_changed";

/// Service du compte.
#[derive(Clone)]
pub struct Account(pub AccountService);

fn changed<R: Runtime>(app: &AppHandle<R>) {
    if app.emit(ACCOUNT_CHANGED, ()).is_err() {
        tracing::warn!("événement {ACCOUNT_CHANGED} non émis");
    }
}

/// État du compte ; rafraîchit la session si besoin.
#[tauri::command]
pub async fn account_status(account: State<'_, Account>) -> Result<AccountStatus, CommandError> {
    Ok(account.0.status().await)
}

/// Ouvre une session.
#[tauri::command]
pub async fn sign_in<R: Runtime>(
    app: AppHandle<R>,
    account: State<'_, Account>,
    email: String,
    password: String,
) -> Result<AccountStatus, CommandError> {
    let status = account.0.sign_in(&email, &password).await?;
    changed(&app);
    Ok(status)
}

/// Crée un compte.
#[tauri::command]
pub async fn sign_up<R: Runtime>(
    app: AppHandle<R>,
    account: State<'_, Account>,
    email: String,
    password: String,
) -> Result<SignUpResult, CommandError> {
    let result = account.0.sign_up(&email, &password).await?;
    if result == SignUpResult::SignedIn {
        changed(&app);
    }
    Ok(result)
}

/// Ferme la session et efface les jetons.
#[tauri::command]
pub async fn sign_out<R: Runtime>(
    app: AppHandle<R>,
    account: State<'_, Account>,
) -> Result<AccountStatus, CommandError> {
    let status = account.0.sign_out().await;
    changed(&app);
    Ok(status)
}

/// Envoie un e-mail de réinitialisation du mot de passe.
#[tauri::command]
pub async fn request_password_reset(
    account: State<'_, Account>,
    email: String,
) -> Result<(), CommandError> {
    Ok(account.0.request_password_reset(&email).await?)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use chrono::Duration;
    use minim_core::Redacted;
    use minim_core::ports::{
        AuthError, AuthProvider, AuthUser, BoxFuture, Session, SessionStore, SessionStoreError,
        SignUpOutcome, SystemClock,
    };
    use serde_json::json;
    use tauri::test::mock_builder;

    use super::*;
    use crate::commands::testing::{call, mock_app};

    struct OneAccount;

    impl AuthProvider for OneAccount {
        fn sign_up<'a>(
            &'a self,
            _: &'a str,
            _: &'a str,
        ) -> BoxFuture<'a, Result<SignUpOutcome, AuthError>> {
            Box::pin(async { Ok(SignUpOutcome::ConfirmationSent) })
        }
        fn sign_in<'a>(
            &'a self,
            email: &'a str,
            password: &'a str,
        ) -> BoxFuture<'a, Result<Session, AuthError>> {
            Box::pin(async move {
                if password != "correct horse" {
                    return Err(AuthError::InvalidCredentials);
                }
                Ok(Session {
                    user: AuthUser {
                        id: "u1".to_owned(),
                        email: email.to_owned(),
                    },
                    access_token: Redacted::new("access".to_owned()),
                    refresh_token: Redacted::new("refresh".to_owned()),
                    expires_at: chrono::Utc::now() + Duration::hours(1),
                })
            })
        }
        fn refresh<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<Session, AuthError>> {
            Box::pin(async { Err(AuthError::Network) })
        }
        fn sign_out<'a>(&'a self, _: &'a str) -> BoxFuture<'a, Result<(), AuthError>> {
            Box::pin(async { Ok(()) })
        }
        fn request_password_reset<'a>(
            &'a self,
            _: &'a str,
        ) -> BoxFuture<'a, Result<(), AuthError>> {
            Box::pin(async { Err(AuthError::RateLimited) })
        }
    }

    #[derive(Default)]
    struct Memory(Mutex<Option<Session>>);

    impl SessionStore for Memory {
        fn load(&self) -> Result<Option<Session>, SessionStoreError> {
            Ok(self.0.lock().unwrap().clone())
        }
        fn save(&self, session: &Session) -> Result<(), SessionStoreError> {
            *self.0.lock().unwrap() = Some(session.clone());
            Ok(())
        }
        fn clear(&self) -> Result<(), SessionStoreError> {
            *self.0.lock().unwrap() = None;
            Ok(())
        }
    }

    fn account(provider: Option<Arc<dyn AuthProvider>>) -> Account {
        Account(AccountService::new(
            provider,
            Arc::new(Memory::default()),
            Arc::new(SystemClock),
        ))
    }

    #[test]
    fn signs_in_and_out_through_the_allowed_commands() {
        let (_app, window) = mock_app(mock_builder().manage(account(Some(Arc::new(OneAccount)))));
        let credentials =
            |password: &str| json!({ "email": "alice@example.com", "password": password });

        assert_eq!(
            call(&window, "account_status", json!({})).unwrap(),
            json!({ "state": "signed_out" })
        );
        assert_eq!(
            call(&window, "sign_in", credentials("faux")).unwrap_err()["code"],
            "invalid_credentials"
        );
        assert_eq!(
            call(&window, "sign_in", credentials("correct horse")).unwrap(),
            json!({ "state": "signed_in", "email": "alice@example.com" })
        );
        assert_eq!(
            call(&window, "sign_up", credentials("pw")).unwrap(),
            json!("confirmation_sent")
        );
        assert_eq!(
            call(
                &window,
                "request_password_reset",
                json!({ "email": "alice@example.com" })
            )
            .unwrap_err()["code"],
            "rate_limited"
        );
        assert_eq!(
            call(&window, "sign_out", json!({})).unwrap(),
            json!({ "state": "signed_out" })
        );
    }

    #[test]
    fn reports_unconfigured_accounts() {
        let (_app, window) = mock_app(mock_builder().manage(account(None)));

        assert_eq!(
            call(&window, "account_status", json!({})).unwrap(),
            json!({ "state": "unavailable" })
        );
        let error = call(
            &window,
            "sign_in",
            json!({ "email": "a@b.c", "password": "x" }),
        )
        .unwrap_err();
        assert_eq!(error["code"], "accounts_unavailable");
    }
}
