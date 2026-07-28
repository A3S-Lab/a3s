use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::PreviewsController;
use super::service::PreviewsService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct PreviewsModule;

impl Module for PreviewsModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-previews"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<PreviewsService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(PreviewsService::new(state.preview_registry())))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<PreviewsService>()?;
        Ok(vec![
            Arc::new(PreviewsController::new(service)).controller()?
        ])
    }
}
