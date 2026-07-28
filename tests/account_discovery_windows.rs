#![cfg(windows)]

mod support;

use std::process::Command;

use support::{a3s_bin, TempWorkspace};

#[test]
fn local_accounts_use_the_native_windows_profile_without_home() {
    let workspace = TempWorkspace::new("account-discovery-windows");
    let profile = workspace.path("profile");
    let local_app_data = workspace.path("local-app-data");
    let app_data = workspace.path("app-data");
    let config = workspace.path("config.acl");

    std::fs::create_dir_all(profile.join(".claude")).unwrap();
    std::fs::write(
        profile.join(".claude/.credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"claude-secret"}}"#,
    )
    .unwrap();

    std::fs::create_dir_all(profile.join(".codex")).unwrap();
    std::fs::write(
        profile.join(".codex/auth.json"),
        r#"{"tokens":{"access_token":"codex-secret"}}"#,
    )
    .unwrap();

    std::fs::create_dir_all(profile.join(".kimi-code/credentials")).unwrap();
    std::fs::write(
        profile.join(".kimi-code/credentials/kimi-code.json"),
        r#"{"access_token":"","refresh_token":"kimi-secret","expires_at":0}"#,
    )
    .unwrap();

    std::fs::create_dir_all(profile.join(".workbuddy")).unwrap();
    std::fs::write(
        profile.join(".workbuddy/settings.json"),
        r#"{"privateAccountState":"workbuddy-secret"}"#,
    )
    .unwrap();
    let workbuddy = local_app_data.join("Programs/WorkBuddy");
    std::fs::create_dir_all(workbuddy.join("resources/app.asar.unpacked/cli/bin")).unwrap();
    std::fs::write(workbuddy.join("WorkBuddy.exe"), []).unwrap();
    std::fs::write(workbuddy.join("resources/app.asar"), []).unwrap();
    std::fs::write(
        workbuddy.join("resources/app.asar.unpacked/cli/bin/codebuddy"),
        "// bundled CodeBuddy CLI fixture\n",
    )
    .unwrap();

    std::fs::write(&config, "").unwrap();
    std::fs::create_dir_all(&app_data).unwrap();

    let output = Command::new(a3s_bin())
        .args(["--json", "auth", "list"])
        .env("A3S_CONFIG_FILE", &config)
        .env("USERPROFILE", &profile)
        .env("LOCALAPPDATA", &local_app_data)
        .env("APPDATA", &app_data)
        .env("PATH", "")
        .env_remove("HOME")
        .env_remove("CLAUDE_CONFIG_DIR")
        .env_remove("CLAUDE_CODE_OAUTH_TOKEN")
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .env_remove("CODEX_HOME")
        .env_remove("A3S_KIMI_HOME")
        .env_remove("A3S_KIMI_DESKTOP_HOME")
        .env_remove("KIMI_CODE_HOME")
        .env_remove("KIMI_SHARE_DIR")
        .env_remove("KIMI_DESKTOP_HOME")
        .env_remove("XDG_CONFIG_HOME")
        .env_remove("WORKBUDDY_CONFIG_DIR")
        .env_remove("CODEBUDDY_CONFIG_DIR")
        .env_remove("A3S_CODEBUDDY_CLI")
        .output()
        .expect("run Windows account discovery");

    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let providers = response["data"]["providers"].as_array().unwrap();
    for id in ["claude-code", "codex", "kimi", "workbuddy"] {
        let provider = providers
            .iter()
            .find(|provider| provider["id"] == id)
            .unwrap_or_else(|| panic!("missing provider {id}: {response}"));
        assert_eq!(provider["signedIn"], true, "provider {id}: {response}");
    }
    for secret in [
        "claude-secret",
        "codex-secret",
        "kimi-secret",
        "workbuddy-secret",
    ] {
        assert!(!response.to_string().contains(secret));
    }
}
