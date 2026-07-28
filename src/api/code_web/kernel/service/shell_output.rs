use super::text::truncate_chars;
use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn session_output(
        &self,
        session_id: &str,
    ) -> BootResult<serde_json::Value> {
        self.kernel_session(session_id).await?;
        let messages = self
            .state
            .messages
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        Ok(session_output_json(session_id, &messages))
    }

    pub(in crate::api::code_web) async fn run_shell_command(
        &self,
        session_id: &str,
        request: ShellSessionRequest,
    ) -> BootResult<serde_json::Value> {
        let session = self.kernel_session(session_id).await?;
        let command = request.command.trim();
        if command.is_empty() {
            return Err(BootError::BadRequest("command is required".to_string()));
        }
        if command.chars().count() > 20_000 {
            return Err(BootError::BadRequest("command is too long".to_string()));
        }

        let cwd = session.workspace().to_path_buf();
        let started_at = chrono::Utc::now();
        let timer = Instant::now();
        let output = timeout(SHELL_COMMAND_TIMEOUT, async {
            let mut process = Command::new("sh");
            process
                .arg("-c")
                .arg(command)
                .current_dir(&cwd)
                .kill_on_drop(true);
            process.output().await
        })
        .await;
        let completed_at = chrono::Utc::now();
        let duration_ms = timer.elapsed().as_millis().min(u128::from(u64::MAX)) as u64;

        let (stdout, stderr, exit_code, timed_out, spawn_error) = match output {
            Ok(Ok(output)) => (
                shell_text(&output.stdout),
                shell_text(&output.stderr),
                output.status.code(),
                false,
                None,
            ),
            Ok(Err(error)) => (
                String::new(),
                format!("failed to run: {error}"),
                None,
                false,
                Some(error.to_string()),
            ),
            Err(_) => (
                String::new(),
                format!(
                    "command timed out after {} seconds",
                    SHELL_COMMAND_TIMEOUT.as_secs()
                ),
                None,
                true,
                None,
            ),
        };
        let mut combined = String::new();
        combined.push_str(&stdout);
        combined.push_str(&stderr);
        let output_text = if combined.trim().is_empty() {
            match exit_code {
                Some(code) => format!("(exit {code})"),
                None => stderr.clone(),
            }
        } else {
            truncate_chars(&combined, SHELL_OUTPUT_MAX_CHARS)
        };
        let is_error = timed_out || spawn_error.is_some() || exit_code.is_none_or(|code| code != 0);
        let record = shell_output_record(ShellOutputRecordInput {
            session_id,
            command,
            cwd: &cwd.display().to_string(),
            stdout: &truncate_chars(&stdout, SHELL_OUTPUT_MAX_CHARS),
            stderr: &truncate_chars(&stderr, SHELL_OUTPUT_MAX_CHARS),
            output: &output_text,
            exit_code,
            is_error,
            timed_out,
            duration_ms,
            started_at: &started_at.to_rfc3339(),
            completed_at: &completed_at.to_rfc3339(),
        });
        self.append_shell_output_message(session_id, &record)
            .await?;

        Ok(json!({
            "sessionId": session_id,
            "command": command,
            "cwd": cwd.display().to_string(),
            "stdout": truncate_chars(&stdout, SHELL_OUTPUT_MAX_CHARS),
            "stderr": truncate_chars(&stderr, SHELL_OUTPUT_MAX_CHARS),
            "output": output_text,
            "exitCode": exit_code,
            "success": !is_error,
            "isError": is_error,
            "timedOut": timed_out,
            "durationMs": duration_ms,
            "startedAt": started_at.to_rfc3339(),
            "completedAt": completed_at.to_rfc3339(),
            "record": record,
        }))
    }

    async fn append_shell_output_message(
        &self,
        session_id: &str,
        record: &Value,
    ) -> BootResult<()> {
        let id = format!("{}-shell", chrono::Utc::now().timestamp_millis());
        let command = record
            .get("input")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let output = record
            .get("output")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let tool_use_id = record
            .get("toolUseId")
            .and_then(Value::as_str)
            .unwrap_or(&id);
        let message = json!({
            "id": id,
            "sessionId": session_id,
            "role": "assistant",
            "content": format!("Shell command finished: {command}"),
            "createdAt": chrono::Utc::now().to_rfc3339(),
            "source": "command:!",
            "contentBlocks": [
                {
                    "type": "tool_use",
                    "id": tool_use_id,
                    "name": "shell_command",
                    "input": {
                        "command": command,
                        "cwd": record.get("cwd").cloned().unwrap_or(Value::Null),
                    }
                },
                {
                    "type": "tool_result",
                    "toolUseId": tool_use_id,
                    "content": output,
                    "isError": record.get("isError").cloned().unwrap_or(Value::Bool(false)),
                    "exitCode": record.get("exitCode").cloned().unwrap_or(Value::Null),
                    "durationMs": record.get("durationMs").cloned().unwrap_or(Value::Null),
                }
            ],
        });
        let is_error = record
            .get("isError")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        self.save_code_web_message_to_timeline(
            session_id,
            &Message::tool_result(tool_use_id, output, is_error),
        )
        .await?;
        self.state
            .messages
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .push(message);
        self.persist_session_state(session_id).await
    }
}

