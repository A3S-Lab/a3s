mod message;

use std::fmt;
use std::sync::{Arc, RwLock};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::sync::Mutex;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;
use zeroize::Zeroizing;

use super::credential_store::{CredentialStoreError, WeixinCredentialStore, WeixinCredentials};
use super::runtime_store::{
    InboxState, OutboundDraft, OutboundState, RuntimeStoreError, WeixinRuntimeStore,
};
use a3s_boot::ilink::{
    GetUpdatesResponse, IlinkAuth, IlinkError, IlinkMessagingTransport, SecretValue,
};
#[cfg(test)]
pub(super) use message::AlphaDisabledHandler;
use message::{accepted_inbound_message, MAX_HANDLER_RESPONSE_BYTES};
pub(super) use message::{InboundHandlerError, WeixinInboundHandler};

const DEFAULT_LONG_POLL_TIMEOUT: Duration = Duration::from_secs(35);

pub(super) struct WeixinMonitorSupervisor {
    transport: Arc<dyn IlinkMessagingTransport>,
    credential_store: Arc<dyn WeixinCredentialStore>,
    runtime_store: WeixinRuntimeStore,
    handler: Arc<dyn WeixinInboundHandler>,
    health: Arc<RwLock<MonitorHealth>>,
    running: Mutex<Option<RunningMonitor>>,
    config: MonitorConfig,
}

struct RunningMonitor {
    cancellation: CancellationToken,
    handle: JoinHandle<()>,
}

impl WeixinMonitorSupervisor {
    pub(super) fn new(
        transport: Arc<dyn IlinkMessagingTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        runtime_store: WeixinRuntimeStore,
        handler: Arc<dyn WeixinInboundHandler>,
    ) -> Self {
        Self::with_config(
            transport,
            credential_store,
            runtime_store,
            handler,
            MonitorConfig::default(),
        )
    }

    fn with_config(
        transport: Arc<dyn IlinkMessagingTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        runtime_store: WeixinRuntimeStore,
        handler: Arc<dyn WeixinInboundHandler>,
        config: MonitorConfig,
    ) -> Self {
        Self {
            transport,
            credential_store,
            runtime_store,
            handler,
            health: Arc::new(RwLock::new(MonitorHealth::stopped())),
            running: Mutex::new(None),
            config,
        }
    }

    #[cfg(test)]
    pub(super) fn for_test(
        transport: Arc<dyn IlinkMessagingTransport>,
        credential_store: Arc<dyn WeixinCredentialStore>,
        runtime_store: WeixinRuntimeStore,
        handler: Arc<dyn WeixinInboundHandler>,
        initial_backoff: Duration,
        max_backoff: Duration,
    ) -> Self {
        let config = MonitorConfig {
            initial_backoff,
            max_backoff,
            notify_stop_timeout: Duration::from_millis(100),
            shutdown_timeout: Duration::from_secs(2),
        };
        Self::with_config(transport, credential_store, runtime_store, handler, config)
    }

