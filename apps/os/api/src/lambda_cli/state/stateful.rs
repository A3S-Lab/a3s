//! Stateful Workload - Stable network IDs and persistent storage.
//!
//! StatefulSets manage stateful applications with stable network identifiers
//! and persistent storage.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// StatefulSet update strategy.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatefulSetUpdateStrategy {
    /// Rolling update with ordered pod management.
    #[default]
    RollingUpdate,
    /// OnDelete - only replace pods that have been manually deleted.
    OnDelete,
}

/// StatefulSet ordering.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatefulSetPersistentVolumeClaimRetentionPolicy {
    /// Retain PVCs when pod is deleted.
    Retain,
    /// Delete PVCs when pod is deleted.
    Delete,
}

impl Default for StatefulSetPersistentVolumeClaimRetentionPolicy {
    fn default() -> Self {
        StatefulSetPersistentVolumeClaimRetentionPolicy::Retain
    }
}

/// StatefulSet selector.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatefulSetSelector {
    #[serde(default)]
    pub match_labels: HashMap<String, String>,
    #[serde(default)]
    pub match_expressions: Vec<StatefulSetSelectorExpression>,
}

/// StatefulSet selector expression.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetSelectorExpression {
    pub key: String,
    pub operator: String,
    #[serde(default)]
    pub values: Vec<String>,
}

/// Volume claim template for StatefulSet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetVolumeClaimTemplate {
    /// Name.
    pub name: String,
    /// Storage class.
    pub storage_class: Option<String>,
    /// Access mode.
    pub access_mode: StatefulSetAccessMode,
    /// Storage size (e.g., "10Gi").
    pub storage: String,
    /// Selector for specific volume.
    #[serde(default)]
    pub selector: Option<StatefulSetSelector>,
}

/// Volume access mode.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum StatefulSetAccessMode {
    ReadWriteOnce,
    ReadOnlyMany,
    ReadWriteMany,
}

impl Default for StatefulSetAccessMode {
    fn default() -> Self {
        StatefulSetAccessMode::ReadWriteOnce
    }
}

/// StatefulSet specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetSpec {
    /// Service name (headless service).
    pub service_name: String,
    /// Pod selector.
    pub selector: StatefulSetSelector,
    /// Pod template.
    pub template: StatefulSetPodTemplate,
    /// Volume claim templates.
    #[serde(default)]
    pub volume_claim_templates: Vec<StatefulSetVolumeClaimTemplate>,
    /// Update strategy.
    #[serde(default)]
    pub update_strategy: StatefulSetUpdateStrategy,
    /// PVC retention policy.
    #[serde(default)]
    pub persistent_volume_claim_retention_policy:
        Option<StatefulSetPersistentVolumeClaimRetentionPolicy>,
    /// Minimum seconds a pod must be ready.
    #[serde(default = "default_min_ready_secs")]
    pub min_ready_secs: u32,
    /// Number of replicas.
    pub replicas: i32,
    /// Revision history limit.
    #[serde(default = "default_revision_history_limit")]
    pub revision_history_limit: i32,
}

fn default_min_ready_secs() -> u32 {
    0
}

fn default_revision_history_limit() -> i32 {
    10
}

/// Pod template for StatefulSet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetPodTemplate {
    /// Metadata.
    pub metadata: StatefulSetPodMetadata,
    /// Pod spec.
    pub spec: StatefulSetPodSpec,
}

/// Pod metadata for StatefulSet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatefulSetPodMetadata {
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// Pod spec for StatefulSet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetPodSpec {
    /// Container image.
    pub image: String,
    /// Command.
    #[serde(default)]
    pub command: Vec<String>,
    /// Arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Port mappings.
    #[serde(default)]
    pub ports: Vec<StatefulSetPortMapping>,
    /// Resource requirements.
    #[serde(default)]
    pub resources: StatefulSetResourceRequirements,
    /// Volume mounts.
    #[serde(default)]
    pub volume_mounts: Vec<StatefulSetVolumeMount>,
    /// Health check.
    #[serde(default)]
    pub health_check: StatefulSetHealthCheck,
}

