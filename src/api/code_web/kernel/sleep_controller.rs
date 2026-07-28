use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;
use crate::api::code_web::dto::SleepSessionRequest;

pub(super) struct KernelSleepController {
    service: Arc<KernelService>,
}

impl KernelSleepController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelSleepController {
    #[post("/v1/kernel/sessions/{session_id}/actions/sleep")]
    async fn sleep_session(
        &self,
        #[param("session_id")] session_id: String,
        #[body] request: SleepSessionRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.sleep_session(&session_id, request).await
    }
}
