//! TEE protocol types for secure communication with TEE environments.

use serde::{Deserialize, Serialize};

/// Message types for TEE communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TeeMessage {
    Request(TeeRequest),
    Response(TeeResponse),
    Heartbeat { timestamp: i64 },
    Error { code: i32, message: String },
}

/// Request to TEE environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeRequest {
    pub id: String,
    pub session_id: String,
    pub request_type: TeeRequestType,
    pub payload: Vec<u8>,
    pub timestamp: i64,
}

impl TeeRequest {
    pub fn new(session_id: String, request_type: TeeRequestType, payload: Vec<u8>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            request_type,
            payload,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Types of TEE requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeRequestType {
    InitSession,
    ProcessMessage,
    ExecuteTool,
    StoreSecret,
    RetrieveSecret,
    DeleteSecret,
    GetSessionState,
    TerminateSession,
}

/// Response from TEE environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeResponse {
    pub request_id: String,
    pub session_id: String,
    pub status: TeeResponseStatus,
    pub payload: Vec<u8>,
    pub timestamp: i64,
}

impl TeeResponse {
    pub fn success(request_id: String, session_id: String, payload: Vec<u8>) -> Self {
        Self {
            request_id,
            session_id,
            status: TeeResponseStatus::Success,
            payload,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn error(request_id: String, session_id: String, code: i32, message: String) -> Self {
        Self {
            request_id,
            session_id,
            status: TeeResponseStatus::Error { code, message },
            payload: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Response status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeResponseStatus {
    Success,
    Error { code: i32, message: String },
    Pending,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tee_request_creation() {
        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::ProcessMessage,
            vec![1, 2, 3],
        );
        assert!(!request.id.is_empty());
        assert_eq!(request.session_id, "session-123");
    }

    #[test]
    fn test_tee_message_serialization() {
        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::InitSession,
            vec![],
        );
        let message = TeeMessage::Request(request);
        let json = serde_json::to_string(&message).unwrap();
        let parsed: TeeMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TeeMessage::Request(_)));
    }

    #[test]
    fn test_tee_response_success() {
        let resp = TeeResponse::success("req-1".into(), "sess-1".into(), vec![42]);
        assert!(matches!(resp.status, TeeResponseStatus::Success));
        assert_eq!(resp.payload, vec![42]);
    }

    #[test]
    fn test_tee_response_error() {
        let resp = TeeResponse::error("req-1".into(), "sess-1".into(), 500, "fail".into());
        assert!(matches!(resp.status, TeeResponseStatus::Error { .. }));
        assert!(resp.payload.is_empty());
    }
}