#[derive(Debug, Clone)]
struct PendingToolUse {
    tool_name: String,
    input: String,
    created_at: Option<String>,
    source_message_id: String,
}

pub(super) struct ShellOutputRecordInput<'a> {
    pub(super) session_id: &'a str,
    pub(super) command: &'a str,
    pub(super) cwd: &'a str,
    pub(super) stdout: &'a str,
    pub(super) stderr: &'a str,
    pub(super) output: &'a str,
    pub(super) exit_code: Option<i32>,
    pub(super) is_error: bool,
    pub(super) timed_out: bool,
    pub(super) duration_ms: u64,
    pub(super) started_at: &'a str,
    pub(super) completed_at: &'a str,
}

fn shell_text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

pub(super) fn shell_output_record(input: ShellOutputRecordInput<'_>) -> Value {
    let tool_use_id = format!(
        "shell-{}",
        input
            .started_at
            .chars()
            .filter(|ch| ch.is_ascii_alphanumeric())
            .collect::<String>()
    );
    json!({
        "id": tool_use_id,
        "index": 0,
        "toolUseId": tool_use_id,
        "toolName": "shell_command",
        "input": input.command,
        "output": input.output,
        "stdout": input.stdout,
        "stderr": input.stderr,
        "cwd": input.cwd,
        "exitCode": input.exit_code,
        "success": !input.is_error,
        "isError": input.is_error,
        "timedOut": input.timed_out,
        "durationMs": input.duration_ms,
        "createdAt": input.started_at,
        "completedAt": input.completed_at,
        "sourceMessageId": Value::Null,
        "resultMessageId": Value::Null,
        "sessionId": input.session_id,
    })
}

pub(super) fn session_output_json(session_id: &str, messages: &[Value]) -> Value {
    let items = tool_output_records_from_messages(messages);
    let total = items.len();
    json!({
        "sessionId": session_id,
        "items": items,
        "total": total,
        "format": "structured-tool-log",
    })
}

