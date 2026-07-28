use std::collections::HashMap;
use std::sync::Arc;

use super::model::{
    safe_display_label, safe_optional_summary, safe_workspace_alias, sanitize_remote_text,
    RemoteAttention, RemoteCapability, RemoteEvidenceConfidence, RemoteProgress, RemoteReadQuery,
    RemoteReadReceipt, RemoteReadResult, RemoteReadScope, RemoteSnapshot, RemoteTarget,
    RemoteTargetId, RemoteTargetKind, RemoteTargetState, SafeReplyExcerpt,
};
use crate::api::code_web::kernel::{
    ManagedChildEvidence, ManagedChildStatus, ManagedGoalStatus, ManagedSessionEvidence,
    ManagedSessionReadPort,
};
use crate::system_agents::{
    AgentActivityConfidence, AgentActivityState, SystemAgentActivity, SystemAgentSnapshot,
};

const MAX_REMOTE_TARGETS: usize = 128;
const MAX_SAFE_REPLY_CHARS: usize = 800;

pub(in crate::api::code_web) struct RemoteAgentReadService {
    managed_source: ManagedSource,
    system_source: SystemSource,
}

enum ManagedSource {
    Live(Arc<ManagedSessionReadPort>),
    #[cfg(test)]
    Static {
        sessions: Vec<ManagedSessionEvidence>,
        replies: HashMap<String, String>,
    },
}

enum SystemSource {
    Native,
    #[cfg(test)]
    Static(SystemAgentSnapshot),
}

impl RemoteAgentReadService {
    pub(in crate::api::code_web) fn new(managed: Arc<ManagedSessionReadPort>) -> Self {
        Self {
            managed_source: ManagedSource::Live(managed),
            system_source: SystemSource::Native,
        }
    }

    #[cfg(test)]
    pub(in crate::api::code_web) fn for_test(
        sessions: Vec<ManagedSessionEvidence>,
        replies: HashMap<String, String>,
        system: SystemAgentSnapshot,
    ) -> Self {
        Self {
            managed_source: ManagedSource::Static { sessions, replies },
            system_source: SystemSource::Static(system),
        }
    }

    pub(in crate::api::code_web) async fn snapshot(
        &self,
        scope: RemoteReadScope,
    ) -> RemoteSnapshot {
        let generated_at_ms = crate::system_agents::epoch_ms();
        let (managed, system) = tokio::join!(
            self.managed_evidence(scope.sessions),
            self.system_evidence(scope.agents)
        );
        let mut warnings = Vec::new();
        let mut items = Vec::new();
        if scope.sessions {
            for session in &managed {
                items.push(managed_target(session, generated_at_ms));
                items.extend(managed_child_targets(session, generated_at_ms));
            }
        }
        if let Some(system) = system {
            if !system.warnings.is_empty() {
                warnings.push("system_agent_evidence_degraded".to_string());
            }
            items.extend(system_targets(&system.activities, generated_at_ms));
        }
        sort_targets(&mut items);
        if items.len() > MAX_REMOTE_TARGETS {
            items.truncate(MAX_REMOTE_TARGETS);
            warnings.push("remote_target_limit_reached".to_string());
        }
        RemoteSnapshot::new(generated_at_ms, items, warnings)
    }

    pub(in crate::api::code_web) async fn query(
        &self,
        query: RemoteReadQuery,
        scope: RemoteReadScope,
    ) -> RemoteReadReceipt {
        let result = match &query {
            RemoteReadQuery::ListTargets => RemoteReadResult::Snapshot(self.snapshot(scope).await),
            RemoteReadQuery::ListSessions => {
                let session_scope = RemoteReadScope {
                    agents: false,
                    sessions: scope.sessions,
                    session_content: scope.session_content,
                };
                RemoteReadResult::Snapshot(self.snapshot(session_scope).await)
            }
            RemoteReadQuery::Inspect(target_id) => {
                let snapshot = self.snapshot(scope).await;
                RemoteReadResult::Target(
                    snapshot
                        .items
                        .into_iter()
                        .find(|target| &target.id == target_id),
                )
            }
            RemoteReadQuery::LatestReply(target_id) => {
                let reply = if scope.sessions && scope.session_content {
                    self.latest_reply(target_id).await
                } else {
                    None
                };
                RemoteReadResult::LatestReply(reply)
            }
        };
        RemoteReadReceipt {
            query,
            generated_at_ms: crate::system_agents::epoch_ms(),
            result,
        }
    }

