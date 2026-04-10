//! Pod Lifecycle Controller - Pod Lifecycle Management.
//!
//! PodLifecycleController manages the full pod lifecycle including:
//! - Finalizer management
//! - Graceful shutdown
//! - Pod status transitions
//! - Orphaned pod cleanup
//! - Stuck pod detection and recovery

use crate::errors::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Pod lifecycle phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LifecyclePhase {
    /// Pod is being created.
    Pending,
    /// Pod is running.
    Running,
    /// Pod succeeded.
    Succeeded,
    /// Pod failed.
    Failed,
    /// Pod is being deleted.
    Terminating,
    /// Unknown state.
    Unknown,
}

/// Lifecycle condition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum LifecycleConditionType {
    /// Pod initialized.
    Initialized,
    /// Containers ready.
    ContainersReady,
    /// Ready.
    Ready,
    /// Pod scheduled.
    PodScheduled,
    /// Disruption target.
    DisruptionTarget,
}

/// Lifecycle condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LifecycleCondition {
    /// Condition type.
    pub condition_type: LifecycleConditionType,
    /// Status.
    pub status: ConditionStatus__,
    /// Last transition time.
    pub last_transition_time: DateTime<Utc>,
    /// Last update time.
    pub last_update_time: DateTime<Utc>,
    /// Reason.
    pub reason: Option<String>,
    /// Message.
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConditionStatus__ {
    True,
    False,
    Unknown,
}

/// Pod termination grace period.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GracePeriod {
    /// Seconds to wait.
    pub seconds: i64,
    /// Was set explicitly.
    pub was_set_explicitly: bool,
    /// Grace period deadline.
    pub deadline: Option<DateTime<Utc>>,
}

/// Finalizer entry.
#[derive(Debug, Clone)]
pub struct FinalizerEntry {
    /// Finalizer name.
    pub name: String,
    /// Added at.
    pub added_at: DateTime<Utc>,
    /// Data associated with finalizer.
    pub data: HashMap<String, String>,
}

/// Pod lifecycle state.
#[derive(Debug, Clone)]
pub struct PodLifecycleState {
    /// Pod key (namespace/name).
    pub pod_key: String,
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
    /// Current phase.
    pub phase: LifecyclePhase,
    /// Conditions.
    pub conditions: Vec<LifecycleCondition>,
    /// Finalizers.
    pub finalizers: Vec<FinalizerEntry>,
    /// Deletion timestamp.
    pub deletion_timestamp: Option<DateTime<Utc>>,
    /// Grace period.
    pub grace_period: GracePeriod,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Started at.
    pub started_at: Option<DateTime<Utc>>,
    /// Last state update.
    pub last_update: DateTime<Utc>,
    /// Init containers completed.
    pub init_containers_completed: bool,
    /// Containers ready.
    pub containers_ready: bool,
}

/// Deletion result.
#[derive(Debug, Clone)]
pub struct DeletionResult {
    /// Pod was deleted.
    pub deleted: bool,
    /// Finalizers remaining.
    pub finalizers_remaining: Vec<String>,
    /// Time until cleanup.
    pub time_until_cleanup: Option<Duration>,
    /// Error if any.
    pub error: Option<String>,
}

/// Lifecycle event.
#[derive(Debug, Clone)]
pub enum LifecycleEvent {
    /// Pod created.
    PodCreated {
        namespace: String,
        name: String,
        uid: String,
    },
    /// Pod started.
    PodStarted { namespace: String, name: String },
    /// Pod terminated.
    PodTerminated {
        namespace: String,
        name: String,
        exit_code: i32,
    },
    /// Pod deleted.
    PodDeleted { namespace: String, name: String },
    /// Finalizer added.
    FinalizerAdded {
        namespace: String,
        name: String,
        finalizer: String,
    },
    /// Finalizer removed.
    FinalizerRemoved {
        namespace: String,
        name: String,
        finalizer: String,
    },
    /// Grace period expired.
    GracePeriodExpired { namespace: String, name: String },
}

/// PodLifecycleController manages pod lifecycle.
pub struct PodLifecycleController {
    /// Pod states.
    pods: RwLock<HashMap<String, PodLifecycleState>>,
    /// Default grace period in seconds.
    default_grace_period_secs: i64,
    /// Finalizer registry.
    finalizers: RwLock<HashMap<String, FinalizerHandler>>,
    /// Running state.
    running: RwLock<bool>,
}

/// Finalizer handler.
#[derive(Debug, Clone)]
pub struct FinalizerHandler {
    /// Name.
    pub name: String,
    /// Description.
    pub description: String,
    /// On add callback key (async fn name).
    pub on_add: Option<String>,
    /// On remove callback key.
    pub on_remove: Option<String>,
    /// Priority.
    pub priority: i32,
}

