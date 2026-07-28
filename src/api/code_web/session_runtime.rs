use std::path::{Path, PathBuf};
use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::host_env::HostEnv;
use a3s_code_core::{
    AgentSession, CodeConfig, LlmClient, PlanningMode, SessionOptions, SystemPromptSlots,
};
use serde_json::{json, Value};

use super::permissions::{
    confirmation_policy_for_mode, permission_checker_for_mode, permission_policy_for_mode,
    CodeWebModeConfirmationProvider,
};
use super::state::{CodeWebSessionControls, CodeWebSessionSettings, CodeWebState};
use crate::budget::{self, BudgetWorkload};
use crate::config;
use crate::tui::skills::{agent_skill_dirs, ensure_builtin_skills_dir};

#[derive(Debug, Clone)]
pub(in crate::api::code_web) struct CodeWebSessionRuntime {
    pub(in crate::api::code_web) skill_dirs: Vec<PathBuf>,
    pub(in crate::api::code_web) os_session: Option<crate::a3s_os::StoredOsSession>,
    os_configured: bool,
    os_address: Option<String>,
    builtin_skill_active: bool,
    capability_skill_active: bool,
    refresh_error: Option<String>,
}

impl CodeWebSessionRuntime {
    pub(in crate::api::code_web) fn os_status_json(&self) -> Value {
        let session = self.os_session.as_ref();
        let address = session
            .map(|session| session.address.clone())
            .or_else(|| self.os_address.clone());
        let origin = address.as_deref().map(crate::a3s_os::os_origin);
        json!({
            "configured": self.os_configured,
            "address": address,
            "origin": origin,
            "signedIn": session.is_some(),
            "label": session.map(crate::a3s_os::StoredOsSession::display_label),
            "loginAtMs": session.map(|session| session.login_at_ms),
            "expiresAtMs": session.and_then(|session| session.expires_at_ms),
            "tokenType": session.and_then(|session| session.token_type.clone()),
            "needsRefresh": session.is_some_and(crate::a3s_os::needs_refresh),
            "capabilitySkillActive": self.capability_skill_active,
            "builtinSkillActive": self.builtin_skill_active,
            "runtimeToolActive": session.is_some(),
            "refreshError": self.refresh_error,
        })
    }
}

pub(in crate::api::code_web) async fn code_web_os_status(
    state: &CodeWebState,
) -> BootResult<Value> {
    Ok(
        code_web_session_runtime_for_workspace(state, &state.default_workspace)
            .await
            .os_status_json(),
    )
}

