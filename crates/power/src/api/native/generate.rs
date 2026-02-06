use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{GenerateRequest, GenerateResponse};
use crate::server::state::AppState;

/// POST /api/generate - Text generation (Ollama-compatible).
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<GenerateRequest>,
) -> impl IntoResponse {
    let model_name = &request.model;

    // Check if model exists
    let manifest = match state.registry.get(model_name) {
        Ok(m) => m,
        Err(_) => {
            return Json(serde_json::json!({
                "error": format!("model '{}' not found", model_name)
            }))
            .into_response();
        }
    };

    // Find backend
    let backend = match state.backends.find_for_format(&manifest.format) {
        Ok(b) => b,
        Err(e) => {
            return Json(serde_json::json!({ "error": e.to_string() })).into_response();
        }
    };

    // Convert to backend types
    let backend_request = crate::backend::types::CompletionRequest {
        prompt: request.prompt,
        temperature: request.options.as_ref().and_then(|o| o.temperature),
        top_p: request.options.as_ref().and_then(|o| o.top_p),
        max_tokens: request.options.as_ref().and_then(|o| o.num_predict),
        stop: request.options.as_ref().and_then(|o| o.stop.clone()),
        stream: request.stream.unwrap_or(false),
    };

    match backend.complete(model_name, backend_request).await {
        Ok(_stream) => {
            // For now, return a placeholder response
            let response = GenerateResponse {
                model: model_name.to_string(),
                response: String::new(),
                done: true,
                total_duration: None,
                load_duration: None,
                eval_count: None,
                eval_duration: None,
            };
            Json(response).into_response()
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })).into_response(),
    }
}
