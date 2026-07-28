pub(super) async fn call_generation_with_progress(
    session: &AgentSession,
    generation_args: Value,
    progress_tx: &mpsc::Sender<AgentEvent>,
    run_clock: &EvidenceFirstRunClock,
    stage_label: &str,
    execution_timeout_ms: u64,
    max_attempts: u8,
) -> Result<ToolCallResult, String> {
    if !(1..=2).contains(&max_attempts) {
        return Err(format!(
            "durable {stage_label} generation requires one or two attempts"
        ));
    }
    let durable_input = serde_json::json!({
        "generation_args": generation_args,
        "max_attempts": max_attempts,
    });
    let encoded = serde_json::to_vec(&durable_input)
        .map_err(|error| format!("encode durable {stage_label} generation input: {error}"))?;
    let mut digest = Sha256::new();
    digest.update(&encoded);
    let digest = format!("{:x}", digest.finalize());
    let label = stable_generation_label(stage_label);
    let workflow_run_id = format!("{}-{label}-{}", run_clock.run_id(), &digest[..16]);
    let workflow_args = serde_json::json!({
        "source": DURABLE_GENERATION_WORKFLOW_SOURCE,
        "input": durable_input,
        "run_id": workflow_run_id,
        "limits": {
            "timeoutMs": execution_timeout_ms,
            "maxToolCalls": 4,
            "maxOutputBytes": 1024 * 1024,
        }
    });
    let workflow = call_tool_with_progress(
        session,
        "dynamic_workflow",
        workflow_args,
        progress_tx,
        true,
    )
    .await?;
    if workflow.exit_code != 0 {
        return Err(workflow
            .output
            .lines()
            .next()
            .unwrap_or("durable structured-generation workflow failed")
            .to_string());
    }
    let canonical =
        deep_research_canonical_workflow_output(&workflow.output, workflow.metadata.as_ref());
    let output = serde_json::from_str::<Value>(&canonical)
        .map_err(|error| format!("decode durable {stage_label} workflow output: {error}"))?;
    let result = output
        .get("result")
        .ok_or_else(|| format!("durable {stage_label} workflow omitted its generation result"))?;
    let result = tool_result_from_durable_generation(result, stage_label)?;
    Ok(result)
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
            .ok_or_else(|| format!("durable {stage_label} generation result omitted its output"))?
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

pub(super) async fn run_dynamic_workflow(
    session: &AgentSession,
    args: Value,
    progress_tx: &mpsc::Sender<AgentEvent>,
) -> Result<ToolCallResult, String> {
    let result =
        call_tool_with_progress(session, "dynamic_workflow", args, progress_tx, true).await?;
    if result.exit_code != 0 {
        return Err(result
            .output
            .lines()
            .next()
            .unwrap_or("dynamic_workflow failed without an error message")
            .to_string());
    }
    Ok(result)
}

pub(super) async fn run_bootstrap_acquisition_stage(
    session: &AgentSession,
    args: Value,
    progress_tx: &mpsc::Sender<AgentEvent>,
    timeout_ms: u64,
) -> Result<ToolCallResult, String> {
    let recovery_args = args.clone();
    let result = within_inquiry_stage_timeout_typed(
        run_dynamic_workflow(session, args, progress_tx),
        timeout_ms,
        "bootstrap acquisition",
    )
    .await;
    match result {
        Ok(result) => Ok(result),
        Err(error @ InquiryStageError::TimedOut { .. }) => {
            recover_bootstrap_acquisition_after_timeout(session, &recovery_args)
                .ok_or_else(|| error.to_string())
        }
        Err(error) => Err(error.to_string()),
    }
}

fn recover_bootstrap_acquisition_after_timeout(
    session: &AgentSession,
    args: &Value,
) -> Option<ToolCallResult> {
    let recovered =
        recover_deep_research_bootstrap_acquisition_from_store(session.workspace(), args)?;
    let output = recovered.output?;
    let expected_query = args.pointer("/input/query").and_then(Value::as_str)?;
    bootstrap_acquisition_value(&output, expected_query)?;
    Some(ToolCallResult {
        name: "dynamic_workflow".to_string(),
        output,
        exit_code: 0,
        metadata: Some(recovered.metadata),
        error_kind: None,
    })
}

fn bootstrap_acquisition_value(output: &str, expected_query: &str) -> Option<Value> {
    let value = serde_json::from_str::<Value>(output).ok()?;
    if value.get("query").and_then(Value::as_str) != Some(expected_query)
        || value.get("mode").and_then(Value::as_str) != Some("bootstrap_acquisition")
        || value
            .pointer("/execution/terminal_authority")
            .and_then(Value::as_str)
            != Some("host_inquiry_reducer")
    {
        return None;
    }
    let acquisition = value.get("acquisition")?.clone();
    let sources = acquisition.pointer("/packet/sources")?.as_array()?;
    if sources.is_empty() || sources.len() > 16 {
        return None;
    }
    let valid = sources.iter().all(|source| {
        source
            .get("source_id")
            .and_then(Value::as_str)
            .is_some_and(|id| !id.trim().is_empty())
            && source
                .get("url_or_path")
                .and_then(Value::as_str)
                .is_some_and(|anchor| !anchor.trim().is_empty())
            && source
                .get("chunks")
                .and_then(Value::as_array)
                .is_some_and(|chunks| {
                    !chunks.is_empty()
                        && chunks.iter().all(|chunk| {
                            chunk
                                .get("chunk_id")
                                .and_then(Value::as_str)
                                .is_some_and(|id| !id.trim().is_empty())
                                && chunk
                                    .get("text")
                                    .and_then(Value::as_str)
                                    .is_some_and(|text| !text.trim().is_empty())
                        })
                })
    });
    valid.then_some(acquisition)
}

pub(super) async fn within_inquiry_stage_timeout<T, F>(
    future: F,
    timeout_ms: u64,
    stage: &str,
) -> Result<T, String>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    within_inquiry_stage_timeout_typed(future, timeout_ms, stage)
        .await
        .map_err(|error| error.to_string())
}

