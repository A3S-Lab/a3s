//! CLI commands.

pub mod annotate;
pub mod api_resources;
pub mod api_versions;
pub mod apply;
pub mod attach;
pub mod auth;
pub mod cluster;
pub mod completion;
pub mod cordon;
pub mod cp;
pub mod delete;
pub mod describe;
pub mod drain;
pub mod exec;
pub mod explain;
pub mod get;
pub mod label;
pub mod logs;
pub mod portforward;
pub mod rollout;
pub mod scale;
pub mod serve;
pub mod taint;
pub mod top;
pub mod version;
pub mod watch;

// Legacy commands (backward compatibility)
pub mod legacy;

use async_trait::async_trait;
use clap::Subcommand;

pub use annotate::AnnotateCommand;
pub use api_resources::ApiResourcesCommand;
pub use api_versions::ApiVersionsCommand;
pub use apply::ApplyCommand;
pub use attach::AttachCommand;
pub use auth::AuthCommand;
pub use cluster::ClusterInfoCommand;
pub use completion::CompletionCommand;
pub use cordon::CordonCommand;
pub use cp::CpCommand;
pub use delete::DeleteCommand;
pub use describe::DescribeCommand;
pub use drain::DrainCommand;
pub use exec::ExecCommand;
pub use explain::ExplainCommand;
pub use get::GetCommand;
pub use label::LabelCommand;
pub use logs::LogsCommand;
pub use portforward::PortForwardCommand;
pub use rollout::RolloutCommand;
pub use scale::ScaleCommand;
pub use serve::ServeCommand;
pub use taint::TaintCommand;
pub use top::TopCommand;
pub use version::VersionCommand;
pub use watch::WatchCommand;

// Legacy command re-exports
pub use legacy::{
    DeployCommand, InitCommand, ListCommand, StatusCommand, TaskCommands, UndeployCommand,
};

#[async_trait]
pub trait Command {
    async fn run(&self) -> crate::errors::Result<()>;
}

/// a3s commands.
#[derive(Subcommand, Debug)]
pub enum Commands {
    /// Get resources (kubectl style).
    Get(GetCommand),
    /// Apply resources from file (kubectl style).
    Apply(ApplyCommand),
    /// Delete resources from file (kubectl style).
    Delete(DeleteCommand),
    /// Show details of a resource (kubectl style).
    Describe(DescribeCommand),
    /// View logs of a resource (kubectl style).
    Logs(LogsCommand),
    /// Attach to a running container (kubectl attach style).
    Attach(AttachCommand),
    /// Copy files to/from a pod (kubectl cp style).
    Cp(CpCommand),
    /// Execute command in a VM (kubectl exec style).
    Exec(ExecCommand),
    /// Show API resources (kubectl api-resources style).
    ApiResources(ApiResourcesCommand),
    /// Show available API versions (kubectl api-versions style).
    ApiVersions(ApiVersionsCommand),
    /// Check authorization (kubectl auth can-i style).
    Auth(AuthCommand),
    /// Show cluster information (kubectl cluster-info style).
    ClusterInfo(ClusterInfoCommand),
    /// Generate shell completion scripts.
    Completion(CompletionCommand),
    /// Explain resource types and fields.
    Explain(ExplainCommand),
    /// Mark node as unschedulable (kubectl cordon style).
    Cordon(CordonCommand),
    /// Drain a node for maintenance (kubectl drain style).
    Drain(DrainCommand),
    /// Show CLI version information.
    Version(VersionCommand),
    /// Watch for resource changes in real-time.
    Watch(WatchCommand),
    /// Forward local ports to a pod (kubectl port-forward style).
    PortForward(PortForwardCommand),
    /// Set labels on resources (kubectl label style).
    Label(LabelCommand),
    /// Set annotations on resources (kubectl annotate style).
    Annotate(AnnotateCommand),
    /// Manage rollouts (kubectl rollout style).
    Rollout(RolloutCommand),
    /// Scale resources (kubectl scale style).
    Scale(ScaleCommand),
    /// Show resource usage (kubectl top style).
    Top(TopCommand),
    /// Start the A3S API server.
    Serve(ServeCommand),
    /// Update or remove node taints.
    Taint(TaintCommand),

    // Legacy commands
    /// Initialize a new agent project.
    Init(InitCommand),
    /// Run agent tasks.
    Task(TaskCommands),
    /// Deploy models and dependencies.
    Deploy(DeployCommand),
    /// Tear down deployed resources.
    Undeploy(UndeployCommand),
    /// Check deployment status.
    Status(StatusCommand),
    /// List all deployed services.
    List(ListCommand),
}

#[async_trait]
impl Command for Commands {
    async fn run(&self) -> crate::errors::Result<()> {
        match self {
            Commands::Get(cmd) => cmd.run().await,
            Commands::Apply(cmd) => cmd.run().await,
            Commands::Delete(cmd) => cmd.run().await,
            Commands::Describe(cmd) => cmd.run().await,
            Commands::Logs(cmd) => cmd.run().await,
            Commands::Attach(cmd) => cmd.run().await,
            Commands::Cp(cmd) => cmd.run().await,
            Commands::Exec(cmd) => cmd.run().await,
            Commands::ApiResources(cmd) => cmd.run().await,
            Commands::ApiVersions(cmd) => cmd.run().await,
            Commands::Auth(cmd) => cmd.run().await,
            Commands::ClusterInfo(cmd) => cmd.run().await,
            Commands::Completion(cmd) => cmd.run().await,
            Commands::Explain(cmd) => cmd.run().await,
            Commands::Cordon(cmd) => cmd.run().await,
            Commands::Drain(cmd) => cmd.run().await,
            Commands::Version(cmd) => cmd.run().await,
            Commands::Watch(cmd) => cmd.run().await,
            Commands::PortForward(cmd) => cmd.run().await,
            Commands::Label(cmd) => cmd.run().await,
            Commands::Annotate(cmd) => cmd.run().await,
            Commands::Rollout(cmd) => cmd.run().await,
            Commands::Scale(cmd) => cmd.run().await,
            Commands::Top(cmd) => cmd.run().await,
            Commands::Serve(cmd) => cmd.run().await,
            Commands::Taint(cmd) => cmd.run().await,
            Commands::Init(cmd) => cmd.run().await,
            Commands::Task(cmd) => cmd.run().await,
            Commands::Deploy(cmd) => cmd.run().await,
            Commands::Undeploy(cmd) => cmd.run().await,
            Commands::Status(cmd) => cmd.run().await,
            Commands::List(cmd) => cmd.run().await,
        }
    }
}
