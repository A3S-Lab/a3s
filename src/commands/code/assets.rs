use crate::cli::args::{
    AgentArgs, AgentCommand, AgentKind, AssetCloneArgs, AssetListArgs, AssetLocation,
    AssetPathArgs, AssetQueryArgs, FlowArgs, FlowCommand, McpArgs, McpCommand, OkfArgs, OkfCommand,
    SkillArgs, SkillCommand,
};
use crate::cli::context::InvocationContext;
use crate::cli::output::render_value;

use super::asset_types::{
    AgentAssetKind, AgentAssetRequest, AssetCloneRequest, AssetCommandOutput, AssetListLocation,
    AssetListRequest, AssetPathRequest, AssetQueryRequest, AssetRequest, FlowAssetRequest,
    McpAssetRequest, OkfAssetRequest, SkillAssetRequest,
};

pub(super) async fn run_agent(args: AgentArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let (command_name, request) = match args.command {
        AgentCommand::List(args) => (
            "code.agent.list",
            AgentAssetRequest::List(list_request(args)),
        ),
        AgentCommand::Clone(args) => (
            "code.agent.clone",
            AgentAssetRequest::Clone(clone_request(args)),
        ),
        AgentCommand::Review(args) => (
            "code.agent.review",
            AgentAssetRequest::Review(path_request(args)),
        ),
        AgentCommand::Activity(args) => (
            "code.agent.activity",
            AgentAssetRequest::Activity(query_request(args)),
        ),
        AgentCommand::Publish(args) => (
            "code.agent.publish",
            AgentAssetRequest::Publish {
                path: args.path,
                kind: agent_kind(args.kind),
            },
        ),
        AgentCommand::Run(args) => (
            "code.agent.run",
            AgentAssetRequest::Run {
                path: args.path,
                kind: optional_agent_kind_value(args.kind),
            },
        ),
        AgentCommand::Deploy(args) => (
            "code.agent.deploy",
            AgentAssetRequest::Deploy(path_request(args)),
        ),
        AgentCommand::Open(args) => (
            "code.agent.open",
            AgentAssetRequest::Open {
                path: args.path,
                kind: optional_agent_kind_value(args.kind),
            },
        ),
        AgentCommand::Logs(args) => (
            "code.agent.logs",
            AgentAssetRequest::Logs {
                path: args.path,
                kind: optional_agent_kind_value(args.kind),
            },
        ),
        AgentCommand::Status(args) => (
            "code.agent.status",
            AgentAssetRequest::Status {
                path: args.path,
                kind: optional_agent_kind_value(args.kind),
            },
        ),
    };
    execute(command_name, AssetRequest::Agent(request), context).await
}

pub(super) async fn run_mcp(args: McpArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let (command_name, request) = match args.command {
        McpCommand::List(args) => ("code.mcp.list", McpAssetRequest::List(list_request(args))),
        McpCommand::Clone(args) => (
            "code.mcp.clone",
            McpAssetRequest::Clone(clone_request(args)),
        ),
        McpCommand::Review(args) => (
            "code.mcp.review",
            McpAssetRequest::Review(path_request(args)),
        ),
        McpCommand::Activity(args) => (
            "code.mcp.activity",
            McpAssetRequest::Activity(query_request(args)),
        ),
        McpCommand::Publish(args) => (
            "code.mcp.publish",
            McpAssetRequest::Publish(path_request(args)),
        ),
        McpCommand::Run(args) => ("code.mcp.run", McpAssetRequest::Run(path_request(args))),
        McpCommand::Test(args) => ("code.mcp.test", McpAssetRequest::Test(path_request(args))),
        McpCommand::Deploy(args) => (
            "code.mcp.deploy",
            McpAssetRequest::Deploy(path_request(args)),
        ),
        McpCommand::Open(args) => ("code.mcp.open", McpAssetRequest::Open(path_request(args))),
        McpCommand::Logs(args) => ("code.mcp.logs", McpAssetRequest::Logs(path_request(args))),
        McpCommand::Status(args) => (
            "code.mcp.status",
            McpAssetRequest::Status(path_request(args)),
        ),
    };
    execute(command_name, AssetRequest::Mcp(request), context).await
}

