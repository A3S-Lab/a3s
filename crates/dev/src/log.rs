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
                        if let Ok(mut h) = log.history.lock() {
                            if h.len() >= HISTORY_CAP {
                                h.pop_front();
                            }
                            h.push_back(entry);
                        } // poisoned — skip entry, don't panic
                    }
                    Err(broadcast::error::RecvError::Closed) => break,
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                }
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_line(service: &str, line: &str) -> LogLine {
        LogLine {
            service: service.to_string(),
            line: line.to_string(),
            color_idx: 0,
        }
    }

    #[test]
    fn test_recent_returns_all_when_under_cap() {
        let (agg, _rx) = LogAggregator::new();
        {
            let mut h = agg.history.lock().unwrap();
            h.push_back(make_line("svc-a", "line1"));
            h.push_back(make_line("svc-b", "line2"));
            h.push_back(make_line("svc-a", "line3"));
        }
        let all = agg.recent(None, 100);
        assert_eq!(all.len(), 3);
        assert_eq!(all[0].line, "line1");
        assert_eq!(all[2].line, "line3");
    }

    #[test]
    fn test_recent_filters_by_service() {
        let (agg, _rx) = LogAggregator::new();
        {
            let mut h = agg.history.lock().unwrap();
            h.push_back(make_line("svc-a", "a1"));
            h.push_back(make_line("svc-b", "b1"));
            h.push_back(make_line("svc-a", "a2"));
        }
        let filtered = agg.recent(Some("svc-a"), 100);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|l| l.service == "svc-a"));
    }

    #[test]
    fn test_recent_respects_n_limit() {
        let (agg, _rx) = LogAggregator::new();
        {
            let mut h = agg.history.lock().unwrap();
            for i in 0..10 {
                h.push_back(make_line("svc", &format!("line{i}")));
            }
        }
        let recent = agg.recent(None, 3);
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].line, "line7");
        assert_eq!(recent[2].line, "line9");
    }

    #[test]
    fn test_history_cap_evicts_oldest() {
        let (agg, _rx) = LogAggregator::new();
        {
            let mut h = agg.history.lock().unwrap();
            for i in 0..HISTORY_CAP + 5 {
                if h.len() >= HISTORY_CAP {
                    h.pop_front();
                }
                h.push_back(make_line("svc", &format!("line{i}")));
            }
        }
        let all = agg.recent(None, usize::MAX);
        assert_eq!(all.len(), HISTORY_CAP);
        assert_eq!(all[0].line, "line5");
    }

    #[test]
    fn test_recent_empty_history() {
        let (agg, _rx) = LogAggregator::new();
        assert_eq!(agg.recent(None, 10).len(), 0);
        assert_eq!(agg.recent(Some("svc"), 10).len(), 0);
    }
}
