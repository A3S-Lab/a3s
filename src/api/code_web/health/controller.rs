use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::HealthService;
use crate::api::code_web::dto::HealthResponse;

pub(super) struct HealthController {
    service: Arc<HealthService>,
}

impl HealthController {
    pub(super) fn new(service: Arc<HealthService>) -> Self {
        Self { service }
    }
}

#[controller("/")]
impl HealthController {
    #[get("/health")]
    async fn health(&self) -> BootResult<HealthResponse> {
        Ok(self.service.health())
    }

    #[get("/v1/health")]
    async fn v1_health(&self) -> BootResult<HealthResponse> {
        Ok(self.service.health())
    }
}
