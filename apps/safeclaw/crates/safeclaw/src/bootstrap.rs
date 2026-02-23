//! Shared gateway bootstrap logic.
//!
//! Both the standalone CLI (`main.rs`) and the embedded Tauri gateway
//! (`server.rs`) need the same startup sequence.  This module provides
//! that sequence as composable building blocks so each consumer is a
//! thin wrapper.

use crate::{
    agent::{AgentBus, AgentEngine, AgentSessionStore, AgentState},
    api::build_app,
    audit::AuditState,
    config::SafeClawConfig,
    privacy::handler::PrivacyState,
    runtime::RuntimeBuilder,
};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;

// ── Config loading ──────────────────────────────────────────────────

/// Walk up from the current directory looking for `.a3s/config.hcl`.
pub fn find_a3s_config() -> Option<PathBuf> {
    let mut dir = std::env::current_dir().ok()?;
    loop {
        let candidate = dir.join(".a3s/config.hcl");
        if candidate.exists() {
            return Some(candidate);
        }
        if !dir.pop() {
            return None;
        }
    }
}

/// If the primary config has no model providers, merge from `.a3s/config.hcl`.
pub fn merge_model_config(config: &mut SafeClawConfig) {
    if config.models.providers.is_empty() {
        if let Some(a3s_config) = find_a3s_config() {
            if let Ok(content) = std::fs::read_to_string(&a3s_config) {
                if let Ok(code_config) = a3s_code::config::CodeConfig::from_hcl(&content) {
                    if !code_config.providers.is_empty() {
                        tracing::info!("Merging model config from {}", a3s_config.display());
                        config.models = code_config;
                    }
                }
            }
        }
    }
}

/// Load configuration using the standard priority chain.
///
/// Priority: explicit path > `./safeclaw.hcl` > `<ancestor>/.a3s/config.hcl`
///           > `~/.config/safeclaw/config.hcl` > default.
///
/// Model config from `.a3s/config.hcl` is always merged in when the
/// primary config has no providers.
pub fn load_config(explicit_path: Option<&PathBuf>) -> Result<(SafeClawConfig, Option<PathBuf>)> {
    let (mut config, config_path) = if let Some(path) = explicit_path {
        let content = std::fs::read_to_string(path)?;
        tracing::info!("Loading config from {}", path.display());
        (
            SafeClawConfig::from_hcl(&content)
                .map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?,
            Some(path.clone()),
        )
    } else if std::path::Path::new("safeclaw.hcl").exists() {
        let content = std::fs::read_to_string("safeclaw.hcl")?;
        tracing::info!("Loading config from ./safeclaw.hcl");
        (
            SafeClawConfig::from_hcl(&content)
                .map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?,
            Some(PathBuf::from("safeclaw.hcl")),
        )
    } else if let Some(a3s_config) = find_a3s_config() {
        let content = std::fs::read_to_string(&a3s_config)?;
        tracing::info!("Loading config from {}", a3s_config.display());
        let code_config = a3s_code::config::CodeConfig::from_hcl(&content)
            .map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?;
        let mut sc = SafeClawConfig::default();
        sc.models = code_config;
        (sc, Some(a3s_config))
    } else if let Some(config_dir) = dirs_next::config_dir() {
        let hcl_path = config_dir.join("safeclaw/config.hcl");
        if hcl_path.exists() {
            let content = std::fs::read_to_string(&hcl_path)?;
            tracing::info!("Loading config from {}", hcl_path.display());
            (
                SafeClawConfig::from_hcl(&content)
                    .map_err(|e| anyhow::anyhow!("Config parse error: {e}"))?,
                Some(hcl_path),
            )
        } else {
            tracing::info!("No config found, using defaults");
            (SafeClawConfig::default(), None)
        }
    } else {
        tracing::info!("No config found, using defaults");
        (SafeClawConfig::default(), None)
    };

    merge_model_config(&mut config);
    Ok((config, config_path))
}

// ── Agent state ─────────────────────────────────────────────────────

/// Build the shared agent state (engine, session manager, skills, memory).
pub async fn build_agent_state(
    mut code_config: a3s_code::config::CodeConfig,
    skills_config: crate::config::SkillsConfig,
    memory_store: Arc<dyn a3s_memory::MemoryStore>,
) -> Result<AgentState> {
    let sessions_dir = AgentSessionStore::default_dir();
    code_config.sessions_dir = Some(sessions_dir.clone());

    let cwd = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .to_string_lossy()
        .to_string();
    let tool_executor = Arc::new(a3s_code::tools::ToolExecutor::new(cwd));

    let default_llm = code_config
        .default_llm_config()
        .map(|llm_cfg| a3s_code::llm::create_client_with_config(llm_cfg));

    if default_llm.is_some() {
        tracing::info!(
            model = code_config.default_model.as_deref().unwrap_or("unknown"),
            "Default LLM client initialized from config"
        );
    } else {
        tracing::info!("No LLM config found — configure via Settings or PUT /api/agent/config");
    }

    // Skill registry
    let skill_registry = Arc::new(a3s_code::skills::SkillRegistry::with_builtins());
    let skills_dir = PathBuf::from(&skills_config.dir);
    if skills_config.auto_load {
        match skill_registry.load_from_dir(&skills_dir) {
            Ok(count) => {
                if count > 0 {
                    tracing::info!(count, dir = %skills_dir.display(), "Loaded skills");
                }
            }
            Err(e) => {
                tracing::debug!("Skills directory not loaded: {e}");
            }
        }
    }

    let session_manager = Arc::new(
        a3s_code::session::SessionManager::with_persistence(
            default_llm,
            tool_executor,
            &sessions_dir,
        )
        .await
        .context("Failed to create SessionManager")?,
    );

    session_manager
        .set_skill_registry(skill_registry, skills_dir)
        .await;
    session_manager.set_memory_store(memory_store).await;

    let store = Arc::new(AgentSessionStore::new(sessions_dir.join("ui-state")));
    let engine = Arc::new(
        AgentEngine::new(session_manager, code_config, store)
            .await
            .map_err(|e| anyhow::anyhow!("Failed to create AgentEngine: {e}"))?,
    );

    Ok(AgentState { engine })
}

