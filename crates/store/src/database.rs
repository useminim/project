//! Ouverture de la base chiffrée : clé, contrôle d'intégrité, réglages,
//! migrations, et mise de côté d'une base illisible.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};
use rusqlite::{Connection, ErrorCode};
use rusqlite_migration::{M, Migrations, SchemaVersion};
use thiserror::Error;

use crate::executions::SqliteExecutionLogRepository;
use crate::key::{DatabaseKey, KeyError, KeyProvider};
use crate::workflows::SqliteWorkflowRepository;

/// Migrations du schéma, dans l'ordre. Une migration publiée n'est jamais
/// modifiée : on en ajoute une nouvelle.
const MIGRATIONS: &[M<'static>] = &[M::up(include_str!("../migrations/001_initial.sql"))];

/// Échec de l'ouverture de la base.
///
/// Le texte de ces erreurs peut être journalisé : il ne contient ni chemin ni
/// donnée.
#[derive(Debug, Error)]
pub enum OpenError {
    /// La base ne se déchiffre pas avec cette clé : clé perdue ou remplacée,
    /// ou fichier qui n'est pas une base minim.
    #[error("base illisible avec la clé disponible")]
    KeyMismatch,
    /// La base se déchiffre, mais des pages sont endommagées.
    #[error("base endommagée")]
    Corrupted,
    /// La clé est inaccessible ; la base n'a pas été touchée.
    #[error("clé de la base inaccessible")]
    Key(#[source] KeyError),
    /// La mise à jour du schéma a échoué.
    #[error("mise à jour du schéma impossible")]
    Migration(#[source] rusqlite_migration::Error),
    /// Le fichier ou son dossier est inaccessible.
    #[error("fichier de la base inaccessible")]
    Io(#[source] io::Error),
    /// Autre erreur de SQLite.
    #[error("erreur de la base")]
    Sqlite(#[source] rusqlite::Error),
}

fn sqlite_error(error: rusqlite::Error) -> OpenError {
    match error.sqlite_error_code() {
        Some(ErrorCode::NotADatabase) => OpenError::KeyMismatch,
        Some(ErrorCode::DatabaseCorrupt) => OpenError::Corrupted,
        _ => OpenError::Sqlite(error),
    }
}

/// Base locale ouverte. Les dépôts partagent une seule connexion, protégée
/// par un verrou : le volume est faible et SQLite n'écrit qu'un à la fois.
#[derive(Debug, Clone)]
pub struct Database {
    connection: Arc<Mutex<Connection>>,
}

impl Database {
    /// Ouvre la base située à `path`, ou la crée si elle n'existe pas, puis
    /// applique les migrations en attente.
    ///
    /// Avant de migrer une base existante, une copie du fichier est faite à
    /// côté (`<base>.avant-v<version>`).
    ///
    /// # Erreurs
    ///
    /// Renvoie [`OpenError::KeyMismatch`] si la clé ne déchiffre pas la base,
    /// [`OpenError::Corrupted`] si des pages sont endommagées, et les autres
    /// variantes en cas d'échec d'accès ou de migration.
    pub fn open(path: &Path, key: &DatabaseKey) -> Result<Self, OpenError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(OpenError::Io)?;
        }
        let mut connection = Connection::open(path).map_err(sqlite_error)?;
        configure_and_check(&connection, key)?;
        migrate(&mut connection, path, &Migrations::from_slice(MIGRATIONS))?;
        Ok(Self {
            connection: Arc::new(Mutex::new(connection)),
        })
    }

    /// Dépôt des workflows.
    #[must_use]
    pub fn workflows(&self) -> SqliteWorkflowRepository {
        SqliteWorkflowRepository::new(self.clone())
    }

    /// Dépôt du journal des exécutions.
    #[must_use]
    pub fn execution_logs(&self) -> SqliteExecutionLogRepository {
        SqliteExecutionLogRepository::new(self.clone())
    }

    /// Verrouille la connexion. Un panic pendant une transaction l'annule ;
    /// la connexion reste utilisable.
    pub(crate) fn lock(&self) -> MutexGuard<'_, Connection> {
        self.connection
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}

/// Clé, contrôle de lecture et d'intégrité, puis réglages.
fn configure_and_check(connection: &Connection, key: &DatabaseKey) -> Result<(), OpenError> {
    // SQLCipher écrit sinon ses propres messages sur la sortie d'erreur.
    connection
        .pragma_update(None, "cipher_log_level", "NONE")
        .map_err(sqlite_error)?;
    connection
        .pragma_update(None, "key", &*key.sqlcipher_literal())
        .map_err(sqlite_error)?;

    // Première lecture : échoue si la clé ne convient pas.
    connection
        .query_row("SELECT count(*) FROM sqlite_master", [], |row| {
            row.get::<_, i64>(0)
        })
        .map_err(sqlite_error)?;

    // Intégrité : authentification de chaque page (HMAC), puis structure.
    let cipher_problems: Vec<String> = connection
        .prepare("PRAGMA cipher_integrity_check")
        .and_then(|mut statement| statement.query_map([], |row| row.get(0))?.collect())
        .map_err(sqlite_error)?;
    let quick_check: String = connection
        .query_row("PRAGMA quick_check", [], |row| row.get(0))
        .map_err(sqlite_error)?;
    if !cipher_problems.is_empty() || quick_check != "ok" {
        return Err(OpenError::Corrupted);
    }

    let journal_mode: String = connection
        .query_row("PRAGMA journal_mode = WAL", [], |row| row.get(0))
        .map_err(sqlite_error)?;
    if !journal_mode.eq_ignore_ascii_case("wal") {
        tracing::warn!(journal_mode, "mode WAL indisponible pour la base locale");
    }
    connection
        .execute_batch("PRAGMA synchronous = FULL; PRAGMA foreign_keys = ON;")
        .map_err(sqlite_error)
}

fn migrate(
    connection: &mut Connection,
    path: &Path,
    migrations: &Migrations<'_>,
) -> Result<(), OpenError> {
    let pending = migrations
        .pending_migrations(connection)
        .map_err(OpenError::Migration)?;
    if pending > 0
        && let SchemaVersion::Inside(version) = migrations
            .current_version(connection)
            .map_err(OpenError::Migration)?
    {
        backup_before_migration(connection, path, version.get())?;
    }
    migrations
        .to_latest(connection)
        .map_err(OpenError::Migration)
}

/// Copie le fichier de la base avant une migration, après avoir reporté le
/// journal WAL dans le fichier principal. La copie reste chiffrée.
fn backup_before_migration(
    connection: &Connection,
    path: &Path,
    version: usize,
) -> Result<(), OpenError> {
    connection
        .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |_| Ok(()))
        .map_err(sqlite_error)?;
    fs::copy(path, sibling(path, &format!("avant-v{version}"))).map_err(OpenError::Io)?;
    tracing::info!(version, "sauvegarde de la base avant migration");
    Ok(())
}

/// `minim.db` → `minim.db.<suffix>`.
fn sibling(path: &Path, suffix: &str) -> PathBuf {
    let mut name = path.file_name().unwrap_or_default().to_os_string();
    name.push(".");
    name.push(suffix);
    path.with_file_name(name)
}

/// Raison pour laquelle une base a été mise de côté.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuarantineReason {
    /// La clé n'existe plus (réinstallation, nouveau profil).
    KeyLost,
    /// La clé disponible ne déchiffre pas la base.
    KeyMismatch,
    /// La base est endommagée.
    Corrupted,
}

/// Base existante qui n'a pas pu être ouverte, renommée pour ne jamais être
/// écrasée.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Quarantined {
    /// Raison de la mise de côté.
    pub reason: QuarantineReason,
    /// Nouvel emplacement du fichier.
    pub moved_to: PathBuf,
}

