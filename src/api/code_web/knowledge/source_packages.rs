//! Safe, deterministic source packages for personal knowledge bases.

use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsStr;
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::personal_bases::{self, KnowledgeStoreError};
use super::{compilation, personal_bases::KnowledgeBaseMutation};
use source_set::{read_source_set, write_source_set};

mod source_set;

pub(super) const SOURCE_SNAPSHOT_PATH: &str = ".a3s/source-snapshot.json";
const SOURCE_SNAPSHOT_SCHEMA: u32 = 1;
const MAX_SOURCE_FILES: usize = 20_000;
const MAX_SOURCE_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSelectionPreview {
    pub(super) schema_version: u32,
    pub(super) selected_count: usize,
    pub(super) source_root_count: usize,
    pub(super) deduplicated_count: usize,
    pub(super) file_count: usize,
    pub(super) directory_count: usize,
    pub(super) bytes: u64,
    pub(super) content_digest: String,
    pub(super) suggested_name: String,
    pub(super) items: Vec<SourceSelectionItem>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSelectionItem {
    pub(super) path: String,
    pub(super) destination: String,
    pub(super) kind: SourceRootKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum SourceRootKind {
    File,
    Directory,
}

impl SourceRootKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Directory => "directory",
        }
    }

    fn parse(value: &str) -> Result<Self, KnowledgeStoreError> {
        match value {
            "file" => Ok(Self::File),
            "directory" => Ok(Self::Directory),
            _ => Err(KnowledgeStoreError::Invalid(format!(
                "unsupported knowledge source kind `{value}`"
            ))),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SourceSelectionPlan {
    roots: Vec<SourceRoot>,
    pub(super) snapshot: SourceSnapshot,
    pub(super) unstable_paths: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SourceRoot {
    path: PathBuf,
    destination: PathBuf,
    kind: SourceRootKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceSnapshot {
    pub(super) schema_version: u32,
    pub(super) content_digest: String,
    pub(super) file_count: usize,
    pub(super) directory_count: usize,
    pub(super) bytes: u64,
    pub(super) files: BTreeMap<String, SourceFileSnapshot>,
    pub(super) directories: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceFileSnapshot {
    pub(super) origin_path: String,
    pub(super) size: u64,
    pub(super) modified_micros: u64,
    pub(super) content_digest: String,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SourceChanges {
    pub(super) changed: bool,
    pub(super) added: usize,
    pub(super) modified: usize,
    pub(super) deleted: usize,
    pub(super) renamed: usize,
    pub(super) total_changed: usize,
    pub(super) previous_files: usize,
    pub(super) current_files: usize,
    pub(super) current_bytes: u64,
    pub(super) unstable_paths: Vec<String>,
    pub(super) safety_pause: Option<String>,
}

pub(super) fn preview_source_selection(
    workspace: &Path,
    paths: &[PathBuf],
) -> Result<SourceSelectionPreview, KnowledgeStoreError> {
    let selected_count = paths.len();
    let plan = plan_source_selection(workspace, paths, None, Duration::ZERO, SystemTime::now())?;
    Ok(plan.preview(selected_count))
}

pub(super) fn create_from_selection(
    workspace: &Path,
    paths: &[PathBuf],
    name: &str,
    description: Option<&str>,
    policy: compilation::CompilationPolicy,
) -> Result<KnowledgeBaseMutation, KnowledgeStoreError> {
    let plan = plan_source_selection(workspace, paths, None, Duration::ZERO, SystemTime::now())?;
    personal_bases::materialize_personal_base(
        workspace,
        name,
        description,
        "selection",
        |staging| {
            materialize_source_package(staging, &plan)?;
            compilation::initialize_for_source_package(
                staging,
                &plan.snapshot.content_digest,
                policy,
                chrono::Utc::now(),
            )
        },
    )
}

pub(super) fn plan_source_selection(
    workspace: &Path,
    paths: &[PathBuf],
    previous: Option<&SourceSnapshot>,
    stable_window: Duration,
    now: SystemTime,
) -> Result<SourceSelectionPlan, KnowledgeStoreError> {
    if paths.is_empty() {
        return Err(KnowledgeStoreError::Invalid(
            "select at least one file or folder for the knowledge base".to_string(),
        ));
    }
    let workspace = canonical_regular_directory(workspace, "workspace")?;
    let managed_root = personal_bases::bases_root(&workspace);
    let roots = normalize_selected_roots(&workspace, &managed_root, paths)?;
    build_plan(roots, previous, stable_window, now)
}

pub(super) fn scan_base_sources(
    base: &Path,
    previous: Option<&SourceSnapshot>,
    stable_window: Duration,
    now: SystemTime,
) -> Result<SourceSelectionPlan, KnowledgeStoreError> {
    let roots = match read_source_set(base)? {
        Some(roots) => roots,
        None => {
            let sources = base.join("sources");
            let metadata = std::fs::symlink_metadata(&sources).map_err(|error| {
                if error.kind() == std::io::ErrorKind::NotFound {
                    KnowledgeStoreError::NotFound(format!(
                        "knowledge source root is unavailable: {}",
                        sources.display()
                    ))
                } else {
                    io_error(&sources, error)
                }
            })?;
            if metadata.file_type().is_symlink() || !metadata.is_dir() {
                return Err(KnowledgeStoreError::Invalid(format!(
                    "knowledge source root must be a regular directory: {}",
                    sources.display()
                )));
            }
            vec![SourceRoot {
                path: sources,
                destination: PathBuf::new(),
                kind: SourceRootKind::Directory,
            }]
        }
    };
    build_plan(roots, previous, stable_window, now)
}

pub(super) fn materialize_source_package(
    staging: &Path,
    plan: &SourceSelectionPlan,
) -> Result<(), KnowledgeStoreError> {
    let sources = staging.join("sources");
    copy_plan_files(&sources, plan)?;
    write_source_set(staging, &plan.roots)?;
    write_snapshot(staging, &plan.snapshot)
}

pub(super) fn sync_source_package(
    base: &Path,
    plan: &SourceSelectionPlan,
) -> Result<(), KnowledgeStoreError> {
    if read_source_set(base)?.is_none() {
        return write_snapshot(base, &plan.snapshot);
    }
    let staging = base.join(format!(
        ".sources.tmp-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    let backup = base.join(format!(
        ".sources.backup-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    let target = base.join("sources");
    let outcome = (|| {
        copy_plan_files(&staging, plan)?;
        if target.exists() {
            std::fs::rename(&target, &backup).map_err(|error| io_error(&target, error))?;
        }
        if let Err(error) = std::fs::rename(&staging, &target) {
            if backup.exists() {
                let _ = std::fs::rename(&backup, &target);
            }
            return Err(io_error(&target, error));
        }
        if backup.exists() {
            std::fs::remove_dir_all(&backup).map_err(|error| io_error(&backup, error))?;
        }
        write_snapshot(base, &plan.snapshot)
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    outcome
}

pub(super) fn load_snapshot(base: &Path) -> Result<Option<SourceSnapshot>, KnowledgeStoreError> {
    let path = base.join(SOURCE_SNAPSHOT_PATH);
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(io_error(&path, error)),
    };
    let snapshot: SourceSnapshot = serde_json::from_slice(&bytes).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("invalid {}: {error}", path.display()))
    })?;
    if snapshot.schema_version != SOURCE_SNAPSHOT_SCHEMA {
        return Err(KnowledgeStoreError::Invalid(format!(
            "unsupported source snapshot schema {}",
            snapshot.schema_version
        )));
    }
    Ok(Some(snapshot))
}

pub(super) fn write_snapshot(
    base: &Path,
    snapshot: &SourceSnapshot,
) -> Result<(), KnowledgeStoreError> {
    let path = base.join(SOURCE_SNAPSHOT_PATH);
    let bytes = serde_json::to_vec_pretty(snapshot).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("could not encode source snapshot: {error}"))
    })?;
    atomic_write(&path, &bytes)
}

pub(super) fn source_changes(
    previous: Option<&SourceSnapshot>,
    current: &SourceSelectionPlan,
) -> SourceChanges {
    let Some(previous) = previous else {
        let total_changed = current.snapshot.file_count;
        return SourceChanges {
            changed: total_changed > 0,
            added: total_changed,
            total_changed,
            current_files: current.snapshot.file_count,
            current_bytes: current.snapshot.bytes,
            unstable_paths: current.unstable_paths.clone(),
            ..SourceChanges::default()
        };
    };

    let mut added_paths = Vec::new();
    let mut deleted_paths = Vec::new();
    let mut modified = 0usize;
    for (path, file) in &current.snapshot.files {
        match previous.files.get(path) {
            None => added_paths.push(path),
            Some(old) if old.content_digest != file.content_digest => modified += 1,
            Some(_) => {}
        }
    }
    for path in previous.files.keys() {
        if !current.snapshot.files.contains_key(path) {
            deleted_paths.push(path);
        }
    }

    let mut deleted_by_digest = HashMap::<&str, usize>::new();
    for path in &deleted_paths {
        if let Some(file) = previous.files.get(*path) {
            *deleted_by_digest.entry(&file.content_digest).or_default() += 1;
        }
    }
    let mut renamed = 0usize;
    for path in &added_paths {
        let Some(file) = current.snapshot.files.get(*path) else {
            continue;
        };
        let Some(count) = deleted_by_digest.get_mut(file.content_digest.as_str()) else {
            continue;
        };
        if *count > 0 {
            *count -= 1;
            renamed += 1;
        }
    }
    let added = added_paths.len().saturating_sub(renamed);
    let deleted = deleted_paths.len().saturating_sub(renamed);
    let total_changed = added + deleted + modified + renamed;
    let safety_pause = safety_pause_reason(previous.file_count, deleted_paths.len(), total_changed);
    SourceChanges {
        changed: previous.content_digest != current.snapshot.content_digest,
        added,
        modified,
        deleted,
        renamed,
        total_changed,
        previous_files: previous.file_count,
        current_files: current.snapshot.file_count,
        current_bytes: current.snapshot.bytes,
        unstable_paths: current.unstable_paths.clone(),
        safety_pause,
    }
}

fn safety_pause_reason(previous_files: usize, deleted: usize, changed: usize) -> Option<String> {
    if previous_files > 0
        && (deleted > 100 || (deleted >= 20 && deleted.saturating_mul(2) > previous_files))
    {
        return Some(format!(
            "automatic compilation paused because {deleted} of {previous_files} source files disappeared"
        ));
    }
    if previous_files > 0
        && (changed > 1_000 || (changed >= 200 && changed.saturating_mul(2) > previous_files))
    {
        return Some(format!(
            "automatic compilation paused because {changed} of {previous_files} source files changed"
        ));
    }
    None
}

impl SourceSelectionPlan {
    fn preview(&self, selected_count: usize) -> SourceSelectionPreview {
        let suggested_name = suggested_name(&self.roots);
        SourceSelectionPreview {
            schema_version: 1,
            selected_count,
            source_root_count: self.roots.len(),
            deduplicated_count: selected_count.saturating_sub(self.roots.len()),
            file_count: self.snapshot.file_count,
            directory_count: self.snapshot.directory_count,
            bytes: self.snapshot.bytes,
            content_digest: self.snapshot.content_digest.clone(),
            suggested_name,
            items: self
                .roots
                .iter()
                .map(|root| SourceSelectionItem {
                    path: root.path.display().to_string(),
                    destination: Path::new("sources")
                        .join(&root.destination)
                        .display()
                        .to_string(),
                    kind: root.kind,
                })
                .collect(),
        }
    }
}

fn normalize_selected_roots(
    workspace: &Path,
    managed_root: &Path,
    paths: &[PathBuf],
) -> Result<Vec<SourceRoot>, KnowledgeStoreError> {
    let mut candidates = Vec::<(usize, PathBuf, SourceRootKind)>::new();
    let mut seen = HashSet::new();
    for (index, requested) in paths.iter().enumerate() {
        let metadata = std::fs::symlink_metadata(requested).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                KnowledgeStoreError::NotFound(format!(
                    "selected knowledge source was not found: {}",
                    requested.display()
                ))
            } else {
                io_error(requested, error)
            }
        })?;
        if metadata.file_type().is_symlink() || (!metadata.is_file() && !metadata.is_dir()) {
            return Err(KnowledgeStoreError::Invalid(format!(
                "knowledge sources must be regular files or directories: {}",
                requested.display()
            )));
        }
        let canonical =
            std::fs::canonicalize(requested).map_err(|error| io_error(requested, error))?;
        if !canonical.starts_with(workspace) {
            return Err(KnowledgeStoreError::Invalid(format!(
                "knowledge source must stay inside the selected workspace: {}",
                canonical.display()
            )));
        }
        if canonical.starts_with(managed_root) {
            return Err(KnowledgeStoreError::Invalid(
                "a managed A3S knowledge directory cannot be packaged as its own source"
                    .to_string(),
            ));
        }
        if seen.insert(canonical.clone()) {
            candidates.push((
                index,
                canonical,
                if metadata.is_dir() {
                    SourceRootKind::Directory
                } else {
                    SourceRootKind::File
                },
            ));
        }
    }
    candidates.sort_by(|left, right| {
        path_depth(&left.1)
            .cmp(&path_depth(&right.1))
            .then_with(|| left.0.cmp(&right.0))
    });
    let mut retained = Vec::<(usize, PathBuf, SourceRootKind)>::new();
    for candidate in candidates {
        if retained.iter().any(|(_, parent, kind)| {
            *kind == SourceRootKind::Directory && candidate.1.starts_with(parent)
        }) {
            continue;
        }
        retained.push(candidate);
    }
    retained.sort_by_key(|candidate| candidate.0);

    let mut reserved = HashSet::new();
    let roots = retained
        .into_iter()
        .map(|(_, path, kind)| {
            let requested = path
                .file_name()
                .and_then(OsStr::to_str)
                .filter(|name| !name.trim().is_empty())
                .unwrap_or("source");
            let destination = unique_destination_name(requested, kind, &mut reserved);
            SourceRoot {
                path,
                destination: PathBuf::from(destination),
                kind,
            }
        })
        .collect::<Vec<_>>();
    if roots.is_empty() {
        return Err(KnowledgeStoreError::Invalid(
            "the selection contains no importable source roots".to_string(),
        ));
    }
    Ok(roots)
}

