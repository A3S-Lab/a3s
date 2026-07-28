use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::CapabilitiesController;
use super::service::CapabilitiesService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct CapabilitiesModule;

impl Module for CapabilitiesModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-capabilities"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<
            CapabilitiesService,
            _,
        >(|module_ref| {
            let state = module_ref.get::<CodeWebState>()?;
            Ok(Arc::new(CapabilitiesService::new(state)))
        })])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<CapabilitiesService>()?;
        Ok(vec![
            Arc::new(CapabilitiesController::new(service)).controller()?
        ])
    }
}
