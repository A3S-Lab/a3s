//! `a3s annotate` command - Set annotations on resources (kubectl annotate style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

/// Annotate command - Set, update, or remove annotations on resources.
#[derive(clap::Parser, Debug)]
pub struct AnnotateCommand {
    /// Resource type (pod, service, deployment, etc.).
    #[arg(short = 't', long)]
    resource_type: Option<String>,

    /// Resource name.
    name: String,

    /// Namespace (for namespaced resources).
    #[arg(short, long, default_value = "default")]
    namespace: String,

    /// Annotation to set in KEY=VALUE format.
    #[arg(last = true)]
    annotations: Vec<String>,

    /// Overwrite existing annotations.
    #[arg(short, long)]
    overwrite: bool,

    /// List annotations on a resource.
    #[arg(long)]
    list: bool,

    /// Remove an annotation (KEY-).
    #[arg(long)]
    remove: Vec<String>,
}

impl AnnotateCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    /// Find sandbox ID by name.
    fn find_sandbox_id(name: &str) -> Result<String> {
        let boxes_dir = Self::boxes_dir();

        if boxes_dir.join(name).exists() {
            return Ok(name.to_string());
        }

        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let info_path = path.join("info.json");
                    if info_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&info_path) {
                            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                                if info
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s == name)
                                    .unwrap_or(false)
                                {
                                    return path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .ok_or_else(|| {
                                            A3sError::Project(format!(
                                                "invalid sandbox id for '{}'",
                                                name
                                            ))
                                        });
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(A3sError::Project(format!("resource '{}' not found", name)))
    }

    /// Get annotations from info.json.
    fn get_annotations(sandbox_id: &str) -> Result<HashMap<String, String>> {
        let info_path = Self::boxes_dir().join(sandbox_id).join("info.json");
        if !info_path.exists() {
            return Ok(HashMap::new());
        }

        let content = std::fs::read_to_string(&info_path)?;
        let info: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| A3sError::Project(e.to_string()))?;

        let mut annotations = HashMap::new();

        if let Some(obj) = info.get("annotations").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    annotations.insert(k.clone(), s.to_string());
                }
            }
        }

        Ok(annotations)
    }

    /// Set annotations in info.json.
    fn set_annotations(sandbox_id: &str, annotations: HashMap<String, String>) -> Result<()> {
        let info_path = Self::boxes_dir().join(sandbox_id).join("info.json");
        if !info_path.exists() {
            return Err(A3sError::Project(format!(
                "resource '{}' does not have an info.json",
                sandbox_id
            )));
        }

        let content = std::fs::read_to_string(&info_path)?;
        let mut info: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| A3sError::Project(e.to_string()))?;

        // Update annotations
        if let Some(obj) = info.as_object_mut() {
            let annotations_json: std::collections::HashMap<String, serde_json::Value> =
                annotations
                    .into_iter()
                    .map(|(k, v)| (k, serde_json::json!(v)))
                    .collect();
            obj.insert(
                "annotations".to_string(),
                serde_json::json!(annotations_json),
            );
        } else {
            return Err(A3sError::Project("invalid info.json format".to_string()));
        }

        let content =
            serde_json::to_string_pretty(&info).map_err(|e| A3sError::Project(e.to_string()))?;
        std::fs::write(&info_path, content)?;

        Ok(())
    }

    /// Parse an annotation string (KEY=VALUE or KEY-).
    fn parse_annotation(s: &str) -> Option<(String, Option<String>)> {
        if s.ends_with('-') {
            let key = &s[..s.len() - 1];
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), None))
        } else if let Some((key, value)) = s.split_once('=') {
            if key.is_empty() {
                return None;
            }
            Some((key.to_string(), Some(value.to_string())))
        } else {
            None
        }
    }

    /// List annotations on a resource.
    fn list_annotations(&self) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let annotations = Self::get_annotations(&sandbox_id)?;

        if annotations.is_empty() {
            println!(
                "No annotations configured for {} '{}'.",
                self.resource_type.as_deref().unwrap_or("resource"),
                self.name
            );
        } else {
            println!(
                "ANNOTATIONS on {} '{}':",
                self.resource_type.as_deref().unwrap_or("resource"),
                self.name
            );
            for (key, value) in &annotations {
                println!("  {}={}", key, value);
            }
        }

        Ok(())
    }

    /// Apply annotation changes to a resource.
    fn apply_annotations(&self) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let mut annotations = Self::get_annotations(&sandbox_id)?;

        // Handle annotation removals first
        for remove_key in &self.remove {
            if annotations.remove(remove_key).is_none() {
                println!("Warning: annotation '{}' not found, ignoring", remove_key);
            } else {
                println!("Removed annotation '{}' from '{}'", remove_key, self.name);
            }
        }

        // Handle new annotations
        for annotation_str in &self.annotations {
            if let Some((key, value_opt)) = Self::parse_annotation(annotation_str) {
                if value_opt.is_none() {
                    annotations.remove(&key);
                    println!("Removed annotation '{}' from '{}'", key, self.name);
                } else if let Some(value) = value_opt {
                    if annotations.contains_key(&key) && !self.overwrite {
                        return Err(A3sError::Project(format!(
                            "annotation '{}' already exists, use --overwrite to replace it",
                            key
                        )));
                    }
                    annotations.insert(key.clone(), value.clone());
                    println!("Annotation '{}' set to '{}' on '{}'", key, value, self.name);
                }
            } else {
                return Err(A3sError::Project(format!(
                    "invalid annotation format: '{}' (expected KEY=VALUE or KEY-)",
                    annotation_str
                )));
            }
        }

        Self::set_annotations(&sandbox_id, annotations)?;

        Ok(())
    }
}

#[async_trait]
impl Command for AnnotateCommand {
    async fn run(&self) -> Result<()> {
        if self.list {
            return self.list_annotations();
        }

        if self.annotations.is_empty() && self.remove.is_empty() {
            return Err(A3sError::Project(
                "no annotations specified (use KEY=VALUE or KEY- for removal)".to_string(),
            ));
        }

        self.apply_annotations()
    }
}
