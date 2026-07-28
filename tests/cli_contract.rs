#[cfg(unix)]
use std::io::{BufRead, Read};
use std::path::PathBuf;
use std::process::{Command, Stdio};
#[cfg(unix)]
use std::thread;

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

#[test]
fn root_help_exposes_only_the_canonical_taxonomy() {
    let output = Command::new(a3s_binary())
        .arg("--help")
        .output()
        .expect("run a3s --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    for command in [
        "code",
        "web",
        "top",
        "box",
        "compose",
        "up",
        "down",
        "ps",
        "logs",
        "bench",
        "search",
        "use",
        "auth",
        "model",
        "config",
        "list",
        "info",
        "install",
        "upgrade",
        "uninstall",
        "doctor",
        "registry",
        "cache",
        "self",
        "version",
        "completion",
        "help",
    ] {
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(command)),
            "missing canonical command {command:?} in:\n{stdout}"
        );
    }
    assert!(
        !stdout
            .lines()
            .any(|line| line.trim_start().starts_with("update ")),
        "legacy update must stay hidden from primary help:\n{stdout}"
    );
}

#[test]
fn command_path_help_matches_nested_generated_help() {
    let canonical = Command::new(a3s_binary())
        .args(["install", "--help"])
        .output()
        .expect("run nested help");
    let explicit = Command::new(a3s_binary())
        .args(["help", "install"])
        .output()
        .expect("run command-path help");

    assert!(canonical.status.success());
    assert!(explicit.status.success());
    assert_eq!(canonical.stdout, explicit.stdout);

    let unknown = Command::new(a3s_binary())
        .args(["help", "not-a-command"])
        .output()
        .expect("run unknown command-path help");
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unrecognized subcommand"));
}

#[test]
fn shell_completion_is_generated_from_the_canonical_tree() {
    let output = Command::new(a3s_binary())
        .args(["completion", "zsh"])
        .output()
        .expect("generate zsh completion");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("#compdef a3s"), "{stdout}");
    assert!(stdout.contains("'install:"), "{stdout}");
    assert!(stdout.contains("'registry:"), "{stdout}");
}

#[test]
fn unsupported_jsonl_is_a_structured_usage_error() {
    let output = Command::new(a3s_binary())
        .args(["--output", "jsonl", "list"])
        .output()
        .expect("run non-streaming command with JSONL");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSONL error");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["command"], "component.list");
    assert_eq!(value["type"], "error");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "usage.invalid");
}

#[test]
fn machine_mode_never_falls_through_to_help_or_interactive_code() {
    for (args, command) in [
        (vec!["--output", "json"], "a3s"),
        (vec!["--output", "json", "code"], "code"),
        (vec!["--output", "json", "code", "resume"], "code.resume"),
    ] {
        let output = Command::new(a3s_binary())
            .args(args)
            .output()
            .expect("reject interactive machine invocation");

        assert_eq!(output.status.code(), Some(2));
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("structured usage error");
        assert_eq!(value["command"], command);
        assert_eq!(value["error"]["code"], "usage.invalid");
    }
}

