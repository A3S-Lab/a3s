use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{ChatCompletionMessage, NativeChatRequest, NativeChatResponse};
use crate::backend::types::{ChatMessage, ChatRequest};
use crate::server::state::AppState;

/// POST /api/chat - Chat completion (Ollama-compatible).
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<NativeChatRequest>,
) -> impl IntoResponse {
    let model_name = &request.model;

    let manifest = match state.registry.get(model_name) {
        Ok(m) => m,
        Err(_) => {
            return Json(serde_json::json!({
                "error": format!("model '{}' not found", model_name)
            }))
            .into_response();
        }
    };

    let backend = match state.backends.find_for_format(&manifest.format) {
        Ok(b) => b,
        Err(e) => {
            return Json(serde_json::json!({ "error": e.to_string() })).into_response();
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
        temperature: request.options.as_ref().and_then(|o| o.temperature),
        top_p: request.options.as_ref().and_then(|o| o.top_p),
        max_tokens: request.options.as_ref().and_then(|o| o.num_predict),
        stop: request.options.as_ref().and_then(|o| o.stop.clone()),
        stream: request.stream.unwrap_or(false),
    };

    match backend.chat(model_name, backend_request).await {
        Ok(_stream) => {
            let response = NativeChatResponse {
                model: model_name.to_string(),
                message: ChatCompletionMessage {
                    role: "assistant".to_string(),
                    content: String::new(),
                },
                done: true,
                total_duration: None,
                eval_count: None,
            };
            Json(response).into_response()
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })).into_response(),
    }
}
