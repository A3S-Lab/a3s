//! Channel supervised restart with exponential backoff.
//!
//! Wraps each `ChannelAdapter` in a background task that automatically
//! restarts the adapter on failure. Backoff starts at `MIN_BACKOFF` and
//! doubles up to `MAX_BACKOFF`. If the adapter stays healthy for
//! `HEALTHY_THRESHOLD`, the backoff resets.

use crate::channels::{ChannelAdapter, ChannelEvent};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

/// Minimum backoff between restart attempts.
const MIN_BACKOFF: Duration = Duration::from_secs(2);
/// Maximum backoff cap.
const MAX_BACKOFF: Duration = Duration::from_secs(60);
/// If the adapter runs longer than this without failing, reset backoff.
const HEALTHY_THRESHOLD: Duration = Duration::from_secs(120);

/// Spawn a supervised background task for a channel adapter.
///
/// The task calls `adapter.start()` in a loop. On failure it waits with
/// exponential backoff before retrying. A `ChannelEvent::Error` is sent
/// on each failure so the audit pipeline can record it.
pub fn spawn_supervised(adapter: Arc<dyn ChannelAdapter>, event_tx: mpsc::Sender<ChannelEvent>) {
    let name = adapter.name().to_string();
    tokio::spawn(async move {
        let mut backoff = MIN_BACKOFF;
        loop {
            let started = tokio::time::Instant::now();
            tracing::info!(channel = %name, "Starting channel adapter");

            match adapter.start(event_tx.clone()).await {
                Ok(()) => {
                    tracing::info!(channel = %name, "Channel adapter exited cleanly");
                    break;
                }
                Err(e) => {
                    let elapsed = started.elapsed();
                    // Reset backoff if the adapter was healthy long enough
                    if elapsed >= HEALTHY_THRESHOLD {
                        backoff = MIN_BACKOFF;
                    }

                    tracing::error!(
                        channel = %name,
                        error = %e,
                        backoff_secs = backoff.as_secs(),
                        "Channel adapter failed, restarting after backoff"
                    );

                    // Notify audit pipeline
                    let _ = event_tx
                        .send(ChannelEvent::Error {
                            channel: name.clone(),
                            error: e.to_string(),
                        })
                        .await;

                    tokio::time::sleep(backoff).await;

                    // Exponential backoff with cap
                    backoff = (backoff * 2).min(MAX_BACKOFF);
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_supervisor_constants() {
        assert!(MIN_BACKOFF < MAX_BACKOFF);
        assert!(HEALTHY_THRESHOLD > MAX_BACKOFF);
    }
}
