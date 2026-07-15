//! Reusable component installation receipts and activation transactions.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{BufReader, BufWriter, Write};
use std::path::{Component, Path, PathBuf};

use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

/// Current on-disk receipt schema.
pub const RECEIPT_SCHEMA_VERSION: u32 = 1;

/// Where a component came from and which owner may mutate it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum InstallProvenance {
    Bundled,
    Homebrew,
    GithubRelease,
    ExternalPath,
    System,
    Delegated,
    LocalPackage,
}

impl InstallProvenance {
    /// Whether A3S may remove receipt-owned paths directly.
    pub fn owns_files(self) -> bool {
        matches!(self, Self::GithubRelease | Self::LocalPackage)
    }
}

/// Machine-owned record for one directly installed component.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentReceipt {
    pub schema_version: u32,
    pub component_id: String,
    pub version: String,
    pub provenance: InstallProvenance,
    pub install_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub executable_path: Option<PathBuf>,
    #[serde(default)]
    pub owned_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default)]
    pub artifact_checksums: BTreeMap<String, String>,
    pub installed_at: String,
}

impl ComponentReceipt {
    /// Validate identity and ownership boundaries before persisting or deleting.
    pub fn validate(&self) -> anyhow::Result<()> {
        if self.schema_version != RECEIPT_SCHEMA_VERSION {
            bail!(
                "unsupported component receipt schema {}; expected {}",
                self.schema_version,
                RECEIPT_SCHEMA_VERSION
            );
        }
        validate_component_id(&self.component_id)?;
        if self.version.trim().is_empty() {
            bail!("component receipt version cannot be empty");
        }
        validate_absolute_clean_path(&self.install_root, "install root")?;

        if let Some(executable) = &self.executable_path {
            validate_child_path(executable, &self.install_root, "executable path")?;
        }

        let mut unique_paths = BTreeSet::new();
        for path in &self.owned_paths {
            validate_absolute_clean_path(path, "owned path")?;
            if !path.starts_with(&self.install_root) {
                bail!(
                    "owned path {} must be inside install root {}",
                    path.display(),
                    self.install_root.display()
                );
            }
            if !unique_paths.insert(path) {
                bail!(
                    "component receipt contains duplicate owned path {}",
                    path.display()
                );
            }
        }
        Ok(())
    }
}

/// Versioned receipt storage rooted in the caller-selected state directory.
#[derive(Debug, Clone)]
pub struct ReceiptStore {
    state_root: PathBuf,
}

impl ReceiptStore {
    pub fn new(state_root: impl Into<PathBuf>) -> Self {
        Self {
            state_root: state_root.into(),
        }
    }

    pub fn state_root(&self) -> &Path {
        &self.state_root
    }

    pub fn receipt_path(&self, component_id: &str) -> anyhow::Result<PathBuf> {
        let segments = validate_component_id(component_id)?;
        let mut path = self.state_root.join("components");
        for segment in &segments[..segments.len() - 1] {
            path.push(segment);
        }
        path.push(format!("{}.json", segments[segments.len() - 1]));
        Ok(path)
    }

