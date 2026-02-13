//! Core event types for the a3s-event system
//!
//! All types use camelCase JSON serialization for wire compatibility.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A single event in the system
///
/// Events are published to subjects following the dot-separated convention:
/// `events.<category>.<topic>` (e.g., `events.market.forex.usd_cny`)
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    /// Unique event identifier (evt-<uuid>)
    pub id: String,

    /// Subject this event was published to
    pub subject: String,

    /// Top-level category for grouping (e.g., "market", "system")
    pub category: String,

    /// Event type identifier (e.g., "forex.rate_change", "deploy.completed")
    ///
    /// Used by schema registry to look up validation rules.
    /// Defaults to empty string for untyped events.
    #[serde(default)]
    pub event_type: String,

    /// Schema version for this event type (e.g., 1, 2, 3)
    ///
    /// Incremented when the payload schema changes.
    /// Defaults to 1 for new events.
    #[serde(default = "default_version")]
    pub version: u32,

    /// Event payload — arbitrary JSON data
    pub payload: serde_json::Value,

    /// Human-readable summary
    pub summary: String,

    /// Source system or service that produced this event
    pub source: String,

    /// Unix timestamp in milliseconds
    pub timestamp: u64,

    /// Optional key-value metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

fn default_version() -> u32 {
    1
}

impl Event {
    /// Create a new event with auto-generated id and timestamp
    pub fn new(
        subject: impl Into<String>,
        category: impl Into<String>,
        summary: impl Into<String>,
        source: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: format!("evt-{}", uuid::Uuid::new_v4()),
            subject: subject.into(),
            category: category.into(),
            event_type: String::new(),
            version: 1,
            payload,
            summary: summary.into(),
            source: source.into(),
            timestamp: now_millis(),
            metadata: HashMap::new(),
        }
    }

    /// Create a typed event with explicit event_type and version
    pub fn typed(
        subject: impl Into<String>,
        category: impl Into<String>,
        event_type: impl Into<String>,
        version: u32,
        summary: impl Into<String>,
        source: impl Into<String>,
        payload: serde_json::Value,
    ) -> Self {
        Self {
            id: format!("evt-{}", uuid::Uuid::new_v4()),
            subject: subject.into(),
            category: category.into(),
            event_type: event_type.into(),
            version,
            payload,
            summary: summary.into(),
            source: source.into(),
            timestamp: now_millis(),
            metadata: HashMap::new(),
        }
    }

    /// Add a metadata entry
    pub fn with_metadata(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.metadata.insert(key.into(), value.into());
        self
    }
}

/// A received event with delivery context
#[derive(Debug, Clone)]
pub struct ReceivedEvent {
    /// The event data
    pub event: Event,

    /// Provider-assigned sequence number
    pub sequence: u64,

    /// Number of delivery attempts
    pub num_delivered: u64,

    /// Stream/topic name
    pub stream: String,
}

/// Subscription filter for creating consumers
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscriptionFilter {
    /// Subscriber identifier (e.g., persona id)
    pub subscriber_id: String,

    /// Subject filter patterns (e.g., ["events.market.>", "events.system.>"])
    pub subjects: Vec<String>,

    /// Whether this is a durable subscription (survives reconnects)
    pub durable: bool,

    /// Provider-specific subscription options (optional)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub options: Option<SubscribeOptions>,
}

/// Event counts grouped by category
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct EventCounts {
    /// Counts per category
    pub categories: HashMap<String, u64>,

    /// Total event count
    pub total: u64,
}

/// Delivery policy for subscriptions
///
/// Controls where a new consumer starts reading from the stream.
/// Maps to provider-native delivery policies (e.g., NATS `DeliverPolicy`).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum DeliverPolicy {
    /// Deliver all available messages
    #[default]
    All,
    /// Deliver starting from the last message
    Last,
    /// Deliver only new messages published after subscription
    New,
    /// Deliver starting from a specific sequence number
    ByStartSequence { sequence: u64 },
    /// Deliver starting from a specific timestamp (Unix milliseconds)
    ByStartTime { timestamp: u64 },
    /// Deliver the last message per subject
    LastPerSubject,
}

/// Options for publishing events
///
/// Exposes provider-native publish capabilities. Unsupported options
/// are ignored by providers that don't support them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishOptions {
    /// Deduplication message ID (NATS: `Nats-Msg-Id` header)
    ///
    /// If set, the provider uses this to deduplicate messages within
    /// its deduplication window.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub msg_id: Option<String>,

    /// Expected last sequence number (optimistic concurrency)
    ///
    /// Publish fails if the stream's last sequence doesn't match.
    /// NATS: `Nats-Expected-Last-Sequence` header.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_sequence: Option<u64>,

    /// Publish timeout in seconds (overrides provider default)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// Options for creating subscriptions