#[derive(Debug, Eq, PartialEq)]
enum InquiryStageError {
    Operation(String),
    TimedOut { stage: String, timeout_ms: u64 },
}

impl std::fmt::Display for InquiryStageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Operation(error) => formatter.write_str(error),
            Self::TimedOut { stage, timeout_ms } => write!(
                formatter,
                "DeepResearch {stage} stage timed out after {timeout_ms} ms"
            ),
        }
    }
}

async fn within_inquiry_stage_timeout_typed<T, F>(
    future: F,
    timeout_ms: u64,
    stage: &str,
) -> Result<T, InquiryStageError>
where
    F: std::future::Future<Output = Result<T, String>>,
{
    match tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), future).await {
        Ok(result) => result.map_err(InquiryStageError::Operation),
        Err(_) => Err(InquiryStageError::TimedOut {
            stage: stage.to_string(),
            timeout_ms,
        }),
    }
}

pub(super) async fn call_tool_with_progress(
    session: &AgentSession,
    name: &str,
    args: Value,
    progress_tx: &mpsc::Sender<AgentEvent>,
    filter_dynamic_workflow_envelope: bool,
) -> Result<ToolCallResult, String> {
    let (progress_rx, join) = session.tool_with_events(name, args);
    forward_tool_call_with_progress(
        name,
        progress_rx,
        join,
        progress_tx,
        filter_dynamic_workflow_envelope,
    )
    .await
}

pub(super) async fn forward_tool_call_with_progress(
    name: &str,
    mut progress_rx: mpsc::Receiver<AgentEvent>,
    mut join: tokio::task::JoinHandle<a3s_code_core::Result<ToolCallResult>>,
    progress_tx: &mpsc::Sender<AgentEvent>,
    filter_dynamic_workflow_envelope: bool,
) -> Result<ToolCallResult, String> {
    let abort = join.abort_handle();
    let mut abort_on_drop = AbortInnerToolOnDrop(Some(abort.clone()));
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
                if filter_dynamic_workflow_envelope && is_dynamic_workflow_envelope(&event) {
                    continue;
                }
                if progress_tx.send(event).await.is_err() {
                    abort.abort();
                    return Err("DeepResearch progress consumer closed".to_string());
                }
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
        if progress_tx.send(event).await.is_err() {
            break;
        }
    }
    result
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

pub(super) fn generated_object<T: DeserializeOwned>(result: &ToolCallResult) -> Result<T, String> {
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

#[cfg(test)]
mod timeout_tests {
    use super::{within_inquiry_stage_timeout_typed, InquiryStageError};

    #[tokio::test]
    async fn operation_text_cannot_impersonate_a_typed_timeout() {
        let result = within_inquiry_stage_timeout_typed(
            async {
                Err::<(), _>(
                    "DeepResearch bootstrap acquisition stage timed out after 1 ms".to_string(),
                )
            },
            1_000,
            "bootstrap acquisition",
        )
        .await;

        assert!(matches!(result, Err(InquiryStageError::Operation(_))));
    }
}
