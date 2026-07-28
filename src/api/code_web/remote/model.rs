use std::fmt;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub(in crate::api::code_web) const REMOTE_SNAPSHOT_SCHEMA_VERSION: u32 = 1;

#[derive(Clone, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(transparent)]
pub(in crate::api::code_web) struct RemoteTargetId(String);

impl RemoteTargetId {
    pub(in crate::api::code_web) fn for_source(kind: RemoteTargetKind, source_id: &str) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(b"a3s.remote-target.v1\0");
        hasher.update(kind.domain_tag());
        hasher.update(b"\0");
        hasher.update(source_id.as_bytes());
        let digest = hasher.finalize();
        let prefix = match kind {
            RemoteTargetKind::ManagedSession => "rtm_",
            RemoteTargetKind::CooperativeAgent => "rtc_",
            RemoteTargetKind::ObservedProcess => "rto_",
        };
        Self(format!("{prefix}{}", hex_prefix(&digest, 12)))
    }

    pub(in crate::api::code_web) fn as_str(&self) -> &str {
        &self.0
    }

    pub(in crate::api::code_web) fn parse(value: &str) -> Option<Self> {
        let valid_prefix =
            value.starts_with("rtm_") || value.starts_with("rtc_") || value.starts_with("rto_");
        let digest = value.get(4..)?;
        (valid_prefix && digest.len() == 24 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .then(|| Self(value.to_string()))
    }

    pub(in crate::api::code_web) fn short_ref(&self) -> &str {
        self.0
            .rsplit_once('_')
            .map(|(_, digest)| &digest[..digest.len().min(8)])
            .unwrap_or(self.0.as_str())
    }
}

impl fmt::Debug for RemoteTargetId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_tuple("RemoteTargetId")
            .field(&self.0)
            .finish()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum RemoteTargetKind {
    ManagedSession,
    CooperativeAgent,
    ObservedProcess,
}

