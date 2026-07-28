use a3s_code_core::{AgentSession, TokenUsage};
use serde::{Deserialize, Serialize};

use super::session_store::CodeWebSessionMetadata;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CreateSessionRequest {
    pub workspace: Option<String>,
    pub cwd: Option<String>,
    pub model: Option<String>,
    pub follow_default_model: Option<bool>,
    pub permission_mode: Option<String>,
    pub planning_mode: Option<String>,
    pub goal_tracking: Option<bool>,
    pub title: Option<String>,
    pub agent_id: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct ChatRequest {
    pub session_id: Option<String>,
    pub workspace: Option<String>,
    pub model: Option<String>,
    pub message: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct SleepSessionRequest {
    pub focus: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct ForkSessionRequest {
    pub focus: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct ShellSessionRequest {
    pub command: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct ConfirmToolUseRequest {
    pub approved: bool,
    pub reason: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct HealthResponse {
    pub schema_version: u32,
    pub ok: bool,
    pub service: String,
    pub app: String,
    pub version: String,
    pub pid: u32,
    pub config_path: String,
    pub workspace: String,
    pub model: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct SessionResponse {
    session_id: String,
    workspace: String,
    cwd: String,
    model: Option<String>,
    follow_default_model: bool,
    permission_mode: String,
    state: String,
    title: Option<String>,
    agent_id: Option<String>,
    created_at: i64,
}

impl SessionResponse {
    pub(in crate::api::code_web) fn from_session(
        session: &AgentSession,
        model: Option<String>,
        follow_default_model: bool,
        permission_mode: String,
        metadata: &CodeWebSessionMetadata,
    ) -> Self {
        let workspace = session.workspace().display().to_string();
        Self {
            session_id: session.session_id().to_string(),
            workspace: workspace.clone(),
            cwd: workspace,
            model,
            follow_default_model,
            permission_mode,
            state: "connected".to_string(),
            title: metadata.title.clone(),
            agent_id: Some(
                metadata
                    .agent_id
                    .clone()
                    .unwrap_or_else(|| "default".to_string()),
            ),
            created_at: if metadata.created_at > 0 {
                metadata.created_at
            } else {
                chrono::Utc::now().timestamp_millis()
            },
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct KernelSessionResponse {
    pub success: bool,
    pub session: SessionResponse,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct SessionListResponse {
    pub items: Vec<SessionResponse>,
    pub total: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct ChatResponse {
    pub session_id: String,
    pub workspace: String,
    pub model: Option<String>,
    pub text: String,
    pub usage: UsageResponse,
    pub tool_calls_count: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct UsageResponse {
    prompt_tokens: usize,
    completion_tokens: usize,
    total_tokens: usize,
}

impl UsageResponse {
    pub(in crate::api::code_web) fn from_usage(usage: TokenUsage) -> Self {
        Self {
            prompt_tokens: usage.prompt_tokens,
            completion_tokens: usage.completion_tokens,
            total_tokens: usage.total_tokens,
        }
    }
}
