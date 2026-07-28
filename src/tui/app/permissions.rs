//! Session policy, DeepResearch tool gates, and terminal permission checks.

use super::*;

pub(super) fn with_recent_workspace_context(
    opts: SessionOptions,
    manifest: &Arc<LocalWorkspaceManifest>,
) -> SessionOptions {
    opts.with_context_provider(Arc::new(RecentWorkspaceFilesContextProvider::new(
        manifest.clone(),
    )))
}

pub(super) fn tui_session_options(
    confirmation: a3s_code_core::hitl::ConfirmationPolicy,
) -> SessionOptions {
    tui_session_options_with_gate_and_grants(
        confirmation,
        DeepResearchReportToolGate::default(),
        TuiPermissionGrants::default(),
    )
}

#[cfg(test)]
pub(super) fn tui_session_options_with_gate(
    confirmation: a3s_code_core::hitl::ConfirmationPolicy,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
) -> SessionOptions {
    tui_session_options_with_gate_and_grants(
        confirmation,
        deep_research_report_tool_gate,
        TuiPermissionGrants::default(),
    )
}

pub(super) fn tui_session_options_with_gate_and_grants(
    confirmation: a3s_code_core::hitl::ConfirmationPolicy,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
    permission_grants: TuiPermissionGrants,
) -> SessionOptions {
    tui_session_options_with_gate_grants_and_execution(
        confirmation,
        deep_research_report_tool_gate,
        permission_grants,
        TuiExecutionPolicy::default(),
    )
}

pub(super) fn tui_session_options_with_gate_grants_and_execution(
    confirmation: a3s_code_core::hitl::ConfirmationPolicy,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
    permission_grants: TuiPermissionGrants,
    execution_policy: TuiExecutionPolicy,
) -> SessionOptions {
    let permission_policy = tui_permission_policy();
    let confirmation_manager =
        TuiModeConfirmationProvider::new(confirmation, execution_policy.clone());
    SessionOptions::new()
        .with_auto_compact(false)
        .with_confirmation_manager(Arc::new(confirmation_manager))
        .with_permission_policy(permission_policy.clone())
        .with_permission_checker(Arc::new(
            TuiHitlPermissionChecker::with_grants_and_execution(
                permission_policy,
                deep_research_report_tool_gate,
                permission_grants,
                execution_policy,
            ),
        ))
        .with_tool_timeout(TOOL_EXEC_TIMEOUT_MS)
        .with_duplicate_tool_call_threshold(TUI_DUPLICATE_TOOL_CALL_THRESHOLD)
}

/// Core serializable permission policy for the TUI.
///
/// The runtime checker below layers structured decisions for bash, git, and
/// batch on top of this policy. Keep this policy conservative and serializable
/// so persisted sessions still have a safe fallback.
pub(super) fn tui_permission_policy() -> a3s_code_core::permissions::PermissionPolicy {
    a3s_code_core::permissions::PermissionPolicy::new()
        .deny_all(&[
            "Read(/**)",
            "Read(**/../**)",
            "Grep(* /**)",
            "Grep(* **/../**)",
            "Glob(/**)",
            "Glob(**/../**)",
            "LS(/**)",
            "LS(**/../**)",
            "Write(/**)",
            "Edit(/**)",
            "Write(**/../**)",
            "Edit(**/../**)",
        ])
        .allow_all(&[
            "Read(*)",
            "Grep(*)",
            "Glob(*)",
            "LS(*)",
            "web_search(*)",
            "web_fetch(*)",
            "mcp__use_*",
        ])
        .ask_all(&[
            "Write(*)",
            "Edit(*)",
            "Patch(*)",
            "Bash(*)",
            "Git(*)",
            "batch(*)",
            "program(*)",
            "task(*)",
            "parallel_task(*)",
            "dynamic_workflow(*)",
            "Skill(*)",
        ])
}

