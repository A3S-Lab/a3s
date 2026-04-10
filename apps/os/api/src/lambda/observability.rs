//! Shared worker observability contracts for control-plane integrations.
//!
//! These types are the stable Rust schema for Lambda worker health, scaling,
//! and autoscaling-facing signals. Runtime implementations inside `worker.rs`
//! should evolve behind these contracts rather than forcing control-plane
//! consumers such as `a3s-os` to re-define ad hoc JSON payloads.
//!
//! Current contract guidance:
//!
//! - `WorkerReadiness`: use for binary "can this worker serve?" checks
//! - `WorkerObservabilityFailure`: use for `/ready` and `/scaling` failure envelopes
//! - `WorkerStatsSnapshot`: use for lightweight worker-local counters
//! - `WorkerScalingContract`: use for scheduler and autoscaling decisions
//! - `WORKER_SCALING_CONTRACT_VERSION`: pin parser behavior in external systems

use crate::storages::pg::LeaseHealthStats;
use crate::{ExecutionLaunchMode, TaskStats};

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct BoxRuntimePoolSnapshot {
    pub launch_mode: ExecutionLaunchMode,
    pub image_pool_count: u64,
    pub idle_vms: u64,
    pub active_vms: u64,
    pub total_vms: u64,
    pub max_total_vms: u64,
    #[serde(default)]
    pub available_vms: u64,
    #[serde(default)]
    pub occupancy_ratio: f64,
    #[serde(default)]
    pub active_ratio: f64,
    #[serde(default)]
    pub has_capacity_pressure: bool,
}

