use std::path::PathBuf;
use std::sync::Arc;

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};

use super::controller::{LoopActionRequest, LoopInitRequest, LoopRunPromptRequest};
use crate::api::code_web::state::CodeWebState;
use crate::tui::loop_engineering::{
    append_run_start, audit_loop, find_loop, init_loop, list_loops, loop_run_prompt_with_runtime,
    LoopAudit, LoopRuntimeMode, LoopSpec, LoopSummary,
};

const RUN_LOG_FILE: &str = "RUN_LOG.md";

pub(in crate::api::code_web) struct LoopsService {
    state: Arc<CodeWebState>,
}

impl LoopsService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) async fn list(
        &self,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.resolve_workspace(workspace.as_deref());
        let workspace_text = workspace.display().to_string();
        let loops = list_loops(&workspace_text);
        Ok(json!({
            "workspace": workspace_text,
            "patterns": loop_patterns_json(),
            "items": loops.iter().map(loop_summary_json).collect::<Vec<_>>(),
            "total": loops.len(),
        }))
    }

    pub(in crate::api::code_web) async fn init(
        &self,
        request: LoopInitRequest,
    ) -> BootResult<Value> {
        let workspace = self.resolve_workspace(request.workspace.as_deref());
        let workspace_text = workspace.display().to_string();
        let arg = init_arg(&request)?;
        let spec = init_loop(&workspace_text, &arg).map_err(loop_bad_request)?;
        let audit = audit_loop(&spec);
        let summary = LoopSummary {
            spec,
            audit,
            last_run: "never".to_string(),
        };
        Ok(json!({
            "workspace": workspace_text,
            "loop": loop_summary_json(&summary),
            "created": true,
        }))
    }

    pub(in crate::api::code_web) async fn get(
        &self,
        loop_id: &str,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.resolve_workspace(workspace.as_deref());
        let workspace_text = workspace.display().to_string();
        let summary = self.find_summary(&workspace_text, loop_id)?;
        Ok(json!({
            "workspace": workspace_text,
            "loop": loop_summary_json(&summary),
        }))
    }

    pub(in crate::api::code_web) async fn audit(
        &self,
        loop_id: &str,
        request: LoopActionRequest,
    ) -> BootResult<Value> {
        let workspace = self.resolve_workspace(request.workspace.as_deref());
        let workspace_text = workspace.display().to_string();
        let spec = find_loop(&workspace_text, loop_id).map_err(loop_not_found)?;
        let audit = audit_loop(&spec);
        Ok(json!({
            "workspace": workspace_text,
            "loopId": spec.id,
            "audit": loop_audit_json(&audit),
        }))
    }

    pub(in crate::api::code_web) async fn logs(
        &self,
        loop_id: &str,
        workspace: Option<String>,
    ) -> BootResult<Value> {
        let workspace = self.resolve_workspace(workspace.as_deref());
        let workspace_text = workspace.display().to_string();
        let spec = find_loop(&workspace_text, loop_id).map_err(loop_not_found)?;
        let path = spec.dir.join(RUN_LOG_FILE);
        let content = std::fs::read_to_string(&path).map_err(fs_error)?;
        Ok(json!({
            "workspace": workspace_text,
            "loopId": spec.id,
            "path": path.display().to_string(),
            "content": content,
        }))
    }

    pub(in crate::api::code_web) async fn run_prompt(
        &self,
        loop_id: &str,
        request: LoopRunPromptRequest,
    ) -> BootResult<Value> {
        let workspace = self.resolve_workspace(request.workspace.as_deref());
        let workspace_text = workspace.display().to_string();
        let spec = find_loop(&workspace_text, loop_id).map_err(loop_not_found)?;
        let runtime_mode = runtime_mode(&request);
        let os_available = matches!(runtime_mode, LoopRuntimeMode::OsAvailable);
        append_run_start(&spec, os_available).map_err(loop_bad_request)?;
        let prompt = loop_run_prompt_with_runtime(&spec, &workspace_text, runtime_mode);
        let summary = self.find_summary(&workspace_text, &spec.id)?;
        Ok(json!({
            "workspace": workspace_text,
            "loopId": spec.id,
            "display": format!("loop {}: {}", spec.id, truncate_chars(&spec.goal, 54)),
            "prompt": prompt,
            "runtimeMode": runtime_mode_label(runtime_mode),
            "osAvailable": os_available,
            "loop": loop_summary_json(&summary),
        }))
    }

    fn resolve_workspace(&self, workspace: Option<&str>) -> PathBuf {
        workspace
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(expand_home)
            .unwrap_or_else(|| self.state.default_workspace.clone())
    }

    fn find_summary(&self, workspace: &str, loop_id: &str) -> BootResult<LoopSummary> {
        let spec = find_loop(workspace, loop_id).map_err(loop_not_found)?;
        list_loops(workspace)
            .into_iter()
            .find(|summary| summary.spec.id == spec.id)
            .ok_or_else(|| BootError::NotFound(format!("loop `{loop_id}` not found")))
    }
}

fn init_arg(request: &LoopInitRequest) -> BootResult<String> {
    if let Some(arg) = request
        .arg
        .as_deref()
        .map(str::trim)
        .filter(|arg| !arg.is_empty())
    {
        return Ok(arg.to_string());
    }

    let name = request.name.as_deref().map(str::trim).unwrap_or_default();
    let pattern = request
        .pattern
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("daily-triage");

    if name.is_empty() || name == pattern {
        Ok(pattern.to_string())
    } else {
        Ok(format!("{name} {pattern}"))
    }
}

