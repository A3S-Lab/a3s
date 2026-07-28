use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::KernelService;
use crate::api::code_web::dto::{ChatRequest, ChatResponse};

pub(super) struct KernelChatController {
    service: Arc<KernelService>,
}

impl KernelChatController {
    pub(super) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl KernelChatController {
    #[post("/chat")]
    async fn chat(&self, #[body] request: ChatRequest) -> BootResult<ChatResponse> {
        self.service.chat(request).await
    }
}
