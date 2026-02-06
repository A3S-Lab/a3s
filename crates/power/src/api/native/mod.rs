pub mod chat;
pub mod embeddings;
pub mod generate;
pub mod models;
pub mod pull;

use axum::routing::{delete, get, post};
use axum::Router;

use crate::server::state::AppState;

/// Build the native (Ollama-compatible) API routes.
pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/generate", post(generate::handler))
        .route("/chat", post(chat::handler))
        .route("/pull", post(pull::handler))
        .route("/tags", get(models::list_handler))
        .route("/show", post(models::show_handler))
        .route("/delete", delete(models::delete_handler))
        .route("/embeddings", post(embeddings::handler))
}
