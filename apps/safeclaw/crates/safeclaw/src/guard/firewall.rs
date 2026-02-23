//! Network firewall for outbound connection control
//!
//! Enforces a whitelist-only policy for outbound network access from within
//! the TEE environment. Since SafeClaw processes run inside a MicroVM,
//! network control is enforced at the application layer by validating
//!
//! **Threat model**: Defends against A1 (malicious user) and A3 (network attacker) at AS-1.
//! See `docs/threat-model.md` §4 AS-1, §5.
//! URLs and hostnames before tool calls are allowed to proceed.
//!
//! Default policy: only LLM API endpoints are allowed.

use super::traits::Firewall;
use crate::audit::{AuditEvent, AuditSeverity, LeakageVector};
use serde::{Deserialize, Serialize};

/// Decision from the firewall check
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FirewallDecision {
    /// Connection allowed (matches whitelist)
    Allow,
    /// Blocked: domain not in whitelist
    BlockDomain,
    /// Blocked: port not allowed
    BlockPort,
    /// Blocked: protocol not allowed (e.g., FTP, raw TCP)
    BlockProtocol,
}

/// Result of a firewall check
#[derive(Debug, Clone)]
pub struct FirewallResult {
    /// Decision
    pub decision: FirewallDecision,
    /// The host that was checked
    pub host: String,
    /// Reason for blocking (if blocked)
    pub reason: Option<String>,
    /// Audit event generated (if blocked)
    pub audit_event: Option<AuditEvent>,
}

/// A single allowed domain entry
#[derive(Debug, Clone, Serialize)]
pub struct AllowedDomain {
    /// Domain pattern (exact match or wildcard prefix like `*.openai.com`)
    pub pattern: String,
    /// Allowed ports (empty = 443 only)
    #[serde(default = "default_https_ports")]
    pub ports: Vec<u16>,
}

/// Deserialize AllowedDomain from either a plain string or a struct.
/// This allows HCL configs to use `allowed_domains = ["api.openai.com"]`
/// instead of requiring `allowed_domains = [{ pattern = "api.openai.com", ports = [443] }]`.
impl<'de> serde::Deserialize<'de> for AllowedDomain {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        use serde::de;

        struct AllowedDomainVisitor;

        impl<'de> de::Visitor<'de> for AllowedDomainVisitor {
            type Value = AllowedDomain;

            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                f.write_str("a domain string or { pattern, ports } object")
            }

            fn visit_str<E: de::Error>(self, v: &str) -> Result<AllowedDomain, E> {
                Ok(AllowedDomain {
                    pattern: v.to_string(),
                    ports: default_https_ports(),
                })
            }

            fn visit_map<M: de::MapAccess<'de>>(
                self,
                mut map: M,
            ) -> Result<AllowedDomain, M::Error> {
                let mut pattern = None;
                let mut ports = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        "pattern" => pattern = Some(map.next_value()?),
                        "ports" => ports = Some(map.next_value()?),
                        _ => {
                            let _ = map.next_value::<serde::de::IgnoredAny>()?;
                        }
                    }
                }
                Ok(AllowedDomain {
                    pattern: pattern.ok_or_else(|| de::Error::missing_field("pattern"))?,
                    ports: ports.unwrap_or_else(default_https_ports),
                })
            }
        }

        deserializer.deserialize_any(AllowedDomainVisitor)
    }
}

fn default_https_ports() -> Vec<u16> {
    vec![443]
}

/// Network policy configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicy {
    /// Enable network firewall
    #[serde(default = "default_true")]
    pub enabled: bool,
    /// Allowed outbound domains
    #[serde(default = "default_allowed_domains")]
    pub allowed_domains: Vec<AllowedDomain>,
    /// Allowed protocols (default: https only)
    #[serde(default = "default_allowed_protocols")]
    pub allowed_protocols: Vec<String>,
    /// Block all outbound by default (true = whitelist mode)
    #[serde(default = "default_true")]
    pub default_deny: bool,
}

fn default_true() -> bool {
    true
}

fn default_allowed_protocols() -> Vec<String> {
    vec!["https".to_string()]
}

