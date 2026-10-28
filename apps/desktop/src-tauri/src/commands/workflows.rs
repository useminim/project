//! Gestion des workflows (prompt 14). Chaque modification émet l'événement
//! [`WORKFLOWS_CHANGED`], pour que tous les écrans se rafraîchissent.

use minim_core::{ValidationError, Workflow, WorkflowDraft, WorkflowId, WorkflowService};
use tauri::{AppHandle, Emitter as _, Runtime, State};

use super::blocking;
use crate::error::{CommandError, ErrorCode};

/// Événement émis après chaque modification des workflows (sans contenu).
pub const WORKFLOWS_CHANGED: &str = "workflows_changed";

/// Service des workflows, absent si le stockage local n'a pas pu être ouvert.
#[derive(Clone)]
pub struct Workflows(pub Option<WorkflowService>);

impl Workflows {
    fn service(&self) -> Result<WorkflowService, CommandError> {
        self.0
            .clone()
            .ok_or_else(|| CommandError::new(ErrorCode::StorageUnavailable))
    }
}

fn changed<R: Runtime>(app: &AppHandle<R>) {
    if app.emit(WORKFLOWS_CHANGED, ()).is_err() {
        tracing::warn!("événement {WORKFLOWS_CHANGED} non émis");
    }
}

/// Workflows dans l'ordre d'évaluation.
#[tauri::command]
pub async fn list_workflows(
    workflows: State<'_, Workflows>,
) -> Result<Vec<Workflow>, CommandError> {
    let service = workflows.service()?;
    blocking(move || Ok(service.list()?)).await
}

/// Un workflow.
#[tauri::command]
pub async fn get_workflow(
    workflows: State<'_, Workflows>,
    id: WorkflowId,
) -> Result<Workflow, CommandError> {
    let service = workflows.service()?;
    blocking(move || Ok(service.get(id)?)).await
}

/// Problèmes de validation d'un brouillon (vide s'il est valide).
#[tauri::command]
pub async fn validate_workflow(
    workflows: State<'_, Workflows>,
    draft: WorkflowDraft,
) -> Result<Vec<ValidationError>, CommandError> {
    let service = workflows.service()?;
    blocking(move || Ok(service.validate(&draft))).await
}

/// Crée un workflow, placé après les autres.
#[tauri::command]
pub async fn create_workflow<R: Runtime>(
    app: AppHandle<R>,
    workflows: State<'_, Workflows>,
    draft: WorkflowDraft,
) -> Result<Workflow, CommandError> {
    let service = workflows.service()?;
    let created = blocking(move || Ok(service.create(draft)?)).await?;
    changed(&app);
    Ok(created)
}

/// Remplace le contenu d'un workflow lu à la version `expected_version`.
#[tauri::command]
pub async fn update_workflow<R: Runtime>(
    app: AppHandle<R>,
    workflows: State<'_, Workflows>,
    id: WorkflowId,
    expected_version: u32,
    draft: WorkflowDraft,
) -> Result<Workflow, CommandError> {
    let service = workflows.service()?;
    let updated = blocking(move || Ok(service.update(id, expected_version, draft)?)).await?;
    changed(&app);
    Ok(updated)
}

/// Supprime un workflow.
#[tauri::command]
pub async fn delete_workflow<R: Runtime>(
    app: AppHandle<R>,
    workflows: State<'_, Workflows>,
    id: WorkflowId,
) -> Result<(), CommandError> {
    let service = workflows.service()?;
    blocking(move || Ok(service.delete(id)?)).await?;
    changed(&app);
    Ok(())
}

/// Active ou désactive un workflow.
#[tauri::command]
pub async fn set_workflow_enabled<R: Runtime>(
    app: AppHandle<R>,
    workflows: State<'_, Workflows>,
    id: WorkflowId,
    enabled: bool,
) -> Result<Workflow, CommandError> {
    let service = workflows.service()?;
    let workflow = blocking(move || Ok(service.set_enabled(id, enabled)?)).await?;
    changed(&app);
    Ok(workflow)
}

