//! Presentation-neutral coding-agent heartbeat and native island bridge.
//!
//! The TUI owns exact lifecycle projection and periodically publishes it. The
//! OS-level `a3s-webview --agent-island` process owns all island rendering and
//! reads only the private, sanitized shared snapshot.

#[path = "agent_presence/control.rs"]
mod control;
#[path = "agent_presence/preference.rs"]
mod preference;
#[path = "agent_presence/projection.rs"]
mod projection;

use super::*;
use crate::system_agents::{
    epoch_ms, AgentActivityState, AgentChildPresence, AgentControlActionKind,
    AgentControlGrantSpec, AgentControlRequest, AgentPresence, AgentPresencePublisher, AgentVendor,
    SystemAgentRefreshResult,
};
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};
use tokio::sync::oneshot;

use self::projection::{child_presence, parent_presence_state};

pub(super) const AGENT_PRESENCE_REFRESH_INTERVAL: Duration = Duration::from_secs(2);
const TERMINAL_STATE_RETENTION: Duration = Duration::from_secs(8);
const AGENT_ISLAND_ENV: &str = "A3S_AGENT_ISLAND";
const AGENT_ISLAND_BIN_ENV: &str = "A3S_AGENT_ISLAND_BIN";
const MAX_AGENT_ISLAND_PROBE_OUTPUT_BYTES: u64 = 8 * 1024;
const AGENT_ISLAND_PROBE_TIMEOUT: Duration = Duration::from_secs(2);
const AGENT_ISLAND_STDERR_DRAIN_TIMEOUT: Duration = Duration::from_secs(1);
const AGENT_ISLAND_SINGLETON_EXIT_MAX: Duration = Duration::from_secs(5);
const AGENT_ISLAND_CONTENTION_RECHECK: Duration = Duration::from_secs(30);
const AGENT_ISLAND_RECOVERY_RECHECK: Duration = Duration::from_secs(60);
const AGENT_ISLAND_STABLE_RUNTIME: Duration = Duration::from_secs(30);
const AGENT_ISLAND_MAX_CONSECUTIVE_FAILURES: u8 = 4;

#[derive(Debug)]
struct RecentTerminalState {
    session_id: String,
    state: AgentActivityState,
    task: Option<String>,
    started_at_ms: Option<u64>,
    finished_at_ms: u64,
    recorded_at: Instant,
}

impl RecentTerminalState {
    fn is_visible_at(&self, session_id: &str, now: Instant) -> bool {
        self.session_id == session_id
            && now.saturating_duration_since(self.recorded_at) <= TERMINAL_STATE_RETENTION
    }
}

#[derive(Debug)]
pub(super) struct AgentPresenceRuntime {
    pub(super) publisher: AgentPresencePublisher,
    pub(super) refreshing: bool,
    webview_binary: Option<PathBuf>,
    terminal: Option<RecentTerminalState>,
    island: AgentIslandSupervisor,
    cancel_requested: HashSet<String>,
    last_warnings: Vec<String>,
}

impl AgentPresenceRuntime {
    pub(super) fn new(webview_binary: Option<PathBuf>) -> Self {
        let publisher = AgentPresencePublisher::from_environment();
        let mut island = AgentIslandSupervisor::default();
        island.set_enabled(publisher.island_preference_enabled());
        Self {
            publisher,
            refreshing: false,
            webview_binary,
            terminal: None,
            island,
            cancel_requested: HashSet::new(),
            last_warnings: Vec::new(),
        }
    }

    pub(super) fn webview_binary(&self) -> Option<&Path> {
        self.webview_binary.as_deref()
    }

    fn recent_terminal(&self, session_id: &str) -> Option<&RecentTerminalState> {
        let now = Instant::now();
        self.terminal
            .as_ref()
            .filter(|terminal| terminal.is_visible_at(session_id, now))
    }