impl RemoteTargetKind {
    fn domain_tag(self) -> &'static [u8] {
        match self {
            Self::ManagedSession => b"managed",
            Self::CooperativeAgent => b"cooperative",
            Self::ObservedProcess => b"observed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum RemoteEvidenceConfidence {
    Authoritative,
    Exact,
    Process,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum RemoteTargetState {
    Planning,
    Working,
    WaitingApproval,
    WaitingInput,
    Queued,
    Paused,
    Idle,
    Completed,
    Failed,
    Cancelled,
    Detected,
    Unknown,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum RemoteAttention {
    None,
    ActionRequired,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum RemoteCapability {
    ReadStatus,
    ReadChildren,
    ReadSafeReply,
    SendMessage,
    CreateSession,
    ArchiveSession,
    Stop,
    Cancel,
    Reply,
    ApproveOnce,
    Deny,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct RemoteProgress {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web) goal_summary: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web) percent: Option<u8>,
    pub(in crate::api::code_web) completed_steps: usize,
    pub(in crate::api::code_web) total_steps: usize,
    pub(in crate::api::code_web) pending_turns: usize,
    pub(in crate::api::code_web) active_turn: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct RemoteTarget {
    pub(in crate::api::code_web) id: RemoteTargetId,
    pub(in crate::api::code_web) kind: RemoteTargetKind,
    pub(in crate::api::code_web) display_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web) workspace_alias: Option<String>,
    pub(in crate::api::code_web) state: RemoteTargetState,
    pub(in crate::api::code_web) state_detail: String,
    pub(in crate::api::code_web) confidence: RemoteEvidenceConfidence,
    pub(in crate::api::code_web) attention: RemoteAttention,
    pub(in crate::api::code_web) evidence_at_ms: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web) parent_id: Option<RemoteTargetId>,
    pub(in crate::api::code_web) capabilities: Vec<RemoteCapability>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(in crate::api::code_web) progress: Option<RemoteProgress>,
}

impl RemoteTarget {
    pub(in crate::api::code_web) fn observed(
        source_id: &str,
        display_name: String,
        workspace_alias: Option<String>,
        evidence_at_ms: u64,
    ) -> Self {
        Self {
            id: RemoteTargetId::for_source(RemoteTargetKind::ObservedProcess, source_id),
            kind: RemoteTargetKind::ObservedProcess,
            display_name,
            workspace_alias,
            state: RemoteTargetState::Detected,
            state_detail: "Process detected; execution state is unknown.".to_string(),
            confidence: RemoteEvidenceConfidence::Process,
            attention: RemoteAttention::None,
            evidence_at_ms,
            parent_id: None,
            capabilities: vec![RemoteCapability::ReadStatus],
            progress: None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct RemoteSnapshotTotals {
    pub(in crate::api::code_web) managed: usize,
    pub(in crate::api::code_web) cooperative: usize,
    pub(in crate::api::code_web) observed: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct RemoteSnapshot {
    pub(in crate::api::code_web) schema_version: u32,
    pub(in crate::api::code_web) generated_at_ms: u64,
    pub(in crate::api::code_web) degraded: bool,
    pub(in crate::api::code_web) warnings: Vec<String>,
    pub(in crate::api::code_web) totals: RemoteSnapshotTotals,
    pub(in crate::api::code_web) items: Vec<RemoteTarget>,
}

impl RemoteSnapshot {
    pub(in crate::api::code_web) fn new(
        generated_at_ms: u64,
        items: Vec<RemoteTarget>,
        warnings: Vec<String>,
    ) -> Self {
        let totals = RemoteSnapshotTotals {
            managed: items
                .iter()
                .filter(|target| target.kind == RemoteTargetKind::ManagedSession)
                .count(),
            cooperative: items
                .iter()
                .filter(|target| target.kind == RemoteTargetKind::CooperativeAgent)
                .count(),
            observed: items
                .iter()
                .filter(|target| target.kind == RemoteTargetKind::ObservedProcess)
                .count(),
        };
        Self {
            schema_version: REMOTE_SNAPSHOT_SCHEMA_VERSION,
            generated_at_ms,
            degraded: !warnings.is_empty(),
            warnings,
            totals,
            items,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) struct RemoteReadScope {
    pub(in crate::api::code_web) agents: bool,
    pub(in crate::api::code_web) sessions: bool,
    pub(in crate::api::code_web) session_content: bool,
}

impl Default for RemoteReadScope {
    fn default() -> Self {
        Self {
            agents: true,
            sessions: true,
            session_content: false,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) enum RemoteReadQuery {
    ListTargets,
    ListSessions,
    Inspect(RemoteTargetId),
    LatestReply(RemoteTargetId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) enum RemoteReadResult {
    Snapshot(RemoteSnapshot),
    Target(Option<RemoteTarget>),
    LatestReply(Option<SafeReplyExcerpt>),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) struct RemoteReadReceipt {
    pub(in crate::api::code_web) query: RemoteReadQuery,
    pub(in crate::api::code_web) generated_at_ms: u64,
    pub(in crate::api::code_web) result: RemoteReadResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) struct SafeReplyExcerpt {
    pub(in crate::api::code_web) target_id: RemoteTargetId,
    pub(in crate::api::code_web) text: String,
    pub(in crate::api::code_web) truncated: bool,
}

pub(in crate::api::code_web) fn safe_display_label(
    value: Option<&str>,
    fallback: &str,
    max_chars: usize,
) -> String {
    value
        .map(|value| sanitize_remote_text(value, max_chars))
        .filter(|value| !value.is_empty() && value != "[redacted]")
        .unwrap_or_else(|| fallback.to_string())
}

pub(in crate::api::code_web) fn safe_optional_summary(
    value: Option<&str>,
    max_chars: usize,
) -> Option<String> {
    value
        .map(|value| sanitize_remote_text(value, max_chars))
        .filter(|value| !value.is_empty())
}

pub(in crate::api::code_web) fn safe_workspace_alias(value: Option<&str>) -> Option<String> {
    let value = value?.replace('\\', "/");
    let basename = value
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or_default();
    let alias = safe_display_label(Some(basename), "workspace", 48);
    Some(alias)
}

pub(in crate::api::code_web) fn sanitize_remote_text(value: &str, max_chars: usize) -> String {
    let normalized = value
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .collect::<String>();
    let mut parts = Vec::new();
    for part in normalized.split_whitespace() {
        if looks_sensitive(part) {
            parts.push("[redacted]".to_string());
        } else if looks_like_absolute_path(part) {
            parts.push("[path]".to_string());
        } else {
            parts.push(part.to_string());
        }
    }
    truncate_chars(&parts.join(" "), max_chars)
}

fn looks_sensitive(value: &str) -> bool {
    let lowered = value.to_ascii_lowercase();
    [
        "authorization:",
        "bearer",
        "bot_token",
        "context_token",
        "api_key",
        "apikey",
        "password=",
        "token=",
        "secret=",
        "sk-",
        "ghp_",
        "github_pat_",
        "-----begin",
    ]
    .iter()
    .any(|marker| lowered.contains(marker))
}

fn looks_like_absolute_path(value: &str) -> bool {
    let value = value.trim_matches(|character: char| {
        matches!(
            character,
            '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';' | ':' | '"' | '\''
        )
    });
    value.starts_with('/')
        || value.starts_with("~/")
        || value.starts_with("\\\\")
        || (value.len() >= 3
            && value.as_bytes()[0].is_ascii_alphabetic()
            && value.as_bytes()[1] == b':'
            && matches!(value.as_bytes()[2], b'/' | b'\\'))
}

pub(in crate::api::code_web) fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let mut truncated = value
        .chars()
        .take(max_chars.saturating_sub(1))
        .collect::<String>();
    truncated.push('…');
    truncated
}

fn hex_prefix(bytes: &[u8], take: usize) -> String {
    let mut value = String::with_capacity(take * 2);
    for byte in bytes.iter().take(take) {
        use std::fmt::Write as _;
        let _ = write!(&mut value, "{byte:02x}");
    }
    value
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remote_ids_are_opaque_stable_and_kind_scoped() {
        let managed = RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, "session-42");
        let managed_again =
            RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, "session-42");
        let observed = RemoteTargetId::for_source(RemoteTargetKind::ObservedProcess, "session-42");

        assert_eq!(managed, managed_again);
        assert_ne!(managed, observed);
        assert!(!managed.as_str().contains("session-42"));
        assert_eq!(managed.as_str().len(), 28);
    }

    #[test]
    fn remote_text_removes_paths_controls_and_common_secret_shapes() {
        let rendered = sanitize_remote_text(
            "Fix /Users/alice/private\nusing sk-live-canary and token=canary",
            200,
        );
        assert_eq!(rendered, "Fix [path] using [redacted] and [redacted]");
        assert!(!rendered.contains("alice"));
        assert!(!rendered.contains("canary"));
        assert_eq!(
            safe_workspace_alias(Some("/Users/alice/project")),
            Some("project".to_string())
        );
    }

    #[test]
    fn observed_targets_are_structurally_read_only() {
        let target = RemoteTarget::observed(
            "process-evidence",
            "Codex".to_string(),
            Some("project".to_string()),
            10,
        );
        assert_eq!(target.capabilities, vec![RemoteCapability::ReadStatus]);
        assert_eq!(target.state, RemoteTargetState::Detected);
        assert_eq!(target.confidence, RemoteEvidenceConfidence::Process);
    }
}