#[test]
fn compatibility_warnings_do_not_pollute_machine_streams() {
    let directory = tempfile::tempdir().expect("temp directory");
    let output = Command::new(a3s_binary())
        .env("HOME", directory.path())
        .args(["--output", "json", "code", "dirs"])
        .output()
        .expect("run a deprecated alias in JSON mode");

    assert!(output.status.success());
    assert!(
        output.stderr.is_empty(),
        "machine diagnostics must be structured"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON result");
    assert_eq!(value["command"], "config.paths");
    assert_eq!(value["ok"], true);
}

#[test]
fn top_uses_one_json_envelope_or_a_terminated_jsonl_stream() {
    let directory = tempfile::tempdir().expect("temp directory");
    let configure = |command: &mut Command| {
        command
            .env("HOME", directory.path())
            .env("A3S_TOP_CONNECTOR", "runc")
            .current_dir(directory.path());
    };

    let mut snapshot = Command::new(a3s_binary());
    configure(&mut snapshot);
    let snapshot = snapshot
        .args(["--output", "json", "top", "--view", "processes"])
        .output()
        .expect("collect a top JSON snapshot");
    assert!(
        snapshot.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&snapshot.stderr)
    );
    let snapshot: serde_json::Value =
        serde_json::from_slice(&snapshot.stdout).expect("top JSON envelope");
    assert_eq!(snapshot["schemaVersion"], 1);
    assert_eq!(snapshot["command"], "top");
    assert_eq!(snapshot["ok"], true);
    assert_eq!(snapshot["data"]["schema"], "a3s.top.snapshot.v1");

    let mut stream = Command::new(a3s_binary());
    configure(&mut stream);
    let stream = stream
        .args([
            "--output",
            "jsonl",
            "top",
            "--view",
            "processes",
            "--watch",
            "--interval",
            "1ms",
            "--count",
            "2",
        ])
        .output()
        .expect("collect a bounded top JSONL stream");
    assert!(
        stream.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stream.stderr)
    );
    let events = String::from_utf8(stream.stdout)
        .expect("top JSONL UTF-8")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("top JSONL event"))
        .collect::<Vec<_>>();
    assert_eq!(events.len(), 3, "{events:#?}");
    assert_eq!(events[0]["type"], "snapshot");
    assert_eq!(events[1]["type"], "snapshot");
    assert_eq!(events[2]["type"], "result");
    assert_eq!(events[2]["ok"], true);
    assert_eq!(events[2]["data"]["snapshots"], 2);
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["schemaVersion"], 1);
        assert_eq!(event["command"], "top");
        assert_eq!(event["sequence"], (index + 1) as u64);
    }
}

#[test]
fn top_rejects_stream_flags_in_single_document_json_mode() {
    let output = Command::new(a3s_binary())
        .args(["--output", "json", "top", "--watch", "--count", "1"])
        .output()
        .expect("reject top JSON streaming");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON error");
    assert_eq!(value["command"], "top");
    assert_eq!(value["error"]["code"], "usage.invalid");
}

#[cfg(unix)]
#[test]
fn top_watch_uses_exit_130_and_a_terminal_cancellation_event() {
    let directory = tempfile::tempdir().expect("temp directory");
    let mut child = Command::new(a3s_binary())
        .args([
            "--output",
            "jsonl",
            "top",
            "--view",
            "processes",
            "--watch",
            "--interval",
            "10s",
        ])
        .env("HOME", directory.path())
        .env("A3S_TOP_CONNECTOR", "runc")
        .current_dir(directory.path())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("start top JSONL watch");
    let stdout = child.stdout.take().expect("top stdout");
    let stderr = child.stderr.take().expect("top stderr");
    let stderr_reader = thread::spawn(move || {
        let mut output = String::new();
        std::io::BufReader::new(stderr)
            .read_to_string(&mut output)
            .expect("read top stderr");
        output
    });
    let mut stdout_reader = std::io::BufReader::new(stdout);
    let mut stdout = String::new();
    stdout_reader
        .read_line(&mut stdout)
        .expect("read initial top snapshot");
    assert!(!stdout.is_empty(), "top did not emit an initial snapshot");
    let signal = Command::new("kill")
        .args(["-INT", &child.id().to_string()])
        .status()
        .expect("signal top watch");
    assert!(signal.success());
    let status = child.wait().expect("wait for top watch");
    stdout_reader
        .read_to_string(&mut stdout)
        .expect("read terminal top event");
    let stderr = stderr_reader.join().expect("join top stderr reader");

    assert_eq!(
        status.code(),
        Some(130),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(stderr.is_empty(), "top JSONL wrote stderr: {stderr}");
    let events = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("top JSONL event"))
        .collect::<Vec<_>>();
    assert!(!events.is_empty(), "top stream was empty");
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence"], (index + 1) as u64, "{events:#?}");
        assert_eq!(event["command"], "top");
    }
    let terminal = events.last().expect("top terminal event");
    assert_eq!(terminal["type"], "error");
    assert_eq!(terminal["error"]["code"], "operation.cancelled");
}

