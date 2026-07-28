use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use a3s_boot::{BootError, Result as BootResult};
use serde::Serialize;
use serde_json::{json, Value};
use tokio::sync::Mutex;

use super::compilation;
use super::controller::{
    KbAddNoteRequest, KbImportRequest, KbSearchRequest, KnowledgeBaseCreateRequest,
    KnowledgeBaseFromSelectionRequest, KnowledgeBaseImportRequest, KnowledgeBasePinRequest,
    KnowledgeBaseSelectionRequest, KnowledgeCompilationPolicyRequest, KnowledgeCompilationRequest,
    KnowledgeCompilationResultRequest,
};
use super::personal_bases::{self, KnowledgeBaseMutation, KnowledgeStoreError};
use super::source_packages;
use crate::api::code_web::state::CodeWebState;
use crate::tui::kbutil::{self, ImportKind, ImportPreview, KbStats, SearchHit};

pub(in crate::api::code_web) struct KnowledgeService {
    state: Arc<CodeWebState>,
    operation_lock: Arc<Mutex<()>>,
    monitored_workspaces: Arc<RwLock<HashSet<PathBuf>>>,
    _compilation_monitor: Option<CompilationMonitor>,
}

fn start_compilation_monitor(
    workspaces: Arc<RwLock<HashSet<PathBuf>>>,
    operation_lock: Arc<Mutex<()>>,
) -> Option<CompilationMonitor> {
    let runtime = tokio::runtime::Handle::try_current().ok()?;
    Some(CompilationMonitor(runtime.spawn(async move {
        let mut interval = tokio::time::interval(std::time::Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        interval.tick().await;
        loop {
            interval.tick().await;
            let scan_targets = workspaces
                .read()
                .map(|workspaces| workspaces.iter().cloned().collect::<Vec<_>>())
                .unwrap_or_default();
            for workspace in scan_targets {
                if !workspace.is_dir() {
                    continue;
                }
                let _guard = operation_lock.lock().await;
                match tokio::task::spawn_blocking(move || {
                    compilation::poll_compilations(&workspace, chrono::Utc::now())
                })
                .await
                {
                    Ok(Ok(_)) => {}
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "knowledge auto-compilation scan failed");
                    }
                    Err(error) => {
                        tracing::warn!(%error, "knowledge auto-compilation task failed");
                    }
                }
            }
        }
    })))
}

struct CompilationMonitor(tokio::task::JoinHandle<()>);

impl Drop for CompilationMonitor {
    fn drop(&mut self) {
        self.0.abort();
    }
}

