//! Asset-scoped OS resource panels.
//!
//! Asset-scoped list subcommands show the signed-in user's OS digital assets
//! (agents, workflows, applications, etc.) through the assets REST API.
//! Asset-scoped runtime activity subcommands show Runtime activity rows
//! through a small set of endpoint candidates, parsed leniently so the panel
//! survives OS API shape drift.

use super::super::*;
use a3s_tui::components::{divider_line_with, DetailPanel, DetailRow, MenuItem, MenuPanel};
use serde_json::Value;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) category: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) visibility: String,
    pub(crate) owner: String,
    pub(crate) updated: String,
    pub(crate) access_url: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeActivityRow {
    pub(crate) id: String,
    pub(crate) name: String,
    pub(crate) asset_category: String,
    pub(crate) kind: String,
    pub(crate) status: String,
    pub(crate) image: String,
    pub(crate) access_url: Option<String>,
    pub(crate) updated: String,
    pub(crate) source: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AssetListFetch {
    pub(crate) rows: Vec<AssetRow>,
    pub(crate) note: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct RuntimeActivityFetch {
    pub(crate) rows: Vec<RuntimeActivityRow>,
    pub(crate) note: String,
}

pub(crate) struct AssetListPanel {
    pub(crate) rows: Vec<AssetRow>,
    pub(crate) sel: usize,
    pub(crate) scroll: usize,
    pub(crate) category: Option<String>,
    pub(crate) query: String,
    pub(crate) searching: bool,
    pub(crate) loading: bool,
    pub(crate) note: String,
}

pub(crate) struct RuntimeActivityPanel {
    pub(crate) rows: Vec<RuntimeActivityRow>,
    pub(crate) sel: usize,
    pub(crate) scroll: usize,
    pub(crate) category: Option<String>,
    pub(crate) query: String,
    pub(crate) searching: bool,
    pub(crate) loading: bool,
    pub(crate) note: String,
}

struct ResourcePanelRender<'a> {
    header: &'a str,
    query: &'a str,
    searching: bool,
    note: &'a str,
    sel: usize,
    scroll: usize,
    total: usize,
    hint: &'a str,
}

fn resource_line(rendered: &str, width: usize) -> String {
    if width == 0 {
        String::new()
    } else {
        pad_to(&truncate(rendered, width), width)
    }
}

fn resource_columns(width: usize) -> (usize, usize, usize) {
    if width == 0 {
        return (0, 0, 0);
    }
    let sep_w = if width >= 3 { 3 } else { 0 };
    let available = width.saturating_sub(sep_w);
    let left = if available < 64 {
        available / 2
    } else {
        (width / 2).clamp(34, 70).min(available.saturating_sub(24))
    };
    let right = available.saturating_sub(left);
    (left, right, sep_w)
}

fn runtime_activity_hint() -> &'static str {
    "↑↓/jk select · / search · Enter/o open selected · r refresh · Esc close"
}

fn http() -> Result<reqwest::Client, String> {
    let builder = reqwest::Client::builder().timeout(std::time::Duration::from_secs(30));
    #[cfg(test)]
    let builder = builder.no_proxy();
    builder.build().map_err(|e| e.to_string())
}

fn with_query(origin: &str, path: &str, query: &str) -> Result<reqwest::Url, String> {
    let mut url = reqwest::Url::parse(&format!("{}{}", origin.trim_end_matches('/'), path))
        .map_err(|e| e.to_string())?;
    let has_limit = url.query_pairs().any(|(key, _)| key == "limit");
    {
        let mut pairs = url.query_pairs_mut();
        if !has_limit {
            pairs.append_pair("limit", "100");
        }
        if !query.trim().is_empty() {
            pairs.append_pair("search", query.trim());
        }
    }
    Ok(url)
}

fn with_asset_query(origin: &str, query: &str) -> Result<reqwest::Url, String> {
    let (category, search) = asset_query_parts(query);
    let mut url = with_query(origin, "/api/v1/assets", &search)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("scope", "mine");
        pairs.append_pair("status", "all");
        if let Some(category) = category {
            pairs.append_pair("category", &category);
        }
    }
    Ok(url)
}

fn with_runtime_query(origin: &str, path: &str, query: &str) -> Result<reqwest::Url, String> {
    let (_, search) = activity_query_parts(query);
    with_query(origin, path, &search)
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

fn envelope_data(v: &Value) -> &Value {
    v.get("data").unwrap_or(v)
}

fn items_of(v: &Value) -> Vec<Value> {
    let data = envelope_data(v);
    for ptr in [
        "/items",
        "/list",
        "/results",
        "/rows",
        "/assets",
        "/services",
        "/deployments",
        "/processes",
        "/invocations",
        "/batches",
    ] {
        if let Some(items) = data.pointer(ptr).and_then(|a| a.as_array()) {
            return items
                .iter()
                .filter(|item| item.is_object())
                .cloned()
                .collect();
        }
    }
    if let Some(items) = data.as_array() {
        return items
            .iter()
            .filter(|item| item.is_object())
            .cloned()
            .collect();
    }
    Vec::new()
}

fn str_at<'a>(v: &'a Value, keys: &[&str]) -> Option<&'a str> {
    for key in keys {
        if let Some(s) = v
            .get(*key)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(s);
        }
    }
    None
}

fn nested_str_at<'a>(v: &'a Value, paths: &[&str]) -> Option<&'a str> {
    for path in paths {
        if let Some(s) = v
            .pointer(path)
            .and_then(|x| x.as_str())
            .filter(|s| !s.is_empty())
        {
            return Some(s);
        }
    }
    None
}

fn bool_flag(v: &Value, keys: &[&str]) -> Option<bool> {
    for key in keys {
        if let Some(b) = v.get(*key).and_then(|x| x.as_bool()) {
            return Some(b);
        }
    }
    None
}

fn short_time(s: &str) -> String {
    s.replace('T', " ")
        .trim_end_matches('Z')
        .chars()
        .take(19)
        .collect()
}

