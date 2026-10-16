//! Stockage local chiffré de minim (D-31) : base SQLite chiffrée en entier par
//! SQLCipher, clé aléatoire de 256 bits gardée par un [`KeyProvider`].
//!
//! Les dépôts implémentent les ports de `minim-core` et passent leurs tests de
//! contrat. Les appels sont bloquants : l'application les exécute hors du fil
//! de l'interface.

mod database;
mod errors;
mod executions;
mod key;
mod workflows;

pub use database::{
    Database, OpenError, QuarantineReason, Quarantined, open_or_recover, quarantine,
};
pub use executions::SqliteExecutionLogRepository;
pub use key::{DatabaseKey, FileKeyProvider, InMemoryKeyProvider, KEY_LEN, KeyError, KeyProvider};
pub use workflows::SqliteWorkflowRepository;
