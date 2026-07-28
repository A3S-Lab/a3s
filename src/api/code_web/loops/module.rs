use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::LoopsController;
use super::service::LoopsService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct LoopsModule;

impl Module for LoopsModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-loops"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<LoopsService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(LoopsService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<LoopsService>()?;
        Ok(vec![Arc::new(LoopsController::new(service)).controller()?])
    }
}
