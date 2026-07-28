//! `/okf` shareable OKF knowledge-package asset surface.
//!
//! OKF packages are team digital assets backed by OS Knowledge service. The
//! personal local knowledge base stays in the sibling `/kb` module.

use super::super::asset_lifecycle;
use super::super::os_progressive;
use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum OkfCommand {
    Select,
    Exit,
    Clone(String),
    List(String),
    Review,
    Activity(String),
    Publish,
    Deploy,
    Status,
    Prototype(String),
    Usage(&'static str),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OkfPackageAsset {
    pub(crate) rel: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) name: String,
    pub(crate) description: String,
}

pub(crate) struct OkfPackagePanel {
    pub(crate) root: std::path::PathBuf,
    pub(crate) packages: Vec<OkfPackageAsset>,
    pub(crate) sel: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OkfDevSession {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) rel: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) root: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OkfOsAction {
    Publish,
    Deploy,
    Status,
}

impl OkfOsAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Deploy => "deploy",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct OkfOsResult {
    pub(crate) action: OkfOsAction,
    pub(crate) asset_name: String,
    pub(crate) asset_id: String,
    pub(crate) view: remote_ui::ViewSpec,
    pub(crate) note: String,
    pub(crate) open_view: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KnowledgeAssetRef {
    id: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct KnowledgeSourceFile {
    path: String,
    bytes: Vec<u8>,
}

pub(crate) fn parse_okf_command(rest: &str) -> OkfCommand {
    let arg = rest.trim();
    if arg.is_empty() {
        return OkfCommand::Select;
    }
    let (head, tail) = arg
        .split_once(char::is_whitespace)
        .map(|(h, t)| (h, t.trim()))
        .unwrap_or((arg, ""));
    match head {
        "off" => {
            if tail.is_empty() {
                OkfCommand::Exit
            } else {
                OkfCommand::Usage("usage: /okf off")
            }
        }
        "exit" | "normal" | "clear" | "stop" => OkfCommand::Usage("usage: /okf off"),
        "clone" => {
            let mut parts = tail.split_whitespace();
            let Some(url) = parts.next() else {
                return OkfCommand::Usage("usage: /okf clone <git-url>");
            };
            if parts.next().is_some() {
                return OkfCommand::Usage("usage: /okf clone <git-url>");
            }
            OkfCommand::Clone(url.to_string())
        }
        "list" => OkfCommand::List(tail.to_string()),
        "review" => {
            if tail.is_empty() {
                OkfCommand::Review
            } else {
                OkfCommand::Usage("usage: /okf review")
            }
        }
        "activity" => OkfCommand::Activity(tail.to_string()),
        "publish" => {
            if tail.is_empty() {
                OkfCommand::Publish
            } else {
                OkfCommand::Usage("usage: /okf publish")
            }
        }
        "run" => OkfCommand::Usage(
            "OKF packages are not runnable assets; use /okf publish or /okf deploy",
        ),
        "debug" => OkfCommand::Usage("unknown /okf command `debug`"),
        "deploy" => {
            if tail.is_empty() {
                OkfCommand::Deploy
            } else {
                OkfCommand::Usage("usage: /okf deploy")
            }
        }
        "status" => {
            if tail.is_empty() {
                OkfCommand::Status
            } else {
                OkfCommand::Usage("usage: /okf status")
            }
        }
        "logs" => {
            OkfCommand::Usage("OKF packages do not expose logs; use /okf status or /okf activity")
        }
        "os" | "open" | "view" | "remote" | "inspect" => OkfCommand::Usage("usage: /okf status"),
        "dashboard" => OkfCommand::Usage("usage: /okf list [query] · /okf status"),
        "add" | "import" | "search" | "vault" => OkfCommand::Usage(
            "personal knowledge-base commands use /kb add/import/search/vault; OKF packages use /okf review/publish/deploy/status",
        ),
        "ps" | "runs" | "jobs" => OkfCommand::Usage("usage: /okf activity [query]"),
        _ => OkfCommand::Prototype(arg.to_string()),
    }
}

fn looks_path_like(s: &str) -> bool {
    s.contains('/')
        || s.contains('\\')
        || s.starts_with('.')
        || s.starts_with('~')
        || s.ends_with(".md")
        || s.ends_with(".txt")
        || s.ends_with(".json")
        || s.ends_with(".yaml")
        || s.ends_with(".yml")
}

#[cfg(test)]
pub(crate) fn okf_package_dir(cwd: &str) -> std::path::PathBuf {
    std::path::Path::new(cwd).join("okf")
}

pub(crate) fn list_okf_packages(root: &std::path::Path) -> Vec<OkfPackageAsset> {
    let mut packages = Vec::new();
    collect_okf_package_dirs(root, root, &mut packages);
    packages.sort_by(|a, b| a.rel.cmp(&b.rel));
    packages
}

fn collect_okf_package_dirs(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<OkfPackageAsset>,
) {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in rd.flatten() {
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') || !path.is_dir() {
            continue;
        }
        if is_okf_package_dir(&path) {
            out.push(okf_package_asset(root, &path));
            continue;
        }
        collect_okf_package_dirs(root, &path, out);
    }
}

fn is_okf_package_dir(path: &std::path::Path) -> bool {
    path.join("README.md").is_file()
        && (path.join("sources").is_dir() || path.join("wiki").is_dir())
}

pub(crate) fn okf_package_asset_from_dir(
    root: &std::path::Path,
    path: &std::path::Path,
) -> Option<OkfPackageAsset> {
    is_okf_package_dir(path).then(|| okf_package_asset(root, path))
}

fn okf_package_asset(root: &std::path::Path, path: &std::path::Path) -> OkfPackageAsset {
    let rel = path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string();
    let (name, description) = okf_package_metadata(path).unwrap_or_else(|| {
        let fallback = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("knowledge-package")
            .to_string();
        (fallback, "OKF knowledge package".to_string())
    });
    OkfPackageAsset {
        rel,
        path: path.to_path_buf(),
        name,
        description,
    }
}

fn okf_package_metadata(path: &std::path::Path) -> Option<(String, String)> {
    readme_metadata(&path.join("README.md")).or_else(|| {
        path.file_name()
            .and_then(|n| n.to_str())
            .map(|name| (name.to_string(), "OKF knowledge package".to_string()))
    })
}

fn readme_metadata(path: &std::path::Path) -> Option<(String, String)> {
    let body = std::fs::read_to_string(path).ok()?;
    let mut title = None;
    let mut description = None;
    for line in body.lines().map(str::trim).filter(|line| !line.is_empty()) {
        if title.is_none() && line.starts_with("# ") {
            title = Some(line.trim_start_matches("# ").trim().to_string());
            continue;
        }
        if description.is_none() && !line.starts_with('#') {
            description = Some(line.to_string());
        }
        if title.is_some() && description.is_some() {
            break;
        }
    }
    title.map(|name| {
        (
            name,
            description.unwrap_or_else(|| "OKF knowledge package".to_string()),
        )
    })
}

pub(crate) fn scaffold_okf_package(
    description: &str,
    root: &std::path::Path,
) -> Result<OkfDevSession, String> {
    let name = asset_lifecycle::scaffold_name(description, "knowledge-package");
    let package = asset_lifecycle::unique_asset_dir(root, &name);
    let final_name = package
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name.as_str())
        .to_string();
    let description =
        asset_lifecycle::scaffold_description(description, &final_name, "OKF knowledge package");

    for dir in [".a3s", "sources", "wiki/concepts", "eval"] {
        std::fs::create_dir_all(package.join(dir))
            .map_err(|e| format!("could not create {}: {e}", package.join(dir).display()))?;
    }

    let rel = asset_lifecycle::normalized_rel(root, &package);
    let dev = OkfDevSession {
        name: final_name.clone(),
        description: description.clone(),
        rel,
        path: package.clone(),
        root: root.to_path_buf(),
    };
    let asset_name = knowledge_asset_name(&dev.name);
    let asset_acl = knowledge_asset_acl(&dev, &asset_name);

