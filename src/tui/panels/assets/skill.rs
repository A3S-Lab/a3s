//! `/skill` local skill asset authoring.
//!
//! Skills are team digital assets too: a short description should be enough to
//! create a local authoring prototype with Function as a Service binding intent.
//! They can be reviewed, published, and deployed, but they are not direct run or
//! execution targets in the TUI.

use super::super::asset_lifecycle;
use super::super::os_progressive;
use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;

#[derive(Clone)]
pub(crate) struct SkillAsset {
    pub(crate) rel: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) name: String,
    pub(crate) description: String,
}

pub(crate) struct SkillPanel {
    pub(crate) root: std::path::PathBuf,
    pub(crate) skills: Vec<SkillAsset>,
    pub(crate) sel: usize,
}

#[derive(Clone)]
pub(crate) struct SkillDevSession {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) rel: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) root: std::path::PathBuf,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum SkillSubcommand {
    Exit,
    Clone(String),
    List(String),
    Review,
    Activity(String),
    Publish,
    Deploy,
    Open,
    Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SkillOsAction {
    Publish,
    Deploy,
    Open,
    Status,
}

impl SkillOsAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Deploy => "deploy",
            Self::Open => "open",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct SkillOsResult {
    pub(crate) action: SkillOsAction,
    pub(crate) asset_name: String,
    pub(crate) asset_id: String,
    pub(crate) view: remote_ui::ViewSpec,
    pub(crate) note: String,
    pub(crate) open_view: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillAssetRef {
    id: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillSourceFile {
    path: String,
    bytes: Vec<u8>,
}

pub(crate) fn parse_skill_subcommand(input: &str) -> Option<Result<SkillSubcommand, String>> {
    let mut parts = input.split_whitespace();
    let head = parts.next()?.to_ascii_lowercase();
    match head.as_str() {
        "off" => {
            if parts.next().is_some() {
                return Some(Err("usage: /skill off".to_string()));
            }
            Some(Ok(SkillSubcommand::Exit))
        }
        "exit" | "normal" | "clear" | "stop" => Some(Err("usage: /skill off".to_string())),
        "clone" => {
            let Some(url) = parts.next() else {
                return Some(Err("usage: /skill clone <git-url>".to_string()));
            };
            if parts.next().is_some() {
                return Some(Err("usage: /skill clone <git-url>".to_string()));
            }
            Some(Ok(SkillSubcommand::Clone(url.to_string())))
        }
        "list" => Some(Ok(SkillSubcommand::List(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "review" => {
            if parts.next().is_some() {
                return Some(Err("usage: /skill review".to_string()));
            }
            Some(Ok(SkillSubcommand::Review))
        }
        "activity" => Some(Ok(SkillSubcommand::Activity(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "ps" | "runs" | "jobs" => Some(Err("usage: /skill activity [query]".to_string())),
        "publish" => {
            if parts.next().is_some() {
                return Some(Err("usage: /skill publish".to_string()));
            }
            Some(Ok(SkillSubcommand::Publish))
        }
        "run" => Some(Err(
            "skills are not runnable assets; use /skill publish or /skill deploy".to_string(),
        )),
        "debug" => Some(Err("unknown /skill command `debug`".to_string())),
        "deploy" => {
            if parts.next().is_some() {
                return Some(Err("usage: /skill deploy".to_string()));
            }
            Some(Ok(SkillSubcommand::Deploy))
        }
        "open" => {
            if parts.next().is_some() {
                return Some(Err("usage: /skill open".to_string()));
            }
            Some(Ok(SkillSubcommand::Open))
        }
        "logs" => Some(Err(
            "skills do not expose runtime logs; use /skill status".to_string()
        )),
        "status" => {
            if parts.next().is_some() {
                return Some(Err("usage: /skill status".to_string()));
            }
            Some(Ok(SkillSubcommand::Status))
        }
        "inspect" => Some(Err("usage: /skill status".to_string())),
        "view" | "remote" => Some(Err("usage: /skill open".to_string())),
        "os" => Some(Err("usage: /skill status".to_string())),
        "dashboard" => Some(Err("usage: /skill list [query] · /skill status".to_string())),
        _ => None,
    }
}

pub(crate) fn list_skill_assets(root: &std::path::Path) -> Vec<SkillAsset> {
    let mut out = Vec::new();
    list_skill_assets_inner(root, root, &mut out);
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out
}

fn list_skill_assets_inner(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<SkillAsset>,
) {
    let entrypoint = dir.join("SKILL.md");
    if entrypoint.is_file() {
        out.push(skill_asset_from_file(root, &entrypoint));
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if !name.starts_with('.') {
                list_skill_assets_inner(root, &path, out);
            }
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("md") && name != "README.md"
        {
            out.push(skill_asset_from_file(root, &path));
        }
    }
}

fn skill_asset_from_file(root: &std::path::Path, path: &std::path::Path) -> SkillAsset {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|part| part.as_os_str().to_string_lossy())
        .collect::<Vec<_>>()
        .join("/");
    let body = std::fs::read_to_string(path).unwrap_or_default();
    let (name, description) = skill_meta(&body).unwrap_or_else(|| {
        let name = path
            .parent()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .or_else(|| path.file_stem().and_then(|n| n.to_str()))
            .unwrap_or("skill")
            .to_string();
        (name, "Local skill asset".to_string())
    });
    SkillAsset {
        rel,
        path: path.to_path_buf(),
        name,
        description,
    }
}

pub(crate) fn skill_dev_session_from_file(
    root: &std::path::Path,
    path: &std::path::Path,
) -> SkillDevSession {
    let asset = skill_asset_from_file(root, path);
    SkillDevSession {
        name: asset.name,
        description: asset.description,
        rel: asset.rel,
        path: asset.path,
        root: root.to_path_buf(),
    }
}

pub(crate) fn scaffold_skill_asset(
    description: &str,
    root: &std::path::Path,
) -> Result<SkillDevSession, String> {
    let name = asset_lifecycle::scaffold_name(description, "skill");
    let package = asset_lifecycle::unique_asset_dir(root, &name);
    let final_name = package
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name.as_str())
        .to_string();
    let description =
        asset_lifecycle::scaffold_description(description, &final_name, "Local skill asset");

    std::fs::create_dir_all(package.join(".a3s"))
        .map_err(|e| format!("could not create {}: {e}", package.join(".a3s").display()))?;
    for dir in ["examples", "tests"] {
        std::fs::create_dir_all(package.join(dir))
            .map_err(|e| format!("could not create {}: {e}", package.join(dir).display()))?;
    }

    let skill_md = format!(
        "---\n\
         name: {name}\n\
         description: {description_json}\n\
         kind: instruction\n\
         allowed-tools: \"Read(*), Grep(*), Glob(*)\"\n\
         ---\n\n\
         # {name}\n\n\
         Use this skill when the user asks for: {description}.\n\n\
         ## Workflow\n\n\
         1. Read the relevant input and restate the task boundary.\n\
         2. Apply the checklist in this package.\n\
         3. Return concise, evidence-backed output.\n\n\
         ## Success Criteria\n\n\
         - The response is scoped to the requested task.\n\
         - Assumptions and missing evidence are stated plainly.\n",
        name = final_name,
        description = description,
        description_json = serde_json::to_string(&description)
            .unwrap_or_else(|_| "\"Local skill asset\"".to_string()),
    );
    let skill_path = package.join("SKILL.md");
    asset_lifecycle::write_scaffold_file(&skill_path, skill_md.as_bytes())?;
    let dev = skill_dev_session_from_file(root, &skill_path);
    let asset_name = skill_asset_name(&dev.name);
    let asset_acl = skill_asset_acl(&dev, &asset_name);

    asset_lifecycle::write_scaffold_file(
        &package.join("README.md"),
        skill_scaffold_readme(&final_name, &description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("examples/example-input.md"),
        format!("# Example Input\n\n{description}\n").as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("examples/example-output.md"),
        b"# Example Output\n\nA concise result with evidence and assumptions.\n",
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("tests/smoke.md"),
        skill_scaffold_tests().as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join(asset_lifecycle::ASSET_ACL_PATH),
        asset_acl.as_bytes(),
    )?;

    Ok(dev)
}

fn skill_scaffold_readme(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n\
         {description}.\n\n\
         ## Source\n\n\
         - `SKILL.md` is the skill source entrypoint.\n\
         - `examples/` and `tests/` contain reusable fixtures.\n\
         - `.a3s/` contains only `asset.acl`.\n\n\
         ## Lifecycle\n\n\
         - `a3s code skill publish .`\n\
         - `a3s code skill deploy .`\n\
         - `a3s code skill status .`\n"
    )
}

fn skill_scaffold_tests() -> &'static str {
    "# Skill Smoke Checklist\n\n\
     1. Confirm `SKILL.md` has valid YAML frontmatter with `kind: instruction`.\n\
     2. Confirm `.a3s/asset.acl` has `definition_path = \"SKILL.md\"`.\n\
     3. Run `/reload` before using the skill in the TUI.\n"
}

fn skill_meta(body: &str) -> Option<(String, String)> {
    let rest = body.trim_start().strip_prefix("---")?;
    let end = rest.find("\n---")?;
    let mut name = None;
    let mut description = None;
    for line in rest[..end].lines() {
        if let Some(value) = line.strip_prefix("name:") {
            name = Some(value.trim().trim_matches(['"', '\'']).to_string());
        } else if let Some(value) = line.strip_prefix("description:") {
            description = Some(value.trim().trim_matches(['"', '\'']).to_string());
        }
    }
    Some((
        name?.trim().to_string(),
        description.unwrap_or_else(|| "Local skill asset".to_string()),
    ))
}

#[cfg(test)]
pub(crate) fn skill_gen_prompt(description: &str, dir: &str) -> String {
    format!(
        "Create a local A3S skill asset prototype from the description below and save it under \
         {dir}. This is a local authoring task: do not open OS, RemoteUI, or a browser.\n\
         Description: {description}\n\
         IMPORTANT: {dir} is OUTSIDE this session's workspace by default, so path-scoped file \
         tools will reject writes there. Use the `bash` tool for ALL file creation and edits under \
         {dir}; do not use path-scoped write/edit tools for this task. Use non-interactive bash \
         commands with full quoted paths. Never run a command that waits on stdin.\n\
         Create {dir}/<kebab-case-name>/ with at least:\n\
         - SKILL.md using valid YAML frontmatter. Use this exact frontmatter shape for a normal \
         skill:\n\
           ---\n\
           name: <kebab-case-name>\n\
           description: <one-line trigger/purpose>\n\
           kind: instruction\n\
           allowed-tools: \"Read(*), Grep(*), Glob(*)\"\n\
           ---\n\
           Do NOT use `kind: skill`; valid kind values are only `instruction`, `persona`, or \
           `tool`, and normal reusable skills should be `instruction`. Do not invent semantic \
           tool names such as `generate_object`; allowed-tools must be concrete tool permission \
           patterns.\n\
         - README.md explaining the skill contract, expected inputs, outputs, and examples.\n\
         - examples/example-input.md and examples/example-output.md.\n\
         - tests/smoke.md with a short manual verification checklist.\n\
         - .a3s/asset.acl as the unified asset manifest for this skill.\n\
         The package-local `.a3s/` directory is metadata-only. Do NOT put `SKILL.md`, README, \
         examples, tests, or other skill source files under `.a3s/`.\n\
         Do NOT create extra generated JSON config files; keep package configuration in \
         `.a3s/asset.acl`. Runtime configuration is synced through OS Function as a Service \
         APIs during publish/deploy, not stored in the asset repository.\n\
         Keep the first version small and usable: one clear trigger, one workflow, conservative \
         tool scope, explicit success criteria, and no secrets. Confirm SKILL.md contains \
         `kind: instruction`, then stop using \
         tools immediately and give a concise final answer with the saved project path and the \
         note that `/reload` makes the new skill available in the TUI."
    )
}

pub(crate) fn skill_dev_prompt(session: &SkillDevSession, request: &str) -> String {
    format!(
        "You are in A3S Code local skill-development mode.\n\
         Current skill: {name}\n\
         Description: {description}\n\
         Skill file: {path}\n\
         Skill root: {root}\n\n\
         User request:\n{request}\n\n\
         Work on this local skill asset iteratively. Read SKILL.md before editing, keep YAML \
         frontmatter valid, preserve a clear trigger description and conservative tool scope, \
         keep examples/tests useful, and maintain Function as a Service metadata when this skill \
         exposes tool-like execution. Do not open OS, RemoteUI, or browser pages for this local \
         skill-dev turn. End with a concise summary and next lifecycle step.\n\n\
         The TUI remains in skill-development mode for `{name}` after this turn; the user can \
         press Esc or run `/skill off` to return to normal mode.",
        name = session.name.as_str(),
        description = session.description.as_str(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

pub(crate) fn skill_review_prompt(path: &std::path::Path, body: &str) -> String {
    let asset_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let contract = super::review::review_report_contract(asset_dir);
    format!(
        "Review this local skill asset without changing files unless the user explicitly asks for \
         fixes.\n\
         Skill path: {path}\n\n\
         Skill definition:\n```markdown\n{body}\n```\n\n\
         Report concise findings on: YAML frontmatter validity, trigger description, tool scope, \
         workflow clarity, examples, tests, safety constraints, reusable team context, and \
         readiness for Function as a Service when the skill exposes tool-like execution. Mention \
         the smallest recommended improvements and whether `/skill publish` or `/skill deploy` is \
         the right next lifecycle step.{contract}",
        path = path.display(),
    )
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

fn envelope_data(v: &serde_json::Value) -> &serde_json::Value {
    v.get("data").unwrap_or(v)
}

fn items_of(v: &serde_json::Value) -> Vec<serde_json::Value> {
    let data = envelope_data(v);
    for ptr in ["/items", "/list", "/results", "/rows", "/assets"] {
        if let Some(items) = data.pointer(ptr).and_then(|a| a.as_array()) {
            return items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect();
        }
    }
    data.as_array()
        .map(|items| {
            items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect()
        })
        .unwrap_or_default()
}

fn envelope_json_is_error(value: &serde_json::Value) -> bool {
    if value.get("error").is_some() || value.get("errors").is_some() {
        return true;
    }
    let code = value
        .get("code")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("statusCode").and_then(|v| v.as_i64()))
        .unwrap_or(200);
    code >= 400
}

fn envelope_text_is_error(text: &str) -> bool {
    serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .is_some_and(|value| envelope_json_is_error(&value))
}

fn response_message(value: &serde_json::Value) -> String {
    json_str_at(value, &["/message", "message", "/error/message", "error"])
        .unwrap_or("unknown error")
        .to_string()
}

fn http() -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder.build().map_err(|e| e.to_string())
}

fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn skill_asset_name(name: &str) -> String {
    format!("skill-{}", asset_slug(name))
}

fn skill_asset_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/assets/{}?embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn skill_asset_search_url(origin: &str, asset_name: &str) -> String {
    format!(
        "{}/admin/kernel/assets?focus=1&category=skill&scope=mine&status=all&search={}&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_name)
    )
}

fn skill_view_spec(url: String) -> remote_ui::ViewSpec {
    remote_ui::ViewSpec {
        url,
        width: Some(1440),
        height: Some(900),
        embeddable: true,
    }
}

fn skill_asset_root(dev: &SkillDevSession) -> std::path::PathBuf {
    if dev.path.file_name().and_then(|name| name.to_str()) == Some("SKILL.md") {
        dev.path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| dev.root.clone())
    } else {
        dev.path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| dev.root.clone())
    }
}

fn collect_skill_source_files(root: &std::path::Path) -> Result<Vec<SkillSourceFile>, String> {
    let mut out = Vec::new();
    collect_skill_source_files_inner(root, root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_skill_source_files_inner(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<SkillSourceFile>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".git" | "target" | "node_modules" | ".venv" | "__pycache__"
            ) {
                continue;
            }
            collect_skill_source_files_inner(root, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if rel.starts_with(".a3s/")
            || matches!(
                rel.as_str(),
                "skill.asset.json" | "skill.runtime-binding.json" | "runtime-binding.json"
            )
        {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        if bytes.len() > 512 * 1024 {
            continue;
        }
        out.push(SkillSourceFile { path: rel, bytes });
    }
    Ok(())
}

fn skill_manifest_json(dev: &SkillDevSession, asset_name: &str) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.skill.asset.v1",
        "category": "skill",
        "name": asset_name,
        "skillName": dev.name.as_str(),
        "description": dev.description.as_str(),
        "definitionPath": "SKILL.md",
        "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        "localPath": dev.rel.as_str(),
        "service": "Function as a Service",
        "createdBy": "a3s-code-tui",
        "runtimeIntent": {
            "kind": "tool",
            "isolation": "serving",
            "agentKind": "tool",
            "runtimeKind": "a3s-function-service",
            "protocol": "skill",
        },
    })
}

fn skill_runtime_binding_json(dev: &SkillDevSession, asset_name: &str) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.skill.runtime-binding.v1",
        "kind": "tool",
        "enabled": true,
        "isolation": "serving",
        "target": {
            "kind": "asset",
            "ref": "main",
            "definitionPath": "SKILL.md",
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        },
        "runtime": {
            "kind": "a3s-function-service",
            "protocol": "skill",
            "agentKind": "tool",
        },
        "env": [],
        "requiredSecrets": [],
        "resources": {},
        "network": {},
        "metadata": {
            "source": "a3s-code-tui",
            "service": "Function as a Service",
            "assetName": asset_name,
            "skillName": dev.name.as_str(),
            "description": dev.description.as_str(),
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
            "localPath": dev.rel.as_str(),
        },
    })
}

fn skill_asset_acl(dev: &SkillDevSession, asset_name: &str) -> String {
    let source = [("definition_path", "SKILL.md")];
    let metadata: [(&str, &str); 0] = [];
    asset_lifecycle::render_asset_acl(asset_lifecycle::AssetAclDocument {
        category: "skill",
        kind: Some("tool"),
        name: asset_name,
        description: dev.description.as_str(),
        local_path: Some(dev.rel.as_str()),
        service: asset_lifecycle::OsService::FunctionAsAService,
        runtime: asset_lifecycle::RuntimeBindingIntent {
            kind: "tool",
            isolation: "serving",
            runtime_kind: "a3s-function-service",
            protocol: Some("skill"),
            agent_kind: Some("tool"),
        },
        source: &source,
        metadata: &metadata,
    })
}

fn skill_runtime_binding_upsert_body(runtime_binding: &serde_json::Value) -> serde_json::Value {
    let target_ref = runtime_binding
        .pointer("/target/ref")
        .and_then(|value| value.as_str())
        .unwrap_or("main");
    let runtime_kind = runtime_binding
        .pointer("/runtime/kind")
        .and_then(|value| value.as_str())
        .unwrap_or("a3s-function-service");
    serde_json::json!({
        "kind": runtime_binding
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or("tool"),
        "isolation": runtime_binding
            .get("isolation")
            .and_then(|value| value.as_str())
            .unwrap_or("serving"),
        "target": {
            "kind": "asset",
            "ref": target_ref,
        },
        "runtime": {
            "kind": runtime_kind,
            "sharedRuntime": runtime_binding
                .pointer("/runtime/sharedRuntime")
                .and_then(|value| value.as_str())
                .unwrap_or("node-20"),
        },
        "env": runtime_binding
            .get("env")
            .filter(|value| value.is_array())
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
        "requiredSecrets": runtime_binding
            .get("requiredSecrets")
            .filter(|value| value.is_array())
            .cloned()
            .unwrap_or_else(|| serde_json::json!([])),
        "resources": runtime_binding
            .get("resources")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
        "network": {},
        "enabled": runtime_binding
            .get("enabled")
            .and_then(|value| value.as_bool())
            .unwrap_or(true),
        "metadata": runtime_binding
            .get("metadata")
            .filter(|value| value.is_object())
            .cloned()
            .unwrap_or_else(|| serde_json::json!({})),
    })
}

fn skill_asset_ref_from_value(
    value: &serde_json::Value,
    fallback_name: &str,
) -> Option<SkillAssetRef> {
    Some(SkillAssetRef {
        id: json_str_at(value, &["/id", "id", "/_id", "_id", "/assetId", "assetId"])?.to_string(),
        name: json_str_at(value, &["/name", "name", "/displayName", "displayName"])
            .unwrap_or(fallback_name)
            .to_string(),
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

fn find_skill_asset(
    value: &serde_json::Value,
    name: &str,
) -> Result<Option<SkillAssetRef>, String> {
    let exact = items_of(value)
        .into_iter()
        .find(|item| json_str_at(item, &["/name", "name"]) == Some(name));
    let Some(asset) = exact else {
        return Ok(None);
    };
    if let Some(actual) = asset_category(&asset) {
        if !actual.eq_ignore_ascii_case("skill") {
            return Err(category_conflict_error(name, actual, "skill"));
        }
    }
    skill_asset_ref_from_value(&asset, name)
        .map(Some)
        .ok_or_else(|| format!("asset `{name}` matched but had no id"))
}

async fn ensure_skill_asset(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    name: &str,
    description: &str,
) -> Result<SkillAssetRef, String> {
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", name),
            ("category", "skill"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(asset) = find_skill_asset(&found, name)? {
        return Ok(asset);
    }
    let resp = client
        .post(&base)
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "ownerType": "user",
            "category": "skill",
            "visibility": "private",
            "description": description,
            "metadata": {
                "service": "Function as a Service",
                "agentKind": "tool",
                "runtimeKind": "a3s-function-service",
                "protocol": "skill",
                "createdBy": "a3s-code-tui",
            },
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() || envelope_json_is_error(&body) {
        return Err(format!(
            "create skill asset failed ({status}): {}",
            response_message(&body)
        ));
    }
    skill_asset_ref_from_value(envelope_data(&body), name)
        .ok_or_else(|| "create skill asset: no id in response".to_string())
}

async fn upload_skill_asset(
    origin: &str,
    token: &str,
    asset_id: &str,
    source_files: &[SkillSourceFile],
    asset_acl: &str,
    _manifest: &serde_json::Value,
    _runtime_binding: &serde_json::Value,
) -> Result<(), String> {
    use base64::Engine;

    let mut files = Vec::new();
    for file in source_files {
        files.push(serde_json::json!({
            "path": file.path,
            "contentBase64": base64::engine::general_purpose::STANDARD.encode(&file.bytes),
        }));
    }
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
            "message": "a3s code /skill: update skill asset",
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
        "upload skill asset failed ({status}): {}",
        truncate(&body, 200)
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum SkillRuntimeBindingSync {
    Synced,
    Unsupported,
    Failed(String),
}

async fn sync_skill_runtime_binding(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> SkillRuntimeBindingSync {
    match sync_skill_runtime_binding_inner(origin, token, asset_id, runtime_binding).await {
        Ok(true) => SkillRuntimeBindingSync::Synced,
        Ok(false) => SkillRuntimeBindingSync::Unsupported,
        Err(err) => SkillRuntimeBindingSync::Failed(err),
    }
}

async fn sync_skill_runtime_binding_inner(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> Result<bool, String> {
    let client = http()?;
    let base = format!(
        "{}/api/v1/assets/{}/runtime-binding",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    );
    let put_resp = client
        .put(&base)
        .bearer_auth(token)
        .json(&skill_runtime_binding_upsert_body(runtime_binding))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let put_status = put_resp.status();
    let put_text = put_resp.text().await.unwrap_or_default();
    if matches!(put_status.as_u16(), 404 | 405) {
        return Ok(false);
    }
    if !put_status.is_success() || envelope_text_is_error(&put_text) {
        return Err(format!("OS runtime-binding sync failed ({put_status})"));
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
    if !validate_status.is_success() || envelope_text_is_error(&validate_text) {
        return Err(format!(
            "OS runtime-binding validation failed ({validate_status})"
        ));
    }
    Ok(true)
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
    if !status.is_success() || envelope_text_is_error(&text) {
        return format!("runtime-binding read failed ({status})");
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
    if !status.is_success() || envelope_text_is_error(&text) {
        return format!("runtime-binding validation failed ({status})");
    }
    "runtime-binding valid".to_string()
}

async fn inspect_skill_asset(
    origin: &str,
    token: &str,
    action: SkillOsAction,
    asset_name: &str,
    skill_name: &str,
) -> Result<SkillOsResult, String> {
    let client = http()?;
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", asset_name),
            ("category", "skill"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let Some(asset) = find_skill_asset(&found, asset_name)? else {
        return Ok(SkillOsResult {
            action,
            asset_name: asset_name.to_string(),
            asset_id: "not-published".to_string(),
            view: skill_view_spec(skill_asset_search_url(origin, asset_name)),
            note: format!(
                "OS status for `{skill_name}`: no Function as a Service skill asset named `{asset_name}` was found. Run `/skill publish` first."
            ),
            open_view: false,
        });
    };
    let (view, note, open_view) = match action {
        SkillOsAction::Open => {
            let fallback = skill_view_spec(skill_asset_url(origin, &asset.id));
            let (view, note) =
                try_skill_progressive_action(&client, origin, token, &asset, skill_name, action)
                    .await
                    .unwrap_or_else(|| {
                        (
                    fallback,
                    "Opened OS skill asset through the Function as a Service skill asset view."
                        .to_string(),
                )
                    });
            (view, note, true)
        }
        SkillOsAction::Status => {
            let binding_status =
                runtime_binding_validation_status(&client, origin, token, &asset.id).await;
            (
                skill_view_spec(skill_asset_url(origin, &asset.id)),
                format!("OS status for `{skill_name}`: asset exists; {binding_status}."),
                false,
            )
        }
        SkillOsAction::Publish | SkillOsAction::Deploy => unreachable!(),
    };
    Ok(SkillOsResult {
        action,
        asset_name: asset.name,
        asset_id: asset.id,
        view,
        note,
        open_view,
    })
}

fn skill_progressive_params(
    asset: &SkillAssetRef,
    skill_name: &str,
    action: SkillOsAction,
) -> serde_json::Value {
    serde_json::json!({
        "functionRef": asset.name,
        "ref": asset.name,
        "name": asset.name,
        "assetId": asset.id,
        "assetName": asset.name,
        "operation": action.label(),
        "input": {
            "skillName": skill_name,
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "payload": {
            "skillName": skill_name,
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "agentKind": "tool",
        "timeoutMs": 120000,
        "idempotencyKey": format!("a3s-code-skill-{}-{}", action.label(), unix_timestamp_secs()),
    })
}

fn skill_progressive_score(action: SkillOsAction, text: &str, operation: &str) -> i32 {
    let combined = format!("{text} {operation}").to_ascii_lowercase();
    let operation = operation.to_ascii_lowercase();
    if matches!(action, SkillOsAction::Open | SkillOsAction::Deploy)
        && (operation.contains("createasset")
            || operation.contains("listasset")
            || operation.contains("getasset")
            || operation.contains("deployability")
            || operation.contains("diagnose")
            || operation.contains("acknowledge")
            || operation.contains("issue")
            || operation.contains("reopen")
            || operation.contains("marketplace")
            || operation.contains("listing")
            || operation.contains("publishing")
            || operation.contains("publish")
            || operation.contains("scaffold")
            || operation.contains("template")
            || operation.contains("preview"))
    {
        return 0;
    }
    if matches!(action, SkillOsAction::Open) && is_mutating_skill_observe_operation(&combined) {
        return 0;
    }
    let mut score = 0;
    if combined.contains("function") || combined.contains("faas") {
        score += 8;
    }
    if combined.contains("skill") {
        score += 8;
    }
    let mut action_hit = false;
    match action {
        SkillOsAction::Open => {
            if (operation.contains("open") && !operation.contains("reopen"))
                || (operation.contains("view") && !operation.contains("preview"))
                || combined.contains("remoteui")
                || combined.contains("skill asset view")
            {
                score += 8;
                action_hit = true;
            }
        }
        SkillOsAction::Deploy
            if operation.contains("deploy")
                || operation.contains("serve")
                || operation.contains("serving")
                || operation.contains("release")
                || operation.contains("binding") =>
        {
            score += 8;
            action_hit = true;
        }
        _ => {}
    }
    if combined.contains("view") || combined.contains("shaped") {
        score += 3;
    }
    if combined.contains("mcp")
        || combined.contains("workflow")
        || combined.contains("agent as a service")
    {
        score -= 10;
    }
    if matches!(action, SkillOsAction::Open | SkillOsAction::Deploy) && !action_hit {
        0
    } else if score >= 8 {
        score
    } else {
        0
    }
}

fn is_mutating_skill_observe_operation(combined: &str) -> bool {
    let has_safe_observe_hint = combined.contains("open")
        || combined.contains("view")
        || combined.contains("get")
        || combined.contains("list")
        || combined.contains("inspect")
        || combined.contains("log");
    let has_mutating_hint = combined.contains("create")
        || combined.contains("update")
        || combined.contains("delete")
        || combined.contains("apply")
        || combined.contains("publish")
        || combined.contains("deploy")
        || combined.contains("build")
        || combined.contains("launch")
        || combined.contains("run")
        || combined.contains("trigger")
        || combined.contains("batch")
        || combined.contains("validate")
        || combined.contains("acknowledge")
        || combined.contains("reopen")
        || combined.contains("close")
        || combined.contains("resolve")
        || combined.contains("issue");
    has_mutating_hint && !has_safe_observe_hint
        || combined.contains("reopen")
        || combined.contains("issue")
}

async fn try_skill_progressive_action(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset: &SkillAssetRef,
    skill_name: &str,
    action: SkillOsAction,
) -> Option<(remote_ui::ViewSpec, String)> {
    let query = match action {
        SkillOsAction::Open => "Function as a Service open skill asset shaped ViewLink",
        SkillOsAction::Deploy => "Function as a Service deploy skill asset shaped ViewLink",
        SkillOsAction::Publish | SkillOsAction::Status => return None,
    };
    let execution = os_progressive::execute_first_matching(
        client,
        origin,
        token,
        query,
        skill_progressive_params(asset, skill_name, action),
        |text, operation| skill_progressive_score(action, text, operation),
    )
    .await?;
    let fallback = match action {
        SkillOsAction::Open | SkillOsAction::Deploy => {
            skill_view_spec(skill_asset_url(origin, &asset.id))
        }
        _ => unreachable!(),
    };
    Some((
        execution.view.unwrap_or(fallback),
        format!(
            "OS Function as a Service accepted skill `{}` through progressive capabilities (`{}`).",
            action.label(),
            execution.operation.operation
        ),
    ))
}

fn append_skill_runtime_binding_sync_note(
    mut note: String,
    runtime_binding_synced: &SkillRuntimeBindingSync,
) -> String {
    match runtime_binding_synced {
        SkillRuntimeBindingSync::Synced => note.push_str(" OS runtime binding was synced."),
        SkillRuntimeBindingSync::Unsupported => note.push_str(
            " OS runtime-binding endpoint was unavailable; runtime-binding intent was saved.",
        ),
        SkillRuntimeBindingSync::Failed(err) => note.push_str(&format!(
            " OS runtime binding could not be synced: {}; runtime-binding intent was saved.",
            truncate(err, 160)
        )),
    }
    note
}

pub(crate) async fn publish_skill_to_os(
    session: crate::a3s_os::StoredOsSession,
    dev: SkillDevSession,
    action: SkillOsAction,
) -> Result<SkillOsResult, String> {
    let origin = crate::a3s_os::os_origin(&session.address);
    let asset_name = skill_asset_name(&dev.name);
    if matches!(action, SkillOsAction::Open | SkillOsAction::Status) {
        return inspect_skill_asset(
            &origin,
            &session.access_token,
            action,
            &asset_name,
            &dev.name,
        )
        .await;
    }
    let client = http()?;
    let asset = ensure_skill_asset(
        &client,
        &origin,
        &session.access_token,
        &asset_name,
        &dev.description,
    )
    .await?;
    let asset_root = skill_asset_root(&dev);
    let source_files = collect_skill_source_files(&asset_root)?;
    let manifest = skill_manifest_json(&dev, &asset_name);
    let runtime_binding = skill_runtime_binding_json(&dev, &asset_name);
    let asset_acl = skill_asset_acl(&dev, &asset_name);
    asset_lifecycle::write_asset_acl(&asset_root, &asset_acl)?;
    upload_skill_asset(
        &origin,
        &session.access_token,
        &asset.id,
        &source_files,
        &asset_acl,
        &manifest,
        &runtime_binding,
    )
    .await?;
    let runtime_binding_synced =
        sync_skill_runtime_binding(&origin, &session.access_token, &asset.id, &runtime_binding)
            .await;
    let (view, note) = match action {
        SkillOsAction::Publish => (
            skill_view_spec(skill_asset_url(&origin, &asset.id)),
            format!(
                "Published `{}` as an OS skill asset backed by Function as a Service.",
                dev.name
            ),
        ),
        SkillOsAction::Deploy => try_skill_progressive_action(
            &client,
            &origin,
            &session.access_token,
            &asset,
            &dev.name,
            action,
        )
        .await
        .unwrap_or_else(|| {
            (
                skill_view_spec(skill_asset_url(&origin, &asset.id)),
                format!(
                    "Deployed `{}` by publishing its serving skill runtime binding for Function as a Service.",
                    dev.name
                ),
            )
        }),
        SkillOsAction::Open | SkillOsAction::Status => {
            unreachable!("read-only skill actions return before publish flow")
        }
    };
    let note = append_skill_runtime_binding_sync_note(note, &runtime_binding_synced);
    Ok(SkillOsResult {
        action,
        asset_name,
        asset_id: asset.id,
        view,
        note,
        open_view: true,
    })
}

fn skill_picker_header(total: usize, root: &std::path::Path, width: usize) -> String {
    truncate(
        &format!(
            "  ✦ skill — select a skill asset ({total} in {})",
            root.to_string_lossy()
        ),
        width,
    )
}

fn skill_picker_hint(width: usize) -> String {
    truncate("  ↑/↓ select · Enter local skill dev · Esc cancel", width)
}

fn skill_picker_lines(
    skills: &[SkillAsset],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) = skill_picker_panel(skills, selected, root, width, height)
    else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn skill_picker_panel(
    skills: &[SkillAsset],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Option<(MenuPanel, usize)> {
    let total = skills.len();
    if total == 0 {
        return None;
    }
    let max_items = height.saturating_sub(8).clamp(3, 12);
    let selected = selected.min(total.saturating_sub(1));
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let items = skills
        .iter()
        .map(|skill| MenuItem::new(skill.rel.clone()).description(skill.description.clone()))
        .collect::<Vec<_>>();

    let panel = MenuPanel::new(skill_picker_header(total, root, width).trim_start())
        .subtitle(skill_picker_hint(width).trim_start())
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

fn skill_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    pub(crate) fn on_skill_os_completed(
        &mut self,
        status_entry: TranscriptEntryId,
        res: Result<SkillOsResult, String>,
    ) {
        match res {
            Ok(result) => {
                self.last_view = Some(result.view.clone());
                self.replace_tracked_line(
                    status_entry,
                    &gutter(
                        TN_CYAN,
                        &format!(
                            "✦ /skill {} · `{}` ({})",
                            result.action.label(),
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
                        &remote_view_button("skill Function as a Service · click to reopen"),
                    ));
                    self.open_remote_view(&result.view);
                } else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  Open view opens the related OS skill asset view"),
                    );
                }
            }
            Err(e) => {
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  /skill OS operation failed: {e}")),
                );
            }
        }
    }

    pub(crate) fn exit_skill_dev(&mut self) {
        match self.skill_dev.take() {
            Some(session) => self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  skill dev off — {} ({})",
                session.name, session.rel
            ))),
            None => self.push_line(&Style::new().fg(TN_GRAY).render("  skill dev is not active")),
        }
        self.relayout();
    }

    pub(crate) fn open_skill_panel(&mut self) {
        let root = self.asset_directories.skill.clone();
        let skills = list_skill_assets(&root);
        if skills.is_empty() {
            self.pending_skill_subcommand = None;
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  no skills in {} — draft one with `/skill <description>` first",
                root.display()
            )));
            return;
        }
        self.skill_picker = Some(SkillPanel {
            root,
            skills,
            sel: 0,
        });
    }

    pub(crate) fn handle_skill_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let panel = self.skill_picker.as_mut()?;
        let last = panel.skills.len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => panel.sel = panel.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.sel = (panel.sel + 1).min(last),
            KeyCode::Esc => {
                cancel_pending_picker(&mut self.skill_picker, &mut self.pending_skill_subcommand)
            }
            KeyCode::Enter => {
                let panel = self.skill_picker.take()?;
                let selected = panel.sel.min(last);
                return self.activate_skill_panel_selection(panel, selected);
            }
            _ => {}
        }
        None
    }

    pub(crate) fn handle_skill_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let panel_state = self.skill_picker.as_ref()?;
        let total = panel_state.skills.len();
        if total == 0 {
            return None;
        }
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let selected = panel_state.sel.min(total - 1);
        let (mut panel, panel_height) = skill_picker_panel(
            &panel_state.skills,
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
            skill_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                let panel_state = self.skill_picker.take()?;
                self.activate_skill_panel_selection(panel_state, index.min(total - 1))
            }
            Some(MenuPanelMsg::Cancelled) => {
                cancel_pending_picker(&mut self.skill_picker, &mut self.pending_skill_subcommand);
                None
            }
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    if let Some(open) = self.skill_picker.as_mut() {
                        open.sel = after;
                    }
                }
                None
            }
        }
    }