fn asset_from_value(v: &Value) -> Option<AssetRow> {
    let id = str_at(v, &["id", "_id", "uuid", "assetId", "ref"])
        .or_else(|| str_at(v, &["name"]))
        .unwrap_or("asset")
        .to_string();
    let name = str_at(v, &["name", "title", "displayName", "label"])
        .unwrap_or(&id)
        .to_string();
    let category = str_at(v, &["category", "assetCategory", "type", "kind"])
        .unwrap_or("asset")
        .to_string();
    let kind = str_at(v, &["agentKind", "appKind", "assetKind", "runtimeKind"])
        .unwrap_or("")
        .to_string();
    let status = str_at(v, &["status", "state", "phase", "publishStatus"])
        .map(str::to_string)
        .or_else(|| {
            bool_flag(v, &["published", "isPublished"])
                .map(|b| if b { "published" } else { "draft" }.to_string())
        })
        .unwrap_or_else(|| "available".to_string());
    let visibility = str_at(v, &["visibility", "scope"])
        .unwrap_or("")
        .to_string();
    let owner = nested_str_at(v, &["/owner/name", "/owner/displayName", "/owner/id"])
        .or_else(|| str_at(v, &["ownerName", "createdBy", "userId"]))
        .unwrap_or("")
        .to_string();
    let updated = str_at(v, &["updatedAt", "modifiedAt", "createdAt", "lastSeenAt"])
        .map(short_time)
        .unwrap_or_default();
    let access_url = str_at(
        v,
        &["accessUrl", "gatewayUrl", "publicUrl", "url", "endpoint"],
    )
    .map(str::to_string);
    Some(AssetRow {
        id,
        name,
        category,
        kind,
        status,
        visibility,
        owner,
        updated,
        access_url,
    })
}

fn inferred_activity_category(v: &Value) -> Option<&'static str> {
    let runtime_plane = str_at(v, &["runtimePlane"])
        .unwrap_or("")
        .to_ascii_lowercase();
    let row_source = str_at(v, &["source"]).unwrap_or("").to_ascii_lowercase();
    let target_kind = str_at(v, &["targetKind"])
        .or_else(|| nested_str_at(v, &["/metadata/targetKind"]))
        .unwrap_or("")
        .to_ascii_lowercase();
    let kind = str_at(v, &["kind", "type", "resourceType", "runtimeKind"])
        .unwrap_or("")
        .to_ascii_lowercase();
    let protocol = nested_str_at(v, &["/metadata/protocol", "/runtime/protocol"])
        .unwrap_or("")
        .to_ascii_lowercase();

    if target_kind == "workflow"
        || kind == "workflow"
        || runtime_plane == "workflow-core"
        || row_source == "workflow-execution"
    {
        return Some("workflow");
    }
    if protocol == "mcp" || target_kind == "mcp" || kind == "mcp" {
        return Some("mcp");
    }
    if protocol == "skill" || target_kind == "skill" || kind == "skill" {
        return Some("skill");
    }
    if target_kind == "agent"
        || target_kind == "agent-session"
        || kind == "agent"
        || kind == "agentic"
        || kind == "tool"
        || kind == "agent-session"
        || runtime_plane == "agent-core"
        || row_source == "kernel-session-runtime"
    {
        return Some("agent");
    }
    None
}

fn activity_from_value(v: &Value, source: &str) -> Option<RuntimeActivityRow> {
    let id = str_at(
        v,
        &[
            "id",
            "_id",
            "uuid",
            "serviceId",
            "deploymentId",
            "invocationId",
            "batchId",
            "name",
        ],
    )?
    .to_string();
    let name = str_at(
        v,
        &[
            "name",
            "serviceName",
            "appName",
            "project",
            "assetName",
            "functionName",
            "worker",
        ],
    )
    .unwrap_or(&id)
    .to_string();
    let asset_category = str_at(v, &["assetCategory", "category", "assetType", "assetKind"])
        .or_else(|| {
            nested_str_at(
                v,
                &[
                    "/asset/category",
                    "/metadata/assetCategory",
                    "/metadata/category",
                    "/metadata/assetType",
                ],
            )
        })
        .map(str::to_string)
        .or_else(|| inferred_activity_category(v).map(str::to_string))
        .unwrap_or_default();
    let kind = str_at(v, &["kind", "type", "resourceType", "runtimeKind"])
        .unwrap_or_else(|| {
            if source.contains("function") {
                "function"
            } else if source.contains("deployment") {
                "deployment"
            } else {
                "service"
            }
        })
        .to_string();
    let status = str_at(v, &["status", "state", "phase", "runtimeStatus"])
        .unwrap_or("unknown")
        .to_string();
    let image = str_at(v, &["image", "containerImage", "ref", "asset", "worker"])
        .unwrap_or("")
        .to_string();
    let access_url = str_at(
        v,
        &["accessUrl", "gatewayUrl", "publicUrl", "url", "endpoint"],
    )
    .map(str::to_string);
    let updated = str_at(
        v,
        &[
            "updatedAt",
            "modifiedAt",
            "startedAt",
            "createdAt",
            "lastSeenAt",
        ],
    )
    .map(short_time)
    .unwrap_or_default();
    Some(RuntimeActivityRow {
        id,
        name,
        asset_category,
        kind,
        status,
        image,
        access_url,
        updated,
        source: source.to_string(),
    })
}

pub(crate) async fn fetch_asset_list(
    address: &str,
    token: &str,
    query: &str,
) -> Result<AssetListFetch, String> {
    let origin = crate::a3s_os::os_origin(address);
    let url = with_asset_query(&origin, query)?;
    let resp = http()?
        .get(url.clone())
        .bearer_auth(token)
        .header("accept", "application/json")
        .send()
        .await
        .map_err(|e| format!("request to {url} failed: {e}"))?;
    let status = resp.status();
    let text = resp.text().await.map_err(|e| e.to_string())?;
    if !status.is_success() {
        return Err(format!(
            "assets returned HTTP {}: {}",
            status.as_u16(),
            truncate(&text, 160)
        ));
    }
    let json: Value =
        serde_json::from_str(&text).map_err(|_| "assets returned non-JSON".to_string())?;
    let mut rows = items_of(&json)
        .iter()
        .filter_map(asset_from_value)
        .collect::<Vec<_>>();
    if !query.trim().is_empty() {
        rows.retain(|row| asset_matches(row, query));
    }
    Ok(AssetListFetch {
        note: format!("{} asset(s) · {}", rows.len(), url.path()),
        rows,
    })
}

