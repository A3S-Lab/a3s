use super::*;
use crate::top::Risk;

#[path = "system_agents_tests/control.rs"]
mod control;

fn process(pid: u32, ppid: u32, agent: Option<AgentKind>) -> ProcessRow {
    ProcessRow {
        pid,
        ppid,
        cpu_pct: 0.0,
        mem_pct: 0.0,
        elapsed: "00:01".to_string(),
        cwd: Some(format!("/workspace/{pid}")),
        command: "agent --secret prompt".to_string(),
        agent,
        risk: Risk::Low,
    }
}

fn presence(instance: &str, pid: u32, state: AgentActivityState) -> AgentPresence {
    AgentPresence::new(
        instance,
        pid,
        format!("/workspace/{pid}"),
        Some("implement the system island".to_string()),
        state,
        Vec::new(),
        epoch_ms(),
    )
}

async fn write_raw_presence(directory: &Path, presence: &AgentPresence) -> PathBuf {
    let path = directory.join(format!("{}.json", presence.instance_id));
    tokio::fs::write(&path, serde_json::to_vec(presence).unwrap())
        .await
        .unwrap();
    path
}

fn write_raw_presence_sync(directory: &Path, presence: &AgentPresence) -> PathBuf {
    let path = directory.join(format!("{}.json", presence.instance_id));
    std::fs::write(&path, serde_json::to_vec(presence).unwrap()).unwrap();
    path
}

#[test]
fn exact_presence_replaces_same_process_and_keeps_inferred_agents() {
    let now = epoch_ms();
    let mut exact = presence("local", 10, AgentActivityState::Working);
    exact.updated_at_ms = now.saturating_sub(9_000);
    let rows = aggregate_activities(
        &[exact],
        &[
            process(10, 1, Some(AgentKind::A3sCode)),
            process(20, 1, Some(AgentKind::Codex)),
        ],
        "local",
        now,
    );

    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].confidence, AgentActivityConfidence::Exact);
    assert!(rows[0].local);
    assert_eq!(rows[0].expires_at_ms, now.saturating_add(1_000));
    assert_eq!(rows[1].confidence, AgentActivityConfidence::Process);
    assert_eq!(rows[1].expires_at_ms, now.saturating_add(PRESENCE_TTL_MS));
    assert_eq!(rows[1].started_at_ms, Some(now.saturating_sub(1_000)));
    assert_eq!(rows[1].task.as_deref(), Some("active process"));
    assert!(!rows[1]
        .task
        .as_deref()
        .unwrap_or_default()
        .contains("secret"));
}

#[test]
fn inferred_process_elapsed_time_projects_a_stable_start_time() {
    let now = 200_000_000;
    for (elapsed, seconds) in [
        ("02:03", 123),
        ("04:02:03", 14_523),
        ("2-04:02:03", 187_323),
    ] {
        let mut detected = process(20, 1, Some(AgentKind::Codex));
        detected.elapsed = elapsed.to_string();
        let rows = aggregate_activities(&[], &[detected], "local", now);

        assert_eq!(
            rows[0].started_at_ms,
            Some(now - seconds * 1_000),
            "{elapsed}"
        );
    }

    for elapsed in ["-", "01:60", "1-24:00:00"] {
        let mut detected = process(20, 1, Some(AgentKind::Codex));
        detected.elapsed = elapsed.to_string();
        let rows = aggregate_activities(&[], &[detected], "local", now);

        assert_eq!(rows[0].started_at_ms, None, "{elapsed}");
    }
}

#[test]
fn nested_same_agent_process_is_collapsed_to_its_root() {
    let processes = [
        process(20, 1, Some(AgentKind::Codex)),
        process(21, 20, Some(AgentKind::Codex)),
        process(22, 21, None),
    ];
    let roots = root_agent_processes(&processes);

    assert_eq!(roots.len(), 1);
    assert_eq!(roots[0].pid, 20);
}

#[test]
fn stale_presence_cannot_resurrect_a_finished_process() {
    let now = epoch_ms();
    let mut stale = presence("old", 44, AgentActivityState::Working);
    stale.updated_at_ms = now.saturating_sub(PRESENCE_TTL_MS + 1);

    let rows = aggregate_activities(&[stale], &[], "new", now);

    assert!(rows.is_empty());
}

