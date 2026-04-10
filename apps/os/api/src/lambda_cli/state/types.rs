//! State types for the control plane.
//!
//! Defines the desired state (user intent) and actual state (runtime reality).

use crate::state::node::{NodeSelector, Toleration};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Desired state for a deployment.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentDesired {
    /// Deployment name.
    pub name: String,
    /// Namespace (for isolation).
    pub namespace: String,
    /// Container image.
    pub image: String,
    /// Desired replica count.
    pub replicas: i32,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Port forwards.
    #[serde(default)]
    pub ports: Vec<PortMapping>,
    /// Current version (for rolling updates).
    pub version: String,
    /// Update strategy.
    #[serde(default)]
    pub strategy: UpdateStrategy,
    /// Resource limits.
    #[serde(default)]
    pub resources: ResourceRequirements,
    /// Health check config.
    #[serde(default)]
    pub health_check: HealthCheckConfig,
    /// Node selector for scheduling.
    #[serde(default)]
    pub node_selector: NodeSelector,
    /// Tolerations for node taints.
    #[serde(default)]
    pub tolerations: Vec<Toleration>,
    /// Labels for the deployment.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Port mapping for a container.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PortMapping {
    pub name: String,
    pub container_port: u16,
    pub host_port: u16,
    pub protocol: String,
}

/// Update strategy for deployments.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum UpdateStrategy {
    /// Replace all pods at once.
    Replace,
    /// Rolling update - replace one at a time.
    RollingUpdate(RollingUpdateConfig),
}

impl Default for UpdateStrategy {
    fn default() -> Self {
        UpdateStrategy::RollingUpdate(RollingUpdateConfig::default())
    }
}

impl UpdateStrategy {
    pub fn rolling_update_config(&self) -> Option<&RollingUpdateConfig> {
        match self {
            UpdateStrategy::RollingUpdate(c) => Some(c),
            _ => None,
        }
    }
}

/// Configuration for rolling updates.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingUpdateConfig {
    /// Maximum number of pods that can be created above the desired count during update.
    /// Can be an absolute number or percentage (e.g., "25%").
    #[serde(default = "default_max_surge")]
    pub max_surge: SurgeOrUnavailable,

    /// Maximum number of pods that can be unavailable during update.
    /// Can be an absolute number or percentage (e.g., "25%").
    #[serde(default = "default_max_unavailable")]
    pub max_unavailable: SurgeOrUnavailable,

    /// Enable automatic rollback on health check failure.
    #[serde(default = "default_rollback_on_failure")]
    pub rollback_on_failure: bool,

    /// Minimum number of seconds a pod must be ready before considered available.
    #[serde(default = "default_min_ready_secs")]
    pub min_ready_secs: u32,
}

fn default_max_surge() -> SurgeOrUnavailable {
    SurgeOrUnavailable::IntOrPercent(1, false)
}
fn default_max_unavailable() -> SurgeOrUnavailable {
    SurgeOrUnavailable::IntOrPercent(0, false)
}
fn default_rollback_on_failure() -> bool {
    true
}
fn default_min_ready_secs() -> u32 {
    10
}

/// Surge or unavailable value - can be absolute number or percentage.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SurgeOrUnavailable {
    /// Absolute number.
    Int(u32),
    /// Percentage or absolute with percentage flag.
    IntOrPercent(u32, bool),
}

impl Default for SurgeOrUnavailable {
    fn default() -> Self {
        SurgeOrUnavailable::Int(0)
    }
}

impl SurgeOrUnavailable {
    /// Resolve against a total count, returning absolute number.
    pub fn resolve(&self, total: i32) -> i32 {
        match self {
            SurgeOrUnavailable::Int(n) => *n as i32,
            SurgeOrUnavailable::IntOrPercent(n, is_percent) => {
                if *is_percent {
                    ((*n as f64 / 100.0) * total as f64).ceil() as i32
                } else {
                    *n as i32
                }
            }
        }
    }
}