    pub(super) fn record_terminal(
        &mut self,
        session_id: String,
        state: AgentActivityState,
        task: Option<String>,
        started_at_ms: Option<u64>,
    ) {
        self.terminal = Some(RecentTerminalState {
            session_id,
            state,
            task,
            started_at_ms,
            finished_at_ms: epoch_ms(),
            recorded_at: Instant::now(),
        });
    }

    fn apply_refresh(
        &mut self,
        result: SystemAgentRefreshResult,
    ) -> Option<AgentIslandLaunchRequest> {
        self.refreshing = false;
        if result.warnings != self.last_warnings {
            for warning in &result.warnings {
                tracing::warn!(%warning, "system-agent presence refresh degraded");
            }
            self.last_warnings = result.warnings;
        }
        let (Some(snapshot_path), Some(lock_path)) = (result.snapshot_path, result.lock_path)
        else {
            return None;
        };
        self.island.observe_snapshot(
            AgentIslandLaunchRequest {
                snapshot_path,
                lock_path,
                binary: self.webview_binary.clone(),
            },
            result.launch_requested,
        )
    }

    fn poll_island(&mut self, now: Instant) -> Option<AgentIslandLaunchRequest> {
        self.island.poll(now)
    }

    fn apply_island_launch_result(
        &mut self,
        result: Result<AgentIslandLaunchOutcome, String>,
        now: Instant,
    ) {
        self.island.apply_launch_result(result, now);
    }
}

