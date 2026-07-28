//! Exact, typed permission grants for the interactive TUI.
//!
//! Session grants live only in memory. Project grants are stored separately in
//! `.a3s/permissions.acl` so granting one capability cannot rewrite unrelated
//! project configuration or comments.

use std::collections::HashMap;
use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};

use a3s_acl::{Block, Document, Value as AclValue};

const PROJECT_PERMISSION_RULES_RELATIVE_PATH: &str = ".a3s/permissions.acl";
const MAX_PERMISSION_RULES_BYTES: u64 = 256 * 1024;
const MAX_PERMISSION_RULES: usize = 256;
const MAX_TOOL_NAME_BYTES: usize = 256;

/// A conservative capability derived from an authoritative tool invocation.
///
/// File mutations are scoped to the exact operation and path. Shell execution
/// is scoped to the exact command. Other tools retain their complete canonical
/// argument object, which may cause a safe re-prompt if a provider changes
/// inconsequential fields but can never broaden the grant.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct ExactPermissionGrant {
    tool_name: String,
    args: serde_json::Value,
}

impl ExactPermissionGrant {
    pub(super) fn from_invocation(tool_name: &str, args: &serde_json::Value) -> Self {
        let normalized_tool = normalize_tool_name(tool_name);
        let projected = match normalized_tool.as_str() {
            "bash" => project_fields(args, &["command"]),
            "write" | "edit" | "patch" => project_fields(args, &["file_path"]),
            "skill" => project_fields(args, &["name", "skill"]),
            _ => None,
        }
        .unwrap_or_else(|| canonicalize_json(args));

        Self {
            tool_name: normalized_tool,
            args: projected,
        }
    }

    fn from_persisted(tool_name: &str, args: serde_json::Value) -> Result<Self, String> {
        validate_tool_name(tool_name)?;
        Ok(Self {
            tool_name: normalize_tool_name(tool_name),
            args: canonicalize_json(&args),
        })
    }

    pub(super) fn matches(&self, tool_name: &str, args: &serde_json::Value) -> bool {
        self.tool_name == normalize_tool_name(tool_name)
            && self.args == Self::from_invocation(tool_name, args).args
    }

    pub(super) fn scope_label(&self) -> String {
        let detail = match self.tool_name.as_str() {
            "bash" => self.args.get("command").and_then(serde_json::Value::as_str),
            "write" | "edit" | "patch" => self
                .args
                .get("file_path")
                .and_then(serde_json::Value::as_str),
            "skill" => self
                .args
                .get("name")
                .or_else(|| self.args.get("skill"))
                .and_then(serde_json::Value::as_str),
            _ => None,
        };
        match detail {
            Some(detail) => format!("{}({detail})", self.tool_name),
            None => format!("{}({})", self.tool_name, canonical_json_string(&self.args)),
        }
    }

    pub(super) fn stable_key(&self) -> String {
        format!(
            "{}\u{0}{}",
            self.tool_name,
            canonical_json_string(&self.args)
        )
    }

    pub(super) fn tool_name(&self) -> &str {
        &self.tool_name
    }

    pub(super) fn args(&self) -> &serde_json::Value {
        &self.args
    }
}

/// Shared by every session rebuild so an in-memory grant does not disappear
/// when the user switches model or effort.
#[derive(Clone, Debug, Default)]
pub(super) struct TuiPermissionGrants {
    session: Arc<RwLock<Vec<ExactPermissionGrant>>>,
    project: Arc<RwLock<Vec<ExactPermissionGrant>>>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub(super) struct PermissionGrantSnapshot {
    pub(super) session: Vec<ExactPermissionGrant>,
    pub(super) project: Vec<ExactPermissionGrant>,
}

impl TuiPermissionGrants {
    pub(super) fn with_project(grants: Vec<ExactPermissionGrant>) -> Self {
        Self {
            session: Arc::default(),
            project: Arc::new(RwLock::new(deduplicate_grants(grants))),
        }
    }