fn tool_output_records_from_messages(messages: &[Value]) -> Vec<Value> {
    let mut records = Vec::new();
    let mut pending: HashMap<String, PendingToolUse> = HashMap::new();
    let mut last_tool_use_id: Option<String> = None;

    for (message_index, message) in messages.iter().enumerate() {
        let source_message_id = string_field(message, &["id", "messageId", "message_id"])
            .unwrap_or_else(|| format!("message-{message_index}"));
        let created_at = string_field(
            message,
            &["createdAt", "created_at", "timestamp", "created"],
        );

        for (block_index, block) in message_blocks(message).into_iter().enumerate() {
            let block_type = string_field(block, &["type"]).unwrap_or_default();
            match block_type.as_str() {
                "tool_use" | "toolUse" | "tool-call-input" => {
                    let tool_use_id = string_field(
                        block,
                        &[
                            "id",
                            "toolUseId",
                            "tool_use_id",
                            "toolCallId",
                            "tool_call_id",
                        ],
                    )
                    .unwrap_or_else(|| format!("tool-{message_index}-{block_index}"));
                    let tool_name = string_field(
                        block,
                        &["name", "toolName", "tool_name", "tool", "function"],
                    )
                    .unwrap_or_else(|| "tool".to_string());
                    let input = first_field(
                        block,
                        &["input", "toolInput", "tool_input", "args", "arguments"],
                    )
                    .map(stringify_json_value)
                    .unwrap_or_default();
                    pending.insert(
                        tool_use_id.clone(),
                        PendingToolUse {
                            tool_name,
                            input,
                            created_at: created_at.clone(),
                            source_message_id: source_message_id.clone(),
                        },
                    );
                    last_tool_use_id = Some(tool_use_id);
                }
                "tool_result" | "toolResult" | "tool-call-output" => {
                    let tool_use_id = string_field(
                        block,
                        &[
                            "toolUseId",
                            "tool_use_id",
                            "toolCallId",
                            "tool_call_id",
                            "id",
                        ],
                    )
                    .or_else(|| last_tool_use_id.clone())
                    .unwrap_or_else(|| format!("tool-result-{message_index}-{block_index}"));
                    let pending_use = pending.remove(&tool_use_id);
                    let tool_name = pending_use
                        .as_ref()
                        .map(|tool_use| tool_use.tool_name.clone())
                        .or_else(|| {
                            string_field(
                                block,
                                &["name", "toolName", "tool_name", "tool", "function"],
                            )
                        })
                        .unwrap_or_else(|| "result".to_string());
                    let input = pending_use
                        .as_ref()
                        .map(|tool_use| tool_use.input.clone())
                        .or_else(|| {
                            first_field(
                                block,
                                &["input", "toolInput", "tool_input", "args", "arguments"],
                            )
                            .map(stringify_json_value)
                        })
                        .unwrap_or_default();
                    records.push(json!({
                        "id": tool_use_id,
                        "index": records.len(),
                        "toolUseId": tool_use_id,
                        "toolName": tool_name,
                        "input": input,
                        "output": first_field(
                            block,
                            &["content", "output", "toolOutput", "tool_output", "result"],
                        )
                        .map(stringify_tool_output)
                        .unwrap_or_default(),
                        "isError": bool_field(block, &["isError", "is_error", "error"]).unwrap_or(false),
                        "exitCode": first_field(block, &["exitCode", "exit_code", "status"]).cloned().unwrap_or(Value::Null),
                        "before": first_field(block, &["before"]).cloned().unwrap_or(Value::Null),
                        "after": first_field(block, &["after"]).cloned().unwrap_or(Value::Null),
                        "filePath": first_field(block, &["filePath", "file_path", "path"]).cloned().unwrap_or(Value::Null),
                        "durationMs": first_field(block, &["durationMs", "duration_ms", "elapsedMs", "elapsed_ms"]).cloned().unwrap_or(Value::Null),
                        "createdAt": pending_use
                            .as_ref()
                            .and_then(|tool_use| tool_use.created_at.clone())
                            .or_else(|| created_at.clone()),
                        "sourceMessageId": pending_use
                            .as_ref()
                            .map(|tool_use| tool_use.source_message_id.clone())
                            .unwrap_or_else(|| source_message_id.clone()),
                        "resultMessageId": source_message_id.clone(),
                    }));
                }
                "tool_call" | "tool" | "completed_tool" | "completed_tool_call" => {
                    if first_field(
                        block,
                        &["output", "content", "toolOutput", "tool_output", "result"],
                    )
                    .is_none()
                    {
                        continue;
                    }
                    let tool_use_id = string_field(
                        block,
                        &[
                            "toolUseId",
                            "tool_use_id",
                            "toolCallId",
                            "tool_call_id",
                            "id",
                        ],
                    )
                    .unwrap_or_else(|| format!("tool-{message_index}-{block_index}"));
                    records.push(json!({
                        "id": tool_use_id,
                        "index": records.len(),
                        "toolUseId": tool_use_id,
                        "toolName": string_field(block, &["toolName", "tool_name", "name", "tool"])
                            .unwrap_or_else(|| "tool".to_string()),
                        "input": first_field(
                            block,
                            &["input", "toolInput", "tool_input", "args", "arguments"],
                        )
                        .map(stringify_json_value)
                        .unwrap_or_default(),
                        "output": first_field(
                            block,
                            &["output", "content", "toolOutput", "tool_output", "result"],
                        )
                        .map(stringify_tool_output)
                        .unwrap_or_default(),
                        "isError": bool_field(block, &["isError", "is_error", "error"]).unwrap_or(false),
                        "exitCode": first_field(block, &["exitCode", "exit_code", "status"]).cloned().unwrap_or(Value::Null),
                        "before": first_field(block, &["before"]).cloned().unwrap_or(Value::Null),
                        "after": first_field(block, &["after"]).cloned().unwrap_or(Value::Null),
                        "filePath": first_field(block, &["filePath", "file_path", "path"]).cloned().unwrap_or(Value::Null),
                        "durationMs": first_field(block, &["durationMs", "duration_ms", "elapsedMs", "elapsed_ms"]).cloned().unwrap_or(Value::Null),
                        "createdAt": created_at.clone(),
                        "sourceMessageId": source_message_id.clone(),
                        "resultMessageId": source_message_id,
                    }));
                }
                _ => {}
            }
        }
    }

    records
}

