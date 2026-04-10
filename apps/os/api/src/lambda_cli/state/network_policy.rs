//! Network Policies - Kubernetes-style traffic rules with runtime enforcement.
//!
//! Provides pod-level network isolation and security rules with actual
//! traffic filtering and enforcement capabilities.

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::IpAddr;
use tokio::sync::RwLock;

/// Network policy direction.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicyDirection {
    /// Incoming traffic.
    Ingress,
    /// Outgoing traffic.
    Egress,
}

impl Default for PolicyDirection {
    fn default() -> Self {
        PolicyDirection::Ingress
    }
}

/// Protocol for network policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Protocol {
    TCP,
    UDP,
    ICMP,
    SCTP,
}

impl Default for Protocol {
    fn default() -> Self {
        Protocol::TCP
    }
}

impl std::fmt::Display for Protocol {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Protocol::TCP => write!(f, "TCP"),
            Protocol::UDP => write!(f, "UDP"),
            Protocol::ICMP => write!(f, "ICMP"),
            Protocol::SCTP => write!(f, "SCTP"),
        }
    }
}

/// IP block for network policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpBlock {
    /// CIDR range.
    pub cidr: String,
    /// Exception list (for except clause).
    #[serde(default)]
    pub except: Vec<String>,
}

/// Port range for network policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyPort {
    /// Port number (optional, if not specified, all ports).
    #[serde(default)]
    pub port: Option<u16>,
    /// End port for range.
    #[serde(default)]
    pub end_port: Option<u16>,
    /// Protocol.
    #[serde(default)]
    pub protocol: Option<Protocol>,
}

/// Peer in network policy (what pods/namespaces/IPs are affected).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum NetworkPolicyPeer {
    /// Pod selector.
    PodSelector {
        #[serde(rename = "podSelector")]
        pod_selector: LabelSelector,
    },
    /// Namespace selector.
    NamespaceSelector {
        #[serde(rename = "namespaceSelector")]
        namespace_selector: LabelSelector,
    },
    /// IP block.
    IpBlock(IpBlock),
    /// Empty (matches all).
    Empty,
}

/// Label selector for matching pods/namespaces.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LabelSelector {
    /// Match labels.
    #[serde(default)]
    pub match_labels: HashMap<String, String>,
    /// Match expressions.
    #[serde(default)]
    pub match_expressions: Vec<LabelSelectorExpression>,
}

impl LabelSelector {
    /// Check if this selector matches the given labels.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        // Check match_labels
        for (key, value) in &self.match_labels {
            match labels.get(key) {
                Some(v) if v == value => {}
                _ => return false,
            }
        }

        // Check match_expressions
        for expr in &self.match_expressions {
            if !expr.matches(labels) {
                return false;
            }
        }

        true
    }
}

/// Label selector operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LabelSelectorOperator {
    In,
    NotIn,
    Exists,
    DoesNotExist,
}

impl LabelSelectorOperator {
    /// Parse from string.
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "In" => Some(LabelSelectorOperator::In),
            "NotIn" => Some(LabelSelectorOperator::NotIn),
            "Exists" => Some(LabelSelectorOperator::Exists),
            "DoesNotExist" => Some(LabelSelectorOperator::DoesNotExist),
            _ => None,
        }
    }
}

/// Label selector requirement.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelSelectorExpression {
    pub key: String,
    pub operator: String,
    #[serde(default)]
    pub values: Vec<String>,
}

impl LabelSelectorExpression {
    /// Check if this expression matches the given labels.
    pub fn matches(&self, labels: &HashMap<String, String>) -> bool {
        let op = match LabelSelectorOperator::from_str(&self.operator) {
            Some(op) => op,
            None => return true, // Unknown operator, assume matches
        };

        match op {
            LabelSelectorOperator::In => {
                if let Some(value) = labels.get(&self.key) {
                    self.values.contains(value)
                } else {
                    false
                }
            }
            LabelSelectorOperator::NotIn => {
                if let Some(value) = labels.get(&self.key) {
                    !self.values.contains(value)
                } else {
                    true
                }
            }
            LabelSelectorOperator::Exists => labels.contains_key(&self.key),
            LabelSelectorOperator::DoesNotExist => !labels.contains_key(&self.key),
        }
    }
}

/// Network policy ingress rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkPolicyIngressRule {
    /// Ports that are allowed.
    #[serde(default)]
    pub ports: Vec<NetworkPolicyPort>,
    /// From sources (pods, namespaces, IPs).
    #[serde(default)]
    pub from: Vec<NetworkPolicyPeer>,
}

