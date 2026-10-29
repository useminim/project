//! Ouverture du stockage local chiffré au démarrage (D-31).
//!
//! La clé de la base est gardée dans le trousseau du système. Sous Linux sans
//! Secret Service, elle est gardée dans un fichier lisible par le seul
//! utilisateur, et l'interface signale une protection réduite. Si la clé est
//! inaccessible, la base n'est pas ouverte : l'interface affiche l'erreur.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use chrono::Utc;
use minim_core::ports::{ExecutionLogRepository, WorkflowRepository};
use minim_store::{
    DatabaseKey, FileKeyProvider, KeyError, KeyProvider, OpenError, QuarantineReason,
    open_or_recover,
};
use serde::Serialize;

/// Nom du fichier de la base, dans le dossier local de l'application.
pub const DATABASE_FILE: &str = "minim.db";

/// Fichier de clé utilisé en repli sous Linux sans Secret Service.
pub const KEY_FILE: &str = "minim.key";

/// Service de l'entrée du trousseau qui garde la clé.
pub const KEYCHAIN_SERVICE: &str = crate::keychain::SERVICE;
/// Compte de l'entrée du trousseau qui garde la clé.
pub const KEYCHAIN_USER: &str = "local-database-key";

/// Raison pour laquelle une ancienne base a été mise de côté.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS))]
#[serde(rename_all = "snake_case")]
pub enum RecoveryReason {
    /// La clé n'existait plus.
    KeyLost,
    /// La clé ne déchiffrait pas la base.
    KeyMismatch,
    /// La base était endommagée.
    Corrupted,
}

impl From<QuarantineReason> for RecoveryReason {
    fn from(reason: QuarantineReason) -> Self {
        match reason {
            QuarantineReason::KeyLost => Self::KeyLost,
            QuarantineReason::KeyMismatch => Self::KeyMismatch,
            QuarantineReason::Corrupted => Self::Corrupted,
        }
    }
}

/// État du stockage local, transmis à l'interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[cfg_attr(test, derive(ts_rs::TS), ts(export))]
pub struct StorageStatus {
    /// La base est ouverte.
    pub available: bool,
    /// La clé est gardée dans un fichier, faute de trousseau.
    pub reduced_protection: bool,
    /// Une ancienne base illisible a été mise de côté au démarrage.
    pub recovered: Option<RecoveryReason>,
}

/// Dépôts ouverts, partagés par les commandes.
#[derive(Clone)]
pub struct Repositories {
    /// Workflows.
    pub workflows: Arc<dyn WorkflowRepository>,
    /// Journal des exécutions.
    pub execution_logs: Arc<dyn ExecutionLogRepository>,
}

/// Stockage tel qu'il est au démarrage, géré par Tauri.
pub struct Storage {
    /// Dépôts, absents si la base n'a pas pu être ouverte.
    pub repositories: Option<Repositories>,
    /// État transmis à l'interface.
    pub status: StorageStatus,
}

/// Fournisseur de clé du trousseau du système.
struct KeychainKeyProvider {
    entry: keyring_core::Entry,
}

fn keychain_error(error: keyring_core::Error) -> KeyError {
    KeyError::Unavailable(Box::new(error))
}

impl KeychainKeyProvider {
    fn open() -> Result<Self, KeyError> {
        let entry = crate::keychain::entry(KEYCHAIN_USER).map_err(keychain_error)?;
        Ok(Self { entry })
    }
}

impl KeyProvider for KeychainKeyProvider {
    fn load(&self) -> Result<Option<DatabaseKey>, KeyError> {
        match self.entry.get_secret() {
            Ok(secret) => DatabaseKey::from_bytes(&secret).map(Some),
            Err(keyring_core::Error::NoEntry) => Ok(None),
            Err(error) => Err(keychain_error(error)),
        }
    }

    fn save(&self, key: &DatabaseKey) -> Result<(), KeyError> {
        self.entry
            .set_secret(key.as_bytes())
            .map_err(keychain_error)
    }
}

