use colored::Colorize;

use crate::error::{DevError, Result};

/// Install and start k3s (lightweight Kubernetes).
/// Requires root/sudo on Linux. On macOS, uses a Lima VM via `limactl`.
pub async fn start() -> Result<()> {
    #[cfg(target_os = "macos")]
    return start_macos().await;

    #[cfg(target_os = "linux")]
    return start_linux().await;

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err(DevError::Config(
        "kube is only supported on macOS and Linux".into(),
    ))
}

/// Stop and clean up k3s.
pub async fn stop() -> Result<()> {
    #[cfg(target_os = "macos")]
    return stop_macos().await;

    #[cfg(target_os = "linux")]
    return stop_linux().await;

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    Err(DevError::Config(
        "kube is only supported on macOS and Linux".into(),
    ))
}

/// Show k3s status.
pub async fn status() -> Result<()> {
    #[cfg(target_os = "macos")]
    return status_macos().await;

    #[cfg(target_os = "linux")]
    return status_linux().await;

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    {
        println!("  {} kube status is only supported on macOS and Linux", "·".dimmed());
        Ok(())
    }
}

// ── macOS: Lima VM ────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
async fn start_macos() -> Result<()> {
    if !cmd_exists("limactl").await {
        println!(
            "  {} limactl not found — installing via Homebrew...",
            "→".cyan()
        );
        run("brew", &["install", "lima"]).await?;
    }

    let list = tokio::process::Command::new("limactl")
        .args(["list", "--format", "{{.Name}}"])
        .output()
        .await
        .map_err(DevError::Io)?;
    let existing = String::from_utf8_lossy(&list.stdout);

    if existing.lines().any(|l| l.trim() == "k3s") {
        println!("  {} k3s VM already exists — starting...", "→".cyan());
        run("limactl", &["start", "k3s"]).await?;
    } else {
        println!("  {} creating k3s Lima VM...", "→".cyan());
        run("limactl", &["start", "--name=k3s", "template://k3s"]).await?;
    }

    merge_kubeconfig_macos().await?;

    println!(
        "  {} k3s is running. Use {} to interact with the cluster.",
        "✓".green(),
        "kubectl".cyan()
    );
    Ok(())
}

#[cfg(target_os = "macos")]
async fn stop_macos() -> Result<()> {
    println!("  {} stopping k3s Lima VM...", "→".cyan());
    run("limactl", &["stop", "k3s"]).await?;
    println!("  {} k3s stopped.", "✓".green());
    Ok(())
}

#[cfg(target_os = "macos")]
async fn status_macos() -> Result<()> {
    if !cmd_exists("limactl").await {
        println!("  {} k3s  {}", "·".dimmed(), "not installed".dimmed());
        return Ok(());
    }

    let out = tokio::process::Command::new("limactl")
        .args(["list", "--format", "{{.Name}} {{.Status}}"])
        .output()
        .await
        .map_err(DevError::Io)?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    let line = stdout.lines().find(|l| l.starts_with("k3s "));

    match line {
        Some(l) if l.contains("Running") => {
            println!("  {} k3s  {}", "●".green(), "running".green())
        }
        Some(l) if l.contains("Stopped") => {
            println!("  {} k3s  {}", "○".yellow(), "stopped".yellow())
        }
        Some(_) => println!("  {} k3s  {}", "○".dimmed(), "exists".dimmed()),
        None => println!("  {} k3s  {}", "·".dimmed(), "not installed".dimmed()),
    }
    Ok(())
}

#[cfg(target_os = "macos")]
async fn merge_kubeconfig_macos() -> Result<()> {
    let home = dirs_next::home_dir()
        .ok_or_else(|| DevError::Config("cannot determine home directory".into()))?;
    let kube_dir = home.join(".kube");
    tokio::fs::create_dir_all(&kube_dir)
        .await
        .map_err(DevError::Io)?;

    let kubeconfig_path = kube_dir.join("config");
    let kubeconfig_str = kubeconfig_path
        .to_str()
        .ok_or_else(|| DevError::Config("kubeconfig path contains non-UTF8 characters".into()))?;

    let output = tokio::process::Command::new("limactl")
        .args(["shell", "k3s", "sudo", "cat", "/etc/rancher/k3s/k3s.yaml"])
        .output()
        .await
        .map_err(DevError::Io)?;

    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(DevError::Config(format!(
            "failed to read k3s kubeconfig: {err}"
        )));
    }

    let raw = String::from_utf8_lossy(&output.stdout);
    let ip_output = tokio::process::Command::new("limactl")
        .args(["shell", "k3s", "hostname", "-I"])
        .output()
        .await
        .map_err(DevError::Io)?;
    let ip = String::from_utf8_lossy(&ip_output.stdout)
        .split_whitespace()
        .next()
        .unwrap_or("127.0.0.1")
        .to_string();

    let patched = raw.replace("127.0.0.1", &ip).replace("default", "k3s");
    tokio::fs::write(&kubeconfig_path, patched.as_bytes())
        .await
        .map_err(DevError::Io)?;

    println!(
        "  {} kubeconfig written to {}",
        "✓".green(),
        kubeconfig_str.dimmed()
    );
    Ok(())
}

