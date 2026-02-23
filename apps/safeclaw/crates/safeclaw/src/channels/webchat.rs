//! WebChat channel adapter for browser-based chat

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{InboundMessage, OutboundMessage};
use crate::config::WebChatConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// WebChat session
#[derive(Debug, Clone)]
pub struct WebChatSession {
    /// Session ID
    pub id: String,
    /// User ID (if authenticated)
    pub user_id: Option<String>,
    /// Connection timestamp
    pub connected_at: i64,
    /// Last activity timestamp
    pub last_activity: i64,
}

/// WebChat channel adapter
pub struct WebChatAdapter {
    config: WebChatConfig,
    base: AdapterBase,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
    sessions: Arc<RwLock<HashMap<String, WebChatSession>>>,
    /// Outbound message senders for each session
    session_senders: Arc<RwLock<HashMap<String, mpsc::Sender<OutboundMessage>>>>,
}

impl WebChatAdapter {
    /// Create a new WebChat adapter
    pub fn new(config: WebChatConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("webchat"),
            event_tx: Arc::new(RwLock::new(None)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            session_senders: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Register a new WebSocket session
    pub async fn register_session(
        &self,
        session_id: &str,
        user_id: Option<String>,
        sender: mpsc::Sender<OutboundMessage>,
    ) -> Result<()> {
        let now = chrono::Utc::now().timestamp_millis();
        let session = WebChatSession {
            id: session_id.to_string(),
            user_id,
            connected_at: now,
            last_activity: now,
        };

        self.sessions
            .write()
            .await
            .insert(session_id.to_string(), session);
        self.session_senders
            .write()
            .await
            .insert(session_id.to_string(), sender);

        tracing::info!("WebChat session registered: {}", session_id);

        Ok(())
    }

    /// Unregister a WebSocket session
    pub async fn unregister_session(&self, session_id: &str) {
        self.sessions.write().await.remove(session_id);
        self.session_senders.write().await.remove(session_id);

        tracing::info!("WebChat session unregistered: {}", session_id);
    }

    /// Handle incoming message from WebSocket
    pub async fn handle_message(&self, session_id: &str, content: &str) -> Result<()> {
        // Update last activity
        if let Some(session) = self.sessions.write().await.get_mut(session_id) {
            session.last_activity = chrono::Utc::now().timestamp_millis();
        }

        // Create inbound message
        let user_id = self
            .sessions
            .read()
            .await
            .get(session_id)
            .and_then(|s| s.user_id.clone())
            .unwrap_or_else(|| session_id.to_string());

        let message = InboundMessage::new("webchat", &user_id, session_id, content).as_dm();

        // Send to event handler
        if let Some(tx) = self.event_tx.read().await.as_ref() {
            tx.send(ChannelEvent::Message(message))
                .await
                .map_err(|e| Error::Channel(format!("Failed to send event: {}", e)))?;
        }

        Ok(())
    }

    /// Get active session count
    pub async fn session_count(&self) -> usize {
        self.sessions.read().await.len()
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<WebChatSession> {
        self.sessions.read().await.get(session_id).cloned()
    }
}

#[async_trait]
impl ChannelAdapter for WebChatAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        if !self.config.enabled {
            return Err(Error::Channel("WebChat is not enabled".to_string()));
        }

        self.base.set_status(AdapterStatus::Starting);

        *self.event_tx.write().await = Some(event_tx.clone());

        // Send connected event
        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "webchat".to_string(),
            })
            .await;

        self.base.set_status(AdapterStatus::Running);

        tracing::info!("WebChat adapter started");

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.base.set_status(AdapterStatus::Stopping);

        // Clear all sessions
        self.sessions.write().await.clear();
        self.session_senders.write().await.clear();

        // Send disconnected event
        if let Some(tx) = self.event_tx.read().await.as_ref() {
            let _ = tx
                .send(ChannelEvent::Disconnected {
                    channel: "webchat".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);

        tracing::info!("WebChat adapter stopped");

        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("WebChat adapter not running".to_string()));
        }

        let session_id = &message.chat_id;

        // Get session sender
        let sender = self
            .session_senders
            .read()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| Error::Channel(format!("Session {} not found", session_id)))?;

        // Send message
        sender
            .send(message)
            .await
            .map_err(|e| Error::Channel(format!("Failed to send message: {}", e)))?;

        Ok(format!("wc-msg-{}", uuid::Uuid::new_v4()))
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("WebChat adapter not running".to_string()));
        }

        // WebChat typing is handled via WebSocket
        tracing::debug!("Sending typing indicator to WebChat session {}", chat_id);

        Ok(())
    }

    async fn edit_message(&self, _chat_id: &str, _message_id: &str, _content: &str) -> Result<()> {
        // WebChat doesn't support message editing in this implementation
        Err(Error::Channel(
            "Message editing not supported for WebChat".to_string(),
        ))
    }

    async fn delete_message(&self, _chat_id: &str, _message_id: &str) -> Result<()> {
        // WebChat doesn't support message deletion in this implementation
        Err(Error::Channel(
            "Message deletion not supported for WebChat".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> WebChatConfig {
        WebChatConfig {
            enabled: true,
            require_auth: false,
            allowed_origins: vec!["*".to_string()],
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = WebChatAdapter::new(config);

        assert_eq!(adapter.name(), "webchat");
        assert!(!adapter.is_connected());
    }

    #[tokio::test]
    async fn test_session_management() {
        let config = create_test_config();
        let adapter = WebChatAdapter::new(config);
        let (tx, _rx) = mpsc::channel(10);

        adapter
            .register_session("session-1", Some("user-1".to_string()), tx)
            .await
            .unwrap();

        assert_eq!(adapter.session_count().await, 1);

        let session = adapter.get_session("session-1").await.unwrap();
        assert_eq!(session.user_id, Some("user-1".to_string()));

        adapter.unregister_session("session-1").await;
        assert_eq!(adapter.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        let config = create_test_config();
        let adapter = WebChatAdapter::new(config);
        let (tx, mut rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();
        assert!(adapter.is_connected());

        // Should receive connected event
        let event = rx.recv().await.unwrap();
        assert!(matches!(event, ChannelEvent::Connected { .. }));

        adapter.stop().await.unwrap();
        assert!(!adapter.is_connected());
    }

    #[tokio::test]
    async fn test_disabled_adapter() {
        let config = WebChatConfig {
            enabled: false,
            ..create_test_config()
        };
        let adapter = WebChatAdapter::new(config);
        let (tx, _rx) = mpsc::channel(10);

        let result = adapter.start(tx).await;
        assert!(result.is_err());
    }
}
