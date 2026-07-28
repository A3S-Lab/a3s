use std::path::{Path, PathBuf};
use std::sync::Arc;

use a3s_code_core::hitl::{
    ConfirmationManager, ConfirmationPolicy, ConfirmationProvider, ConfirmationResponse,
    PendingConfirmationInfo, TimeoutAction,
};
use a3s_code_core::permissions::{
    InteractiveToolGuardrail, PermissionChecker, PermissionDecision, PermissionPolicy,
};

use crate::host_command_guardrail::{host_bash_decision, HostCommandMode};

const HITL_CONFIRM_TIMEOUT_MS: u64 = 60 * 60 * 1000;

const READ_ONLY_TOOLS: &[&str] = &[
    "Read(*)",
    "Grep(*)",
    "Glob(*)",
    "LS(*)",
    "web_search(*)",
    "web_fetch(*)",
    "code_symbols(*)",
    "code_navigation(*)",
    "code_diagnostics(*)",
    "search_skills(*)",
    "generate_object(*)",
];

const INTERACTIVE_TOOLS: &[&str] = &[
    "Write(*)",
    "Edit(*)",
    "Patch(*)",
    "Bash(*)",
    "Git(*)",
    "batch(*)",
    "program(*)",
    "task(*)",
    "parallel_task(*)",
    "dynamic_workflow(*)",
    "Skill(*)",
    "runtime(*)",
];

