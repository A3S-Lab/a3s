//! Workflow persistence — SQLite-backed store for visual workflow documents.

pub mod engine;
pub mod nodes;

use anyhow::{Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

// =============================================================================
// Types
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDoc {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    /// Unix timestamp in milliseconds.
    pub created_at: i64,
    /// Unix timestamp in milliseconds.
    pub updated_at: i64,
    /// Serialized flowgram document (nodes + edges JSON).
    pub document: serde_json::Value,
    /// Agent that owns this workflow.
    pub agent_id: Option<String>,
    /// Session ID bound to this workflow.
    pub session_id: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct CreateWorkflow {
    pub name: String,
    pub description: Option<String>,
    /// Initial document; defaults to a bare start/end graph if absent.
    pub document: Option<serde_json::Value>,
}

#[derive(Debug, Deserialize)]
pub struct UpdateWorkflow {
    pub name: Option<String>,
    pub description: Option<String>,
    pub document: Option<serde_json::Value>,
    pub agent_id: Option<String>,
    pub session_id: Option<String>,
}

// =============================================================================
// Store
// =============================================================================

/// Thread-safe SQLite-backed workflow store.
///
/// Internally holds a `Mutex<Connection>` so it can be shared across Axum
/// request handlers.  All async methods delegate to `spawn_blocking` to avoid
/// blocking the Tokio executor.
#[derive(Clone)]
pub struct WorkflowStore {
    conn: Arc<Mutex<Connection>>,
}

impl WorkflowStore {
    /// Open (or create) the SQLite database at `db_path` and run migrations.
    pub fn open(db_path: PathBuf) -> Result<Self> {
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create db dir {}", parent.display()))?;
        }
        let conn = Connection::open(&db_path)
            .with_context(|| format!("open sqlite db {}", db_path.display()))?;
        // Step 1: create table with legacy schema (session_id) if it doesn't exist yet
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS workflows (
                id          TEXT    PRIMARY KEY,
                name        TEXT    NOT NULL,
                description TEXT,
                created_at  INTEGER NOT NULL,
                updated_at  INTEGER NOT NULL,
                document    TEXT    NOT NULL DEFAULT '{}',
                session_id  TEXT,
                agent_id    TEXT
            );",
        )
        .context("run workflow migrations")?;

        // Step 2: migrate old schema if needed - add agent_id if missing
        let has_agent_id: bool = {
            let mut stmt = conn
                .prepare("PRAGMA table_info(workflows)")
                .context("pragma table_info")?;
            let cols: Vec<String> = stmt
                .query_map([], |row| row.get::<_, String>(1))?
                .filter_map(|r| r.ok())
                .collect();
            cols.iter().any(|c| c == "agent_id")
        };
        if !has_agent_id {
            conn.execute_batch("ALTER TABLE workflows ADD COLUMN agent_id TEXT;")
                .context("add agent_id column")?;
        }
        tracing::info!(path = %db_path.display(), "WorkflowStore initialized");
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    // ── CRUD ──────────────────────────────────────────────────────────────

    pub async fn list(&self) -> Result<Vec<WorkflowDoc>> {
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, name, description, created_at, updated_at, document, agent_id, session_id
                 FROM workflows ORDER BY updated_at DESC",
            )?;
            let rows = stmt.query_map([], row_to_doc)?;
            rows.collect::<Result<Vec<_>, _>>()
                .map_err(anyhow::Error::from)
        })
        .await?
    }

    pub async fn get(&self, id: &str) -> Result<Option<WorkflowDoc>> {
        let conn = self.conn.clone();
        let id = id.to_owned();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let mut stmt = conn.prepare(
                "SELECT id, name, description, created_at, updated_at, document, agent_id, session_id
                 FROM workflows WHERE id = ?1",
            )?;
            let mut rows = stmt.query_map([&id], row_to_doc)?;
            rows.next().transpose().map_err(anyhow::Error::from)
        })
        .await?
    }

    pub async fn create(&self, req: CreateWorkflow) -> Result<WorkflowDoc> {
        let id = uuid::Uuid::new_v4().to_string();
        let now = chrono::Utc::now().timestamp_millis();
        let document = req.document.unwrap_or_else(default_document);
        let doc_str = serde_json::to_string(&document)?;
        let wf = WorkflowDoc {
            id: id.clone(),
            name: req.name.clone(),
            description: req.description.clone(),
            created_at: now,
            updated_at: now,
            document,
            agent_id: None,
            session_id: None,
        };
        let conn = self.conn.clone();
        let name = req.name;
        let description = req.description;
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            conn.execute(
                "INSERT INTO workflows (id, name, description, created_at, updated_at, document)
                 VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![id, name, description, now, now, doc_str],
            )?;
            Ok::<(), anyhow::Error>(())
        })
        .await??;
        Ok(wf)
    }

    pub async fn update(&self, id: &str, patch: UpdateWorkflow) -> Result<Option<WorkflowDoc>> {
        let id = id.to_owned();
        let now = chrono::Utc::now().timestamp_millis();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            // Read current row
            let mut stmt = conn.prepare(
                "SELECT id, name, description, created_at, updated_at, document, agent_id, session_id
                 FROM workflows WHERE id = ?1",
            )?;
            let existing: Option<WorkflowDoc> =
                stmt.query_map([&id], row_to_doc)?.next().transpose()?;
            let Some(mut wf) = existing else {
                return Ok(None);
            };
            if let Some(name) = patch.name {
                wf.name = name;
            }
            if patch.description.is_some() {
                wf.description = patch.description;
            }
            if let Some(doc) = patch.document {
                wf.document = doc;
            }
            if patch.agent_id.is_some() {
                wf.agent_id = patch.agent_id;
            }
            if patch.session_id.is_some() {
                wf.session_id = patch.session_id;
            }
            wf.updated_at = now;
            let doc_str = serde_json::to_string(&wf.document)?;
            conn.execute(
                "UPDATE workflows
                 SET name = ?2, description = ?3, updated_at = ?4, document = ?5, agent_id = ?6, session_id = ?7
                 WHERE id = ?1",
                params![
                    wf.id,
                    wf.name,
                    wf.description,
                    wf.updated_at,
                    doc_str,
                    wf.agent_id,
                    wf.session_id
                ],
            )?;
            Ok(Some(wf))
        })
        .await?
    }

    pub async fn remove(&self, id: &str) -> Result<bool> {
        let id = id.to_owned();
        let conn = self.conn.clone();
        tokio::task::spawn_blocking(move || {
            let conn = conn.lock().unwrap();
            let changed = conn.execute("DELETE FROM workflows WHERE id = ?1", [&id])?;
            Ok(changed > 0)
        })
        .await?
    }
}

// =============================================================================
// Helpers
// =============================================================================

fn row_to_doc(row: &rusqlite::Row<'_>) -> rusqlite::Result<WorkflowDoc> {
    let doc_str: String = row.get(5)?;
    let document =
        serde_json::from_str(&doc_str).unwrap_or(serde_json::Value::Object(Default::default()));
    Ok(WorkflowDoc {
        id: row.get(0)?,
        name: row.get(1)?,
        description: row.get(2)?,
        created_at: row.get(3)?,
        updated_at: row.get(4)?,
        document,
        agent_id: row.get(6)?,
        session_id: row.get(7)?,
    })
}

fn default_document() -> serde_json::Value {
    serde_json::json!({
        "nodes": [
            { "id": "start_0", "type": "start", "meta": { "position": { "x": 180, "y": 300 } }, "data": { "title": "开始" } },
            { "id": "end_0",   "type": "end",   "meta": { "position": { "x": 680, "y": 300 } }, "data": { "title": "结束" } }
        ],
        "edges": [{ "sourceNodeID": "start_0", "targetNodeID": "end_0" }]
    })
}
