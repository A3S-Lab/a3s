//! HTTP server for the a3s-box runtime surface
//!
//! Provides REST API endpoints for workload submission, lifecycle control, and
//! health checks.

use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::Deserialize;
use serde::Serialize;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use std::time::SystemTime;
use tokio::signal;
use tower_http::trace::TraceLayer;

use crate::client::LambdaClient;
use crate::config::{LambdaConfig, SubmissionMode};
use crate::domain::{LambdaError, TaskKind, TaskRequest, TaskResult, TaskStatus};
use crate::executor::ExecutionRegistry;
use crate::pool::VmPoolStats;
use crate::storages::{TaskRecord, TaskStore};
use crate::{
    AgentInvocationRequest, BoxRuntimeSpec, ExecutionCapability, ExecutionPolicy,
    ExecutionTaskSubmissionRequest,
};
use crate::{
    InvocationCancellation, InvocationChildrenSummary, InvocationSummary, TaskCancellation,
};

/// HTTP server for a3s-lambda.
pub struct LambdaServer {
    config: LambdaConfig,
    state: Arc<ServerState>,
}

#[derive(Clone)]
struct ServerState {
    client: Arc<LambdaClient>,
    submission_mode: SubmissionMode,
}

pub struct LambdaServerBuilder {
    config: LambdaConfig,
    execution_registry: Option<Arc<ExecutionRegistry>>,
    task_store: Option<Arc<dyn TaskStore>>,
}

impl LambdaServer {
    /// Create a new server instance
    pub async fn new(config: LambdaConfig) -> Result<Self, LambdaError> {
        Self::builder(config).build().await
    }

    pub fn builder(config: LambdaConfig) -> LambdaServerBuilder {
        LambdaServerBuilder::new(config)
    }

    pub async fn with_execution_registry(
        config: LambdaConfig,
        execution_registry: Arc<ExecutionRegistry>,
    ) -> Result<Self, LambdaError> {
        Self::builder(config)
            .execution_registry(execution_registry)
            .build()
            .await
    }

    /// Build the router with all endpoints
    pub fn router(&self) -> Router {
        Router::new()
            // Task endpoints
            .route("/tasks", post(submit_task))
            .route(
                "/tasks/:id",
                get(get_task).delete(cancel_task).post(replay_task),
            )
            .route("/tasks/:id/result", get(get_task_result))
            .route("/tasks/:id/wait", get(wait_for_task))
            .route("/tasks/status/:status", get(list_tasks_by_status))
            // Agent invocation endpoints
            .route("/invocations", post(submit_invocation))
            .route(
                "/invocations/:id",
                get(get_invocation)
                    .delete(cancel_invocation)
                    .post(replay_invocation),
            )
            .route("/invocations/:id/result", get(get_invocation_result))
            .route("/invocations/:id/wait", get(wait_for_invocation))
            .route("/invocations/:id/summary", get(get_invocation_summary))
            .route(
                "/invocations/status/:status",
                get(list_invocations_by_status),
            )
            // Generic execution task endpoints
            .route("/execution-tasks", post(submit_execution_task))
            .route(
                "/execution-tasks/:id",
                get(get_execution_task)
                    .delete(cancel_execution_task)
                    .post(replay_execution_task),
            )
            .route(
                "/execution-tasks/:id/result",
                get(get_execution_task_result),
            )
            .route("/execution-tasks/:id/wait", get(wait_for_execution_task))
            .route(
                "/execution-tasks/status/:status",
                get(list_execution_tasks_by_status),
            )
            .route(
                "/invocations/:id/execution-tasks",
                get(list_execution_tasks_for_invocation),
            )
            .route(
                "/invocations/:id/execution-tasks/summary",
                get(get_invocation_children_summary),
            )
            // Health endpoints
            .route("/health", get(health_check))
            .route("/ready", get(readiness_check))
            // Metrics endpoint
            .route("/metrics", get(metrics))
            // Pool stats
            .route("/pool/stats", get(pool_stats))
            // Add client as state
            .with_state(self.state.clone())
            // Add tracing middleware
            .layer(TraceLayer::new_for_http())
    }

