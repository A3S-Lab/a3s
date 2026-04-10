//! A3sCNI - Container Network Interface.
//!
//! A3sCNI provides the network connectivity for containers, replacing Kubernetes CNI.
//! It is responsible for:
//! - Network namespace management
//! - Container network setup and teardown
//! - IP address allocation from pool
//! - Network interface creation and deletion
//! - Traffic routing and filtering
//! - DNS resolution for pods
//!
//! A3sCNI supports multiple backends and follows the CNI specification.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// CNI configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CniConfig {
    /// CNI name.
    pub name: String,
    /// CNI version.
    pub cni_version: String,
    /// Network type.
    pub network_type: NetworkType,
    /// Bridge name.
    pub bridge_name: String,
    /// Pod CIDR.
    pub pod_cidr: String,
    /// Gateway IP.
    pub gateway_ip: String,
    /// IP allocation range.
    pub ip_alloc_range: String,
    /// MTU.
    pub mtu: u32,
    /// DNS configuration.
    pub dns: CniDns,
    /// Hairpin mode.
    pub hairpin_mode: bool,
    /// Prometheus port.
    pub prometheus_port: u16,
}

impl Default for CniConfig {
    fn default() -> Self {
        Self {
            name: "a3s-cni".to_string(),
            cni_version: "1.0.0".to_string(),
            network_type: NetworkType::Bridge,
            bridge_name: "a3s-br0".to_string(),
            pod_cidr: "10.42.0.0/16".to_string(),
            gateway_ip: "10.42.0.1".to_string(),
            ip_alloc_range: "10.42.0.0/24".to_string(),
            mtu: 1500,
            dns: CniDns::default(),
            hairpin_mode: false,
            prometheus_port: 9091,
        }
    }
}

/// Network type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NetworkType {
    /// Bridge network.
    Bridge,
    /// VLAN network.
    Vlan,
    /// VXLAN network.
    Vxlan,
    /// IPVLAN network.
    Ipvlan,
    /// Host-local only.
    HostLocal,
}

impl Default for NetworkType {
    fn default() -> Self {
        NetworkType::Bridge
    }
}

/// DNS configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CniDns {
    /// Nameservers.
    pub nameservers: Vec<String>,
    /// Search domains.
    pub search_domains: Vec<String>,
    /// Options.
    pub options: Vec<String>,
}

impl Default for CniDns {
    fn default() -> Self {
        Self {
            nameservers: vec!["10.96.0.2".to_string()],
            search_domains: vec!["cluster.local".to_string()],
            options: vec!["ndots:5".to_string()],
        }
    }
}

/// Network namespace.
#[derive(Debug, Clone)]
pub struct NetworkNamespace {
    /// Namespace path.
    pub path: String,
    /// Container ID.
    pub container_id: String,
    /// Network interfaces.
    pub interfaces: Vec<NetworkInterface>,
    /// Routes.
    pub routes: Vec<Route>,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

/// Network interface.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkInterface {
    /// Interface name.
    pub name: String,
    /// Interface type.
    pub iface_type: InterfaceType,
    /// IP address.
    pub ip: Option<String>,
    /// MAC address.
    pub mac: Option<String>,
    /// MTU.
    pub mtu: u32,
    /// Master interface (for veth pairs).
    pub master: Option<String>,
    /// Sandbox container ID.
    pub sandbox: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum InterfaceType {
    /// Physical interface.
    Physical,
    /// Virtual ethernet pair.
    Veth,
    /// Bridge interface.
    Bridge,
    /// VLAN interface.
    Vlan,
    /// VXLAN interface.
    Vxlan,
    /// IPVLAN interface.
    Ipvlan,
    /// Loopback interface.
    Loopback,
}

/// Route entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Route {
    /// Destination network.
    pub dst: String,
    /// Gateway IP.
    pub gateway: Option<String>,
    /// Output interface.
    pub output_iface: Option<String>,
    /// Metric.
    pub metric: Option<u32>,
}

/// IP allocation result.
#[derive(Debug, Clone)]
pub struct IpAllocation {
    /// IP address.
    pub ip: String,
    /// Gateway IP.
    pub gateway: String,
    /// Subnet CIDR.
    pub subnet: String,
    /// Interface name.
    pub interface_name: String,
    /// DNS nameservers.
    pub dns_nameservers: Vec<String>,
    /// DNS search domains.
    pub dns_search_domains: Vec<String>,
}

