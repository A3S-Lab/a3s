//! EventBus state persistence
//!
//! Provides pluggable persistence for subscription filters so they
//! survive process restarts. The `EventBus` auto-saves on changes
//! and auto-loads on creation when a `StateStore` is configured.

use crate::error::{EventError, Result};
use crate::types::SubscriptionFilter;
use std::collections::HashMap;
use std::path::{Path, PathBuf};

/// Trait for persisting EventBus subscription state
pub trait StateStore: Send + Sync {
    /// Save all subscription filters
    fn save(&self, subscriptions: &HashMap<String, SubscriptionFilter>) -> Result<()>;

    /// Load all subscription filters
    fn load(&self) -> Result<HashMap<String, SubscriptionFilter>>;
}

/// JSON file-based state store
///
/// Persists subscription filters as a JSON file on disk.
/// Atomic writes via temp file + rename to prevent corruption.
pub struct FileStateStore {
    path: PathBuf,
}

impl FileStateStore {
    /// Create a new file state store at the given path
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    /// Get the file path
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl StateStore for FileStateStore {
    fn save(&self, subscriptions: &HashMap<String, SubscriptionFilter>) -> Result<()> {
        let json = serde_json::to_string_pretty(subscriptions)?;

        // Atomic write: write to temp file, then rename
        let tmp_path = self.path.with_extension("tmp");

        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| {
                EventError::Config(format!(
                    "Failed to create state directory {}: {}",
                    parent.display(),
                    e
                ))
            })?;
        }

        std::fs::write(&tmp_path, json).map_err(|e| {
            EventError::Config(format!(
                "Failed to write state file {}: {}",
                tmp_path.display(),
                e
            ))
        })?;

        std::fs::rename(&tmp_path, &self.path).map_err(|e| {
            EventError::Config(format!(
                "Failed to rename state file {} → {}: {}",
                tmp_path.display(),
                self.path.display(),
                e
            ))
        })?;

        tracing::debug!(path = %self.path.display(), "State saved");
        Ok(())
    }

    fn load(&self) -> Result<HashMap<String, SubscriptionFilter>> {
        if !self.path.exists() {
            return Ok(HashMap::new());
        }

        let json = std::fs::read_to_string(&self.path).map_err(|e| {
            EventError::Config(format!(
                "Failed to read state file {}: {}",
                self.path.display(),
                e
            ))
        })?;

        let subscriptions: HashMap<String, SubscriptionFilter> =
            serde_json::from_str(&json).map_err(|e| {
                EventError::Config(format!(
                    "Failed to parse state file {}: {}",
                    self.path.display(),
                    e
                ))
            })?;

        tracing::debug!(
            path = %self.path.display(),
            count = subscriptions.len(),
            "State loaded"
        );
        Ok(subscriptions)
    }
}

/// In-memory state store for testing
///
/// Stores state in memory — lost on drop, but useful for tests.
#[derive(Default)]
pub struct MemoryStateStore {
    state: std::sync::RwLock<HashMap<String, SubscriptionFilter>>,
}

impl StateStore for MemoryStateStore {
    fn save(&self, subscriptions: &HashMap<String, SubscriptionFilter>) -> Result<()> {
        let mut state = self.state.write().map_err(|e| {
            EventError::Config(format!("Failed to acquire state lock: {}", e))
        })?;
        *state = subscriptions.clone();
        Ok(())
    }

    fn load(&self) -> Result<HashMap<String, SubscriptionFilter>> {
        let state = self.state.read().map_err(|e| {
            EventError::Config(format!("Failed to acquire state lock: {}", e))
        })?;
        Ok(state.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::SubscriptionFilter;

    fn sample_filters() -> HashMap<String, SubscriptionFilter> {
        let mut map = HashMap::new();
        map.insert(
            "analyst".to_string(),
            SubscriptionFilter {
                subscriber_id: "analyst".to_string(),
                subjects: vec!["events.market.>".to_string()],
                durable: true,
                options: None,
            },
        );
        map.insert(
            "monitor".to_string(),
            SubscriptionFilter {
                subscriber_id: "monitor".to_string(),
                subjects: vec!["events.system.>".to_string()],
                durable: false,
                options: None,
            },
        );
        map
    }

    #[test]
    fn test_memory_store_save_load() {
        let store = MemoryStateStore::default();
        let filters = sample_filters();

        store.save(&filters).unwrap();
        let loaded = store.load().unwrap();

        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded["analyst"].subscriber_id, "analyst");
        assert!(loaded["analyst"].durable);
        assert_eq!(loaded["monitor"].subjects, vec!["events.system.>"]);
    }

    #[test]
    fn test_memory_store_empty_load() {
        let store = MemoryStateStore::default();
        let loaded = store.load().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_memory_store_overwrite() {
        let store = MemoryStateStore::default();
        let filters = sample_filters();
        store.save(&filters).unwrap();

        let mut updated = HashMap::new();
        updated.insert(
            "new-sub".to_string(),
            SubscriptionFilter {
                subscriber_id: "new-sub".to_string(),
                subjects: vec!["events.>".to_string()],
                durable: true,
                options: None,
            },
        );
        store.save(&updated).unwrap();

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded.contains_key("new-sub"));
    }

    #[test]
    fn test_file_store_save_load() {
        let dir = std::env::temp_dir().join(format!("a3s-event-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("state.json");

        let store = FileStateStore::new(&path);
        let filters = sample_filters();

        store.save(&filters).unwrap();
        assert!(path.exists());

        let loaded = store.load().unwrap();
        assert_eq!(loaded.len(), 2);
        assert_eq!(loaded["analyst"].subscriber_id, "analyst");

        // Verify JSON is human-readable
        let content = std::fs::read_to_string(&path).unwrap();
        assert!(content.contains("analyst"));

        std::fs::remove_dir_all(&dir).unwrap();
    }

    #[test]
    fn test_file_store_load_nonexistent() {
        let store = FileStateStore::new("/tmp/nonexistent-a3s-state.json");
        let loaded = store.load().unwrap();
        assert!(loaded.is_empty());
    }

    #[test]
    fn test_file_store_creates_parent_dirs() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-event-test-{}/nested/deep",
            uuid::Uuid::new_v4()
        ));
        let path = dir.join("state.json");

        let store = FileStateStore::new(&path);
        store.save(&HashMap::new()).unwrap();
        assert!(path.exists());

        std::fs::remove_dir_all(
            dir.parent().unwrap().parent().unwrap(),
        )
        .unwrap();
    }

    #[test]
    fn test_file_store_atomic_write() {
        let dir = std::env::temp_dir().join(format!("a3s-event-test-{}", uuid::Uuid::new_v4()));
        let path = dir.join("state.json");
        let store = FileStateStore::new(&path);

        // Save initial state
        let filters = sample_filters();
        store.save(&filters).unwrap();

        // Save again — tmp file should not linger
        store.save(&filters).unwrap();
        let tmp_path = path.with_extension("tmp");
        assert!(!tmp_path.exists());

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