fn build_plan(
    roots: Vec<SourceRoot>,
    previous: Option<&SourceSnapshot>,
    stable_window: Duration,
    now: SystemTime,
) -> Result<SourceSelectionPlan, KnowledgeStoreError> {
    let mut snapshot = SourceSnapshot {
        schema_version: SOURCE_SNAPSHOT_SCHEMA,
        ..SourceSnapshot::default()
    };
    let mut unstable_paths = Vec::new();
    for root in &roots {
        let metadata = std::fs::symlink_metadata(&root.path).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                KnowledgeStoreError::NotFound(format!(
                    "knowledge source root is unavailable: {}",
                    root.path.display()
                ))
            } else {
                io_error(&root.path, error)
            }
        })?;
        if metadata.file_type().is_symlink() {
            return Err(KnowledgeStoreError::Invalid(format!(
                "knowledge source root cannot be a symbolic link: {}",
                root.path.display()
            )));
        }
        match root.kind {
            SourceRootKind::File => collect_file(
                &root.path,
                &root.destination,
                &metadata,
                previous,
                stable_window,
                now,
                &mut snapshot,
                &mut unstable_paths,
            )?,
            SourceRootKind::Directory => collect_directory(
                &root.path,
                &root.destination,
                previous,
                stable_window,
                now,
                &mut snapshot,
                &mut unstable_paths,
            )?,
        }
    }
    if snapshot.files.is_empty() {
        return Err(KnowledgeStoreError::Invalid(
            "the selected sources contain no importable files".to_string(),
        ));
    }
    snapshot.file_count = snapshot.files.len();
    snapshot.directory_count = snapshot.directories.len();
    snapshot.content_digest = snapshot_digest(&snapshot);
    Ok(SourceSelectionPlan {
        roots,
        snapshot,
        unstable_paths,
    })
}

