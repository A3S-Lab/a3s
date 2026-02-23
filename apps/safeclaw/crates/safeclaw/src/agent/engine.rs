//! Agent engine — direct a3s-code library integration
//!
//! Wraps `a3s_code::session::SessionManager` to provide in-process agent
//! execution, replacing the previous CLI subprocess architecture.
//!
//! ```text
//! UI <-WS(JSON)-> handler.rs -> engine.rs -> a3s-code SessionManager (in-process)
//!                                 └── session_store.rs (UI state only)
//! ```

use crate::agent::session_store::AgentSessionStore;
use crate::agent::types::*;
use a3s_code::agent::AgentEvent;
use a3s_code::commands::{CommandAction, CommandContext, CommandRegistry};
use a3s_code::config::CodeConfig;
use a3s_code::session::SessionManager;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};

/// Core engine wrapping a3s-code's `SessionManager`.
///
/// Manages per-session UI state (browser senders, message history,
/// pending permissions) alongside the a3s-code session lifecycle.
pub struct AgentEngine {
    session_manager: Arc<SessionManager>,
    code_config: Arc<RwLock<CodeConfig>>,
    sessions: Arc<RwLock<HashMap<String, EngineSession>>>,
    store: Arc<AgentSessionStore>,
    /// Optional agent bus for inter-session messaging
    agent_bus: Arc<RwLock<Option<Arc<crate::agent::bus::AgentBus>>>>,
    /// Path to the HCL config file (for live updates via API)
    config_path: Arc<RwLock<Option<std::path::PathBuf>>>,
    /// Slash command registry (shared across sessions)
    command_registry: Arc<CommandRegistry>,
}

/// Per-session UI state tracked by the engine.
struct EngineSession {
    id: String,
    browser_senders: HashMap<String, mpsc::UnboundedSender<String>>,
    state: AgentSessionState,
    message_history: Vec<BrowserIncomingMessage>,
    pending_permissions: HashMap<String, PermissionRequest>,
    generation_handle: Option<tokio::task::JoinHandle<()>>,
    name: Option<String>,
    archived: bool,
    created_at: u64,
    cwd: String,
    model: Option<String>,
    permission_mode: Option<String>,
    /// Bound persona ID (if any)
    persona_id: Option<String>,
    /// Unix timestamp of last activity (message sent or browser connected)
    last_activity_at: u64,
}

impl AgentEngine {
    /// Access a snapshot of the code configuration.
    pub async fn code_config(&self) -> CodeConfig {
        self.code_config.read().await.clone()
    }

    /// Replace the code configuration and hot-swap the default LLM client.
    ///
    /// Updates both the in-memory config and the SessionManager's default LLM
    /// so all subsequent sessions (and sessions without a per-session client)
    /// immediately use the new model without a restart.
    pub async fn update_code_config(&self, new_config: CodeConfig) {
        // Build new default LLM client from updated config
        let new_llm = new_config
            .default_llm_config()
            .map(|llm_cfg| a3s_code::llm::create_client_with_config(llm_cfg));

        // Hot-swap SessionManager's default client
        self.session_manager.set_default_llm(new_llm).await;

        // Update in-memory config
        *self.code_config.write().await = new_config;
    }

    /// Set the path to the HCL config file so the API can persist changes.
    pub async fn set_config_path(&self, path: std::path::PathBuf) {
        *self.config_path.write().await = Some(path);
    }

    /// Get the config file path (if set).
    pub async fn get_config_path(&self) -> Option<std::path::PathBuf> {
        self.config_path.read().await.clone()
    }

    /// Create a new engine from a pre-built `SessionManager` and config.
    pub async fn new(
        session_manager: Arc<SessionManager>,
        code_config: CodeConfig,
        store: Arc<AgentSessionStore>,
    ) -> crate::Result<Self> {
        let engine = Self {
            session_manager,
            code_config: Arc::new(RwLock::new(code_config)),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            store,
            agent_bus: Arc::new(RwLock::new(None)),
            config_path: Arc::new(RwLock::new(None)),
            command_registry: Arc::new(CommandRegistry::new()),
        };

        // Restore persisted UI state from disk
        engine.restore_from_disk().await;

        Ok(engine)
    }

    // =========================================================================
    // Session CRUD
    // =========================================================================

