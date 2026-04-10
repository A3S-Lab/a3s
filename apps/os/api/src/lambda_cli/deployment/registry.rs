//! Service Registry - DNS-based service discovery.
//!
//! Provides pod and service discovery via internal DNS.

use crate::errors::{A3sError, Result};
use crate::state::PodActual;
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::sync::RwLock;

/// Service endpoint - a single pod that can handle requests.
#[derive(Debug, Clone)]
pub struct Endpoint {
    /// Pod ID.
    pub pod_id: String,
    /// Pod IP address.
    pub ip: IpAddr,
    /// Port number.
    pub port: u16,
    /// Whether the endpoint is healthy.
    pub healthy: bool,
    /// Current load (number of active requests).
    pub load: u32,
}

/// Service record - maps a service name to its endpoints.
#[derive(Debug, Clone)]
pub struct ServiceRecord {
    /// Service name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// All endpoints for this service.
    pub endpoints: Vec<Endpoint>,
    /// Created timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Service Registry - maintains a DNS-based service directory.
///
/// Provides service discovery for internal services. When a service is
/// created, it registers a DNS name that resolves to the service's endpoints.
pub struct ServiceRegistry {
    /// Services indexed by namespace/name.
    services: RwLock<HashMap<(String, String), ServiceRecord>>,
    /// Endpoints indexed by pod ID.
    endpoints: RwLock<HashMap<String, Endpoint>>,
}

impl ServiceRegistry {
    /// Create a new service registry.
    pub fn new() -> Self {
        Self {
            services: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(HashMap::new()),
        }
    }

    /// Register a pod as an endpoint for a service.
    pub async fn register_endpoint(
        &self,
        service_name: &str,
        namespace: &str,
        pod: &PodActual,
        port: u16,
    ) -> Result<()> {
        let ip = pod
            .ip
            .as_ref()
            .and_then(|ip_str| ip_str.parse().ok())
            .unwrap_or(IpAddr::from([127, 0, 0, 1]));

        let endpoint = Endpoint {
            pod_id: pod.id.clone(),
            ip,
            port,
            healthy: pod.health == crate::state::HealthStatus::Healthy
                || pod.health == crate::state::HealthStatus::Passthrough,
            load: 0,
        };

        // Add to endpoints index
        self.endpoints
            .write()
            .await
            .insert(pod.id.clone(), endpoint.clone());

        // Add to service record
        let mut services = self.services.write().await;
        let key = (namespace.to_string(), service_name.to_string());
        let record = services
            .entry(key.clone())
            .or_insert_with(|| ServiceRecord {
                name: service_name.to_string(),
                namespace: namespace.to_string(),
                endpoints: Vec::new(),
                created_at: chrono::Utc::now(),
            });

        // Remove any existing endpoint for this pod
        record.endpoints.retain(|e| e.pod_id != pod.id);

        // Add new endpoint
        record.endpoints.push(endpoint);

        tracing::debug!(
            service = %service_name,
            namespace = %namespace,
            pod_id = %pod.id,
            endpoint_count = record.endpoints.len(),
            "endpoint registered"
        );

        Ok(())
    }

    /// Unregister a pod endpoint.
    pub async fn unregister_endpoint(&self, pod_id: &str) -> Result<()> {
        let endpoint = self.endpoints.write().await.remove(pod_id);

        if let Some(_ep) = endpoint {
            // Remove from all services
            let mut services = self.services.write().await;
            for record in services.values_mut() {
                record.endpoints.retain(|e| e.pod_id != pod_id);
            }

            tracing::debug!(
                pod_id = %pod_id,
                "endpoint unregistered"
            );
        }

        Ok(())
    }

    /// Update endpoint health status.
    pub async fn update_health(&self, pod_id: &str, healthy: bool) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;
        if let Some(endpoint) = endpoints.get_mut(pod_id) {
            endpoint.healthy = healthy;
        }

