#![cfg(unix)]

mod support;

use std::fs::OpenOptions;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

use fs2::FileExt;

use support::{
    a3s_bin, box_release_target, configure_component_env, make_executable, sh_quote,
    start_fake_box_release, TempWorkspace,
};

#[test]
fn managed_component_installs_updates_and_uninstalls_by_receipt() {
    if box_release_target().is_none() {
        eprintln!("skipping component lifecycle test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("managed-lifecycle");
    let first_release = start_fake_box_release(&temp, "2.5.2", None);
    let mut install = Command::new(a3s_bin());
    configure_component_env(&mut install, &temp);
    let output = install
        .args(["install", "box", "--source", "release", "--json"])
        .env("A3S_UPDATER_GITHUB_API_BASE", first_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "JSON install emitted progress: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["operations"][0]["version"], "2.5.2");
    drop(first_release);

    let second_release = start_fake_box_release(&temp, "2.6.0", None);
    let mut update = Command::new(a3s_bin());
    configure_component_env(&mut update, &temp);
    let output = update
        .args(["update", "box", "--json"])
        .env("A3S_UPDATER_GITHUB_API_BASE", second_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "legacy JSON update emitted prose: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["operations"][0]["version"], "2.6.0");
    let receipt_path = temp.path("state/components/box.json");
    let receipt: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&receipt_path).unwrap()).unwrap();
    let executable = PathBuf::from(receipt["executablePath"].as_str().unwrap());
    assert!(executable.starts_with(temp.path("data/components/box/2.6.0")));
    assert!(!temp.path("data/components/box/2.5.2").exists());
    drop(second_release);

    let mut uninstall = Command::new(a3s_bin());
    configure_component_env(&mut uninstall, &temp);
    let output = uninstall
        .args(["uninstall", "box", "--json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["operations"][0]["action"], "uninstall");
    assert!(!receipt_path.exists());
    assert!(!temp.path("data/components/box/2.6.0").exists());
}

#[test]
fn explicit_version_replaces_a_different_healthy_release_without_force() {
    if box_release_target().is_none() {
        eprintln!("skipping explicit version test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("managed-explicit-version");
    let first_release = start_fake_box_release(&temp, "2.5.2", None);
    let mut install = Command::new(a3s_bin());
    configure_component_env(&mut install, &temp);
    let output = install
        .args(["install", "box", "--source", "release", "--json"])
        .env("A3S_UPDATER_GITHUB_API_BASE", first_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop(first_release);

    let requested_release = start_fake_box_release(&temp, "2.6.0", None);
    let mut replace = Command::new(a3s_bin());
    configure_component_env(&mut replace, &temp);
    let output = replace
        .args([
            "install",
            "box",
            "--source",
            "release",
            "--version",
            "2.6.0",
            "--json",
        ])
        .env("A3S_UPDATER_GITHUB_API_BASE", requested_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["operations"][0]["changed"], true);
    assert_eq!(result["data"]["operations"][0]["version"], "2.6.0");
    assert!(requested_release
        .requests()
        .iter()
        .any(|path| path.ends_with("/releases/tags/v2.6.0")));
    assert!(!temp.path("data/components/box/2.5.2").exists());
}

#[test]
fn upgrade_without_components_lists_available_managed_upgrades_without_mutation() {
    if box_release_target().is_none() {
        eprintln!("skipping upgrade listing test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("managed-upgrade-list");
    let installed_release = start_fake_box_release(&temp, "2.5.2", None);
    let mut install = Command::new(a3s_bin());
    configure_component_env(&mut install, &temp);
    let output = install
        .args(["install", "box", "--source", "release", "--json"])
        .env("A3S_UPDATER_GITHUB_API_BASE", installed_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    drop(installed_release);

    let receipt_path = temp.path("state/components/box.json");
    let receipt_before = std::fs::read(&receipt_path).unwrap();
    let available_release = start_fake_box_release(&temp, "2.6.0", None);
    let mut upgrade = Command::new(a3s_bin());
    configure_component_env(&mut upgrade, &temp);
    let output = upgrade
        .args(["upgrade", "--json"])
        .env("A3S_UPDATER_GITHUB_API_BASE", available_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "component.upgrade");
    assert_eq!(result["data"]["components"].as_array().unwrap().len(), 1);
    assert_eq!(result["data"]["components"][0]["id"], "box");
    assert_eq!(result["data"]["components"][0]["update"], "available");
    assert_eq!(std::fs::read(&receipt_path).unwrap(), receipt_before);
    assert!(!temp.path("data/components/box/2.6.0").exists());
    let requests = available_release.requests();
    assert!(
        requests
            .iter()
            .any(|path| path.ends_with("/releases/latest")),
        "upgrade listing did not check the latest release: {requests:?}"
    );
    assert!(
        requests.iter().all(|path| !path.starts_with("/assets/")),
        "upgrade listing downloaded a release asset: {requests:?}"
    );
}

#[test]
fn upgrade_listing_excludes_unmanaged_products_and_external_tools() {
    if box_release_target().is_none() {
        eprintln!("skipping unmanaged upgrade listing test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("unmanaged-upgrade-list");
    let bin = temp.path("bin");
    make_executable(
        &bin.join("a3s-box"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'a3s-box 2.5.2\\n'; exit 0; fi\nexit 0\n",
    );
    make_executable(
        &bin.join("a3s-unregistered"),
        "#!/bin/sh\nprintf 'external tool\\n'\n",
    );
    let available_release = start_fake_box_release(&temp, "2.6.0", None);
    let mut upgrade = Command::new(a3s_bin());
    configure_component_env(&mut upgrade, &temp);
    let output = upgrade
        .args(["upgrade", "--json"])
        .env("PATH", &bin)
        .env("A3S_UPDATER_GITHUB_API_BASE", available_release.api_base())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "component.upgrade");
    assert_eq!(result["data"]["components"], serde_json::json!([]));
    assert_eq!(result["data"]["externalTools"], serde_json::json!([]));
}

#[test]
fn homebrew_component_upgrade_uses_the_upgrade_verb() {
    let temp = TempWorkspace::new("homebrew-component-upgrade");
    let bin = temp.path("bin");
    let prefix = temp.path("brew-prefix");
    let log = temp.path("brew.log");
    make_executable(
        &prefix.join("bin/a3s-box"),
        "#!/bin/sh\nif [ \"$1\" = \"--version\" ]; then printf 'a3s-box 2.6.0\\n'; exit 0; fi\nexit 0\n",
    );
    make_executable(
        &bin.join("brew"),
        &format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" >> {}\nif [ \"$1\" = \"--prefix\" ]; then printf '%s\\n' {}; exit 0; fi\ncase \"$1\" in install|upgrade|reinstall|uninstall) exit 0;; esac\nexit 2\n",
            sh_quote(&log),
            sh_quote(&prefix),
        ),
    );

    let mut install = Command::new(a3s_bin());
    configure_component_env(&mut install, &temp);
    let output = install
        .args(["install", "box", "--source", "homebrew", "--json"])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    std::fs::write(&log, "").unwrap();

    let mut upgrade = Command::new(a3s_bin());
    configure_component_env(&mut upgrade, &temp);
    let output = upgrade
        .args(["upgrade", "box", "--json"])
        .env("PATH", &bin)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let commands = std::fs::read_to_string(&log).unwrap();
    assert!(
        commands
            .lines()
            .any(|line| line == "upgrade a3s-lab/tap/a3s-box"),
        "Homebrew commands were:\n{commands}"
    );
    assert!(
        !commands
            .lines()
            .any(|line| line == "install a3s-lab/tap/a3s-box"),
        "upgrade unexpectedly reused brew install:\n{commands}"
    );
}

#[test]
fn component_mutation_waits_for_the_existing_cross_process_lock() {
    if box_release_target().is_none() {
        eprintln!("skipping component lock test on unsupported host target");
        return;
    }
    let temp = TempWorkspace::new("component-operation-lock");
    let lock_path = temp.path("runtime/locks/box.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    lock.lock_exclusive().unwrap();
    let release = start_fake_box_release(&temp, "2.5.2", None);

    let mut install = Command::new(a3s_bin());
    configure_component_env(&mut install, &temp);
    let mut child = install
        .args(["install", "box", "--source", "release", "--json"])
        .env("A3S_UPDATER_GITHUB_API_BASE", release.api_base())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    std::thread::sleep(Duration::from_millis(200));
    assert!(
        child.try_wait().unwrap().is_none(),
        "component command did not wait for its operation lock"
    );
    assert!(
        release.requests().is_empty(),
        "component command accessed the network before acquiring its lock"
    );

    FileExt::unlock(&lock).unwrap();
    drop(lock);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn component_batch_waits_for_the_global_lock_before_journaling() {
    let temp = TempWorkspace::new("component-batch-lock");
    let lock_path = temp.path("runtime/locks/component-batch.lock");
    std::fs::create_dir_all(lock_path.parent().unwrap()).unwrap();
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&lock_path)
        .unwrap();
    lock.lock_exclusive().unwrap();

    let mut install = Command::new(a3s_bin());
    configure_component_env(&mut install, &temp);
    let mut child = install
        .args(["install", "code", "--json"])
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();

    std::thread::sleep(Duration::from_millis(200));
    assert!(
        child.try_wait().unwrap().is_none(),
        "component command did not wait for the global batch lock"
    );
    assert!(
        !temp.path("state/component-operations/active.json").exists(),
        "component command journaled before acquiring the global batch lock"
    );

    FileExt::unlock(&lock).unwrap();
    drop(lock);
    let output = child.wait_with_output().unwrap();
    assert!(
        output.status.success(),
        "status: {}\nstdout: {}\nstderr: {}",
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
