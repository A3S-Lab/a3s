use std::collections::HashMap;
use std::sync::RwLock;

use crate::dirs;
use crate::error::{PowerError, Result};
use crate::model::manifest::ModelManifest;

/// In-memory index of all locally available models, backed by manifest files on disk.
pub struct ModelRegistry {
    models: RwLock<HashMap<String, ModelManifest>>,
}

impl ModelRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            models: RwLock::new(HashMap::new()),
        }
    }

    /// Scan the manifests directory and load all model manifests into memory.
    pub fn scan(&self) -> Result<()> {
        let manifest_dir = dirs::manifests_dir();
        if !manifest_dir.exists() {
            return Ok(());
        }

        let mut models = self.models.write().map_err(|e| {
            PowerError::Config(format!("Failed to acquire registry write lock: {e}"))
        })?;

        models.clear();

        let entries = std::fs::read_dir(&manifest_dir).map_err(|e| {
            PowerError::Io(std::io::Error::other(format!(
                "Failed to read manifests directory {}: {e}",
                manifest_dir.display()
            )))
        })?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                match std::fs::read_to_string(&path) {
                    Ok(content) => match serde_json::from_str::<ModelManifest>(&content) {
                        Ok(manifest) => {
                            models.insert(manifest.name.clone(), manifest);
                        }
                        Err(e) => {
                            tracing::warn!("Skipping invalid manifest {}: {e}", path.display());
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to read manifest {}: {e}", path.display());
                    }
                }
            }
        }

        Ok(())
    }

    /// List all registered models.
    pub fn list(&self) -> Result<Vec<ModelManifest>> {
        let models = self.models.read().map_err(|e| {
            PowerError::Config(format!("Failed to acquire registry read lock: {e}"))
        })?;
        let mut list: Vec<_> = models.values().cloned().collect();
        list.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(list)
    }

    /// Get a specific model by name.
    pub fn get(&self, name: &str) -> Result<ModelManifest> {
        let models = self.models.read().map_err(|e| {
            PowerError::Config(format!("Failed to acquire registry read lock: {e}"))
        })?;
        models
            .get(name)
            .cloned()
            .ok_or_else(|| PowerError::ModelNotFound(name.to_string()))
    }

    /// Register a model manifest (writes to disk and adds to in-memory index).
    pub fn register(&self, manifest: ModelManifest) -> Result<()> {
        let manifest_dir = dirs::manifests_dir();
        std::fs::create_dir_all(&manifest_dir)?;

        let filename = manifest.manifest_filename();
        let path = manifest_dir.join(&filename);
        let content = serde_json::to_string_pretty(&manifest)?;
        std::fs::write(&path, content)?;

        let mut models = self.models.write().map_err(|e| {
            PowerError::Config(format!("Failed to acquire registry write lock: {e}"))
        })?;
        models.insert(manifest.name.clone(), manifest);
        Ok(())
    }

    /// Remove a model from the registry (deletes manifest file from disk).
    pub fn remove(&self, name: &str) -> Result<ModelManifest> {
        let mut models = self.models.write().map_err(|e| {
            PowerError::Config(format!("Failed to acquire registry write lock: {e}"))
        })?;

        let manifest = models
            .remove(name)
            .ok_or_else(|| PowerError::ModelNotFound(name.to_string()))?;

        let manifest_dir = dirs::manifests_dir();
        let filename = manifest.manifest_filename();
        let path = manifest_dir.join(&filename);

        if path.exists() {
            std::fs::remove_file(&path).map_err(|e| {
                PowerError::Io(std::io::Error::other(format!(
                    "Failed to remove manifest file {}: {e}",
                    path.display()
                )))
            })?;
        }

        Ok(manifest)
    }

    /// Check if a model exists in the registry.
    pub fn exists(&self, name: &str) -> bool {
        self.models
            .read()
            .map(|models| models.contains_key(name))
            .unwrap_or(false)
    }

    /// Returns the number of registered models.
    pub fn count(&self) -> usize {
        self.models.read().map(|m| m.len()).unwrap_or(0)
    }
}

impl Default for ModelRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::manifest::{ModelFormat, ModelParameters};
    use std::path::PathBuf;

    fn sample_manifest(name: &str) -> ModelManifest {
        ModelManifest {
            name: name.to_string(),
            format: ModelFormat::Gguf,
            size: 1_000_000,
            sha256: format!("sha256-{name}"),
            parameters: Some(ModelParameters {
                context_length: Some(4096),
                embedding_length: None,
                parameter_count: Some(3_000_000_000),
                quantization: Some("Q4_K_M".to_string()),
            }),
            created_at: chrono::Utc::now(),
            path: PathBuf::from(format!("/tmp/blobs/sha256-{name}")),
        }
    }

    #[test]
    fn test_register_and_list() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let registry = ModelRegistry::new();
        registry.register(sample_manifest("model-a")).unwrap();
        registry.register(sample_manifest("model-b")).unwrap();

        let models = registry.list().unwrap();
        assert_eq!(models.len(), 2);
        assert_eq!(models[0].name, "model-a");
        assert_eq!(models[1].name, "model-b");

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_get_model() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let registry = ModelRegistry::new();
        registry.register(sample_manifest("test-model")).unwrap();

        let model = registry.get("test-model").unwrap();
        assert_eq!(model.name, "test-model");

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_get_missing_model() {
        let registry = ModelRegistry::new();
        let result = registry.get("nonexistent");
        assert!(result.is_err());
    }

    #[test]
    fn test_remove_model() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let registry = ModelRegistry::new();
        registry.register(sample_manifest("to-delete")).unwrap();
        assert!(registry.exists("to-delete"));

        registry.remove("to-delete").unwrap();
        assert!(!registry.exists("to-delete"));

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_exists() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let registry = ModelRegistry::new();
        assert!(!registry.exists("x"));
        registry.register(sample_manifest("x")).unwrap();
        assert!(registry.exists("x"));

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_count() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        let registry = ModelRegistry::new();
        assert_eq!(registry.count(), 0);
        registry.register(sample_manifest("a")).unwrap();
        assert_eq!(registry.count(), 1);
        registry.register(sample_manifest("b")).unwrap();
        assert_eq!(registry.count(), 2);

        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_scan_from_disk() {
        let dir = tempfile::tempdir().unwrap();
        std::env::set_var("A3S_POWER_HOME", dir.path());

        // Write a manifest to the first registry
        let registry1 = ModelRegistry::new();
        registry1.register(sample_manifest("persisted")).unwrap();

        // Create a second registry and scan from disk
        let registry2 = ModelRegistry::new();
        registry2.scan().unwrap();
        assert!(registry2.exists("persisted"));

        std::env::remove_var("A3S_POWER_HOME");
    }
}
