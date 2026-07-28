//! Runtime event projections for the TUI.
//!
//! This is a deliberately small ECS-style island inside the TEA app: runtime
//! events mutate tool/subagent entities by stable ids, while the main `App`
//! renders read-only projections of the current world.

use std::collections::BTreeMap;
use std::time::Instant;

/// Lifecycle state for one model-requested tool call.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ToolCallState {
    Preparing,
    AwaitingApproval,
    Running,
    Succeeded,
    Failed,
    Denied,
    TimedOut,
    Interrupted,
}

impl ToolCallState {
    pub(crate) fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Succeeded | Self::Failed | Self::Denied | Self::TimedOut | Self::Interrupted
        )
    }
}

/// A tool entity projected from preparation through its terminal state.
#[derive(Clone, Debug)]
pub(crate) struct ToolRun {
    pub(crate) name: String,
    args_json: String,
    authoritative_args: Option<serde_json::Value>,
    output: String,
    pub(crate) state: ToolCallState,
    exit_code: Option<i32>,
    terminal_emitted: bool,
}

impl ToolRun {
    pub(crate) fn args(&self) -> Option<serde_json::Value> {
        self.authoritative_args
            .clone()
            .or_else(|| serde_json::from_str(&self.args_json).ok())
    }

    pub(crate) fn output(&self) -> &str {
        &self.output
    }
}

/// A running or just-finished parallel subagent task.
#[derive(Clone, Debug)]
pub(crate) struct SubagentRun {
    pub(crate) agent: String,
    pub(crate) description: String,
    pub(crate) started: Instant,
    pub(crate) ended: Option<Instant>,
    pub(crate) tokens: u64,
    pub(crate) done: bool,
    pub(crate) success: Option<bool>,
    pub(crate) outcome: Option<SubagentOutcome>,
    pub(crate) output: String,
    pub(crate) use_capabilities: Vec<String>,
    parent_result_expected: bool,
}

impl SubagentRun {
    pub(crate) fn display_agent(&self) -> String {
        if !is_use_agent(&self.agent) || self.use_capabilities.is_empty() {
            return self.agent.clone();
        }
        let verb = if self.done { "Used" } else { "Using" };
        format!("{verb} {}", self.use_capabilities.join(" + "))
    }
}

/// Typed terminal outcome for a delegated child run.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SubagentOutcome {
    Succeeded,
    Failed,
    Cancelled,
    TrackingLost,
}

impl SubagentOutcome {
    pub(crate) fn is_success(self) -> bool {
        matches!(self, Self::Succeeded)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct CompletedSubagent {
    pub(crate) task_id: String,
    pub(crate) display_agent: String,
    pub(crate) description: String,
    pub(crate) output: String,
    pub(crate) success: bool,
    pub(crate) outcome: SubagentOutcome,
    /// Foreground task tools already render the same child result in their
    /// terminal card. Background/standalone children need their own cell.
    pub(crate) visible_in_transcript: bool,
}

#[derive(Clone, Debug)]
pub(crate) struct CompletedTool {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) args: Option<serde_json::Value>,
    pub(crate) output: String,
    pub(crate) exit_code: i32,
    pub(crate) state: ToolCallState,
    pub(crate) first_terminal: bool,
}

#[derive(Default, Debug)]
pub(crate) struct RuntimeProjection {
    tools: BTreeMap<String, ToolRun>,
    tool_order: Vec<String>,
    latest_input_tool_id: Option<String>,
    subagents: BTreeMap<String, SubagentRun>,
    subagent_order: Vec<String>,
    subagent_task: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct RuntimeToolCheckpoint {
    tools: BTreeMap<String, ToolRun>,
    tool_order: Vec<String>,
    latest_input_tool_id: Option<String>,
}

impl RuntimeProjection {
    pub(crate) fn clear_turn_entities(&mut self) {
        self.tools.clear();
        self.tool_order.clear();
        self.latest_input_tool_id = None;
        // Completed children are now durable transcript entries. Keep only
        // genuinely running background children in the live projection so a
        // parent turn boundary cannot erase their lifecycle.
        self.subagents.retain(|_, run| !run.done);
        self.subagent_order
            .retain(|task_id| self.subagents.contains_key(task_id));
        if self.subagents.is_empty() {
            self.subagent_task = None;
        }
    }

