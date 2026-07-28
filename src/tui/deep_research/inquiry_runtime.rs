//! Host-side orchestration for replayable, evidence-first research.
//!
//! Raw acquisition starts immediately and is durable before semantic work can
//! fail. One optional outline and one batched extraction are the only model
//! generations on the inquiry path; target decoding and terminal reduction are
//! Host-owned.

use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use a3s::research::{InquiryEvent, InquiryState};
use a3s_code_core::{AgentEvent, AgentSession, ToolCallResult};
use a3s_deep_research::engine::{
    DeepResearchEngine, EngineLimits, GenerationRequest, GenerationStage, ProgressPort,
    PublicationPort, PublicationRequest, ResearchProgress, StructuredGenerationPort,
    WorkflowExecutionPort, WorkflowOutput, WorkflowRequest, WorkflowStage,
};
use serde_json::Value;
use tokio::sync::mpsc;
use tokio::task::JoinHandle;

use self::execution::{
    run_bootstrap_acquisition_stage, run_dynamic_workflow, within_inquiry_stage_timeout,
};
use super::deep_research_artifacts::{
    materialize_deep_research_admitted_report,
    materialize_deep_research_no_evidence_report_in_language,
    materialize_deep_research_source_backed_report_in_language,
    record_deep_research_publication_receipt_in_language, DeepResearchEvidenceFirstPublication,
    ResearchReportArtifacts,
};
use super::deep_research_state_journal::{
    load_research_run_started_at_ms, record_workflow_started, ResearchSpec,
};
use super::{
    deep_research_canonical_workflow_output, deep_research_evidence_scope_from_args,
    validated_inquiry_projection, ValidatedInquiryProjection,
};

const PROGRESS_CHANNEL_CAPACITY: usize = 256;
// Discovery, semantic source admission, and the actual fetch share this stage.
// Keep the stage aligned with the contract hard cap. Source admission gets a
// 60-second active window; a real failure then falls back only for acquisition,
// leaving enough time for bounded fetches and HTML ranges before publication
// applies its unchanged evidence gates.
const BOOTSTRAP_ACQUISITION_STAGE_TIMEOUT_MS: u64 = 150_000;
// After the exact query starts immediately, the semantic plan may contribute
// bounded supplemental queries and one typed-coverage supplemental pass.
// This stage reuses the durable bootstrap packet instead of replacing it.
const PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS: u64 = 600_000;
const DURABLE_GENERATION_WORKFLOW_GRACE_MS: u64 = 15_000;
const REPORT_PROPOSAL_ATTEMPT_TIMEOUT_MS: u64 = 240_000;
const REPORT_PROPOSAL_MAX_ATTEMPTS: u8 = 2;
const REPORT_PROPOSAL_STAGE_TIMEOUT_MS: u64 = REPORT_PROPOSAL_ATTEMPT_TIMEOUT_MS
    * REPORT_PROPOSAL_MAX_ATTEMPTS as u64
    + DURABLE_GENERATION_WORKFLOW_GRACE_MS;
const REPORT_GENERATION_STAGE_COUNT: u64 = 2;
const EVIDENCE_FIRST_FINALIZATION_RESERVE_MS: u64 = 15_000;
pub(crate) const DEEP_RESEARCH_EVIDENCE_FIRST_HOST_TIMEOUT_MS: u64 =
    BOOTSTRAP_ACQUISITION_STAGE_TIMEOUT_MS
        + PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS
        + REPORT_PROPOSAL_STAGE_TIMEOUT_MS * REPORT_GENERATION_STAGE_COUNT
        + EVIDENCE_FIRST_FINALIZATION_RESERVE_MS;
const MIN_INQUIRY_STAGE_TIMEOUT_MS: u64 = 1_000;
const JOURNAL_INITIALIZATION_ATTEMPTS: usize = 8;
const JOURNAL_INITIALIZATION_RETRY_MS: u64 = 10;
const DURABLE_GENERATION_WORKFLOW_SOURCE: &str =
    a3s_deep_research::workflow::GENERATION_WORKFLOW_SOURCE;