#[test]
fn runtime_machine_errors_keep_the_command_envelope() {
    let directory = tempfile::tempdir().expect("temp directory");
    let missing = directory.path().join("missing.acl");
    let output = Command::new(a3s_binary())
        .arg("--config")
        .arg(&missing)
        .args(["--output", "json", "config", "validate"])
        .output()
        .expect("validate missing config");

    assert_eq!(output.status.code(), Some(1));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON error");
    assert_eq!(value["command"], "config.validate");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "operation.failed");
}

#[test]
fn version_is_available_as_a_command_and_a_flag() {
    let command = Command::new(a3s_binary())
        .arg("version")
        .output()
        .expect("run a3s version");
    let flag = Command::new(a3s_binary())
        .arg("--version")
        .output()
        .expect("run a3s --version");

    assert!(command.status.success());
    assert!(flag.status.success());
    assert_eq!(command.stdout, flag.stdout);
    assert!(String::from_utf8_lossy(&command.stdout).starts_with("a3s "));
}

#[test]
fn version_machine_output_is_one_canonical_document() {
    for verbosity in [false, true] {
        let mut command = Command::new(a3s_binary());
        command.args(["--output", "json"]);
        if verbosity {
            command.arg("--verbose");
        }
        let output = command.arg("version").output().expect("run JSON version");

        assert!(output.status.success());
        assert!(output.stderr.is_empty());
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("one version JSON document");
        assert_eq!(value["command"], "version");
        assert_eq!(value["ok"], true);
        assert_eq!(value["data"]["version"], env!("CARGO_PKG_VERSION"));
        assert_eq!(value["data"]["verbose"], verbosity);
        assert!(value["data"]["target"]["os"].is_string());
        assert!(value["data"]["target"]["arch"].is_string());
    }
}

#[test]
fn canonical_self_and_upgrade_commands_have_generated_help() {
    for args in [["self", "update", "--help"], ["upgrade", "--help", ""]] {
        let args = args.into_iter().filter(|arg| !arg.is_empty());
        let output = Command::new(a3s_binary())
            .args(args)
            .output()
            .expect("run command help");
        assert!(
            output.status.success(),
            "stderr: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("Usage:"));
    }
}

#[test]
fn self_update_rejects_jsonl_before_network_access() {
    let output = Command::new(a3s_binary())
        .args([
            "--offline",
            "--output",
            "jsonl",
            "self",
            "update",
            "--check",
        ])
        .output()
        .expect("reject self-update JSONL");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSONL error");
    assert_eq!(value["command"], "self.update");
    assert_eq!(value["type"], "error");
    assert_eq!(value["error"]["code"], "usage.invalid");
}

#[test]
fn unknown_root_commands_are_usage_errors() {
    let output = Command::new(a3s_binary())
        .arg("definitely-not-an-a3s-command")
        .output()
        .expect("run unknown command");

    assert_eq!(output.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&output.stderr).contains("unrecognized subcommand"));
}

#[test]
fn parser_errors_honor_machine_output_without_echoing_arguments() {
    for mode in ["json", "jsonl"] {
        let secret_like_argument = "secret-like-argument-that-must-not-be-echoed";
        let output = Command::new(a3s_binary())
            .args([
                "--output",
                mode,
                "definitely-not-an-a3s-command",
                secret_like_argument,
            ])
            .output()
            .expect("run invalid machine invocation");

        assert_eq!(output.status.code(), Some(2));
        assert!(
            output.stderr.is_empty(),
            "stderr must stay empty in {mode} mode"
        );
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("structured parser error");
        assert_eq!(value["schemaVersion"], 1);
        assert_eq!(value["command"], "a3s");
        assert_eq!(value["ok"], false);
        assert_eq!(value["error"]["code"], "usage.invalid");
        assert!(
            !String::from_utf8_lossy(&output.stdout).contains(secret_like_argument),
            "machine parser errors must not echo raw arguments"
        );
        if mode == "jsonl" {
            assert_eq!(value["type"], "error");
            assert_eq!(value["sequence"], 1);
        }
    }
}

