//! DingTalk channel adapter

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{InboundMessage, OutboundMessage};
use crate::config::DingTalkConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use ring::hmac;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// DingTalk channel adapter
pub struct DingTalkAdapter {
    config: DingTalkConfig,
    base: AdapterBase,
    app_key: Arc<RwLock<Option<String>>>,
    app_secret: Arc<RwLock<Option<String>>>,
    client: reqwest::Client,
    access_token: Arc<RwLock<Option<String>>>,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
}

impl DingTalkAdapter {
    /// Create a new DingTalk adapter
    pub fn new(config: DingTalkConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("dingtalk"),
            app_key: Arc::new(RwLock::new(None)),
            app_secret: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(None)),
            event_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Resolve credentials from env var or inline value
    fn resolve_credentials(config: &DingTalkConfig) -> Result<(String, String)> {
        let app_key = super::resolve_credential(&config.app_key)?;
        let app_secret = super::resolve_credential(&config.app_secret)?;
        Ok((app_key, app_secret))
    }

    /// Obtain access token
    async fn get_access_token(&self) -> Result<String> {
        if let Some(token) = self.access_token.read().await.as_ref() {
            return Ok(token.clone());
        }

        let app_key = self
            .app_key
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("DingTalk app_key not initialized".to_string()))?
            .clone();
        let app_secret = self
            .app_secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("DingTalk app_secret not initialized".to_string()))?
            .clone();

        let url = format!(
            "https://oapi.dingtalk.com/gettoken?appkey={}&appsecret={}",
            app_key, app_secret
        );

        let response =
            self.client.get(&url).send().await.map_err(|e| {
                Error::Channel(format!("Failed to get DingTalk access token: {}", e))
            })?;

        let result: DingTalkTokenResponse = response.json().await.map_err(|e| {
            Error::Channel(format!("Failed to parse DingTalk token response: {}", e))
        })?;

        if result.errcode != 0 {
            return Err(Error::Channel(format!(
                "DingTalk token API error: {}",
                result.errmsg
            )));
        }

        let token = result.access_token;
        *self.access_token.write().await = Some(token.clone());
        Ok(token)
    }

    /// Check if a user is allowed by staffId
    pub fn is_user_allowed(&self, staff_id: &str) -> bool {
        self.config.allowed_users.is_empty()
            || self.config.allowed_users.iter().any(|u| u == staff_id)
    }

    /// Verify callback signature (HMAC-SHA256 of timestamp + secret, base64 encoded)
    pub fn verify_signature(timestamp: &str, secret: &str, expected: &str) -> Result<()> {
        let string_to_sign = format!("{}\n{}", timestamp, secret);
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let signature = hmac::sign(&key, string_to_sign.as_bytes());
        use base64::Engine as _;
        let encoded = base64::engine::general_purpose::STANDARD.encode(signature.as_ref());

        if encoded != expected {
            return Err(Error::Channel("Invalid DingTalk signature".to_string()));
        }

        Ok(())
    }

    /// Parse DingTalk event into InboundMessage
    pub fn parse_event(event: &DingTalkEvent) -> Result<InboundMessage> {
        let mut msg = InboundMessage::new(
            "dingtalk",
            &event.sender_staff_id,
            &event.conversation_id,
            &event.text.content,
        );
        msg.channel_message_id = event.msg_id.clone();
        msg.timestamp = event.create_at;
        msg.is_dm = event.conversation_type == "1";
        Ok(msg)
    }
}

