//! Node Management - Node registration, heartbeats, and capacity tracking.
//!
//! Manages the cluster nodes including node registration, health monitoring
//! via heartbeats, resource capacity tracking, and node conditions.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Node phase - lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NodePhase {
    /// Node is being created.
    Pending,
    /// Node is running.
    Running,
    /// Node is terminating.
    Terminating,
    /// Node has been terminated.
    Terminated,
}

impl Default for NodePhase {
    fn default() -> Self {
        NodePhase::Pending
    }
}

/// Node condition type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NodeConditionType {
    /// Node is ready to accept pods.
    Ready,
    /// Node has memory pressure.
    MemoryPressure,
    /// Node has disk pressure.
    DiskPressure,
    /// Node network is unavailable.
    NetworkUnavailable,
    /// Node has insufficient resources.
    InsufficientResources,
    /// Node conditions are unknown.
    Unknown,
}

impl Default for NodeConditionType {
    fn default() -> Self {
        NodeConditionType::Unknown
    }
}

/// Condition status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ConditionStatus {
    True,
    False,
    Unknown,
}

impl Default for ConditionStatus {
    fn default() -> Self {
        ConditionStatus::Unknown
    }
}

/// Node condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeCondition {
    /// Condition type.
    pub type_: NodeConditionType,
    /// Condition status.
    pub status: ConditionStatus,
    /// Last transition timestamp.
    pub last_transition_time: DateTime<Utc>,
    /// Last update timestamp.
    pub last_update_time: DateTime<Utc>,
    /// Reason for the condition.
    pub reason: Option<String>,
    /// Human-readable message.
    pub message: Option<String>,
}

/// Node taint effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum TaintEffect {
    /// Do not schedule new pods.
    NoSchedule,
    /// Prefer not to schedule new pods.
    PreferNoSchedule,
    /// Do not schedule and evict existing pods.
    NoExecute,
}

impl Default for TaintEffect {
    fn default() -> Self {
        TaintEffect::NoSchedule
    }
}

/// Node taint.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeTaint {
    /// Taint key.
    pub key: String,
    /// Taint value (optional).
    pub value: Option<String>,
    /// Taint effect.
    pub effect: TaintEffect,
    /// Time added (optional).
    pub time_added: Option<DateTime<Utc>>,
}

/// Node taint toleration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Toleration {
    /// Taint key to match.
    pub key: String,
    /// Operator (Exists or Equal).
    pub operator: TolerationOperator,
    /// Taint value (for Equal operator).
    pub value: Option<String>,
    /// Taint effect to match (empty matches all effects).
    pub effect: Option<TaintEffect>,
    /// Toleration seconds (for NoExecute).
    pub toleration_seconds: Option<i64>,
}

/// Toleration operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TolerationOperator {
    /// Exists operator (matches any value).
    Exists,
    /// Equal operator (matches if value equals).
    Equal,
}

impl Default for TolerationOperator {
    fn default() -> Self {
        TolerationOperator::Equal
    }
}

/// Node resource capacity.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeResources {
    /// CPU capacity in millicores.
    pub cpu_millicores: i64,
    /// Memory capacity in bytes.
    pub memory_bytes: i64,
    /// Pod capacity (max pods).
    pub pods: i32,
    /// Storage capacity in bytes.
    pub storage_bytes: Option<i64>,
    /// Ephemeral storage capacity in bytes.
    pub ephemeral_storage_bytes: Option<i64>,
}

/// Node allocatable resources (available for scheduling pods).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeAllocatable {
    /// CPU allocatable in millicores.
    pub cpu_millicores: i64,
    /// Memory allocatable in bytes.
    pub memory_bytes: i64,
    /// Pods allocatable.
    pub pods: i32,
    /// Storage allocatable in bytes.
    pub storage_bytes: Option<i64>,
}

