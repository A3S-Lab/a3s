use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio::fs;
use tokio::process::Command;

use crate::api::code_web::state::CodeWebState;

pub(in crate::api::code_web) struct WorkspaceSearchOptions {
    pub(in crate::api::code_web) case_sensitive: bool,
    pub(in crate::api::code_web) use_regex: bool,
    pub(in crate::api::code_web) match_whole_word: bool,
    pub(in crate::api::code_web) include_pattern: Option<String>,
    pub(in crate::api::code_web) exclude_pattern: Option<String>,
    pub(in crate::api::code_web) max_results: usize,
}

pub(in crate::api::code_web) struct WorkspaceService {
    state: Arc<CodeWebState>,
}

impl WorkspaceService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) fn default_root(&self) -> serde_json::Value {
        json!({
            "root": self.state.default_workspace.display().to_string(),
        })
    }

    pub(in crate::api::code_web) async fn inspect_readiness(
        &self,
        workspace_root: Option<String>,
        repair: bool,
    ) -> BootResult<serde_json::Value> {
        let root = workspace_root
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(expand_home)
            .unwrap_or_else(|| self.state.default_workspace.clone());
        let agents_dir = root.join("agents");
        let sessions_dir = root.join("sessions");

        if repair {
            fs::create_dir_all(&agents_dir).await.map_err(fs_error)?;
            fs::create_dir_all(&sessions_dir).await.map_err(fs_error)?;
        }

        let root_exists = path_exists(&root).await;
        let agents_exists = path_exists(&agents_dir).await;
        let sessions_exists = path_exists(&sessions_dir).await;

        Ok(json!({
            "workspaceRoot": root.display().to_string(),
            "rootExists": root_exists,
            "agentsExists": agents_exists,
            "sessionsExists": sessions_exists,
            "needsRepair": !(root_exists && agents_exists && sessions_exists),
            "platform": std::env::consts::OS,
            "isWindows": cfg!(windows),
        }))
    }

    pub(in crate::api::code_web) async fn init_agent(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let workspace = required_json_path(&request, "workspacePath")?;
        fs::create_dir_all(workspace.join(".a3s"))
            .await
            .map_err(fs_error)?;
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn init_prompt(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let workspace = optional_json_path(&request, "workspace")
            .or_else(|| optional_json_path(&request, "workspacePath"))
            .transpose()?
            .unwrap_or_else(|| self.state.default_workspace.clone());
        let agents_path = workspace.join("AGENTS.md");
        Ok(json!({
            "workspace": workspace.display().to_string(),
            "path": agents_path.display().to_string(),
            "exists": agents_path.is_file(),
            "display": "/init - generate AGENTS.md",
            "prompt": init_agents_prompt(&workspace),
        }))
    }

    pub(in crate::api::code_web) async fn pick_directory(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        super::picker::pick_directory(&self.state.default_workspace, request).await
    }

    pub(in crate::api::code_web) async fn reveal_path(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let requested = required_json_path(&request, "path")?;
        let path = fs::canonicalize(&requested).await.map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                BootError::NotFound(format!("path was not found: {}", requested.display()))
            } else {
                fs_error(error)
            }
        })?;
        let metadata = fs::metadata(&path).await.map_err(fs_error)?;
        reveal_in_file_manager(&path, metadata.is_dir()).await?;
        Ok(json!({
            "revealed": true,
            "path": path.display().to_string(),
        }))
    }

    pub(in crate::api::code_web) async fn create_dir(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let path = required_json_path(&request, "path")?;
        fs::create_dir_all(path).await.map_err(fs_error)?;
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn create_file(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let path = required_json_path(&request, "path")?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(fs_error)?;
        }
        let _file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
            .await
            .map_err(|error| {
                if error.kind() == std::io::ErrorKind::AlreadyExists {
                    BootError::Conflict(format!("path already exists: {}", path.display()))
                } else {
                    fs_error(error)
                }
            })?;
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn write_file(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let path = required_json_path(&request, "path")?;
        let content = request
            .get("content")
            .and_then(Value::as_str)
            .ok_or_else(|| {
                BootError::BadRequest("content is required and must be a string".to_string())
            })?
            .to_string();
        let expected_revision = optional_json_string(&request, "expectedRevision")?;
        let expected_content = optional_json_string(&request, "expectedContent")?;
        if expected_revision.is_some() && expected_content.is_some() {
            return Err(BootError::BadRequest(
                "expectedRevision and expectedContent cannot be used together".to_string(),
            ));
        }
        if expected_revision.as_deref().is_some_and(str::is_empty) {
            return Err(BootError::BadRequest(
                "expectedRevision cannot be empty".to_string(),
            ));
        }

        // Serialize API writes so two browser clients cannot both satisfy the
        // same precondition inside this process. External programs can still
        // modify the file, so the check stays immediately adjacent to write.
        let _write_guard = self.state.workspace_file_write_lock.lock().await;
        if expected_revision.is_some() || expected_content.is_some() {
            let current = fs::read_to_string(&path).await.map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    BootError::PreconditionFailed(format!(
                        "file changed or was removed before save: {}",
                        path.display()
                    ))
                } else {
                    fs_error(error)
                }
            })?;
            let revision_matches = expected_revision
                .as_deref()
                .is_none_or(|expected| expected == content_revision(current.as_bytes()));
            let content_matches = expected_content
                .as_deref()
                .is_none_or(|expected| expected == current);
            if !revision_matches || !content_matches {
                return Err(BootError::PreconditionFailed(format!(
                    "file changed before save: {}",
                    path.display()
                )));
            }
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(fs_error)?;
        }
        fs::write(&path, &content).await.map_err(fs_error)?;
        Ok(json!({
            "success": true,
            "revision": content_revision(content.as_bytes()),
        }))
    }

    pub(in crate::api::code_web) async fn write_binary_file(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let path = required_json_path(&request, "path")?;
        let bytes = request
            .get("data")
            .and_then(Value::as_array)
            .ok_or_else(|| BootError::BadRequest("data is required".to_string()))?
            .iter()
            .map(|value| {
                value
                    .as_u64()
                    .filter(|byte| *byte <= u8::MAX as u64)
                    .map(|byte| byte as u8)
                    .ok_or_else(|| {
                        BootError::BadRequest("data must contain byte values".to_string())
                    })
            })
            .collect::<BootResult<Vec<u8>>>()?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).await.map_err(fs_error)?;
        }
        fs::write(path, bytes).await.map_err(fs_error)?;
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn read_file(
        &self,
        path: String,
    ) -> BootResult<serde_json::Value> {
        let path = required_path(path)?;
        let content = fs::read_to_string(path).await.map_err(fs_error)?;
        Ok(json!({
            "revision": content_revision(content.as_bytes()),
            "content": content,
        }))
    }

    pub(in crate::api::code_web) async fn read_binary_file(
        &self,
        path: String,
    ) -> BootResult<Vec<u8>> {
        let path = required_path(path)?;
        fs::read(path).await.map_err(fs_error)
    }

    pub(in crate::api::code_web) async fn path_exists(
        &self,
        path: String,
    ) -> BootResult<serde_json::Value> {
        let path = required_path(path)?;
        Ok(json!({ "exists": path_exists(&path).await }))
    }

    pub(in crate::api::code_web) async fn delete_path(
        &self,
        path: String,
    ) -> BootResult<serde_json::Value> {
        let path = required_path(path)?;
        let metadata = fs::metadata(&path).await.map_err(fs_error)?;
        if metadata.is_dir() {
            fs::remove_dir_all(path).await.map_err(fs_error)?;
        } else {
            fs::remove_file(path).await.map_err(fs_error)?;
        }
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn read_dir(
        &self,
        path: String,
    ) -> BootResult<Vec<serde_json::Value>> {
        let path = required_path(path)?;
        let mut entries = fs::read_dir(path).await.map_err(fs_error)?;
        let mut items = Vec::new();
        while let Some(entry) = entries.next_entry().await.map_err(fs_error)? {
            let metadata = entry.metadata().await.map_err(fs_error)?;
            let name = entry.file_name().to_string_lossy().to_string();
            let modified_at = metadata.modified().ok().and_then(|time| {
                time.duration_since(UNIX_EPOCH)
                    .ok()
                    .map(|duration| duration.as_millis() as u64)
            });
            items.push(json!({
                "name": name,
                "isDirectory": metadata.is_dir(),
                "isFile": metadata.is_file(),
                "size": metadata.len(),
                "mtimeMs": modified_at,
                "extension": entry.path().extension().and_then(|value| value.to_str()),
                "isBinary": false,
            }));
        }
        items.sort_by(|left, right| {
            let left_dir = left
                .get("isDirectory")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            let right_dir = right
                .get("isDirectory")
                .and_then(Value::as_bool)
                .unwrap_or(false);
            right_dir
                .cmp(&left_dir)
                .then_with(|| value_name(left).cmp(value_name(right)))
        });
        Ok(items)
    }

    pub(in crate::api::code_web) async fn watch(
        &self,
        root_path: String,
    ) -> BootResult<a3s_boot::BootResponse> {
        super::watch::watch_workspace(root_path).await
    }

    pub(in crate::api::code_web) async fn rename_path(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let src = required_json_path(&request, "src")?;
        let dest = required_json_path(&request, "dest")?;
        if let Some(parent) = dest.parent() {
            fs::create_dir_all(parent).await.map_err(fs_error)?;
        }
        fs::rename(src, dest).await.map_err(fs_error)?;
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn copy_path(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let src = required_json_path(&request, "src")?;
        let dest = required_json_path(&request, "dest")?;
        copy_path(&src, &dest).await?;
        Ok(json!({ "success": true }))
    }

    pub(in crate::api::code_web) async fn workspace_files(
        &self,
        root_path: String,
        query: String,
        max_results: usize,
    ) -> BootResult<serde_json::Value> {
        super::catalog::workspace_files(&self.state, root_path, query, max_results).await
    }

    pub(in crate::api::code_web) async fn git_status(
        &self,
        root_path: Option<String>,
    ) -> BootResult<serde_json::Value> {
        super::git::status(&self.state, root_path).await
    }

    pub(in crate::api::code_web) async fn git_diff(
        &self,
        root_path: String,
        path: Option<String>,
        staged: bool,
    ) -> BootResult<serde_json::Value> {
        super::git::diff(&self.state, root_path, path, staged).await
    }

    pub(in crate::api::code_web) async fn git_stage(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        super::git::stage(&self.state, request).await
    }

    pub(in crate::api::code_web) async fn git_unstage(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        super::git::unstage(&self.state, request).await
    }

    pub(in crate::api::code_web) async fn git_commit(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        super::git::commit(&self.state, request).await
    }

    pub(in crate::api::code_web) async fn search_files(
        &self,
        root_path: String,
        query: String,
        options: WorkspaceSearchOptions,
    ) -> BootResult<Vec<serde_json::Value>> {
        if options.use_regex {
            return Err(BootError::BadRequest(
                "regex search is not supported by the local web API yet".to_string(),
            ));
        }
        let query = query.trim().to_string();
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let root = required_path(root_path)?;
        let files = collect_text_candidate_files(
            &root,
            options.include_pattern.as_deref(),
            options.exclude_pattern.as_deref(),
        )
        .await?;
        let mut results = Vec::new();
        let mut total_matches = 0usize;

        for file in files {
            if total_matches >= options.max_results.max(1) {
                break;
            }
            let Ok(content) = fs::read_to_string(&file).await else {
                continue;
            };
            let mut matches = Vec::new();
            for (line_index, line) in content.lines().enumerate() {
                for (start, end) in find_line_matches(
                    line,
                    &query,
                    options.case_sensitive,
                    options.match_whole_word,
                ) {
                    matches.push(json!({
                        "line": line_index + 1,
                        "column": start + 1,
                        "text": line,
                        "matchStart": start,
                        "matchEnd": end,
                    }));
                    total_matches += 1;
                    if total_matches >= options.max_results.max(1) {
                        break;
                    }
                }
                if total_matches >= options.max_results.max(1) {
                    break;
                }
            }
            if !matches.is_empty() {
                results.push(json!({
                    "path": file.display().to_string(),
                    "matches": matches,
                }));
            }
        }

        Ok(results)
    }

    pub(in crate::api::code_web) async fn replace_in_files(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let root = required_json_path(&request, "rootPath")?;
        let query = request
            .get("query")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .ok_or_else(|| BootError::BadRequest("query is required".to_string()))?;
        let replacement = request
            .get("replacement")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let use_regex = request
            .get("useRegex")
            .and_then(Value::as_bool)
            .unwrap_or(false);
        if use_regex {
            return Err(BootError::BadRequest(
                "regex replace is not supported by the local web API yet".to_string(),
            ));
        }
        let options = WorkspaceSearchOptions {
            case_sensitive: request
                .get("caseSensitive")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            use_regex,
            match_whole_word: request
                .get("matchWholeWord")
                .and_then(Value::as_bool)
                .unwrap_or(false),
            include_pattern: request
                .get("includePattern")
                .and_then(Value::as_str)
                .map(str::to_string),
            exclude_pattern: request
                .get("excludePattern")
                .and_then(Value::as_str)
                .map(str::to_string),
            max_results: usize::MAX,
        };
        let files = if let Some(file_paths) = request.get("filePaths").and_then(Value::as_array) {
            file_paths
                .iter()
                .filter_map(Value::as_str)
                .map(|path| required_path(path.to_string()))
                .collect::<BootResult<Vec<_>>>()?
        } else {
            collect_text_candidate_files(
                &root,
                options.include_pattern.as_deref(),
                options.exclude_pattern.as_deref(),
            )
            .await?
        };
        let mut modified_files = Vec::new();
        let mut total_replacements = 0usize;

        for file in files {
            let Ok(content) = fs::read_to_string(&file).await else {
                continue;
            };
            let (next_content, replacements) = replace_text(
                &content,
                query,
                replacement,
                options.case_sensitive,
                options.match_whole_word,
            );
            if replacements == 0 {
                continue;
            }
            fs::write(&file, next_content).await.map_err(fs_error)?;
            total_replacements += replacements;
            modified_files.push(json!({
                "path": file.display().to_string(),
                "replacements": replacements,
            }));
        }

        Ok(json!({
            "filesModified": modified_files.len(),
            "totalReplacements": total_replacements,
            "files": modified_files,
        }))
    }
}

async fn path_exists(path: &Path) -> bool {
    fs::metadata(path).await.is_ok()
}

async fn reveal_in_file_manager(path: &Path, is_directory: bool) -> BootResult<()> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = Command::new("open");
        if is_directory {
            command.arg(path);
        } else {
            command.arg("-R").arg(path);
        }
        command
    };

    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = Command::new("explorer.exe");
        if is_directory {
            command.arg(path);
        } else {
            command.arg("/select,").arg(path);
        }
        command
    };

    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let target = if is_directory {
            path
        } else {
            path.parent().unwrap_or(path)
        };
        let mut command = Command::new("xdg-open");
        command.arg(target);
        command
    };

    #[cfg(not(any(unix, target_os = "windows")))]
    return Err(BootError::ServiceUnavailable(
        "revealing paths is not supported on this platform".to_string(),
    ));

    #[cfg(any(unix, target_os = "windows"))]
    {
        let status = command.status().await.map_err(|error| {
            BootError::Internal(format!("failed to open the system file manager: {error}"))
        })?;
        if !status.success() {
            return Err(BootError::Internal(format!(
                "the system file manager exited with status {status}"
            )));
        }
        Ok(())
    }
}

