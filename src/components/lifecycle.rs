use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use a3s_updater::{
    parse_version, uninstall_owned_files, ComponentReceipt, InstallProvenance,
    RECEIPT_SCHEMA_VERSION,
};
use a3s_use_extension::ReleaseBundlePackage;
use anyhow::{bail, Context};
use serde::Serialize;

use super::catalog::{self, ComponentSpec, Distribution, ReleaseSpec};
use super::discovery::find_state;
use super::id::ComponentId;
use super::lock::ComponentOperationLock;
use super::paths::ComponentPaths;
use super::probe::probe_release;
use super::release_install::{install_release, ResolvedRelease};
use super::state::{ComponentState, Health, Presence};
use crate::registry::ResolvedRegistryPackage;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InstallSource {
    #[default]
    Auto,
    Homebrew,
    Release,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InstallIntent {
    #[default]
    Install,
    Upgrade,
}

impl InstallIntent {
    pub fn action(self) -> &'static str {
        match self {
            Self::Install => "install",
            Self::Upgrade => "upgrade",
        }
    }
}

#[derive(Debug, Clone)]
pub struct InstallRequest {
    pub version: Option<String>,
    pub source: InstallSource,
    pub intent: InstallIntent,
    pub package: Option<PathBuf>,
    pub force: bool,
    pub allow_unsigned: bool,
    pub progress: bool,
    pub resolved_releases: BTreeMap<String, ResolvedRelease>,
    pub resolved_sources: BTreeMap<String, InstallSource>,
    pub resolved_release_bundles: BTreeMap<String, ReleaseBundlePackage>,
    pub resolved_registry_packages: BTreeMap<String, ResolvedRegistryPackage>,
}

