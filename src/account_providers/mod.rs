//! Account-backed model providers discovered from local developer tools.
//!
//! This module is the single integration boundary for external account state.
//! The TUI does not read provider credentials directly: it asks this registry
//! whether an account is available, discovers that account's models, and asks
//! for an `LlmClient` when a model is selected.

mod claude;
mod cli_transport;
mod codebuddy;
pub(crate) mod codex;
mod host_tools;
mod kimi;
mod protocol;

use a3s_code_core::llm::LlmClient;
use anyhow::Result;
use std::path::PathBuf;
use std::sync::Arc;

/// A local developer-tool account that can back an A3S Code session.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum AccountProvider {
    Claude,
    Codex,
    Kimi,
    CodeBuddy,
}

impl AccountProvider {
    pub(crate) const ALL: [Self; 4] = [Self::Claude, Self::Codex, Self::Kimi, Self::CodeBuddy];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Claude => "Claude Code",
            Self::Codex => "Codex",
            Self::Kimi => "Kimi",
            Self::CodeBuddy => "WorkBuddy",
        }
    }

    /// Whether the corresponding local application has reusable account state.
    pub(crate) fn is_available(self) -> bool {
        match self {
            Self::Claude => claude::has_claude_login(),
            Self::Codex => codex::has_codex_login(),
            Self::Kimi => kimi::has_kimi_login(),
            Self::CodeBuddy => codebuddy::has_workbuddy_login(),
        }
    }

    /// Synchronous models suitable for the first `/model` render.
    ///
    /// CodeBuddy account entitlements require a CLI round trip, so it returns a
    /// compatibility list here and replaces it with `discover_models()` once
    /// its tab is opened.
    pub(crate) fn local_models(self) -> Vec<String> {
        match self {
            Self::Claude => claude::models(),
            Self::Codex => codex::cached_codex_models()
                .into_iter()
                .map(|model| model.slug)
                .collect(),
            Self::Kimi => kimi::fallback_models(),
            Self::CodeBuddy => codebuddy::fallback_models(),
        }
    }

    /// Discover the models currently usable by this local account.
    pub(crate) async fn discover_models(self) -> Result<Vec<String>> {
        match self {
            Self::Claude => Ok(self.local_models()),
            Self::Codex => Ok(codex::refresh_codex_models()
                .await?
                .into_iter()
                .map(|model| model.slug)
                .collect()),
            Self::Kimi => kimi::discover_models().await,
            Self::CodeBuddy => codebuddy::discover_models().await,
        }
    }

    pub(crate) fn canonical_model(self, model: &str) -> String {
        match self {
            Self::Claude => claude::canonical_model_name(model),
            Self::Codex | Self::Kimi | Self::CodeBuddy => model.trim().to_string(),
        }
    }

    /// Build the provider-specific client without exposing credentials to TUI
    /// code. `session_id` is used only by the Codex Responses transport.
    pub(crate) fn client(self, model: &str, session_id: &str) -> Result<Arc<dyn LlmClient>> {
        let model = self.canonical_model(model);
        match self {
            Self::Claude => Ok(Arc::new(claude::ClaudeClient::from_claude_login(&model)?)),
            Self::Codex => Ok(Arc::new(codex::CodexClient::from_codex_login(
                &model, session_id,
            )?)),
            Self::Kimi => Ok(Arc::new(kimi::KimiClient::from_kimi_login(&model)?)),
            Self::CodeBuddy => Ok(Arc::new(codebuddy::CodeBuddyClient::from_workbuddy_login(
                &model,
            )?)),
        }
    }

    pub(crate) fn model_context(self, model: &str) -> Option<u32> {
        match self {
            Self::Codex => codex::codex_model_context(model),
            Self::Kimi => kimi::model_context(model),
            Self::Claude | Self::CodeBuddy => None,
        }
    }

    /// Root directory owned by the corresponding developer tool. Relay reads
    /// only its project transcripts; authentication remains provider-owned.
    pub(crate) fn history_root(self) -> Option<PathBuf> {
        match self {
            Self::Claude => claude::claude_config_dir(),
            Self::Codex => codex::codex_home(),
            Self::Kimi => kimi::kimi_home(),
            Self::CodeBuddy => codebuddy::workbuddy_config_dir(),
        }
    }
}
