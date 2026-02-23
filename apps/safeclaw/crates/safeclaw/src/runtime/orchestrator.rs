//! SafeClaw runtime server
//!
//! **Single responsibility**: lifecycle (start/stop) and infrastructure
//! setup (audit pipeline, channel adapters, event loop).
//!
//! Message processing is delegated entirely to `MessageProcessor`.

use crate::audit::{AlertMonitor, AuditEventBus, AuditLog, AuditPersistence};
use crate::channels::{
    supervisor, ChannelAdapter, DingTalkAdapter, DiscordAdapter, FeishuAdapter, SlackAdapter,
    TelegramAdapter, WeComAdapter, WebChatAdapter,
};
use crate::config::SafeClawConfig;
use crate::error::{Error, Result};
use crate::privacy::{
    Classifier, CompositeClassifier, PolicyEngine, RegexBackend, SemanticAnalyzer, SemanticBackend,
};
use crate::runtime::processor::MessageProcessor;
use crate::session::{SessionManager, SessionRouter};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

// ---------------------------------------------------------------------------
// Re-export ProcessedResponse from processor
// ---------------------------------------------------------------------------

pub use crate::runtime::processor::ProcessedResponse;

// ---------------------------------------------------------------------------
// RuntimeState / RuntimeStatus
// ---------------------------------------------------------------------------

/// Runtime server state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RuntimeState {
    Stopped,
    Starting,
    Running,
    ShuttingDown,
}

/// Runtime status information
#[derive(Debug, Clone, Serialize)]
pub struct RuntimeStatus {
    pub state: String,
    pub tee_enabled: bool,
    pub security_level: crate::tee::SecurityLevel,
    pub session_count: usize,
    pub channels: Vec<String>,
    pub a3s_gateway_mode: bool,
}

// ---------------------------------------------------------------------------
// Runtime
// ---------------------------------------------------------------------------

/// SafeClaw runtime server — orchestrates lifecycle and infrastructure.
///
/// Message processing is owned by the embedded `MessageProcessor`.
pub struct Runtime {
    config: SafeClawConfig,
    state: Arc<RwLock<RuntimeState>>,
    session_manager: Arc<SessionManager>,
    channels: Arc<RwLock<HashMap<String, Arc<dyn ChannelAdapter>>>>,
    event_tx: mpsc::Sender<crate::channels::ChannelEvent>,
    event_rx: Arc<RwLock<Option<mpsc::Receiver<crate::channels::ChannelEvent>>>>,
    /// Global audit log shared with the REST API
    global_audit_log: Arc<RwLock<AuditLog>>,
    /// Centralized audit event bus
    audit_bus: Arc<AuditEventBus>,
    /// Alert monitor for rate-based anomaly detection
    alert_monitor: Arc<AlertMonitor>,
    /// Message processing pipeline
    processor: Arc<MessageProcessor>,
}

impl Runtime {
    /// Create a new runtime with the given configuration.
    pub fn new(config: SafeClawConfig) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel(1000);

        let audit_capacity = config.audit.bus_capacity;
        let global_audit_log = Arc::new(RwLock::new(AuditLog::new(audit_capacity)));
        let audit_bus = Arc::new(AuditEventBus::new(audit_capacity, global_audit_log.clone()));
        let alert_monitor = Arc::new(AlertMonitor::new(config.audit.alert.clone()));

        let session_manager = Arc::new(SessionManager::new(config.tee.clone(), audit_bus.clone()));

        // Regex-only classifier — for synchronous redaction helpers
        let classifier = Arc::new(
            Classifier::new(config.privacy.rules.clone(), config.privacy.default_level)
                .map_err(|e| Error::Privacy(format!("Failed to create classifier: {}", e)))?,
        );

        // Full composite chain (regex + semantic) — for routing decisions
        let composite = Arc::new({
            let regex =
                RegexBackend::new(config.privacy.rules.clone(), config.privacy.default_level)
                    .map_err(|e| Error::Privacy(format!("Failed to build regex backend: {}", e)))?;
            let semantic = SemanticBackend::new(SemanticAnalyzer::new());
            CompositeClassifier::new(vec![Box::new(regex), Box::new(semantic)])
        });

        let policy_engine = Arc::new(PolicyEngine::new());

        let session_router = Arc::new(SessionRouter::new(
            session_manager.clone(),
            classifier,
            composite,
            policy_engine,
        ));

        let processor = Arc::new(MessageProcessor::new(
            session_router,
            session_manager.clone(),
            config.channels.feishu.clone(),
        ));

