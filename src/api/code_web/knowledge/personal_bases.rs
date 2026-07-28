//! Filesystem-backed personal knowledge-base catalog.

use std::collections::HashMap;
use std::path::{Component, Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_acl::{Block, Document, Value as AclValue};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sha2::{Digest, Sha256};
use thiserror::Error;

use super::compilation::{self, CompilationSummary};
use super::marketplace::MarketPackage;
use crate::tui::asset_lifecycle::{self, AssetAclDocument, OsService, RuntimeBindingIntent};

const MANIFEST_SCHEMA: &str = "a3s.knowledge-base.v1";
const MANIFEST_PATH: &str = ".a3s/knowledge-base.acl";
const MAX_NAME_CHARS: usize = 80;
const MAX_DESCRIPTION_CHARS: usize = 280;
const MAX_IMPORT_FILES: usize = 20_000;
const MAX_IMPORT_BYTES: u64 = 1024 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct KnowledgeBase {
    pub(super) id: String,
    pub(super) name: String,
    pub(super) description: String,
    pub(super) origin: String,
    pub(super) marketplace_id: Option<String>,
    pub(super) version: String,
    pub(super) pinned: bool,
    pub(super) created_at: String,
    pub(super) updated_at: String,
    pub(super) path: String,
    pub(super) source_count: usize,
    pub(super) concept_count: usize,
    pub(super) bytes: u64,
    pub(super) compilation: CompilationSummary,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct KnowledgeBaseList {
    pub(super) items: Vec<KnowledgeBase>,
    pub(super) warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct KnowledgeBaseMutation {
    pub(super) changed: bool,
    pub(super) knowledge_base: KnowledgeBase,
}

#[derive(Debug, Error)]
pub(super) enum KnowledgeStoreError {
    #[error("{0}")]
    Invalid(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    Conflict(String),
    #[error("{0}")]
    Io(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KnowledgeBaseManifest {
    id: String,
    name: String,
    description: String,
    origin: String,
    marketplace_id: Option<String>,
    version: String,
    pinned: bool,
    created_at: String,
    updated_at: String,
}

pub(super) fn materialize_personal_base<F>(
    workspace: &Path,
    name: &str,
    description: Option<&str>,
    origin: &str,
    initialize: F,
) -> Result<KnowledgeBaseMutation, KnowledgeStoreError>
where
    F: FnOnce(&Path) -> Result<(), KnowledgeStoreError>,
{
    if !matches!(origin, "created" | "imported" | "selection") {
        return Err(KnowledgeStoreError::Invalid(format!(
            "unsupported personal knowledge-base origin `{origin}`"
        )));
    }
    let name = normalize_required_text(name, "knowledge-base name", MAX_NAME_CHARS)?;
    let description = normalize_optional_text(description, MAX_DESCRIPTION_CHARS)
        .unwrap_or_else(|| "Personal knowledge created in A3S Web.".to_string());
    let id = knowledge_base_id(&name);
    validate_base_id(&id)?;
    let now = Utc::now().to_rfc3339();
    let manifest = KnowledgeBaseManifest {
        id: id.clone(),
        name: name.clone(),
        description: description.clone(),
        origin: origin.to_string(),
        marketplace_id: None,
        version: "1.0.0".to_string(),
        pinned: true,
        created_at: now.clone(),
        updated_at: now,
    };
    let root = bases_root(workspace);
    std::fs::create_dir_all(&root).map_err(|error| io_error(&root, error))?;
    let target = root.join(&id);
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(KnowledgeStoreError::Conflict(format!(
            "knowledge base `{id}` already exists"
        )));
    }
    let staging = root.join(format!(
        ".{id}.tmp-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    std::fs::create_dir(&staging).map_err(|error| io_error(&staging, error))?;
    let files = vec![
        (
            "README.md".to_string(),
            format!("# {name}\n\n{description}\n"),
        ),
        (
            "eval/smoke.md".to_string(),
            "# Compilation Check\n\n1. Confirm every concept links to a source.\n2. Record unresolved conflicts and freshness limits.\n"
                .to_string(),
        ),
    ];
    let outcome = initialize(&staging)
        .and_then(|()| materialize_base_in_staging(&staging, &manifest, &files))
        .and_then(|()| {
            std::fs::rename(&staging, &target).map_err(|error| io_error(&target, error))
        });
    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    outcome?;
    Ok(KnowledgeBaseMutation {
        changed: true,
        knowledge_base: knowledge_base_from_directory(&target, &id)?,
    })
}

pub(super) fn bases_root(workspace: &Path) -> PathBuf {
    workspace.join(".a3s").join("kb").join("bases")
}

pub(super) fn list_knowledge_bases(workspace: &Path) -> KnowledgeBaseList {
    let mut result = KnowledgeBaseList::default();
    if let Some(legacy) = legacy_workspace_base(workspace) {
        result.items.push(legacy);
    }

    let root = bases_root(workspace);
    let entries = match std::fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return result,
        Err(error) => {
            result
                .warnings
                .push(format!("could not read {}: {error}", root.display()));
            return result;
        }
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let id = entry.file_name().to_string_lossy().into_owned();
        if id.starts_with('.') || !is_regular_directory(&path) {
            continue;
        }
        match knowledge_base_from_directory(&path, &id) {
            Ok(base) => result.items.push(base),
            Err(error) => result
                .warnings
                .push(format!("ignored knowledge base `{id}`: {error}")),
        }
    }
    result.items.sort_by(|left, right| {
        right
            .pinned
            .cmp(&left.pinned)
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.id.cmp(&right.id))
    });
    result
}

pub(super) fn create_knowledge_base(
    workspace: &Path,
    name: &str,
    description: Option<&str>,
) -> Result<KnowledgeBaseMutation, KnowledgeStoreError> {
    let name = normalize_required_text(name, "knowledge-base name", MAX_NAME_CHARS)?;
    let description = normalize_optional_text(description, MAX_DESCRIPTION_CHARS)
        .unwrap_or_else(|| "Personal knowledge created in A3S Web.".to_string());
    let id = knowledge_base_id(&name);
    validate_base_id(&id)?;
    let now = Utc::now().to_rfc3339();
    let manifest = KnowledgeBaseManifest {
        id: id.clone(),
        name: name.clone(),
        description: description.clone(),
        origin: "created".to_string(),
        marketplace_id: None,
        version: "1.0.0".to_string(),
        pinned: true,
        created_at: now.clone(),
        updated_at: now,
    };
    let files = vec![
        (
            "README.md".to_string(),
            format!("# {name}\n\n{description}\n"),
        ),
        (
            "sources/welcome.md".to_string(),
            "# Welcome\n\nAdd source material here or import it through A3S.\n".to_string(),
        ),
        (
            "wiki/index.md".to_string(),
            format!("# {name}\n\nThis index grows with the concepts in this knowledge base.\n"),
        ),
        (
            "eval/smoke.md".to_string(),
            "# Smoke Evaluation\n\n1. Confirm each concept links to a source.\n2. Record unresolved conflicts and freshness limits.\n".to_string(),
        ),
    ];
    materialize_base(workspace, &manifest, &files)?;
    Ok(KnowledgeBaseMutation {
        changed: true,
        knowledge_base: knowledge_base_from_directory(&bases_root(workspace).join(&id), &id)?,
    })
}

pub(super) fn import_knowledge_base(
    workspace: &Path,
    source: &Path,
    requested_name: Option<&str>,
) -> Result<KnowledgeBaseMutation, KnowledgeStoreError> {
    let source_metadata = std::fs::symlink_metadata(source).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KnowledgeStoreError::NotFound(format!(
                "knowledge import directory was not found: {}",
                source.display()
            ))
        } else {
            io_error(source, error)
        }
    })?;
    if source_metadata.file_type().is_symlink() || !source_metadata.is_dir() {
        return Err(KnowledgeStoreError::Invalid(format!(
            "knowledge import path must be a regular directory: {}",
            source.display()
        )));
    }
    let source = std::fs::canonicalize(source).map_err(|error| io_error(source, error))?;
    let managed_root = bases_root(workspace);
    let workspace_root =
        std::fs::canonicalize(workspace).unwrap_or_else(|_| workspace.to_path_buf());
    if managed_root.starts_with(&source)
        || source.starts_with(&managed_root)
        || workspace_root == source
    {
        return Err(KnowledgeStoreError::Invalid(
            "a managed A3S knowledge directory cannot be imported into itself".to_string(),
        ));
    }

    let fallback_name = source
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("Imported Knowledge");
    let name = normalize_required_text(
        requested_name.unwrap_or(fallback_name),
        "knowledge-base name",
        MAX_NAME_CHARS,
    )?;
    let id = knowledge_base_id(&name);
    validate_base_id(&id)?;
    let now = Utc::now().to_rfc3339();
    let manifest = KnowledgeBaseManifest {
        id: id.clone(),
        name: name.clone(),
        description: format!("Imported from the local folder `{fallback_name}`."),
        origin: "imported".to_string(),
        marketplace_id: None,
        version: "1.0.0".to_string(),
        pinned: true,
        created_at: now.clone(),
        updated_at: now,
    };

    std::fs::create_dir_all(&managed_root).map_err(|error| io_error(&managed_root, error))?;
    let target = managed_root.join(&id);
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(KnowledgeStoreError::Conflict(format!(
            "knowledge base `{id}` already exists"
        )));
    }
    let staging = managed_root.join(format!(
        ".{id}.tmp-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    std::fs::create_dir(&staging).map_err(|error| io_error(&staging, error))?;
    let outcome = (|| {
        let mut budget = ImportBudget::default();
        copy_import_directory(&source, &staging.join("sources"), &mut budget)?;
        if budget.files == 0 {
            return Err(KnowledgeStoreError::Invalid(
                "the selected knowledge directory contains no importable files".to_string(),
            ));
        }
        let files = vec![
            (
                "README.md".to_string(),
                format!("# {name}\n\nImported from `{}`.\n", source.display()),
            ),
            (
                "wiki/index.md".to_string(),
                format!("# {name}\n\nUse this page to organize concepts from the imported notes.\n"),
            ),
            (
                "eval/smoke.md".to_string(),
                "# Import Check\n\n1. Confirm internal links resolve.\n2. Review attachments and unsupported files.\n"
                    .to_string(),
            ),
        ];
        materialize_base_in_staging(&staging, &manifest, &files)?;
        std::fs::rename(&staging, &target).map_err(|error| io_error(&target, error))
    })();
    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    outcome?;
    Ok(KnowledgeBaseMutation {
        changed: true,
        knowledge_base: knowledge_base_from_directory(&target, &id)?,
    })
}

#[derive(Default)]
struct ImportBudget {
    files: usize,
    bytes: u64,
}

fn copy_import_directory(
    source: &Path,
    destination: &Path,
    budget: &mut ImportBudget,
) -> Result<(), KnowledgeStoreError> {
    std::fs::create_dir_all(destination).map_err(|error| io_error(destination, error))?;
    let entries = std::fs::read_dir(source).map_err(|error| io_error(source, error))?;
    for entry in entries {
        let entry = entry.map_err(|error| io_error(source, error))?;
        let name = entry.file_name();
        if should_skip_import_entry(&name) {
            continue;
        }
        let source_path = entry.path();
        let metadata = std::fs::symlink_metadata(&source_path)
            .map_err(|error| io_error(&source_path, error))?;
        if metadata.file_type().is_symlink() {
            continue;
        }
        let destination_path = destination.join(&name);
        if metadata.is_dir() {
            copy_import_directory(&source_path, &destination_path, budget)?;
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        budget.files = budget.files.saturating_add(1);
        budget.bytes = budget.bytes.saturating_add(metadata.len());
        if budget.files > MAX_IMPORT_FILES || budget.bytes > MAX_IMPORT_BYTES {
            return Err(KnowledgeStoreError::Invalid(format!(
                "knowledge import exceeds the limit of {MAX_IMPORT_FILES} files or {MAX_IMPORT_BYTES} bytes"
            )));
        }
        std::fs::copy(&source_path, &destination_path)
            .map_err(|error| io_error(&destination_path, error))?;
    }
    Ok(())
}

fn should_skip_import_entry(name: &std::ffi::OsStr) -> bool {
    matches!(
        name.to_str(),
        Some(".obsidian" | ".git" | ".a3s" | ".trash" | ".DS_Store")
    )
}

pub(super) fn install_market_package(
    workspace: &Path,
    package: MarketPackage,
) -> Result<KnowledgeBaseMutation, KnowledgeStoreError> {
    validate_base_id(package.id)?;
    let target = bases_root(workspace).join(package.id);
    if target.exists() {
        let existing = knowledge_base_from_directory(&target, package.id)?;
        if existing.marketplace_id.as_deref() == Some(package.id) {
            return Ok(KnowledgeBaseMutation {
                changed: false,
                knowledge_base: existing,
            });
        }
        return Err(KnowledgeStoreError::Conflict(format!(
            "knowledge base `{}` already exists and was not installed from this marketplace item",
            package.id
        )));
    }
    let now = Utc::now().to_rfc3339();
    let manifest = KnowledgeBaseManifest {
        id: package.id.to_string(),
        name: package.name.to_string(),
        description: package.description.to_string(),
        origin: "marketplace".to_string(),
        marketplace_id: Some(package.id.to_string()),
        version: package.version.to_string(),
        pinned: true,
        created_at: now.clone(),
        updated_at: now,
    };
    let files = package
        .files
        .iter()
        .map(|file| (file.path.to_string(), file.content.to_string()))
        .collect::<Vec<_>>();
    materialize_base(workspace, &manifest, &files)?;
    Ok(KnowledgeBaseMutation {
        changed: true,
        knowledge_base: knowledge_base_from_directory(&target, package.id)?,
    })
}

pub(super) fn set_pinned(
    workspace: &Path,
    id: &str,
    pinned: bool,
) -> Result<KnowledgeBaseMutation, KnowledgeStoreError> {
    validate_base_id(id)?;
    if id == "workspace" {
        return Err(KnowledgeStoreError::Invalid(
            "the default workspace knowledge base is always pinned".to_string(),
        ));
    }
    let target = bases_root(workspace).join(id);
    if !is_regular_directory(&target) {
        return Err(KnowledgeStoreError::NotFound(format!(
            "knowledge base `{id}` was not found"
        )));
    }
    let manifest_path = target.join(MANIFEST_PATH);
    let mut manifest = read_manifest(&manifest_path, id)?;
    if manifest.pinned == pinned {
        return Ok(KnowledgeBaseMutation {
            changed: false,
            knowledge_base: knowledge_base_from_directory(&target, id)?,
        });
    }
    manifest.pinned = pinned;
    manifest.updated_at = Utc::now().to_rfc3339();
    atomic_write(&manifest_path, render_manifest(&manifest).as_bytes())?;
    Ok(KnowledgeBaseMutation {
        changed: true,
        knowledge_base: knowledge_base_from_directory(&target, id)?,
    })
}

fn materialize_base(
    workspace: &Path,
    manifest: &KnowledgeBaseManifest,
    files: &[(String, String)],
) -> Result<(), KnowledgeStoreError> {
    let root = bases_root(workspace);
    std::fs::create_dir_all(&root).map_err(|error| io_error(&root, error))?;
    let target = root.join(&manifest.id);
    if std::fs::symlink_metadata(&target).is_ok() {
        return Err(KnowledgeStoreError::Conflict(format!(
            "knowledge base `{}` already exists",
            manifest.id
        )));
    }
    let staging = root.join(format!(
        ".{}.tmp-{}-{}",
        manifest.id,
        std::process::id(),
        timestamp_nanos()
    ));
    std::fs::create_dir(&staging).map_err(|error| io_error(&staging, error))?;
    let outcome = materialize_base_in_staging(&staging, manifest, files).and_then(|()| {
        std::fs::rename(&staging, &target).map_err(|error| io_error(&target, error))
    });
    if outcome.is_err() {
        let _ = std::fs::remove_dir_all(&staging);
    }
    outcome
}

fn materialize_base_in_staging(
    staging: &Path,
    manifest: &KnowledgeBaseManifest,
    files: &[(String, String)],
) -> Result<(), KnowledgeStoreError> {
    for (relative, content) in files {
        let relative = safe_relative_path(relative)?;
        let destination = staging.join(relative);
        if let Some(parent) = destination.parent() {
            std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
        }
        std::fs::write(&destination, content).map_err(|error| io_error(&destination, error))?;
    }
    let asset_acl = render_asset_acl(manifest);
    let asset_acl_path = staging.join(asset_lifecycle::ASSET_ACL_PATH);
    if let Some(parent) = asset_acl_path.parent() {
        std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    }
    std::fs::write(&asset_acl_path, asset_acl).map_err(|error| io_error(&asset_acl_path, error))?;
    let manifest_path = staging.join(MANIFEST_PATH);
    std::fs::write(&manifest_path, render_manifest(manifest))
        .map_err(|error| io_error(&manifest_path, error))?;
    Ok(())
}

fn render_asset_acl(manifest: &KnowledgeBaseManifest) -> String {
    let source = [
        ("readme_path", "README.md"),
        ("sources_path", "sources"),
        ("wiki_path", "wiki"),
        ("eval_path", "eval"),
    ];
    let metadata = [("knowledge_base_manifest", MANIFEST_PATH)];
    asset_lifecycle::render_asset_acl(AssetAclDocument {
        category: "knowledge",
        kind: Some("knowledge"),
        name: &manifest.id,
        description: &manifest.description,
        local_path: Some(&manifest.id),
        service: OsService::KnowledgeService,
        runtime: RuntimeBindingIntent {
            kind: "knowledge",
            isolation: "serving",
            runtime_kind: "a3s-knowledge-service",
            protocol: Some("okf"),
            agent_kind: None,
        },
        source: &source,
        metadata: &metadata,
    })
}

fn knowledge_base_from_directory(
    path: &Path,
    expected_id: &str,
) -> Result<KnowledgeBase, KnowledgeStoreError> {
    let manifest = read_manifest(&path.join(MANIFEST_PATH), expected_id)?;
    let stats = base_stats(path);
    Ok(KnowledgeBase {
        id: manifest.id,
        name: manifest.name,
        description: manifest.description,
        origin: manifest.origin,
        marketplace_id: manifest.marketplace_id,
        version: manifest.version,
        pinned: manifest.pinned,
        created_at: manifest.created_at,
        updated_at: manifest.updated_at,
        path: path.display().to_string(),
        source_count: stats.source_count,
        concept_count: stats.concept_count,
        bytes: stats.bytes,
        compilation: compilation::summary_for_base(path),
    })
}

fn legacy_workspace_base(workspace: &Path) -> Option<KnowledgeBase> {
    let path = workspace.join(".a3s").join("kb");
    let sources = path.join("sources");
    let wiki = path.join("wiki");
    if !is_regular_directory(&sources) && !is_regular_directory(&wiki) {
        return None;
    }
    let stats = base_stats(&path);
    let updated_at = stats
        .latest_modified
        .map(system_time_rfc3339)
        .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
    Some(KnowledgeBase {
        id: "workspace".to_string(),
        name: "Workspace Knowledge".to_string(),
        description: "Default personal knowledge used by the /kb workflow.".to_string(),
        origin: "workspace".to_string(),
        marketplace_id: None,
        version: "legacy".to_string(),
        pinned: true,
        created_at: updated_at.clone(),
        updated_at,
        path: path.display().to_string(),
        source_count: stats.source_count,
        concept_count: stats.concept_count,
        bytes: stats.bytes,
        compilation: compilation::summary_for_base(&path),
    })
}

#[derive(Default)]
struct BaseStats {
    source_count: usize,
    concept_count: usize,
    bytes: u64,
    latest_modified: Option<SystemTime>,
}

fn base_stats(root: &Path) -> BaseStats {
    let mut stats = BaseStats::default();
    collect_stats(root, root, &mut stats);
    stats
}

fn collect_stats(root: &Path, directory: &Path, stats: &mut BaseStats) {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() {
            if entry.file_name() == ".a3s" {
                continue;
            }
            collect_stats(root, &path, stats);
            continue;
        }
        if !metadata.is_file() {
            continue;
        }
        stats.bytes = stats.bytes.saturating_add(metadata.len());
        if let Ok(modified) = metadata.modified() {
            stats.latest_modified = Some(
                stats
                    .latest_modified
                    .map(|current| current.max(modified))
                    .unwrap_or(modified),
            );
        }
        let relative = path.strip_prefix(root).unwrap_or(&path);
        if relative.starts_with("sources") {
            stats.source_count += 1;
        }
        if relative.starts_with("wiki")
            && path.extension().and_then(|extension| extension.to_str()) == Some("md")
        {
            stats.concept_count += 1;
        }
    }
}

fn read_manifest(
    path: &Path,
    expected_id: &str,
) -> Result<KnowledgeBaseManifest, KnowledgeStoreError> {
    let metadata = std::fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KnowledgeStoreError::NotFound(format!("missing {}", path.display()))
        } else {
            io_error(path, error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(KnowledgeStoreError::Invalid(format!(
            "{} must be a regular file",
            path.display()
        )));
    }
    let source = std::fs::read_to_string(path).map_err(|error| io_error(path, error))?;
    let document = a3s_acl::parse_acl(&source).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("invalid {}: {error}", path.display()))
    })?;
    let block = document
        .blocks
        .iter()
        .find(|block| block.name == "knowledge_base")
        .ok_or_else(|| {
            KnowledgeStoreError::Invalid(format!("{} has no knowledge_base block", path.display()))
        })?;
    if block.labels.as_slice() != [expected_id] {
        return Err(KnowledgeStoreError::Invalid(format!(
            "{} identity does not match its directory",
            path.display()
        )));
    }
    let required = |key: &str| -> Result<String, KnowledgeStoreError> {
        block
            .attributes
            .get(key)
            .and_then(AclValue::as_str)
            .map(str::to_string)
            .ok_or_else(|| {
                KnowledgeStoreError::Invalid(format!(
                    "{} is missing string attribute `{key}`",
                    path.display()
                ))
            })
    };
    let schema = required("schema")?;
    if schema != MANIFEST_SCHEMA {
        return Err(KnowledgeStoreError::Invalid(format!(
            "unsupported knowledge-base schema `{schema}`"
        )));
    }
    let origin = required("origin")?;
    if !matches!(
        origin.as_str(),
        "created" | "marketplace" | "imported" | "selection"
    ) {
        return Err(KnowledgeStoreError::Invalid(format!(
            "unsupported knowledge-base origin `{origin}`"
        )));
    }
    let marketplace_id = block
        .attributes
        .get("marketplace_id")
        .and_then(AclValue::as_str)
        .map(str::to_string);
    if origin == "marketplace" && marketplace_id.is_none() {
        return Err(KnowledgeStoreError::Invalid(
            "marketplace knowledge base is missing marketplace_id".to_string(),
        ));
    }
    let pinned = block
        .attributes
        .get("pinned")
        .and_then(AclValue::as_bool)
        .unwrap_or(false);
    Ok(KnowledgeBaseManifest {
        id: expected_id.to_string(),
        name: required("name")?,
        description: required("description")?,
        origin,
        marketplace_id,
        version: required("version")?,
        pinned,
        created_at: required("created_at")?,
        updated_at: required("updated_at")?,
    })
}

