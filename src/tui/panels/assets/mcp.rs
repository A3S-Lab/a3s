//! `/mcp` — local multi-turn development for MCP server assets.
//!
//! Bare `/mcp` opens a picker over `mcp_dir()` (`~/.a3s/mcps` or the `mcp_dir`
//! config key). Enter puts the TUI into a local MCP-development context. Later
//! OS subcommands can publish/deploy the asset and, when OS exposes a real MCP
//! runner capability, run/test it. Local selection itself never opens OS or
//! RemoteUI.

use super::super::asset_lifecycle;
use super::super::os_progressive;
use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;

const MAX_MCP_SOURCE_FILES: usize = 200;
const MAX_MCP_SOURCE_BYTES: u64 = 4 * 1024 * 1024;

#[derive(Clone)]
pub(crate) struct McpProject {
    pub(crate) rel: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) name: String,
    pub(crate) description: String,
}

/// `/mcp` selection panel: local MCP assets + cursor.
pub(crate) struct McpPanel {
    /// Absolute path of the MCP root (config `mcp_dir`).
    pub(crate) root: std::path::PathBuf,
    /// MCP assets under the root, sorted by relative path.
    pub(crate) projects: Vec<McpProject>,
    pub(crate) sel: usize,
}

/// The local MCP asset currently being developed in the TUI.
#[derive(Clone)]
pub(crate) struct McpDevSession {
    pub(crate) name: String,
    pub(crate) description: String,
    pub(crate) rel: String,
    pub(crate) path: std::path::PathBuf,
    pub(crate) root: std::path::PathBuf,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum McpOsAction {
    Publish,
    Run,
    Deploy,
    Test,
    Open,
    Logs,
    Status,
}

impl McpOsAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Run => "run",
            Self::Deploy => "deploy",
            Self::Test => "test",
            Self::Open => "open",
            Self::Logs => "logs",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum McpSubcommand {
    Exit,
    Clone(String),
    List(String),
    Review,
    Activity(String),
    Publish,
    Run,
    Deploy,
    Test,
    Open,
    Logs,
    Status,
}

impl McpSubcommand {
    pub(crate) fn os_action(&self) -> Option<McpOsAction> {
        match self {
            Self::Exit | Self::Clone(_) | Self::List(_) | Self::Review | Self::Activity(_) => None,
            Self::Publish => Some(McpOsAction::Publish),
            Self::Run => Some(McpOsAction::Run),
            Self::Deploy => Some(McpOsAction::Deploy),
            Self::Test => Some(McpOsAction::Test),
            Self::Open => Some(McpOsAction::Open),
            Self::Logs => Some(McpOsAction::Logs),
            Self::Status => Some(McpOsAction::Status),
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct McpOsResult {
    pub(crate) action: McpOsAction,
    pub(crate) asset_name: String,
    pub(crate) asset_id: String,
    pub(crate) view: remote_ui::ViewSpec,
    pub(crate) note: String,
    pub(crate) open_view: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct McpAssetRef {
    id: String,
    name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct McpSourceFile {
    path: String,
    bytes: Vec<u8>,
}

const MCP_MARKERS: &[&str] = &["server.js", "server.py", "mcp.py"];

/// List MCP assets recursively, skipping dot-directories except the asset-local
/// `.a3s` metadata folder. An asset is any directory with an MCP marker.
pub(crate) fn list_mcp_projects(root: &std::path::Path) -> Vec<McpProject> {
    let mut out = Vec::new();
    list_mcp_projects_inner(root, root, &mut out);
    out.sort_by(|a, b| a.rel.cmp(&b.rel));
    out.dedup_by(|a, b| a.rel == b.rel);
    out
}

fn list_mcp_projects_inner(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<McpProject>,
) {
    if let Some(project) = mcp_project_from_dir(root, dir) {
        out.push(project);
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        list_mcp_projects_inner(root, &path, out);
    }
}

fn mcp_project_from_dir(root: &std::path::Path, dir: &std::path::Path) -> Option<McpProject> {
    if !MCP_MARKERS.iter().any(|marker| dir.join(marker).is_file()) {
        return None;
    }
    let rel = dir
        .strip_prefix(root)
        .ok()
        .and_then(|p| {
            let s = p.to_string_lossy().replace('\\', "/");
            (!s.is_empty()).then_some(s)
        })
        .unwrap_or_else(|| ".".to_string());
    let (name, description) = mcp_project_metadata(dir).unwrap_or_else(|| {
        (
            mcp_name_from_rel(&rel),
            "Local MCP server asset".to_string(),
        )
    });
    Some(McpProject {
        rel,
        path: dir.to_path_buf(),
        name,
        description,
    })
}

fn mcp_project_metadata(dir: &std::path::Path) -> Option<(String, String)> {
    visible_readme_metadata(&dir.join("README.md"))
}

fn visible_readme_metadata(path: &std::path::Path) -> Option<(String, String)> {
    let body = std::fs::read_to_string(path).ok()?;
    let mut title = None;
    let mut description = None;
    for line in body.lines() {
        let line = line.trim();
        if title.is_none() {
            if let Some(name) = line.strip_prefix("# ") {
                let name = name.trim();
                if !name.is_empty() {
                    title = Some(name.to_string());
                }
            }
            continue;
        }
        if description.is_none() && !line.is_empty() && !line.starts_with('#') {
            description = Some(line.trim_end_matches('.').to_string());
            break;
        }
    }
    title.map(|name| {
        (
            name,
            description.unwrap_or_else(|| "Local MCP server asset".to_string()),
        )
    })
}

pub(crate) fn scaffold_mcp_project(
    description: &str,
    root: &std::path::Path,
) -> Result<McpDevSession, String> {
    let name = asset_lifecycle::scaffold_name(description, "mcp-server");
    let package = asset_lifecycle::unique_asset_dir(root, &name);
    let final_name = package
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name.as_str())
        .to_string();
    let description =
        asset_lifecycle::scaffold_description(description, &final_name, "Local MCP server asset");

    std::fs::create_dir_all(package.join(".a3s"))
        .map_err(|e| format!("could not create {}: {e}", package.join(".a3s").display()))?;
    for dir in ["examples", "tests"] {
        std::fs::create_dir_all(package.join(dir))
            .map_err(|e| format!("could not create {}: {e}", package.join(dir).display()))?;
    }

    let server_config = serde_json::json!({
        "version": "a3s.mcp.server.v1",
        "name": final_name,
        "description": description,
        "transport": "stdio",
        "command": "node",
        "args": ["server.js"],
        "entrypoint": "server.js",
        "tools": [
            {
                "name": "health",
                "description": "Return a lightweight health response for this MCP asset.",
                "inputSchema": {
                    "type": "object",
                    "properties": {},
                    "additionalProperties": false
                }
            }
        ],
        "category": "mcp",
        "runtimeIntent": {
            "service": "Function as a Service",
            "runtimeBinding": {
                "kind": "mcp",
                "isolation": "serving",
                "agentKind": "tool",
                "runtimeKind": "a3s-function-service",
                "protocol": "mcp"
            }
        }
    });
    let rel = asset_lifecycle::normalized_rel(root, &package);
    let dev = McpDevSession {
        name: final_name.clone(),
        description: description.clone(),
        rel,
        path: package.clone(),
        root: root.to_path_buf(),
    };
    let asset_name = mcp_asset_name(&final_name);
    let asset_acl = mcp_asset_acl(&dev, &asset_name, &server_config);

    asset_lifecycle::write_scaffold_file(
        &package.join("server.js"),
        mcp_scaffold_server_js(&description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("README.md"),
        mcp_scaffold_readme(&final_name, &description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("examples/example-request.md"),
        mcp_scaffold_example_request().as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("tests/smoke.md"),
        mcp_scaffold_tests().as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join(asset_lifecycle::ASSET_ACL_PATH),
        asset_acl.as_bytes(),
    )?;

    Ok(dev)
}

fn mcp_scaffold_server_js(description: &str) -> String {
    let description = serde_json::to_string(description).unwrap_or_else(|_| "\"MCP asset\"".into());
    format!(
        "const description = {description};\n\
         \n\
         if (process.argv.includes('--smoke')) {{\n\
         \x20 console.log(JSON.stringify({{ ok: true, description }}));\n\
         \x20 process.exit(0);\n\
         }}\n\
         \n\
         process.stdin.setEncoding('utf8');\n\
         let buffer = '';\n\
         process.stdin.on('data', (chunk) => {{\n\
         \x20 buffer += chunk;\n\
         \x20 for (;;) {{\n\
         \x20\x20 const nl = buffer.indexOf('\\n');\n\
         \x20\x20 if (nl < 0) break;\n\
         \x20\x20 const line = buffer.slice(0, nl).trim();\n\
         \x20\x20 buffer = buffer.slice(nl + 1);\n\
         \x20\x20 if (!line) continue;\n\
         \x20\x20 const request = JSON.parse(line);\n\
         \x20\x20 const response = request.method === 'tools/call'\n\
         \x20\x20\x20 ? {{ jsonrpc: '2.0', id: request.id, result: {{ content: [{{ type: 'text', text: `ok: ${{description}}` }}] }} }}\n\
         \x20\x20\x20 : {{ jsonrpc: '2.0', id: request.id, result: {{ tools: [{{ name: 'health', description }}] }} }};\n\
         \x20\x20 process.stdout.write(JSON.stringify(response) + '\\n');\n\
         \x20 }}\n\
         }});\n"
    )
}

fn mcp_scaffold_readme(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n\
         {description}.\n\n\
         ## Source\n\n\
         - `server.js` is the MCP server source entrypoint.\n\
         - `examples/` and `tests/` contain smoke fixtures.\n\
         - `.a3s/` contains only `asset.acl`.\n\n\
         ## Lifecycle\n\n\
         - `a3s code mcp publish .`\n\
         - `a3s code mcp run .`\n\
         - `a3s code mcp test .`\n\
         - `a3s code mcp deploy .`\n"
    )
}

fn mcp_scaffold_tests() -> &'static str {
    "# MCP Smoke Checklist\n\n\
     1. Run `node --check server.js`.\n\
     2. Run `node server.js --smoke`.\n\
     3. Confirm `.a3s/asset.acl` points source to `server.js`.\n"
}

fn mcp_scaffold_example_request() -> &'static str {
    "# Example Request\n\n\
     Call the `health` tool with an empty argument object during local smoke testing.\n"
}

fn mcp_name_from_rel(rel: &str) -> String {
    rel.rsplit('/')
        .find(|part| !part.trim().is_empty() && *part != ".")
        .unwrap_or("mcp-server")
        .to_string()
}

pub(crate) fn parse_mcp_subcommand(input: &str) -> Option<Result<McpSubcommand, String>> {
    let mut parts = input.split_whitespace();
    let head = parts.next()?.to_ascii_lowercase();
    match head.as_str() {
        "off" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp off".to_string()));
            }
            Some(Ok(McpSubcommand::Exit))
        }
        "exit" | "normal" | "clear" | "stop" => Some(Err("usage: /mcp off".to_string())),
        "clone" => {
            let Some(url) = parts.next() else {
                return Some(Err("usage: /mcp clone <git-url>".to_string()));
            };
            if parts.next().is_some() {
                return Some(Err("usage: /mcp clone <git-url>".to_string()));
            }
            Some(Ok(McpSubcommand::Clone(url.to_string())))
        }
        "list" => Some(Ok(McpSubcommand::List(parts.collect::<Vec<_>>().join(" ")))),
        "review" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp review".to_string()));
            }
            Some(Ok(McpSubcommand::Review))
        }
        "activity" => Some(Ok(McpSubcommand::Activity(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "ps" | "runs" | "jobs" => Some(Err("usage: /mcp activity [query]".to_string())),
        "publish" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp publish".to_string()));
            }
            Some(Ok(McpSubcommand::Publish))
        }
        "deploy" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp deploy".to_string()));
            }
            Some(Ok(McpSubcommand::Deploy))
        }
        "run" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp run".to_string()));
            }
            Some(Ok(McpSubcommand::Run))
        }
        "debug" | "invoke" => Some(Err(format!("unknown /mcp command `{head}`"))),
        "test" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp test".to_string()));
            }
            Some(Ok(McpSubcommand::Test))
        }
        "batch" => Some(Err("unknown /mcp command `batch`".to_string())),
        "open" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp open".to_string()));
            }
            Some(Ok(McpSubcommand::Open))
        }
        "logs" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp logs".to_string()));
            }
            Some(Ok(McpSubcommand::Logs))
        }
        "status" => {
            if parts.next().is_some() {
                return Some(Err("usage: /mcp status".to_string()));
            }
            Some(Ok(McpSubcommand::Status))
        }
        "inspect" => Some(Err("usage: /mcp status".to_string())),
        "view" | "remote" => Some(Err("usage: /mcp open".to_string())),
        "os" => Some(Err("usage: /mcp status".to_string())),
        "dashboard" => Some(Err("usage: /mcp list [query] · /mcp status".to_string())),
        _ => None,
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

