use std::path::{Path, PathBuf};
use std::sync::Arc;

use a3s_boot::ilink::{IlinkClient, IlinkLoginTransport, IlinkMessagingTransport, IlinkModule};
use a3s_boot::{ControllerDefinition, Module, ModuleRef, ProviderDefinition, Result as BootResult};

use super::account_controller::WeixinAccountController;
use super::capability_controller::WeixinCapabilityController;
use super::channel_config::{WeixinChannelConfig, WeixinChannelLoad};
use super::credential_store::PrivateFileCredentialStore;
use super::dto::SafeBlocker;
use super::login_controller::WeixinLoginController;
use super::remote_controller::WeixinRemoteController;
use super::service::WeixinService;
use crate::api::code_web::remote::{RemoteAgentReadService, RemoteModule};
use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct WeixinModule {
    provider: WeixinProvider,
}

enum WeixinProvider {
    ConfiguredWithRemote,
    #[cfg(test)]
    Fixed(Arc<WeixinService>),
}

impl WeixinModule {
    pub(in crate::api::code_web) fn configured() -> Self {
        Self {
            provider: WeixinProvider::ConfiguredWithRemote,
        }
    }

    #[cfg(test)]
    pub(super) fn disabled_isolated() -> Self {
        Self {
            provider: WeixinProvider::Fixed(Arc::new(WeixinService::disabled(None))),
        }
    }

    #[cfg(test)]
    pub(super) fn mock(
        transport: Arc<dyn a3s_boot::ilink::IlinkLoginTransport>,
        credential_store: Arc<dyn super::credential_store::WeixinCredentialStore>,
    ) -> Self {
        let login = Arc::new(super::login_coordinator::WeixinLoginCoordinator::new(
            transport,
            Arc::clone(&credential_store),
        ));
        Self {
            provider: WeixinProvider::Fixed(Arc::new(WeixinService::mock(login, credential_store))),
        }
    }

    #[cfg(test)]
    pub(super) fn mock_with_remote(
        transport: Arc<dyn a3s_boot::ilink::IlinkLoginTransport>,
        credential_store: Arc<dyn super::credential_store::WeixinCredentialStore>,
        remote_read: Arc<RemoteAgentReadService>,
    ) -> Self {
        let login = Arc::new(super::login_coordinator::WeixinLoginCoordinator::new(
            transport,
            Arc::clone(&credential_store),
        ));
        Self {
            provider: WeixinProvider::Fixed(Arc::new(WeixinService::mock_with_remote(
                login,
                credential_store,
                remote_read,
            ))),
        }
    }

    #[cfg(test)]
    pub(super) fn mock_with_monitor(
        login_transport: Arc<dyn a3s_boot::ilink::IlinkLoginTransport>,
        credential_store: Arc<dyn super::credential_store::WeixinCredentialStore>,
        monitor: Arc<super::monitor::WeixinMonitorSupervisor>,
        runtime_store: super::runtime_store::WeixinRuntimeStore,
    ) -> Self {
        let login = Arc::new(super::login_coordinator::WeixinLoginCoordinator::new(
            login_transport,
            Arc::clone(&credential_store),
        ));
        Self {
            provider: WeixinProvider::Fixed(Arc::new(WeixinService::mock_with_monitor(
                login,
                credential_store,
                monitor,
                runtime_store,
            ))),
        }
    }

    #[cfg(test)]
    pub(super) fn production_for_test(
        login_transport: Arc<dyn IlinkLoginTransport>,
        messaging_transport: Arc<dyn IlinkMessagingTransport>,
        credential_store: Arc<dyn super::credential_store::WeixinCredentialStore>,
        remote_read: Arc<RemoteAgentReadService>,
        runtime_directory: PathBuf,
    ) -> Self {
        Self {
            provider: WeixinProvider::Fixed(Arc::new(WeixinService::production(
                login_transport,
                messaging_transport,
                credential_store,
                runtime_directory,
                remote_read,
            ))),
        }
    }
}