const RUNTIME_ACTIVITY_ENDPOINTS: &[&str] = &[
    "/api/v1/runtime/services",
    "/api/v1/runtime/deployments",
    "/api/v1/runtime/jobs",
    "/api/v1/deployments",
    "/api/v1/services",
    "/api/v1/functions/batches",
    "/api/v1/functions/invocations",
];

pub(crate) async fn fetch_runtime_activity(
    address: &str,
    token: &str,
    query: &str,
) -> Result<RuntimeActivityFetch, String> {
    let origin = crate::a3s_os::os_origin(address);
    let client = http()?;
    let mut rows = Vec::new();
    let mut ok_sources = Vec::new();
    let mut errors = Vec::new();
    for endpoint in RUNTIME_ACTIVITY_ENDPOINTS {
        let url = with_runtime_query(&origin, endpoint, query)?;
        let resp = match client
            .get(url.clone())
            .bearer_auth(token)
            .header("accept", "application/json")
            .send()
            .await
        {
            Ok(resp) => resp,
            Err(e) => {
                errors.push(format!("{endpoint}: {e}"));
                continue;
            }
        };
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            errors.push(format!("{endpoint}: HTTP {}", status.as_u16()));
            continue;
        }
        ok_sources.push(endpoint.trim_start_matches("/api/v1/").to_string());
        if let Ok(json) = serde_json::from_str::<Value>(&text) {
            rows.extend(
                items_of(&json)
                    .iter()
                    .filter_map(|item| activity_from_value(item, endpoint)),
            );
        }
    }
    if ok_sources.is_empty() {
        return Err(if errors.is_empty() {
            "no OS runtime activity endpoint responded".to_string()
        } else {
            errors.join(" · ")
        });
    }
    if !query.trim().is_empty() {
        rows.retain(|row| activity_matches(row, query));
    }
    rows.sort_by_key(|row| row.name.to_lowercase());
    rows.dedup_by(|a, b| a.id == b.id && a.name == b.name);
    Ok(RuntimeActivityFetch {
        note: format!("{} runtime row(s) · {}", rows.len(), ok_sources.join(", ")),
        rows,
    })
}

fn asset_matches(row: &AssetRow, query: &str) -> bool {
    let (category, q) = asset_query_parts(query);
    let q = q.to_ascii_lowercase();
    if let Some(category) = category {
        let row_category = row.category.to_lowercase();
        if row_category != category && !row_category.contains(&category) {
            return false;
        }
    }
    q.is_empty()
        || [
            &row.id,
            &row.name,
            &row.category,
            &row.kind,
            &row.status,
            &row.visibility,
            &row.owner,
        ]
        .iter()
        .any(|s| s.to_lowercase().contains(&q))
}

fn asset_query_parts(query: &str) -> (Option<String>, String) {
    let mut category = None;
    let mut terms = Vec::new();
    for part in query.split_whitespace() {
        if let Some(value) = part.strip_prefix("category:") {
            if !value.trim().is_empty() {
                category = Some(value.trim().to_ascii_lowercase());
            }
        } else {
            terms.push(part);
        }
    }
    (category, terms.join(" "))
}

fn activity_matches(row: &RuntimeActivityRow, query: &str) -> bool {
    let (category, search) = activity_query_parts(query);
    if let Some(category) = category {
        let exact_category = row.asset_category.to_ascii_lowercase();
        if !exact_category.is_empty()
            && exact_category != category
            && !exact_category.contains(&category)
        {
            return false;
        }
    }
    let terms = search
        .split_whitespace()
        .map(str::to_ascii_lowercase)
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return true;
    }
    let fields = [
        &row.id,
        &row.name,
        &row.asset_category,
        &row.kind,
        &row.status,
        &row.image,
    ]
    .iter()
    .map(|s| s.to_lowercase())
    .collect::<Vec<_>>();
    terms
        .iter()
        .all(|term| fields.iter().any(|field| field.contains(term)))
}

fn activity_query_parts(query: &str) -> (Option<String>, String) {
    asset_query_parts(query)
}

fn scoped_query(category: Option<&str>, query: &str) -> String {
    let (_, query) = asset_query_parts(query);
    let category = category.map(str::trim).filter(|value| !value.is_empty());
    match (category, query.trim().is_empty()) {
        (Some(category), true) => format!("category:{category}"),
        (Some(category), false) => format!("category:{category} {}", query.trim()),
        (None, true) => String::new(),
        (None, false) => query,
    }
}

fn selected_asset(panel: &AssetListPanel) -> Option<AssetRow> {
    let query = scoped_query(panel.category.as_deref(), &panel.query);
    panel
        .rows
        .iter()
        .filter(|row| asset_matches(row, &query))
        .nth(panel.sel)
        .cloned()
}

fn selected_activity(panel: &RuntimeActivityPanel) -> Option<RuntimeActivityRow> {
    let query = scoped_query(panel.category.as_deref(), &panel.query);
    panel
        .rows
        .iter()
        .filter(|row| activity_matches(row, &query))
        .nth(panel.sel)
        .cloned()
}

fn edit_search(query: &mut String, key: &KeyEvent) -> bool {
    match key.code {
        KeyCode::Backspace => {
            query.pop();
            true
        }
        KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
            query.push(c);
            true
        }
        _ => false,
    }
}

impl App {
    pub(crate) fn open_asset_list_panel(&mut self, query: String) -> Option<Cmd<Msg>> {
        let Some(session) = self.os_session.clone() else {
            self.push_line(&os_required_alert("asset list", self.os_config.is_some()));
            return None;
        };
        let (category, query) = asset_query_parts(&query);
        let q = scoped_query(category.as_deref(), &query);
        self.asset_list = Some(AssetListPanel {
            rows: Vec::new(),
            sel: 0,
            scroll: 0,
            category,
            query,
            searching: false,
            loading: true,
            note: "loading assets…".to_string(),
        });
        Some(cmd::cmd(move || async move {
            Msg::AssetListLoaded(
                fetch_asset_list(&session.address, &session.access_token, &q).await,
            )
        }))
    }

