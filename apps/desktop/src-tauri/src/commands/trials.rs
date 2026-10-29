//! Essai d'un workflow sur un fichier choisi (prompt 17) : simulation, puis
//! exécution réelle inscrite au journal.

use std::path::PathBuf;

use minim_core::{TrialOutcome, TrialService, WorkflowDraft, WorkflowId};
use tauri::State;

use super::blocking;
use crate::error::{CommandError, ErrorCode};

/// Service d'essai, absent si le stockage local n'a pas pu être ouvert.
#[derive(Clone)]
pub struct Trials(pub Option<TrialService>);

impl Trials {
    fn service(&self) -> Result<TrialService, CommandError> {
        self.0
            .clone()
            .ok_or_else(|| CommandError::new(ErrorCode::StorageUnavailable))
    }
}

/// Ce que ferait le brouillon `draft` sur le fichier `path`, sans rien
/// modifier.
#[tauri::command]
pub async fn preview_workflow_on_file(
    trials: State<'_, Trials>,
    draft: WorkflowDraft,
    path: PathBuf,
) -> Result<TrialOutcome, CommandError> {
    let service = trials.service()?;
    blocking(move || Ok(service.preview(&draft, &path)?)).await
}

/// Exécute pour de vrai le workflow enregistré `id` sur le fichier `path`.
#[tauri::command]
pub async fn run_workflow_on_file(
    trials: State<'_, Trials>,
    id: WorkflowId,
    path: PathBuf,
) -> Result<TrialOutcome, CommandError> {
    let service = trials.service()?;
    blocking(move || Ok(service.run(id, &path)?)).await
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use minim_core::WorkflowService;
    use minim_core::ports::SystemClock;
    use minim_core::testing::memory::{InMemoryExecutionLogRepository, InMemoryWorkflowRepository};
    use minim_core::testing::{FakeConverter, SequentialIds};
    use serde_json::json;
    use tauri::test::mock_builder;

    use super::*;
    use crate::commands::testing::{call, mock_app};
    use crate::commands::workflows::Workflows;

    #[test]
    fn previews_then_runs_a_workflow_on_a_file() {
        let workflows = WorkflowService::new(
            Arc::new(InMemoryWorkflowRepository::default()),
            Arc::new(SystemClock),
            Arc::new(SequentialIds::default()),
        );
        let trials = TrialService::new(
            workflows.clone(),
            Arc::new(InMemoryExecutionLogRepository::default()),
            Arc::new(FakeConverter::default()),
            Arc::new(SystemClock),
            Arc::new(SequentialIds::default()),
            Arc::new(|date: chrono::DateTime<chrono::Utc>| date.date_naive()),
        );
        let (_app, window) = mock_app(
            mock_builder()
                .manage(Workflows(Some(workflows)))
                .manage(Trials(Some(trials))),
        );
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("facture.pdf");
        std::fs::write(&file, "contenu").unwrap();
        let draft = json!({
            "name": "Factures",
            "enabled": true,
            "continue_on_match": false,
            "trigger": {
                "match": "all",
                "conditions": [{ "type": "file_name_contains", "value": "facture" }],
                "source": "manual"
            },
            "actions": [{ "type": "rename", "template": "Facture classée.pdf" }]
        });

        let preview = call(
            &window,
            "preview_workflow_on_file",
            json!({ "draft": draft, "path": file }),
        )
        .unwrap();
        let created = call(&window, "create_workflow", json!({ "draft": draft })).unwrap();
        let run = call(
            &window,
            "run_workflow_on_file",
            json!({ "id": created["id"], "path": file }),
        )
        .unwrap();
        let missing = call(
            &window,
            "preview_workflow_on_file",
            json!({ "draft": draft, "path": dir.path().join("absent.pdf") }),
        );

        assert_eq!(preview["evaluation"]["result"], "match");
        assert_eq!(preview["executed"], false);
        assert_eq!(run["executed"], true);
        assert!(dir.path().join("Facture classée.pdf").exists());
        assert_eq!(missing.unwrap_err()["code"], "not_found");
    }
}
