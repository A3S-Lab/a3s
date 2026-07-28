//! Agent Island control dispatch and live-context validation.

use super::*;

impl App {
    pub(in crate::tui) fn apply_agent_island_control(
        &mut self,
        request: AgentControlRequest,
    ) -> Option<Cmd<Msg>> {
        let parent_id = self.agent_presence.publisher.instance_id().to_string();
        match request.action {
            AgentControlActionKind::ApproveOnce
            | AgentControlActionKind::ApproveAlways
            | AgentControlActionKind::Deny
                if request.activity_id == parent_id
                    && request.message.is_none()
                    && self.state == State::Awaiting
                    && self.pending_tools.front().is_some_and(|pending| {
                        request.context == self.agent_island_approval_context(&pending.tool_id)
                    }) =>
            {
                match request.action {
                    AgentControlActionKind::ApproveOnce => self.apply_approval(0).map(cmd::msg),
                    AgentControlActionKind::ApproveAlways => self.apply_approval(1).map(cmd::msg),
                    AgentControlActionKind::Deny => self
                        .deny_current_approval("Denied from Agent Island.")
                        .map(cmd::msg),
                    _ => None,
                }
            }
            AgentControlActionKind::Stop
                if request.activity_id == parent_id
                    && request.message.is_none()
                    && request.context == self.agent_island_parent_context()
                    && self.agent_island_stop_available() =>
            {
                self.begin_stream_interrupt("interrupted from Agent Island")
            }
            AgentControlActionKind::Cancel => {
                if request.message.is_some() {
                    return None;
                }
                let task_id = request
                    .activity_id
                    .strip_prefix(&format!("{parent_id}:"))
                    .map(str::to_string)?;
                let expected_activity = format!("{parent_id}:{task_id}");
                let live = self
                    .runtime
                    .subagent_ids()
                    .into_iter()
                    .zip(self.runtime.subagents())
                    .any(|(id, run)| id == task_id && !run.done);
                if request.activity_id != expected_activity
                    || request.context != self.agent_island_child_context(&task_id)
                    || !live
                {
                    return None;
                }
                if !self.agent_presence.cancel_requested.insert(task_id.clone()) {
                    return None;
                }
                let session = self.session.clone();
                Some(cmd::cmd(move || async move {
                    let cancelled = session.cancel_subagent_task(&task_id).await;
                    Msg::AgentIslandSubagentCancelFinished { task_id, cancelled }
                }))
            }
            AgentControlActionKind::Reply if request.activity_id == parent_id => {
                let message = request.message?.trim().to_string();
                if message.is_empty() {
                    return None;
                }
                let context_matches = match self.state {
                    State::Streaming => {
                        !self.interrupting && request.context == self.agent_island_parent_context()
                    }
                    State::Awaiting => self.pending_tools.front().is_some_and(|pending| {
                        request.context == self.agent_island_approval_context(&pending.tool_id)
                    }),
                    State::Idle | State::Rebuilding => false,
                };
                context_matches.then(|| cmd::msg(Msg::Submit(message)))
            }
            _ => None,
        }
    }

    pub(in crate::tui) fn apply_agent_island_subagent_cancel_result(
        &mut self,
        task_id: String,
        cancelled: bool,
    ) {
        if !cancelled {
            self.agent_presence.cancel_requested.remove(&task_id);
            tracing::debug!(%task_id, "agent island child cancellation was already stale");
        }
    }

    pub(in crate::tui) fn begin_stream_interrupt(&mut self, reason: &str) -> Option<Cmd<Msg>> {
        self.begin_stream_interrupt_with_goal_policy(reason, true)
    }

    pub(in crate::tui) fn begin_send_now_interrupt(&mut self) -> Option<Cmd<Msg>> {
        self.begin_stream_interrupt_with_goal_policy("superseded by Send now", false)
    }

    fn begin_stream_interrupt_with_goal_policy(
        &mut self,
        reason: &str,
        cancel_goal: bool,
    ) -> Option<Cmd<Msg>> {
        if !self.agent_island_stop_available() {
            return None;
        }
        let goal_cancelled = cancel_goal && self.cancel_goal_state(reason);
        self.interrupting = true;
        self.agent_presence
            .publisher
            .reconcile_control_grants(Vec::new(), epoch_ms());
        if self.stream_join.is_none() && self.rx.is_none() && !self.host_progress_inflight {
            self.interrupted_stream_start_token = Some(self.stream_start_token);
        }
        self.stream_start_token = self.stream_start_token.wrapping_add(1);
        self.deep_research_stream_timeout_token =
            self.deep_research_stream_timeout_token.wrapping_add(1);
        let status_entry =
            self.push_tracked_line(&Style::new().fg(TN_YELLOW).render("  ⎋ interrupting…"));
        let session = self.session.clone();
        let join = self.stream_join.take();
        let host_abort = self.host_tool_abort.take();
        let deep_research = self.deep_research_loop.is_some();
        let deep_research_handle = self.deep_research_handle.take();
        self.deep_research_events = None;
        self.host_progress_inflight = false;
        Some(cmd::cmd(move || async move {
            if let Some(host_abort) = host_abort {
                host_abort.abort();
            }
            if let Some(handle) = deep_research_handle {
                let _ = handle.cancel_and_settle().await;
            } else if !deep_research {
                let _ = session
                    .cancel_and_settle(
                        Duration::from_millis(DEEP_RESEARCH_ABORT_GRACE_MS),
                        Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                    )
                    .await;
            }
            if let Some(join) = join {
                let _ = settle_stream_join_for_quit(
                    join,
                    Duration::from_millis(GRACEFUL_QUIT_ABORT_SETTLE_MS),
                )
                .await;
            }
            Msg::Interrupted {
                goal_cancelled,
                status_entry,
            }
        }))
    }

    pub(in crate::tui) fn agent_island_stop_available(&self) -> bool {
        self.state == State::Streaming
            && !self.interrupting
            && !self.stream_join_settling
            && !self.deep_research_subagent_settlement_inflight
    }

    pub(super) fn agent_island_parent_context(&self) -> String {
        format!("{}:{}", self.session_id, self.stream_start_token)
    }

    pub(super) fn agent_island_approval_context(&self, tool_id: &str) -> String {
        format!("{}:approval:{tool_id}", self.session_id)
    }

    pub(super) fn agent_island_child_context(&self, task_id: &str) -> String {
        format!("{}:{task_id}", self.session_id)
    }

    pub(super) fn local_agent_vendor(&self) -> AgentVendor {
        match self.model_source {
            ModelSelectionSource::Claude => AgentVendor::Anthropic,
            ModelSelectionSource::Codex => AgentVendor::OpenAi,
            ModelSelectionSource::Kimi => AgentVendor::Moonshot,
            ModelSelectionSource::CodeBuddy => AgentVendor::Tencent,
            ModelSelectionSource::OsGateway => AgentVendor::A3s,
            ModelSelectionSource::Config => self
                .model
                .as_deref()
                .and_then(AgentVendor::from_hint)
                .unwrap_or_default(),
        }
    }
}