    async fn managed_evidence(&self, enabled: bool) -> Vec<ManagedSessionEvidence> {
        if !enabled {
            return Vec::new();
        }
        match &self.managed_source {
            ManagedSource::Live(source) => source.snapshot().await,
            #[cfg(test)]
            ManagedSource::Static { sessions, .. } => sessions.clone(),
        }
    }

    async fn system_evidence(&self, enabled: bool) -> Option<SystemAgentSnapshot> {
        if !enabled {
            return None;
        }
        match &self.system_source {
            SystemSource::Native => {
                Some(crate::system_agents::collect_system_agent_snapshot().await)
            }
            #[cfg(test)]
            SystemSource::Static(snapshot) => Some(snapshot.clone()),
        }
    }

    async fn latest_reply(&self, target_id: &RemoteTargetId) -> Option<SafeReplyExcerpt> {
        let sessions = self.managed_evidence(true).await;
        let source_id = sessions.iter().find_map(|session| {
            (RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, &session.source_id)
                == *target_id)
                .then_some(session.source_id.as_str())
        })?;
        let raw = match &self.managed_source {
            ManagedSource::Live(source) => source.latest_assistant_reply(source_id).await,
            #[cfg(test)]
            ManagedSource::Static { replies, .. } => replies.get(source_id).cloned(),
        }?;
        let truncated = raw.chars().count() > MAX_SAFE_REPLY_CHARS;
        Some(SafeReplyExcerpt {
            target_id: target_id.clone(),
            text: sanitize_remote_text(&raw, MAX_SAFE_REPLY_CHARS),
            truncated,
        })
    }
}

fn managed_target(session: &ManagedSessionEvidence, generated_at_ms: u64) -> RemoteTarget {
    let (state, attention) = managed_state(session);
    let progress = session.goal.as_ref().map(|goal| RemoteProgress {
        goal_summary: safe_optional_summary(goal.summary.as_deref(), 160),
        percent: Some(goal.progress_percent),
        completed_steps: goal.completed_steps,
        total_steps: goal.total_steps,
        pending_turns: session.queue.pending_turns,
        active_turn: session.queue.active,
    });
    RemoteTarget {
        id: RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, &session.source_id),
        kind: RemoteTargetKind::ManagedSession,
        display_name: safe_display_label(session.title.as_deref(), "A3S session", 80),
        workspace_alias: safe_workspace_alias(Some(&session.workspace)),
        state,
        state_detail: managed_state_detail(state).to_string(),
        confidence: RemoteEvidenceConfidence::Authoritative,
        attention,
        evidence_at_ms: generated_at_ms,
        parent_id: None,
        capabilities: vec![RemoteCapability::ReadStatus, RemoteCapability::ReadChildren],
        progress: Some(progress.unwrap_or(RemoteProgress {
            goal_summary: None,
            percent: None,
            completed_steps: 0,
            total_steps: 0,
            pending_turns: session.queue.pending_turns,
            active_turn: session.queue.active,
        })),
    }
}

fn managed_child_targets(
    session: &ManagedSessionEvidence,
    generated_at_ms: u64,
) -> Vec<RemoteTarget> {
    let parent_id =
        RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, &session.source_id);
    let workspace_alias = safe_workspace_alias(Some(&session.workspace));
    session
        .children
        .iter()
        .map(|child| {
            managed_child_target(child, &parent_id, workspace_alias.clone(), generated_at_ms)
        })
        .collect()
}

