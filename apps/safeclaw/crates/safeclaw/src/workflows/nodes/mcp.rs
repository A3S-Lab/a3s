//! `"mcp"` node — call a tool on a Model Context Protocol (MCP) server.
//!
//! Connects to an MCP server, calls a named tool with rendered arguments, and
//! returns the tool's output. Supports two transport protocols:
//!
//! - **`"sse"`** (default) — HTTP+SSE transport (MCP spec 2024-11-05), widely deployed
//! - **`"streamable-http"`** — Streamable HTTP transport (MCP spec 2025-03-26)
//!
//! # Config schema
//!
//! ```json
//! {
//!   "transport":  "sse",
//!   "server_url": "http://localhost:3000",
//!   "tool_name":  "read_file",
//!   "arguments":  { "path": "{{ start.file_path }}" }
//! }
//! ```
//!
//! | Field | Type | Required | Description |
//! |-------|------|:--------:|-------------|
//! | `transport` | string | — | `"sse"` (default) or `"streamable-http"` |
//! | `server_url` | string | ✅ | Base URL of the MCP server |
//! | `tool_name` | string | ✅ | Name of the tool to call |
//! | `arguments` | object | — | Arguments; string values rendered as Jinja2 templates |
//!
//! ## Template context
//!
//! Same as the `"assign"` node: all global `variables` plus all upstream node
//! outputs keyed by node ID. Upstream inputs shadow variables with the same key.
//!
//! # Output schema
//!
//! ```json
//! { "text": "...", "content": [...], "is_error": false }
//! ```
//!
//! | Field | Description |
//! |-------|-------------|
//! | `text` | Concatenated text from all `"text"`-type content items |
//! | `content` | Full MCP content array as returned by the server |
//! | `is_error` | Whether the server reported a tool-level error |
//!
//! # Example — extract a document via a local MCP server
//!
//! ```json
//! {
//!   "id": "extract",
//!   "type": "mcp",
//!   "data": {
//!     "transport":  "sse",
//!     "server_url": "http://localhost:3001",
//!     "tool_name":  "extract_document",
//!     "arguments":  { "path": "{{ start.file_path }}", "format": "markdown" }
//!   }
//! }
//! ```

use async_trait::async_trait;
use futures_util::StreamExt;
use serde_json::{json, Value};
use std::collections::HashMap;

use a3s_flow::error::{FlowError, Result};
use a3s_flow::node::{ExecContext, Node};

const CLIENT_NAME: &str = "a3s-flow-mcp";
const CLIENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const PROTOCOL_SSE: &str = "2024-11-05";
const PROTOCOL_HTTP: &str = "2025-03-26";

// ── Jinja2 helpers (copied from a3s-flow for independence) ────────────────

/// Build a Jinja2 context from the execution context.
fn build_jinja_context(ctx: &ExecContext) -> HashMap<String, Value> {
    let mut map: HashMap<String, Value> = ctx.variables.clone();
    for (k, v) in &ctx.inputs {
        map.insert(k.clone(), v.clone());
    }
    map
}

/// Render a Jinja2 template string against the given context map.
fn render(template: &str, context: &HashMap<String, Value>) -> Result<String> {
    let env = minijinja::Environment::new();
    env.render_str(template, context)
        .map_err(|e| FlowError::Internal(format!("mcp: template render error: {e}")))
}

// ──────────────────────────────────────────────────────────────────────────

/// MCP node — call a tool on a Model Context Protocol server.
pub struct McpNode;

#[async_trait]
impl Node for McpNode {
    fn node_type(&self) -> &str {
        "mcp"
    }

    async fn execute(&self, ctx: ExecContext) -> Result<Value> {
        let transport = ctx.data["transport"].as_str().unwrap_or("sse");

        let server_url = ctx.data["server_url"]
            .as_str()
            .ok_or_else(|| FlowError::InvalidDefinition("mcp: missing data.server_url".into()))?
            .trim_end_matches('/')
            .to_string();

        let tool_name = ctx.data["tool_name"]
            .as_str()
            .ok_or_else(|| FlowError::InvalidDefinition("mcp: missing data.tool_name".into()))?
            .to_string();

        let arguments = render_arguments(&ctx)?;

        match transport {
            "sse" => call_via_sse(&server_url, &tool_name, arguments).await,
            "streamable-http" => call_via_streamable_http(&server_url, &tool_name, arguments).await,
            other => Err(FlowError::InvalidDefinition(format!(
                "mcp: unknown transport '{other}', expected 'sse' or 'streamable-http'"
            ))),
        }
    }
}

