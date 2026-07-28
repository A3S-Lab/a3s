use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::config::CodeConfig;
use a3s_code_core::skills::SkillRegistry;
use a3s_code_core::{Agent, AgentSession, SessionOptions, WorkspaceServices};
use a3s_deep_research::engine::{
    DeepResearchCancellation, DeepResearchEngine, DeepResearchEngineError, DeepResearchEvent,
    DeepResearchRequest, DeepResearchRequestLimits, DeepResearchResult, EngineLimits,
    EvidenceScope, WorkspaceSourceHint,
};
use tokio::sync::{mpsc, watch};

use super::journal::CodeDeepResearchJournal;
use super::runtime::CodeDeepResearchRuntime;
use super::CodeDeepResearchEvent;

const EVENT_CHANNEL_CAPACITY: usize = 512;
const CANCELLATION_GRACE: Duration = Duration::from_secs(5);
const RESEARCH_TOOL_EXEC_TIMEOUT_MS: u64 = 30 * 60 * 1000;
const RESEARCH_DUPLICATE_TOOL_CALL_THRESHOLD: u32 = 12;
const BOOTSTRAP_STAGE_TIMEOUT_MS: u64 = 150_000;
const PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS: u64 = 600_000;
const REPORT_ATTEMPT_TIMEOUT_MS: u64 = 240_000;
const REPORT_MAX_ATTEMPTS: u8 = 2;
const DURABLE_GENERATION_GRACE_MS: u64 = 15_000;
const REPORT_STAGE_TIMEOUT_MS: u64 =
    REPORT_ATTEMPT_TIMEOUT_MS * REPORT_MAX_ATTEMPTS as u64 + DURABLE_GENERATION_GRACE_MS;

pub(crate) struct CodeDeepResearchRunner {
    workspace: PathBuf,
    code_config: CodeConfig,
    memory_dir: PathBuf,
}

pub(crate) struct CodeDeepResearchLaunch {
    pub(crate) request: DeepResearchRequest,
    pub(crate) skill_names: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct CodeDeepResearchRunnerBudget {
    pub(crate) local_max_steps: usize,
    pub(crate) max_tool_calls: usize,
    pub(crate) max_output_bytes: usize,
}

#[derive(Debug)]
pub(crate) enum CodeDeepResearchRunExit {
    Completed(Box<DeepResearchResult>),
    Cancelled,
}

pub(crate) struct CodeDeepResearchRunHandle {
    run_id: String,
    session: Arc<AgentSession>,
    cancellation: DeepResearchCancellation,
    events_rx: Option<mpsc::Receiver<CodeDeepResearchEvent>>,
    events_tx: mpsc::Sender<CodeDeepResearchEvent>,
    journal: Arc<CodeDeepResearchJournal>,
    completion_rx: watch::Receiver<bool>,
    task: Option<tokio::task::JoinHandle<Result<CodeDeepResearchRunExit, String>>>,
}

impl CodeDeepResearchRunner {
    pub(crate) fn new(
        workspace: impl Into<PathBuf>,
        code_config: CodeConfig,
        memory_dir: PathBuf,
    ) -> Self {
        Self {
            workspace: workspace.into(),
            code_config,
            memory_dir,
        }
    }