    pub(crate) fn finish_turn_entities(&mut self, _now: Instant) {
        for run in self.tools.values_mut() {
            if !run.state.is_terminal() {
                run.state = ToolCallState::Interrupted;
                run.exit_code = Some(130);
                if run.output.trim().is_empty() {
                    run.output = "Interrupted before the tool call completed.".to_string();
                }
            }
        }
        self.latest_input_tool_id = None;
        // Subagents have an independent lifecycle. In particular, a
        // background `task` may outlive the parent model turn; only a real
        // SubagentEnd (or tracker terminal snapshot) may make it terminal.
    }

    pub(crate) fn clear_live_tools(&mut self) {
        self.tools.clear();
        self.tool_order.clear();
        self.latest_input_tool_id = None;
    }

    pub(crate) fn checkpoint_tools(&self) -> RuntimeToolCheckpoint {
        RuntimeToolCheckpoint {
            tools: self.tools.clone(),
            tool_order: self.tool_order.clone(),
            latest_input_tool_id: self.latest_input_tool_id.clone(),
        }
    }

    pub(crate) fn restore_tools(&mut self, checkpoint: RuntimeToolCheckpoint) {
        self.tools = checkpoint.tools;
        self.tool_order = checkpoint.tool_order;
        self.latest_input_tool_id = checkpoint.latest_input_tool_id;
    }

    pub(crate) fn clear_subagent_entities(&mut self) {
        self.subagents.clear();
        self.subagent_order.clear();
        self.subagent_task = None;
    }

    /// Remove a projected child that no longer exists in the authoritative
    /// tracker. Durable transcript ownership remains with the caller.
    #[cfg(test)]
    pub(crate) fn remove_subagent(&mut self, task_id: &str) -> Option<SubagentRun> {
        let removed = self.subagents.remove(task_id)?;
        self.subagent_order.retain(|id| id != task_id);
        if self.subagents.is_empty() {
            self.subagent_task = None;
        }
        Some(removed)
    }

    #[cfg(test)]
    pub(crate) fn active_tool_count(&self) -> usize {
        self.tools
            .values()
            .filter(|run| !run.state.is_terminal())
            .count()
    }

    #[cfg(test)]
    pub(crate) fn active_subagent_count(&self) -> usize {
        self.subagents.values().filter(|run| !run.done).count()
    }

    pub(crate) fn subagents(&self) -> Vec<&SubagentRun> {
        self.subagent_order
            .iter()
            .filter_map(|id| self.subagents.get(id))
            .collect()
    }

    pub(crate) fn subagent_ids(&self) -> Vec<String> {
        self.subagent_order
            .iter()
            .filter(|id| self.subagents.contains_key(*id))
            .cloned()
            .collect()
    }

    pub(crate) fn set_subagent_task(&mut self, task: impl Into<String>) {
        let task = task.into();
        if !task.trim().is_empty() {
            self.subagent_task = Some(task);
        }
    }

    pub(crate) fn subagent_task(&self) -> Option<&str> {
        self.subagent_task.as_deref()
    }

    pub(crate) fn tool(&self, id: &str) -> Option<&ToolRun> {
        self.tools.get(id)
    }

    pub(crate) fn prepare_tool(&mut self, id: String, name: String) {
        let tool = self.ensure_tool(id.clone(), name);
        if !tool.state.is_terminal() {
            tool.state = ToolCallState::Preparing;
        }
        self.latest_input_tool_id = Some(id);
    }

    pub(crate) fn push_tool_input(&mut self, id: Option<&str>, delta: &str) -> bool {
        let id = id
            .map(str::to_string)
            .or_else(|| self.latest_input_tool_id.clone());
        if let Some(tool) = id.as_deref().and_then(|id| self.tools.get_mut(id)) {
            tool.args_json.push_str(delta);
            true
        } else {
            false
        }
    }

    pub(crate) fn await_approval(&mut self, id: String, name: String, args: serde_json::Value) {
        let tool = self.ensure_tool(id.clone(), name);
        tool.authoritative_args = Some(args);
        if !tool.state.is_terminal() {
            tool.state = ToolCallState::AwaitingApproval;
        }
        self.latest_input_tool_id = Some(id);
    }

    pub(crate) fn start_execution(&mut self, id: String, name: String, args: serde_json::Value) {
        let tool = self.ensure_tool(id.clone(), name);
        tool.authoritative_args = Some(args);
        if !tool.state.is_terminal() {
            tool.state = ToolCallState::Running;
        }
        self.latest_input_tool_id = Some(id);
    }

