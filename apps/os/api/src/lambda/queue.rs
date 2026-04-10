//! Task queue integration with a3s-lane
//!
//! This module provides the integration between Lambda tasks and the a3s-lane
//! priority queue system. All workloads are scheduled as a3s-box-backed tasks,
//! even when their adapter semantics differ.

use std::sync::Arc;
use std::time::Duration;

use a3s_lane::{
    Command, EventEmitter, LaneConfig, LaneError, QueueManager, QueueManagerBuilder, RetryPolicy,
};
use async_trait::async_trait;
use serde_json::Value;

use crate::domain::{LambdaError, LambdaTask, TaskKind, TaskStatus};
use crate::executor::{agent::AgentExecutor, BoxRuntimeExecutor, ExecutionRegistry};
use crate::storages::TaskStore;

/// Queue command that wraps a task for execution in the shared a3s-box runtime.
pub struct LambdaCommand {
    task_id: String,
    task: LambdaTask,
    agent_executor: Arc<AgentExecutor>,
    execution_registry: Arc<ExecutionRegistry>,
    task_store: Option<Arc<dyn TaskStore>>,
}

impl LambdaCommand {
    pub fn new(
        task_id: String,
        task: LambdaTask,
        agent_executor: Arc<AgentExecutor>,
        execution_registry: Arc<ExecutionRegistry>,
        task_store: Option<Arc<dyn TaskStore>>,
    ) -> Self {
        Self {
            task_id,
            task,
            agent_executor,
            execution_registry,
            task_store,
        }
    }
}

pub async fn execute_faas_task_payload(
    task: &LambdaTask,
    agent_executor: &AgentExecutor,
    execution_registry: &ExecutionRegistry,
) -> a3s_lane::Result<Value> {
    let timeout = Duration::from_secs(task.timeout_secs as u64);
    BoxRuntimeExecutor::new(agent_executor, execution_registry)
        .execute(task, timeout)
        .await
        .map_err(LaneError::CommandError)
}

#[async_trait]
impl Command for LambdaCommand {
    async fn execute(&self) -> a3s_lane::Result<Value> {
        // Update status to running
        if let Some(ref store) = self.task_store {
            let _ = store
                .update_status(&self.task_id, TaskStatus::Running, None, None)
                .await;
        }

        let result = execute_faas_task_payload(
            &self.task,
            self.agent_executor.as_ref(),
            self.execution_registry.as_ref(),
        )
        .await;

        // Update status based on result
        if let Some(ref store) = self.task_store {
            match &result {
                Ok(value) => {
                    let _ = store
                        .update_status(
                            &self.task_id,
                            TaskStatus::Completed,
                            Some(value.to_string()),
                            None,
                        )
                        .await;
                }
                Err(e) => {
                    let _ = store
                        .update_status(&self.task_id, TaskStatus::Failed, None, Some(e.to_string()))
                        .await;
                }
            }
        }

        result
    }

    fn command_type(&self) -> &str {
        match &self.task.kind {
            TaskKind::AgentRun { .. } => "agent_run",
            TaskKind::Execution {
                executor, handler, ..
            } => {
                if executor == "http" && handler == "extract" {
                    "execution_http_extract"
                } else if executor == "http" && handler == "get" {
                    "execution_http_get"
                } else if executor == "http" && handler == "post" {
                    "execution_http_post"
                } else {
                    "execution"
                }
            }
        }
    }
}

