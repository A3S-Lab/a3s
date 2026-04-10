//! `a3s label` command - Set labels on resources (kubectl label style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::PathBuf;

/// Label command - Set, update, or remove labels on resources.
#[derive(clap::Parser, Debug)]
pub struct LabelCommand {
    /// Resource type (pod, service, deployment, etc.).
    #[arg(short = 't', long)]
    resource_type: Option<String>,

    /// Resource name.
    name: String,

    /// Namespace (for namespaced resources).
    #[arg(short, long, default_value = "default")]
    namespace: String,

    /// Label to set in KEY=VALUE format.
    #[arg(last = true)]
    labels: Vec<String>,

    /// Overwrite existing labels.
    #[arg(short, long)]
    overwrite: bool,

    /// List labels on a resource.
    #[arg(long)]
    list: bool,

    /// Remove a label (KEY-).
    #[arg(long)]
    remove: Vec<String>,
}

impl LabelCommand {
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

    /// Get labels from info.json.
    fn get_labels(sandbox_id: &str) -> Result<HashMap<String, String>> {
        let info_path = Self::boxes_dir().join(sandbox_id).join("info.json");
        if !info_path.exists() {
            return Ok(HashMap::new());
        }

        let content = std::fs::read_to_string(&info_path)?;
        let info: serde_json::Value =
            serde_json::from_str(&content).map_err(|e| A3sError::Project(e.to_string()))?;

        let mut labels = HashMap::new();

        if let Some(obj) = info.get("labels").and_then(|v| v.as_object()) {
            for (k, v) in obj {
                if let Some(s) = v.as_str() {
                    labels.insert(k.clone(), s.to_string());
                }
            }
        }

        Ok(labels)
    }

    /// Set labels in info.json.
    fn set_labels(sandbox_id: &str, labels: HashMap<String, String>) -> Result<()> {
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

        // Update labels
        if let Some(obj) = info.as_object_mut() {
            let labels_json: std::collections::HashMap<String, serde_json::Value> = labels
                .into_iter()
                .map(|(k, v)| (k, serde_json::json!(v)))
                .collect();
            obj.insert("labels".to_string(), serde_json::json!(labels_json));
        } else {
            return Err(A3sError::Project("invalid info.json format".to_string()));
        }

        let content =
            serde_json::to_string_pretty(&info).map_err(|e| A3sError::Project(e.to_string()))?;
        std::fs::write(&info_path, content)?;

        Ok(())
    }

    /// Parse a label string (KEY=VALUE or KEY-).
    fn parse_label(s: &str) -> Option<(String, Option<String>)> {
        if s.ends_with('-') {
            // KEY- means remove
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

    /// List labels on a resource.
    fn list_labels(&self) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let labels = Self::get_labels(&sandbox_id)?;

        if labels.is_empty() {
            println!(
                "No labels configured for {} '{}'.",
                self.resource_type.as_deref().unwrap_or("resource"),
                self.name
            );
        } else {
            println!(
                "LABELS on {} '{}':",
                self.resource_type.as_deref().unwrap_or("resource"),
                self.name
            );
            for (key, value) in &labels {
                println!("  {}={}", key, value);
            }
        }

        Ok(())
    }

    /// Apply label changes to a resource.
    fn apply_labels(&self) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(&self.name)?;
        let mut labels = Self::get_labels(&sandbox_id)?;

        // Handle label removals first
        for remove_key in &self.remove {
            if labels.remove(remove_key).is_none() {
                println!("Warning: label '{}' not found, ignoring", remove_key);
            } else {
                println!("Removed label '{}' from '{}'", remove_key, self.name);
            }
        }

        // Handle new labels
        for label_str in &self.labels {
            if let Some((key, value_opt)) = Self::parse_label(label_str) {
                if value_opt.is_none() {
                    // KEY- format for removal handled above, but support standalone
                    labels.remove(&key);
                    println!("Removed label '{}' from '{}'", key, self.name);
                } else if let Some(value) = value_opt {
                    if labels.contains_key(&key) && !self.overwrite {
                        return Err(A3sError::Project(format!(
                            "label '{}' already exists, use --overwrite to replace it",
                            key
                        )));
                    }
                    labels.insert(key.clone(), value.clone());
                    println!("Label '{}' set to '{}' on '{}'", key, value, self.name);
                }
            } else {
                return Err(A3sError::Project(format!(
                    "invalid label format: '{}' (expected KEY=VALUE or KEY-)",
                    label_str
                )));
            }
        }

        Self::set_labels(&sandbox_id, labels)?;

        Ok(())
    }
}

#[async_trait]
impl Command for LabelCommand {
    async fn run(&self) -> Result<()> {
        if self.list {
            return self.list_labels();
        }

        if self.labels.is_empty() && self.remove.is_empty() {
            return Err(A3sError::Project(
                "no labels specified (use KEY=VALUE or KEY- for removal)".to_string(),
            ));
        }

        self.apply_labels()
    }
}
