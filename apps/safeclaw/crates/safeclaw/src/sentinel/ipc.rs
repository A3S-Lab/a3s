//! IPC protocol for sentinel daemon communication.
//!
//! Transport: newline-delimited JSON over a Unix Domain Socket.
//! Each request is one JSON line; the server replies with one JSON line for
//! analysis requests and sends no reply for fire-and-forget notifications.

use serde::{Deserialize, Serialize};

// ── Request ───────────────────────────────────────────────────────────────────

/// Request sent from the main process to the sentinel daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SentinelRequest {
    AnalyzeToolUse { session_id: String, tool: String, args: serde_json::Value },
    AnalyzePrompt { session_id: String, prompt: String },
    AnalyzeResponse { response: String },
    /// Fire-and-forget: no reply expected.
    SessionCreated { session_id: String, parent_id: Option<String> },
    /// Fire-and-forget: no reply expected.
    SessionDestroyed { session_id: String },
    Ping,
}

impl SentinelRequest {
    /// Returns `true` when the daemon must send a response line.
    pub fn expects_response(&self) -> bool {
        !matches!(
            self,
            SentinelRequest::SessionCreated { .. } | SentinelRequest::SessionDestroyed { .. }
        )
    }
}

// ── Response ──────────────────────────────────────────────────────────────────

/// Response from the sentinel daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SentinelResponse {
    pub should_block: bool,
    pub reason: Option<String>,
}

impl SentinelResponse {
    pub fn allow() -> Self {
        Self { should_block: false, reason: None }
    }

    pub fn block(reason: impl Into<String>) -> Self {
        Self { should_block: true, reason: Some(reason.into()) }
    }
}