/// Mutable execution semantics selected by the TUI for the next run.
///
/// Core calls the permission and confirmation snapshot hooks when a run is
/// admitted. The resulting child policy has its own mode cell, so this shared
/// selector may move to the next turn without changing any in-flight work.
#[derive(Clone)]
pub(super) struct TuiExecutionPolicy {
    mode: Arc<AtomicU8>,
    workspace: Arc<PathBuf>,
}

impl Default for TuiExecutionPolicy {
    fn default() -> Self {
        Self::new(Mode::Default)
    }
}

impl TuiExecutionPolicy {
    const DEFAULT: u8 = 0;
    const PLAN: u8 = 1;
    const AUTO: u8 = 2;

    pub(super) fn new(mode: Mode) -> Self {
        Self::for_workspace(mode, PathBuf::from("."))
    }

    pub(super) fn for_workspace(mode: Mode, workspace: PathBuf) -> Self {
        let policy = Self {
            mode: Arc::new(AtomicU8::new(Self::DEFAULT)),
            workspace: Arc::new(workspace),
        };
        policy.set_mode(mode);
        policy
    }

    pub(super) fn set_mode(&self, mode: Mode) {
        let encoded = match mode {
            Mode::Default => Self::DEFAULT,
            Mode::Plan => Self::PLAN,
            Mode::Auto => Self::AUTO,
        };
        self.mode.store(encoded, Ordering::SeqCst);
    }

    pub(super) fn mode(&self) -> Mode {
        match self.mode.load(Ordering::SeqCst) {
            Self::PLAN => Mode::Plan,
            Self::AUTO => Mode::Auto,
            _ => Mode::Default,
        }
    }

    fn snapshot(&self) -> Self {
        Self {
            mode: Arc::new(AtomicU8::new(match self.mode() {
                Mode::Default => Self::DEFAULT,
                Mode::Plan => Self::PLAN,
                Mode::Auto => Self::AUTO,
            })),
            workspace: Arc::clone(&self.workspace),
        }
    }

    /// Reject a confirmation unexpectedly emitted during an Auto turn.
    ///
    /// Normal Auto calls are resolved by the permission checker and never reach
    /// confirmation. Reaching this fallback therefore means a tool, child run,
    /// or stale integration requested authority outside the established
    /// boundary. Auto is non-interactive and fails that escalation closed.
    pub(super) fn auto_confirmation_decision(
        &self,
        _tool_name: &str,
        _args: &serde_json::Value,
        _workspace: &Path,
    ) -> Option<bool> {
        if self.mode() != Mode::Auto {
            return None;
        }
        Some(false)
    }
}

fn targets_protected_workspace_metadata(tool_name: &str, args: &serde_json::Value) -> bool {
    matches!(tool_name, "write" | "edit" | "patch")
        && args
            .get("file_path")
            .and_then(serde_json::Value::as_str)
            .is_some_and(a3s_code_core::sandbox::is_protected_workspace_path)
}

/// Confirmation provider that preserves interactive HITL for Default/Plan
/// turns while closing the escalation path for an immutable Auto turn.
///
/// Permission `Allow` normally bypasses HITL, but tool-owned metadata (for
/// example, an MCP destructive annotation) may explicitly request a second
/// confirmation check. Such a request is a boundary crossing: Auto rejects it
/// before Core emits a confirmation event instead of silently approving the
/// external side effect.
struct TuiModeConfirmationProvider {
    inner: Arc<a3s_code_core::hitl::ConfirmationManager>,
    execution_policy: TuiExecutionPolicy,
}

impl TuiModeConfirmationProvider {
    fn new(
        policy: a3s_code_core::hitl::ConfirmationPolicy,
        execution_policy: TuiExecutionPolicy,
    ) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(16);
        // TUI approval is fail-closed in every mode. Core keeps
        // `AutoApprove` for non-interactive embedders, but a terminal prompt
        // timing out can never manufacture user consent.
        let timeout_ms = policy.default_timeout_ms;
        let policy = policy.with_timeout(timeout_ms, a3s_code_core::hitl::TimeoutAction::Reject);
        Self {
            inner: Arc::new(a3s_code_core::hitl::ConfirmationManager::new(
                policy, event_tx,
            )),
            execution_policy,
        }
    }

    fn auto_response(
        approved: bool,
    ) -> tokio::sync::oneshot::Receiver<a3s_code_core::hitl::ConfirmationResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = tx.send(a3s_code_core::hitl::ConfirmationResponse {
            approved,
            reason: (!approved)
                .then(|| "Denied by the non-interactive Auto execution boundary.".to_string()),
        });
        rx
    }
}

