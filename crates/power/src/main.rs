use clap::Parser;
use tracing_subscriber::EnvFilter;

use a3s_power::backend;
use a3s_power::cli::{Cli, Commands};
use a3s_power::dirs;
use a3s_power::model::registry::ModelRegistry;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Ensure storage directories exist
    dirs::ensure_dirs()?;

    let cli = Cli::parse();

    // Initialize model registry
    let registry = ModelRegistry::new();
    registry.scan()?;

    // Initialize backends
    let backends = backend::default_backends();

    match cli.command {
        Commands::Run { model, prompt } => {
            a3s_power::cli::run::execute(&model, prompt.as_deref(), &registry, &backends).await?;
        }
        Commands::Pull { model } => {
            a3s_power::cli::pull::execute(&model, &registry).await?;
        }
        Commands::List => {
            a3s_power::cli::list::execute(&registry)?;
        }
        Commands::Show { model } => {
            a3s_power::cli::show::execute(&model, &registry)?;
        }
        Commands::Delete { model } => {
            a3s_power::cli::delete::execute(&model, &registry)?;
        }
        Commands::Serve { host, port } => {
            a3s_power::cli::serve::execute(&host, port).await?;
        }
    }

    Ok(())
}
