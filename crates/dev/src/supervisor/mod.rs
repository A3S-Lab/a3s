use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::process::Child;
use tokio::sync::{broadcast, RwLock};

use crate::config::DevConfig;
use crate::error::{DevError, Result};
use crate::graph::DependencyGraph;
use crate::health::HealthChecker;
use crate::ipc::StatusRow;
use crate::log::LogAggregator;
use crate::proxy::ProxyRouter;
use crate::state::ServiceState;
use crate::watcher::spawn_watcher;

use spawn::{free_port, spawn_process, SpawnSpec};

pub mod ipc;
mod spawn;

#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    #[allow(dead_code)]
    StateChanged { service: String, state: String },
    #[allow(dead_code)]
    HealthChange { service: String, healthy: bool },
    #[allow(dead_code)]
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
    pub fn new(
        config: Arc<DevConfig>,
        proxy: Arc<ProxyRouter>,
    ) -> (Self, broadcast::Receiver<SupervisorEvent>) {
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

    pub fn subscribe_logs(&self) -> broadcast::Receiver<crate::log::LogLine> {
        self.log.subscribe()
    }

    /// Subscribe to raw supervisor events (used by IPC server for log streaming).
    pub fn subscribe_events(&self) -> broadcast::Receiver<SupervisorEvent> {
        self.events.subscribe()
    }

    pub fn log_history(&self, service: Option<&str>, lines: usize) -> Vec<crate::log::LogLine> {
        self.log.recent(service, lines)
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
            free_port()
                .ok_or_else(|| DevError::Config(format!("[{name}] no free port available")))?
        } else {
            svc.port
        };

        // Register proxy route now that the real port is known
        if let Some(sub) = &svc.subdomain {
            self.proxy.update(sub.clone(), port).await;
            tracing::info!("[{name}] starting on :{port} → http://{sub}.localhost");
        } else {
            tracing::info!("[{name}] starting on :{port}");
        }

        let spec = SpawnSpec {
            name,
            svc: &svc,
            port,
            color_idx,
        };
        let result = spawn_process(&spec, &self.log).await?;

        self.handles.write().await.insert(
            name.to_string(),
            ServiceHandle {
                child: result.child,
                state: ServiceState::Running {
                    pid: result.pid,
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
        self.spawn_crash_recovery(name.to_string(), color_idx);

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
                self.spawn_file_watcher(name.to_string(), watch.paths.clone(), watch.ignore.clone());
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
            #[cfg(unix)]
            {
                use nix::sys::signal::{kill, Signal};
                use nix::unistd::Pid;
                if let Some(pid) = h.state.pid() {
                    let _ = kill(Pid::from_raw(pid as i32), Signal::SIGTERM);
                    let _ = tokio::time::timeout(
                        std::time::Duration::from_secs(5),
                        h.child.wait(),
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

    fn emit(&self, event: SupervisorEvent) {
        let _ = self.events.send(event);
    }

    /// Spawn a task that monitors the process and auto-restarts on unexpected exit.
    fn spawn_crash_recovery(&self, svc_name: String, color_idx: usize) {
        let handles = self.handles.clone();
        let events = self.events.clone();
        let config = self.config.clone();
        let log = self.log.clone();
        let proxy = self.proxy.clone();

        tokio::spawn(async move {
            let mut backoff_secs = 1u64;
            loop {
                // Wait for the process to exit
                let exit_status = {
                    let mut map = handles.write().await;
                    if let Some(h) = map.get_mut(&svc_name) {
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
                    match map.get(&svc_name) {
                        Some(h) if matches!(h.state, ServiceState::Stopped) => break,
                        None => break,
                        _ => {}
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
                {
                    backoff_secs = (backoff_secs * 2).min(30);
                }

                let svc_def = match config.service.get(&svc_name) {
                    Some(s) => s.clone(),
                    None => break,
                };
                let port = handles
                    .read()
                    .await
                    .get(&svc_name)
                    .map(|h| h.port)
                    .unwrap_or(svc_def.port);

                let spec = SpawnSpec {
                    name: &svc_name,
                    svc: &svc_def,
                    port,
                    color_idx,
                };
                match spawn_process(&spec, &log).await {
                    Ok(result) => {
                        if let Some(sub) = &svc_def.subdomain {
                            proxy.update(sub.clone(), port).await;
                        }
                        handles.write().await.insert(
                            svc_name.clone(),
                            ServiceHandle {
                                child: result.child,
                                state: ServiceState::Running {
                                    pid: result.pid,
                                    since: Instant::now(),
                                },
                                color_idx,
                                port,
                            },
                        );
                        let _ = events.send(SupervisorEvent::StateChanged {
                            service: svc_name.clone(),
                            state: "running".into(),
                        });
                        backoff_secs = 1;
                    }
                    Err(e) => {
                        tracing::error!("[{svc_name}] restart failed: {e}");
                        break;
                    }
                }
            }
        });
    }

    /// Spawn a task that watches files and restarts the service on change.
    fn spawn_file_watcher(
        &self,
        svc_name: String,
        paths: Vec<std::path::PathBuf>,
        ignore: Vec<String>,
    ) {
        let handles = self.handles.clone();
        let events = self.events.clone();
        let config = self.config.clone();
        let log = self.log.clone();

        let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(8);
        spawn_watcher(svc_name.clone(), paths, ignore, tx);

        tokio::spawn(async move {
            while let Some(changed_svc) = rx.recv().await {
                tracing::info!("[{changed_svc}] file change — restarting");

                let (color_idx, port) = {
                    let mut map = handles.write().await;
                    if let Some(h) = map.get_mut(&changed_svc) {
                        let idx = h.color_idx;
                        let p = h.port;
                        let _ = h.child.kill().await;
                        h.state = ServiceState::Stopped;
                        (idx, p)
                    } else {
                        continue;
                    }
                };

                let _ = events.send(SupervisorEvent::StateChanged {
                    service: changed_svc.clone(),
                    state: "restarting".into(),
                });

                let svc_def = match config.service.get(&changed_svc) {
                    Some(s) => s.clone(),
                    None => continue,
                };

                let spec = SpawnSpec {
                    name: &changed_svc,
                    svc: &svc_def,
                    port,
                    color_idx,
                };
                match spawn_process(&spec, &log).await {
                    Ok(result) => {
                        handles.write().await.insert(
                            changed_svc.clone(),
                            ServiceHandle {
                                child: result.child,
                                state: ServiceState::Running {
                                    pid: result.pid,
                                    since: Instant::now(),
                                },
                                color_idx,
                                port,
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