fn required_json_path(value: &Value, field: &str) -> BootResult<PathBuf> {
    let raw = value
        .get(field)
        .and_then(Value::as_str)
        .ok_or_else(|| BootError::BadRequest(format!("{field} is required")))?;
    required_path(raw.to_string())
}

fn optional_json_path(value: &Value, field: &str) -> Option<BootResult<PathBuf>> {
    value
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| required_path(value.to_string()))
}

fn optional_json_string(value: &Value, field: &str) -> BootResult<Option<String>> {
    match value.get(field) {
        None => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(BootError::BadRequest(format!("{field} must be a string"))),
    }
}

fn content_revision(content: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(content))
}

fn init_agents_prompt(workspace: &Path) -> String {
    let agents_path = workspace.join("AGENTS.md");
    format!(
        "Analyze this codebase at `{workspace}` and create (or update) an AGENTS.md file at `{agents_path}`. \
         Include: a concise project overview, the exact build / test / lint / run commands, \
         the high-level architecture and key directories, and the conventions an AI coding agent should follow. \
         Base everything on what's actually in the workspace, and write the file with your file-writing tool.",
        workspace = workspace.display(),
        agents_path = agents_path.display(),
    )
}

pub(super) fn required_path(value: String) -> BootResult<PathBuf> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(BootError::BadRequest("path is required".to_string()));
    }
    Ok(expand_home(trimmed))
}

