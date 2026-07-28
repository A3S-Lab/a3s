#![cfg(windows)]

use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

#[test]
fn code_smoke_reaches_the_session_on_windows() {
    let temp = tempfile::tempdir().expect("create Code startup test directory");
    let workspace = temp.path().join("workspace");
    let home = temp.path().join("home");
    let config = temp.path().join("config.acl");
    fs::create_dir_all(&workspace).expect("create Code startup workspace");
    fs::create_dir_all(&home).expect("create Code startup home");
    fs::write(
        &config,
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
    .expect("write Code startup config");

    let output = Command::new(a3s_binary())
        .arg("-C")
        .arg(&workspace)
        .arg("--config")
        .arg(&config)
        .arg("code")
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("A3S_CODE_TUI_SMOKE", "1")
        .env("A3S_NO_AUTO_INSTALL", "1")
        .env("A3S_OFFLINE", "1")
        .env("NO_PROXY", "127.0.0.1,localhost")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .output()
        .expect("run Code startup smoke test");

    assert!(
        output.status.success(),
        "Code startup failed:\nstdout: {}\nstderr: {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stdout.contains("[smoke] prompt:") || stderr.contains("[smoke] prompt:"),
        "Code did not reach the smoke session:\nstdout: {}\nstderr: {}",
        stdout,
        stderr
    );
}
