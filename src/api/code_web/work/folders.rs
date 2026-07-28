use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};

use super::model::{RevisionRequest, SaveFolderRequest, WorkArtifact, WorkFolder};
use super::service::WorkService;
use super::storage;
use super::validation::{
    ensure_revision, now_millis, revision_conflict, validate_id, validate_name,
};

impl WorkService {
    pub(super) async fn save_folder(
        &self,
        id: &str,
        request: SaveFolderRequest,
    ) -> BootResult<WorkFolder> {
        validate_id(id, "folder")?;
        if request.folder.id != id {
            return Err(BootError::BadRequest(
                "folder id does not match the request path".to_string(),
            ));
        }
        let _guard = self.mutation_lock.lock().await;
        let current = self.read_folder(id).await?;
        let mut folder = request.folder;
        match current {
            Some(current) => {
                ensure_revision("folder", id, request.expected_revision, current.revision)?;
                folder.created_at = current.created_at;
                folder.revision = current.revision + 1;
            }
            None => {
                if request.expected_revision != 0 {
                    return Err(revision_conflict("folder", id, 0));
                }
                folder.revision = 1;
            }
        }
        self.validate_folder(&folder).await?;
        storage::write_json_atomic(&self.folder_path(id), &folder).await?;
        Ok(folder)
    }

    pub(super) async fn trash_folder(
        &self,
        id: &str,
        request: RevisionRequest,
    ) -> BootResult<WorkFolder> {
        self.mutate_folder(id, request.expected_revision, true)
            .await
    }

    pub(super) async fn restore_folder(
        &self,
        id: &str,
        request: RevisionRequest,
    ) -> BootResult<WorkFolder> {
        self.mutate_folder(id, request.expected_revision, false)
            .await
    }

    pub(super) async fn purge_folder(&self, id: &str) -> BootResult<Value> {
        validate_id(id, "folder")?;
        let _guard = self.mutation_lock.lock().await;
        let folder = self
            .read_folder(id)
            .await?
            .ok_or_else(|| BootError::NotFound(format!("Work folder `{id}` was not found")))?;
        if folder.trashed_at.is_none() {
            return Err(BootError::Conflict(
                "a folder must be in the trash before it can be permanently deleted".to_string(),
            ));
        }
        let artifacts = storage::list_json::<WorkArtifact>(&self.artifacts_dir()).await?;
        let folders = storage::list_json::<WorkFolder>(&self.folders_dir()).await?;
        if artifacts
            .iter()
            .any(|artifact| artifact.folder_id.as_deref() == Some(id))
            || folders
                .iter()
                .any(|child| child.parent_id.as_deref() == Some(id))
        {
            return Err(BootError::Conflict(
                "the folder is not empty and cannot be permanently deleted".to_string(),
            ));
        }
        storage::remove_file_if_exists(&self.folder_path(id)).await?;
        Ok(json!({ "purged": true }))
    }

    async fn mutate_folder(
        &self,
        id: &str,
        expected_revision: u64,
        trash: bool,
    ) -> BootResult<WorkFolder> {
        validate_id(id, "folder")?;
        let _guard = self.mutation_lock.lock().await;
        let mut folder = self
            .read_folder(id)
            .await?
            .ok_or_else(|| BootError::NotFound(format!("Work folder `{id}` was not found")))?;
        ensure_revision("folder", id, expected_revision, folder.revision)?;
        folder.trashed_at = trash.then(now_millis);
        folder.revision += 1;
        folder.updated_at = now_millis();
        storage::write_json_atomic(&self.folder_path(id), &folder).await?;
        Ok(folder)
    }

    async fn validate_folder(&self, folder: &WorkFolder) -> BootResult<()> {
        validate_id(&folder.id, "folder")?;
        validate_name(&folder.name, "folder name")?;
        if folder.revision == 0 {
            return Err(BootError::BadRequest(
                "folder revision must be at least 1".to_string(),
            ));
        }
        let mut parent = folder.parent_id.clone();
        let mut depth = 0usize;
        while let Some(parent_id) = parent {
            validate_id(&parent_id, "folder")?;
            if parent_id == folder.id {
                return Err(BootError::Conflict(
                    "a Work folder cannot contain itself".to_string(),
                ));
            }
            let parent_folder = self.read_folder(&parent_id).await?.ok_or_else(|| {
                BootError::BadRequest(format!("parent Work folder `{parent_id}` was not found"))
            })?;
            if parent_folder.trashed_at.is_some() {
                return Err(BootError::Conflict(format!(
                    "parent Work folder `{parent_id}` is in the trash"
                )));
            }
            parent = parent_folder.parent_id;
            depth += 1;
            if depth > 64 {
                return Err(BootError::Conflict(
                    "Work folder nesting exceeds the supported depth".to_string(),
                ));
            }
        }
        Ok(())
    }
}