impl PodLifecycleController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            pods: RwLock::new(HashMap::new()),
            default_grace_period_secs: 30,
            finalizers: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Register default finalizers
        self.register_default_finalizers().await?;

        tracing::info!(
            grace_period_secs = self.default_grace_period_secs,
            "PodLifecycleController started"
        );

        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("PodLifecycleController stopped");
        Ok(())
    }

    /// Register default finalizers.
    async fn register_default_finalizers(&self) -> Result<()> {
        let mut finalizers = self.finalizers.write().await;

        finalizers.insert(
            "a3s.io/pod-gc".to_string(),
            FinalizerHandler {
                name: "a3s.io/pod-gc".to_string(),
                description: "Cleans up pod resources on deletion".to_string(),
                on_add: None,
                on_remove: Some("cleanup_pod".to_string()),
                priority: 100,
            },
        );

        finalizers.insert(
            "a3s.io volume-protection".to_string(),
            FinalizerHandler {
                name: "a3s.io/volume-protection".to_string(),
                description: "Prevents volume deletion while pod uses it".to_string(),
                on_add: None,
                on_remove: Some("unmount_volumes".to_string()),
                priority: 50,
            },
        );

        finalizers.insert(
            "a3s.io/network-protection".to_string(),
            FinalizerHandler {
                name: "a3s.io/network-protection".to_string(),
                description: "Prevents network cleanup while pod has endpoints".to_string(),
                on_add: None,
                on_remove: Some("cleanup_network".to_string()),
                priority: 50,
            },
        );

        Ok(())
    }

    /// Register a pod.
    pub async fn register_pod(
        &self,
        namespace: &str,
        name: &str,
        uid: &str,
        finalizer_names: Vec<String>,
    ) -> Result<()> {
        let pod_key = format!("{}/{}", namespace, name);

        let finalizers: Vec<FinalizerEntry> = finalizer_names
            .into_iter()
            .map(|name| FinalizerEntry {
                name,
                added_at: Utc::now(),
                data: HashMap::new(),
            })
            .collect();

        let state = PodLifecycleState {
            pod_key: pod_key.clone(),
            namespace: namespace.to_string(),
            name: name.to_string(),
            uid: uid.to_string(),
            phase: LifecyclePhase::Pending,
            conditions: vec![
                LifecycleCondition {
                    condition_type: LifecycleConditionType::Initialized,
                    status: ConditionStatus__::False,
                    last_transition_time: Utc::now(),
                    last_update_time: Utc::now(),
                    reason: Some("WaitForInitContainers".to_string()),
                    message: Some("init containers are running".to_string()),
                },
                LifecycleCondition {
                    condition_type: LifecycleConditionType::ContainersReady,
                    status: ConditionStatus__::False,
                    last_transition_time: Utc::now(),
                    last_update_time: Utc::now(),
                    reason: Some("ContainersNotReady".to_string()),
                    message: Some("containers are not ready".to_string()),
                },
                LifecycleCondition {
                    condition_type: LifecycleConditionType::Ready,
                    status: ConditionStatus__::False,
                    last_transition_time: Utc::now(),
                    last_update_time: Utc::now(),
                    reason: Some("ContainersNotReady".to_string()),
                    message: Some("containers are not ready".to_string()),
                },
                LifecycleCondition {
                    condition_type: LifecycleConditionType::PodScheduled,
                    status: ConditionStatus__::False,
                    last_transition_time: Utc::now(),
                    last_update_time: Utc::now(),
                    reason: Some("WaitForScheduling".to_string()),
                    message: Some("pod is not scheduled".to_string()),
                },
            ],
            finalizers,
            deletion_timestamp: None,
            grace_period: GracePeriod {
                seconds: self.default_grace_period_secs,
                was_set_explicitly: false,
                deadline: None,
            },
            created_at: Utc::now(),
            started_at: None,
            last_update: Utc::now(),
            init_containers_completed: false,
            containers_ready: false,
        };

        let mut pods = self.pods.write().await;
        pods.insert(pod_key, state);

        tracing::debug!(namespace = %namespace, name = %name, "Pod registered");
        Ok(())
    }

    /// Set pod phase to running.
    pub async fn pod_started(&self, namespace: &str, name: &str) -> Result<()> {
        let pod_key = format!("{}/{}", namespace, name);
        let mut pods = self.pods.write().await;

        if let Some(pod) = pods.get_mut(&pod_key) {
            pod.phase = LifecyclePhase::Running;
            pod.started_at = Some(Utc::now());
            pod.last_update = Utc::now();

            // Update conditions
            for cond in &mut pod.conditions {
                if cond.condition_type == LifecycleConditionType::PodScheduled {
                    cond.status = ConditionStatus__::True;
                    cond.last_transition_time = Utc::now();
                    cond.reason = Some("SuccessfullyScheduled".to_string());
                    cond.message = Some("pod is scheduled".to_string());
                }
            }

            tracing::info!(namespace = %namespace, name = %name, "Pod started");
        }

        Ok(())
    }

    /// Set containers ready.
    pub async fn containers_ready(&self, namespace: &str, name: &str) -> Result<()> {
        let pod_key = format!("{}/{}", namespace, name);
        let mut pods = self.pods.write().await;

        if let Some(pod) = pods.get_mut(&pod_key) {
            pod.containers_ready = true;
            pod.last_update = Utc::now();

            for cond in &mut pod.conditions {
                match cond.condition_type {
                    LifecycleConditionType::Initialized => {
                        cond.status = ConditionStatus__::True;
                        cond.last_transition_time = Utc::now();
                    }
                    LifecycleConditionType::ContainersReady => {
                        cond.status = ConditionStatus__::True;
                        cond.last_transition_time = Utc::now();
                        cond.reason = Some("ContainersReady".to_string());
                    }
                    LifecycleConditionType::Ready => {
                        cond.status = ConditionStatus__::True;
                        cond.last_transition_time = Utc::now();
                        cond.reason = Some("ContainersReady".to_string());
                    }
                    _ => {}
                }
            }

            tracing::debug!(namespace = %namespace, name = %name, "Containers ready");
        }

        Ok(())
    }

    /// Initiate pod deletion.
    pub async fn initiate_deletion(
        &self,
        namespace: &str,
        name: &str,
        grace_period_secs: Option<i64>,
    ) -> Result<()> {
        let pod_key = format!("{}/{}", namespace, name);
        let mut pods = self.pods.write().await;

        if let Some(pod) = pods.get_mut(&pod_key) {
            let grace_period = grace_period_secs.unwrap_or(pod.grace_period.seconds);

            pod.phase = LifecyclePhase::Terminating;
            pod.deletion_timestamp = Some(Utc::now());
            pod.grace_period = GracePeriod {
                seconds: grace_period,
                was_set_explicitly: grace_period_secs.is_some(),
                deadline: Some(Utc::now() + Duration::seconds(grace_period)),
            };
            pod.last_update = Utc::now();

            tracing::info!(
                namespace = %namespace,
                name = %name,
                grace_period = %grace_period,
                finalizers = pod.finalizers.len(),
                "Pod deletion initiated"
            );
        }

        Ok(())
    }

    /// Remove finalizer.
    pub async fn remove_finalizer(
        &self,
        namespace: &str,
        name: &str,
        finalizer: &str,
    ) -> Result<()> {
        let pod_key = format!("{}/{}", namespace, name);
        let mut pods = self.pods.write().await;

        if let Some(pod) = pods.get_mut(&pod_key) {
            pod.finalizers.retain(|f| f.name != finalizer);
            pod.last_update = Utc::now();

            tracing::debug!(
                namespace = %namespace,
                name = %name,
                finalizer = %finalizer,
                remaining = pod.finalizers.len(),
                "Finalizer removed"
            );
        }

        Ok(())
    }

    /// Check if pod can be deleted.
    pub async fn can_delete(&self, namespace: &str, name: &str) -> DeletionResult {
        let pod_key = format!("{}/{}", namespace, name);
        let pods = self.pods.read().await;

        if let Some(pod) = pods.get(&pod_key) {
            if pod.finalizers.is_empty() {
                return DeletionResult {
                    deleted: true,
                    finalizers_remaining: vec![],
                    time_until_cleanup: None,
                    error: None,
                };
            }

            let time_until = pod
                .grace_period
                .deadline
                .map(|d| d - Utc::now())
                .filter(|d| d.num_seconds() > 0);

            DeletionResult {
                deleted: false,
                finalizers_remaining: pod.finalizers.iter().map(|f| f.name.clone()).collect(),
                time_until_cleanup: time_until,
                error: None,
            }
        } else {
            DeletionResult {
                deleted: true,
                finalizers_remaining: vec![],
                time_until_cleanup: None,
                error: None,
            }
        }
    }

    /// Finalize pod deletion.
    pub async fn finalize_deletion(&self, namespace: &str, name: &str) -> Result<bool> {
        let pod_key = format!("{}/{}", namespace, name);
        let mut pods = self.pods.write().await;

        if let Some(pod) = pods.get_mut(&pod_key) {
            // Execute finalizer cleanup handlers
            for finalizer in &pod.finalizers {
                tracing::debug!(
                    namespace = %namespace,
                    name = %name,
                    finalizer = %finalizer.name,
                    "Running finalizer cleanup"
                );
                // In real implementation, would call the registered handler
            }

            // Remove pod
            pods.remove(&pod_key);

            tracing::info!(namespace = %namespace, name = %name, "Pod finalized");
            return Ok(true);
        }

        Ok(false)
    }

    /// Check for stuck pods (grace period expired).
    pub async fn check_stuck_pods(&self) -> Vec<PodLifecycleState> {
        let mut stuck = Vec::new();
        let pods = self.pods.read().await;
        let now = Utc::now();

        for (_, pod) in pods.iter() {
            if pod.phase == LifecyclePhase::Terminating {
                if let Some(deadline) = pod.grace_period.deadline {
                    if now > deadline {
                        stuck.push(pod.clone());
                    }
                }
            }
        }

        stuck
    }

    /// Get pod state.
    pub async fn get_pod(&self, namespace: &str, name: &str) -> Option<PodLifecycleState> {
        let pod_key = format!("{}/{}", namespace, name);
        let pods = self.pods.read().await;
        pods.get(&pod_key).cloned()
    }

    /// List all pods.
    pub async fn list_pods(&self) -> Vec<PodLifecycleState> {
        let pods = self.pods.read().await;
        pods.values().cloned().collect()
    }

    /// List terminating pods.
    pub async fn list_terminating_pods(&self) -> Vec<PodLifecycleState> {
        let pods = self.pods.read().await;
        pods.values()
            .filter(|p| p.phase == LifecyclePhase::Terminating)
            .cloned()
            .collect()
    }

    /// Get pending deletion count.
    pub async fn pending_deletions(&self) -> usize {
        let pods = self.pods.read().await;
        pods.values()
            .filter(|p| p.phase == LifecyclePhase::Terminating)
            .count()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for PodLifecycleController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_register_pod() {
        let controller = PodLifecycleController::new();
        controller.start().await.unwrap();

        controller
            .register_pod(
                "default",
                "test-pod",
                "uid-123",
                vec!["a3s.io/pod-gc".to_string()],
            )
            .await
            .unwrap();

        let pod = controller.get_pod("default", "test-pod").await;
        assert!(pod.is_some());
        assert_eq!(pod.unwrap().phase, LifecyclePhase::Pending);
    }

    #[tokio::test]
    async fn test_pod_lifecycle() {
        let controller = PodLifecycleController::new();
        controller.start().await.unwrap();

        controller
            .register_pod("default", "test-pod", "uid-123", vec![])
            .await
            .unwrap();

        controller.pod_started("default", "test-pod").await.unwrap();
        controller
            .containers_ready("default", "test-pod")
            .await
            .unwrap();

        let pod = controller.get_pod("default", "test-pod").await.unwrap();
        assert_eq!(pod.phase, LifecyclePhase::Running);
    }

    #[tokio::test]
    async fn test_deletion_workflow() {
        let controller = PodLifecycleController::new();
        controller.start().await.unwrap();

        controller
            .register_pod(
                "default",
                "test-pod",
                "uid-123",
                vec![
                    "a3s.io/pod-gc".to_string(),
                    "a3s.io/volume-protection".to_string(),
                ],
            )
            .await
            .unwrap();

        controller
            .initiate_deletion("default", "test-pod", Some(10))
            .await
            .unwrap();

        let result = controller.can_delete("default", "test-pod").await;
        assert!(!result.deleted);
        assert_eq!(result.finalizers_remaining.len(), 2);

        // Remove finalizers
        controller
            .remove_finalizer("default", "test-pod", "a3s.io/pod-gc")
            .await
            .unwrap();
        controller
            .remove_finalizer("default", "test-pod", "a3s.io/volume-protection")
            .await
            .unwrap();

        let result = controller.can_delete("default", "test-pod").await;
        assert!(result.deleted);
    }

    #[tokio::test]
    async fn test_stuck_pods() {
        let controller = PodLifecycleController::new();
        controller.start().await.unwrap();

        controller
            .register_pod("default", "stuck-pod", "uid-456", vec![])
            .await
            .unwrap();

        controller
            .initiate_deletion("default", "stuck-pod", Some(0))
            .await
            .unwrap();

        // Wait a tiny bit
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        let stuck = controller.check_stuck_pods().await;
        assert!(!stuck.is_empty());
    }
}
