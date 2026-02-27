//! HTTP and WebSocket handlers for the agent module
//!
//! Provides REST API endpoints for session management and a WebSocket
//! upgrade handler for browser connections.  All handlers delegate to
//! `AgentEngine` which wraps a3s-code's `SessionManager` in-process.

use crate::agent::engine::AgentEngine;
use crate::agent::types::*;
use crate::error::to_json;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, State,
    },
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use futures::{SinkExt, StreamExt};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::mpsc;

/// Shared state for agent handlers
#[derive(Clone)]
pub struct AgentState {
    pub engine: Arc<AgentEngine>,
}

/// Create the agent router with all REST and WebSocket endpoints
pub fn agent_router(state: AgentState) -> Router {
    Router::new()
        // REST endpoints
        .route("/api/agent/sessions", post(create_session))
        .route("/api/agent/sessions", get(list_sessions))
        .route("/api/agent/sessions/:id", get(get_session))
        .route("/api/agent/sessions/:id", patch(update_session))
        .route("/api/agent/sessions/:id", delete(delete_session))
        .route("/api/agent/sessions/:id/relaunch", post(relaunch_session))
        .route("/api/agent/backends", get(list_backends))
        .route("/api/agent/personas", get(list_personas))
        .route("/api/agent/sessions/:id/message", post(send_agent_message))
        .route(
            "/api/agent/sessions/:id/auto-execute",
            put(set_auto_execute),
        )
        .route("/api/agent/sessions/:id/configure", post(configure_session))
        // Lifecycle endpoints
        .route("/api/agent/sessions/:id/archive", post(archive_session))
        .route("/api/agent/sessions/:id/unarchive", post(unarchive_session))
        .route("/api/agent/stats", get(session_stats))
        .route("/api/agent/directory", get(agent_directory))
        // Slash commands
        .route("/api/agent/commands", get(list_commands))
        // Config endpoints
        .route("/api/agent/config", get(get_config))
        .route("/api/agent/config", put(put_config))
        // MCP endpoints
        .route("/api/agent/mcp", get(list_mcp_servers))
        .route("/api/agent/mcp", post(add_mcp_server))
        .route("/api/agent/mcp/:name", delete(remove_mcp_server))
        // WebSocket endpoint (browser only — no more CLI subprocess)
        .route("/ws/agent/browser/:id", get(ws_browser_upgrade))
        .with_state(state)
}

// =============================================================================
// REST handlers
// =============================================================================

/// Create session request body
#[derive(Debug, Deserialize)]
struct CreateSessionRequest {
    model: Option<String>,
    permission_mode: Option<String>,
    cwd: Option<String>,
    persona_id: Option<String>,
    /// API key override for this session's LLM client
    api_key: Option<String>,
    /// Base URL override for this session's LLM client
    base_url: Option<String>,
    /// System prompt override
    system_prompt: Option<String>,
    /// Skills to enable for this session (not yet wired into engine)
    #[allow(dead_code)]
    skills: Option<Vec<String>>,
    /// MCP servers to connect for this session
    #[serde(default)]
    mcp_servers: Vec<a3s_code::mcp::McpServerConfig>,
}

