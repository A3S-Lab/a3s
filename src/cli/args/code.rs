use std::ffi::OsString;
use std::path::PathBuf;

use clap::{Args, Subcommand, ValueEnum};

use super::{PassthroughArgs, TopArgs};

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct CodeArgs {
    #[command(subcommand)]
    pub command: Option<CodeCommand>,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum CodeCommand {
    /// Run one non-interactive coding task.
    Exec(CodeExecArgs),
    /// Resume the newest or selected interactive session.
    Resume(CodeResumeArgs),
    /// Gather evidence and create Markdown and HTML reports.
    #[command(alias = "deepresearch", alias = "deep-research")]
    Research(CodeResearchArgs),
    /// Inspect, export, or delete persisted sessions.
    Session(CodeSessionArgs),
    /// Manage Agent assets.
    Agent(AgentArgs),
    /// Manage MCP assets.
    Mcp(McpArgs),
    /// Manage Skill assets.
    Skill(SkillArgs),
    /// Manage Flow assets.
    Flow(FlowArgs),
    /// Manage OKF knowledge-package assets.
    Okf(OkfArgs),
    /// Manage the workspace knowledge base.
    Kb(KbArgs),
    /// Search or inspect durable context history.
    #[command(alias = "ctx")]
    Context(ContextArgs),
    /// Inspect long-term memory.
    #[command(alias = "mem")]
    Memory(MemoryArgs),

    /// Deprecated alias for top-level authentication.
    #[command(name = "login", hide = true)]
    LegacyLogin(LegacyLoginArgs),
    /// Deprecated alias for top-level authentication.
    #[command(name = "logout", hide = true)]
    LegacyLogout,
    /// Deprecated alias for top-level authentication.
    #[command(name = "auth", hide = true)]
    LegacyAuth(PassthroughArgs),
    /// Deprecated alias for top-level configuration.
    #[command(name = "config", hide = true)]
    LegacyConfig(PassthroughArgs),
    /// Deprecated alias for `a3s config paths`.
    #[command(name = "dirs", hide = true)]
    LegacyDirs,
    /// Deprecated alias for `a3s model list`.
    #[command(name = "models", hide = true)]
    LegacyModels,
    /// Deprecated alias for top-level model commands.
    #[command(name = "model", hide = true)]
    LegacyModel(PassthroughArgs),
    /// Deprecated alias for `a3s top`.
    #[command(name = "top", hide = true)]
    LegacyTop(TopArgs),
    /// Deprecated alias for `a3s self update`.
    #[command(name = "update", hide = true)]
    LegacyUpdate,
    /// Removed alias for A3S Web.
    #[command(name = "serve", hide = true)]
    RemovedServe(PassthroughArgs),
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct CodeExecArgs {
    /// Prompt text. Quote multi-word prompts as one shell argument.
    #[arg(value_name = "PROMPT", conflicts_with = "prompt_file")]
    pub prompt: Option<String>,

    /// Read the prompt from a UTF-8 file.
    #[arg(long, value_name = "PATH", conflicts_with = "prompt")]
    pub prompt_file: Option<PathBuf>,

    /// Select planning or normal execution behavior.
    #[arg(long, value_enum, default_value_t = CodeMode::Default)]
    pub mode: CodeMode,

    /// Override the configured model for this execution.
    #[arg(long, value_name = "PROVIDER/MODEL")]
    pub model: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum CodeMode {
    Plan,
    #[default]
    Default,
    Auto,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct CodeResumeArgs {
    #[arg(value_name = "SESSION_ID")]
    pub session_id: Option<String>,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct CodeResearchArgs {
    /// Research question. Multiple shell words are joined for compatibility.
    #[arg(value_name = "QUERY", required = true)]
    pub query: Vec<String>,

    /// Restrict evidence collection to the workspace and other local sources.
    #[arg(long, conflicts_with = "web")]
    pub local_only: bool,

    /// Explicitly allow web and workspace evidence, overriding query wording.
    #[arg(long, conflicts_with = "local_only")]
    pub web: bool,

    /// Directory that should receive generated report artifacts.
    #[arg(long, value_name = "PATH")]
    pub report_dir: Option<PathBuf>,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct CodeSessionArgs {
    #[command(subcommand)]
    pub command: CodeSessionCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum CodeSessionCommand {
    /// List sessions in the effective workspace.
    List,
    /// Show one session document.
    Show(SessionIdArgs),
    /// Export one session document.
    Export(SessionExportArgs),
    /// Delete one session document without touching workspace files.
    Delete(SessionDeleteArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct SessionIdArgs {
    #[arg(value_name = "SESSION_ID")]
    pub session_id: String,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct SessionExportArgs {
    #[arg(value_name = "SESSION_ID")]
    pub session_id: String,
    #[arg(long, value_name = "PATH")]
    pub output_file: Option<PathBuf>,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct SessionDeleteArgs {
    #[arg(value_name = "SESSION_ID")]
    pub session_id: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct AgentArgs {
    #[command(subcommand)]
    pub command: AgentCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum AgentCommand {
    List(AssetListArgs),
    Clone(AssetCloneArgs),
    Review(AssetPathArgs),
    Activity(AssetQueryArgs),
    Publish(AgentPublishArgs),
    Run(AgentActionArgs),
    Deploy(AssetPathArgs),
    Open(AgentActionArgs),
    Logs(AgentActionArgs),
    Status(AgentActionArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct AgentPublishArgs {
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub kind: AgentKind,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct AgentActionArgs {
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
    #[arg(long, value_enum)]
    pub kind: Option<AgentKind>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum AgentKind {
    Agentic,
    Application,
    Tool,
}

macro_rules! asset_family {
    ($args:ident, $command:ident, [$($verb:ident),+ $(,)?]) => {
        #[derive(Clone, Debug, Args)]
        #[command(subcommand_required = true, arg_required_else_help = true)]
        pub(crate) struct $args {
            #[command(subcommand)]
            pub command: $command,
        }

        #[derive(Clone, Debug, Subcommand)]
        pub(crate) enum $command {
            List(AssetListArgs),
            Clone(AssetCloneArgs),
            Review(AssetPathArgs),
            Activity(AssetQueryArgs),
            $($verb(AssetPathArgs)),+
        }
    };
}

asset_family!(
    McpArgs,
    McpCommand,
    [Publish, Run, Test, Deploy, Open, Logs, Status]
);
asset_family!(SkillArgs, SkillCommand, [Publish, Deploy, Open, Status]);
asset_family!(
    FlowArgs,
    FlowCommand,
    [Publish, Run, Deploy, Open, Logs, Status]
);
asset_family!(OkfArgs, OkfCommand, [Publish, Deploy, Status]);

#[derive(Clone, Debug, Args)]
pub(crate) struct AssetListArgs {
    #[arg(long, value_enum)]
    pub location: AssetLocation,
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum AssetLocation {
    Local,
    Os,
    All,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct AssetCloneArgs {
    #[arg(value_name = "GIT_URL")]
    pub git_url: String,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct AssetPathArgs {
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct AssetQueryArgs {
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct KbArgs {
    #[command(subcommand)]
    pub command: KbCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum KbCommand {
    Stats,
    Add(KbTextArgs),
    Import(KbImportArgs),
    Search(KbTextArgs),
    Path,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct KbTextArgs {
    #[arg(value_name = "TEXT")]
    pub text: String,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct KbImportArgs {
    #[arg(value_name = "FILE_OR_DIRECTORY")]
    pub path: PathBuf,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct ContextArgs {
    #[command(subcommand)]
    pub command: ContextCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum ContextCommand {
    Search(ContextQueryArgs),
    Show(ContextShowArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ContextQueryArgs {
    #[arg(value_name = "QUERY")]
    pub query: String,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct ContextShowArgs {
    #[command(subcommand)]
    pub command: ContextShowCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum ContextShowCommand {
    Event(ContextEventArgs),
    Session(SessionIdArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ContextEventArgs {
    #[arg(value_name = "EVENT_ID")]
    pub event_id: String,
    #[arg(long, default_value_t = 3)]
    pub window: usize,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct MemoryArgs {
    #[command(subcommand)]
    pub command: MemoryCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum MemoryCommand {
    List(MemoryListArgs),
    Stats,
    Path,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct MemoryListArgs {
    #[arg(value_name = "QUERY")]
    pub query: Option<String>,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct LegacyLoginArgs {
    /// Captures unsafe legacy positional credentials for redacted rejection.
    #[arg(value_name = "LEGACY_TOKEN", hide = true)]
    pub values: Vec<OsString>,
}
