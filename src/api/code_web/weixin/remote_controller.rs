use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::WeixinService;
use crate::api::code_web::remote::RemoteSnapshot;

pub(super) struct WeixinRemoteController {
    service: Arc<WeixinService>,
}

impl WeixinRemoteController {
    pub(super) fn new(service: Arc<WeixinService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/weixin")]
impl WeixinRemoteController {
    #[get("/targets")]
    async fn targets(&self) -> BootResult<RemoteSnapshot> {
        self.service.remote_targets().await
    }
}
