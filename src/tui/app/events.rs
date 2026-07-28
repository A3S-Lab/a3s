//! Projection of Core agent events into semantic TUI state.

use super::*;

impl App {
    fn on_llm_turn_start(&mut self, turn: usize) {
        if let Some(checkpoint) = self
            .llm_turn_checkpoint
            .as_ref()
            .filter(|checkpoint| checkpoint.turn == turn)
            .cloned()
        {
            self.rollback_llm_turn_attempt(checkpoint);
            return;
        }

        self.llm_turn_checkpoint = Some(LlmTurnUiCheckpoint {
            turn,
            transcript_len: self.messages.len(),
            streaming: self.streaming.clone(),
            thinking: self.thinking.clone(),
            turn_text: self.turn_text.clone(),
            got_delta: self.got_delta,
            turn_had_agent_activity: self.turn_had_agent_activity,
            turn_text_after_activity: self.turn_text_after_activity,
            runtime_tools: self.runtime.checkpoint_tools(),
        });
    }

    fn rollback_llm_turn_attempt(&mut self, checkpoint: LlmTurnUiCheckpoint) {
        self.messages.truncate(checkpoint.transcript_len);
        self.streaming = checkpoint.streaming;
        self.thinking = checkpoint.thinking;
        self.turn_text = checkpoint.turn_text;
        self.got_delta = checkpoint.got_delta;
        self.turn_had_agent_activity = checkpoint.turn_had_agent_activity;
        self.turn_text_after_activity = checkpoint.turn_text_after_activity;
        self.runtime.restore_tools(checkpoint.runtime_tools);
        self.last_paint = None;
        self.relayout();
        self.rebuild_viewport();
    }