impl App {
    pub(super) fn local_agent_presence(&mut self) -> AgentPresence {
        let now_ms = epoch_ms();
        let now = Instant::now();
        let parent_vendor = self.local_agent_vendor();
        let mut children = self
            .runtime
            .subagent_ids()
            .into_iter()
            .zip(self.runtime.subagents())
            .filter_map(|(id, child)| child_presence(id, child, parent_vendor, now, now_ms))
            .collect::<Vec<_>>();
        let live_child_ids = children
            .iter()
            .filter(|child| child.state == AgentActivityState::Working)
            .map(|child| child.id.clone())
            .collect::<HashSet<_>>();
        self.agent_presence
            .cancel_requested
            .retain(|task_id| live_child_ids.contains(task_id));

        let recent_terminal = self.agent_presence.recent_terminal(&self.session_id);
        let terminal_state = recent_terminal.map(|terminal| terminal.state);
        let terminal_task = recent_terminal.and_then(|terminal| terminal.task.clone());
        let terminal_started_at_ms = recent_terminal.and_then(|terminal| terminal.started_at_ms);
        let terminal_finished_at_ms = recent_terminal.map(|terminal| terminal.finished_at_ms);
        let state = parent_presence_state(
            self.state,
            self.plan
                .tasks()
                .iter()
                .any(|task| task.status == a3s_code_core::planning::TaskStatus::InProgress),
            terminal_state,
        );
        let task = self.running_task.clone().or(terminal_task);
        let started_at_ms = self
            .stream_started
            .map(|started| {
                now_ms
                    .saturating_sub(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
            })
            .or(terminal_started_at_ms)
            .unwrap_or_else(|| self.agent_presence.publisher.started_at_ms());
        let finished_at_ms = (self.state == State::Idle)
            .then_some(terminal_finished_at_ms)
            .flatten();

        let parent_id = self.agent_presence.publisher.instance_id().to_string();
        let attention_reason = (self.state == State::Awaiting)
            .then(|| {
                self.pending_tools
                    .front()
                    .map(|pending| format!("Permission required to run {}.", pending.label.trim()))
            })
            .flatten();
        let mut control_specs = Vec::new();
        if self.state == State::Awaiting {
            if let Some(pending) = self.pending_tools.front() {
                control_specs.push(AgentControlGrantSpec::new(
                    parent_id.clone(),
                    self.agent_island_approval_context(&pending.tool_id),
                    [
                        AgentControlActionKind::ApproveOnce,
                        AgentControlActionKind::ApproveAlways,
                        AgentControlActionKind::Deny,
                        AgentControlActionKind::Reply,
                    ],
                ));
            }
        } else if self.state == State::Streaming && !self.interrupting {
            let mut actions = vec![AgentControlActionKind::Reply];
            if self.agent_island_stop_available() {
                actions.insert(0, AgentControlActionKind::Stop);
            }
            control_specs.push(AgentControlGrantSpec::new(
                parent_id.clone(),
                self.agent_island_parent_context(),
                actions,
            ));
        }
        for child in &children {
            if child.state == AgentActivityState::Working
                && !self.agent_presence.cancel_requested.contains(&child.id)
            {
                control_specs.push(AgentControlGrantSpec::new(
                    format!("{parent_id}:{}", child.id),
                    self.agent_island_child_context(&child.id),
                    [AgentControlActionKind::Cancel],
                ));
            }
        }
        let mut controls = self
            .agent_presence
            .publisher
            .reconcile_control_grants(control_specs, now_ms);
        let parent_actions = controls.remove(&parent_id).unwrap_or_default();
        for child in &mut children {
            child.actions = controls
                .remove(&format!("{parent_id}:{}", child.id))
                .unwrap_or_default();
        }

        AgentPresence::new(
            &parent_id,
            std::process::id(),
            &self.cwd,
            task,
            state,
            children,
            started_at_ms,
        )
        .with_vendor(parent_vendor)
        .with_attention_reason(attention_reason)
        .with_finished_at_ms(finished_at_ms)
        .with_actions(parent_actions)
    }

    pub(super) fn refresh_agent_presence(&mut self) -> Cmd<Msg> {
        self.agent_presence.refreshing = true;
        let local = self.local_agent_presence();
        let publisher = self.agent_presence.publisher.clone();
        cmd::cmd(move || async move {
            Msg::AgentPresenceRefreshed(publisher.publish_collect_and_export(local).await)
        })
    }

    pub(super) fn apply_agent_presence_refresh(
        &mut self,
        mut result: SystemAgentRefreshResult,
    ) -> Option<Cmd<Msg>> {
        let requests = std::mem::take(&mut result.control_requests);
        let mut commands = self
            .agent_presence
            .apply_refresh(result)
            .map(launch_agent_island)
            .into_iter()
            .collect::<Vec<_>>();
        commands.extend(
            requests
                .into_iter()
                .map(|request| cmd::msg(Msg::AgentIslandControl(request))),
        );
        match commands.len() {
            0 => None,
            1 => commands.pop(),
            _ => Some(cmd::batch(commands)),
        }
    }

    pub(super) fn poll_agent_island(&mut self) -> Option<Cmd<Msg>> {
        self.agent_presence
            .poll_island(Instant::now())
            .map(launch_agent_island)
    }

    pub(super) fn apply_agent_island_launch_result(
        &mut self,
        result: Result<AgentIslandLaunchOutcome, String>,
    ) {
        self.agent_presence
            .apply_island_launch_result(result, Instant::now());
    }

    pub(super) fn record_local_agent_terminal(&mut self, state: AgentActivityState) {
        let started_at_ms = self.stream_started.map(|started| {
            epoch_ms()
                .saturating_sub(started.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
        });
        self.agent_presence.record_terminal(
            self.session_id.clone(),
            state,
            self.running_task.clone(),
            started_at_ms,
        );
    }
}

pub(super) fn agent_presence_tick() -> Cmd<Msg> {
    cmd::tick(AGENT_PRESENCE_REFRESH_INTERVAL, Msg::AgentPresenceTick)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AgentIslandLaunchRequest {
    snapshot_path: PathBuf,
    lock_path: PathBuf,
    binary: Option<PathBuf>,
}

#[derive(Debug)]
pub(super) enum AgentIslandLaunchOutcome {
    Spawned(AgentIslandMonitor),
    Skipped(&'static str),
    Unsupported(String),
}

#[derive(Debug)]
enum AgentIslandLifecycle {
    AwaitingSnapshot,
    Launching,
    Running(AgentIslandMonitor),
    Backoff { retry_at: Instant },
    Stopped,
}

#[derive(Debug)]
struct AgentIslandSupervisor {
    enabled: bool,
    lifecycle: AgentIslandLifecycle,
    request: Option<AgentIslandLaunchRequest>,
    consecutive_failures: u8,
}

impl Default for AgentIslandSupervisor {
    fn default() -> Self {
        Self {
            enabled: true,
            lifecycle: AgentIslandLifecycle::AwaitingSnapshot,
            request: None,
            consecutive_failures: 0,
        }
    }
}

impl AgentIslandSupervisor {
    fn observe_snapshot(
        &mut self,
        request: AgentIslandLaunchRequest,
        launch_requested: bool,
    ) -> Option<AgentIslandLaunchRequest> {
        if !self.enabled {
            self.request = None;
            return None;
        }
        if !launch_requested {
            self.request = None;
            if matches!(self.lifecycle, AgentIslandLifecycle::Backoff { .. }) {
                self.lifecycle = AgentIslandLifecycle::AwaitingSnapshot;
                self.consecutive_failures = 0;
            }
            return None;
        }
        self.request = Some(request);
        if !matches!(self.lifecycle, AgentIslandLifecycle::AwaitingSnapshot) {
            return None;
        }
        self.lifecycle = AgentIslandLifecycle::Launching;
        self.request.clone()
    }

    fn poll(&mut self, now: Instant) -> Option<AgentIslandLaunchRequest> {
        if !self.enabled {
            return None;
        }
        let exit = match &mut self.lifecycle {
            AgentIslandLifecycle::Running(monitor) => monitor.try_take_exit(),
            _ => None,
        };
        if let Some(exit) = exit {
            self.apply_exit(exit, now);
        }

        let retry_due = matches!(
            self.lifecycle,
            AgentIslandLifecycle::Backoff { retry_at } if now >= retry_at
        );
        if retry_due {
            let Some(request) = self.request.clone() else {
                self.lifecycle = AgentIslandLifecycle::AwaitingSnapshot;
                return None;
            };
            self.lifecycle = AgentIslandLifecycle::Launching;
            return Some(request);
        }
        None
    }

    fn apply_launch_result(
        &mut self,
        result: Result<AgentIslandLaunchOutcome, String>,
        now: Instant,
    ) {
        if !self.enabled || !matches!(self.lifecycle, AgentIslandLifecycle::Launching) {
            if let Ok(AgentIslandLaunchOutcome::Spawned(mut monitor)) = result {
                monitor.stop();
            }
            tracing::debug!("ignoring stale native system-agent island launch result");
            return;
        }
        if self.request.is_none() {
            self.consecutive_failures = 0;
            if let Ok(AgentIslandLaunchOutcome::Spawned(mut monitor)) = result {
                monitor.stop();
            }
            self.lifecycle = AgentIslandLifecycle::AwaitingSnapshot;
            return;
        }
        match result {
            Ok(AgentIslandLaunchOutcome::Spawned(monitor)) => {
                tracing::debug!("native system-agent island helper launched");
                self.lifecycle = AgentIslandLifecycle::Running(monitor);
            }
            Ok(AgentIslandLaunchOutcome::Skipped(reason)) => {
                tracing::debug!(reason, "native system-agent island helper skipped");
                self.lifecycle = AgentIslandLifecycle::Stopped;
            }
            Ok(AgentIslandLaunchOutcome::Unsupported(error)) => {
                tracing::warn!(%error, "native system-agent island helper is incompatible");
                self.schedule_recovery_recheck(now, &error);
            }
            Err(error) => {
                tracing::warn!(%error, "native system-agent island helper failed to launch");
                self.schedule_retry(now, Duration::ZERO, &error);
            }
        }
    }

    fn apply_exit(&mut self, exit: AgentIslandExit, now: Instant) {
        if self.request.is_none() {
            self.consecutive_failures = 0;
            self.lifecycle = AgentIslandLifecycle::AwaitingSnapshot;
            return;
        }
        if exit.success && exit.ran_for <= AGENT_ISLAND_SINGLETON_EXIT_MAX {
            // Lock contention is a successful, immediate exit: another TUI's
            // helper already owns the per-user island. Recheck infrequently so
            // a surviving TUI can take over after that owner disappears without
            // creating a process loop while it remains healthy.
            tracing::debug!(
                status = %exit.status,
                retry_after_ms = AGENT_ISLAND_CONTENTION_RECHECK.as_millis(),
                "native system-agent island helper observed singleton contention"
            );
            self.consecutive_failures = 0;
            self.lifecycle = AgentIslandLifecycle::Backoff {
                retry_at: now + AGENT_ISLAND_CONTENTION_RECHECK,
            };
            return;
        }

        let reason = if exit.success {
            // A helper that survived startup and later exited cleanly most
            // likely hit its stale-snapshot watchdog. Relaunch it with the same
            // bounded policy used for crashes so a transient exporter outage
            // does not permanently remove the island.
            if exit.ran_for >= AGENT_ISLAND_STABLE_RUNTIME {
                self.consecutive_failures = 0;
            }
            format!("{} after watchdog-length runtime", exit.status)
        } else if exit.detail.is_empty() {
            exit.status
        } else {
            format!("{}: {}", exit.status, exit.detail)
        };
        tracing::warn!(
            runtime_ms = exit.ran_for.as_millis(),
            %reason,
            "native system-agent island helper exited unexpectedly"
        );
        self.schedule_retry(
            now,
            if exit.success {
                Duration::ZERO
            } else {
                exit.ran_for
            },
            &reason,
        );
    }

    fn schedule_retry(&mut self, now: Instant, ran_for: Duration, reason: &str) {
        if ran_for >= AGENT_ISLAND_STABLE_RUNTIME {
            self.consecutive_failures = 0;
        }
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= AGENT_ISLAND_MAX_CONSECUTIVE_FAILURES {
            tracing::warn!(
                failures = self.consecutive_failures,
                %reason,
                retry_after_ms = AGENT_ISLAND_RECOVERY_RECHECK.as_millis(),
                "native system-agent island helper entering recovery cooldown"
            );
            self.consecutive_failures = 0;
            self.lifecycle = AgentIslandLifecycle::Backoff {
                retry_at: now + AGENT_ISLAND_RECOVERY_RECHECK,
            };
            return;
        }
        let delay = agent_island_retry_delay(self.consecutive_failures);
        tracing::debug!(
            failures = self.consecutive_failures,
            retry_after_ms = delay.as_millis(),
            "native system-agent island helper restart scheduled"
        );
        self.lifecycle = AgentIslandLifecycle::Backoff {
            retry_at: now + delay,
        };
    }

    fn schedule_recovery_recheck(&mut self, now: Instant, reason: &str) {
        tracing::debug!(
            %reason,
            retry_after_ms = AGENT_ISLAND_RECOVERY_RECHECK.as_millis(),
            "native system-agent island helper recovery recheck scheduled"
        );
        self.consecutive_failures = 0;
        self.lifecycle = AgentIslandLifecycle::Backoff {
            retry_at: now + AGENT_ISLAND_RECOVERY_RECHECK,
        };
    }
}

fn agent_island_retry_delay(consecutive_failures: u8) -> Duration {
    match consecutive_failures {
        0 | 1 => AGENT_PRESENCE_REFRESH_INTERVAL,
        2 => AGENT_PRESENCE_REFRESH_INTERVAL * 2,
        _ => AGENT_PRESENCE_REFRESH_INTERVAL * 4,
    }
}

#[derive(Debug)]
struct AgentIslandExit {
    success: bool,
    status: String,
    detail: String,
    ran_for: Duration,
}

#[derive(Debug)]
pub(super) struct AgentIslandMonitor {
    exit: oneshot::Receiver<AgentIslandExit>,
    started_at: Instant,
    shutdown: Option<oneshot::Sender<()>>,
}

impl AgentIslandMonitor {
    fn stop(&mut self) {
        if let Some(shutdown) = self.shutdown.take() {
            let _ = shutdown.send(());
        }
    }

    fn try_take_exit(&mut self) -> Option<AgentIslandExit> {
        match self.exit.try_recv() {
            Ok(exit) => Some(exit),
            Err(oneshot::error::TryRecvError::Empty) => None,
            Err(oneshot::error::TryRecvError::Closed) => Some(AgentIslandExit {
                success: false,
                status: "helper monitor stopped before reporting an exit".to_string(),
                detail: String::new(),
                ran_for: self.started_at.elapsed(),
            }),
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct AgentIslandEnvironment {
    setting: Option<OsString>,
    binary_override: Option<PathBuf>,
    ssh: bool,
    display_available: bool,
    linux: bool,
}

impl AgentIslandEnvironment {
    fn current() -> Self {
        Self {
            setting: std::env::var_os(AGENT_ISLAND_ENV),
            binary_override: std::env::var_os(AGENT_ISLAND_BIN_ENV)
                .filter(|value| !value.is_empty())
                .map(PathBuf::from),
            ssh: ["SSH_CONNECTION", "SSH_CLIENT", "SSH_TTY"]
                .into_iter()
                .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty())),
            display_available: ["WAYLAND_DISPLAY", "DISPLAY"]
                .into_iter()
                .any(|name| std::env::var_os(name).is_some_and(|value| !value.is_empty())),
            linux: cfg!(target_os = "linux"),
        }
    }

    fn explicitly_enabled(&self) -> bool {
        self.setting.as_deref().is_some_and(env_truthy)
    }

    fn disabled(&self) -> bool {
        self.setting.as_deref().is_some_and(env_falsey)
    }

    fn skip_reason(&self) -> Option<&'static str> {
        if self.disabled() {
            return Some("disabled by A3S_AGENT_ISLAND");
        }
        if self.explicitly_enabled() {
            return None;
        }
        if self.ssh {
            return Some("SSH session without explicit island enablement");
        }
        if self.linux && !self.display_available {
            return Some("headless Linux session without a display server");
        }
        None
    }
}

fn env_truthy(value: &OsStr) -> bool {
    value
        .to_str()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "on"))
}

fn env_falsey(value: &OsStr) -> bool {
    value
        .to_str()
        .is_some_and(|value| matches!(value.to_ascii_lowercase().as_str(), "0" | "false" | "off"))
}

fn agent_island_args(request: &AgentIslandLaunchRequest) -> Vec<OsString> {
    vec![
        OsString::from("--agent-island"),
        OsString::from("--snapshot"),
        request.snapshot_path.as_os_str().to_os_string(),
        OsString::from("--lock-file"),
        request.lock_path.as_os_str().to_os_string(),
    ]
}

fn resolve_agent_island_binaries(
    environment: &AgentIslandEnvironment,
    preferred: Option<&Path>,
) -> std::io::Result<(bool, Vec<PathBuf>)> {
    if let Some(binary) = &environment.binary_override {
        return Ok((true, vec![binary.clone()]));
    }
    let (explicit, mut candidates) = remote_ui::webview_helper_candidates();
    if explicit {
        return Ok((true, candidates));
    }
    if let Some(preferred) = preferred {
        let preferred = preferred.to_path_buf();
        candidates.retain(|candidate| candidate != &preferred);
        candidates.insert(0, preferred);
    }
    if candidates.is_empty() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "a3s-webview is missing; install it or set A3S_AGENT_ISLAND_BIN",
        ));
    }
    Ok((false, candidates))
}

