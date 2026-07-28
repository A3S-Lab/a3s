use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::PathBuf;
use std::process::ExitStatus;

use crate::registry::RegistryStore;
use a3s_updater::{fetch_latest_release, parse_version, InstallProvenance};
use anyhow::{bail, Context};
use serde::Serialize;

use super::catalog::{self, ComponentKind, Distribution};
use super::discovery::{discover, find_state};
use super::id::ComponentId;
use super::journal::{validate_batch_components, BatchJournal};
use super::lifecycle::{
    install_component, install_component_locked, uninstall_component_locked, InstallIntent,
    InstallRequest, InstallSource, OperationRecord,
};
use super::lock::ComponentOperationLock;
use super::paths::ComponentPaths;
use super::plan::{
    install_plan, uninstall_plan, upgrade_plan, validate_install_plan, OperationPlanSet,
};
use super::state::{ComponentReport, Health, Presence, UpdateState};

mod options;
mod output;

use options::{
    DoctorOptions, InfoOptions, InstallOptions, ListOptions, UninstallOptions, UpdateOptions,
};
use output::{finish_batch, print_available, print_human_report, print_json, print_plans};

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentFailure {
    pub component: ComponentId,
    pub message: String,
}

/// Secret-free component readiness returned to in-process diagnostics.
///
/// Unlike the full discovery state, this type deliberately omits executable
/// paths, probe messages, versions, and provenance so a host can safely pass
/// the result into an untrusted diagnostic prompt.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentHealthStatus {
    Ready,
    Broken,
    Missing,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentHealthCheck {
    pub component: ComponentId,
    pub status: ComponentHealthStatus,
}

impl ComponentHealthCheck {
    pub fn is_ready(&self) -> bool {
        self.status == ComponentHealthStatus::Ready
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentHealthReport {
    pub healthy: bool,
    pub checks: Vec<ComponentHealthCheck>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComponentBatchFailure {
    pub action: &'static str,
    pub plan_digest: String,
    pub operations: Vec<OperationRecord>,
    pub failures: Vec<ComponentFailure>,
}

impl ComponentBatchFailure {
    pub fn is_partial(&self) -> bool {
        !self.operations.is_empty() && !self.failures.is_empty()
    }
}

impl std::fmt::Display for ComponentBatchFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "component {} completed {} target(s) and failed {} target(s)",
            self.action,
            self.operations.len(),
            self.failures.len()
        )?;
        if let Some(failure) = self.failures.first() {
            write!(
                formatter,
                "; first failure: {}: {}",
                failure.component, failure.message
            )?;
        }
        Ok(())
    }
}

impl std::error::Error for ComponentBatchFailure {}

pub async fn run_list(args: Vec<String>) -> anyhow::Result<()> {
    let paths = ComponentPaths::from_env()?;
    let offline = environment_flag("A3S_OFFLINE");
    run_list_with(args, &paths, offline).await
}

pub async fn run_list_with(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
) -> anyhow::Result<()> {
    run_list_with_command(args, paths, offline, "component.list", false, None).await
}

pub async fn run_list_with_registries(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    registries: &RegistryStore,
) -> anyhow::Result<()> {
    run_list_with_command(
        args,
        paths,
        offline,
        "component.list",
        false,
        Some(registries),
    )
    .await
}

pub async fn run_upgrade_list_with(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
) -> anyhow::Result<()> {
    run_list_with_command(args, paths, offline, "component.upgrade", true, None).await
}

pub async fn run_upgrade_list_with_registries(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    registries: &RegistryStore,
) -> anyhow::Result<()> {
    run_list_with_command(
        args,
        paths,
        offline,
        "component.upgrade",
        true,
        Some(registries),
    )
    .await
}

