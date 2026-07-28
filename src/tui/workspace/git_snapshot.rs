//! Conflict-safe Git tree snapshots for isolated forks and conversation rewind.
//!
//! Snapshots use a temporary alternate index. The user's real index is never
//! read as the source of truth and is never modified.

use std::ffi::OsStr;
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Output, Stdio};

const MAX_BINARY_PATCH_BYTES: usize = 128 * 1024 * 1024;
const WORKTREE_DIRECTORY: &str = ".a3s-worktrees";

#[derive(Clone, Debug)]
pub(crate) struct GitTreeSnapshot {
    repository_root: PathBuf,
    workspace_relative: PathBuf,
    head_commit: String,
    head_tree: String,
    tree: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum GitPatchDirection {
    Forward,
    Reverse,
}

#[derive(Clone, Debug)]
pub(crate) struct GitBinaryPatch {
    bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct IsolatedWorktree {
    pub(crate) root: PathBuf,
    pub(crate) workspace: PathBuf,
    pub(crate) branch: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct GitSnapshotError {
    message: String,
    retained_worktree: Option<PathBuf>,
}

impl GitSnapshotError {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retained_worktree: None,
        }
    }

    fn retaining(mut self, path: &Path) -> Self {
        self.retained_worktree = Some(path.to_path_buf());
        self
    }
}

impl fmt::Display for GitSnapshotError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)?;
        if let Some(path) = &self.retained_worktree {
            write!(
                formatter,
                "; the recoverable worktree was retained at {}",
                path.display()
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for GitSnapshotError {}

impl GitTreeSnapshot {
    /// Capture the current content of `workspace` into an immutable Git tree.
    ///
    /// The tree starts at `HEAD`, overlays only the requested workspace scope,
    /// and excludes TUI persistence so session history is copied explicitly.
    pub(crate) fn capture(workspace: &Path) -> Result<Self, GitSnapshotError> {
        let workspace = workspace.canonicalize().map_err(|error| {
            GitSnapshotError::new(format!(
                "could not resolve workspace {}: {error}",
                workspace.display()
            ))
        })?;
        let repository_root = path_from_git(
            &git_output(&workspace, ["rev-parse", "--show-toplevel"])?,
            "repository root",
        )?
        .canonicalize()
        .map_err(|error| {
            GitSnapshotError::new(format!("could not resolve repository root: {error}"))
        })?;
        let workspace_relative = repository_relative_workspace(&repository_root, &workspace)?;
        let head_commit = git_text(
            &repository_root,
            ["rev-parse", "--verify", "HEAD^{commit}"],
            "resolve HEAD commit",
        )?;
        let head_tree = git_text(
            &repository_root,
            ["rev-parse", "--verify", "HEAD^{tree}"],
            "resolve HEAD tree",
        )?;

        let temporary = tempfile::tempdir().map_err(|error| {
            GitSnapshotError::new(format!("could not create an alternate Git index: {error}"))
        })?;
        let alternate_index = temporary.path().join("index");

        let mut read_tree = git_command(&repository_root);
        read_tree
            .env("GIT_INDEX_FILE", &alternate_index)
            .args(["read-tree", &head_commit]);
        checked_output(read_tree, "seed alternate Git index from HEAD")?;

        let scope = git_path(&workspace_relative)?;
        let exclusions = tui_exclusion_pathspecs(&scope);
        let mut add = git_command(&repository_root);
        add.env("GIT_INDEX_FILE", &alternate_index)
            .args(["add", "-A", "--"])
            .arg(&scope);
        for exclusion in &exclusions {
            add.arg(exclusion);
        }
        checked_output(add, "capture workspace content in alternate Git index")?;

        let mut write_tree = git_command(&repository_root);
        write_tree
            .env("GIT_INDEX_FILE", &alternate_index)
            .arg("write-tree");
        let tree = output_text(
            checked_output(write_tree, "write captured Git tree")?,
            "captured Git tree",
        )?;

        Ok(Self {
            repository_root,
            workspace_relative,
            head_commit,
            head_tree,
            tree,
        })
    }

    pub(crate) fn diff_to(
        &self,
        later: &GitTreeSnapshot,
    ) -> Result<GitBinaryPatch, GitSnapshotError> {
        if self.repository_root != later.repository_root
            || self.workspace_relative != later.workspace_relative
        {
            return Err(GitSnapshotError::new(
                "cannot diff snapshots from different Git workspace scopes",
            ));
        }
        binary_patch(
            &self.repository_root,
            &self.workspace_relative,
            &self.tree,
            &later.tree,
        )
    }

    pub(crate) fn patch_from_head(&self) -> Result<GitBinaryPatch, GitSnapshotError> {
        binary_patch(
            &self.repository_root,
            &self.workspace_relative,
            &self.head_tree,
            &self.tree,
        )
    }

    pub(crate) fn repository_root(&self) -> &Path {
        &self.repository_root
    }

    /// Create a sibling branch/worktree at the captured commit and transfer the
    /// captured dirty state. Failures after creation deliberately retain it.
    pub(crate) fn fork_worktree(
        &self,
        identity: &str,
    ) -> Result<IsolatedWorktree, GitSnapshotError> {
        let patch = self.patch_from_head()?;
        let repository_name = safe_component(
            self.repository_root
                .file_name()
                .and_then(OsStr::to_str)
                .unwrap_or("repository"),
            "repository",
        );
        let identity = safe_component(identity, "session");
        let parent = self.repository_root.parent().ok_or_else(|| {
            GitSnapshotError::new(format!(
                "repository {} has no parent for isolated worktrees",
                self.repository_root.display()
            ))
        })?;
        let base = parent.join(WORKTREE_DIRECTORY).join(repository_name);
        fs::create_dir_all(&base).map_err(|error| {
            GitSnapshotError::new(format!(
                "could not create worktree directory {}: {error}",
                base.display()
            ))
        })?;

        let (root, branch) = self.unique_worktree_target(&base, &identity)?;
        let mut add = git_command(&self.repository_root);
        add.args(["worktree", "add", "-b", &branch])
            .arg(&root)
            .arg(&self.head_commit);
        if let Err(error) = checked_output(add, "create isolated Git worktree") {
            return Err(if root.exists() {
                error.retaining(&root)
            } else {
                error
            });
        }

        if let Err(error) = patch.apply_checked(&root, GitPatchDirection::Forward) {
            return Err(error.retaining(&root));
        }

        let workspace = if self.workspace_relative.as_os_str().is_empty() {
            root.clone()
        } else {
            root.join(&self.workspace_relative)
        };
        fs::create_dir_all(&workspace).map_err(|error| {
            GitSnapshotError::new(format!(
                "could not create forked workspace {}: {error}",
                workspace.display()
            ))
            .retaining(&root)
        })?;

        Ok(IsolatedWorktree {
            root,
            workspace,
            branch,
        })
    }

    fn unique_worktree_target(
        &self,
        base: &Path,
        identity: &str,
    ) -> Result<(PathBuf, String), GitSnapshotError> {
        for sequence in 0..100_u8 {
            let suffix = if sequence == 0 {
                identity.to_string()
            } else {
                format!("{identity}-{sequence}")
            };
            let root = base.join(&suffix);
            let branch = format!("a3s/fork-{suffix}");
            if root.exists() || branch_exists(&self.repository_root, &branch)? {
                continue;
            }
            return Ok((root, branch));
        }
        Err(GitSnapshotError::new(format!(
            "could not allocate a unique isolated worktree under {}",
            base.display()
        )))
    }
}

impl GitBinaryPatch {
    pub(crate) fn is_empty(&self) -> bool {
        self.bytes.is_empty()
    }

    /// Validate the whole patch before applying it. A failed check leaves every
    /// file untouched, which is the safety boundary reused by rewind.
    pub(crate) fn apply_checked(
        &self,
        repository: &Path,
        direction: GitPatchDirection,
    ) -> Result<(), GitSnapshotError> {
        if self.is_empty() {
            return Ok(());
        }
        self.check_apply(repository, direction)?;
        git_apply(repository, &self.bytes, direction, false)
    }

    pub(crate) fn check_apply(
        &self,
        repository: &Path,
        direction: GitPatchDirection,
    ) -> Result<(), GitSnapshotError> {
        if self.is_empty() {
            return Ok(());
        }
        git_apply(repository, &self.bytes, direction, true)
    }
}

fn binary_patch(
    repository_root: &Path,
    workspace_relative: &Path,
    before: &str,
    after: &str,
) -> Result<GitBinaryPatch, GitSnapshotError> {
    let scope = git_path(workspace_relative)?;
    let exclusions = tui_exclusion_pathspecs(&scope);
    let mut command = git_command(repository_root);
    command
        .args([
            "diff",
            "--binary",
            "--full-index",
            "--no-ext-diff",
            "--no-textconv",
            before,
            after,
            "--",
        ])
        .arg(&scope);
    for exclusion in &exclusions {
        command.arg(exclusion);
    }
    let output = checked_output(command, "create binary workspace patch")?;
    if output.stdout.len() > MAX_BINARY_PATCH_BYTES {
        return Err(GitSnapshotError::new(format!(
            "workspace patch is {} MiB; the safety limit is {} MiB",
            output.stdout.len().div_ceil(1024 * 1024),
            MAX_BINARY_PATCH_BYTES / (1024 * 1024)
        )));
    }
    Ok(GitBinaryPatch {
        bytes: output.stdout,
    })
}

fn git_apply(
    repository: &Path,
    patch: &[u8],
    direction: GitPatchDirection,
    check: bool,
) -> Result<(), GitSnapshotError> {
    let mut command = git_command(repository);
    command
        .arg("apply")
        .arg("--binary")
        .arg("--whitespace=nowarn");
    if check {
        command.arg("--check");
    }
    if direction == GitPatchDirection::Reverse {
        command.arg("--reverse");
    }
    command
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = command
        .spawn()
        .map_err(|error| GitSnapshotError::new(format!("could not start git apply: {error}")))?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| GitSnapshotError::new("git apply stdin was unavailable"))
        .and_then(|mut stdin| {
            stdin
                .write_all(patch)
                .map_err(|error| GitSnapshotError::new(format!("could not stream patch: {error}")))
        });
    let output = child
        .wait_with_output()
        .map_err(|error| GitSnapshotError::new(format!("could not wait for git apply: {error}")))?;
    write_result?;
    if !output.status.success() {
        let action = if check {
            "patch does not apply cleanly"
        } else {
            "git could not apply the validated patch"
        };
        return Err(command_failure(action, &output));
    }
    Ok(())
}

fn repository_relative_workspace(
    repository_root: &Path,
    workspace: &Path,
) -> Result<PathBuf, GitSnapshotError> {
    workspace
        .strip_prefix(repository_root)
        .map(Path::to_path_buf)
        .map_err(|_| {
            GitSnapshotError::new(format!(
                "workspace {} is outside repository {}",
                workspace.display(),
                repository_root.display()
            ))
        })
}

fn git_path(path: &Path) -> Result<String, GitSnapshotError> {
    if path.as_os_str().is_empty() {
        return Ok(".".to_string());
    }
    let mut parts = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => parts.push(part.to_string_lossy().into_owned()),
            _ => {
                return Err(GitSnapshotError::new(format!(
                    "Git workspace scope must be repository-relative: {}",
                    path.display()
                )))
            }
        }
    }
    Ok(parts.join("/"))
}

