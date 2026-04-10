//! Ingress Controller - HTTP routing for external traffic.
//!
//! Handles incoming HTTP requests and routes them to appropriate services
//! based on host and path rules.

use crate::deployment::loadbalancer::{LoadBalancer, LoadBalancingStrategy};
use crate::deployment::registry::ServiceRegistry;
use crate::errors::{A3sError, Result};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// HTTP route rule - matches requests to a backend service.
#[derive(Debug, Clone)]
pub struct IngressRule {
    /// Host to match (e.g., "example.com" or "*" for any).
    pub host: String,
    /// HTTP paths and their backends.
    pub paths: Vec<IngressPath>,
}

/// A single path rule.
#[derive(Debug, Clone)]
pub struct IngressPath {
    /// Path prefix to match (e.g., "/api" or "/").
    pub path: String,
    /// Service name to forward to.
    pub backend: String,
    /// Backend port.
    pub port: u16,
}

/// Ingress specification.
#[derive(Debug, Clone, Default)]
pub struct IngressSpec {
    /// Ingress rules.
    pub rules: Vec<IngressRule>,
    /// TLS configuration (placeholder for future).
    #[allow(dead_code)]
    pub tls: Vec<IngressTls>,
}

/// TLS configuration.
#[derive(Debug, Clone, Default)]
pub struct IngressTls {
    /// Hosts to terminate TLS for.
    pub hosts: Vec<String>,
    /// Secret name containing TLS cert/key.
    pub secret_name: String,
}

/// An active HTTP request context.
#[derive(Debug)]
pub struct HttpRequest {
    /// HTTP method.
    pub method: String,
    /// Request path.
    pub path: String,
    /// Host header.
    pub host: String,
    /// Client IP address.
    pub client_ip: Option<std::net::IpAddr>,
    /// Headers.
    pub headers: HashMap<String, String>,
}

/// An HTTP response.
#[derive(Debug)]
pub struct HttpResponse {
    /// Status code.
    pub status: u16,
    /// Response body.
    pub body: Vec<u8>,
    /// Headers.
    pub headers: HashMap<String, String>,
}

/// Ingress controller - routes external HTTP traffic to internal services.
pub struct IngressController {
    /// Registered ingress rules.
    rules: RwLock<Vec<IngressSpec>>,
    /// Service registry for endpoint discovery.
    registry: Arc<ServiceRegistry>,
    /// Load balancer.
    load_balancer: LoadBalancer,
}

impl IngressController {
    /// Create a new ingress controller.
    pub fn new(registry: Arc<ServiceRegistry>) -> Self {
        Self {
            rules: RwLock::new(Vec::new()),
            registry,
            load_balancer: LoadBalancer::new(LoadBalancingStrategy::RoundRobin),
        }
    }

    /// Add an ingress rule.
    pub async fn add_ingress(&self, spec: IngressSpec) -> Result<()> {
        let mut rules = self.rules.write().await;
        rules.push(spec);
        Ok(())
    }

    /// Clear all ingress rules.
    pub async fn clear_ingress(&self) -> Result<()> {
        let mut rules = self.rules.write().await;
        rules.clear();
        Ok(())
    }

    /// Route an HTTP request to the appropriate backend.
    pub async fn route(&self, request: &HttpRequest) -> Result<HttpResponse> {
        let rules = self.rules.read().await;

        // Find matching rule
        for spec in rules.iter() {
            for rule in &spec.rules {
                // Check host matches
                if !self.host_matches(&request.host, &rule.host) {
                    continue;
                }

                // Find matching path
                for path_rule in &rule.paths {
                    if self.path_matches(&request.path, &path_rule.path) {
                        return self
                            .forward_to_backend(&path_rule.backend, path_rule.port, request)
                            .await;
                    }
                }
            }
        }

        // No matching rule found
        Err(A3sError::Project(format!(
            "no ingress rule matched request {} {}",
            request.method, request.path
        )))
    }

    /// Check if a host matches a rule host pattern.
    fn host_matches(&self, request_host: &str, rule_host: &str) -> bool {
        if rule_host == "*" {
            return true;
        }
        if rule_host.starts_with("*.") {
            let suffix = &rule_host[1..];
            return request_host.ends_with(suffix) || request_host == &rule_host[2..];
        }
        request_host == rule_host
    }

