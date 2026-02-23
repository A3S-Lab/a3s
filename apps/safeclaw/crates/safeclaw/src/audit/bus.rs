//! Centralized audit event bus
//!
//! Provides a broadcast-based event distribution system that connects
//! all leakage detection producers (InjectionDetector, OutputSanitizer,
//! ToolInterceptor, NetworkFirewall) to consumers (global AuditLog,
//! per-session logs, AlertMonitor, a3s-event bridge).

use super::log::{AuditEvent, AuditLog};
use crate::guard::isolation::SessionIsolation;
use crate::guard::traits::AuditSink;
use std::sync::Arc;
use tokio::sync::{broadcast, RwLock};

/// Centralized audit event bus using `tokio::broadcast`.
///
/// All leakage detection components publish events here. Consumers
/// (global log, per-session forwarder, alert monitor, a3s-event bridge)
/// subscribe independently.
pub struct AuditEventBus {
    tx: broadcast::Sender<AuditEvent>,
    global_log: Arc<RwLock<AuditLog>>,
}

impl AuditEventBus {
    /// Create a new audit event bus.
    ///
    /// `capacity` is the broadcast channel buffer size.
    /// `global_log` is the shared audit log exposed via the REST API.
    pub fn new(capacity: usize, global_log: Arc<RwLock<AuditLog>>) -> Self {
        let (tx, _rx) = broadcast::channel(capacity);
        Self { tx, global_log }
    }

    /// Publish a single audit event.
    ///
    /// Records to the global log synchronously, then broadcasts
    /// to all subscribers. Broadcast failures (no receivers) are
    /// silently ignored — the global log is the source of truth.
    pub async fn publish(&self, event: AuditEvent) {
        self.global_log.write().await.record(event.clone());
        let _ = self.tx.send(event);
    }

    /// Publish multiple audit events.
    pub async fn publish_all(&self, events: Vec<AuditEvent>) {
        if events.is_empty() {
            return;
        }
        let mut log = self.global_log.write().await;
        for event in events {
            log.record(event.clone());
            let _ = self.tx.send(event);
        }
    }

    /// Subscribe to the event stream.
    ///
    /// Returns a broadcast receiver. Consumers that fall behind will
    /// see `RecvError::Lagged` and can skip to the latest events.
    pub fn subscribe(&self) -> broadcast::Receiver<AuditEvent> {
        self.tx.subscribe()
    }

    /// Spawn a background task that forwards bus events to per-session
    /// isolation logs.
    ///
    /// Each event is routed to its session's `AuditLog` inside
    /// `SessionIsolation`. Events for unknown sessions are ignored
    /// (the global log already has them).
    pub fn spawn_session_forwarder(&self, isolation: Arc<SessionIsolation>) {
        let mut rx = self.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        if let Some(guard) = isolation.audit_log(&event.session_id).await {
                            guard.write(|log| log.record(event)).await;
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "Session forwarder lagged, skipped events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        });
    }

    /// Get a reference to the global audit log.
    pub fn global_log(&self) -> &Arc<RwLock<AuditLog>> {
        &self.global_log
    }

    /// Spawn a background task that bridges audit events to `a3s-event::EventBus`.
    ///
    /// Each `AuditEvent` is converted to an `a3s_event::Event` and published
    /// under the subject `audit.<severity>.<vector>`. When the EventBus is
    /// backed by NATS, events flow to the NATS JetStream for distributed
    /// consumption. Falls back to in-memory when NATS is unavailable.
    pub fn spawn_event_bridge(&self, event_bus: Arc<a3s_event::EventBus>) {
        let mut rx = self.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(audit_event) => {
                        let topic = format!(
                            "{}.{}",
                            serde_json::to_value(&audit_event.severity)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_else(|| "unknown".to_string()),
                            serde_json::to_value(&audit_event.vector)
                                .ok()
                                .and_then(|v| v.as_str().map(String::from))
                                .unwrap_or_else(|| "unknown".to_string()),
                        );
                        let payload =
                            serde_json::to_value(&audit_event).unwrap_or(serde_json::Value::Null);

                        if let Err(e) = event_bus
                            .publish(
                                "audit",
                                &topic,
                                &audit_event.description,
                                "safeclaw",
                                payload,
                            )
                            .await
                        {
                            tracing::warn!(
                                error = %e,
                                event_id = %audit_event.id,
                                "Failed to bridge audit event to a3s-event"
                            );
                        }
                    }
                    Err(broadcast::error::RecvError::Lagged(n)) => {
                        tracing::warn!(skipped = n, "a3s-event bridge lagged, skipped events");
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            tracing::info!("a3s-event bridge stopped");
        });
    }
}

