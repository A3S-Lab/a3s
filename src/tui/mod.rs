//! Codex-style terminal UI for the A3S Code agent.
//!
//! Built on the `a3s-tui` TEA framework: it drives an [`AgentSession`] via
//! `session.stream()` and renders the resulting [`AgentEvent`] stream as a live
//! chat transcript, with a scoped approval prompt for tool calls.
//!
//! Streaming bridge: `session.stream()` yields a `tokio::mpsc` receiver. A
//! self-re-issuing "pump" command reads one event, turns it into a `Msg`, and
//! the update handler issues the next pump — feeding the async event stream into
//! the synchronous TEA update loop one event at a time.

use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use a3s_code_core::config::{CodeConfig, OsConfig};
use a3s_code_core::context::RecentWorkspaceFilesContextProvider;
#[cfg(test)]
use a3s_code_core::dynamic_workflow_store_path;
use a3s_code_core::hitl::TimeoutAction;
use a3s_code_core::llm::{ContentBlock, Message};
use a3s_code_core::workspace::{
    LocalWorkspaceManifest, LocalWorkspaceManifestSnapshot, ManifestWorkspaceBackend,
    WorkspaceServices,
};
use a3s_code_core::{
    Agent, AgentEvent, AgentSession, CodeDiagnosticSeverity, CodeError,
    CodeIntelligenceCapabilities, CodeIntelligenceState, CodeLocation, CodePosition,
    CodeSymbolKind, DocumentSymbol, LocalCodeIntelligence, NavigationKind, SessionOptions,
    SymbolInformation, SystemPromptSlots, ToolCallResult, WorkspaceCodeIntelligence,
};
use a3s_lane::{PriorityItem, PriorityQueue};
use a3s_tui::cmd::{self, Cmd};
use a3s_tui::components::textarea::TextareaMsg;
use a3s_tui::components::viewport::ViewportMsg;
use a3s_tui::components::{
    Alert, AlertKind, DiffLineKind, DiffSpan, InlineAction, Meter, Scrollbar, SessionStatusChip,
    Spinner, Textarea, Toast, ToastKind, Viewport,
};
use a3s_tui::event::{KeyEvent, MouseEvent};
use a3s_tui::keymap::{KeyBinding, Keymap};
use a3s_tui::layout::{Constraint, Layout};
use a3s_tui::style::{Color, Style};
use a3s_tui::{
    AgentChrome, Event, KeyCode, KeyModifiers, Model, ProgramBuilder, Theme as TuiTheme,
};
use tokio::sync::{mpsc, Mutex};

use a3s_deep_research::engine::{
    DeepResearchEvent, DeepResearchLifecycle, EvidenceScope, PublicationOutcome, ResearchStage,
};

// Team digital assets.
#[path = "assets/clone.rs"]
pub(crate) mod asset_clone;
#[path = "assets/lifecycle.rs"]
pub(crate) mod asset_lifecycle;
use crate::commands::code::naming as asset_naming;
use crate::research::{
    build_code_deep_research_request, CodeDeepResearchEvent, CodeDeepResearchLaunch,
    CodeDeepResearchRunExit, CodeDeepResearchRunHandle, CodeDeepResearchRunner,
    CodeDeepResearchRunnerBudget,
};

// DeepResearch.
#[path = "deep_research/artifacts.rs"]
mod deep_research_artifacts;
#[path = "deep_research/convergence.rs"]
mod deep_research_convergence;
#[path = "deep_research/evidence_ledger.rs"]
mod deep_research_evidence_ledger;
#[path = "deep_research/host_digest.rs"]
mod deep_research_host_digest;
#[path = "deep_research/host_metadata.rs"]
mod deep_research_host_metadata;
#[path = "deep_research/host_prompt.rs"]
mod deep_research_host_prompt;
#[path = "deep_research/host_report.rs"]
mod deep_research_host_report;
#[path = "deep_research/host_workflow.rs"]
mod deep_research_host_workflow;
#[path = "deep_research/inquiry_runtime.rs"]
mod deep_research_inquiry_runtime;
#[path = "deep_research/report_audit.rs"]
mod deep_research_report_audit;
#[cfg(test)]
#[path = "deep_research/retrieval_contract_tests.rs"]
mod deep_research_retrieval_contract_tests;
#[cfg(test)]
#[path = "deep_research/retrieval_integration_tests.rs"]
mod deep_research_retrieval_integration_tests;
#[path = "deep_research/state_journal.rs"]
mod deep_research_state_journal;
#[path = "deep_research/workflow_store.rs"]
mod deep_research_workflow_store;
#[cfg(test)]
pub(crate) use deep_research_artifacts::deep_research_completed_report_html_for_test;
#[cfg(test)]
pub(crate) use deep_research_artifacts::deep_research_write_report_pair_for_test;
#[cfg(test)]
use deep_research_artifacts::looks_like_deep_research_fallback_draft;
#[cfg(test)]
pub(crate) use deep_research_artifacts::materialize_deep_research_fallback_draft;
use deep_research_artifacts::normalize_research_source_anchor;
#[cfg(test)]
use deep_research_artifacts::research_report_artifacts_from_output_for_query;
#[cfg(test)]
use deep_research_artifacts::workflow_evidence_summary;
pub(crate) use deep_research_artifacts::{
    clean_deep_research_final_text_from_artifacts, materialize_deep_research_recovery_report,
    research_report_artifacts_from_output, resolve_deep_research_run_publication,
    DeepResearchEvidenceFirstPublication, ResearchReportArtifacts,
};
#[cfg(test)]
use deep_research_artifacts::{
    deep_research_report_artifacts_from_output_for_query, deep_research_report_slug,
};
use deep_research_convergence::{
    validated_inquiry_projection, validated_inquiry_publication_outcome, InquiryTerminalOutcome,
    ValidatedInquiryProjection,
};
use deep_research_evidence_ledger::accepted_evidence_ledger;
use deep_research_host_digest::*;
use deep_research_host_metadata::*;
use deep_research_host_prompt::*;
use deep_research_host_report::*;
pub(crate) use deep_research_host_workflow::DeepResearchEvidenceScope;
use deep_research_host_workflow::*;
use deep_research_inquiry_runtime::inquiry_projection_from_workflow;
pub(crate) use deep_research_inquiry_runtime::{
    deep_research_evidence_first_research_spec, spawn_deep_research_evidence_first,
    DEEP_RESEARCH_EVIDENCE_FIRST_HOST_TIMEOUT_MS,
};
pub(crate) use deep_research_state_journal::ResearchOutcome;
use deep_research_state_journal::{
    reconcile_interrupted_latest_run, record_child_event as record_deep_research_child_event,
    record_evidence_ledger as record_deep_research_evidence_ledger,
    record_inquiry_state as record_deep_research_inquiry_state,
    record_run_terminal as record_deep_research_run_terminal,
    record_validated_publication_terminal as record_deep_research_validated_publication_terminal,
    record_workflow_completed as record_deep_research_workflow_completed,
    record_workflow_started as record_deep_research_workflow_started, research_diagnostic,
    research_diff, ResearchDiagnosticKind, ResearchRecoveryDisposition, ResearchRunProjection,
};
pub(crate) use deep_research_workflow_store::{
    ensure_deep_research_workflow_run_id, recover_deep_research_bootstrap_acquisition_from_store,
    recover_deep_research_workflow_run_from_store,
};

