//! Multi-channel message adapters
//!
//! Provides unified interface for receiving and sending messages
//! across different messaging platforms.

mod adapter;
pub mod auth;
pub mod confirmation;
mod dingtalk;
mod discord;
mod feishu;
mod message;
mod slack;
pub mod supervisor;
mod telegram;
mod webchat;
mod wecom;

pub use adapter::{ChannelAdapter, ChannelEvent};
pub use auth::{
    AuthLayer, AuthMiddleware, AuthOutcome, ChannelAuth, DingTalkAuth, DiscordAuth, FeishuAuth,
    SlackAuth, TelegramAuth, WeComAuth,
};
pub use confirmation::{ConfirmationManager, ConfirmationResponse, ConfirmationResult, HitlConfig};
pub use dingtalk::DingTalkAdapter;
pub use discord::{DiscordAdapter, DiscordMessage};
pub use feishu::FeishuAdapter;
pub use message::{InboundMessage, MessageAttachment, OutboundMessage};
pub use slack::SlackAdapter;
pub use telegram::TelegramAdapter;
pub use webchat::WebChatAdapter;
pub use wecom::WeComAdapter;

/// Resolve a credential reference: try environment variable first, fall back to inline value.
///
/// This allows config files to use either:
/// - An env var name: `"FEISHU_APP_SECRET"` → reads `$FEISHU_APP_SECRET`
/// - An inline secret: `"sk-abc123..."` → used directly
pub(crate) fn resolve_credential(credential_ref: &str) -> crate::error::Result<String> {
    if let Ok(val) =
        std::env::var(credential_ref).or_else(|_| std::env::var(credential_ref.to_uppercase()))
    {
        return Ok(val);
    }
    if !credential_ref.is_empty() {
        return Ok(credential_ref.to_string());
    }
    Err(crate::error::Error::Channel(format!(
        "Failed to resolve credential: {}",
        credential_ref
    )))
}
