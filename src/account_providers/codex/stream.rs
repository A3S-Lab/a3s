//! Translate Codex Responses wire events into the provider-neutral A3S stream.

use super::transport::{TransportController, TransportError, WireStream};
use a3s_code_core::llm::{
    ContentBlock, LlmResponse, LlmResponseMeta, Message, StreamEvent, TokenUsage,
};
use serde_json::{json, Value};
use tokio::sync::mpsc;

pub(super) fn into_llm_stream(
    mut wire: WireStream,
    transport: TransportController,
    model: String,
    request_url: String,
) -> mpsc::Receiver<StreamEvent> {
    let (tx, rx) = mpsc::channel(128);
    tokio::spawn(async move {
        let mut state = ResponseState::default();
        while let Some(next) = wire.events.recv().await {
            let event = match next {
                Ok(event) => event,
                Err(error) => {
                    transport.note_stream_failure(wire.kind, &error);
                    tracing::warn!(
                        transport = ?wire.kind,
                        error = %error,
                        "Codex response stream failed"
                    );
                    return;
                }
            };

            match event
                .get("type")
                .and_then(Value::as_str)
                .unwrap_or_default()
            {
                "response.created" => {
                    state.response_id = event
                        .pointer("/response/id")
                        .and_then(Value::as_str)
                        .map(str::to_string);
                }
                "response.output_text.delta" => {
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        state.text.push_str(delta);
                        if tx
                            .send(StreamEvent::TextDelta(delta.to_string()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                "response.reasoning_text.delta" | "response.reasoning_summary_text.delta" => {
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        state.reasoning.push_str(delta);
                        if tx
                            .send(StreamEvent::ReasoningDelta(delta.to_string()))
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                "response.output_item.added" => {
                    let item = event.get("item");
                    if item
                        .and_then(|value| value.get("type"))
                        .and_then(Value::as_str)
                        == Some("function_call")
                    {
                        let id = item_str(item, "id");
                        let call_id = item_str(item, "call_id");
                        let name = item_str(item, "name");
                        state
                            .calls
                            .push((id, (call_id.clone(), name.clone(), String::new())));
                        if tx
                            .send(StreamEvent::ToolUseStart { id: call_id, name })
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                "response.function_call_arguments.delta" => {
                    let item_id = event
                        .get("item_id")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    if let Some(delta) = event.get("delta").and_then(Value::as_str) {
                        let call_id = state
                            .calls
                            .iter_mut()
                            .find(|(key, _)| key == item_id)
                            .and_then(|(_, (call_id, _, arguments))| {
                                arguments.push_str(delta);
                                (!call_id.is_empty()).then(|| call_id.clone())
                            });
                        if tx
                            .send(StreamEvent::ToolUseInputDelta {
                                id: call_id,
                                delta: delta.to_string(),
                            })
                            .await
                            .is_err()
                        {
                            return;
                        }
                    }
                }
                "response.output_item.done" => {
                    let item = event.get("item");
                    if item
                        .and_then(|value| value.get("type"))
                        .and_then(Value::as_str)
                        == Some("function_call")
                    {
                        let id = item_str(item, "id");
                        let completed = (
                            item_str(item, "call_id"),
                            item_str(item, "name"),
                            item_str(item, "arguments"),
                        );
                        if let Some((_, existing)) =
                            state.calls.iter_mut().find(|(key, _)| *key == id)
                        {
                            *existing = completed;
                        } else {
                            state.calls.push((id, completed));
                        }
                    }
                }
                "response.completed" => {
                    state.capture_usage(event.pointer("/response/usage"));
                    let response = state.finish(model, request_url);
                    let _ = tx.send(StreamEvent::Done(response)).await;
                    return;
                }
                "response.failed" | "error" => {
                    let error = TransportError::protocol(response_error_message(&event));
                    transport.note_stream_failure(wire.kind, &error);
                    tracing::warn!(
                        transport = ?wire.kind,
                        error = %error,
                        "Codex backend reported a failed response"
                    );
                    return;
                }
                _ => {}
            }
        }

        let error =
            TransportError::stream_closed("Codex event stream ended before response.completed");
        transport.note_stream_failure(wire.kind, &error);
    });
    rx
}

#[derive(Default)]
struct ResponseState {
    text: String,
    reasoning: String,
    response_id: Option<String>,
    usage: TokenUsage,
    calls: Vec<(String, (String, String, String))>,
}

impl ResponseState {
    fn capture_usage(&mut self, usage: Option<&Value>) {
        let Some(usage) = usage else {
            return;
        };
        self.usage.prompt_tokens = usage
            .get("input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        self.usage.completion_tokens = usage
            .get("output_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        self.usage.total_tokens = usage
            .get("total_tokens")
            .and_then(Value::as_u64)
            .unwrap_or(0) as usize;
        self.usage.cache_read_tokens = usage
            .pointer("/input_tokens_details/cached_tokens")
            .and_then(Value::as_u64)
            .map(|value| value as usize);
    }

    fn finish(mut self, model: String, request_url: String) -> LlmResponse {
        let mut content = Vec::new();
        if !self.text.is_empty() {
            content.push(ContentBlock::Text { text: self.text });
        }
        let has_calls = !self.calls.is_empty();
        for (_, (call_id, name, arguments)) in self.calls {
            content.push(ContentBlock::ToolUse {
                id: call_id,
                name,
                input: parse_args(&arguments),
            });
        }
        LlmResponse {
            message: Message {
                role: "assistant".into(),
                content,
                reasoning_content: (!self.reasoning.is_empty()).then_some(self.reasoning),
            },
            usage: self.usage,
            stop_reason: Some(if has_calls { "tool_calls" } else { "stop" }.into()),
            token_logprobs: Vec::new(),
            meta: Some(LlmResponseMeta {
                provider: Some("codex".into()),
                request_model: Some(model),
                request_url: Some(request_url),
                response_id: self.response_id.take(),
                ..Default::default()
            }),
        }
    }
}

fn item_str(item: Option<&Value>, key: &str) -> String {
    item.and_then(|value| value.get(key))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn parse_args(value: &str) -> Value {
    if value.trim().is_empty() {
        return json!({});
    }
    serde_json::from_str(value).unwrap_or_else(|_| json!({}))
}

fn response_error_message(event: &Value) -> String {
    let code = event
        .pointer("/response/error/code")
        .or_else(|| event.pointer("/error/code"))
        .and_then(Value::as_str)
        .unwrap_or("unknown_error");
    let message = event
        .pointer("/response/error/message")
        .or_else(|| event.pointer("/error/message"))
        .and_then(Value::as_str)
        .unwrap_or("Codex response failed");
    format!("Codex response failed ({code}): {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn response_error_uses_reviewed_fields_only() {
        let event = json!({
            "type": "response.failed",
            "response": {"error": {"code": "bad_request", "message": "invalid input"}},
            "access_token": "must-not-leak"
        });

        let message = response_error_message(&event);

        assert_eq!(
            message,
            "Codex response failed (bad_request): invalid input"
        );
        assert!(!message.contains("must-not-leak"));
    }
}
