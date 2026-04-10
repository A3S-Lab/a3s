//! EndpointSlice Controller - Service Endpoint Management.
//!
//! EndpointSlice manages network endpoints for services, replacing Kubernetes EndpointSlice.
//! It is responsible for:
//! - Tracking pod IP addresses for service backends
//! - EndpointSlice CRUD operations
//! - Endpoint topology (az, hostname, etc.)
//! - Slice management (batching multiple endpoints)
//! - Auto-cleanup of stale endpoints

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Endpoint address type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AddressType {
    /// IPv4 address.
    IPv4,
    /// IPv6 address.
    IPv6,
    /// Fully Qualified Domain Name.
    Fqdn,
}

/// Endpoint address.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointAddress {
    /// Address type.
    pub address_type: AddressType,
    /// IP address or hostname.
    pub ip: String,
    /// Target hostname (for FQDN).
    pub target_ref: Option<EndpointTargetRef>,
    /// Node name.
    pub node_name: Option<String>,
    /// Zone.
    pub zone: Option<String>,
    /// Topology.
    pub topology: HashMap<String, String>,
    /// Hostname.
    pub hostname: Option<String>,
    /// Resolver name.
    pub resolver_name: Option<String>,
    /// Is ready.
    pub is_ready: bool,
    /// Is serving.
    pub is_serving: bool,
    /// Is terminating.
    pub is_terminating: bool,
}

/// Target reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointTargetRef {
    /// Kind (Pod).
    pub kind: String,
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
}

/// Port mapping.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointPort {
    /// Port name.
    pub name: Option<String>,
    /// Port number.
    pub port: i32,
    /// Protocol.
    pub protocol: String,
    /// App protocol.
    pub app_protocol: Option<String>,
}

/// EndpointSlice represents a subset of network endpoints for a service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSlice {
    /// Object metadata.
    pub metadata: EndpointSliceMeta,
    /// Address type.
    pub address_type: AddressType,
    /// Endpoints.
    pub endpoints: Vec<Endpoint>,
    /// Ports.
    pub ports: Vec<EndpointPort>,
}

/// Endpoint with addresses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Endpoint {
    /// Addresses.
    pub addresses: Vec<EndpointAddress>,
    /// Conditions.
    pub conditions: EndpointConditions,
    /// Target ref.
    pub target_ref: Option<EndpointTargetRef>,
    /// Zone.
    pub zone: Option<String>,
    /// Hints.
    pub hints: Option<EndpointHints>,
}

/// Endpoint conditions.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct EndpointConditions {
    /// Ready.
    pub ready: bool,
    /// Serving.
    pub serving: bool,
    /// Terminating.
    pub terminating: bool,
}

/// Endpoint hints.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointHints {
    /// For zones.
    pub for_zones: Vec<String>,
}

/// EndpointSlice metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSliceMeta {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Labels.
    pub labels: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// EndpointSlice desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointSliceDesired {
    /// Service name.
    pub service_name: String,
    /// Namespace.
    pub namespace: String,
    /// Slice name.
    pub slice_name: String,
    /// Address type.
    pub address_type: AddressType,
    /// Pod endpoints.
    pub pod_endpoints: Vec<PodEndpoint>,
    /// Service ports.
    pub service_ports: Vec<EndpointPort>,
}

/// Pod endpoint info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodEndpoint {
    /// Pod name.
    pub pod_name: String,
    /// Namespace.
    pub namespace: String,
    /// Pod IP.
    pub pod_ip: String,
    /// Node name.
    pub node_name: String,
    /// Ready.
    pub ready: bool,
    /// Serving.
    pub serving: bool,
    /// Terminating.
    pub terminating: bool,
    /// Zone.
    pub zone: Option<String>,
    /// Hostname.
    pub hostname: Option<String>,
}

/// EndpointSlice controller.
pub struct EndpointSliceController {
    /// EndpointSlices by service.
    slices: RwLock<HashMap<String, HashMap<String, EndpointSlice>>>,
    /// Max endpoints per slice.
    max_endpoints_per_slice: usize,
}