pub(crate) struct DeepResearchCliSettlement<'a> {
    pub(crate) workspace: &'a std::path::Path,
    pub(crate) run_id: &'a str,
    pub(crate) query: &'a str,
    pub(crate) workflow_succeeded: bool,
    pub(crate) workflow_output: &'a str,
    pub(crate) workflow_metadata: Option<&'a serde_json::Value>,
    pub(crate) requested_outcome: ResearchOutcome,
    pub(crate) artifacts: &'a ResearchReportArtifacts,
    pub(crate) artifact_authority: DeepResearchTerminalArtifactAuthority,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum DeepResearchTerminalArtifactAuthority {
    ValidatedPublication,
    VerifiedRecovery,
}

pub(crate) async fn settle_deep_research_cli_run(
    settlement: DeepResearchCliSettlement<'_>,
) -> Result<ResearchOutcome, String> {
    let DeepResearchCliSettlement {
        workspace,
        run_id,
        query,
        workflow_succeeded,
        workflow_output,
        workflow_metadata,
        requested_outcome,
        artifacts,
        artifact_authority,
    } = settlement;
    record_deep_research_workflow_completed(workspace, run_id, workflow_succeeded)
        .await
        .map_err(|error| format!("record DeepResearch CLI workflow completion: {error:#}"))?;
    let publication =
        resolve_deep_research_run_publication(workspace, query, run_id, workflow_output);
    let projection = if let Some(publication) = match publication {
        Ok(publication) => publication,
        Err(_)
            if requested_outcome == ResearchOutcome::Degraded
                && artifact_authority
                    == DeepResearchTerminalArtifactAuthority::VerifiedRecovery =>
        {
            None
        }
        Err(error) => {
            return Err(format!("validate DeepResearch CLI publication: {error}"));
        }
    } {
        if artifact_authority != DeepResearchTerminalArtifactAuthority::ValidatedPublication {
            return Err(
                "a validated DeepResearch publication was mislabeled as a recovery artifact"
                    .to_string(),
            );
        }
        let publication_outcome = match publication.publication {
            DeepResearchEvidenceFirstPublication::Synthesized => ResearchOutcome::Completed,
            DeepResearchEvidenceFirstPublication::Qualified => ResearchOutcome::Qualified,
            DeepResearchEvidenceFirstPublication::SourceBacked => ResearchOutcome::Degraded,
            DeepResearchEvidenceFirstPublication::NoEvidence => ResearchOutcome::Degraded,
        };
        if publication_outcome != requested_outcome {
            return Err(format!(
                "DeepResearch CLI publication outcome {publication_outcome:?} disagrees with requested outcome {requested_outcome:?}"
            ));
        }
        if publication.artifacts != *artifacts {
            return Err(
                "DeepResearch CLI terminal artifacts differ from the validated publication"
                    .to_string(),
            );
        }
        record_deep_research_validated_publication_terminal(
            workspace,
            run_id,
            requested_outcome,
            artifacts,
            &publication.quality,
        )
        .await
        .map_err(|error| format!("record DeepResearch CLI terminal publication: {error:#}"))?
    } else {
        if artifact_authority == DeepResearchTerminalArtifactAuthority::VerifiedRecovery
            && requested_outcome != ResearchOutcome::Degraded
        {
            return Err(
                "a verified DeepResearch recovery artifact cannot settle a successful outcome"
                    .to_string(),
            );
        }
        let canonical = deep_research_canonical_workflow_output(workflow_output, workflow_metadata);
        let evidence = accepted_evidence_ledger(&canonical, workflow_metadata);
        record_deep_research_evidence_ledger(workspace, run_id, &evidence)
            .await
            .map_err(|error| format!("record DeepResearch CLI accepted evidence: {error:#}"))?;
        record_deep_research_run_terminal(workspace, run_id, requested_outcome, Some(artifacts))
            .await
            .map_err(|error| format!("record DeepResearch CLI terminal report: {error:#}"))?
    };
    if !projection.outcome.is_terminal()
        || !projection.active_steps.is_empty()
        || !projection.active_children.is_empty()
    {
        return Err(format!(
            "DeepResearch CLI journal did not settle: outcome={:?}, active_steps={}, active_children={}",
            projection.outcome,
            projection.active_steps.len(),
            projection.active_children.len()
        ));
    }
    Ok(projection.outcome)
}

#[cfg(test)]
mod deep_research_cli_settlement_tests {
    use super::*;

    #[tokio::test]
    async fn invalid_publication_requires_verified_recovery_authority() {
        let workspace = tempfile::tempdir().expect("create CLI recovery workspace");
        let query = "Assess the current Nimbus support policy";
        let run_id = "invalid-cli-publication-recovery";
        let workflow_output = serde_json::json!({
            "query": query,
            "output_language": "en",
            "mode": "evidence_first_report",
            "publication": {}
        })
        .to_string();
        let artifacts = materialize_deep_research_recovery_report(
            workspace.path(),
            query,
            "the Host publication envelope was invalid",
            &workflow_output,
            None,
        )
        .expect("materialize verified CLI recovery artifacts");
        let args = serde_json::json!({
            "run_id": run_id,
            "input": {
                "query": query,
                "current_date": "2026-07-23",
                "evidence_scope": "web_and_workspace",
                "inquiry_host_managed": true
            }
        });
        record_deep_research_workflow_started(
            workspace.path(),
            run_id,
            deep_research_evidence_first_research_spec(&args),
        )
        .await
        .expect("start CLI recovery journal");

        let rejected = settle_deep_research_cli_run(DeepResearchCliSettlement {
            workspace: workspace.path(),
            run_id,
            query,
            workflow_succeeded: true,
            workflow_output: &workflow_output,
            workflow_metadata: None,
            requested_outcome: ResearchOutcome::Degraded,
            artifacts: &artifacts,
            artifact_authority: DeepResearchTerminalArtifactAuthority::ValidatedPublication,
        })
        .await
        .expect_err("invalid publication must fail closed without recovery authority");
        assert!(rejected.contains("omitted its status"), "{rejected}");

        let outcome = settle_deep_research_cli_run(DeepResearchCliSettlement {
            workspace: workspace.path(),
            run_id,
            query,
            workflow_succeeded: true,
            workflow_output: &workflow_output,
            workflow_metadata: None,
            requested_outcome: ResearchOutcome::Degraded,
            artifacts: &artifacts,
            artifact_authority: DeepResearchTerminalArtifactAuthority::VerifiedRecovery,
        })
        .await
        .expect("verified recovery must close the CLI transaction");
        assert_eq!(outcome, ResearchOutcome::Degraded);
    }
}

