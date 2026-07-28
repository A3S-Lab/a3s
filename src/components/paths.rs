use std::collections::BTreeMap;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use a3s_updater::ReceiptStore;
use anyhow::Context;

use super::catalog::ReleaseSpec;
use super::id::ComponentId;

#[derive(Debug, Clone)]
pub struct ComponentPaths {
    pub data_root: PathBuf,
    pub state_root: PathBuf,
    pub cache_root: PathBuf,
    pub runtime_root: PathBuf,
    pub current_exe: PathBuf,
    pub path_env: Option<OsString>,
    pub home: Option<PathBuf>,
    install_overrides: BTreeMap<String, PathBuf>,
}

impl ComponentPaths {
    pub fn from_env() -> anyhow::Result<Self> {
        let directory = std::env::current_dir()
            .context("failed to determine the current directory for component paths")?;
        Self::from_env_at(&directory)
    }

    /// Resolve component storage from the process environment without relying
    /// on the process-wide current directory for relative overrides.
    ///
    /// The umbrella CLI calls this once while constructing its immutable
    /// invocation context. Component handlers then receive the resolved paths
    /// explicitly instead of rediscovering environment and directory state.
    pub fn from_env_at(directory: &Path) -> anyhow::Result<Self> {
        let home = std::env::var_os("HOME").map(PathBuf::from);
        let local_app_data = std::env::var_os("LOCALAPPDATA").map(PathBuf::from);
        let data_root = configured_root(
            RootKind::Data,
            directory,
            home.as_deref(),
            local_app_data.as_deref(),
        )?;
        let state_root = configured_root(
            RootKind::State,
            directory,
            home.as_deref(),
            local_app_data.as_deref(),
        )?;
        let cache_root = configured_root(
            RootKind::Cache,
            directory,
            home.as_deref(),
            local_app_data.as_deref(),
        )?;
        let runtime_root =
            configured_runtime_root(directory, &state_root, local_app_data.as_deref());
        let current_exe =
            std::env::current_exe().context("failed to determine current a3s executable")?;

        Ok(Self {
            data_root,
            state_root,
            cache_root,
            runtime_root,
            current_exe,
            path_env: std::env::var_os("PATH"),
            home,
            install_overrides: [
                "A3S_BOX_INSTALL_DIR",
                "A3S_BENCH_INSTALL_DIR",
                "A3S_SEARCH_INSTALL_DIR",
                "A3S_USE_INSTALL_DIR",
                "A3S_WEBVIEW_INSTALL_DIR",
            ]
            .into_iter()
            .filter_map(|name| {
                std::env::var_os(name)
                    .map(PathBuf::from)
                    .map(|path| (name.to_string(), absolute_from(path, directory)))
            })
            .collect(),
        })
    }

    pub fn receipt_store(&self) -> ReceiptStore {
        ReceiptStore::new(&self.state_root)
    }

    pub fn component_root(&self, id: &ComponentId) -> PathBuf {
        append_id(self.data_root.join("components"), id)
    }

    pub fn version_root(&self, id: &ComponentId, version: &str) -> PathBuf {
        self.component_root(id).join(version)
    }

    pub fn cache_dir(&self, id: &ComponentId) -> PathBuf {
        append_id(self.cache_root.join("components"), id)
    }

    pub fn operation_lock_path(&self, id: &ComponentId) -> PathBuf {
        let family = id.as_str().split('/').next().unwrap_or(id.as_str());
        self.runtime_root
            .join("locks")
            .join(format!("{family}.lock"))
    }

    pub fn batch_operation_lock_path(&self) -> PathBuf {
        self.runtime_root.join("locks/component-batch.lock")
    }

    pub(crate) fn operation_journal_root(&self) -> PathBuf {
        self.state_root.join("component-operations")
    }

    pub(crate) fn active_operation_journal_path(&self) -> PathBuf {
        self.operation_journal_root().join("active.json")
    }

    pub(crate) fn last_operation_journal_path(&self) -> PathBuf {
        self.operation_journal_root().join("last.json")
    }

    pub(crate) fn interrupted_operation_journal_path(&self) -> PathBuf {
        self.operation_journal_root().join("last-interrupted.json")
    }

    pub fn configured_binary(&self, release: ReleaseSpec) -> Option<PathBuf> {
        self.install_overrides
            .get(release.install_dir_env)
            .map(|directory| directory.join(host_binary_name(release.binary)))
    }

    pub fn sibling_binary(&self, binary: &str) -> Option<PathBuf> {
        self.current_exe
            .parent()
            .map(|parent| parent.join(host_binary_name(binary)))
    }

    pub fn fallback_binary(&self, binary: &str) -> Option<PathBuf> {
        self.home
            .as_deref()
            .map(|home| home.join(".local/bin").join(host_binary_name(binary)))
    }

    #[cfg(test)]
    pub fn for_test(root: &Path) -> Self {
        Self {
            data_root: root.join("data"),
            state_root: root.join("state"),
            cache_root: root.join("cache"),
            runtime_root: root.join("runtime"),
            current_exe: root.join("bin/a3s"),
            path_env: None,
            home: Some(root.join("home")),
            install_overrides: BTreeMap::new(),
        }
    }

    #[cfg(test)]
    pub fn set_install_override(&mut self, variable: &str, directory: PathBuf) {
        self.install_overrides
            .insert(variable.to_string(), directory);
    }
}

