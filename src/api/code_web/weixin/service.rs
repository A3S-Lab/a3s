use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use a3s_boot::{BootError, Result as BootResult};
use chrono::{SecondsFormat, TimeZone, Utc};
use sha2::{Digest, Sha256};

use super::credential_store::{WeixinCredentialStore, WeixinCredentials};
use super::dto::{
    SafeBlocker, StartWeixinLoginRequest, SubmitWeixinVerificationRequest, WeixinAccountResponse,
    WeixinCapabilityResponse, WeixinCapabilityState, WeixinMonitorState, WeixinProtocolMode,
};
use super::login_coordinator::{
    WeixinLoginAttempt, WeixinLoginCoordinator, WeixinLoginError, WeixinLoginState,
};
use super::monitor::{
    MonitorError, MonitorErrorCode, MonitorHealth, MonitorLifecycleState, WeixinInboundHandler,
    WeixinMonitorSupervisor,
};
use super::remote_handler::RemoteReadHandler;
use super::runtime_store::WeixinRuntimeStore;
use crate::api::code_web::remote::{RemoteAgentReadService, RemoteReadScope, RemoteSnapshot};
use a3s_boot::ilink::{IlinkLoginTransport, IlinkMessagingTransport};

pub(super) struct WeixinService {
    runtime: RwLock<Option<Arc<WeixinRuntime>>>,
    production: Option<WeixinProductionRuntime>,
    remote_read: Option<Arc<RemoteAgentReadService>>,
    release_blocker: RwLock<Option<SafeBlocker>>,
}

struct WeixinProductionRuntime {
    login_transport: Arc<dyn IlinkLoginTransport>,
    messaging_transport: Arc<dyn IlinkMessagingTransport>,
    credential_store: Arc<dyn WeixinCredentialStore>,
    runtime_directory: PathBuf,
    initialization_lock: tokio::sync::Mutex<()>,
}

struct WeixinRuntime {
    login: Arc<WeixinLoginCoordinator>,
    credential_store: Arc<dyn WeixinCredentialStore>,
    monitor: Option<Arc<WeixinMonitorSupervisor>>,
    runtime_store: Option<WeixinRuntimeStore>,
    state: RwLock<WeixinCapabilityState>,
    protocol_mode: WeixinProtocolMode,
}

impl WeixinService {
    #[cfg(test)]
    pub(super) fn disabled(remote_read: Option<Arc<RemoteAgentReadService>>) -> Self {
        Self {
            runtime: RwLock::new(None),
            production: None,
            remote_read,
            release_blocker: RwLock::new(None),
        }
    }

    pub(super) fn disabled_with(
        remote_read: Option<Arc<RemoteAgentReadService>>,
        blocker: SafeBlocker,
    ) -> Self {
        Self {
            runtime: RwLock::new(None),
            production: None,
            remote_read,
            release_blocker: RwLock::new(Some(blocker)),
        }
    }

    pub(super) fn production(
        login_transport: Arc<dyn IlinkLoginTransport>,
        messaging_transport: Arc<dyn IlinkMessagingTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        runtime_directory: PathBuf,
        remote_read: Arc<RemoteAgentReadService>,
    ) -> Self {
        Self {
            runtime: RwLock::new(None),
            production: Some(WeixinProductionRuntime {
                login_transport,
                messaging_transport,
                credential_store,
                runtime_directory,
                initialization_lock: tokio::sync::Mutex::new(()),
            }),
            remote_read: Some(remote_read),
            release_blocker: RwLock::new(None),
        }
    }

    #[cfg(test)]
    pub(super) fn mock(
        login: Arc<WeixinLoginCoordinator>,
        credential_store: Arc<dyn WeixinCredentialStore>,
    ) -> Self {
        Self {
            runtime: RwLock::new(Some(Arc::new(WeixinRuntime {
                login,
                credential_store,
                monitor: None,
                runtime_store: None,
                state: RwLock::new(WeixinCapabilityState::Unbound),
                protocol_mode: WeixinProtocolMode::Mock,
            }))),
            production: None,
            remote_read: None,
            release_blocker: RwLock::new(None),
        }
    }

    #[cfg(test)]
    pub(super) fn mock_with_remote(
        login: Arc<WeixinLoginCoordinator>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        remote_read: Arc<RemoteAgentReadService>,
    ) -> Self {
        Self {
            runtime: RwLock::new(Some(Arc::new(WeixinRuntime {
                login,
                credential_store,
                monitor: None,
                runtime_store: None,
                state: RwLock::new(WeixinCapabilityState::Unbound),
                protocol_mode: WeixinProtocolMode::Mock,
            }))),
            production: None,
            remote_read: Some(remote_read),
            release_blocker: RwLock::new(None),
        }
    }

