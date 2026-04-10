//! `a3s drain` command - Drain a node (kubectl drain style).

use crate::commands::Command;
use crate::errors::Result;
use crate::state::store::StateStore;
use async_trait::async_trait;
use std::path::PathBuf;

/// Drain command - Drain a node for maintenance.
#[derive(clap::Parser, Debug)]
pub struct DrainCommand {
    /// Node name.
    name: String,

    /// Ignore daemonsets and unmanaged pods.
    #[arg(short, long)]
    force: bool,

    /// Delete local data pods.
    #[arg(long)]
    delete_local_data: bool,

    /// Grace period in seconds for pod termination.
    #[arg(long, default_value = "30")]
    grace_period: i32,

    /// Skip drain ignore daemonsets.
    #[arg(long)]
    skip_daemonsets: bool,

    /// Drain timeout.
    #[arg(long)]
    timeout: Option<u64>,

    /// List pods on the node without draining.
    #[arg(long)]
    list: bool,
}

impl DrainCommand {
    /// List pods on a node.
    fn list_pods(&self) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let pods = store.load_pods()?;

        let node_pods: Vec<_> = pods
            .values()
            .filter(|p| p.node_name.as_ref() == Some(&self.name))
            .collect();

        if node_pods.is_empty() {
            println!("No pods found on node '{}'", self.name);
            return Ok(());
        }

        println!(
            "{:<40} {:<20} {:<15} {:<10}",
            "NAMESPACE", "NAME", "STATUS", "AGE"
        );
        println!("{}", "-".repeat(85));

        for pod in node_pods {
            let namespace = &pod.namespace;
            let name = &pod.id;
            let status = format!("{:?}", pod.status);
            let age = "unknown";
            println!("{:<40} {:<20} {:<15} {:<10}", namespace, name, status, age);
        }

        Ok(())
    }

    /// Drain the node.
    fn drain_node(&self) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let pods = store.load_pods()?;

        let node_pods: Vec<_> = pods
            .values()
            .filter(|p| p.node_name.as_ref() == Some(&self.name))
            .collect();

        if node_pods.is_empty() {
            println!("No pods found on node '{}', nothing to drain", self.name);
            return Ok(());
        }

        println!("Draining node '{}':", self.name);
        println!("  {} pods found", node_pods.len());

        // In a real implementation, we would delete pods gracefully
        // and mark the node as unschedulable
        println!(
            "node '{}' drained (simulated - implement actual pod eviction)",
            self.name
        );

        Ok(())
    }
}

#[async_trait]
impl Command for DrainCommand {
    async fn run(&self) -> Result<()> {
        if self.list {
            return self.list_pods();
        }
        self.drain_node()
    }
}
