use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::time::Duration;

use a3s_updater::{download_asset, extract_release_archive, verify_sha256, DirectoryActivation};
use anyhow::{bail, Context};
use fs2::FileExt;
use tokio::time::{sleep, Instant};

const WEB_RELEASE_OWNER: &str = "A3S-Lab";
const WEB_RELEASE_REPOSITORY: &str = "a3s";
const DEFAULT_GITHUB_RELEASE_BASE: &str = "https://github.com";
const WEB_ASSET_LOCK_TIMEOUT: Duration = Duration::from_secs(300);
const WEB_ASSET_LOCK_POLL_INTERVAL: Duration = Duration::from_millis(50);

pub(in crate::api) async fn prepare_default_web_dir(
    workspace: &Path,
    offline: bool,
    allow_download: bool,
) -> anyhow::Result<PathBuf> {
    let paths = a3s::components::ComponentPaths::from_env_at(workspace);
    if let Some(directory) =
        find_default_web_dir(paths.as_ref().ok().map(|paths| paths.data_root.as_path()))
    {
        return Ok(directory);
    }

    if offline {
        bail!(
            "web assets were not found and offline mode forbids the first-use download; pass \
             --web-dir, use --api-only, or rerun without --offline"
        );
    }
    if !allow_download {
        bail!(
            "web assets were not found and automatic downloads are disabled by \
             A3S_NO_AUTO_INSTALL=1; pass --web-dir, use --api-only, or allow automatic setup"
        );
    }

    let paths = paths.context(
        "web assets were not found and the A3S data directories could not be resolved; pass \
         --web-dir or use --api-only",
    )?;
    install_matching_web_release(&paths).await.map_err(|error| {
        anyhow::anyhow!(
            "failed to prepare A3S Web assets for a3s {}: {error:#}; pass --web-dir, use \
             --api-only, or retry when the matching GitHub release assets are available",
            env!("CARGO_PKG_VERSION")
        )
    })
}

fn find_default_web_dir(data_root: Option<&Path>) -> Option<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(executable) = std::env::current_exe() {
        candidates.extend(packaged_candidates(&executable));
    }
    if let Some(data_root) = data_root {
        candidates.push(cached_web_dir(data_root));
    }
    if let Ok(cwd) = std::env::current_dir() {
        candidates.extend(upward_candidates(&cwd));
        candidates.push(cwd.join("dist/workspace"));
        candidates.push(cwd.join("dist"));
    }

    find_existing_web_dir(candidates)
}

fn cached_web_dir(data_root: &Path) -> PathBuf {
    data_root.join("web").join(env!("CARGO_PKG_VERSION"))
}

async fn install_matching_web_release(
    paths: &a3s::components::ComponentPaths,
) -> anyhow::Result<PathBuf> {
    let active = cached_web_dir(&paths.data_root);
    let _lock = acquire_asset_lock(&paths.runtime_root).await?;
    if active.join("index.html").is_file() {
        return Ok(clean_path(active));
    }

    let version = env!("CARGO_PKG_VERSION");
    let archive_name = format!("a3s-web-v{version}.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");
    let release_base = github_release_base();
    let archive_url = release_asset_url(&release_base, version, &archive_name);
    let checksum_url = release_asset_url(&release_base, version, &checksum_name);

    // The CLI version already identifies the exact release. Downloading the two
    // deterministic assets avoids spending an anonymous GitHub API request just
    // to rediscover URLs that are fixed by the release naming contract.
    let checksum_file = download_asset(&checksum_url)
        .await
        .with_context(|| format!("failed to download Web checksum '{checksum_name}'"))?;
    let checksum = parse_checksum_file(&checksum_file, &archive_name)?;
    let archive = download_asset(&archive_url)
        .await
        .with_context(|| format!("failed to download Web archive '{archive_name}'"))?;
    verify_sha256(&archive, &checksum)?;

    let web_root = paths.data_root.join("web");
    std::fs::create_dir_all(&web_root)
        .with_context(|| format!("failed to create Web data root {}", web_root.display()))?;
    let staging = tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(&web_root)
        .with_context(|| format!("failed to stage Web assets in {}", web_root.display()))?;
    let unpacked = staging.path().join("unpacked");
    let extracted = extract_release_archive(&archive, &unpacked, &archive_name)?;
    let staged_web = unpacked.join("web");
    validate_release_layout(&unpacked, &staged_web, &extracted)?;

    let activation = DirectoryActivation::activate(&staged_web, &active)?;
    activation.commit()?;
    Ok(clean_path(active))
}

fn github_release_base() -> String {
    std::env::var("A3S_UPDATER_GITHUB_RELEASE_BASE")
        .unwrap_or_else(|_| DEFAULT_GITHUB_RELEASE_BASE.to_string())
        .trim_end_matches('/')
        .to_string()
}

fn release_asset_url(base: &str, version: &str, asset_name: &str) -> String {
    format!(
        "{}/{}/{}/releases/download/v{}/{}",
        base.trim_end_matches('/'),
        WEB_RELEASE_OWNER,
        WEB_RELEASE_REPOSITORY,
        version.trim_start_matches('v'),
        asset_name
    )
}

fn parse_checksum_file(bytes: &[u8], asset_name: &str) -> anyhow::Result<String> {
    let text = std::str::from_utf8(bytes).context("Web asset checksum file is not UTF-8")?;
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty()) {
        let mut fields = line.split_whitespace();
        let Some(digest) = fields.next() else {
            continue;
        };
        if digest.len() != 64 || !digest.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            continue;
        }
        if let Some(name) = fields.next() {
            if name.trim_start_matches('*') != asset_name {
                continue;
            }
        }
        return Ok(digest.to_ascii_lowercase());
    }
    bail!(
        "companion checksum does not contain a valid SHA-256 entry for '{}'",
        asset_name
    )
}

