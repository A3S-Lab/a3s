//! HTTP API handler

use crate::error::to_json;
use crate::runtime::Runtime;
use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// API handler for HTTP endpoints
pub struct ApiHandler {
    #[allow(dead_code)]
    gateway: Arc<Runtime>,
}

impl ApiHandler {
    /// Create a new API handler
    pub fn new(gateway: Arc<Runtime>) -> Self {
        Self { gateway }
    }

    /// Create the router
    pub fn router(gateway: Arc<Runtime>) -> Router {
        Router::new()
            .route("/health", get(health_check))
            .route("/.well-known/a3s-service.json", get(service_discovery))
            .route("/status", get(get_status))
            .route("/sessions", get(list_sessions))
            .route("/sessions/:id", get(get_session))
            .route("/message", post(send_message))
            .route("/api/v1/tee/status", get(get_tee_status))
            .route("/api/v1/taint/entries", get(get_taint_entries))
            .route("/api/v1/channels", get(get_channels))
            .route("/api/v1/security/overview", get(get_security_overview))
            .with_state(gateway)
    }
}

/// Health check response
#[derive(Debug, Serialize)]
struct HealthResponse {
    status: String,
    version: String,
    security_level: crate::tee::SecurityLevel,
}

/// Health check endpoint
async fn health_check(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
        security_level: gateway.security_level(),
    })
}

/// Service discovery endpoint for a3s-gateway
async fn service_discovery() -> impl IntoResponse {
    let descriptor = crate::runtime::integration::build_service_descriptor();
    Json(descriptor)
}

/// Status response
#[derive(Debug, Serialize)]
struct StatusResponse {
    state: String,
    tee_enabled: bool,
    security_level: crate::tee::SecurityLevel,
    tee_available: bool,
    session_count: usize,
    channels: Vec<String>,
}

/// Get gateway status
async fn get_status(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    let state = gateway.state().await;
    let session_count = gateway.session_manager().session_count().await;
    let security_level = gateway.security_level();

    Json(StatusResponse {
        state: format!("{:?}", state),
        tee_enabled: gateway.config().tee.enabled,
        security_level,
        tee_available: security_level == crate::tee::SecurityLevel::TeeHardware,
        session_count,
        channels: gateway.active_channel_names().await,
    })
}

/// Session info response
#[derive(Debug, Serialize)]
struct SessionInfo {
    id: String,
    user_id: String,
    channel_id: String,
    chat_id: String,
    uses_tee: bool,
    created_at: i64,
    message_count: u64,
}

/// List all sessions
async fn list_sessions(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    let sessions = gateway.session_manager().active_sessions().await;
    let mut infos = Vec::new();

    for session in sessions {
        infos.push(SessionInfo {
            id: session.id.clone(),
            user_id: session.user_id.clone(),
            channel_id: session.channel_id.clone(),
            chat_id: session.chat_id.clone(),
            uses_tee: session.uses_tee().await,
            created_at: session.created_at,
            message_count: session.message_count().await,
        });
    }

    Json(infos)
}

/// Get a specific session
async fn get_session(
    State(gateway): State<Arc<Runtime>>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> impl IntoResponse {
    match gateway.session_manager().get_session(&id).await {
        Some(session) => {
            let info = SessionInfo {
                id: session.id.clone(),
                user_id: session.user_id.clone(),
                channel_id: session.channel_id.clone(),
                chat_id: session.chat_id.clone(),
                uses_tee: session.uses_tee().await,
                created_at: session.created_at,
                message_count: session.message_count().await,
            };
            (StatusCode::OK, Json(to_json(info)))
        }
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Session not found"})),
        ),
    }
}

/// Send message request
#[derive(Debug, Deserialize)]
struct SendMessageRequest {
    channel: String,
    chat_id: String,
    content: String,
}

/// Send message response
#[derive(Debug, Serialize)]
struct SendMessageResponse {
    message_id: String,
    status: String,
}

/// Send a message
async fn send_message(
    State(gateway): State<Arc<Runtime>>,
    Json(request): Json<SendMessageRequest>,
) -> impl IntoResponse {
    let channels = gateway.channels().read().await;
    if let Some(channel) = channels.get(&request.channel) {
        let outbound = crate::channels::OutboundMessage::new(
            &request.channel,
            &request.chat_id,
            &request.content,
        );
        match channel.send_message(outbound).await {
            Ok(message_id) => (
                StatusCode::OK,
                Json(SendMessageResponse {
                    message_id,
                    status: "sent".to_string(),
                }),
            ),
            Err(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(SendMessageResponse {
                    message_id: String::new(),
                    status: format!("error: {}", e),
                }),
            ),
        }
    } else {
        (
            StatusCode::NOT_FOUND,
            Json(SendMessageResponse {
                message_id: String::new(),
                status: format!("channel '{}' not found", request.channel),
            }),
        )
    }
}