    #[cfg(test)]
    pub(super) fn mock_with_monitor(
        login: Arc<WeixinLoginCoordinator>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        monitor: Arc<WeixinMonitorSupervisor>,
        runtime_store: WeixinRuntimeStore,
    ) -> Self {
        Self {
            runtime: RwLock::new(Some(Arc::new(WeixinRuntime {
                login,
                credential_store,
                monitor: Some(monitor),
                runtime_store: Some(runtime_store),
                state: RwLock::new(WeixinCapabilityState::Unbound),
                protocol_mode: WeixinProtocolMode::Mock,
            }))),
            production: None,
            remote_read: None,
            release_blocker: RwLock::new(None),
        }
    }

    pub(super) fn capability(&self) -> WeixinCapabilityResponse {
        if self.production.is_some() {
            if let Some(blocker) = self.release_blocker() {
                return WeixinCapabilityResponse::production_unavailable(blocker);
            }
            let state = self
                .runtime_snapshot()
                .map(|runtime| runtime.effective_state())
                .unwrap_or(WeixinCapabilityState::Unbound);
            return WeixinCapabilityResponse::production(state);
        }
        match self.runtime_snapshot() {
            Some(runtime) => WeixinCapabilityResponse::mock(runtime.effective_state()),
            None => self
                .release_blocker()
                .map(WeixinCapabilityResponse::unavailable_with)
                .unwrap_or_else(WeixinCapabilityResponse::unavailable),
        }
    }

    pub(super) async fn account(&self) -> BootResult<WeixinAccountResponse> {
        let runtime = self.runtime().await?;
        runtime.account().await
    }

    pub(super) async fn remote_targets(&self) -> BootResult<RemoteSnapshot> {
        if self.runtime_snapshot().is_none() && self.production.is_none() {
            return Ok(RemoteSnapshot::new(
                crate::system_agents::epoch_ms(),
                Vec::new(),
                vec!["remote_read_disabled".to_string()],
            ));
        }
        self.runtime().await?;
        let remote = self.remote_read.as_ref().ok_or_else(|| {
            BootError::ServiceUnavailable(
                "WeChat remote target inventory is unavailable.".to_string(),
            )
        })?;
        Ok(remote.snapshot(RemoteReadScope::default()).await)
    }

    pub(super) async fn start_login(
        &self,
        request: StartWeixinLoginRequest,
    ) -> BootResult<WeixinLoginAttempt> {
        let runtime = self.runtime().await?;
        let attempt = runtime
            .login
            .start(request.force)
            .await
            .map_err(map_login_error)?;
        runtime.apply_login_state(attempt.state);
        Ok(attempt)
    }

    pub(super) async fn poll_login(&self, attempt_id: &str) -> BootResult<WeixinLoginAttempt> {
        let runtime = self.runtime().await?;
        let attempt = runtime
            .login
            .poll(attempt_id)
            .await
            .map_err(map_login_error)?;
        runtime.apply_login_state(attempt.state);
        if matches!(
            attempt.state,
            WeixinLoginState::Connected | WeixinLoginState::AlreadyBound
        ) {
            runtime.start_monitor().await;
        }
        Ok(attempt)
    }

    pub(super) async fn submit_verification(
        &self,
        attempt_id: &str,
        request: SubmitWeixinVerificationRequest,
    ) -> BootResult<WeixinLoginAttempt> {
        let runtime = self.runtime().await?;
        let attempt = runtime
            .login
            .submit_verify_code(attempt_id, request.code.expose())
            .await
            .map_err(map_login_error)?;
        runtime.apply_login_state(attempt.state);
        Ok(attempt)
    }

    pub(super) async fn cancel_login(&self, attempt_id: &str) -> BootResult<WeixinAccountResponse> {
        let runtime = self.runtime().await?;
        runtime
            .login
            .cancel(attempt_id)
            .await
            .map_err(map_login_error)?;
        runtime.set_state(WeixinCapabilityState::Unbound);
        runtime.account().await
    }

    pub(super) async fn disconnect(&self) -> BootResult<WeixinAccountResponse> {
        let runtime = self.runtime().await?;
        runtime.stop_monitor().await?;
        if let Some(runtime_store) = &runtime.runtime_store {
            runtime_store
                .clear()
                .await
                .map_err(|_| runtime_storage_error())?;
        }
        runtime.login.disconnect().await.map_err(map_login_error)?;
        runtime.set_state(WeixinCapabilityState::Unbound);
        runtime.account().await
    }

