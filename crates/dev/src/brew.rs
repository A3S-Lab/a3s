use std::path::PathBuf;

use colored::Colorize;
use futures_util::StreamExt;
use serde::Deserialize;
use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::error::{DevError, Result};

const FORMULA_API: &str = "https://formulae.brew.sh/api/formula";
const HOMEBREW_CELLAR: &str = "/usr/local/Cellar";
const HOMEBREW_CELLAR_ARM: &str = "/opt/homebrew/Cellar";

/// Content-addressable cache directory: ~/.cache/a3s/brew/<sha256>
fn cache_dir() -> PathBuf {
    dirs_next::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("a3s")
        .join("brew")
}

fn cellar() -> PathBuf {
    let arm = PathBuf::from(HOMEBREW_CELLAR_ARM);
    if arm.exists() {
        arm
    } else {
        PathBuf::from(HOMEBREW_CELLAR)
    }
}

#[derive(Debug, Deserialize)]
struct FormulaInfo {
    #[allow(dead_code)]
    name: String,
    versions: Versions,
    bottle: BottleSpec,
}

#[derive(Debug, Deserialize)]
struct Versions {
    stable: String,
}

#[derive(Debug, Deserialize)]
struct BottleSpec {
    stable: BottleStable,
}

#[derive(Debug, Deserialize)]
struct BottleStable {
    files: std::collections::HashMap<String, BottleFile>,
}

#[derive(Debug, Deserialize)]
struct BottleFile {
    url: String,
    sha256: String,
}

/// Fetch formula metadata from Homebrew API.
async fn fetch_formula(client: &reqwest::Client, name: &str) -> Result<FormulaInfo> {
    let url = format!("{FORMULA_API}/{name}.json");
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| DevError::Config(format!("fetch formula '{name}': {e}")))?;

    if !resp.status().is_success() {
        return Err(DevError::Config(format!(
            "formula '{name}' not found (HTTP {})",
            resp.status()
        )));
    }

    resp.json::<FormulaInfo>()
        .await
        .map_err(|e| DevError::Config(format!("parse formula '{name}': {e}")))
}

/// Pick the best bottle for the current platform.
fn pick_bottle(files: &std::collections::HashMap<String, BottleFile>) -> Option<&BottleFile> {
    // Prefer arm64_sequoia → arm64_sonoma → arm64_ventura → sequoia → sonoma → ventura → all
    let preference = [
        "arm64_sequoia",
        "arm64_sonoma",
        "arm64_ventura",
        "arm64_monterey",
        "sequoia",
        "sonoma",
        "ventura",
        "monterey",
        "all",
    ];
    for key in &preference {
        if let Some(f) = files.get(*key) {
            return Some(f);
        }
    }
    // Fall back to first available
    files.values().next()
}

/// Download a bottle to the content-addressable cache.
/// Returns the cached file path. Skips download if already cached.
async fn download_bottle(
    client: &reqwest::Client,
    name: &str,
    bottle: &BottleFile,
) -> Result<PathBuf> {
    let cache = cache_dir();
    fs::create_dir_all(&cache).await?;

    let cached = cache.join(&bottle.sha256);
    if cached.exists() {
        // Verify sha256
        let data = fs::read(&cached).await?;
        let hash = sha256_hex(&data);
        if hash == bottle.sha256 {
            return Ok(cached);
        }
        // Hash mismatch — re-download
        fs::remove_file(&cached).await?;
    }

    // Stream download
    let resp = client
        .get(&bottle.url)
        .send()
        .await
        .map_err(|e| DevError::Config(format!("download '{name}': {e}")))?;

    let tmp = cache.join(format!("{}.tmp", bottle.sha256));
    let mut file = fs::File::create(&tmp).await?;
    let mut stream = resp.bytes_stream();

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|e| DevError::Config(format!("stream '{name}': {e}")))?;
        file.write_all(&chunk).await?;
    }
    file.flush().await?;
    drop(file);

    // Verify hash
    let data = fs::read(&tmp).await?;
    let hash = sha256_hex(&data);
    if hash != bottle.sha256 {
        fs::remove_file(&tmp).await?;
        return Err(DevError::Config(format!(
            "sha256 mismatch for '{name}': expected {}, got {hash}",
            bottle.sha256
        )));
    }

    fs::rename(&tmp, &cached).await?;
    Ok(cached)
}