#[test]
fn activity_order_puts_attention_and_live_local_state_first() {
    let now = epoch_ms();
    let local = presence("local", 1, AgentActivityState::Working);
    let failed = presence("remote", 2, AgentActivityState::Failed);
    let idle = presence("idle", 3, AgentActivityState::Idle);

    let rows = aggregate_activities(&[idle, local, failed], &[], "local", now);

    assert_eq!(rows[0].state, AgentActivityState::Failed);
    assert_eq!(rows[1].state, AgentActivityState::Working);
    assert!(rows[1].local);
    assert_eq!(rows[2].state, AgentActivityState::Idle);
}

#[test]
fn live_process_detection_outranks_a_retained_completed_lifecycle() {
    let now = epoch_ms();
    let completed = presence("local", 1, AgentActivityState::Completed);
    let rows = aggregate_activities(
        &[completed],
        &[process(20, 1, Some(AgentKind::Codex))],
        "local",
        now,
    );

    assert_eq!(rows[0].confidence, AgentActivityConfidence::Process);
    assert_eq!(rows[0].state, AgentActivityState::Unknown);
    assert_eq!(rows[1].confidence, AgentActivityConfidence::Exact);
    assert_eq!(rows[1].state, AgentActivityState::Completed);
}

#[test]
fn island_launch_accepts_exact_lifecycle_or_a_detected_external_agent() {
    let now = epoch_ms();
    let process_rows = [process(20, 1, Some(AgentKind::Codex))];

    let idle = SystemAgentSnapshot {
        activities: aggregate_activities(
            &[presence("local", 1, AgentActivityState::Idle)],
            &[],
            "local",
            now,
        ),
        warnings: Vec::new(),
    };
    assert!(!snapshot_requests_island_launch(&idle));

    for state in [
        AgentActivityState::Planning,
        AgentActivityState::Working,
        AgentActivityState::WaitingApproval,
        AgentActivityState::Completed,
        AgentActivityState::Failed,
        AgentActivityState::Cancelled,
    ] {
        let snapshot = SystemAgentSnapshot {
            activities: aggregate_activities(
                &[presence("local", 1, state)],
                &process_rows,
                "local",
                now,
            ),
            warnings: Vec::new(),
        };
        assert!(
            snapshot_requests_island_launch(&snapshot),
            "state {state:?} should request the island"
        );
    }

    let process_only = SystemAgentSnapshot {
        activities: aggregate_activities(&[], &process_rows, "local", now),
        warnings: Vec::new(),
    };
    assert!(
        snapshot_requests_island_launch(&process_only),
        "a detected Codex process should be enough to open the shared island"
    );
}

#[tokio::test]
async fn publisher_round_trips_private_heartbeat_and_removes_it() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let local = presence(
        publisher.instance_id(),
        std::process::id(),
        AgentActivityState::Idle,
    );

    publisher.write_presence(&local).await.unwrap();
    let stored = publisher.read_presences(epoch_ms()).await.unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].instance_id, publisher.instance_id());
    assert_eq!(stored[0].workspace, std::process::id().to_string());
    assert!(stored[0].task.is_none());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(publisher.path.as_ref().unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(mode, 0o600);
        let directory_mode = std::fs::metadata(temp.path()).unwrap().permissions().mode() & 0o777;
        assert_eq!(directory_mode, 0o700);
    }

    publisher.remove().await;
    assert!(!publisher.path.as_ref().unwrap().exists());
    assert!(publisher.write_presence(&local).await.is_err());
    assert!(!publisher.path.as_ref().unwrap().exists());
}

#[tokio::test]
async fn registry_discovers_multiple_live_publishers() {
    let temp = tempfile::tempdir().unwrap();
    let left = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let right = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let left_presence = presence(
        left.instance_id(),
        std::process::id(),
        AgentActivityState::Working,
    );
    let right_presence = presence(
        right.instance_id(),
        std::process::id(),
        AgentActivityState::Idle,
    );

    left.write_presence(&left_presence).await.unwrap();
    right.write_presence(&right_presence).await.unwrap();
    let mut stored = left.read_presences(epoch_ms()).await.unwrap();
    stored.sort_by(|a, b| a.instance_id.cmp(&b.instance_id));

    assert_eq!(stored.len(), 2);
    assert!(stored
        .iter()
        .any(|item| item.instance_id == left.instance_id()));
    assert!(stored
        .iter()
        .any(|item| item.instance_id == right.instance_id()));

    left.remove().await;
    right.remove().await;
}