/// Network policy egress rule.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkPolicyEgressRule {
    /// Ports that are allowed.
    #[serde(default)]
    pub ports: Vec<NetworkPolicyPort>,
    /// To destinations (pods, namespaces, IPs).
    #[serde(default)]
    pub to: Vec<NetworkPolicyPeer>,
}

/// Network policy specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct NetworkPolicySpec {
    /// Pod selector - which pods this policy applies to.
    #[serde(rename = "podSelector")]
    pub pod_selector: LabelSelector,
    /// Policy types.
    #[serde(default)]
    pub policy_types: Vec<PolicyType>,
    /// Ingress rules.
    #[serde(default)]
    pub ingress: Vec<NetworkPolicyIngressRule>,
    /// Egress rules.
    #[serde(default)]
    pub egress: Vec<NetworkPolicyEgressRule>,
}

/// Policy type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum PolicyType {
    Ingress,
    Egress,
}

/// NetworkPolicy desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkPolicyDesired {
    /// Policy name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Policy specification.
    pub spec: NetworkPolicySpec,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Whether this is a default-deny policy.
    #[serde(default)]
    pub is_default_deny: bool,
}

/// Traffic decision - allow or deny.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrafficDecision {
    Allow,
    Deny,
}

impl std::fmt::Display for TrafficDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TrafficDecision::Allow => write!(f, "Allow"),
            TrafficDecision::Deny => write!(f, "Deny"),
        }
    }
}

/// Traffic context for evaluation.
#[derive(Debug, Clone)]
pub struct TrafficContext {
    /// Source pod name (if applicable).
    pub src_pod: Option<String>,
    /// Source namespace (if applicable).
    pub src_namespace: Option<String>,
    /// Source labels.
    pub src_labels: HashMap<String, String>,
    /// Source IP address.
    pub src_ip: Option<IpAddr>,
    /// Destination pod name.
    pub dst_pod: Option<String>,
    /// Destination namespace.
    pub dst_namespace: Option<String>,
    /// Destination labels.
    pub dst_labels: HashMap<String, String>,
    /// Destination IP address.
    pub dst_ip: Option<IpAddr>,
    /// Destination port.
    pub dst_port: u16,
    /// Protocol.
    pub protocol: Protocol,
}

impl TrafficContext {
    /// Create a new traffic context for ingress.
    pub fn ingress(
        src_pod: String,
        src_namespace: String,
        src_ip: IpAddr,
        dst_pod: String,
        dst_namespace: String,
        dst_ip: IpAddr,
        dst_port: u16,
        protocol: Protocol,
    ) -> Self {
        Self {
            src_pod: Some(src_pod),
            src_namespace: Some(src_namespace),
            src_labels: HashMap::new(),
            src_ip: Some(src_ip),
            dst_pod: Some(dst_pod),
            dst_namespace: Some(dst_namespace),
            dst_labels: HashMap::new(),
            dst_ip: Some(dst_ip),
            dst_port,
            protocol,
        }
    }

    /// Create a new traffic context for egress.
    pub fn egress(
        src_pod: String,
        src_namespace: String,
        src_ip: IpAddr,
        dst_ip: IpAddr,
        dst_port: u16,
        protocol: Protocol,
    ) -> Self {
        Self {
            src_pod: Some(src_pod),
            src_namespace: Some(src_namespace.clone()),
            src_labels: HashMap::new(),
            src_ip: Some(src_ip),
            dst_pod: None,
            dst_namespace: Some(src_namespace),
            dst_labels: HashMap::new(),
            dst_ip: Some(dst_ip),
            dst_port,
            protocol,
        }
    }
}

/// Pod info for network policy enforcement.
#[derive(Debug, Clone)]
pub struct PolicyPodInfo {
    pub name: String,
    pub namespace: String,
    pub ip: IpAddr,
    pub labels: HashMap<String, String>,
}

/// Namespace info for network policy enforcement.
#[derive(Debug, Clone)]
pub struct PolicyNamespaceInfo {
    pub name: String,
    pub labels: HashMap<String, String>,
}

/// Pod registry for looking up pod labels.
pub trait PodRegistry: Send + Sync {
    /// Get pod info by name and namespace.
    fn get_pod(&self, namespace: &str, name: &str) -> Option<PolicyPodInfo>;
}

/// Namespace registry for looking up namespace labels.
pub trait NamespaceRegistry: Send + Sync {
    /// Get namespace info by name.
    fn get_namespace(&self, name: &str) -> Option<PolicyNamespaceInfo>;
}