    pub(super) fn allows(&self, tool_name: &str, args: &serde_json::Value) -> bool {
        self.session
            .read()
            .ok()
            .is_some_and(|grants| grants.iter().any(|grant| grant.matches(tool_name, args)))
            || self
                .project
                .read()
                .ok()
                .is_some_and(|grants| grants.iter().any(|grant| grant.matches(tool_name, args)))
    }

    pub(super) fn allow_for_session(&self, grant: ExactPermissionGrant) {
        insert_grant(&self.session, grant);
    }

    pub(super) fn allow_for_project(&self, grant: ExactPermissionGrant) {
        insert_grant(&self.project, grant);
    }

    pub(super) fn snapshot(&self) -> PermissionGrantSnapshot {
        PermissionGrantSnapshot {
            session: self
                .session
                .read()
                .map(|grants| grants.clone())
                .unwrap_or_default(),
            project: self
                .project
                .read()
                .map(|grants| grants.clone())
                .unwrap_or_default(),
        }
    }

    pub(super) fn revoke_session(&self, stable_key: &str) -> bool {
        remove_grant(&self.session, stable_key)
    }

    pub(super) fn revoke_project(&self, stable_key: &str) -> bool {
        remove_grant(&self.project, stable_key)
    }

    pub(super) fn replace_project(&self, grants: Vec<ExactPermissionGrant>) {
        if let Ok(mut current) = self.project.write() {
            *current = deduplicate_grants(grants);
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct ProjectPermissionRevocation {
    pub(super) path: PathBuf,
    pub(super) grants: Vec<ExactPermissionGrant>,
    pub(super) removed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct PendingToolApproval {
    pub(super) tool_id: String,
    pub(super) tool_name: String,
    pub(super) args: serde_json::Value,
    pub(super) label: String,
    pub(super) grant: ExactPermissionGrant,
}

impl PendingToolApproval {
    pub(super) fn new(
        tool_id: String,
        tool_name: String,
        args: serde_json::Value,
        label: String,
    ) -> Self {
        let grant = ExactPermissionGrant::from_invocation(&tool_name, &args);
        Self {
            tool_id,
            tool_name,
            args,
            label,
            grant,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ApprovalFeedback {
    pub(super) tool_id: String,
    pub(super) stashed_composer: String,
}

pub(super) fn project_permission_rules_path(workspace: &Path) -> PathBuf {
    workspace.join(PROJECT_PERMISSION_RULES_RELATIVE_PATH)
}

pub(super) fn load_project_permission_grants(
    path: &Path,
) -> Result<Vec<ExactPermissionGrant>, String> {
    let Some(source) = read_bounded_acl(path)? else {
        return Ok(Vec::new());
    };
    parse_project_permission_grants(&source)
}

pub(super) fn persist_project_permission_grant(
    path: &Path,
    grant: ExactPermissionGrant,
) -> Result<PathBuf, String> {
    reject_symlink_target(path)?;
    let mut grants = load_project_permission_grants(path)?;
    grants.push(grant);
    let grants = deduplicate_grants(grants);
    if grants.len() > MAX_PERMISSION_RULES {
        return Err(format!(
            "project permission rule limit exceeded ({MAX_PERMISSION_RULES})"
        ));
    }
    persist_project_permission_grants(path, &grants)
}

pub(super) fn revoke_project_permission_grant(
    path: &Path,
    stable_key: &str,
) -> Result<ProjectPermissionRevocation, String> {
    reject_symlink_target(path)?;
    let mut grants = load_project_permission_grants(path)?;
    let before = grants.len();
    grants.retain(|grant| grant.stable_key() != stable_key);
    let removed = grants.len() != before;
    if removed {
        persist_project_permission_grants(path, &grants)?;
    }
    Ok(ProjectPermissionRevocation {
        path: path.to_path_buf(),
        grants,
        removed,
    })
}

fn persist_project_permission_grants(
    path: &Path,
    grants: &[ExactPermissionGrant],
) -> Result<PathBuf, String> {
    reject_symlink_target(path)?;
    if grants.len() > MAX_PERMISSION_RULES {
        return Err(format!(
            "project permission rule limit exceeded ({MAX_PERMISSION_RULES})"
        ));
    }
    let source = generate_project_permission_grants(grants)?;
    let parent = path
        .parent()
        .ok_or_else(|| format!("permission rule path has no parent: {}", path.display()))?;
    reject_symlink_directory(parent)?;
    std::fs::create_dir_all(parent).map_err(|error| {
        format!(
            "failed to create permission rule directory {}: {error}",
            parent.display()
        )
    })?;
    reject_symlink_directory(parent)?;

    let mut temporary = tempfile::NamedTempFile::new_in(parent).map_err(|error| {
        format!(
            "failed to create temporary permission rule file in {}: {error}",
            parent.display()
        )
    })?;
    if let Ok(metadata) = std::fs::metadata(path) {
        temporary
            .as_file()
            .set_permissions(metadata.permissions())
            .map_err(|error| {
                format!(
                    "failed to preserve permissions for {}: {error}",
                    path.display()
                )
            })?;
    }
    temporary.write_all(source.as_bytes()).map_err(|error| {
        format!(
            "failed to write temporary permission rules for {}: {error}",
            path.display()
        )
    })?;
    temporary.as_file_mut().sync_all().map_err(|error| {
        format!(
            "failed to sync temporary permission rules for {}: {error}",
            path.display()
        )
    })?;
    temporary.persist(path).map_err(|error| {
        format!(
            "failed to atomically replace permission rules {}: {}",
            path.display(),
            error.error
        )
    })?;
    sync_parent_directory(parent)?;
    Ok(path.to_path_buf())
}

fn read_bounded_acl(path: &Path) -> Result<Option<String>, String> {
    let metadata = match std::fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(format!(
                "failed to inspect permission rules {}: {error}",
                path.display()
            ));
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "permission rules must not be a symbolic link: {}",
            path.display()
        ));
    }
    if !metadata.is_file() {
        return Err(format!(
            "permission rule path is not a regular file: {}",
            path.display()
        ));
    }
    if metadata.len() > MAX_PERMISSION_RULES_BYTES {
        return Err(format!(
            "permission rule file exceeds {} bytes: {}",
            MAX_PERMISSION_RULES_BYTES,
            path.display()
        ));
    }

    let mut source = String::with_capacity(metadata.len() as usize);
    OpenOptions::new()
        .read(true)
        .open(path)
        .and_then(|mut file| file.read_to_string(&mut source))
        .map_err(|error| {
            format!(
                "failed to read permission rules {}: {error}",
                path.display()
            )
        })?;
    Ok(Some(source))
}

fn parse_project_permission_grants(source: &str) -> Result<Vec<ExactPermissionGrant>, String> {
    let document = a3s_acl::parse_acl(source)
        .map_err(|error| format!("failed to parse project permission ACL: {error}"))?;
    let mut grants = Vec::new();

    for block in document.blocks {
        if block.name != "permissions" {
            return Err(format!(
                "unexpected top-level entry `{}` in project permission ACL",
                block.name
            ));
        }
        if !block.labels.is_empty() || !block.attributes.is_empty() {
            return Err("the `permissions` block accepts only nested `allow` rules".to_string());
        }
        for rule in block.blocks {
            if rule.name != "allow" {
                return Err(format!(
                    "unsupported permission rule `{}`; only `allow` is accepted",
                    rule.name
                ));
            }
            if !rule.blocks.is_empty() {
                return Err("permission `allow` rules cannot contain nested blocks".to_string());
            }
            let tool_name = rule
                .labels
                .first()
                .filter(|_| rule.labels.len() == 1)
                .ok_or_else(|| {
                    "permission `allow` rules require exactly one tool label".to_string()
                })?;
            let arguments = rule
                .attributes
                .get("arguments")
                .and_then(AclValue::as_str)
                .ok_or_else(|| {
                    format!("permission rule `{tool_name}` requires a string `arguments` value")
                })?;
            if rule.attributes.len() != 1 {
                return Err(format!(
                    "permission rule `{tool_name}` contains unsupported attributes"
                ));
            }
            let args = serde_json::from_str(arguments).map_err(|error| {
                format!("permission rule `{tool_name}` has invalid arguments: {error}")
            })?;
            grants.push(ExactPermissionGrant::from_persisted(tool_name, args)?);
            if grants.len() > MAX_PERMISSION_RULES {
                return Err(format!(
                    "project permission rule limit exceeded ({MAX_PERMISSION_RULES})"
                ));
            }
        }
    }
    Ok(deduplicate_grants(grants))
}

fn generate_project_permission_grants(grants: &[ExactPermissionGrant]) -> Result<String, String> {
    let mut grants = deduplicate_grants(grants.to_vec());
    grants.sort_by_key(ExactPermissionGrant::stable_key);
    let rules = grants
        .iter()
        .map(|grant| Block {
            name: "allow".to_string(),
            labels: vec![grant.tool_name.clone()],
            blocks: Vec::new(),
            attributes: HashMap::from([(
                "arguments".to_string(),
                AclValue::String(canonical_json_string(&grant.args)),
            )]),
        })
        .collect();
    let document = Document {
        blocks: vec![Block {
            name: "permissions".to_string(),
            labels: Vec::new(),
            blocks: rules,
            attributes: HashMap::new(),
        }],
    };
    let generated = a3s_acl::generate_acl(&document);
    let reparsed = parse_project_permission_grants(&generated)?;
    if reparsed != grants {
        return Err("generated project permission ACL did not round-trip".to_string());
    }
    Ok(generated)
}

fn project_fields(args: &serde_json::Value, fields: &[&str]) -> Option<serde_json::Value> {
    let object = args.as_object()?;
    let projected = fields
        .iter()
        .filter_map(|field| {
            object
                .get(*field)
                .map(|value| ((*field).to_string(), canonicalize_json(value)))
        })
        .collect::<serde_json::Map<_, _>>();
    (!projected.is_empty()).then_some(serde_json::Value::Object(projected))
}

fn canonicalize_json(value: &serde_json::Value) -> serde_json::Value {
    match value {
        serde_json::Value::Array(values) => {
            serde_json::Value::Array(values.iter().map(canonicalize_json).collect())
        }
        serde_json::Value::Object(values) => {
            let mut entries = values.iter().collect::<Vec<_>>();
            entries.sort_by_key(|(key, _)| *key);
            serde_json::Value::Object(
                entries
                    .into_iter()
                    .map(|(key, value)| (key.clone(), canonicalize_json(value)))
                    .collect(),
            )
        }
        value => value.clone(),
    }
}

fn canonical_json_string(value: &serde_json::Value) -> String {
    serde_json::to_string(&canonicalize_json(value)).unwrap_or_else(|_| "null".to_string())
}

fn normalize_tool_name(tool_name: &str) -> String {
    tool_name.trim().to_ascii_lowercase()
}

fn validate_tool_name(tool_name: &str) -> Result<(), String> {
    let tool_name = tool_name.trim();
    if tool_name.is_empty() || tool_name.len() > MAX_TOOL_NAME_BYTES {
        return Err("permission rule tool name is empty or too long".to_string());
    }
    if !tool_name
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '_' | '-' | '.' | ':'))
    {
        return Err(format!(
            "permission rule tool name contains unsupported characters: `{tool_name}`"
        ));
    }
    Ok(())
}

fn deduplicate_grants(grants: Vec<ExactPermissionGrant>) -> Vec<ExactPermissionGrant> {
    let mut unique = Vec::new();
    for grant in grants {
        if !unique.contains(&grant) {
            unique.push(grant);
        }
    }
    unique
}

fn insert_grant(target: &RwLock<Vec<ExactPermissionGrant>>, grant: ExactPermissionGrant) {
    if let Ok(mut grants) = target.write() {
        if !grants.contains(&grant) {
            grants.push(grant);
        }
    }
}

fn remove_grant(target: &RwLock<Vec<ExactPermissionGrant>>, stable_key: &str) -> bool {
    let Ok(mut grants) = target.write() else {
        return false;
    };
    let before = grants.len();
    grants.retain(|grant| grant.stable_key() != stable_key);
    grants.len() != before
}

fn reject_symlink_target(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing to replace symbolic-link permission rules: {}",
            path.display()
        )),
        Ok(metadata) if !metadata.is_file() => Err(format!(
            "permission rule path is not a regular file: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect permission rules {}: {error}",
            path.display()
        )),
    }
}

