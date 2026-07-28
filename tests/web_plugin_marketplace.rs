#![cfg(unix)]

mod support;

use std::fs;
use std::io::{Read, Write};
use std::net::TcpStream;
use std::path::Path;
use std::process::Command;
use std::thread;
use std::time::Duration;

use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use support::{a3s_bin, configure_component_env, make_executable, sh_quote, TempWorkspace};

#[path = "support/tuf_test_support.rs"]
mod tuf_test_support;

use tuf_test_support::{extension_archive, TestRepository, TestServer, FUTURE, PACKAGE_VERSION};

#[path = "web_plugin_marketplace/real_e2e.rs"]
mod real_e2e;

#[test]
fn marketplace_install_hot_plugs_a_verified_activity_and_skill() {
    let temp = TempWorkspace::new("web-plugin-marketplace");
    let workspace = temp.path("workspace");
    let web_dir = temp.path("web");
    let config = temp.path("config/config.acl");
    let use_bin = temp.path("use-bin");
    let package_root = temp.path("managed-package");
    let installed_marker = temp.path("installed");
    let session_state = temp.path("web-session-state");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&web_dir).expect("create Web assets");
    fs::create_dir_all(config.parent().expect("config parent")).expect("create config parent");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S plugin Marketplace integration</title>",
    )
    .expect("write Web fixture");
    fs::write(&config, test_config()).expect("write config fixture");

    let activity_html =
        "<!doctype html><title>Science</title><main>Installed Marketplace Activity</main>";
    let activity_css = "main { color: rebeccapurple; }";
    let activity_js =
        "window.parent.postMessage({ protocol: 'a3s.activity.v1', type: 'activity.ready' }, '*');";
    let skill = "---\nname: science\ndescription: Use the installed Science extension.\n---\n# Science\n\nUse the verified Science capability.\n";
    let activity_path = package_root.join("web/activity.html");
    let activity_style_path = package_root.join("web/activity.css");
    let activity_script_path = package_root.join("web/activity.js");
    let skill_path = package_root.join("skills/science/SKILL.md");
    fs::create_dir_all(activity_path.parent().expect("activity parent"))
        .expect("create activity directory");
    fs::create_dir_all(skill_path.parent().expect("skill parent")).expect("create skill directory");
    fs::write(&activity_path, activity_html).expect("write activity asset");
    fs::write(&activity_style_path, activity_css).expect("write activity style");
    fs::write(&activity_script_path, activity_js).expect("write activity script");
    fs::write(&skill_path, skill).expect("write Skill asset");

    let empty_snapshot = snapshot_envelope(1, "1", Vec::new());
    let installed_capability = json!({
        "id": "use/a3s/science",
        "route": "science",
        "version": PACKAGE_VERSION,
        "origin": "extension",
        "enabled": true,
        "readiness": "ready",
        "packageRoot": package_root,
        "surfaces": ["skill"],
        "skills": [{
            "path": skill_path,
            "sha256": sha256(skill.as_bytes()),
        }],
        "activityBar": [{
            "id": "research",
            "title": "科研",
            "description": "Prepare evidence-backed research with the installed Science capability.",
            "icon": "flask-conical",
            "entry": {
                "path": activity_path,
                "sha256": sha256(activity_html.as_bytes()),
                "mediaType": "text/html",
            },
            "styles": [{
                "path": activity_style_path,
                "sha256": sha256(activity_css.as_bytes()),
                "mediaType": "text/css",
            }],
            "scripts": [{
                "path": activity_script_path,
                "sha256": sha256(activity_js.as_bytes()),
                "mediaType": "text/javascript",
            }],
            "skill": "science",
            "order": 80,
        }],
    });
    let installed_snapshot = snapshot_envelope(2, "2", vec![installed_capability]);
    let empty_snapshot_path = temp.path("empty-snapshot.json");
    let installed_snapshot_path = temp.path("installed-snapshot.json");
    let changed_snapshot_path = temp.path("changed-snapshot.json");
    let unchanged_snapshot_path = temp.path("unchanged-snapshot.json");
    let removed_snapshot_path = temp.path("removed-snapshot.json");
    fs::write(
        &empty_snapshot_path,
        serde_json::to_vec(&empty_snapshot).unwrap(),
    )
    .expect("write empty snapshot");
    fs::write(
        &installed_snapshot_path,
        serde_json::to_vec(&installed_snapshot).unwrap(),
    )
    .expect("write installed snapshot");
    fs::write(
        &changed_snapshot_path,
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "ok": true,
            "data": {"changed": true, "registry": installed_snapshot["data"]["registry"]},
        }))
        .unwrap(),
    )
    .expect("write changed snapshot");
    fs::write(
        &unchanged_snapshot_path,
        br#"{"schemaVersion":1,"ok":true,"data":{"changed":false}}"#,
    )
    .expect("write unchanged snapshot");
    fs::write(
        &removed_snapshot_path,
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "ok": true,
            "data": {
                "changed": true,
                "registry": snapshot_envelope(3, "3", Vec::new())["data"]["registry"],
            },
        }))
        .unwrap(),
    )
    .expect("write removed snapshot");
    make_use_fixture(
        &use_bin,
        &installed_marker,
        &package_root,
        UseFixtureSnapshots {
            empty: &empty_snapshot_path,
            installed: &installed_snapshot_path,
            changed: &changed_snapshot_path,
            removed: &removed_snapshot_path,
            unchanged: &unchanged_snapshot_path,
        },
    );

    let repository = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
    let registry_server = TestServer::start(repository.routes.clone());
    let registry_url = registry_server
        .base_url()
        .replacen("127.0.0.1", "localhost", 1);
    enroll_registry(
        &temp,
        &config,
        &use_bin,
        &registry_url,
        &repository.root_sha256,
    );

    let (mut daemon, address) = start_web(
        &temp,
        &workspace,
        &web_dir,
        &config,
        &use_bin,
        &session_state,
    );

    let initial_activities = http_json(&address, "GET", "/api/v1/plugins/activities", None);
    assert_eq!(initial_activities["items"], json!([]));

    let marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    let item = marketplace["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["componentId"] == "use/a3s/science")
        })
        .unwrap_or_else(|| panic!("signed Marketplace package: {marketplace:#}"));
    assert_eq!(item["installed"], false);
    assert_eq!(item["displayName"], "科研");
    assert_eq!(item["sha256"], repository.target_sha256);

    let plan = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/plan",
        Some(&json!({
            "action": "install",
            "componentId": "use/a3s/science",
            "version": PACKAGE_VERSION,
            "channel": "stable",
        })),
    );
    assert_eq!(plan["dryRun"], true);
    let digest = plan["planDigest"]
        .as_str()
        .expect("reviewed plan digest")
        .to_string();
    assert!(
        !installed_marker.exists(),
        "planning must not install the package"
    );

    let applied = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/apply",
        Some(&json!({
            "action": "install",
            "componentId": "use/a3s/science",
            "version": PACKAGE_VERSION,
            "channel": "stable",
            "planDigest": digest,
        })),
    );
    assert!(applied["operations"]
        .as_array()
        .is_some_and(|operations| operations
            .iter()
            .any(|operation| operation["changed"] == true)));
    assert!(installed_marker.is_file());

    let activities = wait_for_activity(&address, "science:research");
    let activity = activities["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["key"] == "science:research"))
        .expect("hot-plugged Activity Bar contribution");
    assert_eq!(activity["packageId"], "use/a3s/science");
    assert_eq!(activity["skill"], "science");
    assert_eq!(activity["enabled"], true);
    let installed_generation = activities["generation"]
        .as_u64()
        .expect("installed registry generation");

    let content = http_json(
        &address,
        "GET",
        "/api/v1/plugins/activities/science%3Aresearch",
        None,
    );
    assert_eq!(content["html"], activity_html);
    assert_eq!(content["styles"], json!([activity_css]));
    assert_eq!(content["scripts"], json!([activity_js]));
    assert_eq!(content["sha256"], sha256(activity_html.as_bytes()));
    assert_eq!(content["skill"], "science");

    let installed_marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    assert!(installed_marketplace["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["componentId"] == "use/a3s/science")
        })
        .is_some_and(|item| item["installed"] == true && item["enabled"] == true));

    let uninstall_plan = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/plan",
        Some(&json!({
            "action": "uninstall",
            "componentId": "use/a3s/science",
        })),
    );
    assert_eq!(uninstall_plan["dryRun"], true);
    let uninstall_digest = uninstall_plan["planDigest"]
        .as_str()
        .expect("reviewed uninstall digest");
    let uninstalled = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/apply",
        Some(&json!({
            "action": "uninstall",
            "componentId": "use/a3s/science",
            "planDigest": uninstall_digest,
        })),
    );
    assert!(uninstalled["operations"]
        .as_array()
        .is_some_and(|operations| operations
            .iter()
            .any(|operation| operation["changed"] == true)));
    wait_for_activity_absent(&address, "science:research", installed_generation);
    let removed_marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    assert!(removed_marketplace["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["componentId"] == "use/a3s/science")
        })
        .is_some_and(|item| item["installed"] == false && item["enabled"] == false));

    daemon.stop();
    wait_until_stopped(&address);
}

