//! Asynchronous commands, stream pumps, subagent watchers, and animation ticks.

use super::*;

/// Read one event from the active run and turn it into a `Msg`.
pub(super) fn pump(rx: SharedRx) -> Cmd<Msg> {
    let source = rx.clone();
    cmd::cmd(move || async move {
        let mut guard = rx.lock().await;
        match guard.recv().await {
            Some(event) => Msg::Agent {
                source,
                event: Box::new(event),
            },
            None => Msg::StreamEnded(source),
        }
    })
}

/// Read one event from the isolated typed DeepResearch run.
pub(super) fn pump_deep_research(rx: SharedDeepResearchRx) -> Cmd<Msg> {
    let source = rx.clone();
    cmd::cmd(move || async move {
        let mut guard = rx.lock().await;
        match guard.recv().await {
            Some(event) => Msg::DeepResearchRunEvent {
                source,
                event: Box::new(event),
            },
            None => Msg::DeepResearchRunEventsEnded { source },
        }
    })
}

/// Wait independently of the lossy progress channel for the root task to end.
pub(super) fn wait_for_deep_research_completion(
    run_id: String,
    mut completion: tokio::sync::watch::Receiver<bool>,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        if !*completion.borrow() {
            let _ = completion.changed().await;
        }
        Msg::DeepResearchRunReady { run_id }
    })
}

pub(super) fn settle_deep_research_run(
    run_id: String,
    handle: CodeDeepResearchRunHandle,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        Msg::DeepResearchRunSettled {
            run_id,
            result: handle.settle().await,
        }
    })
}

pub(super) fn discard_deep_research_run(
    run_id: String,
    handle: CodeDeepResearchRunHandle,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let _ = handle.cancel_and_settle().await;
        Msg::DeepResearchRunDiscarded { run_id }
    })
}

/// Wait for the previous stream worker to release the session's single-flight
/// admission lease before the update loop constructs any follow-up operation.
pub(super) fn wait_for_stream_join(
    session: Arc<AgentSession>,
    stream_join: StreamJoin,
    token: u64,
    synthesis: Option<(String, String)>,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        // A provider worker must never hold queued user input forever after it
        // has already emitted a terminal event. Give normal persistence and
        // lease cleanup a bounded grace period, then abort the stale worker so
        // its cancellation destructor releases the single-flight admission.
        let mut stream_join = stream_join;
        let proxy_result = tokio::time::timeout(
            Duration::from_millis(STREAM_JOIN_SETTLE_GRACE_MS),
            &mut stream_join,
        )
        .await;
        let proxy_settled = matches!(&proxy_result, Ok(Ok(())));
        let proxy_still_pending = proxy_result.is_err();
        if !proxy_settled {
            // The public handle is a guardian proxy. Aborting only that proxy
            // cannot release Core's single-flight lease, so settle the real
            // worker through the session before allowing another turn. The
            // normal grace period was already spent above (or Esc explicitly
            // aborted the proxy), so do not impose the same delay twice.
            let _ = session
                .cancel_and_settle(
                    Duration::ZERO,
                    Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                )
                .await;
            if proxy_still_pending {
                let _ = settle_stream_join_for_quit(
                    stream_join,
                    Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                )
                .await;
            }
        }
        Msg::StreamJoinSettled { token, synthesis }
    })
}

/// A stale stream start still owns a core admission lease. Cancelling the
/// originating session and awaiting its lifecycle handle prevents a detached
/// worker from poisoning the next turn with `SessionBusy`.
pub(super) fn discard_started_stream(
    session: Arc<AgentSession>,
    stream_join: StreamJoin,
    token: u64,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let _ = session
            .cancel_and_settle(
                Duration::from_millis(STREAM_JOIN_SETTLE_GRACE_MS),
                Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
            )
            .await;
        let _ = settle_stream_join_for_quit(
            stream_join,
            Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
        )
        .await;
        Msg::DiscardedStreamSettled { token }
    })
}