fn managed_child_target(
    child: &ManagedChildEvidence,
    parent_id: &RemoteTargetId,
    workspace_alias: Option<String>,
    generated_at_ms: u64,
) -> RemoteTarget {
    let (state, attention, state_detail) = match child.status {
        ManagedChildStatus::Running => (
            RemoteTargetState::Working,
            RemoteAttention::None,
            "The managed child agent reports active work.",
        ),
        ManagedChildStatus::Completed => (
            RemoteTargetState::Completed,
            RemoteAttention::None,
            "The managed child agent reports completion.",
        ),
        ManagedChildStatus::Failed => (
            RemoteTargetState::Failed,
            RemoteAttention::Error,
            "The managed child agent reports failure.",
        ),
        ManagedChildStatus::Cancelled => (
            RemoteTargetState::Cancelled,
            RemoteAttention::None,
            "The managed child agent reports cancellation.",
        ),
        ManagedChildStatus::Unknown => (
            RemoteTargetState::Unknown,
            RemoteAttention::None,
            "The managed child agent state is unknown.",
        ),
    };
    let source_evidence_at_ms = child.updated_at_ms.max(child.started_at_ms);
    RemoteTarget {
        id: RemoteTargetId::for_source(RemoteTargetKind::CooperativeAgent, &child.source_id),
        kind: RemoteTargetKind::CooperativeAgent,
        display_name: safe_display_label(Some(&child.agent), "A3S child agent", 64),
        workspace_alias,
        state,
        state_detail: state_detail.to_string(),
        confidence: RemoteEvidenceConfidence::Authoritative,
        attention,
        evidence_at_ms: if source_evidence_at_ms == 0 {
            generated_at_ms
        } else {
            source_evidence_at_ms.min(generated_at_ms)
        },
        parent_id: Some(parent_id.clone()),
        capabilities: vec![RemoteCapability::ReadStatus],
        progress: Some(RemoteProgress {
            goal_summary: safe_optional_summary(Some(&child.description), 160),
            percent: None,
            completed_steps: 0,
            total_steps: 0,
            pending_turns: 0,
            active_turn: child.status == ManagedChildStatus::Running,
        }),
    }
}

fn managed_state(session: &ManagedSessionEvidence) -> (RemoteTargetState, RemoteAttention) {
    if session.queue.active {
        return (RemoteTargetState::Working, RemoteAttention::None);
    }
    if let Some(goal) = &session.goal {
        if goal.has_error {
            return (RemoteTargetState::Failed, RemoteAttention::Error);
        }
        match goal.status {
            ManagedGoalStatus::Active | ManagedGoalStatus::Retrying => {
                return (RemoteTargetState::Working, RemoteAttention::None)
            }
            ManagedGoalStatus::Paused => {
                return (RemoteTargetState::Paused, RemoteAttention::ActionRequired)
            }
            ManagedGoalStatus::Achieved => {
                return (RemoteTargetState::Completed, RemoteAttention::None)
            }
        }
    }
    if session.queue.paused && session.queue.pending_turns > 0 {
        (RemoteTargetState::Paused, RemoteAttention::ActionRequired)
    } else if session.queue.pending_turns > 0 {
        (RemoteTargetState::Queued, RemoteAttention::None)
    } else {
        (RemoteTargetState::Idle, RemoteAttention::None)
    }
}

fn managed_state_detail(state: RemoteTargetState) -> &'static str {
    match state {
        RemoteTargetState::Working => "A3S reports active managed work.",
        RemoteTargetState::Paused => "Managed work is paused and may need local attention.",
        RemoteTargetState::Queued => "Managed turns are waiting in the A3S queue.",
        RemoteTargetState::Completed => "The managed goal is complete.",
        RemoteTargetState::Failed => "The managed goal reported an error.",
        _ => "The managed session is idle.",
    }
}

