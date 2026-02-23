use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::process::{Child, Command};
use tokio::sync::{broadcast, RwLock};

use crate::config::DevConfig;
use crate::error::{DevError, Result};
use crate::graph::DependencyGraph;
use crate::health::HealthChecker;
use crate::ipc::{socket_path, IpcRequest, IpcResponse, StatusRow};
use crate::log::LogAggregator;
use crate::proxy::ProxyRouter;
use crate::state::ServiceState;
use crate::watcher::spawn_watcher;

#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    #[allow(dead_code)]
    StateChanged { service: String, state: String },
    #[allow(dead_code)]
    HealthChange { service: String, healthy: bool },
    LogLine { service: String, line: String },
}


struct ServiceHandle {
    child: Child,
    state: ServiceState,
    color_idx: usize,
    port: u16,
}

pub struct Supervisor {
    config: Arc<DevConfig>,
    handles: Arc<RwLock<HashMap<String, ServiceHandle>>>,
    events: broadcast::Sender<SupervisorEvent>,
    log: Arc<LogAggregator>,
    proxy: Arc<ProxyRouter>,
}

impl Supervisor {
    pub fn new(config: Arc<DevConfig>, proxy: Arc<ProxyRouter>) -> (Self, broadcast::Receiver<SupervisorEvent>) {
        let (events, rx) = broadcast::channel(4096);
        let (log, log_rx) = LogAggregator::new();
        let log = Arc::new(log);
        tokio::spawn(LogAggregator::print_loop(log_rx));
        LogAggregator::spawn_history_recorder(log.clone());
        (
            Self {
                config,
                handles: Arc::new(RwLock::new(HashMap::new())),
                events,
                log,
                proxy,
            },
            rx,
        )
    }

    pub fn subscribe_logs(&self) -> tokio::sync::broadcast::Receiver<crate::log::LogLine> {
        self.log.subscribe()
    }

    pub async fn start_all(&self) -> Result<()> {
        let graph = DependencyGraph::from_config(&self.config)?;
        let names: Vec<String> = graph.start_order().to_vec();
        for (idx, name) in names.iter().enumerate() {
            self.start_service(name, idx).await?;
        }
        Ok(())
    }

