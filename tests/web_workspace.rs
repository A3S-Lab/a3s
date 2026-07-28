use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

#[test]
fn deep_research_report_route_serves_only_session_workspace_artifacts_with_a_sandbox_csp() {
    let root = tempfile::tempdir().expect("temporary DeepResearch report fixture");
    let workspace = root.path().join("workspace");
    let report_dir = workspace.join(".a3s/research/artifacts/report-fixture");
    let web_dir = root.path().join("web");
    let state_dir = root.path().join("state");
    let config_path = root.path().join("config.acl");
    fs::create_dir_all(&report_dir).expect("create report directory");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        report_dir.join("index.html"),
        "<!doctype html><title>DeepResearch fixture</title><h1>Source-backed report</h1>",
    )
    .expect("write report fixture");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S DeepResearch route test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut daemon, address) = start_detached_web(&workspace, &config_path, &web_dir, &state_dir);

    let session = http_json(
        &address,
        "POST",
        "/api/v1/kernel/sessions",
        Some(&serde_json::json!({ "workspace": workspace }).to_string()),
        "200",
    );
    let session_id = session["session"]["sessionId"]
        .as_str()
        .expect("created session id");
    let response = http_request(
        &address,
        "GET",
        &format!(
            "/api/v1/kernel/sessions/{session_id}/research-artifact?runId=report-fixture&kind=html"
        ),
        None,
    );
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    assert!(
        response
            .to_ascii_lowercase()
            .contains("content-security-policy: sandbox allow-popups"),
        "{response}"
    );
    assert!(response.contains("<h1>Source-backed report</h1>"));

    let rejected = http_json(
        &address,
        "GET",
        &format!(
            "/api/v1/kernel/sessions/{session_id}/research-artifact?runId=..%2Fsecret&kind=html"
        ),
        None,
        "400",
    );
    assert!(rejected["message"]
        .as_str()
        .is_some_and(|message| message.contains("run ID")));

    daemon.stop();
    wait_until_stopped(&address);
}

#[test]
fn workspace_directory_picker_accepts_a_client_selected_directory() {
    let root = tempfile::tempdir().expect("temporary workspace picker fixture");
    let workspace = root.path().join("workspace");
    let selected = root.path().join("selected-workspace");
    let web_dir = root.path().join("web");
    let state_dir = root.path().join("state");
    let config_path = root.path().join("config.acl");
    fs::create_dir_all(&workspace).expect("create default workspace");
    fs::create_dir_all(&selected).expect("create selected workspace");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S workspace picker test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut daemon, address) = start_detached_web(&workspace, &config_path, &web_dir, &state_dir);

    let request = serde_json::json!({ "path": selected }).to_string();
    let response = http_json(
        &address,
        "POST",
        "/api/v1/workspace/actions/pick-directory",
        Some(&request),
        "200",
    );
    assert_eq!(response["cancelled"], false);
    assert_eq!(
        response["path"],
        selected
            .canonicalize()
            .expect("canonical selected workspace")
            .display()
            .to_string()
    );

    daemon.stop();
    wait_until_stopped(&address);
}

#[test]
fn workspace_file_create_is_atomic_and_never_truncates_existing_content() {
    let root = tempfile::tempdir().expect("temporary workspace create fixture");
    let workspace = root.path().join("workspace");
    let web_dir = root.path().join("web");
    let state_dir = root.path().join("state");
    let config_path = root.path().join("config.acl");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S workspace create test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut daemon, address) = start_detached_web(&workspace, &config_path, &web_dir, &state_dir);
    let path = workspace.join("nested/new.ts");
    let request = serde_json::json!({ "path": path }).to_string();

    http_json(
        &address,
        "POST",
        "/api/v1/workspace/create-file",
        Some(&request),
        "200",
    );
    assert_eq!(fs::read_to_string(&path).expect("read created file"), "");

    fs::write(&path, "preserve me").expect("write existing file");
    let conflict = http_json(
        &address,
        "POST",
        "/api/v1/workspace/create-file",
        Some(&request),
        "409",
    );
    assert!(
        conflict["message"]
            .as_str()
            .is_some_and(|message| message.contains("already exists")),
        "{conflict:#}"
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read existing file"),
        "preserve me"
    );

    daemon.stop();
    wait_until_stopped(&address);
}

