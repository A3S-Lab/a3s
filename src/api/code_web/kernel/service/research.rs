use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use tokio_util::sync::CancellationToken;

use super::streaming::send_code_web_event;
use super::*;
use crate::budget::{budget_plan_for_effort_id, BudgetWorkload};
use crate::commands::code::research_runtime::{
    DeepResearchReportStatus, DeepResearchReportSynthesis,
};
use crate::research::{
    build_code_deep_research_request, CodeDeepResearchEvent, CodeDeepResearchLaunch,
    CodeDeepResearchRunExit, CodeDeepResearchRunner, CodeDeepResearchRunnerBudget,
};
use a3s_deep_research::engine::{
    DeepResearchEvent, DeepResearchLifecycle, DeepResearchRequest, EvidenceScope,
    PublicationOutcome, ResearchStage, WorkspaceSourceHint,
};
use a3s_deep_research::report::clean_deep_research_final_text_from_artifacts;

const MAX_RESEARCH_REPORT_BYTES: u64 = 4 * 1024 * 1024;
const RESEARCH_CANCELLED_MESSAGE: &str = "DeepResearch was cancelled by the user.";

pub(in crate::api::code_web) struct DeepResearchArtifactResponse {
    pub(in crate::api::code_web) body: Vec<u8>,
    pub(in crate::api::code_web) content_type: &'static str,
}

