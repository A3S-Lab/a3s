use super::*;

#[test]
fn mixed_selection_deduplicates_children_and_preserves_empty_directories() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();
    let folder = workspace.join("Research");
    std::fs::create_dir_all(folder.join("empty")).expect("create source tree");
    std::fs::write(folder.join("note.md"), "# Note\n").expect("write source note");
    std::fs::write(workspace.join("brief.txt"), "Brief").expect("write source file");

    let preview = preview_source_selection(
        workspace,
        &[
            folder.join("note.md"),
            folder.clone(),
            workspace.join("brief.txt"),
        ],
    )
    .expect("preview mixed selection");

    assert_eq!(preview.selected_count, 3);
    assert_eq!(preview.source_root_count, 2);
    assert_eq!(preview.deduplicated_count, 1);
    assert_eq!(preview.file_count, 2);
    assert!(preview.directory_count >= 2);
    assert_eq!(preview.items[0].destination, "sources/Research");
}

#[test]
fn content_digest_ignores_mtime_only_changes() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let source = temporary.path().join("note.md");
    std::fs::write(&source, "stable").expect("write source");
    let first = plan_source_selection(
        temporary.path(),
        std::slice::from_ref(&source),
        None,
        Duration::ZERO,
        SystemTime::now(),
    )
    .expect("first source plan");
    std::thread::sleep(Duration::from_millis(2));
    std::fs::write(&source, "stable").expect("touch source content");
    let second = plan_source_selection(
        temporary.path(),
        std::slice::from_ref(&source),
        Some(&first.snapshot),
        Duration::ZERO,
        SystemTime::now(),
    )
    .expect("second source plan");

    assert_eq!(
        first.snapshot.content_digest,
        second.snapshot.content_digest
    );
    assert!(!source_changes(Some(&first.snapshot), &second).changed);
}

#[test]
fn large_deletion_and_change_bursts_pause_automatic_compilation() {
    let previous = snapshot_fixture(200, "before");
    let current = SourceSelectionPlan {
        roots: Vec::new(),
        snapshot: snapshot_fixture(150, "after"),
        unstable_paths: Vec::new(),
    };
    let changes = source_changes(Some(&previous), &current);

    assert!(changes.changed);
    assert!(changes.safety_pause.is_some());
}

#[test]
fn routine_changes_in_small_knowledge_bases_do_not_pause_automatic_compilation() {
    let previous = snapshot_fixture(3, "small");
    let current = SourceSelectionPlan {
        roots: Vec::new(),
        snapshot: snapshot_fixture(2, "small"),
        unstable_paths: Vec::new(),
    };
    let changes = source_changes(Some(&previous), &current);

    assert!(changes.changed);
    assert_eq!(changes.deleted, 1);
    assert!(changes.safety_pause.is_none());
}

#[test]
fn selection_creation_has_no_searchable_wiki_before_compilation() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();
    let source = workspace.join("note.md");
    std::fs::write(&source, "# Source\n").expect("write source");

    let mutation = create_from_selection(
        workspace,
        &[source],
        "Source Ready",
        None,
        compilation::CompilationPolicy::Manual,
    )
    .expect("create source-ready knowledge base");
    let base = PathBuf::from(&mutation.knowledge_base.path);

    assert_eq!(mutation.knowledge_base.concept_count, 0);
    assert!(!base.join("wiki").exists());
}

#[test]
fn materialization_is_confined_to_sources_and_round_trips_source_set() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path().join("workspace");
    let staging = temporary.path().join("staging");
    std::fs::create_dir_all(&workspace).expect("create workspace");
    std::fs::write(workspace.join("note.md"), "hello").expect("write source");
    std::fs::create_dir(&staging).expect("create staging");
    let plan = plan_source_selection(
        &workspace,
        &[workspace.join("note.md")],
        None,
        Duration::ZERO,
        SystemTime::now(),
    )
    .expect("source plan");

    materialize_source_package(&staging, &plan).expect("materialize source package");

    assert_eq!(
        std::fs::read_to_string(staging.join("sources/note.md")).expect("read packaged source"),
        "hello"
    );
    assert_eq!(
        read_source_set(&staging).expect("read source set"),
        Some(plan.roots)
    );
    assert_eq!(
        load_snapshot(&staging).expect("read snapshot"),
        Some(plan.snapshot)
    );
}

fn snapshot_fixture(count: usize, prefix: &str) -> SourceSnapshot {
    let files = (0..count)
        .map(|index| {
            (
                format!("{prefix}-{index}.md"),
                SourceFileSnapshot {
                    origin_path: format!("/{prefix}-{index}.md"),
                    size: 1,
                    modified_micros: 1,
                    content_digest: format!("sha256:{prefix}-{index}"),
                },
            )
        })
        .collect::<BTreeMap<_, _>>();
    let mut snapshot = SourceSnapshot {
        schema_version: SOURCE_SNAPSHOT_SCHEMA,
        file_count: files.len(),
        bytes: files.len() as u64,
        files,
        ..SourceSnapshot::default()
    };
    snapshot.content_digest = snapshot_digest(&snapshot);
    snapshot
}
