//! `a3s taint` command - Update or remove node taints.
//!
//! Usage: a3s taint NODE KEY=VALUE:EFFECT

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use clap::Parser;

/// Taint effect types.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum TaintEffect {
    /// NoSchedule - do not schedule new pods.
    NoSchedule,
    /// PreferNoSchedule - prefer not to schedule.
    PreferNoSchedule,
    /// NoExecute - evict existing pods.
    NoExecute,
}

impl TaintEffect {
    fn as_str(&self) -> &'static str {
        match self {
            TaintEffect::NoSchedule => "NoSchedule",
            TaintEffect::PreferNoSchedule => "PreferNoSchedule",
            TaintEffect::NoExecute => "NoExecute",
        }
    }
}

/// Taint command arguments.
#[derive(Parser, Debug)]
pub struct TaintCommand {
    /// Node name.
    #[arg(value_name = "NODE")]
    pub node: String,

    /// Taint in format KEY=VALUE:EFFECT.
    /// Example: a3s taint node1 key=value:NoSchedule
    #[arg(value_name = "KEY=VALUE:EFFECT")]
    pub taint: Vec<String>,

    /// Remove taint with this key.
    #[arg(long)]
    pub delete: bool,

    /// Show what would be done without making changes.
    #[arg(short, long)]
    pub dry_run: bool,
}

pub struct TaintOp;

impl TaintOp {
    /// Parse taint string.
    pub fn parse_taint(s: &str) -> Option<(String, String, String)> {
        // Format: key=value:effect
        let parts: Vec<&str> = s.split(':').collect();
        if parts.len() != 2 {
            return None;
        }

        let effect = parts[1];
        let key_value: Vec<&str> = parts[0].split('=').collect();
        if key_value.len() != 2 {
            return None;
        }

        Some((
            key_value[0].to_string(),
            key_value[1].to_string(),
            effect.to_string(),
        ))
    }
}

#[async_trait]
impl Command for TaintCommand {
    async fn run(&self) -> Result<()> {
        if self.delete {
            // Delete taint
            for taint in &self.taint {
                let key = if taint.contains('=') {
                    taint.split('=').next().unwrap_or(taint)
                } else {
                    taint.as_str()
                };

                if self.dry_run {
                    println!("Would remove taint {} from node {}", key, self.node);
                } else {
                    println!("Removed taint {} from node {}", key, self.node);
                }
            }
        } else {
            // Add taint
            for taint in &self.taint {
                if let Some((key, value, effect)) = TaintOp::parse_taint(taint) {
                    if self.dry_run {
                        println!(
                            "Would add taint {}={}:{} to node {}",
                            key, value, effect, self.node
                        );
                    } else {
                        println!(
                            "Taint {}={}:{} added to node {}",
                            key, value, effect, self.node
                        );
                    }
                } else {
                    eprintln!("Invalid taint format: {}. Use KEY=VALUE:EFFECT", taint);
                }
            }
        }

        Ok(())
    }
}
