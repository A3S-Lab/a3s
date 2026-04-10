//! Multi-namespace support for resource isolation.
//!
//! Provides namespace-scoped resource management, similar to Kubernetes namespaces.

use crate::errors::{A3sError, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;

/// Namespace phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum NamespacePhase {
    /// Namespace is active.
    Active,
    /// Namespace is terminating.
    Terminating,
    /// Namespace has been deleted.
    Deleted,
}

/// Namespace metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceMeta {
    pub name: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}

/// Namespace - represents an isolated compute context.
#[derive(Debug, Clone)]
pub struct Namespace {
    pub meta: NamespaceMeta,
    pub phase: NamespacePhase,
    /// Resource quotas for this namespace.
    quotas: Arc<RwLock<Option<ResourceQuota>>>,
}

impl Namespace {
    /// Create a new namespace.
    pub fn new(name: String) -> Self {
        let now = chrono::Utc::now();
        Self {
            meta: NamespaceMeta {
                name: name.clone(),
                labels: HashMap::new(),
                annotations: HashMap::new(),
                created_at: now,
                updated_at: now,
            },
            phase: NamespacePhase::Active,
            quotas: Arc::new(RwLock::new(None)),
        }
    }

    /// Create a namespace with metadata.
    pub fn with_meta(meta: NamespaceMeta) -> Self {
        Self {
            meta,
            phase: NamespacePhase::Active,
            quotas: Arc::new(RwLock::new(None)),
        }
    }

    /// Set resource quota.
    pub async fn set_quota(&self, quota: ResourceQuota) {
        let mut quotas = self.quotas.write().await;
        *quotas = Some(quota);
    }

    /// Get resource quota.
    pub async fn get_quota(&self) -> Option<ResourceQuota> {
        let quotas = self.quotas.read().await;
        quotas.clone()
    }

    /// Mark namespace as terminating.
    pub fn terminate(&mut self) {
        self.phase = NamespacePhase::Terminating;
        self.meta.updated_at = chrono::Utc::now();
    }
}

/// Resource quota for a namespace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceQuota {
    /// Maximum number of pods.
    pub max_pods: Option<i32>,
    /// Maximum total CPU millicores.
    pub max_cpu_millicores: Option<i32>,
    /// Maximum total memory in bytes.
    pub max_memory_bytes: Option<i64>,
    /// Maximum number of services.
    pub max_services: Option<i32>,
    /// Maximum number of deployments.
    pub max_deployments: Option<i32>,
}

impl Default for ResourceQuota {
    fn default() -> Self {
        Self {
            max_pods: Some(100),
            max_cpu_millicores: Some(10000), // 10 cores
            max_memory_bytes: Some(16 * 1024 * 1024 * 1024), // 16GB
            max_services: Some(50),
            max_deployments: Some(50),
        }
    }
}

impl ResourceQuota {
    /// Check if a new resource request would exceed quota.
    pub fn check_pods(&self, current_pods: i32, requested: i32) -> Result<()> {
        if let Some(max) = self.max_pods {
            if current_pods + requested > max {
                return Err(A3sError::Project(format!(
                    "pod quota exceeded: {} + {} > {}",
                    current_pods, requested, max
                )));
            }
        }
        Ok(())
    }

    /// Check if CPU request would exceed quota.
    pub fn check_cpu(&self, current_millicores: i32, requested_millicores: i32) -> Result<()> {
        if let Some(max) = self.max_cpu_millicores {
            if current_millicores + requested_millicores > max {
                return Err(A3sError::Project(format!(
                    "CPU quota exceeded: {} + {} > {}",
                    current_millicores, requested_millicores, max
                )));
            }
        }
        Ok(())
    }

    /// Check if memory request would exceed quota.
    pub fn check_memory(&self, current_bytes: i64, requested_bytes: i64) -> Result<()> {
        if let Some(max) = self.max_memory_bytes {
            if current_bytes + requested_bytes > max {
                return Err(A3sError::Project(format!(
                    "memory quota exceeded: {} + {} > {}",
                    current_bytes, requested_bytes, max
                )));
            }
        }
        Ok(())
    }
}

/// Namespace registry - manages all namespaces.
pub struct NamespaceRegistry {
    namespaces: RwLock<HashMap<String, Arc<tokio::sync::RwLock<Namespace>>>>,
}

impl NamespaceRegistry {
    pub fn new() -> Self {
        Self {
            namespaces: RwLock::new(HashMap::new()),
        }
    }

    /// Create a new namespace.
    pub async fn create(&self, name: String) -> Result<Arc<tokio::sync::RwLock<Namespace>>> {
        let mut namespaces = self.namespaces.write().await;

        if namespaces.contains_key(&name) {
            return Err(A3sError::Project(format!(
                "namespace '{}' already exists",
                name
            )));
        }

        let ns = Arc::new(tokio::sync::RwLock::new(Namespace::new(name.clone())));
        namespaces.insert(name, ns.clone());
        Ok(ns)
    }

    /// Create a namespace with metadata.
    pub async fn create_with_meta(
        &self,
        meta: NamespaceMeta,
    ) -> Result<Arc<tokio::sync::RwLock<Namespace>>> {
        let mut namespaces = self.namespaces.write().await;
        let name = meta.name.clone();

        if namespaces.contains_key(&name) {
            return Err(A3sError::Project(format!(
                "namespace '{}' already exists",
                name
            )));
        }

        let ns = Arc::new(tokio::sync::RwLock::new(Namespace::with_meta(meta)));
        namespaces.insert(name, ns.clone());
        Ok(ns)
    }