fn snapshot_envelope(generation: u64, revision_digit: &str, capabilities: Vec<Value>) -> Value {
    json!({
        "schemaVersion": 1,
        "ok": true,
        "data": {
            "registry": {
                "schemaVersion": 1,
                "generation": generation,
                "revision": revision_digit.repeat(64),
                "capabilities": capabilities,
            }
        }
    })
}

struct UseFixtureSnapshots<'a> {
    empty: &'a Path,
    installed: &'a Path,
    changed: &'a Path,
    removed: &'a Path,
    unchanged: &'a Path,
}

fn make_use_fixture(
    directory: &Path,
    installed_marker: &Path,
    package_root: &Path,
    snapshots: UseFixtureSnapshots<'_>,
) {
    let installed_components = directory.join("installed-components.json");
    fs::create_dir_all(directory).expect("create A3S Use fixture directory");
    fs::write(
        &installed_components,
        serde_json::to_vec(&json!({
            "schemaVersion": 1,
            "ok": true,
            "data": {
                "components": [{
                    "id": "a3s/science",
                    "description": "Installed signed Science extension",
                    "presence": "managed",
                    "health": "ready",
                    "version": PACKAGE_VERSION,
                    "path": package_root,
                    "trust": "registry-tuf",
                }],
            },
        }))
        .expect("serialize installed component list"),
    )
    .expect("write installed component list");
    make_executable(
        &directory.join("a3s-use"),
        &format!(
            r#"#!/bin/sh
if [ "$1" = "--version" ]; then printf 'a3s-use 0.1.2\n'; exit 0; fi
if [ "$1" = "capability" ] && [ "$2" = "snapshot" ]; then
  state=$(/bin/cat {marker} 2>/dev/null || true)
  if [ "$state" = "installed" ]; then /bin/cat {installed}; elif [ "$state" = "removed" ]; then /bin/cat {removed}; else /bin/cat {empty}; fi
  exit 0
fi
if [ "$1" = "capability" ] && [ "$2" = "watch" ]; then
  state=$(/bin/cat {marker} 2>/dev/null || true)
  if [ "$state" = "installed" ] && [ "$4" = "1" ]; then /bin/cat {changed}; elif [ "$state" = "removed" ] && [ "$4" = "2" ]; then /bin/cat {removed}; else /bin/sleep 0.05; /bin/cat {unchanged}; fi
  exit 0
fi
if [ "$1" = "component" ] && [ "$2" = "list" ]; then
  if [ "$(/bin/cat {marker} 2>/dev/null || true)" = "installed" ]; then
    /bin/cat {installed_components}
  else
    printf '{{"schemaVersion":1,"ok":true,"data":{{"components":[]}}}}\n'
  fi
  exit 0
fi
if [ "$1" = "component" ] && [ "$2" = "status" ]; then
  if [ "$(/bin/cat {marker} 2>/dev/null || true)" = "installed" ]; then
    printf '{{"schemaVersion":1,"ok":true,"data":{{"component":{{"id":"%s","presence":"managed","health":"ready","version":"{version}","trust":"registry-tuf"}}}}}}\n' "$3"
  else
    printf '{{"schemaVersion":1,"ok":true,"data":{{"component":{{"id":"%s","presence":"missing","health":"unknown"}}}}}}\n' "$3"
  fi
  exit 0
fi
if [ "$1" = "component" ] && [ "$2" = "install" ]; then
  printf 'installed\n' > {marker}
  printf '{{"schemaVersion":1,"ok":true,"data":{{"changed":true,"component":{{"id":"%s","version":"{version}","trust":"registry-tuf"}}}}}}\n' "$3"
  exit 0
fi
if [ "$1" = "component" ] && [ "$2" = "uninstall" ]; then
  printf 'removed\n' > {marker}
  printf '{{"schemaVersion":1,"ok":true,"data":{{"changed":true,"component":"%s"}}}}\n' "$3"
  exit 0
fi
exit 2
"#,
            marker = sh_quote(installed_marker),
            installed_components = sh_quote(&installed_components),
            installed = sh_quote(snapshots.installed),
            empty = sh_quote(snapshots.empty),
            changed = sh_quote(snapshots.changed),
            removed = sh_quote(snapshots.removed),
            unchanged = sh_quote(snapshots.unchanged),
            version = PACKAGE_VERSION,
        ),
    );
}

