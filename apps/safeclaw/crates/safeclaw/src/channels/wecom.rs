//! WeCom (WeChat Work) channel adapter

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{InboundMessage, OutboundMessage};
use crate::config::WeComConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// WeCom channel adapter
pub struct WeComAdapter {
    config: WeComConfig,
    base: AdapterBase,
    secret: Arc<RwLock<Option<String>>>,
    token: Arc<RwLock<Option<String>>>,
    client: reqwest::Client,
    access_token: Arc<RwLock<Option<String>>>,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
}

impl WeComAdapter {
    /// Create a new WeCom adapter
    pub fn new(config: WeComConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("wecom"),
            secret: Arc::new(RwLock::new(None)),
            token: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(None)),
            event_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Resolve credentials from env var or inline value
    fn resolve_credentials(config: &WeComConfig) -> Result<(String, String)> {
        let secret = super::resolve_credential(&config.secret)?;
        let token = super::resolve_credential(&config.token)?;
        Ok((secret, token))
    }

    /// Obtain access token
    async fn get_access_token(&self) -> Result<String> {
        if let Some(token) = self.access_token.read().await.as_ref() {
            return Ok(token.clone());
        }

        let secret = self
            .secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("WeCom secret not initialized".to_string()))?
            .clone();

        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/gettoken?corpid={}&corpsecret={}",
            self.config.corp_id, secret
        );

        let response = self
            .client
            .get(&url)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to get WeCom access token: {}", e)))?;

        let result: WeComTokenResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse WeCom token response: {}", e)))?;

        if result.errcode != 0 {
            return Err(Error::Channel(format!(
                "WeCom token API error: {}",
                result.errmsg
            )));
        }

        let token = result.access_token;
        *self.access_token.write().await = Some(token.clone());
        Ok(token)
    }

    /// Check if a user is allowed by userId
    pub fn is_user_allowed(&self, user_id: &str) -> bool {
        self.config.allowed_users.is_empty()
            || self.config.allowed_users.iter().any(|u| u == user_id)
    }

    /// Verify callback URL (sort token, timestamp, nonce; SHA-256 hash; compare with signature)
    pub fn verify_callback(
        token: &str,
        timestamp: &str,
        nonce: &str,
        signature: &str,
    ) -> Result<()> {
        let mut parts = [token, timestamp, nonce];
        parts.sort();
        let combined = parts.join("");

        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let result = format!("{:x}", hasher.finalize());

        if result != signature {
            return Err(Error::Channel("Invalid WeCom signature".to_string()));
        }

        Ok(())
    }

    /// Parse WeCom event into InboundMessage
    pub fn parse_event(event: &WeComEvent) -> Result<InboundMessage> {
        let mut msg = InboundMessage::new(
            "wecom",
            &event.from_user_name,
            &event.from_user_name,
            &event.content,
        );
        msg.channel_message_id = event.msg_id.to_string();
        msg.timestamp = (event.create_time as i64) * 1000;
        msg.is_dm = event.msg_type == "text";
        Ok(msg)
    }
}

