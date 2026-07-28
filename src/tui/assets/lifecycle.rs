//! Shared AI-native lifecycle metadata for A3S Code team assets.
//!
//! The TUI has different slash commands for different asset families. The
//! product contract is intentionally shared, while each command surface only
//! exposes actions that are real for that asset family.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LifecycleStage {
    Create,
    Develop,
    Test,
    Run,
    Publish,
    Deploy,
    Inspect,
    Activity,
    Observe,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub(crate) enum OsService {
    AgentAsAService,
    FunctionAsAService,
    WorkflowAsAService,
    KnowledgeService,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeBindingIntent {
    pub(crate) kind: &'static str,
    pub(crate) isolation: &'static str,
    pub(crate) runtime_kind: &'static str,
    pub(crate) protocol: Option<&'static str>,
    pub(crate) agent_kind: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AssetLifecycle {
    pub(crate) family: &'static str,
    pub(crate) command: &'static str,
    pub(crate) os_category: &'static str,
    pub(crate) service: OsService,
    pub(crate) runtime_binding: RuntimeBindingIntent,
    pub(crate) stages: &'static [LifecycleStage],
}

pub(crate) const ASSET_ACL_PATH: &str = ".a3s/asset.acl";

pub(crate) struct AssetAclDocument<'a> {
    pub(crate) category: &'a str,
    pub(crate) kind: Option<&'a str>,
    pub(crate) name: &'a str,
    pub(crate) description: &'a str,
    pub(crate) local_path: Option<&'a str>,
    pub(crate) service: OsService,
    pub(crate) runtime: RuntimeBindingIntent,
    pub(crate) source: &'a [(&'a str, &'a str)],
    pub(crate) metadata: &'a [(&'a str, &'a str)],
}

pub(crate) fn render_asset_acl(doc: AssetAclDocument<'_>) -> String {
    let mut out = String::new();
    out.push_str("version = \"a3s.asset.v1\"\n");
    out.push_str(&format!("category = {}\n", acl_string(doc.category)));
    if let Some(kind) = doc.kind {
        out.push_str(&format!("kind = {}\n", acl_string(kind)));
    }
    out.push_str(&format!("name = {}\n", acl_string(doc.name)));
    out.push_str(&format!("description = {}\n", acl_string(doc.description)));
    if let Some(local_path) = doc.local_path {
        out.push_str(&format!("local_path = {}\n", acl_string(local_path)));
    }
    out.push_str(&format!(
        "service = {}\n",
        acl_string(service_label(doc.service))
    ));
    out.push_str("created_by = \"a3s-code-tui\"\n\n");

    out.push_str("source {\n");
    for (key, value) in doc.source {
        out.push_str(&format!("  {} = {}\n", acl_key(key), acl_string(value)));
    }
    out.push_str("}\n\n");

    out.push_str("metadata {\n");
    out.push_str(&format!(
        "  asset_acl_path = {}\n",
        acl_string(ASSET_ACL_PATH)
    ));
    for (key, value) in doc.metadata {
        out.push_str(&format!("  {} = {}\n", acl_key(key), acl_string(value)));
    }
    out.push_str("}\n\n");

    out.push_str("runtime {\n");
    out.push_str(&format!("  kind = {}\n", acl_string(doc.runtime.kind)));
    out.push_str(&format!(
        "  isolation = {}\n",
        acl_string(doc.runtime.isolation)
    ));
    out.push_str(&format!(
        "  runtime_kind = {}\n",
        acl_string(doc.runtime.runtime_kind)
    ));
    if let Some(protocol) = doc.runtime.protocol {
        out.push_str(&format!("  protocol = {}\n", acl_string(protocol)));
    }
    if let Some(agent_kind) = doc.runtime.agent_kind {
        out.push_str(&format!("  agent_kind = {}\n", acl_string(agent_kind)));
    }
    out.push_str("}\n");
    out
}

pub(crate) fn write_asset_acl(
    asset_root_or_file: &std::path::Path,
    content: &str,
) -> Result<(), String> {
    let asset_root = if asset_root_or_file.is_file() {
        asset_root_or_file
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
    } else {
        asset_root_or_file
    };
    let path = asset_root.join(ASSET_ACL_PATH);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, content).map_err(|e| format!("could not write {}: {e}", path.display()))
}