/// Port mapping for StatefulSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatefulSetPortMapping {
    pub name: String,
    pub container_port: u16,
    pub host_port: u16,
    pub protocol: String,
}

/// Resource requirements for StatefulSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatefulSetResourceRequirements {
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<String>,
    pub memory_request: Option<String>,
    pub cpu_request: Option<String>,
}

/// Volume mount for StatefulSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatefulSetVolumeMount {
    pub name: String,
    pub mount_path: String,
    #[serde(default)]
    pub read_only: bool,
}

/// Health check for StatefulSet pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StatefulSetHealthCheck {
    pub enabled: bool,
    pub path: Option<String>,
    pub port: Option<u16>,
    pub initial_delay_secs: u32,
    pub period_secs: u32,
    pub timeout_secs: u32,
    pub failure_threshold: u32,
}

/// StatefulSet desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetDesired {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Specification.
    pub spec: StatefulSetSpec,
    /// Current version.
    pub version: String,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// StatefulSet actual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetActual {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Current replicas.
    pub replicas: i32,
    /// Ready replicas.
    pub ready_replicas: i32,
    /// Updated replicas.
    pub updated_replicas: i32,
    /// Current revision.
    pub current_revision: String,
    /// Update revision.
    pub update_revision: String,
    /// Pod identities.
    #[serde(default)]
    pub pod_identities: HashMap<i32, StatefulSetPodIdentity>,
    /// Pod conditions.
    #[serde(default)]
    pub conditions: Vec<StatefulSetCondition>,
}

/// StatefulSet pod identity - stable network identifier.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetPodIdentity {
    /// Ordinal index.
    pub ordinal: i32,
    /// Pod name (includes ordinal).
    pub pod_name: String,
    /// DNS name (service-name.index.svc.cluster.local).
    pub dns_name: String,
    /// Hostname (same as pod_name).
    pub hostname: String,
    /// Storage volumes (PVC names).
    #[serde(default)]
    pub volumes: Vec<String>,
    /// PVC info for each volume.
    #[serde(default)]
    pub pvc_info: HashMap<String, VolumeBindingInfo>,
}

/// Volume binding info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeBindingInfo {
    /// PVC name.
    pub pvc_name: String,
    /// Volume name.
    pub volume_name: String,
    /// Capacity.
    pub capacity: String,
    /// Bound.
    pub bound: bool,
}

/// StatefulSet condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetCondition {
    /// Type.
    pub type_: String,
    /// Status (True/False/Unknown).
    pub status: String,
    /// Message.
    pub message: Option<String>,
    /// Last transition time.
    pub last_transition_time: DateTime<Utc>,
}

/// StatefulSet controller.
pub struct StatefulSetController {
    /// StatefulSets.
    statefulsets: RwLock<HashMap<String, HashMap<String, StatefulSetDesired>>>,
    /// Actual states.
    actuals: RwLock<HashMap<String, HashMap<String, StatefulSetActual>>>,
}

impl StatefulSetController {
    /// Create a new StatefulSet controller.
    pub fn new() -> Self {
        Self {
            statefulsets: RwLock::new(HashMap::new()),
            actuals: RwLock::new(HashMap::new()),
        }
    }

    // ==================== StatefulSet Operations ====================

