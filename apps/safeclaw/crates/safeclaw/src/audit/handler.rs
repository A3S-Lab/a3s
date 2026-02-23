//! HTTP handlers for the Audit API
//!
//! Exposes the leakage prevention audit log via REST endpoints:
//! - GET /api/v1/audit/events       — list audit events (paginated, filterable)
//! - GET /api/v1/audit/events/:id   — get single audit event
//! - GET /api/v1/audit/stats        — summary statistics

use super::alerting::AlertMonitor;
use super::log::{AuditEvent, AuditLog, AuditSeverity};
use crate::error::to_json;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;

/// Shared state for audit handlers
#[derive(Clone)]
pub struct AuditState {
    pub log: Arc<RwLock<AuditLog>>,
    pub alert_monitor: Option<Arc<AlertMonitor>>,
    pub persistence: Option<Arc<crate::audit::AuditPersistence>>,
}

/// Create the audit router
pub fn audit_router(state: AuditState) -> Router {
    Router::new()
        .route("/api/v1/audit/events", get(list_events))
        .route("/api/v1/audit/events/:id", get(get_event))
        .route("/api/v1/audit/stats", get(get_stats))
        .route("/api/v1/audit/alerts", get(get_alerts))
        .route(
            "/api/v1/audit/query",
            get(query_events).post(query_events_post),
        )
        .route("/api/v1/audit/export", get(export_events))
        .with_state(state)
}

// =============================================================================
// Query / Response types
// =============================================================================

