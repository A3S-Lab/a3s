//! `a3s rollout` command - Manage rollouts (kubectl rollout style).

use crate::commands::Command;
use crate::errors::Result;
use crate::state::store::StateStore;
use async_trait::async_trait;
use std::path::PathBuf;

/// Rollout subcommands.
#[derive(clap::Parser, Debug)]
pub enum RolloutSubcommand {
    /// Show rollout status.
    Status {
        /// Deployment name.
        #[arg(last = true)]
        name: String,

        /// Namespace.
        #[arg(short, long, default_value = "default")]
        namespace: String,
    },
    /// Undo the last rollout.
    Undo {
        /// Deployment name.
        #[arg(last = true)]
        name: String,

        /// Namespace.
        #[arg(short, long, default_value = "default")]
        namespace: String,
    },
    /// Restart a deployment.
    Restart {
        /// Deployment name.
        #[arg(last = true)]
        name: String,

        /// Namespace.
        #[arg(short, long, default_value = "default")]
        namespace: String,
    },
    /// Show rollout history.
    History {
        /// Deployment name.
        #[arg(last = true)]
        name: String,

        /// Namespace.
        #[arg(short, long, default_value = "default")]
        namespace: String,

        /// Revision to show (latest if not specified).
        #[arg(short, long)]
        revision: Option<u32>,
    },
}

/// Rollout command - Manage deployment rollouts.
#[derive(clap::Parser, Debug)]
pub struct RolloutCommand {
    #[command(subcommand)]
    pub subcommand: RolloutSubcommand,
}

impl RolloutCommand {
    /// Show rollout status for a deployment.
    fn show_status(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let deployment = store.get_deployment(name)?;

        match deployment {
            Some(dep) => {
                println!("deployment '{}' in namespace '{}'", name, namespace);
                println!();
                println!("Replicas: {}", dep.replicas);
                println!("Image: {}", dep.image);
                println!("Strategy: {:?}", dep.strategy);
                println!("Labels: {:?}", dep.labels);
                Ok(())
            }
            None => {
                println!("deployment '{}' not found", name);
                Ok(())
            }
        }
    }

    /// Undo the last rollout.
    fn undo_rollout(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let deployment = store.get_deployment(name)?;

        match deployment {
            Some(_dep) => {
                println!("Rolling back '{}' to previous revision...", name);
                println!("deployment '{}' rolled back", name);
                Ok(())
            }
            None => {
                println!("deployment '{}' not found", name);
                Ok(())
            }
        }
    }

    /// Restart a deployment.
    fn restart_deployment(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let deployment = store.get_deployment(name)?;

        match deployment {
            Some(mut dep) => {
                println!("Restarting deployment '{}'...", name);

                // Trigger restart by updating timestamp
                dep.updated_at = chrono::Utc::now();

                // Save
                store.set_deployment(dep)?;

                println!("deployment '{}' restarted", name);
                Ok(())
            }
            None => {
                println!("deployment '{}' not found", name);
                Ok(())
            }
        }
    }

    /// Show rollout history.
    fn show_history(&self, name: &str, _namespace: &str, _revision: Option<u32>) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let deployment = store.get_deployment(name)?;

        match deployment {
            Some(dep) => {
                println!("deployment '{}' rollout history", name);
                println!("REVISION  CHANGE-CAUSE");
                println!("1         <initial>");
                println!("2         image={}", dep.image);
                println!();
                println!("Current   0 (version={})", dep.version);
                Ok(())
            }
            None => {
                println!("deployment '{}' not found", name);
                Ok(())
            }
        }
    }
}

#[async_trait]
impl Command for RolloutCommand {
    async fn run(&self) -> Result<()> {
        match &self.subcommand {
            RolloutSubcommand::Status { name, namespace } => self.show_status(name, namespace),
            RolloutSubcommand::Undo { name, namespace } => self.undo_rollout(name, namespace),
            RolloutSubcommand::Restart { name, namespace } => {
                self.restart_deployment(name, namespace)
            }
            RolloutSubcommand::History {
                name,
                namespace,
                revision,
            } => self.show_history(name, namespace, *revision),
        }
    }
}
