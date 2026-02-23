//! Agent message types for browser communication and session state
//!
//! Defines all message types exchanged between:
//! - Server → Browser (JSON via WebSocket)
//! - Browser → Server (JSON via WebSocket)
//! - Session state and process info types

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// =============================================================================
// Server → Browser messages (JSON)
// =============================================================================

/// Message sent to browser clients
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserIncomingMessage {
    SessionInit {
        session: AgentSessionState,
    },
    SessionUpdate {
        session: serde_json::Value,
    },
    Assistant {
        message: AssistantMessageBody,
        parent_tool_use_id: Option<String>,
    },
    StreamEvent {
        event: serde_json::Value,
        parent_tool_use_id: Option<String>,
    },
    Result {
        data: serde_json::Value,
    },
    PermissionRequest {
        request: PermissionRequest,
    },
    PermissionCancelled {
        request_id: String,
    },
    ToolProgress {
        tool_use_id: String,
        tool_name: String,
        elapsed_time_seconds: f64,
    },
    ToolUseSummary {
        summary: String,
        tool_use_ids: Vec<String>,
    },
    StatusChange {
        status: Option<String>,
    },
    Error {
        message: String,
    },
    UserMessage {
        content: String,
        timestamp: u64,
    },
    MessageHistory {
        messages: Vec<BrowserIncomingMessage>,
    },
    SessionNameUpdate {
        name: String,
    },
    /// Incoming agent-to-agent message (display notification or auto-execute)
    AgentMessage {
        message_id: String,
        from_session_id: String,
        topic: String,
        content: String,
        auto_execute: bool,
    },
    /// Slash command response (e.g. /help, /cost, /tools)
    CommandResponse {
        command: String,
        text: String,
        /// Whether the command modified session state (e.g. /clear, /compact)
        state_changed: bool,
    },
}

// =============================================================================
// Browser → Server messages (JSON)
// =============================================================================

/// Message received from browser clients
#[derive(Debug, Clone, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum BrowserOutgoingMessage {
    UserMessage {
        content: String,
        session_id: Option<String>,
        images: Option<Vec<ImageAttachment>>,
    },
    PermissionResponse {
        request_id: String,
        behavior: String,
        updated_input: Option<serde_json::Value>,
        updated_permissions: Option<Vec<serde_json::Value>>,
        message: Option<String>,
    },
    Interrupt,
    SetModel {
        model: String,
    },
    SetPermissionMode {
        mode: String,
    },
    /// Send a message to another agent session via the event bus
    SendAgentMessage {
        /// Target: "broadcast:<topic>" or "mention:<session_id>"
        target: String,
        content: String,
    },
    /// Toggle auto-execute mode for incoming agent messages
    SetAutoExecute {
        enabled: bool,
    },
}

/// Base64 image attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageAttachment {
    pub media_type: String,
    pub data: String,
}

// =============================================================================
// Assistant message types (used in browser messages and history)
// =============================================================================

/// Body of an assistant message
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssistantMessageBody {
    pub id: String,
    #[serde(rename = "type", default)]
    pub msg_type: Option<String>,
    pub role: String,
    pub model: String,
    pub content: Vec<ContentBlock>,
    pub stop_reason: Option<String>,
    #[serde(default)]
    pub usage: Option<serde_json::Value>,
}

/// Content block within an assistant message
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ContentBlock {
    Text {
        text: String,
    },
    ToolUse {
        id: String,
        name: String,
        input: serde_json::Value,
    },
    ToolResult {
        tool_use_id: String,
        content: serde_json::Value,
        #[serde(default)]
        is_error: bool,
    },
    Thinking {
        thinking: String,
        budget_tokens: Option<u32>,
    },
}

// =============================================================================
// Session state types
// =============================================================================

/// Agent session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSessionState {
    pub session_id: String,
    pub model: String,
    pub cwd: String,
    pub tools: Vec<String>,
    pub permission_mode: String,
    pub total_cost_usd: f64,
    pub num_turns: u32,
    pub context_used_percent: f64,
    pub is_compacting: bool,
    /// Bound persona ID (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
}

impl AgentSessionState {
    /// Create a new empty session state
    pub fn new(session_id: String) -> Self {
        Self {
            session_id,
            model: String::new(),
            cwd: String::new(),
            tools: Vec::new(),
            permission_mode: "default".to_string(),
            total_cost_usd: 0.0,
            num_turns: 0,
            context_used_percent: 0.0,
            is_compacting: false,
            persona_id: None,
        }
    }
}

