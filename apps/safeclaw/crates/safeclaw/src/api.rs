//! Unified API router for SafeClaw
//!
//! Merges all module routers into a single axum `Router` with CORS,
//! consistent error handling, and a shared application state.
//!
//! ## Endpoint Map
//!
//! | Prefix                          | Module   | Description                    |
//! |---------------------------------|----------|--------------------------------|
//! | `/health`                       | runtime | Load balancer health probe     |
//! | `/.well-known/a3s-service.json` | runtime | Service discovery              |
//! | `/api/v1/gateway/*`             | runtime | Status, sessions, message, webhook |
//! | `/api/v1/tee/status`            | runtime | TEE security level             |
//! | `/api/v1/channels`              | runtime | Channel listing & config       |
//! | `/api/v1/security/overview`     | runtime | Aggregated security dashboard  |
//! | `/api/v1/taint/entries`         | runtime | Taint tracking entries         |
//! | `/api/v1/memory/*`              | runtime | Memory layers, stats, export   |
//! | `/api/v1/users`                 | runtime | User management (stub Phase 20)|
//! | `/api/v1/privacy/*`             | privacy  | Classify, analyze, scan        |
//! | `/api/v1/audit/*`               | leakage  | Audit events, stats, query     |
//! | `/api/agent/*`                  | agent    | Agent sessions, backends       |
//! | `/ws/agent/browser/:id`         | agent    | Agent WebSocket                |