    pub fn read(&self, component_id: &str) -> anyhow::Result<Option<ComponentReceipt>> {
        let path = self.receipt_path(component_id)?;
        let file = match File::open(&path) {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to open receipt {}", path.display()))
            }
        };
        let receipt: ComponentReceipt = serde_json::from_reader(BufReader::new(file))
            .with_context(|| format!("failed to parse receipt {}", path.display()))?;
        receipt.validate().with_context(|| {
            format!(
                "invalid receipt for component '{}' at {}",
                component_id,
                path.display()
            )
        })?;
        if receipt.component_id != component_id {
            bail!(
                "receipt identity mismatch: requested '{}', found '{}'",
                component_id,
                receipt.component_id
            );
        }
        Ok(Some(receipt))
    }

    pub fn write(&self, receipt: &ComponentReceipt) -> anyhow::Result<()> {
        receipt.validate()?;
        let path = self.receipt_path(&receipt.component_id)?;
        let parent = path
            .parent()
            .context("component receipt path has no parent directory")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create receipt directory {}", parent.display()))?;

        let mut temp = tempfile::NamedTempFile::new_in(parent).with_context(|| {
            format!("failed to create temporary receipt in {}", parent.display())
        })?;
        {
            let mut writer = BufWriter::new(temp.as_file_mut());
            serde_json::to_writer_pretty(&mut writer, receipt)
                .context("failed to serialize component receipt")?;
            writer.write_all(b"\n")?;
            writer.flush()?;
        }
        temp.as_file().sync_all()?;
        temp.persist(&path)
            .map_err(|error| error.error)
            .with_context(|| format!("failed to atomically persist receipt {}", path.display()))?;
        Ok(())
    }

    pub fn remove(&self, component_id: &str) -> anyhow::Result<()> {
        let path = self.receipt_path(component_id)?;
        match std::fs::remove_file(&path) {
            Ok(()) => self.remove_empty_receipt_parents(path.parent()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => {
                Err(error).with_context(|| format!("failed to remove receipt {}", path.display()))
            }
        }
    }

    pub fn list(&self) -> anyhow::Result<Vec<ComponentReceipt>> {
        let root = self.state_root.join("components");
        if !root.exists() {
            return Ok(Vec::new());
        }
        let mut files = Vec::new();
        collect_receipt_files(&root, &mut files)?;
        files.sort();

        let mut receipts = Vec::with_capacity(files.len());
        for path in files {
            let file = File::open(&path)
                .with_context(|| format!("failed to open receipt {}", path.display()))?;
            let receipt: ComponentReceipt = serde_json::from_reader(BufReader::new(file))
                .with_context(|| format!("failed to parse receipt {}", path.display()))?;
            receipt
                .validate()
                .with_context(|| format!("invalid receipt {}", path.display()))?;
            receipts.push(receipt);
        }
        receipts.sort_by(|left, right| left.component_id.cmp(&right.component_id));
        Ok(receipts)
    }

    fn remove_empty_receipt_parents(&self, mut parent: Option<&Path>) -> anyhow::Result<()> {
        let components_root = self.state_root.join("components");
        while let Some(directory) = parent {
            if directory == components_root || !directory.starts_with(&components_root) {
                break;
            }
            match std::fs::remove_dir(directory) {
                Ok(()) => parent = directory.parent(),
                Err(error) if error.kind() == std::io::ErrorKind::DirectoryNotEmpty => break,
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    parent = directory.parent()
                }
                Err(error) => {
                    return Err(error).with_context(|| {
                        format!("failed to remove empty directory {}", directory.display())
                    })
                }
            }
        }
        Ok(())
    }
}

/// Remove only paths proven to be owned by a direct-install receipt.
pub fn uninstall_owned_files(
    receipt: &ComponentReceipt,
    allowed_data_root: &Path,
) -> anyhow::Result<Vec<PathBuf>> {
    receipt.validate()?;
    validate_absolute_clean_path(allowed_data_root, "allowed data root")?;
    if !receipt.provenance.owns_files() {
        bail!(
            "component '{}' is not directly owned by A3S ({:?})",
            receipt.component_id,
            receipt.provenance
        );
    }
    if receipt.install_root == allowed_data_root
        || !receipt.install_root.starts_with(allowed_data_root)
    {
        bail!(
            "component install root {} is outside approved data root {}",
            receipt.install_root.display(),
            allowed_data_root.display()
        );
    }

    let mut paths = receipt.owned_paths.clone();
    paths.sort_by_key(|path| std::cmp::Reverse(path.components().count()));
    let mut removed = Vec::new();
    for path in paths {
        if !path.exists() && std::fs::symlink_metadata(&path).is_err() {
            continue;
        }
        let metadata = std::fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect owned path {}", path.display()))?;
        if metadata.file_type().is_symlink() || metadata.is_file() {
            std::fs::remove_file(&path)
                .with_context(|| format!("failed to remove owned file {}", path.display()))?;
        } else if metadata.is_dir() {
            std::fs::remove_dir_all(&path)
                .with_context(|| format!("failed to remove owned directory {}", path.display()))?;
        } else {
            bail!(
                "refusing to remove unsupported owned path {}",
                path.display()
            );
        }
        removed.push(path);
    }
    Ok(removed)
}

/// Guard for atomic directory activation.
///
/// Dropping without calling commit restores the prior active directory.
#[derive(Debug)]
pub struct DirectoryActivation {
    active: PathBuf,
    backup: Option<PathBuf>,
    committed: bool,
}