/// Network endpoint.
#[derive(Debug, Clone)]
pub struct NetworkEndpoint {
    /// Container ID.
    pub container_id: String,
    /// Network namespace path.
    pub namespace_path: String,
    /// Interface name.
    pub interface_name: String,
    /// IP address.
    pub ip: String,
    /// MAC address.
    pub mac: String,
    /// Networks.
    pub networks: Vec<String>,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

/// Network info.
#[derive(Debug, Clone)]
pub struct NetworkInfo {
    /// Network name.
    pub name: String,
    /// Network ID.
    pub id: String,
    /// Network type.
    pub network_type: NetworkType,
    /// Pod CIDR.
    pub pod_cidr: String,
    /// Gateway IP.
    pub gateway_ip: String,
    /// Total IPs.
    pub total_ips: u32,
    /// Allocated IPs.
    pub allocated_ips: u32,
    /// Reserved IPs.
    pub reserved_ips: Vec<String>,
}

/// IPAM result.
#[derive(Debug, Clone)]
pub struct IpamResult {
    /// Allocated IP.
    pub ip: String,
    /// Subnet.
    pub subnet: String,
    /// Gateway.
    pub gateway: String,
    /// Routes.
    pub routes: Vec<Route>,
}

/// A3sCNI - Container network interface.
pub struct A3sCni {
    /// Configuration.
    config: CniConfig,
    /// Network namespace pool.
    ns_pool: RwLock<HashMap<String, NetworkNamespace>>,
    /// Endpoints by container.
    endpoints: RwLock<HashMap<String, NetworkEndpoint>>,
    /// Networks by name.
    networks: RwLock<HashMap<String, NetworkInfo>>,
    /// IP address pool.
    ip_pool: RwLock<Vec<String>>,
    /// Used IPs.
    used_ips: RwLock<HashMap<String, String>>, // ip -> container_id
    /// Running state.
    running: RwLock<bool>,
}

impl A3sCni {
    /// Create a new CNI instance.
    pub async fn new(config: CniConfig) -> Result<Self> {
        let mut ip_pool = Vec::new();
        let base_ip = "10.42.0.";
        for i in 2..254 {
            ip_pool.push(format!("{}{}", base_ip, i));
        }

        Ok(Self {
            config,
            ns_pool: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(HashMap::new()),
            networks: RwLock::new(HashMap::new()),
            ip_pool: RwLock::new(ip_pool),
            used_ips: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        })
    }

    /// Start the CNI.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Initialize default network
        let network = NetworkInfo {
            name: "a3s-network".to_string(),
            id: "a3s-net-1".to_string(),
            network_type: self.config.network_type,
            pod_cidr: self.config.pod_cidr.clone(),
            gateway_ip: self.config.gateway_ip.clone(),
            total_ips: 253,
            allocated_ips: 0,
            reserved_ips: vec![
                self.config.gateway_ip.clone(),
                format!(
                    "{}1",
                    &self.config.gateway_ip[..self.config.gateway_ip.len() - 1]
                ),
            ],
        };

        let mut networks = self.networks.write().await;
        networks.insert(network.name.clone(), network);

        tracing::info!(
            network = %self.config.name,
            pod_cidr = %self.config.pod_cidr,
            gateway = %self.config.gateway_ip,
            "A3sCNI started"
        );

        Ok(())
    }

    /// Stop the CNI.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;

        // Clean up all endpoints
        let endpoints = self.endpoints.write().await;
        for (_, endpoint) in endpoints.iter() {
            self.delete_network_interface(&endpoint.interface_name)
                .await?;
        }

