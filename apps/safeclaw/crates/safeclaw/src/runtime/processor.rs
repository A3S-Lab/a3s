//! Message processor for SafeClaw runtime
//!
//! Owns the "route → process" pipeline that turns an inbound
//! message into a `ProcessedResponse`.

use crate::agent::AgentEngine;
use crate::channels::{ChannelAdapter, ChannelEvent, InboundMessage, OutboundMessage};
use crate::config::FeishuConfig;
use crate::error::{Error, Result};
use crate::session::SessionManager;
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// ProcessedResponse
// ---------------------------------------------------------------------------

/// Response from processing a message
#[derive(Debug, Clone, Serialize)]
pub struct ProcessedResponse {
    /// Session ID used for processing
    pub session_id: String,
    /// Whether TEE was used
    pub use_tee: bool,
    /// Sensitivity level detected
    pub sensitivity: String,
    /// Outbound message to send back
    pub outbound: OutboundMessage,
}

/// Result of parsing a webhook payload
#[derive(Debug)]
pub enum WebhookParseResult {
    /// A challenge response that must be returned synchronously
    Challenge(ProcessedResponse),
    /// A parsed inbound message ready for processing
    Message(InboundMessage),
    /// A card action callback
    CardAction(CardActionEvent),
    /// No actionable content
    Ignored,
}

/// A card button action event from a channel
#[derive(Debug, Clone)]
pub struct CardActionEvent {
    pub action: String,
    pub tag: String,
    pub user_id: String,
    pub chat_id: String,
    pub message_id: String,
    pub channel: String,
}

