//! Searchable prompt history for `/history` and Ctrl+R.

use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::{MouseEvent, MouseEventKind};

const HISTORY_MAX_RESULTS: usize = 100;
const HISTORY_MAX_VISIBLE_ROWS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HistoryMatch {
    history_index: usize,
    score: i64,
}

pub(crate) struct HistoryPanel {
    query: String,
    matches: Vec<HistoryMatch>,
    selected_history_index: Option<usize>,
    selected_hint: usize,
}

impl HistoryPanel {
    fn new(history: &[String], initial_query: &str) -> Self {
        let query = initial_query.trim().to_string();
        let matches = history_matches(history, &query);
        let selected_history_index = matches.first().map(|found| found.history_index);
        Self {
            query,
            matches,
            selected_history_index,
            selected_hint: 0,
        }
    }

    fn selected_index(&self) -> usize {
        if self.matches.is_empty() {
            return 0;
        }
        self.selected_history_index
            .and_then(|selected| {
                self.matches
                    .iter()
                    .position(|found| found.history_index == selected)
            })
            .unwrap_or_else(|| self.selected_hint.min(self.matches.len() - 1))
    }

    fn select_index(&mut self, index: usize) {
        if self.matches.is_empty() {
            self.selected_history_index = None;
            self.selected_hint = 0;
            return;
        }
        let index = index.min(self.matches.len() - 1);
        self.selected_history_index = Some(self.matches[index].history_index);
        self.selected_hint = index;
    }

    fn select_first(&mut self, history: &[String]) {
        self.matches = history_matches(history, &self.query);
        self.select_index(0);
    }

    fn insert_query(&mut self, text: &str, history: &[String]) {
        for character in text.chars() {
            if character.is_control() {
                if character.is_whitespace() {
                    self.query.push(' ');
                }
            } else {
                self.query.push(character);
            }
        }
        self.select_first(history);
    }

    fn move_selection(&mut self, amount: isize) {
        if self.matches.is_empty() {
            return;
        }
        let current = self.selected_index();
        let next = if amount.is_negative() {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            current
                .saturating_add(amount as usize)
                .min(self.matches.len().saturating_sub(1))
        };
        self.select_index(next);
    }

    fn move_selection_to(&mut self, index: usize) {
        self.select_index(index);
    }

    fn cycle_next(&mut self) {
        if self.matches.is_empty() {
            return;
        }
        let next = (self.selected_index() + 1) % self.matches.len();
        self.select_index(next);
    }

    fn selected_prompt<'a>(&self, history: &'a [String]) -> Option<&'a str> {
        let found = self.matches.get(self.selected_index())?;
        history.get(found.history_index).map(String::as_str)
    }

    fn handle_key(&mut self, key: &KeyEvent, history: &[String]) -> HistoryPanelAction {
        match key.code {
            KeyCode::Esc => return HistoryPanelAction::Close,
            KeyCode::Enter | KeyCode::Tab => {
                return self
                    .selected_prompt(history)
                    .map_or(HistoryPanelAction::None, |prompt| {
                        HistoryPanelAction::Use(prompt.to_string())
                    });
            }
            KeyCode::Up | KeyCode::BackTab => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => {
                self.move_selection(-(HISTORY_MAX_VISIBLE_ROWS as isize));
            }
            KeyCode::PageDown => {
                self.move_selection(HISTORY_MAX_VISIBLE_ROWS as isize);
            }
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Backspace => {
                self.query.pop();
                self.select_first(history);
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.clear();
                self.select_first(history);
            }
            KeyCode::Char('r') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.cycle_next();
            }
            KeyCode::Char(character)
                if !character.is_control()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.query.push(character);
                self.select_first(history);
            }
            _ => {}
        }
        HistoryPanelAction::None
    }
}

enum HistoryPanelAction {
    None,
    Use(String),
    Close,
}

