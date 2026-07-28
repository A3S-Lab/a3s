use std::sync::Arc;

use a3s_boot::{
    BootApplication, BootError, BootRequest, ControllerDefinition, HttpMethod, Module, ModuleRef,
    Result as BootResult,
};
use serde_json::json;

use super::controller::WorkController;
use super::model::{
    CopyArtifactRequest, RestoreVersionRequest, RevisionRequest, SaveArtifactRequest,
    SaveFolderRequest, WorkArtifact, WorkArtifactKind, WorkFolder,
};
use super::service::WorkService;

fn artifact(id: &str, revision: u64) -> WorkArtifact {
    WorkArtifact {
        id: id.to_string(),
        kind: WorkArtifactKind::Document,
        title: "Project brief".to_string(),
        favorite: false,
        created_at: 1,
        updated_at: revision,
        last_opened_at: revision,
        revision,
        content: json!({
            "type": "document",
            "html": "<h1>Project brief</h1>",
            "pageSize": "a4",
        }),
        folder_id: None,
        trashed_at: None,
        source: None,
    }
}

fn folder(id: &str, parent_id: Option<&str>) -> WorkFolder {
    WorkFolder {
        id: id.to_string(),
        name: format!("Folder {id}"),
        parent_id: parent_id.map(str::to_string),
        created_at: 1,
        updated_at: 1,
        revision: 1,
        trashed_at: None,
    }
}

#[test]
fn pdf_artifact_kind_uses_the_public_wire_value() {
    assert_eq!(
        serde_json::to_value(WorkArtifactKind::Pdf).expect("serialize PDF kind"),
        json!("pdf")
    );
}

#[tokio::test]
async fn artifacts_are_durable_and_revision_conflicts_are_rejected() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = WorkService::for_test(directory.path());
    let created = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: artifact("artifact-1", 1),
                expected_revision: 0,
            },
        )
        .await
        .expect("create artifact");
    assert_eq!(created.revision, 1);

    let mut changed = created.clone();
    changed.title = "Annual plan".to_string();
    changed.revision = 2;
    let saved = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: changed.clone(),
                expected_revision: 1,
            },
        )
        .await
        .expect("save artifact");
    assert_eq!(saved.title, "Annual plan");
    assert_eq!(saved.revision, 2);

    let error = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: artifact("artifact-1", 2),
                expected_revision: 1,
            },
        )
        .await
        .expect_err("stale save should conflict");
    assert!(matches!(
        error,
        BootError::Conflict(message) if message.contains("current revision is 2")
    ));

    let reopened = WorkService::for_test(directory.path())
        .artifact("artifact-1")
        .await
        .expect("reopen artifact");
    assert_eq!(reopened, saved);
}

#[tokio::test]
async fn trash_restore_copy_and_purge_preserve_explicit_lifecycle() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = WorkService::for_test(directory.path());
    let original = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: artifact("artifact-1", 1),
                expected_revision: 0,
            },
        )
        .await
        .expect("create artifact");
    let copy = service
        .copy_artifact(
            &original.id,
            CopyArtifactRequest {
                id: "artifact-copy".to_string(),
                title: Some("Project brief copy".to_string()),
                folder_id: None,
            },
        )
        .await
        .expect("copy artifact");
    assert_eq!(copy.revision, 1);
    assert_eq!(copy.title, "Project brief copy");

    let trashed = service
        .trash_artifact(
            &original.id,
            RevisionRequest {
                expected_revision: original.revision,
            },
        )
        .await
        .expect("trash artifact");
    assert!(trashed.trashed_at.is_some());
    let visible = service.library(false).await.expect("visible library");
    assert_eq!(visible["artifacts"].as_array().map(Vec::len), Some(1));

    let restored = service
        .restore_artifact(
            &original.id,
            RevisionRequest {
                expected_revision: trashed.revision,
            },
        )
        .await
        .expect("restore artifact");
    assert!(restored.trashed_at.is_none());
    let trashed_again = service
        .trash_artifact(
            &original.id,
            RevisionRequest {
                expected_revision: restored.revision,
            },
        )
        .await
        .expect("trash artifact again");
    service
        .purge_artifact(&trashed_again.id)
        .await
        .expect("purge artifact");
    assert!(matches!(
        service.artifact(&trashed_again.id).await,
        Err(BootError::NotFound(_))
    ));
}

#[tokio::test]
async fn source_bytes_are_bounded_and_survive_service_restarts() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = WorkService::for_test(directory.path());
    let created = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: artifact("artifact-1", 1),
                expected_revision: 0,
            },
        )
        .await
        .expect("create artifact");
    let with_source = service
        .upload_source(
            &created.id,
            created.revision,
            "brief.docx".to_string(),
            Some(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document"
                    .to_string(),
            ),
            b"office bytes",
        )
        .await
        .expect("upload source");
    assert_eq!(with_source.revision, 2);
    assert_eq!(
        with_source
            .source
            .as_ref()
            .map(|source| source.name.as_str()),
        Some("brief.docx")
    );

    let descriptor = WorkService::for_test(directory.path())
        .source(&created.id)
        .await
        .expect("source descriptor");
    assert_eq!(
        tokio::fs::read(descriptor.path)
            .await
            .expect("read source bytes"),
        b"office bytes"
    );
}