    pub(crate) async fn start<F>(
        self,
        launch: CodeDeepResearchLaunch,
        resolve_llm_client: F,
    ) -> Result<CodeDeepResearchRunHandle, String>
    where
        F: FnOnce(
            &CodeConfig,
            &SessionOptions,
            &str,
        ) -> Result<Arc<dyn a3s_code_core::llm::LlmClient>, String>,
    {
        if !launch.skill_names.is_empty() {
            return Err(
                "DeepResearch skills are not supported by the typed runner; remove selected skills"
                    .to_string(),
            );
        }
        launch.request.validate()?;
        preflight_workspace_source_hints(&self.workspace, &launch.request.workspace_source_hints)
            .await?;
        let session = build_isolated_research_session_with_resolver(
            &self.workspace,
            self.code_config,
            self.memory_dir,
            launch.request.evidence_scope,
            &launch.request.run_id,
            resolve_llm_client,
        )
        .await
        .map_err(|error| error.to_string())?;
        let journal =
            match CodeDeepResearchJournal::create(&self.workspace, &launch.request.run_id).await {
                Ok(journal) => Arc::new(journal),
                Err(error) => {
                    session.close().await;
                    return Err(error);
                }
            };
        let (events_tx, events_rx) = mpsc::channel(EVENT_CHANNEL_CAPACITY);
        let runtime = Arc::new(CodeDeepResearchRuntime::new(
            Arc::clone(&session),
            launch.request.run_id.clone(),
            Arc::clone(&journal),
            events_tx.clone(),
        ));
        let cancellation = DeepResearchCancellation::new();
        let task_cancellation = cancellation.clone();
        let run_id = launch.request.run_id.clone();
        let request = launch.request;
        let (completion_tx, completion_rx) = watch::channel(false);
        let task = tokio::spawn(async move {
            let engine = DeepResearchEngine::new(
                runtime.as_ref(),
                runtime.as_ref(),
                runtime.as_ref(),
                runtime.as_ref(),
            )
            .with_limits(engine_limits());
            let result = match engine.execute_request(request, task_cancellation).await {
                Ok(result) => Ok(CodeDeepResearchRunExit::Completed(Box::new(result))),
                Err(DeepResearchEngineError::Cancelled) => Ok(CodeDeepResearchRunExit::Cancelled),
                Err(error) => Err(error.to_string()),
            };
            let _ = completion_tx.send(true);
            result
        });
        Ok(CodeDeepResearchRunHandle {
            run_id,
            session,
            cancellation,
            events_rx: Some(events_rx),
            events_tx,
            journal,
            completion_rx,
            task: Some(task),
        })
    }
}

impl CodeDeepResearchRunHandle {
    pub(crate) fn take_events(&mut self) -> Option<mpsc::Receiver<CodeDeepResearchEvent>> {
        self.events_rx.take()
    }

    pub(crate) fn completion_signal(&self) -> watch::Receiver<bool> {
        self.completion_rx.clone()
    }

    pub(crate) async fn settle(mut self) -> Result<CodeDeepResearchRunExit, String> {
        let task = self
            .task
            .take()
            .ok_or_else(|| "DeepResearch root task was already settled".to_string())?;
        let result = task
            .await
            .map_err(|error| format!("DeepResearch root task failed: {error}"))
            .and_then(|result| result);
        self.session.close().await;
        result
    }

    pub(crate) async fn cancel_and_settle(self) -> Result<CodeDeepResearchRunExit, String> {
        self.cancel_and_settle_with_grace(CANCELLATION_GRACE).await
    }

