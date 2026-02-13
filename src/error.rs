//! Error types for a3s-event

use thiserror::Error;

/// Errors that can occur in the event system
#[derive(Debug, Error)]
pub enum EventError {
    /// Provider connection failure
    #[error("Connection error: {0}")]
    Connection(String),

    /// Provider-specific backend error (JetStream, Redis, etc.)
    #[error("Provider error: {0}")]
    JetStream(String),

    /// Publish failure
    #[error("Failed to publish event to subject '{subject}': {reason}")]
    Publish {
        subject: String,
        reason: String,
    },

    /// Subscribe failure
    #[error("Failed to subscribe to subject '{subject}': {reason}")]
    Subscribe {
        subject: String,
        reason: String,
    },

    /// Serialization/deserialization failure
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// Event not found
    #[error("Event not found: {0}")]
    NotFound(String),

    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Stream/topic creation or management error
    #[error("Stream error: {0}")]
    Stream(String),

    /// Consumer/subscription creation or management error
    #[error("Consumer error: {0}")]
    Consumer(String),

    /// Acknowledgement failure
    #[error("Failed to acknowledge message: {0}")]
    Ack(String),

    /// Timeout
    #[error("Operation timed out: {0}")]
    Timeout(String),

    /// Provider not supported or not available
    #[error("Provider error: {0}")]
    Provider(String),

    /// Schema validation failure
    #[error("Schema validation failed for event type '{event_type}' v{version}: {reason}")]
    SchemaValidation {
        event_type: String,
        version: u32,
        reason: String,
    },
}

/// Result type alias for event operations
pub type Result<T> = std::result::Result<T, EventError>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_connection_error_display() {
        let err = EventError::Connection("refused".to_string());
        assert_eq!(err.to_string(), "Connection error: refused");
    }

    #[test]
    fn test_publish_error_display() {
        let err = EventError::Publish {
            subject: "events.test.a".to_string(),
            reason: "timeout".to_string(),
        };
        assert!(err.to_string().contains("events.test.a"));
        assert!(err.to_string().contains("timeout"));
    }

    #[test]
    fn test_subscribe_error_display() {
        let err = EventError::Subscribe {
            subject: "events.market.>".to_string(),
            reason: "consumer limit".to_string(),
        };
        assert!(err.to_string().contains("events.market.>"));
    }

    #[test]
    fn test_schema_validation_error_display() {
        let err = EventError::SchemaValidation {
            event_type: "forex.rate".to_string(),
            version: 2,
            reason: "Missing required field 'rate'".to_string(),
        };
        let msg = err.to_string();
        assert!(msg.contains("forex.rate"));
        assert!(msg.contains("v2"));
        assert!(msg.contains("rate"));
    }

    #[test]
    fn test_not_found_error() {
        let err = EventError::NotFound("sub-123".to_string());
        assert!(err.to_string().contains("sub-123"));
    }

    #[test]
    fn test_timeout_error() {
        let err = EventError::Timeout("publish ack".to_string());
        assert!(err.to_string().contains("publish ack"));
    }

    #[test]
    fn test_serialization_error_from() {
        let json_err = serde_json::from_str::<String>("invalid").unwrap_err();
        let err: EventError = json_err.into();
        assert!(matches!(err, EventError::Serialization(_)));
    }
}
