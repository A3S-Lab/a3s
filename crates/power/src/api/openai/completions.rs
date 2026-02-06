use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{CompletionChoice, CompletionRequest, CompletionResponse, Usage};
use crate::server::state::AppState;

/// POST /v1/completions - OpenAI-compatible text completion.
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<CompletionRequest>,
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

    let backend_request = crate::backend::types::CompletionRequest {
        prompt: request.prompt,
        temperature: request.temperature,
        top_p: request.top_p,
        max_tokens: request.max_tokens,
        stop: request.stop.clone(),
        stream: request.stream.unwrap_or(false),
    };

    match backend.complete(model_name, backend_request).await {
        Ok(_stream) => {
            let response = CompletionResponse {
                id: format!("cmpl-{}", uuid::Uuid::new_v4()),
                object: "text_completion".to_string(),
                created: chrono::Utc::now().timestamp(),
                model: model_name.to_string(),
                choices: vec![CompletionChoice {
                    index: 0,
                    text: String::new(),
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
