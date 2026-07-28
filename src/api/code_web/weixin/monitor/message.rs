use async_trait::async_trait;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::super::credential_store::WeixinCredentials;
use super::super::runtime_store::InboundMessage;
use a3s_boot::ilink::{SecretValue, WeixinMessage, MESSAGE_STATE_FINISH, MESSAGE_TYPE_USER};

pub(super) const MAX_HANDLER_RESPONSE_BYTES: usize = 16 * 1024;

#[async_trait]
pub(in crate::api::code_web::weixin) trait WeixinInboundHandler:
    Send + Sync
{
    async fn handle(&self, message: &InboundMessage)
        -> Result<Option<String>, InboundHandlerError>;
}

#[cfg(test)]
pub(in crate::api::code_web::weixin) struct AlphaDisabledHandler;

#[cfg(test)]
#[async_trait]
impl WeixinInboundHandler for AlphaDisabledHandler {
    async fn handle(
        &self,
        _message: &InboundMessage,
    ) -> Result<Option<String>, InboundHandlerError> {
        Ok(Some(
            "A3S WeChat Remote is connected, but remote commands are not enabled yet.".to_string(),
        ))
    }
}

pub(super) fn accepted_inbound_message(
    credentials: &WeixinCredentials,
    message: &WeixinMessage,
) -> Option<InboundMessage> {
    if message.message_type != Some(MESSAGE_TYPE_USER)
        || message.message_state != Some(MESSAGE_STATE_FINISH)
        || message.group_id.is_some()
    {
        return None;
    }
    let sender_id = message.from_user_id.as_ref()?;
    let recipient_id = message.to_user_id.as_ref()?;
    if !constant_time_secret_eq(sender_id, &credentials.owner_id)
        || !constant_time_secret_eq(recipient_id, &credentials.bot_id)
    {
        return None;
    }
    let text = message.text()?;
    if text.is_empty() || text.len() > MAX_HANDLER_RESPONSE_BYTES || text.contains('\0') {
        return None;
    }
    let key = stable_message_key(message)?;
    let run_id = message.run_id.as_ref().and_then(|value| {
        (!value.is_empty()
            && value.len() <= 128
            && value.is_ascii()
            && !value.chars().any(char::is_control))
        .then(|| value.clone())
    });
    Some(InboundMessage {
        key,
        sender_id: sender_id.clone(),
        recipient_id: Some(recipient_id.clone()),
        group_id: None,
        context_token: message.context_token.clone(),
        text: SecretValue::new(text.to_string()).ok()?,
        run_id,
        created_at_ms: message.create_time_ms,
    })
}

fn stable_message_key(message: &WeixinMessage) -> Option<String> {
    let mut hasher = Sha256::new();
    if let Some(message_id) = message.message_id {
        hasher.update(b"message-id\0");
        hasher.update(message_id.to_be_bytes());
    } else {
        let client_id = message.client_id.as_deref()?;
        let sequence = message.seq?;
        if client_id.is_empty() || client_id.contains('\0') {
            return None;
        }
        hasher.update(b"client-sequence\0");
        hasher.update(client_id.as_bytes());
        hasher.update(b"\0");
        hasher.update(sequence.to_be_bytes());
    }
    let digest = hasher.finalize();
    Some(format!("wx-{}", hex_bytes(&digest)))
}

fn hex_bytes(bytes: &[u8]) -> String {
    let mut value = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

fn constant_time_secret_eq(left: &SecretValue, right: &SecretValue) -> bool {
    let left = left.expose().as_bytes();
    let right = right.expose().as_bytes();
    let max_len = left.len().max(right.len());
    let mut difference = left.len() ^ right.len();
    for index in 0..max_len {
        let left_byte = left.get(index).copied().unwrap_or_default();
        let right_byte = right.get(index).copied().unwrap_or_default();
        difference |= usize::from(left_byte ^ right_byte);
    }
    difference == 0
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(in crate::api::code_web::weixin) enum InboundHandlerError {
    #[error("Weixin inbound handler rejected the message")]
    Rejected,
}