// ── Argument template rendering ────────────────────────────────────────────

fn render_arguments(ctx: &ExecContext) -> Result<Value> {
    let raw = match ctx.data.get("arguments") {
        None | Some(Value::Null) => return Ok(json!({})),
        Some(v) if v.is_object() => v.clone(),
        Some(_) => {
            return Err(FlowError::InvalidDefinition(
                "mcp: data.arguments must be an object".into(),
            ))
        }
    };

    let jinja_ctx = build_jinja_context(ctx);
    let mut out = serde_json::Map::new();

    for (key, val) in raw.as_object().unwrap() {
        let rendered =
            if let Some(tmpl) = val.as_str() {
                Value::String(render(tmpl, &jinja_ctx).map_err(|e| {
                    FlowError::InvalidDefinition(format!("mcp: argument '{key}': {e}"))
                })?)
            } else {
                val.clone()
            };
        out.insert(key.clone(), rendered);
    }

    Ok(Value::Object(out))
}

// ── Streamable HTTP transport (MCP 2025-03-26) ────────────────────────────

async fn call_via_streamable_http(
    server_url: &str,
    tool_name: &str,
    arguments: Value,
) -> Result<Value> {
    let client = reqwest::Client::new();

    // 1. Initialize
    let init_resp = client
        .post(server_url)
        .header("Content-Type", "application/json")
        .header("Accept", "application/json, text/event-stream")
        .json(&rpc_request(
            "initialize",
            1,
            json!({
                "protocolVersion": PROTOCOL_HTTP,
                "capabilities": {},
                "clientInfo": { "name": CLIENT_NAME, "version": CLIENT_VERSION },
            }),
        ))
        .send()
        .await
        .map_err(|e| FlowError::Internal(format!("mcp: initialize failed: {e}")))?;

    let session_id = init_resp
        .headers()
        .get("Mcp-Session-Id")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string());

    let init_body: Value = init_resp
        .json()
        .await
        .map_err(|e| FlowError::Internal(format!("mcp: initialize response: {e}")))?;

    if init_body.get("error").is_some() {
        return Err(FlowError::Internal(format!(
            "mcp: initialize error: {}",
            init_body["error"]
        )));
    }

    // 2. notifications/initialized (fire-and-forget)
    let _ = build_post(&client, server_url, session_id.as_deref())
        .json(&rpc_notification("notifications/initialized"))
        .send()
        .await;

    // 3. tools/call
    let tool_resp = build_post(&client, server_url, session_id.as_deref())
        .header("Accept", "application/json, text/event-stream")
        .json(&rpc_request(
            "tools/call",
            2,
            json!({ "name": tool_name, "arguments": arguments }),
        ))
        .send()
        .await
        .map_err(|e| FlowError::Internal(format!("mcp: tools/call failed: {e}")))?;

    let is_sse = tool_resp
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .contains("text/event-stream");

    let body: Value = if is_sse {
        read_sse_rpc_result(tool_resp, 2).await?
    } else {
        tool_resp
            .json()
            .await
            .map_err(|e| FlowError::Internal(format!("mcp: tools/call response: {e}")))?
    };

    extract_tool_result(&body)
}

fn build_post(
    client: &reqwest::Client,
    url: &str,
    session_id: Option<&str>,
) -> reqwest::RequestBuilder {
    let mut req = client.post(url).header("Content-Type", "application/json");
    if let Some(id) = session_id {
        req = req.header("Mcp-Session-Id", id);
    }
    req
}

// ── SSE transport (MCP 2024-11-05) ────────────────────────────────────────