#[test]
fn workspace_text_write_enforces_revision_and_content_preconditions() {
    let root = tempfile::tempdir().expect("temporary conditional write fixture");
    let workspace = root.path().join("workspace");
    let web_dir = root.path().join("web");
    let state_dir = root.path().join("state");
    let config_path = root.path().join("config.acl");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S conditional write test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let path = workspace.join("app.ts");
    fs::write(&path, "export const value = 'base';\n").expect("write baseline source");
    let (mut daemon, address) = start_detached_web(&workspace, &config_path, &web_dir, &state_dir);

    let baseline = http_json(
        &address,
        "GET",
        &format!("/api/v1/workspace/read?path={}", path.display()),
        None,
        "200",
    );
    let baseline_revision = baseline["revision"]
        .as_str()
        .expect("read response revision")
        .to_string();

    fs::write(&path, "export const value = 'external';\n").expect("write external source");
    let stale_write = serde_json::json!({
        "path": path,
        "content": "export const value = 'local';\n",
        "expectedRevision": baseline_revision,
    })
    .to_string();
    let conflict = http_json(
        &address,
        "POST",
        "/api/v1/workspace/write",
        Some(&stale_write),
        "412",
    );
    assert!(conflict["message"]
        .as_str()
        .is_some_and(|message| message.contains("changed")));
    assert_eq!(
        fs::read_to_string(&path).expect("read preserved external source"),
        "export const value = 'external';\n"
    );

    let current = http_json(
        &address,
        "GET",
        &format!("/api/v1/workspace/read?path={}", path.display()),
        None,
        "200",
    );
    let matching_write = serde_json::json!({
        "path": path,
        "content": "export const value = 'local';\n",
        "expectedRevision": current["revision"],
    })
    .to_string();
    let saved = http_json(
        &address,
        "POST",
        "/api/v1/workspace/write",
        Some(&matching_write),
        "200",
    );
    assert_eq!(saved["success"], true);
    assert!(saved["revision"]
        .as_str()
        .is_some_and(|revision| !revision.is_empty()));
    assert_eq!(
        fs::read_to_string(&path).expect("read conditionally saved source"),
        "export const value = 'local';\n"
    );

    let stale_content_write = serde_json::json!({
        "path": path,
        "content": "export const value = 'legacy draft';\n",
        "expectedContent": "export const value = 'stale';\n",
    })
    .to_string();
    http_json(
        &address,
        "POST",
        "/api/v1/workspace/write",
        Some(&stale_content_write),
        "412",
    );
    assert_eq!(
        fs::read_to_string(&path).expect("read source after stale content precondition"),
        "export const value = 'local';\n"
    );

    let matching_content_write = serde_json::json!({
        "path": path,
        "content": "export const value = 'legacy draft';\n",
        "expectedContent": "export const value = 'local';\n",
    })
    .to_string();
    let legacy_saved = http_json(
        &address,
        "POST",
        "/api/v1/workspace/write",
        Some(&matching_content_write),
        "200",
    );
    assert!(legacy_saved["revision"]
        .as_str()
        .is_some_and(|revision| revision.starts_with("sha256:")));
    assert_eq!(
        fs::read_to_string(&path).expect("read content-precondition saved source"),
        "export const value = 'legacy draft';\n"
    );

    let missing_content = serde_json::json!({ "path": path }).to_string();
    let invalid = http_json(
        &address,
        "POST",
        "/api/v1/workspace/write",
        Some(&missing_content),
        "400",
    );
    assert!(invalid["message"]
        .as_str()
        .is_some_and(|message| message.contains("content is required")));
    assert_eq!(
        fs::read_to_string(&path).expect("read source after invalid write"),
        "export const value = 'legacy draft';\n"
    );

    daemon.stop();
    wait_until_stopped(&address);
}