// System integrations.
#[path = "system/skills.rs"]
pub(crate) mod skills;
#[path = "system/update.rs"]
mod update;

// Local workspace.
#[path = "workspace/git_snapshot.rs"]
mod git_snapshot;
#[path = "workspace/gitutil.rs"]
mod gitutil;

// Local and shared knowledge.
#[path = "knowledge/kbutil.rs"]
pub(crate) mod kbutil;

// Context and memory.
#[path = "context/memutil.rs"]
pub(crate) mod memutil;

// OS Runtime bridge.
#[path = "os/progressive.rs"]
mod os_progressive;
#[path = "os/remote_ui.rs"]
pub(crate) mod remote_ui;
#[path = "os/runtime_policy.rs"]
mod runtime_policy;
mod runtime_projection;
mod transcript;

// Terminal UI support.
#[path = "app/agent_presence.rs"]
mod agent_presence;
#[path = "app/actions.rs"]
mod app_actions;
#[path = "app/async_dispatch.rs"]
mod app_async_dispatch;
#[path = "app/commands.rs"]
mod app_commands;
#[path = "app/events.rs"]
mod app_events;
#[path = "app/fork.rs"]
mod app_fork;
#[path = "app/launch.rs"]
mod app_launch;
#[path = "app/live_preview.rs"]
mod app_live_preview;
#[path = "app/permission_rules.rs"]
mod app_permission_rules;
#[path = "app/permissions.rs"]
mod app_permissions;
#[path = "app/projections.rs"]
mod app_projections;
#[path = "app/research_workflow.rs"]
mod app_research_workflow;
#[path = "app/rewind.rs"]
mod app_rewind;
#[path = "app/runtime.rs"]
mod app_runtime;
#[path = "app/selection.rs"]
mod app_selection;
#[path = "app/session_share.rs"]
mod app_session_share;
#[path = "app/session_state.rs"]
mod app_session_state;
#[path = "app/smoke.rs"]
mod app_smoke;
#[path = "app/submit.rs"]
mod app_submit;
#[path = "app/synthesis.rs"]
mod app_synthesis;
#[path = "app/types.rs"]
mod app_types;
#[path = "app/update.rs"]
mod app_update;
#[path = "app/update_dispatch.rs"]
mod app_update_dispatch;
#[path = "app/view.rs"]
mod app_view;
#[path = "app/workflow_capture.rs"]
mod app_workflow_capture;
#[path = "ui/approval.rs"]
mod approval;
#[path = "ui/attachments.rs"]
mod attachments;
#[path = "ui/batch_view.rs"]
mod batch_view;
#[path = "ui/chrome.rs"]
mod chrome;
#[path = "ui/design_markdown.rs"]
mod design_markdown;
#[path = "ui/editor_state.rs"]
mod editor_state;
#[path = "ui/file_change_view.rs"]
mod file_change_view;
#[path = "ui/image.rs"]
mod image;
#[path = "ui/message_chrome.rs"]
mod message_chrome;
#[path = "ui/plan_review.rs"]
mod plan_review;
#[path = "ui/program_preview.rs"]
mod program_preview;
#[path = "ui/render.rs"]
mod render;
#[path = "ui/syntax.rs"]
mod syntax;
#[path = "ui/tool_style.rs"]
mod tool_style;
#[path = "ui/tool_transcript_view.rs"]
mod tool_transcript_view;
#[path = "ui/util.rs"]
mod util;
use agent_presence::{agent_presence_tick, AgentIslandLaunchOutcome};

pub(crate) mod panels;
#[cfg(test)]
use crate::budget::AUTO_COMPACT_THRESHOLD;
use crate::budget::{
    budget_plan_for_effort_index, context_limit_for_model, effort_uses_automatic_delegation,
    resolve_ctx_limit, BudgetPlan, BudgetWorkload, DEFAULT_TUI_EFFORT_INDEX, EFFORT_LEVELS,
    ULTRACODE_INDEX as ULTRACODE,
};
use crate::config::*;
use app_commands::*;
#[cfg(test)]
use app_launch::resumed_transcript_entries;
pub(crate) use app_launch::{resolve_tui_session_store_dir, run_in};
use app_permission_rules::*;
use app_permissions::*;
use app_projections::*;
pub(crate) use app_session_state::tui_session_state_path;
use app_session_state::*;
use app_smoke::run_smoke;
#[cfg(test)]
use app_smoke::{
    deep_research_smoke_execution_deadline, deep_research_smoke_exhausted_phase_message,
    deep_research_smoke_finalization_phase_deadline, deep_research_smoke_phase_deadline,
    deep_research_smoke_remaining_budget, deep_research_smoke_run_deadline,
    run_deep_research_smoke_artifact_step,
};
use app_types::*;
use app_update::*;
use app_workflow_capture::*;
use approval::{ApprovalPrompt, ApprovalPromptMsg};
use asset_naming::*;
use attachments::*;
use chrome::*;
use design_markdown::StreamingMarkdown;
use editor_state::*;
use git_snapshot::*;
use gitutil::*;
use image::*;
use memutil::*;
use message_chrome::*;
pub(crate) use panels::ctx::{parse_ctx_search, strip_controls};
pub(crate) use panels::loop_engineering;
use panels::transcript::{SemanticTranscriptViewport, TranscriptViewportAction};
use plan_review::*;
use render::*;
use runtime_policy::RuntimePolicy;
use runtime_projection::{
    CompletedSubagent, CompletedTool, RuntimeProjection, RuntimeToolCheckpoint, SubagentOutcome,
    ToolCallState,
};
use skills::*;
use syntax::*;
use transcript::{
    join_transcript_blocks, transcript_block_separator, Transcript, TranscriptAnchor,
    TranscriptEntry, TranscriptEntryId, TranscriptPoint, TranscriptSelection,
};
use update::*;
use util::*;