pub(in crate::api::code_web) fn permission_policy_for_mode(_mode: &str) -> PermissionPolicy {
    // Keep a serializable fallback for persistence and delegated child runs.
    // The shared structured checker installed by `permission_checker_for_mode`
    // is authoritative for host-side execution.
    PermissionPolicy::new()
        .deny_all(&[
            "Read(/**)",
            "Read(**/../**)",
            "Grep(* /**)",
            "Grep(* **/../**)",
            "Glob(/**)",
            "Glob(**/../**)",
            "LS(/**)",
            "LS(**/../**)",
            "Write(/**)",
            "Edit(/**)",
            "Write(**/../**)",
            "Edit(**/../**)",
        ])
        .allow("mcp__use_*")
        .allow_all(READ_ONLY_TOOLS)
        .ask_all(INTERACTIVE_TOOLS)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CodeWebExecutionMode {
    Default,
    Plan,
    Auto,
}

impl CodeWebExecutionMode {
    fn from_name(value: &str) -> Self {
        match value.trim().to_ascii_lowercase().as_str() {
            "plan" => Self::Plan,
            "auto" => Self::Auto,
            _ => Self::Default,
        }
    }
}

pub(in crate::api::code_web) struct CodeWebPermissionChecker {
    interactive: InteractiveToolGuardrail,
    mode: CodeWebExecutionMode,
    workspace: PathBuf,
}

impl std::ops::Deref for CodeWebPermissionChecker {
    type Target = InteractiveToolGuardrail;

    fn deref(&self) -> &Self::Target {
        &self.interactive
    }
}

impl PermissionChecker for CodeWebPermissionChecker {
    fn expose_to_model(&self, tool_name: &str) -> bool {
        let tool = tool_name.to_ascii_lowercase();
        if tool.starts_with("mcp__use_") {
            return false;
        }
        self.mode != CodeWebExecutionMode::Plan || plan_tool_is_read_only(&tool)
    }

    fn check(&self, tool_name: &str, args: &serde_json::Value) -> PermissionDecision {
        let tool = tool_name.to_ascii_lowercase();
        if tool.starts_with("mcp__use_") {
            // Keep the parent decision neutral for an explicitly scoped Use
            // worker. The primary Web model never receives this definition.
            return PermissionDecision::Allow;
        }

        if InteractiveToolGuardrail::default()
            .with_workspace(&self.workspace)
            .check(&tool, args)
            == PermissionDecision::Deny
        {
            return PermissionDecision::Deny;
        }

        let protected_workspace_metadata = targets_protected_workspace_metadata(&tool, args);
        match self.mode {
            CodeWebExecutionMode::Plan => {
                if plan_tool_is_read_only(&tool) {
                    PermissionDecision::Allow
                } else {
                    PermissionDecision::Deny
                }
            }
            CodeWebExecutionMode::Auto => {
                if protected_workspace_metadata {
                    return PermissionDecision::Deny;
                }
                match tool.as_str() {
                    "bash" => host_bash_decision(&self.interactive, HostCommandMode::Auto, args),
                    "git" => match InteractiveToolGuardrail::risk_decision(&tool, args) {
                        PermissionDecision::Allow => PermissionDecision::Allow,
                        PermissionDecision::Ask | PermissionDecision::Deny => {
                            PermissionDecision::Deny
                        }
                    },
                    _ if auto_tool_stays_inside_governed_boundaries(&tool) => {
                        PermissionDecision::Allow
                    }
                    _ => PermissionDecision::Deny,
                }
            }
            CodeWebExecutionMode::Default => match tool.as_str() {
                "write" | "edit" | "patch" => {
                    if protected_workspace_metadata {
                        PermissionDecision::Ask
                    } else {
                        PermissionDecision::Allow
                    }
                }
                "bash" => host_bash_decision(&self.interactive, HostCommandMode::Default, args),
                "batch" | "program" | "task" | "parallel_task" | "dynamic_workflow" | "skill" => {
                    PermissionDecision::Allow
                }
                name if name.starts_with("mcp__") => PermissionDecision::Allow,
                _ => InteractiveToolGuardrail::risk_decision(&tool, args),
            },
        }
    }
}

pub(in crate::api::code_web) fn permission_checker_for_mode(
    mode: &str,
    workspace: &Path,
) -> CodeWebPermissionChecker {
    let execution_mode = CodeWebExecutionMode::from_name(mode);
    CodeWebPermissionChecker {
        interactive: InteractiveToolGuardrail::for_mode(mode).with_workspace(workspace),
        mode: execution_mode,
        workspace: workspace.to_path_buf(),
    }
}

pub(in crate::api::code_web) fn confirmation_policy_for_mode(_mode: &str) -> ConfirmationPolicy {
    ConfirmationPolicy::enabled().with_timeout(HITL_CONFIRM_TIMEOUT_MS, TimeoutAction::Reject)
}

pub(in crate::api::code_web) struct CodeWebModeConfirmationProvider {
    inner: Arc<ConfirmationManager>,
    mode: CodeWebExecutionMode,
}

impl CodeWebModeConfirmationProvider {
    pub(in crate::api::code_web) fn new(mode: &str, policy: ConfirmationPolicy) -> Self {
        let (event_tx, _) = tokio::sync::broadcast::channel(16);
        let timeout_ms = policy.default_timeout_ms;
        let policy = policy.with_timeout(timeout_ms, TimeoutAction::Reject);
        Self {
            inner: Arc::new(ConfirmationManager::new(policy, event_tx)),
            mode: CodeWebExecutionMode::from_name(mode),
        }
    }

    fn auto_response() -> tokio::sync::oneshot::Receiver<ConfirmationResponse> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let _ = tx.send(ConfirmationResponse {
            approved: false,
            reason: Some("Denied by the non-interactive Auto execution boundary.".to_string()),
        });
        rx
    }
}

#[async_trait::async_trait]
impl ConfirmationProvider for CodeWebModeConfirmationProvider {
    fn snapshot_for_run(&self) -> Option<Arc<dyn ConfirmationProvider>> {
        Some(Arc::new(Self {
            inner: Arc::clone(&self.inner),
            mode: self.mode,
        }))
    }

    async fn requires_confirmation(&self, tool_name: &str) -> bool {
        self.inner.requires_confirmation(tool_name).await
    }

    async fn confirmation_available_for(
        &self,
        _tool_name: &str,
        _args: &serde_json::Value,
    ) -> bool {
        self.mode != CodeWebExecutionMode::Auto
    }

    async fn request_confirmation(
        &self,
        tool_id: &str,
        tool_name: &str,
        args: &serde_json::Value,
    ) -> tokio::sync::oneshot::Receiver<ConfirmationResponse> {
        if self.mode == CodeWebExecutionMode::Auto {
            Self::auto_response()
        } else {
            self.inner
                .request_confirmation(tool_id, tool_name, args)
                .await
        }
    }