// =============================================================================
// Security monitoring endpoints
// =============================================================================

/// TEE status response
#[derive(Debug, Serialize)]
struct TeeStatusResponse {
    level: crate::tee::SecurityLevel,
    description: String,
    tee_active: bool,
    has_sealed_storage: bool,
}

/// GET /api/v1/tee/status
async fn get_tee_status(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    let level = gateway.security_level();
    Json(TeeStatusResponse {
        description: level.description().to_string(),
        tee_active: level == crate::tee::SecurityLevel::TeeHardware,
        has_sealed_storage: gateway.session_manager().tee_runtime().has_sealed_storage(),
        level,
    })
}

/// Taint entry response (omits the raw original value for safety)
#[derive(Debug, Serialize)]
struct TaintEntryResponse {
    id: String,
    taint_type: crate::guard::taint::TaintType,
    variant_count: usize,
    created_at: i64,
    session_id: String,
}

/// GET /api/v1/taint/entries
async fn get_taint_entries(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    let isolation = gateway.session_manager().isolation();
    let session_ids = isolation.session_ids().await;
    let mut entries: Vec<TaintEntryResponse> = Vec::new();

    for session_id in session_ids {
        if let Some(guard) = isolation.registry(&session_id).await {
            if let Some(session_entries) = guard
                .read(|r| {
                    r.entries()
                        .values()
                        .map(|e| TaintEntryResponse {
                            id: e.id.clone(),
                            taint_type: e.taint_type.clone(),
                            variant_count: e.variants.len(),
                            created_at: e.created_at,
                            session_id: session_id.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .await
            {
                entries.extend(session_entries);
            }
        }
    }

    Json(entries)
}

/// Channel status response
#[derive(Debug, Serialize)]
struct ChannelStatusResponse {
    name: String,
}

/// GET /api/v1/channels
async fn get_channels(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    let names = gateway.active_channel_names().await;
    let channels: Vec<ChannelStatusResponse> = names
        .into_iter()
        .map(|name| ChannelStatusResponse { name })
        .collect();
    Json(channels)
}

/// Security overview response
#[derive(Debug, Serialize)]
struct SecurityOverviewResponse {
    security_level: crate::tee::SecurityLevel,
    tee_active: bool,
    session_count: usize,
    active_channels: usize,
    total_taint_entries: usize,
}

/// GET /api/v1/security/overview
async fn get_security_overview(State(gateway): State<Arc<Runtime>>) -> impl IntoResponse {
    let level = gateway.security_level();
    let session_count = gateway.session_manager().session_count().await;
    let active_channels = gateway.active_channel_names().await.len();

    // Count total taint entries across all sessions
    let isolation = gateway.session_manager().isolation();
    let session_ids = isolation.session_ids().await;
    let mut total_taint_entries = 0usize;
    for session_id in &session_ids {
        if let Some(guard) = isolation.registry(session_id).await {
            if let Some(count) = guard.read(|r| r.len()).await {
                total_taint_entries += count;
            }
        }
    }

    Json(SecurityOverviewResponse {
        tee_active: level == crate::tee::SecurityLevel::TeeHardware,
        security_level: level,
        session_count,
        active_channels,
        total_taint_entries,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::RuntimeBuilder;

    #[tokio::test]
    async fn test_health_check() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = health_check(State(gateway)).await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_service_discovery() {
        let response = service_discovery().await.into_response();
        assert_eq!(response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn test_health_check_includes_security_level() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = health_check(State(gateway)).await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["security_level"], "process_only");
    }

    #[tokio::test]
    async fn test_status_includes_security_level() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = get_status(State(gateway)).await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["security_level"], "process_only");
        assert_eq!(json["tee_available"], false);
    }

    #[tokio::test]
    async fn test_tee_status() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = get_tee_status(State(gateway)).await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["level"], "process_only");
        assert_eq!(json["tee_active"], false);
        assert!(json["description"].is_string());
    }

    #[tokio::test]
    async fn test_taint_entries_empty() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = get_taint_entries(State(gateway)).await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_channels_empty() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = get_channels(State(gateway)).await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert!(json.as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_security_overview() {
        let gateway = Arc::new(RuntimeBuilder::new().build().unwrap());
        let response = get_security_overview(State(gateway)).await.into_response();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["security_level"], "process_only");
        assert_eq!(json["tee_active"], false);
        assert_eq!(json["session_count"], 0);
        assert_eq!(json["active_channels"], 0);
        assert_eq!(json["total_taint_entries"], 0);
    }
}