impl KernelService {
    pub(super) async fn stream_deep_research_turn(
        &self,
        session: Arc<AgentSession>,
        turn: CodeWebQueuedTurn,
        visible_content: String,
    ) -> BootResult<BootResponse> {
        if turn.kind != CodeWebQueuedTurnKind::User {
            self.restore_queued_turn(session.session_id(), &turn.id)
                .await?;
            return Err(BootError::BadRequest(
                "DeepResearch mode is only available for user turns".to_string(),
            ));
        }
        if !turn.skill_names.is_empty() {
            self.restore_queued_turn(session.session_id(), &turn.id)
                .await?;
            return Err(BootError::BadRequest(
                "DeepResearch does not support selected skills; remove skillNames and retry"
                    .to_string(),
            ));
        }

        let session_id = session.session_id().to_string();
        let workspace = session.workspace().to_path_buf();
        let query = turn.content.clone();
        let run_id = turn
            .research_run_id
            .clone()
            .unwrap_or_else(|| code_web_research_run_id(&turn.id));
        let mut code_config = self.state.code_config_snapshot();
        if let Some(model) = self.session_response_model(&session_id).await {
            code_config.default_model = Some(model);
        }
        let memory_dir = research_memory_dir(&workspace, code_config.memory_dir.as_deref());
        let controls = self.session_controls_snapshot(&session_id).await;
        let budget =
            budget_plan_for_effort_id(&controls.effort, None, BudgetWorkload::DeepResearch);
        let request = build_code_deep_research_request(
            Some(run_id.clone()),
            &query,
            EvidenceScope::WebAndWorkspace,
            CodeDeepResearchRunnerBudget {
                local_max_steps: budget.deep_research_child_steps,
                max_tool_calls: budget.workflow_max_tool_calls,
                max_output_bytes: budget.workflow_max_output_bytes,
            },
            turn.context_files
                .iter()
                .cloned()
                .map(WorkspaceSourceHint::new)
                .collect(),
        )
        .map_err(BootError::BadRequest)?;
        let runner = CodeDeepResearchRunner::new(&workspace, code_config, memory_dir);
        let mut handle = match runner
            .start(
                CodeDeepResearchLaunch {
                    request,
                    skill_names: turn.skill_names.clone(),
                },
                crate::session_llm::resolve_session_llm_client,
            )
            .await
        {
            Ok(handle) => handle,
            Err(error) => {
                self.restore_queued_turn(&session_id, &turn.id).await?;
                return Err(deep_research_start_error(error));
            }
        };
        let mut research_events = handle.take_events().ok_or_else(|| {
            BootError::Internal("DeepResearch event stream was already consumed".to_string())
        })?;
        let mut completion_signal = handle.completion_signal();
        let cancellation = Arc::new(CancellationToken::new());
        self.state
            .active_research_runs
            .lock()
            .await
            .insert(session_id.clone(), Arc::clone(&cancellation));
        if let Err(error) = self
            .append_message(&session_id, "user", &visible_content, None)
            .await
        {
            self.state
                .active_research_runs
                .lock()
                .await
                .remove(&session_id);
            let _ = handle.cancel_and_settle().await;
            let _ = self.finish_queued_turn(&session_id, &turn.id, true).await;
            return Err(error);
        }

        let service = Self::new(Arc::clone(&self.state));
        let turn_id = turn.id.clone();
        let (sender, receiver) = tokio::sync::mpsc::channel::<BootResult<SseEvent>>(64);
        tokio::spawn(async move {
            let started_at = Instant::now();
            let tool_id = format!("deep-research-{turn_id}");
            let tool_args = json!({
                "query": query.clone(),
                "scope": "webAndWorkspace",
                "runId": run_id,
                "contextFiles": turn.context_files,
            });
            let mut events = Vec::new();
            emit_research_event(
                &sender,
                &mut events,
                AgentEvent::Start {
                    prompt: format!("DeepResearch: {query}"),
                },
            )
            .await;
            emit_research_event(
                &sender,
                &mut events,
                AgentEvent::AgentModeChanged {
                    mode: "deep_research".to_string(),
                    agent: "deep-research".to_string(),
                    description: "Collecting and validating web and workspace evidence before publishing a report."
                        .to_string(),
                },
            )
            .await;
            emit_research_event(
                &sender,
                &mut events,
                AgentEvent::ToolStart {
                    id: tool_id.clone(),
                    name: "deep_research".to_string(),
                },
            )
            .await;
            emit_research_event(
                &sender,
                &mut events,
                AgentEvent::ToolExecutionStart {
                    id: tool_id.clone(),
                    name: "deep_research".to_string(),
                    args: tool_args.clone(),
                },
            )
            .await;

            let completion_wait = async {
                if !*completion_signal.borrow() {
                    let _ = completion_signal.changed().await;
                }
            };
            tokio::pin!(completion_wait);
            let mut handle = Some(handle);
            let mut events_open = true;
            let completion = loop {
                tokio::select! {
                    biased;
                    _ = cancellation.cancelled() => {
                        let Some(handle) = handle.take() else {
                            break ResearchCompletion::Cancelled;
                        };
                        let _ = handle.cancel_and_settle().await;
                        break ResearchCompletion::Cancelled;
                    }
                    _ = &mut completion_wait => {
                        while let Ok(event) = research_events.try_recv() {
                            forward_deep_research_event(
                                &sender,
                                &mut events,
                                &tool_id,
                                event,
                            )
                            .await;
                        }
                        let Some(handle) = handle.take() else {
                            break ResearchCompletion::Failed(
                                "DeepResearch root handle was already settled".to_string(),
                            );
                        };
                        let exit = handle.settle().await;
                        break research_completion_from_exit(exit, &workspace);
                    }
                    event = research_events.recv(), if events_open => {
                        match event {
                            Some(event) => {
                                forward_deep_research_event(
                                    &sender,
                                    &mut events,
                                    &tool_id,
                                    event,
                                )
                                .await;
                            }
                            None => events_open = false,
                        }
                    }
                }
            };

            if let Some(handle) = handle {
                let _ = handle.cancel_and_settle().await;
            }

            let succeeded = match completion {
                ResearchCompletion::Published(synthesis) => {
                    publish_research_completion(
                        &service,
                        &sender,
                        &mut events,
                        &session_id,
                        &workspace,
                        &tool_id,
                        tool_args,
                        started_at,
                        *synthesis,
                    )
                    .await
                }
                ResearchCompletion::Cancelled => {
                    publish_research_cancellation(
                        &service,
                        &sender,
                        &mut events,
                        &session_id,
                        &tool_id,
                        tool_args,
                        started_at,
                    )
                    .await;
                    false
                }
                ResearchCompletion::Failed(error) => {
                    let message = format!("DeepResearch failed: {error}");
                    publish_research_failure(
                        &service,
                        &sender,
                        &mut events,
                        &session_id,
                        &tool_id,
                        tool_args,
                        started_at,
                        &message,
                    )
                    .await;
                    false
                }
            };

            let mut active = service.state.active_research_runs.lock().await;
            if active
                .get(&session_id)
                .is_some_and(|current| Arc::ptr_eq(current, &cancellation))
            {
                active.remove(&session_id);
            }
            drop(active);
            let _ = service
                .finish_queued_turn(&session_id, &turn_id, !succeeded)
                .await;
        });

        let stream = futures::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|event| (event, receiver))
        });
        Ok(BootResponse::sse(stream))
    }

    pub(in crate::api::code_web) async fn read_deep_research_artifact(
        &self,
        session_id: &str,
        run_id: &str,
        kind: &str,
    ) -> BootResult<DeepResearchArtifactResponse> {
        let session = self.kernel_session(session_id).await?;
        let (report_path, content_type) =
            validated_run_artifact_path(session.workspace(), run_id, kind).await?;
        let metadata = tokio::fs::metadata(&report_path)
            .await
            .map_err(report_io_error)?;
        if metadata.len() > MAX_RESEARCH_REPORT_BYTES {
            return Err(BootError::BadRequest(format!(
                "DeepResearch report exceeds the {MAX_RESEARCH_REPORT_BYTES}-byte display limit"
            )));
        }
        let body = tokio::fs::read(report_path)
            .await
            .map_err(report_io_error)?;
        Ok(DeepResearchArtifactResponse { body, content_type })
    }
}

