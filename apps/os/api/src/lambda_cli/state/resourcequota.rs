//! ResourceQuota Controller - Resource Quota Enforcement.
//!
//! ResourceQuotaController enforces resource quotas per namespace:
//! - Tracking resource usage against quota
//! - Rejecting resource creation when quota exceeded
//! - Computing resource usage across pods, services, etc.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Resource quota scope.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotaScope {
    /// Scope name.
    pub scope_name: String,
    /// Match expressions.
    pub match_expressions: Vec<String>,
}

/// Scope selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScopeSelector {
    /// Operator.
    pub operator: ScopeSelectorOperator,
    /// Scope name.
    pub scope_name: String,
    /// Values.
    pub values: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ScopeSelectorOperator {
    In,
    NotIn,
    Exists,
    DoesNotExist,
}

/// Quota scope type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum QuotaScopeType {
    /// Best effort pods.
    BestEffort,
    /// Not best effort pods.
    NotBestEffort,
    /// Terminating pods.
    Terminating,
    /// Not terminating pods.
    NotTerminating,
    /// QoS class.
    PriorityClass,
}

/// Resource quota spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotaSpec {
    /// Hard limits.
    pub hard: HashMap<String, i64>,
    /// Scope selector.
    pub scope_selector: Option<ScopeSelector>,
    /// Scopes.
    pub scopes: Vec<QuotaScopeType>,
}

/// Resource quota status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotaStatus {
    /// Hard limits.
    pub hard: HashMap<String, i64>,
    /// Used values.
    pub used: HashMap<String, i64>,
    /// Last updated.
    pub last_updated: DateTime<Utc>,
}

/// Resource quota desired.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuotaDesired {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Spec.
    pub spec: ResourceQuotaSpec,
    /// Status.
    pub status: Option<ResourceQuotaStatus>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Resource usage.
#[derive(Debug, Clone, Default)]
pub struct ResourceUsage {
    /// CPU cores (millicores).
    pub cpu: i64,
    /// Memory bytes.
    pub memory: i64,
    /// Pods count.
    pub pods: i64,
    /// Services count.
    pub services: i64,
    /// Secrets count.
    pub secrets: i64,
    /// ConfigMaps count.
    pub configmaps: i64,
    /// PersistentVolumeClaims count.
    pub persistentvolumeclaims: i64,
    /// GPU count.
    pub gpu: i64,
}

/// Quota admission decision.
#[derive(Debug, Clone)]
pub enum QuotaAdmissionDecision {
    /// Allowed.
    Allowed,
    /// Denied with reason.
    Denied { reason: String, message: String },
}

/// ResourceQuotaController enforces resource quotas.
pub struct ResourceQuotaController {
    /// Quotas by namespace/name.
    quotas: RwLock<HashMap<String, ResourceQuotaDesired>>,
    /// Usage by namespace.
    usage: RwLock<HashMap<String, ResourceUsage>>,
    /// Running state.
    running: RwLock<bool>,
}