fn system_targets(activities: &[SystemAgentActivity], generated_at_ms: u64) -> Vec<RemoteTarget> {
    let kinds = activities
        .iter()
        .map(|activity| {
            let kind = match activity.confidence {
                AgentActivityConfidence::Exact => RemoteTargetKind::CooperativeAgent,
                AgentActivityConfidence::Process => RemoteTargetKind::ObservedProcess,
            };
            (activity.id.as_str(), kind)
        })
        .collect::<HashMap<_, _>>();

    activities
        .iter()
        .map(|activity| match activity.confidence {
            AgentActivityConfidence::Process => RemoteTarget::observed(
                &activity.id,
                safe_display_label(Some(&activity.agent), "agent process", 64),
                safe_workspace_alias(activity.workspace.as_deref()),
                generated_at_ms,
            ),
            AgentActivityConfidence::Exact => {
                let state = system_state(activity.state);
                RemoteTarget {
                    id: RemoteTargetId::for_source(
                        RemoteTargetKind::CooperativeAgent,
                        &activity.id,
                    ),
                    kind: RemoteTargetKind::CooperativeAgent,
                    display_name: safe_display_label(Some(&activity.agent), "A3S agent", 64),
                    workspace_alias: safe_workspace_alias(activity.workspace.as_deref()),
                    state,
                    state_detail: system_state_detail(state).to_string(),
                    confidence: RemoteEvidenceConfidence::Exact,
                    attention: system_attention(activity.state),
                    evidence_at_ms: generated_at_ms.min(activity.expires_at_ms),
                    parent_id: activity.parent_id.as_deref().and_then(|parent_id| {
                        kinds
                            .get(parent_id)
                            .map(|kind| RemoteTargetId::for_source(*kind, parent_id))
                    }),
                    capabilities: vec![
                        RemoteCapability::ReadStatus,
                        RemoteCapability::ReadChildren,
                    ],
                    progress: safe_optional_summary(activity.task.as_deref(), 160).map(|summary| {
                        RemoteProgress {
                            goal_summary: Some(summary),
                            percent: None,
                            completed_steps: 0,
                            total_steps: 0,
                            pending_turns: 0,
                            active_turn: matches!(
                                activity.state,
                                AgentActivityState::Planning | AgentActivityState::Working
                            ),
                        }
                    }),
                }
            }
        })
        .collect()
}

fn system_state(state: AgentActivityState) -> RemoteTargetState {
    match state {
        AgentActivityState::Planning => RemoteTargetState::Planning,
        AgentActivityState::Working => RemoteTargetState::Working,
        AgentActivityState::WaitingApproval => RemoteTargetState::WaitingApproval,
        AgentActivityState::WaitingInput => RemoteTargetState::WaitingInput,
        AgentActivityState::Idle => RemoteTargetState::Idle,
        AgentActivityState::Completed => RemoteTargetState::Completed,
        AgentActivityState::Failed => RemoteTargetState::Failed,
        AgentActivityState::Cancelled => RemoteTargetState::Cancelled,
        AgentActivityState::Unknown => RemoteTargetState::Unknown,
    }
}

fn system_attention(state: AgentActivityState) -> RemoteAttention {
    match state {
        AgentActivityState::WaitingApproval | AgentActivityState::WaitingInput => {
            RemoteAttention::ActionRequired
        }
        AgentActivityState::Failed => RemoteAttention::Error,
        _ => RemoteAttention::None,
    }
}

