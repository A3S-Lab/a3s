//! State store for the control plane.
//!
//! Persists desired state and actual state to disk.
//! Uses atomic writes to prevent corruption.

use crate::errors::{A3sError, Result};
use crate::state::batch::{CronJobDesired, JobDesired};
use crate::state::daemon::DaemonSetDesired;
use crate::state::event::Event;
use crate::state::namespace::NamespaceMeta;
use crate::state::network_policy::NetworkPolicyDesired;
use crate::state::pdb::PodDisruptionBudgetDesired;
use crate::state::rbac::{
    ClusterRoleBindingDesired, ClusterRoleDesired, RoleBindingDesired, RoleDesired,
};
use crate::state::service_account::ServiceAccountDesired;
use crate::state::stateful::StatefulSetDesired;
use crate::state::types::{
    ConfigMapDesired, DeploymentDesired, HealthStatus, IngressDesired, PodActual, PodStatus,
    ReplicaSetDesired, RollingUpdateState, SecretDesired, ServiceActual, ServiceDesired,
};
use crate::state::volume::{
    PersistentVolumeClaimDesired, PersistentVolumeDesired, StorageClassDesired,
};
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;

/// State store directory.
const STATE_DIR: &str = ".a3s/state";

/// Main state store for all control plane state.
#[derive(Debug, Clone)]
pub struct StateStore {
    /// Path to state directory.
    path: PathBuf,
}

impl StateStore {
    /// Create a new state store.
    pub fn new(working_dir: &PathBuf) -> Self {
        let path = working_dir.join(STATE_DIR);
        Self { path }
    }

    /// Ensure state directory exists.
    pub fn ensure_dir(&self) -> Result<()> {
        std::fs::create_dir_all(&self.path)
            .map_err(|e| A3sError::Project(format!("failed to create state directory: {}", e)))?;
        Ok(())
    }

    // ==================== Deployment Desired ====================

    /// Path to deployments desired state file.
    fn deployments_path(&self) -> PathBuf {
        self.path.join("deployments_desired.json")
    }

    /// Load all deployment desired states.
    pub fn load_deployments(&self) -> Result<HashMap<String, DeploymentDesired>> {
        let path = self.deployments_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, DeploymentDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse deployments state: {}", e)))?;
        Ok(map)
    }

