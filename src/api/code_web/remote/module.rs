use std::sync::Arc;

use a3s_boot::{Module, ProviderDefinition, ProviderToken, Result as BootResult};

use super::router::RemoteIntentRouter;
use super::service::RemoteAgentReadService;
use crate::api::code_web::kernel::{KernelModule, ManagedSessionReadPort};
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct RemoteModule;

impl Module for RemoteModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-remote"
    }

    fn imports(&self) -> Vec<Arc<dyn Module>> {
        vec![Arc::new(KernelModule)]
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        Ok(vec![
            ProviderDefinition::factory_arc::<RemoteAgentReadService, _>(|module_ref| {
                let managed = module_ref.get::<ManagedSessionReadPort>()?;
                Ok(Arc::new(RemoteAgentReadService::new(managed)))
            }),
            ProviderDefinition::factory_arc::<RemoteIntentRouter, _>(|module_ref| {
                let state = module_ref.get::<CodeWebState>()?;
                let llm = crate::session_llm::resolve_config_llm_client(
                    &state.code_config_snapshot(),
                    &a3s_code_core::SessionOptions::new(),
                    "weixin-intent-router",
                )
                .ok();
                Ok(Arc::new(RemoteIntentRouter::with_optional_llm(llm)))
            }),
        ])
    }

    fn exports(&self) -> BootResult<Vec<ProviderToken>> {
        Ok(vec![
            ProviderToken::of::<RemoteAgentReadService>(),
            ProviderToken::of::<RemoteIntentRouter>(),
        ])
    }
}
