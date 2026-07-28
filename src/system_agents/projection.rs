//! Projection of exact heartbeats and inferred processes into island rows.

use std::collections::{HashMap, HashSet};

use super::{
    workspace_basename, AgentActivityConfidence, AgentActivityState, AgentPresence, AgentVendor,
    SystemAgentActivity, SystemAgentSnapshot, PRESENCE_TTL_MS,
};
use crate::top::ProcessRow;

pub(super) fn aggregate_activities(
    presences: &[AgentPresence],
    processes: &[ProcessRow],
    local_instance_id: &str,
    now_ms: u64,
) -> Vec<SystemAgentActivity> {
    let mut activities = Vec::new();
    let mut exact_pids = HashSet::new();

    for presence in presences
        .iter()
        .filter(|presence| presence.valid_at(now_ms))
    {
        exact_pids.insert(presence.pid);
        let local = presence.instance_id == local_instance_id;
        activities.extend(activities_for_presence(presence, local));
    }

    for process in root_agent_processes(processes) {
        if exact_pids.contains(&process.pid) {
            continue;
        }
        let Some(agent) = process.agent else {
            continue;
        };
        activities.push(SystemAgentActivity {
            id: format!("process:{}", process.pid),
            parent_id: None,
            agent: agent.label().to_string(),
            workspace: process
                .cwd
                .as_deref()
                .map(workspace_basename)
                .filter(|workspace| !workspace.is_empty()),
            // Never expose command arguments: non-interactive agents often
            // carry the full user prompt on their command line.
            task: Some("active process".to_string()),
            reason: None,
            state: AgentActivityState::Unknown,
            confidence: AgentActivityConfidence::Process,
            vendor: AgentVendor::from_hint(agent.label()).unwrap_or_default(),
            started_at_ms: process_started_at_ms(&process.elapsed, now_ms),
            finished_at_ms: None,
            expires_at_ms: now_ms.saturating_add(PRESENCE_TTL_MS),
            actions: Vec::new(),
            local: false,
        });
    }

    sort_activities(&mut activities);
    activities
}

pub(crate) fn activities_for_presence(
    presence: &AgentPresence,
    local: bool,
) -> Vec<SystemAgentActivity> {
    let mut activities = vec![SystemAgentActivity {
        id: presence.instance_id.clone(),
        parent_id: None,
        agent: "a3s-code".to_string(),
        workspace: nonempty(presence.workspace.clone()),
        task: presence.task.clone(),
        reason: presence.reason.clone(),
        state: presence.state,
        confidence: AgentActivityConfidence::Exact,
        vendor: presence.vendor,
        started_at_ms: Some(presence.started_at_ms),
        finished_at_ms: presence.finished_at_ms,
        expires_at_ms: presence.updated_at_ms.saturating_add(PRESENCE_TTL_MS),
        actions: presence.actions.clone(),
        local,
    }];
    activities.extend(presence.children.iter().map(|child| SystemAgentActivity {
        id: format!("{}:{}", presence.instance_id, child.id),
        parent_id: Some(presence.instance_id.clone()),
        agent: child.agent.clone(),
        workspace: nonempty(presence.workspace.clone()),
        task: child.task.clone(),
        reason: None,
        state: child.state,
        confidence: AgentActivityConfidence::Exact,
        vendor: child.vendor,
        started_at_ms: child.started_at_ms,
        finished_at_ms: child.finished_at_ms,
        expires_at_ms: presence.updated_at_ms.saturating_add(PRESENCE_TTL_MS),
        actions: child.actions.clone(),
        local,
    }));
    activities
}

pub(crate) fn sort_activities(activities: &mut [SystemAgentActivity]) {
    activities.sort_by(|left, right| {
        left.state
            .attention_rank()
            .cmp(&right.state.attention_rank())
            .then_with(|| {
                left.confidence
                    .evidence_rank()
                    .cmp(&right.confidence.evidence_rank())
            })
            .then_with(|| right.local.cmp(&left.local))
            .then_with(|| left.parent_id.is_some().cmp(&right.parent_id.is_some()))
            .then_with(|| left.agent.cmp(&right.agent))
            .then_with(|| left.id.cmp(&right.id))
    });
}

pub(super) fn snapshot_requests_island_launch(snapshot: &SystemAgentSnapshot) -> bool {
    snapshot.activities.iter().any(|activity| {
        activity.confidence == AgentActivityConfidence::Process
            || (activity.confidence == AgentActivityConfidence::Exact
                && activity.state.keeps_island_visible())
    })
}

pub(super) fn root_agent_processes(processes: &[ProcessRow]) -> Vec<&ProcessRow> {
    let by_pid = processes
        .iter()
        .map(|process| (process.pid, process))
        .collect::<HashMap<_, _>>();
    processes
        .iter()
        .filter(|process| process.agent.is_some())
        .filter(|process| {
            let agent = process.agent;
            let mut parent = process.ppid;
            let mut visited = HashSet::new();
            while parent != 0 && visited.insert(parent) {
                let Some(candidate) = by_pid.get(&parent) else {
                    break;
                };
                if candidate.agent == agent {
                    return false;
                }
                parent = candidate.ppid;
            }
            true
        })
        .collect()
}

fn nonempty(value: String) -> Option<String> {
    (!value.trim().is_empty()).then_some(value)
}

fn process_started_at_ms(elapsed: &str, now_ms: u64) -> Option<u64> {
    let elapsed_ms = parse_process_elapsed_seconds(elapsed)?.checked_mul(1_000)?;
    now_ms.checked_sub(elapsed_ms)
}

fn parse_process_elapsed_seconds(elapsed: &str) -> Option<u64> {
    let elapsed = elapsed.trim();
    let (days, clock) = match elapsed.split_once('-') {
        Some((days, clock)) => (days.parse::<u64>().ok()?, clock),
        None => (0, elapsed),
    };
    let clock = clock
        .split(':')
        .map(str::parse::<u64>)
        .collect::<Result<Vec<_>, _>>()
        .ok()?;
    let (hours, minutes, seconds) = match clock.as_slice() {
        [minutes, seconds] => (0, *minutes, *seconds),
        [hours, minutes, seconds] => (*hours, *minutes, *seconds),
        _ => return None,
    };
    if minutes >= 60 || seconds >= 60 || (days > 0 && hours >= 24) {
        return None;
    }
    days.checked_mul(86_400)?
        .checked_add(hours.checked_mul(3_600)?)?
        .checked_add(minutes.checked_mul(60)?)?
        .checked_add(seconds)
}
