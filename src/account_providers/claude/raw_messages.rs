use super::super::protocol::{parse_sse_data, AnthropicEventMapper, StreamMeta};
use super::model::canonical_model_name;
use a3s_code_core::llm::{HttpClient, Message, StreamEvent, ToolDefinition};
use futures::StreamExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::error::Error;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const ANTHROPIC_BASE: &str = "https://api.anthropic.com";
const ANTHROPIC_VERSION: &str = "2023-06-01";
const ANTHROPIC_BETA: &str = "oauth-2025-04-20";
const DEFAULT_MAX_TOKENS: usize = 8192;

pub(crate) struct RawMessagesClient {
    access_token: String,
    model: String,
    base_url: String,
    max_tokens: usize,
    http: Arc<dyn HttpClient>,
}

impl RawMessagesClient {
    pub(crate) fn new(access_token: String, model: &str, http: Arc<dyn HttpClient>) -> Self {
        Self {
            access_token,
            model: canonical_model_name(model),
            base_url: ANTHROPIC_BASE.to_string(),
            max_tokens: DEFAULT_MAX_TOKENS,
            http,
        }
    }

    pub(crate) async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> std::result::Result<mpsc::Receiver<StreamEvent>, RawMessagesError> {
        let request_started_at = Instant::now();
        let request_body = self.build_request(messages, system, tools, true);
        let url = format!("{}/v1/messages", self.base_url);
        let header_values = self.headers();
        let headers = header_values
            .iter()
            .map(|(key, value)| (*key, value.as_str()))
            .collect::<Vec<_>>();

        let response = self
            .http
            .post_streaming(&url, headers, &request_body, cancel_token)
            .await
            .map_err(RawMessagesError::terminal)?;
        if !(200..300).contains(&response.status) {
            let message =
                format_api_error(&url, &self.model, response.status, &response.error_body);
            return Err(RawMessagesError {
                message,
                use_cli_fallback: should_fallback_to_claude_cli(
                    response.status,
                    &response.error_body,
                ),
            });
        }

        let (tx, rx) = mpsc::channel(100);
        let mut stream = response.byte_stream;
        let mut mapper = AnthropicEventMapper::new(StreamMeta {
            provider: "claude-code",
            request_model: self.model.clone(),
            request_url: url,
            started_at: request_started_at,
        });

        tokio::spawn(async move {
            let mut buffer = String::new();
            while let Some(chunk_result) = stream.next().await {
                let Ok(chunk) = chunk_result else {
                    break;
                };
                buffer.push_str(&String::from_utf8_lossy(&chunk));

                while let Some(event_end) = buffer.find("\n\n") {
                    let event_data: String = buffer.drain(..event_end).collect();
                    buffer.drain(..2);

                    for line in event_data.lines() {
                        let Some(data) = line.strip_prefix("data: ") else {
                            continue;
                        };
                        let Some(event) = parse_sse_data(data) else {
                            continue;
                        };
                        if mapper.handle(event, &tx).await {
                            return;
                        }
                    }
                }
            }
        });

        Ok(rx)
    }

    fn build_request(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        stream: bool,
    ) -> Value {
        let model = canonical_model_name(&self.model);
        let mut request = json!({
            "model": model,
            "max_tokens": self.max_tokens,
            "messages": messages,
            "stream": stream,
        });

        if let Some(system) = system {
            request["system"] = json!([
                {
                    "type": "text",
                    "text": system,
                    "cache_control": { "type": "ephemeral" }
                }
            ]);
        }

        if !tools.is_empty() {
            let mut tool_defs = tools
                .iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "input_schema": tool.parameters,
                    })
                })
                .collect::<Vec<_>>();
            if let Some(last) = tool_defs.last_mut() {
                last["cache_control"] = json!({ "type": "ephemeral" });
            }
            request["tools"] = json!(tool_defs);
        }

        request
    }

    fn headers(&self) -> Vec<(&'static str, String)> {
        vec![
            ("Authorization", format!("Bearer {}", self.access_token)),
            ("anthropic-version", ANTHROPIC_VERSION.to_string()),
            ("anthropic-beta", ANTHROPIC_BETA.to_string()),
        ]
    }
}

#[derive(Debug)]
pub(crate) struct RawMessagesError {
    message: String,
    use_cli_fallback: bool,
}

impl RawMessagesError {
    pub(crate) fn should_use_cli_fallback(&self) -> bool {
        self.use_cli_fallback
    }

