use super::*;
use a3s_code_core::workspace::WorkspaceFileSystem;
use a3s_code_core::{CodeDiagnostic, CodeIntelligenceStatus};

pub(super) async fn execute_ide_intelligence_query(
    provider: Arc<dyn WorkspaceCodeIntelligence>,
    file_system: Arc<dyn WorkspaceFileSystem>,
    prepared: PreparedIdeIntelligenceQuery,
    cancellation: CancellationToken,
) -> Result<IdeIntelligenceResult, String> {
    if cancellation.is_cancelled() {
        return Err("Code Intelligence query cancelled".to_owned());
    }
    let title = prepared.title;
    let saved_version = prepared.saved_version;
    let dirty_buffer = prepared.dirty_buffer;
    match prepared.task {
        IdeIntelligenceTask::Status => {
            let status = provider.status();
            Ok(status_result(title, status, dirty_buffer))
        }
        IdeIntelligenceTask::DocumentSymbols { path } => {
            let result = provider
                .document_symbols(&path, cancellation)
                .await
                .map_err(|error| error.to_string())?;
            let rows = document_symbol_rows(result.items, path.as_str());
            Ok(query_result(
                title,
                rows,
                result.truncated,
                saved_version,
                dirty_buffer,
                result.workspace_revision,
                result
                    .document
                    .as_ref()
                    .is_some_and(|document| document.stale),
            ))
        }
        IdeIntelligenceTask::WorkspaceSymbols { query } => {
            let result = provider
                .search_symbols(&query, WORKSPACE_SYMBOL_LIMIT, cancellation)
                .await
                .map_err(|error| error.to_string())?;
            let rows = result.items.into_iter().map(workspace_symbol_row).collect();
            Ok(query_result(
                title,
                rows,
                result.truncated,
                saved_version,
                dirty_buffer,
                result.workspace_revision,
                false,
            ))
        }
        IdeIntelligenceTask::Navigate {
            kind,
            path,
            row,
            expanded_col,
        } => {
            let saved = tokio::select! {
                _ = cancellation.cancelled() => {
                    return Err("Code Intelligence query cancelled".to_owned());
                }
                result = file_system.read_text(&path) => result.map_err(|error| {
                    format!("failed to read saved file {}: {error}", path.as_str())
                })?,
            };
            let position = editor_position_to_saved_utf16(&saved, row, expanded_col)?;
            let result = provider
                .navigate(kind, &path, position, cancellation)
                .await
                .map_err(|error| error.to_string())?;
            let rows = result.items.into_iter().map(navigation_row).collect();
            Ok(query_result(
                title,
                rows,
                result.truncated,
                saved_version,
                dirty_buffer,
                result.workspace_revision,
                result
                    .document
                    .as_ref()
                    .is_some_and(|document| document.stale),
            ))
        }
        IdeIntelligenceTask::Diagnostics { path } => {
            let result = provider
                .diagnostics(path.as_ref(), cancellation)
                .await
                .map_err(|error| error.to_string())?;
            let rows = result.items.into_iter().map(diagnostic_row).collect();
            Ok(query_result(
                title,
                rows,
                result.truncated,
                saved_version,
                dirty_buffer,
                result.workspace_revision,
                result
                    .document
                    .as_ref()
                    .is_some_and(|document| document.stale),
            ))
        }
    }
}

fn query_result(
    title: String,
    rows: Vec<IdeIntelligenceRow>,
    truncated: bool,
    saved_version: bool,
    dirty_buffer: bool,
    workspace_revision: u64,
    stale: bool,
) -> IdeIntelligenceResult {
    IdeIntelligenceResult {
        title,
        rows,
        truncated,
        saved_version,
        dirty_buffer,
        stale,
        workspace_revision: Some(workspace_revision),
    }
}

fn status_result(
    title: String,
    status: CodeIntelligenceStatus,
    dirty_buffer: bool,
) -> IdeIntelligenceResult {
    let mut rows = vec![IdeIntelligenceRow {
        text: format!(
            "Workspace: {} · {}",
            intelligence_state_label(status.state),
            capability_labels(status.capabilities)
        ),
        target: None,
    }];
    if let Some(message) = status.message.filter(|message| !message.trim().is_empty()) {
        rows.push(IdeIntelligenceRow {
            text: message,
            target: None,
        });
    }
    for language in status.languages {
        let mut text = format!(
            "{}: {} · {}",
            language.language,
            intelligence_state_label(language.state),
            capability_labels(language.capabilities)
        );
        if let Some(message) = language
            .message
            .filter(|message| !message.trim().is_empty())
        {
            text.push_str(" · ");
            text.push_str(&message);
        }
        rows.push(IdeIntelligenceRow { text, target: None });
    }
    IdeIntelligenceResult {
        title,
        rows,
        truncated: false,
        saved_version: false,
        dirty_buffer,
        stale: false,
        workspace_revision: None,
    }
}

fn document_symbol_rows(symbols: Vec<DocumentSymbol>, path: &str) -> Vec<IdeIntelligenceRow> {
    fn append(
        rows: &mut Vec<IdeIntelligenceRow>,
        symbols: Vec<DocumentSymbol>,
        path: &str,
        depth: usize,
    ) {
        for symbol in symbols {
            let detail = symbol
                .detail
                .as_deref()
                .filter(|detail| !detail.trim().is_empty())
                .map(|detail| format!(" · {detail}"))
                .unwrap_or_default();
            rows.push(IdeIntelligenceRow {
                text: format!(
                    "{}{} · {}{} · {}",
                    "  ".repeat(depth),
                    symbol.name,
                    symbol_kind_label(symbol.kind),
                    detail,
                    display_position(symbol.selection_range.start)
                ),
                target: Some(IdeIntelligenceTarget {
                    path: path.to_owned(),
                    position: symbol.selection_range.start,
                }),
            });
            append(rows, symbol.children, path, depth + 1);
        }
    }

    let mut rows = Vec::new();
    append(&mut rows, symbols, path, 0);
    rows
}