impl Default for RollingUpdateConfig {
    fn default() -> Self {
        Self {
            max_surge: default_max_surge(),
            max_unavailable: default_max_unavailable(),
            rollback_on_failure: default_rollback_on_failure(),
            min_ready_secs: default_min_ready_secs(),
        }
    }
}

/// State of an in-progress rolling update.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollingUpdateState {
    /// Deployment being updated.
    pub deployment: String,
    /// Target version.
    pub target_version: String,
    /// Original version before update.
    pub from_version: String,
    /// Replicas that have been updated.
    pub updated_replicas: i32,
    /// Replicas still running old version.
    pub old_replicas: i32,
    /// Replicas pending update.
    pub pending_replicas: i32,
    /// Whether rollback is in progress.
    pub rollback: bool,
}

/// Resource requirements for a container.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ResourceRequirements {
    /// Memory limit (e.g., "256Mi", "1Gi").
    #[serde(default)]
    pub memory_limit: Option<String>,
    /// CPU limit (e.g., "250m", "1").
    #[serde(default)]
    pub cpu_limit: Option<String>,
    /// Memory request.
    #[serde(default)]
    pub memory_request: Option<String>,
    /// CPU request.
    #[serde(default)]
    pub cpu_request: Option<String>,
}

/// Health check configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HealthCheckConfig {
    /// Enable health checks.
    #[serde(default)]
    pub enabled: bool,
    /// Path for HTTP health check.
    #[serde(default)]
    pub path: Option<String>,
    /// Port for health check.
    #[serde(default)]
    pub port: Option<u16>,
    /// Initial delay seconds before first check.
    #[serde(default = "default_initial_delay")]
    pub initial_delay_secs: u32,
    /// Period seconds between checks.
    #[serde(default = "default_period")]
    pub period_secs: u32,
    /// Timeout for health check.
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
    /// Failure threshold before marking unhealthy.
    #[serde(default = "default_failure_threshold")]
    pub failure_threshold: u32,
}

fn default_initial_delay() -> u32 {
    10
}
fn default_period() -> u32 {
    10
}
fn default_timeout() -> u32 {
    5
}
fn default_failure_threshold() -> u32 {
    3
}

impl Default for HealthCheckConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            path: None,
            port: None,
            initial_delay_secs: default_initial_delay(),
            period_secs: default_period(),
            timeout_secs: default_timeout(),
            failure_threshold: default_failure_threshold(),
        }
    }
}

/// Actual state of a running pod.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodActual {
    /// Unique pod ID (sandbox ID).
    pub id: String,
    /// Deployment this pod belongs to.
    pub deployment: String,
    /// Namespace.
    pub namespace: String,
    /// Current status.
    pub status: PodStatus,
    /// Health status.
    pub health: HealthStatus,
    /// Pod IP address.
    pub ip: Option<String>,
    /// Which version/image this pod is running.
    pub version: String,
    /// Sandbox socket path.
    pub socket_path: Option<String>,
    /// Node name where the pod is scheduled.
    pub node_name: Option<String>,
    /// Created timestamp.
    pub created_at: DateTime<Utc>,
    /// Last health check timestamp.
    pub last_health_check: Option<DateTime<Utc>>,
    /// consecutive failures count.
    pub consecutive_failures: u32,
    /// Ready condition (for readiness probes).
    #[serde(default)]
    pub ready: bool,
}

/// Pod status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PodStatus {
    /// Pod is being created.
    Pending,
    /// Pod is starting.
    Creating,
    /// Pod is running.
    Running,
    /// Pod is being terminated.
    Terminating,
    /// Pod has been terminated.
    Terminated,
    /// Pod failed to start.
    Failed,
}

impl Default for PodStatus {
    fn default() -> Self {
        Self::Pending
    }
}