pub(crate) fn scaffold_name(description: &str, fallback: &str) -> String {
    let lower = description.to_ascii_lowercase();
    if let Some(start) = lower.find("name it exactly") {
        let after = &description[start + "name it exactly".len()..];
        let raw = after
            .trim()
            .trim_start_matches([':', '=', '"', '\'', '`', ' '])
            .split(['.', '\n', ';'])
            .next()
            .unwrap_or("")
            .trim()
            .trim_matches(['"', '\'', '`']);
        let slug = crate::commands::code::naming::asset_slug(raw);
        if slug != "asset" {
            return truncate_slug(slug);
        }
    }

    let words = description
        .split_whitespace()
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    let slug = crate::commands::code::naming::asset_slug(if words.trim().is_empty() {
        fallback
    } else {
        &words
    });
    truncate_slug(slug)
}

pub(crate) fn scaffold_description(description: &str, name: &str, fallback: &str) -> String {
    let mut text = description.trim();
    let lower = description.to_ascii_lowercase();
    if let Some(start) = lower.find("name it exactly") {
        let after = &description[start + "name it exactly".len()..];
        if let Some(dot) = after.find('.') {
            text = after[dot + 1..].trim();
        }
    }
    let text = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .collect::<Vec<_>>()
        .join(" ");
    let text = text.trim().trim_end_matches('.');
    if text.is_empty() {
        format!("{fallback} for {name}")
    } else {
        text.replace('"', "'").chars().take(180).collect()
    }
}

pub(crate) fn unique_asset_dir(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let base = if name.trim().is_empty() {
        "asset"
    } else {
        name
    };
    let first = root.join(base);
    if !first.exists() {
        return first;
    }
    for suffix in 2.. {
        let candidate = root.join(format!("{base}-{suffix}"));
        if !candidate.exists() {
            return candidate;
        }
    }
    unreachable!("unbounded suffix search should always find a free path")
}

pub(crate) fn normalized_rel(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

pub(crate) fn write_scaffold_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("could not write {}: {e}", path.display()))
}

pub(crate) fn write_scaffold_json(
    path: impl AsRef<std::path::Path>,
    value: &serde_json::Value,
) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|e| e.to_string())?;
    write_scaffold_file(path.as_ref(), &bytes)
}

fn truncate_slug(slug: String) -> String {
    const MAX_LEN: usize = 48;
    if slug.chars().count() <= MAX_LEN {
        return slug;
    }
    let mut out = slug.chars().take(MAX_LEN).collect::<String>();
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "asset".to_string()
    } else {
        out
    }
}

fn acl_key(key: &str) -> String {
    key.chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric() || ch == '_' {
                ch
            } else {
                '_'
            }
        })
        .collect()
}

fn acl_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

pub(crate) const MCP_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Run,
    LifecycleStage::Test,
    LifecycleStage::Publish,
    LifecycleStage::Deploy,
    LifecycleStage::Observe,
];

pub(crate) const AGENTIC_AGENT_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Run,
    LifecycleStage::Publish,
    LifecycleStage::Observe,
];

pub(crate) const APPLICATION_AGENT_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Publish,
    LifecycleStage::Deploy,
    LifecycleStage::Observe,
];

pub(crate) const TOOL_AGENT_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Publish,
    LifecycleStage::Activity,
    LifecycleStage::Observe,
];

pub(crate) const SKILL_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Publish,
    LifecycleStage::Deploy,
    LifecycleStage::Inspect,
    LifecycleStage::Activity,
];

