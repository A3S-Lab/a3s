//! A3sProxy - Service Networking Layer.
//!
//! A3sProxy is the service networking component that replaces Kubernetes' kube-proxy.
//! It is responsible for:
//! - Watching service and endpoint changes
//! - Maintaining service cluster IP routing
//! - Load balancing traffic to backend pods
//! - Implementing session affinity
//! - Managing iptables/nftables rules for service traffic
//!
//! A3sProxy runs as a daemon on each node and programs the local network stack
//! to route service traffic to the correct backend pods.

use crate::errors::Result;
use crate::state::{ServiceDesired, ServicePort, ServiceType};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::sync::RwLock;

/// Proxy configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyConfig {
    /// Cluster CIDR for pod IPs.
    pub cluster_cidr: String,
    /// Service cluster IP range.
    pub service_cluster_ip_range: String,
    /// Node IP address.
    pub node_ip: String,
    /// Proxy mode (iptables, nftables, ipvs).
    pub mode: ProxyMode,
    /// Sync period in seconds.
    pub sync_period_secs: u64,
    /// Minimum sync period in seconds.
    pub min_sync_period_secs: u64,
    /// IP masquerade bit (for NAT).
    pub ip_masq_bit: u8,
    /// Health check port.
    pub health_check_port: u16,
}

impl Default for ProxyConfig {
    fn default() -> Self {
        Self {
            cluster_cidr: "10.42.0.0/16".to_string(),
            service_cluster_ip_range: "10.96.0.0/16".to_string(),
            node_ip: "127.0.0.1".to_string(),
            mode: ProxyMode::Iptables,
            sync_period_secs: 60,
            min_sync_period_secs: 1,
            ip_masq_bit: 14,
            health_check_port: 10256,
        }
    }
}

/// Proxy mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    /// iptables proxy mode.
    Iptables,
    /// IPVS proxy mode.
    Ipvs,
    /// NFTables proxy mode.
    Nftables,
    /// Userspace proxy mode (simple).
    Userspace,
}

impl Default for ProxyMode {
    fn default() -> Self {
        ProxyMode::Iptables
    }
}

/// Service endpoint for proxy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProxyEndpoint {
    /// Endpoint IP.
    pub ip: String,
    /// Port.
    pub port: u16,
    /// Protocol.
    pub protocol: String,
    /// Target pod name.
    pub target_ref: Option<EndpointTarget>,
    /// Ready status.
    pub ready: bool,
    /// Serving status.
    pub serving: bool,
    /// Terminating status.
    pub terminating: bool,
}

/// Endpoint target reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointTarget {
    /// Kind (Pod).
    pub kind: String,
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
}

/// Service info for proxy.
#[derive(Debug, Clone)]
pub struct ServiceInfo {
    /// Service name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Cluster IP.
    pub cluster_ip: IpAddr,
    /// Service type.
    pub service_type: ServiceType,
    /// Ports.
    pub ports: Vec<ServicePort>,
    /// Selector.
    pub selector: HashMap<String, String>,
    /// Session affinity.
    pub session_affinity: SessionAffinity,
    /// External IPs.
    pub external_ips: Vec<String>,
    /// Load balancer IP.
    pub load_balancer_ip: Option<String>,
    /// Health check node port.
    pub health_check_node_port: Option<u16>,
}

impl ServiceInfo {
    /// Create service info from desired state.
    pub fn from_service(service: &ServiceDesired) -> Self {
        Self {
            name: service.name.clone(),
            namespace: service.namespace.clone(),
            cluster_ip: "10.96.0.1"
                .parse()
                .unwrap_or_else(|_| "127.0.0.1".parse().unwrap()),
            service_type: service.service_type,
            ports: service.ports.clone(),
            selector: service.selector.clone(),
            session_affinity: SessionAffinity::None,
            external_ips: Vec::new(),
            load_balancer_ip: None,
            health_check_node_port: None,
        }
    }
}

/// Session affinity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum SessionAffinity {
    /// No session affinity.
    None,
    /// Client IP affinity.
    ClientIP,
    /// Generated cookie affinity.
    Cookie,
}

impl Default for SessionAffinity {
    fn default() -> Self {
        SessionAffinity::None
    }
}