/// Health status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Health unknown (pod just started).
    Unknown,
    /// Pod is healthy.
    Healthy,
    /// Pod is unhealthy (failing health checks).
    Unhealthy,
    /// Pod has no health check configured.
    Passthrough,
}

impl Default for HealthStatus {
    fn default() -> Self {
        Self::Unknown
    }
}

/// Service definition for service discovery.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceDesired {
    /// Service name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Service type.
    #[serde(default)]
    pub service_type: ServiceType,
    /// Port definitions.
    pub ports: Vec<ServicePort>,
    /// Selector to match pods.
    pub selector: HashMap<String, String>,
    /// Labels for the service.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// Service type.
#[derive(Debug, Clone, Copy, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ServiceType {
    /// Cluster-internal only.
    #[default]
    ClusterIP,
    /// Expose on each node's IP.
    NodePort,
    /// External load balancer.
    LoadBalancer,
}

/// Service port definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    pub name: String,
    pub port: u16,
    pub target_port: u16,
    pub protocol: String,
}

/// Service actual state (endpoints).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceActual {
    /// Service name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Current endpoints (pod IDs).
    #[serde(default)]
    pub endpoints: Vec<String>,
    /// Load balancer IP (if type is LoadBalancer).
    pub load_balancer_ip: Option<String>,
}

/// ConfigMap desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapDesired {
    /// ConfigMap name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Config data (key-value pairs).
    pub data: HashMap<String, String>,
    /// Binary data.
    #[serde(default)]
    pub binary_data: HashMap<String, String>,
    /// Whether immutable.
    #[serde(default)]
    pub immutable: bool,
    /// Labels for the ConfigMap.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Secret desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretDesired {
    /// Secret name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Secret type.
    #[serde(rename = "type", default)]
    pub secret_type: String,
    /// Secret data (base64 encoded in K8s style).
    pub data: HashMap<String, String>,
    /// String data (plain text, will be base64 encoded).
    #[serde(default)]
    pub string_data: HashMap<String, String>,
    /// Whether immutable.
    #[serde(default)]
    pub immutable: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Ingress rule - matches requests to a backend service.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressRule {
    /// Host to match (e.g., "example.com" or "*" for any).
    pub host: String,
    /// HTTP paths and their backends.
    #[serde(default)]
    pub paths: Vec<IngressPath>,
}

/// A single path rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressPath {
    /// Path prefix to match (e.g., "/api" or "/").
    pub path: String,
    /// Service name to forward to.
    pub backend: String,
    /// Backend port.
    pub port: u16,
}

/// TLS configuration for Ingress.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngressTls {
    /// Hosts to terminate TLS for.
    #[serde(default)]
    pub hosts: Vec<String>,
    /// Secret name containing TLS cert/key.
    #[serde(default)]
    pub secret_name: String,
}

/// Ingress specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct IngressSpec {
    /// Ingress rules.
    #[serde(default)]
    pub rules: Vec<IngressRule>,
    /// TLS configuration.
    #[serde(default)]
    pub tls: Vec<IngressTls>,
}

/// Ingress desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressDesired {
    /// Ingress name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Ingress specification.
    pub spec: IngressSpec,
    /// Labels for the ingress.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// IngressClass parameters reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressClassParameters {
    /// APIVersion of the parameters.
    #[serde(default)]
    pub api_version: Option<String>,
    /// Kind of the parameters.
    #[serde(default)]
    pub kind: Option<String>,
    /// Name of the parameters.
    #[serde(default)]
    pub name: Option<String>,
    /// Namespace of the parameters (if scoped).
    #[serde(default)]
    pub namespace: Option<String>,
}

/// IngressClass desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngressClassDesired {
    /// Name.
    pub name: String,
    /// Controller name (e.g., "traefik.io/ingress-controller").
    pub controller: String,
    /// Ingress class parameters.
    #[serde(default)]
    pub parameters: Option<IngressClassParameters>,
    /// Default ingress class for the cluster.
    #[serde(default)]
    pub is_default: bool,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
}