async fn find_compatible_agent_island_binary(
    candidates: Vec<PathBuf>,
    working_directory: &Path,
) -> (Option<PathBuf>, Vec<String>) {
    let mut rejected = Vec::new();
    for binary in candidates {
        let binary = std::fs::canonicalize(&binary).unwrap_or(binary);
        match probe_agent_island_capability(&binary, working_directory).await {
            Ok(true) => return (Some(binary), rejected),
            Ok(false) => rejected.push(format!("{} is incompatible", binary.display())),
            Err(error) => rejected.push(format!("{}: {error}", binary.display())),
        }
    }
    (None, rejected)
}

fn launch_agent_island(request: AgentIslandLaunchRequest) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let result =
            launch_agent_island_with_environment(request, AgentIslandEnvironment::current())
                .await
                .map_err(|error| error.to_string());
        Msg::AgentIslandLaunchFinished(result)
    })
}

async fn launch_agent_island_with_environment(
    request: AgentIslandLaunchRequest,
    environment: AgentIslandEnvironment,
) -> std::io::Result<AgentIslandLaunchOutcome> {
    if let Some(reason) = environment.skip_reason() {
        return Ok(AgentIslandLaunchOutcome::Skipped(reason));
    }
    let working_directory = request.snapshot_path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "agent island snapshot has no private parent directory",
        )
    })?;
    let (explicit, candidates) =
        resolve_agent_island_binaries(&environment, request.binary.as_deref())?;
    let (binary, rejected) =
        find_compatible_agent_island_binary(candidates, working_directory).await;
    let Some(binary) = binary else {
        let source = if explicit {
            "configured native helper"
        } else {
            "installed native helpers"
        };
        return Ok(AgentIslandLaunchOutcome::Unsupported(format!(
            "{source} do not support --agent-island; update a3s-webview to a compatible release ({})",
            rejected.join("; ")
        )));
    };
    let mut command = tokio::process::Command::new(&binary);
    command
        .args(agent_island_args(&request))
        .current_dir(working_directory)
        .env("PWD", working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        // The singleton may outlive this TUI while other publishers remain.
        // A parent-owned pipe would become broken when this process exits and
        // could terminate the helper on a later diagnostic write.
        .stderr(Stdio::null());
    let child = command.spawn()?;
    Ok(AgentIslandLaunchOutcome::Spawned(
        monitor_agent_island_child(child),
    ))
}

