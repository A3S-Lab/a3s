pub mod chat;
pub mod completions;
pub mod embeddings;
pub mod models;

use axum::routing::{get, post};
use axum::Router;

use crate::server::state::AppState;

/// Build the OpenAI-compatible API routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/chat/completions", post(chat::handler))
        .route("/completions", post(completions::handler))
        .route("/models", get(models::handler))
        .route("/embeddings", post(embeddings::handler))
}
