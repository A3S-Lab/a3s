//! a3s CLI - Agent development and runtime CLI
//!
//! Main entry point for the `a3s` binary.

mod api;
mod cli;
mod commands;
mod config;
mod controller;
mod deployment;
mod errors;
mod state;

use clap::Parser;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use commands::Command;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "a3s=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = cli::Cli::parse();

    if let Err(e) = cli.command.run().await {
        eprintln!("error: {e}");
        return Err(e.into());
    }

    Ok(())
}
