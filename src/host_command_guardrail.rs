//! Shared host Bash policy for Code entry points.
//!
//! The Rust risk classifier proves a deliberately small read-only shell subset.
//! This adapter maps that assessment onto interactive execution modes without
//! treating a lexical guardrail as an operating-system isolation boundary.

use a3s_code_core::permissions::{InteractiveToolGuardrail, PermissionChecker, PermissionDecision};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HostCommandMode {
    Default,
    Plan,
    Auto,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HostBoundaryRequest {
    UseDefault,
    RequireEscalated,
    Invalid,
}

/// Decide whether one Bash invocation may use the host workspace runner.
///
/// `sandbox_permissions` remains part of the Bash tool's compatibility schema,
/// but A3S does not attach a local process sandbox. `require_escalated` therefore
/// means an explicit host-boundary request: Default asks and non-interactive
/// modes deny it.
pub(crate) fn host_bash_decision(
    guardrail: &InteractiveToolGuardrail,
    mode: HostCommandMode,
    args: &serde_json::Value,
) -> PermissionDecision {
    if references_protected_path(args) {
        return PermissionDecision::Deny;
    }

    let mut risk = guardrail.check("bash", args);
    if risk == PermissionDecision::Allow && !has_safe_unattended_read_semantics(args) {
        risk = PermissionDecision::Ask;
    }
    if risk == PermissionDecision::Deny {
        return PermissionDecision::Deny;
    }

    match (host_boundary_request(args), mode, risk) {
        (HostBoundaryRequest::Invalid, _, _) => PermissionDecision::Deny,
        (HostBoundaryRequest::RequireEscalated, HostCommandMode::Default, _) => {
            PermissionDecision::Ask
        }
        (HostBoundaryRequest::RequireEscalated, _, _) => PermissionDecision::Deny,
        (HostBoundaryRequest::UseDefault, HostCommandMode::Plan, _) => PermissionDecision::Deny,
        (HostBoundaryRequest::UseDefault, HostCommandMode::Auto, PermissionDecision::Allow) => {
            PermissionDecision::Allow
        }
        (HostBoundaryRequest::UseDefault, HostCommandMode::Auto, _) => PermissionDecision::Deny,
        (HostBoundaryRequest::UseDefault, HostCommandMode::Default, decision) => decision,
    }
}

fn references_protected_path(args: &serde_json::Value) -> bool {
    args.get("command")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|command| {
            command
                .split_whitespace()
                .map(clean_shell_token)
                .any(is_protected_path_token)
        })
}

fn is_protected_path_token(token: &str) -> bool {
    let normalized = token.replace('\\', "/");
    if is_normalized_protected_path_token(&normalized) {
        return true;
    }

    // Bash can hide a protected name with escapes (`.e\\nv`) or empty
    // quoting (`.e''nv`). Inspect a conservative de-obfuscated spelling as
    // well as the original token. False positives fail closed; this path is a
    // non-bypassable credential/control-plane floor, not the quiet allow-list.
    let deobfuscated = token
        .chars()
        .filter(|character| !matches!(character, '\\' | '\'' | '"' | '$' | '{' | '}'))
        .collect::<String>();
    deobfuscated != token && is_normalized_protected_path_token(&deobfuscated.replace('\\', "/"))
}

fn is_normalized_protected_path_token(normalized: &str) -> bool {
    let normalized = normalized.trim_start_matches("./");
    if normalized.is_empty() {
        return false;
    }
    if let Some((_, value)) = normalized.split_once('=') {
        if is_protected_path_token(value) {
            return true;
        }
    }
    if a3s_code_core::sandbox::is_protected_workspace_path(normalized) {
        return true;
    }

    normalized.split('/').any(|component| {
        let component = component.to_ascii_lowercase();
        component == ".env"
            || component.starts_with(".env.")
            || component.starts_with(".env-")
            || matches!(
                component.as_str(),
                ".ssh"
                    | ".gnupg"
                    | ".git-credentials"
                    | ".netrc"
                    | ".npmrc"
                    | ".pypirc"
                    | ".credentials.json"
                    | "credentials.toml"
                    | "id_rsa"
                    | "id_ed25519"
            )
    })
}

fn clean_shell_token(token: &str) -> &str {
    token.trim_matches(|character: char| {
        matches!(
            character,
            '\'' | '"' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ':'
        )
    })
}

fn has_safe_unattended_read_semantics(args: &serde_json::Value) -> bool {
    args.get("command")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|command| {
            // Backslashes are shell escapes on Unix and path separators on
            // Windows. Leave either spelling behind HITL until the policy is
            // backed by a platform-aware parser.
            !command.contains('\\') && command.split('|').all(safe_unattended_segment)
        })
}

fn safe_unattended_segment(segment: &str) -> bool {
    let tokens = segment.split_whitespace().collect::<Vec<_>>();
    let Some(command) = tokens.first().copied() else {
        return false;
    };
    match command {
        "rg" => !tokens.iter().skip(1).any(|value| {
            matches!(
                *value,
                "--hidden"
                    | "--no-ignore"
                    | "--no-ignore-dot"
                    | "--no-ignore-files"
                    | "--no-ignore-global"
                    | "--no-ignore-parent"
                    | "--no-ignore-vcs"
                    | "-u"
                    | "-uu"
                    | "-uuu"
            )
        }),
        "git" => safe_unattended_git(&tokens),
        _ => true,
    }
}

