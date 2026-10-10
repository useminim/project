//! Données valides pour les tests.

#![allow(
    clippy::unwrap_used,
    reason = "code de test exporté : un échec doit interrompre le test"
)]

use chrono::{DateTime, Duration, Utc};
use ulid::Ulid;

use crate::action::{Action, ActionKind};
use crate::execution::{ActionRecord, ActionStatus, ExecutionLog, ExecutionStatus};
use crate::id::{DocumentId, RunId, UserId, WorkflowId};
use crate::template::Template;
use crate::trigger::{Condition, MatchMode, Trigger, TriggerSource};
use crate::workflow::{CURRENT_SCHEMA_VERSION, Owner, Workflow};

/// Date de référence des tests (1er octobre 2026, 10 h UTC), décalée de
/// `minutes`.
#[must_use]
pub fn date(minutes: i64) -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(1_790_848_800, 0).unwrap() + Duration::minutes(minutes)
}

/// Identifiant de workflow dont l'ULID vaut `n`.
#[must_use]
pub fn workflow_id(n: u128) -> WorkflowId {
    WorkflowId::from_ulid(Ulid::from(n))
}

/// Identifiant de document dont l'ULID vaut `n`.
#[must_use]
pub fn document_id(n: u128) -> DocumentId {
    DocumentId::from_ulid(Ulid::from(n))
}

/// Identifiant d'exécution dont l'ULID vaut `n`.
#[must_use]
pub fn run_id(n: u128) -> RunId {
    RunId::from_ulid(Ulid::from(n))
}

/// Workflow valide : renomme les fichiers dont le nom contient « facture ».
#[must_use]
pub fn workflow(id: WorkflowId, priority: u32) -> Workflow {
    Workflow {
        schema_version: CURRENT_SCHEMA_VERSION,
        id,
        name: format!("Workflow {priority}"),
        version: 1,
        enabled: true,
        priority,
        continue_on_match: false,
        trigger: Trigger {
            match_mode: MatchMode::All,
            conditions: vec![Condition::FileNameContains {
                value: "facture".to_owned(),
                case_sensitive: false,
            }],
            source: TriggerSource::Manual,
        },
        actions: vec![Action::Rename {
            template: Template::parse("{date}.pdf").unwrap(),
        }],
        owner: Owner::User(UserId::from_ulid(Ulid::from(1_u128))),
        created_at: date(0),
        updated_at: date(0),
    }
}

/// Exécution réussie d'un workflow sur un document, commencée à `started_at`.
#[must_use]
pub fn execution_log(
    id: RunId,
    document_id: DocumentId,
    started_at: DateTime<Utc>,
) -> ExecutionLog {
    ExecutionLog {
        id,
        document_id,
        workflow_id: workflow_id(1),
        workflow_version: 1,
        status: ExecutionStatus::Success,
        actions: vec![ActionRecord {
            kind: ActionKind::Rename,
            status: ActionStatus::Success,
            duration_ms: 4,
        }],
        error: None,
        started_at,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_valid_workflow() {
        assert_eq!(workflow(workflow_id(1), 10).validate(), Ok(()));
    }

    #[test]
    fn starts_on_the_reference_date() {
        assert_eq!(date(0).to_rfc3339(), "2026-10-01T10:00:00+00:00");
    }
}
