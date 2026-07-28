//! WorkBuddy account provider backed by its bundled CodeBuddy CLI.
//!
//! Authentication remains owned by WorkBuddy. A3S neither reads nor copies
//! private tokens; it launches the local CLI with the existing WorkBuddy config
//! directory and consumes its documented `stream-json` output.

use super::cli_transport::{account_cli_system_prompt, complete_streaming, CliInvocation};
use crate::user_paths::user_home_dir;
use a3s_code_core::llm::{LlmClient, LlmResponse, Message, StreamEvent, ToolDefinition};
use anyhow::{anyhow, bail, Context, Result};
use async_trait::async_trait;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

#[cfg(windows)]
mod windows;

const DISCOVERY_MODEL: &str = "__a3s_account_model_discovery__";
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(30);

const FALLBACK_MODELS: &[&str] = &[
    "auto",
    "hy3",
    "glm-5.2",
    "glm-5.1",
    "glm-5v-turbo",
    "glm-5.0-turbo",
    "glm-5.0",
    "glm-4.7",
    "kimi-k2.7",
    "kimi-k2.6",
    "kimi-k2.5",
    "minimax-m3",
    "minimax-m2.7",
    "deepseek-v4-flash",
    "deepseek-v4-pro",
    "deepseek-v3-2-volc",
];

#[derive(Clone, Debug)]
struct CodeBuddyCli {
    program: PathBuf,
    prefix_args: Vec<OsString>,
    config_dir: Option<PathBuf>,
    electron_run_as_node: bool,
}

impl CodeBuddyCli {
    fn locate() -> Result<Self> {
        if let Some(override_path) = non_empty_env("A3S_CODEBUDDY_CLI") {
            let path = PathBuf::from(override_path);
            let program = if path.components().count() == 1 {
                find_on_path(path.as_os_str()).unwrap_or(path)
            } else {
                path
            };
            if !program.is_file() {
                bail!(
                    "A3S_CODEBUDDY_CLI does not point to a file: {}",
                    program.display()
                );
            }
            return Ok(Self {
                program,
                prefix_args: Vec::new(),
                config_dir: workbuddy_config_dir(),
                electron_run_as_node: false,
            });
        }

        #[cfg(target_os = "macos")]
        for app in workbuddy_macos_app_candidates() {
            let program = app.join("Contents/MacOS/Electron");
            let archive = app.join("Contents/Resources/app.asar");
            let script = app.join("Contents/Resources/app.asar/cli/bin/codebuddy");
            // `script` is an Electron ASAR virtual path, so the host filesystem
            // correctly reports it as "not a file" even though Electron can
            // execute it. Validate the containing archive instead.
            if program.is_file() && archive.is_file() {
                return Ok(Self {
                    program,
                    prefix_args: vec![script.into_os_string()],
                    config_dir: workbuddy_config_dir(),
                    electron_run_as_node: true,
                });
            }
        }

        #[cfg(windows)]
        for program in windows::workbuddy_executable_candidates() {
            let Some(app_dir) = program.parent() else {
                continue;
            };
            let archive = app_dir.join("resources/app.asar");
            let unpacked_script = app_dir.join("resources/app.asar.unpacked/cli/bin/codebuddy");
            let script = if unpacked_script.is_file() {
                unpacked_script
            } else {
                app_dir.join("resources/app.asar/cli/bin/codebuddy")
            };
            if program.is_file() && archive.is_file() {
                return Ok(Self {
                    program,
                    prefix_args: vec![script.into_os_string()],
                    config_dir: workbuddy_config_dir(),
                    electron_run_as_node: true,
                });
            }
        }

        for name in ["codebuddy", "cbc"] {
            if let Some(program) = find_on_path(OsStr::new(name)) {
                return Ok(Self {
                    program,
                    prefix_args: Vec::new(),
                    config_dir: workbuddy_config_dir(),
                    electron_run_as_node: false,
                });
            }
        }

        bail!("WorkBuddy/CodeBuddy CLI was not found; install WorkBuddy or set A3S_CODEBUDDY_CLI")
    }

