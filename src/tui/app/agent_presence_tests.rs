use super::*;

#[test]
fn task_and_plan_start_project_non_idle_parent_lifecycle() {
    assert_eq!(
        parent_presence_state(State::Streaming, false, None),
        AgentActivityState::Working
    );
    assert_eq!(
        parent_presence_state(State::Streaming, true, None),
        AgentActivityState::Planning
    );
    assert_eq!(
        parent_presence_state(State::Awaiting, false, None),
        AgentActivityState::WaitingApproval
    );
    assert_eq!(
        parent_presence_state(State::Rebuilding, false, None),
        AgentActivityState::Working
    );
}

#[test]
fn idle_parent_uses_only_a_recent_terminal_lifecycle() {
    assert_eq!(
        parent_presence_state(State::Idle, false, None),
        AgentActivityState::Idle
    );
    assert_eq!(
        parent_presence_state(State::Idle, false, Some(AgentActivityState::Completed)),
        AgentActivityState::Completed
    );
}

#[test]
fn parent_terminal_state_uses_the_same_eight_second_retention() {
    let now = Instant::now();
    let recent = RecentTerminalState {
        session_id: "session-a".to_string(),
        state: AgentActivityState::Completed,
        task: Some("task".to_string()),
        started_at_ms: Some(10),
        finished_at_ms: 20,
        recorded_at: now.checked_sub(Duration::from_secs(7)).unwrap(),
    };
    assert!(recent.is_visible_at("session-a", now));
    assert!(!recent.is_visible_at("session-b", now));
    let expired = RecentTerminalState {
        recorded_at: now
            .checked_sub(TERMINAL_STATE_RETENTION + Duration::from_millis(1))
            .unwrap(),
        ..recent
    };

    assert!(!expired.is_visible_at("session-a", now));
}

#[test]
fn failed_parent_terminal_state_is_retained_for_export() {
    let temp = tempfile::tempdir().unwrap();
    let mut runtime = AgentPresenceRuntime {
        publisher: AgentPresencePublisher::for_directory(temp.path().to_path_buf()),
        refreshing: false,
        webview_binary: None,
        terminal: None,
        island: AgentIslandSupervisor::default(),
        cancel_requested: std::collections::HashSet::new(),
        last_warnings: Vec::new(),
    };

    runtime.record_terminal(
        "session-a".to_string(),
        AgentActivityState::Failed,
        Some("failed task".to_string()),
        Some(10),
    );

    let terminal = runtime.recent_terminal("session-a").unwrap();
    assert_eq!(terminal.state, AgentActivityState::Failed);
    assert_eq!(terminal.task.as_deref(), Some("failed task"));
}

#[test]
fn completed_child_presence_expires_after_terminal_retention() {
    let now = Instant::now();
    let old_end = now
        .checked_sub(TERMINAL_STATE_RETENTION + Duration::from_millis(1))
        .unwrap();
    let old_start = old_end.checked_sub(Duration::from_secs(2)).unwrap();
    let mut runtime = RuntimeProjection::default();
    runtime.start_subagent(
        "child".to_string(),
        "review".to_string(),
        "finished review".to_string(),
        old_start,
    );
    runtime.end_subagent(
        "child".to_string(),
        "review".to_string(),
        "done".to_string(),
        true,
        old_end,
    );
    let recent_end = now.checked_sub(Duration::from_secs(1)).unwrap();
    let recent_start = recent_end.checked_sub(Duration::from_secs(2)).unwrap();
    runtime.start_subagent(
        "recent".to_string(),
        "review".to_string(),
        "recent review".to_string(),
        recent_start,
    );
    runtime.end_subagent(
        "recent".to_string(),
        "review".to_string(),
        "done".to_string(),
        true,
        recent_end,
    );

    let expired = child_presence(
        "child".to_string(),
        runtime.subagents()[0],
        AgentVendor::Other,
        now,
        epoch_ms(),
    );
    let recent = child_presence(
        "recent".to_string(),
        runtime.subagents()[1],
        AgentVendor::Other,
        now,
        epoch_ms(),
    )
    .unwrap();

    assert!(expired.is_none());
    assert_eq!(recent.state, AgentActivityState::Completed);
}

