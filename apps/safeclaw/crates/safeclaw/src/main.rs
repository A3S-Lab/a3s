//! SafeClaw - Secure Personal AI Assistant with TEE Support
//!
//! A privacy-focused personal AI assistant that combines multi-channel
//! messaging capabilities with hardware-isolated execution for sensitive data.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use safeclaw::config::{
    ChannelsConfig, DingTalkConfig, DiscordConfig, FeishuConfig, SafeClawConfig, ServerConfig,
    SlackConfig, TeeBackend, TeeConfig, TelegramConfig, WeComConfig, WebChatConfig,
};
use std::io::{self, BufRead, Write};
use std::path::PathBuf;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[derive(Parser)]
#[command(name = "safeclaw")]
#[command(author = "A3S Lab Team")]
#[command(version)]
#[command(about = "Secure Personal AI Assistant with TEE Support")]
struct Cli {
    /// Configuration file path (.hcl)
    #[arg(short, long, env = "SAFECLAW_CONFIG")]
    config: Option<PathBuf>,

    /// Enable verbose logging
    #[arg(short, long)]
    verbose: bool,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the SafeClaw gateway
    Gateway {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value = "18790")]
        port: u16,

        /// Disable TEE mode
        #[arg(long)]
        no_tee: bool,
    },

    /// Start SafeClaw as a backend service (behind a3s-gateway)
    Serve {
        /// Host to bind to
        #[arg(long, default_value = "127.0.0.1")]
        host: String,

        /// Port to listen on
        #[arg(long, default_value = "18790")]
        port: u16,

        /// Disable TEE mode
        #[arg(long)]
        no_tee: bool,
    },

    /// Generate a3s-gateway routing configuration for SafeClaw
    ServerConfig {
        /// Output file path (stdout if not specified)
        #[arg(short, long)]
        output: Option<std::path::PathBuf>,
    },

    /// Run the onboarding wizard
    Onboard {
        /// Install as system daemon
        #[arg(long)]
        install_daemon: bool,
    },

    /// Send a message
    Message {
        /// Target channel
        #[arg(short, long)]
        channel: String,

        /// Target chat ID
        #[arg(short = 't', long)]
        to: String,

        /// Message content
        #[arg(short, long)]
        message: String,

        /// Gateway URL to connect to
        #[arg(long, default_value = "http://127.0.0.1:18790")]
        gateway_url: String,
    },

    /// Run diagnostics
    Doctor,

    /// Show configuration
    Config {
        /// Show default configuration
        #[arg(long)]
        default: bool,
    },

    /// Update safeclaw to the latest version
    Update,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // Apply OS-level hardening before loading any secrets
    if let Err(e) = safeclaw::hardening::harden_process() {
        eprintln!("Warning: process hardening failed: {}", e);
    }

    // Initialize logging
    let log_level = if cli.verbose { "debug" } else { "info" };
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| format!("safeclaw={},tower_http=debug", log_level).into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration via shared bootstrap
    let (mut config, config_path) = safeclaw::bootstrap::load_config(cli.config.as_ref())?;

    match cli.command {
        Commands::Update => {
            return a3s_updater::run_update(&a3s_updater::UpdateConfig {
                binary_name: "safeclaw",
                crate_name: "safeclaw",
                current_version: env!("CARGO_PKG_VERSION"),
                github_owner: "A3S-Lab",
                github_repo: "SafeClaw",
            })
            .await;
        }
        Commands::Gateway { host, port, no_tee } => {
            safeclaw::bootstrap::run_gateway(config, config_path, &host, port, !no_tee).await?;
        }
        Commands::Serve { host, port, no_tee } => {
            config.a3s_gateway.enabled = true;
            safeclaw::bootstrap::run_gateway(config, config_path, &host, port, !no_tee).await?;
        }
        Commands::ServerConfig { output } => {
            generate_gateway_config(&config, output.as_deref())?;
        }
        Commands::Onboard { install_daemon } => {
            run_onboard(install_daemon).await?;
        }
        Commands::Message {
            channel,
            to,
            message,
            gateway_url,
        } => {
            send_message(&channel, &to, &message, &gateway_url).await?;
        }
        Commands::Doctor => {
            run_doctor().await?;
        }
        Commands::Config { default } => {
            show_config(if default { None } else { Some(&config) })?;
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// generate_gateway_config — output a3s-gateway config for SafeClaw
// ---------------------------------------------------------------------------

fn generate_gateway_config(
    _config: &SafeClawConfig,
    output: Option<&std::path::Path>,
) -> Result<()> {
    let descriptor = safeclaw::runtime::build_service_descriptor();
    let json = serde_json::to_string_pretty(&descriptor)
        .context("Failed to serialize service descriptor")?;

    if let Some(path) = output {
        std::fs::write(path, &json)
            .with_context(|| format!("Failed to write service descriptor to {}", path.display()))?;
        println!("Service descriptor written to: {}", path.display());
    } else {
        println!("{}", json);
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// send_message — POST to running gateway
// ---------------------------------------------------------------------------

async fn send_message(channel: &str, to: &str, message: &str, gateway_url: &str) -> Result<()> {
    let client = reqwest::Client::new();

    let health_url = format!("{}/health", gateway_url);
    let health_resp = client
        .get(&health_url)
        .send()
        .await
        .context("Failed to connect to gateway. Is it running?")?;

    if !health_resp.status().is_success() {
        anyhow::bail!(
            "Gateway health check failed with status {}",
            health_resp.status()
        );
    }

    let health: serde_json::Value = health_resp
        .json()
        .await
        .context("Failed to parse health response")?;
    println!(
        "Gateway is healthy (version: {})",
        health["version"].as_str().unwrap_or("unknown")
    );

    let msg_url = format!("{}/message", gateway_url);
    let body = serde_json::json!({
        "channel": channel,
        "chat_id": to,
        "content": message,
    });

    let resp = client
        .post(&msg_url)
        .json(&body)
        .send()
        .await
        .context("Failed to send message to gateway")?;

    let status = resp.status();
    let resp_body: serde_json::Value = resp
        .json()
        .await
        .context("Failed to parse message response")?;

    if status.is_success() {
        println!(
            "Message sent! ID: {}, Status: {}",
            resp_body["message_id"].as_str().unwrap_or("unknown"),
            resp_body["status"].as_str().unwrap_or("unknown"),
        );
    } else {
        anyhow::bail!(
            "Failed to send message (HTTP {}): {}",
            status,
            resp_body["status"].as_str().unwrap_or("unknown error"),
        );
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// prompt_input — interactive stdin helper
// ---------------------------------------------------------------------------

fn prompt_input(question: &str, default: &str) -> String {
    prompt_input_from(
        question,
        default,
        &mut io::stdin().lock(),
        &mut io::stdout(),
    )
}

fn prompt_input_from(
    question: &str,
    default: &str,
    reader: &mut dyn BufRead,
    writer: &mut dyn Write,
) -> String {
    if default.is_empty() {
        let _ = write!(writer, "{}: ", question);
    } else {
        let _ = write!(writer, "{} [{}]: ", question, default);
    }
    let _ = writer.flush();

    let mut input = String::new();
    let _ = reader.read_line(&mut input);
    let trimmed = input.trim();
    if trimmed.is_empty() {
        default.to_string()
    } else {
        trimmed.to_string()
    }
}

fn prompt_yes_no(question: &str, default_yes: bool) -> bool {
    let default_str = if default_yes { "Y/n" } else { "y/N" };
    let answer = prompt_input(&format!("{} [{}]", question, default_str), "");
    if answer.is_empty() {
        return default_yes;
    }
    matches!(answer.to_lowercase().as_str(), "y" | "yes")
}

// ---------------------------------------------------------------------------
// run_onboard — interactive setup wizard
// ---------------------------------------------------------------------------

async fn run_onboard(install_daemon: bool) -> Result<()> {
    println!("SafeClaw Onboarding Wizard");
    println!("=========================");
    println!();
    println!("SafeClaw is a secure personal AI assistant that protects your privacy");
    println!("by running sensitive computations in a Trusted Execution Environment (TEE).");
    println!();

    println!("--- Step 1: Gateway Configuration ---");
    let host = prompt_input("Gateway host", "127.0.0.1");
    let port: u16 = prompt_input("Gateway port", "18790")
        .parse()
        .unwrap_or(18790);
    println!();

    println!("--- Step 2: TEE Configuration ---");
    let tee_enabled = prompt_yes_no("Enable TEE?", true);
    let mut tee_backend = TeeBackend::A3sBox;
    let mut memory_mb: u32 = 2048;
    let mut cpu_cores: u32 = 2;

    if tee_enabled {
        let backend_str = prompt_input(
            "TEE backend (a3s_box / intel_sgx / amd_sev / arm_trustzone)",
            "a3s_box",
        );
        tee_backend = match backend_str.as_str() {
            "intel_sgx" => TeeBackend::IntelSgx,
            "amd_sev" => TeeBackend::AmdSev,
            "arm_trustzone" => TeeBackend::ArmTrustzone,
            _ => TeeBackend::A3sBox,
        };
        memory_mb = prompt_input("Memory (MB)", "2048").parse().unwrap_or(2048);
        cpu_cores = prompt_input("CPU cores", "2").parse().unwrap_or(2);
    }
    println!();

    println!("--- Step 3: Channel Selection ---");
    let mut channels = ChannelsConfig::default();

    if prompt_yes_no("Enable Telegram?", false) {
        let bot_token = prompt_input("  Telegram bot_token", "telegram_bot_token");
        let users_str = prompt_input(
            "  Allowed user IDs (comma-separated, or empty for none)",
            "",
        );
        let allowed_users: Vec<i64> = users_str
            .split(',')
            .filter_map(|s| s.trim().parse().ok())
            .collect();
        channels.telegram = Some(TelegramConfig {
            bot_token,
            allowed_users,
            dm_policy: "pairing".to_string(),
        });
    }

    if prompt_yes_no("Enable Slack?", false) {
        let bot_token = prompt_input("  Slack bot_token", "slack_bot_token");
        let app_token = prompt_input("  Slack app_token", "slack_app_token");
        channels.slack = Some(SlackConfig {
            bot_token,
            app_token,
            allowed_workspaces: vec![],
            dm_policy: "pairing".to_string(),
        });
    }

    if prompt_yes_no("Enable Discord?", false) {
        let bot_token = prompt_input("  Discord bot_token", "discord_bot_token");
        channels.discord = Some(DiscordConfig {
            bot_token,
            allowed_guilds: vec![],
            dm_policy: "pairing".to_string(),
        });
    }

    if prompt_yes_no("Enable WebChat?", true) {
        channels.webchat = Some(WebChatConfig::default());
    }

    if prompt_yes_no("Enable Feishu?", false) {
        let app_id = prompt_input("  Feishu app_id", "");
        let app_secret = prompt_input("  Feishu app_secret", "feishu_app_secret");
        channels.feishu = Some(FeishuConfig {
            app_id,
            app_secret,
            encrypt_key: String::new(),
            verification_token: String::new(),
            allowed_users: vec![],
            dm_policy: "pairing".to_string(),
        });
    }

    if prompt_yes_no("Enable DingTalk?", false) {
        let app_key = prompt_input("  DingTalk app_key", "dingtalk_app_key");
        let app_secret = prompt_input("  DingTalk app_secret", "dingtalk_app_secret");
        let robot_code = prompt_input("  DingTalk robot_code", "");
        channels.dingtalk = Some(DingTalkConfig {
            app_key,
            app_secret,
            robot_code,
            allowed_users: vec![],
            dm_policy: "pairing".to_string(),
        });
    }

    if prompt_yes_no("Enable WeCom?", false) {
        let corp_id = prompt_input("  WeCom corp_id", "");
        let agent_id: u32 = prompt_input("  WeCom agent_id", "1000001")
            .parse()
            .unwrap_or(1000001);
        let secret = prompt_input("  WeCom secret", "wecom_secret");
        channels.wecom = Some(WeComConfig {
            corp_id,
            agent_id,
            secret,
            encoding_aes_key: String::new(),
            token: String::new(),
            allowed_users: vec![],
            dm_policy: "pairing".to_string(),
        });
    }
    println!();

    println!("--- Step 4: AI Model Provider ---");
    let provider = prompt_input("Default provider (anthropic / openai)", "anthropic");
    let api_key = prompt_input("API key", &format!("{}_api_key", provider));

    let default_model_id = match provider.as_str() {
        "openai" => "gpt-4o",
        _ => "claude-sonnet-4-20250514",
    };
    let models_config = a3s_code::config::CodeConfig {
        default_model: Some(format!("{}/{}", provider, default_model_id)),
        providers: vec![a3s_code::config::ProviderConfig {
            name: provider.clone(),
            api_key: Some(api_key),
            base_url: None,
            models: vec![serde_json::from_value(serde_json::json!({
                "id": default_model_id,
                "name": default_model_id,
            }))
            .expect("valid ModelConfig")],
        }],
        ..Default::default()
    };
    println!();

    println!("--- Step 5: Writing Configuration ---");
    let config = SafeClawConfig {
        gateway: ServerConfig {
            host,
            port,
            ..ServerConfig::default()
        },
        tee: TeeConfig {
            enabled: tee_enabled,
            backend: tee_backend,
            memory_mb,
            cpu_cores,
            ..TeeConfig::default()
        },
        channels,
        models: models_config,
        ..Default::default()
    };

    let hcl_str = hcl::to_string(&config).context("Failed to serialize configuration to HCL")?;

    let config_dir = dirs::config_dir()
        .context("Could not determine config directory")?
        .join("safeclaw");
    std::fs::create_dir_all(&config_dir).with_context(|| {
        format!(
            "Failed to create config directory: {}",
            config_dir.display()
        )
    })?;

    let config_path = config_dir.join("config.hcl");
    std::fs::write(&config_path, &hcl_str)
        .with_context(|| format!("Failed to write config file: {}", config_path.display()))?;

    println!("Configuration written to: {}", config_path.display());

    if install_daemon {
        println!();
        println!("--- Step 6: Daemon Installation ---");
        install_system_daemon(&config_path)?;
    }

    println!();
    println!("Onboarding complete! Start the gateway with:");
    println!("  safeclaw gateway");

    Ok(())
}

// ---------------------------------------------------------------------------
// Daemon installation helpers
// ---------------------------------------------------------------------------

fn install_system_daemon(config_path: &std::path::Path) -> Result<()> {
    #[cfg(target_os = "macos")]
    {
        install_launchd_daemon(config_path)?;
    }
    #[cfg(target_os = "linux")]
    {
        install_systemd_daemon(config_path)?;
    }
    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        let _ = config_path;
        println!("Daemon installation is not supported on this platform.");
    }
    Ok(())
}

#[cfg(target_os = "macos")]
fn install_launchd_daemon(config_path: &std::path::Path) -> Result<()> {
    let plist_content = generate_launchd_plist(config_path)?;
    let home = std::env::var("HOME").context("HOME not set")?;
    let agents_dir = PathBuf::from(&home).join("Library/LaunchAgents");
    std::fs::create_dir_all(&agents_dir).with_context(|| {
        format!(
            "Failed to create LaunchAgents directory: {}",
            agents_dir.display()
        )
    })?;

    let plist_path = agents_dir.join("com.a3s.safeclaw.plist");
    std::fs::write(&plist_path, &plist_content)
        .with_context(|| format!("Failed to write plist: {}", plist_path.display()))?;

    println!("LaunchAgent installed: {}", plist_path.display());
    println!("Loading daemon...");

    let status = std::process::Command::new("launchctl")
        .args(["load", "-w"])
        .arg(&plist_path)
        .status()
        .context("Failed to run launchctl")?;

    if status.success() {
        println!("Daemon loaded successfully.");
    } else {
        println!("Warning: launchctl load returned non-zero exit code.");
    }

    Ok(())
}

#[cfg(target_os = "linux")]
fn install_systemd_daemon(config_path: &std::path::Path) -> Result<()> {
    let unit_content = generate_systemd_unit(config_path)?;
    let home = std::env::var("HOME").context("HOME not set")?;
    let units_dir = PathBuf::from(&home).join(".config/systemd/user");
    std::fs::create_dir_all(&units_dir).with_context(|| {
        format!(
            "Failed to create systemd user directory: {}",
            units_dir.display()
        )
    })?;

    let unit_path = units_dir.join("safeclaw.service");
    std::fs::write(&unit_path, &unit_content)
        .with_context(|| format!("Failed to write unit file: {}", unit_path.display()))?;

    println!("Systemd user unit installed: {}", unit_path.display());
    println!("Enabling and starting daemon...");

    let _ = std::process::Command::new("systemctl")
        .args(["--user", "daemon-reload"])
        .status();
    let status = std::process::Command::new("systemctl")
        .args(["--user", "enable", "--now", "safeclaw.service"])
        .status()
        .context("Failed to run systemctl")?;

    if status.success() {
        println!("Daemon enabled and started.");
    } else {
        println!("Warning: systemctl enable returned non-zero exit code.");
    }

    Ok(())
}

#[allow(dead_code)]
fn generate_launchd_plist(config_path: &std::path::Path) -> Result<String> {
    let safeclaw_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("safeclaw"));
    Ok(format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.a3s.safeclaw</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>
        <string>--config</string>
        <string>{}</string>
        <string>gateway</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/safeclaw.stdout.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/safeclaw.stderr.log</string>
</dict>
</plist>"#,
        safeclaw_bin.display(),
        config_path.display(),
    ))
}

#[allow(dead_code)]
fn generate_systemd_unit(config_path: &std::path::Path) -> Result<String> {
    let safeclaw_bin = std::env::current_exe().unwrap_or_else(|_| PathBuf::from("safeclaw"));
    Ok(format!(
        r#"[Unit]
Description=SafeClaw - Secure Personal AI Assistant
After=network.target

[Service]
Type=simple
ExecStart={} --config {} gateway
Restart=on-failure
RestartSec=5

[Install]
WantedBy=default.target"#,
        safeclaw_bin.display(),
        config_path.display(),
    ))
}

// ---------------------------------------------------------------------------
// Doctor and Config commands
// ---------------------------------------------------------------------------

async fn run_doctor() -> Result<()> {
    println!("SafeClaw Doctor");
    println!();

    println!("Checking TEE availability...");
    #[cfg(target_os = "macos")]
    {
        println!("  macOS detected - Apple Hypervisor Framework available");
    }
    #[cfg(target_os = "linux")]
    {
        println!("  Checking KVM...");
        if std::path::Path::new("/dev/kvm").exists() {
            println!("  KVM available");
        } else {
            println!("  KVM not available");
        }
    }

    println!();
    println!("Checking configuration...");
    let config_path = dirs::config_dir().map(|p| p.join("safeclaw").join("config.hcl"));
    if let Some(path) = config_path {
        if path.exists() {
            println!("  Configuration file found: {}", path.display());
        } else {
            println!("  No configuration file found (using defaults)");
        }
    }

    println!();
    println!("Doctor check complete!");

    Ok(())
}

fn show_config(config: Option<&SafeClawConfig>) -> Result<()> {
    let config = config.cloned().unwrap_or_default();
    let hcl = hcl::to_string(&config)?;
    println!("{}", hcl);
    Ok(())
}

mod dirs {
    use std::path::PathBuf;

    pub fn config_dir() -> Option<PathBuf> {
        #[cfg(target_os = "macos")]
        {
            std::env::var("HOME")
                .ok()
                .map(|h| PathBuf::from(h).join("Library/Application Support"))
        }
        #[cfg(target_os = "linux")]
        {
            std::env::var("XDG_CONFIG_HOME")
                .ok()
                .map(PathBuf::from)
                .or_else(|| {
                    std::env::var("HOME")
                        .ok()
                        .map(|h| PathBuf::from(h).join(".config"))
                })
        }
        #[cfg(target_os = "windows")]
        {
            std::env::var("APPDATA").ok().map(PathBuf::from)
        }
        #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
        {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn test_prompt_input_with_default() {
        let mut reader = Cursor::new(b"\n");
        let mut writer = Vec::new();
        let result = prompt_input_from("Host", "127.0.0.1", &mut reader, &mut writer);
        assert_eq!(result, "127.0.0.1");

        let mut reader = Cursor::new(b"0.0.0.0\n");
        let mut writer = Vec::new();
        let result = prompt_input_from("Host", "127.0.0.1", &mut reader, &mut writer);
        assert_eq!(result, "0.0.0.0");
    }

    #[test]
    fn test_daemon_plist_generation() {
        let config_path =
            PathBuf::from("/Users/test/Library/Application Support/safeclaw/config.hcl");
        let plist = generate_launchd_plist(&config_path).unwrap();
        assert!(plist.contains("com.a3s.safeclaw"));
        assert!(plist.contains("--config"));
    }

    #[test]
    fn test_daemon_systemd_generation() {
        let config_path = PathBuf::from("/home/test/.config/safeclaw/config.hcl");
        let unit = generate_systemd_unit(&config_path).unwrap();
        assert!(unit.contains("[Unit]"));
        assert!(unit.contains("[Service]"));
        assert!(unit.contains("Restart=on-failure"));
    }

    #[test]
    fn test_config_parse_hcl() {
        let hcl_str = r#"
            gateway {
                host = "0.0.0.0"
                port = 9090
            }
            tee { enabled = false }
            channels {}
            privacy {}
            models { default_model = "anthropic/claude-sonnet-4-20250514" }
            storage {}
        "#;
        let config: SafeClawConfig = hcl::from_str(hcl_str).unwrap();
        assert_eq!(config.gateway.host, "0.0.0.0");
        assert_eq!(config.gateway.port, 9090);
    }

    #[test]
    fn test_config_parse_hcl_invalid() {
        let result: std::result::Result<SafeClawConfig, _> = hcl::from_str("{{{{ invalid");
        assert!(result.is_err());
    }
}
