use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use a3s_updater::{
    download_asset, extract_release_archive, fetch_release, parse_version, uninstall_owned_files,
    verify_sha256, ComponentReceipt, DirectoryActivation, InstallProvenance,
    RECEIPT_SCHEMA_VERSION,
};
use anyhow::{bail, Context};
use serde::{Deserialize, Serialize};

use super::catalog::ReleaseSpec;
use super::id::ComponentId;
use super::lifecycle::{InstallRequest, OperationRecord};
use super::paths::ComponentPaths;
use super::probe::probe_release;

/// One exact release artifact resolved before a component plan is approved.
///
/// Passing this value into the installer prevents a second `latest` lookup
/// from selecting a different release after the plan digest was checked.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedRelease {
    pub version: String,
    pub tag: String,
    pub target: String,
    pub archive_name: String,
    pub asset_url: String,
    pub sha256: String,
}

pub async fn resolve_release(
    id: &ComponentId,
    release_spec: ReleaseSpec,
    requested_version: Option<&str>,
) -> anyhow::Result<ResolvedRelease> {
    let target = release_spec.asset_family.target().with_context(|| {
        format!(
            "component '{}' has no release for {}-{}",
            id,
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    let release = fetch_release(
        release_spec.github_owner,
        release_spec.github_repo,
        requested_version,
    )
    .await?;
    let version = parse_version(&release.tag_name)?.to_string();
    let archive_name =
        release_spec
            .asset_family
            .archive_name(release_spec.binary, &version, target);
    let asset = release
        .assets
        .iter()
        .find(|asset| asset.name == archive_name)
        .cloned()
        .with_context(|| {
            format!(
                "release v{} has no asset '{}' for component '{}'",
                version, archive_name, id
            )
        })?;
    let sha256 = release_checksum(&release, &asset).await?;
    Ok(ResolvedRelease {
        version,
        tag: release.tag_name,
        target: target.to_string(),
        archive_name,
        asset_url: asset.browser_download_url,
        sha256,
    })
}

pub async fn install_release(
    id: &ComponentId,
    release_spec: ReleaseSpec,
    request: &InstallRequest,
    resolved: Option<&ResolvedRelease>,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    let resolved = match resolved {
        Some(resolved) => {
            validate_resolved_release(id, release_spec, request, resolved)?;
            resolved.clone()
        }
        None => {
            super::progress(
                request.progress,
                format!("a3s: resolving release for '{}'...", id),
            );
            resolve_release(id, release_spec, request.version.as_deref()).await?
        }
    };
    super::progress(
        request.progress,
        format!(
            "a3s: downloading '{}' {} for {}...",
            id, resolved.version, resolved.target
        ),
    );
    let archive = download_asset(&resolved.asset_url).await?;
    verify_sha256(&archive, &resolved.sha256)?;

    let component_root = paths.component_root(id);
    std::fs::create_dir_all(&component_root)?;
    let staging = tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(&component_root)?;
    let unpacked = staging.path().join("unpacked");
    extract_release_archive(&archive, &unpacked, &resolved.archive_name)?;
    let executable_name = release_spec
        .asset_family
        .executable_name(release_spec.binary, &resolved.target);
    let staged_executable = find_unique_file(&unpacked, &executable_name)?;
    if let Some(actual_version) = probe_release(release_spec, &staged_executable)? {
        if parse_version(&actual_version)? != parse_version(&resolved.version)? {
            bail!(
                "downloaded '{}' reported version {}, expected {}",
                id,
                actual_version,
                resolved.version
            );
        }
    }
    let relative_executable = staged_executable.strip_prefix(&unpacked)?.to_path_buf();
    let active = paths.version_root(id, &resolved.version);
    let old_receipt = paths.receipt_store().read(id.as_str())?;
    let activation = DirectoryActivation::activate(&unpacked, &active)?;
    let executable = active.join(relative_executable);
    let receipt = ComponentReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        component_id: id.to_string(),
        version: resolved.version.clone(),
        provenance: InstallProvenance::GithubRelease,
        install_root: active.clone(),
        executable_path: Some(executable.clone()),
        owned_paths: vec![active.clone()],
        source: Some(format!(
            "https://github.com/{}/{}/releases/tag/{}",
            release_spec.github_owner, release_spec.github_repo, resolved.tag
        )),
        artifact_checksums: BTreeMap::from([(
            resolved.archive_name.clone(),
            resolved.sha256.clone(),
        )]),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    paths.receipt_store().write(&receipt)?;
    activation.commit()?;

    if let Some(old) = old_receipt.filter(|old| old.install_root != active) {
        if old.provenance.owns_files() {
            let _ = uninstall_owned_files(&old, &paths.data_root);
        }
    }
    Ok(OperationRecord {
        component: id.clone(),
        action: request.intent.action(),
        changed: true,
        recovered: false,
        version: Some(resolved.version),
        provenance: Some(InstallProvenance::GithubRelease),
        path: Some(executable),
        message: format!(
            "Completed {} for component '{}' from its verified release.",
            request.intent.action(),
            id
        ),
    })
}

fn validate_resolved_release(
    id: &ComponentId,
    release_spec: ReleaseSpec,
    request: &InstallRequest,
    resolved: &ResolvedRelease,
) -> anyhow::Result<()> {
    let target = release_spec.asset_family.target().with_context(|| {
        format!(
            "component '{}' has no release for {}-{}",
            id,
            std::env::consts::OS,
            std::env::consts::ARCH
        )
    })?;
    if resolved.target != target {
        bail!(
            "reviewed release target '{}' does not match current target '{}'",
            resolved.target,
            target
        );
    }
    let version = parse_version(&resolved.version)?;
    if parse_version(&resolved.tag)? != version {
        bail!(
            "reviewed release tag '{}' does not match version '{}'",
            resolved.tag,
            resolved.version
        );
    }
    if let Some(requested) = request.version.as_deref() {
        if parse_version(requested)? != version {
            bail!(
                "reviewed release version '{}' does not match requested version '{}'",
                resolved.version,
                requested
            );
        }
    }
    let expected_archive = release_spec.asset_family.archive_name(
        release_spec.binary,
        &resolved.version,
        &resolved.target,
    );
    if resolved.archive_name != expected_archive {
        bail!(
            "reviewed release asset '{}' does not match expected asset '{}'",
            resolved.archive_name,
            expected_archive
        );
    }
    if resolved.asset_url.trim().is_empty() {
        bail!("reviewed release asset URL cannot be empty");
    }
    if resolved.sha256.len() != 64
        || !resolved
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        bail!("reviewed release SHA-256 digest is invalid");
    }
    Ok(())
}

async fn release_checksum(
    release: &a3s_updater::Release,
    asset: &a3s_updater::Asset,
) -> anyhow::Result<String> {
    if let Some(digest) = asset.digest.as_deref() {
        let digest = digest.strip_prefix("sha256:").unwrap_or(digest);
        if digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return Ok(digest.to_ascii_lowercase());
        }
    }
    for name in checksum_asset_names(&asset.name) {
        let Some(checksum_asset) = release
            .assets
            .iter()
            .find(|candidate| candidate.name == name)
        else {
            continue;
        };
        let bytes = download_asset(&checksum_asset.browser_download_url).await?;
        if let Some(checksum) = parse_checksum_file(&bytes, &asset.name) {
            return Ok(checksum);
        }
    }
    bail!(
        "release asset '{}' has no trusted SHA-256 digest or checksum file",
        asset.name
    )
}

fn checksum_asset_names(asset_name: &str) -> Vec<String> {
    let mut names = vec![format!("{asset_name}.sha256")];
    let base_name = asset_name
        .strip_suffix(".tar.gz")
        .or_else(|| asset_name.strip_suffix(".zip"));
    if let Some(base_name) = base_name {
        let companion = format!("{base_name}.sha256");
        if !names.contains(&companion) {
            names.push(companion);
        }
    }
    names.push("checksums.txt".to_string());
    names
}

fn parse_checksum_file(bytes: &[u8], asset_name: &str) -> Option<String> {
    let text = std::str::from_utf8(bytes).ok()?;
    for line in text.lines() {
        let mut fields = line.split_whitespace();
        let digest = fields.next()?.trim_start_matches("sha256:");
        let file = fields.next().map(|value| value.trim_start_matches('*'));
        if digest.len() == 64
            && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
            && (file.is_none() || file == Some(asset_name))
        {
            return Some(digest.to_ascii_lowercase());
        }
    }
    None
}

fn find_unique_file(root: &Path, file_name: &str) -> anyhow::Result<PathBuf> {
    let mut matches = Vec::new();
    collect_named_files(root, file_name, &mut matches)?;
    match matches.as_slice() {
        [path] => Ok(path.clone()),
        [] => bail!("release archive does not contain '{}'", file_name),
        _ => bail!("release archive contains multiple '{}' files", file_name),
    }
}

fn collect_named_files(
    root: &Path,
    file_name: &str,
    matches: &mut Vec<PathBuf>,
) -> anyhow::Result<()> {
    for entry in std::fs::read_dir(root)? {
        let entry = entry?;
        let path = entry.path();
        if entry.file_type()?.is_dir() {
            collect_named_files(&path, file_name, matches)?;
        } else if path.file_name().and_then(|value| value.to_str()) == Some(file_name) {
            matches.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_checksum_formats() {
        let digest = "a".repeat(64);
        assert_eq!(
            parse_checksum_file(
                format!("{digest}  a3s-use-1.0.0-linux-x86_64.tar.gz\n").as_bytes(),
                "a3s-use-1.0.0-linux-x86_64.tar.gz"
            ),
            Some(digest.clone())
        );
        assert_eq!(
            parse_checksum_file(format!("{digest}\n").as_bytes(), "asset.tar.gz"),
            Some(digest)
        );
        assert_eq!(
            parse_checksum_file(
                format!("{}  other.tar.gz\n", "b".repeat(64)).as_bytes(),
                "asset.tar.gz"
            ),
            None
        );
    }

    #[test]
    fn checks_the_webview_release_companion_checksum_name() {
        assert_eq!(
            checksum_asset_names("a3s-webview-v0.1.3-x86_64-pc-windows-msvc.zip"),
            vec![
                "a3s-webview-v0.1.3-x86_64-pc-windows-msvc.zip.sha256",
                "a3s-webview-v0.1.3-x86_64-pc-windows-msvc.sha256",
                "checksums.txt",
            ]
        );
    }
}
