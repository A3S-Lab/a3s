use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::ContextController;
use super::service::ContextService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct ContextModule;

impl Module for ContextModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-context"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<ContextService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(ContextService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<ContextService>()?;
        Ok(vec![Arc::new(ContextController::new(service)).controller()?])
    }
}
