pub mod router;
pub mod state;

use std::sync::Arc;

use crate::backend;
use crate::config::PowerConfig;
use crate::dirs;
use crate::error::{PowerError, Result};
use crate::model::registry::ModelRegistry;

/// Start the HTTP server with the given configuration.
pub async fn start(config: PowerConfig) -> Result<()> {
    // Ensure storage directories exist
    dirs::ensure_dirs()?;

    // Initialize model registry and scan for existing models
    let registry = Arc::new(ModelRegistry::new());
    registry.scan()?;
    tracing::info!(count = registry.count(), "Loaded model registry");

    // Initialize backends
    let backends = Arc::new(backend::default_backends());
    tracing::info!(
        backends = ?backends.list_names(),
        "Initialized backends"
    );

    let bind_addr = config.bind_address();
    let app_state = state::AppState::new(registry, backends, Arc::new(config));

    let app = router::build(app_state);

    let listener = tokio::net::TcpListener::bind(&bind_addr)
        .await
        .map_err(|e| PowerError::Server(format!("Failed to bind to {bind_addr}: {e}")))?;

    tracing::info!("Server listening on {bind_addr}");

    axum::serve(listener, app)
        .await
        .map_err(|e| PowerError::Server(format!("Server error: {e}")))?;

    Ok(())
}