#[test]
fn registry_file_names_are_canonical_pid_and_hex_instances() {
    let valid = parse_presence_file_name(OsStr::new("42-0123456789abcdef.json")).unwrap();
    assert_eq!(valid.pid, 42);
    assert_eq!(valid.instance_id, "42-0123456789abcdef");

    for invalid in [
        "notes.json",
        "42-0123456789abcde.json",
        "42-0123456789abcdef.json.bak",
        "42-0123456789abcdeg.json",
        "042-0123456789abcdef.json",
        "0-0123456789abcdef.json",
        "42-0123456789abcdef-extra.json",
    ] {
        assert!(
            parse_presence_file_name(OsStr::new(invalid)).is_none(),
            "accepted {invalid}"
        );
    }
}

#[test]
fn relative_status_directory_is_made_absolute_for_the_helper_contract() {
    let current = std::env::current_dir().unwrap();
    let relative = PathBuf::from("private-agent-status");

    let resolved = absolute_status_directory(relative.clone(), Some(&current)).unwrap();

    assert!(resolved.is_absolute());
    assert_eq!(resolved, current.join(relative));
    assert_eq!(
        absolute_status_directory(current.clone(), None),
        Some(current)
    );
    assert!(absolute_status_directory(PathBuf::from("relative"), None).is_none());
}

#[tokio::test]
async fn registry_preserves_non_protocol_and_identity_mismatched_files() {
    let temp = tempfile::tempdir().unwrap();
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let unrelated = temp.path().join("settings.json");
    tokio::fs::write(&unrelated, b"{\"keep\":true}")
        .await
        .unwrap();
    let malformed = temp.path().join("77-0000000000000001.json");
    tokio::fs::write(&malformed, b"not a heartbeat")
        .await
        .unwrap();

    let mismatched = presence("77-0000000000000002", 77, AgentActivityState::Working);
    let mismatched_path = temp.path().join("78-0000000000000002.json");
    tokio::fs::write(&mismatched_path, serde_json::to_vec(&mismatched).unwrap())
        .await
        .unwrap();
    let valid = presence("79-0000000000000003", 79, AgentActivityState::Working);
    let valid_path = write_raw_presence(temp.path(), &valid).await;

    let stored = reader.read_presences(epoch_ms()).await.unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].instance_id, valid.instance_id);
    assert!(unrelated.exists());
    assert!(malformed.exists());
    assert!(mismatched_path.exists());
    assert!(valid_path.exists());
}

#[tokio::test]
async fn unrelated_json_does_not_consume_the_protocol_scan_budget() {
    let temp = tempfile::tempdir().unwrap();
    for index in 0..(MAX_PRESENCE_FILES + 20) {
        tokio::fs::write(temp.path().join(format!("unrelated-{index}.json")), b"{}")
            .await
            .unwrap();
    }
    let valid = presence("81-0000000000000004", 81, AgentActivityState::Working);
    write_raw_presence(temp.path(), &valid).await;
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let stored = reader.read_presences(epoch_ms()).await.unwrap();

    assert_eq!(stored.len(), 1);
    assert_eq!(stored[0].instance_id, valid.instance_id);
}

#[tokio::test]
async fn registry_directory_overflow_is_reported_instead_of_returning_partial_evidence() {
    let _process_probe_guard = agent_island_process_test_lock().lock().await;
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    // Every entry is valid so an implementation that stops at the 256-record
    // evidence cap would miss the independent hard directory limit.
    for index in 1..=MAX_REGISTRY_ENTRIES + 1 {
        let pid = (30_000 + index) as u32;
        let mut item = presence(
            &format!("{pid}-{index:016x}"),
            pid,
            AgentActivityState::Working,
        );
        item.updated_at_ms = now;
        write_raw_presence_sync(temp.path(), &item);
    }
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let error = reader.read_presences(now).await.unwrap_err();

    assert!(error.to_string().contains("directory entry limit"));
}

#[tokio::test]
async fn live_publisher_cap_marks_the_exported_snapshot_degraded() {
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    for index in 1..=MAX_PRESENCE_FILES + 1 {
        let pid = (40_000 + index) as u32;
        let mut item = presence(
            &format!("{pid}-{index:016x}"),
            pid,
            AgentActivityState::Working,
        );
        item.updated_at_ms = now;
        write_raw_presence(temp.path(), &item).await;
    }
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let scan = publisher.scan_presences(now, None).await.unwrap();
    assert_eq!(scan.presences.len(), MAX_PRESENCE_FILES);
    assert!(scan.truncated);

    let local = presence(
        publisher.instance_id(),
        std::process::id(),
        AgentActivityState::Idle,
    );
    let result = publisher.publish_collect_and_export(local).await;
    assert!(result
        .warnings
        .iter()
        .any(|warning| warning.contains("publisher evidence limit")));
    let bytes = tokio::fs::read(result.snapshot_path.unwrap())
        .await
        .unwrap();
    let snapshot: SystemAgentProtocolSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert!(snapshot.degraded);
    assert!(snapshot.activities.len() <= MAX_SYSTEM_ACTIVITIES);
    publisher.remove().await;
}

