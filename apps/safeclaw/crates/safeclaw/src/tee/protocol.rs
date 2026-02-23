//! TEE communication protocol
//!
//! Re-exports shared protocol types from a3s-transport and defines
//! SafeClaw-specific payload structures.

// Re-export shared protocol types from a3s-common (transport module)
pub use a3s_common::{TeeMessage, TeeRequest, TeeRequestType, TeeResponse, TeeResponseStatus};

use serde::{Deserialize, Serialize};

/// Payload for InitSession request
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InitSessionPayload {
    /// User identifier
    pub user_id: String,
    /// Channel identifier
    pub channel_id: String,
    /// Model configuration
    pub model_config: ModelConfigPayload,
    /// Session options
    pub options: SessionOptions,
}

/// Model configuration for TEE session
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelConfigPayload {
    /// Provider name
    pub provider: String,
    /// Model name
    pub model: String,
    /// API key (encrypted)
    pub api_key_encrypted: Vec<u8>,
}

/// Session options
#[allow(dead_code)]
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SessionOptions {
    /// Enable conversation history
    pub enable_history: bool,
    /// Maximum history length
    pub max_history: usize,
    /// Enable tool execution
    pub enable_tools: bool,
    /// Allowed tools
    pub allowed_tools: Vec<String>,
}

/// Payload for ProcessMessage request
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProcessMessagePayload {
    /// User message content
    pub content: String,
    /// Message role
    pub role: String,
    /// Attachments (encrypted)
    pub attachments: Vec<AttachmentPayload>,
}

/// Attachment in a message
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AttachmentPayload {
    /// Attachment type
    pub attachment_type: String,
    /// Attachment data (encrypted)
    pub data: Vec<u8>,
    /// Metadata
    pub metadata: serde_json::Value,
}

/// Payload for tool execution
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecuteToolPayload {
    /// Tool name
    pub tool_name: String,
    /// Tool arguments
    pub arguments: serde_json::Value,
    /// Execution context
    pub context: serde_json::Value,
}

/// Payload for secret storage
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StoreSecretPayload {
    /// Secret key/name
    pub key: String,
    /// Secret value (encrypted)
    pub value: Vec<u8>,
    /// Expiration timestamp (optional)
    pub expires_at: Option<i64>,
}

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

    #[test]
    fn test_safeclaw_payload_serialization() {
        let payload = InitSessionPayload {
            user_id: "user-1".to_string(),
            channel_id: "channel-1".to_string(),
            model_config: ModelConfigPayload {
                provider: "openai".to_string(),
                model: "gpt-4".to_string(),
                api_key_encrypted: vec![1, 2, 3],
            },
            options: SessionOptions {
                enable_history: true,
                max_history: 100,
                enable_tools: true,
                allowed_tools: vec!["search".to_string()],
            },
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: InitSessionPayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.user_id, "user-1");
        assert_eq!(parsed.model_config.provider, "openai");
    }

    #[test]
    fn test_process_message_payload_serialization() {
        let payload = ProcessMessagePayload {
            content: "Hello".to_string(),
            role: "user".to_string(),
            attachments: vec![AttachmentPayload {
                attachment_type: "image".to_string(),
                data: vec![1, 2, 3],
                metadata: serde_json::json!({"size": 1024}),
            }],
        };

        let json = serde_json::to_string(&payload).unwrap();
        let parsed: ProcessMessagePayload = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.content, "Hello");
        assert_eq!(parsed.attachments.len(), 1);
    }
}
