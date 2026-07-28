use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde_json::{json, Value};

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

fn session_document(id: &str) -> Value {
    session_document_at(id, 1_700_000_100)
}

fn session_document_at(id: &str, updated_at: i64) -> Value {
    json!({
        "id": id,
        "config": {
            "name": "Test Session",
            "workspace": ".",
            "system_prompt": null,
            "max_context_length": 200_000,
            "auto_compact": false,
        },
        "state": "Active",
        "messages": [],
        "context_usage": {
            "used_tokens": 0,
            "max_tokens": 200_000,
            "percent": 0.0,
            "turns": 0,
        },
        "total_usage": {
            "prompt_tokens": 0,
            "completion_tokens": 0,
            "total_tokens": 0,
            "cache_read_tokens": null,
            "cache_write_tokens": null,
        },
        "tool_names": [],
        "thinking_enabled": false,
        "thinking_budget": null,
        "created_at": 1_700_000_000_i64,
        "updated_at": updated_at,
    })
}

fn core_session_path(workspace: &Path, id: &str) -> PathBuf {
    let key = URL_SAFE_NO_PAD.encode(id.as_bytes());
    workspace
        .join(".a3s/tui/sessions/v1/sessions")
        .join(format!("id_{key}.json"))
}

fn sidecar_path(workspace: &Path, id: &str) -> PathBuf {
    let key = URL_SAFE_NO_PAD.encode(id.as_bytes());
    workspace
        .join(".a3s/tui/session-state/v1")
        .join(format!("id_{key}.json"))
}

fn write_session(workspace: &Path, id: &str) {
    write_session_at(workspace, id, 1_700_000_100);
}

fn write_session_at(workspace: &Path, id: &str, updated_at: i64) {
    let path = core_session_path(workspace, id);
    std::fs::create_dir_all(path.parent().expect("session parent")).expect("create session parent");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&session_document_at(id, updated_at)).expect("serialize session"),
    )
    .expect("write session");
}

fn run_json(workspace: &Path, args: &[&str]) -> Output {
    Command::new(a3s_binary())
        .arg("--directory")
        .arg(workspace)
        .args(["--output", "json", "code", "session"])
        .args(args)
        .output()
        .expect("run session command")
}

fn successful_json(output: &Output) -> Value {
    assert!(
        output.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("command JSON")
}

#[test]
fn session_commands_use_the_core_v1_store_and_delete_the_tui_sidecar() {
    let directory = tempfile::tempdir().expect("temp directory");
    let id = "abc-123";
    write_session(directory.path(), id);

    let sidecar = sidecar_path(directory.path(), id);
    std::fs::create_dir_all(sidecar.parent().expect("sidecar parent"))
        .expect("create sidecar parent");
    std::fs::write(&sidecar, r#"{"schema_version":1}"#).expect("write sidecar");

    let listed = successful_json(&run_json(directory.path(), &["list"]));
    assert_eq!(listed["command"], "code.session.list");
    assert_eq!(listed["data"]["sessions"][0]["id"], id);
    assert!(listed["data"]["sessions"][0]["bytes"]
        .as_u64()
        .is_some_and(|bytes| bytes > 0));

    let shown = successful_json(&run_json(directory.path(), &["show", id]));
    assert_eq!(shown["command"], "code.session.show");
    assert_eq!(shown["data"]["document"]["id"], id);
    assert_eq!(
        std::fs::canonicalize(PathBuf::from(
            shown["data"]["path"].as_str().expect("session path")
        ))
        .expect("canonical shown path"),
        std::fs::canonicalize(core_session_path(directory.path(), id))
            .expect("canonical expected path")
    );

    let exported = successful_json(&run_json(directory.path(), &["export", id]));
    assert_eq!(exported["command"], "code.session.export");
    assert_eq!(exported["data"]["document"]["id"], id);

    let export_file = directory.path().join("exports/session.json");
    let exported_to_file = successful_json(&run_json(
        directory.path(),
        &[
            "export",
            id,
            "--output-file",
            export_file.to_str().expect("UTF-8 export path"),
        ],
    ));
    assert_eq!(exported_to_file["data"]["id"], id);
    let exported_document: Value =
        serde_json::from_slice(&std::fs::read(&export_file).expect("read export"))
            .expect("parse export");
    assert_eq!(exported_document["id"], id);

    let deleted = successful_json(&run_json(directory.path(), &["delete", id, "--yes"]));
    assert_eq!(deleted["command"], "code.session.delete");
    assert_eq!(deleted["data"], json!({"id": id, "deleted": true}));
    assert!(!core_session_path(directory.path(), id).exists());
    assert!(!sidecar.exists());
}

#[test]
fn session_list_orders_by_core_updated_at_instead_of_file_mtime() {
    let directory = tempfile::tempdir().expect("temp directory");
    write_session_at(directory.path(), "z-newer-session", 200);
    // Write the logically older session last so its filesystem timestamp cannot
    // accidentally become the ordering source again. The IDs are deliberately
    // reverse-alphabetical so an equal-mtime fallback would also fail the test.
    write_session_at(directory.path(), "a-older-session", 100);

    let listed = successful_json(&run_json(directory.path(), &["list"]));
    assert_eq!(listed["data"]["sessions"][0]["id"], "z-newer-session");
    assert_eq!(listed["data"]["sessions"][1]["id"], "a-older-session");
}

#[test]
fn session_list_migrates_the_legacy_store_like_tui_launch() {
    let directory = tempfile::tempdir().expect("temp directory");
    let legacy_root = directory.path().join(".a3s/tui-sessions");
    let id = "legacy-123";
    let key = URL_SAFE_NO_PAD.encode(id.as_bytes());
    let legacy_session = legacy_root
        .join("v1/sessions")
        .join(format!("id_{key}.json"));
    std::fs::create_dir_all(legacy_session.parent().expect("legacy session parent"))
        .expect("create legacy session parent");
    std::fs::write(
        &legacy_session,
        serde_json::to_vec_pretty(&session_document(id)).expect("serialize session"),
    )
    .expect("write legacy session");

    let listed = successful_json(&run_json(directory.path(), &["list"]));
    assert_eq!(listed["data"]["sessions"][0]["id"], id);
    assert!(!legacy_root.exists());
    assert!(core_session_path(directory.path(), id).is_file());
}
