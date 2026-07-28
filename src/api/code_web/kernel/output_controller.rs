use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;

pub(super) struct KernelOutputController {
    service: Arc<KernelService>,
}

impl KernelOutputController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelOutputController {
    #[get("/v1/kernel/sessions/{session_id}/output")]
    async fn kernel_session_output(
        &self,
        #[param("session_id")] session_id: String,
    ) -> BootResult<serde_json::Value> {
        self.service.session_output(&session_id).await
    }
}