fn expand_home(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = crate::user_paths::user_home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn fs_error(error: std::io::Error) -> BootError {
    match error.kind() {
        std::io::ErrorKind::NotFound => BootError::NotFound(error.to_string()),
        std::io::ErrorKind::PermissionDenied => BootError::Forbidden(error.to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            BootError::BadRequest(error.to_string())
        }
        _ => BootError::Io(error),
    }
}

fn value_name(value: &Value) -> &str {
    value.get("name").and_then(Value::as_str).unwrap_or("")
}

async fn copy_path(src: &Path, dest: &Path) -> BootResult<()> {
    let src = src.to_path_buf();
    let dest = dest.to_path_buf();
    tokio::task::spawn_blocking(move || copy_path_sync(&src, &dest))
        .await
        .map_err(|error| BootError::Internal(error.to_string()))?
        .map_err(fs_error)
}

fn copy_path_sync(src: &Path, dest: &Path) -> std::io::Result<()> {
    let metadata = std::fs::metadata(src)?;
    if metadata.is_dir() {
        std::fs::create_dir_all(dest)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            copy_path_sync(&entry.path(), &dest.join(entry.file_name()))?;
        }
    } else {
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::copy(src, dest)?;
    }
    Ok(())
}

async fn collect_text_candidate_files(
    root: &Path,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
) -> BootResult<Vec<PathBuf>> {
    let root = root.to_path_buf();
    let include_pattern = include_pattern.map(str::to_string);
    let exclude_pattern = exclude_pattern.map(str::to_string);
    tokio::task::spawn_blocking(move || {
        let mut files = Vec::new();
        collect_files_sync(
            &root,
            &root,
            include_pattern.as_deref(),
            exclude_pattern.as_deref(),
            &mut files,
        )?;
        Ok(files)
    })
    .await
    .map_err(|error| BootError::Internal(error.to_string()))?
}