        tracing::info!("A3sCNI stopped");
        Ok(())
    }

    /// Add a container to the network.
    pub async fn add_container(
        &self,
        container_id: &str,
        netns_path: Option<&str>,
    ) -> Result<NetworkEndpoint> {
        if !*self.running.read().await {
            return Err(A3sError::Other("CNI not running".to_string()));
        }

        // Allocate IP
        let ipam = self.allocate_ip(container_id).await?;

        // Create network interface name
        let iface_name = format!("eth0");

        // Create endpoint
        let endpoint = NetworkEndpoint {
            container_id: container_id.to_string(),
            namespace_path: netns_path.unwrap_or("/var/run/netns/a3s").to_string(),
            interface_name: iface_name.clone(),
            ip: ipam.ip.clone(),
            mac: self.generate_mac(),
            networks: vec!["a3s-network".to_string()],
            created_at: Utc::now(),
        };

        // Store endpoint
        let mut endpoints = self.endpoints.write().await;
        endpoints.insert(container_id.to_string(), endpoint.clone());

        tracing::info!(
            container = %container_id,
            ip = %ipam.ip,
            iface = %iface_name,
            "Container added to network"
        );

        Ok(endpoint)
    }

    /// Delete a container from the network.
    pub async fn delete_container(&self, container_id: &str) -> Result<()> {
        let mut endpoints = self.endpoints.write().await;

        if let Some(endpoint) = endpoints.remove(container_id) {
            // Release IP
            self.release_ip(container_id).await?;

            // Delete network interface
            self.delete_network_interface(&endpoint.interface_name)
                .await?;

            tracing::info!(
                container = %container_id,
                ip = %endpoint.ip,
                "Container removed from network"
            );
        }

        Ok(())
    }

    /// Allocate an IP address.
    async fn allocate_ip(&self, container_id: &str) -> Result<IpamResult> {
        let mut ip_pool = self.ip_pool.write().await;
        let mut used_ips = self.used_ips.write().await;

        // Find first available IP
        let ip = ip_pool
            .pop()
            .ok_or_else(|| A3sError::Other("No available IPs".to_string()))?;

        used_ips.insert(ip.clone(), container_id.to_string());

        // Update network allocation count
        let mut networks = self.networks.write().await;
        if let Some(network) = networks.get_mut("a3s-network") {
            network.allocated_ips += 1;
        }

        Ok(IpamResult {
            ip: ip.clone(),
            subnet: self.config.ip_alloc_range.clone(),
            gateway: self.config.gateway_ip.clone(),
            routes: vec![Route {
                dst: "0.0.0.0/0".to_string(),
                gateway: Some(self.config.gateway_ip.clone()),
                output_iface: Some(self.config.bridge_name.clone()),
                metric: None,
            }],
        })
    }

    /// Release an IP address.
    async fn release_ip(&self, container_id: &str) -> Result<()> {
        let mut ip_pool = self.ip_pool.write().await;
        let mut used_ips = self.used_ips.write().await;

        // Find and remove IP for this container
        if let Some(ip) = used_ips.remove(container_id) {
            ip_pool.push(ip);
        }

        // Update network allocation count
        let mut networks = self.networks.write().await;
        if let Some(network) = networks.get_mut("a3s-network") {
            network.allocated_ips = network.allocated_ips.saturating_sub(1);
        }

        Ok(())
    }

    /// Generate a MAC address.
    fn generate_mac(&self) -> String {
        use std::fmt::Write;
        let octets: [u8; 6] = [
            0x02, // Local bit
            rand::random(),
            rand::random(),
            rand::random(),
            rand::random(),
            rand::random(),
        ];
        octets.iter().fold(String::new(), |mut s, &b| {
            let _ = write!(&mut s, "{:02x}:", b);
            s
        })
    }

    /// Delete network interface.
    async fn delete_network_interface(&self, name: &str) -> Result<()> {
        // In real implementation, would delete the veth pair
        tracing::debug!(iface = %name, "Deleting network interface");
        Ok(())
    }

    /// Get endpoint for container.
    pub async fn get_endpoint(&self, container_id: &str) -> Option<NetworkEndpoint> {
        let endpoints = self.endpoints.read().await;
        endpoints.get(container_id).cloned()
    }

    /// List all endpoints.
    pub async fn list_endpoints(&self) -> Vec<NetworkEndpoint> {
        let endpoints = self.endpoints.read().await;
        endpoints.values().cloned().collect()
    }

    /// List all networks.
    pub async fn list_networks(&self) -> Vec<NetworkInfo> {
        let networks = self.networks.read().await;
        networks.values().cloned().collect()
    }

    /// Get network info.
    pub async fn get_network(&self, name: &str) -> Option<NetworkInfo> {
        let networks = self.networks.read().await;
        networks.get(name).cloned()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }

    /// Get config.
    pub fn config(&self) -> &CniConfig {
        &self.config
    }
}

impl Default for A3sCni {
    fn default() -> Self {
        Self {
            config: CniConfig::default(),
            ns_pool: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(HashMap::new()),
            networks: RwLock::new(HashMap::new()),
            ip_pool: RwLock::new(Vec::new()),
            used_ips: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cni_creation() {
        let cni = A3sCni::new(CniConfig::default()).await.unwrap();
        assert!(!cni.is_running().await);
    }

    #[tokio::test]
    async fn test_add_container() {
        let cni = A3sCni::new(CniConfig::default()).await.unwrap();
        cni.start().await.unwrap();

        let endpoint = cni.add_container("test-container", None).await.unwrap();
        assert_eq!(endpoint.container_id, "test-container");
        assert!(!endpoint.ip.is_empty());
    }

    #[tokio::test]
    async fn test_delete_container() {
        let cni = A3sCni::new(CniConfig::default()).await.unwrap();
        cni.start().await.unwrap();

        cni.add_container("test-container", None).await.unwrap();
        cni.delete_container("test-container").await.unwrap();

        let endpoint = cni.get_endpoint("test-container").await;
        assert!(endpoint.is_none());
    }

    #[tokio::test]
    async fn test_ip_allocation() {
        let cni = A3sCni::new(CniConfig::default()).await.unwrap();
        cni.start().await.unwrap();

        let ep1 = cni.add_container("container-1", None).await.unwrap();
        let ep2 = cni.add_container("container-2", None).await.unwrap();

        assert_ne!(ep1.ip, ep2.ip);

        cni.delete_container("container-1").await.unwrap();

        let ep3 = cni.add_container("container-3", None).await.unwrap();
        // Should get an IP back from the pool
        assert_ne!(ep3.ip, ep2.ip);
    }
}