/// ReplicaSet selector - matches pods.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicaSetSelector {
    /// Match labels.
    #[serde(default)]
    pub match_labels: HashMap<String, String>,
    /// Match expressions (uses LabelSelectorExpression from network_policy).
    #[serde(default)]
    pub match_expressions: Vec<crate::state::network_policy::LabelSelectorExpression>,
}

/// ReplicaSet specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicaSetSpec {
    /// Minimum number of seconds a pod must be ready.
    pub min_ready_seconds: Option<u32>,
    /// Replicas.
    pub replicas: Option<i32>,
    /// Selector.
    pub selector: ReplicaSetSelector,
    /// Template.
    pub template: Option<PodTemplateSpec>,
}

/// Pod template spec for ReplicaSet.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodTemplateSpec {
    /// Metadata.
    pub metadata: PodTemplateMetadata,
    /// Spec.
    pub spec: Option<PodTemplateSpecCore>,
}

/// Pod template metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodTemplateMetadata {
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// Core pod spec for template.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct PodTemplateSpecCore {
    /// Containers.
    #[serde(default)]
    pub containers: Vec<ContainerSpec>,
    /// Restart policy.
    #[serde(default)]
    pub restart_policy: Option<String>,
}

/// Container specification.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContainerSpec {
    pub name: String,
    pub image: String,
    #[serde(default)]
    pub command: Vec<String>,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: Vec<EnvVar>,
    #[serde(default)]
    pub ports: Vec<ContainerPort>,
    #[serde(default)]
    pub resources: ResourceRequirements,
}

/// Environment variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,
    pub value: Option<String>,
}

/// Container port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPort {
    pub container_port: u16,
    #[serde(default)]
    pub protocol: Option<String>,
}

/// ReplicaSet desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplicaSetDesired {
    /// ReplicaSet name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// ReplicaSet specification.
    pub spec: ReplicaSetSpec,
    /// Labels for the ReplicaSet.
    #[serde(default)]
    pub labels: HashMap<String, String>,
    /// Owner references (for cleanup when owner is deleted).
    #[serde(default)]
    pub owner_references: Vec<OwnerReference>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Owner reference for garbage collection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OwnerReference {
    pub api_version: String,
    pub kind: String,
    pub name: String,
    pub uid: String,
    #[serde(default)]
    pub block_owner_deletion: bool,
    #[serde(default)]
    pub controller: bool,
}

/// ReplicaSet actual state (status).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ReplicaSetStatus {
    /// Replicas.
    pub replicas: i32,
    /// Fully labeled replicas.
    pub fully_labeled_replicas: i32,
    /// Ready replicas.
    pub ready_replicas: i32,
    /// Available replicas.
    pub available_replicas: i32,
    /// Observed generation.
    pub observed_generation: i64,
}

/// Lease spec for leader election (coordination.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseSpec {
    /// Holder identity (who currently holds the lease).
    pub holder_identity: Option<String>,
    /// Lease duration in seconds.
    #[serde(default)]
    pub lease_duration_seconds: i32,
    /// Time when lease was acquired.
    pub acquire_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Time when lease was last renewed.
    pub renew_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Last update time.
    pub last_update_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Number of leader transitions.
    #[serde(default)]
    pub lease_transitions: i32,
}

/// Lease desired state for leader election (coordination.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LeaseDesired {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Spec.
    pub spec: LeaseSpec,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// CSIDriver spec (storage.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CSIDriverSpec {
    /// Name of the CSI driver.
    pub driver: String,
    /// Whether to enable volume publishing.
    #[serde(default)]
    pub attach_required: bool,
    /// Whether to enable pod info mount.
    #[serde(default)]
    pub pod_info_on_mount: bool,
    /// Mount options to pass to CSI driver.
    #[serde(default)]
    pub mount_options: Vec<String>,
    /// Volume lifecycle modes.
    #[serde(default)]
    pub volume_lifecycle_modes: Vec<String>,
}

