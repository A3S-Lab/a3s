use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

use a3s_use_extension::ExtensionManifest;
use flate2::write::GzEncoder;
use flate2::Compression;
use serde_json::{json, Value};

use super::{
    configure_component_env, enroll_registry, http_json, sha256, start_web, test_config,
    wait_for_activity, wait_for_activity_absent, wait_until_stopped, TempWorkspace, TestRepository,
    TestServer, FUTURE,
};

#[test]
#[ignore = "run with `just marketplace-science-e2e`"]
fn real_marketplace_installs_uses_and_removes_packaged_science_extension() {
    let use_binary = required_file("A3S_USE_E2E_BIN");
    let use_bin = use_binary.parent().expect("A3S Use binary parent");
    let source_package = required_directory("A3S_USE_SCIENCE_E2E_PACKAGE");
    let manifest_text = fs::read_to_string(source_package.join("a3s-use-extension.acl"))
        .expect("read packaged Science manifest");
    let manifest = ExtensionManifest::parse_acl(&manifest_text).expect("parse Science manifest");
    assert_eq!(manifest.package_id, "a3s/science");
    assert_eq!(manifest.route, "science");

    let expected_html = read_text_asset(&source_package, "web/activity.html");
    let expected_css = read_text_asset(&source_package, "web/activity.css");
    let expected_js = read_text_asset(&source_package, "web/activity.js");
    let archive = archive_package(&source_package);
    let repository = TestRepository::with_package_version(archive, &manifest.version, 1, FUTURE);
    let registry_server = TestServer::start(repository.routes.clone());
    let registry_url = registry_server
        .base_url()
        .replacen("127.0.0.1", "localhost", 1);

    let temp = TempWorkspace::new("real-web-plugin-marketplace");
    let workspace = temp.path("workspace");
    let web_dir = temp.path("web");
    let config = temp.path("config/config.acl");
    let session_state = temp.path("web-session-state");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&web_dir).expect("create Web assets");
    fs::create_dir_all(config.parent().expect("config parent")).expect("create config parent");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S real plugin Marketplace integration</title>",
    )
    .expect("write Web fixture");
    fs::write(&config, test_config()).expect("write config fixture");

    let version = run_use_json(&temp, &use_binary, &["--version", "--json"]);
    assert_eq!(version["data"]["version"], manifest.version);
    enroll_registry(
        &temp,
        &config,
        use_bin,
        &registry_url,
        &repository.root_sha256,
    );

    let (mut daemon, address) = start_web(
        &temp,
        &workspace,
        &web_dir,
        &config,
        use_bin,
        &session_state,
    );
    assert_eq!(
        http_json(&address, "GET", "/api/v1/plugins/activities", None)["items"],
        json!([])
    );
    let marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    let item = science_marketplace_item(&marketplace);
    assert_eq!(item["displayName"], "科研");
    assert_eq!(item["version"], manifest.version);
    assert_eq!(item["sha256"], repository.target_sha256);
    assert_eq!(item["installed"], false);

    registry_server.clear_requests();
    let plan = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/plan",
        Some(&json!({
            "action": "install",
            "componentId": "use/a3s/science",
            "version": manifest.version,
            "channel": "stable",
        })),
    );
    assert_eq!(plan["dryRun"], true);
    assert!(registry_server
        .requests()
        .iter()
        .all(|request| !request.starts_with("/targets/")));
    let plan_digest = plan["planDigest"]
        .as_str()
        .expect("reviewed install plan digest");

    let applied = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/apply",
        Some(&json!({
            "action": "install",
            "componentId": "use/a3s/science",
            "version": manifest.version,
            "channel": "stable",
            "planDigest": plan_digest,
        })),
    );
    assert!(operation_changed(&applied));
    assert!(registry_server
        .requests()
        .iter()
        .any(|request| request.starts_with("/targets/")));

    let activities = wait_for_activity(&address, "science:research");
    let activity = activities["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["key"] == "science:research"))
        .expect("real Science Activity Bar contribution");
    assert_eq!(activity["title"], "科研");
    assert_eq!(activity["packageId"], "use/a3s/science");
    assert_eq!(activity["skill"], "a3s-use-science");
    let installed_generation = activities["generation"]
        .as_u64()
        .expect("installed registry generation");

    let content = http_json(
        &address,
        "GET",
        "/api/v1/plugins/activities/science%3Aresearch",
        None,
    );
    assert_eq!(content["html"], expected_html);
    assert_eq!(content["styles"], json!([expected_css]));
    assert_eq!(content["scripts"], json!([expected_js]));
    assert_eq!(content["sha256"], sha256(expected_html.as_bytes()));
    assert_eq!(content["skill"], "a3s-use-science");

    let status = run_use_json(
        &temp,
        &use_binary,
        &["component", "status", "a3s/science", "--json"],
    );
    let component = status
        .get("component")
        .or_else(|| status.pointer("/data/component"))
        .expect("installed Science component status");
    assert_eq!(component["presence"], "managed");
    assert_eq!(component["health"], "ready");
    assert_eq!(component["trust"], "registry-tuf");
    let installed_package = PathBuf::from(
        component["path"]
            .as_str()
            .expect("installed Science package path"),
    );
    let receipt = temp.path("state/use/extensions/a3s/science.json");
    let package_parent = temp.path("data/use/extensions/a3s/science");
    assert!(
        receipt.is_file(),
        "signed install receipt was not persisted"
    );
    assert!(installed_package.starts_with(&package_parent));
    assert_eq!(
        fs::read_to_string(installed_package.join("web/activity.html")).unwrap(),
        expected_html
    );
    assert_eq!(
        fs::read_to_string(installed_package.join("web/activity.css")).unwrap(),
        expected_css
    );
    assert_eq!(
        fs::read_to_string(installed_package.join("web/activity.js")).unwrap(),
        expected_js
    );

    let doctor = run_use_json(&temp, &use_binary, &["science", "doctor", "--json"]);
    assert_eq!(doctor["schemaVersion"], 1);
    assert_eq!(doctor["ok"], true);
    assert_eq!(doctor["data"]["sources"].as_array().map(Vec::len), Some(5));

    let mcp_tools = installed_science_mcp_tools(&temp, &use_binary);
    assert_eq!(mcp_tools.len(), 13);
    for required in [
        "science_pubmed_search",
        "science_chembl_search_molecules",
        "science_clinical_trials_search",
        "science_biorxiv_search",
        "science_ensembl_lookup_gene",
    ] {
        assert!(
            mcp_tools.iter().any(|name| name == required),
            "installed Science MCP is missing {required}: {mcp_tools:?}"
        );
    }

    let installed_marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    let installed_item = science_marketplace_item(&installed_marketplace);
    assert_eq!(installed_item["installed"], true);
    assert_eq!(installed_item["enabled"], true);

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
        .expect("reviewed uninstall plan digest");
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
    assert!(operation_changed(&uninstalled));
    wait_for_activity_absent(&address, "science:research", installed_generation);

    let removed_marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    let removed_item = science_marketplace_item(&removed_marketplace);
    assert_eq!(removed_item["installed"], false);
    assert_eq!(removed_item["enabled"], false);
    let component_list = run_use_json(&temp, &use_binary, &["component", "list", "--json"]);
    assert!(component_list["data"]["components"]
        .as_array()
        .is_some_and(|components| components
            .iter()
            .all(|component| component["id"] != "a3s/science")));
    assert!(
        !receipt.exists(),
        "uninstall left the Science receipt behind"
    );
    assert!(
        !installed_package.exists(),
        "uninstall left the installed Science generation behind"
    );
    assert!(
        !package_parent.exists(),
        "uninstall left the Science package store behind"
    );

    daemon.stop();
    wait_until_stopped(&address);
}

