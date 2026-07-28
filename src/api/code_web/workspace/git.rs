use std::collections::HashSet;
use std::path::{Component, Path, PathBuf};
use std::process::Output;
use std::sync::Arc;
use std::time::Duration;

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::{WorkspaceGit, WorkspaceServices};
use serde_json::{json, Value};
use similar::TextDiff;
use tokio::process::Command;
use tokio::time::timeout;

use crate::api::code_web::state::CodeWebState;

use super::service::required_path;

const GIT_COMMAND_TIMEOUT: Duration = Duration::from_secs(30);
const GIT_COMMIT_TIMEOUT: Duration = Duration::from_secs(120);

pub(super) async fn status(state: &CodeWebState, root_path: Option<String>) -> BootResult<Value> {
    let workspace = LocalGitWorkspace::open(state, workspace_root(state, root_path)?).await?;
    workspace.status_json().await
}

pub(super) async fn diff(
    state: &CodeWebState,
    root_path: String,
    path: Option<String>,
    staged: bool,
) -> BootResult<Value> {
    let workspace = LocalGitWorkspace::open(state, required_path(root_path)?).await?;
    workspace.require_repository()?;
    let Some(path) = path else {
        let output = workspace.unified_diff(None, staged, None).await?;
        let content = String::from_utf8_lossy(&output).into_owned();
        return Ok(json!({
            "path": Value::Null,
            "staged": staged,
            "content": content,
            "original": "",
            "modified": "",
            "isBinary": git_reports_binary(&content),
        }));
    };
    let path = validate_relative_path(&path)?;
    let states = workspace.file_states().await?;
    let state_for_path = states.iter().find(|file| file.path == path);
    let original_path = state_for_path.and_then(|file| {
        let renamed = if staged {
            matches!(file.index_status, 'R' | 'C')
        } else {
            matches!(file.worktree_status, 'R' | 'C')
        };
        renamed.then(|| file.original_path.clone()).flatten()
    });
    let unified = workspace
        .unified_diff(Some(&path), staged, original_path.as_deref())
        .await?;
    let unified = String::from_utf8_lossy(&unified).into_owned();

    let original_path = original_path.as_deref().unwrap_or(&path);
    let original = if staged {
        workspace
            .git_content(&workspace.git_spec(Some("HEAD"), original_path))
            .await?
    } else {
        workspace
            .git_content(&workspace.git_spec(None, original_path))
            .await?
    };
    let modified = if staged {
        workspace
            .git_content(&workspace.git_spec(None, &path))
            .await?
    } else {
        worktree_content(&workspace.root, &path).await?
    };
    let manifest_binary = state
        .workspace_manifest_snapshot_for(&workspace.root)
        .await
        .ok()
        .and_then(|snapshot| {
            snapshot
                .files
                .into_iter()
                .find(|file| file.path == path)
                .map(|file| file.binary)
        })
        .unwrap_or(false);
    let mut is_binary = manifest_binary
        || original.is_opaque()
        || modified.is_opaque()
        || git_reports_binary(&unified);
    let original = original.into_text(&mut is_binary);
    let modified = modified.into_text(&mut is_binary);
    let content = if unified.is_empty() && !is_binary && original != modified {
        unified_text(&path, &original, &modified)
    } else {
        unified
    };

    Ok(json!({
        "path": path,
        "staged": staged,
        "content": content,
        "original": if is_binary { "" } else { &original },
        "modified": if is_binary { "" } else { &modified },
        "isBinary": is_binary,
    }))
}

pub(super) async fn stage(state: &CodeWebState, request: Value) -> BootResult<Value> {
    let workspace = LocalGitWorkspace::from_request(state, &request).await?;
    workspace.require_repository()?;
    let paths = workspace.action_paths(required_paths(&request)?).await?;
    let mut args = vec!["add".to_string(), "-A".to_string(), "--".to_string()];
    args.extend(paths);
    ensure_git_success(
        "stage workspace files",
        git_output(&workspace.root, &args, GIT_COMMAND_TIMEOUT).await?,
    )?;
    workspace.status_json().await
}