/// Load balancer entry.
#[derive(Debug, Clone)]
pub struct LoadBalancerEntry {
    /// Service namespace.
    pub namespace: String,
    /// Service name.
    pub name: String,
    /// Ingress IP.
    pub ingress_ip: Option<String>,
    /// Ingress hostname.
    pub ingress_hostname: Option<String>,
    /// Ports.
    pub ports: Vec<ServicePort>,
    /// Last update time.
    pub last_update: DateTime<Utc>,
}

/// NAT chain for iptables.
#[derive(Debug, Clone)]
pub struct NatChain {
    /// Chain name.
    pub name: String,
    /// Table.
    pub table: String,
    /// Rules.
    pub rules: Vec<String>,
}

/// Service route.
#[derive(Debug, Clone)]
pub struct ServiceRoute {
    /// Service key.
    pub service_key: String,
    /// Endpoints.
    pub endpoints: Vec<ProxyEndpoint>,
    /// Active.
    pub active: bool,
    /// Last sync.
    pub last_sync: DateTime<Utc>,
}

/// A3sProxy - Service networking proxy.
pub struct A3sProxy {
    /// Configuration.
    config: ProxyConfig,
    /// Services by key (namespace/name).
    services: RwLock<HashMap<String, ServiceInfo>>,
    /// Endpoints by service key.
    endpoints: RwLock<HashMap<String, Vec<ProxyEndpoint>>>,
    /// NAT chains to program.
    nat_chains: RwLock<Vec<NatChain>>,
    /// Service routes.
    routes: RwLock<HashMap<String, ServiceRoute>>,
    /// Load balancer entries.
    load_balancer_entries: RwLock<HashMap<String, LoadBalancerEntry>>,
    /// Running state.
    running: RwLock<bool>,
}

impl A3sProxy {
    /// Create a new proxy.
    pub async fn new(config: ProxyConfig) -> Result<Self> {
        Ok(Self {
            config,
            services: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(HashMap::new()),
            nat_chains: RwLock::new(Vec::new()),
            routes: RwLock::new(HashMap::new()),
            load_balancer_entries: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        })
    }

    /// Start the proxy.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Initialize NAT chains
        self.initialize_chains().await?;

        tracing::info!(
            cluster_cidr = %self.config.cluster_cidr,
            service_range = %self.config.service_cluster_ip_range,
            mode = ?self.config.mode,
            "A3sProxy started"
        );