#[async_trait::async_trait]
impl a3s_code_core::hitl::ConfirmationProvider for TuiModeConfirmationProvider {
    fn snapshot_for_run(&self) -> Option<Arc<dyn a3s_code_core::hitl::ConfirmationProvider>> {
        Some(Arc::new(Self {
            inner: Arc::clone(&self.inner),
            execution_policy: self.execution_policy.snapshot(),
        }))
    }

    async fn requires_confirmation(&self, tool_name: &str) -> bool {
        self.inner.requires_confirmation(tool_name).await
    }

    async fn confirmation_available_for(
        &self,
        _tool_name: &str,
        _args: &serde_json::Value,
    ) -> bool {
        self.execution_policy.mode() != Mode::Auto
    }

    async fn request_confirmation(
        &self,
        tool_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> tokio::sync::oneshot::Receiver<a3s_code_core::hitl::ConfirmationResponse> {
        if self.execution_policy.mode() == Mode::Auto {
            Self::auto_response(false)
        } else {
            self.inner
                .request_confirmation(tool_id, tool_name, args)
                .await
        }
    }

    async fn confirm(
        &self,
        tool_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<bool, String> {
        self.inner.confirm(tool_id, approved, reason).await
    }

    async fn policy(&self) -> a3s_code_core::hitl::ConfirmationPolicy {
        self.inner.policy().await
    }

    async fn set_policy(&self, policy: a3s_code_core::hitl::ConfirmationPolicy) {
        let timeout_ms = policy.default_timeout_ms;
        self.inner
            .set_policy(policy.with_timeout(timeout_ms, a3s_code_core::hitl::TimeoutAction::Reject))
            .await;
    }

    async fn check_timeouts(&self) -> usize {
        self.inner.check_timeouts().await
    }

    async fn cancel(&self, tool_id: &str) -> bool {
        self.inner.cancel(tool_id).await
    }

    async fn expire(&self, tool_id: &str, _action: a3s_code_core::hitl::TimeoutAction) -> bool {
        self.inner
            .expire(tool_id, a3s_code_core::hitl::TimeoutAction::Reject)
            .await
    }

    async fn cancel_all(&self) -> usize {
        self.inner.cancel_all().await
    }

    async fn pending_confirmations(&self) -> Vec<a3s_code_core::hitl::PendingConfirmationInfo> {
        self.inner.pending_confirmation_details().await
    }
}

fn plan_tool_is_read_only(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read" | "grep" | "glob" | "ls" | "web_search" | "web_fetch"
    )
}

fn auto_tool_stays_inside_governed_boundaries(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read"
            | "grep"
            | "glob"
            | "ls"
            | "code_symbols"
            | "code_navigation"
            | "code_diagnostics"
            | "web_search"
            | "web_fetch"
            | "generate_object"
            | "search_skills"
            | "write"
            | "edit"
            | "patch"
            | "batch"
            | "program"
            | "task"
            | "parallel_task"
            | "dynamic_workflow"
            | "skill"
    ) || tool_name.starts_with("mcp__")
}

#[derive(Clone, Default)]
pub(super) struct DeepResearchReportToolGate {
    phase: Arc<AtomicU8>,
    network_disabled: Arc<std::sync::atomic::AtomicBool>,
    workspace: Arc<std::sync::RwLock<Option<PathBuf>>>,
}

impl DeepResearchReportToolGate {
    const INACTIVE: u8 = 0;
    const EVIDENCE: u8 = 1;
    const SYNTHESIS: u8 = 2;

    pub(super) fn set_evidence_scope(&self, evidence_scope: DeepResearchEvidenceScope) {
        self.network_disabled
            .store(!evidence_scope.network_enabled(), Ordering::SeqCst);
        self.phase.store(Self::EVIDENCE, Ordering::SeqCst);
    }

