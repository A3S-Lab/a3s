//! DeepResearch, tool, plan, and input-history state projections.

use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct DeepResearchLoop {
    pub(super) query: String,
    pub(super) evidence_scope: DeepResearchEvidenceScope,
    pub(super) started_at: Instant,
}

pub(super) fn is_new_remote_view(
    last_view: Option<&remote_ui::ViewSpec>,
    spec: &remote_ui::ViewSpec,
) -> bool {
    last_view != Some(spec)
}

pub(super) fn take_pending_tool_approval(
    pending_tools: &mut VecDeque<PendingToolApproval>,
    tool_id: &str,
) -> Option<(PendingToolApproval, bool)> {
    let index = pending_tools
        .iter()
        .position(|pending| pending.tool_id == tool_id)?;
    let was_front = index == 0;
    pending_tools
        .remove(index)
        .map(|pending| (pending, was_front))
}

pub(super) fn take_pending_tool_for_confirmation(
    pending_tools: &mut VecDeque<PendingToolApproval>,
    expected_tool_id: &str,
) -> Option<PendingToolApproval> {
    if pending_tools
        .front()
        .is_none_or(|pending| pending.tool_id != expected_tool_id)
    {
        return None;
    }
    pending_tools.pop_front()
}

/// Presentation ownership for a model-requested tool call.
///
/// Most tools own a durable transcript cell. Plan updates instead own the
/// pinned checklist above the input; retaining a second transcript cell would
/// show the same state twice. The runtime projection still tracks the call so
/// duplicate terminal delivery cannot reintroduce it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ToolPresentationPolicy {
    Transcript,
    PinnedOnly,
}

pub(super) fn presentation_policy(tool_name: &str) -> ToolPresentationPolicy {
    if tool_name.trim().eq_ignore_ascii_case("update_plan") {
        ToolPresentationPolicy::PinnedOnly
    } else {
        ToolPresentationPolicy::Transcript
    }
}

impl ToolPresentationPolicy {
    pub(super) fn transcript_visible(self) -> bool {
        matches!(self, Self::Transcript)
    }
}

/// Typed materialized view of the active turn's plan.
///
/// Keep semantic task status until the presentation boundary. Storing glyphs
/// and colours here previously collapsed skipped/cancelled back to pending and
/// made synthesis consume UI decoration as domain state.
#[derive(Clone, Debug, Default)]
pub(super) struct PlanProjection {
    tasks: Vec<a3s_code_core::planning::Task>,
}

impl PlanProjection {
    pub(super) fn replace(&mut self, tasks: &[a3s_code_core::planning::Task]) {
        self.tasks = tasks.to_vec();
    }

    pub(super) fn update_status(&mut self, id: &str, status: a3s_code_core::planning::TaskStatus) {
        if let Some(task) = self.tasks.iter_mut().find(|task| task.id == id) {
            task.status = status;
        }
    }

    pub(super) fn tasks(&self) -> &[a3s_code_core::planning::Task] {
        &self.tasks
    }

    pub(super) fn is_empty(&self) -> bool {
        self.tasks.is_empty()
    }

    pub(super) fn clear(&mut self) {
        self.tasks.clear();
    }
}

pub(super) fn history_recall_value(
    history: &[String],
    position: &mut Option<usize>,
    draft: &mut Option<String>,
    current: &str,
    up: bool,
) -> Option<String> {
    if history.is_empty() {
        return None;
    }

    let pos = match (*position, up) {
        (None, true) => {
            *draft = Some(current.to_string());
            history.len() - 1
        }
        (None, false) => return None,
        (Some(i), true) => i.saturating_sub(1),
        (Some(i), false) => i.saturating_add(1),
    };

    if pos >= history.len() {
        *position = None;
        Some(draft.take().unwrap_or_default())
    } else {
        *position = Some(pos);
        Some(history[pos].clone())
    }
}

pub(super) fn should_exit_prompt_mode(
    state: &State,
    shell_mode: bool,
    research_mode: bool,
    key: &KeyEvent,
) -> bool {
    state != &State::Streaming && (shell_mode || research_mode) && key.code == KeyCode::Esc
}
