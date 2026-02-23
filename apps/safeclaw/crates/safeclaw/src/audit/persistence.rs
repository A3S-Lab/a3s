//! JSONL-based audit event persistence with file rotation.
//!
//! Appends audit events to a `.jsonl` file for crash-resilient storage.
//! When the file exceeds `max_file_bytes`, it is rotated (renamed with a
//! timestamp suffix) and a fresh file is started. Old rotated files are
//! pruned to keep at most `max_rotated_files`.

use crate::audit::{AuditEvent, AuditSeverity, LeakageVector};
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};

/// Configuration for audit persistence.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct PersistenceConfig {
    /// Whether persistence is enabled.
    pub enabled: bool,
    /// Subdirectory under the base storage dir (default: `"audit"`).
    pub dir: String,
    /// Maximum size of the active JSONL file before rotation (bytes).
    pub max_file_bytes: u64,
    /// Maximum number of rotated files to keep.
    pub max_rotated_files: usize,
    /// Retention period in days. Rotated files older than this are deleted.
    /// Set to 0 to disable time-based retention (only count-based).
    pub retention_days: u32,
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            dir: "audit".to_string(),
            max_file_bytes: 10 * 1024 * 1024, // 10 MB
            max_rotated_files: 5,
            retention_days: 90,
        }
    }
}

/// Append-only JSONL audit persistence.
pub struct AuditPersistence {
    /// Directory containing audit files.
    dir: PathBuf,
    /// Path to the active JSONL file.
    active_path: PathBuf,
    /// Configuration.
    config: PersistenceConfig,
}

impl AuditPersistence {
    /// Create a new persistence instance, ensuring the directory exists.
    pub async fn new(base_dir: &Path, config: PersistenceConfig) -> crate::error::Result<Self> {
        let dir = base_dir.join(&config.dir);
        fs::create_dir_all(&dir).await.map_err(|e| {
            crate::error::Error::Internal(format!(
                "Failed to create audit directory {}: {}",
                dir.display(),
                e
            ))
        })?;
        let active_path = dir.join("events.jsonl");
        Ok(Self {
            dir,
            active_path,
            config,
        })
    }

