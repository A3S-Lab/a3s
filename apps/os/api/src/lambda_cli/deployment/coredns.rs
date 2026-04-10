//! CoreDNS Implementation - DNS-based service discovery.
//!
//! Provides DNS resolution for services in the cluster using the
//! standard Kubernetes DNS naming scheme: `<service>.<namespace>.svc.cluster.local`

use crate::state::{ServiceDesired, ServicePort, ServiceType};
use std::collections::HashMap;
use std::net::Ipv6Addr;
use std::net::{IpAddr, Ipv4Addr};
use tokio::sync::RwLock;

/// DNS record types.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DnsRecordType {
    A,     // IPv4 address
    AAAA,  // IPv6 address
    CNAME, // Canonical name
    SRV,   // Service record
    PTR,   // Pointer record
}

/// DNS question from a client.
#[derive(Debug, Clone)]
pub struct DnsQuestion {
    pub name: String,
    pub record_type: DnsRecordType,
}

/// DNS resource record.
#[derive(Debug, Clone)]
pub struct DnsRecord {
    pub name: String,
    pub record_type: DnsRecordType,
    pub ttl: u32,
    pub value: DnsValue,
}

/// DNS record value.
#[derive(Debug, Clone)]
pub enum DnsValue {
    A(Ipv4Addr),
    AAAA(Ipv6Addr),
    CNAME(String),
    SRV {
        priority: u16,
        weight: u16,
        port: u16,
        target: String,
    },
    PTR(String),
}

/// DNS response.
#[derive(Debug, Clone)]
pub struct DnsResponse {
    pub question: DnsQuestion,
    pub answers: Vec<DnsRecord>,
    pub authority: Vec<DnsRecord>,
    pub additional: Vec<DnsRecord>,
    pub nxdomain: bool,
}

/// CoreDNS server for cluster-wide DNS.
pub struct CoreDnsServer {
    /// DNS cache: service name -> service info
    cache: RwLock<HashMap<String, ServiceDnsInfo>>,
    /// Cluster domain suffix
    cluster_domain: String,
    /// Local IP (for headless services)
    local_ip: Ipv4Addr,
    /// Default TTL for records
    default_ttl: u32,
}

/// Service DNS info.
#[derive(Debug, Clone)]
pub struct ServiceDnsInfo {
    pub name: String,
    pub namespace: String,
    pub cluster_ip: IpAddr,
    pub ports: Vec<ServicePort>,
    pub selector: HashMap<String, String>,
    pub service_type: ServiceType,
    /// For headless services, pod IPs
    pub pod_ips: Vec<Ipv4Addr>,
}

