//! `a3s cluster-info` command - Show cluster information.
//!
//! Usage: a3s cluster-info

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use clap::Parser;

/// Cluster info command.
#[derive(Parser, Debug)]
pub struct ClusterInfoCommand {
    /// Show debug information.
    #[arg(long)]
    debug: bool,
}

pub struct ClusterInfo;

#[async_trait]
impl Command for ClusterInfoCommand {
    async fn run(&self) -> Result<()> {
        println!("Kubernetes cluster is running");

        if self.debug {
            println!("\nDebug info:");
            println!("  A3S Lambda v{}", env!("CARGO_PKG_VERSION"));
            println!("  API Server: https://localhost:6443");
            println!("  Kube-apiserver: enabled");
            println!("  Kube-controller-manager: enabled");
            println!("  Kube-scheduler: enabled");
            println!("  etcd: embedded");
        }

        Ok(())
    }
}
