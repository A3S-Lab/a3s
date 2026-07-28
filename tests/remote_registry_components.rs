#![cfg(unix)]

mod support;

use std::process::{Command, Output};

use a3s_use_extension::ResolvedRemotePackage;
use support::{a3s_bin, configure_component_env, make_executable, sh_quote, TempWorkspace};

#[path = "support/tuf_test_support.rs"]
mod tuf_test_support;

use tuf_test_support::{
    extension_archive, TestRepository, TestServer, EXPIRED, FUTURE, PACKAGE_VERSION,
};

#[test]
fn signed_registry_plan_is_bound_before_delegating_to_use() {
    let temp = TempWorkspace::new("signed-registry-install");
    let version_one = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
    let server = TestServer::start(version_one.routes.clone());
    let registry_url = localhost_url(&server);
    let config = temp.path("config/config.acl");
    let use_bin = temp.path("use-bin");
    let install_log = temp.path("remote-install.log");
    make_use_fixture(&use_bin, &install_log);
    add_registry(
        &temp,
        &config,
        &use_bin,
        &registry_url,
        &format!("sha256:{}", version_one.root_sha256),
    );

    server.clear_requests();
    let refreshed = run(
        &temp,
        &config,
        &use_bin,
        &["registry", "refresh", "localhost"],
    );
    assert!(refreshed.status.success(), "{refreshed:?}");
    let refreshed = json(&refreshed);
    assert_eq!(
        refreshed["data"]["registries"][0]["metadata"]["targetsVersion"],
        1
    );
    assert_no_target_request(&server);

    server.clear_requests();
    let first_plan = run(
        &temp,
        &config,
        &use_bin,
        &["install", "use/a3s/science", "--dry-run"],
    );
    assert!(first_plan.status.success(), "{first_plan:?}");
    let first_plan = json(&first_plan);
    let first_digest = first_plan["data"]["planDigest"]
        .as_str()
        .unwrap()
        .to_string();
    let first_package =
        &first_plan["data"]["plans"][0]["resolvedRegistryPackages"]["use/a3s/science"];
    assert_eq!(first_package["registryName"], "localhost");
    assert_eq!(first_package["targetsVersion"], 1);
    assert_eq!(first_package["sha256"], version_one.target_sha256);
    assert_no_target_request(&server);
    assert!(!install_log.exists());

    let version_two = TestRepository::new(extension_archive(PACKAGE_VERSION), 2, FUTURE);
    assert_eq!(version_one.root_sha256, version_two.root_sha256);
    server.replace_routes(version_two.routes.clone());
    server.clear_requests();
    let stale = run(
        &temp,
        &config,
        &use_bin,
        &["install", "use/a3s/science", "--plan-digest", &first_digest],
    );
    assert!(!stale.status.success(), "{stale:?}");
    assert_eq!(json(&stale)["error"]["code"], "component.plan_mismatch");
    assert_no_target_request(&server);
    assert!(!install_log.exists());

    let current_plan = run(
        &temp,
        &config,
        &use_bin,
        &["install", "use/a3s/science", "--dry-run"],
    );
    assert!(current_plan.status.success(), "{current_plan:?}");
    let current_plan = json(&current_plan);
    let current_digest = current_plan["data"]["planDigest"].as_str().unwrap();
    let package: ResolvedRemotePackage = serde_json::from_value(
        current_plan["data"]["plans"][0]["resolvedRegistryPackages"]["use/a3s/science"].clone(),
    )
    .unwrap();
    assert_eq!(package.targets_version, 2);
    let registry_plan_digest = package.plan_digest().unwrap();

    server.clear_requests();
    let applied = run(
        &temp,
        &config,
        &use_bin,
        &[
            "install",
            "use/a3s/science",
            "--plan-digest",
            current_digest,
        ],
    );
    assert!(applied.status.success(), "{applied:?}");
    assert_eq!(json(&applied)["data"]["planDigest"], current_digest);
    assert_no_target_request(&server);
    let arguments = std::fs::read_to_string(&install_log).unwrap();
    assert!(arguments.contains("component\ninstall\na3s/science\n--json\n"));
    assert!(arguments.contains("--registry-name\nlocalhost\n"));
    assert!(arguments.contains(&format!("--registry-url\n{registry_url}\n")));
    assert!(arguments.contains(&format!(
        "--trust-root\nsha256:{}\n",
        version_two.root_sha256
    )));
    assert!(arguments.contains(&format!("--registry-plan-digest\n{registry_plan_digest}\n")));
    assert!(arguments.contains(&format!("--version\n{PACKAGE_VERSION}\n")));
    assert!(arguments.contains("--channel\nstable\n"));
    assert!(!arguments.contains("--allow-unsigned"));

    server.clear_requests();
    std::fs::remove_file(&install_log).unwrap();
    let unsigned = run(
        &temp,
        &config,
        &use_bin,
        &["install", "use/a3s/science", "--allow-unsigned"],
    );
    assert!(!unsigned.status.success(), "{unsigned:?}");
    assert!(json(&unsigned)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("explicit local --from package"));
    assert!(server.requests().is_empty());
    assert!(!install_log.exists());
}