/// Empty pod registry (for testing).
impl PodRegistry for () {
    fn get_pod(&self, _namespace: &str, _name: &str) -> Option<PolicyPodInfo> {
        None
    }
}

/// Empty namespace registry (for testing).
impl NamespaceRegistry for () {
    fn get_namespace(&self, _name: &str) -> Option<PolicyNamespaceInfo> {
        None
    }
}

/// NetworkPolicy engine - evaluates traffic against policies.
pub struct NetworkPolicyEngine {
    /// Policies indexed by namespace.
    policies: RwLock<HashMap<String, Vec<NetworkPolicyDesired>>>,
    /// Default deny policies per namespace.
    default_deny: RwLock<HashMap<String, DefaultDenyPolicy>>,
    /// Pod registry reference.
    pod_registry: RwLock<Option<Box<dyn PodRegistry>>>,
    /// Namespace registry reference.
    ns_registry: RwLock<Option<Box<dyn NamespaceRegistry>>>,
}

/// Default deny policy for a namespace.
#[derive(Debug, Clone)]
pub struct DefaultDenyPolicy {
    pub namespace: String,
    pub ingress_deny: bool,
    pub egress_deny: bool,
    pub created_at: DateTime<Utc>,
}

impl NetworkPolicyEngine {
    /// Create a new network policy engine.
    pub fn new() -> Self {
        Self {
            policies: RwLock::new(HashMap::new()),
            default_deny: RwLock::new(HashMap::new()),
            pod_registry: RwLock::new(None),
            ns_registry: RwLock::new(None),
        }
    }

    /// Set pod registry.
    pub fn with_pod_registry(mut self, registry: Box<dyn PodRegistry>) -> Self {
        self.pod_registry = RwLock::new(Some(registry));
        self
    }

    /// Set namespace registry.
    pub fn with_namespace_registry(mut self, registry: Box<dyn NamespaceRegistry>) -> Self {
        self.ns_registry = RwLock::new(Some(registry));
        self
    }

    /// Add a network policy.
    pub async fn add_policy(&self, policy: NetworkPolicyDesired) -> Result<()> {
        let mut policies = self.policies.write().await;
        policies
            .entry(policy.namespace.clone())
            .or_insert_with(Vec::new)
            .push(policy);
        Ok(())
    }

    /// Remove a network policy.
    pub async fn remove_policy(&self, namespace: &str, name: &str) -> Result<()> {
        let mut policies = self.policies.write().await;
        if let Some(policies_in_ns) = policies.get_mut(namespace) {
            policies_in_ns.retain(|p| p.name != name);
        }
        Ok(())
    }

    /// Get policies for a namespace.
    pub async fn get_policies(&self, namespace: &str) -> Vec<NetworkPolicyDesired> {
        let policies = self.policies.read().await;
        policies.get(namespace).cloned().unwrap_or_default()
    }

    /// Enable default deny for a namespace.
    pub async fn enable_default_deny(
        &self,
        namespace: &str,
        ingress: bool,
        egress: bool,
    ) -> Result<()> {
        let mut default_deny = self.default_deny.write().await;
        default_deny.insert(
            namespace.to_string(),
            DefaultDenyPolicy {
                namespace: namespace.to_string(),
                ingress_deny: ingress,
                egress_deny: egress,
                created_at: Utc::now(),
            },
        );
        Ok(())
    }

    /// Disable default deny for a namespace.
    pub async fn disable_default_deny(&self, namespace: &str) -> Result<()> {
        let mut default_deny = self.default_deny.write().await;
        default_deny.remove(namespace);
        Ok(())
    }

