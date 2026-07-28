use super::*;
use a3s_tui::style::visible_len;

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

fn native_session(id: &str, label: &str, modified: u64) -> RelaySession {
    RelaySession {
        identity: RelaySessionIdentity::Native(id.to_string()),
        agent: RelayAgent::A3sCode,
        native_id: Some(id.to_string()),
        seed: None,
        label: label.to_string(),
        modified: UNIX_EPOCH + Duration::from_secs(modified),
        persisted_model: None,
        status: RelaySessionStatus::Saved,
        active_runs: 0,
        active_subagents: 0,
    }
}

fn transcript_session(
    agent: RelayAgent,
    path: &str,
    label: &str,
    seed: &str,
    modified: u64,
) -> RelaySession {
    RelaySession {
        identity: RelaySessionIdentity::Transcript {
            agent,
            path: PathBuf::from(path),
        },
        agent,
        native_id: None,
        seed: Some(seed.to_string()),
        label: label.to_string(),
        modified: UNIX_EPOCH + Duration::from_secs(modified),
        persisted_model: None,
        status: RelaySessionStatus::External,
        active_runs: 0,
        active_subagents: 0,
    }
}

fn loaded_panel(sessions: Vec<RelaySession>) -> RelayPanel {
    let mut panel = RelayPanel::loading(7, 11, "current".to_string());
    assert!(panel.apply_scan(11, Ok(sessions)));
    panel
}

#[test]
fn relay_tabs_include_every_supported_source() {
    assert_eq!(
        RelayAgent::ALL.map(RelayAgent::label),
        ["A3S Code", "Claude Code", "Codex", "WorkBuddy"]
    );
}

#[test]
fn relay_menu_rows_are_bounded_to_the_terminal_width() {
    let mut panel = loaded_panel(vec![transcript_session(
        RelayAgent::WorkBuddy,
        "/history/workbuddy/session.jsonl",
        "a very long WorkBuddy task label that must be clipped safely",
        "task",
        0,
    )]);
    panel.set_tab(3);
    panel.preview = true;

    for line in relay_menu_lines(&panel, 32, 12) {
        assert!(visible_len(&line) <= 32, "{line:?}");
    }
}

#[test]
fn relay_description_surfaces_current_and_background_activity() {
    let mut session = native_session("current", "task", 120);
    session.persisted_model = Some("openai/gpt-5".to_string());
    session.active_runs = 1;
    session.active_subagents = 2;
    let description =
        relay_session_description(&session, "current", UNIX_EPOCH + Duration::from_secs(3_720));

    assert_eq!(relay_session_prefix(&session, "current"), "●");
    assert!(description.contains("current"), "{description}");
    assert!(description.contains("1 unfinished run"), "{description}");
    assert!(description.contains("2 background agents"), "{description}");
    assert!(description.contains("1h"), "{description}");
}

#[test]
fn semantic_selection_survives_refresh_reordering_and_metadata_changes() {
    let first = native_session("first", "first task", 20);
    let selected = native_session("selected", "old task label", 10);
    let mut panel = loaded_panel(vec![first.clone(), selected]);
    panel.move_selection(1);
    panel.query = "task".to_string();
    assert_eq!(
        panel
            .selected_session()
            .and_then(|session| session.native_id.as_deref()),
        Some("selected")
    );

    panel.request_id = 12;
    panel.loading = true;
    let mut updated = native_session("selected", "new task label", 30);
    updated.active_subagents = 2;
    assert!(panel.apply_scan(12, Ok(vec![updated, first])));

    assert_eq!(panel.query, "task");
    assert_eq!(
        panel
            .selected_session()
            .and_then(|session| session.native_id.as_deref()),
        Some("selected")
    );
    assert_eq!(panel.selected_index(), 0);
}

#[test]
fn transcript_path_keeps_selection_stable_after_an_append() {
    let first = transcript_session(
        RelayAgent::Codex,
        "/codex/first.jsonl",
        "first task",
        "first task",
        20,
    );
    let selected = transcript_session(
        RelayAgent::Codex,
        "/codex/selected.jsonl",
        "old latest task",
        "old latest task",
        10,
    );
    let mut panel = loaded_panel(vec![first.clone(), selected]);
    panel.set_tab(2);
    panel.move_selection(1);

    panel.request_id = 12;
    let updated = transcript_session(
        RelayAgent::Codex,
        "/codex/selected.jsonl",
        "new latest task",
        "new latest task",
        30,
    );
    assert!(panel.apply_scan(12, Ok(vec![updated, first])));

    assert_eq!(panel.selected_session().unwrap().label, "new latest task");
    assert_eq!(panel.selected_index(), 0);
}

