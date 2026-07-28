//! Rendering of completed tool calls: labels, arg summaries, and file diffs.

use super::batch_view::{BatchItem, BatchOutcome, BatchSummary};
#[cfg(test)]
use super::file_change_view::{
    mix_diff_color, DIFF_DELETE_BG, DIFF_DELETE_MARKER, DIFF_HEADER_ACTION, DIFF_HEADER_BULLET,
    DIFF_INSERT_BG, DIFF_INSERT_MARKER,
};
use super::file_change_view::{render_compact_file_change, render_full_file_change};
use super::message_chrome::{
    message_marker, message_status, result_message_tone, tool_message_tone, MessageTone,
};
use super::program_preview::{summarize_program_args, summarize_program_calls};
use super::tool_style::{
    header_action_color, highlight_explore_detail, highlight_json_wrapped, highlight_shell,
    highlight_shell_wrapped, highlight_tool_detail,
};
use super::tool_transcript_view::{render_tool_transcript_details, ToolTranscriptSection};
use super::*;
use a3s_tui::style::{slice_visible_cols, strip_ansi, truncate_visible, visible_len, wrap_words};

const MAX_COMMAND_ROWS: usize = 8;
const MAX_EXEC_COMMAND_ROWS: usize = 3;
const MAX_OUTPUT_ROWS: usize = 5;
const MAX_LOGICAL_OUTPUT_LINES: usize = 10;
const MAX_BATCH_ITEM_ROWS: usize = 6;

/// Render one tool call for the Ctrl+T transcript.
///
/// The compact history cell intentionally bounds command and output rows. The
/// transcript is the escape hatch advertised by that cell, so it must retain
/// the complete command, result, and terminal status instead of delegating
/// back to the same bounded renderer.
pub(crate) struct ToolTranscriptInput<'a> {
    pub(crate) name: &'a str,
    pub(crate) state: ToolCallState,
    pub(crate) exit_code: Option<i32>,
    pub(crate) output: &'a str,
    pub(crate) metadata: Option<&'a serde_json::Value>,
    pub(crate) args: Option<&'a serde_json::Value>,
    pub(crate) duration: Option<std::time::Duration>,
    pub(crate) width: usize,
}

pub(crate) fn render_tool_transcript(input: ToolTranscriptInput<'_>) -> String {
    let ToolTranscriptInput {
        name,
        state,
        exit_code,
        output,
        metadata: meta,
        args,
        duration,
        width,
    } = input;
    if width == 0 {
        return String::new();
    }

    if is_exec_tool(name) {
        return render_exec_transcript(
            exec_command(name, args).as_deref().unwrap_or(name),
            state,
            exit_code,
            output,
            duration,
            width,
        );
    }

    let terminal = state.is_terminal();
    let failed = matches!(
        state,
        ToolCallState::Failed
            | ToolCallState::Denied
            | ToolCallState::TimedOut
            | ToolCallState::Interrupted
    );
    let expands_arguments = !has_specialized_tool_verb(name) || mcp_name(name).is_some();
    let mut header = String::new();

    if terminal && state == ToolCallState::Succeeded && is_file_change_tool(name) {
        if let Some(diff) = render_successful_file_change_transcript(name, meta, width) {
            header = diff;
        }
    }

    if header.is_empty() {
        header = if expands_arguments {
            render_transcript_tool_identity(name, state, width)
        } else if terminal {
            render_tool_terminal(name, state, exit_code.unwrap_or(1), "", meta, args, width)
        } else {
            render_live_tool_activity(name, args, "", width, true, state)
        };
    }

    let detail_width = width.saturating_sub(4).max(1);
    let mut sections = Vec::new();
    if expands_arguments {
        if let Some(args) = args {
            let body = render_transcript_arguments(args, detail_width);
            if !body.is_empty() {
                sections.push(ToolTranscriptSection::new("Input", body));
            }
        }
    }

    if !output.trim().is_empty() {
        let structured = !has_specialized_tool_verb(name) || mcp_name(name).is_some();
        let output = if structured {
            completed_structured_output(output)
        } else {
            output.to_string()
        };
        let body = if structured {
            render_full_json_output(&output, detail_width, "")
                .unwrap_or_else(|| render_full_output(&output, detail_width, failed, ""))
        } else {
            render_full_output(&output, detail_width, failed, "")
        };
        if !body.is_empty() {
            sections.push(ToolTranscriptSection::new("Result", body));
        }
    }

    let status = if terminal {
        if name == "batch" {
            if let Some(summary) =
                BatchSummary::from_metadata(meta, args, state == ToolCallState::Succeeded)
            {
                let status = render_batch_transcript_status(&summary, duration, detail_width);
                (summary.outcome != BatchOutcome::Complete || duration.is_some()).then_some(status)
            } else {
                transcript_terminal_status(state, exit_code, duration, detail_width)
            }
        } else {
            transcript_terminal_status(state, exit_code, duration, detail_width)
        }
    } else {
        None
    };

    let details = render_tool_transcript_details(sections, status, width);
    join_cell_parts(header, details)
}

fn render_transcript_tool_identity(name: &str, state: ToolCallState, width: usize) -> String {
    let action = match state {
        ToolCallState::Preparing | ToolCallState::Running => "Calling",
        ToolCallState::AwaitingApproval => "Awaiting approval for",
        ToolCallState::Succeeded => "Called",
        ToolCallState::Failed => "Failed",
        ToolCallState::Denied => "Denied",
        ToolCallState::TimedOut => "Timed out",
        ToolCallState::Interrupted => "Interrupted",
    };
    let identity = mcp_display_name(name).unwrap_or_else(|| sanitize_terminal_text(name));
    render_action_header(
        action,
        Some(&identity),
        width,
        tool_message_tone(state, true),
        "  ",
        false,
    )
}

fn transcript_terminal_status(
    state: ToolCallState,
    exit_code: Option<i32>,
    duration: Option<std::time::Duration>,
    width: usize,
) -> Option<String> {
    if state == ToolCallState::Succeeded && duration.is_none() {
        return None;
    }
    let status = render_transcript_terminal_status(state, exit_code, duration, width);
    (!status.is_empty()).then_some(status)
}

fn render_exec_transcript(
    command: &str,
    state: ToolCallState,
    exit_code: Option<i32>,
    output: &str,
    duration: Option<std::time::Duration>,
    width: usize,
) -> String {
    let command = sanitize_terminal_text(command);
    let mut rows = Vec::new();
    let command_rows = highlight_shell_wrapped(
        &command,
        width.saturating_sub(2).max(1),
        width.saturating_sub(4).max(1),
    );
    for (index, row) in command_rows.into_iter().enumerate() {
        let prefix = if index == 0 { "$ " } else { "    " };
        let prefix = Style::new().fg(TN_SUBTLE).render(prefix);
        rows.push(truncate_visible(
            &format!("{prefix}{}", row.trim_end()),
            width,
        ));
    }
    if rows.is_empty() {
        rows.push("$".to_string());
    }

    let failed = matches!(
        state,
        ToolCallState::Failed
            | ToolCallState::Denied
            | ToolCallState::TimedOut
            | ToolCallState::Interrupted
    );
    if !output.trim().is_empty() {
        rows.push(render_full_output(output, width, failed, ""));
    }
    if state.is_terminal() {
        rows.push(render_transcript_terminal_status(
            state, exit_code, duration, width,
        ));
    }
    rows.join("\n")
}

fn render_transcript_arguments(args: &serde_json::Value, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if let Some(inline) = complete_inline_arguments(args, width) {
        return highlight_tool_detail(&inline);
    }

    let serialized = serde_json::to_string_pretty(args).unwrap_or_else(|_| args.to_string());
    render_full_json_output(&serialized, width, "")
        .unwrap_or_else(|| render_full_output(&serialized, width, false, ""))
}

/// Return a complete, non-truncated one-line argument projection when every
/// value fits. Nested values deliberately fall back to syntax-highlighted JSON
/// so the full transcript never exchanges completeness for compactness.
fn complete_inline_arguments(args: &serde_json::Value, width: usize) -> Option<String> {
    let inline = match args {
        serde_json::Value::Object(fields)
            if fields
                .values()
                .all(|value| !value.is_array() && !value.is_object()) =>
        {
            if fields.is_empty() {
                "{}".to_string()
            } else {
                fields
                    .iter()
                    .map(|(key, value)| {
                        let value = serde_json::to_string(value).unwrap_or_else(|_| "null".into());
                        format!("{key}={value}")
                    })
                    .collect::<Vec<_>>()
                    .join(" · ")
            }
        }
        serde_json::Value::Array(values)
            if values
                .iter()
                .all(|value| !value.is_array() && !value.is_object()) =>
        {
            serde_json::to_string(values).ok()?
        }
        value if !value.is_array() && !value.is_object() => serde_json::to_string(value).ok()?,
        _ => return None,
    };
    let inline = sanitize_terminal_text(&inline);
    (!inline.contains('\n') && visible_len(&inline) <= width).then_some(inline)
}

fn render_full_output(output: &str, width: usize, error: bool, prefix: &str) -> String {
    if width == 0 {
        return String::new();
    }
    let output = sanitize_terminal_text(output);
    let body_width = width.saturating_sub(visible_len(prefix)).max(1);
    let mut rows = Vec::new();
    for logical in output.split('\n') {
        let logical = logical.trim_end_matches('\r');
        for row in wrap_preserving_text(logical, body_width) {
            rows.push(render_prefixed_row(prefix, &row, width, error));
        }
    }
    while rows
        .last()
        .is_some_and(|row| strip_ansi(row).trim().is_empty())
    {
        rows.pop();
    }
    rows.join("\n")
}

fn render_full_json_output(output: &str, width: usize, prefix: &str) -> Option<String> {
    if width == 0 {
        return Some(String::new());
    }
    let pretty = pretty_json(output)?;
    let body_width = width.saturating_sub(visible_len(prefix)).max(1);
    let mut rows = Vec::new();
    for logical in pretty.split('\n') {
        for display_line in responsive_json_display_lines(logical, body_width) {
            for row in highlight_json_wrapped(&display_line, body_width) {
                rows.push(render_styled_prefixed_row(prefix, &row, width));
            }
        }
    }
    Some(rows.join("\n"))
}

/// Move a complete scalar value below its key when both cannot share a narrow
/// row. This preserves short values such as `"result-0"` as one scannable token
/// instead of slicing them after a few characters merely because JSON
/// indentation consumed the remaining columns.
fn responsive_json_display_lines(logical: &str, width: usize) -> Vec<String> {
    if visible_len(logical) <= width {
        return vec![logical.to_string()];
    }
    let trimmed = logical.trim_start_matches(char::is_whitespace);
    let indent_bytes = logical.len().saturating_sub(trimmed.len());
    let indent = &logical[..indent_bytes];
    let Some((key, value)) = trimmed.split_once(": ") else {
        return vec![logical.to_string()];
    };
    let value = value.trim();
    let is_scalar = value.starts_with('"')
        || value.starts_with('-')
        || value.starts_with(|ch: char| ch.is_ascii_digit())
        || value.starts_with("true")
        || value.starts_with("false")
        || value.starts_with("null");
    if !key.starts_with('"') || !is_scalar {
        return vec![logical.to_string()];
    }

    let key_row = format!("{indent}{key}:");
    let value_row = format!("{indent}  {value}");
    if visible_len(&key_row) <= width && visible_len(&value_row) <= width {
        vec![key_row, value_row]
    } else {
        vec![logical.to_string()]
    }
}

fn render_transcript_terminal_status(
    state: ToolCallState,
    exit_code: Option<i32>,
    duration: Option<std::time::Duration>,
    width: usize,
) -> String {
    let (glyph, label, tone) = match state {
        ToolCallState::Succeeded => ("✓", String::new(), MessageTone::Success),
        ToolCallState::Failed => (
            "✗",
            format!("({})", exit_code.unwrap_or(1)),
            MessageTone::Error,
        ),
        ToolCallState::Denied => ("⊘", "denied".to_string(), MessageTone::Warning),
        ToolCallState::TimedOut => ("◷", "timed out".to_string(), MessageTone::Error),
        ToolCallState::Interrupted => ("■", "interrupted".to_string(), MessageTone::Warning),
        ToolCallState::Preparing | ToolCallState::AwaitingApproval | ToolCallState::Running => {
            return String::new();
        }
    };
    render_wrapped_transcript_status(glyph, &label, tone, duration, width)
}

fn render_wrapped_transcript_status(
    glyph: &str,
    label: &str,
    tone: MessageTone,
    duration: Option<std::time::Duration>,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }
    let mut body = label.to_string();
    if let Some(duration) = duration {
        if !body.is_empty() {
            body.push(' ');
        }
        body.push_str("• ");
        body.push_str(&format_transcript_duration(duration));
    }
    if body.is_empty() || width == 1 {
        return truncate_visible(&message_status(glyph, "", tone, true), width);
    }

    let body_width = width.saturating_sub(2).max(1);
    let mut rows = wrap_words(&body, body_width).into_iter();
    let first = rows.next().unwrap_or_default();
    std::iter::once(message_status(glyph, &first, tone, true))
        .chain(rows.map(|row| Style::new().fg(TN_GRAY).render(&row)))
        .collect::<Vec<_>>()
        .join("\n")
}