    pub(crate) fn open_runtime_activity_panel(&mut self, query: String) -> Option<Cmd<Msg>> {
        let Some(session) = self.os_session.clone() else {
            self.push_line(&os_required_alert(
                "runtime activity",
                self.os_config.is_some(),
            ));
            return None;
        };
        let (category, query) = activity_query_parts(&query);
        let q = scoped_query(category.as_deref(), &query);
        self.runtime_activity = Some(RuntimeActivityPanel {
            rows: Vec::new(),
            sel: 0,
            scroll: 0,
            category,
            query,
            searching: false,
            loading: true,
            note: "loading runtime activity…".to_string(),
        });
        Some(cmd::cmd(move || async move {
            Msg::RuntimeActivityLoaded(
                fetch_runtime_activity(&session.address, &session.access_token, &q).await,
            )
        }))
    }

    fn reload_asset_list(&self) -> Option<Cmd<Msg>> {
        let session = self.os_session.clone()?;
        let query = self
            .asset_list
            .as_ref()
            .map(|p| scoped_query(p.category.as_deref(), &p.query))
            .unwrap_or_default();
        Some(cmd::cmd(move || async move {
            Msg::AssetListLoaded(
                fetch_asset_list(&session.address, &session.access_token, &query).await,
            )
        }))
    }

    fn reload_runtime_activity(&self) -> Option<Cmd<Msg>> {
        let session = self.os_session.clone()?;
        let query = self
            .runtime_activity
            .as_ref()
            .map(|p| scoped_query(p.category.as_deref(), &p.query))
            .unwrap_or_default();
        Some(cmd::cmd(move || async move {
            Msg::RuntimeActivityLoaded(
                fetch_runtime_activity(&session.address, &session.access_token, &query).await,
            )
        }))
    }