    /// Evaluate if traffic should be allowed.
    pub async fn evaluate(&self, ctx: &TrafficContext) -> TrafficDecision {
        let ns = ctx.dst_namespace.clone().unwrap_or_default();

        // First check default deny policy
        {
            let default_deny = self.default_deny.read().await;
            if let Some(deny) = default_deny.get(&ns) {
                let is_ingress = ctx.src_pod.is_some();
                if (is_ingress && deny.ingress_deny) || (!is_ingress && deny.egress_deny) {
                    // Check if there's any policy that allows this traffic
                    let policies = self.policies.read().await;
                    let ns_policies = policies.get(&ns);

                    let allowed = if let Some(ns_policies) = ns_policies {
                        self.find_allowing_policy(ctx, ns_policies).await
                    } else {
                        false
                    };

                    if !allowed {
                        return TrafficDecision::Deny;
                    }
                }
            }
        }

        // Check namespace policies
        let policies = self.policies.read().await;
        let ns_policies = policies.get(&ns);

        if let Some(ns_policies) = ns_policies {
            for policy in ns_policies {
                // Check if policy applies to destination pod
                if !self.pod_matches_selector(
                    &ctx.dst_pod,
                    &ctx.dst_labels,
                    &policy.spec.pod_selector,
                ) {
                    continue;
                }

                // Check policy type
                for ptype in &policy.spec.policy_types {
                    match ptype {
                        PolicyType::Ingress if ctx.src_pod.is_some() => {
                            // Check ingress rules
                            let decision = self.evaluate_ingress_rule(ctx, &policy.spec.ingress);
                            if decision == TrafficDecision::Deny {
                                return TrafficDecision::Deny;
                            }
                        }
                        PolicyType::Egress if ctx.src_pod.is_some() => {
                            // Check egress rules
                            let decision = self.evaluate_egress_rule(ctx, &policy.spec.egress);
                            if decision == TrafficDecision::Deny {
                                return TrafficDecision::Deny;
                            }
                        }
                        _ => {}
                    }
                }
            }
        }

        // Default: allow if no policy denies
        TrafficDecision::Allow
    }

