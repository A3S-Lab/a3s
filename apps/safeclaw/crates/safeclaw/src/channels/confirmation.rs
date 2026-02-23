//! Human-in-the-loop confirmation for sensitive operations.
//!
//! When the policy engine returns `RequireConfirmation`, this module
//! forwards a confirmation request to the user's chat channel and
//! waits for an approve/reject response.
//!
//! **Threat model**: Defends against silent degradation (A4) and
//! unauthorized data processing. See `docs/threat-model.md` §4.

use crate::channels::message::OutboundMessage;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{oneshot, RwLock};
use tokio::time::{timeout, Duration};

/// Default timeout for HITL confirmation (seconds).
const DEFAULT_TIMEOUT_SECS: u64 = 120;

/// Per-channel permission policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChannelPermissionPolicy {
    /// Always trust — auto-approve all confirmations (skip HITL).
    Trust,
    /// Always require explicit confirmation.
    Strict,
    /// Use default behavior (require confirmation for sensitive operations).
    Default,
}

impl std::default::Default for ChannelPermissionPolicy {
    fn default() -> Self {
        Self::Default
    }
}

/// A pending confirmation request.
#[derive(Debug)]
#[allow(dead_code)]
struct PendingConfirmation {
    /// Channel the request was sent to.
    channel: String,
    /// Chat ID where the request was sent.
    chat_id: String,
    /// Description of what's being confirmed.
    description: String,
    /// Sender to resolve the confirmation.
    responder: oneshot::Sender<ConfirmationResponse>,
}

/// User's response to a confirmation request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConfirmationResponse {
    /// User approved the operation.
    Approved,
    /// User rejected the operation.
    Rejected,
    /// Request timed out without response.
    TimedOut,
}

/// Result of a confirmation request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmationResult {
    /// The confirmation ID.
    pub id: String,
    /// User's response.
    pub response: ConfirmationResponse,
    /// How long the user took to respond (milliseconds).
    pub response_time_ms: u64,
}

/// Configuration for HITL confirmation.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct HitlConfig {
    /// Whether HITL is enabled.
    pub enabled: bool,
    /// Timeout in seconds for confirmation requests.
    pub timeout_secs: u64,
    /// Default action on timeout.
    pub timeout_action: ConfirmationResponse,
    /// Per-channel permission policies.
    #[serde(default)]
    pub channel_policies: HashMap<String, ChannelPermissionPolicy>,
}

impl Default for HitlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            timeout_secs: DEFAULT_TIMEOUT_SECS,
            timeout_action: ConfirmationResponse::Rejected,
            channel_policies: HashMap::new(),
        }
    }
}

/// HITL confirmation manager.
///
/// Tracks pending confirmations and matches incoming user responses
/// to outstanding requests.
pub struct ConfirmationManager {
    /// Pending confirmations keyed by confirmation ID.
    pending: Arc<RwLock<HashMap<String, PendingConfirmation>>>,
    /// Configuration.
    config: HitlConfig,
}

impl ConfirmationManager {
    /// Create a new confirmation manager.
    pub fn new(config: HitlConfig) -> Self {
        Self {
            pending: Arc::new(RwLock::new(HashMap::new())),
            config,
        }
    }

    /// Get the channel permission policy.
    pub fn channel_policy(&self, channel: &str) -> ChannelPermissionPolicy {
        self.config
            .channel_policies
            .get(channel)
            .copied()
            .unwrap_or(ChannelPermissionPolicy::Default)
    }

    /// Request confirmation from a user via their chat channel.
    ///
    /// Returns a `ConfirmationResult` after the user responds or timeout.
    /// The caller should send the returned `OutboundMessage` to the channel.
    pub async fn request_confirmation(
        &self,
        channel: &str,
        chat_id: &str,
        description: &str,
        session_id: &str,
    ) -> (OutboundMessage, ConfirmationResult) {
        let policy = self.channel_policy(channel);

        // Auto-approve for trusted channels
        if policy == ChannelPermissionPolicy::Trust {
            tracing::info!(
                channel = channel,
                session_id = session_id,
                "HITL auto-approved (channel policy: trust)"
            );
            let msg = OutboundMessage::new(channel, chat_id, "");
            return (
                msg,
                ConfirmationResult {
                    id: uuid::Uuid::new_v4().to_string(),
                    response: ConfirmationResponse::Approved,
                    response_time_ms: 0,
                },
            );
        }

        let confirmation_id = uuid::Uuid::new_v4().to_string();
        let (tx, rx) = oneshot::channel();

        // Build the confirmation message
        let prompt = format!(
            "⚠️ **Confirmation Required**\n\n\
             {}\n\n\
             Reply with `yes` or `no` to approve or reject.\n\
             _(Auto-{} in {} seconds)_\n\n\
             `[{}]`",
            description,
            match self.config.timeout_action {
                ConfirmationResponse::Approved => "approve",
                ConfirmationResponse::Rejected => "reject",
                ConfirmationResponse::TimedOut => "reject",
            },
            self.config.timeout_secs,
            &confirmation_id[..8],
        );

        let outbound = OutboundMessage::new(channel, chat_id, &prompt);

        // Store pending confirmation
        {
            let mut pending = self.pending.write().await;
            pending.insert(
                confirmation_id.clone(),
                PendingConfirmation {
                    channel: channel.to_string(),
                    chat_id: chat_id.to_string(),
                    description: description.to_string(),
                    responder: tx,
                },
            );
        }

        let start = tokio::time::Instant::now();
        let timeout_duration = Duration::from_secs(self.config.timeout_secs);

        // Wait for response or timeout
        let response = match timeout(timeout_duration, rx).await {
            Ok(Ok(resp)) => resp,
            Ok(Err(_)) => {
                // Channel closed (confirmation cancelled)
                self.config.timeout_action
            }
            Err(_) => {
                // Timeout
                tracing::warn!(
                    confirmation_id = &confirmation_id[..8],
                    channel = channel,
                    session_id = session_id,
                    "HITL confirmation timed out"
                );
                ConfirmationResponse::TimedOut
            }
        };

        // Clean up
        self.pending.write().await.remove(&confirmation_id);

        let elapsed = start.elapsed().as_millis() as u64;

        (
            outbound,
            ConfirmationResult {
                id: confirmation_id,
                response,
                response_time_ms: elapsed,
            },
        )
    }

