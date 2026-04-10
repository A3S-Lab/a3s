use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use crate::{BoxRuntimeSpec, BoxWorkloadEnvelope, ExecutionLaunchMode, ExecutionRegistry};
use crate::{RuntimeClass, WorkloadKind};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GuestExecutorArgs {
    pub executor: String,
    pub handler: String,
    pub input: PathBuf,
    pub output: PathBuf,
}

impl GuestExecutorArgs {
    pub fn parse<I, S>(args: I) -> Result<Self, String>
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut executor = None;
        let mut handler = None;
        let mut input = None;
        let mut output = None;

        let mut iter = args.into_iter().map(Into::into);
        while let Some(arg) = iter.next() {
            match arg.as_str() {
                "--executor" => executor = iter.next(),
                "--handler" => handler = iter.next(),
                "--input" => input = iter.next().map(PathBuf::from),
                "--output" => output = iter.next().map(PathBuf::from),
                other => return Err(format!("unsupported executor arg: {other}")),
            }
        }

        Ok(Self {
            executor: executor.ok_or_else(|| "missing required arg `--executor`".to_string())?,
            handler: handler.ok_or_else(|| "missing required arg `--handler`".to_string())?,
            input: input.ok_or_else(|| "missing required arg `--input`".to_string())?,
            output: output.ok_or_else(|| "missing required arg `--output`".to_string())?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct GuestExecutorOutput {
    pub runtime_class: RuntimeClass,
    pub workload_kind: WorkloadKind,
    pub executor: String,
    pub handler: String,
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// Writes a structured error output when execution fails before or after the workload runs.
fn write_error_output(
    output_path: &PathBuf,
    executor: &str,
    handler: &str,
    error: &str,
) -> Result<(), String> {
    let output = GuestExecutorOutput {
        runtime_class: RuntimeClass::A3sBox,
        workload_kind: WorkloadKind::ExecutionTask,
        executor: executor.to_string(),
        handler: handler.to_string(),
        success: false,
        result: None,
        error: Some(error.to_string()),
    };
    write_executor_output(output_path, output)
}

/// Writes the executor output to disk, creating parent directories as needed.
fn write_executor_output(output_path: &PathBuf, output: GuestExecutorOutput) -> Result<(), String> {
    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "failed to create executor output directory {}: {e}",
                parent.display()
            )
        })?;
    }
    fs::write(
        output_path,
        serde_json::to_vec_pretty(&output)
            .map_err(|e| format!("failed to serialize executor output: {e}"))?,
    )
    .map_err(|e| {
        format!(
            "failed to write executor output {}: {e}",
            output_path.display()
        )
    })?;
    Ok(())
}

pub async fn run_guest_executor(args: GuestExecutorArgs) -> Result<(), String> {
    // Read and parse input — write error output if either step fails.
    let input = match fs::read(&args.input) {
        Ok(bytes) => bytes,
        Err(e) => {
            let msg = format!(
                "failed to read executor input {}: {e}",
                args.input.display()
            );
            write_error_output(&args.output, &args.executor, &args.handler, &msg)?;
            return Err(msg);
        }
    };
    let input = match serde_json::from_slice::<serde_json::Value>(&input) {
        Ok(value) => value,
        Err(e) => {
            let msg = format!("failed to parse executor input as json: {e}");
            write_error_output(&args.output, &args.executor, &args.handler, &msg)?;
            return Err(msg);
        }
    };

    // Build execution registry — write error output if creation fails.
    let registry = match execution_registry_from_env() {
        Ok(r) => r,
        Err(e) => {
            write_error_output(&args.output, &args.executor, &args.handler, &e)?;
            return Err(e);
        }
    };
    let timeout = executor_timeout_from_env();

    let envelope = BoxWorkloadEnvelope {
        runtime_class: RuntimeClass::A3sBox,
        workload_kind: WorkloadKind::ExecutionTask,
        runtime: BoxRuntimeSpec {
            runtime: format!("a3s/executor/{}", args.executor),
            entrypoint: "a3s-executor".into(),
            args: vec![
                "--executor".into(),
                args.executor.clone(),
                "--handler".into(),
                args.handler.clone(),
            ],
            env: std::env::vars()
                .filter(|(key, _)| key.starts_with("A3S_"))
                .collect(),
        },
        input,
        labels: Default::default(),
    };

    // Execute the workload — write error output if execution fails.
    let result = match registry.execute_box_workload(&envelope, timeout).await {
        Ok(r) => r,
        Err(e) => {
            write_error_output(&args.output, &args.executor, &args.handler, &e)?;
            return Err(e);
        }
    };

    let output = GuestExecutorOutput {
        runtime_class: RuntimeClass::A3sBox,
        workload_kind: WorkloadKind::ExecutionTask,
        executor: args.executor,
        handler: args.handler,
        success: true,
        result: Some(result),
        error: None,
    };
    write_executor_output(&args.output, output)?;
    Ok(())
}

