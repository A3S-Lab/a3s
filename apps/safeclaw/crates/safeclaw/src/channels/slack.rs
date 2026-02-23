//! Slack channel adapter

use super::adapter::{AdapterBase, AdapterStatus, ChannelAdapter, ChannelEvent};
use super::message::{InboundMessage, OutboundMessage};
use crate::config::SlackConfig;
use crate::error::{Error, Result};
use async_trait::async_trait;
use ring::hmac;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Slack channel adapter
pub struct SlackAdapter {
    config: SlackConfig,
    base: AdapterBase,
    bot_token: Arc<RwLock<Option<String>>>,
    client: reqwest::Client,
    event_tx: Arc<RwLock<Option<mpsc::Sender<ChannelEvent>>>>,
}

impl SlackAdapter {
    /// Create a new Slack adapter
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            base: AdapterBase::new("slack"),
            bot_token: Arc::new(RwLock::new(None)),
            client: reqwest::Client::new(),
            event_tx: Arc::new(RwLock::new(None)),
        }
    }

    /// Resolve bot token from env var or inline value
    fn resolve_token(token_ref: &str) -> Result<String> {
        super::resolve_credential(token_ref)
    }

    /// Check if a workspace is allowed
    pub fn is_workspace_allowed(&self, workspace_id: &str) -> bool {
        self.config.allowed_workspaces.is_empty()
            || self
                .config
                .allowed_workspaces
                .iter()
                .any(|w| w == workspace_id)
    }

    /// Verify request signature (HMAC-SHA256 with signing_secret)
    /// Slack signs requests with: v0=HMAC-SHA256(signing_secret, "v0:{timestamp}:{body}")
    pub fn verify_signature(
        signing_secret: &str,
        timestamp: &str,
        body: &str,
        expected: &str,
    ) -> Result<()> {
        // Check timestamp to prevent replay attacks (within 5 minutes)
        let ts: i64 = timestamp
            .parse()
            .map_err(|_| Error::Channel("Invalid timestamp format".to_string()))?;
        let now = chrono::Utc::now().timestamp();
        if (now - ts).abs() > 300 {
            return Err(Error::Channel("Request timestamp too old".to_string()));
        }

        let sig_basestring = format!("v0:{}:{}", timestamp, body);
        let key = hmac::Key::new(hmac::HMAC_SHA256, signing_secret.as_bytes());
        let signature = hmac::sign(&key, sig_basestring.as_bytes());
        let computed = format!("v0={}", hex::encode(signature.as_ref()));

        if computed != expected {
            return Err(Error::Channel("Invalid signature".to_string()));
        }

        Ok(())
    }

    /// Parse Slack event into InboundMessage
    pub fn parse_event(event: &SlackEvent) -> Result<InboundMessage> {
        match event {
            SlackEvent::Message {
                channel,
                user,
                text,
                ts,
                channel_type,
            } => {
                let mut msg = InboundMessage::new("slack", user, channel, text);
                msg.channel_message_id = ts.clone();
                msg.timestamp = Self::parse_slack_timestamp(ts)?;
                msg.is_dm = channel_type.as_deref() == Some("im");
                Ok(msg)
            }
            SlackEvent::AppMention {
                channel,
                user,
                text,
                ts,
            } => {
                let mut msg = InboundMessage::new("slack", user, channel, text);
                msg.channel_message_id = ts.clone();
                msg.timestamp = Self::parse_slack_timestamp(ts)?;
                msg.is_mention = true;
                Ok(msg)
            }
        }
    }

    /// Parse Slack timestamp (format: "1234567890.123456")
    fn parse_slack_timestamp(ts: &str) -> Result<i64> {
        let parts: Vec<&str> = ts.split('.').collect();
        if parts.is_empty() {
            return Err(Error::Channel("Invalid Slack timestamp".to_string()));
        }
        parts[0]
            .parse::<i64>()
            .map(|s| s * 1000)
            .map_err(|_| Error::Channel("Failed to parse Slack timestamp".to_string()))
    }
}

#[async_trait]
impl ChannelAdapter for SlackAdapter {
    fn name(&self) -> &str {
        self.base.name()
    }

    async fn start(&self, event_tx: mpsc::Sender<ChannelEvent>) -> Result<()> {
        self.base.set_status(AdapterStatus::Starting);

        // Resolve token lazily
        let token = Self::resolve_token(&self.config.bot_token)?;
        *self.bot_token.write().await = Some(token);

        *self.event_tx.write().await = Some(event_tx.clone());

        tracing::info!("Slack adapter starting");

        let _ = event_tx
            .send(ChannelEvent::Connected {
                channel: "slack".to_string(),
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
                    channel: "slack".to_string(),
                    reason: "Adapter stopped".to_string(),
                })
                .await;
        }

