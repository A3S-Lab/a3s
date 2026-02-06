use axum::Router;
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;

use super::state::AppState;
use crate::api;

/// Build the complete axum Router with all API routes.
pub fn build(state: AppState) -> Router {
    Router::new()
        .nest("/api", api::native::routes())
        .nest("/v1", api::openai::routes())
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
