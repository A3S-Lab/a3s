//! Progressive-disclosure rendering for file changes.
//!
//! Compact history gets a width-aware preview that cannot consume an entire
//! terminal. Ctrl+T uses the same visual grammar without the compact row cap.

use a3s_tui::style::{fit_visible, strip_ansi, Color, Style};

use super::{
    agent_chrome, agent_chrome_theme, highlight_diff_spans, lang_of, DiffLineKind, DiffSpan,
};

const COMPACT_DIFF_ROWS_NARROW: usize = 8;
const COMPACT_DIFF_ROWS_MEDIUM: usize = 12;
const COMPACT_DIFF_ROWS_WIDE: usize = 18;

pub(super) const DIFF_HEADER_BULLET: Color = Color::Rgb(120, 123, 125);
pub(super) const DIFF_HEADER_ACTION: Color = Color::Rgb(255, 255, 255);
pub(super) const DIFF_HEADER_DETAIL: Color = Color::Rgb(220, 220, 220);
pub(super) const DIFF_CONTEXT_GUTTER: Color = Color::Rgb(120, 123, 125);
const DIFF_INSERT_GUTTER: Color = Color::Rgb(122, 139, 131);
const DIFF_DELETE_GUTTER: Color = Color::Rgb(150, 125, 123);
pub(super) const DIFF_INSERT_MARKER: Color = Color::Rgb(0, 194, 0);
pub(super) const DIFF_DELETE_MARKER: Color = Color::Rgb(180, 60, 42);
pub(super) const DIFF_INSERT_BG: Color = Color::Rgb(24, 59, 42);
pub(super) const DIFF_DELETE_BG: Color = Color::Rgb(80, 31, 27);
const DIFF_CODE_FG: Color = Color::Rgb(203, 214, 247);

pub(super) fn compact_diff_row_budget(width: usize) -> usize {
    match width {
        0..=39 => COMPACT_DIFF_ROWS_NARROW,
        40..=79 => COMPACT_DIFF_ROWS_MEDIUM,
        _ => COMPACT_DIFF_ROWS_WIDE,
    }
}

pub(super) fn render_compact_file_change(
    action: &str,
    path: &str,
    before: &str,
    after: &str,
    width: usize,
) -> String {
    let rendered = render_file_change(
        action,
        path,
        before,
        after,
        width,
        compact_diff_row_budget(width),
    );
    rewrite_compact_truncation_notice(&rendered, width)
}

pub(super) fn render_full_file_change(
    action: &str,
    path: &str,
    before: &str,
    after: &str,
    width: usize,
) -> String {
    render_file_change(action, path, before, after, width, u16::MAX as usize)
}

fn render_file_change(
    action: &str,
    path: &str,
    before: &str,
    after: &str,
    width: usize,
    max_rows: usize,
) -> String {
    let theme = agent_chrome_theme();
    let chrome = agent_chrome(&theme);
    let lang = lang_of(std::path::Path::new(path));
    chrome
        .diff_texts(path, before, after)
        .action(action)
        .header_colors(DIFF_HEADER_BULLET, DIFF_HEADER_ACTION, DIFF_HEADER_DETAIL)
        .context_color(DIFF_CODE_FG)
        .separator_color(DIFF_CONTEXT_GUTTER)
        .gutter_colors(DIFF_CONTEXT_GUTTER, DIFF_INSERT_GUTTER, DIFF_DELETE_GUTTER)
        .marker_colors(DIFF_INSERT_MARKER, DIFF_DELETE_MARKER)
        .changed_content_colors(DIFF_CODE_FG, mix_diff_color(DIFF_CODE_FG, DIFF_DELETE_BG))
        .changed_backgrounds(Some(DIFF_INSERT_BG), Some(DIFF_DELETE_BG))
        .highlight_content(|kind, content| {
            highlight_diff_spans(content, lang)
                .into_iter()
                .map(|span| {
                    let color = span.color.unwrap_or(DIFF_CODE_FG);
                    let color = if kind == DiffLineKind::Delete {
                        mix_diff_color(color, DIFF_DELETE_BG)
                    } else {
                        color
                    };
                    DiffSpan::new(span.content).color(color)
                })
                .collect()
        })
        .max_lines(max_rows)
        .view(
            width.min(u16::MAX as usize) as u16,
            max_rows.saturating_add(2),
        )
}

fn rewrite_compact_truncation_notice(rendered: &str, width: usize) -> String {
    rendered
        .lines()
        .map(|line| {
            if strip_ansi(line).contains("diff truncated") {
                Style::new()
                    .fg(DIFF_CONTEXT_GUTTER)
                    .render(&fit_visible("    … diff · Ctrl+T", width))
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn mix_diff_color(foreground: Color, background: Color) -> Color {
    match (foreground, background) {
        (Color::Rgb(fr, fg, fb), Color::Rgb(br, bg, bb)) => Color::Rgb(
            ((u16::from(fr) + u16::from(br)) / 2) as u8,
            ((u16::from(fg) + u16::from(bg)) / 2) as u8,
            ((u16::from(fb) + u16::from(bb)) / 2) as u8,
        ),
        (color, _) => color,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::style::{strip_ansi, visible_len};

    #[test]
    fn compact_budget_grows_with_available_horizontal_space() {
        assert_eq!(compact_diff_row_budget(24), 8);
        assert_eq!(compact_diff_row_budget(48), 12);
        assert_eq!(compact_diff_row_budget(80), 18);
    }

    #[test]
    fn compact_diff_points_to_the_complete_transcript() {
        let before = (0..80)
            .map(|index| format!("old-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let after = (0..80)
            .map(|index| format!("new-{index}"))
            .collect::<Vec<_>>()
            .join("\n");

        for width in [24, 48, 80] {
            let rendered =
                render_compact_file_change("Edited", "src/large.rs", &before, &after, width);
            let plain = strip_ansi(&rendered);

            assert!(plain.contains("diff · Ctrl+T"), "{plain}");
            assert!(!plain.contains("diff truncated"), "{plain}");
            assert!(!plain.contains("old-79"), "{plain}");
            assert!(
                rendered.lines().count() <= compact_diff_row_budget(width) + 2,
                "{plain}"
            );
            assert!(rendered.lines().all(|line| visible_len(line) <= width));
        }
    }

    #[test]
    fn full_diff_preserves_the_tail_without_a_compact_hint() {
        let before = (0..40)
            .map(|index| format!("old-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let after = (0..40)
            .map(|index| format!("new-{index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let rendered = render_full_file_change("Edited", "src/large.rs", &before, &after, 80);
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("old-39"), "{plain}");
        assert!(plain.contains("new-39"), "{plain}");
        assert!(!plain.contains("diff · Ctrl+T"), "{plain}");
    }
}