#[tokio::test]
async fn newer_live_presence_is_not_hidden_by_crash_leftovers() {
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    for index in 1..=(MAX_PRESENCE_FILES + 20) {
        let instance = format!("{}-{index:016x}", 10_000 + index);
        let mut stale = presence(
            &instance,
            (10_000 + index) as u32,
            AgentActivityState::Working,
        );
        stale.updated_at_ms = now.saturating_sub(PRESENCE_TTL_MS + 1);
        write_raw_presence(temp.path(), &stale).await;
    }
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    let live = presence("99-0000000000000007", 99, AgentActivityState::Working);
    write_raw_presence(temp.path(), &live).await;
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let stored = reader.read_presences(epoch_ms()).await.unwrap();

    assert!(stored
        .iter()
        .any(|presence| presence.instance_id == live.instance_id));
}

#[tokio::test]
async fn expired_crashed_presences_are_compacted_with_a_per_scan_cap() {
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    for index in 1..=(MAX_STALE_REMOVALS_PER_SCAN + 1) {
        let pid = (50_000 + index) as u32;
        let mut stale = presence(
            &format!("{pid}-{index:016x}"),
            pid,
            AgentActivityState::Working,
        );
        stale.updated_at_ms = now.saturating_sub(PRESENCE_TTL_MS + 1);
        write_raw_presence(temp.path(), &stale).await;
    }
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let observed_pids = HashSet::new();

    reader
        .scan_presences(now, Some(&observed_pids))
        .await
        .unwrap();
    let remaining = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| parse_presence_file_name(&entry.file_name()).is_some())
        .count();
    assert_eq!(remaining, 1);

    reader
        .scan_presences(now, Some(&observed_pids))
        .await
        .unwrap();
    let remaining = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| parse_presence_file_name(&entry.file_name()).is_some())
        .count();
    assert_eq!(remaining, 0);
}

#[tokio::test]
async fn observed_or_unknown_process_state_disables_stale_compaction() {
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    let pid = std::process::id();
    let mut stale = presence(
        &format!("{pid}-00000000000000aa"),
        pid,
        AgentActivityState::Working,
    );
    stale.updated_at_ms = now.saturating_sub(PRESENCE_TTL_MS + 1);
    let path = write_raw_presence(temp.path(), &stale).await;
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let observed_pids = HashSet::from([pid]);
    reader
        .scan_presences(now, Some(&observed_pids))
        .await
        .unwrap();
    assert!(path.exists());

    reader.scan_presences(now, None).await.unwrap();
    assert!(path.exists());
}

#[tokio::test]
async fn future_skewed_crashed_presence_is_compacted_when_its_pid_is_absent() {
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    let pid = 61_001;
    let mut stale = presence(
        &format!("{pid}-00000000000000ab"),
        pid,
        AgentActivityState::Working,
    );
    stale.updated_at_ms = now.saturating_add(PRESENCE_FUTURE_SKEW_MS + 1);
    let path = write_raw_presence(temp.path(), &stale).await;
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let observed_pids = HashSet::new();

    let scan = reader
        .scan_presences(now, Some(&observed_pids))
        .await
        .unwrap();

    assert!(scan.presences.is_empty());
    assert!(!path.exists());
}

#[tokio::test]
async fn future_skewed_presence_is_preserved_for_an_observed_or_unknown_pid() {
    let temp = tempfile::tempdir().unwrap();
    let now = epoch_ms();
    let pid = 61_002;
    let mut stale = presence(
        &format!("{pid}-00000000000000ac"),
        pid,
        AgentActivityState::Working,
    );
    stale.updated_at_ms = now.saturating_add(PRESENCE_FUTURE_SKEW_MS + 1);
    let path = write_raw_presence(temp.path(), &stale).await;
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let observed_pids = HashSet::from([pid]);
    reader
        .scan_presences(now, Some(&observed_pids))
        .await
        .unwrap();
    assert!(path.exists());

    reader.scan_presences(now, None).await.unwrap();
    assert!(path.exists());
}