impl KnowledgeService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        let operation_lock = Arc::new(Mutex::new(()));
        let monitored_workspaces = Arc::new(RwLock::new(HashSet::from([state
            .default_workspace
            .clone()])));
        let monitor = start_compilation_monitor(
            Arc::clone(&monitored_workspaces),
            Arc::clone(&operation_lock),
        );
        Self {
            state,
            operation_lock,
            monitored_workspaces,
            _compilation_monitor: monitor,
        }
    }

    pub(in crate::api::code_web) async fn kb_home(
        &self,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(workspace);
        self.ensure_workspace(&workspace).await?;
        Ok(kb_home_json(&workspace))
    }

    pub(in crate::api::code_web) async fn ensure(
        &self,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(workspace);
        self.ensure_workspace(&workspace).await?;
        Ok(kb_home_json(&workspace))
    }

    pub(in crate::api::code_web) async fn add_note(
        &self,
        request: KbAddNoteRequest,
    ) -> BootResult<Value> {
        let text = request.text.trim().to_string();
        if text.is_empty() {
            return Err(BootError::BadRequest("note text is required".to_string()));
        }
        let workspace = self.workspace_from_request(request.workspace);
        self.ensure_workspace(&workspace).await?;
        let workspace_text = workspace.display().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let summary = tokio::task::spawn_blocking(move || {
            kbutil::add_text_to_kb(&workspace_text, &text, &now)
        })
        .await
        .map_err(|error| BootError::Internal(format!("failed to add KB note: {error}")))?;
        Ok(kb_action_json(&workspace, summary, "note"))
    }

    pub(in crate::api::code_web) async fn import_preview(
        &self,
        request: KbImportRequest,
    ) -> BootResult<Value> {
        let path = required_path_arg(&request.path)?;
        let workspace = self.workspace_from_request(request.workspace);
        let workspace_text = workspace.display().to_string();
        let preview =
            tokio::task::spawn_blocking(move || kbutil::preview_import(&workspace_text, &path))
                .await
                .map_err(|error| {
                    BootError::Internal(format!("failed to preview KB import: {error}"))
                })?
                .map_err(BootError::BadRequest)?;
        Ok(json!({
            "workspaceRoot": workspace.display().to_string(),
            "kbRoot": kbutil::kb_dir(&workspace.display().to_string()).display().to_string(),
            "preview": import_preview_json(&preview),
        }))
    }

    pub(in crate::api::code_web) async fn import(
        &self,
        request: KbImportRequest,
    ) -> BootResult<Value> {
        let path = required_path_arg(&request.path)?;
        let workspace = self.workspace_from_request(request.workspace);
        self.ensure_workspace(&workspace).await?;
        let workspace_text = workspace.display().to_string();
        let now = chrono::Utc::now().to_rfc3339();
        let summary =
            tokio::task::spawn_blocking(move || kbutil::import_to_kb(&workspace_text, &path, &now))
                .await
                .map_err(|error| {
                    BootError::Internal(format!("failed to import into KB: {error}"))
                })?;
        Ok(kb_action_json(&workspace, summary, "import"))
    }

    pub(in crate::api::code_web) async fn search(
        &self,
        request: KbSearchRequest,
    ) -> BootResult<Value> {
        let query = request.query.trim().to_string();
        if query.is_empty() {
            return Err(BootError::BadRequest("query is required".to_string()));
        }
        let workspace = self.workspace_from_request(request.workspace);
        let workspace_text = workspace.display().to_string();
        let query_for_task = query.clone();
        let hits = tokio::task::spawn_blocking(move || {
            kbutil::search_kb(&workspace_text, &query_for_task)
        })
        .await
        .map_err(|error| BootError::Internal(format!("failed to search KB: {error}")))?;
        Ok(json!({
            "workspaceRoot": workspace.display().to_string(),
            "kbRoot": kbutil::kb_dir(&workspace.display().to_string()).display().to_string(),
            "query": query,
            "hits": hits.iter().map(search_hit_json).collect::<Vec<_>>(),
            "total": hits.len(),
        }))
    }

    pub(in crate::api::code_web) async fn marketplace(
        &self,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(workspace);
        let bases = run_blocking("list personal knowledge bases", {
            let workspace = workspace.clone();
            move || Ok(personal_bases::list_knowledge_bases(&workspace))
        })
        .await?;
        let installed = bases
            .items
            .iter()
            .filter_map(|base| base.marketplace_id.as_deref())
            .collect::<std::collections::HashSet<_>>();
        let items = super::marketplace::packages()
            .iter()
            .map(|package| {
                json!({
                    "id": package.id,
                    "name": package.name,
                    "description": package.description,
                    "publisher": package.publisher,
                    "version": package.version,
                    "category": package.category,
                    "tags": package.tags,
                    "featured": package.featured,
                    "updatedAt": package.updated_at,
                    "sourceCount": package.source_count(),
                    "conceptCount": package.concept_count(),
                    "installed": installed.contains(package.id),
                })
            })
            .collect::<Vec<_>>();
        Ok(json!({
            "schemaVersion": 1,
            "format": "okf",
            "workspaceRoot": workspace.display().to_string(),
            "source": {
                "id": "a3s-curated",
                "label": "A3S Curated",
                "kind": "builtin",
                "verified": true,
            },
            "items": items,
            "warnings": bases.warnings,
        }))
    }

    pub(in crate::api::code_web) async fn knowledge_bases(
        &self,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(workspace);
        let bases = run_blocking("list personal knowledge bases", {
            let workspace = workspace.clone();
            move || Ok(personal_bases::list_knowledge_bases(&workspace))
        })
        .await?;
        let total = bases.items.len();
        Ok(json!({
            "schemaVersion": 1,
            "workspaceRoot": workspace.display().to_string(),
            "root": personal_bases::bases_root(&workspace).display().to_string(),
            "items": bases.items,
            "total": total,
            "warnings": bases.warnings,
        }))
    }

    pub(in crate::api::code_web) async fn create_knowledge_base(
        &self,
        request: KnowledgeBaseCreateRequest,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(request.workspace);
        let name = request.name;
        let description = request.description;
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("create personal knowledge base", move || {
            personal_bases::create_knowledge_base(&workspace, &name, description.as_deref())
        })
        .await?;
        Ok(mutation_json(mutation))
    }

    pub(in crate::api::code_web) async fn import_knowledge_base(
        &self,
        request: KnowledgeBaseImportRequest,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(request.workspace);
        let source = PathBuf::from(required_path_arg(&request.path)?);
        let name = request.name;
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("import personal knowledge base", move || {
            personal_bases::import_knowledge_base(&workspace, &source, name.as_deref())
        })
        .await?;
        Ok(mutation_json(mutation))
    }

    pub(in crate::api::code_web) async fn preview_knowledge_base_from_selection(
        &self,
        request: KnowledgeBaseSelectionRequest,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(request.workspace);
        let paths = request
            .paths
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let preview = run_blocking("preview knowledge source selection", move || {
            source_packages::preview_source_selection(&workspace, &paths)
        })
        .await?;
        serialize_value(preview)
    }

    pub(in crate::api::code_web) async fn create_knowledge_base_from_selection(
        &self,
        request: KnowledgeBaseFromSelectionRequest,
    ) -> BootResult<Value> {
        let workspace = self.workspace_from_request(request.workspace);
        let paths = request
            .paths
            .into_iter()
            .map(PathBuf::from)
            .collect::<Vec<_>>();
        let name = request.name;
        let description = request.description;
        let policy = request.compilation_policy;
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("create knowledge base from selected sources", move || {
            source_packages::create_from_selection(
                &workspace,
                &paths,
                &name,
                description.as_deref(),
                policy,
            )
        })
        .await?;
        Ok(mutation_json(mutation))
    }

    pub(in crate::api::code_web) async fn install_marketplace_item(
        &self,
        id: &str,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let package = super::marketplace::package(id).ok_or_else(|| {
            BootError::NotFound(format!("knowledge marketplace item `{id}` was not found"))
        })?;
        let workspace = self.workspace_from_request(workspace);
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("install knowledge marketplace item", move || {
            personal_bases::install_market_package(&workspace, package)
        })
        .await?;
        Ok(mutation_json(mutation))
    }

    pub(in crate::api::code_web) async fn set_knowledge_base_pinned(
        &self,
        id: &str,
        request: KnowledgeBasePinRequest,
    ) -> BootResult<Value> {
        let id = id.to_string();
        let workspace = self.workspace_from_request(request.workspace);
        let pinned = request.pinned;
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("update personal knowledge base", move || {
            personal_bases::set_pinned(&workspace, &id, pinned)
        })
        .await?;
        Ok(mutation_json(mutation))
    }

    pub(in crate::api::code_web) async fn request_knowledge_compilation(
        &self,
        id: &str,
        request: KnowledgeCompilationRequest,
    ) -> BootResult<Value> {
        let id = id.to_string();
        let workspace = self.workspace_from_request(request.workspace);
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("request knowledge compilation", move || {
            compilation::request_compilation(&workspace, &id, chrono::Utc::now())
        })
        .await?;
        serialize_value(mutation)
    }

    pub(in crate::api::code_web) async fn knowledge_compilation_status(
        &self,
        id: &str,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let id = id.to_string();
        let workspace = self.workspace_from_request(workspace);
        let status = run_blocking("read knowledge compilation status", move || {
            compilation::status(&workspace, &id)
        })
        .await?;
        serialize_value(status)
    }

    pub(in crate::api::code_web) async fn set_knowledge_compilation_policy(
        &self,
        id: &str,
        request: KnowledgeCompilationPolicyRequest,
    ) -> BootResult<Value> {
        let id = id.to_string();
        let workspace = self.workspace_from_request(request.workspace);
        let policy = request.policy;
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("update knowledge compilation policy", move || {
            compilation::set_policy(&workspace, &id, policy, chrono::Utc::now())
        })
        .await?;
        serialize_value(mutation)
    }

    pub(in crate::api::code_web) async fn knowledge_source_changes(
        &self,
        id: &str,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let id = id.to_string();
        let workspace = self.workspace_from_request(workspace);
        let _guard = self.operation_lock.lock().await;
        let changes = run_blocking("inspect knowledge source changes", move || {
            compilation::inspect_source_changes(&workspace, &id, chrono::Utc::now())
        })
        .await?;
        serialize_value(changes)
    }

    pub(in crate::api::code_web) async fn cancel_knowledge_compilation(
        &self,
        id: &str,
        job_id: &str,
        request: KnowledgeCompilationRequest,
    ) -> BootResult<Value> {
        let id = id.to_string();
        let job_id = job_id.to_string();
        let workspace = self.workspace_from_request(request.workspace);
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("cancel knowledge compilation", move || {
            compilation::cancel_job(&workspace, &id, &job_id, chrono::Utc::now())
        })
        .await?;
        serialize_value(mutation)
    }

    pub(in crate::api::code_web) async fn claim_knowledge_compilation(
        &self,
        request: KnowledgeCompilationRequest,
    ) -> BootResult<Value> {
        self.workspace_from_request(request.workspace);
        let workspaces = self
            .monitored_workspaces
            .read()
            .map(|workspaces| workspaces.iter().cloned().collect::<Vec<_>>())
            .unwrap_or_else(|_| vec![self.state.default_workspace.clone()]);
        let _guard = self.operation_lock.lock().await;
        let claim = run_blocking("claim knowledge compilation", move || {
            compilation::claim_next_in_workspaces(&workspaces, chrono::Utc::now())
        })
        .await?;
        serialize_value(claim)
    }

    pub(in crate::api::code_web) async fn complete_knowledge_compilation(
        &self,
        id: &str,
        job_id: &str,
        request: KnowledgeCompilationResultRequest,
    ) -> BootResult<Value> {
        let outcome = request.outcome.ok_or_else(|| {
            BootError::BadRequest("knowledge compilation outcome is required".to_string())
        })?;
        let id = id.to_string();
        let job_id = job_id.to_string();
        let workspace = self.workspace_from_request(request.workspace);
        let transient = request.transient.unwrap_or(false);
        let error = request.error;
        let compiler_version = request.compiler_version;
        let _guard = self.operation_lock.lock().await;
        let mutation = run_blocking("complete knowledge compilation", move || {
            compilation::complete_job(
                &workspace,
                &id,
                &job_id,
                outcome,
                transient,
                error.as_deref(),
                compiler_version.as_deref(),
                chrono::Utc::now(),
            )
        })
        .await?;
        serialize_value(mutation)
    }

    async fn ensure_workspace(&self, workspace: &Path) -> BootResult<()> {
        let kb_root = kbutil::kb_dir(&workspace.display().to_string());
        tokio::fs::create_dir_all(kb_root.join("sources"))
            .await
            .map_err(fs_error)?;
        tokio::fs::create_dir_all(kb_root.join("wiki"))
            .await
            .map_err(fs_error)?;
        Ok(())
    }

    fn workspace_from_request(&self, workspace: Option<String>) -> PathBuf {
        let workspace = workspace
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.state.default_workspace.clone());
        if let Ok(mut workspaces) = self.monitored_workspaces.write() {
            workspaces.insert(workspace.clone());
        }
        workspace
    }
}