async fn call_via_sse(server_url: &str, tool_name: &str, arguments: Value) -> Result<Value> {
    let client = reqwest::Client::new();

    // 1. Open SSE connection
    let sse_resp = client
        .get(format!("{server_url}/sse"))
        .header("Accept", "text/event-stream")
        .send()
        .await
        .map_err(|e| FlowError::Internal(format!("mcp: SSE connect failed: {e}")))?;

    if !sse_resp.status().is_success() {
        return Err(FlowError::Internal(format!(
            "mcp: SSE connect returned {}",
            sse_resp.status()
        )));
    }

    let mut stream = sse_resp.bytes_stream();
    let mut buf = SseBuffer::new();

    // 2. Read 'endpoint' event to get the POST URL
    let endpoint_event = read_event_type(&mut stream, &mut buf, "endpoint").await?;
    let post_url = resolve_url(server_url, endpoint_event.data.trim());

    // 3. Initialize
    client
        .post(&post_url)
        .json(&rpc_request(
            "initialize",
            1,
            json!({
                "protocolVersion": PROTOCOL_SSE,
                "capabilities": {},
                "clientInfo": { "name": CLIENT_NAME, "version": CLIENT_VERSION },
            }),
        ))
        .send()
        .await
        .map_err(|e| FlowError::Internal(format!("mcp: initialize failed: {e}")))?;

    wait_for_response_id(&mut stream, &mut buf, 1).await?;

    // 4. notifications/initialized (fire-and-forget)
    let _ = client
        .post(&post_url)
        .json(&rpc_notification("notifications/initialized"))
        .send()
        .await;

    // 5. tools/call
    client
        .post(&post_url)
        .json(&rpc_request(
            "tools/call",
            2,
            json!({ "name": tool_name, "arguments": arguments }),
        ))
        .send()
        .await
        .map_err(|e| FlowError::Internal(format!("mcp: tools/call failed: {e}")))?;

    let response = wait_for_response_id(&mut stream, &mut buf, 2).await?;
    extract_tool_result(&response)
}

// ── SSE helpers ────────────────────────────────────────────────────────────

/// Read SSE events until one with the matching `event:` type is found.
async fn read_event_type<S>(
    stream: &mut S,
    buf: &mut SseBuffer,
    event_type: &str,
) -> Result<SseEvent>
where
    S: futures_util::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    loop {
        while let Some(ev) = buf.next_event() {
            if ev.event_type == event_type {
                return Ok(ev);
            }
        }
        let chunk = stream
            .next()
            .await
            .ok_or_else(|| FlowError::Internal("mcp: SSE stream closed unexpectedly".into()))?
            .map_err(|e| FlowError::Internal(format!("mcp: SSE read error: {e}")))?;
        buf.push(&chunk);
    }
}

/// Read SSE events until a JSON-RPC message with the given `id` arrives.
async fn wait_for_response_id<S>(stream: &mut S, buf: &mut SseBuffer, id: u64) -> Result<Value>
where
    S: futures_util::Stream<Item = std::result::Result<bytes::Bytes, reqwest::Error>> + Unpin,
{
    loop {
        while let Some(ev) = buf.next_event() {
            if ev.event_type == "message" || ev.event_type.is_empty() {
                if let Ok(msg) = serde_json::from_str::<Value>(&ev.data) {
                    if msg["id"].as_u64() == Some(id) {
                        if let Some(err) = msg.get("error") {
                            return Err(FlowError::Internal(format!("mcp: JSON-RPC error: {err}")));
                        }
                        return Ok(msg);
                    }
                }
            }
        }
        let chunk = stream
            .next()
            .await
            .ok_or_else(|| {
                FlowError::Internal(format!(
                    "mcp: SSE stream closed waiting for response id={id}"
                ))
            })?
            .map_err(|e| FlowError::Internal(format!("mcp: SSE read error: {e}")))?;
        buf.push(&chunk);
    }
}

/// Read a streaming HTTP response as SSE until a JSON-RPC result/error is found.
async fn read_sse_rpc_result(response: reqwest::Response, id: u64) -> Result<Value> {
    let mut buf = SseBuffer::new();
    let mut stream = response.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| FlowError::Internal(format!("mcp: SSE read error: {e}")))?;
        buf.push(&chunk);

        while let Some(ev) = buf.next_event() {
            if ev.event_type == "message" || ev.event_type.is_empty() {
                if let Ok(val) = serde_json::from_str::<Value>(&ev.data) {
                    if val["id"].as_u64() == Some(id) {
                        return Ok(val);
                    }
                }
            }
        }
    }

    Err(FlowError::Internal(
        "mcp: SSE stream closed without a result".into(),
    ))
}

// ── SSE buffer / parser ────────────────────────────────────────────────────

struct SseBuffer {
    buf: Vec<u8>,
}

#[derive(Default, Debug)]
struct SseEvent {
    event_type: String,
    data: String,
}

impl SseBuffer {
    fn new() -> Self {
        Self { buf: Vec::new() }
    }