fn runtime_mode(request: &LoopRunPromptRequest) -> LoopRuntimeMode {
    match request.runtime_mode.as_deref().map(str::trim) {
        Some("os") | Some("osAvailable") => LoopRuntimeMode::OsAvailable,
        Some("localAgentDev") | Some("agent") => LoopRuntimeMode::LocalAgentDev,
        Some("local") | Some("localNoOs") => LoopRuntimeMode::LocalNoOs,
        _ if request.os_available.unwrap_or(false) => LoopRuntimeMode::OsAvailable,
        _ => LoopRuntimeMode::LocalNoOs,
    }
}

fn runtime_mode_label(mode: LoopRuntimeMode) -> &'static str {
    match mode {
        LoopRuntimeMode::OsAvailable => "osAvailable",
        LoopRuntimeMode::LocalNoOs => "localNoOs",
        LoopRuntimeMode::LocalAgentDev => "localAgentDev",
    }
}

fn loop_patterns_json() -> Vec<Value> {
    vec![
        json!({
            "id": "daily-triage",
            "label": "Daily triage",
            "cadence": "1d",
            "level": "L1",
            "description": "Inspect workspace state, recent activity, tests, TODOs, and risks.",
        }),
        json!({
            "id": "ci-sweeper",
            "label": "CI sweeper",
            "cadence": "15m",
            "level": "L1",
            "description": "Watch CI and test failures, isolate causes, and prepare small fixes.",
        }),
        json!({
            "id": "pr-babysitter",
            "label": "PR babysitter",
            "cadence": "15m",
            "level": "L1",
            "description": "Track active PR work, review comments, blockers, and follow-up actions.",
        }),
        json!({
            "id": "dependency-sweeper",
            "label": "Dependency sweeper",
            "cadence": "1d",
            "level": "L1",
            "description": "Review dependency drift and suggest low-risk upgrades with evidence.",
        }),
        json!({
            "id": "changelog-drafter",
            "label": "Changelog drafter",
            "cadence": "1d",
            "level": "L1",
            "description": "Draft a human-reviewable changelog from recent commits and release notes.",
        }),
    ]
}

fn loop_summary_json(summary: &LoopSummary) -> Value {
    json!({
        "spec": loop_spec_json(&summary.spec),
        "audit": loop_audit_json(&summary.audit),
        "lastRun": summary.last_run,
    })
}

fn loop_spec_json(spec: &LoopSpec) -> Value {
    json!({
        "id": spec.id,
        "pattern": spec.pattern,
        "goal": spec.goal,
        "level": spec.level,
        "cadence": spec.cadence,
        "osRuntime": spec.os_runtime,
        "worktree": spec.worktree,
        "makerAgent": spec.maker_agent,
        "checkerAgent": spec.checker_agent,
        "budgetTokensPerDay": spec.budget_tokens_per_day,
        "maxIterationsPerRun": spec.max_iterations_per_run,
        "denylist": spec.denylist,
        "connectors": spec.connectors,
        "dir": spec.dir.display().to_string(),
    })
}

fn loop_audit_json(audit: &LoopAudit) -> Value {
    json!({
        "score": audit.score,
        "level": audit.level,
        "passed": audit.passed,
        "missing": audit.missing,
        "warnings": audit.warnings,
    })
}

fn expand_home(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(rest) = path.strip_prefix("~/") {
        if let Some(home) = home_dir() {
            return home.join(rest);
        }
    }
    PathBuf::from(path)
}

fn home_dir() -> Option<PathBuf> {
    crate::user_paths::user_home_dir()
}

fn fs_error(error: std::io::Error) -> BootError {
    match error.kind() {
        std::io::ErrorKind::NotFound => BootError::NotFound(error.to_string()),
        std::io::ErrorKind::PermissionDenied => BootError::BadRequest(error.to_string()),
        _ => BootError::Internal(error.to_string()),
    }
}

fn loop_not_found(error: String) -> BootError {
    BootError::NotFound(error)
}

fn loop_bad_request(error: String) -> BootError {
    BootError::BadRequest(error)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    let keep = max_chars.saturating_sub(1);
    format!("{}...", value.chars().take(keep).collect::<String>())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::loop_engineering::{audit_loop, init_loop};

    fn temp_root(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "a3s-code-web-loop-{name}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn loop_summary_json_exposes_gui_fields() {
        let root = temp_root("summary");
        let cwd = root.to_string_lossy();
        let spec = init_loop(&cwd, "daily-triage").unwrap();
        let summary = LoopSummary {
            audit: audit_loop(&spec),
            spec,
            last_run: "never".to_string(),
        };

        let value = loop_summary_json(&summary);

        assert_eq!(value["spec"]["id"], "daily-triage");
        assert_eq!(value["spec"]["pattern"], "daily-triage");
        assert_eq!(value["spec"]["osRuntime"], true);
        assert!(value["audit"]["score"].as_u64().unwrap_or_default() >= 75);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn init_arg_composes_name_and_pattern_without_slash_command() {
        let request = LoopInitRequest {
            workspace: None,
            name: Some("nightly".to_string()),
            pattern: Some("ci-sweeper".to_string()),
            arg: None,
        };

        assert_eq!(init_arg(&request).unwrap(), "nightly ci-sweeper");
    }

    #[test]
    fn run_prompt_runtime_defaults_to_local_without_os() {
        let request = LoopRunPromptRequest::default();

        assert_eq!(runtime_mode(&request), LoopRuntimeMode::LocalNoOs);
    }
}