    pub async fn start_service(&self, name: &str, color_idx: usize) -> Result<()> {
        let svc = self
            .config
            .service
            .get(name)
            .ok_or_else(|| DevError::UnknownService(name.to_string()))?
            .clone();

        self.emit(SupervisorEvent::StateChanged {
            service: name.to_string(),
            state: "starting".into(),
        });

        // Resolve port: 0 = auto-assign a free port (portless-style)
        let port = if svc.port == 0 {
            free_port().ok_or_else(|| DevError::Config(format!("[{name}] no free port available")))?
        } else {
            svc.port
        };

        // Register/update proxy route now that the real port is known
        if let Some(sub) = &svc.subdomain {
            self.proxy.update(sub.clone(), port).await;
        }

        let parts = split_cmd(&svc.cmd);
        let program = parts.first().map(|s| s.as_str()).unwrap_or("sh");
        let args = &parts[1..];

        // Framework-aware port injection
        let extra_args = framework_port_args(&parts, port);

        let mut cmd = Command::new(program);
        cmd.args(args)
            .args(&extra_args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(&svc.env)
            // Always inject PORT + HOST so any framework can pick them up
            .env("PORT", port.to_string())
            .env("HOST", "127.0.0.1");

        if let Some(dir) = &svc.dir {
            cmd.current_dir(dir);
        }

        let mut child = cmd.spawn().map_err(|e| DevError::Process {
            service: name.to_string(),
            msg: e.to_string(),
        })?;

        let pid = child.id().unwrap_or(0);

        // Attach stdout
        if let Some(stdout) = child.stdout.take() {
            self.log.attach(name.to_string(), color_idx, stdout);
        }

        // Attach stderr — prefix with dim style
        if let Some(stderr) = child.stderr.take() {
            let events = self.events.clone();
            let svc_name = name.to_string();
            tokio::spawn(async move {
                let mut lines = BufReader::new(stderr).lines();
                while let Ok(Some(line)) = lines.next_line().await {
                    let _ = events.send(SupervisorEvent::LogLine {
                        service: svc_name.clone(),
                        line,
                    });
                }
            });
        }

        self.handles.write().await.insert(
            name.to_string(),
            ServiceHandle {
                child,
                state: ServiceState::Running {
                    pid,
                    since: Instant::now(),
                },
                color_idx,
                port,
            },
        );

        self.emit(SupervisorEvent::StateChanged {
            service: name.to_string(),
            state: "running".into(),
        });

        // Crash recovery — monitor process and auto-restart on unexpected exit
        {
            let handles = self.handles.clone();
            let events = self.events.clone();
            let config = self.config.clone();
            let log = self.log.clone();
            let proxy = self.proxy.clone();
            let svc_name = name.to_string();

            tokio::spawn(async move {
                let mut backoff_secs = 1u64;
                loop {
                    // Wait for the process to exit
                    let exit_status = {
                        let mut map = handles.write().await;
                        if let Some(h) = map.get_mut(&svc_name) {
                            // Only monitor if still running
                            if !matches!(h.state, ServiceState::Running { .. }) {
                                break;
                            }
                            h.child.wait().await.ok()
                        } else {
                            break;
                        }
                    };

                    // Check if we were intentionally stopped
                    {
                        let map = handles.read().await;
                        if let Some(h) = map.get(&svc_name) {
                            if matches!(h.state, ServiceState::Stopped) {
                                break;
                            }
                        } else {
                            break;
                        }
                    }

                    let code = exit_status.and_then(|s| s.code());
                    tracing::warn!(
                        "[{svc_name}] exited (code={}) — restarting in {backoff_secs}s",
                        code.map(|c| c.to_string()).unwrap_or_else(|| "?".into())
                    );

                    let _ = events.send(SupervisorEvent::StateChanged {
                        service: svc_name.clone(),
                        state: "restarting".into(),
                    });

                    tokio::time::sleep(std::time::Duration::from_secs(backoff_secs)).await;
                    #[allow(unused_assignments)]
                    { backoff_secs = (backoff_secs * 2).min(30); }

                    // Respawn
                    let svc_def = match config.service.get(&svc_name) {
                        Some(s) => s.clone(),
                        None => break,
                    };

                    let parts = split_cmd(&svc_def.cmd);
                    let program = parts.first().map(|s| s.as_str()).unwrap_or("sh");
                    let args = parts[1..].to_vec();

                    // Reuse the same port that was originally assigned
                    let port = handles
                        .read()
                        .await
                        .get(&svc_name)
                        .map(|h| h.port)
                        .unwrap_or(svc_def.port);
                    let extra_args = framework_port_args(&parts, port);

                    let mut cmd = tokio::process::Command::new(program);
                    cmd.args(&args)
                        .args(&extra_args)
                        .stdout(std::process::Stdio::piped())
                        .stderr(std::process::Stdio::piped())
                        .envs(&svc_def.env)
                        .env("PORT", port.to_string())
                        .env("HOST", "127.0.0.1");
                    if let Some(dir) = &svc_def.dir {
                        cmd.current_dir(dir);
                    }

                    match cmd.spawn() {
                        Ok(mut child) => {
                            let pid = child.id().unwrap_or(0);
                            let idx = handles
                                .read()
                                .await
                                .get(&svc_name)
                                .map(|h| h.color_idx)
                                .unwrap_or(0);
                            if let Some(stdout) = child.stdout.take() {
                                log.attach(svc_name.clone(), idx, stdout);
                            }
                            // Re-register proxy route (port unchanged, but ensures route is live)
                            if let Some(sub) = &svc_def.subdomain {
                                proxy.update(sub.clone(), port).await;
                            }
                            handles.write().await.insert(
                                svc_name.clone(),
                                ServiceHandle {
                                    child,
                                    state: ServiceState::Running {
                                        pid,
                                        since: Instant::now(),
                                    },
                                    color_idx: idx,
                                    port,
                                },
                            );
                            let _ = events.send(SupervisorEvent::StateChanged {
                                service: svc_name.clone(),
                                state: "running".into(),
                            });
                            backoff_secs = 1; // reset on successful start
                        }
                        Err(e) => {
                            tracing::error!("[{svc_name}] restart failed: {e}");
                            break;
                        }
                    }
                }
            });
        }

        // Wait for health before unblocking dependents
        if let Some(checker) = HealthChecker::for_service(&svc) {
            let healthy = checker.wait_healthy(&svc).await;
            self.emit(SupervisorEvent::HealthChange {
                service: name.to_string(),
                healthy,
            });
            if !healthy {
                tracing::warn!(
                    "[{name}] health check failed after {} retries",
                    checker.config.retries
                );
            }
        }

        // File watcher → auto-restart on change
        if let Some(watch) = &svc.watch {
            if watch.restart {
                let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(8);
                spawn_watcher(
                    name.to_string(),
                    watch.paths.clone(),
                    watch.ignore.clone(),
                    tx,
                );

                let handles = self.handles.clone();
                let events = self.events.clone();
                let config = self.config.clone();
                let log = self.log.clone();
                let svc_name = name.to_string();

                tokio::spawn(async move {
                    while let Some(changed_svc) = rx.recv().await {
                        tracing::info!("[{changed_svc}] file change — restarting");

                        // Kill old process
                        {
                            let mut map = handles.write().await;
                            if let Some(h) = map.get_mut(&changed_svc) {
                                let _ = h.child.kill().await;
                                h.state = ServiceState::Stopped;
                            }
                        }

                        let _ = events.send(SupervisorEvent::StateChanged {
                            service: svc_name.clone(),
                            state: "restarting".into(),
                        });

                        // Respawn
                        let svc_def = match config.service.get(&changed_svc) {
                            Some(s) => s.clone(),
                            None => continue,
                        };

                        // Reuse the same port that was originally assigned
                        let existing_port = handles
                            .read()
                            .await
                            .get(&changed_svc)
                            .map(|h| h.port)
                            .unwrap_or(svc_def.port);

                        let parts = split_cmd(&svc_def.cmd);
                        let program = parts.first().map(|s| s.as_str()).unwrap_or("sh");
                        let args = parts[1..].to_vec();
                        let extra_args = framework_port_args(&parts, existing_port);

                        let mut cmd = Command::new(program);
                        cmd.args(&args)
                            .args(&extra_args)
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .envs(&svc_def.env)
                            .env("PORT", existing_port.to_string())
                            .env("HOST", "127.0.0.1");
                        if let Some(dir) = &svc_def.dir {
                            cmd.current_dir(dir);
                        }

                        match cmd.spawn() {
                            Ok(mut child) => {
                                let pid = child.id().unwrap_or(0);
                                let idx = handles
                                    .read()
                                    .await
                                    .get(&changed_svc)
                                    .map(|h| h.color_idx)
                                    .unwrap_or(0);
                                if let Some(stdout) = child.stdout.take() {
                                    log.attach(changed_svc.clone(), idx, stdout);
                                }
                                handles.write().await.insert(
                                    changed_svc.clone(),
                                    ServiceHandle {
                                        child,
                                        state: ServiceState::Running {
                                            pid,
                                            since: Instant::now(),
                                        },
                                        color_idx: idx,
                                        port: existing_port,
                                    },
                                );
                                let _ = events.send(SupervisorEvent::StateChanged {
                                    service: changed_svc.clone(),
                                    state: "running".into(),
                                });
                            }
                            Err(e) => {
                                tracing::error!("[{changed_svc}] restart failed: {e}");
                            }
                        }
                    }
                });
            }
        }

        Ok(())
    }

    pub async fn stop_all(&self) {
        let graph = match DependencyGraph::from_config(&self.config) {
            Ok(g) => g,
            Err(_) => return,
        };
        let names: Vec<String> = graph.stop_order().map(|s| s.to_string()).collect();
        for name in &names {
            self.stop_service(name).await;
        }
    }

    pub async fn stop_service(&self, name: &str) {
        let mut map = self.handles.write().await;
        if let Some(h) = map.get_mut(name) {
            // Graceful SIGTERM, then SIGKILL after 5s
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                if let Some(pid) = h.state.pid() {
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                    let child = &mut h.child;
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        child.wait(),
                    )
                    .await;
                }
            }
            let _ = h.child.kill().await;
            h.state = ServiceState::Stopped;
            self.emit(SupervisorEvent::StateChanged {
                service: name.to_string(),
                state: "stopped".into(),
            });
        }
    }

    pub async fn restart_service(&self, name: &str) -> Result<()> {
        let color_idx = self
            .handles
            .read()
            .await
            .get(name)
            .map(|h| h.color_idx)
            .unwrap_or(0);
        self.stop_service(name).await;
        self.start_service(name, color_idx).await
    }

    pub async fn status_rows(&self) -> Vec<StatusRow> {
        let map = self.handles.read().await;
        self.config
            .service
            .iter()
            .map(|(name, svc)| {
                let handle = map.get(name);
                let state = handle
                    .map(|h| h.state.label().to_string())
                    .unwrap_or_else(|| "pending".into());
                let pid = handle.and_then(|h| h.state.pid());
                let uptime_secs = handle.and_then(|h| {
                    if let ServiceState::Running { since, .. } = h.state {
                        Some(since.elapsed().as_secs())
                    } else {
                        None
                    }
                });
                StatusRow {
                    name: name.clone(),
                    state,
                    pid,
                    port: handle.map(|h| h.port).unwrap_or(svc.port),
                    subdomain: svc.subdomain.clone(),
                    uptime_secs,
                }
            })
            .collect()
    }

    /// Start the Unix socket IPC server. Handles status/stop/restart/logs requests.
    pub async fn serve_ipc(self: Arc<Self>) {
        let path = socket_path();
        let _ = std::fs::remove_file(&path);

        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(e) => {
                tracing::error!("IPC socket bind failed: {e}");
                return;
            }
        };

        tracing::debug!("IPC socket at {}", path.display());

        loop {
            let (stream, _) = match listener.accept().await {
                Ok(s) => s,
                Err(e) => {
                    tracing::warn!("IPC accept error: {e}");
                    continue;
                }
            };

            let sup = self.clone();
            tokio::spawn(async move {
                let (reader, mut writer) = tokio::io::split(stream);
                let mut lines = BufReader::new(reader).lines();

                while let Ok(Some(line)) = lines.next_line().await {
                    let req: IpcRequest = match serde_json::from_str(&line) {
                        Ok(r) => r,
                        Err(e) => {
                            let resp = IpcResponse::Error {
                                msg: format!("bad request: {e}"),
                            };
                            let _ = writer
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&resp).unwrap())
                                        .as_bytes(),
                                )
                                .await;
                            continue;
                        }
                    };

                    match req {
                        IpcRequest::Status => {
                            let rows = sup.status_rows().await;
                            let resp = IpcResponse::Status { rows };
                            let _ = writer
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&resp).unwrap())
                                        .as_bytes(),
                                )
                                .await;
                        }

                        IpcRequest::Stop { services } => {
                            if services.is_empty() {
                                sup.stop_all().await;
                            } else {
                                for name in &services {
                                    sup.stop_service(name).await;
                                }
                            }
                            let resp = IpcResponse::Ok;
                            let _ = writer
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&resp).unwrap())
                                        .as_bytes(),
                                )
                                .await;
                        }

                        IpcRequest::Restart { service } => {
                            let resp = match sup.restart_service(&service).await {
                                Ok(_) => IpcResponse::Ok,
                                Err(e) => IpcResponse::Error { msg: e.to_string() },
                            };
                            let _ = writer
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&resp).unwrap())
                                        .as_bytes(),
                                )
                                .await;
                        }

                        IpcRequest::Logs { service, follow } => {
                            let mut rx = sup.events.subscribe();
                            loop {
                                match rx.recv().await {
                                    Ok(SupervisorEvent::LogLine {
                                        service: svc,
                                        line,
                                    }) => {
                                        if service.as_deref().map_or(true, |f| f == svc) {
                                            let resp = IpcResponse::LogLine {
                                                service: svc,
                                                line,
                                            };
                                            if writer
                                                .write_all(
                                                    format!(
                                                        "{}\n",
                                                        serde_json::to_string(&resp).unwrap()
                                                    )
                                                    .as_bytes(),
                                                )
                                                .await
                                                .is_err()
                                            {
                                                break;
                                            }
                                        }
                                    }
                                    Ok(_) => {}
                                    Err(broadcast::error::RecvError::Closed) => break,
                                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                                }
                                if !follow {
                                    break;
                                }
                            }
                        }

                        IpcRequest::History { service, lines } => {
                            let recent = sup.log.recent(service.as_deref(), lines);
                            for entry in recent {
                                let resp = IpcResponse::LogLine {
                                    service: entry.service,
                                    line: entry.line,
                                };
                                if writer
                                    .write_all(
                                        format!(
                                            "{}\n",
                                            serde_json::to_string(&resp).unwrap()
                                        )
                                        .as_bytes(),
                                    )
                                    .await
                                    .is_err()
                                {
                                    break;
                                }
                            }
                            // Close connection after sending all history lines
                            break;
                        }
                    }
                }
            });
        }
    }

    fn emit(&self, event: SupervisorEvent) {
        let _ = self.events.send(event);
    }
}

