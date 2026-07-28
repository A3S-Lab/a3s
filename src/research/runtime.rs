use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::{AgentEvent, AgentSession, ToolCallResult};
use a3s_deep_research::engine::{
    DeepResearchEvent, GenerationRequest, ProgressPort, PublicationPort, PublicationRequest,
    StructuredGenerationPort, WorkflowExecutionPort, WorkflowOutput, WorkflowRequest,
};
use a3s_deep_research::report::{
    canonical_workflow_output, materialize_deep_research_admitted_report_for_run,
    materialize_deep_research_no_evidence_report_for_run_in_language,
    materialize_deep_research_source_backed_report_for_run_in_language,
    record_deep_research_publication_receipt_in_language, DeepResearchEvidenceFirstPublication,
    ResearchReportArtifacts,
};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use super::journal::CodeDeepResearchJournal;
use super::CodeDeepResearchEvent;

pub(super) struct CodeDeepResearchRuntime {
    session: Arc<AgentSession>,
    run_id: String,
    journal: Arc<CodeDeepResearchJournal>,
    events: mpsc::Sender<CodeDeepResearchEvent>,
}

impl CodeDeepResearchRuntime {
    pub(super) fn new(
        session: Arc<AgentSession>,
        run_id: String,
        journal: Arc<CodeDeepResearchJournal>,
        events: mpsc::Sender<CodeDeepResearchEvent>,
    ) -> Self {
        Self {
            session,
            run_id,
            journal,
            events,
        }
    }

    fn validate_run_id(&self, run_id: &str) -> Result<(), String> {
        if run_id == self.run_id {
            Ok(())
        } else {
            Err("publication request belongs to a different DeepResearch run".to_string())
        }
    }

    fn emit_agent_event(&self, event: AgentEvent) {
        let _ = self.events.try_send(CodeDeepResearchEvent::Agent(event));
    }

    async fn call_tool(
        &self,
        name: &str,
        args: Value,
        filter_dynamic_workflow_envelope: bool,
    ) -> Result<ToolCallResult, String> {
        let (mut progress_rx, mut join) = self.session.tool_with_events(name, args);
        let abort = join.abort_handle();
        let mut abort_on_drop = AbortInnerToolOnDrop(Some(abort));
        let mut progress_open = true;
        let result = loop {
            if !progress_open {
                let result = join
                    .await
                    .map_err(|error| format!("{name} task failed: {error}"))?
                    .map_err(|error| format!("{name} failed: {error}"));
                abort_on_drop.disarm();
                break result;
            }
            tokio::select! {
                biased;
                event = progress_rx.recv() => {
                    let Some(event) = event else {
                        progress_open = false;
                        continue;
                    };
                    if filter_dynamic_workflow_envelope
                        && is_dynamic_workflow_envelope(&event)
                    {
                        continue;
                    }
                    self.emit_agent_event(event);
                }
                result = &mut join => {
                    let result = result
                        .map_err(|error| format!("{name} task failed: {error}"))?
                        .map_err(|error| format!("{name} failed: {error}"));
                    abort_on_drop.disarm();
                    break result;
                }
            }
        };
        while let Ok(event) = progress_rx.try_recv() {
            if filter_dynamic_workflow_envelope && is_dynamic_workflow_envelope(&event) {
                continue;
            }
            self.emit_agent_event(event);
        }
        result
    }