    pub(crate) fn handle_asset_list_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if self.asset_list.as_ref().is_some_and(|p| p.searching) {
            match key.code {
                KeyCode::Esc => {
                    if let Some(p) = self.asset_list.as_mut() {
                        p.searching = false;
                    }
                }
                KeyCode::Enter => {
                    if let Some(p) = self.asset_list.as_mut() {
                        p.searching = false;
                        p.loading = true;
                        p.note = format!("searching `{}`…", p.query);
                    }
                    return self.reload_asset_list();
                }
                _ => {
                    if let Some(p) = self.asset_list.as_mut() {
                        if edit_search(&mut p.query, key) {
                            p.sel = 0;
                            p.scroll = 0;
                        }
                    }
                }
            }
            return None;
        }
        match key.code {
            KeyCode::Esc => {
                self.asset_list = None;
                None
            }
            KeyCode::Char('/') | KeyCode::Char('s') => {
                if let Some(p) = self.asset_list.as_mut() {
                    p.searching = true;
                }
                None
            }
            KeyCode::Char('r') => {
                if let Some(p) = self.asset_list.as_mut() {
                    p.loading = true;
                    p.note = "refreshing…".to_string();
                }
                self.reload_asset_list()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(p) = self.asset_list.as_mut() {
                    p.sel = p.sel.saturating_sub(1);
                    p.scroll = p.scroll.min(p.sel);
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(p) = self.asset_list.as_mut() {
                    let query = scoped_query(p.category.as_deref(), &p.query);
                    let last = p
                        .rows
                        .iter()
                        .filter(|row| asset_matches(row, &query))
                        .count()
                        .saturating_sub(1);
                    p.sel = (p.sel + 1).min(last);
                }
                None
            }
            KeyCode::PageUp => {
                if let Some(p) = self.asset_list.as_mut() {
                    p.sel = p.sel.saturating_sub(10);
                    p.scroll = p.scroll.saturating_sub(10);
                }
                None
            }
            KeyCode::PageDown => {
                if let Some(p) = self.asset_list.as_mut() {
                    let query = scoped_query(p.category.as_deref(), &p.query);
                    let last = p
                        .rows
                        .iter()
                        .filter(|row| asset_matches(row, &query))
                        .count()
                        .saturating_sub(1);
                    p.sel = (p.sel + 10).min(last);
                }
                None
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                let row = self.asset_list.as_ref().and_then(selected_asset);
                if let Some(row) = row.as_ref() {
                    self.open_asset_view(row);
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn handle_runtime_activity_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if self.runtime_activity.as_ref().is_some_and(|p| p.searching) {
            match key.code {
                KeyCode::Esc => {
                    if let Some(p) = self.runtime_activity.as_mut() {
                        p.searching = false;
                    }
                }
                KeyCode::Enter => {
                    if let Some(p) = self.runtime_activity.as_mut() {
                        p.searching = false;
                        p.loading = true;
                        p.note = format!("searching `{}`…", p.query);
                    }
                    return self.reload_runtime_activity();
                }
                _ => {
                    if let Some(p) = self.runtime_activity.as_mut() {
                        if edit_search(&mut p.query, key) {
                            p.sel = 0;
                            p.scroll = 0;
                        }
                    }
                }
            }
            return None;
        }
        match key.code {
            KeyCode::Esc => {
                self.runtime_activity = None;
                None
            }
            KeyCode::Char('/') | KeyCode::Char('s') => {
                if let Some(p) = self.runtime_activity.as_mut() {
                    p.searching = true;
                }
                None
            }
            KeyCode::Char('r') => {
                if let Some(p) = self.runtime_activity.as_mut() {
                    p.loading = true;
                    p.note = "refreshing…".to_string();
                }
                self.reload_runtime_activity()
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(p) = self.runtime_activity.as_mut() {
                    p.sel = p.sel.saturating_sub(1);
                    p.scroll = p.scroll.min(p.sel);
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(p) = self.runtime_activity.as_mut() {
                    let query = scoped_query(p.category.as_deref(), &p.query);
                    let last = p
                        .rows
                        .iter()
                        .filter(|row| activity_matches(row, &query))
                        .count()
                        .saturating_sub(1);
                    p.sel = (p.sel + 1).min(last);
                }
                None
            }
            KeyCode::PageUp => {
                if let Some(p) = self.runtime_activity.as_mut() {
                    p.sel = p.sel.saturating_sub(10);
                    p.scroll = p.scroll.saturating_sub(10);
                }
                None
            }
            KeyCode::PageDown => {
                if let Some(p) = self.runtime_activity.as_mut() {
                    let query = scoped_query(p.category.as_deref(), &p.query);
                    let last = p
                        .rows
                        .iter()
                        .filter(|row| activity_matches(row, &query))
                        .count()
                        .saturating_sub(1);
                    p.sel = (p.sel + 10).min(last);
                }
                None
            }
            KeyCode::Enter | KeyCode::Char('o') => {
                let row = self.runtime_activity.as_ref().and_then(selected_activity);
                if let Some(row) = row.as_ref() {
                    self.open_runtime_view(row);
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn on_asset_list(&mut self, result: Result<AssetListFetch, String>) {
        let Some(panel) = self.asset_list.as_mut() else {
            return;
        };
        panel.loading = false;
        match result {
            Ok(fetch) => {
                panel.rows = fetch.rows;
                panel.sel = panel.sel.min(panel.rows.len().saturating_sub(1));
                panel.note = fetch.note;
            }
            Err(e) => panel.note = format!("✗ {e}"),
        }
    }

    pub(crate) fn on_runtime_activity(&mut self, result: Result<RuntimeActivityFetch, String>) {
        let Some(panel) = self.runtime_activity.as_mut() else {
            return;
        };
        panel.loading = false;
        match result {
            Ok(fetch) => {
                panel.rows = fetch.rows;
                panel.sel = panel.sel.min(panel.rows.len().saturating_sub(1));
                panel.note = fetch.note;
            }
            Err(e) => panel.note = format!("✗ {e}"),
        }
    }

    fn open_asset_view(&mut self, row: &AssetRow) {
        let Some(session) = self.os_session.as_ref() else {
            return;
        };
        let origin = crate::a3s_os::os_origin(&session.address);
        let id_path = path_segment(&row.id);
        let url = row
            .access_url
            .clone()
            .unwrap_or_else(|| format!("{origin}/admin/assets/{id_path}?embed=1"));
        let label = format!("OS asset view · {}", truncate(&row.name, 48));
        let spec = remote_ui::ViewSpec {
            url,
            width: Some(1440),
            height: Some(900),
            embeddable: true,
        };
        self.last_view = Some(spec.clone());
        self.push_line(&gutter(
            ACCENT,
            &remote_view_button(&format!("{label} · click to reopen")),
        ));
        self.open_remote_view(&spec);
    }

    fn open_runtime_view(&mut self, row: &RuntimeActivityRow) {
        let Some(session) = self.os_session.as_ref() else {
            return;
        };
        let origin = crate::a3s_os::os_origin(&session.address);
        let id_path = path_segment(&row.id);
        let url = row.access_url.clone().unwrap_or_else(|| {
            format!("{origin}/admin/infrastructure/batch?service={id_path}&embed=1")
        });
        let label = format!("OS runtime activity · {}", truncate(&row.name, 48));
        let spec = remote_ui::ViewSpec {
            url,
            width: Some(1440),
            height: Some(900),
            embeddable: true,
        };
        self.last_view = Some(spec.clone());
        self.push_line(&gutter(
            ACCENT,
            &remote_view_button(&format!("{label} · click to reopen")),
        ));
        self.open_remote_view(&spec);
    }

    pub(crate) fn render_asset_list(&self, panel: &AssetListPanel) -> String {
        let query = scoped_query(panel.category.as_deref(), &panel.query);
        let rows = panel
            .rows
            .iter()
            .filter(|row| asset_matches(row, &query))
            .collect::<Vec<_>>();
        let scope = panel
            .category
            .as_deref()
            .map(|category| format!(" · category:{category}"))
            .unwrap_or_default();
        let header = format!(
            "OS assets{scope} · {} shown / {} total{}",
            rows.len(),
            panel.rows.len(),
            if panel.loading { " · loading" } else { "" }
        );
        let hint = "↑↓/jk select · / search · Enter/o open selected · r refresh · Esc close";
        let spec = ResourcePanelRender {
            header: &header,
            query: &panel.query,
            searching: panel.searching,
            note: &panel.note,
            sel: panel.sel,
            scroll: panel.scroll,
            total: rows.len(),
            hint,
        };
        let items = rows
            .iter()
            .map(|row| asset_list_item(row))
            .collect::<Vec<_>>();
        self.render_resource_panel(&spec, items, |width| {
            asset_detail(rows.get(panel.sel).copied(), width)
        })
    }

    pub(crate) fn render_runtime_activity(&self, panel: &RuntimeActivityPanel) -> String {
        let query = scoped_query(panel.category.as_deref(), &panel.query);
        let rows = panel
            .rows
            .iter()
            .filter(|row| activity_matches(row, &query))
            .collect::<Vec<_>>();
        let scope = panel
            .category
            .as_deref()
            .map(|category| format!(" · category:{category}"))
            .unwrap_or_default();
        let header = format!(
            "OS runtime activity{scope} · {} shown / {} total{}",
            rows.len(),
            panel.rows.len(),
            if panel.loading { " · loading" } else { "" }
        );
        let spec = ResourcePanelRender {
            header: &header,
            query: &panel.query,
            searching: panel.searching,
            note: &panel.note,
            sel: panel.sel,
            scroll: panel.scroll,
            total: rows.len(),
            hint: runtime_activity_hint(),
        };
        let items = rows
            .iter()
            .map(|row| activity_list_item(row))
            .collect::<Vec<_>>();
        self.render_resource_panel(&spec, items, |width| {
            activity_detail(rows.get(panel.sel).copied(), width)
        })
    }

    fn render_resource_panel<Detail>(
        &self,
        spec: &ResourcePanelRender<'_>,
        items: Vec<MenuItem>,
        detail: Detail,
    ) -> String
    where
        Detail: Fn(usize) -> Vec<String>,
    {
        let width = self.width as usize;
        let h = self.height as usize;
        let (left_w, right_w, sep_w) = resource_columns(width);
        let sep = if sep_w == 0 {
            String::new()
        } else {
            Style::new().fg(TN_GRAY).render(" │ ")
        };
        let mut out = vec![
            resource_line(
                &Style::new()
                    .fg(ACCENT)
                    .bold()
                    .render(&format!("  {}", spec.header)),
                width,
            ),
            resource_line(
                &Style::new().fg(TN_GRAY).render(&format!(
                    "  {}{}",
                    if spec.searching {
                        "search> "
                    } else {
                        "search: "
                    },
                    if spec.query.is_empty() {
                        "—"
                    } else {
                        spec.query
                    }
                )),
                width,
            ),
            resource_line(
                &Style::new()
                    .fg(if spec.note.starts_with('✗') {
                        TN_RED
                    } else {
                        TN_GRAY
                    })
                    .render(&format!("  {}", spec.note)),
                width,
            ),
            resource_line(
                &divider_line_with(width.min(u16::MAX as usize) as u16, "─", TN_GRAY),
                width,
            ),
        ];
        let body = h.saturating_sub(5);
        let sel = spec.sel.min(spec.total.saturating_sub(1));
        let start = if sel < spec.scroll {
            sel
        } else if sel >= spec.scroll + body {
            sel + 1 - body
        } else {
            spec.scroll
        }
        .min(spec.total.saturating_sub(1));
        let detail_lines = detail(right_w);
        let left_lines = resource_menu_lines(items, sel, start, left_w, body);
        for i in 0..body {
            let left = if spec.total == 0 && i == 0 {
                Style::new()
                    .fg(TN_GRAY)
                    .render(&resource_line("  no rows match", left_w))
            } else {
                left_lines
                    .get(i)
                    .cloned()
                    .unwrap_or_else(|| " ".repeat(left_w))
            };
            let right = detail_lines.get(i).cloned().unwrap_or_default();
            out.push(format!("{left}{sep}{}", resource_line(&right, right_w)));
        }
        out.push(resource_line(
            &Style::new().fg(TN_GRAY).render(&format!("  {}", spec.hint)),
            width,
        ));
        out.truncate(h);
        while out.len() < h {
            out.push(String::new());
        }
        out.join("\n")
    }
}

fn resource_menu_lines(
    items: Vec<MenuItem>,
    selected: usize,
    scroll: usize,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    MenuPanel::without_title()
        .items(items)
        .selected(selected)
        .scroll(scroll)
        .max_items(height)
        .show_scroll(true)
        .indent(1)
        .marker("▸")
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED)
        .view(width.min(u16::MAX as usize) as u16, height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn asset_list_item(row: &AssetRow) -> MenuItem {
    MenuItem::new(format!(
        "{} {}",
        truncate(&row.category, 13),
        truncate(&row.status, 11)
    ))
    .description(row.name.clone())
}

fn activity_list_item(row: &RuntimeActivityRow) -> MenuItem {
    MenuItem::new(format!(
        "{} {}",
        truncate(&row.kind, 12),
        truncate(&row.status, 11)
    ))
    .description(row.name.clone())
}

fn asset_detail(row: Option<&AssetRow>, width: usize) -> Vec<String> {
    let Some(row) = row else {
        return DetailPanel::without_title()
            .show_separator(false)
            .indent(0)
            .muted_color(TN_GRAY)
            .row(DetailRow::muted("select an asset"))
            .view(width.min(u16::MAX as usize) as u16, 1)
            .lines()
            .map(str::to_string)
            .collect();
    };

    let mut panel = DetailPanel::new(row.name.clone())
        .show_separator(false)
        .indent(0)
        .title_color(TN_FG)
        .label_color(TN_GRAY)
        .value_color(TN_FG)
        .label_width(10)
        .unlimited_rows();
    for (label, value) in [
        ("id", row.id.as_str()),
        ("category", row.category.as_str()),
        ("kind", row.kind.as_str()),
        ("status", row.status.as_str()),
        ("visibility", row.visibility.as_str()),
        ("owner", row.owner.as_str()),
        ("updated", row.updated.as_str()),
    ] {
        if !value.is_empty() {
            panel = panel.pair(label, value);
        }
    }
    if let Some(url) = &row.access_url {
        panel = panel.pair("access", url);
    }

    panel
        .view(
            width.min(u16::MAX as usize) as u16,
            panel.rows().len().saturating_add(1),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

fn activity_detail(row: Option<&RuntimeActivityRow>, width: usize) -> Vec<String> {
    let Some(row) = row else {
        return DetailPanel::without_title()
            .show_separator(false)
            .indent(0)
            .muted_color(TN_GRAY)
            .row(DetailRow::muted("select a runtime row"))
            .view(width.min(u16::MAX as usize) as u16, 1)
            .lines()
            .map(str::to_string)
            .collect();
    };

    let mut panel = DetailPanel::new(row.name.clone())
        .show_separator(false)
        .indent(0)
        .title_color(TN_FG)
        .label_color(TN_GRAY)
        .value_color(TN_FG)
        .label_width(10)
        .unlimited_rows();
    for (label, value) in [
        ("id", row.id.as_str()),
        ("asset", row.asset_category.as_str()),
        ("kind", row.kind.as_str()),
        ("status", row.status.as_str()),
        ("image/ref", row.image.as_str()),
        ("source", row.source.as_str()),
        ("updated", row.updated.as_str()),
    ] {
        if !value.is_empty() {
            panel = panel.pair(label, value);
        }
    }
    if let Some(url) = &row.access_url {
        panel = panel.pair("access", url);
    }

    panel
        .view(
            width.min(u16::MAX as usize) as u16,
            panel.rows().len().saturating_add(1),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    fn v(s: &str) -> Value {
        serde_json::from_str(s).unwrap()
    }

    async fn spawn_mock_os(captured: Arc<Mutex<Vec<String>>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let requests = captured.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 8192];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    requests.lock().unwrap().push(req);
                    let (status, payload) = mock_response(&line);
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

    fn mock_response(line: &str) -> (&'static str, &'static str) {
        if line.starts_with("GET /api/v1/assets?") {
            return (
                "200 OK",
                r#"{"data":{"items":[{"id":"asset 1?","name":"Payment Agent","category":"agent","status":"published"},{"id":"workflow-1","name":"Unrelated Flow","category":"workflow","status":"published"}]}}"#,
            );
        }
        if line.starts_with("GET /api/v1/runtime/services?") {
            if line.contains("search=mcp-api") {
                return (
                    "200 OK",
                    r#"{"data":{"services":[{"serviceId":"svc-mcp","serviceName":"mcp-api","metadata":{"protocol":"mcp"},"state":"running","image":"img:mcp"},{"serviceId":"svc-workflow","serviceName":"mcp-api","targetKind":"workflow","state":"running","image":"img:workflow"}]}}"#,
                );
            }
            return (
                "200 OK",
                r#"{"data":{"services":[{"serviceId":"svc 1?","serviceName":"api","state":"running","image":"img:v1"},{"serviceId":"svc 2","serviceName":"unrelated-worker","state":"running","image":"img:v2"}]}}"#,
            );
        }
        ("404 Not Found", r#"{"error":"not found"}"#)
    }

    fn captured_lines(captured: &Arc<Mutex<Vec<String>>>) -> Vec<String> {
        captured
            .lock()
            .unwrap()
            .iter()
            .map(|req| req.lines().next().unwrap_or("").to_string())
            .collect()
    }

    #[test]
    fn resource_columns_fit_narrow_width() {
        let (left, right, sep) = resource_columns(30);

        assert!(left > 0, "left column should not collapse");
        assert!(right > 0, "right column should not collapse");
        assert!(left + right + sep <= 30);
    }

    #[test]
    fn resource_lines_are_width_bounded_with_styles() {
        let line = resource_line(
            &Style::new()
                .fg(ACCENT)
                .bold()
                .render("  OS runtime activity · 999 shown / 999 total · loading"),
            32,
        );

        assert!(
            a3s_tui::style::visible_len(&line) <= 32,
            "{}",
            a3s_tui::style::strip_ansi(&line)
        );
    }

    #[test]
    fn resource_menu_lines_use_shared_menu_rows_and_bound_width() {
        let items = vec![
            asset_list_item(&AssetRow {
                id: "asset-1".into(),
                name: "very-long-asset-name-that-would-overflow-the-resource-list-panel".into(),
                category: "knowledge".into(),
                kind: "tool".into(),
                status: "published".into(),
                visibility: "private".into(),
                owner: "roy".into(),
                updated: String::new(),
                access_url: None,
            }),
            asset_list_item(&AssetRow {
                id: "asset-2".into(),
                name: "ops-agent".into(),
                category: "agent".into(),
                kind: "tool".into(),
                status: "draft".into(),
                visibility: "private".into(),
                owner: "roy".into(),
                updated: String::new(),
                access_url: None,
            }),
        ];
        let lines = resource_menu_lines(items, 0, 0, 36, 4);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("knowledge"), "{plain}");
        assert!(plain.contains("very-long-a"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 36),
            "{plain}"
        );
    }

    #[test]
    fn resource_menu_lines_scroll_selected_row_into_view() {
        let items = (0..16)
            .map(|index| {
                activity_list_item(&RuntimeActivityRow {
                    id: format!("svc-{index}"),
                    name: format!("runtime-{index}"),
                    asset_category: "mcp".into(),
                    kind: "service".into(),
                    status: "running".into(),
                    image: "img:v1".into(),
                    access_url: None,
                    updated: String::new(),
                    source: "runtime".into(),
                })
            })
            .collect::<Vec<_>>();
        let plain = resource_menu_lines(items, 14, 0, 40, 6)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("runtime-14"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn asset_detail_uses_shared_detail_panel_and_bounds_width() {
        let row = AssetRow {
            id: "asset-1".into(),
            name: "Payment Agent With A Very Long Display Name".into(),
            category: "agent".into(),
            kind: "tool".into(),
            status: "published".into(),
            visibility: "private".into(),
            owner: "roy".into(),
            updated: "2026-07-04T12:30:00Z".into(),
            access_url: Some("https://os.example/assets/asset-1/launch".into()),
        };

        let lines = asset_detail(Some(&row), 36);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("Payment Agent"), "{plain}");
        assert!(plain.contains("category"), "{plain}");
        assert!(plain.contains("published"), "{plain}");
        assert!(plain.contains("access"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 36),
            "{plain}"
        );

        let empty = asset_detail(None, 24)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(empty.contains("select an asset"), "{empty}");
    }

    #[test]
    fn activity_detail_uses_shared_detail_panel_and_bounds_width() {
        let row = RuntimeActivityRow {
            id: "svc-1".into(),
            name: "Runtime Worker With A Very Long Display Name".into(),
            asset_category: "workflow".into(),
            kind: "service".into(),
            status: "running".into(),
            image: "registry.example/a3s/runtime-worker:2026-07-04".into(),
            access_url: Some("https://runtime.example/services/svc-1".into()),
            updated: "2026-07-04T12:31:00Z".into(),
            source: "runtime".into(),
        };

        let lines = activity_detail(Some(&row), 38);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("Runtime Worker"), "{plain}");
        assert!(plain.contains("workflow"), "{plain}");
        assert!(plain.contains("image/ref"), "{plain}");
        assert!(plain.contains("runtime"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 38),
            "{plain}"
        );
    }

    #[test]
    fn items_of_accepts_common_envelopes() {
        assert_eq!(items_of(&v(r#"{"data":{"items":[{"id":"a"}]}}"#)).len(), 1);
        assert_eq!(
            items_of(&v(r#"{"items":[{"id":"a"},{"id":"b"}]}"#)).len(),
            2
        );
        assert_eq!(items_of(&v(r#"[{"id":"a"}]"#)).len(), 1);
    }

    #[test]
    fn parses_asset_rows_leniently() {
        let row = asset_from_value(&v(
            r#"{"id":"a1","name":"demo","category":"application","agentKind":"tool","published":true,"owner":{"name":"me"},"updatedAt":"2026-07-03T01:02:03Z"}"#,
        ))
        .unwrap();
        assert_eq!(row.id, "a1");
        assert_eq!(row.category, "application");
        assert_eq!(row.kind, "tool");
        assert_eq!(row.status, "published");
        assert_eq!(row.owner, "me");
        assert_eq!(row.updated, "2026-07-03 01:02:03");
    }

    #[test]
    fn parses_runtime_activity_rows_leniently() {
        let row = activity_from_value(
            &v(r#"{"deploymentId":"d1","serviceName":"api","state":"running","containerImage":"img:v1","accessUrl":"https://x"}"#),
            "/api/v1/runtime/deployments",
        )
        .unwrap();
        assert_eq!(row.id, "d1");
        assert_eq!(row.name, "api");
        assert_eq!(row.asset_category, "");
        assert_eq!(row.kind, "deployment");
        assert_eq!(row.status, "running");
        assert_eq!(row.access_url.as_deref(), Some("https://x"));

        let runtime_service = activity_from_value(
            &v(r#"{"id":"wf:1","name":"daily-flow","targetKind":"workflow","runtimePlane":"workflow-core","state":"running"}"#),
            "/api/v1/runtime/services",
        )
        .unwrap();
        assert_eq!(runtime_service.asset_category, "workflow");

        let mcp_service = activity_from_value(
            &v(r#"{"id":"fn:1","name":"weather-mcp","kind":"serving","metadata":{"protocol":"mcp"},"state":"running"}"#),
            "/api/v1/runtime/services",
        )
        .unwrap();
        assert_eq!(mcp_service.asset_category, "mcp");
    }

    #[test]
    fn activity_category_filter_only_hard_filters_explicit_asset_categories() {
        let explicit_workflow = activity_from_value(
            &v(r#"{"serviceId":"svc-workflow","serviceName":"api","assetCategory":"workflow","state":"running"}"#),
            "/api/v1/runtime/services",
        )
        .unwrap();
        assert!(
            !activity_matches(&explicit_workflow, "category:mcp api"),
            "explicit workflow activity must not leak into MCP activity"
        );

        let implicit_runtime_kind = activity_from_value(
            &v(r#"{"serviceId":"svc-agent","serviceName":"api","runtimeBinding":{"kind":"tool"},"state":"running"}"#),
            "/api/v1/runtime/services",
        )
        .unwrap();
        assert_eq!(
            implicit_runtime_kind.asset_category, "",
            "runtime binding kind is not the asset family"
        );
        assert!(
            activity_matches(&implicit_runtime_kind, "category:agent api"),
            "missing asset category should not hide a search-matching runtime row"
        );
    }

    #[test]
    fn search_matches_across_visible_fields() {
        let row = AssetRow {
            id: "a1".into(),
            name: "Payment Agent".into(),
            category: "agent".into(),
            kind: "tool".into(),
            status: "published".into(),
            visibility: "private".into(),
            owner: "roy".into(),
            updated: String::new(),
            access_url: None,
        };
        assert!(asset_matches(&row, "payment"));
        assert!(asset_matches(&row, "tool"));
        assert!(!asset_matches(&row, "workflow"));
    }

    #[test]
    fn scoped_query_keeps_asset_scope_outside_editable_search_terms() {
        assert_eq!(scoped_query(Some("agent"), ""), "category:agent");
        assert_eq!(
            scoped_query(Some("agent"), "Payment Agent"),
            "category:agent Payment Agent"
        );
        assert_eq!(
            scoped_query(Some("agent"), "category:workflow Payment Agent"),
            "category:agent Payment Agent",
            "editable search text must not override the fixed asset-family scope"
        );
        assert_eq!(scoped_query(None, "category:mcp api"), "api");
    }

    #[test]
    fn path_segment_encodes_reserved_chars() {
        assert_eq!(path_segment("asset 1?#"), "asset%201%3F%23");
        assert_eq!(path_segment("café"), "caf%C3%A9");
    }

    #[test]
    fn runtime_activity_hint_stays_read_only() {
        let hint = runtime_activity_hint();

        assert!(hint.contains("open"), "{hint}");
        assert!(hint.contains("refresh"), "{hint}");
        assert!(!hint.contains("runtime view"), "{hint}");
        assert!(!hint.contains(" O "), "{hint}");
        assert!(!hint.contains("stop"), "{hint}");
        assert!(!hint.contains("cancel"), "{hint}");
    }

    #[tokio::test]
    async fn fetch_asset_list_uses_bearer_token_category_and_search_query() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mock_os(captured.clone()).await;
        let fetch = fetch_asset_list(&origin, "tok-asset", "category:agent Payment Agent")
            .await
            .unwrap();
        assert_eq!(fetch.rows.len(), 1);
        assert_eq!(fetch.rows[0].name, "Payment Agent");
        assert_eq!(fetch.rows[0].category, "agent");
        assert!(
            !fetch.rows.iter().any(|row| row.name == "Unrelated Flow"),
            "fetch should keep asset lists category-scoped even if OS ignores filters"
        );
        let requests = captured.lock().unwrap();
        let req = requests.first().unwrap();
        let request_line = req.lines().next().unwrap();
        assert!(request_line.contains("/api/v1/assets?"));
        assert!(request_line.contains("limit=100"));
        assert!(request_line.contains("scope=mine"), "{request_line}");
        assert!(request_line.contains("status=all"), "{request_line}");
        assert!(
            request_line.contains("search=Payment+Agent")
                || request_line.contains("search=Payment%20Agent"),
            "{request_line}"
        );
        assert!(request_line.contains("category=agent"), "{request_line}");
        assert!(req
            .to_ascii_lowercase()
            .contains("authorization: bearer tok-asset"));
    }

    #[tokio::test]
    async fn fetch_runtime_activity_reads_runtime_activity_endpoint() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mock_os(captured.clone()).await;
        let fetch = fetch_runtime_activity(&origin, "tok-service", "api")
            .await
            .unwrap();
        assert_eq!(fetch.rows.len(), 1);
        assert_eq!(fetch.rows[0].id, "svc 1?");
        assert_eq!(fetch.rows[0].name, "api");
        assert_eq!(fetch.rows[0].status, "running");
        assert!(
            !fetch.rows.iter().any(|row| row.name == "unrelated-worker"),
            "fetch should keep runtime activity asset-scoped even if OS ignores search"
        );
        let lines = captured_lines(&captured);
        assert!(lines
            .iter()
            .any(|line| line.contains("/api/v1/runtime/services?limit=100&search=api")));
    }

    #[tokio::test]
    async fn fetch_runtime_activity_keeps_category_local_and_filters_same_named_assets() {
        let captured = Arc::new(Mutex::new(Vec::new()));
        let origin = spawn_mock_os(captured.clone()).await;
        let fetch = fetch_runtime_activity(&origin, "tok-service", "category:mcp mcp-api")
            .await
            .unwrap();

        assert_eq!(fetch.rows.len(), 1);
        assert_eq!(fetch.rows[0].id, "svc-mcp");
        assert_eq!(fetch.rows[0].name, "mcp-api");
        assert_eq!(fetch.rows[0].asset_category, "mcp");
        assert!(
            !fetch.rows.iter().any(|row| row.id == "svc-workflow"),
            "same-name workflow activity must not leak into /mcp activity"
        );
        let lines = captured_lines(&captured);
        assert!(lines.iter().any(|line| {
            line.contains("/api/v1/runtime/services?")
                && line.contains("search=mcp-api")
                && !line.contains("category=mcp")
        }));
    }
}
