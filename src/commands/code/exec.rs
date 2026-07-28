use std::io::IsTerminal;
use std::sync::Arc;

use a3s_code_core::{Agent, AgentEvent};
use anyhow::{bail, Context};
use serde_json::json;
use tokio::io::AsyncReadExt;

use crate::cli::args::{CodeExecArgs, OutputMode};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, write_jsonl, CliError, ExitClass};

const MAX_PROMPT_BYTES: u64 = 16 * 1024 * 1024;

pub(super) async fn run(args: CodeExecArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let prompt_file = args.prompt_file.map(|path| context.resolve_path(path));
    let prompt = read_prompt(args.prompt, prompt_file.as_deref()).await?;
    let (_, code_config) = crate::commands::config::load_active_config(context)?;
    let agent = Agent::from_config(code_config.clone())
        .await
        .map_err(|error| anyhow::anyhow!("failed to load A3S Code: {error}"))?;
    let session_id = execution_id();
    let workspace = &context.directory;
    let mut options = super::exec_policy::session_options(args.mode, workspace, &session_id);
    if let Some(model) = args.model {
        options = options.with_model(model);
    }
    let client =
        crate::session_llm::resolve_session_llm_client(&code_config, &options, &session_id)
            .map_err(anyhow::Error::msg)?;
    options = options.with_llm_client(Arc::clone(&client));
    let session = agent
        .session_builder(workspace.to_string_lossy().to_string())
        .options(options)
        .build()
        .await?;

    let (mut receiver, worker) = session.stream(&prompt, None).await?;
    let mut sequence = 1u64;
    let mut streamed = String::new();
    let mut final_text = String::new();
    let mut usage = serde_json::Value::Null;
    let mut approval_required = None;
    let mut runtime_error = None;
    let mut completed = false;
    let mut cancelled = false;
    loop {
        let event = tokio::select! {
            event = receiver.recv() => event,
            _ = context.cancellation.cancelled() => {
                let _ = session.cancel().await;
                cancelled = true;
                None
            }
        };
        let Some(event) = event else {
            break;
        };
        match &event {
            AgentEvent::TextDelta { text } => {
                streamed.push_str(text);
                if output == OutputMode::Human {
                    print!("{text}");
                }
            }
            AgentEvent::ToolStart { name, .. } if output == OutputMode::Human => {
                eprintln!("tool: {name}");
            }
            AgentEvent::ConfirmationRequired {
                tool_id, tool_name, ..
            } => {
                approval_required = Some(tool_name.clone());
                let _ = session
                    .confirm_tool_use(
                        tool_id,
                        false,
                        Some(
                            "non-interactive execution cannot request hidden approval".to_string(),
                        ),
                    )
                    .await;
                let _ = session.cancel().await;
                if output == OutputMode::Human {
                    eprintln!("denied approval-required tool: {tool_name}");
                }
            }
            AgentEvent::End {
                text,
                usage: event_usage,
                ..
            } => {
                final_text = text.clone();
                usage = serde_json::to_value(event_usage)?;
                completed = true;
            }
            AgentEvent::Error { message } => {
                runtime_error = Some(message.clone());
                if output == OutputMode::Human {
                    eprintln!("error: {message}");
                }
            }
            _ => {}
        }
        if output == OutputMode::Jsonl {
            write_jsonl(&json!({
                "schemaVersion": 1,
                "command": "code.exec",
                "type": "event",
                "sequence": sequence,
                "event": event,
            }))?;
            sequence += 1;
        }
    }
    let worker_result = worker.await;
    if cancelled {
        return Err(CliError::new(
            "operation.cancelled",
            "code execution cancelled",
            ExitClass::Cancelled,
        )
        .with_jsonl_sequence(sequence)
        .into());
    }
    let worker_error = worker_result.err().map(|error| format!("{error:#}"));
    if let Some(failure) =
        terminal_failure(approval_required, runtime_error, worker_error, completed)
    {
        return Err(failure.into_cli_error(sequence).into());
    }
    let emitted_stream = !streamed.is_empty();
    if final_text.is_empty() {
        final_text = streamed.clone();
    }
    if output == OutputMode::Human {
        if !final_text.is_empty() && !emitted_stream {
            println!("{final_text}");
        } else if emitted_stream && !streamed.ends_with('\n') {
            println!();
        }
        return Ok(());
    }
    if output == OutputMode::Jsonl {
        write_jsonl(&json!({
            "schemaVersion": 1,
            "command": "code.exec",
            "type": "result",
            "sequence": sequence,
            "ok": true,
            "data": {"text": final_text, "usage": usage, "sessionId": session_id},
        }))?;
        return Ok(());
    }
    render_value(
        output,
        "code.exec",
        json!({"text": final_text, "usage": usage, "sessionId": session_id}),
        || {},
    )
}

#[derive(Debug, Eq, PartialEq)]
enum TerminalFailure {
    ApprovalRequired { tool_name: String },
    Runtime { message: String },
    Worker { message: String },
    Incomplete,
}