#[test]
fn signed_registry_upgrade_restores_the_recorded_source_and_binds_the_new_target() {
    const NEXT_VERSION: &str = "0.2.0";

    let temp = TempWorkspace::new("signed-registry-upgrade");
    let version_one = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
    let server = TestServer::start(version_one.routes.clone());
    let registry_url = localhost_url(&server);
    let config = temp.path("config/config.acl");
    let use_bin = temp.path("use-bin");
    let install_log = temp.path("remote-upgrade.log");
    make_use_fixture(&use_bin, &install_log);
    add_registry(
        &temp,
        &config,
        &use_bin,
        &registry_url,
        &format!("sha256:{}", version_one.root_sha256),
    );

    let initial = run(
        &temp,
        &config,
        &use_bin,
        &["install", "use/a3s/science", "--dry-run"],
    );
    assert!(initial.status.success(), "{initial:?}");
    let initial = json(&initial);
    let installed: ResolvedRemotePackage = serde_json::from_value(
        initial["data"]["plans"][0]["resolvedRegistryPackages"]["use/a3s/science"].clone(),
    )
    .unwrap();
    make_installed_use_fixture(&use_bin, &install_log, &installed);

    let version_two = TestRepository::with_package_version(
        extension_archive(NEXT_VERSION),
        NEXT_VERSION,
        2,
        FUTURE,
    );
    assert_eq!(version_one.root_sha256, version_two.root_sha256);
    server.replace_routes(version_two.routes.clone());
    server.clear_requests();
    let available = run(&temp, &config, &use_bin, &["upgrade"]);
    assert!(available.status.success(), "{available:?}");
    let available = json(&available);
    let components = available["data"]["components"].as_array().unwrap();
    let science = components
        .iter()
        .find(|component| component["id"] == "use/a3s/science")
        .expect("signed extension should be listed as upgradeable");
    assert_eq!(science["update"], "available");
    assert_no_target_request(&server);

    server.clear_requests();
    let all_plan = run(&temp, &config, &use_bin, &["upgrade", "--all", "--dry-run"]);
    assert!(all_plan.status.success(), "{all_plan:?}");
    let all_plan = json(&all_plan);
    assert_eq!(all_plan["data"]["plans"].as_array().unwrap().len(), 1);
    assert_eq!(all_plan["data"]["plans"][0]["component"], "use/a3s/science");
    assert_no_target_request(&server);

    server.clear_requests();
    let first_plan = run(
        &temp,
        &config,
        &use_bin,
        &["upgrade", "use/a3s/science", "--dry-run"],
    );
    assert!(first_plan.status.success(), "{first_plan:?}");
    let first_plan = json(&first_plan);
    let first_digest = first_plan["data"]["planDigest"]
        .as_str()
        .unwrap()
        .to_string();
    let operation = &first_plan["data"]["plans"][0];
    assert_eq!(operation["action"], "upgrade");
    assert_eq!(operation["source"], "registry:localhost");
    assert_eq!(operation["channel"], "stable");
    assert_eq!(operation["mutates"], true);
    assert_eq!(operation["force"], true);
    assert_eq!(
        operation["resolvedRegistryPackages"]["use/a3s/science"]["version"],
        NEXT_VERSION
    );
    assert_no_target_request(&server);
    assert!(!install_log.exists());

    let version_three = TestRepository::with_package_version(
        extension_archive(NEXT_VERSION),
        NEXT_VERSION,
        3,
        FUTURE,
    );
    server.replace_routes(version_three.routes.clone());
    server.clear_requests();
    let stale = run(
        &temp,
        &config,
        &use_bin,
        &["upgrade", "use/a3s/science", "--plan-digest", &first_digest],
    );
    assert!(!stale.status.success(), "{stale:?}");
    assert_eq!(json(&stale)["error"]["code"], "component.plan_mismatch");
    assert_no_target_request(&server);
    assert!(!install_log.exists());

    let current = run(
        &temp,
        &config,
        &use_bin,
        &["upgrade", "use/a3s/science", "--dry-run"],
    );
    assert!(current.status.success(), "{current:?}");
    let current = json(&current);
    let current_digest = current["data"]["planDigest"].as_str().unwrap();
    let package: ResolvedRemotePackage = serde_json::from_value(
        current["data"]["plans"][0]["resolvedRegistryPackages"]["use/a3s/science"].clone(),
    )
    .unwrap();
    let registry_plan_digest = package.plan_digest().unwrap();

    server.clear_requests();
    let applied = run(
        &temp,
        &config,
        &use_bin,
        &[
            "upgrade",
            "use/a3s/science",
            "--plan-digest",
            current_digest,
        ],
    );
    assert!(applied.status.success(), "{applied:?}");
    assert_no_target_request(&server);
    let arguments = std::fs::read_to_string(&install_log).unwrap();
    assert!(arguments.contains("component\ninstall\na3s/science\n--json\n"));
    assert!(arguments.contains("--force\n"));
    assert!(arguments.contains("--registry-name\nlocalhost\n"));
    assert!(arguments.contains(&format!("--registry-url\n{registry_url}\n")));
    assert!(arguments.contains(&format!("--version\n{NEXT_VERSION}\n")));
    assert!(arguments.contains("--channel\nstable\n"));
    assert!(arguments.contains(&format!("--registry-plan-digest\n{registry_plan_digest}\n")));
    assert!(!arguments.contains("--allow-unsigned"));

    make_installed_use_fixture(&use_bin, &install_log, &package);
    std::fs::remove_file(&install_log).unwrap();
    server.clear_requests();
    let converged = run(
        &temp,
        &config,
        &use_bin,
        &["upgrade", "use/a3s/science", "--dry-run"],
    );
    assert!(converged.status.success(), "{converged:?}");
    let converged = json(&converged);
    assert_eq!(converged["data"]["plans"][0]["mutates"], false);
    assert_eq!(converged["data"]["plans"][0]["force"], false);
    assert_no_target_request(&server);
    assert!(!install_log.exists());

    let downgrade = TestRepository::with_package_version(
        extension_archive(PACKAGE_VERSION),
        PACKAGE_VERSION,
        4,
        FUTURE,
    );
    server.replace_routes(downgrade.routes.clone());
    server.clear_requests();
    let rejected = run(
        &temp,
        &config,
        &use_bin,
        &["upgrade", "use/a3s/science", "--dry-run"],
    );
    assert!(!rejected.status.success(), "{rejected:?}");
    assert!(json(&rejected)["error"]["message"]
        .as_str()
        .unwrap()
        .contains("attempted to downgrade"));
    assert_no_target_request(&server);
    assert!(!install_log.exists());
}

