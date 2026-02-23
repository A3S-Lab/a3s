//! Feishu (Lark) channel adapter

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{InboundMessage, OutboundMessage};
use crate::config::FeishuConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Feishu channel adapter
pub struct FeishuAdapter {
    config: FeishuConfig,
    base: AdapterBase,
    app_secret: Arc<RwLock<Option<String>>>,
    encrypt_key: Arc<RwLock<Option<String>>>,
    client: reqwest::Client,
    access_token: Arc<RwLock<Option<CachedToken>>>,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
    /// Track recently processed message IDs to deduplicate webhook retries
    seen_messages: Arc<RwLock<std::collections::VecDeque<String>>>,
}

/// Cached access token with expiry
struct CachedToken {
    token: String,
    /// When this token expires (with 5-minute safety margin)
    expires_at: std::time::Instant,
}

const SEEN_MESSAGES_CAPACITY: usize = 256;

impl FeishuAdapter {
    /// Create a new Feishu adapter
    pub fn new(config: FeishuConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("feishu"),
            app_secret: Arc::new(RwLock::new(None)),
            encrypt_key: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            access_token: Arc::new(RwLock::new(None)),
            event_tx: Arc::new(RwLock::new(None)),
            seen_messages: Arc::new(RwLock::new(std::collections::VecDeque::with_capacity(
                SEEN_MESSAGES_CAPACITY,
            ))),
        }
    }

    /// Resolve credential: try environment variable first, fall back to inline value.
    fn resolve_credential(credential_ref: &str) -> Result<String> {
        super::resolve_credential(credential_ref)
    }

    /// Obtain tenant access token (with TTL-based caching)
    async fn get_access_token(&self) -> Result<String> {
        // Return cached token if still valid
        if let Some(cached) = self.access_token.read().await.as_ref() {
            if cached.expires_at > std::time::Instant::now() {
                return Ok(cached.token.clone());
            }
            tracing::debug!("Feishu access token expired, refreshing");
        }

        let app_secret = self
            .app_secret
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Feishu app_secret not initialized".to_string()))?
            .clone();

        let url = "https://open.feishu.cn/open-apis/auth/v3/tenant_access_token/internal";
        let payload = serde_json::json!({
            "app_id": self.config.app_id,
            "app_secret": app_secret,
        });

        let response = self
            .client
            .post(url)
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to get Feishu access token: {}", e)))?;

        let result: FeishuTokenResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Feishu token response: {}", e)))?;

        if result.code != 0 {
            return Err(Error::Channel(format!(
                "Feishu token API error: {}",
                result.msg
            )));
        }

        let token = result.tenant_access_token;
        // Feishu tokens expire in `expire` seconds (typically 7200 = 2h).
        // Subtract 5 minutes as safety margin.
        let ttl_secs = result.expire.unwrap_or(7200).saturating_sub(300);
        *self.access_token.write().await = Some(CachedToken {
            token: token.clone(),
            expires_at: std::time::Instant::now() + std::time::Duration::from_secs(ttl_secs as u64),
        });
        Ok(token)
    }

    /// Check if a message ID has been seen recently (deduplication)
    pub async fn is_duplicate(&self, message_id: &str) -> bool {
        let mut seen = self.seen_messages.write().await;
        if seen.iter().any(|id| id == message_id) {
            return true;
        }
        if seen.len() >= SEEN_MESSAGES_CAPACITY {
            seen.pop_front();
        }
        seen.push_back(message_id.to_string());
        false
    }

    /// Check if a user is allowed by open_id
    pub fn is_user_allowed(&self, open_id: &str) -> bool {
        self.config.allowed_users.is_empty()
            || self.config.allowed_users.iter().any(|u| u == open_id)
    }

    /// Verify callback signature (SHA256 of timestamp + nonce + encrypt_key + body)
    pub fn verify_signature(
        timestamp: &str,
        nonce: &str,
        encrypt_key: &str,
        body: &str,
        expected: &str,
    ) -> Result<()> {
        let content = format!("{}{}{}{}", timestamp, nonce, encrypt_key, body);
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let result = format!("{:x}", hasher.finalize());

        if result != expected {
            return Err(Error::Channel("Invalid Feishu signature".to_string()));
        }

        Ok(())
    }

    /// Parse Feishu event into InboundMessage
    pub fn parse_event(event: &FeishuEvent) -> Result<InboundMessage> {
        let mut msg = InboundMessage::new(
            "feishu",
            &event.sender.sender_id.open_id,
            &event.message.chat_id,
            &event.message.content,
        );
        msg.channel_message_id = event.message.message_id.clone();
        // Feishu create_time is already in milliseconds
        msg.timestamp = event.message.create_time as i64;
        msg.is_dm = event.message.chat_type == "p2p";
        msg.is_mention = event
            .message
            .mentions
            .iter()
            .any(|m| m.id.open_id != event.sender.sender_id.open_id);
        Ok(msg)
    }
}

