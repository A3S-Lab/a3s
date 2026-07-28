use super::maintenance::message_text;
use super::text::truncate_chars;
use super::*;

struct RestoredSessionInstall {
    messages: Vec<Value>,
    metadata: CodeWebSessionMetadata,
    controls: CodeWebSessionControls,
    context: CodeWebSessionContext,
    turn_queue: CodeWebStoredTurnQueue,
    settings: CodeWebSessionSettings,
    llm_client: Arc<dyn a3s_code_core::LlmClient>,
}

#[derive(Clone, Copy, Debug, Default)]
pub(in crate::api) struct SessionRestoreReport {
    pub(in crate::api) restored: usize,
    pub(in crate::api) unavailable: usize,
    pub(in crate::api) failed: usize,
}

impl KernelService {
    pub(in crate::api) async fn restore_persisted_sessions(
        &self,
    ) -> BootResult<SessionRestoreReport> {
        let session_ids = self
            .state
            .session_repository
            .list_session_ids()
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let mut report = SessionRestoreReport::default();
        for session_id in session_ids {
            match self.restore_persisted_session(&session_id).await {
                Ok(true) => report.restored += 1,
                Ok(false) => {}
                Err(error) => {
                    if is_unavailable_session_error(&error) {
                        report.unavailable += 1;
                    } else {
                        report.failed += 1;
                    }
                    tracing::warn!(
                        session_id,
                        error = %error,
                        "saved Code Web session was not restored"
                    );
                }
            }
        }
        report.restored += self.migrate_default_workspace_timelines().await;
        if report.unavailable > 0 {
            eprintln!(
                "warning: {} saved Code Web session(s) use models that are unavailable in the \
                 current configuration; their saved data was kept",
                report.unavailable
            );
        }
        if report.failed > 0 {
            eprintln!(
                "warning: {} saved Code Web session(s) could not be restored; their saved data was \
                 kept",
                report.failed
            );
        }
        Ok(report)
    }