fn enroll_registry(
    temp: &TempWorkspace,
    config: &Path,
    use_bin: &Path,
    url: &str,
    root_sha256: &str,
) {
    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, temp);
    let output = command
        .arg("--config")
        .arg(config)
        .args([
            "--output",
            "json",
            "registry",
            "add",
            url,
            "--trust-root",
            &format!("sha256:{root_sha256}"),
            "--yes",
        ])
        .env("A3S_USE_INSTALL_DIR", use_bin)
        .env_remove("A3S_USE_HOME")
        .output()
        .expect("enroll signed registry");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn start_web(
    temp: &TempWorkspace,
    workspace: &Path,
    web_dir: &Path,
    config: &Path,
    use_bin: &Path,
    session_state: &Path,
) -> (DaemonGuard, String) {
    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, temp);
    let output = command
        .arg("--config")
        .arg(config)
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
        .env("A3S_USE_INSTALL_DIR", use_bin)
        .env("A3S_CODE_WEB_STATE_DIR", session_state)
        .env_remove("A3S_USE_HOME")
        .current_dir(workspace)
        .output()
        .expect("start detached Web");
    assert!(
        output.status.success(),
        "stdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    let pid = output_value(&stdout, "Background PID:")
        .parse::<u32>()
        .expect("Web PID");
    let address = output_value(&stdout, "A3S Web:")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .to_string();
    (DaemonGuard::new(pid), address)
}