    /// Append a single event to the active JSONL file.
    pub async fn append(&self, event: &AuditEvent) -> crate::error::Result<()> {
        self.maybe_rotate().await?;
        let mut line = serde_json::to_string(event).map_err(|e| {
            crate::error::Error::Internal(format!("Failed to serialize audit event: {}", e))
        })?;
        line.push('\n');

        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.active_path)
            .await
            .map_err(|e| {
                crate::error::Error::Internal(format!(
                    "Failed to open audit file {}: {}",
                    self.active_path.display(),
                    e
                ))
            })?;
        file.write_all(line.as_bytes()).await.map_err(|e| {
            crate::error::Error::Internal(format!("Failed to write audit event: {}", e))
        })?;
        Ok(())
    }

    /// Load all persisted events from the active file.
    pub async fn load_all(&self) -> Vec<AuditEvent> {
        self.load_from_file(&self.active_path).await
    }

    /// Load the most recent `limit` events from the active file.
    pub async fn load_recent(&self, limit: usize) -> Vec<AuditEvent> {
        let all = self.load_all().await;
        if all.len() <= limit {
            all
        } else {
            all[all.len() - limit..].to_vec()
        }
    }

    /// Rotate the active file if it exceeds `max_file_bytes`.
    async fn maybe_rotate(&self) -> crate::error::Result<()> {
        let meta = match fs::metadata(&self.active_path).await {
            Ok(m) => m,
            Err(_) => return Ok(()), // file doesn't exist yet
        };
        if meta.len() < self.config.max_file_bytes {
            return Ok(());
        }

        // Rename active file with timestamp suffix + counter to avoid collisions
        let ts = chrono::Utc::now().format("%Y%m%dT%H%M%S%.6f").to_string();
        let mut rotated = self.dir.join(format!("events-{}.jsonl", ts));
        // If the file already exists (sub-microsecond collision), append a counter
        let mut counter = 1u32;
        while rotated.exists() {
            rotated = self.dir.join(format!("events-{}-{}.jsonl", ts, counter));
            counter += 1;
        }
        fs::rename(&self.active_path, &rotated).await.map_err(|e| {
            crate::error::Error::Internal(format!("Failed to rotate audit file: {}", e))
        })?;

        // Prune old rotated files
        self.prune_rotated().await?;
        Ok(())
    }

    /// Delete oldest rotated files if we exceed `max_rotated_files`.
    async fn prune_rotated(&self) -> crate::error::Result<()> {
        let mut rotated: Vec<PathBuf> = Vec::new();
        let mut entries = fs::read_dir(&self.dir).await.map_err(|e| {
            crate::error::Error::Internal(format!("Failed to read audit dir: {}", e))
        })?;
        while let Some(entry) = entries.next_entry().await.map_err(|e| {
            crate::error::Error::Internal(format!("Failed to read dir entry: {}", e))
        })? {
            let name: String = entry.file_name().to_string_lossy().to_string();
            if name.starts_with("events-") && name.ends_with(".jsonl") {
                rotated.push(entry.path());
            }
        }
        rotated.sort();

        // Time-based retention: delete files older than retention_days
        if self.config.retention_days > 0 {
            let cutoff =
                chrono::Utc::now() - chrono::Duration::days(self.config.retention_days as i64);
            rotated.retain(|path| {
                if let Some(mtime) = file_modified_time(path) {
                    if mtime < cutoff {
                        let _ = std::fs::remove_file(path);
                        tracing::debug!("Pruned expired audit file: {}", path.display());
                        return false;
                    }
                }
                true
            });
        }

        // Count-based retention: keep at most max_rotated_files
        while rotated.len() > self.config.max_rotated_files {
            if let Some(oldest) = rotated.first().cloned() {
                let _ = fs::remove_file(&oldest).await;
                rotated.remove(0);
            }
        }
        Ok(())
    }

    /// Read events from a single JSONL file.
    async fn load_from_file(&self, path: &Path) -> Vec<AuditEvent> {
        let file = match fs::File::open(path).await {
            Ok(f) => f,
            Err(_) => return Vec::new(),
        };
        let reader = BufReader::new(file);
        let mut lines = reader.lines();
        let mut events = Vec::new();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(event) = serde_json::from_str::<AuditEvent>(&line) {
                events.push(event);
            }
        }
        events
    }

    /// Query persisted events with filters.
    pub async fn query(&self, filter: &AuditQueryFilter) -> Vec<AuditEvent> {
        let all = self.load_all().await;
        all.into_iter().filter(|e| filter.matches(e)).collect()
    }

    /// Export all events (active + rotated) as a single sorted stream.
    pub async fn export_all(&self) -> Vec<AuditEvent> {
        let mut all_events = Vec::new();

        // Load from rotated files first (oldest to newest)
        if let Ok(mut entries) = fs::read_dir(&self.dir).await {
            let mut rotated_paths: Vec<PathBuf> = Vec::new();
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name().to_string_lossy().to_string();
                if name.starts_with("events-") && name.ends_with(".jsonl") {
                    rotated_paths.push(entry.path());
                }
            }
            rotated_paths.sort();
            for path in rotated_paths {
                all_events.extend(self.load_from_file(&path).await);
            }
        }

        // Then load active file
        all_events.extend(self.load_from_file(&self.active_path).await);
        all_events
    }
}

/// Filter for querying persisted audit events.
#[derive(Debug, Default, Clone)]
pub struct AuditQueryFilter {
    /// Filter by session ID.
    pub session_id: Option<String>,
    /// Filter by severity.
    pub severity: Option<AuditSeverity>,
    /// Filter by leakage vector.
    pub vector: Option<LeakageVector>,
    /// Filter by time range start (epoch ms, inclusive).
    pub from_ms: Option<i64>,
    /// Filter by time range end (epoch ms, inclusive).
    pub to_ms: Option<i64>,
    /// Text search in description (case-insensitive).
    pub search: Option<String>,
    /// Maximum results to return.
    pub limit: Option<usize>,
}