    pub(super) fn health(&self) -> MonitorHealth {
        self.health
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    pub(super) async fn start(&self) -> Result<MonitorHealth, MonitorError> {
        let mut running = self.running.lock().await;
        if running
            .as_ref()
            .is_some_and(|monitor| !monitor.handle.is_finished())
        {
            return Ok(self.health());
        }
        if let Some(finished) = running.take() {
            finished.handle.await.map_err(|_| MonitorError::TaskJoin)?;
        }

        let credentials = self
            .credential_store
            .load()
            .await?
            .ok_or(MonitorError::NotBound)?;
        let base_url = self
            .transport
            .validate_account_base_url(&credentials.base_url)?;
        let auth = IlinkAuth::new(base_url, credentials.bot_token.clone());
        set_health(&self.health, MonitorLifecycleState::Starting, 0, None, None);
        let cancellation = CancellationToken::new();
        let worker = MonitorWorker {
            transport: Arc::clone(&self.transport),
            runtime_store: self.runtime_store.clone(),
            handler: Arc::clone(&self.handler),
            health: Arc::clone(&self.health),
            config: self.config,
            credentials,
            auth,
        };
        let task_cancellation = cancellation.clone();
        let handle = tokio::spawn(async move {
            worker.run(task_cancellation).await;
        });
        *running = Some(RunningMonitor {
            cancellation,
            handle,
        });
        Ok(self.health())
    }

    pub(super) async fn pause(&self) -> Result<MonitorHealth, MonitorError> {
        self.stop_as(MonitorLifecycleState::Paused).await
    }

    pub(super) async fn resume(&self) -> Result<MonitorHealth, MonitorError> {
        self.start().await
    }

    pub(super) async fn shutdown(&self) -> Result<MonitorHealth, MonitorError> {
        self.stop_as(MonitorLifecycleState::Stopped).await
    }

    async fn stop_as(
        &self,
        terminal_state: MonitorLifecycleState,
    ) -> Result<MonitorHealth, MonitorError> {
        let running = self.running.lock().await.take();
        if let Some(running) = running {
            running.cancellation.cancel();
            let mut handle = running.handle;
            match tokio::time::timeout(self.config.shutdown_timeout, &mut handle).await {
                Ok(Ok(())) => {}
                Ok(Err(_)) => {
                    set_health(
                        &self.health,
                        MonitorLifecycleState::Degraded,
                        0,
                        Some(MonitorErrorCode::Shutdown),
                        None,
                    );
                    return Err(MonitorError::TaskJoin);
                }
                Err(_) => {
                    handle.abort();
                    let _ = handle.await;
                    set_health(
                        &self.health,
                        MonitorLifecycleState::Degraded,
                        0,
                        Some(MonitorErrorCode::Shutdown),
                        None,
                    );
                    return Err(MonitorError::ShutdownTimeout);
                }
            }
        }
        set_health(&self.health, terminal_state, 0, None, None);
        Ok(self.health())
    }
}

#[derive(Clone, Copy)]
struct MonitorConfig {
    initial_backoff: Duration,
    max_backoff: Duration,
    notify_stop_timeout: Duration,
    shutdown_timeout: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            initial_backoff: Duration::from_secs(1),
            max_backoff: Duration::from_secs(5 * 60),
            notify_stop_timeout: Duration::from_secs(2),
            shutdown_timeout: Duration::from_secs(10),
        }
    }
}

struct MonitorWorker {
    transport: Arc<dyn IlinkMessagingTransport>,
    runtime_store: WeixinRuntimeStore,
    handler: Arc<dyn WeixinInboundHandler>,
    health: Arc<RwLock<MonitorHealth>>,
    config: MonitorConfig,
    credentials: WeixinCredentials,
    auth: IlinkAuth,
}

impl MonitorWorker {
    async fn run(self, cancellation: CancellationToken) {
        let started = match self.notify_start(&cancellation).await {
            WorkerExit::Continue => true,
            WorkerExit::Cancelled => false,
            exit => {
                self.apply_exit(exit);
                return;
            }
        };
        if !started {
            set_health(&self.health, MonitorLifecycleState::Stopped, 0, None, None);
            return;
        }

        let exit = self.poll_loop(&cancellation).await;
        let _ = tokio::time::timeout(
            self.config.notify_stop_timeout,
            self.transport.notify_stop(&self.auth),
        )
        .await;
        self.apply_exit(exit);
    }

    async fn notify_start(&self, cancellation: &CancellationToken) -> WorkerExit {
        let mut failures = 0u32;
        loop {
            let result = tokio::select! {
                _ = cancellation.cancelled() => return WorkerExit::Cancelled,
                result = self.transport.notify_start(&self.auth) => result,
            };
            match result {
                Ok(_) => {
                    set_health(&self.health, MonitorLifecycleState::Running, 0, None, None);
                    return WorkerExit::Continue;
                }
                Err(IlinkError::StaleCredential) => return WorkerExit::StaleCredential,
                Err(error) => {
                    failures = failures.saturating_add(1);
                    set_health(
                        &self.health,
                        MonitorLifecycleState::Degraded,
                        failures,
                        Some(protocol_error_code(&error)),
                        None,
                    );
                    if !wait_backoff(cancellation, self.config, failures).await {
                        return WorkerExit::Cancelled;
                    }
                }
            }
        }
    }

