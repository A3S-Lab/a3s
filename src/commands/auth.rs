use std::ffi::OsStr;
use std::path::Path;

use a3s_code_core::config::OsConfig;
use anyhow::{bail, Context};
use serde_json::json;
use tokio::io::AsyncReadExt;

use crate::account_providers::AccountProvider;
use crate::cli::args::{AuthArgs, AuthCommand, AuthLoginArgs};
use crate::cli::context::InvocationContext;
use crate::cli::output::render_value;

const MAX_TOKEN_BYTES: u64 = 64 * 1024;

pub(crate) async fn run(args: AuthArgs, context: &InvocationContext) -> anyhow::Result<()> {
    match args.command {
        AuthCommand::List => list(context),
        AuthCommand::Status(args) => status(&args.provider, context).await,
        AuthCommand::Login(args) => login(args, context).await,
        AuthCommand::Logout(args) => logout(&args.provider, context),
    }
}

fn list(context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let os_config = super::config::load_active_config(context)
        .ok()
        .and_then(|(_, config)| config.os);
    let os_signed_in = os_config
        .as_ref()
        .and_then(crate::a3s_os::current_session)
        .is_some();
    let claude_signed_in = AccountProvider::Claude.is_available();
    let codex_signed_in = AccountProvider::Codex.is_available();
    let kimi_signed_in = AccountProvider::Kimi.is_available();
    let workbuddy_signed_in = AccountProvider::CodeBuddy.is_available();
    let providers = json!([
        {
            "id": "os",
            "ownership": "managed",
            "configured": os_config.is_some(),
            "signedIn": os_signed_in,
        },
        {
            "id": "claude-code",
            "ownership": "external",
            "configured": true,
            "signedIn": claude_signed_in,
        },
        {
            "id": "codex",
            "ownership": "external",
            "configured": true,
            "signedIn": codex_signed_in,
        },
        {
            "id": "kimi",
            "ownership": "external",
            "configured": true,
            "signedIn": kimi_signed_in,
        },
        {
            "id": "workbuddy",
            "ownership": "external",
            "configured": true,
            "signedIn": workbuddy_signed_in,
        }
    ]);
    render_value(output, "auth.list", json!({"providers": providers}), || {
        println!("PROVIDER       OWNERSHIP  STATUS");
        println!(
            "os             managed    {}",
            if os_signed_in {
                "signed in"
            } else if os_config.is_some() {
                "signed out"
            } else {
                "not configured"
            }
        );
        println!(
            "claude-code    external   {}",
            if claude_signed_in {
                "signed in"
            } else {
                "signed out"
            }
        );
        println!(
            "codex          external   {}",
            if codex_signed_in {
                "signed in"
            } else {
                "signed out"
            }
        );
        println!(
            "kimi           external   {}",
            if kimi_signed_in {
                "signed in"
            } else {
                "signed out"
            }
        );
        println!(
            "workbuddy      external   {}",
            if workbuddy_signed_in {
                "signed in"
            } else {
                "signed out"
            }
        );
    })
}

async fn status(provider: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    ensure_os_provider(provider)?;
    let (config_path, os_config) = load_os_config(context)?;
    let mut session = crate::a3s_os::current_session(&os_config);
    if !context.network.offline && session.as_ref().is_some_and(crate::a3s_os::needs_refresh) {
        let refreshed =
            crate::a3s_os::refresh_session(session.as_ref().expect("session was checked above"))
                .await
                .context("could not refresh the OS session")?;
        session = Some(refreshed);
    }
    let signed_in = session.is_some();
    let account = session.as_ref().map(|value| value.display_label());
    let login_at = session
        .as_ref()
        .map(|value| format_unix_ms(value.login_at_ms));
    let expires_at = session
        .as_ref()
        .and_then(|value| value.expires_at_ms)
        .map(format_unix_ms);
    let data = json!({
        "provider": "os",
        "configured": true,
        "signedIn": signed_in,
        "address": os_config.address,
        "account": account,
        "loginAt": login_at,
        "expiresAt": expires_at,
        "configPath": config_path,
    });
    render_value(output, "auth.status", data, || {
        println!("provider: os");
        println!("config: {}", config_path.display());
        println!("address: {}", os_config.address);
        println!(
            "status: {}",
            if signed_in { "signed in" } else { "signed out" }
        );
        if let Some(account) = account {
            println!("account: {account}");
        }
        if let Some(login_at) = login_at {
            println!("login at: {login_at}");
        }
        if let Some(expires_at) = expires_at {
            println!("expires at: {expires_at}");
        }
    })
}

