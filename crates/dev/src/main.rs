use std::path::PathBuf;
use std::sync::Arc;

use clap::{Parser, Subcommand};
use colored::Colorize;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

mod brew;
mod code;
mod config;
mod error;
mod graph;
mod health;
mod ipc;
mod log;
mod proxy;
mod state;
mod supervisor;
mod ui;
mod watcher;

use config::DevConfig;
use error::{DevError, Result};
use ipc::{socket_path, IpcRequest, IpcResponse};
use supervisor::Supervisor;

#[derive(Parser)]
#[command(
    name = "a3s",
    about = "a3s — local development orchestration for the A3S monorepo",
    allow_external_subcommands = true
)]
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
        /// Run as background daemon (detach from terminal)
        #[arg(short, long)]
        detach: bool,
        /// Disable the web UI (default: enabled on port 10350)
        #[arg(long)]
        no_ui: bool,
        /// Web UI port
        #[arg(long, default_value_t = ui::DEFAULT_UI_PORT)]
        ui_port: u16,
    },
    /// Stop all (or named) services
    Down {
        /// Stop only these services
        services: Vec<String>,
    },
    /// Restart a service
    Restart { service: String },
    /// Show service status
    Status,
    /// Tail logs (all services or one)
    Logs {
        /// Filter to a specific service
        service: Option<String>,
        /// Keep streaming
        #[arg(short, long, default_value_t = true)]
        follow: bool,
    },
    /// Generate a new A3sfile.hcl in the current directory
    Init,
    /// Validate A3sfile.hcl without starting anything
    Validate,
    /// Upgrade a3s to the latest version
    Upgrade,
    /// Add a Homebrew package to A3sfile.hcl and install it
    Add {
        /// Package name(s) to add
        packages: Vec<String>,
    },
    /// Remove a Homebrew package from A3sfile.hcl and uninstall it
    Remove {
        /// Package name(s) to remove
        packages: Vec<String>,
    },
    /// Search for Homebrew packages
    Search {
        /// Search query
        query: String,
    },
    /// Install all brew packages declared in A3sfile.hcl
    Install,
    /// List all installed a3s ecosystem tools and brew packages
    List,
    /// a3s-code agent scaffolding
    Code {
        #[command(subcommand)]
        cmd: CodeCommands,
    },
    /// Proxy to an a3s ecosystem tool (e.g. `a3s box`, `a3s gateway`)
    #[command(external_subcommand)]
    Tool(Vec<String>),
}

#[derive(Subcommand)]
enum CodeCommands {
    /// Scaffold a new a3s-code agent project
    Init {
        /// Directory to create the project in (default: current directory)
        #[arg(default_value = ".")]
        dir: std::path::PathBuf,
    },
}

#[tokio::main]
async fn main() {
    // Parse CLI first so we can read log_level from A3sfile.hcl for `up`
    let cli = Cli::parse();

    let log_level = if matches!(cli.command, Commands::Up { .. }) {
        std::fs::read_to_string(&cli.file)
            .ok()
            .and_then(|s| hcl::from_str::<config::DevConfig>(&s).ok())
            .map(|c| c.dev.log_level)
            .unwrap_or_else(|| "info".into())
    } else {
        "warn".into()
    };

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new(log_level)),
        )
        .without_time()
        .init();

    if let Err(e) = run(cli).await {
        eprintln!("{} {e}", "[a3s]".red().bold());
        std::process::exit(1);
    }
}