async fn run_list_with_command(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    command: &'static str,
    managed_upgrades_only: bool,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<()> {
    let options = ListOptions::parse(&args)?;
    let mut report = discover(paths)?;
    if options.check_updates {
        if offline {
            bail!("component update checks are unavailable in offline mode");
        }
        populate_updates(&mut report, paths, registries).await;
    }
    report.components.retain(|component| {
        (!options.installed || component.presence != Presence::Missing)
            && (!options.available || component.presence == Presence::Missing)
            && (!options.check_updates || component.update == UpdateState::Available)
            && options.kind.is_none_or(|kind| component.kind == kind)
            && (!managed_upgrades_only || is_upgrade_all_candidate(component))
    });
    if managed_upgrades_only {
        report.external_tools.clear();
    }
    if options.json {
        print_json(command, &report, true)?;
    } else {
        print_human_report(&report);
    }
    Ok(())
}

pub async fn run_install(args: Vec<String>) -> anyhow::Result<()> {
    let paths = ComponentPaths::from_env()?;
    let offline = environment_flag("A3S_OFFLINE");
    let progress = !environment_flag("A3S_NO_PROGRESS") && !environment_flag("A3S_QUIET");
    run_install_with(args, &paths, offline, progress).await
}

pub async fn run_install_with(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    progress: bool,
) -> anyhow::Result<()> {
    run_install_with_registry(args, paths, offline, progress, None).await
}

pub async fn run_install_with_registries(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    progress: bool,
    registries: &RegistryStore,
) -> anyhow::Result<()> {
    run_install_with_registry(args, paths, offline, progress, Some(registries)).await
}

async fn run_install_with_registry(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    progress: bool,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<()> {
    let options = InstallOptions::parse(&args)?;
    if options.components.is_empty() {
        return print_available(options.json);
    }
    validate_batch_components(&options.components)?;
    validate_supported_install_policy(&options)?;
    enforce_provenance_policy(&options, paths)?;
    let request = InstallRequest {
        version: options.version.clone(),
        source: options.source,
        intent: InstallIntent::Install,
        package: options.package.clone(),
        force: options.force,
        allow_unsigned: options.allow_unsigned,
        progress,
        resolved_releases: Default::default(),
        resolved_sources: Default::default(),
        resolved_release_bundles: Default::default(),
        resolved_registry_packages: Default::default(),
    };
    for component in &options.components {
        validate_install_plan(component, &request)?;
    }
    preflight_install_sources(
        &options.components,
        &request,
        paths,
        offline,
        options.channel.as_str(),
        registries,
    )
    .await?;
    let _locks = acquire_operation_locks(&options.components, paths).await?;
    let mut prepared = Vec::with_capacity(options.components.len());
    for component in &options.components {
        prepared.push(
            install_plan(
                component,
                &request,
                options.channel.as_str(),
                options.scope.as_str(),
                options.migrate,
                paths,
                registries,
            )
            .await?,
        );
    }
    let plan_set = OperationPlanSet::new(
        "component.install",
        prepared
            .iter()
            .map(|prepared| prepared.plan.clone())
            .collect(),
    )?;
    if options.dry_run {
        return print_plans("component.install", &plan_set, options.json);
    }
    plan_set.verify_expected(options.plan_digest.as_deref())?;
    let mut journal = BatchJournal::begin(
        paths,
        "component.install",
        "install",
        plan_set.digest(),
        &options.components,
    )?;
    let mut operations = Vec::new();
    let mut failures = Vec::new();
    for (component, prepared) in options.components.into_iter().zip(prepared) {
        if let Some(operation) = journal.take_recovered(&component) {
            operations.push(operation);
            continue;
        }
        let mut prepared_request = request.clone();
        prepared_request.resolved_releases = prepared.resolved_releases;
        prepared_request.resolved_sources = prepared.resolved_sources;
        prepared_request.resolved_release_bundles = prepared.resolved_release_bundles;
        prepared_request.resolved_registry_packages = prepared.resolved_registry_packages;
        match install_component_locked(&component, &prepared_request, paths).await {
            Ok(operation) => {
                journal.record_success(&operation)?;
                operations.push(operation);
            }
            Err(error) => {
                let message = format!("{error:#}");
                journal.record_failure(&component, &message)?;
                failures.push(ComponentFailure { component, message });
            }
        }
    }
    journal.finish(failures.is_empty())?;
    finish_batch(
        "component.install",
        "install",
        plan_set.digest().to_string(),
        operations,
        failures,
        options.json,
    )
}

pub fn run_uninstall(args: Vec<String>) -> anyhow::Result<()> {
    let paths = ComponentPaths::from_env()?;
    run_uninstall_with(args, &paths)
}

pub fn run_uninstall_with(args: Vec<String>, paths: &ComponentPaths) -> anyhow::Result<()> {
    let options = UninstallOptions::parse(&args)?;
    if options.components.is_empty() {
        bail!("usage: a3s uninstall <component>... [--cascade] [--purge]");
    }
    validate_batch_components(&options.components)?;
    let _locks = acquire_operation_locks_sync(&options.components, paths)?;
    let plans = options
        .components
        .iter()
        .map(|component| uninstall_plan(component, options.cascade, options.purge, paths))
        .collect::<anyhow::Result<Vec<_>>>()?;
    let plan_set = OperationPlanSet::new("component.uninstall", plans)?;
    if options.dry_run {
        return print_plans("component.uninstall", &plan_set, options.json);
    }
    plan_set.verify_expected(options.plan_digest.as_deref())?;
    let mut journal = BatchJournal::begin(
        paths,
        "component.uninstall",
        "uninstall",
        plan_set.digest(),
        &options.components,
    )?;
    let mut operations = Vec::new();
    let mut failures = Vec::new();
    for component in options.components {
        if let Some(operation) = journal.take_recovered(&component) {
            operations.push(operation);
            continue;
        }
        match uninstall_component_locked(&component, options.cascade, options.purge, paths) {
            Ok(operation) => {
                journal.record_success(&operation)?;
                operations.push(operation);
            }
            Err(error) => {
                let message = format!("{error:#}");
                journal.record_failure(&component, &message)?;
                failures.push(ComponentFailure { component, message });
            }
        }
    }
    journal.finish(failures.is_empty())?;
    finish_batch(
        "component.uninstall",
        "uninstall",
        plan_set.digest().to_string(),
        operations,
        failures,
        options.json,
    )
}

pub async fn run_update(args: Vec<String>) -> anyhow::Result<()> {
    let paths = ComponentPaths::from_env()?;
    let offline = environment_flag("A3S_OFFLINE");
    let progress = !environment_flag("A3S_NO_PROGRESS") && !environment_flag("A3S_QUIET");
    run_update_with(args, &paths, offline, progress).await
}

pub async fn run_update_with(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    progress: bool,
) -> anyhow::Result<()> {
    run_update_with_registry(args, paths, offline, progress, None).await
}

pub async fn run_update_with_registries(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    progress: bool,
    registries: &RegistryStore,
) -> anyhow::Result<()> {
    run_update_with_registry(args, paths, offline, progress, Some(registries)).await
}

async fn run_update_with_registry(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
    progress: bool,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<()> {
    let options = UpdateOptions::parse(&args)?;
    let components = if options.all {
        discover(paths)?
            .components
            .into_iter()
            .filter(is_upgrade_all_candidate)
            .map(|component| component.id)
            .collect::<Vec<_>>()
    } else {
        options.components
    };
    if components.is_empty() {
        bail!("no managed components were selected for update");
    }
    validate_batch_components(&components)?;
    if offline {
        bail!("component upgrades require network access and are unavailable in offline mode");
    }
    let _locks = acquire_operation_locks(&components, paths).await?;
    let mut prepared = Vec::with_capacity(components.len());
    for component in &components {
        prepared.push(upgrade_plan(component, paths, registries).await?);
    }
    let plan_set = OperationPlanSet::new(
        "component.upgrade",
        prepared
            .iter()
            .map(|prepared| prepared.plan.clone())
            .collect(),
    )?;
    if options.dry_run {
        return print_plans("component.upgrade", &plan_set, options.json);
    }
    plan_set.verify_expected(options.plan_digest.as_deref())?;
    let mut journal = BatchJournal::begin(
        paths,
        "component.upgrade",
        "upgrade",
        plan_set.digest(),
        &components,
    )?;
    let mut operations = Vec::new();
    let mut failures = Vec::new();
    for (component, prepared) in components.into_iter().zip(prepared) {
        if let Some(operation) = journal.take_recovered(&component) {
            operations.push(operation);
            continue;
        }
        let operation = async {
            let state = find_state(&component, paths)?;
            if state.presence != Presence::Managed {
                bail!(
                    "component '{}' is not managed by A3S; install it explicitly before updating",
                    component
                );
            }
            if state.provenance == Some(InstallProvenance::Delegated)
                && catalog::find(&component).is_none()
                && prepared.resolved_release_bundles.is_empty()
                && prepared.resolved_registry_packages.is_empty()
            {
                bail!(
                    "local extension '{}' has no recorded upgrade source; install the new package explicitly with 'a3s install {} --from <package> --force --allow-unsigned'",
                    component,
                    component
                );
            }
            let request = InstallRequest {
                source: InstallSource::Auto,
                intent: InstallIntent::Upgrade,
                force: prepared.apply_force,
                progress,
                resolved_releases: prepared.resolved_releases,
                resolved_sources: prepared.resolved_sources,
                resolved_release_bundles: prepared.resolved_release_bundles,
                resolved_registry_packages: prepared.resolved_registry_packages,
                ..InstallRequest::default()
            };
            install_component_locked(&component, &request, paths).await
        }
        .await;
        match operation {
            Ok(operation) => {
                journal.record_success(&operation)?;
                operations.push(operation);
            }
            Err(error) => {
                let message = format!("{error:#}");
                journal.record_failure(&component, &message)?;
                failures.push(ComponentFailure { component, message });
            }
        }
    }
    journal.finish(failures.is_empty())?;
    finish_batch(
        "component.upgrade",
        "upgrade",
        plan_set.digest().to_string(),
        operations,
        failures,
        options.json,
    )
}

fn is_upgrade_all_candidate(component: &super::state::ComponentState) -> bool {
    component.presence == Presence::Managed
        && (component.kind == ComponentKind::Product
            || (component.kind == ComponentKind::Extension
                && matches!(
                    component.trust,
                    super::state::Trust::RegistryTuf | super::state::Trust::ReleaseBundle
                )))
}

pub async fn run_info(args: Vec<String>) -> anyhow::Result<()> {
    let paths = ComponentPaths::from_env()?;
    let offline = environment_flag("A3S_OFFLINE");
    run_info_with(args, &paths, offline).await
}

pub async fn run_info_with(
    args: Vec<String>,
    paths: &ComponentPaths,
    offline: bool,
) -> anyhow::Result<()> {
    let options = InfoOptions::parse(&args)?;
    let id = ComponentId::parse(&options.component)?;
    let state = find_state(&id, paths)?;
    let spec = catalog::find(&id);
    let sources = spec
        .map(|spec| match spec.distribution {
            Distribution::Bundled => vec!["bundled".to_string()],
            Distribution::Delegated { parent } => vec![format!("delegated:{parent}")],
            Distribution::Release(release) => {
                let mut sources = vec![format!(
                    "release:github.com/{}/{}",
                    release.github_owner, release.github_repo
                )];
                if let Some(formula) = release.homebrew_formula {
                    sources.push(format!("homebrew:{formula}"));
                }
                sources
            }
        })
        .unwrap_or_else(|| vec!["delegated:use".to_string()]);
    let latest = if options.versions {
        if offline {
            bail!("version discovery is unavailable in offline mode");
        }
        match spec.and_then(catalog::release) {
            Some(release) => Some(
                fetch_latest_release(release.github_owner, release.github_repo)
                    .await?
                    .tag_name,
            ),
            None => state.version.clone(),
        }
    } else {
        None
    };
    let data = serde_json::json!({
        "schemaVersion": 1,
        "component": state,
        "sources": sources,
        "latestVersion": latest,
    });
    if options.json {
        print_json("component.info", &data, true)?;
    } else {
        println!("component: {}", state.id);
        println!("description: {}", state.description);
        println!("presence: {:?}", state.presence);
        println!("health: {:?}", state.health);
        println!("version: {}", state.version.as_deref().unwrap_or("-"));
        if let Some(path) = &state.path {
            println!("path: {}", path.display());
        }
        if options.sources || options.versions {
            println!("sources:");
            for source in sources {
                println!("  {source}");
            }
        }
        if let Some(latest) = latest {
            println!("latest version: {latest}");
        }
    }
    Ok(())
}

pub fn run_doctor(args: Vec<String>) -> anyhow::Result<bool> {
    let paths = ComponentPaths::from_env()?;
    run_doctor_with(args, &paths)
}

/// Inspect every installed (plus always-required built-in) component without
/// printing and without exposing executable paths or probe output.
pub fn component_health_report(paths: &ComponentPaths) -> anyhow::Result<ComponentHealthReport> {
    let checks = installed_component_checks(discover(paths)?)
        .into_iter()
        .map(|state| ComponentHealthCheck {
            component: state.id,
            status: component_health_status(state.presence, state.health),
        })
        .collect::<Vec<_>>();
    Ok(ComponentHealthReport {
        healthy: checks.iter().all(ComponentHealthCheck::is_ready),
        checks,
    })
}

pub fn run_doctor_with(args: Vec<String>, paths: &ComponentPaths) -> anyhow::Result<bool> {
    let options = DoctorOptions::parse(&args)?;
    let report = discover(paths)?;
    let checks = if let Some(component) = options.component {
        let id = ComponentId::parse(&component)?;
        vec![report
            .components
            .into_iter()
            .find(|state| state.id == id)
            .with_context(|| format!("component '{id}' is not registered"))?]
    } else {
        installed_component_checks(report)
    };
    let healthy = checks.iter().all(|state| state.is_ready());
    if options.json {
        print_json(
            "component.doctor",
            &serde_json::json!({"healthy": healthy, "checks": checks}),
            healthy,
        )?;
    } else {
        for state in &checks {
            println!(
                "{} {:<18} {:?} / {:?}",
                if state.is_ready() { "ok" } else { "failed" },
                state.id,
                state.presence,
                state.health
            );
            if let Some(message) = &state.message {
                println!("  {message}");
            }
        }
    }
    Ok(healthy)
}

fn installed_component_checks(report: ComponentReport) -> Vec<super::state::ComponentState> {
    report
        .components
        .into_iter()
        .filter(|state| state.presence != Presence::Missing || state.kind == ComponentKind::BuiltIn)
        .collect()
}

fn component_health_status(presence: Presence, health: Health) -> ComponentHealthStatus {
    match (presence, health) {
        (Presence::Missing, _) => ComponentHealthStatus::Missing,
        (_, Health::Broken) => ComponentHealthStatus::Broken,
        (_, Health::Ready) => ComponentHealthStatus::Ready,
        (_, Health::Unknown) => ComponentHealthStatus::Unknown,
    }
}

pub async fn run_proxy(component: &str, args: Vec<OsString>) -> anyhow::Result<ExitStatus> {
    let paths = ComponentPaths::from_env()?;
    let allow_auto_install =
        !environment_flag("A3S_NO_AUTO_INSTALL") && !environment_flag("A3S_OFFLINE");
    let progress = !environment_flag("A3S_NO_PROGRESS") && !environment_flag("A3S_QUIET");
    let executable =
        resolve_or_install_with(component, &paths, allow_auto_install, progress).await?;
    let status = tokio::process::Command::new(&executable)
        .args(args)
        .status()
        .await
        .with_context(|| format!("failed to run {}", executable.display()))?;
    Ok(status)
}

pub async fn resolve_or_install(component: &str) -> anyhow::Result<PathBuf> {
    let paths = ComponentPaths::from_env()?;
    let allow_auto_install =
        !environment_flag("A3S_NO_AUTO_INSTALL") && !environment_flag("A3S_OFFLINE");
    let progress = !environment_flag("A3S_NO_PROGRESS") && !environment_flag("A3S_QUIET");
    resolve_or_install_with(component, &paths, allow_auto_install, progress).await
}

/// Resolve an already-ready component without installing or mutating it.
///
/// This is used when one component can optionally expose another component as
/// a route. The component catalog and receipt remain the single source of
/// truth; the consumer receives only the resolved executable path.
pub fn find_ready_executable_with(
    component: &str,
    paths: &ComponentPaths,
) -> anyhow::Result<Option<PathBuf>> {
    let id = ComponentId::parse(component)?;
    let state = find_state(&id, paths)?;
    if !state.is_ready() {
        return Ok(None);
    }
    Ok(state.path)
}

pub async fn resolve_or_install_with(
    component: &str,
    paths: &ComponentPaths,
    allow_auto_install: bool,
    progress: bool,
) -> anyhow::Result<PathBuf> {
    let id = ComponentId::parse(component)?;
    let spec = catalog::find(&id)
        .with_context(|| format!("component '{}' is not registered", component))?;
    let mut state = find_state(&id, paths)?;
    if state.presence == Presence::Missing {
        if !spec.auto_install_on_use {
            bail!(
                "component '{}' is not installed; run 'a3s install {}'",
                id,
                id
            );
        }
        if !allow_auto_install {
            bail!(
                "component '{}' is not installed and first-use installation is disabled; run 'a3s install {}'",
                id,
                id
            );
        }
        super::progress(
            progress,
            format!(
                "a3s: component '{}' is not installed; installing it now...",
                id
            ),
        );
        let request = InstallRequest {
            progress,
            ..InstallRequest::default()
        };
        install_component(&id, &request, paths).await?;
        state = find_state(&id, paths)?;
    }
    if state.health != Health::Ready {
        bail!(
            "component '{}' is present but not healthy; run 'a3s install {} --force'",
            id,
            id
        );
    }
    state
        .path
        .context("resolved component has no executable path")
}

async fn populate_updates(
    report: &mut ComponentReport,
    paths: &ComponentPaths,
    registries: Option<&RegistryStore>,
) {
    for component in &mut report.components {
        if component.presence != Presence::Managed {
            continue;
        }
        if component.kind == ComponentKind::Extension
            && component.trust == super::state::Trust::RegistryTuf
        {
            let result = async {
                let registries = registries.context(
                    "signed extension update checks require the umbrella registry configuration",
                )?;
                let installed =
                    super::discovery::extension_registry_provenance(&component.id, paths)?
                        .context("signed extension has no registry provenance")?;
                let resolved = registries
                    .resolve_upgrade(&paths.state_root, &installed)
                    .await?;
                let current = parse_version(&installed.version)?;
                let latest = parse_version(&resolved.package.version)?;
                if latest < current {
                    bail!(
                        "registry '{}' attempted to downgrade from {} to {}",
                        installed.registry_name,
                        installed.version,
                        resolved.package.version
                    );
                }
                Ok::<_, anyhow::Error>(
                    if latest > current || resolved.package.sha256 != installed.sha256 {
                        UpdateState::Available
                    } else {
                        UpdateState::Current
                    },
                )
            }
            .await;
            match result {
                Ok(update) => component.update = update,
                Err(error) => component.message = Some(format!("Update check failed: {error:#}")),
            }
            continue;
        }

        let Some(spec) = catalog::find(&component.id) else {
            continue;
        };
        let Some(release) = catalog::release(spec) else {
            continue;
        };
        let Some(current) = component.version.as_deref() else {
            continue;
        };
        let latest = match fetch_latest_release(release.github_owner, release.github_repo).await {
            Ok(latest) => latest,
            Err(error) => {
                component.message = Some(format!("Update check failed: {error}"));
                continue;
            }
        };
        let Ok(current) = parse_version(current) else {
            continue;
        };
        let Ok(latest) = parse_version(&latest.tag_name) else {
            continue;
        };
        component.update = if latest > current {
            UpdateState::Available
        } else {
            UpdateState::Current
        };
    }
}

async fn preflight_install_sources(
    components: &[ComponentId],
    request: &InstallRequest,
    paths: &ComponentPaths,
    offline: bool,
    channel: &str,
    registries: Option<&RegistryStore>,
) -> anyhow::Result<()> {
    for component in components {
        if request.package.is_none() && is_external_use_extension(component) {
            let package_id = component
                .relative_to(&ComponentId::parse("use")?)
                .context("external extension is outside the Use namespace")?;
            let bundle = super::release_bundle::resolve_release_bundle(
                paths,
                package_id,
                request.version.as_deref(),
                channel,
            )
            .await;
            if bundle.as_ref().is_ok_and(Option::is_some) {
                continue;
            }
            if offline {
                bundle?;
                bail!(
                    "extension '{}' has no installed A3S Use release bundle and a signed registry cannot be resolved in offline mode",
                    component
                );
            }
            registries
                .context(
                    "extension installation requires an A3S Use release bundle or the umbrella registry configuration",
                )?
                .require_configured_registry()?;
            continue;
        }
        if request.package.is_some() && is_external_use_extension(component) {
            if !offline {
                continue;
            }
            let parent = ComponentId::parse("use")?;
            if find_state(&parent, paths)?.is_ready() {
                continue;
            }
            bail!(
                "component '{}' has a local package, but its parent '{}' is unavailable in offline mode",
                component,
                parent
            );
        }
        if offline && !find_state(component, paths)?.is_ready() {
            bail!(
                "component '{}' is not available from an installed or explicit local source in offline mode",
                component
            );
        }
    }
    Ok(())
}

async fn acquire_operation_locks(
    components: &[ComponentId],
    paths: &ComponentPaths,
) -> anyhow::Result<Vec<ComponentOperationLock>> {
    let targets = operation_lock_targets(components, paths);
    let mut locks = Vec::with_capacity(targets.len() + 1);
    let batch = ComponentId::parse("component-batch")?;
    locks.push(ComponentOperationLock::acquire(paths.batch_operation_lock_path(), &batch).await?);
    for (path, component) in targets {
        locks.push(ComponentOperationLock::acquire(path, &component).await?);
    }
    Ok(locks)
}

fn acquire_operation_locks_sync(
    components: &[ComponentId],
    paths: &ComponentPaths,
) -> anyhow::Result<Vec<ComponentOperationLock>> {
    let targets = operation_lock_targets(components, paths);
    let mut locks = Vec::with_capacity(targets.len() + 1);
    let batch = ComponentId::parse("component-batch")?;
    locks.push(ComponentOperationLock::acquire_sync(
        &paths.batch_operation_lock_path(),
        &batch,
    )?);
    for (path, component) in targets {
        locks.push(ComponentOperationLock::acquire_sync(&path, &component)?);
    }
    Ok(locks)
}

fn operation_lock_targets(
    components: &[ComponentId],
    paths: &ComponentPaths,
) -> BTreeMap<PathBuf, ComponentId> {
    let mut targets = BTreeMap::new();
    for component in components {
        targets
            .entry(paths.operation_lock_path(component))
            .or_insert_with(|| component.clone());
    }
    targets
}

fn environment_flag(name: &str) -> bool {
    std::env::var_os(name).is_some_and(|value| {
        if value.is_empty() {
            return true;
        }
        !matches!(
            value.to_string_lossy().trim().to_ascii_lowercase().as_str(),
            "0" | "false" | "no" | "off"
        )
    })
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum ReleaseChannel {
    #[default]
    Stable,
    Beta,
    Nightly,
}

impl ReleaseChannel {
    fn as_str(self) -> &'static str {
        match self {
            Self::Stable => "stable",
            Self::Beta => "beta",
            Self::Nightly => "nightly",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum InstallScope {
    #[default]
    User,
    System,
}

impl InstallScope {
    fn as_str(self) -> &'static str {
        match self {
            Self::User => "user",
            Self::System => "system",
        }
    }
}

fn validate_supported_install_policy(options: &InstallOptions) -> anyhow::Result<()> {
    let signed_registry_only =
        options.package.is_none() && options.components.iter().all(is_external_use_extension);
    if let Some(version) = options.version.as_deref() {
        parse_version(version).with_context(|| format!("invalid component version '{version}'"))?;
        if options.source == InstallSource::Homebrew {
            bail!("--version requires --source release because Homebrew does not support exact component version selection");
        }
    }
    if options.version.is_some()
        && options.channel != ReleaseChannel::Stable
        && !signed_registry_only
    {
        bail!("--version cannot be combined with a beta or nightly release channel");
    }
    if options.channel != ReleaseChannel::Stable && !signed_registry_only {
        bail!(
            "the '{}' channel is not declared by the selected component sources",
            options.channel.as_str()
        );
    }
    if options.scope == InstallScope::System {
        bail!(
            "system-scope installation is not declared by the selected component sources; use --scope user"
        );
    }
    Ok(())
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

fn enforce_provenance_policy(
    options: &InstallOptions,
    paths: &ComponentPaths,
) -> anyhow::Result<()> {
    let requested = match options.source {
        InstallSource::Homebrew => Some(InstallProvenance::Homebrew),
        InstallSource::Release => Some(InstallProvenance::GithubRelease),
        InstallSource::Auto => None,
    };
    let Some(requested) = requested else {
        return Ok(());
    };
    for component in &options.components {
        let state = find_state(component, paths)?;
        let Some(current) = state.provenance else {
            continue;
        };
        if state.presence == Presence::Managed && current != requested {
            if !options.migrate {
                bail!(
                    "component '{}' is managed through {:?}; changing to {:?} requires --migrate",
                    component,
                    current,
                    requested
                );
            }
            bail!(
                "component '{}' cannot yet migrate atomically from {:?} to {:?}; keep its current source",
                component,
                current,
                requested
            );
        }
    }
    Ok(())
}

fn parse_kind(value: &str) -> anyhow::Result<ComponentKind> {
    match value {
        "built-in" => Ok(ComponentKind::BuiltIn),
        "product" => Ok(ComponentKind::Product),
        "capability" => Ok(ComponentKind::Capability),
        "extension" => Ok(ComponentKind::Extension),
        _ => bail!("unknown component kind '{value}'"),
    }
}

fn required_value<'a>(args: &'a [String], index: usize, option: &str) -> anyhow::Result<&'a str> {
    args.get(index)
        .map(String::as_str)
        .with_context(|| format!("{option} requires a value"))
}

#[cfg(test)]
mod tests {
    use super::super::state::{ComponentState, Trust};
    use super::*;

    #[test]
    fn parses_install_options_in_any_order() {
        let args = [
            "use/acme/slack",
            "--from",
            "./package",
            "--allow-unsigned",
            "--json",
            "--force",
        ]
        .map(str::to_string);
        let options = InstallOptions::parse(&args).unwrap();
        assert_eq!(options.components[0].as_str(), "use/acme/slack");
        assert_eq!(options.package, Some(PathBuf::from("./package")));
        assert!(options.allow_unsigned);
        assert!(options.force);
        assert!(options.json);
    }

    #[test]
    fn rejects_conflicting_list_and_update_options() {
        assert!(
            ListOptions::parse(&["--installed".to_string(), "--available".to_string()]).is_err()
        );
        assert!(UpdateOptions::parse(&["--all".to_string(), "use".to_string()]).is_err());
        assert!(InstallOptions::parse(&[
            "box".to_string(),
            "--dry-run".to_string(),
            format!("--plan-digest={}", "a".repeat(64)),
        ])
        .is_err());
        assert!(UninstallOptions::parse(&[
            "box".to_string(),
            format!("--plan-digest={}", "A".repeat(64)),
        ])
        .is_err());
    }

    #[test]
    fn upgrade_all_selects_only_managed_products() {
        let state = |id: &str, kind, presence| ComponentState {
            id: ComponentId::parse(id).unwrap(),
            kind,
            description: String::new(),
            presence,
            health: Health::Ready,
            update: UpdateState::Unknown,
            trust: Trust::FirstParty,
            provenance: Some(InstallProvenance::GithubRelease),
            version: Some("1.0.0".to_string()),
            path: Some(PathBuf::from("/tmp/component")),
            message: None,
        };

        assert!(is_upgrade_all_candidate(&state(
            "use",
            ComponentKind::Product,
            Presence::Managed,
        )));
        assert!(!is_upgrade_all_candidate(&state(
            "use/browser",
            ComponentKind::Capability,
            Presence::Managed,
        )));
        let mut registry_extension = state(
            "use/acme/slack",
            ComponentKind::Extension,
            Presence::Managed,
        );
        registry_extension.trust = Trust::RegistryTuf;
        assert!(is_upgrade_all_candidate(&registry_extension));
        let local_extension = state(
            "use/acme/local",
            ComponentKind::Extension,
            Presence::Managed,
        );
        assert!(!is_upgrade_all_candidate(&local_extension));
        assert!(!is_upgrade_all_candidate(&state(
            "search",
            ComponentKind::Product,
            Presence::External,
        )));
    }

    #[test]
    fn component_health_report_reuses_doctor_install_filter_without_paths() {
        let state = |id: &str, kind, presence, health| ComponentState {
            id: ComponentId::parse(id).unwrap(),
            kind,
            description: String::new(),
            presence,
            health,
            update: UpdateState::Unknown,
            trust: Trust::FirstParty,
            provenance: None,
            version: Some("1.0.0".to_string()),
            path: Some(PathBuf::from("/sensitive/install/path")),
            message: Some("probe output must stay private".to_string()),
        };
        let checks = installed_component_checks(ComponentReport {
            schema_version: 1,
            components: vec![
                state(
                    "code",
                    ComponentKind::BuiltIn,
                    Presence::Bundled,
                    Health::Ready,
                ),
                state(
                    "box",
                    ComponentKind::Product,
                    Presence::Managed,
                    Health::Broken,
                ),
                state(
                    "search",
                    ComponentKind::Product,
                    Presence::Missing,
                    Health::Unknown,
                ),
            ],
            external_tools: Vec::new(),
        });
        let public = checks
            .into_iter()
            .map(|state| ComponentHealthCheck {
                component: state.id,
                status: component_health_status(state.presence, state.health),
            })
            .collect::<Vec<_>>();

        assert_eq!(public.len(), 2);
        assert_eq!(public[0].status, ComponentHealthStatus::Ready);
        assert_eq!(public[1].status, ComponentHealthStatus::Broken);
        let serialized = serde_json::to_string(&public).unwrap();
        assert!(!serialized.contains("sensitive"));
        assert!(!serialized.contains("probe output"));
    }
}
