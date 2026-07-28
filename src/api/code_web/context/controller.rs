use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::ContextService;

pub(super) struct ContextController {
    service: Arc<ContextService>,
}

impl ContextController {
    pub(super) fn new(service: Arc<ContextService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/context")]
impl ContextController {
    #[get("/memory")]
    async fn memory(
        &self,
        #[query("query")] query: Option<String>,
        #[query("offset")] offset: Option<usize>,
        #[query("limit")] limit: Option<usize>,
        #[query("includeGraph")] include_graph: Option<bool>,
    ) -> BootResult<serde_json::Value> {
        self.service
            .memory(query, offset, limit, include_graph)
            .await
    }

    #[get("/memory/{id}")]
    async fn memory_detail(&self, #[param("id")] id: String) -> BootResult<serde_json::Value> {
        self.service.memory_detail(id).await
    }

    #[get("/ctx/status")]
    async fn ctx_status(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.ctx_status())
    }

    #[post("/ctx/search")]
    async fn ctx_search(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.ctx_search(request).await
    }

    #[post("/ctx/events/show")]
    async fn ctx_show_event(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.ctx_show_event(request).await
    }

    #[post("/ctx/sessions/show")]
    async fn ctx_show_session(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.ctx_show_session(request).await
    }

    #[post("/ctx/memory")]
    async fn ctx_save_memory(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.ctx_save_memory(request).await
    }

    #[get("/top")]
    async fn top(&self) -> BootResult<serde_json::Value> {
        self.service.top().await
    }
}
