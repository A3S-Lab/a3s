use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;

pub(super) struct KernelAgentsController {
    service: Arc<KernelService>,
}

impl KernelAgentsController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/open/kernel")]
impl KernelAgentsController {
    #[get("/agents")]
    async fn list_kernel_agents(&self) -> BootResult<Vec<serde_json::Value>> {
        Ok(self.service.list_agents().await)
    }
}
