//! Health check interface for distributed deployment
//!
//! This module provides the `HealthCheck` trait for monitoring
//! node health in a distributed cluster.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime};
use tokio::sync::{broadcast, RwLock};

use super::{HealthConfig, Result};

/// Health check trait
///
/// Implementations provide health monitoring and reporting for nodes
/// in a distributed cluster.
///
/// # Example
///
/// ```rust,ignore
/// use a3s_lambda::distributed::HealthCheck;
///
/// // Check node health
/// let health = checker.check_health("node-1").await?;
///
/// if health.status == HealthStatus::Healthy {
///     println!("Node is healthy");
/// } else {
///     println!("Node is unhealthy: {:?}", health.issues);
/// }
/// ```
#[async_trait]
pub trait HealthCheck: Send + Sync {
    /// Check health of a specific node
    async fn check_health(&self, node_id: &str) -> Result<NodeHealth>;

    /// Check health of all nodes
    async fn check_all(&self) -> Result<Vec<NodeHealth>>;

    /// Report health status of this node
    async fn report_health(&self, node_id: &str, health: NodeHealth) -> Result<()>;

    /// Get health history for a node
    async fn get_history(&self, node_id: &str, limit: usize) -> Result<Vec<NodeHealth>>;

    /// Subscribe to health change events
    async fn subscribe(&self) -> Result<Box<dyn HealthWatcher>>;
}

/// Node health information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NodeHealth {
    /// Node identifier
    pub node_id: String,

    /// Overall health status
    pub status: HealthStatus,

    /// Individual component checks
    pub checks: HashMap<String, ComponentHealth>,

    /// Health issues (if any)
    pub issues: Vec<HealthIssue>,

    /// Timestamp of health check
    pub timestamp: SystemTime,
}

/// Health status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// All checks passed
    Healthy,

    /// Some non-critical checks failed
    Degraded,

    /// Critical checks failed
    Unhealthy,

    /// Node is not responding
    Unknown,
}

/// Component health check result
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ComponentHealth {
    /// Component name (e.g., "vm_pool", "queue", "storage")
    pub name: String,

    /// Component status
    pub status: HealthStatus,

    /// Check duration in milliseconds
    pub duration_ms: u64,

    /// Additional details
    pub details: HashMap<String, String>,
}

/// Health issue
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthIssue {
    /// Issue severity
    pub severity: IssueSeverity,

    /// Component that reported the issue
    pub component: String,

    /// Issue description
    pub message: String,

    /// Timestamp when issue was detected
    pub detected_at: SystemTime,
}

/// Issue severity level
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IssueSeverity {
    /// Informational (no action needed)
    Info,

    /// Warning (should be investigated)
    Warning,

    /// Error (requires attention)
    Error,

    /// Critical (immediate action required)
    Critical,
}

/// Health change event
#[derive(Debug, Clone)]
pub enum HealthEvent {
    /// Node became healthy
    Healthy(String),

    /// Node became degraded
    Degraded(String, Vec<HealthIssue>),

    /// Node became unhealthy
    Unhealthy(String, Vec<HealthIssue>),

    /// Node status unknown
    Unknown(String),
}

/// Health watcher for monitoring changes
#[async_trait]
pub trait HealthWatcher: Send + Sync {
    /// Get the next health event
    async fn next(&mut self) -> Option<HealthEvent>;

    /// Close the watcher
    async fn close(&mut self);
}

/// No-op health checker (single node)
pub struct NoOpHealthCheck;

#[async_trait]
impl HealthCheck for NoOpHealthCheck {
    async fn check_health(&self, node_id: &str) -> Result<NodeHealth> {
        Ok(NodeHealth {
            node_id: node_id.to_string(),
            status: HealthStatus::Healthy,
            checks: HashMap::new(),
            issues: vec![],
            timestamp: SystemTime::now(),
        })
    }

    async fn check_all(&self) -> Result<Vec<NodeHealth>> {
        Ok(vec![])
    }

    async fn report_health(&self, _node_id: &str, _health: NodeHealth) -> Result<()> {
        Ok(())
    }

    async fn get_history(&self, _node_id: &str, _limit: usize) -> Result<Vec<NodeHealth>> {
        Ok(vec![])
    }

    async fn subscribe(&self) -> Result<Box<dyn HealthWatcher>> {
        Ok(Box::new(NoOpHealthWatcher))
    }
}

struct NoOpHealthWatcher;

#[async_trait]
impl HealthWatcher for NoOpHealthWatcher {
    async fn next(&mut self) -> Option<HealthEvent> {
        None
    }

    async fn close(&mut self) {}
}

/// Node entry in the distributed health monitor.
struct NodeEntry {
    health: NodeHealth,
    last_seen: Instant,
    history: Vec<NodeHealth>,
}

