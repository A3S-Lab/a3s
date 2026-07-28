//! Headless TUI and DeepResearch smoke-mode execution.

use super::*;

/// Headless probe of the same `session.stream()` / `AgentEvent` path the TUI
/// uses. A headless process has no human authority, so any unexpected
/// confirmation request is rejected rather than manufacturing consent.
pub(super) async fn run_smoke(
    session: Arc<AgentSession>,
    workspace: &Path,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
) -> anyhow::Result<()> {
    let prompt = std::env::var("A3S_CODE_TUI_PROMPT")
        .unwrap_or_else(|_| "Reply with exactly one short sentence: what is 2 + 2?".to_string());
    if let Some(query) = prompt.trim().strip_prefix('?') {
        let query = query.trim().to_string();
        if query.is_empty() {
            anyhow::bail!("A3S_CODE_TUI_PROMPT starts with `?` but has no DeepResearch query");
        }
        return run_smoke_deep_research(session, workspace, query, deep_research_report_tool_gate)
            .await;
    }
    eprintln!("[smoke] prompt: {prompt}");
    let _ = stream_smoke_prompt(session.as_ref(), prompt.as_str()).await?;
    Ok(())
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct SmokePhaseDeadline {
    pub(super) phase: &'static str,
    pub(super) run_deadline: Instant,
    pub(super) phase_deadline: Instant,
    pub(super) selected_timeout: Duration,
}

pub(super) fn deep_research_smoke_run_deadline(started_at: Instant) -> Instant {
    started_at + Duration::from_millis(DEEP_RESEARCH_RUN_HARD_TIMEOUT_MS)
}

pub(super) fn deep_research_smoke_execution_deadline(run_deadline: Instant) -> Instant {
    run_deadline
        .checked_sub(Duration::from_millis(
            DEEP_RESEARCH_SMOKE_FINALIZATION_RESERVE_MS,
        ))
        .unwrap_or(run_deadline)
}

pub(super) fn deep_research_smoke_remaining_budget(
    run_deadline: Instant,
    now: Instant,
) -> Duration {
    run_deadline.saturating_duration_since(now)
}

pub(super) fn deep_research_smoke_phase_deadline(
    run_deadline: Instant,
    now: Instant,
    phase_limit: Duration,
    phase: &'static str,
) -> Option<SmokePhaseDeadline> {
    deep_research_smoke_bounded_phase_deadline(
        run_deadline,
        deep_research_smoke_execution_deadline(run_deadline),
        now,
        phase_limit,
        phase,
    )
}

pub(super) fn deep_research_smoke_finalization_phase_deadline(
    run_deadline: Instant,
    now: Instant,
    phase_limit: Duration,
    phase: &'static str,
) -> Option<SmokePhaseDeadline> {
    deep_research_smoke_bounded_phase_deadline(run_deadline, run_deadline, now, phase_limit, phase)
}

fn deep_research_smoke_bounded_phase_deadline(
    run_deadline: Instant,
    budget_deadline: Instant,
    now: Instant,
    phase_limit: Duration,
    phase: &'static str,
) -> Option<SmokePhaseDeadline> {
    let selected_timeout = budget_deadline
        .saturating_duration_since(now)
        .min(phase_limit);
    if selected_timeout.is_zero() {
        return None;
    }
    Some(SmokePhaseDeadline {
        phase,
        run_deadline,
        phase_deadline: now + selected_timeout,
        selected_timeout,
    })
}

#[cfg(test)]
pub(super) fn deep_research_smoke_exhausted_phase_message(phase: &str) -> String {
    format!(
        "DeepResearch {phase} model call timed out after 0 ms because the bounded execution budget was exhausted before the phase could start."
    )
}

impl SmokePhaseDeadline {
    fn phase_remaining(self, now: Instant) -> Duration {
        self.phase_deadline.saturating_duration_since(now)
    }

    fn run_remaining(self, now: Instant) -> Duration {
        deep_research_smoke_remaining_budget(self.run_deadline, now)
    }

    fn selected_timeout_ms(self) -> u64 {
        self.selected_timeout.as_millis().min(u128::from(u64::MAX)) as u64
    }

    fn timeout_message(self) -> String {
        format!(
            "DeepResearch {} model call timed out after {} ms.",
            self.phase,
            self.selected_timeout_ms()
        )
    }
}

fn deep_research_smoke_deadline_error(phase: &str) -> anyhow::Error {
    anyhow::anyhow!(
        "DeepResearch smoke exhausted its absolute {} ms run budget before {phase}",
        DEEP_RESEARCH_RUN_HARD_TIMEOUT_MS
    )
}

fn ensure_deep_research_smoke_budget(run_deadline: Instant, phase: &str) -> anyhow::Result<()> {
    if deep_research_smoke_remaining_budget(run_deadline, Instant::now()).is_zero() {
        Err(deep_research_smoke_deadline_error(phase))
    } else {
        Ok(())
    }
}

pub(super) fn run_deep_research_smoke_artifact_step<T>(
    run_deadline: Instant,
    phase: &str,
    operation: impl FnOnce() -> T,
) -> anyhow::Result<T> {
    ensure_deep_research_smoke_budget(run_deadline, phase)?;
    let result = operation();
    ensure_deep_research_smoke_budget(run_deadline, phase)?;
    Ok(result)
}

async fn stream_smoke_prompt(session: &AgentSession, prompt: &str) -> anyhow::Result<String> {
    stream_smoke_prompt_inner(session, prompt, None).await
}

async fn stream_smoke_prompt_inner(
    session: &AgentSession,
    prompt: &str,
    deadline: Option<SmokePhaseDeadline>,
) -> anyhow::Result<String> {
    let (mut rx, join) = if let Some(deadline) = deadline {
        let remaining = deadline.phase_remaining(Instant::now());
        if remaining.is_zero() {
            let message = deadline.timeout_message();
            eprintln!("\n[smoke] {message}");
            return Ok(message);
        }
        match tokio::time::timeout(remaining, session.stream(prompt, None)).await {
            Ok(result) => result?,
            Err(_) => {
                if let Some(abort_deadline) = deep_research_smoke_finalization_phase_deadline(
                    deadline.run_deadline,
                    Instant::now(),
                    Duration::from_millis(DEEP_RESEARCH_ABORT_GRACE_MS),
                    "abort",
                ) {
                    let cancel_budget = abort_deadline.phase_remaining(Instant::now());
                    if !cancel_budget.is_zero() {
                        let _ = tokio::time::timeout(
                            cancel_budget,
                            session.cancel_and_settle(Duration::ZERO, cancel_budget),
                        )
                        .await;
                    }
                }
                let message = deadline.timeout_message();
                eprintln!("\n[smoke] {message}");
                return Ok(message);
            }
        }
    } else {
        session.stream(prompt, None).await?
    };
    let abort = join.abort_handle();
    let mut streamed = String::new();
    let mut end_text = String::new();
    let mut phase_timer = deadline
        .map(|deadline| Box::pin(tokio::time::sleep(deadline.phase_remaining(Instant::now()))));
    loop {
        let event = if let Some(phase_timer) = phase_timer.as_mut() {
            tokio::select! {
                event = rx.recv() => event,
                _ = phase_timer.as_mut() => {
                    let deadline = deadline.expect("phase timer implies deadline");
                    let abort_deadline = deep_research_smoke_finalization_phase_deadline(
                        deadline.run_deadline,
                        Instant::now(),
                        Duration::from_millis(DEEP_RESEARCH_ABORT_GRACE_MS),
                        "abort",
                    );
                    if let Some(abort_deadline) = abort_deadline {
                        let cancel_budget = abort_deadline.phase_remaining(Instant::now());
                        if !cancel_budget.is_zero() {
                            let _ = tokio::time::timeout(
                                cancel_budget,
                                session.cancel_and_settle(Duration::ZERO, cancel_budget),
                            )
                            .await;
                        }
                        let join_budget = abort_deadline.phase_remaining(Instant::now());
                        if join_budget.is_zero()
                            || tokio::time::timeout(join_budget, join).await.is_err()
                        {
                            abort.abort();
                        }
                    } else {
                        abort.abort();
                    }
                    let message = deadline.timeout_message();
                    eprintln!("\n[smoke] {message}");
                    return Ok(message);
                }
            }
        } else {
            rx.recv().await
        };
        let Some(event) = event else {
            break;
        };
        match event {
            AgentEvent::TextDelta { text } => {
                streamed.push_str(&text);
                print!("{text}");
            }
            AgentEvent::ToolStart { name, .. } => eprintln!("\n[tool start] {name}"),
            AgentEvent::ToolEnd {
                name,
                exit_code,
                output,
                ..
            } => {
                eprintln!(
                    "[tool end] {name} (exit {exit_code}): {}",
                    output.lines().take(2).collect::<Vec<_>>().join(" | ")
                );
            }
            AgentEvent::ConfirmationRequired {
                tool_id, tool_name, ..
            } => {
                eprintln!("[confirm] rejecting {tool_name}: headless smoke has no approver");
                let reason = Some(
                    "Denied because headless smoke execution cannot obtain human approval."
                        .to_string(),
                );
                if let Some(deadline) = deadline {
                    let confirmation_budget = deadline
                        .phase_remaining(Instant::now())
                        .min(deadline.run_remaining(Instant::now()));
                    if !confirmation_budget.is_zero() {
                        let _ = tokio::time::timeout(
                            confirmation_budget,
                            session.confirm_tool_use(&tool_id, false, reason),
                        )
                        .await;
                    }
                } else {
                    let _ = session.confirm_tool_use(&tool_id, false, reason).await;
                }
            }
            AgentEvent::End { text, .. } => {
                if streamed.trim().is_empty() && !text.trim().is_empty() {
                    print!("{text}");
                }
                end_text = text;
                eprintln!("\n[end]");
                break;
            }
            AgentEvent::Error { message } => eprintln!("\n[error] {message}"),
            _ => {}
        }
    }
    // Let the stream task finish (incl. auto-save/persist) before we exit.
    if let Some(deadline) = deadline {
        // An End event already gives us the model result. Persisting the stream
        // worker may use the execution phase's remaining time, but it must not
        // consume the window reserved for recovery artifact publication.
        let join_budget = deadline
            .phase_remaining(Instant::now())
            .min(Duration::from_secs(30));
        if join_budget.is_zero() {
            abort.abort();
        } else {
            match tokio::time::timeout(join_budget, join).await {
                Ok(result) => result?,
                Err(_) => {
                    abort.abort();
                    eprintln!(
                        "[smoke] stream worker did not finish before the execution deadline; continuing with artifact finalization"
                    );
                }
            }
        }
    } else {
        tokio::time::timeout(Duration::from_secs(30), join)
            .await
            .map_err(|_| {
                anyhow::anyhow!("smoke stream worker did not finish after AgentEvent::End")
            })??;
    }
    if end_text.trim().is_empty() {
        Ok(streamed)
    } else {
        Ok(end_text)
    }
}

async fn run_smoke_deep_research(
    session: Arc<AgentSession>,
    workspace: &Path,
    query: String,
    deep_research_report_tool_gate: DeepResearchReportToolGate,
) -> anyhow::Result<()> {
    let run_started_at = Instant::now();
    let run_deadline = deep_research_smoke_run_deadline(run_started_at);
    let evidence_scope = deep_research_default_evidence_scope();
    deep_research_report_tool_gate.set_workspace(workspace);
    deep_research_report_tool_gate.set_evidence_scope(evidence_scope);
    eprintln!("[smoke] deepresearch workflow: host-managed");
    let mut workflow_args = deep_research_workflow_args_with_scope(&query, evidence_scope);
    ensure_deep_research_workflow_run_id(&mut workflow_args);
    let run_id = workflow_args
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("DeepResearch smoke workflow has no run_id"))?
        .to_string();
    record_deep_research_workflow_started(
        workspace,
        &run_id,
        deep_research_evidence_first_research_spec(&workflow_args),
    )
    .await?;
    let (mut progress_rx, mut workflow_join) =
        spawn_deep_research_evidence_first(Arc::clone(&session), workflow_args.clone());
    let workflow_abort = workflow_join.abort_handle();
    let progress_drain = tokio::spawn(async move {
        while let Some(event) = progress_rx.recv().await {
            match event {
                AgentEvent::SubagentStart {
                    task_id,
                    agent,
                    description,
                    ..
                } => eprintln!("[smoke] child start: {agent} {task_id} · {description}"),
                AgentEvent::SubagentProgress {
                    task_id, status, ..
                } => eprintln!("[smoke] child progress: {task_id} · {status}"),
                AgentEvent::SubagentEnd {
                    task_id,
                    success,
                    output,
                    ..
                } => eprintln!(
                    "[smoke] child end: {task_id} · {} · {}",
                    if success { "ok" } else { "failed" },
                    output.lines().next().unwrap_or_default()
                ),
                AgentEvent::ToolExecutionStart { name, args, .. } => eprintln!(
                    "[smoke] child tool start: {name} · {}",
                    args.to_string().chars().take(240).collect::<String>()
                ),
                AgentEvent::ToolEnd {
                    name,
                    exit_code,
                    output,
                    ..
                } => eprintln!(
                    "[smoke] child tool end: {name} ({exit_code}) · {}",
                    output
                        .lines()
                        .next()
                        .unwrap_or_default()
                        .chars()
                        .take(240)
                        .collect::<String>()
                ),
                AgentEvent::PermissionDenied {
                    tool_name, reason, ..
                } => eprintln!("[smoke] child tool denied: {tool_name} · {reason}"),
                AgentEvent::Error { message } => eprintln!("[smoke] child error: {message}"),
                _ => {}
            }
        }
    });
    let configured_timeout_ms = DEEP_RESEARCH_EVIDENCE_FIRST_HOST_TIMEOUT_MS;
    let workflow_deadline = deep_research_smoke_phase_deadline(
        run_deadline,
        Instant::now(),
        Duration::from_millis(configured_timeout_ms),
        "workflow",
    )
    .ok_or_else(|| deep_research_smoke_deadline_error("workflow"))?;
    let timeout_ms = workflow_deadline.selected_timeout_ms();
    let workflow = match tokio::time::timeout(
        workflow_deadline.phase_remaining(Instant::now()),
        &mut workflow_join,
    )
    .await
    {
        Ok(Ok(result)) => result.map_err(|err| err.to_string()),
        Ok(Err(err)) => Err(err.to_string()),
        Err(_) => {
            workflow_abort.abort();
            let abort_grace = workflow_deadline
                .run_remaining(Instant::now())
                .min(Duration::from_millis(DEEP_RESEARCH_ABORT_GRACE_MS));
            if !abort_grace.is_zero() {
                let _ = tokio::time::timeout(abort_grace, &mut workflow_join).await;
            }
            let _ = session
                .cancel_and_settle(
                    Duration::from_millis(DEEP_RESEARCH_ABORT_GRACE_MS),
                    Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                )
                .await;
            let message = format!(
                "dynamic_workflow timed out after {timeout_ms} ms while gathering DeepResearch evidence"
            );
            run_deep_research_smoke_artifact_step(
                run_deadline,
                "workflow timeout artifact fallback",
                || deep_research_workflow_timeout_tool_result(workspace, &workflow_args, message),
            )?
        }
    };
    progress_drain.abort();

    let (mut workflow_output, exit_code, metadata) = match workflow {
        Ok(result) => (result.output, result.exit_code, result.metadata),
        Err(error) => (error, 1, None),
    };
    workflow_output = deep_research_canonical_workflow_output(&workflow_output, metadata.as_ref());
    eprintln!("[smoke] deepresearch workflow exit: {exit_code}");

    let published = match resolve_deep_research_run_publication(
        workspace,
        &query,
        &run_id,
        &workflow_output,
    ) {
        Ok(Some(published)) => published,
        publication => {
            let reason = match publication {
                Ok(None) if exit_code == 0 => {
                    "the standalone DeepResearch engine returned without its required Host publication"
                        .to_string()
                }
                Ok(None) => workflow_output.clone(),
                Err(error) => {
                    format!("the standalone DeepResearch publication failed validation: {error}")
                }
                Ok(Some(_)) => unreachable!("validated publication matched the prior arm"),
            };
            deep_research_report_tool_gate.reset();
            let artifacts = run_deep_research_smoke_artifact_step(
                run_deadline,
                "failed-engine recovery report",
                || {
                    materialize_deep_research_recovery_report(
                        workspace,
                        &query,
                        &reason,
                        &workflow_output,
                        metadata.as_ref(),
                    )
                },
            )?
            .map_err(anyhow::Error::msg)?;
            eprintln!("[smoke] standalone engine did not publish a validated report: {reason}");
            eprintln!(
                "[smoke] recovery report.md: {}",
                artifacts.markdown.display()
            );
            eprintln!("[smoke] recovery index.html: {}", artifacts.html.display());
            let settled = settle_deep_research_cli_run(DeepResearchCliSettlement {
                workspace,
                run_id: &run_id,
                query: &query,
                workflow_succeeded: exit_code == 0,
                workflow_output: &workflow_output,
                workflow_metadata: metadata.as_ref(),
                requested_outcome: ResearchOutcome::Degraded,
                artifacts: &artifacts,
                artifact_authority: DeepResearchTerminalArtifactAuthority::VerifiedRecovery,
            })
            .await
            .map_err(anyhow::Error::msg)?;
            let outcome = match settled {
                ResearchOutcome::Completed => DeepResearchRunOutcome::Completed,
                ResearchOutcome::Qualified => DeepResearchRunOutcome::Qualified,
                ResearchOutcome::Degraded | ResearchOutcome::Failed => {
                    DeepResearchRunOutcome::Degraded
                }
                ResearchOutcome::Active => {
                    unreachable!("terminal recovery settlement remained active")
                }
            };
            return outcome.ensure_smoke_success(&artifacts);
        }
    };

    let outcome = match published.publication {
        DeepResearchEvidenceFirstPublication::Synthesized => DeepResearchRunOutcome::Completed,
        DeepResearchEvidenceFirstPublication::Qualified => DeepResearchRunOutcome::Qualified,
        DeepResearchEvidenceFirstPublication::SourceBacked => DeepResearchRunOutcome::SourceBacked,
        DeepResearchEvidenceFirstPublication::NoEvidence => DeepResearchRunOutcome::NoEvidence,
    };
    let journal_outcome = match outcome {
        DeepResearchRunOutcome::Completed => ResearchOutcome::Completed,
        DeepResearchRunOutcome::Qualified => ResearchOutcome::Qualified,
        DeepResearchRunOutcome::SourceBacked
        | DeepResearchRunOutcome::NoEvidence
        | DeepResearchRunOutcome::Degraded => ResearchOutcome::Degraded,
        DeepResearchRunOutcome::Active => {
            unreachable!("a validated evidence-first publication is terminal")
        }
    };
    let settled = settle_deep_research_cli_run(DeepResearchCliSettlement {
        workspace,
        run_id: &run_id,
        query: &query,
        workflow_succeeded: exit_code == 0,
        workflow_output: &workflow_output,
        workflow_metadata: metadata.as_ref(),
        requested_outcome: journal_outcome,
        artifacts: &published.artifacts,
        artifact_authority: DeepResearchTerminalArtifactAuthority::ValidatedPublication,
    })
    .await
    .map_err(anyhow::Error::msg)?;
    if settled != journal_outcome {
        anyhow::bail!(
            "DeepResearch smoke journal outcome {settled:?} disagrees with publication outcome {journal_outcome:?}"
        );
    }

    deep_research_report_tool_gate.reset();
    let final_text = clean_deep_research_final_text_from_artifacts(&published.artifacts, workspace)
        .unwrap_or_else(|| {
            "DeepResearch published a report, but its Markdown preview was unavailable.".to_string()
        });
    if !final_text.trim().is_empty() {
        println!("{final_text}");
    }
    match published.publication {
        DeepResearchEvidenceFirstPublication::Synthesized => {
            eprintln!(
                "[smoke] quality-gated report.md: {}",
                published.artifacts.markdown.display()
            );
            eprintln!(
                "[smoke] quality-gated index.html: {}",
                published.artifacts.html.display()
            );
        }
        DeepResearchEvidenceFirstPublication::Qualified => {
            eprintln!(
                "[smoke] qualified report with explicit evidence boundaries: {}",
                published.artifacts.html.display()
            );
        }
        DeepResearchEvidenceFirstPublication::SourceBacked => {
            eprintln!(
                "[smoke] report synthesis did not pass the quality gate; source-backed report: {}",
                published.artifacts.html.display()
            );
        }
        DeepResearchEvidenceFirstPublication::NoEvidence => {
            eprintln!(
                "[smoke] no safely publishable evidence; boundary report: {}",
                published.artifacts.html.display()
            );
        }
    }
    run_deep_research_smoke_artifact_step(run_deadline, "final report validation", || {
        outcome.ensure_smoke_success(&published.artifacts)
    })?
}
