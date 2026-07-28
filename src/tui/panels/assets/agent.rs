//! `/agent` — local multi-turn development for a3s-code agent packages.
//!
//! Bare `/agent` opens a picker over `agent_dir()` (`~/.a3s/agents` or the
//! `agent_dir` config key). Enter validates the selected package entrypoint
//! (`agent.md`, `agent.yaml`, `agent.yml`, or a compatible Markdown/YAML
//! definition) and puts the TUI into a local agent-development context.
//! Subsequent user turns are wrapped with the package path, entrypoint path, and
//! editing constraints so the current TUI session can iteratively improve the
//! agent package.
//!
//! `/agent <natural language>` scaffolds a complete local A3S Code agent package
//! under `agent_dir()`.

use super::super::asset_lifecycle;
use super::super::os_progressive;
use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;

#[derive(Clone)]
pub(crate) struct AgentFile {
    /// Package-relative path used as the local asset id.
    pub(crate) rel: String,
    /// Package directory, or the definition file itself for legacy one-file agents.
    pub(crate) path: std::path::PathBuf,
    /// Entrypoint path relative to the package root.
    pub(crate) definition_rel: String,
    /// Absolute Markdown/YAML entrypoint path.
    pub(crate) definition_path: std::path::PathBuf,
}

/// `/agent` selection panel: local agent packages + cursor.
pub(crate) struct AgentPanel {
    /// Absolute path of the agents root (config `agent_dir`).
    pub(crate) root: std::path::PathBuf,
    /// Local agent packages under the root, sorted by package relative path.
    pub(crate) agents: Vec<AgentFile>,
    pub(crate) sel: usize,
}

/// The local agent package currently being developed in the TUI.
#[derive(Clone)]
pub(crate) struct AgentDevSession {
    pub(crate) name: String,
    pub(crate) description: String,
    /// Package-relative path used as the local asset id.
    pub(crate) rel: String,
    /// Entrypoint path relative to the package root.
    pub(crate) definition_rel: String,
    /// Absolute Markdown/YAML entrypoint path.
    pub(crate) path: std::path::PathBuf,
    /// Package directory, or the definition file itself for legacy one-file agents.
    pub(crate) package_path: std::path::PathBuf,
    pub(crate) root: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentOsKind {
    Agentic,
    Application,
    Tool,
}

impl AgentOsKind {
    pub(crate) fn parse(value: &str) -> Result<Self, String> {
        match value.to_ascii_lowercase().as_str() {
            "agentic" => Ok(Self::Agentic),
            "application" => Ok(Self::Application),
            "tool" => Ok(Self::Tool),
            _ => Err(format!("unknown agent kind `{value}`")),
        }
    }

    pub(crate) fn agent_kind(self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Application => "application",
            Self::Tool => "tool",
        }
    }

    fn asset_prefix(self) -> &'static str {
        match self {
            Self::Agentic => "agentic",
            Self::Application => "agent-app",
            Self::Tool => "agent-tool",
        }
    }

    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Agentic => "Agentic",
            Self::Application => "Application",
            Self::Tool => "Tool",
        }
    }

    pub(crate) fn service_label(self) -> &'static str {
        match self {
            Self::Agentic | Self::Application => "Agent as a Service",
            Self::Tool => "Function as a Service",
        }
    }

    fn runtime_mode(self) -> &'static str {
        match self {
            Self::Agentic => "agentic-run",
            Self::Application => "application-deployment",
            Self::Tool => "tool-serving",
        }
    }

    fn runtime_isolation(self) -> &'static str {
        match self {
            Self::Agentic => "serving",
            Self::Application => "container",
            Self::Tool => "serving",
        }
    }

    fn runtime_kind(self) -> &'static str {
        match self {
            Self::Agentic | Self::Application => "a3s-agent-service",
            Self::Tool => "a3s-function-service",
        }
    }

    fn runtime_protocol(self) -> Option<&'static str> {
        match self {
            Self::Agentic | Self::Application => None,
            Self::Tool => Some("agent-tool"),
        }
    }

    fn uses_agent_config_endpoint(self) -> bool {
        matches!(self, Self::Agentic | Self::Application)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AgentOsAction {
    Publish(AgentOsKind),
    Run(AgentOsKind),
    Deploy,
    Open(AgentOsKind),
    Logs(AgentOsKind),
    Status(AgentOsKind),
}

impl AgentOsAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Publish(_) => "publish",
            Self::Run(_) => "run",
            Self::Deploy => "deploy",
            Self::Open(_) => "open",
            Self::Logs(_) => "logs",
            Self::Status(_) => "status",
        }
    }

    pub(crate) fn target_kind(self) -> AgentOsKind {
        match self {
            Self::Publish(kind) | Self::Open(kind) | Self::Logs(kind) | Self::Status(kind) => kind,
            Self::Run(kind) => kind,
            Self::Deploy => AgentOsKind::Application,
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct AgentOsResult {
    pub(crate) action: AgentOsAction,
    pub(crate) kind: AgentOsKind,
    pub(crate) asset_name: String,
    pub(crate) asset_id: String,
    pub(crate) view: remote_ui::ViewSpec,
    pub(crate) note: String,
    pub(crate) open_view: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentAssetRef {
    id: String,
    name: String,
    owner_name: Option<String>,
    default_branch: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentCommitRef {
    sha: String,
    branch: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct AgentNamespaceRef {
    id: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentSubcommand {
    Exit,
    Clone(String),
    List(String),
    Review,
    Activity(String),
    Publish(AgentOsKind),
    Run,
    Deploy,
    Open(AgentOsKind),
    Logs(AgentOsKind),
    Status(AgentOsKind),
}

const AGENT_PUBLISH_USAGE: &str = "usage: /agent publish agentic|application|tool";
const AGENT_OPEN_USAGE: &str = "usage: /agent open [agentic|application|tool]";
const AGENT_LOGS_USAGE: &str = "usage: /agent logs [agentic|application|tool]";
const AGENT_STATUS_USAGE: &str = "usage: /agent status [agentic|application|tool]";

pub(crate) fn parse_agent_subcommand(input: &str) -> Option<Result<AgentSubcommand, String>> {
    let mut parts = input.split_whitespace();
    let head = parts.next()?.to_ascii_lowercase();
    match head.as_str() {
        "off" => {
            if parts.next().is_some() {
                return Some(Err("usage: /agent off".to_string()));
            }
            Some(Ok(AgentSubcommand::Exit))
        }
        "exit" | "normal" | "clear" | "stop" => Some(Err("usage: /agent off".to_string())),
        "clone" => {
            let Some(url) = parts.next() else {
                return Some(Err("usage: /agent clone <git-url>".to_string()));
            };
            if parts.next().is_some() {
                return Some(Err("usage: /agent clone <git-url>".to_string()));
            }
            Some(Ok(AgentSubcommand::Clone(url.to_string())))
        }
        "list" => Some(Ok(AgentSubcommand::List(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "review" => {
            if parts.next().is_some() {
                return Some(Err("usage: /agent review".to_string()));
            }
            Some(Ok(AgentSubcommand::Review))
        }
        "activity" => Some(Ok(AgentSubcommand::Activity(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "ps" | "runs" | "jobs" => Some(Err("usage: /agent activity [query]".to_string())),
        "publish" => {
            let Some(kind) = parts.next() else {
                return Some(Err(AGENT_PUBLISH_USAGE.to_string()));
            };
            if parts.next().is_some() {
                return Some(Err(AGENT_PUBLISH_USAGE.to_string()));
            }
            Some(parse_agent_os_kind(kind, AGENT_PUBLISH_USAGE).map(AgentSubcommand::Publish))
        }
        "run" => {
            if parts.next().is_some() {
                return Some(Err("usage: /agent run".to_string()));
            }
            Some(Ok(AgentSubcommand::Run))
        }
        "debug" | "test" | "invoke" | "batch" => {
            Some(Err(format!("unknown /agent command `{head}`")))
        }
        "deploy" => {
            if parts.next().is_some() {
                return Some(Err("usage: /agent deploy".to_string()));
            }
            Some(Ok(AgentSubcommand::Deploy))
        }
        "open" => {
            let kind = parts.next();
            if parts.next().is_some() {
                return Some(Err(AGENT_OPEN_USAGE.to_string()));
            }
            Some(
                parse_optional_agent_os_kind(kind, AgentOsKind::Agentic, AGENT_OPEN_USAGE)
                    .map(AgentSubcommand::Open),
            )
        }
        "logs" => {
            let kind = parts.next();
            if parts.next().is_some() {
                return Some(Err(AGENT_LOGS_USAGE.to_string()));
            }
            Some(
                parse_optional_agent_os_kind(kind, AgentOsKind::Agentic, AGENT_LOGS_USAGE)
                    .map(AgentSubcommand::Logs),
            )
        }
        "status" => {
            let kind = parts.next();
            if parts.next().is_some() {
                return Some(Err(AGENT_STATUS_USAGE.to_string()));
            }
            Some(
                parse_optional_agent_os_kind(kind, AgentOsKind::Agentic, AGENT_STATUS_USAGE)
                    .map(AgentSubcommand::Status),
            )
        }
        "inspect" => Some(Err(AGENT_STATUS_USAGE.to_string())),
        "view" | "remote" => Some(Err(AGENT_OPEN_USAGE.to_string())),
        "os" => Some(Err(AGENT_STATUS_USAGE.to_string())),
        "dashboard" => Some(Err(
            "usage: /agent list [query] · /agent status [agentic|application|tool]".to_string(),
        )),
        _ => None,
    }
}

fn parse_optional_agent_os_kind(
    value: Option<&str>,
    default: AgentOsKind,
    usage: &'static str,
) -> Result<AgentOsKind, String> {
    match value {
        Some(value) => parse_agent_os_kind(value, usage),
        None => Ok(default),
    }
}

fn parse_agent_os_kind(value: &str, usage: &'static str) -> Result<AgentOsKind, String> {
    AgentOsKind::parse(value).map_err(|_| usage.to_string())
}

/// List local agent packages recursively, skipping dotfiles and dot-directories.
/// A package is normally a directory with `agent.md`, `agent.yaml`, `agent.yml`,
/// or a definition file named after the directory. Legacy one-file definitions
/// are still listed as one-file packages for compatibility.
pub(crate) fn list_agents(root: &std::path::Path) -> Vec<AgentFile> {
    let mut out = Vec::new();
    list_agents_inner(root, root, &mut out);
    out.sort_by(|a, b| {
        a.rel
            .cmp(&b.rel)
            .then_with(|| a.definition_rel.cmp(&b.definition_rel))
    });
    out
}

fn list_agents_inner(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<AgentFile>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries = entries.flatten().collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    if dir != root {
        if let Some(definition_path) = agent_entry_file(dir) {
            out.push(agent_file_from_entry(root, dir, &definition_path));
            return;
        }
    }

    for entry in &entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        if !path.is_file() || !is_agent_definition_file(&path) {
            continue;
        }
        out.push(agent_file_from_entry(root, &path, &path));
    }

    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') || !path.is_dir() {
            continue;
        }
        list_agents_inner(root, &path, out);
    }
}

pub(crate) fn agent_entry_file(dir: &std::path::Path) -> Option<std::path::PathBuf> {
    let candidates = ["agent.md", "agent.yaml", "agent.yml"];
    for candidate in candidates {
        let path = dir.join(candidate);
        if path.is_file() {
            return Some(path);
        }
    }
    let dir_stem = dir.file_name().and_then(|name| name.to_str())?;
    for ext in ["md", "yaml", "yml"] {
        let path = dir.join(format!("{dir_stem}.{ext}"));
        if path.is_file() {
            return Some(path);
        }
    }
    None
}

fn is_agent_definition_file(path: &std::path::Path) -> bool {
    matches!(
        path.extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_ascii_lowercase())
            .as_deref(),
        Some("md" | "yaml" | "yml")
    )
}

fn normalized_rel(root: &std::path::Path, path: &std::path::Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/")
}

fn rel_without_extension(rel: &str) -> String {
    let path = std::path::Path::new(rel);
    match path.file_stem().and_then(|stem| stem.to_str()) {
        Some(stem) => path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
            .map(|parent| {
                let parent = parent
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy())
                    .collect::<Vec<_>>()
                    .join("/");
                format!("{parent}/{stem}")
            })
            .unwrap_or_else(|| stem.to_string()),
        None => rel.to_string(),
    }
}

fn agent_file_from_entry(
    root: &std::path::Path,
    package_path: &std::path::Path,
    definition_path: &std::path::Path,
) -> AgentFile {
    let definition_root_rel = normalized_rel(root, definition_path);
    let (rel, definition_rel, path) = if package_path.is_file() {
        (
            rel_without_extension(&definition_root_rel),
            definition_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or(definition_root_rel),
            definition_path.to_path_buf(),
        )
    } else {
        let package_rel = normalized_rel(root, package_path);
        let rel = if package_rel.is_empty() {
            package_path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| rel_without_extension(&definition_root_rel))
        } else {
            package_rel
        };
        (
            rel,
            normalized_rel(package_path, definition_path),
            package_path.to_path_buf(),
        )
    };
    AgentFile {
        rel,
        path,
        definition_rel,
        definition_path: definition_path.to_path_buf(),
    }
}

fn parse_agent_definition(
    path: &std::path::Path,
    content: &str,
) -> Result<a3s_code_core::subagent::AgentDefinition, String> {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("md") => a3s_code_core::subagent::parse_agent_md(content),
        Some("yaml" | "yml") => a3s_code_core::subagent::parse_agent_yaml(content),
        _ => Err(anyhow::anyhow!("unsupported agent file extension")),
    }
    .map_err(|e| e.to_string())
}

fn agent_picker_header(total: usize, root: &std::path::Path, width: usize) -> String {
    truncate(
        &format!(
            "  ◇ agent — select a package ({total} in {})",
            root.to_string_lossy()
        ),
        width,
    )
}

fn agent_picker_hint(width: usize) -> String {
    truncate("  ↑/↓ select · Enter develop locally · Esc cancel", width)
}

fn agent_picker_lines(
    agents: &[AgentFile],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) = agent_picker_panel(agents, selected, root, width, height)
    else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn agent_picker_panel(
    agents: &[AgentFile],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Option<(MenuPanel, usize)> {
    let total = agents.len();
    if total == 0 {
        return None;
    }
    let max_items = height.saturating_sub(8).clamp(3, 12);
    let selected = selected.min(total.saturating_sub(1));
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let items = agents
        .iter()
        .map(|agent| MenuItem::new(agent.rel.clone()))
        .collect::<Vec<_>>();

    let panel = MenuPanel::new(agent_picker_header(total, root, width).trim_start())
        .subtitle(agent_picker_hint(width).trim_start())
        .items(items)
        .selected(selected)
        .scroll(scroll)
        .max_items(max_items)
        .show_scroll(total > max_items)
        .indent(2)
        .marker("▸")
        .title_color(ACCENT)
        .subtitle_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED);
    Some((panel, max_items + 3))
}

fn agent_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

/// Test fixture for the legacy LLM asset-generation contract.
#[cfg(test)]
pub(crate) fn agent_gen_prompt(description: &str, dir: &str) -> String {
    format!(
        "Create one complete local A3S Code agent package from the description below and save it \
         under {dir}. This is a SMALL asset-folder task: do it directly in this turn — do NOT \
         plan, delegate, or fan out subagents.\n\
         Description: {description}\n\
         IMPORTANT: {dir} is OUTSIDE this session's workspace, so the path-scoped file \
         tools will reject it — use the `bash` tool for ALL file creation and edits under \
         {dir}. Prefer one heredoc script that creates the package directory and all required \
         files in the first tool call. The script must be non-interactive and bounded: use only \
         mkdir/cat heredocs/test/sed/find and `python3 -m json.tool` for JSON checks. Do NOT run \
         the generated agent, do NOT start services, do NOT install dependencies, and do NOT run \
         cargo/npm/pip/git/a3s commands. Never run a command that waits on stdin.\n\
         Create {dir}/<kebab-case-agent-name>/ as a complete A3S Code framework package. \
         The entrypoint file MUST be {dir}/<kebab-case-agent-name>/agent.md with YAML \
         frontmatter, because a3s-code loads `.md`, `.yaml`, and `.yml` agent definitions \
         and Markdown keeps the agent's system prompt readable. Use this exact frontmatter \
         shape:\n\
         ---\n\
         name: <kebab-case-agent-name>\n\
         description: <one-line trigger/purpose>\n\
         tools: Read, Grep, Glob, Bash\n\
         max_steps: 30\n\
         ---\n\
         <system prompt for the agent>\n\
         Do NOT stop at only agent.md. The package MUST include these files:\n\
         - README.md explaining purpose, trigger, inputs, outputs, local development, \
         publish/run/deploy lifecycle, and safety constraints.\n\
         - agent.md as the A3S Code entrypoint definition.\n\
         - prompts/system.md with the expanded system prompt used by agent.md.\n\
         - workflows/operating-procedure.md with step-by-step run workflow, failure handling, \
         and completion criteria.\n\
         - examples/example-input.md and examples/example-output.md.\n\
         - eval/smoke.md with manual evaluation cases and expected pass criteria.\n\
         - tests/smoke.md with a lightweight local verification checklist.\n\
         - .a3s/asset.acl. Default to agentKind=agentic unless the description clearly asks \
         for an application or tool agent.\n\
         The package-local `.a3s/` directory is metadata-only. Do NOT put `agent.md`, \
         prompts, workflows, examples, evals, tests, or other source files under `.a3s/`.\n\
         Do NOT create extra generated JSON config files; keep package configuration in \
         `.a3s/asset.acl`. Runtime configuration is synced through OS APIs during \
         publish/run/deploy, not stored in the asset repository.\n\
         Rules: make `name` kebab-case and stable; keep `description` one line and \
         action-oriented; choose a conservative tools list (omit tools that are not needed); \
         write a practical system prompt with scope, workflow, safety boundaries, examples, \
         and success criteria; do not include secrets. If the folder exists, append -2, -3, … \
         to the folder name. Validate the definition with `test -s \"$FILE\" && sed -n '1,80p' \"$FILE\"` (always pass the file \
         path). Do not execute tests/smoke.md; it is a checklist artifact. After validation \
         succeeds, stop using tools immediately and give a concise final answer with the saved \
         package path and the note that `/agent` starts local interactive development for the \
         complete package."
    )
}

pub(crate) fn scaffold_agent_package(
    description: &str,
    root: &std::path::Path,
) -> Result<AgentDevSession, String> {
    let name = agent_scaffold_name(description);
    let package = unique_agent_package_dir(root, &name);
    let final_name = package
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name.as_str())
        .to_string();
    let description = agent_scaffold_description(description, &final_name);
    let kind = agent_scaffold_kind(description.as_str());
    let system_prompt = agent_scaffold_system_prompt(&final_name, &description);

    std::fs::create_dir_all(package.join(".a3s"))
        .map_err(|e| format!("could not create {}: {e}", package.join(".a3s").display()))?;
    for dir in ["prompts", "workflows", "examples", "eval", "tests"] {
        std::fs::create_dir_all(package.join(dir))
            .map_err(|e| format!("could not create {}: {e}", package.join(dir).display()))?;
    }

    let agent_md = format!(
        "---\n\
         name: {name}\n\
         description: {description}\n\
         tools: Read, Grep, Glob, Bash\n\
         max_steps: 30\n\
         ---\n\n\
         {system_prompt}\n",
        name = final_name,
        description = yaml_string(&description),
    );
    let agent_path = package.join("agent.md");
    write_agent_scaffold_file(&agent_path, agent_md.as_bytes())?;
    write_agent_scaffold_file(&package.join("prompts/system.md"), system_prompt.as_bytes())?;
    write_agent_scaffold_file(
        &package.join("README.md"),
        agent_scaffold_readme(&final_name, &description).as_bytes(),
    )?;
    write_agent_scaffold_file(
        &package.join("workflows/operating-procedure.md"),
        agent_scaffold_workflow(&description).as_bytes(),
    )?;
    write_agent_scaffold_file(
        &package.join("examples/example-input.md"),
        agent_scaffold_example_input(&description).as_bytes(),
    )?;
    write_agent_scaffold_file(
        &package.join("examples/example-output.md"),
        agent_scaffold_example_output().as_bytes(),
    )?;
    write_agent_scaffold_file(
        &package.join("eval/smoke.md"),
        agent_scaffold_eval().as_bytes(),
    )?;
    write_agent_scaffold_file(
        &package.join("tests/smoke.md"),
        agent_scaffold_tests().as_bytes(),
    )?;
    let def = parse_agent_definition(&agent_path, &agent_md)
        .map_err(|e| format!("generated agent.md is invalid: {e}"))?;
    let local_rel = normalized_rel(root, &package);
    let package_source_path = agent_package_source_path(&local_rel);
    let asset_source_path = agent_asset_source_path(&local_rel, "agent.md");
    let asset_acl = agent_asset_acl(
        kind,
        &def,
        &local_rel,
        &package_source_path,
        "agent.md",
        &asset_source_path,
    );
    write_agent_scaffold_file(
        &package.join(asset_lifecycle::ASSET_ACL_PATH),
        asset_acl.as_bytes(),
    )?;
    agent_dev_session_from_file(root, &agent_path)
}

fn agent_scaffold_name(description: &str) -> String {
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
        let slug = asset_slug(raw);
        if slug != "asset" {
            return slug;
        }
    }

    let words = description
        .split_whitespace()
        .filter(|word| !word.eq_ignore_ascii_case("agent"))
        .take(8)
        .collect::<Vec<_>>()
        .join(" ");
    truncate_agent_slug(asset_slug(if words.trim().is_empty() {
        description
    } else {
        &words
    }))
}

fn truncate_agent_slug(slug: String) -> String {
    const MAX_LEN: usize = 48;
    if slug.chars().count() <= MAX_LEN {
        return slug;
    }
    let mut out = slug.chars().take(MAX_LEN).collect::<String>();
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() {
        "agent".to_string()
    } else {
        out
    }
}

fn unique_agent_package_dir(root: &std::path::Path, name: &str) -> std::path::PathBuf {
    let base = if name.trim().is_empty() {
        "agent"
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

fn agent_scaffold_description(description: &str, name: &str) -> String {
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
        format!("A3S Code agent package for {name}")
    } else {
        truncate(&text.replace('"', "'"), 180)
    }
}

fn agent_scaffold_kind(description: &str) -> AgentOsKind {
    let lower = description.to_ascii_lowercase();
    if lower.contains("agentkind=tool")
        || lower.contains("tool agent")
        || lower.contains("tool-kind")
        || lower.contains("function as a service")
    {
        AgentOsKind::Tool
    } else if lower.contains("agentkind=application")
        || lower.contains("application agent")
        || lower.contains("app agent")
    {
        AgentOsKind::Application
    } else {
        AgentOsKind::Agentic
    }
}

fn agent_scaffold_system_prompt(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n\
         You are `{name}`, an A3S Code agent created for this purpose: {description}.\n\n\
         ## Operating Scope\n\
         - Stay focused on the user's requested task and the package resources in this directory.\n\
         - Prefer reading relevant files before making claims or edits.\n\
         - Use tools conservatively and explain blockers with concrete evidence.\n\n\
         ## Workflow\n\
         1. Restate the task in one sentence when the scope is ambiguous.\n\
         2. Gather the smallest useful context from the workspace or user input.\n\
         3. Produce concise, actionable output with file paths, commands, or findings when relevant.\n\
         4. Verify important claims before finalizing.\n\n\
         ## Safety\n\
         - Do not expose secrets or credentials.\n\
         - Do not run destructive commands unless the user explicitly asks and the risk is clear.\n\
         - Keep network and shell usage bounded.\n\n\
         ## Completion Criteria\n\
         - The answer directly addresses the task.\n\
         - Any assumptions, skipped checks, or follow-up risks are named plainly."
    )
}

fn agent_scaffold_readme(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n\
         {description}.\n\n\
         ## Package Layout\n\n\
         - `agent.md` is the A3S Code entrypoint.\n\
         - `prompts/system.md` contains the expanded system prompt.\n\
         - `workflows/operating-procedure.md` defines the run workflow.\n\
         - `examples/` contains sample input and output.\n\
         - `eval/smoke.md` and `tests/smoke.md` define lightweight checks.\n\
         - `.a3s/` contains only `asset.acl`.\n\n\
         ## Lifecycle\n\n\
         - `a3s code agent review agents/{name}/agent.md`\n\
         - `a3s code agent publish agents/{name}/agent.md --kind agentic`\n\
         - `a3s code agent run agents/{name}/agent.md`\n\
         - `a3s code agent status agents/{name}/agent.md --kind agentic`\n\n\
         ## Safety\n\n\
         Keep secrets out of this package. Review tool permissions before publishing."
    )
}

fn agent_scaffold_workflow(description: &str) -> String {
    format!(
        "# Operating Procedure\n\n\
         Purpose: {description}.\n\n\
         1. Inspect the user's input and determine whether the request is in scope.\n\
         2. Gather only the context needed for the task.\n\
         3. Execute the task using the allowed A3S Code tools.\n\
         4. Validate outputs against the success criteria in `agent.md`.\n\
         5. Return a concise result with evidence and next steps.\n\n\
         ## Failure Handling\n\n\
         - If required context is missing, ask for the smallest clarifying input.\n\
         - If a tool is unavailable, explain the missing capability and continue with a safe fallback.\n\
         - If the task is out of scope, decline that part and provide the closest safe alternative."
    )
}

fn agent_scaffold_example_input(description: &str) -> String {
    format!(
        "# Example Input\n\n\
         User task: {description}.\n\n\
         Provide the files, diff, prompt, or structured input that the agent should process."
    )
}

fn agent_scaffold_example_output() -> &'static str {
    "# Example Output\n\n\
     Summary: the requested task was handled.\n\n\
     Findings:\n\
     - `path/or/item`: concise evidence-backed observation.\n\n\
     Next step: run the smoke checklist before publishing."
}

fn agent_scaffold_eval() -> &'static str {
    "# Smoke Evaluation\n\n\
     - The agent definition parses as valid A3S Code Markdown.\n\
     - The agent states its scope and completion criteria.\n\
     - The examples match the intended task.\n\
     - `.a3s/asset.acl` points to the visible source path."
}

