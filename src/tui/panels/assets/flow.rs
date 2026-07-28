//! `/flow` — workflow DAGs as local JSON files, edited and run in the
//! OS workflow designer.
//!
//! Bare `/flow` (login-gated) opens a picker over `flow_dir()` (`~/.a3s/flows`
//! or the `flow_dir` config key); Enter pushes the picked DAG into an OS
//! workflow asset (find-or-create by name, then commit it as
//! `flow.json` — the visible workflow source)
//! and opens `/workflow-designer/<asset-id>` in the authenticated
//! RemoteUI window, where it can be edited and run.
//!
//! `/flow <natural language>` asks the agent to orchestrate a BASIC DAG in
//! the designer-document schema and save it into `flow_dir()` — no login
//! needed (it's a local file until opened).

use super::super::asset_lifecycle;
use super::super::os_progressive;
use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;

/// Canonical visible source path where the designer loads/saves a workflow
/// inside the asset source workspace. `.a3s/` is reserved for `asset.acl`.
pub(crate) const DESIGN_DOCUMENT_PATH: &str = "flow.json";

/// `/flow` selection panel: the DAG JSONs under the flows folder + cursor.
pub(crate) struct FlowPanel {
    /// Absolute path of the flows root (config `flow_dir`).
    pub(crate) root: std::path::PathBuf,
    /// JSON file names (with extension), sorted for a stable panel.
    pub(crate) flows: Vec<String>,
    pub(crate) sel: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FlowSubcommand {
    Clone(String),
    List(String),
    Review(Option<String>),
    Activity(String),
    Publish,
    Run,
    Deploy,
    Open,
    Logs,
    Status,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum FlowOsAction {
    Design,
    Publish,
    Run,
    Deploy,
    Open,
    Logs,
    Status,
}

impl FlowOsAction {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Design => "open",
            Self::Publish => "publish",
            Self::Run => "run",
            Self::Deploy => "deploy",
            Self::Open => "open",
            Self::Logs => "logs",
            Self::Status => "status",
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FlowOsResult {
    pub(crate) action: FlowOsAction,
    pub(crate) asset_name: String,
    pub(crate) asset_id: String,
    pub(crate) view: remote_ui::ViewSpec,
    pub(crate) note: String,
    pub(crate) open_view: bool,
}

/// List workflow source documents under `root`, skipping hidden metadata.
pub(crate) fn list_flows(root: &std::path::Path) -> Vec<String> {
    let mut v = Vec::new();
    list_flows_inner(root, root, &mut v);
    v.sort();
    v
}

fn list_flows_inner(root: &std::path::Path, dir: &std::path::Path, out: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        if path.is_dir() {
            if name.starts_with('.') {
                continue;
            }
            list_flows_inner(root, &path, out);
            continue;
        }
        if !path.is_file() || name != DESIGN_DOCUMENT_PATH {
            continue;
        }
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|part| part.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        if rel.starts_with(".a3s/") || rel.contains("/.a3s/") {
            continue;
        }
        out.push(rel);
    }
}

pub(crate) fn parse_flow_subcommand(input: &str) -> Option<Result<FlowSubcommand, String>> {
    let mut parts = input.split_whitespace();
    let head = parts.next()?.to_ascii_lowercase();
    match head.as_str() {
        "off" | "exit" | "normal" | "clear" | "stop" => Some(Err(
            "flow selection does not enter a persistent dev mode".to_string(),
        )),
        "clone" => {
            let Some(url) = parts.next() else {
                return Some(Err("usage: /flow clone <git-url>".to_string()));
            };
            if parts.next().is_some() {
                return Some(Err("usage: /flow clone <git-url>".to_string()));
            }
            Some(Ok(FlowSubcommand::Clone(url.to_string())))
        }
        "list" => Some(Ok(FlowSubcommand::List(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "review" => {
            let target = parts.next().map(str::to_string);
            if parts.next().is_some() {
                return Some(Err("usage: /flow review [file]".to_string()));
            }
            Some(Ok(FlowSubcommand::Review(target)))
        }
        "activity" => Some(Ok(FlowSubcommand::Activity(
            parts.collect::<Vec<_>>().join(" "),
        ))),
        "ps" | "runs" | "jobs" => Some(Err("usage: /flow activity [query]".to_string())),
        "workflow" | "artifact" => Some(Err(
            "/flow is for OS Workflow as a Service assets; use /flow open, /flow run, or /flow status"
                .to_string(),
        )),
        "publish" => {
            if parts.next().is_some() {
                return Some(Err("usage: /flow publish".to_string()));
            }
            Some(Ok(FlowSubcommand::Publish))
        }
        "run" => {
            if parts.next().is_some() {
                return Some(Err("usage: /flow run".to_string()));
            }
            Some(Ok(FlowSubcommand::Run))
        }
        "debug" => Some(Err("unknown /flow command `debug`".to_string())),
        "deploy" => {
            if parts.next().is_some() {
                return Some(Err("usage: /flow deploy".to_string()));
            }
            Some(Ok(FlowSubcommand::Deploy))
        }
        "open" => {
            if parts.next().is_some() {
                return Some(Err("usage: /flow open".to_string()));
            }
            Some(Ok(FlowSubcommand::Open))
        }
        "logs" => {
            if parts.next().is_some() {
                return Some(Err("usage: /flow logs".to_string()));
            }
            Some(Ok(FlowSubcommand::Logs))
        }
        "status" => {
            if parts.next().is_some() {
                return Some(Err("usage: /flow status".to_string()));
            }
            Some(Ok(FlowSubcommand::Status))
        }
        "inspect" => Some(Err("usage: /flow status".to_string())),
        "view" | "remote" => Some(Err("usage: /flow open".to_string())),
        "os" => Some(Err("usage: /flow status".to_string())),
        "dashboard" => Some(Err("usage: /flow list [query] · /flow status".to_string())),
        _ => None,
    }
}

/// The OS asset a flow file maps to — a deterministic name so re-opening the
/// same file updates the same asset instead of piling up duplicates.
pub(crate) fn flow_asset_name(file_stem: &str) -> String {
    let normalized = file_stem.replace('\\', "/");
    let parts = normalized
        .split('/')
        .filter(|part| !part.trim().is_empty())
        .collect::<Vec<_>>();
    let name = if parts.len() >= 2 && parts[parts.len() - 1] == "flow" {
        parts[parts.len() - 2]
    } else {
        parts.last().copied().unwrap_or(file_stem)
    };
    format!("flow-{}", asset_slug(name))
}

/// The STANDALONE designer page for an asset (login-guarded, no admin
/// chrome — mirrors the OS `/assistant` standalone surface). Route param
/// only; no query support.
pub(crate) fn designer_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/workflow-designer/{}",
        origin.trim_end_matches('/'),
        asset_id
    )
}

pub(crate) fn designer_view_spec(url: String) -> remote_ui::ViewSpec {
    remote_ui::ViewSpec {
        url,
        width: Some(1440),
        height: Some(900),
        embeddable: true,
    }
}

#[derive(Clone, Copy)]
enum FlowProgressiveIntent {
    Designer,
    Run,
    Logs,
}

fn flow_progressive_score(intent: FlowProgressiveIntent, text: &str, operation: &str) -> i32 {
    let combined = format!("{text} {operation}").to_ascii_lowercase();
    let operation = operation.to_ascii_lowercase();
    if operation.contains("createasset")
        || operation.contains("listasset")
        || operation.contains("diagnose")
        || operation.contains("acknowledge")
        || operation.contains("scaffold")
        || operation.contains("template")
        || operation.contains("preview")
    {
        return 0;
    }
    let mut score = 0;
    if combined.contains("workflow") || combined.contains("waas") {
        score += 8;
    }
    if combined.contains("asset") {
        score += 3;
    }
    let mut action_hit = false;
    match intent {
        FlowProgressiveIntent::Designer => {
            if operation.contains("designer")
                || operation.contains("design")
                || combined.contains("/designer")
                || combined.contains("workflow-designer")
            {
                score += 8;
                action_hit = true;
            }
            if operation.contains("open")
                || (operation.contains("view") && !operation.contains("preview"))
            {
                score += 4;
                action_hit = true;
            }
            if combined.contains("log")
                || combined.contains("trace")
                || combined.contains("job")
                || combined.contains("process")
                || combined.contains("observability")
            {
                score -= 6;
            }
        }
        FlowProgressiveIntent::Run => {
            if operation == "runassetworkflow"
                || operation.contains("runassetworkflow")
                || combined.contains("/asset-workflows/{assetid}/run")
            {
                score += 14;
                action_hit = true;
            }
            if operation.contains("dispatchworkflowrun") {
                score += 4;
                action_hit = true;
            }
            if operation.contains("list")
                || operation.contains("get")
                || operation.contains("logs")
                || operation.contains("feedback")
                || operation.contains("schedule")
                || operation.contains("webhook")
                || operation.contains("testnode")
            {
                score -= 12;
            }
        }
        FlowProgressiveIntent::Logs => {
            if operation.contains("workflowrunlog")
                || operation.contains("workflowlogs")
                || operation.contains("listworkflowexecutions")
                || operation.contains("listassetworkflowexecutions")
            {
                score += 10;
                action_hit = true;
            }
            if operation.contains("getjoblogs")
                || operation.contains("downloadjoblogs")
                || operation.contains("getsteplogs")
                || operation.contains("downloadsteplogs")
                || operation.contains("getrunlogs")
                || operation.contains("downloadrunlogs")
                || operation.contains("listjobs")
            {
                return 0;
            }
            if combined.contains("view")
                || combined.contains("remoteui")
                || combined.contains("shaped")
            {
                score += 3;
            }
            if combined.contains("designer") || combined.contains("design") {
                score -= 8;
            }
        }
    }
    if combined.contains("agent as a service")
        || combined.contains("function as a service")
        || combined.contains("mcp")
    {
        score -= 10;
    }
    if action_hit && score >= 8 {
        score
    } else {
        0
    }
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

fn unix_timestamp_secs() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_secs())
        .unwrap_or(0)
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

fn flow_asset_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/assets/{}?embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn flow_asset_search_url(origin: &str, asset_name: &str) -> String {
    format!(
        "{}/assets?category=workflow&scope=mine&status=all&search={}&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_name)
    )
}

fn flow_logs_url(origin: &str, asset_id: &str) -> String {
    format!(
        "{}/admin/infrastructure/batch?asset={}&category=workflow&logs=1&embed=1",
        origin.trim_end_matches('/'),
        path_segment(asset_id)
    )
}

fn workflow_runtime_binding_upsert_body(runtime_binding: &serde_json::Value) -> serde_json::Value {
    let target_ref = runtime_binding
        .pointer("/target/ref")
        .and_then(|value| value.as_str())
        .unwrap_or("main");
    let runtime_kind = runtime_binding
        .pointer("/runtime/kind")
        .and_then(|value| value.as_str())
        .unwrap_or("a3s-workflow-service");
    serde_json::json!({
        "kind": runtime_binding
            .get("kind")
            .and_then(|value| value.as_str())
            .unwrap_or("workflow"),
        "isolation": runtime_binding
            .get("isolation")
            .and_then(|value| value.as_str())
            .unwrap_or("native"),
        "target": {
            "kind": "asset",
            "ref": target_ref,
        },
        "runtime": {
            "kind": runtime_kind,
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

fn workflow_design_name(design: &serde_json::Value, fallback: &str) -> String {
    json_str_at(design, &["/name", "name"])
        .unwrap_or(fallback)
        .to_string()
}

fn workflow_design_description(design: &serde_json::Value) -> String {
    json_str_at(design, &["/description", "description"])
        .unwrap_or("Local workflow DAG")
        .to_string()
}

fn workflow_node_count(design: &serde_json::Value) -> usize {
    design
        .get("nodes")
        .and_then(|nodes| nodes.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn workflow_edge_count(design: &serde_json::Value) -> usize {
    design
        .get("edges")
        .and_then(|edges| edges.as_array())
        .map(Vec::len)
        .unwrap_or(0)
}

fn workflow_manifest_json(
    asset_name: &str,
    local_file: &str,
    design: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.workflow.asset.v1",
        "category": "workflow",
        "name": asset_name,
        "workflowName": workflow_design_name(design, asset_name),
        "description": workflow_design_description(design),
        "designDocumentPath": DESIGN_DOCUMENT_PATH,
        "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        "localFile": local_file,
        "createdBy": "a3s-code-tui",
        "service": "Workflow as a Service",
        "runtimeIntent": {
            "kind": "workflow",
            "isolation": "native",
            "runtimeKind": "a3s-workflow-service",
            "protocol": "workflow",
        },
        "graph": {
            "nodes": workflow_node_count(design),
            "edges": workflow_edge_count(design),
        },
    })
}

fn workflow_runtime_binding_json(
    asset_name: &str,
    local_file: &str,
    design: &serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.workflow.runtime-binding.v1",
        "kind": "workflow",
        "enabled": true,
        "isolation": "native",
        "target": {
            "kind": "asset",
            "ref": "main",
            "designDocumentPath": DESIGN_DOCUMENT_PATH,
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
        },
        "runtime": {
            "kind": "a3s-workflow-service",
            "protocol": "workflow",
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
            "service": "Workflow as a Service",
            "assetCategory": "workflow",
            "assetName": asset_name,
            "workflowName": workflow_design_name(design, asset_name),
            "description": workflow_design_description(design),
            "designDocumentPath": DESIGN_DOCUMENT_PATH,
            "assetAclPath": asset_lifecycle::ASSET_ACL_PATH,
            "localFile": local_file,
            "nodes": workflow_node_count(design),
            "edges": workflow_edge_count(design),
        },
    })
}

fn workflow_asset_acl(asset_name: &str, local_file: &str, design: &serde_json::Value) -> String {
    let source = [("design_document_path", DESIGN_DOCUMENT_PATH)];
    let metadata: [(&str, &str); 0] = [];
    let description = workflow_design_description(design);
    asset_lifecycle::render_asset_acl(asset_lifecycle::AssetAclDocument {
        category: "workflow",
        kind: Some("workflow"),
        name: asset_name,
        description: &description,
        local_path: Some(local_file),
        service: asset_lifecycle::OsService::WorkflowAsAService,
        runtime: asset_lifecycle::RuntimeBindingIntent {
            kind: "workflow",
            isolation: "native",
            runtime_kind: "a3s-workflow-service",
            protocol: Some("workflow"),
            agent_kind: None,
        },
        source: &source,
        metadata: &metadata,
    })
}

async fn workflow_designer_view(
    origin: &str,
    token: &str,
    asset_id: &str,
    asset_name: &str,
) -> (remote_ui::ViewSpec, bool) {
    let fallback = designer_view_spec(designer_url(origin, asset_id));
    let Ok(client) = http() else {
        return (fallback, false);
    };
    let params = serde_json::json!({
        "assetId": asset_id,
        "id": asset_id,
        "workflowAssetId": asset_id,
        "assetName": asset_name,
        "name": asset_name,
        "category": "workflow",
        "source": "a3s-code-tui",
    });
    let Some(execution) = os_progressive::execute_first_matching(
        &client,
        origin,
        token,
        "Workflow as a Service open workflow designer asset shaped view",
        params,
        |text, operation| flow_progressive_score(FlowProgressiveIntent::Designer, text, operation),
    )
    .await
    else {
        return (fallback, false);
    };
    (execution.view.unwrap_or(fallback), true)
}

async fn workflow_logs_view(
    origin: &str,
    token: &str,
    asset_id: &str,
    asset_name: &str,
) -> (remote_ui::ViewSpec, bool) {
    let fallback = designer_view_spec(flow_logs_url(origin, asset_id));
    let Ok(client) = http() else {
        return (fallback, false);
    };
    let params = serde_json::json!({
        "assetId": asset_id,
        "id": asset_id,
        "workflowAssetId": asset_id,
        "assetName": asset_name,
        "name": asset_name,
        "category": "workflow",
        "operation": "logs",
        "source": "a3s-code-tui",
    });
    let Some(execution) = os_progressive::execute_first_matching(
        &client,
        origin,
        token,
        "Workflow as a Service logs runs jobs shaped ViewLink",
        params,
        |text, operation| flow_progressive_score(FlowProgressiveIntent::Logs, text, operation),
    )
    .await
    else {
        return (fallback, false);
    };
    (execution.view.unwrap_or(fallback), true)
}

async fn workflow_run_view(
    origin: &str,
    token: &str,
    asset_id: &str,
    asset_name: &str,
) -> Option<(remote_ui::ViewSpec, String)> {
    let Ok(client) = http() else {
        return None;
    };
    let params = serde_json::json!({
        "assetId": asset_id,
        "id": asset_id,
        "workflowAssetId": asset_id,
        "assetName": asset_name,
        "name": asset_name,
        "category": "workflow",
        "operation": "run",
        "input": {
            "assetId": asset_id,
            "assetName": asset_name,
            "source": "a3s-code-tui",
        },
        "payload": {
            "assetId": asset_id,
            "assetName": asset_name,
            "source": "a3s-code-tui",
        },
        "source": "a3s-code-tui",
        "idempotencyKey": format!("a3s-code-flow-run-{}", unix_timestamp_secs()),
    });
    let execution = os_progressive::execute_first_matching(
        &client,
        origin,
        token,
        "Workflow as a Service run workflow asset shaped ViewLink",
        params,
        |text, operation| flow_progressive_score(FlowProgressiveIntent::Run, text, operation),
    )
    .await?;
    let fallback = designer_view_spec(flow_asset_url(origin, asset_id));
    Some((
        execution.view.unwrap_or(fallback),
        format!(
            "Published `{asset_name}` and executed the Workflow as a Service run through progressive capabilities (`{}`).",
            execution.operation.operation
        ),
    ))
}

fn flow_picker_header(total: usize, root: &std::path::Path, width: usize) -> String {
    truncate(
        &format!(
            "  ⧉ flow — select a DAG ({total} in {})",
            root.to_string_lossy()
        ),
        width,
    )
}

fn flow_picker_hint(width: usize) -> String {
    truncate(
        "  ↑/↓ select · Enter open in the OS workflow designer · Esc cancel",
        width,
    )
}

fn flow_picker_lines(
    flows: &[String],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) = flow_picker_panel(flows, selected, root, width, height)
    else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn flow_picker_panel(
    flows: &[String],
    selected: usize,
    root: &std::path::Path,
    width: usize,
    height: usize,
) -> Option<(MenuPanel, usize)> {
    let total = flows.len();
    if total == 0 {
        return None;
    }
    let max_items = height.saturating_sub(8).clamp(3, 12);
    let selected = selected.min(total.saturating_sub(1));
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let items = flows
        .iter()
        .map(|name| MenuItem::new(name.clone()))
        .collect::<Vec<_>>();

    let panel = MenuPanel::new(flow_picker_header(total, root, width).trim_start())
        .subtitle(flow_picker_hint(width).trim_start())
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

fn flow_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

/// Directive for `/flow <description>`: orchestrate a BASIC DAG in the
/// designer-document schema and save it under the flows folder.
#[cfg(test)]
pub(crate) fn flow_gen_prompt(description: &str, dir: &str) -> String {
    format!(
        "Create a basic local workflow flow asset from the description below and save it under \
         {dir}. This is a SMALL asset-folder task: do it directly in this turn — do NOT \
         plan, delegate, or fan out subagents.\n\
         Description: {description}\n\
         IMPORTANT: {dir} is OUTSIDE this session's workspace, so the path-scoped file \
         tools will reject it — use the `bash` tool (`mkdir -p {dir}`, then write files \
         with heredocs).\n\
         Create {dir}/<kebab-case-name>/ with flow.json and .a3s/asset.acl. The JSON DAG \
         is source and MUST stay at the asset root as `flow.json`; do NOT put workflow \
         source under `.a3s/` or any other hidden metadata directory. Do NOT create \
         extra generated JSON config files; keep package configuration in `.a3s/asset.acl`. \
         The asset.acl must record service=Workflow as a Service, \
         runtimeIntent.kind=workflow, runtime.kind=a3s-workflow-service, and protocol=workflow. \
         MUST follow the OS workflow-designer document schema exactly — this \
         minimal example shows every required field:\n\
         {{\"version\":\"a3s.workflow.design.v1\",\"name\":\"<name>\",\"description\":\
         \"<one line>\",\"triggerEvents\":[],\"variables\":[],\"outputs\":[],\
         \"nodes\":[{{\"id\":\"start\",\"kind\":\"start\",\"name\":\"Start\",\"data\":{{}},\
         \"x\":0,\"y\":0}},{{\"id\":\"step-1\",\"kind\":\"llm\",\"name\":\"<step>\",\
         \"data\":{{}},\"x\":320,\"y\":0}},{{\"id\":\"end\",\"kind\":\"end\",\
         \"name\":\"End\",\"data\":{{}},\"x\":640,\"y\":0}}],\
         \"edges\":[{{\"id\":\"e1\",\"sourceNodeID\":\"start\",\"targetNodeID\":\
         \"step-1\"}},{{\"id\":\"e2\",\"sourceNodeID\":\"step-1\",\"targetNodeID\":\
         \"end\"}}]}}\n\
         Rules: exactly one `start` and one `end` node; node kinds ONLY from: start, \
         end, llm, http, code, condition, loop, template, answer, knowledge-retrieval, \
         question-classifier, parameter-extractor, aggregator; unique kebab-case ids; \
         edges use sourceNodeID/targetNodeID (capital D); lay nodes left-to-right \
         (x += 320 per step, y ±180 for branches); keep it BASIC — 3-7 nodes that \
         match the description, no speculative extras.\n\
         If the folder exists, append -2, -3, … to the folder name. \
         Validate flow.json with `python3 -m json.tool \"$FILE\" > /dev/null && echo OK` \
         (always pass the file path — never run a command that waits on stdin). After \
         validation succeeds, stop using tools immediately and give a concise final answer \
         with the saved path and the note that `/flow` opens it in the OS workflow designer."
    )
}

pub(crate) fn scaffold_flow_asset(
    description: &str,
    root: &std::path::Path,
) -> Result<std::path::PathBuf, String> {
    let name = asset_lifecycle::scaffold_name(description, "workflow");
    let package = asset_lifecycle::unique_asset_dir(root, &name);
    let final_name = package
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(name.as_str())
        .to_string();
    let description =
        asset_lifecycle::scaffold_description(description, &final_name, "Local workflow asset");
    std::fs::create_dir_all(package.join(".a3s"))
        .map_err(|e| format!("could not create {}: {e}", package.join(".a3s").display()))?;
    std::fs::create_dir_all(package.join("tests"))
        .map_err(|e| format!("could not create {}: {e}", package.join("tests").display()))?;

    let design = flow_scaffold_design(&final_name, &description);
    let design_path = package.join(DESIGN_DOCUMENT_PATH);
    let local_file = asset_lifecycle::normalized_rel(root, &design_path);
    let asset_name = flow_asset_name(&final_name);
    let asset_acl = workflow_asset_acl(&asset_name, &local_file, &design);

    asset_lifecycle::write_scaffold_json(&design_path, &design)?;
    asset_lifecycle::write_scaffold_file(
        &package.join("README.md"),
        flow_scaffold_readme(&final_name, &description).as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join("tests/smoke.md"),
        flow_scaffold_tests().as_bytes(),
    )?;
    asset_lifecycle::write_scaffold_file(
        &package.join(asset_lifecycle::ASSET_ACL_PATH),
        asset_acl.as_bytes(),
    )?;
    Ok(design_path)
}

fn flow_scaffold_design(name: &str, description: &str) -> serde_json::Value {
    serde_json::json!({
        "version": "a3s.workflow.design.v1",
        "name": name,
        "description": description,
        "triggerEvents": [],
        "variables": [],
        "outputs": [],
        "nodes": [
            { "id": "start", "kind": "start", "name": "Start", "data": {}, "x": 0, "y": 0 },
            { "id": "step-1", "kind": "llm", "name": "Process request", "data": { "prompt": description }, "x": 320, "y": 0 },
            { "id": "end", "kind": "end", "name": "End", "data": {}, "x": 640, "y": 0 }
        ],
        "edges": [
            { "id": "e1", "sourceNodeID": "start", "targetNodeID": "step-1" },
            { "id": "e2", "sourceNodeID": "step-1", "targetNodeID": "end" }
        ]
    })
}

fn flow_scaffold_readme(name: &str, description: &str) -> String {
    format!(
        "# {name}\n\n\
         {description}.\n\n\
         ## Source\n\n\
         - `flow.json` is the workflow source document loaded by the OS designer.\n\
         - `tests/smoke.md` contains manual validation checks.\n\
         - `.a3s/` contains only `asset.acl`.\n\n\
         ## Lifecycle\n\n\
         - `a3s code flow publish flow.json`\n\
         - `a3s code flow run flow.json`\n\
         - `a3s code flow open flow.json`\n"
    )
}

fn flow_scaffold_tests() -> &'static str {
    "# Workflow Smoke Checklist\n\n\
     1. Validate `flow.json` with `python3 -m json.tool`.\n\
     2. Confirm the DAG has exactly one `start` and one `end` node.\n\
     3. Confirm `.a3s/asset.acl` has `design_document_path = \"flow.json\"`.\n"
}

pub(crate) fn flow_review_prompt(path: &std::path::Path, design_json: &str) -> String {
    let asset_dir = path.parent().unwrap_or_else(|| std::path::Path::new("."));
    let contract = super::review::review_report_contract(asset_dir);
    format!(
        "Review this local workflow flow asset without changing files unless the user explicitly \
         asks for fixes.\n\
         Flow path: {path}\n\n\
         Workflow JSON:\n```json\n{design_json}\n```\n\n\
         Report concise findings on: schema shape, start/end correctness, node ids, edge \
         connectivity, unsupported node kinds, branch/loop clarity, variables/outputs, secret \
         handling, testability, and readiness for Workflow as a Service. Mention the smallest \
         recommended improvements and whether `/flow run` or `/flow deploy` is the right next \
         lifecycle step.{contract}",
        path = path.display(),
    )
}

// ---------------------------------------------------------------------------
// OS assets REST (lenient: parse ids out of serde_json::Value, not rigid DTOs)
// ---------------------------------------------------------------------------

fn http() -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder.build().map_err(|e| e.to_string())
}

/// Read `data.items[]` (paginated envelope) or any nested `items[]` array.
fn items_of(v: &serde_json::Value) -> Vec<serde_json::Value> {
    v.pointer("/data/items")
        .or_else(|| v.pointer("/data"))
        .or_else(|| v.pointer("/items"))
        .and_then(|d| d.as_array().cloned())
        .unwrap_or_default()
}

/// Find a workflow asset by exact name, else create one (private, user-owned;
/// creation auto-initializes backing source storage). Returns the asset id.
pub(crate) async fn ensure_flow_asset(
    origin: &str,
    token: &str,
    name: &str,
) -> Result<String, String> {
    let client = http()?;
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    // Search is ILIKE-broad; require the exact name client-side.
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", name),
            ("category", "workflow"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    if let Some((id, _)) = find_flow_asset(&found, name)? {
        return Ok(id);
    }
    let resp = client
        .post(&base)
        .bearer_auth(token)
        .json(&serde_json::json!({
            "name": name,
            "ownerType": "user",
            "category": "workflow",
            "visibility": "private",
            "description": "Created by a3s code /flow",
            "metadata": {
                "service": "Workflow as a Service",
                "runtimeKind": "a3s-workflow-service",
                "protocol": "workflow",
                "createdBy": "a3s-code-tui",
            },
        }))
        .send()
        .await
        .map_err(|e| e.to_string())?;
    let status = resp.status();
    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "create asset failed ({status}): {}",
            body.get("message").and_then(|m| m.as_str()).unwrap_or("?")
        ));
    }
    body.pointer("/data/id")
        .or_else(|| body.get("id"))
        .and_then(|i| i.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "create asset: no id in response".to_string())
}

/// Commit the DAG into the asset source at the designer's canonical path.
pub(crate) async fn upload_flow_document(
    origin: &str,
    token: &str,
    asset_id: &str,
    design_json: &str,
    asset_acl: &str,
    _manifest: &serde_json::Value,
    _runtime_binding: &serde_json::Value,
) -> Result<(), String> {
    use base64::Engine;
    let b64 = base64::engine::general_purpose::STANDARD.encode(design_json.as_bytes());
    let asset_acl_b64 = base64::engine::general_purpose::STANDARD.encode(asset_acl.as_bytes());
    let resp = http()?
        .post(format!(
            "{}/api/v1/assets/{asset_id}/repository/files",
            origin.trim_end_matches('/')
        ))
        .bearer_auth(token)
        .json(&serde_json::json!({
            "overwrite": true,
            "message": "a3s code /flow: update workflow design",
            "files": [
                { "path": DESIGN_DOCUMENT_PATH, "contentBase64": b64 },
                { "path": asset_lifecycle::ASSET_ACL_PATH, "contentBase64": asset_acl_b64 },
            ],
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
        "upload failed ({status}): {}",
        truncate(&body, 200)
    ))
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FlowRuntimeBindingSync {
    Synced,
    Unsupported,
    Failed(String),
}

async fn sync_flow_runtime_binding(
    origin: &str,
    token: &str,
    asset_id: &str,
    runtime_binding: &serde_json::Value,
) -> FlowRuntimeBindingSync {
    match sync_flow_runtime_binding_inner(origin, token, asset_id, runtime_binding).await {
        Ok(true) => FlowRuntimeBindingSync::Synced,
        Ok(false) => FlowRuntimeBindingSync::Unsupported,
        Err(err) => FlowRuntimeBindingSync::Failed(err),
    }
}

async fn sync_flow_runtime_binding_inner(
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
        .json(&workflow_runtime_binding_upsert_body(runtime_binding))
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

fn find_flow_asset(
    value: &serde_json::Value,
    name: &str,
) -> Result<Option<(String, String)>, String> {
    let exact = items_of(value)
        .into_iter()
        .find(|item| json_str_at(item, &["/name", "name"]) == Some(name));
    let Some(asset) = exact else {
        return Ok(None);
    };
    if let Some(actual) = asset_category(&asset) {
        if !actual.eq_ignore_ascii_case("workflow") {
            return Err(category_conflict_error(name, actual, "workflow"));
        }
    }
    let id = json_str_at(&asset, &["/id", "id", "/_id", "_id", "/assetId", "assetId"])
        .ok_or_else(|| format!("asset `{name}` matched but had no id"))?;
    let name =
        json_str_at(&asset, &["/name", "name", "/displayName", "displayName"]).unwrap_or(name);
    Ok(Some((id.to_string(), name.to_string())))
}

async fn inspect_flow_asset(
    origin: &str,
    token: &str,
    action: FlowOsAction,
    asset_name: &str,
) -> Result<FlowOsResult, String> {
    let client = http()?;
    let base = format!("{}/api/v1/assets", origin.trim_end_matches('/'));
    let found: serde_json::Value = client
        .get(&base)
        .query(&[
            ("scope", "mine"),
            ("status", "all"),
            ("search", asset_name),
            ("category", "workflow"),
            ("limit", "50"),
        ])
        .bearer_auth(token)
        .send()
        .await
        .map_err(|e| e.to_string())?
        .json()
        .await
        .map_err(|e| e.to_string())?;
    let Some((asset_id, found_name)) = find_flow_asset(&found, asset_name)? else {
        return Ok(FlowOsResult {
            action,
            asset_name: asset_name.to_string(),
            asset_id: "not-published".to_string(),
            view: designer_view_spec(flow_asset_search_url(origin, asset_name)),
            note: format!(
                "OS {} for `{asset_name}`: no workflow asset named `{asset_name}` was found. Run `/flow publish` first.",
                action.label()
            ),
            open_view: false,
        });
    };
    if matches!(action, FlowOsAction::Open) {
        let (view, progressive) =
            workflow_designer_view(origin, token, &asset_id, &found_name).await;
        let surface = if progressive {
            "progressive Workflow as a Service ViewLink"
        } else {
            "Workflow as a Service designer fallback"
        };
        return Ok(FlowOsResult {
            action,
            asset_name: found_name,
            asset_id,
            view,
            note: format!("Opened existing OS workflow asset through the {surface}."),
            open_view: true,
        });
    }
    if matches!(action, FlowOsAction::Logs) {
        let (view, progressive) = workflow_logs_view(origin, token, &asset_id, &found_name).await;
        let surface = if progressive {
            "progressive Workflow as a Service logs ViewLink"
        } else {
            "Workflow as a Service logs fallback"
        };
        return Ok(FlowOsResult {
            action,
            asset_name: found_name,
            asset_id,
            view,
            note: format!("Opened existing OS workflow logs through the {surface}."),
            open_view: true,
        });
    }
    let binding_status = runtime_binding_validation_status(&client, origin, token, &asset_id).await;
    Ok(FlowOsResult {
        action,
        asset_name: found_name,
        asset_id: asset_id.clone(),
        view: designer_view_spec(flow_asset_url(origin, &asset_id)),
        note: format!("OS status for `{asset_name}`: asset exists; {binding_status}."),
        open_view: false,
    })
}

fn append_flow_runtime_binding_sync_note(
    mut note: String,
    runtime_binding_synced: &FlowRuntimeBindingSync,
) -> String {
    match runtime_binding_synced {
        FlowRuntimeBindingSync::Synced => note.push_str(" OS runtime binding was synced."),
        FlowRuntimeBindingSync::Unsupported => note.push_str(
            " OS runtime-binding endpoint was unavailable; runtime-binding intent was saved.",
        ),
        FlowRuntimeBindingSync::Failed(err) => note.push_str(&format!(
            " OS runtime binding could not be synced: {}; runtime-binding intent was saved.",
            truncate(err, 160)
        )),
    }
    note
}

pub(crate) async fn publish_flow_to_os(
    session: crate::a3s_os::StoredOsSession,
    file: String,
    design_json: String,
    action: FlowOsAction,
) -> Result<FlowOsResult, String> {
    publish_flow_to_os_with_local_path(session, file, None, design_json, action).await
}

pub(crate) async fn publish_flow_to_os_with_local_path(
    session: crate::a3s_os::StoredOsSession,
    file: String,
    local_path: Option<std::path::PathBuf>,
    design_json: String,
    action: FlowOsAction,
) -> Result<FlowOsResult, String> {
    let origin = crate::a3s_os::os_origin(&session.address);
    let stem = file.trim_end_matches(".json").to_string();
    let design: serde_json::Value = serde_json::from_str(&design_json)
        .map_err(|e| format!("workflow design is not valid JSON: {e}"))?;
    let design_name = workflow_design_name(&design, &stem);
    let asset_name = flow_asset_name(&design_name);
    if matches!(
        action,
        FlowOsAction::Status | FlowOsAction::Open | FlowOsAction::Logs
    ) {
        return inspect_flow_asset(&origin, &session.access_token, action, &asset_name).await;
    }
    let asset_id = ensure_flow_asset(&origin, &session.access_token, &asset_name).await?;
    let manifest = workflow_manifest_json(&asset_name, &file, &design);
    let runtime_binding = workflow_runtime_binding_json(&asset_name, &file, &design);
    let asset_acl = workflow_asset_acl(&asset_name, &file, &design);
    if let Some(local_path) = local_path.as_deref() {
        asset_lifecycle::write_asset_acl(local_path, &asset_acl)?;
    }
    upload_flow_document(
        &origin,
        &session.access_token,
        &asset_id,
        &design_json,
        &asset_acl,
        &manifest,
        &runtime_binding,
    )
    .await?;
    let runtime_binding_synced =
        sync_flow_runtime_binding(&origin, &session.access_token, &asset_id, &runtime_binding)
            .await;
    let (view, note, open_view) = match action {
        FlowOsAction::Publish => (
            designer_view_spec(flow_asset_url(&origin, &asset_id)),
            format!(
                "Published `{asset_name}` as an OS workflow asset backed by Workflow as a Service."
            ),
            true,
        ),
        FlowOsAction::Design => {
            let (view, progressive) =
                workflow_designer_view(&origin, &session.access_token, &asset_id, &asset_name)
                    .await;
            let surface = if progressive {
                "progressive Workflow as a Service ViewLink"
            } else {
                "Workflow as a Service designer fallback"
            };
            (
                view,
                format!("Published `{asset_name}` and opened the {surface} for editing."),
                true,
            )
        }
        FlowOsAction::Run => workflow_run_view(
            &origin,
            &session.access_token,
            &asset_id,
            &asset_name,
        )
        .await
        .map(|(view, note)| (view, note, true))
        .unwrap_or_else(|| {
            (
                designer_view_spec(designer_url(&origin, &asset_id)),
                format!(
                    "Published `{asset_name}` and saved its Workflow as a Service runtime binding; no runnable workflow capability was available, so the designer fallback was opened."
                ),
                true,
            )
        }),
        FlowOsAction::Deploy => (
            designer_view_spec(designer_url(&origin, &asset_id)),
            format!(
                "Published `{asset_name}` and saved its Workflow as a Service runtime binding; no separate workflow deploy capability was available, so the designer fallback was opened."
            ),
            true,
        ),
        FlowOsAction::Open | FlowOsAction::Logs | FlowOsAction::Status => {
            unreachable!("read-only flow actions return before publish flow")
        }
    };
    let note = append_flow_runtime_binding_sync_note(note, &runtime_binding_synced);
    Ok(FlowOsResult {
        action,
        asset_name,
        asset_id,
        view,
        note,
        open_view,
    })
}

impl App {
    /// Open the `/flow` picker (login-gated by the caller).
    pub(crate) fn open_flow_panel(&mut self) {
        let root = self.asset_directories.flow.clone();
        let flows = list_flows(&root);
        if flows.is_empty() {
            self.pending_flow_subcommand = None;
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  no flows in {} — draft one with `/flow <description>` first",
                root.display()
            )));
            return;
        }
        self.flow = Some(FlowPanel {
            root,
            flows,
            sel: 0,
        });
    }

    /// Keys while the `/flow` picker is open — consumes everything.
    pub(crate) fn handle_flow_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let p = self.flow.as_mut()?;
        let last = p.flows.len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => p.sel = p.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => p.sel = (p.sel + 1).min(last),
            KeyCode::Esc => {
                cancel_pending_picker(&mut self.flow, &mut self.pending_flow_subcommand)
            }
            KeyCode::Enter => {
                let panel = self.flow.take()?;
                let file = panel.flows.get(panel.sel.min(last))?.clone();
                let path = panel.root.join(&file);
                let pending = self.pending_flow_subcommand.take();
                if matches!(
                    pending,
                    Some(FlowSubcommand::Open)
                        | Some(FlowSubcommand::Logs)
                        | Some(FlowSubcommand::Status)
                ) {
                    let session = self.os_session.clone()?;
                    let os_action = match pending {
                        Some(FlowSubcommand::Open) => FlowOsAction::Open,
                        Some(FlowSubcommand::Logs) => FlowOsAction::Logs,
                        Some(FlowSubcommand::Status) => FlowOsAction::Status,
                        _ => unreachable!(),
                    };
                    let status_entry =
                        self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
                            "  ⧉ {file} → OS Workflow as a Service {}…",
                            os_action.label()
                        )));
                    return Some(cmd::cmd(move || async move {
                        let result =
                            publish_flow_to_os(session, file, String::new(), os_action).await;
                        Msg::FlowOsCompleted {
                            status_entry,
                            result,
                        }
                    }));
                }
                let design = match std::fs::read_to_string(&path) {
                    Ok(s) => s,
                    Err(e) => {
                        self.push_line(
                            &Style::new()
                                .fg(TN_RED)
                                .render(&format!("  could not read {}: {e}", path.display())),
                        );
                        return None;
                    }
                };
                if serde_json::from_str::<serde_json::Value>(&design).is_err() {
                    self.push_line(&Style::new().fg(TN_RED).render(&format!(
                        "  {} is not valid JSON — fix it (or redraft with /flow <description>)",
                        file
                    )));
                    return None;
                }
                match pending {
                    Some(FlowSubcommand::Review(_)) => {
                        self.messages
                            .push(TranscriptEntry::user(format!("/flow review {file}")));
                        self.engage_autonomy(4);
                        self.review_pending = true;
                        let prompt = flow_review_prompt(&path, &design);
                        let display = format!("⧉ flow review: {}", truncate(&file, 48));
                        return self.start_stream_inner(prompt, display, true, true, false);
                    }
                    Some(FlowSubcommand::Activity(query)) => {
                        let stem = file.trim_end_matches(".json").to_string();
                        let asset_name = flow_asset_name(&stem);
                        let query = runtime_asset_query("workflow", &asset_name, &query);
                        return self.open_runtime_activity_panel(query);
                    }
                    Some(FlowSubcommand::Clone(_))
                    | Some(FlowSubcommand::List(_))
                    | Some(FlowSubcommand::Open)
                    | Some(FlowSubcommand::Logs)
                    | Some(FlowSubcommand::Status)
                    | None => {}
                    Some(action @ FlowSubcommand::Publish)
                    | Some(action @ FlowSubcommand::Run)
                    | Some(action @ FlowSubcommand::Deploy) => {
                        let session = self.os_session.clone()?;
                        let os_action = match action {
                            FlowSubcommand::Publish => FlowOsAction::Publish,
                            FlowSubcommand::Run => FlowOsAction::Run,
                            FlowSubcommand::Deploy => FlowOsAction::Deploy,
                            _ => unreachable!(),
                        };
                        let status_entry =
                            self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
                                "  ⧉ {file} → OS Workflow as a Service {}…",
                                os_action.label()
                            )));
                        let local_path = path.clone();
                        return Some(cmd::cmd(move || async move {
                            let result = publish_flow_to_os_with_local_path(
                                session,
                                file,
                                Some(local_path),
                                design,
                                os_action,
                            )
                            .await;
                            Msg::FlowOsCompleted {
                                status_entry,
                                result,
                            }
                        }));
                    }
                }
                let Some(session) = self.os_session.clone() else {
                    return None; // opener is login-gated; belt and suspenders
                };
                let status_entry = self.push_tracked_line(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render(&format!("  ⧉ {file} → OS Workflow as a Service designer…")),
                );
                let local_path = path.clone();
                return Some(cmd::cmd(move || async move {
                    let result = publish_flow_to_os_with_local_path(
                        session,
                        file,
                        Some(local_path),
                        design,
                        FlowOsAction::Design,
                    )
                    .await;
                    Msg::FlowOsCompleted {
                        status_entry,
                        result,
                    }
                }));
            }
            _ => {}
        }
        None
    }

    pub(crate) fn handle_flow_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let panel_state = self.flow.as_ref()?;
        let total = panel_state.flows.len();
        if total == 0 {
            return None;
        }
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let selected = panel_state.sel.min(total - 1);
        let (mut panel, panel_height) = flow_picker_panel(
            &panel_state.flows,
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
            flow_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                if let Some(open) = self.flow.as_mut() {
                    open.sel = index.min(total - 1);
                }
                self.handle_flow_key(&KeyEvent {
                    code: KeyCode::Enter,
                    modifiers: a3s_tui::KeyModifiers::NONE,
                })
            }
            Some(MenuPanelMsg::Cancelled) => {
                cancel_pending_picker(&mut self.flow, &mut self.pending_flow_subcommand);
                None
            }
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    if let Some(open) = self.flow.as_mut() {
                        open.sel = after;
                    }
                }
                None
            }
        }
    }

    pub(crate) fn on_flow_os_completed(
        &mut self,
        status_entry: TranscriptEntryId,
        res: Result<FlowOsResult, String>,
    ) {
        match res {
            Ok(result) => {
                let lifecycle = asset_lifecycle::workflow_flow_lifecycle();
                debug_assert_eq!(lifecycle.family, "workflow flow");
                debug_assert_eq!(lifecycle.command, "/flow");
                debug_assert_eq!(lifecycle.os_category, "workflow");
                debug_assert_eq!(lifecycle.runtime_binding.kind, "workflow");
                debug_assert_eq!(lifecycle.runtime_binding.isolation, "native");
                debug_assert_eq!(lifecycle.runtime_binding.agent_kind, None);
                debug_assert_eq!(lifecycle.stages, asset_lifecycle::WORKFLOW_STAGES);
                let service = asset_lifecycle::service_label(lifecycle.service);
                self.last_view = Some(result.view.clone());
                self.replace_tracked_line(
                    status_entry,
                    &gutter(
                        TN_CYAN,
                        &format!(
                            "⧉ /flow {} · `{}` ({})",
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
                    let button = format!("{service} · click to reopen");
                    self.push_line(&gutter(ACCENT, &remote_view_button(&button)));
                    self.open_remote_view(&result.view);
                } else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  Open view opens the related OS workflow asset view"),
                    );
                }
            }
            Err(e) => {
                self.replace_tracked_line(
                    status_entry,
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  /flow OS operation failed: {e}")),
                );
            }
        }
    }

    /// Overlay the `/flow` picker above the input.
    pub(crate) fn overlay_flow_menu(&self, composed: String) -> String {
        let Some(p) = self.flow.as_ref() else {
            return composed;
        };
        let width = self.width as usize;
        let menu = flow_picker_lines(&p.flows, p.sel, &p.root, width, self.height as usize);
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
    fn lists_flow_json_sources_sorted_skipping_noncanonical_json() {
        let root = std::env::temp_dir().join(format!("a3s-flows-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("daily-report")).unwrap();
        std::fs::write(root.join("zeta.json"), "{}").unwrap();
        std::fs::write(root.join("alpha.json"), "{}").unwrap();
        std::fs::write(root.join("flow.json"), "{}").unwrap();
        std::fs::write(root.join("daily-report/flow.json"), "{}").unwrap();
        std::fs::write(root.join(".hidden.json"), "{}").unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        std::fs::create_dir_all(root.join("dir.json")).unwrap(); // dir, not a flow
        let fs = list_flows(&root);
        assert_eq!(fs, vec!["daily-report/flow.json", "flow.json"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn lists_visible_canonical_designer_documents_inside_asset_folders() {
        let root = std::env::temp_dir().join(format!("a3s-flow-assets-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        let asset_dir = root.join("daily-report");
        std::fs::create_dir_all(asset_dir.join(".a3s/private")).unwrap();
        std::fs::write(asset_dir.join("daily-report.json"), "{}").unwrap();
        std::fs::write(asset_dir.join("flow.json"), "{}").unwrap();
        std::fs::write(asset_dir.join(".a3s/private/debug.json"), "{}").unwrap();

        let fs = list_flows(&root);
        assert_eq!(fs, vec!["daily-report/flow.json"]);
        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn designer_url_and_asset_name_follow_the_rules() {
        // The standalone (chrome-less) designer page, not the /admin one.
        assert_eq!(
            designer_url("http://180.163.156.38:49164/", "abc-123"),
            "http://180.163.156.38:49164/workflow-designer/abc-123"
        );
        assert_eq!(flow_asset_name("Daily Report 2"), "flow-daily-report-2");
        assert_eq!(flow_asset_name("daily-report/flow"), "flow-daily-report");
    }

    #[test]
    fn existing_flow_asset_must_match_workflow_category() {
        let found = serde_json::json!({
            "data": {
                "items": [
                    {
                        "id": "knowledge-asset",
                        "name": "flow-daily-report",
                        "category": "knowledge"
                    }
                ]
            }
        });

        let err = find_flow_asset(&found, "flow-daily-report").unwrap_err();
        assert!(err.contains("category=knowledge"), "{err}");
        assert!(err.contains("expected workflow"), "{err}");
    }

    #[test]
    fn designer_view_spec_auto_opens_workflow_designer_size() {
        let spec = designer_view_spec(designer_url("https://os.example.com", "asset-1"));
        assert_eq!(spec.url, "https://os.example.com/workflow-designer/asset-1");
        assert_eq!((spec.width, spec.height), (Some(1440), Some(900)));
        assert!(spec.embeddable);
    }

    #[test]
    fn parses_flow_lifecycle_subcommands() {
        assert_eq!(
            parse_flow_subcommand("clone https://github.com/a/flow.git")
                .unwrap()
                .unwrap(),
            FlowSubcommand::Clone("https://github.com/a/flow.git".into())
        );
        assert_eq!(
            parse_flow_subcommand("publish").unwrap().unwrap(),
            FlowSubcommand::Publish
        );
        assert_eq!(
            parse_flow_subcommand("run").unwrap().unwrap(),
            FlowSubcommand::Run
        );
        assert_eq!(
            parse_flow_subcommand("deploy").unwrap().unwrap(),
            FlowSubcommand::Deploy
        );
        assert_eq!(
            parse_flow_subcommand("open").unwrap().unwrap(),
            FlowSubcommand::Open
        );
        assert_eq!(
            parse_flow_subcommand("logs").unwrap().unwrap(),
            FlowSubcommand::Logs
        );
        assert_eq!(
            parse_flow_subcommand("status").unwrap().unwrap(),
            FlowSubcommand::Status
        );
        assert_eq!(
            parse_flow_subcommand("activity failed jobs")
                .unwrap()
                .unwrap(),
            FlowSubcommand::Activity("failed jobs".into())
        );
        assert!(parse_flow_subcommand("ps").unwrap().is_err());
        assert!(parse_flow_subcommand("open now").unwrap().is_err());
        assert!(parse_flow_subcommand("logs now").unwrap().is_err());
        assert!(parse_flow_subcommand("run now").unwrap().is_err());
        assert!(parse_flow_subcommand("workflow").unwrap().is_err());
        assert!(parse_flow_subcommand("artifact").unwrap().is_err());
        assert!(parse_flow_subcommand("inspect").unwrap().is_err());
        assert_eq!(
            parse_flow_subcommand("debug").unwrap().unwrap_err(),
            "unknown /flow command `debug`"
        );
        assert!(parse_flow_subcommand("jobs").unwrap().is_err());
        assert!(parse_flow_subcommand("off").unwrap().is_err());
        assert!(parse_flow_subcommand("publish now").unwrap().is_err());
        for removed in ["view", "remote", "os", "dashboard"] {
            assert!(
                parse_flow_subcommand(removed).unwrap().is_err(),
                "/flow {removed} should not create a workflow prototype"
            );
        }
        assert!(parse_flow_subcommand("make a workflow").is_none());
    }

    #[test]
    fn workflow_package_metadata_carries_workflow_service_binding() {
        let design = serde_json::json!({
            "version": "a3s.workflow.design.v1",
            "name": "Daily digest",
            "description": "Collect and summarize daily signals",
            "nodes": [
                {"id": "start", "kind": "start"},
                {"id": "step-1", "kind": "llm"},
                {"id": "end", "kind": "end"}
            ],
            "edges": [
                {"id": "e1", "sourceNodeID": "start", "targetNodeID": "step-1"},
                {"id": "e2", "sourceNodeID": "step-1", "targetNodeID": "end"}
            ]
        });
        let manifest = workflow_manifest_json("flow-daily-digest", "daily-digest.json", &design);
        let binding =
            workflow_runtime_binding_json("flow-daily-digest", "daily-digest.json", &design);

        assert_eq!(manifest["category"], "workflow");
        assert_eq!(manifest["service"], "Workflow as a Service");
        assert_eq!(manifest["runtimeIntent"]["kind"], "workflow");
        assert_eq!(manifest["runtimeIntent"]["isolation"], "native");
        assert_eq!(
            manifest["runtimeIntent"]["runtimeKind"],
            "a3s-workflow-service"
        );
        assert_eq!(manifest["runtimeIntent"]["protocol"], "workflow");
        assert_eq!(manifest["designDocumentPath"], DESIGN_DOCUMENT_PATH);
        assert!(manifest.get("runtimeBindingPath").is_none());
        assert_eq!(manifest["graph"]["nodes"], 3);
        assert_eq!(binding["kind"], "workflow");
        assert_eq!(binding["isolation"], "native");
        assert_eq!(binding["runtime"]["kind"], "a3s-workflow-service");
        assert!(binding["runtime"].get("agentKind").is_none());
        let upsert = workflow_runtime_binding_upsert_body(&binding);
        assert_eq!(upsert["kind"], "workflow");
        assert_eq!(upsert["runtime"]["kind"], "a3s-workflow-service");
        assert!(upsert["runtime"].get("protocol").is_none());
        assert!(upsert["target"].get("designDocumentPath").is_none());
    }

    #[tokio::test]
    async fn publish_flow_to_os_uploads_design_and_syncs_workflow_service_binding() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_flow_publish_mock(captured.clone()).await;
        let root = std::env::temp_dir().join(format!("a3s-flow-publish-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let design = serde_json::json!({
            "version": "a3s.workflow.design.v1",
            "name": "Daily digest",
            "description": "Collect and summarize daily signals",
            "nodes": [
                {"id": "start", "kind": "start"},
                {"id": "step-1", "kind": "llm"},
                {"id": "end", "kind": "end"}
            ],
            "edges": [
                {"id": "e1", "sourceNodeID": "start", "targetNodeID": "step-1"},
                {"id": "e2", "sourceNodeID": "step-1", "targetNodeID": "end"}
            ]
        })
        .to_string();
        let local_file = root.join("flow.json");
        std::fs::write(&local_file, &design).unwrap();

        let result = publish_flow_to_os_with_local_path(
            session,
            "flow.json".into(),
            Some(local_file),
            design,
            FlowOsAction::Publish,
        )
        .await
        .expect("flow publish should use OS workflow asset APIs");

        assert_eq!(result.asset_name, "flow-daily-digest");
        assert_eq!(result.asset_id, "workflow-asset-1");
        assert!(
            result.note.contains("Workflow as a Service"),
            "{}",
            result.note
        );
        assert!(
            result.note.contains("runtime binding was synced"),
            "{}",
            result.note
        );
        let local_asset_acl = root.join(asset_lifecycle::ASSET_ACL_PATH);
        assert!(local_asset_acl.is_file(), "missing local asset.acl");
        let local_asset_acl_body = std::fs::read_to_string(&local_asset_acl).unwrap();
        assert!(local_asset_acl_body.contains("category = \"workflow\""));
        assert!(local_asset_acl_body.contains("design_document_path"));

        let requests = captured.lock().unwrap().clone();
        let joined = requests.join("\n");
        assert!(joined.contains("GET /api/v1/assets?"), "{joined}");
        assert!(joined.contains("POST /api/v1/assets HTTP/1.1"), "{joined}");
        assert!(joined.contains(r#""name":"flow-daily-digest""#), "{joined}");
        assert!(!joined.contains(r#""name":"flow-flow""#), "{joined}");
        assert!(
            joined.contains("POST /api/v1/assets/workflow-asset-1/repository/files HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined.contains("PUT /api/v1/assets/workflow-asset-1/runtime-binding HTTP/1.1"),
            "{joined}"
        );
        assert!(
            joined
                .contains("POST /api/v1/assets/workflow-asset-1/runtime-binding/validate HTTP/1.1"),
            "{joined}"
        );

        let create = request_body(&requests, "POST /api/v1/assets HTTP/1.1");
        let create_json: serde_json::Value = serde_json::from_str(&create).unwrap();
        assert_eq!(create_json["category"], "workflow");
        assert_eq!(create_json["metadata"]["service"], "Workflow as a Service");
        assert_eq!(
            create_json["metadata"]["runtimeKind"],
            "a3s-workflow-service"
        );
        assert_eq!(create_json["metadata"]["protocol"], "workflow");
        assert_eq!(create_json["metadata"]["createdBy"], "a3s-code-tui");

        let upload = request_body(
            &requests,
            "POST /api/v1/assets/workflow-asset-1/repository/files HTTP/1.1",
        );
        let upload_json: serde_json::Value = serde_json::from_str(&upload).unwrap();
        let files = upload_json["files"].as_array().unwrap();
        assert!(files
            .iter()
            .any(|file| file["path"] == DESIGN_DOCUMENT_PATH));
        assert!(files
            .iter()
            .any(|file| file["path"] == asset_lifecycle::ASSET_ACL_PATH));
        for forbidden in [
            "workflow.asset.json",
            "workflow.runtime-binding.json",
            "runtime-binding.json",
        ] {
            assert!(
                files.iter().all(|file| file["path"] != forbidden),
                "repository upload should not include {forbidden}"
            );
        }

        let synced = request_body(
            &requests,
            "PUT /api/v1/assets/workflow-asset-1/runtime-binding HTTP/1.1",
        );
        let synced_json: serde_json::Value = serde_json::from_str(&synced).unwrap();
        assert_eq!(synced_json["kind"], "workflow");
        assert_eq!(synced_json["runtime"]["kind"], "a3s-workflow-service");
        assert!(synced_json["runtime"].get("protocol").is_none());

        let _ = std::fs::remove_dir_all(&root);
    }

    #[tokio::test]
    async fn design_flow_action_publishes_then_opens_workflow_designer() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_flow_publish_mock(captured.clone()).await;
        let session = crate::a3s_os::StoredOsSession {
            address: origin.clone(),
            access_token: "token".into(),
            refresh_token: None,
            token_type: Some("Bearer".into()),
            expires_at_ms: None,
            account_label: None,
            login_at_ms: 1,
        };
        let design = serde_json::json!({
            "version": "a3s.workflow.design.v1",
            "name": "Daily digest",
            "description": "Collect and summarize daily signals",
            "nodes": [
                {"id": "start", "kind": "start"},
                {"id": "end", "kind": "end"}
            ],
            "edges": [
                {"id": "e1", "sourceNodeID": "start", "targetNodeID": "end"}
            ]
        })
        .to_string();

        let result = publish_flow_to_os(
            session,
            "daily-digest.json".into(),
            design,
            FlowOsAction::Design,
        )
        .await
        .expect("bare /flow should publish then open the designer");

        assert_eq!(result.action, FlowOsAction::Design);
        assert_eq!(
            result.view.url,
            format!("{origin}/workflow-designer/workflow-asset-1")
        );
        assert!(result.note.contains("for editing"), "{}", result.note);
        assert!(!result.note.contains("for `run`"), "{}", result.note);
        let requests = captured.lock().unwrap().join("\n");
        assert!(
            requests.contains("POST /api/v1/assets/workflow-asset-1/repository/files HTTP/1.1"),
            "{requests}"
        );
        assert!(
            requests.contains("PUT /api/v1/assets/workflow-asset-1/runtime-binding HTTP/1.1"),
            "{requests}"
        );
    }

    #[tokio::test]
    async fn run_and_deploy_flow_actions_publish_and_open_workflow_surface() {
        for action in [FlowOsAction::Run, FlowOsAction::Deploy] {
            let captured = Arc::new(Mutex::new(Vec::new()));
            let origin = spawn_flow_publish_mock(captured.clone()).await;
            let session = crate::a3s_os::StoredOsSession {
                address: origin.clone(),
                access_token: "token".into(),
                refresh_token: None,
                token_type: Some("Bearer".into()),
                expires_at_ms: None,
                account_label: None,
                login_at_ms: 1,
            };
            let design = serde_json::json!({
                "version": "a3s.workflow.design.v1",
                "name": "Daily digest",
                "description": "Collect and summarize daily signals",
                "nodes": [
                    {"id": "start", "kind": "start"},
                    {"id": "end", "kind": "end"}
                ],
                "edges": [
                    {"id": "e1", "sourceNodeID": "start", "targetNodeID": "end"}
                ]
            })
            .to_string();

            let result = publish_flow_to_os(session, "daily-digest.json".into(), design, action)
                .await
                .unwrap_or_else(|err| {
                    panic!(
                        "flow {} should publish and open lifecycle surface: {err}",
                        action.label()
                    )
                });

            assert_eq!(result.action, action);
            assert!(result.open_view);
            match action {
                FlowOsAction::Run => {
                    assert_eq!(
                        result.view.url,
                        format!("{origin}/admin/workflows/workflow-asset-1/runs/run-1?embed=1")
                    );
                    assert!(
                        result.note.contains("runAssetWorkflow")
                            && result.note.contains("runtime binding was synced"),
                        "{}",
                        result.note
                    );
                }
                FlowOsAction::Deploy => {
                    assert_eq!(
                        result.view.url,
                        format!("{origin}/workflow-designer/workflow-asset-1")
                    );
                    assert!(
                        result
                            .note
                            .contains("no separate workflow deploy capability")
                            && result.note.contains("runtime binding was synced"),
                        "{}",
                        result.note
                    );
                }
                _ => unreachable!(),
            }

            let requests = captured.lock().unwrap().join("\n");
            assert!(
                requests.contains("POST /api/v1/assets/workflow-asset-1/repository/files HTTP/1.1"),
                "{requests}"
            );
            assert!(
                requests.contains("PUT /api/v1/assets/workflow-asset-1/runtime-binding HTTP/1.1"),
                "{requests}"
            );
            assert!(
                requests.contains(
                    "POST /api/v1/assets/workflow-asset-1/runtime-binding/validate HTTP/1.1"
                ),
                "{requests}"
            );
        }
    }

    #[test]
    fn flow_progressive_score_prefers_workflow_designer_viewlink() {
        let value = serde_json::json!({
            "data": {
                "results": [
                    {
                        "name": "FunctionController_batch",
                        "resource": "functions.batch",
                        "description": "Function as a Service batch MCP tools"
                    },
                    {
                        "name": "WorkflowDesignerController_open",
                        "resource": "workflows.designer",
                        "path": "/api/v1/workflows/{assetId}/designer",
                        "description": "Workflow as a Service designer ViewLink for a workflow asset"
                    }
                ]
            }
        });
        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            flow_progressive_score(FlowProgressiveIntent::Designer, text, operation)
        });
        assert_eq!(candidates[0].module, "workflows");
        assert_eq!(candidates[0].operation, "WorkflowDesignerController_open");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.module != "functions"),
            "Function/MCP candidates must not drive /flow ViewLink selection: {candidates:?}"
        );
        assert!(
            flow_progressive_score(
                FlowProgressiveIntent::Designer,
                "Workflow as a Service Designer ViewLink",
                "WorkflowDesignerController_open"
            ) > 0
        );
        assert_eq!(
            flow_progressive_score(
                FlowProgressiveIntent::Designer,
                "Function as a Service MCP ViewLink",
                "FunctionController_open"
            ),
            0
        );
        assert_eq!(
            flow_progressive_score(
                FlowProgressiveIntent::Designer,
                "Workflow as a Service preview with a shaped view",
                "WorkflowDataMappingController_preview"
            ),
            0,
            "/flow open must not choose preview operations just because they contain `view`"
        );
    }

    #[test]
    fn flow_progressive_score_keeps_logs_separate_from_designer_views() {
        let value = serde_json::json!({
            "data": {
                "results": [
                    {
                        "name": "WorkflowDesignerController_open",
                        "resource": "workflows.designer",
                        "description": "Workflow as a Service designer ViewLink"
                    },
                    {
                        "name": "WorkflowRunLogController_open",
                        "resource": "workflows.logs",
                        "description": "Workflow as a Service logs jobs run history shaped ViewLink"
                    },
                    {
                        "name": "AssetActionsController_getJobLogs",
                        "resource": "assets.actions.runs.jobs",
                        "path": "/api/v1/assets/{assetId}/actions/runs/{runId}/jobs/{jobId}/logs",
                        "description": "获取 Actions job 日志"
                    }
                ]
            }
        });
        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            flow_progressive_score(FlowProgressiveIntent::Logs, text, operation)
        });

        assert_eq!(candidates[0].operation, "WorkflowRunLogController_open");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "WorkflowDesignerController_open"),
            "designer candidates must not drive /flow logs: {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "AssetActionsController_getJobLogs"),
            "logs candidates that require runId/jobId must not drive /flow logs: {candidates:?}"
        );
    }

    #[test]
    fn flow_progressive_score_prefers_exact_workflow_run_operation() {
        let value = serde_json::json!({
            "data": {
                "results": [
                    {
                        "name": "createAsset",
                        "module": "assets",
                        "path": "/api/v1/assets",
                        "description": "创建资产仓库"
                    },
                    {
                        "name": "runAssetWorkflow",
                        "module": "runtimes",
                        "path": "/api/v1/runtimes/asset-workflows/{assetId}/run",
                        "description": "运行资产工作流"
                    },
                    {
                        "name": "AssetWorkflowExecutionController_testNode",
                        "module": "runtimes",
                        "path": "/api/v1/runtimes/asset-workflows/test-node",
                        "description": "单节点测试运行"
                    }
                ]
            }
        });
        let candidates = os_progressive::operation_candidates(&value, |text, operation| {
            flow_progressive_score(FlowProgressiveIntent::Run, text, operation)
        });
        assert_eq!(candidates[0].operation, "runAssetWorkflow");
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "createAsset"),
            "generic asset create must not drive /flow run: {candidates:?}"
        );
        assert!(
            candidates
                .iter()
                .all(|candidate| candidate.operation != "AssetWorkflowExecutionController_testNode"),
            "node tests are not full /flow run: {candidates:?}"
        );
    }

    #[test]
    fn flow_picker_lines_use_bounded_shared_menu_rows() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/flows/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let flows = vec![
            "very-long-workflow-file-name-that-would-overflow-the-panel.json".to_string(),
            "daily-news.json".to_string(),
        ];
        let lines = flow_picker_lines(&flows, 0, &root, 40, 20);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("flow"), "{plain}");
        assert!(plain.contains("select a DAG"), "{plain}");
        assert!(plain.contains("workflow-file-name"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn flow_picker_lines_scroll_selected_flow_into_view() {
        let root = std::path::PathBuf::from("/tmp/flows");
        let flows = (0..16)
            .map(|index| format!("flow-{index}.json"))
            .collect::<Vec<_>>();
        let plain = flow_picker_lines(&flows, 14, &root, 48, 16)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("flow-14.json"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn flow_picker_header_and_hint_fit_fixed_width() {
        let root = std::path::PathBuf::from(
            "/Users/example/.a3s/flows/a/path/that/is/far/too/long/for/a/picker/header",
        );
        let header = flow_picker_header(9, &root, 40);
        let hint = flow_picker_hint(40);
        assert!(a3s_tui::style::visible_len(&header) <= 40, "{header}");
        assert!(a3s_tui::style::visible_len(&hint) <= 40, "{hint}");
    }

    #[test]
    fn flow_picker_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let root = std::path::PathBuf::from("/tmp/flows");
        let flows = (0..4)
            .map(|index| format!("flow-{index}.json"))
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = flow_picker_lines(&flows, 0, &root, width, height).len();
        let y_offset = flow_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = flow_picker_panel(&flows, 0, &root, width, height).expect("panel");
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
    fn flow_picker_click_selects_visible_row_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let root = std::path::PathBuf::from("/tmp/flows");
        let flows = (0..4)
            .map(|index| format!("flow-{index}.json"))
            .collect::<Vec<_>>();
        let width = 48;
        let height = 18;
        let row_count = flow_picker_lines(&flows, 0, &root, width, height).len();
        let y_offset = flow_overlay_y_offset(height, row_count, 5);
        let (mut panel, _) = flow_picker_panel(&flows, 0, &root, width, height).expect("panel");
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
    fn flow_overlay_offset_moves_up_with_more_rows_below() {
        assert!(flow_overlay_y_offset(24, 8, 7) < flow_overlay_y_offset(24, 8, 5));
    }

    #[test]
    fn flow_gen_prompt_carries_schema_rules_and_dir() {
        let p = flow_gen_prompt("fetch news daily and summarize", "/Users/x/.a3s/flows");
        assert!(p.contains("fetch news daily and summarize"));
        assert!(p.contains("/Users/x/.a3s/flows"));
        assert!(p.contains("a3s.workflow.design.v1")); // the schema anchor
        assert!(p.contains("sourceNodeID")); // capital-D edge fields
        assert!(p.contains("exactly one `start` and one `end`"));
        assert!(p.contains("OUTSIDE this session's workspace") && p.contains("bash"));
        assert!(p.contains(".a3s/asset.acl"));
        assert!(p.contains("Do NOT create"));
        assert!(p.contains("extra generated JSON config files"));
        assert!(p.contains("keep package configuration in `.a3s/asset.acl`"));
        assert!(p.contains("MUST stay at the asset root"));
        assert!(p.contains("do NOT put workflow"));
        assert!(p.contains("service=Workflow as a Service"));
        assert!(p.contains("runtimeIntent.kind=workflow"));
        assert!(p.contains("runtime.kind=a3s-workflow-service"));
        assert!(p.contains("protocol=workflow"));
        // The example block itself must be valid JSON (models copy it).
        let start = p.find("{\"version\"").expect("example present");
        let end = p[start..].find("}]}").expect("example closes") + start + 3;
        assert!(serde_json::from_str::<serde_json::Value>(&p[start..end]).is_ok());
    }

    #[test]
    fn scaffold_flow_asset_creates_root_design_and_metadata_acl() {
        let root =
            std::env::temp_dir().join(format!("a3s-code-flow-scaffold-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).unwrap();

        let design_path = scaffold_flow_asset(
            "Name it exactly triage-flow. It triages incoming tickets.",
            &root,
        )
        .unwrap();
        let package = design_path.parent().unwrap();

        for rel in ["README.md", "flow.json", "tests/smoke.md", ".a3s/asset.acl"] {
            assert!(package.join(rel).is_file(), "missing {rel}");
        }
        for rel in [
            "workflow.asset.json",
            "workflow.runtime-binding.json",
            ".a3s/workflow.asset.json",
            ".a3s/workflow.runtime-binding.json",
        ] {
            assert!(!package.join(rel).exists(), "unexpected {rel}");
        }
        assert!(!package.join(".a3s").join("workflows").exists());
        let flows = list_flows(&root);
        assert_eq!(flows, vec!["triage-flow/flow.json"]);
        let asset_acl =
            std::fs::read_to_string(package.join(asset_lifecycle::ASSET_ACL_PATH)).unwrap();
        assert!(asset_acl.contains("category = \"workflow\""));
        assert!(asset_acl.contains("design_document_path = \"flow.json\""));

        let _ = std::fs::remove_dir_all(&root);
    }

    #[test]
    fn items_of_reads_paginated_and_bare_shapes() {
        let paged: serde_json::Value =
            serde_json::json!({"code":200,"data":{"items":[{"id":"a"}],"total":1}});
        assert_eq!(items_of(&paged).len(), 1);
        let bare: serde_json::Value = serde_json::json!({"data":[{"id":"b"}]});
        assert_eq!(items_of(&bare).len(), 1);
        assert!(items_of(&serde_json::json!({"data":{}})).is_empty());
    }

    async fn spawn_flow_publish_mock(captured: Arc<Mutex<Vec<String>>>) -> String {
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
                    let (status, payload) = flow_publish_mock_response(&line, &body);
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

    fn flow_publish_mock_response(line: &str, body: &str) -> (&'static str, &'static str) {
        if line.starts_with("POST /api/v1/kernel/capabilities HTTP/1.1") {
            if body.contains(r#""action":"search""#)
                && body.contains("Workflow as a Service run workflow asset shaped ViewLink")
            {
                return (
                    "200 OK",
                    r#"{"data":{"data":[{"name":"createAsset","module":"assets","path":"/api/v1/assets","description":"创建资产仓库"},{"name":"runAssetWorkflow","module":"runtimes","path":"/api/v1/runtimes/asset-workflows/{assetId}/run","description":"运行资产工作流"},{"name":"AssetWorkflowExecutionController_testNode","module":"runtimes","path":"/api/v1/runtimes/asset-workflows/test-node","description":"单节点测试运行"}]}}"#,
                );
            }
            if body.contains(r#""action":"describe""#) && body.contains("runAssetWorkflow") {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"operation":{"name":"runAssetWorkflow","inputSchema":{"body":{"properties":{"assetId":{"type":"string"},"input":{"type":"object"},"idempotencyKey":{"type":"string"}}}}}}}"#,
                );
            }
            if body.contains(r#""action":"execute""#)
                && body.contains(r#""shaped":true"#)
                && body.contains("runAssetWorkflow")
                && body.contains(r#""assetId":"workflow-asset-1""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"executionId":"run-1"},"view":{"url":"/admin/workflows/workflow-asset-1/runs/run-1?embed=1","width":1280,"height":860}}"#,
                );
            }
            return ("404 Not Found", r#"{"code":404,"message":"not found"}"#);
        }
        if line.starts_with("GET /api/v1/assets?") {
            return ("200 OK", r#"{"data":{"items":[]}}"#);
        }
        if line.starts_with("POST /api/v1/assets HTTP/1.1") {
            if body.contains(r#""category":"workflow""#)
                && body.contains(r#""service":"Workflow as a Service""#)
                && body.contains(r#""runtimeKind":"a3s-workflow-service""#)
                && body.contains(r#""protocol":"workflow""#)
                && body.contains(r#""createdBy":"a3s-code-tui""#)
            {
                return (
                    "200 OK",
                    r#"{"data":{"id":"workflow-asset-1","name":"flow-daily-digest"}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad workflow asset body"}"#,
            );
        }
        if line.starts_with("POST /api/v1/assets/workflow-asset-1/repository/files HTTP/1.1") {
            if body.contains(DESIGN_DOCUMENT_PATH)
                && body.contains(asset_lifecycle::ASSET_ACL_PATH)
                && !body.contains("workflow.asset.json")
                && !body.contains("workflow.runtime-binding.json")
            {
                return ("200 OK", r#"{"ok":true}"#);
            }
            return (
                "422 Unprocessable Entity",
                r#"{"message":"bad workflow upload body"}"#,
            );
        }
        if line.starts_with("PUT /api/v1/assets/workflow-asset-1/runtime-binding HTTP/1.1") {
            if body.contains(r#""kind":"workflow""#)
                && body.contains(r#""a3s-workflow-service""#)
                && !body.contains(r#""version""#)
                && !body.contains(r#""protocol":"workflow""#)
            {
                return (
                    "200 OK",
                    r#"{"code":200,"data":{"assetId":"workflow-asset-1","configured":true}}"#,
                );
            }
            return (
                "422 Unprocessable Entity",
                r#"{"code":422,"message":"bad runtime binding"}"#,
            );
        }
        if line
            .starts_with("POST /api/v1/assets/workflow-asset-1/runtime-binding/validate HTTP/1.1")
        {
            return (
                "200 OK",
                r#"{"code":200,"data":{"assetId":"workflow-asset-1","configured":true,"valid":true,"issues":[]}}"#,
            );
        }
        ("404 Not Found", r#"{"code":404,"message":"not found"}"#)
    }
}