#[test]
fn registry_refresh_rejects_wrong_roots_and_expired_metadata() {
    let wrong_temp = TempWorkspace::new("registry-wrong-root");
    let repository = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
    let server = TestServer::start(repository.routes.clone());
    let registry_url = localhost_url(&server);
    let config = wrong_temp.path("config/config.acl");
    let use_bin = wrong_temp.path("use-bin");
    make_use_fixture(&use_bin, &wrong_temp.path("unused.log"));
    add_registry(
        &wrong_temp,
        &config,
        &use_bin,
        &registry_url,
        &format!("sha256:{}", "f".repeat(64)),
    );
    server.clear_requests();
    let wrong = run(
        &wrong_temp,
        &config,
        &use_bin,
        &["registry", "refresh", "localhost"],
    );
    assert!(!wrong.status.success(), "{wrong:?}");
    assert_no_target_request(&server);

    let expired_temp = TempWorkspace::new("registry-expired");
    let expired = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, EXPIRED);
    let expired_server = TestServer::start(expired.routes.clone());
    let expired_url = localhost_url(&expired_server);
    let expired_config = expired_temp.path("config/config.acl");
    let expired_use_bin = expired_temp.path("use-bin");
    make_use_fixture(&expired_use_bin, &expired_temp.path("unused.log"));
    add_registry(
        &expired_temp,
        &expired_config,
        &expired_use_bin,
        &expired_url,
        &format!("sha256:{}", expired.root_sha256),
    );
    expired_server.clear_requests();
    let output = run(
        &expired_temp,
        &expired_config,
        &expired_use_bin,
        &["registry", "refresh", "localhost"],
    );
    assert!(!output.status.success(), "{output:?}");
    assert_no_target_request(&expired_server);
}