    pub(super) fn set_workspace(&self, workspace: &Path) {
        if let Ok(mut stored) = self.workspace.write() {
            *stored = workspace.canonicalize().ok();
        }
    }

    pub(super) fn workspace(&self) -> Option<PathBuf> {
        self.workspace
            .read()
            .ok()
            .and_then(|workspace| workspace.clone())
    }

    pub(super) fn reset(&self) {
        self.phase.store(Self::INACTIVE, Ordering::SeqCst);
        self.network_disabled.store(false, Ordering::SeqCst);
        if let Ok(mut workspace) = self.workspace.write() {
            *workspace = None;
        }
    }

    #[cfg(test)]
    pub(super) fn set_synthesis_only(&self) {
        self.phase.store(Self::SYNTHESIS, Ordering::SeqCst);
    }

    pub(super) fn evidence_collection(&self) -> bool {
        self.phase.load(Ordering::SeqCst) == Self::EVIDENCE
    }

    pub(super) fn synthesis_only(&self) -> bool {
        self.phase.load(Ordering::SeqCst) == Self::SYNTHESIS
    }

    pub(super) fn finalization_only(&self) -> bool {
        self.synthesis_only()
    }

    pub(super) fn network_disabled(&self) -> bool {
        self.network_disabled.load(Ordering::SeqCst)
    }
}

#[derive(Clone)]
pub(super) struct TuiHitlPermissionChecker {
    base: a3s_code_core::permissions::PermissionPolicy,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
    permission_grants: TuiPermissionGrants,
    execution_policy: TuiExecutionPolicy,
}

impl TuiHitlPermissionChecker {
    #[cfg(test)]
    pub(super) fn new(
        base: a3s_code_core::permissions::PermissionPolicy,
        deep_research_report_tool_gate: DeepResearchReportToolGate,
    ) -> Self {
        Self::with_grants(
            base,
            deep_research_report_tool_gate,
            TuiPermissionGrants::default(),
        )
    }

    #[cfg(test)]
    pub(super) fn with_grants(
        base: a3s_code_core::permissions::PermissionPolicy,
        deep_research_report_tool_gate: DeepResearchReportToolGate,
        permission_grants: TuiPermissionGrants,
    ) -> Self {
        Self::with_grants_and_execution(
            base,
            deep_research_report_tool_gate,
            permission_grants,
            TuiExecutionPolicy::default(),
        )
    }

    pub(super) fn with_grants_and_execution(
        base: a3s_code_core::permissions::PermissionPolicy,
        deep_research_report_tool_gate: DeepResearchReportToolGate,
        permission_grants: TuiPermissionGrants,
        execution_policy: TuiExecutionPolicy,
    ) -> Self {
        Self {
            base,
            deep_research_report_tool_gate,
            permission_grants,
            execution_policy,
        }
    }

