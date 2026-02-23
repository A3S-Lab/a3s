use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use colored::Colorize;
use serde::Serialize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{ChildStderr, ChildStdout};
use tokio::sync::broadcast;

/// A log line emitted by a service.
#[derive(Debug, Clone, Serialize)]
pub struct LogLine {
    pub service: String,
    pub line: String,
    #[serde(skip)]
    pub color_idx: usize,
}

/// Aggregates log lines from all services into a single broadcast channel.
/// Also maintains a ring buffer of recent lines for history replay.
pub struct LogAggregator {
    tx: broadcast::Sender<LogLine>,
    history: Mutex<VecDeque<LogLine>>,
}

const HISTORY_CAP: usize = 1000;

// Fixed palette — one color per service slot (cycles if > 8 services)
const COLORS: &[&str] = &[
    "cyan",
    "green",
    "yellow",
    "magenta",
    "blue",
    "bright cyan",
    "bright green",
    "bright yellow",
];

impl LogAggregator {
    pub fn new() -> (Self, broadcast::Receiver<LogLine>) {
        let (tx, rx) = broadcast::channel(4096);
        (
            Self {
                tx,
                history: Mutex::new(VecDeque::with_capacity(HISTORY_CAP)),
            },
            rx,
        )
    }

    /// Spawn a task that reads lines from `stdout` and broadcasts them.
    pub fn attach(&self, service: String, color_idx: usize, stdout: ChildStdout) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx.send(LogLine {
                    service: service.clone(),
                    line,
                    color_idx,
                });
            }
        });
    }

    /// Spawn a task that reads lines from `stderr` and broadcasts them.
    pub fn attach_stderr(&self, service: String, color_idx: usize, stderr: ChildStderr) {
        let tx = self.tx.clone();
        tokio::spawn(async move {
            let mut reader = BufReader::new(stderr).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx.send(LogLine {
                    service: service.clone(),
                    line,
                    color_idx,
                });
            }
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogLine> {
        self.tx.subscribe()
    }

    /// Return up to `n` recent log lines, optionally filtered by service.
    pub fn recent(&self, service: Option<&str>, n: usize) -> Vec<LogLine> {
        let Ok(history) = self.history.lock() else {
            return vec![];
        };
        history
            .iter()
            .filter(|l| service.is_none_or(|s| l.service == s))
            .rev()
            .take(n)
            .cloned()
            .collect::<Vec<_>>()
            .into_iter()
            .rev()
            .collect()
    }

    /// Print log lines to stdout with colored service prefix.
    /// Also stores lines in the history ring buffer.
    pub async fn print_loop(mut rx: broadcast::Receiver<LogLine>) {
        loop {
            match rx.recv().await {
                Ok(entry) => {
                    let color = COLORS[entry.color_idx % COLORS.len()];
                    let prefix = format!("[{}]", entry.service);
                    let colored_prefix = match color {
                        "cyan" => prefix.cyan().to_string(),
                        "green" => prefix.green().to_string(),
                        "yellow" => prefix.yellow().to_string(),
                        "magenta" => prefix.magenta().to_string(),
                        "blue" => prefix.blue().to_string(),
                        "bright cyan" => prefix.bright_cyan().to_string(),
                        "bright green" => prefix.bright_green().to_string(),
                        "bright yellow" => prefix.bright_yellow().to_string(),
                        _ => prefix.cyan().to_string(),
                    };
                    println!("{} {}", colored_prefix, entry.line);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!(
                        "{}",
                        format!("[a3s] log buffer lagged, dropped {n} lines").yellow()
                    );
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    /// Spawn a task that stores broadcast lines into the history ring buffer.
    pub fn spawn_history_recorder(log: Arc<Self>)
    where
        Self: Send + Sync + 'static,
    {
        let mut rx = log.tx.subscribe();
        tokio::spawn(async move {
            loop {
                match rx.recv().await {
                    Ok(entry) => {
                        match log.history.lock() {
                            Ok(mut h) => {
                                if h.len() >= HISTORY_CAP {
                                    h.pop_front();
                                }
                                h.push_back(entry);
                            }
                            Err(_) => {} // poisoned — skip entry, don't panic
                        }
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                }
            }
        });
    }
}