/// Create a queue manager with optimized lane configuration for a3s-box workloads.
pub async fn create_faas_queue() -> Result<QueueManager, LambdaError> {
    let emitter = EventEmitter::new(1000);

    QueueManagerBuilder::new(emitter)
        // System lane: highest priority for health checks, cleanup
        .with_lane(
            "system",
            LaneConfig::new(1, 5)
                .with_timeout(Duration::from_secs(30))
                .with_retry_policy(RetryPolicy::exponential(2)),
            0,
        )
        // Realtime lane: low-latency API calls
        .with_lane(
            "realtime",
            LaneConfig::new(2, 10)
                .with_timeout(Duration::from_secs(60))
                .with_retry_policy(RetryPolicy::exponential(3))
                .with_pressure_threshold(20),
            1,
        )
        // Shared box-runtime lane for default workloads.
        .with_lane(
            "faas",
            LaneConfig::new(5, 50)
                .with_timeout(Duration::from_secs(300))
                .with_retry_policy(RetryPolicy::exponential(3))
                .with_pressure_threshold(100),
            2,
        )
        // Batch lane: low priority for batch processing
        .with_lane(
            "batch",
            LaneConfig::new(2, 20)
                .with_timeout(Duration::from_secs(600))
                .with_retry_policy(RetryPolicy::exponential(2))
                .with_pressure_threshold(50),
            3,
        )
        // Background lane: lowest priority for cleanup, stats
        .with_lane(
            "background",
            LaneConfig::new(1, 5)
                .with_timeout(Duration::from_secs(300))
                .with_retry_policy(RetryPolicy::none()),
            4,
        )
        .build()
        .await
        .map_err(|e| LambdaError::Internal(format!("failed to create queue: {}", e)))
}

/// Determine which lane a task should be submitted to
pub fn select_lane(task: &LambdaTask) -> &'static str {
    match &task.kind {
        TaskKind::AgentRun { timeout_secs, .. } => {
            if *timeout_secs <= 60 {
                "realtime" // Fast tasks
            } else {
                "faas" // Normal tasks
            }
        }
        TaskKind::Execution {
            timeout_secs,
            preferred_lane,
            ..
        } => {
            if let Some(lane) = preferred_lane.as_deref() {
                return match lane {
                    "system" => "system",
                    "realtime" => "realtime",
                    "faas" => "faas",
                    "batch" => "batch",
                    "background" => "background",
                    _ => default_execution_lane(*timeout_secs),
                };
            }

            default_execution_lane(*timeout_secs)
        }
    }
}

fn default_execution_lane(timeout_secs: u32) -> &'static str {
    if timeout_secs <= 30 {
        "realtime"
    } else {
        "batch"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TaskKind;
    use crate::{
        CapabilityMatchMode, ExecutionCapability, ExecutionPolicy, ExecutionTaskRequest,
        TaskRequest,
    };

    #[test]
    fn test_lane_selection() {
        // Fast agent task -> realtime
        let task = LambdaTask::new(TaskKind::AgentRun {
            agent: "test/agent".into(),
            version: "1.0.0".into(),
            input: serde_json::json!({}),
            box_runtime: None,
            timeout_secs: 30,
        });
        assert_eq!(select_lane(&task), "realtime");

        // Slow agent task -> faas
        let task = LambdaTask::new(TaskKind::AgentRun {
            agent: "test/agent".into(),
            version: "1.0.0".into(),
            input: serde_json::json!({}),
            box_runtime: None,
            timeout_secs: 300,
        });
        assert_eq!(select_lane(&task), "faas");

        // Quick execution -> realtime
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
            timeout_secs: 10,
        });
        assert_eq!(select_lane(&LambdaTask::new(task.kind)), "realtime");

        // Preferred lane overrides default
        let task = TaskRequest::execution_task(ExecutionTaskRequest {
            executor: "http".into(),
            handler: "extract".into(),
            input: serde_json::json!({"url": "https://example.com"}),
            required_capabilities: vec![ExecutionCapability::Network {
                protocol: "http".into(),
                operation: "extract".into(),
                scope: "public".into(),
            }],
            policy: ExecutionPolicy {
                capability_match_mode: CapabilityMatchMode::AllRequired,
                ..Default::default()
            },
            labels: Default::default(),
            preferred_lane: Some("background".into()),
            box_runtime: None,
            timeout_secs: 45,
        });
        assert_eq!(select_lane(&LambdaTask::new(task.kind)), "background");
    }
}