///
/// Exposes provider-native consumer capabilities. Unsupported options
/// are ignored by providers that don't support them.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubscribeOptions {
    /// Maximum delivery attempts before giving up (NATS: `MaxDeliver`)
    ///
    /// After this many failed deliveries, the message is dropped or
    /// routed to a dead letter queue (if configured).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_deliver: Option<i64>,

    /// Backoff intervals in seconds between redelivery attempts
    ///
    /// NATS: maps to consumer `BackOff` durations.
    /// Example: `vec![1, 5, 30]` — retry after 1s, 5s, 30s.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub backoff_secs: Vec<u64>,

    /// Maximum number of unacknowledged messages in flight
    ///
    /// Provides backpressure — consumer won't receive new messages
    /// until pending acks drop below this limit.
    /// NATS: `MaxAckPending`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_ack_pending: Option<i64>,

    /// Where to start consuming from
    #[serde(default)]
    pub deliver_policy: DeliverPolicy,

    /// How long to wait for an ack before redelivery (seconds)
    ///
    /// NATS: `AckWait`. Default depends on provider.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub ack_wait_secs: Option<u64>,
}

/// Current time in Unix milliseconds
fn now_millis() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_event_creation() {
        let event = Event::new(
            "events.market.forex",
            "market",
            "USD/CNY rate change",
            "reuters",
            serde_json::json!({"rate": 7.35}),
        );

        assert!(event.id.starts_with("evt-"));
        assert_eq!(event.subject, "events.market.forex");
        assert_eq!(event.category, "market");
        assert_eq!(event.source, "reuters");
        assert!(event.timestamp > 0);
        assert!(event.metadata.is_empty());
    }

    #[test]
    fn test_event_with_metadata() {
        let event = Event::new(
            "events.system.deploy",
            "system",
            "Deployed v1.2",
            "ci",
            serde_json::json!({}),
        )
        .with_metadata("env", "production")
        .with_metadata("version", "1.2.0");

        assert_eq!(event.metadata.len(), 2);
        assert_eq!(event.metadata["env"], "production");
        assert_eq!(event.metadata["version"], "1.2.0");
    }

    #[test]
    fn test_event_serialization_roundtrip() {
        let event = Event::new(
            "events.market.forex",
            "market",
            "Rate change",
            "reuters",
            serde_json::json!({"rate": 7.35}),
        )
        .with_metadata("region", "asia");

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"subject\":\"events.market.forex\""));
        assert!(json.contains("\"category\":\"market\""));

        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.id, event.id);
        assert_eq!(parsed.subject, event.subject);
        assert_eq!(parsed.metadata["region"], "asia");
    }

    #[test]
    fn test_event_counts_default() {
        let counts = EventCounts::default();
        assert_eq!(counts.total, 0);
        assert!(counts.categories.is_empty());
    }

    #[test]
    fn test_subscription_filter_serialization() {
        let filter = SubscriptionFilter {
            subscriber_id: "financial-analyst".to_string(),
            subjects: vec!["events.market.>".to_string()],
            durable: true,
            options: None,
        };

        let json = serde_json::to_string(&filter).unwrap();
        assert!(json.contains("\"subscriberId\":\"financial-analyst\""));
        assert!(json.contains("\"durable\":true"));

        let parsed: SubscriptionFilter = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.subscriber_id, "financial-analyst");
        assert!(parsed.durable);
    }

    #[test]
    fn test_publish_options_default() {
        let opts = PublishOptions::default();
        assert!(opts.msg_id.is_none());
        assert!(opts.expected_sequence.is_none());
        assert!(opts.timeout_secs.is_none());
    }

    #[test]
    fn test_publish_options_serialization() {
        let opts = PublishOptions {
            msg_id: Some("dedup-123".to_string()),
            expected_sequence: Some(42),
            timeout_secs: Some(5),
        };

        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains("\"msgId\":\"dedup-123\""));
        assert!(json.contains("\"expectedSequence\":42"));
        assert!(json.contains("\"timeoutSecs\":5"));

        let parsed: PublishOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.msg_id.unwrap(), "dedup-123");
        assert_eq!(parsed.expected_sequence.unwrap(), 42);
    }

    #[test]
    fn test_publish_options_skip_none_fields() {
        let opts = PublishOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        assert!(!json.contains("msgId"));
        assert!(!json.contains("expectedSequence"));
        assert!(!json.contains("timeoutSecs"));
    }

    #[test]
    fn test_subscribe_options_default() {
        let opts = SubscribeOptions::default();
        assert!(opts.max_deliver.is_none());
        assert!(opts.backoff_secs.is_empty());
        assert!(opts.max_ack_pending.is_none());
        assert_eq!(opts.deliver_policy, DeliverPolicy::All);
        assert!(opts.ack_wait_secs.is_none());
    }

    #[test]
    fn test_subscribe_options_serialization() {
        let opts = SubscribeOptions {
            max_deliver: Some(5),
            backoff_secs: vec![1, 5, 30],
            max_ack_pending: Some(1000),
            deliver_policy: DeliverPolicy::New,
            ack_wait_secs: Some(30),
        };

        let json = serde_json::to_string(&opts).unwrap();
        assert!(json.contains("\"maxDeliver\":5"));
        assert!(json.contains("\"backoffSecs\":[1,5,30]"));
        assert!(json.contains("\"maxAckPending\":1000"));
        assert!(json.contains("\"ackWaitSecs\":30"));

        let parsed: SubscribeOptions = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.max_deliver.unwrap(), 5);
        assert_eq!(parsed.backoff_secs, vec![1, 5, 30]);
        assert_eq!(parsed.max_ack_pending.unwrap(), 1000);
        assert_eq!(parsed.deliver_policy, DeliverPolicy::New);
    }

    #[test]
    fn test_subscribe_options_skip_empty_fields() {
        let opts = SubscribeOptions::default();
        let json = serde_json::to_string(&opts).unwrap();
        assert!(!json.contains("maxDeliver"));
        assert!(!json.contains("backoffSecs"));
        assert!(!json.contains("maxAckPending"));
        assert!(!json.contains("ackWaitSecs"));
    }

    #[test]
    fn test_deliver_policy_variants() {
        let cases = vec![
            (DeliverPolicy::All, "All"),
            (DeliverPolicy::Last, "Last"),
            (DeliverPolicy::New, "New"),
            (DeliverPolicy::LastPerSubject, "LastPerSubject"),
        ];

        for (policy, _) in &cases {
            let json = serde_json::to_string(policy).unwrap();
            let parsed: DeliverPolicy = serde_json::from_str(&json).unwrap();
            assert_eq!(&parsed, policy);
        }
    }

    #[test]
    fn test_deliver_policy_by_start_sequence() {
        let policy = DeliverPolicy::ByStartSequence { sequence: 100 };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains("\"sequence\":100"));

        let parsed: DeliverPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeliverPolicy::ByStartSequence { sequence: 100 });
    }

    #[test]
    fn test_deliver_policy_by_start_time() {
        let ts = 1700000000000u64;
        let policy = DeliverPolicy::ByStartTime { timestamp: ts };
        let json = serde_json::to_string(&policy).unwrap();
        assert!(json.contains(&format!("\"timestamp\":{}", ts)));

        let parsed: DeliverPolicy = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, DeliverPolicy::ByStartTime { timestamp: ts });
    }

    #[test]
    fn test_event_default_version() {
        let event = Event::new(
            "events.test.a",
            "test",
            "Test",
            "test",
            serde_json::json!({}),
        );
        assert_eq!(event.version, 1);
        assert_eq!(event.event_type, "");
    }

    #[test]
    fn test_event_typed() {
        let event = Event::typed(
            "events.market.forex",
            "market",
            "forex.rate_change",
            2,
            "USD/CNY rate change",
            "reuters",
            serde_json::json!({"rate": 7.35}),
        );

        assert!(event.id.starts_with("evt-"));
        assert_eq!(event.event_type, "forex.rate_change");
        assert_eq!(event.version, 2);
        assert_eq!(event.category, "market");
    }

    #[test]
    fn test_event_version_serialization() {
        let event = Event::typed(
            "events.test.a",
            "test",
            "test.created",
            3,
            "Test",
            "test",
            serde_json::json!({}),
        );

        let json = serde_json::to_string(&event).unwrap();
        assert!(json.contains("\"eventType\":\"test.created\""));
        assert!(json.contains("\"version\":3"));

        let parsed: Event = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.event_type, "test.created");
        assert_eq!(parsed.version, 3);
    }

    #[test]
    fn test_event_version_backward_compat() {
        // Old events without event_type/version should deserialize with defaults
        let json = r#"{
            "id": "evt-123",
            "subject": "events.test.a",
            "category": "test",
            "payload": {},
            "summary": "Test",
            "source": "test",
            "timestamp": 1700000000000
        }"#;

        let event: Event = serde_json::from_str(json).unwrap();
        assert_eq!(event.event_type, "");
        assert_eq!(event.version, 1);
    }
}