fn agent_scaffold_tests() -> &'static str {
    "# Local Smoke Checklist\n\n\
     1. Open `agent.md` and confirm the frontmatter has `name`, `description`, `tools`, and `max_steps`.\n\
     2. Confirm `prompts/system.md` matches the prompt in `agent.md`.\n\
     3. Confirm `.a3s/asset.acl` points to `agent.md`.\n\
     4. Run `/agent review` in A3S Code before publishing.\n\
     5. Publish with the lifecycle command that matches the target kind."
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn write_agent_scaffold_file(path: &std::path::Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    std::fs::write(path, bytes).map_err(|e| format!("could not write {}: {e}", path.display()))
}

fn agent_description(def: &a3s_code_core::subagent::AgentDefinition) -> String {
    let desc = def.description.trim();
    if desc.chars().count() >= 10 {
        desc.to_string()
    } else {
        format!("A3S Code agent definition for {}", def.name)
    }
}

pub(crate) fn agent_asset_name(kind: AgentOsKind, agent_name: &str) -> String {
    format!("{}-{}", kind.asset_prefix(), asset_slug(agent_name))
}

pub(crate) fn agent_dev_session_from_file(
    root: &std::path::Path,
    path: &std::path::Path,
) -> Result<AgentDevSession, String> {
    let source = std::fs::read_to_string(path)
        .map_err(|e| format!("could not read {}: {e}", path.display()))?;
    let def = parse_agent_definition(path, &source)
        .map_err(|e| format!("{} is not a valid agent definition: {e}", path.display()))?;
    let parent = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let path_stem = path.file_stem().and_then(|stem| stem.to_str());
    let parent_stem = parent.file_name().and_then(|name| name.to_str());
    let entry_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("");
    let package_path = if entry_name.starts_with("agent.")
        || path_stem
            .zip(parent_stem)
            .is_some_and(|(path, parent)| path == parent)
    {
        parent.to_path_buf()
    } else {
        path.to_path_buf()
    };
    let agent_file = agent_file_from_entry(root, &package_path, path);
    Ok(AgentDevSession {
        name: def.name.clone(),
        description: agent_description(&def),
        rel: agent_file.rel,
        definition_rel: agent_file.definition_rel,
        path: path.to_path_buf(),
        package_path,
        root: root.to_path_buf(),
    })
}

fn agent_package_source_path(rel: &str) -> String {
    let _ = rel;
    ".".to_string()
}

fn agent_asset_source_path(package_rel: &str, definition_rel: &str) -> String {
    let package = agent_package_source_path(package_rel);
    let definition = definition_rel.trim_start_matches('/').replace('\\', "/");
    if package == "." {
        definition
    } else {
        format!("{package}/{definition}")
    }
}

fn agent_asset_acl(
    kind: AgentOsKind,
    def: &a3s_code_core::subagent::AgentDefinition,
    local_rel: &str,
    package_source_path: &str,
    definition_rel: &str,
    asset_source_path: &str,
) -> String {
    let service = match kind {
        AgentOsKind::Agentic | AgentOsKind::Application => {
            asset_lifecycle::OsService::AgentAsAService
        }
        AgentOsKind::Tool => asset_lifecycle::OsService::FunctionAsAService,
    };
    let runtime = asset_lifecycle::RuntimeBindingIntent {
        kind: if matches!(kind, AgentOsKind::Tool) {
            "tool"
        } else {
            "agent"
        },
        isolation: kind.runtime_isolation(),
        runtime_kind: kind.runtime_kind(),
        protocol: kind.runtime_protocol(),
        agent_kind: Some(kind.agent_kind()),
    };
    let source = [
        ("package_path", package_source_path),
        ("entrypoint", definition_rel),
        ("definition_path", asset_source_path),
    ];
    let metadata: [(&str, &str); 0] = [];
    let description = agent_description(def);
    let mut acl = asset_lifecycle::render_asset_acl(asset_lifecycle::AssetAclDocument {
        category: "agent",
        kind: Some(kind.agent_kind()),
        name: def.name.as_str(),
        description: &description,
        local_path: Some(local_rel),
        service,
        runtime,
        source: &source,
        metadata: &metadata,
    });
    acl.push('\n');
    acl.push_str(&agent_contract_acl_block(kind));
    acl
}

fn agent_contract_acl_block(kind: AgentOsKind) -> String {
    match kind {
        AgentOsKind::Tool => {
            "contract {\n  protocol = \"http+sse\"\n  port = 29653\n  health = \"/healthz\"\n  manifest = \"/api/agent/manifest\"\n  run = \"/api/agent/run\"\n  stream = \"/api/agent/stream\"\n  timeout_sec = 60\n}\n".to_string()
        }
        AgentOsKind::Agentic | AgentOsKind::Application => {
            "contract {\n  protocol = \"http+sse\"\n  port = 29653\n  health = \"/healthz\"\n  manifest = \"/api/agent/manifest\"\n  session_create = \"/api/agent/sessions\"\n  session_message = \"/api/agent/sessions/{sessionId}/messages\"\n  session_events = \"/api/agent/sessions/{sessionId}/events\"\n  session_result = \"/api/agent/sessions/{sessionId}/result\"\n  session_close = \"/api/agent/sessions/{sessionId}\"\n  timeout_sec = 300\n}\n".to_string()
        }
    }
}

fn path_segment(value: &str) -> String {
    let mut out = String::new();
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

pub(crate) fn agent_asset_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/assets/{}?embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

pub(crate) fn agent_asset_search_url(origin: &str, name: &str) -> String {
    format!(
        "{}/admin/kernel/assets?focus=1&scope=mine&status=all&search={}&embed=1",
        origin.trim_end_matches('/'),
        path_segment(name)
    )
}

fn agent_logs_url(origin: &str, kind: AgentOsKind, asset_id: &str) -> String {
    let asset = path_segment(asset_id);
    match kind {
        AgentOsKind::Agentic => format!(
            "{}/admin/kernel/processes?focus=1&asset={asset}&agentKind=agentic&logs=1",
            origin.trim_end_matches('/')
        ),
        AgentOsKind::Application => format!(
            "{}/admin/infrastructure/batch?asset={asset}&agentKind=application&logs=1&embed=1",
            origin.trim_end_matches('/')
        ),
        AgentOsKind::Tool => format!(
            "{}/admin/infrastructure/batch?asset={asset}&agentKind=tool&category=agent&logs=1&embed=1",
            origin.trim_end_matches('/')
        ),
    }
}

pub(crate) fn agent_view_spec(url: String) -> remote_ui::ViewSpec {
    remote_ui::ViewSpec {
        url,
        width: Some(1440),
        height: Some(900),
        embeddable: true,
    }
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn agent_manifest_json(
    kind: AgentOsKind,
    def: &a3s_code_core::subagent::AgentDefinition,
    local_rel: &str,
    package_source_path: &str,
    definition_rel: &str,
    asset_source_path: &str,
) -> serde_json::Value {
    let mut runtime_intent = serde_json::json!({
        "kind": if matches!(kind, AgentOsKind::Tool) { "tool" } else { "agent" },
        "isolation": kind.runtime_isolation(),
        "agentKind": kind.agent_kind(),
        "runtimeKind": kind.runtime_kind(),
    });
    if let Some(protocol) = kind.runtime_protocol() {
        runtime_intent["protocol"] = serde_json::json!(protocol);
    }
    serde_json::json!({
        "version": "a3s.agent.asset.v1",
        "category": "agent",
        "agentKind": kind.agent_kind(),
        "name": def.name.as_str(),
        "description": agent_description(def),
        "packagePath": package_source_path,
        "entrypoint": definition_rel,
        "definitionPath": asset_source_path,
        "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        "localPath": local_rel,
        "service": kind.service_label(),
        "runtimeIntent": runtime_intent,
        "createdBy": "a3s-code-tui",
        "definition": def,
    })
}

pub(crate) fn agent_contract_json(kind: AgentOsKind) -> serde_json::Value {
    match kind {
        AgentOsKind::Tool => serde_json::json!({
            "protocol": "http+sse",
            "port": 29653,
            "health": "/healthz",
            "manifest": "/api/agent/manifest",
            "run": "/api/agent/run",
            "stream": "/api/agent/stream",
            "timeoutSec": 60,
        }),
        AgentOsKind::Agentic | AgentOsKind::Application => serde_json::json!({
            "protocol": "http+sse",
            "port": 29653,
            "health": "/healthz",
            "manifest": "/api/agent/manifest",
            "sessionCreate": "/api/agent/sessions",
            "sessionMessage": "/api/agent/sessions/{sessionId}/messages",
            "sessionEvents": "/api/agent/sessions/{sessionId}/events",
            "sessionResult": "/api/agent/sessions/{sessionId}/result",
            "sessionClose": "/api/agent/sessions/{sessionId}",
            "timeoutSec": 300,
        }),
    }
}

pub(crate) fn agent_config_json(
    kind: AgentOsKind,
    def: &a3s_code_core::subagent::AgentDefinition,
    local_rel: &str,
    package_source_path: &str,
    definition_rel: &str,
    asset_source_path: &str,
) -> serde_json::Value {
    let prompt = def
        .prompt
        .as_deref()
        .map(str::trim)
        .filter(|prompt| !prompt.is_empty())
        .unwrap_or_else(|| def.description.trim());
    let model = match &def.model {
        Some(model) => serde_json::json!({
            "provider": model.provider.as_deref().unwrap_or("custom"),
            "modelId": model.model.as_str(),
        }),
        None => serde_json::json!({
            "provider": "custom",
            "modelId": "default",
        }),
    };
    let tools = agent_config_tools(def);
    let mut config = serde_json::json!({
        "systemPrompt": prompt,
        "model": model,
        "personality": {
            "name": def.name.as_str(),
            "role": agent_description(def),
        },
        "runtimePolicy": {
            "agentKind": kind.agent_kind(),
            "source": "a3s-code-tui",
            "packagePath": package_source_path,
            "entrypoint": definition_rel,
            "definitionPath": asset_source_path,
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
            "localPath": local_rel,
        },
        "safetyPolicy": {
            "permissions": def.permissions,
            "confirmationInheritance": def.confirmation_inheritance,
        },
        "enableThinking": matches!(kind, AgentOsKind::Agentic),
        "enableCaching": true,
    });
    if let Some(max_steps) = def.max_steps {
        config["maxIterations"] = serde_json::json!(max_steps);
    }
    if !tools.is_empty() {
        config["tools"] = serde_json::Value::Array(tools);
    }
    config
}

pub(crate) fn agent_runtime_binding_json(
    kind: AgentOsKind,
    def: &a3s_code_core::subagent::AgentDefinition,
    local_rel: &str,
    package_source_path: &str,
    definition_rel: &str,
    asset_source_path: &str,
) -> serde_json::Value {
    let resources = if matches!(kind, AgentOsKind::Application) {
        serde_json::json!({ "replicas": 1 })
    } else {
        serde_json::json!({})
    };
    let mut runtime = serde_json::json!({
        "kind": kind.runtime_kind(),
        "agentKind": kind.agent_kind(),
        "mode": kind.runtime_mode(),
    });
    if let Some(protocol) = kind.runtime_protocol() {
        runtime["protocol"] = serde_json::json!(protocol);
    }
    serde_json::json!({
        "version": "a3s.agent.runtime-binding.v1",
        "kind": if matches!(kind, AgentOsKind::Tool) { "tool" } else { "agent" },
        "enabled": true,
        "isolation": kind.runtime_isolation(),
        "target": {
            "kind": "asset",
            "ref": "main",
            "packagePath": package_source_path,
            "entrypoint": definition_rel,
            "definitionPath": asset_source_path,
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        },
        "runtime": runtime,
        "env": [],
        "requiredSecrets": [],
        "resources": resources,
        "network": {
            "ingress": matches!(kind, AgentOsKind::Application),
        },
        "metadata": {
            "source": "a3s-code-tui",
            "service": kind.service_label(),
            "agentKind": kind.agent_kind(),
            "agentName": def.name.as_str(),
            "description": agent_description(def),
            "packagePath": package_source_path,
            "entrypoint": definition_rel,
            "definitionPath": asset_source_path,
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
            "localPath": local_rel,
        },
    })
}

fn declared_agent_kind(package_path: &std::path::Path) -> Option<AgentOsKind> {
    let acl = std::fs::read_to_string(package_path.join(asset_lifecycle::ASSET_ACL_PATH)).ok()?;
    acl.lines().find_map(|line| {
        let line = line.trim();
        let value = line
            .strip_prefix("kind =")
            .or_else(|| line.strip_prefix("agent_kind ="))?
            .trim()
            .trim_matches('"')
            .trim();
        match value {
            "agentic" => Some(AgentOsKind::Agentic),
            "application" => Some(AgentOsKind::Application),
            "tool" => Some(AgentOsKind::Tool),
            _ => None,
        }
    })
}

#[derive(Default)]
struct AgentAclRuntimeEnv {
    env: Vec<serde_json::Value>,
    required_secrets: Vec<String>,
}

#[derive(Default)]
struct AgentAclEnvItem {
    name: Option<String>,
    value: Option<String>,
    secret_ref: Option<String>,
    required: Option<bool>,
}

fn merge_agent_acl_runtime_env(
    runtime_binding: &mut serde_json::Value,
    package_path: &std::path::Path,
) {
    let acl_env = parse_agent_acl_runtime_env(package_path);
    if acl_env.env.is_empty() && acl_env.required_secrets.is_empty() {
        return;
    }

    let mut env_by_name = std::collections::BTreeMap::<String, serde_json::Value>::new();
    if let Some(existing) = runtime_binding
        .get("env")
        .and_then(|value| value.as_array())
    {
        for item in existing {
            if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
                env_by_name.insert(name.to_string(), item.clone());
            }
        }
    }
    for item in acl_env.env {
        if let Some(name) = item.get("name").and_then(|value| value.as_str()) {
            env_by_name.insert(name.to_string(), item);
        }
    }

    let mut required = std::collections::BTreeSet::<String>::new();
    if let Some(existing) = runtime_binding
        .get("requiredSecrets")
        .and_then(|value| value.as_array())
    {
        for item in existing {
            if let Some(name) = item.as_str() {
                required.insert(name.to_string());
            }
        }
    }
    required.extend(acl_env.required_secrets);
    for item in env_by_name.values() {
        let required_env = item
            .get("required")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        if required_env {
            if let Some(secret_ref) = item.get("secretRef").and_then(|value| value.as_str()) {
                required.insert(secret_ref.to_string());
            }
        }
    }

    if let Some(obj) = runtime_binding.as_object_mut() {
        obj.insert(
            "env".to_string(),
            serde_json::Value::Array(env_by_name.into_values().collect()),
        );
        obj.insert(
            "requiredSecrets".to_string(),
            serde_json::Value::Array(
                required
                    .into_iter()
                    .map(serde_json::Value::String)
                    .collect(),
            ),
        );
    }
}

fn parse_agent_acl_runtime_env(package_path: &std::path::Path) -> AgentAclRuntimeEnv {
    let Ok(acl) = std::fs::read_to_string(package_path.join(asset_lifecycle::ASSET_ACL_PATH))
    else {
        return AgentAclRuntimeEnv::default();
    };

    let mut out = AgentAclRuntimeEnv::default();
    let mut current: Option<AgentAclEnvItem> = None;
    for raw_line in acl.lines() {
        let line = raw_line.split('#').next().unwrap_or("").trim();
        if line.is_empty() {
            continue;
        }
        if line == "[[runtime.env]]" {
            push_agent_acl_env_item(&mut out, current.take());
            current = Some(AgentAclEnvItem::default());
            continue;
        }
        if line.starts_with('[') {
            push_agent_acl_env_item(&mut out, current.take());
            continue;
        }

        let Some((raw_key, raw_value)) = line.split_once('=') else {
            continue;
        };
        let key = raw_key.trim();
        let value = raw_value.trim();
        if let Some(item) = current.as_mut() {
            match key {
                "name" => item.name = parse_agent_acl_string(value),
                "value" => item.value = parse_agent_acl_string(value),
                "secret_ref" | "secretRef" => item.secret_ref = parse_agent_acl_string(value),
                "required" => item.required = parse_agent_acl_bool(value),
                _ => {}
            }
        } else if matches!(key, "required_secrets" | "requiredSecrets") {
            out.required_secrets
                .extend(parse_agent_acl_string_array(value));
        }
    }
    push_agent_acl_env_item(&mut out, current.take());
    out
}

fn push_agent_acl_env_item(out: &mut AgentAclRuntimeEnv, item: Option<AgentAclEnvItem>) {
    let Some(item) = item else {
        return;
    };
    let Some(name) = item.name.filter(|value| !value.trim().is_empty()) else {
        return;
    };
    let mut obj = serde_json::Map::new();
    obj.insert("name".to_string(), serde_json::Value::String(name));
    if let Some(value) = item.value.filter(|value| !value.trim().is_empty()) {
        obj.insert("value".to_string(), serde_json::Value::String(value));
    }
    if let Some(secret_ref) = item.secret_ref.filter(|value| !value.trim().is_empty()) {
        obj.insert(
            "secretRef".to_string(),
            serde_json::Value::String(secret_ref.clone()),
        );
        if item.required.unwrap_or(true) {
            out.required_secrets.push(secret_ref);
        }
    }
    obj.insert(
        "required".to_string(),
        serde_json::Value::Bool(item.required.unwrap_or(true)),
    );
    out.env.push(serde_json::Value::Object(obj));
}

fn parse_agent_acl_string(value: &str) -> Option<String> {
    let trimmed = value.trim().trim_end_matches(',').trim();
    if trimmed.is_empty() {
        return None;
    }
    if trimmed.starts_with('"') {
        serde_json::from_str::<String>(trimmed).ok()
    } else {
        Some(trimmed.trim_matches('"').to_string())
    }
}

fn parse_agent_acl_bool(value: &str) -> Option<bool> {
    match value.trim().trim_end_matches(',').trim() {
        "true" => Some(true),
        "false" => Some(false),
        _ => None,
    }
}

fn parse_agent_acl_string_array(value: &str) -> Vec<String> {
    let trimmed = value.trim().trim_end_matches(',').trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Vec::new();
    }
    let inner = &trimmed[1..trimmed.len().saturating_sub(1)];
    inner
        .split(',')
        .filter_map(parse_agent_acl_string)
        .filter(|value| !value.is_empty())
        .collect()
}

fn render_agent_acl_runtime_env(runtime_binding: &serde_json::Value) -> String {
    let env = runtime_binding
        .get("env")
        .and_then(|value| value.as_array())
        .cloned()
        .unwrap_or_default();
    let mut required = std::collections::BTreeSet::<String>::new();
    if let Some(items) = runtime_binding
        .get("requiredSecrets")
        .and_then(|value| value.as_array())
    {
        for item in items {
            if let Some(name) = item.as_str() {
                required.insert(name.to_string());
            }
        }
    }
    if env.is_empty() && required.is_empty() {
        return String::new();
    }

    let mut out = String::new();
    if !required.is_empty() {
        out.push_str("\nrequired_secrets = [");
        for (idx, item) in required.iter().enumerate() {
            if idx > 0 {
                out.push_str(", ");
            }
            out.push_str(&agent_acl_string(item));
        }
        out.push_str("]\n");
    }
    for item in env {
        let Some(name) = item.get("name").and_then(|value| value.as_str()) else {
            continue;
        };
        out.push_str("\n[[runtime.env]]\n");
        out.push_str(&format!("name = {}\n", agent_acl_string(name)));
        if let Some(value) = item.get("value").and_then(|value| value.as_str()) {
            out.push_str(&format!("value = {}\n", agent_acl_string(value)));
        }
        if let Some(secret_ref) = item.get("secretRef").and_then(|value| value.as_str()) {
            out.push_str(&format!("secret_ref = {}\n", agent_acl_string(secret_ref)));
        }
        let required = item
            .get("required")
            .and_then(|value| value.as_bool())
            .unwrap_or(true);
        out.push_str(&format!("required = {required}\n"));
    }
    out
}

fn agent_acl_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_string())
}

fn agent_config_tools(def: &a3s_code_core::subagent::AgentDefinition) -> Vec<serde_json::Value> {
    let mut names = std::collections::BTreeMap::<String, String>::new();
    for rule in def
        .permissions
        .allow
        .iter()
        .chain(def.permissions.ask.iter())
    {
        if let Some(name) = tool_name_from_permission_rule(&rule.rule) {
            names.entry(name.to_ascii_lowercase()).or_insert(name);
        }
    }
    names
        .into_values()
        .map(|name| {
            serde_json::json!({
                "id": format!("builtin:{}", asset_slug(&name)),
                "name": name,
                "type": "builtin",
                "enabled": true,
            })
        })
        .collect()
}

fn tool_name_from_permission_rule(rule: &str) -> Option<String> {
    let name = rule
        .split_once('(')
        .map(|(name, _)| name)
        .unwrap_or(rule)
        .trim();
    (!name.is_empty() && name != "*").then(|| name.to_string())
}

fn http() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .no_proxy()
        .build()
        .map_err(|e| e.to_string())
}

fn items_of(v: &serde_json::Value) -> Vec<serde_json::Value> {
    v.pointer("/data/items")
        .or_else(|| v.pointer("/data"))
        .or_else(|| v.pointer("/items"))
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default()
}

fn response_message(body: &serde_json::Value) -> &str {
    body.get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("request failed")
}

fn envelope_data(body: &serde_json::Value) -> &serde_json::Value {
    body.get("data").unwrap_or(body)
}

