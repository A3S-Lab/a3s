use std::sync::Arc;

use super::super::turn_queue::CodeWebStoredTurnQueue;
use super::*;

const MAX_MANAGED_CHILDREN_PER_SESSION: usize = 64;

/// Narrow, read-only view exported by the Kernel module for remote adapters.
///
/// The port intentionally exposes neither `KernelService` nor `CodeWebState`,
/// and it has no mutation methods.
pub(in crate::api::code_web) struct ManagedSessionReadPort {
    service: Arc<KernelService>,
}

impl ManagedSessionReadPort {
    pub(in crate::api::code_web) fn new(service: Arc<KernelService>) -> Self {
        Self { service }
    }

    pub(in crate::api::code_web) async fn snapshot(&self) -> Vec<ManagedSessionEvidence> {
        let metadata = self.service.state.session_metadata.lock().await.clone();
        let controls = self.service.state.session_controls.lock().await.clone();
        let queues = self
            .service
            .state
            .session_turn_queues
            .lock()
            .await
            .iter()
            .map(|(session_id, queue)| (session_id.clone(), queue.snapshot()))
            .collect::<HashMap<_, _>>();
        let runtime_sessions = self.service.state.sessions.lock().await.clone();
        let mut children_by_session = HashMap::with_capacity(runtime_sessions.len());
        for (session_id, session) in runtime_sessions {
            let mut children = session
                .subagent_tasks()
                .await
                .into_iter()
                .map(|task| ManagedChildEvidence {
                    source_id: format!("{session_id}\0{}", task.task_id),
                    agent: task.agent,
                    description: task.description,
                    status: match task.status {
                        a3s_code_core::SubagentStatus::Running => ManagedChildStatus::Running,
                        a3s_code_core::SubagentStatus::Completed => ManagedChildStatus::Completed,
                        a3s_code_core::SubagentStatus::Failed => ManagedChildStatus::Failed,
                        a3s_code_core::SubagentStatus::Cancelled => ManagedChildStatus::Cancelled,
                        _ => ManagedChildStatus::Unknown,
                    },
                    started_at_ms: task.started_ms,
                    updated_at_ms: task.updated_ms,
                })
                .collect::<Vec<_>>();
            children.sort_by(|left, right| {
                managed_child_status_rank(left.status)
                    .cmp(&managed_child_status_rank(right.status))
                    .then_with(|| right.updated_at_ms.cmp(&left.updated_at_ms))
                    .then_with(|| left.source_id.cmp(&right.source_id))
            });
            children.truncate(MAX_MANAGED_CHILDREN_PER_SESSION);
            children_by_session.insert(session_id, children);
        }

        let mut sessions = metadata
            .into_iter()
            .map(|(source_id, metadata)| {
                let controls = controls.get(&source_id).cloned().unwrap_or_default();
                let queue = queues.get(&source_id).cloned().unwrap_or_default();
                ManagedSessionEvidence {
                    children: children_by_session.remove(&source_id).unwrap_or_default(),
                    source_id,
                    title: metadata.title,
                    workspace: metadata.workspace,
                    created_at_ms: nonnegative_timestamp(metadata.created_at),
                    updated_at_ms: nonnegative_timestamp(metadata.updated_at),
                    goal: controls.goal_run.map(|run| ManagedGoalEvidence {
                        summary: controls.goal,
                        status: match run.status {
                            CodeWebGoalStatus::Active => ManagedGoalStatus::Active,
                            CodeWebGoalStatus::Paused => ManagedGoalStatus::Paused,
                            CodeWebGoalStatus::Retrying => ManagedGoalStatus::Retrying,
                            CodeWebGoalStatus::Achieved => ManagedGoalStatus::Achieved,
                        },
                        progress_percent: run.progress_percent.min(100),
                        completed_steps: run.completed_steps,
                        total_steps: run.total_steps,
                        updated_at_ms: nonnegative_timestamp(run.updated_at),
                        has_error: run.last_error.is_some(),
                    }),
                    queue: ManagedQueueEvidence::from_snapshot(&queue),
                }
            })
            .collect::<Vec<_>>();
        sessions.sort_by(|left, right| {
            right
                .updated_at_ms
                .cmp(&left.updated_at_ms)
                .then_with(|| left.source_id.cmp(&right.source_id))
        });
        sessions
    }

