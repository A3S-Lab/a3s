//! `a3s completion` command - Generate shell completion scripts.

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use clap::Command as ClapCommand;
use clap_complete::Shell;

/// Generate shell completion scripts.
#[derive(clap::Parser, Debug)]
pub struct CompletionCommand {
    /// Shell to generate completions for.
    #[arg(value_enum)]
    shell: ClapShell,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ClapShell {
    /// Bash shell
    Bash,
    /// Zsh shell
    Zsh,
    /// Fish shell
    Fish,
    /// PowerShell,
    PowerShell,
    /// Elvish shell
    Elvish,
}

impl CompletionCommand {
    /// Build the a3s command for completion generation.
    fn build_command() -> ClapCommand {
        ClapCommand::new("a3s")
            .version(env!("CARGO_PKG_VERSION"))
            .about("a3s - Agent development and runtime CLI")
            .subcommand(ClapCommand::new("get").about("Get resources (kubectl style)"))
            .subcommand(
                ClapCommand::new("apply").about("Apply resources from file (kubectl style)"),
            )
            .subcommand(
                ClapCommand::new("delete").about("Delete resources from file (kubectl style)"),
            )
            .subcommand(
                ClapCommand::new("describe").about("Show details of a resource (kubectl style)"),
            )
            .subcommand(ClapCommand::new("logs").about("View logs of a resource (kubectl style)"))
            .subcommand(
                ClapCommand::new("attach")
                    .about("Attach to a running container (kubectl attach style)"),
            )
            .subcommand(ClapCommand::new("cp").about("Copy files to/from a pod (kubectl cp style)"))
            .subcommand(
                ClapCommand::new("exec").about("Execute command in a VM (kubectl exec style)"),
            )
            .subcommand(
                ClapCommand::new("api-resources")
                    .about("Show API resources (kubectl api-resources style)"),
            )
            .subcommand(
                ClapCommand::new("api-versions")
                    .about("Show available API versions (kubectl api-versions style)"),
            )
            .subcommand(
                ClapCommand::new("auth").about("Check authorization (kubectl auth can-i style)"),
            )
            .subcommand(
                ClapCommand::new("cluster-info")
                    .about("Show cluster information (kubectl cluster-info style)"),
            )
            .subcommand(ClapCommand::new("completion").about("Generate shell completion scripts"))
            .subcommand(ClapCommand::new("explain").about("Explain resource types and fields"))
            .subcommand(
                ClapCommand::new("cordon")
                    .about("Mark node as unschedulable (kubectl cordon style)"),
            )
            .subcommand(
                ClapCommand::new("drain")
                    .about("Drain a node for maintenance (kubectl drain style)"),
            )
            .subcommand(ClapCommand::new("version").about("Show CLI version information"))
            .subcommand(ClapCommand::new("watch").about("Watch for resource changes in real-time"))
            .subcommand(
                ClapCommand::new("port-forward")
                    .about("Forward local ports to a pod (kubectl port-forward style)"),
            )
            .subcommand(
                ClapCommand::new("label").about("Set labels on resources (kubectl label style)"),
            )
            .subcommand(
                ClapCommand::new("annotate")
                    .about("Set annotations on resources (kubectl annotate style)"),
            )
            .subcommand(
                ClapCommand::new("rollout").about("Manage rollouts (kubectl rollout style)"),
            )
            .subcommand(ClapCommand::new("scale").about("Scale resources (kubectl scale style)"))
            .subcommand(ClapCommand::new("top").about("Show resource usage (kubectl top style)"))
            .subcommand(ClapCommand::new("serve").about("Start the A3S API server"))
            .subcommand(ClapCommand::new("taint").about("Update or remove node taints"))
            .subcommand(ClapCommand::new("init").about("Initialize a new agent project"))
            .subcommand(ClapCommand::new("task").about("Run agent tasks"))
            .subcommand(ClapCommand::new("deploy").about("Deploy models and dependencies"))
            .subcommand(ClapCommand::new("undeploy").about("Tear down deployed resources"))
            .subcommand(ClapCommand::new("status").about("Check deployment status"))
            .subcommand(ClapCommand::new("list").about("List all deployed services"))
    }

    /// Generate completion script for the given shell.
    fn generate_completion(&self) -> Result<()> {
        let mut cmd = Self::build_command();

        match self.shell {
            ClapShell::Bash => {
                clap_complete::generate(Shell::Bash, &mut cmd, "a3s", &mut std::io::stdout());
            }
            ClapShell::Zsh => {
                clap_complete::generate(Shell::Zsh, &mut cmd, "a3s", &mut std::io::stdout());
            }
            ClapShell::Fish => {
                clap_complete::generate(Shell::Fish, &mut cmd, "a3s", &mut std::io::stdout());
            }
            ClapShell::PowerShell => {
                clap_complete::generate(Shell::PowerShell, &mut cmd, "a3s", &mut std::io::stdout());
            }
            ClapShell::Elvish => {
                clap_complete::generate(Shell::Elvish, &mut cmd, "a3s", &mut std::io::stdout());
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Command for CompletionCommand {
    async fn run(&self) -> Result<()> {
        self.generate_completion()
    }
}