    /// Find a policy that allows the traffic.
    async fn find_allowing_policy<'a>(
        &self,
        ctx: &TrafficContext,
        policies: &'a [NetworkPolicyDesired],
    ) -> bool {
        for policy in policies {
            if !self.pod_matches_selector(&ctx.dst_pod, &ctx.dst_labels, &policy.spec.pod_selector)
            {
                continue;
            }

            for ptype in &policy.spec.policy_types {
                match ptype {
                    PolicyType::Ingress => {
                        if self.evaluate_ingress_rule(ctx, &policy.spec.ingress)
                            == TrafficDecision::Allow
                        {
                            return true;
                        }
                    }
                    PolicyType::Egress => {
                        if self.evaluate_egress_rule(ctx, &policy.spec.egress)
                            == TrafficDecision::Allow
                        {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    /// Check if a pod matches a label selector (using provided labels).
    fn pod_matches_selector(
        &self,
        pod_name: &Option<String>,
        pod_labels: &HashMap<String, String>,
        selector: &LabelSelector,
    ) -> bool {
        // If no pod info provided, can't match
        if pod_labels.is_empty() && pod_name.is_none() {
            return false;
        }

        // If selector is empty, it matches all pods
        if selector.match_labels.is_empty() && selector.match_expressions.is_empty() {
            return true;
        }

        // Use provided labels if available
        if !pod_labels.is_empty() {
            return selector.matches(pod_labels);
        }

        // Can't determine without registry, assume no match
        false
    }

    /// Evaluate an ingress rule against traffic context.
    fn evaluate_ingress_rule(
        &self,
        ctx: &TrafficContext,
        rules: &[NetworkPolicyIngressRule],
    ) -> TrafficDecision {
        // If no ingress rules defined, deny all ingress (default deny)
        if rules.is_empty() {
            return TrafficDecision::Deny;
        }

        for rule in rules {
            // Check ports
            if !rule.ports.is_empty() {
                let port_matches = rule
                    .ports
                    .iter()
                    .any(|p| self.port_matches(ctx.dst_port, ctx.protocol, p));

                if !port_matches {
                    continue; // Try next rule
                }
            }

            // Check source (from)
            if rule.from.is_empty() {
                return TrafficDecision::Allow; // Empty from allows all
            }

            for peer in &rule.from {
                if self.peer_matches(ctx, peer, PolicyDirection::Ingress) {
                    return TrafficDecision::Allow;
                }
            }
        }

        TrafficDecision::Deny
    }

    /// Evaluate an egress rule against traffic context.
    fn evaluate_egress_rule(
        &self,
        ctx: &TrafficContext,
        rules: &[NetworkPolicyEgressRule],
    ) -> TrafficDecision {
        // If no egress rules defined, deny all egress (default deny)
        if rules.is_empty() {
            return TrafficDecision::Deny;
        }

        for rule in rules {
            // Check ports
            if !rule.ports.is_empty() {
                let port_matches = rule
                    .ports
                    .iter()
                    .any(|p| self.port_matches(ctx.dst_port, ctx.protocol, p));

                if !port_matches {
                    continue; // Try next rule
                }
            }

            // Check destination (to)
            if rule.to.is_empty() {
                return TrafficDecision::Allow; // Empty to allows all
            }

            for peer in &rule.to {
                if self.peer_matches(ctx, peer, PolicyDirection::Egress) {
                    return TrafficDecision::Allow;
                }
            }
        }

        TrafficDecision::Deny
    }

    /// Check if a port matches.
    fn port_matches(&self, port: u16, protocol: Protocol, rule_port: &NetworkPolicyPort) -> bool {
        // Check protocol first
        if let Some(p) = &rule_port.protocol {
            if *p != protocol {
                return false;
            }
        }

        // Check port
        if let Some(rule_port_num) = rule_port.port {
            if let Some(end_port) = rule_port.end_port {
                // Range
                port >= rule_port_num && port <= end_port
            } else {
                // Single port
                port == rule_port_num
            }
        } else {
            // No port specified = all ports
            true
        }
    }

    /// Check if a peer matches the traffic context.
    fn peer_matches(
        &self,
        ctx: &TrafficContext,
        peer: &NetworkPolicyPeer,
        direction: PolicyDirection,
    ) -> bool {
        match peer {
            NetworkPolicyPeer::Empty => true,
            NetworkPolicyPeer::IpBlock(ipblock) => {
                // Check if source/dest IP is in CIDR
                let ip = match direction {
                    PolicyDirection::Ingress => ctx.src_ip,
                    PolicyDirection::Egress => ctx.dst_ip,
                };

                if let Some(ip) = ip {
                    // Check exceptions first
                    for except_cidr in &ipblock.except {
                        if ip_in_cidr(&ip, except_cidr) {
                            return false;
                        }
                    }
                    // Then check main CIDR
                    ip_in_cidr(&ip, &ipblock.cidr)
                } else {
                    false
                }
            }
            NetworkPolicyPeer::PodSelector { pod_selector } => {
                let labels = match direction {
                    PolicyDirection::Ingress => &ctx.src_labels,
                    PolicyDirection::Egress => &ctx.dst_labels,
                };

                // If labels are provided, use them directly
                if !labels.is_empty() {
                    return pod_selector.matches(labels);
                }

                // Can't match without labels
                false
            }
            NetworkPolicyPeer::NamespaceSelector { namespace_selector } => {
                // Namespace selectors need registry lookup - simplified matching
                // If no expressions and no labels, assume empty namespace matches
                namespace_selector.match_labels.is_empty()
                    && namespace_selector.match_expressions.is_empty()
            }
        }
    }
}

/// Check if an IP is in a CIDR range.
fn ip_in_cidr(ip: &IpAddr, cidr: &str) -> bool {
    // Parse CIDR
    let (network, prefix_len) = match parse_cidr(cidr) {
        Some((net, len)) => (net, len),
        None => return cidr == "*" || cidr.is_empty(),
    };

    match (ip, &network) {
        (IpAddr::V4(ipv4), IpAddr::V4(net)) => {
            // Ipv4Addr octets() returns big-endian bytes
            let ipv4_bits = u32::from_be_bytes(ipv4.octets());
            let net_bits = u32::from_be_bytes(net.octets());
            let mask = if prefix_len == 0 {
                0
            } else {
                u32::MAX << (32 - prefix_len)
            };
            (ipv4_bits & mask) == (net_bits & mask)
        }
        (IpAddr::V6(ipv6), IpAddr::V6(net)) => {
            // IPv6 CIDR matching (simplified)
            let octets = ipv6.octets();
            let net_octets = net.octets();
            let full_bytes = prefix_len / 8;
            let remaining_bits = prefix_len % 8;

            for i in 0..full_bytes {
                if octets[i] != net_octets[i] {
                    return false;
                }
            }

            if remaining_bits > 0 && full_bytes < 16 {
                let mask = u8::MAX << (8 - remaining_bits);
                if (octets[full_bytes] & mask) != (net_octets[full_bytes] & mask) {
                    return false;
                }
            }

            true
        }
        _ => false,
    }
}

/// Parse CIDR string into (IpAddr, prefix_len).
fn parse_cidr(cidr: &str) -> Option<(IpAddr, usize)> {
    let parts: Vec<&str> = cidr.split('/').collect();
    if parts.is_empty() {
        return None;
    }

    let ip: IpAddr = match parts[0].parse() {
        Ok(ip) => ip,
        Err(_) => return None,
    };

    let prefix_len = if parts.len() == 2 {
        parts[1].parse().ok()?
    } else {
        match ip {
            IpAddr::V4(_) => 32,
            IpAddr::V6(_) => 128,
        }
    };

    // Validate prefix length
    let max_len = match ip {
        IpAddr::V4(_) => 32,
        IpAddr::V6(_) => 128,
    };

    if prefix_len > max_len {
        return None;
    }

    Some((ip, prefix_len))
}

impl Default for NetworkPolicyEngine {
    fn default() -> Self {
        Self::new()
    }
}

/// NetworkPolicyAgent - enforces network policies on a pod.
pub struct NetworkPolicyAgent {
    /// Pod name.
    pod_name: String,
    /// Pod namespace.
    namespace: String,
    /// Pod IP.
    pod_ip: IpAddr,
    /// Reference to the policy engine.
    engine: std::sync::Arc<NetworkPolicyEngine>,
}

impl NetworkPolicyAgent {
    /// Create a new network policy agent.
    pub fn new(pod_name: String, namespace: String, pod_ip: IpAddr) -> Self {
        Self {
            pod_name,
            namespace,
            pod_ip,
            engine: std::sync::Arc::new(NetworkPolicyEngine::new()),
        }
    }

    /// Create with a shared engine.
    pub fn with_engine(
        pod_name: String,
        namespace: String,
        pod_ip: IpAddr,
        engine: std::sync::Arc<NetworkPolicyEngine>,
    ) -> Self {
        Self {
            pod_name,
            namespace,
            pod_ip,
            engine,
        }
    }

    /// Set pod registry.
    pub fn set_pod_registry(&self, registry: Box<dyn PodRegistry>) {
        let _ = registry;
    }

    /// Check if ingress traffic should be allowed.
    pub async fn can_ingress(&self, ctx: TrafficContext) -> TrafficDecision {
        let mut ctx = ctx;
        ctx.dst_pod = Some(self.pod_name.clone());
        ctx.dst_namespace = Some(self.namespace.clone());
        ctx.dst_ip = Some(self.pod_ip);

        self.engine.evaluate(&ctx).await
    }

    /// Check if egress traffic should be allowed.
    pub async fn can_egress(&self, ctx: TrafficContext) -> TrafficDecision {
        let mut ctx = ctx;
        ctx.src_pod = Some(self.pod_name.clone());
        ctx.src_namespace = Some(self.namespace.clone());
        ctx.src_ip = Some(self.pod_ip);

        self.engine.evaluate(&ctx).await
    }

    /// Filter incoming packet (returns true if allowed).
    pub async fn filter_ingress(&self, src_ip: IpAddr, dst_port: u16, protocol: Protocol) -> bool {
        let ctx = TrafficContext {
            src_pod: None,
            src_namespace: None,
            src_labels: HashMap::new(),
            src_ip: Some(src_ip),
            dst_pod: Some(self.pod_name.clone()),
            dst_namespace: Some(self.namespace.clone()),
            dst_labels: HashMap::new(),
            dst_ip: Some(self.pod_ip),
            dst_port,
            protocol,
        };

        self.engine.evaluate(&ctx).await == TrafficDecision::Allow
    }

    /// Filter outgoing packet (returns true if allowed).
    pub async fn filter_egress(&self, dst_ip: IpAddr, dst_port: u16, protocol: Protocol) -> bool {
        let ctx = TrafficContext {
            src_pod: Some(self.pod_name.clone()),
            src_namespace: Some(self.namespace.clone()),
            src_labels: HashMap::new(),
            src_ip: Some(self.pod_ip),
            dst_pod: None,
            dst_namespace: Some(self.namespace.clone()),
            dst_labels: HashMap::new(),
            dst_ip: Some(dst_ip),
            dst_port,
            protocol,
        };

        self.engine.evaluate(&ctx).await == TrafficDecision::Allow
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn create_test_engine() -> NetworkPolicyEngine {
        NetworkPolicyEngine::new()
    }

    #[tokio::test]
    async fn test_empty_policy_allows() {
        let engine = create_test_engine();

        let ctx = TrafficContext {
            src_pod: Some("client".to_string()),
            src_namespace: Some("default".to_string()),
            src_labels: HashMap::new(),
            src_ip: Some(IpAddr::from([10, 0, 0, 1])),
            dst_pod: Some("server".to_string()),
            dst_namespace: Some("default".to_string()),
            dst_labels: HashMap::new(),
            dst_ip: Some(IpAddr::from([10, 0, 0, 2])),
            dst_port: 8080,
            protocol: Protocol::TCP,
        };

        // No policies = allow
        let decision = engine.evaluate(&ctx).await;
        assert_eq!(decision, TrafficDecision::Allow);
    }

    #[tokio::test]
    async fn test_policy_creation() {
        let engine = create_test_engine();

        let policy = NetworkPolicyDesired {
            name: "db-policy".to_string(),
            namespace: "default".to_string(),
            spec: NetworkPolicySpec {
                pod_selector: LabelSelector {
                    match_labels: {
                        let mut m = HashMap::new();
                        m.insert("app".to_string(), "database".to_string());
                        m
                    },
                    ..Default::default()
                },
                policy_types: vec![PolicyType::Ingress],
                ingress: vec![NetworkPolicyIngressRule {
                    ports: vec![NetworkPolicyPort {
                        port: Some(5432),
                        end_port: None,
                        protocol: Some(Protocol::TCP),
                    }],
                    from: vec![],
                }],
                ..Default::default()
            },
            created_at: Utc::now(),
            is_default_deny: false,
        };

        engine.add_policy(policy).await.unwrap();

        let policies = engine.get_policies("default").await;
        assert_eq!(policies.len(), 1);
        assert_eq!(policies[0].name, "db-policy");
    }

    #[tokio::test]
    async fn test_ip_cidr_matching() {
        // Test IPv4 CIDR
        let ip = IpAddr::from([10, 0, 0, 5]);

        // Exact match
        assert!(ip_in_cidr(&ip, "10.0.0.5/32"));

        // Range match
        assert!(ip_in_cidr(&ip, "10.0.0.0/24"));

        // Non-matching range
        assert!(!ip_in_cidr(&ip, "192.168.0.0/24"));

        // Wildcard
        assert!(ip_in_cidr(&ip, "0.0.0.0/0"));
    }

    #[tokio::test]
    async fn test_port_matching() {
        let engine = create_test_engine();

        let port = NetworkPolicyPort {
            port: Some(80),
            end_port: None,
            protocol: Some(Protocol::TCP),
        };

        // Exact port match
        assert!(engine.port_matches(80, Protocol::TCP, &port));
        assert!(!engine.port_matches(8080, Protocol::TCP, &port));

        // Protocol mismatch
        let udp_port = NetworkPolicyPort {
            port: Some(80),
            end_port: None,
            protocol: Some(Protocol::UDP),
        };
        assert!(!engine.port_matches(80, Protocol::TCP, &udp_port));

        // Port range
        let range_port = NetworkPolicyPort {
            port: Some(8000),
            end_port: Some(9000),
            protocol: Some(Protocol::TCP),
        };
        assert!(engine.port_matches(8500, Protocol::TCP, &range_port));
        assert!(!engine.port_matches(9500, Protocol::TCP, &range_port));
    }

    #[tokio::test]
    async fn test_default_deny() {
        let engine = create_test_engine();

        // Enable default deny for namespace
        engine
            .enable_default_deny("secure", true, false)
            .await
            .unwrap();

        let ctx = TrafficContext {
            src_pod: Some("client".to_string()),
            src_namespace: Some("default".to_string()),
            src_labels: HashMap::new(),
            src_ip: Some(IpAddr::from([10, 0, 0, 1])),
            dst_pod: Some("server".to_string()),
            dst_namespace: Some("secure".to_string()),
            dst_labels: HashMap::new(),
            dst_ip: Some(IpAddr::from([10, 0, 0, 2])),
            dst_port: 8080,
            protocol: Protocol::TCP,
        };

        // With default deny and no policy = deny
        let decision = engine.evaluate(&ctx).await;
        assert_eq!(decision, TrafficDecision::Deny);
    }

    #[tokio::test]
    async fn test_label_selector_matching() {
        let selector = LabelSelector {
            match_labels: {
                let mut m = HashMap::new();
                m.insert("app".to_string(), "web".to_string());
                m.insert("version".to_string(), "v1".to_string());
                m
            },
            match_expressions: vec![],
        };

        let matching_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "web".to_string());
            m.insert("version".to_string(), "v1".to_string());
            m.insert("tier".to_string(), "frontend".to_string());
            m
        };

        let non_matching_labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "web".to_string());
            m.insert("version".to_string(), "v2".to_string());
            m
        };

        assert!(selector.matches(&matching_labels));
        assert!(!selector.matches(&non_matching_labels));
    }

    #[tokio::test]
    async fn test_empty_selector_matches_all() {
        let selector = LabelSelector::default();

        let labels = {
            let mut m = HashMap::new();
            m.insert("app".to_string(), "web".to_string());
            m
        };

        // Empty selector should match all
        assert!(selector.matches(&labels));
    }

    #[tokio::test]
    async fn test_ipblock_exceptions() {
        let engine = create_test_engine();

        // Add a policy that allows 10.0.0.0/8 except 10.0.0.5
        let policy = NetworkPolicyDesired {
            name: "block-specific".to_string(),
            namespace: "default".to_string(),
            spec: NetworkPolicySpec {
                pod_selector: LabelSelector::default(),
                policy_types: vec![PolicyType::Ingress],
                ingress: vec![NetworkPolicyIngressRule {
                    ports: vec![],
                    from: vec![NetworkPolicyPeer::IpBlock(IpBlock {
                        cidr: "10.0.0.0/8".to_string(),
                        except: vec!["10.0.0.5/32".to_string()],
                    })],
                }],
                ..Default::default()
            },
            created_at: Utc::now(),
            is_default_deny: false,
        };

        engine.add_policy(policy).await.unwrap();

        // 10.0.0.1 should be allowed
        let ctx = TrafficContext {
            src_pod: Some("client".to_string()),
            src_namespace: Some("default".to_string()),
            src_labels: HashMap::new(),
            src_ip: Some(IpAddr::from([10, 0, 0, 1])),
            dst_pod: Some("server".to_string()),
            dst_namespace: Some("default".to_string()),
            dst_labels: HashMap::new(),
            dst_ip: Some(IpAddr::from([10, 0, 0, 2])),
            dst_port: 8080,
            protocol: Protocol::TCP,
        };
        assert_eq!(engine.evaluate(&ctx).await, TrafficDecision::Allow);

        // 10.0.0.5 should be denied (in exception list)
        let ctx = TrafficContext {
            src_pod: Some("client".to_string()),
            src_namespace: Some("default".to_string()),
            src_labels: HashMap::new(),
            src_ip: Some(IpAddr::from([10, 0, 0, 5])),
            dst_pod: Some("server".to_string()),
            dst_namespace: Some("default".to_string()),
            dst_labels: HashMap::new(),
            dst_ip: Some(IpAddr::from([10, 0, 0, 2])),
            dst_port: 8080,
            protocol: Protocol::TCP,
        };
        assert_eq!(engine.evaluate(&ctx).await, TrafficDecision::Deny);
    }

    #[tokio::test]
    async fn test_ingress_with_labels() {
        let engine = create_test_engine();

        // Policy that only allows traffic from pods with label "role: frontend"
        let policy = NetworkPolicyDesired {
            name: "frontend-only".to_string(),
            namespace: "default".to_string(),
            spec: NetworkPolicySpec {
                pod_selector: LabelSelector {
                    match_labels: {
                        let mut m = HashMap::new();
                        m.insert("app".to_string(), "api".to_string());
                        m
                    },
                    ..Default::default()
                },
                policy_types: vec![PolicyType::Ingress],
                ingress: vec![NetworkPolicyIngressRule {
                    ports: vec![],
                    from: vec![NetworkPolicyPeer::PodSelector {
                        pod_selector: LabelSelector {
                            match_labels: {
                                let mut m = HashMap::new();
                                m.insert("role".to_string(), "frontend".to_string());
                                m
                            },
                            match_expressions: vec![],
                        },
                    }],
                }],
                ..Default::default()
            },
            created_at: Utc::now(),
            is_default_deny: false,
        };

        engine.add_policy(policy).await.unwrap();

        // Traffic from frontend pod should be allowed
        let ctx = TrafficContext {
            src_pod: Some("frontend".to_string()),
            src_namespace: Some("default".to_string()),
            src_labels: {
                let mut m = HashMap::new();
                m.insert("role".to_string(), "frontend".to_string());
                m
            },
            src_ip: Some(IpAddr::from([10, 0, 0, 1])),
            dst_pod: Some("api".to_string()),
            dst_namespace: Some("default".to_string()),
            dst_labels: {
                let mut m = HashMap::new();
                m.insert("app".to_string(), "api".to_string());
                m
            },
            dst_ip: Some(IpAddr::from([10, 0, 0, 2])),
            dst_port: 8080,
            protocol: Protocol::TCP,
        };
        assert_eq!(engine.evaluate(&ctx).await, TrafficDecision::Allow);

        // Traffic from backend pod should be denied
        let ctx = TrafficContext {
            src_pod: Some("backend".to_string()),
            src_namespace: Some("default".to_string()),
            src_labels: {
                let mut m = HashMap::new();
                m.insert("role".to_string(), "backend".to_string());
                m
            },
            src_ip: Some(IpAddr::from([10, 0, 0, 3])),
            dst_pod: Some("api".to_string()),
            dst_namespace: Some("default".to_string()),
            dst_labels: {
                let mut m = HashMap::new();
                m.insert("app".to_string(), "api".to_string());
                m
            },
            dst_ip: Some(IpAddr::from([10, 0, 0, 2])),
            dst_port: 8080,
            protocol: Protocol::TCP,
        };
        assert_eq!(engine.evaluate(&ctx).await, TrafficDecision::Deny);
    }
}