pub(in crate::api::code_web) async fn code_web_session_options(
    state: &CodeWebState,
    workspace: &Path,
    session_id: Option<&str>,
    model: Option<String>,
    controls: &CodeWebSessionControls,
    settings: &CodeWebSessionSettings,
) -> BootResult<(SessionOptions, CodeWebSessionRuntime, Arc<dyn LlmClient>)> {
    let evolution = crate::evolution::WorkspaceEvolution::new(workspace);
    if let Err(error) = evolution
        .synchronize_memory_store(config::memory_dir())
        .await
    {
        tracing::warn!(%error, "could not synchronize memory evolution before Web session startup");
    }
    let learned_preferences = match evolution.session_preference_prompt() {
        Ok(preferences) => preferences,
        Err(error) => {
            tracing::warn!(%error, "could not load learned preferences before Web session startup");
            None
        }
    };
    let runtime = code_web_session_runtime_for_workspace(state, workspace).await;
    let context_limit = code_web_context_limit_for_model(state, model.as_deref());
    let budget = budget::budget_plan_for_effort_id(
        &controls.effort,
        Some(context_limit),
        BudgetWorkload::Interactive,
    );
    let permission_policy = permission_policy_for_mode(&settings.permission_mode);
    let permission_checker = permission_checker_for_mode(&settings.permission_mode, workspace);
    let confirmation_manager = CodeWebModeConfirmationProvider::new(
        &settings.permission_mode,
        confirmation_policy_for_mode(&settings.permission_mode),
    );
    let mut options = SessionOptions::new()
        .with_session_store(state.session_repository.core_store())
        .with_workspace_backend(
            state
                .workspace_services_for(workspace)
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?,
        )
        .with_auto_save(true)
        .with_auto_compact(true)
        .with_max_context_tokens(context_limit as usize)
        .with_auto_compact_threshold(state.auto_compact_threshold as f32)
        .with_file_memory(config::memory_dir())
        .with_memory_observer(crate::evolution::EvolutionMemoryObserver::new(evolution))
        .with_skill_dirs(runtime.skill_dirs.clone())
        .with_max_tool_rounds(budget.max_tool_rounds)
        .with_max_parallel_tasks(budget.max_parallel_tasks)
        .with_max_continuation_turns(budget.max_continuation_turns)
        .with_confirmation_manager(Arc::new(confirmation_manager))
        .with_permission_policy(permission_policy)
        .with_permission_checker(Arc::new(permission_checker))
        .with_planning_mode(effective_planning_mode(controls, settings))
        .with_goal_tracking(effective_goal_tracking(controls, settings));
    let session_id = session_id
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| HostEnv::default().next_id());
    options = options.with_session_id(session_id.clone());
    if let Some(model) = model {
        options = options.with_model(model);
    }
    let mut extra_prompt = Vec::new();
    if let Some(session) = runtime.os_session.as_ref() {
        extra_prompt.push(os_platform_guide(&session.address));
    }
    if let Some(preferences) = learned_preferences {
        extra_prompt.push(preferences);
    }
    if !extra_prompt.is_empty() {
        options = options
            .with_prompt_slots(SystemPromptSlots::default().with_extra(extra_prompt.join("\n\n")));
    }

    let llm_client = crate::session_llm::resolve_session_llm_client(
        &state.code_config_snapshot(),
        &options,
        &session_id,
    )
    .map_err(BootError::Internal)?;
    options = options.with_llm_client(Arc::clone(&llm_client));

    Ok((options, runtime, llm_client))
}

pub(in crate::api::code_web) fn code_web_context_limit_for_model(
    state: &CodeWebState,
    model: Option<&str>,
) -> u32 {
    let Some(model) = model.map(str::trim).filter(|model| !model.is_empty()) else {
        return budget::resolve_ctx_limit(None);
    };
    let config = state.code_config_snapshot();
    budget::context_limit_for_model(model, configured_context_limit(&config, model), None)
}

fn configured_context_limit(config: &CodeConfig, model: &str) -> Option<u32> {
    config
        .list_models()
        .into_iter()
        .find_map(|(provider, item)| {
            let qualified = format!("{}/{}", provider.name, item.id);
            (model == qualified || model == item.id)
                .then_some(item.limit.context)
                .filter(|context| *context > 0)
        })
}

pub(in crate::api::code_web) fn activate_session_runtime(
    session: &AgentSession,
    runtime: &CodeWebSessionRuntime,
) {
    let _ = session.register_dynamic_workflow_runtime();
    match runtime.os_session.as_ref() {
        Some(os_session) => {
            let _ = session
                .register_dynamic_tool(Arc::new(crate::runtime_tool::RuntimeTool::new(os_session)));
        }
        None => {
            let _ = session.unregister_dynamic_tool("runtime");
        }
    }
}

pub(in crate::api::code_web) async fn rebuild_code_web_sessions(
    state: &CodeWebState,
) -> BootResult<Vec<Value>> {
    rebuild_code_web_sessions_for_workspace(state, None).await
}

