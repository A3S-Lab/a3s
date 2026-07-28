use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::EvolutionController;
use super::service::EvolutionService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct EvolutionModule;

impl Module for EvolutionModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-evolution"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::factory_arc::<EvolutionService, _>(|module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(EvolutionService::new(state)))
            }),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<EvolutionService>()?;
        Ok(vec![
            Arc::new(EvolutionController::new(service)).controller()?
        ])
    }
}
