//! Cross-session correlator (P4).
//!
//! An async background task that observes events from all sessions in the
//! same agent tree and detects coordinated attack patterns that are
//! individually below the detection threshold.
//!
//! # Design principles
//!
//! - **Never in the blocking path**: events are sent via a bounded `mpsc`
//!   channel.  If the channel is full, the event is silently dropped —
//!   missing a correlation is preferable to blocking an agent turn.
//! - **Emit, don't block**: when a pattern fires, the correlator logs a
//!   structured warning via `tracing`.  Optionally it sends an "alert"
//!   notification through a broadcast channel so other components can react
//!   (e.g., lower Phase 2 thresholds in related sessions).
//! - **Lightweight state**: a fixed-size ring buffer of recent events.
//!   No persistence, no external deps.

use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::sync::mpsc;
use tracing;

// ── Event types ───────────────────────────────────────────────────────────────

/// Event types observable by the correlator.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CorrelatedEventType {
    ToolCall { tool: String },
    NetworkRequest { host: String },
    FileWrite { path: String },
    HighSuspicion,
}

/// A single security-relevant event from any session.
#[derive(Debug, Clone)]
pub struct CorrelatedEvent {
    pub session_id: String,
    pub event_type: CorrelatedEventType,
    pub timestamp: Instant,
}

// ── Alert ─────────────────────────────────────────────────────────────────────

/// A cross-session pattern alert emitted by the correlator.
#[derive(Debug, Clone)]
pub struct CorrelatorAlert {
    pub pattern: &'static str,
    pub description: String,
    pub involved_sessions: Vec<String>,
}

// ── CrossSessionCorrelator ────────────────────────────────────────────────────

/// Async cross-session correlator.
///
/// Call [`CrossSessionCorrelator::submit`] (non-blocking, fire-and-forget)
/// from the sentinel hook handler to feed events.  Alerts are emitted via
/// `tracing::warn!` and optionally sent to `alert_tx`.
pub struct CrossSessionCorrelator {
    event_tx: mpsc::Sender<CorrelatedEvent>,
}

impl CrossSessionCorrelator {
    /// Spawn the correlator background task and return a handle.
    pub fn spawn(
        buffer_size: usize,
        event_window: Duration,
        alert_tx: Option<tokio::sync::broadcast::Sender<CorrelatorAlert>>,
    ) -> Self {
        let (event_tx, event_rx) = mpsc::channel(buffer_size);
        tokio::spawn(run_correlator(event_rx, event_window, alert_tx));
        Self { event_tx }
    }

    /// Submit an event for correlation (non-blocking; drops if buffer full).
    pub fn submit(&self, event: CorrelatedEvent) {
        // `try_send` never blocks — drops silently on full channel.
        let _ = self.event_tx.try_send(event);
    }
}

// ── Background worker ─────────────────────────────────────────────────────────

async fn run_correlator(
    mut rx: mpsc::Receiver<CorrelatedEvent>,
    window: Duration,
    alert_tx: Option<tokio::sync::broadcast::Sender<CorrelatorAlert>>,
) {
    let mut ring: VecDeque<CorrelatedEvent> = VecDeque::with_capacity(1024);

    while let Some(event) = rx.recv().await {
        let now = Instant::now();

        // Evict events outside the correlation window.
        ring.retain(|e| now.duration_since(e.timestamp) < window);
        ring.push_back(event);

        // Run all pattern detectors against the current ring.
        for alert in detect_patterns(&ring) {
            tracing::warn!(
                pattern = alert.pattern,
                sessions = ?alert.involved_sessions,
                "Sentinel cross-session alert: {}",
                alert.description,
            );
            if let Some(ref tx) = alert_tx {
                let _ = tx.send(alert);
            }
        }
    }
}

// ── Pattern detectors ─────────────────────────────────────────────────────────

fn detect_patterns(ring: &VecDeque<CorrelatedEvent>) -> Vec<CorrelatorAlert> {
    let mut alerts = Vec::new();

    if let Some(a) = detect_fan_out_writes(ring) {
        alerts.push(a);
    }
    if let Some(a) = detect_suspicion_burst(ring) {
        alerts.push(a);
    }
    if let Some(a) = detect_network_fan_out(ring) {
        alerts.push(a);
    }

    alerts
}