/// Default allowed domains: major LLM API endpoints
fn default_allowed_domains() -> Vec<AllowedDomain> {
    vec![
        // Anthropic
        AllowedDomain {
            pattern: "api.anthropic.com".to_string(),
            ports: vec![443],
        },
        // OpenAI
        AllowedDomain {
            pattern: "api.openai.com".to_string(),
            ports: vec![443],
        },
        // Google AI
        AllowedDomain {
            pattern: "generativelanguage.googleapis.com".to_string(),
            ports: vec![443],
        },
        // Azure OpenAI (wildcard)
        AllowedDomain {
            pattern: "*.openai.azure.com".to_string(),
            ports: vec![443],
        },
        // AWS Bedrock
        AllowedDomain {
            pattern: "*.amazonaws.com".to_string(),
            ports: vec![443],
        },
    ]
}

impl Default for NetworkPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            allowed_domains: default_allowed_domains(),
            allowed_protocols: default_allowed_protocols(),
            default_deny: true,
        }
    }
}

/// Network firewall that validates outbound connections against a whitelist.
#[derive(Debug, Clone)]
pub struct NetworkFirewall {
    policy: NetworkPolicy,
}

impl NetworkFirewall {
    /// Create a new firewall with the given policy.
    pub fn new(policy: NetworkPolicy) -> Self {
        Self { policy }
    }

    /// Check if a URL is allowed by the firewall policy.
    ///
    /// Parses the URL to extract protocol, host, and port, then validates
    /// each against the configured whitelist.
    pub fn check_url(&self, url: &str, session_id: &str) -> FirewallResult {
        if !self.policy.enabled {
            return FirewallResult {
                decision: FirewallDecision::Allow,
                host: url.to_string(),
                reason: None,
                audit_event: None,
            };
        }

        // Parse URL components
        let (protocol, host, port) = match parse_url(url) {
            Some(parts) => parts,
            None => {
                return self.block(
                    FirewallDecision::BlockProtocol,
                    url,
                    "Failed to parse URL",
                    session_id,
                );
            }
        };

        // Check protocol
        if !self
            .policy
            .allowed_protocols
            .iter()
            .any(|p| p.eq_ignore_ascii_case(&protocol))
        {
            return self.block(
                FirewallDecision::BlockProtocol,
                &host,
                &format!("Protocol '{}' not allowed", protocol),
                session_id,
            );
        }

        // Find matching domain
        let matched_domain = self
            .policy
            .allowed_domains
            .iter()
            .find(|d| domain_matches(&d.pattern, &host));

        match matched_domain {
            None => {
                if self.policy.default_deny {
                    self.block(
                        FirewallDecision::BlockDomain,
                        &host,
                        &format!("Domain '{}' not in whitelist", host),
                        session_id,
                    )
                } else {
                    FirewallResult {
                        decision: FirewallDecision::Allow,
                        host,
                        reason: None,
                        audit_event: None,
                    }
                }
            }
            Some(domain) => {
                // Check port
                if !domain.ports.is_empty() && !domain.ports.contains(&port) {
                    self.block(
                        FirewallDecision::BlockPort,
                        &host,
                        &format!("Port {} not allowed for domain '{}'", port, host),
                        session_id,
                    )
                } else {
                    FirewallResult {
                        decision: FirewallDecision::Allow,
                        host,
                        reason: None,
                        audit_event: None,
                    }
                }
            }
        }
    }

    /// Check if a raw host:port is allowed.
    pub fn check_host(&self, host: &str, port: u16, session_id: &str) -> FirewallResult {
        if !self.policy.enabled {
            return FirewallResult {
                decision: FirewallDecision::Allow,
                host: host.to_string(),
                reason: None,
                audit_event: None,
            };
        }

        let matched_domain = self
            .policy
            .allowed_domains
            .iter()
            .find(|d| domain_matches(&d.pattern, host));

        match matched_domain {
            None => {
                if self.policy.default_deny {
                    self.block(
                        FirewallDecision::BlockDomain,
                        host,
                        &format!("Host '{}' not in whitelist", host),
                        session_id,
                    )
                } else {
                    FirewallResult {
                        decision: FirewallDecision::Allow,
                        host: host.to_string(),
                        reason: None,
                        audit_event: None,
                    }
                }
            }
            Some(domain) => {
                if !domain.ports.is_empty() && !domain.ports.contains(&port) {
                    self.block(
                        FirewallDecision::BlockPort,
                        host,
                        &format!("Port {} not allowed for host '{}'", port, host),
                        session_id,
                    )
                } else {
                    FirewallResult {
                        decision: FirewallDecision::Allow,
                        host: host.to_string(),
                        reason: None,
                        audit_event: None,
                    }
                }
            }
        }
    }