fn tui_exclusion_pathspecs(scope: &str) -> [String; 2] {
    let prefix = if scope == "." {
        String::new()
    } else {
        format!("{scope}/")
    };
    [
        format!(":(exclude){prefix}.a3s/tui"),
        format!(":(exclude){prefix}.a3s/tui/**"),
    ]
}

fn safe_component(value: &str, fallback: &str) -> String {
    let mut result = String::new();
    let mut separator = false;
    for character in value.chars() {
        if character.is_ascii_alphanumeric() {
            result.push(character.to_ascii_lowercase());
            separator = false;
        } else if !separator && !result.is_empty() {
            result.push('-');
            separator = true;
        }
        if result.len() >= 32 {
            break;
        }
    }
    while result.ends_with('-') {
        result.pop();
    }
    if result.is_empty() {
        fallback.to_string()
    } else {
        result
    }
}

fn branch_exists(repository: &Path, branch: &str) -> Result<bool, GitSnapshotError> {
    let output = git_command(repository)
        .args([
            "show-ref",
            "--verify",
            "--quiet",
            &format!("refs/heads/{branch}"),
        ])
        .output()
        .map_err(|error| {
            GitSnapshotError::new(format!("could not inspect Git branches: {error}"))
        })?;
    match output.status.code() {
        Some(0) => Ok(true),
        Some(1) => Ok(false),
        _ => Err(command_failure("could not inspect Git branches", &output)),
    }
}

