use std::sync::Arc;

use a3s_boot::{controller, Result as BootResult};

use super::dto::{
    CodeIntelligenceStatusResponse, DiagnosticResponse, DocumentSymbolResponse, NavigationResponse,
    WorkspaceSymbolResponse,
};
use super::service::CodeIntelligenceService;

pub(super) struct CodeIntelligenceController {
    service: Arc<CodeIntelligenceService>,
}

impl CodeIntelligenceController {
    pub(super) fn new(service: Arc<CodeIntelligenceService>) -> Self {
        Self { service }
    }
}

#[controller("/v1/workspace/code-intelligence")]
impl CodeIntelligenceController {
    #[get("/status")]
    async fn status(
        &self,
        #[query("sessionId")] session_id: Option<String>,
    ) -> BootResult<CodeIntelligenceStatusResponse> {
        self.service.status(session_id).await
    }

    #[get("/outline")]
    async fn document_outline(
        &self,
        #[query("path")] path: String,
        #[query("sessionId")] session_id: Option<String>,
    ) -> BootResult<DocumentSymbolResponse> {
        self.service.document_outline(path, session_id).await
    }

    #[get("/symbols")]
    async fn search_symbols(
        &self,
        #[query("query")] query: String,
        #[query("limit")] limit: Option<usize>,
        #[query("sessionId")] session_id: Option<String>,
    ) -> BootResult<WorkspaceSymbolResponse> {
        self.service.search_symbols(query, limit, session_id).await
    }

    #[get("/navigation")]
    async fn navigate(
        &self,
        #[query("path")] path: String,
        #[query("line")] line: u32,
        #[query("character")] character: u32,
        #[query("kind")] kind: String,
        #[query("sessionId")] session_id: Option<String>,
    ) -> BootResult<NavigationResponse> {
        self.service
            .navigate(path, line, character, kind, session_id)
            .await
    }

    #[get("/diagnostics")]
    async fn diagnostics(
        &self,
        #[query("path")] path: Option<String>,
        #[query("sessionId")] session_id: Option<String>,
    ) -> BootResult<DiagnosticResponse> {
        self.service.diagnostics(path, session_id).await
    }
}