/// Node metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeMeta {
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// Node system info (OS, kernel, container runtime, etc.).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSystemInfo {
    /// Operating system.
    pub operating_system: String,
    /// OS image.
    pub os_image: String,
    /// Kernel version.
    pub kernel_version: String,
    /// Container runtime version.
    pub container_runtime_version: String,
    /// A3slet version.
    pub a3slet_version: String,
    /// Machine ID.
    pub machine_id: String,
    /// Boot ID.
    pub boot_id: String,
    /// Node architecture.
    pub architecture: String,
}

impl Default for NodeSystemInfo {
    fn default() -> Self {
        Self {
            operating_system: "linux".to_string(),
            os_image: "A3S Lambda".to_string(),
            kernel_version: "unknown".to_string(),
            container_runtime_version: "a3s-lambda".to_string(),
            a3slet_version: "1.0.0".to_string(),
            machine_id: "unknown".to_string(),
            boot_id: "unknown".to_string(),
            architecture: "x86_64".to_string(),
        }
    }
}

/// Node desired state (registered configuration).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeDesired {
    /// Node name (unique identifier).
    pub name: String,
    /// Node metadata.
    #[serde(default)]
    pub metadata: NodeMeta,
    /// Node system info.
    #[serde(default)]
    pub system_info: NodeSystemInfo,
    /// Resource capacity.
    #[serde(default)]
    pub capacity: NodeResources,
    /// Allocatable resources.
    #[serde(default)]
    pub allocatable: NodeAllocatable,
    /// Node taints.
    #[serde(default)]
    pub taints: Vec<NodeTaint>,
    /// External ID (for cloud providers).
    pub external_id: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Node actual state (runtime state).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeActual {
    /// Node name.
    pub name: String,
    /// Current phase.
    pub phase: NodePhase,
    /// Node conditions.
    #[serde(default)]
    pub conditions: Vec<NodeCondition>,
    /// Node IP addresses.
    #[serde(default)]
    pub addresses: Vec<NodeAddress>,
    /// Current resources used.
    #[serde(default)]
    pub resources_used: NodeResources,
    /// Number of pods running on this node.
    pub running_pods: i32,
    /// Last heartbeat timestamp.
    pub last_heartbeat: DateTime<Utc>,
    /// Time when node became ready.
    pub ready_time: Option<DateTime<Utc>>,
}

/// Node address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeAddress {
    /// Address type.
    pub type_: NodeAddressType,
    /// Address value.
    pub address: String,
}

/// Node address type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum NodeAddressType {
    /// Hostname.
    Hostname,
    /// External IP.
    ExternalIP,
    /// Internal IP.
    InternalIP,
    /// External DNS.
    ExternalDNS,
    /// Internal DNS.
    InternalDNS,
}

impl Default for NodeAddressType {
    fn default() -> Self {
        NodeAddressType::InternalIP
    }
}

/// Node heartbeat message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHeartbeat {
    /// Node name.
    pub node_name: String,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
    /// Current conditions.
    #[serde(default)]
    pub conditions: Vec<NodeCondition>,
    /// Resources used.
    #[serde(default)]
    pub resources_used: NodeResources,
    /// Running pods count.
    pub running_pods: i32,
    /// Node phase.
    pub phase: NodePhase,
}

/// Node controller - manages node lifecycle.
pub struct NodeController {
    /// Nodes desired state (registered nodes).
    nodes: RwLock<HashMap<String, NodeDesired>>,
    /// Nodes actual state.
    actuals: RwLock<HashMap<String, NodeActual>>,
    /// Heartbeat timeout (seconds) before marking node as NotReady.
    heartbeat_timeout_secs: i64,
}

impl NodeController {
    /// Create a new Node controller.
    pub fn new() -> Self {
        Self {
            nodes: RwLock::new(HashMap::new()),
            actuals: RwLock::new(HashMap::new()),
            heartbeat_timeout_secs: 40, // 40 seconds timeout (3 missed heartbeats at 15s interval)
        }
    }

    // ==================== Node Registration ====================