/// Bound host shutdown even when an extension or transport ignores session
/// cancellation. The owned task is aborted on expiry so the runtime cannot
/// retain a detached close future after the TUI has exited.
pub(super) async fn settle_session_close_for_quit<F>(close: F, grace: Duration) -> bool
where
    F: std::future::Future<Output = ()> + Send + 'static,
{
    let mut close = tokio::spawn(close);
    match tokio::time::timeout(grace, &mut close).await {
        Ok(Ok(())) => true,
        Ok(Err(error)) => {
            tracing::warn!(%error, "Session close task failed during TUI shutdown");
            false
        }
        Err(_) => {
            tracing::warn!(
                timeout = ?grace,
                "Session close did not settle before the TUI shutdown deadline"
            );
            close.abort();
            if tokio::time::timeout(
                Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                &mut close,
            )
            .await
            .is_err()
            {
                tracing::warn!(
                    timeout_ms = GRACEFUL_QUIT_ABORT_SETTLE_MS,
                    "Session close task did not acknowledge abort before host exit"
                );
            }
            false
        }
    }
}

/// Give an active stream a bounded opportunity to observe session cancellation.
/// If it does not finish, abort its task and briefly wait for Tokio to run the
/// cancellation destructor so dropping the TUI cannot silently detach it.
pub(super) async fn settle_stream_join_for_quit(
    mut stream_join: StreamJoin,
    grace: Duration,
) -> bool {
    let abort = stream_join.abort_handle();
    if tokio::time::timeout(grace, &mut stream_join).await.is_ok() {
        return true;
    }

    abort.abort();
    let _ = tokio::time::timeout(
        Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
        &mut stream_join,
    )
    .await;
    false
}

pub(super) fn host_progress_event_is_terminal(event: &AgentEvent) -> bool {
    matches!(event, AgentEvent::End { .. } | AgentEvent::Error { .. })
}

/// Remember the outer host-direct DynamicWorkflow call without letting nested
/// workflow activity replace its stable ID. The completion callback races the
/// progress channel, so accept the terminal event as a fallback when the
/// execution-start message was not painted first.
pub(super) fn capture_host_dynamic_workflow_call_id(
    host_progress_inflight: bool,
    host_tool_call_id: &mut Option<String>,
    event: &AgentEvent,
) {
    if !host_progress_inflight || host_tool_call_id.is_some() {
        return;
    }
    let (id, name) = match event {
        AgentEvent::ToolExecutionStart { id, name, .. }
        | AgentEvent::ToolOutputDelta { id, name, .. }
        | AgentEvent::ToolEnd { id, name, .. } => (id, name),
        _ => return,
    };
    if name == "dynamic_workflow" {
        *host_tool_call_id = Some(id.clone());
    }
}

pub(super) fn pump_manifest(rx: SharedManifestRx) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let mut guard = rx.lock().await;
        loop {
            match guard.recv().await {
                Ok(snapshot) => return Msg::WorkspaceManifest(Box::new(snapshot)),
                Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    return Msg::WorkspaceManifestStopped;
                }
            }
        }
    })
}

pub(super) fn spinner_tick() -> Cmd<Msg> {
    cmd::tick(Duration::from_millis(80), Msg::SpinnerTick)
}

pub(super) const STREAM_COMMIT_TICK_INTERVAL: Duration = Duration::from_nanos(8_333_334);

pub(super) fn stream_commit_tick() -> Cmd<Msg> {
    cmd::tick(STREAM_COMMIT_TICK_INTERVAL, Msg::StreamCommitTick)
}

pub(super) fn resume_after_pending_confirmation_cmd(rx: Option<SharedRx>) -> Cmd<Msg> {
    let mut cmds = vec![spinner_tick(), stream_commit_tick()];
    if let Some(rx) = rx {
        cmds.push(pump(rx));
    }
    cmd::batch(cmds)
}

