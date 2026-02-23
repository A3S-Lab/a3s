//! Unified privacy + protection pipeline
//!
//! `PrivacyPipeline` bundles the output-side protection components
//! (`SessionIsolation`, `InjectionDetector`, `NetworkFirewall`, `AuditEventBus`)
//! behind a single, focused API.  `SessionManager` depends on this type
//! rather than importing those four guard/audit types directly.

use crate::audit::AuditEventBus;
use crate::guard::{
    FirewallResult, InjectionDetector, InjectionResult, InjectionVerdict, InterceptResult,
    NetworkFirewall, NetworkPolicy, OutputSanitizer, SanitizeResult, SessionIsolation,
    TaintRegistry, ToolInterceptor,
};
use std::sync::Arc;

/// Unified protection pipeline for a SafeClaw instance.
///
/// Owns the four cross-cutting protection components and exposes
/// composite operations so callers do not have to coordinate them manually.
pub struct PrivacyPipeline {
    isolation: Arc<SessionIsolation>,
    injection_detector: Arc<InjectionDetector>,
    network_firewall: Arc<NetworkFirewall>,
    audit_bus: Arc<AuditEventBus>,
}

impl PrivacyPipeline {
    /// Create a new pipeline with the given network policy and shared audit bus.
    pub fn new(network_policy: NetworkPolicy, audit_bus: Arc<AuditEventBus>) -> Self {
        Self {
            isolation: Arc::new(SessionIsolation::default()),
            injection_detector: Arc::new(InjectionDetector::new()),
            network_firewall: Arc::new(NetworkFirewall::new(network_policy)),
            audit_bus,
        }
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    pub fn isolation(&self) -> &Arc<SessionIsolation> {
        &self.isolation
    }

    pub fn audit_bus(&self) -> &Arc<AuditEventBus> {
        &self.audit_bus
    }

    pub fn injection_detector(&self) -> &Arc<InjectionDetector> {
        &self.injection_detector
    }

    pub fn network_firewall(&self) -> &Arc<NetworkFirewall> {
        &self.network_firewall
    }

    // -----------------------------------------------------------------------
    // Composite operations
    // -----------------------------------------------------------------------

    /// Sanitize AI output for a session.
    ///
    /// Looks up the per-session taint registry and auto-redacts any matches,
    /// then publishes resulting audit events to the shared bus.
    pub async fn sanitize_output(&self, session_id: &str, output: &str) -> SanitizeResult {
        let result = if let Some(guard) = self.isolation.registry(session_id).await {
            guard
                .read(|registry| OutputSanitizer::sanitize(registry, output, session_id))
                .await
                .unwrap_or_else(|| {
                    OutputSanitizer::sanitize(&TaintRegistry::default(), output, session_id)
                })
        } else {
            OutputSanitizer::sanitize(&TaintRegistry::default(), output, session_id)
        };
        if !result.audit_events.is_empty() {
            self.audit_bus
                .publish_all(result.audit_events.clone())
                .await;
        }
        result
    }

    /// Intercept a tool call for a session.
    ///
    /// Checks tool arguments for tainted data and dangerous command patterns,
    /// then publishes resulting audit events to the shared bus.
    pub async fn intercept_tool_call(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &str,
    ) -> InterceptResult {
        let result = if let Some(guard) = self.isolation.registry(session_id).await {
            guard
                .read(|registry| {
                    ToolInterceptor::intercept(registry, tool_name, arguments, session_id)
                })
                .await
                .unwrap_or_else(|| {
                    ToolInterceptor::intercept(
                        &TaintRegistry::default(),
                        tool_name,
                        arguments,
                        session_id,
                    )
                })
        } else {
            ToolInterceptor::intercept(&TaintRegistry::default(), tool_name, arguments, session_id)
        };
        if !result.audit_events.is_empty() {
            self.audit_bus
                .publish_all(result.audit_events.clone())
                .await;
        }
        result
    }

    /// Check a URL against the network firewall.
    ///
    /// Publishes any resulting audit event to the shared bus.
    pub async fn check_firewall(&self, url: &str, session_id: &str) -> FirewallResult {
        let result = self.network_firewall.check_url(url, session_id);
        if let Some(ref event) = result.audit_event {
            self.audit_bus.publish(event.clone()).await;
        }
        result
    }

    /// Scan content for prompt injection.
    ///
    /// Publishes audit events for `Blocked` and `Suspicious` verdicts.
    /// Returns the full result; the caller decides whether to reject.
    pub async fn check_injection(&self, content: &str, session_id: &str) -> InjectionResult {
        let result = self.injection_detector.scan(content, session_id);
        if result.verdict == InjectionVerdict::Blocked
            || result.verdict == InjectionVerdict::Suspicious
        {
            if !result.audit_events.is_empty() {
                self.audit_bus
                    .publish_all(result.audit_events.clone())
                    .await;
            }
        }
        result
    }
}
