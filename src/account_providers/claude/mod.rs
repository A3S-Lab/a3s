//! Claude Code account-backed `LlmClient`.
//!
//! The public surface is intentionally small: the TUI asks for a model and gets
//! an `LlmClient`. Internally, the client can use either the raw Anthropic
//! Messages API with Claude Code OAuth credentials or the installed `claude`
//! CLI stream-json transport when the raw OAuth bridge is rejected.

pub(crate) mod code_cli;
mod credentials;
mod model;
mod raw_messages;

use a3s_code_core::llm::{
    default_http_client, HttpClient, LlmClient, LlmResponse, Message, StreamEvent, ToolDefinition,
};
use anyhow::{anyhow, Context, Result};
use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

use code_cli::ClaudeCodeCliAdapter;
use credentials::ClaudeCredentials;
use raw_messages::RawMessagesClient;

pub(crate) use credentials::{claude_config_dir, has_claude_login};
pub(crate) use model::{canonical_model_name, models};

static PREFER_CLAUDE_CLI_TRANSPORT: AtomicBool = AtomicBool::new(false);

pub struct ClaudeClient {
    raw_messages: RawMessagesClient,
    code_cli: ClaudeCodeCliAdapter,
}

impl ClaudeClient {
    pub fn from_claude_login(model: &str) -> Result<Self> {
        Self::from_claude_login_with_http(model, default_http_client())
    }

    fn from_claude_login_with_http(model: &str, http: Arc<dyn HttpClient>) -> Result<Self> {
        let model = canonical_model_name(model);
        let credentials = ClaudeCredentials::from_disk()?;
        Ok(Self {
            raw_messages: RawMessagesClient::new(credentials.access_token, &model, http),
            code_cli: ClaudeCodeCliAdapter::new(&model),
        })
    }
}

#[async_trait]
impl LlmClient for ClaudeClient {
    async fn complete(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> Result<LlmResponse> {
        let mut rx = self
            .complete_streaming(messages, system, tools, CancellationToken::new())
            .await?;
        while let Some(event) = rx.recv().await {
            if let StreamEvent::Done(response) = event {
                return Ok(response);
            }
        }
        Err(anyhow!("claude stream closed before message_stop"))
    }

    async fn complete_streaming(
        &self,
        messages: &[Message],
        system: Option<&str>,
        tools: &[ToolDefinition],
        cancel_token: CancellationToken,
    ) -> Result<mpsc::Receiver<StreamEvent>> {
        if PREFER_CLAUDE_CLI_TRANSPORT.load(Ordering::Relaxed) {
            return self
                .code_cli
                .complete_streaming(messages, system, tools, cancel_token)
                .await;
        }

        match self
            .raw_messages
            .complete_streaming(messages, system, tools, cancel_token.clone())
            .await
        {
            Ok(rx) => Ok(rx),
            Err(error) if error.should_use_cli_fallback() => {
                PREFER_CLAUDE_CLI_TRANSPORT.store(true, Ordering::Relaxed);
                self.code_cli
                    .complete_streaming(messages, system, tools, cancel_token)
                    .await
                    .with_context(|| format!("{error}; Claude Code CLI adapter also failed"))
            }
            Err(error) => Err(error.into()),
        }
    }
}