fn kb_home_json(workspace: &Path) -> Value {
    let workspace_text = workspace.display().to_string();
    let stats = kbutil::kb_stats(&workspace_text);
    json!({
        "workspaceRoot": workspace_text,
        "kbRoot": kbutil::kb_dir(&workspace.display().to_string()).display().to_string(),
        "stats": kb_stats_json(&stats),
        "recent": kbutil::recent_sources(&workspace.display().to_string(), 8),
    })
}

fn kb_action_json(workspace: &Path, summary: String, action: &str) -> Value {
    let home = kb_home_json(workspace);
    json!({
        "action": action,
        "success": !summary.trim_start().starts_with('\u{2717}'),
        "summary": summary,
        "workspaceRoot": home["workspaceRoot"].clone(),
        "kbRoot": home["kbRoot"].clone(),
        "stats": home["stats"].clone(),
        "recent": home["recent"].clone(),
    })
}

fn kb_stats_json(stats: &KbStats) -> Value {
    json!({
        "sources": stats.sources,
        "concepts": stats.concepts,
        "imports": stats.imports,
        "bytes": stats.bytes,
    })
}

fn import_preview_json(preview: &ImportPreview) -> Value {
    json!({
        "arg": preview.arg,
        "path": preview.path.display().to_string(),
        "kind": match preview.kind {
            ImportKind::File => "file",
            ImportKind::Folder => "folder",
        },
        "addable": preview.addable,
        "skipped": preview.skipped,
        "capped": preview.capped,
        "bytes": preview.bytes,
    })
}

