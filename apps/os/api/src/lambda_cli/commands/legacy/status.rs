//! `a3s status` command - Check deployment status.

use crate::commands::Command;
use crate::deployment::microvm::MicrovmProvider;
use crate::deployment::provider::{DeploymentProvider, ProviderType};
use crate::deployment::state::DeploymentState;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Check deployment status.
#[derive(clap::Parser, Debug)]
pub struct StatusCommand {
    /// A3sfile path (default: ./A3sfile.hcl).
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Show only model status.
    #[arg(long)]
    models: bool,

    /// Show only dependency status.
    #[arg(long)]
    dependencies: bool,

    /// Watch mode (continuous output).
    #[arg(short, long)]
    watch: bool,
}

impl StatusCommand {
    fn get_a3sfile_path(&self) -> PathBuf {
        self.file
            .clone()
            .unwrap_or_else(|| PathBuf::from("A3sfile.hcl"))
    }

    async fn determine_provider(&self) -> Result<ProviderType> {
        // Only Microvm is supported - check if it's available
        let temp_provider = MicrovmProvider::new(&PathBuf::from(".")).await;
        if temp_provider.is_available().await {
            Ok(ProviderType::Microvm)
        } else {
            Err(A3sError::Project(
                "microvm provider not available (KVM required)".to_string(),
            ))
        }
    }

    fn print_state(&self, state: &DeploymentState) {
        if !self.models {
            // Print models
            if state.models.is_empty() {
                println!("No models deployed.");
            } else {
                println!("Models:");
                println!(
                    "  {:<20} {:<15} {:<12} {}",
                    "NAME", "PROVIDER", "STATUS", "ENDPOINT"
                );
                println!("  {}", "-".repeat(70));
                for (name, model) in &state.models {
                    let endpoint = model.endpoint.as_deref().unwrap_or("N/A");
                    println!(
                        "  {:<20} {:<15} {:<12} {}",
                        name, model.provider, model.status, endpoint
                    );
                }
            }
        }

        if !self.dependencies {
            // Print dependencies
            if !self.models {
                println!();
            }
            if state.dependencies.is_empty() {
                println!("No dependencies deployed.");
            } else {
                println!("Dependencies:");
                println!("  {:<20} {:<40} {}", "NAME", "IMAGE", "STATUS");
                println!("  {}", "-".repeat(70));
                for (name, dep) in &state.dependencies {
                    println!("  {:<20} {:<40} {}", name, dep.image, dep.status);
                }
            }
        }
    }

    async fn create_provider(
        &self,
        provider_type: ProviderType,
        working_dir: &PathBuf,
    ) -> Result<Box<dyn DeploymentProvider>> {
        match provider_type {
            ProviderType::Microvm => Ok(Box::new(MicrovmProvider::new(working_dir).await)),
        }
    }

    pub async fn execute(&self) -> Result<()> {
        let provider_type = self.determine_provider().await?;

        let working_dir = self
            .get_a3sfile_path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let provider = self.create_provider(provider_type, &working_dir).await?;

        // Check availability
        if !provider.is_available().await {
            return Err(A3sError::Project(format!(
                "{} is not available",
                provider.name()
            )));
        }

        if self.watch {
            println!("Watching deployment status (Ctrl+C to exit)...");
            println!();
            loop {
                match provider.get_state().await {
                    Ok(state) => {
                        self.print_state(&state);
                    }
                    Err(_) => {
                        println!("No deployment state found.");
                    }
                }
                tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
                println!();
            }
        } else {
            match provider.get_state().await {
                Ok(state) => {
                    self.print_state(&state);
                }
                Err(e) => {
                    return Err(A3sError::Project(format!(
                        "Failed to get deployment state: {}",
                        e
                    )));
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Command for StatusCommand {
    async fn run(&self) -> Result<()> {
        self.execute().await
    }
}