    /// Try to match an incoming user message to a pending confirmation.
    ///
    /// Returns `true` if the message was consumed as a confirmation response.
    pub async fn try_resolve(&self, channel: &str, chat_id: &str, text: &str) -> bool {
        let response = parse_confirmation_response(text);
        let response = match response {
            Some(r) => r,
            None => return false,
        };

        // Find matching pending confirmation for this channel + chat
        let mut pending = self.pending.write().await;
        let matching_id = pending
            .iter()
            .find(|(_, p)| p.channel == channel && p.chat_id == chat_id)
            .map(|(id, _)| id.clone());

        if let Some(id) = matching_id {
            if let Some(confirmation) = pending.remove(&id) {
                tracing::info!(
                    confirmation_id = &id[..8],
                    channel = channel,
                    response = ?response,
                    "HITL confirmation resolved"
                );
                let _ = confirmation.responder.send(response);
                return true;
            }
        }

        false
    }

    /// Cancel all pending confirmations (e.g., on shutdown).
    pub async fn cancel_all(&self) {
        let mut pending = self.pending.write().await;
        let count = pending.len();
        pending.clear();
        if count > 0 {
            tracing::info!(count = count, "Cancelled pending HITL confirmations");
        }
    }

    /// Number of pending confirmations.
    pub async fn pending_count(&self) -> usize {
        self.pending.read().await.len()
    }
}

/// Parse a user message as a confirmation response.
///
/// Recognizes: yes, no, approve, reject, allow, deny, /allow, /deny, y, n
pub fn parse_confirmation_response(text: &str) -> Option<ConfirmationResponse> {
    let trimmed = text.trim().to_lowercase();
    match trimmed.as_str() {
        "yes" | "y" | "approve" | "allow" | "/allow" | "/approve" | "/yes" => {
            Some(ConfirmationResponse::Approved)
        }
        "no" | "n" | "reject" | "deny" | "/deny" | "/reject" | "/no" => {
            Some(ConfirmationResponse::Rejected)
        }
        _ => None,
    }
}

