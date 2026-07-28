use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::ProcessesController;
use super::service::ProcessesService;

pub(in crate::api::code_web) struct ProcessesModule;

impl Module for ProcessesModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-processes"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::factory_arc::<ProcessesService, _>(|_module_ref| {
                Ok(Arc::new(ProcessesService::new()))
            }),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<ProcessesService>()?;
        Ok(vec![
            Arc::new(ProcessesController::new(service)).controller()?
        ])
    }
}
