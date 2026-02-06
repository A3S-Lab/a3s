use std::sync::Arc;

use crate::backend::BackendRegistry;
use crate::config::PowerConfig;
use crate::model::registry::ModelRegistry;

/// Shared application state accessible to all HTTP handlers.
#[derive(Clone)]
pub struct AppState {
    pub registry: Arc<ModelRegistry>,
    pub backends: Arc<BackendRegistry>,
    pub config: Arc<PowerConfig>,
}

impl AppState {
    pub fn new(
        registry: Arc<ModelRegistry>,
        backends: Arc<BackendRegistry>,
        config: Arc<PowerConfig>,
    ) -> Self {
        Self {
            registry,
            backends,
            config,
        }
    }
}
