//! Agent stream lifecycle, queue draining, and autonomous-run settlement.

use super::*;
use crate::tui::app_rewind::capture_rewind_checkpoint_seed;

struct DeepResearchTerminalJournalRequest<'a> {
    workspace: &'a Path,
    run_id: &'a str,
    query: Option<&'a str>,
    host_managed_inquiry: bool,
    workflow_output: &'a str,
    workflow_metadata: Option<&'a serde_json::Value>,
    outcome: ResearchOutcome,
    artifacts: Option<&'a ResearchReportArtifacts>,
    artifact_authority: Option<DeepResearchTerminalArtifactAuthority>,
}

struct StreamStartRequest {
    prompt: String,
    display_task: String,
    clear_turn_artifacts: bool,
    synthesis: bool,
    runtime_expectation: Option<RuntimeExpectation>,
    submitted_images: Vec<PendingImage>,
    execution_mode: Mode,
    plan_draft: Option<PlanDraftRequest>,
    capture_rewind: bool,
}

fn take_matching_queue_claim<T>(
    active: &mut Option<PriorityItem<T>>,
    active_token: &mut Option<u64>,
    token: u64,
) -> Option<PriorityItem<T>> {
    if *active_token != Some(token) {
        return None;
    }
    *active_token = None;
    active.take()
}