    /// Check if a path matches a rule path pattern.
    fn path_matches(&self, request_path: &str, rule_path: &str) -> bool {
        if rule_path == "/" {
            return true;
        }
        if rule_path.ends_with('/') {
            return request_path.starts_with(rule_path);
        }
        request_path == rule_path || request_path.starts_with(&format!("{}/", rule_path))
    }

    /// Forward request to a backend service.
    async fn forward_to_backend(
        &self,
        service_name: &str,
        _port: u16,
        request: &HttpRequest,
    ) -> Result<HttpResponse> {
        // Get healthy endpoints
        let endpoints = self.registry.get_endpoints(service_name, "default").await?;

        if endpoints.is_empty() {
            return Err(A3sError::Project(format!(
                "no healthy endpoints for service {}",
                service_name
            )));
        }

        // Select endpoint using load balancer
        let endpoint = self
            .load_balancer
            .select(&endpoints, request.client_ip)
            .ok_or_else(|| A3sError::Project("no endpoint selected".to_string()))?;

        tracing::debug!(
            service = %service_name,
            pod_id = %endpoint.pod_id,
            ip = %endpoint.ip,
            "forwarding request to backend"
        );

        // Build the target URL
        let target_url = format!("http://{}:{}{}", endpoint.ip, endpoint.port, request.path);

        // Make the HTTP request
        let client = reqwest::Client::new();
        let mut req_builder = client.request(
            reqwest::Method::from_bytes(request.method.as_bytes()).unwrap_or(reqwest::Method::GET),
            &target_url,
        );

        // Add headers
        for (key, value) in &request.headers {
            if key.to_lowercase() != "host" {
                req_builder = req_builder.header(key, value);
            }
        }

        // Send request
        let response = req_builder
            .send()
            .await
            .map_err(|e| A3sError::Project(format!("failed to forward request: {}", e)))?;

        let status = response.status().as_u16();

        // Collect headers before consuming response
        let mut headers = HashMap::new();
        for (key, value) in response.headers() {
            if let Ok(v) = value.to_str() {
                headers.insert(key.to_string(), v.to_string());
            }
        }

        let body = response
            .bytes()
            .await
            .map_err(|e| A3sError::Project(format!("failed to read response body: {}", e)))?;

        Ok(HttpResponse {
            status,
            body: body.to_vec(),
            headers,
        })
    }

    /// Get the DNS name for a service (convenience method).
    pub fn service_dns_name(&self, name: &str) -> String {
        self.registry.dns_name(name, "default")
    }
}

impl Default for IngressController {
    fn default() -> Self {
        Self::new(Arc::new(ServiceRegistry::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_request(path: &str, host: &str) -> HttpRequest {
        HttpRequest {
            method: "GET".to_string(),
            path: path.to_string(),
            host: host.to_string(),
            client_ip: Some(std::net::IpAddr::from([192, 168, 1, 1])),
            headers: HashMap::new(),
        }
    }

    #[test]
    fn test_host_matching() {
        let controller = IngressController::default();

        // Exact match
        assert!(controller.host_matches("example.com", "example.com"));

        // Wildcard subdomain
        assert!(controller.host_matches("api.example.com", "*.example.com"));
        assert!(controller.host_matches("example.com", "*.example.com"));

        // Any host
        assert!(controller.host_matches("anything.com", "*"));
    }

    #[test]
    fn test_path_matching() {
        let controller = IngressController::default();

        // Exact match
        assert!(controller.path_matches("/api/users", "/api/users"));

        // Root matches everything
        assert!(controller.path_matches("/anything", "/"));

        // Prefix matching with trailing slash
        assert!(controller.path_matches("/api/users/123", "/api/users/"));

        // Non-matching
        assert!(!controller.path_matches("/api", "/api/users"));
    }

    #[tokio::test]
    async fn test_no_matching_rule() {
        let registry = Arc::new(ServiceRegistry::new());
        let controller = IngressController::new(registry);

        let request = make_request("/unknown", "example.com");
        let result = controller.route(&request).await;

        assert!(result.is_err());
    }
}
