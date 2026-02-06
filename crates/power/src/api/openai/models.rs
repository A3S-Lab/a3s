use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{ModelInfo, ModelList};
use crate::server::state::AppState;

/// GET /v1/models - OpenAI-compatible model listing.
pub async fn handler(State(state): State<AppState>) -> impl IntoResponse {
    match state.registry.list() {
        Ok(models) => {
            let model_infos: Vec<ModelInfo> = models
                .iter()
                .map(|m| ModelInfo {
                    id: m.name.clone(),
                    object: "model".to_string(),
                    created: m.created_at.timestamp(),
                    owned_by: "local".to_string(),
                })
                .collect();

            Json(ModelList {
                object: "list".to_string(),
                data: model_infos,
            })
            .into_response()
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