    async fn cancel_and_settle_with_grace(
        mut self,
        grace: Duration,
    ) -> Result<CodeDeepResearchRunExit, String> {
        self.cancellation.cancel();
        let mut task = self
            .task
            .take()
            .ok_or_else(|| "DeepResearch root task was already settled".to_string())?;
        let result = match tokio::time::timeout(grace, &mut task).await {
            Ok(joined) => joined
                .map_err(|error| format!("DeepResearch root task failed: {error}"))
                .and_then(|result| result),
            Err(_) => {
                task.abort();
                let _ = task.await;
                let event = DeepResearchEvent::RunCancelled {
                    run_id: self.run_id.clone(),
                };
                let journal_result = self.journal.append(&event).await;
                let _ = self
                    .events_tx
                    .try_send(CodeDeepResearchEvent::Engine(event));
                journal_result.map(|()| CodeDeepResearchRunExit::Cancelled)
            }
        };
        self.session.close().await;
        result
    }
}

impl Drop for CodeDeepResearchRunHandle {
    fn drop(&mut self) {
        if let Some(task) = self.task.as_ref() {
            self.cancellation.cancel();
            task.abort();
        }
    }
}

pub(crate) fn build_code_deep_research_request(
    run_id: Option<String>,
    query: &str,
    evidence_scope: EvidenceScope,
    budget: CodeDeepResearchRunnerBudget,
    workspace_source_hints: Vec<WorkspaceSourceHint>,
) -> Result<DeepResearchRequest, String> {
    let run_id = run_id.unwrap_or_else(new_research_run_id);
    let workflow_timeout_ms = if evidence_scope.network_enabled() {
        PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS
    } else {
        210_000
    };
    let limits = DeepResearchRequestLimits {
        max_tracks: 4,
        local_max_steps: u8::try_from(budget.local_max_steps.clamp(1, 4))
            .map_err(|_| "DeepResearch local step budget overflowed".to_string())?,
        workflow_timeout_ms,
        max_tool_calls: u16::try_from(budget.max_tool_calls.clamp(4, 240))
            .map_err(|_| "DeepResearch tool-call budget overflowed".to_string())?,
        max_output_bytes: budget.max_output_bytes.clamp(256 * 1024, 2 * 1024 * 1024),
    };
    let request = DeepResearchRequest::new(run_id, query, evidence_scope)
        .with_workspace_source_hints(workspace_source_hints)
        .with_limits(limits);
    request.validate()?;
    Ok(request)
}

fn new_research_run_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!(
        "research-{nanos:016x}-{:08x}-{:016x}",
        std::process::id(),
        rand::random::<u64>()
    )
}

fn engine_limits() -> EngineLimits {
    EngineLimits {
        bootstrap_stage_timeout_ms: BOOTSTRAP_STAGE_TIMEOUT_MS,
        planned_retrieval_stage_timeout_ms: PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS,
        report_attempt_timeout_ms: REPORT_ATTEMPT_TIMEOUT_MS,
        report_stage_timeout_ms: REPORT_STAGE_TIMEOUT_MS,
        report_max_attempts: REPORT_MAX_ATTEMPTS,
        durable_generation_grace_ms: DURABLE_GENERATION_GRACE_MS,
        ..EngineLimits::default()
    }
}

async fn preflight_workspace_source_hints(
    workspace: &Path,
    hints: &[WorkspaceSourceHint],
) -> Result<(), String> {
    if hints.is_empty() {
        return Ok(());
    }
    let root = tokio::fs::canonicalize(workspace)
        .await
        .map_err(|error| format!("resolve DeepResearch workspace: {error}"))?;
    for hint in hints {
        let candidate = root.join(&hint.path);
        let metadata = tokio::fs::symlink_metadata(&candidate)
            .await
            .map_err(|error| {
                format!(
                    "preflight DeepResearch workspace source hint `{}`: {error}",
                    hint.path
                )
            })?;
        if metadata.file_type().is_symlink() || !metadata.is_file() || metadata.len() == 0 {
            return Err(format!(
                "DeepResearch workspace source hint `{}` must be a non-empty plain file",
                hint.path
            ));
        }
        let canonical = tokio::fs::canonicalize(&candidate).await.map_err(|error| {
            format!(
                "resolve DeepResearch workspace source hint `{}`: {error}",
                hint.path
            )
        })?;
        if !canonical.starts_with(&root) {
            return Err(format!(
                "DeepResearch workspace source hint `{}` escaped the workspace",
                hint.path
            ));
        }
    }
    Ok(())
}

fn deep_research_permission_policy(
    evidence_scope: EvidenceScope,
) -> a3s_code_core::permissions::PermissionPolicy {
    let mut allowed = vec![
        "Read(*)", "Grep(*)", "Glob(*)", "LS(*)", "read(*)", "grep(*)", "glob(*)", "ls(*)",
    ];
    if evidence_scope.network_enabled() {
        allowed.extend(["web_search(*)", "web_fetch(*)"]);
    }
    let mut policy = a3s_code_core::permissions::PermissionPolicy::new()
        .deny_all(&[
            "Write(/**)",
            "Edit(/**)",
            "Write(**/../**)",
            "Edit(**/../**)",
        ])
        .allow_all(&allowed);
    policy.default_decision = a3s_code_core::permissions::PermissionDecision::Deny;
    policy
}