enum ResearchCompletion {
    Published(Box<DeepResearchReportSynthesis>),
    Cancelled,
    Failed(String),
}

fn research_completion_from_exit(
    exit: Result<CodeDeepResearchRunExit, String>,
    workspace: &Path,
) -> ResearchCompletion {
    let result = match exit {
        Ok(CodeDeepResearchRunExit::Completed(result)) => *result,
        Ok(CodeDeepResearchRunExit::Cancelled) => return ResearchCompletion::Cancelled,
        Err(error) => return ResearchCompletion::Failed(error),
    };
    if result.lifecycle != DeepResearchLifecycle::Completed {
        return ResearchCompletion::Failed(format!(
            "root task settled with lifecycle {:?}",
            result.lifecycle
        ));
    }
    let Some(text) = clean_deep_research_final_text_from_artifacts(&result.artifacts, workspace)
    else {
        return ResearchCompletion::Failed(format!(
            "published artifacts for run {} could not be read",
            result.run_id
        ));
    };
    ResearchCompletion::Published(Box::new(DeepResearchReportSynthesis {
        run_id: result.run_id,
        text,
        artifacts: result.artifacts,
        status: result.publication,
        quality: result.quality,
    }))
}

async fn forward_deep_research_event(
    sender: &tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
    events: &mut Vec<AgentEvent>,
    tool_id: &str,
    event: CodeDeepResearchEvent,
) {
    match event {
        CodeDeepResearchEvent::Agent(AgentEvent::End { .. } | AgentEvent::Error { .. }) => {}
        CodeDeepResearchEvent::Agent(event) => emit_research_event(sender, events, event).await,
        CodeDeepResearchEvent::Engine(event) => {
            let wire = SseEvent::json(&deep_research_event_wire(&event))
                .map(|event| event.with_event("deep_research"));
            let _ = sender.send(wire).await;
            if let Some(delta) = deep_research_event_delta(&event) {
                emit_research_event(
                    sender,
                    events,
                    AgentEvent::ToolOutputDelta {
                        id: tool_id.to_string(),
                        name: "deep_research".to_string(),
                        delta,
                    },
                )
                .await;
            }
        }
    }
}