fn envelope_data(body: &serde_json::Value) -> &serde_json::Value {
    body.get("data").unwrap_or(body)
}

fn response_message(body: &serde_json::Value) -> &str {
    body.get("message")
        .and_then(|m| m.as_str())
        .unwrap_or("request failed")
}

fn envelope_json_is_error(value: &serde_json::Value) -> bool {
    let code = value
        .get("code")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("statusCode").and_then(|v| v.as_i64()))
        .unwrap_or(200);
    code >= 400
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

fn mcp_asset_name(name: &str) -> String {
    format!("mcp-{}", asset_slug(name))
}

fn mcp_asset_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/assets/{}?embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn mcp_asset_search_url(origin: &str, asset_name: &str) -> String {
    format!(
        "{}/admin/kernel/assets?focus=1&category=mcp&scope=mine&status=all&search={}&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_name)
    )
}

fn mcp_logs_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/infrastructure/batch?asset={}&agentKind=tool&category=mcp&logs=1&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn mcp_function_view_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/infrastructure/batch?asset={}&agentKind=tool&category=mcp&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn mcp_view_spec(url: String) -> remote_ui::ViewSpec {
    remote_ui::ViewSpec {
        url,
        width: Some(1440),
        height: Some(900),
        embeddable: true,
    }
}

fn mcp_metadata_value(dev: &McpDevSession) -> serde_json::Value {
    let entrypoint = ["server.js", "server.py", "mcp.py"]
        .into_iter()
        .find(|rel| dev.path.join(rel).is_file())
        .unwrap_or("server.js");
    serde_json::json!({
        "name": dev.name,
        "description": dev.description,
        "transport": "stdio",
        "command": if entrypoint.ends_with(".py") { "python3" } else { "node" },
        "args": [entrypoint],
        "entrypoint": entrypoint,
        "tools": [],
    })
}

fn mcp_config_name(config: &serde_json::Value, dev: &McpDevSession) -> String {
    json_str_at(config, &["/name", "name"])
        .unwrap_or(&dev.name)
        .to_string()
}

fn mcp_config_description(config: &serde_json::Value, dev: &McpDevSession) -> String {
    json_str_at(config, &["/description", "description"])
        .unwrap_or(&dev.description)
        .to_string()
}

fn mcp_server_config_json(dev: &McpDevSession, raw: &serde_json::Value) -> serde_json::Value {
    let mut config = raw.clone();
    if !config.is_object() {
        config = serde_json::json!({});
    }
    let name = mcp_config_name(&config, dev);
    let description = mcp_config_description(&config, dev);
    let obj = config.as_object_mut().expect("object ensured");
    obj.entry("name".to_string())
        .or_insert_with(|| serde_json::json!(name));
    obj.entry("description".to_string())
        .or_insert_with(|| serde_json::json!(description));
    obj.entry("category".to_string())
        .or_insert_with(|| serde_json::json!("mcp"));
    obj.entry("runtimeIntent".to_string()).or_insert_with(|| {
        serde_json::json!({
            "service": "Function as a Service",
            "runtimeBinding": {
                "kind": "mcp",
                "isolation": "serving",
                "agentKind": "tool",
                "runtimeKind": "a3s-function-service",
                "protocol": "mcp",
            },
        })
    });
    config
}

fn mcp_tool_names(config: &serde_json::Value) -> Vec<String> {
    let Some(tools) = config.get("tools") else {
        return Vec::new();
    };
    let mut out = Vec::new();
    match tools {
        serde_json::Value::Array(items) => {
            for item in items {
                match item {
                    serde_json::Value::String(name) => out.push(name.trim().to_string()),
                    serde_json::Value::Object(_) => {
                        if let Some(name) = json_str_at(item, &["/name", "name", "/id", "id"]) {
                            out.push(name.to_string());
                        }
                    }
                    _ => {}
                }
            }
        }
        serde_json::Value::Object(map) => {
            out.extend(map.keys().cloned());
        }
        _ => {}
    }
    out.retain(|name| !name.trim().is_empty());
    out.sort();
    out.dedup();
    out
}

fn mcp_manifest_json(
    dev: &McpDevSession,
    asset_name: &str,
    server_config: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.mcp.asset.v1",
        "category": "mcp",
        "name": asset_name,
        "mcpName": mcp_config_name(server_config, dev),
        "description": mcp_config_description(server_config, dev),
        "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        "localPath": dev.rel,
        "service": "Function as a Service",
        "runtimeIntent": {
            "kind": "mcp",
            "isolation": "serving",
            "agentKind": "tool",
            "runtimeKind": "a3s-function-service",
            "protocol": "mcp",
        },
        "createdBy": "a3s-code-tui",
        "tools": mcp_tool_names(server_config),
    })
}

fn mcp_runtime_binding_json(
    dev: &McpDevSession,
    asset_name: &str,
    server_config: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.mcp.runtime-binding.v1",
        "kind": "mcp",
        "enabled": true,
        "isolation": "serving",
        "target": {
            "kind": "asset",
            "ref": "main",
            "packageRoot": ".",
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        },
        "runtime": {
            "kind": "a3s-function-service",
            "protocol": "mcp",
            "agentKind": "tool",
        },
        "env": [],
        "requiredSecrets": [],
        "resources": {
            "concurrency": 1,
        },
        "network": {
            "ingress": false,
        },
        "metadata": {
            "source": "a3s-code-tui",
            "assetCategory": "mcp",
            "assetName": asset_name,
            "mcpName": mcp_config_name(server_config, dev),
            "description": mcp_config_description(server_config, dev),
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
            "localPath": dev.rel,
            "tools": mcp_tool_names(server_config),
        },
    })
}

fn mcp_asset_acl(
    dev: &McpDevSession,
    asset_name: &str,
    server_config: &serde_json::Value,
) -> String {
    let source = [("entrypoint", "server.js"), ("package_root", ".")];
    let metadata: [(&str, &str); 0] = [];
    let description = mcp_config_description(server_config, dev);
    asset_lifecycle::render_asset_acl(asset_lifecycle::AssetAclDocument {
        category: "mcp",
        kind: Some("mcp"),
        name: asset_name,
        description: &description,
        local_path: Some(dev.rel.as_str()),
        service: asset_lifecycle::OsService::FunctionAsAService,
        runtime: asset_lifecycle::RuntimeBindingIntent {
            kind: "mcp",
            isolation: "serving",
            runtime_kind: "a3s-function-service",
            protocol: Some("mcp"),
            agent_kind: Some("tool"),
        },
        source: &source,
        metadata: &metadata,
    })
}

fn collect_mcp_source_files(root: &std::path::Path) -> Result<Vec<McpSourceFile>, String> {
    let mut out = Vec::new();
    let mut total = 0_u64;
    collect_mcp_source_files_inner(root, root, &mut out, &mut total)?;
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

fn collect_mcp_source_files_inner(
    root: &std::path::Path,
    dir: &std::path::Path,
    out: &mut Vec<McpSourceFile>,
    total: &mut u64,
) -> Result<(), String> {
    let entries = std::fs::read_dir(dir)
        .map_err(|e| format!("could not read MCP asset directory {}: {e}", dir.display()))?;
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if should_skip_mcp_dir(&name) {
                continue;
            }
            collect_mcp_source_files_inner(root, &path, out, total)?;
            continue;
        }
        if !path.is_file() || should_skip_mcp_file(&name) {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if rel.starts_with(".a3s/") {
            continue;
        }
        let meta = std::fs::metadata(&path)
            .map_err(|e| format!("could not stat MCP asset file {}: {e}", path.display()))?;
        *total = total.saturating_add(meta.len());
        if out.len() >= MAX_MCP_SOURCE_FILES || *total > MAX_MCP_SOURCE_BYTES {
            return Err(format!(
                "MCP asset is too large for direct asset upload (limit: {MAX_MCP_SOURCE_FILES} files / {MAX_MCP_SOURCE_BYTES} bytes)"
            ));
        }
        let bytes = std::fs::read(&path)
            .map_err(|e| format!("could not read MCP asset file {}: {e}", path.display()))?;
        out.push(McpSourceFile { path: rel, bytes });
    }
    Ok(())
}

