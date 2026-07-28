//! Cross-process coding-agent presence used by the Code TUI and system status
//! surfaces.
//!
//! Cooperating `a3s code` TUI processes publish an exact, short-lived heartbeat
//! in the per-user A3S state directory. Other coding agents are discovered
//! through the same process collector as `a3s top`; those rows are deliberately
//! marked as inferred because a live process does not prove that a task is
//! executing.

mod control;
mod preference;
mod projection;
mod text;

use std::collections::{HashMap, HashSet};
use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::Context;
use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::sync::Mutex;

use self::control::{
    sanitize_control_action, sanitize_received_actions, AgentControlGrants, MAX_CONTROL_ACTIONS,
};
pub(crate) use self::control::{
    AgentControlAction, AgentControlActionKind, AgentControlGrantSpec, AgentControlRequest,
};
#[cfg(test)]
use self::control::{
    AgentControlProtocolRequest, CONTROL_REQUEST_DIRECTORY, CONTROL_REQUEST_SCHEMA,
};
#[cfg(test)]
use self::projection::root_agent_processes;
#[allow(unused_imports)]
pub(crate) use self::projection::{activities_for_presence, sort_activities};
use self::projection::{aggregate_activities, snapshot_requests_island_launch};
#[cfg(test)]
use self::text::sanitize_display_text;
use self::text::{sanitize_nonempty, sanitize_optional, workspace_basename};

#[cfg(test)]
static AGENT_ISLAND_PROCESS_TEST_LOCK: std::sync::OnceLock<Mutex<()>> = std::sync::OnceLock::new();

#[cfg(test)]
pub(crate) fn agent_island_process_test_lock() -> &'static Mutex<()> {
    AGENT_ISLAND_PROCESS_TEST_LOCK.get_or_init(|| Mutex::new(()))
}

use crate::top::collect_processes;
#[cfg(test)]
use crate::top::AgentKind;
#[cfg(test)]
use crate::top::ProcessRow;

const PRESENCE_SCHEMA: &str = "a3s.agent_presence.v1";
const SYSTEM_SNAPSHOT_SCHEMA: &str = "a3s.system_agent_snapshot.v1";
const SYSTEM_SNAPSHOT_FILE: &str = "system-snapshot.json";
const ISLAND_LOCK_FILE: &str = "island.lock";
const PRESENCE_TTL_MS: u64 = 10_000;
const PRESENCE_FUTURE_SKEW_MS: u64 = 5_000;
const MAX_REGISTRY_ENTRIES: usize = 4_096;
const MAX_PRESENCE_FILES: usize = 256;
const MAX_PRESENCE_BYTES: u64 = 64 * 1024;
const MAX_STALE_REMOVALS_PER_SCAN: usize = 64;
const MAX_TASK_CHARS: usize = 240;
const MAX_AGENT_CHARS: usize = 64;
const MAX_CHILD_ID_CHARS: usize = 64;
const MAX_WORKSPACE_CHARS: usize = 128;
const MAX_ATTENTION_REASON_CHARS: usize = 240;
const MAX_CHILDREN: usize = 64;
const MAX_SYSTEM_ACTIVITIES: usize = 256;
const MAX_SYSTEM_SNAPSHOT_BYTES: u64 = 1024 * 1024;
const MAX_ACTIVITY_ID_CHARS: usize = 160;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentActivityState {
    Planning,
    Working,
    WaitingApproval,
    WaitingInput,
    Idle,
    Completed,
    Failed,
    Cancelled,
    /// Only a process was observed; no lifecycle event or heartbeat proves
    /// whether the agent is currently executing a task.
    Unknown,
}

impl AgentActivityState {
    pub(crate) fn attention_rank(self) -> u8 {
        match self {
            Self::Failed => 0,
            Self::WaitingApproval | Self::WaitingInput => 1,
            Self::Planning | Self::Working => 2,
            Self::Cancelled => 3,
            Self::Unknown => 4,
            Self::Idle => 5,
            Self::Completed => 6,
        }
    }

