//! Small formatting + layout helpers shared across the TUI.

use super::*;

/// Pad a (possibly styled) string with spaces to `width` display columns.
pub(crate) fn pad_to(s: &str, width: usize) -> String {
    let vis = a3s_tui::style::visible_len(s);
    if vis >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - vis))
    }
}

/// Render a user-authored turn with the same cell geometry as Codex CLI.
///
/// The user surface owns one blank row above and below its content, uses `› `
/// only on the first content row, aligns continuations under the text, and
/// reserves one right-hand wrapping column. The low-contrast surface already
/// matches Codex's 12% white blend over the dark terminal canvas.
pub(crate) fn user_bubble(content: &str, width: usize) -> String {
    if content.is_empty() || width == 0 {
        return String::new();
    }

    let margin = PAD;
    // Keep at least one display column for message content after the marker and
    // its gap. On smaller viewports the surface remains, but the rail yields.
    let marker = if width.saturating_sub(margin) >= 3 {
        "›"
    } else {
        ""
    };
    let gap = usize::from(!marker.is_empty());
    let prefix_width = a3s_tui::style::visible_len(marker) + gap;
    let right_margin = usize::from(width.saturating_sub(prefix_width) > 1);
    let body_width = width
        .saturating_sub(margin)
        .saturating_sub(prefix_width)
        .saturating_sub(right_margin)
        .max(1);
    let lines = content
        .split('\n')
        .flat_map(|line| wrap_user_line(line, body_width))
        .collect::<Vec<_>>();
    let padding = Style::new()
        .bg(SURFACE_USER)
        .render(&" ".repeat(width.saturating_sub(margin)));
    let mut rows = Vec::with_capacity(lines.len().saturating_add(2));
    rows.push(format!("{}{padding}", " ".repeat(margin)));
    for (index, line) in lines.into_iter().enumerate() {
        let prefix = if index == 0 && !marker.is_empty() {
            format!(
                "{}{}",
                Style::new()
                    .fg(TN_FG)
                    .bg(SURFACE_USER)
                    .bold()
                    .dim()
                    .render(marker),
                Style::new().bg(SURFACE_USER).render(&" ".repeat(gap))
            )
        } else {
            Style::new()
                .bg(SURFACE_USER)
                .render(&" ".repeat(prefix_width))
        };
        let content = a3s_tui::style::fit_visible(&line, body_width);
        let content = format!("{content}{}", " ".repeat(right_margin));
        rows.push(format!(
            "{}{prefix}{}",
            " ".repeat(margin),
            Style::new().fg(TN_FG).bg(SURFACE_USER).render(&content)
        ));
    }
    rows.push(format!("{}{padding}", " ".repeat(margin)));
    rows.join("\n")
}

fn wrap_user_line(line: &str, width: usize) -> Vec<String> {
    if line.is_empty() {
        return vec![String::new()];
    }
    let mut rows = Vec::new();
    let mut remaining = line;
    while !remaining.is_empty() {
        let end = visible_prefix_byte_end(remaining, width);
        let end = if end == 0 {
            remaining
                .char_indices()
                .nth(1)
                .map(|(offset, _)| offset)
                .unwrap_or(remaining.len())
        } else {
            end
        };
        rows.push(remaining[..end].to_string());
        remaining = &remaining[end..];
    }
    if rows.is_empty() {
        rows.push(String::new());
    }
    rows
}

/// Return a source byte boundary that fits whole display cells in `width`.
///
/// Advancing by source bytes prevents a wide glyph that crosses a display
/// column boundary from being omitted from the next row. Zero-width combining
/// marks remain attached to the preceding display cell.
fn visible_prefix_byte_end(value: &str, width: usize) -> usize {
    if width == 0 {
        return 0;
    }
    let mut used = 0usize;
    let mut end = 0usize;
    for (offset, ch) in value.char_indices() {
        let char_width = a3s_tui::style::visible_len(&ch.to_string());
        if char_width > 0 && used.saturating_add(char_width) > width {
            break;
        }
        used = used.saturating_add(char_width);
        end = offset + ch.len_utf8();
    }
    end
}