fn format_transcript_duration(duration: std::time::Duration) -> String {
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

/// Render a terminal tool while preserving policy/host terminal states that
/// cannot be inferred from an exit code. Codex keeps approval denial, timeout,
/// and interruption semantically distinct from an ordinary execution failure.
pub(crate) fn render_tool_terminal(
    name: &str,
    state: ToolCallState,
    exit_code: i32,
    output: &str,
    meta: Option<&serde_json::Value>,
    args: Option<&serde_json::Value>,
    width: usize,
) -> String {
    if matches!(state, ToolCallState::Succeeded | ToolCallState::Failed) {
        return render_tool_end(name, exit_code, output, meta, args, width);
    }

    let action = match (name, state) {
        ("dynamic_workflow", ToolCallState::Denied) => "Denied workflow",
        ("dynamic_workflow", ToolCallState::TimedOut) => "Timed out workflow",
        ("dynamic_workflow", ToolCallState::Interrupted) => "Interrupted workflow",
        (_, ToolCallState::Denied) => "Denied",
        (_, ToolCallState::TimedOut) => "Timed out",
        (_, ToolCallState::Interrupted) => "Interrupted",
        _ => "Stopped",
    };
    let detail = if let Some(invocation) = mcp_display_name(name) {
        Some(invocation)
    } else if name == "dynamic_workflow" {
        args.and_then(|args| full_arg_from_keys(args, &["run_id"]))
    } else if is_exec_tool(name) {
        exec_command(name, args).or_else(|| Some(name.to_string()))
    } else if is_explore_tool(name) {
        explore_detail(name, args)
    } else if matches!(name, "web_search" | "web_fetch") {
        args.and_then(|args| arg_summary_for_tool(name, args))
    } else if has_specialized_tool_verb(name) {
        args.and_then(|args| arg_summary_for_tool(name, args))
            .or_else(|| Some(name.to_string()))
    } else {
        Some(generic_tool_invocation(name, args))
    };
    let header = render_action_header(
        action,
        detail.as_deref(),
        width,
        tool_message_tone(state, true),
        "  ",
        false,
    );
    if name == "dynamic_workflow" && looks_like_structured_payload(output) {
        header
    } else {
        join_cell_parts(header, render_output_branch(output, width, true, false))
    }
}

/// Render a completed tool call. File-editing tools (`write`/`edit`) carry
/// `before`/`after`/`file_path` in their metadata — show those as a colored
/// diff; everything else shows a status line + a few lines of output.
pub(crate) fn render_tool_end(
    name: &str,
    exit_code: i32,
    output: &str,
    meta: Option<&serde_json::Value>,
    args: Option<&serde_json::Value>,
    width: usize,
) -> String {
    let ok = exit_code == 0;

    // A failed write/edit may still carry speculative before/after metadata.
    // Never turn that into a successful-looking diff. The authoritative exit
    // status wins.
    if is_file_change_tool(name) {
        if ok {
            if let Some(rendered) = render_successful_file_change(name, meta, width) {
                return rendered;
            }
        } else {
            return render_failed_file_change(name, output, meta, args, width);
        }
    }

    if name == "dynamic_workflow" {
        return render_dynamic_workflow(output, meta, args, ok, width);
    }

    if name == "batch" {
        if let Some(rendered) = render_batch_summary(meta, args, ok, width) {
            return rendered;
        }
    }

    if name == "program" {
        if let Some(rendered) = render_program_summary(output, meta, args, ok, width) {
            return rendered;
        }
        let header = join_cell_parts(
            render_tool_header(name, ok, args, width),
            render_program_intent_preview(args, width),
        );
        return join_cell_parts(header, render_output_branch(output, width, !ok, false));
    }

    if matches!(name, "task" | "parallel_task") {
        let header = render_tool_header(name, ok, args, width);
        if let Some(summary) = render_task_tool_summary(name, output, meta, ok, width) {
            return format!("{header}{summary}");
        }
    }

    if name == "runtime" {
        if let Some(rendered) = render_runtime_summary(output, args, ok, width) {
            return rendered;
        }
    }

    if let Some(invocation) = mcp_display_name(name) {
        return render_completed_mcp(&invocation, output, ok, width);
    }

    if is_exec_tool(name) {
        return render_exec_cell(
            "Ran",
            exec_command(name, args).as_deref(),
            output,
            ok,
            width,
            true,
        );
    }

    if is_explore_tool(name) {
        return render_explore_cell(name, args, output, ok, width, false);
    }

    if matches!(name, "web_search" | "web_fetch") {
        return render_web_cell(name, args, output, ok, width, false);
    }

    render_completed_tool_output_block(name, ok, output, args, width)
}

fn render_completed_tool_output_block(
    name: &str,
    ok: bool,
    output: &str,
    args: Option<&serde_json::Value>,
    width: usize,
) -> String {
    let known = has_specialized_tool_verb(name);
    let arg = if known {
        args.and_then(|args| arg_summary_for_tool(name, args))
            .unwrap_or_default()
    } else {
        generic_tool_invocation(name, args)
    };
    let header = render_action_header(
        if known { tool_verb(name) } else { "Called" },
        (!arg.is_empty()).then_some(arg.as_str()),
        width,
        result_message_tone(ok),
        "  ",
        false,
    );
    let output = completed_structured_output(output);
    let body = render_json_output_branch(&output, width, false)
        .unwrap_or_else(|| render_output_branch(&output, width, !ok, false));
    join_cell_parts(header, body)
}

fn render_dynamic_workflow(
    output: &str,
    meta: Option<&serde_json::Value>,
    args: Option<&serde_json::Value>,
    ok: bool,
    width: usize,
) -> String {
    let workflow = meta.and_then(|meta| meta.get("dynamic_workflow"));
    let run_id = workflow
        .and_then(|workflow| workflow.get("run_id"))
        .or_else(|| args.and_then(|args| args.get("run_id")))
        .and_then(serde_json::Value::as_str);
    let header = render_action_header(
        if ok {
            "Ran workflow"
        } else {
            "Workflow failed"
        },
        run_id,
        width,
        result_message_tone(ok),
        "  ",
        false,
    );

    let Some(steps) = workflow
        .and_then(|workflow| workflow.pointer("/snapshot/steps"))
        .and_then(serde_json::Value::as_object)
    else {
        if looks_like_structured_payload(output) {
            return header;
        }
        return join_cell_parts(header, render_output_branch(output, width, !ok, false));
    };
    let mut step_ids = workflow
        .and_then(|workflow| workflow.get("history"))
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|envelope| {
            let event = envelope.get("event").unwrap_or(envelope);
            (event.get("type").and_then(serde_json::Value::as_str) == Some("step_created"))
                .then(|| event.get("step_id")?.as_str().map(str::to_string))?
        })
        .collect::<Vec<_>>();
    let mut seen = step_ids
        .iter()
        .cloned()
        .collect::<std::collections::HashSet<_>>();
    step_ids.extend(
        steps
            .keys()
            .filter(|step_id| seen.insert((*step_id).clone()))
            .cloned(),
    );
    let rows = step_ids
        .into_iter()
        .filter_map(|step_id| {
            let step = steps.get(&step_id)?;
            let status = step
                .get("status")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let success = matches!(
                status.to_ascii_lowercase().as_str(),
                "completed" | "succeeded" | "success"
            );
            Some((format!("{step_id} · {status}"), status.to_string(), success))
        })
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return join_cell_parts(header, render_output_branch(output, width, !ok, false));
    }
    let body = rows
        .into_iter()
        .enumerate()
        .map(|(index, (row, status, success))| {
            render_lifecycle_row(
                if index == 0 { "  └ " } else { "    " },
                &row,
                &status,
                success,
                width,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    let rendered = join_cell_parts(header, body);
    if !ok && looks_like_failure_diagnostic(output) {
        join_cell_parts(rendered, render_output_branch(output, width, true, false))
    } else {
        rendered
    }
}

fn looks_like_structured_payload(output: &str) -> bool {
    let output = output.trim_start();
    output.starts_with('{') || output.starts_with('[')
}

/// Pretty-print only a complete top-level object or array. Streaming fragments,
/// scalar JSON, and malformed payloads remain source-identical so a live tool
/// cell never jumps into a speculative structure.
fn completed_structured_output(output: &str) -> String {
    let trimmed = output.trim();
    if !matches!(trimmed.as_bytes().first(), Some(b'{') | Some(b'[')) {
        return output.to_string();
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
        .unwrap_or_else(|| output.to_string())
}

fn pretty_json(output: &str) -> Option<String> {
    let trimmed = output.trim();
    if !matches!(trimmed.as_bytes().first(), Some(b'{') | Some(b'[')) {
        return None;
    }
    serde_json::from_str::<serde_json::Value>(trimmed)
        .ok()
        .and_then(|value| serde_json::to_string_pretty(&value).ok())
}

fn looks_like_failure_diagnostic(output: &str) -> bool {
    if looks_like_structured_payload(output) {
        return false;
    }
    let lower = output.to_ascii_lowercase();
    [
        "error",
        "failed",
        "timed out",
        "timeout",
        "cancelled",
        "canceled",
        "interrupted",
        "denied",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

fn render_lifecycle_row(
    prefix: &str,
    label: &str,
    status: &str,
    success: bool,
    width: usize,
) -> String {
    let (glyph, tone) = lifecycle_status_presentation(status, success);
    let label = sanitize_terminal_text(label);
    let row = message_status(glyph, &label, tone, true);
    render_styled_prefixed_row(prefix, &row, width)
}

fn lifecycle_status_presentation(status: &str, success: bool) -> (&'static str, MessageTone) {
    if success {
        return ("✓", MessageTone::Success);
    }
    match status.trim().to_ascii_lowercase().as_str() {
        "running" | "started" | "in_progress" | "in-progress" => ("●", MessageTone::Active),
        "cancelled" | "canceled" | "denied" => ("⊘", MessageTone::Warning),
        "interrupted" => ("■", MessageTone::Warning),
        "timed out" | "timed_out" | "timeout" => ("◷", MessageTone::Error),
        "pending" | "queued" | "unknown" => ("○", MessageTone::Inactive),
        _ => ("✗", MessageTone::Error),
    }
}

fn render_program_intent_preview(args: Option<&serde_json::Value>, width: usize) -> String {
    let Some(preview) = summarize_program_args(args) else {
        return String::new();
    };
    let mut rows = vec![render_program_preview_row(
        "  └ ",
        "intent",
        &preview.intent,
        width,
        false,
    )];
    rows.extend(preview.details.into_iter().map(|detail| {
        render_program_preview_row("    ", detail.label, &detail.value, width, false)
    }));
    rows.join("\n")
}

fn render_program_preview_row(
    prefix: &str,
    label: &str,
    value: &str,
    width: usize,
    failed: bool,
) -> String {
    let label = Style::new().fg(TN_SUBTLE).render(&format!("{label:<7}"));
    let value = if failed {
        message_status("✗", value, MessageTone::Error, false)
    } else {
        Style::new().fg(TN_FG).render(value)
    };
    render_prefixed_row(prefix, &format!("{label}{value}"), width, failed)
}

fn render_program_summary(
    output: &str,
    meta: Option<&serde_json::Value>,
    args: Option<&serde_json::Value>,
    ok: bool,
    width: usize,
) -> Option<String> {
    let program = meta?.get("program")?;
    let calls = program.get("tool_calls")?.as_array()?;
    let detail = program
        .get("language")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            args.and_then(|args| args.get("language"))
                .and_then(serde_json::Value::as_str)
        });
    let header = render_action_header(
        if ok { "Ran program" } else { "Program failed" },
        detail,
        width,
        result_message_tone(ok),
        "  ",
        false,
    );
    let preview = render_program_intent_preview(args, width);
    let header = join_cell_parts(header, preview.clone());
    if calls.is_empty() {
        return Some(join_cell_parts(
            header,
            render_output_branch(output, width, !ok, false),
        ));
    }
    let digest = summarize_program_calls(calls)?;
    let body = render_program_preview_row(
        if preview.is_empty() { "  └ " } else { "    " },
        "actual",
        &digest.text,
        width,
        digest.has_failure,
    );
    Some(join_cell_parts(header, body))
}

fn render_batch_summary(
    meta: Option<&serde_json::Value>,
    args: Option<&serde_json::Value>,
    top_level_ok: bool,
    width: usize,
) -> Option<String> {
    let summary = BatchSummary::from_metadata(meta, args, top_level_ok)?;
    let (action, tone) = match summary.outcome {
        BatchOutcome::Complete => ("Ran batch", MessageTone::Success),
        BatchOutcome::Partial => ("Batch partially completed", MessageTone::Warning),
        BatchOutcome::Failed => ("Batch failed", MessageTone::Error),
    };
    let mut detail = format!("{} tools", summary.total_count);
    if summary.execution_mode == "parallel" {
        detail.push_str(&format!(
            " · parallel ×{}",
            summary.applied_concurrency.max(1)
        ));
    } else {
        detail.push_str(" · serial");
    }
    if summary.failure_count > 0 {
        detail.push_str(&format!(
            " · {}/{} succeeded",
            summary.success_count, summary.total_count
        ));
    }
    let header = render_action_header(action, Some(&detail), width, tone, "  ", false);

    let visible = visible_batch_items(&summary);
    let omitted = summary.items.len().saturating_sub(visible.len());
    let mut rows = visible
        .iter()
        .enumerate()
        .map(|(row_index, item)| {
            let terminal_row = row_index + 1 == visible.len() && omitted == 0;
            render_batch_item(item, if terminal_row { "  └ " } else { "  ├ " }, width)
        })
        .collect::<Vec<_>>();
    if omitted > 0 {
        let hint = Style::new()
            .fg(TN_SUBTLE)
            .render(&format!("… +{omitted} tools · Ctrl+T"));
        rows.push(render_styled_prefixed_row("  └ ", &hint, width));
    }

    Some(join_cell_parts(header, rows.join("\n")))
}

fn visible_batch_items(summary: &BatchSummary) -> Vec<&BatchItem> {
    if summary.items.len() <= MAX_BATCH_ITEM_ROWS {
        return summary.items.iter().collect();
    }

    let visible_limit = MAX_BATCH_ITEM_ROWS.saturating_sub(1);
    let mut selected = Vec::with_capacity(visible_limit);
    if summary.failure_count > 0 {
        selected.extend(
            summary
                .items
                .iter()
                .enumerate()
                .filter_map(|(index, item)| (!item.success).then_some(index))
                .take(visible_limit),
        );
    }
    for index in 0..summary.items.len() {
        if selected.len() >= visible_limit {
            break;
        }
        if !selected.contains(&index) {
            selected.push(index);
        }
    }
    if summary.failure_count == 0 && selected.len() == visible_limit {
        selected[visible_limit - 1] = summary.items.len() - 1;
    }
    selected.sort_unstable();
    selected
        .into_iter()
        .filter_map(|index| summary.items.get(index))
        .collect()
}

fn render_batch_item(item: &BatchItem, prefix: &str, width: usize) -> String {
    let tool = tool_label(&item.tool, item.args.as_ref());
    // Keep the diagnostic before a potentially long invocation so narrow
    // terminals never truncate the only explanation of a failed batch item.
    let label = if !item.success && item.exit_code != 0 {
        format!("exit {} · {tool}", item.exit_code)
    } else {
        tool
    };
    render_lifecycle_row(
        prefix,
        &label,
        if item.success { "succeeded" } else { "failed" },
        item.success,
        width,
    )
}

fn render_batch_transcript_status(
    summary: &BatchSummary,
    duration: Option<std::time::Duration>,
    width: usize,
) -> String {
    let (glyph, label, tone) = match summary.outcome {
        BatchOutcome::Complete => ("✓", String::new(), MessageTone::Success),
        BatchOutcome::Partial => (
            "!",
            format!("partial · {} failed", summary.failure_count),
            MessageTone::Warning,
        ),
        BatchOutcome::Failed => (
            "✗",
            format!("{} failed", summary.failure_count.max(1)),
            MessageTone::Error,
        ),
    };
    render_wrapped_transcript_status(glyph, &label, tone, duration, width)
}

fn is_exec_tool(name: &str) -> bool {
    matches!(name, "bash" | "shell" | "run" | "exec" | "git")
}

fn is_explore_tool(name: &str) -> bool {
    matches!(
        name,
        "read" | "cat" | "grep" | "search" | "ls" | "glob" | "find"
    )
}

fn is_file_change_tool(name: &str) -> bool {
    matches!(
        name,
        "write" | "create" | "edit" | "patch" | "apply_patch" | "delete" | "remove" | "unlink"
    )
}

fn exec_command(name: &str, args: Option<&serde_json::Value>) -> Option<String> {
    let args = args?;
    if matches!(name, "bash" | "shell" | "run" | "exec") {
        return args
            .get("command")
            .and_then(|value| value.as_str())
            .map(sanitize_terminal_text)
            .filter(|command| !command.trim().is_empty());
    }

    let summary = sanitize_terminal_text(&arg_summary(args)?);
    if summary.trim().is_empty() {
        None
    } else if summary.trim_start().starts_with("git ") {
        Some(summary)
    } else {
        Some(format!("git {summary}"))
    }
}

fn mcp_name(name: &str) -> Option<(&str, &str)> {
    let rest = name.strip_prefix("mcp__")?;
    let (server, tool) = rest.split_once("__")?;
    (!server.is_empty() && !tool.is_empty()).then_some((server, tool))
}

fn mcp_invocation(name: &str, args: Option<&serde_json::Value>) -> Option<String> {
    let (server, tool) = mcp_name(name)?;
    let mut invocation = format!("{server}.{tool}");
    if let Some(args) = args {
        let compact = serde_json::to_string(args).unwrap_or_else(|_| "{}".to_string());
        invocation.push('(');
        invocation.push_str(&compact);
        invocation.push(')');
    }
    Some(invocation)
}

fn mcp_display_name(name: &str) -> Option<String> {
    let (server, tool) = mcp_name(name)?;
    Some(format!("{server}.{tool}"))
}

fn render_completed_mcp(invocation: &str, output: &str, ok: bool, width: usize) -> String {
    let header = render_action_header(
        "Called",
        Some(invocation),
        width,
        result_message_tone(ok),
        "  ",
        false,
    );
    let output = completed_structured_output(output);
    let body = render_json_output_branch(&output, width, false)
        .unwrap_or_else(|| render_output_branch(&output, width, !ok, false));
    join_cell_parts(header, body)
}

fn render_exec_cell(
    action: &str,
    command: Option<&str>,
    output: &str,
    ok: bool,
    width: usize,
    completed: bool,
) -> String {
    let header = render_action_header(
        action,
        command,
        width,
        result_message_tone(ok),
        "  │ ",
        true,
    );
    join_cell_parts(header, render_output_branch(output, width, !ok, completed))
}

fn render_web_cell(
    name: &str,
    args: Option<&serde_json::Value>,
    output: &str,
    ok: bool,
    width: usize,
    live: bool,
) -> String {
    let (action, detail) = match name {
        "web_search" => (
            if live {
                "Searching the web"
            } else {
                "Searched the web"
            },
            args.and_then(|args| full_arg_from_keys(args, &["query"])),
        ),
        _ => (
            if live { "Fetching" } else { "Fetched" },
            args.and_then(|args| full_arg_from_keys(args, &["url"])),
        ),
    };
    let detail = detail.map(|detail| {
        if name == "web_search" && !live {
            format!("for {detail}")
        } else {
            detail
        }
    });
    let header = render_action_header(
        action,
        detail.as_deref(),
        width,
        result_message_tone(ok),
        "  ",
        false,
    );

    // Search and fetch results often contain full HTML or provider JSON. Codex
    // keeps successful cells concise and surfaces only failure details here.
    if ok {
        header
    } else {
        join_cell_parts(header, render_output_branch(output, width, true, false))
    }
}

fn explore_detail(name: &str, args: Option<&serde_json::Value>) -> Option<String> {
    let args = args?;
    match name {
        "read" | "cat" => {
            full_arg_from_keys(args, &["file_path", "path"]).map(|path| format!("Read {path}"))
        }
        "grep" | "search" => {
            let query = full_arg_from_keys(args, &["pattern", "query"])?;
            let path = full_arg_from_keys(args, &["path"]);
            Some(match path {
                Some(path) if !path.is_empty() => format!("Search {query} in {path}"),
                _ => format!("Search {query}"),
            })
        }
        "ls" | "glob" | "find" => {
            full_arg_from_keys(args, &["pattern", "path"]).map(|target| format!("List {target}"))
        }
        _ => None,
    }
}

fn render_explore_cell(
    name: &str,
    args: Option<&serde_json::Value>,
    output: &str,
    ok: bool,
    width: usize,
    live: bool,
) -> String {
    let tone = if !ok {
        MessageTone::Error
    } else if live {
        MessageTone::Active
    } else {
        MessageTone::Inactive
    };
    let header = render_action_header(
        if live { "Exploring" } else { "Explored" },
        None,
        width,
        tone,
        "  ",
        false,
    );
    let detail = explore_detail(name, args)
        .map(|detail| render_detail_branch(&detail, width, !ok))
        .unwrap_or_default();
    let mut rendered = join_cell_parts(header, detail);
    if !ok && !output.trim().is_empty() {
        rendered = join_cell_parts(
            rendered,
            render_indented_output(output, width, true, "    "),
        );
    }
    rendered
}

fn render_successful_file_change(
    name: &str,
    meta: Option<&serde_json::Value>,
    width: usize,
) -> Option<String> {
    let meta = meta?;
    let path = meta.get("file_path").and_then(|value| value.as_str())?;
    let before = meta.get("before").and_then(|value| value.as_str());
    let after = meta.get("after").and_then(|value| value.as_str());

    let (action, before, after) = match (name, before, after) {
        ("write" | "create", None, Some(after)) => ("Added", "", after),
        ("delete" | "remove" | "unlink", Some(before), None) => ("Deleted", before, ""),
        (_, Some(before), Some(after)) => ("Edited", before, after),
        _ => return None,
    };

    Some(render_compact_file_change(
        action, path, before, after, width,
    ))
}

fn render_successful_file_change_transcript(
    name: &str,
    meta: Option<&serde_json::Value>,
    width: usize,
) -> Option<String> {
    let meta = meta?;
    let path = meta.get("file_path").and_then(serde_json::Value::as_str)?;
    let before = meta.get("before").and_then(serde_json::Value::as_str);
    let after = meta.get("after").and_then(serde_json::Value::as_str);
    let (action, before, after) = match (name, before, after) {
        ("write" | "create", None, Some(after)) => ("Added", "", after),
        ("delete" | "remove" | "unlink", Some(before), None) => ("Deleted", before, ""),
        (_, Some(before), Some(after)) => ("Edited", before, after),
        _ => return None,
    };
    Some(render_full_file_change(action, path, before, after, width))
}

fn render_failed_file_change(
    name: &str,
    output: &str,
    meta: Option<&serde_json::Value>,
    args: Option<&serde_json::Value>,
    width: usize,
) -> String {
    let action = match name {
        "patch" | "apply_patch" => "Failed to apply patch",
        "write" | "create" => "Failed to write",
        "delete" | "remove" | "unlink" => "Failed to delete",
        _ => "Failed to edit",
    };
    let path = meta
        .and_then(|meta| meta.get("file_path"))
        .or_else(|| args.and_then(|args| args.get("file_path").or_else(|| args.get("path"))))
        .and_then(|value| value.as_str());
    let header = render_action_header(action, path, width, MessageTone::Error, "  ", false);
    join_cell_parts(header, render_output_branch(output, width, true, false))
}

fn render_action_header(
    action: &str,
    detail: Option<&str>,
    width: usize,
    tone: MessageTone,
    continuation_prefix: &str,
    shell_detail: bool,
) -> String {
    if width == 0 {
        return String::new();
    }

    let detail = detail
        .map(sanitize_terminal_text)
        .filter(|detail| !detail.trim().is_empty());
    if shell_detail {
        return render_shell_action_header(
            action,
            detail.as_deref(),
            width,
            tone,
            continuation_prefix,
        );
    }
    let plain = match detail.as_deref() {
        Some(detail) => format!("{action} {detail}"),
        None => action.to_string(),
    };
    let content_width = width
        .saturating_sub(visible_len(continuation_prefix).max(2))
        .max(1);
    let mut rows = if shell_detail {
        wrap_preserving_text(&plain, content_width)
    } else {
        wrap_words(&plain, content_width)
    };
    if !shell_detail {
        pack_detail_onto_first_header_row(action, &mut rows, content_width);
    }
    let max_rows = if shell_detail {
        MAX_EXEC_COMMAND_ROWS
    } else {
        MAX_COMMAND_ROWS
    };
    let rows = limit_rows_from_start(rows, max_rows);

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let line = if index == 0 {
                render_first_header_row(action, &row, tone, shell_detail)
            } else {
                let prefix = Style::new().fg(TN_SUBTLE).render(continuation_prefix);
                let text = if shell_detail {
                    highlight_shell(&row)
                } else {
                    highlight_tool_detail(&row)
                };
                format!("{prefix}{text}")
            };
            truncate_visible(&line, width)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_shell_action_header(
    action: &str,
    detail: Option<&str>,
    width: usize,
    tone: MessageTone,
    continuation_prefix: &str,
) -> String {
    let marker = message_marker(tone);
    let action = Style::new()
        .fg(header_action_color(action))
        .bold()
        .render(action);
    let header = format!("{marker} {action}");
    let Some(detail) = detail else {
        return truncate_visible(&header, width);
    };

    let continuation_width = width
        .saturating_sub(visible_len(continuation_prefix))
        .max(1);
    let first_width = width
        .saturating_sub(visible_len(&header).saturating_add(1))
        .max(1);
    let command_rows = highlight_shell_wrapped(detail, first_width, continuation_width);
    let mut lines = Vec::with_capacity(command_rows.len());
    for (index, row) in command_rows.into_iter().enumerate() {
        let row = row.trim_end();
        if index == 0 {
            lines.push(truncate_visible(&format!("{header} {row}"), width));
        } else {
            let prefix = Style::new().fg(TN_SUBTLE).render(continuation_prefix);
            lines.push(truncate_visible(&format!("{prefix}{row}"), width));
        }
    }
    limit_rows_from_start(lines, MAX_EXEC_COMMAND_ROWS).join("\n")
}

fn wrap_preserving_text(value: &str, width: usize) -> Vec<String> {
    let mut rows = Vec::new();
    for logical in value.split('\n') {
        if logical.is_empty() {
            rows.push(String::new());
            continue;
        }
        let mut from = 0;
        let total = visible_len(logical);
        while from < total {
            let next = from.saturating_add(width).min(total);
            let row = slice_visible_cols(logical, from, next);
            if row.is_empty() {
                break;
            }
            from = from.saturating_add(visible_len(&row));
            rows.push(row.trim_end().to_string());
        }
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

fn pack_detail_onto_first_header_row(action: &str, rows: &mut Vec<String>, width: usize) {
    if rows.len() < 2 || rows[0] != action {
        return;
    }
    let occupied = visible_len(action).saturating_add(1);
    if occupied >= width {
        return;
    }
    let available = width - occupied;
    let next_width = visible_len(&rows[1]);
    let head = slice_visible_cols(&rows[1], 0, available);
    if head.is_empty() {
        return;
    }
    let consumed = visible_len(&head);
    rows[0] = format!("{action} {head}");
    if consumed >= next_width {
        rows.remove(1);
    } else {
        rows[1] = slice_visible_cols(&rows[1], consumed, next_width);
    }
}

fn render_first_header_row(
    action: &str,
    row: &str,
    tone: MessageTone,
    shell_detail: bool,
) -> String {
    let marker = message_marker(tone);
    if let Some(detail) = row.strip_prefix(action) {
        let action = Style::new()
            .fg(header_action_color(action))
            .bold()
            .render(action);
        let detail = detail.strip_prefix(' ').unwrap_or(detail);
        if detail.is_empty() {
            format!("{marker} {action}")
        } else {
            let detail = if shell_detail {
                highlight_shell(detail)
            } else {
                highlight_tool_detail(detail)
            };
            format!("{marker} {action} {detail}")
        }
    } else {
        format!("{marker} {}", Style::new().fg(TN_FG).bold().render(row))
    }
}

fn render_detail_branch(detail: &str, width: usize, _error: bool) -> String {
    if width == 0 {
        return String::new();
    }
    let detail = sanitize_terminal_text(detail);
    let body_width = width.saturating_sub(4).max(1);
    let detail = highlight_explore_detail(&detail);
    let rows = limit_rows_from_start(wrap_words(&detail, body_width), MAX_COMMAND_ROWS);
    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let prefix = if index == 0 { "  └ " } else { "    " };
            render_styled_prefixed_row(prefix, &row, width)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_styled_prefixed_row(prefix: &str, row: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let prefix = Style::new().fg(TN_SUBTLE).render(prefix);
    let available = width.saturating_sub(visible_len(&prefix)).max(1);
    let row = truncate_visible(row, available);
    truncate_visible(&format!("{prefix}{row}"), width)
}

fn render_output_branch(output: &str, width: usize, error: bool, transcript_hint: bool) -> String {
    let body_width = width.saturating_sub(4).max(1);
    let rows = bounded_output_rows(output, body_width, transcript_hint);
    if rows.is_empty() {
        return String::new();
    }

    rows.into_iter()
        .enumerate()
        .map(|(index, row)| {
            let prefix = if index == 0 { "  └ " } else { "    " };
            render_prefixed_row(prefix, &row, width, error)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_json_output_branch(output: &str, width: usize, transcript_hint: bool) -> Option<String> {
    let body_width = width.saturating_sub(4).max(1);
    let rows = bounded_json_rows(output, body_width, transcript_hint)?;
    Some(
        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                let prefix = if index == 0 { "  └ " } else { "    " };
                render_styled_prefixed_row(prefix, &row, width)
            })
            .collect::<Vec<_>>()
            .join("\n"),
    )
}

fn bounded_json_rows(output: &str, row_width: usize, transcript_hint: bool) -> Option<Vec<String>> {
    let pretty = pretty_json(output)?;
    let mut logical_lines = pretty.split('\n').map(str::to_string).collect::<Vec<_>>();
    let mut omitted_before_wrap = 0usize;
    if logical_lines.len() > MAX_LOGICAL_OUTPUT_LINES {
        omitted_before_wrap = logical_lines.len() - MAX_LOGICAL_OUTPUT_LINES;
        let tail = logical_lines.split_off(logical_lines.len() - 5);
        logical_lines.truncate(5);
        logical_lines.extend(tail);
    }

    let rows = logical_lines
        .into_iter()
        .flat_map(|line| highlight_json_wrapped(&line, row_width))
        .collect::<Vec<_>>();
    if rows.len() <= MAX_OUTPUT_ROWS && omitted_before_wrap == 0 {
        return Some(rows);
    }

    let head_count = rows.len().min(2);
    let tail_count = rows.len().saturating_sub(head_count).min(2);
    let omitted = omitted_before_wrap + rows.len().saturating_sub(head_count + tail_count);
    let mut bounded = rows[..head_count].to_vec();
    let hint = if transcript_hint {
        format!("… +{omitted} lines · Ctrl+T")
    } else {
        format!("… +{omitted} lines")
    };
    bounded.push(Style::new().fg(TN_SUBTLE).render(&hint));
    bounded.extend_from_slice(&rows[rows.len().saturating_sub(tail_count)..]);
    Some(bounded)
}

fn render_indented_output(output: &str, width: usize, error: bool, prefix: &str) -> String {
    let body_width = width.saturating_sub(visible_len(prefix)).max(1);
    bounded_output_rows(output, body_width, false)
        .into_iter()
        .map(|row| render_prefixed_row(prefix, &row, width, error))
        .collect::<Vec<_>>()
        .join("\n")
}

fn bounded_output_rows(output: &str, row_width: usize, transcript_hint: bool) -> Vec<String> {
    let output = sanitize_terminal_text(output);
    let mut logical_lines = output
        .split('\n')
        .map(|line| line.trim_end_matches('\r').to_string())
        .collect::<Vec<_>>();
    while logical_lines.last().is_some_and(|line| line.is_empty()) {
        logical_lines.pop();
    }
    if logical_lines.iter().all(|line| line.trim().is_empty()) {
        return Vec::new();
    }

    let mut omitted_before_wrap = 0usize;
    if logical_lines.len() > MAX_LOGICAL_OUTPUT_LINES {
        omitted_before_wrap = logical_lines.len() - MAX_LOGICAL_OUTPUT_LINES;
        let tail = logical_lines.split_off(logical_lines.len() - 5);
        logical_lines.truncate(5);
        logical_lines.extend(tail);
    }

    let rows = logical_lines
        .into_iter()
        .flat_map(|line| wrap_preserving_text(&line, row_width))
        .collect::<Vec<_>>();
    if rows.len() <= MAX_OUTPUT_ROWS && omitted_before_wrap == 0 {
        return rows;
    }

    let head_count = rows.len().min(2);
    let tail_count = rows.len().saturating_sub(head_count).min(2);
    let omitted = omitted_before_wrap + rows.len().saturating_sub(head_count + tail_count);
    let mut bounded = rows[..head_count].to_vec();
    bounded.push(if transcript_hint {
        format!("… +{omitted} lines · Ctrl+T")
    } else {
        format!("… +{omitted} lines")
    });
    bounded.extend_from_slice(&rows[rows.len().saturating_sub(tail_count)..]);
    bounded
}

fn render_prefixed_row(prefix: &str, row: &str, width: usize, _error: bool) -> String {
    if width == 0 {
        return String::new();
    }
    let prefix = Style::new().fg(TN_SUBTLE).render(prefix);
    let available = width.saturating_sub(visible_len(prefix.as_str())).max(1);
    let row = truncate_visible(row, available);
    let row = Style::new().fg(TN_GRAY).render(&row);
    truncate_visible(&format!("{prefix}{row}"), width)
}

fn limit_rows_from_start(mut rows: Vec<String>, max: usize) -> Vec<String> {
    if rows.len() <= max {
        return rows;
    }
    let omitted = rows.len() - (max - 1);
    rows.truncate(max - 1);
    rows.push(format!("… +{omitted} lines"));
    rows
}

fn sanitize_terminal_text(value: &str) -> String {
    strip_ansi(value)
        .chars()
        .filter_map(|ch| match ch {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect()
}

fn join_cell_parts(head: String, tail: String) -> String {
    match (head.is_empty(), tail.is_empty()) {
        (true, _) => tail,
        (_, true) => head,
        (false, false) => format!("{head}\n{tail}"),
    }
}

fn render_task_tool_summary(
    name: &str,
    output: &str,
    meta: Option<&serde_json::Value>,
    ok: bool,
    width: usize,
) -> Option<String> {
    let meta = meta?;
    match name {
        "task" => render_single_task_summary(output, meta, ok, width),
        "parallel_task" => render_parallel_task_summary(meta, ok, width),
        _ => None,
    }
}

fn render_runtime_summary(
    output: &str,
    args: Option<&serde_json::Value>,
    ok: bool,
    width: usize,
) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(output).ok()?;
    let object = value.as_object()?;
    let results = object
        .get("results")
        .and_then(serde_json::Value::as_array)?;
    let count = object
        .get("count")
        .and_then(serde_json::Value::as_u64)
        .map(|count| count as usize)
        .unwrap_or(results.len());
    let worker = object
        .get("worker")
        .and_then(serde_json::Value::as_str)
        .or_else(|| args.and_then(|args| args.get("worker")?.as_str()));
    let succeeded = results
        .iter()
        .filter(|result| {
            result
                .get("state")
                .and_then(serde_json::Value::as_str)
                .is_some_and(|state| {
                    matches!(
                        state.to_ascii_lowercase().as_str(),
                        "completed" | "succeeded" | "success" | "done"
                    )
                })
                && result.get("error").is_none_or(serde_json::Value::is_null)
        })
        .count();
    let header_detail = match worker {
        Some(worker) => format!("{succeeded}/{count} tasks via {}", truncate(worker, 36)),
        None => format!("{succeeded}/{count} tasks"),
    };
    let header = render_action_header(
        if ok { "Used Runtime" } else { "Runtime failed" },
        Some(&header_detail),
        width,
        result_message_tone(ok),
        "  ",
        false,
    );
    let mut rows = results
        .iter()
        .enumerate()
        .map(|(index, result)| {
            let state = result
                .get("state")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("unknown");
            let error = result.get("error").filter(|value| !value.is_null());
            let success = error.is_none()
                && matches!(
                    state.to_ascii_lowercase().as_str(),
                    "completed" | "succeeded" | "success" | "done"
                );
            let id = result
                .get("invocationId")
                .and_then(serde_json::Value::as_str)
                .map(|id| format!(" · {}", truncate(id, 18)))
                .unwrap_or_default();
            (format!("task {}{id} · {state}", index + 1), state, success)
        })
        .collect::<Vec<_>>();
    if object
        .get("partial")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
    {
        rows.push((
            "partial results returned after timeout".to_string(),
            "timed out",
            false,
        ));
    }
    let body = rows
        .into_iter()
        .take(MAX_OUTPUT_ROWS)
        .enumerate()
        .map(|(index, (row, status, success))| {
            render_lifecycle_row(
                if index == 0 { "  └ " } else { "    " },
                &row,
                status,
                success,
                width,
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    Some(join_cell_parts(header, body))
}

fn render_single_task_summary(
    output: &str,
    meta: &serde_json::Value,
    ok: bool,
    width: usize,
) -> Option<String> {
    let agent = meta
        .get("agent")
        .and_then(|v| v.as_str())
        .unwrap_or("agent");
    let task_id = meta.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
    let success = meta.get("success").and_then(|v| v.as_bool()).unwrap_or(ok);
    let status = if success { "completed" } else { "failed" };
    let output_bytes = meta.get("output_bytes").and_then(|v| v.as_u64());
    let artifact = meta.get("artifact_uri").and_then(|v| v.as_str());
    let mut rows = vec![TaskSummaryRow::header(
        format!("Task {status} · {agent}{}", task_id_suffix(task_id)),
        success,
    )];
    if let Some(excerpt) = task_child_excerpt(output) {
        rows.extend(excerpt.lines().map(TaskSummaryRow::child));
    } else if output_bytes == Some(0) {
        rows.push(TaskSummaryRow::child(
            "no child text output; using plan/status for synthesis",
        ));
    } else {
        rows.push(TaskSummaryRow::child(
            "child output stored in task artifact",
        ));
    }
    if let Some(uri) = artifact {
        rows.push(TaskSummaryRow::child(format!(
            "artifact: {}",
            truncate(uri, 96)
        )));
    }
    Some(render_task_rows(&rows, width))
}

fn render_parallel_task_summary(
    meta: &serde_json::Value,
    ok: bool,
    width: usize,
) -> Option<String> {
    let results = meta.get("results").and_then(|v| v.as_array())?;
    if results.is_empty() {
        return None;
    }
    let done = results
        .iter()
        .filter(|r| r.get("success").and_then(|v| v.as_bool()).unwrap_or(ok))
        .count();
    let recovered = meta
        .get("recovered_task_count")
        .and_then(|value| value.as_u64())
        .unwrap_or_default();
    let recovery = if recovered > 0 {
        format!(" · {recovered} recovered")
    } else {
        String::new()
    };
    let mut rows = vec![TaskSummaryRow::header(
        format!("{done}/{} agents succeeded{recovery}", results.len()),
        ok,
    )];
    for result in results.iter().take(4) {
        let success = result
            .get("success")
            .and_then(|v| v.as_bool())
            .unwrap_or(ok);
        let agent = result
            .get("agent")
            .and_then(|v| v.as_str())
            .unwrap_or("agent");
        let task_id = result.get("task_id").and_then(|v| v.as_str()).unwrap_or("");
        let output_bytes = result.get("output_bytes").and_then(|v| v.as_u64());
        let formatted = result
            .get("output_excerpt")
            .or_else(|| result.get("output"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        let retries = result
            .get("retry_attempts")
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        let retry = if retries > 0 { " · retried" } else { "" };
        let detail = if let Some(excerpt) = task_child_excerpt(formatted) {
            truncate(&excerpt.replace('\n', " "), 120)
        } else if output_bytes == Some(0) {
            "no child text output".to_string()
        } else {
            "output stored in artifact".to_string()
        };
        rows.push(TaskSummaryRow::result(
            format!("{agent}{}{retry} · {detail}", task_id_suffix(task_id)),
            success,
        ));
    }
    let more = results.len().saturating_sub(4);
    if more > 0 {
        rows.push(TaskSummaryRow::child(format!(
            "+{more} more agent result(s)"
        )));
    }
    Some(render_task_rows(&rows, width))
}

#[derive(Debug, Clone)]
struct TaskSummaryRow {
    text: String,
    glyph: char,
    glyph_color: Color,
    text_color: Color,
}

impl TaskSummaryRow {
    fn header(text: impl Into<String>, ok: bool) -> Self {
        Self::status(text, ok, TN_FG)
    }

    fn result(text: impl Into<String>, ok: bool) -> Self {
        Self::status(text, ok, TN_GRAY)
    }

    fn child(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            glyph: '·',
            glyph_color: TN_SUBTLE,
            text_color: TN_GRAY,
        }
    }

    fn status(text: impl Into<String>, ok: bool, text_color: Color) -> Self {
        Self {
            text: text.into(),
            glyph: if ok { '✓' } else { '✗' },
            glyph_color: if ok { TN_GREEN } else { TN_RED },
            text_color,
        }
    }
}

fn render_task_rows(rows: &[TaskSummaryRow], width: usize) -> String {
    let block = rows
        .iter()
        .enumerate()
        .map(|(index, row)| {
            let prefix = if index == 0 { "  └ " } else { "    " };
            let glyph = Style::new()
                .fg(row.glyph_color)
                .render(&row.glyph.to_string());
            let text = Style::new().fg(row.text_color).render(&row.text);
            render_prefixed_row(prefix, &format!("{glyph} {text}"), width, false)
        })
        .collect::<Vec<_>>()
        .join("\n");
    if block.is_empty() {
        String::new()
    } else {
        format!("\n{block}")
    }
}

fn task_child_excerpt(formatted: &str) -> Option<String> {
    let tail = formatted
        .split_once("Output:\n")
        .map(|(_, tail)| tail)
        .or_else(|| {
            formatted
                .split_once("Output excerpt:")
                .map(|(_, tail)| tail)
        })
        .unwrap_or(formatted);
    let lines = tail
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .take(3)
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lines.is_empty() {
        None
    } else {
        Some(lines.join("\n"))
    }
}

fn task_id_suffix(task_id: &str) -> String {
    if task_id.is_empty() {
        String::new()
    } else {
        format!(" · {}", truncate(task_id, 24))
    }
}

/// Codex-style past-tense action verb for a completed tool call.
pub(crate) fn tool_verb(name: &str) -> &str {
    match name {
        "bash" | "shell" | "run" | "exec" => "Ran",
        "read" | "cat" => "Read",
        "write" | "create" => "Wrote",
        "edit" | "patch" | "apply_patch" => "Edited",
        "grep" | "search" => "Searched",
        "ls" | "glob" | "find" => "Listed",
        "web_search" => "Searched web",
        "web_fetch" => "Fetched",
        "task" | "parallel_task" => "Delegated",
        "runtime" => "Used Runtime",
        "git" => "Ran git",
        "batch" => "Ran batch",
        "program" => "Ran program",
        "dynamic_workflow" => "Ran workflow",
        "generate_object" => "Generated object",
        "search_skills" => "Searched skills",
        "skill" | "Skill" => "Used skill",
        other => other,
    }
}

pub(crate) fn tool_running_verb(name: &str) -> &str {
    match name {
        "bash" | "shell" | "run" | "exec" => "Running",
        "read" | "cat" => "Reading",
        "write" | "create" => "Writing",
        "edit" | "patch" | "apply_patch" => "Editing",
        "grep" | "search" | "web_search" => "Searching",
        "ls" | "glob" | "find" => "Listing",
        "web_fetch" => "Fetching",
        "task" | "parallel_task" => "Delegating",
        "runtime" => "Running Runtime",
        "git" => "Running git",
        "batch" => "Running batch",
        "program" => "Running program",
        "dynamic_workflow" => "Running workflow",
        "generate_object" => "Generating object",
        "search_skills" => "Searching skills",
        "skill" | "Skill" => "Using skill",
        _ => "Using",
    }
}

fn has_specialized_tool_verb(name: &str) -> bool {
    matches!(
        name,
        "bash"
            | "shell"
            | "run"
            | "exec"
            | "read"
            | "cat"
            | "write"
            | "create"
            | "edit"
            | "patch"
            | "apply_patch"
            | "grep"
            | "search"
            | "ls"
            | "glob"
            | "find"
            | "web_search"
            | "web_fetch"
            | "task"
            | "parallel_task"
            | "runtime"
            | "git"
            | "batch"
            | "program"
            | "dynamic_workflow"
            | "generate_object"
            | "search_skills"
            | "skill"
            | "Skill"
    )
}

fn generic_tool_invocation(name: &str, args: Option<&serde_json::Value>) -> String {
    let Some(args) = args else {
        return name.to_string();
    };
    let detail = match args {
        serde_json::Value::Object(object) => {
            let mut keys = Vec::with_capacity(object.len());
            for key in [
                "command",
                "file_path",
                "path",
                "query",
                "url",
                "pattern",
                "description",
                "prompt",
            ] {
                if object.contains_key(key) {
                    keys.push(key);
                }
            }
            for key in object.keys() {
                if !keys.contains(&key.as_str()) {
                    keys.push(key);
                }
            }
            let mut fields = keys
                .into_iter()
                .take(4)
                .filter_map(|key| {
                    object
                        .get(key)
                        .map(|value| format!("{key}={}", compact_generic_arg_value(value)))
                })
                .collect::<Vec<_>>();
            let omitted = object.len().saturating_sub(fields.len());
            if omitted > 0 {
                fields.push(format!("…+{omitted}"));
            }
            fields.join(", ")
        }
        value => format!("args={}", compact_generic_arg_value(value)),
    };
    format!(
        "{name}({})",
        truncate(&sanitize_terminal_text(&detail), 200)
    )
}

fn compact_generic_arg_value(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(value)
            if !value.is_empty()
                && !value.chars().any(|ch| {
                    ch.is_whitespace() || matches!(ch, ',' | '(' | ')' | '=' | '"' | '\'')
                }) =>
        {
            truncate(value, 80)
        }
        value => serde_json::to_string(value)
            .map(|value| truncate(&value, 100))
            .unwrap_or_else(|_| "null".to_string()),
    }
}

pub(crate) fn tool_label(name: &str, args: Option<&serde_json::Value>) -> String {
    if let Some(invocation) = mcp_invocation(name, args) {
        return invocation;
    }
    if !has_specialized_tool_verb(name) {
        return generic_tool_invocation(name, args);
    }
    let target = args
        .and_then(|args| arg_summary_for_tool(name, args))
        .unwrap_or_default();
    let display = match name {
        "bash" | "shell" | "run" | "exec" => "Bash",
        "read" | "cat" => "Read",
        "write" | "create" => "Write",
        "edit" | "patch" | "apply_patch" => "Update",
        "grep" | "search" => "Grep",
        "ls" => "List",
        "glob" | "find" => "Glob",
        "web_search" => "WebSearch",
        "web_fetch" => "WebFetch",
        "task" | "parallel_task" => "Task",
        "runtime" => "Runtime",
        "git" => "Git",
        "batch" => "Batch",
        "program" => "Program",
        "dynamic_workflow" => "Workflow",
        "generate_object" => "GenerateObject",
        "search_skills" => "SearchSkills",
        "skill" | "Skill" => "Skill",
        other => other,
    };
    if target.is_empty() {
        display.to_string()
    } else {
        format!("{display}({target})")
    }
}

/// Approval prompts need enough information to make a safe decision. Compact
/// activity labels intentionally summarize arguments, but a shell command,
/// proposed file change, or nested batch must not hide the operation being
/// authorized.
pub(crate) fn tool_approval_label(name: &str, args: Option<&serde_json::Value>) -> String {
    if is_exec_tool(name) {
        if let Some(command) = exec_command(name, args) {
            let display = if name == "git" { "Git" } else { "Bash" };
            return format!("{display}({})", command.replace('\n', " ↵ "));
        }
    }

    tool_label(name, args)
}

fn prefixed_preview(value: &str, prefix: &str) -> String {
    value
        .split('\n')
        .map(|line| format!("{prefix}{line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn batch_approval_preview(args: &serde_json::Value) -> Option<String> {
    let invocations = args.get("invocations")?.as_array()?;
    let rows = invocations
        .iter()
        .enumerate()
        .filter_map(|(index, invocation)| {
            let name = invocation.get("tool")?.as_str()?;
            let args = invocation.get("args");
            let label = tool_approval_label(name, args);
            let detail = tool_approval_preview(name, args, 240);
            let combined = if detail.is_empty() {
                label
            } else {
                format!("{label}\n{detail}")
            };
            let mut lines = combined.lines();
            let first = lines.next().unwrap_or(name);
            let mut row = format!("{}. {first}", index + 1);
            for line in lines {
                row.push('\n');
                row.push_str("   ");
                row.push_str(line);
            }
            Some(row)
        })
        .collect::<Vec<_>>();
    (!rows.is_empty()).then(|| rows.join("\n"))
}

fn bound_approval_preview(value: &str, max_lines: usize) -> String {
    let value = sanitize_terminal_text(value);
    let lines = value.lines().collect::<Vec<_>>();
    let shown = lines.len().min(max_lines);
    let mut bounded = lines[..shown]
        .iter()
        .map(|line| truncate(line, 240))
        .collect::<Vec<_>>();
    if lines.len() > shown {
        bounded.push(format!("… +{} lines", lines.len() - shown));
    }
    bounded.join("\n")
}

fn tool_approval_preview(name: &str, args: Option<&serde_json::Value>, width: usize) -> String {
    let Some(args) = args else {
        return String::new();
    };
    let preview = match name {
        "patch" | "apply_patch" => args
            .get("diff")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        "write" | "create" => args
            .get("content")
            .and_then(serde_json::Value::as_str)
            .map(|content| prefixed_preview(content, "+")),
        "edit" => {
            let before = args.get("old_string").and_then(serde_json::Value::as_str);
            let after = args.get("new_string").and_then(serde_json::Value::as_str);
            match (before, after) {
                (Some(before), Some(after)) => Some(format!(
                    "--- current\n+++ proposed\n{}\n{}",
                    prefixed_preview(before, "-"),
                    prefixed_preview(after, "+")
                )),
                _ => None,
            }
        }
        "batch" => batch_approval_preview(args),
        _ if !has_specialized_tool_verb(name) => serde_json::to_string_pretty(args).ok(),
        _ => None,
    };
    let Some(preview) = preview else {
        return String::new();
    };
    bound_approval_preview(&preview, 16)
        .lines()
        .map(|line| {
            let color = if line.starts_with('-') && !line.starts_with("---") {
                TN_RED
            } else if line.starts_with('+') && !line.starts_with("+++") {
                TN_GREEN
            } else {
                TN_GRAY
            };
            let prefix = Style::new().fg(TN_SUBTLE).render("  │ ");
            let available = width.saturating_sub(4).max(1);
            let text = Style::new()
                .fg(color)
                .render(&truncate_visible(line, available));
            truncate_visible(&format!("{prefix}{text}"), width)
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn render_tool_header(
    name: &str,
    ok: bool,
    args: Option<&serde_json::Value>,
    width: usize,
) -> String {
    let arg = args
        .and_then(|args| arg_summary_for_tool(name, args))
        .unwrap_or_default();
    render_action_header(
        tool_verb(name),
        (!arg.is_empty()).then_some(arg.as_str()),
        width,
        result_message_tone(ok),
        "  ",
        false,
    )
}

pub(crate) fn render_live_tool_activity(
    name: &str,
    args: Option<&serde_json::Value>,
    output: &str,
    width: usize,
    active: bool,
    state: ToolCallState,
) -> String {
    let failed = matches!(
        state,
        ToolCallState::Failed
            | ToolCallState::Denied
            | ToolCallState::TimedOut
            | ToolCallState::Interrupted
    );
    let tone = tool_message_tone(state, active);

    if let Some(invocation) = mcp_display_name(name) {
        let action = match state {
            ToolCallState::AwaitingApproval => "Awaiting approval for",
            ToolCallState::Denied => "Denied",
            ToolCallState::TimedOut => "Timed out",
            ToolCallState::Interrupted => "Interrupted",
            ToolCallState::Succeeded | ToolCallState::Failed => "Called",
            ToolCallState::Preparing | ToolCallState::Running => "Calling",
        };
        let header = render_action_header(action, Some(&invocation), width, tone, "  ", false);
        return join_cell_parts(header, render_output_branch(output, width, failed, false));
    }

    if name == "dynamic_workflow" {
        let action = match state {
            ToolCallState::Preparing => "Preparing workflow",
            ToolCallState::AwaitingApproval => "Awaiting approval for workflow",
            ToolCallState::Running => "Running workflow",
            ToolCallState::Succeeded => "Ran workflow",
            ToolCallState::Failed => "Workflow failed",
            ToolCallState::Denied => "Denied workflow",
            ToolCallState::TimedOut => "Timed out workflow",
            ToolCallState::Interrupted => "Interrupted workflow",
        };
        let run_id = args.and_then(|args| full_arg_from_keys(args, &["run_id"]));
        let header = render_action_header(action, run_id.as_deref(), width, tone, "  ", false);
        // Workflow output is a structured host artifact. While the call is
        // active it may contain partial JSON snapshots that are noisy and can
        // expose implementation details; the terminal renderer replaces this
        // card with the authoritative step summary. Only terminal failures
        // need their diagnostic branch before that replacement arrives.
        return if failed {
            join_cell_parts(header, render_output_branch(output, width, true, false))
        } else {
            header
        };
    }

    if is_exec_tool(name) {
        let action = match state {
            ToolCallState::Preparing => "Preparing",
            ToolCallState::AwaitingApproval => "Awaiting approval for",
            ToolCallState::Running => "Running",
            ToolCallState::Succeeded | ToolCallState::Failed => "Ran",
            ToolCallState::Denied => "Denied",
            ToolCallState::TimedOut => "Timed out",
            ToolCallState::Interrupted => "Interrupted",
        };
        let command = exec_command(name, args);
        return render_exec_cell_with_marker(
            action,
            command.as_deref().or(Some(name)),
            output,
            failed,
            width,
            tone,
        );
    }

    if is_explore_tool(name) {
        if state == ToolCallState::AwaitingApproval {
            let detail = explore_detail(name, args).unwrap_or_else(|| name.to_string());
            return render_action_header(
                "Awaiting approval for",
                Some(&detail),
                width,
                tone,
                "  ",
                false,
            );
        }
        let mut cell = render_explore_cell(name, args, output, !failed, width, true);
        if tone != MessageTone::Success {
            cell = recolor_first_marker(&cell, tone);
        }
        return cell;
    }

    if matches!(name, "web_search" | "web_fetch") {
        if state == ToolCallState::AwaitingApproval {
            let detail = args
                .and_then(|args| arg_summary_for_tool(name, args))
                .unwrap_or_else(|| name.to_string());
            return render_action_header(
                "Awaiting approval for",
                Some(&detail),
                width,
                tone,
                "  ",
                false,
            );
        }
        let mut cell = render_web_cell(name, args, output, !failed, width, true);
        if tone != MessageTone::Success {
            cell = recolor_first_marker(&cell, tone);
        }
        return cell;
    }

    if state == ToolCallState::AwaitingApproval {
        let preview = tool_approval_preview(name, args, width);
        if !preview.is_empty() {
            let detail = args
                .and_then(|args| arg_summary_for_tool(name, args))
                .unwrap_or_else(|| name.to_string());
            let header = render_action_header(
                "Awaiting approval for",
                Some(&detail),
                width,
                tone,
                "  ",
                false,
            );
            return join_cell_parts(header, preview);
        }
    }

    let known = has_specialized_tool_verb(name);
    let action = match state {
        ToolCallState::Preparing => "Preparing",
        ToolCallState::AwaitingApproval => "Awaiting approval for",
        ToolCallState::Running if !known => "Calling",
        ToolCallState::Running => tool_running_verb(name),
        ToolCallState::Succeeded | ToolCallState::Failed if !known => "Called",
        ToolCallState::Succeeded | ToolCallState::Failed => tool_verb(name),
        ToolCallState::Denied => "Denied",
        ToolCallState::TimedOut => "Timed out",
        ToolCallState::Interrupted => "Interrupted",
    };
    let arg = if known {
        args.and_then(|args| arg_summary_for_tool(name, args))
    } else {
        Some(generic_tool_invocation(name, args))
    };
    let detail = arg.as_deref().or_else(|| {
        matches!(
            state,
            ToolCallState::Preparing
                | ToolCallState::AwaitingApproval
                | ToolCallState::Denied
                | ToolCallState::TimedOut
                | ToolCallState::Interrupted
        )
        .then_some(name)
    });
    let header = render_action_header(action, detail, width, tone, "  ", false);
    let header = if name == "program" {
        join_cell_parts(header, render_program_intent_preview(args, width))
    } else {
        header
    };
    join_cell_parts(header, render_output_branch(output, width, failed, false))
}

fn render_exec_cell_with_marker(
    action: &str,
    command: Option<&str>,
    output: &str,
    failed: bool,
    width: usize,
    tone: MessageTone,
) -> String {
    let header = render_action_header(action, command, width, tone, "  │ ", true);
    join_cell_parts(header, render_output_branch(output, width, failed, false))
}

fn recolor_first_marker(rendered: &str, tone: MessageTone) -> String {
    let from = message_marker(MessageTone::Success);
    let to = message_marker(tone);
    rendered.replacen(&from, &to, 1)
}

/// Extract a one-line summary of a tool's primary argument.
pub(crate) fn arg_summary(args: &serde_json::Value) -> Option<String> {
    // parallel_task / task: surface the sub-task descriptions so the user can
    // see what's actually being dispatched (not just "Task").
    if let Some(tasks) = args.get("tasks").and_then(|v| v.as_array()) {
        if let Some(summary) = summarize_tasks(tasks, args.get("worker").and_then(|v| v.as_str())) {
            return Some(summary);
        }
    }
    if let Some(invocations) = args.get("invocations").and_then(|v| v.as_array()) {
        let calls = invocations
            .iter()
            .filter_map(|invocation| {
                let tool = invocation.get("tool").and_then(|value| value.as_str())?;
                let detail = invocation
                    .get("args")
                    .and_then(|args| arg_summary_for_tool(tool, args))
                    .unwrap_or_default();
                Some(if detail.is_empty() {
                    tool.to_string()
                } else {
                    format!("{tool}({})", truncate(&detail, 48))
                })
            })
            .collect::<Vec<_>>();
        if !calls.is_empty() {
            let head = calls.iter().take(3).cloned().collect::<Vec<_>>().join(", ");
            let more = calls.len().saturating_sub(3);
            let tail = if more > 0 {
                format!(" +{more} more")
            } else {
                String::new()
            };
            return Some(truncate(
                &format!("{} tools: {head}{tail}", calls.len()),
                180,
            ));
        }
    }
    if let Some(command) = args.get("command").and_then(|v| v.as_str()) {
        let sub = args.get("subcommand").and_then(|v| v.as_str());
        let target = args
            .get("target")
            .or_else(|| args.get("ref"))
            .or_else(|| args.get("name"))
            .or_else(|| args.get("path"))
            .and_then(|v| v.as_str());
        let mut parts = vec![command.to_string()];
        if let Some(sub) = sub {
            parts.push(sub.to_string());
        }
        if let Some(target) = target {
            parts.push(target.to_string());
        }
        return Some(truncate(&parts.join(" "), 120));
    }
    for key in [
        "file_path",
        "path",
        "pattern",
        "query",
        "url",
        "description",
        "prompt",
        "old_string",
        "skill_name",
        "run_id",
        "type",
    ] {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            let v = v.replace('\n', " ");
            return Some(truncate(v.trim(), 120));
        }
    }
    None
}

pub(crate) fn arg_summary_for_tool(name: &str, args: &serde_json::Value) -> Option<String> {
    match name {
        "grep" | "search" => arg_from_keys(args, &["pattern", "path"]),
        "web_search" => arg_from_keys(args, &["query"]),
        "web_fetch" => arg_from_keys(args, &["url"]),
        "read" | "cat" | "write" | "create" | "edit" | "patch" | "apply_patch" => {
            arg_from_keys(args, &["file_path", "path"]).or_else(|| arg_summary(args))
        }
        "ls" | "glob" | "find" => arg_from_keys(args, &["pattern", "path"]),
        "skill" | "Skill" => arg_from_keys(args, &["skill_name", "description", "prompt"]),
        "dynamic_workflow" => arg_from_keys(args, &["run_id"]),
        "generate_object" => arg_from_keys(args, &["schema_name", "prompt"]),
        "search_skills" => arg_from_keys(args, &["query"]),
        _ => arg_summary(args),
    }
}

fn arg_from_keys(args: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        args.get(*key).and_then(|v| v.as_str()).map(|v| {
            let v = v.replace('\n', " ");
            truncate(v.trim(), 120)
        })
    })
}

fn full_arg_from_keys(args: &serde_json::Value, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        args.get(*key)
            .and_then(|value| value.as_str())
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

fn summarize_tasks(tasks: &[serde_json::Value], worker: Option<&str>) -> Option<String> {
    let descs = tasks
        .iter()
        .filter_map(|task| {
            task.as_str().or_else(|| {
                task.get("description")
                    .or_else(|| task.get("prompt"))
                    .or_else(|| task.get("task"))
                    .or_else(|| task.get("query"))
                    .or_else(|| task.get("title"))
                    .or_else(|| task.get("focus"))
                    .and_then(|v| v.as_str())
            })
        })
        .map(|s| truncate(&s.replace('\n', " "), 40))
        .collect::<Vec<_>>();
    if descs.is_empty() {
        return None;
    }
    let head = descs.iter().take(2).cloned().collect::<Vec<_>>().join("; ");
    let more = descs.len().saturating_sub(2);
    let tail = if more > 0 {
        format!(" +{more} more")
    } else {
        String::new()
    };
    let worker = worker
        .filter(|worker| !worker.trim().is_empty())
        .map(|worker| format!(" via {}", truncate(worker.trim(), 28)))
        .unwrap_or_default();
    Some(format!("{} tasks{worker}: {head}{tail}", descs.len()))
}

/// IDE-style unified diff: `└ path (+a -d)` header, then hunks with context
/// lines (dim, no marker), `-`/`+` changes, `⋮` between hunks, and long lines
/// wrapped with the code indented under a blank gutter.
#[cfg(test)]
fn render_diff(path: &str, before: &str, after: &str, width: usize) -> String {
    render_compact_file_change("Edited", path, before, after, width)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_visible_lines_bounded(rendered: &str, width: usize) {
        for line in rendered.lines() {
            assert!(
                a3s_tui::style::visible_len(line) <= width,
                "line exceeds width {width}: {:?}",
                a3s_tui::style::strip_ansi(line)
            );
        }
    }

    #[test]
    fn highlight_shell_colors_tokens_and_preserves_text() {
        let command = "curl -s --method=POST http://x | jq '.items' LIMIT=10";
        let s = highlight_shell(command);
        // Styling was applied (escape sequences present)...
        assert!(s.contains('\u{1b}'));
        assert!(s.contains(&tool_style::TOOL_PROGRAM_COLOR.fg_ansi()));
        assert!(s.contains(&tool_style::TOOL_FLAG_COLOR.fg_ansi()));
        assert!(s.contains(&tool_style::TOOL_STRING_COLOR.fg_ansi()));
        // ...but the visible text is unchanged (single-spaced tokens).
        assert_eq!(a3s_tui::style::strip_ansi(&s), command);
        assert_eq!(highlight_shell(""), "");
    }

    #[test]
    fn tool_details_color_paths_flags_strings_and_numbers() {
        let detail = "Read ./src/main.rs --limit '20 lines' 20";
        let rendered = highlight_tool_detail(detail);
        assert_eq!(a3s_tui::style::strip_ansi(&rendered), detail);
        assert!(rendered.contains(&tool_style::TOOL_ACTION_COLOR.fg_ansi()));
        assert!(rendered.contains(&tool_style::TOOL_FLAG_COLOR.fg_ansi()));
        assert!(rendered.contains(&tool_style::TOOL_STRING_COLOR.fg_ansi()));
    }

    #[test]
    fn terminal_states_have_distinct_glyphs_and_neutral_labels() {
        for (state, exit_code, glyph, label, tone) in [
            (
                ToolCallState::Succeeded,
                Some(0),
                "✓",
                "",
                MessageTone::Success,
            ),
            (
                ToolCallState::Failed,
                Some(7),
                "✗",
                "(7)",
                MessageTone::Error,
            ),
            (
                ToolCallState::Denied,
                Some(1),
                "⊘",
                "denied",
                MessageTone::Warning,
            ),
            (
                ToolCallState::TimedOut,
                Some(124),
                "◷",
                "timed out",
                MessageTone::Error,
            ),
            (
                ToolCallState::Interrupted,
                Some(130),
                "■",
                "interrupted",
                MessageTone::Warning,
            ),
        ] {
            let rendered = render_transcript_terminal_status(state, exit_code, None, 32);
            let expected = if label.is_empty() {
                glyph.to_string()
            } else {
                format!("{glyph} {label}")
            };
            assert_eq!(strip_ansi(&rendered), expected, "state={state:?}");
            assert!(
                rendered.contains(&Style::new().fg(tone.color()).bold().render(glyph)),
                "state={state:?}: {rendered:?}"
            );
            if !label.is_empty() {
                assert!(
                    rendered.contains(&Style::new().fg(TN_GRAY).render(label)),
                    "state={state:?}: {rendered:?}"
                );
                assert!(
                    !rendered.contains(&Style::new().fg(tone.color()).render(label)),
                    "semantic color should stay on the glyph for {state:?}: {rendered:?}"
                );
            }
        }
    }

    #[test]
    fn lifecycle_rows_share_subtle_connectors_and_status_semantics() {
        for (status, success, glyph, tone) in [
            ("completed", true, "✓", MessageTone::Success),
            ("running", false, "●", MessageTone::Active),
            ("cancelled", false, "⊘", MessageTone::Warning),
            ("interrupted", false, "■", MessageTone::Warning),
            ("timed out", false, "◷", MessageTone::Error),
            ("failed", false, "✗", MessageTone::Error),
            ("pending", false, "○", MessageTone::Inactive),
        ] {
            let rendered =
                render_lifecycle_row("  └ ", &format!("verify · {status}"), status, success, 48);
            assert!(
                rendered.contains(&Style::new().fg(TN_SUBTLE).render("  └ ")),
                "{rendered:?}"
            );
            assert!(
                rendered.contains(&Style::new().fg(tone.color()).bold().render(glyph)),
                "status={status}: {rendered:?}"
            );
            assert!(
                rendered.contains(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render(&format!("verify · {status}"))
                ),
                "status={status}: {rendered:?}"
            );
            assert_visible_lines_bounded(&rendered, 48);
        }
    }

    #[test]
    fn program_preview_uses_neutral_hierarchy_until_a_real_failure() {
        let args = serde_json::json!({
            "intent": "verify the message hierarchy",
            "language": "javascript"
        });
        let preview = render_program_intent_preview(Some(&args), 72);

        assert!(preview.contains(&TN_SUBTLE.fg_ansi()), "{preview:?}");
        assert!(preview.contains(&TN_FG.fg_ansi()), "{preview:?}");
        assert!(!preview.contains(&TN_YELLOW.fg_ansi()), "{preview:?}");
        assert!(!preview.contains(&TN_GREEN.fg_ansi()), "{preview:?}");

        let failed = render_program_preview_row("  └ ", "actual", "tool call failed", 72, true);
        assert!(
            failed.contains(&Style::new().fg(TN_RED).bold().render("✗")),
            "{failed:?}"
        );
        assert!(
            failed.contains(&Style::new().fg(TN_FG).render("tool call failed")),
            "{failed:?}"
        );
        assert!(
            !failed.contains(&Style::new().fg(TN_RED).render("tool call failed")),
            "{failed:?}"
        );
    }

    #[test]
    fn explored_group_distinguishes_heading_action_pattern_and_path() {
        let args = serde_json::json!({
            "pattern": "ToolStatusLine",
            "path": "src/tui/ui/render.rs"
        });
        let rendered = render_tool_end("grep", 0, "1 match", None, Some(&args), 80);
        let plain = strip_ansi(&rendered);

        assert_eq!(
            plain,
            "• Explored\n  └ Search ToolStatusLine in src/tui/ui/render.rs"
        );
        assert!(rendered.contains(&Style::new().fg(TN_GRAY).render("•")));
        assert!(!rendered.contains(&Style::new().fg(TN_GREEN).bold().render("•")));
        assert!(rendered.contains(&Style::new().fg(TN_FG).bold().render("Explored")));
        assert!(rendered.contains(&Style::new().fg(TN_CYAN).render("Search")));
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_ARGUMENT_COLOR)
                .render("ToolStatusLine")
        ));
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_PATH_COLOR)
                .render("src/tui/ui/render.rs")
        ));
        assert_visible_lines_bounded(&rendered, 80);
    }

    #[test]
    fn completed_exec_matches_codex_status_syntax_and_output_hierarchy() {
        let args = serde_json::json!({
            "command": "git diff --name-only && git diff --cached --name-only"
        });
        let rendered = render_tool_end(
            "bash",
            0,
            "README.md\nsrc/lib.rs\ntests/cli.rs",
            None,
            Some(&args),
            88,
        );
        let plain = strip_ansi(&rendered);

        assert!(
            plain.starts_with("• Ran git diff --name-only && git diff"),
            "{plain}"
        );
        assert!(rendered.contains(&Style::new().fg(TN_GREEN).bold().render("•")));
        assert!(rendered.contains(&Style::new().fg(TN_FG).bold().render("Ran")));
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_PROGRAM_COLOR)
                .render("git")
        ));
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_FLAG_COLOR)
                .render("--name-only")
        ));
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_OPERATOR_COLOR)
                .render("&&")
        ));
        assert!(rendered.contains(&Style::new().fg(TN_GRAY).render("README.md")));
        assert_visible_lines_bounded(&rendered, 88);
    }

    #[test]
    fn full_width_action_never_consumes_or_drops_the_first_detail_character() {
        let rendered = render_action_header(
            "Awaiting approval for",
            Some("src/tui/ui/message_chrome.rs"),
            23,
            MessageTone::Warning,
            "  ",
            false,
        );
        let plain = strip_ansi(&rendered);
        let rows = plain.lines().collect::<Vec<_>>();

        assert_eq!(rows[0], "• Awaiting approval for", "{plain}");
        assert!(rows[1].starts_with("  src/"), "{plain}");
        assert!(!plain.contains('…'), "{plain}");
        assert_visible_lines_bounded(&rendered, 23);
    }

    #[test]
    fn render_exec_screen_cells_limit_command_to_three_rows_but_transcript_is_complete() {
        let command = format!("printf start-{}-TAIL", "x".repeat(640));
        let args = serde_json::json!({"command": command});

        for width in [32, 48, 80] {
            let rendered = render_tool_end("bash", 0, "", None, Some(&args), width);
            let plain = strip_ansi(&rendered);
            let command_rows = plain
                .lines()
                .take_while(|line| !line.starts_with("  └ "))
                .collect::<Vec<_>>();

            assert_eq!(command_rows.len(), 3, "width {width}:\n{plain}");
            assert!(command_rows[2].contains("… +"), "width {width}:\n{plain}");
            assert_visible_lines_bounded(&rendered, width);

            let transcript = render_tool_transcript(ToolTranscriptInput {
                name: "bash",
                state: ToolCallState::Succeeded,
                exit_code: Some(0),
                output: "",
                metadata: None,
                args: Some(&args),
                duration: Some(std::time::Duration::from_millis(10)),
                width,
            });
            let transcript = strip_ansi(&transcript);
            assert!(transcript.contains("TAIL"), "width {width}:\n{transcript}");
            assert!(!transcript.contains("… +"), "width {width}:\n{transcript}");
        }
    }

    #[test]
    fn render_completed_generic_and_mcp_json_as_pretty_bounded_payloads() {
        let nested = serde_json::json!({"nested": {"enabled": true}}).to_string();
        let array = serde_json::json!([
            "研究报告",
            {"url": format!("https://example.com/{}", "very-long-segment/".repeat(12))}
        ])
        .to_string();

        for width in [32, 48, 80] {
            let generic = render_tool_end(
                "custom_lookup",
                0,
                &nested,
                None,
                Some(&serde_json::json!({"id": 7})),
                width,
            );
            let generic_plain = strip_ansi(&generic);
            assert!(
                generic_plain.contains("  └ {"),
                "width {width}:\n{generic_plain}"
            );
            assert!(
                generic_plain.contains("\"nested\": {")
                    && generic_plain.contains("\"enabled\": true"),
                "width {width}:\n{generic_plain}"
            );
            assert!(!generic_plain.contains("{\"nested\":"), "{generic_plain}");
            assert!(
                generic.contains(
                    &Style::new()
                        .fg(tool_style::TOOL_KEY_COLOR)
                        .render("\"nested\"")
                ),
                "JSON keys should remain scannable at width {width}: {generic:?}"
            );
            assert!(
                generic.contains(
                    &Style::new()
                        .fg(tool_style::TOOL_KEYWORD_COLOR)
                        .render("true")
                ),
                "JSON literals should use the structured-output palette: {generic:?}"
            );
            assert_visible_lines_bounded(&generic, width);

            let mcp = render_tool_end(
                "mcp__search__lookup",
                0,
                &array,
                None,
                Some(&serde_json::json!({"query": "台风"})),
                width,
            );
            let mcp_plain = strip_ansi(&mcp);
            assert!(mcp_plain.contains("  └ ["), "width {width}:\n{mcp_plain}");
            assert!(mcp_plain.contains("\"研究报告\""), "{mcp_plain}");
            assert!(mcp_plain.contains("… +"), "width {width}:\n{mcp_plain}");
            assert!(
                mcp.contains(
                    &Style::new()
                        .fg(tool_style::TOOL_STRING_COLOR)
                        .render("\"研究报告\"")
                ),
                "MCP string values should retain semantic JSON styling: {mcp:?}"
            );
            assert_visible_lines_bounded(&mcp, width);
        }
    }

    #[test]
    fn full_mcp_transcript_keeps_complete_syntax_highlighted_json() {
        let output = serde_json::json!({
            "items": (0..18).map(|index| serde_json::json!({
                "id": index,
                "title": format!("result-{index}"),
                "ready": true
            })).collect::<Vec<_>>()
        })
        .to_string();
        let args = serde_json::json!({"query": "terminal UX"});
        let rendered = render_tool_transcript(ToolTranscriptInput {
            name: "mcp__docs__search",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output: &output,
            metadata: None,
            args: Some(&args),
            duration: None,
            width: 72,
        });
        let plain = strip_ansi(&rendered);

        assert!(
            plain.contains("result-0") && plain.contains("result-17"),
            "{plain}"
        );
        assert!(!plain.contains("… +"), "{plain}");
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_KEY_COLOR)
                .render("\"items\"")
        ));
        assert!(rendered.contains(&Style::new().fg(tool_style::TOOL_NUMBER_COLOR).render("17")));
        assert!(rendered.contains(
            &Style::new()
                .fg(tool_style::TOOL_KEYWORD_COLOR)
                .render("true")
        ));
        assert_visible_lines_bounded(&rendered, 72);
    }

    #[test]
    fn full_generic_transcript_keeps_one_complete_tree_at_product_widths() {
        let args = serde_json::json!({
            "query": "terminal message hierarchy",
            "options": {
                "limit": 12,
                "include_archived": false
            }
        });
        let output = serde_json::json!({
            "items": (0..12)
                .map(|index| serde_json::json!({"id": index, "title": format!("result-{index}")}))
                .collect::<Vec<_>>()
        })
        .to_string();

        for width in [24, 48, 80] {
            let rendered = render_tool_transcript(ToolTranscriptInput {
                name: "custom_lookup",
                state: ToolCallState::Succeeded,
                exit_code: Some(0),
                output: &output,
                metadata: None,
                args: Some(&args),
                duration: None,
                width,
            });
            let plain = strip_ansi(&rendered);
            let first = plain.lines().next().unwrap_or_default();

            assert_eq!(first, "• Called custom_lookup", "width {width}:\n{plain}");
            assert!(plain.contains("  ├ Input"), "width {width}:\n{plain}");
            assert!(plain.contains("  └ Result"), "width {width}:\n{plain}");
            assert_eq!(plain.matches("\"query\"").count(), 1, "{plain}");
            assert_eq!(plain.matches("\"options\"").count(), 1, "{plain}");
            assert!(
                plain.contains("result-0") && plain.contains("result-11"),
                "{plain}"
            );
            assert!(!plain.lines().any(|row| row.trim() == "✓"), "{plain}");
            assert_visible_lines_bounded(&rendered, width);
        }
    }

    #[test]
    fn render_live_or_malformed_structured_payloads_source_identically() {
        for name in ["custom_lookup", "mcp__search__lookup"] {
            for payload in [r#"{"nested":{"ok":"#, r#"{"nested": nope}"#] {
                let live = render_live_tool_activity(
                    name,
                    Some(&serde_json::json!({"query": "研究"})),
                    payload,
                    80,
                    true,
                    ToolCallState::Running,
                );
                assert!(strip_ansi(&live).contains(payload), "{name}: {live}");

                let completed = render_tool_end(
                    name,
                    0,
                    payload,
                    None,
                    Some(&serde_json::json!({"query": "研究"})),
                    80,
                );
                assert!(
                    strip_ansi(&completed).contains(payload),
                    "{name}: {completed}"
                );
            }
        }
    }

    #[test]
    fn render_unknown_tool_args_as_compact_key_value_summary_with_semantic_colors() {
        let args = serde_json::json!({
            "path": "./src/main.rs",
            "url": "https://example.com/research",
            "count": 2,
            "enabled": true,
            "nested": {"depth": 3},
            "label": "研究"
        });

        for width in [32, 48, 80] {
            let rendered = render_live_tool_activity(
                "custom_lookup",
                Some(&args),
                "",
                width,
                true,
                ToolCallState::Running,
            );
            let plain = strip_ansi(&rendered);
            assert!(plain.contains("count=2"), "width {width}:\n{plain}");
            assert!(!plain.contains("{\"count\":"), "width {width}:\n{plain}");
            assert_visible_lines_bounded(&rendered, width);
            if width == 80 {
                assert!(
                    rendered.contains(&tool_style::TOOL_NUMBER_COLOR.fg_ansi()),
                    "{rendered:?}"
                );
                assert!(
                    rendered.contains(&tool_style::TOOL_KEY_COLOR.fg_ansi()),
                    "{rendered:?}"
                );
                assert!(
                    rendered.contains(&tool_style::TOOL_KEYWORD_COLOR.fg_ansi()),
                    "{rendered:?}"
                );
            }
        }

        let colored = highlight_tool_detail(
            "custom_lookup path=./src/main.rs flag=--all count=2 enabled=true",
        );
        assert!(
            colored.contains(&tool_style::TOOL_NUMBER_COLOR.fg_ansi()),
            "{colored:?}"
        );
        assert!(
            colored.contains(&tool_style::TOOL_KEY_COLOR.fg_ansi()),
            "{colored:?}"
        );
        assert!(
            colored.contains(&tool_style::TOOL_KEYWORD_COLOR.fg_ansi()),
            "{colored:?}"
        );

        let transcript = strip_ansi(&render_tool_transcript(ToolTranscriptInput {
            name: "custom_lookup",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output: "ok",
            metadata: None,
            args: Some(&args),
            duration: None,
            width: 80,
        }));
        assert!(transcript.contains("Input"), "{transcript}");
        assert!(transcript.contains("\"nested\": {"), "{transcript}");
        assert!(transcript.contains("\"depth\": 3"), "{transcript}");
    }

    #[test]
    fn live_exec_uses_command_continuation_and_bounded_head_tail_output() {
        let args = serde_json::json!({
            "command": "cargo test very-long-filter-name -- --nocapture"
        });
        let output = (0..16)
            .map(|i| format!("line-{i}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render_live_tool_activity(
            "bash",
            Some(&args),
            &output,
            48,
            true,
            ToolCallState::Running,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);
        let rows = plain.lines().collect::<Vec<_>>();

        assert!(plain.contains("Running cargo test"), "{plain}");
        assert_eq!(rows[0], "• Running cargo test very-long-filter-name --");
        assert_eq!(rows[1], "  │ --nocapture");
        assert_eq!(rows[2], "  └ line-0");
        assert!(plain.contains("… +12 lines"), "{plain}");
        assert!(plain.contains("line-0"));
        assert!(!plain.contains("line-8"));
        assert!(plain.contains("line-15"));
        assert_visible_lines_bounded(&rendered, 48);
        assert!(rendered.contains("\x1b["));
    }

    #[test]
    fn running_tool_matrix_has_bounded_design_activity() {
        let width = 54;
        let cases = [
            (
                "bash",
                serde_json::json!({"command":"cargo test long-filter -- --nocapture"}),
                "Running cargo test",
            ),
            (
                "read",
                serde_json::json!({"file_path":"src/tui/ui/render.rs"}),
                "Exploring\n  └ Read src/tui/ui/render.rs",
            ),
            (
                "write",
                serde_json::json!({"file_path":"src/tui/new.rs"}),
                "Writing src/tui/new.rs",
            ),
            (
                "edit",
                serde_json::json!({"file_path":"src/tui/ui/render.rs"}),
                "Editing src/tui/ui/render.rs",
            ),
            (
                "grep",
                serde_json::json!({"pattern":"RuntimeExpectation", "path":"src"}),
                "Exploring\n  └ Search RuntimeExpectation in src",
            ),
            (
                "glob",
                serde_json::json!({"pattern":"src/**/*.rs"}),
                "Exploring\n  └ List src/**/*.rs",
            ),
            (
                "web_search",
                serde_json::json!({"query":"A3S Runtime RemoteUI"}),
                "Searching the web A3S Runtime",
            ),
            (
                "web_fetch",
                serde_json::json!({"url":"https://example.com/very/long/path"}),
                "Fetching https://example.com",
            ),
            (
                "task",
                serde_json::json!({"description":"Audit terminal rendering"}),
                "Delegating Audit terminal",
            ),
            (
                "parallel_task",
                serde_json::json!({"tasks":["audit running state", "audit failure state"]}),
                "Delegating 2 tasks",
            ),
            (
                "runtime",
                serde_json::json!({"worker":"researcher", "tasks":["collect evidence", "summarize"]}),
                "Running Runtime 2 tasks",
            ),
            (
                "git",
                serde_json::json!({"command":"status", "target":"--short"}),
                "Running git status --short",
            ),
            (
                "batch",
                serde_json::json!({"invocations":[{"tool":"read"}, {"tool":"grep"}]}),
                "Running batch 2 tools",
            ),
            (
                "program",
                serde_json::json!({"type":"script"}),
                "Running program script",
            ),
            (
                "generate_object",
                serde_json::json!({"schema_name":"release_summary", "prompt":"Summarize"}),
                "Generating object release_summary",
            ),
            (
                "search_skills",
                serde_json::json!({"query":"terminal rendering"}),
                "Searching skills terminal rendering",
            ),
            (
                "Skill",
                serde_json::json!({"skill_name":"inspect-surface", "prompt":"Apply"}),
                "Using skill inspect-surface",
            ),
        ];

        for (tool, args, expected) in cases {
            let rendered = render_live_tool_activity(
                tool,
                Some(&args),
                "",
                width,
                true,
                ToolCallState::Running,
            );
            let plain = a3s_tui::style::strip_ansi(&rendered);
            assert!(plain.contains(expected), "{tool} got:\n{plain}");
            assert!(!plain.trim_end().ends_with('…'), "{tool} got: {plain}");
            assert_visible_lines_bounded(&rendered, width);
        }
    }

    #[test]
    fn completed_tool_matrix_has_bounded_design_headers() {
        let width = 56;
        let cases = [
            (
                "bash",
                serde_json::json!({"command":"cargo test very-long-filter-name -- --nocapture"}),
                "Ran cargo test",
            ),
            (
                "read",
                serde_json::json!({"file_path":"README.md"}),
                "Explored\n  └ Read README.md",
            ),
            (
                "write",
                serde_json::json!({"file_path":"src/tui/new.rs"}),
                "Wrote src/tui/new.rs",
            ),
            (
                "edit",
                serde_json::json!({"file_path":"src/tui/ui/render.rs"}),
                "Edited src/tui/ui/render.rs",
            ),
            (
                "grep",
                serde_json::json!({"pattern":"RuntimeExpectation", "path":"src/tui/mod.rs"}),
                "Explored\n  └ Search RuntimeExpectation in src/tui/mod.rs",
            ),
            (
                "glob",
                serde_json::json!({"pattern":"src/**/*.rs"}),
                "Explored\n  └ List src/**/*.rs",
            ),
            (
                "web_search",
                serde_json::json!({"query":"A3S Runtime parallel remote UI report generation"}),
                "Searched the web for A3S Runtime",
            ),
            (
                "web_fetch",
                serde_json::json!({"url":"https://example.com/a/very/long/path/that/should/not/overflow"}),
                "Fetched https://example.com",
            ),
            (
                "runtime",
                serde_json::json!({
                    "worker":"research-agent-with-a-long-name",
                    "tasks":["collect sources for topic one", "compare alternatives for topic two", "summarize risks"]
                }),
                "Used Runtime 3 tasks",
            ),
            (
                "git",
                serde_json::json!({"command":"diff", "target":"HEAD~1"}),
                "Ran git diff HEAD~1",
            ),
            (
                "batch",
                serde_json::json!({
                    "invocations":[
                        {"tool":"read", "args":{"file_path":"README.md"}},
                        {"tool":"glob", "args":{"pattern":"**/*.rs"}},
                        {"tool":"grep", "args":{"pattern":"TODO"}}
                    ]
                }),
                "Ran batch 3 tools",
            ),
            (
                "task",
                serde_json::json!({"description":"Audit tool rendering states"}),
                "Delegated Audit tool",
            ),
            (
                "parallel_task",
                serde_json::json!({"tasks":["audit running state", "audit failure state"]}),
                "Delegated 2 tasks",
            ),
            (
                "program",
                serde_json::json!({"type":"script", "source":"async function run() {}"}),
                "Ran program script",
            ),
            (
                "generate_object",
                serde_json::json!({"schema_name":"release_summary", "prompt":"Summarize"}),
                "Generated object release_summary",
            ),
            (
                "search_skills",
                serde_json::json!({"query":"terminal rendering"}),
                "Searched skills terminal rendering",
            ),
            (
                "Skill",
                serde_json::json!({"skill_name":"inspect-surface", "prompt":"Apply the skill"}),
                "Used skill inspect-surface",
            ),
        ];

        for (tool, args, expected) in cases {
            let rendered = render_tool_end(tool, 0, "ok\n", None, Some(&args), width);
            let plain = a3s_tui::style::strip_ansi(&rendered);
            assert!(
                plain.contains(expected),
                "{tool} should include {expected:?}, got:\n{plain}"
            );
            assert_visible_lines_bounded(&rendered, width);
        }
    }

    #[test]
    fn unknown_tools_use_codex_calling_and_called_fallback() {
        let args = serde_json::json!({"path": "src/lib.rs", "verbose": true});
        let live = render_live_tool_activity(
            "agent_dir_review",
            Some(&args),
            "",
            80,
            true,
            ToolCallState::Running,
        );
        let completed = render_tool_end(
            "agent_dir_review",
            0,
            "review complete",
            None,
            Some(&args),
            80,
        );

        assert_eq!(
            strip_ansi(&live),
            "• Calling agent_dir_review(path=src/lib.rs, verbose=true)"
        );
        assert_eq!(
            strip_ansi(&completed),
            "• Called agent_dir_review(path=src/lib.rs, verbose=true)\n  └ review complete"
        );

        let arbitrary = serde_json::json!({"title": "Bug", "dry_run": false});
        let label = tool_label("custom_issue_tool", Some(&arbitrary));
        assert!(label.starts_with("custom_issue_tool("), "{label}");
        assert!(label.contains("title=Bug"), "{label}");
        assert!(label.contains("dry_run=false"), "{label}");

        let approval = render_live_tool_activity(
            "custom_issue_tool",
            Some(&arbitrary),
            "",
            80,
            true,
            ToolCallState::AwaitingApproval,
        );
        let approval = strip_ansi(&approval);
        assert!(approval.contains("\"title\": \"Bug\""), "{approval}");
        assert!(approval.contains("\"dry_run\": false"), "{approval}");

        let transcript = render_tool_transcript(ToolTranscriptInput {
            name: "custom_issue_tool",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output: "created",
            metadata: None,
            args: Some(&arbitrary),
            duration: None,
            width: 80,
        });
        let transcript = strip_ansi(&transcript);
        assert!(transcript.contains("Input"), "{transcript}");
        assert!(transcript.contains("title=\"Bug\""), "{transcript}");
        assert!(transcript.contains("dry_run=false"), "{transcript}");
        assert!(transcript.contains("Result  created"), "{transcript}");
        assert_eq!(transcript.matches("title=").count(), 1, "{transcript}");
        assert_eq!(transcript.lines().count(), 3, "{transcript}");
    }

    #[test]
    fn failed_completed_tool_uses_error_chrome_and_stays_bounded() {
        let args = serde_json::json!({
            "command": "npm run a-script-with-a-very-long-name-that-fails"
        });
        let rendered = render_tool_end(
            "bash",
            1,
            "first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\nninth\ntenth\neleventh\ntwelfth with a long tail that should be clipped",
            None,
            Some(&args),
            48,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("Ran npm run"));
        assert!(plain.contains("… +9 lines · Ctrl+T"));
        assert!(plain.contains("first"));
        assert!(!plain.contains("sixth"));
        assert!(plain.contains("twelfth"));
        assert_visible_lines_bounded(&rendered, 48);
    }

    #[test]
    fn completed_exec_output_uses_codex_result_branch() {
        let rendered = render_tool_end(
            "bash",
            1,
            "first\nsecond\nthird\nfourth\nfifth\nsixth\nseventh\neighth\nninth\ntenth\neleventh\ntwelfth with a long tail that should be clipped",
            None,
            Some(&serde_json::json!({"command": "npm test"})),
            48,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);
        let lines = plain.lines().collect::<Vec<_>>();

        assert_eq!(lines[0], "• Ran npm test");
        assert_eq!(lines[1], "  └ first");
        assert!(
            lines.iter().any(|line| line == &"    … +9 lines · Ctrl+T"),
            "{plain}"
        );
        assert!(
            lines.iter().any(|line| line.starts_with("    twelfth")),
            "{plain}"
        );
        assert!(
            rendered.contains(&format!("\x1b[{}mtwelfth", TN_GRAY.fg_ansi())),
            "failed output should stay muted while the marker carries the error: {rendered:?}"
        );
        assert_visible_lines_bounded(&rendered, 48);
    }

    #[test]
    fn completed_exec_without_output_stays_on_one_header_row() {
        let rendered = render_tool_end(
            "bash",
            0,
            "",
            None,
            Some(&serde_json::json!({"command": "echo ok"})),
            80,
        );
        assert_eq!(a3s_tui::style::strip_ansi(&rendered), "• Ran echo ok");
    }

    #[test]
    fn exec_output_hint_counts_all_hidden_logical_lines() {
        let output = (0..16)
            .map(|index| format!("line {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render_tool_end(
            "bash",
            0,
            &output,
            None,
            Some(&serde_json::json!({"command": "many-lines"})),
            80,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("… +12 lines"), "{plain}");
        assert!(plain.contains("line 0"), "{plain}");
        assert!(plain.contains("line 15"), "{plain}");
    }

    #[test]
    fn web_cells_use_codex_wording_and_hide_success_bodies() {
        let search = render_tool_end(
            "web_search",
            0,
            "provider response that should not be dumped",
            None,
            Some(&serde_json::json!({"query": "Rust terminal UX"})),
            80,
        );
        let fetch = render_tool_end(
            "web_fetch",
            0,
            "<html>very large page body</html>",
            None,
            Some(&serde_json::json!({"url": "https://example.com/docs"})),
            80,
        );
        let search = a3s_tui::style::strip_ansi(&search);
        let fetch = a3s_tui::style::strip_ansi(&fetch);

        assert_eq!(search, "• Searched the web for Rust terminal UX");
        assert_eq!(fetch, "• Fetched https://example.com/docs");
        assert!(!search.contains("provider response"));
        assert!(!fetch.contains("<html>"));
    }

    #[test]
    fn mcp_cells_keep_arguments_in_the_full_transcript() {
        let args = serde_json::json!({
            "query": "ratatui styling",
            "limit": 3
        });
        let live = render_live_tool_activity(
            "mcp__search__find_docs",
            Some(&args),
            "",
            80,
            true,
            ToolCallState::Running,
        );
        let completed = render_tool_end(
            "mcp__search__find_docs",
            0,
            "Found styling guidance",
            None,
            Some(&args),
            80,
        );
        let transcript = render_tool_transcript(ToolTranscriptInput {
            name: "mcp__search__find_docs",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output: "Found styling guidance",
            metadata: None,
            args: Some(&args),
            duration: None,
            width: 80,
        });
        let live = a3s_tui::style::strip_ansi(&live);
        let completed = a3s_tui::style::strip_ansi(&completed);
        let transcript = a3s_tui::style::strip_ansi(&transcript);

        assert!(live.starts_with("• Calling search.find_docs"), "{live}");
        assert!(!live.contains("ratatui styling"), "{live}");
        assert!(
            completed.starts_with("• Called search.find_docs"),
            "{completed}"
        );
        assert!(!completed.contains("\"query\""), "{completed}");
        assert!(
            completed.ends_with("\n  └ Found styling guidance"),
            "{completed}"
        );
        assert!(transcript.contains("Input"), "{transcript}");
        assert!(
            transcript.contains("query=\"ratatui styling\""),
            "{transcript}"
        );
        assert!(transcript.contains("limit=3"), "{transcript}");
        assert!(
            transcript.contains("Result  Found styling guidance"),
            "{transcript}"
        );
        assert!(!transcript.contains("unknown"), "{transcript}");
    }

    #[test]
    fn failed_edit_never_renders_success_diff() {
        let meta = serde_json::json!({
            "file_path": "src/lib.rs",
            "before": "old line\n",
            "after": "new line\n"
        });
        let rendered = render_tool_end(
            "edit",
            1,
            "old_string was not found",
            Some(&meta),
            Some(&serde_json::json!({"file_path": "src/lib.rs"})),
            80,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.starts_with("• Failed to edit src/lib.rs"), "{plain}");
        assert!(plain.contains("└ old_string was not found"), "{plain}");
        assert!(!plain.contains("Edited src/lib.rs"), "{plain}");
        assert!(!plain.contains("new line"), "{plain}");
        assert!(!plain.contains("(+"), "{plain}");
    }

    #[test]
    fn new_file_diff_is_labeled_added() {
        let meta = serde_json::json!({
            "file_path": "notes.txt",
            "after": "hello\n"
        });
        let rendered = render_tool_end("write", 0, "ok", Some(&meta), None, 80);
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("• Added notes.txt (+1 -0)"), "{plain}");
        assert!(!plain.contains("Edited notes.txt"), "{plain}");
    }

    #[test]
    fn added_and_deleted_diff_headers_keep_exact_width_after_action_selection() {
        let cases = [
            (
                "write",
                serde_json::json!({
                    "file_path": "a/very/long/path/to/notes.txt",
                    "after": "hello\n"
                }),
                "Added",
            ),
            (
                "delete",
                serde_json::json!({
                    "file_path": "a/very/long/path/to/notes.txt",
                    "before": "hello\n"
                }),
                "Deleted",
            ),
        ];

        for width in [28, 36, 48] {
            for (tool, metadata, action) in &cases {
                let rendered = render_tool_end(tool, 0, "", Some(metadata), None, width);
                let header = rendered.lines().next().expect("diff header");
                assert_eq!(visible_len(header), width, "{action}: {header:?}");
                assert!(strip_ansi(header).contains(action), "{action}: {header:?}");
                assert_visible_lines_bounded(&rendered, width);
            }
        }
    }

    #[test]
    fn hostile_output_ansi_is_stripped_and_each_rendered_row_is_bounded() {
        let rendered = render_tool_end(
            "bash",
            1,
            "\x1b[31mred\x1b[0m\n\x1b]0;title\x07second very long output row that must be clipped",
            None,
            Some(&serde_json::json!({"command": "false"})),
            32,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("└ red"), "{plain}");
        assert!(
            rendered.contains(&Style::new().fg(TN_SUBTLE).render("  └ ")),
            "tool output should share the subtle message connector: {rendered:?}"
        );
        assert!(!plain.contains('\x1b'));
        assert_visible_lines_bounded(&rendered, 32);
        for line in rendered.lines().filter(|line| line.contains("\x1b[")) {
            assert!(
                line.ends_with("\x1b[0m"),
                "ANSI leaked across row: {line:?}"
            );
        }
    }

    #[test]
    fn task_summary_uses_codex_branch_grammar() {
        let meta = serde_json::json!({
            "agent": "review",
            "task_id": "task-with-a-long-id-that-still-fits",
            "success": false,
            "output_bytes": 0
        });
        let rendered = render_tool_end("task", 1, "Task failed\nOutput:\n", Some(&meta), None, 44);
        let plain = a3s_tui::style::strip_ansi(&rendered);
        let lines = plain.lines().collect::<Vec<_>>();

        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("  └ ✗ Task failed")),
            "{plain}"
        );
        assert!(
            lines
                .iter()
                .any(|line| line.starts_with("    · no child text output")),
            "{plain}"
        );
        assert!(
            rendered.contains(&format!("\x1b[{}m✗", TN_RED.fg_ansi())),
            "failed task summary should use checklist error glyph color: {rendered:?}"
        );
        assert!(
            rendered.contains(&format!("\x1b[{}mTask failed", TN_FG.fg_ansi())),
            "failed task summary should keep readable neutral text: {rendered:?}"
        );
        assert!(
            !rendered.contains(&format!("\x1b[{}mTask failed", TN_RED.fg_ansi())),
            "failed task summary should reserve red for the glyph: {rendered:?}"
        );
        assert_visible_lines_bounded(&rendered, 44);
    }

    #[test]
    fn parallel_task_summary_marks_each_result_with_checklist_status() {
        let meta = serde_json::json!({
            "recovered_task_count": 1,
            "results": [
                {
                    "agent": "plan",
                    "task_id": "task-ok",
                    "success": true,
                    "output_bytes": 42,
                    "output_excerpt": "ready",
                    "retry_attempts": 1
                },
                {
                    "agent": "review",
                    "task_id": "task-fail",
                    "success": false,
                    "output_bytes": 0,
                    "output": "Task failed\nOutput:\n"
                }
            ]
        });
        let rendered = render_tool_end("parallel_task", 0, "", Some(&meta), None, 58);
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(
            plain.contains("  └ ✓ 1/2 agents succeeded · 1 recovered"),
            "{plain}"
        );
        assert!(
            plain.contains("    ✓ plan · task-ok · retried · ready"),
            "{plain}"
        );
        assert!(
            plain.contains("    ✗ review · task-fail · no child text output"),
            "{plain}"
        );
        assert!(
            rendered.contains(&format!("\x1b[{}m✓", TN_GREEN.fg_ansi())),
            "successful task rows should use checklist success glyphs: {rendered:?}"
        );
        assert!(
            rendered.contains(&format!("\x1b[{}m✗", TN_RED.fg_ansi())),
            "failed task rows should use checklist error glyphs: {rendered:?}"
        );
        assert_visible_lines_bounded(&rendered, 58);
    }

    #[test]
    fn failed_completed_tool_matrix_uses_error_accent_and_stays_bounded() {
        let width = 46;
        let cases = [
            (
                "bash",
                serde_json::json!({"command":"cargo test failing-filter"}),
            ),
            (
                "grep",
                serde_json::json!({"pattern":"needle", "path":"src"}),
            ),
            (
                "web_fetch",
                serde_json::json!({"url":"https://example.com/failing"}),
            ),
            (
                "runtime",
                serde_json::json!({"tasks":["run failing branch"]}),
            ),
            ("Skill", serde_json::json!({"skill_name":"inspect-surface"})),
        ];

        for (tool, args) in cases {
            let rendered = render_tool_end(
                tool,
                1,
                "first\nsecond\nthird\nfourth\nfifth\nsixth",
                None,
                Some(&args),
                width,
            );
            assert!(
                rendered.contains(&TN_RED.fg_ansi()),
                "{tool} got:\n{rendered:?}"
            );
            assert_visible_lines_bounded(&rendered, width);
        }
    }

    #[test]
    fn diff_rendering_uses_shared_diff_view_and_bounds_rows() {
        let before = "let old_value = \"a very long old value that should wrap instead of escaping the viewport\";\nkeep();\n";
        let after = "let new_value = \"a very long new value that should wrap instead of escaping the viewport\";\nkeep();\n";
        let rendered = render_diff(
            "src/tui/a/very/long/path/that/should/not/overflow/render.rs",
            before,
            after,
            48,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);
        let header = rendered.lines().next().expect("diff header");

        assert!(plain.contains("Edited"));
        assert!(plain.contains("+1") && plain.contains("-1"));
        assert!(
            header.contains(&Style::new().fg(DIFF_HEADER_BULLET).bold().render("•")),
            "diff bullet should use the muted header color: {header:?}"
        );
        assert!(
            header.contains(&Style::new().fg(DIFF_HEADER_ACTION).bold().render("Edited")),
            "diff action should use the bright header color: {header:?}"
        );
        assert!(
            header.contains(&Style::new().fg(DIFF_INSERT_MARKER).bold().render("+1")),
            "addition count should use the insert marker color: {header:?}"
        );
        assert!(
            header.contains(&Style::new().fg(DIFF_DELETE_MARKER).bold().render("-1")),
            "deletion count should use the delete marker color: {header:?}"
        );
        assert!(
            rendered.contains(&DIFF_INSERT_BG.bg_ansi()),
            "insert rows should use the reference background: {rendered:?}"
        );
        assert!(
            rendered.contains(&DIFF_DELETE_BG.bg_ansi()),
            "delete rows should use the reference background: {rendered:?}"
        );
        assert!(
            rendered.contains(
                &Style::new()
                    .fg(Color::Rgb(210, 164, 253))
                    .bg(DIFF_INSERT_BG)
                    .render("let")
            ),
            "inserted Rust should retain syntax highlighting: {rendered:?}"
        );
        assert!(
            rendered.contains(
                &Style::new()
                    .fg(mix_diff_color(Color::Rgb(210, 164, 253), DIFF_DELETE_BG,))
                    .bg(DIFF_DELETE_BG)
                    .render("let")
            ),
            "deleted Rust should use the muted syntax color: {rendered:?}"
        );
        assert_visible_lines_bounded(&rendered, 48);
    }

    #[test]
    fn arg_summary_handles_runtime_string_tasks_and_batch_invocations() {
        assert_eq!(
            arg_summary(&serde_json::json!({
                "worker":"researcher",
                "tasks":["alpha branch", "beta branch"]
            })),
            Some("2 tasks via researcher: alpha branch; beta branch".to_string())
        );
        assert_eq!(
            arg_summary(&serde_json::json!({
                "invocations":[
                    {"tool":"read", "args":{"file_path":"README.md"}},
                    {"tool":"bash", "args":{"command":"cargo test"}}
                ]
            })),
            Some("2 tools: read(README.md), bash(cargo test)".to_string())
        );
        assert_eq!(
            arg_summary(&serde_json::json!({
                "worker":"researcher",
                "tasks":[
                    {"query":"official sources"},
                    {"title":"independent analysis", "focus":"contradictions"}
                ]
            })),
            Some("2 tasks via researcher: official sources; independent analysis".to_string())
        );
    }

    #[test]
    fn runtime_completion_uses_structured_task_summary_instead_of_raw_json() {
        let args = serde_json::json!({
            "worker": "researcher",
            "tasks": [{"query":"official"}, {"query":"independent"}]
        });
        let output = serde_json::json!({
            "batchId": "batch-1",
            "worker": "researcher-id",
            "count": 2,
            "results": [
                {"invocationId":"one", "state":"completed", "output":{"answer":"ok"}, "error":null},
                {"invocationId":"two", "state":"failed", "output":null, "error":"boom"}
            ]
        })
        .to_string();
        let rendered = render_tool_end("runtime", 0, &output, None, Some(&args), 72);
        let plain = strip_ansi(&rendered);

        assert!(
            plain.contains("• Used Runtime 1/2 tasks via researcher-id"),
            "{plain}"
        );
        assert!(plain.contains("✓ task 1 · one · completed"), "{plain}");
        assert!(plain.contains("✗ task 2 · two · failed"), "{plain}");
        assert!(!plain.contains("batchId"), "{plain}");
    }

    #[test]
    fn dynamic_workflow_renders_run_and_step_progress_without_raw_snapshot() {
        let args = serde_json::json!({"run_id": "research-42"});
        let meta = serde_json::json!({
            "dynamic_workflow": {
                "run_id": "research-42",
                "history": [
                    {"event":{"type":"step_created", "step_id":"verify"}},
                    {"event":{"type":"step_created", "step_id":"collect"}}
                ],
                "snapshot": {
                    "steps": {
                        "collect": {"status": "completed"},
                        "verify": {"status": "failed"}
                    }
                }
            }
        });
        let rendered = render_tool_end(
            "dynamic_workflow",
            1,
            "raw workflow JSON must stay hidden",
            Some(&meta),
            Some(&args),
            64,
        );
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("• Workflow failed research-42"), "{plain}");
        assert!(plain.contains("✓ collect · completed"), "{plain}");
        assert!(plain.contains("✗ verify · failed"), "{plain}");
        assert!(
            plain.find("verify").unwrap() < plain.find("collect").unwrap(),
            "step rows must follow creation order: {plain}"
        );
        assert!(!plain.contains("raw workflow JSON"), "{plain}");
        assert_visible_lines_bounded(&rendered, 64);
    }

    #[test]
    fn live_dynamic_workflow_hides_partial_snapshot_output() {
        let args = serde_json::json!({"run_id": "research-42"});
        let rendered = render_live_tool_activity(
            "dynamic_workflow",
            Some(&args),
            r#"{"snapshot":{"steps":{"collect":{"status":"running"}}}}"#,
            64,
            true,
            ToolCallState::Running,
        );
        let plain = strip_ansi(&rendered);

        assert_eq!(plain, "• Running workflow research-42");
        assert!(!plain.contains("snapshot"), "{plain}");
        assert_visible_lines_bounded(&rendered, 64);
    }

    #[test]
    fn interrupted_dynamic_workflow_does_not_dump_partial_json() {
        let rendered = render_tool_end(
            "dynamic_workflow",
            130,
            r#"{"snapshot":{"steps":{"collect":{"status":"running"}}"#,
            None,
            Some(&serde_json::json!({"run_id": "research-42"})),
            64,
        );
        let plain = strip_ansi(&rendered);

        assert_eq!(plain, "• Workflow failed research-42");
        assert!(!plain.contains("snapshot"), "{plain}");
    }

    #[test]
    fn program_summary_uses_structured_nested_call_metadata() {
        let meta = serde_json::json!({
            "program": {
                "language": "javascript",
                "tool_calls": [
                    {"tool_name": "read", "success": true, "output_bytes": 128},
                    {"tool_name": "grep", "success": false, "output_bytes": 0}
                ]
            }
        });
        let rendered = render_tool_end(
            "program",
            0,
            "duplicated text summary",
            Some(&meta),
            Some(&serde_json::json!({
                "type": "script",
                "language": "javascript",
                "source": "async function run(ctx) {\n  return await ctx.read({ path: 'README.md' });\n}",
                "inputs": {"query": "inspect repository documentation"}
            })),
            64,
        );
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("• Ran program javascript"), "{plain}");
        assert!(plain.contains("intent"), "{plain}");
        assert!(
            plain.contains("inspect repository documentation"),
            "{plain}"
        );
        assert!(plain.contains("actual"), "{plain}");
        assert!(plain.contains("called read → grep · 1/2 ok"), "{plain}");
        assert!(!plain.contains("async function run"), "{plain}");
        assert!(!plain.contains("output_bytes"), "{plain}");
        assert!(!plain.contains("duplicated text summary"), "{plain}");
        assert_visible_lines_bounded(&rendered, 64);
    }

    #[test]
    fn running_program_previews_deep_research_intent_instead_of_repeated_source() {
        let source = format!(
            "async function run(ctx, inputs) {{\n{}\n}}",
            "  const boilerplate = true;\n".repeat(1_601)
        );
        let rendered = render_live_tool_activity(
            "program",
            Some(&serde_json::json!({
                "type": "script",
                "language": "javascript",
                "source": source,
                "inputs": {
                    "kind": "workflow",
                    "step_outputs": {},
                    "step_failures": {},
                    "input": {
                        "query": "Nimbus 支持状态",
                        "evidence_scope": "web_and_workspace",
                        "inquiry_host_managed": true,
                        "loop_contract": {
                            "pattern": "minimal-deep-research",
                            "hard_caps": {
                                "max_searches": 4,
                                "max_fetches": 8
                            }
                        }
                    }
                }
            })),
            "",
            100,
            true,
            ToolCallState::Running,
        );
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("• Running program script"), "{plain}");
        assert!(plain.contains("intent"), "{plain}");
        assert!(plain.contains("DeepResearch “Nimbus 支持状态”"), "{plain}");
        assert!(
            plain.contains("web + workspace · ≤2 typed-coverage passes · ≤4 searches · ≤8 fetches"),
            "{plain}"
        );
        assert!(
            plain.contains("run initial retrieval and typed semantic coverage"),
            "{plain}"
        );
        assert!(!plain.contains("async function run"), "{plain}");
        assert!(!plain.contains("boilerplate"), "{plain}");
        assert!(!plain.contains("+1601 lines"), "{plain}");
        assert_eq!(plain.lines().count(), 4, "{plain}");
        assert_visible_lines_bounded(&rendered, 100);
    }

    #[test]
    fn partial_batch_renders_semantic_items_instead_of_raw_combined_output() {
        let args = serde_json::json!({
            "invocations": [
                {"tool": "read", "args": {"file_path": "README.md"}},
                {"tool": "bash", "args": {"command": "cargo test"}}
            ]
        });
        let meta = serde_json::json!({
            "status": "partial_failure",
            "execution_mode": "parallel",
            "applied_concurrency": 2,
            "total_count": 2,
            "success_count": 1,
            "failure_count": 1,
            "results": [
                {"index": 0, "tool": "read", "success": true, "exit_code": 0},
                {"index": 1, "tool": "bash", "success": false, "exit_code": 101}
            ]
        });

        for width in [24, 48, 80] {
            let rendered = render_tool_end(
                "batch",
                0,
                "--- [1: read] ---\ncontents\n--- [2: bash] ---\nERROR: failed",
                Some(&meta),
                Some(&args),
                width,
            );
            let plain = strip_ansi(&rendered);

            assert!(plain.contains("Batch partially"), "{plain}");
            assert!(plain.contains("✓ Read"), "{plain}");
            assert!(plain.contains("✗ exit 101"), "{plain}");
            assert!(plain.contains("exit 101"), "{plain}");
            assert!(!plain.contains("contents"), "{plain}");
            assert_visible_lines_bounded(&rendered, width);
            assert!(rendered.contains(&Style::new().fg(TN_GREEN).bold().render("✓")));
            assert!(rendered.contains(&Style::new().fg(TN_RED).bold().render("✗")));
            assert!(
                !rendered.contains(&Style::new().fg(TN_RED).render("Bash")),
                "failed item labels should remain neutral: {rendered:?}"
            );
        }
    }

    #[test]
    fn batch_transcript_reports_partial_outcome_after_complete_output() {
        let args = serde_json::json!({
            "invocations": [
                {"tool": "read", "args": {"file_path": "README.md"}},
                {"tool": "bash", "args": {"command": "cargo test"}}
            ]
        });
        let meta = serde_json::json!({
            "execution_mode": "parallel",
            "applied_concurrency": 2,
            "success_count": 1,
            "failure_count": 1,
            "results": [
                {"index": 0, "tool": "read", "success": true, "exit_code": 0},
                {"index": 1, "tool": "bash", "success": false, "exit_code": 101}
            ]
        });
        let output = "--- [1: read] ---\ncontents\n--- [2: bash] ---\nERROR: failed";
        let rendered = render_tool_transcript(ToolTranscriptInput {
            name: "batch",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output,
            metadata: Some(&meta),
            args: Some(&args),
            duration: Some(std::time::Duration::from_secs(2)),
            width: 80,
        });
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("--- [1: read] ---"), "{plain}");
        assert!(plain.contains("contents"), "{plain}");
        assert!(plain.contains("--- [2: bash] ---"), "{plain}");
        assert!(plain.contains("ERROR: failed"), "{plain}");
        assert!(
            plain.ends_with("  └ ! partial · 1 failed • 2.0s"),
            "{plain}"
        );
        assert!(!plain.ends_with("  └ ✓ • 2.0s"), "{plain}");
        assert!(rendered.contains(&Style::new().fg(TN_YELLOW).bold().render("!")));
        assert_visible_lines_bounded(&rendered, 80);
    }

    #[test]
    fn large_batch_keeps_failures_visible_and_collapses_secondary_successes() {
        let invocations = (0..12)
            .map(|index| {
                serde_json::json!({
                    "tool": "read",
                    "args": {"file_path": format!("file-{index}.rs")}
                })
            })
            .collect::<Vec<_>>();
        let results = (0..12)
            .map(|index| {
                serde_json::json!({
                    "index": index,
                    "tool": "read",
                    "success": index != 9,
                    "exit_code": if index == 9 { 1 } else { 0 }
                })
            })
            .collect::<Vec<_>>();
        let args = serde_json::json!({"invocations": invocations});
        let meta = serde_json::json!({
            "execution_mode": "parallel",
            "applied_concurrency": 8,
            "success_count": 11,
            "failure_count": 1,
            "results": results
        });
        let rendered = render_tool_end("batch", 0, "", Some(&meta), Some(&args), 72);
        let plain = strip_ansi(&rendered);

        assert!(
            plain.contains("file-9.rs"),
            "failed item must stay visible: {plain}"
        );
        assert!(plain.contains("… +7 tools · Ctrl+T"), "{plain}");
        assert!(plain.lines().count() <= 7, "{plain}");
        assert_visible_lines_bounded(&rendered, 72);
    }

    #[test]
    fn exec_transcript_keeps_complete_command_output_and_terminal_status() {
        let output = (0..24)
            .map(|index| format!("output-line-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let args = serde_json::json!({"command":"cargo test --all-targets"});
        let rendered = render_tool_transcript(ToolTranscriptInput {
            name: "bash",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output: &output,
            metadata: None,
            args: Some(&args),
            duration: Some(std::time::Duration::from_millis(1250)),
            width: 80,
        });
        let plain = strip_ansi(&rendered);

        assert!(plain.starts_with("$ cargo test --all-targets\n"), "{plain}");
        assert!(
            rendered.contains(&Style::new().fg(TN_SUBTLE).render("$ ")),
            "full transcript prompt should use the shared subtle chrome: {rendered:?}"
        );
        assert!(
            rendered.contains(
                &Style::new()
                    .fg(tool_style::TOOL_PROGRAM_COLOR)
                    .render("cargo")
            ),
            "full transcript command should retain semantic shell highlighting: {rendered:?}"
        );
        for index in 0..24 {
            assert!(plain.contains(&format!("output-line-{index}")), "{plain}");
        }
        assert!(!plain.contains("… +"), "{plain}");
        assert!(plain.ends_with("✓ • 1.2s"), "{plain}");
        assert_visible_lines_bounded(&rendered, 80);
    }

    #[test]
    fn transcript_preserves_non_exec_results_and_protected_terminal_state() {
        let output = (0..18)
            .map(|index| format!("result-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let args = serde_json::json!({"query":"streaming markdown"});
        let rendered = render_tool_transcript(ToolTranscriptInput {
            name: "mcp__docs__search",
            state: ToolCallState::Denied,
            exit_code: Some(1),
            output: &output,
            metadata: None,
            args: Some(&args),
            duration: None,
            width: 72,
        });
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("Denied docs.search"), "{plain}");
        assert!(
            plain.contains("result-0") && plain.contains("result-17"),
            "{plain}"
        );
        assert!(!plain.contains("… +"), "{plain}");
        assert!(plain.ends_with("  └ ⊘ denied"), "{plain}");
        assert!(
            rendered.contains(&Style::new().fg(TN_YELLOW).bold().render("⊘")),
            "denial should use a warning glyph: {rendered:?}"
        );
        assert!(
            rendered.contains(&Style::new().fg(TN_GRAY).render("denied")),
            "denial label should remain neutral: {rendered:?}"
        );
        assert!(
            !rendered.contains(&Style::new().fg(TN_RED).render("denied")),
            "denial is a policy outcome, not an execution failure: {rendered:?}"
        );
        assert_visible_lines_bounded(&rendered, 72);
    }

    #[test]
    fn file_change_transcript_does_not_reuse_the_compact_diff_limit() {
        let before = (0..240)
            .map(|index| format!("old-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let after = (0..240)
            .map(|index| format!("new-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let meta = serde_json::json!({
            "file_path": "src/large.rs",
            "before": before,
            "after": after
        });
        let args = serde_json::json!({"patch":"*** Begin Patch"});
        let rendered = render_tool_transcript(ToolTranscriptInput {
            name: "patch",
            state: ToolCallState::Succeeded,
            exit_code: Some(0),
            output: "Applied 1 hunk.",
            metadata: Some(&meta),
            args: Some(&args),
            duration: Some(std::time::Duration::from_millis(25)),
            width: 96,
        });
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("old-239"), "{plain}");
        assert!(plain.contains("new-239"), "{plain}");
        assert!(plain.contains("Applied 1 hunk."), "{plain}");
        assert!(!plain.contains("diff truncated"), "{plain}");
        assert!(plain.ends_with("  └ ✓ • 25ms"), "{plain}");
        assert_visible_lines_bounded(&rendered, 96);
    }

    #[test]
    fn approval_label_shows_full_shell_command_and_patch_card_shows_preview() {
        let command = "printf 'one two three' && cargo test a-very-long-filter-name -- --nocapture";
        let shell = tool_approval_label("bash", Some(&serde_json::json!({"command": command})));
        assert_eq!(shell, format!("Bash({command})"));

        let args = serde_json::json!({
            "file_path":"src/lib.rs",
            "diff":"@@ -1,2 +1,2 @@\n-old value\n+new value\n keep"
        });
        assert_eq!(
            tool_approval_label("patch", Some(&args)),
            "Update(src/lib.rs)"
        );
        let patch = render_live_tool_activity(
            "patch",
            Some(&args),
            "",
            80,
            true,
            ToolCallState::AwaitingApproval,
        );
        assert!(
            patch.contains(&Style::new().fg(TN_SUBTLE).render("  │ ")),
            "approval preview should share the subtle message connector: {patch:?}"
        );
        let patch = strip_ansi(&patch);
        assert!(
            patch.starts_with("• Awaiting approval for src/lib.rs\n"),
            "{patch}"
        );
        assert!(patch.contains("  │ -old value"), "{patch}");
        assert!(patch.contains("  │ +new value"), "{patch}");
    }

    #[test]
    fn batch_approval_label_exposes_nested_commands_and_file_changes() {
        let args = serde_json::json!({
            "invocations":[
                {"tool":"bash", "args":{"command":"cargo test --workspace"}},
                {"tool":"write", "args":{"file_path":"notes.txt", "content":"first\nsecond"}}
            ]
        });
        let rendered = render_live_tool_activity(
            "batch",
            Some(&args),
            "",
            96,
            true,
            ToolCallState::AwaitingApproval,
        );
        let label = strip_ansi(&rendered);

        assert!(label.contains("1. Bash(cargo test --workspace)"), "{label}");
        assert!(label.contains("2. Write(notes.txt)"), "{label}");
        assert!(
            label.contains("+first") && label.contains("+second"),
            "{label}"
        );
    }
}