    pub(crate) fn push_tool_output(&mut self, id: &str, name: String, delta: &str) {
        if !self.tools.contains_key(id) {
            self.ensure_tool(id.to_string(), name);
        }
        if let Some(tool) = self.tools.get_mut(id) {
            if !tool.state.is_terminal() {
                tool.state = ToolCallState::Running;
            }
            tool.output.push_str(delta);
        }
    }

    pub(crate) fn end_tool(
        &mut self,
        id: &str,
        name: String,
        authoritative_args: Option<serde_json::Value>,
        output: String,
        exit_code: i32,
    ) -> CompletedTool {
        let (args, state, effective_output, effective_exit_code, first_terminal) = {
            let run = self.ensure_tool(id.to_string(), name.clone());
            if authoritative_args.is_some() {
                run.authoritative_args = authoritative_args;
            }
            let args = run.args();
            let protected_state = matches!(
                run.state,
                ToolCallState::Denied | ToolCallState::TimedOut | ToolCallState::Interrupted
            );
            if !protected_state || run.output.trim().is_empty() {
                run.output = output.clone();
            }
            if !protected_state {
                run.exit_code = Some(exit_code);
                run.state = if exit_code == 0 {
                    ToolCallState::Succeeded
                } else {
                    ToolCallState::Failed
                };
            }
            let first_terminal = !run.terminal_emitted;
            run.terminal_emitted = true;
            (
                args,
                run.state,
                run.output.clone(),
                run.exit_code.unwrap_or(exit_code),
                first_terminal,
            )
        };
        if self.latest_input_tool_id.as_deref() == Some(id) {
            self.latest_input_tool_id = None;
        }
        CompletedTool {
            id: id.to_string(),
            name,
            args,
            output: effective_output,
            exit_code: effective_exit_code,
            state,
            first_terminal,
        }
    }

    pub(crate) fn deny_tool(
        &mut self,
        id: &str,
        name: String,
        args: Option<serde_json::Value>,
        reason: impl Into<String>,
    ) -> CompletedTool {
        self.finish_without_execution(id, name, args, ToolCallState::Denied, reason.into())
    }

    pub(crate) fn timeout_tool(&mut self, id: &str, action_taken: &str) -> Option<CompletedTool> {
        if action_taken == "auto_approved" {
            if let Some(tool) = self.tools.get_mut(id) {
                tool.state = ToolCallState::Running;
            }
            return None;
        }
        let name = self.tools.get(id)?.name.clone();
        Some(self.finish_without_execution(
            id,
            name,
            None,
            ToolCallState::TimedOut,
            "Confirmation timed out; the tool call was denied.".to_string(),
        ))
    }

    pub(crate) fn interrupt_unfinished_tools(&mut self) -> Vec<CompletedTool> {
        let ids = self
            .tool_order
            .iter()
            .filter(|id| {
                self.tools
                    .get(*id)
                    .is_some_and(|run| !run.state.is_terminal())
            })
            .cloned()
            .collect::<Vec<_>>();
        ids.into_iter()
            .filter_map(|id| {
                let name = self.tools.get(&id)?.name.clone();
                Some(self.finish_without_execution(
                    &id,
                    name,
                    None,
                    ToolCallState::Interrupted,
                    "Interrupted before the tool call completed.".to_string(),
                ))
            })
            .collect()
    }

    pub(crate) fn start_subagent(
        &mut self,
        task_id: String,
        agent: String,
        description: String,
        now: Instant,
    ) -> bool {
        let parent_result_expected = self.subagent_parent_result_expected(&agent, &description);
        self.restore_subagent(task_id, agent, description, now, parent_result_expected)
    }

    pub(crate) fn restore_subagent(
        &mut self,
        task_id: String,
        agent: String,
        description: String,
        now: Instant,
        parent_result_expected: bool,
    ) -> bool {
        if let Some(run) = self.subagents.get_mut(&task_id) {
            if !is_use_agent(&agent) {
                run.use_capabilities.clear();
            }
            run.agent = agent;
            run.description = description;
            run.parent_result_expected |= parent_result_expected;
            if !run.done {
                run.ended = None;
            }
            return false;
        }
        self.subagent_order.push(task_id.clone());
        self.subagents.insert(
            task_id.clone(),
            SubagentRun {
                agent,
                description,
                started: now,
                ended: None,
                tokens: 0,
                done: false,
                success: None,
                outcome: None,
                output: String::new(),
                use_capabilities: Vec::new(),
                parent_result_expected,
            },
        );
        true
    }