fn execution_registry_from_env() -> Result<ExecutionRegistry, String> {
    let enabled_adapters = std::env::var("A3S_LAMBDA_EXECUTION_ENABLED_ADAPTERS")
        .ok()
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect::<Vec<_>>()
        })
        .filter(|values| !values.is_empty())
        .unwrap_or_else(|| vec!["http".to_string()]);

    ExecutionRegistry::from_enabled_with_launch_mode(
        enabled_adapters.iter().map(String::as_str),
        ExecutionLaunchMode::HostAdapterCompat,
    )
}

fn executor_timeout_from_env() -> Duration {
    let secs = std::env::var("A3S_LAMBDA_EXECUTOR_TIMEOUT_SECS")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .filter(|value| *value > 0)
        .unwrap_or(300);
    Duration::from_secs(secs)
}

#[cfg(test)]
mod tests {
    use super::{GuestExecutorArgs, GuestExecutorOutput};
    use crate::{RuntimeClass, WorkloadKind};
    use std::path::PathBuf;

    #[test]
    fn parses_guest_executor_args() {
        let args = GuestExecutorArgs::parse([
            "--executor",
            "http",
            "--handler",
            "extract",
            "--input",
            "/workspace/input.json",
            "--output",
            "/workspace/output.json",
        ])
        .expect("args should parse");

        assert_eq!(args.executor, "http");
        assert_eq!(args.handler, "extract");
        assert_eq!(args.input, PathBuf::from("/workspace/input.json"));
        assert_eq!(args.output, PathBuf::from("/workspace/output.json"));
    }

    #[test]
    fn rejects_missing_required_arg() {
        let err = GuestExecutorArgs::parse(["--executor", "http", "--handler", "get"])
            .expect_err("missing input/output should fail");
        assert!(err.contains("missing required arg"));
    }

    #[test]
    fn serializes_guest_executor_output_contract() {
        let output = GuestExecutorOutput {
            runtime_class: RuntimeClass::A3sBox,
            workload_kind: WorkloadKind::ExecutionTask,
            executor: "http".into(),
            handler: "extract".into(),
            success: true,
            result: Some(serde_json::json!({"status": 200})),
            error: None,
        };

        let json = serde_json::to_value(&output).expect("output should serialize");
        assert_eq!(json["runtime_class"], "a3s_box");
        assert_eq!(json["workload_kind"], "execution_task");
        assert_eq!(json["executor"], "http");
        assert_eq!(json["handler"], "extract");
        assert_eq!(json["success"], true);
        assert_eq!(json["result"]["status"], 200);
        assert!(
            json.get("error").is_none(),
            "error should be omitted when None"
        );
    }

    #[test]
    fn serializes_guest_executor_output_with_error() {
        let output = GuestExecutorOutput {
            runtime_class: RuntimeClass::A3sBox,
            workload_kind: WorkloadKind::ExecutionTask,
            executor: "http".into(),
            handler: "extract".into(),
            success: false,
            result: None,
            error: Some("adapter not found".into()),
        };

        let json = serde_json::to_value(&output).expect("output should serialize");
        assert_eq!(json["success"], false);
        assert!(
            json.get("result").is_none(),
            "result should be omitted when None"
        );
        assert_eq!(json["error"], "adapter not found");
    }
}
