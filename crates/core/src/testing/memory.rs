//! Dépôts en mémoire, pour les tests.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::{Mutex, MutexGuard, PoisonError};

use chrono::{DateTime, Utc};

use crate::document::{ContentHash, Document, DocumentStatus};
use crate::execution::ExecutionLog;
use crate::id::{DocumentId, RunId, WorkflowId};
use crate::ports::{
    DocumentRepository, ExecutionLogQuery, ExecutionLogRepository, PRIORITY_STEP, StoreError,
    WaitingZoneRepository, WorkflowRepository,
};
use crate::waiting_zone::WaitingEntry;
use crate::workflow::Workflow;

/// Verrouille sans échouer : un test qui a paniqué en tenant le verrou ne
/// rend pas le dépôt inutilisable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Workflow enregistré, ou tombstone s'il a été supprimé.
#[derive(Debug, Clone)]
struct Stored {
    workflow: Workflow,
    deleted: bool,
}

/// [`WorkflowRepository`] en mémoire.
#[derive(Debug, Default)]
pub struct InMemoryWorkflowRepository {
    workflows: Mutex<BTreeMap<WorkflowId, Stored>>,
}

impl WorkflowRepository for InMemoryWorkflowRepository {
    fn save(&self, workflow: &Workflow) -> Result<(), StoreError> {
        let mut workflows = lock(&self.workflows);
        if let Some(stored) = workflows.get(&workflow.id) {
            let current = stored.workflow.version;
            if current.checked_add(1) != Some(workflow.version) {
                return Err(StoreError::VersionConflict {
                    id: workflow.id.to_string(),
                    stored: current,
                    attempted: workflow.version,
                });
            }
        }
        workflows.insert(
            workflow.id,
            Stored {
                workflow: workflow.clone(),
                deleted: false,
            },
        );
        Ok(())
    }

    fn get(&self, id: WorkflowId) -> Result<Option<Workflow>, StoreError> {
        Ok(lock(&self.workflows)
            .get(&id)
            .filter(|stored| !stored.deleted)
            .map(|stored| stored.workflow.clone()))
    }

    fn list(&self) -> Result<Vec<Workflow>, StoreError> {
        let mut workflows: Vec<_> = lock(&self.workflows)
            .values()
            .filter(|stored| !stored.deleted)
            .map(|stored| stored.workflow.clone())
            .collect();
        workflows.sort_by_key(|workflow| (workflow.priority, workflow.id));
        Ok(workflows)
    }

