//! RuntimeClass - Container Runtime Configuration.
//!
//! RuntimeClass defines a set of container runtime configurations that can be
//! used to run pods. This enables support for multiple runtime profiles
//! (e.g., Kata Containers for security isolation, gVisor for additional sandboxing).

use crate::errors::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Runtime handler - the underlying runtime implementation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RuntimeHandler {
    /// Native runtime (runnc/docker).
    Native,
    /// Kata containers runtime.
    Kata,
    /// gVisor runtime.
    Gvisor,
    /// gVisor runsc runtime.
    Runsc,
    /// Custom handler name.
    Custom,
}

impl Default for RuntimeHandler {
    fn default() -> Self {
        RuntimeHandler::Native
    }
}

/// RuntimeClass spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeClassSpec {
    /// Runtime handler name.
    pub handler: String,
    /// Runtime overhead configuration.
    pub overhead: Option<RuntimeOverhead>,
    /// Scheduling configuration.
    pub scheduling: Option<Scheduling>,
}

/// Runtime overhead - resource overhead for the runtime sandbox.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeOverhead {
    /// Pod overhead in bytes.
    pub pod_limit: HashMap<String, i64>,
}

/// Scheduling constraints for pods using this RuntimeClass.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Scheduling {
    /// Node selector to restrict which nodes this RuntimeClass can use.
    pub node_selector: HashMap<String, String>,
    /// Tolerations for pods using this RuntimeClass.
    pub tolerations: Vec<String>,
}

/// RuntimeClass represents a runtime configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeClass {
    /// Metadata.
    pub metadata: RuntimeClassMeta,
    /// Spec.
    pub spec: RuntimeClassSpec,
}

/// RuntimeClass metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeClassMeta {
    /// Name.
    pub name: String,
    /// Labels.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Annotations.
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// RuntimeClass controller manages runtime configurations.
pub struct RuntimeClassController {
    /// Runtime classes by name.
    classes: RwLock<HashMap<String, RuntimeClass>>,
    /// Default runtime handler.
    default_handler: String,
}

impl RuntimeClassController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            classes: RwLock::new(HashMap::new()),
            default_handler: "native".to_string(),
        }
    }

    /// Set a runtime class.
    pub async fn set(&self, rc: RuntimeClass) -> Result<()> {
        let mut classes = self.classes.write().await;
        classes.insert(rc.metadata.name.clone(), rc);
        Ok(())
    }

    /// Get a runtime class.
    pub async fn get(&self, name: &str) -> Option<RuntimeClass> {
        let classes = self.classes.read().await;
        classes.get(name).cloned()
    }

    /// Delete a runtime class.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let mut classes = self.classes.write().await;
        classes.remove(name);
        Ok(())
    }

    /// List all runtime classes.
    pub async fn list(&self) -> Vec<RuntimeClass> {
        let classes = self.classes.read().await;
        classes.values().cloned().collect()
    }

    /// Check if a handler exists.
    pub async fn handler_exists(&self, handler: &str) -> bool {
        let classes = self.classes.read().await;
        classes.values().any(|rc| rc.spec.handler == handler)
    }

    /// Get default handler name.
    pub fn default_handler(&self) -> &str {
        &self.default_handler
    }
}

impl Default for RuntimeClassController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_runtime_class() {
        let controller = RuntimeClassController::new();

        let rc = RuntimeClass {
            metadata: RuntimeClassMeta {
                name: "Kata".to_string(),
                labels: HashMap::new(),
                annotations: HashMap::new(),
                created_at: Utc::now(),
            },
            spec: RuntimeClassSpec {
                handler: "kata".to_string(),
                overhead: None,
                scheduling: None,
            },
        };

        controller.set(rc.clone()).await.unwrap();

        let found = controller.get("Kata").await;
        assert!(found.is_some());
        assert_eq!(found.unwrap().spec.handler, "kata");
    }

    #[tokio::test]
    async fn test_handler_exists() {
        let controller = RuntimeClassController::new();

        let rc = RuntimeClass {
            metadata: RuntimeClassMeta {
                name: "gvisor".to_string(),
                labels: HashMap::new(),
                annotations: HashMap::new(),
                created_at: Utc::now(),
            },
            spec: RuntimeClassSpec {
                handler: "gvisor".to_string(),
                overhead: None,
                scheduling: None,
            },
        };

        controller.set(rc).await.unwrap();

        assert!(controller.handler_exists("gvisor").await);
        assert!(!controller.handler_exists("kata").await);
    }
}