#[test]
#[ignore = "requires A3S_USE_E2E_BIN pointing to a real a3s-use binary"]
fn full_stack_registry_install_and_upgrade_activate_only_reviewed_targets() {
    const NEXT_VERSION: &str = "0.2.0";

    let use_executable = std::path::PathBuf::from(
        std::env::var_os("A3S_USE_E2E_BIN")
            .expect("A3S_USE_E2E_BIN must point to the real a3s-use binary"),
    );
    assert!(use_executable.is_file(), "{}", use_executable.display());
    let use_bin = use_executable.parent().unwrap();
    let temp = TempWorkspace::new("signed-registry-full-stack");
    let repository = TestRepository::new(extension_archive(PACKAGE_VERSION), 1, FUTURE);
    let server = TestServer::start(repository.routes.clone());
    let registry_url = localhost_url(&server);
    let config = temp.path("config/config.acl");
    add_registry(
        &temp,
        &config,
        use_bin,
        &registry_url,
        &format!("sha256:{}", repository.root_sha256),
    );

    server.clear_requests();
    let plan = run(
        &temp,
        &config,
        use_bin,
        &["install", "use/a3s/science", "--dry-run"],
    );
    assert!(plan.status.success(), "{plan:?}");
    let plan = json(&plan);
    let digest = plan["data"]["planDigest"].as_str().unwrap();
    assert_no_target_request(&server);

    let installed = run(
        &temp,
        &config,
        use_bin,
        &["install", "use/a3s/science", "--plan-digest", digest],
    );
    assert!(installed.status.success(), "{installed:?}");
    assert_eq!(
        server
            .requests()
            .iter()
            .filter(|request| request.starts_with("/targets/"))
            .count(),
        1
    );
    let receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(temp.path("state/use/extensions/a3s/science.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(receipt["trust"], "registry-tuf");
    assert_eq!(receipt["registry"]["sha256"], repository.target_sha256);

    let upgraded_repository = TestRepository::with_package_version(
        extension_archive(NEXT_VERSION),
        NEXT_VERSION,
        2,
        FUTURE,
    );
    assert_eq!(repository.root_sha256, upgraded_repository.root_sha256);
    server.replace_routes(upgraded_repository.routes.clone());
    server.clear_requests();
    let upgrade_plan = run(
        &temp,
        &config,
        use_bin,
        &["upgrade", "use/a3s/science", "--dry-run"],
    );
    assert!(upgrade_plan.status.success(), "{upgrade_plan:?}");
    let upgrade_plan = json(&upgrade_plan);
    let upgrade_digest = upgrade_plan["data"]["planDigest"].as_str().unwrap();
    assert_eq!(upgrade_plan["data"]["plans"][0]["action"], "upgrade");
    assert_eq!(
        upgrade_plan["data"]["plans"][0]["resolvedRegistryPackages"]["use/a3s/science"]["version"],
        NEXT_VERSION
    );
    assert_no_target_request(&server);

    let upgraded = run(
        &temp,
        &config,
        use_bin,
        &[
            "upgrade",
            "use/a3s/science",
            "--plan-digest",
            upgrade_digest,
        ],
    );
    assert!(upgraded.status.success(), "{upgraded:?}");
    assert_eq!(
        server
            .requests()
            .iter()
            .filter(|request| request.starts_with("/targets/"))
            .count(),
        1
    );
    let upgraded_receipt: serde_json::Value = serde_json::from_slice(
        &std::fs::read(temp.path("state/use/extensions/a3s/science.json")).unwrap(),
    )
    .unwrap();
    assert_eq!(upgraded_receipt["version"], NEXT_VERSION);
    assert_eq!(
        upgraded_receipt["registry"]["sha256"],
        upgraded_repository.target_sha256
    );

    server.clear_requests();
    let converged_plan = run(
        &temp,
        &config,
        use_bin,
        &["upgrade", "use/a3s/science", "--dry-run"],
    );
    assert!(converged_plan.status.success(), "{converged_plan:?}");
    let converged_plan = json(&converged_plan);
    assert_eq!(converged_plan["data"]["plans"][0]["mutates"], false);
    let converged_digest = converged_plan["data"]["planDigest"].as_str().unwrap();
    assert_no_target_request(&server);
    let converged = run(
        &temp,
        &config,
        use_bin,
        &[
            "upgrade",
            "use/a3s/science",
            "--plan-digest",
            converged_digest,
        ],
    );
    assert!(converged.status.success(), "{converged:?}");
    assert_eq!(converged_plan["data"]["plans"][0]["force"], false);
    assert_no_target_request(&server);
}

fn add_registry(
    temp: &TempWorkspace,
    config: &std::path::Path,
    use_bin: &std::path::Path,
    url: &str,
    trust_root: &str,
) {
    let output = run(
        temp,
        config,
        use_bin,
        &["registry", "add", url, "--trust-root", trust_root, "--yes"],
    );
    assert!(output.status.success(), "{output:?}");
}

fn run(
    temp: &TempWorkspace,
    config: &std::path::Path,
    use_bin: &std::path::Path,
    args: &[&str],
) -> Output {
    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, temp);
    command
        .arg("--config")
        .arg(config)
        .args(["--output", "json"])
        .args(args)
        .env("A3S_USE_INSTALL_DIR", use_bin)
        .output()
        .unwrap()
}

fn make_use_fixture(directory: &std::path::Path, install_log: &std::path::Path) {
    make_executable(
        &directory.join("a3s-use"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'a3s-use 0.1.1\\n'; exit 0; fi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ]; then printf '{{\"schemaVersion\":1,\"ok\":true,\"data\":{{\"components\":[]}}}}\\n'; exit 0; fi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"status\" ]; then printf '{{\"schemaVersion\":1,\"ok\":true,\"data\":{{\"component\":{{\"id\":\"%s\",\"presence\":\"missing\",\"health\":\"unknown\"}}}}}}\\n' \"$3\"; exit 0; fi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"install\" ]; then printf '%s\\n' \"$@\" > {}; printf '{{\"schemaVersion\":1,\"ok\":true,\"data\":{{\"changed\":true,\"component\":{{\"id\":\"%s\",\"version\":\"{PACKAGE_VERSION}\",\"trust\":\"registry-tuf\"}}}}}}\\n' \"$3\"; exit 0; fi\nexit 2\n",
            sh_quote(install_log)
        ),
    );
}