/// Choisit où garder la clé. Le repli sur fichier n'est permis que sous Linux,
/// et seulement si cela ne fait pas passer une base existante pour une base
/// sans clé : si la base existe et que seul le trousseau peut détenir sa clé,
/// l'ouverture échoue au lieu de mettre la base de côté.
fn choose_key_provider(
    keychain: Result<KeychainKeyProvider, KeyError>,
    directory: &Path,
) -> Result<(Box<dyn KeyProvider>, bool), KeyError> {
    let unavailable = match keychain {
        Ok(keychain) => match keychain.load() {
            Ok(_) => return Ok((Box::new(keychain), false)),
            Err(error) => error,
        },
        Err(error) => error,
    };
    if !cfg!(all(unix, not(target_os = "macos"))) {
        return Err(unavailable);
    }
    let key_file = directory.join(KEY_FILE);
    if key_file.exists() || !directory.join(DATABASE_FILE).exists() {
        tracing::warn!("trousseau indisponible : clé gardée dans un fichier (protection réduite)");
        Ok((Box::new(FileKeyProvider::new(key_file)), true))
    } else {
        Err(unavailable)
    }
}

/// Ouvre le stockage dans `directory` (dossier local de l'application).
/// N'échoue pas : un stockage indisponible est signalé à l'interface.
pub fn open(directory: &Path) -> Storage {
    let unavailable = |error: &OpenError| {
        tracing::error!(%error, "stockage local indisponible");
        Storage {
            repositories: None,
            status: StorageStatus {
                available: false,
                reduced_protection: false,
                recovered: None,
            },
        }
    };

    let (keys, reduced_protection) =
        match choose_key_provider(KeychainKeyProvider::open(), directory) {
            Ok(choice) => choice,
            Err(error) => return unavailable(&OpenError::Key(error)),
        };
    match open_or_recover(&database_path(directory), keys.as_ref(), Utc::now()) {
        Ok((database, quarantined)) => Storage {
            repositories: Some(Repositories {
                workflows: Arc::new(database.workflows()),
                execution_logs: Arc::new(database.execution_logs()),
            }),
            status: StorageStatus {
                available: true,
                reduced_protection,
                recovered: quarantined.map(|quarantined| quarantined.reason.into()),
            },
        },
        Err(error) => unavailable(&error),
    }
}

/// Chemin de la base dans `directory`.
#[must_use]
pub fn database_path(directory: &Path) -> PathBuf {
    directory.join(DATABASE_FILE)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unavailable() -> Result<KeychainKeyProvider, KeyError> {
        Err(KeyError::Unavailable("pas de Secret Service".into()))
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn falls_back_to_a_key_file_for_a_new_database() {
        let dir = tempfile::tempdir().unwrap();

        let (_, reduced) = choose_key_provider(unavailable(), dir.path()).unwrap();

        assert!(reduced);
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn keeps_using_an_existing_key_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DATABASE_FILE), b"base").unwrap();
        std::fs::write(dir.path().join(KEY_FILE), [7_u8; 32]).unwrap();

        let (keys, reduced) = choose_key_provider(unavailable(), dir.path()).unwrap();

        assert!(reduced);
        assert!(keys.load().unwrap().is_some());
    }

    #[test]
    fn never_sets_aside_a_database_whose_key_may_be_in_the_keychain() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(DATABASE_FILE), b"base").unwrap();

        let result = choose_key_provider(unavailable(), dir.path());

        assert!(matches!(result, Err(KeyError::Unavailable(_))));
    }

    #[test]
    fn serializes_the_status_for_the_interface() {
        let status = StorageStatus {
            available: true,
            reduced_protection: true,
            recovered: Some(RecoveryReason::KeyLost),
        };

        assert_eq!(
            serde_json::to_value(status).unwrap(),
            serde_json::json!({
                "available": true,
                "reduced_protection": true,
                "recovered": "key_lost"
            })
        );
    }
}
