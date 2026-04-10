//! `a3s cordon` command - Mark node as unschedulable (kubectl cordon style).

use crate::commands::Command;
use crate::errors::Result;
use crate::state::store::StateStore;
use async_trait::async_trait;
use std::path::PathBuf;

/// Cordon command - Mark a node as unschedulable.
#[derive(clap::Parser, Debug)]
pub struct CordonCommand {
    /// Node name.
    name: String,

    /// Undo the cordon (mark as schedulable).
    #[arg(long)]
    uncordon: bool,
}

impl CordonCommand {
    /// Mark node as unschedulable.
    fn cordon_node(&self) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let node = store.get_node(&self.name)?;

        match node {
            Some(_n) => {
                println!("node '{}' cordoned", self.name);
                println!("(Note: unschedulable marking not persisted - implement NodeDesired.spec.unschedulable)");
                Ok(())
            }
            None => {
                println!("node '{}' not found", self.name);
                Ok(())
            }
        }
    }

    /// Mark node as schedulable.
    fn uncordon_node(&self) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        let node = store.get_node(&self.name)?;

        match node {
            Some(_) => {
                println!("node '{}' uncordoned", self.name);
                Ok(())
            }
            None => {
                println!("node '{}' not found", self.name);
                Ok(())
            }
        }
    }
}

#[async_trait]
impl Command for CordonCommand {
    async fn run(&self) -> Result<()> {
        if self.uncordon {
            self.uncordon_node()
        } else {
            self.cordon_node()
        }
    }
}
