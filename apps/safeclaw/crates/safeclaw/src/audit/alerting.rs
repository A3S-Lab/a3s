//! Real-time anomaly alerting for the audit event pipeline
//!
//! Monitors the audit event bus for critical events and rate-based
//! anomalies (e.g., a session generating too many events in a window).

use super::log::{AuditEvent, AuditSeverity};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Configuration for the alert monitor.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct AlertConfig {
    /// Max events per session within the sliding window before alerting
    pub session_rate_limit: usize,
    /// Sliding window duration in seconds
    pub window_seconds: u64,
    /// Minimum severity to track (events below this are ignored)
    pub min_severity: AuditSeverity,
    /// Enable the alert monitor
    pub enabled: bool,
}

impl Default for AlertConfig {
    fn default() -> Self {
        Self {
            session_rate_limit: 10,
            window_seconds: 60,
            min_severity: AuditSeverity::Warning,
            enabled: true,
        }
    }
}

/// The kind of alert raised.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertKind {
    /// A critical-severity audit event was observed.
    CriticalEvent,
    /// A session exceeded the event rate limit within the window.
    RateLimitExceeded,
}

/// A generated alert.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Alert {
    /// Alert kind
    pub kind: AlertKind,
    /// Session that triggered the alert
    pub session_id: String,
    /// Human-readable description
    pub description: String,
    /// Timestamp (milliseconds since epoch)
    pub timestamp: i64,
}

/// Monitors the audit event bus and generates alerts.
///
/// Maintains per-session sliding windows of event timestamps to detect
/// rate anomalies. Thread-safe via interior mutability.
pub struct AlertMonitor {
    config: AlertConfig,
    /// Per-session sliding window: session_id -> list of event timestamps (ms)
    counters: Arc<RwLock<HashMap<String, Vec<i64>>>>,
    /// Generated alerts
    alerts: Arc<RwLock<Vec<Alert>>>,
}

impl AlertMonitor {
    /// Create a new alert monitor with the given configuration.
    pub fn new(config: AlertConfig) -> Self {
        Self {
            config,
            counters: Arc::new(RwLock::new(HashMap::new())),
            alerts: Arc::new(RwLock::new(Vec::new())),
        }
    }

    /// Process a single audit event, generating alerts as needed.
    pub async fn process_event(&self, event: &AuditEvent) {
        // Skip events below minimum severity
        if !severity_gte(event.severity, self.config.min_severity) {
            return;
        }

        // Critical events generate an immediate alert
        if event.severity == AuditSeverity::Critical {
            let alert = Alert {
                kind: AlertKind::CriticalEvent,
                session_id: event.session_id.clone(),
                description: format!("Critical audit event: {}", event.description),
                timestamp: chrono::Utc::now().timestamp_millis(),
            };
            self.alerts.write().await.push(alert);
        }

        // Sliding window rate check
        let window_ms = (self.config.window_seconds * 1000) as i64;
        let now = chrono::Utc::now().timestamp_millis();
        let cutoff = now - window_ms;

        let mut counters = self.counters.write().await;
        let timestamps = counters
            .entry(event.session_id.clone())
            .or_insert_with(Vec::new);

        // Prune expired timestamps
        timestamps.retain(|&t| t > cutoff);
        timestamps.push(now);

        if timestamps.len() > self.config.session_rate_limit {
            let alert = Alert {
                kind: AlertKind::RateLimitExceeded,
                session_id: event.session_id.clone(),
                description: format!(
                    "Session {} exceeded rate limit: {} events in {}s window (limit: {})",
                    event.session_id,
                    timestamps.len(),
                    self.config.window_seconds,
                    self.config.session_rate_limit,
                ),
                timestamp: now,
            };
            self.alerts.write().await.push(alert);
        }
    }