    fn push(&mut self, chunk: &[u8]) {
        self.buf.extend_from_slice(chunk);
    }

    /// Return the next complete SSE event if one is buffered.
    fn next_event(&mut self) -> Option<SseEvent> {
        // Events are delimited by a blank line (\n\n).
        let pos = self.buf.windows(2).position(|w| w == b"\n\n")?;

        let event_bytes = self.buf[..pos].to_vec();
        self.buf = self.buf[pos + 2..].to_vec();

        parse_sse_event(&event_bytes)
    }
}

fn parse_sse_event(raw: &[u8]) -> Option<SseEvent> {
    let text = std::str::from_utf8(raw).ok()?;
    let mut ev = SseEvent::default();

    for line in text.lines() {
        let line = line.trim_end_matches('\r');
        if let Some(val) = line.strip_prefix("event:") {
            ev.event_type = val.trim().to_string();
        } else if let Some(val) = line.strip_prefix("data:") {
            if !ev.data.is_empty() {
                ev.data.push('\n');
            }
            ev.data.push_str(val.trim());
        }
    }

    if ev.data.is_empty() && ev.event_type.is_empty() {
        return None;
    }
    Some(ev)
}

// ── JSON-RPC helpers ───────────────────────────────────────────────────────

fn rpc_request(method: &str, id: u64, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

fn rpc_notification(method: &str) -> Value {
    json!({ "jsonrpc": "2.0", "method": method })
}

/// Resolve a relative path against the server base URL.
fn resolve_url(base: &str, path: &str) -> String {
    if path.starts_with("http://") || path.starts_with("https://") {
        path.to_string()
    } else {
        format!("{base}{path}")
    }
}

// ── MCP result extraction ──────────────────────────────────────────────────

/// Extract `{ text, content, is_error }` from a JSON-RPC tool result message.
fn extract_tool_result(body: &Value) -> Result<Value> {
    if let Some(err) = body.get("error") {
        return Err(FlowError::Internal(format!("mcp: server error: {err}")));
    }

    let result = &body["result"];
    let is_error = result["isError"].as_bool().unwrap_or(false);
    let content = result["content"].as_array().cloned().unwrap_or_default();

    let text = content
        .iter()
        .filter_map(|item| {
            if item["type"].as_str() == Some("text") {
                item["text"].as_str().map(str::to_string)
            } else {
                None
            }
        })
        .collect::<Vec<_>>()
        .join("\n");

    Ok(json!({ "text": text, "content": content, "is_error": is_error }))
}

// ── Tests ──────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn ctx(data: Value) -> ExecContext {
        ExecContext {
            data,
            ..Default::default()
        }
    }

    // ── Config validation ──────────────────────────────────────────────────

    #[tokio::test]
    async fn rejects_missing_server_url() {
        let err = McpNode
            .execute(ctx(json!({ "tool_name": "foo" })))
            .await
            .unwrap_err();
        assert!(matches!(err, FlowError::InvalidDefinition(_)));
    }

    #[tokio::test]
    async fn rejects_missing_tool_name() {
        let err = McpNode
            .execute(ctx(json!({ "server_url": "http://localhost:3000" })))
            .await
            .unwrap_err();
        assert!(matches!(err, FlowError::InvalidDefinition(_)));
    }

    #[tokio::test]
    async fn rejects_unknown_transport() {
        let err = McpNode
            .execute(ctx(json!({
                "transport":  "grpc",
                "server_url": "http://localhost:3000",
                "tool_name":  "foo",
            })))
            .await
            .unwrap_err();
        assert!(matches!(err, FlowError::InvalidDefinition(_)));
    }

    #[tokio::test]
    async fn rejects_non_object_arguments() {
        let err = McpNode
            .execute(ctx(json!({
                "server_url": "http://localhost:3000",
                "tool_name":  "foo",
                "arguments":  "not an object",
            })))
            .await
            .unwrap_err();
        assert!(matches!(err, FlowError::InvalidDefinition(_)));
    }

    // ── Argument rendering ─────────────────────────────────────────────────

    #[test]
    fn renders_string_arguments_as_jinja2() {
        let ctx = ExecContext {
            data: json!({
                "server_url": "http://localhost",
                "tool_name":  "foo",
                "arguments":  { "path": "{{ file }}", "n": 42 },
            }),
            variables: HashMap::from([("file".to_string(), json!("/tmp/doc.pdf"))]),
            ..Default::default()
        };
        let args = render_arguments(&ctx).unwrap();
        assert_eq!(args["path"], json!("/tmp/doc.pdf"));
        assert_eq!(args["n"], json!(42)); // non-string: passed as-is
    }

    #[test]
    fn null_arguments_returns_empty_object() {
        let ctx = ExecContext {
            data: json!({ "server_url": "http://x", "tool_name": "t" }),
            ..Default::default()
        };
        assert_eq!(render_arguments(&ctx).unwrap(), json!({}));
    }

    // ── SSE buffer ─────────────────────────────────────────────────────────

    #[test]
    fn sse_buffer_parses_endpoint_event() {
        let mut buf = SseBuffer::new();
        buf.push(b"event: endpoint\ndata: /message?sessionId=abc\n\n");
        let ev = buf.next_event().unwrap();
        assert_eq!(ev.event_type, "endpoint");
        assert_eq!(ev.data, "/message?sessionId=abc");
    }

    #[test]
    fn sse_buffer_parses_message_event() {
        let mut buf = SseBuffer::new();
        buf.push(b"event: message\ndata: {\"jsonrpc\":\"2.0\",\"id\":1,\"result\":{}}\n\n");
        let ev = buf.next_event().unwrap();
        assert_eq!(ev.event_type, "message");
        assert!(ev.data.contains("jsonrpc"));
    }

    #[test]
    fn sse_buffer_handles_chunked_delivery() {
        let mut buf = SseBuffer::new();
        buf.push(b"event: endpo");
        assert!(buf.next_event().is_none());
        buf.push(b"int\ndata: /msg\n\n");
        let ev = buf.next_event().unwrap();
        assert_eq!(ev.event_type, "endpoint");
        assert_eq!(ev.data, "/msg");
    }

    #[test]
    fn sse_buffer_parses_multiple_consecutive_events() {
        let mut buf = SseBuffer::new();
        buf.push(b"event: endpoint\ndata: /msg\n\nevent: message\ndata: {\"id\":1}\n\n");
        let e1 = buf.next_event().unwrap();
        let e2 = buf.next_event().unwrap();
        assert_eq!(e1.event_type, "endpoint");
        assert_eq!(e2.event_type, "message");
    }

    #[test]
    fn sse_buffer_empty_returns_none_before_boundary() {
        let mut buf = SseBuffer::new();
        buf.push(b"event: message\ndata: hello");
        assert!(buf.next_event().is_none()); // no \n\n yet
    }

    // ── URL resolution ─────────────────────────────────────────────────────

    #[test]
    fn resolve_absolute_url_passthrough() {
        assert_eq!(
            resolve_url("http://base", "http://other/path"),
            "http://other/path"
        );
    }

    #[test]
    fn resolve_relative_path_joins_base() {
        assert_eq!(
            resolve_url("http://base", "/message?sessionId=abc"),
            "http://base/message?sessionId=abc"
        );
    }

    // ── Result extraction ──────────────────────────────────────────────────

    #[test]
    fn extract_concatenates_text_content() {
        let body = json!({
            "result": {
                "content": [
                    { "type": "text", "text": "hello" },
                    { "type": "text", "text": "world" },
                ],
                "isError": false,
            }
        });
        let out = extract_tool_result(&body).unwrap();
        assert_eq!(out["text"], json!("hello\nworld"));
        assert_eq!(out["is_error"], json!(false));
        assert_eq!(out["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn extract_skips_non_text_content() {
        let body = json!({
            "result": {
                "content": [
                    { "type": "image", "data": "base64abc", "mimeType": "image/png" },
                    { "type": "text", "text": "caption" },
                ],
                "isError": false,
            }
        });
        let out = extract_tool_result(&body).unwrap();
        assert_eq!(out["text"], json!("caption"));
        assert_eq!(out["content"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn extract_propagates_json_rpc_error() {
        let body = json!({ "error": { "code": -32601, "message": "method not found" } });
        let err = extract_tool_result(&body).unwrap_err();
        assert!(matches!(err, FlowError::Internal(_)));
    }

    #[test]
    fn extract_empty_content_gives_empty_text() {
        let body = json!({ "result": { "content": [], "isError": false } });
        let out = extract_tool_result(&body).unwrap();
        assert_eq!(out["text"], json!(""));
    }
}