#[derive(Debug, Deserialize)]
struct ListEventsQuery {
    session: Option<String>,
    severity: Option<String>,
    vector: Option<String>,
    limit: Option<usize>,
    offset: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct AdvancedQuery {
    session: Option<String>,
    severity: Option<String>,
    vector: Option<String>,
    from: Option<i64>,
    to: Option<i64>,
    q: Option<String>,
    limit: Option<usize>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct AuditStatsResponse {
    total_recorded: u64,
    buffered: usize,
    by_severity: SeverityCounts,
}

#[derive(Debug, Serialize)]
struct SeverityCounts {
    info: usize,
    warning: usize,
    high: usize,
    critical: usize,
}

// =============================================================================
// Handlers
// =============================================================================

/// GET /api/v1/audit/events
async fn list_events(
    State(state): State<AuditState>,
    Query(params): Query<ListEventsQuery>,
) -> impl IntoResponse {
    let log = state.log.read().await;
    let limit = params.limit.unwrap_or(50).min(500);
    let offset = params.offset.unwrap_or(0);

    let mut events: Vec<&AuditEvent> = if let Some(session_id) = &params.session {
        log.by_session(session_id)
    } else if let Some(severity_str) = &params.severity {
        match parse_severity(severity_str) {
            Some(s) => log.by_severity(s),
            None => log.recent(log.len()),
        }
    } else {
        log.recent(log.len())
    };

    // Filter by vector if specified
    if let Some(ref vector_str) = params.vector {
        if let Some(vector) = parse_vector(vector_str) {
            events.retain(|e| e.vector == vector);
        }
    }

    // Apply offset + limit
    let total = events.len();
    if offset < events.len() {
        events = events[offset..].to_vec();
    } else {
        events.clear();
    }
    events.truncate(limit);

    // Clone to release the read lock before serializing
    let owned: Vec<AuditEvent> = events.into_iter().cloned().collect();
    Json(serde_json::json!({
        "events": owned,
        "total": total,
        "offset": offset,
        "limit": limit,
    }))
}

/// GET /api/v1/audit/events/:id
async fn get_event(State(state): State<AuditState>, Path(id): Path<String>) -> impl IntoResponse {
    let log = state.log.read().await;

    // Search through recent events for the matching ID
    let all = log.recent(log.len());
    match all.into_iter().find(|e| e.id == id) {
        Some(event) => (StatusCode::OK, Json(to_json(event))),
        None => (
            StatusCode::NOT_FOUND,
            Json(
                serde_json::json!({"error": {"code": "NOT_FOUND", "message": format!("Audit event {} not found", id)}}),
            ),
        ),
    }
}

/// GET /api/v1/audit/stats
async fn get_stats(State(state): State<AuditState>) -> impl IntoResponse {
    let log = state.log.read().await;

    Json(AuditStatsResponse {
        total_recorded: log.total_count(),
        buffered: log.len(),
        by_severity: SeverityCounts {
            info: log.by_severity(AuditSeverity::Info).len(),
            warning: log.by_severity(AuditSeverity::Warning).len(),
            high: log.by_severity(AuditSeverity::High).len(),
            critical: log.by_severity(AuditSeverity::Critical).len(),
        },
    })
}

// =============================================================================
// Alerts query / handler
// =============================================================================

#[derive(Debug, Deserialize)]
struct AlertsQuery {
    limit: Option<usize>,
}

/// GET /api/v1/audit/alerts
async fn get_alerts(
    State(state): State<AuditState>,
    Query(params): Query<AlertsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(500);

    match &state.alert_monitor {
        Some(monitor) => {
            let alerts = monitor.recent_alerts(limit).await;
            Json(serde_json::json!({
                "alerts": alerts,
                "total": monitor.alert_count().await,
            }))
        }
        None => Json(serde_json::json!({
            "alerts": [],
            "total": 0,
            "message": "Alert monitor not enabled",
        })),
    }
}

// =============================================================================
// Advanced query / export handlers
// =============================================================================

fn parse_severity(s: &str) -> Option<AuditSeverity> {
    match s {
        "info" => Some(AuditSeverity::Info),
        "warning" => Some(AuditSeverity::Warning),
        "high" => Some(AuditSeverity::High),
        "critical" => Some(AuditSeverity::Critical),
        _ => None,
    }
}

fn parse_vector(s: &str) -> Option<crate::audit::LeakageVector> {
    use crate::audit::LeakageVector;
    match s {
        "output_channel" => Some(LeakageVector::OutputChannel),
        "tool_call" => Some(LeakageVector::ToolCall),
        "dangerous_command" => Some(LeakageVector::DangerousCommand),
        "network_exfil" => Some(LeakageVector::NetworkExfil),
        "file_exfil" => Some(LeakageVector::FileExfil),
        "auth_failure" => Some(LeakageVector::AuthFailure),
        _ => None,
    }
}

/// GET /api/v1/audit/query — advanced query with time range, vector, text search
async fn query_events(
    State(state): State<AuditState>,
    Query(params): Query<AdvancedQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100).min(1000);

    let filter = crate::audit::AuditQueryFilter {
        session_id: params.session,
        severity: params.severity.as_deref().and_then(parse_severity),
        vector: params.vector.as_deref().and_then(parse_vector),
        from_ms: params.from,
        to_ms: params.to,
        search: params.q,
        limit: Some(limit),
    };

    // Try persisted events first, fall back to in-memory log
    if let Some(ref persistence) = state.persistence {
        let mut results = persistence.query(&filter).await;
        results.truncate(limit);
        Json(serde_json::json!({
            "events": results,
            "total": results.len(),
            "source": "persisted",
        }))
    } else {
        let log = state.log.read().await;
        let all: Vec<&AuditEvent> = log.recent(log.len());
        let filtered: Vec<AuditEvent> = all
            .into_iter()
            .filter(|e| filter.matches(e))
            .take(limit)
            .cloned()
            .collect();
        let count = filtered.len();
        Json(serde_json::json!({
            "events": filtered,
            "total": count,
            "source": "memory",
        }))
    }
}

/// POST /api/v1/audit/query — same as GET but accepts body (for frontend compatibility)
async fn query_events_post(
    State(state): State<AuditState>,
    Json(params): Json<AdvancedQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(100).min(1000);

    let filter = crate::audit::AuditQueryFilter {
        session_id: params.session,
        severity: params.severity.as_deref().and_then(parse_severity),
        vector: params.vector.as_deref().and_then(parse_vector),
        from_ms: params.from,
        to_ms: params.to,
        search: params.q,
        limit: Some(limit),
    };

    if let Some(ref persistence) = state.persistence {
        let mut results = persistence.query(&filter).await;
        results.truncate(limit);
        Json(serde_json::json!({
            "events": results,
            "total": results.len(),
            "source": "persisted",
        }))
    } else {
        let log = state.log.read().await;
        let all: Vec<&AuditEvent> = log.recent(log.len());
        let filtered: Vec<AuditEvent> = all
            .into_iter()
            .filter(|e| filter.matches(e))
            .take(limit)
            .cloned()
            .collect();
        let count = filtered.len();
        Json(serde_json::json!({
            "events": filtered,
            "total": count,
            "source": "memory",
        }))
    }
}

/// GET /api/v1/audit/export — export all persisted events as JSON array
async fn export_events(State(state): State<AuditState>) -> impl IntoResponse {
    if let Some(ref persistence) = state.persistence {
        let events = persistence.export_all().await;
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "events": events,
                "total": events.len(),
            })),
        )
    } else {
        // Fall back to in-memory log
        let log = state.log.read().await;
        let events: Vec<AuditEvent> = log.recent(log.len()).into_iter().cloned().collect();
        let count = events.len();
        (
            StatusCode::OK,
            Json(serde_json::json!({
                "events": events,
                "total": count,
                "source": "memory",
            })),
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::LeakageVector;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn make_app() -> (Router, Arc<RwLock<AuditLog>>) {
        let log = Arc::new(RwLock::new(AuditLog::new(1000)));
        let state = AuditState {
            log: log.clone(),
            alert_monitor: None,
            persistence: None,
        };
        (audit_router(state), log)
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    async fn seed_events(log: &Arc<RwLock<AuditLog>>) {
        let mut l = log.write().await;
        l.record(AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::High,
            LeakageVector::OutputChannel,
            "Tainted data in output".to_string(),
        ));
        l.record(AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::Critical,
            LeakageVector::ToolCall,
            "Blocked tool call with secret".to_string(),
        ));
        l.record(AuditEvent::new(
            "sess-2".to_string(),
            AuditSeverity::Warning,
            LeakageVector::DangerousCommand,
            "Dangerous command detected".to_string(),
        ));
    }

    #[tokio::test]
    async fn test_list_events_empty() {
        let (app, _log) = make_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["events"].as_array().unwrap().len(), 0);
        assert_eq!(json["total"], 0);
    }

    #[tokio::test]
    async fn test_list_events_all() {
        let (app, log) = make_app().await;
        seed_events(&log).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/events")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["events"].as_array().unwrap().len(), 3);
        assert_eq!(json["total"], 3);
    }

    #[tokio::test]
    async fn test_list_events_by_session() {
        let (app, log) = make_app().await;
        seed_events(&log).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/events?session=sess-1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(resp).await;
        assert_eq!(json["events"].as_array().unwrap().len(), 2);
    }

    #[tokio::test]
    async fn test_list_events_by_severity() {
        let (app, log) = make_app().await;
        seed_events(&log).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/events?severity=critical")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(resp).await;
        assert_eq!(json["events"].as_array().unwrap().len(), 1);
        assert_eq!(json["events"][0]["severity"], "critical");
    }

    #[tokio::test]
    async fn test_get_event_not_found() {
        let (app, _log) = make_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/events/nonexistent")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn test_get_event_found() {
        let (app, log) = make_app().await;
        seed_events(&log).await;

        // Get the ID of the first event
        let event_id = {
            let l = log.read().await;
            l.recent(1)[0].id.clone()
        };

        let resp = app
            .oneshot(
                Request::builder()
                    .uri(format!("/api/v1/audit/events/{}", event_id))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["id"], event_id);
    }

    #[tokio::test]
    async fn test_stats_empty() {
        let (app, _log) = make_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["totalRecorded"], 0);
        assert_eq!(json["buffered"], 0);
    }

    #[tokio::test]
    async fn test_stats_with_events() {
        let (app, log) = make_app().await;
        seed_events(&log).await;

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(resp).await;
        assert_eq!(json["totalRecorded"], 3);
        assert_eq!(json["buffered"], 3);
        assert_eq!(json["bySeverity"]["high"], 1);
        assert_eq!(json["bySeverity"]["critical"], 1);
        assert_eq!(json["bySeverity"]["warning"], 1);
    }

    #[tokio::test]
    async fn test_alerts_endpoint_no_monitor() {
        let (app, _log) = make_app().await;
        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/alerts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total"], 0);
        assert!(json["alerts"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn test_alerts_endpoint_with_monitor() {
        use crate::audit::alerting::{AlertConfig, AlertMonitor};

        let log = Arc::new(RwLock::new(AuditLog::new(1000)));
        let monitor = Arc::new(AlertMonitor::new(AlertConfig::default()));

        // Feed a critical event to generate an alert
        let event = AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::Critical,
            LeakageVector::ToolCall,
            "critical tool call".to_string(),
        );
        monitor.process_event(&event).await;

        let state = AuditState {
            log,
            alert_monitor: Some(monitor),
            persistence: None,
        };
        let app = audit_router(state);

        let resp = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/audit/alerts")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["total"], 1);
        assert_eq!(json["alerts"].as_array().unwrap().len(), 1);
        assert_eq!(json["alerts"][0]["sessionId"], "sess-1");
    }
}