    /// Create a StatefulSet.
    pub async fn create_statefulset(&self, ss: StatefulSetDesired) -> Result<()> {
        // Initialize pod identities
        let mut actual = StatefulSetActual {
            name: ss.name.clone(),
            namespace: ss.namespace.clone(),
            replicas: ss.spec.replicas,
            ready_replicas: 0,
            updated_replicas: 0,
            current_revision: ss.version.clone(),
            update_revision: ss.version.clone(),
            pod_identities: HashMap::new(),
            conditions: vec![],
        };

        // Create pod identities for each replica
        for i in 0..ss.spec.replicas {
            let pod_name = format!("{}-{}", ss.name, i);
            let dns_name = format!(
                "{}-{}.{}.{}.svc.cluster.local",
                ss.name, i, ss.spec.service_name, ss.namespace
            );

            // Create PVC names for each volume claim template
            let mut volumes = Vec::new();
            let mut pvc_info = HashMap::new();

            for claim in &ss.spec.volume_claim_templates {
                let pvc_name = format!("{}-{}-{}", claim.name, ss.name, i);
                let volume_name = format!("{}-{}", claim.name, i);
                volumes.push(volume_name.clone());

                pvc_info.insert(
                    volume_name.clone(),
                    VolumeBindingInfo {
                        pvc_name: pvc_name.clone(),
                        volume_name,
                        capacity: claim.storage.clone(),
                        bound: false,
                    },
                );
            }

            actual.pod_identities.insert(
                i,
                StatefulSetPodIdentity {
                    ordinal: i,
                    pod_name: pod_name.clone(),
                    dns_name: dns_name.clone(),
                    hostname: pod_name,
                    volumes,
                    pvc_info,
                },
            );
        }

        let mut statefulsets = self.statefulsets.write().await;
        let namespace = ss.namespace.clone();
        let name = ss.name.clone();
        let ns_sts = statefulsets
            .entry(namespace.clone())
            .or_insert_with(HashMap::new);
        ns_sts.insert(name.clone(), ss);

        drop(statefulsets);

        // Store actual state
        let mut actuals = self.actuals.write().await;
        let ns_actuals = actuals.entry(namespace).or_insert_with(HashMap::new);
        ns_actuals.insert(name, actual);

        Ok(())
    }

    /// Get a StatefulSet.
    pub async fn get_statefulset(&self, namespace: &str, name: &str) -> Option<StatefulSetDesired> {
        let statefulsets = self.statefulsets.read().await;
        statefulsets
            .get(namespace)
            .and_then(|ns| ns.get(name).cloned())
    }

    /// List all StatefulSets in a namespace.
    pub async fn list_statefulsets(&self, namespace: &str) -> Vec<StatefulSetDesired> {
        let statefulsets = self.statefulsets.read().await;
        statefulsets
            .get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all StatefulSets.
    pub async fn list_all_statefulsets(&self) -> Vec<(String, StatefulSetDesired)> {
        let statefulsets = self.statefulsets.read().await;
        statefulsets
            .iter()
            .flat_map(|(ns, ns_stss)| {
                ns_stss
                    .values()
                    .map(|ss| (ns.clone(), ss.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Update a StatefulSet.
    pub async fn update_statefulset(&self, ss: StatefulSetDesired) -> Result<()> {
        let mut statefulsets = self.statefulsets.write().await;
        if let Some(ns_stss) = statefulsets.get_mut(&ss.namespace) {
            if let Some(existing) = ns_stss.get_mut(&ss.name) {
                *existing = ss;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!(
            "StatefulSet {} not found in namespace {}",
            ss.name, ss.namespace
        )))
    }

    /// Delete a StatefulSet.
    pub async fn delete_statefulset(&self, namespace: &str, name: &str) -> Result<()> {
        let mut statefulsets = self.statefulsets.write().await;
        if let Some(ns_stss) = statefulsets.get_mut(namespace) {
            ns_stss.remove(name);
        }
        Ok(())
    }

    // ==================== Actual State Operations ====================

    /// Get actual state.
    pub async fn get_actual(&self, namespace: &str, name: &str) -> Option<StatefulSetActual> {
        let actuals = self.actuals.read().await;
        actuals.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// Update actual state.
    pub async fn update_actual(&self, actual: StatefulSetActual) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        let ns_actuals = actuals
            .entry(actual.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_actuals.insert(actual.name.clone(), actual);
        Ok(())
    }

    /// Update pod identity.
    pub async fn update_pod_identity(
        &self,
        namespace: &str,
        name: &str,
        ordinal: i32,
        identity: StatefulSetPodIdentity,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.pod_identities.insert(ordinal, identity);
                return Ok(());
            }
        }
        Err(A3sError::Project(format!(
            "StatefulSet {} not found in namespace {}",
            name, namespace
        )))
    }

    /// Set PVC as bound.
    pub async fn mark_pvc_bound(
        &self,
        namespace: &str,
        name: &str,
        ordinal: i32,
        volume_name: &str,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                if let Some(identity) = actual.pod_identities.get_mut(&ordinal) {
                    if let Some(pvc_info) = identity.pvc_info.get_mut(volume_name) {
                        pvc_info.bound = true;
                        return Ok(());
                    }
                }
            }
        }
        Err(A3sError::Project(format!(
            "PVC info not found for StatefulSet {} ordinal {} volume {}",
            name, ordinal, volume_name
        )))
    }

    // ==================== Scheduling ====================

    /// Get the ordinal for a pod name.
    pub fn get_ordinal_from_pod_name(pod_name: &str) -> Option<i32> {
        let parts: Vec<&str> = pod_name.rsplitn(2, '-').collect();
        if parts.len() == 2 {
            parts[0].parse().ok()
        } else {
            None
        }
    }

    /// Check if a pod is the first in the ordinal (for ordered deployment).
    pub async fn is_first_ordinal(&self, namespace: &str, name: &str, ordinal: i32) -> bool {
        if ordinal != 0 {
            // Check if all previous ordinals are ready/terminated
            if let Some(actual) = self.get_actual(namespace, name).await {
                for i in 0..ordinal {
                    if let Some(_identity) = actual.pod_identities.get(&i) {
                        // Pod exists, so not first yet
                        return false;
                    }
                }
            }
        }
        true
    }

    /// Get next ordinal to update during rolling update.
    pub async fn get_next_ordinal_to_update(&self, namespace: &str, name: &str) -> Option<i32> {
        if let Some(actual) = self.get_actual(namespace, name).await {
            if actual.current_revision == actual.update_revision {
                return None; // No update in progress
            }

            // Find first ordinal not yet updated
            for (ordinal, identity) in &actual.pod_identities {
                if identity.pod_name.contains(&actual.update_revision) {
                    continue; // Already updated
                }
                return Some(*ordinal);
            }
        }
        None
    }

    /// Add condition.
    pub async fn add_condition(
        &self,
        namespace: &str,
        name: &str,
        condition: StatefulSetCondition,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                // Remove old condition of same type
                actual.conditions.retain(|c| c.type_ != condition.type_);
                actual.conditions.push(condition);
                return Ok(());
            }
        }
        Err(A3sError::Project(format!(
            "StatefulSet {} not found in namespace {}",
            name, namespace
        )))
    }
}

impl Default for StatefulSetController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_controller() -> StatefulSetController {
        StatefulSetController::new()
    }