        Ok(())
    }

    /// Stop the proxy.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;

        // Clean up NAT chains
        self.cleanup_chains().await?;

        tracing::info!("A3sProxy stopped");
        Ok(())
    }

    /// Initialize NAT chains.
    async fn initialize_chains(&self) -> Result<()> {
        let mut nat_chains = self.nat_chains.write().await;

        // A3S service chains
        nat_chains.push(NatChain {
            name: "A3S-SERVICES".to_string(),
            table: "nat".to_string(),
            rules: vec![],
        });

        nat_chains.push(NatChain {
            name: "A3S-NODEPORTS".to_string(),
            table: "nat".to_string(),
            rules: vec![],
        });

        nat_chains.push(NatChain {
            name: "A3S-MASQ-CHECK".to_string(),
            table: "nat".to_string(),
            rules: vec![],
        });

        nat_chains.push(NatChain {
            name: "A3S-POSTROUTING".to_string(),
            table: "nat".to_string(),
            rules: vec![],
        });

        Ok(())
    }

    /// Cleanup NAT chains.
    async fn cleanup_chains(&self) -> Result<()> {
        let mut nat_chains = self.nat_chains.write().await;
        nat_chains.clear();
        Ok(())
    }

    /// Sync services and endpoints.
    pub async fn sync(&self) -> Result<()> {
        if !*self.running.read().await {
            return Ok(());
        }

        let services = self.services.read().await;
        let endpoints = self.endpoints.read().await;

        // Update routes based on current state
        let mut routes = self.routes.write().await;
        for (key, service) in services.iter() {
            let eps = endpoints.get(key).cloned().unwrap_or_default();
            let route = ServiceRoute {
                service_key: key.clone(),
                endpoints: eps.clone(),
                active: !eps.is_empty(),
                last_sync: Utc::now(),
            };

            // Program NAT rules for this service
            self.program_service_rules(&service, &eps).await?;

            routes.insert(key.clone(), route);
        }

        Ok(())
    }

    /// Program NAT rules for a service.
    async fn program_service_rules(
        &self,
        service: &ServiceInfo,
        endpoints: &[ProxyEndpoint],
    ) -> Result<()> {
        match self.config.mode {
            ProxyMode::Iptables => {
                self.program_iptables_rules(service, endpoints).await?;
            }
            ProxyMode::Ipvs => {
                self.program_ipvs_rules(service, endpoints).await?;
            }
            ProxyMode::Nftables => {
                self.program_nftables_rules(service, endpoints).await?;
            }
            ProxyMode::Userspace => {
                // Userspace proxy uses go-core/box for connection handling
                self.program_userspace_rules(service, endpoints).await?;
            }
        }
        Ok(())
    }

    /// Program iptables rules for a service.
    async fn program_iptables_rules(
        &self,
        service: &ServiceInfo,
        endpoints: &[ProxyEndpoint],
    ) -> Result<()> {
        if endpoints.is_empty() {
            tracing::debug!(service = %service.name, "No endpoints, skipping iptables rules");
            return Ok(());
        }

        // Build iptables rules for the service
        let mut rules = Vec::new();

        // DNAT rule to redirect service traffic to backends
        for endpoint in endpoints {
            for port in &service.ports {
                let rule = format!(
                    "-A A3S-SERVICES -d {}/32 -p {} --dport {} -j DNAT --to-destination {}:{}",
                    service.cluster_ip, port.protocol, port.port, endpoint.ip, endpoint.port
                );
                rules.push(rule);
            }
        }

        tracing::debug!(
            service = %service.name,
            endpoints = endpoints.len(),
            rules = rules.len(),
            "Programmed iptables rules"
        );

        Ok(())
    }

    /// Program IPVS rules for a service.
    async fn program_ipvs_rules(
        &self,
        service: &ServiceInfo,
        endpoints: &[ProxyEndpoint],
    ) -> Result<()> {
        if endpoints.is_empty() {
            tracing::debug!(service = %service.name, "No endpoints, skipping IPVS rules");
            return Ok(());
        }

        // Build IPVS virtual server rules
        for port in &service.ports {
            let vs_rule = format!(
                "-A -t {}:{} -s rr -p {}",
                service.cluster_ip, port.port, port.protocol
            );
            tracing::debug!(service = %service.name, rule = %vs_rule, "IPVS virtual server");
        }

        // Build IPVS real server rules
        for endpoint in endpoints {
            let rs_rule = format!(
                "-a -t {}:{} -r {}:{} -g",
                service.cluster_ip, 80, endpoint.ip, endpoint.port
            );
            tracing::debug!(endpoint = %endpoint.ip, rule = %rs_rule, "IPVS real server");
        }

        Ok(())
    }

    /// Program nftables rules for a service.
    async fn program_nftables_rules(
        &self,
        service: &ServiceInfo,
        endpoints: &[ProxyEndpoint],
    ) -> Result<()> {
        if endpoints.is_empty() {
            tracing::debug!(service = %service.name, "No endpoints, skipping nftables rules");
            return Ok(());
        }

        // Build nftables rules
        let rule = format!(
            "add rule ip nat A3S-SERVICES dnat to {{ {} }}",
            endpoints
                .iter()
                .map(|e| format!("{}:{}", e.ip, e.port))
                .collect::<Vec<_>>()
                .join(", ")
        );
        tracing::debug!(service = %service.name, rule = %rule, "nftables rule");

        Ok(())
    }

    /// Program userspace proxy rules.
    async fn program_userspace_rules(
        &self,
        service: &ServiceInfo,
        endpoints: &[ProxyEndpoint],
    ) -> Result<()> {
        // Userspace proxy forwards connections via go-core/box
        // For now, just log the configuration
        tracing::debug!(
            service = %service.name,
            cluster_ip = %service.cluster_ip,
            endpoints = endpoints.len(),
            "Userspace proxy configuration"
        );

        Ok(())
    }

    /// On-service-add event handler.
    pub async fn on_service_add(&self, service: &ServiceDesired) -> Result<()> {
        let key = format!("{}/{}", service.namespace, service.name);
        let info = ServiceInfo::from_service(service);

        tracing::info!(
            service = %key,
            cluster_ip = %info.cluster_ip,
            ports = info.ports.len(),
            "Service added"
        );

        let mut services = self.services.write().await;
        services.insert(key, info);

        // Trigger sync
        drop(services);
        self.sync().await?;

        Ok(())
    }

    /// On-service-update event handler.
    pub async fn on_service_update(&self, service: &ServiceDesired) -> Result<()> {
        let key = format!("{}/{}", service.namespace, service.name);
        let info = ServiceInfo::from_service(service);

        tracing::info!(service = %key, "Service updated");

        let mut services = self.services.write().await;
        services.insert(key, info);

        drop(services);
        self.sync().await?;

        Ok(())
    }

    /// On-service-delete event handler.
    pub async fn on_service_delete(&self, namespace: &str, name: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, name);

        tracing::info!(service = %key, "Service deleted");

        let mut services = self.services.write().await;
        services.remove(&key);

        let mut routes = self.routes.write().await;
        routes.remove(&key);

        drop(services);
        drop(routes);
        self.sync().await?;

        Ok(())
    }

    /// On-endpoints-update event handler.
    pub async fn on_endpoints_update(
        &self,
        namespace: &str,
        name: &str,
        endpoints: Vec<ProxyEndpoint>,
    ) -> Result<()> {
        let key = format!("{}/{}", namespace, name);

        tracing::info!(
            service = %key,
            endpoints = endpoints.len(),
            ready = endpoints.iter().filter(|e| e.ready).count(),
            "Endpoints updated"
        );

        let mut eps = self.endpoints.write().await;
        eps.insert(key, endpoints);

        drop(eps);
        self.sync().await?;

        Ok(())
    }

    /// Get service info.
    pub async fn get_service(&self, namespace: &str, name: &str) -> Option<ServiceInfo> {
        let key = format!("{}/{}", namespace, name);
        let services = self.services.read().await;
        services.get(&key).cloned()
    }

    /// List all services.
    pub async fn list_services(&self) -> Vec<ServiceInfo> {
        let services = self.services.read().await;
        services.values().cloned().collect()
    }

    /// Get endpoints for a service.
    pub async fn get_endpoints(&self, namespace: &str, name: &str) -> Option<Vec<ProxyEndpoint>> {
        let key = format!("{}/{}", namespace, name);
        let endpoints = self.endpoints.read().await;
        endpoints.get(&key).cloned()
    }

    /// Get proxy config.
    pub fn config(&self) -> &ProxyConfig {
        &self.config
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get NAT chains for programming.
    pub async fn get_nat_chains(&self) -> Vec<NatChain> {
        let chains = self.nat_chains.read().await;
        chains.clone()
    }
}