#[async_trait::async_trait]
impl AuditSink for AuditEventBus {
    async fn record(&self, event: AuditEvent) {
        self.publish(event).await;
    }

    async fn record_all(&self, events: Vec<AuditEvent>) {
        self.publish_all(events).await;
    }

    async fn recent(&self, limit: usize) -> Vec<AuditEvent> {
        self.global_log
            .read()
            .await
            .recent(limit)
            .into_iter()
            .cloned()
            .collect()
    }

    async fn by_session(&self, session_id: &str) -> Vec<AuditEvent> {
        self.global_log
            .read()
            .await
            .by_session(session_id)
            .into_iter()
            .cloned()
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditSeverity, LeakageVector};

    fn make_event(session_id: &str, severity: AuditSeverity) -> AuditEvent {
        AuditEvent::new(
            session_id.to_string(),
            severity,
            LeakageVector::OutputChannel,
            format!("test event for {}", session_id),
        )
    }

    #[tokio::test]
    async fn test_publish_records_to_global_log() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log.clone());

        bus.publish(make_event("s1", AuditSeverity::High)).await;

        let l = log.read().await;
        assert_eq!(l.len(), 1);
        assert_eq!(l.total_count(), 1);
    }

    #[tokio::test]
    async fn test_publish_all_records_multiple() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log.clone());

        let events = vec![
            make_event("s1", AuditSeverity::High),
            make_event("s2", AuditSeverity::Warning),
            make_event("s1", AuditSeverity::Critical),
        ];
        bus.publish_all(events).await;

        let l = log.read().await;
        assert_eq!(l.len(), 3);
        assert_eq!(l.by_session("s1").len(), 2);
    }

    #[tokio::test]
    async fn test_publish_all_empty_is_noop() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log.clone());

        bus.publish_all(vec![]).await;
        assert_eq!(log.read().await.len(), 0);
    }

    #[tokio::test]
    async fn test_subscribe_receives_events() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log);

        let mut rx = bus.subscribe();

        bus.publish(make_event("s1", AuditSeverity::Info)).await;

        let received = rx.recv().await.unwrap();
        assert_eq!(received.session_id, "s1");
    }

    #[tokio::test]
    async fn test_multiple_subscribers() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log);

        let mut rx1 = bus.subscribe();
        let mut rx2 = bus.subscribe();

        bus.publish(make_event("s1", AuditSeverity::Warning)).await;

        let e1 = rx1.recv().await.unwrap();
        let e2 = rx2.recv().await.unwrap();
        assert_eq!(e1.session_id, "s1");
        assert_eq!(e2.session_id, "s1");
    }

    #[tokio::test]
    async fn test_session_forwarder_routes_events() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log);
        let isolation = Arc::new(SessionIsolation::default());

        // Initialize a session so the forwarder has a target
        isolation.init_session("s1").await;

        bus.spawn_session_forwarder(isolation.clone());

        // Give the spawned task time to subscribe
        tokio::task::yield_now().await;

        bus.publish(make_event("s1", AuditSeverity::High)).await;

        // Allow the forwarder to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let guard = isolation.audit_log("s1").await.unwrap();
        let count = guard.read(|log| log.len()).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_event_bridge_forwards_to_memory_provider() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log);

        let provider = a3s_event::MemoryProvider::default();
        let event_bus = Arc::new(a3s_event::EventBus::new(provider));

        bus.spawn_event_bridge(event_bus.clone());

        // Give the spawned task time to subscribe
        tokio::task::yield_now().await;

        bus.publish(make_event("s1", AuditSeverity::High)).await;

        // Allow the bridge to process
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        // Verify the event was published to a3s-event
        let counts = event_bus.counts(100).await.unwrap();
        assert_eq!(counts.total, 1);
    }

    #[tokio::test]
    async fn test_event_bridge_subject_format() {
        let log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = AuditEventBus::new(16, log);

        let provider = a3s_event::MemoryProvider::default();
        let event_bus = Arc::new(a3s_event::EventBus::new(provider));

        bus.spawn_event_bridge(event_bus.clone());
        tokio::task::yield_now().await;

        // Publish events with different severities/vectors
        bus.publish(AuditEvent::new(
            "s1".to_string(),
            AuditSeverity::Critical,
            LeakageVector::NetworkExfil,
            "exfil attempt".to_string(),
        ))
        .await;

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let counts = event_bus.counts(100).await.unwrap();
        assert_eq!(counts.total, 1);
        // Event should be in the "audit" category
        assert_eq!(*counts.categories.get("audit").unwrap_or(&0), 1);
    }
}
