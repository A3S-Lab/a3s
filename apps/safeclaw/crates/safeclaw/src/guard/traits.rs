//! Trait definitions for the leakage prevention subsystem.
//!
//! All leakage components are defined as traits with default implementations,
//! allowing consumers to replace any component without modifying core logic.

use super::firewall::FirewallResult;
use super::injection::InjectionResult;
use super::interceptor::InterceptResult;
use super::sanitizer::SanitizeResult;
use super::segments::StructuredMessage;
use super::taint::TaintRegistry;
use crate::audit::AuditEvent;

// ---------------------------------------------------------------------------
// OutputSanitizer — scans agent output for tainted data and redacts it
// ---------------------------------------------------------------------------

/// Scans agent output for tainted data and redacts matches.
pub trait Sanitizer: Send + Sync {
    /// Scan and redact tainted data from output text.
    fn sanitize(&self, registry: &TaintRegistry, output: &str, session_id: &str) -> SanitizeResult;

    /// Quick check: does the output contain any tainted data?
    fn contains_leakage(&self, registry: &TaintRegistry, output: &str) -> bool;
}

// ---------------------------------------------------------------------------
// ToolInterceptor — blocks tool calls with tainted args or dangerous commands
// ---------------------------------------------------------------------------

/// Inspects tool calls for tainted arguments or dangerous commands.
pub trait Interceptor: Send + Sync {
    /// Check a tool call and decide whether to allow or block it.
    fn intercept(
        &self,
        registry: &TaintRegistry,
        tool_name: &str,
        arguments: &str,
        session_id: &str,
    ) -> InterceptResult;
}

// ---------------------------------------------------------------------------
// InjectionDetector — detects prompt injection attacks
// ---------------------------------------------------------------------------

/// Scans input for prompt injection patterns.
pub trait InjectionScanner: Send + Sync {
    /// Scan raw text input for injection patterns.
    fn scan(&self, input: &str, session_id: &str) -> InjectionResult;

    /// Scan structured message segments for injection patterns.
    fn scan_structured(&self, message: &StructuredMessage, session_id: &str) -> InjectionResult;
}

// ---------------------------------------------------------------------------
// NetworkFirewall — enforces egress network policy
// ---------------------------------------------------------------------------

/// Enforces network egress policy (whitelist/blacklist).
pub trait Firewall: Send + Sync {
    /// Check if a URL is allowed by the firewall policy.
    fn check_url(&self, url: &str, session_id: &str) -> FirewallResult;

    /// Check if a host:port is allowed by the firewall policy.
    fn check_host(&self, host: &str, port: u16, session_id: &str) -> FirewallResult;
}

// ---------------------------------------------------------------------------
// AuditSink — receives and stores audit events
// ---------------------------------------------------------------------------

/// Receives audit events for storage and processing.
#[async_trait::async_trait]
pub trait AuditSink: Send + Sync {
    /// Record a single audit event.
    async fn record(&self, event: AuditEvent);

    /// Record multiple audit events.
    async fn record_all(&self, events: Vec<AuditEvent>);

    /// Get recent events (up to `limit`).
    async fn recent(&self, limit: usize) -> Vec<AuditEvent>;

    /// Get events for a specific session.
    async fn by_session(&self, session_id: &str) -> Vec<AuditEvent>;
}
