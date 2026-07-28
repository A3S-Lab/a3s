use super::goal_runtime::goal_continuation_prompt;
use super::*;

impl KernelService {
    pub(in crate::api::code_web) async fn stream_session_message(
        &self,
        session_id: &str,
        request: serde_json::Value,
    ) -> BootResult<BootResponse> {
        let session = self.kernel_session(session_id).await?;
        let queue_id = request
            .get("queueId")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        let queued_turn = match queue_id {
            Some(queue_id) => Some(self.begin_queued_turn(session_id, queue_id).await?),
            None => None,
        };
        let direct_content = request
            .get("content")
            .or_else(|| request.get("message"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty());
        if queued_turn.is_none() && direct_content.is_none() {
            return Err(BootError::BadRequest(
                "content or queueId is required".to_string(),
            ));
        }
        let visible_content = queued_turn
            .as_ref()
            .map(queued_turn_visible_content)
            .unwrap_or_else(|| direct_content.unwrap_or_default().to_string());
        if let Some(research_turn) = queued_turn
            .as_ref()
            .filter(|turn| turn.mode == CodeWebQueuedTurnMode::DeepResearch)
            .cloned()
        {
            return self
                .stream_deep_research_turn(session, research_turn, visible_content)
                .await;
        }
        let prompt = match queued_turn.as_ref().map(|turn| turn.kind) {
            Some(CodeWebQueuedTurnKind::GoalContinuation) => {
                let controls = self.session_controls_snapshot(session_id).await;
                let goal = controls
                    .goal
                    .as_deref()
                    .unwrap_or("the active session goal");
                let attempt = controls
                    .goal_run
                    .as_ref()
                    .map(|run| run.attempts)
                    .unwrap_or_default();
                goal_continuation_prompt(goal, attempt)
            }
            _ => {
                self.compose_session_prompt(session_id, &visible_content)
                    .await
            }
        };
        let history = match self.model_history_for_session(session.as_ref()).await {
            Ok(history) => history,
            Err(error) => {
                if let Some(turn) = queued_turn.as_ref() {
                    let _ = self.restore_queued_turn(session_id, &turn.id).await;
                }
                return Err(error);
            }
        };
        let (mut agent_events, join) = match session.stream(&prompt, Some(&history)).await {
            Ok(stream) => stream,
            Err(error) => {
                if let Some(turn) = queued_turn.as_ref() {
                    let _ = self.restore_queued_turn(session_id, &turn.id).await;
                }
                return Err(BootError::Internal(error.to_string()));
            }
        };
        self.begin_goal_attempt(session_id).await;
        if let Err(error) = self
            .append_message(session_id, "user", &visible_content, None)
            .await
        {
            session.cancel().await;
            if let Some(turn) = queued_turn.as_ref() {
                let _ = self.finish_queued_turn(session_id, &turn.id, true).await;
            }
            return Err(error);
        }

        let service = Self::new(Arc::clone(&self.state));
        let session_id = session_id.to_string();
        let queued_turn_id = queued_turn.as_ref().map(|turn| turn.id.clone());
        let (sender, receiver) = tokio::sync::mpsc::channel::<BootResult<SseEvent>>(64);
        tokio::spawn(async move {
            let mut accumulator = CodeWebStreamAccumulator::default();
            let mut observed_events = Vec::new();
            let mut terminal_error = None;

            while let Some(event) = agent_events.recv().await {
                let finished = matches!(event, AgentEvent::End { .. } | AgentEvent::Error { .. });
                if let AgentEvent::Error { message } = &event {
                    terminal_error = Some(message.clone());
                }
                service.observe_goal_event(&session_id, &event).await;
                observed_events.push(event.clone());
                send_code_web_event(&sender, &event).await;
                accumulator.observe(event);
                if finished {
                    break;
                }
            }

            let join_error = join.await.err().map(|error| error.to_string());
            let result = if let Some(error) = join_error {
                Err(error)
            } else {
                accumulator.finish()
            };

            let succeeded = result.is_ok();
            match result {
                Ok(mut result) => {
                    result.events = observed_events;
                    if let Err(error) = service
                        .persist_stream_result(&session_id, session.as_ref(), &result)
                        .await
                    {
                        let event = AgentEvent::Error {
                            message: format!("failed to persist streamed response: {error}"),
                        };
                        send_code_web_event(&sender, &event).await;
                    }
                }
                Err(error) => {
                    let message = terminal_error.unwrap_or(error);
                    if !observed_events
                        .iter()
                        .any(|event| matches!(event, AgentEvent::Error { .. }))
                    {
                        let event = AgentEvent::Error {
                            message: message.clone(),
                        };
                        send_code_web_event(&sender, &event).await;
                        observed_events.push(event);
                    }
                    let failure_text = format!("Task failed: {message}");
                    let model = service.session_response_model(&session_id).await;
                    let _ = service
                        .append_message_with_events(
                            &session_id,
                            "assistant",
                            &failure_text,
                            model,
                            &observed_events,
                        )
                        .await;
                }
            }

            let goal_continuation = service.pending_goal_continuation(&session_id).await;
            if let Some(turn_id) = queued_turn_id.as_deref() {
                let pause_queue = !succeeded && goal_continuation.is_none();
                let _ = service
                    .finish_queued_turn(&session_id, turn_id, pause_queue)
                    .await;
            }
            if let Some((goal, attempt)) = goal_continuation {
                let _ = service
                    .enqueue_goal_continuation(&session_id, &goal, attempt)
                    .await;
            }
        });

        let stream = futures::stream::unfold(receiver, |mut receiver| async move {
            receiver.recv().await.map(|event| (event, receiver))
        });
        Ok(BootResponse::sse(stream))
    }

    async fn persist_stream_result(
        &self,
        session_id: &str,
        session: &AgentSession,
        result: &CodeWebStreamResult,
    ) -> BootResult<()> {
        let core_summary = result
            .compact_summary
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string);
        if let Some(summary) = core_summary.as_deref() {
            self.maybe_auto_compact(
                session_id,
                session,
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
            self.maybe_auto_compact(session_id, session, result.last_prompt_tokens, None)
                .await;
        }
        if let Err(error) =
            refresh_evolution_runtime_after_turn(self.state.as_ref(), session.workspace()).await
        {
            tracing::warn!(%error, session_id, "could not refresh learned Web session assets after streamed turn");
        }
        Ok(())
    }
}

fn queued_turn_visible_content(turn: &CodeWebQueuedTurn) -> String {
    if turn.kind == CodeWebQueuedTurnKind::GoalContinuation {
        return format!("[goal continuation]\n{}", turn.content);
    }
    let mut sections = Vec::new();
    if !turn.skill_names.is_empty() {
        sections.push(format!(
            "[Selected skills]\n{}\n[/Selected skills]",
            turn.skill_names
                .iter()
                .map(|name| format!("- Use your `{name}` skill."))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    if !turn.context_files.is_empty() {
        sections.push(format!(
            "[Workspace context files]\n{}\n[/Workspace context files]",
            turn.context_files
                .iter()
                .map(|path| format!("- {path}"))
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }
    sections.push(turn.content.clone());
    sections.join("\n\n")
}

pub(super) struct CodeWebStreamResult {
    pub(super) text: String,
    pub(super) usage: TokenUsage,
    pub(super) tool_calls_count: usize,
    pub(super) last_prompt_tokens: usize,
    pub(super) compact_summary: Option<String>,
    pub(super) events: Vec<AgentEvent>,
}

#[derive(Default)]
pub(super) struct CodeWebStreamAccumulator {
    streamed_text: String,
    end_text: Option<String>,
    usage: Option<TokenUsage>,
    tool_calls_count: usize,
    last_prompt_tokens: usize,
    compact_summary: Option<String>,
    error: Option<String>,
}

impl CodeWebStreamAccumulator {
    pub(super) fn observe(&mut self, event: AgentEvent) {
        match event {
            AgentEvent::TextDelta { text } => self.streamed_text.push_str(&text),
            AgentEvent::ToolStart { .. } => {
                self.tool_calls_count = self.tool_calls_count.saturating_add(1)
            }
            AgentEvent::TurnEnd { usage, .. } => {
                self.last_prompt_tokens = usage.prompt_tokens;
            }
            AgentEvent::ContextCompacted {
                summary: Some(summary),
                ..
            } if !summary.trim().is_empty() => {
                // A later compaction summary includes the earlier generation,
                // so retaining only the latest one is sufficient.
                self.compact_summary = Some(summary);
            }
            AgentEvent::End { text, usage, .. } => {
                self.end_text = Some(text);
                self.usage = Some(usage);
            }
            AgentEvent::Error { message } => self.error = Some(message),
            _ => {}
        }
    }

    pub(super) fn finish(self) -> Result<CodeWebStreamResult, String> {
        if let Some(error) = self.error {
            return Err(error);
        }
        let usage = self
            .usage
            .ok_or_else(|| "agent stream ended without a final response".to_string())?;
        let end_text = self.end_text.unwrap_or_default();
        Ok(CodeWebStreamResult {
            text: if end_text.trim().is_empty() {
                self.streamed_text
            } else {
                end_text
            },
            usage,
            tool_calls_count: self.tool_calls_count,
            last_prompt_tokens: self.last_prompt_tokens,
            compact_summary: self.compact_summary,
            events: Vec::new(),
        })
    }
}

pub(super) async fn run_code_web_stream(
    session: &AgentSession,
    prompt: &str,
    history: &[Message],
) -> Result<CodeWebStreamResult, String> {
    let (mut events, join) = session
        .stream(prompt, Some(history))
        .await
        .map_err(|error| error.to_string())?;
    let mut accumulator = CodeWebStreamAccumulator::default();
    let mut observed_events = Vec::new();
    while let Some(event) = events.recv().await {
        let finished = matches!(event, AgentEvent::End { .. } | AgentEvent::Error { .. });
        observed_events.push(event.clone());
        accumulator.observe(event);
        if finished {
            break;
        }
    }
    join.await.map_err(|error| error.to_string())?;
    let mut result = accumulator.finish()?;
    result.events = observed_events;
    Ok(result)
}

pub(super) async fn send_code_web_event(
    sender: &tokio::sync::mpsc::Sender<BootResult<SseEvent>>,
    event: &AgentEvent,
) {
    let _ = sender.send(SseEvent::json(event)).await;
}
