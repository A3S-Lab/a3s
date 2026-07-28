use std::collections::BTreeMap;
use std::fmt;

use serde::{Deserialize, Serialize};
use thiserror::Error;

use a3s_boot::ilink::SecretValue;

pub(super) const RUNTIME_SCHEMA_VERSION: u32 = 1;
pub(super) const MAX_INBOX_ENTRIES: usize = 4_096;
pub(super) const MAX_OUTBOX_ENTRIES: usize = 1_024;
pub(super) const MAX_IDEMPOTENCY_ENTRIES: usize = 4_096;
pub(super) const MAX_BATCH_MESSAGES: usize = 128;
const MAX_KEY_BYTES: usize = 256;
const MAX_RUN_ID_BYTES: usize = 256;
const MAX_LIST_CONTEXT_TARGETS: usize = 12;
const MAX_LIST_CONTEXT_PAGE: u16 = 100;

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct RuntimeState {
    pub(super) schema_version: u32,
    pub(super) next_sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) cursor: Option<SecretValue>,
    #[serde(default)]
    pub(super) inbox: BTreeMap<String, InboxRecord>,
    #[serde(default)]
    pub(super) outbox: BTreeMap<String, OutboundRecord>,
    #[serde(default)]
    pub(super) idempotency: BTreeMap<String, IdempotencyState>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) selection: Option<RemoteSelection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) list_context: Option<RemoteListContext>,
}

impl Default for RuntimeState {
    fn default() -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            next_sequence: 1,
            cursor: None,
            inbox: BTreeMap::new(),
            outbox: BTreeMap::new(),
            idempotency: BTreeMap::new(),
            selection: None,
            list_context: None,
        }
    }
}

impl fmt::Debug for RuntimeState {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RuntimeState")
            .field("schema_version", &self.schema_version)
            .field("next_sequence", &self.next_sequence)
            .field("has_cursor", &self.cursor.is_some())
            .field("inbox_count", &self.inbox.len())
            .field("outbox_count", &self.outbox.len())
            .field("idempotency_count", &self.idempotency.len())
            .field("has_selection", &self.selection.is_some())
            .field("has_list_context", &self.list_context.is_some())
            .finish()
    }
}

impl RuntimeState {
    pub(super) fn validate(&self) -> Result<(), RuntimeModelError> {
        if self.schema_version != RUNTIME_SCHEMA_VERSION || self.next_sequence == 0 {
            return Err(RuntimeModelError::UnsupportedSchema);
        }
        if self.inbox.len() > MAX_INBOX_ENTRIES
            || self.outbox.len() > MAX_OUTBOX_ENTRIES
            || self.idempotency.len() > MAX_IDEMPOTENCY_ENTRIES
        {
            return Err(RuntimeModelError::EntryLimit);
        }
        for (key, record) in &self.inbox {
            validate_key(key)?;
            if record.message.key != *key {
                return Err(RuntimeModelError::InvalidRecord);
            }
            if record.staged_sequence == 0 || record.staged_sequence >= self.next_sequence {
                return Err(RuntimeModelError::InvalidRecord);
            }
            if usize::from(record.staged_index) >= MAX_BATCH_MESSAGES {
                return Err(RuntimeModelError::InvalidRecord);
            }
            record.message.validate()?;
        }
        for (client_id, record) in &self.outbox {
            validate_key(client_id)?;
            if record.client_id != *client_id {
                return Err(RuntimeModelError::InvalidRecord);
            }
            record.validate()?;
        }
        for key in self.idempotency.keys() {
            validate_key(key)?;
        }
        if let Some(selection) = &self.selection {
            selection.validate()?;
        }
        if let Some(context) = &self.list_context {
            context.validate()?;
        }
        Ok(())
    }