fn collect_files_sync(
    root: &Path,
    current: &Path,
    include_pattern: Option<&str>,
    exclude_pattern: Option<&str>,
    files: &mut Vec<PathBuf>,
) -> BootResult<()> {
    for entry in std::fs::read_dir(current).map_err(fs_error)? {
        let entry = entry.map_err(fs_error)?;
        let path = entry.path();
        let metadata = entry.metadata().map_err(fs_error)?;
        if metadata.is_dir() {
            collect_files_sync(root, &path, include_pattern, exclude_pattern, files)?;
        } else if metadata.is_file() {
            let relative = path
                .strip_prefix(root)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            if path_is_included(&relative, include_pattern)
                && !path_is_excluded(&relative, exclude_pattern)
                && looks_like_text_path(&path)
            {
                files.push(path);
            }
        }
    }
    Ok(())
}

fn looks_like_text_path(path: &Path) -> bool {
    match path.extension().and_then(|value| value.to_str()) {
        None => true,
        Some(ext) => matches!(
            ext.to_ascii_lowercase().as_str(),
            "acl"
                | "bash"
                | "c"
                | "css"
                | "csv"
                | "env"
                | "h"
                | "hcl"
                | "html"
                | "js"
                | "json"
                | "jsx"
                | "md"
                | "mdx"
                | "py"
                | "rs"
                | "sh"
                | "toml"
                | "ts"
                | "tsx"
                | "txt"
                | "xml"
                | "yaml"
                | "yml"
        ),
    }
}

