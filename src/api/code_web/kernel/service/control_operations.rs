use super::persistence::code_web_store_dir;
use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn session_status(
        &self,
        session_id: &str,
    ) -> BootResult<serde_json::Value> {
        let session = self.kernel_session(session_id).await?;
        let settings = self.session_settings_snapshot(session_id).await;
        let os_status = code_web_os_status(self.state.as_ref()).await?;
        let runtime_connected = os_status
            .get("runtimeToolActive")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        Ok(json!({
            "sessionId": session.session_id(),
            "status": "idle",
            "cwd": session.workspace().display().to_string(),
            "model": self.effective_model(&settings),
            "followDefaultModel": settings.follow_default_model,
            "permissionMode": settings.permission_mode,
            "planningMode": settings.planning_mode,
            "goalTracking": settings.goal_tracking,
            "mcpServers": [],
            "runtime": {
                "connected": runtime_connected,
                "transport": "rest",
                "os": os_status,
            },
            "commands": ["clear", "compact", "cost", "help", "history", "mcp", "model", "tools"],
        }))
    }

    pub(in crate::api::code_web) async fn effort_levels(&self) -> BootResult<serde_json::Value> {
        Ok(json!({
            "items": effort_levels_json(),
        }))
    }

    pub(in crate::api::code_web) async fn session_controls(
        &self,
        session_id: &str,
    ) -> BootResult<serde_json::Value> {
        let session = self.kernel_session(session_id).await?;
        let controls = self.session_controls_snapshot(session_id).await;
        let settings = self.session_settings_snapshot(session_id).await;
        let model = self.effective_model(&settings);
        let context_limit = code_web_context_limit_for_model(self.state.as_ref(), model.as_deref());
        let context = self
            .controls_context_usage(session_id, session.as_ref(), context_limit)
            .await?;
        Ok(controls_json(
            session_id,
            &controls,
            &settings,
            &context,
            Some(context_limit),
            self.state.auto_compact_threshold,
        ))
    }

    pub(in crate::api::code_web) async fn update_session_controls(
        &self,
        session_id: &str,
        request: serde_json::Value,
    ) -> BootResult<serde_json::Value> {
        self.kernel_session(session_id).await?;
        let (controls_snapshot, effort_changed, goal_changed) = {
            let mut controls_by_session = self.state.session_controls.lock().await;
            let controls = controls_by_session
                .entry(session_id.to_string())
                .or_default();
            let original_effort = controls.effort.clone();
            let original_goal = controls.goal.clone();

            if let Some(effort_value) = request.get("effort") {
                let effort = effort_value
                    .as_str()
                    .ok_or_else(|| BootError::BadRequest("effort must be a string".to_string()))?;
                let profile = normalize_effort(effort).ok_or_else(|| {
                    BootError::BadRequest(format!("unsupported effort level `{effort}`"))
                })?;
                controls.effort = profile.id.to_string();
            }

            if let Some(goal_value) = request.get("goal") {
                match goal_value {
                    Value::Null => {
                        controls.goal = None;
                        controls.goal_run = None;
                    }
                    Value::String(goal) => {
                        let goal = normalize_goal(goal);
                        if controls.goal != goal {
                            controls.goal_run = goal.as_ref().map(|_| {
                                let now = chrono::Utc::now().timestamp_millis();
                                CodeWebGoalRun {
                                    started_at: now,
                                    updated_at: now,
                                    ..CodeWebGoalRun::default()
                                }
                            });
                        }
                        controls.goal = goal;
                    }
                    _ => {
                        return Err(BootError::BadRequest(
                            "goal must be a string or null".to_string(),
                        ));
                    }
                }
            }

            (
                controls.clone(),
                controls.effort != original_effort,
                controls.goal != original_goal,
            )
        };

        if goal_changed {
            self.state
                .session_settings
                .lock()
                .await
                .entry(session_id.to_string())
                .or_default()
                .goal_tracking = Some(controls_snapshot.goal.is_some());
        }

        if effort_changed || goal_changed {
            let settings = self.session_settings_snapshot(session_id).await;
            self.rebuild_session_with_settings(session_id, &settings)
                .await?;
        }

        self.persist_session_state(session_id).await?;

        let settings = self.session_settings_snapshot(session_id).await;
        let model = self.effective_model(&settings);
        let context_limit = code_web_context_limit_for_model(self.state.as_ref(), model.as_deref());
        let session = self.kernel_session(session_id).await?;
        let context = self
            .controls_context_usage(session_id, session.as_ref(), context_limit)
            .await?;
        Ok(controls_json(
            session_id,
            &controls_snapshot,
            &settings,
            &context,
            Some(context_limit),
            self.state.auto_compact_threshold,
        ))
    }

    async fn controls_context_usage(
        &self,
        session_id: &str,
        session: &AgentSession,
        context_limit: u32,
    ) -> BootResult<CodeWebContextUsage> {
        let stored = crate::compact::ContextJsonStore::for_session(
            code_web_store_dir(session.workspace()),
            session_id,
        )
        .load()
        .map_err(|error| BootError::Internal(error.to_string()))?;
        let compact_summary = self
            .session_context_snapshot(session_id)
            .await
            .compact_summary;

        Ok(CodeWebContextUsage {
            estimated_tokens: stored
                .as_ref()
                .map(|context| context.last_prompt_tokens)
                .unwrap_or_default(),
            limit_tokens: context_limit,
            history_messages: stored
                .as_ref()
                .map(|context| context.source_message_count)
                .unwrap_or_default(),
            compacted: compact_summary.is_some()
                || stored
                    .as_ref()
                    .is_some_and(|context| context.compact_generation > 0),
            compact_summary,
        })
    }
}