    fn keeps_island_visible(self) -> bool {
        self != Self::Idle
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentActivityConfidence {
    /// A cooperating agent emitted a fresh lifecycle heartbeat.
    Exact,
    /// Only a matching host process was observed.
    Process,
}

impl AgentActivityConfidence {
    fn evidence_rank(self) -> u8 {
        match self {
            Self::Exact => 0,
            Self::Process => 1,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AgentVendor {
    A3s,
    OpenAi,
    Anthropic,
    Google,
    Cursor,
    Moonshot,
    Tencent,
    Alibaba,
    DeepSeek,
    Mistral,
    #[default]
    #[serde(other)]
    Other,
}

impl AgentVendor {
    pub(crate) fn from_hint(value: &str) -> Option<Self> {
        let value = value.to_ascii_lowercase();
        [
            (Self::A3s, &["a3s"][..]),
            (Self::Anthropic, &["anthropic", "claude"][..]),
            (
                Self::OpenAi,
                &["openai", "codex", "chatgpt", "gpt-", "o1", "o3", "o4"][..],
            ),
            (Self::Google, &["google", "gemini"][..]),
            (Self::Cursor, &["cursor"][..]),
            (Self::Moonshot, &["moonshot", "kimi"][..]),
            (
                Self::Tencent,
                &["tencent", "workbuddy", "codebuddy", "hunyuan"][..],
            ),
            (Self::Alibaba, &["alibaba", "qwen", "dashscope"][..]),
            (Self::DeepSeek, &["deepseek"][..]),
            (Self::Mistral, &["mistral", "codestral"][..]),
        ]
        .into_iter()
        .find_map(|(vendor, hints)| {
            hints
                .iter()
                .any(|hint| value.contains(hint))
                .then_some(vendor)
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SystemAgentActivity {
    pub(crate) id: String,
    pub(crate) parent_id: Option<String>,
    pub(crate) agent: String,
    pub(crate) workspace: Option<String>,
    pub(crate) task: Option<String>,
    pub(crate) reason: Option<String>,
    pub(crate) state: AgentActivityState,
    pub(crate) confidence: AgentActivityConfidence,
    pub(crate) vendor: AgentVendor,
    pub(crate) started_at_ms: Option<u64>,
    pub(crate) finished_at_ms: Option<u64>,
    pub(crate) expires_at_ms: u64,
    pub(crate) actions: Vec<AgentControlAction>,
    pub(crate) local: bool,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SystemAgentSnapshot {
    pub(crate) activities: Vec<SystemAgentActivity>,
    pub(crate) warnings: Vec<String>,
}

/// Outcome of one heartbeat, whole-system collection, and shared snapshot
/// export. The TUI does not retain the collected rows; the native island is the
/// presentation owner and reads only the sanitized file named by
/// `snapshot_path`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct SystemAgentRefreshResult {
    pub(crate) snapshot_path: Option<PathBuf>,
    pub(crate) lock_path: Option<PathBuf>,
    pub(crate) launch_requested: bool,
    pub(crate) control_requests: Vec<AgentControlRequest>,
    pub(crate) warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SystemAgentProtocolSnapshot {
    schema: String,
    updated_at_ms: u64,
    degraded: bool,
    activities: Vec<SystemAgentProtocolActivity>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct SystemAgentProtocolActivity {
    id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    parent_id: Option<String>,
    agent: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    workspace: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    task: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reason: Option<String>,
    state: AgentActivityState,
    confidence: AgentActivityConfidence,
    vendor: AgentVendor,
    #[serde(skip_serializing_if = "Option::is_none")]
    started_at_ms: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    finished_at_ms: Option<u64>,
    expires_at_ms: u64,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    actions: Vec<AgentControlAction>,
}

#[derive(Debug, Default)]
struct PresenceScan {
    presences: Vec<AgentPresence>,
    truncated: bool,
}

impl SystemAgentProtocolSnapshot {
    fn from_collected(snapshot: &SystemAgentSnapshot, updated_at_ms: u64) -> Self {
        let activities = snapshot
            .activities
            .iter()
            .take(MAX_SYSTEM_ACTIVITIES)
            .map(|activity| SystemAgentProtocolActivity {
                id: sanitize_nonempty(&activity.id, MAX_ACTIVITY_ID_CHARS, "agent"),
                parent_id: sanitize_optional(activity.parent_id.as_deref(), MAX_ACTIVITY_ID_CHARS),
                agent: sanitize_nonempty(&activity.agent, MAX_AGENT_CHARS, "agent"),
                workspace: sanitize_optional(activity.workspace.as_deref(), MAX_WORKSPACE_CHARS),
                task: sanitize_optional(activity.task.as_deref(), MAX_TASK_CHARS),
                reason: sanitize_optional(activity.reason.as_deref(), MAX_ATTENTION_REASON_CHARS),
                state: activity.state,
                confidence: activity.confidence,
                vendor: activity.vendor,
                started_at_ms: activity.started_at_ms,
                finished_at_ms: activity.finished_at_ms,
                expires_at_ms: activity.expires_at_ms,
                actions: activity
                    .actions
                    .iter()
                    .filter_map(sanitize_control_action)
                    .take(MAX_CONTROL_ACTIONS)
                    .collect(),
            })
            .collect();
        Self {
            schema: SYSTEM_SNAPSHOT_SCHEMA.to_string(),
            updated_at_ms,
            degraded: !snapshot.warnings.is_empty()
                || snapshot.activities.len() > MAX_SYSTEM_ACTIVITIES,
            activities,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentChildPresence {
    pub(crate) id: String,
    pub(crate) agent: String,
    pub(crate) task: Option<String>,
    pub(crate) state: AgentActivityState,
    #[serde(default)]
    pub(crate) vendor: AgentVendor,
    pub(crate) started_at_ms: Option<u64>,
    #[serde(default)]
    pub(crate) finished_at_ms: Option<u64>,
    #[serde(default)]
    pub(crate) actions: Vec<AgentControlAction>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AgentPresence {
    schema: String,
    pub(crate) instance_id: String,
    pub(crate) pid: u32,
    pub(crate) workspace: String,
    pub(crate) task: Option<String>,
    #[serde(default)]
    pub(crate) reason: Option<String>,
    pub(crate) state: AgentActivityState,
    #[serde(default)]
    pub(crate) vendor: AgentVendor,
    pub(crate) children: Vec<AgentChildPresence>,
    pub(crate) started_at_ms: u64,
    #[serde(default)]
    pub(crate) finished_at_ms: Option<u64>,
    #[serde(default)]
    pub(crate) actions: Vec<AgentControlAction>,
    pub(crate) updated_at_ms: u64,
}

impl AgentPresence {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        instance_id: impl Into<String>,
        pid: u32,
        workspace: impl Into<String>,
        task: Option<String>,
        state: AgentActivityState,
        children: Vec<AgentChildPresence>,
        started_at_ms: u64,
    ) -> Self {
        let children = children
            .into_iter()
            .map(|mut child| {
                child.id = sanitize_nonempty(&child.id, MAX_CHILD_ID_CHARS, "child");
                child.agent = sanitize_nonempty(&child.agent, MAX_AGENT_CHARS, "agent");
                child
            })
            .collect();
        Self {
            schema: PRESENCE_SCHEMA.to_string(),
            instance_id: instance_id.into(),
            pid,
            workspace: workspace.into(),
            // Task text remains local and unredacted. `protocol_copy` decides
            // whether a sanitized label may cross the process boundary.
            task,
            reason: None,
            state,
            vendor: AgentVendor::Other,
            children,
            started_at_ms,
            finished_at_ms: None,
            actions: Vec::new(),
            updated_at_ms: epoch_ms(),
        }
    }

    pub(crate) fn with_vendor(mut self, vendor: AgentVendor) -> Self {
        self.vendor = vendor;
        self
    }

    pub(crate) fn with_attention_reason(mut self, reason: Option<String>) -> Self {
        self.reason = reason;
        self
    }

    pub(crate) fn with_finished_at_ms(mut self, finished_at_ms: Option<u64>) -> Self {
        self.finished_at_ms = finished_at_ms;
        self
    }

    pub(crate) fn with_actions(mut self, actions: Vec<AgentControlAction>) -> Self {
        self.actions = actions;
        self
    }

    fn protocol_copy(&self, share_tasks: bool) -> Self {
        Self {
            schema: PRESENCE_SCHEMA.to_string(),
            instance_id: self.instance_id.clone(),
            pid: self.pid,
            workspace: workspace_basename(&self.workspace),
            task: share_tasks
                .then(|| sanitize_optional(self.task.as_deref(), MAX_TASK_CHARS))
                .flatten(),
            reason: sanitize_optional(self.reason.as_deref(), MAX_ATTENTION_REASON_CHARS),
            state: self.state,
            vendor: self.vendor,
            children: self
                .children
                .iter()
                .take(MAX_CHILDREN)
                .map(|child| AgentChildPresence {
                    id: sanitize_nonempty(&child.id, MAX_CHILD_ID_CHARS, "child"),
                    agent: sanitize_nonempty(&child.agent, MAX_AGENT_CHARS, "agent"),
                    task: share_tasks
                        .then(|| sanitize_optional(child.task.as_deref(), MAX_TASK_CHARS))
                        .flatten(),
                    state: child.state,
                    vendor: child.vendor,
                    started_at_ms: child.started_at_ms,
                    finished_at_ms: child.finished_at_ms,
                    actions: sanitize_received_actions(
                        child.actions.clone(),
                        &self.instance_id,
                        self.updated_at_ms,
                    ),
                })
                .collect(),
            started_at_ms: self.started_at_ms,
            finished_at_ms: self.finished_at_ms,
            actions: sanitize_received_actions(
                self.actions.clone(),
                &self.instance_id,
                self.updated_at_ms,
            ),
            updated_at_ms: self.updated_at_ms,
        }
    }

    fn sanitize_received(&mut self) {
        self.workspace = workspace_basename(&self.workspace);
        self.task = sanitize_optional(self.task.as_deref(), MAX_TASK_CHARS);
        self.reason = sanitize_optional(self.reason.as_deref(), MAX_ATTENTION_REASON_CHARS);
        self.actions = sanitize_received_actions(
            std::mem::take(&mut self.actions),
            &self.instance_id,
            self.updated_at_ms,
        );
        self.children.truncate(MAX_CHILDREN);
        for child in &mut self.children {
            child.id = sanitize_nonempty(&child.id, MAX_CHILD_ID_CHARS, "child");
            child.agent = sanitize_nonempty(&child.agent, MAX_AGENT_CHARS, "agent");
            child.task = sanitize_optional(child.task.as_deref(), MAX_TASK_CHARS);
            child.actions = sanitize_received_actions(
                std::mem::take(&mut child.actions),
                &self.instance_id,
                self.updated_at_ms,
            );
        }
    }

    fn valid_at(&self, now_ms: u64) -> bool {
        self.schema == PRESENCE_SCHEMA
            && !self.instance_id.trim().is_empty()
            && self.updated_at_ms <= now_ms.saturating_add(PRESENCE_FUTURE_SKEW_MS)
            && now_ms.saturating_sub(self.updated_at_ms) <= PRESENCE_TTL_MS
    }

    fn stale_at(&self, now_ms: u64) -> bool {
        self.schema == PRESENCE_SCHEMA
            && !self.instance_id.trim().is_empty()
            && (now_ms.saturating_sub(self.updated_at_ms) > PRESENCE_TTL_MS
                || self.updated_at_ms > now_ms.saturating_add(PRESENCE_FUTURE_SKEW_MS))
    }
}

/// Per-process publisher identity and heartbeat file location.
#[derive(Clone, Debug)]
pub(crate) struct AgentPresencePublisher {
    instance_id: String,
    directory: Option<PathBuf>,
    path: Option<PathBuf>,
    started_at_ms: u64,
    share_tasks: bool,
    closed: Arc<AtomicBool>,
    write_lock: Arc<Mutex<()>>,
    control_grants: Arc<AgentControlGrants>,
}

impl AgentPresencePublisher {
    pub(crate) fn from_environment() -> Self {
        let directory = std::env::var_os("A3S_AGENT_STATUS_DIR")
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                a3s::components::ComponentPaths::from_env()
                    .ok()
                    .map(|paths| paths.state_root.join("code").join("agent-presence"))
            })
            .and_then(|directory| {
                absolute_status_directory(directory, std::env::current_dir().ok().as_deref())
            });
        let share_tasks = std::env::var_os("A3S_AGENT_STATUS_SHARE_TASKS")
            .is_some_and(|value| value == OsStr::new("1"));
        Self::new(directory, share_tasks)
    }

    #[cfg(test)]
    pub(crate) fn for_directory(directory: PathBuf) -> Self {
        Self::new(Some(directory), false)
    }

    #[cfg(test)]
    fn for_directory_with_task_sharing(directory: PathBuf) -> Self {
        Self::new(Some(directory), true)
    }

    fn new(directory: Option<PathBuf>, share_tasks: bool) -> Self {
        let pid = std::process::id();
        let instance_id = format!("{pid}-{:016x}", rand::random::<u64>());
        let path = directory
            .as_ref()
            .map(|directory| directory.join(format!("{instance_id}.json")));
        Self {
            instance_id,
            directory,
            path,
            started_at_ms: epoch_ms(),
            share_tasks,
            closed: Arc::new(AtomicBool::new(false)),
            write_lock: Arc::new(Mutex::new(())),
            control_grants: Arc::new(AgentControlGrants::default()),
        }
    }

    pub(crate) fn instance_id(&self) -> &str {
        &self.instance_id
    }

    pub(crate) fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    /// Reconcile the exact controls that are valid for the next heartbeat.
    ///
    /// Alternatives for one activity share one short-lived token, so accepting
    /// any one decision atomically invalidates the others. The private context
    /// never crosses the process boundary; it is returned only after a matching
    /// request consumes the grant and lets the TUI reject stale UI decisions.
    pub(crate) fn reconcile_control_grants(
        &self,
        specs: impl IntoIterator<Item = AgentControlGrantSpec>,
        now_ms: u64,
    ) -> HashMap<String, Vec<AgentControlAction>> {
        self.control_grants
            .reconcile(&self.instance_id, specs, now_ms)
    }

    /// Publish the local exact state, collect whole-system evidence, and export
    /// the native island's sanitized shared snapshot. File and process failures
    /// are partial: successfully collected rows are still exported with
    /// `degraded: true` whenever the private snapshot file can be replaced.
    pub(crate) async fn publish_collect_and_export(
        &self,
        mut local: AgentPresence,
    ) -> SystemAgentRefreshResult {
        local.updated_at_ms = epoch_ms();
        let publish = self.write_presence(&local);
        let processes = collect_processes();
        let (publish_result, process_result) = tokio::join!(publish, processes);

        let now_ms = epoch_ms();
        let mut warnings = Vec::new();
        if let Err(error) = publish_result {
            warnings.push(format!("heartbeat: {error}"));
        }

        let observed_pids = process_result.as_ref().ok().map(|processes| {
            let mut pids = processes
                .iter()
                .map(|process| process.pid)
                .collect::<HashSet<_>>();
            pids.insert(std::process::id());
            pids
        });
        let scan = match self.scan_presences(now_ms, observed_pids.as_ref()).await {
            Ok(scan) => scan,
            Err(error) => {
                warnings.push(format!("presence: {error}"));
                PresenceScan::default()
            }
        };
        if scan.truncated {
            warnings.push(format!(
                "presence: live registry exceeds the {MAX_PRESENCE_FILES}-publisher evidence limit"
            ));
        }
        let mut presences = scan.presences;
        // The current process remains exact even when its heartbeat directory
        // is unavailable or an atomic replace was briefly invisible to this
        // scan. It must cross the same privacy boundary as remote heartbeats:
        // never aggregate the TUI's full path or raw in-memory task directly.
        presences.retain(|presence| presence.instance_id != local.instance_id);
        presences.push(local.protocol_copy(self.share_tasks));

        let processes = match process_result {
            Ok(processes) => processes,
            Err(error) => {
                warnings.push(format!("processes: {error}"));
                Vec::new()
            }
        };

        let snapshot = SystemAgentSnapshot {
            activities: aggregate_activities(&presences, &processes, &self.instance_id, now_ms),
            warnings,
        };
        let mut result = SystemAgentRefreshResult {
            snapshot_path: None,
            lock_path: self
                .directory
                .as_ref()
                .map(|directory| directory.join(ISLAND_LOCK_FILE)),
            launch_requested: snapshot_requests_island_launch(&snapshot),
            control_requests: Vec::new(),
            warnings: snapshot.warnings.clone(),
        };
        match self.write_system_snapshot(&snapshot, now_ms).await {
            Ok(path) => result.snapshot_path = Some(path),
            Err(error) => result.warnings.push(format!("snapshot: {error}")),
        }
        match self.consume_control_requests(now_ms).await {
            Ok(requests) => result.control_requests = requests,
            Err(error) => result.warnings.push(format!("controls: {error}")),
        }
        result
    }

    async fn collect_read_only_snapshot(&self) -> SystemAgentSnapshot {
        let process_result = collect_processes().await;
        let now_ms = epoch_ms();
        let mut warnings = Vec::new();
        let observed_pids = process_result.as_ref().ok().map(|processes| {
            processes
                .iter()
                .map(|process| process.pid)
                .collect::<HashSet<_>>()
        });
        let scan = match self.scan_presences(now_ms, observed_pids.as_ref()).await {
            Ok(scan) => scan,
            Err(error) => {
                warnings.push(format!("presence: {error}"));
                PresenceScan::default()
            }
        };
        if scan.truncated {
            warnings.push(format!(
                "presence: live registry exceeds the {MAX_PRESENCE_FILES}-publisher evidence limit"
            ));
        }
        let processes = match process_result {
            Ok(processes) => processes,
            Err(error) => {
                warnings.push(format!("processes: {error}"));
                Vec::new()
            }
        };
        SystemAgentSnapshot {
            activities: aggregate_activities(&scan.presences, &processes, "", now_ms),
            warnings,
        }
    }

    pub(crate) async fn remove(&self) {
        self.closed.store(true, Ordering::Release);
        self.control_grants.clear();
        let _write = self.write_lock.lock().await;
        if let Some(path) = &self.path {
            let _ = tokio::fs::remove_file(path).await;
        }
    }

    async fn write_presence(&self, presence: &AgentPresence) -> anyhow::Result<()> {
        let _write = self.write_lock.lock().await;
        if self.closed.load(Ordering::Acquire) {
            anyhow::bail!("publisher is closed");
        }
        let (Some(directory), Some(path)) = (&self.directory, &self.path) else {
            anyhow::bail!("no per-user A3S state directory is available");
        };
        if presence.instance_id != self.instance_id || presence.pid != std::process::id() {
            anyhow::bail!("heartbeat identity does not match its publisher");
        }
        ensure_private_directory(directory)
            .await
            .with_context(|| "secure the agent-presence directory")?;

        let presence = presence.protocol_copy(self.share_tasks);
        let bytes = serde_json::to_vec(&presence)?;
        if bytes.len() as u64 > MAX_PRESENCE_BYTES {
            anyhow::bail!("heartbeat exceeds the protocol size limit");
        }
        let temporary = directory.join(format!(
            ".{}-{:016x}.tmp",
            self.instance_id,
            rand::random::<u64>()
        ));
        let result = async {
            let mut options = tokio::fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options
                .open(&temporary)
                .await
                .with_context(|| "create a private heartbeat temporary file")?;
            file.write_all(&bytes)
                .await
                .with_context(|| "write the heartbeat temporary file")?;
            file.flush()
                .await
                .with_context(|| "flush the heartbeat temporary file")?;
            drop(file);

            if self.closed.load(Ordering::Acquire) {
                anyhow::bail!("publisher closed during heartbeat write");
            }
            replace_presence_file(&temporary, path)
                .await
                .with_context(|| "replace the published heartbeat")
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result
    }

    async fn write_system_snapshot(
        &self,
        snapshot: &SystemAgentSnapshot,
        evidence_at_ms: u64,
    ) -> anyhow::Result<PathBuf> {
        let _write = self.write_lock.lock().await;
        if self.closed.load(Ordering::Acquire) {
            anyhow::bail!("publisher is closed");
        }
        let Some(directory) = &self.directory else {
            anyhow::bail!("no per-user A3S state directory is available");
        };
        ensure_private_directory(directory)
            .await
            .with_context(|| "secure the agent-presence directory")?;

        let snapshot = SystemAgentProtocolSnapshot::from_collected(snapshot, evidence_at_ms);
        let bytes = serde_json::to_vec(&snapshot)?;
        if bytes.len() as u64 > MAX_SYSTEM_SNAPSHOT_BYTES {
            anyhow::bail!("system-agent snapshot exceeds the protocol size limit");
        }

        let path = directory.join(SYSTEM_SNAPSHOT_FILE);
        let temporary = directory.join(format!(
            ".system-snapshot-{:016x}.tmp",
            rand::random::<u64>()
        ));
        let result = async {
            let mut options = tokio::fs::OpenOptions::new();
            options.create_new(true).write(true);
            #[cfg(unix)]
            options.mode(0o600);
            let mut file = options
                .open(&temporary)
                .await
                .with_context(|| "create a private snapshot temporary file")?;
            file.write_all(&bytes)
                .await
                .with_context(|| "write the snapshot temporary file")?;
            file.flush()
                .await
                .with_context(|| "flush the snapshot temporary file")?;
            drop(file);
            if self.closed.load(Ordering::Acquire) {
                anyhow::bail!("publisher closed during snapshot write");
            }
            replace_presence_file(&temporary, &path)
                .await
                .with_context(|| "replace the shared system-agent snapshot")
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&temporary).await;
        }
        result.map(|()| path)
    }

    #[cfg(test)]
    async fn read_presences(&self, now_ms: u64) -> anyhow::Result<Vec<AgentPresence>> {
        Ok(self.scan_presences(now_ms, None).await?.presences)
    }

    async fn scan_presences(
        &self,
        now_ms: u64,
        observed_pids: Option<&HashSet<u32>>,
    ) -> anyhow::Result<PresenceScan> {
        let Some(directory) = &self.directory else {
            return Ok(PresenceScan::default());
        };
        let mut entries = match tokio::fs::read_dir(directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PresenceScan::default())
            }
            Err(error) => return Err(error.into()),
        };
        let mut presences = Vec::new();
        let mut truncated = false;
        let mut entries_seen = 0usize;
        let mut stale_removal_attempts = 0usize;
        while let Some(entry) = entries.next_entry().await? {
            if entries_seen == MAX_REGISTRY_ENTRIES {
                anyhow::bail!("agent-presence registry exceeds the directory entry limit");
            }
            entries_seen += 1;

            let path = entry.path();
            let Some(identity) = parse_presence_file_name(&entry.file_name()) else {
                continue;
            };
            let Ok(file_type) = entry.file_type().await else {
                continue;
            };
            if !file_type.is_file() {
                continue;
            }
            let Ok(metadata) = entry.metadata().await else {
                continue;
            };
            if !metadata.is_file() || metadata.len() > MAX_PRESENCE_BYTES {
                continue;
            }
            let Some(mut presence) = read_presence_file(&path).await else {
                continue;
            };
            if presence.instance_id != identity.instance_id || presence.pid != identity.pid {
                continue;
            }
            if presence.stale_at(now_ms)
                && observed_pids.is_some_and(|pids| !pids.contains(&presence.pid))
            {
                if stale_removal_attempts < MAX_STALE_REMOVALS_PER_SCAN {
                    stale_removal_attempts += 1;
                    let _ = remove_stale_presence(&path, &identity, now_ms, observed_pids.unwrap())
                        .await;
                }
                continue;
            }
            if !presence.valid_at(now_ms) {
                continue;
            }
            presence.sanitize_received();
            if presences.len() == MAX_PRESENCE_FILES {
                // Continue validating and counting after the evidence cap so a
                // live publisher flood cannot look complete. The caller keeps
                // the bounded rows and marks the exported snapshot degraded.
                truncated = true;
            } else {
                presences.push(presence);
            }
        }
        Ok(PresenceScan {
            presences,
            truncated,
        })
    }

    async fn consume_control_requests(
        &self,
        now_ms: u64,
    ) -> anyhow::Result<Vec<AgentControlRequest>> {
        self.control_grants
            .consume_requests(self.directory.as_deref(), &self.instance_id, now_ms)
            .await
    }
}

/// Collect sanitized cooperative heartbeat and inferred process evidence
/// without publishing a heartbeat or writing a shared snapshot.
pub(crate) async fn collect_system_agent_snapshot() -> SystemAgentSnapshot {
    AgentPresencePublisher::from_environment()
        .collect_read_only_snapshot()
        .await
}

fn absolute_status_directory(directory: PathBuf, current_dir: Option<&Path>) -> Option<PathBuf> {
    if directory.is_absolute() {
        Some(directory)
    } else {
        current_dir.map(|current_dir| current_dir.join(directory))
    }
}

#[derive(Debug, PartialEq, Eq)]
struct PresenceFileIdentity {
    instance_id: String,
    pid: u32,
}

fn parse_presence_file_name(file_name: &OsStr) -> Option<PresenceFileIdentity> {
    let file_name = file_name.to_str()?;
    let instance_id = file_name.strip_suffix(".json")?;
    let (pid, nonce) = instance_id.split_once('-')?;
    if pid.is_empty() || nonce.len() != 16 || !nonce.bytes().all(|byte| byte.is_ascii_hexdigit()) {
        return None;
    }
    let parsed_pid = pid.parse::<u32>().ok()?;
    if parsed_pid == 0 || pid != parsed_pid.to_string() {
        return None;
    }
    Some(PresenceFileIdentity {
        instance_id: instance_id.to_string(),
        pid: parsed_pid,
    })
}

async fn read_presence_file(path: &Path) -> Option<AgentPresence> {
    let file = tokio::fs::OpenOptions::new()
        .read(true)
        .open(path)
        .await
        .ok()?;
    let metadata = file.metadata().await.ok()?;
    if !metadata.is_file() || metadata.len() > MAX_PRESENCE_BYTES {
        return None;
    }
    let mut bytes = Vec::with_capacity(metadata.len() as usize);
    file.take(MAX_PRESENCE_BYTES + 1)
        .read_to_end(&mut bytes)
        .await
        .ok()?;
    if bytes.len() as u64 > MAX_PRESENCE_BYTES {
        return None;
    }
    serde_json::from_slice(&bytes).ok()
}

async fn remove_stale_presence(
    path: &Path,
    identity: &PresenceFileIdentity,
    now_ms: u64,
    observed_pids: &HashSet<u32>,
) -> bool {
    let Ok(metadata) = tokio::fs::symlink_metadata(path).await else {
        return false;
    };
    if !metadata.file_type().is_file() {
        return false;
    }
    let Some(presence) = read_presence_file(path).await else {
        return false;
    };
    if presence.instance_id != identity.instance_id
        || presence.pid != identity.pid
        || !presence.stale_at(now_ms)
        || observed_pids.contains(&presence.pid)
    {
        return false;
    }
    tokio::fs::remove_file(path).await.is_ok()
}

pub(crate) fn epoch_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

#[cfg(unix)]
async fn ensure_private_directory(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut builder = tokio::fs::DirBuilder::new();
    builder.recursive(true).mode(0o700);
    builder.create(path).await?;
    tokio::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).await
}

#[cfg(not(unix))]
async fn ensure_private_directory(path: &Path) -> std::io::Result<()> {
    tokio::fs::create_dir_all(path).await
}

#[cfg(not(windows))]
async fn replace_presence_file(temporary: &Path, path: &Path) -> std::io::Result<()> {
    tokio::fs::rename(temporary, path).await
}

#[cfg(windows)]
async fn replace_presence_file(temporary: &Path, path: &Path) -> std::io::Result<()> {
    let temporary = temporary.to_path_buf();
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || replace_file_windows(&temporary, &path))
        .await
        .map_err(|error| std::io::Error::other(format!("atomic replace task failed: {error}")))?
}

#[cfg(windows)]
fn replace_file_windows(temporary: &Path, path: &Path) -> std::io::Result<()> {
    use std::iter::once;
    use std::os::windows::ffi::OsStrExt;
    use windows_sys::Win32::Storage::FileSystem::{
        MoveFileExW, MOVEFILE_REPLACE_EXISTING, MOVEFILE_WRITE_THROUGH,
    };

    let temporary = temporary
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    let path = path
        .as_os_str()
        .encode_wide()
        .chain(once(0))
        .collect::<Vec<_>>();
    // SAFETY: both pointers reference NUL-terminated UTF-16 buffers that stay
    // alive for the call. The files share a private directory/volume, and the
    // replace flags preserve the previous complete file until the move wins.
    let replaced = unsafe {
        MoveFileExW(
            temporary.as_ptr(),
            path.as_ptr(),
            MOVEFILE_REPLACE_EXISTING | MOVEFILE_WRITE_THROUGH,
        )
    };
    if replaced == 0 {
        Err(std::io::Error::last_os_error())
    } else {
        Ok(())
    }
}

#[cfg(test)]
#[path = "system_agents_tests.rs"]
mod tests;
