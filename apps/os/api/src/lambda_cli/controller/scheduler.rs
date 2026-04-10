//! Scheduler - Pod scheduling for A3S Lambda.
//!
//! Implements node selection for pods based on:
//! - Taints and tolerations
//! - Node selectors
//! - Resource capacity (CPU, memory)
//! - Pod affinity/anti-affinity (future)

use crate::errors::Result;
use crate::state::namespace::{parse_cpu_to_millicores, parse_memory_to_bytes};
use crate::state::node::{NodeDesired, NodeSelector, NodeTaint, Toleration};
use crate::state::{DeploymentDesired, PodActual, ResourceRequirements, StateStore};
use std::sync::Arc;

/// Scheduler for selecting nodes for pods.
pub struct Scheduler {
    store: Arc<StateStore>,
}

impl Scheduler {
    /// Create a new scheduler.
    pub fn new(store: Arc<StateStore>) -> Self {
        Self { store }
    }

    /// Schedule a pod for a deployment.
    /// Returns the selected node name, or None if no suitable node is found.
    pub async fn schedule(
        &self,
        deployment: &DeploymentDesired,
        existing_pods: &[PodActual],
    ) -> Result<Option<String>> {
        // Get all nodes from state store
        let nodes = self.store.load_nodes()?;

        // Filter nodes by predicates
        let eligible_nodes: Vec<&NodeDesired> = nodes
            .values()
            .filter(|node| self.node_predicates(node, deployment))
            .collect();

        if eligible_nodes.is_empty() {
            tracing::warn!(
                deployment = %deployment.name,
                "no eligible nodes found for scheduling"
            );
            return Ok(None);
        }

        // Score nodes and select the best one
        let scored_nodes: Vec<(&NodeDesired, i64)> = eligible_nodes
            .iter()
            .map(|node| {
                let score = self.node_score(node, deployment, existing_pods);
                (*node, score)
            })
            .collect();

        // Select node with highest score
        let selected = scored_nodes
            .into_iter()
            .max_by_key(|(_, score)| *score)
            .map(|(node, _)| node.name.clone());

        if let Some(ref node_name) = selected {
            tracing::debug!(
                deployment = %deployment.name,
                node = node_name,
                "scheduled pod to node"
            );
        }

        Ok(selected)
    }

    /// Check if a node passes scheduling predicates.
    fn node_predicates(&self, node: &NodeDesired, deployment: &DeploymentDesired) -> bool {
        // Check 1: Node must be in Ready condition (not implemented yet, assume true)

        // Check 2: Taints and tolerations
        if !self.check_tolerations(node, deployment) {
            return false;
        }

        // Check 3: Node selector
        if !self.check_node_selector(node, deployment) {
            return false;
        }

        // Check 4: Resource capacity
        if !self.check_resources(node, deployment) {
            return false;
        }

        true
    }

    /// Check if tolerations match node taints.
    fn check_tolerations(&self, node: &NodeDesired, deployment: &DeploymentDesired) -> bool {
        // Get tolerations from deployment (default to empty)
        let tolerations = self.get_tolerations(deployment);

        // If no taints on node, tolerate anything
        if node.taints.is_empty() {
            return true;
        }

        // Each node taint must be tolerated
        for taint in &node.taints {
            let mut tolerated = false;

            for toleration in &tolerations {
                if self.tolerates_taint(toleration, taint) {
                    tolerated = true;
                    break;
                }
            }

            if !tolerated {
                return false;
            }
        }

        true
    }

    /// Check if a toleration matches a taint.
    fn tolerates_taint(&self, toleration: &Toleration, taint: &NodeTaint) -> bool {
        // Key must match
        if toleration.key != taint.key {
            return false;
        }

        // Operator
        match toleration.operator {
            crate::state::node::TolerationOperator::Exists => {
                // Exists operator matches any value if key exists
                // Effect doesn't matter if operator is Exists and effect is None
                if toleration.effect.is_none() {
                    return true;
                }
                toleration.effect == Some(taint.effect)
            }
            crate::state::node::TolerationOperator::Equal => {
                // Value must match if specified
                if let (Some(t_value), Some(r_value)) = (&taint.value, &toleration.value) {
                    if t_value != r_value {
                        return false;
                    }
                }
                // Effect must match if specified
                if let Some(effect) = toleration.effect {
                    return effect == taint.effect;
                }
                true
            }
        }
    }

    /// Check if node selector matches.
    fn check_node_selector(&self, node: &NodeDesired, deployment: &DeploymentDesired) -> bool {
        // Get node selector from deployment (default to empty)
        let selector = self.get_node_selector(deployment);

        selector.matches(&node.metadata.labels)
    }

