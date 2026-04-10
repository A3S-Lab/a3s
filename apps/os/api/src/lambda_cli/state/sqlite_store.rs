//! SQLite-based state store for the control plane.
//!
//! Similar to K3s's embedded SQLite, this provides:
//! - ACID transactions
//! - Crash-safe storage
//! - Atomic updates with resource versions
//! - Event history for watch

use crate::errors::{A3sError, Result};
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// State store directory.
const STATE_DIR: &str = ".a3s/state";
const DB_NAME: &str = "state.db";

/// Event type for watch.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum WatchEventType {
    Added,
    Modified,
    Deleted,
}

/// Watch event stored in events table.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent {
    pub event_type: WatchEventType,
    pub object: serde_json::Value,
    pub timestamp: String,
}

/// SQLite-backed state store.
#[derive(Debug, Clone)]
pub struct SqliteStateStore {
    conn: Arc<Mutex<Connection>>,
    path: PathBuf,
}

impl SqliteStateStore {
    /// Create a new SQLite state store.
    pub fn new(working_dir: &PathBuf) -> Result<Self> {
        let path = working_dir.join(STATE_DIR);
        std::fs::create_dir_all(&path)
            .map_err(|e| A3sError::Project(format!("failed to create state directory: {}", e)))?;

        let db_path = path.join(DB_NAME);
        let conn = Connection::open(&db_path)
            .map_err(|e| A3sError::Project(format!("failed to open database: {}", e)))?;

        let store = Self {
            conn: Arc::new(Mutex::new(conn)),
            path,
        };

        store.init_schema()?;
        Ok(store)
    }

    /// Initialize database schema.
    fn init_schema(&self) -> Result<()> {
        let conn = self.conn.lock().unwrap();
        conn.execute_batch(
            r#"
            CREATE TABLE IF NOT EXISTS resources (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                namespace TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                value TEXT NOT NULL,
                resource_version INTEGER NOT NULL DEFAULT 1,
                created_at TEXT NOT NULL,
                updated_at TEXT NOT NULL,
                UNIQUE(kind, namespace, name)
            );

            CREATE TABLE IF NOT EXISTS events (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                kind TEXT NOT NULL,
                namespace TEXT NOT NULL DEFAULT '',
                name TEXT NOT NULL,
                event_type TEXT NOT NULL,
                value TEXT NOT NULL,
                timestamp TEXT NOT NULL
            );

            CREATE INDEX IF NOT EXISTS idx_resources_kind_namespace
                ON resources(kind, namespace);
            CREATE INDEX IF NOT EXISTS idx_resources_name
                ON resources(name);
            CREATE INDEX IF NOT EXISTS idx_events_timestamp
                ON events(timestamp);
            "#,
        )
        .map_err(|e| A3sError::Project(format!("failed to initialize schema: {}", e)))?;
        Ok(())
    }

