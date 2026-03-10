//! Unified API router for SafeClaw
//!
//! Merges all module routers into a single axum `Router` with CORS,
//! consistent error handling, and a shared application state.

use crate::agent::{agent_router, AgentState};
use crate::config::{ChannelAgentConfig, ChannelAgentConfigStore};
use crate::runtime::Runtime;
use crate::workflows::{CreateWorkflow, UpdateWorkflow, WorkflowStore};
use a3s_code::skills::SkillRegistry;
use a3s_flow::{ExecutionState, FlowEngine, FlowError};
use a3s_memory::MemoryStore;
use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        Path, Query, State,
    },
    http::{header, Method, StatusCode},
    response::IntoResponse,
    routing::{delete, get, patch, post, put},
    Json, Router,
};
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tower_http::cors::{Any, CorsLayer};
use uuid::Uuid;

/// Shared state for memory endpoints
#[derive(Clone)]
pub struct MemoryState {
    pub store: Arc<dyn MemoryStore>,
}

/// Build the complete SafeClaw HTTP application
pub fn build_app(
    gateway: Arc<Runtime>,
    agent_state: AgentState,
    skill_registry: Arc<a3s_code::skills::SkillRegistry>,
    memory_store: Arc<dyn MemoryStore>,
    channel_config_store: ChannelAgentConfigStore,
    workflow_store: WorkflowStore,
    flow_engine: Arc<FlowEngine>,
    cors_origins: &[String],
) -> Router {
    let cors = build_cors(cors_origins);
    let gateway_routes = gateway_api_router(gateway.clone(), channel_config_store);
    let memory_routes = memory_api_router(MemoryState {
        store: memory_store,
    });

    Router::new()
        .route("/health", get(health_check))
        .merge(gateway_routes)
        .merge(memory_routes)
        .merge(agent_router(agent_state))
        .merge(fs_router())
        .merge(box_router(skill_registry))
        .merge(workflow_router(workflow_store))
        .merge(execution_router(flow_engine))
        .layer(cors)
}

// =============================================================================
// Gateway sub-router
// =============================================================================

fn gateway_api_router(
    gateway: Arc<Runtime>,
    channel_config_store: ChannelAgentConfigStore,
) -> Router {
    let channel_config_routes = Router::new()
        .route(
            "/api/v1/channels/:channel_id/agent-config",
            get(get_channel_agent_config).patch(update_channel_agent_config),
        )
        .route(
            "/api/v1/channel-agent-configs",
            get(list_channel_agent_configs),
        )
        .with_state(channel_config_store);

    Router::new()
        .route("/api/v1/gateway/status", get(gateway_status))
        .route("/api/v1/gateway/sessions", get(gateway_list_sessions))
        .route("/api/v1/gateway/sessions/:id", get(gateway_get_session))
        .route("/api/v1/gateway/message", post(gateway_send_message))
        .route("/api/v1/gateway/webhook/:channel", post(gateway_webhook))
        .route("/api/v1/tee/status", get(tee_status))
        .route("/api/v1/channels", get(list_channels))
        .route("/api/v1/users", get(list_users).post(create_user))
        .route("/api/v1/users/:id", patch(update_user).delete(delete_user))
        .with_state(gateway)
        .merge(channel_config_routes)
}

// =============================================================================
// Root handlers
// =============================================================================

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    version: String,
}

async fn health_check() -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

// =============================================================================
// Gateway handlers
// =============================================================================

async fn gateway_status(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
) -> impl IntoResponse {
    Json(gateway.status().await)
}

async fn gateway_list_sessions(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
) -> impl IntoResponse {
    let sessions = gateway.session_manager().active_sessions().await;
    let infos: Vec<serde_json::Value> = futures::future::join_all(sessions.iter().map(|s| async {
        serde_json::json!({
            "id": s.id,
            "userId": s.user_id,
            "channelId": s.channel_id,
            "chatId": s.chat_id,
            "usesTee": s.uses_tee().await,
            "createdAt": s.created_at,
            "messageCount": s.message_count().await,
        })
    }))
    .await;
    Json(infos)
}

async fn gateway_get_session(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match gateway.session_manager().get_session(&id).await {
        Some(session) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "id": session.id,
                "userId": session.user_id,
                "channelId": session.channel_id,
                "chatId": session.chat_id,
                "usesTee": session.uses_tee().await,
                "createdAt": session.created_at,
                "messageCount": session.message_count().await,
            })),
        ),
        None => (
            StatusCode::NOT_FOUND,
            Json(
                serde_json::json!({"error": {"code": "NOT_FOUND", "message": "Session not found"}}),
            ),
        ),
    }
}

#[derive(serde::Deserialize)]
struct SendMessageRequest {
    channel: String,
    chat_id: String,
    content: String,
}

async fn gateway_send_message(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
    Json(request): Json<SendMessageRequest>,
) -> impl IntoResponse {
    let channels = gateway.channels().read().await;
    match channels.get(&request.channel) {
        Some(channel) => {
            let outbound = crate::channels::OutboundMessage::new(
                &request.channel,
                &request.chat_id,
                &request.content,
            );
            match channel.send_message(outbound).await {
                Ok(message_id) => (
                    StatusCode::OK,
                    Json(serde_json::json!({"messageId": message_id, "status": "sent"})),
                ),
                Err(e) => (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(
                        serde_json::json!({"error": {"code": "SEND_FAILED", "message": e.to_string()}}),
                    ),
                ),
            }
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(
                serde_json::json!({"error": {"code": "NOT_FOUND", "message": format!("Channel '{}' not found", request.channel)}}),
            ),
        ),
    }
}

async fn gateway_webhook(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
    axum::extract::Path(channel): axum::extract::Path<String>,
    body: String,
) -> impl IntoResponse {
    tracing::info!(
        channel = %channel,
        body_len = body.len(),
        body_preview = %body.chars().take(200).collect::<String>(),
        "Webhook received"
    );

    let parse_result = match gateway.processor().parse_webhook(&channel, &body).await {
        Ok(result) => result,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(
                    serde_json::json!({"error": {"code": "WEBHOOK_ERROR", "message": e.to_string()}}),
                ),
            );
        }
    };

    use crate::runtime::processor::WebhookParseResult;
    match parse_result {
        WebhookParseResult::Challenge(response) => {
            if response.outbound.chat_id == "__challenge__" {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({"challenge": response.outbound.content})),
                );
            }
            (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
        }
        WebhookParseResult::Message(inbound) => {
            let gw = gateway.clone();
            let ch = channel.clone();
            tokio::spawn(async move {
                let channels = gw.channels().read().await;
                let adapter = match channels.get(&ch) {
                    Some(a) => a.clone(),
                    None => return,
                };
                drop(channels);

                if let Err(e) = gw
                    .processor()
                    .process_message_streaming(inbound, adapter)
                    .await
                {
                    tracing::error!(channel = %ch, "Streaming processing failed: {}", e);
                }
            });

            (
                StatusCode::OK,
                Json(serde_json::json!({"status": "accepted"})),
            )
        }
        WebhookParseResult::CardAction(action) => {
            let channels = gateway.channels().read().await;
            let adapter = match channels.get(&channel) {
                Some(a) => a.clone(),
                None => {
                    return (
                        StatusCode::OK,
                        Json(serde_json::json!({"status": "no_adapter"})),
                    );
                }
            };
            drop(channels);

            match gateway
                .processor()
                .handle_card_action(action, adapter)
                .await
            {
                Ok(_) => (StatusCode::OK, Json(serde_json::json!({}))),
                Err(e) => {
                    tracing::error!(channel = %channel, "Card action handling failed: {}", e);
                    (StatusCode::OK, Json(serde_json::json!({})))
                }
            }
        }
        WebhookParseResult::Ignored => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "ignored"})),
        ),
    }
}

