//! Conversion des erreurs internes en `StoreError` du port.

use minim_core::ports::StoreError;

/// Échec d'accès à la base (disque, verrou, SQL).
pub(crate) fn unavailable(error: rusqlite::Error) -> StoreError {
    StoreError::Unavailable(Box::new(error))
}

/// Donnée enregistrée qui ne se relit pas.
pub(crate) fn corrupted(error: impl std::error::Error + Send + Sync + 'static) -> StoreError {
    StoreError::Corrupted(Box::new(error))
}
