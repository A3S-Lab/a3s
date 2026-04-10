use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{postgres::PgPoolOptions, query, FromRow, PgPool};
use std::time::{Duration, SystemTime};
use uuid::Uuid;

use crate::domain::{
    LambdaError, LambdaRepository, LambdaTask, Result, TaskKind, TaskStats, TaskStatus,
};
use crate::queue::select_lane;
use crate::storages::{TaskRecord, TaskStore};

#[allow(dead_code)]
#[derive(FromRow)]
struct TaskRow {
    id: Uuid,
    batch_id: Option<Uuid>,
    kind: serde_json::Value,
    timeout_secs: i32,
    status: String,
    result: Option<serde_json::Value>,
    error: Option<String>,
    worker_id: Option<String>,
    lease_expires_at: Option<DateTime<Utc>>,
    heartbeat_at: Option<DateTime<Utc>>,
    created_at: DateTime<Utc>,
    started_at: Option<DateTime<Utc>>,
    finished_at: Option<DateTime<Utc>>,
}

impl TryFrom<TaskRow> for LambdaTask {
    type Error = LambdaError;

    fn try_from(row: TaskRow) -> Result<Self> {
        let kind: TaskKind = serde_json::from_value(row.kind)
            .map_err(|e| LambdaError::Internal(format!("failed to deserialize task kind: {e}")))?;

        Ok(LambdaTask {
            id: row.id,
            batch_id: row.batch_id,
            kind,
            timeout_secs: row.timeout_secs as u32,
            status: parse_status(&row.status)?,
            result: row.result,
            error: row.error,
            created_at: row.created_at,
            started_at: row.started_at,
            finished_at: row.finished_at,
        })
    }
}

#[derive(FromRow)]
struct StatsRow {
    total: Option<i64>,
    pending: Option<i64>,
    running: Option<i64>,
    completed: Option<i64>,
    failed: Option<i64>,
    cancelled: Option<i64>,
}

#[derive(FromRow)]
struct LeaseHealthRow {
    running_with_expired_lease: Option<i64>,
    running_without_lease: Option<i64>,
    stale_heartbeats: Option<i64>,
}

#[derive(FromRow)]
struct LeaseRow {
    id: Uuid,
    worker_id: String,
    lease_expires_at: DateTime<Utc>,
    heartbeat_at: DateTime<Utc>,
}

