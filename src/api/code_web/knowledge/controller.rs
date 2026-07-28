use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};
use serde::Deserialize;

use super::compilation::{CompilationOutcome, CompilationPolicy};
use super::service::KnowledgeService;

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KbWorkspaceRequest {
    pub(super) workspace: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KbAddNoteRequest {
    pub(super) workspace: Option<String>,
    pub(super) text: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KbImportRequest {
    pub(super) workspace: Option<String>,
    pub(super) path: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KbSearchRequest {
    pub(super) workspace: Option<String>,
    pub(super) query: String,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeBaseCreateRequest {
    pub(super) workspace: Option<String>,
    pub(super) name: String,
    pub(super) description: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeBaseImportRequest {
    pub(super) workspace: Option<String>,
    pub(super) path: String,
    pub(super) name: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeBasePinRequest {
    pub(super) workspace: Option<String>,
    pub(super) pinned: bool,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeBaseSelectionRequest {
    pub(super) workspace: Option<String>,
    pub(super) paths: Vec<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeBaseFromSelectionRequest {
    pub(super) workspace: Option<String>,
    pub(super) paths: Vec<String>,
    pub(super) name: String,
    pub(super) description: Option<String>,
    pub(super) compilation_policy: CompilationPolicy,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeCompilationRequest {
    pub(super) workspace: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeCompilationPolicyRequest {
    pub(super) workspace: Option<String>,
    pub(super) policy: CompilationPolicy,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeCompilationResultRequest {
    pub(super) workspace: Option<String>,
    pub(super) outcome: Option<CompilationOutcome>,
    pub(super) transient: Option<bool>,
    pub(super) error: Option<String>,
    pub(super) compiler_version: Option<String>,
}

pub(super) struct KnowledgeController {
    service: Arc<KnowledgeService>,
}

impl KnowledgeController {
    pub(super) fn new(service: Arc<KnowledgeService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/knowledge")]
impl KnowledgeController {
    #[get("/kb")]
    async fn kb_home(
        &self,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.kb_home(workspace).await
    }

    #[post("/kb/notes")]
    async fn add_note(&self, #[body] request: KbAddNoteRequest) -> BootResult<serde_json::Value> {
        self.service.add_note(request).await
    }

    #[post("/kb/import/preview")]
    async fn import_preview(
        &self,
        #[body] request: KbImportRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.import_preview(request).await
    }

    #[post("/kb/import")]
    async fn import(&self, #[body] request: KbImportRequest) -> BootResult<serde_json::Value> {
        self.service.import(request).await
    }

    #[post("/kb/search")]
    async fn search(&self, #[body] request: KbSearchRequest) -> BootResult<serde_json::Value> {
        self.service.search(request).await
    }

    #[post("/kb/ensure")]
    async fn ensure(&self, #[body] request: KbWorkspaceRequest) -> BootResult<serde_json::Value> {
        self.service.ensure(request.workspace).await
    }

    #[get("/marketplace")]
    async fn marketplace(
        &self,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.marketplace(workspace).await
    }

    #[post("/marketplace/{id}/install")]
    async fn install_marketplace_item(
        &self,
        #[param("id")] id: String,
        #[body] request: KbWorkspaceRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .install_marketplace_item(&id, request.workspace)
            .await
    }

    #[get("/bases")]
    async fn knowledge_bases(
        &self,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.knowledge_bases(workspace).await
    }

    #[post("/bases")]
    async fn create_knowledge_base(
        &self,
        #[body] request: KnowledgeBaseCreateRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.create_knowledge_base(request).await
    }

    #[post("/bases/import")]
    async fn import_knowledge_base(
        &self,
        #[body] request: KnowledgeBaseImportRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.import_knowledge_base(request).await
    }

    #[post("/bases/from-selection/preview")]
    async fn preview_knowledge_base_from_selection(
        &self,
        #[body] request: KnowledgeBaseSelectionRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .preview_knowledge_base_from_selection(request)
            .await
    }

    #[post("/bases/from-selection")]
    async fn create_knowledge_base_from_selection(
        &self,
        #[body] request: KnowledgeBaseFromSelectionRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .create_knowledge_base_from_selection(request)
            .await
    }

    #[post("/bases/{id}/pinned")]
    async fn set_knowledge_base_pinned(
        &self,
        #[param("id")] id: String,
        #[body] request: KnowledgeBasePinRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.set_knowledge_base_pinned(&id, request).await
    }

    #[post("/bases/{id}/compilations")]
    async fn request_knowledge_compilation(
        &self,
        #[param("id")] id: String,
        #[body] request: KnowledgeCompilationRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .request_knowledge_compilation(&id, request)
            .await
    }

    #[get("/bases/{id}/compilation")]
    async fn knowledge_compilation_status(
        &self,
        #[param("id")] id: String,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service
            .knowledge_compilation_status(&id, workspace)
            .await
    }

    #[patch("/bases/{id}/compilation-policy")]
    async fn set_knowledge_compilation_policy(
        &self,
        #[param("id")] id: String,
        #[body] request: KnowledgeCompilationPolicyRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .set_knowledge_compilation_policy(&id, request)
            .await
    }

    #[get("/bases/{id}/source-changes")]
    async fn knowledge_source_changes(
        &self,
        #[param("id")] id: String,
        #[query("workspace")] workspace: Option<String>,
    ) -> BootResult<serde_json::Value> {
        self.service.knowledge_source_changes(&id, workspace).await
    }

    #[post("/bases/{id}/compilations/{job_id}/cancel")]
    async fn cancel_knowledge_compilation(
        &self,
        #[param("id")] id: String,
        #[param("job_id")] job_id: String,
        #[body] request: KnowledgeCompilationRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .cancel_knowledge_compilation(&id, &job_id, request)
            .await
    }

    #[post("/compilations/claim")]
    async fn claim_knowledge_compilation(
        &self,
        #[body] request: KnowledgeCompilationRequest,
    ) -> BootResult<serde_json::Value> {
        self.service.claim_knowledge_compilation(request).await
    }

    #[post("/bases/{id}/compilations/{job_id}/result")]
    async fn complete_knowledge_compilation(
        &self,
        #[param("id")] id: String,
        #[param("job_id")] job_id: String,
        #[body] request: KnowledgeCompilationResultRequest,
    ) -> BootResult<serde_json::Value> {
        self.service
            .complete_knowledge_compilation(&id, &job_id, request)
            .await
    }
}