#[test]
fn stale_scan_cannot_replace_newer_panel_data() {
    let mut panel = loaded_panel(vec![native_session("kept", "kept task", 10)]);
    panel.request_id = 12;
    panel.loading = true;

    assert!(!panel.apply_scan(11, Ok(vec![native_session("stale", "stale task", 20)])));
    assert!(panel.loading);
    assert_eq!(panel.sessions[0].native_id.as_deref(), Some("kept"));
}

#[test]
fn filter_matches_all_terms_across_status_model_and_source_path() {
    let mut native = native_session("paused-id", "Refactor runtime", 20);
    native.status = RelaySessionStatus::Paused;
    native.persisted_model = Some("anthropic/opus".to_string());
    let transcript = transcript_session(
        RelayAgent::Codex,
        "/history/special/session.jsonl",
        "Investigate queue",
        "Finish the queue dashboard",
        10,
    );
    let mut panel = loaded_panel(vec![native, transcript]);

    panel.query = "PAUSED opus".to_string();
    assert_eq!(panel.active_indices().len(), 1);
    assert_eq!(
        panel.selected_session().unwrap().native_id.as_deref(),
        Some("paused-id")
    );

    panel.set_tab(2);
    panel.query = "history special queue".to_string();
    assert_eq!(panel.active_indices().len(), 1);
    assert_eq!(panel.selected_session().unwrap().label, "Investigate queue");
}

#[test]
fn clearing_filter_restores_the_semantically_selected_session() {
    let mut panel = loaded_panel(vec![
        native_session("first", "alpha task", 20),
        native_session("second", "beta task", 10),
    ]);
    panel.move_selection(1);
    assert_eq!(
        panel.selected_session().unwrap().native_id.as_deref(),
        Some("second")
    );

    panel.query = "alpha".to_string();
    assert_eq!(
        panel.selected_session().unwrap().native_id.as_deref(),
        Some("first")
    );
    panel.query.clear();
    assert_eq!(
        panel.selected_session().unwrap().native_id.as_deref(),
        Some("second")
    );
}

#[test]
fn each_source_tab_retains_its_own_selection() {
    let mut panel = loaded_panel(vec![
        native_session("first", "first", 20),
        native_session("second", "second", 10),
        transcript_session(
            RelayAgent::Codex,
            "/codex/first.jsonl",
            "codex first",
            "one",
            20,
        ),
        transcript_session(
            RelayAgent::Codex,
            "/codex/second.jsonl",
            "codex second",
            "two",
            10,
        ),
    ]);
    panel.move_selection(1);
    panel.set_tab(2);
    panel.move_selection(1);
    assert_eq!(panel.selected_session().unwrap().label, "codex second");

    panel.set_tab(0);
    assert_eq!(
        panel.selected_session().unwrap().native_id.as_deref(),
        Some("second")
    );
    panel.set_tab(2);
    assert_eq!(panel.selected_session().unwrap().label, "codex second");
}

#[test]
fn search_mode_routes_text_to_filter_and_keeps_refresh_explicit() {
    let mut panel = loaded_panel(vec![native_session("rust", "Rust runtime", 10)]);

    assert!(matches!(
        panel.handle_key(&key(KeyCode::Char('/'))),
        RelayPanelAction::None
    ));
    for character in "rust".chars() {
        panel.handle_key(&key(KeyCode::Char(character)));
    }
    assert!(panel.searching);
    assert_eq!(panel.query, "rust");
    assert_eq!(panel.active_indices().len(), 1);

    panel.handle_key(&ctrl(KeyCode::Char('u')));
    assert!(panel.query.is_empty());
    panel.handle_key(&key(KeyCode::Esc));
    assert!(!panel.searching);
    assert!(matches!(
        panel.handle_key(&key(KeyCode::Char('R'))),
        RelayPanelAction::Refresh
    ));
}

#[test]
fn filtered_mouse_item_mapping_activates_the_visible_session() {
    let mut panel = loaded_panel(vec![
        native_session("hidden", "unrelated", 20),
        native_session("visible", "needle task", 10),
    ]);
    panel.query = "needle".to_string();

    let session = panel.select_item(0, 0).unwrap();
    assert_eq!(session.native_id.as_deref(), Some("visible"));
}

#[test]
fn refresh_ticks_are_scoped_to_the_open_panel_generation() {
    let panel = loaded_panel(vec![]);

    assert!(panel.accepts_refresh_tick(7));
    assert!(!panel.accepts_refresh_tick(6));
    assert!(!panel.accepts_refresh_tick(8));
}