fn wait_for_activity(address: &str, key: &str) -> Value {
    for _ in 0..100 {
        let catalog = http_json(address, "GET", "/api/v1/plugins/activities", None);
        if catalog["items"]
            .as_array()
            .is_some_and(|items| items.iter().any(|item| item["key"] == key))
        {
            return catalog;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("Activity Bar contribution '{key}' did not hot-plug");
}

fn wait_for_activity_absent(address: &str, key: &str, after_generation: u64) {
    for _ in 0..200 {
        let catalog = http_json(address, "GET", "/api/v1/plugins/activities", None);
        if catalog["items"]
            .as_array()
            .is_some_and(|items| items.iter().all(|item| item["key"] != key))
            && catalog["generation"]
                .as_u64()
                .is_some_and(|generation| generation > after_generation)
        {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("Activity Bar contribution '{key}' did not disappear after uninstall");
}

fn http_json(address: &str, method: &str, path: &str, body: Option<&Value>) -> Value {
    let body = body.map(Value::to_string).unwrap_or_default();
    let mut stream = TcpStream::connect(address).expect("connect to Web API");
    stream
        .set_read_timeout(Some(Duration::from_secs(30)))
        .expect("set response timeout");
    write!(
        stream,
        "{method} {path} HTTP/1.1\r\nHost: {address}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    )
    .expect("write Web API request");
    let mut response = String::new();
    stream
        .read_to_string(&mut response)
        .expect("read Web API response");
    assert!(
        response.starts_with("HTTP/1.1 200"),
        "{method} {path} returned an unexpected response:\n{response}"
    );
    let (_, body) = response
        .split_once("\r\n\r\n")
        .unwrap_or_else(|| panic!("HTTP response has no body: {response}"));
    serde_json::from_str(body).unwrap_or_else(|error| panic!("invalid JSON ({error}): {body}"))
}

fn output_value<'a>(output: &'a str, prefix: &str) -> &'a str {
    output
        .lines()
        .find_map(|line| line.strip_prefix(prefix).map(str::trim))
        .unwrap_or_else(|| panic!("missing '{prefix}' in output:\n{output}"))
}

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
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
        let _ = Command::new("kill")
            .args(["-INT", &self.pid.to_string()])
            .status();
        self.active = false;
    }
}

impl Drop for DaemonGuard {
    fn drop(&mut self) {
        self.stop();
    }
}

fn wait_until_stopped(address: &str) {
    for _ in 0..100 {
        if TcpStream::connect(address).is_err() {
            return;
        }
        thread::sleep(Duration::from_millis(50));
    }
    panic!("Web process still listens on {address}");
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