    /// Register a new node.
    pub async fn register_node(&self, node: NodeDesired) -> Result<()> {
        let mut nodes = self.nodes.write().await;

        // Create initial actual state
        let actual = NodeActual {
            name: node.name.clone(),
            phase: NodePhase::Pending,
            conditions: vec![NodeCondition {
                type_: NodeConditionType::Ready,
                status: ConditionStatus::False,
                last_transition_time: Utc::now(),
                last_update_time: Utc::now(),
                reason: Some("NodeNotReady".to_string()),
                message: Some("Node is not yet ready".to_string()),
            }],
            addresses: vec![],
            resources_used: NodeResources::default(),
            running_pods: 0,
            last_heartbeat: Utc::now(),
            ready_time: None,
        };

        let mut actuals = self.actuals.write().await;
        actuals.insert(node.name.clone(), actual);
        nodes.insert(node.name.clone(), node);

        Ok(())
    }

    /// Get a node's desired state.
    pub async fn get_node(&self, name: &str) -> Option<NodeDesired> {
        let nodes = self.nodes.read().await;
        nodes.get(name).cloned()
    }

    /// List all registered nodes.
    pub async fn list_nodes(&self) -> Vec<NodeDesired> {
        let nodes = self.nodes.read().await;
        nodes.values().cloned().collect()
    }

    /// List all nodes in a specific phase.
    pub async fn list_nodes_by_phase(&self, phase: NodePhase) -> Vec<NodeDesired> {
        let nodes = self.nodes.read().await;
        let actuals = self.actuals.read().await;

        nodes
            .values()
            .filter(|n| {
                if let Some(actual) = actuals.get(&n.name) {
                    actual.phase == phase
                } else {
                    false
                }
            })
            .cloned()
            .collect()
    }

    /// Unregister a node.
    pub async fn unregister_node(&self, name: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        let mut actuals = self.actuals.write().await;
        nodes.remove(name);
        actuals.remove(name);
        Ok(())
    }