#[test]
fn island_launch_gate_defaults_to_desktop_only_and_allows_explicit_enablement() {
    let automatic_desktop = AgentIslandEnvironment {
        display_available: true,
        linux: true,
        ..AgentIslandEnvironment::default()
    };
    assert_eq!(automatic_desktop.skip_reason(), None);

    let headless = AgentIslandEnvironment {
        linux: true,
        ..AgentIslandEnvironment::default()
    };
    assert_eq!(
        headless.skip_reason(),
        Some("headless Linux session without a display server")
    );

    let ssh = AgentIslandEnvironment {
        ssh: true,
        display_available: true,
        ..AgentIslandEnvironment::default()
    };
    assert_eq!(
        ssh.skip_reason(),
        Some("SSH session without explicit island enablement")
    );

    for setting in ["1", "true", "ON"] {
        let explicit = AgentIslandEnvironment {
            setting: Some(OsString::from(setting)),
            ssh: true,
            linux: true,
            ..AgentIslandEnvironment::default()
        };
        assert_eq!(explicit.skip_reason(), None, "setting {setting}");
    }
}

#[test]
fn island_launch_gate_honors_explicit_disablement() {
    for setting in ["0", "false", "OFF"] {
        let environment = AgentIslandEnvironment {
            setting: Some(OsString::from(setting)),
            display_available: true,
            ..AgentIslandEnvironment::default()
        };
        assert_eq!(
            environment.skip_reason(),
            Some("disabled by A3S_AGENT_ISLAND"),
            "setting {setting}"
        );
    }
}

#[test]
fn island_preference_command_accepts_status_on_and_off_only() {
    assert_eq!(
        preference::parse_agent_island_preference_command(""),
        Ok(preference::AgentIslandPreferenceCommand::Status)
    );
    assert_eq!(
        preference::parse_agent_island_preference_command(" status "),
        Ok(preference::AgentIslandPreferenceCommand::Status)
    );
    assert_eq!(
        preference::parse_agent_island_preference_command("ON"),
        Ok(preference::AgentIslandPreferenceCommand::Enable)
    );
    assert_eq!(
        preference::parse_agent_island_preference_command("off"),
        Ok(preference::AgentIslandPreferenceCommand::Disable)
    );
    assert!(preference::parse_agent_island_preference_command("toggle").is_err());
}

#[tokio::test]
async fn disabling_supervisor_stops_a_running_helper_and_blocks_launches() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    let (_exit_tx, exit) = oneshot::channel();
    let (shutdown, shutdown_rx) = oneshot::channel();
    let mut supervisor = AgentIslandSupervisor {
        enabled: true,
        lifecycle: AgentIslandLifecycle::Running(AgentIslandMonitor {
            exit,
            started_at: Instant::now(),
            shutdown: Some(shutdown),
        }),
        request: Some(request.clone()),
        consecutive_failures: 0,
    };

    supervisor.set_enabled(false);
    tokio::time::timeout(Duration::from_secs(1), shutdown_rx)
        .await
        .expect("helper stop signal timed out")
        .expect("helper stop sender was dropped");
    assert!(!supervisor.enabled);
    assert!(supervisor.request.is_none());
    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Stopped
    ));
    assert!(supervisor.observe_snapshot(request.clone(), true).is_none());

    supervisor.set_enabled(true);
    assert_eq!(
        supervisor.observe_snapshot(request.clone(), true),
        Some(request)
    );
}

#[tokio::test]
async fn helper_spawned_after_disable_is_stopped_as_a_stale_launch() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    let mut supervisor = AgentIslandSupervisor::default();
    assert!(supervisor.observe_snapshot(request, true).is_some());
    supervisor.set_enabled(false);

    let (_exit_tx, exit) = oneshot::channel();
    let (shutdown, shutdown_rx) = oneshot::channel();
    supervisor.apply_launch_result(
        Ok(AgentIslandLaunchOutcome::Spawned(AgentIslandMonitor {
            exit,
            started_at: Instant::now(),
            shutdown: Some(shutdown),
        })),
        Instant::now(),
    );

    tokio::time::timeout(Duration::from_secs(1), shutdown_rx)
        .await
        .expect("stale helper stop signal timed out")
        .expect("stale helper stop sender was dropped");
    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Stopped
    ));
}

