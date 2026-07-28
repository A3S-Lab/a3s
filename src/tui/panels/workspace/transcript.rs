//! Full-width semantic transcript viewport used by the Codex-style `Ctrl+T` view.
//!
//! The caller owns semantic transcript composition. This component keeps the
//! already-rendered ANSI content styled, scrollable, searchable, and bounded to
//! the current terminal dimensions, with only the chrome needed to understand
//! and navigate the view.

use a3s_tui::components::{Viewport, ViewportMsg};
use a3s_tui::event::KeyEvent;
use a3s_tui::event::{MouseEvent, MouseEventKind};
use a3s_tui::layout::{Constraint, Layout};
use a3s_tui::style::{fit_visible, strip_ansi, visible_len, Style};
use a3s_tui::{KeyCode, KeyModifiers};

use super::super::{highlight_selection, COMPOSER_CHROME};

const HEADER_ROWS: u16 = 1;
const FOOTER_ROWS: u16 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TranscriptViewportAction {
    Ignored,
    Handled,
    CloseRequested,
}

pub(crate) struct SemanticTranscriptViewport {
    viewport: Viewport,
    content: String,
    search_rows: Vec<String>,
    width: u16,
    height: u16,
    search: TranscriptSearch,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct TranscriptSearch {
    query: String,
    editing: bool,
    matches: Vec<TranscriptMatch>,
    selected: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct TranscriptMatch {
    row: usize,
    start_col: usize,
    end_col: usize,
}

impl SemanticTranscriptViewport {
    pub(crate) fn new(content: &str, width: u16, height: u16) -> Self {
        let mut viewport = Viewport::new(width, transcript_body_height(height));
        viewport.set_content(content);
        Self {
            viewport,
            content: content.to_string(),
            search_rows: wrapped_plain_rows(content, width),
            width,
            height,
            search: TranscriptSearch::default(),
        }
    }

    /// Replace the semantic projection. A view at the bottom follows the latest
    /// content; a view that the user scrolled keeps its bounded row offset.
    pub(crate) fn set_content(&mut self, content: &str) {
        self.content.clear();
        self.content.push_str(content);
        self.viewport.set_content(content);
        self.search_rows = wrapped_plain_rows(content, self.width);
        self.refresh_search_matches(false);
    }

    pub(crate) fn resize(&mut self, width: u16, height: u16) {
        let width_changed = self.width != width;
        self.width = width;
        self.height = height;
        self.viewport.resize(width, transcript_body_height(height));
        if width_changed {
            self.search_rows = wrapped_plain_rows(&self.content, width);
            self.refresh_search_matches(false);
        } else {
            self.reveal_selected_match();
        }
    }

    pub(crate) fn render(&self) -> String {
        let mut body = self.viewport.view();
        if let Some(found) = self.selected_match() {
            let offset = self.viewport.scroll_offset();
            let body_rows = usize::from(transcript_body_height(self.height));
            if found.row >= offset && found.row < offset.saturating_add(body_rows) {
                body = highlight_selection(
                    &body,
                    found.row - offset,
                    found.start_col,
                    found.row - offset,
                    found.end_col,
                );
            }
        }

        let header = transcript_header_line(
            self.viewport.scroll_offset(),
            self.viewport.total_lines(),
            usize::from(transcript_body_height(self.height)),
            self.width as usize,
        );
        let footer = transcript_footer_line(&self.search, self.width as usize);
        let mut layout = Layout::vertical();
        if transcript_has_header(self.height) {
            layout = layout.item(&header, Constraint::Fixed(HEADER_ROWS));
        }
        layout = layout.item(&body, Constraint::Fill);
        if transcript_has_footer(self.height) {
            layout = layout.item(&footer, Constraint::Fixed(FOOTER_ROWS));
        }
        layout.render(self.height)
    }

    pub(crate) fn handle_key(&mut self, key: &KeyEvent) -> TranscriptViewportAction {
        if self.search.editing {
            return self.handle_search_key(key);
        }

        let message = match key.code {
            KeyCode::Up | KeyCode::Char('k') if key.modifiers == KeyModifiers::NONE => {
                Some(ViewportMsg::ScrollUp(1))
            }
            KeyCode::Down | KeyCode::Char('j') if key.modifiers == KeyModifiers::NONE => {
                Some(ViewportMsg::ScrollDown(1))
            }
            KeyCode::PageUp => Some(ViewportMsg::PageUp),
            KeyCode::PageDown => Some(ViewportMsg::PageDown),
            KeyCode::Home => Some(ViewportMsg::Top),
            KeyCode::End => Some(ViewportMsg::Bottom),
            KeyCode::Char('/') if key.modifiers == KeyModifiers::NONE => {
                self.begin_search();
                return TranscriptViewportAction::Handled;
            }
            KeyCode::Char('n') if key.modifiers == KeyModifiers::NONE => {
                self.select_relative_match(true);
                return TranscriptViewportAction::Handled;
            }
            KeyCode::Char('N')
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.select_relative_match(false);
                return TranscriptViewportAction::Handled;
            }
            KeyCode::Esc => return TranscriptViewportAction::CloseRequested,
            KeyCode::Char('q') if key.modifiers == KeyModifiers::NONE => {
                return TranscriptViewportAction::CloseRequested;
            }
            KeyCode::Char(c)
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && c.eq_ignore_ascii_case(&'t') =>
            {
                return TranscriptViewportAction::CloseRequested;
            }
            _ => None,
        };

        let Some(message) = message else {
            return TranscriptViewportAction::Ignored;
        };
        self.viewport.update(message);
        self.viewport.set_auto_scroll(self.viewport.at_bottom());
        TranscriptViewportAction::Handled
    }

    fn begin_search(&mut self) {
        self.search = TranscriptSearch {
            editing: true,
            ..TranscriptSearch::default()
        };
        self.viewport.set_auto_scroll(false);
    }

    fn handle_search_key(&mut self, key: &KeyEvent) -> TranscriptViewportAction {
        match key.code {
            KeyCode::Esc => {
                self.search = TranscriptSearch::default();
                self.viewport.set_auto_scroll(self.viewport.at_bottom());
            }
            KeyCode::Enter => {
                self.search.editing = false;
                self.refresh_search_matches(false);
            }
            KeyCode::Backspace => {
                self.search.query.pop();
                self.refresh_search_matches(true);
            }
            KeyCode::Char(c)
                if key.modifiers == KeyModifiers::NONE || key.modifiers == KeyModifiers::SHIFT =>
            {
                self.search.query.push(c);
                self.refresh_search_matches(true);
            }
            _ => {}
        }
        TranscriptViewportAction::Handled
    }

    fn refresh_search_matches(&mut self, select_near_view: bool) {
        let previous = self.search.selected;
        self.search.matches = transcript_matches(&self.search_rows, &self.search.query);
        self.search.selected = if self.search.matches.is_empty() {
            None
        } else if select_near_view {
            Some(
                self.search
                    .matches
                    .iter()
                    .position(|found| found.row >= self.viewport.scroll_offset())
                    .unwrap_or(0),
            )
        } else {
            Some(previous.unwrap_or(0).min(self.search.matches.len() - 1))
        };
        self.reveal_selected_match();
    }

    fn select_relative_match(&mut self, forward: bool) {
        if self.search.matches.is_empty() {
            return;
        }
        let count = self.search.matches.len();
        self.search.selected = Some(match (self.search.selected, forward) {
            (Some(index), true) => (index + 1) % count,
            (Some(0), false) | (None, false) => count - 1,
            (Some(index), false) => index - 1,
            (None, true) => 0,
        });
        self.reveal_selected_match();
    }

    fn reveal_selected_match(&mut self) {
        let Some(found) = self.selected_match() else {
            return;
        };
        let body_rows = usize::from(transcript_body_height(self.height)).max(1);
        let offset = self.viewport.scroll_offset();
        if found.row < offset || found.row >= offset.saturating_add(body_rows) {
            self.viewport
                .set_scroll_offset(found.row.saturating_sub(body_rows / 3));
        }
        self.viewport.set_auto_scroll(false);
    }

    fn selected_match(&self) -> Option<TranscriptMatch> {
        self.search
            .selected
            .and_then(|index| self.search.matches.get(index).copied())
    }

    pub(crate) fn handle_mouse(&mut self, mouse: &MouseEvent) -> TranscriptViewportAction {
        let message = match mouse.kind {
            MouseEventKind::ScrollUp => ViewportMsg::ScrollUp(3),
            MouseEventKind::ScrollDown => ViewportMsg::ScrollDown(3),
            _ => return TranscriptViewportAction::Ignored,
        };
        self.viewport.update(message);
        self.viewport.set_auto_scroll(self.viewport.at_bottom());
        TranscriptViewportAction::Handled
    }

    #[cfg(test)]
    pub(crate) fn scroll_offset(&self) -> usize {
        self.viewport.scroll_offset()
    }

    #[cfg(test)]
    pub(crate) fn total_lines(&self) -> usize {
        self.viewport.total_lines()
    }

    #[cfg(test)]
    pub(crate) fn at_bottom(&self) -> bool {
        self.viewport.at_bottom()
    }

    #[cfg(test)]
    fn search_state(&self) -> (&str, bool, usize, Option<usize>) {
        (
            &self.search.query,
            self.search.editing,
            self.search.matches.len(),
            self.search.selected,
        )
    }
}

fn transcript_has_header(height: u16) -> bool {
    height >= 2
}

fn transcript_has_footer(height: u16) -> bool {
    height >= 3
}

fn transcript_body_height(height: u16) -> u16 {
    let chrome = u16::from(transcript_has_header(height))
        .saturating_add(u16::from(transcript_has_footer(height)));
    height.saturating_sub(chrome)
}

fn transcript_header_line(offset: usize, total: usize, body_rows: usize, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let title = Style::new()
        .fg(COMPOSER_CHROME.primary)
        .bold()
        .render("Transcript");
    let position = if total == 0 {
        "empty".to_string()
    } else {
        let first = offset.min(total.saturating_sub(1)) + 1;
        let last = offset.saturating_add(body_rows).min(total).max(first);
        if first == 1 && last == total {
            format!("{total} lines")
        } else {
            format!("{first}–{last} / {total}")
        }
    };
    let position = Style::new().fg(COMPOSER_CHROME.secondary).render(&position);
    let used = visible_len(&title).saturating_add(visible_len(&position));
    if used.saturating_add(2) > width {
        return fit_visible(&title, width);
    }
    let rule = Style::new()
        .fg(COMPOSER_CHROME.faint)
        .render(&"─".repeat(width - used - 2));
    fit_visible(&format!("{title} {rule} {position}"), width)
}

fn transcript_footer_line(search: &TranscriptSearch, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    if search.editing {
        let query = sanitized_search_query(&search.query);
        let prompt = Style::new()
            .fg(COMPOSER_CHROME.active)
            .bold()
            .render("Find /");
        let query = Style::new().fg(COMPOSER_CHROME.primary).render(&query);
        let cursor = Style::new().fg(COMPOSER_CHROME.active).render("▌");
        let count = search_result_count(search);
        return fit_visible(&format!("{prompt}{query}{cursor}  {count}"), width);
    }

    if !search.query.is_empty() {
        let prompt = Style::new().fg(COMPOSER_CHROME.active).bold().render("/");
        let query = Style::new()
            .fg(COMPOSER_CHROME.primary)
            .render(&sanitized_search_query(&search.query));
        let count = search_result_count(search);
        let hint = Style::new()
            .fg(COMPOSER_CHROME.faint)
            .render("  n/N match  / new");
        return fit_visible(&format!("{prompt}{query}  {count}{hint}"), width);
    }

    let keys = Style::new()
        .fg(COMPOSER_CHROME.secondary)
        .render("↑↓  PgUp/PgDn  /  Esc");
    let labels = Style::new()
        .fg(COMPOSER_CHROME.faint)
        .render("  scroll · find · close");
    fit_visible(&format!("{keys}{labels}"), width)
}

fn search_result_count(search: &TranscriptSearch) -> String {
    if search.matches.is_empty() {
        return Style::new().fg(COMPOSER_CHROME.error).render("no matches");
    }
    let current = search.selected.unwrap_or(0).min(search.matches.len() - 1) + 1;
    Style::new()
        .fg(COMPOSER_CHROME.secondary)
        .render(&format!("{current}/{}", search.matches.len()))
}

fn sanitized_search_query(query: &str) -> String {
    query.chars().filter(|ch| !ch.is_control()).collect()
}

/// Build the exact display rows that `Viewport` searches and navigates.
///
/// Using a one-row scratch viewport keeps this projection aligned with the
/// canonical ANSI-aware wrapper without reaching into component internals or
/// maintaining a second wrapping algorithm that can drift from the UI.
fn wrapped_plain_rows(content: &str, width: u16) -> Vec<String> {
    let mut wrapped = Viewport::new(width, 1).with_auto_scroll(false);
    wrapped.set_content(content);
    let total = wrapped.total_lines();
    (0..total)
        .map(|row| {
            wrapped.set_scroll_offset(row);
            strip_ansi(wrapped.view().split('\n').next().unwrap_or_default())
        })
        .collect()
}

fn transcript_matches(rows: &[String], query: &str) -> Vec<TranscriptMatch> {
    let query = sanitized_search_query(query);
    if query.trim().is_empty() {
        return Vec::new();
    }
    let needle = query.to_ascii_lowercase();
    let mut matches = Vec::new();
    for (row, line) in rows.iter().enumerate() {
        let haystack = line.to_ascii_lowercase();
        let mut from = 0usize;
        while let Some(relative) = haystack[from..].find(&needle) {
            let start = from + relative;
            let end = start + needle.len();
            matches.push(TranscriptMatch {
                row,
                start_col: visible_len(&line[..start]),
                end_col: visible_len(&line[..end]),
            });
            from = end.max(start + 1);
        }
    }
    matches
}

#[cfg(test)]
mod tests {
    use super::super::super::SURFACE_SELECTED;
    use super::*;
    use a3s_tui::style::{strip_ansi, visible_len, Color, Style};

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    #[test]
    fn render_preserves_message_styles_inside_minimal_transcript_chrome() {
        let user = Style::new()
            .fg(Color::BrightWhite)
            .bg(Color::Rgb(31, 31, 31))
            .render("user request");
        let tool = Style::new().fg(Color::Green).render("• Ran cargo test");
        let content = format!("{user}\n{tool}");
        let view = SemanticTranscriptViewport::new(&content, 48, 4).render();
        let plain = strip_ansi(&view);

        assert!(view.contains("\x1b[97;48;2;31;31;31muser request\x1b[0m"));
        assert!(view.contains("\x1b[32m• Ran cargo test\x1b[0m"));
        assert!(plain.starts_with("Transcript "), "{plain:?}");
        assert!(plain.contains("2 lines"), "{plain:?}");
        assert!(
            plain.contains("user request\n• Ran cargo test"),
            "{plain:?}"
        );
        assert!(plain.contains("/  Esc"), "{plain:?}");
        assert!(!plain.contains("transcript.txt"));
        assert!(!plain.contains("metadata"));
    }

