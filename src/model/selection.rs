use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use super::route::{ModelRoute, ModelSource};

/// Persisted selection shape. Field names remain compatible with existing TUI state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ModelSelection {
    pub(crate) source: ModelSource,
    pub(crate) model: String,
}

impl ModelSelection {
    pub(crate) fn route(&self) -> anyhow::Result<ModelRoute> {
        ModelRoute::new(self.source, self.model.clone())
    }
}

impl From<ModelRoute> for ModelSelection {
    fn from(route: ModelRoute) -> Self {
        Self {
            source: route.source,
            model: route.model,
        }
    }
}

pub(crate) fn selection_path() -> Option<PathBuf> {
    crate::user_paths::user_home_dir().map(|home| home.join(".a3s/tui/model-selection.json"))
}

pub(crate) fn load() -> Option<ModelSelection> {
    let path = selection_path()?;
    if let Ok(Some(selection)) = load_at(&path) {
        return Some(selection);
    }
    let legacy = path.parent()?.parent()?.join("model-selection.json");
    let selection = load_at(&legacy).ok().flatten()?;
    // Best-effort lazy migration. Keep the legacy file for downgrade support.
    let _ = save_at(&path, &selection);
    Some(selection)
}

pub(crate) fn save(selection: &ModelSelection) -> std::io::Result<()> {
    let path = selection_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "user home directory is unavailable",
        )
    })?;
    save_at(&path, selection)
}

fn load_at(path: &Path) -> std::io::Result<Option<ModelSelection>> {
    if !path.exists() {
        return Ok(None);
    }
    let raw = std::fs::read_to_string(path)?;
    let selection = serde_json::from_str::<ModelSelection>(&raw)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    selection
        .route()
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error.to_string()))?;
    Ok(Some(selection))
}

fn save_at(path: &Path, selection: &ModelSelection) -> std::io::Result<()> {
    selection.route().map_err(|error| {
        std::io::Error::new(std::io::ErrorKind::InvalidInput, error.to_string())
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let body = serde_json::to_vec_pretty(selection)
        .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?;
    let temporary = path.with_extension(format!("json.tmp-{}", std::process::id()));
    std::fs::write(&temporary, body)?;
    std::fs::rename(temporary, path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preserves_existing_selection_shape_and_replaces_atomically() {
        let dir =
            std::env::temp_dir().join(format!("a3s-model-selection-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("selection.json");
        let first = ModelSelection {
            source: ModelSource::Codex,
            model: "gpt-5".into(),
        };
        save_at(&path, &first).unwrap();
        assert_eq!(load_at(&path).unwrap(), Some(first));

        let second = ModelSelection {
            source: ModelSource::Claude,
            model: "claude-opus".into(),
        };
        save_at(&path, &second).unwrap();
        assert_eq!(load_at(&path).unwrap(), Some(second));
        assert!(!dir.join("selection.json.tmp").exists());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