pub(crate) const OKF_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Publish,
    LifecycleStage::Deploy,
    LifecycleStage::Inspect,
    LifecycleStage::Activity,
];

pub(crate) const WORKFLOW_STAGES: &[LifecycleStage] = &[
    LifecycleStage::Create,
    LifecycleStage::Develop,
    LifecycleStage::Publish,
    LifecycleStage::Run,
    LifecycleStage::Deploy,
    LifecycleStage::Activity,
    LifecycleStage::Observe,
];

pub(crate) const ASSET_LIFECYCLES: &[AssetLifecycle] = &[
    AssetLifecycle {
        family: "agentic agent",
        command: "/agent",
        os_category: "agent",
        service: OsService::AgentAsAService,
        runtime_binding: RuntimeBindingIntent {
            kind: "agent",
            isolation: "serving",
            runtime_kind: "a3s-agent-service",
            protocol: None,
            agent_kind: Some("agentic"),
        },
        stages: AGENTIC_AGENT_STAGES,
    },
    AssetLifecycle {
        family: "application agent",
        command: "/agent",
        os_category: "agent",
        service: OsService::AgentAsAService,
        runtime_binding: RuntimeBindingIntent {
            kind: "agent",
            isolation: "container",
            runtime_kind: "a3s-agent-service",
            protocol: None,
            agent_kind: Some("application"),
        },
        stages: APPLICATION_AGENT_STAGES,
    },
    AssetLifecycle {
        family: "tool agent",
        command: "/agent",
        os_category: "agent",
        service: OsService::FunctionAsAService,
        runtime_binding: RuntimeBindingIntent {
            kind: "tool",
            isolation: "serving",
            runtime_kind: "a3s-function-service",
            protocol: Some("agent-tool"),
            agent_kind: Some("tool"),
        },
        stages: TOOL_AGENT_STAGES,
    },
    AssetLifecycle {
        family: "MCP server",
        command: "/mcp",
        os_category: "mcp",
        service: OsService::FunctionAsAService,
        runtime_binding: RuntimeBindingIntent {
            kind: "mcp",
            isolation: "serving",
            runtime_kind: "a3s-function-service",
            protocol: Some("mcp"),
            agent_kind: Some("tool"),
        },
        stages: MCP_STAGES,
    },
    AssetLifecycle {
        family: "skill",
        command: "/skill",
        os_category: "skill",
        service: OsService::FunctionAsAService,
        runtime_binding: RuntimeBindingIntent {
            kind: "tool",
            isolation: "serving",
            runtime_kind: "a3s-function-service",
            protocol: Some("skill"),
            agent_kind: Some("tool"),
        },
        stages: SKILL_STAGES,
    },
    AssetLifecycle {
        family: "OKF knowledge package",
        command: "/okf",
        os_category: "knowledge",
        service: OsService::KnowledgeService,
        runtime_binding: RuntimeBindingIntent {
            kind: "knowledge",
            isolation: "serving",
            runtime_kind: "a3s-knowledge-service",
            protocol: Some("okf"),
            agent_kind: None,
        },
        stages: OKF_STAGES,
    },
    AssetLifecycle {
        family: "workflow flow",
        command: "/flow",
        os_category: "workflow",
        service: OsService::WorkflowAsAService,
        runtime_binding: RuntimeBindingIntent {
            kind: "workflow",
            isolation: "native",
            runtime_kind: "a3s-workflow-service",
            protocol: Some("workflow"),
            agent_kind: None,
        },
        stages: WORKFLOW_STAGES,
    },
];

pub(crate) fn workflow_flow_lifecycle() -> &'static AssetLifecycle {
    ASSET_LIFECYCLES
        .iter()
        .find(|item| item.family == "workflow flow")
        .expect("workflow flow lifecycle is part of the built-in contract")
}

