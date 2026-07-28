use std::sync::Arc;

use a3s_boot::{
    ControllerDefinition, Module, ModuleRef, ProviderDefinition, ProviderToken,
    Result as BootResult,
};

use super::agents_controller::KernelAgentsController;
use super::chat_controller::KernelChatController;
use super::compaction_controller::KernelCompactionController;
use super::controls_controller::KernelControlsController;
use super::fork_controller::KernelForkController;
use super::output_controller::KernelOutputController;
use super::service::{KernelService, ManagedSessionReadPort};
use super::sessions_controller::KernelSessionsController;
use super::shell_controller::KernelShellController;
use super::sleep_controller::KernelSleepController;
use super::turn_queue_controller::KernelTurnQueueController;
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct KernelModule;

impl Module for KernelModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-kernel"
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::factory_arc::<KernelService, _>(|module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                Ok(Arc::new(KernelService::new(state)))
            }),
            ProviderDefinition::factory_arc::<ManagedSessionReadPort, _>(|module_ref| {
                let service = module_ref.get::<KernelService>()?;
                Ok(Arc::new(ManagedSessionReadPort::new(service)))
            }),
        ])
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<KernelService>()?;
        Ok(vec![
            Arc::new(KernelAgentsController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelSessionsController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelControlsController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelCompactionController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelSleepController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelTurnQueueController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelForkController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelOutputController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelShellController::new(Arc::clone(&service))).controller()?,
            Arc::new(KernelChatController::new(service)).controller()?,
        ])
    }

    fn exports(&self) -> BootResult<Vec<ProviderToken>> {
        Ok(vec![ProviderToken::of::<ManagedSessionReadPort>()])
    }
}