// =============================================================================
// TEE / Channels handlers
// =============================================================================

async fn tee_status(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
) -> impl IntoResponse {
    let level = gateway.security_level();
    Json(serde_json::json!({
        "securityLevel": level,
        "description": level.description(),
        "attestationAvailable": level == crate::tee::SecurityLevel::TeeHardware,
        "sealedStorageAvailable": level == crate::tee::SecurityLevel::TeeHardware,
    }))
}

async fn list_channels(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
) -> impl IntoResponse {
    let channels = gateway.channels().read().await;
    let config = gateway.config();

    let mut result = Vec::new();
    for (name, adapter) in channels.iter() {
        result.push(serde_json::json!({
            "id": name,
            "name": name,
            "connected": adapter.is_connected(),
            "dmPolicy": match name.as_str() {
                "feishu" => config.channels.feishu.as_ref().map(|c| c.dm_policy.as_str()).unwrap_or("open"),
                "discord" => config.channels.discord.as_ref().map(|c| c.dm_policy.as_str()).unwrap_or("open"),
                "telegram" => config.channels.telegram.as_ref().map(|c| c.dm_policy.as_str()).unwrap_or("open"),
                "slack" => config.channels.slack.as_ref().map(|c| c.dm_policy.as_str()).unwrap_or("open"),
                "dingtalk" => config.channels.dingtalk.as_ref().map(|c| c.dm_policy.as_str()).unwrap_or("open"),
                "wecom" => config.channels.wecom.as_ref().map(|c| c.dm_policy.as_str()).unwrap_or("open"),
                _ => "open",
            },
        }));
    }
    Json(result)
}

async fn update_channel_agent_config(
    State(store): State<ChannelAgentConfigStore>,
    Path(channel_id): Path<String>,
    Json(body): Json<ChannelAgentConfig>,
) -> impl IntoResponse {
    tracing::info!(channel = %channel_id, "Updating channel agent config");
    match store.set(&channel_id, body).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "saved", "channelId": channel_id})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(
                serde_json::json!({"status": "error", "message": format!("Failed to persist config: {}", e)}),
            ),
        ),
    }
}

async fn get_channel_agent_config(
    State(store): State<ChannelAgentConfigStore>,
    Path(channel_id): Path<String>,
) -> impl IntoResponse {
    let config = store.get(&channel_id).await.unwrap_or_default();
    Json(serde_json::json!({"channelId": channel_id, "config": config}))
}

async fn list_channel_agent_configs(
    State(store): State<ChannelAgentConfigStore>,
) -> impl IntoResponse {
    let configs = store.get_all().await;
    Json(serde_json::json!({ "configs": configs }))
}

// =============================================================================
// Memory endpoints
// =============================================================================

fn layer_to_filter(layer: &str) -> LayerFilter {
    match layer {
        "resources" => LayerFilter::Types(vec![a3s_memory::MemoryType::Episodic]),
        "artifacts" => LayerFilter::Types(vec![
            a3s_memory::MemoryType::Semantic,
            a3s_memory::MemoryType::Procedural,
        ]),
        "insights" => LayerFilter::Tag("insight".to_string()),
        _ => LayerFilter::Types(vec![]),
    }
}

enum LayerFilter {
    Types(Vec<a3s_memory::MemoryType>),
    Tag(String),
}

fn memory_api_router(state: MemoryState) -> Router {
    Router::new()
        .route("/api/v1/memory/stats", get(memory_stats))
        .route("/api/v1/memory/export", get(memory_export))
        .route("/api/v1/memory/batch", delete(memory_batch_delete))
        .route("/api/v1/memory/:layer", get(list_memory_layer))
        .route("/api/v1/memory/:singular/:id", delete(delete_memory_entry))
        .with_state(state)
}

#[derive(Debug, Deserialize)]
struct MemoryQuery {
    #[serde(default)]
    search: Option<String>,
    #[serde(default)]
    limit: Option<usize>,
}

fn memory_item_to_json(item: &a3s_memory::MemoryItem) -> serde_json::Value {
    let now = chrono::Utc::now();
    let age_days = (now - item.timestamp).num_days();
    let decay_days = (90 - age_days).max(0);

    let taint_labels: Vec<&str> = item
        .tags
        .iter()
        .filter(|t| t.starts_with("taint:"))
        .map(|t| t.as_str())
        .collect();

    let layer = match item.memory_type {
        a3s_memory::MemoryType::Episodic => "resources",
        a3s_memory::MemoryType::Semantic | a3s_memory::MemoryType::Procedural => "artifacts",
        a3s_memory::MemoryType::Working => "resources",
    };

    serde_json::json!({
        "id": item.id,
        "layer": layer,
        "title": item.metadata.get("title").cloned()
            .unwrap_or_else(|| item.content.chars().take(60).collect::<String>()),
        "content": item.content,
        "taintLabels": taint_labels,
        "tags": item.tags,
        "importance": item.importance,
        "memoryType": format!("{:?}", item.memory_type),
        "createdAt": item.timestamp.timestamp_millis(),
        "lastAccessedAt": item.last_accessed
            .map(|t| t.timestamp_millis())
            .unwrap_or_else(|| item.timestamp.timestamp_millis()),
        "decayDays": decay_days,
        "sessionId": item.metadata.get("session_id").cloned(),
    })
}

async fn list_memory_layer(
    State(state): State<MemoryState>,
    Path(layer): Path<String>,
    Query(params): Query<MemoryQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);

    let items = if let Some(ref query) = params.search {
        match state.store.search(query, limit * 3).await {
            Ok(items) => items,
            Err(e) => {
                tracing::warn!("Memory search failed: {}", e);
                return Json(serde_json::json!({"layer": layer, "entries": [], "total": 0}));
            }
        }
    } else {
        let result = match layer_to_filter(&layer) {
            LayerFilter::Types(types) => state.store.get_recent(limit * 3).await.map(|items| {
                items
                    .into_iter()
                    .filter(|i| types.contains(&i.memory_type))
                    .collect::<Vec<_>>()
            }),
            LayerFilter::Tag(tag) => state.store.search_by_tags(&[tag], limit).await,
        };
        match result {
            Ok(items) => items,
            Err(e) => {
                tracing::warn!("Memory list failed: {}", e);
                return Json(serde_json::json!({"layer": layer, "entries": [], "total": 0}));
            }
        }
    };

    let filtered: Vec<_> = if params.search.is_some() {
        let filter = layer_to_filter(&layer);
        items
            .into_iter()
            .filter(|item| match &filter {
                LayerFilter::Types(types) => types.contains(&item.memory_type),
                LayerFilter::Tag(tag) => item.tags.contains(tag),
            })
            .take(limit)
            .collect()
    } else {
        items.into_iter().take(limit).collect()
    };

    let total = filtered.len();
    let entries: Vec<_> = filtered.iter().map(memory_item_to_json).collect();

    Json(serde_json::json!({"layer": layer, "entries": entries, "total": total}))
}

async fn delete_memory_entry(
    State(state): State<MemoryState>,
    Path((_singular, id)): Path<(String, String)>,
) -> impl IntoResponse {
    match state.store.delete(&id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({"status": "deleted", "id": id})),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"status": "error", "message": e.to_string()})),
        ),
    }
}

async fn memory_batch_delete(
    State(state): State<MemoryState>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let ids: Vec<String> = body
        .get("ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .unwrap_or_default();
    if ids.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"status": "error", "message": "No ids provided"})),
        );
    }
    let mut deleted = 0u32;
    let mut errors = 0u32;
    for id in &ids {
        match state.store.delete(id).await {
            Ok(()) => deleted += 1,
            Err(_) => errors += 1,
        }
    }
    (
        StatusCode::OK,
        Json(serde_json::json!({"status": "ok", "deleted": deleted, "errors": errors})),
    )
}

