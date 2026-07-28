use std::collections::HashMap;
use std::net::IpAddr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, UNIX_EPOCH};

use a3s_boot::{BootError, Result as BootResult};
use rand::distributions::{Alphanumeric, DistString};
use tokio::fs;
use tokio::sync::RwLock;
use url::Url;

use super::model::{
    PreviewCapabilities, PreviewContentSource, PreviewDescriptor, PreviewKind, PreviewSession,
    PreviewSource,
};

const MAX_SESSIONS: usize = 32;
const SESSION_TTL: Duration = Duration::from_secs(12 * 60 * 60);

#[derive(Clone)]
pub(in crate::api) struct PreviewRegistry {
    workspace_root: PathBuf,
    sessions: Arc<RwLock<HashMap<String, PreviewSession>>>,
}

impl PreviewRegistry {
    pub(in crate::api) fn new(workspace_root: PathBuf) -> Self {
        Self {
            workspace_root,
            sessions: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub(in crate::api::code_web) async fn create(
        &self,
        target: String,
    ) -> BootResult<PreviewDescriptor> {
        let target = target.trim();
        if target.is_empty() {
            return Err(BootError::BadRequest("target is required".to_string()));
        }

        let session = if target.starts_with("http://") || target.starts_with("https://") {
            self.local_url_session(target)?
        } else {
            self.path_session(target).await?
        };
        let descriptor = session.descriptor.clone();
        let mut sessions = self.sessions.write().await;
        prune_sessions(&mut sessions, descriptor.created_at);
        sessions.insert(descriptor.id.clone(), session);
        Ok(descriptor)
    }

    pub(in crate::api::code_web) async fn get(&self, id: &str) -> BootResult<PreviewDescriptor> {
        let mut sessions = self.sessions.write().await;
        prune_expired_sessions(&mut sessions, now_millis());
        sessions
            .get(id)
            .map(|session| session.descriptor.clone())
            .ok_or_else(|| BootError::NotFound(format!("preview session was not found: {id}")))
    }

    pub(in crate::api::code_web) async fn remove(&self, id: &str) -> BootResult<()> {
        let mut sessions = self.sessions.write().await;
        prune_expired_sessions(&mut sessions, now_millis());
        let removed = sessions.remove(id);
        if removed.is_none() {
            return Err(BootError::NotFound(format!(
                "preview session was not found: {id}"
            )));
        }
        Ok(())
    }

    pub(in crate::api) async fn content(&self, id: &str) -> Option<PreviewContentSource> {
        let mut sessions = self.sessions.write().await;
        prune_expired_sessions(&mut sessions, now_millis());
        sessions.get(id).and_then(|session| session.content.clone())
    }

    #[cfg(test)]
    pub(super) async fn expire_for_test(&self, id: &str) {
        if let Some(session) = self.sessions.write().await.get_mut(id) {
            session.descriptor.expires_at = now_millis() - 1;
        }
    }

    fn local_url_session(&self, target: &str) -> BootResult<PreviewSession> {
        let mut url = Url::parse(target)
            .map_err(|error| BootError::BadRequest(format!("target URL is invalid: {error}")))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(BootError::BadRequest(
                "only HTTP and HTTPS preview URLs are supported".to_string(),
            ));
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(BootError::BadRequest(
                "preview URLs cannot contain credentials".to_string(),
            ));
        }
        let host = url
            .host_str()
            .ok_or_else(|| BootError::BadRequest("preview URL must include a host".to_string()))?;
        if !is_loopback_host(host) {
            return Err(BootError::BadRequest(
                "preview URL must use localhost or a loopback IP address".to_string(),
            ));
        }
        url.set_fragment(None);
        let url = url.to_string();
        let now = now_millis();
        let id = new_session_id();
        let title = Url::parse(&url)
            .ok()
            .and_then(|value| value.host_str().map(str::to_string))
            .unwrap_or_else(|| "Local app".to_string());
        Ok(PreviewSession {
            descriptor: PreviewDescriptor {
                id,
                kind: PreviewKind::LocalUrl,
                title,
                source: PreviewSource::Url { url: url.clone() },
                content_url: url,
                watch_root: None,
                created_at: now,
                expires_at: now + SESSION_TTL.as_millis() as i64,
                capabilities: PreviewCapabilities {
                    live_reload: false,
                    responsive: true,
                    navigation: true,
                    open_external: true,
                },
            },
            content: None,
        })
    }

    async fn path_session(&self, target: &str) -> BootResult<PreviewSession> {
        let workspace_root = canonicalize_existing(&self.workspace_root, "workspace root").await?;
        let requested = PathBuf::from(target);
        let requested = if requested.is_absolute() {
            requested
        } else {
            workspace_root.join(requested)
        };
        let path = canonicalize_existing(&requested, "preview target").await?;
        if !path.starts_with(&workspace_root) {
            return Err(BootError::BadRequest(
                "preview target must stay inside the active workspace".to_string(),
            ));
        }
        let metadata = fs::metadata(&path).await.map_err(|error| {
            BootError::BadRequest(format!("preview target is unavailable: {error}"))
        })?;
        let is_directory = metadata.is_dir();
        let (kind, content, root_path, title) = if is_directory {
            let entry = canonicalize_existing(&path.join("index.html"), "preview entry file")
                .await
                .map_err(|_| {
                    BootError::BadRequest(
                        "preview directory must contain an index.html file".to_string(),
                    )
                })?;
            if !entry.starts_with(&path) {
                return Err(BootError::BadRequest(
                    "preview index.html cannot resolve outside its directory".to_string(),
                ));
            }
            (
                PreviewKind::StaticSite,
                PreviewContentSource::Directory {
                    root: path.clone(),
                    entry,
                },
                path.clone(),
                file_name(&path),
            )
        } else {
            let kind = preview_kind_for_path(&path)?;
            let parent = path.parent().ok_or_else(|| {
                BootError::BadRequest("preview target must have a parent directory".to_string())
            })?;
            let content = if kind == PreviewKind::StaticSite {
                PreviewContentSource::Directory {
                    root: parent.to_path_buf(),
                    entry: path.clone(),
                }
            } else {
                PreviewContentSource::File { path: path.clone() }
            };
            (kind, content, parent.to_path_buf(), file_name(&path))
        };
        let now = now_millis();
        let id = new_session_id();
        let size = if metadata.is_file() {
            metadata.len()
        } else {
            0
        };
        let mtime_ms = metadata.modified().ok().and_then(|value| {
            value
                .duration_since(UNIX_EPOCH)
                .ok()
                .and_then(|duration| u64::try_from(duration.as_millis()).ok())
        });
        let responsive = kind == PreviewKind::StaticSite;
        Ok(PreviewSession {
            descriptor: PreviewDescriptor {
                content_url: format!("/preview/{id}/"),
                id,
                kind,
                title: title.clone(),
                source: PreviewSource::Path {
                    path: path.display().to_string(),
                    root_path: root_path.display().to_string(),
                    name: title,
                    size,
                    mtime_ms,
                    is_directory,
                    is_binary: is_binary_kind(kind),
                },
                watch_root: Some(root_path.display().to_string()),
                created_at: now,
                expires_at: now + SESSION_TTL.as_millis() as i64,
                capabilities: PreviewCapabilities {
                    live_reload: true,
                    responsive,
                    navigation: responsive,
                    open_external: true,
                },
            },
            content: Some(content),
        })
    }
}

fn preview_kind_for_path(path: &Path) -> BootResult<PreviewKind> {
    let extension = path
        .extension()
        .and_then(|extension| extension.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let kind = match extension.as_str() {
        "html" | "htm" => PreviewKind::StaticSite,
        "pdf" => PreviewKind::Pdf,
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "avif" => PreviewKind::Image,
        "docx" | "xls" | "xlsx" | "ods" | "csv" | "pptx" => PreviewKind::Office,
        "txt" | "md" | "markdown" | "json" | "xml" | "svg" | "yaml" | "yml" | "toml" | "acl"
        | "log" | "css" | "scss" | "js" | "jsx" | "ts" | "tsx" | "rs" | "py" | "sh" | "c" | "h"
        | "cc" | "cpp" | "java" | "go" | "sql" => PreviewKind::Text,
        _ => {
            return Err(BootError::BadRequest(format!(
                "preview format is not supported: {}",
                path.display()
            )))
        }
    };
    Ok(kind)
}

fn is_binary_kind(kind: PreviewKind) -> bool {
    matches!(
        kind,
        PreviewKind::Pdf | PreviewKind::Image | PreviewKind::Office
    )
}

fn is_loopback_host(host: &str) -> bool {
    let lower = host.to_ascii_lowercase();
    lower == "localhost"
        || lower.ends_with(".localhost")
        || lower
            .parse::<IpAddr>()
            .is_ok_and(|address| address.is_loopback())
}

fn prune_sessions(sessions: &mut HashMap<String, PreviewSession>, now: i64) {
    prune_expired_sessions(sessions, now);
    while sessions.len() >= MAX_SESSIONS {
        let Some(oldest) = sessions
            .iter()
            .min_by_key(|(_, session)| session.descriptor.created_at)
            .map(|(id, _)| id.clone())
        else {
            break;
        };
        sessions.remove(&oldest);
    }
}

fn prune_expired_sessions(sessions: &mut HashMap<String, PreviewSession>, now: i64) {
    sessions.retain(|_, session| session.descriptor.expires_at > now);
}

async fn canonicalize_existing(path: &Path, label: &str) -> BootResult<PathBuf> {
    fs::canonicalize(path)
        .await
        .map_err(|error| BootError::BadRequest(format!("{label} is unavailable: {error}")))
}

fn file_name(path: &Path) -> String {
    path.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Preview")
        .to_string()
}

fn new_session_id() -> String {
    Alphanumeric.sample_string(&mut rand::thread_rng(), 32)
}

fn now_millis() -> i64 {
    chrono::Utc::now().timestamp_millis()
}