fn search_hit_json(hit: &SearchHit) -> Value {
    json!({
        "path": hit.path,
        "line": hit.line,
        "snippet": hit.snippet,
    })
}

fn required_path_arg(path: &str) -> BootResult<String> {
    let path = path.trim();
    if path.is_empty() {
        return Err(BootError::BadRequest("path is required".to_string()));
    }
    Ok(path.to_string())
}

fn fs_error(error: std::io::Error) -> BootError {
    BootError::Internal(error.to_string())
}

fn mutation_json(mutation: KnowledgeBaseMutation) -> Value {
    json!({
        "changed": mutation.changed,
        "knowledgeBase": mutation.knowledge_base,
    })
}

fn serialize_value<T: Serialize>(value: T) -> BootResult<Value> {
    serde_json::to_value(value).map_err(|error| {
        BootError::Internal(format!("failed to encode knowledge response: {error}"))
    })
}

async fn run_blocking<T, F>(operation: &str, task: F) -> BootResult<T>
where
    T: Send + 'static,
    F: FnOnce() -> Result<T, KnowledgeStoreError> + Send + 'static,
{
    tokio::task::spawn_blocking(task)
        .await
        .map_err(|error| BootError::Internal(format!("failed to {operation}: {error}")))?
        .map_err(store_error)
}