    #[test]
    fn key_handling_scrolls_pages_and_bounds_home_end() {
        let content = (0..20)
            .map(|index| format!("row {index:02}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = SemanticTranscriptViewport::new(&content, 20, 4);

        assert_eq!(view.scroll_offset(), 18);
        assert!(view.at_bottom());
        assert_eq!(
            view.handle_key(&key(KeyCode::Home)),
            TranscriptViewportAction::Handled
        );
        assert_eq!(view.scroll_offset(), 0);
        assert!(!view.at_bottom());
        assert_eq!(
            view.handle_key(&key(KeyCode::PageDown)),
            TranscriptViewportAction::Handled
        );
        assert_eq!(view.scroll_offset(), 2);
        assert_eq!(
            view.handle_key(&key(KeyCode::End)),
            TranscriptViewportAction::Handled
        );
        assert_eq!(view.scroll_offset(), 18);
        assert!(view.at_bottom());

        assert_eq!(
            view.handle_key(&key(KeyCode::Esc)),
            TranscriptViewportAction::CloseRequested
        );
        assert_eq!(
            view.handle_key(&key(KeyCode::Char('x'))),
            TranscriptViewportAction::Ignored
        );
    }

    #[test]
    fn slash_search_finds_highlights_and_cycles_distant_message_rows() {
        let content = (0..30)
            .map(|index| match index {
                2 | 15 | 28 => format!("result anchor {index}"),
                _ => format!("ordinary row {index}"),
            })
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = SemanticTranscriptViewport::new(&content, 40, 6);

        assert_eq!(
            view.handle_key(&key(KeyCode::Char('/'))),
            TranscriptViewportAction::Handled
        );
        for ch in "result".chars() {
            view.handle_key(&key(KeyCode::Char(ch)));
        }

        assert_eq!(view.search_state(), ("result", true, 3, Some(2)));
        let searching = view.render();
        assert!(strip_ansi(&searching).contains("Find /result▌  3/3"));
        assert!(searching.contains(SURFACE_SELECTED.bg_ansi().as_str()));
        assert!(strip_ansi(&searching).contains("result anchor 28"));

        view.handle_key(&key(KeyCode::Enter));
        assert_eq!(view.search_state(), ("result", false, 3, Some(2)));
        view.handle_key(&key(KeyCode::Char('n')));
        assert_eq!(view.search_state(), ("result", false, 3, Some(0)));
        assert!(view.scroll_offset() <= 2);
        assert!(strip_ansi(&view.render()).contains("result anchor 2"));

        view.handle_key(&KeyEvent {
            code: KeyCode::Char('N'),
            modifiers: KeyModifiers::SHIFT,
        });
        assert_eq!(view.search_state(), ("result", false, 3, Some(2)));
        assert!(strip_ansi(&view.render()).contains("result anchor 28"));
        assert_eq!(
            view.handle_key(&key(KeyCode::Esc)),
            TranscriptViewportAction::CloseRequested
        );
    }

    #[test]
    fn escape_cancels_an_open_search_before_it_can_close_the_view() {
        let mut view = SemanticTranscriptViewport::new("alpha\nbeta", 32, 5);
        view.handle_key(&key(KeyCode::Char('/')));
        view.handle_key(&key(KeyCode::Char('a')));

        assert_eq!(
            view.handle_key(&key(KeyCode::Esc)),
            TranscriptViewportAction::Handled
        );
        assert_eq!(view.search_state(), ("", false, 0, None));
        assert_eq!(
            view.handle_key(&key(KeyCode::Esc)),
            TranscriptViewportAction::CloseRequested
        );
    }

    #[test]
    fn search_survives_live_content_refresh_without_losing_its_selection() {
        let mut view = SemanticTranscriptViewport::new("alpha\none\nalpha", 32, 5);
        view.handle_key(&key(KeyCode::Char('/')));
        for ch in "alpha".chars() {
            view.handle_key(&key(KeyCode::Char(ch)));
        }
        view.handle_key(&key(KeyCode::Enter));
        assert_eq!(view.search_state(), ("alpha", false, 2, Some(0)));

        view.set_content("alpha\none\nalpha\ntwo\nalpha");

        assert_eq!(view.search_state(), ("alpha", false, 3, Some(0)));
        assert!(strip_ansi(&view.render()).contains("/alpha  1/3"));
    }

    #[test]
    fn matching_is_ascii_case_insensitive_and_unicode_column_safe() {
        let rows = wrapped_plain_rows("Result alpha\n结果 Alpha", 80);
        let found = transcript_matches(&rows, "ALPHA");
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].row, 0);
        assert_eq!(found[1].row, 1);
        assert_eq!(found[1].start_col, 5);
        assert_eq!(found[1].end_col, 10);

        let rows = wrapped_plain_rows("tool 结果 ready", 80);
        let chinese = transcript_matches(&rows, "结果");
        assert_eq!(chinese.len(), 1);
        assert_eq!(chinese[0].start_col, 5);
        assert_eq!(chinese[0].end_col, 9);
    }

