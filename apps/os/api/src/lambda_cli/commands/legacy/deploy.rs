//! `a3s deploy` command - Deploy models and dependencies.

use crate::commands::Command;
use crate::config::a3sfile::A3sfile;
use crate::deployment::microvm::MicrovmProvider;
use crate::deployment::provider::{DeploymentConfig, DeploymentProvider, ProviderType};
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Deploy models and dependencies from A3sfile.
#[derive(clap::Parser, Debug)]
pub struct DeployCommand {
    /// A3sfile path (default: ./A3sfile.hcl)
    #[arg(short, long)]
    file: Option<PathBuf>,

    /// Deploy only models.
    #[arg(long)]
    models_only: bool,

    /// Deploy only dependencies.
    #[arg(long)]
    deps_only: bool,

    /// Skip health check verification.
    #[arg(long)]
    skip_health_check: bool,
}

impl DeployCommand {
    fn get_a3sfile_path(&self) -> PathBuf {
        self.file
            .clone()
            .unwrap_or_else(|| PathBuf::from("A3sfile.hcl"))
    }

    fn load_a3sfile(&self) -> Result<A3sfile> {
        let path = self.get_a3sfile_path();
        let content = std::fs::read_to_string(&path).map_err(|e| A3sError::Io(e))?;
        A3sfile::parse(&content).map_err(|e| A3sError::Project(e))
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
        let a3sfile = self.load_a3sfile()?;
        let provider_type = self.determine_provider().await?;

        let working_dir = self
            .get_a3sfile_path()
            .parent()
            .map(|p| p.to_path_buf())
            .unwrap_or_else(|| PathBuf::from("."));

        let project_name = a3sfile.agent.name.clone();

        let config = DeploymentConfig::new(provider_type, project_name, working_dir.clone());

        let provider = self.create_provider(provider_type, &working_dir).await?;

        println!("Deploying with provider: {}", provider.name());
        println!();

        // Check availability
        if !provider.is_available().await {
            return Err(A3sError::Project(format!(
                "{} is not available",
                provider.name()
            )));
        }

        // Deploy all or subset
        let state = if self.models_only {
            // Deploy only models - create filtered A3sfile with no dependencies
            let mut filtered = a3sfile.clone();
            filtered.dependencies.clear();
            println!("Deploying {} model(s) only...", filtered.models.len());
            provider.deploy_all(&filtered, &config).await?
        } else if self.deps_only {
            // Deploy only dependencies - create filtered A3sfile with no models
            let mut filtered = a3sfile.clone();
            filtered.models.clear();
            println!(
                "Deploying {} dependency service(s) only...",
                filtered.dependencies.len()
            );
            provider.deploy_all(&filtered, &config).await?
        } else {
            provider.deploy_all(&a3sfile, &config).await?
        };

        println!("✓ Deployment successful!");
        println!();
        println!(
            "Deployed {} model(s) and {} dependency(s)",
            state.models.len(),
            state.dependencies.len()
        );

        if !state.models.is_empty() {
            println!();
            println!("Models:");
            for (name, model) in &state.models {
                let endpoint = model.endpoint.as_deref().unwrap_or("N/A");
                println!("  - {} ({}): {}", name, model.provider, endpoint);
            }
        }

        if !state.dependencies.is_empty() {
            println!();
            println!("Dependencies:");
            for (name, dep) in &state.dependencies {
                println!(
                    "  - {} ({}): {} endpoint(s)",
                    name,
                    dep.image,
                    dep.endpoints.len()
                );
                for (port_name, port) in &dep.endpoints {
                    println!("      - {}: {}", port_name, port);
                }
            }
        }

        Ok(())
    }
}

#[async_trait]
impl Command for DeployCommand {
    async fn run(&self) -> Result<()> {
        self.execute().await
    }
}
