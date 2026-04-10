//! MicroVM deployment provider - stub implementation
//!
//! VM deployment is disabled because a3s-box-sdk was removed.

use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::config::a3sfile::{A3sfile, Dependency, Model};
use crate::deployment::provider::{
    DeploymentConfig, DeploymentProvider, DependencyDeploymentResult, ModelDeploymentResult,
    ProviderType,
};
use crate::deployment::state::{
    DependencyDeploymentState, DeploymentState, ModelDeploymentState,
};
use crate::errors::Result;

/// Port forward configuration
#[derive(Debug, Clone)]
pub struct PortForward {
    pub guest_port: u16,
    pub host_port: u16,
    pub protocol: String,
}

/// Sandbox options for creating a VM
#[derive(Debug, Clone)]
pub struct SandboxOptions {
    pub image: String,
    pub cpus: u32,
    pub memory_mb: u32,
    pub name: Option<String>,
    pub network: bool,
    pub env: HashMap<String, String>,
    pub port_forwards: Vec<PortForward>,
}

impl Default for SandboxOptions {
    fn default() -> Self {
        Self {
            image: String::new(),
            cpus: 1,
            memory_mb: 256,
            name: None,
            network: true,
            env: HashMap::new(),
            port_forwards: Vec::new(),
        }
    }
}

/// Stub Sandbox - VM execution disabled
#[derive(Debug, Clone)]
pub struct Sandbox {
    pub id: String,
}

impl Sandbox {
    pub fn id(&self) -> &str {
        &self.id
    }
}

/// Result of executing a command in a sandbox
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Stub MicroVM provider - deployment disabled
pub struct MicrovmProvider;

impl MicrovmProvider {
    pub async fn new(_working_dir: &PathBuf) -> Self {
        tracing::warn!("MicrovmProvider is stubbed - VM deployment disabled");
        Self
    }

    pub async fn create_sandbox(&self, _opts: SandboxOptions) -> Result<Sandbox> {
        Err(crate::errors::A3sError::Project(
            "VM deployment disabled - a3s-box-sdk removed".into(),
        ))
    }

    #[allow(dead_code)]
    pub async fn exec_in_sandbox(
        &self,
        _sandbox_id: &str,
        _cmd: &str,
        _args: &[&str],
    ) -> Result<ExecResult> {
        Err(crate::errors::A3sError::Project(
            "VM deployment disabled - a3s-box-sdk removed".into(),
        ))
    }
}

#[async_trait]
impl DeploymentProvider for MicrovmProvider {
    fn name(&self) -> &str {
        "microvm-stub"
    }

    fn provider_type(&self) -> ProviderType {
        ProviderType::Microvm
    }

    async fn deploy_model(
        &self,
        _name: &str,
        _model: &Model,
        _config: &DeploymentConfig,
    ) -> Result<ModelDeploymentResult> {
        Err(crate::errors::A3sError::Project(
            "VM deployment disabled - a3s-box-sdk removed".into(),
        ))
    }

    async fn undeploy_model(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    async fn get_model_status(&self, _name: &str) -> Result<Option<ModelDeploymentState>> {
        Ok(None)
    }

    async fn deploy_dependency(
        &self,
        _name: &str,
        _dep: &Dependency,
        _config: &DeploymentConfig,
    ) -> Result<DependencyDeploymentResult> {
        Err(crate::errors::A3sError::Project(
            "VM deployment disabled - a3s-box-sdk removed".into(),
        ))
    }

    async fn undeploy_dependency(&self, _name: &str) -> Result<()> {
        Ok(())
    }

    async fn get_dependency_status(
        &self,
        _name: &str,
    ) -> Result<Option<DependencyDeploymentState>> {
        Ok(None)
    }

    async fn deploy_all(
        &self,
        _a3sfile: &A3sfile,
        _config: &DeploymentConfig,
    ) -> Result<DeploymentState> {
        Err(crate::errors::A3sError::Project(
            "VM deployment disabled - a3s-box-sdk removed".into(),
        ))
    }

    async fn undeploy_all(&self) -> Result<()> {
        Ok(())
    }

    async fn get_state(&self) -> Result<DeploymentState> {
        Ok(DeploymentState::new(String::new(), String::new()))
    }

    async fn is_available(&self) -> bool {
        false
    }
}
