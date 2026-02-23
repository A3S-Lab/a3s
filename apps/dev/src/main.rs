use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::Colorize;

mod config;
mod error;
mod graph;
mod health;
mod log;
mod proxy;
mod state;
mod supervisor;
mod watcher;

use config::DevConfig;
use error::Result;
use supervisor::Supervisor;

#[derive(Parser)]
#[command(name = "a3s", about = "a3s dev — local development orchestration")]
struct Cli {
    /// Path to A3sfile.hcl (default: ./A3sfile.hcl)
    #[arg(short, long, default_value = "A3sfile.hcl")]
    file: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start all (or named) services in dependency order
    Up {
        /// Services to start (default: all)
        services: Vec<String>,
    },
    /// Stop all (or named) services in reverse dependency order
    Down {
        /// Services to stop (default: all)
        services: Vec<String>,
    },
    /// Restart a service
    Restart {
        service: String,
    },
    /// Show service status
    Status,
    /// Validate A3sfile.hcl without starting anything
    Validate,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .without_time()
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{} {e}", "[a3s error]".red().bold());
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    let cfg = Arc::new(DevConfig::from_file(&cli.file)?);

    match cli.command {
        Commands::Validate => {
            println!("{} A3sfile.hcl is valid ({} services)", "✓".green(), cfg.service.len());
            for (name, svc) in &cfg.service {
                let deps = if svc.depends_on.is_empty() {
                    String::new()
                } else {
                    format!(" (depends on: {})", svc.depends_on.join(", "))
                };
                println!("  {} :{}{}", name.cyan(), svc.port, deps);
            }
            // Validate dependency graph (cycle check)
            graph::DependencyGraph::from_config(&cfg)?;
            println!("{} dependency graph OK", "✓".green());
        }

        Commands::Status => {
            let (sup, _) = Supervisor::new(cfg);
            let rows = sup.status().await;
            println!(
                "{:<16} {:<12} {:<8} {:<6}",
                "SERVICE", "STATE", "PID", "PORT"
            );
            println!("{}", "-".repeat(44));
            for (name, state, pid, port) in rows {
                let state_colored = match state.as_str() {
                    "running" => state.green().to_string(),
                    "starting" => state.yellow().to_string(),
                    "unhealthy" | "failed" => state.red().to_string(),
                    _ => state.dimmed().to_string(),
                };
                println!(
                    "{:<16} {:<12} {:<8} {:<6}",
                    name,
                    state_colored,
                    pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
                    port
                );
            }
        }

        Commands::Up { services } => {
            // Start proxy
            let proxy = proxy::ProxyRouter::new(cfg.dev.proxy_port);
            for (_name, svc) in &cfg.service {
                if let Some(sub) = &svc.subdomain {
                    proxy.register(sub.clone(), svc.port).await;
                }
            }
            let proxy_port = cfg.dev.proxy_port;
            tokio::spawn(async move { proxy.run().await });

            println!(
                "{} proxy on http://*.localhost:{}",
                "→".cyan(),
                proxy_port
            );

            let (sup, _) = Supervisor::new(cfg.clone());
            let sup = Arc::new(sup);

            if services.is_empty() {
                sup.start_all().await?;
            } else {
                for (idx, name) in services.iter().enumerate() {
                    sup.start_service(name, idx).await?;
                }
            }

            // Wait for shutdown signal
            tokio::signal::ctrl_c().await.ok();
            println!("\n{} shutting down...", "→".yellow());
            sup.stop_all().await;
        }

        Commands::Down { services } => {
            let (sup, _) = Supervisor::new(cfg);
            if services.is_empty() {
                sup.stop_all().await;
            } else {
                for name in &services {
                    sup.stop_service(name).await;
                }
            }
        }

        Commands::Restart { service } => {
            let (sup, _) = Supervisor::new(cfg);
            sup.restart_service(&service, 0).await?;
        }
    }

    Ok(())
}
