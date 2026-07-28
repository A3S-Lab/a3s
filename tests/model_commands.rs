#![cfg(unix)]

mod support;

use std::process::{Command, Output, Stdio};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{io::Read, io::Write, net::TcpListener};

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

fn run_json(home: &std::path::Path, config: &std::path::Path, args: &[&str]) -> serde_json::Value {
    let mut json_args = vec!["--json"];
    json_args.extend_from_slice(args);
    let output = run(home, config, &json_args);
    assert!(
        output.status.success(),
        "a3s {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "a3s {args:?} returned invalid JSON: {error}: {}",
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn model_by_id<'a>(response: &'a serde_json::Value, id: &str) -> &'a serde_json::Value {
    response["data"]["models"]
        .as_array()
        .expect("model list data")
        .iter()
        .find(|model| model["id"] == id)
        .unwrap_or_else(|| panic!("model {id:?} missing from {response}"))
}

fn fixture() -> (TempWorkspace, std::path::PathBuf, std::path::PathBuf) {
    let workspace = TempWorkspace::new("model-commands");
    let home = workspace.path("home");
    let config = workspace.path("config.acl");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        &config,
        r#"
default_model = "openai/gpt-test"
providers "openai" {
  apiKey = "not-used"
  baseUrl = "https://example.invalid/v1"
  models "gpt-test" {
    name = "GPT Test"
    reasoning = true
    toolCall = true
    limit = { context = 32000, output = 1024 }
  }
}
"#,
    )
    .unwrap();
    (workspace, home, config)
}