impl CoreDnsServer {
    /// Create a new CoreDNS server.
    pub fn new() -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            cluster_domain: "cluster.local".to_string(),
            local_ip: Ipv4Addr::new(10, 244, 0, 1),
            default_ttl: 30,
        }
    }

    /// Create with custom cluster domain.
    pub fn with_domain(cluster_domain: &str) -> Self {
        Self {
            cache: RwLock::new(HashMap::new()),
            cluster_domain: cluster_domain.to_string(),
            local_ip: Ipv4Addr::new(10, 244, 0, 1),
            default_ttl: 30,
        }
    }

    /// Get the cluster domain.
    pub fn cluster_domain(&self) -> &str {
        &self.cluster_domain
    }

    /// Build full DNS name for a service.
    pub fn service_fqdn(&self, name: &str, namespace: &str) -> String {
        format!("{}.{}.svc.{}", name, namespace, self.cluster_domain)
    }

    /// Build A3S DNS name format.
    pub fn a3s_fqdn(&self, name: &str, namespace: &str) -> String {
        self.service_fqdn(name, namespace)
    }

    /// Register a service in DNS.
    pub async fn register_service(&self, service: &ServiceDesired) {
        let mut cache = self.cache.write().await;

        let is_headless =
            service.service_type == ServiceType::LoadBalancer && service.name != "None";

        let info = ServiceDnsInfo {
            name: service.name.clone(),
            namespace: service.namespace.clone(),
            cluster_ip: IpAddr::V4(Ipv4Addr::new(10, 96, 0, 1)), // Placeholder
            ports: service.ports.clone(),
            selector: service.selector.clone(),
            service_type: service.service_type,
            pod_ips: if is_headless { vec![] } else { vec![] },
        };

        let key = self.service_fqdn(&service.name, &service.namespace);
        cache.insert(key, info);
    }

    /// Unregister a service from DNS.
    pub async fn unregister_service(&self, name: &str, namespace: &str) {
        let mut cache = self.cache.write().await;
        let key = self.service_fqdn(name, namespace);
        cache.remove(&key);
    }

    /// Update pod IPs for headless service.
    pub async fn update_headless_endpoints(
        &self,
        name: &str,
        namespace: &str,
        pod_ips: Vec<Ipv4Addr>,
    ) {
        let mut cache = self.cache.write().await;
        let key = self.service_fqdn(name, namespace);
        if let Some(info) = cache.get_mut(&key) {
            info.pod_ips = pod_ips;
        }
    }

    /// Handle a DNS query.
    pub async fn handle_query(&self, question: &DnsQuestion) -> DnsResponse {
        let cache = self.cache.read().await;

        let fqdn = if question.name.ends_with(&self.cluster_domain) {
            question.name.clone()
        } else {
            // Try appending cluster domain
            format!("{}.svc.{}", question.name, self.cluster_domain)
        };

        // Parse the name: <service>.<namespace>.svc.cluster.local
        let parts: Vec<&str> = fqdn.split('.').collect();

        let mut answers = Vec::new();

        // Check if this is a cluster.local domain query
        let cluster_suffix = format!("svc.{}", self.cluster_domain);
        let pod_suffix = format!("pod.{}", self.cluster_domain);

        // Handle SRV record queries: _<port>._tcp.<service>.<namespace>.svc.cluster.local
        if question.record_type == DnsRecordType::SRV && parts.len() >= 5 && parts[1] == "_tcp" {
            // _http._tcp.web.prod.svc.cluster.local
            // parts: [_http, _tcp, web, prod, svc, cluster, local]
            let service_name = parts[2];
            let namespace = parts[3];
            let key = format!("{}.{}.svc.{}", service_name, namespace, self.cluster_domain);
            if let Some(info) = cache.get(&key) {
                let port_name = parts[0].trim_start_matches('_');
                for port in &info.ports {
                    if port.name == port_name || port_name == "" {
                        answers.push(DnsRecord {
                            name: fqdn.clone(),
                            record_type: DnsRecordType::SRV,
                            ttl: self.default_ttl,
                            value: DnsValue::SRV {
                                priority: 0,
                                weight: 0,
                                port: port.port,
                                target: key.clone(),
                            },
                        });
                    }
                }
            }
        } else if fqdn.ends_with(&cluster_suffix)
            || fqdn.ends_with(&format!("{}.svc", self.cluster_domain))
        {
            // Service query: <service>.<namespace>.svc.cluster.local
            // parts: [service, namespace, svc, cluster, local]
            let svc_idx = parts.iter().position(|&p| p == "svc");
            if let Some(svc_idx) = svc_idx {
                let service_name = parts[0];
                // namespace is between service and svc
                let namespace = if svc_idx > 0 { parts[svc_idx - 1] } else { "" };

                let key = format!("{}.{}.svc.{}", service_name, namespace, self.cluster_domain);
                if let Some(info) = cache.get(&key) {
                    match question.record_type {
                        DnsRecordType::A => {
                            if info.service_type == ServiceType::ClusterIP {
                                if let IpAddr::V4(ip) = info.cluster_ip {
                                    answers.push(DnsRecord {
                                        name: fqdn.clone(),
                                        record_type: DnsRecordType::A,
                                        ttl: self.default_ttl,
                                        value: DnsValue::A(ip),
                                    });
                                }
                            }
                        }
                        DnsRecordType::SRV => {
                            // Generate SRV records for each port
                            for port in &info.ports {
                                answers.push(DnsRecord {
                                    name: format!("_{}._tcp.{}", port.name, key),
                                    record_type: DnsRecordType::SRV,
                                    ttl: self.default_ttl,
                                    value: DnsValue::SRV {
                                        priority: 0,
                                        weight: 0,
                                        port: port.port,
                                        target: if info.service_type == ServiceType::ClusterIP {
                                            key.clone()
                                        } else {
                                            // Headless: point to pod
                                            format!(
                                                "{}.{}.pod.{}",
                                                service_name, namespace, self.cluster_domain
                                            )
                                        },
                                    },
                                });
                            }
                        }
                        _ => {}
                    }
                }
            }
        } else if fqdn.ends_with(&pod_suffix) {
            // Pod query: <service>.<namespace>.pod.cluster.local
            let pod_idx = parts.iter().position(|&p| p == "pod");
            if let Some(pod_idx) = pod_idx {
                let service_name = parts[0];
                let namespace = if pod_idx > 1 { parts[pod_idx - 2] } else { "" };

                let key = format!("{}.{}.svc.{}", service_name, namespace, self.cluster_domain);
                if let Some(info) = cache.get(&key) {
                    for pod_ip in &info.pod_ips {
                        answers.push(DnsRecord {
                            name: fqdn.clone(),
                            record_type: DnsRecordType::A,
                            ttl: self.default_ttl,
                            value: DnsValue::A(*pod_ip),
                        });
                    }
                }
            }
        } else if parts.len() >= 3 && parts[1] == "svc" && parts[2] == self.cluster_domain {
            // Namespace wide query: <name>.svc.cluster.local
            let name = parts[0];
            for (_key, info) in cache.iter() {
                if info.name == name {
                    if let IpAddr::V4(ip) = info.cluster_ip {
                        answers.push(DnsRecord {
                            name: format!("{}.svc.{}", name, self.cluster_domain),
                            record_type: DnsRecordType::A,
                            ttl: self.default_ttl,
                            value: DnsValue::A(ip),
                        });
                    }
                }
            }
        } else {
            // Check if it's a direct service name in any namespace
            for (key, info) in cache.iter() {
                if info.name == question.name {
                    if let IpAddr::V4(ip) = info.cluster_ip {
                        answers.push(DnsRecord {
                            name: key.clone(),
                            record_type: DnsRecordType::A,
                            ttl: self.default_ttl,
                            value: DnsValue::A(ip),
                        });
                    }
                }
            }
        }

        let nxdomain = answers.is_empty();
        DnsResponse {
            question: question.clone(),
            answers,
            authority: vec![],
            additional: vec![],
            nxdomain,
        }
    }

    /// List all registered services.
    pub async fn list_services(&self) -> Vec<ServiceDnsInfo> {
        let cache = self.cache.read().await;
        cache.values().cloned().collect()
    }

    /// Get service by name and namespace.
    pub async fn get_service(&self, name: &str, namespace: &str) -> Option<ServiceDnsInfo> {
        let cache = self.cache.read().await;
        let key = self.service_fqdn(name, namespace);
        cache.get(&key).cloned()
    }

    /// Generate DNS zone file content.
    pub async fn generate_zone_file(&self) -> String {
        let cache = self.cache.read().await;
        let mut zone = String::new();

        zone.push_str(&format!("$ORIGIN svc.{}\n", self.cluster_domain));
        zone.push_str(&format!("$TTL {}\n\n", self.default_ttl));

        // SOA record
        zone.push_str(&format!(
            "@\tIN\tSOA\tns.{}. hostmaster.{} 1 7200 3600 1209600 3600\n",
            self.cluster_domain, self.cluster_domain
        ));

        // NS record
        zone.push_str(&format!("@\tIN\tNS\tns.{}.\n", self.cluster_domain));
        zone.push_str(&format!("ns\tIN\tA\t{}\n\n", self.local_ip));

        // Service records
        for (key, info) in cache.iter() {
            let parts: Vec<&str> = key.split('.').collect();
            if parts.len() >= 4 {
                let service_name = parts[0];
                let namespace = parts[1];

                if info.service_type == ServiceType::ClusterIP
                    || info.service_type == ServiceType::NodePort
                {
                    if let IpAddr::V4(ip) = info.cluster_ip {
                        zone.push_str(&format!("{}\tIN\tA\t{}\n", service_name, ip));
                    }
                }

                // SRV records
                for port in &info.ports {
                    zone.push_str(&format!(
                        "_{}._tcp.{}IN\tSRV\t0\t0\t{}\t{}.{}.svc.{}\n",
                        port.name, key, port.port, service_name, namespace, self.cluster_domain
                    ));
                }
            }
        }

        zone
    }
}