    /// Update a node's desired state.
    pub async fn update_node(&self, node: NodeDesired) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        if nodes.contains_key(&node.name) {
            nodes.insert(node.name.clone(), node);
            Ok(())
        } else {
            Err(A3sError::Project(format!("node {} not found", node.name)))
        }
    }

    // ==================== Node Heartbeat ====================

    /// Process a node heartbeat.
    pub async fn process_heartbeat(&self, heartbeat: NodeHeartbeat) -> Result<()> {
        let mut actuals = self.actuals.write().await;

        if let Some(actual) = actuals.get_mut(&heartbeat.node_name) {
            actual.last_heartbeat = heartbeat.timestamp;
            actual.phase = heartbeat.phase;
            actual.running_pods = heartbeat.running_pods;
            actual.resources_used = heartbeat.resources_used;
            actual.conditions = heartbeat.conditions;

            // Check if node just became ready
            let was_ready = actual.is_ready();
            let is_ready = self.check_node_ready(&actual.conditions);

            if !was_ready && is_ready {
                actual.ready_time = Some(Utc::now());
                self.update_condition(
                    actual,
                    NodeConditionType::Ready,
                    ConditionStatus::True,
                    "NodeReady".to_string(),
                    "Node is ready".to_string(),
                );
            }

            Ok(())
        } else {
            Err(A3sError::Project(format!(
                "Node {} not registered",
                heartbeat.node_name
            )))
        }
    }

    /// Check if conditions indicate node is ready.
    fn check_node_ready(&self, conditions: &[NodeCondition]) -> bool {
        conditions
            .iter()
            .find(|c| c.type_ == NodeConditionType::Ready)
            .map(|c| c.status == ConditionStatus::True)
            .unwrap_or(false)
    }

    /// Update a specific condition.
    fn update_condition(
        &self,
        actual: &mut NodeActual,
        type_: NodeConditionType,
        status: ConditionStatus,
        reason: String,
        message: String,
    ) {
        let now = Utc::now();

        if let Some(condition) = actual.conditions.iter_mut().find(|c| c.type_ == type_) {
            let changed = condition.status != status;
            condition.status = status;
            condition.last_update_time = now;
            if changed {
                condition.last_transition_time = now;
            }
            condition.reason = Some(reason);
            condition.message = Some(message);
        } else {
            actual.conditions.push(NodeCondition {
                type_,
                status,
                last_transition_time: now,
                last_update_time: now,
                reason: Some(reason),
                message: Some(message),
            });
        }
    }

    // ==================== Node Status ====================

    /// Get a node's actual state.
    pub async fn get_node_actual(&self, name: &str) -> Option<NodeActual> {
        let actuals = self.actuals.read().await;
        actuals.get(name).cloned()
    }

    /// Check if a node is ready.
    pub async fn is_node_ready(&self, name: &str) -> bool {
        if let Some(actual) = self.get_node_actual(name).await {
            actual.is_ready()
        } else {
            false
        }
    }

    /// Check for nodes that have missed heartbeats.
    pub async fn find_stale_nodes(&self) -> Vec<String> {
        let actuals = self.actuals.read().await;
        let now = Utc::now();
        let timeout = chrono::Duration::seconds(self.heartbeat_timeout_secs);

        actuals
            .iter()
            .filter(|(_, actual)| {
                actual.phase == NodePhase::Running && (now - actual.last_heartbeat) > timeout
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Mark a node as not ready due to missed heartbeats.
    pub async fn mark_node_not_ready(&self, name: &str) -> Result<()> {
        let mut actuals = self.actuals.write().await;

        if let Some(actual) = actuals.get_mut(name) {
            actual.phase = NodePhase::Terminating;
            self.update_condition(
                actual,
                NodeConditionType::Ready,
                ConditionStatus::False,
                "NodeHeartbeatTimeout".to_string(),
                format!(
                    "Node missed {} heartbeats",
                    self.heartbeat_timeout_secs / 15
                ),
            );
            Ok(())
        } else {
            Err(A3sError::Project(format!("Node {} not found", name)))
        }
    }

    // ==================== Node Capacity ====================

    /// Get node capacity summary.
    pub async fn get_node_capacity(&self, name: &str) -> Option<NodeCapacitySummary> {
        let nodes = self.nodes.read().await;
        let actuals = self.actuals.read().await;

        if let (Some(node), Some(actual)) = (nodes.get(name), actuals.get(name)) {
            Some(NodeCapacitySummary {
                capacity: node.capacity.clone(),
                allocatable: node.allocatable.clone(),
                used: actual.resources_used.clone(),
                available: self.calculate_available(&node.allocatable, &actual.resources_used),
                utilization_percent: self
                    .calculate_utilization(&node.capacity, &actual.resources_used),
            })
        } else {
            None
        }
    }

    /// Calculate available resources.
    fn calculate_available(
        &self,
        allocatable: &NodeAllocatable,
        used: &NodeResources,
    ) -> NodeResources {
        NodeResources {
            cpu_millicores: allocatable
                .cpu_millicores
                .saturating_sub(used.cpu_millicores),
            memory_bytes: allocatable.memory_bytes.saturating_sub(used.memory_bytes),
            pods: allocatable.pods.saturating_sub(used.pods),
            storage_bytes: allocatable
                .storage_bytes
                .map(|s| s.saturating_sub(used.storage_bytes.unwrap_or(0))),
            ephemeral_storage_bytes: None, // NodeAllocatable doesn't track ephemeral storage
        }
    }

    /// Calculate utilization percentage.
    fn calculate_utilization(
        &self,
        capacity: &NodeResources,
        used: &NodeResources,
    ) -> NodeUtilization {
        NodeUtilization {
            cpu_percent: if capacity.cpu_millicores > 0 {
                (used.cpu_millicores as f64 / capacity.cpu_millicores as f64 * 100.0).min(100.0)
            } else {
                0.0
            },
            memory_percent: if capacity.memory_bytes > 0 {
                (used.memory_bytes as f64 / capacity.memory_bytes as f64 * 100.0).min(100.0)
            } else {
                0.0
            },
            pods_percent: if capacity.pods > 0 {
                (used.pods as f64 / capacity.pods as f64 * 100.0).min(100.0)
            } else {
                0.0
            },
        }
    }

    // ==================== Node Filtering ====================

    /// Get nodes matching node selector.
    pub async fn get_nodes_by_selector(&self, selector: &NodeSelector) -> Vec<String> {
        let nodes = self.nodes.read().await;

        nodes
            .values()
            .filter(|node| selector.matches(&node.metadata.labels))
            .map(|n| n.name.clone())
            .collect()
    }

    /// Check if node has sufficient resources for a workload.
    pub async fn has_sufficient_resources(
        &self,
        name: &str,
        cpu_millicores: i64,
        memory_bytes: i64,
        pods: i32,
    ) -> bool {
        if let Some(capacity) = self.get_node_capacity(name).await {
            capacity.available.cpu_millicores >= cpu_millicores
                && capacity.available.memory_bytes >= memory_bytes
                && capacity.available.pods >= pods
        } else {
            false
        }
    }

    /// Find nodes with sufficient resources for a workload.
    pub async fn find_nodes_with_resources(
        &self,
        cpu_millicores: i64,
        memory_bytes: i64,
        pods: i32,
    ) -> Vec<NodeResourcesMatch> {
        let nodes = self.list_nodes().await;
        let mut matches = Vec::new();

        for node in nodes {
            if let Some(capacity) = self.get_node_capacity(&node.name).await {
                let cpu_ok = capacity.available.cpu_millicores >= cpu_millicores;
                let mem_ok = capacity.available.memory_bytes >= memory_bytes;
                let pods_ok = capacity.available.pods >= pods;

                if cpu_ok && mem_ok && pods_ok {
                    matches.push(NodeResourcesMatch {
                        node_name: node.name.clone(),
                        available_cpu: capacity.available.cpu_millicores,
                        available_memory: capacity.available.memory_bytes,
                        available_pods: capacity.available.pods,
                        utilization: capacity.utilization_percent,
                    });
                }
            }
        }

        // Sort by lowest utilization (best fit)
        matches.sort_by(|a, b| {
            a.utilization
                .cpu_percent
                .partial_cmp(&b.utilization.cpu_percent)
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        matches
    }

    // ==================== Taints and Tolerations ====================

    /// Check if a node tolerates a taint.
    pub fn tolerates_taint(&self, tolerations: &[Toleration], taint: &NodeTaint) -> bool {
        tolerations.iter().any(|t| t.matches_taint(taint))
    }

    /// Check if a node can be scheduled with given tolerations.
    pub async fn can_schedule(&self, name: &str, tolerations: &[Toleration]) -> bool {
        let nodes = self.nodes.read().await;

        if let Some(node) = nodes.get(name) {
            // If node has no taints, it can be scheduled
            if node.taints.is_empty() {
                return true;
            }

            // Check if all node taints are tolerated
            node.taints
                .iter()
                .all(|t| self.tolerates_taint(tolerations, t))
        } else {
            false
        }
    }
}

impl Default for NodeController {
    fn default() -> Self {
        Self::new()
    }
}

impl NodeActual {
    /// Check if node is ready.
    pub fn is_ready(&self) -> bool {
        self.conditions
            .iter()
            .find(|c| c.type_ == NodeConditionType::Ready)
            .map(|c| c.status == ConditionStatus::True)
            .unwrap_or(false)
    }

    /// Check if node has memory pressure.
    pub fn has_memory_pressure(&self) -> bool {
        self.conditions
            .iter()
            .find(|c| c.type_ == NodeConditionType::MemoryPressure)
            .map(|c| c.status == ConditionStatus::True)
            .unwrap_or(false)
    }

    /// Check if node has disk pressure.
    pub fn has_disk_pressure(&self) -> bool {
        self.conditions
            .iter()
            .find(|c| c.type_ == NodeConditionType::DiskPressure)
            .map(|c| c.status == ConditionStatus::True)
            .unwrap_or(false)
    }

    /// Check if node network is unavailable.
    pub fn is_network_unavailable(&self) -> bool {
        self.conditions
            .iter()
            .find(|c| c.type_ == NodeConditionType::NetworkUnavailable)
            .map(|c| c.status == ConditionStatus::True)
            .unwrap_or(false)
    }
}

impl Toleration {
    /// Check if this toleration matches a taint.
    pub fn matches_taint(&self, taint: &NodeTaint) -> bool {
        // Key must match
        if self.key != taint.key {
            return false;
        }

        // Effect must match or be empty (matches all)
        if let Some(effect) = &self.effect {
            if *effect != taint.effect {
                return false;
            }
        }

        // Operator check
        match self.operator {
            TolerationOperator::Exists => true, // Exists matches any value
            TolerationOperator::Equal => {
                // Value must match
                self.value.as_ref() == taint.value.as_ref()
            }
        }
    }
}

/// Node selector for workload placement.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NodeSelector {
    /// Match labels.
    #[serde(default)]
    pub match_labels: HashMap<String, String>,
    /// Match expressions.
    #[serde(default)]
    pub match_expressions: Vec<NodeSelectorRequirement>,
}

impl NodeSelector {
    /// Check if labels match this selector.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        // Check match_labels
        for (key, value) in &self.match_labels {
            if labels.get(key) != Some(value) {
                return false;
            }
        }

        // Check match_expressions
        for expr in &self.match_expressions {
            if !expr.matches(labels) {
                return false;
            }
        }

        true
    }
}

/// Node selector requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeSelectorRequirement {
    /// Key to match.
    pub key: String,
    /// Operator.
    pub operator: NodeSelectorOperator,
    /// Values (for In, NotIn,Gt, Lt).
    #[serde(default)]
    pub values: Vec<String>,
}

/// Node selector operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum NodeSelectorOperator {
    /// In set.
    In,
    /// Not in set.
    NotIn,
    /// Exists.
    Exists,
    /// Does not exist.
    DoesNotExist,
    /// Greater than.
    Gt,
    /// Less than.
    Lt,
}

