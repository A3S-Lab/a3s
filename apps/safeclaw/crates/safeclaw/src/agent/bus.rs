//! Agent-to-Agent communication via event bus
//!
//! `AgentBus` connects `AgentEngine` to `a3s_event::EventBus`, enabling
//! sessions to send and receive messages across the event bus.
//!
//! ## Subject convention
//!
//! - Broadcast: `events.agent.broadcast.<topic>` — all subscribed sessions receive it
//! - Mention:   `events.agent.mention.<session_id>` — point-to-point
//!
//! ## Execution modes
//!
//! - `auto` (opt-in): incoming message is fed directly into `generate_response`
//! - `confirm` (default): browser receives `AgentMessage` notification; user approves

use crate::agent::engine::AgentEngine;
use crate::agent::types::{AgentMessageType, BrowserIncomingMessage};
use a3s_event::EventBus;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

// =============================================================================
// Payload type
// =============================================================================

/// Payload carried in agent-to-agent event bus messages
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessagePayload {
    /// Unique message ID
    pub message_id: String,
    pub from_session_id: String,
    pub topic: String,
    pub content: String,
    /// Message type: chat, task_request, task_response
    #[serde(default)]
    pub message_type: AgentMessageType,
    /// ID of the message this is replying to (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reply_to: Option<String>,
    /// Unix timestamp (seconds)
    pub timestamp: u64,
}

// =============================================================================
// AgentBus
// =============================================================================

/// Connects `AgentEngine` to the event bus for agent-to-agent messaging.
pub struct AgentBus {
    engine: Arc<AgentEngine>,
    event_bus: Arc<EventBus>,
    /// Per-session auto-execute flag (default: false = confirm mode)
    auto_execute: Arc<RwLock<HashMap<String, bool>>>,
    /// Per-session rate limiter: (count, window_start_secs)
    rate_limits: Arc<RwLock<HashMap<String, (u32, u64)>>>,
}

/// Maximum auto-execute messages per session per minute
const RATE_LIMIT_PER_MINUTE: u32 = 30;
/// Maximum reconnection backoff in seconds
const MAX_RECONNECT_BACKOFF_SECS: u64 = 30;

