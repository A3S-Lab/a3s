pub mod delete;
pub mod list;
pub mod pull;
pub mod run;
pub mod serve;
pub mod show;

use clap::{Parser, Subcommand};

/// A3S Power - Local model management and serving
#[derive(Debug, Parser)]
#[command(name = "a3s-power", version, about)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

/// Available CLI commands.
#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Pull (if needed), load, and start interactive chat with a model
    Run {
        /// Model name to run (e.g. "llama3.2:3b")
        model: String,

        /// Optional prompt to send directly instead of interactive mode
        #[arg(long)]
        prompt: Option<String>,
    },

    /// Download a model
    Pull {
        /// Model name or URL to download
        model: String,
    },

    /// List locally available models
    List,

    /// Show details about a model
    Show {
        /// Model name to inspect
        model: String,
    },

    /// Delete a local model
    Delete {
        /// Model name to delete
        model: String,
    },

    /// Start the HTTP server
    Serve {
        /// Host address to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value_t = 11435)]
        port: u16,
    },
}