impl TerminalFailure {
    fn code(&self) -> &'static str {
        match self {
            Self::ApprovalRequired { .. } => "approval.required",
            Self::Runtime { .. } | Self::Worker { .. } => "code.exec.failed",
            Self::Incomplete => "code.exec.incomplete",
        }
    }

    fn into_cli_error(self, sequence: u64) -> CliError {
        let code = self.code();
        match self {
            Self::ApprovalRequired { tool_name } => CliError::new(
                code,
                format!(
                    "tool `{tool_name}` requires approval that cannot be requested in non-interactive execution"
                ),
                ExitClass::Failure,
            )
            .with_suggestion(
                "Run the task interactively with `a3s code` or adjust the configured permission policy.",
            )
            .with_details(json!({"tool": tool_name}))
            .with_jsonl_sequence(sequence),
            Self::Runtime { message } | Self::Worker { message } => CliError::new(
                code,
                format!("code execution failed: {message}"),
                ExitClass::Failure,
            )
            .with_jsonl_sequence(sequence),
            Self::Incomplete => CliError::new(
                code,
                "code execution ended without a terminal completion event",
                ExitClass::Failure,
            )
            .with_jsonl_sequence(sequence),
        }
    }
}

fn terminal_failure(
    approval_required: Option<String>,
    runtime_error: Option<String>,
    worker_error: Option<String>,
    completed: bool,
) -> Option<TerminalFailure> {
    if let Some(tool_name) = approval_required {
        return Some(TerminalFailure::ApprovalRequired { tool_name });
    }
    if let Some(message) = runtime_error {
        return Some(TerminalFailure::Runtime { message });
    }
    if !completed {
        return Some(TerminalFailure::Incomplete);
    }
    if let Some(message) = worker_error {
        return Some(TerminalFailure::Worker { message });
    }
    None
}

async fn read_prompt(
    prompt: Option<String>,
    prompt_file: Option<&std::path::Path>,
) -> anyhow::Result<String> {
    let prompt = if let Some(prompt) = prompt {
        prompt
    } else if let Some(path) = prompt_file {
        let metadata = std::fs::metadata(path)
            .with_context(|| format!("could not inspect prompt file {}", path.display()))?;
        if !metadata.is_file() || metadata.len() > MAX_PROMPT_BYTES {
            bail!("prompt file must be a regular UTF-8 file no larger than 16 MiB");
        }
        std::fs::read_to_string(path)
            .with_context(|| format!("could not read prompt file {}", path.display()))?
    } else {
        if std::io::stdin().is_terminal() {
            bail!("a prompt, --prompt-file, or piped stdin is required");
        }
        let mut bytes = Vec::new();
        tokio::io::stdin()
            .take(MAX_PROMPT_BYTES + 1)
            .read_to_end(&mut bytes)
            .await
            .context("could not read prompt from stdin")?;
        if bytes.len() as u64 > MAX_PROMPT_BYTES {
            bail!("piped prompt exceeds 16 MiB");
        }
        String::from_utf8(bytes).context("piped prompt must be UTF-8")?
    };
    if prompt.trim().is_empty() {
        bail!("prompt is empty");
    }
    Ok(prompt)
}

fn execution_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    format!("exec-{nanos:016x}-{:x}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approval_failure_precedes_worker_cancellation() {
        let failure = terminal_failure(
            Some("bash".to_string()),
            None,
            Some("task was cancelled".to_string()),
            false,
        )
        .expect("approval must be terminal");

        assert_eq!(failure.code(), "approval.required");
        assert_eq!(
            failure,
            TerminalFailure::ApprovalRequired {
                tool_name: "bash".to_string()
            }
        );
    }

    #[test]
    fn agent_error_is_a_failed_execution_even_if_the_worker_completes() {
        let failure = terminal_failure(None, Some("provider failed".to_string()), None, false)
            .expect("AgentEvent::Error must be terminal");

        assert_eq!(failure.code(), "code.exec.failed");
        assert_eq!(
            failure,
            TerminalFailure::Runtime {
                message: "provider failed".to_string()
            }
        );
    }

    #[test]
    fn closed_stream_without_end_is_incomplete() {
        let failure = terminal_failure(
            None,
            None,
            Some("worker stopped before completion".to_string()),
            false,
        )
        .expect("a stream without AgentEvent::End must not succeed");

        assert_eq!(failure.code(), "code.exec.incomplete");
        assert_eq!(failure, TerminalFailure::Incomplete);
    }

    #[test]
    fn worker_failure_after_end_is_a_failed_execution() {
        let failure = terminal_failure(
            None,
            None,
            Some("worker failed to settle".to_string()),
            true,
        )
        .expect("a failed worker must not report success");

        assert_eq!(failure.code(), "code.exec.failed");
        assert_eq!(
            failure,
            TerminalFailure::Worker {
                message: "worker failed to settle".to_string()
            }
        );
    }

    #[test]
    fn terminal_completion_without_errors_can_succeed() {
        assert_eq!(terminal_failure(None, None, None, true), None);
    }
}