    /// Check if node has sufficient resources.
    fn check_resources(&self, node: &NodeDesired, deployment: &DeploymentDesired) -> bool {
        let required = self.get_resource_requirements(deployment);

        // Check CPU - use request if available, otherwise use 100m as default
        let cpu_required = if let Some(ref cpu) = required.cpu_request {
            parse_cpu_to_millicores(cpu).unwrap_or(100) as i64
        } else {
            100 // Default to 100m
        };

        // Check memory - use request if available, otherwise use 128Mi as default
        let mem_required = if let Some(ref mem) = required.memory_request {
            parse_memory_to_bytes(mem).unwrap_or(128 * 1024 * 1024)
        } else {
            128 * 1024 * 1024 // Default to 128Mi
        };

        // Check CPU
        if node.allocatable.cpu_millicores < cpu_required {
            return false;
        }

        // Check memory
        if node.allocatable.memory_bytes < mem_required {
            return false;
        }

        // Check pods capacity
        if node.allocatable.pods <= 0 {
            return false;
        }

        true
    }

    /// Score a node for scheduling priority.
    /// Higher score = more preferred.
    fn node_score(
        &self,
        node: &NodeDesired,
        deployment: &DeploymentDesired,
        existing_pods: &[PodActual],
    ) -> i64 {
        let mut score: i64 = 1000; // Base score

        // Bonus for available resources (more available = higher score)
        let required = self.get_resource_requirements(deployment);

        // CPU scoring: prefer nodes with more available CPU
        let cpu_required = if let Some(ref cpu) = required.cpu_request {
            parse_cpu_to_millicores(cpu).unwrap_or(100) as i64
        } else {
            100
        };
        let available_cpu = node.allocatable.cpu_millicores - cpu_required;
        score += available_cpu.max(0) / 100; // 1 point per 100m CPU available

        // Memory scoring: prefer nodes with more available memory
        let mem_required = if let Some(ref mem) = required.memory_request {
            parse_memory_to_bytes(mem).unwrap_or(128 * 1024 * 1024)
        } else {
            128 * 1024 * 1024
        };
        let available_mem = node.allocatable.memory_bytes - mem_required;
        score += available_mem.max(0) / (1024 * 1024 * 100); // 1 point per 100MB available

        // Penalty for existing pods on node (spread across nodes)
        let pod_count = existing_pods
            .iter()
            .filter(|p| p.node_name.as_ref() == Some(&node.name))
            .count() as i64;
        score -= pod_count * 10;

        // Penalty for taints (NoSchedule > PreferNoSchedule > NoExecute)
        for taint in &node.taints {
            match taint.effect {
                crate::state::node::TaintEffect::NoSchedule => score -= 100,
                crate::state::node::TaintEffect::PreferNoSchedule => score -= 50,
                crate::state::node::TaintEffect::NoExecute => score -= 200,
            }
        }

        score.max(0)
    }

    /// Get tolerations from deployment.
    fn get_tolerations(&self, deployment: &DeploymentDesired) -> Vec<Toleration> {
        deployment.tolerations.clone()
    }

    /// Get node selector from deployment.
    fn get_node_selector(&self, deployment: &DeploymentDesired) -> NodeSelector {
        deployment.node_selector.clone()
    }

