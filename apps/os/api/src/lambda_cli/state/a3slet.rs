//! A3slet - Node Agent for Pod Lifecycle Management.
//!
//! A3slet is the node agent that runs on each cluster node, analogous to Kubernetes' kubelet.
//! It is responsible for:
//! - Registering the node with the A3s API server
//! - Watching for pod assignments to this node
//! - Creating, starting, stopping, and deleting pods
//! - Mounting volumes to containers
//! - Handling container health probes
//! - Reporting pod and node status back to the API server
//!
//! Unlike Kubernetes where kubelet is a separate binary, A3slet is embedded
//! in the lambda daemon for simplicity.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::process::Stdio;
use tokio::process::{Child, Command};
use tokio::sync::{mpsc, RwLock};

/// A3slet configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletConfig {
    /// Node name this a3slet is running on.
    pub node_name: String,
    /// API server address.
    pub api_server_addr: String,
    /// A3slet port for health checks.
    pub port: u16,
    /// Root directory for pod state.
    pub root_dir: PathBuf,
    /// Volume mount directory.
    pub volume_dir: PathBuf,
    /// Pod manifest directory (static pods).
    pub manifest_dir: PathBuf,
    /// Container runtime endpoint.
    pub container_runtime: String,
    /// Register this node with API server on startup.
    pub register_node: bool,
}

impl Default for A3sletConfig {
    fn default() -> Self {
        Self {
            node_name: hostname::get()
                .map(|s| s.to_string_lossy().into_owned())
                .unwrap_or_else(|_| "localhost".to_string()),
            api_server_addr: "http://127.0.0.1:8080".to_string(),
            port: 10250,
            root_dir: PathBuf::from("/var/lib/a3s/a3slet"),
            volume_dir: PathBuf::from("/var/lib/a3s/volumes"),
            manifest_dir: PathBuf::from("/etc/a3s/manifests"),
            container_runtime: "a3s-box".to_string(),
            register_node: true,
        }
    }
}

/// Container state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ContainerState {
    Created,
    Running,
    Paused,
    Restarting,
    Exited,
    Dead,
    Unknown,
}

impl Default for ContainerState {
    fn default() -> Self {
        ContainerState::Unknown
    }
}

/// Container status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerStatus {
    /// Container ID.
    pub id: String,
    /// Container name.
    pub name: String,
    /// Container image.
    pub image: String,
    /// Container image ID.
    pub image_id: String,
    /// Current state.
    pub state: ContainerState,
    /// Last termination exit code.
    pub last_exit_code: Option<i32>,
    /// Last termination timestamp.
    pub last_termination_time: Option<DateTime<Utc>>,
    /// Restart count.
    pub restart_count: i32,
    /// Created timestamp.
    pub created: DateTime<Utc>,
}

/// Pod container spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletContainerSpec {
    /// Container name.
    pub name: String,
    /// Container image.
    pub image: String,
    /// Image pull policy.
    pub image_pull_policy: ImagePullPolicy,
    /// Command.
    pub command: Vec<String>,
    /// Arguments.
    pub args: Vec<String>,
    /// Working directory.
    pub working_dir: Option<String>,
    /// Environment variables.
    pub env: HashMap<String, String>,
    /// Volume mounts.
    pub volume_mounts: Vec<A3sletVolumeMount>,
    /// Mount read-only.
    pub read_only: bool,
    /// Liveness probe.
    pub liveness_probe: Option<A3sletProbe>,
    /// Readiness probe.
    pub readiness_probe: Option<A3sletProbe>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ImagePullPolicy {
    Always,
    IfNotPresent,
    Never,
}

impl Default for ImagePullPolicy {
    fn default() -> Self {
        ImagePullPolicy::Always
    }
}

/// Volume mount.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletVolumeMount {
    /// Volume name.
    pub name: String,
    /// Mount path.
    pub mount_path: String,
    /// Read only.
    pub read_only: bool,
    /// Sub-path.
    pub sub_path: Option<String>,
}

