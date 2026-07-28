use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;

pub(super) struct KernelCompactionController {
    service: Arc<KernelService>,
}

impl KernelCompactionController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelCompactionController {
    #[post("/v1/kernel/sessions/{session_id}/actions/compact")]
    async fn compact_session(
        &self,
        #[param("session_id")] session_id: String,
    ) -> BootResult<serde_json::Value> {
        self.service.compact_session(&session_id).await
    }
}