/// CSIDriver desired state (storage.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CSIDriverDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: CSIDriverSpec,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// CSINode spec (storage.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CSINodeSpec {
    /// List of drivers on this node.
    #[serde(default)]
    pub drivers: Vec<CSINodeDriver>,
}

/// CSI driver information on a node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CSINodeDriver {
    /// Name of the CSI driver.
    pub name: String,
    /// Address of the CSI driver.
    pub address: String,
    /// Whether the driver is TopologyAware.
    #[serde(default)]
    pub topology_keys: Vec<String>,
}

/// CSINode desired state (storage.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CSINodeDesired {
    /// Name (node name).
    pub name: String,
    /// Spec.
    pub spec: CSINodeSpec,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// VolumeAttachment spec (storage.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAttachmentSpec {
    /// The volume to attach.
    pub volume: String,
    /// The node to attach the volume to.
    pub node_name: String,
    /// Volume source.
    pub attacher: String,
    /// Source of the volume.
    pub source: Option<VolumeAttachmentSource>,
}

/// Volume attachment source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAttachmentSource {
    /// Persistent volume reference.
    pub persistent_volume_name: Option<String>,
    /// Inline volume spec (for CSI drivers).
    #[serde(default)]
    pub inline_volume_spec: Option<serde_json::Value>,
}

/// VolumeAttachment status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAttachmentStatus {
    /// Whether the volume is attached.
    #[serde(default)]
    pub attached: bool,
    /// Attachment metadata.
    #[serde(default)]
    pub attachment_metadata: std::collections::HashMap<String, String>,
    /// Error message if attachment failed.
    pub error: Option<String>,
}

/// VolumeAttachment desired state (storage.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VolumeAttachmentDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: VolumeAttachmentSpec,
    /// Status.
    pub status: Option<VolumeAttachmentStatus>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// FlowSchema spec (flowcontrol.apiserver.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSchemaSpec {
    /// Priority level for matching requests.
    pub priority_level: String,
    /// Who this FlowSchema applies to.
    #[serde(default)]
    pub rules: Vec<FlowSchemaRule>,
}

/// FlowSchema rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSchemaRule {
    /// Resource requests to match.
    #[serde(default)]
    pub resource_rules: Vec<ResourcePolicyRule>,
    /// Non-resource requests to match.
    #[serde(default)]
    pub non_resource_rules: Vec<NonResourcePolicyRule>,
    /// FlowSchema conditions.
    #[serde(default)]
    pub subjects: Vec<FlowSubject>,
}

/// Resource policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourcePolicyRule {
    /// Resource groups to match.
    #[serde(default)]
    pub resource_groups: Vec<String>,
    /// Resource names to match.
    #[serde(default)]
    pub resources: Vec<String>,
    /// Verbs to match.
    #[serde(default)]
    pub verbs: Vec<String>,
    /// Namespace criterion.
    #[serde(default)]
    pub namespaces: Vec<String>,
}

/// Non-resource policy rule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NonResourcePolicyRule {
    /// Non-resource URLs to match.
    #[serde(default)]
    pub non_resource_urls: Vec<String>,
    /// Verbs to match.
    #[serde(default)]
    pub verbs: Vec<String>,
}

/// Subject for RBAC (FlowSchema).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSubject {
    /// Kind of subject (User, Group, ServiceAccount).
    pub kind: String,
    /// Name of subject.
    pub name: String,
    /// Namespace (for ServiceAccount).
    #[serde(default)]
    pub namespace: Option<String>,
}

/// FlowSchema status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSchemaStatus {
    /// Current state of FlowSchema.
    #[serde(default)]
    pub conditions: Vec<FlowSchemaCondition>,
}

/// FlowSchema condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSchemaCondition {
    /// Condition type.
    #[serde(default)]
    pub r#type: String,
    /// Condition status.
    #[serde(default)]
    pub status: String,
    /// Last transition time.
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Reason for condition.
    #[serde(default)]
    pub reason: String,
    /// Message about condition.
    #[serde(default)]
    pub message: String,
}