    fn apply_environment(&self, command: &mut Command) {
        if self.electron_run_as_node {
            command.env("ELECTRON_RUN_AS_NODE", "1");
        }
        if let Some(config_dir) = &self.config_dir {
            command
                .env("WORKBUDDY_CONFIG_DIR", config_dir)
                .env("CODEBUDDY_CONFIG_DIR", config_dir);
        }
    }

    fn stream_invocation(&self, model: &str, appended_system: Option<&str>) -> CliInvocation {
        let mut args = self.prefix_args.clone();
        args.extend(
            [
                "--print",
                "--output-format",
                "stream-json",
                "--include-partial-messages",
                "--tools",
                "",
                "--max-turns",
                "1",
                "--model",
                model,
                "--setting-sources",
                "user",
            ]
            .into_iter()
            .map(OsString::from),
        );
        if let Some(system) = appended_system.filter(|value| !value.trim().is_empty()) {
            args.push("--append-system-prompt".into());
            args.push(system.into());
        }

        // Keep metadata deliberately concise. In particular, never record the
        // process argument vector or WorkBuddy's transient local auth values.
        let invocation = CliInvocation::new(
            self.program.clone(),
            args,
            "workbuddy-codebuddy-cli",
            model,
            format!("WorkBuddy CodeBuddy CLI · {model}"),
        );
        let invocation = if self.electron_run_as_node {
            invocation.with_env("ELECTRON_RUN_AS_NODE", "1")
        } else {
            invocation
        };
        match &self.config_dir {
            Some(config_dir) => invocation
                .with_env("WORKBUDDY_CONFIG_DIR", config_dir.as_os_str())
                .with_env("CODEBUDDY_CONFIG_DIR", config_dir.as_os_str()),
            None => invocation,
        }
    }
}

pub(crate) struct CodeBuddyClient {
    model: String,
    cli: CodeBuddyCli,
}

impl CodeBuddyClient {
    pub(crate) fn from_workbuddy_login(model: &str) -> Result<Self> {
        let model = model.trim();
        if model.is_empty() || model.starts_with('(') {
            bail!("WorkBuddy model id is empty or unavailable");
        }
        let cli = CodeBuddyCli::locate()?;
        let config_dir = cli.config_dir.as_ref().ok_or_else(|| {
            anyhow!("WorkBuddy account state was not found; open WorkBuddy and sign in")
        })?;
        if !has_account_state(config_dir) {
            bail!("WorkBuddy account state was not found; open WorkBuddy and sign in");
        }
        Ok(Self {
            model: model.to_string(),
            cli,
        })
    }
}

#[async_trait]
impl LlmClient for CodeBuddyClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let mut rx = self
            .complete_streaming(messages, system, tools, CancellationToken::new())
            .await?;
        while let Some(event) = rx.recv().await {
            if let StreamEvent::Done(response) = event {
                return Ok(response);
            }
        }
        Err(anyhow!("WorkBuddy account stream closed before completion"))
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let appended_system = account_cli_system_prompt(system, tools, "WorkBuddy");
        let invocation = self
            .cli
            .stream_invocation(&self.model, appended_system.as_deref());
        complete_streaming(invocation, messages, tools, cancel_token).await
    }
}

pub(crate) fn fallback_models() -> Vec<String> {
    FALLBACK_MODELS
        .iter()
        .map(|model| (*model).to_string())
        .collect()
}

pub(crate) fn has_workbuddy_login() -> bool {
    CodeBuddyCli::locate()
        .ok()
        .and_then(|cli| cli.config_dir)
        .is_some_and(|dir| has_account_state(&dir))
}