#[async_trait]
impl ChannelAdapter for FeishuAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        self.base.set_status(AdapterStatus::Starting);

        // Resolve credentials lazily
        let app_secret = Self::resolve_credential(&self.config.app_secret)?;
        *self.app_secret.write().await = Some(app_secret);

        // Encrypt key is optional — some Feishu bots don't use encrypted callbacks
        if !self.config.encrypt_key.is_empty() {
            if let Ok(key) = Self::resolve_credential(&self.config.encrypt_key) {
                *self.encrypt_key.write().await = Some(key);
            }
        }

        *self.event_tx.write().await = Some(event_tx.clone());

        tracing::info!("Feishu adapter starting (app_id={})", self.config.app_id);

        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "feishu".to_string(),
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
                    channel: "feishu".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);
        tracing::info!("Feishu adapter stopped");
        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("Feishu adapter not running".to_string()));
        }

        tracing::debug!(
            "Sending message to Feishu chat {}: {}",
            message.chat_id,
            message.content
        );

        let token = self.get_access_token().await?;

        // Determine receive_id_type from the ID prefix:
        // oc_ = chat_id, ou_ = open_id, on_ = union_id
        let receive_id_type = if message.chat_id.starts_with("oc_") {
            "chat_id"
        } else if message.chat_id.starts_with("ou_") {
            "open_id"
        } else if message.chat_id.starts_with("on_") {
            "union_id"
        } else {
            "chat_id"
        };

        let url = format!(
            "https://open.feishu.cn/open-apis/im/v1/messages?receive_id_type={}",
            receive_id_type
        );

        // Use custom card if provided, otherwise wrap content in a default markdown card
        let card = if let Some(custom_card) = &message.card {
            custom_card.clone()
        } else {
            serde_json::json!({
                "elements": [{
                    "tag": "markdown",
                    "content": message.content,
                }]
            })
        };
        let payload = serde_json::json!({
            "receive_id": message.chat_id,
            "msg_type": "interactive",
            "content": card.to_string(),
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to send Feishu message: {}", e)))?;

        let result: FeishuApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Feishu response: {}", e)))?;

        if result.code != 0 {
            return Err(Error::Channel(format!("Feishu API error: {}", result.msg)));
        }

        Ok(result.data.message_id.unwrap_or_default())
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Feishu adapter not running".to_string()));
        }

        tracing::debug!("Feishu typing indicator not supported for chat {}", chat_id);
        Ok(())
    }

    async fn edit_message(&self, chat_id: &str, message_id: &str, content: &str) -> Result<()> {
        let card = serde_json::json!({
            "elements": [{
                "tag": "markdown",
                "content": content,
            }]
        });
        self.edit_message_card(chat_id, message_id, &card).await
    }

    async fn edit_message_card(
        &self,
        _chat_id: &str,
        message_id: &str,
        card: &serde_json::Value,
    ) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Feishu adapter not running".to_string()));
        }

        tracing::debug!("Editing Feishu message {} with card", message_id,);

        let token = self.get_access_token().await?;
        let url = format!(
            "https://open.feishu.cn/open-apis/im/v1/messages/{}",
            message_id
        );

        let payload = serde_json::json!({
            "msg_type": "interactive",
            "content": card.to_string(),
        });

        let response = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bearer {}", token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to edit Feishu message: {}", e)))?;

        let result: FeishuApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Feishu response: {}", e)))?;

        if result.code != 0 {
            return Err(Error::Channel(format!("Feishu API error: {}", result.msg)));
        }

        Ok(())
    }

    async fn delete_message(&self, _chat_id: &str, message_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Feishu adapter not running".to_string()));
        }

        tracing::debug!("Deleting Feishu message {}", message_id);

        let token = self.get_access_token().await?;
        let url = format!(
            "https://open.feishu.cn/open-apis/im/v1/messages/{}",
            message_id
        );

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bearer {}", token))
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to delete Feishu message: {}", e)))?;

        let result: FeishuApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Feishu response: {}", e)))?;

        if result.code != 0 {
            return Err(Error::Channel(format!("Feishu API error: {}", result.msg)));
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

/// Feishu token response
#[derive(Debug, Deserialize)]
struct FeishuTokenResponse {
    code: i32,
    msg: String,
    tenant_access_token: String,
    /// Token TTL in seconds (typically 7200 = 2 hours)
    expire: Option<u32>,
}

/// Feishu API response
#[derive(Debug, Deserialize)]
struct FeishuApiResponse {
    code: i32,
    msg: String,
    #[serde(default)]
    data: FeishuApiData,
}

#[derive(Debug, Default, Deserialize)]
struct FeishuApiData {
    message_id: Option<String>,
}

/// Feishu event structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuEvent {
    pub sender: FeishuSender,
    pub message: FeishuMessage,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuSender {
    pub sender_id: FeishuUserId,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuUserId {
    pub open_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMessage {
    pub message_id: String,
    pub chat_id: String,
    pub chat_type: String,
    pub content: String,
    pub create_time: u64,
    pub mentions: Vec<FeishuMention>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeishuMention {
    pub id: FeishuUserId,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> FeishuConfig {
        FeishuConfig {
            app_id: "cli_test_app".to_string(),
            app_secret: "TEST_FEISHU_SECRET".to_string(),
            encrypt_key: "TEST_FEISHU_ENCRYPT_KEY".to_string(),
            verification_token: "TEST_FEISHU_TOKEN".to_string(),
            allowed_users: vec!["ou_user1".to_string(), "ou_user2".to_string()],
            dm_policy: "pairing".to_string(),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = FeishuAdapter::new(config);

        assert_eq!(adapter.name(), "feishu");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_resolve_credential_missing() {
        // Empty string triggers error; non-empty strings are treated as inline values
        let result = FeishuAdapter::resolve_credential("");
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Failed to resolve"));
    }

    #[test]
    fn test_user_allowed() {
        let config = create_test_config();
        let adapter = FeishuAdapter::new(config);

        assert!(adapter.is_user_allowed("ou_user1"));
        assert!(adapter.is_user_allowed("ou_user2"));
        assert!(!adapter.is_user_allowed("ou_unknown"));
    }

    #[test]
    fn test_empty_allowed_users() {
        let config = FeishuConfig {
            allowed_users: vec![],
            ..create_test_config()
        };
        let adapter = FeishuAdapter::new(config);

        assert!(adapter.is_user_allowed("ou_anyone"));
    }

    #[test]
    fn test_verify_signature_valid() {
        let timestamp = "1234567890";
        let nonce = "abc123";
        let encrypt_key = "mykey";
        let body = r#"{"event":"test"}"#;

        let content = format!("{}{}{}{}", timestamp, nonce, encrypt_key, body);
        let mut hasher = Sha256::new();
        hasher.update(content.as_bytes());
        let expected = format!("{:x}", hasher.finalize());

        let result =
            FeishuAdapter::verify_signature(timestamp, nonce, encrypt_key, body, &expected);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_signature_invalid() {
        let timestamp = "1234567890";
        let nonce = "abc123";
        let encrypt_key = "mykey";
        let body = r#"{"event":"test"}"#;

        let result =
            FeishuAdapter::verify_signature(timestamp, nonce, encrypt_key, body, "wrong_hash");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Invalid"));
    }

    #[test]
    fn test_parse_event() {
        let event = FeishuEvent {
            sender: FeishuSender {
                sender_id: FeishuUserId {
                    open_id: "ou_user123".to_string(),
                },
            },
            message: FeishuMessage {
                message_id: "om_123456".to_string(),
                chat_id: "oc_chat123".to_string(),
                chat_type: "group".to_string(),
                content: "Hello Feishu!".to_string(),
                create_time: 1234567890,
                mentions: vec![],
            },
        };

        let msg = FeishuAdapter::parse_event(&event).unwrap();
        assert_eq!(msg.channel, "feishu");
        assert_eq!(msg.sender_id, "ou_user123");
        assert_eq!(msg.chat_id, "oc_chat123");
        assert_eq!(msg.content, "Hello Feishu!");
        assert_eq!(msg.channel_message_id, "om_123456");
        assert!(!msg.is_dm);
        assert!(!msg.is_mention);
    }

    #[test]
    fn test_parse_event_dm() {
        let event = FeishuEvent {
            sender: FeishuSender {
                sender_id: FeishuUserId {
                    open_id: "ou_user123".to_string(),
                },
            },
            message: FeishuMessage {
                message_id: "om_123456".to_string(),
                chat_id: "oc_chat123".to_string(),
                chat_type: "p2p".to_string(),
                content: "Private message".to_string(),
                create_time: 1234567890,
                mentions: vec![],
            },
        };

        let msg = FeishuAdapter::parse_event(&event).unwrap();
        assert!(msg.is_dm);
    }

    #[test]
    fn test_parse_event_mention() {
        let event = FeishuEvent {
            sender: FeishuSender {
                sender_id: FeishuUserId {
                    open_id: "ou_user123".to_string(),
                },
            },
            message: FeishuMessage {
                message_id: "om_123456".to_string(),
                chat_id: "oc_chat123".to_string(),
                chat_type: "group".to_string(),
                content: "@bot help".to_string(),
                create_time: 1234567890,
                mentions: vec![FeishuMention {
                    id: FeishuUserId {
                        open_id: "ou_bot456".to_string(),
                    },
                }],
            },
        };

        let msg = FeishuAdapter::parse_event(&event).unwrap();
        assert!(msg.is_mention);
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        std::env::set_var("TEST_FEISHU_SECRET", "test_secret");
        std::env::set_var("TEST_FEISHU_ENCRYPT_KEY", "test_encrypt_key");
        let config = create_test_config();
        let adapter = FeishuAdapter::new(config);
        let (tx, mut rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();
        assert!(adapter.is_connected());

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, ChannelEvent::Connected { .. }));

        adapter.stop().await.unwrap();
        assert!(!adapter.is_connected());
    }
}