    /// Save all deployment desired states.
    pub fn save_deployments(&self, deployments: &HashMap<String, DeploymentDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.deployments_path();
        let content = serde_json::to_string_pretty(deployments).map_err(|e| {
            A3sError::Project(format!("failed to serialize deployments state: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Get a specific deployment desired state.
    pub fn get_deployment(&self, name: &str) -> Result<Option<DeploymentDesired>> {
        let deployments = self.load_deployments()?;
        Ok(deployments.get(name).cloned())
    }

    /// Set a deployment desired state.
    pub fn set_deployment(&self, deployment: DeploymentDesired) -> Result<()> {
        let mut deployments = self.load_deployments()?;
        deployments.insert(deployment.name.clone(), deployment);
        self.save_deployments(&deployments)
    }

    /// Delete a deployment desired state.
    pub fn delete_deployment(&self, name: &str) -> Result<()> {
        let mut deployments = self.load_deployments()?;
        deployments.remove(name);
        self.save_deployments(&deployments)
    }

    // ==================== DaemonSet Desired ====================

    /// Path to daemonsets desired state file.
    fn daemonsets_path(&self) -> PathBuf {
        self.path.join("daemonsets_desired.json")
    }

    /// Load all daemonset desired states.
    pub fn load_daemonsets(&self) -> Result<HashMap<String, DaemonSetDesired>> {
        let path = self.daemonsets_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, DaemonSetDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse daemonsets state: {}", e)))?;
        Ok(map)
    }

    /// Save all daemonset desired states.
    pub fn save_daemonsets(&self, daemonsets: &HashMap<String, DaemonSetDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.daemonsets_path();
        let content = serde_json::to_string_pretty(daemonsets).map_err(|e| {
            A3sError::Project(format!("failed to serialize daemonsets state: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Get a specific daemonset desired state.
    pub fn get_daemonset(&self, name: &str) -> Result<Option<DaemonSetDesired>> {
        let daemonsets = self.load_daemonsets()?;
        Ok(daemonsets.get(name).cloned())
    }

    /// Set a daemonset desired state.
    pub fn set_daemonset(&self, daemonset: DaemonSetDesired) -> Result<()> {
        let mut daemonsets = self.load_daemonsets()?;
        daemonsets.insert(daemonset.name.clone(), daemonset);
        self.save_daemonsets(&daemonsets)
    }

    /// Delete a daemonset desired state.
    pub fn delete_daemonset(&self, name: &str) -> Result<()> {
        let mut daemonsets = self.load_daemonsets()?;
        daemonsets.remove(name);
        self.save_daemonsets(&daemonsets)
    }

    // ==================== Pod Actual ====================

    /// Path to pods actual state file.
    fn pods_path(&self) -> PathBuf {
        self.path.join("pods_actual.json")
    }

    /// Load all pod actual states.
    pub fn load_pods(&self) -> Result<HashMap<String, PodActual>> {
        let path = self.pods_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, PodActual> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse pods state: {}", e)))?;
        Ok(map)
    }

    /// Save all pod actual states.
    pub fn save_pods(&self, pods: &HashMap<String, PodActual>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.pods_path();
        let content = serde_json::to_string_pretty(pods)
            .map_err(|e| A3sError::Project(format!("failed to serialize pods state: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Get pods by deployment name.
    pub fn get_pods_by_deployment(&self, deployment: &str) -> Result<Vec<PodActual>> {
        let pods = self.load_pods()?;
        Ok(pods
            .into_values()
            .filter(|p| p.deployment == deployment)
            .collect())
    }

    /// Get a specific pod.
    pub fn get_pod(&self, pod_id: &str) -> Result<Option<PodActual>> {
        let pods = self.load_pods()?;
        Ok(pods.get(pod_id).cloned())
    }

    /// Set a pod actual state.
    pub fn set_pod(&self, pod: PodActual) -> Result<()> {
        let mut pods = self.load_pods()?;
        pods.insert(pod.id.clone(), pod);
        self.save_pods(&pods)
    }

    /// Delete a pod.
    pub fn delete_pod(&self, pod_id: &str) -> Result<()> {
        let mut pods = self.load_pods()?;
        pods.remove(pod_id);
        self.save_pods(&pods)
    }

    /// Update pod health status.
    pub fn update_pod_health(&self, pod_id: &str, healthy: bool) -> Result<Option<PodActual>> {
        let mut pods = self.load_pods()?;
        if let Some(pod) = pods.get_mut(pod_id) {
            pod.last_health_check = Some(Utc::now());
            if healthy {
                pod.consecutive_failures = 0;
                pod.health = HealthStatus::Healthy;
                pod.ready = true;
            } else {
                pod.consecutive_failures += 1;
                if pod.consecutive_failures >= 3 {
                    pod.health = HealthStatus::Unhealthy;
                    pod.ready = false;
                }
            }
            let pod = pod.clone();
            self.save_pods(&pods)?;
            Ok(Some(pod))
        } else {
            Ok(None)
        }
    }

    // ==================== Service Desired ====================

    /// Path to services desired state file.
    fn services_path(&self) -> PathBuf {
        self.path.join("services_desired.json")
    }

    /// Load all service desired states.
    pub fn load_services(&self) -> Result<HashMap<String, ServiceDesired>> {
        let path = self.services_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, ServiceDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse services state: {}", e)))?;
        Ok(map)
    }

    /// Save all service desired states.
    pub fn save_services(&self, services: &HashMap<String, ServiceDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.services_path();
        let content = serde_json::to_string_pretty(services)
            .map_err(|e| A3sError::Project(format!("failed to serialize services state: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a service.
    pub fn set_service(&self, service: ServiceDesired) -> Result<()> {
        let mut services = self.load_services()?;
        services.insert(service.name.clone(), service);
        self.save_services(&services)
    }

    /// Delete a service.
    pub fn delete_service(&self, name: &str) -> Result<()> {
        let mut services = self.load_services()?;
        services.remove(name);
        self.save_services(&services)
    }

    // ==================== Service Actual ====================

    /// Path to services actual state file.
    fn services_actual_path(&self) -> PathBuf {
        self.path.join("services_actual.json")
    }

    /// Load all service actual states.
    pub fn load_services_actual(&self) -> Result<HashMap<String, ServiceActual>> {
        let path = self.services_actual_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, ServiceActual> = serde_json::from_str(&content).map_err(|e| {
            A3sError::Project(format!("failed to parse services actual state: {}", e))
        })?;
        Ok(map)
    }

    /// Save all service actual states.
    pub fn save_services_actual(&self, services: &HashMap<String, ServiceActual>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.services_actual_path();
        let content = serde_json::to_string_pretty(services).map_err(|e| {
            A3sError::Project(format!("failed to serialize services actual state: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Update service endpoints.
    pub fn update_service_endpoints(&self, name: &str, endpoints: Vec<String>) -> Result<()> {
        let mut services = self.load_services_actual()?;
        services.insert(
            name.to_string(),
            ServiceActual {
                name: name.to_string(),
                namespace: "default".to_string(),
                endpoints,
                load_balancer_ip: None,
            },
        );
        self.save_services_actual(&services)
    }

    // ==================== Rolling Update State ====================

    /// Path to rolling update state file.
    fn rolling_updates_path(&self) -> PathBuf {
        self.path.join("rolling_updates.json")
    }

    /// Load all rolling update states.
    pub fn load_rolling_updates(&self) -> Result<HashMap<String, RollingUpdateState>> {
        let path = self.rolling_updates_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, RollingUpdateState> =
            serde_json::from_str(&content).map_err(|e| {
                A3sError::Project(format!("failed to parse rolling updates state: {}", e))
            })?;
        Ok(map)
    }

    /// Save all rolling update states.
    pub fn save_rolling_updates(
        &self,
        updates: &HashMap<String, RollingUpdateState>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.rolling_updates_path();
        let content = serde_json::to_string_pretty(updates).map_err(|e| {
            A3sError::Project(format!("failed to serialize rolling updates state: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Get rolling update state for a deployment.
    pub fn get_rolling_update(&self, deployment: &str) -> Result<Option<RollingUpdateState>> {
        let updates = self.load_rolling_updates()?;
        Ok(updates.get(deployment).cloned())
    }

    /// Set rolling update state for a deployment.
    pub fn set_rolling_update(&self, state: RollingUpdateState) -> Result<()> {
        let mut updates = self.load_rolling_updates()?;
        updates.insert(state.deployment.clone(), state);
        self.save_rolling_updates(&updates)
    }

    /// Delete rolling update state for a deployment.
    pub fn delete_rolling_update(&self, deployment: &str) -> Result<()> {
        let mut updates = self.load_rolling_updates()?;
        updates.remove(deployment);
        self.save_rolling_updates(&updates)
    }

    // ==================== ConfigMap Desired ====================

    /// Path to configmaps desired state file.
    fn configmaps_path(&self) -> PathBuf {
        self.path.join("configmaps_desired.json")
    }

    /// Load all configmap desired states.
    pub fn load_configmaps(&self) -> Result<HashMap<String, ConfigMapDesired>> {
        let path = self.configmaps_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, ConfigMapDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse configmaps state: {}", e)))?;
        Ok(map)
    }

    /// Save all configmap desired states.
    pub fn save_configmaps(&self, configmaps: &HashMap<String, ConfigMapDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.configmaps_path();
        let content = serde_json::to_string_pretty(configmaps).map_err(|e| {
            A3sError::Project(format!("failed to serialize configmaps state: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Set a configmap.
    pub fn set_configmap(&self, configmap: ConfigMapDesired) -> Result<()> {
        let mut configmaps = self.load_configmaps()?;
        configmaps.insert(configmap.name.clone(), configmap);
        self.save_configmaps(&configmaps)
    }

    /// Get a configmap by name.
    pub fn get_configmap(&self, name: &str) -> Result<Option<ConfigMapDesired>> {
        let configmaps = self.load_configmaps()?;
        Ok(configmaps.get(name).cloned())
    }

    /// Delete a configmap.
    pub fn delete_configmap(&self, name: &str) -> Result<()> {
        let mut configmaps = self.load_configmaps()?;
        configmaps.remove(name);
        self.save_configmaps(&configmaps)
    }

    // ==================== Secret Desired ====================

    /// Path to secrets desired state file.
    fn secrets_path(&self) -> PathBuf {
        self.path.join("secrets_desired.json")
    }

    /// Load all secret desired states.
    pub fn load_secrets(&self) -> Result<HashMap<String, SecretDesired>> {
        let path = self.secrets_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, SecretDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse secrets state: {}", e)))?;
        Ok(map)
    }

    /// Save all secret desired states.
    pub fn save_secrets(&self, secrets: &HashMap<String, SecretDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.secrets_path();
        let content = serde_json::to_string_pretty(secrets)
            .map_err(|e| A3sError::Project(format!("failed to serialize secrets state: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a secret.
    pub fn set_secret(&self, secret: SecretDesired) -> Result<()> {
        let mut secrets = self.load_secrets()?;
        secrets.insert(secret.name.clone(), secret);
        self.save_secrets(&secrets)
    }

    /// Get a secret by name.
    pub fn get_secret(&self, name: &str) -> Result<Option<SecretDesired>> {
        let secrets = self.load_secrets()?;
        Ok(secrets.get(name).cloned())
    }

    /// Delete a secret.
    pub fn delete_secret(&self, name: &str) -> Result<()> {
        let mut secrets = self.load_secrets()?;
        secrets.remove(name);
        self.save_secrets(&secrets)
    }

    // ==================== Node Desired ====================

    /// Path to nodes desired state file.
    fn nodes_path(&self) -> PathBuf {
        self.path.join("nodes_desired.json")
    }

    /// Load all node desired states.
    pub fn load_nodes(&self) -> Result<HashMap<String, crate::state::NodeDesired>> {
        let path = self.nodes_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, crate::state::NodeDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse nodes state: {}", e)))?;
        Ok(map)
    }

    /// Save all node desired states.
    pub fn save_nodes(&self, nodes: &HashMap<String, crate::state::NodeDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.nodes_path();
        let content = serde_json::to_string_pretty(nodes)
            .map_err(|e| A3sError::Project(format!("failed to serialize nodes state: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Get a specific node desired state.
    pub fn get_node(&self, name: &str) -> Result<Option<crate::state::NodeDesired>> {
        let nodes = self.load_nodes()?;
        Ok(nodes.get(name).cloned())
    }

    /// Set a node desired state.
    pub fn set_node(&self, node: crate::state::NodeDesired) -> Result<()> {
        let mut nodes = self.load_nodes()?;
        nodes.insert(node.name.clone(), node);
        self.save_nodes(&nodes)
    }

    /// Delete a node.
    pub fn delete_node(&self, name: &str) -> Result<()> {
        let mut nodes = self.load_nodes()?;
        nodes.remove(name);
        self.save_nodes(&nodes)
    }

    /// List all node names.
    pub fn list_nodes(&self) -> Result<Vec<String>> {
        let nodes = self.load_nodes()?;
        Ok(nodes.keys().cloned().collect())
    }

    // ==================== Namespace ====================

    /// Path to namespaces state file.
    fn namespaces_path(&self) -> PathBuf {
        self.path.join("namespaces.json")
    }

    /// Load all namespaces.
    pub fn load_namespaces(&self) -> Result<HashMap<String, NamespaceMeta>> {
        let path = self.namespaces_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, NamespaceMeta> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse namespaces: {}", e)))?;
        Ok(map)
    }

    /// Save all namespaces.
    pub fn save_namespaces(&self, namespaces: &HashMap<String, NamespaceMeta>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.namespaces_path();
        let content = serde_json::to_string_pretty(namespaces)
            .map_err(|e| A3sError::Project(format!("failed to serialize namespaces: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a namespace.
    pub fn set_namespace(&self, namespace: NamespaceMeta) -> Result<()> {
        let mut namespaces = self.load_namespaces()?;
        namespaces.insert(namespace.name.clone(), namespace);
        self.save_namespaces(&namespaces)
    }

    /// Get a namespace.
    pub fn get_namespace(&self, name: &str) -> Result<Option<NamespaceMeta>> {
        let namespaces = self.load_namespaces()?;
        Ok(namespaces.get(name).cloned())
    }

    /// Delete a namespace.
    pub fn delete_namespace(&self, name: &str) -> Result<()> {
        let mut namespaces = self.load_namespaces()?;
        namespaces.remove(name);
        self.save_namespaces(&namespaces)
    }

    // ==================== Events ====================

    /// Path to events state file.
    fn events_path(&self) -> PathBuf {
        self.path.join("events.json")
    }

    /// Load all events.
    pub fn load_events(&self) -> Result<Vec<Event>> {
        let path = self.events_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let events: Vec<Event> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse events: {}", e)))?;
        Ok(events)
    }

    /// Save all events.
    pub fn save_events(&self, events: &[Event]) -> Result<()> {
        self.ensure_dir()?;
        let path = self.events_path();
        let content = serde_json::to_string_pretty(events)
            .map_err(|e| A3sError::Project(format!("failed to serialize events: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Add an event.
    pub fn add_event(&self, event: Event) -> Result<()> {
        let mut events = self.load_events()?;
        events.push(event);
        // Keep only last 1000 events
        if events.len() > 1000 {
            events = events.into_iter().skip(1000).collect();
        }
        self.save_events(&events)
    }

    // ==================== Ingress ====================

    /// Path to ingresses state file.
    fn ingresses_path(&self) -> PathBuf {
        self.path.join("ingresses.json")
    }

    /// Load all ingresses (by namespace).
    pub fn load_ingresses(&self) -> Result<HashMap<String, HashMap<String, IngressDesired>>> {
        let path = self.ingresses_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, IngressDesired>> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse ingresses: {}", e)))?;
        Ok(map)
    }

    /// Save all ingresses.
    pub fn save_ingresses(
        &self,
        ingresses: &HashMap<String, HashMap<String, IngressDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.ingresses_path();
        let content = serde_json::to_string_pretty(ingresses)
            .map_err(|e| A3sError::Project(format!("failed to serialize ingresses: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set an ingress.
    pub fn set_ingress(&self, ingress: IngressDesired) -> Result<()> {
        let mut ingresses = self.load_ingresses()?;
        ingresses
            .entry(ingress.namespace.clone())
            .or_insert_with(HashMap::new);
        ingresses
            .get_mut(&ingress.namespace)
            .unwrap()
            .insert(ingress.name.clone(), ingress);
        self.save_ingresses(&ingresses)
    }

    /// Get an ingress.
    pub fn get_ingress(&self, namespace: &str, name: &str) -> Result<Option<IngressDesired>> {
        let ingresses = self.load_ingresses()?;
        Ok(ingresses.get(namespace).and_then(|i| i.get(name).cloned()))
    }

    /// Delete an ingress.
    pub fn delete_ingress(&self, namespace: &str, name: &str) -> Result<()> {
        let mut ingresses = self.load_ingresses()?;
        if let Some(ns_ing) = ingresses.get_mut(namespace) {
            ns_ing.remove(name);
        }
        self.save_ingresses(&ingresses)
    }

    // ==================== StatefulSet ====================

    /// Path to statefulsets state file.
    fn statefulsets_path(&self) -> PathBuf {
        self.path.join("statefulsets.json")
    }

    /// Load all statefulsets (by namespace).
    pub fn load_statefulsets(
        &self,
    ) -> Result<HashMap<String, HashMap<String, StatefulSetDesired>>> {
        let path = self.statefulsets_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, StatefulSetDesired>> =
            serde_json::from_str(&content)
                .map_err(|e| A3sError::Project(format!("failed to parse statefulsets: {}", e)))?;
        Ok(map)
    }

    /// Save all statefulsets.
    pub fn save_statefulsets(
        &self,
        statefulsets: &HashMap<String, HashMap<String, StatefulSetDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.statefulsets_path();
        let content = serde_json::to_string_pretty(statefulsets)
            .map_err(|e| A3sError::Project(format!("failed to serialize statefulsets: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a statefulset.
    pub fn set_statefulset(&self, statefulset: StatefulSetDesired) -> Result<()> {
        let mut statefulsets = self.load_statefulsets()?;
        statefulsets
            .entry(statefulset.namespace.clone())
            .or_insert_with(HashMap::new);
        statefulsets
            .get_mut(&statefulset.namespace)
            .unwrap()
            .insert(statefulset.name.clone(), statefulset);
        self.save_statefulsets(&statefulsets)
    }

    /// Get a statefulset.
    pub fn get_statefulset(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<StatefulSetDesired>> {
        let statefulsets = self.load_statefulsets()?;
        Ok(statefulsets
            .get(namespace)
            .and_then(|s| s.get(name).cloned()))
    }

    /// Delete a statefulset.
    pub fn delete_statefulset(&self, namespace: &str, name: &str) -> Result<()> {
        let mut statefulsets = self.load_statefulsets()?;
        if let Some(ns_ss) = statefulsets.get_mut(namespace) {
            ns_ss.remove(name);
        }
        self.save_statefulsets(&statefulsets)
    }

    // ==================== Job ====================

    /// Path to jobs state file.
    fn jobs_path(&self) -> PathBuf {
        self.path.join("jobs.json")
    }

    /// Load all jobs (by namespace).
    pub fn load_jobs(&self) -> Result<HashMap<String, HashMap<String, JobDesired>>> {
        let path = self.jobs_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, JobDesired>> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse jobs: {}", e)))?;
        Ok(map)
    }

    /// Save all jobs.
    pub fn save_jobs(&self, jobs: &HashMap<String, HashMap<String, JobDesired>>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.jobs_path();
        let content = serde_json::to_string_pretty(jobs)
            .map_err(|e| A3sError::Project(format!("failed to serialize jobs: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a job.
    pub fn set_job(&self, job: JobDesired) -> Result<()> {
        let mut jobs = self.load_jobs()?;
        jobs.entry(job.namespace.clone())
            .or_insert_with(HashMap::new);
        jobs.get_mut(&job.namespace)
            .unwrap()
            .insert(job.name.clone(), job);
        self.save_jobs(&jobs)
    }

    /// Get a job.
    pub fn get_job(&self, namespace: &str, name: &str) -> Result<Option<JobDesired>> {
        let jobs = self.load_jobs()?;
        Ok(jobs.get(namespace).and_then(|j| j.get(name).cloned()))
    }

    /// Delete a job.
    pub fn delete_job(&self, namespace: &str, name: &str) -> Result<()> {
        let mut jobs = self.load_jobs()?;
        if let Some(ns_jobs) = jobs.get_mut(namespace) {
            ns_jobs.remove(name);
        }
        self.save_jobs(&jobs)
    }

    // ==================== CronJob ====================

    /// Path to cronjobs state file.
    fn cronjobs_path(&self) -> PathBuf {
        self.path.join("cronjobs.json")
    }

    /// Load all cronjobs (by namespace).
    pub fn load_cronjobs(&self) -> Result<HashMap<String, HashMap<String, CronJobDesired>>> {
        let path = self.cronjobs_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, CronJobDesired>> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse cronjobs: {}", e)))?;
        Ok(map)
    }

    /// Save all cronjobs.
    pub fn save_cronjobs(
        &self,
        cronjobs: &HashMap<String, HashMap<String, CronJobDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.cronjobs_path();
        let content = serde_json::to_string_pretty(cronjobs)
            .map_err(|e| A3sError::Project(format!("failed to serialize cronjobs: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a cronjob.
    pub fn set_cronjob(&self, cronjob: CronJobDesired) -> Result<()> {
        let mut cronjobs = self.load_cronjobs()?;
        cronjobs
            .entry(cronjob.namespace.clone())
            .or_insert_with(HashMap::new);
        cronjobs
            .get_mut(&cronjob.namespace)
            .unwrap()
            .insert(cronjob.name.clone(), cronjob);
        self.save_cronjobs(&cronjobs)
    }

    /// Get a cronjob.
    pub fn get_cronjob(&self, namespace: &str, name: &str) -> Result<Option<CronJobDesired>> {
        let cronjobs = self.load_cronjobs()?;
        Ok(cronjobs.get(namespace).and_then(|c| c.get(name).cloned()))
    }

    /// Delete a cronjob.
    pub fn delete_cronjob(&self, namespace: &str, name: &str) -> Result<()> {
        let mut cronjobs = self.load_cronjobs()?;
        if let Some(ns_cj) = cronjobs.get_mut(namespace) {
            ns_cj.remove(name);
        }
        self.save_cronjobs(&cronjobs)
    }

    // ==================== NetworkPolicy ====================

    /// Path to networkpolicies state file.
    fn networkpolicies_path(&self) -> PathBuf {
        self.path.join("networkpolicies.json")
    }

    /// Load all networkpolicies.
    pub fn load_network_policies(&self) -> Result<Vec<NetworkPolicyDesired>> {
        let path = self.networkpolicies_path();
        if !path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let policies: Vec<NetworkPolicyDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse networkpolicies: {}", e)))?;
        Ok(policies)
    }

    /// Save all networkpolicies.
    pub fn save_network_policies(&self, policies: &[NetworkPolicyDesired]) -> Result<()> {
        self.ensure_dir()?;
        let path = self.networkpolicies_path();
        let content = serde_json::to_string_pretty(policies).map_err(|e| {
            A3sError::Project(format!("failed to serialize networkpolicies: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Add a networkpolicy.
    pub fn add_network_policy(&self, policy: NetworkPolicyDesired) -> Result<()> {
        let mut policies = self.load_network_policies()?;
        policies.push(policy);
        self.save_network_policies(&policies)
    }

    /// Delete a networkpolicy.
    pub fn delete_network_policy(&self, namespace: &str, name: &str) -> Result<()> {
        let mut policies = self.load_network_policies()?;
        policies.retain(|p| !(p.namespace == namespace && p.name == name));
        self.save_network_policies(&policies)
    }

    // ==================== ReplicaSet ====================

    /// Path to replicasets state file.
    fn replicasets_path(&self) -> PathBuf {
        self.path.join("replicasets.json")
    }

    /// Load all replicasets (by namespace).
    pub fn load_replicasets(&self) -> Result<HashMap<String, HashMap<String, ReplicaSetDesired>>> {
        let path = self.replicasets_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, ReplicaSetDesired>> =
            serde_json::from_str(&content)
                .map_err(|e| A3sError::Project(format!("failed to parse replicasets: {}", e)))?;
        Ok(map)
    }

    /// Save all replicasets.
    pub fn save_replicasets(
        &self,
        replicasets: &HashMap<String, HashMap<String, ReplicaSetDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.replicasets_path();
        let content = serde_json::to_string_pretty(replicasets)
            .map_err(|e| A3sError::Project(format!("failed to serialize replicasets: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a replicaset.
    pub fn set_replicaset(&self, replicaset: ReplicaSetDesired) -> Result<()> {
        let mut replicasets = self.load_replicasets()?;
        replicasets
            .entry(replicaset.namespace.clone())
            .or_insert_with(HashMap::new);
        replicasets
            .get_mut(&replicaset.namespace)
            .unwrap()
            .insert(replicaset.name.clone(), replicaset);
        self.save_replicasets(&replicasets)
    }

    /// Get a replicaset.
    pub fn get_replicaset(&self, namespace: &str, name: &str) -> Result<Option<ReplicaSetDesired>> {
        let replicasets = self.load_replicasets()?;
        Ok(replicasets
            .get(namespace)
            .and_then(|r| r.get(name).cloned()))
    }

    /// Delete a replicaset.
    pub fn delete_replicaset(&self, namespace: &str, name: &str) -> Result<()> {
        let mut replicasets = self.load_replicasets()?;
        if let Some(ns_rs) = replicasets.get_mut(namespace) {
            ns_rs.remove(name);
        }
        self.save_replicasets(&replicasets)
    }

    // ==================== PersistentVolumeClaim ====================

    /// Path to persistentvolumeclaims state file.
    fn pvcs_path(&self) -> PathBuf {
        self.path.join("persistentvolumeclaims.json")
    }

    /// Load all persistentvolumeclaims (by namespace).
    pub fn load_persistent_volume_claims(
        &self,
    ) -> Result<HashMap<String, HashMap<String, PersistentVolumeClaimDesired>>> {
        let path = self.pvcs_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, PersistentVolumeClaimDesired>> =
            serde_json::from_str(&content).map_err(|e| {
                A3sError::Project(format!("failed to parse persistentvolumeclaims: {}", e))
            })?;
        Ok(map)
    }

    /// Save all persistentvolumeclaims.
    pub fn save_persistent_volume_claims(
        &self,
        pvcs: &HashMap<String, HashMap<String, PersistentVolumeClaimDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.pvcs_path();
        let content = serde_json::to_string_pretty(pvcs).map_err(|e| {
            A3sError::Project(format!("failed to serialize persistentvolumeclaims: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Set a persistentvolumeclaim.
    pub fn set_persistent_volume_claim(&self, pvc: PersistentVolumeClaimDesired) -> Result<()> {
        let mut pvcs = self.load_persistent_volume_claims()?;
        pvcs.entry(pvc.namespace.clone())
            .or_insert_with(HashMap::new);
        pvcs.get_mut(&pvc.namespace)
            .unwrap()
            .insert(pvc.name.clone(), pvc);
        self.save_persistent_volume_claims(&pvcs)
    }

    /// Get a persistentvolumeclaim.
    pub fn get_persistent_volume_claim(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<PersistentVolumeClaimDesired>> {
        let pvcs = self.load_persistent_volume_claims()?;
        Ok(pvcs.get(namespace).and_then(|p| p.get(name).cloned()))
    }

    /// Delete a persistentvolumeclaim.
    pub fn delete_persistent_volume_claim(&self, namespace: &str, name: &str) -> Result<()> {
        let mut pvcs = self.load_persistent_volume_claims()?;
        if let Some(ns_pvc) = pvcs.get_mut(namespace) {
            ns_pvc.remove(name);
        }
        self.save_persistent_volume_claims(&pvcs)
    }

    // ==================== PersistentVolume ====================

    /// Path to persistentvolumes state file.
    fn pvs_path(&self) -> PathBuf {
        self.path.join("persistentvolumes.json")
    }

    /// Load all persistentvolumes.
    pub fn load_persistent_volumes(&self) -> Result<HashMap<String, PersistentVolumeDesired>> {
        let path = self.pvs_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, PersistentVolumeDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse persistentvolumes: {}", e)))?;
        Ok(map)
    }

    /// Save all persistentvolumes.
    pub fn save_persistent_volumes(
        &self,
        pvs: &HashMap<String, PersistentVolumeDesired>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.pvs_path();
        let content = serde_json::to_string_pretty(pvs).map_err(|e| {
            A3sError::Project(format!("failed to serialize persistentvolumes: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Set a persistentvolume.
    pub fn set_persistent_volume(&self, pv: PersistentVolumeDesired) -> Result<()> {
        let mut pvs = self.load_persistent_volumes()?;
        pvs.insert(pv.name.clone(), pv);
        self.save_persistent_volumes(&pvs)
    }

    /// Get a persistentvolume.
    pub fn get_persistent_volume(&self, name: &str) -> Result<Option<PersistentVolumeDesired>> {
        let pvs = self.load_persistent_volumes()?;
        Ok(pvs.get(name).cloned())
    }

    /// Delete a persistentvolume.
    pub fn delete_persistent_volume(&self, name: &str) -> Result<()> {
        let mut pvs = self.load_persistent_volumes()?;
        pvs.remove(name);
        self.save_persistent_volumes(&pvs)
    }

    // ==================== StorageClass ====================

    /// Path to storageclasses state file.
    fn storageclasses_path(&self) -> PathBuf {
        self.path.join("storageclasses.json")
    }

    /// Load all storageclasses.
    pub fn load_storage_classes(&self) -> Result<HashMap<String, StorageClassDesired>> {
        let path = self.storageclasses_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, StorageClassDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse storageclasses: {}", e)))?;
        Ok(map)
    }

    /// Save all storageclasses.
    pub fn save_storage_classes(&self, scs: &HashMap<String, StorageClassDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.storageclasses_path();
        let content = serde_json::to_string_pretty(scs)
            .map_err(|e| A3sError::Project(format!("failed to serialize storageclasses: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a storageclass.
    pub fn set_storage_class(&self, sc: StorageClassDesired) -> Result<()> {
        let mut scs = self.load_storage_classes()?;
        scs.insert(sc.name.clone(), sc);
        self.save_storage_classes(&scs)
    }

    /// Get a storageclass.
    pub fn get_storage_class(&self, name: &str) -> Result<Option<StorageClassDesired>> {
        let scs = self.load_storage_classes()?;
        Ok(scs.get(name).cloned())
    }

    /// Delete a storageclass.
    pub fn delete_storage_class(&self, name: &str) -> Result<()> {
        let mut scs = self.load_storage_classes()?;
        scs.remove(name);
        self.save_storage_classes(&scs)
    }

    // ==================== Role ====================

    /// Path to roles state file.
    fn roles_path(&self) -> PathBuf {
        self.path.join("roles.json")
    }

    /// Load all roles (by namespace).
    pub fn load_roles(&self) -> Result<HashMap<String, HashMap<String, RoleDesired>>> {
        let path = self.roles_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, RoleDesired>> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse roles: {}", e)))?;
        Ok(map)
    }

    /// Save all roles.
    pub fn save_roles(&self, roles: &HashMap<String, HashMap<String, RoleDesired>>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.roles_path();
        let content = serde_json::to_string_pretty(roles)
            .map_err(|e| A3sError::Project(format!("failed to serialize roles: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a role.
    pub fn set_role(&self, role: RoleDesired) -> Result<()> {
        let ns = role.namespace.clone().unwrap_or_default();
        let mut roles = self.load_roles()?;
        roles.entry(ns.clone()).or_insert_with(HashMap::new);
        roles.get_mut(&ns).unwrap().insert(role.name.clone(), role);
        self.save_roles(&roles)
    }

    /// Get a role.
    pub fn get_role(&self, namespace: &str, name: &str) -> Result<Option<RoleDesired>> {
        let roles = self.load_roles()?;
        Ok(roles
            .get(namespace)
            .and_then(|ns_roles| ns_roles.get(name).cloned()))
    }

    /// Delete a role.
    pub fn delete_role(&self, namespace: &str, name: &str) -> Result<()> {
        let mut roles = self.load_roles()?;
        if let Some(ns_roles) = roles.get_mut(namespace) {
            ns_roles.remove(name);
        }
        self.save_roles(&roles)
    }

    // ==================== ClusterRole ====================

    /// Path to clusterroles state file.
    fn clusterroles_path(&self) -> PathBuf {
        self.path.join("clusterroles.json")
    }

    /// Load all clusterroles.
    pub fn load_cluster_roles(&self) -> Result<HashMap<String, ClusterRoleDesired>> {
        let path = self.clusterroles_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, ClusterRoleDesired> = serde_json::from_str(&content)
            .map_err(|e| A3sError::Project(format!("failed to parse clusterroles: {}", e)))?;
        Ok(map)
    }

    /// Save all clusterroles.
    pub fn save_cluster_roles(&self, roles: &HashMap<String, ClusterRoleDesired>) -> Result<()> {
        self.ensure_dir()?;
        let path = self.clusterroles_path();
        let content = serde_json::to_string_pretty(roles)
            .map_err(|e| A3sError::Project(format!("failed to serialize clusterroles: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a clusterrole.
    pub fn set_cluster_role(&self, role: ClusterRoleDesired) -> Result<()> {
        let mut roles = self.load_cluster_roles()?;
        roles.insert(role.name.clone(), role);
        self.save_cluster_roles(&roles)
    }

    /// Get a clusterrole.
    pub fn get_cluster_role(&self, name: &str) -> Result<Option<ClusterRoleDesired>> {
        let roles = self.load_cluster_roles()?;
        Ok(roles.get(name).cloned())
    }

    /// Delete a clusterrole.
    pub fn delete_cluster_role(&self, name: &str) -> Result<()> {
        let mut roles = self.load_cluster_roles()?;
        roles.remove(name);
        self.save_cluster_roles(&roles)
    }

    // ==================== RoleBinding ====================

    /// Path to rolebindings state file.
    fn rolebindings_path(&self) -> PathBuf {
        self.path.join("rolebindings.json")
    }

    /// Load all rolebindings (by namespace).
    pub fn load_role_bindings(
        &self,
    ) -> Result<HashMap<String, HashMap<String, RoleBindingDesired>>> {
        let path = self.rolebindings_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, RoleBindingDesired>> =
            serde_json::from_str(&content)
                .map_err(|e| A3sError::Project(format!("failed to parse rolebindings: {}", e)))?;
        Ok(map)
    }

    /// Save all rolebindings.
    pub fn save_role_bindings(
        &self,
        bindings: &HashMap<String, HashMap<String, RoleBindingDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.rolebindings_path();
        let content = serde_json::to_string_pretty(bindings)
            .map_err(|e| A3sError::Project(format!("failed to serialize rolebindings: {}", e)))?;
        atomic_write(&path, content)
    }

    /// Set a rolebinding.
    pub fn set_role_binding(&self, binding: RoleBindingDesired) -> Result<()> {
        let ns = binding.namespace.clone().unwrap_or_default();
        let mut bindings = self.load_role_bindings()?;
        bindings.entry(ns.clone()).or_insert_with(HashMap::new);
        bindings
            .get_mut(&ns)
            .unwrap()
            .insert(binding.name.clone(), binding);
        self.save_role_bindings(&bindings)
    }

    /// Get a rolebinding.
    pub fn get_role_binding(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<RoleBindingDesired>> {
        let bindings = self.load_role_bindings()?;
        Ok(bindings
            .get(namespace)
            .and_then(|ns_bindings| ns_bindings.get(name).cloned()))
    }

    /// Delete a rolebinding.
    pub fn delete_role_binding(&self, namespace: &str, name: &str) -> Result<()> {
        let mut bindings = self.load_role_bindings()?;
        if let Some(ns_bindings) = bindings.get_mut(namespace) {
            ns_bindings.remove(name);
        }
        self.save_role_bindings(&bindings)
    }

    // ==================== ClusterRoleBinding ====================

    /// Path to clusterrolebindings state file.
    fn clusterrolebindings_path(&self) -> PathBuf {
        self.path.join("clusterrolebindings.json")
    }

    /// Load all clusterrolebindings.
    pub fn load_cluster_role_bindings(&self) -> Result<HashMap<String, ClusterRoleBindingDesired>> {
        let path = self.clusterrolebindings_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, ClusterRoleBindingDesired> = serde_json::from_str(&content)
            .map_err(|e| {
                A3sError::Project(format!("failed to parse clusterrolebindings: {}", e))
            })?;
        Ok(map)
    }

    /// Save all clusterrolebindings.
    pub fn save_cluster_role_bindings(
        &self,
        bindings: &HashMap<String, ClusterRoleBindingDesired>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.clusterrolebindings_path();
        let content = serde_json::to_string_pretty(bindings).map_err(|e| {
            A3sError::Project(format!("failed to serialize clusterrolebindings: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Set a clusterrolebinding.
    pub fn set_cluster_role_binding(&self, binding: ClusterRoleBindingDesired) -> Result<()> {
        let mut bindings = self.load_cluster_role_bindings()?;
        bindings.insert(binding.name.clone(), binding);
        self.save_cluster_role_bindings(&bindings)
    }

    /// Get a clusterrolebinding.
    pub fn get_cluster_role_binding(
        &self,
        name: &str,
    ) -> Result<Option<ClusterRoleBindingDesired>> {
        let bindings = self.load_cluster_role_bindings()?;
        Ok(bindings.get(name).cloned())
    }

    /// Delete a clusterrolebinding.
    pub fn delete_cluster_role_binding(&self, name: &str) -> Result<()> {
        let mut bindings = self.load_cluster_role_bindings()?;
        bindings.remove(name);
        self.save_cluster_role_bindings(&bindings)
    }

    // ==================== ServiceAccount ====================

    /// Path to serviceaccounts state file.
    fn serviceaccounts_path(&self) -> PathBuf {
        self.path.join("serviceaccounts.json")
    }

    /// Load all serviceaccounts (by namespace).
    pub fn load_service_accounts(
        &self,
    ) -> Result<HashMap<String, HashMap<String, ServiceAccountDesired>>> {
        let path = self.serviceaccounts_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, ServiceAccountDesired>> =
            serde_json::from_str(&content).map_err(|e| {
                A3sError::Project(format!("failed to parse serviceaccounts: {}", e))
            })?;
        Ok(map)
    }

    /// Save all serviceaccounts.
    pub fn save_service_accounts(
        &self,
        serviceaccounts: &HashMap<String, HashMap<String, ServiceAccountDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.serviceaccounts_path();
        let content = serde_json::to_string_pretty(serviceaccounts).map_err(|e| {
            A3sError::Project(format!("failed to serialize serviceaccounts: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Set a serviceaccount.
    pub fn set_service_account(&self, sa: ServiceAccountDesired) -> Result<()> {
        let mut serviceaccounts = self.load_service_accounts()?;
        serviceaccounts
            .entry(sa.namespace.clone())
            .or_insert_with(HashMap::new);
        serviceaccounts
            .get_mut(&sa.namespace)
            .unwrap()
            .insert(sa.name.clone(), sa);
        self.save_service_accounts(&serviceaccounts)
    }

    /// Get a serviceaccount.
    pub fn get_service_account(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<ServiceAccountDesired>> {
        let serviceaccounts = self.load_service_accounts()?;
        Ok(serviceaccounts
            .get(namespace)
            .and_then(|s| s.get(name).cloned()))
    }

    /// Delete a serviceaccount.
    pub fn delete_service_account(&self, namespace: &str, name: &str) -> Result<()> {
        let mut serviceaccounts = self.load_service_accounts()?;
        if let Some(ns_sa) = serviceaccounts.get_mut(namespace) {
            ns_sa.remove(name);
        }
        self.save_service_accounts(&serviceaccounts)
    }

    // ==================== PodDisruptionBudget ====================

    /// Path to poddisruptionbudgets state file.
    fn poddisruptionbudgets_path(&self) -> PathBuf {
        self.path.join("poddisruptionbudgets.json")
    }

    /// Load all poddisruptionbudgets (by namespace).
    pub fn load_pod_disruption_budgets(
        &self,
    ) -> Result<HashMap<String, HashMap<String, PodDisruptionBudgetDesired>>> {
        let path = self.poddisruptionbudgets_path();
        if !path.exists() {
            return Ok(HashMap::new());
        }
        let content = std::fs::read_to_string(&path)?;
        let map: HashMap<String, HashMap<String, PodDisruptionBudgetDesired>> =
            serde_json::from_str(&content).map_err(|e| {
                A3sError::Project(format!("failed to parse poddisruptionbudgets: {}", e))
            })?;
        Ok(map)
    }

    /// Save all poddisruptionbudgets.
    pub fn save_pod_disruption_budgets(
        &self,
        pdbs: &HashMap<String, HashMap<String, PodDisruptionBudgetDesired>>,
    ) -> Result<()> {
        self.ensure_dir()?;
        let path = self.poddisruptionbudgets_path();
        let content = serde_json::to_string_pretty(pdbs).map_err(|e| {
            A3sError::Project(format!("failed to serialize poddisruptionbudgets: {}", e))
        })?;
        atomic_write(&path, content)
    }

    /// Set a poddisruptionbudget.
    pub fn set_pod_disruption_budget(&self, pdb: PodDisruptionBudgetDesired) -> Result<()> {
        let mut pdbs = self.load_pod_disruption_budgets()?;
        pdbs.entry(pdb.namespace.clone())
            .or_insert_with(HashMap::new);
        pdbs.get_mut(&pdb.namespace)
            .unwrap()
            .insert(pdb.name.clone(), pdb);
        self.save_pod_disruption_budgets(&pdbs)
    }

    /// Get a poddisruptionbudget.
    pub fn get_pod_disruption_budget(
        &self,
        namespace: &str,
        name: &str,
    ) -> Result<Option<PodDisruptionBudgetDesired>> {
        let pdbs = self.load_pod_disruption_budgets()?;
        Ok(pdbs
            .get(namespace)
            .and_then(|ns_pdbs| ns_pdbs.get(name).cloned()))
    }

    /// Delete a poddisruptionbudget.
    pub fn delete_pod_disruption_budget(&self, namespace: &str, name: &str) -> Result<()> {
        let mut pdbs = self.load_pod_disruption_budgets()?;
        if let Some(ns_pdbs) = pdbs.get_mut(namespace) {
            ns_pdbs.remove(name);
        }
        self.save_pod_disruption_budgets(&pdbs)
    }

    // ==================== Utility ====================

    /// Get a diff between desired and actual replicas.
    pub fn get_replica_diff(&self, deployment: &str) -> Result<ReplicaDiff> {
        let desired = self.get_deployment(deployment)?;
        let pods = self.get_pods_by_deployment(deployment)?;

        let desired_replicas = desired.map(|d| d.replicas).unwrap_or(0);
        let actual_running = pods
            .iter()
            .filter(|p| p.status == PodStatus::Running)
            .count() as i32;

        let actual_unhealthy = pods
            .iter()
            .filter(|p| p.health == HealthStatus::Unhealthy)
            .count() as i32;

        Ok(ReplicaDiff {
            deployment: deployment.to_string(),
            desired_replicas,
            actual_replicas: pods.len() as i32,
            running_replicas: actual_running,
            unhealthy_replicas: actual_unhealthy,
            need_create: desired_replicas - actual_running,
            need_delete: (pods.len() as i32 - desired_replicas).max(0),
        })
    }

    /// Clear all state (for testing/reset).
    pub fn clear_all(&self) -> Result<()> {
        if self.path.exists() {
            std::fs::remove_dir_all(&self.path)?;
        }
        Ok(())
    }
}

/// Replica diff result.
#[derive(Debug, Clone)]
pub struct ReplicaDiff {
    pub deployment: String,
    pub desired_replicas: i32,
    pub actual_replicas: i32,
    pub running_replicas: i32,
    pub unhealthy_replicas: i32,
    pub need_create: i32,
    pub need_delete: i32,
}

/// Atomically write content to a file.
fn atomic_write(path: &PathBuf, content: String) -> Result<()> {
    let tmp_path = path.with_extension("tmp");

    // Ensure parent directory exists
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| A3sError::Project(format!("failed to create directory: {}", e)))?;
    }

    // Write to temp file
    std::fs::write(&tmp_path, &content)
        .map_err(|e| A3sError::Project(format!("failed to write temp file: {}", e)))?;

    // Rename to target (atomic on POSIX)
    std::fs::rename(&tmp_path, path)
        .map_err(|e| A3sError::Project(format!("failed to rename temp file: {}", e)))?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::state::{RollingUpdateConfig, UpdateStrategy};
    use std::env::temp_dir;

    fn test_store() -> StateStore {
        let dir = temp_dir().join(format!("a3s-state-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        StateStore::new(&dir)
    }

    #[test]
    fn test_deployment_crud() {
        let store = test_store();
        let deployment = DeploymentDesired {
            name: "nginx".to_string(),
            namespace: "default".to_string(),
            image: "nginx:alpine".to_string(),
            replicas: 3,
            env: HashMap::new(),
            ports: vec![],
            version: "v1".to_string(),
            strategy: UpdateStrategy::RollingUpdate(RollingUpdateConfig::default()),
            resources: Default::default(),
            health_check: Default::default(),
            node_selector: Default::default(),
            tolerations: vec![],
            labels: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Create
        store.set_deployment(deployment.clone()).unwrap();

        // Read
        let loaded = store.get_deployment("nginx").unwrap().unwrap();
        assert_eq!(loaded.name, "nginx");
        assert_eq!(loaded.replicas, 3);

        // Update
        let mut updated = deployment.clone();
        updated.replicas = 5;
        store.set_deployment(updated).unwrap();

        let loaded = store.get_deployment("nginx").unwrap().unwrap();
        assert_eq!(loaded.replicas, 5);

        // Delete
        store.delete_deployment("nginx").unwrap();
        assert!(store.get_deployment("nginx").unwrap().is_none());
    }

    #[test]
    fn test_pod_crud() {
        let store = test_store();
        let pod = PodActual {
            id: "pod-123".to_string(),
            deployment: "nginx".to_string(),
            namespace: "default".to_string(),
            status: PodStatus::Running,
            health: HealthStatus::Healthy,
            ip: Some("10.0.0.1".to_string()),
            version: "v1".to_string(),
            socket_path: Some("/tmp/exec.sock".to_string()),
            node_name: None,
            created_at: Utc::now(),
            last_health_check: Some(Utc::now()),
            consecutive_failures: 0,
            ready: true,
        };

        store.set_pod(pod.clone()).unwrap();

        let loaded = store.get_pod("pod-123").unwrap().unwrap();
        assert_eq!(loaded.deployment, "nginx");
        assert_eq!(loaded.status, PodStatus::Running);

        // Update health
        store.update_pod_health("pod-123", false).unwrap();
        let loaded = store.get_pod("pod-123").unwrap().unwrap();
        assert_eq!(loaded.consecutive_failures, 1);
        assert_eq!(loaded.health, HealthStatus::Healthy); // Not yet unhealthy (need 3 failures)

        store.update_pod_health("pod-123", false).unwrap();
        store.update_pod_health("pod-123", false).unwrap();
        let loaded = store.get_pod("pod-123").unwrap().unwrap();
        assert_eq!(loaded.health, HealthStatus::Unhealthy);
    }

    #[test]
    fn test_replica_diff() {
        let store = test_store();

        // Set deployment with 3 replicas
        let deployment = DeploymentDesired {
            name: "nginx".to_string(),
            namespace: "default".to_string(),
            image: "nginx:alpine".to_string(),
            replicas: 3,
            env: HashMap::new(),
            ports: vec![],
            version: "v1".to_string(),
            strategy: UpdateStrategy::RollingUpdate(RollingUpdateConfig::default()),
            resources: Default::default(),
            health_check: Default::default(),
            node_selector: Default::default(),
            tolerations: vec![],
            labels: HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        store.set_deployment(deployment).unwrap();

        // Add 2 running pods
        for i in 1..=2 {
            let pod = PodActual {
                id: format!("pod-{}", i),
                deployment: "nginx".to_string(),
                namespace: "default".to_string(),
                status: PodStatus::Running,
                health: HealthStatus::Healthy,
                ip: Some(format!("10.0.0.{}", i)),
                version: "v1".to_string(),
                socket_path: None,
                node_name: None,
                created_at: Utc::now(),
                last_health_check: Some(Utc::now()),
                consecutive_failures: 0,
                ready: true,
            };
            store.set_pod(pod).unwrap();
        }

        let diff = store.get_replica_diff("nginx").unwrap();
        assert_eq!(diff.desired_replicas, 3);
        assert_eq!(diff.actual_replicas, 2);
        assert_eq!(diff.running_replicas, 2);
        assert_eq!(diff.need_create, 1);
        assert_eq!(diff.need_delete, 0);
    }
}
