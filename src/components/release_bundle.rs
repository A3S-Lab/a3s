use std::collections::BTreeSet;
use std::path::Path;
use std::time::Duration;

use a3s_use_extension::ReleaseBundlePackage;
use anyhow::{bail, Context};
use serde_json::Value;
use tokio::process::Command;
use tokio::time::timeout;

use super::discovery::find_state;
use super::{ComponentId, ComponentPaths};

const CATALOG_TIMEOUT: Duration = Duration::from_secs(15);
const MAX_CATALOG_OUTPUT: usize = 4 * 1024 * 1024;

pub async fn list_release_bundles_with(
    paths: &ComponentPaths,
) -> anyhow::Result<Vec<ReleaseBundlePackage>> {
    let use_id = ComponentId::parse("use")?;
    let state = find_state(&use_id, paths)?;
    let Some(executable) = state.is_ready().then_some(state.path).flatten() else {
        return Ok(Vec::new());
    };
    list_release_bundles(&executable).await
}

pub(crate) async fn resolve_release_bundle(
    paths: &ComponentPaths,
    package_id: &str,
    requested_version: Option<&str>,
    channel: &str,
) -> anyhow::Result<Option<ReleaseBundlePackage>> {
    if channel != "stable" {
        return Ok(None);
    }
    let packages = list_release_bundles_with(paths).await?;
    Ok(packages.into_iter().find(|package| {
        package.package_id == package_id
            && requested_version.is_none_or(|version| version == package.version)
    }))
}

async fn list_release_bundles(executable: &Path) -> anyhow::Result<Vec<ReleaseBundlePackage>> {
    let output = timeout(
        CATALOG_TIMEOUT,
        Command::new(executable)
            .args(["extension", "catalog", "--json"])
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| {
        anyhow::anyhow!(
            "A3S Use release-bundle catalog timed out after {} seconds",
            CATALOG_TIMEOUT.as_secs()
        )
    })?
    .with_context(|| format!("failed to run {}", executable.display()))?;
    if output.stdout.len() > MAX_CATALOG_OUTPUT || output.stderr.len() > MAX_CATALOG_OUTPUT {
        bail!("A3S Use release-bundle catalog exceeded the supported output size");
    }
    let value: Value = serde_json::from_slice(&output.stdout).with_context(|| {
        format!(
            "A3S Use returned invalid release-bundle JSON{}",
            stderr_suffix(&output.stderr)
        )
    })?;
    if !output.status.success() || value.get("ok").and_then(Value::as_bool) != Some(true) {
        let message = value
            .pointer("/error/message")
            .or_else(|| value.get("message"))
            .and_then(Value::as_str)
            .unwrap_or("release-bundle catalog failed");
        bail!("A3S Use release-bundle catalog failed: {message}");
    }
    let packages = value
        .pointer("/data/packages")
        .cloned()
        .context("A3S Use release-bundle catalog has no packages array")?;
    let packages = serde_json::from_value::<Vec<ReleaseBundlePackage>>(packages)
        .context("A3S Use release-bundle catalog has an invalid package entry")?;
    let mut seen = BTreeSet::new();
    for package in &packages {
        package
            .validate()
            .map_err(anyhow::Error::new)
            .with_context(|| {
                format!(
                    "A3S Use returned invalid release-bundle metadata for '{}'",
                    package.package_id
                )
            })?;
        if !seen.insert(package.package_id.as_str()) {
            bail!(
                "A3S Use returned duplicate release bundle '{}'",
                package.package_id
            );
        }
    }
    Ok(packages)
}

fn stderr_suffix(stderr: &[u8]) -> String {
    let value = String::from_utf8_lossy(stderr);
    let value = value.trim().replace(['\n', '\r'], " ");
    if value.is_empty() {
        String::new()
    } else {
        format!(": {}", value.chars().take(500).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[tokio::test]
    async fn reads_and_validates_the_use_release_bundle_catalog() {
        use std::os::unix::fs::PermissionsExt;

        let temp = tempfile::tempdir().unwrap();
        let executable = temp.path().join("a3s-use");
        std::fs::write(
            &executable,
            r#"#!/bin/sh
printf '%s\n' '{"schemaVersion":1,"ok":true,"data":{"packages":[{"schemaVersion":1,"packageId":"a3s/science","componentId":"use/a3s/science","version":"0.1.2","route":"science","packageSha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","fileCount":8,"byteCount":2048,"surfaces":["cli","mcp","skill"],"activityCount":1}]}}'
"#,
        )
        .unwrap();
        std::fs::set_permissions(&executable, std::fs::Permissions::from_mode(0o755)).unwrap();

        let packages = list_release_bundles(&executable).await.unwrap();
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].package_id, "a3s/science");
        assert_eq!(packages[0].byte_count, 2048);
    }
}