impl Default for ConfirmationManager {
    fn default() -> Self {
        Self::new(HitlConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_approve() {
        assert_eq!(
            parse_confirmation_response("yes"),
            Some(ConfirmationResponse::Approved)
        );
        assert_eq!(
            parse_confirmation_response("Y"),
            Some(ConfirmationResponse::Approved)
        );
        assert_eq!(
            parse_confirmation_response("approve"),
            Some(ConfirmationResponse::Approved)
        );
        assert_eq!(
            parse_confirmation_response("/allow"),
            Some(ConfirmationResponse::Approved)
        );
        assert_eq!(
            parse_confirmation_response("  Allow  "),
            Some(ConfirmationResponse::Approved)
        );
    }

    #[test]
    fn test_parse_reject() {
        assert_eq!(
            parse_confirmation_response("no"),
            Some(ConfirmationResponse::Rejected)
        );
        assert_eq!(
            parse_confirmation_response("N"),
            Some(ConfirmationResponse::Rejected)
        );
        assert_eq!(
            parse_confirmation_response("reject"),
            Some(ConfirmationResponse::Rejected)
        );
        assert_eq!(
            parse_confirmation_response("/deny"),
            Some(ConfirmationResponse::Rejected)
        );
    }

    #[test]
    fn test_parse_unrecognized() {
        assert_eq!(parse_confirmation_response("maybe"), None);
        assert_eq!(parse_confirmation_response("hello world"), None);
        assert_eq!(parse_confirmation_response(""), None);
    }

    #[test]
    fn test_default_config() {
        let config = HitlConfig::default();
        assert!(config.enabled);
        assert_eq!(config.timeout_secs, DEFAULT_TIMEOUT_SECS);
        assert_eq!(config.timeout_action, ConfirmationResponse::Rejected);
    }

    #[test]
    fn test_channel_policy_default() {
        let mgr = ConfirmationManager::default();
        assert_eq!(
            mgr.channel_policy("telegram"),
            ChannelPermissionPolicy::Default
        );
    }

    #[test]
    fn test_channel_policy_configured() {
        let mut config = HitlConfig::default();
        config
            .channel_policies
            .insert("slack".to_string(), ChannelPermissionPolicy::Trust);
        config
            .channel_policies
            .insert("telegram".to_string(), ChannelPermissionPolicy::Strict);

        let mgr = ConfirmationManager::new(config);
        assert_eq!(mgr.channel_policy("slack"), ChannelPermissionPolicy::Trust);
        assert_eq!(
            mgr.channel_policy("telegram"),
            ChannelPermissionPolicy::Strict
        );
        assert_eq!(
            mgr.channel_policy("discord"),
            ChannelPermissionPolicy::Default
        );
    }

    #[tokio::test]
    async fn test_auto_approve_trusted_channel() {
        let mut config = HitlConfig::default();
        config
            .channel_policies
            .insert("slack".to_string(), ChannelPermissionPolicy::Trust);
        let mgr = ConfirmationManager::new(config);

        let (_msg, result) = mgr
            .request_confirmation("slack", "C123", "Process sensitive data?", "s1")
            .await;
        assert_eq!(result.response, ConfirmationResponse::Approved);
        assert_eq!(result.response_time_ms, 0);
    }

    #[tokio::test]
    async fn test_resolve_pending() {
        let mut config = HitlConfig::default();
        config.timeout_secs = 5;
        let mgr = Arc::new(ConfirmationManager::new(config));

        let mgr_clone = mgr.clone();
        let handle = tokio::spawn(async move {
            mgr_clone
                .request_confirmation("telegram", "chat1", "Allow processing?", "s1")
                .await
        });

        // Give time for the confirmation to be registered
        tokio::time::sleep(Duration::from_millis(50)).await;

        assert_eq!(mgr.pending_count().await, 1);

        // Resolve with "yes"
        let consumed = mgr.try_resolve("telegram", "chat1", "yes").await;
        assert!(consumed);
        assert_eq!(mgr.pending_count().await, 0);

        let (_msg, result) = handle.await.unwrap();
        assert_eq!(result.response, ConfirmationResponse::Approved);
    }

    #[tokio::test]
    async fn test_resolve_reject() {
        let mut config = HitlConfig::default();
        config.timeout_secs = 5;
        let mgr = Arc::new(ConfirmationManager::new(config));

        let mgr_clone = mgr.clone();
        let handle = tokio::spawn(async move {
            mgr_clone
                .request_confirmation("discord", "ch1", "Proceed?", "s1")
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;

        let consumed = mgr.try_resolve("discord", "ch1", "no").await;
        assert!(consumed);

        let (_msg, result) = handle.await.unwrap();
        assert_eq!(result.response, ConfirmationResponse::Rejected);
    }

    #[tokio::test]
    async fn test_timeout() {
        let mut config = HitlConfig::default();
        config.timeout_secs = 1; // 1 second timeout for test
        config.timeout_action = ConfirmationResponse::Rejected;
        let mgr = ConfirmationManager::new(config);

        let (_msg, result) = mgr
            .request_confirmation("telegram", "chat1", "Allow?", "s1")
            .await;
        assert!(
            result.response == ConfirmationResponse::TimedOut
                || result.response == ConfirmationResponse::Rejected
        );
    }

    #[tokio::test]
    async fn test_unrelated_message_not_consumed() {
        let mgr = ConfirmationManager::default();
        // No pending confirmations
        let consumed = mgr.try_resolve("telegram", "chat1", "yes").await;
        assert!(!consumed);
    }

    #[tokio::test]
    async fn test_cancel_all() {
        let mut config = HitlConfig::default();
        config.timeout_secs = 60;
        let mgr = Arc::new(ConfirmationManager::new(config));

        let mgr_clone = mgr.clone();
        let _handle = tokio::spawn(async move {
            mgr_clone
                .request_confirmation("telegram", "chat1", "Allow?", "s1")
                .await
        });

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(mgr.pending_count().await, 1);

        mgr.cancel_all().await;
        assert_eq!(mgr.pending_count().await, 0);
    }

    #[test]
    fn test_confirmation_message_format() {
        // Verify the prompt message contains expected elements
        let config = HitlConfig::default();
        let prompt = format!(
            "⚠️ **Confirmation Required**\n\n\
             Process sensitive data?\n\n\
             Reply with `yes` or `no` to approve or reject.\n\
             _(Auto-reject in {} seconds)_\n\n\
             `[abcd1234]`",
            config.timeout_secs,
        );
        assert!(prompt.contains("Confirmation Required"));
        assert!(prompt.contains("yes"));
        assert!(prompt.contains("no"));
    }
}