#[tokio::test]
async fn canonical_malformed_flood_does_not_consume_the_valid_presence_limit() {
    let temp = tempfile::tempdir().unwrap();
    let live = presence("98-0000000000000008", 98, AgentActivityState::Working);
    write_raw_presence(temp.path(), &live).await;
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    for index in 1..=300usize {
        let pid = 20_000 + index;
        let path = temp.path().join(format!("{pid}-{index:016x}.json"));
        tokio::fs::write(path, b"{}" as &[u8]).await.unwrap();
    }
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let stored = reader.read_presences(epoch_ms()).await.unwrap();

    assert!(stored
        .iter()
        .any(|presence| presence.instance_id == live.instance_id));
}

#[cfg(unix)]
#[tokio::test]
async fn registry_ignores_symlinks_even_with_protocol_file_names() {
    use std::os::unix::fs::symlink;

    let temp = tempfile::tempdir().unwrap();
    let outside = tempfile::NamedTempFile::new().unwrap();
    let now = epoch_ms();
    let mut stale = presence("82-0000000000000005", 82, AgentActivityState::Working);
    stale.updated_at_ms = now.saturating_sub(PRESENCE_TTL_MS + 1);
    std::fs::write(outside.path(), serde_json::to_vec(&stale).unwrap()).unwrap();
    let link = temp.path().join("82-0000000000000005.json");
    symlink(outside.path(), &link).unwrap();
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let observed_pids = HashSet::new();

    let stored = reader
        .scan_presences(now, Some(&observed_pids))
        .await
        .unwrap()
        .presences;

    assert!(stored.is_empty());
    assert!(link.symlink_metadata().unwrap().file_type().is_symlink());
    assert!(outside.path().exists());
}

#[tokio::test]
async fn inbound_protocol_strings_are_sanitized_and_bounded() {
    let temp = tempfile::tempdir().unwrap();
    let mut hostile = presence("83-0000000000000006", 83, AgentActivityState::Working);
    hostile.workspace = "/private/source/unsafe\u{1b}[2Jrepo\nname".to_string();
    hostile.task = Some(format!(
        "task\u{1b}]52;c;c2VjcmV0\u{7}\u{202e}{}",
        "x".repeat(MAX_TASK_CHARS + 20)
    ));
    hostile.children = vec![AgentChildPresence {
        id: "child\n\u{1b}[2Jid".to_string(),
        agent: "codex\u{9b}31mred".to_string(),
        task: Some("child\r\nsecret\u{2067}".to_string()),
        state: AgentActivityState::Working,
        vendor: AgentVendor::OpenAi,
        started_at_ms: None,
        finished_at_ms: None,
        actions: Vec::new(),
    }];
    write_raw_presence(temp.path(), &hostile).await;
    let reader = AgentPresencePublisher::for_directory(temp.path().to_path_buf());

    let stored = reader.read_presences(epoch_ms()).await.unwrap();
    let stored = &stored[0];

    for value in [
        stored.workspace.as_str(),
        stored.task.as_deref().unwrap(),
        stored.children[0].id.as_str(),
        stored.children[0].agent.as_str(),
        stored.children[0].task.as_deref().unwrap(),
    ] {
        assert!(!value.chars().any(char::is_control), "{value:?}");
        assert!(!value.contains("[2J"), "{value:?}");
        assert!(!value.contains('\u{202e}'), "{value:?}");
        assert!(!value.contains('\u{2067}'), "{value:?}");
    }
    assert!(!stored.workspace.contains("private"));
    assert!(stored.task.as_ref().unwrap().chars().count() <= MAX_TASK_CHARS);
}

