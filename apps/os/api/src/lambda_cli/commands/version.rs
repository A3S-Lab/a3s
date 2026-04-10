//! `a3s version` command - Show CLI version.

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;

/// Version command - Show CLI version information.
#[derive(clap::Parser, Debug)]
pub struct VersionCommand {
    /// Output format.
    #[arg(short = 'o', long, default_value = "text")]
    output: String,

    /// Show client version only (no server).
    #[arg(long)]
    client: bool,

    /// Show server version (requires API server).
    #[arg(long)]
    server: bool,
}

impl VersionCommand {
    /// Print client version.
    fn print_client_version(&self) -> Result<()> {
        let version = env!("CARGO_PKG_VERSION");
        let name = env!("CARGO_PKG_NAME");

        match self.output.as_str() {
            "json" => {
                println!(
                    r#"{{"clientVersion": {{"major": "1", "minor": "28", "gitVersion": "{}", "gitCommit": "{}", "buildDate": "{}"}}}}"#,
                    version,
                    "unknown",
                    chrono::Utc::now().to_rfc3339()
                );
            }
            "yaml" => {
                println!(
                    "clientVersion:\n  major: \"1\"\n  minor: \"28\"\n  gitVersion: {}\n  gitCommit: unknown\n  buildDate: {}",
                    version,
                    chrono::Utc::now().to_rfc3339()
                );
            }
            _ => {
                println!("{} v{}", name, version);
            }
        }

        Ok(())
    }

    /// Print version info.
    fn print_version(&self) -> Result<()> {
        self.print_client_version()?;

        if self.server && !self.client {
            println!();
            println!("Unable to connect to server: --server requires API server running");
            println!("(Hint: use 'a3s serve' to start the API server)");
        }

        Ok(())
    }
}

#[async_trait]
impl Command for VersionCommand {
    async fn run(&self) -> Result<()> {
        self.print_version()
    }
}