        Ok(Self {
            config,
            state: Arc::new(RwLock::new(RuntimeState::Stopped)),
            session_manager,
            channels: Arc::new(RwLock::new(HashMap::new())),
            event_tx,
            event_rx: Arc::new(RwLock::new(Some(event_rx))),
            global_audit_log,
            audit_bus,
            alert_monitor,
            processor,
        })
    }

    // -----------------------------------------------------------------------
    // Lifecycle
    // -----------------------------------------------------------------------

    pub async fn state(&self) -> RuntimeState {
        *self.state.read().await
    }

    pub async fn start(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state != RuntimeState::Stopped {
            return Err(Error::Runtime("Runtime already running".to_string()));
        }
        *state = RuntimeState::Starting;
        drop(state);

        tracing::info!("Starting SafeClaw runtime");

        // Initialize TEE subsystem
        if self.config.tee.enabled {
            self.session_manager.init_tee().await?;

            let level = self.security_level();
            if level != crate::tee::SecurityLevel::TeeHardware {
                tracing::warn!(
                    security_level = %level,
                    fallback_policy = ?self.config.tee.fallback_policy,
                    "TEE enabled in config but hardware TEE not detected. \
                     Sensitive data routing will use fallback policy."
                );
            }
        }

        // Start audit event pipeline
        self.audit_bus
            .spawn_session_forwarder(self.session_manager.isolation().clone());
        if self.config.audit.alert.enabled {
            self.alert_monitor.spawn(self.audit_bus.subscribe());
        }

        // Start audit persistence
        if self.config.audit.persistence.enabled {
            match AuditPersistence::new(
                &self.config.storage.base_dir,
                self.config.audit.persistence.clone(),
            )
            .await
            {
                Ok(persistence) => {
                    let restored = persistence
                        .load_recent(self.config.audit.bus_capacity)
                        .await;
                    if !restored.is_empty() {
                        let mut log = self.global_audit_log.write().await;
                        for event in &restored {
                            log.record(event.clone());
                        }
                        tracing::info!(count = restored.len(), "Restored audit events from disk");
                    }
                    let persistence = std::sync::Arc::new(persistence);
                    crate::audit::persistence::spawn_persistence_subscriber(
                        self.audit_bus.subscribe(),
                        persistence,
                    );
                }
                Err(e) => {
                    tracing::error!(
                        "Failed to initialize audit persistence: {}. \
                         Audit events will NOT survive restarts.",
                        e
                    );
                }
            }
        }

        // Start a3s-event bridge
        if self.config.audit.event_bridge.enabled {
            let nats_url = &self.config.audit.event_bridge.nats_url;
            if nats_url.is_empty() {
                let provider = a3s_event::MemoryProvider::default();
                let event_bus = Arc::new(a3s_event::EventBus::new(provider));
                self.audit_bus.spawn_event_bridge(event_bus);
                tracing::info!("a3s-event bridge started (in-memory provider)");
            } else {
                match a3s_event::NatsProvider::connect(a3s_event::NatsConfig {
                    url: nats_url.clone(),
                    ..Default::default()
                })
                .await
                {
                    Ok(provider) => {
                        let event_bus = Arc::new(a3s_event::EventBus::new(provider));
                        self.audit_bus.spawn_event_bridge(event_bus);
                        tracing::info!(url = %nats_url, "a3s-event bridge started (NATS provider)");
                    }
                    Err(e) => {
                        tracing::warn!(
                            url = %nats_url,
                            error = %e,
                            "Failed to connect to NATS, falling back to in-memory provider."
                        );
                        let provider = a3s_event::MemoryProvider::default();
                        let event_bus = Arc::new(a3s_event::EventBus::new(provider));
                        self.audit_bus.spawn_event_bridge(event_bus);
                    }
                }
            }
        }

        self.init_channels().await?;
        self.start_event_loop().await;

        *self.state.write().await = RuntimeState::Running;
        tracing::info!(
            "SafeClaw runtime started on {}:{}",
            self.config.gateway.host,
            self.config.gateway.port
        );

        // Log webhook URLs for each active channel
        let channels = self.active_channel_names().await;

        // Try to detect ngrok public URL for convenience
        let public_base = detect_ngrok_url().await.unwrap_or_default();

        for ch in &channels {
            tracing::info!(
                "  Webhook URL: http://{}:{}/api/v1/gateway/webhook/{}",
                self.config.gateway.host,
                self.config.gateway.port,
                ch
            );
            if !public_base.is_empty() {
                tracing::info!(
                    "  Public URL:  {}/api/v1/gateway/webhook/{}",
                    public_base,
                    ch
                );
            }
        }
        if channels.contains(&"feishu".to_string()) {
            if !public_base.is_empty() {
                tracing::info!(
                    "  Feishu Event Subscription URL:  {}/api/v1/gateway/webhook/feishu",
                    public_base
                );
                tracing::info!(
                    "  Feishu Card Request URL:        {}/api/v1/gateway/webhook/feishu",
                    public_base
                );
            } else {
                tracing::info!(
                    "  Feishu Card Request URL (configure in app console): http://{}:{}/api/v1/gateway/webhook/feishu",
                    self.config.gateway.host,
                    self.config.gateway.port
                );
            }
        }

        Ok(())
    }

    pub async fn stop(&self) -> Result<()> {
        let mut state = self.state.write().await;
        if *state != RuntimeState::Running {
            return Ok(());
        }
        *state = RuntimeState::ShuttingDown;
        drop(state);

        tracing::info!("Stopping SafeClaw runtime");

        let channels: Vec<Arc<dyn ChannelAdapter>> = {
            let channels = self.channels.read().await;
            channels.values().cloned().collect()
        };
        for channel in channels {
            if let Err(e) = channel.stop().await {
                tracing::warn!("Failed to stop channel {}: {}", channel.name(), e);
            }
        }

        if self.config.tee.enabled {
            self.session_manager.shutdown_tee().await?;
        }

        *self.state.write().await = RuntimeState::Stopped;
        tracing::info!("SafeClaw runtime stopped");

        Ok(())
    }

    // -----------------------------------------------------------------------
    // Infrastructure setup (private)
    // -----------------------------------------------------------------------

    async fn init_channels(&self) -> Result<()> {
        let mut channels = self.channels.write().await;

        macro_rules! init_channel {
            ($config_opt:expr, $name:expr, $adapter_expr:expr) => {
                if let Some(config) = $config_opt {
                    let adapter: Arc<dyn ChannelAdapter> = Arc::new($adapter_expr(config.clone()));
                    supervisor::spawn_supervised(adapter.clone(), self.event_tx.clone());
                    channels.insert($name.to_string(), adapter);
                }
            };
        }

        init_channel!(
            &self.config.channels.telegram,
            "telegram",
            TelegramAdapter::new
        );

        if let Some(webchat_config) = &self.config.channels.webchat {
            if webchat_config.enabled {
                let adapter: Arc<dyn ChannelAdapter> =
                    Arc::new(WebChatAdapter::new(webchat_config.clone()));
                supervisor::spawn_supervised(adapter.clone(), self.event_tx.clone());
                channels.insert("webchat".to_string(), adapter);
            }
        }

        init_channel!(&self.config.channels.feishu, "feishu", FeishuAdapter::new);
        init_channel!(
            &self.config.channels.dingtalk,
            "dingtalk",
            DingTalkAdapter::new
        );
        init_channel!(&self.config.channels.wecom, "wecom", WeComAdapter::new);
        init_channel!(&self.config.channels.slack, "slack", SlackAdapter::new);
        init_channel!(
            &self.config.channels.discord,
            "discord",
            DiscordAdapter::new
        );

        Ok(())
    }

    async fn start_event_loop(&self) {
        let event_rx = self.event_rx.write().await.take();
        if let Some(mut rx) = event_rx {
            let processor = self.processor.clone();
            let channels = self.channels.clone();

            tokio::spawn(async move {
                while let Some(event) = rx.recv().await {
                    if let Err(e) = processor.handle_channel_event(event, &channels).await {
                        tracing::error!("Error handling event: {}", e);
                    }
                }
            });
        }
    }

    // -----------------------------------------------------------------------
    // Public API — message processing (delegates to MessageProcessor)
    // -----------------------------------------------------------------------

    /// Process an inbound message and return a response.
    pub async fn process_message(
        &self,
        message: crate::channels::InboundMessage,
    ) -> Result<ProcessedResponse> {
        self.processor.process_message(message).await
    }

    /// Process a raw webhook payload from a3s-gateway.
    pub async fn process_webhook(
        &self,
        channel: &str,
        payload: &str,
    ) -> Result<Option<ProcessedResponse>> {
        self.processor.process_webhook(channel, payload).await
    }

    /// Wire up the LLM agent engine.
    pub async fn set_agent_engine(&self, engine: Arc<crate::agent::AgentEngine>) {
        self.processor.set_agent_engine(engine).await;
    }

    // -----------------------------------------------------------------------
    // Accessors
    // -----------------------------------------------------------------------

    pub fn session_manager(&self) -> &Arc<SessionManager> {
        &self.session_manager
    }

    pub fn session_router(&self) -> &Arc<SessionRouter> {
        self.processor.session_router()
    }

    pub fn config(&self) -> &SafeClawConfig {
        &self.config
    }

    pub fn global_audit_log(&self) -> &Arc<RwLock<AuditLog>> {
        &self.global_audit_log
    }

    pub fn alert_monitor(&self) -> &Arc<AlertMonitor> {
        &self.alert_monitor
    }

    pub fn audit_bus(&self) -> &Arc<AuditEventBus> {
        &self.audit_bus
    }

    pub async fn active_channel_names(&self) -> Vec<String> {
        self.channels.read().await.keys().cloned().collect()
    }

    pub fn channels(&self) -> &Arc<RwLock<HashMap<String, Arc<dyn ChannelAdapter>>>> {
        &self.channels
    }

    pub fn processor(&self) -> &Arc<MessageProcessor> {
        &self.processor
    }

    pub fn security_level(&self) -> crate::tee::SecurityLevel {
        self.session_manager.tee_runtime().security_level()
    }

    /// Get event sender for injecting external events (e.g., from a3s-gateway webhooks)
    pub fn event_sender(&self) -> &mpsc::Sender<crate::channels::ChannelEvent> {
        &self.event_tx
    }

    pub async fn status(&self) -> RuntimeStatus {
        let state = *self.state.read().await;
        RuntimeStatus {
            state: format!("{:?}", state),
            tee_enabled: self.config.tee.enabled,
            security_level: self.security_level(),
            session_count: self.session_manager.session_count().await,
            channels: self.active_channel_names().await,
            a3s_gateway_mode: self.config.a3s_gateway.enabled,
        }
    }
}

