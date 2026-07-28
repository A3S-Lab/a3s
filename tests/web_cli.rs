use std::fs;
use std::io::{Read, Write};
use std::net::{TcpListener, TcpStream};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(unix)]
#[path = "web_cli/web_turn_queue.rs"]
mod web_turn_queue;

#[test]
fn web_help_is_top_level_and_documents_background_mode() {
    let output = Command::new(a3s_binary())
        .args(["web", "--help"])
        .output()
        .expect("run a3s web --help");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("a3s web"), "{stdout}");
    assert!(stdout.contains("-d"), "{stdout}");
    assert!(stdout.contains("--replace"), "{stdout}");
    assert!(!stdout.contains("a3s code serve"), "{stdout}");
}

#[test]
fn legacy_code_serve_command_is_rejected() {
    let output = Command::new(a3s_binary())
        .args(["code", "serve", "--help"])
        .output()
        .expect("run removed command");

    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("a3s web"), "{stderr}");
}

#[test]
fn web_log_stream_flags_use_structured_usage_errors() {
    let output = Command::new(a3s_binary())
        .args(["--output", "json", "web", "logs", "--follow"])
        .output()
        .expect("reject JSON log following");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("JSON error");
    assert_eq!(value["command"], "web.logs");
    assert_eq!(value["error"]["code"], "usage.invalid");
}

