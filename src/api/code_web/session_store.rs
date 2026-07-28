use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use a3s_code_core::store::{FileSessionStore, SessionData, SessionStore};
use anyhow::{Context, Result};
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

use super::kernel::turn_queue::CodeWebStoredTurnQueue;
use super::state::{CodeWebSessionControls, CodeWebSessionSettings};

const SESSION_SCHEMA_VERSION: u32 = 1;
const MAX_SESSION_METADATA_BYTES: u64 = 64 * 1024 * 1024;

static TEMP_FILE_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebSessionMetadata {
    pub(in crate::api::code_web) workspace: String,
    pub(in crate::api::code_web) title: Option<String>,
    pub(in crate::api::code_web) agent_id: Option<String>,
    pub(in crate::api::code_web) created_at: i64,
    pub(in crate::api::code_web) updated_at: i64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebStoredContext {
    pub(in crate::api::code_web) compact_summary: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeWebStoredSession {
    schema_version: u32,
    pub(in crate::api::code_web) session_id: String,
    pub(in crate::api::code_web) metadata: CodeWebSessionMetadata,
    #[serde(default)]
    pub(in crate::api::code_web) messages: Vec<Value>,
    #[serde(default)]
    pub(in crate::api::code_web) controls: CodeWebSessionControls,
    #[serde(default)]
    pub(in crate::api::code_web) context: CodeWebStoredContext,
    #[serde(default)]
    pub(in crate::api::code_web) turn_queue: CodeWebStoredTurnQueue,
    #[serde(default)]
    pub(in crate::api::code_web) settings: CodeWebSessionSettings,
}

impl CodeWebStoredSession {
    pub(in crate::api::code_web) fn new(
        session_id: String,
        metadata: CodeWebSessionMetadata,
        messages: Vec<Value>,
        controls: CodeWebSessionControls,
        context: CodeWebStoredContext,
        turn_queue: CodeWebStoredTurnQueue,
        settings: CodeWebSessionSettings,
    ) -> Self {
        Self {
            schema_version: SESSION_SCHEMA_VERSION,
            session_id,
            metadata,
            messages,
            controls,
            context,
            turn_queue,
            settings,
        }
    }

    fn validate(&self, requested_id: &str) -> Result<()> {
        if self.schema_version != SESSION_SCHEMA_VERSION {
            anyhow::bail!(
                "unsupported Code Web session schema version {}",
                self.schema_version
            );
        }
        if self.session_id != requested_id {
            anyhow::bail!(
                "Code Web session file contains id `{}` instead of `{requested_id}`",
                self.session_id
            );
        }
        Ok(())
    }
}

pub(in crate::api) struct CodeWebSessionRepository {
    metadata_dir: PathBuf,
    core_store: Arc<FileSessionStore>,
    write_lock: Mutex<()>,
}

impl CodeWebSessionRepository {
    pub(in crate::api) async fn open_default() -> Result<Self> {
        let root = code_web_store_root()?;
        Self::open(root).await
    }

    pub(in crate::api::code_web) async fn open(root: impl Into<PathBuf>) -> Result<Self> {
        let root = root.into();
        let metadata_dir = root.join("metadata");
        fs::create_dir_all(&metadata_dir).await.with_context(|| {
            format!(
                "failed to create Code Web metadata directory {}",
                metadata_dir.display()
            )
        })?;
        let core_store = Arc::new(
            FileSessionStore::new(root.join("sessions"))
                .await
                .context("failed to open Code Web Core session store")?,
        );
        Ok(Self {
            metadata_dir,
            core_store,
            write_lock: Mutex::new(()),
        })
    }

    pub(in crate::api::code_web) fn core_store(&self) -> Arc<dyn SessionStore> {
        self.core_store.clone()
    }

    pub(in crate::api::code_web) async fn list_session_ids(&self) -> Result<Vec<String>> {
        let mut session_ids: BTreeSet<String> = self.core_store.list().await?.into_iter().collect();
        let mut entries = fs::read_dir(&self.metadata_dir).await.with_context(|| {
            format!(
                "failed to read Code Web metadata directory {}",
                self.metadata_dir.display()
            )
        })?;
        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                if let Some(session_id) = path
                    .file_stem()
                    .and_then(|stem| stem.to_str())
                    .and_then(decode_storage_key)
                {
                    session_ids.insert(session_id);
                }
            }
        }
        Ok(session_ids.into_iter().collect())
    }

    pub(in crate::api::code_web) async fn load_web_session(
        &self,
        session_id: &str,
    ) -> Result<Option<CodeWebStoredSession>> {
        let path = self.metadata_path(session_id);
        let metadata = match fs::metadata(&path).await {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("failed to inspect Code Web session {}", path.display())
                })
            }
        };
        if metadata.len() > MAX_SESSION_METADATA_BYTES {
            anyhow::bail!(
                "Code Web session metadata {} exceeds the {} byte limit",
                path.display(),
                MAX_SESSION_METADATA_BYTES
            );
        }
        let bytes = fs::read(&path)
            .await
            .with_context(|| format!("failed to read Code Web session {}", path.display()))?;
        let stored: CodeWebStoredSession = serde_json::from_slice(&bytes)
            .with_context(|| format!("failed to parse Code Web session {}", path.display()))?;
        stored.validate(session_id)?;
        Ok(Some(stored))
    }

    pub(in crate::api::code_web) async fn load_core_session(
        &self,
        session_id: &str,
    ) -> Result<Option<SessionData>> {
        self.core_store.load(session_id).await
    }

    pub(in crate::api::code_web) async fn save_web_session(
        &self,
        stored: &CodeWebStoredSession,
    ) -> Result<()> {
        stored.validate(&stored.session_id)?;
        let bytes = serde_json::to_vec_pretty(stored)
            .context("failed to serialize Code Web session metadata")?;
        if bytes.len() as u64 > MAX_SESSION_METADATA_BYTES {
            anyhow::bail!(
                "Code Web session metadata exceeds the {} byte limit",
                MAX_SESSION_METADATA_BYTES
            );
        }

        let _guard = self.write_lock.lock().await;
        let path = self.metadata_path(&stored.session_id);
        let temp_path = temporary_path(&path);
        let write_result = async {
            let mut file = fs::OpenOptions::new()
                .create_new(true)
                .write(true)
                .open(&temp_path)
                .await
                .with_context(|| {
                    format!(
                        "failed to create temporary Code Web session {}",
                        temp_path.display()
                    )
                })?;
            file.write_all(&bytes).await.with_context(|| {
                format!("failed to write Code Web session {}", temp_path.display())
            })?;
            file.write_all(b"\n").await?;
            file.sync_all().await.with_context(|| {
                format!("failed to sync Code Web session {}", temp_path.display())
            })?;
            drop(file);
            set_private_permissions(&temp_path).await?;
            fs::rename(&temp_path, &path).await.with_context(|| {
                format!(
                    "failed to replace Code Web session {} with {}",
                    path.display(),
                    temp_path.display()
                )
            })?;
            Ok::<(), anyhow::Error>(())
        }
        .await;
        if write_result.is_err() {
            let _ = fs::remove_file(&temp_path).await;
        }
        write_result
    }

    pub(in crate::api::code_web) async fn delete_session(&self, session_id: &str) -> Result<()> {
        self.core_store.delete(session_id).await?;
        let path = self.metadata_path(session_id);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error)
                .with_context(|| format!("failed to delete Code Web session {}", path.display())),
        }
    }

    pub(in crate::api::code_web) async fn delete_core_session(
        &self,
        session_id: &str,
    ) -> Result<()> {
        self.core_store.delete(session_id).await
    }

    fn metadata_path(&self, session_id: &str) -> PathBuf {
        self.metadata_dir
            .join(format!("{}.json", encoded_storage_key(session_id)))
    }
}

