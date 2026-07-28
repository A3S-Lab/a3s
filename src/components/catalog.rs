use serde::{Deserialize, Serialize};

use super::id::ComponentId;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ComponentKind {
    BuiltIn,
    Product,
    Capability,
    Extension,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Distribution {
    Bundled,
    Release(ReleaseSpec),
    Delegated { parent: &'static str },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReleaseSpec {
    pub binary: &'static str,
    pub github_owner: &'static str,
    pub github_repo: &'static str,
    pub homebrew_formula: Option<&'static str>,
    pub install_dir_env: &'static str,
    pub asset_family: AssetFamily,
    pub probe: ReleaseProbe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReleaseProbe {
    Version,
    AgentIslandContract,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetFamily {
    BoxPackage,
    PortableBinary,
    BenchPackage,
    RustTargetBinary,
}

impl AssetFamily {
    pub fn target(self) -> Option<&'static str> {
        match (self, std::env::consts::OS, std::env::consts::ARCH) {
            (Self::BoxPackage, "macos", "aarch64") => Some("macos-arm64"),
            (Self::BoxPackage, "linux", "aarch64") => Some("linux-arm64"),
            (Self::BoxPackage, "linux", "x86_64") => Some("linux-x86_64"),
            (Self::PortableBinary | Self::BenchPackage, "macos", "aarch64") => Some("darwin-arm64"),
            (Self::PortableBinary | Self::BenchPackage, "macos", "x86_64") => Some("darwin-x86_64"),
            (Self::PortableBinary | Self::BenchPackage, "linux", "aarch64") => Some("linux-arm64"),
            (Self::PortableBinary | Self::BenchPackage, "linux", "x86_64") => Some("linux-x86_64"),
            (Self::PortableBinary | Self::BenchPackage, "windows", "x86_64") => {
                Some("windows-x86_64")
            }
            (Self::RustTargetBinary, "macos", "aarch64") => Some("aarch64-apple-darwin"),
            (Self::RustTargetBinary, "macos", "x86_64") => Some("x86_64-apple-darwin"),
            (Self::RustTargetBinary, "linux", "aarch64") => Some("aarch64-unknown-linux-gnu"),
            (Self::RustTargetBinary, "linux", "x86_64") => Some("x86_64-unknown-linux-gnu"),
            (Self::RustTargetBinary, "windows", "x86_64") => Some("x86_64-pc-windows-msvc"),
            _ => None,
        }
    }

    pub fn archive_name(self, binary: &str, version: &str, target: &str) -> String {
        match self {
            Self::BoxPackage => {
                format!("{binary}-v{version}-{target}.tar.gz")
            }
            Self::PortableBinary | Self::BenchPackage => {
                let extension = if target.starts_with("windows-") {
                    "zip"
                } else {
                    "tar.gz"
                };
                format!("{binary}-{version}-{target}.{extension}")
            }
            Self::RustTargetBinary => {
                let extension = if target.contains("-windows-") {
                    "zip"
                } else {
                    "tar.gz"
                };
                format!("{binary}-v{version}-{target}.{extension}")
            }
        }
    }

    pub fn executable_name(self, binary: &str, target: &str) -> String {
        let windows = match self {
            Self::PortableBinary | Self::BenchPackage => target.starts_with("windows-"),
            Self::RustTargetBinary => target.contains("-windows-"),
            Self::BoxPackage => false,
        };
        if windows {
            format!("{binary}.exe")
        } else {
            binary.to_string()
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ComponentSpec {
    pub id: &'static str,
    pub kind: ComponentKind,
    pub description: &'static str,
    pub distribution: Distribution,
    pub auto_install_on_use: bool,
    pub removable: bool,
}

const COMPONENTS: &[ComponentSpec] = &[
    ComponentSpec {
        id: "code",
        kind: ComponentKind::BuiltIn,
        description: "Interactive coding agent and asset management",
        distribution: Distribution::Bundled,
        auto_install_on_use: false,
        removable: false,
    },
    ComponentSpec {
        id: "box",
        kind: ComponentKind::Product,
        description: "Isolated application and command runtime",
        distribution: Distribution::Release(ReleaseSpec {
            binary: "a3s-box",
            github_owner: "A3S-Lab",
            github_repo: "Box",
            homebrew_formula: Some("a3s-lab/tap/a3s-box"),
            install_dir_env: "A3S_BOX_INSTALL_DIR",
            asset_family: AssetFamily::BoxPackage,
            probe: ReleaseProbe::Version,
        }),
        auto_install_on_use: true,
        removable: true,
    },
    ComponentSpec {
        id: "bench",
        kind: ComponentKind::Product,
        description: "Reproducible agent evaluation",
        distribution: Distribution::Release(ReleaseSpec {
            binary: "a3s-bench",
            github_owner: "A3S-Lab",
            github_repo: "Bench",
            homebrew_formula: None,
            install_dir_env: "A3S_BENCH_INSTALL_DIR",
            asset_family: AssetFamily::BenchPackage,
            probe: ReleaseProbe::Version,
        }),
        auto_install_on_use: false,
        removable: true,
    },
    ComponentSpec {
        id: "search",
        kind: ComponentKind::Product,
        description: "Embeddable meta search engine",
        distribution: Distribution::Release(ReleaseSpec {
            binary: "a3s-search",
            github_owner: "A3S-Lab",
            github_repo: "Search",
            homebrew_formula: Some("a3s-lab/tap/a3s-search"),
            install_dir_env: "A3S_SEARCH_INSTALL_DIR",
            asset_family: AssetFamily::PortableBinary,
            probe: ReleaseProbe::Version,
        }),
        auto_install_on_use: false,
        removable: true,
    },
    ComponentSpec {
        id: "use",
        kind: ComponentKind::Product,
        description: "Browser, Office, and external application capabilities",
        distribution: Distribution::Release(ReleaseSpec {
            binary: "a3s-use",
            github_owner: "A3S-Lab",
            github_repo: "Use",
            homebrew_formula: Some("a3s-lab/tap/a3s-use"),
            install_dir_env: "A3S_USE_INSTALL_DIR",
            asset_family: AssetFamily::PortableBinary,
            probe: ReleaseProbe::Version,
        }),
        auto_install_on_use: true,
        removable: true,
    },
    ComponentSpec {
        id: "use/browser",
        kind: ComponentKind::Capability,
        description: "Browser runtime readiness",
        distribution: Distribution::Delegated { parent: "use" },
        auto_install_on_use: false,
        removable: true,
    },
    ComponentSpec {
        id: "use/office",
        kind: ComponentKind::Capability,
        description: "OfficeCLI runtime readiness",
        distribution: Distribution::Delegated { parent: "use" },
        auto_install_on_use: false,
        removable: true,
    },
    ComponentSpec {
        id: "use/ocr",
        kind: ComponentKind::Capability,
        description: "Native OCR runtime readiness",
        distribution: Distribution::Delegated { parent: "use" },
        auto_install_on_use: false,
        removable: true,
    },
    ComponentSpec {
        id: "webview",
        kind: ComponentKind::Capability,
        description: "Native RemoteUI and Agent Island window helper",
        distribution: Distribution::Release(ReleaseSpec {
            binary: "a3s-webview",
            github_owner: "A3S-Lab",
            github_repo: "WebView",
            homebrew_formula: Some("a3s-lab/tap/a3s-webview"),
            install_dir_env: "A3S_WEBVIEW_INSTALL_DIR",
            asset_family: AssetFamily::RustTargetBinary,
            probe: ReleaseProbe::AgentIslandContract,
        }),
        auto_install_on_use: true,
        removable: true,
    },
];

pub fn all() -> &'static [ComponentSpec] {
    COMPONENTS
}

pub fn find(id: &ComponentId) -> Option<&'static ComponentSpec> {
    COMPONENTS
        .iter()
        .find(|component| component.id == id.as_str())
}

pub fn release(spec: &ComponentSpec) -> Option<ReleaseSpec> {
    match spec.distribution {
        Distribution::Release(release) => Some(release),
        Distribution::Bundled | Distribution::Delegated { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[test]
    fn catalog_ids_are_valid_and_unique() {
        let mut ids = BTreeSet::new();
        for spec in all() {
            ComponentId::parse(spec.id).unwrap();
            assert!(ids.insert(spec.id), "duplicate component {}", spec.id);
        }
    }

    #[test]
    fn catalog_contains_required_use_hierarchy() {
        for id in ["use", "use/browser", "use/office", "use/ocr"] {
            assert!(find(&ComponentId::parse(id).unwrap()).is_some());
        }
    }

    #[test]
    fn catalog_contains_first_use_webview_release() {
        let webview = find(&ComponentId::parse("webview").unwrap())
            .expect("webview must be a registered component");
        assert!(webview.auto_install_on_use);
        let release = release(webview).expect("webview must own a release");
        assert_eq!(release.binary, "a3s-webview");
        assert_eq!(release.github_owner, "A3S-Lab");
        assert_eq!(release.github_repo, "WebView");
        assert_eq!(release.install_dir_env, "A3S_WEBVIEW_INSTALL_DIR");
    }

    #[test]
    fn archive_names_match_existing_release_conventions() {
        assert_eq!(
            AssetFamily::BoxPackage.archive_name("a3s-box", "2.5.2", "linux-x86_64"),
            "a3s-box-v2.5.2-linux-x86_64.tar.gz"
        );
        assert_eq!(
            AssetFamily::PortableBinary.archive_name("a3s-search", "1.4.1", "darwin-arm64"),
            "a3s-search-1.4.1-darwin-arm64.tar.gz"
        );
        assert_eq!(
            AssetFamily::PortableBinary.archive_name("a3s-use", "0.1.0", "windows-x86_64"),
            "a3s-use-0.1.0-windows-x86_64.zip"
        );
        assert_eq!(
            AssetFamily::PortableBinary.executable_name("a3s-use", "windows-x86_64"),
            "a3s-use.exe"
        );
        assert_eq!(
            AssetFamily::RustTargetBinary.archive_name(
                "a3s-webview",
                "0.1.3",
                "x86_64-pc-windows-msvc"
            ),
            "a3s-webview-v0.1.3-x86_64-pc-windows-msvc.zip"
        );
        assert_eq!(
            AssetFamily::RustTargetBinary.executable_name("a3s-webview", "x86_64-pc-windows-msvc"),
            "a3s-webview.exe"
        );
    }
}
