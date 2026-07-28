use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;

pub(super) struct KernelControlsController {
    service: Arc<KernelService>,
}

impl KernelControlsController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelControlsController {
    #[get("/v1/kernel/session-controls/efforts")]
    async fn effort_levels(&self) -> BootResult<serde_json::Value> {
        self.service.effort_levels().await
    }

    #[get("/v1/kernel/sessions/{session_id}/controls")]
    async fn session_controls(
        &self,
        #[param("session_id")] session_id: String,
    ) -> BootResult<serde_json::Value> {
        self.service.session_controls(&session_id).await
    }

    #[patch("/v1/kernel/sessions/{session_id}/controls")]
    async fn update_session_controls(
        &self,
        #[param("session_id")] session_id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service
            .update_session_controls(&session_id, request)
            .await
    }

    #[post("/v1/kernel/sessions/{session_id}/goal/actions/{action}")]
    async fn update_goal_action(
        &self,
        #[param("session_id")] session_id: String,
        #[param("action")] action: String,
    ) -> BootResult<serde_json::Value> {
        self.service.update_goal_action(&session_id, &action).await
    }
}
