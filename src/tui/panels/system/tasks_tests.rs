use super::*;
use a3s_code_core::SubagentProgressEntry;
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

fn task(
    task_id: &str,
    status: SubagentStatus,
    description: &str,
    updated_ms: u64,
) -> SubagentTaskSnapshot {
    SubagentTaskSnapshot {
        task_id: task_id.to_string(),
        parent_session_id: "parent".to_string(),
        child_session_id: format!("child-{task_id}"),
        agent: "reviewer".to_string(),
        description: description.to_string(),
        status,
        started_ms: updated_ms.saturating_sub(1_000),
        updated_ms,
        finished_ms: (status != SubagentStatus::Running).then_some(updated_ms),
        output: None,
        success: None,
        source_anchors: Vec::new(),
        progress: Vec::new(),
    }
}

fn loaded_panel(tasks: Vec<SubagentTaskSnapshot>) -> TaskPanel {
    let mut panel = TaskPanel::loading(7, "session".to_string());
    panel.request_id = 1;
    assert!(panel.apply_data("session", 7, 1, tasks));
    panel
}

#[test]
fn active_tasks_sort_first_and_recent_history_is_bounded() {
    let mut tasks = (0..64)
        .map(|index| {
            task(
                &format!("failed-{index}"),
                SubagentStatus::Failed,
                "failed",
                index,
            )
        })
        .collect::<Vec<_>>();
    tasks.push(task(
        "recent-completed",
        SubagentStatus::Completed,
        "recent completed",
        1_000,
    ));
    tasks.push(task(
        "recent-cancelled",
        SubagentStatus::Cancelled,
        "recent cancelled",
        999,
    ));
    tasks.push(task(
        "running-old",
        SubagentStatus::Running,
        "running old",
        1,
    ));
    tasks.push(task(
        "running-new",
        SubagentStatus::Running,
        "running new",
        100,
    ));

    let tasks = finalize_task_snapshots(tasks);

    assert_eq!(tasks.len(), TASK_PANEL_MAX_RECENT + 2);
    assert_eq!(tasks[0].task_id, "running-new");
    assert_eq!(tasks[1].task_id, "running-old");
    assert!(tasks[..2].iter().all(task_is_running));
    assert!(tasks.iter().any(|task| task.task_id == "recent-completed"));
    assert!(tasks.iter().any(|task| task.task_id == "recent-cancelled"));
    assert!(!tasks.iter().any(|task| task.task_id == "failed-0"));
    assert!(!tasks.iter().any(|task| task.task_id == "failed-1"));
}

#[test]
fn semantic_selection_survives_live_refresh_reordering() {
    let first = task("first", SubagentStatus::Running, "first task", 20);
    let selected = task("selected", SubagentStatus::Running, "selected task", 10);
    let mut panel = loaded_panel(vec![first.clone(), selected]);
    panel.move_selection(1);
    assert_eq!(panel.selected_task().unwrap().task_id, "selected");

    panel.request_id = 2;
    let mut updated = task("selected", SubagentStatus::Completed, "selected task", 30);
    updated.output = Some("finished output".to_string());
    assert!(panel.apply_data("session", 7, 2, vec![updated, first]));

    assert_eq!(panel.selected_task().unwrap().task_id, "selected");
    assert_eq!(
        panel.selected_task().unwrap().output.as_deref(),
        Some("finished output")
    );
}

#[test]
fn stale_session_generation_or_request_cannot_replace_panel_data() {
    let mut panel = loaded_panel(vec![task("kept", SubagentStatus::Running, "kept", 10)]);
    panel.request_id = 2;

    for (session_id, generation, request_id) in
        [("other", 7, 2), ("session", 8, 2), ("session", 7, 1)]
    {
        assert!(!panel.apply_data(
            session_id,
            generation,
            request_id,
            vec![task("stale", SubagentStatus::Completed, "stale", 20)],
        ));
    }
    assert_eq!(panel.tasks[0].task_id, "kept");
}

#[test]
fn filter_matches_status_agent_progress_output_and_task_id() {
    let mut running = task(
        "audit-123",
        SubagentStatus::Running,
        "Audit permission routing",
        10,
    );
    running.progress.push(SubagentProgressEntry {
        timestamp_ms: 10,
        status: "checking fallback".to_string(),
        metadata: serde_json::json!({"phase": "guardrail"}),
    });
    let mut completed = task(
        "build-456",
        SubagentStatus::Completed,
        "Compile workspace",
        20,
    );
    completed.output = Some("all targets passed".to_string());
    let mut panel = loaded_panel(vec![running, completed]);

    for query in ["running audit", "reviewer fallback", "audit-123 permission"] {
        panel.query = query.to_string();
        assert_eq!(panel.visible_indices().len(), 1, "{query}");
        assert_eq!(panel.selected_task().unwrap().task_id, "audit-123");
    }
    panel.query = "completed targets passed".to_string();
    assert_eq!(panel.visible_indices().len(), 1);
    assert_eq!(panel.selected_task().unwrap().task_id, "build-456");
}