async fn login(args: AuthLoginArgs, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    let provider_is_os = args
        .provider_or_legacy
        .as_deref()
        .is_none_or(|value| value == OsStr::new("os"));
    if !provider_is_os || !args.legacy_values.is_empty() {
        bail!(
            "positional credentials are not accepted and only provider `os` is currently managed; use `a3s auth login os --token-stdin`"
        );
    }

    let (_, os_config) = load_os_config(context)?;
    let session = if args.token_stdin {
        let token = read_token_stdin().await?;
        crate::a3s_os::login_with_token(&os_config, &token).await?
    } else if let Some(path) = args.token_file {
        let token = read_protected_token_file(&path)?;
        crate::a3s_os::login_with_token(&os_config, &token).await?
    } else {
        if context.network.offline {
            bail!("browser login is unavailable in offline mode");
        }
        if context.interaction.non_interactive {
            bail!(
                "browser login requires an interactive invocation; use --token-stdin or --token-file"
            );
        }
        crate::a3s_os::login_via_browser(os_config.clone()).await?
    };

    let skill_ready = crate::a3s_os::ensure_capability_skill_dir(&os_config).is_some();
    let account = session.display_label();
    let address = session.address.clone();
    let ssh = if context.network.offline {
        "not synchronized (offline)".to_string()
    } else {
        ssh_key_result(crate::a3s_os::sync_ssh_key(session).await)
    };
    let data = json!({
        "provider": "os",
        "signedIn": true,
        "account": account,
        "address": address,
        "capabilitySkill": skill_ready,
        "sshKey": ssh,
    });
    render_value(output, "auth.login", data, || {
        println!("signed in to OS as {account}");
        println!(
            "capabilities skill: {}",
            if skill_ready {
                "active"
            } else {
                "not installed"
            }
        );
        println!("ssh key: {ssh}");
    })
}

fn logout(provider: &str, context: &InvocationContext) -> anyhow::Result<()> {
    let output = context.output_mode();
    ensure_os_provider(provider)?;
    let (_, os_config) = load_os_config(context)?;
    let removed = crate::a3s_os::logout(&os_config)?;
    crate::a3s_os::remove_capability_skill_dir();
    let data = json!({
        "provider": "os",
        "signedOut": removed,
        "address": os_config.address,
    });
    render_value(output, "auth.logout", data, || {
        if removed {
            println!("signed out from OS");
        } else {
            println!("no stored OS login for {}", os_config.address);
        }
    })
}

fn ensure_os_provider(provider: &str) -> anyhow::Result<()> {
    if provider == "os" {
        Ok(())
    } else {
        bail!("only the managed `os` authentication provider is currently supported")
    }
}

fn load_os_config(context: &InvocationContext) -> anyhow::Result<(std::path::PathBuf, OsConfig)> {
    let (path, config) = super::config::load_active_config(context)?;
    let os = config.os.ok_or_else(|| {
        anyhow::anyhow!(
            "OS is not configured in {}; add `os = \"https://your-os-host\"`",
            path.display()
        )
    })?;
    Ok((path, os))
}

async fn read_token_stdin() -> anyhow::Result<String> {
    let mut input = Vec::new();
    let mut stdin = tokio::io::stdin().take(MAX_TOKEN_BYTES + 1);
    stdin
        .read_to_end(&mut input)
        .await
        .context("could not read protected token input from stdin")?;
    decode_token(input)
}

fn read_protected_token_file(path: &Path) -> anyhow::Result<String> {
    let metadata = std::fs::symlink_metadata(path)
        .with_context(|| format!("could not inspect token file {}", path.display()))?;
    if metadata.file_type().is_symlink() || !metadata.file_type().is_file() {
        bail!("token file must be a regular file and must not be a symbolic link");
    }
    if metadata.len() > MAX_TOKEN_BYTES {
        bail!("token input exceeds the maximum supported size");
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            bail!("token file permissions are too broad; use mode 0600");
        }
    }
    let input = std::fs::read(path)
        .with_context(|| format!("could not read token file {}", path.display()))?;
    decode_token(input)
}

fn decode_token(input: Vec<u8>) -> anyhow::Result<String> {
    if input.len() as u64 > MAX_TOKEN_BYTES {
        bail!("token input exceeds the maximum supported size");
    }
    let token = String::from_utf8(input).context("token input must be UTF-8")?;
    if token.trim().is_empty() {
        bail!("token input is empty");
    }
    Ok(token)
}

fn ssh_key_result(outcome: crate::a3s_os::SshKeyOutcome) -> String {
    match outcome {
        crate::a3s_os::SshKeyOutcome::Registered(fingerprint) => {
            format!("registered ({fingerprint})")
        }
        crate::a3s_os::SshKeyOutcome::AlreadyRegistered => "already registered".to_string(),
        crate::a3s_os::SshKeyOutcome::NoLocalKey => "not found locally".to_string(),
        crate::a3s_os::SshKeyOutcome::Failed(error) => format!("sync skipped: {error}"),
    }
}

fn format_unix_ms(ms: u64) -> String {
    let seconds = (ms / 1000) as i64;
    let nanoseconds = ((ms % 1000) * 1_000_000) as u32;
    chrono::DateTime::<chrono::Utc>::from_timestamp(seconds, nanoseconds)
        .map(|value| value.to_rfc3339())
        .unwrap_or_else(|| ms.to_string())
}
