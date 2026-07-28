use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::EvolutionService;

pub(super) struct EvolutionController {
    service: Arc<EvolutionService>,
}

impl EvolutionController {
    pub(super) fn new(service: Arc<EvolutionService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/evolution")]
impl EvolutionController {
    #[get("/")]
    async fn overview(&self) -> BootResult<serde_json::Value> {
        self.service.overview().await
    }

    #[post("/scan")]
    async fn scan(&self, #[body] _request: serde_json::Value) -> BootResult<serde_json::Value> {
        self.service.scan().await
    }

    #[post("/{id}/materialize")]
    async fn materialize(
        &self,
        #[param("id")] id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.materialize(id, request).await
    }

    #[post("/{id}/reject")]
    async fn reject(
        &self,
        #[param("id")] id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.reject(id, request).await
    }

    #[post("/{id}/reopen")]
    async fn reopen(
        &self,
        #[param("id")] id: String,
        #[body] _request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.reopen(id).await
    }

    #[post("/{id}/rollback")]
    async fn rollback(
        &self,
        #[param("id")] id: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.rollback(id, request).await
    }
}
