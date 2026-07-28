//! Non-interactive DeepResearch execution and report synthesis.

use std::path::{Path, PathBuf};
#[cfg(test)]
use std::sync::Arc;

use a3s_code_core::config::CodeConfig;
#[cfg(test)]
use a3s_code_core::{AgentSession, SessionOptions};
use a3s_deep_research::engine::{
    DeepResearchEvent, DeepResearchLifecycle, EvidenceScope, PublicationOutcome, ResearchStage,
};
use a3s_deep_research::report::{
    clean_deep_research_final_text_from_artifacts, DeepResearchPublicationQuality,
};

use crate::budget::{
    budget_plan_for_effort_index, BudgetPlan, BudgetWorkload, DEFAULT_TUI_EFFORT_INDEX,
};
use crate::research::{
    build_code_deep_research_request, CodeDeepResearchEvent, CodeDeepResearchLaunch,
    CodeDeepResearchRunExit, CodeDeepResearchRunner, CodeDeepResearchRunnerBudget,
};

pub(crate) fn deep_research_default_budget() -> BudgetPlan {
    budget_plan_for_effort_index(DEFAULT_TUI_EFFORT_INDEX, None, BudgetWorkload::DeepResearch)
}

#[cfg(test)]
pub(crate) fn deep_research_workflow_args(query: &str) -> serde_json::Value {
    deep_research_workflow_args_for_scope(query, None)
}

#[cfg(test)]
fn deep_research_workflow_args_for_scope(
    query: &str,
    evidence_scope: Option<crate::tui::DeepResearchEvidenceScope>,
) -> serde_json::Value {
    let request = build_code_deep_research_request(
        Some("cli-workflow-contract-test".to_string()),
        query,
        engine_evidence_scope(evidence_scope),
        runner_budget(deep_research_default_budget()),
        Vec::new(),
    )
    .expect("DeepResearch test request");
    request
        .to_workflow_arguments()
        .expect("DeepResearch workflow arguments")
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeepResearchCliOptions {
    query: String,
    evidence_scope: Option<crate::tui::DeepResearchEvidenceScope>,
}

fn parse_deepresearch_args(args: &[String]) -> anyhow::Result<DeepResearchCliOptions> {
    let mut evidence_scope = None;
    let mut query_parts = Vec::new();
    for arg in args {
        match arg.as_str() {
            "--local" | "--os" => {
                anyhow::bail!(
                    "DeepResearch runtime selection has been removed; use --web or --local-only to choose the evidence scope"
                )
            }
            "--local-only" | "--offline" => {
                if evidence_scope == Some(crate::tui::DeepResearchEvidenceScope::WebAndWorkspace) {
                    anyhow::bail!("--local-only conflicts with --web");
                }
                evidence_scope = Some(crate::tui::DeepResearchEvidenceScope::LocalOnly);
            }
            "--web" => {
                if evidence_scope == Some(crate::tui::DeepResearchEvidenceScope::LocalOnly) {
                    anyhow::bail!("--web conflicts with --local-only");
                }
                evidence_scope = Some(crate::tui::DeepResearchEvidenceScope::WebAndWorkspace);
            }
            "-h" | "--help" | "help" => {
                anyhow::bail!("usage: a3s code deepresearch [--local-only|--web] <query>");
            }
            value if value.starts_with('-') => {
                anyhow::bail!("unknown a3s code deepresearch option `{value}`")
            }
            value => query_parts.push(value.to_string()),
        }
    }
    let query = query_parts.join(" ").trim().to_string();
    if query.is_empty() {
        anyhow::bail!("usage: a3s code deepresearch [--local-only|--web] <query>");
    }
    Ok(DeepResearchCliOptions {
        query,
        evidence_scope,
    })
}

pub(crate) async fn execute_deepresearch_in(
    args: &[String],
    workspace: &Path,
    code_config: CodeConfig,
    memory_dir: PathBuf,
) -> anyhow::Result<DeepResearchReportSynthesis> {
    let opts = parse_deepresearch_args(args)?;
    execute_deepresearch_query_in(
        &opts.query,
        opts.evidence_scope,
        deep_research_default_budget(),
        workspace,
        code_config,
        memory_dir,
    )
    .await
}

pub(crate) async fn execute_deepresearch_query_in(
    query: &str,
    evidence_scope: Option<crate::tui::DeepResearchEvidenceScope>,
    budget: BudgetPlan,
    workspace: &Path,
    code_config: CodeConfig,
    memory_dir: PathBuf,
) -> anyhow::Result<DeepResearchReportSynthesis> {
    let query = query.trim();
    if query.is_empty() {
        anyhow::bail!("DeepResearch query must not be empty");
    }
    let request = build_code_deep_research_request(
        None,
        query,
        engine_evidence_scope(evidence_scope),
        runner_budget(budget),
        Vec::new(),
    )
    .map_err(anyhow::Error::msg)?;
    let runner = CodeDeepResearchRunner::new(workspace, code_config, memory_dir);
    eprintln!("deepresearch: starting typed evidence-first run…");
    let mut handle = runner
        .start(
            CodeDeepResearchLaunch {
                request,
                skill_names: Vec::new(),
            },
            crate::session_llm::resolve_session_llm_client,
        )
        .await
        .map_err(anyhow::Error::msg)?;
    let events = handle
        .take_events()
        .ok_or_else(|| anyhow::anyhow!("DeepResearch event stream was already consumed"))?;
    let event_drain = tokio::spawn(drain_deep_research_events(events));
    let settled = handle.settle().await;
    event_drain
        .await
        .map_err(|error| anyhow::anyhow!("DeepResearch event stream failed: {error}"))?;
    let result = match settled.map_err(anyhow::Error::msg)? {
        CodeDeepResearchRunExit::Completed(result) => *result,
        CodeDeepResearchRunExit::Cancelled => {
            anyhow::bail!("DeepResearch run was cancelled before publication")
        }
    };
    if result.lifecycle != DeepResearchLifecycle::Completed {
        anyhow::bail!(
            "DeepResearch returned a non-terminal lifecycle after settlement: {:?}",
            result.lifecycle
        );
    }
    let text = clean_deep_research_final_text_from_artifacts(&result.artifacts, workspace)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "DeepResearch published unreadable report artifacts for run {}",
                result.run_id
            )
        })?;
    Ok(DeepResearchReportSynthesis {
        run_id: result.run_id,
        text,
        artifacts: result.artifacts,
        status: result.publication,
        quality: result.quality,
    })
}