#[test]
fn missing_group_verbs_are_structured_in_machine_mode() {
    let output = Command::new(a3s_binary())
        .args(["--output", "json", "auth"])
        .output()
        .expect("run a command group without a verb");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON error");
    assert_eq!(value["command"], "a3s");
    assert_eq!(value["error"]["code"], "usage.invalid");
}

#[test]
fn global_context_failures_use_the_selected_command_envelope() {
    let directory = tempfile::tempdir().expect("temp directory");
    let missing = directory.path().join("missing-workspace");
    let output = Command::new(a3s_binary())
        .args(["--output", "json", "--directory"])
        .arg(&missing)
        .arg("version")
        .output()
        .expect("run with a missing workspace");

    assert_eq!(output.status.code(), Some(1));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON error");
    assert_eq!(value["command"], "version");
    assert_eq!(value["error"]["code"], "operation.failed");
}

#[test]
fn code_has_a_typed_canonical_tree_and_rejects_prompt_guessing() {
    let help = Command::new(a3s_binary())
        .args(["code", "--help"])
        .output()
        .expect("run code help");
    assert!(help.status.success());
    let stdout = String::from_utf8_lossy(&help.stdout);
    for command in [
        "exec", "resume", "research", "session", "agent", "mcp", "skill", "flow", "okf", "kb",
        "context", "memory",
    ] {
        assert!(
            stdout
                .lines()
                .any(|line| line.trim_start().starts_with(command)),
            "missing code command {command:?}:\n{stdout}"
        );
    }
    assert!(!stdout.contains("deepresearch"), "{stdout}");

    let unknown = Command::new(a3s_binary())
        .args(["code", "this-used-to-be-guessed-as-a-prompt"])
        .output()
        .expect("run unknown Code word");
    assert_eq!(unknown.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&unknown.stderr).contains("unrecognized subcommand"));
}

#[test]
fn research_rejects_removed_runtime_selection() {
    let directory = tempfile::tempdir().expect("temp directory");
    let config = directory.path().join("config.acl");
    std::fs::write(&config, test_config()).expect("write config");

    let output = Command::new(a3s_binary())
        .arg("--config")
        .arg(&config)
        .args([
            "--output",
            "json",
            "code",
            "research",
            "runtime policy",
            "--runtime",
            "os",
        ])
        .output()
        .expect("run removed research runtime option");

    assert!(!output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["command"], "a3s");
    assert_eq!(value["ok"], false);
    assert_eq!(value["error"]["code"], "usage.invalid");
    assert_eq!(value["error"]["details"]["kind"], "UnknownArgument");
}

#[test]
fn research_help_exposes_explicit_evidence_scope_controls() {
    let help = Command::new(a3s_binary())
        .args(["code", "research", "--help"])
        .output()
        .expect("run research help");
    assert!(help.status.success());
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("--local-only"), "{stdout}");
    assert!(stdout.contains("--web"), "{stdout}");
    assert!(!stdout.contains("--runtime"), "{stdout}");

    let conflict = Command::new(a3s_binary())
        .args([
            "--output",
            "json",
            "code",
            "research",
            "--local-only",
            "--web",
            "conflicting scope",
        ])
        .output()
        .expect("reject conflicting research evidence scopes");
    assert_eq!(conflict.status.code(), Some(2));
    let value: serde_json::Value =
        serde_json::from_slice(&conflict.stdout).expect("structured scope conflict");
    assert_eq!(value["command"], "a3s");
    assert_eq!(value["error"]["code"], "usage.invalid");

    let offline_conflict = Command::new(a3s_binary())
        .args([
            "--output",
            "json",
            "--offline",
            "code",
            "research",
            "--web",
            "conflicting network policy",
        ])
        .output()
        .expect("reject web research under the global offline policy");
    assert_eq!(offline_conflict.status.code(), Some(2));
    let value: serde_json::Value =
        serde_json::from_slice(&offline_conflict.stdout).expect("structured offline conflict");
    assert_eq!(value["command"], "code.research");
    assert_eq!(value["error"]["code"], "usage.invalid");
}

