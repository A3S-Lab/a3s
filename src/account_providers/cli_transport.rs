//! Shared text-only CLI transport for account-backed providers.
//!
//! Claude Code and WorkBuddy expose the same Anthropic-shaped `stream-json`
//! event protocol. Their own tools are disabled and A3S host tools are bridged
//! through a small text envelope, keeping execution and tool-card rendering in
//! A3S rather than in the external CLI process.

use super::host_tools::{host_tool_instructions, parse_host_tool_calls, HostToolParseResult};
use super::protocol::{
    parse_account_cli_stream_event, AnthropicEventMapper, AnthropicStreamEvent, StreamMeta,
};
use a3s_code_core::llm::{
    ContentBlock, LlmResponse, LlmResponseMeta, Message, StreamEvent, TokenUsage, ToolDefinition,
};
use anyhow::{Context, Result};
use serde_json::json;
use std::ffi::OsString;
use std::fmt::Write as _;
use std::path::PathBuf;
use std::process::Stdio;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufRead, AsyncBufReadExt, AsyncRead, AsyncReadExt, AsyncWriteExt, BufReader};
use tokio::process::{Child, Command};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

const PROCESS_SETTLEMENT_TIMEOUT: Duration = Duration::from_secs(1);
const MAX_ACCOUNT_CLI_LINE_BYTES: usize = 8 * 1024 * 1024;
const MAX_ACCOUNT_CLI_STREAM_BYTES: usize = 32 * 1024 * 1024;
const MAX_ACCOUNT_CLI_STDERR_BYTES: usize = 64 * 1024;

#[derive(Debug, PartialEq, Eq)]
enum BoundedLine {
    Eof,
    Line(String),
    TooLong,
}

async fn read_bounded_line<R>(
    reader: &mut R,
    line: &mut Vec<u8>,
    max_bytes: usize,
) -> std::io::Result<BoundedLine>
where
    R: AsyncBufRead + Unpin,
{
    line.clear();
    let mut read_any = false;
    let mut too_long = false;

    loop {
        let available = reader.fill_buf().await?;
        if available.is_empty() {
            if !read_any {
                return Ok(BoundedLine::Eof);
            }
            break;
        }

        let newline = available.iter().position(|byte| *byte == b'\n');
        let content_len = newline.unwrap_or(available.len());
        read_any = true;
        if !too_long {
            let remaining = max_bytes.saturating_sub(line.len());
            if content_len <= remaining {
                line.extend_from_slice(&available[..content_len]);
            } else {
                too_long = true;
            }
        }
        let consumed = content_len + usize::from(newline.is_some());
        reader.consume(consumed);
        if newline.is_some() {
            break;
        }
    }

    if too_long {
        return Ok(BoundedLine::TooLong);
    }
    if line.last() == Some(&b'\r') {
        line.pop();
    }
    String::from_utf8(std::mem::take(line))
        .map(BoundedLine::Line)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))
}

async fn read_bounded_tail<R>(mut reader: R, max_bytes: usize) -> String
where
    R: AsyncRead + Unpin,
{
    let mut retained = Vec::with_capacity(max_bytes.min(4096));
    let mut chunk = [0_u8; 4096];
    loop {
        let count = match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(count) => count,
        };
        if max_bytes == 0 {
            continue;
        }
        if count >= max_bytes {
            retained.clear();
            retained.extend_from_slice(&chunk[count - max_bytes..count]);
            continue;
        }
        let excess = retained
            .len()
            .saturating_add(count)
            .saturating_sub(max_bytes);
        if excess > 0 {
            retained.drain(..excess);
        }
        retained.extend_from_slice(&chunk[..count]);
    }
    String::from_utf8_lossy(&retained).into_owned()
}

fn configure_account_cli_process_group(command: &mut Command) {
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        command.as_std_mut().process_group(0);
    }
    #[cfg(not(unix))]
    let _ = command;
}

struct AccountCliProcessGroup {
    #[cfg(unix)]
    process_group: Option<libc::pid_t>,
}

impl AccountCliProcessGroup {
    fn attach(child: &Child) -> Self {
        Self {
            #[cfg(unix)]
            process_group: child.id().and_then(|pid| libc::pid_t::try_from(pid).ok()),
        }
    }

