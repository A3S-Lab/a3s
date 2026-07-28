use std::path::{Path, PathBuf};

use serde_json::json;

use crate::commands::code::asset_types::AssetCommandOutput;
use crate::tui::{asset_clone, panels};

use super::output::{
    lifecycle_data, open_if_requested, os_asset_category_query, os_note, path_value,
    runtime_asset_query, trim_col,
};
use super::resolve::{
    read_flow_design, resolve_agent_dev, resolve_flow_file, resolve_mcp_dev, resolve_okf_dev,
    resolve_skill_dev,
};
use super::AssetCommandContext;

pub(super) async fn clone_asset(
    family: &'static str,
    url: String,
    root: PathBuf,
) -> anyhow::Result<AssetCommandOutput> {
    let result = asset_clone::clone_asset_source(family, url, root)
        .await
        .map_err(anyhow::Error::msg)?;
    let human = format!(
        "cloned {} asset source\nurl: {}\npath: {}\n",
        result.family,
        result.url,
        result.path.display()
    );
    Ok(AssetCommandOutput::new(
        json!({
            "family": result.family,
            "url": result.url,
            "path": path_value(&result.path),
        }),
        human,
    ))
}

pub(super) async fn list_assets(
    family: &'static str,
    category: &str,
    query: &str,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let session = load_os_session(context).await?;
    let scoped_query = os_asset_category_query(category, query);
    let result = panels::asset_resources::fetch_asset_list(
        &session.address,
        &session.access_token,
        &scoped_query,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let mut human = format!("{}\n", result.note);
    if result.rows.is_empty() {
        human.push_str("(no assets)\n");
    } else {
        human.push_str(&format!(
            "{:<28} {:<30} {:<12} {:<14} {:<12} updated\n",
            "id", "name", "category", "kind", "status"
        ));
    }
    for row in &result.rows {
        human.push_str(&format!(
            "{:<28} {:<30} {:<12} {:<14} {:<12} {}\n",
            trim_col(&row.id, 28),
            trim_col(&row.name, 30),
            trim_col(&row.category, 12),
            trim_col(&row.kind, 14),
            trim_col(&row.status, 12),
            row.updated
        ));
    }
    let rows = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row.id,
                "name": row.name,
                "category": row.category,
                "kind": row.kind,
                "status": row.status,
                "visibility": row.visibility,
                "owner": row.owner,
                "updated": row.updated,
                "accessUrl": row.access_url,
            })
        })
        .collect::<Vec<_>>();
    Ok(AssetCommandOutput::new(
        json!({
            "family": family,
            "location": "os",
            "query": query,
            "scopedQuery": scoped_query,
            "note": result.note,
            "assets": rows,
        }),
        human,
    ))
}