    /// Get a resource by kind, namespace, and name.
    pub async fn get(
        &self,
        kind: &str,
        namespace: &str,
        name: &str,
    ) -> Result<Option<(String, serde_json::Value)>> {
        let conn = self.conn.lock().unwrap();
        let mut stmt = conn
            .prepare(
                "SELECT value, resource_version FROM resources
                 WHERE kind = ?1 AND namespace = ?2 AND name = ?3",
            )
            .map_err(|e| A3sError::Project(format!("prepare failed: {}", e)))?;

        let result = stmt
            .query_row(params![kind, namespace, name], |row| {
                let value: String = row.get(0)?;
                let rv: i64 = row.get(1)?;
                Ok((value, rv))
            })
            .optional()
            .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;

        match result {
            Some((value, rv)) => {
                let json: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| A3sError::Project(format!("deserialize failed: {}", e)))?;
                Ok(Some((format!("{}", rv), json)))
            }
            None => Ok(None),
        }
    }

    /// List resources by kind and optional namespace.
    pub async fn list(
        &self,
        kind: &str,
        namespace: Option<&str>,
    ) -> Result<Vec<(String, String, serde_json::Value)>> {
        let conn = self.conn.lock().unwrap();

        let mut results = Vec::new();

        if let Some(ns) = namespace {
            let mut stmt = conn
                .prepare(
                    "SELECT namespace, name, value, resource_version FROM resources
                     WHERE kind = ?1 AND namespace = ?2 ORDER BY name",
                )
                .map_err(|e| A3sError::Project(format!("prepare failed: {}", e)))?;

            let rows = stmt
                .query_map(params![kind, ns], |row| {
                    let ns: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let value: String = row.get(2)?;
                    let rv: i64 = row.get(3)?;
                    Ok((ns, name, value, rv))
                })
                .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;

            for row in rows {
                let (ns, _name, value, rv) =
                    row.map_err(|e| A3sError::Project(format!("row failed: {}", e)))?;
                let json: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| A3sError::Project(format!("deserialize failed: {}", e)))?;
                results.push((ns, format!("{}", rv), json));
            }
        } else {
            let mut stmt = conn
                .prepare(
                    "SELECT namespace, name, value, resource_version FROM resources
                     WHERE kind = ?1 ORDER BY namespace, name",
                )
                .map_err(|e| A3sError::Project(format!("prepare failed: {}", e)))?;

            let rows = stmt
                .query_map(params![kind], |row| {
                    let ns: String = row.get(0)?;
                    let name: String = row.get(1)?;
                    let value: String = row.get(2)?;
                    let rv: i64 = row.get(3)?;
                    Ok((ns, name, value, rv))
                })
                .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;

            for row in rows {
                let (ns, _name, value, rv) =
                    row.map_err(|e| A3sError::Project(format!("row failed: {}", e)))?;
                let json: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| A3sError::Project(format!("deserialize failed: {}", e)))?;
                results.push((ns, format!("{}", rv), json));
            }
        }

        Ok(results)
    }

    /// Create or update a resource.
    pub async fn upsert(
        &self,
        kind: &str,
        namespace: &str,
        name: &str,
        value: &serde_json::Value,
    ) -> Result<String> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Try to get current resource version
        let current_rv: i64 = conn
            .query_row(
                "SELECT resource_version FROM resources
                 WHERE kind = ?1 AND namespace = ?2 AND name = ?3",
                params![kind, namespace, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?
            .unwrap_or(0);

        let new_rv = current_rv + 1;
        let value_str = serde_json::to_string(value)
            .map_err(|e| A3sError::Project(format!("serialize failed: {}", e)))?;

        // Use INSERT OR REPLACE for upsert
        conn.execute(
            r#"
            INSERT INTO resources (kind, namespace, name, value, resource_version, created_at, updated_at)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
            ON CONFLICT(kind, namespace, name) DO UPDATE SET
                value = excluded.value,
                resource_version = excluded.resource_version,
                updated_at = excluded.updated_at
            "#,
            params![kind, namespace, name, value_str, new_rv, now, now],
        )
        .map_err(|e| A3sError::Project(format!("upsert failed: {}", e)))?;

        // Record event
        let event_type = if current_rv == 0 { "ADDED" } else { "MODIFIED" };

        conn.execute(
            r#"
            INSERT INTO events (kind, namespace, name, event_type, value, timestamp)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6)
            "#,
            params![kind, namespace, name, event_type, value_str, now],
        )
        .map_err(|e| A3sError::Project(format!("event insert failed: {}", e)))?;

        Ok(format!("{}", new_rv))
    }

    /// Delete a resource.
    pub async fn delete(&self, kind: &str, namespace: &str, name: &str) -> Result<bool> {
        let conn = self.conn.lock().unwrap();
        let now = chrono::Utc::now().to_rfc3339();

        // Get value before deleting for event
        let value: Option<String> = conn
            .query_row(
                "SELECT value FROM resources
                 WHERE kind = ?1 AND namespace = ?2 AND name = ?3",
                params![kind, namespace, name],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;

        if let Some(value_str) = value {
            // Record DELETE event
            conn.execute(
                r#"
                INSERT INTO events (kind, namespace, name, event_type, value, timestamp)
                VALUES (?1, ?2, ?3, 'DELETED', ?4, ?5)
                "#,
                params![kind, namespace, name, value_str, now],
            )
            .map_err(|e| A3sError::Project(format!("event insert failed: {}", e)))?;

            // Delete the resource
            let rows_affected = conn
                .execute(
                    "DELETE FROM resources WHERE kind = ?1 AND namespace = ?2 AND name = ?3",
                    params![kind, namespace, name],
                )
                .map_err(|e| A3sError::Project(format!("delete failed: {}", e)))?;

            Ok(rows_affected > 0)
        } else {
            Ok(false)
        }
    }

    /// Watch for changes since a given timestamp.
    pub async fn watch_since(&self, since_timestamp: Option<&str>) -> Result<Vec<WatchEvent>> {
        let conn = self.conn.lock().unwrap();
        let mut events = Vec::new();

        if let Some(ts) = since_timestamp {
            let mut stmt = conn
                .prepare(
                    "SELECT event_type, value, timestamp FROM events WHERE timestamp > ?1 ORDER BY id",
                )
                .map_err(|e| A3sError::Project(format!("prepare failed: {}", e)))?;

            let rows = stmt
                .query_map(params![ts], |row| {
                    let event_type: String = row.get(0)?;
                    let value: String = row.get(1)?;
                    let timestamp: String = row.get(2)?;
                    Ok((event_type, value, timestamp))
                })
                .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;

            for row in rows {
                let (event_type_str, value, timestamp) =
                    row.map_err(|e| A3sError::Project(format!("row failed: {}", e)))?;

                let event_type = match event_type_str.as_str() {
                    "ADDED" => WatchEventType::Added,
                    "MODIFIED" => WatchEventType::Modified,
                    "DELETED" => WatchEventType::Deleted,
                    _ => continue,
                };

                let object: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| A3sError::Project(format!("deserialize failed: {}", e)))?;

                events.push(WatchEvent {
                    event_type,
                    object,
                    timestamp,
                });
            }
        } else {
            let mut stmt = conn
                .prepare("SELECT event_type, value, timestamp FROM events ORDER BY id")
                .map_err(|e| A3sError::Project(format!("prepare failed: {}", e)))?;

            let rows = stmt
                .query_map([], |row| {
                    let event_type: String = row.get(0)?;
                    let value: String = row.get(1)?;
                    let timestamp: String = row.get(2)?;
                    Ok((event_type, value, timestamp))
                })
                .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;

            for row in rows {
                let (event_type_str, value, timestamp) =
                    row.map_err(|e| A3sError::Project(format!("row failed: {}", e)))?;

                let event_type = match event_type_str.as_str() {
                    "ADDED" => WatchEventType::Added,
                    "MODIFIED" => WatchEventType::Modified,
                    "DELETED" => WatchEventType::Deleted,
                    _ => continue,
                };

                let object: serde_json::Value = serde_json::from_str(&value)
                    .map_err(|e| A3sError::Project(format!("deserialize failed: {}", e)))?;

                events.push(WatchEvent {
                    event_type,
                    object,
                    timestamp,
                });
            }
        }

        Ok(events)
    }

    /// Get the latest event timestamp.
    pub async fn get_latest_event_timestamp(&self) -> Result<Option<String>> {
        let conn = self.conn.lock().unwrap();
        let result: Option<String> = conn
            .query_row(
                "SELECT timestamp FROM events ORDER BY id DESC LIMIT 1",
                [],
                |row| row.get(0),
            )
            .optional()
            .map_err(|e| A3sError::Project(format!("query failed: {}", e)))?;
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_store() -> SqliteStateStore {
        let dir = std::env::temp_dir().join(format!("a3s-sqlite-test-{}", uuid::Uuid::new_v4()));
        SqliteStateStore::new(&dir).unwrap()
    }

    #[tokio::test]
    async fn test_crud() {
        let store = temp_store();

        // Create
        let value = serde_json::json!({
            "metadata": {"name": "test"},
            "spec": {"replicas": 3}
        });
        let rv = store
            .upsert("Deployment", "default", "test", &value)
            .await
            .unwrap();
        assert_eq!(rv, "1");

        // Read
        let (got_rv, got_value) = store
            .get("Deployment", "default", "test")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(got_rv, "1");
        assert_eq!(got_value, value);

        // Update
        let value2 = serde_json::json!({
            "metadata": {"name": "test"},
            "spec": {"replicas": 5}
        });
        let rv2 = store
            .upsert("Deployment", "default", "test", &value2)
            .await
            .unwrap();
        assert_eq!(rv2, "2");

        // List
        let items = store.list("Deployment", Some("default")).await.unwrap();
        assert_eq!(items.len(), 1);

        // Delete
        let deleted = store.delete("Deployment", "default", "test").await.unwrap();
        assert!(deleted);

        // Verify deleted
        let got = store.get("Deployment", "default", "test").await.unwrap();
        assert!(got.is_none());
    }

    #[tokio::test]
    async fn test_watch() {
        let store = temp_store();

        // Create
        let value = serde_json::json!({"metadata": {"name": "test"}});
        store
            .upsert("Deployment", "default", "test", &value)
            .await
            .unwrap();

        // Watch from beginning
        let events = store.watch_since(None).await.unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].event_type, WatchEventType::Added));

        // Update
        let value2 = serde_json::json!({"metadata": {"name": "test2"}});
        store
            .upsert("Deployment", "default", "test", &value2)
            .await
            .unwrap();

        // Watch from first event
        let timestamp = events[0].timestamp.clone();
        let events2 = store.watch_since(Some(&timestamp)).await.unwrap();
        assert_eq!(events2.len(), 1);
        assert!(matches!(events2[0].event_type, WatchEventType::Modified));
    }
}
