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
pub struct LogAggregator {
    tx: broadcast::Sender<LogLine>,
}

// Fixed palette — one color per service slot (cycles if > 8 services)
const COLORS: &[&str] = &[
    "cyan", "green", "yellow", "magenta", "blue", "bright cyan", "bright green", "bright yellow",
];

impl LogAggregator {
    pub fn new() -> (Self, broadcast::Receiver<LogLine>) {
        let (tx, rx) = broadcast::channel(4096);
        (Self { tx }, rx)
    }

    /// Spawn a task that reads lines from `stdout` and broadcasts them.
    pub fn attach(
        &self,
        service: String,
        color_idx: usize,
        stdout: ChildStdout,
    ) {
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

    /// Print log lines to stdout with colored service prefix.
    pub async fn print_loop(mut rx: broadcast::Receiver<LogLine>) {
        loop {
            match rx.recv().await {
                Ok(entry) => {
                    let prefix = format!("[{}]", entry.service);
                    println!("{} {}", prefix.cyan(), entry.line);
                }
                Err(broadcast::error::RecvError::Lagged(n)) => {
                    eprintln!("{}", format!("[a3s] log buffer lagged, dropped {n} lines").yellow());
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }
}