/// Create a new agent session
async fn create_session(
    State(state): State<AgentState>,
    Json(request): Json<CreateSessionRequest>,
) -> impl IntoResponse {
    let session_id = uuid::Uuid::new_v4().to_string();

    match state
        .engine
        .create_session(
            &session_id,
            request.model,
            request.permission_mode,
            request.cwd,
            request.persona_id,
            request.api_key,
            request.base_url,
            request.system_prompt,
        )
        .await
    {
        Ok(info) => (StatusCode::CREATED, Json(to_json(info))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// List all agent sessions
async fn list_sessions(State(state): State<AgentState>) -> impl IntoResponse {
    let sessions = state.engine.list_sessions().await;
    Json(sessions)
}

/// Get a specific agent session by ID
async fn get_session(State(state): State<AgentState>, Path(id): Path<String>) -> impl IntoResponse {
    match state.engine.get_session(&id).await {
        Some(info) => (StatusCode::OK, Json(to_json(info))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        ),
    }
}

/// Update session request body
#[derive(Debug, Deserialize)]
struct UpdateSessionRequest {
    name: Option<String>,
    archived: Option<bool>,
    cwd: Option<String>,
}

/// Update a session's name or archived status
async fn update_session(
    State(state): State<AgentState>,
    Path(id): Path<String>,
    Json(request): Json<UpdateSessionRequest>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        );
    }

    if let Some(name) = request.name {
        state.engine.set_name(&id, name).await;
    }
    if let Some(archived) = request.archived {
        state.engine.set_archived(&id, archived).await;
    }

    match state.engine.get_session(&id).await {
        Some(info) => (StatusCode::OK, Json(to_json(info))),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        ),
    }
}

/// Delete a session and remove all state
async fn delete_session(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return StatusCode::NOT_FOUND;
    }

    let _ = state.engine.destroy_session(&id).await;

    StatusCode::NO_CONTENT
}

