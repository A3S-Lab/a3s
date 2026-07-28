use std::sync::Arc;

use a3s_boot::{
    controller, BootError, BootRequest, BootResponse, Result as BootResult, StreamableFile,
};
use futures::stream;
use tokio::io::AsyncReadExt;

use super::model::{
    CopyArtifactRequest, RestoreVersionRequest, RevisionRequest, SaveArtifactRequest,
    SaveFolderRequest, WorkArtifact, WorkArtifactVersion, WorkFolder,
};
use super::service::WorkService;
use super::storage;

pub(super) struct WorkController {
    service: Arc<WorkService>,
}

impl WorkController {
    pub(super) fn new(service: Arc<WorkService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/work")]
impl WorkController {
    #[get("/library")]
    async fn library(
        &self,
        #[query("includeTrash")] include_trash: Option<bool>,
    ) -> BootResult<serde_json::Value> {
        self.service.library(include_trash.unwrap_or(false)).await
    }

    #[get("/artifacts/{id}")]
    async fn artifact(&self, #[param("id")] id: String) -> BootResult<WorkArtifact> {
        self.service.artifact(&id).await
    }

    #[put("/artifacts/{id}")]
    async fn save_artifact(
        &self,
        #[param("id")] id: String,
        #[body] request: SaveArtifactRequest,
    ) -> BootResult<WorkArtifact> {
        self.service.save_artifact(&id, request).await
    }

    #[delete("/artifacts/{id}")]
    async fn trash_artifact(
        &self,
        #[param("id")] id: String,
        #[body] request: RevisionRequest,
    ) -> BootResult<WorkArtifact> {
        self.service.trash_artifact(&id, request).await
    }

    #[post("/artifacts/{id}/restore")]
    async fn restore_artifact(
        &self,
        #[param("id")] id: String,
        #[body] request: RevisionRequest,
    ) -> BootResult<WorkArtifact> {
        self.service.restore_artifact(&id, request).await
    }

    #[delete("/artifacts/{id}/purge")]
    async fn purge_artifact(&self, #[param("id")] id: String) -> BootResult<serde_json::Value> {
        self.service.purge_artifact(&id).await
    }

    #[post("/artifacts/{id}/copy")]
    async fn copy_artifact(
        &self,
        #[param("id")] id: String,
        #[body] request: CopyArtifactRequest,
    ) -> BootResult<WorkArtifact> {
        self.service.copy_artifact(&id, request).await
    }

    #[get("/artifacts/{id}/versions")]
    async fn versions(&self, #[param("id")] id: String) -> BootResult<Vec<WorkArtifactVersion>> {
        self.service.versions(&id).await
    }

    #[post("/artifacts/{id}/versions/restore")]
    async fn restore_version(
        &self,
        #[param("id")] id: String,
        #[body] request: RestoreVersionRequest,
    ) -> BootResult<WorkArtifact> {
        self.service.restore_version(&id, request).await
    }

    #[put("/artifacts/{id}/source")]
    async fn upload_source(
        &self,
        #[param("id")] id: String,
        #[query("expectedRevision")] expected_revision: u64,
        #[query("fileName")] file_name: String,
        #[request] request: BootRequest,
    ) -> BootResult<WorkArtifact> {
        self.service
            .upload_source(
                &id,
                expected_revision,
                file_name,
                request.content_type().map(str::to_string),
                request.body(),
            )
            .await
    }

    #[get("/artifacts/{id}/source", raw)]
    async fn download_source(&self, #[param("id")] id: String) -> BootResult<BootResponse> {
        let descriptor = self.service.source(&id).await?;
        let file = tokio::fs::File::open(&descriptor.path)
            .await
            .map_err(|error| storage::storage_error(&descriptor.path, error))?;
        let stream = stream::try_unfold(file, |mut file| async move {
            let mut bytes = vec![0; 64 * 1024];
            let count = file.read(&mut bytes).await.map_err(BootError::Io)?;
            if count == 0 {
                return Ok(None);
            }
            bytes.truncate(count);
            Ok(Some((bytes, file)))
        });
        let file = StreamableFile::stream(stream)
            .with_content_type(descriptor.metadata.content_type)
            .with_content_length(descriptor.metadata.size)
            .with_attachment(descriptor.metadata.name)?;
        Ok(BootResponse::streamable_file(file).with_header("x-content-type-options", "nosniff"))
    }

    #[put("/folders/{id}")]
    async fn save_folder(
        &self,
        #[param("id")] id: String,
        #[body] request: SaveFolderRequest,
    ) -> BootResult<WorkFolder> {
        self.service.save_folder(&id, request).await
    }

    #[delete("/folders/{id}")]
    async fn trash_folder(
        &self,
        #[param("id")] id: String,
        #[body] request: RevisionRequest,
    ) -> BootResult<WorkFolder> {
        self.service.trash_folder(&id, request).await
    }

    #[post("/folders/{id}/restore")]
    async fn restore_folder(
        &self,
        #[param("id")] id: String,
        #[body] request: RevisionRequest,
    ) -> BootResult<WorkFolder> {
        self.service.restore_folder(&id, request).await
    }

    #[delete("/folders/{id}/purge")]
    async fn purge_folder(&self, #[param("id")] id: String) -> BootResult<serde_json::Value> {
        self.service.purge_folder(&id).await
    }
}