#[test]
fn model_list_use_current_and_reset_share_one_selection() {
    let (_workspace, home, config) = fixture();

    let list = run_json(&home, &config, &["model", "list"]);
    let configured = model_by_id(&list, "openai/gpt-test");
    assert_eq!(configured["source"], "config.acl");
    assert_eq!(configured["contextWindow"], 32_000);

    let config_path = run(&home, &config, &["config", "path"]);
    assert_eq!(
        String::from_utf8_lossy(&config_path.stdout).trim(),
        config.display().to_string()
    );

    let selected = run_json(&home, &config, &["model", "use", "openai/gpt-test"]);
    assert_eq!(selected["data"]["model"], "openai/gpt-test");
    let acl = std::fs::read_to_string(&config).unwrap();
    assert!(acl.contains(r#"default_model = "openai/gpt-test""#));
    assert!(!home.join(".a3s/tui/model-selection.json").exists());

    let current = run_json(&home, &config, &["model", "current"]);
    assert_eq!(current["data"]["model"], "openai/gpt-test");
    assert_eq!(current["data"]["source"], "config.acl");

    let reset = run_json(&home, &config, &["model", "reset"]);
    assert_eq!(reset["data"]["previous"], "openai/gpt-test");
    assert!(!home.join(".a3s/tui/model-selection.json").exists());

    let current = run_json(&home, &config, &["model", "current"]);
    assert!(current["data"]["model"].is_null());
}

#[test]
fn model_use_rejects_routes_missing_from_the_catalog() {
    let (_workspace, home, config) = fixture();
    let before = std::fs::read_to_string(&config).unwrap();
    let output = run(&home, &config, &["model", "use", "codex/not-entitled"]);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("is not available"));
    assert_eq!(std::fs::read_to_string(&config).unwrap(), before);
}

#[test]
fn claude_code_login_models_are_selectable_without_copying_credentials() {
    let (_workspace, home, config) = fixture();
    std::fs::create_dir_all(home.join(".claude")).unwrap();
    std::fs::write(
        home.join(".claude/.credentials.json"),
        r#"{"claudeAiOauth":{"accessToken":"claude-secret"}}"#,
    )
    .unwrap();
    std::fs::write(
        home.join(".claude.json"),
        r#"{"projects":{"demo":{"model":"claude-opus-4-6"}}}"#,
    )
    .unwrap();

    let list = run_json(&home, &config, &["model", "list"]);
    let model = model_by_id(&list, "claude-code/claude-opus-4-6");
    assert_eq!(model["source"], "Claude Code");
    assert!(!list.to_string().contains("claude-secret"));

    let selected = run(
        &home,
        &config,
        &["model", "use", "claude-code/claude-opus-4-6"],
    );
    assert!(selected.status.success());
    let acl = std::fs::read_to_string(&config).unwrap();
    assert!(acl.contains(r#"default_model = "claude-code/claude-opus-4-6""#));
    assert!(!acl.contains("claude-secret"));
}

#[test]
fn codex_login_models_are_selectable_from_the_product_cache() {
    let (_workspace, home, config) = fixture();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::write(
        home.join(".codex/auth.json"),
        r#"{"tokens":{"id_token":"header.eyJleHAiOjF9.signature","access_token":"codex-secret","refresh_token":"codex-refresh-secret"}}"#,
    )
    .unwrap();
    std::fs::write(
        home.join(".codex/models_cache.json"),
        r#"{"models":[{"slug":"gpt-test-codex","visibility":"list","priority":1,"context_window":64000}]}"#,
    )
    .unwrap();

    let list = run_json(&home, &config, &["model", "list"]);
    let model = model_by_id(&list, "codex/gpt-test-codex");
    assert_eq!(model["source"], "Codex");
    assert_eq!(model["contextWindow"], 64_000);
    assert!(!list.to_string().contains("codex-secret"));

    let selected = run(&home, &config, &["model", "use", "codex/gpt-test-codex"]);
    assert!(selected.status.success());
    let acl = std::fs::read_to_string(&config).unwrap();
    assert!(acl.contains(r#"default_model = "codex/gpt-test-codex""#));
    assert!(!acl.contains("codex-secret"));
}

#[test]
fn kimi_desktop_models_are_selectable_without_copying_the_app_key() {
    let (_workspace, home, config) = fixture();
    let daimon = home.join(".config/kimi-desktop/daimon-share/daimon");
    std::fs::create_dir_all(&daimon).unwrap();
    std::fs::write(
        daimon.join("kimi-code-key.json"),
        r#"{"userId":"user-1","apiKey":"kimi-desktop-secret"}"#,
    )
    .unwrap();
    std::fs::write(
        daimon.join("config.json"),
        r#"{
  "model": {
    "current": "k3-agent",
    "providers": {
      "desktop-kimi": {
        "type": "kimi",
        "baseUrl": "https://example.invalid/coding/v1",
        "credential": "kimiCode"
      }
    },
    "models": {
      "k3-agent": {
        "provider": "desktop-kimi",
        "model": "k3-agent",
        "maxContextSize": 262144,
        "capabilities": ["thinking", "tool_use"]
      }
    }
  },
  "kimiCode": {
    "kimiRequestHeaders": {"User-Agent": "Desktop Kimi Work"}
  },
  "credentials": {"kimiCode": {"apiKey": "must-not-be-read-from-config"}}
}"#,
    )
    .unwrap();

    let list = run_json(&home, &config, &["model", "list"]);
    let model = model_by_id(&list, "kimi/k3-agent");
    assert_eq!(model["source"], "Kimi");
    assert_eq!(model["contextWindow"], 262_144);
    assert!(!list.to_string().contains("kimi-desktop-secret"));
    assert!(!list.to_string().contains("must-not-be-read-from-config"));

    let selected = run(&home, &config, &["model", "use", "kimi/k3-agent"]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    assert!(!String::from_utf8_lossy(&selected.stdout).contains("kimi-desktop-secret"));
    let acl = std::fs::read_to_string(&config).unwrap();
    assert!(acl.contains(r#"default_model = "kimi/k3-agent""#));
    assert!(!acl.contains("kimi-desktop-secret"));
    assert!(!acl.contains("must-not-be-read-from-config"));
}

#[test]
fn workbuddy_login_models_are_discovered_without_copying_account_state() {
    let (_workspace, home, config) = fixture();
    std::fs::create_dir_all(home.join(".workbuddy")).unwrap();
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(
        home.join(".workbuddy/settings.json"),
        r#"{"privateAccountState":"workbuddy-secret"}"#,
    )
    .unwrap();
    make_executable(
        &home.join("bin/codebuddy"),
        "#!/bin/sh\nprintf '%s\\n' 'Currently supported models for your account:' '  - glm-5.1' '  - kimi-k2.7'\n",
    );

    let list = run_json(&home, &config, &["model", "list"]);
    assert_eq!(
        model_by_id(&list, "workbuddy/glm-5.1")["source"],
        "WorkBuddy"
    );
    assert_eq!(
        model_by_id(&list, "workbuddy/kimi-k2.7")["source"],
        "WorkBuddy"
    );
    assert!(!list.to_string().contains("workbuddy-secret"));

    let selected = run(&home, &config, &["model", "use", "workbuddy/glm-5.1"]);
    assert!(
        selected.status.success(),
        "{}",
        String::from_utf8_lossy(&selected.stderr)
    );
    let acl = std::fs::read_to_string(&config).unwrap();
    assert!(acl.contains(r#"default_model = "workbuddy/glm-5.1""#));
    assert!(!acl.contains("workbuddy-secret"));
}

#[test]
fn a3s_os_gateway_models_are_selectable_without_storing_the_os_token() {
    let (_workspace, home, config) = fixture();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let server = std::thread::spawn(move || {
        for _ in 0..3 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let size = stream.read(&mut request).unwrap();
            let request = String::from_utf8_lossy(&request[..size]);
            assert!(request
                .to_ascii_lowercase()
                .contains("authorization: bearer os-secret"));
            let body = if request.starts_with("GET /api/v1/users/me ") {
                r#"{"data":{"displayName":"OS Test User"}}"#
            } else {
                assert!(request.starts_with("GET /api/v1/llm/models "));
                r#"{"data":[{"id":"gateway-model","context_length":128000}]}"#
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });
    let mut body = std::fs::read_to_string(&config).unwrap();
    body.push_str(&format!("\nos = \"http://{address}\"\n"));
    std::fs::write(&config, body).unwrap();

    let login = run_with_stdin(
        &home,
        &config,
        &["auth", "login", "os", "--token-stdin"],
        "os-secret",
    );
    assert!(login.status.success());
    let list = run_json(&home, &config, &["model", "list"]);
    let model = model_by_id(&list, "a3s-os/gateway-model");
    assert_eq!(model["source"], "A3S OS");
    assert_eq!(model["contextWindow"], 128_000);
    assert!(!list.to_string().contains("os-secret"));

    let selected = run(&home, &config, &["model", "use", "a3s-os/gateway-model"]);
    assert!(selected.status.success());
    let acl = std::fs::read_to_string(&config).unwrap();
    assert!(acl.contains(r#"default_model = "a3s-os/gateway-model""#));
    assert!(!acl.contains("os-secret"));
    server.join().unwrap();
}

#[test]
fn selecting_a_config_model_does_not_refresh_unrelated_accounts() {
    let (_workspace, home, config) = fixture();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(
        home.join(".codex/auth.json"),
        r#"{"tokens":{"access_token":"codex-secret"}}"#,
    )
    .unwrap();
    let probe = home.join("codex-probed");
    make_executable(
        &home.join("bin/codex"),
        &format!(
            "#!/bin/sh\nprintf probed > '{}'\n/bin/sleep 3\nprintf '%s\\n' '{{\"models\":[]}}'\n",
            probe.display()
        ),
    );

    let output = run(&home, &config, &["model", "use", "openai/gpt-test"]);
    assert!(output.status.success());
    assert!(!probe.exists(), "Codex was probed for a config-only route");
}

#[test]
fn model_list_discovers_slow_remote_sources_concurrently() {
    let (_workspace, home, config) = fixture();
    std::fs::create_dir_all(home.join(".codex")).unwrap();
    std::fs::create_dir_all(home.join("bin")).unwrap();
    std::fs::write(
        home.join(".codex/auth.json"),
        r#"{"tokens":{"access_token":"codex-secret"}}"#,
    )
    .unwrap();
    let codex_started = home.join("codex-started");
    let os_started = home.join("os-started");
    let codex_saw_os = home.join("codex-saw-os");
    make_executable(
        &home.join("bin/codex"),
        &format!(
            "#!/bin/sh\nprintf started > '{}'\n\
             attempts=0\n\
             while [ ! -e '{}' ] && [ \"$attempts\" -lt 100 ]; do\n\
               /bin/sleep 0.05\n\
               attempts=$((attempts + 1))\n\
             done\n\
             if [ -e '{}' ]; then printf observed > '{}'; fi\n\
             printf '%s\\n' '{{\"models\":[{{\"slug\":\"codex-slow\",\"visibility\":\"list\"}}]}}'\n",
            codex_started.display(),
            os_started.display(),
            os_started.display(),
            codex_saw_os.display(),
        ),
    );

    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let address = listener.local_addr().unwrap();
    let os_saw_codex = Arc::new(AtomicBool::new(false));
    let os_saw_codex_from_server = Arc::clone(&os_saw_codex);
    let codex_started_from_server = codex_started.clone();
    let os_started_from_server = os_started.clone();
    let server = std::thread::spawn(move || {
        for _ in 0..2 {
            let (mut stream, _) = listener.accept().unwrap();
            let mut request = [0_u8; 4096];
            let amount = stream.read(&mut request).unwrap();
            assert!(amount > 0, "expected an HTTP request");
            let request = String::from_utf8_lossy(&request[..amount]);
            let body = if request.starts_with("GET /api/v1/users/me ") {
                r#"{"data":{"displayName":"OS Slow User"}}"#
            } else {
                assert!(request.starts_with("GET /api/v1/llm/models "));
                std::fs::write(&os_started_from_server, "started").unwrap();
                for _ in 0..100 {
                    if codex_started_from_server.exists() {
                        os_saw_codex_from_server.store(true, Ordering::SeqCst);
                        break;
                    }
                    std::thread::sleep(std::time::Duration::from_millis(20));
                }
                r#"{"data":[{"id":"os-slow"}]}"#
            };
            write!(
                stream,
                "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });
    let mut body = std::fs::read_to_string(&config).unwrap();
    body.push_str(&format!("\nos = \"http://{address}\"\n"));
    std::fs::write(&config, body).unwrap();
    assert!(run_with_stdin(
        &home,
        &config,
        &["auth", "login", "os", "--token-stdin"],
        "os-secret",
    )
    .status
    .success());

    let output = run(&home, &config, &["model", "list"]);
    server.join().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("codex/codex-slow"));
    assert!(stdout.contains("a3s-os/os-slow"));
    assert!(
        codex_saw_os.exists() && os_saw_codex.load(Ordering::SeqCst),
        "remote discovery did not overlap both sources; stderr={}",
        String::from_utf8_lossy(&output.stderr)
    );
}