impl NodeSelectorRequirement {
    /// Check if this requirement matches labels.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        match self.operator {
            NodeSelectorOperator::In => {
                if let Some(value) = labels.get(&self.key) {
                    self.values.contains(value)
                } else {
                    false
                }
            }
            NodeSelectorOperator::NotIn => {
                if let Some(value) = labels.get(&self.key) {
                    !self.values.contains(value)
                } else {
                    true
                }
            }
            NodeSelectorOperator::Exists => labels.contains_key(&self.key),
            NodeSelectorOperator::DoesNotExist => !labels.contains_key(&self.key),
            NodeSelectorOperator::Gt | NodeSelectorOperator::Lt => {
                if let Some(value) = labels.get(&self.key) {
                    if let (Ok(label_num), Ok(target)) =
                        (value.parse::<i64>(), self.values[0].parse::<i64>())
                    {
                        match self.operator {
                            NodeSelectorOperator::Gt => label_num > target,
                            NodeSelectorOperator::Lt => label_num < target,
                            _ => false,
                        }
                    } else {
                        false
                    }
                } else {
                    false
                }
            }
        }
    }
}

/// Node capacity summary.
#[derive(Debug, Clone)]
pub struct NodeCapacitySummary {
    /// Total capacity.
    pub capacity: NodeResources,
    /// Allocatable resources.
    pub allocatable: NodeAllocatable,
    /// Currently used.
    pub used: NodeResources,
    /// Available for scheduling.
    pub available: NodeResources,
    /// Utilization percentages.
    pub utilization_percent: NodeUtilization,
}