fn function_run_failure_message(text: &str) -> Option<String> {
    let body: serde_json::Value = serde_json::from_str(text).ok()?;
    let data = envelope_data(&body);
    let status = json_str_at(
        data,
        &[
            "/status",
            "status",
            "/state",
            "state",
            "/result/status",
            "/result/state",
            "/execution/status",
            "/execution/state",
        ],
    )
    .map(str::to_ascii_lowercase);
    let failed_status = status.as_deref().is_some_and(|status| {
        matches!(
            status,
            "failed" | "failure" | "error" | "errored" | "denied" | "permission_denied"
        )
    });
    let success_false = data
        .get("success")
        .or_else(|| data.pointer("/result/success"))
        .and_then(|value| value.as_bool())
        == Some(false);
    let error = data
        .get("error")
        .or_else(|| data.pointer("/result/error"))
        .or_else(|| data.pointer("/execution/error"))
        .filter(|value| !value.is_null());
    let envelope_failed = envelope_json_is_error(&body);
    if !failed_status && !success_false && error.is_none() && !envelope_failed {
        return None;
    }

    let message = error
        .and_then(json_failure_text)
        .or_else(|| {
            json_str_at(
                data,
                &[
                    "/message",
                    "message",
                    "/errorMessage",
                    "errorMessage",
                    "/result/message",
                    "/result/errorMessage",
                    "/execution/message",
                    "/execution/errorMessage",
                ],
            )
            .map(str::to_string)
        })
        .or_else(|| {
            body.get("message")
                .and_then(|value| value.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "function run failed".to_string());
    Some(message)
}

fn json_failure_text(value: &serde_json::Value) -> Option<String> {
    if let Some(text) = value
        .as_str()
        .map(str::trim)
        .filter(|text| !text.is_empty())
    {
        return Some(text.to_string());
    }
    if let Some(text) = json_str_at(value, &["/message", "message", "/error", "error"]) {
        return Some(text.to_string());
    }
    serde_json::to_string(value).ok()
}

fn json_str_at<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter().find_map(|key| {
        value
            .pointer(key)
            .or_else(|| value.get(*key))
            .and_then(|v| v.as_str())
            .map(str::trim)
            .filter(|s| !s.is_empty())
    })
}

fn agent_asset_ref_from_value(
    asset: &serde_json::Value,
    fallback_name: &str,
) -> Option<AgentAssetRef> {
    let id = json_str_at(asset, &["/id", "id"])?.to_string();
    Some(AgentAssetRef {
        id,
        name: json_str_at(asset, &["/name", "name"])
            .unwrap_or(fallback_name)
            .to_string(),
        owner_name: json_str_at(
            asset,
            &[
                "/ownerName",
                "ownerName",
                "/owner/name",
                "/owner/displayName",
            ],
        )
        .map(str::to_string),
        default_branch: json_str_at(asset, &["/defaultBranch", "defaultBranch"])
            .map(str::to_string),
    })
}

fn asset_category(value: &serde_json::Value) -> Option<&str> {
    json_str_at(
        value,
        &[
            "/category",
            "category",
            "/assetCategory",
            "assetCategory",
            "/assetType",
            "assetType",
            "/asset/category",
            "/metadata/category",
        ],
    )
}

fn category_conflict_error(name: &str, actual: &str, expected: &str) -> String {
    format!("asset `{name}` already exists with category={actual}; expected {expected}")
}

fn find_agent_asset(
    found: &serde_json::Value,
    name: &str,
    kind: AgentOsKind,
) -> Result<Option<AgentAssetRef>, String> {
    let exact = items_of(found)
        .into_iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name));
    let Some(asset) = exact else {
        return Ok(None);
    };
    if let Some(actual) = asset_category(&asset) {
        if !actual.eq_ignore_ascii_case("agent") {
            return Err(category_conflict_error(name, actual, "agent"));
        }
    }
    let actual_kind = asset.get("agentKind").and_then(|v| v.as_str());
    if actual_kind.is_some_and(|actual| actual != kind.agent_kind()) {
        return Err(format!(
            "asset `{name}` already exists with agentKind={}; expected {}",
            actual_kind.unwrap_or("unknown"),
            kind.agent_kind()
        ));
    }
    agent_asset_ref_from_value(&asset, name)
        .map(Some)
        .ok_or_else(|| format!("asset `{name}` matched but had no id"))
}

async fn fetch_agent_asset(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    fallback_name: &str,
) -> Result<AgentAssetRef, String> {
    let resp = client
        .get(format!(
            "{}/api/v1/assets/{}",
            origin.trim_end_matches('/'),
            path_segment(asset_id)
        ))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() || envelope_json_is_error(&body) {
        return Err(format!(
            "fetch agent asset failed ({status}): {}",
            response_message(&body)
        ));
    }
    agent_asset_ref_from_value(envelope_data(&body), fallback_name)
        .ok_or_else(|| "fetch agent asset: no id in response".to_string())
}

async fn ensure_agent_asset(
    origin: &str,
    token: &str,
    kind: AgentOsKind,
    name: &str,
    description: &str,
    agent_contract: &serde_json::Value,
) -> Result<AgentAssetRef, String> {
    let client = http()?;
    if let Some(asset) = lookup_agent_asset(&client, origin, token, kind, name).await? {
        return Ok(asset);
    }
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let mut metadata = serde_json::json!({
        "service": kind.service_label(),
        "agentKind": kind.agent_kind(),
        "runtimeKind": kind.runtime_kind(),
        "createdBy": "a3s-code-tui",
        "agentContract": agent_contract,
    });
    if let Some(protocol) = kind.runtime_protocol() {
        metadata["protocol"] = serde_json::json!(protocol);
    }
    let resp = client
        .post(&base)
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "ownerType": "user",
            "category": "agent",
            "agentKind": kind.agent_kind(),
            "visibility": "private",
            "description": description,
            "metadata": metadata,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "create agent asset failed ({status}): {}",
            response_message(&body)
        ));
    }
    let asset = agent_asset_ref_from_value(envelope_data(&body), name)
        .ok_or_else(|| "create agent asset: no id in response".to_string())?;
    if asset.owner_name.is_some() && asset.default_branch.is_some() {
        Ok(asset)
    } else {
        fetch_agent_asset(&client, origin, token, &asset.id, name).await
    }
}

async fn sync_agent_contract_metadata(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    kind: AgentOsKind,
    agent_contract: &serde_json::Value,
) -> Result<(), String> {
    let mut metadata = serde_json::json!({
        "service": kind.service_label(),
        "agentKind": kind.agent_kind(),
        "runtimeKind": kind.runtime_kind(),
        "updatedBy": "a3s-code-tui",
        "agentContract": agent_contract,
    });
    if let Some(protocol) = kind.runtime_protocol() {
        metadata["protocol"] = serde_json::json!(protocol);
    }
    let resp = client
        .patch(format!(
            "{}/api/v1/assets/{}",
            origin.trim_end_matches('/'),
            path_segment(asset_id)
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({ "metadata": metadata }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if status.is_success() && !envelope_json_is_error(&body) {
        Ok(())
    } else {
        Err(format!(
            "sync agent contract metadata failed ({status}): {}",
            response_message(&body)
        ))
    }
}

async fn lookup_agent_asset(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    kind: AgentOsKind,
    name: &str,
) -> Result<Option<AgentAssetRef>, String> {
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", name),
            ("category", "agent"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(asset) = find_agent_asset(&found, name, kind)? {
        if asset.owner_name.is_some() && asset.default_branch.is_some() {
            return Ok(Some(asset));
        }
        return fetch_agent_asset(client, origin, token, &asset.id, name)
            .await
            .map(Some);
    }
    Ok(None)
}

#[derive(Clone, Debug)]
pub(crate) struct AgentRepositoryFile {
    pub(crate) path: String,
    pub(crate) bytes: Vec<u8>,
}

#[allow(clippy::too_many_arguments)]
pub(crate) async fn upload_agent_definition(
    origin: &str,
    token: &str,
    asset_id: &str,
    package_files: Vec<AgentRepositoryFile>,
    asset_acl: &str,
    _manifest: &serde_json::Value,
    _config: &serde_json::Value,
    _runtime_binding: &serde_json::Value,
) -> Result<(), String> {
    use base64::Engine;

    let mut files = package_files
        .into_iter()
        .map(|file| {
            serde_json::json!({
                "path": file.path,
                "contentBase64": base64::engine::general_purpose::STANDARD.encode(file.bytes),
            })
        })
        .collect::<Vec<_>>();
    files.push(serde_json::json!({
        "path": asset_lifecycle::ASSET_ACL_PATH,
        "contentBase64": base64::engine::general_purpose::STANDARD.encode(asset_acl.as_bytes()),
    }));
    let resp = http()?
        .post(format!(
            "{}/api/v1/assets/{}/repository/files",
            origin.trim_end_matches('/'),
            path_segment(asset_id)
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "overwrite": true,
            "message": "a3s code /agent: update agent package",
            "files": files,
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    if status.is_success() {
        return Ok(());
    }
    let body = resp.text().await.unwrap_or_default();
    Err(format!(
        "upload agent package failed ({status}): {}",
        truncate(&body, 200)
    ))
}

fn collect_agent_package_files(dev: &AgentDevSession) -> Result<Vec<AgentRepositoryFile>, String> {
    validate_agent_framework_package(dev)?;

    let package_source_path = agent_package_source_path(&dev.rel);
    let mut files = Vec::new();
    collect_agent_package_dir(
        &dev.package_path,
        &dev.package_path,
        &package_source_path,
        &mut files,
    )?;
    if !files
        .iter()
        .any(|file| file.path == agent_asset_source_path(&dev.rel, &dev.definition_rel))
    {
        return Err(format!(
            "agent package {} does not contain entrypoint {}",
            dev.package_path.display(),
            dev.path.display()
        ));
    }
    Ok(files)
}

const REQUIRED_AGENT_PROJECT_FILES: &[&str] = &[
    "README.md",
    "agent.md",
    "prompts/system.md",
    "workflows/operating-procedure.md",
    "examples/example-input.md",
    "examples/example-output.md",
    "eval/smoke.md",
    "tests/smoke.md",
];

fn validate_agent_framework_package(dev: &AgentDevSession) -> Result<(), String> {
    if dev.package_path.is_file() {
        return Err(format!(
            "{} is a single agent definition file. Agent assets must be complete A3S Code autonomous-agent project directories with visible source files: {}.",
            dev.path.display(),
            REQUIRED_AGENT_PROJECT_FILES.join(", ")
        ));
    }
    if dev.definition_rel != "agent.md" {
        return Err(format!(
            "{} uses `{}` as its entrypoint. Agent assets must use a visible `agent.md` entrypoint inside a complete A3S Code project package.",
            dev.package_path.display(),
            dev.definition_rel
        ));
    }

    let missing = REQUIRED_AGENT_PROJECT_FILES
        .iter()
        .copied()
        .filter(|rel| !dev.package_path.join(rel).is_file())
        .collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(format!(
            "{} is not a complete A3S Code autonomous-agent project; missing visible source file(s): {}.",
            dev.package_path.display(),
            missing.join(", ")
        ));
    }

    let metadata_dir = dev.package_path.join(".a3s");
    if metadata_dir.is_dir() {
        for entry in std::fs::read_dir(&metadata_dir)
            .map_err(|e| format!("could not read {}: {e}", metadata_dir.display()))?
        {
            let entry = entry
                .map_err(|e| format!("could not read {} entry: {e}", metadata_dir.display()))?;
            let name = entry.file_name().to_string_lossy().to_string();
            if name != "asset.acl" {
                return Err(format!(
                    "{} must be metadata-only; unexpected `.a3s/{name}`. Keep agent source in visible package files and configuration in `.a3s/asset.acl`.",
                    dev.package_path.display()
                ));
            }
        }
    }
    Ok(())
}

fn collect_agent_package_dir(
    base: &std::path::Path,
    dir: &std::path::Path,
    package_source_path: &str,
    out: &mut Vec<AgentRepositoryFile>,
) -> Result<(), String> {
    let mut entries = std::fs::read_dir(dir)
        .map_err(|e| format!("could not read package directory {}: {e}", dir.display()))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let file_type = match entry.file_type() {
            Ok(file_type) => file_type,
            Err(_) => continue,
        };
        if should_skip_agent_package_entry(&name, file_type.is_dir()) {
            continue;
        }
        if file_type.is_dir() {
            collect_agent_package_dir(base, &path, package_source_path, out)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let rel = normalized_rel(base, &path);
        let bytes =
            std::fs::read(&path).map_err(|e| format!("could not read {}: {e}", path.display()))?;
        out.push(AgentRepositoryFile {
            path: if package_source_path == "." {
                rel.replace('\\', "/")
            } else {
                format!("{package_source_path}/{}", rel.replace('\\', "/"))
            },
            bytes,
        });
    }
    Ok(())
}

fn should_skip_agent_package_entry(name: &str, is_dir: bool) -> bool {
    if matches!(
        name,
        ".DS_Store"
            | ".git"
            | ".hg"
            | ".svn"
            | "agent.asset.json"
            | "agent.config.json"
            | "agent.runtime-binding.json"
            | "runtime-binding.json"
    ) {
        return true;
    }
    if is_dir && name == ".a3s" {
        return true;
    }
    is_dir
        && matches!(
            name,
            "target" | "node_modules" | "dist" | "build" | ".cache" | ".next"
        )
}

pub(crate) async fn sync_agent_config(
    origin: &str,
    token: &str,
    asset_id: &str,
    config: &serde_json::Value,
) -> Result<bool, String> {
    let client = http()?;
    let base = format!(
        "{}/api/v1/assets/{}/agent-config",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    );
    let mut validation = config.clone();
    if let serde_json::Value::Object(map) = &mut validation {
        map.insert("mode".to_string(), serde_json::json!("replace"));
    }
    let validate_resp = client
        .post(format!("{base}/validate"))
        .bearer_auth(token)
        .json(&validation)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let validate_status = validate_resp.status();
    let validate_text = validate_resp.text().await.unwrap_or_default();
    let validation_supported = !matches!(validate_status.as_u16(), 404 | 405);
    if validation_supported {
        if !validate_status.is_success() {
            return Err(format!(
                "OS agent config validation failed ({validate_status})"
            ));
        }
        let validate_json: serde_json::Value =
            serde_json::from_str(&validate_text).map_err(|e| e.to_string())?;
        if envelope_json_is_error(&validate_json) {
            return Err("OS agent config validation failed".to_string());
        }
        if validate_json
            .pointer("/data/valid")
            .and_then(|v| v.as_bool())
            == Some(false)
        {
            let diagnostics = validate_json
                .pointer("/data/diagnostics")
                .map(|v| truncate(&v.to_string(), 180))
                .unwrap_or_else(|| "no diagnostics".to_string());
            return Err(format!(
                "OS agent config validation reported invalid config: {diagnostics}"
            ));
        }
    }

    let put_resp = client
        .put(&base)
        .bearer_auth(token)
        .json(config)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let put_status = put_resp.status();
    let put_text = put_resp.text().await.unwrap_or_default();
    if matches!(put_status.as_u16(), 404 | 405) {
        return Ok(false);
    }
    if !put_status.is_success() {
        return Err(format!("OS agent config sync failed ({put_status})"));
    }
    if serde_json::from_str::<serde_json::Value>(&put_text)
        .ok()
        .is_some_and(|value| envelope_json_is_error(&value))
    {
        return Err("OS agent config sync failed".to_string());
    }
    Ok(true)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AgentRuntimeBindingSync {
    Synced,
    Unsupported,
    Failed(String),
}

pub(crate) async fn sync_agent_runtime_binding(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> AgentRuntimeBindingSync {
    match sync_agent_runtime_binding_inner(origin, token, asset_id, runtime_binding).await {
        Ok(true) => AgentRuntimeBindingSync::Synced,
        Ok(false) => AgentRuntimeBindingSync::Unsupported,
        Err(err) => AgentRuntimeBindingSync::Failed(err),
    }
}

async fn sync_agent_runtime_binding_inner(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> Result<bool, String> {
    let client = http()?;
    let body = agent_runtime_binding_upsert_body(runtime_binding);
    let base = format!(
        "{}/api/v1/assets/{}/runtime-binding",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    );
    let put_resp = client
        .put(&base)
        .bearer_auth(token)
        .json(&body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let put_status = put_resp.status();
    let put_text = put_resp.text().await.unwrap_or_default();
    if matches!(put_status.as_u16(), 404 | 405) {
        return Ok(false);
    }
    if !put_status.is_success() {
        return Err(format!("OS runtime-binding sync failed ({put_status})"));
    }
    if serde_json::from_str::<serde_json::Value>(&put_text)
        .ok()
        .is_some_and(|value| envelope_json_is_error(&value))
    {
        return Err("OS runtime-binding sync failed".to_string());
    }

    let validate_resp = client
        .post(format!("{base}/validate"))
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let validate_status = validate_resp.status();
    let validate_text = validate_resp.text().await.unwrap_or_default();
    if matches!(validate_status.as_u16(), 404 | 405) {
        return Ok(true);
    }
    if !validate_status.is_success() {
        return Err(format!(
            "OS runtime-binding validation failed ({validate_status})"
        ));
    }
    let validate_json: serde_json::Value =
        serde_json::from_str(&validate_text).map_err(|e| e.to_string())?;
    if envelope_json_is_error(&validate_json) {
        return Err("OS runtime-binding validation failed".to_string());
    }
    if validate_json
        .pointer("/data/valid")
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        let issues = validate_json
            .pointer("/data/issues")
            .map(|value| truncate(&value.to_string(), 180))
            .unwrap_or_else(|| "no issues".to_string());
        return Err(format!(
            "OS runtime-binding validation reported invalid binding: {issues}"
        ));
    }
    Ok(true)
}

fn agent_runtime_binding_upsert_body(runtime_binding: &serde_json::Value) -> serde_json::Value {
    let target_ref = runtime_binding
        .pointer("/target/ref")
        .and_then(|value| value.as_str())
        .unwrap_or("main");
    let isolation = runtime_binding
        .get("isolation")
        .and_then(|value| value.as_str())
        .unwrap_or("container");
    let runtime_kind = runtime_binding
        .pointer("/runtime/kind")
        .and_then(|value| value.as_str())
        .unwrap_or("a3s-agent-service");
    let mut runtime = runtime_binding
        .get("runtime")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| {
            serde_json::json!({
                "kind": runtime_kind,
            })
        });
    if let serde_json::Value::Object(map) = &mut runtime {
        map.remove("mode");
        map.remove("agentKind");
        map.remove("protocol");
        if isolation == "serving" && !map.contains_key("sharedRuntime") {
            map.insert("sharedRuntime".to_string(), serde_json::json!("node-20"));
        }
        if isolation == "container"
            && !map.contains_key("image")
            && !map.contains_key("command")
            && !map.contains_key("entrypoint")
        {
            map.insert("command".to_string(), serde_json::json!(runtime_kind));
        }
    }
    let env = runtime_binding
        .get("env")
        .filter(|value| value.is_array())
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let required_secrets = runtime_binding
        .get("requiredSecrets")
        .filter(|value| value.is_array())
        .cloned()
        .unwrap_or_else(|| serde_json::json!([]));
    let resources = runtime_binding
        .get("resources")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));
    let metadata = runtime_binding
        .get("metadata")
        .filter(|value| value.is_object())
        .cloned()
        .unwrap_or_else(|| serde_json::json!({}));

    serde_json::json!({
        "kind": runtime_binding
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or("agent"),
        "isolation": isolation,
        "target": {
            "kind": "asset",
            "ref": target_ref,
        },
        "runtime": runtime,
        "env": env,
        "requiredSecrets": required_secrets,
        "resources": resources,
        "network": {},
        "enabled": runtime_binding
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        "metadata": metadata,
    })
}

async fn agent_config_validation_status(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    config: &serde_json::Value,
) -> String {
    let base = format!(
        "{}/api/v1/assets/{}/agent-config/validate",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    );
    let mut validation = config.clone();
    if let serde_json::Value::Object(map) = &mut validation {
        map.insert("mode".to_string(), serde_json::json!("replace"));
    }
    let resp = match client
        .post(base)
        .bearer_auth(token)
        .json(&validation)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            return format!(
                "agent-config check failed: {}",
                truncate(&err.to_string(), 120)
            );
        }
    };
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if matches!(status.as_u16(), 404 | 405) {
        return "agent-config endpoint unavailable".to_string();
    }
    if !status.is_success() {
        return format!("agent-config validation failed ({status})");
    }
    let Ok(body) = serde_json::from_str::<serde_json::Value>(&text) else {
        return "agent-config validation returned unreadable JSON".to_string();
    };
    if envelope_json_is_error(&body) {
        return "agent-config validation failed".to_string();
    }
    if body
        .pointer("/data/valid")
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        let diagnostics = body
            .pointer("/data/diagnostics")
            .or_else(|| body.pointer("/data/issues"))
            .map(|value| truncate(&value.to_string(), 140))
            .unwrap_or_else(|| "no diagnostics".to_string());
        return format!("agent-config invalid: {diagnostics}");
    }
    "agent-config valid".to_string()
}

async fn runtime_binding_validation_status(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
) -> String {
    let base = format!(
        "{}/api/v1/assets/{}/runtime-binding",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    );
    let resp = match client.get(&base).bearer_auth(token).send().await {
        Ok(resp) => resp,
        Err(err) => {
            return format!(
                "runtime-binding check failed: {}",
                truncate(&err.to_string(), 120)
            );
        }
    };
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if matches!(status.as_u16(), 404 | 405) {
        return "runtime-binding endpoint unavailable".to_string();
    }
    if !status.is_success() {
        return format!("runtime-binding read failed ({status})");
    }
    let Ok(body) = serde_json::from_str::<serde_json::Value>(&text) else {
        return "runtime-binding read returned unreadable JSON".to_string();
    };
    if envelope_json_is_error(&body) {
        return "runtime-binding read failed".to_string();
    }
    if envelope_data(&body)
        .get("configured")
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        return "runtime-binding missing".to_string();
    }

    let resp = match client
        .post(format!("{base}/validate"))
        .bearer_auth(token)
        .send()
        .await
    {
        Ok(resp) => resp,
        Err(err) => {
            return format!(
                "runtime-binding validation failed: {}",
                truncate(&err.to_string(), 120)
            );
        }
    };
    let status = resp.status();
    let text = resp.text().await.unwrap_or_default();
    if matches!(status.as_u16(), 404 | 405) {
        return "runtime-binding saved; validation endpoint unavailable".to_string();
    }
    if !status.is_success() {
        return format!("runtime-binding validation failed ({status})");
    }
    let Ok(body) = serde_json::from_str::<serde_json::Value>(&text) else {
        return "runtime-binding validation returned unreadable JSON".to_string();
    };
    if envelope_json_is_error(&body) {
        return "runtime-binding validation failed".to_string();
    }
    if body
        .pointer("/data/valid")
        .and_then(|value| value.as_bool())
        == Some(false)
    {
        let issues = body
            .pointer("/data/issues")
            .map(|value| truncate(&value.to_string(), 140))
            .unwrap_or_else(|| "no issues".to_string());
        return format!("runtime-binding invalid: {issues}");
    }
    "runtime-binding valid".to_string()
}

async fn try_application_agent_build(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset: &AgentAssetRef,
) -> Option<(remote_ui::ViewSpec, String)> {
    let owner = asset.owner_name.as_deref()?;
    let commit = latest_agent_commit(
        client,
        origin,
        token,
        &asset.id,
        asset.default_branch.as_deref(),
    )
    .await?;
    let build_number = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(1);
    let resp = client
        .post(format!(
            "{}/api/v1/assets/{}/{}/build/agent",
            origin.trim_end_matches('/'),
            path_segment(owner),
            path_segment(&asset.name)
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "commitSha": commit.sha.as_str(),
            "branch": commit.branch.as_str(),
            "buildNumber": build_number,
        }))
        .send()
        .await
        .ok()?;
    let status = resp.status();
    let text = resp.text().await.ok()?;
    if !status.is_success() {
        return None;
    }
    let body = serde_json::from_str::<serde_json::Value>(&text).ok();
    if body.as_ref().is_some_and(envelope_json_is_error) {
        return None;
    }
    let data = body
        .as_ref()
        .map(envelope_data)
        .unwrap_or(&serde_json::Value::Null);
    let success = data
        .get("success")
        .and_then(|value| value.as_bool())
        .unwrap_or(true);
    let version = json_str_at(data, &["/version", "version"]).unwrap_or("pending");
    let package_ref = json_str_at(data, &["/repository", "repository"]).unwrap_or("agent package");
    let view = remote_ui::find_view_url(&text, Some(origin)).unwrap_or_else(|| {
        agent_view_spec(format!(
            "{}/admin/kernel/assets?focus=1",
            origin.trim_end_matches('/')
        ))
    });
    let note = if success {
        if let (Some(package_id), Some(package_version)) = (
            json_str_at(
                data,
                &["/repository", "repository", "/packageId", "packageId"],
            ),
            json_str_at(
                data,
                &["/version", "version", "/packageVersion", "packageVersion"],
            ),
        ) {
            if let Some(launched) = try_launch_application_agent(
                client,
                origin,
                token,
                asset,
                package_id,
                package_version,
            )
            .await
            {
                return Some(launched);
            }
        }
        format!(
            "OS Agent as a Service triggered the application-agent build for `{}` at {} on `{}`. Package `{package_ref}` version `{version}` is the launch input; open OS to choose a namespace and launch.",
            asset.name, commit.sha, commit.branch
        )
    } else {
        let error = json_str_at(data, &["/error", "error"]).unwrap_or("no error detail");
        format!(
            "OS Agent as a Service accepted the application-agent build request, but the build reported failure: {error}."
        )
    };
    Some((view, note))
}