#[test]
fn canonical_asset_discovery_requires_an_explicit_location() {
    let missing = Command::new(a3s_binary())
        .args(["code", "skill", "list"])
        .output()
        .expect("run ambiguous asset list");
    assert_eq!(missing.status.code(), Some(2));
    assert!(String::from_utf8_lossy(&missing.stderr).contains("--location"));

    let help = Command::new(a3s_binary())
        .args(["code", "skill", "list", "--help"])
        .output()
        .expect("run asset list help");
    let stdout = String::from_utf8_lossy(&help.stdout);
    assert!(stdout.contains("--location <LOCATION>"), "{stdout}");
    assert!(stdout.contains("local"), "{stdout}");
    assert!(stdout.contains("os"), "{stdout}");
    assert!(stdout.contains("all"), "{stdout}");
}

#[test]
fn session_list_uses_the_common_machine_envelope() {
    let directory = tempfile::tempdir().expect("temp directory");
    let sessions = directory.path().join(".a3s/tui-sessions");
    std::fs::create_dir_all(&sessions).expect("session directory");
    std::fs::write(
        sessions.join("abc-123.json"),
        r#"{
            "id": "abc-123",
            "config": {
                "name": "Test Session",
                "workspace": ".",
                "system_prompt": null,
                "max_context_length": 200000,
                "auto_compact": false
            },
            "state": "Active",
            "messages": [],
            "context_usage": {
                "used_tokens": 0,
                "max_tokens": 200000,
                "percent": 0.0,
                "turns": 0
            },
            "total_usage": {
                "prompt_tokens": 0,
                "completion_tokens": 0,
                "total_tokens": 0,
                "cache_read_tokens": null,
                "cache_write_tokens": null
            },
            "tool_names": [],
            "thinking_enabled": false,
            "thinking_budget": null,
            "created_at": 1700000000,
            "updated_at": 1700000100
        }"#,
    )
    .expect("session fixture");

    let output = Command::new(a3s_binary())
        .arg("--directory")
        .arg(directory.path())
        .args(["--output", "json", "code", "session", "list"])
        .output()
        .expect("list sessions");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("session JSON");
    assert_eq!(value["command"], "code.session.list");
    assert_eq!(value["data"]["sessions"][0]["id"], "abc-123");
}

#[test]
fn positional_auth_tokens_are_rejected_without_echoing_the_secret() {
    let secret = "super-secret-token-value";
    let output = Command::new(a3s_binary())
        .args(["auth", "login", "os", secret])
        .output()
        .expect("run unsafe auth form");

    assert!(!output.status.success());
    let combined = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        !combined.contains(secret),
        "secret leaked in output: {combined}"
    );
    assert!(combined.contains("--token-stdin"), "{combined}");
}

#[test]
fn config_show_is_canonical_structured_and_redacted() {
    let directory = tempfile::tempdir().expect("temp directory");
    let config = directory.path().join("config.acl");
    std::fs::write(&config, test_config()).expect("write config");

    let output = Command::new(a3s_binary())
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "config", "show"])
        .output()
        .expect("run config show");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON output");
    assert_eq!(value["schemaVersion"], 1);
    assert_eq!(value["command"], "config.show");
    assert_eq!(value["ok"], true);
    assert_eq!(value["data"]["defaultModel"], "openai/model-a");
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(!rendered.contains("top-secret-api-key"), "{rendered}");
}

