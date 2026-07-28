use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::HealthController;
use super::service::HealthService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct HealthModule;

impl Module for HealthModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-health"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<HealthService, _>(
            |module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(HealthService::new(state)))
            },
        )])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<HealthService>()?;
        Ok(vec![Arc::new(HealthController::new(service)).controller()?])
    }
}
