//! Dépôt du journal des exécutions.

use chrono::{DateTime, Utc};
use minim_core::ports::{ExecutionLogQuery, ExecutionLogRepository, StoreError};
use minim_core::{DocumentId, ExecutionLog, ExecutionStatus};
use rusqlite::{ErrorCode, ToSql, params};

use crate::database::Database;
use crate::errors::{corrupted, unavailable};

/// [`ExecutionLogRepository`] dans la base chiffrée.
#[derive(Debug, Clone)]
pub struct SqliteExecutionLogRepository {
    database: Database,
}

impl SqliteExecutionLogRepository {
    pub(crate) const fn new(database: Database) -> Self {
        Self { database }
    }

    fn query(&self, sql: &str, params: &[&dyn ToSql]) -> Result<Vec<ExecutionLog>, StoreError> {
        let connection = self.database.lock();
        let mut statement = connection.prepare(sql).map_err(unavailable)?;
        let bodies = statement
            .query_map(params, |row| row.get::<_, String>(0))
            .map_err(unavailable)?;
        bodies
            .map(|body| serde_json::from_str(&body.map_err(unavailable)?).map_err(corrupted))
            .collect()
    }
}

/// Date en nanosecondes depuis l'époque Unix, pour trier et filtrer.
fn nanos(date: DateTime<Utc>) -> Result<i64, StoreError> {
    date.timestamp_nanos_opt()
        .ok_or_else(|| corrupted(DateOutOfRange))
}

#[derive(Debug, thiserror::Error)]
#[error("date hors de la plage enregistrable (1677-2262)")]
struct DateOutOfRange;

/// Nom du statut, identique à sa forme JSON (vérifié par un test).
const fn status_name(status: ExecutionStatus) -> &'static str {
    match status {
        ExecutionStatus::Success => "success",
        ExecutionStatus::Failed => "failed",
        ExecutionStatus::SentToWaitingZone => "sent_to_waiting_zone",
    }
}

/// Valeur de `LIMIT` ou `OFFSET` : SQLite attend un entier signé.
fn sql_count(count: usize) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}

impl ExecutionLogRepository for SqliteExecutionLogRepository {
    fn append(&self, log: &ExecutionLog) -> Result<(), StoreError> {
        let body = serde_json::to_string(log).map_err(corrupted)?;
        let result = self.database.lock().execute(
            "INSERT INTO execution_logs (id, document_id, workflow_id, status, started_at, body)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
            params![
                log.id.to_string(),
                log.document_id.to_string(),
                log.workflow_id.to_string(),
                status_name(log.status),
                nanos(log.started_at)?,
                body
            ],
        );
        match result {
            Ok(_) => Ok(()),
            Err(error) if error.sqlite_error_code() == Some(ErrorCode::ConstraintViolation) => {
                Err(StoreError::AlreadyExists {
                    id: log.id.to_string(),
                })
            }
            Err(error) => Err(unavailable(error)),
        }
    }

    fn recent(&self, limit: usize) -> Result<Vec<ExecutionLog>, StoreError> {
        self.search(&ExecutionLogQuery {
            limit,
            ..ExecutionLogQuery::default()
        })
    }

    fn for_document(&self, document_id: DocumentId) -> Result<Vec<ExecutionLog>, StoreError> {
        self.query(
            "SELECT body FROM execution_logs WHERE document_id = ?1 ORDER BY started_at, id",
            &[&document_id.to_string()],
        )
    }

    fn search(&self, query: &ExecutionLogQuery) -> Result<Vec<ExecutionLog>, StoreError> {
        // Chaque filtre absent vaut NULL et devient toujours vrai.
        let workflow_id = query.workflow_id.map(|id| id.to_string());
        let status = query.status.map(status_name);
        let from = query.started_from.map(nanos).transpose()?;
        let before = query.started_before.map(nanos).transpose()?;
        let (limit, offset) = (sql_count(query.limit), sql_count(query.offset));
        self.query(
            "SELECT body FROM execution_logs
             WHERE (?1 IS NULL OR workflow_id = ?1)
               AND (?2 IS NULL OR status = ?2)
               AND (?3 IS NULL OR started_at >= ?3)
               AND (?4 IS NULL OR started_at < ?4)
             ORDER BY started_at DESC, id DESC
             LIMIT ?5 OFFSET ?6",
            &[&workflow_id, &status, &from, &before, &limit, &offset],
        )
    }

    fn purge_before(&self, cutoff: DateTime<Utc>) -> Result<usize, StoreError> {
        self.database
            .lock()
            .execute(
                "DELETE FROM execution_logs WHERE started_at < ?1",
                [nanos(cutoff)?],
            )
            .map_err(unavailable)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_statuses_like_their_json_form() {
        for status in [
            ExecutionStatus::Success,
            ExecutionStatus::Failed,
            ExecutionStatus::SentToWaitingZone,
        ] {
            assert_eq!(
                serde_json::to_value(status).unwrap(),
                serde_json::Value::from(status_name(status))
            );
        }
    }
}
