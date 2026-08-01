use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};

use a3s_code_core::{
    Agent, AgentSession, CodeConfig, LlmClient, LocalWorkspaceManifestSnapshot, WorkspaceServices,
};
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use tokio_util::sync::CancellationToken;

use super::kernel::turn_queue::CodeWebSessionTurnQueue;
use super::previews::PreviewRegistry;
use super::session_store::{CodeWebSessionMetadata, CodeWebSessionRepository};
use super::workspace_backend_cache::WorkspaceBackendCache;
use crate::budget::DEFAULT_CODE_WEB_EFFORT_ID;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) enum CodeWebGoalStatus {
    Active,
    Paused,
    Retrying,
    Achieved,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebGoalRun {
    pub(in crate::api::code_web) status: CodeWebGoalStatus,
    pub(in crate::api::code_web) started_at: i64,
    pub(in crate::api::code_web) updated_at: i64,
    pub(in crate::api::code_web) completed_at: Option<i64>,
    pub(in crate::api::code_web) attempts: u32,
    pub(in crate::api::code_web) progress_percent: u8,
    pub(in crate::api::code_web) completed_steps: usize,
    pub(in crate::api::code_web) total_steps: usize,
    pub(in crate::api::code_web) last_error: Option<String>,
    pub(in crate::api::code_web) extracted_goal: Option<String>,
}

impl Default for CodeWebGoalRun {
    fn default() -> Self {
        Self {
            status: CodeWebGoalStatus::Active,
            started_at: 0,
            updated_at: 0,
            completed_at: None,
            attempts: 0,
            progress_percent: 0,
            completed_steps: 0,
            total_steps: 0,
            last_error: None,
            extracted_goal: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebSessionControls {
    pub(in crate::api::code_web) effort: String,
    pub(in crate::api::code_web) goal: Option<String>,
    pub(in crate::api::code_web) goal_run: Option<CodeWebGoalRun>,
}

impl Default for CodeWebSessionControls {
    fn default() -> Self {
        Self {
            effort: DEFAULT_CODE_WEB_EFFORT_ID.to_string(),
            goal: None,
            goal_run: None,
        }
    }
}

#[derive(Clone, Default)]
pub(in crate::api::code_web) struct CodeWebSessionContext {
    pub(in crate::api::code_web) compact_summary: Option<String>,
    pub(in crate::api::code_web) auto_compact:
        Option<crate::compact::auto_compact::AutoCompactController>,
    pub(in crate::api::code_web) llm_client: Option<Arc<dyn LlmClient>>,
}

impl CodeWebSessionContext {
    pub(in crate::api::code_web) fn set_llm_client(&mut self, client: Arc<dyn LlmClient>) {
        self.llm_client = Some(client);
    }

    pub(in crate::api::code_web) fn llm_client(&self) -> Option<Arc<dyn LlmClient>> {
        self.llm_client.clone()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebSessionSettings {
    pub(in crate::api::code_web) model: Option<String>,
    pub(in crate::api::code_web) follow_default_model: bool,
    pub(in crate::api::code_web) permission_mode: String,
    pub(in crate::api::code_web) planning_mode: Option<String>,
    pub(in crate::api::code_web) goal_tracking: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::api::code_web) enum CodeWebUseSetupPhase {
    Preparing,
    Ready,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::api::code_web) struct CodeWebUseSetupStatus {
    pub(in crate::api::code_web) phase: CodeWebUseSetupPhase,
    pub(in crate::api::code_web) message: Option<String>,
}

impl Default for CodeWebSessionSettings {
    fn default() -> Self {
        Self {
            model: None,
            follow_default_model: true,
            permission_mode: "default".to_string(),
            planning_mode: None,
            goal_tracking: None,
        }
    }
}

pub(in crate::api) struct CodeWebState {
    pub(in crate::api::code_web) agent: Arc<Agent>,
    pub(in crate::api::code_web) config_path: PathBuf,
    pub(in crate::api::code_web) auto_compact_threshold: f64,
    pub(in crate::api::code_web) default_workspace: PathBuf,
    pub(in crate::api::code_web) code_config: RwLock<CodeConfig>,
    pub(in crate::api::code_web) session_repository: Arc<CodeWebSessionRepository>,
    pub(in crate::api::code_web) sessions: Mutex<HashMap<String, Arc<AgentSession>>>,
    pub(in crate::api::code_web) messages: Mutex<HashMap<String, Vec<serde_json::Value>>>,
    pub(in crate::api::code_web) session_metadata: Mutex<HashMap<String, CodeWebSessionMetadata>>,
    pub(in crate::api::code_web) session_persist_lock: Mutex<()>,
    pub(in crate::api::code_web) evolution_refresh_lock: Mutex<()>,
    pub(in crate::api::code_web) workspace_file_write_lock: Mutex<()>,
    pub(in crate::api::code_web) session_controls: Mutex<HashMap<String, CodeWebSessionControls>>,
    pub(in crate::api::code_web) session_contexts: Mutex<HashMap<String, CodeWebSessionContext>>,
    pub(in crate::api::code_web) session_settings: Mutex<HashMap<String, CodeWebSessionSettings>>,
    pub(in crate::api::code_web) session_turn_queues:
        Mutex<HashMap<String, CodeWebSessionTurnQueue>>,
    pub(in crate::api::code_web) active_research_runs:
        Mutex<HashMap<String, Arc<CancellationToken>>>,
    use_registry: RwLock<Option<crate::use_registry::UseRegistryHandle>>,
    use_setup_status: RwLock<CodeWebUseSetupStatus>,
    workspace_backends: WorkspaceBackendCache,
    preview_registry: Arc<PreviewRegistry>,
}

impl CodeWebState {
    pub(in crate::api) fn new(
        agent: Arc<Agent>,
        config_path: PathBuf,
        default_workspace: PathBuf,
        code_config: CodeConfig,
        session_repository: Arc<CodeWebSessionRepository>,
    ) -> Self {
        let auto_compact_threshold = crate::config::auto_compact_threshold_for_path(&config_path);
        let preview_registry = Arc::new(PreviewRegistry::new(default_workspace.clone()));
        Self {
            agent,
            config_path,
            auto_compact_threshold,
            default_workspace,
            code_config: RwLock::new(code_config),
            session_repository,
            sessions: Mutex::new(HashMap::new()),
            messages: Mutex::new(HashMap::new()),
            session_metadata: Mutex::new(HashMap::new()),
            session_persist_lock: Mutex::new(()),
            evolution_refresh_lock: Mutex::new(()),
            workspace_file_write_lock: Mutex::new(()),
            session_controls: Mutex::new(HashMap::new()),
            session_contexts: Mutex::new(HashMap::new()),
            session_settings: Mutex::new(HashMap::new()),
            session_turn_queues: Mutex::new(HashMap::new()),
            active_research_runs: Mutex::new(HashMap::new()),
            use_registry: RwLock::new(None),
            use_setup_status: RwLock::new(CodeWebUseSetupStatus {
                phase: CodeWebUseSetupPhase::Preparing,
                message: None,
            }),
            workspace_backends: WorkspaceBackendCache::default(),
            preview_registry,
        }
    }

    pub(in crate::api) fn preview_registry(&self) -> Arc<PreviewRegistry> {
        Arc::clone(&self.preview_registry)
    }

    pub(in crate::api) async fn install_use_registry(
        &self,
        registry: crate::use_registry::UseRegistryHandle,
        warning: Option<String>,
    ) {
        *self
            .use_registry
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = Some(registry.clone());
        let sessions = self
            .sessions
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            registry.attach_session(session);
        }
        *self
            .use_setup_status
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = CodeWebUseSetupStatus {
            phase: CodeWebUseSetupPhase::Ready,
            message: warning,
        };
    }

    pub(in crate::api) fn set_use_setup_unavailable(&self, reason: impl Into<String>) {
        *self
            .use_setup_status
            .write()
            .unwrap_or_else(|poison| poison.into_inner()) = CodeWebUseSetupStatus {
            phase: CodeWebUseSetupPhase::Unavailable,
            message: Some(reason.into()),
        };
    }

    pub(in crate::api::code_web) fn use_setup_status(&self) -> CodeWebUseSetupStatus {
        self.use_setup_status
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    pub(in crate::api::code_web) fn use_registry(
        &self,
    ) -> Option<crate::use_registry::UseRegistryHandle> {
        self.use_registry
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone()
    }

    pub(in crate::api::code_web) fn attach_use_session(&self, session: Arc<AgentSession>) {
        let registry = self
            .use_registry
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        if let Some(registry) = registry {
            registry.attach_session(session);
        }
    }

    pub(in crate::api::code_web) async fn detach_use_session(&self, session_id: &str) {
        let registry = self
            .use_registry
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        if let Some(registry) = registry {
            registry.detach_session(session_id).await;
        }
    }

    pub(in crate::api::code_web) async fn workspace_services_for(
        &self,
        workspace: &std::path::Path,
    ) -> anyhow::Result<Arc<WorkspaceServices>> {
        self.workspace_backends.services_for(workspace).await
    }

    pub(in crate::api::code_web) async fn workspace_manifest_snapshot_for(
        &self,
        workspace: &std::path::Path,
    ) -> anyhow::Result<LocalWorkspaceManifestSnapshot> {
        self.workspace_backends
            .manifest_snapshot_for(workspace)
            .await
    }

    pub(in crate::api::code_web) fn code_config_snapshot(&self) -> CodeConfig {
        self.code_config
            .read()
            .map(|config| config.clone())
            .unwrap_or_default()
    }

    pub(in crate::api::code_web) fn current_default_model(&self) -> Option<String> {
        self.code_config_snapshot().default_model
    }

    pub(in crate::api::code_web) async fn close(&self) {
        let research_runs = self
            .active_research_runs
            .lock()
            .await
            .drain()
            .map(|(_, cancellation)| cancellation)
            .collect::<Vec<_>>();
        for cancellation in research_runs {
            cancellation.cancel();
        }
        let registry = self
            .use_registry
            .read()
            .unwrap_or_else(|poison| poison.into_inner())
            .clone();
        if let Some(registry) = registry {
            registry.shutdown().await;
        }
        let sessions = self
            .sessions
            .lock()
            .await
            .values()
            .cloned()
            .collect::<Vec<_>>();
        for session in sessions {
            if let Err(error) = session.save().await {
                eprintln!(
                    "warning: failed to save Code Web session `{}` during shutdown: {error}",
                    session.session_id()
                );
            }
        }
        self.workspace_backends.close().await;
        self.agent.close().await;
    }
}

#[cfg(test)]
mod tests {
    use a3s_code_core::llm::{StreamEvent, ToolDefinition};
    use a3s_code_core::{LlmClient, LlmResponse, Message};
    use async_trait::async_trait;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    use super::*;

    struct TestClient;

    #[async_trait]
    impl LlmClient for TestClient {
        async fn complete(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            unreachable!("client identity test does not send requests")
        }

        async fn complete_streaming(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
            _cancel_token: CancellationToken,
        ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
            unreachable!("client identity test does not send requests")
        }
    }

    #[test]
    fn session_context_replaces_and_clears_llm_client_by_identity() {
        let first: Arc<dyn LlmClient> = Arc::new(TestClient);
        let second: Arc<dyn LlmClient> = Arc::new(TestClient);
        let mut context = CodeWebSessionContext::default();

        context.set_llm_client(Arc::clone(&first));
        assert!(Arc::ptr_eq(
            &first,
            &context.llm_client().expect("first client")
        ));

        context.set_llm_client(Arc::clone(&second));
        assert!(Arc::ptr_eq(
            &second,
            &context.llm_client().expect("replacement client")
        ));

        let mut contexts = HashMap::from([("session".to_string(), context)]);
        contexts.remove("session");
        assert!(!contexts.contains_key("session"));
    }
}
