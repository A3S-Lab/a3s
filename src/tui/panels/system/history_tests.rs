use super::*;
use a3s_tui::style::{strip_ansi, visible_len};

fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
    }
}

fn ctrl(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::CONTROL,
    }
}

fn texts(history: &[String], matches: &[HistoryMatch]) -> Vec<String> {
    matches
        .iter()
        .map(|found| history[found.history_index].clone())
        .collect()
}

#[test]
fn empty_query_returns_recent_prompts_first_and_bounds_results() {
    let history = (0..120)
        .map(|index| format!("prompt {index}"))
        .collect::<Vec<_>>();

    let matches = history_matches(&history, "");

    assert_eq!(matches.len(), HISTORY_MAX_RESULTS);
    assert_eq!(history[matches[0].history_index], "prompt 119");
    assert_eq!(history[matches[99].history_index], "prompt 20");
}

#[test]
fn fuzzy_search_matches_non_contiguous_words_and_prefers_exact_text() {
    let history = vec![
        "inspect the relay dashboard".to_string(),
        "run cargo focused tests".to_string(),
        "cargo fmt then test".to_string(),
    ];

    let matches = history_matches(&history, "cargo test");
    let result = texts(&history, &matches);

    assert_eq!(result[0], "run cargo focused tests");
    assert!(result.contains(&"cargo fmt then test".to_string()));
    assert!(!result.contains(&"inspect the relay dashboard".to_string()));
}

#[test]
fn duplicate_prompts_keep_distinct_recent_positions() {
    let history = vec![
        "same prompt".to_string(),
        "other".to_string(),
        "same prompt".to_string(),
    ];

    let matches = history_matches(&history, "same");

    assert_eq!(matches.len(), 2);
    assert_eq!(matches[0].history_index, 2);
    assert_eq!(matches[1].history_index, 0);
}

#[test]
fn selection_tracks_the_same_history_entry_until_query_changes() {
    let history = vec![
        "first rust task".to_string(),
        "second rust task".to_string(),
        "third docs task".to_string(),
    ];
    let mut panel = HistoryPanel::new(&history, "rust");
    panel.move_selection(1);

    assert_eq!(panel.selected_prompt(&history), Some("first rust task"));

    let mut extended = history.clone();
    extended.push("new unrelated prompt".to_string());
    assert_eq!(panel.selected_prompt(&extended), Some("first rust task"));

    panel.query.push_str(" second");
    panel.select_first(&extended);
    assert_eq!(panel.selected_prompt(&extended), Some("second rust task"));
}

#[test]
fn ctrl_r_cycles_matches_and_enter_uses_the_selected_prompt() {
    let history = vec![
        "older test prompt".to_string(),
        "newer test prompt".to_string(),
    ];
    let mut panel = HistoryPanel::new(&history, "test");
    assert_eq!(panel.selected_prompt(&history), Some("newer test prompt"));

    assert!(matches!(
        panel.handle_key(&ctrl(KeyCode::Char('r')), &history),
        HistoryPanelAction::None
    ));
    assert_eq!(panel.selected_prompt(&history), Some("older test prompt"));

    match panel.handle_key(&key(KeyCode::Enter), &history) {
        HistoryPanelAction::Use(prompt) => assert_eq!(prompt, "older test prompt"),
        _ => panic!("Enter should accept the selected prompt"),
    }
}

#[test]
fn typing_filters_backspace_restores_and_ctrl_u_clears() {
    let history = vec!["alpha prompt".to_string(), "beta prompt".to_string()];
    let mut panel = HistoryPanel::new(&history, "");

    for character in "beta".chars() {
        panel.handle_key(&key(KeyCode::Char(character)), &history);
    }
    assert_eq!(panel.query, "beta");
    assert_eq!(panel.selected_prompt(&history), Some("beta prompt"));

    panel.handle_key(&key(KeyCode::Backspace), &history);
    assert_eq!(panel.query, "bet");
    panel.handle_key(&ctrl(KeyCode::Char('u')), &history);
    assert!(panel.query.is_empty());
    assert_eq!(panel.selected_prompt(&history), Some("beta prompt"));
}

#[test]
fn pasted_search_text_is_filtered_without_control_characters() {
    let history = vec!["cargo focused tests".to_string(), "update docs".to_string()];
    let mut panel = HistoryPanel::new(&history, "");

    panel.insert_query("cargo\nfocused\u{0}", &history);

    assert_eq!(panel.query, "cargo focused");
    assert_eq!(panel.selected_prompt(&history), Some("cargo focused tests"));
}

#[test]
fn escape_closes_without_selecting_a_prompt() {
    let history = vec!["draft".to_string()];
    let mut panel = HistoryPanel::new(&history, "");

    assert!(matches!(
        panel.handle_key(&key(KeyCode::Esc), &history),
        HistoryPanelAction::Close
    ));
}

#[test]
fn ctrl_r_is_the_history_shortcut() {
    assert!(is_history_panel_key(&ctrl(KeyCode::Char('r'))));
    assert!(!is_history_panel_key(&key(KeyCode::Char('r'))));
}

#[test]
fn history_panel_rows_are_bounded_to_terminal_width() {
    let history =
        vec!["an intentionally long historical prompt that must be clipped safely".to_string()];
    let panel = HistoryPanel::new(&history, "historical");

    let lines = history_menu_lines(&panel, &history, 34, 12);
    for line in &lines {
        assert!(
            visible_len(line) <= 34,
            "{:?}",
            strip_ansi(line).to_string()
        );
    }
}