    async fn confirm(
        &self,
        tool_id: &str,
        approved: bool,
        reason: Option<String>,
    ) -> Result<bool, String> {
        self.inner.confirm(tool_id, approved, reason).await
    }

    async fn policy(&self) -> ConfirmationPolicy {
        self.inner.policy().await
    }

    async fn set_policy(&self, policy: ConfirmationPolicy) {
        let timeout_ms = policy.default_timeout_ms;
        self.inner
            .set_policy(policy.with_timeout(timeout_ms, TimeoutAction::Reject))
            .await;
    }

    async fn check_timeouts(&self) -> usize {
        self.inner.check_timeouts().await
    }

    async fn cancel(&self, tool_id: &str) -> bool {
        self.inner.cancel(tool_id).await
    }

    async fn expire(&self, tool_id: &str, _action: TimeoutAction) -> bool {
        self.inner.expire(tool_id, TimeoutAction::Reject).await
    }

    async fn cancel_all(&self) -> usize {
        self.inner.cancel_all().await
    }

    async fn pending_confirmations(&self) -> Vec<PendingConfirmationInfo> {
        self.inner.pending_confirmation_details().await
    }
}

fn targets_protected_workspace_metadata(tool_name: &str, args: &serde_json::Value) -> bool {
    matches!(tool_name, "write" | "edit" | "patch")
        && args
            .get("file_path")
            .and_then(serde_json::Value::as_str)
            .is_some_and(a3s_code_core::sandbox::is_protected_workspace_path)
}

fn plan_tool_is_read_only(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read" | "grep" | "glob" | "ls" | "web_search" | "web_fetch"
    )
}