impl ResourceQuotaController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            quotas: RwLock::new(HashMap::new()),
            usage: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        tracing::info!("ResourceQuotaController started");
        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("ResourceQuotaController stopped");
        Ok(())
    }

    /// Set quota.
    pub async fn set_quota(&self, quota: ResourceQuotaDesired) -> Result<()> {
        let key = format!("{}/{}", quota.namespace, quota.name);
        let mut quotas = self.quotas.write().await;
        quotas.insert(key, quota);
        Ok(())
    }

    /// Get quota.
    pub async fn get_quota(&self, namespace: &str, name: &str) -> Option<ResourceQuotaDesired> {
        let key = format!("{}/{}", namespace, name);
        let quotas = self.quotas.read().await;
        quotas.get(&key).cloned()
    }

    /// Delete quota.
    pub async fn delete_quota(&self, namespace: &str, name: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, name);
        let mut quotas = self.quotas.write().await;
        quotas.remove(&key);
        Ok(())
    }

    /// Update usage for a namespace.
    pub async fn update_usage(&self, namespace: &str, usage: ResourceUsage) -> Result<()> {
        let mut usage_map = self.usage.write().await;
        usage_map.insert(namespace.to_string(), usage);
        Ok(())
    }

    /// Get usage for a namespace.
    pub async fn get_usage(&self, namespace: &str) -> Option<ResourceUsage> {
        let usage_map = self.usage.read().await;
        usage_map.get(namespace).cloned()
    }

    /// Check if resource creation is allowed.
    pub async fn check_admission(
        &self,
        namespace: &str,
        resource: &str,
        delta: i64,
    ) -> QuotaAdmissionDecision {
        let _key = format!("{}/", namespace);
        let quotas = self.quotas.read().await;
        let usage_map = self.usage.read().await;

        // Find quota for namespace
        let quota = quotas.values().find(|q| q.namespace == namespace);

        match quota {
            Some(q) => {
                // Check if resource is tracked
                if let Some(limit) = q.spec.hard.get(resource) {
                    let current = usage_map
                        .get(namespace)
                        .and_then(|u| Self::get_resource_count(u, resource))
                        .unwrap_or(0);

                    if current + delta > *limit {
                        return QuotaAdmissionDecision::Denied {
                            reason: "QuotaExceeded".to_string(),
                            message: format!(
                                "exceeded quota {} in namespace {}: used {} + {} > limit {}",
                                resource, namespace, current, delta, limit
                            ),
                        };
                    }
                }

                QuotaAdmissionDecision::Allowed
            }
            None => QuotaAdmissionDecision::Allowed,
        }
    }

    /// Get resource count from usage struct.
    fn get_resource_count(usage: &ResourceUsage, resource: &str) -> Option<i64> {
        match resource {
            "cpu" | "limits.cpu" | "requests.cpu" => Some(usage.cpu),
            "memory" | "limits.memory" | "requests.memory" => Some(usage.memory),
            "pods" => Some(usage.pods),
            "services" => Some(usage.services),
            "secrets" => Some(usage.secrets),
            "configmaps" => Some(usage.configmaps),
            "persistentvolumeclaims" => Some(usage.persistentvolumeclaims),
            "nvidia.com/gpu" | "gpu" => Some(usage.gpu),
            _ => None,
        }
    }

    /// Recompute status for a quota.
    pub async fn recompute_status(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<ResourceQuotaStatus> {
        let key = format!("{}/{}", namespace, name);
        let quotas = self.quotas.read().await;
        let usage_map = self.usage.read().await;

        let quota = quotas
            .get(&key)
            .ok_or_else(|| A3sError::Other("Quota not found".to_string()))?;

        let usage = usage_map.get(namespace).cloned().unwrap_or_default();

        let mut status = ResourceQuotaStatus {
            hard: quota.spec.hard.clone(),
            used: HashMap::new(),
            last_updated: Utc::now(),
        };

        for (resource, limit) in &quota.spec.hard {
            let used = Self::get_resource_count(&usage, resource).unwrap_or(0);
            status.used.insert(resource.clone(), used);

            // Check if over quota
            if used > *limit {
                tracing::warn!(
                    namespace = %namespace,
                    quota = %name,
                    resource = %resource,
                    used = %used,
                    limit = %limit,
                    "Quota exceeded"
                );
            }
        }

        // Update quota status
        drop(quotas);
        drop(usage_map);

        let mut quotas = self.quotas.write().await;
        if let Some(q) = quotas.get_mut(&key) {
            q.status = Some(status.clone());
        }

        Ok(status)
    }

    /// List quotas for namespace.
    pub async fn list_namespace_quotas(&self, namespace: &str) -> Vec<ResourceQuotaDesired> {
        let quotas = self.quotas.read().await;
        quotas
            .values()
            .filter(|q| q.namespace == namespace)
            .cloned()
            .collect()
    }

    /// List all quotas.
    pub async fn list_quotas(&self) -> Vec<ResourceQuotaDesired> {
        let quotas = self.quotas.read().await;
        quotas.values().cloned().collect()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for ResourceQuotaController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_quota_creation() {
        let controller = ResourceQuotaController::new();
        controller.start().await.unwrap();

        let mut hard = HashMap::new();
        hard.insert("pods".to_string(), 10);
        hard.insert("cpu".to_string(), 2000);
        hard.insert("memory".to_string(), 1_073_741_824); // 1Gi

        let quota = ResourceQuotaDesired {
            name: "test-quota".to_string(),
            namespace: "default".to_string(),
            spec: ResourceQuotaSpec {
                hard,
                scope_selector: None,
                scopes: vec![],
            },
            status: None,
            created_at: Utc::now(),
        };

        controller.set_quota(quota).await.unwrap();

        let found = controller.get_quota("default", "test-quota").await;
        assert!(found.is_some());
    }

    #[tokio::test]
    async fn test_quota_admission() {
        let controller = ResourceQuotaController::new();
        controller.start().await.unwrap();

        // Set quota
        let mut hard = HashMap::new();
        hard.insert("pods".to_string(), 10);

        controller
            .set_quota(ResourceQuotaDesired {
                name: "test-quota".to_string(),
                namespace: "default".to_string(),
                spec: ResourceQuotaSpec {
                    hard,
                    scope_selector: None,
                    scopes: vec![],
                },
                status: None,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        // Check admission - should be allowed
        let decision = controller.check_admission("default", "pods", 5).await;
        assert!(matches!(decision, QuotaAdmissionDecision::Allowed));

        // Set usage to near limit
        controller
            .update_usage(
                "default",
                ResourceUsage {
                    pods: 8,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        // Check admission - should be allowed (8 + 2 = 10)
        let decision = controller.check_admission("default", "pods", 2).await;
        assert!(matches!(decision, QuotaAdmissionDecision::Allowed));

        // Check admission - should be denied (8 + 5 = 13 > 10)
        let decision = controller.check_admission("default", "pods", 5).await;
        assert!(matches!(decision, QuotaAdmissionDecision::Denied { .. }));
    }

    #[tokio::test]
    async fn test_status_computation() {
        let controller = ResourceQuotaController::new();
        controller.start().await.unwrap();

        let mut hard = HashMap::new();
        hard.insert("pods".to_string(), 10);
        hard.insert("cpu".to_string(), 2000);

        controller
            .set_quota(ResourceQuotaDesired {
                name: "test-quota".to_string(),
                namespace: "default".to_string(),
                spec: ResourceQuotaSpec {
                    hard,
                    scope_selector: None,
                    scopes: vec![],
                },
                status: None,
                created_at: Utc::now(),
            })
            .await
            .unwrap();

        controller
            .update_usage(
                "default",
                ResourceUsage {
                    pods: 5,
                    cpu: 1000,
                    ..Default::default()
                },
            )
            .await
            .unwrap();

        let status = controller
            .recompute_status("default", "test-quota")
            .await
            .unwrap();
        assert_eq!(status.used.get("pods"), Some(&5));
        assert_eq!(status.used.get("cpu"), Some(&1000));
    }
}
