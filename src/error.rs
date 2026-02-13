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
