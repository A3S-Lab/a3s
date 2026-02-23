//! Channel adapter trait and common types

use super::auth::ChannelAuth;
use super::message::{InboundMessage, OutboundMessage};
use crate::error::Result;
use async_trait::async_trait;
use tokio::sync::mpsc;

/// Events from a channel
#[derive(Debug, Clone)]
pub enum ChannelEvent {
    /// New message received
    Message(InboundMessage),
    /// Message edited
    MessageEdited {
        channel: String,
        message_id: String,
        new_content: String,
    },
    /// Message deleted
    MessageDeleted { channel: String, message_id: String },
    /// User started typing
    Typing {
        channel: String,
        chat_id: String,
        user_id: String,
    },
    /// User joined
    UserJoined {
        channel: String,
        chat_id: String,
        user_id: String,
    },
    /// User left
    UserLeft {
        channel: String,
        chat_id: String,
        user_id: String,
    },
    /// Channel connected
    Connected { channel: String },
    /// Channel disconnected
    Disconnected { channel: String, reason: String },
    /// Error occurred
    Error { channel: String, error: String },
}

/// Trait for channel adapters
#[async_trait]
pub trait ChannelAdapter: Send + Sync {
    /// Get the channel name
    fn name(&self) -> &str;

    /// Start the channel adapter
    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()>;

    /// Stop the channel adapter
    async fn stop(&self) -> Result<()>;

    /// Send a message
    async fn send_message(&self, message: OutboundMessage) -> Result<String>;

    /// Send typing indicator
    async fn send_typing(&self, chat_id: &str) -> Result<()>;

    /// Edit a message
    async fn edit_message(&self, chat_id: &str, message_id: &str, content: &str) -> Result<()>;

    /// Edit a message with a custom card JSON (for interactive card updates)
    async fn edit_message_card(
        &self,
        _chat_id: &str,
        message_id: &str,
        card: &serde_json::Value,
    ) -> Result<()> {
        // Default: fall back to edit_message with a text representation
        self.edit_message(_chat_id, message_id, &card.to_string())
            .await
    }

    /// Delete a message
    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<()>;

    /// Check if the adapter is connected
    fn is_connected(&self) -> bool;

    /// Get the channel authenticator for webhook verification.
    ///
    /// Returns `None` if the adapter does not use webhook-based auth
    /// (e.g., long-polling adapters like Telegram).
    fn auth(&self) -> Option<&dyn ChannelAuth> {
        None
    }
}

/// Channel adapter status
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdapterStatus {
    /// Not started
    Stopped,
    /// Starting up
    Starting,
    /// Running and connected
    Running,
    /// Reconnecting after disconnect
    Reconnecting,
    /// Stopping
    Stopping,
    /// Error state
    Error,
}

/// Base implementation helper for channel adapters
pub struct AdapterBase {
    name: String,
    status: std::sync::atomic::AtomicU8,
}

impl AdapterBase {
    /// Create a new adapter base
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            status: std::sync::atomic::AtomicU8::new(AdapterStatus::Stopped as u8),
        }
    }

    /// Get the adapter name
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Get current status
    pub fn status(&self) -> AdapterStatus {
        match self.status.load(std::sync::atomic::Ordering::SeqCst) {
            0 => AdapterStatus::Stopped,
            1 => AdapterStatus::Starting,
            2 => AdapterStatus::Running,
            3 => AdapterStatus::Reconnecting,
            4 => AdapterStatus::Stopping,
            _ => AdapterStatus::Error,
        }
    }

    /// Set status
    pub fn set_status(&self, status: AdapterStatus) {
        self.status
            .store(status as u8, std::sync::atomic::Ordering::SeqCst);
    }

    /// Check if running
    pub fn is_running(&self) -> bool {
        self.status() == AdapterStatus::Running
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_adapter_base() {
        let base = AdapterBase::new("test");
        assert_eq!(base.name(), "test");
        assert_eq!(base.status(), AdapterStatus::Stopped);

        base.set_status(AdapterStatus::Running);
        assert!(base.is_running());
    }

    #[test]
    fn test_channel_event() {
        let msg = InboundMessage::new("telegram", "user123", "chat456", "Hello!");
        let event = ChannelEvent::Message(msg);

        assert!(matches!(event, ChannelEvent::Message(_)));
    }
}
