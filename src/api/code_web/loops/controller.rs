use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};
use serde::Deserialize;

use super::service::LoopsService;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoopInitRequest {
    pub(super) workspace: Option<String>,
    pub(super) name: Option<String>,
    pub(super) pattern: Option<String>,
    pub(super) arg: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoopActionRequest {
    pub(super) workspace: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct LoopRunPromptRequest {
    pub(super) workspace: Option<String>,
    pub(super) os_available: Option<bool>,
    pub(super) runtime_mode: Option<String>,
}

pub(super) struct LoopsController {
    service: Arc<LoopsService>,
}

impl LoopsController {
    pub(super) fn new(service: Arc<LoopsService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/loops")]
impl LoopsController {
    #[get("/")]
    async fn list(
        &self,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.list(workspace).await
    }

    #[post("/")]
    async fn init(&self, #[body] request: LoopInitRequest) -> BootResult<serde_json::Value> {
        self.service.init(request).await
    }

    #[get("/{loop_id}")]
    async fn get(
        &self,
        #[param("loop_id")] loop_id: String,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.get(&loop_id, workspace).await
    }

    #[post("/{loop_id}/audit")]
    async fn audit(
        &self,
        #[param("loop_id")] loop_id: String,
        #[body] request: LoopActionRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.audit(&loop_id, request).await
    }

    #[get("/{loop_id}/logs")]
    async fn logs(
        &self,
        #[param("loop_id")] loop_id: String,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.logs(&loop_id, workspace).await
    }

    #[post("/{loop_id}/run-prompt")]
    async fn run_prompt(
        &self,
        #[param("loop_id")] loop_id: String,
        #[body] request: LoopRunPromptRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.run_prompt(&loop_id, request).await
    }
}
