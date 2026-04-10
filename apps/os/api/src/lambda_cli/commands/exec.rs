//! `a3s exec` command - Execute command in a VM (kubectl exec style).

use crate::commands::Command;
use crate::deployment::microvm::MicrovmProvider;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Execute a command in a running VM.
#[derive(clap::Parser, Debug)]
pub struct ExecCommand {
    /// Pod name.
    name: String,

    /// Container name (for multi-container pods).
    #[arg(short, long)]
    container: Option<String>,

    /// Command to execute.
    #[arg(last = true, allow_hyphen_values = true)]
    command: Vec<String>,

    /// Interactive mode.
    #[arg(short, long)]
    interactive: bool,
}

impl ExecCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    fn get_sandbox_id(&self, name: &str) -> Result<String> {
        // First check if name is already a sandbox ID (UUID format)
        let box_path = Self::boxes_dir().join(name);
        if box_path.exists() {
            return Ok(name.to_string());
        }

        // Otherwise, look for a box with matching deployment name in info.json
        let boxes_dir = Self::boxes_dir();
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
}

#[async_trait]
impl Command for ExecCommand {
    async fn run(&self) -> Result<()> {
        if self.command.is_empty() {
            return Err(A3sError::Project("no command specified".to_string()));
        }

        let sandbox_id = self.get_sandbox_id(&self.name)?;
        let socket_path = Self::boxes_dir()
            .join(&sandbox_id)
            .join("sockets")
            .join("exec.sock");

        if !socket_path.exists() {
            return Err(A3sError::Project(format!(
                "pod '{}' is not running or has no exec socket",
                self.name
            )));
        }

        let provider = MicrovmProvider::new(&PathBuf::from(".")).await;

        let cmd = self
            .command
            .first()
            .cloned()
            .ok_or_else(|| A3sError::Project("no command specified".to_string()))?;
        let args: Vec<&str> = self.command.iter().skip(1).map(|s| s.as_str()).collect();

        let result = provider.exec_in_sandbox(&sandbox_id, &cmd, &args).await?;

        println!("{}", result.stdout);
        if !result.stderr.is_empty() {
            eprintln!("{}", result.stderr);
        }

        if result.exit_code != 0 {
            return Err(A3sError::Project(format!(
                "command exited with code {}",
                result.exit_code
            )));
        }

        Ok(())
    }
}
