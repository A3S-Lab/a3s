//! `a3s undeploy` command - Tear down deployed resources.

use crate::commands::Command;
use crate::deployment::microvm::MicrovmProvider;
use crate::deployment::provider::{DeploymentProvider, ProviderType};
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Tear down deployed resources.
#[derive(clap::Parser, Debug)]
pub struct UndeployCommand {
    /// A3sfile path (default: ./A3sfile.hcl).
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Remove volumes.
    #[arg(long)]
    remove_volumes: bool,
}

impl UndeployCommand {
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

        println!("Undeploying with provider: {}", provider.name());
        println!();

        // Check availability
        if !provider.is_available().await {
            return Err(A3sError::Project(format!(
                "{} is not available",
                provider.name()
            )));
        }

        // Try to get current state
        match provider.get_state().await {
            Ok(state) => {
                println!(
                    "Found {} model(s) and {} dependency(s) to remove",
                    state.models.len(),
                    state.dependencies.len()
                );
            }
            Err(_) => {
                println!("No previous deployment state found, attempting cleanup anyway...");
            }
        }

        // Undeploy all
        provider.undeploy_all().await?;

        println!();
        println!("✓ Undeployment successful!");

        Ok(())
    }
}

#[async_trait]
impl Command for UndeployCommand {
    async fn run(&self) -> Result<()> {
        self.execute().await
    }
}