fn code_web_store_root() -> Result<PathBuf> {
    if let Some(path) = std::env::var_os("A3S_CODE_WEB_STATE_DIR").filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let home = crate::user_paths::user_home_dir().context("user home is unavailable")?;
    Ok(home.join(".a3s").join("code-web"))
}

fn encoded_storage_key(id: &str) -> String {
    format!(
        "id_{}",
        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(id.as_bytes())
    )
}

fn decode_storage_key(key: &str) -> Option<String> {
    let encoded = key.strip_prefix("id_")?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(encoded)
        .ok()?;
    String::from_utf8(bytes).ok()
}

fn temporary_path(path: &Path) -> PathBuf {
    let counter = TEMP_FILE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session.json");
    path.with_file_name(format!(
        ".{file_name}.{}.{}.tmp",
        std::process::id(),
        counter
    ))
}

#[cfg(unix)]
async fn set_private_permissions(path: &Path) -> Result<()> {
    use std::os::unix::fs::PermissionsExt;

    fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))
        .await
        .with_context(|| format!("failed to protect Code Web session {}", path.display()))
}

#[cfg(not(unix))]
async fn set_private_permissions(_path: &Path) -> Result<()> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn temp_store(name: &str) -> PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!(
            "a3s-code-web-session-store-{name}-{}-{nanos}",
            std::process::id()
        ))
    }

    #[test]
    fn historical_schema_without_workspace_remains_readable() {
        let stored: CodeWebStoredSession = serde_json::from_value(json!({
            "schemaVersion": 1,
            "sessionId": "legacy",
            "metadata": {
                "title": "Legacy task",
                "agentId": "default",
                "createdAt": 42
            },
            "messages": [],
            "controls": { "effort": "high", "goal": null },
            "context": { "compactSummary": null },
            "settings": {
                "model": null,
                "followDefaultModel": true,
                "permissionMode": "default",
                "planningMode": null,
                "goalTracking": null
            }
        }))
        .expect("legacy schema");

        assert_eq!(stored.session_id, "legacy");
        assert!(stored.metadata.workspace.is_empty());
        assert_eq!(stored.metadata.title.as_deref(), Some("Legacy task"));
        assert_eq!(stored.controls.effort, "high");
    }

    #[tokio::test]
    async fn metadata_round_trip_is_atomic_and_listable() {
        let root = temp_store("round-trip");
        let repository = CodeWebSessionRepository::open(&root)
            .await
            .expect("open repository");
        let stored = CodeWebStoredSession::new(
            "session/a".to_string(),
            CodeWebSessionMetadata {
                workspace: "/workspace".to_string(),
                title: Some("Persist me".to_string()),
                agent_id: Some("default".to_string()),
                created_at: 1,
                updated_at: 2,
            },
            vec![json!({ "role": "user", "content": "hello" })],
            CodeWebSessionControls::default(),
            CodeWebStoredContext::default(),
            CodeWebStoredTurnQueue::default(),
            CodeWebSessionSettings::default(),
        );

        repository
            .save_web_session(&stored)
            .await
            .expect("save metadata");
        let loaded = repository
            .load_web_session("session/a")
            .await
            .expect("load metadata")
            .expect("stored metadata");
        assert_eq!(loaded.metadata.title.as_deref(), Some("Persist me"));
        assert_eq!(loaded.messages[0]["content"], "hello");
        assert_eq!(
            repository.list_session_ids().await.expect("list sessions"),
            vec!["session/a"]
        );

        repository
            .delete_session("session/a")
            .await
            .expect("delete metadata");
        assert!(repository
            .load_web_session("session/a")
            .await
            .expect("load deleted")
            .is_none());
        fs::remove_dir_all(root).await.expect("remove test store");
    }
}
