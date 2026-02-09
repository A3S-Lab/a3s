//! GitHub Releases API client.

use serde::Deserialize;

/// A GitHub release.
#[derive(Debug, Deserialize)]
pub struct Release {
    /// Tag name (e.g. `"v0.3.0"`).
    pub tag_name: String,
    /// Release notes body (may be absent).
    pub body: Option<String>,
    /// Attached binary assets.
    pub assets: Vec<Asset>,
}

/// A single release asset (downloadable file).
#[derive(Debug, Deserialize)]
pub struct Asset {
    /// File name (e.g. `"a3s-code-0.3.0-darwin-arm64.tar.gz"`).
    pub name: String,
    /// Direct download URL.
    pub browser_download_url: String,
}

/// Fetch the latest release from a GitHub repository.
pub async fn fetch_latest_release(owner: &str, repo: &str) -> anyhow::Result<Release> {
    let url = format!(
        "https://api.github.com/repos/{}/{}/releases/latest",
        owner, repo
    );

    let client = reqwest::Client::builder()
        .user_agent("a3s-updater/0.1")
        .build()
        .map_err(|e| anyhow::anyhow!("Failed to build HTTP client: {}", e))?;

    let response = client
        .get(&url)
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to fetch latest release from {}: {}", url, e))?;

    if !response.status().is_success() {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        return Err(anyhow::anyhow!(
            "GitHub API returned {} for {}: {}",
            status,
            url,
            body
        ));
    }

    let release: Release = response
        .json()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to parse GitHub release JSON: {}", e))?;

    Ok(release)
}

/// Parse a version string, stripping an optional `v` prefix.
pub fn parse_version(tag: &str) -> anyhow::Result<semver::Version> {
    let version_str = tag.strip_prefix('v').unwrap_or(tag);
    semver::Version::parse(version_str)
        .map_err(|e| anyhow::anyhow!("Failed to parse version '{}': {}", tag, e))
}

/// Build the expected asset file name for a given binary/version/platform.
pub fn asset_name(binary_name: &str, version: &str, os: &str, arch: &str) -> String {
    format!("{}-{}-{}-{}.tar.gz", binary_name, version, os, arch)
}

/// Find the asset matching the current platform from a release's asset list.
pub fn find_matching_asset<'a>(
    release: &'a Release,
    binary_name: &str,
    os: &str,
    arch: &str,
) -> Option<&'a Asset> {
    let version = parse_version(&release.tag_name).ok()?;
    let expected = asset_name(binary_name, &version.to_string(), os, arch);
    release.assets.iter().find(|a| a.name == expected)
}
