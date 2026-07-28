use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::CapabilitiesService;

pub(super) struct CapabilitiesController {
    service: Arc<CapabilitiesService>,
}

impl CapabilitiesController {
    pub(super) fn new(service: Arc<CapabilitiesService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/capabilities")]
impl CapabilitiesController {
    #[get("/")]
    async fn overview(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.overview())
    }

    #[post("/dirs/ensure")]
    async fn ensure_dirs(
        &self,
        #[body] _request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.ensure_dirs().await
    }

    #[get("/lifecycles")]
    async fn lifecycles(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.lifecycles())
    }

    #[post("/actions/run")]
    async fn run_action(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.run_action(request).await
    }
}
