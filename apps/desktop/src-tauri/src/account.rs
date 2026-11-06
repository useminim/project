//! Compte de l'utilisateur (D-37) : configuration du projet Supabase et
//! session gardée dans le trousseau du système.

use std::sync::Arc;

use chrono::{DateTime, Utc};
use minim_core::Redacted;
use minim_core::ports::{AuthProvider, AuthUser, Session, SessionStore, SessionStoreError};
use minim_sync::{SupabaseAuth, SupabaseConfig};
use serde::{Deserialize, Serialize};

/// Compte de l'entrée du trousseau qui garde la session.
pub const KEYCHAIN_ACCOUNT: &str = "account-session";

/// Projet Supabase de préproduction, utilisé par défaut par les versions de
/// développement pour que `pnpm dev` suffise à toute l'équipe. Ces valeurs ne
/// sont pas secrètes : la clé publique est faite pour être incluse dans
/// l'application, et l'accès aux données repose sur le RLS, vérifié en CI.
#[cfg(debug_assertions)]
const DEV_DEFAULT: Option<(&str, &str)> = Some((
    "https://xxfafbhpjiztsefmvqgv.supabase.co",
    "sb_publishable_EAF1rHVszHz9WDOrT47Wdw_tm5ZJISP",
));
/// Les versions publiées n'ont pas de projet par défaut.
#[cfg(not(debug_assertions))]
const DEV_DEFAULT: Option<(&str, &str)> = None;

/// Projet Supabase, dans cet ordre :
/// 1. variables d'environnement `MINIM_SUPABASE_URL` et
///    `MINIM_SUPABASE_ANON_KEY` au lancement (une variable vide désactive les
///    comptes) ;
/// 2. les mêmes variables au moment de la compilation (versions publiées) ;
/// 3. en développement seulement, le projet de préproduction.
///
/// Sans configuration, les comptes sont indisponibles et l'application
/// fonctionne en local.
#[must_use]
pub fn config() -> Option<SupabaseConfig> {
    resolve(
        |name| std::env::var(name).ok(),
        (
            option_env!("MINIM_SUPABASE_URL"),
            option_env!("MINIM_SUPABASE_ANON_KEY"),
        ),
        DEV_DEFAULT,
    )
}

fn resolve(
    runtime: impl Fn(&str) -> Option<String>,
    built: (Option<&str>, Option<&str>),
    default: Option<(&str, &str)>,
) -> Option<SupabaseConfig> {
    let url = runtime("MINIM_SUPABASE_URL");
    let anon_key = runtime("MINIM_SUPABASE_ANON_KEY");
    let (url, anon_key) = if url.is_some() || anon_key.is_some() {
        (url, anon_key)
    } else if built.0.is_some() || built.1.is_some() {
        (built.0.map(str::to_owned), built.1.map(str::to_owned))
    } else {
        let (url, anon_key) = default?;
        (Some(url.to_owned()), Some(anon_key.to_owned()))
    };
    let keep = |value: Option<String>| value.filter(|value| !value.trim().is_empty());
    Some(SupabaseConfig {
        url: keep(url)?,
        anon_key: keep(anon_key)?,
    })
}

/// Client des comptes, s'ils sont configurés.
#[must_use]
pub fn provider() -> Option<Arc<dyn AuthProvider>> {
    let config = config()?;
    match SupabaseAuth::new(config) {
        Ok(auth) => Some(Arc::new(auth)),
        Err(error) => {
            tracing::error!(%error, "client des comptes indisponible");
            None
        }
    }
}

/// Session telle qu'elle est écrite dans le trousseau.
#[derive(Serialize, Deserialize)]
struct StoredSession {
    user_id: String,
    email: String,
    access_token: String,
    refresh_token: String,
    expires_at: DateTime<Utc>,
}

/// Session gardée dans le trousseau du système, jamais dans la base ni dans
/// un fichier.
pub struct KeychainSessionStore;