    pub(super) fn check_tool(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> a3s_code_core::permissions::PermissionDecision {
        let base = self.base.check(tool_name, args);
        if matches!(base, a3s_code_core::permissions::PermissionDecision::Deny) {
            return base;
        }

        let evidence_collection = self.deep_research_report_tool_gate.evidence_collection();
        let tool = tool_name.to_ascii_lowercase();
        if tool.starts_with("mcp__use_") {
            // The primary model cannot see this route. Returning the neutral
            // parent Allow lets an explicitly scoped Use worker preserve its
            // stricter per-tool Allow/Ask/Deny decision during delegation.
            return a3s_code_core::permissions::PermissionDecision::Allow;
        }
        if self.deep_research_report_tool_gate.synthesis_only() {
            return a3s_code_core::permissions::PermissionDecision::Deny;
        }

        let boundary_workspace = self
            .deep_research_report_tool_gate
            .workspace()
            .unwrap_or_else(|| self.execution_policy.workspace.as_ref().clone());
        let hard_guardrail = a3s_code_core::permissions::InteractiveToolGuardrail::default()
            .with_workspace(boundary_workspace);
        if a3s_code_core::permissions::PermissionChecker::check(&hard_guardrail, &tool, args)
            == a3s_code_core::permissions::PermissionDecision::Deny
        {
            return a3s_code_core::permissions::PermissionDecision::Deny;
        }
        let guarded_bash_decision = (tool == "bash").then(|| {
            let mode = match self.execution_policy.mode() {
                Mode::Default => crate::host_command_guardrail::HostCommandMode::Default,
                Mode::Plan => crate::host_command_guardrail::HostCommandMode::Plan,
                Mode::Auto => crate::host_command_guardrail::HostCommandMode::Auto,
            };
            crate::host_command_guardrail::host_bash_decision(&hard_guardrail, mode, args)
        });
        if guarded_bash_decision == Some(a3s_code_core::permissions::PermissionDecision::Deny) {
            return a3s_code_core::permissions::PermissionDecision::Deny;
        }
        let protected_workspace_metadata = targets_protected_workspace_metadata(&tool, args);
        if !evidence_collection {
            if tool.starts_with("mcp__use_") && self.execution_policy.mode() != Mode::Plan {
                // The primary model does not receive these definitions. The
                // dedicated Use worker explicitly opts into them, and each MCP
                // wrapper may still escalate this base authorization to HITL.
                return base;
            }
            match self.execution_policy.mode() {
                // Auto never enters HITL. Host shell access fails closed.
                Mode::Auto => {
                    if protected_workspace_metadata {
                        return a3s_code_core::permissions::PermissionDecision::Deny;
                    }
                    if tool == "bash" {
                        return guarded_bash_decision
                            .unwrap_or(a3s_code_core::permissions::PermissionDecision::Deny);
                    }
                    if tool == "git" {
                        return match a3s_code_core::permissions::InteractiveToolGuardrail::risk_decision(
                            &tool, args,
                        ) {
                            a3s_code_core::permissions::PermissionDecision::Allow => {
                                a3s_code_core::permissions::PermissionDecision::Allow
                            }
                            a3s_code_core::permissions::PermissionDecision::Ask
                            | a3s_code_core::permissions::PermissionDecision::Deny => {
                                a3s_code_core::permissions::PermissionDecision::Deny
                            }
                        };
                    }
                    return if auto_tool_stays_inside_governed_boundaries(&tool) {
                        // Orchestrators re-enter this checker for every nested
                        // call. MCP wrappers conservatively request a second
                        // confirmation unless they advertise a closed-world,
                        // read-only operation; Auto converts that escalation
                        // into a denial before an event is emitted.
                        a3s_code_core::permissions::PermissionDecision::Allow
                    } else {
                        // Unknown dynamic tools and explicit remote runtimes
                        // have no proven local boundary. Auto is non-interactive,
                        // so they are denied rather than prompted or trusted.
                        a3s_code_core::permissions::PermissionDecision::Deny
                    };
                }
                // Plan is a true read-only boundary. A remembered grant must
                // never turn a planning turn into an implementation turn.
                Mode::Plan => {
                    return if plan_tool_is_read_only(&tool)
                        && matches!(base, a3s_code_core::permissions::PermissionDecision::Allow)
                    {
                        a3s_code_core::permissions::PermissionDecision::Allow
                    } else {
                        a3s_code_core::permissions::PermissionDecision::Deny
                    };
                }
                Mode::Default => {}
            }
            if self.permission_grants.allows(&tool, args) {
                return a3s_code_core::permissions::PermissionDecision::Allow;
            }
        }
        let decision = if !evidence_collection && self.execution_policy.mode() == Mode::Default {
            match tool.as_str() {
                // Built-in file tools enforce the workspace path boundary.
                "write" | "edit" | "patch" => {
                    if protected_workspace_metadata {
                        a3s_code_core::permissions::PermissionDecision::Ask
                    } else {
                        a3s_code_core::permissions::PermissionDecision::Allow
                    }
                }
                // The shared Rust guardrail silently admits only commands it
                // can prove read-only. Unproven host work retains HITL.
                "bash" => guarded_bash_decision
                    .unwrap_or(a3s_code_core::permissions::PermissionDecision::Deny),
                // These are governed control-plane wrappers. Their nested tool
                // calls pass through the same checker again.
                "batch" | "program" | "task" | "parallel_task" | "dynamic_workflow" | "skill" => {
                    a3s_code_core::permissions::PermissionDecision::Allow
                }
                // MCP annotations own read-only versus side-effect escalation.
                name if name.starts_with("mcp__") => {
                    a3s_code_core::permissions::PermissionDecision::Allow
                }
                _ => {
                    a3s_code_core::permissions::InteractiveToolGuardrail::risk_decision(&tool, args)
                }
            }
        } else {
            a3s_code_core::permissions::InteractiveToolGuardrail::risk_decision(&tool, args)
        };
        if !evidence_collection {
            return decision;
        }

        if self.deep_research_report_tool_gate.network_disabled()
            && matches!(tool.as_str(), "web_search" | "web_fetch")
        {
            return a3s_code_core::permissions::PermissionDecision::Deny;
        }
        if matches!(tool.as_str(), "bash" | "git") {
            return a3s_code_core::permissions::PermissionDecision::Deny;
        }
        if matches!(tool.as_str(), "write" | "edit") {
            return a3s_code_core::permissions::PermissionDecision::Deny;
        }
        if matches!(
            tool.as_str(),
            "parallel_task" | "dynamic_workflow" | "generate_object"
        ) && matches!(
            decision,
            a3s_code_core::permissions::PermissionDecision::Ask
        ) {
            return a3s_code_core::permissions::PermissionDecision::Allow;
        }

        // DeepResearch runs without interactive side effects. Anything that
        // normally needs confirmation is denied rather than silently approved.
        if matches!(
            decision,
            a3s_code_core::permissions::PermissionDecision::Ask
        ) {
            a3s_code_core::permissions::PermissionDecision::Deny
        } else {
            decision
        }
    }
}

impl a3s_code_core::permissions::PermissionChecker for TuiHitlPermissionChecker {
    fn snapshot_for_run(&self) -> Option<Arc<dyn a3s_code_core::permissions::PermissionChecker>> {
        Some(Arc::new(Self {
            base: self.base.clone(),
            deep_research_report_tool_gate: self.deep_research_report_tool_gate.clone(),
            permission_grants: self.permission_grants.clone(),
            execution_policy: self.execution_policy.snapshot(),
        }))
    }