fn workspace_symbol_row(symbol: SymbolInformation) -> IdeIntelligenceRow {
    let container = symbol
        .container_name
        .as_deref()
        .filter(|container| !container.trim().is_empty())
        .map(|container| format!(" · {container}"))
        .unwrap_or_default();
    let target = target_from_location(&symbol.location);
    IdeIntelligenceRow {
        text: format!(
            "{} · {}{} · {}:{}",
            symbol.name,
            symbol_kind_label(symbol.kind),
            container,
            symbol.location.path.as_str(),
            display_position(symbol.location.range.start)
        ),
        target: Some(target),
    }
}

fn navigation_row(location: CodeLocation) -> IdeIntelligenceRow {
    let target = target_from_location(&location);
    IdeIntelligenceRow {
        text: format!(
            "{}:{}",
            location.path.as_str(),
            display_position(location.range.start)
        ),
        target: Some(target),
    }
}

fn diagnostic_row(diagnostic: CodeDiagnostic) -> IdeIntelligenceRow {
    let severity = diagnostic
        .severity
        .map(diagnostic_severity_label)
        .unwrap_or("diagnostic");
    let mut origin = diagnostic.source.unwrap_or_default();
    if let Some(code) = diagnostic.code {
        if !origin.is_empty() {
            origin.push('/');
        }
        origin.push_str(&code);
    }
    let origin = if origin.is_empty() {
        String::new()
    } else {
        format!(" [{origin}]")
    };
    let target = target_from_location(&diagnostic.location);
    IdeIntelligenceRow {
        text: format!(
            "{} · {}:{} · {}{}",
            severity,
            diagnostic.location.path.as_str(),
            display_position(diagnostic.location.range.start),
            diagnostic.message.replace(['\r', '\n'], " "),
            origin
        ),
        target: Some(target),
    }
}

fn target_from_location(location: &CodeLocation) -> IdeIntelligenceTarget {
    IdeIntelligenceTarget {
        path: location.path.as_str().to_owned(),
        position: location.range.start,
    }
}

fn display_position(position: CodePosition) -> String {
    format!(
        "{}:{}",
        position.line.saturating_add(1),
        position.character.saturating_add(1)
    )
}

pub(super) fn navigation_label(kind: NavigationKind) -> &'static str {
    match kind {
        NavigationKind::Definition => "Definition",
        NavigationKind::Declaration => "Declaration",
        NavigationKind::References => "References",
        NavigationKind::Implementations => "Implementations",
    }
}

fn intelligence_state_label(state: CodeIntelligenceState) -> &'static str {
    match state {
        CodeIntelligenceState::Starting => "starting",
        CodeIntelligenceState::Ready => "ready",
        CodeIntelligenceState::Degraded => "degraded",
        CodeIntelligenceState::Unavailable => "unavailable",
    }
}

fn capability_labels(capabilities: CodeIntelligenceCapabilities) -> String {
    let mut labels = Vec::new();
    if capabilities.document_symbols {
        labels.push("document symbols");
    }
    if capabilities.workspace_symbols {
        labels.push("workspace symbols");
    }
    if capabilities.definition {
        labels.push("definition");
    }
    if capabilities.declaration {
        labels.push("declaration");
    }
    if capabilities.references {
        labels.push("references");
    }
    if capabilities.implementations {
        labels.push("implementations");
    }
    if capabilities.diagnostics {
        labels.push("diagnostics");
    }
    if labels.is_empty() {
        "no capabilities".to_owned()
    } else {
        labels.join(", ")
    }
}

fn symbol_kind_label(kind: CodeSymbolKind) -> &'static str {
    match kind {
        CodeSymbolKind::File => "file",
        CodeSymbolKind::Module => "module",
        CodeSymbolKind::Namespace => "namespace",
        CodeSymbolKind::Package => "package",
        CodeSymbolKind::Class => "class",
        CodeSymbolKind::Method => "method",
        CodeSymbolKind::Property => "property",
        CodeSymbolKind::Field => "field",
        CodeSymbolKind::Constructor => "constructor",
        CodeSymbolKind::Enum => "enum",
        CodeSymbolKind::Interface => "interface",
        CodeSymbolKind::Function => "function",
        CodeSymbolKind::Variable => "variable",
        CodeSymbolKind::Constant => "constant",
        CodeSymbolKind::String => "string",
        CodeSymbolKind::Number => "number",
        CodeSymbolKind::Boolean => "boolean",
        CodeSymbolKind::Array => "array",
        CodeSymbolKind::Object => "object",
        CodeSymbolKind::Key => "key",
        CodeSymbolKind::Null => "null",
        CodeSymbolKind::EnumMember => "enum member",
        CodeSymbolKind::Struct => "struct",
        CodeSymbolKind::Event => "event",
        CodeSymbolKind::Operator => "operator",
        CodeSymbolKind::TypeParameter => "type parameter",
        CodeSymbolKind::Unknown => "symbol",
        _ => "symbol",
    }
}

fn diagnostic_severity_label(severity: CodeDiagnosticSeverity) -> &'static str {
    match severity {
        CodeDiagnosticSeverity::Error => "error",
        CodeDiagnosticSeverity::Warning => "warning",
        CodeDiagnosticSeverity::Information => "information",
        CodeDiagnosticSeverity::Hint => "hint",
    }
}
