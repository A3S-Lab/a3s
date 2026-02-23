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
use crate::state::ServiceState;
use crate::watcher::spawn_watcher;

#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    StateChanged { service: String, state: String },
    HealthChange { service: String, healthy: bool },
    LogLine { service: String, line: String },
}

struct ServiceHandle {
    child: Child,
    state: ServiceState,
    color_idx: usize,
}

pub struct Supervisor {
    config: Arc<DevConfig>,
    handles: Arc<RwLock<HashMap<String, ServiceHandle>>>,
    events: broadcast::Sender<SupervisorEvent>,
    log: Arc<LogAggregator>,
}

impl Supervisor {
    pub fn new(config: Arc<DevConfig>) -> (Self, broadcast::Receiver<SupervisorEvent>) {
        let (events, rx) = broadcast::channel(4096);
        let (log, log_rx) = LogAggregator::new();
        tokio::spawn(LogAggregator::print_loop(log_rx));
        (
            Self {
                config,
                handles: Arc::new(RwLock::new(HashMap::new())),
                events,
                log: Arc::new(log),
            },
            rx,
        )
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

        let mut parts = svc.cmd.split_whitespace();
        let program = parts.next().unwrap_or("sh");
        let args: Vec<&str> = parts.collect();

        let mut cmd = Command::new(program);
        cmd.args(&args)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .envs(&svc.env);

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
            },
        );

        self.emit(SupervisorEvent::StateChanged {
            service: name.to_string(),
            state: "running".into(),
        });

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

                        let mut parts = svc_def.cmd.split_whitespace();
                        let program = parts.next().unwrap_or("sh");
                        let args: Vec<&str> = parts.collect();

                        let mut cmd = Command::new(program);
                        cmd.args(&args)
                            .stdout(std::process::Stdio::piped())
                            .stderr(std::process::Stdio::piped())
                            .envs(&svc_def.env);
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
                let state = map
                    .get(name)
                    .map(|h| h.state.label().to_string())
                    .unwrap_or_else(|| "pending".into());
                let pid = map.get(name).and_then(|h| h.state.pid());
                StatusRow {
                    name: name.clone(),
                    state,
                    pid,
                    port: svc.port,
                    subdomain: svc.subdomain.clone(),
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
                    }
                }
            });
        }
    }

    fn emit(&self, event: SupervisorEvent) {
        let _ = self.events.send(event);
    }
}