        let mut services = self.services.write().await;
        for record in services.values_mut() {
            for endpoint in record.endpoints.iter_mut() {
                if endpoint.pod_id == pod_id {
                    endpoint.healthy = healthy;
                }
            }
        }

        Ok(())
    }

    /// Get all healthy endpoints for a service (for load balancing).
    pub async fn get_endpoints(
        &self,
        service_name: &str,
        namespace: &str,
    ) -> Result<Vec<Endpoint>> {
        let services = self.services.read().await;
        let key = (namespace.to_string(), service_name.to_string());

        let record = services.get(&key).ok_or_else(|| {
            A3sError::Project(format!(
                "service {} not found in namespace {}",
                service_name, namespace
            ))
        })?;

        Ok(record
            .endpoints
            .iter()
            .filter(|e| e.healthy)
            .cloned()
            .collect())
    }

    /// Get DNS name for a service.
    pub fn dns_name(&self, service_name: &str, namespace: &str) -> String {
        format!("{}.{}.svc.cluster.local", service_name, namespace)
    }

    /// List all registered services.
    pub async fn list_services(&self) -> Vec<ServiceRecord> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }

    /// Get a service record by name and namespace.
    pub async fn get_service(&self, service_name: &str, namespace: &str) -> Option<ServiceRecord> {
        let services = self.services.read().await;
        services
            .get(&(namespace.to_string(), service_name.to_string()))
            .cloned()
    }
}

impl Default for ServiceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::HealthStatus;
    use crate::state::PodStatus;
    use chrono::Utc;

    fn test_pod(id: &str, ip: &str, healthy: bool) -> PodActual {
        PodActual {
            id: id.to_string(),
            deployment: "test".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Running,
            health: if healthy {
                HealthStatus::Healthy
            } else {
                HealthStatus::Unhealthy
            },
            ip: Some(ip.to_string()),
            version: "v1".to_string(),
            socket_path: None,
            node_name: None,
            created_at: Utc::now(),
            last_health_check: Some(Utc::now()),
            consecutive_failures: 0,
            ready: healthy,
        }
    }

    #[tokio::test]
    async fn test_register_and_discover() {
        let registry = ServiceRegistry::new();

        let pod1 = test_pod("pod-1", "10.0.0.1", true);
        let pod2 = test_pod("pod-2", "10.0.0.2", true);

        registry
            .register_endpoint("my-service", "default", &pod1, 8080)
            .await
            .unwrap();
        registry
            .register_endpoint("my-service", "default", &pod2, 8080)
            .await
            .unwrap();

        let endpoints = registry
            .get_endpoints("my-service", "default")
            .await
            .unwrap();
        assert_eq!(endpoints.len(), 2);

        let dns = registry.dns_name("my-service", "default");
        assert_eq!(dns, "my-service.default.svc.cluster.local");
    }

    #[tokio::test]
    async fn test_unregister() {
        let registry = ServiceRegistry::new();

        let pod = test_pod("pod-1", "10.0.0.1", true);
        registry
            .register_endpoint("my-service", "default", &pod, 8080)
            .await
            .unwrap();

        let endpoints = registry
            .get_endpoints("my-service", "default")
            .await
            .unwrap();
        assert_eq!(endpoints.len(), 1);

        registry.unregister_endpoint("pod-1").await.unwrap();

        let endpoints = registry
            .get_endpoints("my-service", "default")
            .await
            .unwrap();
        assert_eq!(endpoints.len(), 0);
    }

    #[tokio::test]
    async fn test_unhealthy_filtering() {
        let registry = ServiceRegistry::new();

        let pod1 = test_pod("pod-1", "10.0.0.1", true);
        let pod2 = test_pod("pod-2", "10.0.0.2", false);

        registry
            .register_endpoint("my-service", "default", &pod1, 8080)
            .await
            .unwrap();
        registry
            .register_endpoint("my-service", "default", &pod2, 8080)
            .await
            .unwrap();

        let endpoints = registry
            .get_endpoints("my-service", "default")
            .await
            .unwrap();
        // Only healthy endpoints should be returned
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].pod_id, "pod-1");
    }
}