/// Relaunch a session (destroy + recreate with same config)
async fn relaunch_session(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    let existing = match state.engine.get_session(&id).await {
        Some(info) => info,
        None => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Session not found"})),
            );
        }
    };

    // Destroy existing session
    let _ = state.engine.destroy_session(&id).await;

    // Recreate with same config
    match state
        .engine
        .create_session(
            &id,
            existing.model,
            existing.permission_mode,
            Some(existing.cwd),
            existing.persona_id,
            None,
            None,
            None,
        )
        .await
    {
        Ok(info) => (StatusCode::OK, Json(to_json(info))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Available model backend
#[derive(Debug, Serialize)]
struct BackendInfo {
    id: String,
    name: String,
    provider: String,
    is_default: bool,
}

/// Derive a human-readable display name from a model ID.
///
/// Maps well-known model IDs to friendly names; falls back to the raw ID
/// for unrecognized models.
fn model_display_name(model_id: &str) -> String {
    match model_id {
        s if s.starts_with("claude-opus-4") => "Claude Opus 4".to_string(),
        s if s.starts_with("claude-sonnet-4") => "Claude Sonnet 4".to_string(),
        s if s.starts_with("claude-haiku-3-5") | s.starts_with("claude-3-5-haiku") => {
            "Claude 3.5 Haiku".to_string()
        }
        s if s.starts_with("claude-sonnet-3-5") | s.starts_with("claude-3-5-sonnet") => {
            "Claude 3.5 Sonnet".to_string()
        }
        "gpt-4o" => "GPT-4o".to_string(),
        "gpt-4o-mini" => "GPT-4o Mini".to_string(),
        "o1" => "O1".to_string(),
        "o1-mini" => "O1 Mini".to_string(),
        other => other.to_string(),
    }
}

/// List available model backends from configuration
async fn list_backends(State(state): State<AgentState>) -> impl IntoResponse {
    let cfg = state.engine.code_config().await;
    let default_model = cfg.default_model.as_deref().unwrap_or("");
    // default_model is "provider/model" format — extract provider portion
    let default_provider = default_model.split('/').next().unwrap_or("");
    let default_model_id = default_model.split('/').nth(1).unwrap_or("");

    let mut backends = Vec::new();

    for provider in &cfg.providers {
        for model in &provider.models {
            let is_default = provider.name == default_provider && model.id == default_model_id;
            backends.push(BackendInfo {
                id: model.id.clone(),
                name: if model.name.is_empty() {
                    model_display_name(&model.id)
                } else {
                    model.name.clone()
                },
                provider: provider.name.clone(),
                is_default,
            });
        }
    }

    // Sort: default provider first, then alphabetically by provider, then by model id
    backends.sort_by(|a, b| {
        let a_is_default_provider = a.provider == default_provider;
        let b_is_default_provider = b.provider == default_provider;
        b_is_default_provider
            .cmp(&a_is_default_provider)
            .then_with(|| a.provider.cmp(&b.provider))
            .then_with(|| a.id.cmp(&b.id))
    });

    Json(backends)
}

/// List available personas from the skill registry
async fn list_personas(State(state): State<AgentState>) -> impl IntoResponse {
    let personas = state.engine.list_personas().await;
    Json(personas)
}

/// GET /api/agent/commands — list available slash commands
async fn list_commands(State(state): State<AgentState>) -> impl IntoResponse {
    let commands: Vec<serde_json::Value> = state
        .engine
        .list_commands()
        .into_iter()
        .map(|(name, desc)| {
            serde_json::json!({
                "name": format!("/{}", name),
                "description": desc,
            })
        })
        .collect();
    Json(commands)
}

// =============================================================================
// Config handlers
// =============================================================================

/// GET /api/agent/config — return current CodeConfig as JSON
async fn get_config(State(state): State<AgentState>) -> impl IntoResponse {
    let cfg = state.engine.code_config().await;
    Json(serde_json::to_value(&cfg).unwrap_or_default())
}

/// PUT /api/agent/config request body — partial CodeConfig fields the UI can update
#[derive(Debug, Deserialize)]
struct PutConfigRequest {
    default_model: Option<String>,
    providers: Option<Vec<serde_json::Value>>,
}

/// PUT /api/agent/config — update CodeConfig and persist to config file
async fn put_config(
    State(state): State<AgentState>,
    Json(request): Json<PutConfigRequest>,
) -> impl IntoResponse {
    // Build updated config from current + patch
    let mut cfg = state.engine.code_config().await;

    if let Some(default_model) = request.default_model {
        cfg.default_model = Some(default_model);
    }

    if let Some(providers_json) = request.providers {
        match serde_json::from_value::<Vec<a3s_code::config::ProviderConfig>>(
            serde_json::Value::Array(providers_json),
        ) {
            Ok(new_providers) => {
                // Merge: preserve existing apiKey/baseUrl when incoming value is None.
                // The frontend sends None when it doesn't have credentials (they may
                // have been set via env() in HCL or direct file edit).
                let old_providers = &cfg.providers;
                let merged: Vec<a3s_code::config::ProviderConfig> = new_providers
                    .into_iter()
                    .map(|mut np| {
                        if let Some(op) = old_providers.iter().find(|p| p.name == np.name) {
                            // Preserve provider-level credentials if incoming is None
                            if np.api_key.is_none() {
                                np.api_key = op.api_key.clone();
                            }
                            if np.base_url.is_none() {
                                np.base_url = op.base_url.clone();
                            }
                            // Preserve model-level credentials if incoming is None
                            for nm in &mut np.models {
                                if let Some(om) = op.models.iter().find(|m| m.id == nm.id) {
                                    if nm.api_key.is_none() {
                                        nm.api_key = om.api_key.clone();
                                    }
                                    if nm.base_url.is_none() {
                                        nm.base_url = om.base_url.clone();
                                    }
                                }
                            }
                        }
                        np
                    })
                    .collect();
                cfg.providers = merged;
            }
            Err(e) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({"error": format!("Invalid providers: {}", e)})),
                );
            }
        }
    }

    // Persist to config file if path is known
    if let Some(path) = state.engine.get_config_path().await {
        persist_code_config_to_file(&cfg, &path);
    }

    // Hot-reload: updates both in-memory config AND SessionManager's default LLM
    state.engine.update_code_config(cfg.clone()).await;

    (
        StatusCode::OK,
        Json(serde_json::to_value(&cfg).unwrap_or_default()),
    )
}