#[cfg(unix)]
#[test]
fn followed_web_logs_end_with_a_sequenced_cancellation_event() {
    let root = temp_directory("followed-web-logs");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let session_state = root.join("session-state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S followed log test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut daemon, address) = start_detached_web(&root, &config_path, &web_dir, &session_state);

    let mut follower = Command::new(a3s_binary())
        .args(["--output", "jsonl", "web", "logs", "--follow"])
        .env("HOME", &root)
        .current_dir(&root)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("follow managed Web log");
    let stdout = follower.stdout.take().expect("follower stdout");
    let stderr = follower.stderr.take().expect("follower stderr");
    let stdout_reader = thread::spawn(move || {
        let mut output = String::new();
        std::io::BufReader::new(stdout)
            .read_to_string(&mut output)
            .expect("read follower stdout");
        output
    });
    let stderr_reader = thread::spawn(move || {
        let mut output = String::new();
        std::io::BufReader::new(stderr)
            .read_to_string(&mut output)
            .expect("read follower stderr");
        output
    });
    thread::sleep(Duration::from_millis(750));
    stop_process(follower.id());
    let status = follower.wait().expect("wait for log follower");
    let stdout = stdout_reader.join().expect("join stdout reader");
    let stderr = stderr_reader.join().expect("join stderr reader");

    assert_eq!(
        status.code(),
        Some(130),
        "stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.is_empty(),
        "machine cancellation wrote stderr: {stderr}"
    );
    let events = stdout
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSONL event"))
        .collect::<Vec<_>>();
    assert!(!events.is_empty(), "follow stream was empty");
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence"], (index + 1) as u64, "{events:#?}");
        assert_eq!(event["command"], "web.logs");
    }
    let terminal = events.last().expect("terminal cancellation event");
    assert_eq!(terminal["type"], "error");
    assert_eq!(terminal["error"]["code"], "operation.cancelled");

    daemon.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean followed log fixture");
}

#[test]
fn detached_web_process_serves_health_until_stopped() {
    let root = temp_directory("detached-web");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S Web test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

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
        .arg(&root)
        .arg("--web-dir")
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("HOME", &root)
        .current_dir(&root)
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
    let mut daemon = DaemonGuard::new(pid);
    let url = output_value(&stdout, "A3S Web:");
    let address = url
        .strip_prefix("http://")
        .and_then(|value| value.strip_suffix('/'))
        .expect("HTTP URL");

    let response = http_get(address, "/api/v1/health");
    assert!(response.starts_with("HTTP/1.1 200"), "{response}");
    let (_, body) = response
        .split_once("\r\n\r\n")
        .expect("health response body");
    let health: serde_json::Value = serde_json::from_str(body).expect("health JSON");
    assert_eq!(health["ok"], true);
    assert_eq!(health["service"], "a3s-code-web");
    assert_eq!(health["pid"], pid);
    assert_eq!(health["version"], env!("CARGO_PKG_VERSION"));
    let health_workspace = PathBuf::from(health["workspace"].as_str().expect("health workspace"))
        .canonicalize()
        .expect("canonical health workspace");
    assert_eq!(
        health_workspace,
        root.canonicalize().expect("canonical test workspace")
    );

    daemon.stop();
    wait_until_stopped(address);
    fs::remove_dir_all(root).expect("clean temporary web directory");
}

#[test]
fn plugin_api_exposes_catalog_and_fails_closed_without_trust_roots() {
    let root = temp_directory("web-plugin-api");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let session_state = root.join("session-state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S plugin API test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut daemon, address) = start_detached_web(&root, &config_path, &web_dir, &session_state);

    let activities = http_json(&address, "GET", "/api/v1/plugins/activities", None, "200");
    assert_eq!(activities["schemaVersion"], 1);
    assert!(activities["available"].is_boolean());
    assert!(activities["items"].is_array());

    let marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None, "200");
    assert_eq!(marketplace["schemaVersion"], 1);
    assert!(marketplace["registries"]
        .as_array()
        .is_some_and(|registries| registries
            .iter()
            .all(|registry| registry["verified"] == false)));
    assert_eq!(marketplace["items"], serde_json::json!([]));

    let invalid_key = http_json(
        &address,
        "GET",
        "/api/v1/plugins/activities/not-a-key",
        None,
        "400",
    );
    assert!(invalid_key["message"]
        .as_str()
        .is_some_and(|message| message.contains("Activity Bar contribution key")));

    let invalid_plan = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/plan",
        Some(r#"{"action":"install","componentId":"science"}"#),
        "400",
    );
    assert!(invalid_plan["message"]
        .as_str()
        .is_some_and(|message| message.contains("use/<publisher>/<name>")));

    daemon.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean plugin API fixture");
}

#[test]
fn knowledge_marketplace_creates_and_installs_real_personal_bases() {
    let root = temp_directory("web-knowledge-marketplace");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let session_state = root.join("session-state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S knowledge marketplace test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut daemon, address) = start_detached_web(&root, &config_path, &web_dir, &session_state);

    let marketplace = http_json(
        &address,
        "GET",
        "/api/v1/knowledge/marketplace",
        None,
        "200",
    );
    let marketplace = api_data(&marketplace);
    assert_eq!(marketplace["schemaVersion"], 1);
    assert_eq!(marketplace["format"], "okf");
    assert_eq!(marketplace["source"]["verified"], true);
    let market_id = marketplace["items"]
        .as_array()
        .and_then(|items| items.first())
        .and_then(|item| item["id"].as_str())
        .expect("curated OKF marketplace item")
        .to_string();
    assert_eq!(marketplace["items"][0]["installed"], false);

    let created = http_json(
        &address,
        "POST",
        "/api/v1/knowledge/bases",
        Some(r#"{"name":"Project Notes","description":"Local project knowledge"}"#),
        "200",
    );
    let created = api_data(&created);
    let created_id = created["knowledgeBase"]["id"]
        .as_str()
        .expect("created knowledge base id");
    assert_eq!(created["changed"], true);
    assert_eq!(created["knowledgeBase"]["origin"], "created");
    assert!(root
        .join(".a3s/kb/bases")
        .join(created_id)
        .join(".a3s/knowledge-base.acl")
        .is_file());

    let vault = root.join("Research Vault");
    fs::create_dir_all(vault.join("topics")).expect("create Obsidian vault fixture");
    fs::write(vault.join("Home.md"), "# Home\n\n[[topics/Methods]]\n")
        .expect("write Obsidian home note");
    fs::write(vault.join("topics/Methods.md"), "# Methods\n").expect("write Obsidian nested note");
    let import_request = serde_json::json!({ "path": vault, "name": "Research Vault" });
    let imported = http_json(
        &address,
        "POST",
        "/api/v1/knowledge/bases/import",
        Some(&import_request.to_string()),
        "200",
    );
    let imported = api_data(&imported);
    let imported_id = imported["knowledgeBase"]["id"]
        .as_str()
        .expect("imported knowledge base id");
    assert_eq!(imported["knowledgeBase"]["origin"], "imported");
    assert!(root
        .join(".a3s/kb/bases")
        .join(imported_id)
        .join("sources/Home.md")
        .is_file());

    let installed = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/marketplace/{market_id}/install"),
        Some("{}"),
        "200",
    );
    let installed = api_data(&installed);
    assert_eq!(installed["changed"], true);
    assert_eq!(installed["knowledgeBase"]["marketplaceId"], market_id);
    assert_eq!(installed["knowledgeBase"]["origin"], "marketplace");
    assert!(root
        .join(".a3s/kb/bases")
        .join(&market_id)
        .join("README.md")
        .is_file());
    assert!(root
        .join(".a3s/kb/bases")
        .join(&market_id)
        .join(".a3s/asset.acl")
        .is_file());

    let repeated = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/marketplace/{market_id}/install"),
        Some("{}"),
        "200",
    );
    assert_eq!(api_data(&repeated)["changed"], false);

    let bases = http_json(&address, "GET", "/api/v1/knowledge/bases", None, "200");
    let bases = api_data(&bases);
    assert_eq!(bases["total"], 3);
    assert!(bases["items"].as_array().is_some_and(|items| {
        items
            .iter()
            .any(|item| item["id"] == created_id && item["origin"] == "created")
            && items.iter().any(|item| {
                item["id"] == market_id
                    && item["origin"] == "marketplace"
                    && item["marketplaceId"] == market_id
            })
    }));

    let pinned = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/bases/{created_id}/pinned"),
        Some(r#"{"pinned":false}"#),
        "200",
    );
    assert_eq!(api_data(&pinned)["knowledgeBase"]["pinned"], false);

    let refreshed_marketplace = http_json(
        &address,
        "GET",
        "/api/v1/knowledge/marketplace",
        None,
        "200",
    );
    assert!(api_data(&refreshed_marketplace)["items"]
        .as_array()
        .is_some_and(|items| items
            .iter()
            .any(|item| { item["id"] == market_id && item["installed"] == true })));

    daemon.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean knowledge marketplace fixture");
}

#[test]
fn knowledge_selection_creation_and_compilation_are_separate_api_steps() {
    let root = temp_directory("web-knowledge-selection");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let session_state = root.join("session-state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S knowledge selection test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

    let research = root.join("Research");
    fs::create_dir_all(research.join("empty")).expect("create source directories");
    fs::write(research.join("note.md"), "# Research note\n").expect("write source note");
    let brief = root.join("brief.txt");
    fs::write(&brief, "Project brief").expect("write source brief");

    let (mut daemon, address) = start_detached_web(&root, &config_path, &web_dir, &session_state);
    let paths = serde_json::json!([research, root.join("Research/note.md"), brief]);
    let preview_request = serde_json::json!({ "paths": paths.clone() });
    let preview = http_json(
        &address,
        "POST",
        "/api/v1/knowledge/bases/from-selection/preview",
        Some(&preview_request.to_string()),
        "200",
    );
    let preview = api_data(&preview);
    assert_eq!(preview["selectedCount"], 3);
    assert_eq!(preview["sourceRootCount"], 2);
    assert_eq!(preview["deduplicatedCount"], 1);
    assert_eq!(preview["fileCount"], 2);

    let create_request = serde_json::json!({
        "paths": paths,
        "name": "Research Pack",
        "description": "Mixed local sources",
        "compilationPolicy": "manual",
    });
    let created = http_json(
        &address,
        "POST",
        "/api/v1/knowledge/bases/from-selection",
        Some(&create_request.to_string()),
        "200",
    );
    let created = api_data(&created);
    let base_id = created["knowledgeBase"]["id"]
        .as_str()
        .expect("created base ID")
        .to_string();
    assert_eq!(created["knowledgeBase"]["origin"], "selection");
    assert_eq!(
        created["knowledgeBase"]["compilation"]["phase"],
        "source_ready"
    );
    assert_eq!(created["knowledgeBase"]["compilation"]["policy"], "manual");

    let queued = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/bases/{base_id}/compilations"),
        Some("{}"),
        "200",
    );
    assert_eq!(
        api_data(&queued)["knowledgeBase"]["compilation"]["phase"],
        "queued"
    );

    let claim = http_json(
        &address,
        "POST",
        "/api/v1/knowledge/compilations/claim",
        Some("{}"),
        "200",
    );
    let claim = api_data(&claim);
    assert_eq!(claim["claimed"], true);
    let job_id = claim["job"]["id"]
        .as_str()
        .expect("claimed job ID")
        .to_string();
    let output_path = PathBuf::from(claim["outputPath"].as_str().expect("compiler output path"));
    fs::write(
        output_path.join("index.md"),
        "---\ntype: Note\n---\n# Compiled research\n",
    )
    .expect("write compiler output");
    let completed = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/bases/{base_id}/compilations/{job_id}/result"),
        Some(r#"{"outcome":"succeeded","compilerVersion":"test-compiler"}"#),
        "200",
    );
    assert_eq!(
        api_data(&completed)["knowledgeBase"]["compilation"]["phase"],
        "succeeded"
    );
    let wiki = root
        .join(".a3s/kb/bases")
        .join(&base_id)
        .join("wiki/index.md");
    assert!(fs::read_to_string(&wiki)
        .expect("read compiled wiki")
        .contains("Compiled research"));

    let second = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/bases/{base_id}/compilations"),
        Some("{}"),
        "200",
    );
    assert_eq!(
        api_data(&second)["knowledgeBase"]["compilation"]["phase"],
        "queued"
    );
    let second_claim = http_json(
        &address,
        "POST",
        "/api/v1/knowledge/compilations/claim",
        Some("{}"),
        "200",
    );
    let second_claim = api_data(&second_claim);
    let second_job_id = second_claim["job"]["id"]
        .as_str()
        .expect("second claimed job ID");
    let failed = http_json(
        &address,
        "POST",
        &format!("/api/v1/knowledge/bases/{base_id}/compilations/{second_job_id}/result"),
        Some(r#"{"outcome":"failed","transient":false,"error":"fixture failure"}"#),
        "200",
    );
    assert_eq!(
        api_data(&failed)["knowledgeBase"]["compilation"]["phase"],
        "failed"
    );
    assert!(fs::read_to_string(&wiki)
        .expect("read preserved wiki")
        .contains("Compiled research"));

    daemon.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean knowledge selection fixture");
}

#[test]
fn evolution_api_reviews_versions_and_rolls_back_local_assets() {
    let root = temp_directory("web-evolution-api");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let session_state = root.join("session-state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S evolution API test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    seed_evolution_catalog(&root);
    let evolution_state = root.join(".a3s/evolution/state.json");
    let state_before_overview = fs::read(&evolution_state).expect("read seeded evolution state");
    let (mut daemon, address) = start_detached_web(&root, &config_path, &web_dir, &session_state);

    let overview = http_json(&address, "GET", "/api/v1/evolution", None, "200");
    assert_eq!(
        fs::read(&evolution_state).expect("read evolution state after overview"),
        state_before_overview,
        "GET evolution overview must not mutate the catalog"
    );
    let overview = api_data(&overview);
    assert_eq!(overview["schema"], "a3s.code.evolution.v1");
    assert_eq!(overview["stats"]["total"], 2);
    assert_eq!(
        overview["skillRoot"],
        root.join(".a3s/skills").display().to_string()
    );
    assert_eq!(overview["okfRoot"], root.join("okf").display().to_string());
    assert_eq!(overview["policy"]["localOnly"], true);

    let scan = http_json(
        &address,
        "POST",
        "/api/v1/evolution/scan",
        Some("{}"),
        "200",
    );
    assert_eq!(api_data(&scan)["observed"], 0);

    let first = http_json(
        &address,
        "POST",
        "/api/v1/evolution/preference-api-test/materialize",
        Some(r#"{"force":false}"#),
        "200",
    );
    let first = api_data(&first);
    assert_eq!(first["result"]["candidate"]["currentVersion"], 1);
    let asset_path = first["result"]["candidate"]["assetPath"]
        .as_str()
        .expect("materialized preference path");
    assert!(root.join(asset_path).is_file());

    append_evolution_update(&root, "preference-api-test");
    let second = http_json(
        &address,
        "POST",
        "/api/v1/evolution/preference-api-test/materialize",
        Some(r#"{"force":false}"#),
        "200",
    );
    assert_eq!(
        api_data(&second)["result"]["candidate"]["currentVersion"],
        2
    );

    let rollback = http_json(
        &address,
        "POST",
        "/api/v1/evolution/preference-api-test/rollback",
        Some(r#"{"targetVersion":1}"#),
        "200",
    );
    let rollback = api_data(&rollback);
    assert_eq!(rollback["result"]["candidate"]["state"], "rolledBack");
    assert_eq!(rollback["result"]["candidate"]["currentVersion"], 1);
    assert!(rollback["result"]["recoveryPath"]
        .as_str()
        .is_some_and(|path| root.join(path).exists()));

    let baseline = http_json(
        &address,
        "POST",
        "/api/v1/evolution/preference-api-test/rollback",
        Some(r#"{"targetVersion":0}"#),
        "200",
    );
    let baseline = api_data(&baseline);
    assert_eq!(baseline["result"]["candidate"]["state"], "rolledBack");
    assert!(baseline["result"]["candidate"]["currentVersion"].is_null());
    assert!(!root.join(asset_path).exists());

    let restored = http_json(
        &address,
        "POST",
        "/api/v1/evolution/preference-api-test/rollback",
        Some(r#"{"targetVersion":1}"#),
        "200",
    );
    assert_eq!(
        api_data(&restored)["result"]["candidate"]["currentVersion"],
        1
    );
    assert!(root.join(asset_path).is_file());

    let rejected = http_json(
        &address,
        "POST",
        "/api/v1/evolution/okf-api-test/reject",
        Some(r#"{"reason":"Not reusable in this workspace"}"#),
        "200",
    );
    assert_eq!(api_data(&rejected)["state"], "rejected");
    assert_eq!(
        api_data(&rejected)["rejectionReason"],
        "Not reusable in this workspace"
    );
    let reopened = http_json(
        &address,
        "POST",
        "/api/v1/evolution/okf-api-test/reopen",
        Some("{}"),
        "200",
    );
    assert_eq!(api_data(&reopened)["state"], "observing");

    daemon.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean evolution API fixture");
}

#[test]
fn packaged_web_assets_work_from_an_empty_workspace() {
    let root = temp_directory("packaged-web-assets");
    let workspace = root.join("workspace");
    let prefix = root.join("prefix");
    let packaged_binary = prefix
        .join("bin")
        .join(if cfg!(windows) { "a3s.exe" } else { "a3s" });
    let packaged_web = prefix.join("share/a3s/web");
    let config_path = root.join("config.acl");
    let state_home = root.join("state");
    fs::create_dir_all(&workspace).expect("create empty workspace");
    fs::create_dir_all(
        packaged_binary
            .parent()
            .expect("packaged binary parent directory"),
    )
    .expect("create package bin directory");
    fs::create_dir_all(&packaged_web).expect("create packaged Web directory");
    fs::copy(a3s_binary(), &packaged_binary).expect("copy packaged a3s binary");
    fs::write(
        packaged_web.join("index.html"),
        "<!doctype html><title>A3S packaged Web test</title>",
    )
    .expect("write packaged Web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

    let start = Command::new(&packaged_binary)
        .args([
            "-C",
            workspace.to_str().expect("UTF-8 workspace"),
            "--config",
            config_path.to_str().expect("UTF-8 config path"),
            "web",
            "start",
            "--detach",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
        ])
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .output()
        .expect("start Web from packaged layout");
    assert!(
        start.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&start.stdout),
        String::from_utf8_lossy(&start.stderr)
    );
    let stdout = String::from_utf8_lossy(&start.stdout);
    let pid = output_value(&stdout, "Background PID:")
        .parse::<u32>()
        .expect("packaged background PID");
    let mut guard = DaemonGuard::new(pid);
    let address = output_value(&stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let page = http_get(&address, "/");
    assert!(page.starts_with("HTTP/1.1 200"), "{page}");
    assert!(page.contains("A3S packaged Web test"), "{page}");

    let stop = Command::new(&packaged_binary)
        .args([
            "-C",
            workspace.to_str().expect("UTF-8 workspace"),
            "web",
            "stop",
        ])
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .output()
        .expect("stop packaged Web");
    assert!(
        stop.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&stop.stdout),
        String::from_utf8_lossy(&stop.stderr)
    );
    guard.disarm();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean packaged Web fixture");
}

#[test]
fn global_directory_and_relative_acl_reach_detached_web() {
    let root = temp_directory("global-context-web");
    let launch_directory = root.join("launch");
    let workspace = root.join("workspace");
    let state_home = root.join("state");
    fs::create_dir_all(&launch_directory).expect("create launch directory");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::write(workspace.join("config.acl"), test_config()).expect("write relative config");
    let canonical_workspace = workspace.canonicalize().expect("canonical workspace");

    let output = Command::new(a3s_binary())
        .current_dir(&launch_directory)
        .env("HOME", root.join("home"))
        .env("A3S_STATE_HOME", &state_home)
        .env_remove("A3S_CONFIG_FILE")
        .arg("-C")
        .arg(&workspace)
        .args([
            "--config",
            "config.acl",
            "web",
            "start",
            "--detach",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--api-only",
        ])
        .output()
        .expect("start Web with global context");
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
    let mut daemon = DaemonGuard::new(pid);
    let address = output_value(&stdout, "A3S Code API:")
        .trim_start_matches("http://")
        .trim_end_matches("/api/health")
        .to_string();

    let status = Command::new(a3s_binary())
        .current_dir(&launch_directory)
        .env("HOME", root.join("home"))
        .env("A3S_STATE_HOME", &state_home)
        .arg("-C")
        .arg(&workspace)
        .args(["--output", "json", "web", "status"])
        .output()
        .expect("inspect Web through global directory");
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(status["data"]["running"], true);
    assert_eq!(
        status["data"]["workspace"],
        canonical_workspace.display().to_string()
    );
    assert_eq!(
        status["data"]["instance"]["workspace"],
        canonical_workspace.display().to_string()
    );

    let stop = Command::new(a3s_binary())
        .current_dir(&launch_directory)
        .env("HOME", root.join("home"))
        .env("A3S_STATE_HOME", &state_home)
        .arg("-C")
        .arg(&workspace)
        .args(["--output", "json", "web", "stop"])
        .output()
        .expect("stop Web through global directory");
    assert!(
        stop.status.success(),
        "stderr: {}",
        String::from_utf8_lossy(&stop.stderr)
    );
    daemon.disarm();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean global-context Web fixture");
}

#[test]
fn canonical_web_lifecycle_uses_managed_workspace_state() {
    let root = temp_directory("managed-web-lifecycle");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_home = root.join("state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S managed Web test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

    let start = Command::new(a3s_binary())
        .args([
            "web",
            "start",
            "--detach",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--web-dir",
        ])
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("start managed Web");
    assert!(
        start.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&start.stdout),
        String::from_utf8_lossy(&start.stderr)
    );
    let start_stdout = String::from_utf8_lossy(&start.stdout);
    let pid = output_value(&start_stdout, "Background PID:")
        .parse::<u32>()
        .expect("background PID");
    let mut guard = DaemonGuard::new(pid);
    let address = output_value(&start_stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();

    let status = Command::new(a3s_binary())
        .args(["--output", "json", "web", "status"])
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("inspect managed Web");
    assert!(status.status.success());
    let status_json: serde_json::Value =
        serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(status_json["command"], "web.status");
    assert_eq!(status_json["data"]["running"], true);
    assert_eq!(status_json["data"]["instance"]["pid"], pid);
    assert!(
        !String::from_utf8_lossy(&status.stdout).contains("nonce"),
        "control nonce must not be exposed"
    );

    let repeated_start = Command::new(a3s_binary())
        .args([
            "web",
            "start",
            "--detach",
            "--host",
            "127.0.0.1",
            "--port",
            "0",
            "--web-dir",
        ])
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("repeat managed Web start");
    assert!(
        repeated_start.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&repeated_start.stdout),
        String::from_utf8_lossy(&repeated_start.stderr)
    );
    let repeated_stdout = String::from_utf8_lossy(&repeated_start.stdout);
    assert!(
        repeated_stdout.contains("reused"),
        "repeat start should explain that it reused the healthy instance:\n{repeated_stdout}"
    );
    assert_eq!(
        output_value(&repeated_stdout, "Background PID:")
            .parse::<u32>()
            .expect("reused background PID"),
        pid
    );

    let logs = Command::new(a3s_binary())
        .args(["--output", "jsonl", "web", "logs", "--lines", "10"])
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("read managed Web log as JSONL");
    assert!(logs.status.success());
    let events = String::from_utf8_lossy(&logs.stdout)
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let terminal = events.last().expect("terminal log event");
    assert_eq!(terminal["command"], "web.logs");
    assert_eq!(terminal["type"], "result");
    assert_eq!(terminal["ok"], true);

    let stop = Command::new(a3s_binary())
        .args(["--output", "json", "web", "stop"])
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("stop managed Web");
    assert!(
        stop.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&stop.stdout),
        String::from_utf8_lossy(&stop.stderr)
    );
    let stop_json: serde_json::Value = serde_json::from_slice(&stop.stdout).expect("stop JSON");
    assert_eq!(stop_json["data"]["stopped"], true);
    wait_until_stopped(&address);
    guard.disarm();

    let stopped = Command::new(a3s_binary())
        .args(["--output", "json", "web", "status"])
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("inspect stopped Web");
    let stopped_json: serde_json::Value =
        serde_json::from_slice(&stopped.stdout).expect("stopped status JSON");
    assert_eq!(stopped_json["data"]["running"], false);
    fs::remove_dir_all(root).expect("clean managed Web fixture");
}

#[test]
fn web_replace_gracefully_restarts_only_a_managed_instance() {
    let root = temp_directory("replace-managed-web");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_home = root.join("state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S replace test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

    let first = start_managed_web(&root, &config_path, &web_dir, &state_home, &[]);
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let first_pid = output_value(&first_stdout, "Background PID:")
        .parse::<u32>()
        .expect("first background PID");
    let first_address = output_value(&first_stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    let mut first_guard = DaemonGuard::new(first_pid);

    let replacement = start_managed_web(&root, &config_path, &web_dir, &state_home, &["--replace"]);
    assert!(
        replacement.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&replacement.stdout),
        String::from_utf8_lossy(&replacement.stderr)
    );
    let replacement_stdout = String::from_utf8_lossy(&replacement.stdout);
    let replacement_pid = output_value(&replacement_stdout, "Background PID:")
        .parse::<u32>()
        .expect("replacement background PID");
    let replacement_address = output_value(&replacement_stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    assert_ne!(replacement_pid, first_pid);
    first_guard.disarm();
    wait_until_stopped(&first_address);

    let mut replacement_guard = DaemonGuard::new(replacement_pid);
    replacement_guard.stop();
    wait_until_stopped(&replacement_address);
    fs::remove_dir_all(root).expect("clean replacement Web fixture");
}

#[test]
fn concurrent_managed_web_starts_converge_on_one_instance() {
    let root = temp_directory("concurrent-managed-web");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_home = root.join("state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S concurrent start test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

    let spawn = || {
        let mut command = Command::new(a3s_binary());
        command
            .args([
                "web",
                "start",
                "--detach",
                "--host",
                "127.0.0.1",
                "--port",
                "0",
                "--web-dir",
            ])
            .arg(&web_dir)
            .env("A3S_CONFIG_FILE", &config_path)
            .env("A3S_STATE_HOME", &state_home)
            .env("HOME", &root)
            .current_dir(&root)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn concurrent managed Web start")
    };
    let first = spawn();
    let second = spawn();
    let first = first.wait_with_output().expect("wait for first Web start");
    let second = second
        .wait_with_output()
        .expect("wait for second Web start");
    for output in [&first, &second] {
        assert!(
            output.status.success(),
            "stdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    let first_stdout = String::from_utf8_lossy(&first.stdout);
    let second_stdout = String::from_utf8_lossy(&second.stdout);
    let first_pid = output_value(&first_stdout, "Background PID:")
        .parse::<u32>()
        .expect("first background PID");
    let second_pid = output_value(&second_stdout, "Background PID:")
        .parse::<u32>()
        .expect("second background PID");
    assert_eq!(first_pid, second_pid);
    assert!(
        first_stdout.contains("reused") || second_stdout.contains("reused"),
        "one concurrent caller should report reuse:\n{first_stdout}\n{second_stdout}"
    );
    let address = output_value(&first_stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();

    let mut guard = DaemonGuard::new(first_pid);
    guard.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean concurrent Web fixture");
}

#[test]
fn web_port_conflict_never_stops_an_unrelated_listener() {
    let root = temp_directory("foreign-web-port");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_home = root.join("state");
    let session_state = root.join("session-state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S conflict test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let (mut seed, seed_address) =
        start_detached_web(&root, &config_path, &web_dir, &session_state);
    http_json(
        &seed_address,
        "POST",
        "/api/v1/kernel/sessions",
        Some(
            r#"{"title":"Unavailable model fixture","model":"openai/test","followDefaultModel":false}"#,
        ),
        "200",
    );
    seed.stop();
    wait_until_stopped(&seed_address);
    fs::write(&config_path, alternate_test_config()).expect("replace config fixture");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind unrelated listener");
    let port = listener
        .local_addr()
        .expect("foreign listener address")
        .port();

    let output = Command::new(a3s_binary())
        .args([
            "web",
            "--replace",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--web-dir",
        ])
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("A3S_CODE_WEB_STATE_DIR", &session_state)
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("run Web against unrelated listener");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("already in use"), "{stderr}");
    assert!(stderr.contains("no process was stopped"), "{stderr}");
    assert!(
        !stderr.contains("saved Code Web session") && !stderr.contains("Sessions unavailable"),
        "port preflight must happen before session restoration:\n{stderr}"
    );
    assert!(
        listener.local_addr().is_ok(),
        "unrelated listener was closed"
    );

    fs::remove_dir_all(root).expect("clean conflict Web fixture");
}

#[test]
fn web_reuses_a_healthy_unmanaged_a3s_instance_and_replaces_it_only_when_requested() {
    let root = temp_directory("reuse-unmanaged-web");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_home = root.join("state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S unmanaged reuse test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");
    let port = available_port();
    let address = format!("127.0.0.1:{port}");

    let mut foreground = Command::new(a3s_binary())
        .args([
            "web",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--workspace",
        ])
        .arg(&root)
        .arg("--web-dir")
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .expect("start unmanaged foreground Web");
    let foreground_pid = foreground.id();
    let mut foreground_guard = DaemonGuard::new(foreground_pid);
    wait_until_healthy(&address);

    let repeated = Command::new(a3s_binary())
        .args([
            "web",
            "start",
            "--detach",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--web-dir",
        ])
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("reuse unmanaged foreground Web");
    assert!(
        repeated.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&repeated.stdout),
        String::from_utf8_lossy(&repeated.stderr)
    );
    let repeated_stdout = String::from_utf8_lossy(&repeated.stdout);
    assert!(repeated_stdout.contains("reused"), "{repeated_stdout}");
    assert!(
        repeated_stdout.contains("Managed:         no"),
        "{repeated_stdout}"
    );

    let status = Command::new(a3s_binary())
        .args(["--output", "json", "web", "status"])
        .env("A3S_CODE_WEB_HOST", "127.0.0.1")
        .env("A3S_CODE_WEB_PORT", port.to_string())
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("discover unmanaged foreground Web");
    assert!(status.status.success());
    let status: serde_json::Value = serde_json::from_slice(&status.stdout).expect("status JSON");
    assert_eq!(status["data"]["running"], true);
    assert_eq!(status["data"]["managed"], false);
    assert_eq!(status["data"]["instance"]["pid"], foreground_pid);

    let stop = Command::new(a3s_binary())
        .args(["web", "stop"])
        .env("A3S_CODE_WEB_HOST", "127.0.0.1")
        .env("A3S_CODE_WEB_PORT", port.to_string())
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("refuse unmanaged foreground Web stop");
    assert!(!stop.status.success());
    let stderr = String::from_utf8_lossy(&stop.stderr);
    assert!(stderr.contains("not managed"), "{stderr}");
    assert!(stderr.contains("no process was stopped"), "{stderr}");
    wait_until_healthy(&address);

    let replacement = Command::new(a3s_binary())
        .args([
            "web",
            "start",
            "--detach",
            "--replace",
            "--host",
            "127.0.0.1",
            "--port",
            &port.to_string(),
            "--web-dir",
        ])
        .arg(&web_dir)
        .env("A3S_CONFIG_FILE", &config_path)
        .env("A3S_STATE_HOME", &state_home)
        .env("HOME", &root)
        .current_dir(&root)
        .output()
        .expect("replace verified unmanaged Web instance");
    assert!(
        replacement.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&replacement.stdout),
        String::from_utf8_lossy(&replacement.stderr)
    );
    let replacement_stdout = String::from_utf8_lossy(&replacement.stdout);
    let replacement_pid = output_value(&replacement_stdout, "Background PID:")
        .parse::<u32>()
        .expect("replacement background PID");
    assert_ne!(replacement_pid, foreground_pid);
    let replacement_address = output_value(&replacement_stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    assert_eq!(replacement_address, address);

    let deadline = Instant::now() + Duration::from_secs(5);
    let foreground_status = loop {
        if let Some(status) = foreground.try_wait().expect("poll replaced foreground Web") {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "replaced foreground Web did not stop"
        );
        thread::sleep(Duration::from_millis(25));
    };
    assert!(foreground_status.success(), "{foreground_status}");
    foreground_guard.disarm();

    let health = http_json(&address, "GET", "/api/v1/health", None, "200");
    assert_eq!(health["pid"], replacement_pid);
    let mut replacement_guard = DaemonGuard::new(replacement_pid);
    replacement_guard.stop();
    wait_until_stopped(&address);
    fs::remove_dir_all(root).expect("clean unmanaged Web fixture");
}

#[cfg(unix)]
#[test]
fn web_sessions_restore_across_restarts_and_hitl_route_is_registered() {
    let root = temp_directory("web-session-restart");
    let config_path = root.join("config.acl");
    let web_dir = root.join("web");
    let state_dir = root.join("state");
    fs::create_dir_all(&web_dir).expect("create web directory");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S Web persistence test</title>",
    )
    .expect("write web fixture");
    fs::write(&config_path, test_config()).expect("write config fixture");

    let (mut first, first_address) = start_detached_web(&root, &config_path, &web_dir, &state_dir);
    let created = http_json(
        &first_address,
        "POST",
        "/api/v1/kernel/sessions",
        Some(r#"{"title":"Persistence regression","permissionMode":"default"}"#),
        "200",
    );
    let session_id = created["session"]["sessionId"]
        .as_str()
        .expect("created session id")
        .to_string();
    let created_at = created["session"]["createdAt"]
        .as_i64()
        .expect("created timestamp");

    http_json(
        &first_address,
        "PATCH",
        &format!("/api/v1/kernel/sessions/{session_id}/controls"),
        Some(r#"{"effort":"high","goal":"Persist goal"}"#),
        "200",
    );
    http_json(
        &first_address,
        "PATCH",
        &format!("/api/v1/kernel/sessions/{session_id}"),
        Some(r#"{"permissionMode":"plan"}"#),
        "200",
    );
    http_json(
        &first_address,
        "POST",
        &format!("/api/v1/kernel/sessions/{session_id}/actions/shell"),
        Some(r#"{"command":"printf persistence-ok"}"#),
        "200",
    );
    first.stop();
    wait_until_stopped(&first_address);

    let (mut second, second_address) =
        start_detached_web(&root, &config_path, &web_dir, &state_dir);
    let sessions = http_json(
        &second_address,
        "GET",
        "/api/v1/kernel/sessions",
        None,
        "200",
    );
    assert_eq!(sessions["total"], 1);
    assert_eq!(sessions["items"][0]["sessionId"], session_id);
    assert_eq!(sessions["items"][0]["title"], "Persistence regression");
    assert_eq!(sessions["items"][0]["createdAt"], created_at);
    assert_eq!(sessions["items"][0]["permissionMode"], "plan");

    let messages = http_json(
        &second_address,
        "GET",
        &format!("/api/v1/kernel/sessions/{session_id}/messages"),
        None,
        "200",
    );
    assert!(messages["items"].as_array().is_some_and(|items| {
        items.iter().any(|message| {
            message["contentBlocks"].as_array().is_some_and(|blocks| {
                blocks
                    .iter()
                    .any(|block| block["content"] == "persistence-ok")
            })
        })
    }));
    let controls = http_json(
        &second_address,
        "GET",
        &format!("/api/v1/kernel/sessions/{session_id}/controls"),
        None,
        "200",
    );
    assert_eq!(controls["effort"], "high");
    assert_eq!(controls["goal"], "Persist goal");

    let stale_confirmation = http_json(
        &second_address,
        "POST",
        &format!("/api/v1/kernel/sessions/{session_id}/confirmations/stale-tool/confirm"),
        Some(r#"{"approved":true}"#),
        "400",
    );
    assert!(stale_confirmation["message"]
        .as_str()
        .is_some_and(|message| message.contains("no longer pending")));

    http_json(
        &second_address,
        "DELETE",
        &format!("/api/v1/kernel/sessions/{session_id}"),
        None,
        "200",
    );
    second.stop();
    wait_until_stopped(&second_address);

    let (mut third, third_address) = start_detached_web(&root, &config_path, &web_dir, &state_dir);
    let sessions = http_json(
        &third_address,
        "GET",
        "/api/v1/kernel/sessions",
        None,
        "200",
    );
    assert_eq!(sessions["total"], 0);
    third.stop();
    wait_until_stopped(&third_address);
    fs::remove_dir_all(root).expect("clean temporary persistence directory");
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

fn http_get(address: &str, path: &str) -> String {
    http_request(address, "GET", path, None)
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

fn api_data(value: &serde_json::Value) -> &serde_json::Value {
    value.get("data").unwrap_or(value)
}

fn seed_evolution_catalog(workspace: &std::path::Path) {
    let root = workspace.join(".a3s/evolution");
    fs::create_dir_all(&root).expect("create evolution state directory");
    let now = "2026-07-21T08:00:00Z";
    let candidate = |id: &str, kind: &str, pattern: &str, title: &str| {
        serde_json::json!({
            "id": id,
            "kind": kind,
            "patternKey": pattern,
            "patternAliases": [],
            "title": title,
            "summary": format!("Reusable local guidance for {title}."),
            "instructions": ["Review current workspace evidence before applying this guidance."],
            "state": "ready",
            "evidence": [{
                "id": format!("{id}-evidence-one"),
                "memoryId": format!("{id}-memory-one"),
                "sessionId": "session-one",
                "source": if kind == "preference" { "preference" } else { "project_fact" },
                "content": format!("Stable evidence for {title}."),
                "reason": "This pattern changes future task execution.",
                "timestamp": now,
                "importance": 0.9,
                "confidence": 0.95,
                "conflictsWith": [],
                "explicitSignal": true
            }],
            "occurrences": 1,
            "distinctSessions": 1,
            "confidence": 0.95,
            "importance": 0.9,
            "maturity": 0.86,
            "hasConflicts": false,
            "updateAvailable": false,
            "activationPending": false,
            "createdAt": now,
            "updatedAt": now,
            "readyAt": now,
            "materializedAt": null,
            "rejectedAt": null,
            "rolledBackAt": null,
            "rejectionReason": null,
            "assetPath": null,
            "currentVersion": null,
            "versions": [],
            "audit": [{
                "action": "ready",
                "at": now,
                "version": null,
                "note": "Seeded for API integration verification.",
                "recoveryPath": null
            }]
        })
    };
    let catalog = serde_json::json!({
        "schema": "a3s.code.evolution.v1",
        "revision": 1,
        "workspaceRoot": workspace.display().to_string(),
        "updatedAt": now,
        "candidates": [
            candidate(
                "preference-api-test",
                "preference",
                "preference.response.evidence",
                "Evidence-backed responses"
            ),
            candidate(
                "okf-api-test",
                "okf",
                "okf.workspace.architecture",
                "Workspace architecture"
            )
        ]
    });
    fs::write(
        root.join("state.json"),
        serde_json::to_vec_pretty(&catalog).expect("serialize evolution catalog"),
    )
    .expect("write evolution catalog");
}

fn append_evolution_update(workspace: &std::path::Path, candidate_id: &str) {
    let state_path = workspace.join(".a3s/evolution/state.json");
    let mut catalog: serde_json::Value =
        serde_json::from_slice(&fs::read(&state_path).expect("read evolution state"))
            .expect("parse evolution state");
    let candidate = catalog["candidates"]
        .as_array_mut()
        .and_then(|candidates| {
            candidates
                .iter_mut()
                .find(|candidate| candidate["id"] == candidate_id)
        })
        .expect("evolution candidate to update");
    candidate["instructions"]
        .as_array_mut()
        .expect("candidate instructions")
        .push(serde_json::json!(
            "Preserve concrete evidence in the final response."
        ));
    candidate["evidence"]
        .as_array_mut()
        .expect("candidate evidence")
        .push(serde_json::json!({
            "id": "preference-api-test-evidence-two",
            "memoryId": "preference-api-test-memory-two",
            "sessionId": "session-two",
            "source": "preference",
            "content": "A second session repeated the evidence-backed response preference.",
            "reason": "Repeated explicit guidance should update the local preference.",
            "timestamp": "2026-07-21T09:00:00Z",
            "importance": 0.92,
            "confidence": 0.96,
            "conflictsWith": [],
            "explicitSignal": true
        }));
    candidate["occurrences"] = serde_json::json!(2);
    candidate["distinctSessions"] = serde_json::json!(2);
    candidate["updateAvailable"] = serde_json::json!(true);
    candidate["updatedAt"] = serde_json::json!("2026-07-21T09:00:00Z");
    fs::write(
        &state_path,
        serde_json::to_vec_pretty(&catalog).expect("serialize updated evolution state"),
    )
    .expect("write updated evolution state");
}

fn http_request(address: &str, method: &str, path: &str, body: Option<&str>) -> String {
    let mut stream = TcpStream::connect(address).expect("connect to detached web process");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
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
    root: &std::path::Path,
    config_path: &std::path::Path,
    web_dir: &std::path::Path,
    state_dir: &std::path::Path,
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
        .arg(root)
        .arg("--web-dir")
        .arg(web_dir)
        .env("A3S_CONFIG_FILE", config_path)
        .env("A3S_CODE_WEB_STATE_DIR", state_dir)
        .env("HOME", root)
        .current_dir(root)
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

fn start_managed_web(
    root: &std::path::Path,
    config_path: &std::path::Path,
    web_dir: &std::path::Path,
    state_home: &std::path::Path,
    extra_args: &[&str],
) -> std::process::Output {
    let mut command = Command::new(a3s_binary());
    command.args([
        "web",
        "start",
        "--detach",
        "--host",
        "127.0.0.1",
        "--port",
        "0",
        "--web-dir",
    ]);
    command
        .arg(web_dir)
        .args(extra_args)
        .env("A3S_CONFIG_FILE", config_path)
        .env("A3S_STATE_HOME", state_home)
        .env("HOME", root)
        .current_dir(root)
        .output()
        .expect("start managed Web")
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

fn wait_until_healthy(address: &str) {
    for _ in 0..100 {
        if try_http_get(address, "/api/v1/health")
            .is_some_and(|response| response.starts_with("HTTP/1.1 200"))
        {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("Web process did not become healthy at {address}");
}

fn try_http_get(address: &str, path: &str) -> Option<String> {
    let mut stream = TcpStream::connect(address).ok()?;
    stream
        .set_read_timeout(Some(Duration::from_millis(250)))
        .ok()?;
    stream
        .set_write_timeout(Some(Duration::from_millis(250)))
        .ok()?;
    write!(
        stream,
        "GET {path} HTTP/1.1\r\nHost: {address}\r\nConnection: close\r\n\r\n"
    )
    .ok()?;
    let mut response = String::new();
    stream.read_to_string(&mut response).ok()?;
    Some(response)
}

fn available_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .expect("reserve an available port")
        .local_addr()
        .expect("available port address")
        .port()
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
        wait_until_process_exited(self.pid);
        self.active = false;
    }

    fn disarm(&mut self) {
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

#[cfg(unix)]
fn wait_until_process_exited(pid: u32) {
    let pid = pid.to_string();
    let is_running = || {
        Command::new("/bin/kill")
            .args(["-0", &pid])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    };
    let deadline = Instant::now() + Duration::from_secs(5);
    while is_running() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(25));
    }
    if !is_running() {
        return;
    }
    let _ = Command::new("/bin/kill")
        .args(["-KILL", &pid])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let forced_deadline = Instant::now() + Duration::from_secs(1);
    while is_running() && Instant::now() < forced_deadline {
        thread::sleep(Duration::from_millis(25));
    }
}

#[cfg(windows)]
fn wait_until_process_exited(_pid: u32) {}

fn temp_directory(name: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock")
        .as_nanos();
    let path = std::env::temp_dir().join(format!("a3s-{name}-{}-{stamp}", std::process::id()));
    fs::create_dir_all(&path).expect("create temporary directory");
    path
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

fn alternate_test_config() -> &'static str {
    r#"default_model = "openai/other"
providers "openai" {
  apiKey = "test"
  baseUrl = "http://127.0.0.1:1"
  models "other" {
    name = "Other"
    toolCall = true
  }
}
memory { llmExtraction = false }
"#
}
