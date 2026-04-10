//! Daemon Workload - Run one pod per node.
//!
//! DaemonSets ensure that all (or some) nodes run a copy of a specific pod.
//! Typically used for logging, monitoring, and other node-level services.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// DaemonSet strategy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub enum DaemonSetStrategy {
    /// Rolling update - replace pods one by one.
    #[default]
    RollingUpdate,
    /// OnDelete - only replace pods that have been manually deleted.
    OnDelete,
}

/// DaemonSet selector.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonSetSelector {
    /// Match labels.
    #[serde(default)]
    pub match_labels: HashMap<String, String>,
    /// Match expressions.
    #[serde(default)]
    pub match_expressions: Vec<DaemonSetSelectorExpression>,
}

/// DaemonSet selector expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetSelectorExpression {
    pub key: String,
    pub operator: String,
    #[serde(default)]
    pub values: Vec<String>,
}

/// DaemonSet specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetSpec {
    /// Pod template selector.
    pub selector: DaemonSetSelector,
    /// Pod template.
    pub template: DaemonSetPodTemplate,
    /// Update strategy.
    #[serde(default)]
    pub strategy: DaemonSetStrategy,
    /// Minimum number of seconds a pod must be ready.
    #[serde(default = "default_min_ready_secs")]
    pub min_ready_secs: u32,
    /// Number of pods to maintain.
    #[serde(default)]
    pub replicas: i32,
}

fn default_min_ready_secs() -> u32 {
    0
}

/// Pod template for DaemonSet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetPodTemplate {
    /// Metadata.
    pub metadata: DaemonSetPodMetadata,
    /// Pod spec.
    pub spec: DaemonSetPodSpec,
}

/// Pod metadata for DaemonSet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonSetPodMetadata {
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// Pod spec for DaemonSet (simplified from Deployment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetPodSpec {
    /// Container image.
    pub image: String,
    /// Command.
    #[serde(default)]
    pub command: Vec<String>,
    /// Arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Port mappings.
    #[serde(default)]
    pub ports: Vec<DaemonPortMapping>,
    /// Resource requirements.
    #[serde(default)]
    pub resources: DaemonResourceRequirements,
    /// Volume mounts.
    #[serde(default)]
    pub volume_mounts: Vec<DaemonVolumeMount>,
    /// Health check.
    #[serde(default)]
    pub health_check: DaemonHealthCheck,
}

/// Port mapping for DaemonSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonPortMapping {
    pub name: String,
    pub container_port: u16,
    pub host_port: u16,
    pub protocol: String,
}

/// Resource requirements for DaemonSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonResourceRequirements {
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<String>,
    pub memory_request: Option<String>,
    pub cpu_request: Option<String>,
}

/// Volume mount for DaemonSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonVolumeMount {
    pub name: String,
    pub mount_path: String,
    #[serde(default)]
    pub read_only: bool,
}

/// Health check for DaemonSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DaemonHealthCheck {
    pub enabled: bool,
    pub path: Option<String>,
    pub port: Option<u16>,
    pub initial_delay_secs: u32,
    pub period_secs: u32,
    pub timeout_secs: u32,
    pub failure_threshold: u32,
}

/// DaemonSet desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetDesired {
    /// DaemonSet name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// DaemonSet specification.
    pub spec: DaemonSetSpec,
    /// Current version.
    pub version: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// DaemonSet actual state (per node).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetActual {
    /// DaemonSet name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Number of nodes that should have the pod.
    pub desired_nodes: i32,
    /// Number of nodes that have the pod.
    pub current_nodes: i32,
    /// Number of nodes that are ready.
    pub ready_nodes: i32,
    /// Number of nodes that are updated.
    pub updated_nodes: i32,
    /// Node-to-pod mapping.
    #[serde(default)]
    pub node_pods: HashMap<String, DaemonPodStatus>,
}

/// Daemon pod status on a specific node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonPodStatus {
    /// Node name.
    pub node_name: String,
    /// Pod ID.
    pub pod_id: Option<String>,
    /// Pod status.
    pub status: DaemonPodState,
    /// Last update timestamp.
    pub last_updated: DateTime<Utc>,
}

/// Daemon pod state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DaemonPodState {
    /// Pod is being created.
    Pending,
    /// Pod is running.
    Running,
    /// Pod failed to start.
    Failed,
    /// Pod is being deleted.
    Terminating,
}

/// Node registration for DaemonSet scheduling.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonNode {
    /// Node name/ID.
    pub name: String,
    /// Node IP.
    pub ip: String,
    /// Labels for selector matching.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Taints.
    #[serde(default)]
    pub taints: Vec<String>,
    /// Whether the node is schedulable.
    pub schedulable: bool,
    /// Last heartbeat.
    pub last_heartbeat: DateTime<Utc>,
    /// DaemonSet pods running on this node.
    #[serde(default)]
    pub daemon_pods: Vec<String>,
}