pub(crate) fn service_label(service: OsService) -> &'static str {
    match service {
        OsService::AgentAsAService => "Agent as a Service",
        OsService::FunctionAsAService => "Function as a Service",
        OsService::WorkflowAsAService => "Workflow as a Service",
        OsService::KnowledgeService => "Knowledge service",
    }
}

#[cfg(test)]
mod tests {
    use super::super::panels;
    use super::*;

    #[test]
    fn all_team_asset_families_have_capability_scoped_lifecycle_metadata() {
        let expected = [
            ("agentic agent", AGENTIC_AGENT_STAGES),
            ("application agent", APPLICATION_AGENT_STAGES),
            ("tool agent", TOOL_AGENT_STAGES),
            ("MCP server", MCP_STAGES),
            ("skill", SKILL_STAGES),
            ("OKF knowledge package", OKF_STAGES),
            ("workflow flow", WORKFLOW_STAGES),
        ];

        assert_eq!(ASSET_LIFECYCLES.len(), expected.len());
        for (family, stages) in expected {
            let lifecycle = ASSET_LIFECYCLES
                .iter()
                .find(|item| item.family == family)
                .unwrap_or_else(|| panic!("missing lifecycle row for {family}"));
            assert_eq!(
                lifecycle.stages, stages,
                "{family} must expose only lifecycle stages backed by real commands or OS surfaces"
            );
        }
    }

    #[test]
    fn lifecycle_metadata_does_not_claim_unsupported_run_or_deploy_surfaces() {
        let lifecycle = |family: &str| {
            ASSET_LIFECYCLES
                .iter()
                .find(|item| item.family == family)
                .unwrap_or_else(|| panic!("missing lifecycle row for {family}"))
        };

        for family in ["agentic agent", "tool agent"] {
            assert!(
                !lifecycle(family).stages.contains(&LifecycleStage::Deploy),
                "{family} should not claim the application deploy stage"
            );
        }
        assert!(
            lifecycle("MCP server")
                .stages
                .contains(&LifecycleStage::Run),
            "MCP servers expose direct run through /mcp run"
        );
        assert!(
            lifecycle("MCP server")
                .stages
                .contains(&LifecycleStage::Test),
            "MCP servers expose batch testing through /mcp test"
        );
        for family in ["agentic agent", "workflow flow"] {
            assert!(
                lifecycle(family).stages.contains(&LifecycleStage::Run),
                "{family} should claim the run stage it exposes"
            );
        }
        for family in [
            "application agent",
            "tool agent",
            "skill",
            "OKF knowledge package",
        ] {
            assert!(
                !lifecycle(family).stages.contains(&LifecycleStage::Run),
                "{family} should not claim a direct run stage"
            );
        }
        for family in ["skill", "OKF knowledge package"] {
            assert!(
                !lifecycle(family).stages.contains(&LifecycleStage::Observe),
                "{family} should not claim a generic observe stage; use inspect/activity commands"
            );
            assert!(
                lifecycle(family).stages.contains(&LifecycleStage::Inspect),
                "{family} should expose read-only status/open inspection"
            );
            assert!(
                lifecycle(family).stages.contains(&LifecycleStage::Activity),
                "{family} should expose asset-scoped Runtime activity"
            );
        }
    }

    #[test]
    fn workflow_flow_uses_workflow_as_a_service() {
        let flow = ASSET_LIFECYCLES
            .iter()
            .find(|item| item.family == "workflow flow")
            .expect("flow lifecycle row");

        assert_eq!(flow.command, "/flow");
        assert_eq!(flow.os_category, "workflow");
        assert_eq!(flow.service, OsService::WorkflowAsAService);
        assert_eq!(flow.runtime_binding.kind, "workflow");
        assert_eq!(flow.runtime_binding.runtime_kind, "a3s-workflow-service");
        assert_eq!(flow.runtime_binding.protocol, Some("workflow"));
        assert_eq!(flow.runtime_binding.agent_kind, None);
    }