pub(crate) fn is_history_panel_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('r') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn history_matches(history: &[String], query: &str) -> Vec<HistoryMatch> {
    let query = query.trim();
    let mut matches = history
        .iter()
        .enumerate()
        .filter_map(|(history_index, prompt)| {
            if prompt.trim().is_empty() {
                return None;
            }
            let score = if query.is_empty() {
                0
            } else {
                history_match_score(prompt, query)?
            };
            Some(HistoryMatch {
                history_index,
                score,
            })
        })
        .collect::<Vec<_>>();
    matches.sort_by(|left, right| {
        right
            .score
            .cmp(&left.score)
            .then_with(|| right.history_index.cmp(&left.history_index))
    });
    matches.truncate(HISTORY_MAX_RESULTS);
    matches
}

fn history_match_score(text: &str, query: &str) -> Option<i64> {
    let text = text.to_lowercase();
    let query = query.trim().to_lowercase();
    if query.is_empty() {
        return Some(0);
    }

    let text_chars = text.chars().collect::<Vec<_>>();
    let mut score = 0i64;
    for token in query.split_whitespace() {
        let token_chars = token.chars().collect::<Vec<_>>();
        score = score.saturating_add(fuzzy_token_score(&text_chars, &token_chars)?);
    }
    if let Some(byte_index) = text.find(&query) {
        score = score
            .saturating_add(10_000)
            .saturating_sub(i64::try_from(byte_index.min(1_000)).unwrap_or(1_000));
    }
    score = score.saturating_sub(
        i64::try_from(text_chars.len().min(2_000))
            .unwrap_or(2_000)
            .saturating_div(8),
    );
    Some(score)
}

fn fuzzy_token_score(text: &[char], token: &[char]) -> Option<i64> {
    if token.is_empty() {
        return Some(0);
    }
    let mut score = 0i64;
    let mut from = 0usize;
    let mut first = None;
    let mut previous = None;
    for needle in token {
        let relative = text
            .get(from..)?
            .iter()
            .position(|candidate| candidate == needle)?;
        let index = from + relative;
        first.get_or_insert(index);
        score = score.saturating_add(20);
        if previous.is_some_and(|prior| prior + 1 == index) {
            score = score.saturating_add(18);
        }
        if index == 0
            || text
                .get(index - 1)
                .is_some_and(|prior| !prior.is_alphanumeric())
        {
            score = score.saturating_add(14);
        }
        if let Some(prior) = previous {
            score = score.saturating_sub(
                i64::try_from(index.saturating_sub(prior + 1).min(32)).unwrap_or(32),
            );
        }
        previous = Some(index);
        from = index + 1;
    }
    let span = previous
        .unwrap_or_default()
        .saturating_sub(first.unwrap_or_default());
    score = score.saturating_sub(i64::try_from(span.min(256)).unwrap_or(256));
    Some(score)
}

