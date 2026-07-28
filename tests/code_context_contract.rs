use std::path::PathBuf;
use std::process::Command;

#[cfg(unix)]
mod support;

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

#[test]
fn knowledge_and_memory_commands_use_typed_json_and_the_effective_directory() {
    let directory = tempfile::tempdir().expect("temp directory");
    let launch_directory = directory.path().join("launch");
    let workspace = directory.path().join("workspace");
    let memory = workspace.join("relative-memory");
    std::fs::create_dir_all(&launch_directory).expect("launch directory");
    std::fs::create_dir_all(memory.join("items")).expect("memory directory");
    std::fs::write(
        memory.join("index.json"),
        r#"[{"id":"mem-1","content_lower":"needle decision","tags":["architecture"],"importance":0.8,"timestamp":"2026-07-15T00:00:00Z","memory_type":"semantic"}]"#,
    )
    .expect("memory index");
    std::fs::write(
        memory.join("items/mem-1.json"),
        r#"{"content":"Needle decision from the effective workspace","metadata":{"source":"test"}}"#,
    )
    .expect("memory item");
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");

    let run = |args: &[&str]| {
        Command::new(a3s_binary())
            .current_dir(&launch_directory)
            .env("HOME", directory.path().join("home"))
            .env("A3S_MEMORY_DIR", "relative-memory")
            .env_remove("A3S_CONFIG_FILE")
            .arg("-C")
            .arg(&workspace)
            .args(["--output", "json"])
            .args(args)
            .output()
            .expect("run typed Code knowledge command")
    };

    let kb_path = run(&["code", "kb", "path"]);
    assert!(
        kb_path.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&kb_path.stderr)
    );
    let kb_path: serde_json::Value = serde_json::from_slice(&kb_path.stdout).expect("KB JSON");
    assert_eq!(kb_path["command"], "code.kb.path");
    assert_eq!(
        kb_path["data"]["path"],
        canonical_workspace.join(".a3s/kb").display().to_string()
    );

    let add = run(&["code", "kb", "add", "typed knowledge note"]);
    assert!(
        add.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&add.stderr)
    );
    let add: serde_json::Value = serde_json::from_slice(&add.stdout).expect("KB add JSON");
    assert_eq!(add["command"], "code.kb.add");
    assert_eq!(add["data"]["created"], true);

    let search = run(&["code", "kb", "search", "typed knowledge"]);
    assert!(search.status.success());
    let search: serde_json::Value = serde_json::from_slice(&search.stdout).expect("KB search JSON");
    assert_eq!(search["command"], "code.kb.search");
    assert_eq!(search["data"]["hits"].as_array().unwrap().len(), 1);

    let memories = run(&["code", "memory", "list", "needle"]);
    assert!(
        memories.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&memories.stderr)
    );
    let memories: serde_json::Value =
        serde_json::from_slice(&memories.stdout).expect("memory JSON");
    assert_eq!(memories["command"], "code.memory.list");
    assert_eq!(
        memories["data"]["path"],
        canonical_workspace
            .join("relative-memory")
            .display()
            .to_string()
    );
    assert_eq!(memories["data"]["entries"][0]["id"], "mem-1");
    assert_eq!(
        memories["data"]["entries"][0]["content"],
        "Needle decision from the effective workspace"
    );
}

#[cfg(unix)]
#[test]
fn context_history_uses_typed_json_and_the_effective_child_directory() {
    let directory = tempfile::tempdir().expect("temp directory");
    let launch_directory = directory.path().join("launch");
    let workspace = directory.path().join("workspace");
    let bin = directory.path().join("bin");
    let cwd_log = directory.path().join("ctx-cwd.log");
    std::fs::create_dir_all(&launch_directory).expect("launch directory");
    std::fs::create_dir_all(&workspace).expect("workspace directory");
    support::make_executable(
        &bin.join("ctx"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$PWD\" > {}\nif [ \"$1\" = \"search\" ]; then printf '%s\\n' '{{\"results\":[{{\"ctx_event_id\":\"ev-1\",\"ctx_session_id\":\"ses-1\",\"provider\":\"codex\",\"timestamp\":\"2026-07-15T01:02:03Z\",\"title\":\"Migration decision\",\"snippet\":\"Use explicit context\"}}]}}'; exit 0; fi\nprintf 'history for %s\\n' \"$*\"\n",
            support::sh_quote(&cwd_log)
        ),
    );
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");

    let output = Command::new(a3s_binary())
        .current_dir(&launch_directory)
        .env("HOME", directory.path().join("home"))
        .env("PATH", &bin)
        .arg("-C")
        .arg(&workspace)
        .args([
            "--output",
            "json",
            "code",
            "context",
            "search",
            "invocation context",
        ])
        .output()
        .expect("search context history");

    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("context JSON");
    assert_eq!(value["command"], "code.context.search");
    assert_eq!(value["data"]["hits"][0]["eventId"], "ev-1");
    assert_eq!(
        std::fs::read_to_string(cwd_log).unwrap().trim(),
        canonical_workspace.display().to_string()
    );
}