#[tokio::test]
async fn heartbeat_redacts_tasks_by_default_and_shares_only_with_opt_in() {
    let temp = tempfile::tempdir().unwrap();
    let private = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let private_task = "parent-private\nfull prompt\u{1b}[2J".to_string();
    let mut local = AgentPresence::new(
        private.instance_id(),
        std::process::id(),
        "/Users/example/secret-workspace/repository",
        Some(private_task.clone()),
        AgentActivityState::Working,
        vec![AgentChildPresence {
            id: "child-1".to_string(),
            agent: "codex".to_string(),
            task: Some("child-private description".to_string()),
            state: AgentActivityState::Working,
            vendor: AgentVendor::OpenAi,
            started_at_ms: None,
            finished_at_ms: None,
            actions: Vec::new(),
        }],
        epoch_ms(),
    );

    private.write_presence(&local).await.unwrap();
    let bytes = tokio::fs::read(private.path.as_ref().unwrap())
        .await
        .unwrap();
    let serialized = String::from_utf8(bytes).unwrap();
    let stored = private.read_presences(epoch_ms()).await.unwrap();

    assert_eq!(local.task.as_deref(), Some(private_task.as_str()));
    assert_eq!(stored[0].workspace, "repository");
    assert!(stored[0].task.is_none());
    assert!(stored[0].children[0].task.is_none());
    assert!(!serialized.contains("parent-private"));
    assert!(!serialized.contains("child-private"));
    assert!(!serialized.contains("secret-workspace"));
    assert!(!serialized.contains("session_id"));
    assert!(!serialized.contains("model"));
    private.remove().await;

    let shared = AgentPresencePublisher::for_directory_with_task_sharing(temp.path().to_path_buf());
    local.instance_id = shared.instance_id().to_string();
    local.pid = std::process::id();
    shared.write_presence(&local).await.unwrap();
    let stored = shared.read_presences(epoch_ms()).await.unwrap();

    assert_eq!(
        stored[0].task.as_deref(),
        Some("parent-private full prompt")
    );
    assert_eq!(
        stored[0].children[0].task.as_deref(),
        Some("child-private description")
    );
    shared.remove().await;
}

#[tokio::test]
async fn exported_snapshot_redacts_the_local_process_and_is_fresh_and_private() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let local = AgentPresence::new(
        publisher.instance_id(),
        std::process::id(),
        "/Users/example/private-parent/repository",
        Some("raw parent\nsecret prompt\u{1b}[2J".to_string()),
        AgentActivityState::Working,
        vec![AgentChildPresence {
            id: "child-1".to_string(),
            agent: "codex".to_string(),
            task: Some("raw child secret".to_string()),
            state: AgentActivityState::WaitingInput,
            vendor: AgentVendor::OpenAi,
            started_at_ms: Some(epoch_ms().saturating_sub(100)),
            finished_at_ms: None,
            actions: Vec::new(),
        }],
        epoch_ms().saturating_sub(500),
    )
    .with_attention_reason(Some(
        "Permission required\nbefore running Bash(cargo test).\u{1b}[2J".to_string(),
    ));
    let before = epoch_ms();

    let result = publisher.publish_collect_and_export(local).await;
    let after = epoch_ms();
    assert!(result.launch_requested);
    let path = result
        .snapshot_path
        .expect("snapshot export should succeed");
    let bytes = tokio::fs::read(&path).await.unwrap();
    let raw = String::from_utf8(bytes.clone()).unwrap();
    let snapshot: SystemAgentProtocolSnapshot = serde_json::from_slice(&bytes).unwrap();

    assert_eq!(snapshot.schema, SYSTEM_SNAPSHOT_SCHEMA);
    assert!(snapshot.updated_at_ms >= before);
    assert!(snapshot.updated_at_ms <= after);
    let parent = snapshot
        .activities
        .iter()
        .find(|activity| activity.id == publisher.instance_id())
        .expect("local parent should be exported through the privacy protocol");
    assert_eq!(parent.workspace.as_deref(), Some("repository"));
    assert!(parent.task.is_none());
    assert_eq!(
        parent.reason.as_deref(),
        Some("Permission required before running Bash(cargo test).")
    );
    assert_eq!(parent.state, AgentActivityState::Working);
    assert_eq!(parent.confidence, AgentActivityConfidence::Exact);
    let child = snapshot
        .activities
        .iter()
        .find(|activity| activity.parent_id.as_deref() == Some(publisher.instance_id()))
        .expect("local child should be exported through the privacy protocol");
    assert!(child.task.is_none());
    assert_eq!(child.state, AgentActivityState::WaitingInput);
    assert!(!raw.contains("private-parent"));
    assert!(!raw.contains("raw parent"));
    assert!(!raw.contains("raw child"));
    assert!(!raw.contains("\"local\""));
    assert!(raw.contains("\"state\":\"working\""));
    assert!(raw.contains("\"confidence\":\"exact\""));
    assert!(!raw.contains("\"parent_id\":null"));
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mode = std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    publisher.remove().await;
}