async fn try_launch_application_agent(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset: &AgentAssetRef,
    package_id: &str,
    version: &str,
) -> Option<(remote_ui::ViewSpec, String)> {
    let namespace = select_runtime_namespace(client, origin, token).await?;
    let resp = client
        .post(format!(
            "{}/api/v1/runtimes/namespaces/{}/agents/launch",
            origin.trim_end_matches('/'),
            path_segment(&namespace.id)
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "packageId": package_id,
            "version": version,
            "name": asset_slug(&asset.name),
            "replicas": 1,
        }))
        .send()
        .await
        .ok()?;
    let status = resp.status();
    let text = resp.text().await.ok()?;
    if !status.is_success() {
        return None;
    }
    let body = serde_json::from_str::<serde_json::Value>(&text).ok();
    if body.as_ref().is_some_and(envelope_json_is_error) {
        return None;
    }
    let data = body
        .as_ref()
        .map(envelope_data)
        .unwrap_or(&serde_json::Value::Null);
    let deployment = json_str_at(data, &["/deploymentId", "deploymentId", "/name", "name"])
        .unwrap_or("agent deployment");
    let runtime_status = json_str_at(data, &["/status", "status"]).unwrap_or("created");
    let view = remote_ui::find_view_url(&text, Some(origin)).unwrap_or_else(|| {
        agent_view_spec(format!(
            "{}/admin/kernel/processes?focus=1",
            origin.trim_end_matches('/')
        ))
    });
    Some((
        view,
        format!(
            "OS Agent as a Service built and launched `{}` in namespace `{}` as `{deployment}` ({runtime_status}).",
            asset.name, namespace.name
        ),
    ))
}

async fn select_runtime_namespace(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
) -> Option<AgentNamespaceRef> {
    let value = get_json(
        client,
        token,
        &format!(
            "{}/api/v1/runtimes/namespaces?limit=100",
            origin.trim_end_matches('/')
        ),
    )
    .await?;
    let namespaces = items_of(&value)
        .into_iter()
        .filter_map(|item| namespace_ref_from_value(&item))
        .collect::<Vec<_>>();
    namespaces
        .iter()
        .find(|namespace| namespace_is_default(&value, &namespace.id))
        .cloned()
        .or_else(|| {
            namespaces
                .iter()
                .find(|namespace| namespace.name == "default" || namespace.id == "default")
                .cloned()
        })
        .or_else(|| namespaces.into_iter().next())
}

fn namespace_ref_from_value(value: &serde_json::Value) -> Option<AgentNamespaceRef> {
    let id = json_str_at(value, &["/id", "id"])?.to_string();
    Some(AgentNamespaceRef {
        id,
        name: json_str_at(value, &["/name", "name", "/displayName", "displayName"])
            .unwrap_or("default")
            .to_string(),
    })
}

fn namespace_is_default(value: &serde_json::Value, namespace_id: &str) -> bool {
    items_of(value).into_iter().any(|item| {
        json_str_at(&item, &["/id", "id"]) == Some(namespace_id)
            && item
                .get("isDefault")
                .and_then(|flag| flag.as_bool())
                .unwrap_or(false)
    })
}

async fn latest_agent_commit(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    preferred_branch: Option<&str>,
) -> Option<AgentCommitRef> {
    let branch = preferred_branch
        .map(str::trim)
        .filter(|branch| !branch.is_empty())
        .unwrap_or("main");
    if let Some(commit) = fetch_agent_branch_commit(client, origin, token, asset_id, branch).await {
        return Some(commit);
    }
    if let Some(commit) = fetch_agent_branches_commit(client, origin, token, asset_id, branch).await
    {
        return Some(commit);
    }
    fetch_agent_latest_commit(client, origin, token, asset_id, branch).await
}

async fn fetch_agent_branch_commit(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    branch: &str,
) -> Option<AgentCommitRef> {
    let value = get_json(
        client,
        token,
        &format!(
            "{}/api/v1/assets/{}/branches/{}",
            origin.trim_end_matches('/'),
            path_segment(asset_id),
            path_segment(branch)
        ),
    )
    .await?;
    let data = envelope_data(&value);
    Some(AgentCommitRef {
        sha: json_str_at(data, &["/commitSha", "commitSha", "/sha", "sha"])?.to_string(),
        branch: json_str_at(data, &["/name", "name"])
            .unwrap_or(branch)
            .to_string(),
    })
}

async fn fetch_agent_branches_commit(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    preferred_branch: &str,
) -> Option<AgentCommitRef> {
    let value = get_json(
        client,
        token,
        &format!(
            "{}/api/v1/assets/{}/branches",
            origin.trim_end_matches('/'),
            path_segment(asset_id)
        ),
    )
    .await?;
    let branches = items_of(&value);
    let selected = branches
        .iter()
        .find(|branch| json_str_at(branch, &["/name", "name"]) == Some(preferred_branch))
        .or_else(|| {
            branches
                .iter()
                .find(|branch| json_str_at(branch, &["/commitSha", "commitSha"]).is_some())
        })?;
    Some(AgentCommitRef {
        sha: json_str_at(selected, &["/commitSha", "commitSha"])?.to_string(),
        branch: json_str_at(selected, &["/name", "name"])
            .unwrap_or(preferred_branch)
            .to_string(),
    })
}

async fn fetch_agent_latest_commit(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    branch: &str,
) -> Option<AgentCommitRef> {
    let value = get_json(
        client,
        token,
        &format!(
            "{}/api/v1/assets/{}/commits?limit=1",
            origin.trim_end_matches('/'),
            path_segment(asset_id)
        ),
    )
    .await?;
    let commit = items_of(&value).into_iter().next()?;
    Some(AgentCommitRef {
        sha: json_str_at(&commit, &["/sha", "sha", "/commitSha", "commitSha"])?.to_string(),
        branch: branch.to_string(),
    })
}

async fn get_json(client: &reqwest::Client, token: &str, url: &str) -> Option<serde_json::Value> {
    let resp = client.get(url).bearer_auth(token).send().await.ok()?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.ok()?;
    if !status.is_success() || envelope_json_is_error(&body) {
        return None;
    }
    Some(body)
}

async fn try_agent_operation(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    action: AgentOsAction,
) -> Option<(remote_ui::ViewSpec, String)> {
    if matches!(action, AgentOsAction::Run(AgentOsKind::Tool)) {
        return try_agent_rest_operation(client, origin, token, asset_id, action).await;
    }
    if let Some(result) =
        try_agent_capability_operation(client, origin, token, asset_id, action).await
    {
        return Some(result);
    }
    try_agent_rest_operation(client, origin, token, asset_id, action).await
}

async fn try_agent_capability_operation(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    action: AgentOsAction,
) -> Option<(remote_ui::ViewSpec, String)> {
    let operation = match action {
        AgentOsAction::Run(_) => "run",
        AgentOsAction::Deploy => "deploy",
        _ => return None,
    };
    let query = match action {
        AgentOsAction::Run(AgentOsKind::Tool) => "Function as a Service run tool agent asset",
        AgentOsAction::Run(AgentOsKind::Application) => {
            "Agent as a Service run application agent asset"
        }
        AgentOsAction::Run(AgentOsKind::Agentic) => "Agent as a Service run agentic agent asset",
        AgentOsAction::Deploy => "Agent as a Service application agent build launch deploy asset",
        _ => return None,
    };
    let candidates =
        os_progressive::search_operations(client, origin, token, query, |text, operation| {
            agent_progressive_score(text, operation, action)
        })
        .await?;
    for candidate in candidates.into_iter().take(4) {
        let described = os_progressive::describe_operation(client, origin, token, &candidate).await;
        let described_view = described
            .as_ref()
            .and_then(|value| view_spec_from_json(value, origin));
        if matches!(action, AgentOsAction::Deploy)
            && described
                .as_ref()
                .is_some_and(capability_requires_application_deploy_metadata)
        {
            return Some((
                described_view
                    .unwrap_or_else(|| agent_view_spec(agent_asset_url(origin, asset_id))),
                "OS Agent as a Service found the application-agent deploy path; build/package/namespace metadata is required before launch, so the OS asset view was opened."
                    .to_string(),
            ));
        }
        let params = agent_capability_params(described.as_ref(), asset_id, action.target_kind());
        if let Some(execution) = os_progressive::execute_operation(
            client,
            origin,
            token,
            &candidate,
            params.clone(),
            described_view.clone(),
        )
        .await
        {
            if let Some(spec) = execution.view {
                return Some((
                    spec,
                    format!(
                        "OS {} accepted the {operation} request through progressive capabilities.",
                        action.target_kind().service_label()
                    ),
                ));
            }
            return Some((
                agent_view_spec(agent_asset_url(origin, asset_id)),
                format!(
                    "OS {} accepted the {operation} request through progressive capabilities; no run-specific RemoteUI view was returned.",
                    action.target_kind().service_label()
                ),
            ));
        }
        if let Some(result) = try_direct_capability_operation(
            client,
            origin,
            token,
            &candidate,
            &params,
            described_view.clone(),
            asset_id,
            operation,
        )
        .await
        {
            return Some(result);
        }
        if matches!(action, AgentOsAction::Deploy)
            && is_application_deploy_planning_operation(&candidate.operation)
        {
            return Some((
                described_view
                    .unwrap_or_else(|| agent_view_spec(agent_asset_url(origin, asset_id))),
                "OS Agent as a Service found the application-agent deploy path; build/package/namespace metadata is required before launch, so the OS asset view was opened."
                    .to_string(),
            ));
        }
    }
    None
}

fn view_spec_from_json(value: &serde_json::Value, origin: &str) -> Option<remote_ui::ViewSpec> {
    let text = serde_json::to_string(value).ok()?;
    remote_ui::find_view_url(&text, Some(origin))
}

fn capability_requires_application_deploy_metadata(value: &serde_json::Value) -> bool {
    let Some(reference) = capability_schema_ref(value) else {
        return false;
    };
    [
        "TriggerAgentBuildRequestDto",
        "LaunchAgentRequestDto",
        "EvaluateAssetReleaseGateRequestDto",
    ]
    .iter()
    .any(|schema| reference.contains(schema))
}

#[allow(clippy::too_many_arguments)]
async fn try_direct_capability_operation(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    candidate: &os_progressive::ProgressiveOperation,
    params: &serde_json::Value,
    described_view: Option<remote_ui::ViewSpec>,
    asset_id: &str,
    label: &str,
) -> Option<(remote_ui::ViewSpec, String)> {
    if candidate.method.as_deref() != Some("POST") {
        return None;
    }
    let path = candidate.path.as_deref()?;
    if path.contains('{') || !path.starts_with('/') {
        return None;
    }
    let resp = client
        .post(format!("{}{}", origin.trim_end_matches('/'), path))
        .bearer_auth(token)
        .json(params)
        .send()
        .await
        .ok()?;
    let status = resp.status();
    if !status.is_success() {
        return None;
    }
    let is_stream = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|value| value.contains("text/event-stream"));
    if is_stream {
        return Some((
            described_view.unwrap_or_else(|| {
                agent_view_spec(format!(
                    "{}/admin/kernel/processes?focus=1",
                    origin.trim_end_matches('/')
                ))
            }),
            format!(
                "OS Agent as a Service accepted the {label} request through its discovered streaming endpoint."
            ),
        ));
    }
    let text = resp.text().await.ok()?;
    Some((
        remote_ui::find_view_url(&text, Some(origin))
            .or(described_view)
            .unwrap_or_else(|| agent_view_spec(agent_asset_url(origin, asset_id))),
        format!(
            "OS Agent as a Service accepted the {label} request through its discovered endpoint."
        ),
    ))
}

fn envelope_json_is_error(value: &serde_json::Value) -> bool {
    let code = value
        .get("code")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("statusCode").and_then(|v| v.as_i64()))
        .unwrap_or(200);
    code >= 400
}

fn agent_progressive_score(text: &str, operation: &str, action: AgentOsAction) -> i32 {
    let combined = format!("{text} {operation}").to_ascii_lowercase();
    if matches!(action, AgentOsAction::Open(_) | AgentOsAction::Logs(_))
        && is_mutating_agent_observe_operation(operation)
    {
        return 0;
    }
    let action_hit = match action {
        AgentOsAction::Open(_) => {
            combined.contains("open")
                || combined.contains("view")
                || combined.contains("remoteui")
                || combined.contains("manage")
                || combined.contains("asset view")
        }
        AgentOsAction::Logs(_) => {
            combined.contains("log")
                || combined.contains("trace")
                || combined.contains("job")
                || combined.contains("process")
                || combined.contains("observability")
        }
        _ => true,
    };
    if !action_hit {
        return 0;
    }
    if matches!(
        action,
        AgentOsAction::Open(AgentOsKind::Tool) | AgentOsAction::Logs(AgentOsKind::Tool)
    ) && !(combined.contains("function") || combined.contains("faas"))
    {
        return 0;
    }
    match action {
        AgentOsAction::Run(AgentOsKind::Tool) => {
            if !(combined.contains("function") || combined.contains("faas")) {
                return 0;
            }
        }
        AgentOsAction::Run(AgentOsKind::Agentic | AgentOsKind::Application)
            if combined.contains("function as a service") || combined.contains("faas") =>
        {
            return 0;
        }
        _ => {}
    }
    let score = capability_operation_score(text, operation, action);
    if score >= 8 {
        score
    } else {
        0
    }
}

fn is_mutating_agent_observe_operation(operation: &str) -> bool {
    let operation = operation.to_ascii_lowercase();
    let has_safe_observe_hint = operation.contains("open")
        || operation.contains("view")
        || operation.contains("get")
        || operation.contains("list")
        || operation.contains("inspect")
        || operation.contains("log");
    let has_mutating_hint = operation.contains("create")
        || operation.contains("update")
        || operation.contains("delete")
        || operation.contains("apply")
        || operation.contains("publish")
        || operation.contains("deploy")
        || operation.contains("build")
        || operation.contains("launch")
        || operation.contains("run")
        || operation.contains("trigger")
        || operation.contains("batch")
        || operation.contains("validate")
        || operation.contains("acknowledge");
    has_mutating_hint && !has_safe_observe_hint
}

fn capability_operation_score(text: &str, operation: &str, action: AgentOsAction) -> i32 {
    let mut score = 0;
    let operation = operation.to_ascii_lowercase();
    let combined = format!("{text} {operation}").to_ascii_lowercase();
    if combined.contains("agent") {
        score += 4;
    }
    if combined.contains("agent as a service") || combined.contains("aaas") {
        score += 6;
    }
    match action {
        AgentOsAction::Run(kind) => {
            if !combined.contains("run") {
                return 0;
            }
            score += 8;
            match kind {
                AgentOsKind::Tool => {
                    if combined.contains("function") || combined.contains("faas") {
                        score += 7;
                    }
                    if combined.contains("tool") {
                        score += 4;
                    }
                }
                AgentOsKind::Application => {
                    if combined.contains("application") {
                        score += 4;
                    }
                }
                AgentOsKind::Agentic => {
                    if combined.contains("agentic") {
                        score += 3;
                    }
                }
            }
        }
        AgentOsAction::Deploy => {
            if combined.contains("deploy") || combined.contains("deployment") {
                score += 8;
            }
            if combined.contains("build") || combined.contains("release-gate") {
                score += 10;
            }
            if combined.contains("deployability") {
                score += 8;
            }
            if combined.contains("launch") {
                score += 7;
            }
            if combined.contains("asset") {
                score += 3;
            }
            if combined.contains("application") || combined.contains("app") {
                score += 3;
            }
            if operation.contains("agentbuildcontroller_triggeragentbuild") {
                score += 24;
            }
            if operation.contains("agentruntimecontroller_launchagent") {
                score += 16;
            }
            if operation.contains("assetreleasegatecontroller_evaluate") {
                score += 10;
            }
            if operation.contains("assetdeployabilitycontroller_evaluate") {
                score += 6;
            }
            if combined.contains("listdeployments")
                || combined.contains("delete deployment")
                || combined.contains("delete")
                || combined.contains("stopagent")
                || combined.contains("scaleagent")
                || combined.contains("cancel")
            {
                score -= 12;
            }
        }
        AgentOsAction::Open(kind) => {
            if combined.contains("open")
                || combined.contains("view")
                || combined.contains("remoteui")
                || combined.contains("asset")
                || combined.contains("manage")
            {
                score += 8;
            }
            match kind {
                AgentOsKind::Agentic => {
                    if combined.contains("agentic") {
                        score += 4;
                    }
                }
                AgentOsKind::Application => {
                    if combined.contains("application") || combined.contains("app") {
                        score += 4;
                    }
                }
                AgentOsKind::Tool => {
                    if combined.contains("function") || combined.contains("faas") {
                        score += 8;
                    }
                    if combined.contains("tool") {
                        score += 6;
                    }
                    if combined.contains("agent as a service") || combined.contains("aaas") {
                        score -= 10;
                    }
                }
            }
        }
        AgentOsAction::Logs(kind) => {
            if combined.contains("log")
                || combined.contains("trace")
                || combined.contains("job")
                || combined.contains("process")
                || combined.contains("observability")
            {
                score += 8;
            }
            match kind {
                AgentOsKind::Agentic => {
                    if combined.contains("agentic") || combined.contains("debug") {
                        score += 4;
                    }
                }
                AgentOsKind::Application => {
                    if combined.contains("application")
                        || combined.contains("app")
                        || combined.contains("deployment")
                    {
                        score += 4;
                    }
                }
                AgentOsKind::Tool => {
                    if combined.contains("function") || combined.contains("faas") {
                        score += 8;
                    }
                    if combined.contains("tool") {
                        score += 6;
                    }
                    if combined.contains("agent as a service") || combined.contains("aaas") {
                        score -= 10;
                    }
                }
            }
        }
        _ => {}
    }
    let wants_tool_faas = matches!(
        action,
        AgentOsAction::Open(AgentOsKind::Tool) | AgentOsAction::Logs(AgentOsKind::Tool)
    );
    if !wants_tool_faas
        && (combined.contains("function") || combined.contains("faas") || combined.contains("tool"))
    {
        score -= 8;
    }
    score
}