    #[test]
    fn lifecycle_runtime_binding_intents_match_asset_generators() {
        let lifecycle = |family: &str| {
            ASSET_LIFECYCLES
                .iter()
                .find(|item| item.family == family)
                .unwrap_or_else(|| panic!("missing lifecycle row for {family}"))
        };

        for (family, kind, isolation, runtime_kind, protocol, agent_kind) in [
            (
                "agentic agent",
                "agent",
                "serving",
                "a3s-agent-service",
                None,
                Some("agentic"),
            ),
            (
                "application agent",
                "agent",
                "container",
                "a3s-agent-service",
                None,
                Some("application"),
            ),
            (
                "tool agent",
                "tool",
                "serving",
                "a3s-function-service",
                Some("agent-tool"),
                Some("tool"),
            ),
            (
                "MCP server",
                "mcp",
                "serving",
                "a3s-function-service",
                Some("mcp"),
                Some("tool"),
            ),
            (
                "skill",
                "tool",
                "serving",
                "a3s-function-service",
                Some("skill"),
                Some("tool"),
            ),
            (
                "OKF knowledge package",
                "knowledge",
                "serving",
                "a3s-knowledge-service",
                Some("okf"),
                None,
            ),
            (
                "workflow flow",
                "workflow",
                "native",
                "a3s-workflow-service",
                Some("workflow"),
                None,
            ),
        ] {
            let actual = lifecycle(family).runtime_binding;
            assert_eq!(actual.kind, kind, "{family} kind");
            assert_eq!(actual.isolation, isolation, "{family} isolation");
            assert_eq!(actual.runtime_kind, runtime_kind, "{family} runtime kind");
            assert_eq!(actual.protocol, protocol, "{family} protocol");
            assert_eq!(actual.agent_kind, agent_kind, "{family} agent kind");
        }

        let app = lifecycle("application agent");
        assert_eq!(app.runtime_binding.kind, "agent");
        assert_eq!(app.runtime_binding.isolation, "container");
        assert_eq!(app.runtime_binding.runtime_kind, "a3s-agent-service");
        assert_eq!(app.runtime_binding.protocol, None);
        assert_eq!(app.runtime_binding.agent_kind, Some("application"));

        let tool = lifecycle("tool agent");
        assert_eq!(tool.runtime_binding.kind, "tool");
        assert_eq!(tool.runtime_binding.isolation, "serving");
        assert_eq!(tool.runtime_binding.runtime_kind, "a3s-function-service");
        assert_eq!(tool.runtime_binding.protocol, Some("agent-tool"));
        assert_eq!(tool.runtime_binding.agent_kind, Some("tool"));

        let okf = lifecycle("OKF knowledge package");
        assert_eq!(okf.runtime_binding.kind, "knowledge");
        assert_eq!(okf.runtime_binding.isolation, "serving");
        assert_eq!(okf.runtime_binding.runtime_kind, "a3s-knowledge-service");
        assert_eq!(okf.runtime_binding.protocol, Some("okf"));
        assert_eq!(okf.runtime_binding.agent_kind, None);
    }

    #[test]
    fn tool_shaped_assets_stay_on_function_as_a_service() {
        for family in ["tool agent", "MCP server", "skill"] {
            let lifecycle = ASSET_LIFECYCLES
                .iter()
                .find(|item| item.family == family)
                .unwrap_or_else(|| panic!("missing lifecycle row for {family}"));
            assert_eq!(lifecycle.service, OsService::FunctionAsAService);
            assert_eq!(
                lifecycle.runtime_binding.runtime_kind,
                "a3s-function-service"
            );
            assert_eq!(lifecycle.runtime_binding.isolation, "serving");
            assert_eq!(lifecycle.runtime_binding.agent_kind, Some("tool"));
        }
    }

