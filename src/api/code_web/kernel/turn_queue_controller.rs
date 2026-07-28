use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;

pub(super) struct KernelTurnQueueController {
    service: Arc<KernelService>,
}

impl KernelTurnQueueController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelTurnQueueController {
    #[get("/v1/kernel/sessions/{session_id}/turn-queue")]
    async fn session_turn_queue(
        &self,
        #[param("session_id")] session_id: String,
    ) -> BootResult<serde_json::Value> {
        self.service.session_turn_queue(&session_id).await
    }

    #[post("/v1/kernel/sessions/{session_id}/turn-queue")]
    async fn enqueue_session_turn(
        &self,
        #[param("session_id")] session_id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service
            .enqueue_session_turn(&session_id, request)
            .await
    }

    #[patch("/v1/kernel/sessions/{session_id}/turn-queue/{turn_id}")]
    async fn update_session_turn(
        &self,
        #[param("session_id")] session_id: String,
        #[param("turn_id")] turn_id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service
            .update_session_turn(&session_id, &turn_id, request)
            .await
    }

    #[delete("/v1/kernel/sessions/{session_id}/turn-queue/{turn_id}")]
    async fn delete_session_turn(
        &self,
        #[param("session_id")] session_id: String,
        #[param("turn_id")] turn_id: String,
    ) -> BootResult<serde_json::Value> {
        self.service
            .delete_session_turn(&session_id, &turn_id)
            .await
    }

    #[post("/v1/kernel/sessions/{session_id}/turn-queue/reorder")]
    async fn reorder_session_turns(
        &self,
        #[param("session_id")] session_id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service
            .reorder_session_turns(&session_id, request)
            .await
    }

    #[post("/v1/kernel/sessions/{session_id}/turn-queue/actions/{action}")]
    async fn update_session_turn_queue_action(
        &self,
        #[param("session_id")] session_id: String,
        #[param("action")] action: String,
    ) -> BootResult<serde_json::Value> {
        self.service
            .update_session_turn_queue_action(&session_id, &action)
            .await
    }
}