#[tokio::test]
async fn multiple_tuis_publish_one_shared_snapshot_and_singleton_lock_path() {
    let temp = tempfile::tempdir().unwrap();
    let first = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let second = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let first_presence = presence(
        first.instance_id(),
        std::process::id(),
        AgentActivityState::Working,
    );
    let second_presence = presence(
        second.instance_id(),
        std::process::id(),
        AgentActivityState::Working,
    );

    let first_result = first.publish_collect_and_export(first_presence).await;
    let second_result = second.publish_collect_and_export(second_presence).await;

    assert_eq!(first_result.snapshot_path, second_result.snapshot_path);
    assert_eq!(first_result.lock_path, second_result.lock_path);
    assert_eq!(
        first_result.lock_path.as_deref(),
        Some(temp.path().join(ISLAND_LOCK_FILE).as_path())
    );

    first.remove().await;
    second.remove().await;
}

#[tokio::test]
async fn exported_local_tasks_require_opt_in_and_are_sanitized() {
    let temp = tempfile::tempdir().unwrap();
    let publisher =
        AgentPresencePublisher::for_directory_with_task_sharing(temp.path().to_path_buf());
    let local = AgentPresence::new(
        publisher.instance_id(),
        std::process::id(),
        "/private/repository",
        Some("parent\nlabel\u{1b}[2J".to_string()),
        AgentActivityState::Planning,
        vec![AgentChildPresence {
            id: "child-1".to_string(),
            agent: "codex".to_string(),
            task: Some("child\r\nlabel".to_string()),
            state: AgentActivityState::Working,
            vendor: AgentVendor::OpenAi,
            started_at_ms: None,
            finished_at_ms: None,
            actions: Vec::new(),
        }],
        epoch_ms(),
    );

    let result = publisher.publish_collect_and_export(local).await;
    let bytes = tokio::fs::read(result.snapshot_path.unwrap())
        .await
        .unwrap();
    let snapshot: SystemAgentProtocolSnapshot = serde_json::from_slice(&bytes).unwrap();
    let parent = snapshot
        .activities
        .iter()
        .find(|activity| activity.id == publisher.instance_id())
        .unwrap();
    let child = snapshot
        .activities
        .iter()
        .find(|activity| activity.parent_id.as_deref() == Some(publisher.instance_id()))
        .unwrap();

    assert_eq!(parent.task.as_deref(), Some("parent label"));
    assert_eq!(child.task.as_deref(), Some("child label"));
    publisher.remove().await;
}

#[tokio::test]
async fn exported_snapshot_is_bounded_degraded_and_atomically_replaced() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let activities = (0..MAX_SYSTEM_ACTIVITIES + 40)
        .map(|index| SystemAgentActivity {
            id: format!("agent-{index}-{}", "x".repeat(MAX_ACTIVITY_ID_CHARS + 20)),
            parent_id: None,
            agent: format!("agent\n{}", "界".repeat(MAX_AGENT_CHARS + 20)),
            workspace: Some(format!(
                "workspace\u{1b}[2J{}",
                "界".repeat(MAX_WORKSPACE_CHARS)
            )),
            task: Some(format!("task\n{}", "界".repeat(MAX_TASK_CHARS + 20))),
            reason: None,
            state: AgentActivityState::Unknown,
            confidence: AgentActivityConfidence::Process,
            vendor: AgentVendor::Other,
            started_at_ms: None,
            finished_at_ms: None,
            expires_at_ms: epoch_ms().saturating_add(PRESENCE_TTL_MS),
            actions: Vec::new(),
            local: false,
        })
        .collect();
    let first = SystemAgentSnapshot {
        activities,
        warnings: Vec::new(),
    };

    let first_evidence_at_ms = 1_234;
    let path = publisher
        .write_system_snapshot(&first, first_evidence_at_ms)
        .await
        .unwrap();
    let bytes = tokio::fs::read(&path).await.unwrap();
    let snapshot: SystemAgentProtocolSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(snapshot.updated_at_ms, first_evidence_at_ms);
    assert!(snapshot.degraded);
    assert_eq!(snapshot.activities.len(), MAX_SYSTEM_ACTIVITIES);
    assert!(bytes.len() as u64 <= MAX_SYSTEM_SNAPSHOT_BYTES);
    assert!(snapshot.activities.iter().all(|activity| {
        activity.id.chars().count() <= MAX_ACTIVITY_ID_CHARS
            && activity.agent.chars().count() <= MAX_AGENT_CHARS
            && activity
                .workspace
                .as_deref()
                .is_none_or(|value| value.chars().count() <= MAX_WORKSPACE_CHARS)
            && activity
                .task
                .as_deref()
                .is_none_or(|value| value.chars().count() <= MAX_TASK_CHARS)
    }));

    let replacement = SystemAgentSnapshot {
        activities: Vec::new(),
        warnings: Vec::new(),
    };
    publisher
        .write_system_snapshot(&replacement, 5_678)
        .await
        .unwrap();
    let bytes = tokio::fs::read(&path).await.unwrap();
    let snapshot: SystemAgentProtocolSnapshot = serde_json::from_slice(&bytes).unwrap();
    assert!(!snapshot.degraded);
    assert!(snapshot.activities.is_empty());
    let leftovers = std::fs::read_dir(temp.path())
        .unwrap()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry
                .file_name()
                .to_string_lossy()
                .starts_with(".system-snapshot-")
        })
        .count();
    assert_eq!(leftovers, 0);
}