/// Persist `CodeConfig` to the config file.
///
/// Handles two file formats:
/// - `.a3s/config.hcl` — a3s-code format (top-level `default_model` + `providers`)
///   Written by serializing `CodeConfig` directly as JSON (which the HCL loader accepts).
/// - `safeclaw.hcl` / `config.hcl` — SafeClaw format (`models { ... }` block)
///   Read existing file, patch the `models` section, write back.
fn persist_code_config_to_file(cfg: &a3s_code::config::CodeConfig, path: &std::path::Path) {
    // Detect format by checking if the file contains a top-level `models {` block
    let is_safeclaw_format = std::fs::read_to_string(path)
        .map(|content| content.contains("models {") || content.contains("models{"))
        .unwrap_or(false);

    if is_safeclaw_format {
        // SafeClaw format: read full config, patch models, write back
        match std::fs::read_to_string(path) {
            Ok(existing_hcl) => match crate::config::SafeClawConfig::from_hcl(&existing_hcl) {
                Ok(mut full_config) => {
                    full_config.models = cfg.clone();
                    match hcl::to_string(&full_config) {
                        Ok(new_hcl) => {
                            if let Err(e) = std::fs::write(path, &new_hcl) {
                                tracing::warn!("Failed to write config {}: {}", path.display(), e);
                            } else {
                                tracing::info!("Config persisted to {}", path.display());
                            }
                        }
                        Err(e) => tracing::warn!("Failed to serialize SafeClawConfig: {}", e),
                    }
                }
                Err(e) => tracing::warn!("Failed to parse existing SafeClawConfig: {}", e),
            },
            Err(e) => tracing::warn!("Failed to read config {}: {}", path.display(), e),
        }
    } else {
        // a3s-code format: serialize CodeConfig as JSON directly
        // CodeConfig::save_to_file writes JSON which CodeConfig::from_file also accepts
        if let Err(e) = cfg.save_to_file(path) {
            tracing::warn!("Failed to persist CodeConfig to {}: {}", path.display(), e);
        } else {
            tracing::info!("CodeConfig persisted to {}", path.display());
        }
    }
}

/// Send agent message request body
#[derive(Debug, Deserialize)]
struct SendAgentMessageRequest {
    /// "broadcast:<topic>" or "mention:<session_id>"
    target: String,
    content: String,
}

/// Send a message from a session to another agent via the event bus
async fn send_agent_message(
    State(state): State<AgentState>,
    Path(id): Path<String>,
    Json(request): Json<SendAgentMessageRequest>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        );
    }

    match state
        .engine
        .publish_agent_message(&id, &request.target, &request.content)
        .await
    {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

/// Set auto-execute request body
#[derive(Debug, Deserialize)]
struct SetAutoExecuteRequest {
    enabled: bool,
}

/// Toggle auto-execute mode for incoming agent messages on a session
async fn set_auto_execute(
    State(state): State<AgentState>,
    Path(id): Path<String>,
    Json(request): Json<SetAutoExecuteRequest>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        );
    }

    state.engine.set_auto_execute(&id, request.enabled).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"ok": true, "auto_execute": request.enabled})),
    )
}

/// Configure LLM for an existing session (model + credentials)
#[derive(Debug, Deserialize)]
struct ConfigureSessionRequest {
    model: Option<String>,
    api_key: Option<String>,
    base_url: Option<String>,
}

async fn configure_session(
    State(state): State<AgentState>,
    Path(id): Path<String>,
    Json(request): Json<ConfigureSessionRequest>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        );
    }

    let model_id = match request.model.as_deref() {
        Some(m) if !m.is_empty() => m.to_string(),
        _ => state
            .engine
            .code_config()
            .await
            .default_model
            .unwrap_or_default(),
    };

    tracing::info!(
        session_id = %id,
        model = %model_id,
        has_api_key = request.api_key.as_deref().map(|k| !k.is_empty()).unwrap_or(false),
        has_base_url = request.base_url.is_some(),
        "Configuring session LLM"
    );

    if model_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No model specified and no default configured"})),
        );
    }

    let result = if request.api_key.is_some() || request.base_url.is_some() {
        state
            .engine
            .configure_model_with_credentials_pub(
                &id,
                &model_id,
                request.api_key.as_deref(),
                request.base_url.as_deref(),
            )
            .await
    } else {
        state
            .engine
            .configure_model_for_session_pub(&id, &model_id)
            .await
    };

    match result {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"ok": true, "model": model_id})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ),
    }
}

