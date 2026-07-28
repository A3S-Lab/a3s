//! Compact, decision-focused presentation for tool approvals.
//!
//! The transcript owns the operation preview. This surface keeps the pending
//! decision visible above the composer without repeating a warning-colored
//! paragraph or making ordinary option labels look dangerous.

use a3s_tui::event::{MouseButton, MouseEvent, MouseEventKind};
use a3s_tui::style::{fit_visible, strip_ansi, truncate_visible, visible_len, wrap_words, Style};

use super::{SURFACE_SELECTED, TN_FG, TN_GRAY, TN_RED, TN_SUBTLE, TN_YELLOW};

const OPTION_COUNT: usize = 4;
const MAX_DETAIL_ROWS: usize = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ApprovalPromptMsg {
    Selected(usize),
}

/// Approval-only picker with a stable semantic hierarchy:
/// warning glyph, neutral operation, neutral actions, and a red deny glyph.
#[derive(Clone, Debug)]
pub(super) struct ApprovalPrompt {
    label: String,
    selected: usize,
    y_offset: u16,
    denial_feedback: bool,
    saving_project_rule: bool,
}

impl ApprovalPrompt {
    pub(super) fn new(label: impl Into<String>, selected: usize) -> Self {
        Self {
            label: sanitize_label(&label.into()),
            selected: selected.min(OPTION_COUNT - 1),
            y_offset: 0,
            denial_feedback: false,
            saving_project_rule: false,
        }
    }

    pub(super) fn with_denial_feedback(mut self, enabled: bool) -> Self {
        self.denial_feedback = enabled;
        self
    }

    pub(super) fn with_project_rule_saving(mut self, enabled: bool) -> Self {
        self.saving_project_rule = enabled;
        self
    }

    pub(super) fn lines(&self, width: usize) -> Vec<String> {
        if width == 0 {
            return Vec::new();
        }

        if self.denial_feedback {
            return self.denial_feedback_lines(width);
        }
        if self.saving_project_rule {
            return self.project_rule_saving_lines(width);
        }

        let mut lines = vec![self.title_line(width)];
        lines.extend(self.detail_lines(width));
        lines.extend((0..OPTION_COUNT).map(|index| self.option_line(index, width)));
        lines.push(fit_visible(
            &Style::new().fg(TN_SUBTLE).render(&format!(
                "{}Enter select · ↑↓ move · Esc deny",
                indent(width)
            )),
            width,
        ));
        lines
    }

    pub(super) fn selected_index(&self) -> usize {
        self.selected.min(OPTION_COUNT - 1)
    }

    pub(super) fn set_y_offset(&mut self, y_offset: u16) {
        self.y_offset = y_offset;
    }

    pub(super) fn choice_start_row(&self, width: usize) -> usize {
        1 + self.detail_lines(width).len()
    }

