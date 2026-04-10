use axum::{extract::State, http::StatusCode, response::Json, routing::get, Router};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use tokio::sync::watch;

use crate::config::{DatabaseConfig, WorkerConfig};
use crate::domain::{LambdaError, LambdaRepository, Result, TaskStats};
use crate::executor::{agent::AgentExecutor, ExecutionRegistry};
use crate::observability::{
    worker_observability_failure, WorkerReadiness, WorkerReadinessFailure, WorkerScalingContract,
    WorkerScalingSignals, WorkerScalingSnapshot, WorkerStatsSnapshot,
    WORKER_SCALING_CONTRACT_VERSION,
};
use crate::queue::execute_faas_task_payload;
use crate::storages::pg::{LeaseHealthStats, PgTaskStore};

pub struct PgLeaseWorker {
    worker_id: String,
    poll_interval: Duration,
    lease_ttl: Duration,
    heartbeat_interval: Duration,
    release_expired_leases: bool,
    observability_enabled: bool,
    observability_host: String,
    observability_port: u16,
    store: Arc<PgTaskStore>,
    agent_executor: Arc<AgentExecutor>,
    execution_registry: Arc<ExecutionRegistry>,
    stats: Arc<WorkerStats>,
}

#[derive(Default)]
struct WorkerStats {
    claimed_total: AtomicU64,
    completed_total: AtomicU64,
    failed_total: AtomicU64,
    lease_renewal_failures_total: AtomicU64,
    expired_leases_released_total: AtomicU64,
    active_executions: AtomicU64,
}

#[derive(Debug, Clone)]
struct WorkerWindowSnapshot {
    observed_at: Instant,
    worker: WorkerStatsSnapshot,
}

#[derive(Clone)]
struct WorkerObservabilityState {
    worker_id: String,
    store: Arc<PgTaskStore>,
    execution_registry: Arc<ExecutionRegistry>,
    stats: Arc<WorkerStats>,
    stale_heartbeat_after: Duration,
    last_window_snapshot: Arc<Mutex<Option<WorkerWindowSnapshot>>>,
}

impl WorkerObservabilityState {
    async fn stats_snapshot(&self) -> WorkerStatsSnapshot {
        WorkerStatsSnapshot {
            worker_id: self.worker_id.clone(),
            claimed_total: self.stats.claimed_total.load(Ordering::Relaxed),
            completed_total: self.stats.completed_total.load(Ordering::Relaxed),
            failed_total: self.stats.failed_total.load(Ordering::Relaxed),
            lease_renewal_failures_total: self
                .stats
                .lease_renewal_failures_total
                .load(Ordering::Relaxed),
            expired_leases_released_total: self
                .stats
                .expired_leases_released_total
                .load(Ordering::Relaxed),
            active_executions: self.stats.active_executions.load(Ordering::Relaxed),
            box_runtime_pool: Some(self.execution_registry.box_runtime_pool_snapshot().await),
        }
    }
}

impl WorkerStatsSnapshot {
    pub fn to_prometheus(&self) -> String {
        let mut metrics = format!(
            "# HELP a3s_lambda_worker_claimed_total Total tasks claimed by the worker\n\
             # TYPE a3s_lambda_worker_claimed_total counter\n\
             a3s_lambda_worker_claimed_total{{worker_id=\"{worker_id}\"}} {claimed_total}\n\
             # HELP a3s_lambda_worker_completed_total Total tasks completed by the worker\n\
             # TYPE a3s_lambda_worker_completed_total counter\n\
             a3s_lambda_worker_completed_total{{worker_id=\"{worker_id}\"}} {completed_total}\n\
             # HELP a3s_lambda_worker_failed_total Total tasks failed by the worker\n\
             # TYPE a3s_lambda_worker_failed_total counter\n\
             a3s_lambda_worker_failed_total{{worker_id=\"{worker_id}\"}} {failed_total}\n\
             # HELP a3s_lambda_worker_lease_renewal_failures_total Total lease renewal failures\n\
             # TYPE a3s_lambda_worker_lease_renewal_failures_total counter\n\
             a3s_lambda_worker_lease_renewal_failures_total{{worker_id=\"{worker_id}\"}} {lease_renewal_failures_total}\n\
             # HELP a3s_lambda_worker_expired_leases_released_total Total expired leases released by the worker\n\
             # TYPE a3s_lambda_worker_expired_leases_released_total counter\n\
             a3s_lambda_worker_expired_leases_released_total{{worker_id=\"{worker_id}\"}} {expired_leases_released_total}\n\
             # HELP a3s_lambda_worker_active_executions Active executions owned by the worker\n\
             # TYPE a3s_lambda_worker_active_executions gauge\n\
             a3s_lambda_worker_active_executions{{worker_id=\"{worker_id}\"}} {active_executions}\n",
            worker_id = self.worker_id,
            claimed_total = self.claimed_total,
            completed_total = self.completed_total,
            failed_total = self.failed_total,
            lease_renewal_failures_total = self.lease_renewal_failures_total,
            expired_leases_released_total = self.expired_leases_released_total,
            active_executions = self.active_executions,
        );

        if let Some(pool) = self.box_runtime_pool.as_ref() {
            metrics.push_str(&format!(
                "# HELP a3s_lambda_box_runtime_pool_idle_vms Idle VMs currently available in the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_idle_vms gauge\n\
                 a3s_lambda_box_runtime_pool_idle_vms{{worker_id=\"{worker_id}\"}} {idle_vms}\n\
                 # HELP a3s_lambda_box_runtime_pool_active_vms Active VMs currently executing in the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_active_vms gauge\n\
                 a3s_lambda_box_runtime_pool_active_vms{{worker_id=\"{worker_id}\"}} {active_vms}\n\
                 # HELP a3s_lambda_box_runtime_pool_total_vms Total VMs currently allocated in the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_total_vms gauge\n\
                 a3s_lambda_box_runtime_pool_total_vms{{worker_id=\"{worker_id}\"}} {total_vms}\n\
                 # HELP a3s_lambda_box_runtime_pool_max_total_vms Maximum VMs allowed in the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_max_total_vms gauge\n\
                 a3s_lambda_box_runtime_pool_max_total_vms{{worker_id=\"{worker_id}\"}} {max_total_vms}\n\
                 # HELP a3s_lambda_box_runtime_pool_available_vms Remaining VM headroom in the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_available_vms gauge\n\
                 a3s_lambda_box_runtime_pool_available_vms{{worker_id=\"{worker_id}\"}} {available_vms}\n\
                 # HELP a3s_lambda_box_runtime_pool_occupancy_ratio Occupancy ratio of the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_occupancy_ratio gauge\n\
                 a3s_lambda_box_runtime_pool_occupancy_ratio{{worker_id=\"{worker_id}\"}} {occupancy_ratio}\n\
                 # HELP a3s_lambda_box_runtime_pool_active_ratio Active ratio of the box runtime pool\n\
                 # TYPE a3s_lambda_box_runtime_pool_active_ratio gauge\n\
                 a3s_lambda_box_runtime_pool_active_ratio{{worker_id=\"{worker_id}\"}} {active_ratio}\n\
                 # HELP a3s_lambda_box_runtime_pool_capacity_pressure Whether the box runtime pool is near capacity\n\
                 # TYPE a3s_lambda_box_runtime_pool_capacity_pressure gauge\n\
                 a3s_lambda_box_runtime_pool_capacity_pressure{{worker_id=\"{worker_id}\"}} {has_capacity_pressure}\n",
                worker_id = self.worker_id,
                idle_vms = pool.idle_vms,
                active_vms = pool.active_vms,
                total_vms = pool.total_vms,
                max_total_vms = pool.max_total_vms,
                available_vms = pool.available_vms,
                occupancy_ratio = pool.occupancy_ratio,
                active_ratio = pool.active_ratio,
                has_capacity_pressure = if pool.has_capacity_pressure { 1 } else { 0 },
            ));
        }

        metrics
    }
}