    fn terminal(error: anyhow::Error) -> Self {
        Self {
            message: error.to_string(),
            use_cli_fallback: false,
        }
    }
}

impl fmt::Display for RawMessagesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for RawMessagesError {}

#[derive(Debug, Deserialize)]
struct ClaudeApiErrorEnvelope {
    #[serde(default)]
    error: Option<ClaudeApiError>,
    #[serde(default)]
    request_id: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ClaudeApiError {
    #[serde(rename = "type", default)]
    error_type: Option<String>,
    #[serde(default)]
    message: Option<String>,
}

fn format_api_error(url: &str, model: &str, status: u16, body: &str) -> String {
    let parsed = serde_json::from_str::<ClaudeApiErrorEnvelope>(body).ok();
    let error_type = parsed
        .as_ref()
        .and_then(|error| error.error.as_ref())
        .and_then(|error| error.error_type.as_deref());
    let message = parsed
        .as_ref()
        .and_then(|error| error.error.as_ref())
        .and_then(|error| error.message.as_deref())
        .filter(|message| !message.trim().is_empty())
        .unwrap_or(body);
    let request_id = parsed
        .as_ref()
        .and_then(|error| error.request_id.as_deref())
        .map(|id| format!(", request_id={id}"))
        .unwrap_or_default();

    if status == 429 || error_type == Some("rate_limit_error") {
        return format!(
            "Claude Code OAuth bridge rate-limited for {model} at {url} ({status}{request_id}). \
             The installed `claude` CLI may still work because it uses Claude Code's full first-party client path; \
             this a3s bridge currently uses raw OAuth Messages API auth. Server message: {message}"
        );
    }

    if status == 401 || error_type == Some("authentication_error") {
        return format!(
            "Claude Code OAuth bridge authentication failed for {model} at {url} ({status}{request_id}). \
             The installed `claude` CLI may still work because it can use Claude Code's refreshed local login path; \
             this a3s bridge currently uses raw OAuth Messages API auth. Server message: {message}"
        );
    }

    format!("Claude account API error at {url} ({status}{request_id}): {message}")
}

fn should_fallback_to_claude_cli(status: u16, body: &str) -> bool {
    if matches!(status, 401 | 429) {
        return true;
    }
    serde_json::from_str::<ClaudeApiErrorEnvelope>(body)
        .ok()
        .and_then(|error| error.error)
        .and_then(|error| error.error_type)
        .as_deref()
        .is_some_and(|error_type| matches!(error_type, "authentication_error" | "rate_limit_error"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::llm::{default_http_client, HttpResponse, StreamingHttpResponse};
    use anyhow::Result;
    use async_trait::async_trait;
    use std::sync::Mutex;

    #[test]
    fn builds_bearer_headers_for_claude_account() {
        let client =
            RawMessagesClient::new("token-123".into(), "claude-sonnet-4", default_http_client());

        let headers = client.headers();

        assert!(headers.contains(&("Authorization", "Bearer token-123".to_string())));
        assert!(headers.contains(&("anthropic-version", ANTHROPIC_VERSION.to_string())));
        assert!(headers.contains(&("anthropic-beta", ANTHROPIC_BETA.to_string())));
    }

    #[test]
    fn formats_claude_rate_limit_as_bridge_error() {
        let message = format_api_error(
            "https://api.anthropic.com/v1/messages",
            "claude-opus-4-8",
            429,
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"Error"},"request_id":"req_123"}"#,
        );

        assert!(message.contains("Claude Code OAuth bridge rate-limited"));
        assert!(message.contains("claude-opus-4-8"));
        assert!(message.contains("request_id=req_123"));
        assert!(message.contains("raw OAuth Messages API auth"));
    }

    #[test]
    fn formats_claude_authentication_as_bridge_error() {
        let message = format_api_error(
            "https://api.anthropic.com/v1/messages",
            "claude-opus-4-8",
            401,
            r#"{"type":"error","error":{"type":"authentication_error","message":"Invalid authentication credentials"},"request_id":"req_123"}"#,
        );

        assert!(message.contains("Claude Code OAuth bridge authentication failed"));
        assert!(message.contains("claude-opus-4-8"));
        assert!(message.contains("request_id=req_123"));
        assert!(message.contains("refreshed local login path"));
    }

    #[test]
    fn falls_back_to_claude_cli_for_bridge_auth_or_rate_limit_errors() {
        assert!(should_fallback_to_claude_cli(401, "{}"));
        assert!(should_fallback_to_claude_cli(429, "{}"));
        assert!(should_fallback_to_claude_cli(
            400,
            r#"{"type":"error","error":{"type":"authentication_error","message":"Invalid authentication credentials"}}"#
        ));
        assert!(should_fallback_to_claude_cli(
            400,
            r#"{"type":"error","error":{"type":"rate_limit_error","message":"Error"}}"#
        ));
        assert!(!should_fallback_to_claude_cli(
            404,
            r#"{"type":"error","error":{"type":"not_found_error","message":"missing"}}"#
        ));
    }

    #[test]
    fn builds_messages_request_with_prompt_cache() {
        let client =
            RawMessagesClient::new("token".into(), "claude-sonnet-4[1m]", default_http_client());
        let tools = vec![ToolDefinition {
            name: "read_file".into(),
            description: "Read a file".into(),
            parameters: json!({"type":"object"}),
        }];

        let body = client.build_request(&[Message::user("hello")], Some("system"), &tools, true);

        assert_eq!(body["model"], "claude-sonnet-4");
        assert_eq!(body["messages"][0]["role"], "user");
        assert_eq!(body["system"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["tools"][0]["name"], "read_file");
        assert_eq!(body["tools"][0]["cache_control"]["type"], "ephemeral");
        assert_eq!(body["stream"], true);
    }

    #[tokio::test]
    async fn streaming_maps_anthropic_events_to_a3s_events() {
        let http = Arc::new(MockHttp::new(
            r#"data: {"type":"message_start","message":{"id":"msg_1","type":"message","model":"claude-sonnet-4","usage":{"input_tokens":3,"output_tokens":0,"cache_read_input_tokens":1}}}

data: {"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}

data: {"type":"message_delta","delta":{"stop_reason":"end_turn"},"usage":{"output_tokens":2}}

data: {"type":"message_stop"}

"#,
        ));
        let client = RawMessagesClient::new("token".into(), "claude-sonnet-4", http.clone());

        let mut rx = client
            .complete_streaming(
                &[Message::user("hello")],
                None,
                &[],
                CancellationToken::new(),
            )
            .await
            .unwrap();

        assert!(matches!(rx.recv().await, Some(StreamEvent::TextDelta(text)) if text == "hi"));
        let Some(StreamEvent::Done(response)) = rx.recv().await else {
            panic!("expected done event");
        };
        assert_eq!(response.text(), "hi");
        assert_eq!(response.usage.prompt_tokens, 3);
        assert_eq!(response.usage.completion_tokens, 2);
        assert_eq!(response.usage.total_tokens, 5);
        assert_eq!(response.usage.cache_read_tokens, Some(1));
        assert_eq!(response.usage.cache_write_tokens, None);
        assert_eq!(
            response.meta.unwrap().provider.as_deref(),
            Some("claude-code")
        );

        let headers = http.headers.lock().unwrap().clone();
        assert!(headers.contains(&("Authorization".into(), "Bearer token".into())));
        assert!(headers.contains(&("anthropic-beta".into(), ANTHROPIC_BETA.into())));
    }

    struct MockHttp {
        stream: String,
        headers: Mutex<Vec<(String, String)>>,
    }

    impl MockHttp {
        fn new(stream: &str) -> Self {
            Self {
                stream: stream.to_string(),
                headers: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl HttpClient for MockHttp {
        async fn post(
            &self,
            _url: &str,
            _headers: Vec<(&str, &str)>,
            _body: &Value,
            _cancel_token: CancellationToken,
        ) -> Result<HttpResponse> {
            unreachable!("RawMessagesClient uses streaming")
        }

        async fn post_streaming(
            &self,
            _url: &str,
            headers: Vec<(&str, &str)>,
            _body: &Value,
            _cancel_token: CancellationToken,
        ) -> Result<StreamingHttpResponse> {
            *self.headers.lock().unwrap() = headers
                .into_iter()
                .map(|(key, value)| (key.to_string(), value.to_string()))
                .collect();
            let chunks = vec![Ok(self.stream.clone().into())];
            Ok(StreamingHttpResponse {
                status: 200,
                retry_after: None,
                byte_stream: Box::pin(futures::stream::iter(chunks)),
                error_body: String::new(),
            })
        }
    }
}
