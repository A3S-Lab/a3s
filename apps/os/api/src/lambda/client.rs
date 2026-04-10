use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use crate::config::default_runtime_dir;
use crate::config::DatabaseConfig;
use crate::{AgentInvocationRequest, ExecutionTaskRequest, ExecutionTaskSubmissionRequest};
use a3s_lane::QueueManager;
use serde::{Deserialize, Serialize};

use crate::domain::{
    LambdaError, LambdaTask, Result, TaskKind, TaskRequest, TaskResult, TaskStatus,
};
use crate::executor::{agent::AgentExecutor, ExecutionRegistry};
use crate::queue::{create_faas_queue, select_lane, LambdaCommand};
use crate::storages::pg::PgTaskStore;
use crate::storages::{TaskRecord, TaskStore};

/// High-level Lambda client with integrated task queue
///
/// This is the main entry point for SDK users. It provides a simple API
/// for executing tasks with automatic queuing, priority scheduling, and retry logic.
#[derive(Clone)]
pub struct LambdaClient {
    queue_manager: Arc<QueueManager>,
    agent_executor: Arc<AgentExecutor>,
    execution_registry: Arc<ExecutionRegistry>,
    task_store: Option<Arc<dyn TaskStore>>,
    registry_url: String,
}

pub struct LambdaClientBuilder {
    registry_url: String,
    temp_dir: PathBuf,
    execution_registry: Arc<ExecutionRegistry>,
    task_store: Option<Arc<dyn TaskStore>>,
    recover_pending_tasks: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationChildrenSummary {
    pub invocation_id: uuid::Uuid,
    pub invocation_status: Option<TaskStatus>,
    pub total_children: usize,
    pub pending: usize,
    pub running: usize,
    pub completed: usize,
    pub failed: usize,
    pub cancelled: usize,
    pub all_children_terminal: bool,
    pub has_failures: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationSummary {
    pub invocation: TaskRecord,
    pub children: InvocationChildrenSummary,
    pub derived_status: TaskStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskCancellation {
    pub task_id: String,
    pub cancelled: bool,
    pub status: Option<TaskStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationCancellation {
    pub invocation_id: uuid::Uuid,
    pub invocation_cancelled: bool,
    pub invocation_status: Option<TaskStatus>,
    pub cancelled_children: usize,
    pub total_children: usize,
}

impl LambdaClient {
    /// Create a new Lambda client
    ///
    /// # Arguments
    /// * `registry_url` - URL of the A3S Registry (e.g., "http://localhost:3000")
    ///
    /// # Example
    /// ```no_run
    /// use a3s_lambda::LambdaClient;
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = LambdaClient::new("http://localhost:3000").await.unwrap();
    /// }
    /// ```
    pub async fn new(registry_url: impl Into<String>) -> Result<Self> {
        Self::builder(registry_url).build().await
    }

    pub fn builder(registry_url: impl Into<String>) -> LambdaClientBuilder {
        LambdaClientBuilder::new(registry_url)
    }

    /// Create a new Lambda client with custom temp directory
    pub async fn with_temp_dir(registry_url: impl Into<String>, temp_dir: PathBuf) -> Result<Self> {
        Self::builder(registry_url).temp_dir(temp_dir).build().await
    }

    pub async fn with_execution_registry(
        registry_url: impl Into<String>,
        execution_registry: Arc<ExecutionRegistry>,
    ) -> Result<Self> {
        Self::builder(registry_url)
            .execution_registry(execution_registry)
            .build()
            .await
    }

    pub async fn with_temp_dir_and_execution_registry(
        registry_url: impl Into<String>,
        temp_dir: PathBuf,
        execution_registry: Arc<ExecutionRegistry>,
    ) -> Result<Self> {
        Self::builder(registry_url)
            .temp_dir(temp_dir)
            .execution_registry(execution_registry)
            .build()
            .await
    }

    pub async fn with_task_store(
        registry_url: impl Into<String>,
        temp_dir: PathBuf,
        task_store: Option<Arc<dyn TaskStore>>,
    ) -> Result<Self> {
        Self::builder(registry_url)
            .temp_dir(temp_dir)
            .task_store(task_store)
            .build()
            .await
    }

    pub async fn with_task_store_and_execution_registry(
        registry_url: impl Into<String>,
        temp_dir: PathBuf,
        task_store: Option<Arc<dyn TaskStore>>,
        execution_registry: Arc<ExecutionRegistry>,
    ) -> Result<Self> {
        Self::builder(registry_url)
            .temp_dir(temp_dir)
            .task_store(task_store)
            .execution_registry(execution_registry)
            .build()
            .await
    }

    pub async fn submit_task(&self, request: TaskRequest) -> Result<TaskResult> {
        let task = LambdaTask::new(request.kind);
        self.validate_task_submission(&task)?;
        self.persist_task(task).await
    }

    pub async fn from_database_config(
        registry_url: impl Into<String>,
        temp_dir: PathBuf,
        database: &DatabaseConfig,
    ) -> Result<Self> {
        Self::from_database_config_and_execution_registry(
            registry_url,
            temp_dir,
            database,
            Arc::new(ExecutionRegistry::with_defaults()),
        )
        .await
    }

    pub async fn from_database_config_and_execution_registry(
        registry_url: impl Into<String>,
        temp_dir: PathBuf,
        database: &DatabaseConfig,
        execution_registry: Arc<ExecutionRegistry>,
    ) -> Result<Self> {
        let task_store: Option<Arc<dyn TaskStore>> = Some(Arc::new(
            PgTaskStore::new(&database.url, database.max_connections).await?,
        ) as Arc<dyn TaskStore>);

        Self::with_task_store_and_execution_registry(
            registry_url,
            temp_dir,
            task_store,
            execution_registry,
        )
        .await
    }

    /// Recover pending tasks from storage
    async fn recover_pending_tasks(&self, store: Arc<dyn TaskStore>) -> Result<()> {
        let pending = store.list_pending().await?;

        tracing::info!(count = pending.len(), "recovering pending tasks");

        for record in pending {
            let lane = &record.lane;
            let command = Box::new(LambdaCommand::new(
                record.id.clone(),
                record.task.clone(),
                self.agent_executor.clone(),
                self.execution_registry.clone(),
                Some(store.clone()),
            ));

            if let Err(e) = self.queue_manager.submit(lane, command).await {
                tracing::error!(
                    task_id = %record.id,
                    error = %e,
                    "failed to recover task"
                );
            }
        }

        Ok(())
    }

    /// Execute a single task
    ///
    /// Tasks are automatically queued and executed based on priority.
    /// The method blocks until the task completes or fails.
    ///
    /// # Example
    /// ```no_run
    /// use a3s_lambda::{LambdaClient, TaskRequest, TaskKind};
    ///
    /// #[tokio::main]
    /// async fn main() {
    ///     let client = LambdaClient::new("http://localhost:3000").await.unwrap();
    ///
    ///     let task = TaskRequest {
    ///         kind: TaskKind::AgentRun {
    ///             agent: "alice/my-agent".to_string(),
    ///             version: "1.0.0".to_string(),
    ///             input: serde_json::json!({"query": "hello"}),
    ///             box_runtime: None,
    ///             timeout_secs: 300,
    ///         },
    ///     };
    ///
    ///     let result = client.execute_task(task).await.unwrap();
    ///     println!("Success: {:?}", result.status);
    /// }
    /// ```
    pub async fn execute_task(&self, request: TaskRequest) -> Result<TaskResult> {
        let start_time = chrono::Utc::now();

        // Create task from request
        let task = LambdaTask::new(request.kind);
        self.validate_task_submission(&task)?;
        self.execute_faas_task(task, start_time).await
    }

    fn validate_task_submission(&self, task: &LambdaTask) -> Result<()> {
        task.box_workload_envelope()
            .validate()
            .map_err(LambdaError::Internal)?;

        if let TaskKind::Execution {
            executor,
            required_capabilities,
            policy,
            ..
        } = &task.kind
        {
            self.execution_registry
                .validate_capabilities(executor, required_capabilities, policy)
                .map_err(LambdaError::Internal)?;
        }

        Ok(())
    }

    async fn persist_task(&self, task: LambdaTask) -> Result<TaskResult> {
        let task_id = task.id;
        let lane = select_lane(&task);

        tracing::info!(
            task_id = %task_id,
            lane = %lane,
            "persisting task submission"
        );

        self.save_task_record(&task, lane).await?;

        Ok(TaskResult {
            id: task_id,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            duration_ms: None,
        })
    }

    async fn save_task_record(&self, task: &LambdaTask, lane: &str) -> Result<()> {
        if let Some(ref store) = self.task_store {
            let record = TaskRecord {
                id: task.id.to_string(),
                task: task.clone(),
                status: TaskStatus::Pending,
                lane: lane.to_string(),
                created_at: std::time::SystemTime::now(),
                updated_at: std::time::SystemTime::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error: None,
            };
            store.save(record).await?;
        }

        Ok(())
    }

    async fn execute_faas_task(
        &self,
        task: LambdaTask,
        start_time: chrono::DateTime<chrono::Utc>,
    ) -> Result<TaskResult> {
        let task_id = task.id;

        // Select appropriate lane based on task type
        let lane = select_lane(&task);

        tracing::info!(
            task_id = %task_id,
            lane = %lane,
            "submitting task to queue"
        );

        self.save_task_record(&task, lane).await?;

        // Create command and submit to queue
        let command = Box::new(LambdaCommand::new(
            task_id.to_string(),
            task.clone(),
            self.agent_executor.clone(),
            self.execution_registry.clone(),
            self.task_store.clone(),
        ));

        let rx = self
            .queue_manager
            .submit(lane, command)
            .await
            .map_err(|e| LambdaError::Internal(format!("failed to submit task: {}", e)))?;

        // Wait for result
        let result = match rx.await {
            Ok(Ok(value)) => {
                let end_time = chrono::Utc::now();
                let duration_ms = (end_time - start_time).num_milliseconds() as u64;

                tracing::info!(
                    task_id = %task_id,
                    duration_ms = %duration_ms,
                    "task completed successfully"
                );

                TaskResult {
                    id: task_id,
                    status: TaskStatus::Completed,
                    result: Some(value),
                    error: None,
                    duration_ms: Some(duration_ms),
                }
            }
            Ok(Err(e)) => {
                let end_time = chrono::Utc::now();
                let duration_ms = (end_time - start_time).num_milliseconds() as u64;

                tracing::error!(
                    task_id = %task_id,
                    error = %e,
                    "task failed"
                );

                TaskResult {
                    id: task_id,
                    status: TaskStatus::Failed,
                    result: None,
                    error: Some(e.to_string()),
                    duration_ms: Some(duration_ms),
                }
            }
            Err(_) => {
                tracing::error!(
                    task_id = %task_id,
                    "task channel closed unexpectedly"
                );

                return Err(LambdaError::Internal("task channel closed".into()));
            }
        };

        Ok(result)
    }

    /// Invoke a packaged agent as a first-class AaaS workload.
    pub async fn invoke_agent(&self, request: AgentInvocationRequest) -> Result<TaskResult> {
        self.execute_task(TaskRequest::agent_invocation(request))
            .await
    }

    pub async fn submit_invocation(&self, request: AgentInvocationRequest) -> Result<TaskResult> {
        self.submit_task(TaskRequest::agent_invocation(request))
            .await
    }

    /// Execute an agent-owned task such as fetch/crawl/parse-compatible workloads.
    pub async fn execute_execution_task(
        &self,
        request: ExecutionTaskRequest,
    ) -> Result<TaskResult> {
        self.execute_task(TaskRequest::execution_task(request))
            .await
    }

    pub async fn execute_execution_task_submission(
        &self,
        request: ExecutionTaskSubmissionRequest,
    ) -> Result<TaskResult> {
        let task = if let Some(parent_invocation_id) = request.parent_invocation_id {
            TaskRequest::execution_task(request.task)
                .with_parent_invocation_id(parent_invocation_id)
        } else {
            LambdaTask::new(TaskRequest::execution_task(request.task).kind)
        };

        self.execute_faas_task(task, chrono::Utc::now()).await
    }

    pub async fn submit_execution_task_submission(
        &self,
        request: ExecutionTaskSubmissionRequest,
    ) -> Result<TaskResult> {
        let task = if let Some(parent_invocation_id) = request.parent_invocation_id {
            TaskRequest::execution_task(request.task)
                .with_parent_invocation_id(parent_invocation_id)
        } else {
            LambdaTask::new(TaskRequest::execution_task(request.task).kind)
        };

        self.validate_task_submission(&task)?;
        self.persist_task(task).await
    }

    /// Get registry URL
    pub fn registry_url(&self) -> &str {
        &self.registry_url
    }

    /// Get queue manager for advanced operations
    pub fn queue_manager(&self) -> &QueueManager {
        &self.queue_manager
    }

    pub fn execution_registry(&self) -> &ExecutionRegistry {
        &self.execution_registry
    }

    /// Get VM pool statistics
    pub async fn pool_stats(&self) -> crate::pool::VmPoolStats {
        self.agent_executor.pool_stats().await
    }

    /// Get task by ID from storage
    pub async fn get_task(&self, task_id: &str) -> Result<Option<TaskRecord>> {
        if let Some(ref store) = self.task_store {
            store.get(task_id).await
        } else {
            Ok(None)
        }
    }

    pub async fn get_invocation(&self, task_id: &str) -> Result<Option<TaskRecord>> {
        Ok(self
            .get_task(task_id)
            .await?
            .filter(|record| matches!(record.task.kind, crate::domain::TaskKind::AgentRun { .. })))
    }

    pub async fn get_execution_task(&self, task_id: &str) -> Result<Option<TaskRecord>> {
        Ok(self
            .get_task(task_id)
            .await?
            .filter(|record| !matches!(record.task.kind, crate::domain::TaskKind::AgentRun { .. })))
    }

    pub async fn get_task_result(&self, task_id: &str) -> Result<Option<TaskResult>> {
        Ok(self
            .get_task(task_id)
            .await?
            .map(|record| record.task.into()))
    }

    pub async fn get_invocation_result(&self, task_id: &str) -> Result<Option<TaskResult>> {
        Ok(self
            .get_invocation(task_id)
            .await?
            .map(|record| record.task.into()))
    }

    pub async fn get_execution_task_result(&self, task_id: &str) -> Result<Option<TaskResult>> {
        Ok(self
            .get_execution_task(task_id)
            .await?
            .map(|record| record.task.into()))
    }

    pub async fn wait_for_task(
        &self,
        task_id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<Option<TaskResult>> {
        let deadline = tokio::time::Instant::now() + timeout;
        let poll_interval = poll_interval.max(Duration::from_millis(10));

        loop {
            match self.get_task_result(task_id).await? {
                Some(task_result) if is_terminal_status(task_result.status) => {
                    return Ok(Some(task_result));
                }
                Some(_) => {}
                None => return Ok(None),
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(LambdaError::Timeout(format!(
                    "timed out waiting for task {task_id} to reach a terminal state"
                )));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    pub async fn wait_for_invocation(
        &self,
        task_id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<Option<TaskResult>> {
        let deadline = tokio::time::Instant::now() + timeout;
        let poll_interval = poll_interval.max(Duration::from_millis(10));

        loop {
            match self.get_invocation_result(task_id).await? {
                Some(task_result) if is_terminal_status(task_result.status) => {
                    return Ok(Some(task_result));
                }
                Some(_) => {}
                None => return Ok(None),
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(LambdaError::Timeout(format!(
                    "timed out waiting for invocation {task_id} to reach a terminal state"
                )));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    pub async fn wait_for_execution_task(
        &self,
        task_id: &str,
        timeout: Duration,
        poll_interval: Duration,
    ) -> Result<Option<TaskResult>> {
        let deadline = tokio::time::Instant::now() + timeout;
        let poll_interval = poll_interval.max(Duration::from_millis(10));

        loop {
            match self.get_execution_task_result(task_id).await? {
                Some(task_result) if is_terminal_status(task_result.status) => {
                    return Ok(Some(task_result));
                }
                Some(_) => {}
                None => return Ok(None),
            }

            if tokio::time::Instant::now() >= deadline {
                return Err(LambdaError::Timeout(format!(
                    "timed out waiting for execution task {task_id} to reach a terminal state"
                )));
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    pub async fn replay_task(&self, task_id: &str) -> Result<Option<TaskResult>> {
        let Some(record) = self.get_task(task_id).await? else {
            return Ok(None);
        };

        self.replay_record(record).await.map(Some)
    }

    pub async fn replay_invocation(&self, task_id: &str) -> Result<Option<TaskResult>> {
        let Some(record) = self.get_invocation(task_id).await? else {
            return Ok(None);
        };

        self.replay_record(record).await.map(Some)
    }

    pub async fn replay_execution_task(&self, task_id: &str) -> Result<Option<TaskResult>> {
        let Some(record) = self.get_execution_task(task_id).await? else {
            return Ok(None);
        };

        self.replay_record(record).await.map(Some)
    }

    /// List tasks by status
    pub async fn list_tasks(&self, status: TaskStatus) -> Result<Vec<TaskRecord>> {
        if let Some(ref store) = self.task_store {
            store.list_by_status(status).await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn list_invocations(&self, status: TaskStatus) -> Result<Vec<TaskRecord>> {
        Ok(self
            .list_tasks(status)
            .await?
            .into_iter()
            .filter(|record| matches!(record.task.kind, crate::domain::TaskKind::AgentRun { .. }))
            .collect())
    }

    /// Get the number of tasks currently pending in the task store.
    /// This is a lightweight count used primarily for observability and scaling signals.
    pub async fn queue_depth(&self) -> Result<usize> {
        Ok(self.list_tasks(TaskStatus::Pending).await?.len())
    }

    pub async fn list_execution_tasks(&self, status: TaskStatus) -> Result<Vec<TaskRecord>> {
        Ok(self
            .list_tasks(status)
            .await?
            .into_iter()
            .filter(|record| !matches!(record.task.kind, crate::domain::TaskKind::AgentRun { .. }))
            .collect())
    }

    pub async fn list_execution_tasks_for_invocation(
        &self,
        invocation_id: uuid::Uuid,
        status: Option<TaskStatus>,
    ) -> Result<Vec<TaskRecord>> {
        if let Some(ref store) = self.task_store {
            store
                .list_by_parent_invocation(&invocation_id.to_string(), status)
                .await
        } else {
            Ok(Vec::new())
        }
    }

    pub async fn summarize_invocation_children(
        &self,
        invocation_id: uuid::Uuid,
    ) -> Result<InvocationChildrenSummary> {
        let invocation_status = self
            .get_invocation(&invocation_id.to_string())
            .await?
            .map(|record| record.status);
        let records = self
            .list_execution_tasks_for_invocation(invocation_id, None)
            .await?;

        Ok(summarize_invocation_children_records(
            invocation_id,
            invocation_status,
            &records,
        ))
    }

    pub async fn summarize_invocation(
        &self,
        invocation_id: uuid::Uuid,
    ) -> Result<Option<InvocationSummary>> {
        let Some(invocation) = self.get_invocation(&invocation_id.to_string()).await? else {
            return Ok(None);
        };

        let children = self.summarize_invocation_children(invocation_id).await?;
        let derived_status = derive_invocation_status(invocation.status, &children);

        Ok(Some(InvocationSummary {
            invocation,
            children,
            derived_status,
        }))
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<TaskCancellation> {
        let status = self.get_task(task_id).await?.map(|record| record.status);

        let cancelled = if let Some(ref store) = self.task_store {
            store
                .cancel(task_id, Some("cancelled by user".to_string()))
                .await?
        } else {
            false
        };

        let status = if cancelled {
            Some(TaskStatus::Cancelled)
        } else {
            status
        };

        Ok(TaskCancellation {
            task_id: task_id.to_string(),
            cancelled,
            status,
        })
    }

    pub async fn cancel_invocation(
        &self,
        invocation_id: uuid::Uuid,
    ) -> Result<Option<InvocationCancellation>> {
        let Some(invocation) = self.get_invocation(&invocation_id.to_string()).await? else {
            return Ok(None);
        };

        let invocation_cancel = self.cancel_task(&invocation.id).await?;
        let children = self
            .list_execution_tasks_for_invocation(invocation_id, Some(TaskStatus::Pending))
            .await?;

        let mut cancelled_children = 0;
        for child in &children {
            if self.cancel_task(&child.id).await?.cancelled {
                cancelled_children += 1;
            }
        }

        Ok(Some(InvocationCancellation {
            invocation_id,
            invocation_cancelled: invocation_cancel.cancelled,
            invocation_status: invocation_cancel.status.or(Some(invocation.status)),
            cancelled_children,
            total_children: children.len(),
        }))
    }

    pub async fn cancel_execution_task(&self, task_id: &str) -> Result<Option<TaskCancellation>> {
        let Some(task) = self.get_execution_task(task_id).await? else {
            return Ok(None);
        };

        self.cancel_task(&task.id).await.map(Some)
    }

    async fn replay_record(&self, record: TaskRecord) -> Result<TaskResult> {
        let task = replayable_task_from_record(record);
        self.validate_task_submission(&task)?;
        self.persist_task(task).await
    }

    /// Cleanup old completed/failed tasks
    pub async fn cleanup_old_tasks(&self, older_than: Duration) -> Result<usize> {
        if let Some(ref store) = self.task_store {
            store.cleanup_old_tasks(older_than).await
        } else {
            Ok(0)
        }
    }

    /// Gracefully shutdown the client
    ///
    /// Stops accepting new tasks and waits for in-flight tasks to complete.
    pub async fn shutdown(&self, drain_timeout: Duration) -> Result<()> {
        tracing::info!("shutting down Lambda client");

        self.queue_manager.shutdown().await;

        self.queue_manager
            .drain(drain_timeout)
            .await
            .map_err(|e| LambdaError::Internal(format!("drain failed: {}", e)))?;

        tracing::info!("Lambda client shutdown complete");
        Ok(())
    }
}

impl LambdaClientBuilder {
    pub fn new(registry_url: impl Into<String>) -> Self {
        Self {
            registry_url: registry_url.into(),
            temp_dir: default_runtime_dir(),
            execution_registry: Arc::new(ExecutionRegistry::with_defaults()),
            task_store: None,
            recover_pending_tasks: true,
        }
    }

    pub fn temp_dir(mut self, temp_dir: PathBuf) -> Self {
        self.temp_dir = temp_dir;
        self
    }

    pub fn execution_registry(mut self, execution_registry: Arc<ExecutionRegistry>) -> Self {
        self.execution_registry = execution_registry;
        self
    }

    pub fn task_store(mut self, task_store: Option<Arc<dyn TaskStore>>) -> Self {
        self.task_store = task_store;
        self
    }

    pub fn recover_pending_tasks(mut self, recover_pending_tasks: bool) -> Self {
        self.recover_pending_tasks = recover_pending_tasks;
        self
    }

    pub async fn build(self) -> Result<LambdaClient> {
        std::fs::create_dir_all(&self.temp_dir)
            .map_err(|e| LambdaError::Internal(format!("failed to create temp dir: {}", e)))?;

        let task_store = self.task_store;

        let agent_executor = AgentExecutor::new(self.registry_url.clone(), self.temp_dir)
            .await
            .map_err(|e: LambdaError| LambdaError::Internal(e.to_string()))?;

        let queue_manager = create_faas_queue().await?;
        queue_manager
            .start()
            .await
            .map_err(|e| LambdaError::Internal(format!("failed to start queue manager: {}", e)))?;

        let client = LambdaClient {
            queue_manager: Arc::new(queue_manager),
            agent_executor: Arc::new(agent_executor),
            execution_registry: self.execution_registry,
            task_store,
            registry_url: self.registry_url,
        };

        if self.recover_pending_tasks {
            if let Some(ref store) = client.task_store {
                client.recover_pending_tasks(store.clone()).await?;
            }
        }

        Ok(client)
    }
}

#[cfg(test)]
fn uses_postgres_backend(database_url: &str) -> bool {
    database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")
}

fn summarize_invocation_children_records(
    invocation_id: uuid::Uuid,
    invocation_status: Option<TaskStatus>,
    records: &[TaskRecord],
) -> InvocationChildrenSummary {
    let mut summary = InvocationChildrenSummary {
        invocation_id,
        invocation_status,
        total_children: records.len(),
        pending: 0,
        running: 0,
        completed: 0,
        failed: 0,
        cancelled: 0,
        all_children_terminal: true,
        has_failures: false,
    };

    for record in records {
        match record.status {
            TaskStatus::Pending => {
                summary.pending += 1;
                summary.all_children_terminal = false;
            }
            TaskStatus::Running => {
                summary.running += 1;
                summary.all_children_terminal = false;
            }
            TaskStatus::Completed => summary.completed += 1,
            TaskStatus::Failed => {
                summary.failed += 1;
                summary.has_failures = true;
            }
            TaskStatus::Cancelled => summary.cancelled += 1,
        }
    }

    summary
}

fn derive_invocation_status(
    invocation_status: TaskStatus,
    children: &InvocationChildrenSummary,
) -> TaskStatus {
    match invocation_status {
        TaskStatus::Cancelled => return TaskStatus::Cancelled,
        TaskStatus::Failed => return TaskStatus::Failed,
        _ => {}
    }

    if children.running > 0 {
        return TaskStatus::Running;
    }

    if children.pending > 0 {
        return match invocation_status {
            TaskStatus::Pending => TaskStatus::Pending,
            _ => TaskStatus::Running,
        };
    }

    if children.failed > 0 {
        return TaskStatus::Failed;
    }

    if children.total_children > 0 && children.all_children_terminal {
        if children.completed > 0 {
            return TaskStatus::Completed;
        }
        if children.cancelled == children.total_children {
            return TaskStatus::Cancelled;
        }
    }

    invocation_status
}

fn replayable_task_from_record(record: TaskRecord) -> LambdaTask {
    let task = record.task;
    let parent_invocation_id = task.parent_invocation_id();
    let replay = LambdaTask::new(task.kind);

    match parent_invocation_id {
        Some(parent_invocation_id) => replay.with_parent_invocation_id(parent_invocation_id),
        None => replay,
    }
}

fn is_terminal_status(status: TaskStatus) -> bool {
    matches!(
        status,
        TaskStatus::Completed | TaskStatus::Failed | TaskStatus::Cancelled
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storages::TaskRecord;
    use crate::{
        AgentInvocationRequest, CapabilityMatchMode, ExecutionCapability, ExecutionPolicy,
        ExecutionTaskRequest, TaskKind, TaskRequest,
    };
    use std::collections::HashMap;
    use std::time::SystemTime;

    #[tokio::test]
    #[ignore] // Requires registry to be running
    async fn test_client_creation() {
        let client = LambdaClient::new("http://localhost:3000").await;
        assert!(client.is_ok());
    }

    #[test]
    fn detects_postgres_backend_from_url() {
        assert!(uses_postgres_backend("postgres://localhost/a3s"));
        assert!(uses_postgres_backend("postgresql://localhost/a3s"));
        assert!(!uses_postgres_backend("file:///tmp/tasks.db"));
    }

    #[test]
    fn can_disable_pending_recovery_in_builder() {
        let builder = LambdaClient::builder("http://localhost:3000").recover_pending_tasks(false);
        assert!(!builder.recover_pending_tasks);
    }

    #[test]
    fn summarizes_child_status_counts() {
        let invocation_id = uuid::Uuid::new_v4();
        let mk = |status: TaskStatus| {
            let task = TaskRequest::execution_task(ExecutionTaskRequest {
                executor: "http".into(),
                handler: "get".into(),
                input: serde_json::json!({"url": "https://example.com"}),
                required_capabilities: vec![ExecutionCapability::Network {
                    protocol: "http".into(),
                    operation: "fetch".into(),
                    scope: "public".into(),
                }],
                policy: ExecutionPolicy {
                    capability_match_mode: CapabilityMatchMode::AllRequired,
                    ..Default::default()
                },
                labels: Default::default(),
                preferred_lane: None,
                box_runtime: None,
                timeout_secs: 30,
            })
            .with_parent_invocation_id(invocation_id);
            TaskRecord {
                id: task.id.to_string(),
                task,
                status,
                lane: "realtime".into(),
                created_at: SystemTime::now(),
                updated_at: SystemTime::now(),
                started_at: None,
                completed_at: None,
                result: None,
                error: None,
            }
        };

        let summary = summarize_invocation_children_records(
            invocation_id,
            Some(TaskStatus::Running),
            &[
                mk(TaskStatus::Pending),
                mk(TaskStatus::Running),
                mk(TaskStatus::Completed),
                mk(TaskStatus::Failed),
                mk(TaskStatus::Cancelled),
            ],
        );

        assert_eq!(summary.total_children, 5);
        assert_eq!(summary.pending, 1);
        assert_eq!(summary.running, 1);
        assert_eq!(summary.completed, 1);
        assert_eq!(summary.failed, 1);
        assert_eq!(summary.cancelled, 1);
        assert!(!summary.all_children_terminal);
        assert!(summary.has_failures);
        assert_eq!(summary.invocation_status, Some(TaskStatus::Running));
    }

    #[test]
    fn derives_invocation_status_from_children() {
        let invocation_id = uuid::Uuid::new_v4();
        let base = InvocationChildrenSummary {
            invocation_id,
            invocation_status: Some(TaskStatus::Running),
            total_children: 2,
            pending: 0,
            running: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            all_children_terminal: true,
            has_failures: false,
        };

        let mut failed = base.clone();
        failed.failed = 1;
        assert_eq!(
            derive_invocation_status(TaskStatus::Running, &failed),
            TaskStatus::Failed
        );

        let mut running = base.clone();
        running.running = 1;
        running.all_children_terminal = false;
        assert_eq!(
            derive_invocation_status(TaskStatus::Pending, &running),
            TaskStatus::Running
        );

        let mut completed = base.clone();
        completed.completed = 2;
        assert_eq!(
            derive_invocation_status(TaskStatus::Running, &completed),
            TaskStatus::Completed
        );
    }

    #[test]
    fn derives_cancelled_when_all_children_are_cancelled() {
        let invocation_id = uuid::Uuid::new_v4();
        let summary = InvocationChildrenSummary {
            invocation_id,
            invocation_status: Some(TaskStatus::Running),
            total_children: 2,
            pending: 0,
            running: 0,
            completed: 0,
            failed: 0,
            cancelled: 2,
            all_children_terminal: true,
            has_failures: false,
        };

        assert_eq!(
            derive_invocation_status(TaskStatus::Running, &summary),
            TaskStatus::Cancelled
        );
    }

    #[test]
    fn derives_running_when_invocation_is_running_and_children_are_pending() {
        let invocation_id = uuid::Uuid::new_v4();
        let summary = InvocationChildrenSummary {
            invocation_id,
            invocation_status: Some(TaskStatus::Running),
            total_children: 3,
            pending: 3,
            running: 0,
            completed: 0,
            failed: 0,
            cancelled: 0,
            all_children_terminal: false,
            has_failures: false,
        };

        assert_eq!(
            derive_invocation_status(TaskStatus::Running, &summary),
            TaskStatus::Running
        );
    }

    #[test]
    fn derives_failed_and_cancelled_invocation_statuses_as_terminal_overrides() {
        let invocation_id = uuid::Uuid::new_v4();
        let summary = InvocationChildrenSummary {
            invocation_id,
            invocation_status: Some(TaskStatus::Running),
            total_children: 2,
            pending: 0,
            running: 0,
            completed: 2,
            failed: 0,
            cancelled: 0,
            all_children_terminal: true,
            has_failures: false,
        };

        assert_eq!(
            derive_invocation_status(TaskStatus::Failed, &summary),
            TaskStatus::Failed
        );
        assert_eq!(
            derive_invocation_status(TaskStatus::Cancelled, &summary),
            TaskStatus::Cancelled
        );
    }

    #[test]
    fn replay_creates_new_task_and_preserves_parent_invocation() {
        let parent_invocation_id = uuid::Uuid::new_v4();
        let original = TaskRequest::execution_task(ExecutionTaskRequest {
            executor: "http".into(),
            handler: "get".into(),
            input: serde_json::json!({"url": "https://example.com"}),
            required_capabilities: vec![ExecutionCapability::Network {
                protocol: "http".into(),
                operation: "fetch".into(),
                scope: "public".into(),
            }],
            policy: ExecutionPolicy {
                capability_match_mode: CapabilityMatchMode::AllRequired,
                ..Default::default()
            },
            labels: Default::default(),
            preferred_lane: None,
            box_runtime: None,
            timeout_secs: 30,
        })
        .with_parent_invocation_id(parent_invocation_id);

        let record = TaskRecord {
            id: original.id.to_string(),
            task: original.clone(),
            status: TaskStatus::Completed,
            lane: "realtime".into(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            started_at: None,
            completed_at: None,
            result: Some("{\"ok\":true}".into()),
            error: None,
        };

        let replay = replayable_task_from_record(record);

        assert_ne!(replay.id, original.id);
        assert_eq!(replay.parent_invocation_id(), Some(parent_invocation_id));
        assert_eq!(replay.timeout_secs, original.timeout_secs);
        assert!(replay.result.is_none());
        assert!(replay.error.is_none());
        assert_eq!(replay.status, TaskStatus::Pending);
    }

    #[test]
    fn replay_preserves_agent_invocation_payload_but_clears_execution_state() {
        let original = LambdaTask::new(
            TaskRequest::agent_invocation(AgentInvocationRequest {
                agent: "alice/research-agent".into(),
                version: "2026.03".into(),
                input: serde_json::json!({"query": "latest filings", "depth": "deep"}),
                box_runtime: None,
                timeout_secs: 90,
            })
            .kind,
        );

        let record = TaskRecord {
            id: original.id.to_string(),
            task: original.clone(),
            status: TaskStatus::Failed,
            lane: "realtime".into(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            started_at: Some(SystemTime::now()),
            completed_at: Some(SystemTime::now()),
            result: None,
            error: Some("upstream timeout".into()),
        };

        let replay = replayable_task_from_record(record);

        assert_ne!(replay.id, original.id);
        assert_eq!(replay.parent_invocation_id(), None);
        assert_eq!(replay.timeout_secs, original.timeout_secs);
        assert_eq!(replay.status, TaskStatus::Pending);
        assert!(replay.result.is_none());
        assert!(replay.error.is_none());
        assert!(replay.started_at.is_none());
        assert!(replay.finished_at.is_none());

        match (&original.kind, &replay.kind) {
            (
                TaskKind::AgentRun {
                    agent: original_agent,
                    version: original_version,
                    input: original_input,
                    box_runtime: original_box_runtime,
                    timeout_secs: original_kind_timeout,
                },
                TaskKind::AgentRun {
                    agent: replay_agent,
                    version: replay_version,
                    input: replay_input,
                    box_runtime: replay_box_runtime,
                    timeout_secs: replay_kind_timeout,
                },
            ) => {
                assert_eq!(replay_agent, original_agent);
                assert_eq!(replay_version, original_version);
                assert_eq!(replay_input, original_input);
                assert_eq!(replay_box_runtime, original_box_runtime);
                assert_eq!(replay_kind_timeout, original_kind_timeout);
            }
            _ => panic!("expected agent replay to preserve agent task kind"),
        }
    }

    #[test]
    fn replay_preserves_execution_contract_fields_but_drops_old_terminal_state() {
        let mut labels = HashMap::new();
        labels.insert("source".to_string(), "deep-research".to_string());
        labels.insert("priority".to_string(), "high".to_string());

        let original = LambdaTask::new(
            TaskRequest::execution_task(ExecutionTaskRequest {
                executor: "http".into(),
                handler: "fetch".into(),
                input: serde_json::json!({"url": "https://example.com/report.html"}),
                required_capabilities: vec![ExecutionCapability::Network {
                    protocol: "http".into(),
                    operation: "fetch".into(),
                    scope: "public".into(),
                }],
                policy: ExecutionPolicy {
                    capability_match_mode: CapabilityMatchMode::AllRequired,
                    allowed_scopes: vec!["public".into()],
                    ..Default::default()
                },
                labels,
                preferred_lane: Some("background".into()),
                box_runtime: None,
                timeout_secs: 120,
            })
            .kind,
        );

        let record = TaskRecord {
            id: original.id.to_string(),
            task: original.clone(),
            status: TaskStatus::Completed,
            lane: "background".into(),
            created_at: SystemTime::now(),
            updated_at: SystemTime::now(),
            started_at: Some(SystemTime::now()),
            completed_at: Some(SystemTime::now()),
            result: Some("{\"html\":\"<body />\"}".into()),
            error: Some("stale error".into()),
        };

        let replay = replayable_task_from_record(record);

        assert_ne!(replay.id, original.id);
        assert_eq!(replay.timeout_secs, original.timeout_secs);
        assert_eq!(replay.status, TaskStatus::Pending);
        assert!(replay.result.is_none());
        assert!(replay.error.is_none());
        assert!(replay.started_at.is_none());
        assert!(replay.finished_at.is_none());

        match (&original.kind, &replay.kind) {
            (
                TaskKind::Execution {
                    executor: original_executor,
                    handler: original_handler,
                    input: original_input,
                    required_capabilities: original_capabilities,
                    policy: original_policy,
                    labels: original_labels,
                    preferred_lane: original_preferred_lane,
                    box_runtime: original_box_runtime,
                    timeout_secs: original_kind_timeout,
                },
                TaskKind::Execution {
                    executor: replay_executor,
                    handler: replay_handler,
                    input: replay_input,
                    required_capabilities: replay_capabilities,
                    policy: replay_policy,
                    labels: replay_labels,
                    preferred_lane: replay_preferred_lane,
                    box_runtime: replay_box_runtime,
                    timeout_secs: replay_kind_timeout,
                },
            ) => {
                assert_eq!(replay_executor, original_executor);
                assert_eq!(replay_handler, original_handler);
                assert_eq!(replay_input, original_input);
                assert_eq!(replay_capabilities, original_capabilities);
                assert_eq!(replay_policy, original_policy);
                assert_eq!(replay_labels, original_labels);
                assert_eq!(replay_preferred_lane, original_preferred_lane);
                assert_eq!(replay_box_runtime, original_box_runtime);
                assert_eq!(replay_kind_timeout, original_kind_timeout);
            }
            _ => panic!("expected execution replay to preserve execution task kind"),
        }
    }
}