    async fn restore_persisted_session(&self, session_id: &str) -> BootResult<bool> {
        let stored = self
            .state
            .session_repository
            .load_web_session(session_id)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let core = self
            .state
            .session_repository
            .load_core_session(session_id)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        if stored.is_none() && core.is_none() {
            return Ok(false);
        }

        let workspace = stored
            .as_ref()
            .map(|stored| stored.metadata.workspace.trim())
            .filter(|workspace| !workspace.is_empty())
            .map(PathBuf::from)
            .or_else(|| {
                core.as_ref()
                    .map(|session| session.config.workspace.trim())
                    .filter(|workspace| !workspace.is_empty())
                    .map(PathBuf::from)
            })
            .ok_or_else(|| {
                BootError::Internal(format!("persisted session `{session_id}` has no workspace"))
            })?;
        let settings = stored
            .as_ref()
            .map(|stored| stored.settings.clone())
            .unwrap_or_else(|| settings_from_core_session(core.as_ref()));
        let controls = stored
            .as_ref()
            .map(|stored| stored.controls.clone())
            .unwrap_or_default();
        let stored_context = stored
            .as_ref()
            .map(|stored| stored.context.clone())
            .unwrap_or_default();
        let stored_turn_queue = stored
            .as_ref()
            .map(|stored| stored.turn_queue.clone())
            .unwrap_or_default();
        let messages = stored
            .as_ref()
            .map(|stored| stored.messages.clone())
            .filter(|messages| !messages.is_empty())
            .unwrap_or_else(|| {
                visible_messages_from_history(
                    session_id,
                    core.as_ref()
                        .map(|session| session.messages.as_slice())
                        .unwrap_or_default(),
                )
            });
        let mut metadata = stored
            .as_ref()
            .map(|stored| stored.metadata.clone())
            .unwrap_or_default();
        let core_created_at = core
            .as_ref()
            .map(|session| session.created_at.saturating_mul(1000))
            .unwrap_or_default();
        let core_updated_at = core
            .as_ref()
            .map(|session| session.updated_at.saturating_mul(1000))
            .unwrap_or_default();
        if metadata.created_at <= 0 {
            metadata.created_at = if core_created_at > 0 {
                core_created_at
            } else {
                chrono::Utc::now().timestamp_millis()
            };
        }
        if metadata.updated_at <= 0 {
            metadata.updated_at = if core_updated_at > 0 {
                core_updated_at
            } else {
                metadata.created_at
            };
        }
        if metadata.title.is_none() {
            metadata.title = recovered_session_title(&messages);
        }
        if metadata.agent_id.is_none() {
            metadata.agent_id = Some("default".to_string());
        }

        let (options, runtime, llm_client) = code_web_session_options(
            self.state.as_ref(),
            &workspace,
            Some(session_id),
            self.effective_model(&settings),
            &controls,
            &settings,
        )
        .await?;
        let (session, created_fresh) = if core.is_some() {
            match self
                .state
                .agent
                .resume_session_async(session_id, options.clone())
                .await
            {
                Ok(session) => (session, false),
                Err(error) => {
                    eprintln!(
                        "warning: Core snapshot for `{session_id}` could not be resumed; using the Code Web transcript: {error}"
                    );
                    (
                        self.state
                            .agent
                            .session_async(workspace.display().to_string(), Some(options))
                            .await
                            .map_err(|error| BootError::Internal(error.to_string()))?,
                        true,
                    )
                }
            }
        } else {
            (
                self.state
                    .agent
                    .session_async(workspace.display().to_string(), Some(options))
                    .await
                    .map_err(|error| BootError::Internal(error.to_string()))?,
                true,
            )
        };
        let session = Arc::new(session);
        activate_session_runtime(session.as_ref(), &runtime);
        metadata.workspace = session.workspace().display().to_string();
        self.install_restored_session(
            Arc::clone(&session),
            RestoredSessionInstall {
                messages: messages.clone(),
                metadata: metadata.clone(),
                controls: controls.clone(),
                context: CodeWebSessionContext {
                    compact_summary: stored_context.compact_summary.clone(),
                    ..CodeWebSessionContext::default()
                },
                turn_queue: stored_turn_queue.clone(),
                settings: settings.clone(),
                llm_client,
            },
        )
        .await;
        if created_fresh {
            session
                .save()
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?;
        }
        self.state
            .session_repository
            .save_web_session(&CodeWebStoredSession::new(
                session_id.to_string(),
                metadata,
                messages,
                controls,
                stored_context,
                stored_turn_queue,
                settings,
            ))
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        Ok(true)
    }