#[test]
fn model_current_and_use_share_the_acl_config_editor() {
    let directory = tempfile::tempdir().expect("temp directory");
    let config = directory.path().join("config.acl");
    std::fs::write(&config, test_config()).expect("write config");

    let current = Command::new(a3s_binary())
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "model", "current"])
        .output()
        .expect("run model current");
    assert!(current.status.success());
    let value: serde_json::Value =
        serde_json::from_slice(&current.stdout).expect("model current JSON");
    assert_eq!(value["data"]["model"], "openai/model-a");

    let select = Command::new(a3s_binary())
        .arg("--config")
        .arg(&config)
        .args(["model", "use", "openai/model-b", "--scope", "user"])
        .output()
        .expect("run model use");
    assert!(
        select.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&select.stderr)
    );
    let persisted = std::fs::read_to_string(&config).expect("updated config");
    assert!(persisted.contains("# preserve-this-comment"));
    assert!(persisted.contains("default_model = \"openai/model-b\""));
    assert!(!persisted.contains("default_model = \"openai/model-a\""));
}

#[test]
fn effective_acl_resolution_merges_user_and_workspace_with_provenance() {
    let directory = tempfile::tempdir().expect("temp directory");
    let home = directory.path().join("home");
    let workspace = directory.path().join("workspace");
    let user_config = home.join(".a3s/config.acl");
    let workspace_config = workspace.join(".a3s/config.acl");
    std::fs::create_dir_all(user_config.parent().unwrap()).unwrap();
    std::fs::create_dir_all(workspace_config.parent().unwrap()).unwrap();
    std::fs::write(
        &user_config,
        r#"default_model = "openai/base"
providers "openai" {
  apiKey = "user-secret"
  models "base" { name = "Base" }
}
"#,
    )
    .unwrap();
    std::fs::write(
        &workspace_config,
        r#"default_model = "openai/workspace"
providers "openai" {
  baseUrl = "https://workspace.example/v1"
  models "workspace" { name = "Workspace" }
}
"#,
    )
    .unwrap();
    let resolved_workspace_config = workspace_config.canonicalize().unwrap();

    let output = Command::new(a3s_binary())
        .env("HOME", &home)
        .env_remove("A3S_CONFIG_FILE")
        .arg("--directory")
        .arg(&workspace)
        .args(["--output", "json", "config", "show"])
        .output()
        .expect("show effective config");
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["defaultModel"], "openai/workspace");
    assert_eq!(value["data"]["providers"], serde_json::json!(["openai"]));
    assert_eq!(value["data"]["models"].as_array().unwrap().len(), 2);
    assert_eq!(value["data"]["layers"].as_array().unwrap().len(), 2);
    assert_eq!(
        value["data"]["provenance"]["default_model"],
        resolved_workspace_config.display().to_string()
    );
    assert_eq!(
        value["data"]["provenance"]["providers.openai.api_key"],
        user_config.display().to_string()
    );
    assert!(!String::from_utf8_lossy(&output.stdout).contains("user-secret"));

    let current = Command::new(a3s_binary())
        .env("HOME", &home)
        .env_remove("A3S_CONFIG_FILE")
        .arg("--directory")
        .arg(&workspace)
        .args(["--output", "json", "model", "current"])
        .output()
        .expect("show effective model");
    assert!(current.status.success());
    let value: serde_json::Value = serde_json::from_slice(&current.stdout).unwrap();
    assert_eq!(value["data"]["model"], "openai/workspace");
    assert_eq!(
        value["data"]["configSource"],
        resolved_workspace_config.display().to_string()
    );
}