const HITL_CONFIRM_TIMEOUT_MS: u64 = 60 * 60 * 1000;
const BACKGROUND_CONFIRM_TIMEOUT_MS: u64 = 500;
const AUTO_REVIEW_IDLE: Duration = Duration::from_secs(300);
const TOOL_EXEC_TIMEOUT_MS: u64 = 30 * 60 * 1000;
const DEEP_RESEARCH_SMOKE_FINALIZATION_RESERVE_MS: u64 = 10_000;
const DEEP_RESEARCH_ABORT_GRACE_MS: u64 = 2_000;
// The standalone engine owns acquisition, semantic admission, typed claim
// generation, and evidence-preserving editorial planning. Smoke and
// interactive execution therefore share one absolute Host budget instead of
// adding the legacy sectioned-synthesis transaction.
const DEEP_RESEARCH_RUN_HARD_TIMEOUT_MS: u64 = DEEP_RESEARCH_EVIDENCE_FIRST_HOST_TIMEOUT_MS
    + (2 * DEEP_RESEARCH_ABORT_GRACE_MS)
    + DEEP_RESEARCH_SMOKE_FINALIZATION_RESERVE_MS;
const STREAM_START_TIMEOUT_MS: u64 = 10_000;
const STREAM_JOIN_SETTLE_GRACE_MS: u64 = 2_000;
const GRACEFUL_QUIT_STREAM_GRACE_MS: u64 = 2_000;
const GRACEFUL_QUIT_ABORT_SETTLE_MS: u64 = 250;
const GRACEFUL_QUIT_AGENT_PRESENCE_GRACE_MS: u64 = 500;
const GRACEFUL_QUIT_SESSION_CLOSE_GRACE_MS: u64 = 8_000;
const QUEUE_ADMISSION_RETRY_BASE_MS: u64 = 40;
const QUEUE_ADMISSION_RETRY_MAX_MS: u64 = 500;
const TUI_DUPLICATE_TOOL_CALL_THRESHOLD: u32 = 12;
#[allow(dead_code)]
const RESUME_TIMELINE_PAGE_LIMIT: usize = 200;

