use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use anyhow::{bail, Context};
use serde_json::json;
use tokio::io::{AsyncReadExt, AsyncSeekExt};

use crate::api::serve::{WebInstanceRecord, WebInstanceStatus};
use crate::cli::args::{OutputMode, WebArgs, WebCommand, WebLogsArgs, WebStartArgs, WebTargetArgs};
use crate::cli::context::InvocationContext;
use crate::cli::output::{render_value, usage_error, write_jsonl, CliError, ExitClass};

pub(crate) async fn run(args: WebArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        Some(WebCommand::Start(start)) => start_web(start, context).await,
        Some(WebCommand::Stop(target)) => stop(target, context).await,
        Some(WebCommand::Status(target)) => status(target, context).await,
        Some(WebCommand::Logs(args)) => logs(args, context).await,
        Some(WebCommand::Open(target)) => open(target, context).await,
        None => start_web(args.shortcut, context).await,
    }
}

async fn start_web(args: WebStartArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    if output != OutputMode::Human && !args.detach {
        return Err(usage_error(
            "machine output for foreground `a3s web start` is not supported; use `--detach`",
        ));
    }
    let argv = start_argv(args, context)?;
    match crate::api::run_web(
        &argv,
        &context.cancellation,
        context.network.offline,
        context.network.allow_first_use_install,
    )
    .await?
    {
        crate::api::ServeOutcome::Detached { instance, reused } => {
            let url = (!instance.api_only).then(|| format!("http://{}/", instance.address));
            let api_url = format!("http://{}/api/health", instance.address);
            render_value(
                output,
                "web.start",
                json!({
                    "detached": true,
                    "reused": reused,
                    "managed": true,
                    "pid": instance.pid,
                    "url": url,
                    "apiUrl": api_url,
                    "workspace": instance.workspace,
                    "logPath": instance.log_path,
                }),
                || {
                    if reused {
                        println!("Status:          reused healthy managed instance");
                    }
                    if let Some(url) = url {
                        println!("A3S Web:       {url}");
                    } else {
                        println!("A3S Web:       disabled (--api-only)");
                    }
                    println!("A3S Code API:  {api_url}");
                    println!("Background PID: {}", instance.pid);
                    println!("Log:            {}", instance.log_path.display());
                },
            )
        }
        crate::api::ServeOutcome::Existing(instance) => {
            let url = format!("http://{}/", instance.address);
            let api_url = format!("http://{}/api/health", instance.address);
            render_value(
                output,
                "web.start",
                json!({
                    "detached": instance.managed,
                    "reused": true,
                    "managed": instance.managed,
                    "pid": instance.pid,
                    "url": url,
                    "apiUrl": api_url,
                    "workspace": instance.workspace,
                    "version": instance.version,
                }),
                || {
                    println!("Status:          reused healthy A3S Web instance");
                    println!("A3S Web:         {url}");
                    println!("A3S Code API:    {api_url}");
                    println!("Workspace:       {}", instance.workspace.display());
                    if let Some(version) = instance.version.as_deref() {
                        println!("Version:         {version}");
                    }
                    if !instance.managed {
                        println!("Managed:         no (stop it from its original command)");
                    }
                },
            )
        }
        crate::api::ServeOutcome::Help | crate::api::ServeOutcome::ForegroundStopped => Ok(()),
    }
}

async fn status(target: WebTargetArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let workspace = target_workspace(target, context)?;
    let status = crate::api::serve::instance_status(&workspace).await?;
    render_status(status, workspace, output)
}

async fn stop(target: WebTargetArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let workspace = target_workspace(target, context)?;
    let stopped = crate::api::serve::stop_instance(&workspace).await?;
    let data = match &stopped {
        Some(instance) => json!({
            "stopped": true,
            "pid": instance.pid,
            "address": instance.address,
            "workspace": instance.workspace,
        }),
        None => json!({
            "stopped": false,
            "reason": "not-running",
            "workspace": workspace,
        }),
    };
    render_value(output, "web.stop", data, || match stopped {
        Some(instance) => println!(
            "stopped A3S Web for {} (pid {})",
            instance.workspace.display(),
            instance.pid
        ),
        None => println!("A3S Web is not running for {}", workspace.display()),
    })
}

