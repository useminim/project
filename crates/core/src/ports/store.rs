//! Stockage local : un dépôt par type de donnée.
//!
//! Les implémentations sont synchrones ; l'appelant les exécute hors du fil de
//! l'interface. Leur comportement attendu est décrit par les tests de contrat
//! de `testing::contracts`, que toute implémentation doit passer.

use thiserror::Error;

use crate::execution::ExecutionLog;
use crate::id::{DocumentId, WorkflowId};
use crate::waiting_zone::WaitingEntry;
use crate::workflow::Workflow;

/// Échec d'un accès au stockage local.
///
/// Le texte de ces erreurs peut être journalisé : il ne contient que des
/// identifiants, jamais de chemin ni de contenu.
#[derive(Debug, Error)]
pub enum StoreError {
    /// Un élément avec cet identifiant existe déjà.
    #[error("l'élément {id} existe déjà")]
    AlreadyExists {
        /// Identifiant de l'élément.
        id: String,
    },
    /// Des données enregistrées ne peuvent pas être relues.
    #[error("données enregistrées illisibles")]
    Corrupted(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// Le stockage est inaccessible (disque, base verrouillée…).
    #[error("stockage local inaccessible")]
    Unavailable(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Définitions des workflows de l'utilisateur.
pub trait WorkflowRepository: Send + Sync {
    /// Enregistre un workflow, ou remplace celui qui a le même identifiant.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn save(&self, workflow: &Workflow) -> Result<(), StoreError>;

    /// Renvoie le workflow, ou `None` s'il n'existe pas.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn get(&self, id: WorkflowId) -> Result<Option<Workflow>, StoreError>;

    /// Renvoie tous les workflows dans l'ordre d'évaluation : priorité
    /// croissante, puis identifiant croissant (ordre de création) à priorité
    /// égale.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn list(&self) -> Result<Vec<Workflow>, StoreError>;

    /// Supprime le workflow. Renvoie `false` s'il n'existait pas.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn delete(&self, id: WorkflowId) -> Result<bool, StoreError>;
}

/// Journal des exécutions.
pub trait ExecutionLogRepository: Send + Sync {
    /// Ajoute une exécution au journal. Une exécution n'est jamais modifiée.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StoreError::AlreadyExists`] si une exécution porte déjà cet
    /// identifiant.
    fn append(&self, log: &ExecutionLog) -> Result<(), StoreError>;

    /// Renvoie au plus `limit` exécutions, de la plus récente à la plus
    /// ancienne (date de début, puis identifiant).
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn recent(&self, limit: usize) -> Result<Vec<ExecutionLog>, StoreError>;

    /// Renvoie les exécutions d'un document, de la plus ancienne à la plus
    /// récente.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn for_document(&self, document_id: DocumentId) -> Result<Vec<ExecutionLog>, StoreError>;
}

/// Documents en zone d'attente. Implémenté avec la zone d'attente (prompt 23).
pub trait WaitingZoneRepository: Send + Sync {
    /// Place un document en zone d'attente, ou remplace son entrée.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn put(&self, entry: &WaitingEntry) -> Result<(), StoreError>;

    /// Renvoie les documents en attente, du plus ancien au plus récent.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn list(&self) -> Result<Vec<WaitingEntry>, StoreError>;

    /// Retire un document de la zone d'attente. Renvoie `false` s'il n'y était
    /// pas.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn remove(&self, document_id: DocumentId) -> Result<bool, StoreError>;
}