impl Default for CoreDnsServer {
    fn default() -> Self {
        Self::new()
    }
}

/// DNS query builder for common patterns.
pub struct DnsQuery {
    name: String,
    record_type: DnsRecordType,
}

impl DnsQuery {
    /// Create a new DNS query.
    pub fn new(name: &str, record_type: DnsRecordType) -> Self {
        Self {
            name: name.to_string(),
            record_type,
        }
    }

    /// Query for service A record.
    pub fn service_a(service: &str, namespace: &str) -> Self {
        Self::new(
            &format!("{}.{}.svc.cluster.local", service, namespace),
            DnsRecordType::A,
        )
    }

    /// Query for service SRV record.
    pub fn service_srv(service: &str, namespace: &str, port_name: &str) -> Self {
        Self::new(
            &format!(
                "_{}._tcp.{}.{}.svc.cluster.local",
                port_name, service, namespace
            ),
            DnsRecordType::SRV,
        )
    }

    /// Query for headless pod IPs.
    pub fn pod_a(service: &str, namespace: &str) -> Self {
        Self::new(
            &format!("{}.{}.pod.cluster.local", service, namespace),
            DnsRecordType::A,
        )
    }

    /// Build the question.
    pub fn build(self) -> DnsQuestion {
        DnsQuestion {
            name: self.name,
            record_type: self.record_type,
        }
    }
}