pub(in crate::api::code_web) async fn rebuild_code_web_sessions_for_workspace(
    state: &CodeWebState,
    workspace_filter: Option<&Path>,
) -> BootResult<Vec<Value>> {
    let sessions = current_sessions(state)
        .await
        .into_iter()
        .filter(|(_, _, workspace)| {
            workspace_filter.is_none_or(|filter| same_workspace(workspace, filter))
        })
        .collect::<Vec<_>>();
    let mut rebuilt = Vec::new();

    for (session_id, old_session, workspace) in sessions {
        let settings = session_settings(state, &session_id).await;
        let controls = session_controls(state, &session_id).await;
        let model = effective_session_model(state, &settings);
        let (options, runtime, llm_client) = code_web_session_options(
            state,
            &workspace,
            Some(&session_id),
            model,
            &controls,
            &settings,
        )
        .await?;
        let new_session = Arc::new(
            state
                .agent
                .replace_session_async(old_session.as_ref(), options)
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?,
        );
        activate_session_runtime(new_session.as_ref(), &runtime);
        state
            .sessions
            .lock()
            .await
            .insert(session_id.clone(), new_session);
        state
            .session_contexts
            .lock()
            .await
            .entry(session_id.clone())
            .or_default()
            .set_llm_client(llm_client);
        rebuilt.push(json!({
            "sessionId": session_id,
            "workspace": workspace.display().to_string(),
            "skillDirCount": runtime.skill_dirs.len(),
            "builtinSkillActive": runtime.builtin_skill_active,
            "capabilitySkillActive": runtime.capability_skill_active,
            "runtimeToolActive": runtime.os_session.is_some(),
        }));
    }

    Ok(rebuilt)
}

pub(in crate::api::code_web) async fn refresh_evolution_runtime_after_turn(
    state: &CodeWebState,
    workspace: &Path,
) -> BootResult<usize> {
    let _refresh = state.evolution_refresh_lock.lock().await;
    let evolution = crate::evolution::WorkspaceEvolution::new(workspace);
    let pending = evolution
        .pending_session_reload_count()
        .await
        .map_err(|error| BootError::Internal(error.to_string()))?;
    if pending == 0 {
        return Ok(0);
    }

    let rebuilt = rebuild_code_web_sessions_for_workspace(state, Some(workspace)).await?;
    if rebuilt.is_empty() {
        return Ok(0);
    }
    evolution
        .mark_session_assets_activated()
        .await
        .map_err(|error| BootError::Internal(error.to_string()))
}

fn effective_planning_mode(
    controls: &CodeWebSessionControls,
    settings: &CodeWebSessionSettings,
) -> PlanningMode {
    if controls.goal.is_some() || settings.permission_mode == "plan" {
        return PlanningMode::Enabled;
    }
    planning_mode(settings.planning_mode.as_deref())
}

fn effective_goal_tracking(
    controls: &CodeWebSessionControls,
    settings: &CodeWebSessionSettings,
) -> bool {
    controls.goal.is_some() || settings.goal_tracking.unwrap_or(false)
}

fn planning_mode(value: Option<&str>) -> PlanningMode {
    match value {
        Some("enabled") => PlanningMode::Enabled,
        Some("disabled") => PlanningMode::Disabled,
        _ => PlanningMode::Auto,
    }
}

pub(in crate::api::code_web) fn effective_session_model(
    state: &CodeWebState,
    settings: &CodeWebSessionSettings,
) -> Option<String> {
    if settings.follow_default_model {
        return state.current_default_model();
    }
    settings
        .model
        .as_deref()
        .map(str::trim)
        .filter(|model| !model.is_empty())
        .map(ToOwned::to_owned)
        .or_else(|| state.current_default_model())
}