#[test]
fn island_helper_arguments_match_the_native_mode_contract() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };

    assert_eq!(
        agent_island_args(&request),
        vec![
            OsString::from("--agent-island"),
            OsString::from("--snapshot"),
            OsString::from("/private/state/system-snapshot.json"),
            OsString::from("--lock-file"),
            OsString::from("/private/state/island.lock"),
        ]
    );
}

#[test]
fn island_binary_override_is_honored_without_silent_fallback() {
    let override_path = PathBuf::from("/explicit/a3s-webview");
    let environment = AgentIslandEnvironment {
        binary_override: Some(override_path.clone()),
        ..AgentIslandEnvironment::default()
    };

    let (explicit, candidates) = resolve_agent_island_binaries(&environment, None).unwrap();
    assert!(explicit);
    assert_eq!(candidates, vec![override_path]);
}

#[test]
fn successful_export_starts_one_launch_while_the_first_is_in_flight() {
    let temp = tempfile::tempdir().unwrap();
    let webview_binary = temp.path().join("a3s-webview");
    let mut runtime = AgentPresenceRuntime {
        publisher: AgentPresencePublisher::for_directory(temp.path().to_path_buf()),
        refreshing: true,
        webview_binary: Some(webview_binary.clone()),
        terminal: None,
        island: AgentIslandSupervisor::default(),
        cancel_requested: std::collections::HashSet::new(),
        last_warnings: Vec::new(),
    };
    let result = SystemAgentRefreshResult {
        snapshot_path: Some(temp.path().join("system-snapshot.json")),
        lock_path: Some(temp.path().join("island.lock")),
        launch_requested: true,
        control_requests: Vec::new(),
        warnings: Vec::new(),
    };

    let request = runtime.apply_refresh(result.clone()).unwrap();
    assert_eq!(request.binary.as_deref(), Some(webview_binary.as_path()));
    assert!(!runtime.refreshing);
    assert!(runtime.apply_refresh(result).is_none());
    assert!(matches!(
        runtime.island.lifecycle,
        AgentIslandLifecycle::Launching
    ));
}

#[test]
fn idle_export_does_not_start_the_native_island() {
    let temp = tempfile::tempdir().unwrap();
    let mut runtime = AgentPresenceRuntime {
        publisher: AgentPresencePublisher::for_directory(temp.path().to_path_buf()),
        refreshing: true,
        webview_binary: None,
        terminal: None,
        island: AgentIslandSupervisor::default(),
        cancel_requested: std::collections::HashSet::new(),
        last_warnings: Vec::new(),
    };

    assert!(runtime
        .apply_refresh(SystemAgentRefreshResult {
            snapshot_path: Some(temp.path().join("system-snapshot.json")),
            lock_path: Some(temp.path().join("island.lock")),
            launch_requested: false,
            control_requests: Vec::new(),
            warnings: Vec::new(),
        })
        .is_none());
    assert!(!runtime.refreshing);
    assert!(matches!(
        runtime.island.lifecycle,
        AgentIslandLifecycle::AwaitingSnapshot
    ));
    assert!(runtime.island.request.is_none());
}

#[test]
fn failed_export_leaves_the_island_waiting_for_a_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let mut runtime = AgentPresenceRuntime {
        publisher: AgentPresencePublisher::for_directory(temp.path().to_path_buf()),
        refreshing: true,
        webview_binary: None,
        terminal: None,
        island: AgentIslandSupervisor::default(),
        cancel_requested: std::collections::HashSet::new(),
        last_warnings: Vec::new(),
    };

    assert!(runtime
        .apply_refresh(SystemAgentRefreshResult {
            warnings: vec!["snapshot: unavailable".to_string()],
            ..SystemAgentRefreshResult::default()
        })
        .is_none());
    assert!(matches!(
        runtime.island.lifecycle,
        AgentIslandLifecycle::AwaitingSnapshot
    ));
}

