//! Namespace Controller - Namespace Lifecycle Management.
//!
//! NamespaceController manages namespace lifecycle:
//! - Namespace phase transitions (Active, Terminating)
//! - Finalizer management
//! - Namespace deletion coordination
//! - Resource tracking within namespace

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Namespace phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamespacePhase {
    /// Namespace is active.
    Active,
    /// Namespace is terminating.
    Terminating,
    /// Namespace is being finalized.
    Finalizing,
}

/// Namespace condition status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConditionStatus___ {
    True,
    False,
    Unknown,
}

/// Namespace condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceCondition {
    /// Condition type.
    pub condition_type: NamespaceConditionType,
    /// Status.
    pub status: ConditionStatus___,
    /// Last transition time.
    pub last_transition_time: DateTime<Utc>,
    /// Reason.
    pub reason: String,
    /// Message.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NamespaceConditionType {
    /// Namespace has all content removed.
    NamespaceIsEmpty,
    /// Namespace deletion is complete.
    NamespaceDeletionComplete,
}

/// Namespace status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceStatus {
    /// Phase.
    pub phase: NamespacePhase,
    /// Conditions.
    pub conditions: Vec<NamespaceCondition>,
    /// Resource count by kind.
    pub resource_count: HashMap<String, usize>,
    /// Finalizers.
    pub finalizers: Vec<String>,
    /// Deletion timestamp.
    pub deletion_timestamp: Option<DateTime<Utc>>,
}

/// Namespace desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceDesired {
    /// Name.
    pub name: String,
    /// Labels.
    pub labels: HashMap<String, String>,
    /// Annotations.
    pub annotations: HashMap<String, String>,
    /// Status.
    pub status: NamespaceStatus,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

/// Namespace controller state.
#[derive(Debug, Clone)]
pub struct NamespaceControllerState {
    /// Namespace name.
    pub name: String,
    /// Phase.
    pub phase: NamespacePhase,
    /// Resource counts.
    pub resource_counts: HashMap<String, usize>,
    /// Finalizers.
    pub finalizers: Vec<String>,
    /// Deletion timestamp.
    pub deletion_timestamp: Option<DateTime<Utc>>,
    /// Status conditions.
    pub conditions: Vec<NamespaceCondition>,
}

/// Deletion progress.
#[derive(Debug, Clone)]
pub struct NamespaceDeletionProgress {
    /// Namespace.
    pub namespace: String,
    /// Active resources remaining.
    pub active_resources: HashMap<String, usize>,
    /// Terminating resources remaining.
    pub terminating_resources: HashMap<String, usize>,
    /// All resources removed.
    pub all_resources_removed: bool,
    /// Time remaining estimate.
    pub time_remaining_seconds: Option<i64>,
}

/// NamespaceController manages namespace lifecycle.
pub struct NamespaceController {
    /// Namespaces.
    namespaces: RwLock<HashMap<String, NamespaceControllerState>>,
    /// Default finalizers.
    default_finalizers: Vec<String>,
    /// Finalization timeout in seconds.
    finalization_timeout_secs: i64,
    /// Running state.
    running: RwLock<bool>,
}

