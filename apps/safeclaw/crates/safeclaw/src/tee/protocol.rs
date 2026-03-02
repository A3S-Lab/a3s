//! TEE communication protocol
//!
//! Re-exports shared protocol types from a3s-transport.

// Re-export shared protocol types from a3s-common (transport module)
pub use a3s_common::{TeeMessage, TeeRequest, TeeRequestType, TeeResponse, TeeResponseStatus};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_creation() {
        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::ProcessMessage,
            vec![1, 2, 3],
        );

        assert!(!request.id.is_empty());
        assert_eq!(request.session_id, "session-123");
        assert!(request.timestamp > 0);
    }

    #[test]
    fn test_response_success() {
        let response = TeeResponse::success(
            "req-123".to_string(),
            "session-123".to_string(),
            vec![4, 5, 6],
        );

        assert_eq!(response.request_id, "req-123");
        assert!(matches!(response.status, TeeResponseStatus::Success));
    }

    #[test]
    fn test_response_error() {
        let response = TeeResponse::error(
            "req-123".to_string(),
            "session-123".to_string(),
            500,
            "Internal error".to_string(),
        );

        assert!(matches!(
            response.status,
            TeeResponseStatus::Error { code: 500, .. }
        ));
    }

    #[test]
    fn test_message_serialization() {
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
}