fn path_is_included(path: &str, pattern: Option<&str>) -> bool {
    let Some(pattern) = pattern.map(str::trim).filter(|value| !value.is_empty()) else {
        return true;
    };
    path_matches_patterns(path, pattern)
}

fn path_is_excluded(path: &str, pattern: Option<&str>) -> bool {
    let Some(pattern) = pattern.map(str::trim).filter(|value| !value.is_empty()) else {
        return false;
    };
    path_matches_patterns(path, pattern)
}

fn path_matches_patterns(path: &str, pattern: &str) -> bool {
    pattern
        .split(',')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .any(|part| wildcard_match(path, part) || path.contains(part))
}

fn wildcard_match(path: &str, pattern: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(suffix) = pattern.strip_prefix("*.") {
        return path
            .rsplit('/')
            .next()
            .is_some_and(|name| name.ends_with(&format!(".{suffix}")));
    }
    if !pattern.contains('*') {
        return path == pattern;
    }
    let parts: Vec<&str> = pattern.split('*').collect();
    let mut cursor = 0usize;
    for (index, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        let Some(found) = path[cursor..].find(part) else {
            return false;
        };
        if index == 0 && !pattern.starts_with('*') && found != 0 {
            return false;
        }
        cursor += found + part.len();
    }
    pattern.ends_with('*') || parts.last().is_none_or(|last| path.ends_with(last))
}

