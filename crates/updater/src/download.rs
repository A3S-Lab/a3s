//! Download and extract release assets.

use std::path::{Path, PathBuf};

/// Download a `.tar.gz` asset and extract the binary from it.
///
/// Returns the path to the extracted binary inside a temporary directory.
pub async fn download_and_extract(url: &str, binary_name: &str) -> anyhow::Result<PathBuf> {
    let client = reqwest::Client::builder()
        .user_agent("a3s-updater/0.1")
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

    // Create a temp directory for extraction
    let temp_dir = std::env::temp_dir().join(format!("a3s-update-{}", std::process::id()));
    tokio::fs::create_dir_all(&temp_dir).await.map_err(|e| {
        anyhow::anyhow!(
            "Failed to create temp directory {}: {}",
            temp_dir.display(),
            e
        )
    })?;

    // Decompress gzip and extract tar
    extract_tar_gz(&bytes, &temp_dir, binary_name)?;

    let binary_path = temp_dir.join(binary_name);
    if !binary_path.exists() {
        return Err(anyhow::anyhow!(
            "Binary '{}' not found in downloaded archive",
            binary_name
        ));
    }

    Ok(binary_path)
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
