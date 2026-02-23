//! Message processor for SafeClaw runtime
//!
//! Owns the "route → process → sanitize" pipeline that turns an inbound
//! message into a `ProcessedResponse`.  Extracted from `Runtime` so that
//! the server module can focus on lifecycle and infrastructure.
//!
//! **Single responsibility**: given any `InboundMessage`, produce a
//! `ProcessedResponse` (or an error).  Channel dispatch and audit
//! pipeline setup live elsewhere.

use crate::agent::AgentEngine;
use crate::channels::{ChannelAdapter, ChannelEvent, InboundMessage, OutboundMessage};
use crate::config::FeishuConfig;
use crate::error::{Error, Result};
use crate::privacy::CumulativeRiskDecision;
use crate::session::{SessionManager, SessionRouter};
use serde::Serialize;
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;
use tokio::sync::RwLock;

// ---------------------------------------------------------------------------
// ProcessedResponse — shared with server.rs via mod.rs re-export
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
    /// A challenge response that must be returned synchronously (e.g., Feishu URL verification)
    Challenge(ProcessedResponse),
    /// A parsed inbound message ready for processing
    Message(InboundMessage),
    /// A card action callback (e.g., Feishu interactive card button click)
    CardAction(CardActionEvent),
    /// No actionable content (e.g., non-message event, duplicate)
    Ignored,
}

/// A card button action event from a channel (e.g., Feishu card.action.trigger)
#[derive(Debug, Clone)]
pub struct CardActionEvent {
    /// The action value set on the button (e.g., "authorize" or "cancel")
    pub action: String,
    /// Opaque action tag (button name / key)
    pub tag: String,
    /// The user who clicked the button
    pub user_id: String,
    /// The chat where the card was posted
    pub chat_id: String,
    /// The message ID of the card
    pub message_id: String,
    /// Channel name
    pub channel: String,
}

/// A sensitive message pending user authorization before processing.
#[derive(Debug, Clone)]
pub struct PendingSensitiveMessage {
    /// The original inbound message
    pub message: InboundMessage,
    /// The card message ID (for updating after authorization)
    pub card_message_id: String,
    /// Sensitivity level description
    pub sensitivity: String,
    /// Timestamp when the pending entry was created
    pub created_at: std::time::Instant,
}

// ---------------------------------------------------------------------------
// MessageProcessor
// ---------------------------------------------------------------------------

/// Routes, processes, and sanitizes inbound messages.
///
/// All state needed to turn an `InboundMessage` into a `ProcessedResponse`
/// lives here.  `Runtime` holds an `Arc<MessageProcessor>` and delegates
/// public message-handling API to it.
pub struct MessageProcessor {
    session_router: Arc<SessionRouter>,
    session_manager: Arc<SessionManager>,
    agent_engine: Arc<RwLock<Option<Arc<AgentEngine>>>>,
    /// Deduplication window for channels that retry webhook delivery
    seen_message_ids: Arc<RwLock<VecDeque<String>>>,
    /// Optional Feishu config for ACL and dedup
    feishu_config: Option<FeishuConfig>,
    /// Sensitive messages awaiting user authorization (keyed by card message ID)
    pending_sensitive: Arc<RwLock<HashMap<String, PendingSensitiveMessage>>>,
}

