//! Authenticated, one-shot Agent Island controls.
//!
//! The native helper only appends bounded request files to a private queue.
//! Each TUI accepts requests for its own instance and only when they match a
//! currently issued activity grant. Alternative approval decisions share one
//! token, so the earliest valid choice invalidates every sibling action.

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::Path;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tokio::io::AsyncReadExt;

pub(super) const CONTROL_REQUEST_SCHEMA: &str = "a3s.agent_control_request.v1";
pub(super) const CONTROL_REQUEST_DIRECTORY: &str = "control-requests";
const CONTROL_GRANT_TTL_MS: u64 = 12_000;
const CONTROL_REQUEST_TTL_MS: u64 = 10_000;
const CONTROL_REQUEST_FUTURE_SKEW_MS: u64 = 5_000;
const PRESENCE_FUTURE_SKEW_MS: u64 = 5_000;
const MAX_ACTIVITY_ID_CHARS: usize = 160;
pub(super) const MAX_CONTROL_ACTIONS: usize = 4;
const MAX_CONTROL_CONTEXT_CHARS: usize = 192;
const MAX_CONTROL_REQUEST_BYTES: u64 = 8 * 1024;
const MAX_CONTROL_REQUEST_FILES: usize = 512;
const MAX_CONTROL_REQUESTS_PER_SCAN: usize = 16;
const MAX_CONTROL_REQUEST_ID_CHARS: usize = 96;
const MAX_CONTROL_REMOVALS_PER_SCAN: usize = 64;
const CONTROL_TOKEN_HEX_CHARS: usize = 32;
const MAX_REPLY_CHARS: usize = 1_000;
const MAX_REPLY_BYTES: usize = 4 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentControlActionKind {
    ApproveOnce,
    ApproveAlways,
    Deny,
    Stop,
    Cancel,
    Reply,
    #[serde(other)]
    Unknown,
}