struct App {
    session: Arc<AgentSession>,
    active_session: SharedActiveSession,
    /// Live projection of independently managed A3S Use MCP and Skill
    /// extensions into the current Code session.
    use_registry: crate::use_registry::UseRegistrySlot,
    /// Agent + session-rebuild bits, kept so `/model` can switch models by
    /// resuming the session under a new model (no in-place model setter exists).
    agent: Arc<Agent>,
    store: Arc<dyn a3s_code_core::store::SessionStore>,
    confirmation: a3s_code_core::hitl::ConfirmationPolicy,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
    /// This session's id (for model-switch resume + the exit hint).
    session_id: String,
    /// Credential source paired with `model`, persisted per session so a
    /// resumed conversation does not inherit another session's last picker tab.
    model_source: ModelSelectionSource,
    /// Monotonic identity and active request guard for async session rebuilds.
    /// Late results must never replace a newer active session.
    session_rebuild_seq: u64,
    session_rebuild_pending: Option<u64>,
    /// "provider/model" ids from the config, for the /model picker.
    models: Vec<String>,
    /// Context-window size per model id, for the ctx% indicator.
    model_ctx: std::collections::HashMap<String, u32>,
    /// Context window of the active model (0 = unknown).
    context_limit: u32,
    /// Prompt tokens of the last turn = current context fill.
    last_prompt_tokens: usize,
    /// Summary of earlier conversation after a manual `/compact` (reseed).
    compact_summary: Option<String>,
    /// Highest context-fill tier already warned about (0 / 70 / 85), so each
    /// warning prints once per fill-up and re-arms when usage drops back.
    ctx_warned_tier: u8,
    /// Selected index in the /model panel; `Some` means the panel is open.
    model_menu: Option<usize>,
    /// Active tab in the /model panel (0 = config; account tabs when signed in).
    model_tab: usize,
    /// `/relay` session picker and its stale-result guard.
    relay_panel: Option<panels::relay::RelayPanel>,
    relay_scan_seq: u64,
    /// `/tasks` / Ctrl+B delegated-work inspector and cancellation surface.
    task_panel: Option<panels::tasks::TaskPanel>,
    task_panel_seq: u64,
    /// `/permissions` exact session/project grant inspector and revocation surface.
    permission_panel: Option<panels::permissions::PermissionPanel>,
    /// Picker-visible models advertised for the current Codex login.
    codex_account_models: Vec<crate::account_providers::codex::CodexModel>,
    /// Guards the asynchronous Codex catalog refresh from duplicate commands.
    codex_models_loading: bool,
    /// Last successful account catalog refresh; refreshed again after Codex's
    /// five-minute cache window so long-running TUIs see new model rollouts.
    codex_models_refreshed_at: Option<Instant>,
    /// Lazily discovered model ids for account-backed CLI providers.
    account_models: HashMap<crate::account_providers::AccountProvider, Vec<String>>,
    account_models_loading: HashSet<crate::account_providers::AccountProvider>,
    account_model_errors: HashMap<crate::account_providers::AccountProvider, String>,
    /// Custom LLM client to inject for signed-in account tabs; None uses config.acl.
    llm_override: Option<LlmOverride>,
    /// Parsed config used to rebuild config-backed model clients with the same
    /// v5.2 provider capabilities after /model and /effort changes.
    code_config: Arc<CodeConfig>,
    /// Paths resolved once from the immutable CLI invocation and effective ACL.
    asset_directories: crate::commands::config::CodeAssetDirectories,
    config_path: PathBuf,
    memory_dir: PathBuf,
    auto_compact_threshold: f64,
    /// Optional OS endpoint from config.acl; enables /login and /logout.
    os_config: Option<OsConfig>,
    /// Restored OS login (from `~/.a3s/os-auth.json`, persisted across runs);
    /// `None` = signed out. Loaded on startup, set by /login, cleared by /logout.
    os_session: Option<crate::a3s_os::StoredOsSession>,
    /// True while an OS access-token refresh is in flight (guards the BannerTick
    /// trigger from spawning a second refresh before the first resolves).
    os_refreshing: bool,
    /// OS unified-gateway models for the `/model` picker, lazily fetched when
    /// the signed-in user opens the OS Gateway tab. `None` = not fetched yet or
    /// currently loading; `Some([])` = the gateway is unavailable/unconfigured.
    os_gateway_models: Option<Vec<String>>,
    /// True while the OS Gateway tab is fetching its model list. Guards against
    /// spawning duplicate slow requests while the user switches tabs repeatedly.
    os_gateway_models_loading: bool,
    /// The precise reason the last gateway-models fetch failed (e.g. `/v1` not
    /// proxied → HTML, auth error, unreachable), shown in the `/model` picker.
    os_gateway_error: Option<String>,
    /// Last OS view seen in a tool result. Generic tool views are opened by
    /// clicking the inline "Open view" button; owned workflows like `/flow` may
    /// also open their prepared designer view directly.
    last_view: Option<remote_ui::ViewSpec>,
    /// Completed DeepResearch report view captured before settlement. It opens
    /// only when DeepResearch actually finishes.
    pending_deep_research_report_view: Option<remote_ui::ViewSpec>,
    /// Transient state for the active coverage-driven DeepResearch run.
    deep_research_loop: Option<DeepResearchLoop>,
    /// Transient host hand-off data. Event-derived lifecycle and quality state
    /// deliberately remain outside this snapshot.
    deep_research_workflow: DeepResearchWorkflowSnapshot,
    /// Lifecycle owner for the isolated typed DeepResearch runtime.
    deep_research_handle: Option<CodeDeepResearchRunHandle>,
    /// Typed engine and nested-agent progress stream for the active run.
    deep_research_events: Option<SharedDeepResearchRx>,
    /// Terminal classification for the active DeepResearch run. Recovery
    /// artifacts are useful diagnostics but must never be counted as a
    /// completed report.
    deep_research_outcome: DeepResearchRunOutcome,
    /// Monotonic guard for DeepResearch stream watchdogs; stale timeout ticks
    /// must not affect later turns.
    deep_research_stream_timeout_token: u64,
    /// Monotonic identity for asynchronously-started model streams. A late
    /// StreamStarted/StreamError from a cancelled turn must never replace the
    /// receiver of a queued successor.
    stream_start_token: u64,
    /// An interrupted turn whose async `session.stream` admission has not
    /// returned yet. A queued successor cannot start until that stale worker is
    /// cancelled and its single-flight lease has been released.
    interrupted_stream_start_token: Option<u64>,
    /// Continuation deferred behind `interrupted_stream_start_token`. This is
    /// populated when interruption cleanup wins the race with stream admission.
    pending_interrupted_continuation: Option<InterruptedContinuation>,
    /// Required Runtime use for the current autonomous workflow, plus observed
    /// evidence from tool/subagent/view events.
    runtime_expectation: Option<RuntimeExpectation>,
    /// Current model effort (index into EFFORT_LEVELS).
    effort: usize,
    /// `/effort` slider panel: temp selection while open.
    effort_panel: Option<usize>,
    /// `/theme` picker: temp theme index while open.
    theme_panel: Option<usize>,
    /// First Ctrl+C arms quit; a second within the window exits.
    quit_armed: Option<Instant>,
    /// True while `/exit` or confirmed Ctrl+C is cancelling the session and
    /// settling its active stream. All late UI events are ignored until
    /// `QuitReady`, so cancellation cannot start an automatic continuation.
    quitting: bool,
    /// Last user activity; drives the inactivity auto-review.
    last_activity: Instant,
    /// Tracks which real conversation revision was reviewed and rejects stale
    /// asynchronous results. UI status lines and navigation keys do not alter it.
    auto_review: AutoReviewTracker,
    /// Shell mode: a leading `!` becomes the prompt, the rest is the command.
    shell_mode: bool,
    /// Deep-research mode: a leading `?` launches the fixed host-managed
    /// semantic-plan, single-retrieval, closed-evidence, assessment, and report
    /// pipeline. Box turns cyan.
    research_mode: bool,
    /// True from an asset-scoped review submit until its report is parsed (or
    /// the run is interrupted/fails). Gates capture_review so a turn that merely
    /// QUOTES an a3s-review block can't open a phantom checklist.
    review_pending: bool,
    /// True from a `/sleep` submit until its report is parsed (or the run is
    /// interrupted/fails). Gates capture_sleep the same way.
    sleep_pending: bool,
    /// Last parsed asset-review report (issues + checkbox state). Survives the
    /// panel closing so a follow-up asset review can reopen it.
    review: Option<panels::review::ReviewState>,
    /// `/flow` DAG picker (login-gated); open when `Some`.
    flow: Option<panels::flow::FlowPanel>,
    /// A `/flow <action>` submitted before a flow was selected; run after selection.
    pending_flow_subcommand: Option<panels::flow::FlowSubcommand>,
    /// `/agent` definition picker; open when `Some`.
    agent_picker: Option<panels::agent::AgentPanel>,
    /// A `/agent <action>` submitted before an agent was active; run after selection.
    pending_agent_subcommand: Option<panels::agent::AgentSubcommand>,
    /// The local agent currently being developed by ordinary user turns.
    agent_dev: Option<panels::agent::AgentDevSession>,
    /// `/mcp` asset selector; open when `Some`.
    mcp_picker: Option<panels::mcp::McpPanel>,
    /// A `/mcp <action>` submitted before an MCP was active; run after selection.
    pending_mcp_subcommand: Option<panels::mcp::McpSubcommand>,
    /// The local MCP asset currently being developed by ordinary user turns.
    mcp_dev: Option<panels::mcp::McpDevSession>,
    /// `/skill` picker; open when `Some`.
    skill_picker: Option<panels::skill::SkillPanel>,
    /// A `/skill <action>` submitted before a skill was active; run after selection.
    pending_skill_subcommand: Option<panels::skill::SkillSubcommand>,
    /// The local skill currently being developed by ordinary user turns.
    skill_dev: Option<panels::skill::SkillDevSession>,
    /// `/okf` OKF package picker; open when `Some`.
    okf_picker: Option<panels::okf::OkfPackagePanel>,
    /// A `/okf <action>` submitted before an OKF package was active; run after selection.
    pending_okf_subcommand: Option<panels::okf::OkfCommand>,
    /// The local OKF package currently being developed by ordinary user turns.
    okf_dev: Option<panels::okf::OkfDevSession>,
    /// Whether the review issue-checklist overlay is showing.
    review_open: bool,
    /// `ctx` CLI detected at startup (past-session history search).
    ctx_ready: bool,
    /// Last `/ctx` search hits, addressable as `/ctx <n>`.
    ctx_hits: Vec<panels::ctx::CtxHit>,
    /// A transcript window staged by `/ctx <n>`, attached (one-shot) to the
    /// next outgoing message.
    pending_ctx: Option<String>,
    /// True for the single `Msg::Submit` the `/loop` mechanism emits to
    /// auto-continue — so on_submit doesn't attach a staged `/ctx` window to
    /// this machine turn.
    loop_continuation: bool,
    /// ALL assistant text of the current turn (across mid-turn tool-call
    /// finalizes, which clear the live streaming buffer). capture_review scans
    /// this when a provider leaves `End.text` empty.
    turn_text: String,
    /// Transaction boundary for one Core LLM turn. Repeating `TurnStart` with
    /// the same number restores this snapshot before an in-place stream retry.
    llm_turn_checkpoint: Option<LlmTurnUiCheckpoint>,
    /// Active transcript text-selection (mouse drag → highlight → copy on
    /// release); `None` when there's no selection.
    selection: Option<Selection>,
    /// Latest dynamic-workflow artifact (ultracode dynamic workflow or task dispatch),
    /// retained for synthesis and shown collapsed in the transcript.
    last_workflow: Option<String>,
    /// Clipboard images pasted into the composer, sent with the owning message.
    pending_images: Vec<PendingImage>,
    /// Persistent north-star goal (`/goal`), prepended to each prompt.
    goal: Option<String>,
    /// When the current `/goal` was set — drives the "Pursuing goal (1h 32m)"
    /// elapsed timer in the status bar. `None` whenever `goal` is `None`.
    goal_since: Option<Instant>,
    /// Durable `/goal` execution state. Unlike `/loop`, this has no turn cap:
    /// only a matching Core GoalAchieved event can close it.
    goal_run: Option<panels::goal_engineering::GoalRunState>,
    /// Incomplete goal retained across TUI exits. It remains paused until the
    /// startup picker or `/goal resume` explicitly activates it.
    paused_goal: Option<PausedGoalState>,
    /// Selected row in the startup "Resume paused goal?" picker.
    goal_resume_prompt: Option<usize>,
    /// Monotonic invalidation token for delayed goal retries.
    goal_generation: u64,
    /// Retry context retained until Core's stream worker releases its
    /// single-flight lease. Goal continuation starts only after that join.
    pending_goal_failure: Option<String>,
    /// User goal temporarily shadowed by an active DeepResearch task.
    deep_research_goal_restore: Option<(Option<String>, Option<Instant>)>,
    /// Remaining auto-continue turns for `/loop` (0 = off).
    loop_remaining: usize,
    /// ECS-style projection of live runtime tool and subagent entities.
    runtime: RuntimeProjection,
    /// Exact local lifecycle publishing and the system-level island bridge.
    /// Rendering belongs to the independent native `a3s-webview` process.
    agent_presence: agent_presence::AgentPresenceRuntime,
    /// Current host-owned artifact preview. The native window is tracked so it
    /// can be replaced or closed without stopping the shared A3S Web server.
    live_preview: Option<app_live_preview::LivePreviewState>,
    /// Monotonic invalidation guard for asynchronous preview launches.
    preview_launch_seq: u64,
    /// Exact launch currently starting, including its user-visible target.
    live_preview_pending: Option<(u64, String, TranscriptEntryId)>,
    /// Active background completion watchers, keyed by rebuild generation and
    /// task id so session replacement cannot leak stale results into history.
    background_subagent_watches: HashSet<(u64, String)>,
    /// Monotonic identity for asynchronous tracker snapshots. DeepResearch
    /// settlement invalidates older requests before exposing a terminal report.
    subagent_snapshot_request_id: u64,
    deep_research_subagent_settlement_inflight: bool,
    /// Prevent duplicate terminal journal writes while the final projection is
    /// being persisted before the TUI clears its DeepResearch state.
    deep_research_journal_finalization_inflight: bool,
    /// Validated report pair and the Host authority that admitted it.
    deep_research_terminal_artifacts: Option<(
        ResearchReportArtifacts,
        DeepResearchTerminalArtifactAuthority,
    )>,
    /// Monotonic cursor for normalized `AgentEvent` projections.
    deep_research_agent_event_sequence: u64,
    /// Latest replayable DeepResearch view used by pinned TUI projections.
    deep_research_projection: Option<ResearchRunProjection>,
    /// True once this turn used tools/planning/subagents that need a final
    /// user-facing synthesis if the model stops without text afterwards.
    turn_had_agent_activity: bool,
    /// True once assistant text arrived after the latest tool/planning/subagent
    /// activity in this turn.
    turn_text_after_activity: bool,
    /// Guard for the hidden ultracode continuation that turns raw workflow
    /// results into a final answer.
    ultracode_synthesis_inflight: bool,
    /// At most one hidden synthesis continuation per user turn.
    ultracode_synthesis_used: bool,
    /// Project instructions (CLAUDE.md/AGENT.md), injected into the system prompt.
    instructions: Option<String>,
    /// Shared in-memory workspace file manifest, refreshed by a background watcher.
    workspace_manifest: Arc<LocalWorkspaceManifest>,
    workspace_manifest_rx: SharedManifestRx,
    /// Manifest-backed workspace backend used by agent tools.
    workspace_services: Arc<WorkspaceServices>,
    /// Start of the short brand-gradient input-border flourish after Ultracode
    /// activation; cleared as soon as its dedicated animation finishes.
    gradient_until: Option<Instant>,
    gradient_frame: usize,
    /// Invalidates delayed ticks after cancel/reopen or phase handoff.
    ultracode_animation_epoch: u64,
    /// Ultracode confirm animation playing in the /effort panel before it closes.
    effort_anim: Option<Instant>,
    /// Full-width, style-preserving semantic transcript opened by Ctrl+T.
    transcript_view: Option<SemanticTranscriptViewport>,
    viewport: Viewport,
    textarea: Textarea,
    spinner: Spinner,
    streaming: StreamingMarkdown,
    /// Whether the current turn streamed any text deltas (vs. text only at End).
    got_delta: bool,
    /// Set while `/compact` is summarizing — drives the progress bar + blocks input.
    compacting: Option<Instant>,
    /// Set while `/update` is upgrading — drives a progress bar + blocks input;
    /// on success the app restarts into the new binary.
    updating: Option<Instant>,
    /// Host-owned, read-only `/checkup` preflight. The composer stays hidden
    /// until its typed result is handed to the strict Plan audit.
    checkup_inflight: bool,
    /// Last time the streaming viewport was rebuilt — throttles the O(n) rebuild
    /// to ~30fps so a flood of deltas doesn't starve animation on the 1 loop.
    last_paint: Option<Instant>,
    /// Live reasoning ("thinking") text for the current turn, shown dimmed above
    /// the answer and cleared when the answer is finalized.
    thinking: String,
    state: State,
    messages: Transcript,
    rx: Option<SharedRx>,
    stream_join: Option<StreamJoin>,
    /// True after a terminal event while the stream worker is still releasing
    /// persistence and the core single-flight admission lease. Input remains
    /// queue-only until `StreamJoinSettled` arrives.
    stream_join_settling: bool,
    /// Lets Esc release a terminal stream worker immediately when a queued
    /// follow-up is waiting, without starting the next turn before the lease is
    /// actually dropped.
    stream_settle_abort: Option<tokio::task::AbortHandle>,
    /// Abort handle for host-direct tools such as the DeepResearch workflow.
    host_tool_abort: Option<HostToolAbort>,
    /// True while `rx` is carrying host-direct tool progress rather than an
    /// agent stream; channel close must not finish the turn.
    host_progress_inflight: bool,
    /// Stable call ID emitted by the active host-direct tool lifecycle.
    host_tool_call_id: Option<String>,
    interrupting: bool,
    /// Manual tool approvals waiting for a decision, in request order.
    pending_tools: VecDeque<PendingToolApproval>,
    /// Exact session/project grants shared across model and effort rebuilds.
    permission_grants: TuiPermissionGrants,
    /// Mode-aware permission boundary shared with every rebuilt Core session.
    /// It always tracks the immutable mode of the running turn.
    execution_policy: TuiExecutionPolicy,
    /// Dedicated project ACL file for reviewed persistent grants.
    project_permission_rules_path: PathBuf,
    /// Project rule write currently in flight. Its request remains at the FIFO
    /// head until persistence succeeds, so a failed write cannot silently
    /// broaden the active session.
    permission_rule_write_inflight: Option<String>,
    /// Monotonic identity and global lock for atomic project-grant revocation.
    project_permission_revoke_seq: u64,
    project_permission_revoke_inflight: Option<(u64, ExactPermissionGrant)>,
    /// Denial feedback temporarily owns the composer while retaining its draft.
    approval_feedback: Option<ApprovalFeedback>,
    /// Selected row in the tool-approval options panel.
    approval_sel: usize,
    /// Submitted prompts, oldest first, for ↑/↓ recall.
    history: Vec<String>,
    /// `/history` / Ctrl+R fuzzy prompt-history search.
    history_panel: Option<panels::history::HistoryPanel>,
    /// Cursor into `history` while browsing; `None` means "fresh input".
    history_pos: Option<usize>,
    /// Scratch input captured when prompt-history browsing starts.
    history_draft: Option<String>,
    /// Model name reported by the provider (captured from the first turn).
    model: Option<String>,
    /// Cumulative OUTPUT (generated) tokens this session — what `↓` reports.
    output_tokens: usize,
    /// When the current run started, for the live elapsed-time indicator.
    stream_started: Option<Instant>,
    /// Animation counter for the blinking running-tool dot (advances per tick).
    blink_tick: u8,
    /// Frame counter for the welcome-mascot animation.
    anim: u8,
    /// Run mode (Shift+Tab cycles default → plan → auto).
    mode: Mode,
    /// The mode to restore once an autonomous directive run finishes —
    /// `Some` while such a run auto-switched to `Mode::Auto`.
    autonomy_restore: Option<Mode>,
    /// Host turns submitted while the agent is busy. Ordering and FIFO
    /// semantics come directly from a3s-lane.
    queue: PriorityQueue<Queued>,
    /// Submission-time execution mode keyed by a3s-lane's stable sequence.
    /// Keeping this beside the queue avoids mutable footer state changing the
    /// semantics of a turn that was already submitted.
    queued_turn_modes: HashMap<u64, Mode>,
    /// Strict planning requests keyed by the same stable queue sequence.
    queued_plan_drafts: HashMap<u64, PlanDraftRequest>,
    /// Exact pending turn selected for Send now. This control-plane pointer
    /// overrides normal priority/FIFO ordering without rewriting Lane metadata.
    send_now_queued_sequence: Option<u64>,
    /// `/queue` inspection and control modal.
    queue_panel: Option<panels::queue::QueuePanel>,
    /// Pre-turn state admitted with the active user stream. It becomes a
    /// conflict-safe rewind checkpoint only after Core persistence settles.
    active_rewind_checkpoint: Option<RewindCheckpointSeed>,
    /// Recent completed user turns, newest at the back.
    rewind_checkpoints: VecDeque<RewindCheckpoint>,
    /// Monotonic identity for completed rewind checkpoints.
    next_rewind_checkpoint_id: u64,
    /// Stream token whose post-turn Git snapshot is being finalized.
    rewind_finalization_pending: Option<u64>,
    /// A claimed queued turn remains here until Core admits it. Admission
    /// failure restores this exact item, including its original FIFO sequence.
    active_queued_turn: Option<PriorityItem<Queued>>,
    active_queued_turn_token: Option<u64>,
    /// Immutable mode of the current stream, retained after queue admission.
    active_turn_mode: Option<Mode>,
    /// Planning request owned by the current read-only stream.
    active_plan_draft: Option<PlanDraftRequest>,
    queue_retry_generation: u64,
    queue_retry_attempt: u8,
    /// Text of the message currently being processed (the running task).
    running_task: Option<String>,
    /// Typed live plan/TODO projection, pinned above the input and updated from
    /// PlanningEnd/TaskUpdated or the Codex-compatible `update_plan` tool.
    plan: PlanProjection,
    /// Review staged until the completed stream releases Core's single-flight
    /// lease, then promoted to the modal decision boundary below.
    pending_plan_review: Option<PlanReviewState>,
    /// Explicit Approve / Revise / Abandon boundary for a completed plan.
    plan_review: Option<PlanReviewState>,
    /// `/ide` file-tree + viewer panel (Some when open).
    ide: Option<Ide>,
    /// `/memory` full-screen timeline panel (Some when open).
    memory: Option<MemPanel>,
    /// `/evolution` memory-derived candidate review and asset lifecycle panel.
    evolution: Option<panels::evolution::EvolutionPanel>,
    /// Asset-scoped OS digital-asset browser.
    asset_list: Option<panels::asset_resources::AssetListPanel>,
    /// Asset-scoped OS Runtime activity panel.
    runtime_activity: Option<panels::asset_resources::RuntimeActivityPanel>,
    /// `/kb` full-screen local personal knowledge-base panel (Some when open).
    kb: Option<panels::kb::KbPanel>,
    /// `/loop` engineered loop dashboard (Some when open).
    loop_panel: Option<panels::loop_engineering::LoopPanel>,
    /// `/help` overlay panel is showing.
    help_open: bool,
    /// Scroll offset inside the `/help` overlay.
    help_scroll: usize,
    /// Turns completed this session, for the status-bar task counter.
    completed: usize,
    /// Working directory shown for context.
    cwd: String,
    /// Git branch of the workspace (if any), shown in the bottom status bar.
    branch: Option<String>,
    /// Selected index in the `/` command or `$` Skill menu.
    slash_sel: usize,
    /// Exact slash draft whose menu was dismissed with Esc or mouse cancel.
    slash_menu_dismissed_for: Option<String>,
    /// Workspace files (for the `@` file picker) + its selected index.
    files: Vec<String>,
    file_sel: usize,
    /// Expanded directories in the `@` picker tree (collapsed by default).
    at_expanded: std::collections::HashSet<String>,
    /// Count of discoverable Claude skills (incl. plugin-bundled) for the banner.
    skill_count: usize,
    /// Loaded skills (name, description) for the `$` menu + `/plugin`.
    skills: Vec<(String, String)>,
    /// Skill names the user disabled via `/plugin` (persisted, hidden from `$`).
    disabled_skills: std::collections::HashSet<String>,
    /// `/plugin` panel: selected row while open.
    plugins_panel: Option<usize>,
    /// Newer release found at startup (latest version), if any.
    update_available: Option<String>,
    width: u16,
    height: u16,
    keymap: Keymap<Action>,
}

