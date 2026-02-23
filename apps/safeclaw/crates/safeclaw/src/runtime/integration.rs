//! Service discovery for a3s-gateway
//!
//! SafeClaw exposes a well-known endpoint that a3s-gateway can poll
//! to discover this backend service. This replaces the old inverted
//! pattern where SafeClaw generated config for the gateway.

use serde::Serialize;

/// Service descriptor returned by `GET /.well-known/a3s-service.json`
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ServiceDescriptor {
    /// Service name
    pub name: String,
    /// Service version
    pub version: String,
    /// Service type (e.g., "agent-backend")
    pub service_type: String,
    /// Available endpoints
    pub endpoints: Vec<EndpointDescriptor>,
    /// Service capabilities
    pub capabilities: Vec<String>,
    /// Health check URL path
    pub health_check: String,
}

/// Describes a single endpoint this service exposes
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EndpointDescriptor {
    /// Endpoint path
    pub path: String,
    /// HTTP method(s)
    pub methods: Vec<String>,
    /// Protocol (http, ws)
    pub protocol: String,
    /// Human-readable description
    pub description: String,
}

/// Build the service descriptor for this SafeClaw instance
pub fn build_service_descriptor() -> ServiceDescriptor {
    ServiceDescriptor {
        name: "safeclaw".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        service_type: "agent-backend".to_string(),
        endpoints: vec![
            EndpointDescriptor {
                path: "/health".to_string(),
                methods: vec!["GET".to_string()],
                protocol: "http".to_string(),
                description: "Health check".to_string(),
            },
            EndpointDescriptor {
                path: "/status".to_string(),
                methods: vec!["GET".to_string()],
                protocol: "http".to_string(),
                description: "Service status".to_string(),
            },
            EndpointDescriptor {
                path: "/sessions".to_string(),
                methods: vec!["GET".to_string()],
                protocol: "http".to_string(),
                description: "List active sessions".to_string(),
            },
            EndpointDescriptor {
                path: "/message".to_string(),
                methods: vec!["POST".to_string()],
                protocol: "http".to_string(),
                description: "Send a message to a channel".to_string(),
            },
            EndpointDescriptor {
                path: "/api/agent/ws".to_string(),
                methods: vec!["GET".to_string()],
                protocol: "ws".to_string(),
                description: "Agent WebSocket connection".to_string(),
            },
            EndpointDescriptor {
                path: "/api/events".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                protocol: "http".to_string(),
                description: "Event management".to_string(),
            },
            EndpointDescriptor {
                path: "/api/personas".to_string(),
                methods: vec!["GET".to_string(), "POST".to_string()],
                protocol: "http".to_string(),
                description: "Persona management".to_string(),
            },
            EndpointDescriptor {
                path: "/api/settings".to_string(),
                methods: vec!["GET".to_string(), "PUT".to_string()],
                protocol: "http".to_string(),
                description: "Settings management".to_string(),
            },
        ],
        capabilities: vec![
            "multi-agent".to_string(),
            "privacy-classification".to_string(),
            "tee-support".to_string(),
            "multi-channel".to_string(),
            "streaming".to_string(),
        ],
        health_check: "/health".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_build_service_descriptor() {
        let desc = build_service_descriptor();
        assert_eq!(desc.name, "safeclaw");
        assert_eq!(desc.service_type, "agent-backend");
        assert!(!desc.endpoints.is_empty());
        assert!(!desc.capabilities.is_empty());
        assert_eq!(desc.health_check, "/health");
    }

    #[test]
    fn test_service_descriptor_serialization() {
        let desc = build_service_descriptor();
        let json = serde_json::to_string_pretty(&desc).unwrap();
        assert!(json.contains("safeclaw"));
        assert!(json.contains("agentBackend") || json.contains("agent-backend"));

        // Verify camelCase serialization
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value.get("serviceName").is_some() || value.get("serviceType").is_some());
    }

    #[test]
    fn test_endpoints_cover_main_routes() {
        let desc = build_service_descriptor();
        let paths: Vec<&str> = desc.endpoints.iter().map(|e| e.path.as_str()).collect();
        assert!(paths.contains(&"/health"));
        assert!(paths.contains(&"/status"));
        assert!(paths.contains(&"/sessions"));
        assert!(paths.contains(&"/message"));
    }

    #[test]
    fn test_capabilities_include_core_features() {
        let desc = build_service_descriptor();
        assert!(desc.capabilities.contains(&"tee-support".to_string()));
        assert!(desc
            .capabilities
            .contains(&"privacy-classification".to_string()));
        assert!(desc.capabilities.contains(&"multi-agent".to_string()));
    }
}