    pub(crate) fn subagent_needs_completion_watch(&self, task_id: &str) -> bool {
        self.subagents.get(task_id).is_some_and(|run| !run.done)
    }

    pub(crate) fn add_subagent_tokens(&mut self, task_id: &str, tokens: u64) {
        if let Some(run) = self.subagents.get_mut(task_id) {
            run.tokens += tokens;
        }
    }

    pub(crate) fn record_subagent_progress(&mut self, task_id: &str, metadata: &serde_json::Value) {
        let Some(run) = self.subagents.get_mut(task_id) else {
            return;
        };
        let Some(capability) = use_capability_from_progress(&run.agent, metadata) else {
            return;
        };
        if !run.use_capabilities.contains(&capability) {
            run.use_capabilities.push(capability);
        }
    }

    pub(crate) fn end_subagent(
        &mut self,
        task_id: String,
        agent: String,
        output: String,
        success: bool,
        now: Instant,
    ) -> CompletedSubagent {
        let outcome = if success {
            SubagentOutcome::Succeeded
        } else {
            SubagentOutcome::Failed
        };
        self.end_subagent_with_outcome(task_id, agent, output, outcome, now)
    }

    pub(crate) fn end_subagent_with_outcome(
        &mut self,
        task_id: String,
        agent: String,
        output: String,
        outcome: SubagentOutcome,
        now: Instant,
    ) -> CompletedSubagent {
        if !self.subagents.contains_key(&task_id) {
            self.subagent_order.push(task_id.clone());
        }
        let run = self
            .subagents
            .entry(task_id.clone())
            .or_insert_with(|| SubagentRun {
                agent: agent.clone(),
                description: String::new(),
                started: now,
                ended: None,
                tokens: 0,
                done: false,
                success: None,
                outcome: None,
                output: String::new(),
                use_capabilities: Vec::new(),
                // With no observed parent tool there is no other semantic
                // cell that can own this terminal result.
                parent_result_expected: false,
            });
        // A terminal reconciliation is authoritative even if a watcher emits a
        // late generic end event. This is especially important for cancellation
        // and tracking loss, whose typed meaning is more precise than `Failed`.
        let outcome = run.outcome.unwrap_or(outcome);
        if !agent.is_empty() {
            run.agent = agent;
        }
        run.output = output;
        run.done = true;
        run.success = Some(outcome.is_success());
        run.outcome = Some(outcome);
        run.ended = Some(now);
        let display_agent = run.display_agent();
        CompletedSubagent {
            task_id,
            display_agent,
            description: run.description.clone(),
            output: run.output.clone(),
            success: outcome.is_success(),
            outcome,
            visible_in_transcript: !run.parent_result_expected,
        }
    }

    /// Convert a live row whose authoritative tracker disappeared into a
    /// terminal result. This lets the host clear stale footer state while
    /// retaining one explicit transcript outcome when appropriate.
    #[cfg(test)]
    pub(crate) fn mark_subagent_tracking_lost(
        &mut self,
        task_id: &str,
        output: impl Into<String>,
        now: Instant,
    ) -> Option<CompletedSubagent> {
        let agent = self.subagents.get(task_id)?.agent.clone();
        Some(self.end_subagent_with_outcome(
            task_id.to_string(),
            agent,
            output.into(),
            SubagentOutcome::TrackingLost,
            now,
        ))
    }

    fn subagent_parent_result_expected(&self, agent: &str, description: &str) -> bool {
        self.tool_order
            .iter()
            .filter_map(|tool_id| self.tools.get(tool_id))
            .filter_map(|tool| match tool.name.as_str() {
                "task" => tool.args().map(|args| vec![args]),
                "parallel_task" => tool.args().and_then(|args| {
                    args.get("tasks")
                        .and_then(serde_json::Value::as_array)
                        .cloned()
                }),
                _ => None,
            })
            .flatten()
            .filter(|task| subagent_spec_matches(task, agent, description))
            .any(|task| {
                !task
                    .get("background")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false)
            })
    }

    fn ensure_tool(&mut self, id: String, name: String) -> &mut ToolRun {
        if !self.tools.contains_key(&id) {
            self.tool_order.push(id.clone());
            self.tools.insert(
                id.clone(),
                ToolRun {
                    name: name.clone(),
                    args_json: String::new(),
                    authoritative_args: None,
                    output: String::new(),
                    state: ToolCallState::Preparing,
                    exit_code: None,
                    terminal_emitted: false,
                },
            );
        }
        let tool = self.tools.get_mut(&id).expect("inserted tool projection");
        if tool.name.is_empty() {
            tool.name = name;
        }
        tool
    }

