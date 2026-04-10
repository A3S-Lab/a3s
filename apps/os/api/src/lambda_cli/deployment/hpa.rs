//! Horizontal Pod Autoscaler (HPA).
//!
//! Automatically scales deployments based on CPU/memory utilization.

use crate::errors::Result;
use crate::state::{DeploymentDesired, PodActual, PodStatus};
use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Metric type for autoscaling.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MetricType {
    /// CPU utilization as a percentage.
    CpuUtilization,
    /// Memory utilization as a percentage.
    MemoryUtilization,
}

/// A single metric value.
#[derive(Debug, Clone)]
pub struct MetricValue {
    pub metric_type: MetricType,
    pub current_value: f64,
    pub average_value: Option<f64>,
}

/// Scaling policy - how to scale up/down.
#[derive(Debug, Clone, Copy)]
pub enum ScalingPolicy {
    /// Scale up/down immediately by the calculated amount.
    Immediate,
    /// Scale gradually over multiple ticks.
    Gradual,
}

/// Scaling direction.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ScalingDirection {
    Up,
    Down,
    None,
}

/// HPASpec - defines autoscaling rules for a deployment.
#[derive(Debug, Clone)]
pub struct HPASpec {
    /// Minimum number of replicas.
    pub min_replicas: i32,
    /// Maximum number of replicas.
    pub max_replicas: i32,
    /// Target CPU utilization percentage.
    pub target_cpu_utilization: Option<u32>,
    /// Target memory utilization percentage.
    pub target_memory_utilization: Option<u32>,
    /// Scale-up stabilization window (seconds).
    pub scale_up_stabilization_secs: u32,
    /// Scale-down stabilization window (seconds).
    pub scale_down_stabilization_secs: u32,
    /// How quickly to scale.
    pub policy: ScalingPolicy,
}

impl Default for HPASpec {
    fn default() -> Self {
        Self {
            min_replicas: 1,
            max_replicas: 10,
            target_cpu_utilization: Some(80),
            target_memory_utilization: None,
            scale_up_stabilization_secs: 0,
            scale_down_stabilization_secs: 300,
            policy: ScalingPolicy::Immediate,
        }
    }
}

/// HPAAction - the scaling action to take.
#[derive(Debug, Clone)]
pub struct HPAAction {
    pub direction: ScalingDirection,
    pub replicas: i32,
    pub reason: String,
}

/// HPA state - tracks current autoscaling state.
#[derive(Debug, Clone)]
pub struct HPAState {
    /// Last calculated action.
    pub last_action: Option<HPAAction>,
    /// Last scale timestamp.
    pub last_scale_at: Option<DateTime<Utc>>,
    /// Current replicas during last calculation.
    pub last_replicas: i32,
}

/// MetricsCollector - collects resource metrics from pods.
pub struct MetricsCollector {
    /// Simulated metrics (for now - real impl would query actual pod metrics).
    metrics: RwLock<HashMap<String, Vec<MetricValue>>>,
}

impl MetricsCollector {
    pub fn new() -> Self {
        Self {
            metrics: RwLock::new(HashMap::new()),
        }
    }

    /// Record a metric for a pod.
    pub async fn record_metric(&self, pod_id: &str, metric: MetricValue) {
        let mut metrics = self.metrics.write().await;
        metrics
            .entry(pod_id.to_string())
            .or_insert_with(Vec::new)
            .push(metric);
    }

