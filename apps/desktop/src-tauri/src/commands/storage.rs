//! État du stockage local.

use tauri::State;

use crate::storage::{Storage, StorageStatus};

/// État du stockage local : ouvert ou non, protection réduite, ancienne base
/// mise de côté au démarrage.
#[tauri::command]
#[must_use]
pub fn storage_status(storage: State<'_, Storage>) -> StorageStatus {
    storage.status
}
