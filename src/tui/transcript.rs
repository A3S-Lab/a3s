//! Source-backed transcript entries that can be re-rendered after resize.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use a3s_tui::components::{selected_text_range, SelectionRange};
use a3s_tui::style::{strip_ansi, truncate_visible, visible_len};

use super::design_markdown::Markdown;
use super::message_chrome::{
    message_branch, message_marker, message_title, render_notice, sanitize_message_source,
    subagent_message_tone, tool_message_tone, MessageBranch, MessageTone, NoticeKind,
};
use super::render::{
    arg_summary_for_tool, render_live_tool_activity, render_tool_terminal, render_tool_transcript,
    ToolTranscriptInput,
};
use super::runtime_projection::{SubagentOutcome, ToolCallState};
use super::tool_style::highlight_explore_detail;
#[cfg(test)]
use super::TN_CYAN;
use super::{assistant_block, user_bubble, wrap_words, Style, TN_FG, TN_GRAY};

const TRANSCRIPT_BLOCK_SEPARATOR: &str = "\n \n";
const TRANSCRIPT_BLOCK_GAP_ROWS: usize = 1;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum TranscriptEntry {
    /// Already-rendered UI notices and non-reflowable terminal artifacts.
    Preformatted(String),
    /// Source-backed system feedback with stable severity and responsive layout.
    Notice { kind: NoticeKind, source: String },
    /// Raw user text, rendered into the transcript bubble at the current width.
    User { source: String },
    /// Raw assistant Markdown, rendered canonically at the current width.
    AssistantMarkdown { source: String },
    /// Completed model reasoning. Hidden from normal history but retained for
    /// the full semantic Ctrl+T transcript after the live thinking pane clears.
    Reasoning { source: String },
    /// Semantic tool call, retained from preparation through completion.
    Tool(ToolTranscriptEntry),
    /// Terminal delegated child result. Foreground children may be retained
    /// invisibly because their parent task tool owns the same rendered result.
    Subagent(SubagentTranscriptEntry),
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct ToolTranscriptEntry {
    call_id: Option<String>,
    name: String,
    state: ToolCallState,
    args_json: String,
    args: Option<serde_json::Value>,
    output: String,
    metadata: Option<serde_json::Value>,
    exit_code: Option<i32>,
    started_at: Option<Instant>,
    duration: Option<Duration>,
    visible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct SubagentTranscriptEntry {
    task_id: String,
    agent: String,
    task: String,
    outcome: SubagentOutcome,
    output: String,
    visible: bool,
}

impl ToolTranscriptEntry {
    fn args(&self) -> Option<serde_json::Value> {
        self.args
            .clone()
            .or_else(|| serde_json::from_str(&self.args_json).ok())
    }

    fn is_groupable_explore(&self) -> bool {
        self.visible
            && matches!(
                self.state,
                ToolCallState::Preparing | ToolCallState::Running | ToolCallState::Succeeded
            )
            && matches!(
                self.name.as_str(),
                "read" | "cat" | "grep" | "search" | "ls" | "glob" | "find"
            )
    }
}

#[derive(Debug)]
struct StoredTranscriptEntry {
    id: TranscriptEntryId,
    revision: u64,
    entry: TranscriptEntry,
    render_cache: Option<EntryRenderCache>,
}

/// Stable identity for a transcript entry whose presentation may evolve from
/// an in-progress notice into one terminal result.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct TranscriptEntryId(u64);

#[derive(Clone, Debug)]
struct EntryRenderCache {
    revision: u64,
    screen_width: u16,
    content_width: usize,
    activity_phase: Option<bool>,
    block: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptAnchor {
    entry_id: TranscriptEntryId,
    row_in_entry: usize,
}

/// Stable selection endpoint inside one semantic transcript entry.
///
/// `semantic_offset` counts content characters rather than rendered cells, so
/// soft wrapping, continuation indentation, and gutter decoration do not move
/// the endpoint. Row/column hints are retained only for entries that contain no
/// semantic characters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptPoint {
    entry_id: TranscriptEntryId,
    semantic_offset: usize,
    row_hint: usize,
    col_hint: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct TranscriptSelection {
    anchor: TranscriptPoint,
    head: TranscriptPoint,
}

impl TranscriptSelection {
    pub(crate) fn collapsed(point: TranscriptPoint) -> Self {
        Self {
            anchor: point,
            head: point,
        }
    }

    pub(crate) fn set_head(&mut self, point: TranscriptPoint) {
        self.head = point;
    }

    pub(crate) fn anchor(&self) -> TranscriptPoint {
        self.anchor
    }

    pub(crate) fn head(&self) -> TranscriptPoint {
        self.head
    }
}

#[derive(Clone, Copy, Debug)]
struct LayoutSpan {
    entry_id: TranscriptEntryId,
    start_row: usize,
    row_count: usize,
}

#[derive(Debug, Default)]
pub(crate) struct Transcript {
    entries: Vec<StoredTranscriptEntry>,
    tool_positions: HashMap<String, usize>,
    latest_input_tool_id: Option<String>,
    next_id: u64,
    layout: Vec<LayoutSpan>,
    selection_rows: Vec<String>,
}

impl TranscriptEntry {
    pub(crate) fn preformatted(value: impl Into<String>) -> Self {
        Self::Preformatted(value.into())
    }

    pub(crate) fn user(source: impl Into<String>) -> Self {
        Self::User {
            source: source.into(),
        }
    }

    pub(crate) fn notice(kind: NoticeKind, source: impl Into<String>) -> Self {
        Self::Notice {
            kind,
            source: source.into(),
        }
    }

    pub(crate) fn assistant_markdown(source: impl Into<String>) -> Self {
        Self::AssistantMarkdown {
            source: source.into(),
        }
    }

    pub(crate) fn reasoning(source: impl Into<String>) -> Self {
        Self::Reasoning {
            source: source.into(),
        }
    }

    #[cfg(test)]
    pub(crate) fn tool(
        name: impl Into<String>,
        exit_code: i32,
        output: impl Into<String>,
        metadata: Option<serde_json::Value>,
        args: Option<serde_json::Value>,
    ) -> Self {
        Self::Tool(ToolTranscriptEntry {
            call_id: None,
            name: name.into(),
            state: if exit_code == 0 {
                ToolCallState::Succeeded
            } else {
                ToolCallState::Failed
            },
            args_json: String::new(),
            args,
            output: output.into(),
            metadata,
            exit_code: Some(exit_code),
            started_at: None,
            duration: None,
            visible: true,
        })
    }

    #[cfg(test)]
    pub(crate) fn render(&self, screen_width: u16, content_width: usize) -> String {
        self.render_with_activity(screen_width, content_width, true)
    }

    fn render_with_activity(
        &self,
        _screen_width: u16,
        content_width: usize,
        activity_phase: bool,
    ) -> String {
        match self {
            Self::Preformatted(value) => value.clone(),
            Self::Notice { kind, source } => render_notice(*kind, source, content_width),
            Self::User { source } => user_bubble(&sanitize_message_source(source), content_width),
            Self::AssistantMarkdown { source } => {
                let source = sanitize_message_source(source);
                let rendered = Markdown::new()
                    .with_width(content_width.saturating_sub(2).max(1))
                    .render(&source);
                assistant_block(&rendered, content_width)
            }
            Self::Reasoning { .. } => String::new(),
            Self::Subagent(subagent) if !subagent.visible => String::new(),
            Self::Subagent(subagent) => render_subagent_result(subagent, content_width, false),
            Self::Tool(tool) if !tool.visible => String::new(),
            Self::Tool(tool) if tool.state.is_terminal() => render_tool_terminal(
                &tool.name,
                tool.state,
                tool.exit_code.unwrap_or(1),
                &tool.output,
                tool.metadata.as_ref(),
                tool.args().as_ref(),
                content_width,
            ),
            Self::Tool(tool) => render_live_tool_activity(
                &tool.name,
                tool.args().as_ref(),
                &tool.output,
                content_width,
                activity_phase,
                tool.state,
            ),
        }
    }

    fn render_transcript_with_activity(
        &self,
        screen_width: u16,
        content_width: usize,
        activity_phase: bool,
    ) -> String {
        match self {
            Self::Tool(tool) if !tool.visible => String::new(),
            Self::Tool(tool) => render_tool_transcript(ToolTranscriptInput {
                name: &tool.name,
                state: tool.state,
                exit_code: tool.exit_code,
                output: &tool.output,
                metadata: tool.metadata.as_ref(),
                args: tool.args().as_ref(),
                duration: tool.duration,
                width: content_width,
            }),
            Self::Subagent(subagent) if !subagent.visible => String::new(),
            Self::Subagent(subagent) => render_subagent_result(subagent, content_width, true),
            Self::Reasoning { source } => render_reasoning(source, content_width),
            _ => self.render_with_activity(screen_width, content_width, activity_phase),
        }
    }
}

fn render_reasoning(source: &str, width: usize) -> String {
    let source = sanitize_message_source(source);
    if width == 0 || source.trim().is_empty() {
        return String::new();
    }
    let bullet = message_marker(MessageTone::Reasoning);
    let title = message_title("Reasoning", false);
    let mut rows = vec![format!("{bullet} {title}")];
    let mut first = true;
    for line in source.lines() {
        rows.extend(
            wrap_words(line, width.saturating_sub(4).max(1))
                .into_iter()
                .map(|line| {
                    let prefix = message_branch(if first {
                        MessageBranch::Last
                    } else {
                        MessageBranch::Indent
                    });
                    first = false;
                    format!(
                        "{prefix}{}",
                        Style::new().fg(TN_GRAY).italic().render(&line)
                    )
                }),
        );
    }
    rows.into_iter()
        .map(|line| truncate_visible(&line, width))
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_subagent_result(
    subagent: &SubagentTranscriptEntry,
    width: usize,
    full_output: bool,
) -> String {
    if width == 0 {
        return String::new();
    }
    let status = match subagent.outcome {
        SubagentOutcome::Succeeded => "completed",
        SubagentOutcome::Failed => "failed",
        SubagentOutcome::Cancelled => "cancelled",
        SubagentOutcome::TrackingLost => "tracking lost",
    };
    let title = message_title(&format!("Agent {status}"), false);
    let bullet = message_marker(subagent_message_tone(subagent.outcome));
    let agent_name = sanitize_message_source(&subagent.agent);
    let agent = Style::new().fg(TN_GRAY).bold().render(&agent_name);
    let separator = Style::new().fg(super::TN_SUBTLE).render("·");
    let task_id = sanitize_message_source(&subagent.task_id);
    let id = full_output.then(|| {
        Style::new()
            .fg(super::TN_SUBTLE)
            .render(&format!("({task_id})"))
    });
    let header = match id {
        Some(id) => format!("{bullet} {title} {separator} {agent} {id}"),
        None => format!("{bullet} {title} {separator} {agent}"),
    };
    let mut rows = vec![truncate_visible(&header, width)];

    let task_source = sanitize_message_source(&subagent.task);
    let output_source = sanitize_message_source(&subagent.output);
    let task = task_source.trim();
    let output = output_source.trim();
    if !task.is_empty() {
        for (index, line) in wrap_words(task, width.saturating_sub(4).max(1))
            .into_iter()
            .enumerate()
        {
            let branch = match (index, output.is_empty()) {
                (0, false) => MessageBranch::Fork,
                (0, true) => MessageBranch::Last,
                (_, false) => MessageBranch::Pipe,
                (_, true) => MessageBranch::Indent,
            };
            rows.push(format!(
                "{}{}",
                message_branch(branch),
                Style::new().fg(TN_FG).render(&line)
            ));
        }
    }

    if !output.is_empty() {
        let mut output_rows = output
            .lines()
            .flat_map(|line| wrap_words(line, width.saturating_sub(4).max(1)))
            .collect::<Vec<_>>();
        let omitted = if full_output || output_rows.len() <= 8 {
            0
        } else {
            let omitted = output_rows.len() - 8;
            output_rows.truncate(8);
            omitted
        };
        rows.extend(output_rows.into_iter().enumerate().map(|(index, line)| {
            format!(
                "{}{}",
                message_branch(if index == 0 {
                    MessageBranch::Last
                } else {
                    MessageBranch::Indent
                }),
                Style::new().fg(TN_GRAY).render(&line)
            )
        }));
        if omitted > 0 {
            rows.push(truncate_visible(
                &format!(
                    "{}{}",
                    message_branch(MessageBranch::Indent),
                    Style::new()
                        .fg(TN_GRAY)
                        .render(&format!("… +{omitted} lines · Ctrl+T"))
                ),
                width,
            ));
        }
    }
    rows.join("\n")
}

impl Transcript {
    pub(crate) fn from_entries(entries: Vec<TranscriptEntry>) -> Self {
        let mut transcript = Self::default();
        transcript.extend(entries);
        transcript
    }

    pub(crate) fn into_entries(self) -> Vec<TranscriptEntry> {
        self.entries
            .into_iter()
            .map(|stored| stored.entry)
            .collect()
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    /// Return the newest committed assistant Markdown without terminal
    /// rendering, wrapping, or transcript chrome.
    pub(crate) fn latest_assistant_markdown(&self, live_assistant: Option<&str>) -> Option<String> {
        if let Some(source) = live_assistant
            .map(sanitize_export_source)
            .filter(|source| !source.trim().is_empty())
        {
            return Some(source);
        }
        self.entries.iter().rev().find_map(|stored| {
            let TranscriptEntry::AssistantMarkdown { source } = &stored.entry else {
                return None;
            };
            let source = sanitize_export_source(source);
            (!source.trim().is_empty()).then_some(source)
        })
    }

    /// Build a stable, shareable Markdown projection from semantic conversation
    /// entries. Transient UI notices, terminal-width-dependent preformatted
    /// rows, and private reasoning are deliberately excluded.
    pub(crate) fn semantic_markdown(&self, live_assistant: Option<&str>) -> Option<String> {
        let mut blocks = self
            .entries
            .iter()
            .filter_map(|stored| export_entry_markdown(&stored.entry))
            .collect::<Vec<_>>();
        if let Some(source) = live_assistant
            .map(sanitize_export_source)
            .filter(|source| !source.trim().is_empty())
        {
            blocks.push(role_markdown("Assistant", &source));
        }
        (!blocks.is_empty()).then(|| format!("{}\n", blocks.join("\n\n")))
    }

    /// Drop provisional entries appended after an LLM attempt began. Tool
    /// positions and cached layout are rebuilt so a replacement stream can
    /// reuse the same transcript surface without leaving stale call drafts.
    pub(crate) fn truncate(&mut self, len: usize) {
        if len >= self.entries.len() {
            return;
        }
        self.entries.truncate(len);
        self.rebuild_tool_positions();
        self.latest_input_tool_id = None;
        self.layout.clear();
        self.selection_rows.clear();
    }

    pub(crate) fn clear(&mut self) {
        self.entries.clear();
        self.tool_positions.clear();
        self.latest_input_tool_id = None;
        self.layout.clear();
        self.selection_rows.clear();
    }

    pub(crate) fn push(&mut self, entry: TranscriptEntry) {
        self.push_tracked(entry);
    }

    pub(crate) fn push_tracked(&mut self, entry: TranscriptEntry) -> TranscriptEntryId {
        let id = TranscriptEntryId(self.next_id);
        self.next_id = self.next_id.wrapping_add(1);
        if let TranscriptEntry::Tool(tool) = &entry {
            if let Some(call_id) = &tool.call_id {
                self.tool_positions
                    .insert(call_id.clone(), self.entries.len());
            }
        }
        self.entries.push(StoredTranscriptEntry {
            id,
            revision: 0,
            entry,
            render_cache: None,
        });
        id
    }

    /// Replace a tracked notice without assuming it is still the last entry.
    /// Streaming finalization and tool settlement may append other semantic
    /// entries while the asynchronous operation is completing.
    pub(crate) fn replace_preformatted(
        &mut self,
        id: TranscriptEntryId,
        value: impl Into<String>,
    ) -> bool {
        let Some(stored) = self.entries.iter_mut().find(|stored| stored.id == id) else {
            return false;
        };
        let TranscriptEntry::Preformatted(current) = &mut stored.entry else {
            return false;
        };
        *current = value.into();
        stored.revision = stored.revision.wrapping_add(1);
        stored.render_cache = None;
        self.layout.clear();
        self.selection_rows.clear();
        true
    }

    pub(crate) fn extend(&mut self, entries: impl IntoIterator<Item = TranscriptEntry>) {
        for entry in entries {
            self.push(entry);
        }
    }

    #[cfg(test)]
    pub(crate) fn iter(&self) -> impl DoubleEndedIterator<Item = &TranscriptEntry> {
        self.entries.iter().map(|stored| &stored.entry)
    }

    pub(crate) fn start_tool(&mut self, call_id: String, name: String, visible: bool) {
        let index = self.ensure_tool(call_id.clone(), name, visible);
        self.mutate_tool_at(index, |tool| {
            if !tool.state.is_terminal() {
                tool.state = ToolCallState::Preparing;
                tool.started_at.get_or_insert_with(Instant::now);
            }
            tool.visible |= visible;
        });
        self.latest_input_tool_id = Some(call_id);
    }

    pub(crate) fn push_tool_input(&mut self, call_id: Option<&str>, delta: &str) -> bool {
        let call_id = call_id
            .map(str::to_string)
            .or_else(|| self.latest_input_tool_id.clone());
        let Some(index) = call_id
            .as_deref()
            .and_then(|id| self.tool_positions.get(id).copied())
        else {
            return false;
        };
        self.mutate_tool_at(index, |tool| tool.args_json.push_str(delta));
        true
    }

    pub(crate) fn await_tool_approval(
        &mut self,
        call_id: String,
        name: String,
        args: serde_json::Value,
    ) {
        let index = self.ensure_tool(call_id.clone(), name, true);
        self.mutate_tool_at(index, |tool| {
            tool.args = Some(args);
            tool.state = ToolCallState::AwaitingApproval;
            tool.started_at.get_or_insert_with(Instant::now);
            tool.visible = true;
        });
        self.latest_input_tool_id = Some(call_id);
    }

    pub(crate) fn start_tool_execution(
        &mut self,
        call_id: String,
        name: String,
        args: serde_json::Value,
        visible: bool,
    ) {
        self.start_tool_execution_inner(call_id, name, args, visible, true);
    }

    /// Restore a persisted tool call whose original wall-clock timing was not
    /// stored. Keeping `started_at` empty makes Ctrl+T omit elapsed time rather
    /// than inventing a near-zero duration during replay.
    pub(crate) fn restore_tool_execution(
        &mut self,
        call_id: String,
        name: String,
        args: serde_json::Value,
        visible: bool,
    ) {
        self.start_tool_execution_inner(call_id, name, args, visible, false);
    }

    fn start_tool_execution_inner(
        &mut self,
        call_id: String,
        name: String,
        args: serde_json::Value,
        visible: bool,
        track_duration: bool,
    ) {
        let index = self.ensure_tool(call_id.clone(), name, visible);
        self.mutate_tool_at(index, |tool| {
            tool.args = Some(args);
            tool.state = ToolCallState::Running;
            if track_duration {
                tool.started_at.get_or_insert_with(Instant::now);
            }
            tool.visible |= visible;
        });
        self.latest_input_tool_id = Some(call_id);
    }

    pub(crate) fn push_tool_output(
        &mut self,
        call_id: &str,
        name: String,
        delta: &str,
        visible: bool,
    ) {
        let index = self.ensure_tool(call_id.to_string(), name, visible);
        self.mutate_tool_at(index, |tool| {
            tool.output.push_str(delta);
            tool.visible |= visible;
            if !tool.state.is_terminal() {
                tool.state = ToolCallState::Running;
                tool.started_at.get_or_insert_with(Instant::now);
            }
        });
    }

    #[cfg(test)]
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish_tool(
        &mut self,
        call_id: &str,
        name: String,
        args: Option<serde_json::Value>,
        output: String,
        exit_code: i32,
        metadata: Option<serde_json::Value>,
        visible: bool,
    ) -> Option<serde_json::Value> {
        let state = if exit_code == 0 {
            ToolCallState::Succeeded
        } else {
            ToolCallState::Failed
        };
        self.finish_tool_with_state(
            call_id, name, args, output, exit_code, metadata, state, visible,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish_tool_with_state(
        &mut self,
        call_id: &str,
        name: String,
        args: Option<serde_json::Value>,
        output: String,
        exit_code: i32,
        metadata: Option<serde_json::Value>,
        state: ToolCallState,
        visible: bool,
    ) -> Option<serde_json::Value> {
        let index = self.ensure_tool(call_id.to_string(), name, visible);
        self.mutate_tool_at(index, |tool| {
            if args.is_some() {
                tool.args = args;
            }
            tool.metadata = metadata;
            let protected_state = matches!(
                tool.state,
                ToolCallState::Denied | ToolCallState::TimedOut | ToolCallState::Interrupted
            );
            if !protected_state {
                tool.output = output;
                tool.exit_code = Some(exit_code);
                tool.state = state;
                if tool.duration.is_none() {
                    tool.duration = tool.started_at.map(|started| started.elapsed());
                }
            } else if tool.output.trim().is_empty() {
                tool.output = output;
            }
            tool.visible |= visible;
        });
        if self.latest_input_tool_id.as_deref() == Some(call_id) {
            self.latest_input_tool_id = None;
        }
        self.tool_at(index).and_then(ToolTranscriptEntry::args)
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub(crate) fn finish_subagent(
        &mut self,
        task_id: String,
        agent: String,
        task: String,
        success: bool,
        output: String,
        visible: bool,
    ) {
        let outcome = if success {
            SubagentOutcome::Succeeded
        } else {
            SubagentOutcome::Failed
        };
        self.finish_subagent_with_outcome(task_id, agent, task, outcome, output, visible);
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn finish_subagent_with_outcome(
        &mut self,
        task_id: String,
        agent: String,
        task: String,
        outcome: SubagentOutcome,
        output: String,
        visible: bool,
    ) {
        if let Some(index) = self.entries.iter().position(|stored| {
            matches!(
                &stored.entry,
                TranscriptEntry::Subagent(subagent) if subagent.task_id == task_id
            )
        }) {
            let Some(stored) = self.entries.get_mut(index) else {
                return;
            };
            let TranscriptEntry::Subagent(subagent) = &mut stored.entry else {
                return;
            };
            subagent.agent = agent;
            if !task.trim().is_empty() {
                subagent.task = task;
            }
            // Cancellation is authoritative when a late generic failure for
            // the same task arrives from another completion channel. Preserve
            // its explanatory output as well, otherwise the card would say
            // "cancelled" while displaying a contradictory failure message.
            let preserve_cancelled = subagent.outcome == SubagentOutcome::Cancelled
                && outcome != SubagentOutcome::Cancelled;
            if !preserve_cancelled {
                subagent.outcome = outcome;
                subagent.output = output;
            } else if subagent.output.trim().is_empty() {
                subagent.output = output;
            }
            subagent.visible |= visible;
            stored.revision = stored.revision.wrapping_add(1);
            stored.render_cache = None;
            return;
        }
        self.push(TranscriptEntry::Subagent(SubagentTranscriptEntry {
            task_id,
            agent,
            task,
            outcome,
            output,
            visible,
        }));
    }

    pub(crate) fn discard_tool(&mut self, call_id: &str) -> bool {
        let Some(index) = self.tool_positions.remove(call_id) else {
            return false;
        };
        self.entries.remove(index);
        self.rebuild_tool_positions();
        if self.latest_input_tool_id.as_deref() == Some(call_id) {
            self.latest_input_tool_id = None;
        }
        true
    }

    pub(crate) fn interrupt_unfinished_tools(&mut self) {
        let indices = self
            .entries
            .iter()
            .enumerate()
            .filter_map(|(index, stored)| match &stored.entry {
                TranscriptEntry::Tool(tool) if !tool.state.is_terminal() => Some(index),
                _ => None,
            })
            .collect::<Vec<_>>();
        for index in indices {
            self.mutate_tool_at(index, |tool| {
                tool.state = ToolCallState::Interrupted;
                tool.exit_code = Some(130);
                if tool.duration.is_none() {
                    tool.duration = tool.started_at.map(|started| started.elapsed());
                }
                tool.visible = true;
                if tool.output.trim().is_empty() {
                    tool.output = "Interrupted before the tool call completed.".to_string();
                }
            });
        }
        self.latest_input_tool_id = None;
    }

    #[cfg(test)]
    pub(crate) fn render(&mut self, screen_width: u16, content_width: usize) -> Vec<String> {
        self.render_with_activity(screen_width, content_width, true)
    }

    pub(crate) fn render_with_activity(
        &mut self,
        screen_width: u16,
        content_width: usize,
        activity_phase: bool,
    ) -> Vec<String> {
        let mut blocks = Vec::new();
        let mut layout = Vec::new();
        let mut next_block_row = 0usize;
        let mut index = 0usize;
        while index < self.entries.len() {
            if self.is_groupable_explore(index) {
                let start = index;
                index += 1;
                while index < self.entries.len() && self.is_groupable_explore(index) {
                    index += 1;
                }
                let tools = self.entries[start..index]
                    .iter()
                    .filter_map(|stored| match &stored.entry {
                        TranscriptEntry::Tool(tool) => Some(tool),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                let block = normalize_transcript_block(render_explore_group(
                    &tools,
                    content_width,
                    activity_phase,
                ));
                if !block.is_empty() {
                    let (start_row, row_count) =
                        append_rendered_block(&mut blocks, &mut next_block_row, block);
                    for stored in &self.entries[start..index] {
                        layout.push(LayoutSpan {
                            entry_id: stored.id,
                            start_row,
                            row_count,
                        });
                    }
                }
                continue;
            }

            let block = normalize_transcript_block(self.render_entry(
                index,
                screen_width,
                content_width,
                activity_phase,
            ));
            if !block.is_empty() {
                let (start_row, row_count) =
                    append_rendered_block(&mut blocks, &mut next_block_row, block);
                layout.push(LayoutSpan {
                    entry_id: self.entries[index].id,
                    start_row,
                    row_count,
                });
            }
            index += 1;
        }
        self.layout = layout;
        self.selection_rows = join_transcript_blocks(&blocks)
            .split('\n')
            .map(strip_ansi)
            .collect();
        blocks
    }

    /// Render every semantic entry for Ctrl+T without the compact-history
    /// grouping or output bounds. This deliberately does not mutate the main
    /// viewport layout/anchor cache.
    pub(crate) fn render_transcript_with_activity(
        &self,
        screen_width: u16,
        content_width: usize,
        activity_phase: bool,
    ) -> Vec<String> {
        self.entries
            .iter()
            .map(|stored| {
                normalize_transcript_block(stored.entry.render_transcript_with_activity(
                    screen_width,
                    content_width,
                    activity_phase,
                ))
            })
            .filter(|block| !block.is_empty())
            .collect()
    }

    pub(crate) fn anchor_for_row(&self, row: usize) -> Option<TranscriptAnchor> {
        // The viewport also contains the provisional streaming response, which
        // is intentionally absent from this completed-transcript layout. Do
        // not let an offset in that live tail fall back to the last history
        // entry or every stream repaint will snap manual scrolling backwards.
        if row >= self.selection_rows.len() {
            return None;
        }
        let span = self
            .layout
            .iter()
            .find(|span| row >= span.start_row && row < span.start_row + span.row_count)
            .or_else(|| self.layout.iter().rev().find(|span| span.start_row <= row))?;
        Some(TranscriptAnchor {
            entry_id: span.entry_id,
            row_in_entry: row
                .saturating_sub(span.start_row)
                .min(span.row_count.saturating_sub(1)),
        })
    }

    pub(crate) fn row_for_anchor(&self, anchor: TranscriptAnchor) -> Option<usize> {
        let span = self
            .layout
            .iter()
            .find(|span| span.entry_id == anchor.entry_id)?;
        Some(span.start_row + anchor.row_in_entry.min(span.row_count.saturating_sub(1)))
    }

    /// Resolve a rendered transcript cell into a stable semantic endpoint.
    pub(crate) fn point_for_cell(&self, row: usize, col: usize) -> Option<TranscriptPoint> {
        let span = self
            .layout
            .iter()
            .find(|span| row >= span.start_row && row < span.start_row + span.row_count)?;
        let rows = self.rows_for_span(span)?;
        let row_hint = row
            .saturating_sub(span.start_row)
            .min(rows.len().saturating_sub(1));
        Some(TranscriptPoint {
            entry_id: span.entry_id,
            semantic_offset: semantic_offset_for_cell(rows, row_hint, col),
            row_hint,
            col_hint: col,
        })
    }

    /// Project a stable endpoint into the current transcript render.
    pub(crate) fn cell_for_point(&self, point: TranscriptPoint) -> Option<(usize, usize)> {
        let span = self
            .layout
            .iter()
            .find(|span| span.entry_id == point.entry_id)?;
        let rows = self.rows_for_span(span)?;
        let (row, col) =
            cell_for_semantic_offset(rows, point.semantic_offset, point.row_hint, point.col_hint);
        Some((span.start_row.saturating_add(row), col))
    }

    /// Copy the complete semantic selection, including rows outside the
    /// currently visible viewport.
    pub(crate) fn selected_text(&self, selection: TranscriptSelection) -> Option<String> {
        let (anchor_row, anchor_col) = self.cell_for_point(selection.anchor())?;
        let (head_row, head_col) = self.cell_for_point(selection.head())?;
        let range = SelectionRange::from_cells(anchor_row, anchor_col, head_row, head_col);
        Some(selected_text_range(&self.selection_rows.join("\n"), range))
    }

    fn rows_for_span(&self, span: &LayoutSpan) -> Option<&[String]> {
        let end = span
            .start_row
            .saturating_add(span.row_count)
            .min(self.selection_rows.len());
        (span.start_row < end).then_some(&self.selection_rows[span.start_row..end])
    }

    fn ensure_tool(&mut self, call_id: String, name: String, visible: bool) -> usize {
        if let Some(index) = self.tool_positions.get(&call_id).copied() {
            return index;
        }
        let index = self.entries.len();
        self.push(TranscriptEntry::Tool(ToolTranscriptEntry {
            call_id: Some(call_id.clone()),
            name,
            state: ToolCallState::Preparing,
            args_json: String::new(),
            args: None,
            output: String::new(),
            metadata: None,
            exit_code: None,
            started_at: None,
            duration: None,
            visible,
        }));
        self.tool_positions.insert(call_id, index);
        index
    }

    fn mutate_tool_at(&mut self, index: usize, mutate: impl FnOnce(&mut ToolTranscriptEntry)) {
        let Some(stored) = self.entries.get_mut(index) else {
            return;
        };
        let TranscriptEntry::Tool(tool) = &mut stored.entry else {
            return;
        };
        mutate(tool);
        stored.revision = stored.revision.wrapping_add(1);
        stored.render_cache = None;
    }

    fn tool_at(&self, index: usize) -> Option<&ToolTranscriptEntry> {
        match &self.entries.get(index)?.entry {
            TranscriptEntry::Tool(tool) => Some(tool),
            _ => None,
        }
    }

    fn is_groupable_explore(&self, index: usize) -> bool {
        self.tool_at(index)
            .is_some_and(ToolTranscriptEntry::is_groupable_explore)
    }

    fn render_entry(
        &mut self,
        index: usize,
        screen_width: u16,
        content_width: usize,
        activity_phase: bool,
    ) -> String {
        let stored = &mut self.entries[index];
        let cache_phase = match &stored.entry {
            TranscriptEntry::Tool(tool) if !tool.state.is_terminal() => Some(activity_phase),
            _ => None,
        };
        if let Some(cache) = &stored.render_cache {
            if cache.revision == stored.revision
                && cache.screen_width == screen_width
                && cache.content_width == content_width
                && cache.activity_phase == cache_phase
            {
                return cache.block.clone();
            }
        }
        let block = stored
            .entry
            .render_with_activity(screen_width, content_width, activity_phase);
        stored.render_cache = Some(EntryRenderCache {
            revision: stored.revision,
            screen_width,
            content_width,
            activity_phase: cache_phase,
            block: block.clone(),
        });
        block
    }

    fn rebuild_tool_positions(&mut self) {
        self.tool_positions.clear();
        for (index, stored) in self.entries.iter().enumerate() {
            if let TranscriptEntry::Tool(tool) = &stored.entry {
                if let Some(call_id) = &tool.call_id {
                    self.tool_positions.insert(call_id.clone(), index);
                }
            }
        }
    }
}

fn export_entry_markdown(entry: &TranscriptEntry) -> Option<String> {
    match entry {
        TranscriptEntry::User { source } => {
            let source = sanitize_export_source(source);
            (!source.trim().is_empty()).then(|| role_markdown("User", &source))
        }
        TranscriptEntry::AssistantMarkdown { source } => {
            let source = sanitize_export_source(source);
            (!source.trim().is_empty()).then(|| role_markdown("Assistant", &source))
        }
        TranscriptEntry::Tool(tool) if tool.visible => Some(export_tool_markdown(tool)),
        TranscriptEntry::Subagent(subagent) if subagent.visible => {
            Some(export_subagent_markdown(subagent))
        }
        TranscriptEntry::Preformatted(_)
        | TranscriptEntry::Notice { .. }
        | TranscriptEntry::Reasoning { .. }
        | TranscriptEntry::Tool(_)
        | TranscriptEntry::Subagent(_) => None,
    }
}

fn role_markdown(role: &str, source: &str) -> String {
    format!("## {role}\n\n{}", source.trim_matches('\n'))
}

fn export_tool_markdown(tool: &ToolTranscriptEntry) -> String {
    let mut block = format!(
        "### Tool: {}\n\n- Status: {}",
        markdown_code_span(&tool.name),
        markdown_code_span(tool_state_label(tool.state))
    );
    if let Some(exit_code) = tool.exit_code {
        block.push_str(&format!(
            "\n- Exit code: {}",
            markdown_code_span(&exit_code.to_string())
        ));
    }
    if let Some(duration) = tool.duration {
        block.push_str(&format!(
            "\n- Duration: {}",
            markdown_code_span(&export_duration(duration))
        ));
    }

    if let Some(args) = tool.args() {
        let args = serde_json::to_string_pretty(&args).unwrap_or_else(|_| args.to_string());
        block.push_str("\n\n#### Arguments\n\n");
        block.push_str(&markdown_fence("json", &sanitize_export_source(&args)));
    } else {
        let args = sanitize_export_source(&tool.args_json);
        if !args.trim().is_empty() {
            block.push_str("\n\n#### Arguments\n\n");
            block.push_str(&markdown_fence("text", &args));
        }
    }

    let output = sanitize_export_source(&tool.output);
    if !output.trim().is_empty() {
        block.push_str("\n\n#### Output\n\n");
        block.push_str(&markdown_fence("text", &output));
    }

    if let Some(metadata) = tool.metadata.as_ref().filter(|value| !value.is_null()) {
        let metadata =
            serde_json::to_string_pretty(metadata).unwrap_or_else(|_| metadata.to_string());
        block.push_str("\n\n#### Metadata\n\n");
        block.push_str(&markdown_fence("json", &sanitize_export_source(&metadata)));
    }
    block
}

fn export_subagent_markdown(subagent: &SubagentTranscriptEntry) -> String {
    let status = match subagent.outcome {
        SubagentOutcome::Succeeded => "succeeded",
        SubagentOutcome::Failed => "failed",
        SubagentOutcome::Cancelled => "cancelled",
        SubagentOutcome::TrackingLost => "tracking-lost",
    };
    let mut block = format!(
        "### Delegated task: {}\n\n- ID: {}\n- Status: {}",
        markdown_code_span(&subagent.agent),
        markdown_code_span(&subagent.task_id),
        markdown_code_span(status)
    );
    let task = sanitize_export_source(&subagent.task);
    if !task.trim().is_empty() {
        block.push_str("\n\n#### Task\n\n");
        block.push_str(task.trim_matches('\n'));
    }
    let output = sanitize_export_source(&subagent.output);
    if !output.trim().is_empty() {
        block.push_str("\n\n#### Output\n\n");
        block.push_str(output.trim_matches('\n'));
    }
    block
}

fn tool_state_label(state: ToolCallState) -> &'static str {
    match state {
        ToolCallState::Preparing => "preparing",
        ToolCallState::AwaitingApproval => "awaiting-approval",
        ToolCallState::Running => "running",
        ToolCallState::Succeeded => "succeeded",
        ToolCallState::Failed => "failed",
        ToolCallState::Denied => "denied",
        ToolCallState::TimedOut => "timed-out",
        ToolCallState::Interrupted => "interrupted",
    }
}

fn export_duration(duration: Duration) -> String {
    if duration.as_secs() >= 60 {
        return format!(
            "{}m {:02}s",
            duration.as_secs() / 60,
            duration.as_secs() % 60
        );
    }
    if duration.as_secs() > 0 {
        return format!("{:.1}s", duration.as_secs_f64());
    }
    format!("{}ms", duration.as_millis())
}

fn markdown_fence(language: &str, source: &str) -> String {
    let fence = "`".repeat(longest_backtick_run(source).saturating_add(1).max(3));
    format!(
        "{fence}{language}\n{}\n{fence}",
        source.trim_end_matches('\n')
    )
}

fn markdown_code_span(source: &str) -> String {
    let source = sanitize_export_source(source).replace('\n', " ");
    let fence = "`".repeat(longest_backtick_run(&source).saturating_add(1).max(1));
    let padding = if source.starts_with('`')
        || source.starts_with(' ')
        || source.ends_with('`')
        || source.ends_with(' ')
    {
        " "
    } else {
        ""
    };
    format!("{fence}{padding}{source}{padding}{fence}")
}

fn longest_backtick_run(source: &str) -> usize {
    source
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or(0)
}

fn sanitize_export_source(source: &str) -> String {
    strip_ansi(source)
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .chars()
        .filter(|character| matches!(character, '\n' | '\t') || !character.is_control())
        .collect()
}

fn semantic_offset_for_cell(rows: &[String], target_row: usize, target_col: usize) -> usize {
    let mut offset = 0usize;
    for (row_index, row) in rows.iter().enumerate() {
        if row_index > target_row {
            break;
        }
        let mut display_col = 0usize;
        for character in row.chars() {
            if row_index == target_row && display_col >= target_col {
                return offset;
            }
            let width = visible_len(&character.to_string());
            if semantic_selection_character(character)
                && (row_index < target_row || display_col < target_col)
            {
                offset = offset.saturating_add(1);
            }
            display_col = display_col.saturating_add(width);
        }
        if row_index == target_row {
            break;
        }
    }
    offset
}

fn cell_for_semantic_offset(
    rows: &[String],
    target_offset: usize,
    row_hint: usize,
    col_hint: usize,
) -> (usize, usize) {
    let fallback_row = row_hint.min(rows.len().saturating_sub(1));
    let fallback_col = rows
        .get(fallback_row)
        .map_or(0, |row| col_hint.min(visible_len(row)));
    let mut offset = 0usize;
    let mut last_semantic_end = None;

    for (row_index, row) in rows.iter().enumerate() {
        let mut display_col = 0usize;
        for character in row.chars() {
            let width = visible_len(&character.to_string());
            if semantic_selection_character(character) {
                if offset == target_offset {
                    return (row_index, display_col);
                }
                offset = offset.saturating_add(1);
                last_semantic_end = Some((row_index, display_col.saturating_add(width)));
            }
            display_col = display_col.saturating_add(width);
        }
    }

    last_semantic_end.unwrap_or((fallback_row, fallback_col))
}

fn semantic_selection_character(character: char) -> bool {
    character.is_alphanumeric() || character == '_'
}

fn normalize_transcript_block(block: String) -> String {
    block.trim_matches('\n').to_string()
}

fn append_rendered_block(
    blocks: &mut Vec<String>,
    next_block_row: &mut usize,
    block: String,
) -> (usize, usize) {
    if !blocks.is_empty() {
        *next_block_row = next_block_row.saturating_add(TRANSCRIPT_BLOCK_GAP_ROWS);
    }
    let start_row = *next_block_row;
    let row_count = block.lines().count();
    *next_block_row = next_block_row.saturating_add(row_count);
    blocks.push(block);
    (start_row, row_count)
}

/// Join top-level transcript cells with exactly one real terminal row. The
/// visible space survives `str::lines`, viewport partitioning, and trailing-row
/// accounting, so messages, tools, notices, and prompts all share one rule.
pub(crate) fn join_transcript_blocks(blocks: &[String]) -> String {
    blocks
        .iter()
        .map(|block| block.trim_matches('\n'))
        .filter(|block| !block.is_empty())
        .collect::<Vec<_>>()
        .join(TRANSCRIPT_BLOCK_SEPARATOR)
}

pub(crate) fn transcript_block_separator() -> &'static str {
    TRANSCRIPT_BLOCK_SEPARATOR
}

#[derive(Debug)]
enum ExploreAction {
    Read(Vec<String>),
    Other(String),
}

fn render_explore_group(
    tools: &[&ToolTranscriptEntry],
    width: usize,
    activity_phase: bool,
) -> String {
    if tools.is_empty() || width == 0 {
        return String::new();
    }
    let mut actions = Vec::<ExploreAction>::new();
    for tool in tools {
        let args = tool.args().unwrap_or(serde_json::Value::Null);
        match tool.name.as_str() {
            "read" | "cat" => {
                let path = args
                    .get("file_path")
                    .or_else(|| args.get("path"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("file")
                    .to_string();
                match actions.last_mut() {
                    Some(ExploreAction::Read(paths)) => paths.push(path),
                    _ => actions.push(ExploreAction::Read(vec![path])),
                }
            }
            "grep" | "search" => {
                let query = args
                    .get("pattern")
                    .or_else(|| args.get("query"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or("pattern");
                let path = args
                    .get("path")
                    .or_else(|| args.get("file_path"))
                    .and_then(serde_json::Value::as_str);
                actions.push(ExploreAction::Other(match path {
                    Some(path) => format!("Search {query} in {path}"),
                    None => format!("Search {query}"),
                }));
            }
            "ls" | "glob" | "find" => {
                let detail = arg_summary_for_tool(&tool.name, &args).unwrap_or_default();
                actions.push(ExploreAction::Other(if detail.is_empty() {
                    "List files".to_string()
                } else {
                    format!("List {detail}")
                }));
            }
            _ => {}
        }
    }

    let live = tools.iter().any(|tool| !tool.state.is_terminal());
    let tone = if live {
        tool_message_tone(ToolCallState::Running, activity_phase)
    } else {
        MessageTone::Inactive
    };
    let bullet = message_marker(tone);
    let title = message_title(if live { "Exploring" } else { "Explored" }, false);
    let mut rows = vec![format!("{bullet} {title}")];
    for (action_index, action) in actions.into_iter().enumerate() {
        let text = match action {
            ExploreAction::Read(paths) => format!("Read {}", paths.join(", ")),
            ExploreAction::Other(text) => text,
        };
        let styled = highlight_explore_detail(&text);
        let wrapped = wrap_words(&styled, width.saturating_sub(4).max(1));
        for (line_index, line) in wrapped.into_iter().enumerate() {
            let prefix = message_branch(if action_index == 0 && line_index == 0 {
                MessageBranch::Last
            } else {
                MessageBranch::Indent
            });
            rows.push(format!("{prefix}{line}"));
        }
    }
    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::super::tool_style::{TOOL_ARGUMENT_COLOR, TOOL_PATH_COLOR};
    use super::super::ACCENT;
    use super::*;

    fn assert_bounded(rendered: &str, width: usize) {
        for line in rendered.lines() {
            assert!(
                a3s_tui::style::visible_len(line) <= width,
                "line exceeds width {width}: {:?}",
                a3s_tui::style::strip_ansi(line)
            );
        }
    }

    #[test]
    fn assistant_markdown_is_source_backed_across_resize() {
        let entry = TranscriptEntry::assistant_markdown(
            "A paragraph with **formatting** and enough words to wrap at narrow widths.",
        );
        let narrow = entry.render(28, 27);
        let wide = entry.render(80, 79);
        assert!(narrow.lines().count() > wide.lines().count());
        assert_bounded(&narrow, 27);
        assert_bounded(&wide, 79);
    }

    #[test]
    fn semantic_markdown_is_source_backed_and_excludes_private_or_transient_rows() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::preformatted("\x1b[31mtemporary spinner\x1b[0m"),
            TranscriptEntry::notice(NoticeKind::Info, "temporary notice"),
            TranscriptEntry::user("\x1b[2JReview **this**.\r\nKeep the source."),
            TranscriptEntry::reasoning("private chain of thought"),
            TranscriptEntry::assistant_markdown("Committed `answer`.\n"),
        ]);
        transcript.finish_tool(
            "visible-tool",
            "read".into(),
            Some(serde_json::json!({"file_path": "src/main.rs"})),
            "line one\n```danger\nline two".into(),
            0,
            Some(serde_json::json!({"kind": "read"})),
            true,
        );
        transcript.finish_tool(
            "hidden-tool",
            "internal".into(),
            None,
            "hidden output".into(),
            0,
            None,
            false,
        );
        transcript.finish_subagent(
            "task-1".into(),
            "review".into(),
            "Check the patch.".into(),
            true,
            "Looks good.".into(),
            true,
        );

        let markdown = transcript
            .semantic_markdown(Some("Live **tail**.\x1b]0;title\x07"))
            .expect("semantic Markdown");

        assert!(markdown.contains("## User\n\nReview **this**.\nKeep the source."));
        assert!(markdown.contains("## Assistant\n\nCommitted `answer`."));
        assert!(markdown.contains("### Tool: `read`"));
        assert!(markdown.contains("\"file_path\": \"src/main.rs\""));
        assert!(markdown.contains("````text\nline one\n```danger\nline two\n````"));
        assert!(markdown.contains("#### Metadata"));
        assert!(markdown.contains("### Delegated task: `review`"));
        assert!(markdown.contains("## Assistant\n\nLive **tail**."));
        assert!(!markdown.contains("temporary spinner"));
        assert!(!markdown.contains("temporary notice"));
        assert!(!markdown.contains("private chain of thought"));
        assert!(!markdown.contains("hidden output"));
        assert!(!markdown.contains("\x1b"));
        assert!(markdown.ends_with('\n'));
    }

    #[test]
    fn latest_assistant_markdown_returns_sanitized_raw_source() {
        let transcript = Transcript::from_entries(vec![
            TranscriptEntry::assistant_markdown("first"),
            TranscriptEntry::user("next"),
            TranscriptEntry::assistant_markdown("\x1b[31mlatest\tanswer\x1b[0m"),
        ]);

        assert_eq!(
            transcript.latest_assistant_markdown(None).as_deref(),
            Some("latest\tanswer")
        );
        assert_eq!(
            transcript
                .latest_assistant_markdown(Some("\x1b[32mlive\x1b[0m"))
                .as_deref(),
            Some("live")
        );
    }

    #[test]
    fn compositor_gives_user_and_assistant_cells_one_stable_gap_at_product_widths() {
        for width in [24_u16, 48, 80] {
            let mut transcript = Transcript::from_entries(vec![
                TranscriptEntry::user("Review the message hierarchy."),
                TranscriptEntry::assistant_markdown("The hierarchy is now calmer."),
            ]);
            let compact = transcript.render(width, width as usize);
            let complete = transcript.render_transcript_with_activity(width, width as usize, true);

            for (surface, blocks) in [("compact", compact), ("Ctrl+T", complete)] {
                assert_eq!(blocks.len(), 2, "{surface} width {width}");
                let joined = join_transcript_blocks(&blocks);
                let rendered = a3s_tui::style::strip_ansi(&joined);
                let rows = rendered.lines().collect::<Vec<_>>();
                let assistant_row = rows
                    .iter()
                    .position(|row| row.contains("The hierarchy"))
                    .expect("assistant row");
                assert!(
                    rendered
                        .split_whitespace()
                        .collect::<Vec<_>>()
                        .join(" ")
                        .contains("The hierarchy is now calmer."),
                    "{surface} width {width}: {rendered:?}"
                );

                assert!(rows.first().is_some_and(|row| row.trim().is_empty()));
                assert!(rows.iter().any(|row| row.starts_with("› Review")));
                assert!(rows[assistant_row - 1].trim().is_empty());
                // The other empty row is the user surface's own bottom inset;
                // the neutral compositor gap is always the row immediately
                // before the next semantic cell.
                assert!(rows[assistant_row - 2].trim().is_empty());
                assert_eq!(
                    rows.iter().filter(|row| row.trim().is_empty()).count(),
                    3,
                    "{surface} width {width}: {rendered:?}"
                );
                assert!(joined
                    .lines()
                    .take(1)
                    .all(|row| row
                        .contains(&format!("\x1b[{}m", super::super::SURFACE_USER.bg_ansi()))));
                assert!(
                    !rendered.contains("\n\n\n"),
                    "{surface} width {width}: {rendered:?}"
                );
                assert_bounded(&blocks[0], width as usize);
                assert_bounded(&blocks[1], width as usize);
            }
        }
    }

    #[test]
    fn every_top_level_cell_kind_uses_the_same_single_gap_row() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::preformatted("temporary status"),
            TranscriptEntry::notice(NoticeKind::Warning, "Check the active request."),
            TranscriptEntry::user("Run the verification."),
            TranscriptEntry::assistant_markdown("Starting now."),
            TranscriptEntry::reasoning("Inspect the complete transcript."),
        ]);
        for (id, command) in [("tool-one", "cargo check"), ("tool-two", "cargo test")] {
            let args = serde_json::json!({"command": command});
            transcript.start_tool_execution(id.into(), "bash".into(), args.clone(), true);
            transcript.finish_tool(
                id,
                "bash".into(),
                Some(args),
                "completed".into(),
                0,
                None,
                true,
            );
        }
        transcript.finish_subagent(
            "child-one".into(),
            "reviewer".into(),
            "Review the result.".into(),
            true,
            "Looks good.".into(),
            true,
        );

        let compact = transcript.render(80, 79);
        let compact_layout = transcript.layout.clone();
        let complete = transcript.render_transcript_with_activity(80, 79, true);

        for (surface, blocks, spans, expected_blocks) in [
            ("compact", compact, Some(compact_layout), 7),
            ("Ctrl+T", complete, None, 8),
        ] {
            assert_eq!(blocks.len(), expected_blocks, "{surface}");
            let joined = a3s_tui::style::strip_ansi(&join_transcript_blocks(&blocks));
            let rows = joined.split('\n').collect::<Vec<_>>();
            let mut next_row = 0usize;
            for (index, block) in blocks.iter().enumerate() {
                let row_count = block.lines().count();
                next_row += row_count;
                if index + 1 < blocks.len() {
                    assert_eq!(
                        rows[next_row], " ",
                        "{surface} gap after block {index}: {joined:?}"
                    );
                    next_row += TRANSCRIPT_BLOCK_GAP_ROWS;
                }
            }
            assert_eq!(next_row, rows.len(), "{surface}");

            if let Some(mut spans) = spans {
                spans.sort_by_key(|span| span.start_row);
                assert_eq!(spans.len(), blocks.len(), "{surface}");
                for pair in spans.windows(2) {
                    assert_eq!(
                        pair[1].start_row,
                        pair[0].start_row + pair[0].row_count + TRANSCRIPT_BLOCK_GAP_ROWS,
                        "{surface} layout must account for the visible separator"
                    );
                }
            }
        }
    }

    #[test]
    fn truncate_removes_provisional_stream_attempt_entries_and_tool_indexes() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::user(
            "Keep this user message exactly once.",
        )]);
        let checkpoint = transcript.len();
        transcript.push(TranscriptEntry::assistant_markdown(
            "discarded partial answer",
        ));
        transcript.start_tool("partial-tool".into(), "bash".into(), true);
        assert!(transcript.push_tool_input(Some("partial-tool"), r#"{"command":"car"#));

        transcript.truncate(checkpoint);

        assert_eq!(transcript.len(), 1);
        assert!(matches!(
            transcript.iter().next(),
            Some(TranscriptEntry::User { source }) if source == "Keep this user message exactly once."
        ));
        assert!(!transcript.push_tool_input(Some("partial-tool"), "go"));
    }

    #[test]
    fn completed_reasoning_is_hidden_from_history_but_retained_for_ctrl_t() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::reasoning(
            "Inspect the event ordering, then preserve the semantic boundary.",
        )]);

        assert!(transcript.render(80, 79).is_empty());
        let complete = transcript.render_transcript_with_activity(80, 79, true);
        assert_eq!(complete.len(), 1);
        let plain = a3s_tui::style::strip_ansi(&complete[0]);
        let rows = plain.lines().collect::<Vec<_>>();
        assert!(rows.iter().all(|row| !row.trim().is_empty()), "{plain}");
        assert!(plain.contains("• Reasoning"), "{plain}");
        assert!(plain.contains("  └ Inspect the event ordering"), "{plain}");
        assert!(plain.contains("Inspect the event ordering"), "{plain}");
        assert!(complete[0].contains(&message_marker(MessageTone::Reasoning)));
        assert_bounded(&complete[0], 79);
    }

    #[test]
    fn background_subagent_result_is_durable_and_ctrl_t_keeps_full_output() {
        let mut transcript = Transcript::default();
        let output = (0..12)
            .map(|index| format!("result line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        transcript.finish_subagent(
            "task-bg".into(),
            "review".into(),
            "audit the implementation".into(),
            true,
            output,
            true,
        );

        let compact_rendered = transcript.render(80, 79).join("\n");
        let compact = a3s_tui::style::strip_ansi(&compact_rendered);
        assert!(compact.contains("Agent completed · review"), "{compact}");
        assert!(!compact.contains("(task-bg)"), "{compact}");
        assert!(
            compact_rendered.contains(&message_marker(MessageTone::Success)),
            "{compact_rendered:?}"
        );
        assert!(
            compact_rendered.contains(&message_title("Agent completed", false)),
            "{compact_rendered:?}"
        );
        assert!(
            compact_rendered.contains(&Style::new().fg(TN_GRAY).bold().render("review")),
            "{compact_rendered:?}"
        );
        assert!(
            !compact_rendered.contains(&Style::new().fg(TN_CYAN).bold().render("review")),
            "agent identity should not compete with the outcome marker: {compact_rendered:?}"
        );
        assert!(compact.contains("audit the implementation"), "{compact}");
        assert!(
            compact.contains("  ├ audit the implementation"),
            "{compact}"
        );
        assert!(compact.contains("  └ result line 0"), "{compact}");
        assert!(!compact.contains("Output:"), "{compact}");
        assert!(compact.contains("result line 0"), "{compact}");
        assert!(compact.contains("… +4 lines"), "{compact}");
        assert!(!compact.contains("result line 11"), "{compact}");

        let complete_rendered = transcript
            .render_transcript_with_activity(80, 79, true)
            .join("\n");
        let complete = a3s_tui::style::strip_ansi(&complete_rendered);
        assert!(complete.contains("(task-bg)"), "{complete}");
        assert!(complete.contains("result line 11"), "{complete}");
        assert!(!complete.contains("… +"), "{complete}");
    }

    #[test]
    fn semantic_messages_strip_untrusted_terminal_controls_before_styling() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::user("\x1b[2Juser\0 message"),
            TranscriptEntry::assistant_markdown("\x1b]0;title\x07assistant **message**"),
            TranscriptEntry::reasoning("\x1b[31mreasoning\x1b[0m\0 message"),
        ]);
        transcript.finish_subagent(
            "\x1b[2Jtask-id".into(),
            "\x1b[31mreview\x1b[0m".into(),
            "audit\0 task".into(),
            true,
            "safe\x1b]0;title\x07 output".into(),
            true,
        );

        let compact = transcript.render(80, 79).join("\n");
        let complete = transcript
            .render_transcript_with_activity(80, 79, true)
            .join("\n");
        for rendered in [&compact, &complete] {
            assert!(!rendered.contains("\x1b[2J"), "{rendered:?}");
            assert!(!rendered.contains("\x1b]0;title"), "{rendered:?}");
            assert!(!rendered.contains('\0'), "{rendered:?}");
        }
        let compact_plain = a3s_tui::style::strip_ansi(&compact);
        let complete_plain = a3s_tui::style::strip_ansi(&complete);
        assert!(compact_plain.contains("user message"), "{compact_plain}");
        assert!(
            compact_plain.contains("assistant message"),
            "{compact_plain}"
        );
        assert!(compact_plain.contains("audit task"), "{compact_plain}");
        assert!(
            complete_plain.contains("reasoning message"),
            "{complete_plain}"
        );
        assert!(complete_plain.contains("(task-id)"), "{complete_plain}");
    }

    #[test]
    fn foreground_subagent_semantic_cell_stays_hidden_behind_parent_result() {
        let mut transcript = Transcript::default();
        transcript.finish_subagent(
            "task-fg".into(),
            "explore".into(),
            "inspect".into(),
            true,
            "same output as parent task".into(),
            false,
        );

        assert_eq!(transcript.iter().count(), 1);
        assert!(transcript.render(80, 79).is_empty());
        assert!(transcript
            .render_transcript_with_activity(80, 79, true)
            .is_empty());
    }

    #[test]
    fn duplicate_subagent_terminal_delivery_updates_one_durable_cell() {
        let mut transcript = Transcript::default();
        for output in ["event output", "tracker output"] {
            transcript.finish_subagent(
                "task-bg".into(),
                "review".into(),
                "audit".into(),
                true,
                output.into(),
                true,
            );
        }

        assert_eq!(transcript.iter().count(), 1);
        let plain = a3s_tui::style::strip_ansi(&transcript.render(80, 79).join("\n"));
        assert!(plain.contains("tracker output"), "{plain}");
        assert!(!plain.contains("event output"), "{plain}");
    }

    #[test]
    fn cancelled_subagent_resists_a_late_generic_failure() {
        let mut transcript = Transcript::default();
        transcript.finish_subagent_with_outcome(
            "task-bg".into(),
            "review".into(),
            "audit".into(),
            SubagentOutcome::Cancelled,
            "Stopped by user.".into(),
            true,
        );
        transcript.finish_subagent_with_outcome(
            "task-bg".into(),
            "review".into(),
            "audit".into(),
            SubagentOutcome::Failed,
            "Late watcher failure.".into(),
            true,
        );

        let plain = a3s_tui::style::strip_ansi(&transcript.render(80, 79).join("\n"));
        assert!(plain.contains("Agent cancelled · review"), "{plain}");
        assert!(plain.contains("Stopped by user."), "{plain}");
        assert!(!plain.contains("Agent failed"), "{plain}");
        assert!(!plain.contains("Late watcher failure."), "{plain}");
    }

    #[test]
    fn tool_entry_reflows_from_semantic_fields() {
        let entry = TranscriptEntry::tool(
            "bash",
            0,
            "first output line\nsecond output line",
            None,
            Some(serde_json::json!({
                "command": "cargo test a-very-long-filter-name -- --nocapture"
            })),
        );
        assert_bounded(&entry.render(36, 35), 35);
        assert_bounded(&entry.render(80, 79), 79);
    }

    #[test]
    fn tool_completion_updates_start_position_not_completion_order() {
        let mut transcript = Transcript::default();
        transcript.push(TranscriptEntry::assistant_markdown("before"));
        transcript.start_tool("t1".into(), "bash".into(), true);
        transcript.start_tool("t2".into(), "grep".into(), true);
        transcript.finish_tool(
            "t2",
            "grep".into(),
            Some(serde_json::json!({"pattern": "TODO"})),
            "match".into(),
            0,
            None,
            true,
        );
        transcript.finish_tool(
            "t1",
            "bash".into(),
            Some(serde_json::json!({"command": "echo ok"})),
            "ok".into(),
            0,
            None,
            true,
        );
        transcript.push(TranscriptEntry::assistant_markdown("after"));

        let kinds = transcript
            .iter()
            .map(|entry| match entry {
                TranscriptEntry::AssistantMarkdown { source } => source.clone(),
                TranscriptEntry::Tool(tool) => tool.call_id.clone().unwrap(),
                _ => "other".to_string(),
            })
            .collect::<Vec<_>>();
        assert_eq!(kinds, ["before", "t1", "t2", "after"]);
    }

    #[test]
    fn authoritative_end_args_render_without_streamed_input() {
        let mut transcript = Transcript::default();
        transcript.start_tool("t1".into(), "bash".into(), true);
        transcript.finish_tool(
            "t1",
            "bash".into(),
            Some(serde_json::json!({"command": "cargo test"})),
            "ok".into(),
            0,
            None,
            true,
        );
        let plain = a3s_tui::style::strip_ansi(&transcript.render(80, 79).join("\n"));
        assert!(plain.contains("cargo test"), "{plain}");
    }

    #[test]
    fn adjacent_explore_calls_group_without_erasing_meaningful_rereads() {
        let mut transcript = Transcript::default();
        for (id, name, args) in [
            ("r1", "read", serde_json::json!({"file_path": "auth.rs"})),
            ("r2", "read", serde_json::json!({"file_path": "auth.rs"})),
            (
                "g1",
                "grep",
                serde_json::json!({"pattern": "TODO", "path": "src"}),
            ),
        ] {
            transcript.start_tool(id.into(), name.into(), true);
            transcript.finish_tool(id, name.into(), Some(args), String::new(), 0, None, true);
        }
        let blocks = transcript.render(80, 79);
        assert_eq!(blocks.len(), 1);
        let rendered = &blocks[0];
        let plain = a3s_tui::style::strip_ansi(rendered);
        assert_eq!(
            plain.lines().collect::<Vec<_>>(),
            [
                "• Explored",
                "  └ Read auth.rs, auth.rs",
                "    Search TODO in src"
            ],
            "{plain}"
        );
        assert_eq!(plain.matches("auth.rs").count(), 2, "{plain}");
        assert!(
            rendered.contains(&Style::new().fg(TN_CYAN).render("Read")),
            "{rendered:?}"
        );
        assert!(
            rendered.contains(&Style::new().fg(TOOL_ARGUMENT_COLOR).render("TODO")),
            "{rendered:?}"
        );
        assert!(
            rendered.contains(&Style::new().fg(TOOL_PATH_COLOR).render("src")),
            "{rendered:?}"
        );
    }

    #[test]
    fn adjacent_explore_calls_are_grouped_while_live_then_finish_in_place() {
        let mut transcript = Transcript::default();
        transcript.start_tool_execution(
            "read-live".into(),
            "read".into(),
            serde_json::json!({"file_path":"src/lib.rs"}),
            true,
        );
        transcript.start_tool_execution(
            "grep-live".into(),
            "grep".into(),
            serde_json::json!({"pattern":"TODO", "path":"src"}),
            true,
        );

        let live = transcript.render_with_activity(80, 79, true);
        assert_eq!(live.len(), 1);
        let live = a3s_tui::style::strip_ansi(&live[0]);
        assert!(live.starts_with("• Exploring\n"), "{live}");
        assert!(live.contains("Read src/lib.rs"), "{live}");
        assert!(live.contains("Search TODO in src"), "{live}");

        for (id, name) in [("read-live", "read"), ("grep-live", "grep")] {
            transcript.finish_tool(id, name.into(), None, String::new(), 0, None, true);
        }
        let completed = transcript.render_with_activity(80, 79, true);
        assert_eq!(completed.len(), 1);
        let completed = a3s_tui::style::strip_ansi(&completed[0]);
        assert!(completed.starts_with("• Explored\n"), "{completed}");
    }

    #[test]
    fn consecutive_tool_cells_each_receive_one_compositor_gap() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::assistant_markdown(
            "I will verify both layers.",
        )]);
        for (id, command, output) in [
            ("shell-1", "cargo check", "check passed"),
            ("shell-2", "cargo test focused", "test passed"),
        ] {
            let args = serde_json::json!({"command": command});
            transcript.start_tool_execution(id.into(), "bash".into(), args.clone(), true);
            transcript.finish_tool(id, "bash".into(), Some(args), output.into(), 0, None, true);
        }
        transcript.push(TranscriptEntry::assistant_markdown(
            "Both verification layers passed.",
        ));

        let blocks = transcript.render(80, 79);

        assert_eq!(blocks.len(), 4, "each tool call is one semantic block");
        let first_activity = a3s_tui::style::strip_ansi(&blocks[1]);
        let second_activity = a3s_tui::style::strip_ansi(&blocks[2]);
        assert!(first_activity.contains("cargo check"), "{first_activity}");
        assert!(
            second_activity.contains("cargo test focused"),
            "{second_activity}"
        );

        let tool_spans = transcript
            .entries
            .iter()
            .filter_map(|stored| match &stored.entry {
                TranscriptEntry::Tool(_) => transcript
                    .layout
                    .iter()
                    .find(|span| span.entry_id == stored.id)
                    .copied(),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(tool_spans.len(), 2);
        assert_eq!(
            tool_spans[1].start_row,
            tool_spans[0].start_row + tool_spans[0].row_count + TRANSCRIPT_BLOCK_GAP_ROWS,
            "adjacent activity cells must reserve the compositor gap row"
        );

        let flow = a3s_tui::style::strip_ansi(&join_transcript_blocks(&blocks));
        let rows = flow.lines().collect::<Vec<_>>();
        let before = rows
            .iter()
            .position(|row| row.contains("both layers."))
            .expect("assistant before row");
        let first_tool = rows
            .iter()
            .position(|row| row.contains("• Ran cargo check"))
            .expect("first tool row");
        let last_tool = rows
            .iter()
            .rposition(|row| row.contains("test passed"))
            .expect("last tool row");
        let second_tool = rows
            .iter()
            .position(|row| row.contains("• Ran cargo test focused"))
            .expect("second tool row");
        let after = rows
            .iter()
            .position(|row| row.contains("Both verification"))
            .expect("assistant after row");
        assert!(
            rows[before + 1].trim().is_empty() && first_tool == before + 2,
            "assistant-to-tool spacing should be owned by the compositor: {flow}"
        );
        assert!(
            rows[second_tool - 1].trim().is_empty(),
            "tool-to-tool spacing should be owned by the compositor: {flow}"
        );
        assert!(
            rows[last_tool + 1].trim().is_empty() && after == last_tool + 2,
            "tool-to-assistant spacing should be owned by the compositor: {flow}"
        );
    }

    #[test]
    fn hidden_tool_placeholder_can_be_discarded_without_reordering_neighbors() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::user("before")]);
        transcript.start_tool("hidden".into(), "write".into(), false);
        transcript.push(TranscriptEntry::assistant_markdown("after"));
        assert!(transcript.discard_tool("hidden"));
        assert_eq!(transcript.iter().count(), 2);
    }

    #[test]
    fn duplicate_terminal_delivery_updates_one_tool_entry() {
        let mut transcript = Transcript::default();
        transcript.start_tool_execution(
            "host-1".into(),
            "dynamic_workflow".into(),
            serde_json::json!({"run_id": "run-1"}),
            true,
        );
        for output in ["raw", "sanitized"] {
            transcript.finish_tool(
                "host-1",
                "dynamic_workflow".into(),
                Some(serde_json::json!({"run_id": "run-1"})),
                output.to_string(),
                0,
                None,
                true,
            );
        }

        assert_eq!(transcript.iter().count(), 1);
        let TranscriptEntry::Tool(tool) = transcript.iter().next().unwrap() else {
            panic!("expected tool entry");
        };
        assert_eq!(tool.output, "sanitized");
    }

    #[test]
    fn duplicate_tool_end_preserves_denied_terminal_state() {
        let mut transcript = Transcript::default();
        transcript.await_tool_approval(
            "denied-1".into(),
            "bash".into(),
            serde_json::json!({"command": "dangerous"}),
        );
        transcript.finish_tool_with_state(
            "denied-1",
            "bash".into(),
            None,
            "Denied by user".into(),
            1,
            None,
            ToolCallState::Denied,
            true,
        );
        transcript.finish_tool(
            "denied-1",
            "bash".into(),
            Some(serde_json::json!({"command": "dangerous"})),
            "tool execution denied".into(),
            1,
            None,
            true,
        );

        let TranscriptEntry::Tool(tool) = transcript.iter().next().unwrap() else {
            panic!("expected tool entry");
        };
        assert_eq!(tool.state, ToolCallState::Denied);
        assert_eq!(tool.output, "Denied by user");
        let plain = a3s_tui::style::strip_ansi(&transcript.render(80, 79).join("\n"));
        assert!(plain.contains("Denied dangerous"), "{plain}");
        assert!(!plain.contains("Ran dangerous"), "{plain}");
    }

    #[test]
    fn preformatted_entries_are_preserved_verbatim() {
        let value = format!("{}notice", ACCENT.fg_ansi());
        assert_eq!(TranscriptEntry::preformatted(&value).render(40, 39), value);
    }

    #[test]
    fn semantic_notices_reflow_and_keep_severity_across_widths() {
        let entry = TranscriptEntry::notice(
            NoticeKind::Error,
            "无法连接 provider because the configured endpoint did not respond",
        );
        let narrow = entry.render(28, 27);
        let wide = entry.render(80, 79);

        assert!(narrow.lines().count() > wide.lines().count());
        assert!(narrow.contains(&message_marker(MessageTone::Error)));
        assert!(wide.contains(&message_marker(MessageTone::Error)));
        assert_bounded(&narrow, 27);
        assert_bounded(&wide, 79);
    }

    #[test]
    fn mixed_message_gallery_preserves_hierarchy_at_product_widths() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::notice(NoticeKind::Info, "Context auto-compacted at 85%"),
            TranscriptEntry::user("请检查 src/tui/ui/render.rs and preserve the visual hierarchy"),
            TranscriptEntry::assistant_markdown(
                "I’ll inspect the rendering path, then verify `cargo test`.",
            ),
            TranscriptEntry::reasoning(
                "Compare semantic state, message density, and responsive wrapping.",
            ),
        ]);
        transcript.start_tool_execution(
            "read-live".into(),
            "read".into(),
            serde_json::json!({"file_path": "src/tui/ui/render.rs"}),
            true,
        );
        transcript.await_tool_approval(
            "write-awaiting".into(),
            "write".into(),
            serde_json::json!({
                "file_path": "src/tui/ui/message_chrome.rs",
                "content": "semantic message chrome"
            }),
        );
        transcript.finish_tool(
            "exec-done",
            "bash".into(),
            Some(serde_json::json!({"command": "cargo test --bin a3s"})),
            "1234 tests passed".into(),
            0,
            None,
            true,
        );
        let diff_before = (0..60)
            .map(|index| format!("old-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let diff_after = (0..60)
            .map(|index| format!("new-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        transcript.finish_tool(
            "edit-large",
            "edit".into(),
            Some(serde_json::json!({"file_path": "src/large.rs"})),
            "Updated src/large.rs".into(),
            0,
            Some(serde_json::json!({
                "file_path": "src/large.rs",
                "before": diff_before,
                "after": diff_after
            })),
            true,
        );
        transcript.finish_tool(
            "lookup-failed",
            "custom_lookup".into(),
            Some(serde_json::json!({"path": "./fixtures/研究.json", "count": 2})),
            "provider did not respond".into(),
            1,
            None,
            true,
        );
        transcript.finish_tool(
            "batch-partial",
            "batch".into(),
            Some(serde_json::json!({
                "invocations": [
                    {"tool": "read", "args": {"file_path": "README.md"}},
                    {"tool": "bash", "args": {"command": "cargo test"}}
                ]
            })),
            "--- [1: read] ---\ncontents\n--- [2: bash] ---\nERROR: failed".into(),
            0,
            Some(serde_json::json!({
                "execution_mode": "parallel",
                "applied_concurrency": 2,
                "success_count": 1,
                "failure_count": 1,
                "results": [
                    {"index": 0, "tool": "read", "success": true, "exit_code": 0},
                    {"index": 1, "tool": "bash", "success": false, "exit_code": 101}
                ]
            })),
            true,
        );
        transcript.finish_tool(
            "mcp-json",
            "mcp__docs__find".into(),
            Some(serde_json::json!({"query": "terminal UX"})),
            serde_json::json!({
                "documents": [
                    {"title": "Message hierarchy", "score": 0.98},
                    {"title": "Streaming stability", "score": 0.91}
                ]
            })
            .to_string(),
            0,
            None,
            true,
        );
        transcript.finish_tool_with_state(
            "exec-denied",
            "bash".into(),
            Some(serde_json::json!({"command": "rm -rf protected"})),
            "Denied by user policy.".into(),
            1,
            None,
            ToolCallState::Denied,
            true,
        );
        transcript.finish_tool_with_state(
            "fetch-timeout",
            "web_fetch".into(),
            Some(serde_json::json!({"url": "https://example.com/slow"})),
            "Request exceeded the tool deadline.".into(),
            124,
            None,
            ToolCallState::TimedOut,
            true,
        );
        transcript.finish_tool_with_state(
            "lookup-interrupted",
            "custom_lookup".into(),
            Some(serde_json::json!({"query": "cancelled lookup"})),
            "Stopped by user.".into(),
            130,
            None,
            ToolCallState::Interrupted,
            true,
        );
        transcript.finish_subagent(
            "agent-1".into(),
            "reviewer".into(),
            "Audit the tool state matrix".into(),
            true,
            "State transitions are consistent.".into(),
            true,
        );
        transcript.finish_subagent_with_outcome(
            "agent-2".into(),
            "planner".into(),
            "Stop a superseded planning branch".into(),
            SubagentOutcome::Cancelled,
            "Stopped after the primary branch completed.".into(),
            true,
        );
        transcript.finish_subagent_with_outcome(
            "agent-3".into(),
            "auditor".into(),
            "Verify an unavailable provider".into(),
            SubagentOutcome::Failed,
            "Provider authentication was unavailable.".into(),
            true,
        );

        for width in [24_u16, 32, 48, 80, 120] {
            let content_width = usize::from(width.saturating_sub(1));
            let compact = transcript
                .render_with_activity(width, content_width, true)
                .join("\n\n");
            let compact_plain = a3s_tui::style::strip_ansi(&compact);
            let compact_flow = compact_plain
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ");
            let compact_dense = compact_plain
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>();
            assert_bounded(&compact, content_width);
            assert!(
                compact_flow.contains("Context auto-compacted"),
                "{compact_plain}"
            );
            assert!(compact_flow.contains("Exploring"), "{compact_plain}");
            assert!(
                compact_flow.contains("Awaiting approval"),
                "{compact_plain}"
            );
            assert!(
                compact_flow.contains("1234 tests passed"),
                "{compact_plain}"
            );
            assert!(compact_flow.contains("diff · Ctrl+T"), "{compact_plain}");
            assert!(!compact_plain.contains("new-59"), "{compact_plain}");
            assert!(
                compact_flow.contains("Batch partially completed"),
                "{compact_plain}"
            );
            assert!(compact_flow.contains("exit 101"), "{compact_plain}");
            assert!(compact_flow.contains("Called docs.find"), "{compact_plain}");
            assert!(
                compact_dense.contains("providerdidnotrespond"),
                "{compact_plain}"
            );
            assert!(compact_flow.contains("Agent completed"), "{compact_plain}");
            assert!(compact_flow.contains("Denied"), "{compact_plain}");
            assert!(compact_flow.contains("Timed out"), "{compact_plain}");
            assert!(compact_flow.contains("Interrupted"), "{compact_plain}");
            assert!(compact_flow.contains("Agent cancelled"), "{compact_plain}");
            assert!(compact_flow.contains("Agent failed"), "{compact_plain}");
            assert!(!compact_flow.contains("Reasoning"), "{compact_plain}");

            let full = transcript
                .render_transcript_with_activity(width, content_width, true)
                .join("\n\n");
            let full_plain = a3s_tui::style::strip_ansi(&full);
            let full_flow = full_plain.split_whitespace().collect::<Vec<_>>().join(" ");
            let full_dense = full_plain
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>();
            assert_bounded(&full, content_width);
            assert!(full_flow.contains("Reasoning"), "{full_plain}");
            assert!(full_flow.contains("Input"), "{full_plain}");
            assert!(full_flow.contains("⊘ denied"), "{full_plain}");
            assert!(full_flow.contains("◷ timed out"), "{full_plain}");
            assert!(full_flow.contains("■ interrupted"), "{full_plain}");
            assert!(full_dense.contains("new-59"), "{full_plain}");
            assert!(full_dense.contains("Streamingstability"), "{full_plain}");
            assert!(full_flow.contains("! partial · 1 failed"), "{full_plain}");
        }
    }

    #[test]
    fn tracked_notice_finishes_in_place_after_unrelated_entries_arrive() {
        let mut transcript = Transcript::default();
        let status = transcript.push_tracked(TranscriptEntry::preformatted("interrupting…"));
        transcript.push(TranscriptEntry::assistant_markdown("partial answer"));
        transcript.start_tool("tool-after-status".into(), "bash".into(), true);

        assert!(transcript.replace_preformatted(status, "interrupted"));

        let entries = transcript.iter().collect::<Vec<_>>();
        assert_eq!(entries.len(), 3);
        assert_eq!(entries[0], &TranscriptEntry::preformatted("interrupted"));
        let plain = a3s_tui::style::strip_ansi(&transcript.render(80, 79).join("\n"));
        assert_eq!(plain.matches("interrupted").count(), 1, "{plain}");
        assert!(!plain.contains("interrupting"), "{plain}");
        assert!(plain.contains("partial answer"), "{plain}");
    }

    #[test]
    fn replacing_tracked_notice_invalidates_its_render_cache_and_layout() {
        let mut transcript = Transcript::default();
        let status = transcript.push_tracked(TranscriptEntry::preformatted("working…"));
        transcript.render(80, 79);
        assert!(transcript.entries[0].render_cache.is_some());
        assert!(!transcript.layout.is_empty());

        assert!(transcript.replace_preformatted(status, "done"));

        assert!(transcript.entries[0].render_cache.is_none());
        assert!(transcript.layout.is_empty());
        assert_eq!(transcript.render(80, 79), ["done"]);
    }

    #[test]
    fn cleared_transcript_does_not_reuse_a_late_status_id() {
        let mut transcript = Transcript::default();
        let stale = transcript.push_tracked(TranscriptEntry::preformatted("old operation…"));
        transcript.clear();
        let current = transcript.push_tracked(TranscriptEntry::preformatted("new session"));

        assert_ne!(stale, current);
        assert!(!transcript.replace_preformatted(stale, "late result"));
        assert_eq!(transcript.render(80, 79), ["new session"]);
    }

    #[test]
    fn transcript_entry_cache_survives_unrelated_tool_mutation() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::user("one")]);
        transcript.render(40, 39);
        let first_id = transcript.entries[0].id;
        assert!(transcript.entries[0].render_cache.is_some());
        transcript.start_tool("t".into(), "bash".into(), true);
        transcript.push_tool_input(Some("t"), r#"{"command":"echo"}"#);
        assert_eq!(transcript.entries[0].id, first_id);
        assert!(transcript.entries[0].render_cache.is_some());
    }

    #[test]
    fn semantic_anchor_survives_reflow_above_the_entry() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::assistant_markdown(
                "A deliberately long paragraph above the reading position that wraps very differently at narrow and wide terminal widths.",
            ),
            TranscriptEntry::assistant_markdown("anchor target\nsecond target row"),
        ]);
        transcript.render(28, 27);
        let target_id = transcript.entries[1].id;
        let old_span = transcript
            .layout
            .iter()
            .find(|span| span.entry_id == target_id)
            .copied()
            .unwrap();
        let anchor = transcript.anchor_for_row(old_span.start_row + 1).unwrap();

        transcript.render(100, 99);
        let restored = transcript.row_for_anchor(anchor).unwrap();
        let new_span = transcript
            .layout
            .iter()
            .find(|span| span.entry_id == target_id)
            .copied()
            .unwrap();

        assert!(restored >= new_span.start_row);
        assert!(restored < new_span.start_row + new_span.row_count);
        assert_ne!(old_span.start_row, new_span.start_row);
    }

    #[test]
    fn semantic_anchor_does_not_claim_rows_in_the_live_stream_tail() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::user(
            "completed history before the live response",
        )]);
        transcript.render(40, 39);
        let history_rows = transcript.selection_rows.len();

        assert!(transcript
            .anchor_for_row(history_rows.saturating_sub(1))
            .is_some());
        assert!(
            transcript.anchor_for_row(history_rows).is_none(),
            "the first live-stream row must retain an absolute viewport anchor"
        );
        assert!(
            transcript.anchor_for_row(history_rows + 20).is_none(),
            "later live-stream rows must not snap back to completed history"
        );
    }

    #[test]
    fn semantic_selection_survives_entry_reflow_and_keeps_complete_copy_range() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::assistant_markdown(
                "prefix words alpha selection crosses several wrapped rows before omega suffix",
            ),
            TranscriptEntry::assistant_markdown(
                "A later streamed entry must not invalidate a completed-history selection.",
            ),
        ]);
        transcript.render(24, 23);
        let (alpha_row, alpha_col) = cell_containing(&transcript, "alpha");
        let (omega_row, omega_col) = cell_containing(&transcript, "omega");
        let anchor = transcript
            .point_for_cell(alpha_row, alpha_col)
            .expect("alpha semantic point");
        let head = transcript
            .point_for_cell(omega_row, omega_col + "omega".len())
            .expect("omega semantic point");
        let mut selection = TranscriptSelection::collapsed(anchor);
        selection.set_head(head);

        transcript.render(72, 71);

        let copied = transcript
            .selected_text(selection)
            .expect("selection should project after resize");
        let normalized = copied.split_whitespace().collect::<Vec<_>>().join(" ");
        assert!(normalized.starts_with("alpha"), "{normalized:?}");
        assert!(normalized.ends_with("omega"), "{normalized:?}");
        assert!(
            normalized.contains("several wrapped rows"),
            "{normalized:?}"
        );
    }

    #[test]
    fn semantic_selection_ignores_repeated_gutter_and_user_surface_decoration() {
        let mut transcript = Transcript::from_entries(vec![TranscriptEntry::user(
            "start abcdefghijklmnopqrstuvwxyz target finish",
        )]);
        transcript.render(14, 13);
        let (row, col) = cell_containing(&transcript, "target");
        let point = transcript
            .point_for_cell(row, col)
            .expect("target semantic point");

        transcript.render(48, 47);

        let (restored_row, restored_col) = transcript
            .cell_for_point(point)
            .expect("point should survive user-bubble reflow");
        let restored = &transcript.selection_rows[restored_row];
        assert_eq!(
            a3s_tui::style::slice_visible_cols(
                restored,
                restored_col,
                restored_col + visible_len("target")
            ),
            "target"
        );
    }

    #[test]
    fn semantic_selection_copy_is_not_limited_to_a_visible_viewport_window() {
        let mut transcript = Transcript::from_entries(vec![
            TranscriptEntry::assistant_markdown("first anchor"),
            TranscriptEntry::assistant_markdown(
                "middle one\nmiddle two\nmiddle three\nmiddle four\nmiddle five",
            ),
            TranscriptEntry::assistant_markdown("last anchor"),
        ]);
        transcript.render(32, 31);
        let (first_row, first_col) = cell_containing(&transcript, "first");
        let (last_row, last_col) = cell_containing(&transcript, "last");
        let anchor = transcript
            .point_for_cell(first_row, first_col)
            .expect("first point");
        let head = transcript
            .point_for_cell(last_row, last_col + "last".len())
            .expect("last point");
        let mut selection = TranscriptSelection::collapsed(anchor);
        selection.set_head(head);

        let copied = transcript
            .selected_text(selection)
            .expect("full transcript copy");

        assert!(copied.contains("first anchor"), "{copied:?}");
        assert!(copied.contains("middle five"), "{copied:?}");
        assert!(copied.contains("last"), "{copied:?}");
    }

    fn cell_containing(transcript: &Transcript, needle: &str) -> (usize, usize) {
        transcript
            .selection_rows
            .iter()
            .enumerate()
            .find_map(|(row, line)| {
                line.find(needle)
                    .map(|byte| (row, visible_len(&line[..byte])))
            })
            .unwrap_or_else(|| panic!("missing {needle:?} in {:?}", transcript.selection_rows))
    }

    #[test]
    fn ctrl_t_render_keeps_full_tool_output_and_each_call_in_start_order() {
        let mut transcript = Transcript::default();
        let output = (0..18)
            .map(|index| format!("line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        for (id, path) in [("read-1", "first.rs"), ("read-2", "second.rs")] {
            let args = serde_json::json!({"file_path": path});
            transcript.start_tool_execution(id.into(), "read".into(), args.clone(), true);
            transcript.finish_tool(id, "read".into(), Some(args), output.clone(), 0, None, true);
        }

        let compact = transcript.render(80, 79);
        assert_eq!(
            compact.len(),
            1,
            "successful explore calls compact in history"
        );

        let complete = transcript.render_transcript_with_activity(80, 79, true);
        assert_eq!(complete.len(), 2, "Ctrl+T retains each semantic tool call");
        let plain = complete
            .iter()
            .map(|block| a3s_tui::style::strip_ansi(block))
            .collect::<Vec<_>>()
            .join("\n\n");
        assert!(plain.find("first.rs").unwrap() < plain.find("second.rs").unwrap());
        assert_eq!(plain.matches("line-17").count(), 2, "{plain}");
        assert!(!plain.contains("… +"), "{plain}");
    }

    #[test]
    fn restored_tool_does_not_invent_a_replay_duration() {
        let mut transcript = Transcript::default();
        transcript.restore_tool_execution(
            "restored-1".into(),
            "bash".into(),
            serde_json::json!({"command":"echo restored"}),
            true,
        );
        transcript.finish_tool(
            "restored-1",
            "bash".into(),
            None,
            "restored output".into(),
            0,
            None,
            true,
        );

        let TranscriptEntry::Tool(tool) = transcript.iter().next().unwrap() else {
            panic!("expected tool entry");
        };
        assert_eq!(tool.duration, None);
        let plain = a3s_tui::style::strip_ansi(
            &transcript
                .render_transcript_with_activity(80, 79, true)
                .join("\n"),
        );
        assert!(plain.ends_with('✓'), "{plain}");
        assert!(!plain.contains("unknown"), "{plain}");
    }
}