impl Default for InstallRequest {
    fn default() -> Self {
        Self {
            version: None,
            source: InstallSource::Auto,
            intent: InstallIntent::Install,
            package: None,
            force: false,
            allow_unsigned: false,
            progress: true,
            resolved_releases: BTreeMap::new(),
            resolved_sources: BTreeMap::new(),
            resolved_release_bundles: BTreeMap::new(),
            resolved_registry_packages: BTreeMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OperationRecord {
    pub component: ComponentId,
    pub action: &'static str,
    pub changed: bool,
    #[serde(skip_serializing_if = "is_false")]
    pub recovered: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provenance: Option<InstallProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<PathBuf>,
    pub message: String,
}

fn is_false(value: &bool) -> bool {
    !*value
}

pub async fn install_component(
    id: &ComponentId,
    request: &InstallRequest,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    let _lock = ComponentOperationLock::acquire(paths.operation_lock_path(id), id).await?;
    install_component_locked(id, request, paths).await
}

pub(super) async fn install_component_locked(
    id: &ComponentId,
    request: &InstallRequest,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    if let Some(spec) = catalog::find(id) {
        if request.package.is_some() {
            bail!("--from is valid only for external Use extensions");
        }
        if request.allow_unsigned {
            bail!("--allow-unsigned is valid only for external Use extensions");
        }
        if request.version.is_some() && !matches!(spec.distribution, Distribution::Release(_)) {
            bail!(
                "component '{}' does not own a versioned release; --version is not supported",
                id
            );
        }
        if request.source != InstallSource::Auto
            && !matches!(spec.distribution, Distribution::Release(_))
        {
            bail!(
                "component '{}' does not own an install source; --source is not supported",
                id
            );
        }
        return match spec.distribution {
            Distribution::Bundled => install_bundled(id, spec, request, paths),
            Distribution::Release(release) => {
                if request.package.is_some() {
                    bail!("--from is valid only for external Use extensions");
                }
                install_product(id, spec, release, request, paths).await
            }
            Distribution::Delegated { parent } => {
                let parent = ComponentId::parse(parent)?;
                let parent_path = ensure_parent(&parent, paths, request).await?;
                delegate_install(id, &parent, &parent_path, request)
            }
        };
    }

    let use_id = ComponentId::parse("use")?;
    if !id.is_child_of(&use_id) || id.as_str().split('/').count() < 3 {
        bail!("component '{}' is not registered", id);
    }
    if request.source != InstallSource::Auto {
        bail!(
            "external component '{}' is resolved through its package source; --source is not supported",
            id
        );
    }
    if request.package.is_some() {
        if !request.allow_unsigned {
            bail!(
                "external component '{}' uses an unsigned local package; rerun with --allow-unsigned",
                id
            );
        }
        if request.version.is_some() {
            bail!(
                "external component '{}' derives its version from the local package manifest; --version is not supported",
                id
            );
        }
    } else {
        if request.allow_unsigned {
            bail!("--allow-unsigned is valid only with an explicit local --from package");
        }
        match (
            request.resolved_release_bundles.get(id.as_str()),
            request.resolved_registry_packages.get(id.as_str()),
        ) {
            (Some(bundle), None) => validate_release_bundle_resolution(id, bundle)?,
            (None, Some(registry)) => validate_registry_resolution(id, registry)?,
            (None, None) => bail!(
                "external component '{}' has no reviewed release-bundle or signed-registry resolution",
                id
            ),
            (Some(_), Some(_)) => bail!(
                "external component '{}' resolved more than one package source",
                id
            ),
        }
    }
    let parent_path = ensure_parent(&use_id, paths, request).await?;
    delegate_install(id, &use_id, &parent_path, request)
}

#[cfg(test)]
pub fn uninstall_component(
    id: &ComponentId,
    cascade: bool,
    purge: bool,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    let _lock = ComponentOperationLock::acquire_sync(&paths.operation_lock_path(id), id)?;
    uninstall_component_locked(id, cascade, purge, paths)
}

pub(super) fn uninstall_component_locked(
    id: &ComponentId,
    cascade: bool,
    purge: bool,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    if let Some(spec) = catalog::find(id) {
        if !spec.removable {
            bail!("component '{}' is bundled and cannot be uninstalled", id);
        }
        if let Distribution::Delegated { parent } = spec.distribution {
            let parent = ComponentId::parse(parent)?;
            let parent_state = find_state(&parent, paths)?;
            let parent_path = ready_path(&parent_state)?;
            return delegate_uninstall(id, &parent, &parent_path);
        }
    } else {
        let use_id = ComponentId::parse("use")?;
        if !id.is_child_of(&use_id) || id.as_str().split('/').count() < 3 {
            bail!("component '{}' is not registered", id);
        }
        let parent_state = find_state(&use_id, paths)?;
        let parent_path = ready_path(&parent_state)?;
        return delegate_uninstall(id, &use_id, &parent_path);
    }

    let store = paths.receipt_store();
    let receipt = store
        .read(id.as_str())?
        .with_context(|| ownership_error(id, paths))?;
    let children = store
        .list()?
        .into_iter()
        .filter(|candidate| {
            ComponentId::parse(&candidate.component_id)
                .map(|candidate_id| candidate_id.is_child_of(id))
                .unwrap_or(false)
        })
        .collect::<Vec<_>>();
    if !children.is_empty() && !cascade {
        bail!(
            "component '{}' has managed children; rerun with --cascade",
            id
        );
    }
    if cascade {
        for child in children.into_iter().rev() {
            let child_id = ComponentId::parse(&child.component_id)?;
            uninstall_component_locked(&child_id, true, purge, paths)?;
        }
    }

    match receipt.provenance {
        InstallProvenance::Homebrew => uninstall_homebrew(id, &receipt, paths)?,
        provenance if provenance.owns_files() => {
            stop_owned_service(id, &receipt)?;
            uninstall_owned_files(&receipt, &paths.data_root)?;
            store.remove(id.as_str())?;
        }
        _ => bail!(ownership_error(id, paths)),
    }
    if purge {
        let cache = paths.cache_dir(id);
        if cache.exists() {
            std::fs::remove_dir_all(&cache)
                .with_context(|| format!("failed to remove cache {}", cache.display()))?;
        }
    }
    Ok(OperationRecord {
        component: id.clone(),
        action: "uninstall",
        changed: true,
        recovered: false,
        version: Some(receipt.version),
        provenance: Some(receipt.provenance),
        path: receipt.executable_path,
        message: format!("Uninstalled component '{}'.", id),
    })
}

async fn ensure_parent(
    parent: &ComponentId,
    paths: &ComponentPaths,
    request: &InstallRequest,
) -> anyhow::Result<PathBuf> {
    let state = find_state(parent, paths)?;
    if state.is_ready() {
        return ready_path(&state);
    }
    let spec = catalog::find(parent)
        .with_context(|| format!("parent component '{}' is not registered", parent))?;
    let release = catalog::release(spec)
        .with_context(|| format!("parent component '{}' is not installable", parent))?;
    let parent_request = InstallRequest {
        progress: request.progress,
        resolved_releases: request.resolved_releases.clone(),
        resolved_sources: request.resolved_sources.clone(),
        ..InstallRequest::default()
    };
    let record = install_product(parent, spec, release, &parent_request, paths).await?;
    record
        .path
        .context("installed parent did not report an executable path")
}

fn install_bundled(
    id: &ComponentId,
    spec: &ComponentSpec,
    request: &InstallRequest,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    let state = find_state(id, paths)?;
    if !state.is_ready() {
        bail!("bundled component '{}' failed its health check", id);
    }
    Ok(OperationRecord {
        component: id.clone(),
        action: request.intent.action(),
        changed: false,
        recovered: false,
        version: state.version,
        provenance: state.provenance,
        path: state.path,
        message: format!("{} is bundled with a3s.", spec.description),
    })
}

async fn install_product(
    id: &ComponentId,
    _spec: &ComponentSpec,
    release: ReleaseSpec,
    request: &InstallRequest,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    let state = find_state(id, paths)?;
    let requested_version_is_ready = request.version.as_deref().is_none_or(|requested| {
        state
            .version
            .as_deref()
            .is_some_and(|installed| parse_version(installed).ok() == parse_version(requested).ok())
    });
    if state.is_ready() && requested_version_is_ready && !request.force {
        return Ok(OperationRecord {
            component: id.clone(),
            action: request.intent.action(),
            changed: false,
            recovered: false,
            version: state.version,
            provenance: state.provenance,
            path: state.path,
            message: format!("Component '{}' is already ready.", id),
        });
    }
    if state.health == Health::Broken && !request.force {
        bail!(
            "component '{}' is broken; rerun install with --force to repair it",
            id
        );
    }

    let source = resolve_install_source(id, release, request)?;
    match source {
        InstallSource::Homebrew => install_homebrew(id, release, request, paths),
        InstallSource::Release => {
            install_release(
                id,
                release,
                request,
                request.resolved_releases.get(id.as_str()),
                paths,
            )
            .await
        }
        InstallSource::Auto => {
            bail!("automatic install source was not resolved")
        }
    }
}

pub(super) fn resolve_install_source(
    id: &ComponentId,
    release: ReleaseSpec,
    request: &InstallRequest,
) -> anyhow::Result<InstallSource> {
    if let Some(resolved) = request.resolved_sources.get(id.as_str()).copied() {
        if request.source != InstallSource::Auto && request.source != resolved {
            bail!(
                "reviewed install source {:?} does not match requested source {:?}",
                resolved,
                request.source
            );
        }
        return Ok(resolved);
    }
    Ok(match request.source {
        InstallSource::Auto
            if request.version.is_none()
                && release.homebrew_formula.is_some()
                && a3s_is_homebrew_managed() =>
        {
            InstallSource::Homebrew
        }
        InstallSource::Auto => InstallSource::Release,
        explicit => explicit,
    })
}

fn install_homebrew(
    id: &ComponentId,
    release: ReleaseSpec,
    request: &InstallRequest,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationRecord> {
    if request.version.is_some() {
        bail!(
            "component '{}' cannot select an exact version through Homebrew; use --source release",
            id
        );
    }
    let formula = release
        .homebrew_formula
        .with_context(|| format!("component '{}' has no Homebrew formula", id))?;
    let verb = match (request.intent, request.force) {
        (InstallIntent::Upgrade, _) => "upgrade",
        (InstallIntent::Install, true) => "reinstall",
        (InstallIntent::Install, false) => "install",
    };
    super::progress(
        request.progress,
        format!("a3s: {} '{}' with Homebrew...", verb, id),
    );
    let status = Command::new("brew")
        .args([verb, formula])
        .status()
        .context("failed to run Homebrew")?;
    if !status.success() {
        bail!("Homebrew failed to {verb} '{}'", id);
    }
    let prefix_output = Command::new("brew")
        .args(["--prefix", formula])
        .output()
        .context("failed to query Homebrew prefix")?;
    if !prefix_output.status.success() {
        bail!("Homebrew installed '{}', but its prefix is unavailable", id);
    }
    let prefix = PathBuf::from(String::from_utf8(prefix_output.stdout)?.trim());
    let executable = prefix.join("bin").join(release.binary);
    let version = match probe_release(release, &executable)? {
        Some(version) => version,
        None => homebrew_formula_version(formula)?,
    };
    let receipt = ComponentReceipt {
        schema_version: RECEIPT_SCHEMA_VERSION,
        component_id: id.to_string(),
        version: version.clone(),
        provenance: InstallProvenance::Homebrew,
        install_root: prefix,
        executable_path: Some(executable.clone()),
        owned_paths: Vec::new(),
        source: Some(formula.to_string()),
        artifact_checksums: BTreeMap::new(),
        installed_at: chrono::Utc::now().to_rfc3339(),
    };
    paths.receipt_store().write(&receipt)?;
    Ok(OperationRecord {
        component: id.clone(),
        action: request.intent.action(),
        changed: true,
        recovered: false,
        version: Some(version),
        provenance: Some(InstallProvenance::Homebrew),
        path: Some(executable),
        message: format!("Homebrew completed {verb} for component '{}'.", id),
    })
}

fn homebrew_formula_version(formula: &str) -> anyhow::Result<String> {
    let output = Command::new("brew")
        .args(["list", "--versions", formula])
        .output()
        .context("failed to query the installed Homebrew formula version")?;
    if !output.status.success() {
        bail!("Homebrew installed '{formula}', but its version is unavailable");
    }
    let output = String::from_utf8(output.stdout)?;
    output
        .split_whitespace()
        .rev()
        .find(|field| parse_version(field).is_ok())
        .map(str::to_string)
        .with_context(|| format!("Homebrew returned no installed version for formula '{formula}'"))
}

fn delegate_install(
    id: &ComponentId,
    parent: &ComponentId,
    parent_path: &Path,
    request: &InstallRequest,
) -> anyhow::Result<OperationRecord> {
    let relative = id
        .relative_to(parent)
        .context("delegated component is outside its parent namespace")?;
    let mut command = Command::new(parent_path);
    command.args(["component", "install", relative, "--json"]);
    if let Some(package) = &request.package {
        command.arg("--from").arg(package);
    }
    if request.force {
        command.arg("--force");
    }
    if request.allow_unsigned {
        command.arg("--allow-unsigned");
    }
    if let Some(resolved) = request.resolved_registry_packages.get(id.as_str()) {
        validate_registry_resolution(id, resolved)?;
        let plan_digest = resolved.package.plan_digest().map_err(anyhow::Error::new)?;
        command
            .arg("--registry-name")
            .arg(&resolved.registry.name)
            .arg("--registry-url")
            .arg(&resolved.registry.url)
            .arg("--trust-root")
            .arg(&resolved.registry.trust_root)
            .arg("--version")
            .arg(&resolved.package.version)
            .arg("--channel")
            .arg(&resolved.package.channel)
            .arg("--registry-plan-digest")
            .arg(plan_digest);
        if let Some(path) = &resolved.registry.trusted_root_path {
            command.arg("--trusted-root").arg(path);
        }
    }
    if let Some(bundle) = request.resolved_release_bundles.get(id.as_str()) {
        validate_release_bundle_resolution(id, bundle)?;
        command
            .arg("--release-bundle-sha256")
            .arg(&bundle.package_sha256);
    }
    let output = command.output().with_context(|| {
        format!(
            "failed to delegate install to parent component '{}'",
            parent
        )
    })?;
    if !output.status.success() {
        bail!(delegated_failure(parent, "install", &output));
    }
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .context("parent install returned invalid JSON")?;
    validate_delegated_success(&value, "install")?;
    let data = value.get("data").unwrap_or(&value);
    let component = data.get("component");
    Ok(OperationRecord {
        component: id.clone(),
        action: request.intent.action(),
        changed: data
            .get("changed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        recovered: false,
        version: component
            .and_then(|value| value.get("version"))
            .and_then(serde_json::Value::as_str)
            .map(str::to_string),
        provenance: Some(InstallProvenance::Delegated),
        path: component
            .and_then(|value| value.get("path"))
            .and_then(serde_json::Value::as_str)
            .map(PathBuf::from),
        message: format!("Parent component '{}' installed '{}'.", parent, id),
    })
}

fn validate_registry_resolution(
    id: &ComponentId,
    resolved: &ResolvedRegistryPackage,
) -> anyhow::Result<()> {
    let parent = ComponentId::parse("use")?;
    let package_id = id
        .relative_to(&parent)
        .context("signed registry package is outside the Use namespace")?;
    if resolved.package.package_id != package_id {
        bail!(
            "reviewed registry package '{}' does not match component '{}'",
            resolved.package.package_id,
            id
        );
    }
    if resolved.package.registry_name != resolved.registry.name
        || resolved.package.registry_url != resolved.registry.url
        || resolved.package.root_sha256
            != resolved.registry.trust_root.trim_start_matches("sha256:")
    {
        bail!("reviewed registry package provenance is internally inconsistent");
    }
    Ok(())
}

fn validate_release_bundle_resolution(
    id: &ComponentId,
    bundle: &ReleaseBundlePackage,
) -> anyhow::Result<()> {
    bundle.validate().map_err(anyhow::Error::new)?;
    let parent = ComponentId::parse("use")?;
    let package_id = id
        .relative_to(&parent)
        .context("release-bundled extension is outside the Use namespace")?;
    if bundle.package_id != package_id || bundle.component_id != id.as_str() {
        bail!(
            "reviewed release bundle '{}' does not match component '{}'",
            bundle.package_id,
            id
        );
    }
    Ok(())
}

fn delegate_uninstall(
    id: &ComponentId,
    parent: &ComponentId,
    parent_path: &Path,
) -> anyhow::Result<OperationRecord> {
    let relative = id
        .relative_to(parent)
        .context("delegated component is outside its parent namespace")?;
    let output = Command::new(parent_path)
        .args(["component", "uninstall", relative, "--json"])
        .output()
        .with_context(|| {
            format!(
                "failed to delegate uninstall to parent component '{}'",
                parent
            )
        })?;
    if !output.status.success() {
        bail!(delegated_failure(parent, "uninstall", &output));
    }
    let value = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .context("parent uninstall returned invalid JSON")?;
    validate_delegated_success(&value, "uninstall")?;
    let data = value.get("data").unwrap_or(&value);
    Ok(OperationRecord {
        component: id.clone(),
        action: "uninstall",
        changed: data
            .get("changed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(true),
        recovered: false,
        version: None,
        provenance: Some(InstallProvenance::Delegated),
        path: None,
        message: format!("Parent component '{}' uninstalled '{}'.", parent, id),
    })
}

fn validate_delegated_success(value: &serde_json::Value, action: &str) -> anyhow::Result<()> {
    if value
        .get("schemaVersion")
        .and_then(serde_json::Value::as_u64)
        != Some(1)
    {
        bail!("parent {action} returned an incompatible CLI schema");
    }
    if value.get("ok").and_then(serde_json::Value::as_bool) == Some(false) {
        bail!("parent {action} returned an error response with a successful exit status");
    }
    Ok(())
}

fn delegated_failure(parent: &ComponentId, action: &str, output: &Output) -> String {
    let machine_message = serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .ok()
        .and_then(|value| {
            value
                .get("error")
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string)
        });
    let message = machine_message.unwrap_or_else(|| {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        if stderr.trim().is_empty() {
            stdout.trim().to_string()
        } else {
            stderr.trim().to_string()
        }
    });
    format!("parent component '{parent}' rejected {action}: {message}")
}

fn uninstall_homebrew(
    id: &ComponentId,
    receipt: &ComponentReceipt,
    paths: &ComponentPaths,
) -> anyhow::Result<()> {
    let formula = receipt
        .source
        .as_deref()
        .context("Homebrew receipt has no formula")?;
    let status = Command::new("brew")
        .args(["uninstall", formula])
        .status()
        .context("failed to run Homebrew")?;
    if !status.success() {
        bail!("Homebrew failed to uninstall '{}'", id);
    }
    paths.receipt_store().remove(id.as_str())
}

fn stop_owned_service(id: &ComponentId, receipt: &ComponentReceipt) -> anyhow::Result<()> {
    if id.as_str() != "use" {
        return Ok(());
    }
    let Some(executable) = &receipt.executable_path else {
        return Ok(());
    };
    let output = Command::new(executable)
        .args(["mcp", "stop", "--json"])
        .output()
        .context("failed to stop the a3s-use MCP service")?;
    if !output.status.success() {
        bail!(
            "a3s-use MCP service did not stop safely: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    serde_json::from_slice::<serde_json::Value>(&output.stdout)
        .context("a3s-use MCP stop returned invalid JSON")?;
    Ok(())
}

fn ready_path(state: &ComponentState) -> anyhow::Result<PathBuf> {
    if state.presence == Presence::Missing || !state.is_ready() {
        bail!("component '{}' is not ready", state.id);
    }
    state
        .path
        .clone()
        .with_context(|| format!("component '{}' has no executable path", state.id))
}

fn ownership_error(id: &ComponentId, paths: &ComponentPaths) -> String {
    match find_state(id, paths) {
        Ok(state) if state.presence != Presence::Missing => format!(
            "component '{}' is present at {}, but A3S does not own it",
            id,
            state
                .path
                .as_deref()
                .map(Path::display)
                .map(|value| value.to_string())
                .unwrap_or_else(|| "an external location".to_string())
        ),
        _ => format!("component '{}' is not installed", id),
    }
}

fn a3s_is_homebrew_managed() -> bool {
    Command::new("brew")
        .args(["list", "--versions", "a3s"])
        .output()
        .map(|output| output.status.success() && !output.stdout.is_empty())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_request_defaults_to_safe_release_selection() {
        let request = InstallRequest::default();
        assert_eq!(request.source, InstallSource::Auto);
        assert!(!request.force);
        assert!(request.package.is_none());
    }
}