fn monitor_agent_island_child(mut child: tokio::process::Child) -> AgentIslandMonitor {
    let started_at = Instant::now();
    let (exit_tx, exit) = oneshot::channel();
    let (shutdown, mut shutdown_rx) = oneshot::channel();
    tokio::spawn(async move {
        let status = tokio::select! {
            status = child.wait() => status,
            signal = &mut shutdown_rx => {
                if signal.is_ok() {
                    let _ = child.start_kill();
                }
                child.wait().await
            }
        };
        let ran_for = started_at.elapsed();
        let exit = match status {
            Ok(status) => AgentIslandExit {
                success: status.success(),
                status: status.to_string(),
                detail: String::new(),
                ran_for,
            },
            Err(error) => AgentIslandExit {
                success: false,
                status: format!("wait failed: {error}"),
                detail: String::new(),
                ran_for,
            },
        };
        let _ = exit_tx.send(exit);
    });
    AgentIslandMonitor {
        exit,
        started_at,
        shutdown: Some(shutdown),
    }
}

#[cfg(unix)]
fn configure_agent_island_probe(command: &mut tokio::process::Command) {
    use std::os::unix::process::CommandExt;

    command.as_std_mut().process_group(0);
}

#[cfg(unix)]
#[derive(Debug)]
struct AgentIslandProbeProcessGroup {
    process_group: libc::pid_t,
    terminated: bool,
}