#[allow(clippy::too_many_arguments)]
fn collect_directory(
    source: &Path,
    logical: &Path,
    previous: Option<&SourceSnapshot>,
    stable_window: Duration,
    now: SystemTime,
    snapshot: &mut SourceSnapshot,
    unstable_paths: &mut Vec<String>,
) -> Result<(), KnowledgeStoreError> {
    if !logical.as_os_str().is_empty() {
        snapshot
            .directories
            .push(normalized_relative_path(logical)?);
    }
    let mut entries = std::fs::read_dir(source)
        .map_err(|error| io_error(source, error))?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| io_error(source, error))?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let name = entry.file_name();
        if should_skip_source_entry(&name) {
            continue;
        }
        let source_path = entry.path();
        let metadata = std::fs::symlink_metadata(&source_path)
            .map_err(|error| io_error(&source_path, error))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let logical_path = logical.join(&name);
        if metadata.is_dir() {
            collect_directory(
                &source_path,
                &logical_path,
                previous,
                stable_window,
                now,
                snapshot,
                unstable_paths,
            )?;
        } else if metadata.is_file() {
            collect_file(
                &source_path,
                &logical_path,
                &metadata,
                previous,
                stable_window,
                now,
                snapshot,
                unstable_paths,
            )?;
        }
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn collect_file(
    source: &Path,
    logical: &Path,
    metadata: &std::fs::Metadata,
    previous: Option<&SourceSnapshot>,
    stable_window: Duration,
    now: SystemTime,
    snapshot: &mut SourceSnapshot,
    unstable_paths: &mut Vec<String>,
) -> Result<(), KnowledgeStoreError> {
    snapshot.bytes = snapshot.bytes.saturating_add(metadata.len());
    if snapshot.files.len() >= MAX_SOURCE_FILES || snapshot.bytes > MAX_SOURCE_BYTES {
        return Err(KnowledgeStoreError::Invalid(format!(
            "knowledge sources exceed the limit of {MAX_SOURCE_FILES} files or {MAX_SOURCE_BYTES} bytes"
        )));
    }
    let logical = normalized_relative_path(logical)?;
    let modified = metadata.modified().unwrap_or(UNIX_EPOCH);
    if stable_window > Duration::ZERO
        && now.duration_since(modified).unwrap_or_default() < stable_window
    {
        unstable_paths.push(source.display().to_string());
    }
    let modified_micros = system_time_micros(modified);
    let origin_path = source.display().to_string();
    let reusable = previous
        .and_then(|snapshot| snapshot.files.get(&logical))
        .filter(|file| {
            file.origin_path == origin_path
                && file.size == metadata.len()
                && file.modified_micros == modified_micros
        });
    let content_digest = match reusable {
        Some(file) => file.content_digest.clone(),
        None => hash_file(source)?,
    };
    snapshot.files.insert(
        logical,
        SourceFileSnapshot {
            origin_path,
            size: metadata.len(),
            modified_micros,
            content_digest,
        },
    );
    Ok(())
}