async fn code_web_session_runtime_for_workspace(
    state: &CodeWebState,
    workspace: &Path,
) -> CodeWebSessionRuntime {
    let code_config = state.code_config_snapshot();
    let os_config = code_config.os.clone();
    let os_configured = os_config.is_some();
    let os_address = os_config.as_ref().map(|config| config.address.clone());

    let mut skill_dirs = agent_skill_dirs(&workspace.display().to_string());
    let builtin_skill_active = if let Some(dir) = ensure_builtin_skills_dir() {
        skill_dirs.push(dir);
        true
    } else {
        false
    };

    let (os_session, refresh_error) = match os_config.as_ref() {
        Some(config) => current_os_session(config).await,
        None => (None, None),
    };
    let capability_skill_active = if os_session.is_some() {
        os_config
            .as_ref()
            .and_then(crate::a3s_os::ensure_capability_skill_dir)
            .inspect(|dir| skill_dirs.push(dir.clone()))
            .is_some()
    } else {
        false
    };

    if let Some(session) = os_session.as_ref() {
        crate::a3s_os::export_os_env(session);
    }

    skill_dirs.sort();
    skill_dirs.dedup();

    CodeWebSessionRuntime {
        skill_dirs,
        os_session,
        os_configured,
        os_address,
        builtin_skill_active,
        capability_skill_active,
        refresh_error,
    }
}

async fn current_os_session(
    config: &a3s_code_core::config::OsConfig,
) -> (Option<crate::a3s_os::StoredOsSession>, Option<String>) {
    let Some(session) = crate::a3s_os::current_session(config) else {
        return (None, None);
    };

    if !crate::a3s_os::needs_refresh(&session) {
        return (Some(session), None);
    }

    match crate::a3s_os::refresh_session(&session).await {
        Ok(refreshed) => (Some(refreshed), None),
        Err(error) => (Some(session), Some(error.to_string())),
    }
}

async fn current_sessions(state: &CodeWebState) -> Vec<(String, Arc<AgentSession>, PathBuf)> {
    state
        .sessions
        .lock()
        .await
        .iter()
        .map(|(session_id, session)| {
            (
                session_id.clone(),
                Arc::clone(session),
                session.workspace().to_path_buf(),
            )
        })
        .collect()
}

fn same_workspace(left: &Path, right: &Path) -> bool {
    left == right
        || left
            .canonicalize()
            .ok()
            .zip(right.canonicalize().ok())
            .is_some_and(|(left, right)| left == right)
}

async fn session_settings(state: &CodeWebState, session_id: &str) -> CodeWebSessionSettings {
    state
        .session_settings
        .lock()
        .await
        .get(session_id)
        .cloned()
        .unwrap_or_default()
}

async fn session_controls(state: &CodeWebState, session_id: &str) -> CodeWebSessionControls {
    state
        .session_controls
        .lock()
        .await
        .get(session_id)
        .cloned()
        .unwrap_or_default()
}

