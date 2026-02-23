//! Agent session persistence
//!
//! Stores agent session UI state to disk as JSON files with debounced writes.
//! Directory layout:
//! ```text
//! ~/.safeclaw/agent-sessions/ui-state/
//! ├── <session-uuid-1>.json      // PersistedAgentSession
//! └── <session-uuid-2>.json
//! ```

use crate::agent::types::PersistedAgentSession;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task::JoinHandle;

/// Agent session store with debounced file persistence
pub struct AgentSessionStore {
    dir: PathBuf,
    /// Per-session debounce handles
    debounce_handles: Arc<RwLock<HashMap<String, JoinHandle<()>>>>,
}

impl AgentSessionStore {
    /// Create a new session store at the given directory
    pub fn new(dir: PathBuf) -> Self {
        Self {
            dir,
            debounce_handles: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Default directory (~/.safeclaw/agent-sessions/)
    pub fn default_dir() -> PathBuf {
        dirs_next::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".safeclaw")
            .join("agent-sessions")
    }

    /// Ensure the storage directory exists
    pub async fn ensure_dir(&self) -> std::io::Result<()> {
        tokio::fs::create_dir_all(&self.dir).await
    }

    /// Save a session with 150ms debounce
    pub async fn save(&self, session: PersistedAgentSession) {
        let session_id = session.id.clone();

        // Cancel existing debounce timer for this session
        {
            let mut handles = self.debounce_handles.write().await;
            if let Some(handle) = handles.remove(&session_id) {
                handle.abort();
            }
        }

        let dir = self.dir.clone();
        let debounce_handles = self.debounce_handles.clone();
        let id = session_id.clone();

        let handle = tokio::spawn(async move {
            tokio::time::sleep(std::time::Duration::from_millis(150)).await;
            Self::write_session_file(&dir, &session);

            // Clean up handle reference
            let mut handles = debounce_handles.write().await;
            handles.remove(&id);
        });

        self.debounce_handles
            .write()
            .await
            .insert(session_id, handle);
    }

    /// Save a session immediately (for critical state changes)
    pub fn save_sync(&self, session: &PersistedAgentSession) {
        Self::write_session_file(&self.dir, session);
    }

    /// Load a single session by ID
    pub fn load(&self, session_id: &str) -> Option<PersistedAgentSession> {
        let path = self.session_path(session_id);
        Self::read_session_file(&path)
    }

    /// Load all sessions from disk
    pub fn load_all(&self) -> Vec<PersistedAgentSession> {
        let mut sessions = Vec::new();
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(e) => {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!(
                        "Failed to read agent sessions directory {}: {}",
                        self.dir.display(),
                        e
                    );
                }
                return sessions;
            }
        };

        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            if let Some(session) = Self::read_session_file(&path) {
                sessions.push(session);
            }
        }

        sessions
    }

    /// Remove a session from disk
    pub async fn remove(&self, session_id: &str) {
        // Cancel any pending debounce
        {
            let mut handles = self.debounce_handles.write().await;
            if let Some(handle) = handles.remove(session_id) {
                handle.abort();
            }
        }

        let path = self.session_path(session_id);
        if let Err(e) = tokio::fs::remove_file(&path).await {
            if e.kind() != std::io::ErrorKind::NotFound {
                tracing::warn!("Failed to remove session file {}: {}", path.display(), e);
            }
        }
    }

    /// Get the file path for a session
    fn session_path(&self, session_id: &str) -> PathBuf {
        self.dir.join(format!("{}.json", session_id))
    }

    /// Write a session to disk (blocking, used internally)
    fn write_session_file(dir: &Path, session: &PersistedAgentSession) {
        if let Err(e) = std::fs::create_dir_all(dir) {
            tracing::warn!(
                "Failed to create sessions directory {}: {}",
                dir.display(),
                e
            );
            return;
        }
        let path = dir.join(format!("{}.json", session.id));
        match serde_json::to_string_pretty(session) {
            Ok(json) => {
                if let Err(e) = std::fs::write(&path, json) {
                    tracing::warn!("Failed to write session {}: {}", session.id, e);
                }
            }
            Err(e) => {
                tracing::warn!("Failed to serialize session {}: {}", session.id, e);
            }
        }
    }

    /// Read a session from disk (blocking, used internally)
    fn read_session_file(path: &Path) -> Option<PersistedAgentSession> {
        let data = std::fs::read_to_string(path)
            .map_err(|e| {
                if e.kind() != std::io::ErrorKind::NotFound {
                    tracing::warn!("Failed to read session file {}: {}", path.display(), e);
                }
            })
            .ok()?;

        serde_json::from_str(&data)
            .map_err(|e| {
                tracing::warn!("Failed to parse session file {}: {}", path.display(), e);
            })
            .ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::agent::types::AgentSessionState;
    use std::collections::HashMap;
    use tempfile::TempDir;

    fn make_test_session(id: &str) -> PersistedAgentSession {
        PersistedAgentSession {
            id: id.to_string(),
            state: AgentSessionState::new(id.to_string()),
            message_history: Vec::new(),
            pending_messages: Vec::new(),
            pending_permissions: HashMap::new(),
            archived: false,
        }
    }

    #[test]
    fn test_save_sync_and_load() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path()).unwrap();

        let session = make_test_session("test-1");
        store.save_sync(&session);

        let loaded = store.load("test-1");
        assert!(loaded.is_some());
        let loaded = loaded.unwrap();
        assert_eq!(loaded.id, "test-1");
        assert!(!loaded.archived);
    }

    #[test]
    fn test_load_nonexistent_returns_none() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());

        assert!(store.load("nonexistent").is_none());
    }

    #[test]
    fn test_load_all() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path()).unwrap();

        store.save_sync(&make_test_session("s1"));
        store.save_sync(&make_test_session("s2"));
        store.save_sync(&make_test_session("s3"));

        let all = store.load_all();
        assert_eq!(all.len(), 3);
    }

    #[test]
    fn test_load_all_skips_corrupt_files() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path()).unwrap();

        store.save_sync(&make_test_session("good"));
        std::fs::write(dir.path().join("bad.json"), "not valid json").unwrap();

        let all = store.load_all();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].id, "good");
    }

    #[tokio::test]
    async fn test_remove_session() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());
        std::fs::create_dir_all(dir.path()).unwrap();

        store.save_sync(&make_test_session("to-delete"));
        assert!(store.load("to-delete").is_some());

        store.remove("to-delete").await;
        assert!(store.load("to-delete").is_none());
    }

    #[tokio::test]
    async fn test_remove_nonexistent_is_ok() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());
        // Should not panic
        store.remove("nonexistent").await;
    }

    #[tokio::test]
    async fn test_debounced_save() {
        let dir = TempDir::new().unwrap();
        let store = AgentSessionStore::new(dir.path().to_path_buf());
        tokio::fs::create_dir_all(dir.path()).await.unwrap();

        // Multiple rapid saves should debounce
        let mut session = make_test_session("debounce-test");
        store.save(session.clone()).await;
        session.archived = true;
        store.save(session.clone()).await;

        // Wait for debounce to complete
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;

        let loaded = store.load("debounce-test").unwrap();
        // Should have the last value
        assert!(loaded.archived);
    }

    #[test]
    fn test_default_dir() {
        let dir = AgentSessionStore::default_dir();
        assert!(dir.to_string_lossy().contains("agent-sessions"));
    }
}