fn task_stats_to_prometheus(task_store: &str, stats: &TaskStats) -> String {
    format!(
        "# HELP a3s_lambda_task_store_total Total tasks tracked by the task store\n\
         # TYPE a3s_lambda_task_store_total gauge\n\
         a3s_lambda_task_store_total{{task_store=\"{task_store}\"}} {total}\n\
         # HELP a3s_lambda_task_store_pending Pending tasks tracked by the task store\n\
         # TYPE a3s_lambda_task_store_pending gauge\n\
         a3s_lambda_task_store_pending{{task_store=\"{task_store}\"}} {pending}\n\
         # HELP a3s_lambda_task_store_running Running tasks tracked by the task store\n\
         # TYPE a3s_lambda_task_store_running gauge\n\
         a3s_lambda_task_store_running{{task_store=\"{task_store}\"}} {running}\n\
         # HELP a3s_lambda_task_store_completed Completed tasks tracked by the task store\n\
         # TYPE a3s_lambda_task_store_completed gauge\n\
         a3s_lambda_task_store_completed{{task_store=\"{task_store}\"}} {completed}\n\
         # HELP a3s_lambda_task_store_failed Failed tasks tracked by the task store\n\
         # TYPE a3s_lambda_task_store_failed gauge\n\
         a3s_lambda_task_store_failed{{task_store=\"{task_store}\"}} {failed}\n\
         # HELP a3s_lambda_task_store_cancelled Cancelled tasks tracked by the task store\n\
         # TYPE a3s_lambda_task_store_cancelled gauge\n\
         a3s_lambda_task_store_cancelled{{task_store=\"{task_store}\"}} {cancelled}\n",
        task_store = task_store,
        total = stats.total,
        pending = stats.pending,
        running = stats.running,
        completed = stats.completed,
        failed = stats.failed,
        cancelled = stats.cancelled,
    )
}

async fn worker_metrics_response(state: &WorkerObservabilityState) -> Result<String> {
    let snapshot = state.stats_snapshot().await;
    let task_stats = state.store.repository().get_stats().await?;

    Ok(format!(
        "{}{}",
        snapshot.to_prometheus(),
        task_stats_to_prometheus("postgres", &task_stats)
    ))
}

async fn worker_scaling_response(
    state: &WorkerObservabilityState,
) -> Result<WorkerScalingSnapshot> {
    let worker = state.stats_snapshot().await;
    let task_stats = state.store.repository().get_stats().await?;
    let lease_health = state
        .store
        .lease_health_stats(state.stale_heartbeat_after)
        .await?;
    let window = take_window_measurement(state, &worker);
    Ok(build_worker_scaling_snapshot(
        state.worker_id.clone(),
        worker,
        task_stats,
        lease_health,
        window,
    ))
}

fn build_worker_scaling_contract(snapshot: WorkerScalingSnapshot) -> WorkerScalingContract {
    WorkerScalingContract {
        contract_version: WORKER_SCALING_CONTRACT_VERSION.to_string(),
        generated_at_unix_secs: std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs(),
        snapshot,
    }
}

#[derive(Debug, Clone)]
struct WorkerWindowMeasurement {
    window_secs: f64,
    claimed_delta: u64,
    completed_delta: u64,
    failed_delta: u64,
    lease_renewal_failure_delta: u64,
    expired_lease_recovery_delta: u64,
}

fn take_window_measurement(
    state: &WorkerObservabilityState,
    worker: &WorkerStatsSnapshot,
) -> WorkerWindowMeasurement {
    let now = Instant::now();
    let mut guard = state
        .last_window_snapshot
        .lock()
        .expect("worker window snapshot mutex poisoned");

    let measurement = if let Some(previous) = guard.as_ref() {
        WorkerWindowMeasurement {
            window_secs: now.duration_since(previous.observed_at).as_secs_f64(),
            claimed_delta: worker
                .claimed_total
                .saturating_sub(previous.worker.claimed_total),
            completed_delta: worker
                .completed_total
                .saturating_sub(previous.worker.completed_total),
            failed_delta: worker
                .failed_total
                .saturating_sub(previous.worker.failed_total),
            lease_renewal_failure_delta: worker
                .lease_renewal_failures_total
                .saturating_sub(previous.worker.lease_renewal_failures_total),
            expired_lease_recovery_delta: worker
                .expired_leases_released_total
                .saturating_sub(previous.worker.expired_leases_released_total),
        }
    } else {
        WorkerWindowMeasurement {
            window_secs: 0.0,
            claimed_delta: 0,
            completed_delta: 0,
            failed_delta: 0,
            lease_renewal_failure_delta: 0,
            expired_lease_recovery_delta: 0,
        }
    };

    *guard = Some(WorkerWindowSnapshot {
        observed_at: now,
        worker: worker.clone(),
    });

    measurement
}

impl PgLeaseWorker {
    pub async fn from_config(
        worker: &WorkerConfig,
        registry_url: &str,
        temp_dir: PathBuf,
        database: &DatabaseConfig,
        execution_registry: Arc<ExecutionRegistry>,
    ) -> Result<Self> {
        let store = Arc::new(PgTaskStore::new(&database.url, database.max_connections).await?);
        let agent_executor = Arc::new(
            AgentExecutor::new(registry_url.to_string(), temp_dir)
                .await
                .map_err(|e: LambdaError| LambdaError::Internal(e.to_string()))?,
        );

        Ok(Self {
            worker_id: worker
                .worker_id
                .clone()
                .unwrap_or_else(|| format!("worker-{}", uuid::Uuid::new_v4())),
            poll_interval: Duration::from_millis(worker.poll_interval_ms),
            lease_ttl: Duration::from_secs(worker.lease_ttl_secs),
            heartbeat_interval: Duration::from_secs(worker.heartbeat_interval_secs),
            release_expired_leases: worker.release_expired_leases,
            observability_enabled: worker.observability_enabled,
            observability_host: worker.observability_host.clone(),
            observability_port: worker.observability_port,
            store,
            agent_executor,
            execution_registry,
            stats: Arc::new(WorkerStats::default()),
        })
    }

    pub fn worker_id(&self) -> &str {
        &self.worker_id
    }

    pub fn stats_snapshot(&self) -> WorkerStatsSnapshot {
        WorkerStatsSnapshot {
            worker_id: self.worker_id.clone(),
            claimed_total: self.stats.claimed_total.load(Ordering::Relaxed),
            completed_total: self.stats.completed_total.load(Ordering::Relaxed),
            failed_total: self.stats.failed_total.load(Ordering::Relaxed),
            lease_renewal_failures_total: self
                .stats
                .lease_renewal_failures_total
                .load(Ordering::Relaxed),
            expired_leases_released_total: self
                .stats
                .expired_leases_released_total
                .load(Ordering::Relaxed),
            active_executions: self.stats.active_executions.load(Ordering::Relaxed),
            box_runtime_pool: None,
        }
    }