fn store_error(error: impl std::error::Error + Send + Sync + 'static) -> SessionStoreError {
    SessionStoreError(Box::new(error))
}

impl SessionStore for KeychainSessionStore {
    fn load(&self) -> Result<Option<Session>, SessionStoreError> {
        let entry = crate::keychain::entry(KEYCHAIN_ACCOUNT).map_err(store_error)?;
        let secret = match entry.get_secret() {
            Ok(secret) => secret,
            Err(keyring_core::Error::NoEntry) => return Ok(None),
            Err(error) => return Err(store_error(error)),
        };
        let stored: StoredSession = serde_json::from_slice(&secret).map_err(store_error)?;
        Ok(Some(Session {
            user: AuthUser {
                id: stored.user_id,
                email: stored.email,
            },
            access_token: Redacted::new(stored.access_token),
            refresh_token: Redacted::new(stored.refresh_token),
            expires_at: stored.expires_at,
        }))
    }

    fn save(&self, session: &Session) -> Result<(), SessionStoreError> {
        let stored = StoredSession {
            user_id: session.user.id.clone(),
            email: session.user.email.clone(),
            access_token: session.access_token.expose().clone(),
            refresh_token: session.refresh_token.expose().clone(),
            expires_at: session.expires_at,
        };
        let secret = serde_json::to_vec(&stored).map_err(store_error)?;
        crate::keychain::entry(KEYCHAIN_ACCOUNT)
            .and_then(|entry| entry.set_secret(&secret))
            .map_err(store_error)
    }

    fn clear(&self) -> Result<(), SessionStoreError> {
        match crate::keychain::entry(KEYCHAIN_ACCOUNT).and_then(|entry| entry.delete_credential()) {
            Ok(()) | Err(keyring_core::Error::NoEntry) => Ok(()),
            Err(error) => Err(store_error(error)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const DEFAULT: Option<(&str, &str)> = Some(("https://preprod", "preprod-key"));

    fn env<'a>(vars: &'a [(&'a str, &'a str)]) -> impl Fn(&str) -> Option<String> + 'a {
        move |name| {
            vars.iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    fn pair(config: Option<SupabaseConfig>) -> Option<(String, String)> {
        config.map(|config| (config.url, config.anon_key))
    }

    #[test]
    fn falls_back_to_the_default_project_without_configuration() {
        let config = resolve(env(&[]), (None, None), DEFAULT);
        assert_eq!(
            pair(config),
            Some(("https://preprod".to_owned(), "preprod-key".to_owned()))
        );
    }

    #[test]
    fn runtime_variables_win_over_build_values_and_default() {
        let vars = [
            ("MINIM_SUPABASE_URL", "http://127.0.0.1:54321"),
            ("MINIM_SUPABASE_ANON_KEY", "local-key"),
        ];
        let config = resolve(
            env(&vars),
            (Some("https://built"), Some("built-key")),
            DEFAULT,
        );
        assert_eq!(
            pair(config),
            Some(("http://127.0.0.1:54321".to_owned(), "local-key".to_owned()))
        );
    }

    #[test]
    fn build_values_win_over_the_default() {
        let config = resolve(
            env(&[]),
            (Some("https://built"), Some("built-key")),
            DEFAULT,
        );
        assert_eq!(
            pair(config),
            Some(("https://built".to_owned(), "built-key".to_owned()))
        );
    }

    #[test]
    fn an_empty_variable_disables_accounts() {
        let vars = [("MINIM_SUPABASE_URL", "")];
        assert!(resolve(env(&vars), (None, None), DEFAULT).is_none());
    }

    #[test]
    fn a_single_runtime_variable_does_not_mix_with_the_default() {
        let vars = [("MINIM_SUPABASE_URL", "http://127.0.0.1:54321")];
        assert!(resolve(env(&vars), (None, None), DEFAULT).is_none());
    }

    #[test]
    fn no_configuration_and_no_default_means_no_accounts() {
        assert!(resolve(env(&[]), (None, None), None).is_none());
    }
}