// ── Linux: native k3s ────────────────────────────────────────────────────────

#[cfg(target_os = "linux")]
async fn start_linux() -> Result<()> {
    if !cmd_exists("k3s").await {
        println!("  {} k3s not found — installing...", "→".cyan());
        install_k3s_linux().await?;
    }

    let status = tokio::process::Command::new("systemctl")
        .args(["is-active", "--quiet", "k3s"])
        .status()
        .await
        .map_err(DevError::Io)?;

    if status.success() {
        println!("  {} k3s is already running.", "✓".green());
        return Ok(());
    }

    println!("  {} starting k3s service...", "→".cyan());
    run("sudo", &["systemctl", "start", "k3s"]).await?;
    run("sudo", &["systemctl", "enable", "k3s"]).await?;

    let home = dirs_next::home_dir()
        .ok_or_else(|| DevError::Config("cannot determine home directory".into()))?;
    let kube_dir = home.join(".kube");
    tokio::fs::create_dir_all(&kube_dir)
        .await
        .map_err(DevError::Io)?;
    run(
        "sudo",
        &[
            "cp",
            "/etc/rancher/k3s/k3s.yaml",
            kube_dir.join("config").to_str().unwrap_or("/tmp/k3s.yaml"),
        ],
    )
    .await?;
    run(
        "sudo",
        &[
            "chown",
            &format!(
                "{}:{}",
                std::env::var("USER").unwrap_or_default(),
                std::env::var("USER").unwrap_or_default()
            ),
            kube_dir.join("config").to_str().unwrap_or("/tmp/k3s.yaml"),
        ],
    )
    .await?;

    println!(
        "  {} k3s is running. Use {} to interact with the cluster.",
        "✓".green(),
        "kubectl".cyan()
    );
    Ok(())
}

#[cfg(target_os = "linux")]
async fn install_k3s_linux() -> Result<()> {
    let script = tokio::process::Command::new("sh")
        .args(["-c", "curl -sfL https://get.k3s.io | sh -"])
        .status()
        .await
        .map_err(DevError::Io)?;

    if !script.success() {
        return Err(DevError::Config("k3s installation failed".into()));
    }
    Ok(())
}

#[cfg(target_os = "linux")]
async fn stop_linux() -> Result<()> {
    println!("  {} stopping k3s service...", "→".cyan());
    run("sudo", &["systemctl", "stop", "k3s"]).await?;

    if tokio::fs::metadata("/usr/local/bin/k3s-killall.sh")
        .await
        .is_ok()
    {
        println!("  {} running k3s-killall.sh...", "→".cyan());
        run("sudo", &["/usr/local/bin/k3s-killall.sh"]).await?;
    }

    println!("  {} k3s stopped.", "✓".green());
    Ok(())
}

#[cfg(target_os = "linux")]
async fn status_linux() -> Result<()> {
    if !cmd_exists("k3s").await {
        println!("  {} k3s  {}", "·".dimmed(), "not installed".dimmed());
        return Ok(());
    }

    let active = tokio::process::Command::new("systemctl")
        .args(["is-active", "--quiet", "k3s"])
        .status()
        .await
        .map_err(DevError::Io)?;

    if active.success() {
        println!("  {} k3s  {}", "●".green(), "running".green());
    } else {
        println!("  {} k3s  {}", "○".yellow(), "stopped".yellow());
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Run a command, streaming output to stdout, returning error on non-zero exit.
async fn run(program: &str, args: &[&str]) -> Result<()> {
    let status = tokio::process::Command::new(program)
        .args(args)
        .status()
        .await
        .map_err(|e| DevError::Config(format!("failed to run `{program}`: {e}")))?;

    if !status.success() {
        return Err(DevError::Config(format!(
            "`{program} {}` exited with {}",
            args.join(" "),
            status.code().map(|c| c.to_string()).unwrap_or_else(|| "?".into())
        )));
    }
    Ok(())
}

/// Check if a command exists in PATH.
async fn cmd_exists(name: &str) -> bool {
    tokio::process::Command::new("which")
        .arg(name)
        .output()
        .await
        .map(|o| o.status.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cmd_exists_true() {
        assert!(cmd_exists("sh").await);
    }

    #[tokio::test]
    async fn test_cmd_exists_false() {
        assert!(!cmd_exists("__nonexistent_binary_xyz__").await);
    }
}