// ---------------------------------------------------------------------------
// RuntimeBuilder
// ---------------------------------------------------------------------------

pub struct RuntimeBuilder {
    config: SafeClawConfig,
}

impl RuntimeBuilder {
    pub fn new() -> Self {
        Self {
            config: SafeClawConfig::default(),
        }
    }

    pub fn config(mut self, config: SafeClawConfig) -> Self {
        self.config = config;
        self
    }

    pub fn host(mut self, host: impl Into<String>) -> Self {
        self.config.gateway.host = host.into();
        self
    }

    pub fn port(mut self, port: u16) -> Self {
        self.config.gateway.port = port;
        self
    }

    pub fn tee_enabled(mut self, enabled: bool) -> Self {
        self.config.tee.enabled = enabled;
        self
    }

    pub fn build(self) -> Result<Runtime> {
        Runtime::new(self.config)
    }
}

impl Default for RuntimeBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try to detect the ngrok public URL by querying the local ngrok API.
/// Returns `None` if ngrok is not running or the API is unreachable.
async fn detect_ngrok_url() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(1))
        .build()
        .ok()?;

    let resp = client
        .get("http://127.0.0.1:4040/api/tunnels")
        .send()
        .await
        .ok()?;

    let body: serde_json::Value = resp.json().await.ok()?;
    // Prefer https tunnel
    body.get("tunnels")?
        .as_array()?
        .iter()
        .filter_map(|t| t.get("public_url").and_then(|u| u.as_str()))
        .find(|u| u.starts_with("https://"))
        .or_else(|| {
            body.get("tunnels")?
                .as_array()?
                .first()?
                .get("public_url")?
                .as_str()
        })
        .map(|s| s.to_string())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_gateway_creation() {
        let gateway = RuntimeBuilder::new()
            .host("127.0.0.1")
            .port(18790)
            .tee_enabled(true)
            .build()
            .unwrap();

        assert_eq!(gateway.state().await, RuntimeState::Stopped);
        assert_eq!(gateway.config().gateway.port, 18790);
    }

    #[tokio::test]
    async fn test_gateway_lifecycle() {
        let gateway = RuntimeBuilder::new().tee_enabled(false).build().unwrap();

        gateway.start().await.unwrap();
        assert_eq!(gateway.state().await, RuntimeState::Running);

        gateway.stop().await.unwrap();
        assert_eq!(gateway.state().await, RuntimeState::Stopped);
    }
}
