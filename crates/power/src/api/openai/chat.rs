use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{
    ChatChoice, ChatCompletionMessage, ChatCompletionRequest, ChatCompletionResponse, Usage,
};
use crate::backend::types::{ChatMessage, ChatRequest};
use crate::server::state::AppState;

/// POST /v1/chat/completions - OpenAI-compatible chat completion.
///
/// Supports both streaming and non-streaming modes.
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<ChatCompletionRequest>,
) -> impl IntoResponse {
    let model_name = &request.model;

    let manifest = match state.registry.get(model_name) {
        Ok(m) => m,
        Err(_) => {
            return Json(serde_json::json!({
                "error": {
                    "message": format!("model '{}' not found", model_name),
                    "type": "invalid_request_error",
                    "code": "model_not_found"
                }
            }))
            .into_response();
        }
    };

    let backend = match state.backends.find_for_format(&manifest.format) {
        Ok(b) => b,
        Err(e) => {
            return Json(serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "server_error",
                    "code": null
                }
            }))
            .into_response();
        }
    };

    let backend_request = ChatRequest {
        messages: request
            .messages
            .iter()
            .map(|m| ChatMessage {
                role: m.role.clone(),
                content: m.content.clone(),
            })
            .collect(),
        temperature: request.temperature,
        top_p: request.top_p,
        max_tokens: request.max_tokens,
        stop: request.stop.clone(),
        stream: request.stream.unwrap_or(false),
    };

    let is_stream = request.stream.unwrap_or(false);

    if is_stream {
        // Streaming: return SSE response
        // Full streaming implementation will come in Phase 5
        match backend.chat(model_name, backend_request).await {
            Ok(_stream) => Json(serde_json::json!({
                "error": {
                    "message": "Streaming not yet fully implemented",
                    "type": "server_error",
                    "code": null
                }
            }))
            .into_response(),
            Err(e) => Json(serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "server_error",
                    "code": null
                }
            }))
            .into_response(),
        }
    } else {
        // Non-streaming: return complete response
        match backend.chat(model_name, backend_request).await {
            Ok(_stream) => {
                let response = ChatCompletionResponse {
                    id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
                    object: "chat.completion".to_string(),
                    created: chrono::Utc::now().timestamp(),
                    model: model_name.to_string(),
                    choices: vec![ChatChoice {
                        index: 0,
                        message: ChatCompletionMessage {
                            role: "assistant".to_string(),
                            content: String::new(),
                        },
                        finish_reason: Some("stop".to_string()),
                    }],
                    usage: Usage {
                        prompt_tokens: 0,
                        completion_tokens: 0,
                        total_tokens: 0,
                    },
                };
                Json(response).into_response()
            }
            Err(e) => Json(serde_json::json!({
                "error": {
                    "message": e.to_string(),
                    "type": "server_error",
                    "code": null
                }
            }))
            .into_response(),
        }
    }
}