        *self.event_tx.write().await = None;
        self.base.set_status(AdapterStatus::Stopped);
        tracing::info!("Slack adapter stopped");
        Ok(())
    }

    async fn send_message(&self, message: OutboundMessage) -> Result<String> {
        if !self.base.is_running() {
            return Err(Error::Channel("Slack adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Slack bot token not initialized".to_string()))?
            .clone();

        tracing::debug!(
            "Sending message to Slack channel {}: {}",
            message.chat_id,
            message.content
        );

        let url = "https://slack.com/api/chat.postMessage";
        let payload = serde_json::json!({
            "channel": message.chat_id,
            "text": message.content,
            "mrkdwn": message.format == super::message::MessageFormat::Markdown,
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", bot_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to send Slack message: {}", e)))?;

        let result: SlackApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Slack response: {}", e)))?;

        if !result.ok {
            return Err(Error::Channel(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "unknown".to_string())
            )));
        }

        Ok(result.ts.unwrap_or_default())
    }

    async fn send_typing(&self, chat_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Slack adapter not running".to_string()));
        }

        tracing::debug!("Slack typing indicator not supported for chat {}", chat_id);
        Ok(())
    }

    async fn edit_message(&self, chat_id: &str, message_id: &str, content: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Slack adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Slack bot token not initialized".to_string()))?
            .clone();

        tracing::debug!(
            "Editing Slack message {} in channel {}: {}",
            message_id,
            chat_id,
            content
        );

        let url = "https://slack.com/api/chat.update";
        let payload = serde_json::json!({
            "channel": chat_id,
            "ts": message_id,
            "text": content,
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", bot_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to edit Slack message: {}", e)))?;

        let result: SlackApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Slack response: {}", e)))?;

        if !result.ok {
            return Err(Error::Channel(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "unknown".to_string())
            )));
        }

        Ok(())
    }

    async fn delete_message(&self, chat_id: &str, message_id: &str) -> Result<()> {
        if !self.base.is_running() {
            return Err(Error::Channel("Slack adapter not running".to_string()));
        }

        let bot_token = self
            .bot_token
            .read()
            .await
            .as_ref()
            .ok_or_else(|| Error::Channel("Slack bot token not initialized".to_string()))?
            .clone();

        tracing::debug!(
            "Deleting Slack message {} in channel {}",
            message_id,
            chat_id
        );

        let url = "https://slack.com/api/chat.delete";
        let payload = serde_json::json!({
            "channel": chat_id,
            "ts": message_id,
        });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", bot_token))
            .header("Content-Type", "application/json")
            .json(&payload)
            .send()
            .await
            .map_err(|e| Error::Channel(format!("Failed to delete Slack message: {}", e)))?;

        let result: SlackApiResponse = response
            .json()
            .await
            .map_err(|e| Error::Channel(format!("Failed to parse Slack response: {}", e)))?;

        if !result.ok {
            return Err(Error::Channel(format!(
                "Slack API error: {}",
                result.error.unwrap_or_else(|| "unknown".to_string())
            )));
        }

        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.base.is_running()
    }
}

/// Slack API response
#[derive(Debug, Deserialize)]
struct SlackApiResponse {
    ok: bool,
    error: Option<String>,
    ts: Option<String>,
}

/// Slack event types
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SlackEvent {
    #[serde(rename = "message")]
    Message {
        channel: String,
        user: String,
        text: String,
        ts: String,
        channel_type: Option<String>,
    },
    #[serde(rename = "app_mention")]
    AppMention {
        channel: String,
        user: String,
        text: String,
        ts: String,
    },
}