    fn activate_skill_panel_selection(
        &mut self,
        panel: SkillPanel,
        selected: usize,
    ) -> Option<Cmd<Msg>> {
        let last = panel.skills.len().saturating_sub(1);
        let picked = panel.skills.get(selected.min(last))?.clone();
        self.agent_dev = None;
        self.mcp_dev = None;
        self.okf_dev = None;
        self.skill_dev = Some(SkillDevSession {
            name: picked.name.clone(),
            description: picked.description.clone(),
            rel: picked.rel.clone(),
            path: picked.path.clone(),
            root: panel.root,
        });
        self.push_line(&gutter(
            TN_CYAN,
            &format!(
                "✦ skill dev: {} ({}) · Esc or /skill off returns to normal mode",
                picked.name, picked.rel
            ),
        ));
        self.relayout();
        if let Some(pending) = self.pending_skill_subcommand.take() {
            return self.execute_skill_subcommand(pending);
        }
        None
    }

    pub(crate) fn overlay_skill_menu(&self, composed: String) -> String {
        let Some(panel) = self.skill_picker.as_ref() else {
            return composed;
        };
        let width = self.width as usize;
        let menu = skill_picker_lines(
            &panel.skills,
            panel.sel,
            &panel.root,
            width,
            self.height as usize,
        );
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn skill_gen_prompt_carries_asset_and_faas_contract() {
        let prompt = skill_gen_prompt("triage production incidents", "/Users/x/.a3s/skills");
        assert!(prompt.contains("/Users/x/.a3s/skills"));
        assert!(prompt.contains("SKILL.md"));
        assert!(prompt.contains(".a3s/asset.acl"));
        assert!(prompt.contains("kind: instruction"));
        assert!(prompt.contains("valid kind values are only `instruction`, `persona`, or `tool`"));
        assert!(prompt.contains("Do NOT use `kind: skill`"));
        assert!(prompt.contains("metadata-only"));
        assert!(prompt.contains("Do NOT put `SKILL.md`"));
        assert!(prompt.contains("Do not invent semantic tool names such as `generate_object`"));
        assert!(prompt.contains("Function as a Service"));
        assert!(prompt.contains("Do NOT create extra generated JSON config files"));
        assert!(prompt.contains("keep package configuration in `.a3s/asset.acl`"));
        assert!(!prompt.contains("runnable"));
        assert!(!prompt.contains("run/debug"));
        assert!(prompt.contains("/reload"));
    }

    #[test]
    fn scaffold_skill_asset_creates_source_at_root_and_metadata_acl() {
        let root =
            std::env::temp_dir().join(format!("a3s-code-skill-scaffold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let dev = scaffold_skill_asset(
            "Name it exactly incident-brief. It summarizes incident notes.",
            &root,
        )
        .unwrap();

        assert_eq!(dev.name, "incident-brief");
        let package = dev.path.parent().unwrap();
        for rel in [
            "README.md",
            "SKILL.md",
            "examples/example-input.md",
            "examples/example-output.md",
            "tests/smoke.md",
            ".a3s/asset.acl",
        ] {
            assert!(package.join(rel).is_file(), "missing {rel}");
        }
        for rel in [
            "skill.asset.json",
            "skill.runtime-binding.json",
            ".a3s/skill.asset.json",
            ".a3s/skill.runtime-binding.json",
        ] {
            assert!(!package.join(rel).exists(), "unexpected {rel}");
        }
        assert!(!package.join(".a3s/SKILL.md").exists());
        let source_files = collect_skill_source_files(package).unwrap();
        let paths = source_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"SKILL.md"), "{paths:?}");
        assert!(
            paths.iter().all(|path| !path.starts_with(".a3s/")),
            "{paths:?}"
        );
        let asset_acl =
            std::fs::read_to_string(package.join(asset_lifecycle::ASSET_ACL_PATH)).unwrap();
        assert!(asset_acl.contains("category = \"skill\""));
        assert!(asset_acl.contains("definition_path = \"SKILL.md\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn parses_skill_asset_subcommands() {
        assert_eq!(
            parse_skill_subcommand("clone https://github.com/a/b.git")
                .unwrap()
                .unwrap(),
            SkillSubcommand::Clone("https://github.com/a/b.git".into())
        );
        assert_eq!(
            parse_skill_subcommand("review").unwrap().unwrap(),
            SkillSubcommand::Review
        );
        assert_eq!(
            parse_skill_subcommand("publish").unwrap().unwrap(),
            SkillSubcommand::Publish
        );
        assert_eq!(
            parse_skill_subcommand("deploy").unwrap().unwrap(),
            SkillSubcommand::Deploy
        );
        assert_eq!(
            parse_skill_subcommand("open").unwrap().unwrap(),
            SkillSubcommand::Open
        );
        assert_eq!(
            parse_skill_subcommand("status").unwrap().unwrap(),
            SkillSubcommand::Status
        );
        assert_eq!(
            parse_skill_subcommand("activity evaluations")
                .unwrap()
                .unwrap(),
            SkillSubcommand::Activity("evaluations".into())
        );
        assert!(parse_skill_subcommand("ps").unwrap().is_err());
        assert!(parse_skill_subcommand("run").unwrap().is_err());
        assert_eq!(
            parse_skill_subcommand("debug").unwrap().unwrap_err(),
            "unknown /skill command `debug`"
        );
        assert!(parse_skill_subcommand("logs").unwrap().is_err());
        assert!(parse_skill_subcommand("jobs").unwrap().is_err());
        assert!(parse_skill_subcommand("inspect").unwrap().is_err());
        assert!(parse_skill_subcommand("exit").unwrap().is_err());
        assert!(parse_skill_subcommand("review ops").unwrap().is_err());
        assert!(parse_skill_subcommand("deploy ops").unwrap().is_err());
        for removed in ["view", "remote", "os", "dashboard"] {
            assert!(
                parse_skill_subcommand(removed).unwrap().is_err(),
                "/skill {removed} should not create a skill prototype"
            );
        }
        assert!(parse_skill_subcommand("make a skill").is_none());
    }

    #[test]
    fn skill_picker_lines_use_bounded_shared_menu_rows() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/skills/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let skills = vec![
            SkillAsset {
                rel: "nested/very-long-skill-name-that-would-overflow-the-panel/SKILL.md".into(),
                path: root
                    .join("nested/very-long-skill-name-that-would-overflow-the-panel/SKILL.md"),
                name: "long-skill".into(),
                description: "A long skill description that should be trimmed cleanly".into(),
            },
            SkillAsset {
                rel: "ops-triage/SKILL.md".into(),
                path: root.join("ops-triage/SKILL.md"),
                name: "ops-triage".into(),
                description: "Triage incidents".into(),
            },
        ];
        let lines = skill_picker_lines(&skills, 0, &root, 40, 20);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("skill"), "{plain}");
        assert!(plain.contains("select a skill asset"), "{plain}");
        assert!(plain.contains("very-long-skill"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn skill_picker_lines_scroll_selected_skill_into_view() {
        let root = std::path::PathBuf::from("/tmp/skills");
        let skills = (0..16)
            .map(|index| SkillAsset {
                rel: format!("skill-{index}/SKILL.md"),
                path: root.join(format!("skill-{index}/SKILL.md")),
                name: format!("skill-{index}"),
                description: format!("Skill asset {index}"),
            })
            .collect::<Vec<_>>();
        let plain = skill_picker_lines(&skills, 14, &root, 48, 16)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("skill-14/SKILL.md"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn skill_picker_header_and_hint_fit_fixed_width() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/skills/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let header = skill_picker_header(9, &root, 40);
        let hint = skill_picker_hint(40);
        assert!(a3s_tui::style::visible_len(&header) <= 40, "{header}");
        assert!(a3s_tui::style::visible_len(&hint) <= 40, "{hint}");
    }

    #[test]
    fn skill_picker_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let root = std::path::PathBuf::from("/tmp/skills");
        let skills = (0..4)
            .map(|index| SkillAsset {
                rel: format!("skill-{index}/SKILL.md"),
                path: root.join(format!("skill-{index}/SKILL.md")),
                name: format!("skill-{index}"),
                description: format!("Skill asset {index}"),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = skill_picker_lines(&skills, 0, &root, width, height).len();
        let y_offset = skill_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = skill_picker_panel(&skills, 0, &root, width, height).expect("panel");
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
    fn skill_picker_click_selects_visible_row_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let root = std::path::PathBuf::from("/tmp/skills");
        let skills = (0..4)
            .map(|index| SkillAsset {
                rel: format!("skill-{index}/SKILL.md"),
                path: root.join(format!("skill-{index}/SKILL.md")),
                name: format!("skill-{index}"),
                description: format!("Skill asset {index}"),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = skill_picker_lines(&skills, 0, &root, width, height).len();
        let y_offset = skill_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = skill_picker_panel(&skills, 0, &root, width, height).expect("panel");
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
    fn skill_overlay_offset_moves_up_with_more_rows_below() {
        assert!(skill_overlay_y_offset(24, 8, 7) < skill_overlay_y_offset(24, 8, 5));
    }

    #[test]
    fn skill_progressive_score_prefers_faas_viewlinks() {
        let asset = SkillAssetRef {
            id: "asset-1".into(),
            name: "skill-ops-triage".into(),
        };
        let params = skill_progressive_params(&asset, "ops-triage", SkillOsAction::Open);
        assert_eq!(params["assetId"], "asset-1");
        assert_eq!(params["operation"], "open");
        assert_eq!(params["input"]["skillName"], "ops-triage");

        let value = serde_json::json!({
            "data": {
                "items": [
                    {
                        "module": "functions",
                        "operation": "FunctionController_getAsset",
                        "description": "Function as a Service skill asset metadata"
                    },
                    {
                        "module": "marketplace",
                        "operation": "SkillListingController_getSkillPublishingCandidate",
                        "description": "Function as a Service deploy skill asset shaped ViewLink"
                    },
                    {
                        "module": "functions",
                        "operation": "SkillFunctionController_openView",
                        "description": "Function as a Service skill RemoteUI ViewLink open"
                    },
                    {
                        "module": "issues",
                        "operation": "IssueController_reopenIssue",
                        "description": "Reopen issue workflow for a skill support ticket"
                    },
                    {
                        "module": "workflows",
                        "operation": "WorkflowDesignerController_open",
                        "description": "Workflow as a Service designer ViewLink"
                    }
                ]
            }
        });
        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            skill_progressive_score(SkillOsAction::Open, text, operation)
        });

        assert_eq!(candidates[0].operation, "SkillFunctionController_openView");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "FunctionController_getAsset"),
            "asset metadata without an open/view hint should not drive /skill open: {candidates:?}"
        );
        assert!(
            candidates.iter().all(|candidate| candidate.operation
                != "SkillListingController_getSkillPublishingCandidate"),
            "marketplace publishing candidates must not drive /skill open: {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "IssueController_reopenIssue"),
            "issue reopen must not drive /skill open: {candidates:?}"
        );
        assert!(
            skill_progressive_score(
                SkillOsAction::Open,
                "Function as a Service Skill RemoteUI ViewLink OPEN",
                "SkillFunctionController_openView"
            ) > 0
        );
        assert_eq!(
            skill_progressive_score(
                SkillOsAction::Open,
                "Function as a Service skill asset repository preview",
                "AssetCrudController_getScaffoldTemplatePreview"
            ),
            0,
            "/skill open must not choose scaffold/template preview operations"
        );
        assert_eq!(
            skill_progressive_score(
                SkillOsAction::Open,
                "Function as a Service skill issue reopen shaped ViewLink",
                "IssueController_reopenIssue"
            ),
            0,
            "/skill open must not choose issue reopen operations"
        );
        assert!(
            skill_progressive_score(
                SkillOsAction::Deploy,
                "Function as a Service Skill deploy shaped ViewLink",
                "SkillFunctionController_deploy"
            ) > 0
        );
        assert_eq!(
            skill_progressive_score(
                SkillOsAction::Deploy,
                "Function as a Service skill asset metadata",
                "FunctionController_getAsset",
            ),
            0,
            "deploy must require an actual deploy/serving operation hint"
        );
    }

    #[test]
    fn skill_package_metadata_carries_function_service_binding() {
        let dev = SkillDevSession {
            name: "ops-triage".into(),
            description: "Triage incidents".into(),
            rel: "ops/SKILL.md".into(),
            path: std::path::PathBuf::from("/Users/x/.a3s/skills/ops/SKILL.md"),
            root: std::path::PathBuf::from("/Users/x/.a3s/skills"),
        };
        let manifest = skill_manifest_json(&dev, "skill-ops-triage");
        let binding = skill_runtime_binding_json(&dev, "skill-ops-triage");
        let upsert = skill_runtime_binding_upsert_body(&binding);

        assert_eq!(skill_asset_name("Ops Triage"), "skill-ops-triage");
        assert_eq!(manifest["category"], "skill");
        assert_eq!(manifest["service"], "Function as a Service");
        assert_eq!(manifest["runtimeIntent"]["kind"], "tool");
        assert_eq!(
            manifest["runtimeIntent"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(manifest["runtimeIntent"]["protocol"], "skill");
        assert_eq!(manifest["runtimeIntent"]["agentKind"], "tool");
        assert_eq!(binding["kind"], "tool");
        assert_eq!(binding["isolation"], "serving");
        assert_eq!(binding["runtime"]["kind"], "a3s-function-service");
        assert_eq!(binding["runtime"]["protocol"], "skill");
        assert_eq!(binding["runtime"]["agentKind"], "tool");
        assert_eq!(binding["metadata"]["service"], "Function as a Service");
        assert_eq!(upsert["kind"], "tool");
        assert_eq!(upsert["runtime"]["sharedRuntime"], "node-20");
        assert!(upsert["runtime"].get("protocol").is_none());
        assert!(upsert["runtime"].get("agentKind").is_none());
    }

    #[test]
    fn existing_skill_asset_must_match_skill_category() {
        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "agent-asset",
                        "name": "skill-ops-triage",
                        "category": "agent"
                    }
                ]
            }
        });

        let err = find_skill_asset(&found, "skill-ops-triage").unwrap_err();
        assert!(err.contains("category=agent"), "{err}");
        assert!(err.contains("expected skill"), "{err}");
    }

    #[tokio::test]
    async fn publish_skill_to_os_uploads_asset_and_syncs_function_service_binding() {
        let root = std::env::temp_dir().join(format!("a3s-skill-publish-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops/examples")).unwrap();
        std::fs::write(
            root.join("ops/SKILL.md"),
            "---\nname: ops-triage\ndescription: Triage incidents\n---\nBody\n",
        )
        .unwrap();
        std::fs::write(root.join("ops/examples/input.md"), "incident").unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_skill_publish_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = SkillDevSession {
            name: "ops-triage".into(),
            description: "Triage incidents".into(),
            rel: "ops/SKILL.md".into(),
            path: root.join("ops/SKILL.md"),
            root: root.clone(),
        };

        let result = publish_skill_to_os(session, dev, SkillOsAction::Publish)
            .await
            .expect("skill publish should use OS Function as a Service");

        assert_eq!(result.asset_name, "skill-ops-triage");
        assert_eq!(result.asset_id, "skill-asset-1");
        assert!(
            result.note.contains("Function as a Service"),
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
        assert!(joined.contains("GET /api/v1/assets?"), "{joined}");
        assert!(joined.contains("POST /api/v1/assets HTTP/1.1"), "{joined}");
        assert!(
            joined.contains("POST /api/v1/assets/skill-asset-1/repository/files HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/skill-asset-1/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/assets/skill-asset-1/runtime-binding/validate HTTP/1.1"),
            "{joined}"
        );
        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "skill");
        assert_eq!(create_json["metadata"]["service"], "Function as a Service");
        assert_eq!(create_json["metadata"]["agentKind"], "tool");
        assert_eq!(
            create_json["metadata"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(create_json["metadata"]["protocol"], "skill");
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/skill-asset-1/repository/files HTTP/1.1",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let files = upload_json["files"].as_array().unwrap();
        assert!(files.iter().any(|file| file["path"] == "SKILL.md"));
        assert!(files
            .iter()
            .any(|file| file["path"] == asset_lifecycle::ASSET_ACL_PATH));
        for forbidden in [
            "skill.asset.json",
            "skill.runtime-binding.json",
            "runtime-binding.json",
        ] {
            assert!(
                files.iter().all(|file| file["path"] != forbidden),
                "repository upload should not include {forbidden}"
            );
        }

        let synced = request_body(
            &requests,
            "PUT /api/v1/assets/skill-asset-1/runtime-binding HTTP/1.1",
        );
        let synced_json: serde_json::Value = serde_json::from_str(&synced).unwrap();
        assert_eq!(synced_json["kind"], "tool");
        assert_eq!(synced_json["runtime"]["sharedRuntime"], "node-20");
        assert!(synced_json["runtime"].get("protocol").is_none());
        let local_asset_acl = root.join("ops").join(asset_lifecycle::ASSET_ACL_PATH);
        assert!(local_asset_acl.is_file(), "missing local asset.acl");
        let local_asset_acl_body = std::fs::read_to_string(&local_asset_acl).unwrap();
        assert!(local_asset_acl_body.contains("category = \"skill\""));
        assert!(local_asset_acl_body.contains("definition_path = \"SKILL.md\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deploy_skill_to_os_uses_progressive_function_service_view_when_available() {
        let root = std::env::temp_dir().join(format!("a3s-skill-deploy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops")).unwrap();
        std::fs::write(
            root.join("ops/SKILL.md"),
            "---\nname: ops-triage\ndescription: Triage incidents\n---\nBody\n",
        )
        .unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_skill_publish_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = SkillDevSession {
            name: "ops-triage".into(),
            description: "Triage incidents".into(),
            rel: "ops/SKILL.md".into(),
            path: root.join("ops/SKILL.md"),
            root: root.clone(),
        };

        let result = publish_skill_to_os(session, dev, SkillOsAction::Deploy)
            .await
            .expect("skill deploy should use OS Function as a Service");

        assert_eq!(result.action, SkillOsAction::Deploy);
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/functions/skill-ops-triage/deploy?embed=1")
        );
        assert!(
            result.note.contains("progressive capabilities"),
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
        assert!(
            joined.contains("POST /api/v1/kernel/capabilities HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains(r#""action":"search""#)
                && joined.contains("Function as a Service deploy skill asset shaped ViewLink"),
            "{joined}"
        );
        assert!(
            joined.contains(r#""action":"execute""#)
                && joined.contains(r#""shaped":true"#)
                && joined.contains("SkillFunctionController_deploy"),
            "{joined}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_skill_assets_from_skill_md_dirs() {
        let root = std::env::temp_dir().join(format!("a3s-skill-panel-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops/examples")).unwrap();
        std::fs::write(
            root.join("ops/SKILL.md"),
            "---\nname: ops-triage\ndescription: Triage incidents\n---\nBody\n",
        )
        .unwrap();
        std::fs::write(root.join("ops/examples/input.md"), "incident").unwrap();
        let skills = list_skill_assets(&root);
        assert_eq!(skills.len(), 1);
        assert_eq!(skills[0].name, "ops-triage");
        assert_eq!(skills[0].description, "Triage incidents");
        let package_skills = list_skill_assets(&root.join("ops"));
        assert_eq!(package_skills.len(), 1);
        assert_eq!(package_skills[0].rel, "SKILL.md");
        let _ = std::fs::remove_dir_all(&root);
    }

    async fn spawn_skill_publish_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
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
                    let (status, payload) = skill_publish_mock_response(&line, &body);
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

    fn skill_publish_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if line.starts_with("POST /api/v1/kernel/capabilities HTTP/1.1") {
            if body.contains(r#""action":"search""#)
                && body.contains("Function as a Service deploy skill asset shaped ViewLink")
            {
                return (
                    "200 OK",
                    r#"{"data":{"results":[{"module":"functions","operation":"FunctionController_getAsset","description":"Function as a Service skill asset metadata"},{"module":"functions","operation":"SkillFunctionController_deploy","description":"Function as a Service skill deploy shaped ViewLink"}]}}"#,
                );
            }
            if body.contains(r#""action":"describe""#)
                && body.contains("SkillFunctionController_deploy")
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"operation":{"name":"SkillFunctionController_deploy","inputSchema":{"body":{"properties":{"functionRef":{"type":"string"},"assetId":{"type":"string"},"input":{"type":"object"},"agentKind":{"type":"string"},"idempotencyKey":{"type":"string"}}}}}}}"#,
                );
            }
            if body.contains(r#""action":"execute""#)
                && body.contains(r#""shaped":true"#)
                && body.contains("SkillFunctionController_deploy")
                && body.contains(r#""functionRef":"skill-ops-triage""#)
                && body.contains(r#""agentKind":"tool""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"deploymentId":"skill-deploy-1"},"view":{"url":"/admin/functions/skill-ops-triage/deploy?embed=1","width":1280,"height":860}}"#,
                );
            }
            return ("404 Not Found", r#"{"code":404,"message":"not found"}"#);
        }
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""category":"skill""#)
                && body.contains(r#""service":"Function as a Service""#)
                && body.contains(r#""agentKind":"tool""#)
                && body.contains(r#""runtimeKind":"a3s-function-service""#)
                && body.contains(r#""protocol":"skill""#)
                && body.contains(r#""createdBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"skill-asset-1","name":"skill-ops-triage"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad skill asset body"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/skill-asset-1/repository/files HTTP/1.1") {
            if body.contains("SKILL.md")
                && body.contains(asset_lifecycle::ASSET_ACL_PATH)
                && !body.contains("skill.asset.json")
                && !body.contains("skill.runtime-binding.json")
            {
                return ("200 OK", r#"{"ok":true}"#);
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad skill upload body"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/skill-asset-1/runtime-binding HTTP/1.1") {
            if body.contains(r#""kind":"tool""#)
                && body.contains(r#""sharedRuntime":"node-20""#)
                && !body.contains(r#""version""#)
                && !body.contains(r#""protocol":"skill""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"skill-asset-1","configured":true}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad runtime binding"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/skill-asset-1/runtime-binding/validate HTTP/1.1") {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"skill-asset-1","configured":true,"valid":true,"issues":[]}}"#,
            );
        }
        ("404 Not Found", r#"{"code":404,"message":"not found"}"#)
    }
}
