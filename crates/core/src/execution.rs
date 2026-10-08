//! Journal d'exécution : la trace locale du passage d'un document dans un
//! workflow.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::action::ActionKind;
use crate::id::{DocumentId, RunId, WorkflowId};

/// Entrée du journal pour une exécution de workflow sur un document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionLog {
    /// Identifiant de l'exécution.
    pub id: RunId,
    /// Document traité.
    pub document_id: DocumentId,
    /// Workflow exécuté.
    pub workflow_id: WorkflowId,
    /// Version du workflow au moment de l'exécution.
    pub workflow_version: u32,
    /// Résultat global.
    pub status: ExecutionStatus,
    /// Actions exécutées, dans l'ordre.
    pub actions: Vec<ActionRecord>,
    /// Message d'erreur si l'exécution a échoué.
    pub error: Option<String>,
    /// Début de l'exécution.
    pub started_at: DateTime<Utc>,
}

/// Résultat global d'une exécution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExecutionStatus {
    /// Toutes les actions ont réussi.
    Success,
    /// Une action a échoué.
    Failed,
    /// Le document a été placé en zone d'attente.
    SentToWaitingZone,
}

/// Trace d'une action au sein d'une exécution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    /// Type de l'action.
    #[serde(rename = "type")]
    pub kind: ActionKind,
    /// Résultat de l'action.
    pub status: ActionStatus,
    /// Durée de l'action en millisecondes.
    pub duration_ms: u64,
}

/// Résultat d'une action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    /// L'action a réussi.
    Success,
    /// L'action a échoué.
    Failed,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_documented_example() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/execution_log.json")).unwrap();
        let log: ExecutionLog = serde_json::from_value(value.clone()).unwrap();

        assert_eq!(log.status, ExecutionStatus::Success);
        assert_eq!(log.actions[0].kind, ActionKind::Rename);
        assert_eq!(serde_json::to_value(&log).unwrap(), value);
    }

    #[test]
    fn reads_the_waiting_zone_status() {
        let status: ExecutionStatus =
            serde_json::from_value(json!("sent_to_waiting_zone")).unwrap();
        assert_eq!(status, ExecutionStatus::SentToWaitingZone);
    }
}