impl MessageProcessor {
    /// Create a new processor.
    pub fn new(
        session_router: Arc<SessionRouter>,
        session_manager: Arc<SessionManager>,
        feishu_config: Option<FeishuConfig>,
    ) -> Self {
        Self {
            session_router,
            session_manager,
            agent_engine: Arc::new(RwLock::new(None)),
            seen_message_ids: Arc::new(RwLock::new(VecDeque::with_capacity(256))),
            feishu_config,
            pending_sensitive: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Wire up the LLM agent engine.  Must be called before the first message.
    pub async fn set_agent_engine(&self, engine: Arc<AgentEngine>) {
        *self.agent_engine.write().await = Some(engine);
    }

    /// Expose the session router (for external inspection / tests).
    pub fn session_router(&self) -> &Arc<SessionRouter> {
        &self.session_router
    }

    // -----------------------------------------------------------------------
    // Core pipeline: route → process → sanitize
    // -----------------------------------------------------------------------

    /// Route a message, process it (TEE or LLM), sanitize output.
    ///
    /// This is the single implementation of the route→process→sanitize
    /// pipeline.  Both the channel event loop and the direct HTTP API
    /// call this method.
    pub async fn process_message(&self, message: InboundMessage) -> Result<ProcessedResponse> {
        tracing::info!(
            channel = %message.channel,
            sender = %message.sender_id,
            chat_id = %message.chat_id,
            content = %message.content,
            "Processing inbound message"
        );

        let decision = self.session_router.route(&message).await?;

        tracing::info!(
            session = %decision.session_id,
            use_tee = decision.use_tee,
            level = ?decision.classification.level,
            "Routing decision"
        );

        // Reject sessions that have exceeded the cumulative PII disclosure limit.
        if decision.cumulative_decision == CumulativeRiskDecision::Reject {
            return Err(Error::Privacy(
                "Session blocked: cumulative PII disclosure limit exceeded".to_string(),
            ));
        }

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
                            "Agent generation failed: {}",
                            e
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

        tracing::info!(
            session = %decision.session_id,
            response_len = response_content.len(),
            "Agent generation completed"
        );

        // --- Sanitize ---
        let sanitized = self
            .session_manager
            .sanitize_output(&decision.session_id, &response_content)
            .await;
        let response_content = if sanitized.was_redacted {
            tracing::warn!(
                session = %decision.session_id,
                redactions = sanitized.redaction_count,
                "Redacted tainted data from agent output"
            );
            sanitized.sanitized_text
        } else {
            response_content
        };

        let outbound = OutboundMessage::new(&message.channel, &message.chat_id, &response_content)
            .reply_to(&message.channel_message_id);

        Ok(ProcessedResponse {
            session_id: decision.session_id,
            use_tee: decision.use_tee,
            sensitivity: format!("{:?}", decision.classification.level),
            outbound,
        })
    }

    /// Process a message with streaming updates to the channel.
    ///
    /// 1. Routes the message (classification, session lookup)
    /// 2. Sends a placeholder "正在处理中..." message via the adapter
    /// 3. Streams LLM events, periodically editing the placeholder
    /// 4. Does a final edit with the sanitized complete response
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

        let decision = self.session_router.route(&message).await?;

        if decision.cumulative_decision == CumulativeRiskDecision::Reject {
            let warning = "⚠️ **会话安全限制**\n\n\
                该会话已达到累积隐私数据披露上限，为保护您的数据安全，后续消息将不再处理。\n\n\
                请开启新的对话继续使用。";
            let outbound = OutboundMessage::new(&message.channel, &message.chat_id, warning)
                .reply_to(&message.channel_message_id);
            let _ = adapter.send_message(outbound.clone()).await;
            return Ok(ProcessedResponse {
                session_id: decision.session_id,
                use_tee: false,
                sensitivity: format!("{:?}", decision.classification.level),
                outbound,
            });
        }

        // TEE path: send interactive card with authorization buttons
        if decision.use_tee {
            let sensitivity_desc = format!("{}", decision.classification.level);
            let card = serde_json::json!({
                "config": {
                    "wide_screen_mode": true
                },
                "header": {
                    "template": "orange",
                    "title": {
                        "tag": "plain_text",
                        "content": "\u{1f512} 隐私数据保护 — 需要您的授权"
                    }
                },
                "elements": [
                    {
                        "tag": "div",
                        "fields": [
                            {
                                "is_short": true,
                                "text": {
                                    "tag": "lark_md",
                                    "content": format!("**敏感等级**\n{}", sensitivity_desc)
                                }
                            },
                            {
                                "is_short": true,
                                "text": {
                                    "tag": "lark_md",
                                    "content": format!("**检测时间**\n{}", chrono::Local::now().format("%Y-%m-%d %H:%M:%S"))
                                }
                            }
                        ]
                    },
                    {
                        "tag": "div",
                        "text": {
                            "tag": "lark_md",
                            "content": "检测到您的消息包含敏感信息，为保护您的数据安全，需要在**安全执行环境（TEE）**中处理此消息。\n\n请确认是否授权处理："
                        }
                    },
                    {
                        "tag": "action",
                        "actions": [
                            {
                                "tag": "button",
                                "text": {
                                    "tag": "plain_text",
                                    "content": "\u{2705} 授权处理"
                                },
                                "type": "primary",
                                "value": {
                                    "action": "authorize"
                                }
                            },
                            {
                                "tag": "button",
                                "text": {
                                    "tag": "plain_text",
                                    "content": "\u{274c} 取消"
                                },
                                "type": "danger",
                                "value": {
                                    "action": "cancel"
                                }
                            }
                        ]
                    },
                    {
                        "tag": "hr"
                    },
                    {
                        "tag": "note",
                        "elements": [
                            {
                                "tag": "lark_md",
                                "content": "\u{1f6e1}\u{fe0f} SafeClaw 隐私保护引擎"
                            }
                        ]
                    }
                ]
            });

            let card_outbound =
                OutboundMessage::new(&message.channel, &message.chat_id, &sensitivity_desc)
                    .with_card(card);
            let card_msg_id = match adapter.send_message(card_outbound).await {
                Ok(id) => id,
                Err(e) => {
                    tracing::error!("Failed to send authorization card: {}", e);
                    return Err(e);
                }
            };

            // Store the pending message for later processing when user clicks a button
            let pending = PendingSensitiveMessage {
                message: message.clone(),
                card_message_id: card_msg_id.clone(),
                sensitivity: sensitivity_desc.clone(),
                created_at: std::time::Instant::now(),
            };
            self.pending_sensitive
                .write()
                .await
                .insert(card_msg_id.clone(), pending);

            tracing::info!(
                session = %decision.session_id,
                card_message_id = %card_msg_id,
                "Sent authorization card for sensitive message, awaiting user action"
            );

            let outbound = OutboundMessage::new(
                &message.channel,
                &message.chat_id,
                "Awaiting authorization...",
            );
            return Ok(ProcessedResponse {
                session_id: decision.session_id,
                use_tee: true,
                sensitivity: sensitivity_desc,
                outbound,
            });
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

        // --- Sanitize & final update ---
        let response_content = if text_buffer.is_empty() {
            "I received your message but couldn't generate a response.".to_string()
        } else {
            text_buffer
        };

        let sanitized = self
            .session_manager
            .sanitize_output(&decision.session_id, &response_content)
            .await;
        let final_content = if sanitized.was_redacted {
            sanitized.sanitized_text
        } else {
            response_content
        };

        tracing::info!(
            session = %decision.session_id,
            response_len = final_content.len(),
            "Streaming generation completed"
        );

        // Final edit — remove cursor indicator
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
            sensitivity: format!("{:?}", decision.classification.level),
            outbound,
        })
    }

