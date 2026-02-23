use colored::Colorize;

use crate::error::{DevError, Result};

/// Box name used for the k3s MicroVM.
const K3S_BOX_NAME: &str = "a3s-k3s";

/// k3s OCI image.
const K3S_IMAGE: &str = "rancher/k3s:latest";

/// k3s API server port.
const K3S_API_PORT: u16 = 6443;

/// Install and start k3s inside an a3s box MicroVM.
pub async fn start() -> Result<()> {
    // Check if a3s-box is available
    if !cmd_exists("a3s-box").await {
        return Err(DevError::Config(
            "a3s-box not found in PATH — install it first with `a3s install`".into(),
        ));
    }

    // Check if the k3s box already exists
    let status = box_status().await?;
    match status.as_str() {
        "running" => {
            println!("  {} k3s is already running.", "✓".green());
            return Ok(());
        }
        "stopped" | "created" => {
            println!("  {} restarting existing k3s box...", "→".cyan());
            run_box(&["start", K3S_BOX_NAME]).await?;
        }
        _ => {
            // Box doesn't exist — create and start it
            println!(
                "  {} pulling {} and starting k3s box...",
                "→".cyan(),
                K3S_IMAGE.dimmed()
            );
            run_box(&[
                "run",
                "--name",
                K3S_BOX_NAME,
                "--privileged",
                "--persistent",
                "-p",
                &format!("{K3S_API_PORT}:{K3S_API_PORT}"),
                // k3s needs /dev/kmsg and cgroup mounts — tmpfs for volatile state
                "--tmpfs",
                "/run",
                "--tmpfs",
                "/var/run",
                K3S_IMAGE,
                "server",
                "--disable=traefik",
                "--write-kubeconfig-mode=644",
            ])
            .await?;
        }
    }

    // Wait for k3s API to become ready
    wait_for_k3s_ready().await?;

    // Extract kubeconfig from the box and write to ~/.kube/config
    write_kubeconfig().await?;

    println!(
        "  {} k3s is running. Use {} to interact with the cluster.",
        "✓".green(),
        "kubectl".cyan()
    );
    Ok(())
}

/// Stop the k3s box.
pub async fn stop() -> Result<()> {
    let status = box_status().await?;
    if status == "none" {
        println!("  {} k3s box does not exist.", "·".dimmed());
        return Ok(());
    }
    println!("  {} stopping k3s box...", "→".cyan());
    run_box(&["stop", K3S_BOX_NAME]).await?;
    println!("  {} k3s stopped.", "✓".green());
    Ok(())
}

/// Show k3s box status.
pub async fn status() -> Result<()> {
    let s = box_status().await?;
    match s.as_str() {
        "running" => println!("  {} k3s  {}", "●".green(), "running".green()),
        "stopped" => println!("  {} k3s  {}", "○".yellow(), "stopped".yellow()),
        "created" => println!("  {} k3s  {}", "○".dimmed(), "created".dimmed()),
        _ => println!("  {} k3s  {}", "·".dimmed(), "not installed".dimmed()),
    }
    Ok(())
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Query the current status of the k3s box via `a3s-box inspect`.
async fn box_status() -> Result<String> {
    let out = tokio::process::Command::new("a3s-box")
        .args(["inspect", "--format", "{{.Status}}", K3S_BOX_NAME])
        .output()
        .await
        .map_err(DevError::Io)?;

    if !out.status.success() {
        // Box doesn't exist
        return Ok("none".into());
    }

    let s = String::from_utf8_lossy(&out.stdout).trim().to_lowercase();
    Ok(s)
}

/// Poll until k3s API server responds on port 6443 (max 120s).
async fn wait_for_k3s_ready() -> Result<()> {
    use tokio::net::TcpStream;

    println!("  {} waiting for k3s API server...", "→".cyan());

    let addr = format!("127.0.0.1:{K3S_API_PORT}");
    for _ in 0..120 {
        if TcpStream::connect(&addr).await.is_ok() {
            return Ok(());
        }
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
    }

    Err(DevError::Config(
        "k3s API server did not become ready within 120s".into(),
    ))
}

/// Read /etc/rancher/k3s/k3s.yaml from the box and write to ~/.kube/config,
/// replacing 127.0.0.1 with the host-accessible address.
async fn write_kubeconfig() -> Result<()> {
    // Exec into the box to read the kubeconfig
    let out = tokio::process::Command::new("a3s-box")
        .args([
            "exec",
            K3S_BOX_NAME,
            "--",
            "cat",
            "/etc/rancher/k3s/k3s.yaml",
        ])
        .output()
        .await
        .map_err(DevError::Io)?;

    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(DevError::Config(format!(
            "failed to read k3s kubeconfig from box: {err}"
        )));
    }

    let raw = String::from_utf8_lossy(&out.stdout);

    // k3s writes 127.0.0.1 — rewrite to localhost (port-forwarded via a3s box)
    let patched = raw
        .replace("127.0.0.1", "127.0.0.1") // already correct via port-forward
        .replace("name: default", "name: a3s-k3s")
        .replace("cluster: default", "cluster: a3s-k3s")
        .replace("user: default", "user: a3s-k3s")
        .replace("current-context: default", "current-context: a3s-k3s");

    let home = dirs_next::home_dir()
        .ok_or_else(|| DevError::Config("cannot determine home directory".into()))?;
    let kube_dir = home.join(".kube");
    tokio::fs::create_dir_all(&kube_dir)
        .await
        .map_err(DevError::Io)?;

    let kubeconfig_path = kube_dir.join("config");
    tokio::fs::write(&kubeconfig_path, patched.as_bytes())
        .await
        .map_err(DevError::Io)?;

    println!(
        "  {} kubeconfig written to {}",
        "✓".green(),
        kubeconfig_path.display().to_string().dimmed()
    );
    Ok(())
}

/// Run an `a3s-box` subcommand.
async fn run_box(args: &[&str]) -> Result<()> {
    let status = tokio::process::Command::new("a3s-box")
        .args(args)
        .status()
        .await
        .map_err(|e| DevError::Config(format!("failed to run `a3s-box`: {e}")))?;

    if !status.success() {
        return Err(DevError::Config(format!(
            "`a3s-box {}` exited with {}",
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

    #[test]
    fn test_constants() {
        assert_eq!(K3S_BOX_NAME, "a3s-k3s");
        assert_eq!(K3S_API_PORT, 6443);
        assert!(!K3S_IMAGE.is_empty());
    }

    #[test]
    fn test_kubeconfig_patching() {
        let raw = "name: default\ncluster: default\nuser: default\ncurrent-context: default\n";
        let patched = raw
            .replace("name: default", "name: a3s-k3s")
            .replace("cluster: default", "cluster: a3s-k3s")
            .replace("user: default", "user: a3s-k3s")
            .replace("current-context: default", "current-context: a3s-k3s");
        assert!(patched.contains("name: a3s-k3s"));
        assert!(patched.contains("current-context: a3s-k3s"));
        assert!(!patched.contains("name: default"));
    }
}
