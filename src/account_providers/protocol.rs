use a3s_code_core::llm::{
    ContentBlock, LlmResponse, LlmResponseMeta, Message, StreamEvent, TokenUsage,
};
use serde::Deserialize;
use serde_json::{json, Value};
use std::{collections::BTreeMap, time::Instant};
use tokio::sync::mpsc;

#[derive(Clone)]
pub(crate) struct StreamMeta {
    pub provider: &'static str,
    pub request_model: String,
    pub request_url: String,
    pub started_at: Instant,
}

pub(crate) struct AnthropicEventMapper {
    meta: StreamMeta,
    content_blocks: Vec<ContentBlock>,
    text_content: String,
    active_tools: BTreeMap<usize, ActiveTool>,
    usage: TokenUsage,
    stop_reason: Option<String>,
    response_id: Option<String>,
    response_model: Option<String>,
    response_object: Option<String>,
    first_token_ms: Option<u64>,
}

struct ActiveTool {
    id: String,
    name: String,
    input: String,
}

impl AnthropicEventMapper {
    pub fn new(meta: StreamMeta) -> Self {
        Self {
            meta,
            content_blocks: Vec::new(),
            text_content: String::new(),
            active_tools: BTreeMap::new(),
            usage: TokenUsage::default(),
            stop_reason: None,
            response_id: None,
            response_model: None,
            response_object: Some("message".to_string()),
            first_token_ms: None,
        }
    }

    pub async fn handle(
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
            AnthropicStreamEvent::ContentBlockStart {
                index,
                content_block,
            } => match content_block {
                AnthropicContentBlock::Text { text } => {
                    let _ = text;
                }
                AnthropicContentBlock::ToolUse { id, name, input } => {
                    if !self.text_content.is_empty() {
                        self.content_blocks.push(ContentBlock::Text {
                            text: std::mem::take(&mut self.text_content),
                        });
                    }
                    let initial_input = initial_tool_input_json(&input).unwrap_or_default();
                    self.active_tools.insert(
                        index,
                        ActiveTool {
                            id: id.clone(),
                            name: name.clone(),
                            input: initial_input.clone(),
                        },
                    );
                    let _ = tx
                        .send(StreamEvent::ToolUseStart {
                            id: id.clone(),
                            name,
                        })
                        .await;
                    if !initial_input.is_empty() {
                        self.mark_first_token();
                        let _ = tx
                            .send(StreamEvent::ToolUseInputDelta {
                                id: Some(id),
                                delta: initial_input,
                            })
                            .await;
                    }
                }
            },
            AnthropicStreamEvent::ContentBlockDelta { index, delta } => match delta {
                AnthropicDelta::TextDelta { text } => {
                    self.mark_first_token();
                    self.text_content.push_str(&text);
                    let _ = tx.send(StreamEvent::TextDelta(text)).await;
                }
                AnthropicDelta::InputJsonDelta { partial_json } => {
                    self.mark_first_token();
                    let id = self.active_tools.get_mut(&index).map(|tool| {
                        tool.input.push_str(&partial_json);
                        tool.id.clone()
                    });
                    let _ = tx
                        .send(StreamEvent::ToolUseInputDelta {
                            id,
                            delta: partial_json,
                        })
                        .await;
                }
            },
            AnthropicStreamEvent::ContentBlockStop { index } => {
                let Some(tool) = self.active_tools.remove(&index) else {
                    return false;
                };
                let input = parse_tool_input(&tool.input);
                self.content_blocks.push(ContentBlock::ToolUse {
                    id: tool.id,
                    name: tool.name,
                    input,
                });
            }
            AnthropicStreamEvent::MessageDelta {
                delta,
                usage: msg_usage,
            } => {
                self.stop_reason = Some(delta.stop_reason);
                if let Some(input_tokens) = msg_usage.input_tokens.filter(|tokens| *tokens > 0) {
                    self.usage.prompt_tokens = input_tokens;
                }
                if let Some(cache_read_tokens) = msg_usage
                    .cache_read_input_tokens
                    .filter(|tokens| *tokens > 0)
                {
                    self.usage.cache_read_tokens = Some(cache_read_tokens);
                }
                if let Some(cache_write_tokens) = msg_usage
                    .cache_creation_input_tokens
                    .filter(|tokens| *tokens > 0)
                {
                    self.usage.cache_write_tokens = Some(cache_write_tokens);
                }
                self.usage.completion_tokens = msg_usage.output_tokens;
                self.usage.total_tokens = self.usage.prompt_tokens + self.usage.completion_tokens;
            }
            AnthropicStreamEvent::MessageStop => {
                if !self.text_content.is_empty() {
                    self.content_blocks.push(ContentBlock::Text {
                        text: std::mem::take(&mut self.text_content),
                    });
                }
                let response = LlmResponse {
                    message: Message {
                        role: "assistant".to_string(),
                        content: std::mem::take(&mut self.content_blocks),
                        reasoning_content: None,
                    },
                    usage: self.usage.clone(),
                    stop_reason: self.stop_reason.clone(),
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
                return true;
            }
            AnthropicStreamEvent::Ping => {}
            AnthropicStreamEvent::Error => return true,
        }
        false
    }

    fn mark_first_token(&mut self) {
        if self.first_token_ms.is_none() {
            self.first_token_ms = Some(self.meta.started_at.elapsed().as_millis() as u64);
        }
    }
}

pub(crate) fn parse_sse_data(data: &str) -> Option<AnthropicStreamEvent> {
    if data == "[DONE]" {
        return None;
    }
    serde_json::from_str(data).ok()
}

pub(crate) fn parse_account_cli_stream_event(line: &str) -> Option<AnthropicStreamEvent> {
    let value = serde_json::from_str::<Value>(line).ok()?;
    if value.get("type").and_then(Value::as_str) != Some("stream_event") {
        return None;
    }
    serde_json::from_value(value.get("event")?.clone()).ok()
}

