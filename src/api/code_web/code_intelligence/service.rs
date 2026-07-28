use std::path::{Path, PathBuf};
use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::{
    CodeIntelligenceError, CodePosition, NavigationKind, WorkspaceCodeIntelligence, WorkspacePath,
    WorkspaceServices,
};
use tokio_util::sync::CancellationToken;

use super::dto::{
    CodeIntelligenceStatusResponse, DiagnosticResponse, DocumentSymbolResponse, NavigationResponse,
    WorkspaceSymbolResponse,
};
use crate::api::code_web::state::CodeWebState;

const DEFAULT_SYMBOL_LIMIT: usize = 100;
const MAX_SYMBOL_LIMIT: usize = 500;

pub(in crate::api::code_web) struct CodeIntelligenceService {
    state: Arc<CodeWebState>,
}

impl CodeIntelligenceService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) async fn status(
        &self,
        session_id: Option<String>,
    ) -> BootResult<CodeIntelligenceStatusResponse> {
        let provider = self.provider(session_id.as_deref()).await?;
        Ok(provider.status().into())
    }

    pub(in crate::api::code_web) async fn document_outline(
        &self,
        path: String,
        session_id: Option<String>,
    ) -> BootResult<DocumentSymbolResponse> {
        let services = self.services(session_id.as_deref()).await?;
        let path = served_document_path(&services, &path).await?;
        let provider = provider_from(&services)?;
        provider
            .document_symbols(&path, CancellationToken::new())
            .await
            .map(Into::into)
            .map_err(map_query_error)
    }

    pub(in crate::api::code_web) async fn search_symbols(
        &self,
        query: String,
        limit: Option<usize>,
        session_id: Option<String>,
    ) -> BootResult<WorkspaceSymbolResponse> {
        let query = query.trim();
        if query.is_empty() {
            return Err(BootError::BadRequest("query is required".to_owned()));
        }
        let limit = parse_symbol_limit(limit)?;
        self.provider(session_id.as_deref())
            .await?
            .search_symbols(query, limit, CancellationToken::new())
            .await
            .map(Into::into)
            .map_err(map_query_error)
    }

    pub(in crate::api::code_web) async fn navigate(
        &self,
        path: String,
        line: u32,
        character: u32,
        kind: String,
        session_id: Option<String>,
    ) -> BootResult<NavigationResponse> {
        let services = self.services(session_id.as_deref()).await?;
        let path = served_document_path(&services, &path).await?;
        let kind = parse_navigation_kind(&kind)?;
        let provider = provider_from(&services)?;
        provider
            .navigate(
                kind,
                &path,
                CodePosition::new(line, character),
                CancellationToken::new(),
            )
            .await
            .map(Into::into)
            .map_err(map_query_error)
    }

    pub(in crate::api::code_web) async fn diagnostics(
        &self,
        path: Option<String>,
        session_id: Option<String>,
    ) -> BootResult<DiagnosticResponse> {
        let services = self.services(session_id.as_deref()).await?;
        let path = match path {
            Some(path) => Some(served_document_path(&services, &path).await?),
            None => None,
        };
        let provider = provider_from(&services)?;
        provider
            .diagnostics(path.as_ref(), CancellationToken::new())
            .await
            .map(Into::into)
            .map_err(map_query_error)
    }

    async fn services(&self, session_id: Option<&str>) -> BootResult<Arc<WorkspaceServices>> {
        let workspace = {
            let sessions = self.state.sessions.lock().await;
            resolve_served_workspace(&self.state.default_workspace, session_id, |session_id| {
                sessions
                    .get(session_id)
                    .map(|session| session.workspace().to_path_buf())
            })?
        };
        self.state
            .workspace_services_for(&workspace)
            .await
            .map_err(|error| BootError::ServiceUnavailable(error.to_string()))
    }

    async fn provider(
        &self,
        session_id: Option<&str>,
    ) -> BootResult<Arc<dyn WorkspaceCodeIntelligence>> {
        let services = self.services(session_id).await?;
        provider_from(&services)
    }
}

fn resolve_served_workspace(
    default_workspace: &Path,
    session_id: Option<&str>,
    session_workspace: impl FnOnce(&str) -> Option<PathBuf>,
) -> BootResult<PathBuf> {
    let Some(session_id) = session_id else {
        return Ok(default_workspace.to_path_buf());
    };
    let session_id = session_id.trim();
    if session_id.is_empty() {
        return Err(BootError::BadRequest(
            "sessionId must not be empty".to_owned(),
        ));
    }
    session_workspace(session_id)
        .ok_or_else(|| BootError::NotFound(format!("session `{session_id}` was not found")))
}

fn provider_from(services: &WorkspaceServices) -> BootResult<Arc<dyn WorkspaceCodeIntelligence>> {
    services.code_intelligence().ok_or_else(|| {
        BootError::ServiceUnavailable("Code Intelligence is not available".to_owned())
    })
}

async fn served_document_path(
    services: &WorkspaceServices,
    input: &str,
) -> BootResult<WorkspacePath> {
    let input = input.trim();
    if input.is_empty() {
        return Err(BootError::BadRequest("path is required".to_owned()));
    }
    if is_absolute_path(input) {
        return Err(BootError::BadRequest(
            "path must be relative to the served workspace".to_owned(),
        ));
    }
    let path = services
        .normalize_path(input)
        .map_err(|error| BootError::BadRequest(error.to_string()))?;
    if path.is_root() {
        return Err(BootError::BadRequest(
            "path must identify a saved document".to_owned(),
        ));
    }

    let root = services
        .local_root()
        .ok_or_else(|| BootError::Internal("served workspace has no local root".to_owned()))?;
    let resolved = tokio::fs::canonicalize(root.join(path.as_str()))
        .await
        .map_err(|error| {
            BootError::BadRequest(format!(
                "saved document `{}` cannot be resolved: {error}",
                path.as_str()
            ))
        })?;
    if !resolved.starts_with(root) {
        return Err(BootError::Forbidden(
            "path resolves outside the served workspace".to_owned(),
        ));
    }
    let metadata = tokio::fs::metadata(&resolved)
        .await
        .map_err(|error| BootError::BadRequest(error.to_string()))?;
    if !metadata.is_file() {
        return Err(BootError::BadRequest(
            "path must identify a saved document".to_owned(),
        ));
    }
    Ok(path)
}