#[test]
fn workspace_editor_file_catalog_and_git_workflow_are_native() {
    let root = tempfile::tempdir().expect("temporary Web editor fixture");
    let workspace = root.path().join("workspace");
    let web_dir = root.path().join("web");
    let state_dir = root.path().join("state");
    let config_path = root.path().join("config.acl");
    fs::create_dir_all(workspace.join("src")).expect("create source directory");
    fs::create_dir_all(workspace.join("public")).expect("create public directory");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S Web editor contract test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    fs::write(workspace.join("src/app.ts"), "export const value = 1;\n")
        .expect("write tracked source");
    fs::write(
        workspace.join("src/application.ts"),
        "export const application = true;\n",
    )
    .expect("write fuzzy source");
    fs::write(
        workspace.join("src/binary-prose.md"),
        "Editor contract documentation.\n",
    )
    .expect("write binary prose source");
    fs::write(workspace.join("public/logo.png"), [0_u8, 1, 2, 3]).expect("write binary fixture");
    init_repository(&workspace);
    run_git(
        &workspace,
        &[
            "add",
            "--",
            "src/app.ts",
            "src/application.ts",
            "src/binary-prose.md",
        ],
    );
    run_git(&workspace, &["commit", "--quiet", "-m", "Initial source"]);
    fs::write(workspace.join("src/app.ts"), "export const value = 2;\n")
        .expect("modify tracked source");
    fs::write(
        workspace.join("src/binary-prose.md"),
        "Binary files never create text editor models.\n",
    )
    .expect("modify binary prose source");
    fs::write(
        workspace.join("notes.md"),
        "Review the native editor flow.\n",
    )
    .expect("write untracked source");

    let unborn = root.path().join("unborn");
    fs::create_dir_all(&unborn).expect("create unborn workspace");
    init_repository(&unborn);
    fs::write(unborn.join("draft.ts"), "export const draft = true;\n")
        .expect("write unborn source");

    let repository = root.path().join("nested-repository");
    let nested_workspace = repository.join("packages/editor");
    fs::create_dir_all(&nested_workspace).expect("create nested workspace");
    fs::write(
        nested_workspace.join("nested.ts"),
        "export const nested = 1;\n",
    )
    .expect("write nested source");
    fs::write(repository.join("outside.ts"), "export const outside = 1;\n")
        .expect("write outside source");
    init_repository(&repository);
    run_git(&repository, &["add", "--", "."]);
    run_git(&repository, &["commit", "--quiet", "-m", "Nested baseline"]);
    fs::write(
        nested_workspace.join("nested.ts"),
        "export const nested = 2;\n",
    )
    .expect("modify nested source");
    fs::write(repository.join("outside.ts"), "export const outside = 2;\n")
        .expect("modify outside source");

    let (mut daemon, address) = start_detached_web(&workspace, &config_path, &web_dir, &state_dir);
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");
    let root_query = canonical_workspace.display();

    let catalog = http_json(
        &address,
        "GET",
        &format!("/api/v1/workspace/files?rootPath={root_query}&query=app.ts&maxResults=25"),
        None,
        "200",
    );
    assert_eq!(catalog["workspaceRoot"], root_query.to_string());
    assert_eq!(catalog["items"][0]["relativePath"], "src/app.ts");
    assert_eq!(catalog["items"][0]["name"], "app.ts");
    assert_eq!(catalog["items"][0]["isBinary"], false);
    assert_eq!(
        catalog["items"][0]["path"],
        canonical_workspace.join("src/app.ts").display().to_string()
    );
    assert!(catalog["total"].as_u64().is_some_and(|total| total >= 1));

    let binary_catalog = http_json(
        &address,
        "GET",
        &format!("/api/v1/workspace/files?rootPath={root_query}&query=logo&maxResults=25"),
        None,
        "200",
    );
    assert_eq!(
        binary_catalog["items"][0]["relativePath"],
        "public/logo.png"
    );
    assert_eq!(binary_catalog["items"][0]["isBinary"], true);

    let status = get_git_status(&address, &root_query.to_string());
    assert_eq!(status["isGitRepo"], true);
    assert_eq!(status["branch"], "main");
    assert_git_file_status(&status, "src/app.ts", " ", "M");
    assert_git_file_status(&status, "notes.md", "?", "?");

    let diff = get_git_diff(&address, &root_query.to_string(), "src/app.ts", false);
    assert_eq!(diff["path"], "src/app.ts");
    assert_eq!(diff["staged"], false);
    assert_eq!(diff["original"], "export const value = 1;\n");
    assert_eq!(diff["modified"], "export const value = 2;\n");
    assert_eq!(diff["isBinary"], false);
    assert!(diff["content"]
        .as_str()
        .is_some_and(|content| content.contains("-export const value = 1;")
            && content.contains("+export const value = 2;")));

    let prose_diff = get_git_diff(
        &address,
        &root_query.to_string(),
        "src/binary-prose.md",
        false,
    );
    assert_eq!(prose_diff["isBinary"], false);
    assert_eq!(prose_diff["original"], "Editor contract documentation.\n");
    assert_eq!(
        prose_diff["modified"],
        "Binary files never create text editor models.\n"
    );

    let untracked_diff = get_git_diff(&address, &root_query.to_string(), "notes.md", false);
    assert_eq!(untracked_diff["original"], "");
    assert_eq!(
        untracked_diff["modified"],
        "Review the native editor flow.\n"
    );
    assert!(untracked_diff["content"]
        .as_str()
        .is_some_and(|content| content.contains("+Review the native editor flow.")));

    let binary_diff = get_git_diff(&address, &root_query.to_string(), "public/logo.png", false);
    assert_eq!(binary_diff["isBinary"], true);
    assert_eq!(binary_diff["original"], "");
    assert_eq!(binary_diff["modified"], "");

    let invalid_path = http_json(
        &address,
        "GET",
        &format!(
            "/api/v1/workspace/git-diff?rootPath={root_query}&staged=false&path=../outside.ts"
        ),
        None,
        "400",
    );
    assert!(invalid_path["message"]
        .as_str()
        .is_some_and(|message| message.contains("relative")));

    let stage_request = serde_json::json!({
        "rootPath": canonical_workspace,
        "paths": ["src/app.ts", "notes.md"],
    })
    .to_string();
    let staged_status = post_git_action(&address, "git-stage", &stage_request);
    assert_git_file_status(&staged_status, "src/app.ts", "M", " ");
    assert_git_file_status(&staged_status, "notes.md", "A", " ");

    let staged_diff = get_git_diff(&address, &root_query.to_string(), "src/app.ts", true);
    assert_eq!(staged_diff["original"], "export const value = 1;\n");
    assert_eq!(staged_diff["modified"], "export const value = 2;\n");

    let unstage_request = serde_json::json!({
        "rootPath": canonical_workspace,
        "paths": ["notes.md"],
    })
    .to_string();
    let unstaged_status = post_git_action(&address, "git-unstage", &unstage_request);
    assert_git_file_status(&unstaged_status, "notes.md", "?", "?");

    post_git_action(&address, "git-stage", &stage_request);
    let commit_request = serde_json::json!({
        "rootPath": canonical_workspace,
        "message": "Complete Web editor flow",
    })
    .to_string();
    let committed = post_git_action(&address, "git-commit", &commit_request);
    assert_eq!(committed["committed"], true);
    assert_eq!(committed["status"]["isGitRepo"], true);
    let remaining_paths = git_paths(&committed["status"]);
    assert!(!remaining_paths.contains(&"src/app.ts"), "{committed:#}");
    assert!(!remaining_paths.contains(&"notes.md"), "{committed:#}");
    assert!(
        remaining_paths.contains(&"public/logo.png"),
        "{committed:#}"
    );
    assert!(committed["summary"]
        .as_str()
        .is_some_and(|summary| summary.contains("Complete Web editor flow")));
    assert_eq!(
        run_git_output(&workspace, &["log", "-1", "--format=%s"]),
        "Complete Web editor flow"
    );

    let unborn_root = unborn.canonicalize().expect("canonical unborn workspace");
    let unborn_root = unborn_root.display().to_string();
    let unborn_status = get_git_status(&address, &unborn_root);
    assert_eq!(unborn_status["branch"], "main");
    assert_git_file_status(&unborn_status, "draft.ts", "?", "?");
    let unborn_stage = serde_json::json!({
        "rootPath": unborn_root,
        "paths": ["draft.ts"],
    })
    .to_string();
    let unborn_status = post_git_action(&address, "git-stage", &unborn_stage);
    assert_git_file_status(&unborn_status, "draft.ts", "A", " ");
    let unborn_status = post_git_action(&address, "git-unstage", &unborn_stage);
    assert_git_file_status(&unborn_status, "draft.ts", "?", "?");

    let nested_root = nested_workspace
        .canonicalize()
        .expect("canonical nested workspace")
        .display()
        .to_string();
    let nested_status = get_git_status(&address, &nested_root);
    assert_eq!(nested_status["isGitRepo"], true);
    assert_eq!(git_paths(&nested_status), ["nested.ts"]);
    let nested_diff = get_git_diff(&address, &nested_root, "nested.ts", false);
    assert_eq!(nested_diff["original"], "export const nested = 1;\n");
    assert_eq!(nested_diff["modified"], "export const nested = 2;\n");

    daemon.stop();
    wait_until_stopped(&address);
}

