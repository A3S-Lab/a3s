//! `SentinelHookHandler` — bridges the synchronous `HookHandler` trait to the
//! async `SentinelAgent::analyze()` via `tokio::task::block_in_place`.
//!
//! The handler is registered with the a3s-code `HookEngine` for three
//! blocking event types:
//!
//! - `PreToolUse`  — can prevent a tool from executing
//! - `PrePrompt`   — can rewrite or block the user prompt
//! - `PostResponse`— can redact sensitive content before returning to the user
//!
//! All other events are observed asynchronously through the `HookEngine`
//! event channel (fire-and-forget) and do not go through this handler.

use super::SentinelAgent;
use a3s_code::hooks::{HookEvent, HookHandler, HookResponse};
use std::sync::Arc;

/// Hook handler that delegates to `SentinelAgent` for blocking analysis.
pub struct SentinelHookHandler {
    agent: Arc<SentinelAgent>,
}

impl SentinelHookHandler {
    pub fn new(agent: Arc<SentinelAgent>) -> Self {
        Self { agent }
    }
}

impl HookHandler for SentinelHookHandler {
    fn handle(&self, event: &HookEvent) -> HookResponse {
        match event {
            HookEvent::PreToolUse(e) => {
                let verdict = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(self.agent.analyze_pre_tool_use(&e.session_id, &e.tool, &e.args))
                });
                if verdict.should_block {
                    HookResponse::block(verdict.reason.as_deref().unwrap_or("Blocked by sentinel"))
                } else {
                    HookResponse::continue_()
                }
            }

            HookEvent::PrePrompt(e) => {
                let verdict = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(self.agent.analyze_prompt(&e.session_id, &e.prompt))
                });
                if verdict.should_block {
                    HookResponse::block(verdict.reason.as_deref().unwrap_or("Blocked by sentinel"))
                } else {
                    HookResponse::continue_()
                }
            }

            HookEvent::PostResponse(e) => {
                let verdict = tokio::task::block_in_place(|| {
                    tokio::runtime::Handle::current()
                        .block_on(self.agent.analyze_response(&e.response_text))
                });
                if verdict.should_block {
                    HookResponse::block(verdict.reason.as_deref().unwrap_or("Blocked by sentinel"))
                } else if let Some(sanitized) = verdict.sanitized_content {
                    HookResponse::continue_with(serde_json::json!({ "response_text": sanitized }))
                } else {
                    HookResponse::continue_()
                }
            }

            // Other events are handled asynchronously via event channel — pass through.
            _ => HookResponse::continue_(),
        }
    }
}
