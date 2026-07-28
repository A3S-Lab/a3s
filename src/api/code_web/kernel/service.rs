use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Instant;

use a3s_boot::{BootError, BootResponse, Result as BootResult, SseEvent};
use a3s_code_core::store::SessionData;
use a3s_code_core::{
    AgentEvent, AgentSession, ContentBlock, Message, PlanningMode, TokenUsage, ToolErrorKind,
};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::{timeout, Duration};

use super::controls::{
    compose_controlled_prompt, controls_json, effort_levels_json, normalize_effort, normalize_goal,
    CodeWebContextUsage,
};
use super::sleep::{parse_sleep_report, sleep_directive, sleep_today, store_sleep_memories};
use super::turn_queue::{
    CodeWebQueuedTurn, CodeWebQueuedTurnKind, CodeWebQueuedTurnMode, CodeWebSessionTurnQueue,
    CodeWebStoredTurnQueue, GOAL_CONTINUATION_PRIORITY, USER_TURN_PRIORITY,
};
use crate::api::code_web::dto::{
    ChatRequest, ChatResponse, ConfirmToolUseRequest, CreateSessionRequest, ForkSessionRequest,
    KernelSessionResponse, SessionListResponse, SessionResponse, ShellSessionRequest,
    SleepSessionRequest, UsageResponse,
};
use crate::api::code_web::session_runtime::{
    activate_session_runtime, code_web_context_limit_for_model, code_web_os_status,
    code_web_session_options, effective_session_model, refresh_evolution_runtime_after_turn,
};
use crate::api::code_web::session_store::{
    CodeWebSessionMetadata, CodeWebStoredContext, CodeWebStoredSession,
};
use crate::api::code_web::state::{
    CodeWebGoalRun, CodeWebGoalStatus, CodeWebSessionContext, CodeWebSessionControls,
    CodeWebSessionSettings, CodeWebState,
};

mod control_operations;
mod goal_runtime;
mod maintenance;
mod messages;
mod persistence;
mod remote_read;
mod research;
mod sessions;
mod shell_output;
mod streaming;
mod text;
mod turn_queue;

pub(in crate::api::code_web) use remote_read::{
    ManagedChildEvidence, ManagedChildStatus, ManagedGoalStatus, ManagedSessionEvidence,
    ManagedSessionReadPort,
};
#[cfg(test)]
pub(in crate::api::code_web) use remote_read::{ManagedGoalEvidence, ManagedQueueEvidence};

pub(in crate::api) struct KernelService {
    state: Arc<CodeWebState>,
}

const SHELL_COMMAND_TIMEOUT: Duration = Duration::from_secs(30 * 60);
const SHELL_OUTPUT_MAX_CHARS: usize = 128_000;

impl KernelService {
    pub(in crate::api) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) async fn list_agents(&self) -> Vec<serde_json::Value> {
        vec![json!({
            "id": "default",
            "name": "书小安",
            "description": "A3S Code local assistant",
            "tags": ["local", "a3s-code"],
        })]
    }
}

#[cfg(test)]
mod tests;