    fn test_statefulset() -> StatefulSetDesired {
        StatefulSetDesired {
            name: "mysql".to_string(),
            namespace: "default".to_string(),
            spec: StatefulSetSpec {
                service_name: "mysql".to_string(),
                selector: StatefulSetSelector {
                    match_labels: {
                        let mut m = HashMap::new();
                        m.insert("app".to_string(), "mysql".to_string());
                        m
                    },
                    match_expressions: vec![],
                },
                template: StatefulSetPodTemplate {
                    metadata: StatefulSetPodMetadata {
                        labels: {
                            let mut m = HashMap::new();
                            m.insert("app".to_string(), "mysql".to_string());
                            m
                        },
                        annotations: HashMap::new(),
                    },
                    spec: StatefulSetPodSpec {
                        image: "mysql:8".to_string(),
                        command: vec![],
                        args: vec![],
                        env: HashMap::new(),
                        ports: vec![StatefulSetPortMapping {
                            name: "mysql".to_string(),
                            container_port: 3306,
                            host_port: 0,
                            protocol: "TCP".to_string(),
                        }],
                        resources: Default::default(),
                        volume_mounts: vec![StatefulSetVolumeMount {
                            name: "data".to_string(),
                            mount_path: "/var/lib/mysql".to_string(),
                            read_only: false,
                        }],
                        health_check: Default::default(),
                    },
                },
                volume_claim_templates: vec![StatefulSetVolumeClaimTemplate {
                    name: "data".to_string(),
                    storage_class: Some("standard".to_string()),
                    access_mode: StatefulSetAccessMode::ReadWriteOnce,
                    storage: "10Gi".to_string(),
                    selector: None,
                }],
                update_strategy: StatefulSetUpdateStrategy::RollingUpdate,
                persistent_volume_claim_retention_policy: Some(
                    StatefulSetPersistentVolumeClaimRetentionPolicy::Retain,
                ),
                min_ready_secs: 10,
                replicas: 3,
                revision_history_limit: 10,
            },
            version: "v1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_create_statefulset() {
        let controller = test_controller();
        let ss = test_statefulset();

        controller.create_statefulset(ss.clone()).await.unwrap();

        let retrieved = controller.get_statefulset("default", "mysql").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "mysql");
    }