pub(super) async fn unstage(state: &CodeWebState, request: Value) -> BootResult<Value> {
    let workspace = LocalGitWorkspace::from_request(state, &request).await?;
    workspace.require_repository()?;
    let paths = workspace.action_paths(required_paths(&request)?).await?;
    let head = git_output(
        &workspace.root,
        &[
            "rev-parse".into(),
            "--verify".into(),
            "--quiet".into(),
            "HEAD".into(),
        ],
        GIT_COMMAND_TIMEOUT,
    )
    .await?;
    let mut args = if head.status.success() {
        vec![
            "reset".to_string(),
            "--quiet".to_string(),
            "HEAD".to_string(),
            "--".to_string(),
        ]
    } else {
        vec![
            "rm".to_string(),
            "--cached".to_string(),
            "--force".to_string(),
            "-r".to_string(),
            "--ignore-unmatch".to_string(),
            "--".to_string(),
        ]
    };
    args.extend(paths);
    ensure_git_success(
        "unstage workspace files",
        git_output(&workspace.root, &args, GIT_COMMAND_TIMEOUT).await?,
    )?;
    workspace.status_json().await
}

pub(super) async fn commit(state: &CodeWebState, request: Value) -> BootResult<Value> {
    let workspace = LocalGitWorkspace::from_request(state, &request).await?;
    workspace.require_repository()?;
    let message = required_text(&request, "message")?;
    if message.contains('\0') {
        return Err(BootError::BadRequest(
            "commit message must not contain a NUL byte".to_string(),
        ));
    }
    let staged = git_output(
        &workspace.root,
        &[
            "diff".into(),
            "--cached".into(),
            "--quiet".into(),
            "--exit-code".into(),
        ],
        GIT_COMMAND_TIMEOUT,
    )
    .await?;
    match staged.status.code() {
        Some(0) => {
            return Err(BootError::Conflict(
                "there are no staged changes to commit".to_string(),
            ));
        }
        Some(1) => {}
        _ => return Err(git_failure("inspect staged changes", &staged)),
    }
    let output = ensure_git_success(
        "commit staged workspace files",
        git_output(
            &workspace.root,
            &["commit".into(), "--message".into(), message.clone()],
            GIT_COMMIT_TIMEOUT,
        )
        .await?,
    )?;
    let summary = command_message(&output);
    Ok(json!({
        "committed": true,
        "summary": if summary.is_empty() { "Commit created" } else { &summary },
        "status": workspace.status_json().await?,
    }))
}

struct LocalGitWorkspace {
    root: PathBuf,
    repository_prefix: String,
    git: Arc<dyn WorkspaceGit>,
    is_repository: bool,
}

impl LocalGitWorkspace {
    async fn open(state: &CodeWebState, root: PathBuf) -> BootResult<Self> {
        let services = state
            .workspace_services_for(&root)
            .await
            .map_err(|error| BootError::BadRequest(error.to_string()))?;
        let root = local_root(&services)?;
        let git = services.git().ok_or_else(|| {
            BootError::ServiceUnavailable("workspace Git capability is unavailable".to_string())
        })?;
        let is_repository = git.is_repository().await.map_err(|error| {
            BootError::Internal(format!("failed to inspect Git repository: {error}"))
        })?;
        let repository_prefix = if is_repository {
            git_repository_prefix(&root).await?
        } else {
            String::new()
        };
        Ok(Self {
            root,
            repository_prefix,
            git,
            is_repository,
        })
    }

    async fn from_request(state: &CodeWebState, request: &Value) -> BootResult<Self> {
        let root = request
            .get("rootPath")
            .and_then(Value::as_str)
            .ok_or_else(|| BootError::BadRequest("rootPath is required".to_string()))?;
        Self::open(state, required_path(root.to_string())?).await
    }

    fn require_repository(&self) -> BootResult<()> {
        if self.is_repository {
            Ok(())
        } else {
            Err(BootError::BadRequest(
                "workspace is not a Git repository".to_string(),
            ))
        }
    }

    async fn status_json(&self) -> BootResult<Value> {
        if !self.is_repository {
            return Ok(json!({
                "isGitRepo": false,
                "branch": Value::Null,
                "files": [],
            }));
        }
        let aggregate =
            self.git.status().await.map_err(|error| {
                BootError::Internal(format!("failed to read Git status: {error}"))
            })?;
        let branch = if aggregate.branch.is_empty() || aggregate.branch == "(detached)" {
            self.symbolic_branch().await.unwrap_or(aggregate.branch)
        } else {
            aggregate.branch
        };
        let files = self
            .file_states()
            .await?
            .into_iter()
            .map(GitFileState::into_json)
            .collect::<Vec<_>>();
        Ok(json!({
            "isGitRepo": true,
            "branch": branch,
            "files": files,
        }))
    }

