//! Lightweight git context for the status bar.

use std::path::Path;
use std::process::Command;

/// Current Git branch of `dir`, including linked worktrees and detached HEAD.
pub(crate) fn git_branch(dir: &str) -> Option<String> {
    let dir = Path::new(dir);
    if let Some(branch) = git_stdout(dir, &["symbolic-ref", "--quiet", "--short", "HEAD"]) {
        return Some(branch);
    }

    git_stdout(dir, &["rev-parse", "--quiet", "--short", "HEAD"])
        .map(|commit| format!("detached@{commit}"))
}

fn git_stdout(dir: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let value = String::from_utf8(output.stdout).ok()?;
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn git(dir: &Path, args: &[&str]) {
        let output = Command::new("git")
            .arg("-C")
            .arg(dir)
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

    fn repository() -> tempfile::TempDir {
        let root = tempfile::tempdir().expect("temporary repository");
        git(root.path(), &["init"]);
        git(root.path(), &["config", "user.name", "A3S Test"]);
        git(
            root.path(),
            &["config", "user.email", "a3s-test@example.invalid"],
        );
        std::fs::write(root.path().join("README.md"), "initial\n").expect("write fixture");
        git(root.path(), &["add", "README.md"]);
        git(root.path(), &["commit", "-m", "initial"]);
        root
    }

    #[test]
    fn branch_is_discovered_in_normal_and_linked_worktrees() {
        let root = repository();
        let branch = git_branch(root.path().to_str().unwrap()).expect("normal branch");

        let linked_root = tempfile::tempdir().expect("linked worktree parent");
        let linked = linked_root.path().join("linked");
        let output = Command::new("git")
            .arg("-C")
            .arg(root.path())
            .args(["worktree", "add", "-b", "linked-test"])
            .arg(&linked)
            .arg("HEAD")
            .output()
            .expect("create linked worktree");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );

        assert!(!branch.is_empty());
        assert_eq!(
            git_branch(linked.to_str().unwrap()).as_deref(),
            Some("linked-test")
        );
    }

    #[test]
    fn detached_head_is_visible_instead_of_disappearing() {
        let root = repository();
        git(root.path(), &["checkout", "--detach"]);

        let branch = git_branch(root.path().to_str().unwrap()).expect("detached identity");
        assert!(branch.starts_with("detached@"), "{branch}");
        assert!(branch.len() > "detached@".len());
    }

    #[test]
    fn non_repository_has_no_git_identity() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(git_branch(root.path().to_str().unwrap()), None);
    }
}