    pub(super) async fn pause(&self) -> BootResult<WeixinAccountResponse> {
        let runtime = self.runtime().await?;
        let monitor = runtime.monitor.as_ref().ok_or_else(monitor_unavailable)?;
        monitor.pause().await.map_err(map_monitor_error)?;
        runtime.set_state(WeixinCapabilityState::Paused);
        runtime.account().await
    }

    pub(super) async fn resume(&self) -> BootResult<WeixinAccountResponse> {
        let runtime = self.runtime().await?;
        let monitor = runtime.monitor.as_ref().ok_or_else(monitor_unavailable)?;
        runtime.set_state(WeixinCapabilityState::Paused);
        monitor.resume().await.map_err(map_monitor_error)?;
        runtime.account().await
    }

    pub(super) async fn bootstrap(&self) -> BootResult<()> {
        let runtime = match self.runtime_snapshot() {
            Some(runtime) => runtime,
            None if self.production.is_some() => match self.initialize_production().await {
                Ok(runtime) => runtime,
                Err(error) => {
                    eprintln!("warning: Weixin channel initialization is unavailable: {error}");
                    return Ok(());
                }
            },
            None => return Ok(()),
        };
        if runtime.monitor.is_some()
            && match runtime.credential_store.load().await {
                Ok(credentials) => {
                    self.clear_release_blocker();
                    credentials.is_some()
                }
                Err(_) if self.production.is_some() => {
                    self.set_release_blocker(credential_initialization_blocker());
                    return Ok(());
                }
                Err(_) => return Err(credential_storage_error()),
            }
        {
            runtime.set_state(WeixinCapabilityState::Paused);
            runtime.start_monitor().await;
        }
        Ok(())
    }

    pub(super) async fn shutdown(&self) -> BootResult<()> {
        let Some(runtime) = self.runtime_snapshot() else {
            return Ok(());
        };
        runtime.stop_monitor().await
    }

    async fn runtime(&self) -> BootResult<Arc<WeixinRuntime>> {
        match self.runtime_snapshot() {
            Some(runtime) => Ok(runtime),
            None if self.production.is_some() => self.initialize_production().await,
            None => Err(BootError::ServiceUnavailable(
                "Weixin iLink is not configured for this A3S runtime.".to_string(),
            )),
        }
    }

    async fn initialize_production(&self) -> BootResult<Arc<WeixinRuntime>> {
        if let Some(runtime) = self.runtime_snapshot() {
            return Ok(runtime);
        }
        let production = self.production.as_ref().ok_or_else(|| {
            BootError::ServiceUnavailable(
                "Weixin iLink is not configured for this A3S runtime.".to_string(),
            )
        })?;
        let _initialization = production.initialization_lock.lock().await;
        if let Some(runtime) = self.runtime_snapshot() {
            return Ok(runtime);
        }

        let runtime_store = match WeixinRuntimeStore::open(&production.runtime_directory).await {
            Ok(runtime_store) => runtime_store,
            Err(_) => {
                self.set_release_blocker(runtime_initialization_blocker());
                return Err(runtime_storage_error());
            }
        };
        let remote_read = self.remote_read.as_ref().cloned().ok_or_else(|| {
            BootError::Internal("Weixin remote read service is unavailable.".to_string())
        })?;
        let handler: Arc<dyn WeixinInboundHandler> =
            Arc::new(RemoteReadHandler::new(remote_read, runtime_store.clone()));
        let monitor = Arc::new(WeixinMonitorSupervisor::new(
            Arc::clone(&production.messaging_transport),
            Arc::clone(&production.credential_store),
            runtime_store.clone(),
            handler,
        ));
        let login = Arc::new(WeixinLoginCoordinator::new(
            Arc::clone(&production.login_transport),
            Arc::clone(&production.credential_store),
        ));
        let runtime = Arc::new(WeixinRuntime {
            login,
            credential_store: Arc::clone(&production.credential_store),
            monitor: Some(monitor),
            runtime_store: Some(runtime_store),
            state: RwLock::new(WeixinCapabilityState::Unbound),
            protocol_mode: WeixinProtocolMode::Tencent,
        });
        *self
            .runtime
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(Arc::clone(&runtime));
        self.clear_release_blocker();
        Ok(runtime)
    }