fn is_absolute_path(input: &str) -> bool {
    let bytes = input.as_bytes();
    Path::new(input).is_absolute()
        || (bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        || input.starts_with("\\\\")
        || input.starts_with("//")
}

fn parse_navigation_kind(input: &str) -> BootResult<NavigationKind> {
    match input.trim() {
        "definition" => Ok(NavigationKind::Definition),
        "declaration" => Ok(NavigationKind::Declaration),
        "references" => Ok(NavigationKind::References),
        "implementations" => Ok(NavigationKind::Implementations),
        _ => Err(BootError::BadRequest(
            "kind must be definition, declaration, references, or implementations".to_owned(),
        )),
    }
}

fn parse_symbol_limit(limit: Option<usize>) -> BootResult<usize> {
    match limit {
        None => Ok(DEFAULT_SYMBOL_LIMIT),
        Some(limit @ 1..=MAX_SYMBOL_LIMIT) => Ok(limit),
        Some(_) => Err(BootError::BadRequest(format!(
            "limit must be between 1 and {MAX_SYMBOL_LIMIT}"
        ))),
    }
}

fn map_query_error(error: CodeIntelligenceError) -> BootError {
    let message = format!("{}: {error}", error.code());
    match error {
        CodeIntelligenceError::InvalidPath { .. }
        | CodeIntelligenceError::InvalidPosition { .. } => BootError::BadRequest(message),
        CodeIntelligenceError::Timeout { .. } | CodeIntelligenceError::Cancelled => {
            BootError::GatewayTimeout(message)
        }
        CodeIntelligenceError::Unsupported { .. } => BootError::NotImplemented(message),
        CodeIntelligenceError::ProcessExited { .. } | CodeIntelligenceError::Unavailable { .. } => {
            BootError::ServiceUnavailable(message)
        }
        CodeIntelligenceError::Protocol { .. } => BootError::BadGateway(message),
        _ => BootError::Internal(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn absolute_path_forms_are_detected() {
        assert!(is_absolute_path("/tmp/main.rs"));
        assert!(is_absolute_path("C:\\temp\\main.rs"));
        assert!(is_absolute_path("\\\\server\\share\\main.rs"));
    }

    #[tokio::test]
    async fn document_paths_are_relative_and_confined_to_served_workspace() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let source_dir = workspace.path().join("src");
        tokio::fs::create_dir(&source_dir)
            .await
            .expect("create source directory");
        let source = source_dir.join("main.rs");
        tokio::fs::write(&source, "fn main() {}")
            .await
            .expect("write source");
        let services = WorkspaceServices::local(workspace.path());

        let path = served_document_path(&services, "src/main.rs")
            .await
            .expect("relative source path");
        assert_eq!(path.as_str(), "src/main.rs");
        assert!(
            served_document_path(&services, &source.display().to_string())
                .await
                .is_err()
        );
        assert!(served_document_path(&services, "../../outside.rs")
            .await
            .is_err());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn document_symlink_cannot_escape_served_workspace() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let outside = tempfile::tempdir().expect("outside directory");
        let outside_source = outside.path().join("outside.rs");
        tokio::fs::write(&outside_source, "fn outside() {}")
            .await
            .expect("write outside source");
        std::os::unix::fs::symlink(&outside_source, workspace.path().join("linked.rs"))
            .expect("create symlink");
        let services = WorkspaceServices::local(workspace.path());

        let error = served_document_path(&services, "linked.rs")
            .await
            .expect_err("symlink escape must be rejected");

        assert!(matches!(error, BootError::Forbidden(_)));
    }

    #[test]
    fn navigation_kind_contract_is_explicit() {
        assert_eq!(
            parse_navigation_kind("definition").expect("definition"),
            NavigationKind::Definition
        );
        assert!(parse_navigation_kind("typeDefinition").is_err());
    }

    #[test]
    fn symbol_limit_is_bounded_without_silent_clamping() {
        assert_eq!(parse_symbol_limit(None).expect("default limit"), 100);
        assert_eq!(parse_symbol_limit(Some(500)).expect("maximum limit"), 500);
        assert!(parse_symbol_limit(Some(0)).is_err());
        assert!(parse_symbol_limit(Some(501)).is_err());
    }

    #[test]
    fn semantic_errors_keep_stable_codes_and_http_categories() {
        let error = map_query_error(CodeIntelligenceError::InvalidPosition {
            path: WorkspacePath::from_normalized("src/main.rs"),
            position: CodePosition::new(4, 8),
        });

        assert!(matches!(error, BootError::BadRequest(_)));
        assert!(error
            .to_string()
            .contains("CODE_INTELLIGENCE_INVALID_POSITION"));
    }

    #[test]
    fn unknown_session_cannot_select_an_arbitrary_workspace() {
        let error = resolve_served_workspace(Path::new("/served"), Some("missing"), |_| None)
            .expect_err("unknown session must be rejected");

        assert!(matches!(error, BootError::NotFound(_)));
    }
}
