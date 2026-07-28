use std::collections::BTreeMap;
use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};

use a3s_updater::{parse_version, ComponentReceipt, InstallProvenance};
use a3s_use_extension::{ReleaseBundlePackage, ResolvedRemotePackage};
use anyhow::{bail, Context};
use serde::Serialize;
use sha2::{Digest, Sha256};

use super::catalog::{self, Distribution};
use super::discovery::{
    discover, extension_registry_provenance, extension_release_bundle_provenance,
};
use super::id::ComponentId;
use super::lifecycle::{resolve_install_source, InstallRequest, InstallSource};
use super::paths::ComponentPaths;
use super::release_bundle::resolve_release_bundle;
use super::release_install::{resolve_release, ResolvedRelease};
use super::state::{ComponentState, Health, Presence, Trust};
use crate::registry::{RegistryStore, ResolvedRegistryPackage};

const PLAN_SCHEMA_VERSION: u32 = 1;
const PLAN_DIGEST_DOMAIN: &[u8] = b"a3s-component-plan-v1\0";
const LOCAL_PACKAGE_DIGEST_DOMAIN: &[u8] = b"a3s-component-local-package-v1\0";
const MAX_LOCAL_ARCHIVE_BYTES: u64 = 512 * 1024 * 1024;
const MAX_LOCAL_PACKAGE_FILES: u64 = 10_000;
const MAX_LOCAL_PACKAGE_BYTES: u64 = 1_073_741_824;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedPath {
    display: String,
    identity: String,
}

