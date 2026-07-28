use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::dto::{WeixinAccountActionRequest, WeixinAccountResponse};
use super::service::WeixinService;

pub(super) struct WeixinAccountController {
    service: Arc<WeixinService>,
}

impl WeixinAccountController {
    pub(super) fn new(service: Arc<WeixinService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/weixin")]
impl WeixinAccountController {
    #[get("/account")]
    async fn account(&self) -> BootResult<WeixinAccountResponse> {
        self.service.account().await
    }

    #[delete("/account")]
    async fn disconnect(&self) -> BootResult<WeixinAccountResponse> {
        self.service.disconnect().await
    }

    #[post("/account/pause")]
    async fn pause(
        &self,
        #[body] _request: WeixinAccountActionRequest,
    ) -> BootResult<WeixinAccountResponse> {
        self.service.pause().await
    }

    #[post("/account/resume")]
    async fn resume(
        &self,
        #[body] _request: WeixinAccountActionRequest,
    ) -> BootResult<WeixinAccountResponse> {
        self.service.resume().await
    }
}
