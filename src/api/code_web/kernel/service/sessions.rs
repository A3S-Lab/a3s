use super::maintenance::{fork_transcript_from_history, fork_transcript_from_messages};
use super::persistence::code_web_store_dir;
use super::text::truncate_chars;
use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn create_session(
        &self,
        request: CreateSessionRequest,
    ) -> BootResult<SessionResponse> {
        let session = self.create_or_get_session(None, request).await?;
        let settings = self.session_settings_snapshot(session.session_id()).await;
        let metadata = self.session_metadata_snapshot(session.session_id()).await;
        Ok(SessionResponse::from_session(
            &session,
            self.session_response_model(session.session_id()).await,
            settings.follow_default_model,
            settings.permission_mode,
            &metadata,
        ))
    }

    pub(in crate::api::code_web) async fn create_kernel_session(
        &self,
        request: CreateSessionRequest,
    ) -> BootResult<KernelSessionResponse> {
        let session = self.create_or_get_session(None, request).await?;
        let settings = self.session_settings_snapshot(session.session_id()).await;
        let metadata = self.session_metadata_snapshot(session.session_id()).await;
        Ok(KernelSessionResponse {
            success: true,
            session: SessionResponse::from_session(
                &session,
                self.session_response_model(session.session_id()).await,
                settings.follow_default_model,
                settings.permission_mode,
                &metadata,
            ),
        })
    }

    pub(in crate::api::code_web) async fn list_sessions(&self) -> BootResult<SessionListResponse> {
        let mut session_ids: Vec<(i64, String)> = self
            .state
            .session_metadata
            .lock()
            .await
            .iter()
            .map(|(session_id, metadata)| (metadata.updated_at, session_id.clone()))
            .collect();
        session_ids.sort_by(|left, right| right.cmp(left));
        let mut sessions = Vec::new();
        for (_, session_id) in session_ids {
            let session = self.kernel_session(&session_id).await?;
            let settings = self.session_settings_snapshot(&session_id).await;
            let metadata = self.session_metadata_snapshot(&session_id).await;
            sessions.push(SessionResponse::from_session(
                session.as_ref(),
                self.session_response_model(&session_id).await,
                settings.follow_default_model,
                settings.permission_mode,
                &metadata,
            ));
        }
        Ok(SessionListResponse {
            total: sessions.len(),
            items: sessions,
        })
    }

    pub(in crate::api::code_web) async fn get_session(
        &self,
        session_id: &str,
    ) -> BootResult<SessionResponse> {
        let session = self.kernel_session(session_id).await?;
        let settings = self.session_settings_snapshot(session_id).await;
        let metadata = self.session_metadata_snapshot(session_id).await;
        Ok(SessionResponse::from_session(
            session.as_ref(),
            self.session_response_model(session_id).await,
            settings.follow_default_model,
            settings.permission_mode,
            &metadata,
        ))
    }

    pub(in crate::api::code_web) async fn update_session(
        &self,
        session_id: &str,
        patch: serde_json::Value,
    ) -> BootResult<SessionResponse> {
        self.apply_session_update(session_id, patch).await?;
        self.get_session(session_id).await
    }

    pub(in crate::api::code_web) async fn delete_session(
        &self,
        session_id: &str,
    ) -> BootResult<()> {
        let session = self.kernel_session(session_id).await?;
        let workspace = session.workspace().to_path_buf();
        self.state.detach_use_session(session_id).await;
        session.cancel_confirmations().await;
        session.close().await;
        self.state
            .session_repository
            .delete_session(session_id)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let store_dir = code_web_store_dir(&workspace);
        crate::timeline::TimelineJsonlStore::for_session(&store_dir, session_id)
            .clear()
            .map_err(|error| BootError::Internal(error.to_string()))?;
        crate::compact::ContextJsonStore::for_session(&store_dir, session_id)
            .clear()
            .map_err(|error| BootError::Internal(error.to_string()))?;
        self.state.sessions.lock().await.remove(session_id);
        self.state.messages.lock().await.remove(session_id);
        self.state.session_metadata.lock().await.remove(session_id);
        self.state.session_controls.lock().await.remove(session_id);
        self.state.session_contexts.lock().await.remove(session_id);
        self.state.session_settings.lock().await.remove(session_id);
        self.state
            .session_turn_queues
            .lock()
            .await
            .remove(session_id);
        if let Some(cancellation) = self
            .state
            .active_research_runs
            .lock()
            .await
            .remove(session_id)
        {
            cancellation.cancel();
        }
        Ok(())
    }

    pub(in crate::api::code_web) async fn confirm_tool_use(
        &self,
        session_id: &str,
        tool_id: &str,
        request: ConfirmToolUseRequest,
    ) -> BootResult<Value> {
        let tool_id = tool_id.trim();
        if tool_id.is_empty() {
            return Err(BootError::BadRequest("tool id is required".to_string()));
        }
        let session = self.kernel_session(session_id).await?;
        let confirmed = session
            .confirm_tool_use(tool_id, request.approved, request.reason.clone())
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        if !confirmed {
            return Err(BootError::BadRequest(format!(
                "tool confirmation `{tool_id}` is no longer pending"
            )));
        }
        Ok(json!({
            "confirmed": true,
            "approved": request.approved,
            "toolId": tool_id,
        }))
    }

    pub(super) async fn create_or_get_session(
        &self,
        requested_id: Option<String>,
        request: CreateSessionRequest,
    ) -> BootResult<Arc<AgentSession>> {
        if let Some(id) = requested_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty())
        {
            let existing_session = self.state.sessions.lock().await.get(id).cloned();
            if let Some(session) = existing_session {
                self.state
                    .session_controls
                    .lock()
                    .await
                    .entry(id.to_string())
                    .or_default();
                self.state
                    .session_contexts
                    .lock()
                    .await
                    .entry(id.to_string())
                    .or_default();
                self.state
                    .session_settings
                    .lock()
                    .await
                    .entry(id.to_string())
                    .or_default();
                self.state
                    .session_turn_queues
                    .lock()
                    .await
                    .entry(id.to_string())
                    .or_default();
                return Ok(session);
            }
        }

        let workspace = request
            .workspace
            .as_deref()
            .or(request.cwd.as_deref())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.state.default_workspace.clone());
        let settings = self.settings_from_create_request(&request);
        let requested_title = normalize_session_title(request.title.as_deref())?;
        let requested_agent_id = request
            .agent_id
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned);
        let requested_session_id = requested_id
            .as_deref()
            .map(str::trim)
            .filter(|id| !id.is_empty());
        let default_controls = CodeWebSessionControls::default();

        let (session, llm_client) = self
            .create_agent_session(
                &workspace,
                requested_session_id,
                &settings,
                &default_controls,
            )
            .await?;
        self.state
            .sessions
            .lock()
            .await
            .insert(session.session_id().to_string(), Arc::clone(&session));
        self.state
            .messages
            .lock()
            .await
            .entry(session.session_id().to_string())
            .or_default();
        let now = chrono::Utc::now().timestamp_millis();
        self.state.session_metadata.lock().await.insert(
            session.session_id().to_string(),
            CodeWebSessionMetadata {
                workspace: session.workspace().display().to_string(),
                title: requested_title,
                agent_id: Some(requested_agent_id.unwrap_or_else(|| "default".to_string())),
                created_at: now,
                updated_at: now,
            },
        );
        self.state
            .session_controls
            .lock()
            .await
            .entry(session.session_id().to_string())
            .or_insert(default_controls);
        self.state
            .session_contexts
            .lock()
            .await
            .entry(session.session_id().to_string())
            .or_default()
            .set_llm_client(llm_client);
        self.state
            .session_settings
            .lock()
            .await
            .entry(session.session_id().to_string())
            .or_insert(settings);
        self.state
            .session_turn_queues
            .lock()
            .await
            .entry(session.session_id().to_string())
            .or_default();
        session
            .save()
            .await
            .map_err(|error| BootError::Internal(format!("failed to save session: {error}")))?;
        self.persist_session_state(session.session_id()).await?;
        Ok(session)
    }

    pub(super) async fn kernel_session(&self, session_id: &str) -> BootResult<Arc<AgentSession>> {
        self.state
            .sessions
            .lock()
            .await
            .get(session_id)
            .cloned()
            .ok_or_else(|| BootError::NotFound(format!("session `{session_id}` was not found")))
    }

    pub(super) async fn session_controls_snapshot(
        &self,
        session_id: &str,
    ) -> CodeWebSessionControls {
        self.state
            .session_controls
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .clone()
    }

    pub(super) async fn session_context_snapshot(&self, session_id: &str) -> CodeWebSessionContext {
        self.state
            .session_contexts
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .clone()
    }

    pub(super) async fn session_metadata_snapshot(
        &self,
        session_id: &str,
    ) -> CodeWebSessionMetadata {
        if let Some(metadata) = self
            .state
            .session_metadata
            .lock()
            .await
            .get(session_id)
            .cloned()
        {
            return metadata;
        }
        let workspace = self
            .state
            .sessions
            .lock()
            .await
            .get(session_id)
            .map(|session| session.workspace().display().to_string())
            .unwrap_or_default();
        let now = chrono::Utc::now().timestamp_millis();
        let metadata = CodeWebSessionMetadata {
            workspace,
            title: None,
            agent_id: Some("default".to_string()),
            created_at: now,
            updated_at: now,
        };
        self.state
            .session_metadata
            .lock()
            .await
            .insert(session_id.to_string(), metadata.clone());
        metadata
    }

    pub(super) async fn session_llm_client(
        &self,
        session_id: &str,
    ) -> BootResult<Arc<dyn a3s_code_core::LlmClient>> {
        self.state
            .session_contexts
            .lock()
            .await
            .get(session_id)
            .and_then(CodeWebSessionContext::llm_client)
            .ok_or_else(|| {
                BootError::Internal(format!(
                    "session `{session_id}` has no registered LLM client"
                ))
            })
    }

    pub(super) async fn session_settings_snapshot(
        &self,
        session_id: &str,
    ) -> CodeWebSessionSettings {
        self.state
            .session_settings
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .clone()
    }

    fn settings_from_create_request(
        &self,
        request: &CreateSessionRequest,
    ) -> CodeWebSessionSettings {
        let follow_default_model = request.follow_default_model.unwrap_or_else(|| {
            request
                .model
                .as_deref()
                .map(str::trim)
                .unwrap_or("")
                .is_empty()
        });
        let model = if follow_default_model {
            None
        } else {
            request
                .model
                .as_deref()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(ToOwned::to_owned)
                .or_else(|| self.state.current_default_model())
        };
        CodeWebSessionSettings {
            model,
            follow_default_model,
            permission_mode: request
                .permission_mode
                .as_deref()
                .and_then(normalize_permission_mode)
                .unwrap_or_else(|| "default".to_string()),
            planning_mode: request
                .planning_mode
                .as_deref()
                .and_then(normalize_planning_mode),
            goal_tracking: request.goal_tracking,
        }
    }

    pub(super) fn effective_model(&self, settings: &CodeWebSessionSettings) -> Option<String> {
        effective_session_model(self.state.as_ref(), settings)
    }

    pub(super) async fn create_agent_session(
        &self,
        workspace: &Path,
        session_id: Option<&str>,
        settings: &CodeWebSessionSettings,
        controls: &CodeWebSessionControls,
    ) -> BootResult<(Arc<AgentSession>, Arc<dyn a3s_code_core::LlmClient>)> {
        let (options, runtime, llm_client) = code_web_session_options(
            self.state.as_ref(),
            workspace,
            session_id,
            self.effective_model(settings),
            controls,
            settings,
        )
        .await?;
        let session = Arc::new(
            self.state
                .agent
                .session_async(workspace.display().to_string(), Some(options))
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?,
        );
        activate_session_runtime(session.as_ref(), &runtime);
        self.state.attach_use_session(Arc::clone(&session));
        Ok((session, llm_client))
    }

    pub(super) async fn session_response_model(&self, session_id: &str) -> Option<String> {
        let settings = self.session_settings_snapshot(session_id).await;
        self.effective_model(&settings)
    }

    async fn apply_session_update(
        &self,
        session_id: &str,
        patch: serde_json::Value,
    ) -> BootResult<()> {
        self.kernel_session(session_id).await?;
        if let Some(title) = patch.get("title") {
            let title = match title {
                Value::String(title) => normalize_session_title(Some(title))?,
                Value::Null => None,
                _ => {
                    return Err(BootError::BadRequest(
                        "title must be a string or null".to_string(),
                    ))
                }
            };
            self.state
                .session_metadata
                .lock()
                .await
                .entry(session_id.to_string())
                .or_default()
                .title = title;
        }
        let default_model = self.state.current_default_model();
        let (settings, runtime_changed) = {
            let mut settings_by_session = self.state.session_settings.lock().await;
            let settings = settings_by_session
                .entry(session_id.to_string())
                .or_default();
            let runtime_changed = apply_settings_patch(settings, &patch, default_model)?;
            (settings.clone(), runtime_changed)
        };

        if runtime_changed {
            self.rebuild_session_with_settings(session_id, &settings)
                .await?;
        }
        self.persist_session_state(session_id).await
    }

    pub(super) async fn rebuild_session_with_settings(
        &self,
        session_id: &str,
        settings: &CodeWebSessionSettings,
    ) -> BootResult<()> {
        let old_session = self.kernel_session(session_id).await?;
        let workspace = old_session.workspace().to_path_buf();
        let history = old_session.history();
        let messages = self
            .state
            .messages
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let previous_context = self.session_context_snapshot(session_id).await;
        if let Some(carried_context) = build_reconfigure_context(
            previous_context.compact_summary.as_deref(),
            &messages,
            &history,
        ) {
            self.state.session_contexts.lock().await.insert(
                session_id.to_string(),
                CodeWebSessionContext {
                    compact_summary: Some(carried_context),
                    ..CodeWebSessionContext::default()
                },
            );
        }

        let controls = self.session_controls_snapshot(session_id).await;
        let (options, runtime, llm_client) = code_web_session_options(
            self.state.as_ref(),
            &workspace,
            Some(session_id),
            self.effective_model(settings),
            &controls,
            settings,
        )
        .await?;
        let new_session = Arc::new(
            self.state
                .agent
                .replace_session_async(old_session.as_ref(), options)
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?,
        );
        activate_session_runtime(new_session.as_ref(), &runtime);
        self.state.attach_use_session(Arc::clone(&new_session));
        self.state
            .sessions
            .lock()
            .await
            .insert(session_id.to_string(), new_session);
        self.state
            .session_contexts
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .set_llm_client(llm_client);
        Ok(())
    }
}

