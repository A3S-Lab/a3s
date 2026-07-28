use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};
use serde::Deserialize;

use super::service::PluginsService;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginToggleRequest {
    pub(super) enabled: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginReloadRequest {
    pub(super) rebuild_sessions: Option<bool>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginPlanRequest {
    pub(super) action: String,
    pub(super) component_id: String,
    pub(super) version: Option<String>,
    pub(super) channel: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginApplyRequest {
    pub(super) action: String,
    pub(super) component_id: String,
    pub(super) version: Option<String>,
    pub(super) channel: Option<String>,
    pub(super) plan_digest: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PluginPackageToggleRequest {
    pub(super) component_id: String,
    pub(super) enabled: bool,
}

impl Default for PluginReloadRequest {
    fn default() -> Self {
        Self {
            rebuild_sessions: Some(true),
        }
    }
}

pub(super) struct PluginsController {
    service: Arc<PluginsService>,
}

impl PluginsController {
    pub(super) fn new(service: Arc<PluginsService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/plugins")]
impl PluginsController {
    #[get("/")]
    async fn list(
        &self,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.list(workspace).await
    }

    #[post("/{name}/enabled")]
    async fn set_enabled(
        &self,
        #[param("name")] name: String,
        #[body] request: PluginToggleRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.set_enabled(&name, request).await
    }

    #[post("/reload")]
    async fn reload(&self, #[body] request: PluginReloadRequest) -> BootResult<serde_json::Value> {
        self.service.reload(request).await
    }

    #[get("/activities")]
    async fn activities(&self) -> BootResult<serde_json::Value> {
        self.service.activities()
    }

    #[get("/activities/{key}")]
    async fn activity_content(&self, #[param("key")] key: String) -> BootResult<serde_json::Value> {
        self.service.activity_content(&key)
    }

    #[get("/marketplace")]
    async fn marketplace(&self) -> BootResult<serde_json::Value> {
        self.service.marketplace().await
    }

    #[post("/operations/plan")]
    async fn plan_operation(
        &self,
        #[body] request: PluginPlanRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.plan_operation(request).await
    }

    #[post("/operations/apply")]
    async fn apply_operation(
        &self,
        #[body] request: PluginApplyRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.apply_operation(request).await
    }

    #[post("/packages/enabled")]
    async fn set_package_enabled(
        &self,
        #[body] request: PluginPackageToggleRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.set_package_enabled(request).await
    }
}
