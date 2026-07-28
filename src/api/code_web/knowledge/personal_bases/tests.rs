use super::*;
use crate::api::code_web::knowledge::marketplace;

#[test]
fn creates_installs_lists_and_pins_real_okf_directories() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path();

    let created = create_knowledge_base(workspace, "Project Notes", Some("Local notes"))
        .expect("create personal knowledge base");
    assert!(created.changed);
    assert_eq!(created.knowledge_base.origin, "created");
    assert!(Path::new(&created.knowledge_base.path)
        .join(MANIFEST_PATH)
        .is_file());

    let package = marketplace::packages()[0];
    let installed = install_market_package(workspace, package).expect("install market package");
    assert!(installed.changed);
    assert_eq!(
        installed.knowledge_base.marketplace_id.as_deref(),
        Some(package.id)
    );
    assert!(Path::new(&installed.knowledge_base.path)
        .join(asset_lifecycle::ASSET_ACL_PATH)
        .is_file());

    let repeated = install_market_package(workspace, package).expect("repeat market installation");
    assert!(!repeated.changed);

    let unpinned = set_pinned(workspace, &created.knowledge_base.id, false)
        .expect("unpin personal knowledge base");
    assert!(unpinned.changed);
    assert!(!unpinned.knowledge_base.pinned);

    let listed = list_knowledge_bases(workspace);
    assert!(listed.warnings.is_empty(), "{:?}", listed.warnings);
    assert_eq!(listed.items.len(), 2);
}

#[test]
fn unicode_only_names_receive_safe_stable_ids() {
    let first = knowledge_base_id("量子研究");
    assert_eq!(first, knowledge_base_id("量子研究"));
    assert!(first.starts_with("kb-"));
    validate_base_id(&first).expect("safe ID");
}

#[test]
fn invalid_package_paths_never_escape_the_base() {
    assert!(safe_relative_path("../outside").is_err());
    assert!(safe_relative_path("/outside").is_err());
    assert!(safe_relative_path("wiki/index.md").is_ok());
}

#[test]
fn imports_an_obsidian_vault_without_application_metadata() {
    let temporary = tempfile::tempdir().expect("temporary workspace");
    let workspace = temporary.path().join("workspace");
    let vault = temporary.path().join("Research Vault");
    std::fs::create_dir_all(vault.join("topics")).expect("create vault topics");
    std::fs::create_dir_all(vault.join(".obsidian")).expect("create Obsidian metadata");
    std::fs::write(vault.join("Home.md"), "# Home\n\n[[topics/Methods]]\n")
        .expect("write vault home");
    std::fs::write(vault.join("topics/Methods.md"), "# Methods\n")
        .expect("write nested vault note");
    std::fs::write(vault.join(".obsidian/workspace.json"), "{}")
        .expect("write Obsidian workspace state");

    let imported = import_knowledge_base(&workspace, &vault, None)
        .expect("import Obsidian vault as personal knowledge");

    assert!(imported.changed);
    assert_eq!(imported.knowledge_base.name, "Research Vault");
    assert_eq!(imported.knowledge_base.origin, "imported");
    let target = Path::new(&imported.knowledge_base.path);
    assert_eq!(
        std::fs::read_to_string(target.join("sources/Home.md")).expect("read imported home"),
        "# Home\n\n[[topics/Methods]]\n"
    );
    assert!(target.join("sources/topics/Methods.md").is_file());
    assert!(!target.join("sources/.obsidian").exists());
    assert!(target.join(MANIFEST_PATH).is_file());
    assert_eq!(imported.knowledge_base.source_count, 2);
}
