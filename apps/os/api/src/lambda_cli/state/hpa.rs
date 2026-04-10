//! HPA - Horizontal Pod Autoscaler.
//!
//! HPA automatically scales workloads based on resource utilization.
//! It replaces Kubernetes HPA with an A3s-native implementation.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Scaling policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ScalingPolicy {
    /// Scale up and down.
    Scales,
    /// Scale up only.
    ScaleUp,
    /// Scale down only.
    ScaleDown,
}

/// HPAMetricSpec defines the metrics to scale on.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HPAMetricSpec {
    /// Metric type.
    pub metric_type: MetricType,
    /// Resource metric.
    pub resource: Option<ResourceMetricSpec>,
    /// Pods metric.
    pub pods: Option<PodsMetricSpec>,
    /// External metric.
    pub external: Option<ExternalMetricSpec>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MetricType {
    /// Resource metric.
    Resource,
    /// Pods metric.
    Pods,
    /// External metric.
    External,
    /// Object metric.
    Object,
}

/// Resource metric specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetricSpec {
    /// Resource name.
    pub name: ResourceName,
    /// Target average utilization.
    pub target_average_utilization: Option<i32>,
    /// Target average value.
    pub target_average_value: Option<i64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ResourceName {
    Cpu,
    Memory,
}

/// Pods metric specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodsMetricSpec {
    /// Metric name.
    pub metric_name: String,
    /// Target average value.
    pub target_average_value: i64,
}

/// External metric specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExternalMetricSpec {
    /// Metric name.
    pub metric_name: String,
    /// Target value.
    pub target_value: Option<i64>,
    /// Target average value.
    pub target_average_value: Option<i64>,
    /// Selector.
    pub selector: HashMap<String, String>,
}

/// HPABehavior defines the scaling behavior.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HPABehavior {
    /// Scale up policy.
    pub scale_up: Option<ScalingRules>,
    /// Scale down policy.
    pub scale_down: Option<ScalingRules>,
}

/// Scaling rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingRules {
    /// Stabilization window seconds.
    pub stabilization_window_seconds: i32,
    /// Policies.
    pub policies: Vec<ScalingPolicyRule>,
}

/// Scaling policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScalingPolicyRule {
    /// Policy type.
    pub policy_type: ScalingPolicy,
    /// Period seconds.
    pub period_seconds: i32,
    /// Value.
    pub value: i32,
}

/// HPASpec defines the desired behavior of the autoscaler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HPASpec {
    /// Scale target reference.
    pub scale_target_ref: ScaleTargetRef,
    /// Minimum replicas.
    pub min_replicas: Option<i32>,
    /// Maximum replicas.
    pub max_replicas: i32,
    /// Metrics.
    pub metrics: Vec<HPAMetricSpec>,
    /// Behavior.
    pub behavior: Option<HPABehavior>,
}

/// Scale target reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScaleTargetRef {
    /// API version.
    pub api_version: Option<String>,
    /// Kind (Deployment, StatefulSet, etc).
    pub kind: String,
    /// Name.
    pub name: String,
}

/// HPAStatus defines the observed state of the autoscaler.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HPAStatus {
    /// Observed generation.
    pub observed_generation: i64,
    /// Current replicas.
    pub current_replicas: i32,
    /// Desired replicas.
    pub desired_replicas: i32,
    /// Last scale time.
    pub last_scale_time: Option<DateTime<Utc>>,
    /// Current metrics.
    pub current_metrics: Vec<MetricStatus>,
    /// Conditions.
    pub conditions: Vec<HPACondition>,
}

/// Metric status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricStatus {
    /// Metric type.
    pub metric_type: MetricType,
    /// Resource status.
    pub resource: Option<ResourceMetricStatus>,
    /// Pods status.
    pub pods: Option<PodsMetricStatus>,
}

/// Resource metric status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceMetricStatus {
    /// Name.
    pub name: ResourceName,
    /// Current average utilization.
    pub current_average_utilization: Option<i32>,
    /// Current average value.
    pub current_average_value: Option<i64>,
}

/// Pods metric status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodsMetricStatus {
    /// Metric name.
    pub metric_name: String,
    /// Current average value.
    pub current_average_value: i64,
}