    fn expose_to_model(&self, tool_name: &str) -> bool {
        if tool_name.to_ascii_lowercase().starts_with("mcp__use_") {
            return false;
        }
        if !a3s_code_core::permissions::PermissionChecker::expose_to_model(&self.base, tool_name) {
            return false;
        }
        let tool = tool_name.to_ascii_lowercase();
        if self.deep_research_report_tool_gate.synthesis_only() {
            return false;
        }
        if self.deep_research_report_tool_gate.evidence_collection() {
            return match tool.as_str() {
                "read" | "grep" | "glob" | "ls" => true,
                "web_search" | "web_fetch" => {
                    !self.deep_research_report_tool_gate.network_disabled()
                }
                _ => false,
            };
        }
        if self.execution_policy.mode() == Mode::Plan {
            return plan_tool_is_read_only(&tool);
        }
        !tool.starts_with("mcp__use_")
    }

    fn check(
        &self,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> a3s_code_core::permissions::PermissionDecision {
        self.check_tool(tool_name, args)
    }
}

pub(super) fn instant_from_epoch_ms(epoch_ms: u64) -> Instant {
    let now = Instant::now();
    if epoch_ms == 0 {
        return now;
    }
    let wall_now_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(epoch_ms);
    let age_ms = wall_now_ms.saturating_sub(epoch_ms);
    now.checked_sub(Duration::from_millis(age_ms))
        .unwrap_or(now)
}

pub(super) fn touch_workspace_file_path_for_manifest(
    manifest: &LocalWorkspaceManifest,
    workspace: &str,
    path: &Path,
) {
    let root = Path::new(workspace);
    if let Ok(relative) = path.strip_prefix(root) {
        if let Some(path) = relative.to_str() {
            manifest.touch_file(path);
        }
    }
}

#[cfg(test)]
#[path = "permissions/execution_policy_tests.rs"]
mod execution_policy_tests;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum RuntimeEvidenceMode {
    Any,
    ParallelReportView,
}

#[derive(Clone)]
pub(super) struct RuntimeExpectation {
    label: String,
    policy: RuntimePolicy,
    evidence_mode: RuntimeEvidenceMode,
    runtime_tool: bool,
    parallel_work: bool,
    remote_view: bool,
    warned_missing: bool,
}

impl RuntimeExpectation {
    pub(super) fn required(label: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            policy: RuntimePolicy::Required,
            evidence_mode: RuntimeEvidenceMode::Any,
            runtime_tool: false,
            parallel_work: false,
            remote_view: false,
            warned_missing: false,
        }
    }