/// A sensitive message pending user authorization before processing.
#[derive(Debug, Clone)]
pub struct PendingSensitiveMessage {
    pub message: InboundMessage,
    pub card_message_id: String,
    pub sensitivity: String,
    pub created_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// MessageProcessor
// ---------------------------------------------------------------------------

pub struct MessageProcessor {
    session_manager: Arc<SessionManager>,
    agent_engine: Arc<RwLock<Option<Arc<AgentEngine>>>>,
    seen_message_ids: Arc<RwLock<VecDeque<String>>>,
    feishu_config: Option<FeishuConfig>,
    pending_sensitive: Arc<RwLock<HashMap<String, PendingSensitiveMessage>>>,
}

impl MessageProcessor {
    pub fn new(session_manager: Arc<SessionManager>, feishu_config: Option<FeishuConfig>) -> Self {
        Self {
            session_manager,
            agent_engine: Arc::new(RwLock::new(None)),
            seen_message_ids: Arc::new(RwLock::new(VecDeque::with_capacity(256))),
            feishu_config,
            pending_sensitive: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub async fn set_agent_engine(&self, engine: Arc<AgentEngine>) {
        *self.agent_engine.write().await = Some(engine);
    }

    // -----------------------------------------------------------------------
    // Core pipeline
    // -----------------------------------------------------------------------

    pub async fn process_message(&self, message: InboundMessage) -> Result<ProcessedResponse> {
        tracing::info!(
            channel = %message.channel,
            sender = %message.sender_id,
            chat_id = %message.chat_id,
            content = %message.content,
            "Processing inbound message"
        );

        let decision = self.session_manager.route_message(&message).await?;

        tracing::info!(
            session = %decision.session_id,
            use_tee = decision.use_tee,
            "Routing decision"
        );

        // --- Process ---
        let response_content = if decision.use_tee {
            self.session_manager
                .process_in_tee(&decision.session_id, &message.content)
                .await?
        } else {
            let engine = self.agent_engine.read().await;
            if let Some(engine) = engine.as_ref() {
                engine
                    .generate_response(&decision.session_id, &message.content)
                    .await
                    .unwrap_or_else(|e| {
                        tracing::error!(
                            session = %decision.session_id,
                            "Agent generation failed: {}", e
                        );
                        format!(
                            "Sorry, I encountered an error processing your message: {}",
                            e
                        )
                    })
            } else {
                tracing::warn!("No agent engine configured, cannot process message");
                "Agent engine not configured. Please set up an LLM provider.".to_string()
            }
        };

        let outbound = OutboundMessage::new(&message.channel, &message.chat_id, &response_content)
            .reply_to(&message.channel_message_id);

        Ok(ProcessedResponse {
            session_id: decision.session_id,
            use_tee: decision.use_tee,
            sensitivity: format!("{:?}", decision.sensitivity),
            outbound,
        })
    }

    /// Process a message with streaming updates to the channel.
    pub async fn process_message_streaming(
        &self,
        message: InboundMessage,
        adapter: Arc<dyn ChannelAdapter>,
    ) -> Result<ProcessedResponse> {
        use a3s_code::agent::AgentEvent;

        tracing::info!(
            channel = %message.channel,
            sender = %message.sender_id,
            content = %message.content,
            "Processing inbound message (streaming)"
        );

        let decision = self.session_manager.route_message(&message).await?;

        // TEE path: not supported in streaming mode, fall back to non-streaming
        if decision.use_tee {
            return self.process_message(message).await;
        }

        // --- Send placeholder ---
        let placeholder = OutboundMessage::new(&message.channel, &message.chat_id, "正在处理中...");
        let placeholder_id = match adapter.send_message(placeholder).await {
            Ok(id) => id,
            Err(e) => {
                tracing::error!("Failed to send placeholder: {}", e);
                return Err(e);
            }
        };

        // --- Stream generation ---
        let engine = self.agent_engine.read().await;
        let engine = engine
            .as_ref()
            .ok_or_else(|| Error::Runtime("Agent engine not configured".to_string()))?;

        let (mut event_rx, _join_handle) = engine
            .generate_response_streaming(&decision.session_id, &message.content)
            .await?;

        let mut text_buffer = String::new();
        let mut last_update = tokio::time::Instant::now();
        let throttle = std::time::Duration::from_millis(1000);
        let timeout_duration = std::time::Duration::from_secs(120);

        loop {
            match tokio::time::timeout(timeout_duration, event_rx.recv()).await {
                Ok(Some(event)) => match &event {
                    AgentEvent::TextDelta { text: delta } => {
                        text_buffer.push_str(delta);
                        if last_update.elapsed() >= throttle {
                            let display = format!("{}▍", &text_buffer);
                            if let Err(e) = adapter
                                .edit_message(&message.chat_id, &placeholder_id, &display)
                                .await
                            {
                                tracing::warn!("Failed to update streaming message: {}", e);
                            }
                            last_update = tokio::time::Instant::now();
                        }
                    }
                    AgentEvent::End {
                        text: final_text, ..
                    } => {
                        if !final_text.is_empty() {
                            text_buffer = final_text.clone();
                        }
                        break;
                    }
                    AgentEvent::Error { message: err_msg } => {
                        let error_text = format!("⚠️ {}", err_msg);
                        let _ = adapter
                            .edit_message(&message.chat_id, &placeholder_id, &error_text)
                            .await;
                        return Err(Error::Runtime(format!("Agent error: {}", err_msg)));
                    }
                    _ => {}
                },
                Ok(None) => break,
                Err(_) => {
                    tracing::warn!(
                        session = %decision.session_id,
                        "Agent generation timed out after 120s"
                    );
                    break;
                }
            }
        }

        let final_content = if text_buffer.is_empty() {
            "I received your message but couldn't generate a response.".to_string()
        } else {
            text_buffer
        };

        tracing::info!(
            session = %decision.session_id,
            response_len = final_content.len(),
            "Streaming generation completed"
        );

        if let Err(e) = adapter
            .edit_message(&message.chat_id, &placeholder_id, &final_content)
            .await
        {
            tracing::error!("Failed to send final streaming update: {}", e);
        }

        let outbound = OutboundMessage::new(&message.channel, &message.chat_id, &final_content)
            .reply_to(&message.channel_message_id);

        Ok(ProcessedResponse {
            session_id: decision.session_id,
            use_tee: decision.use_tee,
            sensitivity: format!("{:?}", decision.sensitivity),
            outbound,
        })
    }

    // -----------------------------------------------------------------------
    // Card action handling
    // -----------------------------------------------------------------------

    pub async fn handle_card_action(
        &self,
        action: CardActionEvent,
        adapter: Arc<dyn ChannelAdapter>,
    ) -> Result<Option<serde_json::Value>> {
        let pending = self
            .pending_sensitive
            .write()
            .await
            .remove(&action.message_id);

        let pending = match pending {
            Some(p) => p,
            None => {
                tracing::warn!(
                    message_id = %action.message_id,
                    "Card action for unknown or expired pending message"
                );
                return Ok(None);
            }
        };

        let chat_id = if action.chat_id.is_empty() {
            pending.message.chat_id.clone()
        } else {
            action.chat_id.clone()
        };

        match action.action.as_str() {
            "authorize" => {
                tracing::info!(
                    user = %action.user_id,
                    message_id = %action.message_id,
                    "User authorized sensitive message processing"
                );

                let session_manager_clone = self.session_manager.clone();
                let message = pending.message.clone();
                let msg_id = action.message_id.clone();
                let adapter = adapter.clone();

                tokio::spawn(async move {
                    let decision = match session_manager_clone.route_message(&message).await {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::error!("Route failed after authorization: {}", e);
                            return;
                        }
                    };

                    match session_manager_clone
                        .process_in_tee(&decision.session_id, &message.content)
                        .await
                    {
                        Ok(content) => {
                            let _ = adapter.edit_message(&chat_id, &msg_id, &content).await;
                        }
                        Err(e) => {
                            tracing::warn!("TEE processing failed: {}", e);
                            let _ = adapter
                                .edit_message(&chat_id, &msg_id, "TEE processing failed.")
                                .await;
                        }
                    }
                });

                Ok(None)
            }
            "cancel" => {
                tracing::info!(
                    user = %action.user_id,
                    message_id = %action.message_id,
                    "User cancelled sensitive message processing"
                );
                Ok(None)
            }
            other => {
                tracing::warn!(action = other, "Unknown card action");
                Ok(None)
            }
        }
    }

    // -----------------------------------------------------------------------
    // Webhook dispatch
    // -----------------------------------------------------------------------

    pub async fn process_webhook(
        &self,
        channel: &str,
        payload: &str,
    ) -> Result<Option<ProcessedResponse>> {
        match self.parse_webhook(channel, payload).await? {
            WebhookParseResult::Challenge(response) => Ok(Some(response)),
            WebhookParseResult::Message(msg) => Ok(Some(self.process_message(msg).await?)),
            WebhookParseResult::CardAction(_) => {
                tracing::debug!("CardAction received in process_webhook (no adapter), ignoring");
                Ok(None)
            }
            WebhookParseResult::Ignored => Ok(None),
        }
    }

    pub async fn parse_webhook(&self, channel: &str, payload: &str) -> Result<WebhookParseResult> {
        match channel {
            "feishu" => self.parse_feishu_webhook(payload).await,
            "discord" => self.parse_discord_webhook(payload),
            "telegram" | "slack" | "dingtalk" | "wecom" => {
                match self.parse_generic_webhook(channel, payload)? {
                    Some(msg) => Ok(WebhookParseResult::Message(msg)),
                    None => Ok(WebhookParseResult::Ignored),
                }
            }
            _ => Err(Error::Channel(format!("Unknown channel: {}", channel))),
        }
    }

    fn parse_discord_webhook(&self, payload: &str) -> Result<WebhookParseResult> {
        use crate::channels::{DiscordAdapter, DiscordMessage};

        let msg: DiscordMessage = serde_json::from_str(payload)
            .map_err(|e| Error::Channel(format!("Failed to parse Discord payload: {}", e)))?;

        if msg.author.bot.unwrap_or(false) {
            return Ok(WebhookParseResult::Ignored);
        }

        if msg.content.trim().is_empty() {
            return Ok(WebhookParseResult::Ignored);
        }

        let inbound = DiscordAdapter::parse_message(&msg)?;
        Ok(WebhookParseResult::Message(inbound))
    }

    pub async fn handle_channel_event(
        &self,
        event: ChannelEvent,
        channels: &Arc<RwLock<HashMap<String, Arc<dyn ChannelAdapter>>>>,
    ) -> Result<()> {
        match event {
            ChannelEvent::Message(message) => {
                tracing::debug!(
                    sender = %message.sender_id,
                    channel = %message.channel,
                    "Received message: {}",
                    message.content
                );

                let channel_name = message.channel.clone();
                let response = self.process_message(message).await?;

                let channels = channels.read().await;
                if let Some(channel) = channels.get(&channel_name) {
                    channel.send_message(response.outbound).await?;
                }
            }
            ChannelEvent::Connected { channel } => {
                tracing::info!("Channel {} connected", channel);
            }
            ChannelEvent::Disconnected { channel, reason } => {
                tracing::warn!("Channel {} disconnected: {}", channel, reason);
            }
            ChannelEvent::Error { channel, error } => {
                tracing::error!("Channel {} error: {}", channel, error);
            }
            _ => {
                tracing::debug!("Unhandled event: {:?}", event);
            }
        }
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Private helpers
    // -----------------------------------------------------------------------

    async fn parse_feishu_webhook(&self, payload: &str) -> Result<WebhookParseResult> {
        let payload = if let Some(feishu_config) = &self.feishu_config {
            if !feishu_config.encrypt_key.is_empty() {
                let encrypt_key = crate::channels::resolve_credential(&feishu_config.encrypt_key)
                    .unwrap_or_default();
                if !encrypt_key.is_empty() {
                    let outer: serde_json::Value =
                        serde_json::from_str(payload).map_err(|e| Error::Channel(e.to_string()))?;
                    if let Some(enc) = outer.get("encrypt").and_then(|v| v.as_str()) {
                        std::borrow::Cow::Owned(feishu_decrypt(enc, &encrypt_key)?)
                    } else {
                        std::borrow::Cow::Borrowed(payload)
                    }
                } else {
                    std::borrow::Cow::Borrowed(payload)
                }
            } else {
                std::borrow::Cow::Borrowed(payload)
            }
        } else {
            std::borrow::Cow::Borrowed(payload)
        };
        let payload: &str = &payload;
        let parsed: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| Error::Channel(e.to_string()))?;

        tracing::info!(
            "Feishu decrypted payload: {}",
            serde_json::to_string(&parsed).unwrap_or_default()
        );

        // Case 1: URL verification challenge
        if let Some(challenge) = parsed.get("challenge") {
            tracing::info!("Feishu URL verification challenge received");
            let challenge_str = challenge.as_str().unwrap_or_default();
            let outbound = OutboundMessage::new("feishu", "__challenge__", challenge_str);
            return Ok(WebhookParseResult::Challenge(ProcessedResponse {
                session_id: String::new(),
                use_tee: false,
                sensitivity: "none".to_string(),
                outbound,
            }));
        }

        let event_type = parsed
            .pointer("/header/event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Card action callback
        if event_type == "card.action.trigger" {
            let action_value = parsed
                .pointer("/event/action/value")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));
            let action_str = action_value
                .get("action")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tag = parsed
                .pointer("/event/action/tag")
                .and_then(|v| v.as_str())
                .unwrap_or("button")
                .to_string();
            let user_id = parsed
                .pointer("/event/operator/open_id")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let message_id = parsed
                .pointer("/event/context/open_message_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let chat_id = parsed
                .pointer("/event/context/open_chat_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if action_str.is_empty() || message_id.is_empty() {
                return Ok(WebhookParseResult::Ignored);
            }

            return Ok(WebhookParseResult::CardAction(CardActionEvent {
                action: action_str,
                tag,
                user_id,
                chat_id,
                message_id,
                channel: "feishu".to_string(),
            }));
        }

        if event_type != "im.message.receive_v1" {
            return Ok(WebhookParseResult::Ignored);
        }

        let event = parsed.get("event").ok_or_else(|| {
            Error::Channel("Feishu event callback missing 'event' field".to_string())
        })?;

        let sender_open_id = event
            .pointer("/sender/sender_id/open_id")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown");

        let message = event
            .get("message")
            .ok_or_else(|| Error::Channel("Feishu event missing 'message' field".to_string()))?;

        let message_id = message["message_id"].as_str().unwrap_or("");
        let chat_id = message["chat_id"].as_str().unwrap_or("");
        let chat_type = message["chat_type"].as_str().unwrap_or("");
        let msg_type = message["message_type"].as_str().unwrap_or("");

        // Deduplication
        if !message_id.is_empty() {
            let mut seen = self.seen_message_ids.write().await;
            if seen.iter().any(|id| id == message_id) {
                return Ok(WebhookParseResult::Ignored);
            }
            if seen.len() >= 256 {
                seen.pop_front();
            }
            seen.push_back(message_id.to_string());
        }

        if msg_type != "text" {
            return Ok(WebhookParseResult::Ignored);
        }

        let content_str = message["content"].as_str().unwrap_or("{}");
        let content_parsed: serde_json::Value = serde_json::from_str(content_str)
            .unwrap_or_else(|_| serde_json::json!({"text": content_str}));
        let text = content_parsed["text"].as_str().unwrap_or("").to_string();

        if text.is_empty() {
            return Ok(WebhookParseResult::Ignored);
        }

        // ACL check
        if let Some(feishu_config) = &self.feishu_config {
            if !feishu_config.allowed_users.is_empty()
                && !feishu_config
                    .allowed_users
                    .iter()
                    .any(|u| u == sender_open_id)
            {
                tracing::warn!(
                    sender = sender_open_id,
                    "Feishu message from unauthorized user, ignoring"
                );
                return Ok(WebhookParseResult::Ignored);
            }
        }

        let mut inbound = InboundMessage::new("feishu", sender_open_id, chat_id, &text);
        inbound.channel_message_id = message_id.to_string();
        inbound.is_dm = chat_type == "p2p";

        if let Some(mentions) = message.get("mentions").and_then(|m| m.as_array()) {
            inbound.is_mention = mentions
                .iter()
                .any(|m| m.pointer("/id/open_id").and_then(|v| v.as_str()) != Some(sender_open_id));
        }

        Ok(WebhookParseResult::Message(inbound))
    }

    fn parse_generic_webhook(
        &self,
        channel: &str,
        payload: &str,
    ) -> Result<Option<InboundMessage>> {
        let parsed: serde_json::Value =
            serde_json::from_str(payload).map_err(|e| Error::Channel(e.to_string()))?;

        let content = parsed["content"]
            .as_str()
            .or_else(|| parsed["text"].as_str())
            .or_else(|| parsed["message"].as_str())
            .unwrap_or("")
            .to_string();

        if content.is_empty() {
            return Ok(None);
        }

        let sender_id = parsed["sender_id"]
            .as_str()
            .or_else(|| parsed["user_id"].as_str())
            .or_else(|| parsed["from"].as_str())
            .unwrap_or("unknown")
            .to_string();

        let chat_id = parsed["chat_id"]
            .as_str()
            .or_else(|| parsed["channel_id"].as_str())
            .unwrap_or(&sender_id)
            .to_string();

        Ok(Some(InboundMessage::new(
            channel, &sender_id, &chat_id, &content,
        )))
    }
}

// ---------------------------------------------------------------------------
// Feishu AES-256-CBC decryption
// ---------------------------------------------------------------------------

fn feishu_decrypt(encrypt: &str, encrypt_key: &str) -> Result<String> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    use base64::Engine;
    use sha2::{Digest, Sha256};