    async fn poll_loop(&self, cancellation: &CancellationToken) -> WorkerExit {
        let mut long_poll_timeout = DEFAULT_LONG_POLL_TIMEOUT;
        let mut failures = 0u32;
        loop {
            match self.process_pending(cancellation).await {
                Ok(()) => {}
                Err(WorkerError::Cancelled) => return WorkerExit::Cancelled,
                Err(WorkerError::StaleCredential) => return WorkerExit::StaleCredential,
                Err(WorkerError::Runtime) => {
                    return WorkerExit::Degraded(MonitorErrorCode::Storage)
                }
                Err(WorkerError::Protocol(error)) => {
                    failures = failures.saturating_add(1);
                    set_health(
                        &self.health,
                        MonitorLifecycleState::Degraded,
                        failures,
                        Some(protocol_error_code(&error)),
                        None,
                    );
                    if !wait_backoff(cancellation, self.config, failures).await {
                        return WorkerExit::Cancelled;
                    }
                    continue;
                }
            }

            let checkpoint = self.runtime_store.checkpoint().await;
            let cursor = Zeroizing::new(
                checkpoint
                    .cursor
                    .as_ref()
                    .map(|value| value.expose().to_string())
                    .unwrap_or_default(),
            );
            let result = tokio::select! {
                _ = cancellation.cancelled() => return WorkerExit::Cancelled,
                result = self.transport.get_updates(&self.auth, &cursor, long_poll_timeout) => result,
            };
            match result {
                Ok(response) => {
                    if let Some(timeout_ms) = response.long_polling_timeout_ms {
                        long_poll_timeout = Duration::from_millis(timeout_ms);
                    }
                    match self.process_update(response, cancellation).await {
                        Ok(()) => {
                            failures = 0;
                            set_health(
                                &self.health,
                                MonitorLifecycleState::Running,
                                0,
                                None,
                                Some(unix_time_ms()),
                            );
                        }
                        Err(WorkerError::Cancelled) => return WorkerExit::Cancelled,
                        Err(WorkerError::StaleCredential) => return WorkerExit::StaleCredential,
                        Err(WorkerError::Runtime) => {
                            return WorkerExit::Degraded(MonitorErrorCode::Storage)
                        }
                        Err(WorkerError::Protocol(error)) => {
                            failures = failures.saturating_add(1);
                            set_health(
                                &self.health,
                                MonitorLifecycleState::Degraded,
                                failures,
                                Some(protocol_error_code(&error)),
                                None,
                            );
                            if !wait_backoff(cancellation, self.config, failures).await {
                                return WorkerExit::Cancelled;
                            }
                        }
                    }
                }
                Err(IlinkError::StaleCredential) => return WorkerExit::StaleCredential,
                Err(error) => {
                    failures = failures.saturating_add(1);
                    set_health(
                        &self.health,
                        MonitorLifecycleState::Degraded,
                        failures,
                        Some(protocol_error_code(&error)),
                        None,
                    );
                    if !wait_backoff(cancellation, self.config, failures).await {
                        return WorkerExit::Cancelled;
                    }
                }
            }
        }
    }

    async fn process_update(
        &self,
        response: GetUpdatesResponse,
        cancellation: &CancellationToken,
    ) -> Result<(), WorkerError> {
        let messages = response
            .messages
            .iter()
            .filter_map(|message| accepted_inbound_message(&self.credentials, message))
            .collect::<Vec<_>>();
        match response.update_cursor {
            Some(cursor) => {
                let checkpoint = self.runtime_store.checkpoint().await;
                let cursor_unchanged = checkpoint
                    .cursor
                    .as_ref()
                    .is_some_and(|current| current.expose() == cursor.expose());
                if !messages.is_empty() || !cursor_unchanged {
                    self.runtime_store
                        .stage_inbound(cursor, messages)
                        .await
                        .map_err(|_| WorkerError::Runtime)?;
                }
            }
            None if response.messages.is_empty() => {}
            None => {
                return Err(WorkerError::Protocol(IlinkError::InvalidResponse(
                    "monitor_cursor",
                )))
            }
        }
        self.process_pending(cancellation).await
    }

    async fn process_pending(&self, cancellation: &CancellationToken) -> Result<(), WorkerError> {
        self.process_inbox(cancellation).await?;
        self.process_outbox(cancellation).await
    }