    asset_lifecycle::write_scaffold_file(
        &package.join("README.md"),
        okf_scaffold_readme(&dev.name, &dev.description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("sources/overview.md"),
        format!("# Source Overview\n\n{}\n", dev.description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("wiki/index.md"),
        format!("# {}\n\n{}\n", dev.name, dev.description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("wiki/concepts/example.md"),
        "# Example Concept\n\nDescribe the first concept this OKF package teaches.\n".as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("eval/smoke.md"),
        okf_scaffold_eval().as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join(asset_lifecycle::ASSET_ACL_PATH),
        asset_acl.as_bytes(),
    )?;

    Ok(dev)
}

fn okf_scaffold_readme(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n\
         {description}.\n\n\
         ## Source\n\n\
         - `sources/`, `wiki/`, and `eval/` contain knowledge content and checks.\n\
         - `.a3s/` contains only `asset.acl`.\n\n\
         ## Lifecycle\n\n\
         - `a3s code okf publish .`\n\
         - `a3s code okf deploy .`\n\
         - `a3s code okf status .`\n"
    )
}

fn okf_scaffold_eval() -> &'static str {
    "# OKF Smoke Evaluation\n\n\
     1. Confirm `README.md` states the package scope and source provenance.\n\
     2. Confirm `wiki/index.md` links to the primary concept pages.\n\
     3. Confirm `.a3s/asset.acl` points to the visible knowledge sources.\n"
}

#[cfg(test)]
pub(crate) fn okf_package_gen_prompt(description: &str, cwd: &str) -> String {
    let dir = okf_package_dir(cwd);
    let dir = dir.display();
    format!(
        "Create a local OKF knowledge package prototype from the description below and save it \
         under {dir}. This is a local authoring task: do not open OS, RemoteUI, or a browser.\n\
         Description: {description}\n\
         Create {dir}/<kebab-case-name>/ with at least README.md, sources/, \
         wiki/index.md, wiki/concepts/example.md, eval/smoke.md, and .a3s/asset.acl. \
         The package-local `.a3s/` directory is metadata-only. Do NOT put README, \
         sources, wiki, eval, or other knowledge source files under `.a3s/`. Do NOT create \
         extra generated JSON config files; keep package configuration in `.a3s/asset.acl`. \
         The package should be ready for OKF create/develop/publish/deploy plus \
         status/activity inspection: describe source provenance, \
         concept schema, validation/evaluation checks, index/update expectations, and how OS \
         Knowledge service indexing/evaluation should consume it later. Use service=Knowledge \
         service, runtimeIntent.kind=knowledge, runtime.kind=a3s-knowledge-service, protocol=okf, \
         isolation=serving, and operations index/evaluate/report. Then stop using tools immediately and give a concise final \
         answer with the saved package path and the note that `/okf` selects OKF packages while \
         `/kb vault` browses the local personal knowledge base."
    )
}

pub(crate) fn okf_dev_prompt(session: &OkfDevSession, request: &str) -> String {
    format!(
        "You are in A3S Code local OKF knowledge-package development mode.\n\
         Current package: {name}\n\
         Description: {description}\n\
         Package path: {path}\n\
         Package root: {root}\n\n\
         User request:\n{request}\n\n\
         Work on this local OKF package iteratively. Read README.md, sources/, \
         wiki/, and eval/ before editing when they exist. Keep source provenance, concept schema, \
         generated concept pages, evaluation notes, and `.a3s/asset.acl` consistent. \
         Do not open OS, RemoteUI, or browser pages for this local package-development turn. \
         Validate changed Markdown/content files and end with a concise summary plus the next lifecycle step.\n\n\
         The TUI remains in OKF-development mode for `{name}` after this turn; the user can press \
         Esc or run `/okf off` to return to normal mode.",
        name = session.name.as_str(),
        description = session.description.as_str(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

pub(crate) fn okf_lifecycle_prompt(
    action: &str,
    session: &OkfDevSession,
    os_runtime: bool,
) -> String {
    let review_contract = if action == "review" {
        super::review::review_report_contract(&session.path)
    } else {
        String::new()
    };
    let runtime = if os_runtime {
        "OS is signed in. Prefer the `runtime` tool and OS progressive capabilities when they \
         expose knowledge indexing, evaluation, report, or RemoteUI ViewLink operations. Keep \
         shaped responses intact so a returned view can be opened by the host."
    } else {
        "OS is not signed in. Stay local: validate files, run lightweight checks, and clearly \
         report what OS knowledge-service action is blocked by login."
    };
    format!(
        "Run `/okf {action}` for this selected OKF knowledge package.\n\
         Package: {name}\n\
         Description: {description}\n\
         Package path: {path}\n\
         Package root: {root}\n\n\
         {runtime}\n\n\
         For review, report package/schema/source/evaluation gaps without changing files unless \
         explicitly asked. For publish, prepare or update package metadata and knowledge asset \
         manifest. For deploy, prepare the knowledge-service binding and use indexing, \
         evaluation, report, or RemoteUI operations when OS exposes them. For status, inspect \
         the existing OS knowledge asset and runtime binding without mutating package files. \
         End with concise findings, changed files if any, and the next asset-scoped \
         command.{review_contract}",
        name = session.name.as_str(),
        description = session.description.as_str(),
        path = session.path.display(),
        root = session.root.display(),
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

fn http() -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder.build().map_err(|e| e.to_string())
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

fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn knowledge_asset_name(package_name: &str) -> String {
    let mut slug = String::new();
    for ch in package_name.chars() {
        if ch.is_ascii_alphanumeric() {
            slug.push(ch.to_ascii_lowercase());
        } else if !slug.ends_with('-') {
            slug.push('-');
        }
    }
    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "knowledge-package".to_string()
    } else if slug.starts_with("knowledge-") {
        slug.chars().take(72).collect()
    } else {
        format!("knowledge-{}", slug.chars().take(62).collect::<String>())
    }
}

fn knowledge_asset_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/assets/{}?embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn knowledge_asset_search_url(origin: &str, asset_name: &str) -> String {
    format!(
        "{}/assets?category=knowledge&scope=mine&status=all&search={}&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_name)
    )
}

fn knowledge_service_view_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/knowledge/{}?embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn knowledge_view_spec(url: String) -> remote_ui::ViewSpec {
    remote_ui::ViewSpec {
        url,
        width: Some(1280),
        height: Some(860),
        embeddable: true,
    }
}

fn collect_knowledge_source_files(
    root: &std::path::Path,
) -> Result<Vec<KnowledgeSourceFile>, String> {
    let mut out = Vec::new();
    collect_knowledge_source_files_inner(root, root, &mut out)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_knowledge_source_files_inner(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<KnowledgeSourceFile>,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir).map_err(|e| format!("read {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if matches!(
                name.as_str(),
                ".a3s" | ".git" | "target" | "node_modules" | ".venv" | "__pycache__"
            ) {
                continue;
            }
            collect_knowledge_source_files_inner(root, &path, out)?;
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|e| format!("read {}: {e}", path.display()))?;
        if bytes.len() > 1024 * 1024 {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if rel.starts_with(".a3s/") {
            continue;
        }
        if matches!(
            rel.as_str(),
            "package.okf.json"
                | "knowledge.asset.json"
                | "knowledge.runtime-binding.json"
                | "runtime-binding.json"
        ) {
            continue;
        }
        out.push(KnowledgeSourceFile { path: rel, bytes });
    }
    Ok(())
}

fn knowledge_manifest_json(dev: &OkfDevSession, asset_name: &str) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.knowledge.asset.v1",
        "category": "knowledge",
        "name": asset_name,
        "packageName": dev.name.as_str(),
        "description": dev.description.as_str(),
        "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        "localPath": dev.rel.as_str(),
        "service": "Knowledge service",
        "createdBy": "a3s-code-tui",
        "runtimeIntent": {
            "kind": "knowledge",
            "isolation": "serving",
            "runtimeKind": "a3s-knowledge-service",
            "protocol": "okf",
            "operations": ["index", "evaluate", "report"],
        },
    })
}

fn knowledge_runtime_binding_json(dev: &OkfDevSession, asset_name: &str) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.knowledge.runtime-binding.v1",
        "kind": "knowledge",
        "enabled": true,
        "isolation": "serving",
        "target": {
            "kind": "asset",
            "ref": "main",
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        },
        "runtime": {
            "kind": "a3s-knowledge-service",
            "protocol": "okf",
            "operations": ["index", "evaluate", "report"],
        },
        "env": [],
        "requiredSecrets": [],
        "resources": {},
        "network": {},
        "metadata": {
            "source": "a3s-code-tui",
            "service": "Knowledge service",
            "assetName": asset_name,
            "packageName": dev.name.as_str(),
            "description": dev.description.as_str(),
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
            "localPath": dev.rel.as_str(),
        },
    })
}