impl EndpointSliceController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            slices: RwLock::new(HashMap::new()),
            max_endpoints_per_slice: 100,
        }
    }

    /// Update endpoints for a service.
    pub async fn update_service_endpoints(
        &self,
        service_name: &str,
        namespace: &str,
        pod_endpoints: Vec<PodEndpoint>,
        service_ports: Vec<EndpointPort>,
    ) -> Result<()> {
        let key = format!("{}/{}", namespace, service_name);

        // Filter to ready endpoints only
        let ready_endpoints: Vec<_> = pod_endpoints.into_iter().filter(|ep| ep.ready).collect();

        // Build endpoints with addresses
        let endpoints: Vec<Endpoint> = ready_endpoints
            .iter()
            .map(|ep| Endpoint {
                addresses: vec![EndpointAddress {
                    address_type: AddressType::IPv4,
                    ip: ep.pod_ip.clone(),
                    target_ref: Some(EndpointTargetRef {
                        kind: "Pod".to_string(),
                        namespace: ep.namespace.clone(),
                        name: ep.pod_name.clone(),
                        uid: format!("uid-{}", ep.pod_name),
                    }),
                    node_name: Some(ep.node_name.clone()),
                    zone: ep.zone.clone(),
                    topology: {
                        let mut t = HashMap::new();
                        if let Some(ref zone) = ep.zone {
                            t.insert("topology.kubernetes.io/zone".to_string(), zone.clone());
                        }
                        t.insert("kubernetes.io/hostname".to_string(), ep.node_name.clone());
                        t
                    },
                    hostname: ep.hostname.clone(),
                    resolver_name: None,
                    is_ready: ep.ready,
                    is_serving: ep.serving,
                    is_terminating: ep.terminating,
                }],
                conditions: EndpointConditions {
                    ready: ep.ready,
                    serving: ep.serving,
                    terminating: ep.terminating,
                },
                target_ref: Some(EndpointTargetRef {
                    kind: "Pod".to_string(),
                    namespace: ep.namespace.clone(),
                    name: ep.pod_name.clone(),
                    uid: format!("uid-{}", ep.pod_name),
                }),
                zone: ep.zone.clone(),
                hints: None,
            })
            .collect();

        // Batch into slices
        let slices = self.batch_into_slices(namespace, service_name, endpoints, service_ports);

        // Store slices
        let mut all_slices = self.slices.write().await;
        all_slices.insert(key.clone(), slices);

        tracing::debug!(
            service = %service_name,
            namespace = %namespace,
            total_endpoints = ready_endpoints.len(),
            "Endpoints updated"
        );

        Ok(())
    }

    /// Batch endpoints into slices.
    fn batch_into_slices(
        &self,
        namespace: &str,
        service_name: &str,
        endpoints: Vec<Endpoint>,
        ports: Vec<EndpointPort>,
    ) -> HashMap<String, EndpointSlice> {
        let mut slices = HashMap::new();
        let total = endpoints.len();
        let max = self.max_endpoints_per_slice;

        for (i, chunk) in endpoints.chunks(max).enumerate() {
            let slice_name = if i == 0 {
                format!("{}-{}", service_name, "xxxx")
            } else {
                format!("{}-{}-{}", service_name, "xxxx", i)
            };

            let mut labels = HashMap::new();
            labels.insert(
                "kubernetes.io/service-name".to_string(),
                service_name.to_string(),
            );
            if total > max {
                labels.insert(
                    "endpointslice.kubernetes.io/managed-by".to_string(),
                    "a3s-controller".to_string(),
                );
            }

            let slice = EndpointSlice {
                metadata: EndpointSliceMeta {
                    name: slice_name.clone(),
                    namespace: namespace.to_string(),
                    labels,
                    created_at: Utc::now(),
                    updated_at: Utc::now(),
                },
                address_type: AddressType::IPv4,
                endpoints: chunk.to_vec(),
                ports: ports.clone(),
            };

            slices.insert(slice_name, slice);
        }

        slices
    }

    /// Get EndpointSlices for a service.
    pub async fn get_service_slices(
        &self,
        service_name: &str,
        namespace: &str,
    ) -> Vec<EndpointSlice> {
        let key = format!("{}/{}", namespace, service_name);
        let slices = self.slices.read().await;
        slices
            .get(&key)
            .map(|m| m.values().cloned().collect())
            .unwrap_or_default()
    }

    /// Get total endpoint count for a service.
    pub async fn get_endpoint_count(&self, service_name: &str, namespace: &str) -> usize {
        let slices = self.get_service_slices(service_name, namespace).await;
        slices.iter().map(|s| s.endpoints.len()).sum()
    }

    /// Delete all slices for a service.
    pub async fn delete_service_slices(&self, service_name: &str, namespace: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, service_name);
        let mut slices = self.slices.write().await;
        slices.remove(&key);

        tracing::debug!(
            service = %service_name,
            namespace = %namespace,
            "EndpointSlices deleted"
        );

        Ok(())
    }

    /// List all slices.
    pub async fn list_all_slices(&self) -> Vec<EndpointSlice> {
        let slices = self.slices.read().await;
        slices.values().flat_map(|m| m.values().cloned()).collect()
    }
}