    async fn symbolic_branch(&self) -> Option<String> {
        let output = git_output(
            &self.root,
            &[
                "symbolic-ref".into(),
                "--quiet".into(),
                "--short".into(),
                "HEAD".into(),
            ],
            GIT_COMMAND_TIMEOUT,
        )
        .await
        .ok()?;
        output
            .status
            .success()
            .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
            .filter(|branch| !branch.is_empty())
    }

    async fn file_states(&self) -> BootResult<Vec<GitFileState>> {
        let output = ensure_git_success(
            "read workspace file status",
            git_output(
                &self.root,
                &[
                    "status".into(),
                    "--porcelain=v1".into(),
                    "-z".into(),
                    "--untracked-files=all".into(),
                    "--ignore-submodules=none".into(),
                    "--renames".into(),
                    "--".into(),
                    ".".into(),
                ],
                GIT_COMMAND_TIMEOUT,
            )
            .await?,
        )?;
        Ok(parse_porcelain_status(
            &output.stdout,
            &self.repository_prefix,
        ))
    }

    async fn action_paths(&self, requested: Vec<String>) -> BootResult<Vec<String>> {
        let states = self.file_states().await?;
        let mut seen = HashSet::new();
        let mut paths = Vec::new();
        for path in requested {
            let path = validate_relative_path(&path)?;
            if seen.insert(path.clone()) {
                paths.push(path.clone());
            }
            if let Some(original) = states
                .iter()
                .find(|file| file.path == path)
                .and_then(|file| file.original_path.as_deref())
            {
                let original = validate_relative_path(original)?;
                if seen.insert(original.clone()) {
                    paths.push(original);
                }
            }
        }
        Ok(paths)
    }

    async fn unified_diff(
        &self,
        path: Option<&str>,
        staged: bool,
        original_path: Option<&str>,
    ) -> BootResult<Vec<u8>> {
        let mut args = vec![
            "diff".to_string(),
            "--no-color".to_string(),
            "--no-ext-diff".to_string(),
            "--no-textconv".to_string(),
        ];
        if staged {
            args.push("--cached".to_string());
        }
        if let Some(path) = path {
            args.push("--".to_string());
            if let Some(original) = original_path.filter(|original| *original != path) {
                args.push(original.to_string());
            }
            args.push(path.to_string());
        }
        Ok(ensure_git_success(
            "read workspace diff",
            git_output(&self.root, &args, GIT_COMMAND_TIMEOUT).await?,
        )?
        .stdout)
    }

    async fn git_content(&self, spec: &str) -> BootResult<FileContent> {
        let kind = git_output(
            &self.root,
            &["cat-file".into(), "-t".into(), spec.to_string()],
            GIT_COMMAND_TIMEOUT,
        )
        .await?;
        if !kind.status.success() {
            return Ok(FileContent::Missing);
        }
        if String::from_utf8_lossy(&kind.stdout).trim() != "blob" {
            return Ok(FileContent::Opaque);
        }
        let content = ensure_git_success(
            "read Git file content",
            git_output(
                &self.root,
                &["cat-file".into(), "blob".into(), spec.to_string()],
                GIT_COMMAND_TIMEOUT,
            )
            .await?,
        )?;
        Ok(FileContent::Bytes(content.stdout))
    }

    fn git_spec(&self, revision: Option<&str>, path: &str) -> String {
        let repository_path = format!("{}{path}", self.repository_prefix);
        match revision {
            Some(revision) => format!("{revision}:{repository_path}"),
            None => format!(":{repository_path}"),
        }
    }
}

fn workspace_root(state: &CodeWebState, root_path: Option<String>) -> BootResult<PathBuf> {
    root_path
        .map(required_path)
        .transpose()
        .map(|root| root.unwrap_or_else(|| state.default_workspace.clone()))
}