const BACKGROUND_SUBAGENT_MAX_MISSING_POLLS: usize = 30;

pub(super) fn subagent_watch_is_current(
    current_session_id: &str,
    current_generation: u64,
    event_session_id: &str,
    event_generation: u64,
) -> bool {
    current_session_id == event_session_id && current_generation == event_generation
}

pub(super) fn subagent_snapshot_is_current(
    current_session_id: &str,
    current_generation: u64,
    current_request_id: u64,
    settlement_inflight: bool,
    event_session_id: &str,
    event_generation: u64,
    event_request_id: u64,
) -> bool {
    !settlement_inflight
        && current_request_id == event_request_id
        && subagent_watch_is_current(
            current_session_id,
            current_generation,
            event_session_id,
            event_generation,
        )
}

pub(super) fn subagent_snapshot_matches_spec(
    snapshot: &a3s_code_core::SubagentTaskSnapshot,
    spec: &serde_json::Value,
) -> bool {
    let agent = spec
        .get("agent")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let description = spec
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    (agent.is_empty() || snapshot.agent.is_empty() || agent == snapshot.agent)
        && (description.is_empty()
            || snapshot.description.is_empty()
            || description == snapshot.description)
}

pub(super) fn subagent_parent_result_expected_in_history(
    history: &[Message],
    snapshot: &a3s_code_core::SubagentTaskSnapshot,
) -> bool {
    history.iter().any(|message| {
        message.content.iter().any(|block| {
            let ContentBlock::ToolUse { name, input, .. } = block else {
                return false;
            };
            let specs = match name.as_str() {
                "task" => vec![input],
                "parallel_task" => input
                    .get("tasks")
                    .and_then(serde_json::Value::as_array)
                    .map(|tasks| tasks.iter().collect())
                    .unwrap_or_default(),
                _ => return false,
            };
            specs.into_iter().any(|spec| {
                subagent_snapshot_matches_spec(snapshot, spec)
                    && !spec
                        .get("background")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false)
            })
        })
    })
}

pub(super) fn load_subagent_snapshots(
    session: Arc<AgentSession>,
    session_id: String,
    generation: u64,
    request_id: u64,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let history = session.history();
        let snapshots = session
            .subagent_tasks()
            .await
            .into_iter()
            .map(|snapshot| RestoredSubagentSnapshot {
                parent_result_expected: subagent_parent_result_expected_in_history(
                    &history, &snapshot,
                ),
                snapshot,
            })
            .collect();
        Msg::SubagentSnapshots {
            session_id,
            generation,
            request_id,
            snapshots,
        }
    })
}

pub(super) fn watch_background_subagent(
    session: Arc<AgentSession>,
    session_id: String,
    generation: u64,
    task_id: String,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let mut missing_polls = 0usize;
        loop {
            if session.is_closed() || missing_polls >= BACKGROUND_SUBAGENT_MAX_MISSING_POLLS {
                return Msg::BackgroundSubagentWatchStopped {
                    session_id,
                    generation,
                    task_id,
                };
            }
            match session.subagent_task(&task_id).await {
                Some(snapshot) if snapshot.status != a3s_code_core::SubagentStatus::Running => {
                    let outcome = match snapshot.status {
                        a3s_code_core::SubagentStatus::Completed => SubagentOutcome::Succeeded,
                        a3s_code_core::SubagentStatus::Cancelled => SubagentOutcome::Cancelled,
                        a3s_code_core::SubagentStatus::Failed => SubagentOutcome::Failed,
                        a3s_code_core::SubagentStatus::Running => unreachable!(
                            "running snapshots are filtered before terminal reconciliation"
                        ),
                        _ => SubagentOutcome::TrackingLost,
                    };
                    let output = snapshot.output.unwrap_or_else(|| match snapshot.status {
                        a3s_code_core::SubagentStatus::Cancelled => "Task cancelled.".to_string(),
                        a3s_code_core::SubagentStatus::Failed => "Task failed.".to_string(),
                        _ => String::new(),
                    });
                    return Msg::BackgroundSubagentFinished {
                        session_id,
                        generation,
                        task_id: snapshot.task_id,
                        agent: snapshot.agent,
                        output,
                        outcome,
                        finished_ms: snapshot.finished_ms.unwrap_or(snapshot.updated_ms),
                    };
                }
                Some(_) => missing_polls = 0,
                None => missing_polls += 1,
            }
            // Background completion is UI-informational; one poll per second
            // avoids an idle hot loop while still surfacing results promptly.
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    })
}