fn make_installed_use_fixture(
    directory: &std::path::Path,
    install_log: &std::path::Path,
    installed: &ResolvedRemotePackage,
) {
    let component = serde_json::json!({
        "id": "a3s/science",
        "description": "Installed signed science extension",
        "presence": "managed",
        "health": "ready",
        "version": installed.version,
        "path": "/tmp/a3s-use-science",
        "trust": "registry-tuf",
        "registry": installed
    });
    let list_path = directory.join("component-list.json");
    let status_path = directory.join("component-status.json");
    std::fs::write(
        &list_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "ok": true,
            "data": {"components": [component.clone()]}
        }))
        .unwrap(),
    )
    .unwrap();
    std::fs::write(
        &status_path,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion": 1,
            "ok": true,
            "data": {"component": component}
        }))
        .unwrap(),
    )
    .unwrap();
    make_executable(
        &directory.join("a3s-use"),
        &format!(
            "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'a3s-use 0.1.1\\n'; exit 0; fi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"list\" ]; then /bin/cat {}; exit 0; fi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"status\" ]; then /bin/cat {}; exit 0; fi\nif [ \"$1\" = \"component\" ] && [ \"$2\" = \"install\" ]; then printf '%s\\n' \"$@\" > {}; printf '{{\"schemaVersion\":1,\"ok\":true,\"data\":{{\"changed\":true,\"component\":{{\"id\":\"%s\",\"version\":\"0.2.0\",\"trust\":\"registry-tuf\"}}}}}}\\n' \"$3\"; exit 0; fi\nexit 2\n",
            sh_quote(&list_path),
            sh_quote(&status_path),
            sh_quote(install_log)
        ),
    );
}

fn localhost_url(server: &TestServer) -> String {
    server.base_url().replacen("127.0.0.1", "localhost", 1)
}

fn json(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|error| {
        panic!(
            "invalid JSON ({error}): stdout={:?}, stderr={:?}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

fn assert_no_target_request(server: &TestServer) {
    assert!(server
        .requests()
        .iter()
        .all(|request| !request.starts_with("/targets/")));
}
