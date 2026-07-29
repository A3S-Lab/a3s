use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

const RETIRED_PACKAGE_VERSION: &str = "0.0.0-a3s-retired";

fn repository_path(relative: impl AsRef<Path>) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

#[test]
fn bridge_matches_the_legacy_updater_contract_and_fails_closed() {
    let root = repository_path("release-compat/support/managed-srt");
    let package_path = root.join("node_modules/@anthropic-ai/sandbox-runtime/package.json");
    let package: Value = serde_json::from_slice(&fs::read(&package_path).unwrap()).unwrap();
    assert_eq!(
        package.get("name").and_then(Value::as_str),
        Some("@anthropic-ai/sandbox-runtime")
    );
    assert_eq!(
        package.get("version").and_then(Value::as_str),
        Some(RETIRED_PACKAGE_VERSION)
    );

    let lock_path = root.join("package-lock.json");
    let lock: Value = serde_json::from_slice(&fs::read(&lock_path).unwrap()).unwrap();
    assert_eq!(lock.get("lockfileVersion").and_then(Value::as_u64), Some(3));
    assert_eq!(
        lock.pointer("/packages/node_modules~1@anthropic-ai~1sandbox-runtime/version")
            .and_then(Value::as_str),
        Some(RETIRED_PACKAGE_VERSION)
    );
    assert_eq!(
        lock.pointer("/packages//dependencies/@anthropic-ai~1sandbox-runtime")
            .and_then(Value::as_str),
        Some(RETIRED_PACKAGE_VERSION)
    );

    let cli_path = root.join("node_modules/@anthropic-ai/sandbox-runtime/dist/cli.js");
    let cli = fs::read_to_string(&cli_path).unwrap();
    assert!(cli.contains("Anthropic SRT was removed from A3S"));
    assert!(cli.contains("process.exitCode = 1"));
    for forbidden in ["child_process", "require(", "import ", "eval("] {
        assert!(
            !cli.contains(forbidden),
            "bridge must not contain {forbidden}"
        );
    }

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(
            fs::metadata(cli_path).unwrap().permissions().mode() & 0o111,
            0
        );
    }
}

#[test]
fn release_workflow_packages_and_verifies_the_bridge() {
    let workflow =
        fs::read_to_string(repository_path(".github/workflows/a3s-cli-release.yml")).unwrap();
    assert!(workflow.contains("include: web,release-compat,${{ matrix.helper }}"));
    assert!(workflow.contains("bridge_root='release-compat/support/managed-srt'"));
    assert!(workflow.contains("bridge_lock=\"${bridge_root}/package-lock.json\""));
    assert!(workflow.contains(
        "bridge_cli=\"${bridge_root}/node_modules/@anthropic-ai/sandbox-runtime/dist/cli.js\""
    ));

    let manifest = fs::read_to_string(repository_path("Cargo.toml")).unwrap();
    assert!(manifest.contains("\"!tests/legacy_self_update_bridge.rs\""));
    assert!(manifest.contains("\"!release-compat/**\""));
}
