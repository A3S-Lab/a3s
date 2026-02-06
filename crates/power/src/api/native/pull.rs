use axum::extract::State;
use axum::response::IntoResponse;
use axum::Json;

use crate::api::types::{PullRequest, PullResponse};
use crate::server::state::AppState;

/// POST /api/pull - Pull/download a model (Ollama-compatible).
///
/// Currently returns a non-streaming response. Streaming progress via SSE
/// will be implemented in a future phase.
pub async fn handler(
    State(state): State<AppState>,
    Json(request): Json<PullRequest>,
) -> impl IntoResponse {
    let model_name = &request.name;

    if state.registry.exists(model_name) {
        return Json(PullResponse {
            status: "success".to_string(),
            digest: None,
            total: None,
            completed: None,
        })
        .into_response();
    }

    // For now, only direct URL downloads are supported
    if !model_name.starts_with("http://") && !model_name.starts_with("https://") {
        return Json(PullResponse {
            status: "error: model registry not yet implemented, provide a direct URL".to_string(),
            digest: None,
            total: None,
            completed: None,
        })
        .into_response();
    }

    match crate::model::pull::pull_model(model_name, model_name, None).await {
        Ok(manifest) => {
            let digest = format!("sha256:{}", &manifest.sha256);
            let size = manifest.size;
            if let Err(e) = state.registry.register(manifest) {
                return Json(PullResponse {
                    status: format!("error: {e}"),
                    digest: None,
                    total: None,
                    completed: None,
                })
                .into_response();
            }
            Json(PullResponse {
                status: "success".to_string(),
                digest: Some(digest),
                total: Some(size),
                completed: Some(size),
            })
            .into_response()
        }
        Err(e) => Json(PullResponse {
            status: format!("error: {e}"),
            digest: None,
            total: None,
            completed: None,
        })
        .into_response(),
    }
}
