use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{EmbeddingData, EmbeddingRequest, EmbeddingResponse, EmbeddingUsage};
use crate::server::state::AppState;

/// POST /v1/embeddings - OpenAI-compatible embedding generation.
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<EmbeddingRequest>,
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

    let input_texts = request.input.into_vec();
    let backend_request = crate::backend::types::EmbeddingRequest {
        input: input_texts.clone(),
    };

    match backend.embed(model_name, backend_request).await {
        Ok(result) => {
            let data: Vec<EmbeddingData> = result
                .embeddings
                .into_iter()
                .enumerate()
                .map(|(i, emb)| EmbeddingData {
                    object: "embedding".to_string(),
                    embedding: emb,
                    index: i as u32,
                })
                .collect();

            let response = EmbeddingResponse {
                object: "list".to_string(),
                data,
                model: model_name.to_string(),
                usage: EmbeddingUsage {
                    prompt_tokens: 0,
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