pub struct PgLambdaRepository {
    pool: PgPool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskLease {
    pub task_id: Uuid,
    pub worker_id: String,
    pub lease_expires_at: DateTime<Utc>,
    pub heartbeat_at: DateTime<Utc>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct LeaseHealthStats {
    pub running_with_expired_lease: u64,
    pub running_without_lease: u64,
    pub stale_heartbeats: u64,
}

impl PgLambdaRepository {
    pub async fn connect(database_url: &str, max_connections: u32) -> Result<Self> {
        let pool = PgPoolOptions::new()
            .max_connections(max_connections)
            .connect(database_url)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;

        let repo = Self::new(pool);
        repo.migrate().await?;
        Ok(repo)
    }

    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn migrate(&self) -> Result<()> {
        query(include_str!("migrations/0001_lambda_tasks.sql"))
            .execute(&self.pool)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;

        query("ALTER TABLE lambda_tasks ADD COLUMN IF NOT EXISTS worker_id TEXT")
            .execute(&self.pool)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;
        query("ALTER TABLE lambda_tasks ADD COLUMN IF NOT EXISTS lease_expires_at TIMESTAMPTZ")
            .execute(&self.pool)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;
        query("ALTER TABLE lambda_tasks ADD COLUMN IF NOT EXISTS heartbeat_at TIMESTAMPTZ")
            .execute(&self.pool)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;
        query("CREATE INDEX IF NOT EXISTS idx_lambda_tasks_worker_id ON lambda_tasks(worker_id)")
            .execute(&self.pool)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;
        query(
            "CREATE INDEX IF NOT EXISTS idx_lambda_tasks_lease_expires_at ON lambda_tasks(lease_expires_at)",
        )
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn health_check(&self) -> Result<()> {
        query("SELECT 1")
            .execute(&self.pool)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;
        Ok(())
    }

    pub async fn lease_health_stats(
        &self,
        stale_heartbeat_after: Duration,
    ) -> Result<LeaseHealthStats> {
        let stale_before = Utc::now()
            - chrono::Duration::from_std(stale_heartbeat_after).map_err(|e| {
                LambdaError::Internal(format!("invalid stale heartbeat duration: {e}"))
            })?;

        let row = sqlx::query_as::<_, LeaseHealthRow>(
            r#"
            SELECT
                COUNT(*) FILTER (
                    WHERE status = 'running'
                      AND lease_expires_at IS NOT NULL
                      AND lease_expires_at < NOW()
                ) as running_with_expired_lease,
                COUNT(*) FILTER (
                    WHERE status = 'running'
                      AND (worker_id IS NULL OR lease_expires_at IS NULL OR heartbeat_at IS NULL)
                ) as running_without_lease,
                COUNT(*) FILTER (
                    WHERE status = 'running'
                      AND heartbeat_at IS NOT NULL
                      AND heartbeat_at < $1
                ) as stale_heartbeats
            FROM lambda_tasks
            "#,
        )
        .bind(stale_before)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(LeaseHealthStats {
            running_with_expired_lease: row.running_with_expired_lease.unwrap_or(0) as u64,
            running_without_lease: row.running_without_lease.unwrap_or(0) as u64,
            stale_heartbeats: row.stale_heartbeats.unwrap_or(0) as u64,
        })
    }
}

#[async_trait]
impl LambdaRepository for PgLambdaRepository {
    async fn create_task(&self, task: &LambdaTask) -> Result<()> {
        let kind_json = serde_json::to_value(&task.kind)
            .map_err(|e| LambdaError::Internal(format!("failed to serialize task kind: {e}")))?;

        sqlx::query(
            r#"
            INSERT INTO lambda_tasks (
                id, batch_id, kind, timeout_secs, status,
                result, error, created_at, started_at, finished_at
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
        )
        .bind(task.id)
        .bind(task.batch_id)
        .bind(kind_json)
        .bind(task.timeout_secs as i32)
        .bind(task.status.to_string())
        .bind(&task.result)
        .bind(&task.error)
        .bind(task.created_at)
        .bind(task.started_at)
        .bind(task.finished_at)
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(())
    }

    async fn find_task(&self, id: Uuid) -> Result<Option<LambdaTask>> {
        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, batch_id, kind, timeout_secs, status,
                   result, error, worker_id, lease_expires_at, heartbeat_at,
                   created_at, started_at, finished_at
            FROM lambda_tasks
            WHERE id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        row.map(LambdaTask::try_from).transpose()
    }

    async fn find_batch_tasks(&self, batch_id: Uuid) -> Result<Vec<LambdaTask>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, batch_id, kind, timeout_secs, status,
                   result, error, worker_id, lease_expires_at, heartbeat_at,
                   created_at, started_at, finished_at
            FROM lambda_tasks
            WHERE batch_id = $1
            ORDER BY created_at
            "#,
        )
        .bind(batch_id)
        .fetch_all(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        rows.into_iter().map(LambdaTask::try_from).collect()
    }

    async fn cancel_task(&self, id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE lambda_tasks
            SET status = 'cancelled', finished_at = NOW()
            WHERE id = $1 AND status = 'pending'
            "#,
        )
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn claim_next(&self, worker_id: &str, lease_ttl: Duration) -> Result<Option<LambdaTask>> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;

        let lease_expires_at = Utc::now()
            + chrono::Duration::from_std(lease_ttl)
                .map_err(|e| LambdaError::Internal(format!("invalid lease ttl: {e}")))?;

        let row = sqlx::query_as::<_, TaskRow>(
            r#"
            SELECT id, batch_id, kind, timeout_secs, status,
                   result, error, worker_id, lease_expires_at, heartbeat_at,
                   created_at, started_at, finished_at
            FROM lambda_tasks
            WHERE status = 'pending'
               OR (status = 'running' AND lease_expires_at IS NOT NULL AND lease_expires_at < NOW())
            ORDER BY created_at
            LIMIT 1
            FOR UPDATE SKIP LOCKED
            "#,
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        if let Some(row) = row {
            sqlx::query(
                r#"
                UPDATE lambda_tasks
                SET status = 'running',
                    worker_id = $2,
                    started_at = COALESCE(started_at, NOW()),
                    heartbeat_at = NOW(),
                    lease_expires_at = $3
                WHERE id = $1
                "#,
            )
            .bind(row.id)
            .bind(worker_id)
            .bind(lease_expires_at)
            .execute(&mut *tx)
            .await
            .map_err(|e| LambdaError::Database(e.to_string()))?;

            tx.commit()
                .await
                .map_err(|e| LambdaError::Database(e.to_string()))?;

            let mut task = LambdaTask::try_from(row)?;
            task.status = TaskStatus::Running;
            task.started_at = Some(Utc::now());
            Ok(Some(task))
        } else {
            tx.rollback()
                .await
                .map_err(|e| LambdaError::Database(e.to_string()))?;
            Ok(None)
        }
    }

    async fn renew_lease(&self, id: Uuid, worker_id: &str, lease_ttl: Duration) -> Result<bool> {
        let lease_expires_at = Utc::now()
            + chrono::Duration::from_std(lease_ttl)
                .map_err(|e| LambdaError::Internal(format!("invalid lease ttl: {e}")))?;

        let result = sqlx::query(
            r#"
            UPDATE lambda_tasks
            SET heartbeat_at = NOW(),
                lease_expires_at = $3
            WHERE id = $1
              AND worker_id = $2
              AND status = 'running'
            "#,
        )
        .bind(id)
        .bind(worker_id)
        .bind(lease_expires_at)
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn release_expired_leases(&self) -> Result<u64> {
        let result = sqlx::query(
            r#"
            UPDATE lambda_tasks
            SET status = 'pending',
                worker_id = NULL,
                heartbeat_at = NULL,
                lease_expires_at = NULL
            WHERE status = 'running'
              AND lease_expires_at IS NOT NULL
              AND lease_expires_at < NOW()
            "#,
        )
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(result.rows_affected())
    }

    async fn complete_task(&self, id: Uuid, result: serde_json::Value) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE lambda_tasks
            SET status = 'completed', result = $1, finished_at = NOW(),
                worker_id = NULL, heartbeat_at = NULL, lease_expires_at = NULL
            WHERE id = $2
            "#,
        )
        .bind(result)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(())
    }

    async fn fail_task(&self, id: Uuid, error: &str) -> Result<()> {
        sqlx::query(
            r#"
            UPDATE lambda_tasks
            SET status = 'failed', error = $1, finished_at = NOW(),
                worker_id = NULL, heartbeat_at = NULL, lease_expires_at = NULL
            WHERE id = $2
            "#,
        )
        .bind(error)
        .bind(id)
        .execute(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(())
    }

    async fn get_stats(&self) -> Result<TaskStats> {
        let row = sqlx::query_as::<_, StatsRow>(
            r#"
            SELECT
                COUNT(*) as total,
                COUNT(*) FILTER (WHERE status = 'pending') as pending,
                COUNT(*) FILTER (WHERE status = 'running') as running,
                COUNT(*) FILTER (WHERE status = 'completed') as completed,
                COUNT(*) FILTER (WHERE status = 'failed') as failed,
                COUNT(*) FILTER (WHERE status = 'cancelled') as cancelled
            FROM lambda_tasks
            "#,
        )
        .fetch_one(&self.pool)
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(TaskStats {
            total: row.total.unwrap_or(0) as u64,
            pending: row.pending.unwrap_or(0) as u64,
            running: row.running.unwrap_or(0) as u64,
            completed: row.completed.unwrap_or(0) as u64,
            failed: row.failed.unwrap_or(0) as u64,
            cancelled: row.cancelled.unwrap_or(0) as u64,
        })
    }
}

/// PostgreSQL-backed task storage for the Lambda runtime.
pub struct PgTaskStore {
    repo: PgLambdaRepository,
}

impl PgTaskStore {
    pub async fn new(database_url: &str, max_connections: u32) -> Result<Self> {
        let repo = PgLambdaRepository::connect(database_url, max_connections).await?;
        Ok(Self { repo })
    }

    pub fn repository(&self) -> &PgLambdaRepository {
        &self.repo
    }

    pub async fn health_check(&self) -> Result<()> {
        self.repo.health_check().await
    }

    pub async fn lease_health_stats(
        &self,
        stale_heartbeat_after: Duration,
    ) -> Result<LeaseHealthStats> {
        self.repo.lease_health_stats(stale_heartbeat_after).await
    }

    pub async fn claim_next_with_lease(
        &self,
        worker_id: &str,
        lease_ttl: Duration,
    ) -> Result<Option<TaskRecord>> {
        self.repo
            .claim_next(worker_id, lease_ttl)
            .await?
            .map(faas_task_to_record)
            .transpose()
    }

    pub async fn renew_lease(
        &self,
        task_id: &str,
        worker_id: &str,
        lease_ttl: Duration,
    ) -> Result<bool> {
        self.repo
            .renew_lease(parse_task_id(task_id)?, worker_id, lease_ttl)
            .await
    }

    pub async fn release_expired_leases(&self) -> Result<u64> {
        self.repo.release_expired_leases().await
    }

    pub async fn get_task_lease(&self, task_id: &str) -> Result<Option<TaskLease>> {
        let task_id = parse_task_id(task_id)?;
        let row = sqlx::query_as::<_, LeaseRow>(
            "SELECT id, worker_id, lease_expires_at, heartbeat_at
             FROM lambda_tasks
             WHERE id = $1 AND worker_id IS NOT NULL AND lease_expires_at IS NOT NULL AND heartbeat_at IS NOT NULL",
        )
        .bind(task_id)
        .fetch_optional(self.repo.pool())
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        row.map(lease_row_to_lease).transpose()
    }
}

#[async_trait]
impl TaskStore for PgTaskStore {
    async fn save(&self, record: TaskRecord) -> Result<()> {
        self.repo.create_task(&record.task).await
    }

    async fn cancel(&self, task_id: &str, error: Option<String>) -> Result<bool> {
        let task_id = parse_task_id(task_id)?;

        let result = sqlx::query(
            "UPDATE lambda_tasks
             SET status = 'cancelled', error = $2, finished_at = NOW(),
                 worker_id = NULL, heartbeat_at = NULL, lease_expires_at = NULL
             WHERE id = $1 AND status = 'pending'",
        )
        .bind(task_id)
        .bind(error)
        .execute(self.repo.pool())
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(result.rows_affected() > 0)
    }

    async fn update_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        result: Option<String>,
        error: Option<String>,
    ) -> Result<()> {
        let task_id = parse_task_id(task_id)?;

        match status {
            TaskStatus::Pending => {
                sqlx::query(
                    "UPDATE lambda_tasks
                     SET status = 'pending', result = NULL, error = NULL, started_at = NULL, finished_at = NULL,
                         worker_id = NULL, heartbeat_at = NULL, lease_expires_at = NULL
                     WHERE id = $1",
                )
                .bind(task_id)
                .execute(self.repo.pool())
                .await
                .map_err(|e| LambdaError::Database(e.to_string()))?;
                Ok(())
            }
            TaskStatus::Running => {
                sqlx::query(
                    "UPDATE lambda_tasks
                     SET status = 'running', started_at = COALESCE(started_at, NOW()), error = NULL
                     WHERE id = $1",
                )
                .bind(task_id)
                .execute(self.repo.pool())
                .await
                .map_err(|e| LambdaError::Database(e.to_string()))?;
                Ok(())
            }
            TaskStatus::Completed => {
                let result_json = parse_optional_json(result)?;
                self.repo
                    .complete_task(task_id, result_json.unwrap_or(serde_json::Value::Null))
                    .await
            }
            TaskStatus::Failed => {
                self.repo
                    .fail_task(task_id, error.as_deref().unwrap_or("task failed"))
                    .await
            }
            TaskStatus::Cancelled => {
                sqlx::query(
                    "UPDATE lambda_tasks
                     SET status = 'cancelled', error = $2, finished_at = NOW(),
                         worker_id = NULL, heartbeat_at = NULL, lease_expires_at = NULL
                     WHERE id = $1",
                )
                .bind(task_id)
                .bind(error)
                .execute(self.repo.pool())
                .await
                .map_err(|e| LambdaError::Database(e.to_string()))?;
                Ok(())
            }
        }
    }

    async fn get(&self, task_id: &str) -> Result<Option<TaskRecord>> {
        let task_id = parse_task_id(task_id)?;
        let row = sqlx::query_as::<_, TaskRow>(
            "SELECT id, batch_id, kind, timeout_secs, status,
                    result, error, worker_id, lease_expires_at, heartbeat_at,
                    created_at, started_at, finished_at
             FROM lambda_tasks
             WHERE id = $1",
        )
        .bind(task_id)
        .fetch_optional(self.repo.pool())
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        row.map(task_row_to_record).transpose()
    }

    async fn list_by_status(&self, status: TaskStatus) -> Result<Vec<TaskRecord>> {
        let rows = sqlx::query_as::<_, TaskRow>(
            "SELECT id, batch_id, kind, timeout_secs, status,
                    result, error, worker_id, lease_expires_at, heartbeat_at,
                    created_at, started_at, finished_at
             FROM lambda_tasks
             WHERE status = $1
             ORDER BY created_at ASC",
        )
        .bind(status.to_string())
        .fetch_all(self.repo.pool())
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        rows.into_iter().map(task_row_to_record).collect()
    }

    async fn list_by_parent_invocation(
        &self,
        parent_invocation_id: &str,
        status: Option<TaskStatus>,
    ) -> Result<Vec<TaskRecord>> {
        let parent_invocation_id = parse_task_id(parent_invocation_id)?;

        let rows = match status {
            Some(status) => {
                sqlx::query_as::<_, TaskRow>(
                    "SELECT id, batch_id, kind, timeout_secs, status,
                            result, error, worker_id, lease_expires_at, heartbeat_at,
                            created_at, started_at, finished_at
                     FROM lambda_tasks
                     WHERE batch_id = $1 AND status = $2
                     ORDER BY created_at ASC",
                )
                .bind(parent_invocation_id)
                .bind(status.to_string())
                .fetch_all(self.repo.pool())
                .await
            }
            None => {
                sqlx::query_as::<_, TaskRow>(
                    "SELECT id, batch_id, kind, timeout_secs, status,
                            result, error, worker_id, lease_expires_at, heartbeat_at,
                            created_at, started_at, finished_at
                     FROM lambda_tasks
                     WHERE batch_id = $1
                     ORDER BY created_at ASC",
                )
                .bind(parent_invocation_id)
                .fetch_all(self.repo.pool())
                .await
            }
        }
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        rows.into_iter().map(task_row_to_record).collect()
    }

    async fn list_pending(&self) -> Result<Vec<TaskRecord>> {
        self.list_by_status(TaskStatus::Pending).await
    }

    async fn cleanup_old_tasks(&self, older_than: Duration) -> Result<usize> {
        let cutoff = Utc::now()
            - chrono::Duration::from_std(older_than)
                .map_err(|e| LambdaError::Internal(format!("invalid cleanup duration: {e}")))?;

        let result = sqlx::query(
            "DELETE FROM lambda_tasks
             WHERE status IN ('completed', 'failed', 'cancelled') AND finished_at < $1",
        )
        .bind(cutoff)
        .execute(self.repo.pool())
        .await
        .map_err(|e| LambdaError::Database(e.to_string()))?;

        Ok(result.rows_affected() as usize)
    }
}

fn task_row_to_record(row: TaskRow) -> Result<TaskRecord> {
    let task = LambdaTask::try_from(row)?;
    faas_task_to_record(task)
}

fn faas_task_to_record(task: LambdaTask) -> Result<TaskRecord> {
    let lane = select_lane(&task).to_string();

    Ok(TaskRecord {
        id: task.id.to_string(),
        status: task.status,
        lane,
        created_at: datetime_to_system_time(task.created_at),
        updated_at: datetime_to_system_time(
            task.finished_at
                .or(task.started_at)
                .unwrap_or(task.created_at),
        ),
        started_at: task.started_at.map(datetime_to_system_time),
        completed_at: task.finished_at.map(datetime_to_system_time),
        result: task.result.as_ref().map(|value| value.to_string()),
        error: task.error.clone(),
        task,
    })
}

fn lease_row_to_lease(row: LeaseRow) -> Result<TaskLease> {
    Ok(TaskLease {
        task_id: row.id,
        worker_id: row.worker_id,
        lease_expires_at: row.lease_expires_at,
        heartbeat_at: row.heartbeat_at,
    })
}

fn datetime_to_system_time(dt: DateTime<Utc>) -> SystemTime {
    use std::time::{Duration as StdDuration, UNIX_EPOCH};

    if dt.timestamp() >= 0 {
        UNIX_EPOCH + StdDuration::from_secs(dt.timestamp() as u64)
    } else {
        UNIX_EPOCH
    }
}

fn parse_status(value: &str) -> Result<TaskStatus> {
    match value {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "cancelled" => Ok(TaskStatus::Cancelled),
        other => Err(LambdaError::Internal(format!(
            "invalid task status in database: {other}"
        ))),
    }
}

fn parse_task_id(task_id: &str) -> Result<Uuid> {
    Uuid::parse_str(task_id)
        .map_err(|e| LambdaError::Internal(format!("invalid task id {task_id}: {e}")))
}

fn parse_optional_json(input: Option<String>) -> Result<Option<serde_json::Value>> {
    input
        .map(|value| {
            serde_json::from_str(&value).map_err(|e| {
                LambdaError::Internal(format!("failed to deserialize task result: {e}"))
            })
        })
        .transpose()
}