async fn logs(args: WebLogsArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    if args.lines == 0 {
        return Err(usage_error("--lines must be greater than zero"));
    }
    if args.follow && output == OutputMode::Json {
        return Err(usage_error(
            "`a3s web logs --follow` requires human or JSONL output",
        ));
    }
    let workspace = target_workspace(args.target, context)?;
    let status = crate::api::serve::instance_status(&workspace).await?;
    let instance = status
        .instance
        .ok_or_else(|| anyhow::anyhow!("A3S Web has no managed log for {}", workspace.display()))?;
    let existing = crate::api::serve::read_log_tail(&instance.log_path, args.lines)?;

    let mut next_sequence = 1;
    match output {
        OutputMode::Human => print!("{existing}"),
        OutputMode::Json => {
            render_value(
                output,
                "web.logs",
                json!({"path": instance.log_path, "content": existing}),
                || {},
            )?;
        }
        OutputMode::Jsonl => {
            next_sequence = emit_log_lines(&existing, next_sequence)?;
        }
    }
    if args.follow {
        follow_log(&instance, output, next_sequence, &context.cancellation).await?;
    } else if output == OutputMode::Jsonl {
        emit_log_result(next_sequence, &instance.log_path, false)?;
    }
    Ok(())
}

async fn open(target: WebTargetArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let workspace = target_workspace(target, context)?;
    let instance = crate::api::serve::open_instance(&workspace).await?;
    let url = format!("http://{}/", instance.address);
    let api_only = instance.api_only.unwrap_or(false);
    if output == OutputMode::Human && !api_only {
        open_url(&url, context)?;
    }
    render_value(
        output,
        "web.open",
        json!({
            "url": url,
            "opened": output == OutputMode::Human && !api_only,
            "managed": instance.managed,
        }),
        || {
            if api_only {
                println!(
                    "A3S Web UI is disabled; API: http://{}/api/health",
                    instance.address
                );
            } else {
                println!("opened {url}");
            }
        },
    )
}

fn render_status(
    status: WebInstanceStatus,
    workspace: PathBuf,
    output: OutputMode,
) -> anyhow::Result<()> {
    let instance_data = status
        .instance
        .as_ref()
        .map(|instance| {
            json!({
                "pid": instance.pid,
                "address": instance.address,
                "workspace": instance.workspace,
                "logPath": instance.log_path,
                "startedAtMs": instance.started_at_ms,
                "apiOnly": instance.api_only,
                "version": instance.version,
                "managed": true,
            })
        })
        .or_else(|| {
            status.observed.as_ref().map(|instance| {
                json!({
                    "pid": instance.pid,
                    "address": instance.address,
                    "workspace": instance.workspace,
                    "apiOnly": instance.api_only,
                    "version": instance.version,
                    "managed": false,
                })
            })
        });
    let data = json!({
        "running": status.running,
        "stale": status.stale,
        "managed": status.managed,
        "workspace": workspace,
        "instance": instance_data,
    });
    render_value(output, "web.status", data, || {
        if status.running {
            if let Some(instance) = status.instance {
                println!("running");
                println!("url: http://{}/", instance.address);
                println!("pid: {}", instance.pid);
                println!("workspace: {}", instance.workspace.display());
                println!("log: {}", instance.log_path.display());
            } else if let Some(instance) = status.observed {
                println!("running");
                println!("url: http://{}/", instance.address);
                if let Some(pid) = instance.pid {
                    println!("pid: {pid}");
                }
                println!("workspace: {}", instance.workspace.display());
                println!("managed: no (stop it from its original command)");
            }
        } else if status.stale {
            println!("stale instance record for {}", workspace.display());
        } else {
            println!("not running for {}", workspace.display());
        }
    })
}

fn start_argv(args: WebStartArgs, context: &InvocationContext) -> anyhow::Result<Vec<String>> {
    let output = context.output_mode();
    let mut argv = Vec::new();
    if args.detach {
        argv.push("--detach".to_string());
    }
    if args.replace {
        argv.push("--replace".to_string());
    }
    if let Some(host) = args.host {
        argv.extend(["--host".to_string(), host]);
    }
    if let Some(port) = args.port {
        argv.extend(["--port".to_string(), port.to_string()]);
    }
    if let Some(workspace) = args.legacy_workspace {
        if output == OutputMode::Human {
            eprintln!("warning: `--workspace` is deprecated; use global `--directory`/`-C`");
        }
        let workspace = context.resolve_path(workspace);
        argv.extend([
            "--workspace".to_string(),
            path_to_string(&workspace, "workspace")?,
        ]);
    } else {
        argv.extend([
            "--workspace".to_string(),
            path_to_string(&context.directory, "workspace")?,
        ]);
    }
    if let Some(config) = context.explicit_config.as_deref() {
        argv.extend([
            "--config".to_string(),
            path_to_string(config, "config path")?,
        ]);
    }
    if let Some(web_dir) = args.web_dir {
        argv.extend([
            "--web-dir".to_string(),
            path_to_string(&web_dir, "Web asset directory")?,
        ]);
    }
    if args.api_only {
        argv.push("--api-only".to_string());
    }
    Ok(argv)
}