impl AgentControlActionKind {
    fn is_supported(self) -> bool {
        !matches!(self, Self::Unknown)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentControlAction {
    pub(crate) action: AgentControlActionKind,
    pub(crate) token: String,
    pub(crate) target_instance_id: String,
    pub(crate) expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentControlGrantSpec {
    pub(crate) activity_id: String,
    pub(crate) context: String,
    pub(crate) actions: Vec<AgentControlActionKind>,
}

impl AgentControlGrantSpec {
    pub(crate) fn new(
        activity_id: impl Into<String>,
        context: impl Into<String>,
        actions: impl IntoIterator<Item = AgentControlActionKind>,
    ) -> Self {
        Self {
            activity_id: activity_id.into(),
            context: context.into(),
            actions: actions.into_iter().collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AgentControlRequest {
    pub(crate) activity_id: String,
    pub(crate) context: String,
    pub(crate) action: AgentControlActionKind,
    pub(crate) message: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentControlGrant {
    context: String,
    actions: Vec<AgentControlActionKind>,
    token: String,
    expires_at_ms: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(super) struct AgentControlProtocolRequest {
    pub(super) schema: String,
    pub(super) request_id: String,
    pub(super) target_instance_id: String,
    pub(super) activity_id: String,
    pub(super) action: AgentControlActionKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) message: Option<String>,
    pub(super) token: String,
    pub(super) created_at_ms: u64,
    pub(super) expires_at_ms: u64,
}

#[derive(Debug, Default)]
pub(super) struct AgentControlGrants {
    grants: Mutex<HashMap<String, AgentControlGrant>>,
}

impl AgentControlGrants {
    pub(super) fn reconcile(
        &self,
        target_instance_id: &str,
        specs: impl IntoIterator<Item = AgentControlGrantSpec>,
        now_ms: u64,
    ) -> HashMap<String, Vec<AgentControlAction>> {
        let mut specs = specs
            .into_iter()
            .filter_map(|mut spec| {
                if !valid_bounded_identifier(&spec.activity_id, MAX_ACTIVITY_ID_CHARS)
                    || !valid_bounded_identifier(&spec.context, MAX_CONTROL_CONTEXT_CHARS)
                {
                    return None;
                }
                spec.actions.retain(|action| action.is_supported());
                spec.actions.truncate(MAX_CONTROL_ACTIONS);
                let mut seen = HashSet::new();
                spec.actions.retain(|action| seen.insert(*action));
                (!spec.actions.is_empty()).then_some(spec)
            })
            .collect::<Vec<_>>();
        specs.sort_by(|left, right| left.activity_id.cmp(&right.activity_id));
        specs.dedup_by(|left, right| left.activity_id == right.activity_id);

        let expires_at_ms = now_ms.saturating_add(CONTROL_GRANT_TTL_MS);
        let mut grants = self
            .grants
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let desired = specs
            .iter()
            .map(|spec| spec.activity_id.as_str())
            .collect::<HashSet<_>>();
        grants.retain(|activity_id, _| desired.contains(activity_id.as_str()));

        let mut actions_by_activity = HashMap::new();
        for spec in specs {
            let grant = grants
                .entry(spec.activity_id.clone())
                .and_modify(|grant| {
                    if grant.context != spec.context
                        || grant.actions != spec.actions
                        || grant.expires_at_ms < now_ms
                    {
                        grant.context = spec.context.clone();
                        grant.actions = spec.actions.clone();
                        grant.token = format!("{:032x}", rand::random::<u128>());
                    }
                    grant.expires_at_ms = expires_at_ms;
                })
                .or_insert_with(|| AgentControlGrant {
                    context: spec.context,
                    actions: spec.actions,
                    token: format!("{:032x}", rand::random::<u128>()),
                    expires_at_ms,
                });
            actions_by_activity.insert(
                spec.activity_id,
                grant
                    .actions
                    .iter()
                    .map(|action| AgentControlAction {
                        action: *action,
                        token: grant.token.clone(),
                        target_instance_id: target_instance_id.to_string(),
                        expires_at_ms: grant.expires_at_ms,
                    })
                    .collect(),
            );
        }
        actions_by_activity
    }

    pub(super) fn clear(&self) {
        self.grants
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clear();
    }

    pub(super) async fn consume_requests(
        &self,
        directory: Option<&Path>,
        target_instance_id: &str,
        now_ms: u64,
    ) -> anyhow::Result<Vec<AgentControlRequest>> {
        let Some(directory) = directory else {
            return Ok(Vec::new());
        };
        let queue = directory.join(CONTROL_REQUEST_DIRECTORY);
        let metadata = match tokio::fs::symlink_metadata(&queue).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error.into()),
        };
        validate_private_control_directory(&metadata)?;

        let mut entries = tokio::fs::read_dir(&queue).await?;
        let mut entries_seen = 0usize;
        let mut removals = 0usize;
        let mut candidates = Vec::new();
        while let Some(entry) = entries.next_entry().await? {
            if entries_seen == MAX_CONTROL_REQUEST_FILES {
                anyhow::bail!("control request queue exceeds the directory entry limit");
            }
            entries_seen += 1;

            let Some(file_request_id) = parse_control_request_file_name(&entry.file_name()) else {
                continue;
            };
            let path = entry.path();
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let Some(request) = read_control_request_file(&path).await else {
                remove_bounded_control_file(&path, &mut removals).await;
                continue;
            };
            if request.request_id != file_request_id
                || !control_request_has_valid_shape(&request)
                || !control_request_is_fresh(&request, now_ms)
            {
                remove_bounded_control_file(&path, &mut removals).await;
                continue;
            }
            if request.target_instance_id == target_instance_id {
                candidates.push((path, request));
            }
        }

        candidates.sort_by(|left, right| {
            left.1
                .created_at_ms
                .cmp(&right.1.created_at_ms)
                .then_with(|| left.1.request_id.cmp(&right.1.request_id))
        });
        let mut accepted = Vec::new();
        for (path, request) in candidates {
            if accepted.len() == MAX_CONTROL_REQUESTS_PER_SCAN {
                break;
            }
            if let Some(request) = self.consume_grant(&request, target_instance_id, now_ms) {
                accepted.push(request);
            }
            remove_bounded_control_file(&path, &mut removals).await;
        }
        Ok(accepted)
    }

    fn consume_grant(
        &self,
        request: &AgentControlProtocolRequest,
        target_instance_id: &str,
        now_ms: u64,
    ) -> Option<AgentControlRequest> {
        if request.target_instance_id != target_instance_id {
            return None;
        }
        let mut grants = self
            .grants
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let grant = grants.get(&request.activity_id)?;
        let valid = request.token == grant.token
            && request.action.is_supported()
            && grant.actions.contains(&request.action)
            && request.expires_at_ms >= now_ms
            && request.expires_at_ms <= grant.expires_at_ms
            && grant.expires_at_ms >= now_ms;
        if !valid {
            return None;
        }
        let context = grant.context.clone();
        grants.remove(&request.activity_id);
        Some(AgentControlRequest {
            activity_id: request.activity_id.clone(),
            context,
            action: request.action,
            message: request.message.clone(),
        })
    }
}

pub(super) fn sanitize_control_action(action: &AgentControlAction) -> Option<AgentControlAction> {
    (action.action.is_supported()
        && valid_control_token(&action.token)
        && valid_bounded_identifier(&action.target_instance_id, MAX_ACTIVITY_ID_CHARS)
        && action.expires_at_ms > 0)
        .then(|| action.clone())
}

pub(super) fn sanitize_received_actions(
    actions: Vec<AgentControlAction>,
    target_instance_id: &str,
    evidence_at_ms: u64,
) -> Vec<AgentControlAction> {
    let latest_expiry = evidence_at_ms
        .saturating_add(CONTROL_GRANT_TTL_MS)
        .saturating_add(PRESENCE_FUTURE_SKEW_MS);
    let mut seen = HashSet::new();
    actions
        .iter()
        .filter_map(sanitize_control_action)
        .filter(|action| action.target_instance_id == target_instance_id)
        .filter(|action| {
            action.expires_at_ms >= evidence_at_ms && action.expires_at_ms <= latest_expiry
        })
        .filter(|action| seen.insert(action.action))
        .take(MAX_CONTROL_ACTIONS)
        .collect()
}

fn valid_bounded_identifier(value: &str, max_chars: usize) -> bool {
    let count = value.chars().count();
    count > 0
        && count <= max_chars
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{206f}'
                )
        })
}

fn valid_control_token(token: &str) -> bool {
    token.len() == CONTROL_TOKEN_HEX_CHARS && token.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn parse_control_request_file_name(file_name: &OsStr) -> Option<String> {
    let request_id = file_name
        .to_str()?
        .strip_prefix("control-")?
        .strip_suffix(".json")?;
    (request_id.len() <= MAX_CONTROL_REQUEST_ID_CHARS
        && !request_id.is_empty()
        && request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'))
    .then(|| request_id.to_string())
}

fn control_request_has_valid_shape(request: &AgentControlProtocolRequest) -> bool {
    request.schema == CONTROL_REQUEST_SCHEMA
        && request.action.is_supported()
        && valid_control_token(&request.token)
        && valid_bounded_identifier(&request.target_instance_id, MAX_ACTIVITY_ID_CHARS)
        && valid_bounded_identifier(&request.activity_id, MAX_ACTIVITY_ID_CHARS)
        && request.request_id.len() <= MAX_CONTROL_REQUEST_ID_CHARS
        && !request.request_id.is_empty()
        && request
            .request_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        && request.created_at_ms <= request.expires_at_ms
        && match request.action {
            AgentControlActionKind::Reply => {
                request.message.as_deref().is_some_and(valid_reply_message)
            }
            _ => request.message.is_none(),
        }
}

fn valid_reply_message(message: &str) -> bool {
    let message = message.trim();
    !message.is_empty()
        && message.len() <= MAX_REPLY_BYTES
        && message.chars().count() <= MAX_REPLY_CHARS
        && !message.chars().any(|character| {
            (character.is_control() && !matches!(character, '\n' | '\t'))
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{202a}'..='\u{202e}'
                        | '\u{2066}'..='\u{206f}'
                )
        })
}

fn control_request_is_fresh(request: &AgentControlProtocolRequest, now_ms: u64) -> bool {
    request.created_at_ms <= now_ms.saturating_add(CONTROL_REQUEST_FUTURE_SKEW_MS)
        && now_ms.saturating_sub(request.created_at_ms) <= CONTROL_REQUEST_TTL_MS
        && request.expires_at_ms >= now_ms
        && request.expires_at_ms
            <= request
                .created_at_ms
                .saturating_add(CONTROL_GRANT_TTL_MS)
                .saturating_add(CONTROL_REQUEST_FUTURE_SKEW_MS)
}

async fn read_control_request_file(path: &Path) -> Option<AgentControlProtocolRequest> {
    let metadata = tokio::fs::symlink_metadata(path).await.ok()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_CONTROL_REQUEST_BYTES
        || !control_request_file_is_private(&metadata)
    {
        return None;
    }

    let mut options = tokio::fs::OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    options.custom_flags(libc::O_NOFOLLOW);
    let file = options.open(path).await.ok()?;
    let opened_metadata = file.metadata().await.ok()?;
    if !opened_metadata.is_file()
        || opened_metadata.len() > MAX_CONTROL_REQUEST_BYTES
        || !control_request_file_is_private(&opened_metadata)
    {
        return None;
    }
    let mut bytes = Vec::with_capacity(opened_metadata.len() as usize);
    file.take(MAX_CONTROL_REQUEST_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MAX_CONTROL_REQUEST_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

fn control_request_file_is_private(metadata: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        // SAFETY: geteuid has no preconditions and only reads process state.
        metadata.uid() == unsafe { libc::geteuid() } && metadata.permissions().mode() & 0o077 == 0
    }
    #[cfg(not(unix))]
    {
        let _ = metadata;
        true
    }
}

fn validate_private_control_directory(metadata: &std::fs::Metadata) -> anyhow::Result<()> {
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        anyhow::bail!("control request queue is not a real directory");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};

        // SAFETY: geteuid has no preconditions and only reads process state.
        if metadata.uid() != unsafe { libc::geteuid() } {
            anyhow::bail!("control request queue must be owned by the current user");
        }
        if metadata.permissions().mode() & 0o077 != 0 {
            anyhow::bail!("control request queue permissions must not grant group or other access");
        }
    }
    Ok(())
}

async fn remove_bounded_control_file(path: &Path, removals: &mut usize) {
    if *removals < MAX_CONTROL_REMOVALS_PER_SCAN {
        *removals += 1;
        let _ = tokio::fs::remove_file(path).await;
    }
}