pub(super) async fn run_skill(args: SkillArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let (command_name, request) = match args.command {
        SkillCommand::List(args) => (
            "code.skill.list",
            SkillAssetRequest::List(list_request(args)),
        ),
        SkillCommand::Clone(args) => (
            "code.skill.clone",
            SkillAssetRequest::Clone(clone_request(args)),
        ),
        SkillCommand::Review(args) => (
            "code.skill.review",
            SkillAssetRequest::Review(path_request(args)),
        ),
        SkillCommand::Activity(args) => (
            "code.skill.activity",
            SkillAssetRequest::Activity(query_request(args)),
        ),
        SkillCommand::Publish(args) => (
            "code.skill.publish",
            SkillAssetRequest::Publish(path_request(args)),
        ),
        SkillCommand::Deploy(args) => (
            "code.skill.deploy",
            SkillAssetRequest::Deploy(path_request(args)),
        ),
        SkillCommand::Open(args) => (
            "code.skill.open",
            SkillAssetRequest::Open(path_request(args)),
        ),
        SkillCommand::Status(args) => (
            "code.skill.status",
            SkillAssetRequest::Status(path_request(args)),
        ),
    };
    execute(command_name, AssetRequest::Skill(request), context).await
}

pub(super) async fn run_flow(args: FlowArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let (command_name, request) = match args.command {
        FlowCommand::List(args) => ("code.flow.list", FlowAssetRequest::List(list_request(args))),
        FlowCommand::Clone(args) => (
            "code.flow.clone",
            FlowAssetRequest::Clone(clone_request(args)),
        ),
        FlowCommand::Review(args) => (
            "code.flow.review",
            FlowAssetRequest::Review(path_request(args)),
        ),
        FlowCommand::Activity(args) => (
            "code.flow.activity",
            FlowAssetRequest::Activity(query_request(args)),
        ),
        FlowCommand::Publish(args) => (
            "code.flow.publish",
            FlowAssetRequest::Publish(path_request(args)),
        ),
        FlowCommand::Run(args) => ("code.flow.run", FlowAssetRequest::Run(path_request(args))),
        FlowCommand::Deploy(args) => (
            "code.flow.deploy",
            FlowAssetRequest::Deploy(path_request(args)),
        ),
        FlowCommand::Open(args) => ("code.flow.open", FlowAssetRequest::Open(path_request(args))),
        FlowCommand::Logs(args) => ("code.flow.logs", FlowAssetRequest::Logs(path_request(args))),
        FlowCommand::Status(args) => (
            "code.flow.status",
            FlowAssetRequest::Status(path_request(args)),
        ),
    };
    execute(command_name, AssetRequest::Flow(request), context).await
}

pub(super) async fn run_okf(args: OkfArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let (command_name, request) = match args.command {
        OkfCommand::List(args) => ("code.okf.list", OkfAssetRequest::List(list_request(args))),
        OkfCommand::Clone(args) => (
            "code.okf.clone",
            OkfAssetRequest::Clone(clone_request(args)),
        ),
        OkfCommand::Review(args) => (
            "code.okf.review",
            OkfAssetRequest::Review(path_request(args)),
        ),
        OkfCommand::Activity(args) => (
            "code.okf.activity",
            OkfAssetRequest::Activity(query_request(args)),
        ),
        OkfCommand::Publish(args) => (
            "code.okf.publish",
            OkfAssetRequest::Publish(path_request(args)),
        ),
        OkfCommand::Deploy(args) => (
            "code.okf.deploy",
            OkfAssetRequest::Deploy(path_request(args)),
        ),
        OkfCommand::Status(args) => (
            "code.okf.status",
            OkfAssetRequest::Status(path_request(args)),
        ),
    };
    execute(command_name, AssetRequest::Okf(request), context).await
}

async fn execute(
    command_name: &'static str,
    request: AssetRequest,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    let output = super::asset_runtime::execute_asset_request(request, context).await?;
    render(command_name, output, context)
}

fn render(
    command_name: &'static str,
    output: AssetCommandOutput,
    context: &InvocationContext,
) -> anyhow::Result<()> {
    render_value(context.output_mode(), command_name, output.data, || {
        print!("{}", output.human)
    })
}

fn list_request(args: AssetListArgs) -> AssetListRequest {
    let location = match args.location {
        AssetLocation::Local => AssetListLocation::Local,
        AssetLocation::Os => AssetListLocation::Os,
        AssetLocation::All => AssetListLocation::All,
    };
    AssetListRequest {
        location,
        query: args.query,
    }
}

fn clone_request(args: AssetCloneArgs) -> AssetCloneRequest {
    AssetCloneRequest {
        git_url: args.git_url,
    }
}

fn path_request(args: AssetPathArgs) -> AssetPathRequest {
    AssetPathRequest { path: args.path }
}

fn query_request(args: AssetQueryArgs) -> AssetQueryRequest {
    AssetQueryRequest { query: args.query }
}

fn optional_agent_kind_value(kind: Option<AgentKind>) -> AgentAssetKind {
    kind.map(agent_kind).unwrap_or_default()
}

fn agent_kind(kind: AgentKind) -> AgentAssetKind {
    match kind {
        AgentKind::Agentic => AgentAssetKind::Agentic,
        AgentKind::Application => AgentAssetKind::Application,
        AgentKind::Tool => AgentAssetKind::Tool,
    }
}
