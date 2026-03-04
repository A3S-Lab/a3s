//! Download and extract release assets.

use std::path::{Path, PathBuf};
use tempfile::TempDir;

/// Download a `.tar.gz` asset and extract the binary from it.
///
/// Returns `(binary_path, temp_dir)`. The caller must keep `TempDir` alive
/// until the extracted binary has been consumed (e.g. copied into place).
/// Dropping `TempDir` automatically cleans up the temporary directory.
pub async fn download_and_extract(
    url: &str,
    binary_name: &str,
) -> anyhow::Result<(PathBuf, TempDir)> {
    let client = reqwest::Client::builder()
        .user_agent("a3s-updater/0.1")
        .timeout(std::time::Duration::from_secs(300))
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to download asset from {}: {}", url, e))?;

    if !response.status().is_success() {
        return Err(anyhow::anyhow!(
            "Download failed with status {} for {}",
            response.status(),
            url
        ));
    }

    let bytes = response
        .bytes()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to read download body: {}", e))?;

    // Create a cryptographically unique temp directory (auto-cleaned on drop)
    let temp_dir = TempDir::new()
        .map_err(|e| anyhow::anyhow!("Failed to create temp directory: {}", e))?;

    // Decompress gzip and extract tar
    extract_tar_gz(&bytes, temp_dir.path(), binary_name)?;

    let binary_path = temp_dir.path().join(binary_name);
    if !binary_path.exists() {
        return Err(anyhow::anyhow!(
            "Binary '{}' not found in downloaded archive",
            binary_name
        ));
    }

    Ok((binary_path, temp_dir))
}

/// Extract a `.tar.gz` archive, looking for the target binary.
fn extract_tar_gz(data: &[u8], dest_dir: &Path, binary_name: &str) -> anyhow::Result<()> {
    let gz = flate2::read::GzDecoder::new(data);
    let mut archive = tar::Archive::new(gz);

    for entry in archive
        .entries()
        .map_err(|e| anyhow::anyhow!("Failed to read tar entries: {}", e))?
    {
        let mut entry = entry.map_err(|e| anyhow::anyhow!("Failed to read tar entry: {}", e))?;

        // Only extract regular files — reject symlinks and hardlinks to prevent
        // path traversal via crafted archives.
        let entry_type = entry.header().entry_type();
        if !entry_type.is_file() {
            continue;
        }

        let path = entry
            .path()
            .map_err(|e| anyhow::anyhow!("Failed to read entry path: {}", e))?;

        // Extract only the target binary (may be at root or nested)
        let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");

        if file_name == binary_name {
            entry
                .unpack(dest_dir.join(binary_name))
                .map_err(|e| anyhow::anyhow!("Failed to extract '{}': {}", binary_name, e))?;
            return Ok(());
        }
    }

    Err(anyhow::anyhow!(
        "Binary '{}' not found in archive",
        binary_name
    ))
}