    // -----------------------------------------------------------------------
    // Card action handling (authorization flow)
    // -----------------------------------------------------------------------

    /// Handle a card button action (e.g., user clicked "authorize" or "cancel").
    ///
    /// Looks up the pending sensitive message by `card_message_id`, then either
    /// processes it in TEE (authorize) or sends a cancellation notice (cancel).
    ///
    /// Returns an optional updated card JSON to send back as the synchronous
    /// response (Feishu card callbacks can return updated card content within 3s).
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
                let msg_id = action.message_id.clone();
                let adapter = adapter.clone();
                tokio::spawn(async move {
                    let expired_card = Self::make_result_card(
                        "grey",
                        "\u{23f0} 请求已过期",
                        "该授权请求已过期或已被处理。",
                    );
                    let _ = adapter.edit_message_card("", &msg_id, &expired_card).await;
                });
                return Ok(None);
            }
        };

        // Resolve chat_id: prefer action callback, fall back to pending message
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

                // Spawn background: first update to loading card, then TEE processing
                let session_router = self.session_router.clone();
                let session_manager = self.session_manager.clone();
                let message = pending.message.clone();
                let msg_id = action.message_id.clone();
                let adapter = adapter.clone();

                tokio::spawn(async move {
                    // Update card to loading state (no buttons)
                    let loading_card = Self::make_loading_card(
                        "blue",
                        "\u{1f512} 已授权 — 处理中",
                        "正在安全执行环境（TEE）中处理您的消息，请稍候...",
                    );
                    let _ = adapter
                        .edit_message_card(&chat_id, &msg_id, &loading_card)
                        .await;
                    let decision = match session_router.route(&message).await {
                        Ok(d) => d,
                        Err(e) => {
                            tracing::error!("Route failed after authorization: {}", e);
                            let err_card = Self::make_result_card(
                                "red",
                                "\u{26a0}\u{fe0f} 处理失败",
                                &format!("路由失败：{}", e),
                            );
                            let _ = adapter
                                .edit_message_card(&chat_id, &msg_id, &err_card)
                                .await;
                            return;
                        }
                    };

                    match session_manager
                        .process_in_tee(&decision.session_id, &message.content)
                        .await
                    {
                        Ok(content) => {
                            let sanitized = session_manager
                                .sanitize_output(&decision.session_id, &content)
                                .await;
                            let final_content = if sanitized.was_redacted {
                                sanitized.sanitized_text
                            } else {
                                content
                            };
                            let success_card = Self::make_result_card(
                                "green",
                                "\u{2705} 处理完成",
                                &final_content,
                            );
                            let _ = adapter
                                .edit_message_card(&chat_id, &msg_id, &success_card)
                                .await;
                        }
                        Err(tee_err) => {
                            tracing::warn!(
                                session = %decision.session_id,
                                "TEE processing failed after authorization: {}",
                                tee_err
                            );
                            let err_card = Self::make_result_card(
                                "red",
                                "\u{26a0}\u{fe0f} 执行失败",
                                "安全执行环境（TEE）当前不可用，无法处理敏感数据。\n为保护您的隐私，该消息未被处理。请联系管理员。",
                            );
                            let _ = adapter
                                .edit_message_card(&chat_id, &msg_id, &err_card)
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
                let msg_id = action.message_id.clone();
                let adapter = adapter.clone();
                tokio::spawn(async move {
                    let cancel_card = Self::make_result_card(
                        "red",
                        "\u{274c} 已取消",
                        "该消息未被处理，您的敏感数据已被保护。",
                    );
                    let _ = adapter
                        .edit_message_card(&chat_id, &msg_id, &cancel_card)
                        .await;
                });
                Ok(None)
            }
            other => {
                tracing::warn!(action = other, "Unknown card action");
                Ok(None)
            }
        }
    }

    /// Build a simple Feishu result card with a colored header and markdown body.
    fn make_result_card(template: &str, title: &str, body: &str) -> serde_json::Value {
        serde_json::json!({
            "config": {
                "wide_screen_mode": true
            },
            "header": {
                "template": template,
                "title": {
                    "tag": "plain_text",
                    "content": title
                }
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": body
                    }
                },
                {
                    "tag": "hr"
                },
                {
                    "tag": "note",
                    "elements": [
                        {
                            "tag": "lark_md",
                            "content": "\u{1f6e1}\u{fe0f} SafeClaw 隐私保护引擎"
                        }
                    ]
                }
            ]
        })
    }

    /// Build a loading card (no buttons, non-interactive) for immediate feedback.
    fn make_loading_card(template: &str, title: &str, body: &str) -> serde_json::Value {
        serde_json::json!({
            "config": {
                "wide_screen_mode": true
            },
            "header": {
                "template": template,
                "title": {
                    "tag": "plain_text",
                    "content": title
                }
            },
            "elements": [
                {
                    "tag": "div",
                    "text": {
                        "tag": "lark_md",
                        "content": body
                    }
                },
                {
                    "tag": "hr"
                },
                {
                    "tag": "note",
                    "elements": [
                        {
                            "tag": "lark_md",
                            "content": "\u{1f6e1}\u{fe0f} SafeClaw 隐私保护引擎"
                        }
                    ]
                }
            ]
        })
    }

    // -----------------------------------------------------------------------
    // Webhook dispatch
    // -----------------------------------------------------------------------

    /// Dispatch a raw webhook payload to the appropriate channel parser.
    pub async fn process_webhook(
        &self,
        channel: &str,
        payload: &str,
    ) -> Result<Option<ProcessedResponse>> {
        match self.parse_webhook(channel, payload).await? {
            WebhookParseResult::Challenge(response) => Ok(Some(response)),
            WebhookParseResult::Message(msg) => Ok(Some(self.process_message(msg).await?)),
            WebhookParseResult::CardAction(_) => {
                // Card actions are handled via handle_card_action with an adapter;
                // the non-streaming process_webhook path cannot dispatch them.
                tracing::debug!("CardAction received in process_webhook (no adapter), ignoring");
                Ok(None)
            }
            WebhookParseResult::Ignored => Ok(None),
        }
    }

    /// Parse a raw webhook payload into a `WebhookParseResult`.
    ///
    /// Handles channel-specific parsing (decrypt, dedup, ACL) without
    /// processing the message. Callers can then choose between
    /// `process_message` or `process_message_streaming`.
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

    /// Parse a Discord `MESSAGE_CREATE` webhook payload.
    ///
    /// Discord sends the full message object directly. Bot messages are ignored
    /// to prevent self-reply loops.
    fn parse_discord_webhook(&self, payload: &str) -> Result<WebhookParseResult> {
        use crate::channels::{DiscordAdapter, DiscordMessage};

        let msg: DiscordMessage = serde_json::from_str(payload)
            .map_err(|e| Error::Channel(format!("Failed to parse Discord payload: {}", e)))?;

        // Ignore messages from bots (including self)
        if msg.author.bot.unwrap_or(false) {
            return Ok(WebhookParseResult::Ignored);
        }

        // Ignore empty messages
        if msg.content.trim().is_empty() {
            return Ok(WebhookParseResult::Ignored);
        }

        let inbound = DiscordAdapter::parse_message(&msg)?;
        Ok(WebhookParseResult::Message(inbound))
    }

    /// Handle a channel event from the event loop.
    ///
    /// Processes `ChannelEvent::Message` through the full pipeline and
    /// dispatches the response back via the `channels` map.  Other event
    /// variants are logged and discarded.
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

    /// Parse a Feishu v2 webhook, including challenge response and dedup.
    async fn parse_feishu_webhook(&self, payload: &str) -> Result<WebhookParseResult> {
        // If encrypt_key is configured, the body may be {"encrypt": "base64..."}
        // Decrypt it first before any other processing.
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

        // Signature verification: done at HTTP layer via X-Lark-Signature header.

        // Case 2: Event callback v2.0
        let event_type = parsed
            .pointer("/header/event_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        // Card action callback (card.action.trigger event)
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
                tracing::debug!("Ignoring card action with missing action or message_id");
                return Ok(WebhookParseResult::Ignored);
            }

            tracing::info!(
                action = %action_str,
                user = %user_id,
                message_id = %message_id,
                "Received Feishu card action callback"
            );

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
            tracing::debug!(event_type, "Ignoring non-message Feishu event");
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
                tracing::debug!(message_id, "Ignoring duplicate Feishu message");
                return Ok(WebhookParseResult::Ignored);
            }
            if seen.len() >= 256 {
                seen.pop_front();
            }
            seen.push_back(message_id.to_string());
        }

        if msg_type != "text" {
            tracing::debug!(msg_type, "Ignoring non-text Feishu message");
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

        tracing::info!(
            sender = sender_open_id,
            chat_id,
            chat_type,
            "Received Feishu message"
        );

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

    /// Parse a generic channel webhook payload into an `InboundMessage`.
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

/// Decrypt a Feishu encrypted callback payload.
///
/// Algorithm (matches all official Feishu SDKs):
/// 1. key = SHA-256(encrypt_key)
/// 2. raw = base64_decode(encrypt_field)
/// 3. iv  = raw[0..16], ciphertext = raw[16..]
/// 4. plaintext = AES-256-CBC-decrypt(key, iv, ciphertext)  [PKCS7 padding]
fn feishu_decrypt(encrypt: &str, encrypt_key: &str) -> Result<String> {
    use aes::cipher::{block_padding::Pkcs7, BlockDecryptMut, KeyIvInit};
    use base64::Engine;
    use sha2::{Digest, Sha256};

    // 1. Derive 32-byte AES key from encrypt_key via SHA-256
    let key: [u8; 32] = Sha256::digest(encrypt_key.as_bytes()).into();

    // 2. Base64-decode the encrypted field
    let raw = base64::engine::general_purpose::STANDARD
        .decode(encrypt)
        .map_err(|e| Error::Channel(format!("Feishu decrypt: invalid base64: {}", e)))?;

    if raw.len() < 16 {
        return Err(Error::Channel(
            "Feishu decrypt: encrypted payload too short".to_string(),
        ));
    }

    // 3. Split IV (first 16 bytes) and ciphertext
    let (iv, ciphertext) = raw.split_at(16);
    let iv: [u8; 16] = iv.try_into().expect("iv is 16 bytes");

    // 4. AES-256-CBC decrypt with PKCS7 unpadding
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
    use crate::audit::AuditEventBus;
    use crate::config::SafeClawConfig;
    use crate::privacy::{
        Classifier, CompositeClassifier, PolicyEngine, RegexBackend, SemanticAnalyzer,
        SemanticBackend,
    };
    use crate::session::{SessionManager, SessionRouter};
    use std::sync::Arc;
    use tokio::sync::RwLock;

    fn make_processor() -> MessageProcessor {
        let config = SafeClawConfig::default();
        let audit_log = Arc::new(RwLock::new(crate::audit::AuditLog::new(100)));
        let audit_bus = Arc::new(AuditEventBus::new(100, audit_log));
        let session_manager = Arc::new(SessionManager::new(config.tee.clone(), audit_bus));

        let classifier = Arc::new(
            Classifier::new(config.privacy.rules.clone(), config.privacy.default_level).unwrap(),
        );
        let regex =
            RegexBackend::new(config.privacy.rules.clone(), config.privacy.default_level).unwrap();
        let semantic = SemanticBackend::new(SemanticAnalyzer::new());
        let composite = Arc::new(CompositeClassifier::new(vec![
            Box::new(regex),
            Box::new(semantic),
        ]));
        let policy_engine = Arc::new(PolicyEngine::new());

        let session_router = Arc::new(SessionRouter::new(
            session_manager.clone(),
            classifier,
            composite,
            policy_engine,
        ));
        MessageProcessor::new(session_router, session_manager, None)
    }

    #[test]
    fn test_parse_generic_webhook_empty_content() {
        let p = make_processor();
        let payload = r#"{"user_id": "u1"}"#;
        let result = p.parse_generic_webhook("slack", payload).unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_parse_discord_webhook_message() {
        let p = make_processor();
        let payload = r#"{
            "id": "msg123",
            "channel_id": "chan456",
            "guild_id": "guild789",
            "content": "hello from discord",
            "timestamp": "2024-01-01T12:00:00+00:00",
            "author": {"id": "user111", "username": "alice", "bot": false},
            "mentions": []
        }"#;
        let result = p.parse_discord_webhook(payload).unwrap();
        match result {
            WebhookParseResult::Message(msg) => {
                assert_eq!(msg.channel, "discord");
                assert_eq!(msg.sender_id, "user111");
                assert_eq!(msg.chat_id, "chan456");
                assert_eq!(msg.content, "hello from discord");
            }
            _ => panic!("Expected Message, got {:?}", result),
        }
    }

    #[test]
    fn test_parse_discord_webhook_ignores_bot() {
        let p = make_processor();
        let payload = r#"{
            "id": "msg123",
            "channel_id": "chan456",
            "content": "bot message",
            "timestamp": "2024-01-01T12:00:00+00:00",
            "author": {"id": "bot999", "username": "mybot", "bot": true},
            "mentions": []
        }"#;
        let result = p.parse_discord_webhook(payload).unwrap();
        assert!(matches!(result, WebhookParseResult::Ignored));
    }

    #[test]
    fn test_parse_discord_webhook_ignores_empty_content() {
        let p = make_processor();
        let payload = r#"{
            "id": "msg123",
            "channel_id": "chan456",
            "content": "   ",
            "timestamp": "2024-01-01T12:00:00+00:00",
            "author": {"id": "user111", "username": "alice", "bot": false},
            "mentions": []
        }"#;
        let result = p.parse_discord_webhook(payload).unwrap();
        assert!(matches!(result, WebhookParseResult::Ignored));
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

    #[test]
    fn test_parse_generic_webhook_fallback_fields() {
        let p = make_processor();
        let payload = r#"{"from": "u2", "text": "world"}"#;
        let msg = p
            .parse_generic_webhook("telegram", payload)
            .unwrap()
            .unwrap();
        assert_eq!(msg.sender_id, "u2");
        assert_eq!(msg.content, "world");
    }

    #[tokio::test]
    async fn test_process_webhook_unknown_channel() {
        let p = make_processor();
        let result = p.process_webhook("unknown", "{}").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("Unknown channel"));
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

    #[tokio::test]
    async fn test_feishu_dedup() {
        let p = make_processor();
        let payload = |msg_id: &str, text: &str| {
            format!(
                r#"{{
                    "schema": "2.0",
                    "header": {{"event_type": "im.message.receive_v1"}},
                    "event": {{
                        "sender": {{"sender_id": {{"open_id": "ou_abc"}}}},
                        "message": {{
                            "message_id": "{}",
                            "chat_id": "c1",
                            "chat_type": "p2p",
                            "message_type": "text",
                            "content": "{{\"text\": \"{}\"}}"
                        }}
                    }}
                }}"#,
                msg_id, text
            )
        };

        // First delivery: parsed into a message
        let r1 = p
            .parse_webhook("feishu", &payload("msg_001", "hello"))
            .await
            .unwrap();
        assert!(matches!(r1, WebhookParseResult::Message(_)));

        // Second delivery with same message_id: deduped
        let r2 = p
            .parse_webhook("feishu", &payload("msg_001", "hello"))
            .await
            .unwrap();
        assert!(matches!(r2, WebhookParseResult::Ignored));
    }

    #[tokio::test]
    async fn test_feishu_card_action_parsed() {
        let p = make_processor();
        // Feishu card callbacks use event subscription v2.0 format
        let payload = r#"{
            "schema": "2.0",
            "header": {"event_type": "card.action.trigger", "app_id": "cli_test"},
            "event": {
                "operator": {"open_id": "ou_user1"},
                "action": {
                    "value": {"action": "authorize"},
                    "tag": "button"
                },
                "context": {
                    "open_message_id": "om_card_001",
                    "open_chat_id": "oc_chat_001"
                }
            }
        }"#;

        let result = p.parse_webhook("feishu", payload).await.unwrap();
        match result {
            WebhookParseResult::CardAction(action) => {
                assert_eq!(action.action, "authorize");
                assert_eq!(action.user_id, "ou_user1");
                assert_eq!(action.message_id, "om_card_001");
                assert_eq!(action.chat_id, "oc_chat_001");
                assert_eq!(action.channel, "feishu");
            }
            _ => panic!("Expected CardAction"),
        }
    }

    #[tokio::test]
    async fn test_feishu_card_action_cancel_parsed() {
        let p = make_processor();
        let payload = r#"{
            "schema": "2.0",
            "header": {"event_type": "card.action.trigger", "app_id": "cli_test"},
            "event": {
                "operator": {"open_id": "ou_user2"},
                "action": {
                    "value": {"action": "cancel"},
                    "tag": "button"
                },
                "context": {
                    "open_message_id": "om_card_002",
                    "open_chat_id": "oc_chat_002"
                }
            }
        }"#;

        let result = p.parse_webhook("feishu", payload).await.unwrap();
        match result {
            WebhookParseResult::CardAction(action) => {
                assert_eq!(action.action, "cancel");
                assert_eq!(action.user_id, "ou_user2");
                assert_eq!(action.message_id, "om_card_002");
            }
            _ => panic!("Expected CardAction"),
        }
    }

    #[tokio::test]
    async fn test_feishu_card_action_missing_action_ignored() {
        let p = make_processor();
        let payload = r#"{
            "schema": "2.0",
            "header": {"event_type": "card.action.trigger", "app_id": "cli_test"},
            "event": {
                "operator": {"open_id": "ou_user1"},
                "action": {
                    "value": {},
                    "tag": "button"
                },
                "context": {
                    "open_message_id": "om_card_003",
                    "open_chat_id": "oc_chat_003"
                }
            }
        }"#;

        let result = p.parse_webhook("feishu", payload).await.unwrap();
        assert!(matches!(result, WebhookParseResult::Ignored));
    }

    #[tokio::test]
    async fn test_pending_sensitive_store() {
        let p = make_processor();
        let msg = InboundMessage::new("feishu", "ou_user1", "oc_chat1", "my ssn is 123-45-6789");
        let pending = PendingSensitiveMessage {
            message: msg,
            card_message_id: "om_card_100".to_string(),
            sensitivity: "HighlySensitive".to_string(),
            created_at: std::time::Instant::now(),
        };

        p.pending_sensitive
            .write()
            .await
            .insert("om_card_100".to_string(), pending);

        assert!(p.pending_sensitive.read().await.contains_key("om_card_100"));

        // Remove it
        let removed = p.pending_sensitive.write().await.remove("om_card_100");
        assert!(removed.is_some());
        assert!(!p.pending_sensitive.read().await.contains_key("om_card_100"));
    }

    #[tokio::test]
    async fn test_card_action_via_process_webhook_returns_none() {
        let p = make_processor();
        let payload = r#"{
            "schema": "2.0",
            "header": {"event_type": "card.action.trigger", "app_id": "cli_test"},
            "event": {
                "operator": {"open_id": "ou_user1"},
                "action": {
                    "value": {"action": "authorize"},
                    "tag": "button"
                },
                "context": {
                    "open_message_id": "om_card_010",
                    "open_chat_id": "oc_chat_010"
                }
            }
        }"#;

        // process_webhook (non-streaming) should return None for card actions
        let result = p.process_webhook("feishu", payload).await.unwrap();
        assert!(result.is_none());
    }
}