    async fn process_inbox(&self, cancellation: &CancellationToken) -> Result<(), WorkerError> {
        let mut pending = self
            .runtime_store
            .checkpoint()
            .await
            .inbox
            .into_values()
            .filter(|record| record.state == InboxState::Staged)
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| {
            left.staged_sequence
                .cmp(&right.staged_sequence)
                .then_with(|| left.staged_index.cmp(&right.staged_index))
                .then_with(|| left.message.key.cmp(&right.message.key))
        });
        for record in pending {
            let response = tokio::select! {
                _ = cancellation.cancelled() => return Err(WorkerError::Cancelled),
                response = self.handler.handle(&record.message) => response,
            };
            match response {
                Ok(Some(text)) if valid_handler_response(&text) => {
                    let draft = OutboundDraft {
                        recipient_id: record.message.sender_id.clone(),
                        context_token: record.message.context_token.clone(),
                        text: SecretValue::new(text).map_err(|_| WorkerError::Runtime)?,
                        run_id: record.message.run_id.clone(),
                    };
                    self.runtime_store
                        .queue_outbound_for_inbox(&record.message.key, draft)
                        .await
                        .map_err(|_| WorkerError::Runtime)?;
                }
                Ok(None) => self
                    .runtime_store
                    .set_inbox_state(&record.message.key, InboxState::Completed)
                    .await
                    .map_err(|_| WorkerError::Runtime)?,
                Ok(Some(_)) | Err(_) => self
                    .runtime_store
                    .set_inbox_state(&record.message.key, InboxState::Quarantined)
                    .await
                    .map_err(|_| WorkerError::Runtime)?,
            }
        }
        Ok(())
    }

    async fn process_outbox(&self, cancellation: &CancellationToken) -> Result<(), WorkerError> {
        let mut pending = self
            .runtime_store
            .checkpoint()
            .await
            .outbox
            .into_values()
            .filter(|message| message.state == OutboundState::Pending)
            .collect::<Vec<_>>();
        pending.sort_by(|left, right| {
            left.source_inbox_sequence
                .is_none()
                .cmp(&right.source_inbox_sequence.is_none())
                .then_with(|| left.source_inbox_sequence.cmp(&right.source_inbox_sequence))
                .then_with(|| left.source_inbox_index.cmp(&right.source_inbox_index))
                .then_with(|| left.created_at_ms.cmp(&right.created_at_ms))
                .then_with(|| left.client_id.cmp(&right.client_id))
        });
        for message in pending {
            if cancellation.is_cancelled() {
                return Err(WorkerError::Cancelled);
            }
            self.runtime_store
                .set_outbound_state(&message.client_id, OutboundState::Sending)
                .await
                .map_err(|_| WorkerError::Runtime)?;
            let result = tokio::select! {
                _ = cancellation.cancelled() => {
                    self.runtime_store
                        .set_outbound_state(&message.client_id, OutboundState::OutcomeUnknown)
                        .await
                        .map_err(|_| WorkerError::Runtime)?;
                    return Err(WorkerError::Cancelled);
                }
                result = self.transport.send_text(
                    &self.auth,
                    &message.recipient_id,
                    message.context_token.as_ref(),
                    &message.client_id,
                    message.run_id.as_deref(),
                    message.text.expose(),
                ) => result,
            };
            match result {
                Ok(_) => self
                    .runtime_store
                    .set_outbound_state(&message.client_id, OutboundState::Sent)
                    .await
                    .map_err(|_| WorkerError::Runtime)?,
                Err(IlinkError::StaleCredential) => {
                    self.runtime_store
                        .set_outbound_state(&message.client_id, OutboundState::Failed)
                        .await
                        .map_err(|_| WorkerError::Runtime)?;
                    return Err(WorkerError::StaleCredential);
                }
                Err(error) => {
                    let state = if matches!(error, IlinkError::Timeout | IlinkError::Transport) {
                        OutboundState::OutcomeUnknown
                    } else {
                        OutboundState::Failed
                    };
                    self.runtime_store
                        .set_outbound_state(&message.client_id, state)
                        .await
                        .map_err(|_| WorkerError::Runtime)?;
                    return Err(WorkerError::Protocol(error));
                }
            }
        }
        Ok(())
    }

    fn apply_exit(&self, exit: WorkerExit) {
        match exit {
            WorkerExit::Continue | WorkerExit::Cancelled => {
                set_health(&self.health, MonitorLifecycleState::Stopped, 0, None, None)
            }
            WorkerExit::StaleCredential => set_health(
                &self.health,
                MonitorLifecycleState::StaleCredential,
                0,
                Some(MonitorErrorCode::StaleCredential),
                None,
            ),
            WorkerExit::Degraded(code) => set_health(
                &self.health,
                MonitorLifecycleState::Degraded,
                0,
                Some(code),
                None,
            ),
        }
    }
}