#[derive(Clone, Copy, Debug)]
struct EvidenceFirstRuntimeLimits {
    bootstrap_stage_timeout_ms: u64,
    planned_retrieval_stage_timeout_ms: u64,
    report_proposal_attempt_timeout_ms: u64,
    report_proposal_stage_timeout_ms: u64,
}

const EVIDENCE_FIRST_RUNTIME_LIMITS: EvidenceFirstRuntimeLimits = EvidenceFirstRuntimeLimits {
    bootstrap_stage_timeout_ms: BOOTSTRAP_ACQUISITION_STAGE_TIMEOUT_MS,
    planned_retrieval_stage_timeout_ms: PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS,
    report_proposal_attempt_timeout_ms: REPORT_PROPOSAL_ATTEMPT_TIMEOUT_MS,
    report_proposal_stage_timeout_ms: REPORT_PROPOSAL_STAGE_TIMEOUT_MS,
};

struct A3sDeepResearchRuntime<'a> {
    session: &'a AgentSession,
    progress_tx: &'a mpsc::Sender<AgentEvent>,
    run_clock: &'a EvidenceFirstRunClock,
}

#[async_trait::async_trait]
impl StructuredGenerationPort for A3sDeepResearchRuntime<'_> {
    async fn generate_object(&self, request: GenerationRequest) -> Result<Value, String> {
        let execution_timeout_ms = if request.stage == GenerationStage::Planning {
            self.run_clock
                .pre_report_stage_timeout_ms(request.execution_timeout_ms)
                .ok_or_else(|| {
                    "the shared DeepResearch deadline left no outline-planner budget after reserving report proposal and finalization"
                        .to_string()
                })?
        } else {
            request.execution_timeout_ms
        };
        let result = execution::call_generation_with_progress(
            self.session,
            request.arguments,
            self.progress_tx,
            self.run_clock,
            request.stage.label(),
            execution_timeout_ms,
            request.max_attempts,
        )
        .await?;
        execution::generated_object::<Value>(&result)
    }
}

#[async_trait::async_trait]
impl WorkflowExecutionPort for A3sDeepResearchRuntime<'_> {
    async fn execute_workflow(&self, request: WorkflowRequest) -> Result<WorkflowOutput, String> {
        let arguments = crate::research::adapt_dynamic_workflow_arguments(request.arguments);
        let result = match request.stage {
            WorkflowStage::Bootstrap => {
                run_bootstrap_acquisition_stage(
                    self.session,
                    arguments,
                    self.progress_tx,
                    request.timeout_ms,
                )
                .await
            }
            WorkflowStage::PlannedRetrieval => {
                within_inquiry_stage_timeout(
                    run_dynamic_workflow(self.session, arguments, self.progress_tx),
                    request.timeout_ms,
                    request.stage.label(),
                )
                .await
            }
        }?;
        Ok(WorkflowOutput {
            output: result.output,
            metadata: result.metadata,
        })
    }
}

