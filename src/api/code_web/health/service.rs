use std::sync::Arc;

use crate::api::code_web::dto::HealthResponse;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct HealthService {
    state: Arc<CodeWebState>,
}

impl HealthService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) fn health(&self) -> HealthResponse {
        let code_config = self.state.code_config_snapshot();
        HealthResponse {
            schema_version: 1,
            ok: true,
            service: "a3s-code-web".to_string(),
            app: "书小安".to_string(),
            version: env!("CARGO_PKG_VERSION").to_string(),
            pid: std::process::id(),
            config_path: self.state.config_path.display().to_string(),
            workspace: self.state.default_workspace.display().to_string(),
            model: code_config.default_model,
        }
    }
}