#[test]
#[ignore = "run with `just marketplace-science-e2e`"]
fn real_marketplace_installs_the_release_bundled_science_extension_without_a_registry() {
    let source_use_binary = required_file("A3S_USE_E2E_BIN");
    let source_package = required_directory("A3S_USE_SCIENCE_E2E_PACKAGE");
    let manifest_text = fs::read_to_string(source_package.join("a3s-use-extension.acl"))
        .expect("read packaged Science manifest");
    let manifest = ExtensionManifest::parse_acl(&manifest_text).expect("parse Science manifest");

    let temp = TempWorkspace::new("real-web-plugin-release-bundle");
    let release_root = temp.path("use-release");
    let use_binary = release_root.join("a3s-use");
    let release_package = release_root.join("extensions/a3s/science");
    fs::create_dir_all(&release_root).expect("create A3S Use release root");
    fs::copy(&source_use_binary, &use_binary).expect("copy A3S Use release binary");
    copy_directory(&source_package, &release_package);

    let catalog = run_use_json(&temp, &use_binary, &["extension", "catalog", "--json"]);
    let bundle = catalog["data"]["packages"]
        .as_array()
        .and_then(|packages| {
            packages
                .iter()
                .find(|package| package["packageId"] == "a3s/science")
        })
        .unwrap_or_else(|| panic!("Science release bundle missing: {catalog:#}"));
    assert_eq!(bundle["version"], manifest.version);
    assert_eq!(bundle["activityCount"], 1);
    let package_sha256 = bundle["packageSha256"]
        .as_str()
        .expect("release bundle digest")
        .to_string();

    let workspace = temp.path("workspace");
    let web_dir = temp.path("web");
    let config = temp.path("config/config.acl");
    let session_state = temp.path("web-session-state");
    fs::create_dir_all(&workspace).expect("create workspace");
    fs::create_dir_all(&web_dir).expect("create Web assets");
    fs::create_dir_all(config.parent().expect("config parent")).expect("create config parent");
    fs::write(
        web_dir.join("index.html"),
        "<!doctype html><title>A3S release-bundle Marketplace integration</title>",
    )
    .expect("write Web fixture");
    fs::write(&config, test_config()).expect("write config fixture");

    let (mut daemon, address) = start_web(
        &temp,
        &workspace,
        &web_dir,
        &config,
        &release_root,
        &session_state,
    );
    let marketplace = http_json(&address, "GET", "/api/v1/plugins/marketplace", None);
    let item = science_marketplace_item(&marketplace);
    assert_eq!(item["displayName"], "科研");
    assert_eq!(item["registryName"], "A3S 发行包");
    assert_eq!(item["sourceKind"], "release-bundle");
    assert_eq!(item["sha256"], package_sha256);
    assert_eq!(item["installed"], false);
    assert!(marketplace["registries"].as_array().is_some_and(|sources| {
        sources
            .iter()
            .any(|source| source["sourceKind"] == "release-bundle" && source["verified"] == true)
    }));

    let plan = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/plan",
        Some(&json!({
            "action": "install",
            "componentId": "use/a3s/science",
            "version": manifest.version,
            "channel": "stable",
        })),
    );
    assert_eq!(plan["dryRun"], true);
    assert_eq!(plan["plans"][0]["source"], "release-bundle:a3s-use");
    assert_eq!(
        plan["plans"][0]["resolvedReleaseBundles"]["use/a3s/science"]["packageSha256"],
        package_sha256
    );
    let plan_digest = plan["planDigest"]
        .as_str()
        .expect("reviewed bundle install plan digest");
    let applied = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/apply",
        Some(&json!({
            "action": "install",
            "componentId": "use/a3s/science",
            "version": manifest.version,
            "channel": "stable",
            "planDigest": plan_digest,
        })),
    );
    assert!(operation_changed(&applied));

    let activities = wait_for_activity(&address, "science:research");
    let installed_generation = activities["generation"]
        .as_u64()
        .expect("installed registry generation");
    let activity = activities["items"]
        .as_array()
        .and_then(|items| items.iter().find(|item| item["key"] == "science:research"))
        .expect("release-bundled Science activity");
    assert_eq!(activity["title"], "科研");
    let content = http_json(
        &address,
        "GET",
        "/api/v1/plugins/activities/science%3Aresearch",
        None,
    );
    assert_eq!(
        content["html"],
        read_text_asset(&source_package, "web/activity.html")
    );

    let status = run_use_json(
        &temp,
        &use_binary,
        &["component", "status", "a3s/science", "--json"],
    );
    let component = status
        .get("component")
        .or_else(|| status.pointer("/data/component"))
        .expect("release-bundled Science status");
    assert_eq!(component["trust"], "release-bundle");
    assert_eq!(component["packageSha256"], package_sha256);
    assert_eq!(component["health"], "ready");

    let uninstall_plan = http_json(
        &address,
        "POST",
        "/api/v1/plugins/operations/plan",
        Some(&json!({
            "action": "uninstall",
            "componentId": "use/a3s/science",
        })),
    );
    let uninstall_digest = uninstall_plan["planDigest"]
        .as_str()
        .expect("reviewed bundle uninstall digest");
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
    assert!(operation_changed(&uninstalled));
    wait_for_activity_absent(&address, "science:research", installed_generation);

    daemon.stop();
    wait_until_stopped(&address);
}