async fn run(cli: Cli) -> Result<()> {
    match &cli.command {
        Commands::Up { services, detach, no_ui, ui_port } => {
            if *detach {
                // Re-launch self as background daemon, dropping --detach flag
                let exe = std::env::current_exe()
                    .map_err(|e| DevError::Config(format!("cannot find self: {e}")))?;
                let mut args: Vec<String> = vec![
                    "--file".into(),
                    cli.file.display().to_string(),
                    "up".into(),
                ];
                args.extend(services.iter().cloned());

                std::process::Command::new(&exe)
                    .args(&args)
                    .stdin(std::process::Stdio::null())
                    .stdout(std::process::Stdio::null())
                    .stderr(std::process::Stdio::null())
                    .spawn()
                    .map_err(|e| DevError::Config(format!("failed to daemonize: {e}")))?;

                println!("{} a3s daemon started in background", "✓".green());
                println!("  run {} to check status", "a3s status".cyan());
                println!("  run {} to stop", "a3s down".cyan());
                return Ok(());
            }

            let cfg = Arc::new(DevConfig::from_file(&cli.file)?);

            // Install missing brew packages before starting services
            let missing = brew::missing_packages(&cfg.brew.packages);
            if !missing.is_empty() {
                brew::install_packages(&missing).await?;
            }

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

            tokio::spawn(sup.clone().serve_ipc());

            // Start web UI
            if !no_ui {
                let ui_port = *ui_port;
                let sup_ui = sup.clone();
                tokio::spawn(async move { ui::serve(sup_ui, ui_port).await });
                println!("{} ui     http://localhost:{}", "→".cyan(), ui_port);
                // Open browser after a short delay
                tokio::spawn(async move {
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let _ = std::process::Command::new("open")
                        .arg(format!("http://localhost:{ui_port}"))
                        .spawn();
                });
            }

            if services.is_empty() {
                sup.start_all().await?;
            } else {
                for (idx, name) in services.iter().enumerate() {
                    sup.start_service(name, idx).await?;
                }
            }

            tokio::signal::ctrl_c().await.ok();
            println!("\n{} shutting down...", "→".yellow());
            sup.stop_all().await;
            let _ = std::fs::remove_file(socket_path());
        }

        Commands::Init => {
            let path = &cli.file;
            if path.exists() {
                return Err(DevError::Config(format!(
                    "{} already exists — delete it first or use a different path",
                    path.display()
                )));
            }
            std::fs::write(path, INIT_TEMPLATE)
                .map_err(|e| DevError::Config(format!("write {}: {e}", path.display())))?;
            println!("{} created {}", "✓".green(), path.display().to_string().cyan());

            // Ask if user wants to git init
            if !std::path::Path::new(".git").exists() {
                print!("  initialize git repository? [Y/n] ");
                std::io::Write::flush(&mut std::io::stdout()).ok();
                let mut input = String::new();
                std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut input).ok();
                let answer = input.trim().to_lowercase();
                if answer.is_empty() || answer == "y" || answer == "yes" {
                    let status = std::process::Command::new("git")
                        .arg("init")
                        .status()
                        .map_err(|e| DevError::Config(format!("git init: {e}")))?;
                    if status.success() {
                        // Write .gitignore
                        let gitignore = ".gitignore";
                        if !std::path::Path::new(gitignore).exists() {
                            std::fs::write(gitignore, "target/\n.env\n*.sock\n")
                                .map_err(|e| DevError::Config(format!("write .gitignore: {e}")))?;
                            println!("{} created .gitignore", "✓".green());
                        }
                    }
                }
            }

            println!("  edit {}, then run {} to start your services", path.display().to_string().cyan(), "a3s up".cyan());
        }

        Commands::Validate => {
            let cfg = Arc::new(DevConfig::from_file(&cli.file)?);
            println!(
                "{} A3sfile.hcl is valid ({} services, {} brew packages)",
                "✓".green(),
                cfg.service.len(),
                cfg.brew.packages.len()
            );
            if !cfg.brew.packages.is_empty() {
                println!("  brew: {}", cfg.brew.packages.join(", ").dimmed());
            }
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
                    "{:<16} {:<12} {:<8} {:<6} {:<24} {}",
                    "SERVICE".bold(),
                    "STATE".bold(),
                    "PID".bold(),
                    "PORT".bold(),
                    "URL".bold(),
                    "UPTIME".bold(),
                );
                println!("{}", "─".repeat(72).dimmed());
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
                    let uptime = row.uptime_secs.map(format_uptime).unwrap_or_else(|| "-".into());
                    println!(
                        "{:<16} {:<20} {:<8} {:<6} {:<24} {}",
                        row.name,
                        state_colored,
                        row.pid.map(|p| p.to_string()).unwrap_or_else(|| "-".into()),
                        row.port,
                        url.dimmed(),
                        uptime.dimmed(),
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

        Commands::Upgrade => {
            let config = a3s_updater::UpdateConfig {
                binary_name: "a3s",
                crate_name: "a3s",
                current_version: env!("CARGO_PKG_VERSION"),
                github_owner: "A3S-Lab",
                github_repo: "Dev",
            };
            a3s_updater::run_update(&config)
                .await
                .map_err(|e| DevError::Config(e.to_string()))?;
        }

        Commands::Add { packages } => {
            if packages.is_empty() {
                return Err(DevError::Config("specify at least one package".into()));
            }
            let mut cfg = DevConfig::from_file(&cli.file)?;
            let mut added = vec![];
            for pkg in packages {
                if brew::add_to_list(&mut cfg.brew.packages, pkg) {
                    added.push(pkg.clone());
                } else {
                    println!("  {} {} already in A3sfile.hcl", "·".dimmed(), pkg.cyan());
                }
            }
            if !added.is_empty() {
                brew::write_brew_block(&cli.file, &cfg.brew.packages)?;
                brew::install_packages(&added).await?;
                println!("{} added: {}", "✓".green(), added.join(", ").cyan());
            }
        }

        Commands::Remove { packages } => {
            if packages.is_empty() {
                return Err(DevError::Config("specify at least one package".into()));
            }
            let mut cfg = DevConfig::from_file(&cli.file)?;
            let mut removed = vec![];
            for pkg in packages {
                if brew::remove_from_list(&mut cfg.brew.packages, pkg) {
                    removed.push(pkg.clone());
                } else {
                    println!("  {} {} not in A3sfile.hcl", "·".dimmed(), pkg.cyan());
                }
            }
            if !removed.is_empty() {
                brew::write_brew_block(&cli.file, &cfg.brew.packages)?;
                for pkg in &removed {
                    brew::uninstall_package(pkg).await?;
                }
                println!("{} removed: {}", "✓".green(), removed.join(", ").cyan());
            }
        }

        Commands::Search { query } => {
            brew::search_packages(query).await?;
        }

        Commands::Install => {
            let cfg = DevConfig::from_file(&cli.file)?;
            if cfg.brew.packages.is_empty() {
                println!("{} no brew packages declared", "·".dimmed());
                return Ok(());
            }
            let missing = brew::missing_packages(&cfg.brew.packages);
            if missing.is_empty() {
                println!("{} all brew packages already installed", "✓".green());
            } else {
                brew::install_packages(&missing).await?;
                println!("{} installed {} package(s)", "✓".green(), missing.len());
            }
        }

        Commands::List => {
            // a3s ecosystem tools
            let tools = [
                ("box",     "a3s-box",     "A3S-Lab/Box"),
                ("gateway", "a3s-gateway", "A3S-Lab/Gateway"),
                ("power",   "a3s-power",   "A3S-Lab/Power"),
            ];

            println!("{:<12} {:<16} {}", "TOOL".bold(), "BINARY".bold(), "STATUS".bold());
            println!("{}", "─".repeat(44).dimmed());
            for (alias, binary, _repo) in &tools {
                let installed = which_binary(binary);
                let status = if installed {
                    "installed".green().to_string()
                } else {
                    "not installed".dimmed().to_string()
                };
                println!("{:<12} {:<16} {}", alias, binary, status);
            }

            // brew packages from A3sfile.hcl
            if let Ok(cfg) = DevConfig::from_file(&cli.file) {
                if !cfg.brew.packages.is_empty() {
                    println!();
                    println!("{}", "brew packages:".bold());
                    for pkg in &cfg.brew.packages {
                        let installed = brew::missing_packages(&[pkg.clone()]).is_empty();
                        let status = if installed {
                            "installed".green().to_string()
                        } else {
                            "missing".yellow().to_string()
                        };
                        println!("  {:<20} {}", pkg.cyan(), status);
                    }
                }
            }
        }

        Commands::Tool(args) => {
            let tool = &args[0];
            let rest = &args[1..];
            proxy_tool(tool, rest).await?;
        }

        Commands::Code { cmd } => match cmd {
            CodeCommands::Init { dir } => {
                println!("{} a3s-code agent scaffolding\r\n", "→".cyan());
                let lang = code::prompt_language()?;
                code::scaffold(dir, lang)?;
                let lang_name = match lang {
                    code::Language::Python => "Python",
                    code::Language::TypeScript => "TypeScript",
                };
                println!(
                    "{} scaffolded {} agent in {}\r\n",
                    "✓".green(),
                    lang_name.cyan(),
                    dir.display()
                );
                println!("  config.hcl    — agent configuration");
                println!("  skills/       — custom tool skills");
                println!("  agents/       — agent runner scripts");
            }
        },
    }

    Ok(())
}

