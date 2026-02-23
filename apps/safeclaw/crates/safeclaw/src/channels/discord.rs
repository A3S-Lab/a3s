//! Discord channel adapter

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{InboundMessage, OutboundMessage};
use crate::config::DiscordConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Discord channel adapter
pub struct DiscordAdapter {
    config: DiscordConfig,
    base: AdapterBase,
    bot_token: Arc<RwLock<Option<String>>>,
    client: reqwest::Client,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
}

impl DiscordAdapter {
    /// Create a new Discord adapter
    pub fn new(config: DiscordConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("discord"),
            bot_token: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            event_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Resolve bot token from env var or inline value
    fn resolve_token(token_ref: &str) -> Result<String> {
        super::resolve_credential(token_ref)
    }

    /// Check if a guild is allowed
    pub fn is_guild_allowed(&self, guild_id: u64) -> bool {
        self.config.allowed_guilds.is_empty() || self.config.allowed_guilds.contains(&guild_id)
    }

    /// Parse Discord message into InboundMessage
    pub fn parse_message(msg: &DiscordMessage) -> Result<InboundMessage> {
        let mut inbound =
            InboundMessage::new("discord", &msg.author.id, &msg.channel_id, &msg.content);
        inbound.channel_message_id = msg.id.clone();
        inbound.sender_name = Some(msg.author.username.clone());
        inbound.timestamp = Self::parse_discord_timestamp(&msg.timestamp)?;
        inbound.is_dm = msg.guild_id.is_none();
        inbound.is_mention = msg.mentions.iter().any(|u| u.bot.unwrap_or(false));
        Ok(inbound)
    }

    /// Parse Discord timestamp (ISO 8601 format)
    fn parse_discord_timestamp(ts: &str) -> Result<i64> {
        chrono::DateTime::parse_from_rfc3339(ts)
            .map(|dt| dt.timestamp_millis())
            .map_err(|_| Error::Channel("Failed to parse Discord timestamp".to_string()))
    }
}

#[async_trait]
impl ChannelAdapter for DiscordAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        self.base.set_status(AdapterStatus::Starting);

        // Resolve token lazily
        let token = Self::resolve_token(&self.config.bot_token)?;
        *self.bot_token.write().await = Some(token);

        *self.event_tx.write().await = Some(event_tx.clone());

        tracing::info!("Discord adapter starting");

        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "discord".to_string(),
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
                    channel: "discord".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);
        tracing::info!("Discord adapter stopped");
        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("Discord adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Discord bot token not initialized".to_string()))?
            .clone();

        tracing::debug!(
            "Sending message to Discord channel {}: {}",
            message.chat_id,
            message.content
        );

        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages",
            message.chat_id
        );
        let payload = serde_json::json!({
            "content": message.content,
        });

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bot {}", bot_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to send Discord message: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Channel(format!(
                "Discord API error {}: {}",
                status, error_text
            )));
        }

        let result: DiscordMessage = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Discord response: {}", e)))?;

        Ok(result.id)
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Discord adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Discord bot token not initialized".to_string()))?
            .clone();

        tracing::debug!("Sending typing indicator to Discord channel {}", chat_id);

        let url = format!("https://discord.com/api/v10/channels/{}/typing", chat_id);

        let response = self
            .client
            .post(&url)
            .header("Authorization", format!("Bot {}", bot_token))
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to send Discord typing: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            return Err(Error::Channel(format!(
                "Discord typing API error: {}",
                status
            )));
        }

        Ok(())
    }

    async fn edit_message(&self, chat_id: &str, message_id: &str, content: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Discord adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Discord bot token not initialized".to_string()))?
            .clone();

        tracing::debug!(
            "Editing Discord message {} in channel {}: {}",
            message_id,
            chat_id,
            content
        );

        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages/{}",
            chat_id, message_id
        );
        let payload = serde_json::json!({
            "content": content,
        });

        let response = self
            .client
            .patch(&url)
            .header("Authorization", format!("Bot {}", bot_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to edit Discord message: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Channel(format!(
                "Discord API error {}: {}",
                status, error_text
            )));
        }

        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Discord adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Discord bot token not initialized".to_string()))?
            .clone();

        tracing::debug!(
            "Deleting Discord message {} in channel {}",
            message_id,
            chat_id
        );

        let url = format!(
            "https://discord.com/api/v10/channels/{}/messages/{}",
            chat_id, message_id
        );

        let response = self
            .client
            .delete(&url)
            .header("Authorization", format!("Bot {}", bot_token))
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to delete Discord message: {}", e)))?;

        if !response.status().is_success() {
            let status = response.status();
            let error_text = response.text().await.unwrap_or_default();
            return Err(Error::Channel(format!(
                "Discord API error {}: {}",
                status, error_text
            )));
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

/// Discord message structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordMessage {
    pub id: String,
    pub channel_id: String,
    pub guild_id: Option<String>,
    pub content: String,
    pub timestamp: String,
    pub author: DiscordUser,
    pub mentions: Vec<DiscordUser>,
}