fn knowledge_asset_acl(dev: &OkfDevSession, asset_name: &str) -> String {
    let source = [
        ("readme_path", "README.md"),
        ("sources_path", "sources"),
        ("wiki_path", "wiki"),
        ("eval_path", "eval"),
    ];
    let metadata: [(&str, &str); 0] = [];
    asset_lifecycle::render_asset_acl(asset_lifecycle::AssetAclDocument {
        category: "knowledge",
        kind: Some("knowledge"),
        name: asset_name,
        description: dev.description.as_str(),
        local_path: Some(dev.rel.as_str()),
        service: asset_lifecycle::OsService::KnowledgeService,
        runtime: asset_lifecycle::RuntimeBindingIntent {
            kind: "knowledge",
            isolation: "serving",
            runtime_kind: "a3s-knowledge-service",
            protocol: Some("okf"),
            agent_kind: None,
        },
        source: &source,
        metadata: &metadata,
    })
}

fn knowledge_runtime_binding_upsert_body(runtime_binding: &serde_json::Value) -> serde_json::Value {
    let target_ref = runtime_binding
        .pointer("/target/ref")
        .and_then(|value| value.as_str())
        .unwrap_or("main");
    let runtime_kind = runtime_binding
        .pointer("/runtime/kind")
        .and_then(|value| value.as_str())
        .unwrap_or("a3s-knowledge-service");
    serde_json::json!({
        "kind": runtime_binding
            .get("kind")
            .and_then(|value| value.as_str())
            .filter(|kind| matches!(*kind, "agent" | "workflow" | "service" | "job" | "mcp" | "tool"))
            .unwrap_or("service"),
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

fn knowledge_asset_ref_from_value(
    value: &serde_json::Value,
    fallback_name: &str,
) -> Option<KnowledgeAssetRef> {
    Some(KnowledgeAssetRef {
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

fn find_knowledge_asset(
    value: &serde_json::Value,
    name: &str,
) -> Result<Option<KnowledgeAssetRef>, String> {
    let exact = items_of(value)
        .into_iter()
        .find(|item| json_str_at(item, &["/name", "name"]) == Some(name));
    let Some(asset) = exact else {
        return Ok(None);
    };
    if let Some(actual) = asset_category(&asset) {
        if !actual.eq_ignore_ascii_case("knowledge") {
            return Err(category_conflict_error(name, actual, "knowledge"));
        }
    }
    knowledge_asset_ref_from_value(&asset, name)
        .map(Some)
        .ok_or_else(|| format!("asset `{name}` matched but had no id"))
}

async fn ensure_knowledge_asset(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    name: &str,
    description: &str,
) -> Result<KnowledgeAssetRef, String> {
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", name),
            ("category", "knowledge"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some(asset) = find_knowledge_asset(&found, name)? {
        return Ok(asset);
    }
    let resp = client
        .post(&base)
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "ownerType": "user",
            "category": "knowledge",
            "visibility": "private",
            "description": description,
            "metadata": {
                "service": "Knowledge service",
                "runtimeKind": "a3s-knowledge-service",
                "protocol": "okf",
                "operations": ["index", "evaluate", "report"],
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
            "create knowledge asset failed ({status}): {}",
            response_message(&body)
        ));
    }
    knowledge_asset_ref_from_value(envelope_data(&body), name)
        .ok_or_else(|| "create knowledge asset: no id in response".to_string())
}

async fn upload_knowledge_package(
    origin: &str,
    token: &str,
    asset_id: &str,
    source_files: &[KnowledgeSourceFile],
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
            "message": "a3s code /okf: update OKF knowledge package",
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
        "upload knowledge package failed ({status}): {}",
        truncate(&body, 200)
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum KnowledgeRuntimeBindingSync {
    Synced,
    Unsupported,
    Failed(String),
}

async fn sync_knowledge_runtime_binding(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> KnowledgeRuntimeBindingSync {
    match sync_knowledge_runtime_binding_inner(origin, token, asset_id, runtime_binding).await {
        Ok(true) => KnowledgeRuntimeBindingSync::Synced,
        Ok(false) => KnowledgeRuntimeBindingSync::Unsupported,
        Err(err) => KnowledgeRuntimeBindingSync::Failed(err),
    }
}

async fn sync_knowledge_runtime_binding_inner(
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
        .json(&knowledge_runtime_binding_upsert_body(runtime_binding))
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

async fn inspect_knowledge_asset(
    origin: &str,
    token: &str,
    action: OkfOsAction,
    asset_name: &str,
    package_name: &str,
) -> Result<OkfOsResult, String> {
    let client = http()?;
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", asset_name),
            ("category", "knowledge"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let Some(asset) = find_knowledge_asset(&found, asset_name)? else {
        return Ok(OkfOsResult {
            action,
            asset_name: asset_name.to_string(),
            asset_id: "not-published".to_string(),
            view: knowledge_view_spec(knowledge_asset_search_url(origin, asset_name)),
            note: format!(
                "OS {} for `{package_name}`: no knowledge asset named `{asset_name}` was found. Run `/okf publish` first.",
                action.label()
            ),
            open_view: false,
        });
    };
    let binding_status = runtime_binding_validation_status(&client, origin, token, &asset.id).await;
    Ok(OkfOsResult {
        action,
        asset_name: asset.name,
        asset_id: asset.id.clone(),
        view: knowledge_view_spec(knowledge_asset_url(origin, &asset.id)),
        note: format!("OS status for `{package_name}`: asset exists; {binding_status}."),
        open_view: false,
    })
}

fn knowledge_progressive_params(
    asset: &KnowledgeAssetRef,
    package_name: &str,
    action: OkfOsAction,
) -> serde_json::Value {
    serde_json::json!({
        "assetId": asset.id,
        "assetName": asset.name,
        "knowledgeRef": asset.name,
        "ref": asset.name,
        "name": asset.name,
        "packageName": package_name,
        "operation": action.label(),
        "input": {
            "assetId": asset.id,
            "assetName": asset.name,
            "packageName": package_name,
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "payload": {
            "assetId": asset.id,
            "assetName": asset.name,
            "packageName": package_name,
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "timeoutMs": 180000,
        "idempotencyKey": format!("a3s-code-kb-{}-{}", action.label(), unix_timestamp_secs()),
    })
}

fn knowledge_progressive_score(action: OkfOsAction, text: &str, operation: &str) -> i32 {
    let combined = format!("{text} {operation}").to_ascii_lowercase();
    let operation = operation.to_ascii_lowercase();
    if operation.contains("createasset")
        || operation.contains("listasset")
        || operation.contains("deployability")
        || operation.contains("diagnose")
        || operation.contains("acknowledge")
        || operation.contains("marketplace")
        || operation.contains("assetpackage")
    {
        return 0;
    }
    let mut score = 0;
    if combined.contains("knowledge") || combined.contains("okf") {
        score += 10;
    }
    match action {
        OkfOsAction::Deploy => {
            if operation.contains("deploy")
                || operation.contains("ingest")
                || operation.contains("index")
            {
                score += 8;
            } else {
                return 0;
            }
        }
        OkfOsAction::Publish | OkfOsAction::Status => return 0,
    }
    if combined.contains("view") || combined.contains("remoteui") || combined.contains("shaped") {
        score += 3;
    }
    if combined.contains("mcp")
        || combined.contains("workflow")
        || combined.contains("function as a service")
        || combined.contains("agent as a service")
    {
        score -= 8;
    }
    if score >= 10 {
        score
    } else {
        0
    }
}

async fn try_knowledge_progressive_action(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset: &KnowledgeAssetRef,
    package_name: &str,
    action: OkfOsAction,
) -> Option<(remote_ui::ViewSpec, String)> {
    let query = match action {
        OkfOsAction::Deploy => "Knowledge service deploy OKF package shaped ViewLink",
        OkfOsAction::Publish | OkfOsAction::Status => return None,
    };
    let execution = os_progressive::execute_first_matching(
        client,
        origin,
        token,
        query,
        knowledge_progressive_params(asset, package_name, action),
        |text, operation| knowledge_progressive_score(action, text, operation),
    )
    .await?;
    let fallback = knowledge_view_spec(knowledge_service_view_url(origin, &asset.id));
    Some((
        execution.view.unwrap_or(fallback),
        format!(
            "OS Knowledge service accepted `{}` through progressive capabilities (`{}`).",
            action.label(),
            execution.operation.operation
        ),
    ))
}

fn append_knowledge_runtime_binding_sync_note(
    mut note: String,
    runtime_binding_synced: &KnowledgeRuntimeBindingSync,
) -> String {
    match runtime_binding_synced {
        KnowledgeRuntimeBindingSync::Synced => note.push_str(" OS runtime binding was synced."),
        KnowledgeRuntimeBindingSync::Unsupported => note.push_str(
            " OS runtime-binding endpoint was unavailable; runtime-binding intent was saved.",
        ),
        KnowledgeRuntimeBindingSync::Failed(err) => note.push_str(&format!(
            " OS runtime binding could not be synced: {}; runtime-binding intent was saved.",
            truncate(err, 160)
        )),
    }
    note
}

pub(crate) async fn publish_okf_to_os(
    session: crate::a3s_os::StoredOsSession,
    dev: OkfDevSession,
    action: OkfOsAction,
) -> Result<OkfOsResult, String> {
    let origin = crate::a3s_os::os_origin(&session.address);
    let asset_name = knowledge_asset_name(&dev.name);
    if matches!(action, OkfOsAction::Status) {
        return inspect_knowledge_asset(
            &origin,
            &session.access_token,
            action,
            &asset_name,
            &dev.name,
        )
        .await;
    }
    let client = http()?;
    let asset = ensure_knowledge_asset(
        &client,
        &origin,
        &session.access_token,
        &asset_name,
        &dev.description,
    )
    .await?;
    let source_files = collect_knowledge_source_files(&dev.path)?;
    let manifest = knowledge_manifest_json(&dev, &asset_name);
    let runtime_binding = knowledge_runtime_binding_json(&dev, &asset_name);
    let asset_acl = knowledge_asset_acl(&dev, &asset_name);
    asset_lifecycle::write_asset_acl(&dev.path, &asset_acl)?;
    upload_knowledge_package(
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
        sync_knowledge_runtime_binding(&origin, &session.access_token, &asset.id, &runtime_binding)
            .await;
    let (view, note) = match action {
        OkfOsAction::Publish => (
            knowledge_view_spec(knowledge_asset_url(&origin, &asset.id)),
            format!(
                "Published `{}` as an OS knowledge asset backed by Knowledge service metadata.",
                dev.name
            ),
        ),
        OkfOsAction::Deploy => try_knowledge_progressive_action(
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
                knowledge_view_spec(knowledge_service_view_url(&origin, &asset.id)),
                format!(
                    "Published `{}`; opened the Knowledge service view because progressive `{}` was unavailable.",
                    dev.name,
                    action.label()
                ),
            )
        }),
        OkfOsAction::Status => {
            unreachable!("read-only OKF actions return before publish flow")
        }
    };
    let note = append_knowledge_runtime_binding_sync_note(note, &runtime_binding_synced);
    Ok(OkfOsResult {
        action,
        asset_name,
        asset_id: asset.id,
        view,
        note,
        open_view: true,
    })
}

impl App {
    pub(crate) fn handle_okf_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        match parse_okf_command(rest) {
            OkfCommand::Select => {
                self.open_okf_package_panel();
                None
            }
            OkfCommand::Exit => {
                self.exit_okf_dev();
                None
            }
            OkfCommand::Clone(url) => {
                let root = self.asset_directories.okf.clone();
                self.clone_asset_command("okf", url, root)
            }
            OkfCommand::List(query) => {
                self.open_asset_list_panel(os_asset_category_query("knowledge", &query))
            }
            command @ (OkfCommand::Review
            | OkfCommand::Activity(_)
            | OkfCommand::Publish
            | OkfCommand::Deploy
            | OkfCommand::Status) => self.execute_okf_asset_command(command),
            OkfCommand::Prototype(description) => {
                if looks_path_like(&description)
                    && kbutil::preview_import(&self.cwd, &description).is_ok()
                {
                    self.push_line(&Style::new().fg(TN_YELLOW).render(
                        "  path detected - use `/kb import <path>` for personal knowledge files, or describe the OKF package to create",
                    ));
                    return None;
                }
                self.textarea.clear();
                let root = self.asset_directories.okf.clone();
                match scaffold_okf_package(&description, &root) {
                    Ok(dev) => {
                        self.agent_dev = None;
                        self.mcp_dev = None;
                        self.skill_dev = None;
                        self.okf_dev = Some(dev.clone());
                        self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                            "  ⌁ scaffolded OKF package → {}",
                            dev.path.display()
                        )));
                        self.push_line(&gutter(
                            TN_CYAN,
                            &format!(
                                "⌁ okf dev: {} ({}) · Esc or /okf off returns to normal mode",
                                dev.name, dev.rel
                            ),
                        ));
                        self.relayout();
                        None
                    }
                    Err(e) => {
                        self.push_line(
                            &Style::new()
                                .fg(TN_RED)
                                .render(&format!("  /okf scaffold failed: {e}")),
                        );
                        None
                    }
                }
            }
            OkfCommand::Usage(usage) => {
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {usage}")));
                None
            }
        }
    }

    fn execute_okf_asset_command(&mut self, command: OkfCommand) -> Option<Cmd<Msg>> {
        match command {
            OkfCommand::Activity(query) => {
                let Some(okf_dev) = self.okf_dev.clone() else {
                    self.pending_okf_subcommand = Some(OkfCommand::Activity(query));
                    self.open_okf_package_panel();
                    return None;
                };
                self.open_runtime_activity_panel(runtime_asset_query(
                    "knowledge",
                    &okf_dev.name,
                    &query,
                ))
            }
            OkfCommand::Review => {
                let Some(okf_dev) = self.okf_dev.clone() else {
                    self.pending_okf_subcommand = Some(OkfCommand::Review);
                    self.open_okf_package_panel();
                    return None;
                };
                self.messages.push(TranscriptEntry::user("/okf review"));
                self.engage_autonomy(4);
                self.review_pending = true;
                let prompt = okf_lifecycle_prompt("review", &okf_dev, self.os_session.is_some());
                let display = format!("⌁ {} review", okf_dev.name);
                self.start_stream_inner(prompt, display, true, true, false)
            }
            OkfCommand::Publish | OkfCommand::Deploy | OkfCommand::Status => {
                let action = match command {
                    OkfCommand::Publish => "publish",
                    OkfCommand::Deploy => "deploy",
                    OkfCommand::Status => "status",
                    _ => unreachable!(),
                };
                let Some(okf_dev) = self.okf_dev.clone() else {
                    self.pending_okf_subcommand = Some(command);
                    self.open_okf_package_panel();
                    return None;
                };
                let Some(session) = self.os_session.clone() else {
                    if matches!(command, OkfCommand::Status) {
                        self.push_line(
                            &Style::new()
                                .fg(TN_YELLOW)
                                .render(&format!("  /okf {action} needs /login first")),
                        );
                        return None;
                    }
                    self.messages
                        .push(TranscriptEntry::user(format!("/okf {action}")));
                    self.engage_autonomy(6);
                    let prompt = okf_lifecycle_prompt(action, &okf_dev, false);
                    let display = format!("⌁ {} {action}", okf_dev.name);
                    return self.start_stream_inner(prompt, display, true, true, false);
                };
                let os_action = match command {
                    OkfCommand::Publish => OkfOsAction::Publish,
                    OkfCommand::Deploy => OkfOsAction::Deploy,
                    OkfCommand::Status => OkfOsAction::Status,
                    _ => unreachable!(),
                };
                self.messages
                    .push(TranscriptEntry::user(format!("/okf {action}")));
                let status_entry =
                    self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ⌁ {} → OS Knowledge service {}…",
                        okf_dev.name,
                        os_action.label()
                    )));
                Some(cmd::cmd(move || async move {
                    let result = publish_okf_to_os(session, okf_dev, os_action).await;
                    Msg::OkfOsCompleted {
                        status_entry,
                        result,
                    }
                }))
            }
            _ => unreachable!("non-asset OKF command routed to execute_okf_asset_command"),
        }
    }

    pub(crate) fn exit_okf_dev(&mut self) {
        match self.okf_dev.take() {
            Some(session) => self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  okf dev off — {} ({})",
                session.name, session.rel
            ))),
            None => self.push_line(&Style::new().fg(TN_GRAY).render("  okf dev is not active")),
        }
        self.relayout();
    }

    pub(crate) fn open_okf_package_panel(&mut self) {
        let root = self.asset_directories.okf.clone();
        let packages = list_okf_packages(&root);
        if packages.is_empty() {
            self.pending_okf_subcommand = None;
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  no OKF packages in {} — draft one with `/okf <description>` first",
                root.display()
            )));
            self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  Personal KB includes notes, imports, search, and the vault"),
            );
            return;
        }
        self.okf_picker = Some(OkfPackagePanel {
            root,
            packages,
            sel: 0,
        });
    }
}