fn git_command(repository: &Path) -> Command {
    let mut command = Command::new("git");
    command.arg("-C").arg(repository);
    command
}

fn git_output<I, S>(repository: &Path, args: I) -> Result<Output, GitSnapshotError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    let mut command = git_command(repository);
    command.args(args);
    checked_output(command, "inspect Git repository")
}

fn git_text<I, S>(repository: &Path, args: I, description: &str) -> Result<String, GitSnapshotError>
where
    I: IntoIterator<Item = S>,
    S: AsRef<OsStr>,
{
    output_text(git_output(repository, args)?, description)
}

fn checked_output(mut command: Command, action: &str) -> Result<Output, GitSnapshotError> {
    let output = command
        .output()
        .map_err(|error| GitSnapshotError::new(format!("{action}: {error}")))?;
    if output.status.success() {
        Ok(output)
    } else {
        Err(command_failure(action, &output))
    }
}

fn command_failure(action: &str, output: &Output) -> GitSnapshotError {
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stderr = stderr.trim();
    if stderr.is_empty() {
        GitSnapshotError::new(format!("{action} failed with status {}", output.status))
    } else {
        GitSnapshotError::new(format!("{action}: {stderr}"))
    }
}

fn output_text(output: Output, description: &str) -> Result<String, GitSnapshotError> {
    let value = String::from_utf8(output.stdout).map_err(|error| {
        GitSnapshotError::new(format!("{description} was not valid UTF-8: {error}"))
    })?;
    let value = value.trim();
    if value.is_empty() {
        Err(GitSnapshotError::new(format!("{description} was empty")))
    } else {
        Ok(value.to_string())
    }
}