fn required_file(name: &str) -> PathBuf {
    let path = required_path(name);
    assert!(path.is_file(), "{name} is not a file: {}", path.display());
    path
}

fn required_directory(name: &str) -> PathBuf {
    let path = required_path(name);
    assert!(
        path.is_dir(),
        "{name} is not a directory: {}",
        path.display()
    );
    path
}

fn required_path(name: &str) -> PathBuf {
    let value = std::env::var_os(name).unwrap_or_else(|| {
        panic!("{name} is required; run this ignored test through `just marketplace-science-e2e`")
    });
    let path = PathBuf::from(value);
    assert!(path.is_absolute(), "{name} must be an absolute path");
    path
}

fn read_text_asset(package: &Path, relative: &str) -> String {
    fs::read_to_string(package.join(relative))
        .unwrap_or_else(|error| panic!("failed to read packaged {relative}: {error}"))
}

fn archive_package(package: &Path) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let encoder = GzEncoder::new(&mut bytes, Compression::default());
        let mut archive = tar::Builder::new(encoder);
        archive
            .append_dir_all("package", package)
            .expect("archive packaged Science extension");
        archive.finish().expect("finish Science package archive");
        let encoder = archive.into_inner().expect("finish Science tar stream");
        encoder.finish().expect("finish Science gzip stream");
    }
    bytes
}