fn copy_plan_files(
    destination: &Path,
    plan: &SourceSelectionPlan,
) -> Result<(), KnowledgeStoreError> {
    std::fs::create_dir_all(destination).map_err(|error| io_error(destination, error))?;
    for directory in &plan.snapshot.directories {
        let relative = safe_relative_path(directory)?;
        let target = destination.join(relative);
        std::fs::create_dir_all(&target).map_err(|error| io_error(&target, error))?;
    }
    for (relative, source) in &plan.snapshot.files {
        let relative = safe_relative_path(relative)?;
        let target = destination.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        std::fs::copy(&source.origin_path, &target).map_err(|error| io_error(&target, error))?;
        let copied_digest = hash_file(&target)?;
        if copied_digest != source.content_digest {
            return Err(KnowledgeStoreError::Conflict(format!(
                "knowledge source changed while it was being packaged: {}",
                source.origin_path
            )));
        }
    }
    Ok(())
}

fn suggested_name(roots: &[SourceRoot]) -> String {
    let first = roots
        .first()
        .and_then(|root| root.path.file_name())
        .and_then(OsStr::to_str)
        .filter(|value| !value.trim().is_empty())
        .unwrap_or("Knowledge");
    if roots.len() == 1 {
        return first.chars().take(80).collect();
    }
    format!("{first} and {} more", roots.len() - 1)
        .chars()
        .take(80)
        .collect()
}

