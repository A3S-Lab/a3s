use super::persistence::code_web_store_dir;
use super::streaming::run_code_web_stream;
use super::text::truncate_chars;
use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn compact_session(
        &self,
        session_id: &str,
    ) -> BootResult<serde_json::Value> {
        let session = self.kernel_session(session_id).await?;
        self.model_history_for_session(session.as_ref()).await?;
        let workspace = session.workspace().to_path_buf();
        let timeline_store = crate::timeline::TimelineJsonlStore::for_session(
            code_web_store_dir(&workspace),
            session_id,
        );
        let history_messages = timeline_store
            .metadata()
            .map_err(|error| BootError::Internal(error.to_string()))?
            .source_message_count;
        if history_messages == 0 {
            return Err(BootError::BadRequest("nothing to compact yet".to_string()));
        }

        let llm_client = self.session_llm_client(session_id).await?;
        let summary = crate::compact::compact_timeline(llm_client, &timeline_store)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let summary =
            summary.ok_or_else(|| BootError::BadRequest("nothing to compact yet".to_string()))?;
        self.finalize_code_web_compact(session_id, session.as_ref(), &summary)
            .await?;

        Ok(json!({
            "sessionId": session_id,
            "compacted": true,
            "summary": summary,
            "historyMessages": history_messages,
            "completedAt": chrono::Utc::now().to_rfc3339(),
        }))
    }

    pub(in crate::api::code_web) async fn sleep_session(
        &self,
        session_id: &str,
        request: SleepSessionRequest,
    ) -> BootResult<serde_json::Value> {
        let session = self.kernel_session(session_id).await?;
        let focus = request
            .focus
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
            .to_string();
        let today = sleep_today();
        let directive = sleep_directive(&focus, false, &today);
        let history = self.model_history_for_session(session.as_ref()).await?;
        self.save_code_web_message_to_timeline(session_id, &Message::user(&directive))
            .await?;
        let result = run_code_web_stream(session.as_ref(), &directive, &history)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let assistant_text = result.text.clone();
        let usage = UsageResponse::from_usage(result.usage.clone());
        let tool_calls_count = result.tool_calls_count;
        let core_summary = result
            .compact_summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        let (report_captured, saved_memories) = match parse_sleep_report(&assistant_text) {
            Some(memories) => (
                true,
                store_sleep_memories(session.as_ref(), memories, &today).await?,
            ),
            None => (false, Vec::new()),
        };
        if let Some(summary) = core_summary.as_deref() {
            self.maybe_auto_compact(
                session_id,
                session.as_ref(),
                result.last_prompt_tokens,
                Some(summary),
            )
            .await;
        }
        self.save_code_web_message_to_timeline(session_id, &Message::assistant(&assistant_text))
            .await?;

        let display = if focus.is_empty() {
            "Sleep consolidation".to_string()
        } else {
            format!("Sleep consolidation focus: {focus}")
        };
        let summary = if !report_captured {
            "Sleep consolidation finished without a parseable memory report.".to_string()
        } else if saved_memories.is_empty() {
            "Sleep consolidation finished with no durable memories to save.".to_string()
        } else {
            format!(
                "Sleep consolidation saved {} durable memories.",
                saved_memories.len()
            )
        };
        self.append_visible_message(session_id, "user", &display, None)
            .await;
        self.append_visible_message(session_id, "system", &summary, None)
            .await;
        if core_summary.is_none() {
            self.maybe_auto_compact(
                session_id,
                session.as_ref(),
                result.last_prompt_tokens,
                None,
            )
            .await;
        }
        self.persist_session_state(session_id).await?;

        Ok(json!({
            "sessionId": session_id,
            "focus": focus,
            "date": today,
            "reportCaptured": report_captured,
            "savedCount": saved_memories.len(),
            "memories": saved_memories,
            "assistantText": assistant_text,
            "usage": usage,
            "toolCallsCount": tool_calls_count,
            "completedAt": chrono::Utc::now().to_rfc3339(),
        }))
    }

    pub(in crate::api::code_web) async fn fork_session(
        &self,
        session_id: &str,
        request: ForkSessionRequest,
    ) -> BootResult<serde_json::Value> {
        let source_session = self.kernel_session(session_id).await?;
        let focus = request
            .focus
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or("")
            .to_string();
        let source_history = source_session.history();
        let source_messages = self
            .state
            .messages
            .lock()
            .await
            .get(session_id)
            .cloned()
            .unwrap_or_default();
        if source_history.is_empty() && source_messages.is_empty() {
            return Err(BootError::BadRequest(
                "nothing to fork yet - start a conversation first".to_string(),
            ));
        }

        let workspace = source_session.workspace().to_path_buf();
        let source_settings = self.session_settings_snapshot(session_id).await;
        let model = self.effective_model(&source_settings);
        let target_settings = source_settings.clone();
        let source_controls = self.session_controls_snapshot(session_id).await;
        let (target_session, llm_client) = self
            .create_agent_session(&workspace, None, &target_settings, &source_controls)
            .await?;
        let target_session_id = target_session.session_id().to_string();
        let source_context = self.session_context_snapshot(session_id).await;
        let mut target_context = CodeWebSessionContext {
            compact_summary: build_fork_context(
                session_id,
                &focus,
                source_context.compact_summary.as_deref(),
                &source_messages,
                &source_history,
            ),
            ..CodeWebSessionContext::default()
        };
        target_context.set_llm_client(llm_client);
        let target_messages =
            fork_messages(session_id, &target_session_id, &source_messages, &focus);
        let title = fork_title(&focus);
        let source_metadata = self.session_metadata_snapshot(session_id).await;
        let now = chrono::Utc::now().timestamp_millis();
        let metadata = CodeWebSessionMetadata {
            workspace: workspace.display().to_string(),
            title: Some(title.clone()),
            agent_id: source_metadata
                .agent_id
                .or_else(|| Some("default".to_string())),
            created_at: now,
            updated_at: now,
        };

        self.state
            .sessions
            .lock()
            .await
            .insert(target_session_id.clone(), Arc::clone(&target_session));
        self.state
            .messages
            .lock()
            .await
            .insert(target_session_id.clone(), target_messages.clone());
        self.state
            .session_metadata
            .lock()
            .await
            .insert(target_session_id.clone(), metadata.clone());
        self.state
            .session_controls
            .lock()
            .await
            .insert(target_session_id.clone(), source_controls);
        self.state
            .session_contexts
            .lock()
            .await
            .insert(target_session_id.clone(), target_context);
        self.state
            .session_settings
            .lock()
            .await
            .insert(target_session_id.clone(), target_settings.clone());
        self.state.session_turn_queues.lock().await.insert(
            target_session_id.clone(),
            CodeWebSessionTurnQueue::default(),
        );
        target_session
            .save()
            .await
            .map_err(|error| BootError::Internal(format!("failed to save session: {error}")))?;
        self.persist_session_state(&target_session_id).await?;

        Ok(json!({
            "sourceSessionId": session_id,
            "sessionId": target_session_id,
            "focus": focus,
            "workspace": workspace.display().to_string(),
            "model": model,
            "title": title,
            "copiedMessages": source_messages.len(),
            "messages": target_messages,
            "session": SessionResponse::from_session(
                target_session.as_ref(),
                model.clone(),
                target_settings.follow_default_model,
                target_settings.permission_mode,
                &metadata,
            ),
            "createdAt": chrono::Utc::now().to_rfc3339(),
        }))
    }
}