impl App {
    pub(crate) fn handle_okf_package_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let panel = self.okf_picker.as_mut()?;
        let last = panel.packages.len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => panel.sel = panel.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => panel.sel = (panel.sel + 1).min(last),
            KeyCode::Esc => {
                cancel_pending_picker(&mut self.okf_picker, &mut self.pending_okf_subcommand)
            }
            KeyCode::Enter => {
                let panel = self.okf_picker.take()?;
                let selected = panel.sel.min(last);
                return self.activate_okf_package_selection(panel, selected);
            }
            _ => {}
        }
        None
    }

    pub(crate) fn handle_okf_package_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let panel_state = self.okf_picker.as_ref()?;
        let total = panel_state.packages.len();
        if total == 0 {
            return None;
        }
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let selected = panel_state.sel.min(total - 1);
        let (mut panel, panel_height) = okf_picker_panel(
            &panel_state.packages,
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
            okf_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                let panel_state = self.okf_picker.take()?;
                self.activate_okf_package_selection(panel_state, index.min(total - 1))
            }
            Some(MenuPanelMsg::Cancelled) => {
                cancel_pending_picker(&mut self.okf_picker, &mut self.pending_okf_subcommand);
                None
            }
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    if let Some(open) = self.okf_picker.as_mut() {
                        open.sel = after;
                    }
                }
                None
            }
        }
    }

    fn activate_okf_package_selection(
        &mut self,
        panel: OkfPackagePanel,
        selected: usize,
    ) -> Option<Cmd<Msg>> {
        let last = panel.packages.len().saturating_sub(1);
        let picked = panel.packages.get(selected.min(last))?.clone();
        self.agent_dev = None;
        self.mcp_dev = None;
        self.skill_dev = None;
        self.okf_dev = Some(OkfDevSession {
            name: picked.name.clone(),
            description: picked.description.clone(),
            rel: picked.rel.clone(),
            path: picked.path.clone(),
            root: panel.root,
        });
        self.push_line(&gutter(
            TN_CYAN,
            &format!(
                "⌁ okf dev: {} ({}) · Esc or /okf off returns to normal mode",
                picked.name, picked.rel
            ),
        ));
        self.relayout();
        if let Some(pending) = self.pending_okf_subcommand.take() {
            return self.execute_okf_asset_command(pending);
        }
        None
    }

    pub(crate) fn overlay_okf_package_menu(&self, composed: String) -> String {
        let Some(panel) = self.okf_picker.as_ref() else {
            return composed;
        };
        let width = self.width as usize;
        let menu = okf_picker_lines(
            &panel.packages,
            panel.sel,
            &panel.root,
            width,
            self.height as usize,
        );
        self.overlay_list(composed, &menu)
    }
}