fn message_blocks(message: &Value) -> Vec<&Value> {
    if let Some(blocks) = first_field(message, &["contentBlocks", "content_blocks", "blocks"])
        .and_then(Value::as_array)
    {
        return blocks.iter().collect();
    }

    if let Some(blocks) = message.get("content").and_then(Value::as_array) {
        return blocks.iter().collect();
    }

    if let Some(nested_message) = message.get("message") {
        if let Some(blocks) = first_field(
            nested_message,
            &["contentBlocks", "content_blocks", "blocks"],
        )
        .and_then(Value::as_array)
        {
            return blocks.iter().collect();
        }
        if let Some(blocks) = nested_message.get("content").and_then(Value::as_array) {
            return blocks.iter().collect();
        }
    }

    match message.get("type").and_then(Value::as_str) {
        Some("tool_use" | "toolUse" | "tool_result" | "toolResult" | "tool_call" | "tool") => {
            vec![message]
        }
        _ => Vec::new(),
    }
}

fn first_field<'a>(value: &'a Value, keys: &[&str]) -> Option<&'a Value> {
    let map = value.as_object()?;
    keys.iter()
        .filter_map(|key| map.get(*key))
        .find(|field| !field.is_null())
}

fn string_field(value: &Value, keys: &[&str]) -> Option<String> {
    first_field(value, keys).and_then(json_scalar_to_string)
}

fn bool_field(value: &Value, keys: &[&str]) -> Option<bool> {
    match first_field(value, keys)? {
        Value::Bool(value) => Some(*value),
        Value::String(value) => match value.trim().to_ascii_lowercase().as_str() {
            "true" | "yes" | "1" => Some(true),
            "false" | "no" | "0" => Some(false),
            _ => None,
        },
        _ => None,
    }
}

fn json_scalar_to_string(value: &Value) -> Option<String> {
    match value {
        Value::String(value) => {
            let value = value.trim();
            (!value.is_empty()).then(|| value.to_string())
        }
        Value::Number(value) => Some(value.to_string()),
        Value::Bool(value) => Some(value.to_string()),
        _ => None,
    }
}

fn stringify_json_value(value: &Value) -> String {
    match value {
        Value::Null => String::new(),
        Value::String(value) => value.clone(),
        _ => serde_json::to_string_pretty(value).unwrap_or_else(|_| value.to_string()),
    }
}

fn stringify_tool_output(value: &Value) -> String {
    match value {
        Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    if let Value::String(value) = item {
                        return Some(value.as_str());
                    }
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .or_else(|| item.get("message"))
                        .and_then(Value::as_str)
                })
                .collect::<Vec<_>>()
                .join("\n");
            if text.trim().is_empty() {
                stringify_json_value(value)
            } else {
                text
            }
        }
        _ => stringify_json_value(value),
    }
}