fn deep_research_event_wire(event: &DeepResearchEvent) -> Value {
    match event {
        DeepResearchEvent::RunStarted { run_id, query } => json!({
            "type": "run_started",
            "run_id": run_id,
            "query": query,
        }),
        DeepResearchEvent::StageStarted { run_id, stage } => json!({
            "type": "stage_started",
            "run_id": run_id,
            "stage": stage,
        }),
        DeepResearchEvent::StageCompleted { run_id, stage } => json!({
            "type": "stage_completed",
            "run_id": run_id,
            "stage": stage,
        }),
        DeepResearchEvent::StageDegraded {
            run_id,
            stage,
            reason,
        } => json!({
            "type": "stage_degraded",
            "run_id": run_id,
            "stage": stage,
            "reason": reason,
        }),
        DeepResearchEvent::PublicationCompleted {
            run_id,
            outcome,
            quality,
            ..
        } => json!({
            "type": "publication_completed",
            "run_id": run_id,
            "outcome": outcome,
            "quality": quality,
            "artifact_kinds": ["markdown", "html"],
        }),
        DeepResearchEvent::RunCompleted { run_id, outcome } => json!({
            "type": "run_completed",
            "run_id": run_id,
            "outcome": outcome,
        }),
        DeepResearchEvent::RunCancelled { run_id } => json!({
            "type": "run_cancelled",
            "run_id": run_id,
        }),
        DeepResearchEvent::RunFailed { run_id, message } => json!({
            "type": "run_failed",
            "run_id": run_id,
            "message": message,
        }),
    }
}

fn deep_research_event_delta(event: &DeepResearchEvent) -> Option<String> {
    match event {
        DeepResearchEvent::RunStarted { .. } => Some("DeepResearch run started.\n".to_string()),
        DeepResearchEvent::StageStarted { stage, .. } => {
            Some(format!("DeepResearch: {}.\n", research_stage_id(*stage)))
        }
        DeepResearchEvent::StageCompleted { stage, .. } => Some(format!(
            "DeepResearch completed {}.\n",
            research_stage_id(*stage)
        )),
        DeepResearchEvent::StageDegraded { stage, reason, .. } => Some(format!(
            "DeepResearch degraded {}: {reason}\n",
            research_stage_id(*stage)
        )),
        DeepResearchEvent::PublicationCompleted {
            outcome, quality, ..
        } => Some(format!(
            "DeepResearch published {} with {}/{} relevant sources.\n",
            publication_outcome_id(*outcome),
            quality.relevant_source_count,
            quality.source_count
        )),
        DeepResearchEvent::RunCompleted { outcome, .. } => Some(format!(
            "DeepResearch completed with {} publication.\n",
            publication_outcome_id(*outcome)
        )),
        DeepResearchEvent::RunCancelled { .. } => {
            Some("DeepResearch cancellation settled.\n".to_string())
        }
        DeepResearchEvent::RunFailed { message, .. } => {
            Some(format!("DeepResearch failed: {message}\n"))
        }
    }
}

fn research_stage_id(stage: ResearchStage) -> &'static str {
    match stage {
        ResearchStage::Planning => "planning",
        ResearchStage::BootstrapRetrieval => "bootstrap retrieval",
        ResearchStage::PlannedRetrieval => "planned retrieval",
        ResearchStage::SourcePublication => "source publication",
        ResearchStage::ReportGeneration => "report generation",
        ResearchStage::FinalPublication => "final publication",
    }
}

fn publication_outcome_id(outcome: PublicationOutcome) -> &'static str {
    match outcome {
        PublicationOutcome::Synthesized => "synthesized",
        PublicationOutcome::Qualified => "qualified",
        PublicationOutcome::SourceBacked => "source-backed",
        PublicationOutcome::NoEvidence => "no-evidence",
    }
}

pub(super) fn code_web_research_run_id(turn_id: &str) -> String {
    let normalized = turn_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '-'
            }
        })
        .take(120)
        .collect::<String>();
    format!("web-{normalized}")
}

pub(super) fn validate_code_web_research_turn(
    query: &str,
    context_files: &[String],
    skill_names: &[String],
) -> Result<(), String> {
    if !skill_names.is_empty() {
        return Err(
            "DeepResearch does not support selected skills; remove skillNames and retry"
                .to_string(),
        );
    }
    DeepResearchRequest::new(
        "web-queue-contract-validation",
        query,
        EvidenceScope::WebAndWorkspace,
    )
    .with_workspace_source_hints(
        context_files
            .iter()
            .cloned()
            .map(WorkspaceSourceHint::new)
            .collect(),
    )
    .validate()
}

