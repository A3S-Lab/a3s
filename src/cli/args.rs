use std::ffi::OsString;
use std::path::PathBuf;

use a3s::components::ComponentId;
use clap::{Args, Parser, Subcommand, ValueEnum};

mod code;
pub(crate) use code::*;
mod admin;
pub(crate) use admin::*;

#[derive(Debug, Parser)]
#[command(
    name = "a3s",
    version,
    about = "A3S agent platform CLI",
    propagate_version = true,
    disable_help_subcommand = true
)]
pub(crate) struct Cli {
    /// Run as if A3S was started in this directory.
    #[arg(short = 'C', long, global = true, value_name = "PATH")]
    pub directory: Option<PathBuf>,

    /// Use one explicit A3S ACL configuration file.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<PathBuf>,

    /// Select human, JSON, or JSONL output for root-owned commands.
    #[arg(long, global = true, value_enum, default_value_t = OutputMode::Human)]
    pub output: OutputMode,

    /// Shorthand for --output json.
    #[arg(long, global = true, conflicts_with = "output")]
    pub json: bool,

    /// Suppress nonessential human output.
    #[arg(short, long, global = true, conflicts_with = "verbose")]
    pub quiet: bool,

    /// Increase diagnostic detail; repeat for more detail.
    #[arg(short, long, global = true, action = clap::ArgAction::Count)]
    pub verbose: u8,

    /// Control terminal color.
    #[arg(long, global = true, value_enum, default_value_t = ColorMode::Auto)]
    pub color: ColorMode,

    /// Disable progress bars and spinners.
    #[arg(long, global = true)]
    pub no_progress: bool,

    /// Disable network access and first-use downloads.
    #[arg(long, global = true)]
    pub offline: bool,

    /// Never prompt for input.
    #[arg(long, global = true)]
    pub non_interactive: bool,

    #[command(subcommand)]
    pub command: Option<RootCommand>,
}