pub(super) fn deep_research_subagent_cancelled_output(
    exit: DeepResearchSettlementExit,
) -> &'static str {
    match exit {
        DeepResearchSettlementExit::ReportReady => {
            "Task cancelled because the parent DeepResearch report completed."
        }
        DeepResearchSettlementExit::Interrupted => {
            "Task cancelled because the parent DeepResearch run was interrupted."
        }
    }
}

pub(super) fn deep_research_subagent_tracking_lost_output(
    exit: DeepResearchSettlementExit,
) -> &'static str {
    match exit {
        DeepResearchSettlementExit::ReportReady => {
            "Subagent tracking ended when the parent DeepResearch report completed."
        }
        DeepResearchSettlementExit::Interrupted => {
            "Subagent tracking ended when the parent DeepResearch run was interrupted."
        }
    }
}

pub(super) fn deep_research_subagent_settlement_from_snapshot(
    snapshot: a3s_code_core::SubagentTaskSnapshot,
    exit: DeepResearchSettlementExit,
) -> DeepResearchSubagentSettlement {
    let outcome = match snapshot.status {
        a3s_code_core::SubagentStatus::Completed => SubagentOutcome::Succeeded,
        a3s_code_core::SubagentStatus::Cancelled => SubagentOutcome::Cancelled,
        a3s_code_core::SubagentStatus::Failed => SubagentOutcome::Failed,
        a3s_code_core::SubagentStatus::Running => SubagentOutcome::TrackingLost,
        _ => SubagentOutcome::TrackingLost,
    };
    let output = snapshot.output.unwrap_or_else(|| match outcome {
        SubagentOutcome::Cancelled => deep_research_subagent_cancelled_output(exit).to_string(),
        SubagentOutcome::Failed => "Task failed.".to_string(),
        SubagentOutcome::TrackingLost => {
            "Subagent tracking ended before a terminal event was observed.".to_string()
        }
        SubagentOutcome::Succeeded => String::new(),
    });
    DeepResearchSubagentSettlement {
        task_id: snapshot.task_id,
        agent: snapshot.agent,
        output,
        outcome,
        finished_ms: snapshot.finished_ms.unwrap_or(snapshot.updated_ms),
    }
}