fn history_prompt_label(prompt: &str) -> String {
    prompt.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn history_age_label(history_len: usize, history_index: usize) -> String {
    let prompts_ago = history_len.saturating_sub(history_index.saturating_add(1));
    match prompts_ago {
        0 => "latest".to_string(),
        1 => "1 prompt ago".to_string(),
        count => format!("{count} prompts ago"),
    }
}

fn history_menu_panel(panel: &HistoryPanel, history: &[String], max_items: usize) -> MenuPanel {
    let items = if panel.matches.is_empty() {
        vec![MenuItem::new(if history.is_empty() {
            "(no prompts in this session)"
        } else {
            "(no prompts match this search)"
        })
        .prefix("·")
        .color(TN_GRAY)
        .disabled(true)]
    } else {
        panel
            .matches
            .iter()
            .filter_map(|found| {
                history.get(found.history_index).map(|prompt| {
                    MenuItem::new(history_prompt_label(prompt))
                        .prefix("↶")
                        .description(history_age_label(history.len(), found.history_index))
                        .color(TN_CYAN)
                })
            })
            .collect()
    };
    let subtitle = if panel.query.is_empty() {
        "Type to fuzzy-search this session's prompts".to_string()
    } else {
        format!("Find: {}▌", history_prompt_label(&panel.query))
    };

    MenuPanel::new(format!(
        "Prompt history · {}/{}",
        panel.matches.len(),
        history.len()
    ))
    .subtitle(subtitle)
    .items(items)
    .selected(panel.selected_index())
    .max_items(max_items)
    .footer("↑/↓ select · Enter/Tab use · Ctrl+R next · Ctrl+U clear · Esc close")
    .indent(2)
    .marker("›")
    .title_color(ACCENT)
    .subtitle_color(TN_GRAY)
    .text_color(TN_FG)
    .muted_color(TN_GRAY)
    .selected_colors(TN_FG, SURFACE_SELECTED)
    .disabled_color(TN_GRAY)
}

fn history_panel_max_rows(height: usize) -> usize {
    height.saturating_sub(9).clamp(3, HISTORY_MAX_VISIBLE_ROWS)
}

fn history_panel_height(match_count: usize, max_items: usize) -> usize {
    let visible = match_count.max(1).min(max_items);
    // Title + subtitle + rows + optional scroll status + footer.
    4 + visible
}

fn history_menu_lines(
    panel: &HistoryPanel,
    history: &[String],
    width: usize,
    max_items: usize,
) -> Vec<String> {
    let count = panel.matches.len();
    history_menu_panel(panel, history, max_items)
        .view(
            width.min(u16::MAX as usize) as u16,
            history_panel_height(count, max_items),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

fn history_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    pub(crate) fn open_history_panel(&mut self, initial_query: &str) {
        if self.history.is_empty() {
            self.push_notice(NoticeKind::Info, "No prompts in this session");
            return;
        }
        self.history_panel = Some(HistoryPanel::new(&self.history, initial_query));
        self.relayout();
    }

    pub(crate) fn handle_history_panel_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let action = self.history_panel.as_mut()?.handle_key(key, &self.history);
        match action {
            HistoryPanelAction::None => None,
            HistoryPanelAction::Use(prompt) => {
                self.history_panel = None;
                self.history_pos = None;
                self.history_draft = None;
                // The accepted entry owns its own literal prefix. Do not let
                // the mode that happened to be active before Ctrl+R reinterpret
                // a normal historical prompt as shell or research input.
                self.shell_mode = false;
                self.research_mode = false;
                self.textarea.set_value(&prompt);
                self.relayout();
                None
            }
            HistoryPanelAction::Close => {
                self.history_panel = None;
                self.relayout();
                None
            }
        }
    }

    pub(crate) fn handle_history_panel_paste(&mut self, text: &str) {
        if let Some(panel) = self.history_panel.as_mut() {
            panel.insert_query(text, &self.history);
        }
    }

    pub(crate) fn handle_history_panel_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.history_panel.as_mut()?.move_selection(-1);
                return None;
            }
            MouseEventKind::ScrollDown => {
                self.history_panel.as_mut()?.move_selection(1);
                return None;
            }
            _ => {}
        }

        let panel = self.history_panel.as_ref()?;
        let max_items = history_panel_max_rows(self.height as usize);
        let height = history_panel_height(panel.matches.len(), max_items);
        let mut menu = history_menu_panel(panel, &self.history, max_items);
        let row_count = menu.view(self.width, height).lines().count();
        if row_count == 0 {
            return None;
        }
        menu.set_y_offset(history_overlay_y_offset(
            self.height as usize,
            row_count,
            self.overlay_rows_below(),
        ));
        match menu.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                if let Some(panel) = self.history_panel.as_mut() {
                    panel.select_index(index);
                }
                None
            }
            Some(MenuPanelMsg::Cancelled) | None => None,
        }
    }

    pub(crate) fn overlay_history_menu(&self, composed: String) -> String {
        let Some(panel) = self.history_panel.as_ref() else {
            return composed;
        };
        let lines = history_menu_lines(
            panel,
            &self.history,
            self.width as usize,
            history_panel_max_rows(self.height as usize),
        );
        self.overlay_list(composed, &lines)
    }
}

#[cfg(test)]
#[path = "history_tests.rs"]
mod tests;
