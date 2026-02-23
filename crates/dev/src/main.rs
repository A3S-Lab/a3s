use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::Colorize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

mod config;
mod error;
mod graph;
mod health;
mod ipc;
mod log;
mod proxy;
mod state;
mod supervisor;
mod watcher;

use config::DevConfig;
use error::{DevError, Result};
use ipc::{socket_path, IpcRequest, IpcResponse};
use supervisor::Supervisor;

#[derive(Parser)]
#[command(name = "a3s", about = "a3s — local development orchestration for the A3S monorepo")]
struct Cli {
    /// Path to A3sfile.hcl
    #[arg(short, long, default_value = "A3sfile.hcl")]
    file: PathBuf,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start all (or named) services in dependency order
    Up {
        /// Start only these services
        services: Vec<String>,
    },
    /// Stop all (or named) services
    Down {
        /// Stop only these services
        services: Vec<String>,
    },
    /// Restart a service
    Restart {
        service: String,
    },
    /// Show service status
    Status,
    /// Tail logs (all services or one)
    Logs {
        /// Filter to a specific service
        service: Option<String>,
        /// Keep streaming (default: true)
        #[arg(short, long, default_value_t = true)]
        follow: bool,
    },
    /// Validate A3sfile.hcl without starting anything
    Validate,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("warn")),
        )
        .without_time()
        .init();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{} {e}", "[a3s]".red().bold());
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        // `up` is the daemon — it owns the process and IPC socket
        Commands::Up { services } => {
            let cfg = Arc::new(DevConfig::from_file(&cli.file)?);

            // Start proxy
            let proxy = proxy::ProxyRouter::new(cfg.dev.proxy_port);
            for (_name, svc) in &cfg.service {
                if let Some(sub) = &svc.subdomain {
                    proxy.register(sub.clone(), svc.port).await;
                }
            }
            let proxy_port = cfg.dev.proxy_port;
            tokio::spawn(async move { proxy.run().await });
            println!("{} proxy  http://*.localhost:{}", "→".cyan(), proxy_port);

            let (sup, _) = Supervisor::new(cfg.clone());
            let sup = Arc::new(sup);

            // Start IPC server
            tokio::spawn(sup.clone().serve_ipc());

            // Start services
            if services.is_empty() {
                sup.start_all().await?;
            } else {
                for (idx, name) in services.iter().enumerate() {
                    sup.start_service(name, idx).await?;
                }
            }

            // Wait for Ctrl-C
            tokio::signal::ctrl_c().await.ok();
            println!("\n{} shutting down...", "→".yellow());
            sup.stop_all().await;

            // Clean up socket
            let _ = std::fs::remove_file(socket_path());
        }

        // All other commands are IPC clients
        Commands::Validate => {
            let cfg = Arc::new(DevConfig::from_file(&cli.file)?);
            println!("{} A3sfile.hcl is valid ({} services)", "✓".green(), cfg.service.len());
            for (name, svc) in &cfg.service {
                let deps = if svc.depends_on.is_empty() {
                    String::new()
                } else {
                    format!(" → depends on: {}", svc.depends_on.join(", "))
                };
                let sub = svc
                    .subdomain
                    .as_deref()
                    .map(|s| format!(" (http://{s}.localhost)"))
                    .unwrap_or_default();
                println!("  {} :{}{}{}", name.cyan(), svc.port, sub, deps.dimmed());
            }
            graph::DependencyGraph::from_config(&cfg)?;
            println!("{} dependency graph OK", "✓".green());
        }

        Commands::Status => {
            let resp = ipc_send(IpcRequest::Status).await?;
            if let IpcResponse::Status { rows } = resp {
                println!(
                    "{:<16} {:<12} {:<8} {:<6} {}",
                    "SERVICE".bold(),
                    "STATE".bold(),
                    "PID".bold(),
                    "PORT".bold(),
                    "URL".bold()
                );
                println!("{}", "─".repeat(60).dimmed());
                for row in rows {
                    let state_colored = match row.state.as_str() {
                        "running" => row.state.green().to_string(),
                        "starting" | "restarting" => row.state.yellow().to_string(),
                        "unhealthy" | "failed" => row.state.red().to_string(),
                        _ => row.state.dimmed().to_string(),
                    };
                    let url = row
                        .subdomain
                        .map(|s| format!("http://{s}.localhost"))
                        .unwrap_or_default();
                    println!(
                        "{:<16} {:<20} {:<8} {:<6} {}",
                        row.name,
                        state_colored,
                        row.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
                        row.port,
                        url.dimmed()
                    );
                }
            }
        }

        Commands::Down { services } => {
            ipc_send(IpcRequest::Stop {
                services: services.clone(),
            })
            .await?;
            println!("{} stopped", "✓".green());
        }

        Commands::Restart { service } => {
            ipc_send(IpcRequest::Restart {
                service: service.clone(),
            })
            .await?;
            println!("{} restarted {}", "✓".green(), service.cyan());
        }

        Commands::Logs { service, follow } => {
            stream_logs(service.clone(), *follow).await?;
        }
    }

    Ok(())
}

/// Send a single IPC request and return the response.
async fn ipc_send(req: IpcRequest) -> Result<IpcResponse> {
    let stream = UnixStream::connect(socket_path()).await.map_err(|_| {
        DevError::Config("no running a3s daemon — run `a3s up` first".into())
    })?;

    let (reader, mut writer) = tokio::io::split(stream);
    let line = serde_json::to_string(&req).unwrap();
    writer.write_all(format!("{line}\n").as_bytes()).await?;

    let mut lines = BufReader::new(reader).lines();
    let resp_line = lines
        .next_line()
        .await?
        .ok_or_else(|| DevError::Config("daemon closed connection".into()))?;

    serde_json::from_str(&resp_line)
        .map_err(|e| DevError::Config(format!("bad IPC response: {e}")))
}

/// Stream log lines from the daemon.
async fn stream_logs(service: Option<String>, follow: bool) -> Result<()> {
    let stream = UnixStream::connect(socket_path()).await.map_err(|_| {
        DevError::Config("no running a3s daemon — run `a3s up` first".into())
    })?;

    let (reader, mut writer) = tokio::io::split(stream);
    let req = IpcRequest::Logs {
        service,
        follow,
    };
    writer
        .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
        .await?;

    let mut lines = BufReader::new(reader).lines();
    while let Ok(Some(line)) = lines.next_line().await {
        if let Ok(IpcResponse::LogLine { service, line: text }) =
            serde_json::from_str::<IpcResponse>(&line)
        {
            println!("{} {}", format!("[{service}]").cyan(), text);
        }
    }

    Ok(())
}
