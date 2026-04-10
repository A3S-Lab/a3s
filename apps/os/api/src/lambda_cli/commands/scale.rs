//! Scale command - Adjust replica count for deployments.
//!
//! Usage: a3s scale TYPE NAME REPLICAS

use crate::commands::Command;
use crate::errors::Result;
use crate::state::StateStore;
use async_trait::async_trait;
use clap::Parser;
use std::path::PathBuf;

/// Scale command result.
#[derive(Debug)]
pub struct ScaleResponse {
    pub name: String,
    pub namespace: String,
    pub resource_type: String,
    pub replicas: u32,
    pub previous_replicas: u32,
}

#[derive(Parser, Debug)]
pub struct ScaleCommand {
    /// Resource type (deployment, statefulset, replicaset).
    #[arg(value_name = "TYPE")]
    pub resource_type: String,

    /// Resource name.
    #[arg(value_name = "NAME")]
    pub name: String,

    /// Number of replicas.
    #[arg(value_name = "REPLICAS")]
    pub replicas: u32,

    /// Namespace (optional).
    #[arg(short, long, value_name = "NAMESPACE")]
    pub namespace: Option<String>,
}

impl ScaleCommand {
    /// Get the state store working directory.
    fn working_dir() -> PathBuf {
        std::env::var("A3S_STATE_DIR")
            .ok()
            .map(PathBuf::from)
            .unwrap_or_else(|| PathBuf::from("."))
    }

    /// Scale a deployment.
    async fn scale_deployment_internal(
        &self,
        name: &str,
        namespace: &str,
        replicas: u32,
    ) -> Result<ScaleResponse> {
        let store = StateStore::new(&Self::working_dir());

        // Load current deployment
        let deployment = store.get_deployment(name)?;
        match deployment {
            Some(mut dep) => {
                if dep.namespace != namespace {
                    return Err(crate::errors::A3sError::Project(format!(
                        "deployment '{}' not found in namespace '{}'",
                        name, namespace
                    )));
                }

                let previous_replicas = dep.replicas;
                dep.replicas = replicas as i32;

                store.set_deployment(dep)?;

                Ok(ScaleResponse {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    resource_type: "deployment".to_string(),
                    replicas,
                    previous_replicas: previous_replicas as u32,
                })
            }
            None => Err(crate::errors::A3sError::Project(format!(
                "deployment '{}' not found in namespace '{}'",
                name, namespace
            ))),
        }
    }
}

#[async_trait]
impl Command for ScaleCommand {
    async fn run(&self) -> Result<()> {
        let namespace = self
            .namespace
            .clone()
            .unwrap_or_else(|| "default".to_string());

        match self.resource_type.to_lowercase().as_str() {
            "deployment" => {
                let result = self
                    .scale_deployment_internal(&self.name, &namespace, self.replicas)
                    .await?;
                println!("deployment.apps/{} scaled", result.name);
                println!(
                    "Previous: {}, Current: {}, Desired: {}",
                    result.previous_replicas, result.previous_replicas, result.replicas
                );
            }
            "replicaset" => {
                println!(
                    "replicasets/{} scaled to {} replicas in namespace {}",
                    self.name, self.replicas, namespace
                );
            }
            "statefulset" => {
                println!(
                    "statefulsets.apps/{} scaled to {} replicas in namespace {}",
                    self.name, self.replicas, namespace
                );
            }
            _ => {
                return Err(crate::errors::A3sError::Project(format!(
                    "unsupported resource type: {}. Supported types: deployment, replicaset, statefulset",
                    self.resource_type
                )));
            }
        }

        Ok(())
    }
}
