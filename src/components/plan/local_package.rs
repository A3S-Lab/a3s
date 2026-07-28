use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use sha2::{Digest, Sha256};

use super::{path_identity, PlannedLocalPackage, PlannedPath};

const LOCAL_PACKAGE_DIGEST_DOMAIN: &[u8] = b"a3s-component-local-package-v1\0";
const MAX_LOCAL_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LOCAL_PACKAGE_FILES: u64 = 10_000;
const MAX_LOCAL_PACKAGE_BYTES: u64 = 1_073_741_824;

pub(super) async fn fingerprint_local_package(path: &Path) -> anyhow::Result<PlannedLocalPackage> {
    let path = path.to_path_buf();
    tokio::task::spawn_blocking(move || fingerprint_local_package_blocking(&path))
        .await
        .context("local package fingerprint task failed")?
}

fn fingerprint_local_package_blocking(path: &Path) -> anyhow::Result<PlannedLocalPackage> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("failed to inspect local package {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        bail!(
            "local package source '{}' is a symbolic link",
            path.display()
        );
    }

    let mut digest = Sha256::new();
    digest.update(LOCAL_PACKAGE_DIGEST_DOMAIN);
    let (kind, file_count, byte_count) = if metadata.is_file() {
        if metadata.len() > MAX_LOCAL_ARCHIVE_BYTES {
            bail!(
                "local package archive exceeds the {} byte compressed-size limit",
                MAX_LOCAL_ARCHIVE_BYTES
            );
        }
        hash_file(&mut digest, path, "root", &metadata)?;
        ("file", 1, metadata.len())
    } else if metadata.is_dir() {
        let mut entries = Vec::new();
        collect_local_entries(path, path, &mut entries)?;
        entries.sort_by_key(|entry| path_identity(&entry.0));
        let mut file_count = 0_u64;
        let mut byte_count = 0_u64;
        for (relative, absolute, metadata) in entries {
            let identity = path_identity(&relative);
            if metadata.is_dir() {
                hash_field(&mut digest, b"directory", identity.as_bytes());
            } else if metadata.is_file() {
                hash_file(&mut digest, &absolute, &identity, &metadata)?;
                file_count = file_count
                    .checked_add(1)
                    .context("local package file count overflow")?;
                byte_count = byte_count
                    .checked_add(metadata.len())
                    .context("local package byte count overflow")?;
                if file_count > MAX_LOCAL_PACKAGE_FILES || byte_count > MAX_LOCAL_PACKAGE_BYTES {
                    bail!(
                        "local package exceeds the {} file or {} byte limit",
                        MAX_LOCAL_PACKAGE_FILES,
                        MAX_LOCAL_PACKAGE_BYTES
                    );
                }
            } else {
                bail!(
                    "local package entry '{}' is not a regular file or directory",
                    absolute.display()
                );
            }
        }
        ("directory", file_count, byte_count)
    } else {
        bail!(
            "local package source '{}' is not a regular file or directory",
            path.display()
        );
    };

    Ok(PlannedLocalPackage {
        path: PlannedPath::new(path),
        kind,
        sha256: format!("{:x}", digest.finalize()),
        file_count,
        byte_count,
    })
}

fn collect_local_entries(
    root: &Path,
    directory: &Path,
    output: &mut Vec<(PathBuf, PathBuf, std::fs::Metadata)>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(directory).with_context(|| {
        format!(
            "failed to read local package directory {}",
            directory.display()
        )
    })? {
        let entry = entry.with_context(|| {
            format!(
                "failed to read local package entry in {}",
                directory.display()
            )
        })?;
        let absolute = entry.path();
        let metadata = std::fs::symlink_metadata(&absolute).with_context(|| {
            format!(
                "failed to inspect local package entry {}",
                absolute.display()
            )
        })?;
        if metadata.file_type().is_symlink() {
            bail!(
                "local package entry '{}' is a symbolic link",
                absolute.display()
            );
        }
        let relative = absolute
            .strip_prefix(root)
            .context("local package entry escaped its source root")?
            .to_path_buf();
        output.push((relative, absolute.clone(), metadata.clone()));
        if metadata.is_dir() {
            collect_local_entries(root, &absolute, output)?;
        }
    }
    Ok(())
}

fn hash_file(
    digest: &mut Sha256,
    path: &Path,
    identity: &str,
    metadata: &std::fs::Metadata,
) -> anyhow::Result<()> {
    hash_field(digest, b"file", identity.as_bytes());
    hash_field(digest, b"size", &metadata.len().to_le_bytes());
    hash_field(digest, b"mode", &file_mode(metadata).to_le_bytes());
    let mut file = File::open(path)
        .with_context(|| format!("failed to open local package file {}", path.display()))?;
    let mut buffer = [0_u8; 64 * 1024];
    let mut bytes_read = 0_u64;
    loop {
        let count = file
            .read(&mut buffer)
            .with_context(|| format!("failed to read local package file {}", path.display()))?;
        if count == 0 {
            break;
        }
        digest.update(&buffer[..count]);
        bytes_read = bytes_read
            .checked_add(count as u64)
            .context("local package file size overflow")?;
    }
    if bytes_read != metadata.len() {
        bail!(
            "local package file '{}' changed while its plan was computed",
            path.display()
        );
    }
    Ok(())
}

fn hash_field(digest: &mut Sha256, label: &[u8], value: &[u8]) {
    digest.update((label.len() as u64).to_le_bytes());
    digest.update(label);
    digest.update((value.len() as u64).to_le_bytes());
    digest.update(value);
}

#[cfg(unix)]
fn file_mode(metadata: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;

    metadata.permissions().mode() & 0o777
}

#[cfg(not(unix))]
fn file_mode(metadata: &std::fs::Metadata) -> u32 {
    u32::from(metadata.permissions().readonly())
}