fn find_line_matches(
    line: &str,
    query: &str,
    case_sensitive: bool,
    match_whole_word: bool,
) -> Vec<(usize, usize)> {
    let haystack = if case_sensitive {
        line.to_string()
    } else {
        line.to_lowercase()
    };
    let needle = if case_sensitive {
        query.to_string()
    } else {
        query.to_lowercase()
    };
    if needle.is_empty() {
        return Vec::new();
    }
    let mut matches = Vec::new();
    let mut cursor = 0usize;
    while cursor <= haystack.len() {
        let Some(offset) = haystack[cursor..].find(&needle) else {
            break;
        };
        let start = cursor + offset;
        let end = start + needle.len();
        if !match_whole_word || is_whole_word(line, start, end) {
            matches.push((start, end));
        }
        cursor = end.max(start + 1);
    }
    matches
}

fn replace_text(
    content: &str,
    query: &str,
    replacement: &str,
    case_sensitive: bool,
    match_whole_word: bool,
) -> (String, usize) {
    let mut output = String::with_capacity(content.len());
    let mut replacements = 0usize;
    for (line_index, line) in content.split_inclusive('\n').enumerate() {
        let line_body = line.strip_suffix('\n').unwrap_or(line);
        let newline = if line.ends_with('\n') { "\n" } else { "" };
        let matches = find_line_matches(line_body, query, case_sensitive, match_whole_word);
        if matches.is_empty() {
            output.push_str(line_body);
            output.push_str(newline);
            continue;
        }
        let mut cursor = 0usize;
        for (start, end) in matches {
            output.push_str(&line_body[cursor..start]);
            output.push_str(replacement);
            cursor = end;
            replacements += 1;
        }
        output.push_str(&line_body[cursor..]);
        output.push_str(newline);
        if line_index == 0 && content.is_empty() {
            break;
        }
    }
    if content.is_empty() {
        (content.to_string(), 0)
    } else {
        (output, replacements)
    }
}

fn is_whole_word(line: &str, start: usize, end: usize) -> bool {
    let before = line[..start].chars().next_back();
    let after = line[end..].chars().next();
    !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
}

fn is_word_char(value: char) -> bool {
    value.is_ascii_alphanumeric() || value == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn init_agents_prompt_matches_tui_init_contract() {
        let workspace = PathBuf::from("/tmp/a3s-web-init");
        let prompt = init_agents_prompt(&workspace);

        assert!(prompt.contains("/tmp/a3s-web-init"));
        assert!(prompt.contains("AGENTS.md"));
        assert!(prompt.contains("build / test / lint / run commands"));
        assert!(prompt.contains("AI coding agent should follow"));
        assert!(prompt.contains("file-writing tool"));
    }
}