    /// Spawn a background task that subscribes to the bus and processes events.
    pub fn spawn(self: &Arc<Self>, mut rx: broadcast::Receiver<AuditEvent>) {
        let monitor = Arc::clone(self);
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        monitor.process_event(&event).await;
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "Alert monitor lagged, skipped events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    /// Get recent alerts, newest first.
    pub async fn recent_alerts(&self, limit: usize) -> Vec<Alert> {
        let alerts = self.alerts.read().await;
        alerts.iter().rev().take(limit).cloned().collect()
    }

    /// Total number of alerts generated.
    pub async fn alert_count(&self) -> usize {
        self.alerts.read().await.len()
    }
}

/// Returns true if `a >= b` in the severity ordering.
fn severity_gte(a: AuditSeverity, b: AuditSeverity) -> bool {
    severity_rank(a) >= severity_rank(b)
}

fn severity_rank(s: AuditSeverity) -> u8 {
    match s {
        AuditSeverity::Info => 0,
        AuditSeverity::Warning => 1,
        AuditSeverity::High => 2,
        AuditSeverity::Critical => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::LeakageVector;

    fn make_event(session_id: &str, severity: AuditSeverity) -> AuditEvent {
        AuditEvent::new(
            session_id.to_string(),
            severity,
            LeakageVector::OutputChannel,
            format!("test event for {}", session_id),
        )
    }

    #[tokio::test]
    async fn test_critical_event_generates_alert() {
        let monitor = AlertMonitor::new(AlertConfig::default());
        let event = make_event("s1", AuditSeverity::Critical);

        monitor.process_event(&event).await;

        let alerts = monitor.recent_alerts(10).await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].kind, AlertKind::CriticalEvent);
        assert_eq!(alerts[0].session_id, "s1");
    }

    #[tokio::test]
    async fn test_info_events_ignored_with_default_config() {
        let monitor = AlertMonitor::new(AlertConfig::default());
        let event = make_event("s1", AuditSeverity::Info);

        monitor.process_event(&event).await;

        assert_eq!(monitor.alert_count().await, 0);
    }

    #[tokio::test]
    async fn test_rate_limit_exceeded() {
        let config = AlertConfig {
            session_rate_limit: 3,
            window_seconds: 60,
            min_severity: AuditSeverity::Warning,
            enabled: true,
        };
        let monitor = AlertMonitor::new(config);

        // Fire 4 events (exceeds limit of 3)
        for _ in 0..4 {
            monitor
                .process_event(&make_event("s1", AuditSeverity::Warning))
                .await;
        }

        let alerts = monitor.recent_alerts(10).await;
        let rate_alerts: Vec<_> = alerts
            .iter()
            .filter(|a| a.kind == AlertKind::RateLimitExceeded)
            .collect();
        assert!(!rate_alerts.is_empty());
        assert_eq!(rate_alerts[0].session_id, "s1");
    }

    #[tokio::test]
    async fn test_different_sessions_tracked_separately() {
        let config = AlertConfig {
            session_rate_limit: 2,
            window_seconds: 60,
            min_severity: AuditSeverity::Warning,
            enabled: true,
        };
        let monitor = AlertMonitor::new(config);

        // 2 events for s1 (at limit, no alert)
        monitor
            .process_event(&make_event("s1", AuditSeverity::Warning))
            .await;
        monitor
            .process_event(&make_event("s1", AuditSeverity::Warning))
            .await;

        // 2 events for s2 (at limit, no alert)
        monitor
            .process_event(&make_event("s2", AuditSeverity::High))
            .await;
        monitor
            .process_event(&make_event("s2", AuditSeverity::High))
            .await;

        assert_eq!(monitor.alert_count().await, 0);

        // 1 more for s1 → exceeds limit
        monitor
            .process_event(&make_event("s1", AuditSeverity::Warning))
            .await;

        let alerts = monitor.recent_alerts(10).await;
        assert_eq!(alerts.len(), 1);
        assert_eq!(alerts[0].session_id, "s1");
    }

    #[tokio::test]
    async fn test_recent_alerts_limit() {
        let monitor = AlertMonitor::new(AlertConfig::default());

        for i in 0..5 {
            let event = make_event(&format!("s{}", i), AuditSeverity::Critical);
            monitor.process_event(&event).await;
        }

        let alerts = monitor.recent_alerts(3).await;
        assert_eq!(alerts.len(), 3);
        // Newest first
        assert_eq!(alerts[0].session_id, "s4");
    }

    #[tokio::test]
    async fn test_alert_serialization() {
        let alert = Alert {
            kind: AlertKind::CriticalEvent,
            session_id: "sess-1".to_string(),
            description: "Test alert".to_string(),
            timestamp: 1000,
        };
        let json = serde_json::to_string(&alert).unwrap();
        assert!(json.contains("\"sessionId\""));
        assert!(json.contains("\"critical_event\""));
        let parsed: Alert = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.kind, AlertKind::CriticalEvent);
    }
}