fn sha256_hex(data: &[u8]) -> String {
    use std::fmt::Write;
    // Simple SHA-256 via ring
    let digest = ring::digest::digest(&ring::digest::SHA256, data);
    let mut s = String::with_capacity(64);
    for b in digest.as_ref() {
        write!(s, "{b:02x}").unwrap();
    }
    s
}

/// Install a single package: download bottle, extract to Cellar, link.
async fn install_one(client: &reqwest::Client, name: &str) -> Result<()> {
    let formula = fetch_formula(client, name).await?;
    let version = &formula.versions.stable;

    // Check if already installed
    let install_path = cellar().join(name).join(version);
    if install_path.exists() {
        println!("  {} {} already installed", "✓".green(), name.cyan());
        return Ok(());
    }

    let bottle = pick_bottle(&formula.bottle.stable.files).ok_or_else(|| {
        DevError::Config(format!("no bottle available for '{name}' on this platform"))
    })?;

    print!("  {} {} {}...", "↓".cyan(), name.cyan(), version.dimmed());

    let cached = download_bottle(client, name, bottle).await?;

    // Extract tar.gz to Cellar
    let cellar = cellar();
    fs::create_dir_all(&cellar).await?;

    // Use system tar for extraction (handles .tar.gz and .tar.xz)
    let status = tokio::process::Command::new("tar")
        .args([
            "-xzf",
            cached.to_str().unwrap(),
            "-C",
            cellar.to_str().unwrap(),
        ])
        .status()
        .await
        .map_err(DevError::Io)?;

    if !status.success() {
        return Err(DevError::Config(format!(
            "failed to extract bottle for '{name}'"
        )));
    }

    // Link via brew (handles keg-only, conflicts, etc.)
    let _ = tokio::process::Command::new("brew")
        .args(["link", "--overwrite", name])
        .status()
        .await;

    println!(" {}", "done".green());
    Ok(())
}

/// Install all packages in parallel.
pub async fn install_packages(packages: &[String]) -> Result<()> {
    if packages.is_empty() {
        return Ok(());
    }

    println!(
        "{} installing {} brew package(s)...",
        "→".cyan(),
        packages.len()
    );

    let client = reqwest::Client::builder()
        .user_agent("a3s-dev/0.1")
        .build()
        .map_err(|e| DevError::Config(e.to_string()))?;

    // Parallel install — fetch all formulas first, then download concurrently
    let mut tasks = tokio::task::JoinSet::new();
    for pkg in packages {
        let client = client.clone();
        let name = pkg.clone();
        tasks.spawn(async move { install_one(&client, &name).await });
    }

    let mut failed = vec![];
    while let Some(res) = tasks.join_next().await {
        match res {
            Ok(Ok(())) => {}
            Ok(Err(e)) => failed.push(e.to_string()),
            Err(e) => failed.push(e.to_string()),
        }
    }

    if !failed.is_empty() {
        return Err(DevError::Config(format!(
            "brew install failed:\n{}",
            failed.join("\n")
        )));
    }

    Ok(())
}

/// Check which packages are not yet installed.
pub fn missing_packages(packages: &[String]) -> Vec<String> {
    packages
        .iter()
        .filter(|pkg| {
            let path = cellar().join(pkg.as_str());
            !path.exists()
        })
        .cloned()
        .collect()
}

/// Uninstall a package via brew.
pub async fn uninstall_package(name: &str) -> Result<()> {
    let status = tokio::process::Command::new("brew")
        .args(["uninstall", name])
        .status()
        .await?;

    if !status.success() {
        return Err(DevError::Config(format!("brew uninstall '{name}' failed")));
    }
    Ok(())
}

