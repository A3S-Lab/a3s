use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use colored::Colorize;
use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::ChildStdout;
use tokio::sync::broadcast;

/// A log line emitted by a service.
#[derive(Debug, Clone)]
pub struct LogLine {
    pub service: String,
    pub line: String,
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
    "cyan", "green", "yellow", "magenta", "blue", "bright cyan", "bright green", "bright yellow",
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
        let _color = COLORS[color_idx % COLORS.len()];
        tokio::spawn(async move {
            let mut reader = BufReader::new(stdout).lines();
            while let Ok(Some(line)) = reader.next_line().await {
                let _ = tx.send(LogLine {
                    service: service.clone(),
                    line,
                });
            }
        });
    }

    pub fn subscribe(&self) -> broadcast::Receiver<LogLine> {
        self.tx.subscribe()
    }

    /// Return up to `n` recent log lines, optionally filtered by service.
    pub fn recent(&self, service: Option<&str>, n: usize) -> Vec<LogLine> {
        let history = self.history.lock().unwrap();
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
                    let prefix = format!("[{}]", entry.service);
                    println!("{} {}", prefix.cyan(), entry.line);
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
                        let mut h = log.history.lock().unwrap();
                        if h.len() >= HISTORY_CAP {
                            h.pop_front();
                        }
                        h.push_back(entry);
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                }
            }
        });
    }
}