/// Agent process info (metadata about a session)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentProcessInfo {
    pub session_id: String,
    pub pid: Option<u32>,
    pub state: AgentProcessState,
    pub exit_code: Option<i32>,
    pub model: Option<String>,
    pub permission_mode: Option<String>,
    pub cwd: String,
    pub created_at: u64,
    /// Kept for API compatibility; always None in engine mode
    pub cli_session_id: Option<String>,
    pub archived: bool,
    pub name: Option<String>,
    /// Bound persona ID (if any)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub persona_id: Option<String>,
}

/// Agent process lifecycle state
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentProcessState {
    /// Session created, ready for requests
    Connected,
    /// Actively generating a response
    Running,
    /// Session terminated
    Exited,
}

/// Permission request from the agent
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PermissionRequest {
    pub request_id: String,
    pub tool_name: String,
    pub input: serde_json::Value,
    pub permission_suggestions: Option<Vec<serde_json::Value>>,
    pub description: Option<String>,
    pub tool_use_id: Option<String>,
    pub agent_id: Option<String>,
    pub timestamp: u64,
}

/// Persisted agent session (for disk storage)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedAgentSession {
    pub id: String,
    pub state: AgentSessionState,
    pub message_history: Vec<BrowserIncomingMessage>,
    pub pending_messages: Vec<String>,
    pub pending_permissions: HashMap<String, PermissionRequest>,
    pub archived: bool,
}

/// Persona info for the personas API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersonaInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub tags: Vec<String>,
    pub version: Option<String>,
}

/// Session statistics for the stats API
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionStats {
    pub active: u32,
    pub archived: u32,
    pub with_browsers: u32,
    pub generating: u32,
    pub total: u32,
    pub total_cost_usd: f64,
}

/// Agent-to-agent message type for structured collaboration
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentMessageType {
    /// Normal conversation
    Chat,
    /// Task request (expects a response)
    TaskRequest,
    /// Response to a task request
    TaskResponse,
}

impl Default for AgentMessageType {
    fn default() -> Self {
        Self::Chat
    }
}

/// Entry in the agent directory (discoverable agents)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentDirectoryEntry {
    pub session_id: String,
    pub persona_id: Option<String>,
    pub persona_name: Option<String>,
    pub status: String,
    pub auto_execute: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_browser_incoming_message_serialization() {
        let msg = BrowserIncomingMessage::Error {
            message: "test error".to_string(),
        };
        let json = serde_json::to_string(&msg).unwrap();
        assert!(json.contains("test error"));
        assert!(json.contains("\"type\":\"error\""));
    }

    #[test]
    fn test_browser_outgoing_message_deserialization() {
        let json = r#"{"type":"user_message","content":"hello"}"#;
        let msg: BrowserOutgoingMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, BrowserOutgoingMessage::UserMessage { .. }));

        let json = r#"{"type":"interrupt"}"#;
        let msg: BrowserOutgoingMessage = serde_json::from_str(json).unwrap();
        assert!(matches!(msg, BrowserOutgoingMessage::Interrupt));
    }

    #[test]
    fn test_agent_session_state_new() {
        let state = AgentSessionState::new("test-id".to_string());
        assert_eq!(state.session_id, "test-id");
        assert_eq!(state.total_cost_usd, 0.0);
        assert_eq!(state.num_turns, 0);
        assert!(!state.is_compacting);
    }

    #[test]
    fn test_content_block_serialization() {
        let block = ContentBlock::ToolUse {
            id: "tu1".to_string(),
            name: "Bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
        };
        let json = serde_json::to_string(&block).unwrap();
        assert!(json.contains("tool_use"));
        assert!(json.contains("Bash"));

        let parsed: ContentBlock = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, ContentBlock::ToolUse { .. }));
    }

    #[test]
    fn test_permission_request_serialization() {
        let req = PermissionRequest {
            request_id: "r1".to_string(),
            tool_name: "Bash".to_string(),
            input: serde_json::json!({"command": "ls"}),
            permission_suggestions: None,
            description: Some("Run ls".to_string()),
            tool_use_id: Some("tu1".to_string()),
            agent_id: None,
            timestamp: 1700000000,
        };
        let json = serde_json::to_string(&req).unwrap();
        let parsed: PermissionRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.request_id, "r1");
        assert_eq!(parsed.tool_name, "Bash");
    }

    #[test]
    fn test_agent_process_state_serialization() {
        let state = AgentProcessState::Running;
        let json = serde_json::to_string(&state).unwrap();
        assert_eq!(json, "\"running\"");

        let parsed: AgentProcessState = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed, AgentProcessState::Running);
    }
}