pub(super) fn resolve_base_path(
    workspace: &Path,
    id: &str,
) -> Result<PathBuf, KnowledgeStoreError> {
    if id == "workspace" {
        let path = workspace.join(".a3s").join("kb");
        if is_regular_directory(&path) {
            return Ok(path);
        }
        return Err(KnowledgeStoreError::NotFound(
            "workspace knowledge base was not found".to_string(),
        ));
    }
    validate_base_id(id)?;
    let path = bases_root(workspace).join(id);
    if !is_regular_directory(&path) {
        return Err(KnowledgeStoreError::NotFound(format!(
            "knowledge base `{id}` was not found"
        )));
    }
    Ok(path)
}

pub(super) fn knowledge_base_by_id(
    workspace: &Path,
    id: &str,
) -> Result<KnowledgeBase, KnowledgeStoreError> {
    let path = resolve_base_path(workspace, id)?;
    if id == "workspace" {
        return legacy_workspace_base(workspace).ok_or_else(|| {
            KnowledgeStoreError::NotFound("workspace knowledge base was not found".to_string())
        });
    }
    knowledge_base_from_directory(&path, id)
}

fn render_manifest(manifest: &KnowledgeBaseManifest) -> String {
    let mut attributes = HashMap::new();
    attributes.insert(
        "schema".to_string(),
        AclValue::String(MANIFEST_SCHEMA.to_string()),
    );
    attributes.insert("name".to_string(), AclValue::String(manifest.name.clone()));
    attributes.insert(
        "description".to_string(),
        AclValue::String(manifest.description.clone()),
    );
    attributes.insert(
        "origin".to_string(),
        AclValue::String(manifest.origin.clone()),
    );
    if let Some(marketplace_id) = &manifest.marketplace_id {
        attributes.insert(
            "marketplace_id".to_string(),
            AclValue::String(marketplace_id.clone()),
        );
    }
    attributes.insert(
        "version".to_string(),
        AclValue::String(manifest.version.clone()),
    );
    attributes.insert("pinned".to_string(), AclValue::Bool(manifest.pinned));
    attributes.insert(
        "created_at".to_string(),
        AclValue::String(manifest.created_at.clone()),
    );
    attributes.insert(
        "updated_at".to_string(),
        AclValue::String(manifest.updated_at.clone()),
    );
    a3s_acl::generate_acl(&Document {
        blocks: vec![Block {
            name: "knowledge_base".to_string(),
            labels: vec![manifest.id.clone()],
            blocks: Vec::new(),
            attributes,
        }],
    })
}

