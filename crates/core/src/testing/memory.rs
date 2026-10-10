//! Dépôts en mémoire, pour les tests.

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard, PoisonError};

use crate::execution::ExecutionLog;
use crate::id::{DocumentId, RunId, WorkflowId};
use crate::ports::{ExecutionLogRepository, StoreError, WorkflowRepository};
use crate::workflow::Workflow;

/// Verrouille sans échouer : un test qui a paniqué en tenant le verrou ne
/// rend pas le dépôt inutilisable.
fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

/// [`WorkflowRepository`] en mémoire.
#[derive(Debug, Default)]
pub struct InMemoryWorkflowRepository {
    workflows: Mutex<BTreeMap<WorkflowId, Workflow>>,
}

impl WorkflowRepository for InMemoryWorkflowRepository {
    fn save(&self, workflow: &Workflow) -> Result<(), StoreError> {
        lock(&self.workflows).insert(workflow.id, workflow.clone());
        Ok(())
    }

    fn get(&self, id: WorkflowId) -> Result<Option<Workflow>, StoreError> {
        Ok(lock(&self.workflows).get(&id).cloned())
    }

    fn list(&self) -> Result<Vec<Workflow>, StoreError> {
        let mut workflows: Vec<_> = lock(&self.workflows).values().cloned().collect();
        workflows.sort_by_key(|workflow| (workflow.priority, workflow.id));
        Ok(workflows)
    }

    fn delete(&self, id: WorkflowId) -> Result<bool, StoreError> {
        Ok(lock(&self.workflows).remove(&id).is_some())
    }
}

/// [`ExecutionLogRepository`] en mémoire.
#[derive(Debug, Default)]
pub struct InMemoryExecutionLogRepository {
    logs: Mutex<BTreeMap<RunId, ExecutionLog>>,
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
        let mut logs: Vec<_> = lock(&self.logs).values().cloned().collect();
        logs.sort_by_key(|log| std::cmp::Reverse((log.started_at, log.id)));
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
}
