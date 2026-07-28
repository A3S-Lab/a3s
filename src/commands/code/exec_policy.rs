use std::path::Path;
use std::sync::Arc;

use a3s_code_core::hitl::{ConfirmationPolicy, TimeoutAction};
use a3s_code_core::permissions::{
    InteractiveToolGuardrail, PermissionChecker, PermissionDecision, PermissionPolicy,
};
use a3s_code_core::{PlanningMode, SessionOptions};

use crate::cli::args::CodeMode;
use crate::host_command_guardrail::{host_bash_decision, HostCommandMode};

struct ExecPermissionChecker {
    interactive: InteractiveToolGuardrail,
    host_mode: HostCommandMode,
}

impl PermissionChecker for ExecPermissionChecker {
    fn expose_to_model(&self, tool_name: &str) -> bool {
        !(self.host_mode == HostCommandMode::Plan && tool_name.eq_ignore_ascii_case("bash"))
            && self.interactive.expose_to_model(tool_name)
    }

    fn check(&self, tool_name: &str, args: &serde_json::Value) -> PermissionDecision {
        if tool_name.eq_ignore_ascii_case("bash") {
            host_bash_decision(&self.interactive, self.host_mode, args)
        } else {
            self.interactive.check(tool_name, args)
        }
    }
}

pub(super) fn session_options(
    mode: CodeMode,
    workspace: &Path,
    session_id: &str,
) -> SessionOptions {
    let permission_policy = permission_policy();
    SessionOptions::new()
        .with_session_id(session_id)
        .with_planning_mode(planning_mode(mode))
        .with_confirmation_policy(
            ConfirmationPolicy::enabled().with_timeout(30_000, TimeoutAction::Reject),
        )
        .with_permission_policy(permission_policy)
        .with_permission_checker(Arc::new(ExecPermissionChecker {
            interactive: InteractiveToolGuardrail::for_mode(mode_name(mode))
                .with_workspace(workspace),
            host_mode: host_mode(mode),
        }))
}

fn planning_mode(mode: CodeMode) -> PlanningMode {
    match mode {
        CodeMode::Plan => PlanningMode::Enabled,
        CodeMode::Default => PlanningMode::Disabled,
        CodeMode::Auto => PlanningMode::Auto,
    }
}

fn mode_name(mode: CodeMode) -> &'static str {
    match mode {
        CodeMode::Plan => "plan",
        CodeMode::Default => "default",
        CodeMode::Auto => "auto",
    }
}

fn host_mode(mode: CodeMode) -> HostCommandMode {
    match mode {
        CodeMode::Default => HostCommandMode::Default,
        CodeMode::Plan => HostCommandMode::Plan,
        CodeMode::Auto => HostCommandMode::Auto,
    }
}

fn permission_policy() -> PermissionPolicy {
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
        .allow_all(&[
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
        ])
        .ask_all(&[
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
        ])
}

#[cfg(test)]
mod tests {
    use a3s_code_core::permissions::PermissionDecision;
    use a3s_code_core::PlanningMode;
    use serde_json::json;

    use super::*;

    #[test]
    fn auto_mode_allows_bounded_edits_but_preserves_the_safety_floor() {
        let workspace = tempfile::tempdir().unwrap();
        let options = session_options(CodeMode::Auto, workspace.path(), "exec-test");
        let checker = options
            .permission_checker
            .as_ref()
            .expect("exec must install a permission checker");

        assert_eq!(options.planning_mode, PlanningMode::Auto);
        assert!(
            options
                .confirmation_policy
                .as_ref()
                .expect("exec must install a confirmation manager policy")
                .enabled
        );
        assert_eq!(
            checker.check("write", &json!({"file_path": "answer.txt"})),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("bash", &json!({"command": "pwd"})),
            PermissionDecision::Allow
        );
        assert_eq!(
            checker.check("bash", &json!({"command": "cargo test"})),
            PermissionDecision::Deny
        );
        assert_eq!(
            checker.check("bash", &json!({"command": "rm -rf /"})),
            PermissionDecision::Deny
        );
    }

    #[test]
    fn default_and_plan_modes_preserve_their_interactive_boundaries() {
        let workspace = tempfile::tempdir().unwrap();
        for (mode, planning) in [
            (CodeMode::Default, PlanningMode::Disabled),
            (CodeMode::Plan, PlanningMode::Enabled),
        ] {
            let options = session_options(mode, workspace.path(), "exec-test");
            let checker = options
                .permission_checker
                .as_ref()
                .expect("exec must install a permission checker");

            assert_eq!(options.planning_mode, planning);
            assert_eq!(
                checker.check("write", &json!({"file_path": "answer.txt"})),
                PermissionDecision::Ask
            );
            let expected_bash = match mode {
                CodeMode::Default => PermissionDecision::Allow,
                CodeMode::Plan => PermissionDecision::Deny,
                CodeMode::Auto => unreachable!(),
            };
            assert_eq!(
                checker.check("bash", &json!({"command": "pwd"})),
                expected_bash
            );
        }
    }
}
