use super::*;
use a3s_tui::style::{strip_ansi, visible_len};

fn grant(tool: &str, args: serde_json::Value) -> ExactPermissionGrant {
    ExactPermissionGrant::from_invocation(tool, &args)
}

fn panel_with_both_scopes() -> PermissionPanel {
    PermissionPanel::new(PermissionGrantSnapshot {
        session: vec![
            grant("bash", serde_json::json!({"command": "cargo test"})),
            grant(
                "write",
                serde_json::json!({"file_path": "README.md", "content": "updated"}),
            ),
        ],
        project: vec![grant(
            "edit",
            serde_json::json!({"file_path": "src/lib.rs", "new_string": "updated"}),
        )],
    })
}

#[test]
fn menu_separates_session_and_project_grants() {
    let panel = panel_with_both_scopes();
    let plain = strip_ansi(&permission_menu_lines(&panel, 96, 12).join("\n"));

    assert!(plain.contains("S"), "{plain}");
    assert!(
        plain.contains("session · expires when this TUI exits"),
        "{plain}"
    );
    assert!(plain.contains("P"), "{plain}");
    assert!(plain.contains("project · .a3s/permissions.acl"), "{plain}");
    assert!(plain.contains("2 session · 1 project"), "{plain}");
}

#[test]
fn filter_searches_scope_tool_and_exact_arguments() {
    let mut panel = panel_with_both_scopes();
    panel.query = "project src/lib.rs".to_string();
    panel.reset_filter_selection();

    let selected = panel.selected_grant().expect("project grant should match");
    assert_eq!(selected.scope, PermissionGrantScope::Project);
    assert_eq!(selected.grant.tool_name(), "edit");

    panel.query = "session cargo test".to_string();
    panel.reset_filter_selection();
    assert_eq!(
        panel
            .selected_grant()
            .expect("session grant should match")
            .grant
            .tool_name(),
        "bash"
    );
}

#[test]
fn revocation_requires_two_matching_keypresses() {
    let mut panel = panel_with_both_scopes();
    let delete = KeyEvent {
        code: KeyCode::Delete,
        modifiers: KeyModifiers::NONE,
    };

    assert!(matches!(
        panel.handle_key(&delete),
        PermissionPanelAction::None
    ));
    assert!(panel.revoke_armed.is_some());
    assert!(matches!(
        panel.handle_key(&delete),
        PermissionPanelAction::Revoke(PermissionGrantRow {
            scope: PermissionGrantScope::Session,
            ..
        })
    ));
    assert!(panel.revoke_armed.is_none());
}

#[test]
fn moving_selection_disarms_a_pending_revocation() {
    let mut panel = panel_with_both_scopes();
    panel.arm_or_revoke_selected();
    assert!(panel.revoke_armed.is_some());

    panel.move_selection(1);
    assert!(panel.revoke_armed.is_none());
    assert_eq!(panel.selected_grant().unwrap().grant.tool_name(), "write");
}

#[test]
fn snapshot_refresh_preserves_stable_selection_when_possible() {
    let mut panel = panel_with_both_scopes();
    panel.move_selection(2);
    let selected = panel.selected_grant().unwrap().identity();

    panel.sync_snapshot(PermissionGrantSnapshot {
        session: vec![grant(
            "read",
            serde_json::json!({"file_path": "Cargo.toml"}),
        )],
        project: vec![grant(
            "edit",
            serde_json::json!({"file_path": "src/lib.rs", "new_string": "different"}),
        )],
    });

    assert_eq!(panel.selected_grant().unwrap().identity(), selected);
}

#[test]
fn exact_details_explain_scope_arguments_and_revocation_boundary() {
    let panel = panel_with_both_scopes();
    let details = permission_details_document(panel.selected_grant().unwrap());

    assert!(details.contains("Scope: session"), "{details}");
    assert!(details.contains("Tool: bash"), "{details}");
    assert!(details.contains("\"command\": \"cargo test\""), "{details}");
    assert!(
        details.contains("future permission checks only"),
        "{details}"
    );
    assert!(details.contains("does not cancel a tool"), "{details}");
}

#[test]
fn empty_and_narrow_menus_remain_renderable() {
    let panel = PermissionPanel::new(PermissionGrantSnapshot::default());
    for width in [1_usize, 20, 48] {
        let lines = permission_menu_lines(&panel, width, 12);
        assert!(!lines.is_empty());
        assert!(
            lines.iter().all(|line| visible_len(line) <= width),
            "width={width}: {lines:?}"
        );
    }
    let plain = strip_ansi(&permission_menu_lines(&panel, 64, 12).join("\n"));
    assert!(plain.contains("no remembered permission grants"), "{plain}");
}
