//! Comptes par Supabase Auth (API REST `/auth/v1`).
//!
//! La clé « anon » de Supabase est publique par conception : elle identifie
//! le projet, les droits viennent du jeton de l'utilisateur et des règles
//! d'accès. La clé « service » n'est jamais dans l'application (D-37).

use std::time::Duration;

use chrono::Utc;
use minim_core::Redacted;
use minim_core::ports::{AuthError, AuthProvider, AuthUser, BoxFuture, Session, SignUpOutcome};
use reqwest::{Client, Response, StatusCode};
use serde::Deserialize;
use serde_json::json;

use crate::http::{self, send_with_retry};

/// Projet Supabase à joindre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SupabaseConfig {
    /// Adresse du projet (`https://<projet>.supabase.co`, ou
    /// `http://127.0.0.1:54321` en développement local).
    pub url: String,
    /// Clé publique (« anon ») du projet.
    pub anon_key: String,
}

/// Comptes Supabase.
#[derive(Debug, Clone)]
pub struct SupabaseAuth {
    client: Client,
    config: SupabaseConfig,
    backoff: Duration,
}

impl SupabaseAuth {
    /// Client des comptes du projet `config`.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le client HTTP ne peut pas être construit.
    pub fn new(config: SupabaseConfig) -> reqwest::Result<Self> {
        Ok(Self {
            client: http::client()?,
            config: SupabaseConfig {
                url: config.url.trim_end_matches('/').to_owned(),
                ..config
            },
            backoff: http::default_backoff(),
        })
    }

    /// Attente avant la première nouvelle tentative (raccourcie en test).
    #[must_use]
    pub const fn with_backoff(mut self, backoff: Duration) -> Self {
        self.backoff = backoff;
        self
    }

    async fn post(
        &self,
        path: &str,
        body: serde_json::Value,
        bearer: Option<&str>,
    ) -> Result<Response, AuthError> {
        let url = format!("{}/auth/v1/{path}", self.config.url);
        send_with_retry(
            || {
                let request = self
                    .client
                    .post(&url)
                    .header("apikey", &self.config.anon_key)
                    .json(&body);
                match bearer {
                    Some(token) => request.bearer_auth(token),
                    None => request,
                }
            },
            self.backoff,
        )
        .await
        .map_err(|_| AuthError::Network)
    }
}

/// Réponse « session » de Supabase Auth.
#[derive(Deserialize)]
struct SessionBody {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
    user: UserBody,
}

#[derive(Deserialize)]
struct UserBody {
    id: String,
    #[serde(default)]
    email: Option<String>,
}

/// Erreur de Supabase Auth (selon la version : `error_code`, `code` ou
/// `error`).
#[derive(Deserialize, Default)]
struct ErrorBody {
    #[serde(default)]
    error_code: Option<String>,
    #[serde(default)]
    code: Option<serde_json::Value>,
    #[serde(default)]
    error: Option<String>,
}

impl ErrorBody {
    fn code(&self) -> String {
        self.error_code
            .clone()
            .or_else(|| {
                self.code
                    .as_ref()
                    .and_then(|code| code.as_str().map(str::to_owned))
            })
            .or_else(|| self.error.clone())
            .unwrap_or_default()
    }
}

/// Erreur typée d'une réponse en échec. Le message du serveur n'est jamais
/// repris (il peut contenir l'adresse e-mail).
async fn error_of(response: Response) -> AuthError {
    let status = response.status();
    let body: ErrorBody = response.json().await.unwrap_or_default();
    match body.code().as_str() {
        "invalid_credentials" | "invalid_grant" if status != StatusCode::TOO_MANY_REQUESTS => {
            AuthError::InvalidCredentials
        }
        "email_not_confirmed" => AuthError::EmailNotConfirmed,
        "user_already_exists" | "email_exists" => AuthError::EmailTaken,
        "weak_password" => AuthError::WeakPassword,
        "refresh_token_not_found"
        | "refresh_token_already_used"
        | "session_not_found"
        | "session_expired"
        | "bad_jwt" => AuthError::SessionExpired,
        "over_request_rate_limit" | "over_email_send_rate_limit" => AuthError::RateLimited,
        _ if status == StatusCode::TOO_MANY_REQUESTS => AuthError::RateLimited,
        _ if status == StatusCode::UNAUTHORIZED => AuthError::SessionExpired,
        _ => AuthError::Server {
            status: status.as_u16(),
        },
    }
}

async fn session_of(response: Response) -> Result<Session, AuthError> {
    if !response.status().is_success() {
        return Err(error_of(response).await);
    }
    let status = response.status().as_u16();
    let body: SessionBody = response
        .json()
        .await
        .map_err(|_| AuthError::Server { status })?;
    Ok(session(body))
}

fn session(body: SessionBody) -> Session {
    Session {
        user: AuthUser {
            id: body.user.id,
            email: body.user.email.unwrap_or_default(),
        },
        access_token: Redacted::new(body.access_token),
        refresh_token: Redacted::new(body.refresh_token),
        expires_at: Utc::now() + chrono::Duration::seconds(body.expires_in.max(0)),
    }
}

impl AuthProvider for SupabaseAuth {
    fn sign_up<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> BoxFuture<'a, Result<SignUpOutcome, AuthError>> {
        Box::pin(async move {
            let response = self
                .post(
                    "signup",
                    json!({ "email": email, "password": password }),
                    None,
                )
                .await?;
            if !response.status().is_success() {
                return Err(error_of(response).await);
            }
            let status = response.status().as_u16();
            let body: serde_json::Value = response
                .json()
                .await
                .map_err(|_| AuthError::Server { status })?;
            // Avec confirmation de l'e-mail, Supabase renvoie l'utilisateur
            // seul ; sans, il renvoie directement une session.
            if body.get("access_token").is_some() {
                let body: SessionBody =
                    serde_json::from_value(body).map_err(|_| AuthError::Server { status })?;
                Ok(SignUpOutcome::SignedIn(session(body)))
            } else {
                Ok(SignUpOutcome::ConfirmationSent)
            }
        })
    }

    fn sign_in<'a>(
        &'a self,
        email: &'a str,
        password: &'a str,
    ) -> BoxFuture<'a, Result<Session, AuthError>> {
        Box::pin(async move {
            let response = self
                .post(
                    "token?grant_type=password",
                    json!({ "email": email, "password": password }),
                    None,
                )
                .await?;
            session_of(response).await
        })
    }

    fn refresh<'a>(&'a self, refresh_token: &'a str) -> BoxFuture<'a, Result<Session, AuthError>> {
        Box::pin(async move {
            let response = self
                .post(
                    "token?grant_type=refresh_token",
                    json!({ "refresh_token": refresh_token }),
                    None,
                )
                .await?;
            session_of(response).await
        })
    }

    fn sign_out<'a>(&'a self, access_token: &'a str) -> BoxFuture<'a, Result<(), AuthError>> {
        Box::pin(async move {
            let response = self
                .post("logout?scope=local", json!({}), Some(access_token))
                .await?;
            // Une session déjà expirée côté serveur est déjà fermée.
            if response.status().is_success() || response.status() == StatusCode::UNAUTHORIZED {
                Ok(())
            } else {
                Err(error_of(response).await)
            }
        })
    }

    fn request_password_reset<'a>(
        &'a self,
        email: &'a str,
    ) -> BoxFuture<'a, Result<(), AuthError>> {
        Box::pin(async move {
            let response = self
                .post("recover", json!({ "email": email }), None)
                .await?;
            if response.status().is_success() {
                Ok(())
            } else {
                Err(error_of(response).await)
            }
        })
    }
}