    /// Get a namespace by name.
    pub async fn get(&self, name: &str) -> Result<Arc<tokio::sync::RwLock<Namespace>>> {
        let namespaces = self.namespaces.read().await;
        namespaces
            .get(name)
            .cloned()
            .ok_or_else(|| A3sError::Project(format!("namespace '{}' not found", name)))
    }

    /// List all namespaces.
    pub async fn list(&self) -> Vec<Arc<tokio::sync::RwLock<Namespace>>> {
        let namespaces = self.namespaces.read().await;
        namespaces.values().cloned().collect()
    }

    /// Delete a namespace.
    pub async fn delete(&self, name: &str) -> Result<()> {
        let mut namespaces = self.namespaces.write().await;

        if let Some(ns) = namespaces.get(name) {
            let mut guard = ns.write().await;
            guard.terminate();
        }

        // Remove after marking as terminating (allows status checks)
        namespaces.remove(name);
        Ok(())
    }

    /// Check if namespace exists.
    pub async fn exists(&self, name: &str) -> bool {
        let namespaces = self.namespaces.read().await;
        namespaces.contains_key(name)
    }
}

impl Default for NamespaceRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Parse resource string like "100m" (CPU) or "1Gi" (memory) to bytes or millicores.
pub fn parse_cpu_to_millicores(cpu: &str) -> Result<i32> {
    let cpu = cpu.trim();
    if cpu.ends_with('m') {
        // e.g., "100m" = 100 millicores (already in milliunits, no conversion)
        let value = cpu[..cpu.len() - 1]
            .parse::<f64>()
            .map_err(|e| A3sError::Project(format!("invalid CPU value '{}': {}", cpu, e)))?;
        Ok(value.round() as i32)
    } else {
        // e.g., "1" = 1 core = 1000 millicores, or "2.5" = 2.5 cores
        let value = cpu
            .parse::<f64>()
            .map_err(|e| A3sError::Project(format!("invalid CPU value '{}': {}", cpu, e)))?;
        Ok((value * 1000.0).round() as i32)
    }
}

pub fn parse_memory_to_bytes(memory: &str) -> Result<i64> {
    let memory = memory.trim();
    let multiplier = if memory.ends_with("Ki") {
        1024
    } else if memory.ends_with("Mi") {
        1024 * 1024
    } else if memory.ends_with("Gi") {
        1024 * 1024 * 1024
    } else if memory.ends_with("K") {
        1000
    } else if memory.ends_with("M") {
        1000 * 1000
    } else if memory.ends_with("G") {
        1000 * 1000 * 1000
    } else {
        return Err(A3sError::Project(format!(
            "unknown memory unit in '{}'",
            memory
        )));
    };

    let value_str = if memory.ends_with("Ki")
        || memory.ends_with("Mi")
        || memory.ends_with("Gi")
        || memory.ends_with("K")
        || memory.ends_with("M")
        || memory.ends_with("G")
    {
        &memory[..memory.len() - 2]
    } else {
        return Err(A3sError::Project(format!(
            "unknown memory unit in '{}'",
            memory
        )));
    };

    let value: i64 = value_str
        .parse()
        .map_err(|e| A3sError::Project(format!("invalid memory value '{}': {}", memory, e)))?;

    Ok(value * multiplier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_cpu() {
        assert_eq!(parse_cpu_to_millicores("100m").unwrap(), 100);
        assert_eq!(parse_cpu_to_millicores("1").unwrap(), 1000);
        assert_eq!(parse_cpu_to_millicores("2.5").unwrap(), 2500);
    }

    #[test]
    fn test_parse_memory() {
        assert_eq!(parse_memory_to_bytes("1Ki").unwrap(), 1024);
        assert_eq!(parse_memory_to_bytes("1Mi").unwrap(), 1024 * 1024);
        assert_eq!(parse_memory_to_bytes("1Gi").unwrap(), 1024 * 1024 * 1024);
    }

    #[tokio::test]
    async fn test_namespace_crud() {
        let registry = NamespaceRegistry::new();

        // Create
        let ns = registry.create("test".to_string()).await.unwrap();
        assert_eq!(ns.read().await.meta.name, "test");

        // Get
        let ns2 = registry.get("test").await.unwrap();
        assert_eq!(ns2.read().await.meta.name, "test");

        // Exists
        assert!(registry.exists("test").await);
        assert!(!registry.exists("other").await);

        // Delete
        registry.delete("test").await.unwrap();
        assert!(!registry.exists("test").await);
    }

    #[tokio::test]
    async fn test_quota_checking() {
        let quota = ResourceQuota::default();

        // Should pass - 50 + 10 = 60 < 100
        quota.check_pods(50, 10).unwrap();
        // Should fail - 91 + 10 = 101 > 100
        quota.check_pods(91, 10).unwrap_err();

        // Should pass - 5000 + 2000 = 7000 < 10000
        quota.check_cpu(5000, 2000).unwrap();
        // Should fail - 9000 + 2000 = 11000 > 10000
        quota.check_cpu(9000, 2000).unwrap_err();
    }
}