fn validate_release_layout(
    unpacked: &Path,
    staged_web: &Path,
    extracted: &[PathBuf],
) -> anyhow::Result<()> {
    if !staged_web.join("index.html").is_file() {
        bail!("release Web archive does not contain web/index.html");
    }
    for file in extracted {
        let relative = file
            .strip_prefix(unpacked)
            .with_context(|| format!("extracted path {} escaped staging", file.display()))?;
        if !relative.starts_with("web") {
            bail!(
                "release Web archive contains a file outside the web directory: {}",
                relative.display()
            );
        }
    }
    Ok(())
}

async fn acquire_asset_lock(runtime_root: &Path) -> anyhow::Result<WebAssetLock> {
    let path = runtime_root.join("web/assets.lock");
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create Web asset lock directory {}",
                parent.display()
            )
        })?;
    }
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&path)
        .with_context(|| format!("failed to open Web asset lock {}", path.display()))?;
    let deadline = Instant::now() + WEB_ASSET_LOCK_TIMEOUT;
    loop {
        match file.try_lock_exclusive() {
            Ok(()) => return Ok(WebAssetLock { file }),
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                if Instant::now() >= deadline {
                    bail!(
                        "timed out waiting for another process to prepare A3S Web assets at {}",
                        path.display()
                    );
                }
                sleep(WEB_ASSET_LOCK_POLL_INTERVAL).await;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("failed to lock Web assets at {}", path.display()))
            }
        }
    }
}

struct WebAssetLock {
    file: File,
}

impl Drop for WebAssetLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

fn packaged_candidates(executable: &Path) -> Vec<PathBuf> {
    let mut candidates = packaged_layout_candidates(executable);
    if let Ok(canonical) = executable.canonicalize() {
        if canonical != executable {
            candidates.extend(packaged_layout_candidates(&canonical));
        }
    }
    candidates
}

fn packaged_layout_candidates(executable: &Path) -> Vec<PathBuf> {
    let Some(bin_dir) = executable.parent() else {
        return Vec::new();
    };

    let mut candidates = vec![bin_dir.join("web")];
    if let Some(prefix) = bin_dir.parent() {
        candidates.push(prefix.join("share/a3s/web"));
    }
    candidates
}

fn find_existing_web_dir(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates
        .into_iter()
        .map(clean_path)
        .find(|candidate| candidate.join("index.html").is_file())
}

fn upward_candidates(start: &Path) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    let mut current = Some(start);
    while let Some(dir) = current {
        candidates.push(dir.join("apps/web/dist/workspace"));
        candidates.push(dir.join("apps/web/dist"));
        current = dir.parent();
    }
    candidates
}

fn clean_path(path: PathBuf) -> PathBuf {
    path.canonicalize().unwrap_or(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn packaged_layouts_cover_archives_and_install_prefixes() {
        let executable = Path::new("prefix").join("bin").join("a3s");

        assert_eq!(
            packaged_layout_candidates(&executable),
            vec![
                Path::new("prefix").join("bin/web"),
                Path::new("prefix").join("share/a3s/web"),
            ]
        );
    }

    #[test]
    fn deterministic_release_url_does_not_use_the_github_api() {
        assert_eq!(
            release_asset_url("https://github.com/", "0.9.8", "a3s-web-v0.9.8.tar.gz"),
            "https://github.com/A3S-Lab/a3s/releases/download/v0.9.8/a3s-web-v0.9.8.tar.gz"
        );
    }

    #[test]
    fn existing_packaged_assets_win_over_cached_and_development_fallbacks() {
        let root = tempfile::tempdir().expect("temporary asset layouts");
        let packaged = root.path().join("bin/web");
        let cached = root.path().join("data/web/0.9.8");
        let development = root.path().join("apps/web/dist/workspace");
        for directory in [&packaged, &cached, &development] {
            std::fs::create_dir_all(directory).expect("Web directory");
            std::fs::write(directory.join("index.html"), "web").expect("Web index");
        }

        let found = find_existing_web_dir([packaged.clone(), cached, development])
            .expect("existing Web directory");

        assert_eq!(found, packaged.canonicalize().expect("canonical package"));
    }

    #[test]
    fn checksum_parser_requires_the_requested_archive_entry() {
        let digest = "a".repeat(64);
        let checksum = format!("{digest}  another.tar.gz\n{digest} *a3s-web-v0.9.8.tar.gz\n");

        assert_eq!(
            parse_checksum_file(checksum.as_bytes(), "a3s-web-v0.9.8.tar.gz")
                .expect("matching checksum"),
            digest
        );
    }

    #[cfg(unix)]
    #[test]
    fn executable_symlink_can_find_assets_in_its_install_prefix() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().expect("temporary package layout");
        let linked_bin = root.path().join("bin");
        let package_bin = root.path().join("Cellar/a3s/0.9.6/bin");
        let package_web = root.path().join("Cellar/a3s/0.9.6/share/a3s/web");
        std::fs::create_dir_all(&linked_bin).expect("linked bin directory");
        std::fs::create_dir_all(&package_bin).expect("package bin directory");
        std::fs::create_dir_all(&package_web).expect("package Web directory");
        std::fs::write(package_bin.join("a3s"), "binary").expect("package binary");
        std::fs::write(package_web.join("index.html"), "web").expect("package Web index");
        let executable = linked_bin.join("a3s");
        symlink(package_bin.join("a3s"), &executable).expect("binary symlink");

        let found = find_existing_web_dir(packaged_candidates(&executable))
            .expect("Web assets beside the canonical executable prefix");

        assert_eq!(
            found,
            package_web.canonicalize().expect("canonical Web path")
        );
    }
}
