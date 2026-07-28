use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::CodeIntelligenceController;
use super::service::CodeIntelligenceService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct CodeIntelligenceModule;

impl Module for CodeIntelligenceModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-code-intelligence"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![ProviderDefinition::factory_arc::<
            CodeIntelligenceService,
            _,
        >(|module_ref| {
            let state = module_ref.get::<CodeWebState>()?;
            Ok(Arc::new(CodeIntelligenceService::new(state)))
        })])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<CodeIntelligenceService>()?;
        Ok(vec![
            Arc::new(CodeIntelligenceController::new(service)).controller()?
        ])
    }
}
