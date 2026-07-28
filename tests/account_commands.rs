#![cfg(unix)]

mod support;

use std::io::Write;
use std::process::{Command, Output, Stdio};

use support::{a3s_bin, make_executable, TempWorkspace};

fn run(home: &std::path::Path, config: &std::path::Path, args: &[&str]) -> Output {
    command(home, config, args)
        .output()
        .unwrap_or_else(|error| panic!("failed to run a3s {args:?}: {error}"))
}

fn run_with_stdin(
    home: &std::path::Path,
    config: &std::path::Path,
    args: &[&str],
    input: &str,
) -> Output {
    let mut command = command(home, config, args);
    command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command
        .spawn()
        .unwrap_or_else(|error| panic!("failed to run a3s {args:?}: {error}"));
    child
        .stdin
        .take()
        .expect("piped stdin")
        .write_all(input.as_bytes())
        .expect("write protected token input");
    child.wait_with_output().expect("collect a3s output")
}

fn command(home: &std::path::Path, config: &std::path::Path, args: &[&str]) -> Command {
    let mut command = Command::new(a3s_bin());
    command
        .args(args)
        .env("HOME", home)
        .env("A3S_CONFIG_FILE", config)
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CODEX_HOME")
        .env_remove("A3S_KIMI_HOME")
        .env_remove("A3S_KIMI_DESKTOP_HOME")
        .env_remove("A3S_KIMI_BASE_URL")
        .env_remove("A3S_KIMI_OAUTH_HOST")
        .env_remove("KIMI_CODE_HOME")
        .env_remove("KIMI_SHARE_DIR")
        .env_remove("KIMI_DESKTOP_HOME")
        .env_remove("KIMI_CODE_BASE_URL")
        .env_remove("KIMI_CODE_OAUTH_HOST")
        .env("A3S_CODEBUDDY_CLI", home.join("bin/codebuddy"))
        .env("PATH", home.join("bin"))
        .env("RUST_BACKTRACE", "0");
    command
}

#[test]
fn external_account_commands_never_mutate_owner_credentials() {
    let workspace = TempWorkspace::new("account-product-delegation");
    let home = workspace.path("home");
    let config = workspace.path("config.acl");
    let log = workspace.path("product.log");
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(&config, "").unwrap();
    for product in ["claude", "codex"] {
        make_executable(
            &home.join("bin").join(product),
            &format!(
                "#!/bin/sh\nprintf '{}:%s\\n' \"$*\" >> '{}'\n",
                product,
                log.display()
            ),
        );
    }

    for args in [
        &["auth", "login", "claude-code"][..],
        &["auth", "logout", "claude-code"][..],
        &["auth", "login", "codex"][..],
        &["auth", "logout", "codex"][..],
        &["auth", "login", "kimi"][..],
        &["auth", "logout", "kimi"][..],
    ] {
        let output = run(&home, &config, args);
        assert!(
            !output.status.success(),
            "external credential mutation unexpectedly succeeded for {args:?}"
        );
    }
    assert!(!log.exists(), "A3S must not invoke an external login owner");
}

#[test]
fn account_list_reports_product_owned_login_sources_without_secrets() {
    let workspace = TempWorkspace::new("account-list");
    let home = workspace.path("home");
    let config = workspace.path("config.acl");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(&config, "os = \"http://127.0.0.1:1\"\n").unwrap();
    let daimon = home.join(".config/kimi-desktop/daimon-share/daimon");
    std::fs::create_dir_all(&daimon).unwrap();
    std::fs::write(
        daimon.join("kimi-code-key.json"),
        r#"{"apiKey":"kimi-account-secret"}"#,
    )
    .unwrap();
    std::fs::write(
        daimon.join("config.json"),
        r#"{"model":{"current":"k3-agent","providers":{"kimi":{"type":"kimi","baseUrl":"https://example.invalid/coding/v1"}},"models":{"k3-agent":{"provider":"kimi","model":"k3-agent","maxContextSize":262144,"capabilities":["thinking"]}}}}"#,
    )
    .unwrap();

    let login = run_with_stdin(
        &home,
        &config,
        &[
            "--offline",
            "--json",
            "auth",
            "login",
            "os",
            "--token-stdin",
        ],
        "secret",
    );
    assert!(login.status.success());
    assert!(!String::from_utf8_lossy(&login.stdout).contains("secret"));

    let output = run(&home, &config, &["--json", "auth", "list"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let providers = value["data"]["providers"].as_array().unwrap();
    let os = providers
        .iter()
        .find(|provider| provider["id"] == "os")
        .unwrap();
    assert_eq!(os["ownership"], "managed");
    assert_eq!(os["signedIn"], true);
    for id in ["claude-code", "codex", "kimi", "workbuddy"] {
        let provider = providers
            .iter()
            .find(|provider| provider["id"] == id)
            .unwrap();
        assert_eq!(provider["ownership"], "external");
    }
    let kimi = providers
        .iter()
        .find(|provider| provider["id"] == "kimi")
        .unwrap();
    assert_eq!(kimi["signedIn"], true);
    assert!(!String::from_utf8_lossy(&output.stdout).contains("secret"));

    let logout = run(&home, &config, &["auth", "logout", "os"]);
    assert!(logout.status.success());
    assert!(!home.join(".a3s/os-auth.json").exists());
}

#[test]
fn codex_account_stays_signed_in_when_only_the_id_token_has_expired() {
    let workspace = TempWorkspace::new("account-codex-expired-id-token");
    let home = workspace.path("home");
    let config = workspace.path("config.acl");
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::write(&config, "").unwrap();
    std::fs::write(
        home.join(".codex/auth.json"),
        r#"{"tokens":{"id_token":"header.eyJleHAiOjF9.signature","access_token":"codex-secret","refresh_token":"codex-refresh-secret"}}"#,
    )
    .unwrap();

    let output = run(&home, &config, &["--json", "auth", "list"]);
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let codex = value["data"]["providers"]
        .as_array()
        .unwrap()
        .iter()
        .find(|provider| provider["id"] == "codex")
        .unwrap();
    assert_eq!(codex["signedIn"], true);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(!stdout.contains("codex-secret"));
    assert!(!stdout.contains("codex-refresh-secret"));
}