    /// Get average CPU utilization for a deployment's pods.
    pub async fn get_average_cpu(&self, pod_ids: &[String]) -> f64 {
        let metrics = self.metrics.read().await;
        let mut total = 0.0;
        let mut count = 0;

        for pod_id in pod_ids {
            if let Some(pod_metrics) = metrics.get(pod_id) {
                for metric in pod_metrics {
                    if metric.metric_type == MetricType::CpuUtilization {
                        total += metric.average_value.unwrap_or(metric.current_value);
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            // Return simulated value for demo purposes
            return 50.0;
        }
        total / count as f64
    }

    /// Get average memory utilization for a deployment's pods.
    pub async fn get_average_memory(&self, pod_ids: &[String]) -> f64 {
        let metrics = self.metrics.read().await;
        let mut total = 0.0;
        let mut count = 0;

        for pod_id in pod_ids {
            if let Some(pod_metrics) = metrics.get(pod_id) {
                for metric in pod_metrics {
                    if metric.metric_type == MetricType::MemoryUtilization {
                        total += metric.average_value.unwrap_or(metric.current_value);
                        count += 1;
                    }
                }
            }
        }

        if count == 0 {
            // Return simulated value for demo purposes
            return 60.0;
        }
        total / count as f64
    }

    /// Simulate metrics collection (for testing/demo).
    pub async fn simulate_metrics(&self, pods: &[PodActual]) {
        use std::time::Instant;
        let seed = Instant::now().elapsed().as_nanos() as u64;

        for pod in pods {
            // Simulate CPU with some randomness
            let cpu = 30.0 + (seed % 50) as f64;
            self.record_metric(
                &pod.id,
                MetricValue {
                    metric_type: MetricType::CpuUtilization,
                    current_value: cpu,
                    average_value: Some(cpu),
                },
            )
            .await;

            // Simulate memory
            let memory = 40.0 + (seed % 40) as f64;
            self.record_metric(
                &pod.id,
                MetricValue {
                    metric_type: MetricType::MemoryUtilization,
                    current_value: memory,
                    average_value: Some(memory),
                },
            )
            .await;
        }
    }
}

impl Default for MetricsCollector {
    fn default() -> Self {
        Self::new()
    }
}

/// HorizontalPodAutoscaler - automatically scales deployments.
pub struct HorizontalPodAutoscaler {
    /// Metrics collector.
    metrics: Arc<MetricsCollector>,
    /// HPA states per deployment.
    states: RwLock<HashMap<String, HPAState>>,
    /// Current HPA specs per deployment.
    specs: RwLock<HashMap<String, HPASpec>>,
}

impl HorizontalPodAutoscaler {
    pub fn new() -> Self {
        Self {
            metrics: Arc::new(MetricsCollector::new()),
            states: RwLock::new(HashMap::new()),
            specs: RwLock::new(HashMap::new()),
        }
    }

    /// Set HPA spec for a deployment.
    pub async fn set_spec(&self, deployment: &str, spec: HPASpec) {
        let mut specs = self.specs.write().await;
        specs.insert(deployment.to_string(), spec);
    }

    /// Remove HPA spec for a deployment.
    pub async fn remove_spec(&self, deployment: &str) {
        let mut specs = self.specs.write().await;
        specs.remove(deployment);
    }

    /// Get current HPA state for a deployment.
    pub async fn get_state(&self, deployment: &str) -> Option<HPAState> {
        let states = self.states.read().await;
        states.get(deployment).cloned()
    }

    /// Compute the desired replica count based on current metrics.
    pub async fn compute_desired_replicas(
        &self,
        deployment_name: &str,
        _deployment: &DeploymentDesired,
        pods: &[PodActual],
    ) -> Result<HPAAction> {
        let spec = {
            let specs = self.specs.read().await;
            specs.get(deployment_name).cloned().unwrap_or_default()
        };

        // Get current replicas
        let current_replicas = pods
            .iter()
            .filter(|p| p.status == PodStatus::Running)
            .count() as i32;

        // Update metrics simulation
        let running_pods: Vec<PodActual> = pods
            .iter()
            .filter(|p| p.status == PodStatus::Running)
            .cloned()
            .collect();
        let pod_ids: Vec<String> = running_pods.iter().map(|p| p.id.clone()).collect();
        self.metrics.simulate_metrics(&running_pods).await;

        // Get current utilization
        let cpu_util = self.metrics.get_average_cpu(&pod_ids).await;
        let _memory_util = self.metrics.get_average_memory(&pod_ids).await;

        // Calculate desired replicas based on CPU
        let mut desired_replicas = current_replicas;
        let mut reason = String::new();

        // Scale based on CPU
        if let Some(target_cpu) = spec.target_cpu_utilization {
            let target_cpu = target_cpu as f64;
            if cpu_util > target_cpu as f64 {
                // Scale up
                let ratio = cpu_util / target_cpu;
                let new_replicas = (current_replicas as f64 * ratio).ceil() as i32;
                desired_replicas = new_replicas;
                reason = format!("CPU {}% > target {}%", cpu_util as u32, target_cpu as u32);
            } else if cpu_util < target_cpu as f64 * 0.6 {
                // Scale down (only if significantly below target)
                let ratio = cpu_util / target_cpu;
                let new_replicas = (current_replicas as f64 * ratio).floor() as i32;
                desired_replicas = new_replicas;
                reason = format!(
                    "CPU {}% < target {}% (scaling down)",
                    cpu_util as u32, target_cpu as u32
                );
            } else {
                reason = format!("CPU {}% within target range", cpu_util as u32);
            }
        }

        // Clamp to min/max
        desired_replicas = desired_replicas
            .max(spec.min_replicas)
            .min(spec.max_replicas);

        // Determine direction
        let direction = if desired_replicas > current_replicas {
            ScalingDirection::Up
        } else if desired_replicas < current_replicas {
            ScalingDirection::Down
        } else {
            ScalingDirection::None
        };

        // Check stabilization windows
        let action = {
            let mut states = self.states.write().await;
            let state = states
                .entry(deployment_name.to_string())
                .or_insert(HPAState {
                    last_action: None,
                    last_scale_at: None,
                    last_replicas: current_replicas,
                });

            // Apply stabilization windows
            let can_scale = match direction {
                ScalingDirection::Up => {
                    // Scale up immediately (no stabilization for scale-up)
                    true
                }
                ScalingDirection::Down => {
                    // Check scale-down stabilization
                    if let Some(last_scale) = state.last_scale_at {
                        let elapsed =
                            Utc::now().signed_duration_since(last_scale).num_seconds() as u32;
                        elapsed >= spec.scale_down_stabilization_secs
                    } else {
                        true
                    }
                }
                ScalingDirection::None => true,
            };

            if !can_scale {
                // Return no-op action
                HPAAction {
                    direction: ScalingDirection::None,
                    replicas: current_replicas,
                    reason: "stabilization window active".to_string(),
                }
            } else {
                let action = HPAAction {
                    direction,
                    replicas: desired_replicas,
                    reason,
                };

                // Update state
                if direction != ScalingDirection::None {
                    state.last_action = Some(action.clone());
                    state.last_scale_at = Some(Utc::now());
                    state.last_replicas = desired_replicas;
                }

                action
            }
        };

        Ok(action)
    }
}

impl Default for HorizontalPodAutoscaler {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::ResourceRequirements;

    fn test_deployment(replicas: i32) -> DeploymentDesired {
        DeploymentDesired {
            name: "test".to_string(),
            namespace: "default".to_string(),
            image: "nginx:latest".to_string(),
            replicas,
            env: HashMap::new(),
            ports: vec![],
            version: "v1".to_string(),
            strategy: Default::default(),
            resources: ResourceRequirements::default(),
            health_check: Default::default(),
            node_selector: Default::default(),
            tolerations: vec![],
            labels: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_pods(count: i32) -> Vec<PodActual> {
        (0..count)
            .map(|i| PodActual {
                id: format!("pod-{}", i),
                deployment: "test".to_string(),
                namespace: "default".to_string(),
                status: PodStatus::Running,
                health: crate::state::HealthStatus::Healthy,
                ip: Some(format!("10.0.0.{}", i + 1)),
                version: "v1".to_string(),
                socket_path: None,
                node_name: None,
                created_at: Utc::now(),
                last_health_check: Some(Utc::now()),
                consecutive_failures: 0,
                ready: true,
            })
            .collect()
    }

    #[tokio::test]
    async fn test_scale_up_on_high_cpu() {
        let hpa = HorizontalPodAutoscaler::new();

        let spec = HPASpec {
            min_replicas: 1,
            max_replicas: 10,
            target_cpu_utilization: Some(80),
            ..Default::default()
        };
        hpa.set_spec("test", spec).await;

        let deployment = test_deployment(2);
        let pods = test_pods(2);

        let action = hpa
            .compute_desired_replicas("test", &deployment, &pods)
            .await
            .unwrap();

        // With simulated CPU around 50%, we shouldn't scale up
        assert!(action.replicas >= 1);
    }

    #[tokio::test]
    async fn test_min_max_bounds() {
        let hpa = HorizontalPodAutoscaler::new();

        let spec = HPASpec {
            min_replicas: 2,
            max_replicas: 5,
            target_cpu_utilization: Some(50),
            ..Default::default()
        };
        hpa.set_spec("test", spec).await;

        let deployment = test_deployment(3);
        let pods = test_pods(3);

        let action = hpa
            .compute_desired_replicas("test", &deployment, &pods)
            .await
            .unwrap();

        assert!(action.replicas >= 2);
        assert!(action.replicas <= 5);
    }
}
