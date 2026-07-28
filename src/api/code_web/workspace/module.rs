use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::WorkspaceController;
use super::service::WorkspaceService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct WorkspaceModule;

impl Module for WorkspaceModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-workspace"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::factory_arc::<WorkspaceService, _>(|module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(WorkspaceService::new(state)))
            }),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<WorkspaceService>()?;
        Ok(vec![
            Arc::new(WorkspaceController::new(service)).controller()?
        ])
    }
}