    pub(super) fn handle_mouse(
        &mut self,
        mouse: &MouseEvent,
        width: usize,
    ) -> Option<ApprovalPromptMsg> {
        if width == 0 || self.denial_feedback || self.saving_project_rule {
            return None;
        }
        let local_row = mouse.row.checked_sub(self.y_offset)? as usize;
        if local_row >= self.lines(width).len() {
            return None;
        }

        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.selected = self.selected_index().saturating_sub(1);
                None
            }
            MouseEventKind::ScrollDown => {
                self.selected = (self.selected_index() + 1).min(OPTION_COUNT - 1);
                None
            }
            MouseEventKind::Down(MouseButton::Left) => {
                let index = local_row.checked_sub(self.choice_start_row(width))?;
                if index < OPTION_COUNT {
                    self.selected = index;
                    Some(ApprovalPromptMsg::Selected(index))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn title_line(&self, width: usize) -> String {
        let prefix = indent(width);
        let glyph = Style::new().fg(TN_YELLOW).bold().render("◆");
        let title = Style::new().fg(TN_FG).bold().render("Permission required");
        fit_visible(&format!("{prefix}{glyph} {title}"), width)
    }

    fn detail_lines(&self, width: usize) -> Vec<String> {
        if self.label.is_empty() {
            return Vec::new();
        }

        let first_prefix = format!(
            "{}{}  ",
            indent(width),
            Style::new().fg(TN_SUBTLE).render("Run")
        );
        let continuation = " ".repeat(visible_len(&first_prefix));
        let available = width.saturating_sub(visible_len(&first_prefix)).max(1);
        let mut rows = wrap_words(&self.label, available);
        if rows.len() > MAX_DETAIL_ROWS {
            rows.truncate(MAX_DETAIL_ROWS);
            if let Some(last) = rows.last_mut() {
                *last = ellipsize(last, available);
            }
        }

        rows.into_iter()
            .enumerate()
            .map(|(index, row)| {
                let prefix = if index == 0 {
                    first_prefix.as_str()
                } else {
                    continuation.as_str()
                };
                let detail = Style::new()
                    .fg(if index == 0 { TN_FG } else { TN_GRAY })
                    .render(&truncate_visible(&row, available));
                fit_visible(&format!("{prefix}{detail}"), width)
            })
            .collect()
    }

    fn option_line(&self, index: usize, width: usize) -> String {
        let (glyph, label, glyph_color) = match index {
            0 => ("↵", "Allow once", TN_SUBTLE),
            1 => ("◎", "Allow exact capability for this session", TN_SUBTLE),
            2 => ("⌘", "Add exact capability rule to project", TN_SUBTLE),
            _ => ("⊘", "Deny and tell the agent why", TN_RED),
        };
        let marker = if index == self.selected_index() {
            "❯"
        } else {
            " "
        };
        let raw = format!("{}{marker} {}  {glyph} {label}", indent(width), index + 1);

        if index == self.selected_index() {
            return Style::new()
                .fg(TN_FG)
                .bg(SURFACE_SELECTED)
                .render(&fit_visible(&raw, width));
        }

        let prefix = format!("{}{marker} {}  ", indent(width), index + 1);
        let prefix = Style::new().fg(TN_SUBTLE).render(&prefix);
        let glyph = Style::new().fg(glyph_color).render(glyph);
        let label = Style::new().fg(TN_FG).render(&format!(" {label}"));
        fit_visible(&format!("{prefix}{glyph}{label}"), width)
    }

    fn denial_feedback_lines(&self, width: usize) -> Vec<String> {
        let prefix = indent(width);
        let glyph = Style::new().fg(TN_RED).bold().render("⊘");
        let title = Style::new()
            .fg(TN_FG)
            .bold()
            .render("Tell the agent what to do instead");
        let mut lines = vec![fit_visible(&format!("{prefix}{glyph} {title}"), width)];
        lines.extend(self.detail_lines(width));
        lines.push(fit_visible(
            &Style::new()
                .fg(TN_GRAY)
                .render(&format!("{prefix}Type guidance in the composer below.")),
            width,
        ));
        lines.push(fit_visible(
            &Style::new()
                .fg(TN_SUBTLE)
                .render(&format!("{prefix}Enter deny with feedback · Esc back")),
            width,
        ));
        lines
    }

    fn project_rule_saving_lines(&self, width: usize) -> Vec<String> {
        let prefix = indent(width);
        let glyph = Style::new().fg(TN_YELLOW).bold().render("◆");
        let title = Style::new()
            .fg(TN_FG)
            .bold()
            .render("Saving project permission rule…");
        let mut lines = vec![fit_visible(&format!("{prefix}{glyph} {title}"), width)];
        lines.extend(self.detail_lines(width));
        lines.push(fit_visible(
            &Style::new().fg(TN_SUBTLE).render(&format!(
                "{prefix}The tool remains paused until the ACL write succeeds."
            )),
            width,
        ));
        lines
    }
}

fn indent(width: usize) -> String {
    " ".repeat(width.min(2))
}

fn ellipsize(value: &str, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if visible_len(value) < width {
        return format!("{value}…");
    }
    format!("{}…", truncate_visible(value, width.saturating_sub(1)))
}

fn sanitize_label(value: &str) -> String {
    strip_ansi(value)
        .chars()
        .filter_map(|ch| match ch {
            '\n' | '\r' | '\t' => Some(' '),
            ch if ch.is_control() => None,
            ch => Some(ch),
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::style::{strip_ansi, visible_len};

    #[test]
    fn approval_surface_uses_semantic_glyphs_and_neutral_labels() {
        let prompt = ApprovalPrompt::new("Bash(cargo test --workspace)", 0);
        let lines = prompt.lines(72);
        let plain = lines
            .iter()
            .map(|line| strip_ansi(line))
            .collect::<Vec<_>>();

        assert!(plain[0].contains("◆ Permission required"), "{plain:?}");
        assert!(
            plain[1].contains("Run  Bash(cargo test --workspace)"),
            "{plain:?}"
        );
        assert!(plain.iter().any(|line| line.contains("1  ↵ Allow once")));
        assert!(plain
            .iter()
            .any(|line| line.contains("2  ◎ Allow exact capability for this session")));
        assert!(plain
            .iter()
            .any(|line| line.contains("3  ⌘ Add exact capability rule to project")));
        assert!(plain
            .iter()
            .any(|line| line.contains("4  ⊘ Deny and tell the agent why")));
        assert!(lines[0].contains(&TN_YELLOW.fg_ansi()));
        assert!(lines.iter().any(|line| line.contains(&TN_RED.fg_ansi())));
        assert!(lines.iter().all(|line| visible_len(line) == 72));
    }

    #[test]
    fn long_operation_is_bounded_without_hiding_the_decision_rows() {
        let prompt = ApprovalPrompt::new(format!("Bash({})", "command ".repeat(40)), 1);
        for width in [24, 42, 80] {
            let lines = prompt.lines(width);
            assert!(lines.len() <= 8, "{lines:?}");
            assert_eq!(prompt.choice_start_row(width), 3);
            assert!(lines.iter().all(|line| visible_len(line) == width));
            assert!(strip_ansi(&lines[2]).contains('…'));
        }
    }

    #[test]
    fn denial_feedback_replaces_choices_with_composer_guidance() {
        let prompt = ApprovalPrompt::new("Write(src/lib.rs)", 3).with_denial_feedback(true);
        let plain = prompt
            .lines(56)
            .into_iter()
            .map(|line| strip_ansi(&line))
            .collect::<Vec<_>>();

        assert!(plain[0].contains("Tell the agent what to do instead"));
        assert!(plain.iter().any(|line| line.contains("Type guidance")));
        assert!(plain.iter().any(|line| line.contains("Enter deny")));
        assert!(!plain.iter().any(|line| line.contains("Allow once")));
    }
}