    /// Get the current policy.
    pub fn policy(&self) -> &NetworkPolicy {
        &self.policy
    }

    fn block(
        &self,
        decision: FirewallDecision,
        host: &str,
        reason: &str,
        session_id: &str,
    ) -> FirewallResult {
        let event = AuditEvent::new(
            session_id.to_string(),
            AuditSeverity::High,
            LeakageVector::NetworkExfil,
            format!("Network firewall blocked: {}", reason),
        );

        tracing::warn!(
            session_id = session_id,
            host = host,
            reason = reason,
            "Network firewall blocked outbound connection"
        );

        FirewallResult {
            decision,
            host: host.to_string(),
            reason: Some(reason.to_string()),
            audit_event: Some(event),
        }
    }
}

impl Firewall for NetworkFirewall {
    fn check_url(&self, url: &str, session_id: &str) -> FirewallResult {
        NetworkFirewall::check_url(self, url, session_id)
    }

    fn check_host(&self, host: &str, port: u16, session_id: &str) -> FirewallResult {
        NetworkFirewall::check_host(self, host, port, session_id)
    }
}

/// Check if a domain pattern matches a hostname.
///
/// Supports exact match and wildcard prefix (`*.example.com`).
fn domain_matches(pattern: &str, host: &str) -> bool {
    let pattern_lower = pattern.to_ascii_lowercase();
    let host_lower = host.to_ascii_lowercase();

    if pattern_lower.starts_with("*.") {
        let suffix = &pattern_lower[1..]; // ".example.com"
        host_lower.ends_with(suffix) || host_lower == pattern_lower[2..]
    } else {
        host_lower == pattern_lower
    }
}

/// Parse a URL into (protocol, host, port).
///
/// Handles common URL formats:
/// - `https://api.openai.com/v1/chat`
/// - `http://localhost:8080/path`
/// - `api.openai.com` (assumes https:443)
fn parse_url(url: &str) -> Option<(String, String, u16)> {
    let url = url.trim();

    // Extract protocol
    let (protocol, rest) = if let Some(idx) = url.find("://") {
        (url[..idx].to_lowercase(), &url[idx + 3..])
    } else {
        ("https".to_string(), url)
    };

    // Extract host:port (strip path)
    let host_port = rest.split('/').next().unwrap_or(rest);

    // Strip userinfo (user:pass@host)
    let host_port = if let Some(idx) = host_port.rfind('@') {
        &host_port[idx + 1..]
    } else {
        host_port
    };

    // Split host and port
    let (host, port) = if let Some(idx) = host_port.rfind(':') {
        let port_str = &host_port[idx + 1..];
        if let Ok(p) = port_str.parse::<u16>() {
            (host_port[..idx].to_string(), p)
        } else {
            (host_port.to_string(), default_port(&protocol))
        }
    } else {
        (host_port.to_string(), default_port(&protocol))
    };

    if host.is_empty() {
        return None;
    }

    Some((protocol, host, port))
}