    pub(super) fn apply(&mut self, record: &JournalRecord) -> Result<(), RuntimeModelError> {
        if record.schema_version != RUNTIME_SCHEMA_VERSION {
            return Err(RuntimeModelError::UnsupportedSchema);
        }
        if record.sequence != self.next_sequence {
            return Err(RuntimeModelError::SequenceMismatch);
        }
        match &record.event {
            RuntimeEvent::StageInboundBatch { cursor, messages } => {
                if messages.len() > MAX_BATCH_MESSAGES {
                    return Err(RuntimeModelError::EntryLimit);
                }
                let additional = messages
                    .iter()
                    .filter(|message| !self.inbox.contains_key(&message.key))
                    .count();
                if self.inbox.len().saturating_add(additional) > MAX_INBOX_ENTRIES {
                    return Err(RuntimeModelError::EntryLimit);
                }
                for (staged_index, message) in messages.iter().enumerate() {
                    message.validate()?;
                    let staged_index =
                        u16::try_from(staged_index).map_err(|_| RuntimeModelError::EntryLimit)?;
                    self.inbox
                        .entry(message.key.clone())
                        .or_insert_with(|| InboxRecord {
                            message: message.clone(),
                            state: InboxState::Staged,
                            staged_sequence: record.sequence,
                            staged_index,
                        });
                }
                self.cursor = Some(cursor.clone());
            }
            RuntimeEvent::SetInboxState { key, state } => {
                validate_key(key)?;
                let record = self
                    .inbox
                    .get_mut(key)
                    .ok_or(RuntimeModelError::MissingEntry)?;
                if record.state != InboxState::Staged && record.state != *state {
                    return Err(RuntimeModelError::InvalidTransition);
                }
                record.state = *state;
            }
            RuntimeEvent::QueueOutbound { message } => {
                message.validate()?;
                if self.outbox.len() >= MAX_OUTBOX_ENTRIES {
                    return Err(RuntimeModelError::EntryLimit);
                }
                if self.outbox.contains_key(&message.client_id) {
                    return Err(RuntimeModelError::DuplicateEntry);
                }
                self.outbox
                    .insert(message.client_id.clone(), message.clone());
            }
            RuntimeEvent::QueueOutboundForInbox { key, message } => {
                validate_key(key)?;
                message.validate()?;
                if self.outbox.len() >= MAX_OUTBOX_ENTRIES {
                    return Err(RuntimeModelError::EntryLimit);
                }
                if self.outbox.contains_key(&message.client_id) {
                    return Err(RuntimeModelError::DuplicateEntry);
                }
                let inbox = self
                    .inbox
                    .get_mut(key)
                    .ok_or(RuntimeModelError::MissingEntry)?;
                if inbox.state != InboxState::Staged {
                    return Err(RuntimeModelError::InvalidTransition);
                }
                self.outbox
                    .insert(message.client_id.clone(), message.clone());
                inbox.state = InboxState::Completed;
            }
            RuntimeEvent::SetOutboundState { client_id, state } => {
                validate_key(client_id)?;
                let record = self
                    .outbox
                    .get_mut(client_id)
                    .ok_or(RuntimeModelError::MissingEntry)?;
                if !record.state.can_transition_to(*state) {
                    return Err(RuntimeModelError::InvalidTransition);
                }
                record.state = *state;
            }
            RuntimeEvent::ReserveIdempotency { key } => {
                validate_key(key)?;
                if self.idempotency.len() >= MAX_IDEMPOTENCY_ENTRIES {
                    return Err(RuntimeModelError::EntryLimit);
                }
                if self.idempotency.contains_key(key) {
                    return Err(RuntimeModelError::DuplicateEntry);
                }
                self.idempotency
                    .insert(key.clone(), IdempotencyState::Reserved);
            }
            RuntimeEvent::SetIdempotencyState { key, state } => {
                validate_key(key)?;
                let current = self
                    .idempotency
                    .get_mut(key)
                    .ok_or(RuntimeModelError::MissingEntry)?;
                if *current != IdempotencyState::Reserved && *current != *state {
                    return Err(RuntimeModelError::InvalidTransition);
                }
                *current = *state;
            }
            RuntimeEvent::SetSelection { selection } => {
                if let Some(selection) = selection {
                    selection.validate()?;
                }
                self.selection = selection.clone();
            }
            RuntimeEvent::SetListContext { context } => {
                if let Some(context) = context {
                    context.validate()?;
                }
                self.list_context = context.clone();
            }
            RuntimeEvent::Reset => {
                self.cursor = None;
                self.inbox.clear();
                self.outbox.clear();
                self.idempotency.clear();
                self.selection = None;
                self.list_context = None;
            }
        }
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(RuntimeModelError::SequenceOverflow)?;
        self.validate()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct JournalRecord {
    pub(super) schema_version: u32,
    pub(super) sequence: u64,
    pub(super) event: RuntimeEvent,
}

impl JournalRecord {
    pub(super) fn new(sequence: u64, event: RuntimeEvent) -> Self {
        Self {
            schema_version: RUNTIME_SCHEMA_VERSION,
            sequence,
            event,
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub(super) enum RuntimeEvent {
    StageInboundBatch {
        cursor: SecretValue,
        messages: Vec<InboundMessage>,
    },
    SetInboxState {
        key: String,
        state: InboxState,
    },
    QueueOutbound {
        message: OutboundRecord,
    },
    QueueOutboundForInbox {
        key: String,
        message: OutboundRecord,
    },
    SetOutboundState {
        client_id: String,
        state: OutboundState,
    },
    ReserveIdempotency {
        key: String,
    },
    SetIdempotencyState {
        key: String,
        state: IdempotencyState,
    },
    SetSelection {
        selection: Option<RemoteSelection>,
    },
    SetListContext {
        context: Option<RemoteListContext>,
    },
    Reset,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web::weixin) enum RemoteListKind {
    Targets,
    Sessions,
    Disambiguation,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::api::code_web::weixin) struct RemoteListContext {
    pub(in crate::api::code_web::weixin) kind: RemoteListKind,
    pub(in crate::api::code_web::weixin) page: u16,
    pub(in crate::api::code_web::weixin) target_ids: Vec<String>,
    pub(in crate::api::code_web::weixin) listed_at_ms: u64,
}

impl RemoteListContext {
    fn validate(&self) -> Result<(), RuntimeModelError> {
        if self.page == 0
            || self.page > MAX_LIST_CONTEXT_PAGE
            || self.listed_at_ms == 0
            || self.target_ids.len() > MAX_LIST_CONTEXT_TARGETS
        {
            return Err(RuntimeModelError::InvalidRecord);
        }
        for (index, target_id) in self.target_ids.iter().enumerate() {
            validate_remote_target_id(target_id)?;
            if self.target_ids[..index].contains(target_id) {
                return Err(RuntimeModelError::DuplicateEntry);
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::api::code_web::weixin) struct RemoteSelection {
    pub(in crate::api::code_web::weixin) target_id: String,
    pub(in crate::api::code_web::weixin) selected_at_ms: u64,
}

impl RemoteSelection {
    fn validate(&self) -> Result<(), RuntimeModelError> {
        validate_remote_target_id(&self.target_id)?;
        if self.selected_at_ms == 0 {
            return Err(RuntimeModelError::InvalidRecord);
        }
        Ok(())
    }
}

fn validate_remote_target_id(target_id: &str) -> Result<(), RuntimeModelError> {
    let valid_prefix = target_id.starts_with("rtm_")
        || target_id.starts_with("rtc_")
        || target_id.starts_with("rto_");
    let digest = target_id.get(4..).unwrap_or_default();
    if !valid_prefix || digest.len() != 24 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return Err(RuntimeModelError::InvalidRecord);
    }
    Ok(())
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::api::code_web::weixin) struct InboundMessage {
    pub(in crate::api::code_web::weixin) key: String,
    pub(in crate::api::code_web::weixin) sender_id: SecretValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) recipient_id: Option<SecretValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) group_id: Option<SecretValue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) context_token: Option<SecretValue>,
    pub(in crate::api::code_web::weixin) text: SecretValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) created_at_ms: Option<u64>,
}

impl InboundMessage {
    fn validate(&self) -> Result<(), RuntimeModelError> {
        validate_key(&self.key)?;
        validate_optional_ascii(&self.run_id, MAX_RUN_ID_BYTES)
    }
}

impl fmt::Debug for InboundMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("InboundMessage")
            .field("key", &"[REDACTED]")
            .field("has_group", &self.group_id.is_some())
            .field("has_context_token", &self.context_token.is_some())
            .field("has_run_id", &self.run_id.is_some())
            .field("created_at_ms", &self.created_at_ms)
            .finish()
    }
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::api::code_web::weixin) struct InboxRecord {
    pub(in crate::api::code_web::weixin) message: InboundMessage,
    pub(in crate::api::code_web::weixin) state: InboxState,
    pub(in crate::api::code_web::weixin) staged_sequence: u64,
    #[serde(default)]
    pub(in crate::api::code_web::weixin) staged_index: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web::weixin) enum InboxState {
    Staged,
    Completed,
    Quarantined,
}

#[derive(Clone)]
pub(in crate::api::code_web::weixin) struct OutboundDraft {
    pub(in crate::api::code_web::weixin) recipient_id: SecretValue,
    pub(in crate::api::code_web::weixin) context_token: Option<SecretValue>,
    pub(in crate::api::code_web::weixin) text: SecretValue,
    pub(in crate::api::code_web::weixin) run_id: Option<String>,
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(in crate::api::code_web::weixin) struct OutboundRecord {
    pub(in crate::api::code_web::weixin) client_id: String,
    pub(in crate::api::code_web::weixin) recipient_id: SecretValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) context_token: Option<SecretValue>,
    pub(in crate::api::code_web::weixin) text: SecretValue,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) run_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) source_inbox_sequence: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web::weixin) source_inbox_index: Option<u16>,
    pub(in crate::api::code_web::weixin) created_at_ms: u64,
    pub(in crate::api::code_web::weixin) state: OutboundState,
}

