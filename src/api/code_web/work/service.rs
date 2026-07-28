use std::path::PathBuf;
use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};
use tokio::fs;
use tokio::sync::Mutex;

use super::model::{
    CopyArtifactRequest, RestoreVersionRequest, RevisionRequest, SaveArtifactRequest, WorkArtifact,
    WorkArtifactVersion, WorkFolder, WorkSourceFile,
};
use super::storage;
use super::validation::{
    ensure_revision, now_millis, revision_conflict, validate_content_type, validate_file_name,
    validate_id, validate_name,
};
use crate::api::code_web::state::CodeWebState;

pub(super) const MAX_ARTIFACT_BYTES: usize = 16 * 1024 * 1024;
pub(super) const MAX_SOURCE_BYTES: usize = 50 * 1024 * 1024;
const MAX_HISTORY_ENTRIES: usize = 50;

pub(super) struct WorkSourceDescriptor {
    pub(super) path: PathBuf,
    pub(super) metadata: WorkSourceFile,
}

pub(super) struct WorkService {
    pub(super) root: PathBuf,
    pub(super) mutation_lock: Mutex<()>,
}

impl WorkService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        let root = state
            .config_path
            .parent()
            .map(|parent| parent.join("work"))
            .unwrap_or_else(|| PathBuf::from(".a3s/work"));
        Self::from_root(root)
    }

    fn from_root(root: PathBuf) -> Self {
        Self {
            root,
            mutation_lock: Mutex::new(()),
        }
    }

    pub(super) async fn library(&self, include_trash: bool) -> BootResult<Value> {
        let mut artifacts = storage::list_json::<WorkArtifact>(&self.artifacts_dir()).await?;
        let mut folders = storage::list_json::<WorkFolder>(&self.folders_dir()).await?;
        if !include_trash {
            artifacts.retain(|artifact| artifact.trashed_at.is_none());
            folders.retain(|folder| folder.trashed_at.is_none());
        }
        artifacts.sort_by(|left, right| {
            right
                .last_opened_at
                .cmp(&left.last_opened_at)
                .then_with(|| right.updated_at.cmp(&left.updated_at))
        });
        folders.sort_by_key(|folder| folder.name.to_lowercase());
        Ok(json!({
            "artifacts": artifacts,
            "folders": folders,
            "limits": {
                "artifactBytes": MAX_ARTIFACT_BYTES,
                "sourceBytes": MAX_SOURCE_BYTES,
                "historyEntries": MAX_HISTORY_ENTRIES,
            },
            "storage": "server",
        }))
    }

    pub(super) async fn artifact(&self, id: &str) -> BootResult<WorkArtifact> {
        validate_id(id, "artifact")?;
        self.read_artifact(id)
            .await?
            .ok_or_else(|| BootError::NotFound(format!("Work artifact `{id}` was not found")))
    }

    pub(super) async fn save_artifact(
        &self,
        id: &str,
        request: SaveArtifactRequest,
    ) -> BootResult<WorkArtifact> {
        validate_id(id, "artifact")?;
        if request.artifact.id != id {
            return Err(BootError::BadRequest(
                "artifact id does not match the request path".to_string(),
            ));
        }
        let _guard = self.mutation_lock.lock().await;
        let current = self.read_artifact(id).await?;
        let mut artifact = request.artifact;
        match current {
            Some(current) => {
                if request.expected_revision != current.revision {
                    if artifact == current {
                        return Ok(current);
                    }
                    return Err(revision_conflict("artifact", id, current.revision));
                }
                artifact.created_at = current.created_at;
                artifact.source = current.source.clone();
                artifact.revision = artifact.revision.max(current.revision + 1);
                self.validate_artifact(&artifact).await?;
                self.save_history(&current).await?;
            }
            None => {
                if request.expected_revision != 0 {
                    return Err(revision_conflict("artifact", id, 0));
                }
                artifact.revision = artifact.revision.max(1);
                artifact.source = None;
                self.validate_artifact(&artifact).await?;
            }
        }
        storage::write_json_atomic(&self.artifact_path(id), &artifact).await?;
        self.prune_history(id).await?;
        Ok(artifact)
    }

    pub(super) async fn trash_artifact(
        &self,
        id: &str,
        request: RevisionRequest,
    ) -> BootResult<WorkArtifact> {
        self.mutate_artifact(id, request.expected_revision, |artifact| {
            artifact.trashed_at = Some(now_millis());
        })
        .await
    }

    pub(super) async fn restore_artifact(
        &self,
        id: &str,
        request: RevisionRequest,
    ) -> BootResult<WorkArtifact> {
        self.mutate_artifact(id, request.expected_revision, |artifact| {
            artifact.trashed_at = None;
        })
        .await
    }

    pub(super) async fn purge_artifact(&self, id: &str) -> BootResult<Value> {
        validate_id(id, "artifact")?;
        let _guard = self.mutation_lock.lock().await;
        let artifact = self.artifact(id).await?;
        if artifact.trashed_at.is_none() {
            return Err(BootError::Conflict(
                "an artifact must be in the trash before it can be permanently deleted".to_string(),
            ));
        }
        storage::remove_file_if_exists(&self.artifact_path(id)).await?;
        storage::remove_dir_if_exists(&self.history_dir(id)).await?;
        storage::remove_dir_if_exists(&self.binary_dir(id)).await?;
        Ok(json!({ "purged": true }))
    }

    pub(super) async fn copy_artifact(
        &self,
        source_id: &str,
        request: CopyArtifactRequest,
    ) -> BootResult<WorkArtifact> {
        validate_id(source_id, "artifact")?;
        validate_id(&request.id, "artifact")?;
        let _guard = self.mutation_lock.lock().await;
        if self.read_artifact(&request.id).await?.is_some() {
            return Err(BootError::Conflict(format!(
                "Work artifact `{}` already exists",
                request.id
            )));
        }
        let source = self.artifact(source_id).await?;
        let now = now_millis();
        let mut artifact = source.clone();
        artifact.id = request.id;
        artifact.title = request
            .title
            .unwrap_or_else(|| format!("{} copy", source.title));
        artifact.folder_id = request.folder_id;
        artifact.created_at = now;
        artifact.updated_at = now;
        artifact.last_opened_at = now;
        artifact.revision = 1;
        artifact.trashed_at = None;
        self.validate_artifact(&artifact).await?;
        if source.source.is_some() {
            let copied = storage::copy_file_if_exists(
                &self.source_path(source_id),
                &self.source_path(&artifact.id),
            )
            .await?;
            if !copied {
                return Err(BootError::Internal(format!(
                    "source metadata exists for Work artifact `{source_id}`, but its bytes are missing"
                )));
            }
        }
        storage::write_json_atomic(&self.artifact_path(&artifact.id), &artifact).await?;
        Ok(artifact)
    }

    pub(super) async fn versions(&self, id: &str) -> BootResult<Vec<WorkArtifactVersion>> {
        let current = self.artifact(id).await?;
        let mut artifacts = storage::list_json::<WorkArtifact>(&self.history_dir(id)).await?;
        artifacts.push(current.clone());
        artifacts.sort_by_key(|artifact| std::cmp::Reverse(artifact.revision));
        Ok(artifacts
            .into_iter()
            .map(|artifact| WorkArtifactVersion {
                revision: artifact.revision,
                updated_at: artifact.updated_at,
                current: artifact.revision == current.revision,
                artifact,
            })
            .collect())
    }

    pub(super) async fn restore_version(
        &self,
        id: &str,
        request: RestoreVersionRequest,
    ) -> BootResult<WorkArtifact> {
        validate_id(id, "artifact")?;
        let _guard = self.mutation_lock.lock().await;
        let current = self.artifact(id).await?;
        ensure_revision("artifact", id, request.expected_revision, current.revision)?;
        let mut restored =
            storage::read_json_optional::<WorkArtifact>(&self.history_path(id, request.version))
                .await?
                .ok_or_else(|| {
                    BootError::NotFound(format!(
                        "revision {} of Work artifact `{id}` was not found",
                        request.version
                    ))
                })?;
        self.save_history(&current).await?;
        restored.id = current.id;
        restored.created_at = current.created_at;
        restored.updated_at = now_millis();
        restored.last_opened_at = current.last_opened_at;
        restored.revision = current.revision + 1;
        restored.source = current.source;
        restored.trashed_at = current.trashed_at;
        self.validate_artifact(&restored).await?;
        storage::write_json_atomic(&self.artifact_path(id), &restored).await?;
        self.prune_history(id).await?;
        Ok(restored)
    }

    pub(super) async fn upload_source(
        &self,
        id: &str,
        expected_revision: u64,
        file_name: String,
        content_type: Option<String>,
        bytes: &[u8],
    ) -> BootResult<WorkArtifact> {
        validate_id(id, "artifact")?;
        let file_name = validate_file_name(file_name)?;
        if bytes.len() > MAX_SOURCE_BYTES {
            return Err(BootError::PayloadTooLarge(format!(
                "Work source files are limited to {MAX_SOURCE_BYTES} bytes"
            )));
        }
        let content_type = validate_content_type(content_type)?;
        let _guard = self.mutation_lock.lock().await;
        let mut artifact = self.artifact(id).await?;
        ensure_revision("artifact", id, expected_revision, artifact.revision)?;
        self.save_history(&artifact).await?;
        storage::write_bytes_atomic(&self.source_path(id), bytes).await?;
        artifact.source = Some(WorkSourceFile {
            name: file_name,
            content_type,
            size: bytes.len() as u64,
            updated_at: now_millis(),
        });
        artifact.revision += 1;
        artifact.updated_at = now_millis();
        storage::write_json_atomic(&self.artifact_path(id), &artifact).await?;
        self.prune_history(id).await?;
        Ok(artifact)
    }

    pub(super) async fn source(&self, id: &str) -> BootResult<WorkSourceDescriptor> {
        let artifact = self.artifact(id).await?;
        let metadata = artifact.source.ok_or_else(|| {
            BootError::NotFound(format!("Work artifact `{id}` has no source file"))
        })?;
        let path = self.source_path(id);
        if !fs::try_exists(&path)
            .await
            .map_err(|error| storage::storage_error(&path, error))?
        {
            return Err(BootError::Internal(format!(
                "source metadata exists for Work artifact `{id}`, but its bytes are missing"
            )));
        }
        Ok(WorkSourceDescriptor { path, metadata })
    }

    async fn mutate_artifact(
        &self,
        id: &str,
        expected_revision: u64,
        mutate: impl FnOnce(&mut WorkArtifact),
    ) -> BootResult<WorkArtifact> {
        validate_id(id, "artifact")?;
        let _guard = self.mutation_lock.lock().await;
        let mut artifact = self.artifact(id).await?;
        ensure_revision("artifact", id, expected_revision, artifact.revision)?;
        self.save_history(&artifact).await?;
        mutate(&mut artifact);
        artifact.revision += 1;
        artifact.updated_at = now_millis();
        if artifact.trashed_at.is_none() {
            self.validate_artifact(&artifact).await?;
        }
        storage::write_json_atomic(&self.artifact_path(id), &artifact).await?;
        self.prune_history(id).await?;
        Ok(artifact)
    }

    async fn validate_artifact(&self, artifact: &WorkArtifact) -> BootResult<()> {
        validate_id(&artifact.id, "artifact")?;
        validate_name(&artifact.title, "artifact title")?;
        if artifact.revision == 0 {
            return Err(BootError::BadRequest(
                "artifact revision must be at least 1".to_string(),
            ));
        }
        let content_type = artifact
            .content
            .get("type")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BootError::BadRequest("artifact content.type is required".to_string())
            })?;
        if content_type != artifact.kind.as_str() {
            return Err(BootError::BadRequest(format!(
                "artifact kind `{}` does not match content type `{content_type}`",
                artifact.kind.as_str()
            )));
        }
        let encoded = serde_json::to_vec(artifact)
            .map_err(|error| BootError::BadRequest(format!("invalid Work artifact: {error}")))?;
        if encoded.len() > MAX_ARTIFACT_BYTES {
            return Err(BootError::PayloadTooLarge(format!(
                "Work artifacts are limited to {MAX_ARTIFACT_BYTES} bytes"
            )));
        }
        if artifact.trashed_at.is_none() {
            if let Some(folder_id) = artifact.folder_id.as_deref() {
                validate_id(folder_id, "folder")?;
                let folder = self.read_folder(folder_id).await?.ok_or_else(|| {
                    BootError::BadRequest(format!("Work folder `{folder_id}` was not found"))
                })?;
                if folder.trashed_at.is_some() {
                    return Err(BootError::Conflict(format!(
                        "Work folder `{folder_id}` is in the trash"
                    )));
                }
            }
        }
        Ok(())
    }

    async fn save_history(&self, artifact: &WorkArtifact) -> BootResult<()> {
        storage::write_json_atomic(
            &self.history_path(&artifact.id, artifact.revision),
            artifact,
        )
        .await
    }

    async fn prune_history(&self, id: &str) -> BootResult<()> {
        let directory = self.history_dir(id);
        let mut entries = match fs::read_dir(&directory).await {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(storage::storage_error(&directory, error)),
        };
        let mut revisions = Vec::new();
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|error| storage::storage_error(&directory, error))?
        {
            let path = entry.path();
            let Some(revision) = path
                .file_stem()
                .and_then(|value| value.to_str())
                .and_then(|value| value.parse::<u64>().ok())
            else {
                continue;
            };
            revisions.push((revision, path));
        }
        revisions.sort_by_key(|(revision, _)| *revision);
        let remove_count = revisions.len().saturating_sub(MAX_HISTORY_ENTRIES);
        for (_, path) in revisions.into_iter().take(remove_count) {
            storage::remove_file_if_exists(&path).await?;
        }
        Ok(())
    }

    async fn read_artifact(&self, id: &str) -> BootResult<Option<WorkArtifact>> {
        storage::read_json_optional(&self.artifact_path(id)).await
    }

    pub(super) async fn read_folder(&self, id: &str) -> BootResult<Option<WorkFolder>> {
        storage::read_json_optional(&self.folder_path(id)).await
    }

    pub(super) fn artifacts_dir(&self) -> PathBuf {
        self.root.join("artifacts")
    }

    fn artifact_path(&self, id: &str) -> PathBuf {
        self.artifacts_dir().join(format!("{id}.json"))
    }

    pub(super) fn folders_dir(&self) -> PathBuf {
        self.root.join("folders")
    }

    pub(super) fn folder_path(&self, id: &str) -> PathBuf {
        self.folders_dir().join(format!("{id}.json"))
    }

    fn history_dir(&self, id: &str) -> PathBuf {
        self.root.join("history").join(id)
    }

    fn history_path(&self, id: &str, revision: u64) -> PathBuf {
        self.history_dir(id).join(format!("{revision}.json"))
    }

    fn binary_dir(&self, id: &str) -> PathBuf {
        self.root.join("binaries").join(id)
    }

    fn source_path(&self, id: &str) -> PathBuf {
        self.binary_dir(id).join("source")
    }
}

#[cfg(test)]
impl WorkService {
    pub(super) fn for_test(root: &std::path::Path) -> Self {
        Self::from_root(root.to_path_buf())
    }
}