    let key: [u8; 32] = Sha256::digest(encrypt_key.as_bytes()).into();

    let raw = base64::engine::general_purpose::STANDARD
        .decode(encrypt)
        .map_err(|e| Error::Channel(format!("Feishu decrypt: invalid base64: {}", e)))?;

    if raw.len() < 16 {
        return Err(Error::Channel(
            "Feishu decrypt: encrypted payload too short".to_string(),
        ));
    }

    let (iv, ciphertext) = raw.split_at(16);
    let iv: [u8; 16] = iv.try_into().expect("iv is 16 bytes");

    type Aes256CbcDec = cbc::Decryptor<aes::Aes256>;
    let mut buf = ciphertext.to_vec();
    let plaintext = Aes256CbcDec::new(&key.into(), &iv.into())
        .decrypt_padded_mut::<Pkcs7>(&mut buf)
        .map_err(|e| Error::Channel(format!("Feishu decrypt: AES error: {}", e)))?;

    String::from_utf8(plaintext.to_vec())
        .map_err(|e| Error::Channel(format!("Feishu decrypt: invalid UTF-8: {}", e)))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::SafeClawConfig;
    use crate::session::SessionManager;
    use std::sync::Arc;

    fn make_processor() -> MessageProcessor {
        let config = SafeClawConfig::default();
        let session_manager = Arc::new(SessionManager::new(config.tee.clone()));
        MessageProcessor::new(session_manager, None)
    }