pub(super) async fn runtime_activity(
    family: &'static str,
    category: &str,
    query: &str,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let session = load_os_session(context).await?;
    let scoped_query = runtime_asset_query(category, "", query);
    let result = panels::asset_resources::fetch_runtime_activity(
        &session.address,
        &session.access_token,
        &scoped_query,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let mut human = format!("{}\n", result.note);
    if result.rows.is_empty() {
        human.push_str("(no runtime activity)\n");
    } else {
        human.push_str(&format!(
            "{:<28} {:<30} {:<12} {:<14} {:<12} updated\n",
            "id", "name", "category", "kind", "status"
        ));
    }
    for row in &result.rows {
        human.push_str(&format!(
            "{:<28} {:<30} {:<12} {:<14} {:<12} {}\n",
            trim_col(&row.id, 28),
            trim_col(&row.name, 30),
            trim_col(&row.asset_category, 12),
            trim_col(&row.kind, 14),
            trim_col(&row.status, 12),
            row.updated
        ));
    }
    let rows = result
        .rows
        .iter()
        .map(|row| {
            json!({
                "id": row.id,
                "name": row.name,
                "assetCategory": row.asset_category,
                "kind": row.kind,
                "status": row.status,
                "image": row.image,
                "accessUrl": row.access_url,
                "updated": row.updated,
                "source": row.source,
            })
        })
        .collect::<Vec<_>>();
    Ok(AssetCommandOutput::new(
        json!({
            "family": family,
            "query": query,
            "scopedQuery": scoped_query,
            "note": result.note,
            "activity": rows,
        }),
        human,
    ))
}

pub(super) async fn run_agent_os(
    action: panels::agent::AgentOsAction,
    path_arg: Option<&Path>,
    open_requested: bool,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let dev = resolve_agent_dev(path_arg.map(Path::to_path_buf), context)?;
    let session = load_os_session(context).await?;
    let result = panels::agent::publish_agent_to_os(session, dev, action)
        .await
        .map_err(anyhow::Error::msg)?;
    let opened_with = open_if_requested(open_requested && context.interactive, &result.view)?;
    let human = format!(
        "agent {} {}: {} ({})\n{}",
        result.action.label(),
        result.kind.label(),
        result.asset_name,
        result.asset_id,
        os_note(&result.note, &result.view, opened_with.as_deref())
    );
    Ok(AssetCommandOutput::new(
        lifecycle_data(
            "agent",
            result.action.label(),
            Some(result.kind.label()),
            &result.asset_name,
            &result.asset_id,
            &result.note,
            &result.view,
            opened_with,
        ),
        human,
    ))
}

pub(super) async fn run_mcp_os(
    action: panels::mcp::McpOsAction,
    path_arg: Option<&Path>,
    open_requested: bool,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let dev = resolve_mcp_dev(path_arg.map(Path::to_path_buf), context)?;
    let session = load_os_session(context).await?;
    let result = panels::mcp::publish_mcp_to_os(session, dev, action)
        .await
        .map_err(anyhow::Error::msg)?;
    let opened_with = open_if_requested(open_requested && context.interactive, &result.view)?;
    let human = format!(
        "mcp {}: {} ({})\n{}",
        result.action.label(),
        result.asset_name,
        result.asset_id,
        os_note(&result.note, &result.view, opened_with.as_deref())
    );
    Ok(AssetCommandOutput::new(
        lifecycle_data(
            "mcp",
            result.action.label(),
            None,
            &result.asset_name,
            &result.asset_id,
            &result.note,
            &result.view,
            opened_with,
        ),
        human,
    ))
}

pub(super) async fn run_skill_os(
    action: panels::skill::SkillOsAction,
    path_arg: Option<&Path>,
    open_requested: bool,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let dev = resolve_skill_dev(path_arg.map(Path::to_path_buf), context)?;
    let session = load_os_session(context).await?;
    let result = panels::skill::publish_skill_to_os(session, dev, action)
        .await
        .map_err(anyhow::Error::msg)?;
    let opened_with = open_if_requested(open_requested && context.interactive, &result.view)?;
    let human = format!(
        "skill {}: {} ({})\n{}",
        result.action.label(),
        result.asset_name,
        result.asset_id,
        os_note(&result.note, &result.view, opened_with.as_deref())
    );
    Ok(AssetCommandOutput::new(
        lifecycle_data(
            "skill",
            result.action.label(),
            None,
            &result.asset_name,
            &result.asset_id,
            &result.note,
            &result.view,
            opened_with,
        ),
        human,
    ))
}

pub(super) async fn run_flow_os(
    action: panels::flow::FlowOsAction,
    path_arg: Option<&Path>,
    open_requested: bool,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let flow = resolve_flow_file(path_arg.map(Path::to_path_buf), context)?;
    let design = read_flow_design(&flow.path)?;
    let session = load_os_session(context).await?;
    let result = panels::flow::publish_flow_to_os_with_local_path(
        session,
        flow.rel,
        Some(flow.path.clone()),
        design,
        action,
    )
    .await
    .map_err(anyhow::Error::msg)?;
    let opened_with = open_if_requested(open_requested && context.interactive, &result.view)?;
    let human = format!(
        "flow {}: {} ({})\n{}",
        result.action.label(),
        result.asset_name,
        result.asset_id,
        os_note(&result.note, &result.view, opened_with.as_deref())
    );
    Ok(AssetCommandOutput::new(
        lifecycle_data(
            "flow",
            result.action.label(),
            None,
            &result.asset_name,
            &result.asset_id,
            &result.note,
            &result.view,
            opened_with,
        ),
        human,
    ))
}

pub(super) async fn run_okf_os(
    action: panels::okf::OkfOsAction,
    path_arg: Option<&Path>,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let dev = resolve_okf_dev(path_arg.map(Path::to_path_buf), context)?;
    let session = load_os_session(context).await?;
    let result = panels::okf::publish_okf_to_os(session, dev, action)
        .await
        .map_err(anyhow::Error::msg)?;
    let human = format!(
        "okf {}: {} ({})\n{}",
        result.action.label(),
        result.asset_name,
        result.asset_id,
        os_note(&result.note, &result.view, None)
    );
    Ok(AssetCommandOutput::new(
        lifecycle_data(
            "okf",
            result.action.label(),
            None,
            &result.asset_name,
            &result.asset_id,
            &result.note,
            &result.view,
            None,
        ),
        human,
    ))
}

async fn load_os_session(
    context: &AssetCommandContext,
) -> anyhow::Result<crate::a3s_os::StoredOsSession> {
    let os_config = context.os_config.as_ref().ok_or_else(|| {
        anyhow::anyhow!("OS is not configured in the effective A3S ACL configuration")
    })?;
    let session = crate::a3s_os::current_session(os_config)
        .ok_or_else(|| anyhow::anyhow!("not signed in to OS; run `a3s auth login os`"))?;
    let session = if crate::a3s_os::needs_refresh(&session) {
        crate::a3s_os::refresh_session(&session).await?
    } else {
        session
    };
    Ok(session)
}