fn local_root(services: &WorkspaceServices) -> BootResult<PathBuf> {
    services
        .local_root()
        .map(Path::to_path_buf)
        .ok_or_else(|| BootError::Internal("workspace services did not expose a local root".into()))
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct GitFileState {
    path: String,
    original_path: Option<String>,
    index_status: char,
    worktree_status: char,
}

impl GitFileState {
    fn into_json(self) -> Value {
        let index_status = self.index_status.to_string();
        let worktree_status = self.worktree_status.to_string();
        json!({
            "path": self.path,
            "indexStatus": index_status,
            "worktreeStatus": worktree_status,
            "status": format!("{index_status}{worktree_status}"),
        })
    }
}

fn parse_porcelain_status(output: &[u8], repository_prefix: &str) -> Vec<GitFileState> {
    let records = output
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
        .collect::<Vec<_>>();
    let mut files = Vec::new();
    let mut index = 0usize;
    while index < records.len() {
        let record = records[index];
        index += 1;
        if record.len() < 4 || record[2] != b' ' {
            continue;
        }
        let index_status = record[0] as char;
        let worktree_status = record[1] as char;
        let Ok(repository_path) = std::str::from_utf8(&record[3..]) else {
            continue;
        };
        let Some(path) = strip_repository_prefix(repository_path, repository_prefix) else {
            continue;
        };
        let original_path =
            if matches!(index_status, 'R' | 'C') || matches!(worktree_status, 'R' | 'C') {
                let original = records.get(index);
                if original.is_some() {
                    index += 1;
                }
                original
                    .and_then(|record| std::str::from_utf8(record).ok())
                    .and_then(|path| strip_repository_prefix(path, repository_prefix))
                    .map(str::to_string)
            } else {
                None
            };
        files.push(GitFileState {
            path: path.to_string(),
            original_path,
            index_status,
            worktree_status,
        });
    }
    files.sort_by(|left, right| {
        left.path
            .to_lowercase()
            .cmp(&right.path.to_lowercase())
            .then_with(|| left.path.cmp(&right.path))
    });
    files
}

fn strip_repository_prefix<'a>(path: &'a str, prefix: &str) -> Option<&'a str> {
    if prefix.is_empty() {
        Some(path)
    } else {
        path.strip_prefix(prefix).filter(|path| !path.is_empty())
    }
}

async fn git_repository_prefix(root: &Path) -> BootResult<String> {
    let output = ensure_git_success(
        "resolve workspace repository prefix",
        git_output(
            root,
            &["rev-parse".into(), "--show-prefix".into()],
            GIT_COMMAND_TIMEOUT,
        )
        .await?,
    )?;
    let prefix = String::from_utf8_lossy(&output.stdout)
        .trim()
        .trim_start_matches("./")
        .trim_end_matches('/')
        .to_string();
    Ok(if prefix.is_empty() {
        String::new()
    } else {
        format!("{prefix}/")
    })
}

fn required_paths(request: &Value) -> BootResult<Vec<String>> {
    let paths = request
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| BootError::BadRequest("paths is required".to_string()))?;
    if paths.is_empty() {
        return Err(BootError::BadRequest(
            "paths must contain at least one file".to_string(),
        ));
    }
    paths
        .iter()
        .map(|path| {
            path.as_str()
                .map(str::to_string)
                .ok_or_else(|| BootError::BadRequest("paths must contain strings".to_string()))
        })
        .collect()
}

fn required_text(request: &Value, field: &str) -> BootResult<String> {
    request
        .get(field)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| BootError::BadRequest(format!("{field} is required")))
}

fn validate_relative_path(path: &str) -> BootResult<String> {
    if path.contains('\0') {
        return Err(BootError::BadRequest(
            "Git file path must not contain a NUL byte".to_string(),
        ));
    }
    let normalized = path.trim().replace('\\', "/");
    let candidate = Path::new(&normalized);
    if normalized.is_empty()
        || candidate.is_absolute()
        || candidate
            .components()
            .any(|component| !matches!(component, Component::Normal(_)))
    {
        return Err(BootError::BadRequest(
            "Git file path must be a workspace-relative path without traversal".to_string(),
        ));
    }
    Ok(candidate
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => Some(value.to_string_lossy()),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

async fn git_output(root: &Path, args: &[String], limit: Duration) -> BootResult<Output> {
    let mut command = Command::new("git");
    command
        .arg("-C")
        .arg(root)
        .args(args)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("LC_ALL", "C")
        .env("LANG", "C")
        .kill_on_drop(true);
    timeout(limit, command.output())
        .await
        .map_err(|_| {
            BootError::RequestTimeout(format!(
                "Git command timed out after {} seconds",
                limit.as_secs()
            ))
        })?
        .map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                BootError::ServiceUnavailable("Git executable was not found".to_string())
            } else {
                BootError::Internal(format!("failed to run Git: {error}"))
            }
        })
}

fn ensure_git_success(operation: &str, output: Output) -> BootResult<Output> {
    if output.status.success() {
        Ok(output)
    } else {
        Err(git_failure(operation, &output))
    }
}

fn git_failure(operation: &str, output: &Output) -> BootError {
    let detail = command_message(output);
    BootError::Conflict(if detail.is_empty() {
        format!("could not {operation}")
    } else {
        format!("could not {operation}: {detail}")
    })
}