    /// Run the server
    pub async fn run(self) -> Result<(), LambdaError> {
        let addr = SocketAddr::from((
            self.config
                .server
                .host
                .parse::<std::net::IpAddr>()
                .map_err(|e| LambdaError::Internal(format!("Invalid host address: {}", e)))?,
            self.config.server.port,
        ));

        let app = self.router();

        tracing::info!("Starting a3s-lambda server on {}", addr);

        let listener = tokio::net::TcpListener::bind(&addr)
            .await
            .map_err(|e| LambdaError::Internal(format!("Failed to bind: {}", e)))?;

        axum::serve(listener, app)
            .with_graceful_shutdown(shutdown_signal(
                self.state.client.clone(),
                self.config.server.shutdown_timeout_secs,
            ))
            .await
            .map_err(|e| LambdaError::Internal(format!("Server error: {}", e)))?;

        Ok(())
    }
}

impl LambdaServerBuilder {
    pub fn new(config: LambdaConfig) -> Self {
        Self {
            config,
            execution_registry: None,
            task_store: None,
        }
    }

    pub fn execution_registry(mut self, execution_registry: Arc<ExecutionRegistry>) -> Self {
        self.execution_registry = Some(execution_registry);
        self
    }

    pub fn task_store(mut self, task_store: Arc<dyn TaskStore>) -> Self {
        self.task_store = Some(task_store);
        self
    }

    pub async fn build(self) -> Result<LambdaServer, LambdaError> {
        self.config
            .validate_with_default_task_store(self.task_store.is_none())?;
        let execution_registry = match self.execution_registry {
            Some(registry) => registry,
            None => Arc::new(
                ExecutionRegistry::from_enabled_with_launch_mode(
                    &self.config.execution.enabled_adapters,
                    self.config.execution.launch_mode,
                )
                .map_err(|e| {
                    LambdaError::Internal(format!("failed to build execution registry: {e}"))
                })?,
            ),
        };

        let temp_dir = std::path::PathBuf::from(&self.config.registry.temp_dir);
        let submission_mode = self.config.resolved_submission_mode();
        let client = {
            use crate::storages::pg::PgTaskStore;

            let task_store: Option<Arc<dyn TaskStore>> = match self.task_store {
                Some(task_store) => Some(task_store),
                None => Some(Arc::new(
                    PgTaskStore::new(
                        &self.config.database.url,
                        self.config.database.max_connections,
                    )
                    .await?,
                ) as Arc<dyn TaskStore>),
            };

            LambdaClient::builder(&self.config.registry.url)
                .temp_dir(temp_dir)
                .task_store(task_store)
                .execution_registry(execution_registry)
                .recover_pending_tasks(!matches!(submission_mode, SubmissionMode::PersistOnly))
                .build()
                .await?
        };

        Ok(LambdaServer {
            config: self.config,
            state: Arc::new(ServerState {
                client: Arc::new(client),
                submission_mode,
            }),
        })
    }
}

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    timestamp: u64,
}

#[derive(Debug, Serialize)]
struct ReadinessResponse {
    status: String,
    vm_pool: VmPoolStats,
    queue_depth: usize,
    timestamp: u64,
}

#[derive(Debug, Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct InvocationResponse {
    id: String,
    status: TaskStatus,
    agent: String,
    version: String,
    input: serde_json::Value,
    box_runtime: BoxRuntimeSpec,
    timeout_secs: u32,
    lane: String,
    created_at: u64,
    updated_at: u64,
    started_at: Option<u64>,
    completed_at: Option<u64>,
    result: Option<String>,
    error: Option<String>,
}

#[derive(Debug, Serialize)]
struct ExecutionTaskResponse {
    id: String,
    parent_invocation_id: Option<String>,
    status: TaskStatus,
    executor: String,
    handler: String,
    box_runtime: BoxRuntimeSpec,
    required_capabilities: Vec<ExecutionCapability>,
    policy: ExecutionPolicy,
    timeout_secs: u32,
    lane: String,
    created_at: u64,
    updated_at: u64,
    started_at: Option<u64>,
    completed_at: Option<u64>,
    result: Option<String>,
    error: Option<String>,
    input: serde_json::Value,
    labels: std::collections::HashMap<String, String>,
    preferred_lane: Option<String>,
}