fn copy_directory(source: &Path, destination: &Path) {
    fs::create_dir_all(destination)
        .unwrap_or_else(|error| panic!("create {}: {error}", destination.display()));
    for entry in
        fs::read_dir(source).unwrap_or_else(|error| panic!("read {}: {error}", source.display()))
    {
        let entry = entry.expect("read package entry");
        let source_path = entry.path();
        let destination_path = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(&source_path).expect("inspect package entry");
        assert!(
            !metadata.file_type().is_symlink(),
            "package contains a link"
        );
        if metadata.is_dir() {
            copy_directory(&source_path, &destination_path);
        } else {
            fs::copy(&source_path, &destination_path)
                .unwrap_or_else(|error| panic!("copy {}: {error}", source_path.display()));
        }
    }
}

fn science_marketplace_item(marketplace: &Value) -> &Value {
    marketplace["items"]
        .as_array()
        .and_then(|items| {
            items
                .iter()
                .find(|item| item["componentId"] == "use/a3s/science")
        })
        .unwrap_or_else(|| panic!("signed Science Marketplace item: {marketplace:#}"))
}

fn operation_changed(result: &Value) -> bool {
    result["operations"].as_array().is_some_and(|operations| {
        operations
            .iter()
            .any(|operation| operation["changed"] == true)
    })
}

fn run_use_json(temp: &TempWorkspace, use_binary: &Path, args: &[&str]) -> Value {
    let mut command = Command::new(use_binary);
    configure_component_env(&mut command, temp);
    let output = command
        .args(args)
        .env_remove("A3S_USE_HOME")
        .output()
        .unwrap_or_else(|error| panic!("failed to run {}: {error}", use_binary.display()));
    assert!(
        output.status.success(),
        "{} {} failed:\nstdout:\n{}\nstderr:\n{}",
        use_binary.display(),
        args.join(" "),
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "{} {} returned invalid JSON ({error}): {}",
            use_binary.display(),
            args.join(" "),
            String::from_utf8_lossy(&output.stdout)
        )
    })
}

fn installed_science_mcp_tools(temp: &TempWorkspace, use_binary: &Path) -> Vec<String> {
    let mut command = Command::new(use_binary);
    configure_component_env(&mut command, temp);
    let child = command
        .args(["mcp", "serve", "a3s/science"])
        .env_remove("A3S_USE_HOME")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("launch installed Science MCP server");
    let mut child = ChildGuard(child);
    let mut stdin = child.0.stdin.take().expect("Science MCP stdin");
    let stdout = child.0.stdout.take().expect("Science MCP stdout");
    let mut reader = BufReader::new(stdout);

    write_mcp_message(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": {"name": "a3s-marketplace-e2e", "version": "1"}
            }
        }),
    );
    let initialized = read_mcp_response(&mut reader, 1);
    assert_eq!(
        initialized["result"]["serverInfo"]["name"],
        "a3s-use-science"
    );

    write_mcp_message(
        &mut stdin,
        json!({"jsonrpc": "2.0", "method": "notifications/initialized", "params": {}}),
    );
    write_mcp_message(
        &mut stdin,
        json!({"jsonrpc": "2.0", "id": 2, "method": "tools/list", "params": {}}),
    );
    let tools = read_mcp_response(&mut reader, 2);
    tools["result"]["tools"]
        .as_array()
        .expect("Science MCP tools array")
        .iter()
        .map(|tool| {
            tool["name"]
                .as_str()
                .expect("Science MCP tool name")
                .to_string()
        })
        .collect()
}

fn write_mcp_message(stdin: &mut impl Write, value: Value) {
    serde_json::to_writer(&mut *stdin, &value).expect("write Science MCP message");
    stdin
        .write_all(b"\n")
        .expect("terminate Science MCP message");
    stdin.flush().expect("flush Science MCP message");
}

fn read_mcp_response(reader: &mut impl BufRead, id: u64) -> Value {
    for _ in 0..20 {
        let mut line = String::new();
        let bytes = reader
            .read_line(&mut line)
            .expect("read Science MCP response");
        assert!(
            bytes > 0,
            "Science MCP closed before responding to request {id}"
        );
        let value: Value = serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("invalid Science MCP response ({error}): {line}"));
        if value["id"].as_u64() == Some(id) {
            return value;
        }
    }
    panic!("Science MCP did not respond to request {id}");
}

struct ChildGuard(Child);

impl Drop for ChildGuard {
    fn drop(&mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}