    fn terminate(&mut self) {
        #[cfg(unix)]
        if let Some(process_group) = self.process_group.take() {
            // SAFETY: the account CLI was spawned as the leader of a dedicated
            // process group, so the negative pid covers all descendants.
            unsafe {
                libc::kill(-process_group, libc::SIGKILL);
            }
        }
    }
}

impl Drop for AccountCliProcessGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

async fn terminate_account_cli(child: &mut Child, process_group: &mut AccountCliProcessGroup) {
    process_group.terminate();
    let _ = child.start_kill();
    let _ = tokio::time::timeout(PROCESS_SETTLEMENT_TIMEOUT, child.wait()).await;
}

async fn settle_stderr(mut task: JoinHandle<String>) -> String {
    match tokio::time::timeout(PROCESS_SETTLEMENT_TIMEOUT, &mut task).await {
        Ok(Ok(stderr)) => stderr,
        Ok(Err(_)) => String::new(),
        Err(_) => {
            task.abort();
            String::new()
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct CliInvocation {
    program: PathBuf,
    args: Vec<OsString>,
    env: Vec<(OsString, OsString)>,
    provider: &'static str,
    model: String,
    request_label: String,
}

impl CliInvocation {
    pub(crate) fn new(
        program: impl Into<PathBuf>,
        args: Vec<OsString>,
        provider: &'static str,
        model: impl Into<String>,
        request_label: impl Into<String>,
    ) -> Self {
        Self {
            program: program.into(),
            args,
            env: Vec::new(),
            provider,
            model: model.into(),
            request_label: request_label.into(),
        }
    }

    pub(crate) fn with_env(mut self, key: impl Into<OsString>, value: impl Into<OsString>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    #[cfg(test)]
    pub(crate) fn request_label(&self) -> &str {
        &self.request_label
    }
}

pub(crate) fn account_cli_system_prompt(
    system: Option<&str>,
    tools: &[ToolDefinition],
    transport_name: &str,
) -> Option<String> {
    let mut prompt = String::new();
    if let Some(system) = system.filter(|value| !value.trim().is_empty()) {
        prompt.push_str("# A3S System\n\n");
        prompt.push_str(system.trim());
        prompt.push_str("\n\n");
    }
    if let Some(instructions) = host_tool_instructions(transport_name, tools) {
        prompt.push_str(&instructions);
    }
    (!prompt.trim().is_empty()).then_some(prompt)
}

pub(crate) fn account_cli_prompt(messages: &[Message]) -> String {
    let mut prompt = String::from("# Conversation\n");
    for message in messages {
        prompt.push('\n');
        prompt.push_str(match message.role.as_str() {
            "assistant" => "Assistant",
            "user" => "User",
            "system" => "System",
            role => role,
        });
        prompt.push_str(":\n");
        for block in &message.content {
            match block {
                ContentBlock::Text { text } => {
                    prompt.push_str(text);
                    prompt.push('\n');
                }
                ContentBlock::Image { source } => {
                    let _ = writeln!(prompt, "[image omitted: {}]", source.media_type);
                }
                ContentBlock::ToolUse { id, name, input } => {
                    let block = json!({"id": id, "name": name, "input": input});
                    prompt.push_str(
                        "### A3S host tool call record\n\n\
                         This call has already been requested. Do not repeat it unless the user explicitly asks.\n\n\
                         ```json\n",
                    );
                    let _ = writeln!(prompt, "{block}");
                    prompt.push_str("```\n");
                }
                ContentBlock::ToolResult {
                    tool_use_id,
                    content,
                    is_error,
                } => {
                    let status = if is_error.unwrap_or(false) {
                        "error"
                    } else {
                        "ok"
                    };
                    let block = json!({
                        "tool_use_id": tool_use_id,
                        "status": status,
                        "content": content.as_text(),
                    });
                    prompt.push_str("### A3S host tool result record\n\n```json\n");
                    let _ = writeln!(prompt, "{block}");
                    prompt.push_str("```\n");
                }
            }
        }
    }
    prompt
}

pub(crate) async fn complete_streaming(
    invocation: CliInvocation,
    messages: &[Message],
    tools: &[ToolDefinition],
    cancel_token: CancellationToken,
) -> Result<mpsc::Receiver<StreamEvent>> {
    let request_started_at = Instant::now();
    let prompt = account_cli_prompt(messages);
    let mut command = Command::new(&invocation.program);
    command
        .args(&invocation.args)
        .envs(invocation.env.iter().cloned())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_account_cli_process_group(&mut command);
    let mut child = command
        .spawn()
        .with_context(|| format!("start {} account CLI", invocation.provider))?;
    let mut process_group = AccountCliProcessGroup::attach(&child);

    let mut stdin = child
        .stdin
        .take()
        .with_context(|| format!("open {} account CLI stdin", invocation.provider))?;
    tokio::spawn(async move {
        let _ = stdin.write_all(prompt.as_bytes()).await;
    });

    let stdout = child
        .stdout
        .take()
        .with_context(|| format!("capture {} account CLI stdout", invocation.provider))?;
    let stderr = child
        .stderr
        .take()
        .with_context(|| format!("capture {} account CLI stderr", invocation.provider))?;

    let (tx, rx) = mpsc::channel(100);
    let host_tools = tools.to_vec();
    tokio::spawn(async move {
        let stderr_task = tokio::spawn(read_bounded_tail(
            BufReader::new(stderr),
            MAX_ACCOUNT_CLI_STDERR_BYTES,
        ));

        let meta = StreamMeta {
            provider: invocation.provider,
            request_model: invocation.model.clone(),
            request_url: invocation.request_label,
            started_at: request_started_at,
        };
        let mut stdout = BufReader::new(stdout);
        let mut line_buffer = Vec::new();
        let mut stream_bytes = 0usize;
        let mut native_mapper = host_tools
            .is_empty()
            .then(|| AnthropicEventMapper::new(meta.clone()));
        let mut host_tool_mapper =
            (!host_tools.is_empty()).then(|| AccountCliHostToolMapper::new(meta, host_tools));
        let mut failure = None;
        let mut completed = false;
        let mut terminate_after_stream = false;

        loop {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    terminate_account_cli(&mut child, &mut process_group).await;
                    let _ = settle_stderr(stderr_task).await;
                    return;
                }
                line = read_bounded_line(
                    &mut stdout,
                    &mut line_buffer,
                    MAX_ACCOUNT_CLI_LINE_BYTES,
                ) => {
                    let line = match line {
                        Ok(BoundedLine::Line(line)) => line,
                        Ok(BoundedLine::Eof) => break,
                        Ok(BoundedLine::TooLong) => {
                            failure = Some(CliFailure::OutputLimit);
                            terminate_after_stream = true;
                            break;
                        }
                        Err(_) => {
                            failure = Some(CliFailure::StreamRead);
                            terminate_after_stream = true;
                            break;
                        }
                    };
                    stream_bytes = stream_bytes.saturating_add(line.len());
                    if stream_bytes > MAX_ACCOUNT_CLI_STREAM_BYTES {
                        failure = Some(CliFailure::OutputLimit);
                        terminate_after_stream = true;
                        break;
                    }
                    let Some(event) = parse_account_cli_stream_event(&line) else {
                        failure = failure.or_else(|| classify_unstructured_output(&line));
                        continue;
                    };
                    if matches!(event, AnthropicStreamEvent::Error) {
                        failure = Some(CliFailure::Provider);
                        terminate_after_stream = true;
                        break;
                    }
                    let done = if completed {
                        false
                    } else if let Some(mapper) = native_mapper.as_mut() {
                        mapper.handle(event, &tx).await
                    } else if let Some(mapper) = host_tool_mapper.as_mut() {
                        mapper.handle(event, &tx).await
                    } else {
                        false
                    };
                    if done {
                        // WorkBuddy emits assistant/result records after
                        // `message_stop`, sometimes duplicating a large final
                        // answer. Keep draining stdout so the child cannot
                        // deadlock on a full pipe while preserving exactly one
                        // A3S `Done` event.
                        completed = true;
                    }
                }
            }
        }

        let status = if terminate_after_stream {
            terminate_account_cli(&mut child, &mut process_group).await;
            None
        } else {
            tokio::select! {
                _ = cancel_token.cancelled() => {
                    terminate_account_cli(&mut child, &mut process_group).await;
                    let _ = settle_stderr(stderr_task).await;
                    return;
                }
                status = child.wait() => status.ok(),
            }
        };
        // Account requests own no service process after their stream ends.
        process_group.terminate();
        let stderr = settle_stderr(stderr_task).await;
        if completed {
            return;
        }
        let failure = failure
            .or_else(|| classify_unstructured_output(&stderr))
            .unwrap_or(CliFailure::Incomplete);
        let detail = failure_message(
            invocation.provider,
            &invocation.model,
            failure,
            status.as_ref().and_then(std::process::ExitStatus::code),
        );
        let _ = tx.send(StreamEvent::TextDelta(detail)).await;
    });

    Ok(rx)
}

#[derive(Clone, Copy)]
enum CliFailure {
    Auth,
    Model,
    Provider,
    StreamRead,
    OutputLimit,
    Incomplete,
}

fn classify_unstructured_output(text: &str) -> Option<CliFailure> {
    let lower = text.to_ascii_lowercase();
    if lower.contains("currently supported models") || lower.contains("service info not found") {
        Some(CliFailure::Model)
    } else if lower.contains("not logged in")
        || lower.contains("please login")
        || lower.contains("please log in")
        || lower.contains("authentication required")
        || lower.contains("unauthorized")
    {
        Some(CliFailure::Auth)
    } else {
        None
    }
}

fn failure_message(
    provider: &str,
    model: &str,
    failure: CliFailure,
    status: Option<i32>,
) -> String {
    match failure {
        CliFailure::Auth => format!(
            "{provider} account is not signed in; sign in with the local application and retry"
        ),
        CliFailure::Model => format!(
            "{provider} no longer offers model `{model}`; reopen /model to refresh the account list"
        ),
        CliFailure::Provider => format!("{provider} returned a provider error for `{model}`"),
        CliFailure::StreamRead => format!("{provider} account stream could not be read"),
        CliFailure::OutputLimit => {
            format!("{provider} account stream exceeded the bounded transport output limit")
        }
        CliFailure::Incomplete => match status {
            Some(code) => format!("{provider} account CLI exited with status {code}"),
            None => format!("{provider} account stream ended before completion"),
        },
    }
}

struct AccountCliHostToolMapper {
    meta: StreamMeta,
    tools: Vec<ToolDefinition>,
    text: String,
    usage: TokenUsage,
    stop_reason: Option<String>,
    response_id: Option<String>,
    response_model: Option<String>,
    response_object: Option<String>,
    first_token_ms: Option<u64>,
    visible_pending: String,
    protocol_started: bool,
}

impl AccountCliHostToolMapper {
    fn new(meta: StreamMeta, tools: Vec<ToolDefinition>) -> Self {
        Self {
            meta,
            tools,
            text: String::new(),
            usage: TokenUsage::default(),
            stop_reason: None,
            response_id: None,
            response_model: None,
            response_object: Some("message".into()),
            first_token_ms: None,
            visible_pending: String::new(),
            protocol_started: false,
        }
    }

    async fn handle(
        &mut self,
        event: AnthropicStreamEvent,
        tx: &mpsc::Sender<StreamEvent>,
    ) -> bool {
        match event {
            AnthropicStreamEvent::MessageStart { message } => {
                self.response_id = message.id;
                self.response_model = message.model;
                self.response_object = message.message_type;
                self.usage.prompt_tokens = message.usage.input_tokens;
                self.usage.cache_read_tokens = message.usage.cache_read_input_tokens;
                self.usage.cache_write_tokens = message.usage.cache_creation_input_tokens;
            }
            AnthropicStreamEvent::ContentBlockDelta {
                delta: super::protocol::AnthropicDelta::TextDelta { text },
                ..
            } => {
                self.mark_first_token();
                self.text.push_str(&text);
                self.stream_visible_text(&text, tx).await;
            }
            AnthropicStreamEvent::ContentBlockDelta { .. } => {}
            AnthropicStreamEvent::MessageDelta { delta, usage } => {
                self.stop_reason = Some(delta.stop_reason);
                if let Some(input_tokens) = usage.input_tokens.filter(|tokens| *tokens > 0) {
                    self.usage.prompt_tokens = input_tokens;
                }
                if let Some(cache_read_tokens) =
                    usage.cache_read_input_tokens.filter(|tokens| *tokens > 0)
                {
                    self.usage.cache_read_tokens = Some(cache_read_tokens);
                }
                if let Some(cache_write_tokens) = usage
                    .cache_creation_input_tokens
                    .filter(|tokens| *tokens > 0)
                {
                    self.usage.cache_write_tokens = Some(cache_write_tokens);
                }
                self.usage.completion_tokens = usage.output_tokens;
                self.usage.total_tokens = self.usage.prompt_tokens + self.usage.completion_tokens;
            }
            AnthropicStreamEvent::MessageStop => {
                self.finish(tx).await;
                return true;
            }
            AnthropicStreamEvent::Error => return false,
            _ => {}
        }
        false
    }

    async fn finish(&mut self, tx: &mpsc::Sender<StreamEvent>) {
        let mut content = Vec::new();
        let mut stop_reason = self.stop_reason.clone();

        match parse_host_tool_calls(&self.text, &self.tools) {
            HostToolParseResult::Calls(calls) => {
                stop_reason = Some("tool_use".into());
                for call in calls {
                    let input_delta = call.input.to_string();
                    let _ = tx
                        .send(StreamEvent::ToolUseStart {
                            id: call.id.clone(),
                            name: call.name.clone(),
                        })
                        .await;
                    let _ = tx
                        .send(StreamEvent::ToolUseInputDelta {
                            id: None,
                            delta: input_delta,
                        })
                        .await;
                    content.push(call.into_content_block());
                }
            }
            HostToolParseResult::Invalid(reason) => {
                stop_reason = Some("host_tool_protocol_error".into());
                content.push(ContentBlock::Text {
                    text: format!(
                        "The account model returned an invalid host-tool request ({reason}). Retry the turn."
                    ),
                });
            }
            HostToolParseResult::NoCall if !self.text.is_empty() => {
                let text = std::mem::take(&mut self.text);
                if !self.visible_pending.is_empty() {
                    let pending = std::mem::take(&mut self.visible_pending);
                    let _ = tx.send(StreamEvent::TextDelta(pending)).await;
                }
                content.push(ContentBlock::Text { text });
            }
            HostToolParseResult::NoCall => {}
        }

        let response = LlmResponse {
            message: Message {
                role: "assistant".into(),
                content,
                reasoning_content: None,
            },
            usage: self.usage.clone(),
            stop_reason,
            token_logprobs: Vec::new(),
            meta: Some(LlmResponseMeta {
                provider: Some(self.meta.provider.into()),
                request_model: Some(self.meta.request_model.clone()),
                request_url: Some(self.meta.request_url.clone()),
                response_id: self.response_id.clone(),
                response_model: self.response_model.clone(),
                response_object: self.response_object.clone(),
                first_token_ms: self.first_token_ms,
                duration_ms: Some(self.meta.started_at.elapsed().as_millis() as u64),
            }),
        };
        let _ = tx.send(StreamEvent::Done(response)).await;
    }

    fn mark_first_token(&mut self) {
        if self.first_token_ms.is_none() {
            self.first_token_ms = Some(self.meta.started_at.elapsed().as_millis() as u64);
        }
    }

    async fn stream_visible_text(&mut self, delta: &str, tx: &mpsc::Sender<StreamEvent>) {
        if self.protocol_started {
            return;
        }
        self.visible_pending.push_str(delta);

        if let Some(start) = tool_protocol_start(&self.visible_pending) {
            let visible = self.visible_pending[..start].to_string();
            self.visible_pending.clear();
            self.protocol_started = true;
            if !visible.is_empty() {
                let _ = tx.send(StreamEvent::TextDelta(visible)).await;
            }
            return;
        }

        let held = partial_tool_protocol_suffix_len(&self.visible_pending);
        let visible_end = self.visible_pending.len().saturating_sub(held);
        if visible_end > 0 {
            let visible = self.visible_pending[..visible_end].to_string();
            self.visible_pending.drain(..visible_end);
            let _ = tx.send(StreamEvent::TextDelta(visible)).await;
        }
    }
}

const TOOL_PROTOCOL_PREFIXES: &[&str] = &[
    "<function_calls>",
    "<invoke ",
    "<A3S_TOOL_CALLS>",
    "<tool_calls:",
    "<tool_call:",
    "<A3S_ASSISTANT_TOOL_CALL>",
    "<A3S_TOOL_RESULT>",
];

fn tool_protocol_start(text: &str) -> Option<usize> {
    TOOL_PROTOCOL_PREFIXES
        .iter()
        .filter_map(|prefix| text.find(prefix))
        .min()
}

fn partial_tool_protocol_suffix_len(text: &str) -> usize {
    TOOL_PROTOCOL_PREFIXES
        .iter()
        .map(|prefix| {
            (1..prefix.len())
                .rev()
                .find(|length| text.ends_with(&prefix[..*length]))
                .unwrap_or(0)
        })
        .max()
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const FIXTURE_PROCESS_TIMEOUT: Duration = Duration::from_secs(10);

    #[tokio::test]
    async fn bounded_line_reader_drains_an_oversized_record_before_the_next_line() {
        let input = b"0123456789\nok\r\n";
        let mut reader = BufReader::new(&input[..]);
        let mut line = Vec::new();

        assert_eq!(
            read_bounded_line(&mut reader, &mut line, 8).await.unwrap(),
            BoundedLine::TooLong
        );
        assert_eq!(
            read_bounded_line(&mut reader, &mut line, 8).await.unwrap(),
            BoundedLine::Line("ok".to_string())
        );
        assert_eq!(
            read_bounded_line(&mut reader, &mut line, 8).await.unwrap(),
            BoundedLine::Eof
        );
    }

    #[tokio::test]
    async fn stderr_reader_retains_only_the_bounded_tail() {
        let stderr = read_bounded_tail(BufReader::new(&b"0123456789"[..]), 6).await;
        assert_eq!(stderr, "456789");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn cancellation_after_stdout_eof_kills_account_cli_descendants() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("account-cli");
        let started = directory.path().join("started");
        let descendant_started = directory.path().join("descendant-started");
        let leaked = directory.path().join("cancellation-leak");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\n: > '{}'\n(: > '{}'; sleep 0.60; : > '{}') &\n\
                 while [ ! -e '{}' ]; do :; done\n\
                 exec 1>&- 2>&-\nwait\n",
                started.display(),
                descendant_started.display(),
                leaked.display(),
                descendant_started.display(),
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        let invocation = CliInvocation::new(
            executable,
            Vec::new(),
            "fixture",
            "fixture-model",
            "fixture account CLI",
        );
        let cancellation = CancellationToken::new();
        let mut receiver = complete_streaming(
            invocation,
            &[Message::user("hello")],
            &[],
            cancellation.clone(),
        )
        .await
        .unwrap();
        tokio::time::timeout(FIXTURE_PROCESS_TIMEOUT, async {
            while !started.exists() || !descendant_started.exists() {
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("fixture account CLI did not start its descendant");

        cancellation.cancel();
        tokio::time::timeout(FIXTURE_PROCESS_TIMEOUT, async {
            while receiver.recv().await.is_some() {}
        })
        .await
        .expect("account CLI task did not settle after cancellation");
        tokio::time::sleep(Duration::from_millis(700)).await;

        assert!(
            !leaked.exists(),
            "cancelling an account request must kill every CLI descendant"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn malformed_protocol_output_kills_account_cli_descendants() {
        use std::os::unix::fs::PermissionsExt;

        let directory = tempfile::tempdir().unwrap();
        let executable = directory.path().join("account-cli");
        let descendant_started = directory.path().join("descendant-started");
        let leaked = directory.path().join("protocol-leak");
        std::fs::write(
            &executable,
            format!(
                "#!/bin/sh\n(: > '{}'; sleep 0.30; : > '{}') &\n\
                 while [ ! -e '{}' ]; do :; done\n\
                 printf '\\377\\n'\nwait\n",
                descendant_started.display(),
                leaked.display(),
                descendant_started.display(),
            ),
        )
        .unwrap();
        let mut permissions = std::fs::metadata(&executable).unwrap().permissions();
        permissions.set_mode(0o755);
        std::fs::set_permissions(&executable, permissions).unwrap();
        let invocation = CliInvocation::new(
            executable,
            Vec::new(),
            "fixture",
            "fixture-model",
            "fixture account CLI",
        );
        let mut receiver = complete_streaming(
            invocation,
            &[Message::user("hello")],
            &[],
            CancellationToken::new(),
        )
        .await
        .unwrap();

        let diagnostic = tokio::time::timeout(FIXTURE_PROCESS_TIMEOUT, receiver.recv())
            .await
            .expect("transport did not report its protocol failure")
            .expect("transport closed without a protocol diagnostic");
        assert!(matches!(
            diagnostic,
            StreamEvent::TextDelta(ref detail)
                if detail.contains("account stream could not be read")
        ));
        assert!(
            descendant_started.exists(),
            "fixture must start its descendant before emitting malformed output"
        );
        while receiver.recv().await.is_some() {}
        tokio::time::sleep(Duration::from_millis(400)).await;

        assert!(
            !leaked.exists(),
            "a malformed account stream must terminate every CLI descendant"
        );
    }

    #[test]
    fn prompt_preserves_tool_history_as_structured_blocks() {
        let prompt = account_cli_prompt(&[
            Message::user("hello"),
            Message {
                role: "assistant".into(),
                content: vec![ContentBlock::ToolUse {
                    id: "toolu_1".into(),
                    name: "read".into(),
                    input: json!({"file_path":"README.md"}),
                }],
                reasoning_content: None,
            },
            Message::tool_result("toolu_1", "contents", false),
        ]);

        assert!(prompt.contains("User:\nhello"));
        assert!(prompt.contains("### A3S host tool call record"));
        assert!(prompt.contains("### A3S host tool result record"));
        assert!(!prompt.contains("<A3S_ASSISTANT_TOOL_CALL>"));
        assert!(!prompt.contains("<A3S_TOOL_RESULT>"));
        assert!(prompt.contains("\"status\":\"ok\""));
    }

    #[test]
    fn system_prompt_names_transport_and_never_embeds_tools_in_user_history() {
        let prompt = account_cli_system_prompt(
            Some("Be concise."),
            &[ToolDefinition {
                name: "read".into(),
                description: "Read a file".into(),
                parameters: json!({"type":"object"}),
            }],
            "WorkBuddy",
        )
        .unwrap();

        assert!(prompt.contains("# A3S System"));
        assert!(prompt.contains("WorkBuddy's own built-in tools"));
        assert!(prompt.contains("<function_calls>"));
    }

    #[tokio::test]
    async fn host_tool_mapper_emits_a3s_tool_events() {
        let tools = vec![ToolDefinition {
            name: "read".into(),
            description: "Read a file".into(),
            parameters: json!({
                "type":"object",
                "properties":{"file_path":{"type":"string"}},
                "required":["file_path"]
            }),
        }];
        let mut mapper = AccountCliHostToolMapper::new(
            StreamMeta {
                provider: "fake-account-cli",
                request_model: "model".into(),
                request_url: "fake account CLI".into(),
                started_at: Instant::now(),
            },
            tools,
        );
        let (tx, mut rx) = mpsc::channel(10);
        let lines = [
            r#"{"type":"stream_event","event":{"type":"message_start","message":{"id":"msg_1","type":"message","model":"model","usage":{"input_tokens":3,"output_tokens":0}}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"<function_calls><invoke name=\"Read\"><parameter name=\"file_path\">README.md</parameter></invoke></function_calls>"}}}"#,
            r#"{"type":"stream_event","event":{"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":7}}}"#,
            r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
        ];
        for line in lines {
            let event = parse_account_cli_stream_event(line).unwrap();
            if mapper.handle(event, &tx).await {
                break;
            }
        }

        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::ToolUseStart { name, .. }) if name == "read"
        ));
        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::ToolUseInputDelta { delta, .. }) if delta.contains("README.md")
        ));
        let Some(StreamEvent::Done(response)) = rx.recv().await else {
            panic!("expected done");
        };
        assert_eq!(
            response.tool_calls()[0].args,
            json!({"file_path":"README.md"})
        );
    }

    #[tokio::test]
    async fn host_tool_mapper_streams_normal_text_without_waiting_for_message_stop() {
        let mut mapper = AccountCliHostToolMapper::new(
            StreamMeta {
                provider: "fake-account-cli",
                request_model: "model".into(),
                request_url: "fake account CLI".into(),
                started_at: Instant::now(),
            },
            vec![ToolDefinition {
                name: "read".into(),
                description: "Read a file".into(),
                parameters: json!({"type":"object"}),
            }],
        );
        let (tx, mut rx) = mpsc::channel(10);
        let first = parse_account_cli_stream_event(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Hello"}}}"#,
        )
        .unwrap();

        assert!(!mapper.handle(first, &tx).await);
        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::TextDelta(text)) if text == "Hello"
        ));
    }

    #[tokio::test]
    async fn host_tool_mapper_hides_split_workbuddy_protocol_after_prose() {
        let mut mapper = AccountCliHostToolMapper::new(
            StreamMeta {
                provider: "fake-account-cli",
                request_model: "hy3".into(),
                request_url: "fake account CLI".into(),
                started_at: Instant::now(),
            },
            vec![ToolDefinition {
                name: "ls".into(),
                description: "List a directory".into(),
                parameters: json!({
                    "type":"object",
                    "properties":{"path":{"type":"string"}},
                    "required":["path"]
                }),
            }],
        );
        let (tx, mut rx) = mpsc::channel(10);
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"I'll list the workspace.<tool_"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"calls:group_1>\n<tool_call:call_1>ls\">\n<parameter name=\"path\">/work/a3s</parameter>\n</invoke>\n</function_calls>"}}}"#,
            r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
        ] {
            let event = parse_account_cli_stream_event(line).unwrap();
            if mapper.handle(event, &tx).await {
                break;
            }
        }

        let mut events = Vec::new();
        while let Ok(event) = rx.try_recv() {
            events.push(event);
        }
        assert!(events.iter().any(
            |event| matches!(event, StreamEvent::TextDelta(text) if text == "I'll list the workspace.")
        ));
        assert!(!events
            .iter()
            .any(|event| matches!(event, StreamEvent::TextDelta(text) if text.contains('<'))));
        assert!(events
            .iter()
            .any(|event| matches!(event, StreamEvent::ToolUseStart { name, .. } if name == "ls")));
        let response = events.into_iter().find_map(|event| match event {
            StreamEvent::Done(response) => Some(response),
            _ => None,
        });
        assert_eq!(
            response.unwrap().tool_calls()[0].args,
            json!({"path":"/work/a3s"})
        );
    }

    #[tokio::test]
    async fn host_tool_mapper_never_exposes_echoed_legacy_history_tags() {
        let mut mapper = AccountCliHostToolMapper::new(
            StreamMeta {
                provider: "fake-account-cli",
                request_model: "hy3".into(),
                request_url: "fake account CLI".into(),
                started_at: Instant::now(),
            },
            vec![],
        );
        let (tx, mut rx) = mpsc::channel(10);
        for line in [
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"Continuing from the previous result.<A3S_ASSISTANT_"}}}"#,
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"TOOL_CALL>\n{\"id\":\"call_1\"}\n</A3S_ASSISTANT_TOOL_CALL>\n<A3S_TOOL_RESULT>\n{\"status\":\"ok\"}\n</A3S_TOOL_RESULT>"}}}"#,
            r#"{"type":"stream_event","event":{"type":"message_stop"}}"#,
        ] {
            let event = parse_account_cli_stream_event(line).unwrap();
            if mapper.handle(event, &tx).await {
                break;
            }
        }

        let mut response = None;
        while let Ok(event) = rx.try_recv() {
            match event {
                StreamEvent::TextDelta(text) => {
                    assert!(!text.contains("<A3S"));
                }
                StreamEvent::Done(done) => response = Some(done),
                _ => {}
            }
        }

        let response = response.expect("expected final response");
        for block in response.message.content {
            if let ContentBlock::Text { text } = block {
                assert!(!text.contains("<A3S"));
            }
        }
        assert_eq!(
            response.stop_reason.as_deref(),
            Some("host_tool_protocol_error")
        );
    }
}