fn system_state_detail(state: RemoteTargetState) -> &'static str {
    match state {
        RemoteTargetState::Planning => "The cooperative agent reports planning.",
        RemoteTargetState::Working => "The cooperative agent reports active work.",
        RemoteTargetState::WaitingApproval => "The cooperative agent is waiting for approval.",
        RemoteTargetState::WaitingInput => "The cooperative agent is waiting for input.",
        RemoteTargetState::Completed => "The cooperative agent reports completion.",
        RemoteTargetState::Failed => "The cooperative agent reports failure.",
        RemoteTargetState::Cancelled => "The cooperative agent reports cancellation.",
        _ => "The cooperative agent is not reporting active work.",
    }
}

fn sort_targets(targets: &mut [RemoteTarget]) {
    targets.sort_by(|left, right| {
        attention_rank(left.attention)
            .cmp(&attention_rank(right.attention))
            .then_with(|| state_rank(left.state).cmp(&state_rank(right.state)))
            .then_with(|| kind_rank(left.kind).cmp(&kind_rank(right.kind)))
            .then_with(|| left.display_name.cmp(&right.display_name))
            .then_with(|| left.id.cmp(&right.id))
    });
}

fn attention_rank(attention: RemoteAttention) -> u8 {
    match attention {
        RemoteAttention::Error => 0,
        RemoteAttention::ActionRequired => 1,
        RemoteAttention::None => 2,
    }
}

fn state_rank(state: RemoteTargetState) -> u8 {
    match state {
        RemoteTargetState::Failed => 0,
        RemoteTargetState::WaitingApproval | RemoteTargetState::WaitingInput => 1,
        RemoteTargetState::Planning | RemoteTargetState::Working => 2,
        RemoteTargetState::Queued | RemoteTargetState::Paused => 3,
        RemoteTargetState::Detected | RemoteTargetState::Unknown => 4,
        RemoteTargetState::Idle => 5,
        RemoteTargetState::Cancelled | RemoteTargetState::Completed => 6,
    }
}