fn get_git_status(address: &str, root: &str) -> serde_json::Value {
    http_json(
        address,
        "GET",
        &format!("/api/v1/workspace/git-status?rootPath={root}"),
        None,
        "200",
    )
}

fn get_git_diff(address: &str, root: &str, path: &str, staged: bool) -> serde_json::Value {
    http_json(
        address,
        "GET",
        &format!("/api/v1/workspace/git-diff?rootPath={root}&staged={staged}&path={path}"),
        None,
        "200",
    )
}

fn post_git_action(address: &str, action: &str, request: &str) -> serde_json::Value {
    http_json(
        address,
        "POST",
        &format!("/api/v1/workspace/{action}"),
        Some(request),
        "200",
    )
}

fn git_paths(status: &serde_json::Value) -> Vec<&str> {
    status["files"]
        .as_array()
        .expect("Git status files")
        .iter()
        .filter_map(|file| file["path"].as_str())
        .collect()
}

fn assert_git_file_status(
    status: &serde_json::Value,
    path: &str,
    index_status: &str,
    worktree_status: &str,
) {
    let file = status["files"]
        .as_array()
        .and_then(|files| files.iter().find(|file| file["path"] == path))
        .unwrap_or_else(|| panic!("missing Git status for {path}: {status:#}"));
    assert_eq!(file["indexStatus"], index_status, "{file:#}");
    assert_eq!(file["worktreeStatus"], worktree_status, "{file:#}");
    assert_eq!(
        file["status"],
        format!("{index_status}{worktree_status}"),
        "{file:#}"
    );
}