/// Daemon controller - manages DaemonSet lifecycle.
pub struct DaemonController {
    /// DaemonSets.
    daemonsets: RwLock<HashMap<String, HashMap<String, DaemonSetDesired>>>,
    /// DaemonSet actual states.
    actuals: RwLock<HashMap<String, HashMap<String, DaemonSetActual>>>,
    /// Registered nodes.
    nodes: RwLock<HashMap<String, DaemonNode>>,
}

impl DaemonController {
    /// Create a new Daemon controller.
    pub fn new() -> Self {
        Self {
            daemonsets: RwLock::new(HashMap::new()),
            actuals: RwLock::new(HashMap::new()),
            nodes: RwLock::new(HashMap::new()),
        }
    }

    // ==================== DaemonSet Operations ====================

    /// Create a DaemonSet.
    pub async fn create_daemonset(&self, ds: DaemonSetDesired) -> Result<()> {
        let mut daemonsets = self.daemonsets.write().await;
        let ns_daemonsets = daemonsets
            .entry(ds.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_daemonsets.insert(ds.name.clone(), ds);
        Ok(())
    }

    /// Get a DaemonSet.
    pub async fn get_daemonset(&self, namespace: &str, name: &str) -> Option<DaemonSetDesired> {
        let daemonsets = self.daemonsets.read().await;
        daemonsets
            .get(namespace)
            .and_then(|ns| ns.get(name).cloned())
    }

    /// List all DaemonSets in a namespace.
    pub async fn list_daemonsets(&self, namespace: &str) -> Vec<DaemonSetDesired> {
        let daemonsets = self.daemonsets.read().await;
        daemonsets
            .get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all DaemonSets.
    pub async fn list_all_daemonsets(&self) -> Vec<(String, DaemonSetDesired)> {
        let daemonsets = self.daemonsets.read().await;
        daemonsets
            .iter()
            .flat_map(|(ns, ns_dss)| {
                ns_dss
                    .values()
                    .map(|ds| (ns.clone(), ds.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Update a DaemonSet.
    pub async fn update_daemonset(&self, ds: DaemonSetDesired) -> Result<()> {
        let mut daemonsets = self.daemonsets.write().await;
        if let Some(ns_daemonsets) = daemonsets.get_mut(&ds.namespace) {
            if let Some(existing) = ns_daemonsets.get_mut(&ds.name) {
                *existing = ds;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!(
            "DaemonSet {} not found in namespace {}",
            ds.name, ds.namespace
        )))
    }

    /// Delete a DaemonSet.
    pub async fn delete_daemonset(&self, namespace: &str, name: &str) -> Result<()> {
        let mut daemonsets = self.daemonsets.write().await;
        if let Some(ns_daemonsets) = daemonsets.get_mut(namespace) {
            ns_daemonsets.remove(name);
        }
        Ok(())
    }

    // ==================== Node Operations ====================

    /// Register a node.
    pub async fn register_node(&self, node: DaemonNode) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.insert(node.name.clone(), node);
        Ok(())
    }

    /// Unregister a node.
    pub async fn unregister_node(&self, name: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(name);
        Ok(())
    }

    /// Get a node.
    pub async fn get_node(&self, name: &str) -> Option<DaemonNode> {
        let nodes = self.nodes.read().await;
        nodes.get(name).cloned()
    }

    /// List all nodes.
    pub async fn list_nodes(&self) -> Vec<DaemonNode> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// Get nodes matching a selector.
    pub async fn get_matching_nodes(&self, selector: &DaemonSetSelector) -> Vec<DaemonNode> {
        let nodes = self.nodes.read().await;
        nodes
            .values()
            .filter(|node| self.node_matches_selector(node, selector))
            .cloned()
            .collect()
    }

    /// Check if a node matches a selector.
    fn node_matches_selector(&self, node: &DaemonNode, selector: &DaemonSetSelector) -> bool {
        // Check match_labels
        for (key, value) in &selector.match_labels {
            match node.labels.get(key) {
                Some(v) if v == value => {}
                _ => return false,
            }
        }

        // Check match_expressions
        for expr in &selector.match_expressions {
            let matches = match expr.operator.as_str() {
                "In" => {
                    if let Some(node_value) = node.labels.get(&expr.key) {
                        expr.values.contains(node_value)
                    } else {
                        false
                    }
                }
                "NotIn" => {
                    if let Some(node_value) = node.labels.get(&expr.key) {
                        !expr.values.contains(node_value)
                    } else {
                        true
                    }
                }
                "Exists" => node.labels.contains_key(&expr.key),
                "DoesNotExist" => !node.labels.contains_key(&expr.key),
                _ => false,
            };
            if !matches {
                return false;
            }
        }

        true
    }

    /// Update node heartbeat.
    pub async fn node_heartbeat(&self, name: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if let Some(node) = nodes.get_mut(name) {
            node.last_heartbeat = Utc::now();
            Ok(())
        } else {
            Err(A3sError::Project(format!("node {} not found", name)))
        }
    }

    // ==================== DaemonSet Actual State ====================

    /// Initialize or update actual state for a DaemonSet.
    pub async fn update_actual(&self, actual: DaemonSetActual) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        let ns_actuals = actuals
            .entry(actual.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_actuals.insert(actual.name.clone(), actual);
        Ok(())
    }

    /// Get actual state for a DaemonSet.
    pub async fn get_actual(&self, namespace: &str, name: &str) -> Option<DaemonSetActual> {
        let actuals = self.actuals.read().await;
        actuals.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    // ==================== Scheduling ====================

    /// Compute which nodes should run a DaemonSet pod.
    pub async fn compute_desired_nodes(&self, ds: &DaemonSetDesired) -> Vec<String> {
        self.get_matching_nodes(&ds.spec.selector)
            .await
            .into_iter()
            .filter(|node| node.schedulable)
            .map(|node| node.name)
            .collect()
    }

    /// Check if a pod needs to be created on a node.
    pub async fn should_run_on_node(&self, namespace: &str, name: &str, node_name: &str) -> bool {
        // Check if DaemonSet exists
        let ds = match self.get_daemonset(namespace, name).await {
            Some(ds) => ds,
            None => return false,
        };

        // Check if node matches selector
        let nodes = self.get_matching_nodes(&ds.spec.selector).await;
        if !nodes.iter().any(|n| n.name == node_name) {
            return false;
        }

        // Check if pod already exists and is healthy
        if let Some(actual) = self.get_actual(namespace, name).await {
            if let Some(pod_status) = actual.node_pods.get(node_name) {
                return pod_status.status == DaemonPodState::Running;
            }
        }

        true
    }

    /// Mark a pod as scheduled on a node.
    pub async fn mark_pod_scheduled(
        &self,
        namespace: &str,
        name: &str,
        node_name: &str,
        pod_id: &str,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;

        // Get or create actual
        let actual = actuals
            .entry(namespace.to_string())
            .or_insert_with(HashMap::new)
            .entry(name.to_string())
            .or_insert_with(|| DaemonSetActual {
                name: name.to_string(),
                namespace: namespace.to_string(),
                desired_nodes: 0,
                current_nodes: 0,
                ready_nodes: 0,
                updated_nodes: 0,
                node_pods: HashMap::new(),
            });

        actual.node_pods.insert(
            node_name.to_string(),
            DaemonPodStatus {
                node_name: node_name.to_string(),
                pod_id: Some(pod_id.to_string()),
                status: DaemonPodState::Running,
                last_updated: Utc::now(),
            },
        );

        // Update counts
        actual.current_nodes = actual.node_pods.len() as i32;
        actual.ready_nodes = actual
            .node_pods
            .values()
            .filter(|p| p.status == DaemonPodState::Running)
            .count() as i32;

        Ok(())
    }

    /// Mark a pod as deleted from a node.
    pub async fn mark_pod_deleted(
        &self,
        namespace: &str,
        name: &str,
        node_name: &str,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;

        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                if let Some(pod_status) = actual.node_pods.get_mut(node_name) {
                    pod_status.status = DaemonPodState::Terminating;
                }
            }
        }

        Ok(())
    }

    /// Finalize pod deletion (remove from actual state).
    pub async fn finalize_pod_deletion(
        &self,
        namespace: &str,
        name: &str,
        node_name: &str,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;

        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.node_pods.remove(node_name);
                actual.current_nodes = actual.node_pods.len() as i32;
                actual.ready_nodes = actual
                    .node_pods
                    .values()
                    .filter(|p| p.status == DaemonPodState::Running)
                    .count() as i32;
            }
        }

        Ok(())
    }
}

impl Default for DaemonController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_controller() -> DaemonController {
        DaemonController::new()
    }

    fn test_daemonset() -> DaemonSetDesired {
        DaemonSetDesired {
            name: "logging-agent".to_string(),
            namespace: "default".to_string(),
            spec: DaemonSetSpec {
                selector: DaemonSetSelector {
                    match_labels: {
                        let mut m = HashMap::new();
                        m.insert("app".to_string(), "logging".to_string());
                        m
                    },
                    match_expressions: vec![],
                },
                template: DaemonSetPodTemplate {
                    metadata: DaemonSetPodMetadata {
                        labels: {
                            let mut m = HashMap::new();
                            m.insert("app".to_string(), "logging".to_string());
                            m
                        },
                        annotations: HashMap::new(),
                    },
                    spec: DaemonSetPodSpec {
                        image: "logging-agent:latest".to_string(),
                        command: vec!["/bin/sh".to_string(), "-c".to_string()],
                        args: vec!["sleep infinity".to_string()],
                        env: HashMap::new(),
                        ports: vec![],
                        resources: Default::default(),
                        volume_mounts: vec![],
                        health_check: Default::default(),
                    },
                },
                strategy: DaemonSetStrategy::RollingUpdate,
                min_ready_secs: 10,
                replicas: 3,
            },
            version: "v1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_node(name: &str, labels: HashMap<String, String>) -> DaemonNode {
        DaemonNode {
            name: name.to_string(),
            ip: format!(
                "10.0.0.{}",
                name.chars().last().unwrap().to_digit(10).unwrap()
            ),
            labels,
            taints: vec![],
            schedulable: true,
            last_heartbeat: Utc::now(),
            daemon_pods: vec![],
        }
    }

    #[tokio::test]
    async fn test_create_daemonset() {
        let controller = test_controller();
        let ds = test_daemonset();

        controller.create_daemonset(ds.clone()).await.unwrap();

        let retrieved = controller.get_daemonset("default", "logging-agent").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "logging-agent");
    }

    #[tokio::test]
    async fn test_node_selector() {
        let controller = test_controller();

        // Register nodes
        let node1_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "logging".to_string());
            m.insert("env".to_string(), "prod".to_string());
            m
        };
        let node2_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "web".to_string());
            m
        };

        controller
            .register_node(test_node("node-1", node1_labels))
            .await
            .unwrap();
        controller
            .register_node(test_node("node-2", node2_labels))
            .await
            .unwrap();

        // Selector that matches logging app
        let selector = DaemonSetSelector {
            match_labels: {
                let mut m = HashMap::new();
                m.insert("app".to_string(), "logging".to_string());
                m
            },
            match_expressions: vec![],
        };

        let matching = controller.get_matching_nodes(&selector).await;
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0].name, "node-1");

        // Selector with expression
        let selector2 = DaemonSetSelector {
            match_labels: HashMap::new(),
            match_expressions: vec![DaemonSetSelectorExpression {
                key: "app".to_string(),
                operator: "In".to_string(),
                values: vec!["logging".to_string(), "monitoring".to_string()],
            }],
        };

        let matching2 = controller.get_matching_nodes(&selector2).await;
        assert_eq!(matching2.len(), 1);
        assert_eq!(matching2[0].name, "node-1");
    }

    #[tokio::test]
    async fn test_compute_desired_nodes() {
        let controller = test_controller();

        // Register nodes
        let node1_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "logging".to_string());
            m
        };
        let node2_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "logging".to_string());
            m
        };
        let node3_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "web".to_string());
            m
        };

        controller
            .register_node(test_node("node-1", node1_labels))
            .await
            .unwrap();
        controller
            .register_node(test_node("node-2", node2_labels))
            .await
            .unwrap();
        controller
            .register_node(test_node("node-3", node3_labels))
            .await
            .unwrap();

        let ds = test_daemonset();
        let desired = controller.compute_desired_nodes(&ds).await;
        assert_eq!(desired.len(), 2);
        assert!(desired.contains(&"node-1".to_string()));
        assert!(desired.contains(&"node-2".to_string()));
    }

    #[tokio::test]
    async fn test_pod_scheduling() {
        let controller = test_controller();

        // Register node
        let labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "logging".to_string());
            m
        };
        controller
            .register_node(test_node("node-1", labels))
            .await
            .unwrap();

        let ds = test_daemonset();
        controller.create_daemonset(ds).await.unwrap();

        // Check if should run
        let should_run = controller
            .should_run_on_node("default", "logging-agent", "node-1")
            .await;
        assert!(should_run);

        // Mark pod as scheduled
        controller
            .mark_pod_scheduled("default", "logging-agent", "node-1", "pod-123")
            .await
            .unwrap();

        // Check actual state
        let actual = controller.get_actual("default", "logging-agent").await;
        assert!(actual.is_some());
        let actual = actual.unwrap();
        assert_eq!(actual.current_nodes, 1);
        assert_eq!(actual.ready_nodes, 1);

        // Check should still run (already running)
        let should_run = controller
            .should_run_on_node("default", "logging-agent", "node-1")
            .await;
        assert!(should_run);
    }

    #[tokio::test]
    async fn test_node_heartbeat() {
        let controller = test_controller();

        let labels = HashMap::new();
        controller
            .register_node(test_node("node-1", labels))
            .await
            .unwrap();

        // Wait a bit
        tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;

        // Update heartbeat
        controller.node_heartbeat("node-1").await.unwrap();

        let node = controller.get_node("node-1").await.unwrap();
        assert!(node.last_heartbeat > Utc::now() - chrono::Duration::seconds(1));
    }
}