fn format_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m{}s", secs / 60, secs % 60)
    } else {
        format!("{}h{}m", secs / 3600, (secs % 3600) / 60)
    }
}
fn which_binary(name: &str) -> bool {
    std::process::Command::new("which")
        .arg(name)
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// Known a3s ecosystem tools: alias -> (binary, github_owner, github_repo)
fn ecosystem_tool(alias: &str) -> Option<(&'static str, &'static str, &'static str)> {
    match alias {
        "box"     => Some(("a3s-box",     "A3S-Lab", "Box")),
        "gateway" => Some(("a3s-gateway", "A3S-Lab", "Gateway")),
        "power"   => Some(("a3s-power",   "A3S-Lab", "Power")),
        _ => None,
    }
}

/// Proxy a command to an a3s ecosystem tool, auto-installing if missing.
async fn proxy_tool(alias: &str, args: &[String]) -> Result<()> {
    let (binary, owner, repo) = ecosystem_tool(alias).ok_or_else(|| {
        DevError::Config(format!(
            "unknown tool '{alias}' — run `a3s list` to see available tools"
        ))
    })?;

    if !which_binary(binary) {
        println!(
            "{} {} not found — installing from {}/{}...",
            "→".cyan(), binary.cyan(), owner, repo
        );
        let config = a3s_updater::UpdateConfig {
            binary_name: binary,
            crate_name: binary,
            current_version: "0.0.0", // force install
            github_owner: owner,
            github_repo: repo,
        };
        a3s_updater::run_update(&config)
            .await
            .map_err(|e| DevError::Config(format!("failed to install {binary}: {e}")))?;
    }

    // Replace current process with the tool
    use std::os::unix::process::CommandExt;
    let err = std::process::Command::new(binary).args(args).exec();
    Err(DevError::Process {
        service: binary.to_string(),
        msg: err.to_string(),
    })
}

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

async fn stream_logs(service: Option<String>, follow: bool) -> Result<()> {
    // First replay history (last 200 lines)
    {
        let stream = UnixStream::connect(socket_path()).await.map_err(|_| {
            DevError::Config("no running a3s daemon — run `a3s up` first".into())
        })?;
        let (reader, mut writer) = tokio::io::split(stream);
        let req = IpcRequest::History {
            service: service.clone(),
            lines: 200,
        };
        writer
            .write_all(format!("{}\n", serde_json::to_string(&req).unwrap()).as_bytes())
            .await?;
        let mut lines = BufReader::new(reader).lines();
        while let Ok(Some(line)) = lines.next_line().await {
            if let Ok(IpcResponse::LogLine { service: svc, line: text }) =
                serde_json::from_str::<IpcResponse>(&line)
            {
                println!("{} {}", format!("[{svc}]").dimmed(), text);
            }
        }
    }

    if !follow {
        return Ok(());
    }

    // Then stream live
    let stream = UnixStream::connect(socket_path()).await.map_err(|_| {
        DevError::Config("no running a3s daemon — run `a3s up` first".into())
    })?;
    let (reader, mut writer) = tokio::io::split(stream);
    let req = IpcRequest::Logs { service, follow: true };
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

const INIT_TEMPLATE: &str = r#"# A3sfile.hcl — generated by `a3s init`
# Run `a3s up` to start all services, `a3s validate` to check config.

dev {
  proxy_port = 7080
  log_level  = "info"
}

# brew {
#   packages = [
#     "redis",
#     "postgresql@16",
#   ]
# }

# service "api" {
#   cmd        = "cargo run -p my-api"
#   dir        = "./services/api"
#   port       = 3000
#   subdomain  = "api"
#   depends_on = ["db"]
#
#   env = {
#     DATABASE_URL = "postgres://localhost:5432/dev"
#   }
#
#   watch {
#     paths   = ["./services/api/src"]
#     ignore  = ["target"]
#     restart = true
#   }
#
#   health {
#     type     = "http"
#     path     = "/health"
#     interval = "2s"
#     timeout  = "1s"
#     retries  = 5
#   }
# }
#
# service "db" {
#   cmd  = "postgres -D /usr/local/var/postgresql@16"
#   port = 5432
#
#   health {
#     type    = "tcp"
#     timeout = "1s"
#     retries = 10
#   }
# }
"#;
