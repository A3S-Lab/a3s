use std::sync::Arc;

use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::controller::KnowledgeController;
use super::service::KnowledgeService;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct KnowledgeModule;

impl Module for KnowledgeModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-knowledge"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::factory_arc::<KnowledgeService, _>(|module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(KnowledgeService::new(state)))
            }),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<KnowledgeService>()?;
        Ok(vec![
            Arc::new(KnowledgeController::new(service)).controller()?
        ])
    }
}
