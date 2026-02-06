use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{
    DeleteRequest, NativeModelDetails, NativeModelInfo, ShowRequest, ShowResponse,
};
use crate::model::storage;
use crate::server::state::AppState;

/// GET /api/tags - List local models (Ollama-compatible).
pub async fn list_handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.registry.list() {
        Ok(models) => {
            let model_infos: Vec<NativeModelInfo> = models
                .iter()
                .map(|m| NativeModelInfo {
                    name: m.name.clone(),
                    modified_at: m.created_at.to_rfc3339(),
                    size: m.size,
                    digest: format!("sha256:{}", &m.sha256),
                    details: NativeModelDetails {
                        format: m.format.to_string(),
                        parameter_size: m
                            .parameters
                            .as_ref()
                            .and_then(|p| p.parameter_count)
                            .map(|c| format!("{c}")),
                        quantization_level: m
                            .parameters
                            .as_ref()
                            .and_then(|p| p.quantization.clone()),
                    },
                })
                .collect();
            Json(serde_json::json!({ "models": model_infos })).into_response()
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })).into_response(),
    }
}

/// POST /api/show - Show model details (Ollama-compatible).
pub async fn show_handler(
    State(state): State<AppState>,
    Json(request): Json<ShowRequest>,
) -> impl IntoResponse {
    match state.registry.get(&request.name) {
        Ok(manifest) => {
            let response = ShowResponse {
                modelfile: String::new(),
                parameters: serde_json::to_string_pretty(&manifest.parameters).unwrap_or_default(),
                template: String::new(),
                details: NativeModelDetails {
                    format: manifest.format.to_string(),
                    parameter_size: manifest
                        .parameters
                        .as_ref()
                        .and_then(|p| p.parameter_count)
                        .map(|c| format!("{c}")),
                    quantization_level: manifest
                        .parameters
                        .as_ref()
                        .and_then(|p| p.quantization.clone()),
                },
            };
            Json(response).into_response()
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })).into_response(),
    }
}

/// DELETE /api/delete - Delete a model (Ollama-compatible).
pub async fn delete_handler(
    State(state): State<AppState>,
    Json(request): Json<DeleteRequest>,
) -> impl IntoResponse {
    match state.registry.remove(&request.name) {
        Ok(manifest) => {
            if let Err(e) = storage::delete_blob(&manifest) {
                tracing::warn!(model = %manifest.name, "Failed to delete blob: {e}");
            }
            Json(serde_json::json!({ "status": "success" })).into_response()
        }
        Err(e) => Json(serde_json::json!({ "error": e.to_string() })).into_response(),
    }
}