    pub(super) fn required_report_view(label: impl Into<String>) -> Self {
        Self {
            evidence_mode: RuntimeEvidenceMode::ParallelReportView,
            ..Self::required(label)
        }
    }

    pub(super) fn record_tool(&mut self, name: &str) {
        match name {
            "runtime" => self.runtime_tool = true,
            "dynamic_workflow" | "parallel_task" | "task" => self.parallel_work = true,
            _ => {}
        }
    }

    pub(super) fn record_parallel_work(&mut self) {
        self.parallel_work = true;
    }

    pub(super) fn record_remote_view(&mut self) {
        self.remote_view = true;
    }

    pub(super) fn has_parallel_evidence(&self) -> bool {
        self.runtime_tool || self.parallel_work
    }

    pub(super) fn is_satisfied(&self) -> bool {
        match self.evidence_mode {
            RuntimeEvidenceMode::Any => self.has_parallel_evidence() || self.remote_view,
            RuntimeEvidenceMode::ParallelReportView => {
                self.has_parallel_evidence() && self.remote_view
            }
        }
    }

    pub(super) fn missing_expectation(&self) -> String {
        match self.evidence_mode {
            RuntimeEvidenceMode::Any => {
                "expected `dynamic_workflow`, `runtime`, `parallel_task`, or an OS shaped `.view`/`viewUrl` response"
                    .to_string()
            }
            RuntimeEvidenceMode::ParallelReportView => match (self.has_parallel_evidence(), self.remote_view) {
                (false, false) => {
                    "expected `dynamic_workflow`/OS Runtime/`parallel_task` fan-out plus an OS shaped `.view`/`viewUrl` report response".to_string()
                }
                (false, true) => {
                    "expected `dynamic_workflow`/OS Runtime/`parallel_task` fan-out before the report view".to_string()
                }
                (true, false) => {
                    "expected an OS shaped `.view`/`viewUrl` response for the report".to_string()
                }
                (true, true) => unreachable!("satisfied expectations are filtered before warning"),
            },
        }
    }

    pub(super) fn missing_warning(&mut self) -> Option<String> {
        if self.policy != RuntimePolicy::Required || self.is_satisfied() || self.warned_missing {
            return None;
        }
        self.warned_missing = true;
        Some(format!(
            "  Runtime evidence missing for {} - {} before the final answer",
            self.label,
            self.missing_expectation()
        ))
    }

    pub(super) fn corrective_prompt(&self) -> Option<String> {
        if self.policy != RuntimePolicy::Required || self.is_satisfied() {
            return None;
        }
        Some(format!(
            "The previous turn ended without the required OS Runtime evidence for {}: {}. \
             Continue the same task, explicitly use `dynamic_workflow` first; inside it use \
             the signed-in `runtime` tool or a host-side `parallel_task` step as required, \
             create or surface the shaped OS `.view`/`viewUrl` report response when required, \
             and only then give the final answer. If the OS capability is unavailable, explain exactly \
             which OS endpoint or response field is missing and provide local report artifact paths.",
            self.label,
            self.missing_expectation()
        ))
    }
}