    fn runtime_snapshot(&self) -> Option<Arc<WeixinRuntime>> {
        self.runtime
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    fn release_blocker(&self) -> Option<SafeBlocker> {
        self.release_blocker
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    fn set_release_blocker(&self, blocker: SafeBlocker) {
        *self
            .release_blocker
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(blocker);
    }

    fn clear_release_blocker(&self) {
        *self
            .release_blocker
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = None;
    }
}

impl WeixinRuntime {
    fn state(&self) -> WeixinCapabilityState {
        *self
            .state
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
    }

    fn set_state(&self, state: WeixinCapabilityState) {
        *self
            .state
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = state;
    }

    fn apply_login_state(&self, state: WeixinLoginState) {
        let state = match state {
            WeixinLoginState::WaitingForScan
            | WeixinLoginState::Scanned
            | WeixinLoginState::VerificationRequired
            | WeixinLoginState::VerificationSubmitted
            | WeixinLoginState::Redirected => WeixinCapabilityState::Binding,
            WeixinLoginState::Connected | WeixinLoginState::AlreadyBound => {
                WeixinCapabilityState::Paused
            }
            WeixinLoginState::Expired | WeixinLoginState::VerificationBlocked => {
                WeixinCapabilityState::Unbound
            }
        };
        self.set_state(state);
    }

    fn effective_state(&self) -> WeixinCapabilityState {
        let state = self.state();
        if matches!(
            state,
            WeixinCapabilityState::Unbound | WeixinCapabilityState::Binding
        ) {
            return state;
        }
        self.monitor
            .as_ref()
            .map(|monitor| capability_state_from_monitor(&monitor.health()))
            .unwrap_or(state)
    }

    async fn start_monitor(&self) {
        let Some(monitor) = &self.monitor else {
            return;
        };
        match monitor.start().await {
            Ok(_) => {}
            Err(_) => self.set_state(WeixinCapabilityState::Degraded),
        }
    }

    async fn stop_monitor(&self) -> BootResult<()> {
        if let Some(monitor) = &self.monitor {
            monitor.shutdown().await.map_err(map_monitor_error)?;
        }
        Ok(())
    }

    async fn account(&self) -> BootResult<WeixinAccountResponse> {
        let credentials = self
            .credential_store
            .load()
            .await
            .map_err(|_| credential_storage_error())?;
        let bound = credentials.is_some();
        let mut state = self.effective_state();
        if bound && matches!(state, WeixinCapabilityState::Unbound) {
            state = WeixinCapabilityState::Paused;
            self.set_state(state);
        } else if !bound && state != WeixinCapabilityState::Binding {
            state = WeixinCapabilityState::Unbound;
            self.set_state(state);
        }
        let monitor_health = self.monitor.as_ref().map(|monitor| monitor.health());
        Ok(WeixinAccountResponse {
            schema_version: 1,
            state,
            protocol_mode: self.protocol_mode,
            bound,
            owner_label: credentials.as_ref().map(owner_label),
            monitor_state: monitor_state(bound, monitor_health.as_ref()),
            mutations_enabled: false,
            last_update_at: monitor_health
                .as_ref()
                .and_then(|health| health.last_update_at_ms)
                .and_then(format_timestamp),
            last_error: monitor_health
                .as_ref()
                .and_then(|health| health.last_error)
                .map(monitor_blocker),
        })
    }
}

fn capability_state_from_monitor(health: &MonitorHealth) -> WeixinCapabilityState {
    match health.state {
        MonitorLifecycleState::Stopped | MonitorLifecycleState::Paused => {
            WeixinCapabilityState::Paused
        }
        MonitorLifecycleState::Starting => WeixinCapabilityState::Binding,
        MonitorLifecycleState::Running => WeixinCapabilityState::Active,
        MonitorLifecycleState::Degraded => WeixinCapabilityState::Degraded,
        MonitorLifecycleState::StaleCredential => WeixinCapabilityState::StaleCredential,
    }
}

fn monitor_state(bound: bool, health: Option<&MonitorHealth>) -> WeixinMonitorState {
    let Some(health) = health else {
        return if bound {
            WeixinMonitorState::Paused
        } else {
            WeixinMonitorState::Stopped
        };
    };
    match health.state {
        MonitorLifecycleState::Stopped => WeixinMonitorState::Stopped,
        MonitorLifecycleState::Starting => WeixinMonitorState::Starting,
        MonitorLifecycleState::Running => WeixinMonitorState::Running,
        MonitorLifecycleState::Paused => WeixinMonitorState::Paused,
        MonitorLifecycleState::Degraded => WeixinMonitorState::Degraded,
        MonitorLifecycleState::StaleCredential => WeixinMonitorState::StaleCredential,
    }
}

fn monitor_blocker(code: MonitorErrorCode) -> super::dto::SafeBlocker {
    let code = match code {
        MonitorErrorCode::Network => "monitor_network",
        MonitorErrorCode::Upstream => "monitor_upstream",
        MonitorErrorCode::Protocol => "monitor_protocol",
        MonitorErrorCode::Storage => "monitor_storage",
        MonitorErrorCode::StaleCredential => "monitor_stale_credential",
        MonitorErrorCode::Shutdown => "monitor_shutdown",
    };
    super::dto::SafeBlocker {
        code: code.to_string(),
        message: "The WeChat monitor requires local attention.".to_string(),
    }
}

fn format_timestamp(timestamp_ms: u64) -> Option<String> {
    let timestamp_ms = i64::try_from(timestamp_ms).ok()?;
    Utc.timestamp_millis_opt(timestamp_ms)
        .single()
        .map(|timestamp| timestamp.to_rfc3339_opts(SecondsFormat::Millis, true))
}

fn owner_label(credentials: &WeixinCredentials) -> String {
    let digest = Sha256::digest(credentials.owner_id.expose().as_bytes());
    let fingerprint = digest[..4]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    format!("WeChat owner • {fingerprint}")
}

fn map_login_error(error: WeixinLoginError) -> BootError {
    match error {
        WeixinLoginError::AlreadyBound => {
            BootError::Conflict("A WeChat account is already bound.".to_string())
        }
        WeixinLoginError::AttemptNotFound => {
            BootError::NotFound("WeChat login attempt was not found.".to_string())
        }
        WeixinLoginError::AttemptSuperseded => {
            BootError::Conflict("WeChat login attempt was superseded.".to_string())
        }
        WeixinLoginError::InvalidVerifyCode => {
            BootError::BadRequest("WeChat verification code is invalid.".to_string())
        }
        WeixinLoginError::VerificationNotRequested => BootError::Conflict(
            "The current WeChat login attempt does not require verification.".to_string(),
        ),
        WeixinLoginError::VerifyLimitReached => {
            BootError::TooManyRequests("WeChat verification retry limit was reached.".to_string())
        }
        WeixinLoginError::InvalidProtocolState | WeixinLoginError::MissingExistingCredential => {
            BootError::BadGateway("iLink returned an inconsistent login state.".to_string())
        }
        WeixinLoginError::Protocol(error) => match error {
            a3s_boot::ilink::IlinkError::Timeout => {
                BootError::GatewayTimeout("iLink login request timed out.".to_string())
            }
            a3s_boot::ilink::IlinkError::StaleCredential => BootError::PreconditionFailed(
                "The WeChat credential is stale and must be rebound.".to_string(),
            ),
            _ => BootError::BadGateway("iLink login request failed.".to_string()),
        },
        WeixinLoginError::Credential(_) => credential_storage_error(),
    }
}

fn credential_storage_error() -> BootError {
    BootError::Internal("WeChat credential storage is unavailable.".to_string())
}

fn runtime_storage_error() -> BootError {
    BootError::Internal("WeChat runtime storage is unavailable.".to_string())
}

fn runtime_initialization_blocker() -> SafeBlocker {
    SafeBlocker {
        code: "ilink_runtime_storage_unavailable".to_string(),
        message: "The local Weixin runtime store could not be opened safely.".to_string(),
    }
}

fn credential_initialization_blocker() -> SafeBlocker {
    SafeBlocker {
        code: "ilink_credential_storage_unavailable".to_string(),
        message: "The local Weixin credential store could not be opened safely.".to_string(),
    }
}

fn monitor_unavailable() -> BootError {
    BootError::ServiceUnavailable(
        "The WeChat monitor is not available in this runtime.".to_string(),
    )
}

fn map_monitor_error(error: MonitorError) -> BootError {
    match error {
        MonitorError::NotBound => {
            BootError::PreconditionFailed("No WeChat account is bound.".to_string())
        }
        MonitorError::Protocol(a3s_boot::ilink::IlinkError::StaleCredential) => {
            BootError::PreconditionFailed(
                "The WeChat credential is stale and must be rebound.".to_string(),
            )
        }
        MonitorError::Protocol(a3s_boot::ilink::IlinkError::Timeout) => {
            BootError::GatewayTimeout("The WeChat monitor request timed out.".to_string())
        }
        MonitorError::Protocol(_) => {
            BootError::BadGateway("The WeChat monitor request failed.".to_string())
        }
        MonitorError::Credential(_) => credential_storage_error(),
        MonitorError::Runtime(_) => runtime_storage_error(),
        MonitorError::TaskJoin | MonitorError::ShutdownTimeout => {
            BootError::Internal("The WeChat monitor could not shut down cleanly.".to_string())
        }
    }
}