impl AuditQueryFilter {
    /// Check if an event matches this filter.
    pub fn matches(&self, event: &AuditEvent) -> bool {
        if let Some(ref sid) = self.session_id {
            if event.session_id != *sid {
                return false;
            }
        }
        if let Some(ref sev) = self.severity {
            if event.severity != *sev {
                return false;
            }
        }
        if let Some(ref vec) = self.vector {
            if event.vector != *vec {
                return false;
            }
        }
        if let Some(from) = self.from_ms {
            if event.timestamp < from {
                return false;
            }
        }
        if let Some(to) = self.to_ms {
            if event.timestamp > to {
                return false;
            }
        }
        if let Some(ref q) = self.search {
            if !event.description.to_lowercase().contains(&q.to_lowercase()) {
                return false;
            }
        }
        true
    }
}

/// Get file modification time as a chrono DateTime.
fn file_modified_time(path: &Path) -> Option<chrono::DateTime<chrono::Utc>> {
    let meta = std::fs::metadata(path).ok()?;
    let modified = meta.modified().ok()?;
    Some(chrono::DateTime::from(modified))
}

/// Spawn a background task that subscribes to the audit bus and persists events.
pub fn spawn_persistence_subscriber(
    mut rx: tokio::sync::broadcast::Receiver<AuditEvent>,
    persistence: std::sync::Arc<AuditPersistence>,
) {
    tokio::spawn(async move {
        loop {
            match rx.recv().await {
                Ok(event) => {
                    if let Err(e) = persistence.append(&event).await {
                        tracing::error!("Audit persistence write failed: {}", e);
                    }
                }
                Err(tokio::sync::broadcast::error::RecvError::Lagged(n)) => {
                    tracing::warn!("Audit persistence subscriber lagged, missed {} events", n);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    tracing::info!("Audit bus closed, persistence subscriber exiting");
                    break;
                }
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditSeverity, LeakageVector};

    fn make_event(detail: &str) -> AuditEvent {
        AuditEvent::new(
            "test-session".to_string(),
            AuditSeverity::Warning,
            LeakageVector::OutputChannel,
            detail.to_string(),
        )
    }

    #[tokio::test]
    async fn test_append_and_load() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig {
            enabled: true,
            dir: "audit".to_string(),
            max_file_bytes: 10 * 1024 * 1024,
            max_rotated_files: 5,
            retention_days: 90,
        };
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        let event = make_event("test event 1");
        p.append(&event).await.unwrap();

        let loaded = p.load_all().await;
        assert_eq!(loaded.len(), 1);
        assert_eq!(loaded[0].description, "test event 1");
    }

    #[tokio::test]
    async fn test_load_recent() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        for i in 0..10 {
            p.append(&make_event(&format!("event {}", i)))
                .await
                .unwrap();
        }

        let recent = p.load_recent(3).await;
        assert_eq!(recent.len(), 3);
        assert_eq!(recent[0].description, "event 7");
        assert_eq!(recent[2].description, "event 9");
    }

    #[tokio::test]
    async fn test_load_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        let loaded = p.load_all().await;
        assert!(loaded.is_empty());
    }

    #[tokio::test]
    async fn test_rotation() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig {
            enabled: true,
            dir: "audit".to_string(),
            max_file_bytes: 200, // tiny threshold to trigger rotation
            max_rotated_files: 2,
            ..Default::default()
        };
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        // Write enough events to trigger rotation
        for i in 0..20 {
            p.append(&make_event(&format!("rotation event {}", i)))
                .await
                .unwrap();
        }

        // Active file should exist and rotated files should be pruned
        let mut count = 0;
        let mut entries = fs::read_dir(tmp.path().join("audit")).await.unwrap();
        while let Some(_) = entries.next_entry().await.unwrap() {
            count += 1;
        }
        // At most max_rotated_files + 1 (active)
        assert!(count <= 3, "Expected at most 3 files, got {}", count);
    }

    #[tokio::test]
    async fn test_query_by_session() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        p.append(&AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::High,
            LeakageVector::OutputChannel,
            "event A".to_string(),
        ))
        .await
        .unwrap();
        p.append(&AuditEvent::new(
            "sess-2".to_string(),
            AuditSeverity::Warning,
            LeakageVector::ToolCall,
            "event B".to_string(),
        ))
        .await
        .unwrap();

        let filter = AuditQueryFilter {
            session_id: Some("sess-1".to_string()),
            ..Default::default()
        };
        let results = p.query(&filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "event A");
    }

    #[tokio::test]
    async fn test_query_by_severity() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        p.append(&AuditEvent::new(
            "s1".to_string(),
            AuditSeverity::Critical,
            LeakageVector::ToolCall,
            "critical".to_string(),
        ))
        .await
        .unwrap();
        p.append(&AuditEvent::new(
            "s1".to_string(),
            AuditSeverity::Info,
            LeakageVector::OutputChannel,
            "info".to_string(),
        ))
        .await
        .unwrap();

        let filter = AuditQueryFilter {
            severity: Some(AuditSeverity::Critical),
            ..Default::default()
        };
        let results = p.query(&filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "critical");
    }

    #[tokio::test]
    async fn test_query_by_vector() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        p.append(&AuditEvent::new(
            "s1".to_string(),
            AuditSeverity::High,
            LeakageVector::NetworkExfil,
            "network".to_string(),
        ))
        .await
        .unwrap();
        p.append(&AuditEvent::new(
            "s1".to_string(),
            AuditSeverity::High,
            LeakageVector::ToolCall,
            "tool".to_string(),
        ))
        .await
        .unwrap();

        let filter = AuditQueryFilter {
            vector: Some(LeakageVector::NetworkExfil),
            ..Default::default()
        };
        let results = p.query(&filter).await;
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].description, "network");
    }

    #[tokio::test]
    async fn test_query_text_search() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        p.append(&make_event("blocked API key leak")).await.unwrap();
        p.append(&make_event("normal output check")).await.unwrap();
        p.append(&make_event("API key in tool args")).await.unwrap();

        let filter = AuditQueryFilter {
            search: Some("api key".to_string()),
            ..Default::default()
        };
        let results = p.query(&filter).await;
        assert_eq!(results.len(), 2);
    }

    #[tokio::test]
    async fn test_query_time_range() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig::default();
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        let now = chrono::Utc::now().timestamp_millis();
        p.append(&make_event("event 1")).await.unwrap();
        p.append(&make_event("event 2")).await.unwrap();

        // Query with from_ms = now (should include events created at/after now)
        let filter = AuditQueryFilter {
            from_ms: Some(now),
            ..Default::default()
        };
        let results = p.query(&filter).await;
        assert!(results.len() <= 2);

        // Query with to_ms = 0 (should include nothing)
        let filter = AuditQueryFilter {
            to_ms: Some(0),
            ..Default::default()
        };
        let results = p.query(&filter).await;
        assert!(results.is_empty());
    }

    #[tokio::test]
    async fn test_export_all_includes_rotated() {
        let tmp = tempfile::tempdir().unwrap();
        let config = PersistenceConfig {
            enabled: true,
            dir: "audit".to_string(),
            max_file_bytes: 200,
            max_rotated_files: 50, // enough to hold all rotated files
            retention_days: 90,
        };
        let p = AuditPersistence::new(tmp.path(), config).await.unwrap();

        for i in 0..20 {
            p.append(&make_event(&format!("export event {}", i)))
                .await
                .unwrap();
        }

        let all = p.export_all().await;
        assert_eq!(all.len(), 20);
    }

    #[tokio::test]
    async fn test_retention_days_default() {
        let config = PersistenceConfig::default();
        assert_eq!(config.retention_days, 90);
    }

    #[test]
    fn test_query_filter_matches() {
        let event = AuditEvent::new(
            "sess-1".to_string(),
            AuditSeverity::High,
            LeakageVector::ToolCall,
            "blocked dangerous tool".to_string(),
        );

        // Empty filter matches everything
        assert!(AuditQueryFilter::default().matches(&event));

        // Session match
        let f = AuditQueryFilter {
            session_id: Some("sess-1".to_string()),
            ..Default::default()
        };
        assert!(f.matches(&event));

        // Session mismatch
        let f = AuditQueryFilter {
            session_id: Some("sess-2".to_string()),
            ..Default::default()
        };
        assert!(!f.matches(&event));

        // Combined filter
        let f = AuditQueryFilter {
            session_id: Some("sess-1".to_string()),
            severity: Some(AuditSeverity::High),
            search: Some("dangerous".to_string()),
            ..Default::default()
        };
        assert!(f.matches(&event));
    }
}