#[cfg(unix)]
impl AgentIslandProbeProcessGroup {
    fn attach(child: &tokio::process::Child) -> std::io::Result<Self> {
        let process_group = child
            .id()
            .and_then(|pid| libc::pid_t::try_from(pid).ok())
            .ok_or_else(|| {
                std::io::Error::other("native helper probe has no valid process-group id")
            })?;
        Ok(Self {
            process_group,
            terminated: false,
        })
    }

    fn terminate(&mut self) {
        if self.terminated {
            return;
        }
        self.terminated = true;
        // SAFETY: the probe was spawned into a new process group whose id is
        // the direct child's pid. A negative pid targets that entire group,
        // including descendants that inherited the probe's output handles.
        unsafe {
            libc::kill(-self.process_group, libc::SIGKILL);
        }
    }
}

#[cfg(unix)]
impl Drop for AgentIslandProbeProcessGroup {
    fn drop(&mut self) {
        self.terminate();
    }
}

#[cfg(unix)]
async fn probe_agent_island_capability(
    binary: &Path,
    working_directory: &Path,
) -> std::io::Result<bool> {
    let mut command = tokio::process::Command::new(binary);
    command
        .args(["--agent-island", "--help"])
        .current_dir(working_directory)
        .env("PWD", working_directory)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    configure_agent_island_probe(&mut command);
    let mut child = command.spawn()?;
    let mut process_group = AgentIslandProbeProcessGroup::attach(&child)?;
    let stdout = child.stdout.take();
    let stderr = child.stderr.take();
    let mut stdout = tokio::spawn(read_bounded(stdout, MAX_AGENT_ISLAND_PROBE_OUTPUT_BYTES));
    let mut stderr = tokio::spawn(read_bounded(stderr, MAX_AGENT_ISLAND_PROBE_OUTPUT_BYTES));
    let started = Instant::now();

    match tokio::time::timeout(AGENT_ISLAND_PROBE_TIMEOUT, child.wait()).await {
        Ok(status) => {
            // Capability probes never need background work. Terminating the
            // group here also closes pipes inherited by a descendant after the
            // direct child has already exited.
            process_group.terminate();
            status?;
        }
        Err(_) => {
            process_group.terminate();
            let _ = child.start_kill();
            let _ = tokio::time::timeout(AGENT_ISLAND_STDERR_DRAIN_TIMEOUT, child.wait()).await;
            stdout.abort();
            stderr.abort();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "timed out probing {} for --agent-island support",
                    binary.display()
                ),
            ));
        }
    }
    let remaining = AGENT_ISLAND_PROBE_TIMEOUT.saturating_sub(started.elapsed());
    let outputs = tokio::time::timeout(remaining, async {
        let stdout = (&mut stdout).await.unwrap_or_default();
        let stderr = (&mut stderr).await.unwrap_or_default();
        (stdout, stderr)
    })
    .await;
    let (stdout, stderr) = match outputs {
        Ok(outputs) => outputs,
        Err(_) => {
            stdout.abort();
            stderr.abort();
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                format!(
                    "timed out reading {} --agent-island capability output",
                    binary.display()
                ),
            ));
        }
    };
    Ok(a3s::components::webview_supports_agent_island_output(
        &stdout, &stderr,
    ))
}

