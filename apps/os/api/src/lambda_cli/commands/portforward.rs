//! `a3s port-forward` command - Forward local ports to pod ports (kubectl port-forward style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// PortForward command - Forward local ports to a pod.
#[derive(clap::Parser, Debug)]
pub struct PortForwardCommand {
    /// Pod name.
    name: String,

    /// Local port to listen on (use 0 for random assignment).
    #[arg(short = 'p', long)]
    port: Option<u16>,

    /// Target port on the pod.
    #[arg(short = 't', long)]
    target_port: Option<u16>,

    /// Container name (for multi-container pods).
    #[arg(short, long)]
    container: Option<String>,

    /// Protocol to use.
    #[arg(long, default_value = "tcp")]
    protocol: String,

    /// List available port forwards.
    #[arg(long)]
    list: bool,
}

impl PortForwardCommand {
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

    /// Get port forwards from sandbox info.
    fn get_port_forwards(&self, sandbox_id: &str) -> Result<Vec<(u16, u16)>> {
        let info_path = Self::boxes_dir().join(sandbox_id).join("info.json");
        if !info_path.exists() {
            return Ok(vec![]);
        }

        let content = std::fs::read_to_string(&info_path)?;
        let info: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| A3sError::Project(e.to_string()))?;

        // Try port_forwards array first
        if let Some(forwards) = info.get("port_forwards").and_then(|v| v.as_array()) {
            let mut result = Vec::new();
            for forward in forwards {
                let host = forward
                    .get("host_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                let guest = forward
                    .get("guest_port")
                    .and_then(|v| v.as_u64())
                    .unwrap_or(0) as u16;
                if host != 0 && guest != 0 {
                    result.push((host, guest));
                }
            }
            return Ok(result);
        }

        // Try direct ports
        if let Some(port) = info.get("port").and_then(|v| v.as_u64()) {
            return Ok(vec![(port as u16, port as u16)]);
        }

        if let Some(ports) = info.get("ports").and_then(|v| v.as_object()) {
            let mut result = Vec::new();
            for (_, port_val) in ports {
                if let Some(port) = port_val.as_u64() {
                    result.push((port as u16, port as u16));
                }
            }
            return Ok(result);
        }

        Ok(vec![])
    }

    /// Show port forwards for a pod.
    fn show_port_forwards(&self, sandbox_id: &str) -> Result<()> {
        let forwards = self.get_port_forwards(sandbox_id)?;

        if forwards.is_empty() {
            println!("No port forwards configured for pod '{}'", self.name);
            return Ok(());
        }

        println!(
            "{:<40} {:<12} {:<12} {:<10}",
            "NAME", "LOCAL_PORT", "PORT", "PROTOCOL"
        );
        println!("{}", "-".repeat(75));

        for (local, guest) in &forwards {
            println!(
                "{:<40} {:<12} {:<12} {:<10}",
                sandbox_id, local, guest, "tcp"
            );
        }

        Ok(())
    }
}

#[async_trait]
impl Command for PortForwardCommand {
    async fn run(&self) -> Result<()> {
        if self.list {
            // List port forwards for the pod
            let sandbox_id = Self::find_sandbox_id(&self.name)?;
            return self.show_port_forwards(&sandbox_id);
        }

        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let target_port = self.target_port.unwrap_or_else(|| {
            self.get_port_forwards(&sandbox_id)
                .ok()
                .and_then(|f| f.first().map(|(_, g)| *g))
                .unwrap_or(8000)
        });
        let local_port = self.port.unwrap_or(0);

        // Show what would be forwarded
        println!("Port forward configuration for pod '{}':", self.name);
        println!("  Sandbox ID: {}", sandbox_id);
        println!(
            "  Local port: {}",
            if local_port == 0 {
                "random".to_string()
            } else {
                local_port.to_string()
            }
        );
        println!("  Target port: {}", target_port);
        println!();

        // Check if the sandbox is running by looking for sockets
        let socket_dir = Self::boxes_dir().join(&sandbox_id).join("sockets");
        if socket_dir.exists() {
            if let Ok(entries) = std::fs::read_dir(&socket_dir) {
                let sockets: Vec<_> = entries
                    .flatten()
                    .filter(|e| e.path().extension().map(|ex| ex == "sock").unwrap_or(false))
                    .collect();

                if sockets.is_empty() {
                    println!("Pod is not running (no sockets found)");
                } else {
                    println!(
                        "Pod appears to be running ({} socket(s) found)",
                        sockets.len()
                    );
                    for socket in &sockets {
                        if let Some(name) = socket.file_name().to_str() {
                            println!("  - {}", name);
                        }
                    }
                }
            }
        } else {
            println!("Pod is not running (no socket directory found)");
        }

        println!();
        println!("Note: Full port forwarding requires runtime support for socket-based proxying.");
        println!("The BoxSdk does not currently support dynamic port forwarding.");
        println!(
            "Use 'a3s exec {} -- curl http://localhost:{}/<path>' to access services.",
            self.name, target_port
        );

        Ok(())
    }
}
