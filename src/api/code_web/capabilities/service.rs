use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};
use tokio::fs;
use tokio::process::Command;
use tokio::time::timeout;

use crate::api::code_web::state::CodeWebState;
use crate::config;

const ACTION_TIMEOUT: Duration = Duration::from_secs(180);

pub(in crate::api::code_web) struct CapabilitiesService {
    state: Arc<CodeWebState>,
}

impl CapabilitiesService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) fn overview(&self) -> serde_json::Value {
        let dirs = self.capability_dirs();
        json!({
            "app": "a3s-code-web",
            "workspaceRoot": self.state.default_workspace.display().to_string(),
            "configPath": self.state.config_path.display().to_string(),
            "defaultModel": self.state.current_default_model(),
            "dirs": dirs
                .iter()
                .map(capability_dir_summary)
                .collect::<Vec<_>>(),
        })
    }

    pub(in crate::api::code_web) async fn ensure_dirs(&self) -> BootResult<serde_json::Value> {
        let dirs = self.capability_dirs();
        for dir in &dirs {
            fs::create_dir_all(&dir.path).await.map_err(fs_error)?;
        }
        fs::create_dir_all(self.kb_dir().join("sources"))
            .await
            .map_err(fs_error)?;
        fs::create_dir_all(self.kb_dir().join("wiki"))
            .await
            .map_err(fs_error)?;
        Ok(self.overview())
    }

    pub(in crate::api::code_web) fn lifecycles(&self) -> serde_json::Value {
        lifecycle_catalog()
    }

    pub(in crate::api::code_web) async fn run_action(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let family = required_text(&request, "family")?;
        let action = required_text(&request, "action")?;
        let args = build_code_action_args(&request, family, action)?;
        let started = Instant::now();
        let executable = std::env::current_exe().map_err(|error| {
            BootError::Internal(format!("could not locate current a3s executable: {error}"))
        })?;

        let mut command = Command::new(executable);
        command
            .args(&args)
            .current_dir(&self.state.default_workspace)
            .kill_on_drop(true);

        let output = timeout(ACTION_TIMEOUT, command.output())
            .await
            .map_err(|_| {
                BootError::Internal(format!(
                    "a3s code {family} {action} timed out after {} seconds",
                    ACTION_TIMEOUT.as_secs()
                ))
            })?
            .map_err(|error| {
                BootError::Internal(format!("failed to run a3s code action: {error}"))
            })?;

        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        let exit_code = output.status.code();

        Ok(json!({
            "success": output.status.success(),
            "family": family,
            "action": action,
            "args": args,
            "command": format!("a3s {}", args.join(" ")),
            "exitCode": exit_code,
            "stdout": stdout,
            "stderr": stderr,
            "durationMs": started.elapsed().as_millis() as u64,
        }))
    }

    fn capability_dirs(&self) -> Vec<CapabilityDir> {
        vec![
            CapabilityDir::new(
                "agents",
                "Agents",
                "agent",
                config::agent_dir(),
                "Local agent definitions",
            ),
            CapabilityDir::new("mcp", "MCP", "mcp", config::mcp_dir(), "MCP server assets"),
            CapabilityDir::new(
                "skills",
                "Skills",
                "skill",
                self.skill_dir(),
                "Local skill packages",
            ),
            CapabilityDir::new(
                "flows",
                "Flows",
                "flow",
                config::flow_dir(),
                "Workflow DAG assets",
            ),
            CapabilityDir::new(
                "okf",
                "OKF",
                "okf",
                self.okf_dir(),
                "Knowledge package assets",
            ),
            CapabilityDir::new(
                "memory",
                "Memory",
                "memory",
                config::memory_dir(),
                "Long-term memory store",
            ),
        ]
    }

    fn kb_dir(&self) -> PathBuf {
        self.state.default_workspace.join(".a3s").join("kb")
    }

    fn okf_dir(&self) -> PathBuf {
        self.state.default_workspace.join("okf")
    }

    fn skill_dir(&self) -> PathBuf {
        self.state.default_workspace.join(".a3s").join("skills")
    }
}