    async fn migrate_default_workspace_timelines(&self) -> usize {
        let workspace = self.state.default_workspace.clone();
        let store_dir = code_web_store_dir(&workspace);
        let timeline_dir = store_dir.join("timelines");
        let mut entries = match tokio::fs::read_dir(&timeline_dir).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return 0,
            Err(error) => {
                eprintln!(
                    "warning: failed to scan legacy Code Web timelines at {}: {error}",
                    timeline_dir.display()
                );
                return 0;
            }
        };
        let mut migrated = 0;
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path
                .extension()
                .is_none_or(|extension| extension != "jsonl")
            {
                continue;
            }
            let Some(session_id) = path
                .file_stem()
                .and_then(|stem| stem.to_str())
                .map(ToOwned::to_owned)
            else {
                continue;
            };
            if self.state.sessions.lock().await.contains_key(&session_id) {
                continue;
            }
            let history =
                match crate::timeline::TimelineJsonlStore::for_session(&store_dir, &session_id)
                    .load_all()
                {
                    Ok(events) => crate::timeline::messages_from_events(&events),
                    Err(error) => {
                        eprintln!(
                        "warning: failed to read legacy Code Web timeline `{session_id}`: {error}"
                    );
                        continue;
                    }
                };
            let messages = visible_messages_from_history(&session_id, &history);
            if messages.is_empty() {
                continue;
            }
            let timestamp = file_timestamp_millis(&path)
                .unwrap_or_else(|| chrono::Utc::now().timestamp_millis());
            let settings = CodeWebSessionSettings::default();
            let controls = CodeWebSessionControls::default();
            let metadata = CodeWebSessionMetadata {
                workspace: workspace.display().to_string(),
                title: recovered_session_title(&messages),
                agent_id: Some("default".to_string()),
                created_at: timestamp,
                updated_at: timestamp,
            };
            let (options, runtime, llm_client) = match code_web_session_options(
                self.state.as_ref(),
                &workspace,
                Some(&session_id),
                self.effective_model(&settings),
                &controls,
                &settings,
            )
            .await
            {
                Ok(result) => result,
                Err(error) => {
                    eprintln!(
                        "warning: failed to prepare legacy Code Web timeline `{session_id}`: {error}"
                    );
                    continue;
                }
            };
            let session = match self
                .state
                .agent
                .session_async(workspace.display().to_string(), Some(options))
                .await
            {
                Ok(session) => Arc::new(session),
                Err(error) => {
                    eprintln!(
                        "warning: failed to migrate legacy Code Web timeline `{session_id}`: {error}"
                    );
                    continue;
                }
            };
            activate_session_runtime(session.as_ref(), &runtime);
            self.install_restored_session(
                Arc::clone(&session),
                RestoredSessionInstall {
                    messages,
                    metadata,
                    controls,
                    context: CodeWebSessionContext::default(),
                    turn_queue: CodeWebStoredTurnQueue::default(),
                    settings,
                    llm_client,
                },
            )
            .await;
            if let Err(error) = session.save().await {
                eprintln!(
                    "warning: failed to save migrated Code Web timeline `{session_id}`: {error}"
                );
                continue;
            }
            if let Err(error) = self.persist_session_state(&session_id).await {
                eprintln!(
                    "warning: failed to index migrated Code Web timeline `{session_id}`: {error}"
                );
                continue;
            }
            migrated += 1;
        }
        migrated
    }

    async fn install_restored_session(
        &self,
        session: Arc<AgentSession>,
        restored: RestoredSessionInstall,
    ) {
        let RestoredSessionInstall {
            messages,
            metadata,
            controls,
            mut context,
            turn_queue,
            settings,
            llm_client,
        } = restored;
        let session_id = session.session_id().to_string();
        context.set_llm_client(llm_client);
        self.state
            .sessions
            .lock()
            .await
            .insert(session_id.clone(), session);
        self.state
            .messages
            .lock()
            .await
            .insert(session_id.clone(), messages);
        self.state
            .session_metadata
            .lock()
            .await
            .insert(session_id.clone(), metadata);
        self.state
            .session_controls
            .lock()
            .await
            .insert(session_id.clone(), controls);
        self.state
            .session_contexts
            .lock()
            .await
            .insert(session_id.clone(), context);
        self.state
            .session_settings
            .lock()
            .await
            .insert(session_id.clone(), settings);
        self.state
            .session_turn_queues
            .lock()
            .await
            .insert(session_id, CodeWebSessionTurnQueue::restore(turn_queue));
    }

    pub(super) async fn persist_session_state(&self, session_id: &str) -> BootResult<()> {
        let _persist_guard = self.state.session_persist_lock.lock().await;
        let session = self.kernel_session(session_id).await?;
        let metadata = {
            let mut metadata_by_session = self.state.session_metadata.lock().await;
            let metadata = metadata_by_session
                .entry(session_id.to_string())
                .or_insert_with(CodeWebSessionMetadata::default);
            if metadata.workspace.is_empty() {
                metadata.workspace = session.workspace().display().to_string();
            }
            if metadata.created_at <= 0 {
                metadata.created_at = chrono::Utc::now().timestamp_millis();
            }
            metadata.updated_at = chrono::Utc::now().timestamp_millis();
            metadata.clone()
        };
        let messages = self
            .state
            .messages
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        let controls = self.session_controls_snapshot(session_id).await;
        let context = self.session_context_snapshot(session_id).await;
        let settings = self.session_settings_snapshot(session_id).await;
        let turn_queue = self.session_turn_queue_snapshot(session_id).await;
        let stored = CodeWebStoredSession::new(
            session_id.to_string(),
            metadata,
            messages,
            controls,
            CodeWebStoredContext {
                compact_summary: context.compact_summary,
            },
            turn_queue,
            settings,
        );
        self.state
            .session_repository
            .save_web_session(&stored)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))
    }

    pub(super) async fn save_code_web_message_to_timeline(
        &self,
        session_id: &str,
        message: &Message,
    ) -> BootResult<()> {
        let session = self.kernel_session(session_id).await?;
        let store_dir = code_web_store_dir(session.workspace());
        let context_limit = self.context_limit_for_session(session_id).await;
        save_code_web_timeline_message(
            &store_dir,
            session_id,
            message,
            context_limit,
            self.state.auto_compact_threshold,
        )
        .map_err(|error| BootError::Internal(error.to_string()))
    }
}