#[test]
fn explicit_acl_and_typed_environment_override_normal_resolution() {
    let directory = tempfile::tempdir().expect("temp directory");
    let home = directory.path().join("home");
    let workspace = directory.path().join("workspace");
    let explicit = directory.path().join("explicit.acl");
    std::fs::create_dir_all(home.join(".a3s")).unwrap();
    std::fs::create_dir_all(workspace.join(".a3s")).unwrap();
    std::fs::write(
        home.join(".a3s/config.acl"),
        "default_model = \"openai/user\"\n",
    )
    .unwrap();
    std::fs::write(
        workspace.join(".a3s/config.acl"),
        "default_model = \"openai/workspace\"\n",
    )
    .unwrap();
    std::fs::write(
        &explicit,
        r#"default_model = "openai/explicit"
providers "openai" {
  models "explicit" { name = "Explicit" }
  models "environment" { name = "Environment" }
}
"#,
    )
    .unwrap();

    let output = Command::new(a3s_binary())
        .env("HOME", &home)
        .env("A3S_DEFAULT_MODEL", "openai/environment")
        .arg("--directory")
        .arg(&workspace)
        .arg("--config")
        .arg(&explicit)
        .args(["--output", "json", "config", "show"])
        .output()
        .expect("show explicit effective config");
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["data"]["explicit"], true);
    assert_eq!(value["data"]["layers"].as_array().unwrap().len(), 1);
    assert_eq!(value["data"]["defaultModel"], "openai/environment");
    assert_eq!(
        value["data"]["provenance"]["default_model"],
        "environment:A3S_DEFAULT_MODEL"
    );
}

#[test]
fn relative_explicit_acl_resolves_from_the_effective_directory() {
    let directory = tempfile::tempdir().expect("temp directory");
    let launch_directory = directory.path().join("launch");
    let workspace = directory.path().join("workspace");
    std::fs::create_dir_all(&launch_directory).expect("launch directory");
    std::fs::create_dir_all(&workspace).expect("workspace directory");
    std::fs::write(workspace.join("config.acl"), test_config()).expect("write relative config");
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");

    let output = Command::new(a3s_binary())
        .current_dir(&launch_directory)
        .env("HOME", directory.path().join("home"))
        .env_remove("A3S_CONFIG_FILE")
        .arg("-C")
        .arg(&workspace)
        .args([
            "--config",
            "config.acl",
            "--output",
            "json",
            "config",
            "show",
        ])
        .output()
        .expect("resolve a relative explicit config");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("config JSON");
    assert_eq!(value["data"]["explicit"], true);
    assert_eq!(value["data"]["defaultModel"], "openai/model-a");
    assert_eq!(
        value["data"]["path"],
        canonical_workspace.join("config.acl").display().to_string()
    );
}

#[test]
fn relative_platform_roots_resolve_from_the_effective_directory() {
    let directory = tempfile::tempdir().expect("temp directory");
    let launch_directory = directory.path().join("launch");
    let workspace = directory.path().join("workspace");
    std::fs::create_dir_all(&launch_directory).expect("launch directory");
    std::fs::create_dir_all(&workspace).expect("workspace directory");
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");

    let output = Command::new(a3s_binary())
        .current_dir(&launch_directory)
        .env("HOME", directory.path().join("home"))
        .env("A3S_DATA_HOME", "relative-data")
        .env("A3S_STATE_HOME", "relative-state")
        .env("A3S_CACHE_HOME", "relative-cache")
        .env_remove("A3S_CONFIG_FILE")
        .arg("-C")
        .arg(&workspace)
        .args(["--output", "json", "config", "paths"])
        .output()
        .expect("resolve relative platform roots");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("paths JSON");
    assert_eq!(
        value["data"]["data"],
        canonical_workspace
            .join("relative-data")
            .display()
            .to_string()
    );
    assert_eq!(
        value["data"]["state"],
        canonical_workspace
            .join("relative-state")
            .display()
            .to_string()
    );
    assert_eq!(
        value["data"]["cache"],
        canonical_workspace
            .join("relative-cache")
            .display()
            .to_string()
    );
}

fn test_config() -> &'static str {
    r#"# preserve-this-comment
default_model = "openai/model-a"
os = "http://127.0.0.1:9"

providers "openai" {
  apiKey = "top-secret-api-key"
  baseUrl = "https://example.com/v1"

  models "model-a" { name = "Model A" }
  models "model-b" { name = "Model B" }
}
"#
}