impl NamespaceController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            namespaces: RwLock::new(HashMap::new()),
            default_finalizers: vec!["a3s.io/namespace-protection".to_string()],
            finalization_timeout_secs: 300, // 5 minutes
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        tracing::info!(
            default_finalizers = ?self.default_finalizers,
            finalization_timeout_secs = self.finalization_timeout_secs,
            "NamespaceController started"
        );
        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("NamespaceController stopped");
        Ok(())
    }

    /// Register namespace.
    pub async fn register_namespace(&self, name: &str) -> Result<()> {
        let mut namespaces = self.namespaces.write().await;

        namespaces.insert(
            name.to_string(),
            NamespaceControllerState {
                name: name.to_string(),
                phase: NamespacePhase::Active,
                resource_counts: HashMap::new(),
                finalizers: self.default_finalizers.clone(),
                deletion_timestamp: None,
                conditions: vec![NamespaceCondition {
                    condition_type: NamespaceConditionType::NamespaceIsEmpty,
                    status: ConditionStatus___::True,
                    last_transition_time: Utc::now(),
                    reason: "NamespaceCreated".to_string(),
                    message: "Namespace is active".to_string(),
                }],
            },
        );

        tracing::debug!(namespace = %name, "Namespace registered");
        Ok(())
    }

    /// Initiate namespace deletion.
    pub async fn initiate_deletion(&self, name: &str) -> Result<()> {
        let mut namespaces = self.namespaces.write().await;

        if let Some(ns) = namespaces.get_mut(name) {
            ns.phase = NamespacePhase::Terminating;
            ns.deletion_timestamp = Some(Utc::now());

            // Update condition
            for cond in &mut ns.conditions {
                if cond.condition_type == NamespaceConditionType::NamespaceIsEmpty {
                    cond.status = ConditionStatus___::False;
                    cond.last_transition_time = Utc::now();
                    cond.reason = "NamespaceDeletionStarted".to_string();
                    cond.message = "Namespace is being deleted".to_string();
                }
            }

            tracing::info!(namespace = %name, "Namespace deletion initiated");
        }

        Ok(())
    }

    /// Remove finalizer.
    pub async fn remove_finalizer(&self, namespace: &str, finalizer: &str) -> Result<()> {
        let mut namespaces = self.namespaces.write().await;

        if let Some(ns) = namespaces.get_mut(namespace) {
            ns.finalizers.retain(|f| *f != finalizer);

            tracing::debug!(
                namespace = %namespace,
                finalizer = %finalizer,
                remaining = ns.finalizers.len(),
                "Finalizer removed from namespace"
            );
        }

        Ok(())
    }

    /// Check if namespace can be deleted.
    pub async fn can_delete(&self, name: &str) -> bool {
        let namespaces = self.namespaces.read().await;

        if let Some(ns) = namespaces.get(name) {
            ns.finalizers.is_empty()
        } else {
            true
        }
    }

    /// Update resource count for a kind.
    pub async fn update_resource_count(
        &self,
        namespace: &str,
        kind: &str,
        count: usize,
    ) -> Result<()> {
        let mut namespaces = self.namespaces.write().await;

        if let Some(ns) = namespaces.get_mut(namespace) {
            ns.resource_counts.insert(kind.to_string(), count);

            // Update NamespaceIsEmpty condition
            let total: usize = ns.resource_counts.values().sum();
            for cond in &mut ns.conditions {
                if cond.condition_type == NamespaceConditionType::NamespaceIsEmpty {
                    let was_empty = cond.status == ConditionStatus___::True;
                    let is_empty = total == 0;

                    if was_empty != is_empty {
                        cond.status = if is_empty {
                            ConditionStatus___::True
                        } else {
                            ConditionStatus___::False
                        };
                        cond.last_transition_time = Utc::now();
                        cond.reason = if is_empty {
                            "ResourcesSatisfied".to_string()
                        } else {
                            "ResourcesRemaining".to_string()
                        };
                        cond.message = if is_empty {
                            "All resources have been removed".to_string()
                        } else {
                            format!("{} resources remaining", total)
                        };
                    }
                }
            }
        }

        Ok(())
    }

    /// Get deletion progress.
    pub async fn get_deletion_progress(&self, name: &str) -> Option<NamespaceDeletionProgress> {
        let namespaces = self.namespaces.read().await;

        if let Some(ns) = namespaces.get(name) {
            let all_removed = ns.resource_counts.values().all(|&c| c == 0);

            Some(NamespaceDeletionProgress {
                namespace: name.to_string(),
                active_resources: ns.resource_counts.clone(),
                terminating_resources: HashMap::new(),
                all_resources_removed: all_removed,
                time_remaining_seconds: None,
            })
        } else {
            None
        }
    }

    /// Finalize namespace deletion.
    pub async fn finalize_deletion(&self, name: &str) -> Result<bool> {
        let mut namespaces = self.namespaces.write().await;

        if let Some(ns) = namespaces.get_mut(name) {
            // Check if all finalizers are removed
            if !ns.finalizers.is_empty() {
                tracing::warn!(
                    namespace = %name,
                    finalizers = ?ns.finalizers,
                    "Cannot finalize namespace - finalizers remain"
                );
                return Ok(false);
            }

            // Check if all resources are removed
            let total: usize = ns.resource_counts.values().sum();
            if total > 0 {
                tracing::warn!(
                    namespace = %name,
                    resources = total,
                    "Cannot finalize namespace - resources remain"
                );
                return Ok(false);
            }

            // Remove namespace
            namespaces.remove(name);

            tracing::info!(namespace = %name, "Namespace finalized");
            return Ok(true);
        }

        Ok(false)
    }

    /// Get namespace state.
    pub async fn get_namespace(&self, name: &str) -> Option<NamespaceControllerState> {
        let namespaces = self.namespaces.read().await;
        namespaces.get(name).cloned()
    }

    /// List all namespaces.
    pub async fn list_namespaces(&self) -> Vec<NamespaceControllerState> {
        let namespaces = self.namespaces.read().await;
        namespaces.values().cloned().collect()
    }

    /// List terminating namespaces.
    pub async fn list_terminating(&self) -> Vec<String> {
        let namespaces = self.namespaces.read().await;
        namespaces
            .values()
            .filter(|ns| ns.phase == NamespacePhase::Terminating)
            .map(|ns| ns.name.clone())
            .collect()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for NamespaceController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_namespace_registration() {
        let controller = NamespaceController::new();
        controller.start().await.unwrap();

        controller.register_namespace("test-ns").await.unwrap();

        let ns = controller.get_namespace("test-ns").await;
        assert!(ns.is_some());
        assert_eq!(ns.unwrap().phase, NamespacePhase::Active);
    }

    #[tokio::test]
    async fn test_namespace_deletion_workflow() {
        let controller = NamespaceController::new();
        controller.start().await.unwrap();

        controller.register_namespace("test-ns").await.unwrap();

        // Start deletion
        controller.initiate_deletion("test-ns").await.unwrap();

        let ns = controller.get_namespace("test-ns").await.unwrap();
        assert_eq!(ns.phase, NamespacePhase::Terminating);
        assert!(ns.deletion_timestamp.is_some());

        // Remove finalizers
        controller
            .remove_finalizer("test-ns", "a3s.io/namespace-protection")
            .await
            .unwrap();

        // Update resource count to 0
        controller
            .update_resource_count("test-ns", "Pod", 0)
            .await
            .unwrap();

        // Finalize
        let finalized = controller.finalize_deletion("test-ns").await.unwrap();
        assert!(finalized);

        let ns = controller.get_namespace("test-ns").await;
        assert!(ns.is_none());
    }

    #[tokio::test]
    async fn test_cannot_delete_with_finalizers() {
        let controller = NamespaceController::new();
        controller.start().await.unwrap();

        controller.register_namespace("test-ns").await.unwrap();
        controller.initiate_deletion("test-ns").await.unwrap();

        let can_delete = controller.can_delete("test-ns").await;
        assert!(!can_delete);

        controller
            .remove_finalizer("test-ns", "a3s.io/namespace-protection")
            .await
            .unwrap();

        let can_delete = controller.can_delete("test-ns").await;
        assert!(can_delete);
    }

    #[tokio::test]
    async fn test_resource_count_tracking() {
        let controller = NamespaceController::new();
        controller.start().await.unwrap();

        controller.register_namespace("test-ns").await.unwrap();

        controller
            .update_resource_count("test-ns", "Pod", 5)
            .await
            .unwrap();
        controller
            .update_resource_count("test-ns", "Service", 2)
            .await
            .unwrap();

        let ns = controller.get_namespace("test-ns").await.unwrap();
        assert_eq!(ns.resource_counts.get("Pod"), Some(&5));
        assert_eq!(ns.resource_counts.get("Service"), Some(&2));
    }
}