    #[test]
    fn search_coordinates_follow_wrapped_rows_and_reflow_after_resize() {
        let content = "0123456789target\nend";
        let mut view = SemanticTranscriptViewport::new(content, 10, 6);
        view.handle_key(&key(KeyCode::Char('/')));
        for ch in "target".chars() {
            view.handle_key(&key(KeyCode::Char(ch)));
        }

        assert_eq!(
            view.selected_match(),
            Some(TranscriptMatch {
                row: 1,
                start_col: 0,
                end_col: 6,
            })
        );
        let narrow = view.render();
        assert!(strip_ansi(&narrow).contains("target"), "{narrow:?}");
        assert!(narrow.contains(SURFACE_SELECTED.bg_ansi().as_str()));

        view.resize(20, 6);

        assert_eq!(
            view.selected_match(),
            Some(TranscriptMatch {
                row: 0,
                start_col: 10,
                end_col: 16,
            })
        );
        let wide = view.render();
        assert!(strip_ansi(&wide).contains("0123456789target"), "{wide:?}");
        assert!(wide.contains(SURFACE_SELECTED.bg_ansi().as_str()));
    }

    #[test]
    fn resize_rewraps_to_full_width_and_clamps_scroll_bounds() {
        let styled = Style::new()
            .fg(Color::Cyan)
            .render("abcdefghij klmnopqrst uvwxyz");
        let content = format!("{styled}\nsecond semantic row\nthird semantic row");
        let mut view = SemanticTranscriptViewport::new(&content, 14, 4);
        view.handle_key(&key(KeyCode::Home));

        view.resize(7, 5);
        let rendered = view.render();
        let rows = rendered.lines().collect::<Vec<_>>();

        assert_eq!(rows.len(), 5);
        assert!(rows.iter().all(|row| visible_len(row) <= 7));
        assert!(rendered.contains("\x1b[36m"));
        assert_eq!(view.scroll_offset(), 0);
        assert!(view.total_lines() >= 3);

        view.handle_key(&key(KeyCode::End));
        assert!(view.at_bottom());
        assert_eq!(
            view.scroll_offset(),
            view.total_lines()
                .saturating_sub(usize::from(transcript_body_height(5)))
        );

        view.resize(40, 10);
        assert_eq!(view.scroll_offset(), 0);
        assert!(view.at_bottom());
        assert!(view.render().lines().all(|row| visible_len(row) <= 40));
    }