fn path_from_git(output: &Output, description: &str) -> Result<PathBuf, GitSnapshotError> {
    output_text(
        Output {
            status: output.status,
            stdout: output.stdout.clone(),
            stderr: output.stderr.clone(),
        },
        description,
    )
    .map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestRepository {
        _root: tempfile::TempDir,
        repository: PathBuf,
    }

    impl TestRepository {
        fn path(&self) -> &Path {
            &self.repository
        }
    }

    fn git(repository: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(repository)
            .args(args)
            .output()
            .expect("run git");
        assert!(
            output.status.success(),
            "git {:?} failed: {}",
            args,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    fn repository() -> TestRepository {
        let root = tempfile::tempdir().expect("temporary repository");
        let repository = root.path().join("repository");
        std::fs::create_dir_all(&repository).unwrap();
        git(&repository, &["init"]);
        git(&repository, &["config", "user.name", "A3S Test"]);
        git(
            &repository,
            &["config", "user.email", "a3s-test@example.invalid"],
        );
        std::fs::write(repository.join("tracked.txt"), "base\n").unwrap();
        git(&repository, &["add", "tracked.txt"]);
        git(&repository, &["commit", "-m", "initial"]);
        TestRepository {
            _root: root,
            repository,
        }
    }

    fn real_index(repository: &Path) -> Vec<u8> {
        let git_dir = git_text(
            repository,
            ["rev-parse", "--absolute-git-dir"],
            "Git directory",
        )
        .unwrap();
        std::fs::read(Path::new(&git_dir).join("index")).unwrap()
    }

    #[test]
    fn isolated_fork_transfers_dirty_and_untracked_files_without_touching_index() {
        let root = repository();
        std::fs::write(root.path().join("tracked.txt"), "dirty\n").unwrap();
        std::fs::write(root.path().join("untracked.txt"), "new\n").unwrap();
        std::fs::create_dir_all(root.path().join(".a3s/tui")).unwrap();
        std::fs::write(root.path().join(".a3s/tui/private.json"), "private\n").unwrap();

        let index_before = real_index(root.path());
        let snapshot = GitTreeSnapshot::capture(root.path()).unwrap();
        assert_eq!(real_index(root.path()), index_before);

        let fork = snapshot.fork_worktree("dirty-fixture").unwrap();
        assert_eq!(
            std::fs::read_to_string(fork.workspace.join("tracked.txt")).unwrap(),
            "dirty\n"
        );
        assert_eq!(
            std::fs::read_to_string(fork.workspace.join("untracked.txt")).unwrap(),
            "new\n"
        );
        assert!(!fork.workspace.join(".a3s/tui/private.json").exists());
        assert_eq!(real_index(root.path()), index_before);
    }

    #[test]
    fn nested_workspace_capture_does_not_transfer_sibling_changes() {
        let root = repository();
        let nested = root.path().join("nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(nested.join("inside.txt"), "inside base\n").unwrap();
        std::fs::write(root.path().join("outside.txt"), "outside base\n").unwrap();
        git(root.path(), &["add", "nested/inside.txt", "outside.txt"]);
        git(root.path(), &["commit", "-m", "nested fixture"]);

        std::fs::write(nested.join("inside.txt"), "inside dirty\n").unwrap();
        std::fs::write(nested.join("new.txt"), "inside new\n").unwrap();
        std::fs::write(root.path().join("outside.txt"), "outside dirty\n").unwrap();
        std::fs::write(root.path().join("outside-new.txt"), "outside new\n").unwrap();

        let snapshot = GitTreeSnapshot::capture(&nested).unwrap();
        let fork = snapshot.fork_worktree("nested-fixture").unwrap();
        assert_eq!(
            std::fs::read_to_string(fork.workspace.join("inside.txt")).unwrap(),
            "inside dirty\n"
        );
        assert_eq!(
            std::fs::read_to_string(fork.workspace.join("new.txt")).unwrap(),
            "inside new\n"
        );
        assert_eq!(
            std::fs::read_to_string(fork.root.join("outside.txt")).unwrap(),
            "outside base\n"
        );
        assert!(!fork.root.join("outside-new.txt").exists());
    }

    #[test]
    fn reverse_patch_refuses_conflicting_content_without_partial_rewind() {
        let root = repository();
        let before = GitTreeSnapshot::capture(root.path()).unwrap();
        std::fs::write(root.path().join("tracked.txt"), "after\n").unwrap();
        let after = GitTreeSnapshot::capture(root.path()).unwrap();
        let patch = before.diff_to(&after).unwrap();

        std::fs::write(root.path().join("tracked.txt"), "conflict\n").unwrap();
        let error = patch
            .apply_checked(root.path(), GitPatchDirection::Reverse)
            .unwrap_err();

        assert!(error.to_string().contains("does not apply cleanly"));
        assert_eq!(
            std::fs::read_to_string(root.path().join("tracked.txt")).unwrap(),
            "conflict\n"
        );
    }

    #[test]
    fn reverse_patch_restores_pre_turn_files_without_touching_real_index() {
        let root = repository();
        let index_before = real_index(root.path());
        let before = GitTreeSnapshot::capture(root.path()).unwrap();
        std::fs::write(root.path().join("tracked.txt"), "after\n").unwrap();
        std::fs::write(root.path().join("created.txt"), "created\n").unwrap();
        let after = GitTreeSnapshot::capture(root.path()).unwrap();
        let patch = before.diff_to(&after).unwrap();

        patch
            .apply_checked(root.path(), GitPatchDirection::Reverse)
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(root.path().join("tracked.txt")).unwrap(),
            "base\n"
        );
        assert!(!root.path().join("created.txt").exists());
        assert_eq!(real_index(root.path()), index_before);
    }
}
