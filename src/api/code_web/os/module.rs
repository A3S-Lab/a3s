use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::OsController;
use super::service::OsService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct OsModule;

impl Module for OsModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-os"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<OsService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(OsService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<OsService>()?;
        Ok(vec![Arc::new(OsController::new(service)).controller()?])
    }
}
