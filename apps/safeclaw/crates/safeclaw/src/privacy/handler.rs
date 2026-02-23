//! HTTP handlers for the Privacy API
//!
//! Provides REST endpoints for privacy classification and semantic analysis:
//! - POST /api/v1/privacy/classify   — regex-based classification
//! - POST /api/v1/privacy/analyze    — semantic PII disclosure detection
//! - POST /api/v1/privacy/scan       — combined scan (regex + semantic)

use crate::config::SensitivityLevel;
use crate::privacy::classifier::Classifier;
use crate::privacy::semantic::{SemanticAnalyzer, SemanticMatch};
use axum::{extract::State, response::IntoResponse, routing::post, Json, Router};
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// Shared state for privacy handlers
#[derive(Clone)]
pub struct PrivacyState {
    pub classifier: Arc<Classifier>,
    pub semantic: Arc<SemanticAnalyzer>,
}

/// Create the privacy router
pub fn privacy_router(state: PrivacyState) -> Router {
    Router::new()
        .route("/api/v1/privacy/classify", post(classify))
        .route("/api/v1/privacy/analyze", post(analyze))
        .route("/api/v1/privacy/scan", post(scan))
        .with_state(state)
}

// =============================================================================
// Request / Response types
// =============================================================================

/// Request body for classification endpoints
#[derive(Debug, Deserialize)]
pub struct ClassifyRequest {
    pub text: String,
}

/// Response from regex classification
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyResponse {
    pub level: String,
    pub requires_tee: bool,
    pub matches: Vec<ClassifyMatch>,
}

/// A single regex classification match
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClassifyMatch {
    pub rule: String,
    pub level: String,
    pub start: usize,
    pub end: usize,
    pub redacted: String,
}

/// Response from semantic analysis
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnalyzeResponse {
    pub level: String,
    pub requires_tee: bool,
    pub matches: Vec<SemanticMatchResponse>,
}

/// A single semantic match in the response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SemanticMatchResponse {
    pub category: String,
    pub trigger: String,
    pub redacted_value: String,
    pub level: String,
    pub confidence: f64,
    pub start: usize,
    pub end: usize,
}

impl From<&SemanticMatch> for SemanticMatchResponse {
    fn from(m: &SemanticMatch) -> Self {
        Self {
            category: format!("{:?}", m.category),
            trigger: m.trigger.clone(),
            redacted_value: m.redacted_value.clone(),
            level: format!("{:?}", m.level),
            confidence: m.confidence,
            start: m.start,
            end: m.end,
        }
    }
}

/// Combined scan response
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanResponse {
    pub level: String,
    pub requires_tee: bool,
    pub regex_matches: Vec<ClassifyMatch>,
    pub semantic_matches: Vec<SemanticMatchResponse>,
}

// =============================================================================
// Helpers
// =============================================================================

/// Convert SensitivityLevel to a numeric rank for comparison
fn level_rank(level: SensitivityLevel) -> u8 {
    match level {
        SensitivityLevel::Public => 0,
        SensitivityLevel::Normal => 1,
        SensitivityLevel::Sensitive => 2,
        SensitivityLevel::HighlySensitive => 3,
        SensitivityLevel::Critical => 4,
    }
}

// =============================================================================
// Handlers
// =============================================================================

/// POST /api/v1/privacy/classify — regex-based classification
async fn classify(
    State(state): State<PrivacyState>,
    Json(request): Json<ClassifyRequest>,
) -> impl IntoResponse {
    let result = state.classifier.classify(&request.text);

    let matches = result
        .matches
        .iter()
        .map(|m| ClassifyMatch {
            rule: m.rule_name.clone(),
            level: format!("{:?}", m.level),
            start: m.start,
            end: m.end,
            redacted: m.redacted.clone(),
        })
        .collect();

    Json(ClassifyResponse {
        level: format!("{:?}", result.level),
        requires_tee: level_rank(result.level) >= level_rank(SensitivityLevel::Sensitive),
        matches,
    })
}

/// POST /api/v1/privacy/analyze — semantic PII disclosure detection
async fn analyze(
    State(state): State<PrivacyState>,
    Json(request): Json<ClassifyRequest>,
) -> impl IntoResponse {
    let result = state.semantic.analyze(&request.text);

    Json(AnalyzeResponse {
        level: format!("{:?}", result.level),
        requires_tee: result.requires_tee,
        matches: result
            .matches
            .iter()
            .map(SemanticMatchResponse::from)
            .collect(),
    })
}

/// POST /api/v1/privacy/scan — combined scan (regex + semantic)
async fn scan(
    State(state): State<PrivacyState>,
    Json(request): Json<ClassifyRequest>,
) -> impl IntoResponse {
    let regex_result = state.classifier.classify(&request.text);
    let semantic_result = state.semantic.analyze(&request.text);

    let mut max_level = regex_result.level;
    if level_rank(semantic_result.level) > level_rank(max_level) {
        max_level = semantic_result.level;
    }

    let requires_tee = level_rank(max_level) >= level_rank(SensitivityLevel::Sensitive);

    Json(ScanResponse {
        level: format!("{:?}", max_level),
        requires_tee,
        regex_matches: regex_result
            .matches
            .iter()
            .map(|m| ClassifyMatch {
                rule: m.rule_name.clone(),
                level: format!("{:?}", m.level),
                start: m.start,
                end: m.end,
                redacted: m.redacted.clone(),
            })
            .collect(),
        semantic_matches: semantic_result
            .matches
            .iter()
            .map(SemanticMatchResponse::from)
            .collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    fn make_state() -> PrivacyState {
        let classifier = Arc::new(Classifier::new(vec![], SensitivityLevel::Normal).unwrap());
        let semantic = Arc::new(SemanticAnalyzer::new());
        PrivacyState {
            classifier,
            semantic,
        }
    }

    fn make_app() -> Router {
        privacy_router(make_state())
    }

    async fn body_json(response: axum::response::Response) -> serde_json::Value {
        let body = axum::body::to_bytes(response.into_body(), 1024 * 64)
            .await
            .unwrap();
        serde_json::from_slice(&body).unwrap()
    }

    #[tokio::test]
    async fn test_classify_normal_text() {
        let app = make_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/privacy/classify")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"hello world"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["level"], "Normal");
        assert_eq!(json["requiresTee"], false);
        assert_eq!(json["matches"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_analyze_password_disclosure() {
        let app = make_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/privacy/analyze")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"my password is hunter2"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["requiresTee"], true);
        let matches = json["matches"].as_array().unwrap();
        assert!(!matches.is_empty());
        assert_eq!(matches[0]["category"], "Password");
    }

    #[tokio::test]
    async fn test_analyze_clean_text() {
        let app = make_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/privacy/analyze")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"the weather is nice today"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        let json = body_json(resp).await;
        assert_eq!(json["level"], "Normal");
        assert_eq!(json["matches"].as_array().unwrap().len(), 0);
    }

    #[tokio::test]
    async fn test_scan_combined() {
        let app = make_app();
        let resp = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/privacy/scan")
                    .header("content-type", "application/json")
                    .body(Body::from(r#"{"text":"my password is hunter2"}"#))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(resp.status(), StatusCode::OK);
        let json = body_json(resp).await;
        assert_eq!(json["requiresTee"], true);
        assert!(!json["semanticMatches"].as_array().unwrap().is_empty());
    }
}