impl OutboundRecord {
    fn validate(&self) -> Result<(), RuntimeModelError> {
        validate_key(&self.client_id)?;
        validate_optional_ascii(&self.run_id, MAX_RUN_ID_BYTES)?;
        match (self.source_inbox_sequence, self.source_inbox_index) {
            (Some(sequence), Some(index))
                if sequence > 0 && usize::from(index) < MAX_BATCH_MESSAGES =>
            {
                Ok(())
            }
            (None, None) => Ok(()),
            _ => Err(RuntimeModelError::InvalidRecord),
        }
    }
}

impl fmt::Debug for OutboundRecord {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OutboundRecord")
            .field("client_id", &"[REDACTED]")
            .field("has_context_token", &self.context_token.is_some())
            .field("has_run_id", &self.run_id.is_some())
            .field("has_source_inbox", &self.source_inbox_sequence.is_some())
            .field("created_at_ms", &self.created_at_ms)
            .field("state", &self.state)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web::weixin) enum OutboundState {
    Pending,
    Sending,
    Sent,
    Failed,
    OutcomeUnknown,
}

impl OutboundState {
    fn can_transition_to(self, next: Self) -> bool {
        self == next
            || matches!(
                (self, next),
                (Self::Pending, Self::Sending | Self::Failed)
                    | (
                        Self::Sending,
                        Self::Sent | Self::Failed | Self::OutcomeUnknown
                    )
            )
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web::weixin) enum IdempotencyState {
    Reserved,
    Succeeded,
    Failed,
    OutcomeUnknown,
}

fn validate_key(value: &str) -> Result<(), RuntimeModelError> {
    if value.is_empty()
        || value.len() > MAX_KEY_BYTES
        || !value.is_ascii()
        || value.chars().any(char::is_control)
    {
        return Err(RuntimeModelError::InvalidRecord);
    }
    Ok(())
}

fn validate_optional_ascii(
    value: &Option<String>,
    max_bytes: usize,
) -> Result<(), RuntimeModelError> {
    if value.as_ref().is_some_and(|value| {
        value.is_empty()
            || value.len() > max_bytes
            || !value.is_ascii()
            || value.chars().any(char::is_control)
    }) {
        return Err(RuntimeModelError::InvalidRecord);
    }
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, PartialEq, Eq)]
pub(in crate::api::code_web::weixin) enum RuntimeModelError {
    #[error("Weixin runtime schema is unsupported")]
    UnsupportedSchema,
    #[error("Weixin runtime sequence is invalid")]
    SequenceMismatch,
    #[error("Weixin runtime sequence overflowed")]
    SequenceOverflow,
    #[error("Weixin runtime record is invalid")]
    InvalidRecord,
    #[error("Weixin runtime entry limit was reached")]
    EntryLimit,
    #[error("Weixin runtime entry already exists")]
    DuplicateEntry,
    #[error("Weixin runtime entry was not found")]
    MissingEntry,
    #[error("Weixin runtime transition is invalid")]
    InvalidTransition,
}
