use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::ConfigController;
use super::service::ConfigService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct ConfigModule;

impl Module for ConfigModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-config"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<ConfigService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(ConfigService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<ConfigService>()?;
        Ok(vec![Arc::new(ConfigController::new(service)).controller()?])
    }
}
