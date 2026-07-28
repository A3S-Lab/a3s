use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::WorkController;
use super::service::WorkService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct WorkModule;

impl Module for WorkModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-work"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<WorkService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(WorkService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<WorkService>()?;
        Ok(vec![Arc::new(WorkController::new(service)).controller()?])
    }
}
