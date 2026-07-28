//! Shared visual semantics for transcript messages.
//!
//! Message renderers own their content, but state, markers, and connector
//! contrast must remain stable across tools, reasoning, and delegated agents.

use a3s_tui::style::{strip_ansi, truncate_visible, wrap_words, Color, Style};

use super::runtime_projection::{SubagentOutcome, ToolCallState};
use super::{ACCENT, TN_FG, TN_GRAY, TN_GREEN, TN_PURPLE, TN_RED, TN_SUBTLE, TN_YELLOW};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MessageTone {
    Neutral,
    Active,
    Inactive,
    Success,
    Warning,
    Error,
    Reasoning,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum NoticeKind {
    Info,
    Success,
    Warning,
    Error,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum MessageBranch {
    Fork,
    Last,
    Pipe,
    Indent,
}

impl MessageTone {
    pub(super) fn color(self) -> Color {
        match self {
            Self::Neutral | Self::Inactive => TN_GRAY,
            Self::Active => ACCENT,
            Self::Success => TN_GREEN,
            Self::Warning => TN_YELLOW,
            Self::Error => TN_RED,
            Self::Reasoning => TN_PURPLE,
        }
    }
}

pub(super) fn tool_message_tone(state: ToolCallState, activity_phase: bool) -> MessageTone {
    match state {
        ToolCallState::Preparing => MessageTone::Neutral,
        ToolCallState::AwaitingApproval => MessageTone::Warning,
        ToolCallState::Running if activity_phase => MessageTone::Active,
        ToolCallState::Running => MessageTone::Inactive,
        ToolCallState::Succeeded => MessageTone::Success,
        ToolCallState::Failed | ToolCallState::TimedOut => MessageTone::Error,
        ToolCallState::Denied | ToolCallState::Interrupted => MessageTone::Warning,
    }
}

pub(super) fn result_message_tone(ok: bool) -> MessageTone {
    if ok {
        MessageTone::Success
    } else {
        MessageTone::Error
    }
}

pub(super) fn subagent_message_tone(outcome: SubagentOutcome) -> MessageTone {
    match outcome {
        SubagentOutcome::Succeeded => MessageTone::Success,
        SubagentOutcome::Failed => MessageTone::Error,
        SubagentOutcome::Cancelled | SubagentOutcome::TrackingLost => MessageTone::Warning,
    }
}

pub(super) fn message_marker(tone: MessageTone) -> String {
    let style = Style::new().fg(tone.color());
    if matches!(
        tone,
        MessageTone::Active | MessageTone::Success | MessageTone::Warning | MessageTone::Error
    ) {
        style.bold().render("•")
    } else {
        style.render("•")
    }
}

/// Render a compact status with semantic color confined to its glyph.
///
/// The label remains readable in monochrome terminals and avoids turning a
/// complete result row into a success/error banner.
pub(super) fn message_status(glyph: &str, label: &str, tone: MessageTone, quiet: bool) -> String {
    let glyph = Style::new().fg(tone.color()).bold().render(glyph);
    let label = label.trim();
    if label.is_empty() {
        glyph
    } else {
        format!(
            "{glyph} {}",
            Style::new()
                .fg(if quiet { TN_GRAY } else { TN_FG })
                .render(label)
        )
    }
}

pub(super) fn message_title(title: &str, quiet: bool) -> String {
    Style::new()
        .fg(if quiet { TN_GRAY } else { TN_FG })
        .bold()
        .render(title)
}

pub(super) fn message_branch(kind: MessageBranch) -> String {
    let prefix = match kind {
        MessageBranch::Fork => "  ├ ",
        MessageBranch::Last => "  └ ",
        MessageBranch::Pipe => "  │ ",
        MessageBranch::Indent => "    ",
    };
    Style::new().fg(TN_SUBTLE).render(prefix)
}

pub(super) fn render_notice(kind: NoticeKind, source: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let source = sanitize_message_source(source);
    let source = source.trim();
    if source.is_empty() {
        return String::new();
    }

    let tone = match kind {
        NoticeKind::Info => MessageTone::Neutral,
        NoticeKind::Success => MessageTone::Success,
        NoticeKind::Warning => MessageTone::Warning,
        NoticeKind::Error => MessageTone::Error,
    };
    let body_color = if kind == NoticeKind::Info {
        TN_GRAY
    } else {
        TN_FG
    };
    let body_width = width.saturating_sub(4).max(1);
    let mut first = true;
    let mut rows = Vec::new();
    for logical in source.lines() {
        for row in wrap_words(logical, body_width) {
            let prefix = if first {
                format!("  {} ", message_marker(tone))
            } else {
                "    ".to_string()
            };
            let row = Style::new().fg(body_color).render(&row);
            rows.push(truncate_visible(&format!("{prefix}{row}"), width));
            first = false;
        }
    }
    rows.join("\n")
}

pub(super) fn sanitize_message_source(source: &str) -> String {
    strip_ansi(source)
        .chars()
        .filter_map(|ch| match ch {
            '\n' => Some('\n'),
            '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_states_have_stable_product_semantics() {
        assert_eq!(
            tool_message_tone(ToolCallState::Preparing, true),
            MessageTone::Neutral
        );
        assert_eq!(
            tool_message_tone(ToolCallState::Running, true),
            MessageTone::Active
        );
        assert_eq!(
            tool_message_tone(ToolCallState::Running, false),
            MessageTone::Inactive
        );
        assert_eq!(
            tool_message_tone(ToolCallState::AwaitingApproval, true),
            MessageTone::Warning
        );
        assert_eq!(
            tool_message_tone(ToolCallState::Succeeded, true),
            MessageTone::Success
        );
        assert_eq!(
            tool_message_tone(ToolCallState::Failed, true),
            MessageTone::Error
        );
        assert_eq!(
            tool_message_tone(ToolCallState::Denied, true),
            MessageTone::Warning
        );
        assert_eq!(
            tool_message_tone(ToolCallState::Interrupted, true),
            MessageTone::Warning
        );
        assert_eq!(
            tool_message_tone(ToolCallState::TimedOut, true),
            MessageTone::Error
        );
    }

    #[test]
    fn notices_color_only_the_marker_and_stay_responsive() {
        for (kind, tone) in [
            (NoticeKind::Info, MessageTone::Neutral),
            (NoticeKind::Success, MessageTone::Success),
            (NoticeKind::Warning, MessageTone::Warning),
            (NoticeKind::Error, MessageTone::Error),
        ] {
            for width in [24, 48, 80] {
                let rendered = render_notice(
                    kind,
                    "模型连接失败，请检查 provider configuration before retrying",
                    width,
                );
                assert!(rendered.contains(&message_marker(tone)), "{rendered:?}");
                for row in rendered.lines() {
                    assert!(a3s_tui::style::visible_len(row) <= width, "{rendered:?}");
                }
            }
        }
    }

    #[test]
    fn notices_strip_hostile_terminal_controls() {
        let rendered = render_notice(NoticeKind::Error, "\x1b[2Jbad\0 message", 40);

        assert_eq!(a3s_tui::style::strip_ansi(&rendered), "  • bad message");
        assert!(!rendered.contains("\x1b[2J"));
        assert!(!rendered.contains('\0'));
    }

    #[test]
    fn status_color_is_confined_to_the_glyph() {
        let rendered = message_status("⊘", "denied", MessageTone::Warning, true);

        assert_eq!(strip_ansi(&rendered), "⊘ denied");
        assert!(rendered.contains(&Style::new().fg(TN_YELLOW).bold().render("⊘")));
        assert!(rendered.contains(&Style::new().fg(TN_GRAY).render("denied")));
        assert!(!rendered.contains(&Style::new().fg(TN_YELLOW).render("denied")));
    }
}