/// FlowSchema desired state (flowcontrol.apiserver.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FlowSchemaDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: FlowSchemaSpec,
    /// Status.
    pub status: Option<FlowSchemaStatus>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PriorityLevelConfiguration spec (flowcontrol.apiserver.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityLevelConfigurationSpec {
    /// Type of priority level (Exempt, Limited).
    #[serde(default)]
    pub r#type: String,
    /// Limited priority level configuration.
    #[serde(default)]
    pub limited: Option<LimitedPriorityLevelConfiguration>,
}

/// Limited priority level configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitedPriorityLevelConfiguration {
    /// Assured fairness.
    #[serde(default)]
    pub assure_fairness: bool,
    /// Nominal concurrency limit.
    #[serde(default)]
    pub nominal_concurrency_shares: i32,
    /// Limit that applies to this priority level.
    #[serde(default)]
    pub limit: Option<LimitSettings>,
}

/// Limit settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LimitSettings {
    /// Type of limit (InQueue, InFlight).
    #[serde(default)]
    pub limit_type: String,
    /// The limit value.
    #[serde(default)]
    pub value: i64,
}

/// PriorityLevelConfiguration status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityLevelConfigurationStatus {
    /// Current state of PriorityLevelConfiguration.
    #[serde(default)]
    pub conditions: Vec<PriorityLevelCondition>,
}

/// PriorityLevelConfiguration condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityLevelCondition {
    /// Condition type.
    #[serde(default)]
    pub r#type: String,
    /// Condition status.
    #[serde(default)]
    pub status: String,
    /// Last transition time.
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Reason for condition.
    #[serde(default)]
    pub reason: String,
    /// Message about condition.
    #[serde(default)]
    pub message: String,
}

/// PriorityLevelConfiguration desired state (flowcontrol.apiserver.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityLevelConfigurationDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: PriorityLevelConfigurationSpec,
    /// Status.
    pub status: Option<PriorityLevelConfigurationStatus>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// CertificateSigningRequest spec (certificates.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateSigningRequestSpec {
    /// Requested signer.
    pub signer_name: String,
    /// Certificate request data (base64 encoded).
    pub request: String,
    /// Allowed Usages.
    #[serde(default)]
    pub usages: Vec<String>,
    /// Credentials.
    #[serde(default)]
    pub credentials: Option<String>,
}

/// CertificateSigningRequest status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateSigningRequestStatus {
    /// Certificate (base64 encoded).
    #[serde(default)]
    pub certificate: Option<String>,
    /// Condition information.
    #[serde(default)]
    pub conditions: Vec<CertificateSigningRequestCondition>,
}

/// CertificateSigningRequest condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateSigningRequestCondition {
    /// Condition type (Approved, Denied).
    pub r#type: String,
    /// Status (True, False, Unknown).
    pub status: String,
    /// Last transition time.
    pub last_transition_time: Option<chrono::DateTime<chrono::Utc>>,
    /// Reason for condition.
    pub reason: String,
    /// Message about condition.
    pub message: String,
}

/// CertificateSigningRequest desired state (certificates.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CertificateSigningRequestDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: CertificateSigningRequestSpec,
    /// Status.
    pub status: Option<CertificateSigningRequestStatus>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// MutatingWebhookConfiguration spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutatingWebhookConfigurationSpec {
    /// Webhooks.
    #[serde(default)]
    pub webhooks: Vec<Webhook>,
}

/// ValidatingWebhookConfiguration spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatingWebhookConfigurationSpec {
    /// Webhooks.
    #[serde(default)]
    pub webhooks: Vec<Webhook>,
}

