use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::dto::{StartWeixinLoginRequest, SubmitWeixinVerificationRequest, WeixinAccountResponse};
use super::login_coordinator::WeixinLoginAttempt;
use super::service::WeixinService;

pub(super) struct WeixinLoginController {
    service: Arc<WeixinService>,
}

impl WeixinLoginController {
    pub(super) fn new(service: Arc<WeixinService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/weixin")]
impl WeixinLoginController {
    #[post("/login-attempts")]
    async fn start(
        &self,
        #[body] request: StartWeixinLoginRequest,
    ) -> BootResult<WeixinLoginAttempt> {
        self.service.start_login(request).await
    }

    #[get("/login-attempts/{attempt_id}")]
    async fn poll(
        &self,
        #[param("attempt_id")] attempt_id: String,
    ) -> BootResult<WeixinLoginAttempt> {
        self.service.poll_login(&attempt_id).await
    }

    #[post("/login-attempts/{attempt_id}/verification")]
    async fn submit_verification(
        &self,
        #[param("attempt_id")] attempt_id: String,
        #[body] request: SubmitWeixinVerificationRequest,
    ) -> BootResult<WeixinLoginAttempt> {
        self.service.submit_verification(&attempt_id, request).await
    }

    #[delete("/login-attempts/{attempt_id}")]
    async fn cancel(
        &self,
        #[param("attempt_id")] attempt_id: String,
    ) -> BootResult<WeixinAccountResponse> {
        self.service.cancel_login(&attempt_id).await
    }
}