fn initial_tool_input_json(input: &Value) -> Option<String> {
    match input {
        Value::Object(map) if map.is_empty() => None,
        Value::Null => None,
        value => serde_json::to_string(value).ok(),
    }
}

fn parse_tool_input(input: &str) -> Value {
    if input.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(input).unwrap_or_else(|error| {
        json!({
            "__parse_error": format!(
                "Malformed tool arguments: {error}. Raw input: {input}"
            )
        })
    })
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AnthropicContentBlock {
    #[serde(rename = "text")]
    Text { text: String },
    #[serde(rename = "tool_use")]
    ToolUse {
        id: String,
        name: String,
        input: Value,
    },
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicUsage {
    pub(crate) input_tokens: usize,
    #[serde(rename = "output_tokens")]
    _output_tokens: usize,
    #[serde(default)]
    pub(crate) cache_read_input_tokens: Option<usize>,
    #[serde(default)]
    pub(crate) cache_creation_input_tokens: Option<usize>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AnthropicStreamEvent {
    #[serde(rename = "message_start")]
    MessageStart { message: AnthropicMessageStart },
    #[serde(rename = "content_block_start")]
    ContentBlockStart {
        index: usize,
        content_block: AnthropicContentBlock,
    },
    #[serde(rename = "content_block_delta")]
    ContentBlockDelta { index: usize, delta: AnthropicDelta },
    #[serde(rename = "content_block_stop")]
    ContentBlockStop { index: usize },
    #[serde(rename = "message_delta")]
    MessageDelta {
        delta: AnthropicMessageDeltaData,
        usage: AnthropicOutputUsage,
    },
    #[serde(rename = "message_stop")]
    MessageStop,
    #[serde(rename = "ping")]
    Ping,
    #[serde(rename = "error")]
    Error,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicMessageStart {
    #[serde(default)]
    pub(crate) id: Option<String>,
    #[serde(default)]
    pub(crate) model: Option<String>,
    #[serde(rename = "type", default)]
    pub(crate) message_type: Option<String>,
    pub(crate) usage: AnthropicUsage,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub(crate) enum AnthropicDelta {
    #[serde(rename = "text_delta")]
    TextDelta { text: String },
    #[serde(rename = "input_json_delta")]
    InputJsonDelta { partial_json: String },
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicMessageDeltaData {
    pub(crate) stop_reason: String,
}

#[derive(Debug, Deserialize)]
pub(crate) struct AnthropicOutputUsage {
    pub(crate) output_tokens: usize,
    #[serde(default)]
    pub(crate) input_tokens: Option<usize>,
    #[serde(default)]
    pub(crate) cache_read_input_tokens: Option<usize>,
    #[serde(default)]
    pub(crate) cache_creation_input_tokens: Option<usize>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_account_cli_stream_event_lines() {
        let event = parse_account_cli_stream_event(
            r#"{"type":"stream_event","event":{"type":"content_block_delta","index":0,"delta":{"type":"text_delta","text":"hi"}}}"#,
        )
        .unwrap();

        assert!(matches!(
            event,
            AnthropicStreamEvent::ContentBlockDelta {
                index: 0,
                delta: AnthropicDelta::TextDelta { text }
            } if text == "hi"
        ));
        assert!(parse_account_cli_stream_event(r#"{"type":"system"}"#).is_none());
    }

    #[tokio::test]
    async fn maps_interleaved_tool_deltas_to_their_call_ids() {
        let mut mapper = AnthropicEventMapper::new(StreamMeta {
            provider: "test",
            request_model: "claude-test".into(),
            request_url: "test://messages".into(),
            started_at: Instant::now(),
        });
        let (tx, mut rx) = mpsc::channel(16);
        let events = [
            r#"{"type":"content_block_start","index":0,"content_block":{"type":"tool_use","id":"tool_a","name":"read","input":{}}}"#,
            r#"{"type":"content_block_start","index":1,"content_block":{"type":"tool_use","id":"tool_b","name":"search","input":{}}}"#,
            r#"{"type":"content_block_delta","index":1,"delta":{"type":"input_json_delta","partial_json":"{\"query\":\"beta\"}"}}"#,
            r#"{"type":"content_block_delta","index":0,"delta":{"type":"input_json_delta","partial_json":"{\"path\":\"alpha\"}"}}"#,
            r#"{"type":"content_block_stop","index":0}"#,
            r#"{"type":"content_block_stop","index":1}"#,
            r#"{"type":"message_stop"}"#,
        ];

        for event in events {
            let event = parse_sse_data(event).expect("valid Anthropic event");
            if mapper.handle(event, &tx).await {
                break;
            }
        }

        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::ToolUseStart { id, name })
                if id == "tool_a" && name == "read"
        ));
        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::ToolUseStart { id, name })
                if id == "tool_b" && name == "search"
        ));
        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::ToolUseInputDelta { id: Some(id), delta })
                if id == "tool_b" && delta == r#"{"query":"beta"}"#
        ));
        assert!(matches!(
            rx.recv().await,
            Some(StreamEvent::ToolUseInputDelta { id: Some(id), delta })
                if id == "tool_a" && delta == r#"{"path":"alpha"}"#
        ));
        let Some(StreamEvent::Done(response)) = rx.recv().await else {
            panic!("expected done event");
        };
        let calls = response.tool_calls();
        assert_eq!(calls.len(), 2);
        assert_eq!(calls[0].id, "tool_a");
        assert_eq!(calls[0].args, json!({"path": "alpha"}));
        assert_eq!(calls[1].id, "tool_b");
        assert_eq!(calls[1].args, json!({"query": "beta"}));
    }
}