fn deep_research_start_error(error: String) -> BootError {
    if error.contains("workspace source hint")
        || error.contains("workspace source hints")
        || error.contains("DeepResearch request")
    {
        BootError::BadRequest(error)
    } else {
        BootError::Internal(error)
    }
}

#[allow(clippy::too_many_arguments)]
async fn publish_research_completion(
    service: &KernelService,
    sender: &tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
    events: &mut Vec<AgentEvent>,
    session_id: &str,
    workspace: &Path,
    tool_id: &str,
    tool_args: Value,
    started_at: Instant,
    synthesis: DeepResearchReportSynthesis,
) -> bool {
    let artifacts = match research_artifact_metadata(workspace, &synthesis) {
        Ok(artifacts) => artifacts,
        Err(error) => {
            publish_research_failure(
                service, sender, events, session_id, tool_id, tool_args, started_at, &error,
            )
            .await;
            return false;
        }
    };
    let status = report_status_id(synthesis.status);
    emit_research_event(
        sender,
        events,
        AgentEvent::ToolEnd {
            id: tool_id.to_string(),
            name: "deep_research".to_string(),
            args: Some(tool_args),
            output: format!("DeepResearch published a {status} report."),
            exit_code: 0,
            metadata: Some(json!({
                "duration_ms": duration_millis(started_at),
                "report": artifacts,
            })),
            error_kind: None,
        },
    )
    .await;
    let end = AgentEvent::End {
        text: synthesis.text.clone(),
        usage: TokenUsage::default(),
        verification_summary: Box::new(
            a3s_code_core::verification::VerificationSummary::from_reports(&[]),
        ),
        meta: None,
    };
    events.push(end.clone());
    if let Err(error) = service
        .append_message_with_events(
            session_id,
            "assistant",
            &synthesis.text,
            service.session_response_model(session_id).await,
            events,
        )
        .await
    {
        events.pop();
        let message = format!(
            "DeepResearch report was written but its Web response could not be saved: {error}"
        );
        emit_research_event(sender, events, AgentEvent::Error { message }).await;
        return false;
    }
    send_code_web_event(sender, &end).await;
    true
}

#[allow(clippy::too_many_arguments)]
async fn publish_research_failure(
    service: &KernelService,
    sender: &tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
    events: &mut Vec<AgentEvent>,
    session_id: &str,
    tool_id: &str,
    tool_args: Value,
    started_at: Instant,
    message: &str,
) {
    emit_research_event(
        sender,
        events,
        AgentEvent::ToolEnd {
            id: tool_id.to_string(),
            name: "deep_research".to_string(),
            args: Some(tool_args),
            output: message.to_string(),
            exit_code: 1,
            metadata: Some(json!({ "duration_ms": duration_millis(started_at) })),
            error_kind: None,
        },
    )
    .await;
    let error = AgentEvent::Error {
        message: message.to_string(),
    };
    events.push(error.clone());
    let _ = service
        .append_message_with_events(
            session_id,
            "assistant",
            message,
            service.session_response_model(session_id).await,
            events,
        )
        .await;
    send_code_web_event(sender, &error).await;
}

#[allow(clippy::too_many_arguments)]
async fn publish_research_cancellation(
    service: &KernelService,
    sender: &tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
    events: &mut Vec<AgentEvent>,
    session_id: &str,
    tool_id: &str,
    tool_args: Value,
    started_at: Instant,
) {
    let (tool_end, terminal) =
        research_cancellation_events(tool_id, tool_args, duration_millis(started_at));
    emit_research_event(sender, events, tool_end).await;
    events.push(terminal.clone());
    if let Err(error) = service
        .append_message_with_events(
            session_id,
            "assistant",
            RESEARCH_CANCELLED_MESSAGE,
            service.session_response_model(session_id).await,
            events,
        )
        .await
    {
        events.pop();
        let message =
            format!("DeepResearch was cancelled but its Web response could not be saved: {error}");
        emit_research_event(sender, events, AgentEvent::Error { message }).await;
        return;
    }
    send_code_web_event(sender, &terminal).await;
}

