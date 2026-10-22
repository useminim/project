//! Zone d'attente : les documents qui attendent une décision de
//! l'utilisateur.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::id::{DocumentId, RunId, WorkflowId};

/// Raison pour laquelle un document est en zone d'attente.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS))]
#[serde(tag = "type", rename_all = "snake_case")]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[cfg_attr(feature = "ts", derive(ts_rs::TS), ts(export))]
pub struct WaitingEntry {
    /// Document concerné.
    pub document_id: DocumentId,
    /// Raison de l'attente.
    pub reason: WaitingReason,
    /// Date d'entrée en zone d'attente.
    pub entered_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn reads_the_documented_example() {
        let value: serde_json::Value =
            serde_json::from_str(include_str!("../tests/fixtures/waiting_entry.json")).unwrap();
        let entry: WaitingEntry = serde_json::from_value(value.clone()).unwrap();

        let WaitingReason::Uncertain { candidates } = &entry.reason else {
            panic!("raison inattendue : {:?}", entry.reason);
        };
        assert_eq!(candidates.len(), 2);
        assert_eq!(serde_json::to_value(&entry).unwrap(), value);
    }

    #[test]
    fn reads_every_reason() {
        let reasons = json!([
            { "type": "no_match" },
            { "type": "uncertain", "candidates": ["wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y"] },
            {
                "type": "action_failed",
                "workflow_id": "wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y",
                "run_id": "run_01J8ZK3V9Q4X7M2N5P6R8T0W1Y"
            }
        ]);
        let parsed: Vec<WaitingReason> = serde_json::from_value(reasons.clone()).unwrap();

        assert_eq!(serde_json::to_value(&parsed).unwrap(), reasons);
    }
}
