//! Stockage local : un dépôt par type de donnée.
//!
//! Les implémentations sont synchrones ; l'appelant les exécute hors du fil de
//! l'interface. Leur comportement attendu est décrit par les tests de contrat
//! de `testing::contracts`, que toute implémentation doit passer.

use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::document::{ContentHash, Document, DocumentStatus};
use crate::execution::{ExecutionLog, ExecutionStatus};
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
    /// Un workflow a été modifié entre sa lecture et son enregistrement : la
    /// version enregistrée doit être exactement celle qui précède.
    #[error(
        "conflit de version sur {id} : version {stored} enregistrée, version {attempted} proposée"
    )]
    VersionConflict {
        /// Identifiant du workflow.
        id: String,
        /// Version actuellement enregistrée.
        stored: u32,
        /// Version proposée.
        attempted: u32,
    },
    /// Un autre document a déjà ce contenu (même empreinte).
    #[error("le document {existing} a déjà ce contenu")]
    DuplicateContent {
        /// Identifiant du document qui a déjà ce contenu.
        existing: String,
    },
    /// Le changement de statut d'un document n'est pas permis.
    #[error("le document {id} ne peut pas passer de {from:?} à {to:?}")]
    InvalidTransition {
        /// Identifiant du document.
        id: String,
        /// Statut enregistré.
        from: DocumentStatus,
        /// Statut demandé.
        to: DocumentStatus,
    },
    /// L'ordre demandé ne contient pas exactement les workflows existants.
    #[error("l'ordre demandé ne correspond pas aux workflows existants")]
    InvalidOrder,
    /// Des données enregistrées ne peuvent pas être relues.
    #[error("données enregistrées illisibles")]
    Corrupted(#[source] Box<dyn std::error::Error + Send + Sync>),
    /// Le stockage est inaccessible (disque, base verrouillée…).
    #[error("stockage local inaccessible")]
    Unavailable(#[source] Box<dyn std::error::Error + Send + Sync>),
}

/// Définitions des workflows de l'utilisateur.
///
/// Un workflow supprimé est conservé comme tombstone (utile à la
/// synchronisation) : il n'apparaît plus dans `get` ni `list`, mais garde sa
/// version.
pub trait WorkflowRepository: Send + Sync {
    /// Enregistre un nouveau workflow, ou remplace celui qui a le même
    /// identifiant (contrôle de concurrence optimiste).
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StoreError::VersionConflict`] si un workflow (ou un
    /// tombstone) porte déjà cet identifiant et que `workflow.version` n'est
    /// pas exactement sa version plus 1.
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

    /// Supprime le workflow : il devient un tombstone et sa version augmente
    /// de 1. Renvoie `false` s'il n'existait pas ou était déjà supprimé.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn delete(&self, id: WorkflowId) -> Result<bool, StoreError>;

    /// Réordonne les workflows en une seule transaction : le premier de
    /// `ordered` reçoit la priorité 10, le suivant 20, etc. Chaque workflow
    /// dont la priorité change voit sa version augmenter de 1 et sa date de
    /// modification passer à `updated_at`.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StoreError::InvalidOrder`] si `ordered` ne contient pas
    /// exactement une fois chaque workflow existant ; rien n'est alors
    /// modifié.
    fn reorder(&self, ordered: &[WorkflowId], updated_at: DateTime<Utc>) -> Result<(), StoreError>;
}

/// Écart entre deux priorités consécutives après un réordonnancement, pour
/// pouvoir insérer un workflow entre deux autres sans tout renuméroter.
pub const PRIORITY_STEP: u32 = 10;

/// Critères de recherche dans le journal des exécutions. Les résultats vont
/// de la plus récente à la plus ancienne.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExecutionLogQuery {
    /// Uniquement les exécutions de ce workflow.
    pub workflow_id: Option<WorkflowId>,
    /// Uniquement les exécutions de ce statut.
    pub status: Option<ExecutionStatus>,
    /// Uniquement les exécutions commencées à cette date ou après.
    pub started_from: Option<DateTime<Utc>>,
    /// Uniquement les exécutions commencées avant cette date (exclue).
    pub started_before: Option<DateTime<Utc>>,
    /// Nombre maximal de résultats.
    pub limit: usize,
    /// Nombre de résultats à sauter (pagination).
    pub offset: usize,
}

impl Default for ExecutionLogQuery {
    fn default() -> Self {
        Self {
            workflow_id: None,
            status: None,
            started_from: None,
            started_before: None,
            limit: 50,
            offset: 0,
        }
    }
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

    /// Renvoie les exécutions qui répondent à `query`, de la plus récente à la
    /// plus ancienne (date de début, puis identifiant).
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn search(&self, query: &ExecutionLogQuery) -> Result<Vec<ExecutionLog>, StoreError>;

    /// Supprime les exécutions commencées avant `cutoff` (durée de
    /// rétention) et renvoie leur nombre.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn purge_before(&self, cutoff: DateTime<Utc>) -> Result<usize, StoreError>;
}

/// Documents reçus par minim.
pub trait DocumentRepository: Send + Sync {
    /// Enregistre un nouveau document, ou remplace celui qui a le même
    /// identifiant (nouvel emplacement après une action, retraitement…). Le
    /// statut est enregistré tel quel : les changements de statut au fil du
    /// traitement passent par [`DocumentRepository::update_status`].
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StoreError::DuplicateContent`] si un autre document a la
    /// même empreinte ; rien n'est alors modifié.
    fn save(&self, document: &Document) -> Result<(), StoreError>;

    /// Renvoie le document, ou `None` s'il n'existe pas.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn get(&self, id: DocumentId) -> Result<Option<Document>, StoreError>;

    /// Renvoie le document qui a ce contenu, ou `None` : permet de ne pas
    /// traiter deux fois le même fichier.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn find_by_hash(&self, sha256: &ContentHash) -> Result<Option<Document>, StoreError>;

    /// Fait passer le document au statut `status`, si la transition est
    /// permise (voir [`DocumentStatus::can_become`]). Renvoie `false` si le
    /// document n'existe pas.
    ///
    /// # Erreurs
    ///
    /// Renvoie [`StoreError::InvalidTransition`] si la transition n'est pas
    /// permise ; rien n'est alors modifié.
    fn update_status(&self, id: DocumentId, status: DocumentStatus) -> Result<bool, StoreError>;

    /// Renvoie les documents qui ont ce statut, du plus ancien au plus récent
    /// (date de réception, puis identifiant).
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn list_by_status(&self, status: DocumentStatus) -> Result<Vec<Document>, StoreError>;
}

/// Documents en zone d'attente.
pub trait WaitingZoneRepository: Send + Sync {
    /// Place un document en zone d'attente, ou remplace son entrée.
    ///
    /// # Erreurs
    ///
    /// Renvoie une erreur si le stockage échoue.
    fn put(&self, entry: &WaitingEntry) -> Result<(), StoreError>;

    /// Renvoie les documents en attente, du plus ancien au plus récent (date
    /// d'entrée, puis identifiant du document).
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