pub(crate) async fn build_isolated_research_session_with_resolver<F>(
    workspace: &Path,
    mut code_config: CodeConfig,
    memory_dir: PathBuf,
    evidence_scope: EvidenceScope,
    session_id: &str,
    resolve_llm_client: F,
) -> anyhow::Result<Arc<AgentSession>>
where
    F: FnOnce(
        &CodeConfig,
        &SessionOptions,
        &str,
    ) -> Result<Arc<dyn a3s_code_core::llm::LlmClient>, String>,
{
    let workspace = workspace.to_string_lossy().to_string();
    let permission_policy = deep_research_permission_policy(evidence_scope);
    code_config.skill_dirs.clear();
    let opts = SessionOptions::new()
        .with_session_id(session_id)
        .with_confirmation_policy(a3s_code_core::hitl::ConfirmationPolicy::default())
        .with_permission_policy(permission_policy)
        .with_tool_timeout(RESEARCH_TOOL_EXEC_TIMEOUT_MS)
        .with_duplicate_tool_call_threshold(RESEARCH_DUPLICATE_TOOL_CALL_THRESHOLD)
        .with_file_memory(memory_dir)
        .with_workspace_backend(WorkspaceServices::local_with_manifest(&workspace))
        .with_skill_registry(Arc::new(SkillRegistry::new()))
        .with_continuation(false)
        .with_max_parallel_tasks(1)
        .with_auto_delegation_enabled(false)
        .with_auto_parallel_delegation(false)
        .with_manual_delegation_enabled(true);
    let llm_client = resolve_llm_client(&code_config, &opts, session_id)
        .map_err(|error| anyhow::anyhow!("failed to resolve DeepResearch model: {error}"))?;
    let agent = Agent::from_config(code_config)
        .await
        .map_err(|error| anyhow::anyhow!("failed to load DeepResearch agent: {error}"))?;
    let session = agent
        .session_async(workspace, Some(opts.with_llm_client(llm_client)))
        .await?;
    session.register_dynamic_workflow_runtime()?;
    Ok(Arc::new(session))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{budget_plan_for_effort_index, BudgetWorkload, DEFAULT_TUI_EFFORT_INDEX};

    #[test]
    fn shared_request_normalization_closes_local_only_network_access() {
        let budget = budget_plan_for_effort_index(
            DEFAULT_TUI_EFFORT_INDEX,
            None,
            BudgetWorkload::DeepResearch,
        );
        let budget = CodeDeepResearchRunnerBudget {
            local_max_steps: budget.deep_research_child_steps,
            max_tool_calls: budget.workflow_max_tool_calls,
            max_output_bytes: budget.workflow_max_output_bytes,
        };
        let request = build_code_deep_research_request(
            Some("normalized-run".to_string()),
            "Inspect the workspace",
            EvidenceScope::LocalOnly,
            budget,
            Vec::new(),
        )
        .expect("request");
        let arguments = request.to_workflow_arguments().expect("workflow arguments");

        assert_eq!(arguments["input"]["evidence_scope"], "local_only");
        assert_eq!(arguments["input"]["local_max_steps"], 4);
        assert_eq!(arguments["limits"]["maxToolCalls"], 240);
        assert_eq!(arguments["limits"]["maxOutputBytes"], 2 * 1024 * 1024);
    }

    #[tokio::test]
    async fn source_hint_preflight_rejects_missing_and_empty_files() {
        let workspace = tempfile::tempdir().expect("workspace");
        let missing = vec![WorkspaceSourceHint::new("missing.md")];
        assert!(preflight_workspace_source_hints(workspace.path(), &missing)
            .await
            .is_err());

        std::fs::write(workspace.path().join("empty.md"), []).expect("empty source");
        let empty = vec![WorkspaceSourceHint::new("empty.md")];
        assert!(preflight_workspace_source_hints(workspace.path(), &empty)
            .await
            .is_err());
    }
}