fn is_unavailable_session_error(error: &BootError) -> bool {
    let message = error.to_string();
    message.contains("not found in config")
        || message.contains("was not found in provider")
        || message.contains("was not found, or has no API key")
}

pub(super) fn code_web_store_dir(workspace: &Path) -> PathBuf {
    workspace.join(".a3s").join("tui-sessions")
}

pub(super) fn seed_code_web_timeline(
    store_dir: &Path,
    session_id: &str,
    history: &[Message],
    context_limit: usize,
    auto_compact_threshold: f64,
) -> anyhow::Result<()> {
    let timeline_store = crate::timeline::TimelineJsonlStore::for_session(store_dir, session_id);
    if timeline_store.metadata()?.source_message_count > 0 {
        return Ok(());
    }

    let mut source_event_count = 0;
    for (message_index, message) in history.iter().enumerate() {
        let events = crate::timeline::events_for_message(
            session_id,
            message_index,
            source_event_count as u64,
            message,
            chrono::Utc::now().timestamp_millis(),
        );
        source_event_count += events.len();
        for event in &events {
            timeline_store.append(event)?;
        }
    }
    let metadata = crate::timeline::TimelineMetadata {
        source_file_bytes: timeline_store.file_len()?,
        source_event_count,
        source_message_count: history.len(),
        active_summary_index: history.iter().rposition(crate::compact::is_compact_message),
        compact_generation: history
            .iter()
            .filter(|message| crate::compact::is_compact_message(message))
            .count() as u32,
    };
    let context = crate::compact::ModelContextState::rebuild_from_timeline_with_metadata(
        history,
        crate::compact::ProjectionBudget::for_token_limit(context_limit),
        metadata,
        0,
        context_limit.min(u32::MAX as usize) as u32,
        auto_compact_threshold,
    );
    crate::compact::ContextJsonStore::for_session(store_dir, session_id).save(&context)
}

pub(super) fn save_code_web_timeline_message(
    store_dir: &Path,
    session_id: &str,
    message: &Message,
    context_limit: usize,
    auto_compact_threshold: f64,
) -> anyhow::Result<()> {
    let timeline_store = crate::timeline::TimelineJsonlStore::for_session(store_dir, session_id);
    let context_store = crate::compact::ContextJsonStore::for_session(store_dir, session_id);
    let source_file_bytes = timeline_store.file_len()?;
    let cached_context = context_store.load()?.filter(|context| {
        context.context_version == 2
            && context.source_file_bytes == source_file_bytes
            && context.context_limit as usize == context_limit
    });
    let next_index = cached_context
        .as_ref()
        .map(|context| context.source_message_count)
        .unwrap_or(0);
    let next_seq = cached_context
        .as_ref()
        .map(|context| context.source_event_count as u64)
        .unwrap_or(0);
    let appended_events = crate::timeline::events_for_message(
        session_id,
        next_index,
        next_seq,
        message,
        chrono::Utc::now().timestamp_millis(),
    );
    for event in &appended_events {
        timeline_store.append(event)?;
    }

    let budget = crate::compact::ProjectionBudget::for_token_limit(context_limit);
    if let Some(mut context) = cached_context {
        let last_prompt_tokens = context.last_prompt_tokens;
        context.append_timeline_message(
            message,
            appended_events.len(),
            timeline_store.file_len()?,
            budget,
        );
        context.update_runtime_metadata(
            last_prompt_tokens,
            context_limit.min(u32::MAX as usize) as u32,
            auto_compact_threshold,
        );
        context_store.save(&context)?;
        return Ok(());
    }

    let all_events = timeline_store.load_all()?;
    let timeline_messages = crate::timeline::messages_from_events(&all_events);
    let metadata = timeline_store.metadata()?;
    let context = crate::compact::ModelContextState::rebuild_from_timeline_with_metadata(
        &timeline_messages,
        budget,
        metadata,
        0,
        context_limit.min(u32::MAX as usize) as u32,
        auto_compact_threshold,
    );
    context_store.save(&context)?;
    Ok(())
}

