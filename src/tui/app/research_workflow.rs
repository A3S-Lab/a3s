//! Typed DeepResearch launch, progress, and completion controller actions.

use super::*;

impl App {
    pub(super) fn start_deep_research_workflow(
        &mut self,
        query: String,
        evidence_scope: DeepResearchEvidenceScope,
        runtime_expectation: Option<RuntimeExpectation>,
    ) -> Option<Cmd<Msg>> {
        if self.deep_research_handle.is_some() {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  a DeepResearch run is already active"),
            );
            return None;
        }

        let budget = deep_research_budget_for_effort_index(self.effort, self.context_limit);
        let runner_budget = CodeDeepResearchRunnerBudget {
            local_max_steps: budget.deep_research_child_steps,
            max_tool_calls: budget.workflow_max_tool_calls,
            max_output_bytes: budget.workflow_max_output_bytes,
        };
        let engine_scope = match evidence_scope {
            DeepResearchEvidenceScope::LocalOnly => EvidenceScope::LocalOnly,
            DeepResearchEvidenceScope::WebAndWorkspace => EvidenceScope::WebAndWorkspace,
        };
        let request = match build_code_deep_research_request(
            None,
            &query,
            engine_scope,
            runner_budget,
            Vec::new(),
        ) {
            Ok(request) => request,
            Err(error) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  could not prepare DeepResearch: {error}")),
                );
                return None;
            }
        };
        let workflow_args = match request.to_workflow_arguments() {
            Ok(arguments) => arguments,
            Err(error) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  could not prepare DeepResearch: {error}")),
                );
                return None;
            }
        };
        let run_id = request.run_id.clone();

        self.auto_review.on_user_turn();
        self.last_activity = Instant::now();
        self.streaming.clear();
        self.got_delta = false;
        self.turn_text.clear();
        self.turn_had_agent_activity = false;
        self.turn_text_after_activity = false;
        if self.deep_research_goal_restore.is_none() {
            self.deep_research_goal_restore = Some((self.goal.clone(), self.goal_since));
        }
        self.goal = Some(deep_research_goal(&query));
        self.goal_since = Some(Instant::now());
        self.engage_single_turn_autonomy();
        self.deep_research_loop = Some(DeepResearchLoop {
            query: query.clone(),
            evidence_scope,
            started_at: Instant::now(),
        });
        self.deep_research_workflow.reset_for_run();
        self.deep_research_workflow.args = Some(workflow_args);
        self.deep_research_workflow.typed_runner = true;
        self.deep_research_outcome = DeepResearchRunOutcome::Active;
        self.deep_research_subagent_settlement_inflight = false;
        self.deep_research_journal_finalization_inflight = false;
        self.deep_research_terminal_artifacts = None;
        self.deep_research_agent_event_sequence = 0;
        self.deep_research_projection = None;
        self.deep_research_events = None;
        self.pending_deep_research_report_view = None;
        self.deep_research_report_tool_gate
            .set_workspace(Path::new(&self.cwd));
        self.deep_research_report_tool_gate
            .set_evidence_scope(evidence_scope);
        if let Some(expectation) = runtime_expectation {
            self.runtime_expectation = Some(expectation);
        }
        self.ultracode_synthesis_inflight = false;
        self.ultracode_synthesis_used = false;
        self.last_paint = None;
        self.viewport.set_auto_scroll(true);
        self.plan.clear();
        self.runtime.clear_turn_entities();
        let display_task = format!("✦\u{200A}{query}");
        self.runtime.set_subagent_task(display_task.clone());
        self.running_task = Some(display_task);
        self.state = State::Streaming;
        self.relayout();
        self.stream_started = Some(Instant::now());
        self.spinner.start();
        self.push_line(
            &Style::new()
                .fg(TN_GRAY)
                .render("  ⇉ starting isolated evidence-first research…"),
        );
        self.rebuild_viewport();

        let mut code_config = (*self.code_config).clone();
        if self.model.is_some() {
            code_config.default_model = self.model.clone();
        }
        let selected_llm = self.effort_session_opts(false).llm_client.clone();
        let runner = CodeDeepResearchRunner::new(
            PathBuf::from(&self.cwd),
            code_config,
            self.memory_dir.clone(),
        );
        Some(cmd::batch(vec![
            cmd::cmd(move || async move {
                let result = runner
                    .start(
                        CodeDeepResearchLaunch {
                            request,
                            skill_names: Vec::new(),
                        },
                        move |config, options, session_id| {
                            if let Some(client) = selected_llm {
                                Ok(client)
                            } else {
                                crate::session_llm::resolve_session_llm_client(
                                    config, options, session_id,
                                )
                            }
                        },
                    )
                    .await
                    .and_then(|mut handle| {
                        let events = handle.take_events().ok_or_else(|| {
                            "DeepResearch event stream was already consumed".to_string()
                        })?;
                        Ok((handle, Arc::new(Mutex::new(events))))
                    });
                Msg::DeepResearchRunStarted { run_id, result }
            }),
            spinner_tick(),
            stream_commit_tick(),
        ]))
    }

    pub(super) fn on_deep_research_run_started(
        &mut self,
        run_id: String,
        result: Result<(CodeDeepResearchRunHandle, SharedDeepResearchRx), String>,
    ) -> Option<Cmd<Msg>> {
        if !self.deep_research_run_is_current(&run_id)
            || self.state != State::Streaming
            || self.interrupting
        {
            return result
                .ok()
                .map(|(handle, _)| discard_deep_research_run(run_id, handle));
        }
        let (handle, events) = match result {
            Ok(started) => started,
            Err(error) => return self.fail_deep_research_run(&run_id, &error),
        };
        let completion = handle.completion_signal();
        self.deep_research_handle = Some(handle);
        self.deep_research_events = Some(events.clone());
        self.rx = None;
        self.stream_join = None;
        self.host_tool_abort = None;
        self.host_progress_inflight = true;
        self.host_tool_call_id = None;
        self.push_line(
            &Style::new()
                .fg(TN_GRAY)
                .render("  ◇ DeepResearch runtime ready"),
        );
        Some(cmd::batch(vec![
            pump_deep_research(events),
            wait_for_deep_research_completion(run_id, completion),
        ]))
    }

    pub(super) fn on_deep_research_run_event(
        &mut self,
        source: SharedDeepResearchRx,
        event: CodeDeepResearchEvent,
    ) -> Option<Cmd<Msg>> {
        if !self
            .deep_research_events
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &source))
            || self.interrupting
        {
            return None;
        }
        let command = match event {
            CodeDeepResearchEvent::Agent(event) => {
                if host_progress_event_is_terminal(&event) {
                    None
                } else {
                    self.on_agent_event(event)
                }
            }
            CodeDeepResearchEvent::Engine(event) => {
                self.present_deep_research_engine_event(&event);
                None
            }
        };
        let next = pump_deep_research(source);
        Some(match command {
            Some(command) => cmd::batch(vec![command, next]),
            None => next,
        })
    }

    pub(super) fn on_deep_research_events_ended(
        &mut self,
        source: SharedDeepResearchRx,
    ) -> Option<Cmd<Msg>> {
        if self
            .deep_research_events
            .as_ref()
            .is_some_and(|current| Arc::ptr_eq(current, &source))
        {
            self.deep_research_events = None;
        }
        None
    }

    pub(super) fn on_deep_research_run_ready(&mut self, run_id: String) -> Option<Cmd<Msg>> {
        if !self.deep_research_run_is_current(&run_id) || self.interrupting {
            return None;
        }
        let handle = self.deep_research_handle.take()?;
        self.deep_research_events = None;
        self.host_progress_inflight = false;
        settle_deep_research_run(run_id, handle).into()
    }

    pub(super) fn on_deep_research_run_settled(
        &mut self,
        run_id: String,
        result: Result<CodeDeepResearchRunExit, String>,
    ) -> Option<Cmd<Msg>> {
        if !self.deep_research_run_is_current(&run_id)
            || self.state != State::Streaming
            || self.interrupting
        {
            return None;
        }
        self.deep_research_handle = None;
        self.deep_research_events = None;
        self.host_progress_inflight = false;
        self.host_tool_call_id = None;

        let result = match result {
            Ok(CodeDeepResearchRunExit::Completed(result)) => *result,
            Ok(CodeDeepResearchRunExit::Cancelled) => {
                return self.fail_deep_research_run(
                    &run_id,
                    "the research run was cancelled before publication",
                )
            }
            Err(error) => return self.fail_deep_research_run(&run_id, &error),
        };
        if result.lifecycle != DeepResearchLifecycle::Completed {
            return self.fail_deep_research_run(
                &run_id,
                &format!(
                    "the research run settled with lifecycle {:?}",
                    result.lifecycle
                ),
            );
        }

        self.deep_research_workflow.output = Some(result.output_json());
        self.deep_research_workflow.metadata = None;
        let outcome = match result.publication {
            PublicationOutcome::Synthesized => DeepResearchRunOutcome::Completed,
            PublicationOutcome::Qualified => DeepResearchRunOutcome::Qualified,
            PublicationOutcome::SourceBacked => DeepResearchRunOutcome::SourceBacked,
            PublicationOutcome::NoEvidence => DeepResearchRunOutcome::NoEvidence,
        };
        let final_text =
            clean_deep_research_final_text_from_artifacts(&result.artifacts, Path::new(&self.cwd))
                .unwrap_or_else(|| {
                    "DeepResearch published a report, but its Markdown preview was unavailable."
                        .to_string()
                });
        self.loop_remaining = 0;
        self.stage_deep_research_report(
            &result.artifacts,
            outcome,
            DeepResearchTerminalArtifactAuthority::ValidatedPublication,
        );
        self.mark_assistant_text(&final_text);
        self.turn_text.clear();
        self.turn_text.push_str(&final_text);
        self.messages
            .push(TranscriptEntry::assistant_markdown(final_text));
        let (color, message) = match result.publication {
            PublicationOutcome::Synthesized => (
                TN_GREEN,
                format!(
                    "  ✓ DeepResearch published a quality-gated synthesized report at {}",
                    result.artifacts.html.display()
                ),
            ),
            PublicationOutcome::Qualified => (
                TN_YELLOW,
                format!(
                    "  ◐ DeepResearch published a qualified report with explicit evidence boundaries at {}",
                    result.artifacts.html.display()
                ),
            ),
            PublicationOutcome::SourceBacked => (
                TN_YELLOW,
                format!(
                    "  ⚠ DeepResearch preserved a source-backed report after synthesis did not pass admission at {}",
                    result.artifacts.html.display()
                ),
            ),
            PublicationOutcome::NoEvidence => (
                TN_YELLOW,
                format!(
                    "  ⚠ DeepResearch published an explicit no-evidence boundary report at {}",
                    result.artifacts.html.display()
                ),
            ),
        };
        self.push_line(&Style::new().fg(color).render(&message));
        self.rebuild_viewport();
        self.complete_turn()
    }

    fn deep_research_run_is_current(&self, run_id: &str) -> bool {
        self.deep_research_workflow
            .args
            .as_ref()
            .and_then(|args| args.get("run_id"))
            .and_then(serde_json::Value::as_str)
            == Some(run_id)
    }

    fn present_deep_research_engine_event(&mut self, event: &DeepResearchEvent) {
        match event {
            DeepResearchEvent::StageStarted { stage, .. } => self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render(&format!("  ◇ {}…", deep_research_stage_label(*stage))),
            ),
            DeepResearchEvent::StageDegraded { stage, reason, .. } => {
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                    "  ◐ {} degraded: {reason}",
                    deep_research_stage_label(*stage)
                )))
            }
            DeepResearchEvent::PublicationCompleted {
                outcome, quality, ..
            } => self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  ◇ publication {} · {}/{} relevant sources",
                publication_outcome_label(*outcome),
                quality.relevant_source_count,
                quality.source_count
            ))),
            DeepResearchEvent::RunFailed { message, .. } => self.push_line(
                &Style::new()
                    .fg(TN_RED)
                    .render(&format!("  DeepResearch failed: {message}")),
            ),
            DeepResearchEvent::RunStarted { .. }
            | DeepResearchEvent::StageCompleted { .. }
            | DeepResearchEvent::RunCompleted { .. }
            | DeepResearchEvent::RunCancelled { .. } => {}
        }
    }

    fn fail_deep_research_run(&mut self, run_id: &str, error: &str) -> Option<Cmd<Msg>> {
        if !self.deep_research_run_is_current(run_id) {
            return None;
        }
        self.deep_research_handle = None;
        self.deep_research_events = None;
        self.host_progress_inflight = false;
        self.loop_remaining = 0;
        let status = format!("DeepResearch failed before publishing a report: {error}");
        self.push_line(&Style::new().fg(TN_RED).render(&format!("  ✗ {status}")));
        self.mark_assistant_text(&status);
        self.turn_text.clear();
        self.turn_text.push_str(&status);
        self.messages
            .push(TranscriptEntry::assistant_markdown(status));
        self.rebuild_viewport();
        self.complete_turn()
    }
}

fn deep_research_stage_label(stage: ResearchStage) -> &'static str {
    match stage {
        ResearchStage::Planning => "planning research questions",
        ResearchStage::BootstrapRetrieval => "gathering initial evidence",
        ResearchStage::PlannedRetrieval => "retrieving planned evidence",
        ResearchStage::SourcePublication => "publishing source-backed findings",
        ResearchStage::ReportGeneration => "synthesizing the report",
        ResearchStage::FinalPublication => "publishing final artifacts",
    }
}

fn publication_outcome_label(outcome: PublicationOutcome) -> &'static str {
    match outcome {
        PublicationOutcome::Synthesized => "synthesized",
        PublicationOutcome::Qualified => "qualified",
        PublicationOutcome::SourceBacked => "source-backed",
        PublicationOutcome::NoEvidence => "no-evidence",
    }
}
