//! SafeClaw runtime server
//!
//! **Single responsibility**: lifecycle (start/stop) and infrastructure
//! setup (channel adapters, event loop).
//!
//! Message processing is delegated entirely to `MessageProcessor`.

use crate::channels::{
    supervisor, ChannelAdapter, DingTalkAdapter, DiscordAdapter, FeishuAdapter, SlackAdapter,
    TelegramAdapter, WeComAdapter, WebChatAdapter,
};
use crate::config::SafeClawConfig;
use crate::error::{Error, Result};
use crate::runtime::processor::MessageProcessor;
use crate::session::SessionManager;
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
    /// Message processing pipeline
    processor: Arc<MessageProcessor>,
}

impl Runtime {
    /// Create a new runtime with the given configuration.
    pub fn new(config: SafeClawConfig) -> Result<Self> {
        let (event_tx, event_rx) = mpsc::channel(1000);

        let session_manager = Arc::new(SessionManager::new(config.tee.clone()));

        let processor = Arc::new(MessageProcessor::new(
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

    pub fn config(&self) -> &SafeClawConfig {
        &self.config
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

    /// Get event sender for injecting external events
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
