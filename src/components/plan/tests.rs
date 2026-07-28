use std::path::{Path, PathBuf};

use super::super::catalog::ComponentKind;
use super::super::state::{Trust, UpdateState};
use super::*;

fn fixture(message: &str) -> OperationPlan {
    OperationPlan {
        schema_version: PLAN_SCHEMA_VERSION,
        component: ComponentId::parse("box").unwrap(),
        action: "install",
        source: "release".to_string(),
        requested_source: Some("release".to_string()),
        channel: Some("stable".to_string()),
        scope: Some("user".to_string()),
        migration: Some(false),
        target: "linux-x86_64".to_string(),
        ownership: "a3s".to_string(),
        mutates: true,
        requested_version: Some("1.2.3".to_string()),
        local_package: None,
        resolved_sources: BTreeMap::from([("box".to_string(), "github-release".to_string())]),
        resolved_releases: BTreeMap::new(),
        resolved_release_bundles: BTreeMap::new(),
        resolved_registry_packages: BTreeMap::new(),
        prerequisites: BTreeMap::new(),
        force: Some(false),
        allow_unsigned: Some(false),
        cascade: None,
        purge: None,
        current: None,
        message: message.to_string(),
    }
}

#[test]
fn digest_is_stable_and_excludes_presentation_text() {
    let first = OperationPlanSet::new("component.install", vec![fixture("first")]).unwrap();
    let second = OperationPlanSet::new("component.install", vec![fixture("second")]).unwrap();
    assert_eq!(first.plan_digest, second.plan_digest);
    assert_eq!(first.plan_digest.len(), 64);
    assert!(first
        .plan_digest
        .bytes()
        .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase()));
}

#[test]
fn digest_changes_with_semantics_or_operation_order() {
    let first = fixture("same");
    let mut forced = first.clone();
    forced.force = Some(true);
    let mut different_version = first.clone();
    different_version.requested_version = Some("2.0.0".to_string());
    let mut different_source = first.clone();
    different_source.source = "homebrew:a3s-lab/tap/a3s-box".to_string();
    different_source.requested_source = Some("homebrew".to_string());
    let mut purged = first.clone();
    purged.purge = Some(true);
    assert_ne!(
        plan_digest("component.install", std::slice::from_ref(&first)).unwrap(),
        plan_digest("component.install", &[forced.clone()]).unwrap()
    );
    for changed in [different_version, different_source, purged] {
        assert_ne!(
            plan_digest("component.install", std::slice::from_ref(&first)).unwrap(),
            plan_digest("component.install", &[changed]).unwrap()
        );
    }
    assert_ne!(
        plan_digest("component.install", &[first.clone(), forced.clone()]).unwrap(),
        plan_digest("component.install", &[forced, first]).unwrap()
    );
}

#[test]
fn current_state_is_part_of_the_digest() {
    let mut first = fixture("same");
    first.current = Some(PlannedCurrentState::from(&ComponentState {
        id: ComponentId::parse("box").unwrap(),
        kind: ComponentKind::Product,
        description: String::new(),
        presence: Presence::Managed,
        health: Health::Ready,
        update: UpdateState::Unknown,
        trust: Trust::FirstParty,
        provenance: Some(InstallProvenance::GithubRelease),
        version: Some("1.0.0".to_string()),
        path: Some(PathBuf::from("/components/box")),
        message: None,
    }));
    let mut second = first.clone();
    second.current.as_mut().unwrap().version = Some("2.0.0".to_string());
    assert_ne!(
        plan_digest("component.install", &[first]).unwrap(),
        plan_digest("component.install", &[second]).unwrap()
    );
}

#[test]
fn receipt_ownership_and_checksums_are_part_of_the_digest() {
    let mut first = fixture("same");
    first.current = Some(PlannedCurrentState {
        presence: Presence::Managed,
        health: Health::Ready,
        provenance: Some(InstallProvenance::GithubRelease),
        version: Some("1.0.0".to_string()),
        path: Some(PlannedPath::new(Path::new("/components/box/a3s-box"))),
        receipt: Some(PlannedReceipt {
            schema_version: 1,
            component_id: "box".to_string(),
            version: "1.0.0".to_string(),
            provenance: InstallProvenance::GithubRelease,
            install_root: PlannedPath::new(Path::new("/components/box")),
            executable_path: Some(PlannedPath::new(Path::new("/components/box/a3s-box"))),
            owned_paths: vec![PlannedPath::new(Path::new("/components/box"))],
            source: Some("https://example.invalid/releases/v1.0.0".to_string()),
            artifact_checksums: BTreeMap::from([("box.tar.gz".to_string(), "a".repeat(64))]),
        }),
    });
    let mut second = first.clone();
    second
        .current
        .as_mut()
        .unwrap()
        .receipt
        .as_mut()
        .unwrap()
        .artifact_checksums
        .insert("box.tar.gz".to_string(), "b".repeat(64));
    assert_ne!(
        plan_digest("component.uninstall", &[first]).unwrap(),
        plan_digest("component.uninstall", &[second]).unwrap()
    );
}

#[test]
fn expected_digest_rejects_a_changed_plan() {
    let plan = OperationPlanSet::new("component.install", vec![fixture("same")]).unwrap();
    plan.verify_expected(Some(plan.digest())).unwrap();
    let error = plan.verify_expected(Some(&"0".repeat(64))).unwrap_err();
    let mismatch = error.downcast_ref::<ComponentPlanMismatch>().unwrap();
    assert_eq!(mismatch.expected, "0".repeat(64));
    assert_eq!(mismatch.actual, plan.digest());
}

#[test]
fn resolved_release_is_part_of_the_digest() {
    let mut first = fixture("same");
    first.resolved_releases.insert(
        "box".to_string(),
        ResolvedRelease {
            version: "1.2.3".to_string(),
            tag: "v1.2.3".to_string(),
            target: "linux-x86_64".to_string(),
            archive_name: "a3s-box-v1.2.3-linux-x86_64.tar.gz".to_string(),
            asset_url: "https://example.invalid/box.tar.gz".to_string(),
            sha256: "a".repeat(64),
        },
    );
    let mut second = first.clone();
    second.resolved_releases.get_mut("box").unwrap().sha256 = "b".repeat(64);
    assert_ne!(
        plan_digest("component.install", &[first]).unwrap(),
        plan_digest("component.install", &[second]).unwrap()
    );
}

#[tokio::test]
async fn local_package_fingerprint_is_stable_and_tracks_content() {
    let temp = tempfile::tempdir().unwrap();
    let package = temp.path().join("package");
    std::fs::create_dir_all(package.join("nested")).unwrap();
    std::fs::write(package.join("nested/tool"), b"first").unwrap();

    let first = fingerprint_local_package(&package).await.unwrap();
    let repeated = fingerprint_local_package(&package).await.unwrap();
    assert_eq!(first, repeated);
    assert_eq!(first.file_count, 1);
    assert_eq!(first.byte_count, 5);

    std::fs::write(package.join("nested/tool"), b"other").unwrap();
    let changed = fingerprint_local_package(&package).await.unwrap();
    assert_ne!(first.sha256, changed.sha256);

    let mut first_plan = fixture("same");
    first_plan.local_package = Some(first);
    let mut changed_plan = first_plan.clone();
    changed_plan.local_package = Some(changed);
    assert_ne!(
        plan_digest("component.install", &[first_plan]).unwrap(),
        plan_digest("component.install", &[changed_plan]).unwrap()
    );
}