    pub(super) fn on_agent_event(&mut self, event: AgentEvent) -> Option<Cmd<Msg>> {
        // After an interrupt, both receivers are cleared — ignore late events.
        if self.rx.is_none() && self.deep_research_events.is_none() {
            return None;
        }
        if self.interrupting {
            return None;
        }
        capture_host_dynamic_workflow_call_id(
            self.host_progress_inflight,
            &mut self.host_tool_call_id,
            &event,
        );
        match event {
            AgentEvent::TurnStart { turn } => {
                self.on_llm_turn_start(turn);
            }
            AgentEvent::TextDelta { text } => {
                self.mark_assistant_text(&text);
                self.got_delta = true;
                self.turn_text.push_str(&text);
                if self.deep_research_loop.is_some()
                    && self.deep_research_report_tool_gate.finalization_only()
                {
                    return self.rx.clone().map(pump);
                }
                if self.streaming.push(&text) {
                    self.streaming.commit_catch_up_tick(Instant::now());
                    self.update_viewport_with_stream();
                }
            }
            AgentEvent::ReasoningDelta { text } => {
                self.thinking.push_str(&text);
                self.update_viewport_with_stream();
            }
            AgentEvent::ToolStart { id, name } => {
                let transcript_visible = presentation_policy(&name).transcript_visible();
                self.mark_agent_activity();
                // A tool event is an authoritative transcript boundary. Seal
                // the assistant segment even when its last Markdown construct
                // is incomplete; later text belongs to a new assistant entry.
                self.finalize_streaming();
                self.messages
                    .start_tool(id.clone(), name.clone(), transcript_visible);
                self.runtime.prepare_tool(id, name);
                self.update_viewport_with_stream();
            }
            AgentEvent::ToolInputDelta { id, delta } => {
                self.messages.push_tool_input(id.as_deref(), &delta);
                self.runtime.push_tool_input(id.as_deref(), &delta);
                let update_plan_args = id.as_deref().and_then(|id| {
                    let tool = self.runtime.tool(id)?;
                    (presentation_policy(&tool.name) == ToolPresentationPolicy::PinnedOnly)
                        .then(|| tool.args())
                        .flatten()
                });
                if let Some(args) = update_plan_args {
                    self.apply_update_plan_args(&args);
                }
                self.update_viewport_with_stream();
            }
            AgentEvent::ToolExecutionStart { id, name, args } => {
                self.mark_agent_activity();
                let policy = presentation_policy(&name);
                if policy == ToolPresentationPolicy::PinnedOnly {
                    self.apply_update_plan_args(&args);
                }
                self.messages.start_tool_execution(
                    id.clone(),
                    name.clone(),
                    args.clone(),
                    policy.transcript_visible(),
                );
                self.runtime.start_execution(id, name, args);
                self.update_viewport_with_stream();
            }
            AgentEvent::ToolOutputDelta { id, name, delta } => {
                self.messages.push_tool_output(
                    &id,
                    name.clone(),
                    &delta,
                    presentation_policy(&name).transcript_visible(),
                );
                self.runtime.push_tool_output(&id, name, &delta);
                if let Some(output) = self.runtime.tool(&id).map(|tool| tool.output().to_string()) {
                    if let Some(spec) = self.find_remote_view_spec(&output) {
                        self.remember_remote_view(spec);
                    }
                }
                self.update_viewport_with_stream();
            }
            AgentEvent::ToolEnd {
                id,
                name,
                args,
                output,
                exit_code,
                metadata,
                ..
            } => {
                self.mark_agent_activity();
                if presentation_policy(&name) == ToolPresentationPolicy::PinnedOnly {
                    if let Some(args) = args.as_ref() {
                        self.apply_update_plan_args(args);
                    }
                }
                let completed = self.runtime.end_tool(
                    &id,
                    name.clone(),
                    args.clone(),
                    output.clone(),
                    exit_code,
                );
                if presentation_policy(&name) == ToolPresentationPolicy::PinnedOnly {
                    self.messages.discard_tool(&id);
                } else {
                    self.messages.finish_tool_with_state(
                        &id,
                        name.clone(),
                        completed.args.clone().or(args),
                        completed.output.clone(),
                        completed.exit_code,
                        metadata,
                        completed.state,
                        true,
                    );
                }
                self.rebuild_viewport();
                self.record_runtime_tool_evidence(&name);
                if completed.first_terminal {
                    self.capture_workflow(&name, completed.args.as_ref());
                }
                if let Some(spec) = self.find_remote_view_spec(&output) {
                    self.remember_remote_view(spec);
                }
            }
            // Parallel/child task lifecycle (parallel_task, task) — show each
            // sub-task starting, its progress, and how it finished.
            AgentEvent::SubagentStart {
                task_id,
                agent,
                description,
                started_ms,
                ..
            } => {
                self.mark_agent_activity();
                self.finalize_streaming();
                self.record_runtime_parallel_evidence();
                let journal_cmd = self
                    .deep_research_loop
                    .is_some()
                    .then(|| {
                        self.record_deep_research_child_event_cmd(
                            task_id.clone(),
                            true,
                            serde_json::json!({
                                "task_id": task_id,
                                "agent": agent,
                                "description": description,
                                "started_ms": started_ms,
                            }),
                        )
                    })
                    .flatten();
                // Track it in the live bottom panel instead of a transcript line.
                let first_start = self.runtime.start_subagent(
                    task_id.clone(),
                    agent.clone(),
                    description.clone(),
                    instant_from_epoch_ms(started_ms),
                );
                self.relayout();
                if first_start && self.runtime.subagent_needs_completion_watch(&task_id) {
                    let generation = self.session_rebuild_seq;
                    self.background_subagent_watches
                        .insert((generation, task_id.clone()));
                    let mut commands = vec![watch_background_subagent(
                        self.session.clone(),
                        self.session_id.clone(),
                        generation,
                        task_id,
                    )];
                    if let Some(journal_cmd) = journal_cmd {
                        commands.push(journal_cmd);
                    } else if let Some(rx) = self.rx.clone() {
                        commands.push(pump(rx));
                    }
                    return Some(cmd::batch(commands));
                }
                if journal_cmd.is_some() {
                    return journal_cmd;
                }
            }
            AgentEvent::SubagentProgress {
                task_id, metadata, ..
            } => {
                self.mark_agent_activity();
                self.runtime.record_subagent_progress(&task_id, &metadata);
                // Per-child OUTPUT tokens for the panel's `↓`. Each child turn-end
                // reports that turn's completion_tokens once, so SUM them across
                // turns (tool-event progress carries no usage, so it won't add).
                // The old code took max(total_tokens), i.e. the largest single
                // turn's prompt+completion ≈ the child's context size, not output.
                let toks = metadata
                    .get("completion_tokens")
                    .or_else(|| metadata.pointer("/usage/completion_tokens"))
                    .and_then(|v| v.as_u64());
                if let Some(t) = toks {
                    self.runtime.add_subagent_tokens(&task_id, t);
                }
                self.refresh_transcript_view();
            }
            AgentEvent::SubagentEnd {
                task_id,
                agent,
                output,
                success,
                finished_ms,
                ..
            } => {
                self.mark_agent_activity();
                let journal_cmd = self
                    .deep_research_loop
                    .is_some()
                    .then(|| {
                        self.record_deep_research_child_event_cmd(
                            task_id.clone(),
                            false,
                            serde_json::json!({
                                "task_id": task_id,
                                "agent": agent,
                                "success": success,
                                "finished_ms": finished_ms,
                            }),
                        )
                    })
                    .flatten();
                let completed = self.runtime.end_subagent(
                    task_id,
                    agent,
                    output,
                    success,
                    instant_from_epoch_ms(finished_ms),
                );
                self.push_subagent_completion(completed);
                if journal_cmd.is_some() {
                    return journal_cmd;
                }
            }
            AgentEvent::ContextCompacted {
                before_messages,
                after_messages,
                percent_before,
                ..
            } => {
                // The core auto-compacted mid-turn (pruned tool outputs + summarized
                // old messages). Core commits the compacted generation to the
                // durable session and re-arms itself for later cycles. Reset the
                // displayed fill until the next authoritative TurnEnd usage.
                self.output_tokens = 0;
                self.last_prompt_tokens = 0;
                self.ctx_warned_tier = 0;

                // Only surface real message-count reductions. Prune-only rounds
                // (equal count, smaller content) remain quiet to avoid noise.
                if after_messages < before_messages {
                    // Core calculates this against the active model's context
                    // window, so it matches the footer without rescaling.
                    let pct = (percent_before * 100.0).round().clamp(0.0, 100.0) as u32;
                    self.push_notice(
                        NoticeKind::Info,
                        format!(
                            "Context auto-compacted at {pct}% · {before_messages} → {after_messages} messages"
                        ),
                    );
                }
            }
            AgentEvent::ConfirmationRequired {
                tool_id,
                tool_name,
                args,
                ..
            } => {
                if let Some(approved) = self.execution_policy.auto_confirmation_decision(
                    &tool_name,
                    &args,
                    Path::new(&self.cwd),
                ) {
                    // Auto owns the decision before any approval projection is
                    // created. This keeps late or tool-owned confirmation
                    // events from flashing "Awaiting approval" in the
                    // transcript or runtime panel.
                    let session = self.session.clone();
                    let reason =
                        (!approved).then(|| "Denied by the Auto mode hard guardrail.".to_string());
                    return Some(cmd::cmd(move || async move {
                        let _ = session.confirm_tool_use(&tool_id, approved, reason).await;
                        Msg::Resume
                    }));
                }
                self.runtime
                    .await_approval(tool_id.clone(), tool_name.clone(), args.clone());
                if presentation_policy(&tool_name) == ToolPresentationPolicy::PinnedOnly {
                    self.messages.start_tool_execution(
                        tool_id.clone(),
                        tool_name.clone(),
                        args.clone(),
                        false,
                    );
                } else {
                    self.messages.await_tool_approval(
                        tool_id.clone(),
                        tool_name.clone(),
                        args.clone(),
                    );
                }
                self.update_viewport_with_stream();
                // Claude-style: no "requests:" transcript line — the prompt on
                // the activity line shows the tool; after approval the tool just
                // runs and its result lands via ToolEnd.
                let was_empty = self.pending_tools.is_empty();
                self.state = State::Awaiting;
                let label = tool_approval_label(&tool_name, Some(&args));
                self.pending_tools
                    .push_back(PendingToolApproval::new(tool_id, tool_name, args, label));
                if was_empty {
                    self.approval_sel = 0;
                }
                // Keep one pump parked on the event stream while awaiting input:
                // the confirmation can also resolve by timeout or an external
                // provider, and those events must clear the overlay.
                return self.rx.clone().map(pump);
            }
            AgentEvent::ConfirmationReceived {
                tool_id,
                approved,
                reason,
            } => {
                self.restore_approval_feedback_for(&tool_id);
                let pending = take_pending_tool_approval(&mut self.pending_tools, &tool_id);
                if !approved {
                    let reason = reason
                        .filter(|value| !value.trim().is_empty())
                        .unwrap_or_else(|| "Denied by user.".to_string());
                    if let Some((name, args)) = self
                        .runtime
                        .tool(&tool_id)
                        .map(|tool| (tool.name.clone(), tool.args()))
                    {
                        let completed = self.runtime.deny_tool(&tool_id, name, args, reason);
                        self.push_terminal_tool(completed);
                    }
                }
                if pending.as_ref().is_some_and(|(_, was_front)| *was_front) {
                    self.approval_sel = 0;
                }
                if pending.is_some() && self.pending_tools.is_empty() {
                    self.state = State::Streaming;
                    return Some(self.resume_after_pending_confirmation());
                }
            }
            AgentEvent::ConfirmationTimeout {
                tool_id,
                action_taken,
            } => {
                self.restore_approval_feedback_for(&tool_id);
                let pending = take_pending_tool_approval(&mut self.pending_tools, &tool_id);
                if let Some(completed) = self.runtime.timeout_tool(&tool_id, &action_taken) {
                    self.push_terminal_tool(completed);
                }
                if pending.as_ref().is_some_and(|(_, was_front)| *was_front) {
                    self.approval_sel = 0;
                }
                if pending.is_some() && self.pending_tools.is_empty() {
                    self.state = State::Streaming;
                    return Some(self.resume_after_pending_confirmation());
                }
            }
            AgentEvent::PermissionDenied {
                tool_id,
                tool_name,
                args,
                reason,
            } => {
                let completed = self.runtime.deny_tool(
                    &tool_id,
                    tool_name,
                    Some(args),
                    format!("Permission denied: {reason}"),
                );
                self.push_terminal_tool(completed);
            }
            // Live context fill: every LLM round-trip reports its prompt size,
            // so ctx% (and the fill warnings) track DURING long multi-tool
            // turns instead of freezing until End.
            AgentEvent::TurnEnd { usage, .. } => {
                self.llm_turn_checkpoint = None;
                if usage.prompt_tokens > 0 {
                    self.last_prompt_tokens = usage.prompt_tokens;
                    self.maybe_warn_ctx();
                }
            }
            AgentEvent::End {
                text, usage, meta, ..
            } => {
                self.record_local_agent_terminal(
                    crate::system_agents::AgentActivityState::Completed,
                );
                let review_text = if text.is_empty() {
                    self.turn_text.clone()
                } else {
                    text.clone()
                };
                // /loop: stop once the agent signals completion (the word DONE).
                // Not during /sleep: its completion signal is the a3s-sleep
                // report itself, and consolidation narration ("what was done
                // today") would false-trigger this and end the run early.
                if self.loop_remaining > 0 && !self.sleep_pending {
                    let r = review_text.clone();
                    if r.split(|c: char| !c.is_alphabetic())
                        .any(|w| w.eq_ignore_ascii_case("done"))
                    {
                        self.loop_remaining = 0;
                    }
                }
                // Asset review scans the WHOLE turn's text: with a delta-only
                // provider a tool call after the report would have cleared the
                // live buffer, losing a fully delivered report.
                // Only fall back to End.text when the provider never streamed
                // deltas this turn. Using the live buffer's emptiness here dups
                // text: a mid-turn finalize (e.g. a tool call) empties the buffer,
                // so End.text (the full message) would be appended a second time.
                if !self.got_delta && !text.is_empty() {
                    self.mark_assistant_text(&text);
                    self.streaming.push(&text);
                }
                self.finalize_streaming();
                // Asset code review: a ```a3s-review report in the final message
                // ends the review loop and opens the issue checklist.
                self.capture_review(&review_text);
                // `/sleep`: an ```a3s-sleep report ends the consolidation loop
                // and persists the distilled memories (async, batched below).
                let sleep_save = self.capture_sleep(&review_text);
                self.capture_research_report_view(&review_text);
                self.disarm_sleep_if_over(sleep_save.is_some());
                // `↓` counts OUTPUT (generated) tokens. Summing total_tokens per
                // turn re-counts the whole context every turn (the prompt is
                // re-sent each round) and balloons far past what was generated.
                // completion_tokens is the output; fall back to total-prompt if a
                // provider omits it.
                self.output_tokens += if usage.completion_tokens > 0 {
                    usage.completion_tokens
                } else {
                    usage.total_tokens.saturating_sub(usage.prompt_tokens)
                };
                // ctx% is NOT updated here: End.usage.prompt_tokens is the
                // per-turn SUM of every round's prompt (the context is re-sent
                // each round, same ballooning as above), not the current
                // context size — a multi-round turn would read rounds× too
                // high and fire false fill warnings. The TurnEnd arm already
                // recorded the real per-round size, final round included.
                if self.model.is_none() {
                    self.model = meta.and_then(|m| m.response_model.or(m.request_model));
                }
                // Count the turn, idle, then continue /loop or drain the queue.
                // A captured sleep report's save runs alongside.
                return match (sleep_save, self.complete_turn()) {
                    (Some(save), Some(next)) => Some(cmd::batch(vec![save, next])),
                    (save, next) => save.or(next),
                };
            }
            AgentEvent::Error { message } => {
                self.record_local_agent_terminal(crate::system_agents::AgentActivityState::Failed);
                self.finalize_streaming();
                self.preserve_interrupted_tools();
                self.push_notice(NoticeKind::Error, &message);
                self.loop_remaining = 0; // a failed turn stops the /loop
                self.review_pending = false; // and abandons an asset review
                self.sleep_pending = false; // and a `/sleep` consolidation
                if self.goal_run.is_some() {
                    self.pending_goal_failure = Some(message);
                } else {
                    self.restore_autonomy();
                }
                let completed_stream_join = self.stream_join.take();
                self.finish();
                if let Some(completed_stream_join) = completed_stream_join {
                    return Some(self.wait_for_completed_stream_join(completed_stream_join, None));
                }
                // Don't strand messages queued while this turn was running.
                return self.continue_after_stream_settled(None);
            }
            AgentEvent::GoalExtracted { goal } => {
                self.record_goal_extracted(&goal);
            }
            AgentEvent::GoalProgress { progress, .. } => {
                self.record_goal_progress(progress);
            }
            AgentEvent::GoalAchieved { goal, .. } => {
                self.record_goal_achieved(&goal);
            }
            // Planning mode: capture the plan and live task-status updates for
            // the pinned TODO panel above the input.
            AgentEvent::PlanningEnd { plan, .. } => {
                self.mark_agent_activity();
                self.set_plan(&plan.steps);
            }
            AgentEvent::TaskUpdated { tasks, .. } => {
                self.mark_agent_activity();
                self.set_plan(&tasks);
            }
            // Per-step lifecycle also drives the panel, in case TaskUpdated is
            // sparse: a step turns ▶ on start and ✔/✗/⊘ on completion.
            AgentEvent::StepStart { step_id, .. } => {
                self.mark_agent_activity();
                self.set_task_status(&step_id, a3s_code_core::planning::TaskStatus::InProgress);
            }
            AgentEvent::StepEnd {
                step_id, status, ..
            } => {
                self.mark_agent_activity();
                self.set_task_status(&step_id, status);
            }
            // TurnStart, ToolInputDelta, memory, confirmation echoes,
            // etc. — not surfaced in this MVP.
            _ => {}
        }
        // Keep draining the stream.
        self.rx.clone().map(pump)
    }
}