async fn memory_stats(State(state): State<MemoryState>) -> impl IntoResponse {
    let all_items = state.store.get_recent(10000).await.unwrap_or_default();
    let now = chrono::Utc::now();

    let mut resources = 0u32;
    let mut artifacts = 0u32;
    let mut insights = 0u32;
    let mut tainted = 0u32;
    let mut expiring_7 = 0u32;
    let mut expiring_30 = 0u32;

    for item in &all_items {
        let age_days = (now - item.timestamp).num_days();
        let decay_days = (90 - age_days).max(0);
        let is_insight = item.tags.iter().any(|t| t == "insight");
        if item.tags.iter().any(|t| t.starts_with("taint:")) {
            tainted += 1;
        }
        if decay_days <= 7 {
            expiring_7 += 1;
        } else if decay_days <= 30 {
            expiring_30 += 1;
        }

        if is_insight {
            insights += 1;
        } else {
            match item.memory_type {
                a3s_memory::MemoryType::Episodic | a3s_memory::MemoryType::Working => {
                    resources += 1
                }
                a3s_memory::MemoryType::Semantic | a3s_memory::MemoryType::Procedural => {
                    artifacts += 1
                }
            }
        }
    }

    Json(serde_json::json!({
        "layers": {"resources": resources, "artifacts": artifacts, "insights": insights},
        "expiringIn7Days": expiring_7,
        "expiringIn30Days": expiring_30,
        "tainted": tainted,
        "total": all_items.len(),
    }))
}

#[derive(Debug, Deserialize)]
struct ExportQuery {
    #[serde(default)]
    layer: Option<String>,
    #[serde(default = "default_export_format")]
    format: String,
}

fn default_export_format() -> String {
    "json".to_string()
}

async fn memory_export(
    State(state): State<MemoryState>,
    Query(params): Query<ExportQuery>,
) -> impl IntoResponse {
    let all_items = state.store.get_recent(10000).await.unwrap_or_default();
    let items: Vec<_> = if let Some(ref layer) = params.layer {
        let filter = layer_to_filter(layer);
        all_items
            .into_iter()
            .filter(|item| match &filter {
                LayerFilter::Types(types) => types.contains(&item.memory_type),
                LayerFilter::Tag(tag) => item.tags.contains(tag),
            })
            .collect()
    } else {
        all_items
    };

    if params.format == "csv" {
        let mut csv = String::from("id,title,content,tags,importance,memoryType,createdAt\n");
        for item in &items {
            let title = item
                .metadata
                .get("title")
                .cloned()
                .unwrap_or_else(|| item.content.chars().take(60).collect());
            let escape = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            csv.push_str(&format!(
                "{},{},{},{},{},{:?},{}\n",
                item.id,
                escape(&title),
                escape(&item.content),
                escape(&item.tags.join(";")),
                item.importance,
                item.memory_type,
                item.timestamp.to_rfc3339()
            ));
        }
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "text/csv; charset=utf-8")],
            csv,
        )
            .into_response()
    } else {
        let entries: Vec<_> = items.iter().map(memory_item_to_json).collect();
        let json = serde_json::to_string_pretty(&serde_json::json!({
            "exported_at": chrono::Utc::now().to_rfc3339(),
            "count": entries.len(),
            "layer": params.layer,
            "entries": entries,
        }))
        .unwrap_or_default();
        (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/json; charset=utf-8")],
            json,
        )
            .into_response()
    }
}

// =============================================================================
// User management stubs
// =============================================================================

async fn list_users() -> impl IntoResponse {
    Json(
        serde_json::json!({"users": [], "total": 0, "message": "User management available in Phase 20"}),
    )
}

async fn create_user(Json(_body): Json<serde_json::Value>) -> impl IntoResponse {
    (
        StatusCode::CREATED,
        Json(
            serde_json::json!({"status": "accepted", "message": "User management available in Phase 20"}),
        ),
    )
}

async fn update_user(
    Path(id): Path<String>,
    Json(_body): Json<serde_json::Value>,
) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(
            serde_json::json!({"status": "accepted", "userId": id, "message": "User management available in Phase 20"}),
        ),
    )
}

async fn delete_user(Path(id): Path<String>) -> impl IntoResponse {
    (
        StatusCode::OK,
        Json(
            serde_json::json!({"status": "accepted", "userId": id, "message": "User management available in Phase 20"}),
        ),
    )
}

// =============================================================================
// CORS
// =============================================================================

fn build_cors(origins: &[String]) -> CorsLayer {
    let cors = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([header::CONTENT_TYPE, header::AUTHORIZATION, header::ACCEPT]);

    if origins.is_empty() {
        cors.allow_origin(Any)
    } else {
        let parsed: Vec<_> = origins.iter().filter_map(|o| o.parse().ok()).collect();
        cors.allow_origin(parsed)
    }
}

// =============================================================================
// Filesystem API
// =============================================================================

fn fs_router() -> Router {
    Router::new()
        .route("/api/fs/tree", get(fs_tree))
        .route("/api/fs/file", get(fs_read_file).put(fs_write_file))
        .route("/api/fs/create", post(fs_create))
        .route("/api/fs/rename", post(fs_rename))
        .route("/api/fs/delete", delete(fs_delete))
        .route("/ws/fs/watch", get(fs_watch_ws))
        .route("/api/git/worktrees", get(git_worktrees))
        .route("/api/git/checkout", post(git_checkout))
}

#[derive(Debug, Deserialize)]
struct GitRepoQuery {
    path: String,
}

#[derive(Debug, Serialize)]
struct GitWorktree {
    path: String,
    branch: String,
    is_current: bool,
}