    async fn call_generation(&self, request: GenerationRequest) -> Result<ToolCallResult, String> {
        if !(1..=2).contains(&request.max_attempts) {
            return Err("durable generation requires one or two attempts".to_string());
        }
        let stage_label = request.stage.label();
        let durable_input = serde_json::json!({
            "generation_args": request.arguments,
            "max_attempts": request.max_attempts,
        });
        let encoded = serde_json::to_vec(&durable_input)
            .map_err(|error| format!("encode durable {stage_label} generation: {error}"))?;
        let digest = format!("{:x}", Sha256::digest(encoded));
        let workflow_run_id = format!(
            "{}-{}-{}",
            self.run_id,
            stable_generation_label(stage_label),
            &digest[..16]
        );
        let workflow_args = serde_json::json!({
            "source": a3s_deep_research::workflow::GENERATION_WORKFLOW_SOURCE,
            "input": durable_input,
            "run_id": workflow_run_id,
            "limits": {
                "timeoutMs": request.execution_timeout_ms,
                "maxToolCalls": 4,
                "maxOutputBytes": 1024 * 1024,
            }
        });
        let workflow = tokio::time::timeout(
            Duration::from_millis(request.execution_timeout_ms),
            self.call_tool("dynamic_workflow", workflow_args, true),
        )
        .await
        .map_err(|_| {
            format!(
                "durable {stage_label} generation timed out after {} ms",
                request.execution_timeout_ms
            )
        })??;
        if workflow.exit_code != 0 {
            return Err(workflow
                .output
                .lines()
                .next()
                .unwrap_or("durable structured-generation workflow failed")
                .to_string());
        }
        let canonical = canonical_workflow_output(&workflow.output, workflow.metadata.as_ref());
        let output = serde_json::from_str::<Value>(&canonical)
            .map_err(|error| format!("decode durable {stage_label} workflow: {error}"))?;
        let result = output
            .get("result")
            .ok_or_else(|| format!("durable {stage_label} workflow omitted its result"))?;
        tool_result_from_durable_generation(result, stage_label)
    }
}

#[async_trait::async_trait]
impl StructuredGenerationPort for CodeDeepResearchRuntime {
    async fn generate_object(&self, request: GenerationRequest) -> Result<Value, String> {
        let result = self.call_generation(request).await?;
        generated_object::<Value>(&result)
    }
}

#[async_trait::async_trait]
impl WorkflowExecutionPort for CodeDeepResearchRuntime {
    async fn execute_workflow(&self, request: WorkflowRequest) -> Result<WorkflowOutput, String> {
        let arguments = adapt_dynamic_workflow_arguments(request.arguments);
        let result = tokio::time::timeout(
            Duration::from_millis(request.timeout_ms),
            self.call_tool("dynamic_workflow", arguments, true),
        )
        .await
        .map_err(|_| {
            format!(
                "DeepResearch {} timed out after {} ms",
                request.stage.label(),
                request.timeout_ms
            )
        })??;
        if result.exit_code != 0 {
            return Err(result
                .output
                .lines()
                .next()
                .unwrap_or("dynamic_workflow failed without a diagnostic")
                .to_string());
        }
        Ok(WorkflowOutput {
            output: result.output,
            metadata: result.metadata,
        })
    }
}

/// Keep the standalone engine's request compatible with the exact dynamic
/// workflow schema published by the pinned Code Core release.
pub(crate) fn adapt_dynamic_workflow_arguments(mut arguments: Value) -> Value {
    if let Some(limits) = arguments.get_mut("limits").and_then(Value::as_object_mut) {
        limits.remove("maxConcurrentGenerations");
    }
    arguments
}

