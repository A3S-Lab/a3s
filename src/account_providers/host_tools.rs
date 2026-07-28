use a3s_code_core::llm::{ContentBlock, ToolDefinition};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

const TOOL_CALLS_OPEN: &str = "<A3S_TOOL_CALLS>";
const TOOL_CALLS_CLOSE: &str = "</A3S_TOOL_CALLS>";
const PROTOCOL_VERSION: &str = "a3s.host_tools.v1";
static HOST_TOOL_CALL_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HostToolCall {
    pub id: String,
    pub name: String,
    pub input: Value,
}

impl HostToolCall {
    pub(crate) fn into_content_block(self) -> ContentBlock {
        ContentBlock::ToolUse {
            id: self.id,
            name: self.name,
            input: self.input,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HostToolParseResult {
    NoCall,
    Calls(Vec<HostToolCall>),
    Invalid(String),
}

pub(crate) fn host_tool_instructions(
    transport_name: &str,
    tools: &[ToolDefinition],
) -> Option<String> {
    if tools.is_empty() {
        return None;
    }

    let names = tools
        .iter()
        .map(|tool| format!("`{}`", tool.name))
        .collect::<Vec<_>>()
        .join(", ");
    let tools_json = serde_json::to_string_pretty(
        &tools
            .iter()
            .map(|tool| {
                json!({
                    "name": tool.name,
                    "description": tool.description,
                    "input_schema": tool.parameters,
                })
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());

    Some(format!(
        "# A3S Host Tools\n\n\
         Protocol: {PROTOCOL_VERSION}\n\n\
         a3s-code host tools are available in this session. {transport_name}'s own \
         built-in tools are disabled for this transport, so when you need files, \
         commands, web access, skills, or subagents, request a3s host tools \
         instead of trying to execute {transport_name} tools directly. Do not describe \
         the tool call in prose.\n\n\
         Preferred account-CLI-compatible form:\n\n\
         <function_calls>\n\
         <invoke name=\"read\">\n\
         <parameter name=\"file_path\">README.md</parameter>\n\
         </invoke>\n\
         </function_calls>\n\n\
         For complex JSON inputs, put the JSON value inside the parameter text:\n\n\
         <function_calls>\n\
         <invoke name=\"parallel_task\">\n\
         <parameter name=\"tasks\">[{{\"agent\":\"explore\",\"description\":\"Find API entrypoints\",\"prompt\":\"Inspect the API module.\"}}]</parameter>\n\
         </invoke>\n\
         </function_calls>\n\n\
         Rules:\n\
         - Use only these tool names: {names}.\n\
         - Parameter names and values must match the tool's JSON schema exactly; \
         for file tools use `file_path`, not `path`.\n\
         - Request multiple independent tools with multiple `<invoke>` blocks \
         when useful; a3s decides whether they can run in parallel.\n\
         - If you need a final answer and no tool is needed, answer normally with \
         no envelope.\n\
         - After a3s returns an A3S host-tool result record in conversation \
         history, continue from that observation; do not repeat a recorded call.\n\n\
         Available tools:\n\
         ```json\n{tools_json}\n```\n\n"
    ))
}

pub(crate) fn parse_host_tool_calls(text: &str, tools: &[ToolDefinition]) -> HostToolParseResult {
    if let Some(payload) = extract_tool_payload(text) {
        let envelope = match parse_tool_envelope(payload) {
            Ok(envelope) => envelope,
            Err(error) => return HostToolParseResult::Invalid(error),
        };
        return build_host_tool_calls(envelope.calls, tools, true);
    }
    if text.contains(TOOL_CALLS_OPEN) {
        return HostToolParseResult::Invalid("unterminated a3s host tool envelope".into());
    }

    if text.contains("<tool_calls:") || text.contains("<tool_call:") {
        return parse_workbuddy_tagged_calls(text, tools);
    }

    if text.contains("<A3S_ASSISTANT_TOOL_CALL>") || text.contains("<A3S_TOOL_RESULT>") {
        return HostToolParseResult::Invalid(
            "the account model echoed an internal host-tool history record".into(),
        );
    }

    if text.contains("<function_calls>") || text.contains("<invoke ") {
        return parse_claude_function_calls(text, tools);
    }

    HostToolParseResult::NoCall
}

fn build_host_tool_calls(
    items: Vec<ToolCallEnvelopeItem>,
    tools: &[ToolDefinition],
    strict_unknown_tools: bool,
) -> HostToolParseResult {
    if items.is_empty() {
        return HostToolParseResult::Invalid("tool envelope contains no calls".into());
    }
    let valid_tools = tools.iter().map(|tool| tool.name.as_str()).collect();
    let tool_by_name = tools
        .iter()
        .map(|tool| (tool.name.as_str(), tool))
        .collect::<HashMap<_, _>>();
    let tool_names = tool_name_lookup(tools);
    let mut calls = Vec::new();
    for (index, call) in items.into_iter().enumerate() {
        let Some(raw_name) = call.name.filter(|name| !name.trim().is_empty()) else {
            return HostToolParseResult::Invalid(format!(
                "tool call {} is missing `name`",
                index + 1
            ));
        };
        let Some(name) = normalize_tool_name(&raw_name, &valid_tools, &tool_names) else {
            if !strict_unknown_tools {
                continue;
            }
            return HostToolParseResult::Invalid(format!(
                "unknown a3s host tool `{}`",
                raw_name.trim()
            ));
        };
        let Some(tool) = tool_by_name.get(name.as_str()) else {
            return HostToolParseResult::Invalid(format!(
                "unknown a3s host tool `{}`",
                raw_name.trim()
            ));
        };
        let input = match normalize_tool_input(&name, call.input) {
            Ok(input) => input,
            Err(error) => return HostToolParseResult::Invalid(error),
        };
        if let Err(error) = validate_required_input(tool, &input) {
            return HostToolParseResult::Invalid(error);
        }
        calls.push(HostToolCall {
            id: call
                .id
                .filter(|id| !id.trim().is_empty())
                .unwrap_or_else(|| {
                    let sequence = HOST_TOOL_CALL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
                    format!("account_cli_tool_{sequence}_{}", index + 1)
                }),
            name,
            input,
        });
    }
    if calls.is_empty() {
        HostToolParseResult::Invalid("tool output did not contain any usable a3s host calls".into())
    } else {
        HostToolParseResult::Calls(calls)
    }
}

fn extract_tool_payload(text: &str) -> Option<&str> {
    let start = text.find(TOOL_CALLS_OPEN)? + TOOL_CALLS_OPEN.len();
    let rest = &text[start..];
    let end = rest.find(TOOL_CALLS_CLOSE)?;
    Some(&rest[..end])
}

fn parse_tool_envelope(payload: &str) -> Result<ToolCallEnvelope, String> {
    let payload = strip_markdown_json_fence(payload.trim());
    serde_json::from_str::<ToolCallEnvelope>(payload)
        .or_else(|first_error| {
            extract_first_json_object(payload)
                .ok_or_else(|| first_error.to_string())
                .and_then(|json| {
                    serde_json::from_str::<ToolCallEnvelope>(json).map_err(|error| {
                        format!("invalid a3s host tool JSON: {error}; first parse: {first_error}")
                    })
                })
        })
        .map_err(|error| format!("invalid a3s host tool JSON: {error}"))
}

fn strip_markdown_json_fence(payload: &str) -> &str {
    let trimmed = payload.trim();
    let Some(after_ticks) = trimmed.strip_prefix("```") else {
        return trimmed;
    };
    let after_header = after_ticks
        .find('\n')
        .map(|idx| &after_ticks[idx + 1..])
        .unwrap_or(after_ticks);
    after_header
        .trim()
        .strip_suffix("```")
        .unwrap_or(after_header)
        .trim()
}

fn extract_first_json_object(input: &str) -> Option<&str> {
    let start = input.find('{')?;
    let mut depth = 0usize;
    let mut in_string = false;
    let mut escaped = false;
    for (offset, ch) in input[start..].char_indices() {
        if in_string {
            if escaped {
                escaped = false;
            } else if ch == '\\' {
                escaped = true;
            } else if ch == '"' {
                in_string = false;
            }
            continue;
        }
        match ch {
            '"' => in_string = true,
            '{' => depth += 1,
            '}' => {
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return Some(&input[start..start + offset + ch.len_utf8()]);
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_claude_function_calls(text: &str, tools: &[ToolDefinition]) -> HostToolParseResult {
    let mut items = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<invoke ") {
        rest = &rest[start..];
        let Some(header_end) = rest.find('>') else {
            return HostToolParseResult::Invalid("unterminated Claude function invoke".into());
        };
        let header = &rest[..=header_end];
        let body_start = header_end + 1;
        let Some(body_end) = rest[body_start..].find("</invoke>") else {
            return HostToolParseResult::Invalid("unterminated Claude function invoke".into());
        };
        let body = &rest[body_start..body_start + body_end];
        let name = xml_attr(header, "name");
        let input = Value::Object(parse_claude_parameters(body));
        items.push(ToolCallEnvelopeItem {
            id: None,
            name,
            input: Some(input),
        });
        rest = &rest[body_start + body_end + "</invoke>".len()..];
    }

    build_host_tool_calls(dedupe_items(items), tools, false)
}

/// Parse the tagged function-call dialect emitted by some WorkBuddy models.
/// WorkBuddy replaces the opening `function_calls` / `invoke` tags with
/// `<tool_calls:group-id>` and `<tool_call:call-id>name">`, while retaining
/// the ordinary parameter and closing tags.
fn parse_workbuddy_tagged_calls(text: &str, tools: &[ToolDefinition]) -> HostToolParseResult {
    let mut items = Vec::new();
    let mut rest = text;
    while let Some(start) = rest.find("<tool_call:") {
        rest = &rest[start + "<tool_call:".len()..];
        let Some(id_end) = rest.find('>') else {
            return HostToolParseResult::Invalid("unterminated WorkBuddy tool-call id".into());
        };
        let id = rest[..id_end].trim();
        if id.is_empty()
            || id.len() > 128
            || !id
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':'))
        {
            return HostToolParseResult::Invalid("invalid WorkBuddy tool-call id".into());
        }

        rest = &rest[id_end + 1..];
        let Some(name_end) = rest.find("\">") else {
            return HostToolParseResult::Invalid("unterminated WorkBuddy tool name".into());
        };
        let name = decode_xml_entities(rest[..name_end].trim());
        if name.is_empty() {
            return HostToolParseResult::Invalid("WorkBuddy tool call is missing a name".into());
        }

        let body_start = name_end + "\">".len();
        let Some(body_end) = rest[body_start..].find("</invoke>") else {
            return HostToolParseResult::Invalid("unterminated WorkBuddy function invoke".into());
        };
        let body = &rest[body_start..body_start + body_end];
        items.push(ToolCallEnvelopeItem {
            id: Some(id.to_string()),
            name: Some(name),
            input: Some(Value::Object(parse_claude_parameters(body))),
        });
        rest = &rest[body_start + body_end + "</invoke>".len()..];
    }

    if items.is_empty() {
        return HostToolParseResult::Invalid("WorkBuddy tool envelope contains no calls".into());
    }
    build_host_tool_calls(dedupe_items(items), tools, true)
}

fn parse_claude_parameters(body: &str) -> serde_json::Map<String, Value> {
    let mut params = serde_json::Map::new();
    let mut rest = body;
    while let Some(start) = rest.find("<parameter ") {
        rest = &rest[start..];
        let Some(header_end) = rest.find('>') else {
            break;
        };
        let header = &rest[..=header_end];
        let value_start = header_end + 1;
        let Some(value_end) = rest[value_start..].find("</parameter>") else {
            break;
        };
        if let Some(name) = xml_attr(header, "name") {
            let raw = decode_xml_entities(rest[value_start..value_start + value_end].trim());
            params.insert(name, parse_parameter_value(&raw));
        }
        rest = &rest[value_start + value_end + "</parameter>".len()..];
    }
    params
}

fn xml_attr(tag: &str, attr: &str) -> Option<String> {
    let needle = format!("{attr}=\"");
    let start = tag.find(&needle)? + needle.len();
    let rest = &tag[start..];
    let end = rest.find('"')?;
    Some(decode_xml_entities(&rest[..end]))
}

fn decode_xml_entities(value: &str) -> String {
    value
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&amp;", "&")
}

fn parse_parameter_value(raw: &str) -> Value {
    serde_json::from_str(raw).unwrap_or_else(|_| Value::String(raw.to_string()))
}

fn dedupe_items(items: Vec<ToolCallEnvelopeItem>) -> Vec<ToolCallEnvelopeItem> {
    let mut seen = HashSet::new();
    let mut out = Vec::new();
    for item in items {
        let key = format!(
            "{}\n{}",
            item.name.as_deref().unwrap_or_default(),
            item.input
                .as_ref()
                .map(Value::to_string)
                .unwrap_or_else(String::new)
        );
        if seen.insert(key) {
            out.push(item);
        }
    }
    out
}

fn tool_name_lookup(tools: &[ToolDefinition]) -> HashMap<String, String> {
    let mut names = HashMap::new();
    let available = tools
        .iter()
        .map(|tool| tool.name.as_str())
        .collect::<HashSet<_>>();

    for tool in tools {
        names.insert(tool_name_key(&tool.name), tool.name.clone());
    }

    for (alias, canonical) in [
        ("Read", "read"),
        ("ReadFile", "read"),
        ("read_file", "read"),
        ("Bash", "bash"),
        ("Shell", "bash"),
        ("Run", "bash"),
        ("Grep", "grep"),
        ("Search", "grep"),
        ("Glob", "glob"),
        ("LS", "ls"),
        ("List", "ls"),
        ("Write", "write"),
        ("WriteFile", "write"),
        ("write_file", "write"),
        ("Edit", "edit"),
        ("Update", "edit"),
        ("Patch", "patch"),
        ("Task", "task"),
        ("ParallelTask", "parallel_task"),
        ("parallelTask", "parallel_task"),
        ("WebSearch", "web_search"),
        ("WebFetch", "web_fetch"),
        ("Skill", "Skill"),
    ] {
        if available.contains(canonical) {
            names
                .entry(tool_name_key(alias))
                .or_insert(canonical.into());
        }
    }

    names
}

fn normalize_tool_name(
    raw: &str,
    valid_tools: &HashSet<&str>,
    tool_names: &HashMap<String, String>,
) -> Option<String> {
    let trimmed = raw.trim();
    if valid_tools.contains(trimmed) {
        return Some(trimmed.to_string());
    }
    tool_names.get(&tool_name_key(trimmed)).cloned()
}

fn tool_name_key(name: &str) -> String {
    name.trim()
        .chars()
        .flat_map(char::to_lowercase)
        .map(|ch| if ch == '-' || ch == ' ' { '_' } else { ch })
        .collect()
}

fn normalize_tool_input(name: &str, input: Option<Value>) -> Result<Value, String> {
    let mut input = input.unwrap_or_else(|| json!({}));
    if let Value::String(raw) = &input {
        input = serde_json::from_str(raw.trim()).map_err(|error| {
            format!("tool `{name}` arguments were a string but not valid JSON: {error}")
        })?;
    }

    let Value::Object(map) = &mut input else {
        return Err(format!("tool `{name}` input must be a JSON object"));
    };

    if requires_file_path(name) && !map.contains_key("file_path") {
        for alias in ["path", "file", "filename", "filepath"] {
            if let Some(value) = map.remove(alias) {
                map.insert("file_path".into(), value);
                break;
            }
        }
    }
    if name == "grep" && !map.contains_key("pattern") {
        if let Some(value) = map.remove("query") {
            map.insert("pattern".into(), value);
        }
    }
    if name == "web_search" && !map.contains_key("query") {
        for alias in ["pattern", "search"] {
            if let Some(value) = map.remove(alias) {
                map.insert("query".into(), value);
                break;
            }
        }
    }

    Ok(input)
}

fn requires_file_path(name: &str) -> bool {
    matches!(name, "read" | "write" | "edit" | "patch")
}

fn validate_required_input(tool: &ToolDefinition, input: &Value) -> Result<(), String> {
    let required = tool
        .parameters
        .get("required")
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(Value::as_str)
        .collect::<Vec<_>>();
    if required.is_empty() {
        return Ok(());
    }

    let Some(map) = input.as_object() else {
        return Err(format!("tool `{}` input must be a JSON object", tool.name));
    };
    let missing = required
        .iter()
        .filter(|field| !map.contains_key(**field))
        .copied()
        .collect::<Vec<_>>();
    if missing.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "tool `{}` input is missing required field(s): {}",
            tool.name,
            missing.join(", ")
        ))
    }
}

#[derive(Debug, Deserialize)]
struct ToolCallEnvelope {
    #[serde(default)]
    calls: Vec<ToolCallEnvelopeItem>,
}

#[derive(Debug, Deserialize)]
struct ToolCallEnvelopeItem {
    #[serde(default)]
    id: Option<String>,
    #[serde(default, alias = "tool")]
    name: Option<String>,
    #[serde(default)]
    #[serde(alias = "args", alias = "arguments")]
    input: Option<Value>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools() -> Vec<ToolDefinition> {
        vec![
            ToolDefinition {
                name: "read".into(),
                description: "Read a file".into(),
                parameters: json!({
                    "type":"object",
                    "properties":{"file_path":{"type":"string"}},
                    "required":["file_path"]
                }),
            },
            ToolDefinition {
                name: "bash".into(),
                description: "Run a command".into(),
                parameters: json!({
                    "type":"object",
                    "properties":{"command":{"type":"string"}},
                    "required":["command"]
                }),
            },
        ]
    }

    #[test]
    fn instructions_include_tool_schema_and_account_cli_protocol() {
        let instructions = host_tool_instructions("WorkBuddy", &tools()).unwrap();

        assert!(instructions.contains("WorkBuddy's own built-in tools"));
        assert!(!instructions.contains("<A3S_TOOL_CALLS>"));
        assert!(instructions.contains("<function_calls>"));
        assert!(instructions.contains("<invoke name=\"read\">"));
        assert!(instructions.contains("\"name\": \"read\""));
        assert!(instructions.contains("\"file_path\""));
        assert!(instructions.contains("input_schema"));
    }

    #[test]
    fn parses_valid_host_tool_calls() {
        let result = parse_host_tool_calls(
            r#"noise
<A3S_TOOL_CALLS>
{"calls":[{"name":"read","input":{"file_path":"README.md"}},{"id":"custom","name":"bash","input":{"command":"pwd"}}]}
</A3S_TOOL_CALLS>"#,
            &tools(),
        );
        let HostToolParseResult::Calls(calls) = result else {
            panic!("expected calls, got {result:?}");
        };

        assert_eq!(calls.len(), 2);
        assert!(calls[0].id.starts_with("account_cli_tool_"));
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].input, json!({"file_path":"README.md"}));
        assert_eq!(calls[1].id, "custom");
    }

    #[test]
    fn generated_tool_ids_are_unique_across_account_cli_rounds() {
        let text = r#"<function_calls><invoke name="Read"><parameter name="file_path">README.md</parameter></invoke></function_calls>"#;
        let HostToolParseResult::Calls(first) = parse_host_tool_calls(text, &tools()) else {
            panic!("expected first call");
        };
        let HostToolParseResult::Calls(second) = parse_host_tool_calls(text, &tools()) else {
            panic!("expected second call");
        };

        assert_ne!(first[0].id, second[0].id);
    }

    #[test]
    fn normalizes_common_claude_code_tool_names_and_args() {
        let result = parse_host_tool_calls(
            r#"<A3S_TOOL_CALLS>{"calls":[{"tool":"Read","args":{"path":"README.md"}}]}</A3S_TOOL_CALLS>"#,
            &tools(),
        );
        let HostToolParseResult::Calls(calls) = result else {
            panic!("expected calls, got {result:?}");
        };

        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].input, json!({"file_path":"README.md"}));
    }

    #[test]
    fn rejects_unknown_tools_plain_text_and_missing_required_fields() {
        assert_eq!(
            parse_host_tool_calls("plain answer", &tools()),
            HostToolParseResult::NoCall
        );
        assert!(matches!(
            parse_host_tool_calls(
                r#"<A3S_TOOL_CALLS>{"calls":[{"name":"unknown","input":{}}]}</A3S_TOOL_CALLS>"#,
                &tools(),
            ),
            HostToolParseResult::Invalid(reason) if reason.contains("unknown")
        ));
        assert!(matches!(
            parse_host_tool_calls(
                r#"<A3S_TOOL_CALLS>{"calls":[{"name":"bash","input":{}}]}</A3S_TOOL_CALLS>"#,
                &tools(),
            ),
            HostToolParseResult::Invalid(reason) if reason.contains("command")
        ));
    }

    #[test]
    fn accepts_fenced_json_inside_envelope() {
        let result = parse_host_tool_calls(
            r#"<A3S_TOOL_CALLS>
```json
{"calls":[{"name":"bash","input":{"command":"pwd"}}]}
```
</A3S_TOOL_CALLS>"#,
            &tools(),
        );

        assert!(matches!(result, HostToolParseResult::Calls(calls) if calls[0].name == "bash"));
    }

    #[test]
    fn parses_claude_code_function_call_xml() {
        let result = parse_host_tool_calls(
            r#"<function_calls>
<invoke name="Read">
<parameter name="file_path">README.md</parameter>
</invoke>
</function_calls>
<function_calls>
<invoke name="Bash">
<parameter name="command">pwd</parameter>
</invoke>
</function_calls>"#,
            &tools(),
        );
        let HostToolParseResult::Calls(calls) = result else {
            panic!("expected calls, got {result:?}");
        };

        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].name, "read");
        assert_eq!(calls[0].input, json!({"file_path":"README.md"}));
        assert_eq!(calls[1].name, "bash");
        assert_eq!(calls[1].input, json!({"command":"pwd"}));
    }

    #[test]
    fn parses_workbuddy_tagged_function_call_xml() {
        let ls = ToolDefinition {
            name: "ls".into(),
            description: "List a directory".into(),
            parameters: json!({
                "type":"object",
                "properties":{"path":{"type":"string"}},
                "required":["path"]
            }),
        };
        let result = parse_host_tool_calls(
            r#"I'll list the workspace.<tool_calls:group_1>
<tool_call:call_1>ls">
<parameter name="path">/work/a3s</parameter>
</invoke>
</function_calls>"#,
            &[ls],
        );
        let HostToolParseResult::Calls(calls) = result else {
            panic!("expected calls, got {result:?}");
        };

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].id, "call_1");
        assert_eq!(calls[0].name, "ls");
        assert_eq!(calls[0].input, json!({"path":"/work/a3s"}));
    }

    #[test]
    fn dedupes_repeated_claude_code_function_calls() {
        let result = parse_host_tool_calls(
            r#"<function_calls><invoke name="Read"><parameter name="file_path">README.md</parameter></invoke></function_calls>
<function_calls><invoke name="Read"><parameter name="file_path">README.md</parameter></invoke></function_calls>"#,
            &tools(),
        );
        let HostToolParseResult::Calls(calls) = result else {
            panic!("expected calls, got {result:?}");
        };

        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].input, json!({"file_path":"README.md"}));
    }
}