#[test]
fn retry_backoff_is_tick_driven_and_cools_down_after_four_consecutive_failures() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    let mut supervisor = AgentIslandSupervisor::default();
    assert_eq!(
        supervisor.observe_snapshot(request.clone(), true),
        Some(request.clone())
    );

    let mut now = Instant::now();
    for failure in 1..AGENT_ISLAND_MAX_CONSECUTIVE_FAILURES {
        supervisor.apply_launch_result(Err(format!("failure {failure}")), now);
        let delay = agent_island_retry_delay(failure);
        assert!(supervisor
            .poll(now + delay.saturating_sub(Duration::from_millis(1)))
            .is_none());
        now += delay;
        assert_eq!(supervisor.poll(now), Some(request.clone()));
    }

    supervisor.apply_launch_result(Err("final failure".to_string()), now);
    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Backoff { .. }
    ));
    assert_eq!(supervisor.consecutive_failures, 0);
    assert!(supervisor
        .poll(now + AGENT_ISLAND_RECOVERY_RECHECK - Duration::from_millis(1))
        .is_none());
    assert_eq!(
        supervisor.poll(now + AGENT_ISLAND_RECOVERY_RECHECK),
        Some(request)
    );
}

#[test]
fn obsolete_helper_rechecks_after_a_bounded_recovery_cooldown() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    let mut supervisor = AgentIslandSupervisor::default();
    assert!(supervisor.observe_snapshot(request, true).is_some());

    let now = Instant::now();
    supervisor.apply_launch_result(
        Ok(AgentIslandLaunchOutcome::Unsupported(
            "RemoteUI-only helper".to_string(),
        )),
        now,
    );

    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Backoff { .. }
    ));
    assert_eq!(supervisor.consecutive_failures, 0);
    assert!(supervisor
        .poll(now + AGENT_ISLAND_RECOVERY_RECHECK - Duration::from_millis(1))
        .is_none());
    assert!(supervisor
        .poll(now + AGENT_ISLAND_RECOVERY_RECHECK)
        .is_some());
}

#[test]
fn idle_snapshot_cancels_a_pending_helper_retry() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    let mut supervisor = AgentIslandSupervisor::default();
    assert!(supervisor.observe_snapshot(request.clone(), true).is_some());
    let now = Instant::now();
    supervisor.apply_launch_result(Err("launch failed".to_string()), now);
    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Backoff { .. }
    ));

    assert!(supervisor.observe_snapshot(request, false).is_none());
    assert!(supervisor.request.is_none());
    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::AwaitingSnapshot
    ));
    assert!(supervisor
        .poll(now + AGENT_ISLAND_RECOVERY_RECHECK)
        .is_none());
}

#[test]
fn singleton_contention_rechecks_infrequently_for_eventual_takeover() {
    let mut supervisor = AgentIslandSupervisor::default();
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    supervisor.request = Some(request.clone());
    let now = Instant::now();

    supervisor.apply_exit(
        AgentIslandExit {
            success: true,
            status: "exit status: 0".to_string(),
            detail: String::new(),
            ran_for: Duration::from_millis(100),
        },
        now,
    );

    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Backoff { .. }
    ));
    assert_eq!(supervisor.consecutive_failures, 0);
    assert!(supervisor
        .poll(now + AGENT_ISLAND_CONTENTION_RECHECK - Duration::from_millis(1))
        .is_none());
    assert_eq!(
        supervisor.poll(now + AGENT_ISLAND_CONTENTION_RECHECK),
        Some(request)
    );
}

#[test]
fn watchdog_length_success_is_relaunched_with_bounded_backoff() {
    let request = AgentIslandLaunchRequest {
        snapshot_path: PathBuf::from("/private/state/system-snapshot.json"),
        lock_path: PathBuf::from("/private/state/island.lock"),
        binary: None,
    };
    let mut supervisor = AgentIslandSupervisor {
        request: Some(request.clone()),
        ..AgentIslandSupervisor::default()
    };
    let now = Instant::now();

    supervisor.apply_exit(
        AgentIslandExit {
            success: true,
            status: "exit status: 0".to_string(),
            detail: String::new(),
            ran_for: Duration::from_secs(20),
        },
        now,
    );

    assert!(matches!(
        supervisor.lifecycle,
        AgentIslandLifecycle::Backoff { .. }
    ));
    assert_eq!(supervisor.consecutive_failures, 1);
    assert_eq!(
        supervisor.poll(now + agent_island_retry_delay(1)),
        Some(request)
    );
}

