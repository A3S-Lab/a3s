use a3s_code_core::{
    CodeDiagnostic, CodeDiagnosticSeverity, CodeIntelligenceCapabilities,
    CodeIntelligenceLanguageStatus, CodeIntelligenceState, CodeIntelligenceStatus, CodeLocation,
    CodePosition, CodeQueryResult, CodeRange, CodeSymbolKind, DocumentSnapshot, DocumentSymbol,
    SymbolInformation,
};
use serde::Serialize;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeIntelligenceStatusResponse {
    state: CodeIntelligenceStateDto,
    capabilities: CodeIntelligenceCapabilitiesDto,
    languages: Vec<CodeIntelligenceLanguageStatusDto>,
    message: Option<String>,
}

impl From<CodeIntelligenceStatus> for CodeIntelligenceStatusResponse {
    fn from(status: CodeIntelligenceStatus) -> Self {
        Self {
            state: status.state.into(),
            capabilities: status.capabilities.into(),
            languages: status.languages.into_iter().map(Into::into).collect(),
            message: status.message,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeIntelligenceLanguageStatusDto {
    language: String,
    state: CodeIntelligenceStateDto,
    capabilities: CodeIntelligenceCapabilitiesDto,
    message: Option<String>,
}

impl From<CodeIntelligenceLanguageStatus> for CodeIntelligenceLanguageStatusDto {
    fn from(status: CodeIntelligenceLanguageStatus) -> Self {
        Self {
            language: status.language.to_string(),
            state: status.state.into(),
            capabilities: status.capabilities.into(),
            message: status.message,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CodeIntelligenceCapabilitiesDto {
    document_symbols: bool,
    workspace_symbols: bool,
    definition: bool,
    declaration: bool,
    references: bool,
    implementations: bool,
    diagnostics: bool,
}

impl From<CodeIntelligenceCapabilities> for CodeIntelligenceCapabilitiesDto {
    fn from(capabilities: CodeIntelligenceCapabilities) -> Self {
        Self {
            document_symbols: capabilities.document_symbols,
            workspace_symbols: capabilities.workspace_symbols,
            definition: capabilities.definition,
            declaration: capabilities.declaration,
            references: capabilities.references,
            implementations: capabilities.implementations,
            diagnostics: capabilities.diagnostics,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum CodeIntelligenceStateDto {
    Starting,
    Ready,
    Degraded,
    Unavailable,
}

impl From<CodeIntelligenceState> for CodeIntelligenceStateDto {
    fn from(state: CodeIntelligenceState) -> Self {
        match state {
            CodeIntelligenceState::Starting => Self::Starting,
            CodeIntelligenceState::Ready => Self::Ready,
            CodeIntelligenceState::Degraded => Self::Degraded,
            CodeIntelligenceState::Unavailable => Self::Unavailable,
        }
    }
}

pub(in crate::api::code_web) type DocumentSymbolResponse = QueryResponse<DocumentSymbolDto>;
pub(in crate::api::code_web) type WorkspaceSymbolResponse = QueryResponse<SymbolInformationDto>;
pub(in crate::api::code_web) type NavigationResponse = QueryResponse<CodeLocationDto>;
pub(in crate::api::code_web) type DiagnosticResponse = QueryResponse<CodeDiagnosticDto>;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct QueryResponse<T> {
    items: Vec<T>,
    truncated: bool,
    workspace_revision: u64,
    document: Option<DocumentSnapshotDto>,
}

impl<T, U> From<CodeQueryResult<T>> for QueryResponse<U>
where
    U: From<T>,
{
    fn from(result: CodeQueryResult<T>) -> Self {
        Self {
            items: result.items.into_iter().map(Into::into).collect(),
            truncated: result.truncated,
            workspace_revision: result.workspace_revision,
            document: result.document.map(Into::into),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DocumentSnapshotDto {
    revision: u64,
    content_hash: String,
    stale: bool,
}

impl From<DocumentSnapshot> for DocumentSnapshotDto {
    fn from(snapshot: DocumentSnapshot) -> Self {
        Self {
            revision: snapshot.revision.value(),
            content_hash: snapshot.content_hash,
            stale: snapshot.stale,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct DocumentSymbolDto {
    name: String,
    detail: Option<String>,
    kind: CodeSymbolKindDto,
    range: CodeRangeDto,
    selection_range: CodeRangeDto,
    children: Vec<DocumentSymbolDto>,
}

impl From<DocumentSymbol> for DocumentSymbolDto {
    fn from(symbol: DocumentSymbol) -> Self {
        Self {
            name: symbol.name,
            detail: symbol.detail,
            kind: symbol.kind.into(),
            range: symbol.range.into(),
            selection_range: symbol.selection_range.into(),
            children: symbol.children.into_iter().map(Into::into).collect(),
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct SymbolInformationDto {
    name: String,
    kind: CodeSymbolKindDto,
    location: CodeLocationDto,
    container_name: Option<String>,
}

impl From<SymbolInformation> for SymbolInformationDto {
    fn from(symbol: SymbolInformation) -> Self {
        Self {
            name: symbol.name,
            kind: symbol.kind.into(),
            location: symbol.location.into(),
            container_name: symbol.container_name,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeDiagnosticDto {
    location: CodeLocationDto,
    severity: Option<CodeDiagnosticSeverityDto>,
    code: Option<String>,
    source: Option<String>,
    message: String,
}

impl From<CodeDiagnostic> for CodeDiagnosticDto {
    fn from(diagnostic: CodeDiagnostic) -> Self {
        Self {
            location: diagnostic.location.into(),
            severity: diagnostic.severity.map(Into::into),
            code: diagnostic.code,
            source: diagnostic.source,
            message: diagnostic.message,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum CodeDiagnosticSeverityDto {
    Error,
    Warning,
    Information,
    Hint,
}

impl From<CodeDiagnosticSeverity> for CodeDiagnosticSeverityDto {
    fn from(severity: CodeDiagnosticSeverity) -> Self {
        match severity {
            CodeDiagnosticSeverity::Error => Self::Error,
            CodeDiagnosticSeverity::Warning => Self::Warning,
            CodeDiagnosticSeverity::Information => Self::Information,
            CodeDiagnosticSeverity::Hint => Self::Hint,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(in crate::api::code_web) struct CodeLocationDto {
    path: String,
    range: CodeRangeDto,
}

impl From<CodeLocation> for CodeLocationDto {
    fn from(location: CodeLocation) -> Self {
        Self {
            path: location.path.as_str().to_owned(),
            range: location.range.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CodeRangeDto {
    start: CodePositionDto,
    end: CodePositionDto,
}

impl From<CodeRange> for CodeRangeDto {
    fn from(range: CodeRange) -> Self {
        Self {
            start: range.start.into(),
            end: range.end.into(),
        }
    }
}

#[derive(Debug, Serialize)]
struct CodePositionDto {
    line: u32,
    character: u32,
}

impl From<CodePosition> for CodePositionDto {
    fn from(position: CodePosition) -> Self {
        Self {
            line: position.line,
            character: position.character,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum CodeSymbolKindDto {
    File,
    Module,
    Namespace,
    Package,
    Class,
    Method,
    Property,
    Field,
    Constructor,
    Enum,
    Interface,
    Function,
    Variable,
    Constant,
    String,
    Number,
    Boolean,
    Array,
    Object,
    Key,
    Null,
    EnumMember,
    Struct,
    Event,
    Operator,
    TypeParameter,
    Unknown,
}

impl From<CodeSymbolKind> for CodeSymbolKindDto {
    fn from(kind: CodeSymbolKind) -> Self {
        match kind {
            CodeSymbolKind::File => Self::File,
            CodeSymbolKind::Module => Self::Module,
            CodeSymbolKind::Namespace => Self::Namespace,
            CodeSymbolKind::Package => Self::Package,
            CodeSymbolKind::Class => Self::Class,
            CodeSymbolKind::Method => Self::Method,
            CodeSymbolKind::Property => Self::Property,
            CodeSymbolKind::Field => Self::Field,
            CodeSymbolKind::Constructor => Self::Constructor,
            CodeSymbolKind::Enum => Self::Enum,
            CodeSymbolKind::Interface => Self::Interface,
            CodeSymbolKind::Function => Self::Function,
            CodeSymbolKind::Variable => Self::Variable,
            CodeSymbolKind::Constant => Self::Constant,
            CodeSymbolKind::String => Self::String,
            CodeSymbolKind::Number => Self::Number,
            CodeSymbolKind::Boolean => Self::Boolean,
            CodeSymbolKind::Array => Self::Array,
            CodeSymbolKind::Object => Self::Object,
            CodeSymbolKind::Key => Self::Key,
            CodeSymbolKind::Null => Self::Null,
            CodeSymbolKind::EnumMember => Self::EnumMember,
            CodeSymbolKind::Struct => Self::Struct,
            CodeSymbolKind::Event => Self::Event,
            CodeSymbolKind::Operator => Self::Operator,
            CodeSymbolKind::TypeParameter => Self::TypeParameter,
            _ => Self::Unknown,
        }
    }
}

#[cfg(test)]
mod tests {
    use a3s_code_core::{
        CodePosition, CodeRange, CodeSymbolKind, DocumentRevision, DocumentSnapshot, DocumentSymbol,
    };

    use super::*;

    #[test]
    fn query_response_preserves_saved_document_evidence() {
        let result = CodeQueryResult {
            items: vec![DocumentSymbol {
                name: "main".to_owned(),
                detail: None,
                kind: CodeSymbolKind::Function,
                range: CodeRange::new(CodePosition::new(1, 0), CodePosition::new(3, 1)),
                selection_range: CodeRange::new(CodePosition::new(1, 3), CodePosition::new(1, 7)),
                children: Vec::new(),
            }],
            truncated: true,
            workspace_revision: 17,
            document: Some(DocumentSnapshot {
                revision: DocumentRevision::new(4),
                content_hash: "hash".to_owned(),
                stale: false,
            }),
        };

        let value =
            serde_json::to_value(DocumentSymbolResponse::from(result)).expect("serialize response");

        assert_eq!(value["workspaceRevision"], 17);
        assert_eq!(value["truncated"], true);
        assert_eq!(value["document"]["revision"], 4);
        assert_eq!(value["document"]["contentHash"], "hash");
        assert_eq!(value["items"][0]["kind"], "function");
        assert_eq!(value["items"][0]["selectionRange"]["start"]["character"], 3);
    }
}