    fn finish_without_execution(
        &mut self,
        id: &str,
        name: String,
        authoritative_args: Option<serde_json::Value>,
        state: ToolCallState,
        output: String,
    ) -> CompletedTool {
        let (args, first_terminal) = {
            let run = self.ensure_tool(id.to_string(), name.clone());
            if authoritative_args.is_some() {
                run.authoritative_args = authoritative_args;
            }
            let args = run.args();
            run.output = output.clone();
            run.exit_code = Some(1);
            run.state = state;
            let first_terminal = !run.terminal_emitted;
            run.terminal_emitted = true;
            (args, first_terminal)
        };
        if self.latest_input_tool_id.as_deref() == Some(id) {
            self.latest_input_tool_id = None;
        }
        CompletedTool {
            id: id.to_string(),
            name,
            args,
            output,
            exit_code: 1,
            state,
            first_terminal,
        }
    }
}

fn subagent_spec_matches(task: &serde_json::Value, agent: &str, description: &str) -> bool {
    let task_agent = task
        .get("agent")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    let task_description = task
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or_default();
    (agent.is_empty() || task_agent.is_empty() || task_agent == agent)
        && (description.is_empty()
            || task_description.is_empty()
            || task_description == description)
}

fn use_capability_from_progress(agent: &str, metadata: &serde_json::Value) -> Option<String> {
    if !is_use_agent(agent) {
        return None;
    }
    let tool = metadata.get("tool")?.as_str()?;
    let rest = tool.strip_prefix("mcp__use_")?;
    let (route, operation) = rest.split_once("__")?;
    if route.is_empty()
        || operation.is_empty()
        || !route.chars().all(|character| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '-' | '_')
        })
    {
        return None;
    }
    Some(humanize_identifier(route))
}

fn is_use_agent(agent: &str) -> bool {
    agent.trim().eq_ignore_ascii_case("use")
}