fn reject_symlink_directory(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "permission rule directory must not be a symbolic link: {}",
            path.display()
        )),
        Ok(metadata) if !metadata.is_dir() => Err(format!(
            "permission rule parent is not a directory: {}",
            path.display()
        )),
        Ok(_) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(format!(
            "failed to inspect permission rule directory {}: {error}",
            path.display()
        )),
    }
}

#[cfg(unix)]
fn sync_parent_directory(path: &Path) -> Result<(), String> {
    std::fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| {
            format!(
                "failed to sync permission rule directory {}: {error}",
                path.display()
            )
        })
}

#[cfg(not(unix))]
fn sync_parent_directory(_path: &Path) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_and_shell_grants_use_narrow_authoritative_fields() {
        let bash = ExactPermissionGrant::from_invocation(
            "Bash",
            &serde_json::json!({
                "command": "cargo test -p a3s",
                "description": "provider decoration"
            }),
        );
        assert!(bash.matches(
            "bash",
            &serde_json::json!({
                "description": "different decoration",
                "command": "cargo test -p a3s"
            })
        ));
        assert!(!bash.matches(
            "bash",
            &serde_json::json!({"command": "cargo test --workspace"})
        ));

        let write = ExactPermissionGrant::from_invocation(
            "Write",
            &serde_json::json!({"file_path": "src/lib.rs", "content": "first"}),
        );
        assert!(write.matches(
            "write",
            &serde_json::json!({"file_path": "src/lib.rs", "content": "second"})
        ));
        assert!(!write.matches(
            "write",
            &serde_json::json!({"file_path": "src/main.rs", "content": "second"})
        ));
    }

    #[test]
    fn generated_acl_round_trips_and_deduplicates_exact_rules() {
        let grants = vec![
            ExactPermissionGrant::from_invocation(
                "bash",
                &serde_json::json!({"command": "cargo test"}),
            ),
            ExactPermissionGrant::from_invocation(
                "bash",
                &serde_json::json!({"command": "cargo test"}),
            ),
            ExactPermissionGrant::from_invocation(
                "write",
                &serde_json::json!({"file_path": "src/lib.rs", "content": "ignored"}),
            ),
        ];
        let source = generate_project_permission_grants(&grants).unwrap();
        assert!(source.contains("permissions {"));
        assert!(source.contains("allow \"bash\""));
        assert!(!source.contains("ignored"));
        assert_eq!(parse_project_permission_grants(&source).unwrap().len(), 2);
    }

    #[test]
    fn project_rule_write_is_atomic_and_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let path = project_permission_rules_path(temp.path());
        let grant = ExactPermissionGrant::from_invocation(
            "edit",
            &serde_json::json!({
                "file_path": "src/lib.rs",
                "old_string": "a",
                "new_string": "b"
            }),
        );
        persist_project_permission_grant(&path, grant.clone()).unwrap();
        persist_project_permission_grant(&path, grant).unwrap();

        let loaded = load_project_permission_grants(&path).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].matches(
            "edit",
            &serde_json::json!({
                "file_path": "src/lib.rs",
                "old_string": "different",
                "new_string": "content"
            })
        ));
        let leftovers = std::fs::read_dir(path.parent().unwrap())
            .unwrap()
            .filter_map(Result::ok)
            .filter(|entry| entry.path() != path)
            .collect::<Vec<_>>();
        assert!(leftovers.is_empty(), "{leftovers:?}");
    }

    #[test]
    fn snapshots_keep_scopes_separate_and_session_revocation_is_exact() {
        let project = ExactPermissionGrant::from_invocation(
            "bash",
            &serde_json::json!({"command": "cargo test"}),
        );
        let retained = ExactPermissionGrant::from_invocation(
            "write",
            &serde_json::json!({"file_path": "README.md", "content": "first"}),
        );
        let revoked = ExactPermissionGrant::from_invocation(
            "edit",
            &serde_json::json!({"file_path": "src/lib.rs", "new_string": "second"}),
        );
        let grants = TuiPermissionGrants::with_project(vec![project.clone()]);
        grants.allow_for_session(retained.clone());
        grants.allow_for_session(revoked.clone());

        let before = grants.snapshot();
        assert_eq!(before.project, vec![project]);
        assert_eq!(before.session, vec![retained.clone(), revoked.clone()]);

        assert!(grants.revoke_session(&revoked.stable_key()));
        assert!(!grants.revoke_session(&revoked.stable_key()));
        let after = grants.snapshot();
        assert_eq!(after.session, vec![retained]);
        assert!(after.project.len() == 1);

        let project_key = after.project[0].stable_key();
        assert!(grants.revoke_project(&project_key));
        assert!(grants.snapshot().project.is_empty());
    }

    #[test]
    fn project_rule_revocation_rewrites_acl_and_reports_missing_rules() {
        let temp = tempfile::tempdir().unwrap();
        let path = project_permission_rules_path(temp.path());
        let retained = ExactPermissionGrant::from_invocation(
            "bash",
            &serde_json::json!({"command": "cargo check"}),
        );
        let revoked = ExactPermissionGrant::from_invocation(
            "edit",
            &serde_json::json!({"file_path": "README.md", "new_string": "updated"}),
        );
        persist_project_permission_grant(&path, retained.clone()).unwrap();
        persist_project_permission_grant(&path, revoked.clone()).unwrap();

        let result = revoke_project_permission_grant(&path, &revoked.stable_key()).unwrap();
        assert!(result.removed);
        assert_eq!(result.path, path);
        assert_eq!(result.grants, vec![retained.clone()]);
        assert_eq!(
            load_project_permission_grants(&result.path).unwrap(),
            vec![retained.clone()]
        );

        let missing = revoke_project_permission_grant(&result.path, &revoked.stable_key()).unwrap();
        assert!(!missing.removed);
        assert_eq!(missing.grants, vec![retained]);
    }

    #[cfg(unix)]
    #[test]
    fn project_rule_write_refuses_symbolic_link_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target.acl");
        std::fs::write(&target, "sentinel").unwrap();
        let path = project_permission_rules_path(temp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&target, &path).unwrap();

        let error = persist_project_permission_grant(
            &path,
            ExactPermissionGrant::from_invocation(
                "bash",
                &serde_json::json!({"command": "cargo test"}),
            ),
        )
        .unwrap_err();
        assert!(error.contains("symbolic"), "{error}");
        assert_eq!(std::fs::read_to_string(target).unwrap(), "sentinel");
    }

    #[cfg(unix)]
    #[test]
    fn project_rule_revocation_refuses_symbolic_link_target() {
        use std::os::unix::fs::symlink;

        let temp = tempfile::tempdir().unwrap();
        let target = temp.path().join("target.acl");
        let grant = ExactPermissionGrant::from_invocation(
            "bash",
            &serde_json::json!({"command": "cargo test"}),
        );
        std::fs::write(
            &target,
            generate_project_permission_grants(std::slice::from_ref(&grant)).unwrap(),
        )
        .unwrap();
        let path = project_permission_rules_path(temp.path());
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        symlink(&target, &path).unwrap();

        let error = revoke_project_permission_grant(&path, &grant.stable_key()).unwrap_err();
        assert!(error.contains("symbolic"), "{error}");
        assert_eq!(
            parse_project_permission_grants(&std::fs::read_to_string(target).unwrap()).unwrap(),
            vec![grant]
        );
    }
}
