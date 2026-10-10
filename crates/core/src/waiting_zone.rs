//! Zone d'attente : les documents qui attendent une décision de
//! l'utilisateur.

use chrono::{DateTime, Utc};

use crate::id::{DocumentId, RunId, WorkflowId};

/// Raison pour laquelle un document est en zone d'attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WaitingReason {
    /// Aucun workflow ne correspond au document.
    NoMatch,
    /// La correspondance est incertaine pour ces workflows.
    Uncertain {
        /// Workflows candidats, par ordre de priorité.
        candidates: Vec<WorkflowId>,
    },
    /// Une action du workflow a échoué.
    ActionFailed {
        /// Workflow exécuté.
        workflow_id: WorkflowId,
        /// Exécution en échec, détaillée dans le journal.
        run_id: RunId,
    },
}

/// Document en zone d'attente.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingEntry {
    /// Document concerné.
    pub document_id: DocumentId,
    /// Raison de l'attente.
    pub reason: WaitingReason,
    /// Date d'entrée en zone d'attente.
    pub entered_at: DateTime<Utc>,
}