#[tokio::test]
async fn close_cannot_race_a_waiting_write_into_recreating_the_heartbeat() {
    let temp = tempfile::tempdir().unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let local = presence(
        publisher.instance_id(),
        std::process::id(),
        AgentActivityState::Working,
    );
    let lease = publisher.write_lock.lock().await;
    let writer_publisher = publisher.clone();
    let writer = tokio::spawn(async move { writer_publisher.write_presence(&local).await });
    tokio::task::yield_now().await;
    let remover_publisher = publisher.clone();
    let remover = tokio::spawn(async move { remover_publisher.remove().await });
    while !publisher.closed.load(Ordering::Acquire) {
        tokio::task::yield_now().await;
    }
    drop(lease);

    assert!(writer.await.unwrap().is_err());
    remover.await.unwrap();
    assert!(!publisher.path.as_ref().unwrap().exists());
}

#[test]
fn agent_island_preference_defaults_on_and_round_trips() {
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("agent-presence");

    assert!(preference::is_enabled(Some(&directory)));
    preference::set_enabled(Some(&directory), false).unwrap();
    assert!(!preference::is_enabled(Some(&directory)));
    assert!(directory
        .join(preference::AGENT_ISLAND_DISABLED_FILE)
        .is_file());
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;

        let marker = std::fs::metadata(directory.join(preference::AGENT_ISLAND_DISABLED_FILE))
            .unwrap()
            .permissions()
            .mode()
            & 0o777;
        assert_eq!(marker, 0o600);
        let directory_mode = std::fs::metadata(&directory).unwrap().permissions().mode() & 0o777;
        assert_eq!(directory_mode, 0o700);
    }

    let disabled_publisher = AgentPresencePublisher::for_directory(directory.clone());
    assert!(!disabled_publisher.island_preference_enabled());

    preference::set_enabled(Some(&directory), true).unwrap();
    assert!(preference::is_enabled(Some(&directory)));
    let enabled_publisher = AgentPresencePublisher::for_directory(directory);
    assert!(enabled_publisher.island_preference_enabled());
}

#[tokio::test]
async fn island_opt_out_does_not_disable_the_shared_presence_protocol() {
    let temp = tempfile::tempdir().unwrap();
    preference::set_enabled(Some(temp.path()), false).unwrap();
    let publisher = AgentPresencePublisher::for_directory(temp.path().to_path_buf());
    let local = presence(
        publisher.instance_id(),
        std::process::id(),
        AgentActivityState::Working,
    );

    publisher.write_presence(&local).await.unwrap();
    assert!(publisher.path.as_ref().unwrap().is_file());
    assert!(!publisher.island_preference_enabled());
}

#[test]
fn terminal_text_sanitizer_removes_controls_and_bounds_unicode() {
    let hostile = format!(
        "first\n\u{1b}[2Jhidden\u{1b}]52;c;Y2xpcGJvYXJk\u{7} \u{9b}31mred \u{202e}实现 e\u{301} {}",
        "x".repeat(MAX_TASK_CHARS + 20)
    );
    let value = sanitize_display_text(&hostile, MAX_TASK_CHARS);

    assert!(!value.chars().any(char::is_control), "{value:?}");
    assert!(!value.contains("[2J"), "{value:?}");
    assert!(!value.contains("]52"), "{value:?}");
    assert!(!value.contains('\u{202e}'), "{value:?}");
    assert!(value.contains("实现"), "{value:?}");
    assert!(value.contains("e\u{301}"), "{value:?}");
    assert!(value.chars().count() <= MAX_TASK_CHARS);
}