/// Hex encoding helper (avoids adding hex crate dependency)
mod hex {
    pub fn encode(data: &[u8]) -> String {
        data.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_config() -> SlackConfig {
        SlackConfig {
            bot_token: "TEST_SLACK_BOT_TOKEN".to_string(),
            app_token: "TEST_SLACK_APP_TOKEN".to_string(),
            allowed_workspaces: vec!["T01234567".to_string()],
            dm_policy: "pairing".to_string(),
        }
    }

    #[test]
    fn test_adapter_creation() {
        let config = create_test_config();
        let adapter = SlackAdapter::new(config);

        assert_eq!(adapter.name(), "slack");
        assert!(!adapter.is_connected());
    }

    #[test]
    fn test_resolve_token_missing() {
        let result = SlackAdapter::resolve_token("");
        assert!(result.is_err());
        let err_msg = result.err().unwrap().to_string();
        assert!(err_msg.contains("Failed to resolve"));
    }

    #[test]
    fn test_workspace_allowed() {
        let config = create_test_config();
        let adapter = SlackAdapter::new(config);

        assert!(adapter.is_workspace_allowed("T01234567"));
        assert!(!adapter.is_workspace_allowed("T99999999"));
    }

    #[test]
    fn test_empty_allowed_workspaces() {
        let config = SlackConfig {
            allowed_workspaces: vec![],
            ..create_test_config()
        };
        let adapter = SlackAdapter::new(config);

        assert!(adapter.is_workspace_allowed("any_workspace"));
    }

    #[test]
    fn test_verify_signature_valid() {
        let signing_secret = "test_signing_secret";
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let body = r#"token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J"#;

        let sig_basestring = format!("v0:{}:{}", timestamp, body);
        let key = hmac::Key::new(hmac::HMAC_SHA256, signing_secret.as_bytes());
        let signature = hmac::sign(&key, sig_basestring.as_bytes());
        let expected = format!("v0={}", hex::encode(signature.as_ref()));

        let result = SlackAdapter::verify_signature(signing_secret, &timestamp, body, &expected);
        assert!(result.is_ok());
    }

    #[test]
    fn test_verify_signature_invalid() {
        let signing_secret = "test_signing_secret";
        let timestamp = chrono::Utc::now().timestamp().to_string();
        let body = r#"token=xyzz0WbapA4vBCDEFasx0q6G&team_id=T1DC2JH3J"#;

        let result = SlackAdapter::verify_signature(signing_secret, &timestamp, body, "v0=wrong");
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid signature"));
    }

    #[test]
    fn test_verify_signature_old_timestamp() {
        let signing_secret = "test_signing_secret";
        let old_timestamp = (chrono::Utc::now().timestamp() - 400).to_string();
        let body = r#"token=test"#;

        let result = SlackAdapter::verify_signature(signing_secret, &old_timestamp, body, "v0=any");
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("too old"));
    }

    #[test]
    fn test_parse_event_message() {
        let event = SlackEvent::Message {
            channel: "C123456".to_string(),
            user: "U123456".to_string(),
            text: "Hello world".to_string(),
            ts: "1234567890.123456".to_string(),
            channel_type: Some("channel".to_string()),
        };

        let msg = SlackAdapter::parse_event(&event).unwrap();
        assert_eq!(msg.channel, "slack");
        assert_eq!(msg.sender_id, "U123456");
        assert_eq!(msg.chat_id, "C123456");
        assert_eq!(msg.content, "Hello world");
        assert_eq!(msg.channel_message_id, "1234567890.123456");
        assert!(!msg.is_dm);
        assert!(!msg.is_mention);
    }

    #[test]
    fn test_parse_event_dm() {
        let event = SlackEvent::Message {
            channel: "D123456".to_string(),
            user: "U123456".to_string(),
            text: "Private message".to_string(),
            ts: "1234567890.123456".to_string(),
            channel_type: Some("im".to_string()),
        };

        let msg = SlackAdapter::parse_event(&event).unwrap();
        assert!(msg.is_dm);
    }

    #[test]
    fn test_parse_event_mention() {
        let event = SlackEvent::AppMention {
            channel: "C123456".to_string(),
            user: "U123456".to_string(),
            text: "<@U987654> help me".to_string(),
            ts: "1234567890.123456".to_string(),
        };

        let msg = SlackAdapter::parse_event(&event).unwrap();
        assert!(msg.is_mention);
        assert!(!msg.is_dm);
    }

    #[test]
    fn test_parse_slack_timestamp() {
        let ts = "1234567890.123456";
        let result = SlackAdapter::parse_slack_timestamp(ts).unwrap();
        assert_eq!(result, 1234567890000);
    }

    #[test]
    fn test_parse_slack_timestamp_invalid() {
        let ts = "invalid";
        let result = SlackAdapter::parse_slack_timestamp(ts);
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_adapter_lifecycle() {
        std::env::set_var("TEST_SLACK_BOT_TOKEN", "xoxb-test-token");
        let config = create_test_config();
        let adapter = SlackAdapter::new(config);
        let (tx, mut rx) = mpsc::channel(10);

        adapter.start(tx).await.unwrap();
        assert!(adapter.is_connected());

        let event = rx.recv().await.unwrap();
        assert!(matches!(event, ChannelEvent::Connected { .. }));

        adapter.stop().await.unwrap();
        assert!(!adapter.is_connected());
    }
}