    #[test]
    fn test_parse_generic_webhook_empty_content() {
        let p = make_processor();
        let payload = r#"{"user_id": "u1"}"#;
        let result = p.parse_generic_webhook("slack", payload).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_generic_webhook_with_content() {
        let p = make_processor();
        let payload = r#"{"sender_id": "u1", "chat_id": "c1", "content": "hello"}"#;
        let msg = p.parse_generic_webhook("slack", payload).unwrap().unwrap();
        assert_eq!(msg.content, "hello");
        assert_eq!(msg.sender_id, "u1");
        assert_eq!(msg.channel, "slack");
    }

    #[tokio::test]
    async fn test_process_webhook_unknown_channel() {
        let p = make_processor();
        let result = p.process_webhook("unknown", "{}").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_feishu_challenge() {
        let p = make_processor();
        let payload = r#"{"challenge": "abc123", "token": "t", "type": "url_verification"}"#;
        let result = p.process_webhook("feishu", payload).await.unwrap().unwrap();
        assert_eq!(result.outbound.chat_id, "__challenge__");
        assert_eq!(result.outbound.content, "abc123");
    }

    #[tokio::test]
    async fn test_feishu_non_message_event_ignored() {
        let p = make_processor();
        let payload = r#"{"schema": "2.0", "header": {"event_type": "bot.added"}}"#;
        let result = p.process_webhook("feishu", payload).await.unwrap();
        assert!(result.is_none());
    }
}
