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

/// Projet Supabase : variables d'environnement `MINIM_SUPABASE_URL` et
/// `MINIM_SUPABASE_ANON_KEY` au lancement (développement), sinon les mêmes
/// variables au moment de la compilation (versions publiées). Sans
/// configuration, les comptes sont indisponibles et l'application fonctionne
/// en local.
#[must_use]
pub fn config() -> Option<SupabaseConfig> {
    let read = |name: &str, built: Option<&'static str>| {
        std::env::var(name)
            .ok()
            .or_else(|| built.map(str::to_owned))
            .filter(|value| !value.trim().is_empty())
    };
    Some(SupabaseConfig {
        url: read("MINIM_SUPABASE_URL", option_env!("MINIM_SUPABASE_URL"))?,
        anon_key: read(
            "MINIM_SUPABASE_ANON_KEY",
            option_env!("MINIM_SUPABASE_ANON_KEY"),
        )?,
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