// ── Memory store ────────────────────────────────────────────────────

/// Initialize the shared file-backed memory store.
pub async fn init_memory_store() -> Result<Arc<dyn a3s_memory::MemoryStore>> {
    let memory_dir = dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("safeclaw")
        .join("memory");
    let file_store = a3s_memory::FileMemoryStore::new(memory_dir.clone())
        .await
        .context("Failed to initialize memory store")?;
    let store: Arc<dyn a3s_memory::MemoryStore> = Arc::new(file_store);
    tracing::info!(dir = %memory_dir.display(), "Memory store initialized");
    Ok(store)
}

// ── Gateway runner ──────────────────────────────────────────────────

/// A running gateway instance with its engine handle.
pub struct GatewayHandle {
    pub engine: Arc<AgentEngine>,
    gateway: Arc<crate::runtime::Runtime>,
}

impl GatewayHandle {
    pub async fn shutdown(self) {
        self.engine.shutdown().await;
        let _ = self.gateway.stop().await;
    }
}

/// Build and start the full gateway stack. Returns the axum `Router`
/// and a `GatewayHandle` for shutdown.
///
/// The caller is responsible for binding the router to a listener.
pub async fn start_gateway(
    config: SafeClawConfig,
    config_path: Option<PathBuf>,
    host: &str,
    port: u16,
    tee_enabled: bool,
) -> Result<(axum::Router, GatewayHandle)> {
    let models = config.models.clone();
    let skills_config = config.skills.clone();
    let lifecycle_config = config.session_lifecycle.clone();

    let memory_store = init_memory_store().await?;

    let gateway = RuntimeBuilder::new()
        .config(config)
        .host(host)
        .port(port)
        .tee_enabled(tee_enabled)
        .build()?;

    let agent_state = build_agent_state(models, skills_config, memory_store.clone()).await?;
    if let Some(path) = config_path {
        agent_state.engine.set_config_path(path).await;
    }
    gateway.set_agent_engine(agent_state.engine.clone()).await;

    // Wire agent bus (in-memory)
    {
        let provider = a3s_event::MemoryProvider::default();
        let event_bus = Arc::new(a3s_event::EventBus::new(provider));
        let agent_bus = Arc::new(AgentBus::new(agent_state.engine.clone(), event_bus));
        agent_state.engine.set_bus(agent_bus.clone()).await;
        agent_bus.start();
        tracing::info!("AgentBus started (in-memory provider)");
    }

    agent_state.engine.start_lifecycle_task(lifecycle_config);

    gateway.start().await?;
    let gateway = Arc::new(gateway);

    let audit_state = AuditState {
        log: gateway.global_audit_log().clone(),
        alert_monitor: Some(gateway.alert_monitor().clone()),
        persistence: None,
    };

    let privacy_state = PrivacyState {
        classifier: Arc::new(
            crate::privacy::classifier::Classifier::new(
                crate::config::default_classification_rules(),
                crate::config::SensitivityLevel::Normal,
            )
            .expect("default classifier"),
        ),
        semantic: Arc::new(crate::privacy::semantic::SemanticAnalyzer::new()),
    };

    let channel_config_dir = dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("safeclaw");
    let channel_config_store =
        crate::config::ChannelAgentConfigStore::new(channel_config_dir).await;

    let engine = agent_state.engine.clone();
    let app = build_app(
        gateway.clone(),
        agent_state,
        privacy_state,
        audit_state,
        memory_store,
        channel_config_store,
        &[],
    );

    Ok((app, GatewayHandle { engine, gateway }))
}

/// Build, bind, serve, and shut down the full gateway in one call.
///
/// This is the highest-level entry point — it calls [`start_gateway`],
/// binds the listener, runs `axum::serve` with graceful shutdown on
/// Ctrl-C, then tears everything down.  Both the standalone CLI and
/// the embedded Tauri gateway use this.
pub async fn run_gateway(
    config: SafeClawConfig,
    config_path: Option<PathBuf>,
    host: &str,
    port: u16,
    tee_enabled: bool,
) -> Result<()> {
    let (app, handle) = start_gateway(config, config_path, host, port, tee_enabled).await?;

    let addr: std::net::SocketAddr = format!("{host}:{port}")
        .parse()
        .context("Invalid listen address")?;

    tracing::info!(%addr, "SafeClaw gateway listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c().await.ok();
        })
        .await
        .context("HTTP server error")?;

    tracing::info!("Shutting down...");
    handle.shutdown().await;

    Ok(())
}