#[derive(Clone, Copy)]
enum WorkerExit {
    Continue,
    Cancelled,
    StaleCredential,
    Degraded(MonitorErrorCode),
}

enum WorkerError {
    Cancelled,
    StaleCredential,
    Protocol(IlinkError),
    Runtime,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) enum MonitorLifecycleState {
    Stopped,
    Starting,
    Running,
    Paused,
    Degraded,
    StaleCredential,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum MonitorErrorCode {
    Network,
    Upstream,
    Protocol,
    Storage,
    StaleCredential,
    Shutdown,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct MonitorHealth {
    pub(super) state: MonitorLifecycleState,
    pub(super) consecutive_failures: u32,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_error: Option<MonitorErrorCode>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) last_update_at_ms: Option<u64>,
}

impl MonitorHealth {
    fn stopped() -> Self {
        Self {
            state: MonitorLifecycleState::Stopped,
            consecutive_failures: 0,
            last_error: None,
            last_update_at_ms: None,
        }
    }
}

impl fmt::Debug for MonitorHealth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MonitorHealth")
            .field("state", &self.state)
            .field("consecutive_failures", &self.consecutive_failures)
            .field("last_error", &self.last_error)
            .field("has_last_update", &self.last_update_at_ms.is_some())
            .finish()
    }
}

fn set_health(
    health: &RwLock<MonitorHealth>,
    state: MonitorLifecycleState,
    consecutive_failures: u32,
    last_error: Option<MonitorErrorCode>,
    last_update_at_ms: Option<u64>,
) {
    let mut health = health.write().unwrap_or_else(|poison| poison.into_inner());
    let previous_update = health.last_update_at_ms;
    *health = MonitorHealth {
        state,
        consecutive_failures,
        last_error,
        last_update_at_ms: last_update_at_ms.or(previous_update),
    };
}

fn protocol_error_code(error: &IlinkError) -> MonitorErrorCode {
    match error {
        IlinkError::Timeout | IlinkError::Transport => MonitorErrorCode::Network,
        IlinkError::HttpStatus(_) | IlinkError::Protocol { .. } => MonitorErrorCode::Upstream,
        IlinkError::StaleCredential => MonitorErrorCode::StaleCredential,
        _ => MonitorErrorCode::Protocol,
    }
}

fn valid_handler_response(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAX_HANDLER_RESPONSE_BYTES && !text.contains('\0')
}

async fn wait_backoff(
    cancellation: &CancellationToken,
    config: MonitorConfig,
    failures: u32,
) -> bool {
    let exponent = failures.saturating_sub(1).min(20);
    let multiplier = 1u32 << exponent;
    let ceiling = config
        .initial_backoff
        .saturating_mul(multiplier)
        .min(config.max_backoff);
    let ceiling_ms = ceiling.as_millis().min(u64::MAX as u128) as u64;
    let delay_ms = if ceiling_ms == 0 {
        0
    } else {
        rand::random::<u64>() % ceiling_ms.saturating_add(1)
    };
    tokio::select! {
        _ = cancellation.cancelled() => false,
        _ = tokio::time::sleep(Duration::from_millis(delay_ms)) => true,
    }
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[derive(Debug, Error)]
pub(super) enum MonitorError {
    #[error("no Weixin account is bound")]
    NotBound,
    #[error("Weixin monitor protocol setup failed")]
    Protocol(#[from] IlinkError),
    #[error("Weixin credential storage failed")]
    Credential(#[from] CredentialStoreError),
    #[error("Weixin runtime storage failed")]
    Runtime(#[from] RuntimeStoreError),
    #[error("Weixin monitor task failed")]
    TaskJoin,
    #[error("Weixin monitor shutdown timed out")]
    ShutdownTimeout,
}