#[derive(Debug, Serialize)]
struct InvocationSummaryResponse {
    invocation: InvocationResponse,
    children: InvocationChildrenSummary,
    derived_status: TaskStatus,
}

#[derive(Debug, Serialize)]
struct TaskCancellationResponse {
    task_id: String,
    cancelled: bool,
    status: Option<TaskStatus>,
}

#[derive(Debug, Serialize)]
struct InvocationCancellationResponse {
    invocation_id: String,
    invocation_cancelled: bool,
    invocation_status: Option<TaskStatus>,
    cancelled_children: usize,
    total_children: usize,
}

#[derive(Debug, Deserialize)]
struct WaitQuery {
    #[serde(default = "default_wait_timeout_ms")]
    timeout_ms: u64,
    #[serde(default = "default_wait_poll_interval_ms")]
    poll_interval_ms: u64,
}

// ============================================================================
// Handlers
// ============================================================================

/// Submit a new task
async fn submit_task(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<TaskRequest>,
) -> Result<Json<TaskResult>, AppError> {
    tracing::info!("Received task submission");

    let result = match state.submission_mode {
        SubmissionMode::PersistOnly => state.client.submit_task(request).await?,
        SubmissionMode::Auto | SubmissionMode::AwaitCompletion => {
            state.client.execute_task(request).await?
        }
    };

    Ok(Json(result))
}

/// Submit a first-class agent invocation.
async fn submit_invocation(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<AgentInvocationRequest>,
) -> Result<Json<TaskResult>, AppError> {
    tracing::info!("Received agent invocation submission");
    Ok(Json(match state.submission_mode {
        SubmissionMode::PersistOnly => state.client.submit_invocation(request).await?,
        SubmissionMode::Auto | SubmissionMode::AwaitCompletion => {
            state.client.invoke_agent(request).await?
        }
    }))
}

/// Submit a first-class execution task owned by an agent.
async fn submit_execution_task(
    State(state): State<Arc<ServerState>>,
    Json(request): Json<ExecutionTaskSubmissionRequest>,
) -> Result<Json<TaskResult>, AppError> {
    tracing::info!("Received execution task submission");
    Ok(Json(match state.submission_mode {
        SubmissionMode::PersistOnly => {
            state
                .client
                .submit_execution_task_submission(request)
                .await?
        }
        SubmissionMode::Auto | SubmissionMode::AwaitCompletion => {
            state
                .client
                .execute_execution_task_submission(request)
                .await?
        }
    }))
}

