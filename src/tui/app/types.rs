//! Shared application messages, rebuild state, and asynchronous runtime types.

use super::*;

/// Shared, single-consumer receiver for the active agent run. Wrapped so the
/// pump command can own a clone; pumps run sequentially, so the mutex never
/// actually contends.
pub(super) type SharedRx = Arc<Mutex<mpsc::Receiver<AgentEvent>>>;
pub(super) type SharedDeepResearchRx = Arc<Mutex<mpsc::Receiver<CodeDeepResearchEvent>>>;
pub(super) type SharedManifestRx =
    Arc<Mutex<tokio::sync::broadcast::Receiver<LocalWorkspaceManifestSnapshot>>>;
pub(super) type SharedActiveSession = Arc<std::sync::Mutex<Arc<AgentSession>>>;
pub(super) type StreamJoin = tokio::task::JoinHandle<()>;
pub(super) type HostToolAbort = tokio::task::AbortHandle;

#[derive(Clone)]
pub(super) struct LlmTurnUiCheckpoint {
    pub(super) turn: usize,
    pub(super) transcript_len: usize,
    pub(super) streaming: StreamingMarkdown,
    pub(super) thinking: String,
    pub(super) turn_text: String,
    pub(super) got_delta: bool,
    pub(super) turn_had_agent_activity: bool,
    pub(super) turn_text_after_activity: bool,
    pub(super) runtime_tools: RuntimeToolCheckpoint,
}