    fn delete(&self, id: WorkflowId) -> Result<bool, StoreError> {
        let mut workflows = lock(&self.workflows);
        match workflows.get_mut(&id) {
            Some(stored) if !stored.deleted => {
                stored.deleted = true;
                stored.workflow.version = stored.workflow.version.saturating_add(1);
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    fn reorder(&self, ordered: &[WorkflowId], updated_at: DateTime<Utc>) -> Result<(), StoreError> {
        let mut workflows = lock(&self.workflows);
        let live: BTreeSet<_> = workflows
            .iter()
            .filter(|(_, stored)| !stored.deleted)
            .map(|(id, _)| *id)
            .collect();
        let requested: BTreeSet<_> = ordered.iter().copied().collect();
        if requested.len() != ordered.len() || requested != live {
            return Err(StoreError::InvalidOrder);
        }
        let mut priority = 0_u32;
        for id in ordered {
            priority = priority.saturating_add(PRIORITY_STEP);
            if let Some(stored) = workflows.get_mut(id) {
                let workflow = &mut stored.workflow;
                if workflow.priority != priority {
                    workflow.priority = priority;
                    workflow.version = workflow.version.saturating_add(1);
                    workflow.updated_at = updated_at;
                }
            }
        }
        Ok(())
    }
}

/// [`ExecutionLogRepository`] en mémoire.
#[derive(Debug, Default)]
pub struct InMemoryExecutionLogRepository {
    logs: Mutex<BTreeMap<RunId, ExecutionLog>>,
}

impl InMemoryExecutionLogRepository {
    /// Exécutions de la plus récente à la plus ancienne.
    fn newest_first(&self) -> Vec<ExecutionLog> {
        let mut logs: Vec<_> = lock(&self.logs).values().cloned().collect();
        logs.sort_by_key(|log| std::cmp::Reverse((log.started_at, log.id)));
        logs
    }
}

impl ExecutionLogRepository for InMemoryExecutionLogRepository {
    fn append(&self, log: &ExecutionLog) -> Result<(), StoreError> {
        let mut logs = lock(&self.logs);
        if logs.contains_key(&log.id) {
            return Err(StoreError::AlreadyExists {
                id: log.id.to_string(),
            });
        }
        logs.insert(log.id, log.clone());
        Ok(())
    }

    fn recent(&self, limit: usize) -> Result<Vec<ExecutionLog>, StoreError> {
        let mut logs = self.newest_first();
        logs.truncate(limit);
        Ok(logs)
    }

    fn for_document(&self, document_id: DocumentId) -> Result<Vec<ExecutionLog>, StoreError> {
        let mut logs: Vec<_> = lock(&self.logs)
            .values()
            .filter(|log| log.document_id == document_id)
            .cloned()
            .collect();
        logs.sort_by_key(|log| (log.started_at, log.id));
        Ok(logs)
    }

    fn search(&self, query: &ExecutionLogQuery) -> Result<Vec<ExecutionLog>, StoreError> {
        Ok(self
            .newest_first()
            .into_iter()
            .filter(|log| query.workflow_id.is_none_or(|id| log.workflow_id == id))
            .filter(|log| query.status.is_none_or(|status| log.status == status))
            .filter(|log| query.started_from.is_none_or(|from| log.started_at >= from))
            .filter(|log| {
                query
                    .started_before
                    .is_none_or(|before| log.started_at < before)
            })
            .skip(query.offset)
            .take(query.limit)
            .collect())
    }

    fn purge_before(&self, cutoff: DateTime<Utc>) -> Result<usize, StoreError> {
        let mut logs = lock(&self.logs);
        let before = logs.len();
        logs.retain(|_, log| log.started_at >= cutoff);
        Ok(before - logs.len())
    }
}

/// [`DocumentRepository`] en mémoire.
#[derive(Debug, Default)]
pub struct InMemoryDocumentRepository {
    documents: Mutex<BTreeMap<DocumentId, Document>>,
}

impl DocumentRepository for InMemoryDocumentRepository {
    fn save(&self, document: &Document) -> Result<(), StoreError> {
        lock(&self.documents).insert(document.id, document.clone());
        Ok(())
    }

    fn get(&self, id: DocumentId) -> Result<Option<Document>, StoreError> {
        Ok(lock(&self.documents).get(&id).cloned())
    }

    fn find_by_hash(&self, sha256: &ContentHash) -> Result<Option<Document>, StoreError> {
        Ok(lock(&self.documents)
            .values()
            .filter(|document| document.sha256 == *sha256)
            .min_by_key(|document| (document.received_at, document.id))
            .cloned())
    }

    fn update_status(&self, id: DocumentId, status: DocumentStatus) -> Result<bool, StoreError> {
        let mut documents = lock(&self.documents);
        let Some(document) = documents.get_mut(&id) else {
            return Ok(false);
        };
        document
            .transition(status)
            .map_err(|error| StoreError::InvalidTransition {
                id: id.to_string(),
                from: error.from,
                to: error.to,
            })?;
        Ok(true)
    }

    fn list_by_status(&self, status: DocumentStatus) -> Result<Vec<Document>, StoreError> {
        let mut documents: Vec<_> = lock(&self.documents)
            .values()
            .filter(|document| document.status == status)
            .cloned()
            .collect();
        documents.sort_by_key(|document| (document.received_at, document.id));
        Ok(documents)
    }
}

/// [`WaitingZoneRepository`] en mémoire.
#[derive(Debug, Default)]
pub struct InMemoryWaitingZoneRepository {
    entries: Mutex<BTreeMap<DocumentId, WaitingEntry>>,
}

impl WaitingZoneRepository for InMemoryWaitingZoneRepository {
    fn put(&self, entry: &WaitingEntry) -> Result<(), StoreError> {
        lock(&self.entries).insert(entry.document_id, entry.clone());
        Ok(())
    }

    fn list(&self) -> Result<Vec<WaitingEntry>, StoreError> {
        let mut entries: Vec<_> = lock(&self.entries).values().cloned().collect();
        entries.sort_by_key(|entry| (entry.entered_at, entry.document_id));
        Ok(entries)
    }

    fn remove(&self, document_id: DocumentId) -> Result<bool, StoreError> {
        Ok(lock(&self.entries).remove(&document_id).is_some())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::contracts;

    #[test]
    fn workflow_repository_passes_the_contract() {
        contracts::workflow_repository_contract(&InMemoryWorkflowRepository::default());
    }

    #[test]
    fn execution_log_repository_passes_the_contract() {
        contracts::execution_log_repository_contract(&InMemoryExecutionLogRepository::default());
    }

    #[test]
    fn document_repository_passes_the_contract() {
        contracts::document_repository_contract(&InMemoryDocumentRepository::default());
    }

    #[test]
    fn waiting_zone_repository_passes_the_contract() {
        contracts::waiting_zone_repository_contract(&InMemoryWaitingZoneRepository::default());
    }
}