fn init_repository(workspace: &Path) {
    run_git(workspace, &["init", "--quiet"]);
    run_git(workspace, &["symbolic-ref", "HEAD", "refs/heads/main"]);
    run_git(workspace, &["config", "user.name", "A3S Test"]);
    run_git(workspace, &["config", "user.email", "a3s-test@example.com"]);
}

fn run_git(workspace: &Path, args: &[&str]) {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .expect("run Git fixture command");
    assert!(
        output.status.success(),
        "git {} failed:\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_git_output(workspace: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(args)
        .output()
        .expect("run Git fixture command");
    assert!(
        output.status.success(),
        "git {} failed: {}",
        args.join(" "),
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("Git fixture output is UTF-8")
        .trim()
        .to_string()
}

fn http_json(
    address: &str,
    method: &str,
    path: &str,
    body: Option<&str>,
    expected_status: &str,
) -> serde_json::Value {
    let response = http_request(address, method, path, body);
    assert!(
        response.starts_with(&format!("HTTP/1.1 {expected_status}")),
        "{response}"
    );
    let (_, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("HTTP response has no body separator: {response}"));
    serde_json::from_str(body)
        .unwrap_or_else(|error| panic!("HTTP response body is not JSON ({error}): {response}"))
}

fn http_request(address: &str, method: &str, path: &str, body: Option<&str>) -> String {
    let mut stream = TcpStream::connect(address).expect("connect to detached web process");
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .expect("set read timeout");
    let body = body.unwrap_or_default();
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write HTTP request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read HTTP response");
    response
}

fn start_detached_web(
    workspace: &Path,
    config_path: &Path,
    web_dir: &Path,
    state_dir: &Path,
) -> (DaemonGuard, String) {
    let output = Command::new(a3s_binary())
        .args([
            "web",
            "-d",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--workspace",
        ])
        .arg(workspace)
        .arg("--web-dir")
        .arg(web_dir)
        .env("A3S_CONFIG_FILE", config_path)
        .env("A3S_CODE_WEB_STATE_DIR", state_dir.join("code-web"))
        .env("A3S_STATE_HOME", state_dir.join("runtime"))
        .env("HOME", state_dir.join("home"))
        .current_dir(workspace)
        .output()
        .expect("start detached web process");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid = output_value(&stdout, "Background PID:")
        .parse::<u32>()
        .expect("background PID");
    let url = output_value(&stdout, "A3S Web:");
    let address = url
        .strip_prefix("http://")
        .and_then(|value| value.strip_suffix('/'))
        .expect("HTTP URL")
        .to_string();
    (DaemonGuard::new(pid), address)
}

fn a3s_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_a3s"))
}

fn output_value<'a>(output: &'a str, prefix: &str) -> &'a str {
    output
        .lines()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim))
        .unwrap_or_else(|| panic!("missing `{prefix}` in output:\n{output}"))
}

fn wait_until_stopped(address: &str) {
    for _ in 0..50 {
        if TcpStream::connect(address).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("detached web process still listens on {address}");
}

struct DaemonGuard {
    pid: u32,
    active: bool,
}

impl DaemonGuard {
    fn new(pid: u32) -> Self {
        Self { pid, active: true }
    }

    fn stop(&mut self) {
        if !self.active {
            return;
        }
        stop_process(self.pid);
        self.active = false;
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

#[cfg(unix)]
fn stop_process(pid: u32) {
    let _ = Command::new("kill")
        .args(["-INT", &pid.to_string()])
        .status();
}

#[cfg(windows)]
fn stop_process(pid: u32) {
    let _ = Command::new("taskkill")
        .args(["/PID", &pid.to_string(), "/T", "/F"])
        .status();
}

fn test_config() -> &'static str {
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
"#
}