fn lifecycle_catalog() -> serde_json::Value {
    json!({
        "stages": [
            stage("create", "Create", "Draft a new local asset package"),
            stage("develop", "Develop", "Edit and validate package files locally"),
            stage("test", "Test", "Run service-backed tests"),
            stage("run", "Run", "Start a runtime execution"),
            stage("publish", "Publish", "Publish the local asset to A3S OS"),
            stage("deploy", "Deploy", "Sync or deploy a serving runtime binding"),
            stage("inspect", "Inspect", "Open or inspect an existing OS asset"),
            stage("activity", "Activity", "Inspect runtime activity"),
            stage("observe", "Observe", "Open status, logs, or view links"),
        ],
        "families": [
            lifecycle(
                "agentic-agent",
                "agents",
                "agent",
                "Agentic agent",
                "Agent as a Service",
                ["create", "develop", "run", "publish", "observe"],
                ["local", "clone", "list", "activity", "review", "publish", "run", "open", "logs", "status"],
            ),
            lifecycle(
                "application-agent",
                "agents",
                "agent",
                "Application agent",
                "Agent as a Service",
                ["create", "develop", "publish", "deploy", "observe"],
                ["local", "clone", "list", "activity", "review", "publish", "deploy", "open", "logs", "status"],
            ),
            lifecycle(
                "tool-agent",
                "agents",
                "agent",
                "Tool agent",
                "Function as a Service",
                ["create", "develop", "publish", "activity", "observe"],
                ["local", "clone", "list", "activity", "review", "publish", "open", "logs", "status"],
            ),
            lifecycle(
                "mcp-server",
                "mcp",
                "mcp",
                "MCP server",
                "Function as a Service",
                ["create", "develop", "run", "test", "publish", "deploy", "observe"],
                ["local", "clone", "list", "activity", "review", "publish", "run", "deploy", "test", "open", "logs", "status"],
            ),
            lifecycle(
                "skill",
                "skills",
                "skill",
                "Skill",
                "Function as a Service",
                ["create", "develop", "publish", "deploy", "inspect", "activity"],
                ["local", "clone", "list", "activity", "review", "publish", "deploy", "open", "status"],
            ),
            lifecycle(
                "okf-package",
                "okf",
                "okf",
                "OKF knowledge package",
                "Knowledge service",
                ["create", "develop", "publish", "deploy", "inspect", "activity"],
                ["local", "clone", "list", "activity", "review", "publish", "deploy", "status"],
            ),
            lifecycle(
                "workflow-flow",
                "flows",
                "flow",
                "Workflow flow",
                "Workflow as a Service",
                ["create", "develop", "publish", "run", "deploy", "activity", "observe"],
                ["local", "clone", "list", "activity", "review", "publish", "run", "deploy", "open", "logs", "status"],
            ),
        ],
    })
}

fn stage(id: &str, label: &str, description: &str) -> serde_json::Value {
    json!({
        "id": id,
        "label": label,
        "description": description,
    })
}

fn lifecycle<const STAGES: usize, const ACTIONS: usize>(
    id: &str,
    dir_id: &str,
    family: &str,
    label: &str,
    service: &str,
    stages: [&str; STAGES],
    actions: [&str; ACTIONS],
) -> serde_json::Value {
    json!({
        "id": id,
        "dirId": dir_id,
        "family": family,
        "label": label,
        "service": service,
        "stages": stages.to_vec(),
        "actions": actions.to_vec(),
    })
}

fn build_code_action_args(request: &Value, family: &str, action: &str) -> BootResult<Vec<String>> {
    match family {
        "agent" => build_agent_action_args(request, action),
        "mcp" => build_asset_action_args(
            request,
            "mcp",
            action,
            &[
                "local", "clone", "list", "activity", "review", "publish", "run", "deploy", "test",
                "open", "logs", "status",
            ],
        ),
        "skill" => build_asset_action_args(
            request,
            "skill",
            action,
            &[
                "local", "clone", "list", "activity", "review", "publish", "deploy", "open",
                "status",
            ],
        ),
        "flow" => build_asset_action_args(
            request,
            "flow",
            action,
            &[
                "local", "clone", "list", "activity", "review", "publish", "run", "deploy", "open",
                "logs", "status",
            ],
        ),
        "okf" => build_asset_action_args(
            request,
            "okf",
            action,
            &[
                "local", "clone", "list", "activity", "review", "publish", "deploy", "status",
            ],
        ),
        other => Err(BootError::BadRequest(format!(
            "unsupported capability family `{other}`"
        ))),
    }
}

fn build_agent_action_args(request: &Value, action: &str) -> BootResult<Vec<String>> {
    match action {
        "local" | "list" | "activity" => query_action_args("agent", action, request),
        "clone" => clone_action_args("agent", request),
        "review" | "run" | "deploy" => path_action_args("agent", action, request),
        "publish" => {
            let kind = required_agent_kind(request)?;
            let mut args = vec![
                "code".to_string(),
                "agent".to_string(),
                "publish".to_string(),
                kind,
            ];
            append_optional_field(&mut args, request, "path");
            Ok(args)
        }
        "open" | "logs" | "status" => {
            let mut args = vec!["code".to_string(), "agent".to_string(), action.to_string()];
            if let Some(kind) = optional_agent_kind(request)? {
                args.push(kind);
            }
            append_optional_field(&mut args, request, "path");
            Ok(args)
        }
        other => Err(BootError::BadRequest(format!(
            "unsupported agent action `{other}`"
        ))),
    }
}

