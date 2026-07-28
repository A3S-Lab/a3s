use std::path::{Path, PathBuf};

#[cfg(test)]
use a3s_code_core::config::CodeConfig;
use a3s_code_core::config::OsConfig;
use serde_json::json;

use crate::cli::context::InvocationContext;
use crate::commands::code::asset_types::{
    AgentAssetKind, AgentAssetRequest, AssetCommandOutput, AssetListLocation, AssetListRequest,
    AssetQueryRequest, AssetRequest, FlowAssetRequest, McpAssetRequest, OkfAssetRequest,
    SkillAssetRequest,
};
use crate::tui::panels;

mod output;
mod remote;
mod resolve;
#[cfg(test)]
mod tests;

use output::{
    local_agents_output, local_flows_output, local_mcps_output, local_okf_output,
    local_skills_output, review_output,
};
#[cfg(test)]
pub(crate) use output::{os_asset_category_query, runtime_asset_query};
use remote::{
    clone_asset, list_assets, run_agent_os, run_flow_os, run_mcp_os, run_okf_os, run_skill_os,
    runtime_activity,
};
use resolve::{read_flow_design, resolve_flow_file, resolve_mcp_dev, resolve_skill_dev};
pub(crate) use resolve::{resolve_agent_dev, resolve_okf_dev};

pub(crate) async fn execute_asset_request(
    request: AssetRequest,
    invocation: &InvocationContext,
) -> anyhow::Result<AssetCommandOutput> {
    let context = AssetCommandContext::from_invocation(invocation)?;
    run_asset_request(request, &context).await
}

pub(crate) async fn run_asset_request(
    request: AssetRequest,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    match request {
        AssetRequest::Agent(request) => run_agent(request, context).await,
        AssetRequest::Mcp(request) => run_mcp(request, context).await,
        AssetRequest::Skill(request) => run_skill(request, context).await,
        AssetRequest::Flow(request) => run_flow(request, context).await,
        AssetRequest::Okf(request) => run_okf(request, context).await,
    }
}

pub(crate) struct AssetCommandContext {
    workspace: PathBuf,
    home: Option<PathBuf>,
    directories: crate::commands::config::CodeAssetDirectories,
    os_config: Option<OsConfig>,
    interactive: bool,
}

impl AssetCommandContext {
    fn from_invocation(invocation: &InvocationContext) -> anyhow::Result<Self> {
        let directories = crate::commands::config::code_asset_directories(invocation)?;
        let os_config = crate::commands::config::load_active_config(invocation)
            .ok()
            .and_then(|(_, config)| config.os);
        Ok(Self {
            workspace: invocation.directory.clone(),
            home: invocation.home.clone(),
            directories,
            os_config,
            interactive: invocation.output_mode() == crate::cli::args::OutputMode::Human,
        })
    }

    #[cfg(test)]
    pub(crate) fn from_process() -> anyhow::Result<Self> {
        let workspace = std::env::current_dir()?;
        let workspace_text = workspace.to_string_lossy();
        Ok(Self {
            workspace: workspace.clone(),
            home: crate::user_paths::user_home_dir(),
            directories: crate::commands::config::CodeAssetDirectories {
                agent: crate::config::agent_dir(),
                mcp: crate::config::mcp_dir(),
                skill: crate::config::skill_dir(),
                flow: crate::config::flow_dir(),
                okf: panels::okf::okf_package_dir(&workspace_text),
            },
            os_config: crate::config::find_config()
                .and_then(|path| CodeConfig::from_file(Path::new(&path)).ok())
                .and_then(|config| config.os),
            interactive: true,
        })
    }
}