fn engine_evidence_scope(
    evidence_scope: Option<crate::tui::DeepResearchEvidenceScope>,
) -> EvidenceScope {
    match evidence_scope.unwrap_or(crate::tui::DeepResearchEvidenceScope::WebAndWorkspace) {
        crate::tui::DeepResearchEvidenceScope::LocalOnly => EvidenceScope::LocalOnly,
        crate::tui::DeepResearchEvidenceScope::WebAndWorkspace => EvidenceScope::WebAndWorkspace,
    }
}

fn runner_budget(budget: BudgetPlan) -> CodeDeepResearchRunnerBudget {
    CodeDeepResearchRunnerBudget {
        local_max_steps: budget.deep_research_child_steps,
        max_tool_calls: budget.workflow_max_tool_calls,
        max_output_bytes: budget.workflow_max_output_bytes,
    }
}

async fn drain_deep_research_events(
    mut events: tokio::sync::mpsc::Receiver<CodeDeepResearchEvent>,
) {
    while let Some(event) = events.recv().await {
        match event {
            CodeDeepResearchEvent::Engine(DeepResearchEvent::StageStarted { stage, .. }) => {
                eprintln!("deepresearch: {}…", research_stage_label(stage));
            }
            CodeDeepResearchEvent::Engine(DeepResearchEvent::StageDegraded {
                stage,
                reason,
                ..
            }) => {
                eprintln!(
                    "deepresearch: {} degraded: {reason}",
                    research_stage_label(stage)
                );
            }
            CodeDeepResearchEvent::Engine(DeepResearchEvent::RunFailed { message, .. }) => {
                eprintln!("deepresearch: run failed: {message}");
            }
            CodeDeepResearchEvent::Engine(_) | CodeDeepResearchEvent::Agent(_) => {}
        }
    }
}