async fn finalize_deep_research_terminal_journal(
    request: DeepResearchTerminalJournalRequest<'_>,
) -> Result<ResearchRunProjection, String> {
    let DeepResearchTerminalJournalRequest {
        workspace,
        run_id,
        query,
        host_managed_inquiry,
        workflow_output,
        workflow_metadata,
        outcome,
        artifacts,
        artifact_authority,
    } = request;
    let evidence_first = match query {
        Some(query) => {
            match resolve_deep_research_run_publication(workspace, query, run_id, workflow_output) {
                Ok(publication) => publication,
                Err(_)
                    if outcome == ResearchOutcome::Degraded
                        && artifacts.is_some()
                        && artifact_authority
                            == Some(DeepResearchTerminalArtifactAuthority::VerifiedRecovery) =>
                {
                    None
                }
                Err(error) => return Err(error),
            }
        }
        None => None,
    };
    if let Some(published) = evidence_first {
        if artifact_authority != Some(DeepResearchTerminalArtifactAuthority::ValidatedPublication) {
            return Err(
                "a validated evidence-first publication was mislabeled as a recovery artifact"
                    .to_string(),
            );
        }
        let published_outcome = match published.publication {
            DeepResearchEvidenceFirstPublication::Synthesized => ResearchOutcome::Completed,
            DeepResearchEvidenceFirstPublication::Qualified => ResearchOutcome::Qualified,
            DeepResearchEvidenceFirstPublication::SourceBacked => ResearchOutcome::Degraded,
            DeepResearchEvidenceFirstPublication::NoEvidence => ResearchOutcome::Degraded,
        };
        if published_outcome != outcome {
            return Err(format!(
                "evidence-first publication outcome {published_outcome:?} disagrees with terminal outcome {outcome:?}"
            ));
        }
        if artifacts != Some(&published.artifacts) {
            return Err(
                "evidence-first terminal artifacts differ from the validated publication"
                    .to_string(),
            );
        }
        return record_deep_research_validated_publication_terminal(
            workspace,
            run_id,
            outcome,
            &published.artifacts,
            &published.quality,
        )
        .await
        .map_err(|error| error.to_string());
    } else {
        if artifact_authority == Some(DeepResearchTerminalArtifactAuthority::VerifiedRecovery)
            && outcome != ResearchOutcome::Degraded
        {
            return Err(
                "a verified DeepResearch recovery artifact cannot settle a successful outcome"
                    .to_string(),
            );
        }
        let successful_report = matches!(
            outcome,
            ResearchOutcome::Completed | ResearchOutcome::Qualified
        );
        if successful_report {
            match deep_research_inquiry_publication_outcome(workflow_output, workflow_metadata) {
                Ok(Some(DeepResearchRunOutcome::Completed))
                    if outcome == ResearchOutcome::Completed => {}
                Ok(Some(DeepResearchRunOutcome::Qualified))
                    if outcome == ResearchOutcome::Qualified => {}
                Ok(Some(actual)) => {
                    return Err(format!(
                        "DeepResearch report outcome disagrees with terminal Inquiry: {actual:?}"
                    ));
                }
                Ok(None) if host_managed_inquiry => {
                    return Err(
                        "host-managed DeepResearch cannot journal success without an Inquiry projection"
                            .to_string(),
                    );
                }
                Ok(None) => {}
                Err(error) => return Err(error),
            }
        }
        match inquiry_projection_from_workflow(workflow_output, workflow_metadata) {
            Ok(Some((events, state))) => {
                record_deep_research_inquiry_state(workspace, run_id, &events, &state)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            Ok(None) if host_managed_inquiry && successful_report => {
                return Err(
                    "host-managed DeepResearch terminal journal omitted its Inquiry projection"
                        .to_string(),
                );
            }
            Ok(None) => {}
            Err(error) if successful_report => return Err(error),
            Err(_) => {}
        }
    }

    record_deep_research_run_terminal(workspace, run_id, outcome, artifacts)
        .await
        .map_err(|error| error.to_string())
}

impl App {
    /// Change the composer mode without mutating the semantics of an admitted
    /// turn. When no turn is active, keep Core's live policy in sync
    /// immediately so the footer and authorization boundary cannot diverge.
    pub(super) fn set_composer_mode(&mut self, mode: Mode) {
        self.mode = mode;
        if self.active_turn_mode.is_none() {
            self.execution_policy.set_mode(mode);
        }
    }

    pub(super) fn start_stream_inner(
        &mut self,
        prompt: String,
        display_task: String,
        clear_turn_artifacts: bool,
        include_attachments: bool,
        synthesis: bool,
    ) -> Option<Cmd<Msg>> {
        self.start_stream_inner_with_runtime(
            prompt,
            display_task,
            clear_turn_artifacts,
            include_attachments,
            synthesis,
            None,
        )
    }

    pub(super) fn start_stream_inner_with_runtime(
        &mut self,
        prompt: String,
        display_task: String,
        clear_turn_artifacts: bool,
        include_attachments: bool,
        synthesis: bool,
        runtime_expectation: Option<RuntimeExpectation>,
    ) -> Option<Cmd<Msg>> {
        let execution_mode = self.mode;
        let submitted_images = if include_attachments {
            std::mem::take(&mut self.pending_images)
        } else {
            Vec::new()
        };
        self.start_stream_inner_with_runtime_and_images(StreamStartRequest {
            prompt,
            display_task,
            clear_turn_artifacts,
            synthesis,
            runtime_expectation,
            submitted_images,
            execution_mode,
            plan_draft: None,
            capture_rewind: false,
        })
    }

    #[allow(clippy::too_many_arguments)]
    fn start_stream_inner_with_runtime_and_images(
        &mut self,
        request: StreamStartRequest,
    ) -> Option<Cmd<Msg>> {
        let StreamStartRequest {
            prompt,
            display_task,
            clear_turn_artifacts,
            synthesis,
            runtime_expectation,
            submitted_images,
            execution_mode,
            plan_draft,
            capture_rewind,
        } = request;
        let attachments = match submitted_images
            .iter()
            .map(PendingImage::attachment)
            .collect::<std::io::Result<Vec<_>>>()
        {
            Ok(attachments) => attachments,
            Err(error) => {
                restore_submitted_images(&mut self.pending_images, submitted_images);
                self.push_notice(
                    NoticeKind::Error,
                    format!("Image attachment could not be read: {error}"),
                );
                self.relayout();
                return None;
            }
        };
        self.active_turn_mode = Some(execution_mode);
        self.active_plan_draft = plan_draft;
        self.execution_policy.set_mode(execution_mode);
        if clear_turn_artifacts && !synthesis {
            self.auto_review.on_user_turn();
            self.last_activity = Instant::now();
        }
        self.streaming.clear();
        self.llm_turn_checkpoint = None;
        self.got_delta = false; // track if this turn streamed any text deltas
        self.turn_text.clear();
        self.turn_had_agent_activity = false;
        self.turn_text_after_activity = false;
        if let Some(expectation) = runtime_expectation {
            self.runtime_expectation = Some(expectation);
        }
        self.stream_join_settling = false;
        self.stream_settle_abort = None;
        self.ultracode_synthesis_inflight = synthesis;
        if !synthesis {
            self.ultracode_synthesis_used = false;
        }
        self.last_paint = None; // first delta of the turn paints immediately
        self.viewport.set_auto_scroll(true); // sending a message jumps to latest
        if clear_turn_artifacts {
            self.plan.clear(); // fresh plan per user turn; planning events refill it

            // Keep completed agents visible until the next user turn; a fresh
            // user turn starts a fresh runtime-entity projection.
            self.runtime.clear_turn_entities();
            self.runtime.set_subagent_task(display_task.clone());
        } else {
            self.runtime.clear_live_tools();
        }
        let rewind_task = display_task.clone();
        self.running_task = Some(display_task);
        self.state = State::Streaming;
        self.relayout();
        self.stream_started = Some(Instant::now());
        self.spinner.start();
        self.rebuild_viewport();
        let session = self.session.clone();
        let rewind_request = capture_rewind.then(|| {
            (
                self.store.clone(),
                self.session_id.clone(),
                PathBuf::from(&self.cwd),
                rewind_task,
                session.history(),
            )
        });
        // Keep the agent aligned with the standing goal (display stays clean).
        // Internal synthesis keeps its marker first so Core can reliably
        // suppress runtime auto-delegation for that final-answer-only turn.
        let prompt = match (&self.goal, synthesis) {
            (_, true) => prompt,
            (Some(g), false) => format!("[Ongoing goal: {g}]\n\n{prompt}"),
            (None, false) => prompt,
        };
        self.stream_start_token = self.stream_start_token.wrapping_add(1);
        let stream_start_token = self.stream_start_token;
        // (A `/ctx <n>` staged transcript window is attached upstream, only to a
        // genuine typed user message — see on_submit — never to a `/loop`,
        // asset review, `?`, or synthesis continuation.)
        // ultracode no longer rewrites the user turn. Whether a turn plans and
        // fans out is decided by the core's message-gated planning
        // (PlanningMode::Auto) plus the `parallel_task` tool description — not an
        // unconditional per-turn imperative, which made even "hi" trigger a plan
        // and workspace exploration.
        let commands = vec![
            cmd::cmd(move || async move {
                let rewind_checkpoint = match rewind_request {
                    Some((store, session_id, workspace, task, history_before)) => Some(
                        capture_rewind_checkpoint_seed(
                            store,
                            session_id,
                            workspace,
                            task,
                            history_before,
                        )
                        .await,
                    ),
                    None => None,
                };
                let res =
                    tokio::time::timeout(Duration::from_millis(STREAM_START_TIMEOUT_MS), async {
                        if attachments.is_empty() {
                            session.stream(prompt.as_str(), None).await
                        } else {
                            session
                                .stream_with_attachments(prompt.as_str(), &attachments, None)
                                .await
                        }
                    })
                    .await;
                match res {
                    Ok(Ok((rx, join))) => Msg::StreamStarted {
                        token: stream_start_token,
                        session: Arc::clone(&session),
                        rx: Arc::new(Mutex::new(rx)),
                        join,
                        submitted_images,
                        rewind_checkpoint,
                    },
                    Ok(Err(error)) => {
                        let retryable_admission = matches!(&error, CodeError::SessionBusy { .. });
                        Msg::StreamError {
                            token: stream_start_token,
                            error: error.to_string(),
                            retryable_admission,
                            submitted_images,
                        }
                    }
                    Err(_) => Msg::StreamError {
                        token: stream_start_token,
                        error: format!(
                            "model stream admission timed out after {STREAM_START_TIMEOUT_MS} ms"
                        ),
                        retryable_admission: true,
                        submitted_images,
                    },
                }
            }),
            spinner_tick(),
            stream_commit_tick(),
        ];
        Some(cmd::batch(commands))
    }

    /// Enqueue a turn together with the execution mode visible at submission.
    /// a3s-lane owns ordering; this sidecar owns immutable execution semantics.
    pub(super) fn enqueue_turn(
        &mut self,
        priority: a3s_lane::Priority,
        queued: Queued,
        mode: Mode,
    ) -> u64 {
        let sequence = self.queue.push(priority, queued);
        self.queued_turn_modes.insert(sequence, mode);
        sequence
    }

    pub(super) fn enqueue_plan_turn(
        &mut self,
        priority: a3s_lane::Priority,
        queued: Queued,
        request: PlanDraftRequest,
    ) -> u64 {
        let sequence = self.enqueue_turn(priority, queued, Mode::Plan);
        self.queued_plan_drafts.insert(sequence, request);
        sequence
    }

    /// Pop exactly one queued user turn.
    pub(super) fn drain_queue(&mut self) -> Option<Cmd<Msg>> {
        // A queued successor must not race a session rebuild or an interrupted
        // stream admission that still has a chance to acquire the core's
        // single-flight lease. The corresponding completion handler calls this
        // method again as soon as the barrier clears.
        if self.state != State::Idle
            || self.session_rebuild_pending.is_some()
            || self.interrupted_stream_start_token.is_some()
            || self.active_queued_turn.is_some()
            || self.pending_plan_review.is_some()
            || self.plan_review.is_some()
        {
            return None;
        }
        let next = panels::queue::take_next_priority_item(
            &mut self.queue,
            &mut self.send_now_queued_sequence,
        );
        if let Some(next) = next {
            let sequence = next.sequence();
            let execution_mode = self
                .queued_turn_modes
                .get(&sequence)
                .copied()
                .unwrap_or(self.mode);
            let plan_draft = self.queued_plan_drafts.get(&sequence).cloned();
            let queued = next.value().clone();
            if let Some((query, evidence_scope)) = queued.deep_research {
                let command = self.start_deep_research_workflow(
                    query,
                    evidence_scope,
                    queued.runtime_expectation,
                );
                if command.is_none() {
                    self.queue.restore(next);
                    self.relayout();
                } else {
                    if self.send_now_queued_sequence == Some(sequence) {
                        self.send_now_queued_sequence = None;
                    }
                    self.queued_turn_modes.remove(&sequence);
                    self.queued_plan_drafts.remove(&sequence);
                }
                return command;
            }
            let command = self.start_stream_inner_with_runtime_and_images(StreamStartRequest {
                prompt: queued.text,
                display_task: queued.display,
                clear_turn_artifacts: true,
                synthesis: false,
                runtime_expectation: queued.runtime_expectation,
                submitted_images: queued.images,
                execution_mode,
                plan_draft,
                capture_rewind: true,
            });
            if command.is_some() {
                self.active_queued_turn_token = Some(self.stream_start_token);
                self.active_queued_turn = Some(next);
            } else {
                self.queue.restore(next);
                self.relayout();
            }
            return command;
        }

        None
    }

    /// Commit the queue claim once Core has admitted its stream. Before this
    /// point the exact a3s-lane entry remains restorable on `SessionBusy` or an
    /// admission timeout.
    pub(super) fn commit_active_queued_turn(&mut self, token: u64) -> bool {
        let Some(item) = take_matching_queue_claim(
            &mut self.active_queued_turn,
            &mut self.active_queued_turn_token,
            token,
        ) else {
            return false;
        };
        self.queued_turn_modes.remove(&item.sequence());
        self.queued_plan_drafts.remove(&item.sequence());
        if self.send_now_queued_sequence == Some(item.sequence()) {
            self.send_now_queued_sequence = None;
        }
        self.queue_retry_generation = self.queue_retry_generation.wrapping_add(1);
        self.queue_retry_attempt = 0;
        true
    }

    /// Return a turn whose stream was never admitted to its original priority
    /// and FIFO position. Its images are Arc-backed, so dropping the attempted
    /// admission copy cannot invalidate the retained queue payload.
    pub(super) fn restore_active_queued_turn(&mut self, token: u64) -> bool {
        if let Some(item) = take_matching_queue_claim(
            &mut self.active_queued_turn,
            &mut self.active_queued_turn_token,
            token,
        ) {
            self.queue.restore(item);
            self.relayout();
            return true;
        }
        false
    }

    pub(super) fn discard_active_queued_turn(&mut self, token: u64) {
        if let Some(item) = take_matching_queue_claim(
            &mut self.active_queued_turn,
            &mut self.active_queued_turn_token,
            token,
        ) {
            self.queued_turn_modes.remove(&item.sequence());
            self.queued_plan_drafts.remove(&item.sequence());
            if self.send_now_queued_sequence == Some(item.sequence()) {
                self.send_now_queued_sequence = None;
            }
        }
    }

    pub(super) fn retry_queued_turn_after_admission_failure(&mut self) -> Cmd<Msg> {
        self.queue_retry_generation = self.queue_retry_generation.wrapping_add(1);
        self.queue_retry_attempt = self.queue_retry_attempt.saturating_add(1);
        let generation = self.queue_retry_generation;
        let shift = u32::from(self.queue_retry_attempt.saturating_sub(1).min(4));
        let delay_ms = QUEUE_ADMISSION_RETRY_BASE_MS
            .saturating_mul(1_u64 << shift)
            .min(QUEUE_ADMISSION_RETRY_MAX_MS);
        cmd::cmd(move || async move {
            tokio::time::sleep(Duration::from_millis(delay_ms)).await;
            Msg::QueueRetry { generation }
        })
    }

    pub(super) fn interrupted_continuation(
        goal_cancelled: bool,
        deep_research_interrupted: bool,
    ) -> InterruptedContinuation {
        if deep_research_interrupted {
            InterruptedContinuation::SettleDeepResearch
        } else if goal_cancelled {
            InterruptedContinuation::RestoreGoalMode
        } else {
            InterruptedContinuation::DrainQueue
        }
    }

    pub(super) fn defer_or_continue_after_interrupt(
        &mut self,
        continuation: InterruptedContinuation,
    ) -> Option<Cmd<Msg>> {
        if self.interrupted_stream_start_token.is_some() {
            self.pending_interrupted_continuation = Some(continuation);
            // Keep submissions queue-only until the stale admission has been
            // cancelled. This closes the race where its cleanup could cancel a
            // newly started queue head on the same session.
            self.state = State::Streaming;
            self.relayout();
            return None;
        }
        self.continue_after_interrupt(continuation)
    }

    pub(super) fn on_interrupted_stream_start_settled(&mut self, token: u64) -> Option<Cmd<Msg>> {
        if self.interrupted_stream_start_token != Some(token) {
            return None;
        }
        self.interrupted_stream_start_token = None;
        let Some(continuation) = self.pending_interrupted_continuation.take() else {
            // The cancellation command has not produced `Msg::Interrupted`
            // yet. That handler will observe the cleared barrier and continue.
            return None;
        };
        self.state = State::Idle;
        self.relayout();
        self.continue_after_interrupt(continuation)
    }

    fn continue_after_interrupt(
        &mut self,
        continuation: InterruptedContinuation,
    ) -> Option<Cmd<Msg>> {
        match continuation {
            InterruptedContinuation::SettleDeepResearch => {
                self.settle_or_finalize_deep_research(DeepResearchSettlementExit::Interrupted)
            }
            InterruptedContinuation::RestoreGoalMode => {
                self.restore_autonomy();
                self.restore_goal_planning_mode()
            }
            InterruptedContinuation::DrainQueue => {
                self.restore_autonomy();
                self.drain_queue()
            }
        }
    }

    pub(super) fn has_queued_turn(&self) -> bool {
        !self.queue.is_empty()
    }

    pub(super) fn wait_for_completed_stream_join(
        &mut self,
        stream_join: StreamJoin,
        synthesis: Option<(String, String)>,
    ) -> Cmd<Msg> {
        self.stream_settle_abort = Some(stream_join.abort_handle());
        self.stream_join_settling = true;
        self.state = State::Streaming;
        self.relayout();
        wait_for_stream_join(
            Arc::clone(&self.session),
            stream_join,
            self.stream_start_token,
            synthesis,
        )
    }

    /// Shared turn-completion: count the turn, wait for the stream lifecycle to
    /// settle, consume queued user input, then continue autonomous work.
    /// Called from BOTH the normal `AgentEvent::End` arm (the happy path, which
    /// returns without re-pumping so `StreamEnded` never fires) and the
    /// `StreamEnded` channel-closed arm — previously this lived only in
    /// `StreamEnded`, so on success the queue never drained and `/loop` ran once.
    pub(super) fn complete_turn(&mut self) -> Option<Cmd<Msg>> {
        if self.deep_research_loop.is_some() {
            self.deep_research_stream_timeout_token =
                self.deep_research_stream_timeout_token.wrapping_add(1);
        }
        if self.deep_research_loop.as_ref().is_some_and(|state| {
            state.started_at.elapsed() >= Duration::from_millis(DEEP_RESEARCH_RUN_HARD_TIMEOUT_MS)
        }) {
            self.loop_remaining = 0;
        }
        let degraded_deep_research = self.deep_research_loop.is_some()
            && matches!(self.deep_research_outcome, DeepResearchRunOutcome::Degraded);
        if self.state == State::Streaming && !degraded_deep_research {
            self.completed += 1;
        }
        self.warn_missing_runtime_evidence();
        let plan_review = self.take_plan_review_candidate();
        // Goal iterations are evaluated by Core before End. A generic hidden
        // synthesis turn must not replace that event gate or consume the turn
        // as if it were complete.
        let synthesis = if plan_review.is_some()
            || self.deep_research_loop.is_some()
            || degraded_deep_research
            || self.goal_run.is_some()
        {
            None
        } else {
            self.prepare_ultracode_synthesis()
        };
        let completed_stream_join = self.stream_join.take();
        self.finish();
        self.pending_plan_review = plan_review;
        if let Some(completed_stream_join) = completed_stream_join {
            // Keep input queue-only until the worker has completed persistence,
            // cleanup, and release of core's single-flight admission lease.
            return Some(self.wait_for_completed_stream_join(completed_stream_join, synthesis));
        }
        self.discard_active_rewind_checkpoint();
        self.continue_after_stream_settled(synthesis)
    }

    pub(super) fn continue_after_stream_settled(
        &mut self,
        synthesis: Option<(String, String)>,
    ) -> Option<Cmd<Msg>> {
        if self.activate_pending_plan_review() {
            return None;
        }
        if self.goal_run.as_ref().is_some_and(|run| run.achieved) {
            self.pending_goal_failure = None;
            return self.finish_achieved_goal();
        }
        if self.goal_run.is_some() {
            // User messages queued while the iteration ran still execute under
            // the forced-planning goal session. Otherwise schedule the next
            // host-owned iteration without a fixed cap.
            if !self.queue.is_empty() {
                return self.drain_queue();
            }
            let failure = self.pending_goal_failure.take();
            return self.continue_goal_run(failure);
        }
        self.pending_goal_failure = None;
        // A DeepResearch run completes its closed report transaction before
        // queued turns can change session context. Esc remains the explicit
        // way to interrupt the active run.
        if self.deep_research_loop.is_none() && self.has_queued_turn() {
            return self.drain_queue();
        }
        if let Some((prompt, display_task)) = synthesis {
            return self.start_ultracode_synthesis(prompt, display_task);
        }
        self.continue_completed_turn()
    }

    /// Select the next turn only after the prior stream lifecycle has fully
    /// settled. Keeping this selection deferred is important: queued
    /// DeepResearch starts `tool_with_events` synchronously, while normal model
    /// streams start when their returned command is polled.
    pub(super) fn continue_completed_turn(&mut self) -> Option<Cmd<Msg>> {
        // Do not interleave queued turns before DeepResearch terminal
        // journaling has settled.
        if self.deep_research_loop.is_none() && self.has_queued_turn() {
            return self.drain_queue();
        }
        // Required runtime evidence is a deliverable, not just a warning. In
        // autonomous runs, spend the next loop turn on a targeted correction
        // before falling back to the generic "Continue" prompt.
        if self.loop_remaining > 0 {
            if let Some(prompt) = self
                .runtime_expectation
                .as_ref()
                .and_then(RuntimeExpectation::corrective_prompt)
            {
                self.loop_remaining -= 1;
                let n = self.loop_remaining;
                self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                    "  ↻ runtime evidence retry ({n} left · Esc to stop)"
                )));
                self.loop_continuation = true;
                return Some(cmd::msg(Msg::Submit(prompt)));
            }
        }
        // /loop: auto-continue until the agent says DONE, the cap is hit, or Esc.
        // DeepResearch never receives a hidden continuation.
        if self.loop_remaining > 0 {
            if self.deep_research_loop.is_some() {
                self.loop_remaining = 0;
            } else {
                self.loop_remaining -= 1;
                let n = self.loop_remaining;
                let prompt =
                    "Continue. If the task is fully complete, reply DONE and stop.".to_string();
                self.push_line(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render(&format!("  ↻ loop ({n} left · Esc to stop)")),
                );
                // Mark the continuation as machine-driven so on_submit doesn't
                // attach a staged `/ctx` window to it.
                self.loop_continuation = true;
                return Some(cmd::msg(Msg::Submit(prompt)));
            }
        }
        // The loop is drained (or was never armed): an autonomous run that
        // auto-switched to auto mode is over — restore the user's mode.
        if self.loop_remaining == 0 {
            if self.deep_research_loop.is_some() {
                self.invalidate_subagent_snapshots();
                return self
                    .settle_or_finalize_deep_research(DeepResearchSettlementExit::ReportReady);
            }
            self.open_pending_deep_research_report_view();
            self.restore_autonomy();
        }
        // Run the next queued message (submitted while busy), if any.
        self.drain_queue()
    }

    pub(super) fn settle_or_finalize_deep_research(
        &mut self,
        exit: DeepResearchSettlementExit,
    ) -> Option<Cmd<Msg>> {
        match self.begin_deep_research_subagent_settlement(exit) {
            Some(settlement) => Some(settlement),
            None => self.finalize_deep_research_settlement(exit),
        }
    }

    pub(super) fn begin_deep_research_subagent_settlement(
        &mut self,
        exit: DeepResearchSettlementExit,
    ) -> Option<Cmd<Msg>> {
        if self.deep_research_loop.is_none() || self.deep_research_subagent_settlement_inflight {
            return None;
        }
        let mut task_ids = self.runtime.subagent_ids();
        if task_ids.is_empty() {
            return None;
        }
        task_ids.sort();
        self.deep_research_subagent_settlement_inflight = true;
        self.state = State::Streaming;
        self.spinner.start();
        self.relayout();
        Some(settle_deep_research_subagents(
            Arc::clone(&self.session),
            self.session_id.clone(),
            self.session_rebuild_seq,
            task_ids,
            exit,
        ))
    }

    pub(super) fn finalize_deep_research_settlement(
        &mut self,
        exit: DeepResearchSettlementExit,
    ) -> Option<Cmd<Msg>> {
        if self.deep_research_journal_finalization_inflight {
            return None;
        }
        if self.deep_research_workflow.typed_runner {
            return self.complete_deep_research_settlement(exit);
        }
        let run_id = self
            .deep_research_workflow
            .args
            .as_ref()
            .and_then(|args| args.get("run_id"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let host_managed_inquiry = self
            .deep_research_workflow
            .args
            .as_ref()
            .is_some_and(deep_research_host_managed_inquiry);
        let query = self
            .deep_research_workflow
            .args
            .as_ref()
            .and_then(|args| args.pointer("/input/query"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        let outcome = match self.deep_research_outcome {
            DeepResearchRunOutcome::Active if exit == DeepResearchSettlementExit::Interrupted => {
                Some(ResearchOutcome::Failed)
            }
            DeepResearchRunOutcome::Active => None,
            DeepResearchRunOutcome::Completed => Some(ResearchOutcome::Completed),
            DeepResearchRunOutcome::Qualified => Some(ResearchOutcome::Qualified),
            DeepResearchRunOutcome::SourceBacked | DeepResearchRunOutcome::NoEvidence => {
                Some(ResearchOutcome::Degraded)
            }
            DeepResearchRunOutcome::Degraded => Some(ResearchOutcome::Degraded),
        };
        if let (Some(run_id), Some(outcome)) = (run_id, outcome) {
            self.deep_research_journal_finalization_inflight = true;
            let workspace = PathBuf::from(&self.cwd);
            let terminal_artifacts = self.deep_research_terminal_artifacts.clone();
            let artifacts = terminal_artifacts
                .as_ref()
                .map(|(artifacts, _)| artifacts.clone());
            let artifact_authority = terminal_artifacts.as_ref().map(|(_, authority)| *authority);
            let workflow_output = self
                .deep_research_workflow
                .output
                .clone()
                .unwrap_or_default();
            let workflow_metadata = self.deep_research_workflow.metadata.clone();
            return Some(cmd::cmd(move || async move {
                let result =
                    finalize_deep_research_terminal_journal(DeepResearchTerminalJournalRequest {
                        workspace: &workspace,
                        run_id: &run_id,
                        query: query.as_deref(),
                        host_managed_inquiry,
                        workflow_output: &workflow_output,
                        workflow_metadata: workflow_metadata.as_ref(),
                        outcome,
                        artifacts: artifacts.as_ref(),
                        artifact_authority,
                    })
                    .await;
                Msg::DeepResearchJournalFinalized {
                    run_id,
                    exit,
                    result,
                }
            }));
        }
        self.complete_deep_research_settlement(exit)
    }

    pub(super) fn complete_deep_research_settlement(
        &mut self,
        exit: DeepResearchSettlementExit,
    ) -> Option<Cmd<Msg>> {
        if exit.opens_report() && self.deep_research_outcome.report_ready() {
            self.open_pending_deep_research_report_view();
        } else {
            self.pending_deep_research_report_view = None;
        }
        self.restore_autonomy();
        self.drain_queue()
    }

    pub(super) fn resume_after_pending_confirmation(&self) -> Cmd<Msg> {
        resume_after_pending_confirmation_cmd(self.rx.clone())
    }

    /// An autonomous directive run is starting (/sleep, asset reviews,
    /// asset run/deploy, /flow drafts, /loop): switch to non-interactive Auto
    /// so tool prompts cannot stall it, and arm the loop budget that re-prompts until
    /// the deliverable lands. The prior mode is restored when the run ends
    /// (loop drained, interrupt, error, or /clear). A user already in auto
    /// mode keeps it — nothing is remembered or restored.
    pub(super) fn engage_autonomy(&mut self, budget: usize) {
        self.loop_remaining = self.loop_remaining.max(budget);
        if self.mode != Mode::Auto {
            self.autonomy_restore = Some(self.mode);
            self.set_composer_mode(Mode::Auto);
            self.push_line(&Style::new().fg(TN_GRAY).render(
                "  ⏵⏵ auto mode engaged for this task — restores when it completes (Esc stops)",
            ));
        }
    }

    pub(super) fn engage_single_turn_autonomy(&mut self) {
        if self.mode != Mode::Auto {
            self.autonomy_restore = Some(self.mode);
            self.set_composer_mode(Mode::Auto);
            self.push_line(&Style::new().fg(TN_GRAY).render(
                "  ⏵⏵ auto mode engaged for this task — restores when it completes (Esc stops)",
            ));
        }
    }

    pub(super) fn record_runtime_tool_evidence(&mut self, name: &str) {
        if let Some(expectation) = &mut self.runtime_expectation {
            expectation.record_tool(name);
        }
    }

    pub(super) fn record_runtime_parallel_evidence(&mut self) {
        if let Some(expectation) = &mut self.runtime_expectation {
            expectation.record_parallel_work();
        }
    }

    pub(super) fn record_runtime_view_evidence(&mut self) {
        if let Some(expectation) = &mut self.runtime_expectation {
            expectation.record_remote_view();
        }
    }

    pub(super) fn warn_missing_runtime_evidence(&mut self) {
        let warning = self
            .runtime_expectation
            .as_mut()
            .and_then(RuntimeExpectation::missing_warning);
        if let Some(warning) = warning {
            self.push_line(&Style::new().fg(TN_YELLOW).render(&warning));
        }
    }

    /// Restore the pre-autonomy mode (no-op when nothing was auto-switched).
    pub(super) fn restore_autonomy(&mut self) {
        self.runtime_expectation = None;
        self.deep_research_loop = None;
        if let Some((goal, goal_since)) = self.deep_research_goal_restore.take() {
            self.goal = goal;
            self.goal_since = goal_since;
        }
        self.deep_research_workflow.clear();
        debug_assert!(self.deep_research_handle.is_none());
        self.deep_research_events = None;
        self.deep_research_outcome = DeepResearchRunOutcome::Active;
        self.deep_research_journal_finalization_inflight = false;
        self.deep_research_terminal_artifacts = None;
        self.deep_research_agent_event_sequence = 0;
        self.deep_research_projection = None;
        self.pending_deep_research_report_view = None;
        self.deep_research_report_tool_gate.reset();
        self.deep_research_subagent_settlement_inflight = false;
        if let Some(prev) = self.autonomy_restore.take() {
            self.set_composer_mode(prev);
            self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  ⏵ autonomous task ended — auto mode restored to your previous mode"),
            );
        }
    }

    pub(super) fn record_deep_research_child_event_cmd(
        &mut self,
        task_id: String,
        started: bool,
        payload: serde_json::Value,
    ) -> Option<Cmd<Msg>> {
        let run_id = self
            .deep_research_workflow
            .args
            .as_ref()?
            .get("run_id")?
            .as_str()?
            .to_string();
        self.deep_research_agent_event_sequence =
            self.deep_research_agent_event_sequence.saturating_add(1);
        let sequence = self.deep_research_agent_event_sequence;
        let workspace = PathBuf::from(&self.cwd);
        Some(cmd::cmd(move || async move {
            let result = record_deep_research_child_event(
                &workspace, &run_id, sequence, &task_id, started, payload,
            )
            .await
            .map_err(|error| error.to_string());
            Msg::DeepResearchJournalEventRecorded { run_id, result }
        }))
    }
}

#[cfg(test)]
mod queue_claim_tests {
    use super::*;

    #[test]
    fn admission_failure_restores_the_original_lane_position() {
        let mut queue = PriorityQueue::new();
        queue.push(USER_TURN_PRIORITY, "first");
        queue.push(USER_TURN_PRIORITY, "second");
        queue.push(SYNTHETIC_TURN_PRIORITY, "continuation");

        let mut active = queue.pop();
        let mut active_token = Some(9);
        let claimed = take_matching_queue_claim(&mut active, &mut active_token, 9)
            .expect("matching admission claim");
        queue.restore(claimed);

        let ordered = queue
            .ordered()
            .into_iter()
            .map(|item| *item.value())
            .collect::<Vec<_>>();
        assert_eq!(ordered, ["first", "second", "continuation"]);
    }

    #[test]
    fn stale_stream_token_cannot_consume_the_current_lane_claim() {
        let mut queue = PriorityQueue::new();
        queue.push(USER_TURN_PRIORITY, "current");
        let mut active = queue.pop();
        let original_sequence = active.as_ref().expect("active claim").sequence();
        let mut active_token = Some(12);

        assert!(take_matching_queue_claim(&mut active, &mut active_token, 11).is_none());
        assert_eq!(active_token, Some(12));
        assert_eq!(
            active.as_ref().expect("claim must remain").sequence(),
            original_sequence
        );
    }

    #[test]
    fn matching_lane_claim_is_consumed_at_most_once() {
        let mut queue = PriorityQueue::new();
        queue.push(USER_TURN_PRIORITY, "once");
        let mut active = queue.pop();
        let mut active_token = Some(21);

        assert!(take_matching_queue_claim(&mut active, &mut active_token, 21).is_some());
        assert!(take_matching_queue_claim(&mut active, &mut active_token, 21).is_none());
        assert!(active.is_none());
        assert_eq!(active_token, None);
    }
}

#[cfg(test)]
mod evidence_first_journal_tests {
    use super::*;

    #[tokio::test]
    async fn source_backed_publication_settles_as_degraded_without_an_inquiry_projection() {
        let workspace = tempfile::tempdir().expect("create evidence-first journal workspace");
        let query = "Which Nimbus release is supported?";
        let run_id = "evidence-first-tui-settlement";
        let acquisition = serde_json::json!({
            "query": query,
            "mode": "bootstrap_acquisition",
            "acquisition": {
                "status": "success",
                "packet": {
                    "version": 1,
                    "focuses": [],
                    "sources": [{
                        "source_id": "bootstrap-web-source-1",
                        "title": "Official Nimbus support record",
                        "url_or_path": "https://example.test/nimbus",
                        "chunks": [{
                            "chunk_id": "bootstrap-web-source-1:chunk:1",
                            "text": "The official record states that Nimbus version 2 receives fixes through September 2027."
                        }]
                    }]
                }
            }
        });
        let artifacts =
            super::super::deep_research_artifacts::materialize_deep_research_source_backed_report(
                workspace.path(),
                query,
                &acquisition.to_string(),
                None,
            )
            .expect("materialize source-backed settlement report")
            .expect("source-backed settlement artifacts");
        let slug = super::super::deep_research_artifacts::deep_research_report_slug(query);
        let workflow_output = serde_json::json!({
            "query": query,
            "mode": "evidence_first_report",
            "publication": {
                "status": "source_backed",
                "markdown": format!(".a3s/research/{slug}/report.md"),
                "html": format!(".a3s/research/{slug}/index.html"),
                "quality": {
                    "direct_answer_count": 0,
                    "finding_count": 0,
                    "accepted_claim_count": 0,
                    "cited_source_count": 0,
                    "substantive_character_count": 0,
                    "relevant_source_count": 1,
                    "source_count": 1
                }
            }
        })
        .to_string();
        let args = serde_json::json!({
            "run_id": run_id,
            "input": {
                "query": query,
                "current_date": "2026-07-21",
                "evidence_scope": "web_and_workspace",
                "inquiry_host_managed": true
            }
        });
        record_deep_research_workflow_started(
            workspace.path(),
            run_id,
            deep_research_evidence_first_research_spec(&args),
        )
        .await
        .expect("start evidence-first settlement journal");
        record_deep_research_workflow_completed(workspace.path(), run_id, true)
            .await
            .expect("close evidence-first workflow track");

        let projection =
            finalize_deep_research_terminal_journal(DeepResearchTerminalJournalRequest {
                workspace: workspace.path(),
                run_id,
                query: Some(query),
                host_managed_inquiry: true,
                workflow_output: &workflow_output,
                workflow_metadata: None,
                outcome: ResearchOutcome::Degraded,
                artifacts: Some(&artifacts),
                artifact_authority: Some(
                    DeepResearchTerminalArtifactAuthority::ValidatedPublication,
                ),
            })
            .await
            .expect("settle evidence-first publication without Inquiry state");

        assert_eq!(projection.outcome, ResearchOutcome::Degraded);
        assert!(projection.active_steps.is_empty());
        assert!(projection.active_children.is_empty());
        assert!(projection.artifact_evidence_head.is_some());
    }

    #[tokio::test]
    async fn invalid_publication_settles_only_with_verified_recovery_artifacts() {
        let workspace = tempfile::tempdir().expect("create recovery journal workspace");
        let query = "Assess the current Nimbus support policy";
        let run_id = "invalid-publication-recovery-settlement";
        let workflow_output = serde_json::json!({
            "query": query,
            "mode": "evidence_first_report",
            "publication": {}
        })
        .to_string();
        let artifacts = materialize_deep_research_recovery_report(
            workspace.path(),
            query,
            "the Host publication envelope was invalid",
            &workflow_output,
            None,
        )
        .expect("materialize verified recovery artifacts");
        let args = serde_json::json!({
            "run_id": run_id,
            "input": {
                "query": query,
                "current_date": "2026-07-23",
                "evidence_scope": "web_and_workspace",
                "inquiry_host_managed": true
            }
        });
        record_deep_research_workflow_started(
            workspace.path(),
            run_id,
            deep_research_evidence_first_research_spec(&args),
        )
        .await
        .expect("start invalid-publication journal");
        record_deep_research_workflow_completed(workspace.path(), run_id, true)
            .await
            .expect("close invalid-publication workflow track");

        let rejected =
            finalize_deep_research_terminal_journal(DeepResearchTerminalJournalRequest {
                workspace: workspace.path(),
                run_id,
                query: Some(query),
                host_managed_inquiry: true,
                workflow_output: &workflow_output,
                workflow_metadata: None,
                outcome: ResearchOutcome::Degraded,
                artifacts: Some(&artifacts),
                artifact_authority: Some(
                    DeepResearchTerminalArtifactAuthority::ValidatedPublication,
                ),
            })
            .await
            .expect_err("an invalid publication must fail closed without recovery authority");
        assert!(rejected.contains("omitted its status"), "{rejected}");

        let projection =
            finalize_deep_research_terminal_journal(DeepResearchTerminalJournalRequest {
                workspace: workspace.path(),
                run_id,
                query: Some(query),
                host_managed_inquiry: true,
                workflow_output: &workflow_output,
                workflow_metadata: None,
                outcome: ResearchOutcome::Degraded,
                artifacts: Some(&artifacts),
                artifact_authority: Some(DeepResearchTerminalArtifactAuthority::VerifiedRecovery),
            })
            .await
            .expect("verified recovery must close the degraded transaction");

        assert_eq!(projection.outcome, ResearchOutcome::Degraded);
        assert!(projection.active_steps.is_empty());
        assert!(projection.active_children.is_empty());
        assert!(projection.artifact_evidence_head.is_some());
    }

    #[tokio::test]
    async fn synthesized_publication_records_nonzero_claim_and_citation_metrics() {
        let workspace = tempfile::tempdir().expect("create synthesized journal workspace");
        let query = "Which Nimbus release is supported?";
        let run_id = "evidence-first-synthesized-settlement";
        let catalog = super::super::deep_research_artifacts::DeepResearchSourceCatalog {
            sources: vec![super::super::deep_research_artifacts::DeepResearchCatalogSource {
                alias: "source-1".to_string(),
                title: "Official Nimbus support record".to_string(),
                anchor: "https://docs.rs/nimbus/latest/nimbus/".to_string(),
                chunks: vec![
                    "The official record states that Nimbus version 2 receives fixes through September 2027."
                        .to_string(),
                ],
                claim_eligible: true,
                semantically_admitted: true,
                relevant_track_ids: vec!["request.primary".to_string()],
                coverage: Vec::new(),
            }],
            omitted_source_count: 0,
            omitted_chunk_count: 0,
        };
        let report = super::super::deep_research_artifacts::admit_deep_research_report_proposal(
            query,
            &catalog,
            serde_json::json!({
                "labels": {
                    "answer": "Direct Answer",
                    "findings": "Findings",
                    "recommendations": "Recommendations",
                    "limitations": "Limitations",
                    "evidence_boundary": "This report publishes no conclusion beyond the fetched evidence.",
                    "sources": "Sources"
                },
                "summary": [{
                    "text": "Nimbus version 2 receives fixes through September 2027.",
                    "source_aliases": ["source-1"],
                    "track_ids": ["request.primary"]
                }],
                "findings": [{
                    "text": "The official record identifies Nimbus version 2 and September 2027 as the support boundary.",
                    "source_aliases": ["source-1"],
                    "track_ids": ["request.primary"]
                }],
                "recommendations": [],
                "limitations": []
            }),
        )
        .expect("admit synthesized settlement report")
        .expect("quality-gated settlement report");
        let artifacts =
            super::super::deep_research_artifacts::materialize_deep_research_admitted_report(
                workspace.path(),
                query,
                &report,
            )
            .expect("materialize synthesized settlement report");
        let slug = super::super::deep_research_artifacts::deep_research_report_slug(query);
        let workflow_output = serde_json::json!({
            "query": query,
            "mode": "evidence_first_report",
            "publication": {
                "status": "synthesized",
                "markdown": format!(".a3s/research/{slug}/report.md"),
                "html": format!(".a3s/research/{slug}/index.html"),
                "quality": {
                    "direct_answer_count": 1,
                    "finding_count": 1,
                    "accepted_claim_count": 2,
                    "cited_source_count": 1,
                    "substantive_character_count": report.substantive_character_count,
                    "relevant_source_count": 1,
                    "source_count": 1
                }
            }
        })
        .to_string();
        let args = serde_json::json!({
            "run_id": run_id,
            "input": {
                "query": query,
                "current_date": "2026-07-22",
                "evidence_scope": "web_and_workspace",
                "inquiry_host_managed": true
            }
        });
        record_deep_research_workflow_started(
            workspace.path(),
            run_id,
            deep_research_evidence_first_research_spec(&args),
        )
        .await
        .expect("start synthesized settlement journal");
        record_deep_research_workflow_completed(workspace.path(), run_id, true)
            .await
            .expect("close synthesized workflow track");

        let projection =
            finalize_deep_research_terminal_journal(DeepResearchTerminalJournalRequest {
                workspace: workspace.path(),
                run_id,
                query: Some(query),
                host_managed_inquiry: true,
                workflow_output: &workflow_output,
                workflow_metadata: None,
                outcome: ResearchOutcome::Completed,
                artifacts: Some(&artifacts),
                artifact_authority: Some(
                    DeepResearchTerminalArtifactAuthority::ValidatedPublication,
                ),
            })
            .await
            .expect("settle synthesized publication");

        assert_eq!(projection.outcome, ResearchOutcome::Completed);
        assert_eq!(projection.accepted_evidence_count, 1);
        assert_eq!(projection.source_count, 1);
        assert_eq!(projection.claim_count, 2);
        assert_eq!(projection.report_cited_source_count, Some(1));
    }
}
