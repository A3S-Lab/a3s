use super::*;

pub(super) fn parent_presence_state(
    state: State,
    plan_in_progress: bool,
    recent_terminal: Option<AgentActivityState>,
) -> AgentActivityState {
    match state {
        State::Awaiting => AgentActivityState::WaitingApproval,
        State::Rebuilding => AgentActivityState::Working,
        State::Streaming if plan_in_progress => AgentActivityState::Planning,
        State::Streaming => AgentActivityState::Working,
        State::Idle => recent_terminal.unwrap_or(AgentActivityState::Idle),
    }
}

pub(super) fn child_presence(
    id: String,
    child: &runtime_projection::SubagentRun,
    fallback_vendor: AgentVendor,
    now: Instant,
    now_ms: u64,
) -> Option<AgentChildPresence> {
    if child
        .ended
        .is_some_and(|ended| now.saturating_duration_since(ended) > TERMINAL_STATE_RETENTION)
    {
        return None;
    }

    let state = match child.outcome {
        Some(runtime_projection::SubagentOutcome::Succeeded) => AgentActivityState::Completed,
        Some(runtime_projection::SubagentOutcome::Failed) => AgentActivityState::Failed,
        Some(runtime_projection::SubagentOutcome::Cancelled) => AgentActivityState::Cancelled,
        Some(runtime_projection::SubagentOutcome::TrackingLost) => AgentActivityState::Unknown,
        None => AgentActivityState::Working,
    };
    Some(AgentChildPresence {
        id,
        agent: child.display_agent(),
        task: nonempty_presence_text(&child.description),
        state,
        vendor: AgentVendor::from_hint(&child.agent).unwrap_or(fallback_vendor),
        started_at_ms: Some(
            now_ms.saturating_sub(
                now.saturating_duration_since(child.started)
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64,
            ),
        ),
        finished_at_ms: child.ended.map(|ended| {
            now_ms.saturating_sub(
                now.saturating_duration_since(ended)
                    .as_millis()
                    .min(u128::from(u64::MAX)) as u64,
            )
        }),
        actions: Vec::new(),
    })
}

fn nonempty_presence_text(value: &str) -> Option<String> {
    let value = value.split_whitespace().collect::<Vec<_>>().join(" ");
    (!value.is_empty()).then_some(value)
}