/// Settle every child owned by a terminal DeepResearch run before exposing the
/// parent as complete. A restored `Running` snapshot can have no live canceller;
/// record a synthetic terminal event in that case so a later tracker reload
/// cannot resurrect the footer, while retaining the more accurate
/// `TrackingLost` outcome in the TUI projection.
pub(super) fn settle_deep_research_subagents(
    session: Arc<AgentSession>,
    session_id: String,
    generation: u64,
    task_ids: Vec<String>,
    exit: DeepResearchSettlementExit,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let mut settlements = Vec::with_capacity(task_ids.len());
        for task_id in task_ids {
            let Some(snapshot) = session.subagent_task(&task_id).await else {
                settlements.push(DeepResearchSubagentSettlement {
                    task_id,
                    agent: "deep-research".to_string(),
                    output: "Subagent tracking ended before a terminal event was observed."
                        .to_string(),
                    outcome: SubagentOutcome::TrackingLost,
                    finished_ms: SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .map(|duration| duration.as_millis() as u64)
                        .unwrap_or(0),
                });
                continue;
            };
            if !snapshot.parent_session_id.is_empty() && snapshot.parent_session_id != session_id {
                settlements.push(DeepResearchSubagentSettlement {
                    task_id,
                    agent: snapshot.agent,
                    output: "Subagent tracking moved to a different parent session.".to_string(),
                    outcome: SubagentOutcome::TrackingLost,
                    finished_ms: snapshot.updated_ms,
                });
                continue;
            }
            if snapshot.status != a3s_code_core::SubagentStatus::Running {
                settlements.push(deep_research_subagent_settlement_from_snapshot(
                    snapshot, exit,
                ));
                continue;
            }

            let cancellation_started = session.cancel_subagent_task(&task_id).await;
            if let Some(after_cancel) = session.subagent_task(&task_id).await {
                if after_cancel.status != a3s_code_core::SubagentStatus::Running {
                    settlements.push(deep_research_subagent_settlement_from_snapshot(
                        after_cancel,
                        exit,
                    ));
                    continue;
                }
            } else if cancellation_started {
                settlements.push(DeepResearchSubagentSettlement {
                    task_id,
                    agent: snapshot.agent,
                    output: deep_research_subagent_cancelled_output(exit).to_string(),
                    outcome: SubagentOutcome::Cancelled,
                    finished_ms: snapshot.updated_ms,
                });
                continue;
            }

            let finished_ms = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|duration| duration.as_millis() as u64)
                .unwrap_or(snapshot.updated_ms);
            let output = deep_research_subagent_tracking_lost_output(exit).to_string();
            session
                .subagent_tracker()
                .record_event(&AgentEvent::SubagentEnd {
                    task_id: task_id.clone(),
                    session_id: snapshot.child_session_id,
                    agent: snapshot.agent.clone(),
                    output: output.clone(),
                    success: false,
                    finished_ms,
                })
                .await;
            settlements.push(DeepResearchSubagentSettlement {
                task_id,
                agent: snapshot.agent,
                output,
                outcome: SubagentOutcome::TrackingLost,
                finished_ms,
            });
        }
        Msg::DeepResearchSubagentsSettled {
            session_id,
            generation,
            exit,
            settlements,
        }
    })
}

/// Drives the welcome-mascot animation while the banner is on screen.
pub(super) fn banner_tick() -> Cmd<Msg> {
    cmd::tick(Duration::from_millis(280), Msg::BannerTick)
}

pub(super) fn ultracode_tick(epoch: u64) -> Cmd<Msg> {
    cmd::tick(ULTRACODE_ANIMATION_TICK, Msg::UltracodeTick { epoch })
}

pub(super) fn advance_ultracode_animation_epoch(epoch: &mut u64) -> u64 {
    *epoch = epoch.wrapping_add(1);
    *epoch
}

pub(super) fn ultracode_tick_is_current(current_epoch: u64, message_epoch: u64) -> bool {
    current_epoch == message_epoch
}

pub(super) fn ultracode_rebuild_starts_border(
    selected_effort: Option<usize>,
    succeeded: bool,
) -> bool {
    succeeded && selected_effort == Some(ULTRACODE)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum UltracodeTickAction {
    ContinueConfirm,
    BeginRebuild,
    ContinueBorder,
    ClearBorder,
    Idle,
}

pub(super) fn ultracode_tick_action(
    confirm_elapsed: Option<Duration>,
    border_elapsed: Option<Duration>,
) -> UltracodeTickAction {
    if let Some(elapsed) = confirm_elapsed {
        return if elapsed < ULTRACODE_CONFIRM_ANIMATION {
            UltracodeTickAction::ContinueConfirm
        } else {
            UltracodeTickAction::BeginRebuild
        };
    }

    if let Some(elapsed) = border_elapsed {
        return if elapsed < ULTRACODE_BORDER_ANIMATION {
            UltracodeTickAction::ContinueBorder
        } else {
            UltracodeTickAction::ClearBorder
        };
    }

    UltracodeTickAction::Idle
}