fn kind_rank(kind: RemoteTargetKind) -> u8 {
    match kind {
        RemoteTargetKind::ManagedSession => 0,
        RemoteTargetKind::CooperativeAgent => 1,
        RemoteTargetKind::ObservedProcess => 2,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::code_web::kernel::{
        ManagedChildEvidence, ManagedChildStatus, ManagedGoalEvidence, ManagedQueueEvidence,
    };
    use crate::system_agents::{AgentVendor, SystemAgentActivity};

    fn managed() -> ManagedSessionEvidence {
        ManagedSessionEvidence {
            source_id: "session-secret-id".to_string(),
            title: Some("Fix /Users/alice/private with sk-live-canary".to_string()),
            workspace: "/Users/alice/project".to_string(),
            created_at_ms: 1,
            updated_at_ms: 2,
            goal: Some(ManagedGoalEvidence {
                summary: Some("Run token=canary tests".to_string()),
                status: ManagedGoalStatus::Active,
                progress_percent: 40,
                completed_steps: 2,
                total_steps: 5,
                updated_at_ms: 2,
                has_error: false,
            }),
            queue: ManagedQueueEvidence {
                pending_turns: 1,
                active: true,
                paused: false,
            },
            children: vec![ManagedChildEvidence {
                source_id: "managed-child-secret".to_string(),
                agent: "explore".to_string(),
                description: "Inspect /Users/alice/child with token=canary".to_string(),
                status: ManagedChildStatus::Running,
                started_at_ms: 1,
                updated_at_ms: 2,
            }],
        }
    }

    fn system_snapshot() -> SystemAgentSnapshot {
        SystemAgentSnapshot {
            activities: vec![
                SystemAgentActivity {
                    id: "presence-exact-secret".to_string(),
                    parent_id: None,
                    agent: "a3s-code".to_string(),
                    workspace: Some("web".to_string()),
                    task: Some("Review panel".to_string()),
                    reason: None,
                    state: AgentActivityState::WaitingInput,
                    confidence: AgentActivityConfidence::Exact,
                    vendor: AgentVendor::A3s,
                    started_at_ms: Some(1),
                    finished_at_ms: None,
                    expires_at_ms: u64::MAX,
                    actions: Vec::new(),
                    local: false,
                },
                SystemAgentActivity {
                    id: "process:4242".to_string(),
                    parent_id: None,
                    agent: "codex".to_string(),
                    workspace: Some("repo".to_string()),
                    task: Some("active process".to_string()),
                    reason: None,
                    state: AgentActivityState::Unknown,
                    confidence: AgentActivityConfidence::Process,
                    vendor: AgentVendor::OpenAi,
                    started_at_ms: Some(1),
                    finished_at_ms: None,
                    expires_at_ms: u64::MAX,
                    actions: Vec::new(),
                    local: false,
                },
            ],
            warnings: Vec::new(),
        }
    }

    #[tokio::test]
    async fn remote_snapshot_truthfully_separates_and_redacts_all_target_kinds() {
        let service =
            RemoteAgentReadService::for_test(vec![managed()], HashMap::new(), system_snapshot());
        let snapshot = service.snapshot(RemoteReadScope::default()).await;
        assert_eq!(snapshot.totals.managed, 1);
        assert_eq!(snapshot.totals.cooperative, 2);
        assert_eq!(snapshot.totals.observed, 1);
        let serialized = serde_json::to_string(&snapshot).unwrap();
        for forbidden in [
            "session-secret-id",
            "presence-exact-secret",
            "managed-child-secret",
            "4242",
            "/Users/alice",
            "sk-live-canary",
            "token=canary",
        ] {
            assert!(!serialized.contains(forbidden), "leaked {forbidden}");
        }
        let observed = snapshot
            .items
            .iter()
            .find(|target| target.kind == RemoteTargetKind::ObservedProcess)
            .unwrap();
        assert_eq!(observed.capabilities, vec![RemoteCapability::ReadStatus]);
        assert_eq!(observed.state, RemoteTargetState::Detected);
        let managed_parent = snapshot
            .items
            .iter()
            .find(|target| target.kind == RemoteTargetKind::ManagedSession)
            .unwrap();
        let managed_child = snapshot
            .items
            .iter()
            .find(|target| {
                target.kind == RemoteTargetKind::CooperativeAgent
                    && target.confidence == RemoteEvidenceConfidence::Authoritative
            })
            .unwrap();
        assert_eq!(managed_child.parent_id.as_ref(), Some(&managed_parent.id));
        assert_eq!(
            managed_child.capabilities,
            vec![RemoteCapability::ReadStatus]
        );
        assert_eq!(managed_child.state, RemoteTargetState::Working);
    }

    #[tokio::test]
    async fn latest_reply_requires_content_scope_and_is_bounded_and_redacted() {
        let mut replies = HashMap::new();
        replies.insert(
            "session-secret-id".to_string(),
            format!(
                "Done /Users/alice/private with sk-live-canary {}",
                "x".repeat(900)
            ),
        );
        let service = RemoteAgentReadService::for_test(vec![managed()], replies, system_snapshot());
        let target_id =
            RemoteTargetId::for_source(RemoteTargetKind::ManagedSession, "session-secret-id");
        let disabled = service
            .query(
                RemoteReadQuery::LatestReply(target_id.clone()),
                RemoteReadScope::default(),
            )
            .await;
        assert_eq!(disabled.result, RemoteReadResult::LatestReply(None));

        let enabled = service
            .query(
                RemoteReadQuery::LatestReply(target_id),
                RemoteReadScope {
                    session_content: true,
                    ..RemoteReadScope::default()
                },
            )
            .await;
        let RemoteReadResult::LatestReply(Some(reply)) = enabled.result else {
            panic!("expected safe reply")
        };
        assert!(reply.truncated);
        assert!(!reply.text.contains("/Users/alice"));
        assert!(!reply.text.contains("sk-live-canary"));
        assert!(reply.text.chars().count() <= MAX_SAFE_REPLY_CHARS);
    }
}
