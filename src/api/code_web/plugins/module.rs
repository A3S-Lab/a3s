use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::PluginsController;
use super::service::PluginsService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct PluginsModule;

impl Module for PluginsModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-plugins"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<PluginsService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(PluginsService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<PluginsService>()?;
        Ok(vec![Arc::new(PluginsController::new(service)).controller()?])
    }
}
