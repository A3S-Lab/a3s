use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::service::ConfigService;

pub(super) struct ConfigController {
    service: Arc<ConfigService>,
}

impl ConfigController {
    pub(super) fn new(service: Arc<ConfigService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/config")]
impl ConfigController {
    #[get("/public/system-info")]
    async fn system_info(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.system_info())
    }

    #[get("/assistant")]
    async fn assistant_settings(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.assistant_settings())
    }

    #[get("/")]
    async fn app_settings(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.app_settings())
    }

    #[put("/")]
    async fn replace_app_settings(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.update_app_settings(request)
    }

    #[patch("/")]
    async fn update_app_settings(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.update_app_settings(request)
    }

    #[get("/categories/{name}")]
    async fn config_category(
        &self,
        #[param("name")] name: String,
    ) -> BootResult<serde_json::Value> {
        self.service.config_category(&name)
    }

    #[put("/categories/{name}")]
    async fn update_config_category(
        &self,
        #[param("name")] name: String,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.update_config_category(&name, request)
    }

    #[post("/validate")]
    async fn validate_config(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.validate(request)
    }

    #[get("/diagnostics/llm")]
    async fn llm_diagnostics(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.llm_diagnostics())
    }

    #[get("/llm/models")]
    async fn model_catalog(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.model_catalog())
    }

    #[get("/llm/models/refresh")]
    async fn refresh_model_catalog(&self) -> BootResult<serde_json::Value> {
        Ok(self.service.refresh_model_catalog().await)
    }

    #[post("/llm/providers/models/fetch")]
    async fn fetch_provider_models(
        &self,
        #[body] request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.service.fetch_provider_models(request)
    }
}