    /// Create a new agent session.
    ///
    /// Creates both an a3s-code session and the corresponding UI state.
    pub async fn create_session(
        &self,
        session_id: &str,
        model: Option<String>,
        permission_mode: Option<String>,
        cwd: Option<String>,
        persona_id: Option<String>,
        api_key: Option<String>,
        base_url: Option<String>,
        system_prompt_override: Option<String>,
    ) -> crate::Result<AgentProcessInfo> {
        let workspace = cwd.unwrap_or_else(|| {
            std::env::current_dir()
                .unwrap_or_else(|_| std::path::PathBuf::from("/tmp"))
                .to_string_lossy()
                .to_string()
        });

        // Resolve persona system prompt, with override taking precedence
        let system_prompt = if system_prompt_override.is_some() {
            system_prompt_override
        } else if let Some(ref pid) = persona_id {
            self.resolve_persona_system_prompt(pid).await
        } else {
            None
        };

        // Build a3s-code session config
        let mut session_config = a3s_code::session::SessionConfig {
            name: String::new(),
            workspace: workspace.clone(),
            system_prompt,
            ..Default::default()
        };

        // Set permission/confirmation policy based on mode
        if let Some(ref mode) = permission_mode {
            let (perm, confirm) = permission_mode_to_policies(mode);
            session_config.permission_policy = Some(perm);
            session_config.confirmation_policy = Some(confirm);
        }

        // Create a3s-code session
        self.session_manager
            .create_session(session_id.to_string(), session_config)
            .await
            .map_err(|e| {
                crate::Error::Runtime(format!("Failed to create a3s-code session: {}", e))
            })?;

        // Configure LLM client — prefer explicit credentials, fall back to config lookup
        if let Some(ref model_id) = model {
            let result = if api_key.is_some() || base_url.is_some() {
                self.configure_model_with_credentials(
                    session_id,
                    model_id,
                    api_key.as_deref(),
                    base_url.as_deref(),
                )
                .await
            } else {
                self.configure_model_for_session(session_id, model_id).await
            };
            if let Err(e) = result {
                tracing::warn!(
                    session_id = %session_id,
                    model = %model_id,
                    "Failed to configure model: {}",
                    e
                );
            }
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut state = AgentSessionState::new(session_id.to_string());
        state.persona_id = persona_id.clone();

        let engine_session = EngineSession {
            id: session_id.to_string(),
            browser_senders: HashMap::new(),
            state,
            message_history: Vec::new(),
            pending_permissions: HashMap::new(),
            generation_handle: None,
            name: None,
            archived: false,
            created_at: now,
            cwd: workspace.clone(),
            model: model.clone(),
            permission_mode: permission_mode.clone(),
            persona_id: persona_id.clone(),
            last_activity_at: now,
        };

        // Update state fields
        let mut sessions = self.sessions.write().await;
        sessions.insert(session_id.to_string(), engine_session);

        // Update the session state with model/cwd info
        if let Some(es) = sessions.get_mut(session_id) {
            es.state.model = model.clone().unwrap_or_default();
            es.state.cwd = workspace.clone();
            es.state.permission_mode = permission_mode
                .clone()
                .unwrap_or_else(|| "default".to_string());
            // Populate tool names from executor
            es.state.tools = self
                .session_manager
                .list_tools()
                .iter()
                .map(|t| t.name.clone())
                .collect();
        }

        let info = AgentProcessInfo {
            session_id: session_id.to_string(),
            pid: None,
            state: AgentProcessState::Connected,
            exit_code: None,
            model,
            permission_mode,
            cwd: workspace,
            created_at: now,
            cli_session_id: None,
            archived: false,
            name: None,
            persona_id,
        };

        Ok(info)
    }

    /// Destroy a session and clean up all state.
    pub async fn destroy_session(&self, session_id: &str) -> crate::Result<()> {
        // Cancel any running generation
        {
            let mut sessions = self.sessions.write().await;
            if let Some(es) = sessions.get_mut(session_id) {
                if let Some(handle) = es.generation_handle.take() {
                    handle.abort();
                }
            }
        }

        // Destroy a3s-code session (ignore error if it doesn't exist there)
        let _ = self.session_manager.destroy_session(session_id).await;

        // Remove UI state
        self.sessions.write().await.remove(session_id);

        // Remove from disk
        self.store.remove(session_id).await;

        Ok(())
    }

    /// List all sessions as `AgentProcessInfo`.
    pub async fn list_sessions(&self) -> Vec<AgentProcessInfo> {
        let sessions = self.sessions.read().await;
        sessions.values().map(|es| es.to_process_info()).collect()
    }

    /// Get a single session's info.
    pub async fn get_session(&self, session_id: &str) -> Option<AgentProcessInfo> {
        let sessions = self.sessions.read().await;
        sessions.get(session_id).map(|es| es.to_process_info())
    }

    /// Set a session's display name.
    pub async fn set_name(&self, session_id: &str, name: String) {
        let mut sessions = self.sessions.write().await;
        if let Some(es) = sessions.get_mut(session_id) {
            es.name = Some(name.clone());

            // Notify connected browsers
            let msg = BrowserIncomingMessage::SessionNameUpdate { name };
            let json = serde_json::to_string(&msg).unwrap_or_default();
            for sender in es.browser_senders.values() {
                let _ = sender.send(json.clone());
            }

            self.persist_session(es);
        }
    }

    /// Set a session's archived flag.
    pub async fn set_archived(&self, session_id: &str, archived: bool) {
        let mut sessions = self.sessions.write().await;
        if let Some(es) = sessions.get_mut(session_id) {
            es.archived = archived;
            self.persist_session(es);
        }
    }

    // =========================================================================
    // Browser WebSocket lifecycle
    // =========================================================================

    /// Register a browser WebSocket connection.
    ///
    /// Sends `SessionInit`, `MessageHistory`, and pending permissions for
    /// state replay on reconnect. Returns `false` if the session doesn't exist.
    pub async fn handle_browser_open(
        &self,
        session_id: &str,
        browser_id: &str,
        sender: mpsc::UnboundedSender<String>,
    ) -> bool {
        let mut sessions = self.sessions.write().await;
        let es = match sessions.get_mut(session_id) {
            Some(es) => es,
            None => return false,
        };

        es.touch_activity();

        // Send session_init
        let init_msg = BrowserIncomingMessage::SessionInit {
            session: es.state.clone(),
        };
        if let Ok(json) = serde_json::to_string(&init_msg) {
            let _ = sender.send(json);
        }

        // Send message history
        if !es.message_history.is_empty() {
            let history_msg = BrowserIncomingMessage::MessageHistory {
                messages: es.message_history.clone(),
            };
            if let Ok(json) = serde_json::to_string(&history_msg) {
                let _ = sender.send(json);
            }
        }

        // Send pending permission requests
        for perm in es.pending_permissions.values() {
            let perm_msg = BrowserIncomingMessage::PermissionRequest {
                request: perm.clone(),
            };
            if let Ok(json) = serde_json::to_string(&perm_msg) {
                let _ = sender.send(json);
            }
        }

        // Single-user desktop app — only one browser connection per session.
        // Drop all previous senders before registering the new one.
        es.browser_senders.clear();
        es.browser_senders.insert(browser_id.to_string(), sender);
        true
    }

    /// Unregister a browser WebSocket connection.
    pub async fn handle_browser_close(&self, session_id: &str, browser_id: &str) {
        let mut sessions = self.sessions.write().await;
        if let Some(es) = sessions.get_mut(session_id) {
            es.browser_senders.remove(browser_id);
        }
    }

    // =========================================================================
    // Browser message dispatch
    // =========================================================================

    /// Handle a message from a browser client.
    pub async fn handle_browser_message(&self, session_id: &str, msg: BrowserOutgoingMessage) {
        match msg {
            BrowserOutgoingMessage::UserMessage {
                content, images: _, ..
            } => {
                // Store in history
                let now = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_secs();

                let user_msg = BrowserIncomingMessage::UserMessage {
                    content: content.clone(),
                    timestamp: now,
                };

                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(es) = sessions.get_mut(session_id) {
                        es.touch_activity();
                        es.message_history.push(user_msg.clone());

                        // Broadcast user message to all browsers for echo
                        let json = serde_json::to_string(&user_msg).unwrap_or_default();
                        for sender in es.browser_senders.values() {
                            let _ = sender.send(json.clone());
                        }
                    }
                }

                // Slash command interception — dispatch before LLM
                tracing::debug!(session_id = %session_id, content = %content, content_bytes = ?content.as_bytes(), "Received user message");
                if CommandRegistry::is_command(&content) {
                    tracing::info!(session_id = %session_id, content = %content, "Slash command detected");
                    let ctx = self.build_command_context(session_id).await;
                    if let Some(output) = self.command_registry.dispatch(&content, &ctx) {
                        let cmd_name = content.trim().split_whitespace().next().unwrap_or("/");
                        tracing::info!(session_id = %session_id, command = %cmd_name, "Slash command dispatched");

                        // Handle post-command actions
                        if let Some(ref action) = output.action {
                            match action {
                                CommandAction::Compact => {
                                    let _ = self.session_manager.compact(session_id).await;
                                }
                                CommandAction::ClearHistory => {
                                    let _ = self.session_manager.clear(session_id).await;
                                    // Also clear engine-side message history
                                    let mut sessions = self.sessions.write().await;
                                    if let Some(es) = sessions.get_mut(session_id) {
                                        es.message_history.clear();
                                        es.state.num_turns = 0;
                                        self.persist_session(es);
                                    }
                                }
                                CommandAction::SwitchModel(model) => {
                                    if let Err(e) =
                                        self.configure_model_for_session(session_id, model).await
                                    {
                                        tracing::warn!(session_id, model = %model, "Failed to switch model via /model: {}", e);
                                    } else {
                                        let mut sessions = self.sessions.write().await;
                                        if let Some(es) = sessions.get_mut(session_id) {
                                            es.model = Some(model.clone());
                                            es.state.model = model.clone();
                                        }
                                    }
                                }
                            }
                        }

                        // Send command response to browser
                        self.broadcast(
                            session_id,
                            &BrowserIncomingMessage::CommandResponse {
                                command: cmd_name.to_string(),
                                text: output.text,
                                state_changed: output.state_changed,
                            },
                        )
                        .await;
                        // Ensure browser clears running state (no generation was started)
                        self.broadcast(
                            session_id,
                            &BrowserIncomingMessage::StatusChange {
                                status: Some("idle".to_string()),
                            },
                        )
                        .await;
                        return;
                    }
                }

                // Auto-configure LLM if not yet set (e.g. session created before credentials were available)
                {
                    let cfg = self.code_config.read().await;
                    if let Some(default_model) = cfg.default_model.clone() {
                        drop(cfg);
                        if let Err(e) = self
                            .configure_model_for_session(session_id, &default_model)
                            .await
                        {
                            tracing::debug!(
                                session_id = %session_id,
                                "Auto-configure skipped (already configured or no config): {}",
                                e
                            );
                        }
                    }
                }

                // Start generation
                self.spawn_generation(session_id, &content).await;
            }
            BrowserOutgoingMessage::PermissionResponse {
                request_id,
                behavior,
                ..
            } => {
                let approved = behavior == "allow";
                tracing::info!(
                    session_id = %session_id,
                    request_id = %request_id,
                    behavior = %behavior,
                    approved = approved,
                    "Received PermissionResponse from browser"
                );

                // Remove from pending
                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(es) = sessions.get_mut(session_id) {
                        es.pending_permissions.remove(&request_id);
                    }
                }

                if let Err(e) = self
                    .session_manager
                    .confirm_tool(session_id, &request_id, approved, None)
                    .await
                {
                    tracing::warn!(
                        session_id = %session_id,
                        request_id = %request_id,
                        "Failed to confirm tool: {}",
                        e
                    );
                }
            }
            BrowserOutgoingMessage::Interrupt => {
                // Cancel running generation
                {
                    let mut sessions = self.sessions.write().await;
                    if let Some(es) = sessions.get_mut(session_id) {
                        if let Some(handle) = es.generation_handle.take() {
                            handle.abort();
                        }
                    }
                }
                let _ = self.session_manager.cancel_operation(session_id).await;

                // Notify browsers of idle state
                self.broadcast(
                    session_id,
                    &BrowserIncomingMessage::StatusChange {
                        status: Some("idle".to_string()),
                    },
                )
                .await;
            }
            BrowserOutgoingMessage::SetModel { model } => {
                if let Err(e) = self.configure_model_for_session(session_id, &model).await {
                    tracing::warn!(
                        session_id = %session_id,
                        model = %model,
                        "Failed to set model: {}",
                        e
                    );
                } else {
                    let mut sessions = self.sessions.write().await;
                    if let Some(es) = sessions.get_mut(session_id) {
                        es.model = Some(model.clone());
                        es.state.model = model;
                    }
                }
            }
            BrowserOutgoingMessage::SetPermissionMode { mode } => {
                let (_perm, confirm) = permission_mode_to_policies(&mode);
                if let Err(e) = self
                    .session_manager
                    .set_confirmation_policy(session_id, confirm)
                    .await
                {
                    tracing::warn!(
                        session_id = %session_id,
                        mode = %mode,
                        "Failed to set permission mode: {}",
                        e
                    );
                } else {
                    let mut sessions = self.sessions.write().await;
                    if let Some(es) = sessions.get_mut(session_id) {
                        es.permission_mode = Some(mode.clone());
                        es.state.permission_mode = mode;
                    }
                }
            }
            BrowserOutgoingMessage::SendAgentMessage { target, content } => {
                if let Err(e) = self
                    .publish_agent_message(session_id, &target, &content)
                    .await
                {
                    tracing::warn!(
                        session_id = %session_id,
                        target = %target,
                        "Failed to publish agent message: {}",
                        e
                    );
                }
            }
            BrowserOutgoingMessage::SetAutoExecute { enabled } => {
                self.set_auto_execute(session_id, enabled).await;
            }
        }
    }

    // =========================================================================
    // Generation
    // =========================================================================

    /// Spawn a streaming generation task for the given session.
    async fn spawn_generation(&self, session_id: &str, prompt: &str) {
        // Cancel any existing generation
        {
            let mut sessions = self.sessions.write().await;
            if let Some(es) = sessions.get_mut(session_id) {
                if let Some(handle) = es.generation_handle.take() {
                    handle.abort();
                }
                es.touch_activity();
            }
        }

        // Notify browsers that we're running
        self.broadcast(
            session_id,
            &BrowserIncomingMessage::StatusChange {
                status: Some("running".to_string()),
            },
        )
        .await;

        // Start streaming generation
        let result = self
            .session_manager
            .generate_streaming(session_id, prompt)
            .await;

        let (mut event_rx, _join_handle) = match result {
            Ok((rx, jh)) => (rx, jh),
            Err(e) => {
                tracing::error!(
                    session_id = %session_id,
                    "Failed to start generation: {}",
                    e
                );
                self.broadcast(
                    session_id,
                    &BrowserIncomingMessage::Error {
                        message: format!("Failed to start generation: {}", e),
                    },
                )
                .await;
                self.broadcast(
                    session_id,
                    &BrowserIncomingMessage::StatusChange {
                        status: Some("idle".to_string()),
                    },
                )
                .await;
                return;
            }
        };

        let sessions = self.sessions.clone();
        let store = self.store.clone();
        let sid = session_id.to_string();

        let handle = tokio::spawn(async move {
            let mut text_buffer = String::new();
            // Accumulate content blocks (text + tool_use + tool_result) for the
            // final assistant message so the UI can render tool call details.
            let mut content_blocks: Vec<ContentBlock> = Vec::new();
            // Track tool input deltas keyed by tool_use id
            let mut tool_input_buffers: std::collections::HashMap<String, String> =
                std::collections::HashMap::new();

            while let Some(event) = event_rx.recv().await {
                // Accumulate text for final message
                if let AgentEvent::TextDelta { ref text } = event {
                    text_buffer.push_str(text);
                }

                // Accumulate tool calls into content blocks
                match &event {
                    AgentEvent::ToolStart { id, name } => {
                        // Flush any accumulated text before the tool call
                        if !text_buffer.is_empty() {
                            content_blocks.push(ContentBlock::Text {
                                text: text_buffer.clone(),
                            });
                            text_buffer.clear();
                        }
                        tool_input_buffers.insert(id.clone(), String::new());
                        // We'll add the ToolUse block when we have the full input (at ToolEnd)
                        let _ = name; // used later
                    }
                    AgentEvent::ToolInputDelta { delta } => {
                        // Accumulate partial JSON input — we don't know the tool_id here,
                        // so append to the most recent tool input buffer.
                        if let Some((_id, buf)) = tool_input_buffers.iter_mut().last() {
                            buf.push_str(delta);
                        }
                    }
                    AgentEvent::ToolEnd {
                        id,
                        name,
                        output,
                        exit_code,
                    } => {
                        let input_str = tool_input_buffers.remove(id).unwrap_or_default();
                        let input_val: serde_json::Value =
                            serde_json::from_str(&input_str).unwrap_or(serde_json::json!({}));
                        content_blocks.push(ContentBlock::ToolUse {
                            id: id.clone(),
                            name: name.clone(),
                            input: input_val,
                        });
                        content_blocks.push(ContentBlock::ToolResult {
                            tool_use_id: id.clone(),
                            content: serde_json::Value::String(output.clone()),
                            is_error: *exit_code != 0,
                        });
                    }
                    _ => {}
                }

                let browser_messages = translate_event(&event);

                // For End events, we broadcast our own rich assistant message
                // (with tool_use blocks), so filter out the text-only one from
                // translate_event to avoid duplicates.
                // Also replace the Result message with a richer one that includes
                // session stats (context usage, token counts, etc.).
                let browser_messages: Vec<_> = if matches!(event, AgentEvent::End { .. }) {
                    browser_messages
                        .into_iter()
                        .filter(|m| !matches!(m, BrowserIncomingMessage::Assistant { .. }))
                        .map(|m| {
                            if let BrowserIncomingMessage::Result { ref data } = m {
                                // Enrich Result with session stats
                                let mut enriched = data.clone();
                                if let Ok(sessions_guard) = sessions.try_read() {
                                    if let Some(es) = sessions_guard.get(&sid) {
                                        enriched["num_turns"] =
                                            serde_json::json!(es.state.num_turns);
                                        enriched["total_cost_usd"] =
                                            serde_json::json!(es.state.total_cost_usd);
                                    }
                                }
                                // Add token usage from the End event
                                if let AgentEvent::End { ref usage, .. } = event {
                                    enriched["input_tokens"] =
                                        serde_json::json!(usage.prompt_tokens);
                                    enriched["output_tokens"] =
                                        serde_json::json!(usage.completion_tokens);
                                    enriched["cache_read_tokens"] =
                                        serde_json::json!(usage.cache_read_tokens.unwrap_or(0));
                                    enriched["cache_write_tokens"] =
                                        serde_json::json!(usage.cache_write_tokens.unwrap_or(0));
                                    // Estimate context usage: prompt_tokens / max_context (200k default)
                                    let max_ctx = 200_000u64;
                                    let pct = ((usage.prompt_tokens as f64 / max_ctx as f64)
                                        * 100.0)
                                        .min(100.0);
                                    enriched["context_used_percent"] = serde_json::json!(pct);
                                }
                                BrowserIncomingMessage::Result { data: enriched }
                            } else {
                                m
                            }
                        })
                        .collect()
                } else {
                    browser_messages
                };

                // Log all events for debugging
                tracing::debug!(
                    session_id = %sid,
                    event_type = ?std::mem::discriminant(&event),
                    num_browser_msgs = browser_messages.len(),
                    "Agent event received"
                );

                // Store permissions and update session state
                match &event {
                    AgentEvent::ConfirmationRequired {
                        tool_id,
                        tool_name,
                        args,
                        ..
                    } => {
                        tracing::info!(
                            session_id = %sid,
                            tool_id = %tool_id,
                            tool_name = %tool_name,
                            "ConfirmationRequired — sending permission_request to browser"
                        );
                        let now = std::time::SystemTime::now()
                            .duration_since(std::time::UNIX_EPOCH)
                            .unwrap_or_default()
                            .as_secs();
                        let perm = PermissionRequest {
                            request_id: tool_id.clone(),
                            tool_name: tool_name.clone(),
                            input: args.clone(),
                            permission_suggestions: None,
                            description: None,
                            tool_use_id: Some(tool_id.clone()),
                            agent_id: None,
                            timestamp: now,
                        };
                        let mut sessions = sessions.write().await;
                        if let Some(es) = sessions.get_mut(&sid) {
                            es.pending_permissions.insert(tool_id.clone(), perm);
                        }
                    }
                    AgentEvent::ConfirmationReceived { tool_id, .. }
                    | AgentEvent::ConfirmationTimeout { tool_id, .. } => {
                        let mut sessions = sessions.write().await;
                        if let Some(es) = sessions.get_mut(&sid) {
                            es.pending_permissions.remove(tool_id);
                        }
                    }
                    AgentEvent::TurnEnd { turn, usage } => {
                        let mut sessions = sessions.write().await;
                        if let Some(es) = sessions.get_mut(&sid) {
                            es.state.num_turns = *turn as u32;
                            es.state.total_cost_usd += estimate_cost_from_usage(usage);
                        }
                    }
                    AgentEvent::End { ref text, .. } => {
                        // Flush any remaining text into content blocks
                        let final_text = if text.is_empty() {
                            text_buffer.clone()
                        } else {
                            text.clone()
                        };
                        if !final_text.is_empty() {
                            content_blocks.push(ContentBlock::Text { text: final_text });
                        }
                        // If no blocks accumulated at all, add empty text
                        if content_blocks.is_empty() {
                            content_blocks.push(ContentBlock::Text {
                                text: String::new(),
                            });
                        }

                        // Build complete assistant message with all content blocks
                        // (tool_use + tool_result + text) for rich UI rendering
                        let assistant_msg = BrowserIncomingMessage::Assistant {
                            message: AssistantMessageBody {
                                id: uuid::Uuid::new_v4().to_string(),
                                msg_type: Some("message".to_string()),
                                role: "assistant".to_string(),
                                model: String::new(),
                                content: content_blocks.clone(),
                                stop_reason: Some("end_turn".to_string()),
                                usage: None,
                            },
                            parent_tool_use_id: None,
                        };

                        // Store in history AND broadcast the rich message directly
                        // (replacing the text-only one from translate_event)
                        let mut sessions = sessions.write().await;
                        if let Some(es) = sessions.get_mut(&sid) {
                            es.message_history.push(assistant_msg.clone());
                            persist_session_with_store(es, &store);
                            // Broadcast the rich assistant message
                            if let Ok(json) = serde_json::to_string(&assistant_msg) {
                                for sender in es.browser_senders.values() {
                                    let _ = sender.send(json.clone());
                                }
                            }
                        }
                    }
                    _ => {}
                }

                // Broadcast translated messages to all browsers
                let sessions_read = sessions.read().await;
                if let Some(es) = sessions_read.get(&sid) {
                    for browser_msg in &browser_messages {
                        if let Ok(json) = serde_json::to_string(browser_msg) {
                            for sender in es.browser_senders.values() {
                                let _ = sender.send(json.clone());
                            }
                        }
                    }
                }
            }

            // Generation complete — send idle status
            let idle = BrowserIncomingMessage::StatusChange {
                status: Some("idle".to_string()),
            };
            let sessions_read = sessions.read().await;
            if let Some(es) = sessions_read.get(&sid) {
                if let Ok(json) = serde_json::to_string(&idle) {
                    for sender in es.browser_senders.values() {
                        let _ = sender.send(json.clone());
                    }
                }
            }
            drop(sessions_read);

            // Clear generation handle
            let mut sessions = sessions.write().await;
            if let Some(es) = sessions.get_mut(&sid) {
                es.generation_handle = None;
            }
        });

        // Store the handle
        let mut sessions = self.sessions.write().await;
        if let Some(es) = sessions.get_mut(session_id) {
            es.generation_handle = Some(handle);
        }
    }

    // =========================================================================
    // Channel message processing (non-WebSocket)
    // =========================================================================

    /// Generate a text response for a channel message.
    ///
    /// Unlike `spawn_generation` (browser WebSocket), this method collects all
    /// streaming events and returns the final text. Used by the Runtime's event
    /// processor to handle messages from Telegram, Slack, Discord, etc.
    ///
    /// If no agent session exists for the given ID, one is created with
    /// `trust` permission mode (auto-approve all tool calls for chat channels).
    pub async fn generate_response(&self, session_id: &str, prompt: &str) -> crate::Result<String> {
        // Ensure agent session exists, configured with the default model
        if self.get_session(session_id).await.is_none() {
            let default_model = self.code_config.read().await.default_model.clone();
            self.create_session(
                session_id,
                default_model,
                Some("trust".to_string()),
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
        }

        // Start streaming generation
        let (mut event_rx, _join_handle) = self
            .session_manager
            .generate_streaming(session_id, prompt)
            .await
            .map_err(|e| crate::Error::Runtime(format!("Failed to start generation: {}", e)))?;

        // Collect text from streaming events with a timeout
        let mut text = String::new();
        let timeout_duration = std::time::Duration::from_secs(120);
        loop {
            match tokio::time::timeout(timeout_duration, event_rx.recv()).await {
                Ok(Some(event)) => match &event {
                    AgentEvent::TextDelta { text: delta } => {
                        text.push_str(delta);
                    }
                    AgentEvent::End {
                        text: final_text, ..
                    } => {
                        if !final_text.is_empty() {
                            return Ok(final_text.clone());
                        }
                        break;
                    }
                    AgentEvent::Error { message } => {
                        return Err(crate::Error::Runtime(format!("Agent error: {}", message)));
                    }
                    _ => {}
                },
                Ok(None) => break, // channel closed
                Err(_) => {
                    tracing::warn!(session_id, "Agent generation timed out after 120s");
                    break;
                }
            }
        }

        if text.is_empty() {
            Ok("I received your message but couldn't generate a response.".to_string())
        } else {
            Ok(text)
        }
    }

    /// Start streaming generation and return the event receiver.
    ///
    /// Unlike `generate_response` which collects all events into a final string,
    /// this returns the raw event receiver so callers can process events
    /// incrementally (e.g., for progressive message updates in chat channels).
    pub async fn generate_response_streaming(
        &self,
        session_id: &str,
        prompt: &str,
    ) -> crate::Result<(
        mpsc::Receiver<AgentEvent>,
        tokio::task::JoinHandle<anyhow::Result<a3s_code::agent::AgentResult>>,
    )> {
        if self.get_session(session_id).await.is_none() {
            let default_model = self.code_config.read().await.default_model.clone();
            self.create_session(
                session_id,
                default_model,
                Some("trust".to_string()),
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
        }

        self.session_manager
            .generate_streaming(session_id, prompt)
            .await
            .map_err(|e| crate::Error::Runtime(format!("Failed to start generation: {}", e)))
    }

    // =========================================================================
    // Helpers
    // =========================================================================

    /// Broadcast a message to all browser connections for a session.
    async fn broadcast(&self, session_id: &str, msg: &BrowserIncomingMessage) {
        let sessions = self.sessions.read().await;
        if let Some(es) = sessions.get(session_id) {
            if let Ok(json) = serde_json::to_string(msg) {
                for sender in es.browser_senders.values() {
                    let _ = sender.send(json.clone());
                }
            }
        }
    }

    // =========================================================================
    // Agent bus integration
    // =========================================================================

    /// Attach an `AgentBus` to this engine.
    ///
    /// Called once during Runtime startup after the bus is created.
    pub async fn set_bus(&self, bus: Arc<crate::agent::bus::AgentBus>) {
        *self.agent_bus.write().await = Some(bus);
    }

    /// Publish a message to another agent via the event bus.
    ///
    /// `target`: `"broadcast:<topic>"` or `"mention:<session_id>"`
    pub async fn publish_agent_message(
        &self,
        from_session_id: &str,
        target: &str,
        content: &str,
    ) -> crate::Result<()> {
        let bus = self.agent_bus.read().await;
        match bus.as_ref() {
            Some(b) => b.publish(from_session_id, target, content).await,
            None => Err(crate::Error::Runtime(
                "Agent bus not configured".to_string(),
            )),
        }
    }

    /// Set auto-execute mode for incoming agent messages on a session.
    pub async fn set_auto_execute(&self, session_id: &str, enabled: bool) {
        let bus = self.agent_bus.read().await;
        if let Some(b) = bus.as_ref() {
            b.set_auto_execute(session_id, enabled).await;
        }
    }

    /// Get auto-execute mode for a session (default: false).
    pub async fn get_auto_execute(&self, session_id: &str) -> bool {
        let bus = self.agent_bus.read().await;
        match bus.as_ref() {
            Some(b) => b.get_auto_execute(session_id).await,
            None => false,
        }
    }

    /// Broadcast a message to all browser connections for a session (public).
    ///
    /// Used by `AgentBus` to deliver incoming agent messages to the browser.
    pub async fn broadcast_to_session(&self, session_id: &str, msg: &BrowserIncomingMessage) {
        self.broadcast(session_id, msg).await;
    }

    // =========================================================================
    // Slash commands
    // =========================================================================

    /// Build a `CommandContext` for slash command dispatch.
    async fn build_command_context(&self, session_id: &str) -> CommandContext {
        let sessions = self.sessions.read().await;
        let es = sessions.get(session_id);

        let (workspace, model, history_len, total_cost) = match es {
            Some(es) => (
                es.cwd.clone(),
                es.model.clone().unwrap_or_default(),
                es.message_history.len(),
                es.state.total_cost_usd,
            ),
            None => (String::new(), String::new(), 0, 0.0),
        };

        let tool_names: Vec<String> = self
            .session_manager
            .list_tools()
            .iter()
            .map(|t| t.name.clone())
            .collect();

        CommandContext {
            session_id: session_id.to_string(),
            workspace,
            model,
            history_len,
            total_tokens: 0,
            total_cost,
            tool_names,
            mcp_servers: Vec::new(),
        }
    }

    /// List all available slash commands (name + description).
    pub fn list_commands(&self) -> Vec<(&str, &str)> {
        self.command_registry.list()
    }

    /// Resolve a persona's system prompt content by ID.
    ///
    /// Looks up the persona skill from the session manager's skill registry
    /// and returns its content for injection as the session system prompt.
    async fn resolve_persona_system_prompt(&self, persona_id: &str) -> Option<String> {
        let registry = self.session_manager.skill_registry().await?;
        let skill = registry.get(persona_id)?;
        if skill.kind != a3s_code::skills::SkillKind::Persona {
            tracing::warn!(persona_id, "Skill exists but is not a Persona kind");
            return None;
        }
        Some(skill.content.clone())
    }

    /// List all available personas from the skill registry.
    pub async fn list_personas(&self) -> Vec<PersonaInfo> {
        let Some(registry) = self.session_manager.skill_registry().await else {
            return Vec::new();
        };
        registry
            .personas()
            .into_iter()
            .map(|s| PersonaInfo {
                id: s.name.clone(),
                name: s.name.clone(),
                description: s.description.clone(),
                tags: s.tags.clone(),
                version: s.version.clone(),
            })
            .collect()
    }

    /// Configure the LLM client for a session based on model ID.
    ///
    /// Public wrapper for configure_model_for_session (used by handler)
    pub async fn configure_model_for_session_pub(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> crate::Result<()> {
        self.configure_model_for_session(session_id, model_id).await
    }

    /// Public wrapper for configure_model_with_credentials (used by handler)
    pub async fn configure_model_with_credentials_pub(
        &self,
        session_id: &str,
        model_id: &str,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> crate::Result<()> {
        self.configure_model_with_credentials(session_id, model_id, api_key, base_url)
            .await
    }

    /// Searches all providers for the given model ID and constructs the
    /// appropriate `LlmConfig`.
    ///
    /// When the config has a matching provider/model but no API key (common for
    /// free or env-var-authenticated endpoints like Kimi), falls back to
    /// `configure_model_with_credentials` which creates a client with an empty
    /// key — letting the underlying HTTP client or proxy handle auth.
    async fn configure_model_for_session(
        &self,
        session_id: &str,
        model_id: &str,
    ) -> crate::Result<()> {
        // model_id may be "provider/model" format or just "model"
        let cfg = self.code_config.read().await;
        let llm_config = if let Some((provider_name, bare_model)) = model_id.split_once('/') {
            cfg.llm_config(provider_name, bare_model)
        } else {
            cfg.providers
                .iter()
                .find_map(|p| cfg.llm_config(&p.name, model_id))
        };
        drop(cfg);

        if let Some(llm_config) = llm_config {
            self.session_manager
                .configure(session_id, None, None, Some(llm_config))
                .await
                .map_err(|e| crate::Error::Runtime(format!("Failed to configure session: {}", e)))
        } else {
            // No full LlmConfig (likely missing API key in config) — fall back to
            // credential-based configuration which tolerates empty keys.
            tracing::debug!(
                session_id = %session_id,
                model = %model_id,
                "No API key in config for model, falling back to credential-based configure"
            );
            self.configure_model_with_credentials(session_id, model_id, None, None)
                .await
        }
    }

    /// Configure LLM client for a session using explicit credentials (api_key / base_url).
    /// Used when the frontend passes credentials directly rather than relying on HCL config.
    ///
    /// Resolution order for API key:
    ///   1. Explicit `api_key` parameter
    ///   2. Provider-level `api_key` from config
    ///   3. Model-level `api_key` from config
    ///   4. Environment variable: `<PROVIDER>_API_KEY` (e.g. `OPENAI_API_KEY`)
    ///
    /// Resolution order for base URL:
    ///   1. Explicit `base_url` parameter
    ///   2. Model-level `base_url` from config
    ///   3. Provider-level `base_url` from config
    ///   4. Environment variable: `<PROVIDER>_BASE_URL` (e.g. `OPENAI_BASE_URL`)
    async fn configure_model_with_credentials(
        &self,
        session_id: &str,
        model_id: &str,
        api_key: Option<&str>,
        base_url: Option<&str>,
    ) -> crate::Result<()> {
        use a3s_code::llm::factory::LlmConfig;

        // Determine provider from "provider/model" format or fall back to config lookup
        let (provider, bare_model) = if let Some((p, m)) = model_id.split_once('/') {
            (p.to_string(), m.to_string())
        } else {
            let cfg = self.code_config.read().await;
            let provider = cfg
                .providers
                .iter()
                .find(|p| p.models.iter().any(|m| m.id == model_id))
                .map(|p| p.name.clone())
                .unwrap_or_else(|| "anthropic".to_string());
            (provider, model_id.to_string())
        };

        // Resolve API key: explicit > config (model > provider) > env var
        let resolved_key = if let Some(k) = api_key.filter(|k| !k.is_empty()) {
            k.to_string()
        } else {
            let cfg = self.code_config.read().await;
            let from_config = cfg
                .providers
                .iter()
                .find(|p| p.name == provider)
                .and_then(|p| {
                    // Model-level key first, then provider-level
                    let model_key = p
                        .models
                        .iter()
                        .find(|m| m.id == bare_model)
                        .and_then(|m| m.api_key.clone());
                    model_key.or_else(|| p.api_key.clone())
                })
                .unwrap_or_default();
            if from_config.is_empty() {
                // Fall back to env var: <PROVIDER>_API_KEY (e.g. OPENAI_API_KEY)
                let env_key = format!("{}_API_KEY", provider.to_uppercase());
                std::env::var(&env_key).unwrap_or_default()
            } else {
                from_config
            }
        };

        // Resolve base URL: explicit > config (model > provider) > env var
        let resolved_base_url = if let Some(url) = base_url.filter(|u| !u.is_empty()) {
            Some(url.to_string())
        } else {
            let cfg = self.code_config.read().await;
            let from_config = cfg
                .providers
                .iter()
                .find(|p| p.name == provider)
                .and_then(|p| {
                    let model_url = p
                        .models
                        .iter()
                        .find(|m| m.id == bare_model)
                        .and_then(|m| m.base_url.clone());
                    model_url.or_else(|| p.base_url.clone())
                });
            if from_config.is_some() {
                from_config
            } else {
                // Fall back to env var: <PROVIDER>_BASE_URL (e.g. OPENAI_BASE_URL)
                let env_key = format!("{}_BASE_URL", provider.to_uppercase());
                std::env::var(&env_key).ok()
            }
        };

        let mut llm_config = LlmConfig::new(&provider, &bare_model, resolved_key);
        if let Some(url) = resolved_base_url {
            llm_config.base_url = Some(url);
        }

        tracing::info!(
            session_id = %session_id,
            provider = %provider,
            model = %bare_model,
            has_api_key = !llm_config.api_key.expose().is_empty(),
            has_base_url = llm_config.base_url.is_some(),
            "Configuring LLM with credentials"
        );

        self.session_manager
            .configure(session_id, None, None, Some(llm_config))
            .await
            .map_err(|e| crate::Error::Runtime(format!("Failed to configure session: {}", e)))
    }

    /// Persist an engine session to disk via the store.
    fn persist_session(&self, es: &EngineSession) {
        let persisted = PersistedAgentSession {
            id: es.id.clone(),
            state: es.state.clone(),
            message_history: es.message_history.clone(),
            pending_messages: Vec::new(),
            pending_permissions: es.pending_permissions.clone(),
            archived: es.archived,
        };
        self.store.save_sync(&persisted);
    }

    /// Restore sessions from disk on startup.
    async fn restore_from_disk(&self) {
        let persisted_sessions = self.store.load_all();
        let mut sessions = self.sessions.write().await;

        for ps in persisted_sessions {
            let mut es = EngineSession {
                id: ps.id.clone(),
                browser_senders: HashMap::new(),
                state: ps.state.clone(),
                message_history: ps.message_history,
                pending_permissions: ps.pending_permissions,
                generation_handle: None,
                name: None,
                archived: ps.archived,
                created_at: 0,
                cwd: ps.state.cwd.clone(),
                model: if ps.state.model.is_empty() {
                    None
                } else {
                    Some(ps.state.model.clone())
                },
                permission_mode: Some(ps.state.permission_mode.clone()),
                persona_id: ps.state.persona_id.clone(),
                last_activity_at: 0,
            };

            // Try to restore a3s-code session with full LLM history first
            let restored = self
                .session_manager
                .restore_session_by_id(&ps.id)
                .await
                .is_ok();

            if !restored {
                // Fallback: create fresh session if a3s-code store doesn't have it
                let session_config = a3s_code::session::SessionConfig {
                    name: String::new(),
                    workspace: es.cwd.clone(),
                    ..Default::default()
                };
                let _ = self
                    .session_manager
                    .create_session(ps.id.clone(), session_config)
                    .await;
                tracing::debug!(session_id = %ps.id, "Created fresh a3s-code session (no stored LLM history)");
            } else {
                tracing::debug!(session_id = %ps.id, "Restored a3s-code session with LLM history");
            }

            // Re-apply model config (API keys aren't persisted in a3s-code store).
            // If the saved model is no longer in config, fall back to the default model.
            let mut configured = false;
            if !ps.state.model.is_empty() {
                if let Err(e) = self
                    .configure_model_for_session(&ps.id, &ps.state.model)
                    .await
                {
                    tracing::warn!(
                        session_id = %ps.id,
                        model = %ps.state.model,
                        "Failed to re-configure saved model after restore: {}, will try default",
                        e
                    );
                } else {
                    configured = true;
                }
            }
            if !configured {
                let cfg = self.code_config.read().await;
                if let Some(ref default_model) = cfg.default_model {
                    let dm = default_model.clone();
                    drop(cfg);
                    if let Err(e) = self.configure_model_for_session(&ps.id, &dm).await {
                        tracing::warn!(
                            session_id = %ps.id,
                            model = %dm,
                            "Failed to configure default model after restore: {}",
                            e
                        );
                    } else {
                        // Update session state to reflect the actual model in use
                        es.model = Some(dm.clone());
                        es.state.model = dm;
                    }
                }
            }

            sessions.insert(ps.id, es);
        }

        tracing::info!(count = sessions.len(), "Restored agent sessions from disk");
    }

    // =========================================================================
    // Session lifecycle management
    // =========================================================================

    /// Start the background session lifecycle task.
    ///
    /// Periodically scans sessions and:
    /// - Auto-archives idle sessions (no browser + no generation + exceeded idle timeout)
    /// - Purges archived sessions older than purge_after
    /// - Enforces max_sessions by archiving the oldest idle sessions
    pub fn start_lifecycle_task(self: &Arc<Self>, config: crate::config::SessionLifecycleConfig) {
        let engine = Arc::clone(self);
        let interval = std::time::Duration::from_secs(config.cleanup_interval_secs);

        tracing::info!(
            idle_timeout = config.idle_timeout_secs,
            purge_after = config.purge_after_secs,
            max_sessions = config.max_sessions,
            interval = config.cleanup_interval_secs,
            "Session lifecycle task started"
        );

        tokio::spawn(async move {
            let mut ticker = tokio::time::interval(interval);
            ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

            loop {
                ticker.tick().await;
                engine.run_lifecycle_sweep(&config).await;
            }
        });
    }

    /// Run a single lifecycle sweep.
    async fn run_lifecycle_sweep(&self, config: &crate::config::SessionLifecycleConfig) {
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        let mut to_archive = Vec::new();
        let mut to_purge = Vec::new();

        // Phase 1: identify sessions to archive or purge
        {
            let sessions = self.sessions.read().await;
            for (sid, es) in sessions.iter() {
                let idle_secs = now.saturating_sub(es.last_activity_at);
                let has_browsers = !es.browser_senders.is_empty();
                let has_generation = es.generation_handle.is_some();

                if es.archived {
                    // Purge archived sessions past retention
                    if idle_secs > config.purge_after_secs {
                        to_purge.push(sid.clone());
                    }
                } else if !has_browsers && !has_generation && idle_secs > config.idle_timeout_secs {
                    // Auto-archive idle sessions
                    to_archive.push(sid.clone());
                }
            }
        }

        // Phase 2: enforce max_sessions — archive oldest idle sessions
        {
            let sessions = self.sessions.read().await;
            let active_count = sessions.values().filter(|es| !es.archived).count();
            if active_count > config.max_sessions {
                let excess = active_count - config.max_sessions;
                let mut candidates: Vec<_> = sessions
                    .iter()
                    .filter(|(sid, es)| {
                        !es.archived
                            && es.browser_senders.is_empty()
                            && es.generation_handle.is_none()
                            && !to_archive.contains(sid)
                    })
                    .map(|(sid, es)| (sid.clone(), es.last_activity_at))
                    .collect();
                candidates.sort_by_key(|(_, ts)| *ts);
                for (sid, _) in candidates.into_iter().take(excess) {
                    to_archive.push(sid);
                }
            }
        }

        // Phase 3: execute
        for sid in &to_archive {
            self.set_archived(sid, true).await;
            tracing::info!(session_id = %sid, "Auto-archived idle session");
        }

        for sid in &to_purge {
            if let Err(e) = self.destroy_session(sid).await {
                tracing::warn!(session_id = %sid, "Failed to purge archived session: {}", e);
            } else {
                tracing::info!(session_id = %sid, "Purged archived session");
            }
        }
    }

    /// Graceful shutdown: cancel all generations and persist all sessions.
    pub async fn shutdown(&self) {
        tracing::info!("Shutting down agent engine...");

        let mut sessions = self.sessions.write().await;
        for (sid, es) in sessions.iter_mut() {
            // Cancel running generations
            if let Some(handle) = es.generation_handle.take() {
                handle.abort();
                tracing::debug!(session_id = %sid, "Cancelled generation on shutdown");
            }
            // Persist final state
            persist_session_with_store(es, &self.store);
        }

        tracing::info!(count = sessions.len(), "Agent engine shutdown complete");
    }

    /// List all sessions as an agent discovery directory.
    pub async fn list_agent_directory(&self) -> Vec<AgentDirectoryEntry> {
        let sessions = self.sessions.read().await;
        let mut entries = Vec::new();

        for es in sessions.values() {
            let status = if es.archived {
                "archived"
            } else if es.generation_handle.is_some() {
                "busy"
            } else if !es.browser_senders.is_empty() {
                "active"
            } else {
                "idle"
            };

            let auto_execute = match self.agent_bus.try_read() {
                Ok(guard) => match guard.as_ref() {
                    Some(bus) => bus.get_auto_execute(&es.id).await,
                    None => false,
                },
                Err(_) => false,
            };

            entries.push(AgentDirectoryEntry {
                session_id: es.id.clone(),
                persona_id: es.persona_id.clone(),
                persona_name: es.persona_id.clone(), // persona_id is the skill name
                status: status.to_string(),
                auto_execute,
            });
        }

        entries
    }

    /// Get session statistics.
    pub async fn session_stats(&self) -> SessionStats {
        let sessions = self.sessions.read().await;
        let mut active = 0u32;
        let mut archived = 0u32;
        let mut with_browsers = 0u32;
        let mut generating = 0u32;
        let mut total_cost = 0.0f64;

        for es in sessions.values() {
            if es.archived {
                archived += 1;
            } else {
                active += 1;
            }
            if !es.browser_senders.is_empty() {
                with_browsers += 1;
            }
            if es.generation_handle.is_some() {
                generating += 1;
            }
            total_cost += es.state.total_cost_usd;
        }

        SessionStats {
            active,
            archived,
            with_browsers,
            generating,
            total: sessions.len() as u32,
            total_cost_usd: total_cost,
        }
    }
}

/// Persist session helper that can be called from spawned tasks.
fn persist_session_with_store(es: &EngineSession, store: &AgentSessionStore) {
    let persisted = PersistedAgentSession {
        id: es.id.clone(),
        state: es.state.clone(),
        message_history: es.message_history.clone(),
        pending_messages: Vec::new(),
        pending_permissions: es.pending_permissions.clone(),
        archived: es.archived,
    };
    store.save_sync(&persisted);
}

impl EngineSession {
    /// Update the last activity timestamp to now.
    fn touch_activity(&mut self) {
        self.last_activity_at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
    }

    /// Convert to the REST API response type.
    fn to_process_info(&self) -> AgentProcessInfo {
        AgentProcessInfo {
            session_id: self.id.clone(),
            pid: None,
            state: if self.generation_handle.is_some() {
                AgentProcessState::Running
            } else {
                AgentProcessState::Connected
            },
            exit_code: None,
            model: self.model.clone(),
            permission_mode: self.permission_mode.clone(),
            cwd: self.cwd.clone(),
            created_at: self.created_at,
            cli_session_id: None,
            archived: self.archived,
            name: self.name.clone(),
            persona_id: self.persona_id.clone(),
        }
    }
}

// =============================================================================
// Event translation (pure functions)
// =============================================================================

/// Translate an `AgentEvent` into zero or more `BrowserIncomingMessage`s.
///
/// This is a pure function with no side effects, making it easy to test.
pub fn translate_event(event: &AgentEvent) -> Vec<BrowserIncomingMessage> {
    match event {
        AgentEvent::Start { .. } => {
            vec![BrowserIncomingMessage::StatusChange {
                status: Some("running".to_string()),
            }]
        }
        AgentEvent::TextDelta { text } => {
            vec![BrowserIncomingMessage::StreamEvent {
                event: serde_json::json!({
                    "type": "content_block_delta",
                    "delta": {
                        "type": "text_delta",
                        "text": text,
                    }
                }),
                parent_tool_use_id: None,
            }]
        }
        AgentEvent::ToolStart { id, name } => {
            vec![BrowserIncomingMessage::StreamEvent {
                event: serde_json::json!({
                    "type": "content_block_start",
                    "content_block": {
                        "type": "tool_use",
                        "id": id,
                        "name": name,
                    }
                }),
                parent_tool_use_id: None,
            }]
        }
        AgentEvent::ToolEnd {
            id,
            name,
            output,
            exit_code,
        } => {
            // Send tool_end stream event with output for UI display
            vec![BrowserIncomingMessage::StreamEvent {
                event: serde_json::json!({
                    "type": "tool_end",
                    "tool_use_id": id,
                    "tool_name": name,
                    "output": if output.len() > 500 {
                        format!("{}…", &output[..500])
                    } else {
                        output.clone()
                    },
                    "is_error": *exit_code != 0,
                }),
                parent_tool_use_id: Some(id.clone()),
            }]
        }
        AgentEvent::TurnEnd { turn, usage } => {
            vec![BrowserIncomingMessage::SessionUpdate {
                session: serde_json::json!({
                    "num_turns": turn,
                    "total_cost_usd": estimate_cost_from_usage(usage),
                    "context_used_percent": 0.0,
                }),
            }]
        }
        AgentEvent::End { text, usage } => {
            let result_data = serde_json::json!({
                "subtype": "success",
                "is_error": false,
                "result": text,
                "total_cost_usd": estimate_cost_from_usage(usage),
            });
            vec![
                BrowserIncomingMessage::Assistant {
                    message: AssistantMessageBody {
                        id: uuid::Uuid::new_v4().to_string(),
                        msg_type: Some("message".to_string()),
                        role: "assistant".to_string(),
                        model: String::new(),
                        content: vec![ContentBlock::Text { text: text.clone() }],
                        stop_reason: Some("end_turn".to_string()),
                        usage: None,
                    },
                    parent_tool_use_id: None,
                },
                BrowserIncomingMessage::Result { data: result_data },
                BrowserIncomingMessage::StatusChange {
                    status: Some("idle".to_string()),
                },
            ]
        }
        AgentEvent::Error { message } => {
            vec![BrowserIncomingMessage::Error {
                message: message.clone(),
            }]
        }
        AgentEvent::ConfirmationRequired {
            tool_id,
            tool_name,
            args,
            ..
        } => {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            vec![BrowserIncomingMessage::PermissionRequest {
                request: PermissionRequest {
                    request_id: tool_id.clone(),
                    tool_name: tool_name.clone(),
                    input: args.clone(),
                    permission_suggestions: None,
                    description: None,
                    tool_use_id: Some(tool_id.clone()),
                    agent_id: None,
                    timestamp: now,
                },
            }]
        }
        AgentEvent::ConfirmationReceived { tool_id, .. }
        | AgentEvent::ConfirmationTimeout { tool_id, .. } => {
            vec![BrowserIncomingMessage::PermissionCancelled {
                request_id: tool_id.clone(),
            }]
        }
        AgentEvent::PermissionDenied { reason, .. } => {
            vec![BrowserIncomingMessage::Error {
                message: format!("Permission denied: {}", reason),
            }]
        }
        AgentEvent::SubagentStart { .. }
        | AgentEvent::SubagentProgress { .. }
        | AgentEvent::SubagentEnd { .. } => {
            // Forward subagent events as generic stream events
            if let Ok(val) = serde_json::to_value(event) {
                vec![BrowserIncomingMessage::StreamEvent {
                    event: val,
                    parent_tool_use_id: None,
                }]
            } else {
                vec![]
            }
        }
        AgentEvent::TaskUpdated { .. } => {
            if let Ok(val) = serde_json::to_value(event) {
                vec![BrowserIncomingMessage::StreamEvent {
                    event: val,
                    parent_tool_use_id: None,
                }]
            } else {
                vec![]
            }
        }
        AgentEvent::ContextCompacted { .. } => {
            vec![BrowserIncomingMessage::SessionUpdate {
                session: serde_json::json!({
                    "is_compacting": false,
                }),
            }]
        }
        AgentEvent::ToolOutputDelta { id, name, delta } => {
            vec![BrowserIncomingMessage::StreamEvent {
                event: serde_json::json!({
                    "type": "tool_output_delta",
                    "tool_use_id": id,
                    "tool_name": name,
                    "delta": delta,
                }),
                parent_tool_use_id: Some(id.clone()),
            }]
        }
        // Suppress internal events that don't need to reach the browser
        AgentEvent::TurnStart { .. }
        | AgentEvent::ContextResolving { .. }
        | AgentEvent::ContextResolved { .. }
        | AgentEvent::CommandDeadLettered { .. }
        | AgentEvent::CommandRetry { .. }
        | AgentEvent::QueueAlert { .. }
        | AgentEvent::MemoryStored { .. }
        | AgentEvent::MemoryRecalled { .. }
        | AgentEvent::MemoriesSearched { .. }
        | AgentEvent::MemoryCleared { .. }
        | AgentEvent::PlanningStart { .. }
        | AgentEvent::PlanningEnd { .. }
        | AgentEvent::StepStart { .. }
        | AgentEvent::StepEnd { .. }
        | AgentEvent::GoalExtracted { .. }
        | AgentEvent::GoalProgress { .. }
        | AgentEvent::GoalAchieved { .. }
        | AgentEvent::ExternalTaskPending { .. }
        | AgentEvent::ExternalTaskCompleted { .. }
        | AgentEvent::PersistenceFailed { .. } => {
            vec![]
        }
        // Catch-all for any new AgentEvent variants added upstream
        _ => {
            vec![]
        }
    }
}

/// Convert a permission mode string to permission and confirmation policies.
fn permission_mode_to_policies(
    mode: &str,
) -> (
    a3s_code::permissions::PermissionPolicy,
    a3s_code::hitl::ConfirmationPolicy,
) {
    match mode {
        "plan" | "strict" => (
            a3s_code::permissions::PermissionPolicy::strict(),
            a3s_code::hitl::ConfirmationPolicy::enabled()
                .with_timeout(120_000, a3s_code::hitl::TimeoutAction::Reject),
        ),
        "yolo" | "permissive" | "trust" | "bypassPermissions" => (
            a3s_code::permissions::PermissionPolicy::permissive(),
            a3s_code::hitl::ConfirmationPolicy::default(),
        ),
        _ => (
            // Default "agent" mode: auto-approve read-only (Query lane),
            // require confirmation for write/execute operations.
            // 120s timeout for browser-based HITL (users need time to review).
            a3s_code::permissions::PermissionPolicy::new(),
            a3s_code::hitl::ConfirmationPolicy::enabled()
                .with_yolo_lanes([a3s_code::queue::SessionLane::Query])
                .with_timeout(120_000, a3s_code::hitl::TimeoutAction::Reject),
        ),
    }
}

/// Rough cost estimate from `TokenUsage`.
fn estimate_cost_from_usage(usage: &a3s_code::llm::TokenUsage) -> f64 {
    // Rough estimate: $3/M input, $15/M output (Sonnet-class pricing)
    let input_cost = usage.prompt_tokens as f64 * 3.0 / 1_000_000.0;
    let output_cost = usage.completion_tokens as f64 * 15.0 / 1_000_000.0;
    input_cost + output_cost
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code::agent::AgentEvent;
    use a3s_code::llm::TokenUsage;

    #[test]
    fn test_translate_text_delta() {
        let event = AgentEvent::TextDelta {
            text: "Hello".to_string(),
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::StreamEvent { event, .. } => {
                assert_eq!(event["delta"]["text"], "Hello");
            }
            _ => panic!("Expected StreamEvent"),
        }
    }

    #[test]
    fn test_translate_tool_start() {
        let event = AgentEvent::ToolStart {
            id: "t1".to_string(),
            name: "Bash".to_string(),
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::StreamEvent { event, .. } => {
                assert_eq!(event["content_block"]["name"], "Bash");
                assert_eq!(event["content_block"]["id"], "t1");
            }
            _ => panic!("Expected StreamEvent"),
        }
    }

    #[test]
    fn test_translate_tool_end() {
        let event = AgentEvent::ToolEnd {
            id: "t1".to_string(),
            name: "Bash".to_string(),
            output: "ok".to_string(),
            exit_code: 0,
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            msgs[0],
            BrowserIncomingMessage::StreamEvent { .. }
        ));
    }

    #[test]
    fn test_translate_end() {
        let event = AgentEvent::End {
            text: "Done".to_string(),
            usage: TokenUsage::default(),
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 3);
        assert!(matches!(msgs[0], BrowserIncomingMessage::Assistant { .. }));
        assert!(matches!(msgs[1], BrowserIncomingMessage::Result { .. }));
        assert!(matches!(
            msgs[2],
            BrowserIncomingMessage::StatusChange { .. }
        ));
    }

    #[test]
    fn test_translate_error() {
        let event = AgentEvent::Error {
            message: "oops".to_string(),
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::Error { message } => assert_eq!(message, "oops"),
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_translate_confirmation_required() {
        let event = AgentEvent::ConfirmationRequired {
            tool_id: "t1".to_string(),
            tool_name: "Bash".to_string(),
            args: serde_json::json!({"command": "rm -rf /"}),
            timeout_ms: 30000,
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::PermissionRequest { request } => {
                assert_eq!(request.request_id, "t1");
                assert_eq!(request.tool_name, "Bash");
            }
            _ => panic!("Expected PermissionRequest"),
        }
    }

    #[test]
    fn test_translate_confirmation_received() {
        let event = AgentEvent::ConfirmationReceived {
            tool_id: "t1".to_string(),
            approved: true,
            reason: None,
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::PermissionCancelled { request_id } => {
                assert_eq!(request_id, "t1");
            }
            _ => panic!("Expected PermissionCancelled"),
        }
    }

    #[test]
    fn test_translate_permission_denied() {
        let event = AgentEvent::PermissionDenied {
            tool_id: "t1".to_string(),
            tool_name: "Bash".to_string(),
            args: serde_json::json!({}),
            reason: "not allowed".to_string(),
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::Error { message } => {
                assert!(message.contains("not allowed"));
            }
            _ => panic!("Expected Error"),
        }
    }

    #[test]
    fn test_translate_start() {
        let event = AgentEvent::Start {
            prompt: "hi".to_string(),
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        assert!(matches!(
            msgs[0],
            BrowserIncomingMessage::StatusChange { .. }
        ));
    }

    #[test]
    fn test_translate_turn_end() {
        let event = AgentEvent::TurnEnd {
            turn: 3,
            usage: TokenUsage {
                prompt_tokens: 1000,
                completion_tokens: 500,
                ..Default::default()
            },
        };
        let msgs = translate_event(&event);
        assert_eq!(msgs.len(), 1);
        match &msgs[0] {
            BrowserIncomingMessage::SessionUpdate { session } => {
                assert_eq!(session["num_turns"], 3);
            }
            _ => panic!("Expected SessionUpdate"),
        }
    }

    #[test]
    fn test_translate_internal_events_suppressed() {
        let events = vec![
            AgentEvent::TurnStart { turn: 1 },
            AgentEvent::ContextResolving { providers: vec![] },
            AgentEvent::ContextResolved {
                total_items: 0,
                total_tokens: 0,
            },
            AgentEvent::PlanningStart {
                prompt: "test".to_string(),
            },
        ];

        for event in events {
            let msgs = translate_event(&event);
            assert!(msgs.is_empty(), "Expected no messages for {:?}", event);
        }
    }

    #[test]
    fn test_permission_mode_to_policy() {
        let (perm, _confirm) = permission_mode_to_policies("strict");
        assert!(perm.enabled);

        let (perm, _confirm) = permission_mode_to_policies("yolo");
        assert!(perm.enabled);

        let (perm, _confirm) = permission_mode_to_policies("default");
        assert!(perm.enabled);
    }

    #[test]
    fn test_estimate_cost() {
        let usage = TokenUsage {
            prompt_tokens: 1_000_000,
            completion_tokens: 100_000,
            ..Default::default()
        };
        let cost = estimate_cost_from_usage(&usage);
        // 1M input * $3/M + 100K output * $15/M = $3 + $1.50 = $4.50
        assert!((cost - 4.5).abs() < 0.01);
    }
}