pub(super) fn persist_code_web_compact_summary(
    store_dir: &Path,
    session_id: &str,
    summary: &str,
    context_limit: usize,
    auto_compact_threshold: f64,
) -> anyhow::Result<()> {
    let mut messages = Vec::new();
    crate::compact::append_compact_summary(&mut messages, summary);
    let Some(message) = messages.first() else {
        return Ok(());
    };
    save_code_web_timeline_message(
        store_dir,
        session_id,
        message,
        context_limit,
        auto_compact_threshold,
    )
}

fn settings_from_core_session(session: Option<&SessionData>) -> CodeWebSessionSettings {
    let Some(session) = session else {
        return CodeWebSessionSettings::default();
    };
    let model = session
        .llm_config
        .as_ref()
        .map(|config| format!("{}/{}", config.provider, config.model))
        .or_else(|| session.model_name.clone());
    let permission_mode = if session
        .config
        .permission_policy
        .as_ref()
        .is_some_and(|policy| !policy.enabled)
        || session
            .config
            .confirmation_policy
            .as_ref()
            .is_some_and(|policy| !policy.enabled)
    {
        // Compatibility for legacy sessions that encoded auto mode by disabling
        // one or both safety layers. New sessions keep both layers enabled and
        // persist the explicit web setting separately.
        "auto"
    } else {
        "default"
    };
    CodeWebSessionSettings {
        follow_default_model: model.is_none(),
        model,
        permission_mode: permission_mode.to_string(),
        planning_mode: Some(
            match session.config.planning_mode {
                PlanningMode::Auto => "auto",
                PlanningMode::Enabled => "enabled",
                PlanningMode::Disabled => "disabled",
            }
            .to_string(),
        ),
        goal_tracking: Some(session.config.goal_tracking),
    }
}

fn visible_messages_from_history(session_id: &str, history: &[Message]) -> Vec<Value> {
    let now = chrono::Utc::now();
    history
        .iter()
        .filter(|message| !crate::compact::is_compact_message(message))
        .enumerate()
        .map(|(index, message)| {
            let content = message_text(message);
            let mut visible = json!({
                "id": format!("{session_id}-restored-{index}"),
                "sessionId": session_id,
                "role": message.role.clone(),
                "content": content,
                "createdAt": (now + chrono::Duration::milliseconds(index as i64)).to_rfc3339(),
                "_a3sProjection": true,
            });
            if message.content.iter().any(|block| {
                !matches!(
                    block,
                    ContentBlock::Text { .. } | ContentBlock::Image { .. }
                )
            }) {
                if let Ok(blocks) = serde_json::to_value(&message.content) {
                    visible["contentBlocks"] = blocks;
                }
            }
            visible
        })
        .collect()
}

fn recovered_session_title(messages: &[Value]) -> Option<String> {
    messages.iter().find_map(|message| {
        if message.get("role").and_then(Value::as_str) != Some("user") {
            return None;
        }
        let content = message.get("content").and_then(Value::as_str)?.trim();
        (!content.is_empty()).then(|| truncate_chars(content.lines().next().unwrap_or(content), 56))
    })
}

fn file_timestamp_millis(path: &Path) -> Option<i64> {
    let modified = std::fs::metadata(path).ok()?.modified().ok()?;
    let duration = modified.duration_since(std::time::UNIX_EPOCH).ok()?;
    Some(duration.as_millis().min(i64::MAX as u128) as i64)
}

#[cfg(test)]
mod restore_report_tests {
    use super::*;

    #[test]
    fn missing_model_errors_are_grouped_as_unavailable_sessions() {
        let error = BootError::Internal(
            "provider 'fixture' or model 'removed' not found in config".to_string(),
        );

        assert!(is_unavailable_session_error(&error));
    }

    #[test]
    fn storage_failures_are_not_misreported_as_model_availability() {
        let error = BootError::Internal("persisted session JSON is truncated".to_string());

        assert!(!is_unavailable_session_error(&error));
    }
}