impl Module for WeixinModule {
    fn name(&self) -> &'static str {
        "a3s-code-web-weixin"
    }

    fn imports(&self) -> Vec<Arc<dyn Module>> {
        match &self.provider {
            WeixinProvider::ConfiguredWithRemote => vec![
                Arc::new(RemoteModule),
                Arc::new(IlinkModule::weixin(format!(
                    "A3S/{}",
                    env!("CARGO_PKG_VERSION")
                ))),
            ],
            #[cfg(test)]
            WeixinProvider::Fixed(_) => Vec::new(),
        }
    }

    fn providers(&self) -> BootResult<Vec<ProviderDefinition>> {
        match &self.provider {
            WeixinProvider::ConfiguredWithRemote => Ok(vec![ProviderDefinition::factory_arc::<
                WeixinService,
                _,
            >(configured_service)]),
            #[cfg(test)]
            WeixinProvider::Fixed(service) => {
                Ok(vec![ProviderDefinition::from_arc(Arc::clone(service))])
            }
        }
    }

    fn controllers(&self, module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        let service = module_ref.get::<WeixinService>()?;
        Ok(vec![
            Arc::new(WeixinCapabilityController::new(Arc::clone(&service))).controller()?,
            Arc::new(WeixinAccountController::new(Arc::clone(&service))).controller()?,
            Arc::new(WeixinLoginController::new(Arc::clone(&service))).controller()?,
            Arc::new(WeixinRemoteController::new(service)).controller()?,
        ])
    }

    fn on_application_bootstrap(
        &self,
        module_ref: ModuleRef,
    ) -> a3s_boot::BoxFuture<'static, BootResult<()>> {
        Box::pin(async move {
            let service = module_ref.get::<WeixinService>()?;
            service.bootstrap().await
        })
    }

    fn on_application_shutdown(
        &self,
        module_ref: ModuleRef,
    ) -> a3s_boot::BoxFuture<'static, BootResult<()>> {
        Box::pin(async move {
            let service = module_ref.get::<WeixinService>()?;
            service.shutdown().await
        })
    }
}

fn configured_service(module_ref: &ModuleRef) -> BootResult<Arc<WeixinService>> {
    let remote_read = module_ref.get::<RemoteAgentReadService>()?;
    let state = module_ref.get::<CodeWebState>()?;
    match WeixinChannelConfig::load(&state.config_path) {
        WeixinChannelLoad::Enabled => {}
        WeixinChannelLoad::Unavailable(blocker) => {
            return Ok(Arc::new(WeixinService::disabled_with(
                Some(remote_read),
                blocker,
            )))
        }
    }
    let client = module_ref.get::<IlinkClient>()?;
    let login_transport: Arc<dyn IlinkLoginTransport> = client.clone();
    let messaging_transport: Arc<dyn IlinkMessagingTransport> = client;
    let state_root = match weixin_state_root(&state.config_path) {
        Ok(state_root) => state_root,
        Err(blocker) => {
            return Ok(Arc::new(WeixinService::disabled_with(
                Some(remote_read),
                blocker,
            )))
        }
    };
    let credential_store = Arc::new(PrivateFileCredentialStore::new(
        state_root.join("credentials"),
    ));
    Ok(Arc::new(WeixinService::production(
        login_transport,
        messaging_transport,
        credential_store,
        state_root.join("runtime"),
        remote_read,
    )))
}

fn weixin_state_root(config_path: &Path) -> Result<PathBuf, SafeBlocker> {
    let parent = config_path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    std::fs::canonicalize(parent)
        .map(|parent| parent.join("channels").join("weixin"))
        .map_err(|_| SafeBlocker {
            code: "ilink_state_path_unavailable".to_string(),
            message: "The local Weixin state directory could not be resolved safely.".to_string(),
        })
}
