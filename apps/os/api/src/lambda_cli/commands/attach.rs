//! `a3s attach` command - Attach to a running container (kubectl attach style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};

/// Attach to a running container's stdin/stdout/stderr.
#[derive(clap::Parser, Debug)]
pub struct AttachCommand {
    /// Pod name.
    name: String,

    /// Container name (for multi-container pods).
    #[arg(short, long)]
    container: Option<String>,

    /// Disable stdin.
    #[arg(long)]
    disable_stdin: bool,

    /// Attach to tty.
    #[arg(short = 't', long)]
    tty: bool,

    /// Tail console log only (non-interactive).
    #[arg(long)]
    console: bool,
}

impl AttachCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    /// Find sandbox ID by name (either sandbox ID or deployment name).
    fn find_sandbox_id(name: &str) -> Result<String> {
        let boxes_dir = Self::boxes_dir();

        // First check if name is already a sandbox ID
        if boxes_dir.join(name).exists() {
            return Ok(name.to_string());
        }

        // Search for deployment name in info.json
        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let info_path = path.join("info.json");
                    if info_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&info_path) {
                            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                                if info
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s == name)
                                    .unwrap_or(false)
                                {
                                    return path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .ok_or_else(|| {
                                            A3sError::Project(format!(
                                                "invalid sandbox id for '{}'",
                                                name
                                            ))
                                        });
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(A3sError::Project(format!("pod '{}' not found", name)))
    }

    /// Get sandbox info including console log path and PTY socket.
    fn get_sandbox_info(&self, sandbox_id: &str) -> Result<(Option<PathBuf>, Option<PathBuf>)> {
        let info_path = Self::boxes_dir().join(sandbox_id).join("info.json");
        if !info_path.exists() {
            return Ok((None, None));
        }

        let content = std::fs::read_to_string(&info_path)?;
        let info: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| A3sError::Project(e.to_string()))?;

        let console_log = info
            .get("console_log")
            .and_then(|v| v.as_str())
            .map(PathBuf::from);

        let pty_socket = Self::boxes_dir()
            .join(sandbox_id)
            .join("sockets")
            .join("pty.sock");

        let pty_socket = if pty_socket.exists() {
            Some(pty_socket)
        } else {
            None
        };

        Ok((console_log, pty_socket))
    }

    /// Tail the console log file.
    async fn tail_console_log(&self, log_path: &PathBuf) -> Result<()> {
        let file = tokio::fs::File::open(log_path)
            .await
            .map_err(|e| A3sError::Io(e))?;

        let mut reader = BufReader::new(file).lines();
        println!(
            "Tailing console log: {}. Press Ctrl+C to detach.",
            log_path.display()
        );

        loop {
            match reader.next_line().await {
                Ok(Some(line)) => {
                    println!("{}", line);
                }
                Ok(None) => {
                    // EOF - file might still be open, wait for more
                    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                }
                Err(e) => {
                    eprintln!("Error reading console log: {}", e);
                    break;
                }
            }
        }

        Ok(())
    }

    /// Check if sandbox is running.
    fn is_running(&self, sandbox_id: &str) -> bool {
        let socket_dir = Self::boxes_dir().join(sandbox_id).join("sockets");
        socket_dir.exists()
            && std::fs::read_dir(socket_dir)
                .map(|mut d| d.next().is_some())
                .unwrap_or(false)
    }

    /// Get list of available sockets for a sandbox.
    fn list_sockets(&self, sandbox_id: &str) -> Vec<String> {
        let socket_dir = Self::boxes_dir().join(sandbox_id).join("sockets");
        if !socket_dir.exists() {
            return vec![];
        }

        std::fs::read_dir(socket_dir)
            .map(|entries| {
                entries
                    .flatten()
                    .filter_map(|e| e.file_name().to_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default()
    }
}

#[async_trait]
impl Command for AttachCommand {
    async fn run(&self) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let (console_log, pty_socket) = self.get_sandbox_info(&sandbox_id)?;

        // Check if sandbox is running
        if !self.is_running(&sandbox_id) {
            return Err(A3sError::Project(format!(
                "pod '{}' is not running",
                self.name
            )));
        }

        // Console log tailing mode (non-interactive)
        if self.console {
            if let Some(ref log_path) = console_log {
                if log_path.exists() {
                    return self.tail_console_log(log_path).await;
                } else {
                    return Err(A3sError::Project(format!(
                        "console log not found at {}",
                        log_path.display()
                    )));
                }
            } else {
                return Err(A3sError::Project(
                    "console log not configured for this pod".to_string(),
                ));
            }
        }

        // Interactive TTY mode (requires PTY socket and external PTY handler)
        if self.tty {
            if let Some(_socket) = pty_socket {
                // Use the a3s-box CLI for actual PTY attach
                let output = std::process::Command::new("a3s-box")
                    .args(["attach", &self.name])
                    .output();

                match output {
                    Ok(out) => {
                        if out.status.success() {
                            print!("{}", String::from_utf8_lossy(&out.stdout));
                            return Ok(());
                        } else {
                            eprint!("{}", String::from_utf8_lossy(&out.stderr));
                            return Err(A3sError::Project(format!(
                                "attach failed with exit code: {:?}",
                                out.status.code()
                            )));
                        }
                    }
                    Err(_) => {
                        return Err(A3sError::Project(
                            "a3s-box CLI not found - install a3s-box to use interactive attach"
                                .to_string(),
                        ));
                    }
                }
            } else {
                return Err(A3sError::Project(
                    "PTY socket not found - this pod may not support interactive mode".to_string(),
                ));
            }
        }

        // Default: show attach options and status
        println!("Attaching to pod '{}' (sandbox: {})", self.name, sandbox_id);
        println!();

        let sockets = self.list_sockets(&sandbox_id);
        if !sockets.is_empty() {
            println!("Available sockets:");
            for socket in &sockets {
                println!("  - {}", socket);
            }
            println!();
        }

        if pty_socket.is_some() {
            println!("Interactive mode available:");
            println!(
                "  a3s attach {} -t    # Attach with PTY (requires a3s-box CLI)",
                self.name
            );
        }

        if console_log.is_some() {
            println!("Console log available:");
            println!("  a3s attach {} --console   # Tail console log", self.name);
        }

        println!();
        println!(
            "Use 'a3s exec {} -- <command>' to execute commands.",
            self.name
        );

        Err(A3sError::Project(
            "specify -t for interactive attach or --console to tail log".to_string(),
        ))
    }
}