fn research_stage_label(stage: ResearchStage) -> &'static str {
    match stage {
        ResearchStage::Planning => "planning research questions",
        ResearchStage::BootstrapRetrieval => "gathering initial evidence",
        ResearchStage::PlannedRetrieval => "retrieving planned evidence",
        ResearchStage::SourcePublication => "publishing source-backed findings",
        ResearchStage::ReportGeneration => "synthesizing the report",
        ResearchStage::FinalPublication => "publishing final artifacts",
    }
}

pub(crate) type DeepResearchReportStatus = PublicationOutcome;
pub(crate) type ResearchReportArtifacts = a3s_deep_research::report::ResearchReportArtifacts;

#[derive(Debug)]
pub(crate) struct DeepResearchReportSynthesis {
    pub(crate) run_id: String,
    pub(crate) text: String,
    pub(crate) artifacts: ResearchReportArtifacts,
    pub(crate) status: DeepResearchReportStatus,
    pub(crate) quality: DeepResearchPublicationQuality,
}

#[cfg(test)]
async fn build_deepresearch_session(
    workspace: &str,
    code_config: CodeConfig,
    memory_dir: PathBuf,
) -> anyhow::Result<Arc<AgentSession>> {
    let session_id = deep_research_execution_id();
    crate::research::build_isolated_research_session_with_resolver(
        Path::new(workspace),
        code_config,
        memory_dir,
        EvidenceScope::WebAndWorkspace,
        &session_id,
        crate::session_llm::resolve_session_llm_client,
    )
    .await
}

#[cfg(test)]
fn deep_research_execution_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("research-{nanos:016x}-{:x}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;
    // Frozen replay tests stay isolated from production control flow.
    #[path = "baseline.rs"]
    mod baseline;
    #[path = "cli.rs"]
    mod cli;
    #[path = "workflow.rs"]
    mod workflow;
    use a3s_code_core::llm::{
        ContentBlock, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage, ToolDefinition,
    };
    use async_trait::async_trait;
    use std::collections::VecDeque;
    use std::sync::Mutex;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    struct ScriptedLlmClient {
        responses: Mutex<VecDeque<LlmResponse>>,
    }

    #[async_trait]
    impl LlmClient for ScriptedLlmClient {
        fn model_generation_concurrency(&self) -> a3s_code_core::llm::ModelGenerationConcurrency {
            a3s_code_core::llm::ModelGenerationConcurrency::bounded(
                std::num::NonZeroUsize::new(1).expect("scripted test concurrency is non-zero"),
            )
        }

        async fn complete(
            &self,
            messages: &[Message],
            system: Option<&str>,
            tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            Ok(self.response_for_messages(messages, system, tools))
        }

        async fn complete_streaming(
            &self,
            messages: &[Message],
            system: Option<&str>,
            tools: &[ToolDefinition],
            _cancel_token: CancellationToken,
        ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
            let response = self.response_for_messages(messages, system, tools);
            let (tx, rx) = mpsc::channel(1);
            tokio::spawn(async move {
                let _ = tx.send(StreamEvent::Done(response)).await;
            });
            Ok(rx)
        }

        fn native_structured_support(
            &self,
        ) -> a3s_code_core::llm::structured::NativeStructuredSupport {
            a3s_code_core::llm::structured::NativeStructuredSupport::ForcedTool
        }
    }

    impl ScriptedLlmClient {
        fn new(responses: Vec<LlmResponse>) -> Self {
            Self {
                responses: Mutex::new(responses.into()),
            }
        }

        fn response_for_messages(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
        ) -> LlmResponse {
            self.next_response()
        }

        fn next_response(&self) -> LlmResponse {
            self.responses
                .lock()
                .unwrap()
                .pop_front()
                .unwrap_or_else(|| text_response("DONE"))
        }
    }

    fn text_response(text: impl Into<String>) -> LlmResponse {
        LlmResponse {
            message: Message {
                role: "assistant".into(),
                content: vec![ContentBlock::Text { text: text.into() }],
                reasoning_content: None,
            },
            usage: TokenUsage::default(),
            stop_reason: Some("stop".into()),
            token_logprobs: Vec::new(),
            meta: None,
        }
    }
}