use crate::agent::{agent_router, AgentState};
use crate::audit::handler::{audit_router, AuditState};
use crate::audit::AuditLog;
use crate::config::{ChannelAgentConfig, ChannelAgentConfigStore};
use crate::privacy::handler::{privacy_router, PrivacyState};
use crate::runtime::Runtime;
use a3s_memory::MemoryStore;
use axum::{
    extract::{Path, Query, State},
    http::{header, Method, StatusCode},
    response::IntoResponse,
    routing::{delete, get, patch, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use tower_http::cors::{Any, CorsLayer};

/// Combined application state holding references to all subsystems
#[derive(Clone)]
pub struct AppState {
    pub gateway: Arc<Runtime>,
    pub audit_log: Arc<RwLock<AuditLog>>,
}

/// Shared state for memory endpoints
#[derive(Clone)]
pub struct MemoryState {
    pub store: Arc<dyn MemoryStore>,
}

/// Build the complete SafeClaw HTTP application
///
/// Merges all module routers, adds CORS middleware, and returns a single
/// `Router` ready to be served by `axum::serve`.
pub fn build_app(
    gateway: Arc<Runtime>,
    agent_state: AgentState,
    privacy_state: PrivacyState,
    audit_state: AuditState,
    memory_store: Arc<dyn MemoryStore>,
    channel_config_store: ChannelAgentConfigStore,
    cors_origins: &[String],
) -> Router {
    let cors = build_cors(cors_origins);
    let gateway_routes = gateway_api_router(gateway.clone(), channel_config_store);
    let memory_routes = memory_api_router(MemoryState {
        store: memory_store,
    });

    Router::new()
        .route("/health", get(health_check))
        .route("/.well-known/a3s-service.json", get(service_discovery))
        .merge(gateway_routes)
        .merge(memory_routes)
        .merge(agent_router(agent_state))
        .merge(privacy_router(privacy_state))
        .merge(audit_router(audit_state))
        .layer(cors)
}

// =============================================================================
// Gateway sub-router (re-mounted under /api/v1/gateway)
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
        // Security & observability endpoints
        .route("/api/v1/tee/status", get(tee_status))
        .route("/api/v1/channels", get(list_channels))
        .route("/api/v1/security/overview", get(security_overview))
        .route("/api/v1/taint/entries", get(list_taint_entries))
        // User management endpoints (stub — Phase 20)
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

async fn service_discovery() -> impl IntoResponse {
    Json(crate::runtime::build_service_descriptor())
}

// =============================================================================
// Gateway handlers (delegating to Runtime methods)
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

    // Parse the webhook synchronously — handles decrypt, dedup, ACL
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
        // Challenge must be returned synchronously
        WebhookParseResult::Challenge(response) => {
            if response.outbound.chat_id == "__challenge__" {
                return (
                    StatusCode::OK,
                    Json(serde_json::json!({"challenge": response.outbound.content})),
                );
            }
            (StatusCode::OK, Json(serde_json::json!({"status": "ok"})))
        }
        // Message: return 200 immediately, process with streaming in background
        WebhookParseResult::Message(inbound) => {
            let gw = gateway.clone();
            let ch = channel.clone();
            tokio::spawn(async move {
                let channels = gw.channels().read().await;
                let adapter = match channels.get(&ch) {
                    Some(a) => a.clone(),
                    None => {
                        tracing::warn!(channel = %ch, "No channel adapter registered");
                        return;
                    }
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
        // Card action: must return updated card JSON synchronously (within 3s)
        WebhookParseResult::CardAction(action) => {
            let channels = gateway.channels().read().await;
            let adapter = match channels.get(&channel) {
                Some(a) => a.clone(),
                None => {
                    tracing::warn!(channel = %channel, "No channel adapter for card action");
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
// TEE / Channels / Security / Taint handlers
// =============================================================================

/// GET /api/v1/tee/status
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

/// GET /api/v1/channels — list active channels with metadata
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

/// PATCH /api/v1/channels/:channel_id/agent-config — persist per-channel agent config
async fn update_channel_agent_config(
    State(store): State<ChannelAgentConfigStore>,
    Path(channel_id): Path<String>,
    Json(body): Json<ChannelAgentConfig>,
) -> impl IntoResponse {
    tracing::info!(channel = %channel_id, "Updating channel agent config");
    match store.set(&channel_id, body).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({
                "status": "saved",
                "channelId": channel_id,
            })),
        ),
        Err(e) => {
            tracing::warn!(channel = %channel_id, "Failed to persist channel agent config: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "status": "error",
                    "message": format!("Failed to persist config: {}", e),
                })),
            )
        }
    }
}

/// GET /api/v1/channels/:channel_id/agent-config — retrieve per-channel agent config
async fn get_channel_agent_config(
    State(store): State<ChannelAgentConfigStore>,
    Path(channel_id): Path<String>,
) -> impl IntoResponse {
    let config = store.get(&channel_id).await.unwrap_or_default();
    Json(serde_json::json!({
        "channelId": channel_id,
        "config": config,
    }))
}

/// GET /api/v1/channel-agent-configs — list all per-channel agent configs
async fn list_channel_agent_configs(
    State(store): State<ChannelAgentConfigStore>,
) -> impl IntoResponse {
    let configs = store.get_all().await;
    Json(serde_json::json!({ "configs": configs }))
}

/// GET /api/v1/security/overview — aggregated security dashboard data
async fn security_overview(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
) -> impl IntoResponse {
    let level = gateway.security_level();
    let channels = gateway.channels().read().await;
    let channel_count = channels.len();
    let connected_count = channels.values().filter(|a| a.is_connected()).count();
    drop(channels);

    let session_count = gateway.session_manager().active_sessions().await.len();
    let audit_log = gateway.global_audit_log().read().await;
    let total_events = audit_log.total_count();
    let critical_count = audit_log
        .by_severity(crate::audit::AuditSeverity::Critical)
        .len();
    let high_count = audit_log
        .by_severity(crate::audit::AuditSeverity::High)
        .len();
    drop(audit_log);

    Json(serde_json::json!({
        "tee": {
            "securityLevel": level,
            "description": level.description(),
        },
        "channels": {
            "total": channel_count,
            "connected": connected_count,
        },
        "sessions": {
            "active": session_count,
        },
        "audit": {
            "totalEvents": total_events,
            "criticalCount": critical_count,
            "highCount": high_count,
        },
    }))
}

/// GET /api/v1/taint/entries — list taint tracking entries across sessions
async fn list_taint_entries(
    axum::extract::State(gateway): axum::extract::State<Arc<Runtime>>,
) -> impl IntoResponse {
    let isolation = gateway.session_manager().isolation();
    let session_ids = isolation.session_ids().await;
    let mut entries = Vec::new();

    for sid in &session_ids {
        if let Some(guard) = isolation.registry(sid).await {
            let session_entries = guard
                .read(|reg| {
                    reg.entries()
                        .values()
                        .map(|entry| {
                            serde_json::json!({
                                "id": entry.id,
                                "sessionId": sid,
                                "taintType": entry.taint_type,
                                "variantCount": entry.variants.len(),
                                "createdAt": entry.created_at,
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .await;
            if let Some(session_entries) = session_entries {
                entries.extend(session_entries);
            }
        }
    }

    Json(entries)
}

// =============================================================================
// Memory endpoints — backed by a3s-memory MemoryStore
// =============================================================================

/// Memory layer → MemoryType mapping.
///
/// The UI uses a three-layer model (resources / artifacts / insights) which maps
/// to a3s-memory's `MemoryType` enum:
///   - resources → Episodic (raw observations from sessions)
///   - artifacts → Semantic + Procedural (structured knowledge & patterns)
///   - insights  → items tagged "insight" with high importance (cross-session synthesis)
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

/// Serialize a MemoryItem into the JSON shape the UI expects.
fn memory_item_to_json(item: &a3s_memory::MemoryItem) -> serde_json::Value {
    let now = chrono::Utc::now();
    let age_days = (now - item.timestamp).num_days();
    let decay_days = (90 - age_days).max(0);

    // Taint labels are stored as tags with "taint:" prefix
    let taint_labels: Vec<&str> = item
        .tags
        .iter()
        .filter(|t| t.starts_with("taint:"))
        .map(|t| t.as_str())
        .collect();

    // Derive layer from memory type
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

/// GET /api/v1/memory/:layer — list memory entries for a layer
async fn list_memory_layer(
    State(state): State<MemoryState>,
    Path(layer): Path<String>,
    Query(params): Query<MemoryQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100);

    // If search query provided, search across all items then filter by layer
    let items = if let Some(ref query) = params.search {
        match state.store.search(query, limit * 3).await {
            Ok(items) => items,
            Err(e) => {
                tracing::warn!("Memory search failed: {}", e);
                return Json(serde_json::json!({
                    "layer": layer, "entries": [], "total": 0
                }));
            }
        }
    } else {
        // Fetch by layer filter
        let result = match layer_to_filter(&layer) {
            LayerFilter::Types(types) => {
                // get_recent returns all types; we filter in-memory
                state.store.get_recent(limit * 3).await.map(|items| {
                    items
                        .into_iter()
                        .filter(|i| types.contains(&i.memory_type))
                        .collect::<Vec<_>>()
                })
            }
            LayerFilter::Tag(tag) => state.store.search_by_tags(&[tag], limit).await,
        };
        match result {
            Ok(items) => items,
            Err(e) => {
                tracing::warn!("Memory list failed: {}", e);
                return Json(serde_json::json!({
                    "layer": layer, "entries": [], "total": 0
                }));
            }
        }
    };

    // Filter by layer if we did a search (search returns all types)
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

    Json(serde_json::json!({
        "layer": layer,
        "entries": entries,
        "total": total,
    }))
}

/// DELETE /api/v1/memory/:singular/:id — delete a memory entry
async fn delete_memory_entry(
    State(state): State<MemoryState>,
    Path((_singular, id)): Path<(String, String)>,
) -> impl IntoResponse {
    tracing::info!(id = %id, "Memory entry delete requested");
    match state.store.delete(&id).await {
        Ok(()) => (
            StatusCode::OK,
            Json(serde_json::json!({ "status": "deleted", "id": id })),
        ),
        Err(e) => {
            tracing::warn!(id = %id, "Memory delete failed: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({ "status": "error", "message": e.to_string() })),
            )
        }
    }
}

/// DELETE /api/v1/memory/batch — batch delete memory entries
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
            Json(serde_json::json!({ "status": "error", "message": "No ids provided" })),
        );
    }

    let mut deleted = 0u32;
    let mut errors = 0u32;
    for id in &ids {
        match state.store.delete(id).await {
            Ok(()) => deleted += 1,
            Err(e) => {
                tracing::warn!(id = %id, "Batch delete failed for entry: {}", e);
                errors += 1;
            }
        }
    }

    tracing::info!(deleted, errors, "Batch memory delete completed");
    (
        StatusCode::OK,
        Json(serde_json::json!({ "status": "ok", "deleted": deleted, "errors": errors })),
    )
}

/// GET /api/v1/memory/stats — memory statistics across all layers
async fn memory_stats(State(state): State<MemoryState>) -> impl IntoResponse {
    // Fetch all items to compute stats
    let all_items = state.store.get_recent(10000).await.unwrap_or_default();
    let now = chrono::Utc::now();

    let mut resources = 0u32;
    let mut artifacts = 0u32;
    let mut insights = 0u32;
    let mut imp_resources = 0f64;
    let mut imp_artifacts = 0f64;
    let mut imp_insights = 0f64;
    let mut expiring_7 = 0u32;
    let mut expiring_30 = 0u32;
    let mut tainted = 0u32;

    for item in &all_items {
        let age_days = (now - item.timestamp).num_days();
        let decay_days = (90 - age_days).max(0);
        let is_insight = item.tags.iter().any(|t| t == "insight");
        let has_taint = item.tags.iter().any(|t| t.starts_with("taint:"));

        if has_taint {
            tainted += 1;
        }
        if decay_days <= 7 {
            expiring_7 += 1;
        } else if decay_days <= 30 {
            expiring_30 += 1;
        }

        if is_insight {
            insights += 1;
            imp_insights += item.importance as f64;
        } else {
            match item.memory_type {
                a3s_memory::MemoryType::Episodic | a3s_memory::MemoryType::Working => {
                    resources += 1;
                    imp_resources += item.importance as f64;
                }
                a3s_memory::MemoryType::Semantic | a3s_memory::MemoryType::Procedural => {
                    artifacts += 1;
                    imp_artifacts += item.importance as f64;
                }
            }
        }
    }

    let avg = |sum: f64, count: u32| {
        if count == 0 {
            0.0
        } else {
            sum / count as f64
        }
    };

    Json(serde_json::json!({
        "layers": { "resources": resources, "artifacts": artifacts, "insights": insights },
        "avgImportance": {
            "resources": avg(imp_resources, resources),
            "artifacts": avg(imp_artifacts, artifacts),
            "insights": avg(imp_insights, insights),
        },
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

/// GET /api/v1/memory/export — export memory entries as JSON or CSV
async fn memory_export(
    State(state): State<MemoryState>,
    Query(params): Query<ExportQuery>,
) -> impl IntoResponse {
    let all_items = state.store.get_recent(10000).await.unwrap_or_default();

    // Filter by layer if specified
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
            // Escape CSV fields
            let escape = |s: &str| format!("\"{}\"", s.replace('"', "\"\""));
            csv.push_str(&format!(
                "{},{},{},{},{},{:?},{}\n",
                item.id,
                escape(&title),
                escape(&item.content),
                escape(&item.tags.join(";")),
                item.importance,
                item.memory_type,
                item.timestamp.to_rfc3339(),
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
// User management endpoints (stub — Phase 20)
// =============================================================================

/// GET /api/v1/users
async fn list_users() -> impl IntoResponse {
    // Stub: return empty list until Phase 20 multi-user support
    Json(serde_json::json!({
        "users": [],
        "total": 0,
        "message": "User management available in Phase 20",
    }))
}

/// POST /api/v1/users
async fn create_user(Json(body): Json<serde_json::Value>) -> impl IntoResponse {
    tracing::info!(user = %body, "User creation requested");
    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "status": "accepted",
            "message": "User management available in Phase 20",
        })),
    )
}

/// PATCH /api/v1/users/:id
async fn update_user(
    Path(id): Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    tracing::info!(user_id = %id, updates = %body, "User update requested");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "accepted",
            "userId": id,
            "message": "User management available in Phase 20",
        })),
    )
}

/// DELETE /api/v1/users/:id
async fn delete_user(Path(id): Path<String>) -> impl IntoResponse {
    tracing::info!(user_id = %id, "User deletion requested");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "status": "accepted",
            "userId": id,
            "message": "User management available in Phase 20",
        })),
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

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_health_check() {
        let resp = health_check().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_service_discovery() {
        let resp = service_discovery().await.into_response();
        assert_eq!(resp.status(), StatusCode::OK);
    }

    #[test]
    fn test_build_cors_empty_origins() {
        let _cors = build_cors(&[]);
    }

    #[test]
    fn test_build_cors_with_origins() {
        let _cors = build_cors(&[
            "http://localhost:1420".to_string(),
            "https://app.example.com".to_string(),
        ]);
    }
}
