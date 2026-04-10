//! NodeLease - Node heartbeat and lease management.
//!
//! NodeLease is used by kubelet to send periodic heartbeats to the API server
//! to indicate node health. This replaces the older node status mechanism.

use crate::errors::Result;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Lease spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseSpec {
    /// Holder identity.
    pub holder_identity: Option<String>,
    /// Lease duration in seconds.
    pub lease_duration_seconds: i32,
    /// Acquire time.
    pub acquire_time: Option<DateTime<Utc>>,
    /// Renew time.
    pub renew_time: Option<DateTime<Utc>>,
    /// Last update time.
    pub last_update_time: Option<DateTime<Utc>>,
    /// Retry count.
    pub lease_transitions: i32,
}

/// Lease object.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lease {
    /// Metadata.
    pub metadata: LeaseMetadata,
    /// Spec.
    pub spec: LeaseSpec,
}

/// Lease metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseMetadata {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// UID.
    pub uid: Option<String>,
    /// Resource version.
    pub resource_version: Option<String>,
    /// Generation.
    pub generation: Option<i64>,
    /// Creation timestamp.
    pub creation_timestamp: DateTime<Utc>,
    /// Labels.
    pub labels: HashMap<String, String>,
}

/// NodeLease controller manages node leases.
pub struct NodeLeaseController {
    /// Leases by node name.
    leases: RwLock<HashMap<String, Lease>>,
    /// Default lease duration in seconds.
    default_lease_duration_secs: i32,
    /// Renew interval in seconds.
    renew_interval_secs: i32,
    /// Max time without renewal before considered dead.
    grace_period_secs: i32,
}

impl NodeLeaseController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            leases: RwLock::new(HashMap::new()),
            default_lease_duration_secs: 40, // Kubernetes default
            renew_interval_secs: 10,
            grace_period_secs: 120,
        }
    }

    /// Create or update a lease for a node.
    pub async fn update_lease(&self, node_name: &str, holder_identity: &str) -> Result<()> {
        let now = Utc::now();
        let mut leases = self.leases.write().await;

        if let Some(lease) = leases.get_mut(node_name) {
            // Update existing lease
            lease.spec.holder_identity = Some(holder_identity.to_string());
            lease.spec.renew_time = Some(now);
            lease.spec.last_update_time = Some(now);
            lease.spec.lease_transitions += 1;
        } else {
            // Create new lease
            let lease = Lease {
                metadata: LeaseMetadata {
                    name: node_name.to_string(),
                    namespace: "default".to_string(),
                    uid: None,
                    resource_version: None,
                    generation: Some(1),
                    creation_timestamp: now,
                    labels: HashMap::new(),
                },
                spec: LeaseSpec {
                    holder_identity: Some(holder_identity.to_string()),
                    lease_duration_seconds: self.default_lease_duration_secs,
                    acquire_time: Some(now),
                    renew_time: Some(now),
                    last_update_time: Some(now),
                    lease_transitions: 0,
                },
            };
            leases.insert(node_name.to_string(), lease);
        }

        Ok(())
    }

    /// Get lease for a node.
    pub async fn get_lease(&self, node_name: &str) -> Option<Lease> {
        let leases = self.leases.read().await;
        leases.get(node_name).cloned()
    }

    /// Check if node is alive (lease is current).
    pub async fn is_node_alive(&self, node_name: &str) -> bool {
        if let Some(lease) = self.get_lease(node_name).await {
            if let Some(renew_time) = lease.spec.renew_time {
                let elapsed = Utc::now() - renew_time;
                elapsed.num_seconds() < self.grace_period_secs as i64
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Get stale nodes (lease not renewed within grace period).
    pub async fn get_stale_nodes(&self) -> Vec<String> {
        let now = Utc::now();
        let leases = self.leases.read().await;
        let grace = Duration::seconds(self.grace_period_secs as i64);

        leases
            .iter()
            .filter(|(_, lease)| {
                if let Some(renew_time) = lease.spec.renew_time {
                    now - renew_time > grace
                } else {
                    true
                }
            })
            .map(|(name, _)| name.clone())
            .collect()
    }

    /// Delete lease for a node.
    pub async fn delete_lease(&self, node_name: &str) -> Result<()> {
        let mut leases = self.leases.write().await;
        leases.remove(node_name);
        Ok(())
    }

    /// List all leases.
    pub async fn list_leases(&self) -> Vec<Lease> {
        let leases = self.leases.read().await;
        leases.values().cloned().collect()
    }

    /// Get default lease duration.
    pub fn default_lease_duration(&self) -> i32 {
        self.default_lease_duration_secs
    }
}

impl Default for NodeLeaseController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_lease_update() {
        let controller = NodeLeaseController::new();

        controller
            .update_lease("node-1", "kubelet-node-1")
            .await
            .unwrap();

        let lease = controller.get_lease("node-1").await;
        assert!(lease.is_some());
        assert_eq!(
            lease.unwrap().spec.holder_identity,
            Some("kubelet-node-1".to_string())
        );
    }

    #[tokio::test]
    async fn test_node_alive() {
        let controller = NodeLeaseController::new();

        controller
            .update_lease("node-1", "kubelet-node-1")
            .await
            .unwrap();

        let alive = controller.is_node_alive("node-1").await;
        assert!(alive);
    }

    #[tokio::test]
    async fn test_stale_nodes() {
        let controller = NodeLeaseController::new();

        // Create a "stale" lease by manually inserting old data
        let now = Utc::now();
        let stale_time = now - Duration::seconds(200);

        let lease = Lease {
            metadata: LeaseMetadata {
                name: "stale-node".to_string(),
                namespace: "default".to_string(),
                uid: None,
                resource_version: None,
                generation: Some(1),
                creation_timestamp: stale_time,
                labels: HashMap::new(),
            },
            spec: LeaseSpec {
                holder_identity: Some("kubelet-stale".to_string()),
                lease_duration_seconds: 40,
                acquire_time: Some(stale_time),
                renew_time: Some(stale_time),
                last_update_time: Some(stale_time),
                lease_transitions: 0,
            },
        };

        {
            let mut leases = controller.leases.write().await;
            leases.insert("stale-node".to_string(), lease);
        }

        let stale = controller.get_stale_nodes().await;
        assert!(stale.contains(&"stale-node".to_string()));
    }
}
