//! Sentinel daemon manager (parent-side).
//!
//! Spawns the sentinel security observer as an isolated child process via the
//! hidden `sentinel-daemon` CLI subcommand, communicates with it over a Unix
//! Domain Socket (newline-delimited JSON), and runs a watchdog task that
//! automatically restarts the child on crash.
//!
//! # Fail-open guarantee
//!
//! If the IPC call times out or the socket is unavailable, the caller receives
//! `SecurityVerdict::allow()`.  The sentinel may never block normal agent
//! operation due to its own failure.
//!
//! # Restart policy
//!
//! Up to `MAX_RESTARTS` crashes within `RESTART_WINDOW_SECS` are tolerated.
//! After that the watchdog exits and the sentinel is effectively disabled for
//! the lifetime of the parent process.

use super::ipc::{SentinelRequest, SentinelResponse};
use super::{SecurityVerdict, SentinelObserver};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

/// IPC round-trip timeout — must fit within Phase 1's 5 ms latency budget.
const IPC_TIMEOUT_MS: u64 = 500;
const MAX_RESTARTS: usize = 3;
const RESTART_WINDOW_SECS: u64 = 60;

// ── SentinelDaemon ────────────────────────────────────────────────────────────

/// Handle to the sentinel daemon child process.
///
/// All analysis methods communicate with the child over a Unix Domain Socket.
/// Implements [`SentinelObserver`] as a drop-in replacement for the in-process
/// [`SentinelAgent`](super::SentinelAgent).
///
/// When this handle is dropped (i.e., the main SafeClaw process shuts down),
/// the watchdog task receives a cancellation signal and the daemon socket is
/// cleaned up automatically by the child process.
#[derive(Debug)]
pub struct SentinelDaemon {
    socket_path: PathBuf,
    /// Signals the watchdog to stop restarting the child on the next iteration.
    shutdown_tx: tokio::sync::watch::Sender<bool>,
}

