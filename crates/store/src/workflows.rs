//! Dépôt des workflows.

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, Utc};
use minim_core::ports::{PRIORITY_STEP, StoreError, WorkflowRepository};
use minim_core::{Workflow, WorkflowId};
use rusqlite::{OptionalExtension as _, Transaction, params};

use crate::database::Database;
use crate::errors::{corrupted, unavailable};

/// [`WorkflowRepository`] dans la base chiffrée.
#[derive(Debug, Clone)]
pub struct SqliteWorkflowRepository {
    database: Database,
}

impl SqliteWorkflowRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }
}

fn to_json(workflow: &Workflow) -> Result<String, StoreError> {
    serde_json::to_string(workflow).map_err(corrupted)
}

fn from_json(body: &str) -> Result<Workflow, StoreError> {
    serde_json::from_str(body).map_err(corrupted)
}

/// Écrit le workflow tel quel (insertion ou remplacement), hors tombstone.
fn write(transaction: &Transaction<'_>, workflow: &Workflow) -> Result<(), StoreError> {
    transaction
        .execute(
            "INSERT INTO workflows (id, priority, version, deleted, body) VALUES (?1, ?2, ?3, 0, ?4)
             ON CONFLICT (id) DO UPDATE SET
                 priority = excluded.priority,
                 version = excluded.version,
                 deleted = 0,
                 body = excluded.body",
            params![
                workflow.id.to_string(),
                workflow.priority,
                workflow.version,
                to_json(workflow)?
            ],
        )
        .map_err(unavailable)?;
    Ok(())
}

impl WorkflowRepository for SqliteWorkflowRepository {
    fn save(&self, workflow: &Workflow) -> Result<(), StoreError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(unavailable)?;
        let stored: Option<u32> = transaction
            .query_row(
                "SELECT version FROM workflows WHERE id = ?1",
                [workflow.id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        if let Some(stored) = stored
            && stored.checked_add(1) != Some(workflow.version)
        {
            return Err(StoreError::VersionConflict {
                id: workflow.id.to_string(),
                stored,
                attempted: workflow.version,
            });
        }
        write(&transaction, workflow)?;
        transaction.commit().map_err(unavailable)
    }

    fn get(&self, id: WorkflowId) -> Result<Option<Workflow>, StoreError> {
        let body: Option<String> = self
            .database
            .lock()
            .query_row(
                "SELECT body FROM workflows WHERE id = ?1 AND deleted = 0",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        body.as_deref().map(from_json).transpose()
    }

    fn list(&self) -> Result<Vec<Workflow>, StoreError> {
        let connection = self.database.lock();
        let mut statement = connection
            .prepare("SELECT body FROM workflows WHERE deleted = 0 ORDER BY priority, id")
            .map_err(unavailable)?;
        let bodies = statement
            .query_map([], |row| row.get::<_, String>(0))
            .map_err(unavailable)?;
        bodies
            .map(|body| from_json(&body.map_err(unavailable)?))
            .collect()
    }

    fn delete(&self, id: WorkflowId) -> Result<bool, StoreError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(unavailable)?;
        let body: Option<String> = transaction
            .query_row(
                "SELECT body FROM workflows WHERE id = ?1 AND deleted = 0",
                [id.to_string()],
                |row| row.get(0),
            )
            .optional()
            .map_err(unavailable)?;
        let Some(body) = body else {
            return Ok(false);
        };
        let mut tombstone = from_json(&body)?;
        tombstone.version = tombstone.version.saturating_add(1);
        transaction
            .execute(
                "UPDATE workflows SET version = ?2, deleted = 1, body = ?3 WHERE id = ?1",
                params![id.to_string(), tombstone.version, to_json(&tombstone)?],
            )
            .map_err(unavailable)?;
        transaction.commit().map_err(unavailable)?;
        Ok(true)
    }

    fn reorder(&self, ordered: &[WorkflowId], updated_at: DateTime<Utc>) -> Result<(), StoreError> {
        let mut connection = self.database.lock();
        let transaction = connection.transaction().map_err(unavailable)?;
        let mut live: BTreeMap<String, Workflow> = BTreeMap::new();
        {
            let mut statement = transaction
                .prepare("SELECT id, body FROM workflows WHERE deleted = 0")
                .map_err(unavailable)?;
            let rows = statement
                .query_map([], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .map_err(unavailable)?;
            for row in rows {
                let (id, body) = row.map_err(unavailable)?;
                live.insert(id, from_json(&body)?);
            }
        }

        let requested: BTreeSet<String> = ordered.iter().map(ToString::to_string).collect();
        if requested.len() != ordered.len() || !requested.iter().eq(live.keys()) {
            return Err(StoreError::InvalidOrder);
        }

        let mut priority = 0_u32;
        for id in ordered {
            priority = priority.saturating_add(PRIORITY_STEP);
            let Some(workflow) = live.get_mut(&id.to_string()) else {
                return Err(StoreError::InvalidOrder);
            };
            if workflow.priority != priority {
                workflow.priority = priority;
                workflow.version = workflow.version.saturating_add(1);
                workflow.updated_at = updated_at;
                write(&transaction, workflow)?;
            }
        }
        transaction.commit().map_err(unavailable)
    }
}
