//! Audit logging for leakage prevention events
//!
//! Generates structured audit events when sensitive data leakage
//! is detected or blocked. Events can be forwarded to NATS, logged,
//! or stored for compliance.
//!
//! **Threat model**: Supports forensics and compliance across all attack surfaces.
//! See `docs/threat-model.md` §5.

use serde::{Deserialize, Serialize};
use std::collections::VecDeque;

/// Severity level of an audit event
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AuditSeverity {
    /// Informational (e.g., normal classification)
    Info,
    /// Warning (e.g., sensitive data detected but handled)
    Warning,
    /// High (e.g., leakage attempt blocked)
    High,
    /// Critical (e.g., repeated leakage attempts)
    Critical,
}

/// The vector through which leakage was attempted
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeakageVector {
    /// AI output contained tainted data
    OutputChannel,
    /// Tool call arguments contained tainted data
    ToolCall,
    /// Dangerous command pattern detected
    DangerousCommand,
    /// Network exfiltration attempt
    NetworkExfil,
    /// File write with tainted data
    FileExfil,
    /// Channel authentication failure
    AuthFailure,
}

/// A structured audit event for leakage prevention
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEvent {
    /// Unique event ID
    pub id: String,
    /// Session that triggered the event
    pub session_id: String,
    /// Severity level
    pub severity: AuditSeverity,
    /// Leakage vector
    pub vector: LeakageVector,
    /// Human-readable description
    pub description: String,
    /// Taint labels involved in this event (for taint audit trail)
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub taint_labels: Vec<String>,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: i64,
}

impl AuditEvent {
    /// Create a new audit event
    pub fn new(
        session_id: String,
        severity: AuditSeverity,
        vector: LeakageVector,
        description: String,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            severity,
            vector,
            description,
            taint_labels: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    /// Create an audit event with taint labels for audit trail.
    pub fn with_taint_labels(
        session_id: String,
        severity: AuditSeverity,
        vector: LeakageVector,
        description: String,
        taint_labels: Vec<String>,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            severity,
            vector,
            description,
            taint_labels,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// In-memory audit log with bounded capacity
///
/// Stores recent audit events for querying. In production,
/// events would also be forwarded to NATS JetStream.
#[derive(Debug)]
pub struct AuditLog {
    /// Bounded event buffer
    events: VecDeque<AuditEvent>,
    /// Maximum number of events to retain
    capacity: usize,
    /// Total events recorded (including evicted)
    total_count: u64,
}

impl AuditLog {
    /// Create a new audit log with the given capacity
    pub fn new(capacity: usize) -> Self {
        Self {
            events: VecDeque::with_capacity(capacity),
            capacity,
            total_count: 0,
        }
    }

    /// Record an audit event
    pub fn record(&mut self, event: AuditEvent) {
        tracing::warn!(
            session_id = %event.session_id,
            severity = ?event.severity,
            vector = ?event.vector,
            "Leakage audit: {}",
            event.description
        );

        if self.events.len() >= self.capacity {
            self.events.pop_front();
        }
        self.events.push_back(event);
        self.total_count += 1;
    }

    /// Record multiple events
    pub fn record_all(&mut self, events: Vec<AuditEvent>) {
        for event in events {
            self.record(event);
        }
    }

    /// Get recent events (newest first)
    pub fn recent(&self, limit: usize) -> Vec<&AuditEvent> {
        self.events.iter().rev().take(limit).collect()
    }

    /// Get events for a specific session
    pub fn by_session(&self, session_id: &str) -> Vec<&AuditEvent> {
        self.events
            .iter()
            .filter(|e| e.session_id == session_id)
            .collect()
    }

    /// Get events by severity
    pub fn by_severity(&self, severity: AuditSeverity) -> Vec<&AuditEvent> {
        self.events
            .iter()
            .filter(|e| e.severity == severity)
            .collect()
    }

    /// Total number of events ever recorded
    pub fn total_count(&self) -> u64 {
        self.total_count
    }

    /// Number of events currently in the buffer
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Check if the log is empty
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Clear all events
    pub fn clear(&mut self) {
        self.events.clear();
    }
}

impl Default for AuditLog {
    fn default() -> Self {
        Self::new(10_000)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_event_creation() {
        let event = AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::High,
            LeakageVector::OutputChannel,
            "Tainted data in output".to_string(),
        );
        assert!(!event.id.is_empty());
        assert_eq!(event.session_id, "sess-1");
        assert_eq!(event.severity, AuditSeverity::High);
        assert!(event.timestamp > 0);
    }

    #[test]
    fn test_audit_event_serialization() {
        let event = AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::Critical,
            LeakageVector::ToolCall,
            "Blocked tool call".to_string(),
        );
        let json = serde_json::to_string(&event).unwrap();
        let parsed: AuditEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.session_id, "sess-1");
        // Verify camelCase
        assert!(json.contains("sessionId"));
    }

    #[test]
    fn test_audit_log_record_and_query() {
        let mut log = AuditLog::new(100);

        log.record(AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::High,
            LeakageVector::OutputChannel,
            "Event 1".to_string(),
        ));
        log.record(AuditEvent::new(
            "sess-2".to_string(),
            AuditSeverity::Warning,
            LeakageVector::ToolCall,
            "Event 2".to_string(),
        ));
        log.record(AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::Critical,
            LeakageVector::DangerousCommand,
            "Event 3".to_string(),
        ));

