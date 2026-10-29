//! Commandes Tauri appelées par l'interface (via `invokeCommand`).
//!
//! Ce sont des adaptateurs fins : elles délèguent aux services de
//! `minim-core`, exécutent les appels bloquants hors du fil principal et
//! convertissent les erreurs en `CommandError`. Chaque commande doit être
//! déclarée dans `build.rs` et autorisée dans `capabilities/default.json`.

pub mod account;
pub mod storage;
#[cfg(test)]
mod testing;
pub mod trials;
pub mod workflows;

use tauri::Runtime;
use tauri::ipc::Invoke;

use crate::error::CommandError;

/// Toutes les commandes de l'application (liste à garder alignée sur
/// `build.rs`).
pub fn handler<R: Runtime>() -> impl Fn(Invoke<R>) -> bool + Send + Sync + 'static {
    tauri::generate_handler![
        storage::storage_status,
        workflows::list_workflows,
        workflows::get_workflow,
        workflows::validate_workflow,
        workflows::preview_name,
        workflows::create_workflow,
        workflows::update_workflow,
        workflows::delete_workflow,
        workflows::set_workflow_enabled,
        workflows::reorder_workflows,
        trials::preview_workflow_on_file,
        trials::run_workflow_on_file,
        account::account_status,
        account::sign_in,
        account::sign_up,
        account::sign_out,
        account::request_password_reset,
    ]
}

/// Exécute `work` sur un fil du pool bloquant de Tauri.
async fn blocking<T, F>(work: F) -> Result<T, CommandError>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, CommandError> + Send + 'static,
{
    tauri::async_runtime::spawn_blocking(work)
        .await
        .map_err(|error| CommandError::internal(&error))?
}
