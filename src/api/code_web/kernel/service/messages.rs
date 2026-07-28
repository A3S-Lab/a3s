use super::maintenance::compact_visible_messages_after_success;
use super::persistence::{
    code_web_store_dir, persist_code_web_compact_summary, seed_code_web_timeline,
};
use super::streaming::run_code_web_stream;
use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn session_messages(
        &self,
        session_id: &str,
    ) -> BootResult<serde_json::Value> {
        self.kernel_session(session_id).await?;
        let messages = self
            .state
            .messages
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        Ok(json!({
            "items": messages,
            "total": messages.len(),
            "page": 1,
            "limit": messages.len().max(1),
        }))
    }

    pub(in crate::api::code_web) async fn clear_session_messages(
        &self,
        session_id: &str,
    ) -> BootResult<serde_json::Value> {
        let old_session = self.kernel_session(session_id).await?;
        let workspace = old_session.workspace().to_path_buf();
        self.state.detach_use_session(session_id).await;
        old_session.cancel_confirmations().await;
        old_session.close().await;
        self.state
            .session_repository
            .delete_core_session(session_id)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let store_dir = code_web_store_dir(&workspace);
        crate::timeline::TimelineJsonlStore::for_session(&store_dir, session_id)
            .clear()
            .map_err(|error| BootError::Internal(error.to_string()))?;
        crate::compact::ContextJsonStore::for_session(&store_dir, session_id)
            .clear()
            .map_err(|error| BootError::Internal(error.to_string()))?;

        let controls = self.session_controls_snapshot(session_id).await;
        let settings = self.session_settings_snapshot(session_id).await;
        let (new_session, llm_client) = self
            .create_agent_session(&workspace, Some(session_id), &settings, &controls)
            .await?;
        new_session
            .save()
            .await
            .map_err(|error| BootError::Internal(format!("failed to save session: {error}")))?;
        self.state
            .sessions
            .lock()
            .await
            .insert(session_id.to_string(), new_session);
        self.state
            .messages
            .lock()
            .await
            .insert(session_id.to_string(), Vec::new());
        self.state
            .session_contexts
            .lock()
            .await
            .insert(session_id.to_string(), {
                let mut context = CodeWebSessionContext::default();
                context.set_llm_client(llm_client);
                context
            });
        self.persist_session_state(session_id).await?;
        Ok(json!({
            "sessionId": session_id,
            "cleared": true,
            "items": [],
            "total": 0,
        }))
    }

    pub(in crate::api::code_web) async fn run_session_message(
        &self,
        session_id: &str,
        request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        let content = request
            .get("content")
            .or_else(|| request.get("message"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| BootError::BadRequest("content is required".to_string()))?;
        let session = self.kernel_session(session_id).await?;
        let prompt = self.compose_session_prompt(session_id, content).await;
        let history = self.model_history_for_session(session.as_ref()).await?;
        self.append_message(session_id, "user", content, None)
            .await?;
        let result = run_code_web_stream(session.as_ref(), &prompt, &history)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let core_summary = result
            .compact_summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        if let Some(summary) = core_summary.as_deref() {
            self.maybe_auto_compact(
                session_id,
                session.as_ref(),
                result.last_prompt_tokens,
                Some(summary),
            )
            .await;
        }
        self.append_message_with_events(
            session_id,
            "assistant",
            &result.text,
            self.session_response_model(session_id).await,
            &result.events,
        )
        .await?;
        if core_summary.is_none() {
            self.maybe_auto_compact(
                session_id,
                session.as_ref(),
                result.last_prompt_tokens,
                None,
            )
            .await;
        }
        if let Err(error) =
            refresh_evolution_runtime_after_turn(self.state.as_ref(), session.workspace()).await
        {
            tracing::warn!(%error, session_id, "could not refresh learned Web session assets after turn");
        }
        Ok(json!({
            "sessionId": session_id,
            "accepted": true,
            "events": result.events,
            "completedAt": chrono::Utc::now().to_rfc3339(),
        }))
    }

    pub(in crate::api::code_web) async fn chat(
        &self,
        request: ChatRequest,
    ) -> BootResult<ChatResponse> {
        let message = request.message.trim().to_string();
        if message.is_empty() {
            return Err(BootError::BadRequest("message cannot be empty".to_string()));
        }

        let session_request = CreateSessionRequest {
            workspace: request.workspace,
            cwd: None,
            model: request.model,
            follow_default_model: None,
            permission_mode: None,
            planning_mode: None,
            goal_tracking: None,
            title: None,
            agent_id: None,
        };
        let session = self
            .create_or_get_session(request.session_id, session_request)
            .await?;
        let session_id = session.session_id().to_string();
        let prompt = self.compose_session_prompt(&session_id, &message).await;
        let history = self.model_history_for_session(session.as_ref()).await?;
        self.append_message(&session_id, "user", &message, None)
            .await?;
        let result = run_code_web_stream(session.as_ref(), &prompt, &history)
            .await
            .map_err(|e| BootError::Internal(e.to_string()))?;
        let core_summary = result
            .compact_summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        if let Some(summary) = core_summary.as_deref() {
            self.maybe_auto_compact(
                &session_id,
                session.as_ref(),
                result.last_prompt_tokens,
                Some(summary),
            )
            .await;
        }
        self.append_message_with_events(
            &session_id,
            "assistant",
            &result.text,
            self.session_response_model(&session_id).await,
            &result.events,
        )
        .await?;
        if core_summary.is_none() {
            self.maybe_auto_compact(
                &session_id,
                session.as_ref(),
                result.last_prompt_tokens,
                None,
            )
            .await;
        }
        if let Err(error) =
            refresh_evolution_runtime_after_turn(self.state.as_ref(), session.workspace()).await
        {
            tracing::warn!(%error, %session_id, "could not refresh learned Web session assets after turn");
        }

        Ok(ChatResponse {
            session_id,
            workspace: session.workspace().display().to_string(),
            model: self.session_response_model(session.session_id()).await,
            text: result.text,
            usage: UsageResponse::from_usage(result.usage),
            tool_calls_count: result.tool_calls_count,
        })
    }

    pub(super) async fn compose_session_prompt(
        &self,
        session_id: &str,
        user_prompt: &str,
    ) -> String {
        if user_prompt.trim_start().starts_with('/') {
            return user_prompt.to_string();
        }

        let controls = self.session_controls_snapshot(session_id).await;
        compose_controlled_prompt(&controls, user_prompt)
    }

    pub(super) async fn model_history_for_session(
        &self,
        session: &AgentSession,
    ) -> BootResult<Vec<Message>> {
        let session_id = session.session_id();
        let store_dir = code_web_store_dir(session.workspace());
        let timeline_store =
            crate::timeline::TimelineJsonlStore::for_session(&store_dir, session_id);
        let context_limit = self.context_limit_for_session(session_id).await;
        let threshold = self.state.auto_compact_threshold;
        let mut metadata = timeline_store
            .metadata()
            .map_err(|error| BootError::Internal(error.to_string()))?;
        if metadata.source_message_count == 0 {
            let legacy_history = session.history();
            if !legacy_history.is_empty() {
                seed_code_web_timeline(
                    &store_dir,
                    session_id,
                    &legacy_history,
                    context_limit,
                    threshold,
                )
                .map_err(|error| BootError::Internal(error.to_string()))?;
                metadata = timeline_store
                    .metadata()
                    .map_err(|error| BootError::Internal(error.to_string()))?;
            }
        }

        let context_store = crate::compact::ContextJsonStore::for_session(&store_dir, session_id);
        if let Some(mut context) = context_store
            .load()
            .map_err(|error| BootError::Internal(error.to_string()))?
            .filter(|context| {
                context.matches_timeline(metadata)
                    && context.context_limit as usize == context_limit
            })
        {
            context.update_runtime_metadata(
                context.last_prompt_tokens,
                context_limit.min(u32::MAX as usize) as u32,
                threshold,
            );
            context_store
                .save(&context)
                .map_err(|error| BootError::Internal(error.to_string()))?;
            return Ok(context.messages);
        }

        let timeline = timeline_store
            .load_all()
            .map(|events| crate::timeline::messages_from_events(&events))
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let context = crate::compact::ModelContextState::rebuild_from_timeline_with_metadata(
            &timeline,
            crate::compact::ProjectionBudget::for_token_limit(context_limit),
            metadata,
            0,
            context_limit.min(u32::MAX as usize) as u32,
            threshold,
        );
        context_store
            .save(&context)
            .map_err(|error| BootError::Internal(error.to_string()))?;
        Ok(context.messages)
    }

    pub(super) async fn maybe_auto_compact(
        &self,
        session_id: &str,
        session: &AgentSession,
        last_prompt_tokens: usize,
        core_summary: Option<&str>,
    ) {
        let context_limit = self.context_limit_for_session(session_id).await;
        let threshold = self.state.auto_compact_threshold;
        if last_prompt_tokens > 0 {
            self.persist_code_web_context_usage(
                session_id,
                session.workspace(),
                last_prompt_tokens,
                context_limit,
                threshold,
            );
        }

        if let Some(summary) = core_summary.filter(|value| !value.trim().is_empty()) {
            let result = self
                .finalize_code_web_compact(session_id, session, summary.trim())
                .await;
            let mut contexts = self.state.session_contexts.lock().await;
            let context = contexts.entry(session_id.to_string()).or_default();
            let controller = context.auto_compact.get_or_insert_with(|| {
                crate::compact::auto_compact::AutoCompactController::new(
                    threshold,
                    context_limit.min(u32::MAX as usize) as u32,
                )
            });
            if result.is_ok() {
                controller.finish_success(0);
            } else {
                controller.finish_failure();
            }
            drop(contexts);
            if let Err(error) = result {
                eprintln!("warning: failed to persist Core compaction for {session_id}: {error}");
            }
            return;
        }

        if last_prompt_tokens == 0 {
            return;
        }
        let should_compact = {
            let mut contexts = self.state.session_contexts.lock().await;
            let context = contexts.entry(session_id.to_string()).or_default();
            let controller = context.auto_compact.get_or_insert_with(|| {
                crate::compact::auto_compact::AutoCompactController::new(
                    threshold,
                    context_limit.min(u32::MAX as usize) as u32,
                )
            });
            controller.update_policy(threshold, context_limit.min(u32::MAX as usize) as u32);
            controller.observe_prompt_tokens(last_prompt_tokens) && controller.start()
        };
        if !should_compact {
            return;
        }

        let timeline_store = crate::timeline::TimelineJsonlStore::for_session(
            code_web_store_dir(session.workspace()),
            session_id,
        );
        let result = match self.session_llm_client(session_id).await {
            Ok(llm_client) => {
                match crate::compact::compact_timeline(llm_client, &timeline_store).await {
                    Ok(Some(summary)) => {
                        self.finalize_code_web_compact(session_id, session, &summary)
                            .await
                    }
                    Ok(None) => Err(BootError::Internal(
                        "automatic compact found an empty timeline".to_string(),
                    )),
                    Err(error) => Err(BootError::Internal(error)),
                }
            }
            Err(error) => Err(error),
        };

        let mut contexts = self.state.session_contexts.lock().await;
        let context = contexts.entry(session_id.to_string()).or_default();
        if let Some(controller) = context.auto_compact.as_mut() {
            if result.is_ok() {
                controller.finish_success(0);
            } else {
                controller.finish_failure();
            }
        }
        if let Err(error) = result {
            eprintln!("warning: automatic compact failed for {session_id}: {error}");
        }
    }

    pub(super) async fn finalize_code_web_compact(
        &self,
        session_id: &str,
        session: &AgentSession,
        summary: &str,
    ) -> BootResult<()> {
        let workspace = session.workspace().to_path_buf();
        let context_limit = self.context_limit_for_session(session_id).await;
        let threshold = self.state.auto_compact_threshold;
        persist_code_web_compact_summary(
            &code_web_store_dir(&workspace),
            session_id,
            summary,
            context_limit,
            threshold,
        )
        .map_err(|error| BootError::Internal(error.to_string()))?;

        let controls = self.session_controls_snapshot(session_id).await;
        let settings = self.session_settings_snapshot(session_id).await;
        let (options, runtime, llm_client) = code_web_session_options(
            self.state.as_ref(),
            &workspace,
            Some(session_id),
            self.effective_model(&settings),
            &controls,
            &settings,
        )
        .await?;
        let new_session = Arc::new(
            self.state
                .agent
                .replace_session_async(session, options)
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
        {
            let mut messages_by_session = self.state.messages.lock().await;
            let current_messages = messages_by_session.remove(session_id).unwrap_or_default();
            messages_by_session.insert(
                session_id.to_string(),
                compact_visible_messages_after_success(session_id, current_messages, summary),
            );
        }
        let mut contexts = self.state.session_contexts.lock().await;
        let context = contexts.entry(session_id.to_string()).or_default();
        context.compact_summary = Some(summary.to_string());
        context.set_llm_client(llm_client);
        drop(contexts);
        self.persist_session_state(session_id).await
    }

    fn persist_code_web_context_usage(
        &self,
        session_id: &str,
        workspace: &Path,
        last_prompt_tokens: usize,
        context_limit: usize,
        threshold: f64,
    ) {
        let store = crate::compact::ContextJsonStore::for_session(
            code_web_store_dir(workspace),
            session_id,
        );
        if let Ok(Some(mut context)) = store.load() {
            context.update_runtime_metadata(
                last_prompt_tokens,
                context_limit.min(u32::MAX as usize) as u32,
                threshold,
            );
            let _ = store.save(&context);
        }
    }

    pub(super) async fn append_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        model: Option<String>,
    ) -> BootResult<()> {
        self.append_message_with_events(session_id, role, content, model, &[])
            .await
    }

    pub(super) async fn append_message_with_events(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        model: Option<String>,
        events: &[AgentEvent],
    ) -> BootResult<()> {
        let events = if events.is_empty() {
            None
        } else {
            Some(
                serde_json::to_value(events)
                    .map_err(|error| BootError::Internal(error.to_string()))?,
            )
        };
        self.save_code_web_message_to_timeline(session_id, &message_for_role(role, content))
            .await?;
        self.append_visible_message_with_events(session_id, role, content, model, events)
            .await;
        self.persist_session_state(session_id).await
    }

    pub(super) async fn append_visible_message(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        model: Option<String>,
    ) {
        self.append_visible_message_with_events(session_id, role, content, model, None)
            .await;
    }

    async fn append_visible_message_with_events(
        &self,
        session_id: &str,
        role: &str,
        content: &str,
        model: Option<String>,
        events: Option<Value>,
    ) {
        let message = visible_message_json(session_id, role, content, model, events);
        self.state
            .messages
            .lock()
            .await
            .entry(session_id.to_string())
            .or_default()
            .push(message);
    }

    pub(super) async fn context_limit_for_session(&self, session_id: &str) -> usize {
        let settings = self.session_settings_snapshot(session_id).await;
        let model = self.effective_model(&settings);
        code_web_context_limit_for_model(self.state.as_ref(), model.as_deref()) as usize
    }
}

pub(super) fn visible_message_json(
    session_id: &str,
    role: &str,
    content: &str,
    model: Option<String>,
    events: Option<Value>,
) -> Value {
    let id = format!("{}-{}", chrono::Utc::now().timestamp_millis(), role);
    let mut message = json!({
        "id": id,
        "sessionId": session_id,
        "role": role,
        "content": content,
        "createdAt": chrono::Utc::now().to_rfc3339(),
    });
    if let Some(model) = model {
        message["model"] = Value::String(model);
    }
    if let Some(events) = events {
        message["events"] = events;
    }
    message
}

fn message_for_role(role: &str, content: &str) -> Message {
    match role {
        "assistant" => Message::assistant(content),
        "user" => Message::user(content),
        _ => Message {
            role: role.to_string(),
            content: vec![ContentBlock::Text {
                text: content.to_string(),
            }],
            reasoning_content: None,
        },
    }
}