/// Bind to port 0 and return the OS-assigned free port.
fn free_port() -> Option<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    Some(listener.local_addr().ok()?.port())
}

/// Shell-style command splitting: handles single/double quotes and backslash escapes.
/// e.g. `node server.js --title 'hello world'` → ["node", "server.js", "--title", "hello world"]
fn split_cmd(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = cmd.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if !in_single => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// Detect framework from the command and inject `--port <port>` if needed.
/// Some frameworks ignore PORT env var and require an explicit CLI flag.
/// `parts` is the full split command (program + args).
fn framework_port_args(parts: &[String], port: u16) -> Vec<String> {
    let p = port.to_string();
    // Direct framework binaries
    let direct = ["vite", "next", "astro", "nuxt", "remix", "svelte-kit", "wrangler"];
    // Package runners that delegate to a framework as the second token
    let runners = ["npx", "pnpm", "yarn", "bunx"];

    let program = parts.first().map(|s| s.as_str()).unwrap_or("");
    let second = parts.get(1).map(|s| s.as_str()).unwrap_or("");

    let framework = if direct.contains(&program) {
        program
    } else if runners.contains(&program) {
        // e.g. `npx vite`, `pnpm exec next`, `yarn vite`
        // skip "exec"/"run"/"dlx" shims and look at the next meaningful token
        if second == "exec" || second == "run" || second == "dlx" {
            parts.get(2).map(|s| s.as_str()).unwrap_or("")
        } else {
            second
        }
    } else {
        ""
    };

    if direct.contains(&framework) {
        vec!["--port".into(), p]
    } else {
        vec![]
    }
}
