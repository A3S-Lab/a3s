use super::*;

#[test]
fn parses_supported_ide_commands_without_claiming_editor_commands() {
    assert_eq!(
        parse_ide_intelligence_command("status"),
        Some(Ok(IdeIntelligenceCommand::Status))
    );
    assert_eq!(
        parse_ide_intelligence_command("symbols"),
        Some(Ok(IdeIntelligenceCommand::Symbols { query: None }))
    );
    assert_eq!(
        parse_ide_intelligence_command("symbols Runtime Registry"),
        Some(Ok(IdeIntelligenceCommand::Symbols {
            query: Some("Runtime Registry".to_owned())
        }))
    );
    assert_eq!(
        parse_ide_intelligence_command("diagnostics workspace"),
        Some(Ok(IdeIntelligenceCommand::Diagnostics { workspace: true }))
    );
    for (command, kind) in [
        ("definition", NavigationKind::Definition),
        ("declaration", NavigationKind::Declaration),
        ("references", NavigationKind::References),
        ("implementations", NavigationKind::Implementations),
    ] {
        assert_eq!(
            parse_ide_intelligence_command(command),
            Some(Ok(IdeIntelligenceCommand::Navigate(kind)))
        );
    }
    assert!(parse_ide_intelligence_command("diagnostics file")
        .expect("semantic command")
        .is_err());
    assert!(parse_ide_intelligence_command("w").is_none());
}

#[test]
fn semantic_commands_are_scoped_to_the_workspace_ide_surface() {
    let workspace = Ide::workspace(Vec::new());
    assert!(parse_ide_intelligence_command_for_ide(&workspace, "status").is_some());

    let config = Ide::browse(Vec::new(), "config");
    assert!(parse_ide_intelligence_command_for_ide(&config, "status").is_none());

    // A reused editor can have the same display title without becoming the
    // actual workspace `/ide` product surface.
    let readonly = Ide::browse(Vec::new(), "workspace");
    assert!(parse_ide_intelligence_command_for_ide(&readonly, "symbols query").is_none());

    let mut knowledge_base = Ide::browse(Vec::new(), "knowledge base");
    knowledge_base.kb_root = Some(PathBuf::from(".a3s/kb"));
    assert!(
        parse_ide_intelligence_command_for_ide(&knowledge_base, "diagnostics workspace").is_none()
    );
}

#[test]
fn maps_expanded_tabs_and_astral_characters_to_saved_utf16() {
    let text = "\t😀call();\n";
    assert_eq!(
        editor_position_to_saved_utf16(text, 0, 4).unwrap(),
        CodePosition::new(0, 1)
    );
    assert_eq!(
        editor_position_to_saved_utf16(text, 0, 5).unwrap(),
        CodePosition::new(0, 3)
    );
    assert_eq!(saved_utf16_to_editor_column("\t😀call();", 1).unwrap(), 4);
    assert_eq!(saved_utf16_to_editor_column("\t😀call();", 3).unwrap(), 5);
    assert!(saved_utf16_to_editor_column("😀", 1).is_err());
}

#[test]
fn rejects_cursor_positions_that_only_exist_in_an_unsaved_buffer() {
    let error = editor_position_to_saved_utf16("short\n", 0, 12).unwrap_err();
    assert!(error.contains("saved version"));
    assert!(editor_position_to_saved_utf16("short\n", 2, 0).is_err());
}

#[test]
fn rejects_provider_paths_that_escape_the_workspace() {
    let root = tempfile::tempdir().expect("temporary workspace");
    let services = WorkspaceServices::local(root.path());
    assert!(services.normalize_path("../outside.rs").is_err());
}

#[cfg(unix)]
#[tokio::test]
async fn semantic_jump_uses_workspace_symlink_containment() {
    use std::os::unix::fs::symlink;

    let root = tempfile::tempdir().expect("temporary workspace");
    let outside = tempfile::tempdir().expect("temporary outside directory");
    let outside_file = outside.path().join("secret.rs");
    std::fs::write(&outside_file, "fn secret() {}\n").unwrap();
    symlink(&outside_file, root.path().join("escape.rs")).unwrap();
    let services = WorkspaceServices::local(root.path());
    let path = services.normalize_path("escape.rs").unwrap();
    let result = read_ide_intelligence_jump(
        services.fs(),
        path,
        root.path().join("escape.rs"),
        CodePosition::new(0, 0),
        CancellationToken::new(),
    )
    .await;
    assert!(result.is_err());
}

#[test]
fn stale_request_ids_cannot_replace_the_active_view() {
    let mut ide = Ide::workspace(Vec::new());
    ide.intelligence_request_id = 8;
    ide.intelligence = Some(IdeIntelligenceView::loading(8, "new", true, false));
    assert!(ide_intelligence_request_is_current(&ide, 8));
    assert!(!ide_intelligence_request_is_current(&ide, 7));
    ide.intelligence_request_id = 9;
    assert!(!ide_intelligence_request_is_current(&ide, 8));
}

