use super::model::canonical_model_name;
use crate::account_providers::cli_transport::{
    account_cli_system_prompt, complete_streaming, CliInvocation,
};
use a3s_code_core::llm::{Message, StreamEvent, ToolDefinition};
use anyhow::Result;
use std::ffi::OsString;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

pub(crate) struct ClaudeCodeCliAdapter {
    model: String,
}

impl ClaudeCodeCliAdapter {
    pub(crate) fn new(model: &str) -> Self {
        Self {
            model: canonical_model_name(model),
        }
    }

    pub(crate) async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        let appended_system = account_cli_system_prompt(system, tools, "Claude Code");
        let args = claude_cli_args(&self.model, appended_system.as_deref());
        let request_label = claude_cli_request_label(&args);
        let invocation = CliInvocation::new(
            "claude",
            args.into_iter().map(OsString::from).collect(),
            "claude-code-cli",
            self.model.clone(),
            request_label,
        );
        complete_streaming(invocation, messages, tools, cancel_token).await
    }
}

fn claude_cli_args(model: &str, appended_system: Option<&str>) -> Vec<String> {
    let mut args = vec![
        "-p".into(),
        "--safe-mode".into(),
        "--model".into(),
        canonical_model_name(model),
        "--output-format".into(),
        "stream-json".into(),
        "--verbose".into(),
        "--include-partial-messages".into(),
        "--tools".into(),
        String::new(),
        "--no-session-persistence".into(),
    ];
    if let Some(appended_system) =
        appended_system.filter(|appended_system| !appended_system.trim().is_empty())
    {
        args.push("--append-system-prompt".into());
        args.push(appended_system.to_string());
    }
    args
}

fn claude_cli_request_label(args: &[String]) -> String {
    let mut redacted = Vec::with_capacity(args.len());
    let mut redact_next = false;
    for arg in args {
        if redact_next {
            redacted.push("[system prompt redacted]".to_string());
            redact_next = false;
            continue;
        }
        redacted.push(arg.clone());
        if arg == "--append-system-prompt" || arg == "--system-prompt" {
            redact_next = true;
        }
    }
    format!("claude {}", redacted.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::account_providers::cli_transport::account_cli_system_prompt;
    use serde_json::json;

    #[test]
    fn args_use_safe_text_only_streaming_mode() {
        let args = claude_cli_args(" claude-opus-4-8[1m] ", Some("secret system prompt"));

        assert!(args.contains(&"--safe-mode".to_string()));
        assert!(args.contains(&"--no-session-persistence".to_string()));
        assert!(args.contains(&"--include-partial-messages".to_string()));
        assert_eq!(
            args.windows(2)
                .find(|window| window[0] == "--model")
                .map(|window| window[1].as_str()),
            Some("claude-opus-4-8")
        );
        assert_eq!(
            args.windows(2)
                .find(|window| window[0] == "--tools")
                .map(|window| window[1].as_str()),
            Some("")
        );
        assert_eq!(
            claude_cli_request_label(&args),
            "claude -p --safe-mode --model claude-opus-4-8 --output-format stream-json --verbose --include-partial-messages --tools  --no-session-persistence --append-system-prompt [system prompt redacted]"
        );
    }

    #[test]
    fn shared_system_prompt_injects_host_tool_protocol() {
        let prompt = account_cli_system_prompt(
            Some("Be concise."),
            &[ToolDefinition {
                name: "read_file".into(),
                description: "Read a file".into(),
                parameters: json!({"type":"object"}),
            }],
            "Claude Code",
        )
        .unwrap();

        assert!(prompt.contains("# A3S System"));
        assert!(prompt.contains("Claude Code's own built-in tools"));
        assert!(prompt.contains("<function_calls>"));
    }
}