// =============================================================================
// Lifecycle handlers
// =============================================================================

/// Archive a session
async fn archive_session(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        );
    }
    state.engine.set_archived(&id, true).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"ok": true, "archived": true})),
    )
}

/// Unarchive a session
async fn unarchive_session(
    State(state): State<AgentState>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    if state.engine.get_session(&id).await.is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        );
    }
    state.engine.set_archived(&id, false).await;
    (
        StatusCode::OK,
        Json(serde_json::json!({"ok": true, "archived": false})),
    )
}

/// GET /api/agent/stats — session statistics
async fn session_stats(State(state): State<AgentState>) -> impl IntoResponse {
    let stats = state.engine.session_stats().await;
    Json(stats)
}

/// GET /api/agent/directory — agent discovery directory
async fn agent_directory(State(state): State<AgentState>) -> impl IntoResponse {
    let directory = state.engine.list_agent_directory().await;
    Json(directory)
}

// =============================================================================
// WebSocket handler
// =============================================================================

/// WebSocket upgrade handler for browser connections
async fn ws_browser_upgrade(
    ws: WebSocketUpgrade,
    Path(session_id): Path<String>,
    State(state): State<AgentState>,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_browser_ws(socket, session_id, state))
}

/// Handle browser WebSocket connection (JSON protocol)
///
/// Browser sends/receives JSON messages. On connect, receives session_init,
/// message history, and pending permission requests for state replay.
async fn handle_browser_ws(socket: WebSocket, session_id: String, state: AgentState) {
    let browser_id = uuid::Uuid::new_v4().to_string();
    tracing::info!(
        session_id = %session_id,
        browser_id = %browser_id,
        "Browser WebSocket connected"
    );

    let (mut ws_sender, mut ws_receiver) = socket.split();

    // Channel for engine → browser outbound messages
    let (tx, mut rx) = mpsc::unbounded_channel::<String>();

    // Register with engine (sends session_init, history, pending permissions)
    let registered = state
        .engine
        .handle_browser_open(&session_id, &browser_id, tx)
        .await;

    if !registered {
        tracing::warn!(
            session_id = %session_id,
            "Browser connected to unknown session"
        );
        let error_msg = serde_json::json!({
            "type": "error",
            "message": "Session not found"
        });
        let _ = ws_sender.send(Message::Text(error_msg.to_string())).await;
        return;
    }

    // Forward engine → browser messages
    let send_session_id = session_id.clone();
    let send_task = tokio::spawn(async move {
        while let Some(msg) = rx.recv().await {
            if ws_sender.send(Message::Text(msg)).await.is_err() {
                tracing::debug!(
                    session_id = %send_session_id,
                    "Browser WebSocket send failed"
                );
                break;
            }
        }
    });

    // Receive browser → engine messages (JSON)
    let recv_engine = state.engine.clone();
    let recv_session_id = session_id.clone();
    let recv_task = tokio::spawn(async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            match msg {
                Message::Text(text) => {
                    match serde_json::from_str::<BrowserOutgoingMessage>(&text) {
                        Ok(browser_msg) => {
                            recv_engine
                                .handle_browser_message(&recv_session_id, browser_msg)
                                .await;
                        }
                        Err(e) => {
                            let preview = &text[..text.len().min(200)];
                            tracing::warn!(
                                session_id = %recv_session_id,
                                "Invalid browser message: {} (raw: {})",
                                e,
                                preview
                            );
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
    });

    tokio::select! {
        _ = send_task => {}
        _ = recv_task => {}
    }

    // Clean up browser connection in engine
    state
        .engine
        .handle_browser_close(&session_id, &browser_id)
        .await;

    tracing::info!(
        session_id = %session_id,
        browser_id = %browser_id,
        "Browser WebSocket disconnected"
    );
}

// =============================================================================
// MCP handlers
// =============================================================================

/// List all MCP servers and their status
async fn list_mcp_servers(_state: State<AgentState>) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

/// Add and connect an MCP server
async fn add_mcp_server(
    _state: State<AgentState>,
    _body: axum::body::Bytes,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

/// Disconnect and remove an MCP server
async fn remove_mcp_server(
    _state: State<AgentState>,
    _name: Path<String>,
) -> impl IntoResponse {
    StatusCode::NOT_IMPLEMENTED
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::engine::AgentEngine;
    use crate::agent::session_store::AgentSessionStore;
    use a3s_code::config::{CodeConfig, ProviderConfig};
    use tempfile::TempDir;

    fn test_model(id: &str) -> a3s_code::config::ModelConfig {
        serde_json::from_value(serde_json::json!({ "id": id })).unwrap()
    }

    async fn make_state() -> (AgentState, TempDir) {
        let dir = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path()).unwrap();

        let code_config = CodeConfig {
            sessions_dir: Some(dir.path().to_path_buf()),
            ..Default::default()
        };

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .to_string_lossy()
            .to_string();
        let tool_executor = Arc::new(a3s_code::tools::ToolExecutor::new(cwd));
        let session_manager = Arc::new(
            a3s_code::session::SessionManager::with_persistence(None, tool_executor, dir.path())
                .await
                .unwrap(),
        );
        let store = Arc::new(AgentSessionStore::new(dir.path().join("ui-state")));
        let engine = Arc::new(
            AgentEngine::new(session_manager, code_config, store)
                .await
                .unwrap(),
        );

        let state = AgentState { engine };
        (state, dir)
    }

    #[tokio::test]
    async fn test_agent_state_is_clone() {
        let (state, _dir) = make_state().await;
        let _cloned = state.clone();
    }

    #[tokio::test]
    async fn test_agent_router_builds() {
        let (state, _dir) = make_state().await;
        let _router = agent_router(state);
    }

    #[test]
    fn test_backend_info_serialization() {
        let info = BackendInfo {
            id: "claude-sonnet-4-20250514".to_string(),
            name: "Claude Sonnet 4".to_string(),
            provider: "anthropic".to_string(),
            is_default: true,
        };
        let json = serde_json::to_string(&info).unwrap();
        assert!(json.contains("claude-sonnet-4-20250514"));
        assert!(json.contains("Claude Sonnet 4"));
        assert!(json.contains("\"provider\":\"anthropic\""));
        assert!(json.contains("\"is_default\":true"));
    }

    #[test]
    fn test_model_display_name_known_models() {
        assert_eq!(
            model_display_name("claude-opus-4-20250514"),
            "Claude Opus 4"
        );
        assert_eq!(
            model_display_name("claude-sonnet-4-20250514"),
            "Claude Sonnet 4"
        );
        assert_eq!(
            model_display_name("claude-haiku-3-5-20241022"),
            "Claude 3.5 Haiku"
        );
        assert_eq!(model_display_name("gpt-4o"), "GPT-4o");
        assert_eq!(model_display_name("gpt-4o-mini"), "GPT-4o Mini");
        assert_eq!(model_display_name("o1"), "O1");
    }

    #[test]
    fn test_model_display_name_unknown_falls_back() {
        assert_eq!(model_display_name("my-custom-model"), "my-custom-model");
    }

    #[test]
    fn test_create_session_request_deserialization() {
        let json = r#"{"model":"claude-sonnet-4-20250514","permission_mode":"default"}"#;
        let req: CreateSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.model.as_deref(), Some("claude-sonnet-4-20250514"));
        assert_eq!(req.permission_mode.as_deref(), Some("default"));
        assert!(req.cwd.is_none());
    }

    #[test]
    fn test_create_session_request_minimal() {
        let json = r#"{}"#;
        let req: CreateSessionRequest = serde_json::from_str(json).unwrap();
        assert!(req.model.is_none());
        assert!(req.permission_mode.is_none());
        assert!(req.cwd.is_none());
    }

    #[test]
    fn test_update_session_request_deserialization() {
        let json = r#"{"name":"My Session","archived":true}"#;
        let req: UpdateSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name.as_deref(), Some("My Session"));
        assert_eq!(req.archived, Some(true));
    }

    #[test]
    fn test_update_session_request_partial() {
        let json = r#"{"name":"New Name"}"#;
        let req: UpdateSessionRequest = serde_json::from_str(json).unwrap();
        assert_eq!(req.name.as_deref(), Some("New Name"));
        assert!(req.archived.is_none());
    }

    #[tokio::test]
    async fn test_list_backends_returns_config_models() {
        let (state, _dir) = make_state().await;
        let response = list_backends(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .unwrap();
        let backends: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        // Default CodeConfig has no providers, so empty list
        assert_eq!(backends.len(), 0);
    }

    #[tokio::test]
    async fn test_list_backends_custom_config() {
        let dir = TempDir::new().unwrap();
        let code_config = CodeConfig {
            default_model: Some("custom/my-model-v2".to_string()),
            providers: vec![ProviderConfig {
                name: "custom".to_string(),
                api_key: Some("key".to_string()),
                base_url: None,
                models: vec![test_model("my-model-v1"), test_model("my-model-v2")],
            }],
            sessions_dir: Some(dir.path().to_path_buf()),
            ..Default::default()
        };

        let cwd = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
            .to_string_lossy()
            .to_string();
        let tool_executor = Arc::new(a3s_code::tools::ToolExecutor::new(cwd));
        let session_manager = Arc::new(
            a3s_code::session::SessionManager::with_persistence(None, tool_executor, dir.path())
                .await
                .unwrap(),
        );
        let store = Arc::new(AgentSessionStore::new(dir.path().join("ui-state")));
        let engine = Arc::new(
            AgentEngine::new(session_manager, code_config, store)
                .await
                .unwrap(),
        );
        let state = AgentState { engine };

        let response = list_backends(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .unwrap();
        let backends: Vec<serde_json::Value> = serde_json::from_slice(&body).unwrap();

        assert_eq!(backends.len(), 2);
        assert_eq!(backends[0]["provider"], "custom");

        // my-model-v2 is the default
        let default_backend = backends.iter().find(|b| b["is_default"] == true).unwrap();
        assert_eq!(default_backend["id"], "my-model-v2");
        // Unknown model should use raw ID as display name
        assert_eq!(default_backend["name"], "my-model-v2");
    }

    #[tokio::test]
    async fn test_list_sessions_empty() {
        let (state, _dir) = make_state().await;
        let response = list_sessions(State(state)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_get_session_not_found() {
        let (state, _dir) = make_state().await;
        let response = get_session(State(state), Path("nonexistent".to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_delete_session_not_found() {
        let (state, _dir) = make_state().await;
        let response = delete_session(State(state), Path("nonexistent".to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_relaunch_session_not_found() {
        let (state, _dir) = make_state().await;
        let response = relaunch_session(State(state), Path("nonexistent".to_string()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_update_session_not_found() {
        let (state, _dir) = make_state().await;
        let req = UpdateSessionRequest {
            name: Some("New Name".to_string()),
            archived: None,
        };
        let response = update_session(State(state), Path("nonexistent".to_string()), Json(req))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_create_and_get_session() {
        let (state, _dir) = make_state().await;

        let req = CreateSessionRequest {
            model: None,
            permission_mode: None,
            cwd: Some("/tmp".to_string()),
            persona_id: None,
            api_key: None,
            base_url: None,
            system_prompt: None,
            skills: None,
            mcp_servers: vec![],
        };
        let response = create_session(State(state.clone()), Json(req))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::CREATED);

        let body = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .unwrap();
        let info: serde_json::Value = serde_json::from_slice(&body).unwrap();
        let session_id = info["session_id"].as_str().unwrap().to_string();

        // Should be findable
        let response = get_session(State(state.clone()), Path(session_id.clone()))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::OK);

        // Delete it
        let response = delete_session(State(state), Path(session_id))
            .await
            .into_response();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
    }
}