    pub(in crate::api::code_web) async fn latest_assistant_reply(
        &self,
        source_id: &str,
    ) -> Option<String> {
        self.service
            .state
            .messages
            .lock()
            .await
            .get(source_id)
            .and_then(|messages| {
                messages.iter().rev().find_map(|message| {
                    (message.get("role").and_then(Value::as_str) == Some("assistant"))
                        .then(|| message.get("content").and_then(Value::as_str))
                        .flatten()
                        .map(ToOwned::to_owned)
                })
            })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) struct ManagedSessionEvidence {
    pub(in crate::api::code_web) source_id: String,
    pub(in crate::api::code_web) title: Option<String>,
    pub(in crate::api::code_web) workspace: String,
    pub(in crate::api::code_web) created_at_ms: u64,
    pub(in crate::api::code_web) updated_at_ms: u64,
    pub(in crate::api::code_web) goal: Option<ManagedGoalEvidence>,
    pub(in crate::api::code_web) queue: ManagedQueueEvidence,
    pub(in crate::api::code_web) children: Vec<ManagedChildEvidence>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) struct ManagedChildEvidence {
    pub(in crate::api::code_web) source_id: String,
    pub(in crate::api::code_web) agent: String,
    pub(in crate::api::code_web) description: String,
    pub(in crate::api::code_web) status: ManagedChildStatus,
    pub(in crate::api::code_web) started_at_ms: u64,
    pub(in crate::api::code_web) updated_at_ms: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) enum ManagedChildStatus {
    Running,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) struct ManagedGoalEvidence {
    pub(in crate::api::code_web) summary: Option<String>,
    pub(in crate::api::code_web) status: ManagedGoalStatus,
    pub(in crate::api::code_web) progress_percent: u8,
    pub(in crate::api::code_web) completed_steps: usize,
    pub(in crate::api::code_web) total_steps: usize,
    pub(in crate::api::code_web) updated_at_ms: u64,
    pub(in crate::api::code_web) has_error: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::api::code_web) enum ManagedGoalStatus {
    Active,
    Paused,
    Retrying,
    Achieved,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::api::code_web) struct ManagedQueueEvidence {
    pub(in crate::api::code_web) pending_turns: usize,
    pub(in crate::api::code_web) active: bool,
    pub(in crate::api::code_web) paused: bool,
}

impl ManagedQueueEvidence {
    fn from_snapshot(snapshot: &CodeWebStoredTurnQueue) -> Self {
        Self {
            pending_turns: snapshot.items.len(),
            active: snapshot.active.is_some(),
            paused: snapshot.paused,
        }
    }
}

fn nonnegative_timestamp(value: i64) -> u64 {
    u64::try_from(value).unwrap_or_default()
}

fn managed_child_status_rank(status: ManagedChildStatus) -> u8 {
    match status {
        ManagedChildStatus::Failed => 0,
        ManagedChildStatus::Running => 1,
        ManagedChildStatus::Unknown => 2,
        ManagedChildStatus::Cancelled => 3,
        ManagedChildStatus::Completed => 4,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn managed_read_port_aggregates_live_state_without_exposing_mutations() {
        let temporary = tempfile::tempdir().expect("create temporary Code Web state");
        let workspace = temporary.path().join("workspace");
        std::fs::create_dir_all(&workspace).expect("create workspace");
        let code_config = a3s_code_core::CodeConfig::from_acl(
            r#"
                default_model = "openai/test-model"
                providers "openai" {
                  apiKey = "sk-test"
                  baseUrl = "https://example.com/v1"
                  models "test-model" {}
                }
            "#,
        )
        .expect("parse test config");
        let agent = Arc::new(
            a3s_code_core::Agent::from_config(code_config.clone())
                .await
                .expect("create test agent"),
        );
        let repository = Arc::new(
            crate::api::code_web::session_store::CodeWebSessionRepository::open(
                temporary.path().join("state"),
            )
            .await
            .expect("open session repository"),
        );
        let state = Arc::new(CodeWebState::new(
            agent,
            temporary.path().join("config.acl"),
            workspace.clone(),
            code_config,
            repository,
        ));
        let managed_session = Arc::new(
            state
                .agent
                .session_async(workspace.display().to_string(), None)
                .await
                .expect("create managed session"),
        );
        let source_id = managed_session.session_id().to_string();
        managed_session
            .subagent_tracker()
            .record_event(&AgentEvent::SubagentStart {
                task_id: "child-task-secret".to_string(),
                session_id: "child-session-secret".to_string(),
                parent_session_id: source_id.clone(),
                agent: "explore".to_string(),
                description: "Inspect the remote target projection".to_string(),
                started_ms: 1_750,
            })
            .await;
        state
            .sessions
            .lock()
            .await
            .insert(source_id.clone(), managed_session);
        state.session_metadata.lock().await.insert(
            source_id.clone(),
            CodeWebSessionMetadata {
                workspace: workspace.display().to_string(),
                title: Some("Remote status work".to_string()),
                agent_id: Some("default".to_string()),
                created_at: 1_000,
                updated_at: 2_000,
            },
        );
        state.session_controls.lock().await.insert(
            source_id.clone(),
            CodeWebSessionControls {
                goal: Some("Finish the read-only target panel".to_string()),
                goal_run: Some(CodeWebGoalRun {
                    status: CodeWebGoalStatus::Retrying,
                    updated_at: 1_900,
                    progress_percent: 60,
                    completed_steps: 3,
                    total_steps: 5,
                    ..CodeWebGoalRun::default()
                }),
                ..CodeWebSessionControls::default()
            },
        );
        let mut queue = CodeWebSessionTurnQueue::default();
        queue.enqueue(queued_turn("turn-active", "active request", 1_500));
        queue
            .begin("turn-active", 1_600)
            .expect("begin active turn");
        queue.enqueue(queued_turn("turn-pending", "pending request", 1_700));
        state
            .session_turn_queues
            .lock()
            .await
            .insert(source_id.clone(), queue);
        state.messages.lock().await.insert(
            source_id.clone(),
            vec![
                serde_json::json!({ "role": "assistant", "content": "Earlier reply" }),
                serde_json::json!({ "role": "user", "content": "Continue" }),
                serde_json::json!({ "role": "assistant", "content": "Latest safe reply" }),
            ],
        );

        let service = Arc::new(KernelService::new(Arc::clone(&state)));
        let port = ManagedSessionReadPort::new(service);
        let sessions = port.snapshot().await;

        assert_eq!(sessions.len(), 1);
        let session = &sessions[0];
        assert_eq!(session.source_id, source_id);
        assert_eq!(session.title.as_deref(), Some("Remote status work"));
        assert_eq!(session.created_at_ms, 1_000);
        assert_eq!(session.updated_at_ms, 2_000);
        assert_eq!(session.queue.pending_turns, 1);
        assert!(session.queue.active);
        assert!(!session.queue.paused);
        let goal = session.goal.as_ref().expect("managed goal evidence");
        assert_eq!(goal.status, ManagedGoalStatus::Retrying);
        assert_eq!(goal.progress_percent, 60);
        assert_eq!(goal.completed_steps, 3);
        assert_eq!(goal.total_steps, 5);
        assert_eq!(
            goal.summary.as_deref(),
            Some("Finish the read-only target panel")
        );
        assert_eq!(session.children.len(), 1);
        assert_eq!(session.children[0].agent, "explore");
        assert_eq!(session.children[0].status, ManagedChildStatus::Running);
        assert_eq!(
            session.children[0].description,
            "Inspect the remote target projection"
        );
        assert_eq!(
            port.latest_assistant_reply(&source_id).await.as_deref(),
            Some("Latest safe reply")
        );

        state.close().await;
    }

    fn queued_turn(id: &str, content: &str, enqueued_at: i64) -> CodeWebQueuedTurn {
        CodeWebQueuedTurn {
            id: id.to_string(),
            kind: CodeWebQueuedTurnKind::User,
            content: content.to_string(),
            context_files: Vec::new(),
            skill_names: Vec::new(),
            mode: CodeWebQueuedTurnMode::Standard,
            research_run_id: None,
            priority: USER_TURN_PRIORITY,
            enqueued_at,
        }
    }
}
