use std::path::PathBuf;
use std::time::Duration;

use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tokio::sync::mpsc;

const DEBOUNCE_MS: u64 = 500;

/// Watches a set of paths and sends the service name on the channel when a change is detected.
pub fn spawn_watcher(
    service: String,
    paths: Vec<PathBuf>,
    ignore: Vec<String>,
    tx: mpsc::Sender<String>,
) {
    std::thread::spawn(move || {
        let (raw_tx, raw_rx) = std::sync::mpsc::channel::<notify::Result<Event>>();
        let mut watcher = match RecommendedWatcher::new(raw_tx, notify::Config::default()) {
            Ok(w) => w,
            Err(e) => {
                tracing::error!("watcher init failed for {service}: {e}");
                return;
            }
        };

        for path in &paths {
            if let Err(e) = watcher.watch(path, RecursiveMode::Recursive) {
                tracing::warn!("cannot watch {}: {e}", path.display());
            }
        }

        let mut last_trigger = std::time::Instant::now()
            .checked_sub(Duration::from_millis(DEBOUNCE_MS + 1))
            .unwrap_or(std::time::Instant::now());

        for res in raw_rx {
            match res {
                Ok(event) => {
                    // Filter ignored patterns
                    let relevant = event.paths.iter().any(|p| {
                        let s = p.to_string_lossy();
                        !ignore.iter().any(|ig| s.contains(ig.as_str()))
                    });
                    if !relevant {
                        continue;
                    }
                    // Debounce
                    let now = std::time::Instant::now();
                    if now.duration_since(last_trigger) >= Duration::from_millis(DEBOUNCE_MS) {
                        last_trigger = now;
                        let _ = tx.blocking_send(service.clone());
                    }
                }
                Err(e) => tracing::warn!("watch error for {service}: {e}"),
            }
        }
    });
}