    pub fn prometheus_metrics(&self) -> String {
        self.stats_snapshot().to_prometheus()
    }

    pub async fn readiness(&self) -> Result<WorkerReadiness> {
        self.store.health_check().await?;
        let stats = self.store.repository().get_stats().await?;
        let mut readiness = build_worker_readiness(self.worker_id.clone(), stats);
        readiness.box_runtime_pool =
            Some(self.execution_registry.box_runtime_pool_snapshot().await);
        Ok(readiness)
    }

    pub async fn run(self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);

        #[cfg(unix)]
        let mut sigterm = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .map_err(|e| {
                LambdaError::Internal(format!("failed to install SIGTERM handler: {e}"))
            })?;

        tracing::info!(
            worker_id = %self.worker_id,
            poll_interval_ms = self.poll_interval.as_millis(),
            lease_ttl_secs = self.lease_ttl.as_secs(),
            heartbeat_interval_secs = self.heartbeat_interval.as_secs(),
            release_expired_leases = self.release_expired_leases,
            observability_enabled = self.observability_enabled,
            observability_bind = %format!("{}:{}", self.observability_host, self.observability_port),
            "starting pg lease worker"
        );

        let observability = if self.observability_enabled {
            Some(tokio::spawn(run_worker_observability_server(
                WorkerObservabilityState {
                    worker_id: self.worker_id.clone(),
                    store: self.store.clone(),
                    execution_registry: self.execution_registry.clone(),
                    stats: self.stats.clone(),
                    stale_heartbeat_after: self.lease_ttl,
                    last_window_snapshot: Arc::new(Mutex::new(None)),
                },
                self.observability_host.clone(),
                self.observability_port,
                shutdown_rx.clone(),
            )))
        } else {
            None
        };

        loop {
            #[cfg(unix)]
            {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    _ = sigterm.recv() => {
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    result = self.run_once(shutdown_rx.clone()) => {
                        result?;
                    }
                }
            }

            #[cfg(not(unix))]
            {
                tokio::select! {
                    _ = tokio::signal::ctrl_c() => {
                        let _ = shutdown_tx.send(true);
                        break;
                    }
                    result = self.run_once(shutdown_rx.clone()) => {
                        result?;
                    }
                }
            }
        }

        let stats = self.stats_snapshot();
        tracing::info!(
            worker_id = %stats.worker_id,
            claimed_total = stats.claimed_total,
            completed_total = stats.completed_total,
            failed_total = stats.failed_total,
            lease_renewal_failures_total = stats.lease_renewal_failures_total,
            expired_leases_released_total = stats.expired_leases_released_total,
            active_executions = stats.active_executions,
            "worker stopped"
        );
        tracing::debug!(
            metrics = %stats.to_prometheus(),
            "worker metrics snapshot"
        );

        if let Some(observability) = observability {
            let _ = observability.await.map_err(|error| {
                LambdaError::Internal(format!("worker observability task failed: {error}"))
            })?;
        }

        Ok(())
    }

    async fn run_once(&self, shutdown_rx: watch::Receiver<bool>) -> Result<()> {
        if self.release_expired_leases {
            let released = self.store.release_expired_leases().await?;
            if released > 0 {
                self.stats
                    .expired_leases_released_total
                    .fetch_add(released, Ordering::Relaxed);
                tracing::warn!(released, "released expired task leases");
            }
        }

        let Some(record) = self
            .store
            .claim_next_with_lease(&self.worker_id, self.lease_ttl)
            .await?
        else {
            tokio::time::sleep(self.poll_interval).await;
            return Ok(());
        };

        tracing::info!(
            worker_id = %self.worker_id,
            task_id = %record.id,
            lane = %record.lane,
            "claimed task with lease"
        );
        self.stats.claimed_total.fetch_add(1, Ordering::Relaxed);

        self.process_record(record, shutdown_rx).await
    }

    async fn process_record(
        &self,
        record: crate::TaskRecord,
        mut shutdown_rx: watch::Receiver<bool>,
    ) -> Result<()> {
        let task_id = record.id.clone();
        let heartbeat_store = self.store.clone();
        let heartbeat_task_id = task_id.clone();
        let heartbeat_worker_id = self.worker_id.clone();
        let heartbeat_interval = self.heartbeat_interval;
        let lease_ttl = self.lease_ttl;
        let heartbeat_stats = self.stats.clone();

        self.stats.active_executions.fetch_add(1, Ordering::Relaxed);

        let heartbeat = tokio::spawn(async move {
            loop {
                tokio::select! {
                    changed = shutdown_rx.changed() => {
                        if changed.is_ok() && *shutdown_rx.borrow() {
                            break;
                        }
                    }
                    _ = tokio::time::sleep(heartbeat_interval) => {
                        match heartbeat_store
                            .renew_lease(&heartbeat_task_id, &heartbeat_worker_id, lease_ttl)
                            .await
                        {
                            Ok(true) => {}
                            Ok(false) => {
                                heartbeat_stats
                                    .lease_renewal_failures_total
                                    .fetch_add(1, Ordering::Relaxed);
                                tracing::warn!(
                                    task_id = %heartbeat_task_id,
                                    worker_id = %heartbeat_worker_id,
                                    "lease renewal lost ownership"
                                );
                                break;
                            }
                            Err(error) => {
                                heartbeat_stats
                                    .lease_renewal_failures_total
                                    .fetch_add(1, Ordering::Relaxed);
                                tracing::error!(
                                    task_id = %heartbeat_task_id,
                                    worker_id = %heartbeat_worker_id,
                                    error = %error,
                                    "failed to renew task lease"
                                );
                                break;
                            }
                        }
                    }
                }
            }
        });

        let result = execute_faas_task_payload(
            &record.task,
            self.agent_executor.as_ref(),
            self.execution_registry.as_ref(),
        )
        .await;

        heartbeat.abort();
        let _ = heartbeat.await;
        self.stats.active_executions.fetch_sub(1, Ordering::Relaxed);

        match result {
            Ok(value) => {
                self.stats.completed_total.fetch_add(1, Ordering::Relaxed);
                self.store
                    .repository()
                    .complete_task(record.task.id, value)
                    .await?
            }
            Err(error) => {
                self.stats.failed_total.fetch_add(1, Ordering::Relaxed);
                self.store
                    .repository()
                    .fail_task(record.task.id, &error.to_string())
                    .await?
            }
        }

        let stats = self.stats_snapshot();
        tracing::info!(
            worker_id = %stats.worker_id,
            claimed_total = stats.claimed_total,
            completed_total = stats.completed_total,
            failed_total = stats.failed_total,
            lease_renewal_failures_total = stats.lease_renewal_failures_total,
            expired_leases_released_total = stats.expired_leases_released_total,
            active_executions = stats.active_executions,
            "worker stats updated"
        );
        tracing::debug!(
            metrics = %stats.to_prometheus(),
            "worker metrics snapshot"
        );

        Ok(())
    }
}