fn research_cancellation_events(
    tool_id: &str,
    tool_args: Value,
    duration_ms: u64,
) -> (AgentEvent, AgentEvent) {
    (
        AgentEvent::ToolEnd {
            id: tool_id.to_string(),
            name: "deep_research".to_string(),
            args: Some(tool_args),
            output: RESEARCH_CANCELLED_MESSAGE.to_string(),
            exit_code: 1,
            metadata: Some(json!({
                "duration_ms": duration_ms,
                "cancelled": true,
                "message": RESEARCH_CANCELLED_MESSAGE,
            })),
            error_kind: Some(ToolErrorKind::Cancelled {
                op: "deep_research".to_string(),
            }),
        },
        AgentEvent::End {
            text: RESEARCH_CANCELLED_MESSAGE.to_string(),
            usage: TokenUsage::default(),
            verification_summary: Box::new(
                a3s_code_core::verification::VerificationSummary::from_reports(&[]),
            ),
            meta: None,
        },
    )
}

async fn emit_research_event(
    sender: &tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
    events: &mut Vec<AgentEvent>,
    event: AgentEvent,
) {
    send_code_web_event(sender, &event).await;
    events.push(event);
}

fn research_memory_dir(workspace: &Path, configured: Option<&Path>) -> PathBuf {
    match configured {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => workspace.join(path),
        None => workspace.join(".a3s/memory"),
    }
}

fn research_artifact_metadata(
    _workspace: &Path,
    synthesis: &DeepResearchReportSynthesis,
) -> Result<Value, String> {
    a3s_deep_research::report::validate_deep_research_run_id(&synthesis.run_id)?;
    Ok(json!({
        "runId": synthesis.run_id,
        "status": report_status_id(synthesis.status),
        "quality": synthesis.quality,
        "artifactKinds": ["markdown", "html"],
    }))
}

fn report_status_id(status: DeepResearchReportStatus) -> &'static str {
    match status {
        DeepResearchReportStatus::Synthesized => "synthesized",
        DeepResearchReportStatus::Qualified => "qualified",
        DeepResearchReportStatus::SourceBacked => "source_backed",
        DeepResearchReportStatus::NoEvidence => "no_evidence",
    }
}

fn duration_millis(started_at: Instant) -> u64 {
    u64::try_from(started_at.elapsed().as_millis()).unwrap_or(u64::MAX)
}

async fn validated_run_artifact_path(
    workspace: &Path,
    run_id: &str,
    kind: &str,
) -> BootResult<(PathBuf, &'static str)> {
    let (file_name, content_type) = match kind.trim() {
        "html" => ("index.html", "text/html; charset=utf-8"),
        "markdown" => ("report.md", "text/markdown; charset=utf-8"),
        _ => {
            return Err(BootError::BadRequest(
                "kind must be `html` or `markdown`".to_string(),
            ))
        }
    };
    let relative_dir =
        a3s_deep_research::report::deep_research_run_artifact_relative_directory(run_id)
            .map_err(BootError::BadRequest)?;
    let workspace = tokio::fs::canonicalize(workspace)
        .await
        .map_err(report_io_error)?;
    let artifact_dir = tokio::fs::canonicalize(workspace.join(relative_dir))
        .await
        .map_err(report_io_error)?;
    if !artifact_dir.starts_with(&workspace) {
        return Err(BootError::Forbidden(
            "DeepResearch artifact directory escapes the active workspace".to_string(),
        ));
    }
    let report_path = artifact_dir.join(file_name);
    let link_metadata = tokio::fs::symlink_metadata(&report_path)
        .await
        .map_err(report_io_error)?;
    if link_metadata.file_type().is_symlink() || !link_metadata.is_file() {
        return Err(BootError::Forbidden(
            "DeepResearch artifact must be a plain file".to_string(),
        ));
    }
    let report_path = tokio::fs::canonicalize(report_path)
        .await
        .map_err(report_io_error)?;
    if report_path.parent() != Some(artifact_dir.as_path()) {
        return Err(BootError::Forbidden(
            "DeepResearch artifact escaped its run directory".to_string(),
        ));
    }
    Ok((report_path, content_type))
}