pub(super) fn compact_visible_messages_after_success(
    session_id: &str,
    mut messages: Vec<Value>,
    _summary: &str,
) -> Vec<Value> {
    messages.push(json!({
        "id": format!("{}-compact", chrono::Utc::now().timestamp_millis()),
        "sessionId": session_id,
        "role": "system",
        "content": "Context compacted for the model.",
        "createdAt": chrono::Utc::now().to_rfc3339(),
    }));
    messages
}

fn fork_title(focus: &str) -> String {
    let focus = focus.trim();
    if focus.is_empty() {
        "Forked session".to_string()
    } else {
        format!("Fork: {}", truncate_chars(focus, 48))
    }
}

pub(super) fn fork_messages(
    source_session_id: &str,
    target_session_id: &str,
    source_messages: &[serde_json::Value],
    focus: &str,
) -> Vec<serde_json::Value> {
    let now = chrono::Utc::now();
    let mut messages: Vec<serde_json::Value> = source_messages
        .iter()
        .enumerate()
        .map(|(index, message)| {
            let mut cloned = message.clone();
            if let Value::Object(map) = &mut cloned {
                map.insert(
                    "id".to_string(),
                    Value::String(format!("{}-fork-copy-{index}", now.timestamp_millis())),
                );
                map.insert(
                    "sessionId".to_string(),
                    Value::String(target_session_id.to_string()),
                );
            }
            cloned
        })
        .collect();
    let focus_line = focus
        .trim()
        .is_empty()
        .then(String::new)
        .unwrap_or_else(|| format!("\nFocus: {}", focus.trim()));
    messages.push(json!({
        "id": format!("{}-fork", now.timestamp_millis()),
        "sessionId": target_session_id,
        "role": "system",
        "content": format!("Forked from session `{source_session_id}`.{focus_line}"),
        "createdAt": now.to_rfc3339(),
    }));
    messages
}

