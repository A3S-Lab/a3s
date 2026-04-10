//! Watch/Notify - Real-time state change notifications.
//!
//! Provides a mechanism to watch for state changes on resources,
//! similar to `kubectl watch`.

use crate::errors::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio::time::Duration;

/// Event type for watched resources.
#[derive(Debug, Clone)]
pub enum WatchEvent<T: Clone> {
    /// Resource was added.
    Added(T),
    /// Resource was modified.
    Modified(T),
    /// Resource was deleted.
    Deleted(T),
    /// Error occurred while watching.
    Error(String),
}

/// A watch handle - allows canceling a watch.
#[derive(Debug, Clone)]
pub struct WatchHandle {
    id: u64,
    tx: broadcast::Sender<()>,
}

impl WatchHandle {
    /// Stop watching.
    pub fn stop(&self) {
        let _ = self.tx.send(());
    }
}

/// Watcher - manages subscriptions to resource changes.
pub struct Watcher {
    id_counter: tokio::sync::RwLock<u64>,
    subscriptions: tokio::sync::RwLock<HashMap<u64, broadcast::Sender<()>>>,
}

impl Watcher {
    pub fn new() -> Self {
        Self {
            id_counter: tokio::sync::RwLock::new(0),
            subscriptions: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Subscribe to stop signals for a watch.
    pub async fn subscribe(&self) -> (u64, broadcast::Receiver<()>) {
        let id = {
            let mut counter = self.id_counter.write().await;
            let id = *counter;
            *counter += 1;
            id
        };

        let (tx, rx) = broadcast::channel(1);
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.insert(id, tx);

        (id, rx)
    }

    /// Unsubscribe from a watch.
    pub async fn unsubscribe(&self, id: u64) {
        let mut subscriptions = self.subscriptions.write().await;
        subscriptions.remove(&id);
    }

    /// Notify all watchers to stop.
    pub async fn notify_all(&self) {
        let subscriptions = self.subscriptions.read().await;
        for (_, tx) in subscriptions.iter() {
            let _ = tx.send(());
        }
    }

    /// Notify a specific watcher to stop.
    pub async fn notify(&self, id: u64) {
        let subscriptions = self.subscriptions.read().await;
        if let Some(tx) = subscriptions.get(&id) {
            let _ = tx.send(());
        }
    }
}

impl Default for Watcher {
    fn default() -> Self {
        Self::new()
    }
}

/// State watch - provides a stream of state changes.
pub struct StateWatch<T: Clone> {
    /// Current state snapshot.
    current: std::sync::Arc<std::sync::Mutex<T>>,
    /// Broadcast channel for updates.
    tx: broadcast::Sender<WatchEvent<T>>,
}

impl<T: Clone> StateWatch<T> {
    /// Create a new state watch with initial state.
    pub fn new(initial: T) -> (Self, broadcast::Receiver<WatchEvent<T>>) {
        let (tx, rx) = broadcast::channel(100);
        let watch = Self {
            current: std::sync::Arc::new(std::sync::Mutex::new(initial)),
            tx,
        };
        (watch, rx)
    }

    /// Update the state and broadcast to all watchers.
    pub fn set(&self, new_state: T) {
        let event = WatchEvent::Modified(new_state.clone());
        *self.current.lock().unwrap() = new_state;
        let _ = self.tx.send(event);
    }

    /// Get current state snapshot.
    pub fn get(&self) -> T {
        self.current.lock().unwrap().clone()
    }

    /// Watch for state changes.
    pub fn watch(&self) -> broadcast::Receiver<WatchEvent<T>> {
        self.tx.subscribe()
    }
}

/// Trait for watchable resources.
pub trait Watchable<T: Clone + Send + Sync>: Send + Sync {
    /// Watch for state changes.
    fn watch(&self) -> broadcast::Receiver<WatchEvent<T>>;
}

/// Composite watcher - watches multiple resource types.
pub struct CompositeWatcher {
    watchers: HashMap<String, Arc<dyn Watchable<serde_json::Value> + Send + Sync>>,
}

impl CompositeWatcher {
    pub fn new() -> Self {
        Self {
            watchers: HashMap::new(),
        }
    }

    /// Register a watcher for a resource type.
    pub fn register(
        &mut self,
        resource_type: &str,
        watch: Arc<dyn Watchable<serde_json::Value> + Send + Sync>,
    ) {
        self.watchers.insert(resource_type.to_string(), watch);
    }
}

impl Default for CompositeWatcher {
    fn default() -> Self {
        Self::new()
    }
}

/// Retry policy for watch operations.
#[derive(Debug, Clone)]
pub struct RetryPolicy {
    /// Maximum number of retries.
    pub max_retries: u32,
    /// Initial backoff delay.
    pub initial_delay: Duration,
    /// Maximum backoff delay.
    pub max_delay: Duration,
    /// Backoff multiplier.
    pub multiplier: f64,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_retries: 5,
            initial_delay: Duration::from_secs(1),
            max_delay: Duration::from_secs(30),
            multiplier: 2.0,
        }
    }
}

impl RetryPolicy {
    /// Calculate the delay for a given attempt.
    pub fn backoff(&self, attempt: u32) -> Duration {
        let delay = self.initial_delay.as_secs_f64() * self.multiplier.powi(attempt as i32);
        Duration::from_secs_f64(delay.min(self.max_delay.as_secs_f64()))
    }
}

/// Backoff retry for watch operations.
pub async fn with_backoff<F, Fut, T>(policy: &RetryPolicy, mut f: F) -> Result<T>
where
    F: FnMut() -> Fut,
    Fut: std::future::Future<Output = Result<T>>,
{
    let mut attempt = 0u32;
    loop {
        match f().await {
            Ok(t) => return Ok(t),
            Err(e) if attempt < policy.max_retries => {
                let delay = policy.backoff(attempt);
                tracing::warn!(
                    attempt = attempt + 1,
                    delay_secs = delay.as_secs(),
                    error = %e,
                    "watch failed, retrying"
                );
                tokio::time::sleep(delay).await;
                attempt += 1;
            }
            Err(e) => return Err(e),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_watch_events() {
        let (watch, mut rx) = StateWatch::new(serde_json::json!({"count": 0}));

        // Update state
        watch.set(serde_json::json!({"count": 1}));

        // Should receive Modified event
        let event = rx.recv().await.unwrap();
        match event {
            WatchEvent::Modified(v) => {
                assert_eq!(v["count"], 1);
            }
            _ => panic!("expected Modified event"),
        }
    }

    #[tokio::test]
    async fn test_retry_policy_backoff() {
        let policy = RetryPolicy::default();

        assert_eq!(policy.backoff(0), Duration::from_secs(1));
        assert_eq!(policy.backoff(1), Duration::from_secs(2));
        assert_eq!(policy.backoff(2), Duration::from_secs(4));

        // Should cap at max_delay
        assert_eq!(policy.backoff(10), Duration::from_secs(30));
    }
}