#[async_trait]
impl ChannelAdapter for DingTalkAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        self.base.set_status(AdapterStatus::Starting);

        // Resolve credentials lazily
        let (app_key, app_secret) = Self::resolve_credentials(&self.config)?;
        *self.app_key.write().await = Some(app_key);
        *self.app_secret.write().await = Some(app_secret);

        *self.event_tx.write().await = Some(event_tx.clone());

        tracing::info!(
            "DingTalk adapter starting (robot_code={})",
            self.config.robot_code
        );

        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "dingtalk".to_string(),
            })
            .await;

        self.base.set_status(AdapterStatus::Running);
        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.base.set_status(AdapterStatus::Stopping);

        if let Some(tx) = self.event_tx.read().await.as_ref() {
            let _ = tx
                .send(ChannelEvent::Disconnected {
                    channel: "dingtalk".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);
        tracing::info!("DingTalk adapter stopped");
        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("DingTalk adapter not running".to_string()));
        }

        tracing::debug!(
            "Sending message to DingTalk chat {}: {}",
            message.chat_id,
            message.content
        );

        let token = self.get_access_token().await?;
        let url = format!(
            "https://oapi.dingtalk.com/robot/send?access_token={}",
            token
        );
        let payload = serde_json::json!({
            "msgtype": "text",
            "text": {
                "content": message.content,
            },
        });

        let response = self
            .client
            .post(&url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to send DingTalk message: {}", e)))?;

        let result: DingTalkApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse DingTalk response: {}", e)))?;

        if result.errcode != 0 {
            return Err(Error::Channel(format!(
                "DingTalk API error: {}",
                result.errmsg
            )));
        }

        Ok(uuid::Uuid::new_v4().to_string())
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("DingTalk adapter not running".to_string()));
        }

        tracing::debug!(
            "Typing indicator not supported for DingTalk chat {}",
            chat_id
        );
        Ok(())
    }

    async fn edit_message(&self, _chat_id: &str, _message_id: &str, _content: &str) -> Result<()> {
        Err(Error::Channel(
            "Message editing not supported for DingTalk".to_string(),
        ))
    }

    async fn delete_message(&self, _chat_id: &str, _message_id: &str) -> Result<()> {
        Err(Error::Channel(
            "Message deletion not supported for DingTalk".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

/// DingTalk token response
#[derive(Debug, Deserialize)]
struct DingTalkTokenResponse {
    errcode: i32,
    errmsg: String,
    access_token: String,
}

/// DingTalk API response
#[derive(Debug, Deserialize)]
struct DingTalkApiResponse {
    errcode: i32,
    errmsg: String,
}

/// DingTalk event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkEvent {
    pub msg_id: String,
    pub conversation_id: String,
    pub conversation_type: String,
    pub sender_staff_id: String,
    pub text: DingTalkText,
    pub create_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DingTalkText {
    pub content: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> DingTalkConfig {
        DingTalkConfig {
            app_key: "TEST_DINGTALK_KEY".to_string(),
            app_secret: "TEST_DINGTALK_SECRET".to_string(),
            robot_code: "robot_test".to_string(),
            allowed_users: vec!["staff001".to_string(), "staff002".to_string()],
            dm_policy: "pairing".to_string(),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = DingTalkAdapter::new(config);

        assert_eq!(adapter.name(), "dingtalk");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_resolve_credentials_missing() {
        let config = DingTalkConfig {
            app_key: "".to_string(),
            app_secret: "".to_string(),
            robot_code: "robot_test".to_string(),
            allowed_users: vec![],
            dm_policy: "open".to_string(),
        };
        let result = DingTalkAdapter::resolve_credentials(&config);
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Failed to resolve"));
    }

    #[test]
    fn test_user_allowed() {
        let config = create_test_config();
        let adapter = DingTalkAdapter::new(config);

        assert!(adapter.is_user_allowed("staff001"));
        assert!(adapter.is_user_allowed("staff002"));
        assert!(!adapter.is_user_allowed("staff999"));
    }

    #[test]
    fn test_empty_allowed_users() {
        let config = DingTalkConfig {
            allowed_users: vec![],
            ..create_test_config()
        };
        let adapter = DingTalkAdapter::new(config);

        assert!(adapter.is_user_allowed("anyone"));
    }

    #[test]
    fn test_verify_signature_valid() {
        let timestamp = "1234567890";
        let secret = "mysecret";

        let string_to_sign = format!("{}\n{}", timestamp, secret);
        let key = hmac::Key::new(hmac::HMAC_SHA256, secret.as_bytes());
        let signature = hmac::sign(&key, string_to_sign.as_bytes());
        use base64::Engine as _;
        let expected = base64::engine::general_purpose::STANDARD.encode(signature.as_ref());

        let result = DingTalkAdapter::verify_signature(timestamp, secret, &expected);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_signature_invalid() {
        let timestamp = "1234567890";
        let secret = "mysecret";

        let result = DingTalkAdapter::verify_signature(timestamp, secret, "wrong");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid"));
    }

    #[test]
    fn test_parse_event() {
        let event = DingTalkEvent {
            msg_id: "msg123456".to_string(),
            conversation_id: "cid123".to_string(),
            conversation_type: "2".to_string(),
            sender_staff_id: "staff001".to_string(),
            text: DingTalkText {
                content: "Hello DingTalk!".to_string(),
            },
            create_at: 1234567890000,
        };

        let msg = DingTalkAdapter::parse_event(&event).unwrap();
        assert_eq!(msg.channel, "dingtalk");
        assert_eq!(msg.sender_id, "staff001");
        assert_eq!(msg.chat_id, "cid123");
        assert_eq!(msg.content, "Hello DingTalk!");
        assert_eq!(msg.channel_message_id, "msg123456");
        assert!(!msg.is_dm);
    }

    #[test]
    fn test_parse_event_dm() {
        let event = DingTalkEvent {
            msg_id: "msg123456".to_string(),
            conversation_id: "cid123".to_string(),
            conversation_type: "1".to_string(),
            sender_staff_id: "staff001".to_string(),
            text: DingTalkText {
                content: "Private message".to_string(),
            },
            create_at: 1234567890000,
        };

        let msg = DingTalkAdapter::parse_event(&event).unwrap();
        assert!(msg.is_dm);
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        std::env::set_var("TEST_DINGTALK_KEY", "test_key");
        std::env::set_var("TEST_DINGTALK_SECRET", "test_secret");
        let config = create_test_config();
        let adapter = DingTalkAdapter::new(config);
        let (tx, mut rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();
        assert!(adapter.is_connected());

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, ChannelEvent::Connected { .. }));

        adapter.stop().await.unwrap();
        assert!(!adapter.is_connected());
    }

    #[tokio::test]
    async fn test_edit_message_not_supported() {
        std::env::set_var("TEST_DINGTALK_KEY", "test_key");
        std::env::set_var("TEST_DINGTALK_SECRET", "test_secret");
        let config = create_test_config();
        let adapter = DingTalkAdapter::new(config);
        let (tx, _rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();

        let result = adapter
            .edit_message("chat123", "msg123", "new content")
            .await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("not supported"));
    }

    #[tokio::test]
    async fn test_delete_message_not_supported() {
        std::env::set_var("TEST_DINGTALK_KEY", "test_key");
        std::env::set_var("TEST_DINGTALK_SECRET", "test_secret");
        let config = create_test_config();
        let adapter = DingTalkAdapter::new(config);
        let (tx, _rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();

        let result = adapter.delete_message("chat123", "msg123").await;
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("not supported"));
    }
}
