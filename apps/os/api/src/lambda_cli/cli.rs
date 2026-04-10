//! CLI structure shared between main and commands.

use clap::Parser;

/// CLI argument parser for a3s.
#[derive(Parser, Debug)]
#[command(
    name = "a3s",
    version,
    about = "a3s - Agent development and runtime CLI",
    long_about = None
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: crate::commands::Commands,
}