/// Detect multiple distinct sessions writing files in a short window
/// (coordinated exfiltration pattern).
fn detect_fan_out_writes(ring: &VecDeque<CorrelatedEvent>) -> Option<CorrelatorAlert> {
    const MIN_SESSIONS: usize = 3;

    let mut sessions: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for e in ring {
        if matches!(e.event_type, CorrelatedEventType::FileWrite { .. }) {
            sessions.insert(&e.session_id);
        }
    }
    if sessions.len() >= MIN_SESSIONS {
        Some(CorrelatorAlert {
            pattern: "fan-out-write",
            description: format!(
                "{} sessions performing concurrent file writes (possible coordinated exfiltration)",
                sessions.len()
            ),
            involved_sessions: sessions.into_iter().map(str::to_string).collect(),
        })
    } else {
        None
    }
}

/// Detect a burst of high-suspicion events across multiple sessions.
fn detect_suspicion_burst(ring: &VecDeque<CorrelatedEvent>) -> Option<CorrelatorAlert> {
    const MIN_SESSIONS: usize = 3;

    let mut sessions: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for e in ring {
        if e.event_type == CorrelatedEventType::HighSuspicion {
            sessions.insert(&e.session_id);
        }
    }
    if sessions.len() >= MIN_SESSIONS {
        Some(CorrelatorAlert {
            pattern: "suspicion-burst",
            description: format!(
                "{} sessions simultaneously flagged as high-suspicion",
                sessions.len()
            ),
            involved_sessions: sessions.into_iter().map(str::to_string).collect(),
        })
    } else {
        None
    }
}

/// Detect multiple sessions making requests to different external hosts
/// (distributed data scatter pattern).
fn detect_network_fan_out(ring: &VecDeque<CorrelatedEvent>) -> Option<CorrelatorAlert> {
    const MIN_UNIQUE_HOSTS: usize = 5;
    const MIN_SESSIONS: usize = 2;

    let mut by_session: std::collections::HashMap<&str, std::collections::HashSet<&str>> =
        std::collections::HashMap::new();

    for e in ring {
        if let CorrelatedEventType::NetworkRequest { ref host } = e.event_type {
            by_session.entry(&e.session_id).or_default().insert(host.as_str());
        }
    }

    let unique_hosts: std::collections::HashSet<&str> =
        by_session.values().flatten().copied().collect();
    let involved: Vec<String> = by_session.keys().map(|s| s.to_string()).collect();

    if involved.len() >= MIN_SESSIONS && unique_hosts.len() >= MIN_UNIQUE_HOSTS {
        Some(CorrelatorAlert {
            pattern: "network-fan-out",
            description: format!(
                "{} sessions making requests to {} distinct hosts",
                involved.len(),
                unique_hosts.len()
            ),
            involved_sessions: involved,
        })
    } else {
        None
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn write_event(session_id: &str) -> CorrelatedEvent {
        CorrelatedEvent {
            session_id: session_id.to_string(),
            event_type: CorrelatedEventType::FileWrite { path: "/tmp/x".to_string() },
            timestamp: Instant::now(),
        }
    }

    fn suspicion_event(session_id: &str) -> CorrelatedEvent {
        CorrelatedEvent {
            session_id: session_id.to_string(),
            event_type: CorrelatedEventType::HighSuspicion,
            timestamp: Instant::now(),
        }
    }

    #[test]
    fn test_fan_out_write_fires_at_threshold() {
        let ring: VecDeque<_> = ["s1", "s2", "s3"].iter().map(|s| write_event(s)).collect();
        let alerts = detect_patterns(&ring);
        assert!(alerts.iter().any(|a| a.pattern == "fan-out-write"));
    }

    #[test]
    fn test_fan_out_write_below_threshold() {
        let ring: VecDeque<_> = ["s1", "s2"].iter().map(|s| write_event(s)).collect();
        let alerts = detect_patterns(&ring);
        assert!(!alerts.iter().any(|a| a.pattern == "fan-out-write"));
    }

    #[test]
    fn test_suspicion_burst_fires() {
        let ring: VecDeque<_> =
            ["s1", "s2", "s3"].iter().map(|s| suspicion_event(s)).collect();
        let alerts = detect_patterns(&ring);
        assert!(alerts.iter().any(|a| a.pattern == "suspicion-burst"));
    }

    #[test]
    fn test_network_fan_out_fires() {
        let ring: VecDeque<_> = (0..5)
            .flat_map(|host_i| {
                let host = format!("host{host_i}.com");
                ["s1", "s2"].iter().map(move |s| CorrelatedEvent {
                    session_id: s.to_string(),
                    event_type: CorrelatedEventType::NetworkRequest { host: host.clone() },
                    timestamp: Instant::now(),
                })
            })
            .collect();
        let alerts = detect_patterns(&ring);
        assert!(alerts.iter().any(|a| a.pattern == "network-fan-out"));
    }
}
