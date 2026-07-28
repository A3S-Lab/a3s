use std::fmt;
use std::io::{self, Write};
use std::process::ExitCode;

use serde_json::{json, Value};

use super::args::OutputMode;

pub(crate) fn render_value(
    mode: OutputMode,
    command: &'static str,
    data: Value,
    human: impl FnOnce(),
) -> anyhow::Result<()> {
    render_value_with_warnings(mode, command, data, Vec::new(), human)
}

pub(crate) fn render_value_with_warnings(
    mode: OutputMode,
    command: &'static str,
    data: Value,
    warnings: Vec<String>,
    human: impl FnOnce(),
) -> anyhow::Result<()> {
    match mode {
        OutputMode::Human => human(),
        OutputMode::Json => write_json(&json!({
            "schemaVersion": 1,
            "command": command,
            "ok": true,
            "data": data,
            "warnings": warnings,
        }))?,
        OutputMode::Jsonl => {
            return Err(usage_error(format!(
                "`{command}` does not support JSONL output"
            )))
        }
    }
    Ok(())
}

pub(crate) fn write_json(value: &Value) -> anyhow::Result<()> {
    write_stdout(&serde_json::to_vec_pretty(value)?)
}

pub(crate) fn write_jsonl(value: &Value) -> anyhow::Result<()> {
    write_stdout(&serde_json::to_vec(value)?)
}

fn write_stdout(bytes: &[u8]) -> anyhow::Result<()> {
    let stdout = io::stdout();
    let mut stdout = stdout.lock();
    stdout.write_all(bytes)?;
    stdout.write_all(b"\n")?;
    stdout.flush()?;
    Ok(())
}

pub(crate) fn render_error(
    mode: OutputMode,
    command: &'static str,
    error: &anyhow::Error,
) -> ExitCode {
    if is_broken_pipe(error) {
        return ExitCode::SUCCESS;
    }
    let structured = error.downcast_ref::<CliError>();
    let component_batch = error.downcast_ref::<a3s::components::ComponentBatchFailure>();
    let plan_mismatch = error.downcast_ref::<a3s::components::ComponentPlanMismatch>();
    let message = structured
        .map(|error| error.message.clone())
        .or_else(|| component_batch.map(ToString::to_string))
        .or_else(|| plan_mismatch.map(ToString::to_string))
        .unwrap_or_else(|| format!("{error:#}"));
    let code = structured
        .map(|error| error.code)
        .or_else(|| {
            component_batch.map(|batch| {
                if batch.is_partial() {
                    "component.partial"
                } else {
                    "component.failed"
                }
            })
        })
        .or_else(|| plan_mismatch.map(|_| "component.plan_mismatch"))
        .unwrap_or("operation.failed");
    let suggestion = structured
        .and_then(|error| error.suggestion.as_deref())
        .or_else(|| {
            component_batch
                .is_some()
                .then_some("Inspect each component failure and rerun only failed targets.")
        })
        .or_else(|| {
            plan_mismatch
                .is_some()
                .then_some("Rerun with --dry-run, review the new plan, and use its planDigest.")
        });
    let details = structured
        .map(|error| error.details.clone())
        .or_else(|| component_batch.and_then(|batch| serde_json::to_value(batch).ok()))
        .or_else(|| plan_mismatch.and_then(|mismatch| serde_json::to_value(mismatch).ok()))
        .unwrap_or_else(|| json!({}));
    match mode {
        OutputMode::Human => {
            eprintln!("a3s: {message}");
            if let Some(suggestion) = suggestion {
                eprintln!("hint: {suggestion}");
            }
        }
        OutputMode::Json => {
            let value = json!({
                "schemaVersion": 1,
                "command": command,
                "ok": false,
                "error": {
                    "code": code,
                    "message": message,
                    "suggestion": suggestion,
                    "details": details,
                },
                "warnings": [],
            });
            if let Err(error) = write_json(&value) {
                return output_write_failure(&error);
            }
        }
        OutputMode::Jsonl => {
            let value = json!({
                "schemaVersion": 1,
                "command": command,
                "type": "error",
                "sequence": structured.map(|error| error.jsonl_sequence).unwrap_or(1),
                "ok": false,
                "error": {
                    "code": code,
                    "message": message,
                    "suggestion": suggestion,
                    "details": details,
                },
            });
            if let Err(error) = write_jsonl(&value) {
                return output_write_failure(&error);
            }
        }
    }
    structured
        .map(|error| error.class)
        .or_else(|| {
            component_batch.map(|batch| {
                if batch.is_partial() {
                    ExitClass::Partial
                } else {
                    ExitClass::Failure
                }
            })
        })
        .unwrap_or(ExitClass::Failure)
        .exit_code()
}

fn output_write_failure(error: &anyhow::Error) -> ExitCode {
    if is_broken_pipe(error) {
        ExitCode::SUCCESS
    } else {
        eprintln!("a3s: failed to write command output: {error:#}");
        ExitCode::FAILURE
    }
}

fn is_broken_pipe(error: &anyhow::Error) -> bool {
    error.chain().any(|cause| {
        cause
            .downcast_ref::<io::Error>()
            .is_some_and(|error| error.kind() == io::ErrorKind::BrokenPipe)
    })
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExitClass {
    Failure,
    Usage,
    Partial,
    Cancelled,
}

impl ExitClass {
    fn exit_code(self) -> ExitCode {
        ExitCode::from(match self {
            Self::Failure => 1,
            Self::Usage => 2,
            Self::Partial => 3,
            Self::Cancelled => 130,
        })
    }
}

#[derive(Debug)]
pub(crate) struct CliError {
    code: &'static str,
    message: String,
    suggestion: Option<String>,
    details: Value,
    class: ExitClass,
    jsonl_sequence: u64,
}

impl CliError {
    pub(crate) fn new(code: &'static str, message: impl Into<String>, class: ExitClass) -> Self {
        Self {
            code,
            message: message.into(),
            suggestion: None,
            details: json!({}),
            class,
            jsonl_sequence: 1,
        }
    }

    pub(crate) fn with_suggestion(mut self, suggestion: impl Into<String>) -> Self {
        self.suggestion = Some(suggestion.into());
        self
    }

    pub(crate) fn with_details(mut self, details: Value) -> Self {
        self.details = details;
        self
    }

    pub(crate) fn with_jsonl_sequence(mut self, sequence: u64) -> Self {
        self.jsonl_sequence = sequence.max(1);
        self
    }
}

impl fmt::Display for CliError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CliError {}

pub(crate) fn usage_error(message: impl Into<String>) -> anyhow::Error {
    CliError::new("usage.invalid", message, ExitClass::Usage).into()
}

pub(crate) fn coded_error(
    code: &'static str,
    message: impl Into<String>,
    class: ExitClass,
) -> anyhow::Error {
    CliError::new(code, message, class).into()
}
