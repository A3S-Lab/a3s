//! Telegram channel adapter
//!
//! Implements the Telegram Bot API for sending and receiving messages.
//! Supports long polling for incoming updates and the Bot API for outbound messages.
//!
//! API Reference: https://core.telegram.org/bots/api

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{
    AttachmentType, InboundMessage, MessageAttachment, MessageFormat, OutboundMessage,
};
use crate::config::TelegramConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

const TELEGRAM_API_BASE: &str = "https://api.telegram.org/bot";

/// Telegram channel adapter
pub struct TelegramAdapter {
    config: TelegramConfig,
    base: AdapterBase,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
    /// HTTP client for Telegram Bot API
    client: reqwest::Client,
    /// Resolved bot token (from env var)
    bot_token: Arc<RwLock<Option<String>>>,
    /// Shutdown signal
    shutdown_tx: Arc<RwLock<Option<tokio::sync::watch::Sender<bool>>>>,
}

impl TelegramAdapter {
    /// Create a new Telegram adapter
    pub fn new(config: TelegramConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("telegram"),
            event_tx: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            bot_token: Arc::new(RwLock::new(None)),
            shutdown_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Check if a user is allowed
    pub fn is_user_allowed(&self, user_id: i64) -> bool {
        self.config.allowed_users.is_empty() || self.config.allowed_users.contains(&user_id)
    }

    /// Resolve the bot token from env var or inline value
    fn resolve_token(&self) -> Result<String> {
        super::resolve_credential(&self.config.bot_token)
    }

    /// Build API URL for a method
    fn api_url(token: &str, method: &str) -> String {
        format!("{}{}/{}", TELEGRAM_API_BASE, token, method)
    }

    /// Call Telegram Bot API and return the result
    async fn api_call(&self, method: &str, body: &serde_json::Value) -> Result<serde_json::Value> {
        let token = self.bot_token.read().await;
        let token = token
            .as_ref()
            .ok_or_else(|| Error::Channel("Telegram bot token not resolved".to_string()))?;

        let url = Self::api_url(token, method);
        let resp = self
            .client
            .post(&url)
            .json(body)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Telegram API request failed: {}", e)))?;

        let status = resp.status();
        let body: serde_json::Value = resp
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Telegram response: {}", e)))?;

        if !status.is_success() || body.get("ok") != Some(&serde_json::Value::Bool(true)) {
            let description = body["description"].as_str().unwrap_or("unknown error");
            return Err(Error::Channel(format!(
                "Telegram API error ({}): {}",
                status, description
            )));
        }

        Ok(body["result"].clone())
    }

    /// Parse a Telegram Update into an InboundMessage
    fn parse_update(&self, update: &serde_json::Value) -> Option<InboundMessage> {
        let message = update
            .get("message")
            .or_else(|| update.get("edited_message"))?;

        let from = message.get("from")?;
        let user_id = from["id"].as_i64()?;

        // Check user allowlist
        if !self.is_user_allowed(user_id) {
            tracing::debug!(user_id, "Telegram user not in allowlist, ignoring");
            return None;
        }

        let chat = message.get("chat")?;
        let chat_id = chat["id"].as_i64()?.to_string();
        let chat_type = chat["type"].as_str().unwrap_or("private");
        let is_dm = chat_type == "private";

        let sender_id = user_id.to_string();
        let sender_name = [
            from.get("first_name").and_then(|v| v.as_str()),
            from.get("last_name").and_then(|v| v.as_str()),
        ]
        .iter()
        .filter_map(|s| *s)
        .collect::<Vec<_>>()
        .join(" ");

        let text = message
            .get("text")
            .or_else(|| message.get("caption"))
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        let message_id = message["message_id"].as_i64().unwrap_or(0).to_string();

        let mut msg = InboundMessage::new("telegram", &sender_id, &chat_id, &text);
        msg.channel_message_id = message_id;
        if !sender_name.is_empty() {
            msg.sender_name = Some(sender_name);
        }
        msg.is_dm = is_dm;
        msg.raw = Some(update.clone());

        // Check for bot mention in group chats
        if !is_dm {
            if let Some(entities) = message.get("entities").and_then(|v| v.as_array()) {
                msg.is_mention = entities.iter().any(|e| {
                    e["type"].as_str() == Some("mention")
                        || e["type"].as_str() == Some("text_mention")
                });
            }
        }

        // Check for reply
        if let Some(reply) = message.get("reply_to_message") {
            msg.reply_to = reply["message_id"].as_i64().map(|id| id.to_string());
        }

        // Parse attachments
        if let Some(photo) = message.get("photo").and_then(|v| v.as_array()) {
            if let Some(largest) = photo.last() {
                let mut att = MessageAttachment::new(AttachmentType::Image);
                att.width = largest["width"].as_u64().map(|v| v as u32);
                att.height = largest["height"].as_u64().map(|v| v as u32);
                att.size = largest["file_size"].as_u64();
                msg.attachments.push(att);
            }
        }

        if let Some(doc) = message.get("document") {
            let mut att = MessageAttachment::new(AttachmentType::Document);
            att.filename = doc["file_name"].as_str().map(|s| s.to_string());
            att.mime_type = doc["mime_type"].as_str().map(|s| s.to_string());
            att.size = doc["file_size"].as_u64();
            msg.attachments.push(att);
        }

        if message.get("voice").is_some() {
            msg.attachments
                .push(MessageAttachment::new(AttachmentType::Voice));
        }

        if message.get("sticker").is_some() {
            msg.attachments
                .push(MessageAttachment::new(AttachmentType::Sticker));
        }

        Some(msg)
    }

    /// Long polling loop for receiving updates
    async fn poll_loop(
        client: reqwest::Client,
        token: String,
        event_tx: mpsc::Sender<ChannelEvent>,
        mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
        adapter: Arc<TelegramAdapter>,
    ) {
        let mut offset: i64 = 0;
        let poll_timeout = 30; // seconds

        tracing::info!("Telegram long polling started");

        loop {
            // Check shutdown
            if *shutdown_rx.borrow() {
                break;
            }

            let url = Self::api_url(&token, "getUpdates");
            let body = serde_json::json!({
                "offset": offset,
                "timeout": poll_timeout,
                "allowed_updates": ["message", "edited_message"]
            });

            let result = tokio::select! {
                resp = client.post(&url).json(&body).send() => resp,
                _ = shutdown_rx.changed() => break,
            };

            match result {
                Ok(resp) => {
                    if let Ok(data) = resp.json::<serde_json::Value>().await {
                        if let Some(updates) = data["result"].as_array() {
                            for update in updates {
                                // Update offset
                                if let Some(update_id) = update["update_id"].as_i64() {
                                    offset = update_id + 1;
                                }

                                // Check for edited message
                                if update.get("edited_message").is_some() {
                                    if let Some(msg) = update.get("edited_message") {
                                        let chat_id =
                                            msg["chat"]["id"].as_i64().unwrap_or(0).to_string();
                                        let message_id =
                                            msg["message_id"].as_i64().unwrap_or(0).to_string();
                                        let new_content =
                                            msg["text"].as_str().unwrap_or("").to_string();
                                        let _ = event_tx
                                            .send(ChannelEvent::MessageEdited {
                                                channel: "telegram".to_string(),
                                                message_id,
                                                new_content,
                                            })
                                            .await;
                                        let _ = chat_id; // used for context
                                    }
                                    continue;
                                }

                                // Parse and dispatch message
                                if let Some(msg) = adapter.parse_update(update) {
                                    if event_tx.send(ChannelEvent::Message(msg)).await.is_err() {
                                        tracing::warn!("Event channel closed, stopping poll loop");
                                        return;
                                    }
                                }
                            }
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!("Telegram poll error: {}, retrying in 5s", e);
                    tokio::time::sleep(std::time::Duration::from_secs(5)).await;
                }
            }
        }

        tracing::info!("Telegram long polling stopped");
    }

    /// Convert MessageFormat to Telegram parse_mode
    fn parse_mode(format: &MessageFormat) -> Option<&'static str> {
        match format {
            MessageFormat::Markdown => Some("MarkdownV2"),
            MessageFormat::Html => Some("HTML"),
            MessageFormat::Text => None,
        }
    }
}

#[async_trait]
impl ChannelAdapter for TelegramAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        self.base.set_status(AdapterStatus::Starting);

        // Resolve bot token
        let token = self.resolve_token()?;
        *self.bot_token.write().await = Some(token.clone());

        // Store event sender
        *self.event_tx.write().await = Some(event_tx.clone());

        // Verify token with getMe
        let me = self
            .api_call("getMe", &serde_json::json!({}))
            .await
            .map_err(|e| Error::Channel(format!("Failed to verify Telegram bot token: {}", e)))?;

        let bot_name = me["username"].as_str().unwrap_or("unknown");
        tracing::info!(bot = bot_name, "Telegram bot authenticated");

        // Send connected event
        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "telegram".to_string(),
            })
            .await;

        // Start long polling in background
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        *self.shutdown_tx.write().await = Some(shutdown_tx);

        let client = self.client.clone();
        let adapter = Arc::new(TelegramAdapter {
            config: self.config.clone(),
            base: AdapterBase::new("telegram"),
            event_tx: Arc::new(RwLock::new(Some(event_tx.clone()))),
            client: self.client.clone(),
            bot_token: Arc::new(RwLock::new(Some(token.clone()))),
            shutdown_tx: Arc::new(RwLock::new(None)),
        });

        tokio::spawn(Self::poll_loop(
            client,
            token,
            event_tx,
            shutdown_rx,
            adapter,
        ));

        self.base.set_status(AdapterStatus::Running);

        Ok(())
    }

    async fn stop(&self) -> Result<()> {
        self.base.set_status(AdapterStatus::Stopping);

        // Signal shutdown to poll loop
        if let Some(tx) = self.shutdown_tx.write().await.take() {
            let _ = tx.send(true);
        }

        // Send disconnected event
        if let Some(tx) = self.event_tx.read().await.as_ref() {
            let _ = tx
                .send(ChannelEvent::Disconnected {
                    channel: "telegram".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        *self.bot_token.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);

        tracing::info!("Telegram adapter stopped");

        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("Telegram adapter not running".to_string()));
        }

        // Send typing indicator first if requested
        if message.show_typing {
            let _ = self.send_typing(&message.chat_id).await;
        }

        let mut body = serde_json::json!({
            "chat_id": message.chat_id,
            "text": message.content,
        });

        if let Some(parse_mode) = Self::parse_mode(&message.format) {
            body["parse_mode"] = serde_json::Value::String(parse_mode.to_string());
        }

        if let Some(ref reply_to) = message.reply_to {
            if let Ok(msg_id) = reply_to.parse::<i64>() {
                body["reply_to_message_id"] = serde_json::Value::Number(msg_id.into());
            }
        }

        let result = self.api_call("sendMessage", &body).await?;

        let message_id = result["message_id"].as_i64().unwrap_or(0).to_string();

        Ok(message_id)
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Telegram adapter not running".to_string()));
        }

        let body = serde_json::json!({
            "chat_id": chat_id,
            "action": "typing"
        });

        let _ = self.api_call("sendChatAction", &body).await;

        Ok(())
    }

    async fn edit_message(&self, chat_id: &str, message_id: &str, content: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Telegram adapter not running".to_string()));
        }

        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id.parse::<i64>().unwrap_or(0),
            "text": content,
        });

        self.api_call("editMessageText", &body).await?;

        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Telegram adapter not running".to_string()));
        }

        let body = serde_json::json!({
            "chat_id": chat_id,
            "message_id": message_id.parse::<i64>().unwrap_or(0),
        });

        self.api_call("deleteMessage", &body).await?;

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> TelegramConfig {
        TelegramConfig {
            bot_token: "test_token".to_string(),
            allowed_users: vec![123456789],
            dm_policy: "pairing".to_string(),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = TelegramAdapter::new(config);

        assert_eq!(adapter.name(), "telegram");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_user_allowed() {
        let config = create_test_config();
        let adapter = TelegramAdapter::new(config);

        assert!(adapter.is_user_allowed(123456789));
        assert!(!adapter.is_user_allowed(987654321));
    }

    #[test]
    fn test_empty_allowed_users() {
        let config = TelegramConfig {
            bot_token: "test_token".to_string(),
            allowed_users: vec![],
            dm_policy: "open".to_string(),
        };
        let adapter = TelegramAdapter::new(config);

        assert!(adapter.is_user_allowed(123456789));
        assert!(adapter.is_user_allowed(987654321));
    }

    #[test]
    fn test_api_url() {
        let url = TelegramAdapter::api_url("123:ABC", "sendMessage");
        assert_eq!(url, "https://api.telegram.org/bot123:ABC/sendMessage");
    }

    #[test]
    fn test_parse_mode() {
        assert_eq!(
            TelegramAdapter::parse_mode(&MessageFormat::Markdown),
            Some("MarkdownV2")
        );
        assert_eq!(
            TelegramAdapter::parse_mode(&MessageFormat::Html),
            Some("HTML")
        );
        assert_eq!(TelegramAdapter::parse_mode(&MessageFormat::Text), None);
    }

    #[test]
    fn test_parse_update_private_message() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            allowed_users: vec![],
            dm_policy: "open".to_string(),
        };
        let adapter = TelegramAdapter::new(config);

        let update = serde_json::json!({
            "update_id": 100,
            "message": {
                "message_id": 42,
                "from": {
                    "id": 12345,
                    "first_name": "John",
                    "last_name": "Doe"
                },
                "chat": {
                    "id": 12345,
                    "type": "private"
                },
                "text": "Hello bot!"
            }
        });

        let msg = adapter.parse_update(&update).unwrap();
        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.sender_id, "12345");
        assert_eq!(msg.sender_name, Some("John Doe".to_string()));
        assert_eq!(msg.chat_id, "12345");
        assert_eq!(msg.content, "Hello bot!");
        assert_eq!(msg.channel_message_id, "42");
        assert!(msg.is_dm);
        assert!(!msg.is_mention);
    }

    #[test]
    fn test_parse_update_group_mention() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            allowed_users: vec![],
            dm_policy: "open".to_string(),
        };
        let adapter = TelegramAdapter::new(config);

        let update = serde_json::json!({
            "update_id": 101,
            "message": {
                "message_id": 43,
                "from": { "id": 12345, "first_name": "Jane" },
                "chat": { "id": -100123, "type": "group" },
                "text": "@mybot help me",
                "entities": [{ "type": "mention", "offset": 0, "length": 6 }]
            }
        });

        let msg = adapter.parse_update(&update).unwrap();
        assert!(!msg.is_dm);
        assert!(msg.is_mention);
        assert_eq!(msg.chat_id, "-100123");
    }

    #[test]
    fn test_parse_update_with_reply() {
        let config = TelegramAdapter::new(TelegramConfig {
            bot_token: "test".to_string(),
            allowed_users: vec![],
            dm_policy: "open".to_string(),
        });

        let update = serde_json::json!({
            "update_id": 102,
            "message": {
                "message_id": 44,
                "from": { "id": 12345, "first_name": "Bob" },
                "chat": { "id": 12345, "type": "private" },
                "text": "reply text",
                "reply_to_message": { "message_id": 40 }
            }
        });

        let msg = config.parse_update(&update).unwrap();
        assert_eq!(msg.reply_to, Some("40".to_string()));
    }

    #[test]
    fn test_parse_update_filtered_user() {
        let config = TelegramConfig {
            bot_token: "test".to_string(),
            allowed_users: vec![99999],
            dm_policy: "pairing".to_string(),
        };
        let adapter = TelegramAdapter::new(config);

        let update = serde_json::json!({
            "update_id": 103,
            "message": {
                "message_id": 45,
                "from": { "id": 12345, "first_name": "Blocked" },
                "chat": { "id": 12345, "type": "private" },
                "text": "should be filtered"
            }
        });

        assert!(adapter.parse_update(&update).is_none());
    }

    #[test]
    fn test_parse_update_with_photo() {
        let config = TelegramAdapter::new(TelegramConfig {
            bot_token: "test".to_string(),
            allowed_users: vec![],
            dm_policy: "open".to_string(),
        });

        let update = serde_json::json!({
            "update_id": 104,
            "message": {
                "message_id": 46,
                "from": { "id": 12345, "first_name": "Alice" },
                "chat": { "id": 12345, "type": "private" },
                "caption": "Check this out",
                "photo": [
                    { "file_id": "small", "width": 90, "height": 90, "file_size": 1000 },
                    { "file_id": "large", "width": 800, "height": 600, "file_size": 50000 }
                ]
            }
        });

        let msg = config.parse_update(&update).unwrap();
        assert_eq!(msg.content, "Check this out");
        assert_eq!(msg.attachments.len(), 1);
        assert_eq!(msg.attachments[0].attachment_type, AttachmentType::Image);
        assert_eq!(msg.attachments[0].width, Some(800));
        assert_eq!(msg.attachments[0].height, Some(600));
    }
}