#[async_trait::async_trait]
impl PublicationPort for A3sDeepResearchRuntime<'_> {
    async fn publish(
        &self,
        request: PublicationRequest,
    ) -> Result<ResearchReportArtifacts, String> {
        match request {
            PublicationRequest::SourceBacked {
                run_id,
                query,
                output_language,
                workflow_output,
                workflow_metadata,
                quality,
            } => {
                self.validate_publication_run_id(&run_id)?;
                let artifacts = materialize_deep_research_source_backed_report_in_language(
                    self.session.workspace(),
                    &query,
                    &workflow_output,
                    workflow_metadata.as_ref(),
                    &output_language,
                )?
                .ok_or_else(|| {
                    "source catalog disappeared before deterministic publication".to_string()
                })?;
                record_deep_research_publication_receipt_in_language(
                    self.session.workspace(),
                    &query,
                    &output_language,
                    &run_id,
                    DeepResearchEvidenceFirstPublication::SourceBacked,
                    quality,
                    &artifacts,
                )?;
                Ok(artifacts)
            }
            PublicationRequest::Synthesized {
                run_id,
                query,
                output_language,
                report,
                publication,
                quality,
            } => {
                self.validate_publication_run_id(&run_id)?;
                if !matches!(
                    publication,
                    DeepResearchEvidenceFirstPublication::Synthesized
                        | DeepResearchEvidenceFirstPublication::Qualified
                ) {
                    return Err(
                        "generated report publication requested a non-generated outcome"
                            .to_string(),
                    );
                }
                let artifacts = materialize_deep_research_admitted_report(
                    self.session.workspace(),
                    &query,
                    &report,
                )?;
                record_deep_research_publication_receipt_in_language(
                    self.session.workspace(),
                    &query,
                    &output_language,
                    &run_id,
                    publication,
                    quality,
                    &artifacts,
                )?;
                Ok(artifacts)
            }
            PublicationRequest::NoEvidence {
                run_id,
                query,
                output_language,
                quality,
            } => {
                self.validate_publication_run_id(&run_id)?;
                let artifacts = materialize_deep_research_no_evidence_report_in_language(
                    self.session.workspace(),
                    &query,
                    &output_language,
                )?;
                record_deep_research_publication_receipt_in_language(
                    self.session.workspace(),
                    &query,
                    &output_language,
                    &run_id,
                    DeepResearchEvidenceFirstPublication::NoEvidence,
                    quality,
                    &artifacts,
                )?;
                Ok(artifacts)
            }
        }
    }
}

impl A3sDeepResearchRuntime<'_> {
    fn validate_publication_run_id(&self, run_id: &str) -> Result<(), String> {
        if run_id == self.run_clock.run_id() {
            Ok(())
        } else {
            Err("publication request belongs to a different DeepResearch run".to_string())
        }
    }
}

#[async_trait::async_trait]
impl ProgressPort for A3sDeepResearchRuntime<'_> {
    async fn report_progress(&self, _progress: ResearchProgress) -> Result<(), String> {
        // A3S forwards the finer-grained tool event streams from each port.
        Ok(())
    }
}

#[path = "inquiry_runtime/execution.rs"]
mod execution;

/// Spawn the standalone engine for every new evidence-first run. The engine
/// preserves exact-query acquisition, closed semantic evidence selection, and
/// a source-backed artifact before attempting the optional report proposal.
/// The legacy Inquiry path below remains only for journal compatibility.
pub(crate) fn spawn_deep_research_evidence_first(
    session: Arc<AgentSession>,
    args: Value,
) -> (
    mpsc::Receiver<AgentEvent>,
    JoinHandle<Result<ToolCallResult, String>>,
) {
    let (progress_tx, progress_rx) = mpsc::channel(PROGRESS_CHANNEL_CAPACITY);
    let join =
        tokio::spawn(async move { run_evidence_first_research(session, args, progress_tx).await });
    (progress_rx, join)
}

pub(crate) fn deep_research_evidence_first_research_spec(args: &Value) -> ResearchSpec {
    let query = args
        .pointer("/input/query")
        .and_then(Value::as_str)
        .unwrap_or_default();
    ResearchSpec {
        query: query.to_string(),
        current_date: args
            .pointer("/input/current_date")
            .and_then(Value::as_str)
            .map(str::to_string)
            .unwrap_or_else(|| chrono::Local::now().date_naive().to_string()),
        evidence_scope: deep_research_evidence_scope_from_args(args)
            .label()
            .to_string(),
        required_claims: Vec::new(),
        total_budget_ms: DEEP_RESEARCH_EVIDENCE_FIRST_HOST_TIMEOUT_MS,
        retrieval_stage_budget_ms: BOOTSTRAP_ACQUISITION_STAGE_TIMEOUT_MS
            + PLANNED_RETRIEVAL_STAGE_TIMEOUT_MS,
        question_review_stage_budget_ms: REPORT_PROPOSAL_STAGE_TIMEOUT_MS
            * REPORT_GENERATION_STAGE_COUNT,
        finalization_reserve_ms: EVIDENCE_FIRST_FINALIZATION_RESERVE_MS,
        host_pid: std::process::id(),
    }
}