/// Renomme la base illisible (et ses fichiers `-wal` et `-shm`) en
/// `<base>.illisible-<date>` : elle n'est jamais écrasée.
///
/// # Erreurs
///
/// Renvoie une erreur si le fichier ne peut pas être renommé.
pub fn quarantine(path: &Path, now: DateTime<Utc>) -> io::Result<PathBuf> {
    let target = sibling(path, &format!("illisible-{}", now.format("%Y%m%dT%H%M%SZ")));
    fs::rename(path, &target)?;
    for suffix in ["-wal", "-shm"] {
        let mut companion = path.as_os_str().to_os_string();
        companion.push(suffix);
        let companion = PathBuf::from(companion);
        if companion.exists() {
            let mut moved = target.as_os_str().to_os_string();
            moved.push(suffix);
            fs::rename(&companion, PathBuf::from(moved))?;
        }
    }
    Ok(target)
}

/// Ouvre la base avec la clé du fournisseur, en la créant au premier
/// lancement. Une base existante illisible (clé perdue ou différente, base
/// endommagée) est mise de côté et remplacée par une base neuve : l'appelant
/// en informe l'utilisateur.
///
/// # Erreurs
///
/// Renvoie [`OpenError::Key`] si le fournisseur de clé ne répond pas : rien
/// n'est alors modifié, ni la base ni la clé.
pub fn open_or_recover(
    path: &Path,
    keys: &dyn KeyProvider,
    now: DateTime<Utc>,
) -> Result<(Database, Option<Quarantined>), OpenError> {
    let existing = path.exists();
    let Some(key) = keys.load().map_err(OpenError::Key)? else {
        let quarantined = if existing {
            Some(set_aside(path, QuarantineReason::KeyLost, now)?)
        } else {
            None
        };
        let key = DatabaseKey::generate().map_err(OpenError::Key)?;
        keys.save(&key).map_err(OpenError::Key)?;
        return Ok((Database::open(path, &key)?, quarantined));
    };

    match Database::open(path, &key) {
        Ok(database) => Ok((database, None)),
        Err(error @ (OpenError::KeyMismatch | OpenError::Corrupted)) if existing => {
            let reason = if matches!(error, OpenError::Corrupted) {
                QuarantineReason::Corrupted
            } else {
                QuarantineReason::KeyMismatch
            };
            let quarantined = set_aside(path, reason, now)?;
            Ok((Database::open(path, &key)?, Some(quarantined)))
        }
        Err(error) => Err(error),
    }
}

