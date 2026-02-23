use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;

use tokio::process::{Child, Command};
use tokio::sync::{broadcast, RwLock};

use crate::config::DevConfig;
use crate::error::{DevError, Result};
use crate::graph::DependencyGraph;
use crate::health::HealthChecker;
use crate::log::LogAggregator;
use crate::state::ServiceState;
use crate::watcher::spawn_watcher;

#[derive(Debug, Clone)]
pub enum SupervisorEvent {
    StateChanged { service: String, state: String },
    HealthChange { service: String, healthy: bool },
}

struct ServiceHandle {
    child: Child,
    state: ServiceState,
}

pub struct Supervisor {
    config: Arc<DevConfig>,
    handles: Arc<RwLock<HashMap<String, ServiceHandle>>>,
    events: broadcast::Sender<SupervisorEvent>,
    log: Arc<LogAggregator>,
}

impl Supervisor {
    pub fn new(config: Arc<DevConfig>) -> (Self, broadcast::Receiver<SupervisorEvent>) {
        let (events, rx) = broadcast::channel(256);
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
        let svc = self.config.service.get(name).ok_or_else(|| DevError::UnknownService(name.to_string()))?;

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

        if let Some(stdout) = child.stdout.take() {
            self.log.attach(name.to_string(), color_idx, stdout);
        }

        let state = ServiceState::Running {
            pid,
            since: Instant::now(),
        };

        self.handles.write().await.insert(
            name.to_string(),
            ServiceHandle { child, state },
        );

        self.emit(SupervisorEvent::StateChanged {
            service: name.to_string(),
            state: "running".into(),
        });

        // Wait for health check before returning (unblocks dependents)
        if let Some(checker) = HealthChecker::for_service(svc) {
            let healthy = checker.wait_healthy(svc).await;
            self.emit(SupervisorEvent::HealthChange {
                service: name.to_string(),
                healthy,
            });
            if !healthy {
                tracing::warn!("[{name}] health check failed after {} retries", checker.config.retries);
            }
        }

        // Spawn file watcher if configured
        if let Some(watch) = &svc.watch {
            if watch.restart {
                let (tx, mut rx) = tokio::sync::mpsc::channel::<String>(8);
                spawn_watcher(name.to_string(), watch.paths.clone(), watch.ignore.clone(), tx);

                let handles = self.handles.clone();
                let svc_name = name.to_string();
                let events = self.events.clone();
                tokio::spawn(async move {
                    while let Some(svc) = rx.recv().await {
                        tracing::info!("[{svc}] file change detected, restarting...");
                        let mut map = handles.write().await;
                        if let Some(h) = map.get_mut(&svc) {
                            let _ = h.child.kill().await;
                            h.state = ServiceState::Stopped;
                            let _ = events.send(SupervisorEvent::StateChanged {
                                service: svc_name.clone(),
                                state: "stopped".into(),
                            });
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
            let _ = h.child.kill().await;
            h.state = ServiceState::Stopped;
            self.emit(SupervisorEvent::StateChanged {
                service: name.to_string(),
                state: "stopped".into(),
            });
        }
    }

    pub async fn restart_service(&self, name: &str, color_idx: usize) -> Result<()> {
        self.stop_service(name).await;
        self.start_service(name, color_idx).await
    }

    pub async fn status(&self) -> Vec<(String, String, Option<u32>, u16)> {
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
                (name.clone(), state, pid, svc.port)
            })
            .collect()
    }

    fn emit(&self, event: SupervisorEvent) {
        let _ = self.events.send(event);
    }
}