fn command_message(output: &Output) -> String {
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    match (stdout.is_empty(), stderr.is_empty()) {
        (false, false) => format!("{stdout}\n{stderr}"),
        (false, true) => stdout,
        (true, false) => stderr,
        (true, true) => String::new(),
    }
}

enum FileContent {
    Missing,
    Bytes(Vec<u8>),
    Opaque,
}

impl FileContent {
    fn is_opaque(&self) -> bool {
        matches!(self, Self::Opaque)
    }

    fn into_text(self, is_binary: &mut bool) -> String {
        match self {
            Self::Missing => String::new(),
            Self::Opaque => {
                *is_binary = true;
                String::new()
            }
            Self::Bytes(bytes) => match String::from_utf8(bytes) {
                Ok(value) => value,
                Err(_) => {
                    *is_binary = true;
                    String::new()
                }
            },
        }
    }
}

async fn worktree_content(root: &Path, path: &str) -> BootResult<FileContent> {
    let path = root.join(path);
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(FileContent::Missing);
        }
        Err(error) => return Err(BootError::Io(error)),
    };
    if metadata.file_type().is_symlink() {
        let target = tokio::fs::read_link(path).await.map_err(BootError::Io)?;
        return Ok(FileContent::Bytes(
            target.to_string_lossy().as_bytes().to_vec(),
        ));
    }
    if !metadata.is_file() {
        return Ok(FileContent::Opaque);
    }
    tokio::fs::read(path)
        .await
        .map(FileContent::Bytes)
        .map_err(BootError::Io)
}

fn git_reports_binary(content: &str) -> bool {
    content.lines().any(|line| {
        line == "GIT binary patch"
            || (line.starts_with("Binary files ") && line.ends_with(" differ"))
    })
}

fn unified_text(path: &str, original: &str, modified: &str) -> String {
    let original_header = format!("a/{path}");
    let modified_header = format!("b/{path}");
    TextDiff::from_lines(original, modified)
        .unified_diff()
        .header(&original_header, &modified_header)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn porcelain_parser_preserves_spaces_and_rename_sources() {
        let output =
            b" M src/app.ts\0?? notes with spaces.md\0R  src/new name.ts\0src/old name.ts\0";

        assert_eq!(
            parse_porcelain_status(output, ""),
            vec![
                GitFileState {
                    path: "notes with spaces.md".to_string(),
                    original_path: None,
                    index_status: '?',
                    worktree_status: '?',
                },
                GitFileState {
                    path: "src/app.ts".to_string(),
                    original_path: None,
                    index_status: ' ',
                    worktree_status: 'M',
                },
                GitFileState {
                    path: "src/new name.ts".to_string(),
                    original_path: Some("src/old name.ts".to_string()),
                    index_status: 'R',
                    worktree_status: ' ',
                },
            ]
        );
    }

    #[test]
    fn porcelain_paths_are_scoped_to_a_nested_workspace() {
        let output = b" M crates/app/src/main.rs\0?? outside.md\0";

        assert_eq!(
            parse_porcelain_status(output, "crates/app/"),
            vec![GitFileState {
                path: "src/main.rs".to_string(),
                original_path: None,
                index_status: ' ',
                worktree_status: 'M',
            }]
        );
    }

    #[test]
    fn git_paths_are_normalized_and_cannot_escape_the_workspace() {
        assert_eq!(validate_relative_path("src\\app.ts").unwrap(), "src/app.ts");
        for invalid in ["", ".", "../secret", "src/../../secret", "/tmp/secret"] {
            assert!(validate_relative_path(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn fallback_unified_diff_contains_complete_text_change() {
        let diff = unified_text("src/app.ts", "const value = 1;\n", "const value = 2;\n");

        assert!(diff.contains("--- a/src/app.ts"), "{diff}");
        assert!(diff.contains("+++ b/src/app.ts"), "{diff}");
        assert!(diff.contains("-const value = 1;"), "{diff}");
        assert!(diff.contains("+const value = 2;"), "{diff}");
    }

    #[test]
    fn binary_detection_only_accepts_git_metadata_lines() {
        assert!(git_reports_binary(
            "diff --git a/logo.png b/logo.png\nBinary files a/logo.png and b/logo.png differ\n"
        ));
        assert!(git_reports_binary(
            "diff --git a/logo.png b/logo.png\nGIT binary patch\nliteral 4\n"
        ));
        assert!(!git_reports_binary(
            "@@ -1 +1 @@\n-Binary files never create text models.\n+Binary files remain opaque in the editor.\n"
        ));
        assert!(!git_reports_binary(
            "The phrase GIT binary patch is documentation, not diff metadata."
        ));
    }
}