/// Node resource utilization.
#[derive(Debug, Clone)]
pub struct NodeUtilization {
    /// CPU utilization percent.
    pub cpu_percent: f64,
    /// Memory utilization percent.
    pub memory_percent: f64,
    /// Pods utilization percent.
    pub pods_percent: f64,
}

/// Node resources match for scheduling.
#[derive(Debug, Clone)]
pub struct NodeResourcesMatch {
    /// Node name.
    pub node_name: String,
    /// Available CPU in millicores.
    pub available_cpu: i64,
    /// Available memory in bytes.
    pub available_memory: i64,
    /// Available pods.
    pub available_pods: i32,
    /// Current utilization.
    pub utilization: NodeUtilization,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_node_controller() -> NodeController {
        NodeController::new()
    }

    fn test_node() -> NodeDesired {
        NodeDesired {
            name: "node-1".to_string(),
            metadata: NodeMeta {
                labels: {
                    let mut m = HashMap::new();
                    m.insert("node-role".to_string(), "worker".to_string());
                    m.insert("gpu".to_string(), "true".to_string());
                    m
                },
                annotations: Default::default(),
            },
            system_info: NodeSystemInfo::default(),
            capacity: NodeResources {
                cpu_millicores: 4000,
                memory_bytes: 16 * 1024 * 1024 * 1024, // 16GB
                pods: 110,
                storage_bytes: Some(512 * 1024 * 1024 * 1024), // 512GB
                ephemeral_storage_bytes: Some(100 * 1024 * 1024 * 1024), // 100GB
            },
            allocatable: NodeAllocatable {
                cpu_millicores: 3500,
                memory_bytes: 14 * 1024 * 1024 * 1024, // 14GB
                pods: 100,
                storage_bytes: Some(500 * 1024 * 1024 * 1024),
            },
            taints: vec![],
            external_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_register_node() {
        let controller = test_node_controller();
        let node = test_node();

        controller.register_node(node.clone()).await.unwrap();

        let retrieved = controller.get_node("node-1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "node-1");
    }

    #[tokio::test]
    async fn test_node_heartbeat() {
        let controller = test_node_controller();
        let node = test_node();

        controller.register_node(node).await.unwrap();

        let heartbeat = NodeHeartbeat {
            node_name: "node-1".to_string(),
            timestamp: Utc::now(),
            conditions: vec![NodeCondition {
                type_: NodeConditionType::Ready,
                status: ConditionStatus::True,
                last_transition_time: Utc::now(),
                last_update_time: Utc::now(),
                reason: Some("NodeReady".to_string()),
                message: Some("Node is ready".to_string()),
            }],
            resources_used: NodeResources {
                cpu_millicores: 1000,
                memory_bytes: 4 * 1024 * 1024 * 1024,
                pods: 10,
                storage_bytes: None,
                ephemeral_storage_bytes: None,
            },
            running_pods: 10,
            phase: NodePhase::Running,
        };

        controller.process_heartbeat(heartbeat).await.unwrap();

        let actual = controller.get_node_actual("node-1").await;
        assert!(actual.is_some());
        let actual = actual.unwrap();
        assert!(actual.is_ready());
        assert_eq!(actual.running_pods, 10);
    }

    #[tokio::test]
    async fn test_node_capacity() {
        let controller = test_node_controller();
        let node = test_node();

        controller.register_node(node).await.unwrap();

        let capacity = controller.get_node_capacity("node-1").await;
        assert!(capacity.is_some());
        let cap = capacity.unwrap();
        assert_eq!(cap.allocatable.cpu_millicores, 3500);
        assert_eq!(cap.allocatable.memory_bytes, 14 * 1024 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_node_selector() {
        let controller = test_node_controller();
        let node = test_node();

        controller.register_node(node).await.unwrap();

        let selector = NodeSelector {
            match_labels: {
                let mut m = HashMap::new();
                m.insert("node-role".to_string(), "worker".to_string());
                m
            },
            match_expressions: vec![],
        };

        let matching = controller.get_nodes_by_selector(&selector).await;
        assert_eq!(matching.len(), 1);
        assert_eq!(matching[0], "node-1");

        // Non-matching selector
        let selector = NodeSelector {
            match_labels: {
                let mut m = HashMap::new();
                m.insert("node-role".to_string(), "master".to_string());
                m
            },
            match_expressions: vec![],
        };

        let matching = controller.get_nodes_by_selector(&selector).await;
        assert!(matching.is_empty());
    }

    #[tokio::test]
    async fn test_find_nodes_with_resources() {
        let controller = test_node_controller();
        let node = test_node();

        controller.register_node(node).await.unwrap();

        let matches = controller
            .find_nodes_with_resources(1000, 2 * 1024 * 1024 * 1024, 5)
            .await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].node_name, "node-1");

        // Request more than available
        let matches = controller
            .find_nodes_with_resources(5000, 2 * 1024 * 1024 * 1024, 5)
            .await;
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_tolerations() {
        let taint = NodeTaint {
            key: "node.a3s.io/not-ready".to_string(),
            value: None,
            effect: TaintEffect::NoSchedule,
            time_added: None,
        };

        // Matching toleration
        let toleration = Toleration {
            key: "node.a3s.io/not-ready".to_string(),
            operator: TolerationOperator::Exists,
            value: None,
            effect: None,
            toleration_seconds: None,
        };

        assert!(toleration.matches_taint(&taint));

        // Non-matching toleration (different key)
        let toleration = Toleration {
            key: "other-key".to_string(),
            operator: TolerationOperator::Exists,
            value: None,
            effect: None,
            toleration_seconds: None,
        };

        assert!(!toleration.matches_taint(&taint));
    }

    #[tokio::test]
    async fn test_unregister_node() {
        let controller = test_node_controller();
        let node = test_node();

        controller.register_node(node).await.unwrap();
        let retrieved = controller.get_node("node-1").await;
        assert!(retrieved.is_some());

        controller.unregister_node("node-1").await.unwrap();
        let retrieved = controller.get_node("node-1").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_node_selector_expressions() {
        let selector = NodeSelector {
            match_labels: Default::default(),
            match_expressions: vec![NodeSelectorRequirement {
                key: "gpu".to_string(),
                operator: NodeSelectorOperator::In,
                values: vec!["true".to_string()],
            }],
        };

        let labels_true = {
            let mut m = HashMap::new();
            m.insert("gpu".to_string(), "true".to_string());
            m
        };

        let labels_false = {
            let mut m = HashMap::new();
            m.insert("gpu".to_string(), "false".to_string());
            m
        };

        assert!(selector.matches(&labels_true));
        assert!(!selector.matches(&labels_false));

        // Test Exists operator - both true and false values should match since key exists
        let selector = NodeSelector {
            match_labels: Default::default(),
            match_expressions: vec![NodeSelectorRequirement {
                key: "gpu".to_string(),
                operator: NodeSelectorOperator::Exists,
                values: vec![],
            }],
        };

        assert!(selector.matches(&labels_true));
        assert!(selector.matches(&labels_false)); // Key exists even if value is "false"

        // Test DoesNotExist operator - key must not exist
        // DoesNotExist gpu should NOT match labels_true (gpu exists)
        let selector = NodeSelector {
            match_labels: Default::default(),
            match_expressions: vec![NodeSelectorRequirement {
                key: "gpu".to_string(),
                operator: NodeSelectorOperator::DoesNotExist,
                values: vec![],
            }],
        };

        assert!(!selector.matches(&labels_true)); // gpu exists, so DoesNotExist fails
        assert!(!selector.matches(&labels_false)); // gpu exists, so DoesNotExist fails

        // Use a map without the key to test DoesNotExist matching
        let labels_no_key = HashMap::new();
        let selector = NodeSelector {
            match_labels: Default::default(),
            match_expressions: vec![NodeSelectorRequirement {
                key: "nonexistent".to_string(),
                operator: NodeSelectorOperator::DoesNotExist,
                values: vec![],
            }],
        };

        // DoesNotExist nonexistent should match labels_no_key (key doesn't exist)
        assert!(selector.matches(&labels_no_key));
        // DoesNotExist nonexistent also matches labels_true because nonexistent isn't in labels_true
        assert!(selector.matches(&labels_true));
    }
}