#[async_trait::async_trait]
impl PublicationPort for CodeDeepResearchRuntime {
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
                self.validate_run_id(&run_id)?;
                let artifacts = materialize_deep_research_source_backed_report_for_run_in_language(
                    self.session.workspace(),
                    &run_id,
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
                self.validate_run_id(&run_id)?;
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
                let artifacts = materialize_deep_research_admitted_report_for_run(
                    self.session.workspace(),
                    &run_id,
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
                self.validate_run_id(&run_id)?;
                let artifacts = materialize_deep_research_no_evidence_report_for_run_in_language(
                    self.session.workspace(),
                    &run_id,
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

#[async_trait::async_trait]
impl ProgressPort for CodeDeepResearchRuntime {
    async fn report_progress(
        &self,
        progress: a3s_deep_research::engine::ResearchProgress,
    ) -> Result<(), String> {
        let event = match progress {
            a3s_deep_research::engine::ResearchProgress::Started(stage) => {
                DeepResearchEvent::StageStarted {
                    run_id: self.run_id.clone(),
                    stage,
                }
            }
            a3s_deep_research::engine::ResearchProgress::Completed(stage) => {
                DeepResearchEvent::StageCompleted {
                    run_id: self.run_id.clone(),
                    stage,
                }
            }
            a3s_deep_research::engine::ResearchProgress::Degraded { stage, reason } => {
                DeepResearchEvent::StageDegraded {
                    run_id: self.run_id.clone(),
                    stage,
                    reason,
                }
            }
        };
        self.report_event(event).await
    }

    async fn report_event(&self, event: DeepResearchEvent) -> Result<(), String> {
        self.journal.append(&event).await?;
        let _ = self.events.try_send(CodeDeepResearchEvent::Engine(event));
        Ok(())
    }
}

struct AbortInnerToolOnDrop(Option<tokio::task::AbortHandle>);

impl AbortInnerToolOnDrop {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for AbortInnerToolOnDrop {
    fn drop(&mut self) {
        if let Some(abort) = self.0.take() {
            abort.abort();
        }
    }
}

fn stable_generation_label(label: &str) -> String {
    let label = label
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '-'
            }
        })
        .collect::<String>();
    if label.is_empty() {
        "generation".to_string()
    } else {
        label
    }
}

fn tool_result_from_durable_generation(
    value: &Value,
    stage_label: &str,
) -> Result<ToolCallResult, String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("durable {stage_label} generation returned a non-object result"))?;
    Ok(ToolCallResult {
        name: object
            .get("name")
            .or_else(|| object.get("tool"))
            .and_then(Value::as_str)
            .unwrap_or("generate_object")
            .to_string(),
        output: object
            .get("output")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("durable {stage_label} generation omitted its output"))?
            .to_string(),
        exit_code: object
            .get("exit_code")
            .and_then(Value::as_i64)
            .and_then(|value| i32::try_from(value).ok())
            .unwrap_or_default(),
        metadata: object.get("metadata").cloned(),
        error_kind: None,
    })
}

fn generated_object<T: DeserializeOwned>(result: &ToolCallResult) -> Result<T, String> {
    if result.exit_code != 0 {
        return Err(result
            .output
            .lines()
            .next()
            .unwrap_or("structured generation failed")
            .to_string());
    }
    let envelope = serde_json::from_str::<Value>(&result.output)
        .map_err(|error| format!("structured generation returned invalid JSON: {error}"))?;
    let object = envelope
        .get("object")
        .cloned()
        .ok_or_else(|| "structured generation response omitted object".to_string())?;
    serde_json::from_value(object)
        .map_err(|error| format!("structured generation object violated its contract: {error}"))
}

fn is_dynamic_workflow_envelope(event: &AgentEvent) -> bool {
    match event {
        AgentEvent::ToolStart { name, .. }
        | AgentEvent::ToolExecutionStart { name, .. }
        | AgentEvent::ToolOutputDelta { name, .. }
        | AgentEvent::ToolEnd { name, .. } => name == "dynamic_workflow",
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn workflow_arguments_match_the_published_code_core_schema() {
        let arguments = serde_json::json!({
            "source": "async function run() {}",
            "limits": {
                "timeoutMs": 600_000,
                "maxToolCalls": 56,
                "maxOutputBytes": 8_388_608,
                "maxConcurrentGenerations": 2,
            },
        });

        let adapted = adapt_dynamic_workflow_arguments(arguments);

        assert_eq!(adapted["limits"]["timeoutMs"], 600_000);
        assert_eq!(adapted["limits"]["maxToolCalls"], 56);
        assert_eq!(adapted["limits"]["maxOutputBytes"], 8_388_608);
        assert!(adapted["limits"].get("maxConcurrentGenerations").is_none());
    }
}