impl Default for EndpointSliceController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_update_endpoints() {
        let controller = EndpointSliceController::new();

        let pod_endpoints = vec![
            PodEndpoint {
                pod_name: "pod-1".to_string(),
                namespace: "default".to_string(),
                pod_ip: "10.0.0.1".to_string(),
                node_name: "node-1".to_string(),
                ready: true,
                serving: true,
                terminating: false,
                zone: Some("us-east-1a".to_string()),
                hostname: None,
            },
            PodEndpoint {
                pod_name: "pod-2".to_string(),
                namespace: "default".to_string(),
                pod_ip: "10.0.0.2".to_string(),
                node_name: "node-2".to_string(),
                ready: true,
                serving: true,
                terminating: false,
                zone: Some("us-east-1b".to_string()),
                hostname: None,
            },
        ];

        let service_ports = vec![EndpointPort {
            name: Some("http".to_string()),
            port: 80,
            protocol: "TCP".to_string(),
            app_protocol: Some("http".to_string()),
        }];

        controller
            .update_service_endpoints("my-service", "default", pod_endpoints, service_ports)
            .await
            .unwrap();

        let slices = controller.get_service_slices("my-service", "default").await;
        assert!(!slices.is_empty());
        assert_eq!(slices.iter().map(|s| s.endpoints.len()).sum::<usize>(), 2);
    }

    #[tokio::test]
    async fn test_delete_slices() {
        let controller = EndpointSliceController::new();

        let pod_endpoints = vec![PodEndpoint {
            pod_name: "pod-1".to_string(),
            namespace: "default".to_string(),
            pod_ip: "10.0.0.1".to_string(),
            node_name: "node-1".to_string(),
            ready: true,
            serving: true,
            terminating: false,
            zone: None,
            hostname: None,
        }];

        controller
            .update_service_endpoints("my-service", "default", pod_endpoints, vec![])
            .await
            .unwrap();

        controller
            .delete_service_slices("my-service", "default")
            .await
            .unwrap();

        let slices = controller.get_service_slices("my-service", "default").await;
        assert!(slices.is_empty());
    }

    #[tokio::test]
    async fn test_endpoint_filtering() {
        let controller = EndpointSliceController::new();

        let pod_endpoints = vec![
            PodEndpoint {
                pod_name: "pod-1".to_string(),
                namespace: "default".to_string(),
                pod_ip: "10.0.0.1".to_string(),
                node_name: "node-1".to_string(),
                ready: true,
                serving: true,
                terminating: false,
                zone: None,
                hostname: None,
            },
            PodEndpoint {
                pod_name: "pod-2".to_string(),
                namespace: "default".to_string(),
                pod_ip: "10.0.0.2".to_string(),
                node_name: "node-2".to_string(),
                ready: false, // Not ready
                serving: false,
                terminating: false,
                zone: None,
                hostname: None,
            },
        ];

        controller
            .update_service_endpoints("my-service", "default", pod_endpoints, vec![])
            .await
            .unwrap();

        let slices = controller.get_service_slices("my-service", "default").await;
        let total = slices.iter().map(|s| s.endpoints.len()).sum::<usize>();
        assert_eq!(total, 1); // Only the ready endpoint
    }
}