/// Search Homebrew formulae by name/description via the API.
pub async fn search_packages(query: &str) -> Result<()> {
    use colored::Colorize;

    let client = reqwest::Client::builder()
        .user_agent("a3s-dev/0.1")
        .build()
        .map_err(|e| DevError::Config(e.to_string()))?;

    // Homebrew search API returns a list of formula names matching the query
    let url = "https://formulae.brew.sh/api/formula.json".to_string();
    let resp = client
        .get(&url)
        .send()
        .await
        .map_err(|e| DevError::Config(format!("search failed: {e}")))?;

    #[derive(serde::Deserialize)]
    struct FormulaEntry {
        name: String,
        desc: Option<String>,
    }

    let entries: Vec<FormulaEntry> = resp
        .json()
        .await
        .map_err(|e| DevError::Config(format!("parse search results: {e}")))?;

    let q = query.to_lowercase();
    let matches: Vec<&FormulaEntry> = entries
        .iter()
        .filter(|e| {
            e.name.to_lowercase().contains(&q)
                || e.desc.as_deref().unwrap_or("").to_lowercase().contains(&q)
        })
        .take(20)
        .collect();

    if matches.is_empty() {
        println!("{} no results for '{query}'", "·".dimmed());
        return Ok(());
    }

    println!("{} results for '{}':", matches.len(), query.cyan());
    for entry in matches {
        let desc = entry.desc.as_deref().unwrap_or("");
        println!("  {:<28} {}", entry.name.cyan(), desc.dimmed());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_add_to_list_new() {
        let mut pkgs = vec!["redis".to_string()];
        assert!(add_to_list(&mut pkgs, "postgres"));
        assert_eq!(pkgs, vec!["redis", "postgres"]);
    }

    #[test]
    fn test_add_to_list_duplicate() {
        let mut pkgs = vec!["redis".to_string()];
        assert!(!add_to_list(&mut pkgs, "redis"));
        assert_eq!(pkgs.len(), 1);
    }

    #[test]
    fn test_remove_from_list_existing() {
        let mut pkgs = vec!["redis".to_string(), "postgres".to_string()];
        assert!(remove_from_list(&mut pkgs, "redis"));
        assert_eq!(pkgs, vec!["postgres"]);
    }

    #[test]
    fn test_remove_from_list_missing() {
        let mut pkgs = vec!["redis".to_string()];
        assert!(!remove_from_list(&mut pkgs, "postgres"));
        assert_eq!(pkgs.len(), 1);
    }

    #[test]
    fn test_write_brew_block_replace() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("A3sfile.hcl");
        std::fs::write(&path, "brew {\n  packages = [\n  \"redis\",\n  ]\n}\n").unwrap();
        write_brew_block(&path, &["redis".to_string(), "postgres".to_string()]).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("\"redis\""));
        assert!(result.contains("\"postgres\""));
    }

    #[test]
    fn test_write_brew_block_append() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("A3sfile.hcl");
        std::fs::write(&path, "# empty\n").unwrap();
        write_brew_block(&path, &["redis".to_string()]).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(result.contains("brew {"));
        assert!(result.contains("\"redis\""));
    }

    #[test]
    fn test_write_brew_block_empty_removes_block() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("A3sfile.hcl");
        std::fs::write(&path, "brew {\n  packages = [\n  \"redis\",\n  ]\n}\n").unwrap();
        write_brew_block(&path, &[]).unwrap();
        let result = std::fs::read_to_string(&path).unwrap();
        assert!(!result.contains("brew {"));
    }
}

/// Add a package to the packages list (deduplicates).
pub fn add_to_list(packages: &mut Vec<String>, name: &str) -> bool {
    if packages.iter().any(|p| p == name) {
        return false;
    }
    packages.push(name.to_string());
    true
}

/// Remove a package from the packages list.
pub fn remove_from_list(packages: &mut Vec<String>, name: &str) -> bool {
    let before = packages.len();
    packages.retain(|p| p != name);
    packages.len() < before
}

/// Rewrite the brew.packages list in A3sfile.hcl.
/// Simple approach: regenerate the brew block in-place.
pub fn write_brew_block(path: &std::path::Path, packages: &[String]) -> Result<()> {
    let src = std::fs::read_to_string(path)
        .map_err(|e| DevError::Config(format!("read {}: {e}", path.display())))?;

    let new_block = if packages.is_empty() {
        String::new()
    } else {
        let list = packages
            .iter()
            .map(|p| format!("  \"{p}\""))
            .collect::<Vec<_>>()
            .join(",\n");
        format!("brew {{\n  packages = [\n{list},\n  ]\n}}\n")
    };

    // Replace existing brew block or append
    let result = if let Some(start) = src.find("brew {") {
        // Find matching closing brace
        let after = &src[start..];
        let mut depth = 0usize;
        let mut end = start;
        for (i, ch) in after.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = start + i + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        // Consume trailing newline
        let end = if src.as_bytes().get(end) == Some(&b'\n') {
            end + 1
        } else {
            end
        };
        format!("{}{}{}", &src[..start], new_block, &src[end..])
    } else {
        format!("{new_block}\n{src}")
    };

    std::fs::write(path, result)
        .map_err(|e| DevError::Config(format!("write {}: {e}", path.display())))?;
    Ok(())
}
