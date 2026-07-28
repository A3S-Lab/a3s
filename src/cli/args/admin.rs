use a3s::components::ComponentId;
use clap::{Args, Subcommand, ValueEnum};

#[derive(Clone, Debug, Args)]
pub(crate) struct InfoArgs {
    #[arg(value_name = "COMPONENT")]
    pub component: ComponentId,
    /// Query and include known versions.
    #[arg(long)]
    pub versions: bool,
    /// Include every catalog-declared source.
    #[arg(long)]
    pub sources: bool,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct DoctorArgs {
    #[arg(value_name = "COMPONENT")]
    pub component: Option<ComponentId>,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct RegistryArgs {
    #[command(subcommand)]
    pub command: RegistryCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum RegistryCommand {
    /// List the official and explicitly trusted registries.
    List,
    /// Show one registry and its trust identity.
    Show(RegistryNameArgs),
    /// Trust and add a registry URL.
    Add(RegistryAddArgs),
    /// Remove an explicitly added registry.
    Remove(RegistryRemoveArgs),
    /// Check registry reachability and metadata freshness.
    Refresh(RegistryRefreshArgs),
}

#[derive(Clone, Debug, Args)]
pub(crate) struct RegistryNameArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct RegistryAddArgs {
    #[arg(value_name = "URL")]
    pub url: String,
    /// TUF root file or sha256 digest establishing trust.
    #[arg(long, value_name = "FILE_OR_DIGEST")]
    pub trust_root: String,
    /// Accept the explicit trust operation without prompting.
    #[arg(long)]
    pub yes: bool,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct RegistryRemoveArgs {
    #[arg(value_name = "NAME")]
    pub name: String,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct RegistryRefreshArgs {
    #[arg(value_name = "NAME")]
    pub name: Option<String>,
}

#[derive(Clone, Debug, Args)]
#[command(subcommand_required = true, arg_required_else_help = true)]
pub(crate) struct CacheArgs {
    #[command(subcommand)]
    pub command: CacheCommand,
}

#[derive(Clone, Debug, Subcommand)]
pub(crate) enum CacheCommand {
    /// Print the recreatable cache root.
    Path,
    /// Report cache size and entry counts.
    Status,
    /// Remove expired and unreferenced temporary entries.
    Prune(CacheMutationArgs),
    /// Remove all recreatable A3S cache content.
    Clean(CacheCleanArgs),
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct CacheMutationArgs {
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct CacheCleanArgs {
    #[arg(long)]
    pub dry_run: bool,
    #[arg(long)]
    pub yes: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum CompletionShell {
    Bash,
    Zsh,
    Fish,
    Powershell,
    Elvish,
}

#[derive(Clone, Debug, Args)]
pub(crate) struct CompletionArgs {
    #[arg(value_enum)]
    pub shell: CompletionShell,
}

#[derive(Clone, Debug, Default, Args)]
pub(crate) struct HelpArgs {
    #[arg(value_name = "COMMAND")]
    pub command: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
pub(crate) enum ComponentKindArg {
    #[value(name = "built-in")]
    BuiltIn,
    Product,
    Capability,
    Extension,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum ReleaseChannelArg {
    #[default]
    Stable,
    Beta,
    Nightly,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, ValueEnum)]
pub(crate) enum InstallScopeArg {
    #[default]
    User,
    System,
}