    #[test]
    fn asset_commands_are_capability_based() {
        for input in [
            "clone https://github.com/a/asset.git",
            "list",
            "review",
            "activity",
            "publish agentic",
            "publish application",
            "publish tool",
            "run",
            "deploy",
            "open",
            "logs",
            "status",
        ] {
            assert!(
                panels::agent::parse_agent_subcommand(input).is_some(),
                "/agent should parse lifecycle subcommand `{input}`"
            );
        }
        assert!(
            matches!(panels::agent::parse_agent_subcommand("debug"), Some(Err(_))),
            "/agent should reject debug instead of creating a prototype"
        );
        assert!(
            matches!(panels::agent::parse_agent_subcommand("ps"), Some(Err(_))),
            "/agent should reject legacy ps and point to activity"
        );
        for input in [
            "clone https://github.com/a/asset.git",
            "list",
            "review",
            "activity",
            "publish",
            "run",
            "test",
            "deploy",
            "open",
            "logs",
            "status",
        ] {
            assert!(
                matches!(panels::mcp::parse_mcp_subcommand(input), Some(Ok(_))),
                "/mcp should parse lifecycle subcommand `{input}`"
            );
        }
        for input in ["debug", "invoke"] {
            assert!(
                matches!(panels::mcp::parse_mcp_subcommand(input), Some(Err(_))),
                "/mcp should reject {input} instead of creating a prototype"
            );
        }
        assert!(
            matches!(panels::mcp::parse_mcp_subcommand("ps"), Some(Err(_))),
            "/mcp should reject legacy ps and point to activity"
        );
        for input in [
            "clone https://github.com/a/asset.git",
            "list",
            "review",
            "activity",
            "publish",
            "deploy",
            "open",
            "status",
        ] {
            assert!(
                matches!(panels::skill::parse_skill_subcommand(input), Some(Ok(_))),
                "/skill should parse lifecycle subcommand `{input}`"
            );
        }
        for input in ["run", "debug", "logs"] {
            assert!(
                matches!(panels::skill::parse_skill_subcommand(input), Some(Err(_))),
                "/skill should reject unsupported subcommand `{input}`"
            );
        }
        assert!(
            matches!(panels::skill::parse_skill_subcommand("ps"), Some(Err(_))),
            "/skill should reject legacy ps and point to activity"
        );
        for input in ["review ops", "deploy ops"] {
            assert!(
                matches!(panels::skill::parse_skill_subcommand(input), Some(Err(_))),
                "/skill should reject target arguments it cannot honor"
            );
        }
        for input in [
            "clone https://github.com/a/asset.git",
            "list",
            "review",
            "activity",
            "publish",
            "run",
            "deploy",
            "open",
            "logs",
            "status",
        ] {
            assert!(
                panels::flow::parse_flow_subcommand(input).is_some(),
                "/flow should parse lifecycle subcommand `{input}`"
            );
        }
        assert!(
            matches!(panels::flow::parse_flow_subcommand("debug"), Some(Err(_))),
            "/flow should reject debug instead of creating a prototype"
        );
        assert!(
            matches!(panels::flow::parse_flow_subcommand("ps"), Some(Err(_))),
            "/flow should reject legacy ps and point to activity"
        );
        for input in [
            "clone https://github.com/a/asset.git",
            "list",
            "review",
            "activity",
            "publish",
            "deploy",
            "status",
        ] {
            assert!(
                !matches!(
                    panels::okf::parse_okf_command(input),
                    panels::okf::OkfCommand::Prototype(_)
                ),
                "/okf should parse lifecycle subcommand `{input}`"
            );
        }
        for input in [
            "run",
            "debug",
            "os",
            "open",
            "view",
            "dashboard",
            "add",
            "import",
            "search",
            "vault",
            "logs",
            "ps",
        ] {
            assert!(
                matches!(
                    panels::okf::parse_okf_command(input),
                    panels::okf::OkfCommand::Usage(_)
                ),
                "/okf should reject unsupported subcommand `{input}`"
            );
        }
    }
}