async fn run_worker_observability_server(
    state: WorkerObservabilityState,
    host: String,
    port: u16,
    mut shutdown_rx: watch::Receiver<bool>,
) -> Result<()> {
    let ip = host
        .parse::<std::net::IpAddr>()
        .map_err(|e| LambdaError::Internal(format!("invalid worker observability host: {e}")))?;
    let addr = SocketAddr::from((ip, port));

    let app = Router::new()
        .route("/health", get(worker_health))
        .route("/ready", get(worker_ready))
        .route("/stats", get(worker_stats))
        .route("/scaling", get(worker_scaling))
        .route("/metrics", get(worker_metrics))
        .with_state(Arc::new(state));

    tracing::info!("starting worker observability server on {}", addr);

    let listener = tokio::net::TcpListener::bind(&addr)
        .await
        .map_err(|e| LambdaError::Internal(format!("failed to bind worker observability: {e}")))?;

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            loop {
                if shutdown_rx.changed().await.is_err() || *shutdown_rx.borrow() {
                    break;
                }
            }
        })
        .await
        .map_err(|e| LambdaError::Internal(format!("worker observability server error: {e}")))?;

    Ok(())
}

async fn worker_health() -> Json<serde_json::Value> {
    Json(serde_json::json!({ "status": "healthy" }))
}

async fn worker_ready(
    State(state): State<Arc<WorkerObservabilityState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    match worker_readiness_response(
        state.worker_id.clone(),
        state.store.as_ref(),
        state.execution_registry.as_ref(),
    )
    .await
    {
        Ok(readiness) => (StatusCode::OK, Json(serde_json::json!(readiness))),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!(build_worker_readiness_failure(
                state.worker_id.clone(),
                error,
            ))),
        ),
    }
}

async fn worker_stats(
    State(state): State<Arc<WorkerObservabilityState>>,
) -> Json<WorkerStatsSnapshot> {
    Json(state.stats_snapshot().await)
}

async fn worker_scaling(
    State(state): State<Arc<WorkerObservabilityState>>,
) -> (StatusCode, Json<serde_json::Value>) {
    match worker_scaling_response(state.as_ref()).await {
        Ok(snapshot) => (
            StatusCode::OK,
            Json(serde_json::json!(build_worker_scaling_contract(snapshot))),
        ),
        Err(error) => (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!(build_worker_readiness_failure(
                state.worker_id.clone(),
                error,
            ))),
        ),
    }
}

async fn worker_metrics(State(state): State<Arc<WorkerObservabilityState>>) -> String {
    match worker_metrics_response(state.as_ref()).await {
        Ok(metrics) => metrics,
        Err(error) => format!(
            "# HELP a3s_lambda_worker_metrics_error Worker metrics collection errors\n\
             # TYPE a3s_lambda_worker_metrics_error gauge\n\
             a3s_lambda_worker_metrics_error{{worker_id=\"{worker_id}\",error=\"{error}\"}} 1\n",
            worker_id = state.worker_id,
            error = sanitize_prometheus_label_value(&error.to_string()),
        ),
    }
}

fn sanitize_prometheus_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

fn build_worker_readiness(worker_id: String, stats: TaskStats) -> WorkerReadiness {
    WorkerReadiness {
        status: "ready".to_string(),
        worker_id,
        task_store: "postgres".to_string(),
        database: "ok".to_string(),
        stats,
        box_runtime_pool: None,
    }
}

fn build_worker_scaling_snapshot(
    worker_id: String,
    worker: WorkerStatsSnapshot,
    task_stats: TaskStats,
    lease_health: LeaseHealthStats,
    window: WorkerWindowMeasurement,
) -> WorkerScalingSnapshot {
    let active_executions = worker.active_executions;
    let pending_tasks = task_stats.pending;
    let running_tasks = task_stats.running;
    let completed_total = worker.completed_total;
    let failed_total = worker.failed_total;
    let claimed_total = worker.claimed_total;
    let lease_renewal_failures_total = worker.lease_renewal_failures_total;
    let expired_leases_released_total = worker.expired_leases_released_total;
    let has_backlog = pending_tasks > 0;
    let backlog_per_active_execution = if active_executions == 0 {
        pending_tasks as f64
    } else {
        pending_tasks as f64 / active_executions as f64
    };
    let terminal_total = completed_total + failed_total;
    let failure_rate = if terminal_total == 0 {
        0.0
    } else {
        failed_total as f64 / terminal_total as f64
    };
    let lease_renewal_failure_rate = if claimed_total == 0 {
        0.0
    } else {
        lease_renewal_failures_total as f64 / claimed_total as f64
    };
    let expired_lease_recovery_rate = if claimed_total == 0 {
        0.0
    } else {
        expired_leases_released_total as f64 / claimed_total as f64
    };
    let has_failure_pressure = failed_total > 0 && failure_rate >= 0.2;
    let has_recovery_pressure =
        lease_renewal_failures_total > 0 || expired_leases_released_total > 0;
    let has_stuck_task_pressure = lease_health.running_with_expired_lease > 0
        || lease_health.running_without_lease > 0
        || lease_health.stale_heartbeats > 0;
    let recent_terminal_delta = window.completed_delta + window.failed_delta;
    let recent_failure_rate = if recent_terminal_delta == 0 {
        0.0
    } else {
        window.failed_delta as f64 / recent_terminal_delta as f64
    };
    let recent_lease_renewal_failure_rate = if window.claimed_delta == 0 {
        0.0
    } else {
        window.lease_renewal_failure_delta as f64 / window.claimed_delta as f64
    };
    let recent_recovery_rate = if window.claimed_delta == 0 {
        0.0
    } else {
        window.expired_lease_recovery_delta as f64 / window.claimed_delta as f64
    };
    let has_recent_failure_surge = window.failed_delta > 0 && recent_failure_rate >= 0.5;
    let has_recent_recovery_surge =
        window.lease_renewal_failure_delta > 0 || window.expired_lease_recovery_delta > 0;
    let scale_out_recommended = pending_tasks > active_executions
        || (has_backlog && has_recovery_pressure)
        || has_stuck_task_pressure
        || has_recent_failure_surge
        || has_recent_recovery_surge;
    let scale_in_recommended = pending_tasks == 0
        && active_executions == 0
        && !has_failure_pressure
        && !has_recovery_pressure
        && !has_stuck_task_pressure
        && !has_recent_failure_surge
        && !has_recent_recovery_surge;

    WorkerScalingSnapshot {
        worker_id,
        task_store: "postgres".to_string(),
        lease_health: lease_health.clone(),
        box_runtime_pool: worker.box_runtime_pool.clone(),
        signals: WorkerScalingSignals {
            pending_tasks,
            running_tasks,
            active_executions,
            backlog_per_active_execution,
            failure_rate,
            lease_renewal_failure_rate,
            expired_lease_recovery_rate,
            expired_lease_pressure: lease_health.running_with_expired_lease,
            stale_heartbeat_pressure: lease_health.stale_heartbeats,
            running_without_lease_pressure: lease_health.running_without_lease,
            recent_window_secs: window.window_secs,
            recent_claimed_delta: window.claimed_delta,
            recent_completed_delta: window.completed_delta,
            recent_failed_delta: window.failed_delta,
            recent_lease_renewal_failure_delta: window.lease_renewal_failure_delta,
            recent_expired_lease_recovery_delta: window.expired_lease_recovery_delta,
            recent_failure_rate,
            recent_lease_renewal_failure_rate,
            recent_recovery_rate,
            has_backlog,
            has_failure_pressure,
            has_recovery_pressure,
            has_stuck_task_pressure,
            has_recent_failure_surge,
            has_recent_recovery_surge,
            scale_out_recommended,
            scale_in_recommended,
        },
        task_stats,
        worker,
    }
}