fn target_workspace(target: WebTargetArgs, context: &InvocationContext) -> anyhow::Result<PathBuf> {
    let output = context.output_mode();
    if let Some(workspace) = target.legacy_workspace {
        if output == OutputMode::Human {
            eprintln!("warning: `--workspace` is deprecated; use global `--directory`/`-C`");
        }
        Ok(context.resolve_path(workspace))
    } else {
        Ok(context.directory.clone())
    }
}

fn path_to_string(path: &Path, label: &str) -> anyhow::Result<String> {
    path.to_str()
        .map(str::to_string)
        .with_context(|| format!("{label} must be valid UTF-8"))
}

fn open_url(url: &str, context: &InvocationContext) -> anyhow::Result<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        command.arg(url);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = Command::new("xdg-open");
        command.arg(url);
        command
    };
    #[cfg(windows)]
    let mut command = {
        let mut command = Command::new("cmd");
        command.args(["/C", "start", "", url]);
        command
    };
    let status = command
        .current_dir(&context.directory)
        .status()
        .with_context(|| format!("could not open {url}"))?;
    if !status.success() {
        bail!("browser opener exited with {status}");
    }
    Ok(())
}

fn emit_log_lines(content: &str, start_sequence: u64) -> anyhow::Result<u64> {
    let mut sequence = start_sequence;
    for line in content.lines() {
        write_jsonl(&json!({
            "schemaVersion": 1,
            "command": "web.logs",
            "type": "log",
            "sequence": sequence,
            "line": line,
        }))
        .map_err(|error| log_stream_error(error, sequence))?;
        sequence += 1;
    }
    Ok(sequence)
}

fn emit_log_result(sequence: u64, path: &Path, followed: bool) -> anyhow::Result<()> {
    write_jsonl(&json!({
        "schemaVersion": 1,
        "command": "web.logs",
        "type": "result",
        "sequence": sequence,
        "ok": true,
        "data": {"path": path, "followed": followed},
    }))
    .map_err(|error| log_stream_error(error, sequence))
}

async fn follow_log(
    instance: &WebInstanceRecord,
    output: OutputMode,
    mut sequence: u64,
    cancellation: &tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut file = tokio::fs::File::open(&instance.log_path)
        .await
        .with_context(|| format!("could not open {}", instance.log_path.display()))
        .map_err(|error| follow_error(error, output, sequence))?;
    let mut offset = file
        .metadata()
        .await
        .map_err(anyhow::Error::from)
        .map_err(|error| follow_error(error, output, sequence))?
        .len();
    loop {
        tokio::select! {
            _ = cancellation.cancelled() => {
                return Err(CliError::new(
                    "operation.cancelled",
                    "Web log following cancelled",
                    ExitClass::Cancelled,
                )
                .with_jsonl_sequence(sequence)
                .into());
            },
            _ = tokio::time::sleep(Duration::from_millis(250)) => {}
        }
        let length = file
            .metadata()
            .await
            .map_err(anyhow::Error::from)
            .map_err(|error| follow_error(error, output, sequence))?
            .len();
        if length < offset {
            offset = 0;
        }
        if length == offset {
            continue;
        }
        file.seek(std::io::SeekFrom::Start(offset))
            .await
            .map_err(anyhow::Error::from)
            .map_err(|error| follow_error(error, output, sequence))?;
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes)
            .await
            .map_err(anyhow::Error::from)
            .map_err(|error| follow_error(error, output, sequence))?;
        offset = length;
        let content = String::from_utf8_lossy(&bytes);
        match output {
            OutputMode::Human => print!("{content}"),
            OutputMode::Jsonl => sequence = emit_log_lines(&content, sequence)?,
            OutputMode::Json => unreachable!("JSON follow is rejected before this function"),
        }
    }
}

fn follow_error(error: anyhow::Error, output: OutputMode, sequence: u64) -> anyhow::Error {
    if output == OutputMode::Jsonl {
        log_stream_error(error, sequence)
    } else {
        error
    }
}

fn log_stream_error(error: anyhow::Error, sequence: u64) -> anyhow::Error {
    if error.downcast_ref::<CliError>().is_some() {
        return error;
    }
    CliError::new(
        "web.logs.failed",
        format!("Web log stream failed: {error:#}"),
        ExitClass::Failure,
    )
    .with_jsonl_sequence(sequence)
    .into()
}