impl Drop for SentinelDaemon {
    fn drop(&mut self) {
        // Signal the watchdog: do not restart after the next child exit.
        let _ = self.shutdown_tx.send(true);
        // Best-effort: remove socket file so no stale entries linger.
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

impl SentinelDaemon {
    /// Spawn the sentinel child process and return the daemon handle.
    ///
    /// The child is started with the hidden `sentinel-daemon` subcommand of the
    /// same `safeclaw` binary.  A watchdog Tokio task monitors the child and
    /// restarts it on unexpected exit.
    pub fn spawn(sentinel_dir: &Path, config_path: Option<&Path>) -> Arc<Self> {
        let socket_path = runtime_socket_path();
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let daemon = Arc::new(Self { socket_path: socket_path.clone(), shutdown_tx });

        tokio::spawn(watchdog(
            socket_path,
            sentinel_dir.to_path_buf(),
            config_path.map(|p| p.to_path_buf()),
            shutdown_rx,
        ));

        daemon
    }

    // ── IPC ──────────────────────────────────────────────────────────────────

    async fn send(&self, req: SentinelRequest) -> SentinelResponse {
        match tokio::time::timeout(
            Duration::from_millis(IPC_TIMEOUT_MS),
            connect_and_send(&self.socket_path, &req),
        )
        .await
        {
            Ok(Ok(resp)) => resp,
            Ok(Err(e)) => {
                tracing::debug!("Sentinel IPC error: {e}");
                SentinelResponse::allow()
            }
            Err(_) => {
                tracing::debug!(IPC_TIMEOUT_MS, "Sentinel IPC timeout — failing open");
                SentinelResponse::allow()
            }
        }
    }

    /// Send a fire-and-forget notification (no reply needed).
    fn fire_and_forget(&self, req: SentinelRequest) {
        let socket_path = self.socket_path.clone();
        tokio::spawn(async move {
            let _ = connect_and_send(&socket_path, &req).await;
        });
    }
}

async fn connect_and_send(
    socket_path: &Path,
    req: &SentinelRequest,
) -> anyhow::Result<SentinelResponse> {
    let stream = UnixStream::connect(socket_path).await?;
    let (reader, mut writer) = stream.into_split();

    let mut line = serde_json::to_string(req)?;
    line.push('\n');
    writer.write_all(line.as_bytes()).await?;

    let mut lines = BufReader::new(reader).lines();
    let resp_line = lines
        .next_line()
        .await?
        .ok_or_else(|| anyhow::anyhow!("daemon closed connection without reply"))?;
    Ok(serde_json::from_str(&resp_line)?)
}

// ── SentinelObserver impl ─────────────────────────────────────────────────────

#[async_trait::async_trait]
impl SentinelObserver for SentinelDaemon {
    async fn analyze_pre_tool_use(
        &self,
        session_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> SecurityVerdict {
        let resp = self
            .send(SentinelRequest::AnalyzeToolUse {
                session_id: session_id.to_string(),
                tool: tool.to_string(),
                args: args.clone(),
            })
            .await;
        ipc_to_verdict(resp)
    }

    async fn analyze_prompt(&self, session_id: &str, prompt: &str) -> SecurityVerdict {
        let resp = self
            .send(SentinelRequest::AnalyzePrompt {
                session_id: session_id.to_string(),
                prompt: prompt.to_string(),
            })
            .await;
        ipc_to_verdict(resp)
    }

    async fn analyze_response(&self, response_text: &str) -> SecurityVerdict {
        let resp = self
            .send(SentinelRequest::AnalyzeResponse { response: response_text.to_string() })
            .await;
        ipc_to_verdict(resp)
    }

    fn on_session_created(&self, session_id: &str, parent_id: Option<&str>) {
        self.fire_and_forget(SentinelRequest::SessionCreated {
            session_id: session_id.to_string(),
            parent_id: parent_id.map(str::to_string),
        });
    }

    fn on_session_destroyed(&self, session_id: &str) {
        self.fire_and_forget(SentinelRequest::SessionDestroyed {
            session_id: session_id.to_string(),
        });
    }
}

fn ipc_to_verdict(resp: SentinelResponse) -> SecurityVerdict {
    if resp.should_block {
        SecurityVerdict::block(resp.reason.unwrap_or_else(|| "Sentinel daemon block".to_string()))
    } else {
        SecurityVerdict::allow()
    }
}

// ── Socket path ───────────────────────────────────────────────────────────────

fn runtime_socket_path() -> PathBuf {
    let pid = std::process::id();
    let dir = std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::temp_dir());
    dir.join(format!("safeclaw-sentinel-{pid}.sock"))
}

// ── Watchdog ──────────────────────────────────────────────────────────────────

async fn watchdog(
    socket_path: PathBuf,
    sentinel_dir: PathBuf,
    config_path: Option<PathBuf>,
    mut shutdown_rx: tokio::sync::watch::Receiver<bool>,
) {
    let current_exe = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("safeclaw"));
    let mut restart_times: Vec<Instant> = Vec::new();

    loop {
        // Stop if the daemon handle was dropped (main process shutting down).
        if *shutdown_rx.borrow() {
            tracing::info!("Sentinel watchdog: shutdown signal received — stopping");
            return;
        }

        // Evict timestamps outside the restart window.
        let now = Instant::now();
        restart_times
            .retain(|t| now.duration_since(*t) < Duration::from_secs(RESTART_WINDOW_SECS));

        if restart_times.len() >= MAX_RESTARTS {
            tracing::error!(
                max = MAX_RESTARTS,
                window_secs = RESTART_WINDOW_SECS,
                "Sentinel daemon crash loop detected — disabling sentinel (fail-open)"
            );
            return;
        }

        // Spawn the child.
        let mut cmd = tokio::process::Command::new(&current_exe);
        cmd.arg("sentinel-daemon")
            .arg("--socket")
            .arg(&socket_path)
            .arg("--sentinel-dir")
            .arg(&sentinel_dir)
            .stdin(std::process::Stdio::null());
        if let Some(ref cfg) = config_path {
            cmd.arg("--config").arg(cfg);
        }

        match cmd.spawn() {
            Ok(mut child) => {
                let pid = child.id().unwrap_or(0);
                tracing::info!(pid, socket = %socket_path.display(), "Sentinel daemon spawned");

                // Wait for child exit OR shutdown signal from parent.
                tokio::select! {
                    wait_result = child.wait() => {
                        match wait_result {
                            Ok(status) if status.success() => {
                                tracing::info!(pid, "Sentinel daemon exited cleanly");
                                return;
                            }
                            Ok(status) => {
                                tracing::warn!(pid, ?status, "Sentinel daemon crashed — restarting");
                            }
                            Err(e) => {
                                tracing::warn!(pid, "Sentinel daemon wait error: {e} — restarting");
                            }
                        }
                    }
                    _ = shutdown_rx.changed() => {
                        if *shutdown_rx.borrow() {
                            tracing::info!(pid, "Sentinel watchdog: killing daemon (main process exit)");
                            let _ = child.kill().await;
                            let _ = std::fs::remove_file(&socket_path);
                            return;
                        }
                    }
                }
            }
            Err(e) => {
                tracing::warn!("Failed to spawn sentinel daemon: {e}");
            }
        }

        restart_times.push(Instant::now());
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
}
