use std::path::Path;

use serde_json::json;

use crate::commands::code::asset_types::AssetCommandOutput;
use crate::tui::{panels, remote_ui};

pub(crate) fn os_asset_category_query(category: &str, query: &str) -> String {
    let query = query.trim();
    if query.is_empty() {
        format!("category:{category}")
    } else {
        format!("category:{category} {query}")
    }
}

pub(crate) fn runtime_asset_query(category: &str, asset_hint: &str, query: &str) -> String {
    let category = category.trim();
    let asset_hint = asset_hint.trim();
    let query = query.trim();
    let mut parts = Vec::new();
    if !category.is_empty() {
        parts.push(format!("category:{category}"));
    }
    if !asset_hint.is_empty() {
        parts.push(asset_hint.to_string());
    }
    if !query.is_empty() {
        parts.push(query.to_string());
    }
    parts.join(" ")
}

pub(super) fn local_agents_output(query: &str, root: &Path) -> AssetCommandOutput {
    let rows = panels::agent::list_agents(root)
        .into_iter()
        .filter(|row| matches_query(&row.rel, query))
        .collect::<Vec<_>>();
    let mut human = format!(
        "{} local agent package(s) in {}\n",
        rows.len(),
        root.display()
    );
    for row in &rows {
        human.push_str(&format!(
            "{}\t{}\t{}\n",
            row.rel,
            row.definition_rel,
            row.path.display()
        ));
    }
    let assets = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.rel,
                "path": path_value(&row.path),
                "definition": row.definition_rel,
                "definitionPath": path_value(&row.definition_path),
            })
        })
        .collect::<Vec<_>>();
    local_list_output("agent", query, root, assets, human)
}

pub(super) fn local_mcps_output(query: &str, root: &Path) -> AssetCommandOutput {
    let rows = panels::mcp::list_mcp_projects(root)
        .into_iter()
        .filter(|row| matches_query(&format!("{} {}", row.rel, row.name), query))
        .collect::<Vec<_>>();
    let mut human = format!("{} local MCP asset(s) in {}\n", rows.len(), root.display());
    for row in &rows {
        human.push_str(&format!(
            "{}\t{}\t{}\n",
            row.rel,
            row.name,
            row.path.display()
        ));
    }
    let assets = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.rel,
                "name": row.name,
                "description": row.description,
                "path": path_value(&row.path),
            })
        })
        .collect::<Vec<_>>();
    local_list_output("mcp", query, root, assets, human)
}

pub(super) fn local_skills_output(query: &str, root: &Path) -> AssetCommandOutput {
    let rows = panels::skill::list_skill_assets(root)
        .into_iter()
        .filter(|row| matches_query(&format!("{} {}", row.rel, row.name), query))
        .collect::<Vec<_>>();
    let mut human = format!(
        "{} local skill asset(s) in {}\n",
        rows.len(),
        root.display()
    );
    for row in &rows {
        human.push_str(&format!(
            "{}\t{}\t{}\n",
            row.rel,
            row.name,
            row.path.display()
        ));
    }
    let assets = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.rel,
                "name": row.name,
                "description": row.description,
                "path": path_value(&row.path),
            })
        })
        .collect::<Vec<_>>();
    local_list_output("skill", query, root, assets, human)
}

pub(super) fn local_flows_output(query: &str, root: &Path) -> AssetCommandOutput {
    let rows = panels::flow::list_flows(root)
        .into_iter()
        .filter(|row| matches_query(row, query))
        .collect::<Vec<_>>();
    let mut human = format!(
        "{} local workflow file(s) in {}\n",
        rows.len(),
        root.display()
    );
    for row in &rows {
        human.push_str(&format!("{}\t{}\n", row, root.join(row).display()));
    }
    let assets = rows
        .iter()
        .map(|row| json!({"id": row, "path": path_value(&root.join(row))}))
        .collect::<Vec<_>>();
    local_list_output("flow", query, root, assets, human)
}

pub(super) fn local_okf_output(query: &str, root: &Path) -> AssetCommandOutput {
    let rows = panels::okf::list_okf_packages(root)
        .into_iter()
        .filter(|row| matches_query(&format!("{} {}", row.rel, row.name), query))
        .collect::<Vec<_>>();
    let mut human = format!(
        "{} local OKF package(s) in {}\n",
        rows.len(),
        root.display()
    );
    for row in &rows {
        human.push_str(&format!(
            "{}\t{}\t{}\n",
            row.rel,
            row.name,
            row.path.display()
        ));
    }
    let assets = rows
        .iter()
        .map(|row| {
            json!({
                "id": row.rel,
                "name": row.name,
                "description": row.description,
                "path": path_value(&row.path),
            })
        })
        .collect::<Vec<_>>();
    local_list_output("okf", query, root, assets, human)
}