#[tokio::test]
async fn version_history_can_restore_content_as_a_new_revision() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = WorkService::for_test(directory.path());
    let created = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: artifact("artifact-1", 1),
                expected_revision: 0,
            },
        )
        .await
        .expect("create artifact");
    let mut changed = created.clone();
    changed.title = "Changed title".to_string();
    changed.revision = 2;
    let changed = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: changed,
                expected_revision: 1,
            },
        )
        .await
        .expect("save revision 2");

    let versions = service.versions("artifact-1").await.expect("list versions");
    assert_eq!(
        versions
            .iter()
            .map(|version| version.revision)
            .collect::<Vec<_>>(),
        vec![2, 1]
    );
    let restored = service
        .restore_version(
            "artifact-1",
            RestoreVersionRequest {
                version: 1,
                expected_revision: changed.revision,
            },
        )
        .await
        .expect("restore revision");
    assert_eq!(restored.revision, 3);
    assert_eq!(restored.title, "Project brief");
}

#[tokio::test]
async fn folders_reject_cycles_and_non_empty_purge() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = WorkService::for_test(directory.path());
    let parent = service
        .save_folder(
            "parent",
            SaveFolderRequest {
                folder: folder("parent", None),
                expected_revision: 0,
            },
        )
        .await
        .expect("create parent");
    let child = service
        .save_folder(
            "child",
            SaveFolderRequest {
                folder: folder("child", Some("parent")),
                expected_revision: 0,
            },
        )
        .await
        .expect("create child");

    let mut cyclic = parent.clone();
    cyclic.parent_id = Some(child.id.clone());
    let error = service
        .save_folder(
            &parent.id,
            SaveFolderRequest {
                folder: cyclic,
                expected_revision: parent.revision,
            },
        )
        .await
        .expect_err("cycle should fail");
    assert!(matches!(error, BootError::Conflict(_)));

    let trashed_parent = service
        .trash_folder(
            &parent.id,
            RevisionRequest {
                expected_revision: parent.revision,
            },
        )
        .await
        .expect("trash parent");
    assert!(trashed_parent.trashed_at.is_some());
    let error = service
        .purge_folder(&parent.id)
        .await
        .expect_err("non-empty folder should not purge");
    assert!(matches!(error, BootError::Conflict(_)));
}

#[tokio::test]
async fn identifiers_and_content_kinds_are_validated_before_path_use() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = WorkService::for_test(directory.path());
    let error = service
        .save_artifact(
            "../outside",
            SaveArtifactRequest {
                artifact: artifact("../outside", 1),
                expected_revision: 0,
            },
        )
        .await
        .expect_err("path traversal should fail");
    assert!(matches!(error, BootError::BadRequest(_)));

    let mut mismatched = artifact("artifact-1", 1);
    mismatched.content["type"] = json!("spreadsheet");
    let error = service
        .save_artifact(
            "artifact-1",
            SaveArtifactRequest {
                artifact: mismatched,
                expected_revision: 0,
            },
        )
        .await
        .expect_err("kind mismatch should fail");
    assert!(matches!(error, BootError::BadRequest(_)));
}

#[tokio::test]
async fn controller_routes_round_trip_artifacts_and_stream_source_files() {
    let directory = tempfile::tempdir().expect("temporary Work directory");
    let service = Arc::new(WorkService::for_test(directory.path()));
    let app = BootApplication::builder()
        .global_prefix("/api")
        .import(TestWorkModule {
            service: Arc::clone(&service),
        })
        .build()
        .expect("build Work test application");

    let request = SaveArtifactRequest {
        artifact: artifact("artifact-1", 1),
        expected_revision: 0,
    };
    let created = app
        .call(
            BootRequest::new(HttpMethod::Put, "/api/v1/work/artifacts/artifact-1")
                .with_header("accept", "application/json")
                .with_content_type("application/json")
                .with_body(serde_json::to_vec(&request).expect("encode artifact")),
        )
        .await
        .expect("create artifact through controller");
    assert_eq!(created.status(), 200);
    assert_eq!(
        created
            .body_json::<WorkArtifact>()
            .expect("artifact response")
            .revision,
        1
    );

    let uploaded = app
        .call(
            BootRequest::new(
                HttpMethod::Put,
                "/api/v1/work/artifacts/artifact-1/source?expectedRevision=1&fileName=brief.docx",
            )
            .with_header("accept", "application/json")
            .with_content_type(
                "application/vnd.openxmlformats-officedocument.wordprocessingml.document",
            )
            .with_body("office bytes"),
        )
        .await
        .expect("upload source through controller");
    assert_eq!(
        uploaded
            .body_json::<WorkArtifact>()
            .expect("uploaded artifact")
            .source
            .as_ref()
            .map(|source| source.size),
        Some(12)
    );

    let downloaded = app
        .call(BootRequest::new(
            HttpMethod::Get,
            "/api/v1/work/artifacts/artifact-1/source",
        ))
        .await
        .expect("download source through controller");
    assert!(downloaded.is_streaming());
    assert_eq!(
        downloaded.content_type(),
        Some("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
    );
    assert_eq!(
        downloaded.header("content-disposition"),
        Some(r#"attachment; filename="brief.docx""#)
    );
}

struct TestWorkModule {
    service: Arc<WorkService>,
}

impl Module for TestWorkModule {
    fn name(&self) -> &'static str {
        "test-work"
    }

    fn controllers(&self, _module_ref: &ModuleRef) -> BootResult<Vec<ControllerDefinition>> {
        Ok(vec![Arc::new(WorkController::new(Arc::clone(
            &self.service,
        )))
        .controller()?])
    }
}