pub(super) fn build_fork_context(
    source_session_id: &str,
    focus: &str,
    previous_summary: Option<&str>,
    source_messages: &[serde_json::Value],
    source_history: &[Message],
) -> Option<String> {
    let mut parts = Vec::new();
    if let Some(summary) = previous_summary
        .map(str::trim)
        .filter(|value| !value.is_empty())
    {
        parts.push(format!("Existing compacted summary:\n{summary}"));
    }
    let visible_transcript = fork_transcript_from_messages(source_messages);
    if !visible_transcript.is_empty() {
        parts.push(format!(
            "Visible conversation transcript:\n{visible_transcript}"
        ));
    } else {
        let history_transcript = fork_transcript_from_history(source_history);
        if !history_transcript.is_empty() {
            parts.push(format!("Core conversation history:\n{history_transcript}"));
        }
    }
    if parts.is_empty() {
        return None;
    }
    let focus_line = focus
        .trim()
        .is_empty()
        .then(String::new)
        .unwrap_or_else(|| format!("\nFork focus: {}", focus.trim()));
    Some(truncate_chars(
        &format!(
            "This session was forked from `{source_session_id}`.{focus_line}\n\n{}",
            parts.join("\n\n")
        ),
        12_000,
    ))
}

pub(super) fn fork_transcript_from_messages(messages: &[serde_json::Value]) -> String {
    let mut lines = Vec::new();
    for message in messages.iter().rev().take(24).rev() {
        let role = message
            .get("role")
            .and_then(Value::as_str)
            .unwrap_or("message");
        let Some(content) = message.get("content").and_then(Value::as_str) else {
            continue;
        };
        let content = content.trim();
        if content.is_empty() {
            continue;
        }
        lines.push(format!("{role}: {}", truncate_chars(content, 1200)));
    }
    truncate_chars(&lines.join("\n\n"), 10_000)
}

pub(super) fn fork_transcript_from_history(history: &[Message]) -> String {
    let mut lines = Vec::new();
    for message in history.iter().rev().take(24).rev() {
        let content = message_text(message);
        if content.trim().is_empty() {
            continue;
        }
        lines.push(format!(
            "{}: {}",
            message.role,
            truncate_chars(content.trim(), 1200)
        ));
    }
    truncate_chars(&lines.join("\n\n"), 10_000)
}

pub(super) fn message_text(message: &Message) -> String {
    message
        .content
        .iter()
        .filter_map(|block| match block {
            ContentBlock::Text { text } => Some(text.as_str()),
            ContentBlock::ToolUse { name, .. } => Some(name.as_str()),
            ContentBlock::ToolResult { .. } | ContentBlock::Image { .. } => None,
        })
        .collect::<Vec<_>>()
        .join("\n")
}