#[async_trait]
impl ChannelAdapter for WeComAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        self.base.set_status(AdapterStatus::Starting);

        // Resolve credentials lazily
        let (secret, token) = Self::resolve_credentials(&self.config)?;
        *self.secret.write().await = Some(secret);
        *self.token.write().await = Some(token);

        *self.event_tx.write().await = Some(event_tx.clone());

        tracing::info!(
            "WeCom adapter starting (corp_id={}, agent_id={})",
            self.config.corp_id,
            self.config.agent_id
        );

        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "wecom".to_string(),
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
                    channel: "wecom".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);
        tracing::info!("WeCom adapter stopped");
        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("WeCom adapter not running".to_string()));
        }

        tracing::debug!(
            "Sending message to WeCom user {}: {}",
            message.chat_id,
            message.content
        );

        let token = self.get_access_token().await?;
        let url = format!(
            "https://qyapi.weixin.qq.com/cgi-bin/message/send?access_token={}",
            token
        );
        let payload = serde_json::json!({
            "touser": message.chat_id,
            "msgtype": "text",
            "agentid": self.config.agent_id,
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
            .map_err(|e| Error::Channel(format!("Failed to send WeCom message: {}", e)))?;

        let result: WeComApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse WeCom response: {}", e)))?;

        if result.errcode != 0 {
            return Err(Error::Channel(format!(
                "WeCom API error: {}",
                result.errmsg
            )));
        }

        Ok(result
            .msgid
            .unwrap_or_else(|| uuid::Uuid::new_v4().to_string()))
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("WeCom adapter not running".to_string()));
        }

        tracing::debug!("Typing indicator not supported for WeCom chat {}", chat_id);
        Ok(())
    }

    async fn edit_message(&self, _chat_id: &str, _message_id: &str, _content: &str) -> Result<()> {
        Err(Error::Channel(
            "Message editing not supported for WeCom".to_string(),
        ))
    }

    async fn delete_message(&self, _chat_id: &str, _message_id: &str) -> Result<()> {
        Err(Error::Channel(
            "Message deletion not supported for WeCom".to_string(),
        ))
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

/// WeCom token response
#[derive(Debug, Deserialize)]
struct WeComTokenResponse {
    errcode: i32,
    errmsg: String,
    access_token: String,
}

/// WeCom API response
#[derive(Debug, Deserialize)]
struct WeComApiResponse {
    errcode: i32,
    errmsg: String,
    msgid: Option<String>,
}

/// WeCom event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WeComEvent {
    #[serde(rename = "ToUserName")]
    pub to_user_name: String,
    #[serde(rename = "FromUserName")]
    pub from_user_name: String,
    #[serde(rename = "CreateTime")]
    pub create_time: u64,
    #[serde(rename = "MsgType")]
    pub msg_type: String,
    #[serde(rename = "Content")]
    pub content: String,
    #[serde(rename = "MsgId")]
    pub msg_id: u64,
    #[serde(rename = "AgentID")]
    pub agent_id: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> WeComConfig {
        WeComConfig {
            corp_id: "ww_test_corp".to_string(),
            agent_id: 1000001,
            secret: "TEST_WECOM_SECRET".to_string(),
            encoding_aes_key: "TEST_WECOM_AES_KEY".to_string(),
            token: "TEST_WECOM_TOKEN".to_string(),
            allowed_users: vec!["user001".to_string(), "user002".to_string()],
            dm_policy: "pairing".to_string(),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = WeComAdapter::new(config);

        assert_eq!(adapter.name(), "wecom");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_resolve_credentials_missing() {
        let config = WeComConfig {
            secret: "".to_string(),
            token: "".to_string(),
            ..create_test_config()
        };
        let result = WeComAdapter::resolve_credentials(&config);
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Failed to resolve"));
    }

    #[test]
    fn test_user_allowed() {
        let config = create_test_config();
        let adapter = WeComAdapter::new(config);

        assert!(adapter.is_user_allowed("user001"));
        assert!(adapter.is_user_allowed("user002"));
        assert!(!adapter.is_user_allowed("user999"));
    }

    #[test]
    fn test_empty_allowed_users() {
        let config = WeComConfig {
            allowed_users: vec![],
            ..create_test_config()
        };
        let adapter = WeComAdapter::new(config);

        assert!(adapter.is_user_allowed("anyone"));
    }

    #[test]
    fn test_verify_callback_valid() {
        let token = "mytoken";
        let timestamp = "1234567890";
        let nonce = "abc123";

        let mut parts = [token, timestamp, nonce];
        parts.sort();
        let combined = parts.join("");
        let mut hasher = Sha256::new();
        hasher.update(combined.as_bytes());
        let expected = format!("{:x}", hasher.finalize());

        let result = WeComAdapter::verify_callback(token, timestamp, nonce, &expected);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_callback_invalid() {
        let token = "mytoken";
        let timestamp = "1234567890";
        let nonce = "abc123";

        let result = WeComAdapter::verify_callback(token, timestamp, nonce, "wrong");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid"));
    }

    #[test]
    fn test_parse_event() {
        let event = WeComEvent {
            to_user_name: "corp123".to_string(),
            from_user_name: "user001".to_string(),
            create_time: 1234567890,
            msg_type: "text".to_string(),
            content: "Hello WeCom!".to_string(),
            msg_id: 123456789,
            agent_id: 1000001,
        };

        let msg = WeComAdapter::parse_event(&event).unwrap();
        assert_eq!(msg.channel, "wecom");
        assert_eq!(msg.sender_id, "user001");
        assert_eq!(msg.chat_id, "user001");
        assert_eq!(msg.content, "Hello WeCom!");
        assert_eq!(msg.channel_message_id, "123456789");
        assert!(msg.is_dm);
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        std::env::set_var("TEST_WECOM_SECRET", "test_secret");
        std::env::set_var("TEST_WECOM_TOKEN", "test_token");
        let config = create_test_config();
        let adapter = WeComAdapter::new(config);
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
        std::env::set_var("TEST_WECOM_SECRET", "test_secret");
        std::env::set_var("TEST_WECOM_TOKEN", "test_token");
        let config = create_test_config();
        let adapter = WeComAdapter::new(config);
        let (tx, _rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();

        let result: Result<()> = adapter
            .edit_message("user123", "msg123", "new content")
            .await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not supported"));
    }

    #[tokio::test]
    async fn test_delete_message_not_supported() {
        std::env::set_var("TEST_WECOM_SECRET", "test_secret");
        std::env::set_var("TEST_WECOM_TOKEN", "test_token");
        let config = create_test_config();
        let adapter = WeComAdapter::new(config);
        let (tx, _rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();

        let result: Result<()> = adapter.delete_message("user123", "msg123").await;
        assert!(result.is_err());
        assert!(result.err().unwrap().to_string().contains("not supported"));
    }
}