/// Discord user structure
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscordUser {
    pub id: String,
    pub username: String,
    pub bot: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> DiscordConfig {
        DiscordConfig {
            bot_token: "TEST_DISCORD_BOT_TOKEN".to_string(),
            allowed_guilds: vec![123456789012345678],
            dm_policy: "pairing".to_string(),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = DiscordAdapter::new(config);

        assert_eq!(adapter.name(), "discord");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_resolve_token_missing() {
        let result = DiscordAdapter::resolve_token("");
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Failed to resolve"));
    }

    #[test]
    fn test_guild_allowed() {
        let config = create_test_config();
        let adapter = DiscordAdapter::new(config);

        assert!(adapter.is_guild_allowed(123456789012345678));
        assert!(!adapter.is_guild_allowed(999999999999999999));
    }

    #[test]
    fn test_empty_allowed_guilds() {
        let config = DiscordConfig {
            allowed_guilds: vec![],
            ..create_test_config()
        };
        let adapter = DiscordAdapter::new(config);

        assert!(adapter.is_guild_allowed(123456789012345678));
        assert!(adapter.is_guild_allowed(999999999999999999));
    }

    #[test]
    fn test_parse_message() {
        let msg = DiscordMessage {
            id: "123456789".to_string(),
            channel_id: "987654321".to_string(),
            guild_id: Some("111222333".to_string()),
            content: "Hello Discord!".to_string(),
            timestamp: "2024-01-01T12:00:00+00:00".to_string(),
            author: DiscordUser {
                id: "user123".to_string(),
                username: "testuser".to_string(),
                bot: Some(false),
            },
            mentions: vec![],
        };

        let inbound = DiscordAdapter::parse_message(&msg).unwrap();
        assert_eq!(inbound.channel, "discord");
        assert_eq!(inbound.sender_id, "user123");
        assert_eq!(inbound.chat_id, "987654321");
        assert_eq!(inbound.content, "Hello Discord!");
        assert_eq!(inbound.channel_message_id, "123456789");
        assert_eq!(inbound.sender_name, Some("testuser".to_string()));
        assert!(!inbound.is_dm);
        assert!(!inbound.is_mention);
    }

    #[test]
    fn test_parse_message_dm() {
        let msg = DiscordMessage {
            id: "123456789".to_string(),
            channel_id: "987654321".to_string(),
            guild_id: None,
            content: "Private message".to_string(),
            timestamp: "2024-01-01T12:00:00+00:00".to_string(),
            author: DiscordUser {
                id: "user123".to_string(),
                username: "testuser".to_string(),
                bot: Some(false),
            },
            mentions: vec![],
        };

        let inbound = DiscordAdapter::parse_message(&msg).unwrap();
        assert!(inbound.is_dm);
    }

    #[test]
    fn test_parse_message_mention() {
        let msg = DiscordMessage {
            id: "123456789".to_string(),
            channel_id: "987654321".to_string(),
            guild_id: Some("111222333".to_string()),
            content: "<@bot123> help".to_string(),
            timestamp: "2024-01-01T12:00:00+00:00".to_string(),
            author: DiscordUser {
                id: "user123".to_string(),
                username: "testuser".to_string(),
                bot: Some(false),
            },
            mentions: vec![DiscordUser {
                id: "bot123".to_string(),
                username: "mybot".to_string(),
                bot: Some(true),
            }],
        };

        let inbound = DiscordAdapter::parse_message(&msg).unwrap();
        assert!(inbound.is_mention);
    }

    #[test]
    fn test_parse_discord_timestamp() {
        let ts = "2024-01-01T12:00:00+00:00";
        let result = DiscordAdapter::parse_discord_timestamp(ts).unwrap();
        assert!(result > 0);
    }

    #[test]
    fn test_parse_discord_timestamp_invalid() {
        let ts = "invalid";
        let result = DiscordAdapter::parse_discord_timestamp(ts);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        std::env::set_var("TEST_DISCORD_BOT_TOKEN", "test_bot_token");
        let config = create_test_config();
        let adapter = DiscordAdapter::new(config);
        let (tx, mut rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();
        assert!(adapter.is_connected());

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, ChannelEvent::Connected { .. }));

        adapter.stop().await.unwrap();
        assert!(!adapter.is_connected());
    }
}