impl AgentBus {
    /// Create a new `AgentBus`.
    pub fn new(engine: Arc<AgentEngine>, event_bus: Arc<EventBus>) -> Self {
        Self {
            engine,
            event_bus,
            auto_execute: Arc::new(RwLock::new(HashMap::new())),
            rate_limits: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Set auto-execute mode for a session.
    pub async fn set_auto_execute(&self, session_id: &str, enabled: bool) {
        self.auto_execute
            .write()
            .await
            .insert(session_id.to_string(), enabled);
    }

    /// Get auto-execute mode for a session (default: false).
    pub async fn get_auto_execute(&self, session_id: &str) -> bool {
        *self
            .auto_execute
            .read()
            .await
            .get(session_id)
            .unwrap_or(&false)
    }

    /// Publish a message to another agent via the event bus.
    ///
    /// `target` format:
    /// - `"broadcast:<topic>"` → publishes to `agent.broadcast.<topic>`
    /// - `"mention:<session_id>"` → publishes to `agent.mention.<session_id>`
    pub async fn publish(
        &self,
        from_session_id: &str,
        target: &str,
        content: &str,
    ) -> crate::Result<()> {
        let (category_topic, topic_label) = if let Some(topic) = target.strip_prefix("broadcast:") {
            (format!("broadcast.{}", topic), topic.to_string())
        } else if let Some(sid) = target.strip_prefix("mention:") {
            (format!("mention.{}", sid), sid.to_string())
        } else {
            return Err(crate::Error::Runtime(format!(
                "Invalid agent message target '{}': expected 'broadcast:<topic>' or 'mention:<session_id>'",
                target
            )));
        };

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let payload = AgentMessagePayload {
            message_id: uuid::Uuid::new_v4().to_string(),
            from_session_id: from_session_id.to_string(),
            topic: topic_label,
            content: content.to_string(),
            message_type: AgentMessageType::default(),
            reply_to: None,
            timestamp: now,
        };

        self.event_bus
            .publish(
                "agent",
                &category_topic,
                "agent message",
                from_session_id,
                serde_json::to_value(&payload).unwrap_or_default(),
            )
            .await
            .map_err(|e| {
                crate::Error::Runtime(format!("Failed to publish agent message: {}", e))
            })?;

        Ok(())
    }

    /// Start the subscription loops.
    ///
    /// Spawns two tasks:
    /// 1. Subscribe `agent.broadcast.*` — delivers to all active sessions
    /// 2. Subscribe `agent.mention.*`   — delivers to the specific session
    pub fn start(self: Arc<Self>) {
        let bus = self.clone();
        tokio::spawn(async move {
            bus.run_broadcast_loop().await;
        });

        let bus = self.clone();
        tokio::spawn(async move {
            bus.run_mention_loop().await;
        });
    }

    // =========================================================================
    // Subscription loops (with auto-reconnect)
    // =========================================================================

    async fn run_broadcast_loop(&self) {
        let mut attempts = 0u32;
        loop {
            let broadcast_subject = self
                .event_bus
                .provider_arc()
                .build_subject("agent", "broadcast.>");

            let mut sub = match self
                .event_bus
                .provider_arc()
                .subscribe(&broadcast_subject)
                .await
            {
                Ok(s) => {
                    attempts = 0;
                    tracing::info!(subject = %broadcast_subject, "AgentBus: broadcast subscription started");
                    s
                }
                Err(e) => {
                    tracing::error!("AgentBus: failed to subscribe to broadcast: {}", e);
                    let delay = std::cmp::min(1u64 << attempts, MAX_RECONNECT_BACKOFF_SECS);
                    attempts = attempts.saturating_add(1);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    continue;
                }
            };

            loop {
                match sub.next().await {
                    Ok(Some(received)) => {
                        if let Ok(payload) = serde_json::from_value::<AgentMessagePayload>(
                            received.event.payload.clone(),
                        ) {
                            self.deliver_to_all_sessions(&payload).await;
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("AgentBus: broadcast subscription closed, reconnecting...");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("AgentBus: broadcast receive error: {}", e);
                    }
                }
            }

            // Reconnect with backoff
            let delay = std::cmp::min(1u64 << attempts, MAX_RECONNECT_BACKOFF_SECS);
            attempts = attempts.saturating_add(1);
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    }

    async fn run_mention_loop(&self) {
        let mut attempts = 0u32;
        loop {
            let mention_subject = self
                .event_bus
                .provider_arc()
                .build_subject("agent", "mention.>");

            let mut sub = match self
                .event_bus
                .provider_arc()
                .subscribe(&mention_subject)
                .await
            {
                Ok(s) => {
                    attempts = 0;
                    tracing::info!(subject = %mention_subject, "AgentBus: mention subscription started");
                    s
                }
                Err(e) => {
                    tracing::error!("AgentBus: failed to subscribe to mentions: {}", e);
                    let delay = std::cmp::min(1u64 << attempts, MAX_RECONNECT_BACKOFF_SECS);
                    attempts = attempts.saturating_add(1);
                    tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
                    continue;
                }
            };

            loop {
                match sub.next().await {
                    Ok(Some(received)) => {
                        if let Ok(payload) = serde_json::from_value::<AgentMessagePayload>(
                            received.event.payload.clone(),
                        ) {
                            // Extract target session_id from subject: events.agent.mention.<session_id>
                            let target_session_id = received
                                .event
                                .subject
                                .split('.')
                                .last()
                                .unwrap_or("")
                                .to_string();

                            if !target_session_id.is_empty() {
                                self.deliver_to_session(&target_session_id, &payload).await;
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::warn!("AgentBus: mention subscription closed, reconnecting...");
                        break;
                    }
                    Err(e) => {
                        tracing::warn!("AgentBus: mention receive error: {}", e);
                    }
                }
            }

            // Reconnect with backoff
            let delay = std::cmp::min(1u64 << attempts, MAX_RECONNECT_BACKOFF_SECS);
            attempts = attempts.saturating_add(1);
            tokio::time::sleep(std::time::Duration::from_secs(delay)).await;
        }
    }

    // =========================================================================
    // Delivery
    // =========================================================================

    /// Deliver a broadcast message to all active sessions.
    async fn deliver_to_all_sessions(&self, payload: &AgentMessagePayload) {
        let sessions = self.engine.list_sessions().await;
        for session in sessions {
            // Don't deliver to the sender
            if session.session_id == payload.from_session_id {
                continue;
            }
            self.deliver_to_session(&session.session_id, payload).await;
        }
    }

    /// Deliver a message to a specific session.
    async fn deliver_to_session(&self, session_id: &str, payload: &AgentMessagePayload) {
        let auto = self.get_auto_execute(session_id).await;
        let message_id = payload.message_id.clone();

        if auto {
            // Rate limit check for auto-execute
            if !self.check_rate_limit(session_id).await {
                tracing::warn!(
                    session_id,
                    from = %payload.from_session_id,
                    "AgentBus: auto-execute rate limited, falling back to confirm mode"
                );
                // Fall through to confirm mode
                let msg = BrowserIncomingMessage::AgentMessage {
                    message_id,
                    from_session_id: payload.from_session_id.clone(),
                    topic: payload.topic.clone(),
                    content: payload.content.clone(),
                    auto_execute: false,
                };
                self.engine.broadcast_to_session(session_id, &msg).await;
                return;
            }

            // Auto mode: feed directly into agent generation
            tracing::debug!(
                session_id,
                from = %payload.from_session_id,
                "AgentBus: auto-executing incoming message"
            );
            if let Err(e) = self
                .engine
                .generate_response(session_id, &payload.content)
                .await
            {
                tracing::warn!(session_id, "AgentBus: auto-execute failed: {}", e);
            }
        } else {
            // Confirm mode: notify browser, let user approve
            let msg = BrowserIncomingMessage::AgentMessage {
                message_id,
                from_session_id: payload.from_session_id.clone(),
                topic: payload.topic.clone(),
                content: payload.content.clone(),
                auto_execute: false,
            };
            self.engine.broadcast_to_session(session_id, &msg).await;
        }
    }

    /// Check and update rate limit for auto-execute on a session.
    /// Returns `true` if the message is allowed, `false` if rate limited.
    async fn check_rate_limit(&self, session_id: &str) -> bool {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut limits = self.rate_limits.write().await;
        let entry = limits.entry(session_id.to_string()).or_insert((0, now));

        // Reset window if more than 60 seconds have passed
        if now.saturating_sub(entry.1) >= 60 {
            entry.0 = 0;
            entry.1 = now;
        }

        if entry.0 >= RATE_LIMIT_PER_MINUTE {
            return false;
        }

        entry.0 += 1;
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::session_store::AgentSessionStore;
    use a3s_code::config::CodeConfig;
    use a3s_event::MemoryProvider;
    use tempfile::TempDir;

    async fn make_engine(dir: &std::path::Path) -> Arc<AgentEngine> {
        let code_config = CodeConfig {
            sessions_dir: Some(dir.to_path_buf()),
            ..Default::default()
        };
        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .to_string_lossy()
            .to_string();
        let tool_executor = Arc::new(a3s_code::tools::ToolExecutor::new(cwd));
        let session_manager = Arc::new(
            a3s_code::session::SessionManager::with_persistence(None, tool_executor, dir)
                .await
                .unwrap(),
        );
        let store = Arc::new(AgentSessionStore::new(dir.join("ui-state")));
        Arc::new(
            AgentEngine::new(session_manager, code_config, store)
                .await
                .unwrap(),
        )
    }

    #[tokio::test]
    async fn test_publish_broadcast_invalid_target() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(dir.path()).await;
        let event_bus = Arc::new(EventBus::new(MemoryProvider::default()));
        let bus = Arc::new(AgentBus::new(engine, event_bus));

        let result = bus.publish("session-1", "invalid-target", "hello").await;
        assert!(result.is_err());
        assert!(result
            .unwrap_err()
            .to_string()
            .contains("Invalid agent message target"));
    }

    #[tokio::test]
    async fn test_publish_broadcast_valid() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(dir.path()).await;
        let event_bus = Arc::new(EventBus::new(MemoryProvider::default()));
        let bus = Arc::new(AgentBus::new(engine, event_bus.clone()));

        let result = bus
            .publish("session-1", "broadcast:code-review", "please review")
            .await;
        assert!(result.is_ok());

        // Verify event was published
        let counts = event_bus.counts(10).await.unwrap();
        assert_eq!(counts.total, 1);
    }

    #[tokio::test]
    async fn test_publish_mention_valid() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(dir.path()).await;
        let event_bus = Arc::new(EventBus::new(MemoryProvider::default()));
        let bus = Arc::new(AgentBus::new(engine, event_bus.clone()));

        let result = bus
            .publish("session-1", "mention:session-2", "hey you")
            .await;
        assert!(result.is_ok());

        let counts = event_bus.counts(10).await.unwrap();
        assert_eq!(counts.total, 1);
    }

    #[tokio::test]
    async fn test_auto_execute_default_false() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(dir.path()).await;
        let event_bus = Arc::new(EventBus::new(MemoryProvider::default()));
        let bus = AgentBus::new(engine, event_bus);

        assert!(!bus.get_auto_execute("any-session").await);
    }

    #[tokio::test]
    async fn test_auto_execute_toggle() {
        let dir = TempDir::new().unwrap();
        let engine = make_engine(dir.path()).await;
        let event_bus = Arc::new(EventBus::new(MemoryProvider::default()));
        let bus = AgentBus::new(engine, event_bus);

        bus.set_auto_execute("s1", true).await;
        assert!(bus.get_auto_execute("s1").await);

        bus.set_auto_execute("s1", false).await;
        assert!(!bus.get_auto_execute("s1").await);
    }
}
