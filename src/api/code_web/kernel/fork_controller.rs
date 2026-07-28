use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;
use crate::api::code_web::dto::ForkSessionRequest;

pub(super) struct KernelForkController {
    service: Arc<KernelService>,
}

impl KernelForkController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelForkController {
    #[post("/v1/kernel/sessions/{session_id}/actions/fork")]
    async fn fork_session(
        &self,
        #[param("session_id")] session_id: String,
        #[body] request: ForkSessionRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.fork_session(&session_id, request).await
    }
}