/// Webhook definition.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Webhook {
    /// Name of webhook.
    pub name: String,
    /// Client config for webhook.
    #[serde(default)]
    pub client_config: WebhookClientConfig,
    /// Rules when webhook applies.
    #[serde(default)]
    pub rules: Vec<RuleWithOperations>,
}

/// Webhook client config.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct WebhookClientConfig {
    /// URL for webhook.
    #[serde(default)]
    pub url: Option<String>,
    /// Service reference.
    #[serde(default)]
    pub service: Option<ServiceReference>,
    /// CA bundle.
    #[serde(default)]
    pub ca_bundle: Option<String>,
}

/// Service reference for webhook.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceReference {
    /// Namespace of service.
    pub namespace: String,
    /// Name of service.
    pub name: String,
    /// Port of service.
    #[serde(default)]
    pub port: Option<u16>,
}

/// Rule with operations.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuleWithOperations {
    /// Operations (CREATE, UPDATE, DELETE, CONNECT).
    #[serde(default)]
    pub operations: Vec<String>,
    /// Rule scope (Cluster, Namespaced, NonResource).
    #[serde(default)]
    pub scope: Option<String>,
    /// API groups.
    #[serde(default)]
    pub api_groups: Vec<String>,
    /// API versions.
    #[serde(default)]
    pub api_versions: Vec<String>,
    /// Resources.
    #[serde(default)]
    pub resources: Vec<String>,
    /// Non-resource URLs.
    #[serde(default)]
    pub non_resource_urls: Vec<String>,
}

/// MutatingWebhookConfiguration desired state (admissionregistration.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MutatingWebhookConfigurationDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: MutatingWebhookConfigurationSpec,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// ValidatingWebhookConfiguration desired state (admissionregistration.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidatingWebhookConfigurationDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: ValidatingWebhookConfigurationSpec,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// PriorityClass spec (scheduling.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityClassSpec {
    /// Preemption policy (PreemptLowerPriority, Never).
    #[serde(default)]
    pub preemption_policy: String,
    /// Global default priority value.
    #[serde(default)]
    pub global_default: bool,
    /// Priority value.
    pub value: i32,
    /// Description.
    #[serde(default)]
    pub description: String,
}

/// PriorityClass desired state (scheduling.k8s.io/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PriorityClassDesired {
    /// Name.
    pub name: String,
    /// Spec.
    pub spec: PriorityClassSpec,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}

/// Endpoint address (core/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointAddress {
    /// IP address.
    #[serde(default)]
    pub ip: String,
    /// Hostname.
    #[serde(default)]
    pub hostname: Option<String>,
    /// Node name.
    #[serde(default)]
    pub node_name: Option<String>,
    /// Target reference.
    #[serde(default)]
    pub target_ref: Option<EndpointTargetRef>,
}

/// Endpoint target reference (core/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointTargetRef {
    /// Kind.
    #[serde(default)]
    pub kind: String,
    /// Name.
    #[serde(default)]
    pub name: String,
    /// Namespace.
    #[serde(default)]
    pub namespace: Option<String>,
}

/// Endpoint port (core/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointPort {
    /// Port number.
    pub port: i32,
    /// Protocol.
    #[serde(default)]
    pub protocol: String,
    /// App protocol.
    #[serde(default)]
    pub app_protocol: Option<String>,
    /// Hostname.
    #[serde(default)]
    pub hostname: Option<String>,
}

/// Endpoints subset (core/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointsSubset {
    /// Addresses.
    #[serde(default)]
    pub addresses: Vec<EndpointAddress>,
    /// Not ready addresses.
    #[serde(default)]
    pub notReadyAddresses: Vec<EndpointAddress>,
    /// Ports.
    #[serde(default)]
    pub ports: Vec<EndpointPort>,
}

/// Endpoints desired state (core/v1).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EndpointsDesired {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Subsets.
    #[serde(default)]
    pub subsets: Vec<EndpointsSubset>,
    /// Creation timestamp.
    pub created_at: chrono::DateTime<chrono::Utc>,
}
