//! `a3s logs` command - View logs (kubectl logs style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::time::{timeout, Duration};

/// View logs from a pod.
#[derive(clap::Parser, Debug)]
pub struct LogsCommand {
    /// Pod name.
    name: String,

    /// Number of lines to show from end.
    #[arg(short, long, default_value = "100")]
    tail: usize,

    /// Follow log output.
    #[arg(short, long)]
    follow: bool,

    /// Show previous container logs (for restarts).
    #[arg(long)]
    previous: bool,

    /// Container name (for multi-container pods).
    #[arg(short, long)]
    container: Option<String>,

    /// Show timestamps on each line.
    #[arg(long)]
    timestamps: bool,

    /// Show logs since duration (e.g., 1h, 30m, 2h15m).
    #[arg(long)]
    since: Option<String>,
}

impl LogsCommand {
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

    fn get_log_path(&self) -> Result<PathBuf> {
        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let log_path = Self::boxes_dir()
            .join(&sandbox_id)
            .join("logs")
            .join("console.log");
        if !log_path.exists() {
            return Err(A3sError::Project(format!(
                "pod '{}' has no logs",
                self.name
            )));
        }
        Ok(log_path)
    }

    /// Parse duration string (e.g., "1h30m", "30m", "2h") into seconds.
    fn parse_since_duration(s: &str) -> Option<u64> {
        let s = s.to_lowercase();
        let mut total_secs: u64 = 0;
        let mut current_num = String::new();

        for c in s.chars() {
            if c.is_ascii_digit() {
                current_num.push(c);
            } else if c == 'h' {
                let hours: u64 = current_num.parse().unwrap_or(0);
                total_secs += hours * 3600;
                current_num.clear();
            } else if c == 'm' {
                let mins: u64 = current_num.parse().unwrap_or(0);
                total_secs += mins * 60;
                current_num.clear();
            } else if c == 's' {
                let secs: u64 = current_num.parse().unwrap_or(0);
                total_secs += secs;
                current_num.clear();
            }
        }

        if total_secs > 0 {
            Some(total_secs)
        } else {
            None
        }
    }
}

#[async_trait]
impl Command for LogsCommand {
    async fn run(&self) -> Result<()> {
        let log_path = self.get_log_path()?;

        let content = std::fs::read_to_string(&log_path)?;

        // Calculate the cutoff time if --since is specified
        let cutoff_time = self.since.as_ref().and_then(|s| {
            Self::parse_since_duration(s)
                .map(|secs| chrono::Utc::now() - chrono::Duration::seconds(secs as i64))
        });

        // Parse and filter lines
        let all_lines: Vec<&str> = content.lines().collect();
        let filtered_lines: Vec<&str> = if let Some(cutoff) = cutoff_time {
            // Simple timestamp detection - look for ISO8601 timestamps at start of line
            all_lines
                .iter()
                .filter(|line| {
                    if line.len() < 20 {
                        return true; // Keep lines without obvious timestamps
                    }
                    // Try to parse timestamp from line start
                    if let Ok(ts) =
                        chrono::DateTime::parse_from_rfc3339(&line[..std::cmp::min(line.len(), 30)])
                    {
                        ts.with_timezone(&chrono::Utc) >= cutoff
                    } else {
                        true // Keep if can't parse
                    }
                })
                .cloned()
                .collect()
        } else {
            all_lines
        };

        // Take last N lines after filtering
        let lines: Vec<&str> = filtered_lines
            .iter()
            .rev()
            .take(self.tail)
            .map(|s| *s)
            .collect();
        for line in lines.iter().rev() {
            if self.timestamps {
                // Try to extract timestamp or add current time
                if line.len() > 20 {
                    if let Ok(_ts) =
                        chrono::DateTime::parse_from_rfc3339(&line[..std::cmp::min(line.len(), 30)])
                    {
                        println!("{}", line);
                    } else {
                        println!(
                            "{} {}",
                            chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                            line
                        );
                    }
                } else {
                    println!(
                        "{} {}",
                        chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"),
                        line
                    );
                }
            } else {
                println!("{}", line);
            }
        }

        if self.follow {
            println!("Following logs... (Ctrl+C to exit)");
            let file = tokio::fs::File::open(&log_path)
                .await
                .map_err(|e| A3sError::Project(format!("failed to open log file: {}", e)))?;
            let mut reader = BufReader::new(file);

            let mut line = String::new();
            loop {
                match timeout(Duration::from_secs(1), reader.read_line(&mut line)).await {
                    Ok(Ok(0)) => break,
                    Ok(Ok(_)) => {
                        if self.timestamps && line.len() > 0 {
                            print!("{} ", chrono::Utc::now().format("%Y-%m-%dT%H:%M:%SZ"));
                        }
                        print!("{}", line);
                    }
                    Ok(Err(e)) => {
                        eprintln!("Error reading log: {}", e);
                        break;
                    }
                    Err(_) => {
                        // Timeout, continue loop
                        line.clear();
                    }
                }
            }
        }

        Ok(())
    }
}
