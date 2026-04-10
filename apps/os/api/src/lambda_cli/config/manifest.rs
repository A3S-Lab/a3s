//! Manifest schema for K8s-style declarative deployment.
//!
//! Supports HCL format with Kind, apiVersion, metadata, and spec blocks.

use crate::state::DaemonSetSpec;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Manifest document root.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Manifest {
    /// API version (e.g., "a3s.io/v1").
    #[serde(rename = "apiVersion")]
    pub api_version: String,

    /// Resource kind (e.g., "Service", "Deployment", "Model").
    pub kind: String,

    /// Standard Kubernetes-style metadata.
    pub metadata: ObjectMeta,

    /// Resource-specific specification.
    #[serde(default)]
    pub spec: ManifestSpec,
}

/// Standard object metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub name: String,

    #[serde(rename = "generateName", default)]
    pub generate_name: Option<String>,

    #[serde(default)]
    pub namespace: Option<String>,

    #[serde(default)]
    pub labels: HashMap<String, String>,

    #[serde(default)]
    pub annotations: HashMap<String, String>,

    #[serde(default)]
    pub uid: Option<String>,

    #[serde(rename = "creationTimestamp", default)]
    pub creation_timestamp: Option<String>,
}

/// Manifest specification - kind-specific content.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ManifestSpec {
    Service(ServiceSpec),
    Deployment(DeploymentSpec),
    DaemonSet(DaemonSetSpec),
    Model(ModelSpec),
    ConfigMap(ConfigMapSpec),
    Secret(SecretSpec),
    Revision(RevisionSpec),
    /// Raw spec for extensibility.
    Raw(serde_json::Value),
}

impl Default for ManifestSpec {
    fn default() -> Self {
        ManifestSpec::Raw(serde_json::Value::Object(serde_json::Map::new()))
    }
}

/// Service specification (network exposure).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceSpec {
    /// Service type.
    #[serde(rename = "serviceType", default)]
    pub service_type: ServiceType,

    /// Port definitions.
    pub ports: Vec<ServicePort>,

    /// Selector for pods.
    #[serde(default)]
    pub selector: HashMap<String, String>,
}

/// Service type.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum ServiceType {
    ClusterIP,
    NodePort,
    LoadBalancer,
    #[default]
    ExternalName,
}

/// Service port definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServicePort {
    pub name: String,
    pub port: u16,

    #[serde(rename = "targetPort", default)]
    pub target_port: Option<u16>,

    #[serde(default)]
    pub protocol: Option<String>,

    #[serde(rename = "nodePort", default)]
    pub node_port: Option<u16>,
}

/// Deployment specification (workload with replicas).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentSpec {
    /// Number of replicas.
    #[serde(default = "default_replicas")]
    pub replicas: i32,

    /// Pod selector.
    pub selector: LabelSelector,

    /// Pod template.
    pub template: PodTemplateSpec,
}

/// Label selector.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelSelector {
    #[serde(rename = "matchLabels", default)]
    pub match_labels: HashMap<String, String>,

    #[serde(rename = "matchExpressions", default)]
    pub match_expressions: Vec<LabelSelectorRequirement>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LabelSelectorRequirement {
    pub key: String,
    pub operator: String,

    #[serde(default)]
    pub values: Vec<String>,
}

/// Pod template specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodTemplateSpec {
    #[serde(default)]
    pub metadata: Option<ObjectMeta>,

    pub spec: PodSpec,
}

/// Pod specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PodSpec {
    /// Containers.
    pub containers: Vec<Container>,

    #[serde(default)]
    pub volumes: Vec<Volume>,

    #[serde(rename = "restartPolicy", default)]
    pub restart_policy: Option<String>,
}

/// Container specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Container {
    pub name: String,
    pub image: String,

    #[serde(rename = "imagePullPolicy", default)]
    pub image_pull_policy: Option<String>,

    #[serde(default)]
    pub command: Option<Vec<String>>,

    #[serde(default)]
    pub args: Option<Vec<String>>,

    #[serde(default)]
    pub env: Vec<EnvVar>,

    #[serde(rename = "envFrom", default)]
    pub env_from: Vec<EnvSource>,

    #[serde(default)]
    pub ports: Vec<ContainerPort>,

    #[serde(default)]
    pub resources: Option<ResourceRequirements>,
}