    #[tokio::test]
    async fn test_pod_identities_created() {
        let controller = test_controller();
        let ss = test_statefulset();

        controller.create_statefulset(ss).await.unwrap();

        let actual = controller.get_actual("default", "mysql").await;
        assert!(actual.is_some());

        let actual = actual.unwrap();
        assert_eq!(actual.pod_identities.len(), 3);

        // Check first identity
        let identity0 = actual.pod_identities.get(&0).unwrap();
        assert_eq!(identity0.pod_name, "mysql-0");
        assert_eq!(
            identity0.dns_name,
            "mysql-0.mysql.default.svc.cluster.local"
        );
        assert_eq!(identity0.hostname, "mysql-0");
        assert_eq!(identity0.volumes.len(), 1);

        // Check PVC info
        let pvc_info = identity0.pvc_info.get("data-0").unwrap();
        assert_eq!(pvc_info.pvc_name, "data-mysql-0");
        assert_eq!(pvc_info.capacity, "10Gi");
        assert!(!pvc_info.bound); // Not yet bound
    }

    #[tokio::test]
    async fn test_ordinal_from_pod_name() {
        assert_eq!(
            StatefulSetController::get_ordinal_from_pod_name("mysql-0"),
            Some(0)
        );
        assert_eq!(
            StatefulSetController::get_ordinal_from_pod_name("mysql-1"),
            Some(1)
        );
        assert_eq!(
            StatefulSetController::get_ordinal_from_pod_name("mysql-10"),
            Some(10)
        );
        assert_eq!(
            StatefulSetController::get_ordinal_from_pod_name("mysql"),
            None
        );
        assert_eq!(
            StatefulSetController::get_ordinal_from_pod_name("invalid"),
            None
        );
    }

    #[tokio::test]
    async fn test_mark_pvc_bound() {
        let controller = test_controller();
        let ss = test_statefulset();

        controller.create_statefulset(ss).await.unwrap();

        // Mark PVC as bound
        controller
            .mark_pvc_bound("default", "mysql", 0, "data-0")
            .await
            .unwrap();

        // Verify bound
        let actual = controller.get_actual("default", "mysql").await.unwrap();
        let pvc_info = actual
            .pod_identities
            .get(&0)
            .unwrap()
            .pvc_info
            .get("data-0")
            .unwrap();
        assert!(pvc_info.bound);
    }

    #[tokio::test]
    async fn test_delete_statefulset() {
        let controller = test_controller();
        let ss = test_statefulset();

        controller.create_statefulset(ss).await.unwrap();

        let retrieved = controller.get_statefulset("default", "mysql").await;
        assert!(retrieved.is_some());

        controller
            .delete_statefulset("default", "mysql")
            .await
            .unwrap();

        let retrieved = controller.get_statefulset("default", "mysql").await;
        assert!(retrieved.is_none());
    }

    #[tokio::test]
    async fn test_add_condition() {
        let controller = test_controller();
        let ss = test_statefulset();

        controller.create_statefulset(ss).await.unwrap();

        let condition = StatefulSetCondition {
            type_: "RollingUpdate".to_string(),
            status: "True".to_string(),
            message: Some("update in progress".to_string()),
            last_transition_time: Utc::now(),
        };

        controller
            .add_condition("default", "mysql", condition)
            .await
            .unwrap();

        let actual = controller.get_actual("default", "mysql").await.unwrap();
        assert_eq!(actual.conditions.len(), 1);
        assert_eq!(actual.conditions[0].type_, "RollingUpdate");
        assert_eq!(actual.conditions[0].status, "True");
    }
}