impl Cli {
    pub(crate) fn output_mode(&self) -> OutputMode {
        if self.json {
            OutputMode::Json
        } else {
            self.output
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum OutputMode {
    #[default]
    Human,
    Json,
    Jsonl,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum ColorMode {
    #[default]
    Auto,
    Always,
    Never,
}

#[derive(Debug, Subcommand)]
pub(crate) enum RootCommand {
    /// Launch or automate the A3S coding agent.
    Code(CodeArgs),
    /// Run the local A3S Web application and API.
    Web(WebArgs),
    /// Monitor agents, containers, sessions, and events.
    Top(TopArgs),
    /// Run the registered A3S Box product.
    Box(ProxyArgs),
    /// Manage a multi-service application with A3S Box.
    Compose(ProxyArgs),
    /// Create and start the current Compose application.
    Up(ProxyArgs),
    /// Stop and remove the current Compose application.
    Down(ProxyArgs),
    /// List services in the current Compose application.
    Ps(ProxyArgs),
    /// View logs from the current Compose application.
    Logs(ProxyArgs),
    /// Run the registered A3S Bench product.
    Bench(ProxyArgs),
    /// Run the registered A3S Search product.
    Search(ProxyArgs),
    /// Use Browser, Office, or an installed A3S Use extension.
    Use(ProxyArgs),
    /// Manage account authentication.
    Auth(AuthArgs),
    /// Discover and select runtime models.
    Model(ModelArgs),
    /// Inspect and edit A3S ACL configuration.
    Config(ConfigArgs),
    /// List registered components and discovered external tools.
    List(ListArgs),
    /// Show component status, sources, and available versions.
    Info(InfoArgs),
    /// Install or repair registered components.
    Install(InstallArgs),
    /// List or apply component upgrades.
    Upgrade(UpgradeArgs),
    /// Remove only component-owned files.
    Uninstall(UninstallArgs),
    /// Run read-only installation and health diagnostics.
    Doctor(DoctorArgs),
    /// Manage trusted component registries.
    Registry(RegistryArgs),
    /// Inspect or remove recreatable cache data.
    Cache(CacheArgs),
    /// Manage the A3S executable itself.
    #[command(name = "self")]
    Self_(SelfArgs),
    /// Print A3S version information.
    Version,
    /// Generate shell completion source.
    Completion(CompletionArgs),
    /// Show help for a command path.
    Help(HelpArgs),
    /// Compatibility route for the former overloaded update command.
    #[command(name = "update", hide = true)]
    LegacyUpdate(PassthroughArgs),
}

#[derive(Clone, Debug, Args)]
#[command(trailing_var_arg = true, disable_help_flag = true)]
pub(crate) struct PassthroughArgs {
    #[arg(allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}

#[derive(Clone, Debug, Args)]
#[command(
    trailing_var_arg = true,
    disable_help_flag = true,
    disable_version_flag = true
)]
pub(crate) struct ProxyArgs {
    #[arg(allow_hyphen_values = true)]
    pub args: Vec<OsString>,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct WebArgs {
    #[command(subcommand)]
    pub command: Option<WebCommand>,

    #[command(flatten)]
    pub shortcut: WebStartArgs,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum WebCommand {
    /// Start A3S Web in the foreground or as a managed instance.
    Start(WebStartArgs),
    /// Stop the managed instance for the effective workspace.
    Stop(WebTargetArgs),
    /// Inspect the managed instance for the effective workspace.
    Status(WebTargetArgs),
    /// Read or follow the managed instance log.
    Logs(WebLogsArgs),
    /// Open the managed instance in the default browser.
    Open(WebTargetArgs),
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct WebStartArgs {
    /// Run as a managed background instance.
    #[arg(short = 'd', long = "detach")]
    pub detach: bool,

    /// Gracefully replace a verified A3S Web instance; never stop an unrelated process.
    #[arg(long)]
    pub replace: bool,

    /// Listen host. Defaults to A3S_CODE_WEB_HOST or 127.0.0.1.
    #[arg(long, value_name = "HOST")]
    pub host: Option<String>,

    /// Listen port. Use 0 to select an available port.
    #[arg(long, value_name = "PORT")]
    pub port: Option<u16>,

    /// Deprecated workspace spelling; use global --directory/-C.
    #[arg(short = 'w', long = "workspace", value_name = "PATH", hide = true)]
    pub legacy_workspace: Option<PathBuf>,

    /// Directory containing built Web assets.
    #[arg(long, value_name = "PATH")]
    pub web_dir: Option<PathBuf>,

    /// Serve only the API without Web assets.
    #[arg(long)]
    pub api_only: bool,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct WebTargetArgs {
    /// Deprecated workspace spelling; use global --directory/-C.
    #[arg(short = 'w', long = "workspace", value_name = "PATH", hide = true)]
    pub legacy_workspace: Option<PathBuf>,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct WebLogsArgs {
    #[command(flatten)]
    pub target: WebTargetArgs,

    /// Continue printing appended log data until interrupted.
    #[arg(short, long)]
    pub follow: bool,

    /// Number of existing lines to print before following.
    #[arg(short = 'n', long, default_value_t = 100)]
    pub lines: usize,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct TopArgs {
    /// Focus one container by name or ID.
    #[arg(long, value_name = "CONTAINER", conflicts_with = "legacy_container")]
    pub container: Option<String>,

    /// Select the initial monitor view.
    #[arg(long, value_enum, conflicts_with = "legacy_view")]
    pub view: Option<TopView>,

    /// Select the container runtime connector.
    #[arg(long, value_enum)]
    pub connector: Option<TopConnector>,

    /// Show only active containers.
    #[arg(short = 'a', long, alias = "active-only", conflicts_with = "all")]
    pub active: bool,

    /// Include stopped containers.
    #[arg(long, conflicts_with = "active")]
    pub all: bool,

    /// Filter visible rows.
    #[arg(short, long, value_name = "TEXT")]
    pub filter: Option<String>,

    /// Select the sort field.
    #[arg(short, long, value_enum)]
    pub sort: Option<TopSort>,

    /// Reverse the selected sort order.
    #[arg(short, long)]
    pub reverse: bool,

    /// Filter by risk level.
    #[arg(long, value_enum)]
    pub risk: Option<TopRisk>,

    /// Filter observer events by kind.
    #[arg(long, value_enum)]
    pub kind: Option<TopEventKind>,

    /// Emit repeated machine snapshots.
    #[arg(long)]
    pub watch: bool,

    /// Snapshot or refresh interval, for example 1500ms or 2s.
    #[arg(long, value_name = "DURATION")]
    pub interval: Option<String>,

    /// Stop after this many machine snapshots.
    #[arg(long, value_name = "COUNT", requires = "watch")]
    pub count: Option<usize>,

    /// Restore the compact column set.
    #[arg(long, alias = "compact-columns")]
    pub compact: bool,

    /// Hide table headers.
    #[arg(long)]
    pub no_header: bool,

    /// Invert terminal colors.
    #[arg(short, long)]
    pub invert: bool,

    /// Deprecated positional container shorthand. A duration after --watch is
    /// interpreted as the former combined watch grammar.
    #[arg(value_name = "LEGACY_CONTAINER", hide = true)]
    pub legacy_container: Option<String>,

    #[arg(long, hide = true, group = "legacy_view")]
    pub agents: bool,
    #[arg(long = "sessions", hide = true, group = "legacy_view")]
    pub view_sessions: bool,
    #[arg(long = "containers", hide = true, group = "legacy_view")]
    pub view_containers: bool,
    #[arg(long = "processes", hide = true, group = "legacy_view")]
    pub view_processes: bool,
    #[arg(long = "events", hide = true, group = "legacy_view")]
    pub view_events: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum TopView {
    Agents,
    Sessions,
    Containers,
    Processes,
    Events,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum TopConnector {
    #[value(name = "a3s-box")]
    A3sBox,
    Docker,
    Runc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum TopSort {
    Cpu,
    Mem,
    Net,
    Block,
    Pids,
    State,
    Id,
    Uptime,
    Name,
    Tokens,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum TopRisk {
    All,
    Medium,
    High,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum TopEventKind {
    All,
    Tool,
    Security,
    File,
    Egress,
    Llm,
    Other,
}

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct AuthArgs {
    #[command(subcommand)]
    pub command: AuthCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum AuthCommand {
    /// List managed and discovered account providers.
    List,
    /// Show account and credential status.
    Status(AuthProviderArgs),
    /// Sign in through OAuth or protected token input.
    Login(AuthLoginArgs),
    /// Remove the stored managed session.
    Logout(AuthProviderArgs),
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct AuthProviderArgs {
    /// Authentication provider. The initial managed provider is os.
    #[arg(value_name = "PROVIDER", default_value = "os", value_parser = ["os"])]
    pub provider: String,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct AuthLoginArgs {
    /// Authentication provider. Values other than os are treated as unsafe
    /// legacy positional input and are never echoed.
    #[arg(value_name = "PROVIDER")]
    pub provider_or_legacy: Option<OsString>,

    /// Read an existing bearer token from standard input.
    #[arg(long, conflicts_with = "token_file")]
    pub token_stdin: bool,

    /// Read an existing bearer token from a protected file.
    #[arg(long, value_name = "PATH", conflicts_with = "token_stdin")]
    pub token_file: Option<PathBuf>,

    /// Captures unsafe legacy positional credentials so they can be rejected
    /// without Clap reflecting their values in an error message.
    #[arg(value_name = "LEGACY_TOKEN", hide = true)]
    pub legacy_values: Vec<OsString>,
}

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct ModelArgs {
    #[command(subcommand)]
    pub command: ModelCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ModelCommand {
    /// List configured and compatible account-backed models.
    List,
    /// Show the effective default model.
    Current,
    /// Select and persist a validated default model.
    Use(ModelUseArgs),
    /// Remove the selected default model from one config layer.
    Reset(ModelScopeArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ModelUseArgs {
    /// Source-qualified model ID, for example openai/gpt-5.
    #[arg(value_name = "PROVIDER/MODEL")]
    pub model: String,

    #[command(flatten)]
    pub target: ModelScopeArgs,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ModelScopeArgs {
    /// ACL layer to update when --config is not present.
    #[arg(long, value_enum, default_value_t = ConfigScope::User)]
    pub scope: ConfigScope,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum ConfigScope {
    Workspace,
    #[default]
    User,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ConfigScopeArgs {
    /// ACL layer to use when --config is not present.
    #[arg(long, value_enum, default_value_t = ConfigScope::User)]
    pub scope: ConfigScope,
}

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct ConfigArgs {
    #[command(subcommand)]
    pub command: ConfigCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum ConfigCommand {
    /// Print the active ACL config path.
    Path,
    /// Print A3S config, data, state, cache, and asset paths.
    Paths,
    /// Print an effective redacted configuration summary.
    Show,
    /// Create a starter A3S ACL configuration.
    Init(ConfigInitArgs),
    /// Open an ACL configuration in VISUAL or EDITOR.
    Edit(ConfigScopeArgs),
    /// Parse and validate an ACL configuration.
    Validate(ConfigValidateArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct ConfigInitArgs {
    /// ACL layer to create when --config is not present.
    #[arg(long, value_enum, default_value_t = ConfigScope::User)]
    pub scope: ConfigScope,
    /// Replace an existing config with the starter template.
    #[arg(long)]
    pub force: bool,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct ConfigValidateArgs {
    /// Config path. Defaults to the active config.
    #[arg(value_name = "PATH")]
    pub path: Option<PathBuf>,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct ListArgs {
    /// Show only installed or otherwise present components.
    #[arg(long, conflicts_with = "available")]
    pub installed: bool,
    /// Show only missing components that can be installed.
    #[arg(long, conflicts_with = "installed")]
    pub available: bool,
    /// Query release sources and show available upgrades.
    #[arg(long)]
    pub updates: bool,
    /// Filter by component kind.
    #[arg(long, value_enum)]
    pub kind: Option<ComponentKindArg>,
}

#[derive(Clone, Debug, Default, Args)]
#[command(disable_version_flag = true)]
pub(crate) struct InstallArgs {
    /// Registered component IDs.
    #[arg(value_name = "COMPONENT")]
    pub components: Vec<ComponentId>,
    /// Install one exact component version.
    #[arg(long, value_name = "VERSION")]
    pub version: Option<String>,
    /// Select a supported source.
    #[arg(long, value_name = "SOURCE")]
    pub source: Option<String>,
    /// Select a release channel.
    #[arg(long, value_enum, default_value_t = ReleaseChannelArg::Stable)]
    pub channel: ReleaseChannelArg,
    /// Select user or system ownership scope.
    #[arg(long, value_enum, default_value_t = InstallScopeArg::User)]
    pub scope: InstallScopeArg,
    /// Install an explicit local package.
    #[arg(long = "from", value_name = "PATH")]
    pub package: Option<PathBuf>,
    /// Repair or reinstall using current provenance.
    #[arg(long)]
    pub force: bool,
    /// Permit an explicit provenance or scope migration.
    #[arg(long)]
    pub migrate: bool,
    /// Resolve and print the operation plan without mutation.
    #[arg(long)]
    pub dry_run: bool,
    /// Apply only if the newly resolved plan matches this reviewed SHA-256 digest.
    #[arg(
        long,
        value_name = "SHA256",
        conflicts_with = "dry_run",
        value_parser = parse_plan_digest
    )]
    pub plan_digest: Option<String>,
    /// Explicitly trust an unsigned local development package.
    #[arg(long)]
    pub allow_unsigned: bool,
    /// Accept the operation plan without prompting.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct UpgradeArgs {
    /// Managed component IDs. With no IDs, only list available upgrades.
    #[arg(value_name = "COMPONENT", conflicts_with = "all")]
    pub components: Vec<ComponentId>,
    /// Upgrade every eligible managed component.
    #[arg(long)]
    pub all: bool,
    /// Accept the operation plan without prompting.
    #[arg(long)]
    pub yes: bool,
    /// Resolve and print the operation plan without mutation.
    #[arg(long)]
    pub dry_run: bool,
    /// Apply only if the newly resolved plan matches this reviewed SHA-256 digest.
    #[arg(
        long,
        value_name = "SHA256",
        conflicts_with = "dry_run",
        value_parser = parse_plan_digest
    )]
    pub plan_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct UninstallArgs {
    /// Registered component IDs.
    #[arg(value_name = "COMPONENT", required = true)]
    pub components: Vec<ComponentId>,
    /// Remove managed children before their parent.
    #[arg(long)]
    pub cascade: bool,
    /// Also remove component-owned cache and runtime state.
    #[arg(long)]
    pub purge: bool,
    /// Accept the operation plan without prompting.
    #[arg(long)]
    pub yes: bool,
    /// Resolve and print the operation plan without mutation.
    #[arg(long)]
    pub dry_run: bool,
    /// Apply only if the newly resolved plan matches this reviewed SHA-256 digest.
    #[arg(
        long,
        value_name = "SHA256",
        conflicts_with = "dry_run",
        value_parser = parse_plan_digest
    )]
    pub plan_digest: Option<String>,
}

fn parse_plan_digest(value: &str) -> Result<String, String> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err("expected exactly 64 lowercase hexadecimal characters".to_string());
    }
    Ok(value.to_string())
}

#[derive(Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct SelfArgs {
    #[command(subcommand)]
    pub command: SelfCommand,
}

#[derive(Debug, Subcommand)]
pub(crate) enum SelfCommand {
    /// Check for and install a newer A3S CLI release.
    Update(SelfUpdateArgs),
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct SelfUpdateArgs {
    /// Check availability without modifying the installation.
    #[arg(long)]
    pub check: bool,
    /// Resolve and print the update without applying it.
    #[arg(long)]
    pub dry_run: bool,
    /// Accept the update plan without prompting.
    #[arg(long)]
    pub yes: bool,
}