fn store_error(error: KnowledgeStoreError) -> BootError {
    match error {
        KnowledgeStoreError::Invalid(message) => BootError::BadRequest(message),
        KnowledgeStoreError::NotFound(message) => BootError::NotFound(message),
        KnowledgeStoreError::Conflict(message) => BootError::Conflict(message),
        KnowledgeStoreError::Io(message) => BootError::Internal(message),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kb_stats_json_matches_tui_kb_stats_fields() {
        let stats = KbStats {
            sources: 2,
            concepts: 3,
            imports: 1,
            bytes: 128,
        };
        assert_eq!(
            kb_stats_json(&stats),
            json!({
                "sources": 2,
                "concepts": 3,
                "imports": 1,
                "bytes": 128,
            })
        );
    }

    #[test]
    fn import_preview_json_keeps_tui_preview_contract() {
        let preview = ImportPreview {
            arg: "notes".to_string(),
            path: PathBuf::from("/tmp/notes"),
            kind: ImportKind::Folder,
            addable: 4,
            skipped: 1,
            capped: true,
            bytes: 4096,
        };
        let json = import_preview_json(&preview);
        assert_eq!(json["kind"], "folder");
        assert_eq!(json["addable"], 4);
        assert_eq!(json["skipped"], 1);
        assert_eq!(json["capped"], true);
        assert_eq!(json["bytes"], 4096);
    }
}