/// Distributed health monitor - tracks health of multiple nodes in a cluster.
///
/// This monitor maintains:
/// - Node registry with heartbeat tracking
/// - Health history with configurable TTL
/// - Health event broadcasting
/// - Component-level health checks
pub struct DistributedHealthMonitor {
    /// Node entries indexed by node ID.
    nodes: RwLock<HashMap<String, NodeEntry>>,
    /// Health event broadcaster.
    event_tx: broadcast::Sender<HealthEvent>,
    /// Configuration.
    #[allow(dead_code)]
    config: HealthConfig,
    /// Maximum history entries per node.
    max_history: usize,
    /// TTL for node health entries.
    node_ttl: Duration,
}

impl DistributedHealthMonitor {
    /// Create a new distributed health monitor.
    pub fn new(config: HealthConfig) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            nodes: RwLock::new(HashMap::new()),
            event_tx,
            config,
            max_history: 100,
            node_ttl: Duration::from_secs(60),
        }
    }

    /// Create with default configuration.
    pub fn default_monitor() -> Self {
        Self::new(HealthConfig::default())
    }

    /// Register a node and start tracking its health.
    pub async fn register_node(&self, node_id: String) -> Result<()> {
        let mut nodes = self.nodes.write().await;

        if nodes.contains_key(&node_id) {
            return Ok(()); // Already registered
        }

        let entry = NodeEntry {
            health: healthy_node(node_id.clone()),
            last_seen: Instant::now(),
            history: Vec::new(),
        };

        nodes.insert(node_id.clone(), entry);

        // Broadcast node registration
        let _ = self.event_tx.send(HealthEvent::Healthy(node_id));

        Ok(())
    }

    /// Unregister a node.
    pub async fn unregister_node(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;
        nodes.remove(node_id);
        Ok(())
    }

    /// Update health status for a node.
    pub async fn update_health(&self, node_id: &str, health: NodeHealth) -> Result<()> {
        let mut nodes = self.nodes.write().await;

        let entry = nodes.get_mut(node_id).ok_or_else(|| {
            super::HealthError::Internal(format!("node '{}' not registered", node_id))
        })?;

        // Check if status changed
        let old_status = entry.health.status;
        entry.last_seen = Instant::now();

        // Add to history
        entry.history.push(entry.health.clone());
        if entry.history.len() > self.max_history {
            entry.history.remove(0);
        }

        // Update current health
        entry.health = health.clone();

        // Broadcast status change
        if old_status != health.status {
            let event = match health.status {
                HealthStatus::Healthy => HealthEvent::Healthy(node_id.to_string()),
                HealthStatus::Degraded => {
                    HealthEvent::Degraded(node_id.to_string(), health.issues.clone())
                }
                HealthStatus::Unhealthy => {
                    HealthEvent::Unhealthy(node_id.to_string(), health.issues.clone())
                }
                HealthStatus::Unknown => HealthEvent::Unknown(node_id.to_string()),
            };
            let _ = self.event_tx.send(event);
        }

        Ok(())
    }

    /// Record a heartbeat from a node.
    pub async fn record_heartbeat(&self, node_id: &str) -> Result<()> {
        let mut nodes = self.nodes.write().await;

        if let Some(entry) = nodes.get_mut(node_id) {
            entry.last_seen = Instant::now();
        }

        Ok(())
    }

    /// Get nodes that haven't sent a heartbeat within the TTL.
    pub async fn get_stale_nodes(&self) -> Vec<String> {
        let nodes = self.nodes.read().await;
        let now = Instant::now();

        nodes
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_seen) > self.node_ttl)
            .map(|(id, _)| id.clone())
            .collect()
    }

    /// Check health of all registered nodes.
    pub async fn check_all_nodes(&self) -> Vec<NodeHealth> {
        let nodes = self.nodes.read().await;
        let now = Instant::now();

        nodes
            .iter()
            .filter(|(_, entry)| now.duration_since(entry.last_seen) <= self.node_ttl)
            .map(|(_, entry)| entry.health.clone())
            .collect()
    }

    /// Get overall cluster health.
    pub async fn cluster_health(&self) -> ClusterHealth {
        let nodes = self.nodes.read().await;

        if nodes.is_empty() {
            return ClusterHealth {
                status: HealthStatus::Unknown,
                total_nodes: 0,
                healthy_nodes: 0,
                degraded_nodes: 0,
                unhealthy_nodes: 0,
                unknown_nodes: 0,
            };
        }

        let mut healthy = 0;
        let mut degraded = 0;
        let mut unhealthy = 0;
        let mut unknown = 0;

        for (_, entry) in nodes.iter() {
            match entry.health.status {
                HealthStatus::Healthy => healthy += 1,
                HealthStatus::Degraded => degraded += 1,
                HealthStatus::Unhealthy => unhealthy += 1,
                HealthStatus::Unknown => unknown += 1,
            }
        }

        let total = nodes.len();
        let status = if unhealthy > 0 {
            HealthStatus::Unhealthy
        } else if degraded > 0 {
            HealthStatus::Degraded
        } else if healthy == total {
            HealthStatus::Healthy
        } else {
            HealthStatus::Unknown
        };

        ClusterHealth {
            status,
            total_nodes: total,
            healthy_nodes: healthy,
            degraded_nodes: degraded,
            unhealthy_nodes: unhealthy,
            unknown_nodes: unknown,
        }
    }

    /// Get event receiver for health changes.
    pub fn subscribe(&self) -> broadcast::Receiver<HealthEvent> {
        self.event_tx.subscribe()
    }
}

