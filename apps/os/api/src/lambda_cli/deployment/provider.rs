//! Deployment provider traits and configuration.
//!
//! Defines the core interface for deploying models and dependencies.

use crate::config::a3sfile::{A3sfile, Dependency, Model};
use crate::deployment::state::{
    DependencyDeploymentState, DeploymentState, DeploymentStatus, ModelDeploymentState,
};
use crate::errors::Result;
use async_trait::async_trait;
use std::path::PathBuf;

/// Deployment provider type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderType {
    Microvm,
}

impl std::fmt::Display for ProviderType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Microvm => write!(f, "microvm"),
        }
    }
}

impl std::str::FromStr for ProviderType {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "microvm" | "micro-vm" | "firecracker" => Ok(Self::Microvm),
            _ => Err(format!("unknown provider: {s}")),
        }
    }
}

/// Deployment configuration.
#[derive(Debug, Clone)]
pub struct DeploymentConfig {
    pub provider: ProviderType,
    pub project_name: String,
    pub working_dir: PathBuf,
}

impl DeploymentConfig {
    pub fn new(provider: ProviderType, project_name: String, working_dir: PathBuf) -> Self {
        Self {
            provider,
            project_name,
            working_dir,
        }
    }
}

/// Result of a model deployment operation.
#[derive(Debug, Clone)]
pub struct ModelDeploymentResult {
    pub name: String,
    pub endpoint: Option<String>,
    pub status: DeploymentStatus,
}

/// Result of a dependency deployment operation.
#[derive(Debug, Clone)]
pub struct DependencyDeploymentResult {
    pub name: String,
    pub endpoints: std::collections::HashMap<String, u16>,
    pub container_id: Option<String>,
    pub status: DeploymentStatus,
}

/// Core trait for deployment providers.
#[async_trait]
pub trait DeploymentProvider: Send + Sync {
    fn name(&self) -> &str;
    fn provider_type(&self) -> ProviderType;

    /// Deploy a single model.
    async fn deploy_model(
        &self,
        name: &str,
        model: &Model,
        config: &DeploymentConfig,
    ) -> Result<ModelDeploymentResult>;

    /// Undeploy a single model.
    async fn undeploy_model(&self, name: &str) -> Result<()>;

    /// Get status of a deployed model.
    async fn get_model_status(&self, name: &str) -> Result<Option<ModelDeploymentState>>;

    /// Deploy a single dependency service.
    async fn deploy_dependency(
        &self,
        name: &str,
        dep: &Dependency,
        config: &DeploymentConfig,
    ) -> Result<DependencyDeploymentResult>;

    /// Undeploy a single dependency service.
    async fn undeploy_dependency(&self, name: &str) -> Result<()>;

    /// Get status of a deployed dependency.
    async fn get_dependency_status(&self, name: &str) -> Result<Option<DependencyDeploymentState>>;

    /// Deploy all models and dependencies from an A3sfile.
    async fn deploy_all(
        &self,
        a3sfile: &A3sfile,
        config: &DeploymentConfig,
    ) -> Result<DeploymentState>;

    /// Undeploy all deployed resources.
    async fn undeploy_all(&self) -> Result<()>;

    /// Get current deployment state.
    async fn get_state(&self) -> Result<DeploymentState>;

    /// Check if provider is available (e.g., KVM available).
    async fn is_available(&self) -> bool;
}