fn build_asset_action_args(
    request: &Value,
    family: &str,
    action: &str,
    supported: &[&str],
) -> BootResult<Vec<String>> {
    if !supported.contains(&action) {
        return Err(BootError::BadRequest(format!(
            "unsupported {family} action `{action}`"
        )));
    }

    match action {
        "local" | "list" | "activity" => query_action_args(family, action, request),
        "clone" => clone_action_args(family, request),
        "review" | "publish" | "deploy" | "test" | "run" | "open" | "logs" | "status" => {
            path_action_args(family, action, request)
        }
        other => Err(BootError::BadRequest(format!(
            "unsupported {family} action `{other}`"
        ))),
    }
}

fn query_action_args(family: &str, action: &str, request: &Value) -> BootResult<Vec<String>> {
    let mut args = vec!["code".to_string(), family.to_string(), action.to_string()];
    if let Some(query) = optional_text(request, "query") {
        args.extend(query.split_whitespace().map(str::to_string));
    }
    Ok(args)
}

fn clone_action_args(family: &str, request: &Value) -> BootResult<Vec<String>> {
    Ok(vec![
        "code".to_string(),
        family.to_string(),
        "clone".to_string(),
        required_text(request, "url")?.to_string(),
    ])
}

fn path_action_args(family: &str, action: &str, request: &Value) -> BootResult<Vec<String>> {
    let mut args = vec!["code".to_string(), family.to_string(), action.to_string()];
    append_optional_field(&mut args, request, "path");
    Ok(args)
}

fn append_optional_field(args: &mut Vec<String>, request: &Value, name: &str) {
    if let Some(value) = optional_text(request, name) {
        args.push(value.to_string());
    }
}

fn required_text<'a>(request: &'a Value, name: &str) -> BootResult<&'a str> {
    optional_text(request, name).ok_or_else(|| BootError::BadRequest(format!("{name} is required")))
}

fn optional_text<'a>(request: &'a Value, name: &str) -> Option<&'a str> {
    request
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
}

fn required_agent_kind(request: &Value) -> BootResult<String> {
    optional_agent_kind(request)?
        .ok_or_else(|| BootError::BadRequest("kind is required for agent publish".to_string()))
}

fn optional_agent_kind(request: &Value) -> BootResult<Option<String>> {
    let Some(kind) = optional_text(request, "kind") else {
        return Ok(None);
    };
    match kind {
        "agentic" | "application" | "tool" => Ok(Some(kind.to_string())),
        other => Err(BootError::BadRequest(format!(
            "unsupported agent kind `{other}`"
        ))),
    }
}

struct CapabilityDir {
    id: &'static str,
    label: &'static str,
    kind: &'static str,
    path: PathBuf,
    description: &'static str,
}

impl CapabilityDir {
    fn new(
        id: &'static str,
        label: &'static str,
        kind: &'static str,
        path: PathBuf,
        description: &'static str,
    ) -> Self {
        Self {
            id,
            label,
            kind,
            path,
            description,
        }
    }
}

fn capability_dir_summary(dir: &CapabilityDir) -> serde_json::Value {
    let exists = dir.path.is_dir();
    json!({
        "id": dir.id,
        "label": dir.label,
        "kind": dir.kind,
        "path": dir.path.display().to_string(),
        "description": dir.description,
        "exists": exists,
        "itemCount": if exists { top_level_item_count(&dir.path) } else { 0 },
    })
}

fn top_level_item_count(path: &Path) -> usize {
    std::fs::read_dir(path)
        .ok()
        .map(|entries| entries.filter_map(Result::ok).count())
        .unwrap_or(0)
}

fn fs_error(error: std::io::Error) -> BootError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => BootError::Forbidden(error.to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            BootError::BadRequest(error.to_string())
        }
        _ => BootError::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn lifecycle_catalog_exposes_mcp_run_without_debug_or_invoke() {
        let catalog = lifecycle_catalog();
        let stages = catalog["stages"].as_array().unwrap();
        assert!(stages.iter().any(|stage| stage["id"] == "run"));
        assert!(stages.iter().all(|stage| stage["id"] != "debug"));

        let mcp = catalog["families"]
            .as_array()
            .unwrap()
            .iter()
            .find(|family| family["id"] == "mcp-server")
            .expect("mcp lifecycle");
        let mcp_stages = mcp["stages"].as_array().unwrap();
        let mcp_actions = mcp["actions"].as_array().unwrap();
        assert!(mcp_stages.iter().any(|stage| stage == "run"));
        assert!(mcp_actions.iter().any(|action| action == "run"));
        assert!(mcp_stages.iter().all(|stage| stage != "debug"));
        assert!(mcp_actions.iter().all(|action| action != "debug"));
        assert!(mcp_actions.iter().all(|action| action != "invoke"));
    }

    #[test]
    fn capability_action_builder_rejects_mcp_debug_and_invoke() {
        let request = json!({"path": "mcps/weather"});
        assert_eq!(
            build_code_action_args(&request, "mcp", "run").unwrap(),
            vec!["code", "mcp", "run", "mcps/weather"]
        );
        assert!(build_code_action_args(&request, "mcp", "debug").is_err());
        assert!(build_code_action_args(&request, "mcp", "invoke").is_err());
    }
}