fn default_port(protocol: &str) -> u16 {
    match protocol {
        "https" => 443,
        "http" => 80,
        "ftp" => 21,
        _ => 443,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn firewall() -> NetworkFirewall {
        NetworkFirewall::new(NetworkPolicy::default())
    }

    // ---- URL parsing ----

    #[test]
    fn test_parse_url_https() {
        let (proto, host, port) = parse_url("https://api.openai.com/v1/chat").unwrap();
        assert_eq!(proto, "https");
        assert_eq!(host, "api.openai.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_url_http_with_port() {
        let (proto, host, port) = parse_url("http://localhost:8080/path").unwrap();
        assert_eq!(proto, "http");
        assert_eq!(host, "localhost");
        assert_eq!(port, 8080);
    }

    #[test]
    fn test_parse_url_bare_host() {
        let (proto, host, port) = parse_url("api.anthropic.com").unwrap();
        assert_eq!(proto, "https");
        assert_eq!(host, "api.anthropic.com");
        assert_eq!(port, 443);
    }

    #[test]
    fn test_parse_url_empty() {
        assert!(parse_url("").is_none());
    }

    // ---- Domain matching ----

    #[test]
    fn test_domain_exact_match() {
        assert!(domain_matches("api.openai.com", "api.openai.com"));
        assert!(domain_matches("api.openai.com", "API.OPENAI.COM"));
        assert!(!domain_matches("api.openai.com", "evil.openai.com"));
    }

    #[test]
    fn test_domain_wildcard_match() {
        assert!(domain_matches(
            "*.openai.azure.com",
            "myinstance.openai.azure.com"
        ));
        assert!(domain_matches("*.openai.azure.com", "openai.azure.com"));
        assert!(!domain_matches("*.openai.azure.com", "evil.com"));
    }

    // ---- Firewall decisions ----

    #[test]
    fn test_allow_openai() {
        let fw = firewall();
        let result = fw.check_url("https://api.openai.com/v1/chat/completions", "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);
        assert!(result.audit_event.is_none());
    }

    #[test]
    fn test_allow_anthropic() {
        let fw = firewall();
        let result = fw.check_url("https://api.anthropic.com/v1/messages", "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);
    }

    #[test]
    fn test_allow_azure_wildcard() {
        let fw = firewall();
        let result = fw.check_url(
            "https://myinstance.openai.azure.com/openai/deployments",
            "s1",
        );
        assert_eq!(result.decision, FirewallDecision::Allow);
    }

    #[test]
    fn test_block_unknown_domain() {
        let fw = firewall();
        let result = fw.check_url("https://evil-exfil.com/steal", "s1");
        assert_eq!(result.decision, FirewallDecision::BlockDomain);
        assert!(result.reason.is_some());
        assert!(result.audit_event.is_some());
        let event = result.audit_event.unwrap();
        assert_eq!(event.vector, LeakageVector::NetworkExfil);
        assert_eq!(event.severity, AuditSeverity::High);
    }

    #[test]
    fn test_block_http_protocol() {
        let fw = firewall();
        let result = fw.check_url("http://api.openai.com/v1/chat", "s1");
        assert_eq!(result.decision, FirewallDecision::BlockProtocol);
    }

    #[test]
    fn test_block_ftp_protocol() {
        let fw = firewall();
        let result = fw.check_url("ftp://files.example.com/data", "s1");
        assert_eq!(result.decision, FirewallDecision::BlockProtocol);
    }

    #[test]
    fn test_block_wrong_port() {
        let fw = firewall();
        let result = fw.check_url("https://api.openai.com:8080/v1/chat", "s1");
        assert_eq!(result.decision, FirewallDecision::BlockPort);
    }

    #[test]
    fn test_disabled_firewall_allows_all() {
        let fw = NetworkFirewall::new(NetworkPolicy {
            enabled: false,
            ..Default::default()
        });
        let result = fw.check_url("https://evil.com/steal", "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);
    }

    #[test]
    fn test_check_host_allowed() {
        let fw = firewall();
        let result = fw.check_host("api.anthropic.com", 443, "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);
    }

    #[test]
    fn test_check_host_blocked() {
        let fw = firewall();
        let result = fw.check_host("evil.com", 443, "s1");
        assert_eq!(result.decision, FirewallDecision::BlockDomain);
    }

    #[test]
    fn test_check_host_wrong_port() {
        let fw = firewall();
        let result = fw.check_host("api.openai.com", 80, "s1");
        assert_eq!(result.decision, FirewallDecision::BlockPort);
    }

    #[test]
    fn test_custom_policy() {
        let policy = NetworkPolicy {
            enabled: true,
            allowed_domains: vec![AllowedDomain {
                pattern: "internal.corp.com".to_string(),
                ports: vec![443, 8443],
            }],
            allowed_protocols: vec!["https".to_string()],
            default_deny: true,
        };
        let fw = NetworkFirewall::new(policy);

        // Custom domain allowed
        let result = fw.check_url("https://internal.corp.com/api", "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);

        // Custom domain on alt port allowed
        let result = fw.check_url("https://internal.corp.com:8443/api", "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);

        // Default LLM domains now blocked (not in custom policy)
        let result = fw.check_url("https://api.openai.com/v1/chat", "s1");
        assert_eq!(result.decision, FirewallDecision::BlockDomain);
    }

    #[test]
    fn test_default_deny_false() {
        let policy = NetworkPolicy {
            enabled: true,
            allowed_domains: vec![],
            allowed_protocols: vec!["https".to_string()],
            default_deny: false,
        };
        let fw = NetworkFirewall::new(policy);

        // Unknown domain allowed when default_deny is false
        let result = fw.check_url("https://anything.com/path", "s1");
        assert_eq!(result.decision, FirewallDecision::Allow);
    }
}