async fn run_evidence_first_research(
    session: Arc<AgentSession>,
    args: Value,
    progress_tx: mpsc::Sender<AgentEvent>,
) -> Result<ToolCallResult, String> {
    run_evidence_first_research_with_limits(
        session,
        args,
        progress_tx,
        EVIDENCE_FIRST_RUNTIME_LIMITS,
    )
    .await
}

async fn run_evidence_first_research_with_limits(
    session: Arc<AgentSession>,
    args: Value,
    progress_tx: mpsc::Sender<AgentEvent>,
    limits: EvidenceFirstRuntimeLimits,
) -> Result<ToolCallResult, String> {
    let host_id = format!(
        "host-deep-research-{}",
        args.get("run_id")
            .and_then(Value::as_str)
            .unwrap_or("unassigned")
    );
    send_progress(
        &progress_tx,
        AgentEvent::ToolExecutionStart {
            id: host_id,
            name: "dynamic_workflow".to_string(),
            args: args.clone(),
        },
    )
    .await?;
    let run_clock = EvidenceFirstRunClock::initialize(&session, &args).await?;
    execute_evidence_first_research(&session, &args, &progress_tx, &run_clock, limits).await
}

async fn execute_evidence_first_research(
    session: &AgentSession,
    args: &Value,
    progress_tx: &mpsc::Sender<AgentEvent>,
    run_clock: &EvidenceFirstRunClock,
    limits: EvidenceFirstRuntimeLimits,
) -> Result<ToolCallResult, String> {
    let runtime = A3sDeepResearchRuntime {
        session,
        progress_tx,
        run_clock,
    };
    let engine_limits = EngineLimits {
        bootstrap_stage_timeout_ms: limits.bootstrap_stage_timeout_ms,
        planned_retrieval_stage_timeout_ms: limits.planned_retrieval_stage_timeout_ms,
        report_attempt_timeout_ms: limits.report_proposal_attempt_timeout_ms,
        report_stage_timeout_ms: limits.report_proposal_stage_timeout_ms,
        ..EngineLimits::default()
    };
    let run = DeepResearchEngine::new(&runtime, &runtime, &runtime, &runtime)
        .with_limits(engine_limits)
        .execute(args.clone())
        .await
        .map_err(|error| error.to_string())?;
    Ok(ToolCallResult {
        name: "dynamic_workflow".to_string(),
        output: run.output_json(),
        exit_code: 0,
        metadata: None,
        error_kind: None,
    })
}

pub(super) fn inquiry_projection_from_workflow(
    workflow_output: &str,
    workflow_metadata: Option<&Value>,
) -> Result<Option<(Vec<InquiryEvent>, InquiryState)>, String> {
    let canonical = deep_research_canonical_workflow_output(workflow_output, workflow_metadata);
    let value = serde_json::from_str::<Value>(&canonical)
        .map_err(|error| format!("decode DeepResearch inquiry projection: {error}"))?;
    match validated_inquiry_projection(&value)? {
        ValidatedInquiryProjection::LegacyCheckedLoop => Ok(None),
        ValidatedInquiryProjection::Inquiry { events, state } => Ok(Some((events, *state))),
    }
}

#[derive(Clone, Debug)]
struct EvidenceFirstDeadline {
    deadline: Instant,
    report_reserve: Duration,
    finalization_reserve: Duration,
}

impl EvidenceFirstDeadline {
    fn from_started_at_ms(
        started_at_ms: u64,
        total_budget_ms: u64,
        report_reserve_ms: u64,
        finalization_reserve_ms: u64,
        now: Instant,
    ) -> Self {
        let elapsed_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .ok()
            .map(|duration| duration.as_millis().min(u128::from(u64::MAX)) as u64)
            .and_then(|wall_now_ms| wall_now_ms.checked_sub(started_at_ms))
            .unwrap_or(u64::MAX);
        let remaining_ms = total_budget_ms.saturating_sub(elapsed_ms);
        Self {
            deadline: now
                .checked_add(Duration::from_millis(remaining_ms))
                .unwrap_or(now),
            report_reserve: Duration::from_millis(report_reserve_ms.min(total_budget_ms)),
            finalization_reserve: Duration::from_millis(
                finalization_reserve_ms.min(total_budget_ms),
            ),
        }
    }

