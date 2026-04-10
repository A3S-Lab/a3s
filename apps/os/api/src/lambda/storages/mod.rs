use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::SystemTime;
use uuid::Uuid;

use crate::domain::{LambdaTask, Result, TaskStatus};

pub mod pg;

/// Task record for persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRecord {
    pub id: String,
    pub task: LambdaTask,
    pub status: TaskStatus,
    pub lane: String,
    pub created_at: SystemTime,
    pub updated_at: SystemTime,
    pub started_at: Option<SystemTime>,
    pub completed_at: Option<SystemTime>,
    pub result: Option<String>,
    pub error: Option<String>,
}

impl TaskRecord {
    pub fn parent_invocation_id(&self) -> Option<Uuid> {
        self.task.parent_invocation_id()
    }
}

/// Task storage trait for persistence.
#[async_trait]
pub trait TaskStore: Send + Sync {
    async fn save(&self, record: TaskRecord) -> Result<()>;

    async fn cancel(&self, task_id: &str, error: Option<String>) -> Result<bool>;

    async fn update_status(
        &self,
        task_id: &str,
        status: TaskStatus,
        result: Option<String>,
        error: Option<String>,
    ) -> Result<()>;

    async fn get(&self, task_id: &str) -> Result<Option<TaskRecord>>;

    async fn list_by_status(&self, status: TaskStatus) -> Result<Vec<TaskRecord>>;

    async fn list_by_parent_invocation(
        &self,
        parent_invocation_id: &str,
        status: Option<TaskStatus>,
    ) -> Result<Vec<TaskRecord>>;

    async fn list_pending(&self) -> Result<Vec<TaskRecord>>;

    async fn cleanup_old_tasks(&self, older_than: std::time::Duration) -> Result<usize>;
}