fn humanize_identifier(value: &str) -> String {
    value
        .split(['-', '_'])
        .filter(|word| !word.is_empty())
        .map(|word| {
            let mut characters = word.chars();
            match characters.next() {
                Some(first) => format!("{}{}", first.to_ascii_uppercase(), characters.as_str()),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_projection_tracks_live_tool_by_id_and_completes_once() {
        let mut projection = RuntimeProjection::default();

        projection.prepare_tool("t1".into(), "bash".into());
        assert!(projection.push_tool_input(Some("t1"), r#"{"command":"echo hi"}"#));
        projection.start_execution(
            "t1".into(),
            "bash".into(),
            serde_json::json!({"command": "echo hi"}),
        );
        projection.push_tool_output("t1", "bash".into(), "hi\n");

        let live = projection.tool("t1").expect("live tool");
        assert_eq!(live.name, "bash");
        assert_eq!(live.args().unwrap()["command"], "echo hi");
        assert_eq!(live.output(), "hi\n");
        assert_eq!(live.state, ToolCallState::Running);
        assert_eq!(projection.active_tool_count(), 1);

        let completed = projection.end_tool(
            "t1",
            "bash".into(),
            Some(serde_json::json!({"command": "echo hi"})),
            "hi\n".into(),
            0,
        );
        assert_eq!(completed.args.unwrap()["command"], "echo hi");
        assert_eq!(completed.state, ToolCallState::Succeeded);
        assert_eq!(completed.output, "hi\n");
        assert!(completed.first_terminal);
        assert_eq!(projection.active_tool_count(), 0);
    }

    #[test]
    fn unknown_tool_end_never_borrows_another_call() {
        let mut projection = RuntimeProjection::default();

        projection.prepare_tool("actual".into(), "bash".into());
        projection.push_tool_input(Some("actual"), r#"{"command":"pwd"}"#);

        let completed = projection.end_tool(
            "missing",
            "bash".into(),
            Some(serde_json::json!({"command": "other"})),
            "/work\n".into(),
            0,
        );

        assert_eq!(completed.args.unwrap()["command"], "other");
        assert_eq!(projection.active_tool_count(), 1);
        assert_eq!(
            projection.tool("actual").unwrap().args().unwrap()["command"],
            "pwd"
        );
        assert_eq!(completed.output, "/work\n");
        assert!(completed.first_terminal);
    }

    #[test]
    fn input_deltas_bind_to_explicit_parallel_call_ids() {
        let mut projection = RuntimeProjection::default();

        projection.prepare_tool("a".into(), "bash".into());
        projection.prepare_tool("b".into(), "grep".into());
        projection.push_tool_input(Some("a"), r#"{"command":"cargo test"}"#);
        projection.push_tool_input(Some("b"), r#"{"pattern":"TODO"}"#);

        assert_eq!(projection.active_tool_count(), 2);
        assert_eq!(
            projection.tool("a").unwrap().args().unwrap()["command"],
            "cargo test"
        );
        assert_eq!(
            projection.tool("b").unwrap().args().unwrap()["pattern"],
            "TODO"
        );
    }

    #[test]
    fn denied_tool_is_terminal_and_retained() {
        let mut projection = RuntimeProjection::default();

        projection.prepare_tool("a".into(), "bash".into());
        projection.await_approval(
            "a".into(),
            "bash".into(),
            serde_json::json!({"command": "rm -rf nope"}),
        );
        let completed = projection.deny_tool("a", "bash".into(), None, "Denied by user");

        assert_eq!(completed.state, ToolCallState::Denied);
        assert_eq!(completed.output, "Denied by user");
        assert!(completed.first_terminal);
        assert_eq!(projection.active_tool_count(), 0);
        assert_eq!(projection.tool("a").unwrap().state, ToolCallState::Denied);
    }

    #[test]
    fn later_tool_end_cannot_downgrade_denied_state_to_failed() {
        let mut projection = RuntimeProjection::default();
        projection.prepare_tool("a".into(), "bash".into());
        projection.await_approval(
            "a".into(),
            "bash".into(),
            serde_json::json!({"command": "dangerous"}),
        );
        let denied = projection.deny_tool("a", "bash".into(), None, "Denied by user");
        assert!(denied.first_terminal);

        let completed = projection.end_tool(
            "a",
            "bash".into(),
            Some(serde_json::json!({"command": "dangerous"})),
            "tool execution denied".into(),
            1,
        );

        assert_eq!(completed.state, ToolCallState::Denied);
        assert_eq!(completed.output, "Denied by user");
        assert!(!completed.first_terminal);
        assert_eq!(projection.tool("a").unwrap().state, ToolCallState::Denied);
    }

    #[test]
    fn subagent_projection_counts_only_running_agents() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();

        projection.start_subagent("a".into(), "explore".into(), "inspect".into(), now);
        projection.start_subagent("b".into(), "review".into(), "audit".into(), now);
        projection.add_subagent_tokens("a", 12);
        projection.end_subagent("a".into(), "explore".into(), "done".into(), true, now);

        assert_eq!(projection.active_subagent_count(), 1);
        let runs = projection.subagents();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].tokens, 12);
        assert!(runs[0].done);
        assert!(!runs[1].done);
    }

    #[test]
    fn use_subagent_projects_ordered_capabilities_from_standard_mcp_progress() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.start_subagent(
            "use-1".into(),
            "use".into(),
            "Gather browser evidence and update the workbook".into(),
            now,
        );

        for tool in [
            "mcp__use_browser__agent_browser_open",
            "mcp__use_browser__browser_snapshot",
            "mcp__use_office__office_validate",
        ] {
            projection.record_subagent_progress(
                "use-1",
                &serde_json::json!({ "tool": tool, "exit_code": 0 }),
            );
        }

        let run = projection.subagents()[0];
        assert_eq!(run.use_capabilities, ["Browser", "Office"]);
        assert_eq!(run.display_agent(), "Using Browser + Office");

        let completed = projection.end_subagent(
            "use-1".into(),
            "use".into(),
            "Evidence collected.".into(),
            true,
            now,
        );
        assert_eq!(projection.subagents()[0].agent, "use");
        assert_eq!(completed.display_agent, "Used Browser + Office");
    }

    #[test]
    fn use_capability_projection_requires_the_dedicated_use_worker() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.start_subagent("review".into(), "review".into(), "Audit".into(), now);
        projection.record_subagent_progress(
            "review",
            &serde_json::json!({ "tool": "mcp__use_browser__browser_open" }),
        );
        projection.start_subagent("use".into(), "use".into(), "Query MCP".into(), now);
        projection.record_subagent_progress(
            "use",
            &serde_json::json!({ "tool": "mcp__search__find_docs" }),
        );

        let runs = projection.subagents();
        assert!(runs[0].use_capabilities.is_empty());
        assert_eq!(runs[0].display_agent(), "review");
        assert!(runs[1].use_capabilities.is_empty());
        assert_eq!(runs[1].display_agent(), "use");
    }

    #[test]
    fn subagent_projection_preserves_first_start_time_on_duplicate_start() {
        let mut projection = RuntimeProjection::default();
        let first = Instant::now();
        let duplicate = first + std::time::Duration::from_secs(5);
        let end = first + std::time::Duration::from_secs(8);

        projection.start_subagent(
            "a".into(),
            "explore".into(),
            "first description".into(),
            first,
        );
        projection.start_subagent(
            "a".into(),
            "general".into(),
            "refreshed description".into(),
            duplicate,
        );
        projection.end_subagent("a".into(), "general".into(), "done".into(), true, end);

        let runs = projection.subagents();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].agent, "general");
        assert_eq!(runs[0].description, "refreshed description");
        assert_eq!(
            runs[0]
                .ended
                .unwrap()
                .saturating_duration_since(runs[0].started),
            std::time::Duration::from_secs(8)
        );
    }

    #[test]
    fn foreground_child_result_is_owned_by_parent_tool_cell() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.start_execution(
            "parent".into(),
            "task".into(),
            serde_json::json!({
                "agent": "review",
                "description": "audit",
                "prompt": "Audit it"
            }),
        );
        projection.start_subagent("child".into(), "review".into(), "audit".into(), now);

        assert!(projection.subagent_needs_completion_watch("child"));
        let completed = projection.end_subagent(
            "child".into(),
            "review".into(),
            "findings".into(),
            true,
            now,
        );
        assert!(!completed.visible_in_transcript);
        assert_eq!(completed.output, "findings");
        assert_eq!(completed.outcome, SubagentOutcome::Succeeded);
    }

    #[test]
    fn background_child_owns_its_terminal_transcript_cell() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.start_execution(
            "parent".into(),
            "task".into(),
            serde_json::json!({
                "agent": "review",
                "description": "audit later",
                "prompt": "Audit it",
                "background": true
            }),
        );
        projection.start_subagent("child".into(), "review".into(), "audit later".into(), now);

        assert!(projection.subagent_needs_completion_watch("child"));
        let completed = projection.end_subagent(
            "child".into(),
            "review".into(),
            "background findings".into(),
            true,
            now,
        );
        assert!(completed.visible_in_transcript);
        assert_eq!(completed.description, "audit later");
        assert_eq!(completed.output, "background findings");
        assert_eq!(completed.outcome, SubagentOutcome::Succeeded);
    }

    #[test]
    fn restored_running_background_child_remains_live_and_watchable() {
        let mut projection = RuntimeProjection::default();
        projection.restore_subagent(
            "restored".into(),
            "review".into(),
            "resume audit".into(),
            Instant::now(),
            false,
        );

        assert_eq!(projection.active_subagent_count(), 1);
        assert!(projection.subagent_needs_completion_watch("restored"));
        projection.finish_turn_entities(Instant::now());
        assert_eq!(projection.active_subagent_count(), 1);
    }

    #[test]
    fn finish_turn_entities_does_not_fail_a_background_subagent() {
        let mut projection = RuntimeProjection::default();
        let start = Instant::now();
        let finish = start + std::time::Duration::from_secs(4);

        projection.set_subagent_task("DeepResearch market map");
        projection.prepare_tool("tool".into(), "parallel_task".into());
        projection.start_subagent("a".into(), "researcher".into(), "inspect".into(), start);
        projection.start_subagent("b".into(), "reviewer".into(), "audit".into(), start);
        projection.end_subagent("a".into(), "researcher".into(), "done".into(), true, finish);

        projection.finish_turn_entities(finish);

        assert_eq!(projection.active_tool_count(), 0);
        assert_eq!(projection.active_subagent_count(), 1);
        let runs = projection.subagents();
        assert_eq!(runs.len(), 2);
        assert!(runs[0].done);
        assert_eq!(runs[0].success, Some(true));
        assert!(!runs[1].done);
        assert_eq!(runs[1].success, None);
        assert_eq!(runs[1].ended, None);
        assert_eq!(projection.subagent_task(), Some("DeepResearch market map"));
    }

    #[test]
    fn clear_turn_entities_prunes_completed_but_preserves_running_subagents() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.set_subagent_task("background audit");
        projection.start_subagent("done".into(), "review".into(), "finished".into(), now);
        projection.end_subagent("done".into(), "review".into(), "complete".into(), true, now);
        projection.start_subagent("live".into(), "explore".into(), "still running".into(), now);

        projection.clear_turn_entities();

        let runs = projection.subagents();
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].description, "still running");
        assert!(!runs[0].done);
        assert_eq!(projection.subagent_task(), Some("background audit"));
    }

    #[test]
    fn clear_turn_entities_resets_subagent_task_title() {
        let mut projection = RuntimeProjection::default();
        projection.set_subagent_task("DeepResearch market map");
        projection.clear_turn_entities();
        assert_eq!(projection.subagent_task(), None);
    }

    #[test]
    fn tool_checkpoint_removes_only_the_retried_attempt_draft() {
        let mut projection = RuntimeProjection::default();
        projection.prepare_tool("completed".into(), "read".into());
        projection.end_tool(
            "completed",
            "read".into(),
            Some(serde_json::json!({"file_path": "src/lib.rs"})),
            "done".into(),
            0,
        );
        let checkpoint = projection.checkpoint_tools();

        projection.prepare_tool("partial".into(), "bash".into());
        projection.push_tool_input(Some("partial"), r#"{"command":"car"#);
        projection.restore_tools(checkpoint);

        assert!(projection.tool("completed").is_some());
        assert!(projection.tool("partial").is_none());
        assert_eq!(projection.active_tool_count(), 0);
    }

    #[test]
    fn typed_cancelled_outcome_is_not_downgraded_by_late_failed_end() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.start_subagent("child".into(), "review".into(), "audit".into(), now);

        let cancelled = projection.end_subagent_with_outcome(
            "child".into(),
            "review".into(),
            "Task cancelled.".into(),
            SubagentOutcome::Cancelled,
            now,
        );
        let late = projection.end_subagent(
            "child".into(),
            "review".into(),
            "Task cancelled by caller.".into(),
            false,
            now,
        );

        assert_eq!(cancelled.outcome, SubagentOutcome::Cancelled);
        assert_eq!(late.outcome, SubagentOutcome::Cancelled);
        assert!(!late.success);
        assert_eq!(
            projection.subagents()[0].outcome,
            Some(SubagentOutcome::Cancelled)
        );
        assert_eq!(projection.active_subagent_count(), 0);
    }

    #[test]
    fn every_active_subagent_needs_an_authoritative_completion_watch() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.start_execution(
            "parent".into(),
            "task".into(),
            serde_json::json!({
                "agent": "review",
                "description": "foreground audit",
                "prompt": "Audit it"
            }),
        );
        projection.start_subagent(
            "foreground".into(),
            "review".into(),
            "foreground audit".into(),
            now,
        );
        projection.start_subagent(
            "background".into(),
            "explore".into(),
            "background scan".into(),
            now,
        );

        assert!(projection.subagent_needs_completion_watch("foreground"));
        assert!(projection.subagent_needs_completion_watch("background"));

        projection.end_subagent(
            "foreground".into(),
            "review".into(),
            "done".into(),
            true,
            now,
        );
        assert!(!projection.subagent_needs_completion_watch("foreground"));
    }

    #[test]
    fn stale_subagents_can_be_terminalized_or_removed_for_reconciliation() {
        let mut projection = RuntimeProjection::default();
        let now = Instant::now();
        projection.set_subagent_task("parallel audit");
        projection.start_subagent("lost".into(), "review".into(), "audit".into(), now);
        projection.start_subagent("remove".into(), "explore".into(), "scan".into(), now);

        let lost = projection
            .mark_subagent_tracking_lost("lost", "Tracker no longer reports this task.", now)
            .expect("tracking-lost terminal result");
        assert_eq!(lost.outcome, SubagentOutcome::TrackingLost);
        assert!(!lost.success);
        assert_eq!(projection.active_subagent_count(), 1);

        let removed = projection
            .remove_subagent("remove")
            .expect("stale row should be removable");
        assert_eq!(removed.description, "scan");
        assert_eq!(projection.active_subagent_count(), 0);
        assert_eq!(projection.subagent_task(), Some("parallel audit"));

        assert!(projection.remove_subagent("lost").is_some());
        assert_eq!(projection.subagent_task(), None);
        assert!(projection.remove_subagent("missing").is_none());
    }
}