fn safe_unattended_git(tokens: &[&str]) -> bool {
    let mut index = 1;
    while matches!(
        tokens.get(index).copied(),
        Some("--no-pager" | "-P" | "--no-optional-locks")
    ) {
        index += 1;
    }
    let Some(subcommand) = tokens.get(index).copied() else {
        return false;
    };
    let args = &tokens[index + 1..];
    match subcommand {
        "status" | "branch" | "rev-parse" | "ls-files" => true,
        "log" => !args
            .iter()
            .any(|value| matches!(*value, "-p" | "--patch" | "--raw")),
        "diff" => {
            let metadata_only = args.iter().any(|value| {
                matches!(
                    *value,
                    "--name-only" | "--name-status" | "--stat" | "--numstat" | "--shortstat"
                )
            });
            let explicit_paths = args
                .iter()
                .position(|value| *value == "--")
                .is_some_and(|separator| separator + 1 < args.len());
            metadata_only || explicit_paths
        }
        "remote" => args.is_empty(),
        "blame" => args.iter().any(|value| !value.starts_with('-')),
        "show" | "grep" => false,
        _ => false,
    }
}

fn host_boundary_request(args: &serde_json::Value) -> HostBoundaryRequest {
    match args.get("sandbox_permissions") {
        None => HostBoundaryRequest::UseDefault,
        Some(serde_json::Value::String(value)) if value == "use_default" => {
            HostBoundaryRequest::UseDefault
        }
        Some(serde_json::Value::String(value)) if value == "require_escalated" => {
            HostBoundaryRequest::RequireEscalated
        }
        Some(_) => HostBoundaryRequest::Invalid,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn guardrail() -> InteractiveToolGuardrail {
        InteractiveToolGuardrail::default().with_workspace(".")
    }

    #[test]
    fn default_allows_only_rust_proven_read_only_commands_without_hitl() {
        let guardrail = guardrail();
        for command in [
            "pwd",
            "ls -la",
            "rg Permission src | head -20",
            "git --no-pager diff -- README.md",
        ] {
            assert_eq!(
                host_bash_decision(
                    &guardrail,
                    HostCommandMode::Default,
                    &json!({"command": command}),
                ),
                PermissionDecision::Allow,
                "read-only host command should be quiet: {command}"
            );
        }

        for command in [
            "cargo test",
            "printf result > output.txt",
            "cat *",
            "echo $(date)",
            "git -C .. status",
            "git diff",
            "git diff --check",
            "git remote -v",
            "rg --hidden TOKEN .",
            "cat docs\\guide.md",
        ] {
            assert_eq!(
                host_bash_decision(
                    &guardrail,
                    HostCommandMode::Default,
                    &json!({"command": command}),
                ),
                PermissionDecision::Ask,
                "unproven host command must retain HITL: {command}"
            );
        }
    }

    #[test]
    fn non_interactive_modes_fail_closed_outside_the_proven_subset() {
        let guardrail = guardrail();
        for mode in [HostCommandMode::Plan, HostCommandMode::Auto] {
            assert_eq!(
                host_bash_decision(&guardrail, mode, &json!({"command": "cargo test"}),),
                PermissionDecision::Deny
            );
        }
        assert_eq!(
            host_bash_decision(
                &guardrail,
                HostCommandMode::Plan,
                &json!({"command": "pwd"}),
            ),
            PermissionDecision::Deny
        );
        assert_eq!(
            host_bash_decision(
                &guardrail,
                HostCommandMode::Auto,
                &json!({"command": "pwd"}),
            ),
            PermissionDecision::Allow
        );
    }

    #[test]
    fn credential_and_control_paths_cannot_bypass_workspace_file_policy() {
        let guardrail = guardrail();
        for command in [
            "cat .env",
            "head services/api/.env.production",
            "cat .git/config",
            "cat .a3s/config.acl",
            "rg --file=.env TOKEN .",
            "cat ~/.ssh/id_ed25519",
            "cat .e\\nv",
            "cat .e''nv",
            "cat $'.env'",
        ] {
            assert_eq!(
                host_bash_decision(
                    &guardrail,
                    HostCommandMode::Default,
                    &json!({"command": command}),
                ),
                PermissionDecision::Deny,
                "credential and control paths must not be readable through Bash: {command}"
            );
        }
    }

    #[test]
    fn explicit_escalation_invalid_metadata_and_catastrophic_commands_fail_closed() {
        let guardrail = guardrail();
        assert_eq!(
            host_bash_decision(
                &guardrail,
                HostCommandMode::Default,
                &json!({
                    "command": "pwd",
                    "sandbox_permissions": "require_escalated",
                    "justification": "needs a host capability"
                }),
            ),
            PermissionDecision::Ask
        );
        for mode in [HostCommandMode::Plan, HostCommandMode::Auto] {
            assert_eq!(
                host_bash_decision(
                    &guardrail,
                    mode,
                    &json!({
                        "command": "pwd",
                        "sandbox_permissions": "require_escalated"
                    }),
                ),
                PermissionDecision::Deny
            );
        }
        assert_eq!(
            host_bash_decision(
                &guardrail,
                HostCommandMode::Default,
                &json!({"command": "pwd", "sandbox_permissions": 7}),
            ),
            PermissionDecision::Deny
        );
        assert_eq!(
            host_bash_decision(
                &guardrail,
                HostCommandMode::Default,
                &json!({"command": "rm -rf /"}),
            ),
            PermissionDecision::Deny
        );
    }

    #[cfg(unix)]
    #[test]
    fn existing_workspace_symlinks_are_never_silently_followed() {
        use std::os::unix::fs::symlink;

        let workspace = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        symlink(outside.path(), workspace.path().join("escape")).unwrap();
        let guardrail = InteractiveToolGuardrail::default().with_workspace(workspace.path());

        for mode in [HostCommandMode::Default, HostCommandMode::Auto] {
            assert_eq!(
                host_bash_decision(
                    &guardrail,
                    mode,
                    &json!({"command": "cat escape/secret.txt"}),
                ),
                PermissionDecision::Deny
            );
        }
    }
}