impl DirectoryActivation {
    pub fn activate(staged: &Path, active: &Path) -> anyhow::Result<Self> {
        if !staged.is_dir() {
            bail!(
                "staged component directory does not exist: {}",
                staged.display()
            );
        }
        let parent = active
            .parent()
            .context("active component path has no parent directory")?;
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create active parent {}", parent.display()))?;

        let backup = if active.exists() {
            let backup = unique_backup_path(active);
            std::fs::rename(active, &backup).with_context(|| {
                format!(
                    "failed to stage prior component {} as {}",
                    active.display(),
                    backup.display()
                )
            })?;
            Some(backup)
        } else {
            None
        };

        if let Err(error) = std::fs::rename(staged, active) {
            if let Some(backup) = &backup {
                let _ = std::fs::rename(backup, active);
            }
            return Err(error).with_context(|| {
                format!(
                    "failed to activate staged component {} at {}",
                    staged.display(),
                    active.display()
                )
            });
        }

        Ok(Self {
            active: active.to_path_buf(),
            backup,
            committed: false,
        })
    }

    pub fn commit(mut self) -> anyhow::Result<()> {
        if let Some(backup) = self.backup.clone() {
            if backup.is_dir() {
                std::fs::remove_dir_all(&backup).with_context(|| {
                    format!("failed to remove activation backup {}", backup.display())
                })?;
            } else if backup.exists() {
                std::fs::remove_file(&backup).with_context(|| {
                    format!("failed to remove activation backup {}", backup.display())
                })?;
            }
            self.backup = None;
        }
        self.committed = true;
        Ok(())
    }
}

impl Drop for DirectoryActivation {
    fn drop(&mut self) {
        if self.committed {
            return;
        }
        if self.active.is_dir() {
            let _ = std::fs::remove_dir_all(&self.active);
        } else if self.active.exists() {
            let _ = std::fs::remove_file(&self.active);
        }
        if let Some(backup) = &self.backup {
            let _ = std::fs::rename(backup, &self.active);
        }
    }
}

fn validate_component_id(component_id: &str) -> anyhow::Result<Vec<&str>> {
    let segments = component_id.split('/').collect::<Vec<_>>();
    if segments.is_empty() || segments.iter().any(|segment| !valid_id_segment(segment)) {
        bail!("invalid component ID '{component_id}'");
    }
    Ok(segments)
}

fn valid_id_segment(segment: &str) -> bool {
    let mut chars = segment.chars();
    matches!(chars.next(), Some(first) if first.is_ascii_lowercase())
        && chars.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn validate_absolute_clean_path(path: &Path, label: &str) -> anyhow::Result<()> {
    if !path.is_absolute() {
        bail!("{label} must be absolute: {}", path.display());
    }
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir | Component::CurDir))
    {
        bail!("{label} must not contain traversal: {}", path.display());
    }
    Ok(())
}

fn validate_child_path(path: &Path, install_root: &Path, label: &str) -> anyhow::Result<()> {
    validate_absolute_clean_path(path, label)?;
    if path == install_root || !path.starts_with(install_root) {
        bail!(
            "{label} {} must be a child of install root {}",
            path.display(),
            install_root.display()
        );
    }
    Ok(())
}

fn collect_receipt_files(root: &Path, output: &mut Vec<PathBuf>) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(root)
        .with_context(|| format!("failed to read receipt directory {}", root.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let file_type = entry.file_type()?;
        if file_type.is_dir() {
            collect_receipt_files(&path, output)?;
        } else if file_type.is_file()
            && path.extension().and_then(|value| value.to_str()) == Some("json")
        {
            output.push(path);
        }
    }
    Ok(())
}