/// Prefix a message block with a Codex-style colored • gutter on its first line
/// and align the rest under the text.
pub(crate) fn gutter(color: Color, content: &str) -> String {
    let lines = content.lines().collect::<Vec<_>>();
    if lines.is_empty() {
        return String::new();
    }

    a3s_tui::components::GutterBlock::lines(lines)
        .margin(PAD)
        .marker("•")
        .marker_color(color)
        .view()
}

/// Render one assistant-authored Markdown cell.
///
/// Vertical separation belongs to the transcript compositor, not the cell.
/// Keeping cells spacing-free gives messages, notices, and tool calls the same
/// boundary rule and avoids doubled rows when two padded cells meet.
pub(crate) fn assistant_block(content: &str, width: usize) -> String {
    assistant_body(content, width)
}

/// Split a streaming assistant cell at its stable/tail boundary. The stable
/// fragment is newline-terminated so `Viewport::set_content_parts` can retain
/// its wrapped rows while replacing only the mutable tail. The caller inserts
/// the same transcript gap used by committed entries before this cell.
pub(crate) fn assistant_stream_block_parts(
    stable: &str,
    tail: &str,
    width: usize,
) -> Option<(String, String)> {
    let stable = assistant_body(stable, width);
    let tail = assistant_body(tail, width);
    if stable.is_empty() && tail.is_empty() {
        return None;
    }

    let mut prefix = String::new();
    if !stable.is_empty() {
        prefix.push_str(&stable);
        prefix.push('\n');
    }
    Some((prefix, tail))
}

fn assistant_body(content: &str, width: usize) -> String {
    if content.is_empty() || width == 0 {
        String::new()
    } else {
        gutter(TN_GRAY, content)
    }
}

fn input_chrome_width(width: usize) -> u16 {
    width.min(u16::MAX as usize) as u16
}

pub(crate) fn input_rule(width: usize, color: Color) -> String {
    if width == 0 {
        return String::new();
    }

    let theme = agent_chrome_theme();
    agent_chrome(&theme)
        .input_border()
        .margin(PAD)
        .rule_color(color)
        .view(input_chrome_width(width))
}

pub(crate) fn input_gradient_rule(width: usize, palette: &[Color], offset: usize) -> String {
    if width == 0 || palette.is_empty() {
        return String::new();
    }

    let theme = agent_chrome_theme();
    agent_chrome(&theme)
        .input_border()
        .margin(PAD)
        .rule('━')
        .rainbow(palette.to_vec(), offset)
        .view(input_chrome_width(width))
}

pub(crate) fn input_status_rule(width: usize, border_color: Color, label: &str) -> String {
    if width == 0 {
        return String::new();
    }

    let theme = agent_chrome_theme();
    agent_chrome(&theme)
        .input_border()
        .margin(PAD)
        .rule_color(border_color)
        .label(label)
        .view(input_chrome_width(width))
}

pub(crate) fn input_prompt_line(
    prompt: &str,
    color: Color,
    text: &str,
    tint_text: bool,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }

    let theme = agent_chrome_theme();
    let chrome = agent_chrome(&theme);
    let mut line = chrome
        .prompt(format!("{prompt} "))
        .text(text)
        .margin(PAD)
        .width(width)
        .prompt_style(Style::new().fg(color).bold());
    if tint_text {
        line = line.text_style(Style::new().fg(color));
    }
    line.view()
}

pub(crate) fn thinking_block(text: &str, width: usize) -> String {
    let text = text.trim();
    if text.is_empty() || width == 0 {
        return String::new();
    }

    a3s_tui::components::WrappedPrefixBlock::new(text)
        .margin(PAD)
        .width(width)
        .prefixes("• ", "  ")
        .style(Style::new().fg(TN_GRAY).italic())
        .view()
}

pub(crate) fn compact_progress_line(elapsed: Duration, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let prefix = Style::new().fg(ACCENT).render(&format!(
        "✦ Compacting context… {} / {} ",
        fmt_elapsed(elapsed),
        fmt_elapsed(crate::compact::MANUAL_COMPACT_TIMEOUT),
    ));
    let progress_width = width
        .saturating_sub(a3s_tui::style::visible_len(&prefix))
        .clamp(1, 29);
    let phase = (elapsed.as_millis() / 120) % 20;
    let pulse = if phase <= 10 { phase } else { 20 - phase };
    let value = 0.15 + (pulse as f64 / 10.0) * 0.7;
    let progress = a3s_tui::components::Progress::new()
        .value(value)
        .width(progress_width.min(u16::MAX as usize) as u16)
        .show_percentage(false)
        .filled_char('▰')
        .empty_char('▱')
        .filled_color(ACCENT)
        .empty_color(TN_GRAY)
        .view();

    a3s_tui::style::fit_visible(&format!("{prefix}{progress}"), width)
}