#[cfg(windows)]
async fn probe_agent_island_capability(
    binary: &Path,
    _working_directory: &Path,
) -> std::io::Result<bool> {
    // Windows capability checks use the same bounded PE/target/marker
    // validation as self-update. Avoiding execution removes the process-tree
    // escape entirely instead of relying on a racy post-spawn Job assignment.
    a3s::components::webview_binary_supports_agent_island(binary)
}

#[cfg(not(any(unix, windows)))]
async fn probe_agent_island_capability(
    binary: &Path,
    _working_directory: &Path,
) -> std::io::Result<bool> {
    Err(std::io::Error::new(
        std::io::ErrorKind::Unsupported,
        format!(
            "native helper capability probing is unsupported on this platform: {}",
            binary.display()
        ),
    ))
}

async fn read_bounded<R>(reader: Option<R>, limit: u64) -> Vec<u8>
where
    R: AsyncRead + Unpin,
{
    let Some(mut reader) = reader else {
        return Vec::new();
    };
    let limit = usize::try_from(limit).unwrap_or(usize::MAX);
    let mut bytes = Vec::with_capacity(limit.min(8 * 1024));
    let mut chunk = [0_u8; 4096];
    loop {
        let read = match reader.read(&mut chunk).await {
            Ok(0) | Err(_) => break,
            Ok(read) => read,
        };
        let retained = limit.saturating_sub(bytes.len()).min(read);
        bytes.extend_from_slice(&chunk[..retained]);
    }
    bytes
}

#[cfg(test)]
#[path = "agent_presence_tests.rs"]
mod tests;
