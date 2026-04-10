use async_trait::async_trait;
use std::time::Duration;
use uuid::Uuid;

use crate::{LambdaError, LambdaTask};

type Result<T> = std::result::Result<T, LambdaError>;

#[async_trait]
pub trait LambdaRepository: Send + Sync {
    async fn create_task(&self, task: &LambdaTask) -> Result<()>;

    async fn find_task(&self, id: Uuid) -> Result<Option<LambdaTask>>;

    async fn find_batch_tasks(&self, batch_id: Uuid) -> Result<Vec<LambdaTask>>;

    async fn cancel_task(&self, id: Uuid) -> Result<bool>;

    async fn claim_next(&self, worker_id: &str, lease_ttl: Duration) -> Result<Option<LambdaTask>>;

    async fn renew_lease(&self, id: Uuid, worker_id: &str, lease_ttl: Duration) -> Result<bool>;

    async fn release_expired_leases(&self) -> Result<u64>;

    async fn complete_task(&self, id: Uuid, result: serde_json::Value) -> Result<()>;

    async fn fail_task(&self, id: Uuid, error: &str) -> Result<()>;

    async fn get_stats(&self) -> Result<TaskStats>;
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TaskStats {
    pub total: u64,
    pub pending: u64,
    pub running: u64,
    pub completed: u64,
    pub failed: u64,
    pub cancelled: u64,
}