#[test]
fn stale_completion_cannot_mutate_the_latest_result_view() {
    let mut ide = Ide::workspace(Vec::new());
    ide.intelligence_request_id = 8;
    ide.intelligence = Some(IdeIntelligenceView::loading(8, "latest", true, false));

    let stale = IdeIntelligenceResult {
        title: "stale".to_owned(),
        rows: Vec::new(),
        truncated: false,
        saved_version: true,
        dirty_buffer: false,
        stale: false,
        workspace_revision: Some(1),
    };
    assert!(!apply_ide_intelligence_result_to_ide(
        &mut ide,
        7,
        Ok(stale)
    ));
    assert_eq!(
        ide.intelligence.as_ref().map(|view| view.title.as_str()),
        Some("latest")
    );

    let latest = IdeIntelligenceResult {
        title: "complete".to_owned(),
        rows: Vec::new(),
        truncated: true,
        saved_version: true,
        dirty_buffer: false,
        stale: true,
        workspace_revision: Some(2),
    };
    assert!(apply_ide_intelligence_result_to_ide(
        &mut ide,
        8,
        Ok(latest)
    ));
    let view = ide.intelligence.as_ref().unwrap();
    assert_eq!(view.title, "complete");
    assert!(view.truncated);
    assert!(view.stale);
    assert_eq!(view.workspace_revision, Some(2));
}

#[test]
fn latest_query_cancels_and_supersedes_the_previous_query() {
    let mut ide = Ide::workspace(Vec::new());
    let (first_id, first_cancellation) = replace_ide_intelligence_request(&mut ide);
    ide.intelligence = Some(IdeIntelligenceView::loading(first_id, "first", true, false));

    let (second_id, second_cancellation) = replace_ide_intelligence_request(&mut ide);
    ide.intelligence = Some(IdeIntelligenceView::loading(
        second_id, "second", true, false,
    ));

    assert!(first_cancellation.is_cancelled());
    assert!(!second_cancellation.is_cancelled());
    assert_ne!(first_id, second_id);
    assert!(!ide_intelligence_request_is_current(&ide, first_id));
    assert!(ide_intelligence_request_is_current(&ide, second_id));
}

#[test]
fn latest_jump_cancels_and_supersedes_the_previous_jump() {
    let mut ide = Ide::workspace(Vec::new());
    ide.intelligence_request_id = 8;
    ide.intelligence = Some(IdeIntelligenceView::loading(8, "results", true, false));

    let (first_id, first_cancellation) = replace_ide_intelligence_jump_request(&mut ide);
    assert!(ide_intelligence_jump_request_is_current(&ide, 8, first_id));
    assert!(!first_cancellation.is_cancelled());

    let (second_id, second_cancellation) = replace_ide_intelligence_jump_request(&mut ide);
    assert!(first_cancellation.is_cancelled());
    assert!(!second_cancellation.is_cancelled());
    assert_ne!(first_id, second_id);
    assert!(!ide_intelligence_jump_request_is_current(&ide, 8, first_id));
    assert!(ide_intelligence_jump_request_is_current(&ide, 8, second_id));
    assert!(!ide_intelligence_jump_request_is_current(
        &ide, 7, second_id
    ));
}

#[test]
fn dirty_jump_preserves_the_same_file_and_rejects_a_different_file() {
    let mut ide = Ide::workspace(Vec::new());
    let path = PathBuf::from("src/current.rs");
    let mut file = IdeFile::new(
        path.clone(),
        vec!["unsaved first".to_owned(), "unsaved second".to_owned()],
        false,
        false,
    );
    file.dirty = true;
    ide.file = Some(file);

    assert!(validate_ide_intelligence_jump_target(&ide, &path).is_ok());
    assert_eq!(
        validate_ide_intelligence_jump_target(&ide, Path::new("src/other.rs")),
        Err(DIRTY_JUMP_MESSAGE)
    );

    let preserved = install_ide_intelligence_jump(
        &mut ide,
        IdeIntelligenceJump {
            path,
            lines: vec!["saved first".to_owned(), "saved second".to_owned()],
            row: 1,
            col: 99,
        },
        10,
    );
    assert!(preserved);
    let file = ide.file.as_ref().unwrap();
    assert!(file.dirty);
    assert_eq!(file.lines[0], "unsaved first");
    assert_eq!(file.lines[1], "unsaved second");
    assert_eq!(file.row, 1);
    assert_eq!(file.col, "unsaved second".chars().count());
}

#[test]
fn dropping_an_ide_cancels_active_semantic_work() {
    let (query, jump) = {
        let ide = Ide::workspace(Vec::new());
        (
            ide.intelligence_cancellation.clone(),
            ide.intelligence_jump_cancellation.clone(),
        )
    };
    assert!(query.is_cancelled());
    assert!(jump.is_cancelled());
}

#[test]
fn dirty_result_notice_explicitly_ignores_unsaved_edits() {
    let view = IdeIntelligenceView::loading(1, "symbols", true, true);
    let notice = ide_intelligence_notice(&view);
    assert!(notice.contains("UNSAVED EDITS IGNORED"));
    assert!(notice.contains("saved version"));
}

#[test]
fn workspace_footer_discovers_semantic_commands() {
    let hint = ide_intelligence_command_hint();
    assert!(hint.contains(":status"));
    assert!(hint.contains(":symbols"));
    assert!(hint.contains(":definition"));
    assert!(hint.contains(":diagnostics"));
}