#[test]
fn cancel_requires_two_matching_presses_and_tracks_inflight_task() {
    let mut panel = loaded_panel(vec![task(
        "running",
        SubagentStatus::Running,
        "running",
        10,
    )]);

    assert!(matches!(
        panel.handle_key(&key(KeyCode::Char('x'))),
        TaskPanelAction::None
    ));
    assert_eq!(panel.cancel_armed.as_deref(), Some("running"));
    match panel.handle_key(&key(KeyCode::Char('x'))) {
        TaskPanelAction::Cancel(task_id) => assert_eq!(task_id, "running"),
        _ => panic!("second X should request cancellation"),
    }
    assert_eq!(panel.cancel_inflight.as_deref(), Some("running"));
    assert!(panel.cancel_armed.is_none());
}

#[test]
fn escape_disarms_cancellation_before_closing_panel() {
    let mut panel = loaded_panel(vec![task(
        "running",
        SubagentStatus::Running,
        "running",
        10,
    )]);
    panel.handle_key(&key(KeyCode::Char('X')));

    assert!(matches!(
        panel.handle_key(&key(KeyCode::Esc)),
        TaskPanelAction::None
    ));
    assert!(panel.cancel_armed.is_none());
    assert!(matches!(
        panel.handle_key(&key(KeyCode::Esc)),
        TaskPanelAction::Close
    ));
}

#[test]
fn completed_task_refuses_cancellation_without_emitting_an_action() {
    let mut panel = loaded_panel(vec![task("done", SubagentStatus::Completed, "done", 10)]);

    assert!(matches!(
        panel.handle_key(&key(KeyCode::Delete)),
        TaskPanelAction::None
    ));
    assert!(panel.error.as_deref().unwrap().contains("running task"));
    assert!(panel.cancel_armed.is_none());
}

#[test]
fn search_mode_owns_printable_shortcuts_until_escape() {
    let mut panel = loaded_panel(vec![task(
        "rust",
        SubagentStatus::Running,
        "Rust checks",
        10,
    )]);

    panel.handle_key(&key(KeyCode::Char('/')));
    for character in "rust".chars() {
        panel.handle_key(&key(KeyCode::Char(character)));
    }
    assert!(panel.searching);
    assert_eq!(panel.query, "rust");
    assert_eq!(panel.visible_indices().len(), 1);

    panel.handle_key(&ctrl(KeyCode::Char('u')));
    assert!(panel.query.is_empty());
    panel.handle_key(&key(KeyCode::Esc));
    assert!(!panel.searching);
    assert!(matches!(
        panel.handle_key(&key(KeyCode::Char('R'))),
        TaskPanelAction::Refresh
    ));
}

#[test]
fn ctrl_b_is_a_panel_toggle_and_never_search_text() {
    let mut panel = loaded_panel(Vec::new());

    assert!(is_task_panel_key(&ctrl(KeyCode::Char('b'))));
    assert!(matches!(
        panel.handle_key(&ctrl(KeyCode::Char('b'))),
        TaskPanelAction::Close
    ));
}

#[test]
fn task_detail_document_contains_progress_and_terminal_output() {
    let mut snapshot = task(
        "task/detail",
        SubagentStatus::Completed,
        "Inspect output",
        20,
    );
    snapshot.progress.push(SubagentProgressEntry {
        timestamp_ms: 15,
        status: "reading".to_string(),
        metadata: serde_json::json!({"file": "src/main.rs"}),
    });
    snapshot.output = Some("Complete result".to_string());

    let document = task_output_document(&snapshot);

    assert!(document.contains("Task ID: task/detail"));
    assert!(document.contains("reading"));
    assert!(document.contains("src/main.rs"));
    assert!(document.contains("Complete result"));
    assert_eq!(task_output_title("task/detail"), "task-task_detail.txt");
}

#[test]
fn task_panel_rows_are_bounded_to_terminal_width() {
    let mut snapshot = task(
        "long",
        SubagentStatus::Running,
        "A very long delegated task description that must be clipped safely",
        10,
    );
    snapshot.progress.push(SubagentProgressEntry {
        timestamp_ms: 10,
        status: "a very long progress status that also needs clipping".to_string(),
        metadata: serde_json::Value::Null,
    });
    let mut panel = loaded_panel(vec![snapshot]);
    panel.preview = true;

    for line in task_menu_lines(&panel, 32, 12) {
        assert!(visible_len(&line) <= 32, "{line:?}");
    }
}