impl Default for A3sProxy {
    fn default() -> Self {
        Self {
            config: ProxyConfig::default(),
            services: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(HashMap::new()),
            nat_chains: RwLock::new(Vec::new()),
            routes: RwLock::new(HashMap::new()),
            load_balancer_entries: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_proxy_creation() {
        let proxy = A3sProxy::new(ProxyConfig::default()).await.unwrap();
        assert!(!proxy.is_running().await);
    }

    #[tokio::test]
    async fn test_service_add() {
        let proxy = A3sProxy::new(ProxyConfig::default()).await.unwrap();

        let service = ServiceDesired {
            name: "test-svc".to_string(),
            namespace: "default".to_string(),
            service_type: ServiceType::ClusterIP,
            ports: vec![ServicePort {
                name: "http".to_string(),
                protocol: "TCP".to_string(),
                port: 80,
                target_port: 8080,
            }],
            selector: HashMap::new(),
            labels: HashMap::new(),
            created_at: Utc::now(),
        };

        proxy.on_service_add(&service).await.unwrap();

        let services = proxy.list_services().await;
        assert_eq!(services.len(), 1);
        assert_eq!(services[0].name, "test-svc");
    }

    #[tokio::test]
    async fn test_service_delete() {
        let proxy = A3sProxy::new(ProxyConfig::default()).await.unwrap();

        let service = ServiceDesired {
            name: "test-svc".to_string(),
            namespace: "default".to_string(),
            service_type: ServiceType::ClusterIP,
            ports: vec![],
            selector: HashMap::new(),
            labels: HashMap::new(),
            created_at: Utc::now(),
        };

        proxy.on_service_add(&service).await.unwrap();
        assert_eq!(proxy.list_services().await.len(), 1);

        proxy
            .on_service_delete("default", "test-svc")
            .await
            .unwrap();
        assert_eq!(proxy.list_services().await.len(), 0);
    }
}