/// Greedy word-wrap of plain text to `width` display columns, with blank lines
/// dropped so compact previews stay single-spaced.
pub(crate) fn wrap_words(text: &str, width: usize) -> Vec<String> {
    a3s_tui::style::wrap_words_compact(text, width)
}

/// Byte offset of the char at index `char_idx` (for in-place string edits).
pub(crate) fn char_byte(s: &str, char_idx: usize) -> usize {
    s.char_indices()
        .nth(char_idx)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// A fresh session id for each launch (timestamp + pid; UUID-ish, no dep).
pub(crate) fn new_session_id() -> String {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or(0);
    format!("{:016x}-{:x}", nanos, std::process::id())
}

/// "1h 32m" / "1m 05s" / "42s".
pub(crate) fn fmt_elapsed(d: Duration) -> String {
    let s = d.as_secs();
    if s >= 3600 {
        format!("{}h {:02}m", s / 3600, (s % 3600) / 60)
    } else if s >= 60 {
        format!("{}m {:02}s", s / 60, s % 60)
    } else {
        format!("{s}s")
    }
}

/// "79.9k" / "512".
pub(crate) fn humanize(n: usize) -> String {
    if n >= 1000 {
        format!("{:.1}k", n as f64 / 1000.0)
    } else {
        n.to_string()
    }
}

/// Render `text` with a soft highlight gliding left-to-right (loading shimmer).
pub(crate) fn shimmer(text: &str, phase: usize) -> String {
    a3s_tui::components::ShimmerText::new(text)
        .phase(phase)
        .colors(TN_GRAY, TN_FG)
        .spread(5.0)
        .speed_divisor(3)
        .cycle_gap(12)
        .view()
}

/// Truncate to `max` DISPLAY COLUMNS (not chars) with an ellipsis. Callers pass
/// a column budget (panel widths), so counting chars overflowed the fixed-height
/// panels on CJK/wide text (every CJK char is 2 columns) and corrupted the
/// layout. Delegates to the width-aware, ANSI-preserving tui helper.
pub(crate) fn truncate(s: &str, max: usize) -> String {
    a3s_tui::style::truncate_visible(s, max)
}

#[cfg(test)]
mod tests {
    use super::{
        assistant_block, assistant_stream_block_parts, compact_progress_line, gutter,
        input_gradient_rule, input_prompt_line, input_rule, input_status_rule, shimmer,
        thinking_block, truncate, user_bubble, wrap_words, ACCENT, SURFACE_USER, TN_FG, TN_GRAY,
    };
    use a3s_tui::style::{strip_ansi, visible_len, Color, Style};
    use std::time::Duration;

    #[test]
    fn wraps_on_word_boundaries_without_splitting_words() {
        let lines = wrap_words("the quick brown fox jumps", 9);
        assert!(lines.iter().all(|l| l.chars().count() <= 9), "{lines:?}");
        // No word is broken: rejoining with spaces reproduces the input words.
        assert_eq!(
            lines.join(" ").split_whitespace().collect::<Vec<_>>(),
            vec!["the", "quick", "brown", "fox", "jumps"]
        );
    }

    #[test]
    fn collapses_blank_lines_to_stay_single_spaced() {
        let lines = wrap_words("alpha\n\n\nbeta", 40);
        assert_eq!(lines, vec!["alpha".to_string(), "beta".to_string()]);
    }

    #[test]
    fn hard_breaks_a_word_longer_than_width() {
        let lines = wrap_words("supercalifragilistic", 5);
        assert!(lines.iter().all(|l| l.chars().count() <= 5), "{lines:?}");
        assert_eq!(lines.concat(), "supercalifragilistic");
    }

    #[test]
    fn never_returns_empty_for_blank_input() {
        assert_eq!(wrap_words("   ", 10), vec![String::new()]);
    }

    #[test]
    fn wrap_words_counts_display_columns_for_wide_unicode() {
        // 6 wide chars = 12 columns; this text has no spaces so it's one token that
        // must hard-break by COLUMN budget, never exceeding the width.
        let lines = wrap_words("かなテストあ", 8);
        for l in &lines {
            assert!(
                a3s_tui::style::visible_len(l) <= 8,
                "line wider than 8 columns: {l:?}"
            );
        }
        assert_eq!(lines.concat(), "かなテストあ");
    }

    #[test]
    fn wrap_words_uses_shared_compact_width_helper() {
        let text = "alpha\n\n中文测试内容 beta";

        assert_eq!(
            wrap_words(text, 8),
            a3s_tui::style::wrap_words_compact(text, 8)
        );
    }

    #[test]
    fn truncate_budgets_display_columns_not_chars() {
        // 5 wide chars = 10 columns; a 6-column budget must fit (<= 6 cols incl ...),
        // which char-counting would have overflowed to ~10 columns.
        let out = truncate("アイウエオ", 6);
        assert!(
            a3s_tui::style::visible_len(&out) <= 6,
            "truncated string exceeds 6 columns: {out:?}"
        );
        assert!(out.ends_with('…'));
        // Fits-as-is when within budget.
        assert_eq!(truncate("ok", 6), "ok");
    }

    #[test]
    fn gutter_uses_shared_block_shape() {
        let rendered = gutter(Color::Green, "hello\nworld");
        let plain = strip_ansi(&rendered);

        assert_eq!(plain, "• hello\n  world");
        assert!(rendered.contains("\x1b[1;32m•\x1b[0m"));
    }

    #[test]
    fn gutter_preserves_styled_content() {
        let styled = Style::new().fg(Color::Yellow).render("styled");
        let rendered = gutter(Color::Green, &styled);

        assert!(rendered.contains("\x1b[33mstyled\x1b[0m"));
        assert_eq!(strip_ansi(&rendered), "• styled");
    }

    #[test]
    fn gutter_keeps_empty_input_empty() {
        assert_eq!(gutter(Color::Green, ""), "");
    }

    #[test]
    fn assistant_block_leaves_vertical_spacing_to_the_transcript() {
        let rendered = assistant_block("hello\nworld", 20);
        let plain = strip_ansi(&rendered);

        assert_eq!(plain.lines().collect::<Vec<_>>(), ["• hello", "  world"]);
        assert!(plain.lines().all(|row| !row.trim().is_empty()));
        assert!(rendered.lines().all(|row| visible_len(row) <= 20));
        assert_eq!(assistant_block("", 20), "");
        assert_eq!(assistant_block("hello", 0), "");
    }

    #[test]
    fn streaming_assistant_parts_do_not_create_private_gap_rows() {
        for (stable, tail) in [("stable", ""), ("", "tail"), ("stable", "tail")] {
            let (prefix, suffix) =
                assistant_stream_block_parts(stable, tail, 20).expect("stream parts");
            if stable.is_empty() {
                assert!(prefix.is_empty());
            } else {
                assert!(prefix.ends_with('\n'));
            }
            let rendered = strip_ansi(&format!("{prefix}{suffix}"));
            let rows = rendered.lines().collect::<Vec<_>>();

            assert_eq!(
                rows.iter().filter(|row| row.trim().is_empty()).count(),
                0,
                "{rendered:?}"
            );
        }

        let (stable_prefix, stable_suffix) =
            assistant_stream_block_parts("stable", "", 20).expect("stable parts");
        assert_eq!(
            format!("{stable_prefix}{stable_suffix}").trim_end_matches('\n'),
            assistant_block("stable", 20)
        );
        let (tail_prefix, tail_suffix) =
            assistant_stream_block_parts("", "tail", 20).expect("tail parts");
        assert_eq!(
            format!("{tail_prefix}{tail_suffix}"),
            assistant_block("tail", 20)
        );
        assert!(assistant_stream_block_parts("", "", 20).is_none());
        assert!(assistant_stream_block_parts("stable", "tail", 0).is_none());
    }

    #[test]
    fn user_bubble_uses_codex_surface_and_message_rail() {
        let rendered = user_bubble("hello\nworld", 20);
        let plain = strip_ansi(&rendered);
        let rows = plain.lines().collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![
                "                    ",
                "› hello             ",
                "  world             ",
                "                    ",
            ]
        );
        assert!(rendered.lines().all(|row| visible_len(row) == 20));
        assert!(rendered.contains(
            &Style::new()
                .fg(TN_FG)
                .bg(SURFACE_USER)
                .bold()
                .dim()
                .render("›")
        ));
        assert!(rendered.contains(
            &Style::new()
                .fg(TN_FG)
                .bg(SURFACE_USER)
                .render("hello             ")
        ));
        assert!(rows[0].trim().is_empty() && rows[3].trim().is_empty());
        assert!(rendered
            .lines()
            .all(|row| row.contains(&format!("\x1b[{}m", SURFACE_USER.bg_ansi()))));
    }

    #[test]
    fn user_bubble_keeps_empty_input_empty() {
        assert_eq!(user_bubble("", 20), "");
        assert_eq!(user_bubble("hello", 0), "");
    }

    #[test]
    fn user_bubble_stays_within_narrow_viewport() {
        let rendered = user_bubble("hi", 6);
        let rows = strip_ansi(&rendered)
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(rows, vec!["      ", "› hi  ", "      "]);
        assert!(rendered.lines().all(|line| visible_len(line) == 6));

        for width in 1..=5 {
            let rendered = user_bubble("hi", width);
            let plain = strip_ansi(&rendered);
            assert!(
                rendered.lines().all(|line| visible_len(line) == width),
                "width {width}: {plain:?}"
            );
            assert!(plain.contains('h'), "width {width}: {plain:?}");
            assert!(plain.contains('i'), "width {width}: {plain:?}");
            assert!(!plain.contains('…'), "width {width}: {plain:?}");
        }
    }

    #[test]
    fn user_bubble_wraps_long_and_wide_text_without_losing_content() {
        let rendered = user_bubble("abcdefghij\n中文测试", 8);
        let rows = strip_ansi(&rendered)
            .lines()
            .map(str::to_string)
            .collect::<Vec<_>>();

        assert_eq!(
            rows,
            vec![
                "        ",
                "› abcde ",
                "  fghij ",
                "  中文  ",
                "  测试  ",
                "        ",
            ]
        );
        assert!(rendered.lines().all(|line| visible_len(line) == 8));
    }

    #[test]
    fn input_prompt_line_uses_shared_prompt_component() {
        let rendered = input_prompt_line("❯", Color::Cyan, "cargo test\n--all", false, 24);
        let plain = strip_ansi(&rendered);
        let rows = plain.lines().collect::<Vec<_>>();

        assert!(rows[0].starts_with("❯ cargo test"));
        assert!(rows[1].starts_with("  --all"));
        assert!(rendered.lines().all(|line| visible_len(line) == 24));
        assert!(rendered.contains(&Style::new().fg(Color::Cyan).bold().render("❯ ")));
        assert!(!rendered.contains(&format!("\x1b[{}m", SURFACE_USER.bg_ansi())));
    }

    #[test]
    fn input_prompt_line_can_tint_modal_input_text() {
        let rendered = input_prompt_line("?", Color::Cyan, "research mode", true, 28);

        assert_eq!(strip_ansi(&rendered).trim_end(), "? research mode");
        assert!(rendered.contains(&Style::new().fg(Color::Cyan).bold().render("? ")));
        assert!(rendered.contains(&Style::new().fg(Color::Cyan).render("research mode")));
    }

    #[test]
    fn input_rules_restore_outlined_composer_and_status_chip() {
        let plain = strip_ansi(&input_rule(20, Color::BrightBlack));
        assert_eq!(visible_len(&plain), 20);
        assert!(plain.starts_with('─'));

        let status = input_status_rule(48, Color::BrightBlack, "◇ high");
        let status_plain = strip_ansi(&status);
        assert_eq!(visible_len(&status), 48);
        assert!(status_plain.starts_with('─'), "{status_plain}");
        assert!(!status_plain.contains("context"), "{status_plain}");
        assert!(status_plain.contains("◇ high"), "{status_plain}");
        assert!(status.contains(&ACCENT.fg_ansi()));
    }

    #[test]
    fn input_gradient_rule_animates_brand_ribbon_without_a_surface_fill() {
        let palette = [
            Color::Rgb(86, 156, 255),
            Color::Rgb(70, 214, 255),
            Color::Rgb(255, 101, 155),
            Color::Rgb(190, 124, 255),
        ];
        let first = input_gradient_rule(24, &palette, 0);
        let next = input_gradient_rule(24, &palette, 1);

        assert_eq!(visible_len(&first), 24);
        assert_eq!(strip_ansi(&first), "━━━━━━━━━━━━━━━━━━━━━━━━");
        assert_ne!(first, next);
        assert!(palette.iter().all(|color| first.contains(&color.fg_ansi())));
        assert!(!first.contains(&SURFACE_USER.bg_ansi()));
    }

    #[test]
    fn thinking_block_uses_shared_wrapped_prefix_block() {
        let rendered = thinking_block("alpha beta gamma delta", 16);
        let plain = strip_ansi(&rendered);
        let rows = plain.lines().collect::<Vec<_>>();

        assert!(rows[0].starts_with("• alpha"));
        assert!(rows.iter().skip(1).all(|row| row.starts_with("  ")));
        assert!(rendered.lines().all(|line| visible_len(line) == 16));
        assert!(rendered.contains(&format!("\x1b[3;{}m• alpha", TN_GRAY.fg_ansi())));
    }

    #[test]
    fn thinking_block_keeps_empty_input_empty() {
        assert_eq!(thinking_block("   ", 40), "");
        assert_eq!(thinking_block("thinking", 0), "");
    }

    #[test]
    fn thinking_block_wraps_wide_unicode_by_display_width() {
        let rendered = thinking_block("中文测试内容", 13);
        let plain = strip_ansi(&rendered);
        let rows = plain.lines().collect::<Vec<_>>();

        assert!(rows[0].starts_with("• 中文测试内"));
        assert!(rows[1].starts_with("  容"));
        assert!(rendered.lines().all(|line| visible_len(line) == 13));
    }

    #[test]
    fn compact_progress_line_uses_shared_progress_bar() {
        let rendered = compact_progress_line(Duration::from_secs(15), 80);
        let plain = strip_ansi(&rendered);

        assert!(plain.starts_with("✦ Compacting context"), "{plain}");
        assert!(plain.contains("Compacting context"), "{plain}");
        assert!(plain.contains("15s / 1m 00s"), "{plain}");
        assert!(!plain.contains('%'), "{plain}");
        assert!(plain.contains('▰'), "{plain}");
        assert!(plain.contains('▱'), "{plain}");
        assert!(rendered.contains(&format!("\x1b[{}m", ACCENT.fg_ansi())));
        assert!(visible_len(&rendered) <= 80);
    }

    #[test]
    fn compact_progress_line_animates_without_claiming_completion_percentage() {
        let first = strip_ansi(&compact_progress_line(Duration::from_millis(600), 80));
        let second = strip_ansi(&compact_progress_line(Duration::from_millis(1_800), 80));

        assert_ne!(first, second);
        assert!(!first.contains('%'));
        assert!(!second.contains('%'));
    }

    #[test]
    fn compact_progress_line_stays_bounded_on_narrow_widths() {
        let rendered = compact_progress_line(Duration::from_secs(8), 18);

        assert_eq!(visible_len(&rendered), 18);
    }

    #[test]
    fn shimmer_uses_shared_component_settings() {
        let rendered = shimmer("Working…", 0);
        let expected = a3s_tui::components::ShimmerText::new("Working…")
            .phase(0)
            .colors(TN_GRAY, TN_FG)
            .spread(5.0)
            .speed_divisor(3)
            .cycle_gap(12)
            .view();

        assert_eq!(rendered, expected);
        assert_eq!(strip_ansi(&rendered), "Working…");
        assert!(rendered.contains(&Style::new().fg(TN_FG).bold().render("W")));
        assert!(!rendered.contains("38;2;125;182;255"));
    }

    #[test]
    fn shimmer_preserves_complex_glyph_display_width() {
        let rendered = shimmer("工作e\u{301}", 0);

        assert_eq!(strip_ansi(&rendered), "工作e\u{301}");
        assert_eq!(visible_len(&rendered), 5);
    }
}