    fn pre_report_stage_timeout_ms(&self, now: Instant, requested_timeout_ms: u64) -> Option<u64> {
        let available = self
            .deadline
            .saturating_duration_since(now)
            .saturating_sub(self.report_reserve)
            .saturating_sub(self.finalization_reserve);
        let available_ms = available.as_millis().min(u128::from(u64::MAX)) as u64;
        let selected = requested_timeout_ms.min(available_ms);
        (selected >= MIN_INQUIRY_STAGE_TIMEOUT_MS).then_some(selected)
    }
}

/// Carries the durable run identity and one absolute budget origin shared by
/// every standalone-engine stage. It does not interpret query text or source
/// content.
#[derive(Debug)]
struct EvidenceFirstRunClock {
    run_id: String,
    deadline: EvidenceFirstDeadline,
}

impl EvidenceFirstRunClock {
    async fn initialize(session: &AgentSession, args: &Value) -> Result<Self, String> {
        let run_id = args
            .get("run_id")
            .and_then(Value::as_str)
            .filter(|run_id| !run_id.trim().is_empty())
            .ok_or_else(|| "DeepResearch runtime requires a run_id".to_string())?
            .to_string();
        let workspace = session.workspace();
        let spec = deep_research_evidence_first_research_spec(args);
        let total_budget_ms = spec.total_budget_ms;
        let report_reserve_ms = spec.question_review_stage_budget_ms;
        let finalization_reserve_ms = spec.finalization_reserve_ms;
        let mut last_error = None;
        for attempt in 0..JOURNAL_INITIALIZATION_ATTEMPTS {
            match record_workflow_started(workspace, &run_id, spec.clone()).await {
                Ok(()) => {
                    let started_at_ms = load_research_run_started_at_ms(workspace, &run_id)
                        .await
                        .map_err(|error| {
                            format!(
                                "load durable DeepResearch deadline origin for `{run_id}`: {error}"
                            )
                        })?
                        .ok_or_else(|| {
                            format!(
                                "DeepResearch run `{run_id}` persisted without a durable deadline origin"
                            )
                        })?;
                    return Ok(Self {
                        run_id,
                        deadline: EvidenceFirstDeadline::from_started_at_ms(
                            started_at_ms,
                            total_budget_ms,
                            report_reserve_ms,
                            finalization_reserve_ms,
                            Instant::now(),
                        ),
                    });
                }
                Err(error) => {
                    last_error = Some(error);
                    if attempt + 1 < JOURNAL_INITIALIZATION_ATTEMPTS {
                        tokio::time::sleep(Duration::from_millis(JOURNAL_INITIALIZATION_RETRY_MS))
                            .await;
                    }
                }
            }
        }
        let detail = last_error
            .map(|error| error.to_string())
            .unwrap_or_else(|| "initialization attempts ended without an error".to_string());
        Err(format!("initialize DeepResearch run journal: {detail}"))
    }

    fn pre_report_stage_timeout_ms(&self, requested_timeout_ms: u64) -> Option<u64> {
        self.deadline
            .pre_report_stage_timeout_ms(Instant::now(), requested_timeout_ms)
    }

    fn run_id(&self) -> &str {
        &self.run_id
    }
}

async fn send_progress(
    progress_tx: &mpsc::Sender<AgentEvent>,
    event: AgentEvent,
) -> Result<(), String> {
    progress_tx
        .send(event)
        .await
        .map_err(|_| "DeepResearch progress consumer closed".to_string())
}

#[cfg(test)]
#[path = "inquiry_runtime/evidence_first_tests.rs"]
mod evidence_first_tests;
#[cfg(test)]
#[path = "inquiry_runtime/product_adapter_tests.rs"]
mod product_adapter_tests;
