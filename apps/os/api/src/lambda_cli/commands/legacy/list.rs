//! `a3s list` command - List all deployed services.

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use std::path::PathBuf;

/// List all deployed services across all projects.
#[derive(clap::Parser, Debug)]
pub struct ListCommand {
    /// Show only running services.
    #[arg(short, long)]
    running: bool,

    /// Show all services including stopped.
    #[arg(short, long)]
    all: bool,
}

impl ListCommand {
    /// Scan ~/.a3s/boxes/ for running VMs.
    fn list_running_boxes(&self) -> Vec<BoxInfo> {
        let boxes_dir = dirs_home()
            .map(|h| h.join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"));

        if !boxes_dir.exists() {
            return Vec::new();
        }

        let mut boxes = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let socket_path = path.join("sockets").join("exec.sock");
                    let running = socket_path.exists();
                    if self.running && !running {
                        continue;
                    }

                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy().to_string())
                        .unwrap_or_default();

                    // Try to read logs to get more info
                    let log_path = path.join("logs").join("console.log");
                    let image = Self::extract_image_from_logs(&log_path)
                        .unwrap_or_else(|| "unknown".to_string());

                    boxes.push(BoxInfo {
                        id: name,
                        image,
                        status: if running { "running" } else { "stopped" }.to_string(),
                        socket: if running {
                            Some(socket_path.to_string_lossy().to_string())
                        } else {
                            None
                        },
                    });
                }
            }
        }
        boxes
    }

    fn extract_image_from_logs(log_path: &PathBuf) -> Option<String> {
        if !log_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(log_path).ok()?;
        // Look for image in logs like "Pulling OCI image" or container entrypoint
        if let Some(line) = content.lines().find(|l| l.contains("reference")) {
            Some(line.split("reference").nth(1)?.trim().to_string())
        } else if let Some(line) = content.lines().find(|l| l.contains("/")) {
            Some(line.trim().to_string())
        } else {
            None
        }
    }

    fn print_boxes(&self, boxes: &[BoxInfo]) {
        if boxes.is_empty() {
            println!("No deployed services found.");
            return;
        }

        println!(
            "{:<36} {:<25} {:<10} {}",
            "ID", "IMAGE", "STATUS", "EXEC SOCKET"
        );
        println!("{}", "-".repeat(90));
        for box_info in boxes {
            let socket = box_info.socket.as_ref().map(|s| s.as_str()).unwrap_or("-");
            println!(
                "{:<36} {:<25} {:<10} {}",
                box_info.id, box_info.image, box_info.status, socket
            );
        }
        println!();
        println!("Total: {} service(s)", boxes.len());
    }
}

struct BoxInfo {
    id: String,
    image: String,
    status: String,
    socket: Option<String>,
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var("A3S_HOME")
        .ok()
        .map(PathBuf::from)
        .or_else(|| dirs::home_dir().map(|h| h.join(".a3s")))
}

#[async_trait]
impl Command for ListCommand {
    async fn run(&self) -> Result<()> {
        let boxes = self.list_running_boxes();

        if boxes.is_empty() && !self.all {
            println!("No running services found.");
            println!("Use --all to show stopped services.");
        } else {
            self.print_boxes(&boxes);
        }

        Ok(())
    }
}