async fn git_worktrees(Query(q): Query<GitRepoQuery>) -> impl IntoResponse {
    let output = std::process::Command::new("git")
        .args(["worktree", "list", "--porcelain"])
        .current_dir(&q.path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            let mut worktrees: Vec<GitWorktree> = Vec::new();
            let mut current_path = String::new();
            let mut current_branch = String::new();
            let mut is_bare = false;

            for line in stdout.lines() {
                if let Some(p) = line.strip_prefix("worktree ") {
                    current_path = p.to_string();
                    current_branch = String::new();
                    is_bare = false;
                } else if let Some(b) = line.strip_prefix("branch refs/heads/") {
                    current_branch = b.to_string();
                } else if line == "bare" {
                    is_bare = true;
                } else if line.is_empty() && !current_path.is_empty() && !is_bare {
                    let is_current = current_path == q.path
                        || std::fs::canonicalize(&current_path)
                            .ok()
                            .zip(std::fs::canonicalize(&q.path).ok())
                            .map(|(a, b)| a == b)
                            .unwrap_or(false);
                    worktrees.push(GitWorktree {
                        path: current_path.clone(),
                        branch: if current_branch.is_empty() {
                            "HEAD".to_string()
                        } else {
                            current_branch.clone()
                        },
                        is_current,
                    });
                    current_path.clear();
                }
            }
            // flush last entry
            if !current_path.is_empty() && !is_bare {
                let is_current = current_path == q.path
                    || std::fs::canonicalize(&current_path)
                        .ok()
                        .zip(std::fs::canonicalize(&q.path).ok())
                        .map(|(a, b)| a == b)
                        .unwrap_or(false);
                worktrees.push(GitWorktree {
                    path: current_path,
                    branch: if current_branch.is_empty() {
                        "HEAD".to_string()
                    } else {
                        current_branch
                    },
                    is_current,
                });
            }
            (
                StatusCode::OK,
                Json(serde_json::to_value(worktrees).unwrap_or_default()),
            )
                .into_response()
        }
        _ => (StatusCode::OK, Json(serde_json::json!([]))).into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct GitCheckoutBody {
    path: String,
    branch: String,
}

async fn git_checkout(Json(body): Json<GitCheckoutBody>) -> impl IntoResponse {
    let output = std::process::Command::new("git")
        .args(["checkout", &body.branch])
        .current_dir(&body.path)
        .output();

    match output {
        Ok(out) if out.status.success() => {
            (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response()
        }
        Ok(out) => (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": String::from_utf8_lossy(&out.stderr).to_string()})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

fn default_depth() -> usize {
    3
}

#[derive(Debug, Deserialize)]
struct FsPathQuery {
    path: String,
    #[serde(default = "default_depth")]
    depth: usize,
}

#[derive(Debug, Serialize)]
struct FsNode {
    name: String,
    path: String,
    is_dir: bool,
    children: Option<Vec<FsNode>>,
}

async fn fs_tree(Query(q): Query<FsPathQuery>) -> impl IntoResponse {
    let root = std::path::Path::new(&q.path);
    if !root.exists() || !root.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Path does not exist or is not a directory"})),
        )
            .into_response();
    }
    match build_tree(root, q.depth) {
        Ok(node) => (
            StatusCode::OK,
            Json(serde_json::to_value(node).unwrap_or_default()),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

fn build_tree(path: &std::path::Path, depth: usize) -> std::io::Result<FsNode> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned());
    let is_dir = path.is_dir();

    let children = if is_dir && depth > 0 {
        let mut entries: Vec<FsNode> = std::fs::read_dir(path)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                // Skip hidden files/dirs
                !e.file_name().to_string_lossy().starts_with('.')
            })
            .filter_map(|e| build_tree(&e.path(), depth - 1).ok())
            .collect();
        // Dirs first, then files, both alphabetically
        entries.sort_by(|a, b| {
            b.is_dir
                .cmp(&a.is_dir)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        Some(entries)
    } else if is_dir {
        Some(vec![])
    } else {
        None
    };

    Ok(FsNode {
        name,
        path: path.to_string_lossy().into_owned(),
        is_dir,
        children,
    })
}

#[derive(Debug, Deserialize)]
struct FsFileQuery {
    path: String,
    #[serde(default)]
    raw: u8,
}

async fn fs_read_file(Query(q): Query<FsFileQuery>) -> impl IntoResponse {
    let path = std::path::Path::new(&q.path);
    if !path.exists() || path.is_dir() {
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "File not found"})),
        )
            .into_response();
    }

    if q.raw == 1 {
        // Return raw bytes with guessed mime type
        let mime = mime_guess::from_path(path).first_or_octet_stream();
        match std::fs::read(path) {
            Ok(bytes) => (
                StatusCode::OK,
                [(axum::http::header::CONTENT_TYPE, mime.as_ref().to_string())],
                bytes,
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response(),
        }
    } else {
        match std::fs::read_to_string(path) {
            Ok(content) => (
                StatusCode::OK,
                Json(serde_json::json!({"content": content, "path": q.path})),
            )
                .into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct FsWriteBody {
    path: String,
    content: String,
}

async fn fs_write_file(Json(body): Json<FsWriteBody>) -> impl IntoResponse {
    match std::fs::write(&body.path, &body.content) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct FsCreateBody {
    path: String,
    is_dir: bool,
}

async fn fs_create(Json(body): Json<FsCreateBody>) -> impl IntoResponse {
    let result = if body.is_dir {
        std::fs::create_dir_all(&body.path)
    } else {
        // Create parent dirs if needed, then create empty file
        if let Some(parent) = std::path::Path::new(&body.path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        std::fs::File::create(&body.path).map(|_| ())
    };
    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct FsRenameBody {
    from: String,
    to: String,
}

async fn fs_rename(Json(body): Json<FsRenameBody>) -> impl IntoResponse {
    if let Some(parent) = std::path::Path::new(&body.to).parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    match std::fs::rename(&body.from, &body.to) {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct FsDeleteQuery {
    path: String,
}

async fn fs_delete(Query(q): Query<FsDeleteQuery>) -> impl IntoResponse {
    let path = std::path::Path::new(&q.path);
    let result = if path.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    };
    match result {
        Ok(()) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn fs_watch_ws(ws: WebSocketUpgrade, Query(q): Query<FsPathQuery>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_fs_watch(socket, q.path))
}

async fn handle_fs_watch(mut socket: WebSocket, path: String) {
    use notify::{Config, Event, RecommendedWatcher, RecursiveMode, Watcher};

    let (tx, mut rx) = tokio::sync::mpsc::channel::<notify::Result<Event>>(64);
    let handle = tokio::runtime::Handle::current();

    let mut watcher = match RecommendedWatcher::new(
        move |res| {
            let tx = tx.clone();
            handle.spawn(async move {
                let _ = tx.send(res).await;
            });
        },
        Config::default(),
    ) {
        Ok(w) => w,
        Err(e) => {
            let _ = socket
                .send(Message::Text(
                    serde_json::json!({"error": e.to_string()}).to_string(),
                ))
                .await;
            return;
        }
    };

    if let Err(e) = watcher.watch(std::path::Path::new(&path), RecursiveMode::Recursive) {
        let _ = socket
            .send(Message::Text(
                serde_json::json!({"error": e.to_string()}).to_string(),
            ))
            .await;
        return;
    }

    loop {
        tokio::select! {
            Some(event) = rx.recv() => {
                if let Ok(ev) = event {
                    // Only notify on create/remove/rename — skip modify (content changes)
                    let kind = format!("{:?}", ev.kind);
                    let is_structural = matches!(
                        ev.kind,
                        notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                            | notify::EventKind::Modify(notify::event::ModifyKind::Name(_))
                    );
                    if is_structural {
                        let msg = serde_json::json!({
                            "type": "change",
                            "kind": kind,
                            "paths": ev.paths.iter().map(|p| p.to_string_lossy()).collect::<Vec<_>>(),
                        });
                        if socket.send(Message::Text(msg.to_string())).await.is_err() {
                            break;
                        }
                    }
                }
            }
            Some(msg) = socket.next() => {
                // Client closed connection
                if matches!(msg, Ok(Message::Close(_)) | Err(_)) {
                    break;
                }
            }
        }
    }
}

// =============================================================================
// A3S Box API — delegates to `a3s box` CLI
// =============================================================================

/// Synchronously check whether `a3s-box` is reachable on this system.
/// Used at startup by bootstrap to decide whether to register the box skill.
pub fn box_is_installed() -> bool {
    std::process::Command::new(find_a3s_box())
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Shared state for box API handlers that need to inject skills after install.
#[derive(Clone)]
struct BoxState {
    skill_registry: Arc<SkillRegistry>,
}

fn box_router(skill_registry: Arc<SkillRegistry>) -> Router {
    let state = BoxState { skill_registry };
    Router::new()
        .route("/api/v1/box/check", get(box_check))
        .route("/api/v1/box/install", post(box_install))
        .route("/api/v1/box/containers", get(box_list_containers))
        .route("/api/v1/box/containers/:id", get(box_inspect_container))
        .route("/api/v1/box/containers/:id/start", post(box_start))
        .route("/api/v1/box/containers/:id/stop", post(box_stop))
        .route("/api/v1/box/containers/:id/restart", post(box_restart))
        .route("/api/v1/box/containers/:id/pause", post(box_pause))
        .route("/api/v1/box/containers/:id/unpause", post(box_unpause))
        .route("/api/v1/box/containers/:id", delete(box_remove))
        .route("/api/v1/box/stats", get(box_stats))
        .route("/api/v1/box/images", get(box_list_images))
        .route("/api/v1/box/images/pull", post(box_pull_image))
        .route("/api/v1/box/images/prune", post(box_prune_images))
        .route("/api/v1/box/images/remove", delete(box_remove_image))
        .route(
            "/api/v1/box/networks",
            get(box_list_networks).post(box_create_network),
        )
        .route("/api/v1/box/networks/:id", delete(box_remove_network))
        .route(
            "/api/v1/box/volumes",
            get(box_list_volumes).post(box_create_volume),
        )
        .route("/api/v1/box/volumes/:name", delete(box_remove_volume))
        .route("/api/v1/box/volumes/prune", post(box_prune_volumes))
        .route("/api/v1/box/snapshots", get(box_list_snapshots))
        .route("/api/v1/box/snapshots/:id", delete(box_remove_snapshot))
        .route(
            "/api/v1/box/snapshots/:id/restore",
            post(box_restore_snapshot),
        )
        .route("/api/v1/box/info", get(box_info))
        .route("/api/v1/box/df", get(box_df))
        .route("/api/v1/box/system/prune", post(box_system_prune))
        .with_state(state)
}

/// Resolve the `a3s-box` binary path.
///
/// Checks `A3S_BOX_BIN` env override first, then common install locations
/// (Homebrew paths and `~/.local/bin`), then falls back to a bare PATH lookup.
fn find_a3s_box() -> String {
    if let Ok(bin) = std::env::var("A3S_BOX_BIN") {
        return bin;
    }
    let mut candidates: Vec<String> = vec![
        "/opt/homebrew/bin/a3s-box".into(),
        "/usr/local/bin/a3s-box".into(),
    ];
    if let Some(home) = dirs::home_dir() {
        candidates.push(
            home.join(".local/bin/a3s-box")
                .to_string_lossy()
                .into_owned(),
        );
    }
    for candidate in &candidates {
        if std::path::Path::new(candidate.as_str()).exists() {
            return candidate.clone();
        }
    }
    "a3s-box".to_string()
}

/// Run `a3s box <args>` and return stdout, or an error response.
async fn run_box(args: &[&str]) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    let program = find_a3s_box();
    let out = tokio::process::Command::new(&program)
        .args(args)
        .output()
        .await
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to run a3s-box: {e}")})),
            )
        })?;
    if !out.status.success() {
        let stderr = String::from_utf8_lossy(&out.stderr).to_string();
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": stderr})),
        ));
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

#[derive(Debug, Deserialize)]
struct BoxAllQuery {
    #[serde(default = "default_true")]
    all: bool,
}

fn default_true() -> bool {
    true
}

async fn box_list_containers(Query(q): Query<BoxAllQuery>) -> impl IntoResponse {
    // Get IDs via ps, then inspect each for full JSON
    let ps_args: &[&str] = if q.all {
        &["ps", "-a", "-q"]
    } else {
        &["ps", "-q"]
    };
    let ids_out = match run_box(ps_args).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };
    let ids: Vec<&str> = ids_out
        .lines()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .collect();

    let mut containers = Vec::new();
    for id in ids {
        if let Ok(json_str) = run_box(&["inspect", id]).await {
            if let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) {
                containers.push(inspect_to_box_info(v));
            }
        }
    }
    Json(containers).into_response()
}

async fn box_inspect_container(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["inspect", &id]).await {
        Ok(s) => match serde_json::from_str::<serde_json::Value>(&s) {
            Ok(v) => (StatusCode::OK, Json(inspect_to_box_info(v))).into_response(),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": e.to_string()})),
            )
                .into_response(),
        },
        Err(e) => e.into_response(),
    }
}

/// Convert `a3s box inspect` JSON to the frontend BoxInfo shape.
fn inspect_to_box_info(v: serde_json::Value) -> serde_json::Value {
    let status = v["status"].as_str().unwrap_or("unknown");
    let created_at = v["created_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis())
        .unwrap_or(0);
    let started_at = v["started_at"]
        .as_str()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.timestamp_millis());

    let ports: Vec<String> = v["port_map"]
        .as_array()
        .map(|arr| {
            arr.iter()
                .filter_map(|p| {
                    let host_port = p["host_port"].as_u64()?;
                    let container_port = p["container_port"].as_u64()?;
                    let proto = p["protocol"].as_str().unwrap_or("tcp");
                    Some(format!("0.0.0.0:{host_port}->{container_port}/{proto}"))
                })
                .collect()
        })
        .unwrap_or_default();

    let networks: Vec<String> = if let Some(n) = v["network_name"].as_str() {
        vec![n.to_string()]
    } else {
        vec![v["network_mode"].as_str().unwrap_or("tsi").to_string()]
    };

    serde_json::json!({
        "id": v["short_id"].as_str().unwrap_or(v["id"].as_str().unwrap_or("")),
        "name": v["name"].as_str().unwrap_or(""),
        "image": v["image"].as_str().unwrap_or(""),
        "status": status,
        "cpus": v["cpus"].as_u64().unwrap_or(1),
        "memory": format!("{}MB", v["memory_mb"].as_u64().unwrap_or(512)),
        "created_at": created_at,
        "started_at": started_at,
        "ports": ports,
        "labels": v["labels"].clone(),
        "networks": networks,
        "tee": false,
        "restart_policy": v["restart_policy"].as_str().unwrap_or("no"),
        "health_status": v["health_status"].as_str().unwrap_or("none"),
    })
}

async fn box_start(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["start", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_stop(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["stop", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_restart(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["restart", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_pause(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["pause", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_unpause(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["unpause", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct ForceQuery {
    #[serde(default)]
    force: bool,
}

async fn box_remove(Path(id): Path<String>, Query(q): Query<ForceQuery>) -> impl IntoResponse {
    let args: Vec<&str> = if q.force {
        vec!["rm", "-f", &id]
    } else {
        vec!["rm", &id]
    };
    match run_box(&args).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_stats() -> impl IntoResponse {
    // `a3s box stats` is not available without running containers; return empty for now
    Json(serde_json::json!([])).into_response()
}

async fn box_list_images() -> impl IntoResponse {
    let out = match run_box(&[
        "images",
        "--format",
        "{{.Reference}}\t{{.Size}}\t{{.Pulled}}",
    ])
    .await
    {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    let images: Vec<serde_json::Value> = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            let reference = parts.first().copied().unwrap_or("");
            let (repo, tag) = reference.rsplit_once(':').unwrap_or((reference, "latest"));
            let size_str = parts.get(1).copied().unwrap_or("0");
            let size = parse_size(size_str);
            let pulled = parts.get(2).copied().unwrap_or("");
            let created_at = chrono::DateTime::parse_from_rfc3339(pulled)
                .map(|dt| dt.timestamp_millis())
                .unwrap_or(0);
            serde_json::json!({
                "id": format!("sha256:{}", &uuid::Uuid::new_v4().to_string().replace('-', "")[..12]),
                "repository": repo,
                "tag": tag,
                "size": size,
                "created_at": created_at,
                "digest": reference,
            })
        })
        .collect();

    Json(images).into_response()
}

fn parse_size(s: &str) -> u64 {
    let s = s.trim();
    if let Some(n) = s.strip_suffix(" GB") {
        return (n.trim().parse::<f64>().unwrap_or(0.0) * 1_073_741_824.0) as u64;
    }
    if let Some(n) = s.strip_suffix(" MB") {
        return (n.trim().parse::<f64>().unwrap_or(0.0) * 1_048_576.0) as u64;
    }
    if let Some(n) = s.strip_suffix(" KB") {
        return (n.trim().parse::<f64>().unwrap_or(0.0) * 1024.0) as u64;
    }
    s.parse::<u64>().unwrap_or(0)
}

#[derive(Debug, Deserialize)]
struct PullImageBody {
    image: String,
}

async fn box_pull_image(Json(body): Json<PullImageBody>) -> impl IntoResponse {
    match run_box(&["pull", &body.image]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_prune_images() -> impl IntoResponse {
    match run_box(&["image-prune"]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"reclaimed": 0}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_remove_image(Query(q): Query<RemoveImageQuery>) -> impl IntoResponse {
    let reference = match q.reference {
        Some(ref r) if !r.is_empty() => r.clone(),
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "missing reference"})),
            )
                .into_response()
        }
    };
    let args: Vec<&str> = if q.force {
        vec!["rmi", "-f", &reference]
    } else {
        vec!["rmi", &reference]
    };
    match run_box(&args).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

#[derive(Debug, Deserialize)]
struct RemoveImageQuery {
    reference: Option<String>,
    #[serde(default)]
    force: bool,
}

async fn box_list_networks() -> impl IntoResponse {
    // a3s box doesn't have a network list command yet; return empty
    Json(serde_json::json!([])).into_response()
}

#[derive(Debug, Deserialize)]
struct CreateNetworkBody {
    name: String,
    #[serde(default)]
    driver: Option<String>,
}

async fn box_create_network(Json(body): Json<CreateNetworkBody>) -> impl IntoResponse {
    let driver = body.driver.as_deref().unwrap_or("bridge");
    match run_box(&["network", "create", "--driver", driver, &body.name]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_remove_network(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["network", "rm", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_list_volumes() -> impl IntoResponse {
    let out = match run_box(&[
        "volume",
        "ls",
        "--format",
        "{{.Name}}\t{{.Driver}}\t{{.Mountpoint}}",
    ])
    .await
    {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!([])).into_response(),
    };

    let volumes: Vec<serde_json::Value> = out
        .lines()
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.splitn(3, '\t').collect();
            serde_json::json!({
                "name": parts.first().copied().unwrap_or(""),
                "driver": parts.get(1).copied().unwrap_or("local"),
                "mountpoint": parts.get(2).copied().unwrap_or(""),
                "size": 0,
                "created_at": 0,
                "labels": {},
            })
        })
        .collect();

    Json(volumes).into_response()
}

#[derive(Debug, Deserialize)]
struct CreateVolumeBody {
    name: String,
    #[serde(default)]
    driver: Option<String>,
}

async fn box_create_volume(Json(body): Json<CreateVolumeBody>) -> impl IntoResponse {
    let mut args = vec!["volume", "create"];
    let driver_str;
    if let Some(ref d) = body.driver {
        driver_str = d.clone();
        args.extend_from_slice(&["--driver", &driver_str]);
    }
    args.push(&body.name);
    match run_box(&args).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_remove_volume(Path(name): Path<String>) -> impl IntoResponse {
    match run_box(&["volume", "rm", &name]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_prune_volumes() -> impl IntoResponse {
    match run_box(&["volume", "prune"]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"reclaimed": 0}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_list_snapshots() -> impl IntoResponse {
    let out = match run_box(&["snapshot", "ls"]).await {
        Ok(s) => s,
        Err(_) => return Json(serde_json::json!([])).into_response(),
    };

    // Parse tabular output: SNAPSHOT ID  BOX  DESCRIPTION  SIZE  CREATED
    let snapshots: Vec<serde_json::Value> = out
        .lines()
        .skip(1) // header
        .filter(|l| !l.trim().is_empty())
        .map(|line| {
            let parts: Vec<&str> = line.split_whitespace().collect();
            serde_json::json!({
                "id": parts.first().copied().unwrap_or(""),
                "box_id": parts.get(1).copied().unwrap_or(""),
                "box_name": parts.get(1).copied().unwrap_or(""),
                "description": parts.get(2).copied().unwrap_or(""),
                "size": 0,
                "created_at": 0,
            })
        })
        .collect();

    Json(snapshots).into_response()
}

async fn box_remove_snapshot(Path(id): Path<String>) -> impl IntoResponse {
    match run_box(&["snapshot", "rm", &id]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_restore_snapshot(Path(id): Path<String>) -> impl IntoResponse {
    // snapshot restore requires <box> <snapshot> — id format: "<box_id>/<snapshot_id>"
    let parts: Vec<&str> = id.splitn(2, '/').collect();
    let args = if parts.len() == 2 {
        vec!["snapshot", "restore", parts[0], parts[1]]
    } else {
        vec!["snapshot", "restore", &id]
    };
    match run_box(&args).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"ok": true}))).into_response(),
        Err(e) => e.into_response(),
    }
}

async fn box_info() -> impl IntoResponse {
    let out = match run_box(&["info"]).await {
        Ok(s) => s,
        Err(e) => return e.into_response(),
    };

    // Parse text output from `a3s box info`
    let mut version = String::new();
    let os = std::env::consts::OS.to_string();
    let arch = std::env::consts::ARCH.to_string();
    let mut boxes_running = 0u64;
    let mut boxes_stopped = 0u64;
    let mut images_count = 0u64;
    let mut tee_available = false;
    let mut tee_backend = String::new();

    for line in out.lines() {
        let line = line.trim();
        if let Some(v) = line.strip_prefix("a3s-box version ") {
            version = v.trim().to_string();
        } else if line.starts_with("Virtualization:") {
            tee_backend = line.replace("Virtualization:", "").trim().to_string();
            tee_available = line.contains("SEV") || line.contains("TDX");
        } else if let Some(v) = line.strip_prefix("Boxes:") {
            // "1 total, 0 running"
            for part in v.split(',') {
                let part = part.trim();
                if let Some(n) = part.strip_suffix(" total") {
                    let total: u64 = n.trim().parse().unwrap_or(0);
                    boxes_stopped = total;
                } else if let Some(n) = part.strip_suffix(" running") {
                    boxes_running = n.trim().parse().unwrap_or(0);
                    boxes_stopped = boxes_stopped.saturating_sub(boxes_running);
                }
            }
        } else if let Some(v) = line.strip_prefix("Images:") {
            images_count = v
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
        }
    }

    let cpus = std::thread::available_parallelism()
        .map(|n| n.get() as u64)
        .unwrap_or(1);
    let memory_total = 0u64; // not available without sysinfo

    Json(serde_json::json!({
        "version": version,
        "os": os,
        "arch": arch,
        "cpus": cpus,
        "memory_total": memory_total,
        "boxes_running": boxes_running,
        "boxes_stopped": boxes_stopped,
        "images_count": images_count,
        "tee_available": tee_available,
        "tee_backend": tee_backend,
    }))
    .into_response()
}

async fn box_df() -> impl IntoResponse {
    match run_box(&["df"]).await {
        Ok(out) => {
            // Parse tabular output — best effort
            let mut images_size = 0u64;
            let mut containers_size = 0u64;
            let mut volumes_size = 0u64;
            for line in out.lines().skip(1) {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if parts.len() < 3 {
                    continue;
                }
                let kind = parts[0].to_lowercase();
                let size = parse_size(parts.last().unwrap_or(&"0"));
                if kind.contains("image") {
                    images_size = size;
                } else if kind.contains("container") || kind.contains("box") {
                    containers_size = size;
                } else if kind.contains("volume") {
                    volumes_size = size;
                }
            }
            let total = images_size + containers_size + volumes_size;
            Json(serde_json::json!({
                "images_size": images_size,
                "containers_size": containers_size,
                "volumes_size": volumes_size,
                "cache_size": 0,
                "total": total,
            }))
            .into_response()
        }
        Err(e) => e.into_response(),
    }
}

async fn box_system_prune() -> impl IntoResponse {
    match run_box(&["system-prune", "--force"]).await {
        Ok(_) => (StatusCode::OK, Json(serde_json::json!({"reclaimed": 0}))).into_response(),
        Err(e) => e.into_response(),
    }
}

// =============================================================================
// Box installation check & auto-install
// =============================================================================

/// GET /api/v1/box/check — returns whether `a3s-box` is available.
async fn box_check() -> impl IntoResponse {
    let bin = find_a3s_box();
    let out = tokio::process::Command::new(&bin)
        .arg("--version")
        .output()
        .await;

    let (installed, version) = match out {
        Ok(o) if o.status.success() => {
            let ver = String::from_utf8_lossy(&o.stdout).trim().to_string();
            (true, Some(ver))
        }
        _ => (false, None),
    };

    Json(serde_json::json!({ "installed": installed, "version": version }))
}

/// POST /api/v1/box/install — downloads the latest a3s-box release from
/// GitHub and installs it to `~/.local/bin/`. Streams progress as
/// `text/plain` (newline-delimited). Sends `[done]` or `[error]...` sentinels.
async fn box_install(State(state): State<BoxState>) -> impl IntoResponse {
    use axum::body::Body;
    use bytes::Bytes;
    use tokio::io::AsyncWriteExt;
    use tokio_stream::wrappers::ReceiverStream;

    let (tx, rx) = tokio::sync::mpsc::channel::<Bytes>(64);
    let skill_registry = state.skill_registry.clone();

    tokio::spawn(async move {
        let send = |msg: String| {
            let tx = tx.clone();
            async move {
                let _ = tx.send(Bytes::from(msg)).await;
            }
        };

        // Determine the release asset name for the current platform.
        let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("macos", "aarch64") => "macos-arm64",
            ("linux", "aarch64") => "linux-arm64",
            ("linux", "x86_64") => "linux-x86_64",
            (os, arch) => {
                send(format!("[error] 不支持的平台: {os}/{arch}\n")).await;
                return;
            }
        };

        let client = match reqwest::Client::builder().user_agent("safeclaw").build() {
            Ok(c) => c,
            Err(e) => {
                send(format!("[error] 初始化 HTTP 客户端失败: {e}\n")).await;
                return;
            }
        };

        // Fetch latest release metadata from GitHub API.
        send("正在获取最新版本信息…\n".to_string()).await;
        let release: serde_json::Value = match client
            .get("https://api.github.com/repos/A3S-Lab/Box/releases/latest")
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(r) => match r.json().await {
                Ok(v) => v,
                Err(e) => {
                    send(format!("[error] 解析版本信息失败: {e}\n")).await;
                    return;
                }
            },
            Err(e) => {
                send(format!("[error] 获取版本信息失败: {e}\n")).await;
                return;
            }
        };

        let tag = release["tag_name"]
            .as_str()
            .unwrap_or("latest")
            .trim_start_matches('v')
            .to_string();
        let asset_name = format!("a3s-box-v{tag}-{platform}.tar.gz");

        // Find the browser download URL from the assets list (fall back to
        // constructing the URL directly).
        let url = release["assets"]
            .as_array()
            .and_then(|assets| {
                assets
                    .iter()
                    .find(|a| a["name"].as_str() == Some(asset_name.as_str()))
            })
            .and_then(|a| a["browser_download_url"].as_str())
            .map(String::from)
            .unwrap_or_else(|| {
                format!("https://github.com/A3S-Lab/Box/releases/download/v{tag}/{asset_name}")
            });

        send(format!("正在下载 a3s-box v{tag}…\n")).await;

        // Download the tarball, streaming to a temp file.
        let mut response = match client
            .get(&url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(r) => r,
            Err(e) => {
                send(format!("[error] 下载失败: {e}\n")).await;
                return;
            }
        };

        let tmp_dir =
            std::env::temp_dir().join(format!("a3s-box-install-{}", uuid::Uuid::new_v4()));
        if let Err(e) = tokio::fs::create_dir_all(&tmp_dir).await {
            send(format!("[error] 创建临时目录失败: {e}\n")).await;
            return;
        }
        let tarball_path = tmp_dir.join(&asset_name);

        let mut file = match tokio::fs::File::create(&tarball_path).await {
            Ok(f) => f,
            Err(e) => {
                send(format!("[error] 创建临时文件失败: {e}\n")).await;
                return;
            }
        };

        let total = response.content_length();
        let mut downloaded: u64 = 0;
        let mut last_report: u64 = 0;

        loop {
            match response.chunk().await {
                Ok(Some(chunk)) => {
                    if let Err(e) = file.write_all(&chunk).await {
                        send(format!("[error] 写入失败: {e}\n")).await;
                        return;
                    }
                    downloaded += chunk.len() as u64;
                    // Report progress every ~256 KB as a structured sentinel.
                    if downloaded.saturating_sub(last_report) >= 262_144 {
                        last_report = downloaded;
                        send(format!(
                            "[progress:{}:{}]\n",
                            downloaded,
                            total.unwrap_or(0)
                        ))
                        .await;
                    }
                }
                Ok(None) => break,
                Err(e) => {
                    send(format!("[error] 下载中断: {e}\n")).await;
                    return;
                }
            }
        }
        drop(file);

        send("正在解压安装文件…\n".to_string()).await;

        // Prepare install directory: ~/.local/bin/
        let install_dir = match dirs::home_dir() {
            Some(h) => h.join(".local/bin"),
            None => {
                send("[error] 无法确定用户目录\n".to_string()).await;
                return;
            }
        };
        if let Err(e) = tokio::fs::create_dir_all(&install_dir).await {
            send(format!("[error] 创建安装目录失败: {e}\n")).await;
            return;
        }

        // Extract tarball into install directory.
        let extract_out = tokio::process::Command::new("tar")
            .args([
                "-xzf",
                tarball_path.to_str().unwrap_or_default(),
                "-C",
                install_dir.to_str().unwrap_or_default(),
            ])
            .output()
            .await;

        match extract_out {
            Ok(o) if o.status.success() => {}
            Ok(o) => {
                let err = String::from_utf8_lossy(&o.stderr);
                send(format!("[error] 解压失败: {err}\n")).await;
                let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
                return;
            }
            Err(e) => {
                send(format!("[error] 解压失败: {e}\n")).await;
                let _ = tokio::fs::remove_dir_all(&tmp_dir).await;
                return;
            }
        }

        // The tarball extracts to a versioned subdirectory (e.g., a3s-box-v0.8.0-macos-arm64/).
        // Move binaries from the subdirectory to ~/.local/bin/ root.
        send("正在移动二进制文件...\n".to_string()).await;

        // Find the extracted directory (should be the only directory in install_dir after extraction)
        let mut extracted_dir = None;
        if let Ok(mut entries) = tokio::fs::read_dir(&install_dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                if entry.path().is_dir()
                    && entry.file_name().to_string_lossy().starts_with("a3s-box")
                {
                    extracted_dir = Some(entry.path());
                    break;
                }
            }
        }

        if let Some(src_dir) = extracted_dir {
            // Move each binary from src_dir to install_dir
            for bin in &[
                "a3s-box",
                "a3s-box-shim",
                "a3s-box-guest-init",
                "a3s-box-cri",
            ] {
                let src = src_dir.join(bin);
                let dst = install_dir.join(bin);
                if src.exists() {
                    if let Err(e) = tokio::fs::rename(&src, &dst).await {
                        tracing::warn!("Failed to move {}: {}", bin, e);
                    }
                }
            }
            // Remove the now-empty extracted directory
            let _ = tokio::fs::remove_dir_all(&src_dir).await;
        }

        // Ensure all installed binaries are executable.
        for bin in &[
            "a3s-box",
            "a3s-box-shim",
            "a3s-box-guest-init",
            "a3s-box-cri",
        ] {
            let path = install_dir.join(bin);
            if path.exists() {
                let _ = tokio::process::Command::new("chmod")
                    .args(["+x", path.to_str().unwrap_or_default()])
                    .output()
                    .await;
            }
        }

        let _ = tokio::fs::remove_dir_all(&tmp_dir).await;

        send(format!("已安装到 {}\n", install_dir.display())).await;

        // Inject the a3s-box skill into the live registry so it's available
        // immediately without requiring a restart.
        if let Some(skill) = crate::skills::a3s_box_skill() {
            let name = skill.name.clone();
            match skill_registry.register(skill) {
                Ok(_) => tracing::info!(skill = %name, "Injected a3s-box skill after install"),
                Err(e) => tracing::warn!(skill = %name, "a3s-box skill injection failed: {e}"),
            }
        }

        send("[done]\n".to_string()).await;
        // tx drops here — stream ends
    });

    let stream = ReceiverStream::new(rx).map(Ok::<_, std::io::Error>);
    (
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        Body::from_stream(stream),
    )
}

// =============================================================================
// Workflow API
// =============================================================================

fn workflow_router(store: WorkflowStore) -> Router {
    Router::new()
        .route("/api/workflows", get(wf_list).post(wf_create))
        .route(
            "/api/workflows/:id",
            get(wf_get).patch(wf_update).delete(wf_delete),
        )
        .with_state(store)
}

async fn wf_list(State(store): State<WorkflowStore>) -> impl IntoResponse {
    match store.list().await {
        Ok(list) => Json(list).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn wf_get(State(store): State<WorkflowStore>, Path(id): Path<String>) -> impl IntoResponse {
    match store.get(&id).await {
        Ok(Some(wf)) => Json(wf).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn wf_create(
    State(store): State<WorkflowStore>,
    Json(body): Json<CreateWorkflow>,
) -> impl IntoResponse {
    match store.create(body).await {
        Ok(wf) => (StatusCode::CREATED, Json(wf)).into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn wf_update(
    State(store): State<WorkflowStore>,
    Path(id): Path<String>,
    Json(body): Json<UpdateWorkflow>,
) -> impl IntoResponse {
    match store.update(&id, body).await {
        Ok(Some(wf)) => Json(wf).into_response(),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn wf_delete(
    State(store): State<WorkflowStore>,
    Path(id): Path<String>,
) -> impl IntoResponse {
    match store.remove(&id).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "not found"})),
        )
            .into_response(),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// =============================================================================
// Workflow Execution API
//
// Exposes a3s-flow's FlowEngine over HTTP so the UI (and external callers)
// can start, control, and inspect live workflow executions, including full
// CRUD on the shared mutable context.
//
// Routes:
//   POST   /api/executions                    — start a new execution
//   GET    /api/executions/:id/state          — get lifecycle state + result
//   POST   /api/executions/:id/pause          — pause at next wave boundary
//   POST   /api/executions/:id/resume         — resume a paused execution
//   DELETE /api/executions/:id                — terminate immediately
//   GET    /api/executions/:id/context        — read full context snapshot
//   PUT    /api/executions/:id/context/:key   — set a single context entry
//   DELETE /api/executions/:id/context/:key   — delete a single context entry
// =============================================================================

#[derive(Clone)]
struct ExecState {
    engine: Arc<FlowEngine>,
}

fn execution_router(engine: Arc<FlowEngine>) -> Router {
    Router::new()
        .route("/api/executions", post(exec_start))
        .route("/api/executions/:id/state", get(exec_state))
        .route("/api/executions/:id/pause", post(exec_pause))
        .route("/api/executions/:id/resume", post(exec_resume))
        .route("/api/executions/:id", delete(exec_terminate))
        .route("/api/executions/:id/context", get(exec_context_get))
        .route(
            "/api/executions/:id/context/:key",
            put(exec_context_set).delete(exec_context_delete),
        )
        .with_state(ExecState { engine })
}

// ── Request / response types ───────────────────────────────────────────────

#[derive(Deserialize)]
struct StartExecutionRequest {
    definition: serde_json::Value,
    #[serde(default)]
    variables: std::collections::HashMap<String, serde_json::Value>,
}

#[derive(Serialize)]
struct StartExecutionResponse {
    id: Uuid,
}

#[derive(Serialize)]
struct ExecutionStateResponse {
    state: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Deserialize)]
struct SetContextEntryRequest {
    value: serde_json::Value,
}

// ── Helpers ────────────────────────────────────────────────────────────────

fn parse_exec_id(raw: &str) -> Result<Uuid, (StatusCode, Json<serde_json::Value>)> {
    raw.parse::<Uuid>().map_err(|_| {
        (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "invalid execution id"})),
        )
    })
}

fn flow_err_response(e: FlowError) -> (StatusCode, Json<serde_json::Value>) {
    let status = match &e {
        FlowError::ExecutionNotFound(_) => StatusCode::NOT_FOUND,
        FlowError::InvalidTransition { .. } => StatusCode::CONFLICT,
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (status, Json(serde_json::json!({"error": e.to_string()})))
}

fn state_to_response(state: ExecutionState) -> ExecutionStateResponse {
    match state {
        ExecutionState::Running => ExecutionStateResponse {
            state: "running",
            result: None,
            error: None,
        },
        ExecutionState::Paused => ExecutionStateResponse {
            state: "paused",
            result: None,
            error: None,
        },
        ExecutionState::Completed(result) => ExecutionStateResponse {
            state: "completed",
            result: serde_json::to_value(&result).ok(),
            error: None,
        },
        ExecutionState::Failed(msg) => ExecutionStateResponse {
            state: "failed",
            result: None,
            error: Some(msg),
        },
        ExecutionState::Terminated => ExecutionStateResponse {
            state: "terminated",
            result: None,
            error: None,
        },
    }
}

// ── Handlers ───────────────────────────────────────────────────────────────

async fn exec_start(
    State(s): State<ExecState>,
    Json(body): Json<StartExecutionRequest>,
) -> impl IntoResponse {
    match s.engine.start(&body.definition, body.variables).await {
        Ok(id) => (
            StatusCode::CREATED,
            Json(serde_json::to_value(StartExecutionResponse { id }).unwrap()),
        )
            .into_response(),
        Err(e) => {
            let (status, body) = flow_err_response(e);
            (status, body).into_response()
        }
    }
}

async fn exec_state(State(s): State<ExecState>, Path(id): Path<String>) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.state(id).await {
        Ok(state) => Json(state_to_response(state)).into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

async fn exec_pause(State(s): State<ExecState>, Path(id): Path<String>) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.pause(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

async fn exec_resume(State(s): State<ExecState>, Path(id): Path<String>) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.resume(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

async fn exec_terminate(State(s): State<ExecState>, Path(id): Path<String>) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.terminate(id).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

async fn exec_context_get(State(s): State<ExecState>, Path(id): Path<String>) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.get_context(id).await {
        Ok(ctx) => Json(ctx).into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

async fn exec_context_set(
    State(s): State<ExecState>,
    Path((id, key)): Path<(String, String)>,
    Json(body): Json<SetContextEntryRequest>,
) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.set_context_entry(id, key, body.value).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

async fn exec_context_delete(
    State(s): State<ExecState>,
    Path((id, key)): Path<(String, String)>,
) -> impl IntoResponse {
    let id = match parse_exec_id(&id) {
        Ok(v) => v,
        Err(e) => return e.into_response(),
    };
    match s.engine.delete_context_entry(id, &key).await {
        Ok(true) => StatusCode::NO_CONTENT.into_response(),
        Ok(false) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "key not found"})),
        )
            .into_response(),
        Err(e) => flow_err_response(e).into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let resp = health_check().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_build_cors_empty_origins() {
        let _cors = build_cors(&[]);
    }
}