/// Get task by ID
async fn get_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskRecord>, AppError> {
    tracing::debug!("Getting task: {}", task_id);

    let task = state
        .client
        .get_task(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task not found: {}", task_id)))?;

    Ok(Json(task))
}

async fn cancel_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskCancellationResponse>, AppError> {
    let cancellation = state.client.cancel_task(&task_id).await?;
    Ok(Json(task_cancellation_response(cancellation)))
}

async fn get_task_result(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .get_task_result(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn replay_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .replay_task(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn wait_for_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
    Query(wait): Query<WaitQuery>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .wait_for_task(
            &task_id,
            Duration::from_millis(wait.timeout_ms),
            Duration::from_millis(wait.poll_interval_ms),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Task not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn get_invocation(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<InvocationResponse>, AppError> {
    let task = state
        .client
        .get_invocation(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Invocation not found: {}", task_id)))?;

    Ok(Json(invocation_response(task)?))
}

async fn get_execution_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<ExecutionTaskResponse>, AppError> {
    let task = state
        .client
        .get_execution_task(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution task not found: {}", task_id)))?;

    Ok(Json(execution_task_response(task)?))
}

async fn get_invocation_result(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .get_invocation_result(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Invocation not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn replay_invocation(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .replay_invocation(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Invocation not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn wait_for_invocation(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
    Query(wait): Query<WaitQuery>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .wait_for_invocation(
            &task_id,
            Duration::from_millis(wait.timeout_ms),
            Duration::from_millis(wait.poll_interval_ms),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Invocation not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn cancel_invocation(
    State(state): State<Arc<ServerState>>,
    Path(invocation_id): Path<uuid::Uuid>,
) -> Result<Json<InvocationCancellationResponse>, AppError> {
    let cancellation = state
        .client
        .cancel_invocation(invocation_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Invocation not found: {}", invocation_id)))?;

    Ok(Json(invocation_cancellation_response(cancellation)))
}

async fn cancel_execution_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskCancellationResponse>, AppError> {
    let cancellation = state
        .client
        .cancel_execution_task(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution task not found: {}", task_id)))?;

    Ok(Json(task_cancellation_response(cancellation)))
}

async fn get_execution_task_result(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .get_execution_task_result(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution task not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn replay_execution_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .replay_execution_task(&task_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution task not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn wait_for_execution_task(
    State(state): State<Arc<ServerState>>,
    Path(task_id): Path<String>,
    Query(wait): Query<WaitQuery>,
) -> Result<Json<TaskResult>, AppError> {
    let result = state
        .client
        .wait_for_execution_task(
            &task_id,
            Duration::from_millis(wait.timeout_ms),
            Duration::from_millis(wait.poll_interval_ms),
        )
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Execution task not found: {}", task_id)))?;

    Ok(Json(result))
}

async fn get_invocation_summary(
    State(state): State<Arc<ServerState>>,
    Path(invocation_id): Path<uuid::Uuid>,
) -> Result<Json<InvocationSummaryResponse>, AppError> {
    let summary = state
        .client
        .summarize_invocation(invocation_id)
        .await?
        .ok_or_else(|| AppError::NotFound(format!("Invocation not found: {}", invocation_id)))?;

    Ok(Json(invocation_summary_response(summary)?))
}

/// List tasks by status
async fn list_tasks_by_status(
    State(state): State<Arc<ServerState>>,
    Path(status_str): Path<String>,
) -> Result<Json<Vec<TaskRecord>>, AppError> {
    let status = parse_task_status(&status_str)?;

    let tasks = state.client.list_tasks(status).await?;

    Ok(Json(tasks))
}

async fn list_invocations_by_status(
    State(state): State<Arc<ServerState>>,
    Path(status_str): Path<String>,
) -> Result<Json<Vec<InvocationResponse>>, AppError> {
    let status = parse_task_status(&status_str)?;
    let records = state.client.list_invocations(status).await?;
    Ok(Json(
        records
            .into_iter()
            .map(invocation_response)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

async fn list_execution_tasks_by_status(
    State(state): State<Arc<ServerState>>,
    Path(status_str): Path<String>,
) -> Result<Json<Vec<ExecutionTaskResponse>>, AppError> {
    let status = parse_task_status(&status_str)?;
    let records = state.client.list_execution_tasks(status).await?;
    Ok(Json(
        records
            .into_iter()
            .map(execution_task_response)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

async fn list_execution_tasks_for_invocation(
    State(state): State<Arc<ServerState>>,
    Path(invocation_id): Path<uuid::Uuid>,
) -> Result<Json<Vec<ExecutionTaskResponse>>, AppError> {
    let records = state
        .client
        .list_execution_tasks_for_invocation(invocation_id, None)
        .await?;
    Ok(Json(
        records
            .into_iter()
            .map(execution_task_response)
            .collect::<Result<Vec<_>, _>>()?,
    ))
}

async fn get_invocation_children_summary(
    State(state): State<Arc<ServerState>>,
    Path(invocation_id): Path<uuid::Uuid>,
) -> Result<Json<InvocationChildrenSummary>, AppError> {
    Ok(Json(
        state
            .client
            .summarize_invocation_children(invocation_id)
            .await?,
    ))
}

/// Health check endpoint (liveness probe)
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    })
}

/// Readiness check endpoint (readiness probe)
async fn readiness_check(
    State(state): State<Arc<ServerState>>,
) -> Result<Json<ReadinessResponse>, AppError> {
    // Check VM pool
    let pool_stats = state.client.pool_stats().await;

    // Check if pool is completely exhausted
    if pool_stats.idle == 0 && pool_stats.active >= pool_stats.max_total {
        return Err(AppError::ServiceUnavailable(
            "VM pool exhausted".to_string(),
        ));
    }

    // Check database connectivity and get queue depth for observability.
    if state.client.get_task("readiness-check").await.is_err() {
        // DB connectivity verified via get_task (task not found is expected)
    }
    let queue_depth = state.client.queue_depth().await.unwrap_or(0);

    Ok(Json(ReadinessResponse {
        status: "ready".to_string(),
        vm_pool: pool_stats,
        queue_depth,
        timestamp: SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    }))
}

/// Metrics endpoint (Prometheus format)
async fn metrics(State(state): State<Arc<ServerState>>) -> String {
    let pool_stats = state.client.pool_stats().await;

    // Simple Prometheus text format
    format!(
        "# HELP a3s_lambda_vm_pool_size Total VM pool size\n\
         # TYPE a3s_lambda_vm_pool_size gauge\n\
         a3s_lambda_vm_pool_size {}\n\
         # HELP a3s_lambda_vm_pool_idle Idle VMs in pool\n\
         # TYPE a3s_lambda_vm_pool_idle gauge\n\
         a3s_lambda_vm_pool_idle {}\n\
         # HELP a3s_lambda_vm_pool_active Active VMs\n\
         # TYPE a3s_lambda_vm_pool_active gauge\n\
         a3s_lambda_vm_pool_active {}\n",
        pool_stats.max_total, pool_stats.idle, pool_stats.active
    )
}

/// Get VM pool statistics
async fn pool_stats(State(state): State<Arc<ServerState>>) -> Json<VmPoolStats> {
    Json(state.client.pool_stats().await)
}

// ============================================================================
// Error Handling
// ============================================================================

enum AppError {
    Internal(LambdaError),
    BadRequest(String),
    NotFound(String),
    ServiceUnavailable(String),
}

fn default_wait_timeout_ms() -> u64 {
    30_000
}

fn default_wait_poll_interval_ms() -> u64 {
    250
}

fn parse_task_status(status_str: &str) -> Result<TaskStatus, AppError> {
    match status_str {
        "pending" => Ok(TaskStatus::Pending),
        "running" => Ok(TaskStatus::Running),
        "completed" => Ok(TaskStatus::Completed),
        "failed" => Ok(TaskStatus::Failed),
        "cancelled" => Ok(TaskStatus::Cancelled),
        _ => Err(AppError::BadRequest(format!(
            "Invalid status: {}",
            status_str
        ))),
    }
}

fn invocation_response(record: TaskRecord) -> Result<InvocationResponse, AppError> {
    let TaskKind::AgentRun {
        agent,
        version,
        input,
        box_runtime,
        timeout_secs,
    } = record.task.kind
    else {
        return Err(AppError::Internal(LambdaError::Internal(
            "task is not an agent invocation".to_string(),
        )));
    };

    let resolved_box_runtime =
        box_runtime.unwrap_or_else(|| BoxRuntimeSpec::for_agent_invocation(&agent, &version));

    Ok(InvocationResponse {
        id: record.id,
        status: record.status,
        agent,
        version,
        input,
        box_runtime: resolved_box_runtime,
        timeout_secs,
        lane: record.lane,
        created_at: unix_secs(record.created_at),
        updated_at: unix_secs(record.updated_at),
        started_at: record.started_at.map(unix_secs),
        completed_at: record.completed_at.map(unix_secs),
        result: record.result,
        error: record.error,
    })
}

fn execution_task_response(record: TaskRecord) -> Result<ExecutionTaskResponse, AppError> {
    let task = record.task;
    let parent_invocation_id = task.parent_invocation_id().map(|id| id.to_string());
    let timeout_secs = task.timeout_secs;
    let (
        executor,
        handler,
        input,
        required_capabilities,
        policy,
        labels,
        preferred_lane,
        box_runtime,
    ) = match task.kind {
        TaskKind::Execution {
            executor,
            handler,
            input,
            required_capabilities,
            policy,
            labels,
            preferred_lane,
            box_runtime,
            ..
        } => (
            executor,
            handler,
            input,
            required_capabilities,
            policy,
            labels,
            preferred_lane,
            box_runtime,
        ),
        TaskKind::AgentRun { .. } => {
            return Err(AppError::Internal(LambdaError::Internal(
                "task is not an execution task".to_string(),
            )))
        }
    };

    let resolved_box_runtime =
        box_runtime.unwrap_or_else(|| BoxRuntimeSpec::for_execution_adapter(&executor, &handler));

    Ok(ExecutionTaskResponse {
        id: record.id,
        parent_invocation_id,
        status: record.status,
        executor,
        handler,
        box_runtime: resolved_box_runtime,
        required_capabilities,
        policy,
        timeout_secs,
        lane: record.lane,
        created_at: unix_secs(record.created_at),
        updated_at: unix_secs(record.updated_at),
        started_at: record.started_at.map(unix_secs),
        completed_at: record.completed_at.map(unix_secs),
        result: record.result,
        error: record.error,
        input,
        labels,
        preferred_lane,
    })
}

fn invocation_summary_response(
    summary: InvocationSummary,
) -> Result<InvocationSummaryResponse, AppError> {
    Ok(InvocationSummaryResponse {
        invocation: invocation_response(summary.invocation)?,
        children: summary.children,
        derived_status: summary.derived_status,
    })
}

fn task_cancellation_response(cancellation: TaskCancellation) -> TaskCancellationResponse {
    TaskCancellationResponse {
        task_id: cancellation.task_id,
        cancelled: cancellation.cancelled,
        status: cancellation.status,
    }
}

fn invocation_cancellation_response(
    cancellation: InvocationCancellation,
) -> InvocationCancellationResponse {
    InvocationCancellationResponse {
        invocation_id: cancellation.invocation_id.to_string(),
        invocation_cancelled: cancellation.invocation_cancelled,
        invocation_status: cancellation.invocation_status,
        cancelled_children: cancellation.cancelled_children,
        total_children: cancellation.total_children,
    }
}

fn unix_secs(time: SystemTime) -> u64 {
    time.duration_since(SystemTime::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl From<LambdaError> for AppError {
    fn from(err: LambdaError) -> Self {
        AppError::Internal(err)
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match self {
            AppError::Internal(e) => match e {
                LambdaError::Timeout(message) => (StatusCode::REQUEST_TIMEOUT, "timeout", message),
                other => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "internal_error",
                    other.to_string(),
                ),
            },
            AppError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "bad_request", msg),
            AppError::NotFound(msg) => (StatusCode::NOT_FOUND, "not_found", msg),
            AppError::ServiceUnavailable(msg) => {
                (StatusCode::SERVICE_UNAVAILABLE, "unavailable", msg)
            }
        };

        let body = Json(ErrorResponse {
            error: error_type.to_string(),
            message,
        });

        (status, body).into_response()
    }
}

// ============================================================================
// Graceful Shutdown
// ============================================================================

async fn shutdown_signal(client: Arc<LambdaClient>, timeout_secs: u64) {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    tracing::info!("Shutdown signal received, starting graceful shutdown");

    // Gracefully shutdown the client
    let timeout = std::time::Duration::from_secs(timeout_secs);
    if let Err(e) = client.shutdown(timeout).await {
        tracing::error!("Error during shutdown: {}", e);
    }

    tracing::info!("Graceful shutdown complete");
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::body::to_bytes;
    use serde_json::Value;

    struct MockTaskStore;

    #[async_trait]
    impl TaskStore for MockTaskStore {
        async fn save(&self, _record: TaskRecord) -> crate::Result<()> {
            Ok(())
        }

        async fn cancel(&self, _task_id: &str, _error: Option<String>) -> crate::Result<bool> {
            Ok(false)
        }

        async fn update_status(
            &self,
            _task_id: &str,
            _status: TaskStatus,
            _result: Option<String>,
            _error: Option<String>,
        ) -> crate::Result<()> {
            Ok(())
        }

        async fn get(&self, _task_id: &str) -> crate::Result<Option<TaskRecord>> {
            Ok(None)
        }

        async fn list_by_status(&self, _status: TaskStatus) -> crate::Result<Vec<TaskRecord>> {
            Ok(Vec::new())
        }

        async fn list_by_parent_invocation(
            &self,
            _parent_invocation_id: &str,
            _status: Option<TaskStatus>,
        ) -> crate::Result<Vec<TaskRecord>> {
            Ok(Vec::new())
        }

        async fn list_pending(&self) -> crate::Result<Vec<TaskRecord>> {
            Ok(Vec::new())
        }

        async fn cleanup_old_tasks(&self, _older_than: Duration) -> crate::Result<usize> {
            Ok(0)
        }
    }

    #[test]
    fn builder_starts_without_custom_task_store() {
        let builder = LambdaServerBuilder::new(LambdaConfig::default());
        assert!(builder.task_store.is_none());
        assert!(builder.execution_registry.is_none());
    }

    #[test]
    fn builder_accepts_custom_task_store_injection() {
        let builder =
            LambdaServerBuilder::new(LambdaConfig::default()).task_store(Arc::new(MockTaskStore));

        assert!(builder.task_store.is_some());
    }

    #[test]
    fn custom_task_store_changes_validation_requirement() {
        let mut config = LambdaConfig::default();
        config.database.url = "custom://memory".into();

        let builder_without_custom_store = LambdaServerBuilder::new(config.clone());
        assert!(builder_without_custom_store
            .config
            .validate_with_default_task_store(builder_without_custom_store.task_store.is_none())
            .is_err());

        let builder_with_custom_store =
            LambdaServerBuilder::new(config).task_store(Arc::new(MockTaskStore));
        assert!(builder_with_custom_store
            .config
            .validate_with_default_task_store(builder_with_custom_store.task_store.is_none())
            .is_ok());
    }

    #[test]
    fn unix_secs_returns_expected_epoch_seconds() {
        let now = SystemTime::UNIX_EPOCH + Duration::from_secs(42);
        assert_eq!(unix_secs(now), 42);
    }

    #[tokio::test]
    async fn app_error_timeout_maps_to_408_response() {
        let response =
            AppError::from(LambdaError::Timeout("wait timed out".into())).into_response();
        assert_eq!(response.status(), StatusCode::REQUEST_TIMEOUT);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: Value = serde_json::from_slice(&body).expect("decode body");
        assert_eq!(json["error"], "timeout");
        assert_eq!(json["message"], "wait timed out");
    }

    #[tokio::test]
    async fn app_error_service_unavailable_maps_to_503_response() {
        let response = AppError::ServiceUnavailable("pool exhausted".into()).into_response();
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: Value = serde_json::from_slice(&body).expect("decode body");
        assert_eq!(json["error"], "unavailable");
        assert_eq!(json["message"], "pool exhausted");
    }

    #[tokio::test]
    async fn app_error_bad_request_maps_to_400_response() {
        let response = AppError::BadRequest("invalid payload".into()).into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: Value = serde_json::from_slice(&body).expect("decode body");
        assert_eq!(json["error"], "bad_request");
        assert_eq!(json["message"], "invalid payload");
    }

    #[tokio::test]
    async fn app_error_not_found_maps_to_404_response() {
        let response = AppError::NotFound("task missing".into()).into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: Value = serde_json::from_slice(&body).expect("decode body");
        assert_eq!(json["error"], "not_found");
        assert_eq!(json["message"], "task missing");
    }

    #[tokio::test]
    async fn app_error_internal_non_timeout_maps_to_500_response() {
        let response = AppError::from(LambdaError::Internal("unexpected panic boundary".into()))
            .into_response();
        assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);

        let body = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("read response body");
        let json: Value = serde_json::from_slice(&body).expect("decode body");
        assert_eq!(json["error"], "internal_error");
        assert_eq!(json["message"], "internal error: unexpected panic boundary");
    }
}