fn unique_destination_name(
    requested: &str,
    kind: SourceRootKind,
    reserved: &mut HashSet<String>,
) -> String {
    for index in 0..1_000 {
        let candidate = if index == 0 {
            requested.to_string()
        } else {
            duplicate_name(requested, index + 1, kind)
        };
        if reserved.insert(candidate.to_lowercase()) {
            return candidate;
        }
    }
    format!("source-{}", reserved.len() + 1)
}

fn duplicate_name(name: &str, index: usize, kind: SourceRootKind) -> String {
    if kind == SourceRootKind::Directory {
        return format!("{name} ({index})");
    }
    let path = Path::new(name);
    let stem = path.file_stem().and_then(OsStr::to_str).unwrap_or(name);
    match path.extension().and_then(OsStr::to_str) {
        Some(extension) => format!("{stem} ({index}).{extension}"),
        None => format!("{stem} ({index})"),
    }
}

fn should_skip_source_entry(name: &OsStr) -> bool {
    let Some(name) = name.to_str() else {
        return true;
    };
    if matches!(
        name,
        ".a3s"
            | ".git"
            | ".obsidian"
            | ".trash"
            | ".cache"
            | "__pycache__"
            | "node_modules"
            | "target"
            | "wiki"
            | "eval"
            | ".DS_Store"
            | ".tantivy"
    ) {
        return true;
    }
    let lower = name.to_ascii_lowercase();
    name.starts_with("~$")
        || lower.ends_with(".tmp")
        || lower.ends_with(".temp")
        || lower.ends_with(".swp")
        || lower.ends_with(".part")
}