    /// Get resource requirements from deployment.
    fn get_resource_requirements(&self, deployment: &DeploymentDesired) -> ResourceRequirements {
        deployment.resources.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::StateStore;
    use crate::state::{HealthStatus, PodStatus};
    use chrono::Utc;
    use std::collections::HashMap;
    use std::env::temp_dir;

    fn test_store() -> Arc<StateStore> {
        let dir = temp_dir().join(format!("a3s-scheduler-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        Arc::new(StateStore::new(&dir))
    }

    fn test_node() -> NodeDesired {
        NodeDesired {
            name: "node-1".to_string(),
            metadata: crate::state::node::NodeMeta {
                labels: {
                    let mut m = HashMap::new();
                    m.insert("node-role".to_string(), "worker".to_string());
                    m.insert("gpu".to_string(), "false".to_string());
                    m
                },
                annotations: Default::default(),
            },
            system_info: Default::default(),
            capacity: crate::state::node::NodeResources {
                cpu_millicores: 4000,
                memory_bytes: 8 * 1024 * 1024 * 1024,
                pods: 110,
                storage_bytes: Some(100 * 1024 * 1024 * 1024),
                ephemeral_storage_bytes: Some(50 * 1024 * 1024 * 1024),
            },
            allocatable: crate::state::node::NodeAllocatable {
                cpu_millicores: 3500,
                memory_bytes: 7 * 1024 * 1024 * 1024,
                pods: 108,
                storage_bytes: Some(90 * 1024 * 1024 * 1024),
            },
            taints: vec![],
            external_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_deployment() -> DeploymentDesired {
        DeploymentDesired {
            name: "test-app".to_string(),
            namespace: "default".to_string(),
            image: "nginx:latest".to_string(),
            replicas: 3,
            env: Default::default(),
            ports: vec![],
            version: "v1".to_string(),
            strategy: crate::state::UpdateStrategy::Replace,
            resources: ResourceRequirements {
                memory_limit: None,
                cpu_limit: None,
                memory_request: Some("128Mi".to_string()),
                cpu_request: Some("100m".to_string()),
            },
            health_check: Default::default(),
            node_selector: NodeSelector::default(),
            tolerations: vec![],
            labels: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_schedule_no_taints() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let node = test_node();
        let deployment = test_deployment();

        let eligible = scheduler.node_predicates(&node, &deployment);
        assert!(eligible);
    }

    #[tokio::test]
    async fn test_schedule_taint_no_toleration() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let mut node = test_node();
        node.taints = vec![NodeTaint {
            key: "node.a3s.io/gpu".to_string(),
            value: Some("true".to_string()),
            effect: crate::state::node::TaintEffect::NoSchedule,
            time_added: None,
        }];

        let deployment = test_deployment();

        let eligible = scheduler.node_predicates(&node, &deployment);
        assert!(!eligible);
    }

    #[tokio::test]
    async fn test_schedule_taint_with_toleration() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let mut node = test_node();
        node.taints = vec![NodeTaint {
            key: "node.a3s.io/gpu".to_string(),
            value: Some("true".to_string()),
            effect: crate::state::node::TaintEffect::NoSchedule,
            time_added: None,
        }];

        let mut deployment = test_deployment();
        deployment.tolerations = vec![Toleration {
            key: "node.a3s.io/gpu".to_string(),
            operator: crate::state::node::TolerationOperator::Equal,
            value: Some("true".to_string()),
            effect: Some(crate::state::node::TaintEffect::NoSchedule),
            toleration_seconds: None,
        }];

        let eligible = scheduler.node_predicates(&node, &deployment);
        assert!(eligible);
    }

    #[tokio::test]
    async fn test_schedule_node_selector_match() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let node = test_node();
        let mut deployment = test_deployment();
        deployment.node_selector = NodeSelector {
            match_labels: {
                let mut m = HashMap::new();
                m.insert("node-role".to_string(), "worker".to_string());
                m
            },
            match_expressions: vec![],
        };

        let eligible = scheduler.node_predicates(&node, &deployment);
        assert!(eligible);
    }

    #[tokio::test]
    async fn test_schedule_node_selector_mismatch() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let node = test_node();
        let mut deployment = test_deployment();
        deployment.node_selector = NodeSelector {
            match_labels: {
                let mut m = HashMap::new();
                m.insert("node-role".to_string(), "master".to_string());
                m
            },
            match_expressions: vec![],
        };

        let eligible = scheduler.node_predicates(&node, &deployment);
        assert!(!eligible);
    }

    #[tokio::test]
    async fn test_schedule_insufficient_resources() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let mut node = test_node();
        node.allocatable.cpu_millicores = 50; // Not enough CPU

        let deployment = test_deployment();

        let eligible = scheduler.node_predicates(&node, &deployment);
        assert!(!eligible);
    }

    #[tokio::test]
    async fn test_schedule_pod_anti_affinity() {
        let store = test_store();
        let scheduler = Scheduler::new(store);

        let node = test_node();
        let deployment = test_deployment();

        // Node with no existing pods
        let empty_pods: Vec<PodActual> = vec![];
        let score_no_pods = scheduler.node_score(&node, &deployment, &empty_pods);

        // Node with existing pods
        let pods_with_existing = vec![PodActual {
            id: "pod-1".to_string(),
            deployment: "other-app".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Running,
            health: HealthStatus::Healthy,
            ip: Some("10.0.0.1".to_string()),
            version: "v1".to_string(),
            socket_path: None,
            node_name: Some("node-1".to_string()),
            created_at: Utc::now(),
            last_health_check: Some(Utc::now()),
            consecutive_failures: 0,
            ready: true,
        }];
        let score_with_pods = scheduler.node_score(&node, &deployment, &pods_with_existing);

        // Node with existing pods should have lower score
        assert!(score_no_pods > score_with_pods);
    }
}
