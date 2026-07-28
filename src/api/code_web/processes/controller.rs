use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::ProcessesService;

pub(super) struct ProcessesController {
    service: Arc<ProcessesService>,
}

impl ProcessesController {
    pub(super) fn new(service: Arc<ProcessesService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl ProcessesController {
    #[get("/v1/processes/top")]
    async fn top(&self) -> BootResult<serde_json::Value> {
        self.service.top().await
    }
}