#[async_trait]
impl HealthCheck for DistributedHealthMonitor {
    async fn check_health(&self, node_id: &str) -> Result<NodeHealth> {
        let nodes = self.nodes.read().await;
        nodes
            .get(node_id)
            .map(|e| e.health.clone())
            .ok_or_else(|| super::HealthError::Internal(format!("node '{}' not found", node_id)))
    }

    async fn check_all(&self) -> Result<Vec<NodeHealth>> {
        Ok(self.check_all_nodes().await)
    }

    async fn report_health(&self, node_id: &str, health: NodeHealth) -> Result<()> {
        // First ensure node is registered
        {
            let nodes = self.nodes.read().await;
            if !nodes.contains_key(node_id) {
                drop(nodes);
                self.register_node(node_id.to_string()).await?;
            }
        }
        self.update_health(node_id, health).await
    }

    async fn get_history(&self, node_id: &str, limit: usize) -> Result<Vec<NodeHealth>> {
        let nodes = self.nodes.read().await;
        let entry = nodes
            .get(node_id)
            .ok_or_else(|| super::HealthError::Internal(format!("node '{}' not found", node_id)))?;

        let start = entry.history.len().saturating_sub(limit);
        Ok(entry.history[start..].to_vec())
    }

    async fn subscribe(&self) -> Result<Box<dyn HealthWatcher>> {
        Ok(Box::new(DistributedHealthWatcher {
            receiver: self.event_tx.subscribe(),
        }))
    }
}

/// Health watcher implementation for distributed monitor.
struct DistributedHealthWatcher {
    receiver: broadcast::Receiver<HealthEvent>,
}

#[async_trait]
impl HealthWatcher for DistributedHealthWatcher {
    async fn next(&mut self) -> Option<HealthEvent> {
        self.receiver.recv().await.ok()
    }

    async fn close(&mut self) {
        // Broadcast channel doesn't need explicit close
    }
}

/// Cluster health summary.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterHealth {
    /// Overall cluster status.
    pub status: HealthStatus,
    /// Total number of nodes.
    pub total_nodes: usize,
    /// Number of healthy nodes.
    pub healthy_nodes: usize,
    /// Number of degraded nodes.
    pub degraded_nodes: usize,
    /// Number of unhealthy nodes.
    pub unhealthy_nodes: usize,
    /// Number of unknown nodes.
    pub unknown_nodes: usize,
}

/// Helper to create a healthy node health report
pub fn healthy_node(node_id: impl Into<String>) -> NodeHealth {
    NodeHealth {
        node_id: node_id.into(),
        status: HealthStatus::Healthy,
        checks: HashMap::new(),
        issues: vec![],
        timestamp: SystemTime::now(),
    }
}

/// Helper to create an unhealthy node health report
pub fn unhealthy_node(node_id: impl Into<String>, issues: Vec<HealthIssue>) -> NodeHealth {
    NodeHealth {
        node_id: node_id.into(),
        status: HealthStatus::Unhealthy,
        checks: HashMap::new(),
        issues,
        timestamp: SystemTime::now(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_noop_health_check() {
        let checker = NoOpHealthCheck;

        let health = checker.check_health("test-node").await.unwrap();
        assert_eq!(health.status, HealthStatus::Healthy);
        assert!(health.issues.is_empty());
    }

    #[test]
    fn test_health_helpers() {
        let healthy = healthy_node("node-1");
        assert_eq!(healthy.status, HealthStatus::Healthy);

        let unhealthy = unhealthy_node(
            "node-2",
            vec![HealthIssue {
                severity: IssueSeverity::Critical,
                component: "vm_pool".to_string(),
                message: "Pool exhausted".to_string(),
                detected_at: SystemTime::now(),
            }],
        );
        assert_eq!(unhealthy.status, HealthStatus::Unhealthy);
        assert_eq!(unhealthy.issues.len(), 1);
    }
}