#[derive(Clone, Copy, PartialEq)]
pub(super) enum State {
    Idle,
    Streaming,
    Awaiting,
    Rebuilding,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum InterruptedContinuation {
    DrainQueue,
    RestoreGoalMode,
    SettleDeepResearch,
}

#[derive(Clone, Copy, Debug)]
pub(super) enum ViewportAnchor {
    Bottom,
    Transcript(TranscriptAnchor),
    Absolute(usize),
}

#[derive(Clone)]
#[allow(clippy::enum_variant_names)]
pub(super) enum Action {
    ScrollUp,
    ScrollDown,
    ScrollTop,
    ScrollBottom,
}

/// Set by `/update` when an upgrade is available: after the TUI exits (terminal
/// restored), `run` performs the upgrade (Homebrew or standalone download) and
/// re-execs the freshly-installed binary.
pub(super) static UPGRADE_ON_EXIT: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);
/// The latest version tag, stashed by `/update` for the post-exit upgrade.
pub(super) static LATEST: std::sync::Mutex<Option<String>> = std::sync::Mutex::new(None);

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AutoReviewKey {
    pub(super) session_id: String,
    pub(super) revision: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AutoReviewTicket {
    pub(super) id: u64,
    pub(super) key: AutoReviewKey,
}

#[derive(Debug)]
pub(super) struct AutoReviewTracker {
    pub(super) revision: u64,
    pub(super) reviewed: Option<AutoReviewKey>,
    pub(super) inflight: Option<AutoReviewTicket>,
    pub(super) next_ticket_id: u64,
}

impl AutoReviewTracker {
    pub(super) fn new(revision: u64) -> Self {
        Self {
            revision,
            reviewed: None,
            inflight: None,
            next_ticket_id: 0,
        }
    }

    pub(super) fn on_user_turn(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    pub(super) fn current_key(&self, session_id: &str) -> AutoReviewKey {
        AutoReviewKey {
            session_id: session_id.to_string(),
            revision: self.revision,
        }
    }

    pub(super) fn current_is_reviewed(&self, session_id: &str) -> bool {
        self.reviewed
            .as_ref()
            .is_some_and(|key| key.session_id == session_id && key.revision == self.revision)
    }

    /// Mark the current conversation revision as considered and, when it has a
    /// real user turn, issue a unique ticket for the asynchronous review.
    pub(super) fn begin(
        &mut self,
        session_id: &str,
        has_user_turn: bool,
    ) -> Option<AutoReviewTicket> {
        let key = self.current_key(session_id);
        if self.reviewed.as_ref() == Some(&key) {
            return None;
        }
        self.reviewed = Some(key.clone());
        if !has_user_turn {
            return None;
        }

        self.next_ticket_id = self.next_ticket_id.wrapping_add(1);
        let ticket = AutoReviewTicket {
            id: self.next_ticket_id,
            key,
        };
        // A newer conversation may replace an older in-flight ticket. The old
        // result will fail the exact-ticket check in `accept` and cannot clear it.
        self.inflight = Some(ticket.clone());
        Some(ticket)
    }

    pub(super) fn accept(&mut self, ticket: &AutoReviewTicket, session_id: &str) -> bool {
        if self.inflight.as_ref() != Some(ticket) {
            return false;
        }
        self.inflight = None;
        ticket.key.session_id == session_id
            && ticket.key.revision == self.revision
            && self.reviewed.as_ref() == Some(&ticket.key)
    }
}

pub(super) fn auto_review_history_has_user_turn(history: &[Message]) -> bool {
    history
        .iter()
        .any(|message| message.role == "user" && !message.text().trim().is_empty())
}

pub(super) enum SessionRebuildAction {
    Model {
        model: String,
        source: ModelSelectionSource,
        llm_override: Option<LlmOverride>,
        context_limit: u32,
    },
    Effort {
        selected: usize,
        codex_effort: Option<CodexEffortStatus>,
    },
    GoalStart {
        generation: u64,
        previous_effort: usize,
        previous_goal: Option<String>,
        previous_goal_since: Option<Instant>,
    },
    GoalResume {
        generation: u64,
        paused: PausedGoalState,
    },
    GoalRestore,
    Compact {
        summary: String,
        session_id: String,
    },
    Fork {
        session_id: String,
    },
    Rewind {
        session_id: String,
        files_rewound: bool,
        warning: Option<String>,
    },
    Relay {
        restore: RelayRestoreState,
    },
    Clear {
        session_id: String,
    },
    Reload {
        skill_count: usize,
    },
    EvolutionRuntime {
        pending_assets: usize,
        stream_token: u64,
        synthesis: Option<(String, String)>,
    },
    Refresh {
        failure_context: Option<&'static str>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct WorktreeForkResult {
    pub(super) session_id: String,
    pub(super) workspace: PathBuf,
    pub(super) worktree_root: PathBuf,
    pub(super) branch: String,
}

#[derive(Clone, Debug)]
pub(super) struct RewindCheckpointSeed {
    pub(super) source_session_id: String,
    pub(super) task: String,
    pub(super) history_before: Vec<Message>,
    pub(super) session_before: Option<a3s_code_core::store::SessionData>,
    pub(super) git_before: Option<GitTreeSnapshot>,
    pub(super) warning: Option<String>,
}

#[derive(Clone, Debug)]
pub(super) struct RewindCheckpoint {
    pub(super) id: u64,
    pub(super) source_session_id: String,
    pub(super) task: String,
    pub(super) history_before: Vec<Message>,
    pub(super) session_before: Option<a3s_code_core::store::SessionData>,
    pub(super) file_patch: Option<GitBinaryPatch>,
    pub(super) repository_root: Option<PathBuf>,
    pub(super) warning: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RewindResult {
    pub(super) checkpoint_id: u64,
    pub(super) session_id: String,
    pub(super) files_rewound: bool,
    pub(super) warning: Option<String>,
}

pub(super) struct SessionRebuildProfile {
    pub(super) session_id: String,
    pub(super) model: Option<String>,
    pub(super) effort: usize,
    pub(super) context_limit: u32,
    pub(super) llm_override: Option<LlmOverride>,
    pub(super) compact_summary: Option<String>,
}

pub(super) struct RelayRestoreState {
    pub(super) session_id: String,
    pub(super) model: Option<String>,
    pub(super) model_source: ModelSelectionSource,
    pub(super) effort: usize,
    pub(super) mode: Mode,
    pub(super) context_limit: u32,
    pub(super) llm_override: Option<LlmOverride>,
    pub(super) theme: Option<usize>,
    pub(super) paused_goal: Option<PausedGoalState>,
}

pub(super) struct IdeIntelligenceResult {
    pub(super) title: String,
    pub(super) rows: Vec<IdeIntelligenceRow>,
    pub(super) truncated: bool,
    pub(super) saved_version: bool,
    pub(super) dirty_buffer: bool,
    pub(super) stale: bool,
    pub(super) workspace_revision: Option<u64>,
}

pub(super) struct IdeIntelligenceJump {
    pub(super) path: PathBuf,
    pub(super) lines: Vec<String>,
    pub(super) row: usize,
    pub(super) col: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum SessionRebuildMode {
    /// Reconfigure an existing persisted session without ever replacing a
    /// failed resume with an empty session using the same id.
    ResumeExisting,
    /// Materialize a deliberately new id for `/clear` or `/compact`.
    CreateFresh,
}

pub(super) enum Msg {
    Term(Event),
    // Boxed: AgentEvent is large; keeps the Msg enum small.
    Agent {
        source: SharedRx,
        event: Box<AgentEvent>,
    },
    Submit(String),
    /// Cancel the active turn and run this immutable submission before normal
    /// queued follow-ups.
    SubmitNow(String),
    StreamStarted {
        token: u64,
        session: Arc<AgentSession>,
        rx: SharedRx,
        join: StreamJoin,
        /// Temporary previews retained until stream admission succeeds.
        submitted_images: Vec<PendingImage>,
        /// Pre-turn conversation and Git state captured before admission.
        rewind_checkpoint: Option<RewindCheckpointSeed>,
    },
    StreamEnded(SharedRx),
    StreamJoinSettled {
        token: u64,
        synthesis: Option<(String, String)>,
    },
    DiscardedStreamSettled {
        token: u64,
    },
    /// Session cancellation and the active stream worker have settled enough
    /// for the terminal program to restore the shell without detaching work.
    QuitReady,
    StreamError {
        token: u64,
        error: String,
        /// Admission can be retried without duplicating a persisted user turn.
        retryable_admission: bool,
        /// Restored to the composer when stream admission fails.
        submitted_images: Vec<PendingImage>,
    },
    /// Bounded backoff before retrying a queued turn that raced Core's
    /// single-flight lease release.
    QueueRetry {
        generation: u64,
    },
    WorkspaceManifest(Box<LocalWorkspaceManifestSnapshot>),
    WorkspaceManifestStopped,
    IdeIntelligenceCompleted {
        request_id: u64,
        result: Result<IdeIntelligenceResult, String>,
    },
    IdeIntelligenceJumpCompleted {
        request_id: u64,
        jump_request_id: u64,
        result: Result<IdeIntelligenceJump, String>,
    },
    LivePreviewLaunched {
        request_id: u64,
        status_entry: TranscriptEntryId,
        result: Box<Result<app_live_preview::LivePreviewLaunch, String>>,
    },
    SpinnerTick,
    /// Advance Codex-style Markdown commit animation independently from the
    /// slower status spinner.
    StreamCommitTick,
    /// Advance the welcome-mascot animation frame.
    BannerTick,
    /// Refresh the independent whole-system coding-agent collector/exporter.
    AgentPresenceTick,
    /// Completion of an exact heartbeat plus sanitized shared snapshot export.
    AgentPresenceRefreshed(crate::system_agents::SystemAgentRefreshResult),
    /// Best-effort native system-island helper launch result.
    AgentIslandLaunchFinished(Result<AgentIslandLaunchOutcome, String>),
    /// One validated, one-shot inline decision submitted by the native island.
    AgentIslandControl(crate::system_agents::AgentControlRequest),
    /// Result of routing a child-row cancellation to Core's real task tracker.
    AgentIslandSubagentCancelFinished {
        task_id: String,
        cancelled: bool,
    },
    /// Drive the short, high-frame-rate Ultracode activation transition.
    UltracodeTick {
        epoch: u64,
    },
    ModalConfirm {
        tool_id: String,
        approved: bool,
        reason: Option<String>,
    },
    PersistProjectPermission {
        tool_id: String,
        grant: ExactPermissionGrant,
    },
    ProjectPermissionPersisted {
        tool_id: String,
        grant: ExactPermissionGrant,
        result: Result<PathBuf, String>,
    },
    /// Result of an explicitly confirmed project-grant revocation.
    ProjectPermissionRevoked {
        request_id: u64,
        stable_key: String,
        result: Result<ProjectPermissionRevocation, String>,
    },
    /// Authoritative delegated-task snapshot for an open `/tasks` panel.
    TaskPanelData {
        session_id: String,
        generation: u64,
        request_id: u64,
        tasks: Vec<a3s_code_core::SubagentTaskSnapshot>,
    },
    /// Periodic in-memory refresh scoped to one `/tasks` panel generation.
    TaskPanelTick {
        generation: u64,
    },
    /// Result of the panel's explicitly confirmed task cancellation.
    TaskPanelCancelFinished {
        session_id: String,
        generation: u64,
        task_id: String,
        cancelled: bool,
    },
    BackgroundSubagentFinished {
        session_id: String,
        generation: u64,
        task_id: String,
        agent: String,
        output: String,
        outcome: SubagentOutcome,
        finished_ms: u64,
    },
    BackgroundSubagentWatchStopped {
        session_id: String,
        generation: u64,
        task_id: String,
    },
    SubagentSnapshots {
        session_id: String,
        generation: u64,
        request_id: u64,
        snapshots: Vec<RestoredSubagentSnapshot>,
    },
    /// The active DeepResearch parent reached a report terminal state. Its
    /// children must be terminal before the report view opens and autonomy is
    /// restored, otherwise the footer advertises work after the parent ended.
    DeepResearchSubagentsSettled {
        session_id: String,
        generation: u64,
        exit: DeepResearchSettlementExit,
        settlements: Vec<DeepResearchSubagentSettlement>,
    },
    DeepResearchJournalFinalized {
        run_id: String,
        exit: DeepResearchSettlementExit,
        result: Result<ResearchRunProjection, String>,
    },
    DeepResearchJournalEventRecorded {
        run_id: String,
        result: Result<ResearchRunProjection, String>,
    },
    Resume,
    Interrupted {
        goal_cancelled: bool,
        status_entry: TranscriptEntryId,
    },
    /// Output of a `!`-prefixed shell command.
    ShellOutput(String),
    /// Atomic, no-clobber Markdown export of the current semantic transcript.
    SessionExported {
        status_entry: TranscriptEntryId,
        result: Result<(PathBuf, u64), String>,
    },
    /// Live A3S Use capability projection and provider readiness.
    UseStatus {
        status_entry: TranscriptEntryId,
        text: String,
    },
    /// Typed, secret-free host inspection completed before `/checkup` starts
    /// its strict read-only workspace audit.
    CheckupPreflightCompleted {
        status_entry: TranscriptEntryId,
        result: Result<panels::checkup::CheckupPreflight, String>,
    },
    ResearchDiagnostic(Result<String, String>),
    /// The isolated typed DeepResearch runtime and its event stream are ready.
    DeepResearchRunStarted {
        run_id: String,
        result: Result<(CodeDeepResearchRunHandle, SharedDeepResearchRx), String>,
    },
    /// One typed engine or nested-agent event from the isolated research run.
    DeepResearchRunEvent {
        source: SharedDeepResearchRx,
        event: Box<CodeDeepResearchEvent>,
    },
    /// The typed event stream closed before the completion watcher fired.
    DeepResearchRunEventsEnded {
        source: SharedDeepResearchRx,
    },
    /// The root research task reached a terminal state and is ready to join.
    DeepResearchRunReady {
        run_id: String,
    },
    /// The root task and isolated session have both settled.
    DeepResearchRunSettled {
        run_id: String,
        result: Result<CodeDeepResearchRunExit, String>,
    },
    /// A stale asynchronously-started run was cancelled and joined.
    DeepResearchRunDiscarded {
        run_id: String,
    },
    /// `/update` version check finished: the latest version tag, if reachable.
    UpdatePlan(Option<String>),
    /// OS login completed.
    OsLogin {
        status_entry: TranscriptEntryId,
        result: Result<String, String>,
    },
    /// Post-login SSH-key sync finished (registers the local pubkey with OS).
    SshKeySynced(crate::a3s_os::SshKeyOutcome),
    /// OS access token was refreshed (or refresh failed) in the background.
    OsRefreshed(Result<crate::a3s_os::StoredOsSession, String>),
    /// OS unified-gateway model ids fetched for the `/model` picker.
    OsGatewayModels {
        login_at_ms: u64,
        result: Result<Vec<crate::a3s_os::GatewayModel>, String>,
    },
    /// Models discovered from a detected local developer-tool account.
    AccountModels {
        provider: crate::account_providers::AccountProvider,
        result: Result<Vec<String>, String>,
    },
    /// Host-owned continuation for an active `/goal`. The generation makes a
    /// delayed retry inert after Esc, `/goal clear`, or a replacement goal.
    GoalContinue {
        generation: u64,
        prompt: String,
    },
    /// A streaming `/goal clear` finished cancelling and joining the old run.
    GoalCleared,
    /// Picker-visible models refreshed through the signed-in Codex CLI.
    CodexModels(Result<Vec<crate::account_providers::codex::CodexModel>, String>),
    /// An async session rebuild for `/model`, `/effort`, or another
    /// session-mutating TUI action completed.
    SessionRebuilt {
        request_id: u64,
        action: SessionRebuildAction,
        result: Box<panels::model::SessionRebuildResult>,
    },
    /// `/fork` copied the session under a new id (Ok) — swap the active session to
    /// it — or failed (Err with a reason).
    Forked {
        request_id: u64,
        result: Result<String, String>,
    },
    /// `/fork worktree` created an isolated Git worktree and copied the complete
    /// session into its workspace-local store. The current TUI stays attached
    /// to its original workspace.
    WorktreeForked {
        request_id: u64,
        result: Result<WorktreeForkResult, String>,
    },
    /// Post-turn Git capture finished and queued continuation may proceed.
    RewindCheckpointFinalized {
        token: u64,
        checkpoint: RewindCheckpoint,
        synthesis: Option<(String, String)>,
    },
    /// `/rewind` created a pre-turn session fork and safely reversed its file
    /// patch, or refused without changing files.
    Rewound {
        request_id: u64,
        result: Result<RewindResult, String>,
    },
    /// A background scan of native and external coding-agent transcripts.
    RelayData {
        request_id: u64,
        result: Result<Vec<panels::relay::RelaySession>, String>,
    },
    /// Periodic `/relay` refresh scoped to one open-panel generation.
    RelayRefreshTick {
        generation: u64,
    },
    /// `/memory` graph data loaded (timeline + details + derived graph).
    MemoryLoaded(MemPanelData),
    /// A `/memory` forget-candidate deletion finished, with fresh graph data.
    MemoryForgotten(Result<(String, MemPanelData), String>),
    /// `/evolution` loaded the current memory-derived candidate catalog.
    EvolutionLoaded(Result<crate::evolution::EvolutionOverview, String>),
    /// A candidate review/materialize/rollback action completed.
    EvolutionMutated(Result<panels::evolution::EvolutionUiMutation, String>),
    /// Post-turn check for automatically materialized session assets. This
    /// remains a queue barrier until a required session rebuild has settled.
    EvolutionRuntimeChecked {
        stream_token: u64,
        synthesis: Option<(String, String)>,
        result: Result<usize, String>,
    },
    /// A rebuilt session loaded materialized local skills.
    EvolutionSkillsActivated(Result<usize, String>),
    /// Asset-scoped OS asset list loaded.
    AssetListLoaded(Result<panels::asset_resources::AssetListFetch, String>),
    /// Runtime activity rows loaded for an asset-scoped activity panel.
    RuntimeActivityLoaded(Result<panels::asset_resources::RuntimeActivityFetch, String>),
    /// `/kb import` finished; carries the one-line summary to show.
    KbAdded(String),
    /// `/ctx <query>` finished: raw `ctx search --json` stdout (or the error).
    CtxResults {
        status_entry: TranscriptEntryId,
        result: Result<String, String>,
    },
    /// `/ctx <n>` finished: (hit title, transcript window) to stage as context.
    CtxWindow {
        status_entry: TranscriptEntryId,
        result: Result<(String, String), String>,
    },
    /// `/ctx save <n>` finished: Ok(hit title) once written to the memory store.
    CtxSaved(Result<String, String>),
    /// `/sleep` finished persisting its consolidated memories (count on Ok).
    SleepSaved(Result<usize, String>),
    /// `/flow` published/opened/inspected an OS Workflow as a Service asset.
    FlowOsCompleted {
        status_entry: TranscriptEntryId,
        result: Result<panels::flow::FlowOsResult, String>,
    },
    /// `/agent` published/opened an OS agent asset through Agent as a Service or Function as a Service.
    AgentOsCompleted {
        status_entry: TranscriptEntryId,
        result: Result<panels::agent::AgentOsResult, String>,
    },
    /// `/mcp` published/ran/tested an OS Function as a Service MCP asset.
    McpOsCompleted {
        status_entry: TranscriptEntryId,
        result: Result<panels::mcp::McpOsResult, String>,
    },
    /// `/skill` published/deployed/inspected an OS Function as a Service skill asset.
    SkillOsCompleted {
        status_entry: TranscriptEntryId,
        result: Result<panels::skill::SkillOsResult, String>,
    },
    /// `/okf` published/deployed an OS Knowledge service package asset.
    OkfOsCompleted {
        status_entry: TranscriptEntryId,
        result: Result<panels::okf::OkfOsResult, String>,
    },
    /// Asset source was cloned into the local asset workspace.
    AssetCloned {
        status_entry: TranscriptEntryId,
        result: Result<asset_clone::AssetCloneResult, String>,
    },
    /// `/memory` → ctx back-jump finished: (ctx event id, transcript window).
    CtxMemorySource(Result<(String, String), String>),
    /// Inactivity auto-review summary text, tagged so stale background results
    /// cannot appear after a new turn, `/clear`, compact, or fork.
    AutoReview {
        ticket: AutoReviewTicket,
        text: String,
    },
    /// `/compact` completed its direct, tool-free summary request.
    Compacted(Result<Option<String>, String>),
    /// Startup update check completed with the latest published version (if any).
    UpdateCheck(Option<String>),
}

pub(super) struct RestoredSubagentSnapshot {
    pub(super) snapshot: a3s_code_core::SubagentTaskSnapshot,
    pub(super) parent_result_expected: bool,
}

pub(super) struct DeepResearchSubagentSettlement {
    pub(super) task_id: String,
    pub(super) agent: String,
    pub(super) output: String,
    pub(super) outcome: SubagentOutcome,
    pub(super) finished_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum DeepResearchSettlementExit {
    ReportReady,
    Interrupted,
}

impl DeepResearchSettlementExit {
    pub(super) fn opens_report(self) -> bool {
        matches!(self, Self::ReportReady)
    }
}

impl From<Event> for Msg {
    fn from(event: Event) -> Self {
        // Ctrl+C is handled in the key loop as a global graceful quit key.
        Msg::Term(event)
    }
}