/// HPA condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HPACondition {
    /// Condition type.
    pub condition_type: HPAConditionType,
    /// Status.
    pub status: ConditionStatus_,
    /// Last transition time.
    pub last_transition_time: DateTime<Utc>,
    /// Last update time.
    pub last_update_time: DateTime<Utc>,
    /// Reason.
    pub reason: String,
    /// Message.
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HPAConditionType {
    /// Able to scale.
    Scalable,
    /// Scaling limited.
    ScalingLimited,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConditionStatus_ {
    True,
    False,
    Unknown,
}

/// HorizontalPodAutoscaler desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HPADesired {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Spec.
    pub spec: HPASpec,
    /// Status.
    pub status: Option<HPAStatus>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Metric value.
#[derive(Debug, Clone)]
pub struct MetricValue {
    /// Current value.
    pub current: f64,
    /// Average value across pods.
    pub average: Option<f64>,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
}

/// Metric result.
#[derive(Debug, Clone)]
pub struct MetricResult {
    /// Metric name.
    pub metric_name: String,
    /// Current value.
    pub current: MetricValue,
    /// Desired value.
    pub desired: MetricValue,
    /// Ready pods.
    pub ready_pods: i32,
}

/// Scale recommendation.
#[derive(Debug, Clone)]
pub struct ScaleRecommendation {
    /// Recommended replicas.
    pub replicas: i32,
    /// Reason.
    pub reason: String,
    /// Metric results.
    pub metric_results: Vec<MetricResult>,
}

/// HPA controller.
pub struct HPAController {
    /// HPAs by namespace/name.
    hpas: RwLock<HashMap<String, HPADesired>>,
    /// Current replicas per target.
    current_replicas: RwLock<HashMap<String, i32>>,
    /// Replica history for stabilization.
    replica_history: RwLock<HashMap<String, Vec<(DateTime<Utc>, i32)>>>,
}

impl HPAController {
    /// Create a new HPA controller.
    pub fn new() -> Self {
        Self {
            hpas: RwLock::new(HashMap::new()),
            current_replicas: RwLock::new(HashMap::new()),
            replica_history: RwLock::new(HashMap::new()),
        }
    }

    /// Set HPA.
    pub async fn set_hpa(&self, hpa: HPADesired) -> Result<()> {
        let key = format!("{}/{}", hpa.namespace, hpa.name);
        let mut hpas = self.hpas.write().await;
        hpas.insert(key, hpa);
        Ok(())
    }

    /// Get HPA.
    pub async fn get_hpa(&self, namespace: &str, name: &str) -> Option<HPADesired> {
        let key = format!("{}/{}", namespace, name);
        let hpas = self.hpas.read().await;
        hpas.get(&key).cloned()
    }

    /// Delete HPA.
    pub async fn delete_hpa(&self, namespace: &str, name: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, name);
        let mut hpas = self.hpas.write().await;
        hpas.remove(&key);

        let mut current = self.current_replicas.write().await;
        current.remove(&key);

        let mut history = self.replica_history.write().await;
        history.remove(&key);

        Ok(())
    }

    /// Calculate desired replicas based on metrics.
    pub async fn calculate_replicas(
        &self,
        namespace: &str,
        name: &str,
        current_metrics: Vec<MetricStatus>,
    ) -> Result<ScaleRecommendation> {
        let hpa = self
            .get_hpa(namespace, name)
            .await
            .ok_or_else(|| A3sError::Other("HPA not found".to_string()))?;

        let spec = &hpa.spec;
        let min_replicas = spec.min_replicas.unwrap_or(1);
        let max_replicas = spec.max_replicas;

        // Calculate desired replicas based on metrics
        let mut desired_replicas = hpa
            .status
            .as_ref()
            .map(|s| s.current_replicas)
            .unwrap_or(min_replicas);
        let mut metric_results = Vec::new();
        let mut reasons = Vec::new();

        for metric in &spec.metrics {
            match metric.metric_type {
                MetricType::Resource => {
                    if let Some(ref resource) = metric.resource {
                        let status = current_metrics
                            .iter()
                            .find(|m| matches!(m.metric_type, MetricType::Resource));

                        if let Some(MetricStatus {
                            resource:
                                Some(ResourceMetricStatus {
                                    current_average_utilization,
                                    ..
                                }),
                            ..
                        }) = status
                        {
                            let current_util = current_average_utilization.unwrap_or(0) as f64;
                            let target_util =
                                resource.target_average_utilization.unwrap_or(0) as f64;

                            if target_util > 0.0 {
                                let ratio = current_util / target_util;
                                let suggested = (desired_replicas as f64 * ratio).ceil() as i32;
                                desired_replicas = desired_replicas.max(suggested);

                                reasons.push(format!(
                                    "current utilization {} / target {} = ratio {}",
                                    current_util, target_util, ratio
                                ));

                                metric_results.push(MetricResult {
                                    metric_name: format!("{:?}", resource.name),
                                    current: MetricValue {
                                        current: current_util,
                                        average: Some(current_util),
                                        timestamp: Utc::now(),
                                    },
                                    desired: MetricValue {
                                        current: target_util,
                                        average: Some(target_util),
                                        timestamp: Utc::now(),
                                    },
                                    ready_pods: desired_replicas,
                                });
                            }
                        }
                    }
                }
                MetricType::Pods => {
                    if let Some(ref pods) = metric.pods {
                        reasons.push(format!("pods metric: {}", pods.metric_name));
                    }
                }
                MetricType::External => {
                    if let Some(ref external) = metric.external {
                        reasons.push(format!("external metric: {}", external.metric_name));
                    }
                }
                MetricType::Object => {
                    reasons.push("object metric".to_string());
                }
            }
        }

        // Apply bounds
        let desired = desired_replicas.clamp(min_replicas, max_replicas);

        // Apply stabilization window for scale down
        let final_replicas = if desired < desired_replicas {
            self.apply_stabilization(namespace, name, desired).await
        } else {
            desired
        };

        // Record history
        self.record_replica(namespace, name, final_replicas).await;

        // Update status
        self.update_status(namespace, name, final_replicas, current_metrics)
            .await?;

        Ok(ScaleRecommendation {
            replicas: final_replicas,
            reason: reasons.join(", "),
            metric_results,
        })
    }

    /// Apply stabilization window for scale down.
    async fn apply_stabilization(&self, namespace: &str, name: &str, desired: i32) -> i32 {
        let key = format!("{}/{}", namespace, name);
        let history = self.replica_history.read().await;

        if let Some(entries) = history.get(&key) {
            let now = Utc::now();
            let window_secs = 300; // 5 minutes default

            // Find max replicas in window
            let max_in_window = entries
                .iter()
                .filter(|(time, _)| (now - *time).num_seconds() < window_secs)
                .map(|(_, replicas)| *replicas)
                .max()
                .unwrap_or(desired);

            return max_in_window.max(desired);
        }

        desired
    }

    /// Record replica count.
    async fn record_replica(&self, namespace: &str, name: &str, replicas: i32) {
        let key = format!("{}/{}", namespace, name);
        let mut history = self.replica_history.write().await;

        let entries = history.entry(key).or_insert_with(Vec::new);
        entries.push((Utc::now(), replicas));

        // Keep only last hour
        let cutoff = Utc::now() - chrono::Duration::hours(1);
        entries.retain(|(time, _)| *time > cutoff);
    }

    /// Update HPA status.
    async fn update_status(
        &self,
        namespace: &str,
        name: &str,
        replicas: i32,
        current_metrics: Vec<MetricStatus>,
    ) -> Result<()> {
        let key = format!("{}/{}", namespace, name);

        let mut hpas = self.hpas.write().await;
        if let Some(hpa) = hpas.get_mut(&key) {
            hpa.status = Some(HPAStatus {
                observed_generation: 1,
                current_replicas: replicas,
                desired_replicas: replicas,
                last_scale_time: Some(Utc::now()),
                current_metrics,
                conditions: vec![HPACondition {
                    condition_type: HPAConditionType::Scalable,
                    status: ConditionStatus_::True,
                    last_transition_time: Utc::now(),
                    last_update_time: Utc::now(),
                    reason: "ScalingActive".to_string(),
                    message: "Ready to scale".to_string(),
                }],
            });
        }

        Ok(())
    }

    /// List all HPAs.
    pub async fn list_hpas(&self) -> Vec<HPADesired> {
        let hpas = self.hpas.read().await;
        hpas.values().cloned().collect()
    }

    /// Get current replicas for a target.
    pub async fn get_current_replicas(&self, namespace: &str, name: &str) -> Option<i32> {
        let key = format!("{}/{}", namespace, name);
        let current = self.current_replicas.read().await;
        current.get(&key).copied()
    }
}