pub const WORKER_SCALING_CONTRACT_VERSION: &str = "a3s.lambda.scaling.v1";

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerStatsSnapshot {
    pub worker_id: String,
    pub claimed_total: u64,
    pub completed_total: u64,
    pub failed_total: u64,
    pub lease_renewal_failures_total: u64,
    pub expired_leases_released_total: u64,
    pub active_executions: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_runtime_pool: Option<BoxRuntimePoolSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerReadiness {
    pub status: String,
    pub worker_id: String,
    pub task_store: String,
    pub database: String,
    pub stats: TaskStats,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_runtime_pool: Option<BoxRuntimePoolSnapshot>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerReadinessFailure {
    pub status: String,
    pub worker_id: String,
    pub task_store: String,
    pub database: String,
    pub error: String,
}

pub type WorkerObservabilityFailure = WorkerReadinessFailure;

pub fn worker_observability_failure(
    worker_id: impl Into<String>,
    error: impl Into<String>,
) -> WorkerObservabilityFailure {
    WorkerObservabilityFailure {
        status: "not_ready".to_string(),
        worker_id: worker_id.into(),
        task_store: "postgres".to_string(),
        database: "error".to_string(),
        error: error.into(),
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerScalingSnapshot {
    pub worker_id: String,
    pub task_store: String,
    pub task_stats: TaskStats,
    pub lease_health: LeaseHealthStats,
    pub worker: WorkerStatsSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub box_runtime_pool: Option<BoxRuntimePoolSnapshot>,
    pub signals: WorkerScalingSignals,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerScalingContract {
    pub contract_version: String,
    pub generated_at_unix_secs: u64,
    pub snapshot: WorkerScalingSnapshot,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct WorkerScalingSignals {
    pub pending_tasks: u64,
    pub running_tasks: u64,
    pub active_executions: u64,
    pub backlog_per_active_execution: f64,
    pub failure_rate: f64,
    pub lease_renewal_failure_rate: f64,
    pub expired_lease_recovery_rate: f64,
    pub expired_lease_pressure: u64,
    pub stale_heartbeat_pressure: u64,
    pub running_without_lease_pressure: u64,
    pub recent_window_secs: f64,
    pub recent_claimed_delta: u64,
    pub recent_completed_delta: u64,
    pub recent_failed_delta: u64,
    pub recent_lease_renewal_failure_delta: u64,
    pub recent_expired_lease_recovery_delta: u64,
    pub recent_failure_rate: f64,
    pub recent_lease_renewal_failure_rate: f64,
    pub recent_recovery_rate: f64,
    pub has_backlog: bool,
    pub has_failure_pressure: bool,
    pub has_recovery_pressure: bool,
    pub has_stuck_task_pressure: bool,
    pub has_recent_failure_surge: bool,
    pub has_recent_recovery_surge: bool,
    pub scale_out_recommended: bool,
    pub scale_in_recommended: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaling_contract_round_trips_through_json() {
        let contract = WorkerScalingContract {
            contract_version: WORKER_SCALING_CONTRACT_VERSION.to_string(),
            generated_at_unix_secs: 1_770_000_000,
            snapshot: WorkerScalingSnapshot {
                worker_id: "worker-a".into(),
                task_store: "postgres".into(),
                task_stats: TaskStats {
                    total: 10,
                    pending: 2,
                    running: 3,
                    completed: 4,
                    failed: 1,
                    cancelled: 0,
                },
                lease_health: LeaseHealthStats {
                    running_with_expired_lease: 1,
                    running_without_lease: 0,
                    stale_heartbeats: 2,
                },
                worker: WorkerStatsSnapshot {
                    worker_id: "worker-a".into(),
                    claimed_total: 7,
                    completed_total: 4,
                    failed_total: 1,
                    lease_renewal_failures_total: 1,
                    expired_leases_released_total: 2,
                    active_executions: 3,
                    box_runtime_pool: Some(BoxRuntimePoolSnapshot {
                        launch_mode: ExecutionLaunchMode::HostAdapterCompat,
                        image_pool_count: 1,
                        idle_vms: 2,
                        active_vms: 1,
                        total_vms: 3,
                        max_total_vms: 20,
                        available_vms: 17,
                        occupancy_ratio: 0.15,
                        active_ratio: 0.05,
                        has_capacity_pressure: false,
                    }),
                },
                box_runtime_pool: Some(BoxRuntimePoolSnapshot {
                    launch_mode: ExecutionLaunchMode::HostAdapterCompat,
                    image_pool_count: 1,
                    idle_vms: 2,
                    active_vms: 1,
                    total_vms: 3,
                    max_total_vms: 20,
                    available_vms: 17,
                    occupancy_ratio: 0.15,
                    active_ratio: 0.05,
                    has_capacity_pressure: false,
                }),
                signals: WorkerScalingSignals {
                    pending_tasks: 2,
                    running_tasks: 3,
                    active_executions: 3,
                    backlog_per_active_execution: 0.67,
                    failure_rate: 0.2,
                    lease_renewal_failure_rate: 0.14,
                    expired_lease_recovery_rate: 0.29,
                    expired_lease_pressure: 1,
                    stale_heartbeat_pressure: 2,
                    running_without_lease_pressure: 0,
                    recent_window_secs: 15.0,
                    recent_claimed_delta: 3,
                    recent_completed_delta: 1,
                    recent_failed_delta: 1,
                    recent_lease_renewal_failure_delta: 1,
                    recent_expired_lease_recovery_delta: 0,
                    recent_failure_rate: 0.5,
                    recent_lease_renewal_failure_rate: 0.33,
                    recent_recovery_rate: 0.0,
                    has_backlog: true,
                    has_failure_pressure: true,
                    has_recovery_pressure: true,
                    has_stuck_task_pressure: true,
                    has_recent_failure_surge: true,
                    has_recent_recovery_surge: true,
                    scale_out_recommended: true,
                    scale_in_recommended: false,
                },
            },
        };

        let json = serde_json::to_string(&contract).expect("serialize contract");
        let decoded: WorkerScalingContract =
            serde_json::from_str(&json).expect("deserialize contract");

        assert_eq!(decoded.contract_version, WORKER_SCALING_CONTRACT_VERSION);
        assert_eq!(decoded.snapshot.worker_id, "worker-a");
        assert_eq!(decoded.snapshot.signals.recent_failed_delta, 1);
        assert!(decoded.snapshot.signals.scale_out_recommended);
    }

    #[test]
    fn observability_failure_round_trips_through_json() {
        let failure = worker_observability_failure("worker-a", "Database error: connection reset");

        let json = serde_json::to_string(&failure).expect("serialize failure");
        let decoded: WorkerObservabilityFailure =
            serde_json::from_str(&json).expect("deserialize failure");

        assert_eq!(decoded.status, "not_ready");
        assert_eq!(decoded.worker_id, "worker-a");
        assert_eq!(decoded.task_store, "postgres");
        assert_eq!(decoded.database, "error");
        assert!(decoded.error.contains("Database error"));
    }

    #[test]
    fn box_runtime_pool_snapshot_backfills_new_fields_when_missing() {
        let json = serde_json::json!({
            "launch_mode": "host_adapter_compat",
            "image_pool_count": 1,
            "idle_vms": 2,
            "active_vms": 1,
            "total_vms": 3,
            "max_total_vms": 20
        });

        let decoded: BoxRuntimePoolSnapshot =
            serde_json::from_value(json).expect("deserialize legacy pool snapshot");

        assert_eq!(decoded.available_vms, 0);
        assert_eq!(decoded.occupancy_ratio, 0.0);
        assert_eq!(decoded.active_ratio, 0.0);
        assert!(!decoded.has_capacity_pressure);
    }
}