        assert_eq!(log.len(), 3);
        assert_eq!(log.total_count(), 3);

        // Query by session
        let sess1_events = log.by_session("sess-1");
        assert_eq!(sess1_events.len(), 2);

        // Query by severity
        let high_events = log.by_severity(AuditSeverity::High);
        assert_eq!(high_events.len(), 1);

        // Recent events (newest first)
        let recent = log.recent(2);
        assert_eq!(recent.len(), 2);
        assert_eq!(recent[0].description, "Event 3");
    }

    #[test]
    fn test_audit_log_capacity_eviction() {
        let mut log = AuditLog::new(3);

        for i in 0..5 {
            log.record(AuditEvent::new(
                "sess-1".to_string(),
                AuditSeverity::Info,
                LeakageVector::OutputChannel,
                format!("Event {}", i),
            ));
        }

        assert_eq!(log.len(), 3);
        assert_eq!(log.total_count(), 5);

        // Oldest events should be evicted
        let recent = log.recent(10);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].description, "Event 4");
    }

    #[test]
    fn test_audit_log_clear() {
        let mut log = AuditLog::new(100);
        log.record(AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::Info,
            LeakageVector::OutputChannel,
            "test".to_string(),
        ));
        assert!(!log.is_empty());

        log.clear();
        assert!(log.is_empty());
        assert_eq!(log.len(), 0);
    }

    #[test]
    fn test_audit_log_record_all() {
        let mut log = AuditLog::new(100);
        let events = vec![
            AuditEvent::new(
                "s1".to_string(),
                AuditSeverity::High,
                LeakageVector::OutputChannel,
                "e1".to_string(),
            ),
            AuditEvent::new(
                "s2".to_string(),
                AuditSeverity::Warning,
                LeakageVector::ToolCall,
                "e2".to_string(),
            ),
        ];
        log.record_all(events);
        assert_eq!(log.len(), 2);
    }

    #[test]
    fn test_severity_ordering() {
        // Just verify all variants serialize correctly
        for severity in [
            AuditSeverity::Info,
            AuditSeverity::Warning,
            AuditSeverity::High,
            AuditSeverity::Critical,
        ] {
            let json = serde_json::to_string(&severity).unwrap();
            let parsed: AuditSeverity = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, severity);
        }
    }

    #[test]
    fn test_leakage_vector_serialization() {
        for vector in [
            LeakageVector::OutputChannel,
            LeakageVector::ToolCall,
            LeakageVector::DangerousCommand,
            LeakageVector::NetworkExfil,
            LeakageVector::FileExfil,
            LeakageVector::AuthFailure,
        ] {
            let json = serde_json::to_string(&vector).unwrap();
            let parsed: LeakageVector = serde_json::from_str(&json).unwrap();
            assert_eq!(parsed, vector);
        }
    }

    #[test]
    fn test_audit_event_with_taint_labels() {
        let event = AuditEvent::with_taint_labels(
            "sess-1".to_string(),
            AuditSeverity::High,
            LeakageVector::OutputChannel,
            "Tainted data leaked".to_string(),
            vec!["pii:email".to_string(), "pii:phone".to_string()],
        );
        assert_eq!(event.taint_labels.len(), 2);
        assert!(event.taint_labels.contains(&"pii:email".to_string()));

        // Verify serialization includes taint_labels
        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("taintLabels"));
        assert!(json.contains("pii:email"));
    }

    #[test]
    fn test_audit_event_empty_taint_labels_skipped() {
        let event = AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::Info,
            LeakageVector::OutputChannel,
            "No taint".to_string(),
        );
        assert!(event.taint_labels.is_empty());

        // Empty taint_labels should be skipped in serialization
        let json = serde_json::to_string(&event).unwrap();
        assert!(!json.contains("taintLabels"));
    }
}
