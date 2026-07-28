use std::collections::{BTreeMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::sync::Mutex;
use tokio::time::timeout;

use super::controller::{
    PluginApplyRequest, PluginPackageToggleRequest, PluginPlanRequest, PluginReloadRequest,
    PluginToggleRequest,
};
use crate::api::code_web::session_runtime::rebuild_code_web_sessions;
use crate::api::code_web::state::CodeWebState;
use crate::tui::skills::{
    agent_skill_dirs, count_skill_files, load_disabled_skills, load_skills, save_disabled_skills,
};

pub(in crate::api::code_web) struct PluginsService {
    state: Arc<CodeWebState>,
    operation_lock: Mutex<()>,
}

const PLUGIN_OPERATION_TIMEOUT: Duration = Duration::from_secs(180);
const MARKETPLACE_REFRESH_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_PLUGIN_COMMAND_OUTPUT: usize = 4 * 1024 * 1024;

impl PluginsService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self {
            state,
            operation_lock: Mutex::new(()),
        }
    }

    pub(in crate::api::code_web) async fn list(
        &self,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        Ok(self.snapshot(workspace, Vec::new(), false))
    }

    pub(in crate::api::code_web) async fn set_enabled(
        &self,
        raw_name: &str,
        request: PluginToggleRequest,
    ) -> BootResult<Value> {
        let name = normalize_skill_name(raw_name)?;
        let dirs = self.default_skill_dirs();
        let skills = load_skills(&dirs);
        if !skills.iter().any(|(skill_name, _)| skill_name == &name) {
            return Err(BootError::NotFound(format!(
                "skill/plugin `{name}` was not found"
            )));
        }

        let mut disabled = load_disabled_skills();
        let enabled = apply_enabled(&mut disabled, &name, request.enabled);
        save_disabled_skills(&disabled);

        let mut response = self.snapshot(None, Vec::new(), false);
        if let Some(object) = response.as_object_mut() {
            object.insert(
                "updated".to_string(),
                json!({
                    "name": name,
                    "enabled": enabled,
                }),
            );
        }
        Ok(response)
    }

    pub(in crate::api::code_web) async fn reload(
        &self,
        request: PluginReloadRequest,
    ) -> BootResult<Value> {
        let rebuild_sessions = request.rebuild_sessions.unwrap_or(true);
        let rebuilt_sessions = if rebuild_sessions {
            self.rebuild_sessions().await?
        } else {
            Vec::new()
        };

        Ok(self.snapshot(None, rebuilt_sessions, true))
    }

    pub(in crate::api::code_web) fn activities(&self) -> BootResult<Value> {
        let Some(registry) = self.state.use_registry() else {
            return Ok(json!({
                "schemaVersion": 1,
                "available": false,
                "generation": 0,
                "revision": "",
                "items": [],
            }));
        };
        let mut value = serde_json::to_value(registry.activity_catalog())
            .map_err(|error| BootError::Internal(error.to_string()))?;
        if let Some(object) = value.as_object_mut() {
            object.insert("available".to_string(), Value::Bool(true));
        }
        Ok(value)
    }

    pub(in crate::api::code_web) fn activity_content(&self, key: &str) -> BootResult<Value> {
        let key = normalize_activity_key(key)?;
        let registry = self.state.use_registry().ok_or_else(|| {
            BootError::ServiceUnavailable("A3S Use is not installed or ready".to_string())
        })?;
        let content = registry.activity_content(&key).ok_or_else(|| {
            BootError::NotFound(format!(
                "enabled Activity Bar contribution `{key}` was not found"
            ))
        })?;
        serde_json::to_value(content).map_err(|error| BootError::Internal(error.to_string()))
    }

    pub(in crate::api::code_web) async fn marketplace(&self) -> BootResult<Value> {
        let component_paths =
            a3s::components::ComponentPaths::from_env_at(&self.state.default_workspace)
                .map_err(|error| BootError::Internal(error.to_string()))?;
        let registry_root = self
            .state
            .config_path
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join("registries");
        let store = a3s::registry::RegistryStore::new(registry_root);
        let records = store
            .list()
            .map_err(|error| BootError::Internal(error.to_string()))?;
        let installed = self
            .state
            .use_registry()
            .map(|registry| registry.package_statuses())
            .unwrap_or_default();

        let mut registries = Vec::new();
        let mut items = Vec::new();
        let mut bundled_identities = HashSet::new();
        match a3s::components::list_release_bundles_with(&component_paths).await {
            Ok(packages) if !packages.is_empty() => {
                let package_count = packages.len();
                for package in packages {
                    let component_id = package.component_id;
                    let package_id = package.package_id;
                    let version = package.version;
                    let package_sha256 = package.package_sha256;
                    let installed_enabled = installed.get(&component_id).copied();
                    bundled_identities.insert((
                        package_id.clone(),
                        version.clone(),
                        "stable".to_string(),
                    ));
                    items.push(json!({
                        "componentId": component_id,
                        "packageId": package_id.clone(),
                        "displayName": package_display_name(&package_id),
                        "registryName": "A3S 发行包",
                        "registryUrl": "a3s-use://release-bundles",
                        "sourceKind": "release-bundle",
                        "version": version.clone(),
                        "channel": "stable",
                        "target": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                        "archiveName": format!("release-bundle/{package_id}"),
                        "length": package.byte_count,
                        "sha256": package_sha256.clone(),
                        "integrityDigest": package_sha256,
                        "installed": installed_enabled.is_some(),
                        "enabled": installed_enabled.unwrap_or(false),
                    }));
                }
                registries.push(json!({
                    "name": "A3S 发行包",
                    "url": "a3s-use://release-bundles",
                    "sourceKind": "release-bundle",
                    "configured": true,
                    "verified": true,
                    "hostTarget": format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
                    "metadata": {
                        "packageTargets": package_count,
                    },
                }));
            }
            Ok(_) => {}
            Err(error) => {
                registries.push(json!({
                    "name": "A3S 发行包",
                    "url": "a3s-use://release-bundles",
                    "sourceKind": "release-bundle",
                    "configured": true,
                    "verified": false,
                    "error": concise_error(&error.to_string()),
                }));
            }
        }
        for record in records {
            if !record.configured {
                registries.push(json!({
                    "name": record.name,
                    "url": record.url,
                    "sourceKind": "registry",
                    "configured": false,
                    "verified": false,
                }));
                continue;
            }
            let trusted = match record.trusted_registry(&component_paths.state_root) {
                Ok(trusted) => trusted,
                Err(error) => {
                    registries.push(json!({
                        "name": record.name,
                        "url": record.url,
                        "sourceKind": "registry",
                        "configured": true,
                        "verified": false,
                        "error": concise_error(&error.to_string()),
                    }));
                    continue;
                }
            };
            let catalog = match timeout(
                MARKETPLACE_REFRESH_TIMEOUT,
                a3s_use_extension::list_remote_packages(&trusted),
            )
            .await
            {
                Ok(Ok(catalog)) => catalog,
                Ok(Err(error)) => {
                    registries.push(json!({
                        "name": record.name,
                        "url": record.url,
                        "sourceKind": "registry",
                        "configured": true,
                        "verified": false,
                        "error": concise_error(&error.to_string()),
                    }));
                    continue;
                }
                Err(_) => {
                    registries.push(json!({
                        "name": record.name,
                        "url": record.url,
                        "sourceKind": "registry",
                        "configured": true,
                        "verified": false,
                        "error": format!("registry verification timed out after {} seconds", MARKETPLACE_REFRESH_TIMEOUT.as_secs()),
                    }));
                    continue;
                }
            };
            for package in latest_signed_packages(catalog.packages) {
                if bundled_identities.contains(&(
                    package.package_id.clone(),
                    package.version.clone(),
                    package.channel.clone(),
                )) {
                    continue;
                }
                let component_id = format!("use/{}", package.package_id);
                let signed_plan_digest = package
                    .plan_digest()
                    .map_err(|error| BootError::Internal(error.to_string()))?;
                let installed_enabled = installed.get(&component_id).copied();
                items.push(json!({
                    "componentId": component_id,
                    "packageId": package.package_id,
                    "displayName": package_display_name(&package.package_id),
                    "registryName": package.registry_name,
                    "registryUrl": package.registry_url,
                    "sourceKind": "registry",
                    "version": package.version,
                    "channel": package.channel,
                    "target": package.target,
                    "archiveName": package.archive_name,
                    "length": package.length,
                    "sha256": package.sha256,
                    "signedPlanDigest": signed_plan_digest,
                    "installed": installed_enabled.is_some(),
                    "enabled": installed_enabled.unwrap_or(false),
                }));
            }
            registries.push(json!({
                "name": record.name,
                "url": record.url,
                "sourceKind": "registry",
                "configured": true,
                "verified": true,
                "metadata": catalog.metadata,
                "hostTarget": catalog.host_target,
            }));
        }
        items.sort_by(|left, right| {
            left["componentId"]
                .as_str()
                .cmp(&right["componentId"].as_str())
                .then_with(|| left["channel"].as_str().cmp(&right["channel"].as_str()))
        });
        Ok(json!({
            "schemaVersion": 1,
            "verifiedAt": chrono::Utc::now().to_rfc3339(),
            "registries": registries,
            "items": items,
        }))
    }

    pub(in crate::api::code_web) async fn plan_operation(
        &self,
        request: PluginPlanRequest,
    ) -> BootResult<Value> {
        let _guard = self.operation_lock.lock().await;
        let args = plugin_operation_args(
            &request.action,
            &request.component_id,
            request.version.as_deref(),
            request.channel.as_deref(),
            None,
        )?;
        self.run_a3s_json(args, JsonOutputOwner::Root).await
    }

    pub(in crate::api::code_web) async fn apply_operation(
        &self,
        request: PluginApplyRequest,
    ) -> BootResult<Value> {
        validate_plan_digest(&request.plan_digest)?;
        let _guard = self.operation_lock.lock().await;
        let args = plugin_operation_args(
            &request.action,
            &request.component_id,
            request.version.as_deref(),
            request.channel.as_deref(),
            Some(&request.plan_digest),
        )?;
        self.run_a3s_json(args, JsonOutputOwner::Root).await
    }

    pub(in crate::api::code_web) async fn set_package_enabled(
        &self,
        request: PluginPackageToggleRequest,
    ) -> BootResult<Value> {
        let package_id = normalize_component_id(&request.component_id)?;
        let package_id = package_id
            .strip_prefix("use/")
            .ok_or_else(|| BootError::BadRequest("invalid Use package ID".to_string()))?;
        let _guard = self.operation_lock.lock().await;
        self.run_a3s_json(
            use_extension_toggle_args(package_id, request.enabled),
            JsonOutputOwner::UseProxy,
        )
        .await
    }

    async fn run_a3s_json(
        &self,
        args: Vec<String>,
        output_owner: JsonOutputOwner,
    ) -> BootResult<Value> {
        let executable = std::env::current_exe().map_err(|error| {
            BootError::Internal(format!("could not locate current a3s executable: {error}"))
        })?;
        let mut command = Command::new(executable);
        command
            .arg("--config")
            .arg(&self.state.config_path)
            .arg("--directory")
            .arg(&self.state.default_workspace)
            .args(json_invocation_args(output_owner, args))
            .current_dir(&self.state.default_workspace)
            .kill_on_drop(true);
        let output = timeout(PLUGIN_OPERATION_TIMEOUT, command.output())
            .await
            .map_err(|_| {
                BootError::GatewayTimeout(format!(
                    "plugin operation timed out after {} seconds",
                    PLUGIN_OPERATION_TIMEOUT.as_secs()
                ))
            })?
            .map_err(|error| BootError::BadGateway(format!("failed to run a3s: {error}")))?;
        if output.stdout.len() > MAX_PLUGIN_COMMAND_OUTPUT
            || output.stderr.len() > MAX_PLUGIN_COMMAND_OUTPUT
        {
            return Err(BootError::BadGateway(
                "plugin operation output exceeded the supported size".to_string(),
            ));
        }
        let value: Value = serde_json::from_slice(&output.stdout).map_err(|error| {
            BootError::BadGateway(format!(
                "a3s returned invalid JSON: {error}{}",
                stderr_suffix(&output.stderr)
            ))
        })?;
        let ok = value.get("ok").and_then(Value::as_bool) == Some(true);
        if !output.status.success() || !ok {
            let message = value
                .get("message")
                .and_then(Value::as_str)
                .or_else(|| value.pointer("/error/message").and_then(Value::as_str))
                .unwrap_or("plugin operation failed");
            return Err(BootError::Conflict(concise_error(message)));
        }
        value
            .get("data")
            .cloned()
            .ok_or_else(|| BootError::BadGateway("a3s JSON response has no data".to_string()))
    }

    fn snapshot(
        &self,
        workspace: Option<String>,
        rebuilt_sessions: Vec<Value>,
        reloaded: bool,
    ) -> Value {
        let workspace = self.workspace_from_request(workspace);
        let workspace_text = workspace.display().to_string();
        let dirs = agent_skill_dirs(&workspace_text);
        let disabled = load_disabled_skills();
        let mut sources_by_name: BTreeMap<String, Vec<Value>> = BTreeMap::new();

        let dir_summaries = dirs
            .iter()
            .map(|dir| {
                let dir_skills = load_skills(std::slice::from_ref(dir));
                for (name, _) in &dir_skills {
                    sources_by_name
                        .entry(name.clone())
                        .or_default()
                        .push(json!({
                            "path": dir.display().to_string(),
                        }));
                }
                json!({
                    "path": dir.display().to_string(),
                    "exists": dir.is_dir(),
                    "itemCount": count_skill_files(std::slice::from_ref(dir)),
                })
            })
            .collect::<Vec<_>>();

        let skills = load_skills(&dirs);
        let total = skills.len();
        let items = skills
            .into_iter()
            .map(|(name, description)| {
                let enabled = !disabled.contains(&name);
                json!({
                    "name": name,
                    "command": format!("/{name}"),
                    "description": description,
                    "enabled": enabled,
                    "sources": sources_by_name.remove(&name).unwrap_or_default(),
                })
            })
            .collect::<Vec<_>>();
        let disabled_count = items
            .iter()
            .filter(|item| item.get("enabled").and_then(Value::as_bool) == Some(false))
            .count();

        json!({
            "workspaceRoot": workspace.display().to_string(),
            "dirs": dir_summaries,
            "items": items,
            "total": total,
            "enabledCount": total.saturating_sub(disabled_count),
            "disabledCount": disabled_count,
            "reloaded": reloaded,
            "reloadedAt": if reloaded { Some(chrono::Utc::now().to_rfc3339()) } else { None },
            "rebuiltSessions": rebuilt_sessions,
        })
    }

    async fn rebuild_sessions(&self) -> BootResult<Vec<Value>> {
        rebuild_code_web_sessions(self.state.as_ref()).await
    }

    fn workspace_from_request(&self, workspace: Option<String>) -> PathBuf {
        workspace
            .as_deref()
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| self.state.default_workspace.clone())
    }

    fn default_skill_dirs(&self) -> Vec<PathBuf> {
        agent_skill_dirs(&self.state.default_workspace.display().to_string())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum JsonOutputOwner {
    Root,
    UseProxy,
}

fn json_invocation_args(output_owner: JsonOutputOwner, args: Vec<String>) -> Vec<String> {
    let mut invocation = Vec::with_capacity(args.len() + 4);
    if output_owner == JsonOutputOwner::Root {
        invocation.extend(["--output".to_string(), "json".to_string()]);
    }
    invocation.extend(["--non-interactive".to_string(), "--no-progress".to_string()]);
    invocation.extend(args);
    invocation
}

fn use_extension_toggle_args(package_id: &str, enabled: bool) -> Vec<String> {
    vec![
        "use".to_string(),
        "extension".to_string(),
        if enabled { "enable" } else { "disable" }.to_string(),
        package_id.to_string(),
        "--json".to_string(),
    ]
}

fn latest_signed_packages(
    packages: Vec<a3s_use_extension::ResolvedRemotePackage>,
) -> Vec<a3s_use_extension::ResolvedRemotePackage> {
    let mut latest =
        BTreeMap::<(String, String, String), a3s_use_extension::ResolvedRemotePackage>::new();
    for package in packages {
        let key = (
            package.registry_name.clone(),
            package.package_id.clone(),
            package.channel.clone(),
        );
        let replace = latest.get(&key).is_none_or(|current| {
            match (
                a3s_updater::parse_version(&package.version),
                a3s_updater::parse_version(&current.version),
            ) {
                (Ok(candidate), Ok(installed)) => candidate > installed,
                _ => package.version > current.version,
            }
        });
        if replace {
            latest.insert(key, package);
        }
    }
    latest.into_values().collect()
}

fn package_display_name(package_id: &str) -> String {
    if package_id == "a3s/science" {
        return "科研".to_string();
    }
    package_id
        .rsplit('/')
        .next()
        .unwrap_or(package_id)
        .split('-')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut characters = part.chars();
            characters
                .next()
                .map(|first| first.to_ascii_uppercase().to_string() + characters.as_str())
                .unwrap_or_default()
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn plugin_operation_args(
    action: &str,
    component_id: &str,
    version: Option<&str>,
    channel: Option<&str>,
    plan_digest: Option<&str>,
) -> BootResult<Vec<String>> {
    let component_id = normalize_component_id(component_id)?;
    let action = action.trim();
    let mut args = match action {
        "install" => vec!["install".to_string(), component_id],
        "upgrade" => vec!["upgrade".to_string(), component_id],
        "uninstall" => vec!["uninstall".to_string(), component_id],
        _ => {
            return Err(BootError::BadRequest(
                "plugin action must be install, upgrade, or uninstall".to_string(),
            ))
        }
    };
    if action == "install" {
        if let Some(version) = normalize_optional_value(version, "version", 64)? {
            args.extend(["--version".to_string(), version]);
        }
        if let Some(channel) = normalize_optional_value(channel, "channel", 16)? {
            if !matches!(channel.as_str(), "stable" | "beta" | "nightly") {
                return Err(BootError::BadRequest(
                    "channel must be stable, beta, or nightly".to_string(),
                ));
            }
            args.extend(["--channel".to_string(), channel]);
        }
    } else if version.is_some() || channel.is_some() {
        return Err(BootError::BadRequest(format!(
            "{action} does not accept version or channel"
        )));
    }
    if let Some(plan_digest) = plan_digest {
        validate_plan_digest(plan_digest)?;
        args.extend([
            "--plan-digest".to_string(),
            plan_digest.to_string(),
            "--yes".to_string(),
        ]);
    } else {
        args.push("--dry-run".to_string());
    }
    Ok(args)
}

fn normalize_component_id(value: &str) -> BootResult<String> {
    let value = value.trim();
    let segments = value.split('/').collect::<Vec<_>>();
    if segments.len() != 3
        || segments[0] != "use"
        || !segments[1..].iter().copied().all(valid_segment)
    {
        return Err(BootError::BadRequest(
            "plugin component IDs must be use/<publisher>/<name>".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn normalize_activity_key(value: &str) -> BootResult<String> {
    let value = value.trim();
    let segments = value.split(':').collect::<Vec<_>>();
    if segments.len() != 2 || !segments.into_iter().all(valid_segment) {
        return Err(BootError::BadRequest(
            "invalid Activity Bar contribution key".to_string(),
        ));
    }
    Ok(value.to_string())
}

fn valid_segment(value: &str) -> bool {
    let mut characters = value.chars();
    matches!(characters.next(), Some(first) if first.is_ascii_lowercase())
        && characters.all(|character| {
            character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
        })
}

fn normalize_optional_value(
    value: Option<&str>,
    label: &str,
    max_chars: usize,
) -> BootResult<Option<String>> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| {
            if value.chars().count() > max_chars || value.chars().any(char::is_whitespace) {
                Err(BootError::BadRequest(format!("invalid plugin {label}")))
            } else {
                Ok(value.to_string())
            }
        })
        .transpose()
}

fn validate_plan_digest(value: &str) -> BootResult<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(BootError::BadRequest(
            "planDigest must contain 64 lowercase hexadecimal characters".to_string(),
        ));
    }
    Ok(())
}

fn concise_error(value: &str) -> String {
    let value = value.trim().replace(['\n', '\r'], " ");
    let mut concise = value.chars().take(500).collect::<String>();
    if value.chars().count() > 500 {
        concise.push('…');
    }
    concise
}

fn stderr_suffix(stderr: &[u8]) -> String {
    let stderr = concise_error(&String::from_utf8_lossy(stderr));
    if stderr.is_empty() {
        String::new()
    } else {
        format!(": {stderr}")
    }
}

fn normalize_skill_name(raw_name: &str) -> BootResult<String> {
    let name = raw_name.trim().trim_start_matches('/').trim();
    if name.is_empty() {
        return Err(BootError::BadRequest(
            "skill/plugin name is required".to_string(),
        ));
    }
    Ok(name.to_string())
}

fn apply_enabled(disabled: &mut HashSet<String>, name: &str, enabled: Option<bool>) -> bool {
    let target_enabled = enabled.unwrap_or_else(|| disabled.contains(name));
    if target_enabled {
        disabled.remove(name);
    } else {
        disabled.insert(name.to_string());
    }
    target_enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn apply_enabled_matches_plugin_toggle_semantics() {
        let mut disabled = HashSet::from(["reviewer".to_string()]);
        assert!(apply_enabled(&mut disabled, "reviewer", None));
        assert!(!disabled.contains("reviewer"));

        assert!(!apply_enabled(&mut disabled, "reviewer", None));
        assert!(disabled.contains("reviewer"));

        assert!(apply_enabled(&mut disabled, "reviewer", Some(true)));
        assert!(!disabled.contains("reviewer"));

        assert!(!apply_enabled(&mut disabled, "reviewer", Some(false)));
        assert!(disabled.contains("reviewer"));
    }

    #[test]
    fn plugin_operation_requires_reviewed_digest_and_use_namespace() {
        assert_eq!(
            plugin_operation_args(
                "install",
                "use/a3s/science",
                Some("1.2.3"),
                Some("stable"),
                None,
            )
            .unwrap(),
            [
                "install",
                "use/a3s/science",
                "--version",
                "1.2.3",
                "--channel",
                "stable",
                "--dry-run",
            ]
        );
        let digest = "a".repeat(64);
        let apply =
            plugin_operation_args("uninstall", "use/a3s/science", None, None, Some(&digest))
                .unwrap();
        assert!(apply
            .windows(2)
            .any(|args| args == ["--plan-digest", digest.as_str()]));
        assert!(plugin_operation_args("install", "code", None, None, None).is_err());
        assert!(
            plugin_operation_args("install", "use/a3s/science", None, None, Some("unsigned"))
                .is_err()
        );
    }

    #[test]
    fn use_extension_toggle_keeps_json_output_owned_by_the_child_cli() {
        let invocation = json_invocation_args(
            JsonOutputOwner::UseProxy,
            use_extension_toggle_args("a3s/science", false),
        );

        assert_eq!(
            invocation,
            [
                "--non-interactive",
                "--no-progress",
                "use",
                "extension",
                "disable",
                "a3s/science",
                "--json",
            ]
        );
        assert!(!invocation
            .windows(2)
            .any(|arguments| arguments == ["--output", "json"]));
    }

    #[test]
    fn marketplace_uses_the_research_product_name_for_the_science_package() {
        assert_eq!(package_display_name("a3s/science"), "科研");
        assert_eq!(package_display_name("acme/data-tools"), "Data Tools");
    }
}