    #[test]
    fn content_refresh_follows_bottom_but_preserves_scrolled_offset() {
        let content = (0..8)
            .map(|index| format!("row {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = SemanticTranscriptViewport::new(&content, 20, 5);

        view.set_content(&format!("{content}\nrow 8"));
        assert!(view.at_bottom());
        assert_eq!(view.scroll_offset(), 6);

        view.handle_key(&key(KeyCode::PageUp));
        let anchored = view.scroll_offset();
        view.set_content(&format!("{content}\nrow 8\nrow 9"));
        assert_eq!(view.scroll_offset(), anchored);
        assert!(!view.at_bottom());
    }

    #[test]
    fn mouse_wheel_scrolls_without_leaking_to_the_hidden_chat() {
        use a3s_tui::event::MouseEvent;

        let content = (0..12)
            .map(|index| format!("row {index}"))
            .collect::<Vec<_>>()
            .join("\n");
        let mut view = SemanticTranscriptViewport::new(&content, 20, 5);
        let bottom = view.scroll_offset();

        let action = view.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollUp,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::NONE,
        });

        assert_eq!(action, TranscriptViewportAction::Handled);
        assert_eq!(view.scroll_offset(), bottom.saturating_sub(3));
        assert!(!view.at_bottom());
    }

    #[test]
    fn transcript_chrome_degrades_without_stealing_the_only_body_row() {
        assert_eq!(transcript_body_height(1), 1);
        assert_eq!(transcript_body_height(2), 1);
        assert_eq!(transcript_body_height(3), 1);
        assert_eq!(transcript_body_height(6), 4);

        for width in [1_u16, 8, 24, 48] {
            for height in [1_u16, 2, 3, 6] {
                let view = SemanticTranscriptViewport::new(
                    "one semantic row\nsecond semantic row",
                    width,
                    height,
                )
                .render();
                assert!(view.lines().count() <= height as usize, "{view:?}");
                assert!(
                    view.lines().all(|row| visible_len(row) <= width as usize),
                    "width={width}, height={height}: {view:?}"
                );
            }
        }
    }
}