fn agent_observe_progressive_params(
    asset: &AgentAssetRef,
    kind: AgentOsKind,
    display_name: &str,
    action: AgentOsAction,
) -> serde_json::Value {
    serde_json::json!({
        "assetId": asset.id,
        "assetName": asset.name,
        "agentAssetId": asset.id,
        "agentAssetName": asset.name,
        "agentKind": kind.agent_kind(),
        "kind": kind.agent_kind(),
        "functionRef": asset.name,
        "ref": asset.name,
        "name": asset.name,
        "displayName": display_name,
        "operation": action.label(),
        "input": {
            "assetId": asset.id,
            "assetName": asset.name,
            "agentKind": kind.agent_kind(),
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "payload": {
            "assetId": asset.id,
            "assetName": asset.name,
            "agentKind": kind.agent_kind(),
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "source": "a3s-code-tui",
    })
}

async fn try_agent_progressive_observe(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    action: AgentOsAction,
    asset: &AgentAssetRef,
    display_name: &str,
) -> Option<(remote_ui::ViewSpec, String)> {
    let kind = action.target_kind();
    let query = match (action, kind) {
        (AgentOsAction::Open(_), AgentOsKind::Tool) => {
            "Function as a Service open tool agent asset shaped ViewLink"
        }
        (AgentOsAction::Logs(_), AgentOsKind::Tool) => {
            "Function as a Service tool agent runtime logs shaped ViewLink"
        }
        (AgentOsAction::Open(_), AgentOsKind::Agentic) => {
            "Agent as a Service open agentic agent asset shaped ViewLink"
        }
        (AgentOsAction::Logs(_), AgentOsKind::Agentic) => {
            "Agent as a Service agentic debug run logs shaped ViewLink"
        }
        (AgentOsAction::Open(_), AgentOsKind::Application) => {
            "Agent as a Service open application agent asset shaped ViewLink"
        }
        (AgentOsAction::Logs(_), AgentOsKind::Application) => {
            "Agent as a Service application agent deployment logs shaped ViewLink"
        }
        _ => return None,
    };
    let execution = os_progressive::execute_first_matching(
        client,
        origin,
        token,
        query,
        agent_observe_progressive_params(asset, kind, display_name, action),
        |text, operation| agent_progressive_score(text, operation, action),
    )
    .await?;
    let fallback = match action {
        AgentOsAction::Open(_) => agent_view_spec(agent_asset_url(origin, &asset.id)),
        AgentOsAction::Logs(_) => agent_view_spec(agent_logs_url(origin, kind, &asset.id)),
        _ => unreachable!(),
    };
    Some((
        execution.view.unwrap_or(fallback),
        format!(
            "OS {} accepted `/agent {}` through progressive capabilities (`{}`).",
            kind.service_label(),
            action.label(),
            execution.operation.operation
        ),
    ))
}

fn is_application_deploy_planning_operation(operation: &str) -> bool {
    let operation = operation.to_ascii_lowercase();
    operation.contains("agentbuildcontroller_triggeragentbuild")
        || operation.contains("assetdeployabilitycontroller_evaluate")
        || operation.contains("assetreleasegatecontroller_evaluate")
        || operation.contains("agentruntimecontroller_launchagent")
}

fn agent_capability_params(
    described: Option<&serde_json::Value>,
    asset_id: &str,
    kind: AgentOsKind,
) -> serde_json::Value {
    if described
        .and_then(capability_schema_ref)
        .is_some_and(|schema| {
            schema.contains("AgenticDebugRunRequestDto")
                || schema.contains("AgentDebugRunRequestDto")
        })
    {
        return serde_json::json!({ "assetId": asset_id });
    }
    let names = described.map(capability_param_names).unwrap_or_default();
    let mut params = serde_json::Map::new();
    if names.is_empty() {
        params.insert("assetId".to_string(), serde_json::json!(asset_id));
        params.insert(
            "agentKind".to_string(),
            serde_json::json!(kind.agent_kind()),
        );
        params.insert("source".to_string(), serde_json::json!("a3s-code-tui"));
        return serde_json::Value::Object(params);
    }
    let mut saw_asset_id = false;
    let mut saw_kind = false;
    for name in names {
        let lower = name.to_ascii_lowercase();
        let value = if lower == "id"
            || (lower.contains("asset") && lower.contains("id"))
            || (lower.contains("agent") && lower.contains("id"))
        {
            saw_asset_id = true;
            Some(serde_json::json!(asset_id))
        } else if lower.contains("kind") || lower.ends_with("type") {
            saw_kind = true;
            Some(serde_json::json!(kind.agent_kind()))
        } else if lower.contains("source") || lower.contains("client") {
            Some(serde_json::json!("a3s-code-tui"))
        } else {
            None
        };
        if let Some(value) = value {
            params.insert(name, value);
        }
    }
    if !saw_asset_id {
        params.insert("assetId".to_string(), serde_json::json!(asset_id));
    }
    if !saw_kind {
        params.insert(
            "agentKind".to_string(),
            serde_json::json!(kind.agent_kind()),
        );
    }
    serde_json::Value::Object(params)
}

fn capability_schema_ref(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Object(obj) => {
            if let Some(reference) = obj.get("$ref").and_then(|v| v.as_str()) {
                return Some(reference.to_string());
            }
            obj.values().find_map(capability_schema_ref)
        }
        serde_json::Value::Array(arr) => arr.iter().find_map(capability_schema_ref),
        _ => None,
    }
}

fn capability_param_names(value: &serde_json::Value) -> Vec<String> {
    let mut out = Vec::new();
    collect_capability_param_names(value, &mut out);
    out.sort();
    out.dedup();
    out
}

fn collect_capability_param_names(value: &serde_json::Value, out: &mut Vec<String>) {
    match value {
        serde_json::Value::Object(obj) => {
            if let Some(properties) = value
                .pointer("/params/properties")
                .and_then(|v| v.as_object())
            {
                out.extend(properties.keys().cloned());
            }
            if let Some(properties) = obj
                .get("parameters")
                .or_else(|| obj.get("inputSchema"))
                .or_else(|| obj.get("schema"))
                .and_then(|v| v.get("properties"))
                .and_then(|v| v.as_object())
            {
                out.extend(properties.keys().cloned());
            }
            for child in obj.values() {
                collect_capability_param_names(child, out);
            }
        }
        serde_json::Value::Array(arr) => {
            for child in arr {
                collect_capability_param_names(child, out);
            }
        }
        _ => {}
    }
}

async fn try_agent_rest_operation(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset_id: &str,
    action: AgentOsAction,
) -> Option<(remote_ui::ViewSpec, String)> {
    let operation = match action {
        AgentOsAction::Run(_) => "run",
        AgentOsAction::Deploy => "deploy",
        _ => return None,
    };
    let id = path_segment(asset_id);
    let urls = match action {
        AgentOsAction::Run(AgentOsKind::Tool) => {
            vec![format!("{origin}/api/v1/runtime/functions/{id}/run")]
        }
        AgentOsAction::Run(_) => vec![
            format!("{origin}/api/v1/agents/{id}/runs"),
            format!("{origin}/api/v1/agents/{id}/run"),
            format!("{origin}/api/v1/assets/{id}/runs"),
        ],
        AgentOsAction::Deploy => vec![
            format!("{origin}/api/v1/agents/{id}/deployments"),
            format!("{origin}/api/v1/agents/{id}/deploy"),
            format!("{origin}/api/v1/assets/{id}/deployments"),
        ],
        _ => Vec::new(),
    };
    let mut last_error = String::new();
    for url in urls {
        let body = match action {
            AgentOsAction::Run(AgentOsKind::Tool) => serde_json::json!({
                "input": {
                    "query": "Run the default tool-agent smoke check from a3s code.",
                    "source": "a3s-code-tui",
                    "operation": operation,
                },
                "config": {
                    "source": "a3s-code-tui",
                },
                "timeoutMs": 60_000,
            }),
            _ => serde_json::json!({
                "source": "a3s-code-tui",
                "assetId": asset_id,
                "agentKind": action.target_kind().agent_kind(),
                "input": {
                    "source": "a3s-code-tui",
                    "assetId": asset_id,
                    "agentKind": action.target_kind().agent_kind(),
                    "operation": operation
                },
                "payload": {
                    "source": "a3s-code-tui",
                    "assetId": asset_id,
                    "agentKind": action.target_kind().agent_kind(),
                    "operation": operation
                }
            }),
        };
        let resp = client
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await;
        let resp = match resp {
            Ok(resp) => resp,
            Err(e) => {
                last_error = e.to_string();
                continue;
            }
        };
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if status.is_success() {
            if let Some(message) = function_run_failure_message(&text) {
                return Some((
                    agent_view_spec(agent_asset_url(origin, asset_id)),
                    format!(
                        "OS {} accepted the {operation} request, but the run failed: {message}",
                        action.target_kind().service_label()
                    ),
                ));
            }
            if let Some(spec) = remote_ui::find_view_url(&text, Some(origin)) {
                return Some((
                    spec,
                    format!(
                        "OS {} accepted the {operation} request.",
                        action.target_kind().service_label()
                    ),
                ));
            }
            return Some((
                agent_view_spec(agent_asset_url(origin, asset_id)),
                format!(
                    "OS {} accepted the {operation} request; no RemoteUI view was returned, so the OS asset view was opened.",
                    action.target_kind().service_label()
                ),
            ));
        }
        if matches!(status.as_u16(), 404 | 405 | 409 | 422 | 501) {
            last_error = truncate(&text, 180);
            continue;
        }
        last_error = format!("{status}: {}", truncate(&text, 180));
    }
    if last_error.is_empty() {
        None
    } else {
        Some((
            agent_view_spec(agent_asset_url(origin, asset_id)),
            format!(
                "The OS {} {operation} endpoint was unavailable ({last_error}); opened the OS asset view instead.",
                action.target_kind().service_label()
            ),
        ))
    }
}

async fn inspect_agent_asset(
    origin: &str,
    token: &str,
    action: AgentOsAction,
    asset_name: &str,
    def: &a3s_code_core::subagent::AgentDefinition,
    dev: &AgentDevSession,
) -> Result<AgentOsResult, String> {
    let kind = action.target_kind();
    let client = http()?;
    let package_source_path = agent_package_source_path(&dev.rel);
    let asset_source_path = agent_asset_source_path(&dev.rel, &dev.definition_rel);
    let config = agent_config_json(
        kind,
        def,
        &dev.rel,
        &package_source_path,
        &dev.definition_rel,
        &asset_source_path,
    );
    let Some(asset) = lookup_agent_asset(&client, origin, token, kind, asset_name).await? else {
        return Ok(AgentOsResult {
            action,
            kind,
            asset_name: asset_name.to_string(),
            asset_id: "not-published".to_string(),
            view: agent_view_spec(agent_asset_search_url(origin, asset_name)),
            note: format!(
                "OS {} for `{}`: no {} {} asset named `{}` was found. Run `/agent publish {}` first.",
                action.label(),
                def.name,
                kind.label(),
                kind.service_label(),
                asset_name,
                kind.agent_kind()
            ),
            open_view: false,
        });
    };

    if matches!(action, AgentOsAction::Open(_) | AgentOsAction::Logs(_)) {
        let fallback = match action {
            AgentOsAction::Open(_) => agent_view_spec(agent_asset_url(origin, &asset.id)),
            AgentOsAction::Logs(_) => agent_view_spec(agent_logs_url(origin, kind, &asset.id)),
            _ => unreachable!(),
        };
        let (view, note) =
            try_agent_progressive_observe(&client, origin, token, action, &asset, &def.name)
                .await
                .unwrap_or_else(|| {
                    let surface = match action {
                        AgentOsAction::Open(_) => kind.service_label().to_string(),
                        AgentOsAction::Logs(_) => format!("{} logs view", kind.service_label()),
                        _ => unreachable!(),
                    };
                    (
                        fallback,
                        format!("Opened existing OS agent asset through the {surface}."),
                    )
                });
        return Ok(AgentOsResult {
            action,
            kind,
            asset_name: asset.name,
            asset_id: asset.id,
            view,
            note,
            open_view: true,
        });
    }

    let config_status = if kind.uses_agent_config_endpoint() {
        agent_config_validation_status(&client, origin, token, &asset.id, &config).await
    } else {
        "agent-config endpoint not used for Function as a Service tool agents".to_string()
    };
    let runtime_binding_status =
        runtime_binding_validation_status(&client, origin, token, &asset.id).await;
    Ok(AgentOsResult {
        action,
        kind,
        asset_name: asset.name,
        asset_id: asset.id.clone(),
        view: agent_view_spec(agent_asset_url(origin, &asset.id)),
        note: format!(
            "OS status for `{}`: asset exists; {}; {}.",
            def.name, config_status, runtime_binding_status
        ),
        open_view: false,
    })
}

pub(crate) async fn publish_agent_to_os(
    session: crate::a3s_os::StoredOsSession,
    dev: AgentDevSession,
    action: AgentOsAction,
) -> Result<AgentOsResult, String> {
    let action = match action {
        AgentOsAction::Run(default_kind) => {
            AgentOsAction::Run(declared_agent_kind(&dev.package_path).unwrap_or(default_kind))
        }
        other => other,
    };
    let source = std::fs::read_to_string(&dev.path)
        .map_err(|e| format!("could not read {}: {e}", dev.path.display()))?;
    let def = parse_agent_definition(&dev.path, &source).map_err(|e| {
        format!(
            "{} is not a valid agent definition: {e}",
            dev.path.display()
        )
    })?;
    let origin = crate::a3s_os::os_origin(&session.address);
    let kind = action.target_kind();
    if matches!(action, AgentOsAction::Deploy) {
        if let Some(declared) = declared_agent_kind(&dev.package_path) {
            if !matches!(declared, AgentOsKind::Application) {
                return Err(format!(
                    "`agent deploy` is only for application agents; this package declares {}. Use `agent publish {}` or `agent run` instead.",
                    declared.label(),
                    declared.agent_kind()
                ));
            }
        }
    }
    let asset_name = agent_asset_name(kind, &def.name);
    let agent_contract = agent_contract_json(kind);
    if matches!(
        action,
        AgentOsAction::Status(_) | AgentOsAction::Open(_) | AgentOsAction::Logs(_)
    ) {
        return inspect_agent_asset(
            &origin,
            &session.access_token,
            action,
            &asset_name,
            &def,
            &dev,
        )
        .await;
    }
    let description = agent_description(&def);
    let asset = ensure_agent_asset(
        &origin,
        &session.access_token,
        kind,
        &asset_name,
        &description,
        &agent_contract,
    )
    .await?;
    let asset_id = asset.id.clone();
    let package_source_path = agent_package_source_path(&dev.rel);
    let asset_source_path = agent_asset_source_path(&dev.rel, &dev.definition_rel);
    let package_files = collect_agent_package_files(&dev)?;
    let config = agent_config_json(
        kind,
        &def,
        &dev.rel,
        &package_source_path,
        &dev.definition_rel,
        &asset_source_path,
    );
    let manifest = agent_manifest_json(
        kind,
        &def,
        &dev.rel,
        &package_source_path,
        &dev.definition_rel,
        &asset_source_path,
    );
    let mut runtime_binding = agent_runtime_binding_json(
        kind,
        &def,
        &dev.rel,
        &package_source_path,
        &dev.definition_rel,
        &asset_source_path,
    );
    merge_agent_acl_runtime_env(&mut runtime_binding, &dev.package_path);
    let mut asset_acl = agent_asset_acl(
        kind,
        &def,
        &dev.rel,
        &package_source_path,
        &dev.definition_rel,
        &asset_source_path,
    );
    asset_acl.push_str(&render_agent_acl_runtime_env(&runtime_binding));
    asset_lifecycle::write_asset_acl(&dev.package_path, &asset_acl)?;
    let client = http()?;
    sync_agent_contract_metadata(
        &client,
        &origin,
        &session.access_token,
        &asset.id,
        kind,
        &agent_contract,
    )
    .await?;
    let upload_error = match upload_agent_definition(
        &origin,
        &session.access_token,
        &asset.id,
        package_files,
        &asset_acl,
        &manifest,
        &config,
        &runtime_binding,
    )
    .await
    {
        Ok(()) => None,
        Err(err) if matches!(action, AgentOsAction::Run(AgentOsKind::Tool)) => Some(err),
        Err(err) => return Err(err),
    };
    let config_synced = if kind.uses_agent_config_endpoint() {
        Some(
            sync_agent_config(&origin, &session.access_token, &asset.id, &config)
                .await
                .map_err(|e| format!("{e}; asset config was still saved"))?,
        )
    } else {
        None
    };
    let runtime_binding_synced =
        sync_agent_runtime_binding(&origin, &session.access_token, &asset.id, &runtime_binding)
            .await;

    let (view, note) = match action {
        AgentOsAction::Publish(_) | AgentOsAction::Open(_) => (
            agent_view_spec(agent_asset_url(&origin, &asset_id)),
            format!(
                "Published `{}` as an OS {} {} asset.",
                def.name,
                kind.label(),
                kind.service_label()
            ),
        ),
        AgentOsAction::Logs(_) => {
            let note = match kind {
                AgentOsKind::Agentic => format!(
                    "Opened OS agentic debug-run processes and logs for `{}` when available.",
                    def.name
                ),
                AgentOsKind::Application => format!(
                    "Opened OS Runtime deployment/service logs for `{}` when available.",
                    def.name
                ),
                AgentOsKind::Tool => format!(
                    "Opened OS Function as a Service logs for tool agent `{}` when available.",
                    def.name
                ),
            };
            (
                agent_view_spec(agent_logs_url(&origin, kind, &asset_id)),
                note,
            )
        }
        AgentOsAction::Run(_) | AgentOsAction::Deploy => {
            let direct_build = if matches!(action, AgentOsAction::Deploy) {
                try_application_agent_build(&client, &origin, &session.access_token, &asset).await
            } else {
                None
            };
            let operation_result = match direct_build {
                Some(result) => Some(result),
                None => {
                    try_agent_operation(&client, &origin, &session.access_token, &asset.id, action)
                        .await
                }
            };
            match operation_result {
                Some((view, note)) => (view, note),
                None => (
                    agent_view_spec(agent_asset_url(&origin, &asset.id)),
                    format!(
                        "Published `{}`; opened the OS asset view because no Agent as a Service endpoint was discovered.",
                        def.name
                    ),
                ),
            }
        }
        AgentOsAction::Status(_) => unreachable!("status returns before publish flow"),
    };
    let mut note = append_runtime_binding_sync_note(
        append_agent_config_note(note, kind, config_synced),
        &runtime_binding_synced,
    );
    if let Some(err) = upload_error {
        note.push_str(&format!(
            " Repository upload failed before run ({err}); OS Function as a Service still ran against the existing asset metadata."
        ));
    }

    Ok(AgentOsResult {
        action,
        kind,
        asset_name,
        asset_id,
        view,
        note,
        open_view: true,
    })
}

fn append_config_sync_note(mut note: String, config_synced: bool) -> String {
    if config_synced {
        note.push_str(" OS agent config was synced.");
    } else {
        note.push_str(" OS agent-config endpoint was unavailable; asset config was saved.");
    }
    note
}

fn append_agent_config_note(
    note: String,
    kind: AgentOsKind,
    config_synced: Option<bool>,
) -> String {
    match config_synced {
        Some(synced) => append_config_sync_note(note, synced),
        None if matches!(kind, AgentOsKind::Tool) => {
            let mut note = note;
            note.push_str(
                " Tool agents use Function as a Service; asset config was saved as metadata.",
            );
            note
        }
        None => note,
    }
}

fn append_runtime_binding_sync_note(
    mut note: String,
    runtime_binding_synced: &AgentRuntimeBindingSync,
) -> String {
    match runtime_binding_synced {
        AgentRuntimeBindingSync::Synced => {
            note.push_str(" OS runtime binding was synced.");
        }
        AgentRuntimeBindingSync::Unsupported => {
            note.push_str(
                " OS runtime-binding endpoint was unavailable; runtime-binding intent was saved.",
            );
        }
        AgentRuntimeBindingSync::Failed(err) => {
            note.push_str(&format!(
                " OS runtime binding could not be synced: {}; runtime-binding intent was saved.",
                truncate(err, 160)
            ));
        }
    }
    note
}

pub(crate) fn agent_dev_prompt(session: &AgentDevSession, request: &str) -> String {
    format!(
        "You are in A3S Code local agent-package development mode.\n\
         Current agent: {name}\n\
         Description: {description}\n\
         Package: {package}\n\
         Entrypoint: {path}\n\
         Agents root: {root}\n\n\
         User request:\n{request}\n\n\
         Work on this local agent package iteratively. Read the entrypoint from disk before \
         editing; if normal file tools cannot access it because it is outside the workspace, use \
         non-interactive bash commands with the full quoted path. Keep the entrypoint valid for \
         a3s-code: Markdown agents need YAML frontmatter followed by the system prompt; YAML/YML \
         agents must remain valid YAML. Preserve or improve the stable agent `name`, trigger \
         `description`, tools, model, max_steps, workflow guidance, package resources, and success criteria according \
         to the user's request. Do not open OS, WebIDE, RemoteUI, or browser pages for this local \
         agent-dev turn. Validate the file after edits by printing its first relevant lines and, \
         when practical, parsing or sanity-checking the frontmatter/YAML. End with a concise \
         summary of changes and any next suggested improvement.\n\n\
         The TUI remains in agent package-development mode for `{name}` after this turn; the user can \
         press Esc or run `/agent off` to return to normal mode.",
        name = session.name.as_str(),
        description = session.description.as_str(),
        package = session.package_path.display(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

pub(crate) fn agent_review_prompt(session: &AgentDevSession) -> String {
    let contract = super::review::review_report_contract(&session.root);
    format!(
        "Review this local A3S agent package without changing files unless the user explicitly asks \
         for fixes.\n\
         Agent name: {name}\n\
         Description: {description}\n\
         Package path: {package}\n\
         Entrypoint path: {path}\n\
         Agent root: {root}\n\n\
         Read the package and entrypoint, then report concise findings on: YAML/frontmatter \
         validity, trigger clarity, scope boundaries, tool permissions, workflow instructions, \
         safety constraints, package resources, success criteria, examples/tests, and readiness \
         for Agent as a Service. Mention the \
         smallest recommended improvements and whether `/agent run` or `/agent deploy` is the \
         right next lifecycle step.{contract}",
        name = session.name.as_str(),
        description = session.description.as_str(),
        package = session.package_path.display(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

pub(crate) fn agent_goal_label(session: &AgentDevSession, goal: &str) -> String {
    format!(
        "Agent `{}` goal: {}",
        session.name,
        goal.trim().trim_end_matches('.')
    )
}

pub(crate) fn agent_loop_prompt(session: &AgentDevSession, loop_prompt: &str) -> String {
    format!(
        "You are running A3S Code loop engineering inside local /agent package-development mode.\n\
         Active agent: {name}\n\
         Description: {description}\n\
         Package: {package}\n\
         Entrypoint: {path}\n\
         Agents root: {root}\n\n\
         Agent-loop rules:\n\
         - Keep this loop scoped to the active agent package and its loop artifacts.\n\
         - Stay local: do not open OS, WebIDE, RemoteUI, browser pages, or OS workflow designer.\n\
         - Read the current package entrypoint before proposing or applying changes.\n\
         - Use maker/checker passes: one pass improves the package, a separate pass verifies \
         frontmatter/YAML validity, tool scope, trigger description, workflow guidance, and \
         success criteria.\n\
         - Validate the file after edits when practical, then summarize report paths and changes.\n\n\
         {loop_prompt}",
        name = session.name.as_str(),
        description = session.description.as_str(),
        package = session.package_path.display(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

impl App {
    pub(crate) fn on_agent_os_completed(
        &mut self,
        status_entry: TranscriptEntryId,
        res: Result<AgentOsResult, String>,
    ) {
        match res {
            Ok(result) => {
                self.last_view = Some(result.view.clone());
                self.replace_tracked_line(
                    status_entry,
                    &gutter(
                        TN_CYAN,
                        &format!(
                            "◇ /agent {} · {} · `{}` ({})",
                            result.action.label(),
                            result.kind.label(),
                            result.asset_name,
                            result.asset_id
                        ),
                    ),
                );
                self.push_line(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render(&format!("  {}", result.note)),
                );
                if result.open_view {
                    self.push_line(&gutter(
                        ACCENT,
                        &remote_view_button(&format!(
                            "{} · click to reopen",
                            result.kind.service_label()
                        )),
                    ));
                    self.open_remote_view(&result.view);
                } else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  Open view opens the related OS asset view"),
                    );
                }
            }
            Err(e) => {
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  /agent OS operation failed: {e}")),
                );
            }
        }
    }

    pub(crate) fn exit_agent_dev(&mut self) {
        match self.agent_dev.take() {
            Some(session) => self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  agent dev off — {} ({})",
                session.name, session.rel
            ))),
            None => self.push_line(&Style::new().fg(TN_GRAY).render("  agent dev is not active")),
        }
        self.relayout();
    }

    /// Open the `/agent` picker.
    pub(crate) fn open_agent_panel(&mut self) {
        let root = self.asset_directories.agent.clone();
        let agents = list_agents(&root);
        if agents.is_empty() {
            self.pending_agent_subcommand = None;
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  no agents in {} — draft one with `/agent <description>` first",
                root.display()
            )));
            return;
        }
        self.agent_picker = Some(AgentPanel {
            root,
            agents,
            sel: 0,
        });
    }

    pub(crate) fn activate_agent_package_path(
        &mut self,
        package_path: &std::path::Path,
    ) -> Option<Cmd<Msg>> {
        let root = self.asset_directories.agent.clone();
        let agents = list_agents(&root);
        let Some(sel) = agents
            .iter()
            .position(|agent| package_path == agent.path || agent.path.starts_with(package_path))
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  scaffolded package does not contain a recognized agent entrypoint"),
            );
            return None;
        };
        self.activate_agent_panel_selection(AgentPanel { root, agents, sel }, sel)
    }

    /// Keys while the `/agent` picker is open — consumes everything.
    pub(crate) fn handle_agent_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let p = self.agent_picker.as_mut()?;
        let last = p.agents.len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => p.sel = p.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => p.sel = (p.sel + 1).min(last),
            KeyCode::Esc => {
                cancel_pending_picker(&mut self.agent_picker, &mut self.pending_agent_subcommand)
            }
            KeyCode::Enter => {
                let panel = self.agent_picker.take()?;
                let selected = panel.sel.min(last);
                return self.activate_agent_panel_selection(panel, selected);
            }
            _ => {}
        }
        None
    }

    pub(crate) fn handle_agent_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let panel_state = self.agent_picker.as_ref()?;
        let total = panel_state.agents.len();
        if total == 0 {
            return None;
        }
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let selected = panel_state.sel.min(total - 1);
        let (mut panel, panel_height) = agent_picker_panel(
            &panel_state.agents,
            selected,
            &panel_state.root,
            width,
            self.height as usize,
        )?;
        let row_count = panel.view(width as u16, panel_height).lines().count();
        if row_count == 0 {
            return None;
        }
        let y_offset =
            agent_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                let panel_state = self.agent_picker.take()?;
                self.activate_agent_panel_selection(panel_state, index.min(total - 1))
            }
            Some(MenuPanelMsg::Cancelled) => {
                cancel_pending_picker(&mut self.agent_picker, &mut self.pending_agent_subcommand);
                None
            }
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    if let Some(open) = self.agent_picker.as_mut() {
                        open.sel = after;
                    }
                }
                None
            }
        }
    }

    fn activate_agent_panel_selection(
        &mut self,
        panel: AgentPanel,
        selected: usize,
    ) -> Option<Cmd<Msg>> {
        let last = panel.agents.len().saturating_sub(1);
        let picked = panel.agents.get(selected.min(last))?.clone();
        let source = match std::fs::read_to_string(&picked.definition_path) {
            Ok(s) => s,
            Err(e) => {
                self.push_line(&Style::new().fg(TN_RED).render(&format!(
                    "  could not read {}: {e}",
                    picked.definition_path.display()
                )));
                return None;
            }
        };
        let def = match parse_agent_definition(&picked.definition_path, &source) {
            Ok(def) => def,
            Err(e) => {
                self.push_line(&Style::new().fg(TN_RED).render(&format!(
                    "  {} entrypoint {} is not a valid agent definition — fix it (or redraft with /agent <description>): {e}",
                    picked.rel, picked.definition_rel
                )));
                return None;
            }
        };
        self.mcp_dev = None;
        self.skill_dev = None;
        self.okf_dev = None;
        self.agent_dev = Some(AgentDevSession {
            name: def.name.clone(),
            description: agent_description(&def),
            rel: picked.rel.clone(),
            definition_rel: picked.definition_rel.clone(),
            path: picked.definition_path.clone(),
            package_path: picked.path.clone(),
            root: panel.root,
        });
        self.push_line(&gutter(
            TN_CYAN,
            &format!(
                "◇ agent dev: {} ({} · {}) · Esc or /agent off returns to normal mode",
                def.name, picked.rel, picked.definition_rel
            ),
        ));
        self.relayout();
        if let Some(pending) = self.pending_agent_subcommand.take() {
            return self.execute_agent_subcommand(pending);
        }
        None
    }

    /// Overlay the `/agent` picker above the input.
    pub(crate) fn overlay_agent_menu(&self, composed: String) -> String {
        let Some(p) = self.agent_picker.as_ref() else {
            return composed;
        };
        let width = self.width as usize;
        let menu = agent_picker_lines(&p.agents, p.sel, &p.root, width, self.height as usize);
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn temp_root(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!("a3s-{name}-{}-{nanos}", std::process::id()))
    }

    fn write_complete_agent_test_package(
        package: &std::path::Path,
        source: &str,
    ) -> std::path::PathBuf {
        for dir in ["prompts", "workflows", "examples", "eval", "tests"] {
            std::fs::create_dir_all(package.join(dir)).unwrap();
        }
        let path = package.join("agent.md");
        std::fs::write(&path, source).unwrap();
        std::fs::write(package.join("README.md"), "# Test agent\n").unwrap();
        std::fs::write(
            package.join("prompts/system.md"),
            "Use the package source as the agent contract.\n",
        )
        .unwrap();
        std::fs::write(
            package.join("workflows/operating-procedure.md"),
            "Inspect, plan, execute, and report.\n",
        )
        .unwrap();
        std::fs::write(package.join("examples/example-input.md"), "Input\n").unwrap();
        std::fs::write(package.join("examples/example-output.md"), "Output\n").unwrap();
        std::fs::write(package.join("eval/smoke.md"), "Smoke eval\n").unwrap();
        std::fs::write(package.join("tests/smoke.md"), "Smoke test\n").unwrap();
        path
    }

    #[test]
    fn lists_agent_packages_recursively_sorted_skipping_dotfiles() {
        let root = temp_root("agents");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("nested")).unwrap();
        std::fs::create_dir_all(root.join(".secret")).unwrap();
        std::fs::write(root.join("zeta.yaml"), "name: z\ndescription: z").unwrap();
        std::fs::write(root.join("alpha.md"), "---\nname: a\n---\nbody").unwrap();
        std::fs::write(root.join("nested/beta.yml"), "name: b\ndescription: b").unwrap();
        std::fs::write(root.join(".hidden.md"), "x").unwrap();
        std::fs::write(root.join(".secret/gamma.md"), "x").unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();

        let agents = list_agents(&root);
        let rels: Vec<_> = agents.into_iter().map(|a| a.rel).collect();
        assert_eq!(rels, vec!["alpha", "nested/beta", "zeta"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn collect_agent_package_files_rejects_single_file_agents() {
        let root = temp_root("agent-single-file");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("reviewer.md");
        std::fs::write(
            &path,
            "---\nname: reviewer\ndescription: Review code changes\n---\nReview.\n",
        )
        .unwrap();
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes".into(),
            rel: "reviewer".into(),
            definition_rel: "reviewer.md".into(),
            path: path.clone(),
            package_path: path,
            root: root.clone(),
        };

        let err = collect_agent_package_files(&dev).expect_err("single-file agents are invalid");

        assert!(
            err.contains("complete A3S Code autonomous-agent project"),
            "{err}"
        );
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn collect_agent_package_files_rejects_bare_agent_md_package() {
        let root = temp_root("agent-bare-md");
        let _ = std::fs::remove_dir_all(&root);
        let package = root.join("reviewer");
        std::fs::create_dir_all(&package).unwrap();
        let path = package.join("agent.md");
        std::fs::write(
            &path,
            "---\nname: reviewer\ndescription: Review code changes\n---\nReview.\n",
        )
        .unwrap();
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes".into(),
            rel: "reviewer".into(),
            definition_rel: "agent.md".into(),
            path,
            package_path: package,
            root: root.clone(),
        };

        let err = collect_agent_package_files(&dev).expect_err("bare agent.md package is invalid");

        assert!(err.contains("missing visible source file"), "{err}");
        assert!(err.contains("README.md"), "{err}");
        assert!(err.contains("prompts/system.md"), "{err}");
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_agent_definition_with_core_parser() {
        let root = temp_root("agent-parse");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("reviewer.md");
        let body = r#"---
name: reviewer
description: Review changes
---
Be precise.
"#;
        let def = parse_agent_definition(&path, body).unwrap();
        assert_eq!(def.name, "reviewer");
        assert_eq!(def.prompt.as_deref(), Some("Be precise."));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn agent_picker_lines_use_bounded_shared_menu_rows() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/agents/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let agents = vec![
            AgentFile {
                rel: "nested/very-long-agent-package-name-that-would-overflow-the-panel".into(),
                path: root
                    .join("nested/very-long-agent-package-name-that-would-overflow-the-panel"),
                definition_rel: "agent.md".into(),
                definition_path: root.join(
                    "nested/very-long-agent-package-name-that-would-overflow-the-panel/agent.md",
                ),
            },
            AgentFile {
                rel: "reviewer".into(),
                path: root.join("reviewer"),
                definition_rel: "agent.md".into(),
                definition_path: root.join("reviewer/agent.md"),
            },
        ];
        let lines = agent_picker_lines(&agents, 0, &root, 40, 20);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("agent"), "{plain}");
        assert!(plain.contains("select a package"), "{plain}");
        assert!(plain.contains("very-long-agent-package"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn agent_picker_lines_scroll_selected_agent_into_view() {
        let root = std::path::PathBuf::from("/tmp/agents");
        let agents = (0..16)
            .map(|index| AgentFile {
                rel: format!("agent-{index}"),
                path: root.join(format!("agent-{index}")),
                definition_rel: "agent.md".into(),
                definition_path: root.join(format!("agent-{index}/agent.md")),
            })
            .collect::<Vec<_>>();
        let plain = agent_picker_lines(&agents, 14, &root, 48, 16)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("agent-14"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn agent_picker_header_and_hint_fit_fixed_width() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/agents/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let header = agent_picker_header(9, &root, 40);
        let hint = agent_picker_hint(40);
        assert!(a3s_tui::style::visible_len(&header) <= 40, "{header}");
        assert!(a3s_tui::style::visible_len(&hint) <= 40, "{hint}");
    }

    #[test]
    fn agent_picker_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let root = std::path::PathBuf::from("/tmp/agents");
        let agents = (0..4)
            .map(|index| AgentFile {
                rel: format!("agent-{index}"),
                path: root.join(format!("agent-{index}")),
                definition_rel: "agent.md".into(),
                definition_path: root.join(format!("agent-{index}/agent.md")),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = agent_picker_lines(&agents, 0, &root, width, height).len();
        let y_offset = agent_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = agent_picker_panel(&agents, 0, &root, width, height).expect("panel");
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: y_offset + 2,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, None);
        assert_eq!(panel.selected_index(), 1);
    }

    #[test]
    fn agent_picker_click_selects_visible_row_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let root = std::path::PathBuf::from("/tmp/agents");
        let agents = (0..4)
            .map(|index| AgentFile {
                rel: format!("agent-{index}"),
                path: root.join(format!("agent-{index}")),
                definition_rel: "agent.md".into(),
                definition_path: root.join(format!("agent-{index}/agent.md")),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = agent_picker_lines(&agents, 0, &root, width, height).len();
        let y_offset = agent_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = agent_picker_panel(&agents, 0, &root, width, height).expect("panel");
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: y_offset + 3,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, Some(MenuPanelMsg::Selected(1)));
    }

    #[test]
    fn agent_overlay_offset_moves_up_with_more_rows_below() {
        assert!(agent_overlay_y_offset(24, 8, 7) < agent_overlay_y_offset(24, 8, 5));
    }

    #[test]
    fn agent_gen_prompt_carries_format_rules_and_dir() {
        let p = agent_gen_prompt("review rust diffs", "/Users/x/.a3s/agents");
        assert!(p.contains("review rust diffs"));
        assert!(p.contains("/Users/x/.a3s/agents"));
        assert!(p.contains("complete local A3S Code agent package"));
        assert!(p.contains("YAML frontmatter"));
        assert!(p.contains("name: <kebab-case-agent-name>"));
        assert!(p.contains("Do NOT stop at only agent.md"));
        assert!(p.contains("README.md"));
        assert!(p.contains("prompts/system.md"));
        assert!(p.contains("workflows/operating-procedure.md"));
        assert!(p.contains("examples/example-input.md"));
        assert!(p.contains("examples/example-output.md"));
        assert!(p.contains("eval/smoke.md"));
        assert!(p.contains("tests/smoke.md"));
        assert!(p.contains(".a3s/asset.acl"));
        assert!(p.contains("Do NOT create extra generated JSON config files"));
        assert!(p.contains("keep package configuration in `.a3s/asset.acl`"));
        assert!(p.contains("metadata-only"));
        assert!(p.contains("Do NOT put `agent.md`"));
        assert!(p.contains("OUTSIDE this session's workspace") && p.contains("bash"));
        assert!(p.contains("Never run a command that waits on stdin"));
    }

    #[test]
    fn scaffold_agent_package_creates_complete_framework_package() {
        let root = temp_root("agent-scaffold");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let dev = scaffold_agent_package(
            "Name it exactly a3s-e2e-review-agent. It reviews pull-request diffs for risky Rust changes.",
            &root,
        )
        .unwrap();

        assert_eq!(dev.name, "a3s-e2e-review-agent");
        assert_eq!(dev.definition_rel, "agent.md");
        for rel in [
            "README.md",
            "agent.md",
            "prompts/system.md",
            "workflows/operating-procedure.md",
            "examples/example-input.md",
            "examples/example-output.md",
            "eval/smoke.md",
            "tests/smoke.md",
            ".a3s/asset.acl",
        ] {
            assert!(dev.package_path.join(rel).is_file(), "missing {rel}");
        }
        assert!(!dev.package_path.join(".a3s/agent.md").exists());
        assert!(!dev.package_path.join(".a3s/prompts/system.md").exists());
        for rel in [
            "agent.asset.json",
            "agent.config.json",
            "agent.runtime-binding.json",
            ".a3s/agent.asset.json",
            ".a3s/agent.config.json",
            ".a3s/agent.runtime-binding.json",
        ] {
            assert!(!dev.package_path.join(rel).exists(), "unexpected {rel}");
        }
        let asset_acl =
            std::fs::read_to_string(dev.package_path.join(asset_lifecycle::ASSET_ACL_PATH))
                .unwrap();
        assert!(asset_acl.contains("version = \"a3s.asset.v1\""));
        assert!(asset_acl.contains("category = \"agent\""));
        assert!(asset_acl.contains("definition_path = \"agent.md\""));

        let package_files = collect_agent_package_files(&dev).unwrap();
        assert!(package_files.iter().any(|file| file.path == "README.md"));
        assert!(
            package_files
                .iter()
                .all(|file| !file.path.contains("/.a3s/")),
            "publish source collection should not duplicate generated .a3s metadata"
        );

        let listed = list_agents(&root);
        assert!(listed
            .iter()
            .any(|agent| agent.rel == "a3s-e2e-review-agent"));
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn scaffold_agent_package_honors_tool_and_application_hints() {
        let root = temp_root("agent-scaffold-kind");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let tool = scaffold_agent_package(
            "Name it exactly sql-checker. Build a tool agent for Function as a Service SQL checks.",
            &root,
        )
        .unwrap();
        let app = scaffold_agent_package(
            "Name it exactly portal-agent. Build an application agent for a small portal.",
            &root,
        )
        .unwrap();

        let tool_source = std::fs::read_to_string(&tool.path).unwrap();
        let tool_def = parse_agent_definition(&tool.path, &tool_source).unwrap();
        let tool_package_source_path = agent_package_source_path(&tool.rel);
        let tool_asset_source_path = agent_asset_source_path(&tool.rel, &tool.definition_rel);
        let tool_manifest = agent_manifest_json(
            AgentOsKind::Tool,
            &tool_def,
            &tool.rel,
            &tool_package_source_path,
            &tool.definition_rel,
            &tool_asset_source_path,
        );
        let tool_binding = agent_runtime_binding_json(
            AgentOsKind::Tool,
            &tool_def,
            &tool.rel,
            &tool_package_source_path,
            &tool.definition_rel,
            &tool_asset_source_path,
        );
        assert_eq!(tool_manifest["agentKind"], "tool");
        assert_eq!(tool_manifest["service"], "Function as a Service");
        assert_eq!(tool_manifest["runtimeIntent"]["kind"], "tool");
        assert_eq!(
            tool_manifest["runtimeIntent"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(tool_manifest["runtimeIntent"]["protocol"], "agent-tool");
        assert_eq!(tool_binding["kind"], "tool");
        assert_eq!(tool_binding["runtime"]["protocol"], "agent-tool");

        let app_source = std::fs::read_to_string(&app.path).unwrap();
        let app_def = parse_agent_definition(&app.path, &app_source).unwrap();
        let app_package_source_path = agent_package_source_path(&app.rel);
        let app_asset_source_path = agent_asset_source_path(&app.rel, &app.definition_rel);
        let app_manifest = agent_manifest_json(
            AgentOsKind::Application,
            &app_def,
            &app.rel,
            &app_package_source_path,
            &app.definition_rel,
            &app_asset_source_path,
        );
        let app_binding = agent_runtime_binding_json(
            AgentOsKind::Application,
            &app_def,
            &app.rel,
            &app_package_source_path,
            &app.definition_rel,
            &app_asset_source_path,
        );
        assert_eq!(app_manifest["agentKind"], "application");
        assert_eq!(app_manifest["service"], "Agent as a Service");
        assert_eq!(app_manifest["runtimeIntent"]["kind"], "agent");
        assert_eq!(app_manifest["runtimeIntent"]["isolation"], "container");
        assert_eq!(app_binding["kind"], "agent");
        assert_eq!(app_binding["runtime"]["mode"], "application-deployment");
        assert_eq!(app_binding["network"]["ingress"], true);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_agent_os_subcommands() {
        assert_eq!(
            parse_agent_subcommand("publish agentic").unwrap().unwrap(),
            AgentSubcommand::Publish(AgentOsKind::Agentic)
        );
        assert_eq!(
            parse_agent_subcommand("publish application")
                .unwrap()
                .unwrap(),
            AgentSubcommand::Publish(AgentOsKind::Application)
        );
        assert_eq!(
            parse_agent_subcommand("publish tool").unwrap().unwrap(),
            AgentSubcommand::Publish(AgentOsKind::Tool)
        );
        assert_eq!(
            parse_agent_subcommand("run").unwrap().unwrap(),
            AgentSubcommand::Run
        );
        assert_eq!(
            parse_agent_subcommand("deploy").unwrap().unwrap(),
            AgentSubcommand::Deploy
        );
        assert_eq!(
            parse_agent_subcommand("open").unwrap().unwrap(),
            AgentSubcommand::Open(AgentOsKind::Agentic)
        );
        assert_eq!(
            parse_agent_subcommand("logs application").unwrap().unwrap(),
            AgentSubcommand::Logs(AgentOsKind::Application)
        );
        assert_eq!(
            parse_agent_subcommand("logs tool").unwrap().unwrap(),
            AgentSubcommand::Logs(AgentOsKind::Tool)
        );
        assert_eq!(
            parse_agent_subcommand("status application")
                .unwrap()
                .unwrap(),
            AgentSubcommand::Status(AgentOsKind::Application)
        );
        assert_eq!(
            parse_agent_subcommand("off").unwrap().unwrap(),
            AgentSubcommand::Exit
        );
        assert_eq!(
            parse_agent_subcommand("activity failed runs")
                .unwrap()
                .unwrap(),
            AgentSubcommand::Activity("failed runs".into())
        );
        assert_eq!(
            AgentOsAction::Publish(AgentOsKind::Agentic).label(),
            "publish"
        );
        assert!(parse_agent_subcommand("run extra").unwrap().is_err());
        assert_eq!(
            parse_agent_subcommand("logs app").unwrap().unwrap_err(),
            AGENT_LOGS_USAGE
        );
        assert_eq!(
            parse_agent_subcommand("open function")
                .unwrap()
                .unwrap_err(),
            AGENT_OPEN_USAGE
        );
        assert_eq!(
            parse_agent_subcommand("status agent").unwrap().unwrap_err(),
            AGENT_STATUS_USAGE
        );
        assert_eq!(
            parse_agent_subcommand("publish function")
                .unwrap()
                .unwrap_err(),
            AGENT_PUBLISH_USAGE
        );
        assert!(parse_agent_subcommand("ps").unwrap().is_err());
        assert_eq!(
            parse_agent_subcommand("debug").unwrap().unwrap_err(),
            "unknown /agent command `debug`"
        );
        assert!(parse_agent_subcommand("jobs").unwrap().is_err());
        assert!(parse_agent_subcommand("inspect").unwrap().is_err());
        for removed in ["view", "remote", "os", "dashboard"] {
            assert!(
                parse_agent_subcommand(removed).unwrap().is_err(),
                "/agent {removed} should not create an agent prototype"
            );
        }
        assert!(parse_agent_subcommand("make me a reviewer").is_none());
    }

    #[test]
    fn tool_agents_use_function_as_a_service_metadata() {
        let def = a3s_code_core::subagent::AgentDefinition::new(
            "Tool Captain",
            "Run a small reusable tool",
        );
        let package_source_path = agent_package_source_path("tools/captain");
        let asset_source_path = agent_asset_source_path("tools/captain", "agent.md");
        let manifest = agent_manifest_json(
            AgentOsKind::Tool,
            &def,
            "tools/captain",
            &package_source_path,
            "agent.md",
            &asset_source_path,
        );
        let runtime_binding = agent_runtime_binding_json(
            AgentOsKind::Tool,
            &def,
            "tools/captain",
            &package_source_path,
            "agent.md",
            &asset_source_path,
        );

        assert_eq!(
            agent_asset_name(AgentOsKind::Tool, "Tool Captain"),
            "agent-tool-tool-captain"
        );
        assert_eq!(manifest["category"], "agent");
        assert_eq!(manifest["agentKind"], "tool");
        assert_eq!(manifest["service"], "Function as a Service");
        assert_eq!(manifest["runtimeIntent"]["kind"], "tool");
        assert_eq!(manifest["runtimeIntent"]["isolation"], "serving");
        assert_eq!(manifest["runtimeIntent"]["agentKind"], "tool");
        assert_eq!(
            manifest["runtimeIntent"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(manifest["runtimeIntent"]["protocol"], "agent-tool");
        assert_eq!(runtime_binding["kind"], "tool");
        assert_eq!(runtime_binding["isolation"], "serving");
        assert_eq!(runtime_binding["runtime"]["kind"], "a3s-function-service");
        assert_eq!(runtime_binding["runtime"]["protocol"], "agent-tool");
        assert_eq!(runtime_binding["runtime"]["agentKind"], "tool");
        assert_eq!(
            runtime_binding["metadata"]["service"],
            "Function as a Service"
        );
        let upsert = agent_runtime_binding_upsert_body(&runtime_binding);
        assert_eq!(upsert["kind"], "tool");
        assert_eq!(upsert["runtime"]["kind"], "a3s-function-service");
        assert_eq!(upsert["runtime"]["sharedRuntime"], "node-20");
        assert!(upsert["runtime"].get("protocol").is_none());
        assert!(upsert["runtime"].get("agentKind").is_none());
        assert_eq!(AgentOsKind::Tool.service_label(), "Function as a Service");
        assert_eq!(AgentOsKind::Agentic.service_label(), "Agent as a Service");
        assert_eq!(
            AgentOsKind::Application.service_label(),
            "Agent as a Service"
        );
        assert!(parse_agent_subcommand("run extra").unwrap().is_err());
    }

    #[test]
    fn agent_os_asset_helpers_encode_kind_and_manifest() {
        let def = a3s_code_core::subagent::AgentDefinition::new(
            "Review Captain",
            "Review code changes carefully",
        )
        .with_model(a3s_code_core::subagent::ModelConfig::with_provider(
            "openai", "gpt-4o",
        ))
        .with_max_steps(12)
        .with_permissions(
            a3s_code_core::permissions::PermissionPolicy::new()
                .allow("Read(src/**/*.rs)")
                .ask("Bash(cargo test:*)"),
        )
        .with_prompt("Review the patch and return crisp findings.");
        let package_source_path = agent_package_source_path("review/captain");
        let asset_source_path = agent_asset_source_path("review/captain", "agent.md");
        let manifest = agent_manifest_json(
            AgentOsKind::Application,
            &def,
            "review/captain",
            &package_source_path,
            "agent.md",
            &asset_source_path,
        );
        let config = agent_config_json(
            AgentOsKind::Application,
            &def,
            "review/captain",
            &package_source_path,
            "agent.md",
            &asset_source_path,
        );
        let runtime_binding = agent_runtime_binding_json(
            AgentOsKind::Application,
            &def,
            "review/captain",
            &package_source_path,
            "agent.md",
            &asset_source_path,
        );

        assert_eq!(
            agent_asset_name(AgentOsKind::Agentic, "Review Captain"),
            "agentic-review-captain"
        );
        assert_eq!(
            agent_asset_name(AgentOsKind::Application, "Review Captain"),
            "agent-app-review-captain"
        );
        assert_eq!(
            agent_asset_name(AgentOsKind::Tool, "Review Captain"),
            "agent-tool-review-captain"
        );
        assert_eq!(package_source_path, ".");
        assert_eq!(asset_source_path, "agent.md");
        assert_eq!(manifest["category"], "agent");
        assert_eq!(manifest["agentKind"], "application");
        assert_eq!(manifest["service"], "Agent as a Service");
        assert_eq!(manifest["runtimeIntent"]["kind"], "agent");
        assert_eq!(manifest["runtimeIntent"]["isolation"], "container");
        assert_eq!(manifest["runtimeIntent"]["agentKind"], "application");
        assert_eq!(
            manifest["runtimeIntent"]["runtimeKind"],
            "a3s-agent-service"
        );
        assert_eq!(manifest["packagePath"], ".");
        assert_eq!(manifest["entrypoint"], "agent.md");
        assert_eq!(manifest["definitionPath"], "agent.md");
        assert_eq!(manifest["assetAclPath"], asset_lifecycle::ASSET_ACL_PATH);
        assert!(manifest.get("configPath").is_none());
        assert!(manifest.get("runtimeBindingPath").is_none());
        assert_eq!(manifest["definition"]["name"], "Review Captain");
        assert_eq!(
            config["systemPrompt"],
            "Review the patch and return crisp findings."
        );
        assert_eq!(config["model"]["provider"], "openai");
        assert_eq!(config["model"]["modelId"], "gpt-4o");
        assert_eq!(config["maxIterations"], 12);
        assert_eq!(config["runtimePolicy"]["agentKind"], "application");
        assert_eq!(config["runtimePolicy"]["packagePath"], ".");
        assert_eq!(config["runtimePolicy"]["entrypoint"], "agent.md");
        assert_eq!(
            config["runtimePolicy"]["assetAclPath"],
            asset_lifecycle::ASSET_ACL_PATH
        );
        assert!(config["runtimePolicy"].get("runtimeBindingPath").is_none());
        assert!(config["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "Read"));
        assert!(config["tools"]
            .as_array()
            .unwrap()
            .iter()
            .any(|tool| tool["name"] == "Bash"));
        assert_eq!(runtime_binding["kind"], "agent");
        assert_eq!(runtime_binding["enabled"], true);
        assert_eq!(runtime_binding["isolation"], "container");
        assert!(runtime_binding["env"].as_array().is_some());
        assert_eq!(runtime_binding["runtime"]["kind"], "a3s-agent-service");
        assert_eq!(runtime_binding["runtime"]["agentKind"], "application");
        assert_eq!(runtime_binding["runtime"]["mode"], "application-deployment");
        assert_eq!(runtime_binding["resources"]["replicas"], 1);
        assert_eq!(runtime_binding["metadata"]["definitionPath"], "agent.md");
        assert_eq!(
            runtime_binding["metadata"]["assetAclPath"],
            asset_lifecycle::ASSET_ACL_PATH
        );
        let upsert = agent_runtime_binding_upsert_body(&runtime_binding);
        assert!(upsert.get("version").is_none());
        assert_eq!(upsert["kind"], "agent");
        assert_eq!(upsert["isolation"], "container");
        assert_eq!(
            upsert["target"],
            serde_json::json!({
                "kind": "asset",
                "ref": "main",
            })
        );
        assert_eq!(
            upsert["runtime"],
            serde_json::json!({
                "kind": "a3s-agent-service",
                "command": "a3s-agent-service",
            })
        );
        assert_eq!(upsert["resources"]["replicas"], 1);
        assert_eq!(upsert["network"], serde_json::json!({}));
        assert_eq!(upsert["metadata"]["definitionPath"], "agent.md");
        assert_eq!(
            agent_asset_url("https://os.example.com/", "asset 1?#"),
            "https://os.example.com/admin/assets/asset%201%3F%23?embed=1"
        );
        assert_eq!(
            agent_logs_url("https://os.example.com/", AgentOsKind::Agentic, "asset 1"),
            "https://os.example.com/admin/kernel/processes?focus=1&asset=asset%201&agentKind=agentic&logs=1"
        );
        assert_eq!(
            agent_logs_url(
                "https://os.example.com/",
                AgentOsKind::Application,
                "asset 1"
            ),
            "https://os.example.com/admin/infrastructure/batch?asset=asset%201&agentKind=application&logs=1&embed=1"
        );
        assert_eq!(
            agent_logs_url("https://os.example.com/", AgentOsKind::Tool, "asset 1"),
            "https://os.example.com/admin/infrastructure/batch?asset=asset%201&agentKind=tool&category=agent&logs=1&embed=1"
        );
    }

    #[test]
    fn existing_agent_asset_must_match_requested_kind() {
        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "asset-tool",
                        "name": "agentic-review-captain",
                        "category": "agent",
                        "agentKind": "tool"
                    }
                ]
            }
        });

        let err =
            find_agent_asset(&found, "agentic-review-captain", AgentOsKind::Agentic).unwrap_err();
        assert!(err.contains("agentKind=tool"), "{err}");

        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "asset-agentic",
                        "name": "agentic-review-captain",
                        "ownerName": "admin",
                        "defaultBranch": "main",
                        "category": "agent",
                        "agentKind": "agentic"
                    }
                ]
            }
        });
        let asset = find_agent_asset(&found, "agentic-review-captain", AgentOsKind::Agentic)
            .unwrap()
            .unwrap();
        assert_eq!(asset.id, "asset-agentic");
        assert_eq!(asset.owner_name.as_deref(), Some("admin"));
        assert_eq!(asset.default_branch.as_deref(), Some("main"));
    }

    #[test]
    fn existing_agent_asset_must_match_agent_category() {
        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "workflow-asset",
                        "name": "agentic-review-captain",
                        "category": "workflow",
                        "agentKind": "agentic"
                    }
                ]
            }
        });

        let err =
            find_agent_asset(&found, "agentic-review-captain", AgentOsKind::Agentic).unwrap_err();
        assert!(err.contains("category=workflow"), "{err}");
        assert!(err.contains("expected agent"), "{err}");
    }

    #[test]
    fn capability_candidate_selection_prefers_aaas_over_faas() {
        let value = serde_json::json!({
            "data": {
                "items": [
                    {
                        "module": "functions",
                        "operation": "runFunctionBatch",
                        "description": "Function as a Service tool agent batch run"
                    },
                    {
                        "name": "AgentDebugRunController_runAgentic",
                        "resource": "runtimes.agent_debug_runs.agentic",
                        "path": "/api/v1/runtimes/agent-debug-runs/agentic",
                        "description": "Agent as a Service run for agentic agent assets"
                    }
                ]
            }
        });

        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            agent_progressive_score(text, operation, AgentOsAction::Run(AgentOsKind::Agentic))
        });

        assert_eq!(candidates[0].module, "runtimes");
        assert_eq!(
            candidates[0].operation,
            "AgentDebugRunController_runAgentic"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.module != "functions"),
            "tool Function as a Service operations must not be selected for /agent run: {candidates:?}"
        );
        assert!(
            agent_progressive_score(
                "Agent as a Service RUN for Agentic Agent assets",
                "AgentDebugRunController_runAgentic",
                AgentOsAction::Run(AgentOsKind::Agentic),
            ) > 0
        );
        assert_eq!(
            agent_progressive_score(
                "FUNCTION AS A SERVICE tool agent batch run",
                "runFunctionBatch",
                AgentOsAction::Run(AgentOsKind::Agentic),
            ),
            0
        );
    }

    #[test]
    fn tool_run_candidate_selection_uses_faas_operations() {
        let value = serde_json::json!({
            "data": {
                "items": [
                    {
                        "name": "AgentDebugRunController_runAgentic",
                        "resource": "runtimes.agent_debug_runs.agentic",
                        "path": "/api/v1/runtimes/agent-debug-runs/agentic",
                        "description": "Agent as a Service run for agentic agent assets"
                    },
                    {
                        "module": "functions",
                        "operation": "runFunctionBatch",
                        "description": "Function as a Service tool agent run"
                    }
                ]
            }
        });

        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            agent_progressive_score(text, operation, AgentOsAction::Run(AgentOsKind::Tool))
        });

        assert_eq!(candidates[0].module, "functions");
        assert_eq!(candidates[0].operation, "runFunctionBatch");
        assert!(
            agent_progressive_score(
                "FUNCTION AS A SERVICE tool agent run",
                "runFunctionBatch",
                AgentOsAction::Run(AgentOsKind::Tool),
            ) > 0
        );
    }

    #[test]
    fn deploy_candidate_selection_prefers_agent_build_over_generic_deployments() {
        let value = serde_json::json!({
            "data": {
                "results": [
                    {
                        "name": "ResourceDeploymentController_listDeployments",
                        "resource": "infrastructure.deployments",
                        "path": "/api/v1/infrastructure/deployments",
                        "description": "List Deployment resources"
                    },
                    {
                        "name": "ResourceDeploymentController_deleteDeployment",
                        "resource": "infrastructure.deployments",
                        "path": "/api/v1/infrastructure/deployments/{namespace}/{name}",
                        "description": "Delete Deployment"
                    },
                    {
                        "name": "AssetDeployabilityController_evaluate",
                        "resource": "assets.deployability",
                        "path": "/api/v1/assets/{id}/deployability",
                        "description": "Deployment readiness check"
                    },
                    {
                        "name": "AgentBuildController_triggerAgentBuild",
                        "resource": "assets.build.agent",
                        "path": "/api/v1/assets/{owner}/{name}/build/agent",
                        "description": "Trigger application agent build"
                    }
                ]
            }
        });

        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            agent_progressive_score(text, operation, AgentOsAction::Deploy)
        });

        assert_eq!(candidates[0].module, "assets");
        assert_eq!(
            candidates[0].operation,
            "AgentBuildController_triggerAgentBuild"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| { !candidate.operation.contains("ResourceDeploymentController") }),
            "generic infrastructure deployment operations must not drive /agent deploy: {candidates:?}"
        );
    }

    #[test]
    fn agent_observe_candidate_selection_respects_target_service() {
        let asset = AgentAssetRef {
            id: "asset-tool".into(),
            name: "agent-tool-reviewer".into(),
            owner_name: Some("admin".into()),
            default_branch: Some("main".into()),
        };
        let params = agent_observe_progressive_params(
            &asset,
            AgentOsKind::Tool,
            "reviewer",
            AgentOsAction::Open(AgentOsKind::Tool),
        );
        assert_eq!(params["assetId"], "asset-tool");
        assert_eq!(params["agentKind"], "tool");
        assert_eq!(params["operation"], "open");

        let value = serde_json::json!({
            "data": {
                "items": [
                    {
                        "module": "agents",
                        "operation": "AgentController_open",
                        "description": "Agent as a Service open tool agent asset ViewLink"
                    },
                    {
                        "module": "functions",
                        "operation": "FunctionAgentController_openView",
                        "description": "Function as a Service tool agent RemoteUI ViewLink open"
                    },
                    {
                        "module": "assets",
                        "operation": "AssetController_get",
                        "description": "Agent asset metadata"
                    }
                ]
            }
        });

        let tool_candidates = os_progressive::operation_candidates(&value, |text, operation| {
            agent_progressive_score(text, operation, AgentOsAction::Open(AgentOsKind::Tool))
        });
        assert_eq!(
            tool_candidates[0].operation,
            "FunctionAgentController_openView"
        );
        assert!(
            tool_candidates
                .iter()
                .all(|candidate| candidate.module != "agents"),
            "tool-agent observe actions should stay on Function as a Service: {tool_candidates:?}"
        );

        let app_candidates = os_progressive::operation_candidates(&value, |text, operation| {
            agent_progressive_score(
                text,
                operation,
                AgentOsAction::Open(AgentOsKind::Application),
            )
        });
        assert_eq!(app_candidates[0].operation, "AgentController_open");
        assert_eq!(
            agent_progressive_score(
                "Agent as a Service run agentic asset and return ViewLink",
                "AgentDebugRunController_runAgentic",
                AgentOsAction::Open(AgentOsKind::Agentic),
            ),
            0,
            "/agent open must not start an agentic run"
        );
        assert_eq!(
            agent_progressive_score(
                "Agent as a Service apply application configuration and return ViewLink",
                "AgentConfigurationSessionController_apply",
                AgentOsAction::Open(AgentOsKind::Application),
            ),
            0,
            "/agent open must not apply application configuration"
        );
    }

    #[test]
    fn agent_capability_params_follow_described_schema_names() {
        let described = serde_json::json!({
            "data": {
                "operation": {
                    "parameters": {
                        "properties": {
                            "agentAssetID": { "type": "string" },
                            "kind": { "type": "string" },
                            "clientName": { "type": "string" },
                            "optionalNote": { "type": "string" }
                        }
                    }
                }
            }
        });

        let params =
            agent_capability_params(Some(&described), "asset-123", AgentOsKind::Application);

        assert_eq!(params["agentAssetID"], "asset-123");
        assert_eq!(params["kind"], "application");
        assert_eq!(params["clientName"], "a3s-code-tui");
        assert!(params.get("optionalNote").is_none());
    }

    #[test]
    fn agent_capability_params_follow_agentic_debug_run_schema_ref() {
        let described = serde_json::json!({
            "data": {
                "operation": {
                    "inputSchema": {
                        "body": {
                            "$ref": "#/components/schemas/AgenticDebugRunRequestDto"
                        }
                    }
                }
            }
        });

        let params = agent_capability_params(Some(&described), "asset-123", AgentOsKind::Agentic);

        assert_eq!(params, serde_json::json!({ "assetId": "asset-123" }));
    }

    #[tokio::test]
    async fn agent_run_prefers_progressive_capabilities_view() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_capability_mock(captured.clone()).await;
        let client = http().unwrap();

        let (view, note) = try_agent_operation(
            &client,
            &origin,
            "token",
            "asset-123",
            AgentOsAction::Run(AgentOsKind::Agentic),
        )
        .await
        .expect("capabilities run should succeed");

        let requests = captured.lock().unwrap().join("\n");
        assert_eq!(
            view.url,
            format!("{origin}/admin/agents/runs/run-1?embed=1"),
            "{note}\n{requests}"
        );
        assert!(note.contains("progressive capabilities"), "{note}");
        assert!(requests.contains("\"action\":\"search\""), "{requests}");
        assert!(requests.contains("\"action\":\"describe\""), "{requests}");
        assert!(requests.contains("\"action\":\"execute\""), "{requests}");
        assert!(
            requests.contains("\"agentAssetId\":\"asset-123\""),
            "{requests}"
        );
        assert!(
            !requests.contains("/api/v1/agents/asset-123"),
            "REST Agent as a Service fallback should not run after a capabilities view succeeds: {requests}"
        );
    }

    #[tokio::test]
    async fn agent_run_uses_discovered_streaming_endpoint_when_capability_execute_rejects_sse() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_direct_fallback_mock(captured.clone()).await;
        let client = http().unwrap();

        let (view, note) = try_agent_operation(
            &client,
            &origin,
            "token",
            "asset-123",
            AgentOsAction::Run(AgentOsKind::Agentic),
        )
        .await
        .expect("direct streaming fallback should succeed");

        assert_eq!(view.url, format!("{origin}/admin/kernel/processes?focus=1"));
        assert!(note.contains("streaming endpoint"), "{note}");
        let requests = captured.lock().unwrap().join("\n");
        assert!(
            requests.contains("POST /api/v1/runtimes/agent-debug-runs/agentic HTTP/1.1"),
            "{requests}"
        );
        assert!(requests.contains("\"assetId\":\"asset-123\""), "{requests}");
        assert!(
            !requests.contains("\"agentKind\"") && !requests.contains("\"source\""),
            "AgenticDebugRunRequestDto should receive only schema-backed fields: {requests}"
        );
    }

    #[tokio::test]
    async fn agent_deploy_opens_asset_view_when_build_metadata_is_required() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_deploy_planning_mock(captured.clone()).await;
        let client = http().unwrap();

        let (view, note) = try_agent_operation(
            &client,
            &origin,
            "token",
            "asset-123",
            AgentOsAction::Deploy,
        )
        .await
        .expect("deploy planning view should be returned");

        assert_eq!(view.url, format!("{origin}/admin/kernel/assets?focus=1"));
        assert!(note.contains("build/package/namespace"), "{note}");
        let requests = captured.lock().unwrap().join("\n");
        assert!(requests.contains("\"action\":\"search\""), "{requests}");
        assert!(requests.contains("\"action\":\"describe\""), "{requests}");
        assert!(
            !requests.contains("\"action\":\"execute\""),
            "build metadata is missing, so capabilities execute should not be called: {requests}"
        );
    }

    #[tokio::test]
    async fn agent_status_checks_existing_asset_without_mutating_it() {
        let root = temp_root("agent-status");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("reviewer.md");
        let source = r#"---
name: reviewer
description: Review code changes carefully
---
Review the target carefully.
"#;
        std::fs::write(&path, source).unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_agent_status_mock(captured.clone(), true).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "reviewer".into(),
            definition_rel: "reviewer.md".into(),
            path: path.clone(),
            package_path: path.clone(),
            root: root.clone(),
        };

        let result = publish_agent_to_os(session, dev, AgentOsAction::Status(AgentOsKind::Agentic))
            .await
            .expect("status should inspect existing OS asset");

        assert_eq!(result.action, AgentOsAction::Status(AgentOsKind::Agentic));
        assert_eq!(result.asset_id, "asset-123");
        assert!(!result.open_view);
        assert!(result.note.contains("asset exists"), "{}", result.note);
        assert!(
            result.note.contains("agent-config valid"),
            "{}",
            result.note
        );
        assert!(
            result.note.contains("runtime-binding valid"),
            "{}",
            result.note
        );
        let requests = captured.lock().unwrap().join("\n");
        assert!(requests.contains("GET /api/v1/assets?"), "{requests}");
        assert!(
            requests.contains("POST /api/v1/assets/asset-123/agent-config/validate HTTP/1.1"),
            "{requests}"
        );
        assert!(
            requests.contains("GET /api/v1/assets/asset-123/runtime-binding HTTP/1.1"),
            "{requests}"
        );
        assert!(
            requests.contains("POST /api/v1/assets/asset-123/runtime-binding/validate HTTP/1.1"),
            "{requests}"
        );
        assert!(
            !requests.contains("POST /api/v1/assets HTTP/1.1")
                && !requests.contains("/repository/files"),
            "status must not create or upload: {requests}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn agent_status_reports_not_published_without_creating_asset() {
        let root = temp_root("agent-status-missing");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("reviewer.md");
        std::fs::write(
            &path,
            "---\nname: reviewer\ndescription: Review code changes carefully\n---\nReview.\n",
        )
        .unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_agent_status_mock(captured.clone(), false).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "reviewer".into(),
            definition_rel: "reviewer.md".into(),
            path: path.clone(),
            package_path: path.clone(),
            root: root.clone(),
        };

        let result = publish_agent_to_os(session, dev, AgentOsAction::Status(AgentOsKind::Agentic))
            .await
            .expect("status should report missing OS asset");

        assert_eq!(result.asset_id, "not-published");
        assert!(!result.open_view);
        assert!(result.note.contains("no Agentic Agent as a Service asset"));
        assert!(result.note.contains("/agent publish agentic"));
        let requests = captured.lock().unwrap().join("\n");
        assert!(requests.contains("GET /api/v1/assets?"), "{requests}");
        assert!(
            !requests.contains("POST /api/v1/assets HTTP/1.1")
                && !requests.contains("/repository/files")
                && !requests.contains("/agent-config")
                && !requests.contains("/runtime-binding"),
            "missing status must not mutate or validate absent asset: {requests}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn agent_open_observes_existing_asset_without_mutating_it() {
        let root = temp_root("agent-open");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("reviewer.md");
        std::fs::write(
            &path,
            "---\nname: reviewer\ndescription: Review code changes carefully\n---\nReview the target carefully.\n",
        )
        .unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_agent_status_mock(captured.clone(), true).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "reviewer".into(),
            definition_rel: "reviewer.md".into(),
            path: path.clone(),
            package_path: path.clone(),
            root: root.clone(),
        };

        let result = publish_agent_to_os(session, dev, AgentOsAction::Open(AgentOsKind::Agentic))
            .await
            .expect("open should inspect the existing OS asset");

        assert_eq!(result.action, AgentOsAction::Open(AgentOsKind::Agentic));
        assert_eq!(result.asset_id, "asset-123");
        assert!(result.open_view);
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/assets/asset-123?embed=1")
        );
        let requests = captured.lock().unwrap().join("\n");
        assert!(requests.contains("GET /api/v1/assets?"), "{requests}");
        assert!(
            requests.contains("POST /api/v1/kernel/capabilities"),
            "{requests}"
        );
        assert!(
            !requests.contains("POST /api/v1/assets HTTP/1.1")
                && !requests.contains("/repository/files")
                && !requests.contains("/agent-config/validate")
                && !requests.contains("/runtime-binding"),
            "open must not create, upload, or validate: {requests}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn publish_agent_to_os_full_chain_creates_uploads_and_runs_aaas() {
        let root = temp_root("agent-aaas-full");
        let _ = std::fs::remove_dir_all(&root);
        let package = root.join("reviewer");
        let source = r#"---
name: reviewer
description: Review code changes carefully
max_steps: 20
---
Review the target carefully.
"#;
        let path = write_complete_agent_test_package(&package, source);
        std::fs::write(package.join("prompts/checklist.md"), "- inspect tests\n").unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_full_agent_os_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "reviewer".into(),
            definition_rel: "agent.md".into(),
            path: path.clone(),
            package_path: package.clone(),
            root: root.clone(),
        };

        let result = publish_agent_to_os(session, dev, AgentOsAction::Run(AgentOsKind::Agentic))
            .await
            .expect("full /agent run chain should succeed");

        assert_eq!(result.asset_name, "agentic-reviewer");
        assert_eq!(result.asset_id, "asset-123");
        assert_eq!(result.kind, AgentOsKind::Agentic);
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/agents/runs/run-1?embed=1")
        );
        assert!(result.note.contains("agent config was synced"));
        assert!(
            result.note.contains("runtime binding was synced"),
            "{}",
            result.note
        );
        let local_asset_acl = package.join(asset_lifecycle::ASSET_ACL_PATH);
        assert!(local_asset_acl.is_file(), "missing local asset.acl");
        let local_asset_acl_body = std::fs::read_to_string(&local_asset_acl).unwrap();
        assert!(local_asset_acl_body.contains("category = \"agent\""));
        assert!(local_asset_acl_body.contains("definition_path = \"agent.md\""));

        let requests = captured.lock().unwrap().clone();
        let joined = requests.join("\n");
        assert!(joined.contains("GET /api/v1/assets?"), "{joined}");
        assert!(joined.contains("POST /api/v1/assets HTTP/1.1"), "{joined}");
        assert!(
            joined.contains("POST /api/v1/assets/asset-123/repository/files HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/assets/asset-123/agent-config/validate HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/asset-123/agent-config HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/asset-123/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/assets/asset-123/runtime-binding/validate HTTP/1.1"),
            "{joined}"
        );
        assert!(joined.contains("\"action\":\"execute\""), "{joined}");
        assert!(
            joined.contains("\"agentAssetId\":\"asset-123\""),
            "{joined}"
        );

        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "agent");
        assert_eq!(create_json["agentKind"], "agentic");
        assert_eq!(create_json["name"], "agentic-reviewer");
        assert_eq!(create_json["metadata"]["service"], "Agent as a Service");
        assert_eq!(create_json["metadata"]["agentKind"], "agentic");
        assert_eq!(create_json["metadata"]["runtimeKind"], "a3s-agent-service");
        assert!(create_json["metadata"].get("protocol").is_none());
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");
        assert_eq!(
            create_json["metadata"]["agentContract"]["sessionCreate"],
            "/api/agent/sessions"
        );

        let patch = request_body(&requests, "PATCH /api/v1/assets/asset-123 HTTP/1.1");
        let patch_json: serde_json::Value = serde_json::from_str(&patch).unwrap();
        assert_eq!(
            patch_json["metadata"]["agentContract"]["sessionCreate"],
            "/api/agent/sessions"
        );

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/asset-123/repository/files HTTP/1.1",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let files = upload_json["files"].as_array().unwrap();
        assert!(files.iter().any(|file| file["path"] == "agent.md"));
        assert!(files
            .iter()
            .any(|file| file["path"] == "prompts/checklist.md"));
        assert!(files
            .iter()
            .any(|file| file["path"] == asset_lifecycle::ASSET_ACL_PATH));
        assert!(files.iter().all(|file| {
            !file["path"]
                .as_str()
                .unwrap_or_default()
                .starts_with(".a3s/agents/")
        }));
        for forbidden in [
            "agent.asset.json",
            "agent.config.json",
            "agent.runtime-binding.json",
            "runtime-binding.json",
        ] {
            assert!(
                files.iter().all(|file| file["path"] != forbidden),
                "repository upload should not include {forbidden}"
            );
        }

        let validate = request_body(
            &requests,
            "POST /api/v1/assets/asset-123/agent-config/validate HTTP/1.1",
        );
        let validate_json: serde_json::Value = serde_json::from_str(&validate).unwrap();
        assert_eq!(validate_json["mode"], "replace");
        assert_eq!(
            validate_json["systemPrompt"],
            "Review the target carefully."
        );
        let synced = request_body(
            &requests,
            "PUT /api/v1/assets/asset-123/agent-config HTTP/1.1",
        );
        let synced_json: serde_json::Value = serde_json::from_str(&synced).unwrap();
        assert_eq!(synced_json["runtimePolicy"]["agentKind"], "agentic");
        assert_eq!(
            synced_json["runtimePolicy"]["assetAclPath"],
            asset_lifecycle::ASSET_ACL_PATH
        );
        assert!(synced_json["runtimePolicy"]
            .get("runtimeBindingPath")
            .is_none());
        let binding = request_body(
            &requests,
            "PUT /api/v1/assets/asset-123/runtime-binding HTTP/1.1",
        );
        let binding_json: serde_json::Value = serde_json::from_str(&binding).unwrap();
        assert_eq!(binding_json["kind"], "agent");
        assert!(binding_json["runtime"].get("agentKind").is_none());
        assert_eq!(binding_json["metadata"]["definitionPath"], "agent.md");
        assert!(binding_json["metadata"].get("configPath").is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn publish_tool_agent_uses_function_as_a_service_without_agent_config_sync() {
        let root = temp_root("agent-tool-faas");
        let _ = std::fs::remove_dir_all(&root);
        let package = root.join("tooler");
        let source = r#"---
name: tooler
description: Run reusable tool actions
max_steps: 8
---
Run the requested tool action carefully.
"#;
        let path = write_complete_agent_test_package(&package, source);
        std::fs::create_dir_all(package.join("schemas")).unwrap();
        std::fs::write(package.join("schemas/input.json"), "{}\n").unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_tool_agent_publish_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = AgentDevSession {
            name: "tooler".into(),
            description: "Run reusable tool actions".into(),
            rel: "tooler".into(),
            definition_rel: "agent.md".into(),
            path: path.clone(),
            package_path: package.clone(),
            root: root.clone(),
        };

        let result = publish_agent_to_os(session, dev, AgentOsAction::Publish(AgentOsKind::Tool))
            .await
            .expect("tool agent publish should use Function as a Service");

        assert_eq!(result.asset_name, "agent-tool-tooler");
        assert_eq!(result.asset_id, "asset-tool");
        assert_eq!(result.kind, AgentOsKind::Tool);
        assert!(
            result.note.contains("Function as a Service"),
            "{}",
            result.note
        );
        assert!(
            !result.note.contains("agent config was synced"),
            "{}",
            result.note
        );
        assert!(
            result.note.contains("asset config was saved"),
            "{}",
            result.note
        );
        assert!(
            result.note.contains("runtime binding was synced"),
            "{}",
            result.note
        );

        let requests = captured.lock().unwrap().clone();
        let joined = requests.join("\n");
        assert!(joined.contains("POST /api/v1/assets HTTP/1.1"), "{joined}");
        assert!(
            joined.contains("POST /api/v1/assets/asset-tool/repository/files HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/asset-tool/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/assets/asset-tool/runtime-binding/validate HTTP/1.1"),
            "{joined}"
        );
        assert!(
            !joined.contains("/agent-config"),
            "tool agents must not call Agent as a Service agent-config endpoints: {joined}"
        );

        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "agent");
        assert_eq!(create_json["agentKind"], "tool");
        assert_eq!(create_json["name"], "agent-tool-tooler");
        assert_eq!(create_json["metadata"]["service"], "Function as a Service");
        assert_eq!(create_json["metadata"]["agentKind"], "tool");
        assert_eq!(
            create_json["metadata"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(create_json["metadata"]["protocol"], "agent-tool");
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");
        assert_eq!(
            create_json["metadata"]["agentContract"]["run"],
            "/api/agent/run"
        );

        let patch = request_body(&requests, "PATCH /api/v1/assets/asset-tool HTTP/1.1");
        let patch_json: serde_json::Value = serde_json::from_str(&patch).unwrap();
        assert_eq!(
            patch_json["metadata"]["agentContract"]["run"],
            "/api/agent/run"
        );

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/asset-tool/repository/files HTTP/1.1",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let files = upload_json["files"].as_array().unwrap();
        assert!(files.iter().any(|file| file["path"] == "agent.md"));
        assert!(files
            .iter()
            .any(|file| file["path"] == "schemas/input.json"));
        assert!(files
            .iter()
            .any(|file| file["path"] == asset_lifecycle::ASSET_ACL_PATH));
        for forbidden in [
            "agent.asset.json",
            "agent.config.json",
            "agent.runtime-binding.json",
            "runtime-binding.json",
        ] {
            assert!(
                files.iter().all(|file| file["path"] != forbidden),
                "repository upload should not include {forbidden}"
            );
        }

        let synced = request_body(
            &requests,
            "PUT /api/v1/assets/asset-tool/runtime-binding HTTP/1.1",
        );
        let synced_json: serde_json::Value = serde_json::from_str(&synced).unwrap();
        assert_eq!(synced_json["kind"], "tool");
        assert_eq!(synced_json["runtime"]["kind"], "a3s-function-service");
        assert!(synced_json["runtime"].get("protocol").is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deploy_rejects_packages_declared_as_tool_or_agentic() {
        for (decl_key, decl_kind) in [("kind", "tool"), ("agent_kind", "agentic")] {
            let root = temp_root(&format!("agent-deploy-rejects-{decl_kind}"));
            let _ = std::fs::remove_dir_all(&root);
            let package = root.join(decl_kind);
            std::fs::create_dir_all(package.join(".a3s")).unwrap();
            let path = package.join("agent.md");
            std::fs::write(
                &path,
                format!("---\nname: {decl_kind}-agent\ndescription: Not an app\n---\nDo work.\n"),
            )
            .unwrap();
            std::fs::write(
                package.join(asset_lifecycle::ASSET_ACL_PATH),
                format!(
                    "version = \"a3s.asset.v1\"\ncategory = \"agent\"\n{decl_key} = \"{decl_kind}\"\n"
                ),
            )
            .unwrap();
            let dev = AgentDevSession {
                name: format!("{decl_kind}-agent"),
                description: "Not an app".into(),
                rel: decl_kind.into(),
                definition_rel: "agent.md".into(),
                path: path.clone(),
                package_path: package.clone(),
                root: root.clone(),
            };
            let session = crate::a3s_os::StoredOsSession {
                address: "http://127.0.0.1:9".into(),
                access_token: "token".into(),
                refresh_token: None,
                token_type: Some("Bearer".into()),
                expires_at_ms: None,
                account_label: None,
                login_at_ms: 1,
            };

            let err = publish_agent_to_os(session, dev, AgentOsAction::Deploy)
                .await
                .expect_err("deploy should reject non-application packages before OS calls");
            assert!(err.contains("only for application agents"), "{err}");
            assert!(err.contains(decl_kind), "{err}");

            let _ = std::fs::remove_dir_all(&root);
        }
    }

    #[tokio::test]
    async fn publish_agent_to_os_application_deploy_builds_and_launches_when_metadata_is_available()
    {
        let root = temp_root("agent-aaas-build");
        let _ = std::fs::remove_dir_all(&root);
        let package = root.join("reviewer");
        let source = r#"---
name: reviewer
description: Review code changes carefully
max_steps: 20
---
Review the target carefully.
"#;
        let path = write_complete_agent_test_package(&package, source);
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_application_build_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = AgentDevSession {
            name: "reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "reviewer".into(),
            definition_rel: "agent.md".into(),
            path: path.clone(),
            package_path: package.clone(),
            root: root.clone(),
        };

        let result = publish_agent_to_os(session, dev, AgentOsAction::Deploy)
            .await
            .expect("application /agent deploy should trigger OS build");

        assert_eq!(result.asset_name, "agent-app-reviewer");
        assert_eq!(result.asset_id, "asset-app");
        assert_eq!(result.kind, AgentOsKind::Application);
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/kernel/processes?focus=1")
        );
        assert!(
            result.note.contains("built and launched"),
            "{}",
            result.note
        );
        assert!(result.note.contains("agent config was synced"));
        assert!(result.note.contains("runtime binding was synced"));

        let requests = captured.lock().unwrap().clone();
        let joined = requests.join("\n");
        assert!(
            joined.contains("GET /api/v1/assets/asset-app/branches/main HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/assets/admin/agent-app-reviewer/build/agent HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("GET /api/v1/runtimes/namespaces?limit=100 HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/runtimes/namespaces/default/agents/launch HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/asset-app/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/assets/asset-app/runtime-binding/validate HTTP/1.1"),
            "{joined}"
        );
        assert!(
            !joined.contains("POST /api/v1/kernel/capabilities"),
            "direct build should avoid the planning fallback once it succeeds: {joined}"
        );
        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "agent");
        assert_eq!(create_json["agentKind"], "application");
        assert_eq!(create_json["metadata"]["service"], "Agent as a Service");
        assert_eq!(create_json["metadata"]["agentKind"], "application");
        assert_eq!(create_json["metadata"]["runtimeKind"], "a3s-agent-service");
        assert!(create_json["metadata"].get("protocol").is_none());
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");
        assert_eq!(
            create_json["metadata"]["agentContract"]["sessionCreate"],
            "/api/agent/sessions"
        );
        let patch = request_body(&requests, "PATCH /api/v1/assets/asset-app HTTP/1.1");
        let patch_json: serde_json::Value = serde_json::from_str(&patch).unwrap();
        assert_eq!(
            patch_json["metadata"]["agentContract"]["sessionCreate"],
            "/api/agent/sessions"
        );
        let build = request_body(
            &requests,
            "POST /api/v1/assets/admin/agent-app-reviewer/build/agent HTTP/1.1",
        );
        let build_json: serde_json::Value = serde_json::from_str(&build).unwrap();
        assert_eq!(build_json["commitSha"], "commit-app");
        assert_eq!(build_json["branch"], "main");
        assert!(build_json["buildNumber"].as_u64().is_some());
        let launch = request_body(
            &requests,
            "POST /api/v1/runtimes/namespaces/default/agents/launch HTTP/1.1",
        );
        let launch_json: serde_json::Value = serde_json::from_str(&launch).unwrap();
        assert_eq!(launch_json["packageId"], "agents/admin/agent-app-reviewer");
        assert_eq!(launch_json["version"], "commit-app-1");
        assert_eq!(launch_json["name"], "agent-app-reviewer");
        assert_eq!(launch_json["replicas"], 1);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn sync_agent_runtime_binding_reports_unsupported_endpoint() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_runtime_binding_unsupported_mock(captured.clone()).await;
        let binding = serde_json::json!({
            "kind": "agent",
            "isolation": "serving",
            "target": { "kind": "asset", "ref": "main" },
            "runtime": { "kind": "a3s-agent-service" },
            "env": [],
            "requiredSecrets": [],
            "resources": {},
            "network": {},
            "enabled": true,
            "metadata": { "agentKind": "agentic" },
        });

        let result = sync_agent_runtime_binding(&origin, "token", "asset 123", &binding).await;

        assert_eq!(result, AgentRuntimeBindingSync::Unsupported);
        let requests = captured.lock().unwrap().join("\n");
        assert!(
            requests.contains("PUT /api/v1/assets/asset%20123/runtime-binding HTTP/1.1"),
            "{requests}"
        );
    }

    async fn spawn_capability_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = capability_mock_response(&line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_runtime_binding_unsupported_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let payload = r#"{"code":404,"message":"not found"}"#;
                    let resp = format!(
                        "HTTP/1.1 404 Not Found\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_agent_status_mock(
        captured: Arc<Mutex<Vec<String>>>,
        asset_exists: bool,
    ) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = agent_status_mock_response(&line, &body, asset_exists);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_deploy_planning_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = deploy_planning_mock_response(&line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_direct_fallback_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, content_type, payload) =
                        direct_fallback_mock_response(&line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: {content_type}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_full_agent_os_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 32768];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = full_agent_os_mock_response(&line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_tool_agent_publish_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 32768];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = tool_agent_publish_mock_response(&line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    async fn spawn_application_build_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 32768];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = application_build_mock_response(&line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    fn request_body(requests: &[String], prefix: &str) -> String {
        requests
            .iter()
            .find(|request| request.starts_with(prefix))
            .and_then(|request| request.split_once('\n').map(|(_, body)| body.to_string()))
            .unwrap_or_else(|| panic!("missing request {prefix}; got:\n{}", requests.join("\n")))
    }

    fn agent_status_mock_response(
        line: &str,
        body: &str,
        asset_exists: bool,
    ) -> (&'static str, &'static str) {
        if line.starts_with("GET /api/v1/assets?") {
            if asset_exists {
                return (
                    "200 OK",
                    r#"{"data":{"items":[{"id":"asset-123","name":"agentic-reviewer","ownerName":"admin","defaultBranch":"main","category":"agent","agentKind":"agentic"}]}}"#,
                );
            }
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1")
            || line.contains("/repository/files")
            || line.starts_with("PUT /api/v1/assets/asset-123/agent-config HTTP/1.1")
            || line.starts_with("PUT /api/v1/assets/asset-123/runtime-binding HTTP/1.1")
        {
            return (
                "500 Internal Server Error",
                r#"{"code":500,"message":"status mock forbids writes"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-123/agent-config/validate HTTP/1.1") {
            if body.contains(r#""systemPrompt":"Review the target carefully.""#)
                && body.contains(r#""mode":"replace""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-123","assetName":"agentic-reviewer","valid":true,"diagnostics":[],"summary":{},"validatedAt":"2026-01-01T00:00:00Z"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad config status body"}"#,
            );
        }
        if line.starts_with("GET /api/v1/assets/asset-123/runtime-binding HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"asset-123","configured":true,"binding":{"kind":"agent","isolation":"serving"},"validation":{"assetId":"asset-123","configured":true,"valid":true,"requiredSecrets":[],"missingSecrets":[],"expiredSecrets":[],"issues":[],"checkedAt":"2026-01-01T00:00:00Z"}}}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-123/runtime-binding/validate HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"asset-123","configured":true,"valid":true,"requiredSecrets":[],"missingSecrets":[],"expiredSecrets":[],"issues":[],"checkedAt":"2026-01-01T00:00:00Z"}}"#,
            );
        }
        ("404 Not Found", r#"{"code":404,"message":"not found"}"#)
    }

    fn capability_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if !line.starts_with("POST /api/v1/kernel/capabilities ") {
            return ("404 Not Found", r#"{"code":404,"message":"not found"}"#);
        }
        if body.contains(r#""action":"search""#) {
            return (
                "200 OK",
                r#"{"code":200,"data":{"results":[{"name":"AgentDebugRunController_runAgentic","resource":"runtimes.agent_debug_runs.agentic","path":"/api/v1/runtimes/agent-debug-runs/agentic","action":"stream","method":"POST","description":"Agent as a Service run for agentic agent assets"}]}}"#,
            );
        }
        if body.contains(r#""action":"describe""#) {
            return (
                "200 OK",
                r#"{"code":200,"data":{"operation":{"parameters":{"properties":{"agentAssetId":{"type":"string"},"agentKind":{"type":"string"},"source":{"type":"string"}}}}}}"#,
            );
        }
        if body.contains(r#""action":"execute""#) {
            return (
                "200 OK",
                r#"{"code":200,"data":{"runId":"run-1"},"view":{"url":"/admin/agents/runs/run-1?embed=1","width":1280,"height":860}}"#,
            );
        }
        (
            "400 Bad Request",
            r#"{"code":400,"message":"bad mock request"}"#,
        )
    }

    fn direct_fallback_mock_response(
        line: &str,
        body: &str,
    ) -> (&'static str, &'static str, &'static str) {
        if line.starts_with("POST /api/v1/runtimes/agent-debug-runs/agentic HTTP/1.1") {
            return ("201 Created", "text/event-stream", "");
        }
        if !line.starts_with("POST /api/v1/kernel/capabilities ") {
            return (
                "404 Not Found",
                "application/json",
                r#"{"code":404,"message":"not found"}"#,
            );
        }
        if body.contains(r#""action":"search""#) {
            return (
                "200 OK",
                "application/json",
                r#"{"code":200,"data":{"results":[{"name":"AgentDebugRunController_runAgentic","resource":"runtimes.agent_debug_runs.agentic","path":"/api/v1/runtimes/agent-debug-runs/agentic","method":"POST","description":"Agent as a Service run for agentic agent assets"}]}}"#,
            );
        }
        if body.contains(r#""action":"describe""#) {
            return (
                "200 OK",
                "application/json",
                r##"{"code":200,"data":{"success":true,"module":"runtimes","operation":{"name":"AgentDebugRunController_runAgentic","method":"POST","path":"/api/v1/runtimes/agent-debug-runs/agentic","inputSchema":{"body":{"$ref":"#/components/schemas/AgenticDebugRunRequestDto"}}},"view":{"url":"/admin/kernel/processes?focus=1","width":1440,"height":900}}}"##,
            );
        }
        if body.contains(r#""action":"execute""#) {
            return (
                "400 Bad Request",
                "application/json",
                r#"{"code":400,"message":"validation failed"}"#,
            );
        }
        (
            "400 Bad Request",
            "application/json",
            r#"{"code":400,"message":"bad mock request"}"#,
        )
    }

    fn deploy_planning_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if !line.starts_with("POST /api/v1/kernel/capabilities ") {
            return ("404 Not Found", r#"{"code":404,"message":"not found"}"#);
        }
        if body.contains(r#""action":"search""#) {
            return (
                "200 OK",
                r#"{"code":200,"data":{"results":[{"name":"ResourceDeploymentController_listDeployments","resource":"infrastructure.deployments","path":"/api/v1/infrastructure/deployments","description":"List deployments"},{"name":"AgentBuildController_triggerAgentBuild","resource":"assets.build.agent","path":"/api/v1/assets/{owner}/{name}/build/agent","method":"POST","description":"Trigger application agent build"}]}}"#,
            );
        }
        if body.contains(r#""action":"describe""#) {
            return (
                "200 OK",
                r##"{"code":200,"data":{"success":true,"module":"assets","operation":{"name":"AgentBuildController_triggerAgentBuild","method":"POST","path":"/api/v1/assets/{owner}/{name}/build/agent","inputSchema":{"body":{"$ref":"#/components/schemas/TriggerAgentBuildRequestDto"}}},"view":{"url":"/admin/kernel/assets?focus=1","width":1440,"height":900}}}"##,
            );
        }
        (
            "400 Bad Request",
            r#"{"code":400,"message":"bad mock request"}"#,
        )
    }

    fn full_agent_os_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""agentKind":"agentic""#)
                && body.contains(r#""category":"agent""#)
                && body.contains(r#""service":"Agent as a Service""#)
                && body.contains(r#""runtimeKind":"a3s-agent-service""#)
                && body.contains(r#""createdBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"asset-123","name":"agentic-reviewer","ownerName":"admin","defaultBranch":"main"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad asset body"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-123/repository/files HTTP/1.1") {
            if body.contains(asset_lifecycle::ASSET_ACL_PATH)
                && body.contains(r#""path":"agent.md""#)
                && !body.contains("agent.asset.json")
                && !body.contains("agent.config.json")
                && !body.contains("agent.runtime-binding.json")
            {
                return ("200 OK", r#"{"ok":true}"#);
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad upload body"}"#,
            );
        }
        if line.starts_with("PATCH /api/v1/assets/asset-123 HTTP/1.1") {
            if body.contains(r#""agentContract""#)
                && body.contains(r#""sessionCreate":"/api/agent/sessions""#)
                && body.contains(r#""updatedBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"asset-123","name":"agentic-reviewer","metadata":{"agentContract":{"sessionCreate":"/api/agent/sessions"}}}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad asset metadata patch"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-123/agent-config/validate HTTP/1.1") {
            if body.contains(r#""systemPrompt":"Review the target carefully.""#)
                && body.contains(r#""mode":"replace""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-123","assetName":"agentic-reviewer","valid":true,"diagnostics":[],"summary":{},"validatedAt":"2026-01-01T00:00:00Z"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad config"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/asset-123/agent-config HTTP/1.1") {
            if body.contains(r#""systemPrompt":"Review the target carefully.""#)
                && body.contains(r#""agentKind":"agentic""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-123","assetName":"agentic-reviewer","config":{}}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad config"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/asset-123/runtime-binding HTTP/1.1") {
            if body.contains(r#""kind":"agent""#)
                && body.contains(r#""isolation":"serving""#)
                && body.contains(r#""agentKind":"agentic""#)
                && body.contains(r#""sharedRuntime":"node-20""#)
                && body.contains(r#""env":[]"#)
                && !body.contains(r#""version""#)
                && !body.contains(r#""mode""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-123","configured":true}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad runtime binding"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-123/runtime-binding/validate HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"asset-123","configured":true,"valid":true,"requiredSecrets":[],"missingSecrets":[],"expiredSecrets":[],"issues":[],"checkedAt":"2026-01-01T00:00:00Z"}}"#,
            );
        }
        capability_mock_response(line, body)
    }

    fn tool_agent_publish_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if line.contains("/agent-config") {
            return (
                "500 Internal Server Error",
                r#"{"code":500,"message":"tool agent mock forbids agent-config endpoints"}"#,
            );
        }
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""agentKind":"tool""#)
                && body.contains(r#""category":"agent""#)
                && body.contains(r#""service":"Function as a Service""#)
                && body.contains(r#""runtimeKind":"a3s-function-service""#)
                && body.contains(r#""protocol":"agent-tool""#)
                && body.contains(r#""createdBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"asset-tool","name":"agent-tool-tooler","ownerName":"admin","defaultBranch":"main"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad tool agent asset body"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-tool/repository/files HTTP/1.1") {
            if body.contains(asset_lifecycle::ASSET_ACL_PATH)
                && body.contains(r#""path":"agent.md""#)
                && !body.contains("agent.asset.json")
                && !body.contains("agent.config.json")
                && !body.contains("agent.runtime-binding.json")
            {
                return ("200 OK", r#"{"ok":true}"#);
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad tool agent upload body"}"#,
            );
        }
        if line.starts_with("PATCH /api/v1/assets/asset-tool HTTP/1.1") {
            if body.contains(r#""agentContract""#)
                && body.contains(r#""run":"/api/agent/run""#)
                && body.contains(r#""updatedBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"asset-tool","name":"agent-tool-tooler","metadata":{"agentContract":{"run":"/api/agent/run"}}}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad tool agent metadata patch"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/asset-tool/runtime-binding HTTP/1.1") {
            if body.contains(r#""kind":"tool""#)
                && body.contains(r#""isolation":"serving""#)
                && body.contains(r#""kind":"a3s-function-service""#)
                && body.contains(r#""sharedRuntime":"node-20""#)
                && !body.contains(r#""version""#)
                && !body.contains(r#""mode""#)
                && !body.contains(r#""protocol":"agent-tool""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-tool","configured":true}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad tool runtime binding"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-tool/runtime-binding/validate HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"asset-tool","configured":true,"valid":true,"requiredSecrets":[],"missingSecrets":[],"expiredSecrets":[],"issues":[],"checkedAt":"2026-01-01T00:00:00Z"}}"#,
            );
        }
        ("404 Not Found", r#"{"code":404,"message":"not found"}"#)
    }

    fn application_build_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""agentKind":"application""#)
                && body.contains(r#""category":"agent""#)
                && body.contains(r#""service":"Agent as a Service""#)
                && body.contains(r#""runtimeKind":"a3s-agent-service""#)
                && body.contains(r#""createdBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"asset-app","name":"agent-app-reviewer","ownerName":"admin","defaultBranch":"main"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad asset body"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-app/repository/files HTTP/1.1") {
            if body.contains(asset_lifecycle::ASSET_ACL_PATH)
                && body.contains(r#""path":"agent.md""#)
                && !body.contains("agent.asset.json")
                && !body.contains("agent.config.json")
                && !body.contains("agent.runtime-binding.json")
            {
                return ("200 OK", r#"{"ok":true}"#);
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad upload body"}"#,
            );
        }
        if line.starts_with("PATCH /api/v1/assets/asset-app HTTP/1.1") {
            if body.contains(r#""agentContract""#)
                && body.contains(r#""sessionCreate":"/api/agent/sessions""#)
                && body.contains(r#""updatedBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"asset-app","name":"agent-app-reviewer","metadata":{"agentContract":{"sessionCreate":"/api/agent/sessions"}}}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad app agent metadata patch"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-app/agent-config/validate HTTP/1.1") {
            if body.contains(r#""agentKind":"application""#) && body.contains(r#""mode":"replace""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-app","assetName":"agent-app-reviewer","valid":true,"diagnostics":[],"summary":{},"validatedAt":"2026-01-01T00:00:00Z"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad config"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/asset-app/agent-config HTTP/1.1") {
            if body.contains(r#""systemPrompt":"Review the target carefully.""#)
                && body.contains(r#""agentKind":"application""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-app","assetName":"agent-app-reviewer","config":{}}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad config"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/asset-app/runtime-binding HTTP/1.1") {
            if body.contains(r#""kind":"agent""#)
                && body.contains(r#""isolation":"container""#)
                && body.contains(r#""agentKind":"application""#)
                && body.contains(r#""command":"a3s-agent-service""#)
                && body.contains(r#""replicas":1"#)
                && !body.contains(r#""version""#)
                && !body.contains(r#""mode""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"asset-app","configured":true}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad runtime binding"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/asset-app/runtime-binding/validate HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"asset-app","configured":true,"valid":true,"requiredSecrets":[],"missingSecrets":[],"expiredSecrets":[],"issues":[],"checkedAt":"2026-01-01T00:00:00Z"}}"#,
            );
        }
        if line.starts_with("GET /api/v1/assets/asset-app/branches/main HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"id":"branch-main","assetId":"asset-app","name":"main","commitSha":"commit-app"}}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/admin/agent-app-reviewer/build/agent HTTP/1.1") {
            if body.contains(r#""commitSha":"commit-app""#)
                && body.contains(r#""branch":"main""#)
                && body.contains(r#""buildNumber":"#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"success":true,"repository":"agents/admin/agent-app-reviewer","version":"commit-app-1"},"view":{"url":"/admin/kernel/assets?focus=1","width":1440,"height":900}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad build body"}"#,
            );
        }
        if line.starts_with("GET /api/v1/runtimes/namespaces?limit=100 HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"items":[{"id":"system","name":"system","isDefault":false},{"id":"default","name":"default","isDefault":true}]}}"#,
            );
        }
        if line.starts_with("POST /api/v1/runtimes/namespaces/default/agents/launch HTTP/1.1") {
            if body.contains(r#""packageId":"agents/admin/agent-app-reviewer""#)
                && body.contains(r#""version":"commit-app-1""#)
                && body.contains(r#""name":"agent-app-reviewer""#)
            {
                return (
                    "201 Created",
                    r#"{"code":201,"data":{"deploymentId":"deploy-1","name":"agent-app-reviewer","packageId":"agents/admin/agent-app-reviewer","packageVersion":"commit-app-1","status":"running"},"view":{"url":"/admin/kernel/processes?focus=1","width":1440,"height":900}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad launch body"}"#,
            );
        }
        ("404 Not Found", r#"{"code":404,"message":"not found"}"#)
    }

    #[test]
    fn agent_dev_prompt_keeps_work_local_and_names_exit_path() {
        let session = AgentDevSession {
            name: "code-reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "review/code-reviewer".into(),
            definition_rel: "agent.md".into(),
            path: std::path::PathBuf::from("/Users/x/.a3s/agents/review/code-reviewer/agent.md"),
            package_path: std::path::PathBuf::from("/Users/x/.a3s/agents/review/code-reviewer"),
            root: std::path::PathBuf::from("/Users/x/.a3s/agents"),
        };
        let p = agent_dev_prompt(&session, "add a security checklist");
        assert!(p.contains("code-reviewer"), "{p}");
        assert!(p.contains("add a security checklist"), "{p}");
        assert!(
            p.contains("/Users/x/.a3s/agents/review/code-reviewer/agent.md"),
            "{p}"
        );
        assert!(p.contains("Do not open OS"), "{p}");
        assert!(p.contains("/agent off") && p.contains("Esc"), "{p}");
    }

    #[test]
    fn agent_goal_label_scopes_goal_to_active_agent() {
        let session = AgentDevSession {
            name: "code-reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "review/code-reviewer".into(),
            definition_rel: "agent.md".into(),
            path: std::path::PathBuf::from("/Users/x/.a3s/agents/review/code-reviewer/agent.md"),
            package_path: std::path::PathBuf::from("/Users/x/.a3s/agents/review/code-reviewer"),
            root: std::path::PathBuf::from("/Users/x/.a3s/agents"),
        };

        assert_eq!(
            agent_goal_label(&session, "tighten security checks."),
            "Agent `code-reviewer` goal: tighten security checks"
        );
    }

    #[test]
    fn agent_loop_prompt_keeps_engineered_loop_local_and_agent_scoped() {
        let session = AgentDevSession {
            name: "code-reviewer".into(),
            description: "Review code changes carefully".into(),
            rel: "review/code-reviewer".into(),
            definition_rel: "agent.md".into(),
            path: std::path::PathBuf::from("/Users/x/.a3s/agents/review/code-reviewer/agent.md"),
            package_path: std::path::PathBuf::from("/Users/x/.a3s/agents/review/code-reviewer"),
            root: std::path::PathBuf::from("/Users/x/.a3s/agents"),
        };
        let p = agent_loop_prompt(&session, "Run this A3S Code engineered loop.");

        assert!(p.contains("loop engineering inside local /agent"), "{p}");
        assert!(p.contains("code-reviewer"), "{p}");
        assert!(
            p.contains("/Users/x/.a3s/agents/review/code-reviewer/agent.md"),
            "{p}"
        );
        assert!(p.contains("Stay local"), "{p}");
        assert!(p.contains("do not open OS, WebIDE, RemoteUI"), "{p}");
        assert!(p.contains("maker/checker"), "{p}");
        assert!(p.contains("Run this A3S Code engineered loop."), "{p}");
    }
}