fn report_io_error(error: std::io::Error) -> BootError {
    match error.kind() {
        std::io::ErrorKind::NotFound => BootError::NotFound(error.to_string()),
        std::io::ErrorKind::PermissionDenied => BootError::Forbidden(error.to_string()),
        _ => BootError::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_metadata_exposes_run_identity_without_filesystem_paths() {
        let workspace = Path::new("/tmp/workspace");
        let synthesis = DeepResearchReportSynthesis {
            run_id: "artifact-metadata-test".to_string(),
            text: "report".to_string(),
            artifacts: crate::commands::code::research_runtime::ResearchReportArtifacts {
                markdown: workspace
                    .join(".a3s/research/artifacts/artifact-metadata-test/report.md"),
                html: workspace.join(".a3s/research/artifacts/artifact-metadata-test/index.html"),
            },
            status: DeepResearchReportStatus::Synthesized,
            quality: a3s_deep_research::report::DeepResearchPublicationQuality::default(),
        };

        let metadata = research_artifact_metadata(workspace, &synthesis).expect("report metadata");

        assert_eq!(metadata["status"], "synthesized");
        assert_eq!(metadata["runId"], "artifact-metadata-test");
        assert_eq!(metadata["artifactKinds"], json!(["markdown", "html"]));
        assert!(metadata.get("markdownPath").is_none());
        assert!(metadata.get("htmlPath").is_none());
    }

    #[test]
    fn typed_sse_publication_event_omits_filesystem_paths() {
        let event = DeepResearchEvent::PublicationCompleted {
            run_id: "typed-sse-test".to_string(),
            outcome: PublicationOutcome::SourceBacked,
            quality: a3s_deep_research::report::DeepResearchPublicationQuality::default(),
            artifacts: crate::commands::code::research_runtime::ResearchReportArtifacts {
                markdown: PathBuf::from(
                    "/private/workspace/.a3s/research/artifacts/typed-sse-test/report.md",
                ),
                html: PathBuf::from(
                    "/private/workspace/.a3s/research/artifacts/typed-sse-test/index.html",
                ),
            },
        };

        let wire = deep_research_event_wire(&event);

        assert_eq!(wire["type"], "publication_completed");
        assert_eq!(wire["run_id"], "typed-sse-test");
        assert_eq!(wire["outcome"], "source_backed");
        assert_eq!(wire["artifact_kinds"], json!(["markdown", "html"]));
        assert!(wire.get("artifacts").is_none());
        assert!(!wire.to_string().contains("/private/workspace"));
    }

    #[test]
    fn cancelled_research_uses_a_typed_non_error_terminal_event() {
        let (tool_end, terminal) = research_cancellation_events(
            "deep-research-cancelled",
            json!({ "query": "cancel me" }),
            42,
        );
        let tool_end = serde_json::to_value(tool_end).expect("tool event");
        let terminal = serde_json::to_value(terminal).expect("terminal event");

        assert_eq!(tool_end["type"], "tool_end");
        assert_eq!(tool_end["exit_code"], 1);
        assert_eq!(tool_end["error_kind"]["type"], "cancelled");
        assert_eq!(tool_end["metadata"]["cancelled"], true);
        assert_eq!(terminal["type"], "agent_end");
        assert_eq!(terminal["text"], "DeepResearch was cancelled by the user.");
    }

    #[tokio::test]
    async fn artifact_lookup_uses_run_identity_and_kind() {
        let workspace = tempfile::tempdir().expect("workspace");
        let report_dir = workspace
            .path()
            .join(".a3s/research/artifacts/artifact-path-test");
        std::fs::create_dir_all(&report_dir).expect("report directory");
        std::fs::write(report_dir.join("index.html"), "<!doctype html>").expect("report");
        std::fs::write(report_dir.join("report.md"), "# Report").expect("markdown");

        let (valid, content_type) =
            validated_run_artifact_path(workspace.path(), "artifact-path-test", "html")
                .await
                .expect("valid report identity");
        assert!(valid.ends_with(".a3s/research/artifacts/artifact-path-test/index.html"));
        assert_eq!(content_type, "text/html; charset=utf-8");
        assert!(
            validated_run_artifact_path(workspace.path(), "../index", "html")
                .await
                .is_err()
        );
        assert!(
            validated_run_artifact_path(workspace.path(), "artifact-path-test", "path")
                .await
                .is_err()
        );
    }
}