/// Réordonne les workflows ; renvoie la liste dans le nouvel ordre.
#[tauri::command]
pub async fn reorder_workflows<R: Runtime>(
    app: AppHandle<R>,
    workflows: State<'_, Workflows>,
    ordered: Vec<WorkflowId>,
) -> Result<Vec<Workflow>, CommandError> {
    let service = workflows.service()?;
    let list = blocking(move || Ok(service.reorder(&ordered)?)).await?;
    changed(&app);
    Ok(list)
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use minim_core::ports::SystemClock;
    use minim_core::testing::SequentialIds;
    use minim_core::testing::memory::InMemoryWorkflowRepository;
    use serde_json::{Value, json};
    use tauri::Listener as _;
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::{InvokeRequest, WebviewWindow, WebviewWindowBuilder};

    use super::*;

    /// Application simulée, avec la configuration et la capability réelles.
    fn app(workflows: Workflows) -> (tauri::App<MockRuntime>, WebviewWindow<MockRuntime>) {
        let app = mock_builder()
            .manage(workflows)
            .invoke_handler(crate::commands::handler())
            .build(tauri::generate_context!())
            .unwrap();
        let window = WebviewWindowBuilder::new(&app, "main", tauri::WebviewUrl::default())
            .build()
            .unwrap();
        (app, window)
    }

    fn service() -> Workflows {
        Workflows(Some(WorkflowService::new(
            Arc::new(InMemoryWorkflowRepository::default()),
            Arc::new(SystemClock),
            Arc::new(SequentialIds::default()),
        )))
    }

    fn call(window: &WebviewWindow<MockRuntime>, cmd: &str, body: Value) -> Result<Value, Value> {
        get_ipc_response(
            window,
            InvokeRequest {
                cmd: cmd.into(),
                callback: CallbackFn(0),
                error: CallbackFn(1),
                url: if cfg!(windows) {
                    "http://tauri.localhost"
                } else {
                    "tauri://localhost"
                }
                .parse()
                .unwrap(),
                body: InvokeBody::Json(body),
                headers: Default::default(),
                invoke_key: INVOKE_KEY.to_owned(),
            },
        )
        .map(|response| response.deserialize::<Value>().unwrap())
    }

    fn draft(name: &str) -> Value {
        json!({
            "name": name,
            "enabled": true,
            "continue_on_match": false,
            "trigger": {
                "match": "all",
                "conditions": [{ "type": "file_name_contains", "value": "facture" }],
                "source": "manual"
            },
            "actions": [{ "type": "rename", "template": "{date}.pdf" }]
        })
    }

    #[test]
    fn manages_workflows_through_the_allowed_commands() {
        let (app, window) = app(service());
        let events = Arc::new(Mutex::new(0));
        let counter = Arc::clone(&events);
        app.listen(WORKFLOWS_CHANGED, move |_| *counter.lock().unwrap() += 1);

        let created = call(
            &window,
            "create_workflow",
            json!({ "draft": draft("Factures") }),
        )
        .unwrap();
        let id = created["id"].clone();
        let updated = call(
            &window,
            "update_workflow",
            json!({ "id": id, "expectedVersion": 1, "draft": draft("Factures fournisseurs") }),
        )
        .unwrap();
        let stale = call(
            &window,
            "update_workflow",
            json!({ "id": id, "expectedVersion": 1, "draft": draft("Ancienne version") }),
        )
        .unwrap_err();
        let disabled = call(
            &window,
            "set_workflow_enabled",
            json!({ "id": id, "enabled": false }),
        )
        .unwrap();
        let second = call(
            &window,
            "create_workflow",
            json!({ "draft": draft("Devis") }),
        )
        .unwrap();
        let reordered = call(
            &window,
            "reorder_workflows",
            json!({ "ordered": [second["id"], id] }),
        )
        .unwrap();
        let fetched = call(&window, "get_workflow", json!({ "id": id })).unwrap();
        call(&window, "delete_workflow", json!({ "id": second["id"] })).unwrap();
        let listed = call(&window, "list_workflows", json!({})).unwrap();

        assert_eq!(updated["version"], 2);
        assert_eq!(stale["code"], "conflict");
        assert_eq!(disabled["enabled"], false);
        assert_eq!(reordered[0]["name"], "Devis");
        assert_eq!(fetched["priority"], 20);
        assert_eq!(listed.as_array().unwrap().len(), 1);
        assert_eq!(
            *events.lock().unwrap(),
            6,
            "un événement par modification réussie"
        );
    }

    #[test]
    fn validates_without_saving() {
        let (_app, window) = app(service());
        let mut invalid = draft(" ");
        invalid["actions"] = json!([]);

        let issues = call(&window, "validate_workflow", json!({ "draft": invalid })).unwrap();
        let refused = call(&window, "create_workflow", json!({ "draft": invalid })).unwrap_err();

        assert_eq!(
            issues,
            json!([{ "path": "name", "code": "empty_name" }, { "path": "actions", "code": "no_actions" }])
        );
        assert_eq!(refused["code"], "validation_failed");
        assert_eq!(refused["details"]["issues"], issues);
        assert_eq!(
            call(&window, "list_workflows", json!({})).unwrap(),
            json!([])
        );
    }

    #[test]
    fn reports_missing_items_and_storage() {
        let (_app, window) = app(service());
        let missing = call(
            &window,
            "get_workflow",
            json!({ "id": "wf_01J8ZK3V9Q4X7M2N5P6R8T0W1Y" }),
        );
        let missing = missing.unwrap_err();
        assert_eq!(missing["code"], "not_found", "{missing}");

        let (_app, window) = app(Workflows(None));
        let unavailable = call(&window, "list_workflows", json!({})).unwrap_err();
        assert_eq!(unavailable["code"], "storage_unavailable");
    }

    #[test]
    fn refuses_commands_the_capability_does_not_allow() {
        let (_app, window) = app(service());

        // Commande du noyau Tauri retirée de la capability (`core:default`).
        let refused = call(&window, "plugin:app|version", json!({}));

        assert!(refused.is_err(), "{refused:?}");
    }
}
