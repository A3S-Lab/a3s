use super::super::state::{ComponentState, Trust};
use super::*;

#[test]
fn parses_install_options_in_any_order() {
    let args = [
        "use/acme/slack",
        "--from",
        "./package",
        "--allow-unsigned",
        "--json",
        "--force",
    ]
    .map(str::to_string);
    let options = InstallOptions::parse(&args).unwrap();
    assert_eq!(options.components[0].as_str(), "use/acme/slack");
    assert_eq!(options.package, Some(PathBuf::from("./package")));
    assert!(options.allow_unsigned);
    assert!(options.force);
    assert!(options.json);
}

#[test]
fn rejects_conflicting_list_and_update_options() {
    assert!(ListOptions::parse(&["--installed".to_string(), "--available".to_string()]).is_err());
    assert!(UpdateOptions::parse(&["--all".to_string(), "use".to_string()]).is_err());
    assert!(InstallOptions::parse(&[
        "box".to_string(),
        "--dry-run".to_string(),
        format!("--plan-digest={}", "a".repeat(64)),
    ])
    .is_err());
    assert!(UninstallOptions::parse(&[
        "box".to_string(),
        format!("--plan-digest={}", "A".repeat(64)),
    ])
    .is_err());
}

#[test]
fn upgrade_all_selects_only_managed_products() {
    let state = |id: &str, kind, presence| ComponentState {
        id: ComponentId::parse(id).unwrap(),
        kind,
        description: String::new(),
        presence,
        health: Health::Ready,
        update: UpdateState::Unknown,
        trust: Trust::FirstParty,
        provenance: Some(InstallProvenance::GithubRelease),
        version: Some("1.0.0".to_string()),
        path: Some(PathBuf::from("/tmp/component")),
        message: None,
    };

    assert!(is_upgrade_all_candidate(&state(
        "use",
        ComponentKind::Product,
        Presence::Managed,
    )));
    assert!(!is_upgrade_all_candidate(&state(
        "use/browser",
        ComponentKind::Capability,
        Presence::Managed,
    )));
    let mut registry_extension = state(
        "use/acme/slack",
        ComponentKind::Extension,
        Presence::Managed,
    );
    registry_extension.trust = Trust::RegistryTuf;
    assert!(is_upgrade_all_candidate(&registry_extension));
    let local_extension = state(
        "use/acme/local",
        ComponentKind::Extension,
        Presence::Managed,
    );
    assert!(!is_upgrade_all_candidate(&local_extension));
    assert!(!is_upgrade_all_candidate(&state(
        "search",
        ComponentKind::Product,
        Presence::External,
    )));
}

#[test]
fn component_health_report_reuses_doctor_install_filter_without_paths() {
    let state = |id: &str, kind, presence, health| ComponentState {
        id: ComponentId::parse(id).unwrap(),
        kind,
        description: String::new(),
        presence,
        health,
        update: UpdateState::Unknown,
        trust: Trust::FirstParty,
        provenance: None,
        version: Some("1.0.0".to_string()),
        path: Some(PathBuf::from("/sensitive/install/path")),
        message: Some("probe output must stay private".to_string()),
    };
    let checks = installed_component_checks(ComponentReport {
        schema_version: 1,
        components: vec![
            state(
                "code",
                ComponentKind::BuiltIn,
                Presence::Bundled,
                Health::Ready,
            ),
            state(
                "box",
                ComponentKind::Product,
                Presence::Managed,
                Health::Broken,
            ),
            state(
                "search",
                ComponentKind::Product,
                Presence::Missing,
                Health::Unknown,
            ),
        ],
        external_tools: Vec::new(),
    });
    let public = checks
        .into_iter()
        .map(|state| ComponentHealthCheck {
            component: state.id,
            status: component_health_status(state.presence, state.health),
        })
        .collect::<Vec<_>>();

    assert_eq!(public.len(), 2);
    assert_eq!(public[0].status, ComponentHealthStatus::Ready);
    assert_eq!(public[1].status, ComponentHealthStatus::Broken);
    let serialized = serde_json::to_string(&public).unwrap();
    assert!(!serialized.contains("sensitive"));
    assert!(!serialized.contains("probe output"));
}