fn okf_picker_header(total: usize, root: &std::path::Path, width: usize) -> String {
    truncate(
        &format!("  /okf packages · {total} · {}", root.display()),
        width,
    )
}

fn okf_picker_hint(width: usize) -> String {
    truncate(
        "  ↑↓ choose · Enter develop · Esc cancel · /kb for personal notes",
        width,
    )
}

fn okf_picker_lines(
    packages: &[OkfPackageAsset],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) = okf_picker_panel(packages, selected, root, width, height)
    else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn okf_picker_panel(
    packages: &[OkfPackageAsset],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Option<(MenuPanel, usize)> {
    let total = packages.len();
    if total == 0 {
        return None;
    }
    let max_items = height.saturating_sub(8).clamp(3, 12);
    let selected = selected.min(total.saturating_sub(1));
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let items = packages
        .iter()
        .map(|package| MenuItem::new(package.name.clone()).description(package.description.clone()))
        .collect::<Vec<_>>();

    let panel = MenuPanel::new(okf_picker_header(total, root, width).trim_start())
        .subtitle(okf_picker_hint(width).trim_start())
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

fn okf_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    pub(crate) fn on_okf_os_completed(
        &mut self,
        status_entry: TranscriptEntryId,
        res: Result<OkfOsResult, String>,
    ) {
        match res {
            Ok(result) => {
                self.last_view = Some(result.view.clone());
                self.replace_tracked_line(
                    status_entry,
                    &gutter(
                        TN_CYAN,
                        &format!(
                            "⌁ /okf {} · `{}` ({})",
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
                        &remote_view_button("Knowledge service · click to reopen"),
                    ));
                    self.open_remote_view(&result.view);
                } else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  Open view opens the related OS knowledge asset view"),
                    );
                }
            }
            Err(e) => {
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  /okf OS operation failed: {e}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    #[test]
    fn okf_generation_prompt_uses_capability_scoped_lifecycle() {
        let prompt = okf_package_gen_prompt("ops knowledge", "/tmp/work");

        assert!(prompt.contains("/tmp/work/okf"), "{prompt}");
        let old_root = [".a3s", "kb", "packages"].join("/");
        assert!(!prompt.contains(&old_root), "{prompt}");
        assert!(prompt.contains("create/develop/publish/deploy"), "{prompt}");
        assert!(prompt.contains("status/activity inspection"), "{prompt}");
        assert!(prompt.contains(".a3s/asset.acl"), "{prompt}");
        assert!(prompt.contains("Do NOT create"), "{prompt}");
        assert!(
            prompt.contains("extra generated JSON config files"),
            "{prompt}"
        );
        assert!(
            prompt.contains("keep package configuration in `.a3s/asset.acl`"),
            "{prompt}"
        );
        assert!(prompt.contains("metadata-only"), "{prompt}");
        assert!(prompt.contains("Do NOT put README"), "{prompt}");
        assert!(prompt.contains("runtimeIntent.kind=knowledge"), "{prompt}");
        assert!(
            prompt.contains("runtime.kind=a3s-knowledge-service"),
            "{prompt}"
        );
        assert!(prompt.contains("protocol=okf"), "{prompt}");
        assert!(!prompt.contains("create/develop/debug"), "{prompt}");
        assert!(!prompt.contains("run/debug"), "{prompt}");
        let stale_observe_phrase = ["publish", "deploy", "observe"].join("/");
        assert!(!prompt.contains(&stale_observe_phrase), "{prompt}");
        assert!(prompt.contains("Knowledge service indexing/evaluation"));
    }

    #[test]
    fn scaffold_okf_package_creates_sources_outside_metadata_acl() {
        let root =
            std::env::temp_dir().join(format!("a3s-code-okf-scaffold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let dev = scaffold_okf_package(
            "Name it exactly outage-runbook. It stores outage response guidance.",
            &root,
        )
        .unwrap();

        assert_eq!(dev.name, "outage-runbook");
        for rel in [
            "README.md",
            "sources/overview.md",
            "wiki/index.md",
            "wiki/concepts/example.md",
            "eval/smoke.md",
            ".a3s/asset.acl",
        ] {
            assert!(dev.path.join(rel).is_file(), "missing {rel}");
        }
        for rel in [
            "package.okf.json",
            "knowledge.asset.json",
            "knowledge.runtime-binding.json",
            ".a3s/knowledge.asset.json",
            ".a3s/knowledge.runtime-binding.json",
        ] {
            assert!(!dev.path.join(rel).exists(), "unexpected {rel}");
        }
        assert!(!dev.path.join(".a3s/wiki/index.md").exists());
        let source_files = collect_knowledge_source_files(&dev.path).unwrap();
        let paths = source_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(!paths.contains(&"package.okf.json"), "{paths:?}");
        assert!(paths.contains(&"wiki/index.md"), "{paths:?}");
        assert!(
            paths.iter().all(|path| !path.starts_with(".a3s/")),
            "{paths:?}"
        );
        let asset_acl =
            std::fs::read_to_string(dev.path.join(asset_lifecycle::ASSET_ACL_PATH)).unwrap();
        assert!(asset_acl.contains("category = \"knowledge\""));
        assert!(asset_acl.contains("readme_path = \"README.md\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn okf_package_dir_is_separate_from_personal_kb_vault() {
        let root = okf_package_dir("/tmp/work");
        assert_eq!(root, std::path::PathBuf::from("/tmp/work/okf"));
        assert!(!root.starts_with(kbutil::kb_dir("/tmp/work")));
    }

    #[test]
    fn okf_lifecycle_prompt_does_not_claim_run_command() {
        let session = OkfDevSession {
            name: "ops-knowledge".into(),
            description: "Operations knowledge package".into(),
            rel: "ops".into(),
            path: std::path::PathBuf::from("/Users/x/okf/ops"),
            root: std::path::PathBuf::from("/Users/x/okf/ops"),
        };
        let prompt = okf_lifecycle_prompt("deploy", &session, true);

        assert!(prompt.contains("Run `/okf deploy`"), "{prompt}");
        assert!(prompt.contains("knowledge-service binding"), "{prompt}");
        assert!(prompt.contains("indexing"), "{prompt}");
        assert!(!prompt.contains("For run"), "{prompt}");
        assert!(!prompt.contains("OKF bundle"), "{prompt}");
    }

    #[test]
    fn parses_explicit_okf_subcommands() {
        assert_eq!(parse_okf_command(""), OkfCommand::Select);
        assert_eq!(parse_okf_command("off"), OkfCommand::Exit);
        assert_eq!(
            parse_okf_command("exit"),
            OkfCommand::Usage("usage: /okf off")
        );
        assert_eq!(parse_okf_command("publish"), OkfCommand::Publish);
        assert_eq!(parse_okf_command("deploy"), OkfCommand::Deploy);
        assert_eq!(parse_okf_command("status"), OkfCommand::Status);
        assert_eq!(
            parse_okf_command("clone https://github.com/a/kb.git"),
            OkfCommand::Clone("https://github.com/a/kb.git".into())
        );
        assert_eq!(
            parse_okf_command("clone https://github.com/a/kb.git extra"),
            OkfCommand::Usage("usage: /okf clone <git-url>")
        );
        assert!(matches!(
            parse_okf_command("run"),
            OkfCommand::Usage(
                "OKF packages are not runnable assets; use /okf publish or /okf deploy"
            )
        ));
        assert!(matches!(
            parse_okf_command("debug now"),
            OkfCommand::Usage("unknown /okf command `debug`")
        ));
        assert!(matches!(
            parse_okf_command("logs"),
            OkfCommand::Usage("OKF packages do not expose logs; use /okf status or /okf activity")
        ));
        for removed in ["open", "view", "remote", "inspect", "os"] {
            assert_eq!(
                parse_okf_command(removed),
                OkfCommand::Usage("usage: /okf status")
            );
        }
        assert_eq!(
            parse_okf_command("dashboard"),
            OkfCommand::Usage("usage: /okf list [query] · /okf status")
        );
        for personal_kb_command in ["add", "import", "search", "vault"] {
            assert_eq!(
                parse_okf_command(personal_kb_command),
                OkfCommand::Usage(
                    "personal knowledge-base commands use /kb add/import/search/vault; OKF packages use /okf review/publish/deploy/status"
                )
            );
        }
        assert_eq!(
            parse_okf_command("activity stale indexes"),
            OkfCommand::Activity("stale indexes".into())
        );
        assert_eq!(
            parse_okf_command("ps"),
            OkfCommand::Usage("usage: /okf activity [query]")
        );
        assert!(matches!(
            parse_okf_command("jobs"),
            OkfCommand::Usage("usage: /okf activity [query]")
        ));
    }

    #[test]
    fn bare_okf_text_creates_a_package_prototype() {
        assert_eq!(
            parse_okf_command("some pasted note"),
            OkfCommand::Prototype("some pasted note".into())
        );
    }

    #[test]
    fn lists_okf_packages_from_package_root() {
        let root = std::env::temp_dir().join(format!("a3s-okf-packages-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops/sources")).unwrap();
        std::fs::write(
            root.join("ops/README.md"),
            "# ops-runbook\n\nOperations runbook\n",
        )
        .unwrap();
        std::fs::create_dir_all(root.join(".hidden")).unwrap();
        std::fs::write(root.join(".hidden/README.md"), "# hidden\n").unwrap();

        let packages = list_okf_packages(&root);
        assert_eq!(packages.len(), 1);
        assert_eq!(packages[0].name, "ops-runbook");
        assert_eq!(packages[0].description, "Operations runbook");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn okf_picker_lines_use_bounded_shared_menu_rows() {
        let root = std::path::PathBuf::from(
            "/Users/example/okf/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let packages = vec![
            OkfPackageAsset {
                rel: "nested/very-long-okf-package-name-that-would-overflow-the-panel".into(),
                path: root.join("nested/very-long-okf-package-name-that-would-overflow-the-panel"),
                name: "very-long-okf-package-name-that-would-overflow-the-panel".into(),
                description: "A long knowledge package description that should be trimmed cleanly"
                    .into(),
            },
            OkfPackageAsset {
                rel: "ops-runbook".into(),
                path: root.join("ops-runbook"),
                name: "ops-runbook".into(),
                description: "Operations runbook".into(),
            },
        ];
        let lines = okf_picker_lines(&packages, 0, &root, 40, 20);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("okf packages"), "{plain}");
        assert!(plain.contains("very-long-okf"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn okf_picker_lines_scroll_selected_package_into_view() {
        let root = std::path::PathBuf::from("/tmp/okf");
        let packages = (0..16)
            .map(|index| OkfPackageAsset {
                rel: format!("okf-{index}"),
                path: root.join(format!("okf-{index}")),
                name: format!("okf-{index}"),
                description: format!("Knowledge package {index}"),
            })
            .collect::<Vec<_>>();
        let plain = okf_picker_lines(&packages, 14, &root, 48, 16)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("okf-14"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn okf_picker_header_and_hint_fit_fixed_width() {
        let root = std::path::PathBuf::from(
            "/Users/example/okf/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let header = okf_picker_header(9, &root, 40);
        let hint = okf_picker_hint(40);
        assert!(a3s_tui::style::visible_len(&header) <= 40, "{header}");
        assert!(a3s_tui::style::visible_len(&hint) <= 40, "{hint}");
    }

    #[test]
    fn okf_picker_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let root = std::path::PathBuf::from("/tmp/okf");
        let packages = (0..4)
            .map(|index| OkfPackageAsset {
                rel: format!("okf-{index}"),
                path: root.join(format!("okf-{index}")),
                name: format!("okf-{index}"),
                description: format!("Knowledge package {index}"),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = okf_picker_lines(&packages, 0, &root, width, height).len();
        let y_offset = okf_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = okf_picker_panel(&packages, 0, &root, width, height).expect("panel");
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
    fn okf_picker_click_selects_visible_row_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let root = std::path::PathBuf::from("/tmp/okf");
        let packages = (0..4)
            .map(|index| OkfPackageAsset {
                rel: format!("okf-{index}"),
                path: root.join(format!("okf-{index}")),
                name: format!("okf-{index}"),
                description: format!("Knowledge package {index}"),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = okf_picker_lines(&packages, 0, &root, width, height).len();
        let y_offset = okf_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = okf_picker_panel(&packages, 0, &root, width, height).expect("panel");
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
    fn okf_overlay_offset_moves_up_with_more_rows_below() {
        assert!(okf_overlay_y_offset(24, 8, 7) < okf_overlay_y_offset(24, 8, 5));
    }

    #[test]
    fn okf_dev_prompt_keeps_work_local_and_names_exit_path() {
        let session = OkfDevSession {
            name: "ops-runbook".into(),
            description: "Operations runbook".into(),
            rel: "ops".into(),
            path: std::path::PathBuf::from("/Users/x/okf/ops"),
            root: std::path::PathBuf::from("/Users/x/okf"),
        };
        let prompt = okf_dev_prompt(&session, "add an incident concept");
        assert!(prompt.contains("ops-runbook"));
        assert!(prompt.contains("Do not open OS"));
        assert!(prompt.contains("/okf off"));
    }

    #[test]
    fn knowledge_package_metadata_carries_knowledge_service_binding() {
        let dev = OkfDevSession {
            name: "ops-runbook".into(),
            description: "Operations runbook".into(),
            rel: "ops".into(),
            path: std::path::PathBuf::from("/Users/x/okf/ops"),
            root: std::path::PathBuf::from("/Users/x/okf"),
        };
        let manifest = knowledge_manifest_json(&dev, "knowledge-ops-runbook");
        let binding = knowledge_runtime_binding_json(&dev, "knowledge-ops-runbook");
        let upsert = knowledge_runtime_binding_upsert_body(&binding);

        assert_eq!(knowledge_asset_name("Ops Runbook"), "knowledge-ops-runbook");
        assert_eq!(manifest["category"], "knowledge");
        assert_eq!(manifest["service"], "Knowledge service");
        assert_eq!(manifest["runtimeIntent"]["kind"], "knowledge");
        assert_eq!(
            manifest["runtimeIntent"]["runtimeKind"],
            "a3s-knowledge-service"
        );
        assert_eq!(manifest["runtimeIntent"]["protocol"], "okf");
        assert_eq!(binding["kind"], "knowledge");
        assert_eq!(binding["isolation"], "serving");
        assert_eq!(binding["runtime"]["kind"], "a3s-knowledge-service");
        assert_eq!(binding["runtime"]["protocol"], "okf");
        assert_eq!(binding["metadata"]["service"], "Knowledge service");
        assert_eq!(upsert["kind"], "service");
        assert_eq!(upsert["runtime"]["sharedRuntime"], "node-20");
        assert!(upsert["runtime"].get("protocol").is_none());
    }

    #[test]
    fn existing_knowledge_asset_must_match_knowledge_category() {
        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "skill-asset",
                        "name": "knowledge-ops-runbook",
                        "category": "skill"
                    }
                ]
            }
        });

        let err = find_knowledge_asset(&found, "knowledge-ops-runbook").unwrap_err();
        assert!(err.contains("category=skill"), "{err}");
        assert!(err.contains("expected knowledge"), "{err}");
    }

    #[test]
    fn knowledge_progressive_score_prefers_knowledge_viewlinks() {
        let asset = KnowledgeAssetRef {
            id: "asset-1".into(),
            name: "knowledge-ops-runbook".into(),
        };
        let params = knowledge_progressive_params(&asset, "ops-runbook", OkfOsAction::Deploy);
        assert_eq!(params["assetId"], "asset-1");
        assert_eq!(params["operation"], "deploy");
        assert_eq!(params["input"]["packageName"], "ops-runbook");
        assert!(
            knowledge_progressive_score(
                OkfOsAction::Deploy,
                "Knowledge service OKF index evaluate report shaped RemoteUI ViewLink",
                "KnowledgePackageController_runIndex"
            ) > 0
        );
        assert_eq!(
            knowledge_progressive_score(
                OkfOsAction::Deploy,
                "Function as a Service batch MCP tools shaped view",
                "FunctionController_batch"
            ),
            0
        );
        assert_eq!(
            knowledge_progressive_score(
                OkfOsAction::Deploy,
                "Workflow as a Service designer ViewLink",
                "WorkflowDesignerController_open"
            ),
            0
        );
        assert_eq!(
            knowledge_progressive_score(
                OkfOsAction::Deploy,
                "Knowledge service deploy OKF package shaped ViewLink",
                "createAsset",
            ),
            0,
            "generic asset create must not drive /okf deploy"
        );
        assert_eq!(
            knowledge_progressive_score(
                OkfOsAction::Deploy,
                "Knowledge service diagnostics",
                "AssetDiagnoseController_acknowledgeReport",
            ),
            0,
            "diagnosis acknowledgement must not drive /okf deploy"
        );
    }

    #[test]
    fn knowledge_source_upload_collects_only_visible_sources() {
        let root = std::env::temp_dir().join(format!("a3s-kb-source-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops/.a3s")).unwrap();
        std::fs::create_dir_all(root.join("ops/wiki")).unwrap();
        std::fs::write(root.join("ops/README.md"), "# Operations\n").unwrap();
        std::fs::write(root.join("ops/wiki/index.md"), "concept").unwrap();
        std::fs::write(
            root.join("ops/.a3s/asset.acl"),
            "version = \"a3s.asset.v1\"",
        )
        .unwrap();

        let files = collect_knowledge_source_files(&root.join("ops")).unwrap();
        let paths = files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["README.md", "wiki/index.md"]);

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn publish_okf_to_os_uploads_package_and_syncs_knowledge_binding() {
        let root = std::env::temp_dir().join(format!("a3s-kb-publish-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops/wiki")).unwrap();
        std::fs::write(
            root.join("ops/README.md"),
            "# ops-runbook\n\nOperations runbook\n",
        )
        .unwrap();
        std::fs::write(root.join("ops/wiki/index.md"), "# Operations").unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_kb_publish_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = OkfDevSession {
            name: "ops-runbook".into(),
            description: "Operations runbook".into(),
            rel: "ops".into(),
            path: root.join("ops"),
            root: root.clone(),
        };

        let result = publish_okf_to_os(session, dev, OkfOsAction::Publish)
            .await
            .expect("OKF publish should use OS knowledge asset APIs");

        assert_eq!(result.asset_name, "knowledge-ops-runbook");
        assert_eq!(result.asset_id, "knowledge-asset-1");
        assert!(result.note.contains("Knowledge service"), "{}", result.note);
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
            joined.contains("POST /api/v1/assets/knowledge-asset-1/repository/files HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/knowledge-asset-1/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains(
                "POST /api/v1/assets/knowledge-asset-1/runtime-binding/validate HTTP/1.1"
            ),
            "{joined}"
        );

        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "knowledge");
        assert_eq!(create_json["metadata"]["service"], "Knowledge service");
        assert_eq!(
            create_json["metadata"]["runtimeKind"],
            "a3s-knowledge-service"
        );
        assert_eq!(create_json["metadata"]["protocol"], "okf");
        assert_eq!(
            create_json["metadata"]["operations"],
            serde_json::json!(["index", "evaluate", "report"])
        );
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/knowledge-asset-1/repository/files HTTP/1.1",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let files = upload_json["files"].as_array().unwrap();
        assert!(files.iter().any(|file| file["path"] == "README.md"));
        assert!(files.iter().any(|file| file["path"] == "wiki/index.md"));
        assert!(files
            .iter()
            .any(|file| file["path"] == asset_lifecycle::ASSET_ACL_PATH));
        for forbidden in [
            "package.okf.json",
            "knowledge.asset.json",
            "knowledge.runtime-binding.json",
            "runtime-binding.json",
        ] {
            assert!(
                files.iter().all(|file| file["path"] != forbidden),
                "repository upload should not include {forbidden}"
            );
        }

        let synced = request_body(
            &requests,
            "PUT /api/v1/assets/knowledge-asset-1/runtime-binding HTTP/1.1",
        );
        let synced_json: serde_json::Value = serde_json::from_str(&synced).unwrap();
        assert_eq!(synced_json["kind"], "service");
        assert_eq!(synced_json["runtime"]["sharedRuntime"], "node-20");
        assert!(synced_json["runtime"].get("protocol").is_none());
        let local_asset_acl = root.join("ops").join(asset_lifecycle::ASSET_ACL_PATH);
        assert!(local_asset_acl.is_file(), "missing local asset.acl");
        let local_asset_acl_body = std::fs::read_to_string(&local_asset_acl).unwrap();
        assert!(local_asset_acl_body.contains("category = \"knowledge\""));
        assert!(local_asset_acl_body.contains("readme_path = \"README.md\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn deploy_okf_to_os_uses_progressive_knowledge_service_view_when_available() {
        let root = std::env::temp_dir().join(format!("a3s-kb-deploy-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("ops/wiki")).unwrap();
        std::fs::write(
            root.join("ops/README.md"),
            "# ops-runbook\n\nOperations runbook\n",
        )
        .unwrap();
        std::fs::write(root.join("ops/wiki/index.md"), "# Operations").unwrap();
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_kb_publish_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let dev = OkfDevSession {
            name: "ops-runbook".into(),
            description: "Operations runbook".into(),
            rel: "ops".into(),
            path: root.join("ops"),
            root: root.clone(),
        };

        let result = publish_okf_to_os(session, dev, OkfOsAction::Deploy)
            .await
            .expect("OKF deploy should use OS Knowledge service");

        assert_eq!(result.action, OkfOsAction::Deploy);
        assert_eq!(result.asset_name, "knowledge-ops-runbook");
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/knowledge/knowledge-asset-1/deploy?embed=1")
        );
        assert!(
            result.note.contains("progressive capabilities")
                && result.note.contains("runtime binding was synced"),
            "{}",
            result.note
        );

        let requests = captured.lock().unwrap().clone();
        let joined = requests.join("\n");
        assert!(
            joined.contains("POST /api/v1/assets/knowledge-asset-1/repository/files HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/knowledge-asset-1/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("POST /api/v1/kernel/capabilities HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains(r#""action":"search""#)
                && joined.contains("Knowledge service deploy OKF package shaped ViewLink"),
            "{joined}"
        );
        assert!(
            joined.contains(r#""action":"execute""#)
                && joined.contains(r#""shaped":true"#)
                && joined.contains("KnowledgePackageController_deploy")
                && joined.contains(r#""packageName":"ops-runbook""#),
            "{joined}"
        );

        let _ = std::fs::remove_dir_all(&root);
    }

    async fn spawn_kb_publish_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = kb_publish_mock_response(&line, &body);
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

    fn kb_publish_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if line.starts_with("POST /api/v1/kernel/capabilities HTTP/1.1") {
            if body.contains(r#""action":"search""#)
                && body.contains("Knowledge service deploy OKF package shaped ViewLink")
            {
                return (
                    "200 OK",
                    r#"{"data":{"results":[{"module":"knowledge","operation":"KnowledgePackageController_deploy","description":"Knowledge service deploy OKF package shaped ViewLink"}]}}"#,
                );
            }
            if body.contains(r#""action":"describe""#)
                && body.contains("KnowledgePackageController_deploy")
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"operation":{"name":"KnowledgePackageController_deploy","inputSchema":{"body":{"properties":{"assetId":{"type":"string"},"packageName":{"type":"string"},"input":{"type":"object"},"idempotencyKey":{"type":"string"}}}}}}}"#,
                );
            }
            if body.contains(r#""action":"execute""#)
                && body.contains(r#""shaped":true"#)
                && body.contains("KnowledgePackageController_deploy")
                && body.contains(r#""assetId":"knowledge-asset-1""#)
                && body.contains(r#""packageName":"ops-runbook""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"deploymentId":"knowledge-deploy-1"},"view":{"url":"/admin/knowledge/knowledge-asset-1/deploy?embed=1","width":1280,"height":860}}"#,
                );
            }
            return ("404 Not Found", r#"{"code":404,"message":"not found"}"#);
        }
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""category":"knowledge""#)
                && body.contains(r#""service":"Knowledge service""#)
                && body.contains(r#""runtimeKind":"a3s-knowledge-service""#)
                && body.contains(r#""protocol":"okf""#)
                && body.contains(r#""operations":["index","evaluate","report"]"#)
                && body.contains(r#""createdBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"knowledge-asset-1","name":"knowledge-ops-runbook"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad knowledge asset body"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/knowledge-asset-1/repository/files HTTP/1.1") {
            if body.contains("README.md")
                && body.contains("wiki/index.md")
                && body.contains(asset_lifecycle::ASSET_ACL_PATH)
                && !body.contains("package.okf.json")
                && !body.contains("knowledge.asset.json")
                && !body.contains("knowledge.runtime-binding.json")
            {
                return ("200 OK", r#"{"ok":true}"#);
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad knowledge upload body"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/knowledge-asset-1/runtime-binding HTTP/1.1") {
            if body.contains(r#""kind":"service""#)
                && body.contains(r#""a3s-knowledge-service""#)
                && body.contains(r#""sharedRuntime":"node-20""#)
                && !body.contains(r#""version""#)
                && !body.contains(r#""protocol":"okf""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"knowledge-asset-1","configured":true}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad runtime binding"}"#,
            );
        }
        if line
            .starts_with("POST /api/v1/assets/knowledge-asset-1/runtime-binding/validate HTTP/1.1")
        {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"knowledge-asset-1","configured":true,"valid":true,"issues":[]}}"#,
            );
        }
        ("404 Not Found", r#"{"code":404,"message":"not found"}"#)
    }
}
