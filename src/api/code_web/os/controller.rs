use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};
use serde::Deserialize;

use super::service::OsService;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OsTokenLoginRequest {
    pub(super) token: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OsBrowserLoginRequest {}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OsLogoutRequest {}

pub(super) struct OsController {
    service: Arc<OsService>,
}

impl OsController {
    pub(super) fn new(service: Arc<OsService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/os")]
impl OsController {
    #[get("/account")]
    async fn account(&self) -> BootResult<serde_json::Value> {
        self.service.account().await
    }

    #[get("/session")]
    async fn session(&self) -> BootResult<serde_json::Value> {
        self.service.account().await
    }

    #[post("/login/token")]
    async fn login_with_token(
        &self,
        #[body] request: OsTokenLoginRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.login_with_token(request).await
    }

    #[post("/login/browser")]
    async fn login_with_browser(
        &self,
        #[body] _request: OsBrowserLoginRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.login_with_browser().await
    }

    #[post("/logout")]
    async fn logout(&self, #[body] _request: OsLogoutRequest) -> BootResult<serde_json::Value> {
        self.service.logout().await
    }
}