impl PlannedPath {
    fn new(path: &std::path::Path) -> Self {
        Self {
            display: path.to_string_lossy().into_owned(),
            identity: path_identity(path),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedLocalPackage {
    path: PlannedPath,
    kind: &'static str,
    sha256: String,
    file_count: u64,
    byte_count: u64,
}

#[derive(Debug, Clone)]
pub(super) struct PreparedOperationPlan {
    pub(super) plan: OperationPlan,
    pub(super) resolved_releases: BTreeMap<String, ResolvedRelease>,
    pub(super) resolved_sources: BTreeMap<String, InstallSource>,
    pub(super) resolved_release_bundles: BTreeMap<String, ReleaseBundlePackage>,
    pub(super) resolved_registry_packages: BTreeMap<String, ResolvedRegistryPackage>,
    pub(super) apply_force: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct PlannedCurrentState {
    presence: Presence,
    health: Health,
    #[serde(skip_serializing_if = "Option::is_none")]
    provenance: Option<InstallProvenance>,
    #[serde(skip_serializing_if = "Option::is_none")]
    version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<PlannedPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    receipt: Option<PlannedReceipt>,
}

impl From<&ComponentState> for PlannedCurrentState {
    fn from(state: &ComponentState) -> Self {
        Self {
            presence: state.presence,
            health: state.health,
            provenance: state.provenance,
            version: state.version.clone(),
            path: state.path.as_deref().map(PlannedPath::new),
            receipt: None,
        }
    }
}

impl PlannedCurrentState {
    fn with_receipt(state: &ComponentState, paths: &ComponentPaths) -> anyhow::Result<Self> {
        let mut planned = Self::from(state);
        planned.receipt = paths
            .receipt_store()
            .read(state.id.as_str())?
            .as_ref()
            .map(PlannedReceipt::from);
        Ok(planned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
struct PlannedReceipt {
    schema_version: u32,
    component_id: String,
    version: String,
    provenance: InstallProvenance,
    install_root: PlannedPath,
    #[serde(skip_serializing_if = "Option::is_none")]
    executable_path: Option<PlannedPath>,
    owned_paths: Vec<PlannedPath>,
    #[serde(skip_serializing_if = "Option::is_none")]
    source: Option<String>,
    artifact_checksums: BTreeMap<String, String>,
}

impl From<&ComponentReceipt> for PlannedReceipt {
    fn from(receipt: &ComponentReceipt) -> Self {
        let mut owned_paths = receipt
            .owned_paths
            .iter()
            .map(|path| PlannedPath::new(path))
            .collect::<Vec<_>>();
        owned_paths.sort_by(|left, right| left.identity.cmp(&right.identity));
        Self {
            schema_version: receipt.schema_version,
            component_id: receipt.component_id.clone(),
            version: receipt.version.clone(),
            provenance: receipt.provenance,
            install_root: PlannedPath::new(&receipt.install_root),
            executable_path: receipt.executable_path.as_deref().map(PlannedPath::new),
            owned_paths,
            source: receipt.source.clone(),
            artifact_checksums: receipt.artifact_checksums.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OperationPlan {
    schema_version: u32,
    component: ComponentId,
    action: &'static str,
    source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_source: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    migration: Option<bool>,
    target: String,
    ownership: String,
    mutates: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    requested_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    local_package: Option<PlannedLocalPackage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    resolved_sources: BTreeMap<String, String>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    resolved_releases: BTreeMap<String, ResolvedRelease>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    resolved_release_bundles: BTreeMap<String, ReleaseBundlePackage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    resolved_registry_packages: BTreeMap<String, ResolvedRemotePackage>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    prerequisites: BTreeMap<String, PlannedCurrentState>,
    #[serde(skip_serializing_if = "Option::is_none")]
    force: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    allow_unsigned: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cascade: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    purge: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    current: Option<PlannedCurrentState>,
    message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct OperationPlanSet {
    pub(super) plan_schema_version: u32,
    pub(super) plan_command: &'static str,
    pub(super) plan_digest: String,
    pub(super) plans: Vec<OperationPlan>,
}

impl OperationPlanSet {
    pub(super) fn new(command: &'static str, plans: Vec<OperationPlan>) -> anyhow::Result<Self> {
        let plan_digest = plan_digest(command, &plans)?;
        Ok(Self {
            plan_schema_version: PLAN_SCHEMA_VERSION,
            plan_command: command,
            plan_digest,
            plans,
        })
    }

    pub(super) fn print_human(&self) {
        println!("plan digest: {}", self.plan_digest);
        for plan in &self.plans {
            println!();
            println!("component: {}", plan.component);
            println!("action: {}", plan.action);
            println!("source: {}", plan.source);
            if let Some(source) = &plan.requested_source {
                println!("requested source: {source}");
            }
            if let Some(channel) = &plan.channel {
                println!("channel: {channel}");
            }
            if let Some(scope) = &plan.scope {
                println!("scope: {scope}");
            }
            if let Some(migration) = plan.migration {
                println!("migration: {migration}");
            }
            if let Some(version) = &plan.requested_version {
                println!("requested version: {version}");
            }
            if let Some(package) = &plan.local_package {
                println!("local package: {}", package.path.display);
                println!("local package sha256: {}", package.sha256);
            }
            for (component, release) in &plan.resolved_releases {
                println!(
                    "resolved release: {component} {} {} {}",
                    release.version, release.archive_name, release.sha256
                );
            }
            for (component, bundle) in &plan.resolved_release_bundles {
                println!(
                    "resolved release bundle: {component} {} {} {}",
                    bundle.version, bundle.route, bundle.package_sha256
                );
            }
            for (component, package) in &plan.resolved_registry_packages {
                println!(
                    "resolved registry package: {component} {} {} {} {}",
                    package.registry_name, package.version, package.target_name, package.sha256
                );
            }
            for (component, state) in &plan.prerequisites {
                println!(
                    "prerequisite: {component} {:?} {:?}",
                    state.presence, state.health
                );
            }
            println!("target: {}", plan.target);
            println!("ownership: {}", plan.ownership);
            println!("mutates: {}", plan.mutates);
            println!("{}", plan.message);
        }
    }

    pub(super) fn digest(&self) -> &str {
        &self.plan_digest
    }

    pub(super) fn verify_expected(&self, expected: Option<&str>) -> anyhow::Result<()> {
        let Some(expected) = expected else {
            return Ok(());
        };
        if expected == self.plan_digest {
            return Ok(());
        }
        Err(ComponentPlanMismatch {
            expected: expected.to_string(),
            actual: self.plan_digest.clone(),
        }
        .into())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentPlanMismatch {
    pub expected: String,
    pub actual: String,
}

impl std::fmt::Display for ComponentPlanMismatch {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "component plan changed after review: expected {}, resolved {}",
            self.expected, self.actual
        )
    }
}

impl std::error::Error for ComponentPlanMismatch {}

pub(super) fn validate_install_plan(
    id: &ComponentId,
    request: &InstallRequest,
) -> anyhow::Result<()> {
    let spec = catalog::find(id);
    let external = spec.is_none() && is_external_use_extension(id);
    if spec.is_none() && !external {
        bail!("component '{}' is not registered", id);
    }
    if external {
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
        } else if request.allow_unsigned {
            bail!("--allow-unsigned is valid only with an explicit local --from package");
        }
        return Ok(());
    }
    if request.package.is_some() {
        bail!("--from is valid only for external Use extensions");
    }
    if request.allow_unsigned {
        bail!("--allow-unsigned is valid only for external Use extensions");
    }
    if request.version.is_some()
        && !matches!(
            spec.map(|spec| spec.distribution),
            Some(Distribution::Release(_))
        )
    {
        bail!(
            "component '{}' does not own a versioned release; --version is not supported",
            id
        );
    }
    if request.source != InstallSource::Auto
        && !matches!(
            spec.map(|spec| spec.distribution),
            Some(Distribution::Release(_))
        )
    {
        bail!(
            "component '{}' does not own an install source; --source is not supported",
            id
        );
    }
    Ok(())
}

pub(super) async fn install_plan(
    id: &ComponentId,
    request: &InstallRequest,
    channel: &str,
    scope: &str,
    migrate: bool,
    paths: &ComponentPaths,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<PreparedOperationPlan> {
    validate_install_plan(id, request)?;
    let state = discover(paths)?
        .components
        .into_iter()
        .find(|component| &component.id == id);
    let spec = catalog::find(id);
    let external = spec.is_none() && is_external_use_extension(id);

    let requested_version_is_ready = request.version.as_deref().is_none_or(|requested| {
        state
            .as_ref()
            .and_then(|state| state.version.as_deref())
            .is_some_and(|installed| parse_version(installed).ok() == parse_version(requested).ok())
    });
    let already_ready = state.as_ref().is_some_and(ComponentState::is_ready)
        && requested_version_is_ready
        && !request.force
        && !external;
    let local_package = match request.package.as_deref() {
        Some(path) => Some(fingerprint_local_package(path).await?),
        None => None,
    };
    let mut resolved_releases = BTreeMap::new();
    let mut resolved_sources = BTreeMap::new();
    let mut prerequisites = BTreeMap::new();
    let mut resolved_release_bundles = BTreeMap::new();
    let mut resolved_registry_packages = BTreeMap::new();
    let (source, ownership) = if external {
        prepare_parent_release(
            &ComponentId::parse("use")?,
            paths,
            &mut resolved_sources,
            &mut resolved_releases,
            &mut prerequisites,
        )
        .await?;
        if request.package.is_some() {
            ("local-package".to_string(), "parent:use".to_string())
        } else {
            let package_id = id
                .relative_to(&ComponentId::parse("use")?)
                .context("external extension is outside the Use namespace")?;
            let bundle =
                resolve_release_bundle(paths, package_id, request.version.as_deref(), channel)
                    .await;
            match bundle {
                Ok(Some(bundle)) => {
                    resolved_release_bundles.insert(id.to_string(), bundle);
                    (
                        "release-bundle:a3s-use".to_string(),
                        "parent:use".to_string(),
                    )
                }
                bundle => {
                    let registry_store = registries.context(
                        "extension installation requires an A3S Use release bundle or the umbrella registry configuration",
                    )?;
                    let resolved = registry_store
                        .resolve_package(
                            &paths.state_root,
                            package_id,
                            request.version.as_deref(),
                            channel,
                        )
                        .await
                        .with_context(|| match bundle {
                            Ok(None) => {
                                format!("A3S Use has no matching release bundle for '{package_id}'")
                            }
                            Err(error) => {
                                format!("A3S Use release-bundle discovery also failed: {error:#}")
                            }
                            Ok(Some(package)) => format!(
                                "A3S Use release bundle '{}' could not be selected",
                                package.package_id
                            ),
                        })?;
                    let source = format!("registry:{}", resolved.registry.name);
                    resolved_registry_packages.insert(id.to_string(), resolved);
                    (source, "parent:use".to_string())
                }
            }
        }
    } else {
        match spec.map(|spec| spec.distribution) {
            Some(Distribution::Bundled) => ("bundled".to_string(), "bundled".to_string()),
            Some(Distribution::Delegated { parent }) => {
                prepare_parent_release(
                    &ComponentId::parse(parent)?,
                    paths,
                    &mut resolved_sources,
                    &mut resolved_releases,
                    &mut prerequisites,
                )
                .await?;
                (format!("delegated:{parent}"), format!("parent:{parent}"))
            }
            Some(Distribution::Release(_)) if already_ready => (
                state
                    .as_ref()
                    .and_then(|state| state.provenance)
                    .map(|value| format!("existing:{value:?}").to_ascii_lowercase())
                    .unwrap_or_else(|| "existing".to_string()),
                state
                    .as_ref()
                    .and_then(|state| state.provenance)
                    .map(|value| match value {
                        InstallProvenance::Homebrew => "package-manager:homebrew".to_string(),
                        _ => "a3s".to_string(),
                    })
                    .unwrap_or_else(|| "a3s".to_string()),
            ),
            Some(Distribution::Release(release)) => {
                let selected = resolve_install_source(id, release, request)?;
                resolved_sources.insert(id.to_string(), selected);
                match selected {
                    InstallSource::Release => {
                        let resolved =
                            resolve_release(id, release, request.version.as_deref()).await?;
                        resolved_releases.insert(id.to_string(), resolved);
                        ("github-release".to_string(), "a3s".to_string())
                    }
                    InstallSource::Homebrew => {
                        let formula = release.homebrew_formula.with_context(|| {
                            format!("component '{}' has no Homebrew formula", id)
                        })?;
                        (
                            format!("homebrew:{formula}"),
                            "package-manager:homebrew".to_string(),
                        )
                    }
                    InstallSource::Auto => bail!("automatic install source was not resolved"),
                }
            }
            None => bail!("component '{}' has no install ownership", id),
        }
    };
    let current = state
        .as_ref()
        .map(|state| PlannedCurrentState::with_receipt(state, paths))
        .transpose()?;

    let plan = OperationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        component: id.clone(),
        action: "install",
        source,
        requested_source: Some(install_source_name(request.source).to_string()),
        channel: Some(channel.to_string()),
        scope: Some(scope.to_string()),
        migration: Some(migrate),
        target: host_target(),
        ownership,
        mutates: !already_ready,
        requested_version: request.version.clone(),
        local_package,
        resolved_sources: planned_sources(&resolved_sources)?,
        resolved_releases: resolved_releases.clone(),
        resolved_release_bundles: resolved_release_bundles.clone(),
        resolved_registry_packages: resolved_registry_packages
            .iter()
            .map(|(component, resolved)| (component.clone(), resolved.package.clone()))
            .collect(),
        prerequisites,
        force: Some(request.force),
        allow_unsigned: Some(request.allow_unsigned),
        cascade: None,
        purge: None,
        current,
        message: if already_ready {
            "Already healthy; apply would be a no-op.".to_string()
        } else {
            "Apply would resolve, verify, stage, activate, and health-check the component."
                .to_string()
        },
    };
    Ok(PreparedOperationPlan {
        plan,
        resolved_releases,
        resolved_sources,
        resolved_release_bundles,
        resolved_registry_packages,
        apply_force: request.force,
    })
}

pub(super) fn uninstall_plan(
    id: &ComponentId,
    cascade: bool,
    purge: bool,
    paths: &ComponentPaths,
) -> anyhow::Result<OperationPlan> {
    let state = super::discovery::find_state(id, paths)?;
    if matches!(
        state.presence,
        Presence::External | Presence::System | Presence::Bundled
    ) {
        bail!(
            "component '{}' is not owned by A3S and cannot be uninstalled",
            id
        );
    }
    let mut prerequisites = BTreeMap::new();
    let parent = match catalog::find(id).map(|spec| spec.distribution) {
        Some(Distribution::Delegated { parent }) => Some(ComponentId::parse(parent)?),
        None if is_external_use_extension(id) => Some(ComponentId::parse("use")?),
        _ => None,
    };
    if let Some(parent) = parent {
        let parent_state = super::discovery::find_state(&parent, paths)?;
        if !parent_state.is_ready() {
            bail!("parent component '{}' is not ready", parent);
        }
        prerequisites.insert(
            parent.to_string(),
            PlannedCurrentState::with_receipt(&parent_state, paths)?,
        );
    }
    Ok(OperationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        component: id.clone(),
        action: "uninstall",
        source: state
            .provenance
            .map(|value| format!("{value:?}").to_ascii_lowercase())
            .unwrap_or_else(|| "unknown".to_string()),
        requested_source: None,
        channel: None,
        scope: None,
        migration: None,
        target: host_target(),
        ownership: "receipt-or-parent-owned".to_string(),
        mutates: state.presence != Presence::Missing,
        requested_version: None,
        local_package: None,
        resolved_sources: BTreeMap::new(),
        resolved_releases: BTreeMap::new(),
        resolved_release_bundles: BTreeMap::new(),
        resolved_registry_packages: BTreeMap::new(),
        prerequisites,
        force: None,
        allow_unsigned: None,
        cascade: Some(cascade),
        purge: Some(purge),
        current: Some(PlannedCurrentState::with_receipt(&state, paths)?),
        message: if purge {
            "Apply would remove owned files and component-owned recreatable cache.".to_string()
        } else {
            "Apply would remove only receipt-owned files or delegate to the owning parent."
                .to_string()
        },
    })
}

pub(super) async fn upgrade_plan(
    id: &ComponentId,
    paths: &ComponentPaths,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<PreparedOperationPlan> {
    let state = super::discovery::find_state(id, paths)?;
    if state.presence != Presence::Managed {
        bail!("component '{}' is not managed by A3S", id);
    }
    let Some(spec) = catalog::find(id) else {
        if !is_external_use_extension(id) {
            bail!("component '{}' is not registered", id);
        }
        if state.trust == Trust::ReleaseBundle {
            return release_bundle_extension_upgrade_plan(id, state, paths).await;
        }
        return registry_extension_upgrade_plan(id, state, paths, registries).await;
    };
    let release = catalog::release(spec)
        .with_context(|| format!("component '{}' has no managed release", id))?;
    let selected = match state.provenance {
        Some(InstallProvenance::Homebrew) => InstallSource::Homebrew,
        Some(InstallProvenance::GithubRelease) => InstallSource::Release,
        Some(provenance) => bail!(
            "component '{}' cannot be upgraded from {:?} provenance",
            id,
            provenance
        ),
        None => bail!("component '{}' has no recorded upgrade provenance", id),
    };
    let resolved_sources = BTreeMap::from([(id.to_string(), selected)]);
    let mut resolved_releases = BTreeMap::new();
    let source = match selected {
        InstallSource::Release => {
            resolved_releases.insert(id.to_string(), resolve_release(id, release, None).await?);
            "github-release".to_string()
        }
        InstallSource::Homebrew => {
            let formula = release
                .homebrew_formula
                .with_context(|| format!("component '{}' has no Homebrew formula", id))?;
            format!("homebrew:{formula}")
        }
        InstallSource::Auto => bail!("automatic install source was not resolved"),
    };
    let plan = OperationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        component: id.clone(),
        action: "upgrade",
        source,
        requested_source: None,
        channel: None,
        scope: None,
        migration: None,
        target: host_target(),
        ownership: "existing-provenance".to_string(),
        mutates: true,
        requested_version: None,
        local_package: None,
        resolved_sources: planned_sources(&resolved_sources)?,
        resolved_releases: resolved_releases.clone(),
        resolved_release_bundles: BTreeMap::new(),
        resolved_registry_packages: BTreeMap::new(),
        prerequisites: BTreeMap::new(),
        force: Some(true),
        allow_unsigned: None,
        cascade: None,
        purge: None,
        current: Some(PlannedCurrentState::with_receipt(&state, paths)?),
        message: "Apply would install the exact resolved artifact through the existing provenance."
            .to_string(),
    };
    Ok(PreparedOperationPlan {
        plan,
        resolved_releases,
        resolved_sources,
        resolved_release_bundles: BTreeMap::new(),
        resolved_registry_packages: BTreeMap::new(),
        apply_force: true,
    })
}

async fn release_bundle_extension_upgrade_plan(
    id: &ComponentId,
    state: ComponentState,
    paths: &ComponentPaths,
) -> anyhow::Result<PreparedOperationPlan> {
    let installed = extension_release_bundle_provenance(id, paths)?.with_context(|| {
        format!(
            "extension '{}' is marked as release-bundled but has no bundle provenance",
            id
        )
    })?;
    let package_id = id
        .relative_to(&ComponentId::parse("use")?)
        .context("release-bundled extension is outside the Use namespace")?;
    let resolved = resolve_release_bundle(paths, package_id, None, "stable")
        .await?
        .with_context(|| {
            format!(
                "the installed A3S Use release does not carry bundle '{}'",
                package_id
            )
        })?;
    let installed_version = parse_version(&installed.version).with_context(|| {
        format!(
            "installed release bundle '{}' has an invalid version",
            package_id
        )
    })?;
    let resolved_version = parse_version(&resolved.version)
        .with_context(|| format!("release bundle '{}' has an invalid version", package_id))?;
    if resolved_version < installed_version {
        bail!(
            "A3S Use attempted to downgrade release bundle '{}' from {} to {}",
            package_id,
            installed.version,
            resolved.version
        );
    }
    let mutates = installed.version != resolved.version
        || installed.package_sha256 != resolved.package_sha256;
    let resolved_release_bundles = BTreeMap::from([(id.to_string(), resolved.clone())]);
    let plan = OperationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        component: id.clone(),
        action: "upgrade",
        source: "release-bundle:a3s-use".to_string(),
        requested_source: None,
        channel: Some("stable".to_string()),
        scope: None,
        migration: None,
        target: host_target(),
        ownership: "parent:use".to_string(),
        mutates,
        requested_version: None,
        local_package: None,
        resolved_sources: BTreeMap::new(),
        resolved_releases: BTreeMap::new(),
        resolved_release_bundles: resolved_release_bundles.clone(),
        resolved_registry_packages: BTreeMap::new(),
        prerequisites: BTreeMap::new(),
        force: Some(mutates),
        allow_unsigned: Some(false),
        cascade: None,
        purge: None,
        current: Some(PlannedCurrentState::with_receipt(&state, paths)?),
        message: if mutates {
            "Apply would install the exact package carried by the current A3S Use release."
                .to_string()
        } else {
            "The installed package matches the current A3S Use release bundle; apply would be a no-op."
                .to_string()
        },
    };
    Ok(PreparedOperationPlan {
        plan,
        resolved_releases: BTreeMap::new(),
        resolved_sources: BTreeMap::new(),
        resolved_release_bundles,
        resolved_registry_packages: BTreeMap::new(),
        apply_force: mutates,
    })
}

async fn registry_extension_upgrade_plan(
    id: &ComponentId,
    state: ComponentState,
    paths: &ComponentPaths,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<PreparedOperationPlan> {
    let installed = extension_registry_provenance(id, paths)?.with_context(|| {
        format!(
            "local extension '{}' has no recorded signed upgrade source; install the new package explicitly with 'a3s install {} --from <package> --force --allow-unsigned'",
            id, id
        )
    })?;
    let registries = registries
        .context("signed extension upgrade requires the umbrella registry configuration")?;
    let resolved = registries
        .resolve_upgrade(&paths.state_root, &installed)
        .await?;
    let installed_version = parse_version(&installed.version)
        .with_context(|| format!("installed extension '{}' has an invalid version", id))?;
    let resolved_version = parse_version(&resolved.package.version).with_context(|| {
        format!(
            "registry returned an invalid version for extension '{}'",
            id
        )
    })?;
    if resolved_version < installed_version {
        bail!(
            "registry '{}' attempted to downgrade extension '{}' from {} to {}",
            installed.registry_name,
            id,
            installed.version,
            resolved.package.version
        );
    }

    let mutates = installed.version != resolved.package.version
        || installed.sha256 != resolved.package.sha256;
    let apply_force = mutates;
    let source = format!("registry:{}", resolved.registry.name);
    let channel = installed.channel.clone();
    let resolved_registry_packages = BTreeMap::from([(id.to_string(), resolved.clone())]);
    let plan = OperationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        component: id.clone(),
        action: "upgrade",
        source,
        requested_source: None,
        channel: Some(channel),
        scope: None,
        migration: None,
        target: host_target(),
        ownership: "parent:use".to_string(),
        mutates,
        requested_version: None,
        local_package: None,
        resolved_sources: BTreeMap::new(),
        resolved_releases: BTreeMap::new(),
        resolved_release_bundles: BTreeMap::new(),
        resolved_registry_packages: BTreeMap::from([(id.to_string(), resolved.package.clone())]),
        prerequisites: BTreeMap::new(),
        force: Some(apply_force),
        allow_unsigned: Some(false),
        cascade: None,
        purge: None,
        current: Some(PlannedCurrentState::with_receipt(&state, paths)?),
        message: if mutates {
            "Apply would install the exact signed target from the extension's recorded registry."
                .to_string()
        } else {
            "The recorded registry resolves to the installed target; apply would be a no-op."
                .to_string()
        },
    };
    Ok(PreparedOperationPlan {
        plan,
        resolved_releases: BTreeMap::new(),
        resolved_sources: BTreeMap::new(),
        resolved_release_bundles: BTreeMap::new(),
        resolved_registry_packages,
        apply_force,
    })
}

async fn prepare_parent_release(
    parent: &ComponentId,
    paths: &ComponentPaths,
    resolved_sources: &mut BTreeMap<String, InstallSource>,
    resolved_releases: &mut BTreeMap<String, ResolvedRelease>,
    prerequisites: &mut BTreeMap<String, PlannedCurrentState>,
) -> anyhow::Result<()> {
    let state = super::discovery::find_state(parent, paths)?;
    prerequisites.insert(
        parent.to_string(),
        PlannedCurrentState::with_receipt(&state, paths)?,
    );
    if state.is_ready() {
        return Ok(());
    }
    let spec = catalog::find(parent)
        .with_context(|| format!("parent component '{}' is not registered", parent))?;
    let release = catalog::release(spec)
        .with_context(|| format!("parent component '{}' is not installable", parent))?;
    let request = InstallRequest {
        progress: false,
        ..InstallRequest::default()
    };
    let source = resolve_install_source(parent, release, &request)?;
    resolved_sources.insert(parent.to_string(), source);
    if source == InstallSource::Release {
        resolved_releases.insert(
            parent.to_string(),
            resolve_release(parent, release, None).await?,
        );
    }
    Ok(())
}

fn planned_sources(
    sources: &BTreeMap<String, InstallSource>,
) -> anyhow::Result<BTreeMap<String, String>> {
    sources
        .iter()
        .map(|(component, source)| {
            let id = ComponentId::parse(component)?;
            let release = catalog::find(&id).and_then(catalog::release);
            let source = match (source, release) {
                (InstallSource::Auto, _) => "auto".to_string(),
                (InstallSource::Homebrew, Some(release)) => format!(
                    "homebrew:{}",
                    release.homebrew_formula.with_context(|| {
                        format!("component '{}' has no Homebrew formula", component)
                    })?
                ),
                (InstallSource::Release, Some(release)) => format!(
                    "github-release:{}/{}",
                    release.github_owner, release.github_repo
                ),
                (_, None) => bail!("component '{}' has no resolved install source", component),
            };
            Ok((component.clone(), source))
        })
        .collect()
}

fn install_source_name(source: InstallSource) -> &'static str {
    match source {
        InstallSource::Auto => "auto",
        InstallSource::Homebrew => "homebrew",
        InstallSource::Release => "release",
    }
}

async fn fingerprint_local_package(path: &Path) -> anyhow::Result<PlannedLocalPackage> {
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

fn is_external_use_extension(id: &ComponentId) -> bool {
    let mut segments = id.as_str().split('/');
    matches!(
        (
            segments.next(),
            segments.next(),
            segments.next(),
            segments.next()
        ),
        (Some("use"), Some(_), Some(_), None)
    )
}

fn host_target() -> String {
    format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH)
}

#[cfg(unix)]
fn path_identity(path: &std::path::Path) -> String {
    use std::os::unix::ffi::OsStrExt;

    format!("unix-hex:{}", encode_hex(path.as_os_str().as_bytes()))
}

#[cfg(windows)]
fn path_identity(path: &std::path::Path) -> String {
    use std::os::windows::ffi::OsStrExt;

    let words = path.as_os_str().encode_wide().collect::<Vec<_>>();
    let mut bytes = Vec::with_capacity(words.len() * 2);
    for word in words {
        bytes.extend_from_slice(&word.to_le_bytes());
    }
    format!("windows-utf16le-hex:{}", encode_hex(&bytes))
}

#[cfg(not(any(unix, windows)))]
fn path_identity(path: &std::path::Path) -> String {
    format!("display:{}", path.to_string_lossy())
}

fn encode_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(HEX[(byte >> 4) as usize] as char);
        output.push(HEX[(byte & 0x0f) as usize] as char);
    }
    output
}

fn plan_digest(command: &'static str, plans: &[OperationPlan]) -> anyhow::Result<String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CanonicalPlanSet<'a> {
        schema_version: u32,
        command: &'a str,
        plans: Vec<CanonicalOperationPlan<'a>>,
    }

    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct CanonicalOperationPlan<'a> {
        schema_version: u32,
        component: &'a ComponentId,
        action: &'a str,
        source: &'a str,
        requested_source: &'a Option<String>,
        channel: &'a Option<String>,
        scope: &'a Option<String>,
        migration: Option<bool>,
        target: &'a str,
        ownership: &'a str,
        mutates: bool,
        requested_version: &'a Option<String>,
        local_package: &'a Option<PlannedLocalPackage>,
        resolved_sources: &'a BTreeMap<String, String>,
        resolved_releases: &'a BTreeMap<String, ResolvedRelease>,
        resolved_release_bundles: &'a BTreeMap<String, ReleaseBundlePackage>,
        resolved_registry_packages: &'a BTreeMap<String, ResolvedRemotePackage>,
        prerequisites: &'a BTreeMap<String, PlannedCurrentState>,
        force: Option<bool>,
        allow_unsigned: Option<bool>,
        cascade: Option<bool>,
        purge: Option<bool>,
        current: &'a Option<PlannedCurrentState>,
    }

    let canonical = CanonicalPlanSet {
        schema_version: PLAN_SCHEMA_VERSION,
        command,
        plans: plans
            .iter()
            .map(|plan| CanonicalOperationPlan {
                schema_version: plan.schema_version,
                component: &plan.component,
                action: plan.action,
                source: &plan.source,
                requested_source: &plan.requested_source,
                channel: &plan.channel,
                scope: &plan.scope,
                migration: plan.migration,
                target: &plan.target,
                ownership: &plan.ownership,
                mutates: plan.mutates,
                requested_version: &plan.requested_version,
                local_package: &plan.local_package,
                resolved_sources: &plan.resolved_sources,
                resolved_releases: &plan.resolved_releases,
                resolved_release_bundles: &plan.resolved_release_bundles,
                resolved_registry_packages: &plan.resolved_registry_packages,
                prerequisites: &plan.prerequisites,
                force: plan.force,
                allow_unsigned: plan.allow_unsigned,
                cascade: plan.cascade,
                purge: plan.purge,
                current: &plan.current,
            })
            .collect(),
    };
    let bytes = serde_json::to_vec(&canonical).context("failed to encode component plan")?;
    let mut digest = Sha256::new();
    digest.update(PLAN_DIGEST_DOMAIN);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

#[cfg(test)]
mod tests {
    use super::super::catalog::ComponentKind;
    use super::super::state::Trust;
    use super::super::state::UpdateState;
    use super::*;

    fn fixture(message: &str) -> OperationPlan {
        OperationPlan {
            schema_version: PLAN_SCHEMA_VERSION,
            component: ComponentId::parse("box").unwrap(),
            action: "install",
            source: "release".to_string(),
            requested_source: Some("release".to_string()),
            channel: Some("stable".to_string()),
            scope: Some("user".to_string()),
            migration: Some(false),
            target: "linux-x86_64".to_string(),
            ownership: "a3s".to_string(),
            mutates: true,
            requested_version: Some("1.2.3".to_string()),
            local_package: None,
            resolved_sources: BTreeMap::from([("box".to_string(), "github-release".to_string())]),
            resolved_releases: BTreeMap::new(),
            resolved_release_bundles: BTreeMap::new(),
            resolved_registry_packages: BTreeMap::new(),
            prerequisites: BTreeMap::new(),
            force: Some(false),
            allow_unsigned: Some(false),
            cascade: None,
            purge: None,
            current: None,
            message: message.to_string(),
        }
    }

    #[test]
    fn digest_is_stable_and_excludes_presentation_text() {
        let first = OperationPlanSet::new("component.install", vec![fixture("first")]).unwrap();
        let second = OperationPlanSet::new("component.install", vec![fixture("second")]).unwrap();
        assert_eq!(first.plan_digest, second.plan_digest);
        assert_eq!(first.plan_digest.len(), 64);
        assert!(first
            .plan_digest
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
    }

    #[test]
    fn digest_changes_with_semantics_or_operation_order() {
        let first = fixture("same");
        let mut forced = first.clone();
        forced.force = Some(true);
        let mut different_version = first.clone();
        different_version.requested_version = Some("2.0.0".to_string());
        let mut different_source = first.clone();
        different_source.source = "homebrew:a3s-lab/tap/a3s-box".to_string();
        different_source.requested_source = Some("homebrew".to_string());
        let mut purged = first.clone();
        purged.purge = Some(true);
        assert_ne!(
            plan_digest("component.install", std::slice::from_ref(&first)).unwrap(),
            plan_digest("component.install", &[forced.clone()]).unwrap()
        );
        for changed in [different_version, different_source, purged] {
            assert_ne!(
                plan_digest("component.install", std::slice::from_ref(&first)).unwrap(),
                plan_digest("component.install", &[changed]).unwrap()
            );
        }
        assert_ne!(
            plan_digest("component.install", &[first.clone(), forced.clone()]).unwrap(),
            plan_digest("component.install", &[forced, first]).unwrap()
        );
    }

    #[test]
    fn current_state_is_part_of_the_digest() {
        let mut first = fixture("same");
        first.current = Some(PlannedCurrentState::from(&ComponentState {
            id: ComponentId::parse("box").unwrap(),
            kind: ComponentKind::Product,
            description: String::new(),
            presence: Presence::Managed,
            health: Health::Ready,
            update: UpdateState::Unknown,
            trust: Trust::FirstParty,
            provenance: Some(InstallProvenance::GithubRelease),
            version: Some("1.0.0".to_string()),
            path: Some(PathBuf::from("/components/box")),
            message: None,
        }));
        let mut second = first.clone();
        second.current.as_mut().unwrap().version = Some("2.0.0".to_string());
        assert_ne!(
            plan_digest("component.install", &[first]).unwrap(),
            plan_digest("component.install", &[second]).unwrap()
        );
    }

    #[test]
    fn receipt_ownership_and_checksums_are_part_of_the_digest() {
        let mut first = fixture("same");
        first.current = Some(PlannedCurrentState {
            presence: Presence::Managed,
            health: Health::Ready,
            provenance: Some(InstallProvenance::GithubRelease),
            version: Some("1.0.0".to_string()),
            path: Some(PlannedPath::new(Path::new("/components/box/a3s-box"))),
            receipt: Some(PlannedReceipt {
                schema_version: 1,
                component_id: "box".to_string(),
                version: "1.0.0".to_string(),
                provenance: InstallProvenance::GithubRelease,
                install_root: PlannedPath::new(Path::new("/components/box")),
                executable_path: Some(PlannedPath::new(Path::new("/components/box/a3s-box"))),
                owned_paths: vec![PlannedPath::new(Path::new("/components/box"))],
                source: Some("https://example.invalid/releases/v1.0.0".to_string()),
                artifact_checksums: BTreeMap::from([("box.tar.gz".to_string(), "a".repeat(64))]),
            }),
        });
        let mut second = first.clone();
        second
            .current
            .as_mut()
            .unwrap()
            .receipt
            .as_mut()
            .unwrap()
            .artifact_checksums
            .insert("box.tar.gz".to_string(), "b".repeat(64));
        assert_ne!(
            plan_digest("component.uninstall", &[first]).unwrap(),
            plan_digest("component.uninstall", &[second]).unwrap()
        );
    }

    #[test]
    fn expected_digest_rejects_a_changed_plan() {
        let plan = OperationPlanSet::new("component.install", vec![fixture("same")]).unwrap();
        plan.verify_expected(Some(plan.digest())).unwrap();
        let error = plan.verify_expected(Some(&"0".repeat(64))).unwrap_err();
        let mismatch = error.downcast_ref::<ComponentPlanMismatch>().unwrap();
        assert_eq!(mismatch.expected, "0".repeat(64));
        assert_eq!(mismatch.actual, plan.digest());
    }

    #[test]
    fn resolved_release_is_part_of_the_digest() {
        let mut first = fixture("same");
        first.resolved_releases.insert(
            "box".to_string(),
            ResolvedRelease {
                version: "1.2.3".to_string(),
                tag: "v1.2.3".to_string(),
                target: "linux-x86_64".to_string(),
                archive_name: "a3s-box-v1.2.3-linux-x86_64.tar.gz".to_string(),
                asset_url: "https://example.invalid/box.tar.gz".to_string(),
                sha256: "a".repeat(64),
            },
        );
        let mut second = first.clone();
        second.resolved_releases.get_mut("box").unwrap().sha256 = "b".repeat(64);
        assert_ne!(
            plan_digest("component.install", &[first]).unwrap(),
            plan_digest("component.install", &[second]).unwrap()
        );
    }

    #[tokio::test]
    async fn local_package_fingerprint_is_stable_and_tracks_content() {
        let temp = tempfile::tempdir().unwrap();
        let package = temp.path().join("package");
        std::fs::create_dir_all(package.join("nested")).unwrap();
        std::fs::write(package.join("nested/tool"), b"first").unwrap();

        let first = fingerprint_local_package(&package).await.unwrap();
        let repeated = fingerprint_local_package(&package).await.unwrap();
        assert_eq!(first, repeated);
        assert_eq!(first.file_count, 1);
        assert_eq!(first.byte_count, 5);

        std::fs::write(package.join("nested/tool"), b"other").unwrap();
        let changed = fingerprint_local_package(&package).await.unwrap();
        assert_ne!(first.sha256, changed.sha256);

        let mut first_plan = fixture("same");
        first_plan.local_package = Some(first);
        let mut changed_plan = first_plan.clone();
        changed_plan.local_package = Some(changed);
        assert_ne!(
            plan_digest("component.install", &[first_plan]).unwrap(),
            plan_digest("component.install", &[changed_plan]).unwrap()
        );
    }
}