impl Default for HPAController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_hpa_creation() {
        let controller = HPAController::new();

        let hpa = HPADesired {
            name: "my-hpa".to_string(),
            namespace: "default".to_string(),
            spec: HPASpec {
                scale_target_ref: ScaleTargetRef {
                    api_version: Some("apps/v1".to_string()),
                    kind: "Deployment".to_string(),
                    name: "my-app".to_string(),
                },
                min_replicas: Some(2),
                max_replicas: 10,
                metrics: vec![HPAMetricSpec {
                    metric_type: MetricType::Resource,
                    resource: Some(ResourceMetricSpec {
                        name: ResourceName::Cpu,
                        target_average_utilization: Some(50),
                        target_average_value: None,
                    }),
                    pods: None,
                    external: None,
                }],
                behavior: None,
            },
            status: None,
            created_at: Utc::now(),
        };

        controller.set_hpa(hpa).await.unwrap();

        let found = controller.get_hpa("default", "my-hpa").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().spec.max_replicas, 10);
    }

    #[tokio::test]
    async fn test_scale_calculation() {
        let controller = HPAController::new();

        let hpa = HPADesired {
            name: "my-hpa".to_string(),
            namespace: "default".to_string(),
            spec: HPASpec {
                scale_target_ref: ScaleTargetRef {
                    api_version: Some("apps/v1".to_string()),
                    kind: "Deployment".to_string(),
                    name: "my-app".to_string(),
                },
                min_replicas: Some(2),
                max_replicas: 10,
                metrics: vec![HPAMetricSpec {
                    metric_type: MetricType::Resource,
                    resource: Some(ResourceMetricSpec {
                        name: ResourceName::Cpu,
                        target_average_utilization: Some(50),
                        target_average_value: None,
                    }),
                    pods: None,
                    external: None,
                }],
                behavior: None,
            },
            status: Some(HPAStatus {
                observed_generation: 1,
                current_replicas: 2,
                desired_replicas: 2,
                last_scale_time: None,
                current_metrics: vec![],
                conditions: vec![],
            }),
            created_at: Utc::now(),
        };

        controller.set_hpa(hpa).await.unwrap();

        // Simulate 100% CPU utilization (should double replicas)
        let metrics = vec![MetricStatus {
            metric_type: MetricType::Resource,
            resource: Some(ResourceMetricStatus {
                name: ResourceName::Cpu,
                current_average_utilization: Some(100),
                current_average_value: None,
            }),
            pods: None,
        }];

        let recommendation = controller
            .calculate_replicas("default", "my-hpa", metrics)
            .await
            .unwrap();

        assert_eq!(recommendation.replicas, 4); // 100/50 = 2x, 2 * 2 = 4
    }

    #[tokio::test]
    async fn test_scale_bounds() {
        let controller = HPAController::new();

        let hpa = HPADesired {
            name: "my-hpa".to_string(),
            namespace: "default".to_string(),
            spec: HPASpec {
                scale_target_ref: ScaleTargetRef {
                    api_version: Some("apps/v1".to_string()),
                    kind: "Deployment".to_string(),
                    name: "my-app".to_string(),
                },
                min_replicas: Some(2),
                max_replicas: 5,
                metrics: vec![],
                behavior: None,
            },
            status: Some(HPAStatus {
                observed_generation: 1,
                current_replicas: 4,
                desired_replicas: 4,
                last_scale_time: None,
                current_metrics: vec![],
                conditions: vec![],
            }),
            created_at: Utc::now(),
        };

        controller.set_hpa(hpa).await.unwrap();

        // When no metrics are available, HPA should maintain current replicas
        let recommendation = controller
            .calculate_replicas("default", "my-hpa", vec![])
            .await
            .unwrap();

        assert_eq!(recommendation.replicas, 4); // Stays at current when no metrics
    }
}