fn set_aside(
    path: &Path,
    reason: QuarantineReason,
    now: DateTime<Utc>,
) -> Result<Quarantined, OpenError> {
    let moved_to = quarantine(path, now).map_err(OpenError::Io)?;
    tracing::warn!(
        ?reason,
        "base locale illisible mise de côté, base neuve créée"
    );
    Ok(Quarantined { reason, moved_to })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backs_up_an_existing_database_before_migrating() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("minim.db");
        let key = DatabaseKey::generate().unwrap();
        let first = [M::up("CREATE TABLE a (x INTEGER) STRICT;")];
        let both = [
            first[0].clone(),
            M::up("CREATE TABLE b (x INTEGER) STRICT;"),
        ];

        let mut connection = Connection::open(&path).unwrap();
        configure_and_check(&connection, &key).unwrap();
        migrate(&mut connection, &path, &Migrations::from_slice(&first)).unwrap();
        assert!(
            !dir.path().join("minim.db.avant-v1").exists(),
            "pas de copie pour une base neuve"
        );
        connection.execute("INSERT INTO a VALUES (42)", []).unwrap();

        migrate(&mut connection, &path, &Migrations::from_slice(&both)).unwrap();
        drop(connection);

        // La copie est la base d'avant la migration, toujours chiffrée.
        let backup = Connection::open(dir.path().join("minim.db.avant-v1")).unwrap();
        configure_and_check(&backup, &key).unwrap();
        let value: i64 = backup
            .query_row("SELECT x FROM a", [], |row| row.get(0))
            .unwrap();
        assert_eq!(value, 42);
        let tables: i64 = backup
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'b'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(tables, 0);
    }
}