async fn run_agent(
    request: AgentAssetRequest,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    match request {
        AgentAssetRequest::List(request) => {
            run_asset_list(
                "agent",
                "agent",
                request,
                &context.directories.agent,
                local_agents_output,
                context,
            )
            .await
        }
        AgentAssetRequest::Clone(request) => {
            clone_asset("agent", request.git_url, context.directories.agent.clone()).await
        }
        AgentAssetRequest::Review(request) => {
            let dev = resolve_agent_dev(request.path, context)?;
            let prompt = panels::agent::agent_review_prompt(&dev);
            Ok(review_output("agent", &dev.path, prompt))
        }
        AgentAssetRequest::Activity(request) => {
            runtime_activity("agent", "agent", query_text(&request), context).await
        }
        AgentAssetRequest::Publish { path, kind } => {
            run_agent_os(
                panels::agent::AgentOsAction::Publish(agent_os_kind(kind)),
                path.as_deref(),
                false,
                context,
            )
            .await
        }
        AgentAssetRequest::Run { path, kind } => {
            run_agent_os(
                panels::agent::AgentOsAction::Run(agent_os_kind(kind)),
                path.as_deref(),
                false,
                context,
            )
            .await
        }
        AgentAssetRequest::Deploy(request) => {
            run_agent_os(
                panels::agent::AgentOsAction::Deploy,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        AgentAssetRequest::Open { path, kind } => {
            run_agent_os(
                panels::agent::AgentOsAction::Open(agent_os_kind(kind)),
                path.as_deref(),
                true,
                context,
            )
            .await
        }
        AgentAssetRequest::Logs { path, kind } => {
            run_agent_os(
                panels::agent::AgentOsAction::Logs(agent_os_kind(kind)),
                path.as_deref(),
                false,
                context,
            )
            .await
        }
        AgentAssetRequest::Status { path, kind } => {
            run_agent_os(
                panels::agent::AgentOsAction::Status(agent_os_kind(kind)),
                path.as_deref(),
                false,
                context,
            )
            .await
        }
    }
}

async fn run_mcp(
    request: McpAssetRequest,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    match request {
        McpAssetRequest::List(request) => {
            run_asset_list(
                "mcp",
                "mcp",
                request,
                &context.directories.mcp,
                local_mcps_output,
                context,
            )
            .await
        }
        McpAssetRequest::Clone(request) => {
            clone_asset("mcp", request.git_url, context.directories.mcp.clone()).await
        }
        McpAssetRequest::Review(request) => {
            let dev = resolve_mcp_dev(request.path, context)?;
            let prompt = panels::mcp::mcp_review_prompt(&dev);
            Ok(review_output("mcp", &dev.path, prompt))
        }
        McpAssetRequest::Activity(request) => {
            runtime_activity("mcp", "mcp", query_text(&request), context).await
        }
        McpAssetRequest::Publish(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Publish,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        McpAssetRequest::Run(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Run,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        McpAssetRequest::Test(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Test,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        McpAssetRequest::Deploy(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Deploy,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        McpAssetRequest::Open(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Open,
                request.path.as_deref(),
                true,
                context,
            )
            .await
        }
        McpAssetRequest::Logs(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Logs,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        McpAssetRequest::Status(request) => {
            run_mcp_os(
                panels::mcp::McpOsAction::Status,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
    }
}

async fn run_skill(
    request: SkillAssetRequest,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    match request {
        SkillAssetRequest::List(request) => {
            run_asset_list(
                "skill",
                "skill",
                request,
                &context.directories.skill,
                local_skills_output,
                context,
            )
            .await
        }
        SkillAssetRequest::Clone(request) => {
            clone_asset("skill", request.git_url, context.directories.skill.clone()).await
        }
        SkillAssetRequest::Review(request) => {
            let dev = resolve_skill_dev(request.path, context)?;
            let body = std::fs::read_to_string(&dev.path)
                .map_err(|e| anyhow::anyhow!("could not read {}: {e}", dev.path.display()))?;
            let prompt = panels::skill::skill_review_prompt(&dev.path, &body);
            Ok(review_output("skill", &dev.path, prompt))
        }
        SkillAssetRequest::Activity(request) => {
            runtime_activity("skill", "skill", query_text(&request), context).await
        }
        SkillAssetRequest::Publish(request) => {
            run_skill_os(
                panels::skill::SkillOsAction::Publish,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        SkillAssetRequest::Deploy(request) => {
            run_skill_os(
                panels::skill::SkillOsAction::Deploy,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        SkillAssetRequest::Open(request) => {
            run_skill_os(
                panels::skill::SkillOsAction::Open,
                request.path.as_deref(),
                true,
                context,
            )
            .await
        }
        SkillAssetRequest::Status(request) => {
            run_skill_os(
                panels::skill::SkillOsAction::Status,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
    }
}

async fn run_flow(
    request: FlowAssetRequest,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    match request {
        FlowAssetRequest::List(request) => {
            run_asset_list(
                "flow",
                "workflow",
                request,
                &context.directories.flow,
                local_flows_output,
                context,
            )
            .await
        }
        FlowAssetRequest::Clone(request) => {
            clone_asset(
                "workflow",
                request.git_url,
                context.directories.flow.clone(),
            )
            .await
        }
        FlowAssetRequest::Review(request) => {
            let flow = resolve_flow_file(request.path, context)?;
            let design = read_flow_design(&flow.path)?;
            let prompt = panels::flow::flow_review_prompt(&flow.path, &design);
            Ok(review_output("flow", &flow.path, prompt))
        }
        FlowAssetRequest::Activity(request) => {
            runtime_activity("flow", "workflow", query_text(&request), context).await
        }
        FlowAssetRequest::Publish(request) => {
            run_flow_os(
                panels::flow::FlowOsAction::Publish,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        FlowAssetRequest::Run(request) => {
            run_flow_os(
                panels::flow::FlowOsAction::Run,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        FlowAssetRequest::Deploy(request) => {
            run_flow_os(
                panels::flow::FlowOsAction::Deploy,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        FlowAssetRequest::Open(request) => {
            run_flow_os(
                panels::flow::FlowOsAction::Open,
                request.path.as_deref(),
                true,
                context,
            )
            .await
        }
        FlowAssetRequest::Logs(request) => {
            run_flow_os(
                panels::flow::FlowOsAction::Logs,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
        FlowAssetRequest::Status(request) => {
            run_flow_os(
                panels::flow::FlowOsAction::Status,
                request.path.as_deref(),
                false,
                context,
            )
            .await
        }
    }
}

async fn run_okf(
    request: OkfAssetRequest,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    match request {
        OkfAssetRequest::List(request) => {
            run_asset_list(
                "okf",
                "knowledge",
                request,
                &context.directories.okf,
                local_okf_output,
                context,
            )
            .await
        }
        OkfAssetRequest::Clone(request) => {
            clone_asset("okf", request.git_url, context.directories.okf.clone()).await
        }
        OkfAssetRequest::Review(request) => {
            let dev = resolve_okf_dev(request.path, context)?;
            let signed_in = context
                .os_config
                .as_ref()
                .and_then(crate::a3s_os::current_session)
                .is_some();
            let prompt = panels::okf::okf_lifecycle_prompt("review", &dev, signed_in);
            Ok(review_output("okf", &dev.path, prompt))
        }
        OkfAssetRequest::Activity(request) => {
            runtime_activity("okf", "knowledge", query_text(&request), context).await
        }
        OkfAssetRequest::Publish(request) => {
            run_okf_os(
                panels::okf::OkfOsAction::Publish,
                request.path.as_deref(),
                context,
            )
            .await
        }
        OkfAssetRequest::Deploy(request) => {
            run_okf_os(
                panels::okf::OkfOsAction::Deploy,
                request.path.as_deref(),
                context,
            )
            .await
        }
        OkfAssetRequest::Status(request) => {
            run_okf_os(
                panels::okf::OkfOsAction::Status,
                request.path.as_deref(),
                context,
            )
            .await
        }
    }
}

async fn run_asset_list(
    family: &'static str,
    category: &'static str,
    request: AssetListRequest,
    local_root: &Path,
    local_list: fn(&str, &Path) -> AssetCommandOutput,
    context: &AssetCommandContext,
) -> anyhow::Result<AssetCommandOutput> {
    let query = request.query.as_deref().unwrap_or_default();
    match request.location {
        AssetListLocation::Local => Ok(local_list(query, local_root)),
        AssetListLocation::Os => list_assets(family, category, query, context).await,
        AssetListLocation::All => {
            let local = local_list(query, local_root);
            let os = list_assets(family, category, query, context).await?;
            Ok(AssetCommandOutput::new(
                json!({
                    "family": family,
                    "location": "all",
                    "query": request.query,
                    "local": local.data,
                    "os": os.data,
                }),
                format!("{}{}", local.human, os.human),
            ))
        }
    }
}

fn query_text(request: &AssetQueryRequest) -> &str {
    request.query.as_deref().unwrap_or_default()
}

fn agent_os_kind(kind: AgentAssetKind) -> panels::agent::AgentOsKind {
    match kind {
        AgentAssetKind::Agentic => panels::agent::AgentOsKind::Agentic,
        AgentAssetKind::Application => panels::agent::AgentOsKind::Application,
        AgentAssetKind::Tool => panels::agent::AgentOsKind::Tool,
    }
}