fn normalize_session_title(value: Option<&str>) -> BootResult<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let value = value.trim();
    if value.chars().count() > 200 {
        return Err(BootError::BadRequest(
            "title must not exceed 200 characters".to_string(),
        ));
    }
    Ok((!value.is_empty()).then(|| value.to_string()))
}

pub(super) fn apply_settings_patch(
    settings: &mut CodeWebSessionSettings,
    patch: &Value,
    default_model: Option<String>,
) -> BootResult<bool> {
    let Some(patch) = patch.as_object() else {
        return Err(BootError::BadRequest(
            "session update body must be an object".to_string(),
        ));
    };
    let previous = settings.clone();

    let follow_default_patch = match patch.get("followDefaultModel") {
        Some(Value::Bool(value)) => Some(*value),
        Some(Value::Null) | None => None,
        Some(_) => {
            return Err(BootError::BadRequest(
                "followDefaultModel must be a boolean".to_string(),
            ));
        }
    };

    if let Some(model_value) = patch.get("model") {
        match model_value {
            Value::String(model) => {
                let model = model.trim();
                if model.is_empty() {
                    settings.model = None;
                    settings.follow_default_model = follow_default_patch.unwrap_or(true);
                } else {
                    settings.model = Some(model.to_string());
                    settings.follow_default_model = follow_default_patch.unwrap_or(false);
                }
            }
            Value::Null => {
                settings.model = None;
                settings.follow_default_model = follow_default_patch.unwrap_or(true);
            }
            _ => return Err(BootError::BadRequest("model must be a string".to_string())),
        }
    } else if let Some(follow_default_model) = follow_default_patch {
        settings.follow_default_model = follow_default_model;
        if follow_default_model {
            settings.model = None;
        } else if settings.model.is_none() {
            settings.model = default_model;
        }
    }

    if let Some(permission_mode_value) = patch.get("permissionMode") {
        let permission_mode = permission_mode_value
            .as_str()
            .ok_or_else(|| BootError::BadRequest("permissionMode must be a string".to_string()))?;
        settings.permission_mode = normalize_permission_mode(permission_mode).ok_or_else(|| {
            BootError::BadRequest(format!("unsupported permissionMode `{permission_mode}`"))
        })?;
    }

    if let Some(planning_mode_value) = patch.get("planningMode") {
        match planning_mode_value {
            Value::String(planning_mode) => {
                settings.planning_mode =
                    Some(normalize_planning_mode(planning_mode).ok_or_else(|| {
                        BootError::BadRequest(format!("unsupported planningMode `{planning_mode}`"))
                    })?);
            }
            Value::Null => settings.planning_mode = None,
            _ => {
                return Err(BootError::BadRequest(
                    "planningMode must be a string or null".to_string(),
                ));
            }
        }
    }

    if let Some(goal_tracking_value) = patch.get("goalTracking") {
        match goal_tracking_value {
            Value::Bool(goal_tracking) => settings.goal_tracking = Some(*goal_tracking),
            Value::Null => settings.goal_tracking = None,
            _ => {
                return Err(BootError::BadRequest(
                    "goalTracking must be a boolean or null".to_string(),
                ));
            }
        }
    }

    Ok(previous != *settings)
}

fn normalize_permission_mode(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "default" | "plan" | "auto" => Some(value.trim().to_ascii_lowercase()),
        _ => None,
    }
}

fn normalize_planning_mode(value: &str) -> Option<String> {
    match value.trim().to_ascii_lowercase().as_str() {
        "auto" | "enabled" | "disabled" => Some(value.trim().to_ascii_lowercase()),
        _ => None,
    }
}

fn build_reconfigure_context(
    previous_summary: Option<&str>,
    messages: &[serde_json::Value],
    history: &[Message],
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(summary) = previous_summary
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("Existing compacted summary:\n{summary}"));
    }
    let visible_transcript = fork_transcript_from_messages(messages);
    if !visible_transcript.is_empty() {
        parts.push(format!(
            "Visible conversation transcript:\n{visible_transcript}"
        ));
    } else {
        let history_transcript = fork_transcript_from_history(history);
        if !history_transcript.is_empty() {
            parts.push(format!("Core conversation history:\n{history_transcript}"));
        }
    }
    if parts.is_empty() {
        None
    } else {
        Some(truncate_chars(
            &format!(
                "This session was reconfigured. Continue with this carried context:\n\n{}",
                parts.join("\n\n")
            ),
            12_000,
        ))
    }
}
