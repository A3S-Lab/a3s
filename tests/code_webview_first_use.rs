#![cfg(all(windows, target_arch = "x86_64"))]

mod support;

use std::io::{Cursor, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use support::{a3s_bin, FakeReleaseServer, TempWorkspace};

const WEBVIEW_VERSION: &str = "0.1.3";
const WEBVIEW_ARCHIVE: &str = "a3s-webview-v0.1.3-x86_64-pc-windows-msvc.zip";
const CLI_VERSION: &str = env!("CARGO_PKG_VERSION");

#[test]
fn native_windows_profile_supplies_the_user_config_path_without_home() {
    let workspace = TempWorkspace::new("config-userprofile");
    let profile = workspace.path("profile");
    let project = workspace.path("project");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::create_dir_all(&project).unwrap();

    let output = Command::new(a3s_bin())
        .current_dir(&project)
        .arg("config")
        .arg("paths")
        .env_remove("HOME")
        .env("USERPROFILE", &profile)
        .env("LOCALAPPDATA", workspace.path("local-app-data"))
        .env("A3S_DATA_HOME", workspace.path("data"))
        .env("A3S_STATE_HOME", workspace.path("state"))
        .env("A3S_CACHE_HOME", workspace.path("cache"))
        .env("A3S_RUNTIME_HOME", workspace.path("runtime"))
        .env_remove("A3S_CONFIG_FILE")
        .output()
        .expect("resolve config paths without HOME");

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8(output.stdout).unwrap();
    let expected = profile.join(".a3s/config.acl");
    assert!(stdout.contains(&expected.display().to_string()), "{stdout}");
}

#[test]
fn component_install_uses_native_windows_profile_when_home_is_unset() {
    let workspace = TempWorkspace::new("webview-userprofile-first-use");
    let release = start_fake_webview_release();
    let profile = workspace.path("profile");
    let project = workspace.path("project");
    std::fs::create_dir_all(&profile).unwrap();
    std::fs::create_dir_all(&project).unwrap();

    let output = Command::new(a3s_bin())
        .current_dir(&project)
        .arg("--no-progress")
        .arg("install")
        .arg("webview")
        .env_remove("HOME")
        .env("USERPROFILE", &profile)
        .env("LOCALAPPDATA", workspace.path("local-app-data"))
        .env("A3S_DATA_HOME", workspace.path("data"))
        .env("A3S_STATE_HOME", workspace.path("state"))
        .env("A3S_CACHE_HOME", workspace.path("cache"))
        .env("A3S_RUNTIME_HOME", workspace.path("runtime"))
        .env("A3S_UPDATER_GITHUB_API_BASE", release.api_base())
        .env("PATH", "")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env_remove("A3S_CONFIG_FILE")
        .env_remove("A3S_OFFLINE")
        .env_remove("A3S_NO_AUTO_INSTALL")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .output()
        .expect("install WebView without HOME");

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt_path = workspace.path("state/components/webview.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    assert_eq!(receipt["componentId"], "webview");
    assert_eq!(receipt["version"], WEBVIEW_VERSION);
}

#[test]
fn code_tui_first_use_installs_webview_before_the_smoke_session() {
    let workspace = TempWorkspace::new("code-webview-first-use");
    install_ready_use_fixture(&workspace);
    let release = start_fake_webview_release();
    let project = workspace.path("project");
    let home = workspace.path("home");
    let config = workspace.path("config.acl");
    std::fs::create_dir_all(&project).unwrap();
    std::fs::create_dir_all(&home).unwrap();
    write_config(&config);

    let output = Command::new(a3s_bin())
        .arg("-C")
        .arg(&project)
        .arg("--config")
        .arg(&config)
        .arg("code")
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("A3S_DATA_HOME", workspace.path("data"))
        .env("A3S_STATE_HOME", workspace.path("state"))
        .env("A3S_CACHE_HOME", workspace.path("cache"))
        .env("A3S_RUNTIME_HOME", workspace.path("runtime"))
        .env("A3S_CODE_TUI_SMOKE", "1")
        .env("A3S_UPDATER_GITHUB_API_BASE", release.api_base())
        .env("PATH", "")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env_remove("A3S_OFFLINE")
        .env_remove("A3S_NO_AUTO_INSTALL")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .output()
        .expect("run Code first-use smoke");

    assert!(
        output.status.success(),
        "stdout={}\nstderr={}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let receipt_path = workspace.path("state/components/webview.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    assert_eq!(receipt["componentId"], "webview");
    assert_eq!(receipt["version"], WEBVIEW_VERSION);
    let executable = PathBuf::from(receipt["executablePath"].as_str().unwrap());
    assert!(executable.is_file(), "{}", executable.display());

    let requests = release.requests();
    assert!(
        requests
            .iter()
            .any(|path| path == "/repos/A3S-Lab/WebView/releases/latest"),
        "{requests:?}"
    );
    assert!(
        requests
            .iter()
            .any(|path| path == &format!("/assets/{WEBVIEW_ARCHIVE}")),
        "{requests:?}"
    );
}

fn start_fake_webview_release() -> FakeReleaseServer {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file("a3s-webview.exe", zip::write::SimpleFileOptions::default())
        .unwrap();
    writer.write_all(&fake_webview_pe()).unwrap();
    let archive = writer.finish().unwrap().into_inner();
    FakeReleaseServer::start("WebView", WEBVIEW_VERSION, WEBVIEW_ARCHIVE, archive)
}

fn fake_webview_pe() -> Vec<u8> {
    let mut binary = vec![0_u8; 0x80];
    binary[..2].copy_from_slice(b"MZ");
    binary[0x3c..0x40].copy_from_slice(&0x40_u32.to_le_bytes());
    binary[0x40..0x44].copy_from_slice(b"PE\0\0");
    binary[0x44..0x46].copy_from_slice(&0x8664_u16.to_le_bytes());
    binary.extend_from_slice(
        b"usage: a3s-webview --agent-island --snapshot <absolute-path> --lock-file <absolute-path>",
    );
    binary.extend_from_slice(b"a3s.system_agent_snapshot.v1");
    binary
}

fn install_ready_use_fixture(workspace: &TempWorkspace) {
    let install_root = workspace.path(&format!("data/components/use/{CLI_VERSION}"));
    let executable = install_root.join("a3s-use.exe");
    std::fs::create_dir_all(&install_root).unwrap();
    std::fs::copy(a3s_bin(), &executable).unwrap();
    let receipt_path = workspace.path("state/components/use.json");
    std::fs::create_dir_all(receipt_path.parent().unwrap()).unwrap();
    std::fs::write(
        receipt_path,
        serde_json::to_vec_pretty(&serde_json::json!({
            "schemaVersion": 1,
            "componentId": "use",
            "version": CLI_VERSION,
            "provenance": "github-release",
            "installRoot": install_root,
            "executablePath": executable,
            "ownedPaths": [install_root],
            "source": "fixture",
            "artifactChecksums": {},
            "installedAt": "2026-07-20T00:00:00Z"
        }))
        .unwrap(),
    )
    .unwrap();
}

fn write_config(path: &Path) {
    std::fs::write(
        path,
        r#"default_model = "openai/test"
providers "openai" {
  apiKey = "test"
  baseUrl = "http://127.0.0.1:1"
  models "test" {
    name = "Test"
    toolCall = true
  }
}
memory { llmExtraction = false }
"#,
    )
    .unwrap();
}
