use std::path::PathBuf;

#[derive(Clone, Debug)]
pub(crate) enum AssetRequest {
    Agent(AgentAssetRequest),
    Mcp(McpAssetRequest),
    Skill(SkillAssetRequest),
    Flow(FlowAssetRequest),
    Okf(OkfAssetRequest),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum AssetListLocation {
    Local,
    Os,
    All,
}

#[derive(Clone, Debug)]
pub(crate) struct AssetListRequest {
    pub(crate) location: AssetListLocation,
    pub(crate) query: Option<String>,
}

#[derive(Clone, Debug)]
pub(crate) struct AssetCloneRequest {
    pub(crate) git_url: String,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AssetPathRequest {
    pub(crate) path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default)]
pub(crate) struct AssetQueryRequest {
    pub(crate) query: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum AgentAssetKind {
    #[default]
    Agentic,
    Application,
    Tool,
}

#[derive(Clone, Debug)]
pub(crate) enum AgentAssetRequest {
    List(AssetListRequest),
    Clone(AssetCloneRequest),
    Review(AssetPathRequest),
    Activity(AssetQueryRequest),
    Publish {
        path: Option<PathBuf>,
        kind: AgentAssetKind,
    },
    Run {
        path: Option<PathBuf>,
        kind: AgentAssetKind,
    },
    Deploy(AssetPathRequest),
    Open {
        path: Option<PathBuf>,
        kind: AgentAssetKind,
    },
    Logs {
        path: Option<PathBuf>,
        kind: AgentAssetKind,
    },
    Status {
        path: Option<PathBuf>,
        kind: AgentAssetKind,
    },
}

#[derive(Clone, Debug)]
pub(crate) enum McpAssetRequest {
    List(AssetListRequest),
    Clone(AssetCloneRequest),
    Review(AssetPathRequest),
    Activity(AssetQueryRequest),
    Publish(AssetPathRequest),
    Run(AssetPathRequest),
    Test(AssetPathRequest),
    Deploy(AssetPathRequest),
    Open(AssetPathRequest),
    Logs(AssetPathRequest),
    Status(AssetPathRequest),
}

#[derive(Clone, Debug)]
pub(crate) enum SkillAssetRequest {
    List(AssetListRequest),
    Clone(AssetCloneRequest),
    Review(AssetPathRequest),
    Activity(AssetQueryRequest),
    Publish(AssetPathRequest),
    Deploy(AssetPathRequest),
    Open(AssetPathRequest),
    Status(AssetPathRequest),
}

#[derive(Clone, Debug)]
pub(crate) enum FlowAssetRequest {
    List(AssetListRequest),
    Clone(AssetCloneRequest),
    Review(AssetPathRequest),
    Activity(AssetQueryRequest),
    Publish(AssetPathRequest),
    Run(AssetPathRequest),
    Deploy(AssetPathRequest),
    Open(AssetPathRequest),
    Logs(AssetPathRequest),
    Status(AssetPathRequest),
}

#[derive(Clone, Debug)]
pub(crate) enum OkfAssetRequest {
    List(AssetListRequest),
    Clone(AssetCloneRequest),
    Review(AssetPathRequest),
    Activity(AssetQueryRequest),
    Publish(AssetPathRequest),
    Deploy(AssetPathRequest),
    Status(AssetPathRequest),
}

#[derive(Debug)]
pub(crate) struct AssetCommandOutput {
    pub(crate) data: serde_json::Value,
    pub(crate) human: String,
}

impl AssetCommandOutput {
    pub(crate) fn new(data: serde_json::Value, human: impl Into<String>) -> Self {
        Self {
            data,
            human: human.into(),
        }
    }
}
