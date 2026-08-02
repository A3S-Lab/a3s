#![cfg(unix)]

mod support;

use std::os::unix::fs::symlink;
use std::process::Command;

use support::{a3s_bin, configure_component_env, TempWorkspace};

#[test]
fn registry_lifecycle_uses_isolated_acl_files() {
    let temp = TempWorkspace::new("registry-lifecycle");
    let config = temp.path("config/config.acl");
    let digest = format!("sha256:{}", "a".repeat(64));

    let mut add = Command::new(a3s_bin());
    configure_component_env(&mut add, &temp);
    let output = add
        .arg("--config")
        .arg(&config)
        .args([
            "--output",
            "json",
            "registry",
            "add",
            "https://acme.example/components/",
            "--trust-root",
            &digest,
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "registry.add");
    assert_eq!(result["data"]["registry"]["name"], "acme");
    let registry_file = temp.path("config/registries/acme.acl");
    let acl = std::fs::read_to_string(&registry_file).unwrap();
    assert!(acl.contains("registry \"acme\""), "{acl}");
    assert!(acl.contains("trust_root"), "{acl}");

    let mut list = Command::new(a3s_bin());
    configure_component_env(&mut list, &temp);
    let output = list
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "list"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let registries = result["data"]["registries"].as_array().unwrap();
    assert!(registries.iter().any(|registry| registry["name"] == "a3s"));
    assert!(registries.iter().any(|registry| registry["name"] == "acme"));

    let mut show = Command::new(a3s_bin());
    configure_component_env(&mut show, &temp);
    let output = show
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "show", "acme"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["registry"]["trustRoot"], digest);

    let mut remove = Command::new(a3s_bin());
    configure_component_env(&mut remove, &temp);
    let output = remove
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "remove", "acme", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(!registry_file.exists());
}

#[test]
fn official_registry_source_can_be_replaced_disabled_and_restored() {
    let temp = TempWorkspace::new("replaceable-registry-source");
    let config = temp.path("config/config.acl");
    let first_digest = format!("sha256:{}", "c".repeat(64));
    let second_digest = format!("sha256:{}", "d".repeat(64));

    let mut replace = Command::new(a3s_bin());
    configure_component_env(&mut replace, &temp);
    let output = replace
        .arg("--config")
        .arg(&config)
        .args([
            "--output",
            "json",
            "registry",
            "replace",
            "a3s",
            "https://mirror.example/a3s/",
            "--trust-root",
            &first_digest,
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "registry.replace");
    assert_eq!(result["data"]["registry"]["name"], "a3s");
    assert_eq!(result["data"]["registry"]["builtIn"], false);
    assert_eq!(result["data"]["registry"]["enabled"], true);
    let registry_file = temp.path("config/registries/a3s.acl");
    let acl = std::fs::read_to_string(&registry_file).unwrap();
    assert!(acl.contains("registry \"a3s\""), "{acl}");
    assert!(acl.contains("enabled = true"), "{acl}");

    let mut disable = Command::new(a3s_bin());
    configure_component_env(&mut disable, &temp);
    let output = disable
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "disable", "a3s"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "registry.disable");
    assert_eq!(result["data"]["registry"]["enabled"], false);

    let mut enable = Command::new(a3s_bin());
    configure_component_env(&mut enable, &temp);
    let output = enable
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "enable", "a3s"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );

    let mut replace_again = Command::new(a3s_bin());
    configure_component_env(&mut replace_again, &temp);
    let output = replace_again
        .arg("--config")
        .arg(&config)
        .args([
            "--output",
            "json",
            "registry",
            "replace",
            "a3s",
            "https://private.example/packages/",
            "--trust-root",
            &second_digest,
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["data"]["registry"]["url"],
        "https://private.example/packages/"
    );
    assert_eq!(result["data"]["registry"]["trustRoot"], second_digest);

    let mut remove = Command::new(a3s_bin());
    configure_component_env(&mut remove, &temp);
    let output = remove
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "remove", "a3s", "--yes"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!registry_file.exists());

    let mut show = Command::new(a3s_bin());
    configure_component_env(&mut show, &temp);
    let output = show
        .arg("--config")
        .arg(&config)
        .args(["--output", "json", "registry", "show", "a3s"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["data"]["registry"]["builtIn"], true);
    assert_eq!(result["data"]["registry"]["configured"], false);
    assert_eq!(
        result["data"]["registry"]["url"],
        "https://components.a3s.dev/"
    );
}

