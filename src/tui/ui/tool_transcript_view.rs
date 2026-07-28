//! Progressive detail layout for the full tool transcript.
//!
//! Compact history answers “what happened?”. Ctrl+T answers “with what input,
//! result, and terminal state?”. This component gives those details one stable
//! tree instead of repeating arguments in the heading and then emitting
//! disconnected payload and status rows.

use a3s_tui::style::{truncate_visible, visible_len, Style};

use super::TN_SUBTLE;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ToolTranscriptSection {
    label: String,
    body: String,
}

impl ToolTranscriptSection {
    pub(super) fn new(label: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            body: body.into(),
        }
    }

    fn is_empty(&self) -> bool {
        self.body.is_empty()
    }
}

/// Render labeled tool details beneath an already-rendered semantic heading.
///
/// A short value stays on the section row. Structured or wrapped content moves
/// below it with a continuation rail. The final section owns `└` unless a
/// terminal status follows it.
pub(super) fn render_tool_transcript_details(
    sections: impl IntoIterator<Item = ToolTranscriptSection>,
    status: Option<String>,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }

    let sections = sections
        .into_iter()
        .filter(|section| !section.is_empty())
        .collect::<Vec<_>>();
    let status = status.filter(|status| !status.is_empty());
    let mut rows = Vec::new();

    for (index, section) in sections.iter().enumerate() {
        let has_next = index + 1 < sections.len() || status.is_some();
        let connector = if has_next { "  ├ " } else { "  └ " };
        let body_rows = section.body.split('\n').collect::<Vec<_>>();
        let label = Style::new().fg(TN_SUBTLE).render(&section.label);
        let inline = (body_rows.len() == 1).then(|| {
            format!(
                "{}{label}  {}",
                Style::new().fg(TN_SUBTLE).render(connector),
                body_rows[0]
            )
        });

        if let Some(inline) = inline.filter(|inline| visible_len(inline) <= width) {
            rows.push(inline);
            continue;
        }

        rows.push(truncate_visible(
            &format!("{}{label}", Style::new().fg(TN_SUBTLE).render(connector)),
            width,
        ));
        let continuation = if has_next { "  │ " } else { "    " };
        let continuation = Style::new().fg(TN_SUBTLE).render(continuation);
        for row in body_rows {
            let available = width.saturating_sub(visible_len(&continuation)).max(1);
            rows.push(truncate_visible(
                &format!("{continuation}{}", truncate_visible(row, available)),
                width,
            ));
        }
    }

    if let Some(status) = status {
        for (index, row) in status.split('\n').enumerate() {
            let connector =
                Style::new()
                    .fg(TN_SUBTLE)
                    .render(if index == 0 { "  └ " } else { "    " });
            let available = width.saturating_sub(visible_len(&connector)).max(1);
            rows.push(truncate_visible(
                &format!("{connector}{}", truncate_visible(row, available)),
                width,
            ));
        }
    }

    rows.join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::style::strip_ansi;

    #[test]
    fn short_sections_form_one_compact_scan_path() {
        let rendered = render_tool_transcript_details(
            [
                ToolTranscriptSection::new("Input", "query=\"terminal UX\""),
                ToolTranscriptSection::new("Result", "3 matches"),
            ],
            Some("✓ • 120ms".to_string()),
            48,
        );

        assert_eq!(
            strip_ansi(&rendered),
            "  ├ Input  query=\"terminal UX\"\n  ├ Result  3 matches\n  └ ✓ • 120ms"
        );
    }

    #[test]
    fn multiline_sections_keep_the_tree_connected_and_width_bounded() {
        let rendered = render_tool_transcript_details(
            [ToolTranscriptSection::new(
                "Result",
                "{\n  \"items\": [\n    \"一个很长的结果\"\n  ]\n}",
            )],
            None,
            24,
        );
        let plain = strip_ansi(&rendered);

        assert_eq!(
            plain,
            "  └ Result\n    {\n      \"items\": [\n        \"一个很长的结果\"\n      ]\n    }"
        );
        assert!(rendered.lines().all(|row| visible_len(row) <= 24));
    }

    #[test]
    fn empty_sections_do_not_leave_orphaned_connectors() {
        let rendered = render_tool_transcript_details(
            [ToolTranscriptSection::new("Input", "")],
            Some("⊘ denied".to_string()),
            24,
        );

        assert_eq!(strip_ansi(&rendered), "  └ ⊘ denied");
    }

    #[test]
    fn multiline_terminal_status_keeps_all_words_on_narrow_rows() {
        let rendered = render_tool_transcript_details(
            [ToolTranscriptSection::new("Result", "partial output")],
            Some("! partial · 1\nfailed".to_string()),
            18,
        );

        assert_eq!(
            strip_ansi(&rendered),
            "  ├ Result\n  │ partial output\n  └ ! partial · 1\n    failed"
        );
    }
}