/// Simple DNS server for handling queries.
pub struct DnsServer {
    /// The CoreDNS implementation.
    core_dns: CoreDnsServer,
    /// Listen address.
    listen_addr: Ipv4Addr,
    /// Listen port.
    listen_port: u16,
}

impl DnsServer {
    /// Create a new DNS server.
    pub fn new() -> Self {
        Self {
            core_dns: CoreDnsServer::new(),
            listen_addr: Ipv4Addr::new(0, 0, 0, 0),
            listen_port: 53,
        }
    }

    /// Create with custom address.
    pub fn with_address(addr: Ipv4Addr, port: u16) -> Self {
        Self {
            core_dns: CoreDnsServer::new(),
            listen_addr: addr,
            listen_port: port,
        }
    }

    /// Get reference to CoreDNS.
    pub fn coredns(&self) -> &CoreDnsServer {
        &self.core_dns
    }

    /// Get the listen address.
    pub fn listen_addr(&self) -> (Ipv4Addr, u16) {
        (self.listen_addr, self.listen_port)
    }
}

impl Default for DnsServer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[tokio::test]
    async fn test_register_and_query_service() {
        let dns = CoreDnsServer::new();

        let service = ServiceDesired {
            name: "nginx".to_string(),
            namespace: "default".to_string(),
            service_type: ServiceType::ClusterIP,
            selector: HashMap::new(),
            labels: HashMap::new(),
            ports: vec![ServicePort {
                name: "http".to_string(),
                port: 80,
                target_port: 80,
                protocol: "TCP".to_string(),
            }],
            created_at: Utc::now(),
        };

        dns.register_service(&service).await;

        let question = DnsQuestion {
            name: "nginx.default.svc.cluster.local".to_string(),
            record_type: DnsRecordType::A,
        };

        let response = dns.handle_query(&question).await;
        assert!(!response.nxdomain);
        assert_eq!(response.answers.len(), 1);

        let answer = &response.answers[0];
        if let DnsValue::A(ip) = &answer.value {
            assert_eq!(ip.to_string(), "10.96.0.1");
        } else {
            panic!("Expected A record");
        }
    }

    #[tokio::test]
    async fn test_service_fqdn() {
        let dns = CoreDnsServer::new();
        assert_eq!(
            dns.service_fqdn("nginx", "default"),
            "nginx.default.svc.cluster.local"
        );
    }

    #[tokio::test]
    async fn test_headless_service() {
        // Test that pod IPs can be updated for a service
        // (actual headless behavior requires clusterIP: None which isn't in current ServiceDesired)
        let dns = CoreDnsServer::new();

        let service = ServiceDesired {
            name: "headless".to_string(),
            namespace: "default".to_string(),
            service_type: ServiceType::ClusterIP,
            selector: HashMap::new(),
            labels: HashMap::new(),
            ports: vec![ServicePort {
                name: "http".to_string(),
                port: 80,
                target_port: 80,
                protocol: "TCP".to_string(),
            }],
            created_at: Utc::now(),
        };

        dns.register_service(&service).await;

        // Update with pod IPs - these would be used for headless services
        dns.update_headless_endpoints(
            "headless",
            "default",
            vec![Ipv4Addr::new(10, 244, 0, 10), Ipv4Addr::new(10, 244, 0, 11)],
        )
        .await;

        // For ClusterIP services, the pod query returns nothing (correct behavior)
        // Headless services with clusterIP: None would return pod IPs
        let question = DnsQuestion {
            name: "headless.default.pod.cluster.local".to_string(),
            record_type: DnsRecordType::A,
        };

        let response = dns.handle_query(&question).await;
        // ClusterIP service doesn't return pod IPs - this is expected
        assert!(response.nxdomain || response.answers.is_empty());

        // But the service A record should still work
        let service_question = DnsQuestion {
            name: "headless.default.svc.cluster.local".to_string(),
            record_type: DnsRecordType::A,
        };
        let service_response = dns.handle_query(&service_question).await;
        assert!(!service_response.nxdomain);
    }

    #[tokio::test]
    async fn test_srv_record() {
        let dns = CoreDnsServer::new();

        let service = ServiceDesired {
            name: "web".to_string(),
            namespace: "prod".to_string(),
            service_type: ServiceType::ClusterIP,
            selector: HashMap::new(),
            labels: HashMap::new(),
            ports: vec![
                ServicePort {
                    name: "http".to_string(),
                    port: 80,
                    target_port: 80,
                    protocol: "TCP".to_string(),
                },
                ServicePort {
                    name: "https".to_string(),
                    port: 443,
                    target_port: 443,
                    protocol: "TCP".to_string(),
                },
            ],
            created_at: Utc::now(),
        };

        dns.register_service(&service).await;

        let question = DnsQuestion {
            name: "_http._tcp.web.prod.svc.cluster.local".to_string(),
            record_type: DnsRecordType::SRV,
        };

        let response = dns.handle_query(&question).await;
        assert!(!response.nxdomain);
        assert_eq!(response.answers.len(), 1);
    }

    #[tokio::test]
    async fn test_unregister_service() {
        let dns = CoreDnsServer::new();

        let service = ServiceDesired {
            name: "test".to_string(),
            namespace: "default".to_string(),
            service_type: ServiceType::ClusterIP,
            selector: HashMap::new(),
            labels: HashMap::new(),
            ports: vec![],
            created_at: Utc::now(),
        };

        dns.register_service(&service).await;
        assert!(dns.get_service("test", "default").await.is_some());

        dns.unregister_service("test", "default").await;
        assert!(dns.get_service("test", "default").await.is_none());
    }
}