#[test]
fn registry_rejects_urls_that_can_leak_secrets_or_change_identity() {
    let temp = TempWorkspace::new("registry-url-policy");
    let config = temp.path("config/config.acl");
    for url in [
        "https://user:secret@acme.example/components/",
        "https://acme.example/components/?token=secret",
        "https://acme.example/components/#alternate",
        "http://acme.example/components/",
    ] {
        let mut command = Command::new(a3s_bin());
        configure_component_env(&mut command, &temp);
        let output = command
            .arg("--config")
            .arg(&config)
            .args([
                "registry",
                "add",
                url,
                "--trust-root",
                &format!("sha256:{}", "b".repeat(64)),
                "--yes",
            ])
            .output()
            .unwrap();
        assert!(!output.status.success(), "unexpectedly accepted {url}");
    }
    assert!(!temp.path("config/registries").exists());
}

#[test]
fn registry_file_trust_root_is_copied_into_owned_configuration() {
    let temp = TempWorkspace::new("registry-root-copy");
    let config = temp.path("config/config.acl");
    let source = temp.path("bootstrap-root.json");
    let root_bytes = br#"{"signed":{"_type":"root","version":1}}"#;
    std::fs::write(&source, root_bytes).unwrap();

    let mut add = Command::new(a3s_bin());
    configure_component_env(&mut add, &temp);
    let output = add
        .arg("--config")
        .arg(&config)
        .args([
            "--output",
            "json",
            "registry",
            "add",
            "https://files.example/components/",
            "--trust-root",
            source.to_str().unwrap(),
            "--yes",
        ])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");

    let owned = temp.path("config/registries/files/root.json");
    assert_eq!(std::fs::read(&owned).unwrap(), root_bytes);
    std::fs::write(&source, b"changed outside registry ownership").unwrap();
    assert_eq!(std::fs::read(&owned).unwrap(), root_bytes);
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        result["data"]["registry"]["trustedRootPath"],
        owned.to_string_lossy().to_string()
    );

    let mut remove = Command::new(a3s_bin());
    configure_component_env(&mut remove, &temp);
    let output = remove
        .arg("--config")
        .arg(&config)
        .args(["registry", "remove", "files", "--yes"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(!owned.exists());
}

#[test]
fn cache_dry_run_and_clean_stay_inside_the_owned_root() {
    let temp = TempWorkspace::new("cache-boundary");
    let cache = temp.path("cache");
    let outside = temp.path("keep.txt");
    std::fs::create_dir_all(cache.join("nested")).unwrap();
    std::fs::write(cache.join("nested/data.bin"), b"cache").unwrap();
    std::fs::write(&outside, b"keep").unwrap();

    let mut dry_run = Command::new(a3s_bin());
    configure_component_env(&mut dry_run, &temp);
    let output = dry_run
        .args(["--output", "json", "cache", "clean", "--dry-run"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let result: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(result["command"], "cache.clean");
    assert_eq!(result["data"]["dryRun"], true);
    assert!(cache.join("nested/data.bin").is_file());

    let mut clean = Command::new(a3s_bin());
    configure_component_env(&mut clean, &temp);
    let output = clean.args(["cache", "clean", "--yes"]).output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(cache.is_dir());
    assert!(std::fs::read_dir(&cache).unwrap().next().is_none());
    assert_eq!(std::fs::read(&outside).unwrap(), b"keep");
}

#[test]
fn cache_clean_refuses_a_symbolic_link_root() {
    let temp = TempWorkspace::new("cache-symlink-root");
    let target = temp.path("target");
    let link = temp.path("cache-link");
    std::fs::create_dir_all(&target).unwrap();
    std::fs::write(target.join("important.txt"), b"keep").unwrap();
    symlink(&target, &link).unwrap();

    let mut command = Command::new(a3s_bin());
    configure_component_env(&mut command, &temp);
    let output = command
        .env("A3S_CACHE_HOME", &link)
        .args(["cache", "clean", "--yes"])
        .output()
        .unwrap();

    assert!(!output.status.success());
    assert!(target.join("important.txt").is_file());
    assert!(String::from_utf8_lossy(&output.stderr).contains("symbolic-link"));
}
