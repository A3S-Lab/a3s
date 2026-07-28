use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;
use crate::api::code_web::dto::ShellSessionRequest;

pub(super) struct KernelShellController {
    service: Arc<KernelService>,
}

impl KernelShellController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelShellController {
    #[post("/v1/kernel/sessions/{session_id}/actions/shell")]
    async fn run_shell_command(
        &self,
        #[param("session_id")] session_id: String,
        #[body] request: ShellSessionRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.run_shell_command(&session_id, request).await
    }
}