fn unique_backup_path(active: &Path) -> PathBuf {
    let parent = active.parent().unwrap_or_else(|| Path::new("."));
    let name = active
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("component");
    for suffix in 0..u32::MAX {
        let candidate = parent.join(format!(".{name}.a3s-backup-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    parent.join(format!(".{name}.a3s-backup"))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn receipt(root: &Path, id: &str) -> ComponentReceipt {
        let install_root = root.join("data/components").join(id).join("1.2.3");
        ComponentReceipt {
            schema_version: RECEIPT_SCHEMA_VERSION,
            component_id: id.to_string(),
            version: "1.2.3".to_string(),
            provenance: InstallProvenance::GithubRelease,
            install_root: install_root.clone(),
            executable_path: Some(install_root.join("bin/tool")),
            owned_paths: vec![install_root],
            source: Some("https://example.invalid/release".to_string()),
            artifact_checksums: BTreeMap::from([(
                "archive.tar.gz".to_string(),
                "abc123".to_string(),
            )]),
            installed_at: "2026-07-14T00:00:00Z".to_string(),
        }
    }

    #[test]
    fn receipt_round_trip_and_nested_listing() {
        let temp = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(temp.path().join("state"));
        let browser = receipt(temp.path(), "use/browser");
        let use_product = receipt(temp.path(), "use");

        store.write(&browser).unwrap();
        store.write(&use_product).unwrap();

        assert_eq!(store.read("use/browser").unwrap(), Some(browser.clone()));
        assert_eq!(store.list().unwrap(), vec![use_product, browser]);
        assert!(store
            .receipt_path("use/browser")
            .unwrap()
            .ends_with("components/use/browser.json"));

        store.remove("use/browser").unwrap();
        assert_eq!(store.read("use/browser").unwrap(), None);
    }

    #[test]
    fn receipt_rejects_invalid_identity_and_escaping_paths() {
        let temp = tempfile::tempdir().unwrap();
        let store = ReceiptStore::new(temp.path().join("state"));
        assert!(store.receipt_path("use/../box").is_err());
        assert!(store.receipt_path("Use/browser").is_err());

        let mut invalid = receipt(temp.path(), "use");
        invalid.owned_paths = vec![temp.path().join("outside")];
        assert!(invalid.validate().is_err());
        assert!(store.write(&invalid).is_err());
    }

    #[test]
    fn uninstall_removes_only_receipt_owned_paths() {
        let temp = tempfile::tempdir().unwrap();
        let data_root = temp.path().join("data");
        let item = receipt(temp.path(), "use");
        std::fs::create_dir_all(item.install_root.join("bin")).unwrap();
        std::fs::write(item.install_root.join("bin/tool"), "owned").unwrap();
        let user_data = data_root.join("profiles/browser-profile");
        std::fs::create_dir_all(user_data.parent().unwrap()).unwrap();
        std::fs::write(&user_data, "preserve").unwrap();

        let removed = uninstall_owned_files(&item, &data_root).unwrap();

        assert_eq!(removed, vec![item.install_root.clone()]);
        assert!(!item.install_root.exists());
        assert_eq!(std::fs::read_to_string(user_data).unwrap(), "preserve");
    }

    #[test]
    fn uninstall_refuses_external_ownership() {
        let temp = tempfile::tempdir().unwrap();
        let mut item = receipt(temp.path(), "use");
        item.provenance = InstallProvenance::ExternalPath;
        assert!(uninstall_owned_files(&item, &temp.path().join("data")).is_err());
    }

    #[test]
    fn activation_rolls_back_until_committed() {
        let temp = tempfile::tempdir().unwrap();
        let active = temp.path().join("active");
        let staged = temp.path().join("staged");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(active.join("version"), "old").unwrap();
        std::fs::write(staged.join("version"), "new").unwrap();

        {
            let _activation = DirectoryActivation::activate(&staged, &active).unwrap();
            assert_eq!(
                std::fs::read_to_string(active.join("version")).unwrap(),
                "new"
            );
        }

        assert_eq!(
            std::fs::read_to_string(active.join("version")).unwrap(),
            "old"
        );
    }

    #[test]
    fn activation_commit_keeps_new_directory() {
        let temp = tempfile::tempdir().unwrap();
        let active = temp.path().join("active");
        let staged = temp.path().join("staged");
        std::fs::create_dir_all(&active).unwrap();
        std::fs::create_dir_all(&staged).unwrap();
        std::fs::write(active.join("version"), "old").unwrap();
        std::fs::write(staged.join("version"), "new").unwrap();

        DirectoryActivation::activate(&staged, &active)
            .unwrap()
            .commit()
            .unwrap();

        assert_eq!(
            std::fs::read_to_string(active.join("version")).unwrap(),
            "new"
        );
        assert!(!staged.exists());
    }
}
