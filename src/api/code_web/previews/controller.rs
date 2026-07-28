use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::model::{CreatePreviewRequest, PreviewDescriptor};
use super::service::PreviewsService;

pub(super) struct PreviewsController {
    service: Arc<PreviewsService>,
}

impl PreviewsController {
    pub(super) fn new(service: Arc<PreviewsService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl PreviewsController {
    #[post("/v1/previews")]
    async fn create_preview(
        &self,
        #[body] request: CreatePreviewRequest,
    ) -> BootResult<PreviewDescriptor> {
        self.service.create(request).await
    }

    #[get("/v1/previews/{id}")]
    async fn preview(&self, #[param("id")] id: String) -> BootResult<PreviewDescriptor> {
        self.service.get(&id).await
    }

    #[delete("/v1/previews/{id}")]
    async fn stop_preview(&self, #[param("id")] id: String) -> BootResult<serde_json::Value> {
        self.service.remove(&id).await
    }
}