fn build_worker_readiness_failure(worker_id: String, error: LambdaError) -> WorkerReadinessFailure {
    worker_observability_failure(worker_id, error.to_string())
}

async fn worker_readiness_response(
    worker_id: String,
    store: &PgTaskStore,
    execution_registry: &ExecutionRegistry,
) -> Result<WorkerReadiness> {
    store.health_check().await?;
    let stats = store.repository().get_stats().await?;
    let mut readiness = build_worker_readiness(worker_id, stats);
    readiness.box_runtime_pool = Some(execution_registry.box_runtime_pool_snapshot().await);
    Ok(readiness)
}

#[cfg(test)]
mod tests {
    use super::{
        build_worker_readiness, build_worker_readiness_failure, build_worker_scaling_contract,
        sanitize_prometheus_label_value, task_stats_to_prometheus, worker_health, worker_metrics,
        worker_ready, worker_scaling, worker_stats, WorkerObservabilityState, WorkerStats,
        WorkerStatsSnapshot,
    };
    use crate::{ExecutionRegistry, LambdaError, TaskStats};
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
        routing::get,
        Router,
    };
    use sqlx::{postgres::PgPoolOptions, query};
    use std::sync::atomic::Ordering;
    use std::sync::Arc;
    use std::sync::Mutex;
    use std::time::Duration;
    use tower::util::ServiceExt;
    use uuid::Uuid;

    use crate::observability::worker_observability_failure;
    use crate::storages::pg::{LeaseHealthStats, PgTaskStore};

    struct TestDatabase {
        base_url: String,
        scoped_url: String,
        schema: String,
    }

    impl TestDatabase {
        async fn create() -> Self {
            let base_url = std::env::var("A3S_LAMBDA_TEST_PG_URL")
                .expect("A3S_LAMBDA_TEST_PG_URL must be set for worker observability tests");
            let schema = format!("lambda_worker_test_{}", Uuid::new_v4().simple());

            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&base_url)
                .await
                .expect("failed to connect to base postgres database");

            query(&format!("CREATE SCHEMA IF NOT EXISTS \"{schema}\""))
                .execute(&pool)
                .await
                .expect("failed to create isolated test schema");

            let separator = if base_url.contains('?') { '&' } else { '?' };
            let scoped_url = format!("{base_url}{separator}options=-csearch_path%3D{schema}");

            Self {
                base_url,
                scoped_url,
                schema,
            }
        }

        async fn drop(self) {
            let pool = PgPoolOptions::new()
                .max_connections(1)
                .connect(&self.base_url)
                .await
                .expect("failed to reconnect to base postgres database");

            query(&format!(
                "DROP SCHEMA IF EXISTS \"{}\" CASCADE",
                self.schema
            ))
            .execute(&pool)
            .await
            .expect("failed to drop isolated test schema");
        }
    }

    fn worker_observability_router(state: WorkerObservabilityState) -> Router {
        Router::new()
            .route("/health", get(worker_health))
            .route("/ready", get(worker_ready))
            .route("/stats", get(worker_stats))
            .route("/scaling", get(worker_scaling))
            .route("/metrics", get(worker_metrics))
            .with_state(Arc::new(state))
    }

    async fn read_json_value(response: axum::response::Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), usize::MAX)
            .await
            .expect("failed to read body");
        serde_json::from_slice(&bytes).expect("failed to decode json")
    }

    #[test]
    fn renders_prometheus_worker_metrics() {
        let snapshot = WorkerStatsSnapshot {
            worker_id: "worker-a".into(),
            claimed_total: 5,
            completed_total: 4,
            failed_total: 1,
            lease_renewal_failures_total: 2,
            expired_leases_released_total: 3,
            active_executions: 1,
            box_runtime_pool: Some(crate::observability::BoxRuntimePoolSnapshot {
                launch_mode: crate::ExecutionLaunchMode::MicroVmPreferred,
                image_pool_count: 1,
                idle_vms: 2,
                active_vms: 1,
                total_vms: 3,
                max_total_vms: 10,
                available_vms: 7,
                occupancy_ratio: 0.3,
                active_ratio: 0.1,
                has_capacity_pressure: false,
            }),
        };

        let metrics = snapshot.to_prometheus();

        assert!(metrics.contains("a3s_lambda_worker_claimed_total{worker_id=\"worker-a\"} 5"));
        assert!(metrics.contains("a3s_lambda_worker_completed_total{worker_id=\"worker-a\"} 4"));
        assert!(metrics.contains("a3s_lambda_worker_failed_total{worker_id=\"worker-a\"} 1"));
        assert!(metrics
            .contains("a3s_lambda_worker_lease_renewal_failures_total{worker_id=\"worker-a\"} 2"));
        assert!(metrics
            .contains("a3s_lambda_worker_expired_leases_released_total{worker_id=\"worker-a\"} 3"));
        assert!(metrics.contains("a3s_lambda_worker_active_executions{worker_id=\"worker-a\"} 1"));
        assert!(metrics.contains("a3s_lambda_box_runtime_pool_idle_vms{worker_id=\"worker-a\"} 2"));
        assert!(
            metrics.contains("a3s_lambda_box_runtime_pool_available_vms{worker_id=\"worker-a\"} 7")
        );
        assert!(metrics
            .contains("a3s_lambda_box_runtime_pool_occupancy_ratio{worker_id=\"worker-a\"} 0.3"));
        assert!(metrics
            .contains("a3s_lambda_box_runtime_pool_capacity_pressure{worker_id=\"worker-a\"} 0"));
    }

    #[test]
    fn builds_readiness_payload() {
        let readiness = build_worker_readiness(
            "worker-a".into(),
            TaskStats {
                total: 10,
                pending: 3,
                running: 2,
                completed: 4,
                failed: 1,
                cancelled: 0,
            },
        );

        assert_eq!(readiness.status, "ready");
        assert_eq!(readiness.worker_id, "worker-a");
        assert_eq!(readiness.task_store, "postgres");
        assert_eq!(readiness.database, "ok");
        assert_eq!(readiness.stats.total, 10);
    }

    #[test]
    fn builds_not_ready_payload() {
        let failure = build_worker_readiness_failure(
            "worker-b".into(),
            LambdaError::Database("connection refused".into()),
        );

        assert_eq!(failure.status, "not_ready");
        assert_eq!(failure.worker_id, "worker-b");
        assert_eq!(failure.task_store, "postgres");
        assert_eq!(failure.database, "error");
        assert!(failure.error.contains("connection refused"));
    }

    #[test]
    fn shared_failure_builder_matches_worker_failure_payload() {
        let failure =
            worker_observability_failure("worker-b", "Database error: connection refused");

        assert_eq!(failure.status, "not_ready");
        assert_eq!(failure.worker_id, "worker-b");
        assert_eq!(failure.task_store, "postgres");
        assert_eq!(failure.database, "error");
        assert!(failure.error.contains("connection refused"));
    }

    #[test]
    fn renders_task_store_prometheus_metrics() {
        let metrics = task_stats_to_prometheus(
            "postgres",
            &TaskStats {
                total: 9,
                pending: 3,
                running: 2,
                completed: 2,
                failed: 1,
                cancelled: 1,
            },
        );

        assert!(metrics.contains("a3s_lambda_task_store_total{task_store=\"postgres\"} 9"));
        assert!(metrics.contains("a3s_lambda_task_store_pending{task_store=\"postgres\"} 3"));
        assert!(metrics.contains("a3s_lambda_task_store_running{task_store=\"postgres\"} 2"));
        assert!(metrics.contains("a3s_lambda_task_store_completed{task_store=\"postgres\"} 2"));
        assert!(metrics.contains("a3s_lambda_task_store_failed{task_store=\"postgres\"} 1"));
        assert!(metrics.contains("a3s_lambda_task_store_cancelled{task_store=\"postgres\"} 1"));
    }

    #[test]
    fn sanitizes_prometheus_label_values() {
        let value = sanitize_prometheus_label_value("db \"down\"\nretry\\later");
        assert_eq!(value, "db \\\"down\\\"\\nretry\\\\later");
    }

    #[test]
    fn builds_worker_scaling_snapshot() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 5,
                completed_total: 4,
                failed_total: 1,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 2,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 10,
                pending: 6,
                running: 3,
                completed: 1,
                failed: 0,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 1,
                running_without_lease: 0,
                stale_heartbeats: 2,
            },
            super::WorkerWindowMeasurement {
                window_secs: 15.0,
                claimed_delta: 4,
                completed_delta: 1,
                failed_delta: 2,
                lease_renewal_failure_delta: 1,
                expired_lease_recovery_delta: 1,
            },
        );

        assert_eq!(snapshot.worker_id, "worker-a");
        assert_eq!(snapshot.task_store, "postgres");
        assert_eq!(snapshot.signals.pending_tasks, 6);
        assert_eq!(snapshot.signals.running_tasks, 3);
        assert_eq!(snapshot.signals.active_executions, 2);
        assert_eq!(snapshot.signals.backlog_per_active_execution, 3.0);
        assert_eq!(snapshot.signals.failure_rate, 0.2);
        assert_eq!(snapshot.signals.lease_renewal_failure_rate, 0.0);
        assert_eq!(snapshot.signals.expired_lease_recovery_rate, 0.0);
        assert_eq!(snapshot.signals.expired_lease_pressure, 1);
        assert_eq!(snapshot.signals.stale_heartbeat_pressure, 2);
        assert_eq!(snapshot.signals.running_without_lease_pressure, 0);
        assert_eq!(snapshot.signals.recent_window_secs, 15.0);
        assert_eq!(snapshot.signals.recent_claimed_delta, 4);
        assert_eq!(snapshot.signals.recent_completed_delta, 1);
        assert_eq!(snapshot.signals.recent_failed_delta, 2);
        assert_eq!(snapshot.signals.recent_lease_renewal_failure_delta, 1);
        assert_eq!(snapshot.signals.recent_expired_lease_recovery_delta, 1);
        assert_eq!(snapshot.signals.recent_failure_rate, 2.0 / 3.0);
        assert_eq!(snapshot.signals.recent_lease_renewal_failure_rate, 0.25);
        assert_eq!(snapshot.signals.recent_recovery_rate, 0.25);
        assert!(snapshot.signals.has_backlog);
        assert!(snapshot.signals.has_failure_pressure);
        assert!(!snapshot.signals.has_recovery_pressure);
        assert!(snapshot.signals.has_stuck_task_pressure);
        assert!(snapshot.signals.has_recent_failure_surge);
        assert!(snapshot.signals.has_recent_recovery_surge);
        assert!(snapshot.signals.scale_out_recommended);
        assert!(!snapshot.signals.scale_in_recommended);
    }

    #[test]
    fn builds_worker_scaling_contract() {
        let before = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time before unix epoch")
            .as_secs();
        let contract = build_worker_scaling_contract(super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 1,
                completed_total: 1,
                failed_total: 0,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 1,
                pending: 0,
                running: 0,
                completed: 1,
                failed: 0,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 0.0,
                claimed_delta: 0,
                completed_delta: 0,
                failed_delta: 0,
                lease_renewal_failure_delta: 0,
                expired_lease_recovery_delta: 0,
            },
        ));
        let after = std::time::SystemTime::now()
            .duration_since(std::time::SystemTime::UNIX_EPOCH)
            .expect("system time after unix epoch")
            .as_secs();

        assert_eq!(contract.contract_version, "a3s.lambda.scaling.v1");
        assert!(contract.generated_at_unix_secs >= before);
        assert!(contract.generated_at_unix_secs <= after);
        assert_eq!(contract.snapshot.worker_id, "worker-a");
    }

    #[test]
    fn scaling_snapshot_recommends_scale_in_when_idle_and_healthy() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 10,
                completed_total: 10,
                failed_total: 0,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 10,
                pending: 0,
                running: 0,
                completed: 10,
                failed: 0,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 30.0,
                claimed_delta: 0,
                completed_delta: 0,
                failed_delta: 0,
                lease_renewal_failure_delta: 0,
                expired_lease_recovery_delta: 0,
            },
        );

        assert!(!snapshot.signals.scale_out_recommended);
        assert!(snapshot.signals.scale_in_recommended);
        assert!(!snapshot.signals.has_failure_pressure);
        assert!(!snapshot.signals.has_recovery_pressure);
        assert!(!snapshot.signals.has_stuck_task_pressure);
    }

    #[test]
    fn scaling_snapshot_recommends_scale_out_on_recent_recovery_surge() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 20,
                completed_total: 18,
                failed_total: 0,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 20,
                pending: 0,
                running: 0,
                completed: 18,
                failed: 0,
                cancelled: 2,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 15.0,
                claimed_delta: 4,
                completed_delta: 2,
                failed_delta: 0,
                lease_renewal_failure_delta: 1,
                expired_lease_recovery_delta: 1,
            },
        );

        assert!(snapshot.signals.has_recent_recovery_surge);
        assert!(snapshot.signals.scale_out_recommended);
        assert!(!snapshot.signals.scale_in_recommended);
    }

    #[test]
    fn scaling_snapshot_handles_zero_claimed_without_division() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 0,
                completed_total: 0,
                failed_total: 0,
                lease_renewal_failures_total: 5,
                expired_leases_released_total: 3,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 8,
                pending: 4,
                running: 0,
                completed: 0,
                failed: 0,
                cancelled: 4,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 10.0,
                claimed_delta: 0,
                completed_delta: 0,
                failed_delta: 0,
                lease_renewal_failure_delta: 2,
                expired_lease_recovery_delta: 1,
            },
        );

        assert_eq!(snapshot.signals.lease_renewal_failure_rate, 0.0);
        assert_eq!(snapshot.signals.expired_lease_recovery_rate, 0.0);
        assert_eq!(snapshot.signals.recent_lease_renewal_failure_rate, 0.0);
        assert_eq!(snapshot.signals.recent_recovery_rate, 0.0);
        assert!(snapshot.signals.has_backlog);
        assert!(snapshot.signals.scale_out_recommended);
    }

    #[test]
    fn scaling_snapshot_recommends_scale_out_on_stuck_task_pressure_alone() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 8,
                completed_total: 8,
                failed_total: 0,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 1,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 8,
                pending: 0,
                running: 1,
                completed: 7,
                failed: 0,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 1,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 20.0,
                claimed_delta: 0,
                completed_delta: 0,
                failed_delta: 0,
                lease_renewal_failure_delta: 0,
                expired_lease_recovery_delta: 0,
            },
        );

        assert!(snapshot.signals.has_stuck_task_pressure);
        assert!(!snapshot.signals.has_backlog);
        assert!(snapshot.signals.scale_out_recommended);
        assert!(!snapshot.signals.scale_in_recommended);
    }

    #[test]
    fn scaling_snapshot_recommends_scale_out_on_recent_failure_surge_alone() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 12,
                completed_total: 10,
                failed_total: 2,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 12,
                pending: 0,
                running: 0,
                completed: 10,
                failed: 2,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 15.0,
                claimed_delta: 4,
                completed_delta: 1,
                failed_delta: 3,
                lease_renewal_failure_delta: 0,
                expired_lease_recovery_delta: 0,
            },
        );

        assert!(snapshot.signals.has_recent_failure_surge);
        assert_eq!(snapshot.signals.recent_failure_rate, 0.75);
        assert!(snapshot.signals.scale_out_recommended);
        assert!(!snapshot.signals.scale_in_recommended);
    }

    #[test]
    fn scaling_snapshot_never_recommends_scale_in_when_recovery_pressure_exists() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 5,
                completed_total: 5,
                failed_total: 0,
                lease_renewal_failures_total: 1,
                expired_leases_released_total: 0,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 5,
                pending: 0,
                running: 0,
                completed: 5,
                failed: 0,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 30.0,
                claimed_delta: 0,
                completed_delta: 0,
                failed_delta: 0,
                lease_renewal_failure_delta: 0,
                expired_lease_recovery_delta: 0,
            },
        );

        assert!(snapshot.signals.has_recovery_pressure);
        assert!(!snapshot.signals.scale_in_recommended);
    }

    #[test]
    fn scaling_snapshot_never_recommends_scale_in_when_failure_pressure_exists() {
        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 10,
                completed_total: 4,
                failed_total: 2,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 0,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 6,
                pending: 0,
                running: 0,
                completed: 4,
                failed: 2,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            super::WorkerWindowMeasurement {
                window_secs: 30.0,
                claimed_delta: 0,
                completed_delta: 0,
                failed_delta: 0,
                lease_renewal_failure_delta: 0,
                expired_lease_recovery_delta: 0,
            },
        );

        assert!(snapshot.signals.has_failure_pressure);
        assert!(!snapshot.signals.scale_in_recommended);
    }

    #[test]
    fn first_window_snapshot_has_zero_recent_deltas() {
        let measurement = super::WorkerWindowMeasurement {
            window_secs: 0.0,
            claimed_delta: 0,
            completed_delta: 0,
            failed_delta: 0,
            lease_renewal_failure_delta: 0,
            expired_lease_recovery_delta: 0,
        };

        let snapshot = super::build_worker_scaling_snapshot(
            "worker-a".into(),
            WorkerStatsSnapshot {
                worker_id: "worker-a".into(),
                claimed_total: 3,
                completed_total: 2,
                failed_total: 1,
                lease_renewal_failures_total: 0,
                expired_leases_released_total: 0,
                active_executions: 1,
                box_runtime_pool: None,
            },
            TaskStats {
                total: 3,
                pending: 0,
                running: 1,
                completed: 2,
                failed: 1,
                cancelled: 0,
            },
            LeaseHealthStats {
                running_with_expired_lease: 0,
                running_without_lease: 0,
                stale_heartbeats: 0,
            },
            measurement,
        );

        assert_eq!(snapshot.signals.recent_window_secs, 0.0);
        assert_eq!(snapshot.signals.recent_claimed_delta, 0);
        assert_eq!(snapshot.signals.recent_completed_delta, 0);
        assert_eq!(snapshot.signals.recent_failed_delta, 0);
        assert_eq!(snapshot.signals.recent_failure_rate, 0.0);
        assert!(!snapshot.signals.has_recent_failure_surge);
        assert!(!snapshot.signals.has_recent_recovery_surge);
    }

    #[tokio::test]
    #[ignore = "requires A3S_LAMBDA_TEST_PG_URL and a reachable Postgres instance"]
    async fn worker_observability_endpoints_report_health_stats_metrics_and_ready() {
        let database = TestDatabase::create().await;
        let store = Arc::new(
            PgTaskStore::new(&database.scoped_url, 5)
                .await
                .expect("failed to open postgres task store"),
        );
        let stats = Arc::new(WorkerStats::default());
        stats.claimed_total.store(7, Ordering::Relaxed);
        stats.completed_total.store(5, Ordering::Relaxed);
        stats.failed_total.store(2, Ordering::Relaxed);
        stats
            .lease_renewal_failures_total
            .store(1, Ordering::Relaxed);
        stats
            .expired_leases_released_total
            .store(3, Ordering::Relaxed);
        stats.active_executions.store(4, Ordering::Relaxed);

        let app = worker_observability_router(WorkerObservabilityState {
            worker_id: "worker-test".into(),
            store,
            execution_registry: Arc::new(ExecutionRegistry::with_defaults()),
            stats,
            stale_heartbeat_after: Duration::from_secs(30),
            last_window_snapshot: Arc::new(Mutex::new(None)),
        });

        let health_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/health")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("health request failed");
        assert_eq!(health_response.status(), StatusCode::OK);
        let health = read_json_value(health_response).await;
        assert_eq!(health["status"], "healthy");

        let ready_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("ready request failed");
        assert_eq!(ready_response.status(), StatusCode::OK);
        let ready = read_json_value(ready_response).await;
        assert_eq!(ready["status"], "ready");
        assert_eq!(ready["worker_id"], "worker-test");
        assert_eq!(ready["database"], "ok");
        assert_eq!(ready["stats"]["total"], 0);

        let stats_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/stats")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("stats request failed");
        assert_eq!(stats_response.status(), StatusCode::OK);
        let snapshot = read_json_value(stats_response).await;
        assert_eq!(snapshot["worker_id"], "worker-test");
        assert_eq!(snapshot["claimed_total"], 7);
        assert_eq!(snapshot["completed_total"], 5);
        assert_eq!(snapshot["failed_total"], 2);
        assert_eq!(snapshot["active_executions"], 4);

        let scaling_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/scaling")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("scaling request failed");
        assert_eq!(scaling_response.status(), StatusCode::OK);
        let scaling = read_json_value(scaling_response).await;
        assert_eq!(scaling["contract_version"], "a3s.lambda.scaling.v1");
        assert!(
            scaling["generated_at_unix_secs"]
                .as_u64()
                .expect("generated_at should be u64")
                > 0
        );
        let snapshot = &scaling["snapshot"];
        assert_eq!(snapshot["worker_id"], "worker-test");
        assert_eq!(snapshot["task_store"], "postgres");
        assert_eq!(snapshot["lease_health"]["running_with_expired_lease"], 0);
        assert_eq!(snapshot["lease_health"]["running_without_lease"], 0);
        assert_eq!(snapshot["lease_health"]["stale_heartbeats"], 0);
        assert_eq!(snapshot["signals"]["pending_tasks"], 0);
        assert_eq!(snapshot["signals"]["running_tasks"], 0);
        assert_eq!(snapshot["signals"]["active_executions"], 4);
        assert_eq!(snapshot["signals"]["backlog_per_active_execution"], 0.0);
        assert_eq!(snapshot["signals"]["failure_rate"], 2.0 / 7.0);
        assert_eq!(snapshot["signals"]["lease_renewal_failure_rate"], 1.0 / 7.0);
        assert_eq!(
            snapshot["signals"]["expired_lease_recovery_rate"],
            3.0 / 7.0
        );
        assert_eq!(snapshot["signals"]["expired_lease_pressure"], 0);
        assert_eq!(snapshot["signals"]["stale_heartbeat_pressure"], 0);
        assert_eq!(snapshot["signals"]["running_without_lease_pressure"], 0);
        assert_eq!(snapshot["signals"]["has_backlog"], false);
        assert_eq!(snapshot["signals"]["has_failure_pressure"], true);
        assert_eq!(snapshot["signals"]["has_recovery_pressure"], true);
        assert_eq!(snapshot["signals"]["has_stuck_task_pressure"], false);
        assert_eq!(snapshot["signals"]["scale_out_recommended"], false);
        assert_eq!(snapshot["signals"]["scale_in_recommended"], false);
        assert_eq!(snapshot["signals"]["recent_claimed_delta"], 0);
        assert_eq!(snapshot["signals"]["recent_completed_delta"], 0);
        assert_eq!(snapshot["signals"]["recent_failed_delta"], 0);
        assert_eq!(snapshot["signals"]["recent_window_secs"], 0.0);
        assert_eq!(snapshot["box_runtime_pool"]["available_vms"], 0);
        assert_eq!(snapshot["box_runtime_pool"]["occupancy_ratio"], 0.0);
        assert_eq!(snapshot["box_runtime_pool"]["active_ratio"], 0.0);
        assert_eq!(snapshot["box_runtime_pool"]["has_capacity_pressure"], false);

        let scaling_response_2 = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/scaling")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("second scaling request failed");
        assert_eq!(scaling_response_2.status(), StatusCode::OK);
        let scaling_2 = read_json_value(scaling_response_2).await;
        let snapshot_2 = &scaling_2["snapshot"];
        assert!(
            snapshot_2["signals"]["recent_window_secs"]
                .as_f64()
                .expect("recent_window_secs should be f64")
                >= 0.0
        );
        assert_eq!(snapshot_2["signals"]["recent_claimed_delta"], 0);
        assert_eq!(snapshot_2["signals"]["recent_completed_delta"], 0);
        assert_eq!(snapshot_2["signals"]["recent_failed_delta"], 0);
        assert_eq!(
            snapshot_2["signals"]["recent_lease_renewal_failure_delta"],
            0
        );
        assert_eq!(
            snapshot_2["signals"]["recent_expired_lease_recovery_delta"],
            0
        );
        assert_eq!(snapshot_2["signals"]["recent_failure_rate"], 0.0);
        assert_eq!(
            snapshot_2["signals"]["recent_lease_renewal_failure_rate"],
            0.0
        );
        assert_eq!(snapshot_2["signals"]["recent_recovery_rate"], 0.0);
        assert_eq!(snapshot_2["signals"]["has_recent_failure_surge"], false);
        assert_eq!(snapshot_2["signals"]["has_recent_recovery_surge"], false);
        assert!(
            snapshot_2["signals"]["recent_window_secs"]
                .as_f64()
                .expect("recent_window_secs should be f64")
                >= snapshot["signals"]["recent_window_secs"]
                    .as_f64()
                    .expect("recent_window_secs should be f64")
        );

        let metrics_response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("metrics request failed");
        assert_eq!(metrics_response.status(), StatusCode::OK);
        let metrics_bytes = to_bytes(metrics_response.into_body(), usize::MAX)
            .await
            .expect("failed to read metrics body");
        let metrics = String::from_utf8(metrics_bytes.to_vec()).expect("metrics should be utf-8");
        assert!(metrics.contains("a3s_lambda_worker_claimed_total{worker_id=\"worker-test\"} 7"));
        assert!(metrics.contains("a3s_lambda_worker_completed_total{worker_id=\"worker-test\"} 5"));
        assert!(metrics.contains("a3s_lambda_worker_failed_total{worker_id=\"worker-test\"} 2"));
        assert!(
            metrics.contains("a3s_lambda_worker_active_executions{worker_id=\"worker-test\"} 4")
        );
        assert!(metrics
            .contains("a3s_lambda_box_runtime_pool_available_vms{worker_id=\"worker-test\"} 0"));
        assert!(metrics.contains("a3s_lambda_task_store_total{task_store=\"postgres\"} 0"));
        assert!(metrics.contains("a3s_lambda_task_store_pending{task_store=\"postgres\"} 0"));

        database.drop().await;
    }

    #[tokio::test]
    #[ignore = "requires A3S_LAMBDA_TEST_PG_URL and a reachable Postgres instance"]
    async fn worker_observability_endpoints_report_store_failures_consistently() {
        let database = TestDatabase::create().await;
        let store = Arc::new(
            PgTaskStore::new(&database.scoped_url, 5)
                .await
                .expect("failed to open postgres task store"),
        );
        let stats = Arc::new(WorkerStats::default());

        let app = worker_observability_router(WorkerObservabilityState {
            worker_id: "worker-test".into(),
            store,
            execution_registry: Arc::new(ExecutionRegistry::with_defaults()),
            stats,
            stale_heartbeat_after: Duration::from_secs(30),
            last_window_snapshot: Arc::new(Mutex::new(None)),
        });

        database.drop().await;

        let ready_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/ready")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("ready request failed");
        assert_eq!(ready_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let ready = read_json_value(ready_response).await;
        assert_eq!(ready["status"], "not_ready");
        assert_eq!(ready["worker_id"], "worker-test");
        assert_eq!(ready["task_store"], "postgres");
        assert_eq!(ready["database"], "error");
        assert!(ready["error"]
            .as_str()
            .expect("error should be string")
            .contains("Database error"));

        let scaling_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/scaling")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("scaling request failed");
        assert_eq!(scaling_response.status(), StatusCode::SERVICE_UNAVAILABLE);
        let scaling = read_json_value(scaling_response).await;
        assert_eq!(scaling["status"], "not_ready");
        assert_eq!(scaling["worker_id"], "worker-test");
        assert_eq!(scaling["task_store"], "postgres");
        assert_eq!(scaling["database"], "error");
        assert!(scaling["error"]
            .as_str()
            .expect("error should be string")
            .contains("Database error"));

        let metrics_response = app
            .oneshot(
                Request::builder()
                    .uri("/metrics")
                    .body(Body::empty())
                    .expect("build request"),
            )
            .await
            .expect("metrics request failed");
        assert_eq!(metrics_response.status(), StatusCode::OK);
        let metrics_bytes = to_bytes(metrics_response.into_body(), usize::MAX)
            .await
            .expect("failed to read metrics body");
        let metrics = String::from_utf8(metrics_bytes.to_vec()).expect("metrics should be utf-8");
        assert!(metrics.contains("a3s_lambda_worker_metrics_error"));
        assert!(metrics.contains("worker_id=\"worker-test\""));
        assert!(metrics.contains("error=\"Database error:"));
    }
}