fn normalize_required_text(
    value: &str,
    label: &str,
    max_chars: usize,
) -> Result<String, KnowledgeStoreError> {
    let value = normalize_text(value);
    if value.is_empty() {
        return Err(KnowledgeStoreError::Invalid(format!("{label} is required")));
    }
    if value.chars().count() > max_chars {
        return Err(KnowledgeStoreError::Invalid(format!(
            "{label} must be at most {max_chars} characters"
        )));
    }
    Ok(value)
}

fn normalize_optional_text(value: Option<&str>, max_chars: usize) -> Option<String> {
    value
        .map(normalize_text)
        .filter(|value| !value.is_empty())
        .map(|value| value.chars().take(max_chars).collect())
}

fn normalize_text(value: &str) -> String {
    value.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn knowledge_base_id(name: &str) -> String {
    let mut slug = String::new();
    for character in name.chars() {
        if character.is_ascii_alphanumeric() {
            slug.push(character.to_ascii_lowercase());
        } else if !slug.is_empty() && !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        let digest = Sha256::digest(name.as_bytes());
        return format!("kb-{}", hex_prefix(&digest, 12));
    }
    slug.chars().take(56).collect::<String>()
}

fn hex_prefix(bytes: &[u8], digits: usize) -> String {
    bytes
        .iter()
        .flat_map(|byte| format!("{byte:02x}").chars().collect::<Vec<_>>())
        .take(digits)
        .collect()
}

fn validate_base_id(id: &str) -> Result<(), KnowledgeStoreError> {
    if id.is_empty()
        || id.len() > 64
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(KnowledgeStoreError::Invalid(
            "knowledge-base ID must contain only lowercase letters, digits, and hyphens"
                .to_string(),
        ));
    }
    Ok(())
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
            "invalid knowledge package path `{value}`"
        )));
    }
    Ok(path.to_path_buf())
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), KnowledgeStoreError> {
    let parent = path.parent().ok_or_else(|| {
        KnowledgeStoreError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    let temporary = parent.join(format!(
        ".knowledge-base.acl.tmp-{}-{}",
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

fn is_regular_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.is_dir() && !metadata.file_type().is_symlink())
        .unwrap_or(false)
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn system_time_rfc3339(value: SystemTime) -> String {
    DateTime::<Utc>::from(value).to_rfc3339()
}

fn io_error(path: &Path, error: std::io::Error) -> KnowledgeStoreError {
    KnowledgeStoreError::Io(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests;