fn should_skip_mcp_dir(name: &str) -> bool {
    matches!(
        name,
        ".a3s" | ".git" | "node_modules" | "target" | "dist" | "build" | ".venv" | "__pycache__"
    )
}

fn should_skip_mcp_file(name: &str) -> bool {
    matches!(
        name,
        ".DS_Store"
            | "mcp.asset.json"
            | "mcp.server.json"
            | "mcp.runtime-binding.json"
            | "runtime-binding.json"
            | "mcp.json"
            | "package.json"
    ) || name.starts_with(".env")
}

fn mcp_asset_ref_from_value(asset: &serde_json::Value, fallback_name: &str) -> Option<McpAssetRef> {
    let id = json_str_at(asset, &["/id", "id"])?.to_string();
    Some(McpAssetRef {
        id,
        name: json_str_at(asset, &["/name", "name"])
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

fn find_mcp_asset(found: &serde_json::Value, name: &str) -> Result<Option<McpAssetRef>, String> {
    let exact = items_of(found)
        .into_iter()
        .find(|a| a.get("name").and_then(|n| n.as_str()) == Some(name));
    let Some(asset) = exact else {
        return Ok(None);
    };
    if let Some(actual) = asset_category(&asset) {
        if !actual.eq_ignore_ascii_case("mcp") {
            return Err(category_conflict_error(name, actual, "mcp"));
        }
    }
    mcp_asset_ref_from_value(&asset, name)
        .map(Some)
        .ok_or_else(|| format!("asset `{name}` matched but had no id"))
}

async fn lookup_mcp_asset(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    name: &str,
) -> Result<Option<McpAssetRef>, String> {
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", name),
            ("category", "mcp"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    find_mcp_asset(&found, name)
}

async fn ensure_mcp_asset(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    name: &str,
    description: &str,
) -> Result<McpAssetRef, String> {
    if let Some(asset) = lookup_mcp_asset(client, origin, token, name).await? {
        return Ok(asset);
    }
    let resp = client
        .post(format!("{}/api/v1/assets", origin.trim_end_matches('/')))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "ownerType": "user",
            "category": "mcp",
            "visibility": "private",
            "description": description,
            "metadata": {
                "runtimeBindingKind": "mcp",
                "service": "Function as a Service",
                "agentKind": "tool",
                "runtimeKind": "a3s-function-service",
                "protocol": "mcp",
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
            "create MCP asset failed ({status}): {}",
            response_message(&body)
        ));
    }
    mcp_asset_ref_from_value(envelope_data(&body), name)
        .ok_or_else(|| "create MCP asset: no id in response".to_string())
}

#[allow(clippy::too_many_arguments)]
async fn upload_mcp_project(
    origin: &str,
    token: &str,
    asset_id: &str,
    source_files: &[McpSourceFile],
    asset_acl: &str,
    _manifest: &serde_json::Value,
    _server_config: &serde_json::Value,
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
            "message": "a3s code /mcp: update MCP server asset",
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
        "upload MCP asset failed ({status}): {}",
        truncate(&body, 200)
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum McpRuntimeBindingSync {
    Synced,
    Unsupported,
    Failed(String),
}

async fn sync_mcp_runtime_binding(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> McpRuntimeBindingSync {
    match sync_mcp_runtime_binding_inner(origin, token, asset_id, runtime_binding).await {
        Ok(true) => McpRuntimeBindingSync::Synced,
        Ok(false) => McpRuntimeBindingSync::Unsupported,
        Err(err) => McpRuntimeBindingSync::Failed(err),
    }
}

async fn sync_mcp_runtime_binding_inner(
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
        .json(&mcp_runtime_binding_upsert_body(runtime_binding))
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

fn mcp_runtime_binding_upsert_body(runtime_binding: &serde_json::Value) -> serde_json::Value {
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
            .unwrap_or("mcp"),
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

async fn inspect_mcp_asset(
    origin: &str,
    token: &str,
    action: McpOsAction,
    asset_name: &str,
    mcp_name: &str,
) -> Result<McpOsResult, String> {
    let client = http()?;
    let Some(asset) = lookup_mcp_asset(&client, origin, token, asset_name).await? else {
        return Ok(McpOsResult {
            action,
            asset_name: asset_name.to_string(),
            asset_id: "not-published".to_string(),
            view: mcp_view_spec(mcp_asset_search_url(origin, asset_name)),
            note: format!(
                "OS status for `{mcp_name}`: no Function as a Service MCP asset named `{asset_name}` was found. Run `/mcp publish` first."
            ),
            open_view: false,
        });
    };
    let runtime_binding_status =
        runtime_binding_validation_status(&client, origin, token, &asset.id).await;
    if matches!(action, McpOsAction::Open | McpOsAction::Logs) {
        let fallback = match action {
            McpOsAction::Open => mcp_view_spec(mcp_asset_url(origin, &asset.id)),
            McpOsAction::Logs => mcp_view_spec(mcp_logs_url(origin, &asset.id)),
            _ => unreachable!(),
        };
        let (view, note) =
            try_mcp_progressive_observe(&client, origin, token, action, &asset, mcp_name)
                .await
                .unwrap_or_else(|| {
                    let surface = match action {
                        McpOsAction::Open => "Function as a Service MCP asset view",
                        McpOsAction::Logs => "Function as a Service MCP logs view",
                        _ => unreachable!(),
                    };
                    (
                        fallback,
                        format!("Opened OS MCP asset through the {surface}."),
                    )
                });
        return Ok(McpOsResult {
            action,
            asset_name: asset.name,
            asset_id: asset.id,
            view,
            note,
            open_view: true,
        });
    }
    Ok(McpOsResult {
        action,
        asset_name: asset.name,
        asset_id: asset.id.clone(),
        view: mcp_view_spec(mcp_asset_url(origin, &asset.id)),
        note: format!("OS status for `{mcp_name}`: asset exists; {runtime_binding_status}."),
        open_view: false,
    })
}

fn mcp_function_input(
    mode: &str,
    asset: &McpAssetRef,
    server_config: &serde_json::Value,
    tool_name: Option<&str>,
) -> serde_json::Value {
    serde_json::json!({
        "assetId": asset.id,
        "assetName": asset.name,
        "mode": mode,
        "protocol": "mcp",
        "tool": tool_name,
        "arguments": {},
        "source": "a3s-code-tui",
        "serverConfig": server_config,
    })
}

fn mcp_function_ref(asset: &McpAssetRef) -> &str {
    &asset.id
}

fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
}

fn mcp_progressive_params(
    asset: &McpAssetRef,
    server_config: &serde_json::Value,
    input: serde_json::Value,
    inputs: Option<Vec<serde_json::Value>>,
) -> serde_json::Value {
    let config = serde_json::json!({
        "assetId": asset.id,
        "assetName": asset.name,
        "category": "mcp",
        "protocol": "mcp",
    });
    let mut params = serde_json::json!({
        "ref": mcp_function_ref(asset),
        "functionRef": mcp_function_ref(asset),
        "worker": mcp_function_ref(asset),
        "name": mcp_function_ref(asset),
        "assetId": asset.id,
        "assetName": asset.name,
        "input": input,
        "config": config,
        "timeoutMs": 120000,
        "idempotencyKey": format!("a3s-code-mcp-progressive-{}", unix_timestamp_secs()),
        "serverConfig": server_config,
    });
    if let Some(inputs) = inputs {
        params["inputs"] = serde_json::Value::Array(inputs);
    }
    params
}

fn mcp_observe_progressive_params(
    asset: &McpAssetRef,
    mcp_name: &str,
    action: McpOsAction,
) -> serde_json::Value {
    serde_json::json!({
        "assetId": asset.id,
        "assetName": asset.name,
        "functionRef": asset.name,
        "ref": asset.name,
        "worker": asset.name,
        "name": asset.name,
        "mcpName": mcp_name,
        "category": "mcp",
        "operation": action.label(),
        "input": {
            "assetId": asset.id,
            "assetName": asset.name,
            "mcpName": mcp_name,
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "payload": {
            "assetId": asset.id,
            "assetName": asset.name,
            "mcpName": mcp_name,
            "operation": action.label(),
            "source": "a3s-code-tui",
        },
        "timeoutMs": 120000,
        "idempotencyKey": format!("a3s-code-mcp-{}-{}", action.label(), unix_timestamp_secs()),
    })
}

fn mcp_progressive_score(action: McpOsAction, text: &str, operation: &str) -> i32 {
    let combined = format!("{text} {operation}").to_ascii_lowercase();
    let operation_lower = operation.to_ascii_lowercase();
    if matches!(action, McpOsAction::Open | McpOsAction::Logs)
        && (operation_lower.contains("createasset")
            || operation_lower.contains("listasset")
            || operation_lower.contains("getasset")
            || operation_lower.contains("deployability")
            || operation_lower.contains("diagnose")
            || operation_lower.contains("acknowledge")
            || operation_lower.contains("marketplace")
            || operation_lower.contains("scaffold")
            || operation_lower.contains("template")
            || operation_lower.contains("preview"))
    {
        return 0;
    }
    if matches!(action, McpOsAction::Open | McpOsAction::Logs)
        && is_mutating_mcp_observe_operation(&combined, operation)
    {
        return 0;
    }
    if matches!(action, McpOsAction::Open | McpOsAction::Logs) && !combined.contains("mcp") {
        return 0;
    }
    let mut score = 0;
    if combined.contains("function") || combined.contains("faas") {
        score += 8;
    }
    if combined.contains("mcp") {
        score += 8;
    }
    if matches!(action, McpOsAction::Run | McpOsAction::Test) && !combined.contains("mcp") {
        return 0;
    }
    if combined.contains("tool") {
        score += 3;
    }
    let mut action_hit = false;
    match action {
        McpOsAction::Run => {
            if operation_lower.contains("run")
                || combined.contains("run mcp")
                || combined.contains("mcp run")
            {
                score += 12;
                action_hit = true;
            }
            if combined.contains("batch") {
                score -= 5;
            }
        }
        McpOsAction::Test => {
            if combined.contains("batch") {
                score += 12;
                action_hit = true;
            }
        }
        McpOsAction::Open => {
            if (operation_lower.contains("open") && !operation_lower.contains("reopen"))
                || (operation_lower.contains("view") && !operation_lower.contains("preview"))
                || combined.contains("remoteui")
                || combined.contains("mcp asset view")
            {
                score += 10;
                action_hit = true;
            }
            if combined.contains("logs") || combined.contains("log view") {
                score -= 4;
            }
        }
        McpOsAction::Logs
            if operation_lower.contains("log")
                || operation_lower.contains("trace")
                || operation_lower.contains("inspect")
                || operation_lower.contains("observability") =>
        {
            score += 10;
            action_hit = true;
        }
        _ => {}
    }
    if combined.contains("agent as a service") || combined.contains("workflow") {
        score -= 10;
    }
    if matches!(
        action,
        McpOsAction::Run | McpOsAction::Test | McpOsAction::Open | McpOsAction::Logs
    ) && !action_hit
    {
        0
    } else if score >= 8 {
        score
    } else {
        0
    }
}

fn is_mutating_mcp_observe_operation(_combined: &str, operation: &str) -> bool {
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
        || operation.contains("reopen")
        || operation.contains("close")
        || operation.contains("resolve")
        || operation.contains("run")
        || operation.contains("trigger")
        || operation.contains("batch")
        || operation.contains("validate")
        || operation.contains("acknowledge");
    has_mutating_hint && !has_safe_observe_hint
}

async fn try_mcp_progressive_observe(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    action: McpOsAction,
    asset: &McpAssetRef,
    mcp_name: &str,
) -> Option<(remote_ui::ViewSpec, String)> {
    let query = match action {
        McpOsAction::Open => "Function as a Service open MCP asset shaped ViewLink",
        McpOsAction::Logs => "Function as a Service MCP runtime logs shaped ViewLink",
        _ => return None,
    };
    let execution = os_progressive::execute_first_matching(
        client,
        origin,
        token,
        query,
        mcp_observe_progressive_params(asset, mcp_name, action),
        |text, operation| mcp_progressive_score(action, text, operation),
    )
    .await?;
    let fallback = match action {
        McpOsAction::Open => mcp_view_spec(mcp_asset_url(origin, &asset.id)),
        McpOsAction::Logs => mcp_view_spec(mcp_logs_url(origin, &asset.id)),
        _ => unreachable!(),
    };
    Some((
        execution.view.unwrap_or(fallback),
        format!(
            "OS Function as a Service accepted MCP `{}` through progressive capabilities (`{}`).",
            action.label(),
            execution.operation.operation
        ),
    ))
}

#[allow(clippy::too_many_arguments)]
async fn try_mcp_progressive_function(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    action: McpOsAction,
    asset: &McpAssetRef,
    server_config: &serde_json::Value,
    input: serde_json::Value,
    inputs: Option<Vec<serde_json::Value>>,
) -> Option<(remote_ui::ViewSpec, String)> {
    let query = match action {
        McpOsAction::Run => "Function as a Service run MCP tool asset shaped view",
        McpOsAction::Test => "Function as a Service batch MCP tools shaped view",
        _ => return None,
    };
    let params = mcp_progressive_params(asset, server_config, input, inputs);
    let execution = os_progressive::execute_first_matching(
        client,
        origin,
        token,
        query,
        params,
        |text, operation| mcp_progressive_score(action, text, operation),
    )
    .await?;
    let view = execution
        .view
        .unwrap_or_else(|| mcp_view_spec(mcp_function_view_url(origin, &asset.id)));
    let note = match action {
        McpOsAction::Run => format!(
            "OS Function as a Service accepted the MCP run through progressive capabilities (`{}`).",
            execution.operation.operation
        ),
        McpOsAction::Test => format!(
            "OS Function as a Service accepted the MCP batch test through progressive capabilities (`{}`).",
            execution.operation.operation
        ),
        _ => unreachable!(),
    };
    Some((view, note))
}

async fn run_mcp_function(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset: &McpAssetRef,
    server_config: &serde_json::Value,
) -> Result<(remote_ui::ViewSpec, String), String> {
    let tool = mcp_tool_names(server_config).into_iter().next();
    let input = mcp_function_input("run", asset, server_config, tool.as_deref());
    if let Some(result) = try_mcp_progressive_function(
        client,
        origin,
        token,
        McpOsAction::Run,
        asset,
        server_config,
        input.clone(),
        None,
    )
    .await
    {
        return Ok(result);
    }
    Err(format!(
        "Published `{}` but OS did not expose a runnable MCP capability yet. Use `mcp deploy`, `mcp open`, or `mcp status` until the OS MCP runner API is available.",
        asset.name
    ))
}

async fn batch_test_mcp_function(
    client: &reqwest::Client,
    origin: &str,
    token: &str,
    asset: &McpAssetRef,
    server_config: &serde_json::Value,
) -> Result<(remote_ui::ViewSpec, String), String> {
    let mut tools = mcp_tool_names(server_config);
    if tools.is_empty() {
        tools.push("list-tools".to_string());
    }
    let inputs = tools
        .iter()
        .take(3)
        .map(|tool| mcp_function_input("test", asset, server_config, Some(tool)))
        .collect::<Vec<_>>();
    let primary_input = inputs
        .first()
        .cloned()
        .unwrap_or_else(|| mcp_function_input("test", asset, server_config, None));
    if let Some(result) = try_mcp_progressive_function(
        client,
        origin,
        token,
        McpOsAction::Test,
        asset,
        server_config,
        primary_input,
        Some(inputs.clone()),
    )
    .await
    {
        return Ok(result);
    }
    Err(format!(
        "Published `{}` but OS did not expose an MCP test capability yet. Use `mcp deploy`, `mcp open`, or `mcp status` until the OS MCP runner API is available.",
        asset.name
    ))
}

fn append_mcp_runtime_binding_sync_note(
    mut note: String,
    runtime_binding_synced: &McpRuntimeBindingSync,
) -> String {
    match runtime_binding_synced {
        McpRuntimeBindingSync::Synced => {
            note.push_str(" OS runtime binding was synced.");
        }
        McpRuntimeBindingSync::Unsupported => {
            note.push_str(
                " OS runtime-binding endpoint was unavailable; runtime-binding intent was saved.",
            );
        }
        McpRuntimeBindingSync::Failed(err) => {
            note.push_str(&format!(
                " OS runtime binding could not be synced: {}; runtime-binding intent was saved.",
                truncate(err, 160)
            ));
        }
    }
    note
}

pub(crate) async fn publish_mcp_to_os(
    session: crate::a3s_os::StoredOsSession,
    dev: McpDevSession,
    action: McpOsAction,
) -> Result<McpOsResult, String> {
    let origin = crate::a3s_os::os_origin(&session.address);
    let raw_config = mcp_metadata_value(&dev);
    let server_config = mcp_server_config_json(&dev, &raw_config);
    let mcp_name = mcp_config_name(&server_config, &dev);
    let description = mcp_config_description(&server_config, &dev);
    let asset_name = mcp_asset_name(&mcp_name);

    if matches!(
        action,
        McpOsAction::Status | McpOsAction::Open | McpOsAction::Logs
    ) {
        return inspect_mcp_asset(
            &origin,
            &session.access_token,
            action,
            &asset_name,
            &mcp_name,
        )
        .await;
    }

    let client = http()?;
    let asset = ensure_mcp_asset(
        &client,
        &origin,
        &session.access_token,
        &asset_name,
        &description,
    )
    .await?;
    let source_files = collect_mcp_source_files(&dev.path)?;
    let manifest = mcp_manifest_json(&dev, &asset_name, &server_config);
    let runtime_binding = mcp_runtime_binding_json(&dev, &asset_name, &server_config);
    let asset_acl = mcp_asset_acl(&dev, &asset_name, &server_config);
    asset_lifecycle::write_asset_acl(&dev.path, &asset_acl)?;
    upload_mcp_project(
        &origin,
        &session.access_token,
        &asset.id,
        &source_files,
        &asset_acl,
        &manifest,
        &server_config,
        &runtime_binding,
    )
    .await?;
    let runtime_binding_synced =
        sync_mcp_runtime_binding(&origin, &session.access_token, &asset.id, &runtime_binding).await;

    let (view, note) = match action {
        McpOsAction::Publish => (
            mcp_view_spec(mcp_asset_url(&origin, &asset.id)),
            format!(
                "Published `{mcp_name}` as an OS MCP asset backed by Function as a Service."
            ),
        ),
        McpOsAction::Deploy => (
            mcp_view_spec(mcp_asset_url(&origin, &asset.id)),
            format!(
                "Deployed `{mcp_name}` by publishing its serving MCP runtime binding for Function as a Service."
            ),
        ),
        McpOsAction::Run => run_mcp_function(
            &client,
            &origin,
            &session.access_token,
            &asset,
            &server_config,
        )
        .await?,
        McpOsAction::Test => batch_test_mcp_function(
            &client,
            &origin,
            &session.access_token,
            &asset,
            &server_config,
        )
        .await?,
        McpOsAction::Open | McpOsAction::Logs | McpOsAction::Status => {
            unreachable!("read-only MCP actions return before publish flow")
        }
    };
    let note = append_mcp_runtime_binding_sync_note(note, &runtime_binding_synced);
    Ok(McpOsResult {
        action,
        asset_name,
        asset_id: asset.id,
        view,
        note,
        open_view: true,
    })
}

fn mcp_picker_header(total: usize, root: &std::path::Path, width: usize) -> String {
    truncate(
        &format!(
            "  ◆ mcp — select a server asset ({total} in {})",
            root.to_string_lossy()
        ),
        width,
    )
}

fn mcp_picker_hint(width: usize) -> String {
    truncate("  ↑/↓ select · Enter local MCP dev · Esc cancel", width)
}

fn mcp_picker_lines(
    projects: &[McpProject],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) = mcp_picker_panel(projects, selected, root, width, height)
    else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn mcp_picker_panel(
    projects: &[McpProject],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Option<(MenuPanel, usize)> {
    let total = projects.len();
    if total == 0 {
        return None;
    }
    let max_items = height.saturating_sub(8).clamp(3, 12);
    let selected = selected.min(total.saturating_sub(1));
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let items = projects
        .iter()
        .map(|project| MenuItem::new(project.rel.clone()).description(project.description.clone()))
        .collect::<Vec<_>>();

    let panel = MenuPanel::new(mcp_picker_header(total, root, width).trim_start())
        .subtitle(mcp_picker_hint(width).trim_start())
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

fn mcp_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

/// Directive for `/mcp <description>`: create a local MCP server asset.
#[cfg(test)]
pub(crate) fn mcp_gen_prompt(description: &str, dir: &str) -> String {
    format!(
        "Create a local MCP server asset from the description below and save it under \
         {dir}. This is a local authoring task: do not open OS, RemoteUI, or a browser.\n\
         Description: {description}\n\
         IMPORTANT: {dir} is OUTSIDE this session's workspace, so path-scoped file tools \
         will reject writes there. Use the `bash` tool for ALL file creation and edits under \
         {dir}; do not use path-scoped write/edit tools for this task. Use non-interactive bash \
         commands with full quoted paths. Never run a command that waits on stdin. Prefer a \
         single bash heredoc script that creates the directory and ALL required files; the first \
         bash command should leave a complete asset package on disk.\n\
         Create {dir}/<kebab-case-name>/ with a minimal runnable MCP server, README.md, \
         examples/, tests/, and .a3s/asset.acl. \
         The package-local `.a3s/` directory is metadata-only. Do NOT put `server.js`, \
         README, examples, tests, or other source files under `.a3s/`. \
         Do NOT create extra generated JSON config files; keep package configuration in \
         `.a3s/asset.acl`. Runtime configuration is synced through OS Function as a Service \
         APIs during publish/deploy, not stored in the asset repository.\n\
         Keep the first implementation tiny. Prefer a small stdio server stub over an elaborate framework implementation. Prefer stdio for local \
         development. Add a small smoke script file, but do NOT start a \
         long-running MCP stdio server during generation. For validation, use static or one-shot \
         checks only, such as `python3 -m py_compile server.py` or \
         `node --check server.js`; do not start the server process. After validation succeeds, \
         stop using tools immediately and give a concise final answer with the saved asset path \
         and the note that `/mcp` starts local MCP dev."
    )
}

pub(crate) fn mcp_dev_prompt(session: &McpDevSession, request: &str) -> String {
    format!(
        "You are in A3S Code local MCP-development mode.\n\
         Current MCP asset: {name}\n\
         Description: {description}\n\
         Asset path: {path}\n\
         MCP root: {root}\n\n\
         User request:\n{request}\n\n\
         Work on this local MCP server asset iteratively. Read the current asset files \
         before editing. Keep the server runnable locally, preserve valid tool schemas, and \
         keep metadata ready for OS Function as a Service: category=mcp, runtime binding \
         kind=mcp, serving isolation by default, and tool calls that can map to Function \
         run/batch with agentKind=tool. Do not open OS, WebIDE, RemoteUI, or browser pages \
         for this local MCP-dev turn. Validate changed JSON and run a lightweight local smoke \
         check when practical. End with a concise summary and the next best MCP Function as a Service step.\n\n\
         The TUI remains in MCP-development mode for `{name}` after this turn; the user can \
         press Esc or run `/mcp off` to return to normal mode.",
        name = session.name.as_str(),
        description = session.description.as_str(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

pub(crate) fn mcp_review_prompt(session: &McpDevSession) -> String {
    let contract = super::review::review_report_contract(&session.path);
    format!(
        "Review this local MCP server asset without changing files unless the user explicitly asks \
         for fixes.\n\
         MCP name: {name}\n\
         Description: {description}\n\
         Project path: {path}\n\
         MCP root: {root}\n\n\
         Read the project metadata and key implementation files. Report concise findings on: MCP \
         protocol correctness, tool schemas, entrypoint/run command, local smoke tests, secret \
         handling, packaging, .a3s metadata, and readiness for Function as a Service. Mention the \
         smallest recommended improvements and whether `/mcp run` or `/mcp deploy` is the right \
         next lifecycle step.{contract}",
        name = session.name.as_str(),
        description = session.description.as_str(),
        path = session.path.display(),
        root = session.root.display(),
    )
}

impl App {
    pub(crate) fn on_mcp_os_completed(
        &mut self,
        status_entry: TranscriptEntryId,
        res: Result<McpOsResult, String>,
    ) {
        match res {
            Ok(result) => {
                self.last_view = Some(result.view.clone());
                self.replace_tracked_line(
                    status_entry,
                    &gutter(
                        TN_CYAN,
                        &format!(
                            "◆ /mcp {} · `{}` ({})",
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
                        &remote_view_button("MCP Function as a Service · click to reopen"),
                    ));
                    self.open_remote_view(&result.view);
                } else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  Open view opens the related OS MCP asset view"),
                    );
                }
            }
            Err(e) => {
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  /mcp OS operation failed: {e}")),
                );
            }
        }
    }

    pub(crate) fn exit_mcp_dev(&mut self) {
        match self.mcp_dev.take() {
            Some(session) => self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  mcp dev off — {} ({})",
                session.name, session.rel
            ))),
            None => self.push_line(&Style::new().fg(TN_GRAY).render("  mcp dev is not active")),
        }
        self.relayout();
    }

    /// Open the `/mcp` picker.
    pub(crate) fn open_mcp_panel(&mut self) {
        let root = self.asset_directories.mcp.clone();
        let projects = list_mcp_projects(&root);
        if projects.is_empty() {
            self.pending_mcp_subcommand = None;
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  no MCP assets in {} — draft one with `/mcp <description>` first",
                root.display()
            )));
            return;
        }
        self.mcp_picker = Some(McpPanel {
            root,
            projects,
            sel: 0,
        });
    }

    /// Keys while the `/mcp` picker is open — consumes everything.
    pub(crate) fn handle_mcp_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let p = self.mcp_picker.as_mut()?;
        let last = p.projects.len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => p.sel = p.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => p.sel = (p.sel + 1).min(last),
            KeyCode::Esc => {
                cancel_pending_picker(&mut self.mcp_picker, &mut self.pending_mcp_subcommand)
            }
            KeyCode::Enter => {
                let panel = self.mcp_picker.take()?;
                let selected = panel.sel.min(last);
                return self.activate_mcp_panel_selection(panel, selected);
            }
            _ => {}
        }
        None
    }

    pub(crate) fn handle_mcp_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let panel_state = self.mcp_picker.as_ref()?;
        let total = panel_state.projects.len();
        if total == 0 {
            return None;
        }
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let selected = panel_state.sel.min(total - 1);
        let (mut panel, panel_height) = mcp_picker_panel(
            &panel_state.projects,
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
            mcp_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                let panel_state = self.mcp_picker.take()?;
                self.activate_mcp_panel_selection(panel_state, index.min(total - 1))
            }
            Some(MenuPanelMsg::Cancelled) => {
                cancel_pending_picker(&mut self.mcp_picker, &mut self.pending_mcp_subcommand);
                None
            }
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    if let Some(open) = self.mcp_picker.as_mut() {
                        open.sel = after;
                    }
                }
                None
            }
        }
    }

    fn activate_mcp_panel_selection(
        &mut self,
        panel: McpPanel,
        selected: usize,
    ) -> Option<Cmd<Msg>> {
        let last = panel.projects.len().saturating_sub(1);
        let picked = panel.projects.get(selected.min(last))?.clone();
        self.agent_dev = None;
        self.skill_dev = None;
        self.okf_dev = None;
        self.mcp_dev = Some(McpDevSession {
            name: picked.name.clone(),
            description: picked.description.clone(),
            rel: picked.rel.clone(),
            path: picked.path.clone(),
            root: panel.root,
        });
        self.push_line(&gutter(
            TN_CYAN,
            &format!(
                "◆ mcp dev: {} ({}) · Esc or /mcp off returns to normal mode",
                picked.name, picked.rel
            ),
        ));
        self.relayout();
        if let Some(pending) = self.pending_mcp_subcommand.take() {
            return self.execute_mcp_subcommand(pending);
        }
        None
    }

    /// Overlay the `/mcp` picker above the input.
    pub(crate) fn overlay_mcp_menu(&self, composed: String) -> String {
        let Some(p) = self.mcp_picker.as_ref() else {
            return composed;
        };
        let width = self.width as usize;
        let menu = mcp_picker_lines(&p.projects, p.sel, &p.root, width, self.height as usize);
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
        std::env::temp_dir().join(format!("a3s-code-{name}-{}", std::process::id()))
    }

    #[test]
    fn existing_mcp_asset_must_match_mcp_category() {
        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "skill-asset",
                        "name": "mcp-weather-tools",
                        "category": "skill"
                    }
                ]
            }
        });

        let err = find_mcp_asset(&found, "mcp-weather-tools").unwrap_err();
        assert!(err.contains("category=skill"), "{err}");
        assert!(err.contains("expected mcp"), "{err}");
    }

    #[tokio::test]
    async fn ensure_mcp_asset_create_payload_carries_function_service_metadata() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_asset_create_mock(captured.clone()).await;
        let client = http().expect("client");

        let asset = ensure_mcp_asset(
            &client,
            &origin,
            "token",
            "mcp-weather-tools",
            "Weather tools",
        )
        .await
        .expect("asset should be created");

        assert_eq!(asset.id, "mcp-asset-1");
        let requests = captured.lock().unwrap().clone();
        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "mcp");
        assert_eq!(create_json["metadata"]["runtimeBindingKind"], "mcp");
        assert_eq!(create_json["metadata"]["service"], "Function as a Service");
        assert_eq!(create_json["metadata"]["agentKind"], "tool");
        assert_eq!(
            create_json["metadata"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(create_json["metadata"]["protocol"], "mcp");
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");
    }

    async fn spawn_mcp_asset_create_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
        spawn_mcp_os_mock(captured, mcp_asset_create_mock_response).await
    }

    async fn spawn_mcp_os_mock<F>(captured: Arc<Mutex<Vec<String>>>, responder: F) -> String
    where
        F: Fn(&str, &str) -> (&'static str, String) + Send + Sync + 'static,
    {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let responder = Arc::new(responder);
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let captured = captured.clone();
                let responder = responder.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 65536];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    captured.lock().unwrap().push(format!("{line}\n{body}"));
                    let (status, payload) = responder(&line, &body);
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

    fn has_request(requests: &[String], prefix: &str) -> bool {
        requests.iter().any(|request| request.starts_with(prefix))
    }

    fn request_count(requests: &[String], prefix: &str) -> usize {
        requests
            .iter()
            .filter(|request| request.starts_with(prefix))
            .count()
    }

    fn assert_no_mcp_asset_mutation(requests: &[String]) {
        assert!(
            !has_request(requests, "POST /api/v1/assets HTTP/1.1"),
            "read-only MCP action must not create assets:\n{}",
            requests.join("\n")
        );
        assert!(
            !requests.iter().any(
                |request| request.starts_with("POST ") && request.contains("/repository/files")
            ),
            "read-only MCP action must not upload repository files:\n{}",
            requests.join("\n")
        );
        assert!(
            !requests
                .iter()
                .any(|request| request.starts_with("PUT ") && request.contains("/runtime-binding")),
            "read-only MCP action must not write runtime binding:\n{}",
            requests.join("\n")
        );
    }

    fn mcp_asset_create_mock_response(line: &str, body: &str) -> (&'static str, String) {
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#.to_string());
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""category":"mcp""#)
                && body.contains(r#""runtimeBindingKind":"mcp""#)
                && body.contains(r#""service":"Function as a Service""#)
                && body.contains(r#""agentKind":"tool""#)
                && body.contains(r#""runtimeKind":"a3s-function-service""#)
                && body.contains(r#""protocol":"mcp""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"mcp-asset-1","name":"mcp-weather-tools"}}"#.to_string(),
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad MCP asset body"}"#.to_string(),
            );
        }
        ("404 Not Found", r#"{"message":"not found"}"#.to_string())
    }

    fn mcp_existing_asset_mock_response(line: &str, _body: &str) -> (&'static str, String) {
        if line.starts_with("GET /api/v1/assets?") {
            return (
                "200 OK",
                r#"{"data":{"items":[{"id":"mcp-asset-1","name":"mcp-weather-tools","category":"mcp"}]}}"#.to_string(),
            );
        }
        if line.starts_with("GET /api/v1/assets/mcp-asset-1/runtime-binding ") {
            return (
                "200 OK",
                r#"{"data":{"configured":true,"kind":"mcp"}}"#.to_string(),
            );
        }
        if line.starts_with("POST /api/v1/assets/mcp-asset-1/runtime-binding/validate ") {
            return ("200 OK", r#"{"data":{"valid":true}}"#.to_string());
        }
        if line.starts_with("POST /api/v1/kernel/capabilities ") {
            return (
                "404 Not Found",
                r#"{"code":404,"message":"capabilities unavailable in mock"}"#.to_string(),
            );
        }
        ("404 Not Found", r#"{"message":"not found"}"#.to_string())
    }

    fn mcp_publish_function_mock_response(line: &str, body: &str) -> (&'static str, String) {
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#.to_string());
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            return mcp_asset_create_mock_response(line, body);
        }
        if line.starts_with("POST /api/v1/assets/mcp-asset-1/repository/files ") {
            return ("200 OK", r#"{"data":{"ok":true}}"#.to_string());
        }
        if line.starts_with("PUT /api/v1/assets/mcp-asset-1/runtime-binding ") {
            return ("200 OK", r#"{"data":{"configured":true}}"#.to_string());
        }
        if line.starts_with("POST /api/v1/assets/mcp-asset-1/runtime-binding/validate ") {
            return ("200 OK", r#"{"data":{"valid":true}}"#.to_string());
        }
        if line.starts_with("POST /api/v1/kernel/capabilities ") {
            return (
                "404 Not Found",
                r#"{"code":404,"message":"capabilities unavailable in mock"}"#.to_string(),
            );
        }
        ("404 Not Found", r#"{"message":"not found"}"#.to_string())
    }

    fn mcp_publish_progressive_mock_response(line: &str, body: &str) -> (&'static str, String) {
        if !line.starts_with("POST /api/v1/kernel/capabilities ") {
            return mcp_publish_function_mock_response(line, body);
        }
        if body.contains(r#""action":"search""#) {
            return (
                "200 OK",
                r#"{"code":200,"data":{"results":[{"name":"runMcpAsset","module":"mcp/runtime","operation":"runMcpAsset","resource":"mcp.runtime","method":"POST","description":"Function as a Service MCP run shaped view"}]}}"#
                    .to_string(),
            );
        }
        if body.contains(r#""action":"describe""#) && body.contains(r#""operation":"runMcpAsset""#)
        {
            return (
                "200 OK",
                r#"{"code":200,"data":{"operation":{"name":"runMcpAsset","inputSchema":{"properties":{"assetId":{"type":"string"},"input":{"type":"object"},"config":{"type":"object"}}}},"view":{"url":"/admin/mcp/runs/mcp-asset-1?embed=1","width":1280,"height":860}}}"#
                    .to_string(),
            );
        }
        if body.contains(r#""action":"execute""#) && body.contains(r#""operation":"runMcpAsset""#) {
            return (
                "200 OK",
                r#"{"code":200,"data":{"runId":"mcp-run-1","viewUrl":"/admin/mcp/runs/mcp-asset-1?embed=1"}}"#
                    .to_string(),
            );
        }
        ("404 Not Found", r#"{"message":"not found"}"#.to_string())
    }

    fn test_os_session(origin: String) -> crate::a3s_os::StoredOsSession {
        crate::a3s_os::StoredOsSession {
            address: origin,
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: Some("test-os".into()),
            login_at_ms: 1,
        }
    }

    fn write_mcp_fixture(name: &str) -> (std::path::PathBuf, McpDevSession) {
        let root = temp_root(name);
        let _ = std::fs::remove_dir_all(&root);
        let project = root.join("weather-tools");
        std::fs::create_dir_all(project.join(".a3s")).unwrap();
        std::fs::write(
            project.join("README.md"),
            "# weather-tools\n\nWeather MCP tools\n",
        )
        .unwrap();
        std::fs::write(
            project.join(asset_lifecycle::ASSET_ACL_PATH),
            "version = \"a3s.asset.v1\"",
        )
        .unwrap();
        std::fs::write(project.join("server.js"), "console.log('weather tools');\n").unwrap();
        let dev = McpDevSession {
            name: "weather-tools".into(),
            description: "Weather MCP tools".into(),
            rel: "weather-tools".into(),
            path: project,
            root: root.clone(),
        };
        (root, dev)
    }

    #[tokio::test]
    async fn mcp_status_checks_existing_asset_without_mutating_it() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_os_mock(captured.clone(), mcp_existing_asset_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-status-os");

        let result = publish_mcp_to_os(test_os_session(origin), dev, McpOsAction::Status)
            .await
            .expect("status should inspect existing MCP asset");
        let requests = captured.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(result.action, McpOsAction::Status);
        assert_eq!(result.asset_name, "mcp-weather-tools");
        assert_eq!(result.asset_id, "mcp-asset-1");
        assert!(!result.open_view);
        assert!(result.note.contains("asset exists"), "{}", result.note);
        assert!(
            result.note.contains("runtime-binding valid"),
            "{}",
            result.note
        );
        assert!(result.view.url.contains("/admin/assets/mcp-asset-1"));
        assert_eq!(request_count(&requests, "GET /api/v1/assets?"), 1);
        assert_no_mcp_asset_mutation(&requests);
    }

    #[tokio::test]
    async fn mcp_open_observes_existing_asset_without_mutating_it() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_os_mock(captured.clone(), mcp_existing_asset_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-open-os");

        let result = publish_mcp_to_os(test_os_session(origin), dev, McpOsAction::Open)
            .await
            .expect("open should inspect existing MCP asset");
        let requests = captured.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(result.action, McpOsAction::Open);
        assert!(result.open_view);
        assert!(result.view.url.contains("/admin/assets/mcp-asset-1"));
        assert!(result.view.url.contains("embed=1"));
        assert!(
            result.note.contains("Function as a Service MCP asset view"),
            "{}",
            result.note
        );
        assert_eq!(
            request_count(&requests, "POST /api/v1/kernel/capabilities "),
            1,
            "/mcp open should try the progressive ViewLink path before fallback"
        );
        assert_no_mcp_asset_mutation(&requests);
    }

    #[tokio::test]
    async fn mcp_logs_observes_existing_asset_without_mutating_it() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_os_mock(captured.clone(), mcp_existing_asset_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-logs-os");

        let result = publish_mcp_to_os(test_os_session(origin), dev, McpOsAction::Logs)
            .await
            .expect("logs should inspect existing MCP asset");
        let requests = captured.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(result.action, McpOsAction::Logs);
        assert!(result.open_view);
        assert!(result.view.url.contains("/admin/infrastructure/batch"));
        assert!(result.view.url.contains("asset=mcp-asset-1"));
        assert!(result.view.url.contains("logs=1"));
        assert!(
            result.note.contains("Function as a Service MCP logs view"),
            "{}",
            result.note
        );
        assert_eq!(
            request_count(&requests, "POST /api/v1/kernel/capabilities "),
            1,
            "/mcp logs should try the progressive ViewLink path before fallback"
        );
        assert_no_mcp_asset_mutation(&requests);
    }

    #[tokio::test]
    async fn publish_mcp_to_os_run_requires_mcp_runner_capability_without_runtime_function_fallback(
    ) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_os_mock(captured.clone(), mcp_publish_function_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-run-os");

        let err = publish_mcp_to_os(test_os_session(origin), dev, McpOsAction::Run)
            .await
            .expect_err("run should fail clearly when OS exposes no MCP runner");
        let requests = captured.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            err.contains("did not expose a runnable MCP capability"),
            "{err}"
        );

        assert!(has_request(&requests, "POST /api/v1/assets HTTP/1.1"));
        assert!(has_request(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/repository/files "
        ));
        assert!(has_request(
            &requests,
            "PUT /api/v1/assets/mcp-asset-1/runtime-binding "
        ));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/run "
        ));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/batch "
        ));

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/repository/files ",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let uploaded_paths = upload_json["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|file| file["path"].as_str())
            .collect::<Vec<_>>();
        assert!(uploaded_paths.contains(&"server.js"), "{uploaded_paths:?}");
        assert!(
            uploaded_paths.contains(&asset_lifecycle::ASSET_ACL_PATH),
            "{uploaded_paths:?}"
        );
        for forbidden in [
            "mcp.asset.json",
            "mcp.server.json",
            "mcp.runtime-binding.json",
            "runtime-binding.json",
            "package.json",
        ] {
            assert!(
                !uploaded_paths.contains(&forbidden),
                "repository upload should not include {forbidden}: {uploaded_paths:?}"
            );
        }
    }

    #[tokio::test]
    async fn publish_mcp_to_os_run_uses_progressive_mcp_runner_when_available() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin =
            spawn_mcp_os_mock(captured.clone(), mcp_publish_progressive_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-run-progressive-os");

        let result = publish_mcp_to_os(test_os_session(origin.clone()), dev, McpOsAction::Run)
            .await
            .expect("run should use the OS MCP runner capability");
        let requests = captured.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);

        assert_eq!(result.action, McpOsAction::Run);
        assert_eq!(result.asset_id, "mcp-asset-1");
        assert!(result.open_view);
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/mcp/runs/mcp-asset-1?embed=1")
        );
        assert!(
            result.note.contains("MCP run") && result.note.contains("runtime binding was synced"),
            "{}",
            result.note
        );

        assert!(has_request(&requests, "POST /api/v1/assets HTTP/1.1"));
        assert!(has_request(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/repository/files "
        ));
        assert!(has_request(
            &requests,
            "PUT /api/v1/assets/mcp-asset-1/runtime-binding "
        ));
        assert!(has_request(&requests, "POST /api/v1/kernel/capabilities "));
        let execute = request_body(&requests, "POST /api/v1/kernel/capabilities ");
        assert!(execute.contains(r#""action":"search""#), "{execute}");
        assert!(
            requests.iter().any(|request| {
                request.contains(r#""action":"execute""#)
                    && request.contains(r#""module":"mcp/runtime""#)
                    && request.contains(r#""operation":"runMcpAsset""#)
                    && request.contains(r#""assetId":"mcp-asset-1""#)
            }),
            "missing progressive MCP execute request:\n{}",
            requests.join("\n")
        );
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/run "
        ));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/batch "
        ));
    }

    #[tokio::test]
    async fn publish_mcp_to_os_test_requires_mcp_runner_capability_without_runtime_function_fallback(
    ) {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_os_mock(captured.clone(), mcp_publish_function_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-test-os");

        let err = publish_mcp_to_os(test_os_session(origin), dev, McpOsAction::Test)
            .await
            .expect_err("test should fail clearly when OS exposes no MCP test capability");
        let requests = captured.lock().unwrap().clone();
        let _ = std::fs::remove_dir_all(&root);

        assert!(
            err.contains("did not expose an MCP test capability"),
            "{err}"
        );
        assert!(has_request(&requests, "POST /api/v1/assets HTTP/1.1"));
        assert!(has_request(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/repository/files "
        ));
        assert!(has_request(
            &requests,
            "PUT /api/v1/assets/mcp-asset-1/runtime-binding "
        ));
        assert!(has_request(&requests, "POST /api/v1/kernel/capabilities "));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/run "
        ));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/batch "
        ));
    }

    #[tokio::test]
    async fn publish_mcp_to_os_deploy_syncs_serving_runtime_binding_without_invoking_function() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mcp_os_mock(captured.clone(), mcp_publish_function_mock_response).await;
        let (root, dev) = write_mcp_fixture("mcp-deploy-os");

        let result = publish_mcp_to_os(test_os_session(origin.clone()), dev, McpOsAction::Deploy)
            .await
            .expect("deploy should publish and sync MCP serving runtime binding");
        let requests = captured.lock().unwrap().clone();

        assert_eq!(result.action, McpOsAction::Deploy);
        assert_eq!(result.asset_id, "mcp-asset-1");
        assert!(result.open_view);
        assert_eq!(
            result.view.url,
            format!("{origin}/admin/assets/mcp-asset-1?embed=1")
        );
        assert!(
            result.note.contains("Deployed `weather-tools`")
                && result.note.contains("runtime binding was synced"),
            "{}",
            result.note
        );

        assert!(has_request(&requests, "POST /api/v1/assets HTTP/1.1"));
        assert!(has_request(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/repository/files "
        ));
        assert!(has_request(
            &requests,
            "PUT /api/v1/assets/mcp-asset-1/runtime-binding "
        ));
        assert!(has_request(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/runtime-binding/validate "
        ));
        assert!(!has_request(&requests, "POST /api/v1/kernel/capabilities "));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/run "
        ));
        assert!(!has_request(
            &requests,
            "POST /api/v1/runtime/functions/mcp-asset-1/batch "
        ));

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/mcp-asset-1/repository/files ",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let uploaded_paths = upload_json["files"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|file| file["path"].as_str())
            .collect::<Vec<_>>();
        assert!(uploaded_paths.contains(&"server.js"), "{uploaded_paths:?}");
        assert!(
            uploaded_paths.contains(&asset_lifecycle::ASSET_ACL_PATH),
            "{uploaded_paths:?}"
        );
        for forbidden in [
            "mcp.asset.json",
            "mcp.server.json",
            "mcp.runtime-binding.json",
            "runtime-binding.json",
            "package.json",
        ] {
            assert!(
                !uploaded_paths.contains(&forbidden),
                "repository upload should not include {forbidden}: {uploaded_paths:?}"
            );
        }

        let binding = request_body(&requests, "PUT /api/v1/assets/mcp-asset-1/runtime-binding ");
        let binding_json: serde_json::Value = serde_json::from_str(&binding).unwrap();
        assert_eq!(binding_json["kind"], "mcp");
        assert_eq!(binding_json["isolation"], "serving");
        assert_eq!(binding_json["target"]["kind"], "asset");
        assert_eq!(binding_json["target"]["ref"], "main");
        assert_eq!(binding_json["runtime"]["kind"], "a3s-function-service");
        assert_eq!(binding_json["runtime"]["sharedRuntime"], "node-20");
        assert_eq!(binding_json["enabled"], true);
        assert_eq!(binding_json["metadata"]["source"], "a3s-code-tui");
        assert_eq!(binding_json["metadata"]["assetCategory"], "mcp");
        assert_eq!(binding_json["metadata"]["assetName"], "mcp-weather-tools");
        assert_eq!(binding_json["metadata"]["mcpName"], "weather-tools");
        assert_eq!(
            binding_json["metadata"]["tools"],
            serde_json::json!([]),
            "MCP tools should not be inferred from removed JSON config files"
        );
        let local_asset_acl = root
            .join("weather-tools")
            .join(asset_lifecycle::ASSET_ACL_PATH);
        assert!(local_asset_acl.is_file(), "missing local asset.acl");
        let local_asset_acl_body = std::fs::read_to_string(&local_asset_acl).unwrap();
        assert!(local_asset_acl_body.contains("category = \"mcp\""));
        assert!(local_asset_acl_body.contains("entrypoint = \"server.js\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_mcp_projects_by_marker_and_metadata() {
        let root = temp_root("mcp-list");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("calc")).unwrap();
        std::fs::create_dir_all(root.join("nested/search")).unwrap();
        std::fs::create_dir_all(root.join(".hidden/skip")).unwrap();
        std::fs::write(
            root.join("calc/README.md"),
            "# calc-tools\n\nCalculator tools\n",
        )
        .unwrap();
        std::fs::write(root.join("calc/server.js"), "console.log('calc')").unwrap();
        std::fs::write(
            root.join("nested/search/README.md"),
            "# search-tools\n\nSearch tools\n",
        )
        .unwrap();
        std::fs::write(
            root.join("nested/search/server.js"),
            "console.log('search')",
        )
        .unwrap();
        std::fs::write(root.join(".hidden/skip/mcp.json"), "{}").unwrap();

        let projects = list_mcp_projects(&root);
        let rels = projects.iter().map(|p| p.rel.as_str()).collect::<Vec<_>>();
        assert_eq!(rels, vec!["calc", "nested/search"]);
        assert_eq!(projects[0].name, "calc-tools");
        assert_eq!(projects[0].description, "Calculator tools");

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mcp_picker_lines_use_bounded_shared_menu_rows() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/mcps/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let projects = vec![
            McpProject {
                rel: "nested/very-long-mcp-server-name-that-would-overflow-the-panel".into(),
                path: root.join("nested/very-long-mcp-server-name-that-would-overflow-the-panel"),
                name: "long-mcp".into(),
                description: "A long MCP server description that should be trimmed cleanly".into(),
            },
            McpProject {
                rel: "weather-tools".into(),
                path: root.join("weather-tools"),
                name: "weather-tools".into(),
                description: "Weather tools".into(),
            },
        ];
        let lines = mcp_picker_lines(&projects, 0, &root, 40, 20);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("mcp"), "{plain}");
        assert!(plain.contains("select a server asset"), "{plain}");
        assert!(plain.contains("very-long-mcp-server"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn mcp_picker_lines_scroll_selected_project_into_view() {
        let root = std::path::PathBuf::from("/tmp/mcps");
        let projects = (0..16)
            .map(|index| McpProject {
                rel: format!("mcp-{index}"),
                path: root.join(format!("mcp-{index}")),
                name: format!("mcp-{index}"),
                description: format!("MCP server {index}"),
            })
            .collect::<Vec<_>>();
        let plain = mcp_picker_lines(&projects, 14, &root, 48, 16)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("mcp-14"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn mcp_picker_header_and_hint_fit_fixed_width() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/mcps/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let header = mcp_picker_header(9, &root, 40);
        let hint = mcp_picker_hint(40);
        assert!(a3s_tui::style::visible_len(&header) <= 40, "{header}");
        assert!(a3s_tui::style::visible_len(&hint) <= 40, "{hint}");
    }

    #[test]
    fn mcp_picker_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let root = std::path::PathBuf::from("/tmp/mcps");
        let projects = (0..4)
            .map(|index| McpProject {
                rel: format!("mcp-{index}"),
                path: root.join(format!("mcp-{index}")),
                name: format!("mcp-{index}"),
                description: format!("MCP server {index}"),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = mcp_picker_lines(&projects, 0, &root, width, height).len();
        let y_offset = mcp_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = mcp_picker_panel(&projects, 0, &root, width, height).expect("panel");
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
    fn mcp_picker_click_selects_visible_row_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let root = std::path::PathBuf::from("/tmp/mcps");
        let projects = (0..4)
            .map(|index| McpProject {
                rel: format!("mcp-{index}"),
                path: root.join(format!("mcp-{index}")),
                name: format!("mcp-{index}"),
                description: format!("MCP server {index}"),
            })
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = mcp_picker_lines(&projects, 0, &root, width, height).len();
        let y_offset = mcp_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = mcp_picker_panel(&projects, 0, &root, width, height).expect("panel");
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
    fn mcp_overlay_offset_moves_up_with_more_rows_below() {
        assert!(mcp_overlay_y_offset(24, 8, 7) < mcp_overlay_y_offset(24, 8, 5));
    }

    #[test]
    fn parses_mcp_exit_subcommand_only() {
        assert_eq!(
            parse_mcp_subcommand("off").unwrap().unwrap(),
            McpSubcommand::Exit
        );
        assert!(parse_mcp_subcommand("off now").unwrap().is_err());
        assert!(parse_mcp_subcommand("weather server").is_none());
    }

    #[test]
    fn parses_mcp_lifecycle_subcommands() {
        let cases = [
            ("publish", McpSubcommand::Publish, McpOsAction::Publish),
            ("run", McpSubcommand::Run, McpOsAction::Run),
            ("deploy", McpSubcommand::Deploy, McpOsAction::Deploy),
            ("test", McpSubcommand::Test, McpOsAction::Test),
            ("open", McpSubcommand::Open, McpOsAction::Open),
            ("logs", McpSubcommand::Logs, McpOsAction::Logs),
            ("status", McpSubcommand::Status, McpOsAction::Status),
        ];
        for (input, expected, action) in cases {
            let parsed = parse_mcp_subcommand(input).unwrap().unwrap();
            assert_eq!(parsed, expected, "{input}");
            assert_eq!(parsed.os_action(), Some(action), "{input}");
        }
        assert_eq!(
            parse_mcp_subcommand("activity failed invocations")
                .unwrap()
                .unwrap(),
            McpSubcommand::Activity("failed invocations".into())
        );
        assert!(parse_mcp_subcommand("ps").unwrap().is_err());
        assert_eq!(
            parse_mcp_subcommand("batch").unwrap().unwrap_err(),
            "unknown /mcp command `batch`"
        );
        assert!(parse_mcp_subcommand("inspect").unwrap().is_err());
        assert!(parse_mcp_subcommand("jobs").unwrap().is_err());
        assert!(parse_mcp_subcommand("run weather").unwrap().is_err());
        assert_eq!(
            parse_mcp_subcommand("debug").unwrap().unwrap_err(),
            "unknown /mcp command `debug`"
        );
        assert_eq!(
            parse_mcp_subcommand("invoke").unwrap().unwrap_err(),
            "unknown /mcp command `invoke`"
        );
        assert!(parse_mcp_subcommand("publish now").unwrap().is_err());
        for removed in ["view", "remote", "os", "dashboard"] {
            assert!(
                parse_mcp_subcommand(removed).unwrap().is_err(),
                "/mcp {removed} should not create an MCP prototype"
            );
        }
        assert!(parse_mcp_subcommand("weather server").is_none());
    }

    #[test]
    fn mcp_gen_prompt_carries_faas_contract_and_dir() {
        let prompt = mcp_gen_prompt("weather tools", "/Users/x/.a3s/mcps");
        assert!(prompt.contains("/Users/x/.a3s/mcps"));
        assert!(prompt.contains(".a3s/asset.acl"));
        assert!(prompt.contains("Do NOT create extra generated JSON config files"));
        assert!(prompt.contains("keep package configuration in `.a3s/asset.acl`"));
        assert!(prompt.contains("metadata-only"));
        assert!(prompt.contains("Do NOT put `server.js`"));
        assert!(prompt.contains("Function as a Service"));
        assert!(prompt.contains("do not start the server process"));
        assert!(!prompt.contains("timeout 5s"));
    }

    #[test]
    fn scaffold_mcp_project_creates_source_at_root_and_metadata_acl() {
        let root = temp_root("mcp-scaffold");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let dev = scaffold_mcp_project(
            "Name it exactly sql-checker. It exposes SQL checking tools.",
            &root,
        )
        .unwrap();

        assert_eq!(dev.name, "sql-checker");
        for rel in [
            "README.md",
            "server.js",
            "examples/example-request.md",
            "tests/smoke.md",
            ".a3s/asset.acl",
        ] {
            assert!(dev.path.join(rel).is_file(), "missing {rel}");
        }
        for rel in [
            "package.json",
            "mcp.asset.json",
            "mcp.server.json",
            "mcp.runtime-binding.json",
            ".a3s/mcp.asset.json",
            ".a3s/mcp.server.json",
            ".a3s/mcp.runtime-binding.json",
        ] {
            assert!(!dev.path.join(rel).exists(), "unexpected {rel}");
        }
        assert!(!dev.path.join(".a3s/server.js").exists());
        let source_files = collect_mcp_source_files(&dev.path).unwrap();
        let paths = source_files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"server.js"), "{paths:?}");
        assert!(!paths.contains(&"package.json"), "{paths:?}");
        assert!(
            paths.iter().all(|path| !path.starts_with(".a3s/")),
            "{paths:?}"
        );
        let asset_acl =
            std::fs::read_to_string(dev.path.join(asset_lifecycle::ASSET_ACL_PATH)).unwrap();
        assert!(asset_acl.contains("category = \"mcp\""));
        assert!(asset_acl.contains("package_root = \".\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn mcp_dev_prompt_keeps_work_local_and_names_exit_path() {
        let session = McpDevSession {
            name: "weather-tools".into(),
            description: "Weather MCP tools".into(),
            rel: "weather-tools".into(),
            path: std::path::PathBuf::from("/Users/x/.a3s/mcps/weather-tools"),
            root: std::path::PathBuf::from("/Users/x/.a3s/mcps"),
        };
        let prompt = mcp_dev_prompt(&session, "add a forecast tool");
        assert!(prompt.contains("local MCP-development mode"));
        assert!(prompt.contains("Function as a Service"));
        assert!(prompt.contains("/mcp off") && prompt.contains("Esc"));
    }

    #[test]
    fn mcp_progressive_score_prefers_faas_observe_viewlinks() {
        let asset = McpAssetRef {
            id: "asset-1".into(),
            name: "mcp-weather-tools".into(),
        };
        let params = mcp_observe_progressive_params(&asset, "weather-tools", McpOsAction::Open);
        assert_eq!(params["assetId"], "asset-1");
        assert_eq!(params["operation"], "open");
        assert_eq!(params["input"]["mcpName"], "weather-tools");

        let value = serde_json::json!({
            "data": {
                "items": [
                    {
                        "module": "functions",
                        "operation": "FunctionController_getAsset",
                        "description": "Function as a Service MCP asset metadata"
                    },
                    {
                        "module": "assets",
                        "operation": "listAssets",
                        "description": "Function as a Service open MCP asset shaped ViewLink"
                    },
                    {
                        "module": "functions",
                        "operation": "McpFunctionController_openView",
                        "description": "Function as a Service MCP RemoteUI ViewLink open"
                    },
                    {
                        "module": "agents",
                        "operation": "AgentController_open",
                        "description": "Agent as a Service RemoteUI ViewLink"
                    }
                ]
            }
        });
        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            mcp_progressive_score(McpOsAction::Open, text, operation)
        });

        assert_eq!(candidates[0].operation, "McpFunctionController_openView");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "FunctionController_getAsset"),
            "asset metadata without an open/view hint should not drive /mcp open: {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "listAssets"),
            "generic asset list must not drive /mcp open: {candidates:?}"
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Open,
                "Function as a Service MCP asset create operation with a returned ViewLink",
                "AssetController_createAsset"
            ),
            0,
            "/mcp open must not choose a mutating create operation"
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Open,
                "Function as a Service MCP asset repository preview",
                "AssetCrudController_getScaffoldTemplatePreview"
            ),
            0,
            "/mcp open must not choose scaffold/template preview operations"
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Open,
                "Issue workflow reopen action",
                "IssueController_reopenIssue"
            ),
            0,
            "/mcp open must not choose issue mutations"
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Logs,
                "Function as a Service MCP runtime logs shaped ViewLink",
                "McpFunctionController_logs"
            )
            .cmp(&0),
            std::cmp::Ordering::Greater
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Logs,
                "Function as a Service MCP run code function",
                "FunctionController_runCodeFunction"
            ),
            0,
            "/mcp logs must not run the function"
        );
    }

    #[test]
    fn mcp_progressive_score_requires_mcp_semantics_for_run() {
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Run,
                "Runtime Function tool run operation",
                "runFunction"
            ),
            0,
            "/mcp run must not choose generic Runtime Function operations"
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Run,
                "Function as a Service MCP execute operation",
                "executeMcpAsset"
            ),
            0,
            "/mcp run must not treat execute as a run capability"
        );
        assert_eq!(
            mcp_progressive_score(
                McpOsAction::Run,
                "Function as a Service MCP run shaped view",
                "runMcpAsset"
            )
            .cmp(&0),
            std::cmp::Ordering::Greater
        );
    }

    #[test]
    fn mcp_asset_payloads_keep_function_as_a_service_contract() {
        let session = McpDevSession {
            name: "weather-tools".into(),
            description: "Weather MCP tools".into(),
            rel: "weather-tools".into(),
            path: std::path::PathBuf::from("/Users/x/.a3s/mcps/weather-tools"),
            root: std::path::PathBuf::from("/Users/x/.a3s/mcps"),
        };
        let raw = serde_json::json!({
            "name": "weather-tools",
            "description": "Weather MCP tools",
            "transport": "stdio",
            "entrypoint": "server.js",
            "tools": [
                { "name": "forecast" },
                { "name": "current" }
            ]
        });
        let config = mcp_server_config_json(&session, &raw);
        let asset_name = mcp_asset_name(&mcp_config_name(&config, &session));
        let manifest = mcp_manifest_json(&session, &asset_name, &config);
        let binding = mcp_runtime_binding_json(&session, &asset_name, &config);
        let upsert = mcp_runtime_binding_upsert_body(&binding);

        assert_eq!(asset_name, "mcp-weather-tools");
        assert_eq!(
            config["runtimeIntent"]["runtimeBinding"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(config["runtimeIntent"]["runtimeBinding"]["protocol"], "mcp");
        assert_eq!(manifest["category"], "mcp");
        assert_eq!(manifest["service"], "Function as a Service");
        assert_eq!(manifest["runtimeIntent"]["kind"], "mcp");
        assert_eq!(manifest["runtimeIntent"]["isolation"], "serving");
        assert_eq!(manifest["runtimeIntent"]["agentKind"], "tool");
        assert_eq!(
            manifest["runtimeIntent"]["runtimeKind"],
            "a3s-function-service"
        );
        assert_eq!(manifest["runtimeIntent"]["protocol"], "mcp");
        assert!(manifest.get("serverConfigPath").is_none());
        assert!(manifest.get("runtimeBindingPath").is_none());
        assert_eq!(binding["kind"], "mcp");
        assert_eq!(binding["isolation"], "serving");
        assert_eq!(binding["runtime"]["kind"], "a3s-function-service");
        assert_eq!(binding["runtime"]["protocol"], "mcp");
        assert_eq!(binding["runtime"]["agentKind"], "tool");
        assert_eq!(binding["metadata"]["tools"][0], "current");
        assert_eq!(upsert["kind"], "mcp");
        assert_eq!(upsert["runtime"]["sharedRuntime"], "node-20");
        assert!(upsert["runtime"].get("agentKind").is_none());
        assert!(upsert["runtime"].get("protocol").is_none());
        assert!(upsert["target"].get("configPath").is_none());
    }

    #[test]
    fn mcp_source_upload_collects_visible_sources_only() {
        let root = temp_root("mcp-source");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".a3s")).unwrap();
        std::fs::create_dir_all(root.join("src")).unwrap();
        std::fs::create_dir_all(root.join("node_modules/pkg")).unwrap();
        std::fs::write(root.join("src/server.js"), "console.log('ok')").unwrap();
        std::fs::write(root.join(".env"), "SECRET=1").unwrap();
        std::fs::write(root.join(".a3s/asset.acl"), "version = \"a3s.asset.v1\"").unwrap();
        std::fs::write(root.join("node_modules/pkg/index.js"), "ignored").unwrap();

        let files = collect_mcp_source_files(&root).unwrap();
        let paths = files
            .iter()
            .map(|file| file.path.as_str())
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["src/server.js"]);

        let _ = std::fs::remove_dir_all(&root);
    }
}
