use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{NativeEmbeddingRequest, NativeEmbeddingResponse};
use crate::backend::types::EmbeddingRequest;
use crate::server::state::AppState;

/// POST /api/embeddings - Generate embeddings (Ollama-compatible).
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<NativeEmbeddingRequest>,
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

    let backend_request = EmbeddingRequest {
        input: vec![request.prompt],
    };

    match backend.embed(model_name, backend_request).await {
        Ok(response) => {
            let embedding = response.embeddings.into_iter().next().unwrap_or_default();
            Json(NativeEmbeddingResponse { embedding }).into_response()
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })).into_response(),
    }
}