fn configured_runtime_root(
    directory: &Path,
    state_root: &Path,
    local_app_data: Option<&Path>,
) -> PathBuf {
    if let Some(value) = std::env::var_os("A3S_RUNTIME_HOME").filter(|value| !value.is_empty()) {
        return absolute_from(PathBuf::from(value), directory);
    }

    #[cfg(unix)]
    if let Some(value) = std::env::var_os("XDG_RUNTIME_DIR").filter(|value| !value.is_empty()) {
        return absolute_from(PathBuf::from(value), directory).join("a3s");
    }

    #[cfg(windows)]
    if let Some(local_app_data) = local_app_data {
        return local_app_data.join("A3S/Runtime");
    }

    #[cfg(not(windows))]
    let _ = local_app_data;

    state_root.join("runtime")
}

pub(super) fn host_binary_name(binary: &str) -> String {
    if cfg!(windows) && !binary.ends_with(".exe") {
        format!("{binary}.exe")
    } else {
        binary.to_string()
    }
}

#[derive(Clone, Copy)]
enum RootKind {
    Data,
    State,
    Cache,
}

impl RootKind {
    fn a3s_variable(self) -> &'static str {
        match self {
            Self::Data => "A3S_DATA_HOME",
            Self::State => "A3S_STATE_HOME",
            Self::Cache => "A3S_CACHE_HOME",
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn xdg_variable(self) -> &'static str {
        match self {
            Self::Data => "XDG_DATA_HOME",
            Self::State => "XDG_STATE_HOME",
            Self::Cache => "XDG_CACHE_HOME",
        }
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    fn unix_fallback(self) -> &'static str {
        match self {
            Self::Data => ".local/share/a3s",
            Self::State => ".local/state/a3s",
            Self::Cache => ".cache/a3s",
        }
    }

    #[cfg(windows)]
    fn windows_suffix(self) -> &'static str {
        match self {
            Self::Data => "A3S/Data",
            Self::State => "A3S/State",
            Self::Cache => "A3S/Cache",
        }
    }
}

fn configured_root(
    kind: RootKind,
    directory: &Path,
    home: Option<&Path>,
    local_app_data: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    #[cfg(not(windows))]
    let _ = local_app_data;
    #[cfg(windows)]
    let _ = home;

    if let Some(value) = std::env::var_os(kind.a3s_variable()).filter(|value| !value.is_empty()) {
        return Ok(absolute_from(PathBuf::from(value), directory));
    }

    #[cfg(target_os = "macos")]
    if let Some(home) = home {
        let suffix = match kind {
            RootKind::Data => "Library/Application Support/A3S",
            RootKind::State => "Library/Application Support/A3S/State",
            RootKind::Cache => "Library/Caches/A3S",
        };
        return Ok(home.join(suffix));
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if let Some(value) = std::env::var_os(kind.xdg_variable()).filter(|value| !value.is_empty())
        {
            return Ok(absolute_from(PathBuf::from(value), directory).join("a3s"));
        }
        if let Some(home) = home {
            return Ok(home.join(kind.unix_fallback()));
        }
    }

    #[cfg(windows)]
    if let Some(local_app_data) = local_app_data {
        return Ok(local_app_data.join(kind.windows_suffix()));
    }

    anyhow::bail!(
        "{} is not set and no home directory is available",
        kind.a3s_variable()
    )
}

fn absolute_from(path: PathBuf, directory: &Path) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        directory.join(path)
    }
}

fn append_id(mut root: PathBuf, id: &ComponentId) -> PathBuf {
    for segment in id.as_str().split('/') {
        root.push(segment);
    }
    root
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn component_paths_preserve_hierarchy() {
        let temp = tempfile::tempdir().unwrap();
        let mut paths = ComponentPaths::for_test(temp.path());
        let id = ComponentId::parse("use/browser").unwrap();
        assert_eq!(
            paths.version_root(&id, "1.0.0"),
            temp.path().join("data/components/use/browser/1.0.0")
        );
        assert_eq!(
            paths.cache_dir(&id),
            temp.path().join("cache/components/use/browser")
        );
        assert_eq!(
            paths.operation_lock_path(&id),
            temp.path().join("runtime/locks/use.lock")
        );
        assert_eq!(
            paths.batch_operation_lock_path(),
            temp.path().join("runtime/locks/component-batch.lock")
        );
        assert_eq!(
            paths.active_operation_journal_path(),
            temp.path().join("state/component-operations/active.json")
        );
        paths.set_install_override("A3S_USE_INSTALL_DIR", temp.path().join("use-bin"));
        let use_spec = crate::components::catalog::find(&ComponentId::parse("use").unwrap())
            .and_then(crate::components::catalog::release)
            .unwrap();
        assert_eq!(
            paths.configured_binary(use_spec),
            Some(
                temp.path()
                    .join("use-bin")
                    .join(host_binary_name("a3s-use"))
            )
        );
        paths.set_install_override("A3S_WEBVIEW_INSTALL_DIR", temp.path().join("webview-bin"));
        let webview_spec =
            crate::components::catalog::find(&ComponentId::parse("webview").unwrap())
                .and_then(crate::components::catalog::release)
                .unwrap();
        assert_eq!(
            paths.configured_binary(webview_spec),
            Some(
                temp.path()
                    .join("webview-bin")
                    .join(host_binary_name("a3s-webview"))
            )
        );
    }
}