fn local_list_output(
    family: &'static str,
    query: &str,
    root: &Path,
    assets: Vec<serde_json::Value>,
    human: String,
) -> AssetCommandOutput {
    AssetCommandOutput::new(
        json!({
            "family": family,
            "location": "local",
            "query": query,
            "root": path_value(root),
            "assets": assets,
        }),
        human,
    )
}

pub(super) fn review_output(
    family: &'static str,
    path: &Path,
    prompt: String,
) -> AssetCommandOutput {
    let human = if prompt.ends_with('\n') {
        prompt.clone()
    } else {
        format!("{prompt}\n")
    };
    AssetCommandOutput::new(
        json!({
            "family": family,
            "path": path_value(path),
            "prompt": prompt,
        }),
        human,
    )
}

pub(super) fn path_value(path: &Path) -> serde_json::Value {
    if let Some(path) = path.to_str() {
        return serde_json::Value::String(path.to_string());
    }

    native_path_value(path)
}

#[cfg(unix)]
fn native_path_value(path: &Path) -> serde_json::Value {
    use std::os::unix::ffi::OsStrExt;

    let value = path
        .as_os_str()
        .as_bytes()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    json!({
        "display": path.to_string_lossy(),
        "encoding": "unix-bytes-hex",
        "value": value,
    })
}

#[cfg(windows)]
fn native_path_value(path: &Path) -> serde_json::Value {
    use std::os::windows::ffi::OsStrExt;

    let value = path
        .as_os_str()
        .encode_wide()
        .map(|unit| format!("{unit:04x}"))
        .collect::<String>();
    json!({
        "display": path.to_string_lossy(),
        "encoding": "windows-wide-hex",
        "value": value,
    })
}

#[cfg(not(any(unix, windows)))]
fn native_path_value(path: &Path) -> serde_json::Value {
    json!({
        "display": path.to_string_lossy(),
        "encoding": "platform-native-lossy",
    })
}

fn matches_query(text: &str, query: &str) -> bool {
    let query = query.trim().to_ascii_lowercase();
    query.is_empty() || text.to_ascii_lowercase().contains(&query)
}

pub(super) fn os_note(note: &str, view: &remote_ui::ViewSpec, opened_with: Option<&str>) -> String {
    let mut human = format!("{note}\nview: {}\n", view.url);
    if let Some(width) = view.width {
        human.push_str(&format!("width: {width}"));
        if let Some(height) = view.height {
            human.push_str(&format!(", height: {height}"));
        }
        human.push('\n');
    }
    if let Some(opened_with) = opened_with {
        human.push_str(&format!("opened: {opened_with}\n"));
    }
    human
}

pub(super) fn open_if_requested(
    open_requested: bool,
    view: &remote_ui::ViewSpec,
) -> anyhow::Result<Option<String>> {
    if !open_requested {
        return Ok(None);
    }
    let opened = remote_ui::open_window(view)
        .map_err(|e| anyhow::anyhow!("could not open RemoteUI view: {e}"))?;
    let opened = match opened {
        remote_ui::OpenedWith::Webview => "webview",
        remote_ui::OpenedWith::Browser => "browser",
    };
    Ok(Some(opened.to_string()))
}

#[allow(clippy::too_many_arguments)]
pub(super) fn lifecycle_data(
    family: &'static str,
    action: &'static str,
    kind: Option<&'static str>,
    asset_name: &str,
    asset_id: &str,
    note: &str,
    view: &remote_ui::ViewSpec,
    opened_with: Option<String>,
) -> serde_json::Value {
    json!({
        "family": family,
        "action": action,
        "kind": kind,
        "assetName": asset_name,
        "assetId": asset_id,
        "note": note,
        "view": {
            "url": view.url,
            "width": view.width,
            "height": view.height,
            "embeddable": view.embeddable,
        },
        "openedWith": opened_with,
    })
}

pub(super) fn trim_col(value: &str, width: usize) -> String {
    let mut out = value.chars().take(width).collect::<String>();
    if value.chars().count() > width && width >= 1 {
        out.pop();
        out.push('~');
    }
    out
}