/// Health probe.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletProbe {
    /// Probe type.
    pub probe_type: ProbeType,
    /// Initial delay seconds.
    pub initial_delay_secs: i32,
    /// Timeout seconds.
    pub timeout_secs: i32,
    /// Period seconds.
    pub period_secs: i32,
    /// Success threshold.
    pub success_threshold: i32,
    /// Failure threshold.
    pub failure_threshold: i32,
    /// HTTP probe settings.
    pub http_get: Option<HttpProbe>,
    /// Exec probe settings.
    pub exec: Option<ExecProbe>,
    /// TCP probe settings.
    pub tcp_socket: Option<TcpSocketProbe>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ProbeType {
    /// HTTP probe.
    Http,
    /// Exec probe.
    Exec,
    /// TCP socket probe.
    Tcp,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HttpProbe {
    /// Path.
    pub path: String,
    /// Port.
    pub port: i32,
    /// Host.
    pub host: Option<String>,
    /// HTTP scheme.
    pub scheme: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecProbe {
    /// Command.
    pub command: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TcpSocketProbe {
    /// Port.
    pub port: i32,
    /// Host.
    pub host: Option<String>,
}

/// Pod spec for a3slet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletPodSpec {
    /// Pod name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// UID.
    pub uid: String,
    /// Containers.
    pub containers: Vec<A3sletContainerSpec>,
    /// Volumes.
    pub volumes: Vec<A3sletVolume>,
    /// Restart policy.
    pub restart_policy: String,
    /// Node name.
    pub node_name: String,
    /// Host network.
    pub host_network: bool,
    /// Host PID.
    pub host_pid: bool,
    /// Host IPC.
    pub host_ipc: bool,
}

/// Volume source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletVolume {
    /// Volume name.
    pub name: String,
    /// Volume source type.
    pub source: A3sletVolumeSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum A3sletVolumeSource {
    /// Empty dir volume.
    EmptyDir,
    /// Host path volume.
    HostPath { path: String, kind: HostPathKind },
    /// ConfigMap.
    ConfigMap {
        name: String,
        items: HashMap<String, String>,
    },
    /// Secret.
    Secret { name: String },
    /// PersistentVolumeClaim.
    PersistentVolumeClaim { claim_name: String, read_only: bool },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum HostPathKind {
    Directory,
    DirectoryOrCreate,
    File,
    FileOrCreate,
    Socket,
    CharDevice,
    BlockDevice,
}

impl Default for HostPathKind {
    fn default() -> Self {
        HostPathKind::Directory
    }
}

/// Pod status reported by a3slet.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct A3sletPodStatus {
    /// Pod name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// UID.
    pub uid: String,
    /// Container statuses.
    pub container_statuses: Vec<ContainerStatus>,
    /// Phase.
    pub phase: PodPhase,
    /// Message.
    pub message: Option<String>,
    /// Reason.
    pub reason: Option<String>,
    /// Host IP.
    pub host_ip: String,
    /// Pod IP.
    pub pod_ip: String,
    /// Start time.
    pub start_time: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PodPhase {
    Pending,
    Running,
    Succeeded,
    Failed,
    Unknown,
}

impl Default for PodPhase {
    fn default() -> Self {
        PodPhase::Pending
    }
}

/// Running container.
#[derive(Debug)]
pub struct RunningContainer {
    /// Container name.
    pub name: String,
    /// Process handle.
    pub child: Child,
    /// Exit code channel.
    pub exit_code: Option<i32>,
}

/// Managed pod state.
struct ManagedPod {
    /// Pod spec.
    spec: A3sletPodSpec,
    /// Running containers.
    containers: HashMap<String, RunningContainer>,
    /// Pod status.
    status: A3sletPodStatus,
}

/// A3slet - Node agent for pod management.
pub struct A3slet {
    /// Configuration.
    config: A3sletConfig,
    /// Managed pods.
    pods: RwLock<HashMap<String, ManagedPod>>,
    /// Event sender.
    event_sender: mpsc::Sender<A3sletEvent>,
    /// Running state.
    running: RwLock<bool>,
}

impl A3slet {
    /// Create a new a3slet.
    pub async fn new(config: A3sletConfig) -> Result<Self> {
        let (event_sender, _event_receiver) = mpsc::channel(100);
        Ok(Self {
            config,
            pods: RwLock::new(HashMap::new()),
            event_sender,
            running: RwLock::new(false),
        })
    }

    /// Start the a3slet.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;

        // Ensure directories exist
        tokio::fs::create_dir_all(&self.config.root_dir).await?;
        tokio::fs::create_dir_all(&self.config.volume_dir).await?;

        tracing::info!(
            node_name = %self.config.node_name,
            api_server = %self.config.api_server_addr,
            "A3slet started"
        );

        Ok(())
    }

    /// Stop the a3slet.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;

        // Stop all pods
        let mut pods = self.pods.write().await;
        for (pod_key, pod) in pods.iter_mut() {
            tracing::info!(pod = %pod_key, "Stopping pod");
            for (container_name, container) in pod.containers.iter_mut() {
                let _ = container.child.kill().await;
                tracing::debug!(container = %container_name, "Container stopped");
            }
        }

        tracing::info!(node_name = %self.config.node_name, "A3slet stopped");
        Ok(())
    }

    /// Sync a pod - create or update.
    pub async fn sync_pod(&self, spec: A3sletPodSpec) -> Result<()> {
        let pod_key = format!("{}/{}", spec.namespace, spec.name);

        // Check if pod already exists
        let mut pods = self.pods.write().await;
        if let Some(existing) = pods.get_mut(&pod_key) {
            // Update existing pod
            tracing::info!(pod = %pod_key, "Updating pod");
            self.update_pod(existing, spec).await?;
        } else {
            // Create new pod
            tracing::info!(pod = %pod_key, "Creating pod");
            self.create_pod(&mut pods, spec).await?;
        }

        Ok(())
    }

    /// Delete a pod.
    pub async fn delete_pod(&self, namespace: &str, name: &str) -> Result<()> {
        let pod_key = format!("{}/{}", namespace, name);
        let mut pods = self.pods.write().await;

        if let Some(mut pod) = pods.remove(&pod_key) {
            tracing::info!(pod = %pod_key, "Deleting pod");
            for (_, container) in &mut pod.containers {
                let _ = container.child.kill().await;
            }
        }

        Ok(())
    }

    /// Get pod status.
    pub async fn get_pod_status(&self, namespace: &str, name: &str) -> Option<A3sletPodStatus> {
        let pods = self.pods.read().await;
        let pod_key = format!("{}/{}", namespace, name);
        pods.get(&pod_key).map(|p| p.status.clone())
    }

    /// List all pods managed by this a3slet.
    pub async fn list_pods(&self) -> Vec<A3sletPodSpec> {
        let pods = self.pods.read().await;
        pods.values().map(|p| p.spec.clone()).collect()
    }

    /// Create a new pod.
    async fn create_pod(
        &self,
        pods: &mut HashMap<String, ManagedPod>,
        spec: A3sletPodSpec,
    ) -> Result<()> {
        let pod_key = format!("{}/{}", spec.namespace, spec.name);
        let mut containers: HashMap<String, RunningContainer> = HashMap::new();

        // Create volumes first
        for volume in &spec.volumes {
            self.ensure_volume(&spec, volume).await?;
        }

        // Start each container
        for container_spec in &spec.containers {
            let container = self.start_container(&spec, container_spec).await?;
            containers.insert(container_spec.name.clone(), container);
        }

        let status = A3sletPodStatus {
            name: spec.name.clone(),
            namespace: spec.namespace.clone(),
            uid: spec.uid.clone(),
            container_statuses: vec![],
            phase: PodPhase::Running,
            message: None,
            reason: None,
            host_ip: "127.0.0.1".to_string(),
            pod_ip: "10.0.0.1".to_string(),
            start_time: Utc::now(),
        };

        let managed_pod = ManagedPod {
            spec,
            containers,
            status,
        };

        pods.insert(pod_key, managed_pod);
        Ok(())
    }

    /// Update an existing pod.
    async fn update_pod(&self, pod: &mut ManagedPod, new_spec: A3sletPodSpec) -> Result<()> {
        // Find containers to add, remove, or restart
        let new_container_names: Vec<_> =
            new_spec.containers.iter().map(|c| c.name.clone()).collect();
        let old_container_names: Vec<_> =
            pod.spec.containers.iter().map(|c| c.name.clone()).collect();

        // Remove containers that are no longer needed
        for name in &old_container_names {
            if !new_container_names.contains(name) {
                if let Some(mut container) = pod.containers.remove(name) {
                    let _ = container.child.kill().await;
                    tracing::debug!(container = %name, "Container removed");
                }
            }
        }

        // Add or restart containers
        for container_spec in &new_spec.containers {
            if let Some(existing) = pod.containers.get_mut(container_spec.name.as_str()) {
                // Check if container needs restart (spec changed)
                if self.container_spec_changed(&pod.spec, container_spec) {
                    tracing::info!(container = %container_spec.name, "Restarting container");
                    let _ = existing.child.kill().await;
                    if let Ok(new_container) = self.start_container(&new_spec, container_spec).await
                    {
                        *existing = new_container;
                    }
                }
            } else {
                // Start new container
                tracing::info!(container = %container_spec.name, "Starting container");
                if let Ok(container) = self.start_container(&new_spec, container_spec).await {
                    pod.containers
                        .insert(container_spec.name.clone(), container);
                }
            }
        }

        pod.spec = new_spec;
        Ok(())
    }

    /// Check if container spec changed.
    fn container_spec_changed(
        &self,
        old_spec: &A3sletPodSpec,
        new_spec: &A3sletContainerSpec,
    ) -> bool {
        if let Some(old_container) = old_spec.containers.iter().find(|c| c.name == new_spec.name) {
            old_container.image != new_spec.image
                || old_container.command != new_spec.command
                || old_container.args != new_spec.args
        } else {
            true
        }
    }

    /// Ensure volume is prepared.
    async fn ensure_volume(&self, pod_spec: &A3sletPodSpec, volume: &A3sletVolume) -> Result<()> {
        match &volume.source {
            A3sletVolumeSource::EmptyDir => {
                let volume_path = self.get_pod_volume_path(pod_spec, &volume.name);
                tokio::fs::create_dir_all(&volume_path).await?;
            }
            A3sletVolumeSource::HostPath { path, kind } => {
                if matches!(
                    kind,
                    HostPathKind::DirectoryOrCreate | HostPathKind::FileOrCreate
                ) {
                    tokio::fs::create_dir_all(path).await?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    /// Get pod volume path.
    fn get_pod_volume_path(&self, spec: &A3sletPodSpec, volume_name: &str) -> PathBuf {
        self.config
            .volume_dir
            .join(&spec.namespace)
            .join(&spec.name)
            .join(volume_name)
    }

    /// Start a container.
    async fn start_container(
        &self,
        pod_spec: &A3sletPodSpec,
        container_spec: &A3sletContainerSpec,
    ) -> Result<RunningContainer> {
        // Build volume mounts for this container
        let volume_mounts: HashMap<String, PathBuf> = pod_spec
            .volumes
            .iter()
            .filter_map(|v| {
                container_spec
                    .volume_mounts
                    .iter()
                    .find(|m| m.name == v.name)
                    .map(|m| {
                        let _mount_path = PathBuf::from(&m.mount_path);
                        let source = self.get_pod_volume_path(pod_spec, &v.name);
                        (m.mount_path.clone(), source)
                    })
            })
            .collect();

        // Use a3s-box CLI to run the container
        let mut args = vec![
            "box".to_string(),
            "run".to_string(),
            "--name".to_string(),
            format!("{}-{}", pod_spec.name, container_spec.name),
            "--image".to_string(),
            container_spec.image.clone(),
        ];

        // Add command
        if !container_spec.command.is_empty() {
            args.push("--cmd".to_string());
            args.push(container_spec.command.join(" "));
        }

        // Add environment variables
        for (key, value) in &container_spec.env {
            args.push("--env".to_string());
            args.push(format!("{}={}", key, value));
        }

        // Add volume mounts
        for (mount_path, source) in &volume_mounts {
            args.push("--mount".to_string());
            args.push(format!("{}:{}", source.display(), mount_path));
        }

        // Add working directory
        if let Some(ref wd) = container_spec.working_dir {
            args.push("--workdir".to_string());
            args.push(wd.clone());
        }

        tracing::debug!(
            container = %container_spec.name,
            image = %container_spec.image,
            cmd = ?args,
            "Starting container"
        );

        // Execute via a3s-box CLI
        let child = Command::new("a3s")
            .args(&args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                A3sError::Other(format!(
                    "failed to spawn container {}: {}",
                    container_spec.name, e
                ))
            })?;

        Ok(RunningContainer {
            name: container_spec.name.clone(),
            child,
            exit_code: None,
        })
    }

    /// Check container health via probes.
    pub async fn check_probe(&self, probe: &A3sletProbe) -> bool {
        match &probe.probe_type {
            ProbeType::Http => {
                if let Some(ref http) = probe.http_get {
                    self.check_http_probe(http).await
                } else {
                    false
                }
            }
            ProbeType::Exec => {
                if let Some(ref exec) = probe.exec {
                    self.check_exec_probe(exec).await
                } else {
                    false
                }
            }
            ProbeType::Tcp => {
                if let Some(ref tcp) = probe.tcp_socket {
                    self.check_tcp_probe(tcp).await
                } else {
                    false
                }
            }
        }
    }

    /// Check HTTP probe.
    async fn check_http_probe(&self, http: &HttpProbe) -> bool {
        let url = format!(
            "{}://{}:{}{}",
            http.scheme,
            http.host.as_deref().unwrap_or("localhost"),
            http.port,
            http.path
        );
        // Simplified - in real implementation would do HTTP request
        tracing::debug!(url = %url, "HTTP probe check");
        true
    }

    /// Check exec probe.
    async fn check_exec_probe(&self, exec: &ExecProbe) -> bool {
        let result = Command::new(&exec.command[0])
            .args(&exec.command[1..])
            .output()
            .await;
        result.map(|o| o.status.success()).unwrap_or(false)
    }

    /// Check TCP probe.
    async fn check_tcp_probe(&self, tcp: &TcpSocketProbe) -> bool {
        let addr = format!(
            "{}:{}",
            tcp.host.as_deref().unwrap_or("localhost"),
            tcp.port
        );
        // Simplified - would use TCP connection check
        tracing::debug!(addr = %addr, "TCP probe check");
        true
    }

    /// Get a3slet config.
    pub fn config(&self) -> &A3sletConfig {
        &self.config
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

/// A3slet event for status reporting.
#[derive(Debug, Clone)]
pub struct A3sletEvent {
    /// Event type.
    pub event_type: A3sletEventType,
    /// Pod namespace.
    pub namespace: String,
    /// Pod name.
    pub pod_name: String,
    /// Container name (optional).
    pub container_name: Option<String>,
    /// Timestamp.
    pub timestamp: DateTime<Utc>,
    /// Message.
    pub message: String,
}

/// Event type.
#[derive(Debug, Clone)]
pub enum A3sletEventType {
    /// Pod created.
    PodCreated,
    /// Pod started.
    PodStarted,
    /// Pod stopped.
    PodStopped,
    /// Pod deleted.
    PodDeleted,
    /// Container started.
    ContainerStarted,
    /// Container stopped.
    ContainerStopped,
    /// Container restarted.
    ContainerRestarted,
    /// Probe failure.
    ProbeFailure,
    /// Volume mounted.
    VolumeMounted,
    /// Volume unmounted.
    VolumeUnmounted,
}

impl Default for A3slet {
    fn default() -> Self {
        Self {
            config: A3sletConfig::default(),
            pods: RwLock::new(HashMap::new()),
            event_sender: mpsc::channel(100).0,
            running: RwLock::new(false),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_a3slet_creation() {
        let config = A3sletConfig::default();
        let a3slet = A3slet::new(config).await.unwrap();
        assert!(!a3slet.is_running().await);
    }

    #[tokio::test]
    #[ignore] // Requires container runtime (nginx image)
    async fn test_pod_lifecycle() {
        let config = A3sletConfig::default();
        let a3slet = A3slet::new(config).await.unwrap();

        let pod_spec = A3sletPodSpec {
            name: "test-pod".to_string(),
            namespace: "default".to_string(),
            uid: "test-uid".to_string(),
            containers: vec![A3sletContainerSpec {
                name: "nginx".to_string(),
                image: "nginx:latest".to_string(),
                image_pull_policy: ImagePullPolicy::Always,
                command: vec![],
                args: vec![],
                working_dir: None,
                env: HashMap::new(),
                volume_mounts: vec![],
                read_only: false,
                liveness_probe: None,
                readiness_probe: None,
            }],
            volumes: vec![],
            restart_policy: "Always".to_string(),
            node_name: "test-node".to_string(),
            host_network: false,
            host_pid: false,
            host_ipc: false,
        };

        // Sync pod
        a3slet.sync_pod(pod_spec.clone()).await.unwrap();

        // List pods
        let pods = a3slet.list_pods().await;
        assert_eq!(pods.len(), 1);

        // Delete pod
        a3slet.delete_pod("default", "test-pod").await.unwrap();

        let pods = a3slet.list_pods().await;
        assert_eq!(pods.len(), 0);
    }
}