#[tokio::test]
async fn helper_monitor_reaps_a_failed_child_and_reports_its_exit() {
    let _process_probe_guard = crate::system_agents::agent_island_process_test_lock()
        .lock()
        .await;
    let child = tokio::process::Command::new(std::env::current_exe().unwrap())
        .arg("--definitely-not-a-valid-libtest-option")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut monitor = monitor_agent_island_child(child);

    let exit = tokio::time::timeout(Duration::from_secs(10), async {
        loop {
            if let Some(exit) = monitor.try_take_exit() {
                return exit;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("child monitor did not report the reaped process");

    assert!(!exit.success);
    assert!(!exit.status.is_empty());
}

#[cfg(unix)]
#[tokio::test]
async fn helper_monitor_stops_a_live_child_on_user_opt_out() {
    let child = tokio::process::Command::new("sleep")
        .arg("30")
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let mut monitor = monitor_agent_island_child(child);
    monitor.stop();

    let exit = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            if let Some(exit) = monitor.try_take_exit() {
                return exit;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("disabled helper was not terminated");

    assert!(!exit.success);
}

#[tokio::test]
async fn bounded_reader_retains_the_prefix_while_draining_to_eof() {
    use tokio::io::AsyncWriteExt;

    let payload = vec![b'x'; 64 * 1024];
    let (mut writer, reader) = tokio::io::duplex(16);
    let write = tokio::spawn(async move {
        writer.write_all(&payload).await.unwrap();
    });

    let retained = tokio::time::timeout(Duration::from_secs(1), read_bounded(Some(reader), 17))
        .await
        .expect("bounded reader stopped draining after reaching its retention limit");
    write.await.unwrap();

    assert_eq!(retained, vec![b'x'; 17]);
}

#[cfg(unix)]
#[tokio::test]
async fn incompatible_preferred_helper_falls_back_to_a_compatible_candidate() {
    use std::os::unix::fs::PermissionsExt;

    let _process_probe_guard = crate::system_agents::agent_island_process_test_lock()
        .lock()
        .await;
    let temp = tempfile::tempdir().unwrap();
    let stale = temp.path().join("stale-webview");
    let compatible = temp.path().join("compatible-webview");
    std::fs::write(
        &stale,
        "#!/bin/sh\nprintf '%s\\n' 'usage: a3s-webview --url <http(s)://...>' >&2\nexit 2\n",
    )
    .unwrap();
    std::fs::write(
        &compatible,
        "#!/bin/sh\nprintf '%s\\n' 'usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>' >&2\nexit 2\n",
    )
    .unwrap();
    for helper in [&stale, &compatible] {
        std::fs::set_permissions(helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    let (selected, rejected) =
        find_compatible_agent_island_binary(vec![stale.clone(), compatible.clone()], temp.path())
            .await;

    assert_eq!(
        selected,
        Some(compatible.canonicalize().unwrap()),
        "rejected candidates: {rejected:?}"
    );
    assert_eq!(rejected.len(), 1);
    assert!(rejected[0].contains(&stale.display().to_string()));
}

#[cfg(unix)]
#[tokio::test]
async fn helper_runs_from_the_private_snapshot_directory() {
    use std::os::unix::fs::PermissionsExt;

    let _process_probe_guard = crate::system_agents::agent_island_process_test_lock()
        .lock()
        .await;
    let temp = tempfile::tempdir().unwrap();
    let state = temp.path().join("private-state");
    std::fs::create_dir(&state).unwrap();
    let helper = temp.path().join("a3s-webview-test");
    std::fs::write(
        &helper,
        r#"#!/bin/sh
if [ "$2" = "--help" ]; then
  pwd > observed-probe-cwd
  printf '%s\n' 'usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>' >&2
  exit 2
fi
pwd > observed-cwd
"#,
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();
    let request = AgentIslandLaunchRequest {
        snapshot_path: state.join("system-snapshot.json"),
        lock_path: state.join("island.lock"),
        binary: None,
    };
    let environment = AgentIslandEnvironment {
        binary_override: Some(helper),
        ..AgentIslandEnvironment::default()
    };

    let outcome = launch_agent_island_with_environment(request, environment)
        .await
        .unwrap();
    let AgentIslandLaunchOutcome::Spawned(mut monitor) = outcome else {
        panic!("compatible helper should be spawned, got {outcome:?}");
    };
    tokio::time::timeout(Duration::from_secs(3), async {
        while monitor.try_take_exit().is_none() {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("test helper did not exit");

    let expected = state.canonicalize().unwrap();
    for observation in ["observed-probe-cwd", "observed-cwd"] {
        let observed = std::fs::read_to_string(state.join(observation)).unwrap();
        assert_eq!(Path::new(observed.trim()).canonicalize().unwrap(), expected);
    }
}

#[cfg(unix)]
struct ProbeDescendantCleanup {
    pid: Option<libc::pid_t>,
}

#[cfg(unix)]
impl ProbeDescendantCleanup {
    async fn from_pid_file(pid_file: &Path) -> Self {
        let deadline = Instant::now() + Duration::from_secs(1);
        loop {
            match std::fs::read_to_string(pid_file) {
                Ok(pid) => {
                    return Self {
                        pid: Some(pid.trim().parse().unwrap()),
                    };
                }
                Err(error)
                    if error.kind() == std::io::ErrorKind::NotFound
                        && Instant::now() < deadline =>
                {
                    tokio::time::sleep(Duration::from_millis(5)).await;
                }
                Err(error) => {
                    panic!(
                        "failed to read probe descendant pid from {}: {error}",
                        pid_file.display()
                    );
                }
            }
        }
    }

    fn is_running(&self) -> bool {
        let Some(pid) = self.pid else {
            return false;
        };
        // SAFETY: signal 0 performs existence/permission checking only.
        if unsafe { libc::kill(pid, 0) } == 0 {
            return true;
        }
        std::io::Error::last_os_error().raw_os_error() != Some(libc::ESRCH)
    }

    async fn assert_terminated(mut self) {
        let deadline = Instant::now() + Duration::from_secs(2);
        while self.is_running() && Instant::now() < deadline {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
        if self.is_running() {
            let pid = self.pid.unwrap();
            // Ensure a failed containment assertion cannot leak the test child.
            // SAFETY: this guard owns the pid recorded by the test helper.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
            self.pid = None;
            panic!("capability-probe descendant {pid} was still running");
        }
        self.pid = None;
    }
}

#[cfg(unix)]
impl Drop for ProbeDescendantCleanup {
    fn drop(&mut self) {
        if let Some(pid) = self.pid.take() {
            // SAFETY: this guard owns the pid recorded by the test helper.
            unsafe {
                libc::kill(pid, libc::SIGKILL);
            }
        }
    }
}

#[cfg(unix)]
#[tokio::test]
async fn capability_probe_reaps_a_descendant_that_holds_output_open() {
    use std::os::unix::fs::PermissionsExt;

    let _process_probe_guard = crate::system_agents::agent_island_process_test_lock()
        .lock()
        .await;
    let temp = tempfile::tempdir().unwrap();
    let helper = temp.path().join("a3s-webview-probe-test");
    std::fs::write(
        &helper,
        r#"#!/bin/sh
sleep 30 &
printf '%s\n' "$!" > descendant.pid
printf '%s\n' 'usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>' >&2
exit 0
"#,
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        probe_agent_island_capability(&helper, temp.path()),
    )
    .await;
    let descendant =
        ProbeDescendantCleanup::from_pid_file(&temp.path().join("descendant.pid")).await;
    let supported = result
        .expect("descendant-held output pipes exceeded the probe bound")
        .unwrap();

    assert!(supported);
    descendant.assert_terminated().await;
}

#[cfg(unix)]
#[tokio::test]
async fn capability_probe_timeout_terminates_its_descendant() {
    use std::os::unix::fs::PermissionsExt;

    let _process_probe_guard = crate::system_agents::agent_island_process_test_lock()
        .lock()
        .await;
    let temp = tempfile::tempdir().unwrap();
    let helper = temp.path().join("a3s-webview-probe-timeout-test");
    std::fs::write(
        &helper,
        r#"#!/bin/sh
sleep 30 &
printf '%s\n' "$!" > descendant.pid
sleep 30
"#,
    )
    .unwrap();
    std::fs::set_permissions(&helper, std::fs::Permissions::from_mode(0o755)).unwrap();

    let result = tokio::time::timeout(
        Duration::from_secs(5),
        probe_agent_island_capability(&helper, temp.path()),
    )
    .await;
    let descendant =
        ProbeDescendantCleanup::from_pid_file(&temp.path().join("descendant.pid")).await;
    let error = result
        .expect("hanging helper exceeded the probe's termination bound")
        .unwrap_err();

    assert_eq!(error.kind(), std::io::ErrorKind::TimedOut);
    descendant.assert_terminated().await;
}