/// Ask the local CLI to validate an impossible model id. CodeBuddy returns the
/// current account entitlement list in that validation response. This keeps
/// account policy in WorkBuddy and avoids parsing or copying private tokens.
pub(crate) async fn discover_models() -> Result<Vec<String>> {
    let cli = CodeBuddyCli::locate()?;
    let config_dir = cli.config_dir.as_ref().ok_or_else(|| {
        anyhow!("WorkBuddy account state was not found; open WorkBuddy and sign in")
    })?;
    if !has_account_state(config_dir) {
        bail!("WorkBuddy account state was not found; open WorkBuddy and sign in");
    }

    let mut args = cli.prefix_args.clone();
    args.extend(
        [
            "--print",
            "--output-format",
            "json",
            "--tools",
            "",
            "--max-turns",
            "1",
            "--model",
            DISCOVERY_MODEL,
            "--setting-sources",
            "user",
            "List account models.",
        ]
        .into_iter()
        .map(OsString::from),
    );
    let mut command = Command::new(&cli.program);
    command
        .args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    cli.apply_environment(&mut command);

    let output = tokio::time::timeout(DISCOVERY_TIMEOUT, command.output())
        .await
        .map_err(|_| anyhow!("WorkBuddy model discovery timed out"))?
        .context("start WorkBuddy model discovery")?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let mut models = parse_supported_models(&stdout);
    if models.is_empty() {
        models = parse_supported_models(&stderr);
    }
    if !models.is_empty() {
        return Ok(models);
    }

    let combined_lower = format!("{stdout}\n{stderr}").to_ascii_lowercase();
    if combined_lower.contains("login")
        || combined_lower.contains("unauthorized")
        || combined_lower.contains("authentication")
    {
        bail!("WorkBuddy account is not signed in; open WorkBuddy and sign in");
    }
    bail!("WorkBuddy CLI did not return an account model list")
}

fn parse_supported_models(output: &str) -> Vec<String> {
    let mut collecting = false;
    let mut seen = HashSet::new();
    let mut models = Vec::new();
    for line in output.lines() {
        let line = line.trim();
        if line.contains("Currently supported models for your account")
            || line.starts_with("Currently supported:")
        {
            collecting = true;
            continue;
        }
        if !collecting {
            continue;
        }
        let Some(model) = line.strip_prefix('-').map(str::trim) else {
            if !models.is_empty() {
                break;
            }
            continue;
        };
        if is_model_id(model) && seen.insert(model.to_string()) {
            models.push(model.to_string());
        }
    }
    models
}

fn is_model_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | '/' | ':'))
}

pub(crate) fn workbuddy_config_dir() -> Option<PathBuf> {
    non_empty_env("WORKBUDDY_CONFIG_DIR")
        .or_else(|| non_empty_env("CODEBUDDY_CONFIG_DIR"))
        .map(PathBuf::from)
        .or_else(|| user_home_dir().map(|home| home.join(".workbuddy")))
}

fn has_account_state(config_dir: &Path) -> bool {
    config_dir.is_dir()
        && [
            config_dir.join("workbuddy.db"),
            config_dir.join("local_storage"),
            config_dir.join("app/session"),
            config_dir.join("settings.json"),
        ]
        .iter()
        .any(|path| path.exists())
}

fn non_empty_env(name: &str) -> Option<OsString> {
    std::env::var_os(name).filter(|value| !value.is_empty())
}

#[cfg(target_os = "macos")]
fn workbuddy_macos_app_candidates() -> Vec<PathBuf> {
    let mut candidates = vec![PathBuf::from("/Applications/WorkBuddy.app")];
    if let Some(home) = user_home_dir() {
        candidates.push(home.join("Applications/WorkBuddy.app"));
    }
    candidates
}