fn os_platform_guide(base_url: &str) -> String {
    format!(
        "[OS platform] You are signed in to the A3S OS platform at {base_url}. \
When the user says OS, they mean this platform, not the local operating system. \
Use the exported A3S_OS_BASE_URL and A3S_OS_TOKEN environment variables, the \
a3s-os-capabilities skill, and the runtime tool for login-gated platform work. \
For progressive API execute calls, preserve shaped view data so the host can \
show the user an authenticated Open view action."
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::config::{ModelConfig, ModelLimit, ProviderConfig};
    use a3s_code_core::memory::MemoryObservation;
    use a3s_memory::{MemoryItem, MemoryType};
    use std::collections::HashMap;

    struct MemoryDirEnv {
        previous: Option<std::ffi::OsString>,
    }

    impl MemoryDirEnv {
        fn install(path: &Path) -> Self {
            let previous = std::env::var_os("A3S_MEMORY_DIR");
            std::env::set_var("A3S_MEMORY_DIR", path);
            Self { previous }
        }
    }

    impl Drop for MemoryDirEnv {
        fn drop(&mut self) {
            match self.previous.take() {
                Some(value) => std::env::set_var("A3S_MEMORY_DIR", value),
                None => std::env::remove_var("A3S_MEMORY_DIR"),
            }
        }
    }

    async fn test_state(root: &Path, workspace: &Path) -> Arc<CodeWebState> {
        let config = CodeConfig::from_acl(
            r#"
                default_model = "openai/test-model"
                providers "openai" {
                  apiKey = "test-key"
                  baseUrl = "http://127.0.0.1:1/v1"
                  models "test-model" {}
                }
            "#,
        )
        .unwrap();
        let agent = Arc::new(
            a3s_code_core::Agent::from_config(config.clone())
                .await
                .unwrap(),
        );
        let repository = Arc::new(
            crate::api::code_web::session_store::CodeWebSessionRepository::open(
                root.join("sessions"),
            )
            .await
            .unwrap(),
        );
        Arc::new(CodeWebState::new(
            agent,
            root.join("config.acl"),
            workspace.to_path_buf(),
            config,
            repository,
        ))
    }

    async fn install_session(state: &CodeWebState, id: &str) -> Arc<AgentSession> {
        let settings = CodeWebSessionSettings::default();
        let controls = CodeWebSessionControls::default();
        let (options, runtime, llm_client) = code_web_session_options(
            state,
            &state.default_workspace,
            Some(id),
            state.current_default_model(),
            &controls,
            &settings,
        )
        .await
        .unwrap();
        let session = Arc::new(
            state
                .agent
                .session_async(state.default_workspace.display().to_string(), Some(options))
                .await
                .unwrap(),
        );
        activate_session_runtime(session.as_ref(), &runtime);
        state
            .sessions
            .lock()
            .await
            .insert(id.to_string(), Arc::clone(&session));
        state
            .session_settings
            .lock()
            .await
            .insert(id.to_string(), settings);
        state
            .session_controls
            .lock()
            .await
            .insert(id.to_string(), controls);
        state
            .session_contexts
            .lock()
            .await
            .entry(id.to_string())
            .or_default()
            .set_llm_client(llm_client);
        session
    }

    fn evolution_observation(
        id: &str,
        session: &str,
        workspace: &Path,
        kind: &str,
        pattern: &str,
        title: &str,
    ) -> MemoryObservation {
        let source = if kind == "preference" {
            "preference"
        } else {
            "workflow"
        };
        let item = MemoryItem::new(format!("Stable local evidence for {title}."))
            .with_type(if kind == "preference" {
                MemoryType::Semantic
            } else {
                MemoryType::Procedural
            })
            .with_importance(0.92)
            .with_metadata("source", source)
            .with_metadata("scope", "workspace")
            .with_metadata("workspace", workspace.display().to_string())
            .with_metadata("session_id", session)
            .with_metadata("confidence", "0.96")
            .with_metadata("evolution_schema", "a3s.evolution.signal.v1")
            .with_metadata("evolution_kind", kind)
            .with_metadata("evolution_pattern", pattern)
            .with_metadata("evolution_title", title)
            .with_metadata(
                "evolution_summary",
                format!("Apply the reusable local guidance for {title}."),
            )
            .with_metadata(
                "evolution_instructions",
                r#"["Apply this guidance when the task matches.","Verify the current workspace evidence first."]"#,
            );
        let mut incoming = item.clone();
        incoming.id = id.to_string();
        MemoryObservation {
            incoming: incoming.clone(),
            stored: incoming,
            merged: false,
        }
    }

    #[test]
    fn active_goal_forces_planning_and_goal_tracking() {
        let controls = CodeWebSessionControls {
            goal: Some("Ship verified queue semantics".to_string()),
            ..CodeWebSessionControls::default()
        };
        let settings = CodeWebSessionSettings {
            planning_mode: Some("disabled".to_string()),
            goal_tracking: Some(false),
            ..CodeWebSessionSettings::default()
        };

        assert_eq!(
            effective_planning_mode(&controls, &settings),
            PlanningMode::Enabled
        );
        assert!(effective_goal_tracking(&controls, &settings));
    }

    #[test]
    fn sessions_without_a_goal_keep_their_selected_execution_modes() {
        let controls = CodeWebSessionControls::default();
        let settings = CodeWebSessionSettings {
            planning_mode: Some("disabled".to_string()),
            goal_tracking: Some(false),
            ..CodeWebSessionSettings::default()
        };

        assert_eq!(
            effective_planning_mode(&controls, &settings),
            PlanningMode::Disabled
        );
        assert!(!effective_goal_tracking(&controls, &settings));
    }

    #[test]
    fn plan_execution_mode_forces_planning_without_enabling_goal_tracking() {
        let controls = CodeWebSessionControls::default();
        let settings = CodeWebSessionSettings {
            permission_mode: "plan".to_string(),
            planning_mode: Some("disabled".to_string()),
            goal_tracking: Some(false),
            ..CodeWebSessionSettings::default()
        };

        assert_eq!(
            effective_planning_mode(&controls, &settings),
            PlanningMode::Enabled
        );
        assert!(!effective_goal_tracking(&controls, &settings));
    }

    #[test]
    fn os_status_does_not_expose_tokens() {
        let runtime = CodeWebSessionRuntime {
            skill_dirs: Vec::new(),
            os_session: Some(crate::a3s_os::StoredOsSession {
                address: "https://os.example.com/platform".to_string(),
                access_token: "secret-access-token".to_string(),
                refresh_token: Some("secret-refresh-token".to_string()),
                token_type: Some("Bearer".to_string()),
                expires_at_ms: Some(42),
                account_label: Some("Ada".to_string()),
                login_at_ms: 1,
            }),
            os_configured: true,
            os_address: Some("https://os.example.com/platform".to_string()),
            builtin_skill_active: true,
            capability_skill_active: true,
            refresh_error: None,
        };

        let status = runtime.os_status_json();
        let encoded = status.to_string();
        assert_eq!(status["signedIn"], true);
        assert_eq!(status["label"], "Ada");
        assert_eq!(status["origin"], "https://os.example.com");
        assert!(!encoded.contains("secret-access-token"));
        assert!(!encoded.contains("secret-refresh-token"));
    }

    #[test]
    fn configured_context_limit_matches_qualified_and_plain_model_ids() {
        let config = CodeConfig {
            providers: vec![ProviderConfig {
                name: "openai".to_string(),
                api_key: None,
                base_url: None,
                headers: HashMap::new(),
                session_id_header: None,
                models: vec![ModelConfig {
                    id: "tiny".to_string(),
                    name: "Tiny".to_string(),
                    family: String::new(),
                    api_key: None,
                    base_url: None,
                    headers: HashMap::new(),
                    session_id_header: None,
                    attachment: false,
                    reasoning: false,
                    tool_call: true,
                    temperature: true,
                    release_date: None,
                    modalities: Default::default(),
                    cost: Default::default(),
                    limit: ModelLimit {
                        context: 32_768,
                        output: 0,
                    },
                }],
            }],
            ..Default::default()
        };

        assert_eq!(
            configured_context_limit(&config, "openai/tiny"),
            Some(32_768)
        );
        assert_eq!(configured_context_limit(&config, "tiny"), Some(32_768));
        assert_eq!(
            budget::context_limit_for_model(
                "openai/tiny",
                configured_context_limit(&config, "openai/tiny"),
                None,
            ),
            32_768
        );
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn post_turn_refresh_activates_an_automatically_learned_skill_for_all_sessions() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let _memory = MemoryDirEnv::install(&temp.path().join("memory"));
        let state = test_state(temp.path(), &workspace).await;
        let first = install_session(state.as_ref(), "session-one").await;
        let second = install_session(state.as_ref(), "session-two").await;
        assert!(same_workspace(first.workspace(), &workspace));
        assert!(same_workspace(second.workspace(), &workspace));
        assert!(!first
            .skill_names()
            .contains(&"learned-release-checks".to_string()));

        let evolution = crate::evolution::WorkspaceEvolution::new(&workspace);
        for (id, session) in [
            ("evidence-one", "session-one"),
            ("evidence-two", "session-two"),
            ("evidence-three", "session-two"),
        ] {
            evolution
                .observe(evolution_observation(
                    id,
                    session,
                    &workspace,
                    "skill",
                    "workflow.release.learned-checks",
                    "Learned release checks",
                ))
                .await
                .unwrap();
        }
        assert_eq!(evolution.pending_session_reload_count().await.unwrap(), 1);

        let activated = refresh_evolution_runtime_after_turn(state.as_ref(), &workspace)
            .await
            .unwrap();

        assert_eq!(activated, 1);
        assert_eq!(evolution.pending_session_reload_count().await.unwrap(), 0);
        let sessions = state.sessions.lock().await;
        let rebuilt_first = sessions.get("session-one").unwrap();
        let rebuilt_second = sessions.get("session-two").unwrap();
        assert!(!Arc::ptr_eq(&first, rebuilt_first));
        assert!(!Arc::ptr_eq(&second, rebuilt_second));
        for session in [rebuilt_first, rebuilt_second] {
            let names = session.skill_names();
            assert!(
                names.contains(&"learned-release-checks".to_string()),
                "rebuilt Skill registry did not include the learned Skill: {names:?}"
            );
        }
        drop(sessions);
        state.close().await;
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn failed_multi_session_rebuild_keeps_the_activation_barrier_pending() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let _memory = MemoryDirEnv::install(&temp.path().join("memory"));
        let state = test_state(temp.path(), &workspace).await;
        install_session(state.as_ref(), "session-one").await;
        install_session(state.as_ref(), "session-two").await;
        state.session_settings.lock().await.insert(
            "session-two".to_string(),
            CodeWebSessionSettings {
                model: Some("missing-provider/missing-model".to_string()),
                follow_default_model: false,
                ..CodeWebSessionSettings::default()
            },
        );

        let evolution = crate::evolution::WorkspaceEvolution::new(&workspace);
        for (id, session) in [
            ("barrier-one", "session-one"),
            ("barrier-two", "session-two"),
            ("barrier-three", "session-two"),
        ] {
            evolution
                .observe(evolution_observation(
                    id,
                    session,
                    &workspace,
                    "skill",
                    "workflow.release.barrier-checks",
                    "Barrier release checks",
                ))
                .await
                .unwrap();
        }

        assert!(
            refresh_evolution_runtime_after_turn(state.as_ref(), &workspace)
                .await
                .is_err()
        );
        assert_eq!(evolution.pending_session_reload_count().await.unwrap(), 1);
        state.close().await;
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn web_session_options_inject_and_remove_the_active_preference_prompt() {
        let _lock = crate::TEST_ENV_LOCK
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let _memory = MemoryDirEnv::install(&temp.path().join("memory"));
        let state = test_state(temp.path(), &workspace).await;
        let evolution = crate::evolution::WorkspaceEvolution::new(&workspace);
        evolution
            .observe(evolution_observation(
                "preference-one",
                "session-one",
                &workspace,
                "preference",
                "preference.response.local-evidence",
                "Local evidence responses",
            ))
            .await
            .unwrap();
        let candidate_id = evolution.overview().await.unwrap().candidates[0].id.clone();
        evolution.materialize(&candidate_id, false).await.unwrap();

        let (options, _, _) = code_web_session_options(
            state.as_ref(),
            &workspace,
            Some("preference-session"),
            state.current_default_model(),
            &CodeWebSessionControls::default(),
            &CodeWebSessionSettings::default(),
        )
        .await
        .unwrap();
        let extra = options
            .prompt_slots
            .as_ref()
            .and_then(|slots| slots.extra.as_deref())
            .unwrap();
        assert!(extra.contains("# Learned Local Preferences"));
        assert!(extra.contains("Apply this guidance when the task matches."));
        assert!(!extra.contains("Stable local evidence"));

        evolution.rollback(&candidate_id, Some(0)).await.unwrap();
        let (options, _, _) = code_web_session_options(
            state.as_ref(),
            &workspace,
            Some("baseline-session"),
            state.current_default_model(),
            &CodeWebSessionControls::default(),
            &CodeWebSessionSettings::default(),
        )
        .await
        .unwrap();
        assert!(options
            .prompt_slots
            .as_ref()
            .and_then(|slots| slots.extra.as_deref())
            .is_none());
        state.close().await;
    }
}