fn snapshot_digest(snapshot: &SourceSnapshot) -> String {
    let mut digest = Sha256::new();
    digest.update(b"a3s-knowledge-source-snapshot-v1\0");
    for (path, file) in &snapshot.files {
        hash_field(&mut digest, path.as_bytes());
        hash_field(&mut digest, file.content_digest.as_bytes());
        hash_field(&mut digest, &file.size.to_le_bytes());
    }
    format!("sha256:{:x}", digest.finalize())
}

fn hash_file(path: &Path) -> Result<String, KnowledgeStoreError> {
    let mut file = File::open(path).map_err(|error| io_error(path, error))?;
    let mut digest = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let count = file
            .read(&mut buffer)
            .map_err(|error| io_error(path, error))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
    }
    Ok(format!("sha256:{:x}", digest.finalize()))
}

fn hash_field(digest: &mut Sha256, value: &[u8]) {
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

fn normalized_relative_path(path: &Path) -> Result<String, KnowledgeStoreError> {
    if path.as_os_str().is_empty() {
        return Ok(String::new());
    }
    if path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(KnowledgeStoreError::Invalid(format!(
            "invalid knowledge source destination `{}`",
            path.display()
        )));
    }
    Ok(path
        .components()
        .filter_map(|component| match component {
            Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect::<Vec<_>>()
        .join("/"))
}

fn safe_relative_path(value: &str) -> Result<PathBuf, KnowledgeStoreError> {
    let path = Path::new(value);
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || !path
            .components()
            .all(|component| matches!(component, Component::Normal(_)))
    {
        return Err(KnowledgeStoreError::Invalid(format!(
            "invalid knowledge source destination `{value}`"
        )));
    }
    Ok(path.to_path_buf())
}

fn canonical_regular_directory(path: &Path, label: &str) -> Result<PathBuf, KnowledgeStoreError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| io_error(path, error))?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() {
        return Err(KnowledgeStoreError::Invalid(format!(
            "{label} must be a regular directory: {}",
            path.display()
        )));
    }
    std::fs::canonicalize(path).map_err(|error| io_error(path, error))
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), KnowledgeStoreError> {
    let parent = path.parent().ok_or_else(|| {
        KnowledgeStoreError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let temporary = parent.join(format!(
        ".source-state.tmp-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    std::fs::write(&temporary, content).map_err(|error| io_error(&temporary, error))?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(io_error(path, error));
    }
    Ok(())
}

fn io_error(path: &Path, error: std::io::Error) -> KnowledgeStoreError {
    KnowledgeStoreError::Io(format!("{}: {error}", path.display()))
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn system_time_micros(value: SystemTime) -> u64 {
    value
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_micros().min(u128::from(u64::MAX)) as u64)
        .unwrap_or_default()
}

fn path_depth(path: &Path) -> usize {
    path.components().count()
}

#[cfg(test)]
mod tests;