fn find_on_path(name: &OsStr) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(name);
        if candidate.is_file() {
            return Some(candidate);
        }
        #[cfg(windows)]
        for extension in ["exe", "cmd", "bat"] {
            let candidate = dir.join(format!("{}.{}", name.to_string_lossy(), extension));
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_account_model_validation_response_in_order() {
        let output = r#"
400 model unavailable
Currently supported models for your account:
  - auto
  - glm-5.1
  - kimi-k2.7
  - glm-5.1
Please use --model <model_id> to specify a valid model.
"#;

        assert_eq!(
            parse_supported_models(output),
            vec!["auto", "glm-5.1", "kimi-k2.7"]
        );
    }

    #[test]
    fn rejects_non_model_lines_from_discovery_output() {
        let output = "Currently supported:\n - good-model\n - bad model\n - <secret>";
        assert_eq!(parse_supported_models(output), vec!["good-model"]);
    }

    #[test]
    fn stream_metadata_never_contains_system_prompt_or_argument_vector() {
        let cli = CodeBuddyCli {
            program: PathBuf::from("codebuddy"),
            prefix_args: Vec::new(),
            config_dir: Some(PathBuf::from("/tmp/workbuddy")),
            electron_run_as_node: false,
        };
        let invocation = cli.stream_invocation("glm-5.1", Some("private system prompt"));

        assert_eq!(
            invocation.request_label(),
            "WorkBuddy CodeBuddy CLI · glm-5.1"
        );
        assert!(!invocation.request_label().contains("private"));
        assert!(!invocation
            .request_label()
            .contains("--append-system-prompt"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn fake_codebuddy_cli_streams_through_shared_transport() {
        use std::os::unix::fs::PermissionsExt;

        let dir = std::env::temp_dir().join(format!(
            "a3s-codebuddy-fake-{}-{}",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("codebuddy");
        std::fs::write(
            &script,
            r#"#!/bin/sh
printf '%s\n' '{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","type":"message","model":"glm-5.1","usage":{"input_tokens":0,"output_tokens":0}}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_start","index":0,"content_block":{"type":"text","text":""}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"FAKE_OK"}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"input_tokens":3,"output_tokens":2}}}'
printf '%s\n' '{"type":"stream_event","event":{"type":"message_stop"}}'
"#,
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&script).unwrap().permissions();
        permissions.set_mode(0o700);
        std::fs::set_permissions(&script, permissions).unwrap();

        let client = CodeBuddyClient {
            model: "glm-5.1".into(),
            cli: CodeBuddyCli {
                program: script,
                prefix_args: Vec::new(),
                config_dir: None,
                electron_run_as_node: false,
            },
        };
        let response = client
            .complete(&[Message::user("hello")], None, &[])
            .await
            .unwrap();

        assert_eq!(response.text(), "FAKE_OK");
        assert_eq!(response.usage.prompt_tokens, 3);
        let _ = std::fs::remove_dir_all(dir);
    }

    /// Opt-in real-account coverage. CI remains credential-free; maintainers
    /// run this with `A3S_TEST_WORKBUDDY_REAL=1` on a signed-in workstation.
    #[tokio::test]
    async fn real_workbuddy_account_completes_an_a3s_tool_round() {
        if std::env::var("A3S_TEST_WORKBUDDY_REAL").as_deref() != Ok("1") {
            return;
        }
        let model =
            std::env::var("A3S_TEST_WORKBUDDY_MODEL").unwrap_or_else(|_| "glm-5.1".to_string());
        let client = CodeBuddyClient::from_workbuddy_login(&model).unwrap();
        let tools = vec![ToolDefinition {
            name: "read".into(),
            description: "Read a file from the A3S workspace".into(),
            parameters: serde_json::json!({
                "type":"object",
                "properties":{"file_path":{"type":"string"}},
                "required":["file_path"]
            }),
        }];
        let user = Message::user(
            "First call the A3S read tool for README.md. After its result, reply with exactly WORKBUDDY_TOOL_OK.",
        );
        let mut first_stream = client
            .complete_streaming(
                std::slice::from_ref(&user),
                Some("Follow the A3S host-tool protocol."),
                &tools,
                CancellationToken::new(),
            )
            .await
            .unwrap();
        let mut streamed_protocol_text = String::new();
        let mut first = None;
        while let Some(event) = first_stream.recv().await {
            match event {
                StreamEvent::TextDelta(text) => streamed_protocol_text.push_str(&text),
                StreamEvent::Done(response) => {
                    first = Some(response);
                    break;
                }
                _ => {}
            }
        }
        assert!(
            streamed_protocol_text.trim().is_empty(),
            "host-tool protocol leaked into visible text: {streamed_protocol_text:?}"
        );
        let first = first.expect("WorkBuddy stream should complete");
        let calls = first.tool_calls();
        assert_eq!(calls.len(), 1, "first response: {}", first.text());
        assert!(first.usage.prompt_tokens > 0);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].args["file_path"], "README.md");

        let result = Message::tool_result(&calls[0].id, "A3S README fixture", false);
        let second = client
            .complete(
                &[user, first.message, result],
                Some("Follow the A3S host-tool protocol."),
                &tools,
            )
            .await
            .unwrap();
        assert_eq!(second.text().trim(), "WORKBUDDY_TOOL_OK");
    }
}
