use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::dto::WeixinCapabilityResponse;
use super::service::WeixinService;

pub(super) struct WeixinCapabilityController {
    service: Arc<WeixinService>,
}

impl WeixinCapabilityController {
    pub(super) fn new(service: Arc<WeixinService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/weixin")]
impl WeixinCapabilityController {
    #[get("/capability")]
    async fn capability(&self) -> BootResult<WeixinCapabilityResponse> {
        Ok(self.service.capability())
    }
}