fn auto_tool_stays_inside_governed_boundaries(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "read"
            | "grep"
            | "glob"
            | "ls"
            | "code_symbols"
            | "code_navigation"
            | "code_diagnostics"
            | "web_search"
            | "web_fetch"
            | "generate_object"
            | "search_skills"
            | "write"
            | "edit"
            | "patch"
            | "batch"
            | "program"
            | "task"
            | "parallel_task"
            | "dynamic_workflow"
            | "skill"
    ) || tool_name.starts_with("mcp__")
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::permissions::{
        PermissionChecker, PermissionDecision, ToolRiskAction, ToolRiskLevel,
    };
    use serde_json::json;

    fn checker(mode: &str) -> CodeWebPermissionChecker {
        permission_checker_for_mode(mode, Path::new("."))
    }

    #[test]
    fn default_mode_uses_the_rust_guardrail_for_host_bash() {
        let checker = checker("default");
        assert_eq!(
            checker.check("read", &json!({ "file_path": "src/main.rs" })),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "pwd" })),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "cargo test" })),
            PermissionDecision::Ask
        );
        assert_eq!(
            checker.check(
                "bash",
                &json!({
                    "command": "pwd",
                    "sandbox_permissions": "require_escalated",
                    "justification": "needs host access"
                })
            ),
            PermissionDecision::Ask
        );
        assert_eq!(
            checker.check("git", &json!({ "command": "status" })),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("write", &json!({ "file_path": "src/main.rs" })),
            PermissionDecision::Allow
        );
        assert!(confirmation_policy_for_mode("default").enabled);
    }

    #[test]
    fn plan_mode_is_a_non_escalatable_read_only_boundary() {
        let checker = checker("plan");
        assert_eq!(
            checker.check("grep", &json!({ "pattern": "TODO" })),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "pwd" })),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check("write", &json!({ "file_path": "src/main.rs" })),
            PermissionDecision::Deny
        );
        assert!(!checker.expose_to_model("bash"));
        assert!(!checker.expose_to_model("write"));
        assert!(checker.expose_to_model("read"));
        assert!(confirmation_policy_for_mode("plan").enabled);
    }

    #[test]
    fn auto_mode_allows_only_rust_proven_read_only_host_bash() {
        let checker = checker("auto");
        assert_eq!(
            checker.check("write", &json!({ "file_path": "src/main.rs" })),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "cargo test" })),
            PermissionDecision::Deny
        );
        for command in ["pwd", "rg mkfs README.md", "cat docs/mkfs-guide.md"] {
            assert_eq!(
                checker.check("bash", &json!({ "command": command })),
                PermissionDecision::Allow,
                "Auto should allow Rust-proven read-only Bash: {command}"
            );
        }
        assert_eq!(
            checker.check(
                "bash",
                &json!({
                    "command": "cargo test",
                    "sandbox_permissions": "require_escalated",
                    "justification": "needs host access"
                })
            ),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check("runtime", &json!({ "tasks": ["external work"] })),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check(
                "git",
                &json!({ "command": "checkout", "ref": "feature", "force": true })
            ),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check("git", &json!({ "command": "unknown" })),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "cat *" })),
            PermissionDecision::Deny,
            "Auto must deny shell syntax that the Rust guardrail cannot prove read-only"
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "mkfs /dev/disk9" })),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check("bash", &json!({ "command": "rm -rf /" })),
            PermissionDecision::Deny
        );
        assert!(
            confirmation_policy_for_mode("auto").enabled,
            "the provider keeps a serializable fail-closed policy"
        );
    }

    #[test]
    fn web_auto_keeps_explainable_risk_for_guarded_host_bash() {
        let checker = checker("auto");
        for (tool, args, level, action, permission) in [
            (
                "read",
                json!({"file_path": "src/main.rs"}),
                ToolRiskLevel::Routine,
                ToolRiskAction::Allow,
                PermissionDecision::Allow,
            ),
            (
                "write",
                json!({"file_path": "src/main.rs"}),
                ToolRiskLevel::Bounded,
                ToolRiskAction::Allow,
                PermissionDecision::Allow,
            ),
            (
                "bash",
                json!({"command": "cargo test"}),
                ToolRiskLevel::High,
                ToolRiskAction::ReviewByLlm,
                PermissionDecision::Deny,
            ),
            (
                "bash",
                json!({"command": "rm -rf /"}),
                ToolRiskLevel::Critical,
                ToolRiskAction::RuleDeny,
                PermissionDecision::Deny,
            ),
        ] {
            assert_eq!(checker.assess(tool, &args).level, level);
            assert_eq!(checker.risk_action(tool, &args), action);
            assert_eq!(checker.check(tool, &args), permission);
        }

        assert!(confirmation_policy_for_mode("auto").enabled);
    }

    #[tokio::test]
    async fn auto_confirmation_provider_rejects_unexpected_escalation_without_a_prompt() {
        let provider =
            CodeWebModeConfirmationProvider::new("auto", confirmation_policy_for_mode("auto"));
        assert!(
            !provider
                .confirmation_available_for("runtime", &json!({}))
                .await
        );

        let response = provider
            .request_confirmation("tool-1", "runtime", &json!({}))
            .await
            .await
            .expect("auto rejection response");
        assert!(!response.approved);
        assert!(provider.pending_confirmations().await.is_empty());
    }

    #[test]
    fn web_policy_persists_a_conservative_fallback_in_every_mode() {
        for mode in ["default", "plan", "auto"] {
            let policy = permission_policy_for_mode(mode);
            assert!(policy.enabled, "{mode} must not disable the policy");
            assert_eq!(
                policy.check("read", &json!({ "file_path": "src/main.rs" })),
                PermissionDecision::Allow
            );
            assert_eq!(
                policy.check("bash", &json!({ "command": "rm -rf /" })),
                PermissionDecision::Ask,
                "the serializable fallback must not silently allow side effects"
            );
            assert_eq!(
                policy.check("mcp__use_browser__agent_browser_open", &json!({})),
                PermissionDecision::Allow,
                "the serializable parent policy must not erase the explicit Use worker policy"
            );
        }
    }

    #[test]
    fn primary_web_model_hides_raw_use_tools_without_blocking_the_worker() {
        let checker = checker("default");
        let tool = "mcp__use_office__office_list";

        assert!(!checker.expose_to_model(tool));
        assert_eq!(checker.check(tool, &json!({})), PermissionDecision::Allow);
    }
}
