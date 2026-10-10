//! Synchronisation des métadonnées avec le cloud.
//!
//! Les méthodes sont asynchrones mais le trait reste utilisable derrière
//! `Arc<dyn CloudSync>` : elles renvoient une [`BoxFuture`] écrite à la main
//! plutôt qu'un `async fn`, ce qui ne lie `minim-core` à aucun runtime ni à
//! aucune crate (D-27). Une implémentation écrit simplement
//! `Box::pin(async move { … })`.
//!
//! La forme des changements est provisoire : elle sera revue avec le
//! chiffrement des métadonnées (prompt 41) et la synchronisation (prompt 42).

use std::future::Future;
use std::pin::Pin;

use thiserror::Error;

use crate::id::WorkflowId;

/// Future renvoyée par les méthodes asynchrones des ports.
pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Position dans l'historique des changements du serveur.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Revision(pub u64);

/// Données déjà chiffrées sur la machine, opaques pour le serveur.
#[derive(Clone, PartialEq, Eq)]
pub struct EncryptedPayload(pub Vec<u8>);

impl std::fmt::Debug for EncryptedPayload {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EncryptedPayload({} octets)", self.0.len())
    }
}

/// Changement d'un workflow à synchroniser.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncChange {
    /// Workflow concerné.
    pub workflow_id: WorkflowId,
    /// Définition chiffrée ; `None` pour une suppression.
    pub payload: Option<EncryptedPayload>,
}

/// Changements reçus du serveur.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyncBatch {
    /// Changements, dans l'ordre où le serveur les a reçus.
    pub changes: Vec<SyncChange>,
    /// Révision atteinte, à passer au prochain appel de [`CloudSync::pull`].
    pub revision: Revision,
}

/// Échec de la synchronisation.
#[derive(Debug, Error)]
pub enum SyncError {
    /// Le serveur est injoignable ; la synchronisation reprendra plus tard.
    #[error("serveur injoignable")]
    Offline,
    /// La session a expiré ou les droits sont insuffisants.
    #[error("accès refusé par le serveur")]
    Unauthorized,
    /// Le serveur a reçu des changements plus récents : il faut d'abord les
    /// récupérer.
    #[error("changements plus récents sur le serveur")]
    Conflict,
    /// Autre échec du transport ou du serveur.
    #[error("échec de la synchronisation")]
    Transport(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Échange des métadonnées chiffrées avec le serveur.
pub trait CloudSync: Send + Sync {
    /// Envoie des changements locaux et renvoie la révision atteinte.
    fn push<'a>(&'a self, changes: &'a [SyncChange]) -> BoxFuture<'a, Result<Revision, SyncError>>;

    /// Récupère les changements postérieurs à `since`, ou tous si `since` vaut
    /// `None`.
    fn pull(&self, since: Option<Revision>) -> BoxFuture<'_, Result<SyncBatch, SyncError>>;
}