impl App {
    fn composer_input_is_hidden(&self) -> bool {
        self.goal_resume_prompt.is_some()
            || self.state == State::Awaiting
            || (self.plan_review.is_some() && !self.plan_review_input_active())
            || self.transcript_view.is_some()
            || self.queue_panel.is_some()
            || self.history_panel.is_some()
            || self.model_menu.is_some()
            || self.relay_panel.is_some()
            || self.task_panel.is_some()
            || self.permission_panel.is_some()
            || self.checkup_inflight
            || self.effort_panel.is_some()
            || self.theme_panel.is_some()
            || self.plugins_panel.is_some()
            || self.review_open
            || self.memory.is_some()
            || self.evolution.is_some()
            || self.asset_list.is_some()
            || self.runtime_activity.is_some()
            || self.kb.is_some()
            || self.loop_panel.is_some()
            || self.flow.is_some()
            || self.agent_picker.is_some()
            || self.mcp_picker.is_some()
            || self.skill_picker.is_some()
            || self.okf_picker.is_some()
            || self.help_open
    }

    fn begin_graceful_quit(&mut self) -> Option<Cmd<Msg>> {
        if self.quitting {
            return None;
        }

        // Checkpoint the resumable UI state immediately. The goal's loop files
        // are marked paused only after the stream has settled, avoiding a race
        // with an in-flight iteration that may still be writing STATE.md.
        if let Err(error) = self.persist_tui_session_state() {
            tracing::warn!(%error, "failed to checkpoint TUI session settings before exit");
        }

        self.quitting = true;
        self.interrupting = true;
        if let Some(ide) = self.ide.as_mut() {
            ide.intelligence_cancellation.cancel();
            ide.intelligence_jump_cancellation.cancel();
        }
        self.preview_launch_seq = self.preview_launch_seq.wrapping_add(1);
        self.live_preview_pending = None;
        self.live_preview = None;
        self.stream_start_token = self.stream_start_token.wrapping_add(1);
        self.deep_research_stream_timeout_token =
            self.deep_research_stream_timeout_token.wrapping_add(1);
        self.push_line(&Style::new().fg(TN_YELLOW).render("  exiting…"));

        let session = Arc::clone(&self.session);
        let stream_join = self.stream_join.take();
        let host_tool_abort = self.host_tool_abort.take();
        let deep_research_handle = self.deep_research_handle.take();
        self.deep_research_events = None;
        let agent_presence = self.agent_presence.publisher.clone();
        self.rx = None;

        Some(cmd::cmd(move || async move {
            // Remove the exact heartbeat before potentially waiting on model
            // cleanup. A blocked filesystem must not wedge quit; if bounded
            // removal times out, the normal heartbeat TTL retires the row.
            if tokio::time::timeout(
                Duration::from_millis(GRACEFUL_QUIT_AGENT_PRESENCE_GRACE_MS),
                agent_presence.remove(),
            )
            .await
            .is_err()
            {
                tracing::warn!("timed out removing the local agent-presence heartbeat");
            }
            if let Some(abort) = host_tool_abort {
                abort.abort();
            }
            if let Some(handle) = deep_research_handle {
                let _ = handle.cancel_and_settle().await;
            }

            let close = settle_session_close_for_quit(
                async move {
                    session.close().await;
                },
                Duration::from_millis(GRACEFUL_QUIT_SESSION_CLOSE_GRACE_MS),
            );
            match stream_join {
                Some(stream_join) => {
                    let settle = settle_stream_join_for_quit(
                        stream_join,
                        Duration::from_millis(GRACEFUL_QUIT_STREAM_GRACE_MS),
                    );
                    let _ = tokio::join!(close, settle);
                }
                None => {
                    close.await;
                }
            }

            Msg::QuitReady
        }))
    }

    fn finish_graceful_quit(&mut self) -> Option<Cmd<Msg>> {
        self.pause_goal_for_exit();
        if let Err(error) = self.persist_tui_session_state() {
            tracing::warn!(%error, "failed to finalize TUI session settings before exit");
        }
        Some(cmd::quit())
    }

    fn request_subagent_snapshots(&mut self) -> Cmd<Msg> {
        self.subagent_snapshot_request_id = self.subagent_snapshot_request_id.wrapping_add(1);
        load_subagent_snapshots(
            self.session.clone(),
            self.session_id.clone(),
            self.session_rebuild_seq,
            self.subagent_snapshot_request_id,
        )
    }

    fn invalidate_subagent_snapshots(&mut self) {
        self.subagent_snapshot_request_id = self.subagent_snapshot_request_id.wrapping_add(1);
    }

    pub(crate) fn touch_workspace_file(&self, path: &str) {
        self.workspace_manifest.touch_file(path);
    }

    pub(crate) fn viewport_content_width(&self) -> usize {
        viewport_content_width_for(self.width)
    }

    fn transcript_markdown_width(&self) -> usize {
        transcript_markdown_width_for(self.width)
    }
}

include!("approval_layout.rs");