/// Environment variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVar {
    pub name: String,

    #[serde(default)]
    pub value: Option<String>,

    #[serde(rename = "valueFrom", default)]
    pub value_from: Option<EnvVarSource>,
}

/// Environment source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvSource {
    #[serde(rename = "configMapRef", default)]
    pub config_map_ref: Option<ConfigMapRef>,

    #[serde(rename = "secretRef", default)]
    pub secret_ref: Option<SecretRef>,
}

/// ConfigMap reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapRef {
    pub name: String,
}

/// Secret reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretRef {
    pub name: String,
}

/// Environment variable source.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnvVarSource {
    #[serde(default)]
    pub config_map_key_ref: Option<ConfigMapKeyRef>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapKeyRef {
    pub name: String,
    pub key: String,
}

/// Container port.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerPort {
    #[serde(rename = "containerPort")]
    pub container_port: u16,

    #[serde(default)]
    pub protocol: Option<String>,

    #[serde(default)]
    pub name: Option<String>,
}

/// Resource requirements.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceRequirements {
    #[serde(default)]
    pub limits: HashMap<String, String>,

    #[serde(default)]
    pub requests: HashMap<String, String>,
}

/// Volume definition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Volume {
    pub name: String,

    #[serde(default)]
    pub host_path: Option<HostPathVolumeSource>,

    #[serde(default)]
    pub config_map: Option<ConfigMapVolumeSource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HostPathVolumeSource {
    pub path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapVolumeSource {
    pub name: String,
}

/// Model specification (AI model deployment).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelSpec {
    /// Model provider (ollama, vllm, openai).
    pub provider: String,

    /// Model name.
    pub name: String,

    #[serde(default)]
    pub repository: Option<String>,

    #[serde(default)]
    pub api_key: Option<String>,

    #[serde(default)]
    pub endpoint: Option<String>,

    #[serde(default)]
    pub deployment: ModelDeploymentConfig,
}

/// Model deployment configuration.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ModelDeploymentConfig {
    #[serde(rename = "type", default = "default_deployment_type")]
    pub deployment_type: String,

    #[serde(default)]
    pub gpu: bool,

    #[serde(default)]
    pub memory: Option<String>,

    #[serde(default)]
    pub replicas: Option<i32>,

    #[serde(rename = "minReplicas", default)]
    pub min_replicas: Option<i32>,

    #[serde(rename = "maxReplicas", default)]
    pub max_replicas: Option<i32>,
}

/// ConfigMap specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfigMapSpec {
    pub data: HashMap<String, String>,

    #[serde(rename = "binaryData", default)]
    pub binary_data: HashMap<String, String>,

    #[serde(default)]
    pub immutable: bool,
}

/// Secret specification (sensitive data).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecretSpec {
    /// Secret type (determines how data is interpreted).
    #[serde(rename = "type", default)]
    pub secret_type: SecretType,

    /// Secret data (must be base64 encoded in K8s style).
    pub data: HashMap<String, String>,

    /// String data (plain text, will be base64 encoded on apply).
    #[serde(default)]
    pub string_data: HashMap<String, String>,

    #[serde(default)]
    pub immutable: bool,
}

/// Secret type (K8s-compatible).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SecretType {
    /// Opaque secret (generic).
    #[default]
    Opaque,
    /// Service account token.
    ServiceAccountToken,
    /// Docker config JSON.
    DockerConfigJson,
    /// Basic auth.
    BasicAuth,
    /// TLS cert.
    Tls,
}

/// Revision specification (immutable snapshot, Knative-style).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RevisionSpec {
    pub container: Container,

    #[serde(rename = "minScale", default)]
    pub min_scale: Option<i32>,

    #[serde(rename = "maxScale", default)]
    pub max_scale: Option<i32>,

    #[serde(rename = "concurrencyLimit", default)]
    pub concurrency_limit: Option<i32>,

    #[serde(default)]
    pub timeout: Option<String>,
}

fn default_replicas() -> i32 {
    1
}

fn default_deployment_type() -> String {
    "microvm".to_string()
}

impl Manifest {
    /// Check if this is a supported kind.
    pub fn is_supported_kind(&self) -> bool {
        matches!(
            self.kind.as_str(),
            "Service" | "Deployment" | "Model" | "ConfigMap" | "Secret" | "Revision"
        )
    }
}
