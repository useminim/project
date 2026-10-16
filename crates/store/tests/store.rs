//! Base chiffrée réelle : contrats des dépôts, chiffrement, erreurs et
//! récupération.

#![allow(
    clippy::unwrap_used,
    reason = "fichier de tests : clippy.toml n'autorise unwrap que dans les fonctions #[test]"
)]

use std::fs;
use std::path::{Path, PathBuf};

use minim_core::ports::{ExecutionLogRepository as _, WorkflowRepository as _};
use minim_core::testing::{contracts, samples};
use minim_store::{
    Database, DatabaseKey, InMemoryKeyProvider, KeyError, KeyProvider, OpenError, QuarantineReason,
    open_or_recover,
};

fn temp_db() -> (tempfile::TempDir, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("minim.db");
    (dir, path)
}

fn contains(path: &Path, needle: &[u8]) -> bool {
    fs::read(path).is_ok_and(|bytes| bytes.windows(needle.len()).any(|window| window == needle))
}

#[test]
fn workflow_repository_passes_the_contract() {
    let (_dir, path) = temp_db();
    let database = Database::open(&path, &DatabaseKey::generate().unwrap()).unwrap();

    contracts::workflow_repository_contract(&database.workflows());
}

#[test]
fn execution_log_repository_passes_the_contract() {
    let (_dir, path) = temp_db();
    let database = Database::open(&path, &DatabaseKey::generate().unwrap()).unwrap();

    contracts::execution_log_repository_contract(&database.execution_logs());
}

#[test]
fn never_writes_data_in_clear() {
    let (dir, path) = temp_db();
    let key = DatabaseKey::generate().unwrap();
    let mut workflow = samples::workflow(samples::workflow_id(1), 10);
    workflow.name = "Dossier TEMOIN-PATIENT-DUPONT".to_owned();
    {
        let database = Database::open(&path, &key).unwrap();
        database.workflows().save(&workflow).unwrap();
        // Base encore ouverte : le journal WAL doit lui aussi être chiffré.
        for entry in fs::read_dir(dir.path()).unwrap() {
            assert!(!contains(&entry.unwrap().path(), b"TEMOIN-PATIENT-DUPONT"));
        }
    }
    for entry in fs::read_dir(dir.path()).unwrap() {
        assert!(!contains(&entry.unwrap().path(), b"TEMOIN-PATIENT-DUPONT"));
    }

    let reopened = Database::open(&path, &key).unwrap();
    assert_eq!(
        reopened.workflows().get(workflow.id).unwrap(),
        Some(workflow)
    );
}

#[test]
fn refuses_a_wrong_key() {
    let (_dir, path) = temp_db();
    drop(Database::open(&path, &DatabaseKey::generate().unwrap()).unwrap());

    let result = Database::open(&path, &DatabaseKey::generate().unwrap());

    assert!(matches!(result, Err(OpenError::KeyMismatch)));
}

#[test]
fn reports_a_truncated_database_as_corrupted() {
    let key = DatabaseKey::generate().unwrap();
    for keep in [|len: usize| len - 1000, |_: usize| 4096] {
        let (_dir, path) = temp_db();
        {
            let database = Database::open(&path, &key).unwrap();
            for n in 1..=20 {
                database
                    .execution_logs()
                    .append(&samples::execution_log(
                        samples::run_id(n),
                        samples::document_id(n),
                        samples::date(i64::try_from(n).unwrap()),
                    ))
                    .unwrap();
            }
        }
        let bytes = fs::read(&path).unwrap();
        fs::write(&path, &bytes[..keep(bytes.len())]).unwrap();

        let result = Database::open(&path, &key);

        assert!(matches!(result, Err(OpenError::Corrupted)), "{result:?}");
    }
}

#[test]
fn creates_the_key_and_the_database_on_first_launch() {
    let (_dir, path) = temp_db();
    let keys = InMemoryKeyProvider::default();

    let (database, quarantined) = open_or_recover(&path, &keys, samples::date(0)).unwrap();

    assert_eq!(quarantined, None);
    assert!(keys.load().unwrap().is_some());
    database
        .workflows()
        .save(&samples::workflow(samples::workflow_id(1), 10))
        .unwrap();
    drop(database);
    let (reopened, quarantined) = open_or_recover(&path, &keys, samples::date(1)).unwrap();
    assert_eq!(quarantined, None);
    assert_eq!(reopened.workflows().list().unwrap().len(), 1);
}

#[test]
fn sets_aside_a_database_whose_key_is_lost() {
    let (dir, path) = temp_db();
    drop(Database::open(&path, &DatabaseKey::generate().unwrap()).unwrap());
    let original = fs::read(&path).unwrap();
    let keys = InMemoryKeyProvider::default();

    let (database, quarantined) = open_or_recover(&path, &keys, samples::date(0)).unwrap();

    let quarantined = quarantined.unwrap();
    assert_eq!(quarantined.reason, QuarantineReason::KeyLost);
    assert_eq!(
        quarantined.moved_to,
        dir.path().join("minim.db.illisible-20261001T100000Z")
    );
    assert_eq!(
        fs::read(&quarantined.moved_to).unwrap(),
        original,
        "jamais écrasée"
    );
    assert_eq!(database.workflows().list().unwrap(), []);
}

#[test]
fn sets_aside_a_database_that_the_key_cannot_read() {
    let (_dir, path) = temp_db();
    drop(Database::open(&path, &DatabaseKey::generate().unwrap()).unwrap());
    let keys = InMemoryKeyProvider::with_key(DatabaseKey::generate().unwrap());

    let (_database, quarantined) = open_or_recover(&path, &keys, samples::date(0)).unwrap();

    let quarantined = quarantined.unwrap();
    assert_eq!(quarantined.reason, QuarantineReason::KeyMismatch);
    assert!(quarantined.moved_to.exists());
}

struct UnavailableKeys;

impl KeyProvider for UnavailableKeys {
    fn load(&self) -> Result<Option<DatabaseKey>, KeyError> {
        Err(KeyError::Unavailable("trousseau verrouillé".into()))
    }

    fn save(&self, _key: &DatabaseKey) -> Result<(), KeyError> {
        Err(KeyError::Unavailable("trousseau verrouillé".into()))
    }
}

#[test]
fn leaves_everything_untouched_when_the_keychain_is_unavailable() {
    let (dir, path) = temp_db();
    drop(Database::open(&path, &DatabaseKey::generate().unwrap()).unwrap());
    let before: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();

    let result = open_or_recover(&path, &UnavailableKeys, samples::date(0));

    assert!(matches!(
        result,
        Err(OpenError::Key(KeyError::Unavailable(_)))
    ));
    let after: Vec<_> = fs::read_dir(dir.path())
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect();
    assert_eq!(before, after);
}
