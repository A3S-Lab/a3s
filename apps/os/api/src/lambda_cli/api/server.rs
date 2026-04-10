//! REST API Server - A3S cluster API for resource management.
//!
//! Implements a REST API server compatible with A3S API patterns,
//! enabling a3s CLI and other tools to interact with the cluster.

use crate::api::field_selector::{FieldSelector, LabelSelector as FieldLabelSelector};
use crate::errors::{A3sError, Result};
use crate::state::batch::{CronJobController, JobController};
use crate::state::daemon::DaemonSetDesired;
use crate::state::event::EventController;
use crate::state::rbac::{
    ClusterRoleBindingDesired, ClusterRoleDesired, RoleBindingDesired, RoleDesired,
};
use crate::state::stateful::StatefulSetDesired;
use crate::state::volume::{PersistentVolumeClaimDesired, PersistentVolumeDesired, VolumeManager};
use crate::state::{
    AccessMode, CSIDriverDesired, CSIDriverSpec, CSINodeDesired, CertificateSigningRequestDesired,
    CertificateSigningRequestSpec, ClaimResources, ConfigMapDesired, ContainerPort, ContainerSpec,
    CronJobDesired, DaemonController, DeploymentDesired, EndpointAddress, EndpointPort,
    EndpointTargetRef, EndpointsDesired, EndpointsSubset, EnvVar, FlowSchemaDesired,
    FlowSchemaSpec, HPADesired, IngressClassDesired, IngressDesired, IngressPath, IngressRule,
    IngressSpec, IngressTls, JobDesired, LabelSelector, LabelSelectorExpression, LeaseDesired,
    LimitRangeController, LimitType, MutatingWebhookConfigurationDesired,
    MutatingWebhookConfigurationSpec, Namespace, NetworkPolicyDesired, NetworkPolicySpec,
    NodeDesired, PodActual, PodDisruptionBudgetDesired, PodDisruptionBudgetSpec,
    PodTemplateMetadata, PodTemplateSpec, PodTemplateSpecCore, PolicyType, PortMapping,
    PriorityClassDesired, PriorityClassSpec, PriorityLevelConfigurationDesired,
    PriorityLevelConfigurationSpec, RbacController, ReplicaSetDesired, ReplicaSetSelector,
    ReplicaSetSpec, ResourceQuotaController, RuntimeClass, RuntimeClassController, SecretDesired,
    ServiceAccountController, ServiceActual, ServiceDesired, ServicePort, ServiceType,
    SqliteStateStore, StatefulSetController, StorageClassController, TokenController,
    UpdateStrategy, ValidatingWebhookConfigurationDesired, ValidatingWebhookConfigurationSpec,
    VolumeAttachmentDesired, VolumeAttachmentSpec, VolumeAttachmentStatus, VolumeMode,
    VolumeStatus, Webhook, WebhookClientConfig,
};
use async_trait::async_trait;
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::RwLock as StdRwLock;
use tokio::sync::{broadcast, RwLock};
use tokio_stream::wrappers::{BroadcastStream, UnboundedReceiverStream};

/// Parse a Kubernetes-style storage string (e.g., "10Gi", "100Mi") to bytes.
fn parse_storage_string(s: &str) -> i64 {
    let s = s.trim();
    if s.is_empty() {
        return 10 * 1024 * 1024 * 1024; // Default 10Gi
    }

    let (value, unit) = if let Some(idx) = s.find(|c: char| !c.is_ascii_digit() && c != '.') {
        (&s[..idx], &s[idx..])
    } else {
        (s, "")
    };

    let value: f64 = value.parse().unwrap_or(0.0);

    match unit.to_lowercase().as_str() {
        "ki" | "kib" => (value * 1024.0) as i64,
        "mi" | "mib" => (value * 1024.0 * 1024.0) as i64,
        "gi" | "gib" => (value * 1024.0 * 1024.0 * 1024.0) as i64,
        "ti" | "tib" => (value * 1024.0 * 1024.0 * 1024.0 * 1024.0) as i64,
        "pi" | "pib" => (value * 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0) as i64,
        "ei" | "eib" => (value * 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0 * 1024.0) as i64,
        "k" | "k8" => (value * 1000.0) as i64,
        "m" => (value * 1000.0 * 1000.0) as i64,
        "g" => (value * 1000.0 * 1000.0 * 1000.0) as i64,
        "t" => (value * 1000.0 * 1000.0 * 1000.0 * 1000.0) as i64,
        "p" => (value * 1000.0 * 1000.0 * 1000.0 * 1000.0 * 1000.0) as i64,
        "e" => (value * 1000.0 * 1000.0 * 1000.0 * 1000.0 * 1000.0 * 1000.0) as i64,
        _ => (value * 1024.0 * 1024.0 * 1024.0) as i64, // Default to GiB
    }
}

/// Pagination parameters extracted from query string.
#[derive(Debug, Clone)]
pub struct PaginationParams {
    pub limit: Option<usize>,
    pub continue_token: Option<String>,
}

impl PaginationParams {
    /// Parse pagination params from query string.
    pub fn parse(query: &HashMap<String, String>) -> Self {
        let limit = query.get("limit").and_then(|v| v.parse::<usize>().ok());
        let continue_token = query.get("continue").cloned();
        Self {
            limit,
            continue_token,
        }
    }

    /// Calculate offset from continue token.
    /// Continue token is base64-encoded offset.
    pub fn offset(&self) -> usize {
        if let Some(ref token) = self.continue_token {
            use base64::Engine;
            if let Ok(decoded) = base64::engine::general_purpose::STANDARD.decode(token) {
                if let Ok(offset_str) = String::from_utf8(decoded) {
                    return offset_str.parse().unwrap_or(0);
                }
            }
        }
        0
    }
}

/// Encode offset as continue token (base64).
pub fn encode_continue_token(offset: usize) -> Option<String> {
    use base64::Engine;
    let token = offset.to_string();
    Some(base64::engine::general_purpose::STANDARD.encode(token))
}

/// Paginated list result.
pub struct PaginatedList<T> {
    pub items: Vec<T>,
    pub continue_token: Option<String>,
    pub remaining: usize,
}

impl<T> PaginatedList<T> {
    pub fn new(items: Vec<T>, total_count: usize, offset: usize) -> Self {
        let remaining = total_count.saturating_sub(offset + items.len());
        let continue_token = if remaining > 0 {
            encode_continue_token(offset + items.len())
        } else {
            None
        };
        Self {
            items,
            continue_token,
            remaining,
        }
    }
}

/// API server configuration.
#[derive(Debug, Clone)]
pub struct ApiServerConfig {
    /// Listen address.
    pub listen_addr: String,
    /// Listen port.
    pub port: u16,
    /// Enable read-only endpoints.
    pub read_only: bool,
    /// Enable debug endpoints.
    pub debug: bool,
}

impl Default for ApiServerConfig {
    fn default() -> Self {
        Self {
            listen_addr: "0.0.0.0".to_string(),
            port: 6443,
            read_only: false,
            debug: false,
        }
    }
}

/// API resource version.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceVersion {
    #[serde(rename = "resourceVersion")]
    pub resource_version: String,
}

/// API list response metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListMeta {
    #[serde(rename = "resourceVersion")]
    pub resource_version: String,
    #[serde(rename = "continue")]
    pub continue_token: Option<String>,
}

/// API object metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObjectMeta {
    pub name: String,
    pub namespace: Option<String>,
    pub uid: String,
    #[serde(rename = "resourceVersion")]
    pub resource_version: String,
    pub labels: HashMap<String, String>,
    pub annotations: HashMap<String, String>,
    #[serde(rename = "creationTimestamp")]
    pub creation_timestamp: String,
}

/// API representation of a namespace for serialization.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiNamespace {
    #[serde(rename = "apiVersion")]
    pub api_version: String,
    pub kind: String,
    pub metadata: NamespaceMeta,
}

/// Namespace metadata for API responses.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamespaceMeta {
    pub name: String,
    pub labels: HashMap<String, String>,
    #[serde(rename = "resourceVersion")]
    pub resource_version: String,
    #[serde(rename = "creationTimestamp")]
    pub creation_timestamp: String,
}

/// API status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Status {
    #[serde(rename = "type")]
    pub kind: String,
    pub api_version: String,
    pub metadata: StatusMeta,
    pub status: String,
    pub message: Option<String>,
    pub reason: Option<String>,
    pub details: Option<StatusDetails>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusMeta {
    #[serde(rename = "resourceVersion")]
    pub resource_version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatusDetails {
    pub name: Option<String>,
    pub group: Option<String>,
    pub kind: Option<String>,
    pub uid: Option<String>,
}

/// Watch event type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WatchEventType {
    Added,
    Modified,
    Deleted,
}

/// Watch event for API streaming.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiWatchEvent {
    #[serde(rename = "type")]
    pub event_type: WatchEventType,
    pub object: serde_json::Value,
    /// Resource kind (e.g., "Pod", "Service"). Used for filtering watch streams.
    pub kind: String,
}

impl ApiWatchEvent {
    /// Create an ADDED event.
    pub fn added(object: serde_json::Value, kind: String) -> Self {
        Self {
            event_type: WatchEventType::Added,
            object,
            kind,
        }
    }

    /// Create a MODIFIED event.
    pub fn modified(object: serde_json::Value, kind: String) -> Self {
        Self {
            event_type: WatchEventType::Modified,
            object,
            kind,
        }
    }

    /// Create a DELETED event.
    pub fn deleted(object: serde_json::Value, kind: String) -> Self {
        Self {
            event_type: WatchEventType::Deleted,
            object,
            kind,
        }
    }

    /// Convert to SSE event data.
    pub fn to_sse_data(&self) -> String {
        serde_json::to_string(self).unwrap_or_default()
    }
}

/// Generic watch event (for internal use).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WatchEvent<T> {
    #[serde(rename = "type")]
    pub event_type: WatchEventType,
    pub object: T,
}

/// API response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiResponse<T> {
    pub kind: String,
    pub api_version: String,
    pub metadata: Option<ObjectMeta>,
    #[serde(flatten)]
    pub data: T,
}

impl<T> ApiResponse<T> {
    pub fn ok(kind: &str, api_version: &str, data: T) -> Self {
        Self {
            kind: kind.to_string(),
            api_version: api_version.to_string(),
            metadata: None,
            data,
        }
    }

    pub fn with_meta(kind: &str, api_version: &str, metadata: ObjectMeta, data: T) -> Self {
        Self {
            kind: kind.to_string(),
            api_version: api_version.to_string(),
            metadata: Some(metadata),
            data,
        }
    }
}

/// List response wrapper.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListResponse<T> {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<T>,
}

/// Role list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleListResponse {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<RoleDesired>,
}

/// Cluster role list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleListResponse {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<ClusterRoleDesired>,
}

/// Role binding list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoleBindingListResponse {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<RoleBindingDesired>,
}

/// Cluster role binding list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClusterRoleBindingListResponse {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<ClusterRoleBindingDesired>,
}

/// DaemonSet list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DaemonSetListResponse {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<DaemonSetDesired>,
}

/// StatefulSet list response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StatefulSetListResponse {
    pub kind: String,
    pub api_version: String,
    pub metadata: ListMeta,
    pub items: Vec<StatefulSetDesired>,
}

/// API server state shared across handlers.
pub struct ApiServerState {
    /// Namespace controller.
    pub namespaces: RwLock<HashMap<String, Namespace>>,
    /// Pods by namespace.
    pub pods: RwLock<HashMap<String, HashMap<String, PodActual>>>,
    /// Services by namespace.
    pub services: RwLock<HashMap<String, HashMap<String, ServiceDesired>>>,
    /// Service actual state (for LoadBalancer IPs, endpoints).
    pub services_actual: RwLock<HashMap<String, HashMap<String, ServiceActual>>>,
    /// Deployments by namespace.
    pub deployments: RwLock<HashMap<String, HashMap<String, DeploymentDesired>>>,
    /// ConfigMaps by namespace.
    pub configmaps: RwLock<HashMap<String, HashMap<String, ConfigMapDesired>>>,
    /// Secrets by namespace.
    pub secrets: RwLock<HashMap<String, HashMap<String, SecretDesired>>>,
    /// Persistent volumes.
    pub persistent_volumes: RwLock<HashMap<String, PersistentVolumeDesired>>,
    /// Persistent volume claims by namespace.
    pub persistent_volume_claims:
        RwLock<HashMap<String, HashMap<String, PersistentVolumeClaimDesired>>>,
    /// Service accounts.
    pub service_accounts: Arc<ServiceAccountController>,
    /// RBAC controller.
    pub rbac: Arc<RbacController>,
    /// Storage classes.
    pub storage_classes: Arc<StorageClassController>,
    /// Volume manager.
    pub volume_manager: Arc<VolumeManager>,
    /// Nodes.
    pub nodes: RwLock<HashMap<String, NodeDesired>>,
    /// Event controller.
    pub events: Arc<EventController>,
    /// DaemonSet controller.
    pub daemonsets: Arc<DaemonController>,
    /// StatefulSet controller.
    pub statefulsets: Arc<StatefulSetController>,
    /// Ingresses by namespace.
    pub ingresses: RwLock<HashMap<String, HashMap<String, IngressDesired>>>,
    /// Job controller.
    pub jobs: Arc<JobController>,
    /// CronJob controller.
    pub cronjobs: Arc<CronJobController>,
    /// ReplicaSets by namespace.
    pub replicasets: RwLock<HashMap<String, HashMap<String, ReplicaSetDesired>>>,
    /// NetworkPolicies by namespace.
    pub networkpolicies: RwLock<HashMap<String, HashMap<String, NetworkPolicyDesired>>>,
    /// PodDisruptionBudgets by namespace.
    pub poddisruptionbudgets: RwLock<HashMap<String, HashMap<String, PodDisruptionBudgetDesired>>>,
    /// IngressClasses (cluster-scoped).
    pub ingressclasses: RwLock<HashMap<String, IngressClassDesired>>,
    /// CSIDrivers (storage.k8s.io/v1).
    pub csi_drivers: RwLock<HashMap<String, CSIDriverDesired>>,
    /// RuntimeClasses (node.k8s.io/v1).
    pub runtime_classes: Arc<RuntimeClassController>,
    /// CSINodes (storage.k8s.io/v1).
    pub csi_nodes: RwLock<HashMap<String, CSINodeDesired>>,
    /// VolumeAttachments (storage.k8s.io/v1).
    pub volume_attachments: RwLock<HashMap<String, VolumeAttachmentDesired>>,
    /// FlowSchemas (flowcontrol.apiserver.k8s.io/v1).
    pub flow_schemas: RwLock<HashMap<String, FlowSchemaDesired>>,
    /// PriorityLevelConfigurations (flowcontrol.apiserver.k8s.io/v1).
    pub priority_level_configurations: RwLock<HashMap<String, PriorityLevelConfigurationDesired>>,
    /// PriorityClasses (scheduling.k8s.io/v1).
    pub priority_classes: RwLock<HashMap<String, PriorityClassDesired>>,
    /// CertificateSigningRequests (certificates.k8s.io/v1).
    pub certificate_signing_requests: RwLock<HashMap<String, CertificateSigningRequestDesired>>,
    /// MutatingWebhookConfigurations (admissionregistration.k8s.io/v1).
    pub mutating_webhook_configurations:
        RwLock<HashMap<String, MutatingWebhookConfigurationDesired>>,
    /// ValidatingWebhookConfigurations (admissionregistration.k8s.io/v1).
    pub validating_webhook_configurations:
        RwLock<HashMap<String, ValidatingWebhookConfigurationDesired>>,
    /// HorizontalPodAutoscalers (autoscaling/v1).
    pub hpas: RwLock<HashMap<String, HashMap<String, HPADesired>>>,
    /// Endpoints (core/v1).
    pub endpoints: RwLock<HashMap<String, HashMap<String, EndpointsDesired>>>,
    /// Leases for leader election (coordination.k8s.io/v1).
    pub leases: RwLock<HashMap<String, HashMap<String, LeaseDesired>>>,
    /// LimitRange controller.
    pub limitranges: Arc<LimitRangeController>,
    /// ResourceQuota controller.
    pub resource_quotas: Arc<ResourceQuotaController>,
    /// Token controller for authentication.
    pub token_controller: Arc<TokenController>,
    /// SQLite state store (K3s-style persistent backend).
    pub sqlite_store: Option<Arc<SqliteStateStore>>,
    /// Resource version counter.
    resource_version: RwLock<u64>,
    /// Watch event broadcast channel.
    watch_tx: broadcast::Sender<ApiWatchEvent>,
}

impl ApiServerState {
    pub fn new(
        service_accounts: Arc<ServiceAccountController>,
        rbac: Arc<RbacController>,
        storage_classes: Arc<StorageClassController>,
        volume_manager: Arc<VolumeManager>,
        events: Arc<EventController>,
        daemonsets: Arc<DaemonController>,
        statefulsets: Arc<StatefulSetController>,
        jobs: Arc<JobController>,
        cronjobs: Arc<CronJobController>,
        limitranges: Arc<LimitRangeController>,
        resource_quotas: Arc<ResourceQuotaController>,
        token_controller: Arc<TokenController>,
        runtime_classes: Arc<RuntimeClassController>,
        sqlite_store: Option<Arc<SqliteStateStore>>,
    ) -> Self {
        let (watch_tx, _) = broadcast::channel(100);
        Self {
            namespaces: RwLock::new(HashMap::new()),
            pods: RwLock::new(HashMap::new()),
            services: RwLock::new(HashMap::new()),
            services_actual: RwLock::new(HashMap::new()),
            deployments: RwLock::new(HashMap::new()),
            configmaps: RwLock::new(HashMap::new()),
            secrets: RwLock::new(HashMap::new()),
            persistent_volumes: RwLock::new(HashMap::new()),
            persistent_volume_claims: RwLock::new(HashMap::new()),
            service_accounts,
            rbac,
            storage_classes,
            volume_manager,
            nodes: RwLock::new(HashMap::new()),
            events,
            daemonsets,
            statefulsets,
            ingresses: RwLock::new(HashMap::new()),
            jobs,
            cronjobs,
            replicasets: RwLock::new(HashMap::new()),
            networkpolicies: RwLock::new(HashMap::new()),
            poddisruptionbudgets: RwLock::new(HashMap::new()),
            ingressclasses: RwLock::new(HashMap::new()),
            csi_drivers: RwLock::new(HashMap::new()),
            runtime_classes,
            csi_nodes: RwLock::new(HashMap::new()),
            volume_attachments: RwLock::new(HashMap::new()),
            flow_schemas: RwLock::new(HashMap::new()),
            priority_level_configurations: RwLock::new(HashMap::new()),
            priority_classes: RwLock::new(HashMap::new()),
            certificate_signing_requests: RwLock::new(HashMap::new()),
            mutating_webhook_configurations: RwLock::new(HashMap::new()),
            validating_webhook_configurations: RwLock::new(HashMap::new()),
            hpas: RwLock::new(HashMap::new()),
            endpoints: RwLock::new(HashMap::new()),
            leases: RwLock::new(HashMap::new()),
            limitranges,
            resource_quotas,
            token_controller,
            sqlite_store,
            resource_version: RwLock::new(1),
            watch_tx,
        }
    }

    /// Get next resource version.
    pub async fn next_resource_version(&self) -> String {
        let mut rv = self.resource_version.write().await;
        *rv += 1;
        rv.to_string()
    }

    /// Get current resource version.
    pub async fn current_resource_version(&self) -> String {
        let rv = self.resource_version.read().await;
        rv.to_string()
    }

    /// Subscribe to watch events.
    pub fn watch_subscribe(&self) -> broadcast::Receiver<ApiWatchEvent> {
        self.watch_tx.subscribe()
    }

    /// Broadcast a watch event.
    pub async fn broadcast_watch(&self, event: ApiWatchEvent) {
        let _ = self.watch_tx.send(event);
    }

    /// Start background task to poll SQLite and broadcast new watch events.
    /// This enables true SSE push - clients receive events as they happen.
    pub fn start_watch_broadcaster(&self) {
        let sqlite_store = match &self.sqlite_store {
            Some(store) => store.clone(),
            None => return,
        };

        let tx = self.watch_tx.clone();
        let last_timestamp: StdRwLock<Option<String>> = StdRwLock::new(None);

        // Spawn background task
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_millis(500));

            loop {
                interval.tick().await;

                let last_ts = last_timestamp.read().unwrap().clone();

                // Poll for new events since last timestamp
                match sqlite_store.watch_since(last_ts.as_deref()).await {
                    Ok(events) => {
                        for event in events {
                            // Update last timestamp
                            *last_timestamp.write().unwrap() = Some(event.timestamp.clone());

                            // Extract kind from object
                            let kind = event
                                .object
                                .get("kind")
                                .and_then(|v| v.as_str())
                                .unwrap_or("")
                                .to_string();

                            // Broadcast event
                            let watch_event = ApiWatchEvent {
                                event_type: match event.event_type {
                                    crate::state::WatchEventType::Added => WatchEventType::Added,
                                    crate::state::WatchEventType::Modified => {
                                        WatchEventType::Modified
                                    }
                                    crate::state::WatchEventType::Deleted => {
                                        WatchEventType::Deleted
                                    }
                                },
                                object: event.object,
                                kind,
                            };
                            let _ = tx.send(watch_event);
                        }
                    }
                    Err(_) => {
                        // Silently continue on error
                    }
                }
            }
        });
    }
}

/// HTTP method for API requests.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum HttpMethod {
    Get,
    Post,
    Put,
    Patch,
    Delete,
}

impl HttpMethod {
    /// Convert from axum Method.
    pub fn from_axum(method: &axum::http::Method) -> Option<Self> {
        match *method {
            axum::http::Method::GET => Some(HttpMethod::Get),
            axum::http::Method::POST => Some(HttpMethod::Post),
            axum::http::Method::PUT => Some(HttpMethod::Put),
            axum::http::Method::PATCH => Some(HttpMethod::Patch),
            axum::http::Method::DELETE => Some(HttpMethod::Delete),
            _ => None,
        }
    }
}

/// API path handler trait.
#[async_trait]
pub trait ApiHandler: Send + Sync {
    /// Handle a request.
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus>;
}

/// Response type.
pub enum ApiResponseOrStatus {
    Response(String),
    Created(String),
    Status(Status),
    SseStream(String),
    NotFound,
    MethodNotAllowed,
    Conflict,
}

/// API server.
pub struct ApiServer {
    /// Configuration.
    config: ApiServerConfig,
    /// State.
    state: Arc<ApiServerState>,
    /// Routes.
    routes: HashMap<String, Box<dyn ApiHandler>>,
}

impl ApiServer {
    /// Create a new API server.
    pub fn new(config: ApiServerConfig, state: Arc<ApiServerState>) -> Self {
        let mut routes = HashMap::new();

        // Register routes
        routes.insert(
            "GET /api/v1/namespaces".to_string(),
            Box::new(NamespaceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{name}".to_string(),
            Box::new(NamespaceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces".to_string(),
            Box::new(NamespaceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{name}".to_string(),
            Box::new(NamespaceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/pods".to_string(),
            Box::new(PodHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/pods".to_string(),
            Box::new(PodHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/pods/{name}".to_string(),
            Box::new(PodHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/pods/{name}".to_string(),
            Box::new(PodHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/pods/{name}".to_string(),
            Box::new(PodHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/pods/{name}/log".to_string(),
            Box::new(PodHandler) as Box<dyn ApiHandler>,
        );
        // Exec API - kubectl exec
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/pods/{name}/exec".to_string(),
            Box::new(ExecHandler) as Box<dyn ApiHandler>,
        );
        // Attach API - kubectl attach
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/pods/{name}/attach".to_string(),
            Box::new(AttachHandler) as Box<dyn ApiHandler>,
        );
        // PortForward API - kubectl port-forward
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/pods/{name}/portforward".to_string(),
            Box::new(PortForwardHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/services".to_string(),
            Box::new(ServiceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/services".to_string(),
            Box::new(ServiceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/services/{name}".to_string(),
            Box::new(ServiceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/services/{name}".to_string(),
            Box::new(ServiceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/services/{name}".to_string(),
            Box::new(ServiceHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/apps/v1/deployments".to_string(),
            Box::new(DeploymentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/deployments".to_string(),
            Box::new(DeploymentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/deployments/{name}".to_string(),
            Box::new(DeploymentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/deployments/{name}".to_string(),
            Box::new(DeploymentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/deployments/{name}".to_string(),
            Box::new(DeploymentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/persistentvolumes".to_string(),
            Box::new(PersistentVolumeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/persistentvolumes".to_string(),
            Box::new(PersistentVolumeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/persistentvolumes/{name}".to_string(),
            Box::new(PersistentVolumeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/persistentvolumes/{name}".to_string(),
            Box::new(PersistentVolumeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/persistentvolumeclaims".to_string(),
            Box::new(PersistentVolumeClaimHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/persistentvolumeclaims".to_string(),
            Box::new(PersistentVolumeClaimHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/persistentvolumeclaims/{name}".to_string(),
            Box::new(PersistentVolumeClaimHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/persistentvolumeclaims/{name}".to_string(),
            Box::new(PersistentVolumeClaimHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/persistentvolumeclaims/{name}".to_string(),
            Box::new(PersistentVolumeClaimHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/serviceaccounts".to_string(),
            Box::new(ServiceAccountHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/serviceaccounts".to_string(),
            Box::new(ServiceAccountHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/serviceaccounts/{name}".to_string(),
            Box::new(ServiceAccountHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/serviceaccounts/{name}".to_string(),
            Box::new(ServiceAccountHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/serviceaccounts/{name}".to_string(),
            Box::new(ServiceAccountHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/storage.a3s.io/v1/storageclasses".to_string(),
            Box::new(StorageClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/configmaps".to_string(),
            Box::new(ConfigMapHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/configmaps".to_string(),
            Box::new(ConfigMapHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/configmaps/{name}".to_string(),
            Box::new(ConfigMapHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/configmaps/{name}".to_string(),
            Box::new(ConfigMapHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/configmaps/{name}".to_string(),
            Box::new(ConfigMapHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/secrets".to_string(),
            Box::new(SecretHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/secrets".to_string(),
            Box::new(SecretHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/secrets/{name}".to_string(),
            Box::new(SecretHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/secrets/{name}".to_string(),
            Box::new(SecretHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/secrets/{name}".to_string(),
            Box::new(SecretHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/nodes".to_string(),
            Box::new(NodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/nodes".to_string(),
            Box::new(NodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/nodes/{name}".to_string(),
            Box::new(NodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/nodes/{name}".to_string(),
            Box::new(NodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/nodes/{name}".to_string(),
            Box::new(NodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/events".to_string(),
            Box::new(EventHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/rbac.authorization.a3s.io/v1/roles".to_string(),
            Box::new(RoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/rbac.authorization.a3s.io/v1/clusterroles".to_string(),
            Box::new(ClusterRoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/rbac.authorization.a3s.io/v1/rolebindings".to_string(),
            Box::new(RoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/rbac.authorization.a3s.io/v1/clusterrolebindings".to_string(),
            Box::new(ClusterRoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/apps/v1/daemonsets".to_string(),
            Box::new(DaemonSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/apps/v1/namespaces/{namespace}/daemonsets".to_string(),
            Box::new(DaemonSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/apps/v1/namespaces/{namespace}/daemonsets/{name}".to_string(),
            Box::new(DaemonSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/apps/v1/namespaces/{namespace}/daemonsets/{name}".to_string(),
            Box::new(DaemonSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/apps/v1/namespaces/{namespace}/daemonsets/{name}".to_string(),
            Box::new(DaemonSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/apps/v1/statefulsets".to_string(),
            Box::new(StatefulSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/apps/v1/namespaces/{namespace}/statefulsets".to_string(),
            Box::new(StatefulSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/apps/v1/namespaces/{namespace}/statefulsets/{name}".to_string(),
            Box::new(StatefulSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/apps/v1/namespaces/{namespace}/statefulsets/{name}".to_string(),
            Box::new(StatefulSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/apps/v1/namespaces/{namespace}/statefulsets/{name}".to_string(),
            Box::new(StatefulSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/storage.a3s.io/v1/storageclasses".to_string(),
            Box::new(StorageClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/storage.a3s.io/v1/storageclasses/{name}".to_string(),
            Box::new(StorageClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/storage.a3s.io/v1/storageclasses/{name}".to_string(),
            Box::new(StorageClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/rbac.authorization.a3s.io/v1/namespaces/{namespace}/roles".to_string(),
            Box::new(RoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/rbac.authorization.a3s.io/v1/namespaces/{namespace}/roles/{name}"
                .to_string(),
            Box::new(RoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/rbac.authorization.a3s.io/v1/namespaces/{namespace}/roles/{name}"
                .to_string(),
            Box::new(RoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/rbac.authorization.a3s.io/v1/clusterroles".to_string(),
            Box::new(ClusterRoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/rbac.authorization.a3s.io/v1/clusterroles/{name}".to_string(),
            Box::new(ClusterRoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/rbac.authorization.a3s.io/v1/clusterroles/{name}".to_string(),
            Box::new(ClusterRoleHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/rbac.authorization.a3s.io/v1/namespaces/{namespace}/rolebindings"
                .to_string(),
            Box::new(RoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/rbac.authorization.a3s.io/v1/namespaces/{namespace}/rolebindings/{name}"
                .to_string(),
            Box::new(RoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/rbac.authorization.a3s.io/v1/namespaces/{namespace}/rolebindings/{name}"
                .to_string(),
            Box::new(RoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/rbac.authorization.a3s.io/v1/clusterrolebindings".to_string(),
            Box::new(ClusterRoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/rbac.authorization.a3s.io/v1/clusterrolebindings/{name}".to_string(),
            Box::new(ClusterRoleBindingHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/rbac.authorization.a3s.io/v1/clusterrolebindings/{name}".to_string(),
            Box::new(ClusterRoleBindingHandler) as Box<dyn ApiHandler>,
        );

        // Ingress routes
        routes.insert(
            "GET /apis/networking.a3s.io/v1/ingresses".to_string(),
            Box::new(IngressHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/networking.a3s.io/v1/namespaces/{namespace}/ingresses/{name}".to_string(),
            Box::new(IngressHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/networking.a3s.io/v1/namespaces/{namespace}/ingresses".to_string(),
            Box::new(IngressHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/networking.a3s.io/v1/namespaces/{namespace}/ingresses/{name}".to_string(),
            Box::new(IngressHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/networking.a3s.io/v1/namespaces/{namespace}/ingresses/{name}".to_string(),
            Box::new(IngressHandler) as Box<dyn ApiHandler>,
        );

        // Job routes (batch/v1)
        routes.insert(
            "GET /apis/batch/v1/jobs".to_string(),
            Box::new(JobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/batch/v1/namespaces/{namespace}/jobs/{name}".to_string(),
            Box::new(JobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/batch/v1/namespaces/{namespace}/jobs".to_string(),
            Box::new(JobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/batch/v1/namespaces/{namespace}/jobs/{name}".to_string(),
            Box::new(JobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/batch/v1/namespaces/{namespace}/jobs/{name}".to_string(),
            Box::new(JobHandler) as Box<dyn ApiHandler>,
        );

        // CronJob routes (batch/v1)
        routes.insert(
            "GET /apis/batch/v1/cronjobs".to_string(),
            Box::new(CronJobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/batch/v1/namespaces/{namespace}/cronjobs/{name}".to_string(),
            Box::new(CronJobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/batch/v1/namespaces/{namespace}/cronjobs".to_string(),
            Box::new(CronJobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/batch/v1/namespaces/{namespace}/cronjobs/{name}".to_string(),
            Box::new(CronJobHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/batch/v1/namespaces/{namespace}/cronjobs/{name}".to_string(),
            Box::new(CronJobHandler) as Box<dyn ApiHandler>,
        );

        // ReplicaSet routes (apps/v1)
        routes.insert(
            "GET /apis/apps/v1/replicasets".to_string(),
            Box::new(ReplicaSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/apps/v1/namespaces/{namespace}/replicasets/{name}".to_string(),
            Box::new(ReplicaSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/apps/v1/namespaces/{namespace}/replicasets".to_string(),
            Box::new(ReplicaSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/apps/v1/namespaces/{namespace}/replicasets/{name}".to_string(),
            Box::new(ReplicaSetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/apps/v1/namespaces/{namespace}/replicasets/{name}".to_string(),
            Box::new(ReplicaSetHandler) as Box<dyn ApiHandler>,
        );

        // NetworkPolicy routes (networking.a3s.io/v1)
        routes.insert(
            "GET /apis/networking.a3s.io/v1/networkpolicies".to_string(),
            Box::new(NetworkPolicyHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/networking.a3s.io/v1/namespaces/{namespace}/networkpolicies".to_string(),
            Box::new(NetworkPolicyHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/networking.a3s.io/v1/namespaces/{namespace}/networkpolicies/{name}"
                .to_string(),
            Box::new(NetworkPolicyHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/networking.a3s.io/v1/namespaces/{namespace}/networkpolicies".to_string(),
            Box::new(NetworkPolicyHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/networking.a3s.io/v1/namespaces/{namespace}/networkpolicies/{name}"
                .to_string(),
            Box::new(NetworkPolicyHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/networking.a3s.io/v1/namespaces/{namespace}/networkpolicies/{name}"
                .to_string(),
            Box::new(NetworkPolicyHandler) as Box<dyn ApiHandler>,
        );

        // PodDisruptionBudget routes (policy/v1beta1)
        routes.insert(
            "GET /apis/policy/v1beta1/namespaces/{namespace}/poddisruptionbudgets".to_string(),
            Box::new(PodDisruptionBudgetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/policy/v1beta1/namespaces/{namespace}/poddisruptionbudgets/{name}"
                .to_string(),
            Box::new(PodDisruptionBudgetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/policy/v1beta1/namespaces/{namespace}/poddisruptionbudgets".to_string(),
            Box::new(PodDisruptionBudgetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/policy/v1beta1/namespaces/{namespace}/poddisruptionbudgets/{name}"
                .to_string(),
            Box::new(PodDisruptionBudgetHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/policy/v1beta1/namespaces/{namespace}/poddisruptionbudgets/{name}"
                .to_string(),
            Box::new(PodDisruptionBudgetHandler) as Box<dyn ApiHandler>,
        );

        // Authentication APIs
        routes.insert(
            "POST /api/v1/tokenreview".to_string(),
            Box::new(TokenReviewHandler) as Box<dyn ApiHandler>,
        );

        // LimitRange routes (v1)
        routes.insert(
            "GET /api/v1/limitranges".to_string(),
            Box::new(LimitRangeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/limitranges".to_string(),
            Box::new(LimitRangeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/limitranges/{name}".to_string(),
            Box::new(LimitRangeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/limitranges".to_string(),
            Box::new(LimitRangeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/limitranges/{name}".to_string(),
            Box::new(LimitRangeHandler) as Box<dyn ApiHandler>,
        );

        // ResourceQuota routes (v1)
        routes.insert(
            "GET /api/v1/resourcequotas".to_string(),
            Box::new(ResourceQuotaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/resourcequotas".to_string(),
            Box::new(ResourceQuotaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/resourcequotas/{name}".to_string(),
            Box::new(ResourceQuotaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/resourcequotas".to_string(),
            Box::new(ResourceQuotaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/resourcequotas/{name}".to_string(),
            Box::new(ResourceQuotaHandler) as Box<dyn ApiHandler>,
        );

        // IngressClass routes (networking.k8s.io/v1)
        routes.insert(
            "GET /apis/networking.k8s.io/v1/ingressclasses".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/networking.k8s.io/v1/ingressclasses/{name}".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/networking.k8s.io/v1/ingressclasses".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/networking.k8s.io/v1/ingressclasses/{name}".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );

        // Lease routes (coordination.k8s.io/v1)
        routes.insert(
            "GET /apis/coordination.k8s.io/v1/leases".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/coordination.k8s.io/v1/namespaces/{namespace}/leases".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/coordination.k8s.io/v1/namespaces/{namespace}/leases/{name}".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/coordination.k8s.io/v1/namespaces/{namespace}/leases".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/coordination.k8s.io/v1/namespaces/{namespace}/leases/{name}".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/coordination.k8s.io/v1/namespaces/{namespace}/leases/{name}".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );

        // CSIDriver routes (storage.k8s.io/v1)
        routes.insert(
            "GET /apis/storage.k8s.io/v1/csidrivers".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/storage.k8s.io/v1/csidrivers".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/storage.k8s.io/v1/csidrivers/{name}".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/storage.k8s.io/v1/csidrivers/{name}".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );

        // RuntimeClass routes (node.k8s.io/v1)
        routes.insert(
            "GET /apis/node.k8s.io/v1/runtimeclasses".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/node.k8s.io/v1/runtimeclasses".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/node.k8s.io/v1/runtimeclasses/{name}".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/node.k8s.io/v1/runtimeclasses/{name}".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );

        // Mirror .k8s.io routes under .a3s.io
        // storage.a3s.io/v1 - CSIDrivers
        routes.insert(
            "GET /apis/storage.a3s.io/v1/csidrivers".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/storage.a3s.io/v1/csidrivers".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/storage.a3s.io/v1/csidrivers/{name}".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/storage.a3s.io/v1/csidrivers/{name}".to_string(),
            Box::new(CSIDriverHandler) as Box<dyn ApiHandler>,
        );

        // networking.a3s.io/v1 - IngressClasses (already exists as networking.k8s.io)
        routes.insert(
            "GET /apis/networking.a3s.io/v1/ingressclasses".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/networking.a3s.io/v1/ingressclasses".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/networking.a3s.io/v1/ingressclasses/{name}".to_string(),
            Box::new(IngressClassHandler) as Box<dyn ApiHandler>,
        );

        // node.a3s.io/v1 - RuntimeClasses
        routes.insert(
            "GET /apis/node.a3s.io/v1/runtimeclasses".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/node.a3s.io/v1/runtimeclasses".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/node.a3s.io/v1/runtimeclasses/{name}".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/node.a3s.io/v1/runtimeclasses/{name}".to_string(),
            Box::new(RuntimeClassHandler) as Box<dyn ApiHandler>,
        );

        // coordination.a3s.io/v1 - Leases
        routes.insert(
            "GET /apis/coordination.a3s.io/v1/leases".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/coordination.a3s.io/v1/namespaces/{namespace}/leases".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/coordination.a3s.io/v1/namespaces/{namespace}/leases/{name}".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/coordination.a3s.io/v1/namespaces/{namespace}/leases".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/coordination.a3s.io/v1/namespaces/{namespace}/leases/{name}".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/coordination.a3s.io/v1/namespaces/{namespace}/leases/{name}".to_string(),
            Box::new(LeaseHandler) as Box<dyn ApiHandler>,
        );

        // CSINode routes (storage.k8s.io/v1)
        routes.insert(
            "GET /apis/storage.k8s.io/v1/csinodes".to_string(),
            Box::new(CSINodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/storage.k8s.io/v1/csinodes".to_string(),
            Box::new(CSINodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/storage.k8s.io/v1/csinodes/{name}".to_string(),
            Box::new(CSINodeHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/storage.k8s.io/v1/csinodes/{name}".to_string(),
            Box::new(CSINodeHandler) as Box<dyn ApiHandler>,
        );

        // VolumeAttachment routes (storage.k8s.io/v1)
        routes.insert(
            "GET /apis/storage.k8s.io/v1/volumeattachments".to_string(),
            Box::new(VolumeAttachmentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/storage.k8s.io/v1/volumeattachments".to_string(),
            Box::new(VolumeAttachmentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/storage.k8s.io/v1/volumeattachments/{name}".to_string(),
            Box::new(VolumeAttachmentHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/storage.k8s.io/v1/volumeattachments/{name}".to_string(),
            Box::new(VolumeAttachmentHandler) as Box<dyn ApiHandler>,
        );

        // FlowSchema routes (flowcontrol.apiserver.k8s.io/v1)
        routes.insert(
            "GET /apis/flowcontrol.apiserver.k8s.io/v1/flowchemas".to_string(),
            Box::new(FlowSchemaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/flowcontrol.apiserver.k8s.io/v1/flowchemas".to_string(),
            Box::new(FlowSchemaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/flowcontrol.apiserver.k8s.io/v1/flowchemas/{name}".to_string(),
            Box::new(FlowSchemaHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/flowcontrol.apiserver.k8s.io/v1/flowchemas/{name}".to_string(),
            Box::new(FlowSchemaHandler) as Box<dyn ApiHandler>,
        );

        // PriorityLevelConfiguration routes (flowcontrol.apiserver.k8s.io/v1)
        routes.insert(
            "GET /apis/flowcontrol.apiserver.k8s.io/v1/prioritylevelconfigurations".to_string(),
            Box::new(PriorityLevelHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/flowcontrol.apiserver.k8s.io/v1/prioritylevelconfigurations".to_string(),
            Box::new(PriorityLevelHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/flowcontrol.apiserver.k8s.io/v1/prioritylevelconfigurations/{name}"
                .to_string(),
            Box::new(PriorityLevelHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/flowcontrol.apiserver.k8s.io/v1/prioritylevelconfigurations/{name}"
                .to_string(),
            Box::new(PriorityLevelHandler) as Box<dyn ApiHandler>,
        );

        // PriorityClass routes (scheduling.k8s.io/v1)
        routes.insert(
            "GET /apis/scheduling.k8s.io/v1/priorityclasses".to_string(),
            Box::new(PriorityClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/scheduling.k8s.io/v1/priorityclasses".to_string(),
            Box::new(PriorityClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/scheduling.k8s.io/v1/priorityclasses/{name}".to_string(),
            Box::new(PriorityClassHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/scheduling.k8s.io/v1/priorityclasses/{name}".to_string(),
            Box::new(PriorityClassHandler) as Box<dyn ApiHandler>,
        );

        // CertificateSigningRequest routes (certificates.k8s.io/v1)
        routes.insert(
            "GET /apis/certificates.k8s.io/v1/certificatesigningrequests".to_string(),
            Box::new(CSRHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/certificates.k8s.io/v1/certificatesigningrequests".to_string(),
            Box::new(CSRHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/certificates.k8s.io/v1/certificatesigningrequests/{name}".to_string(),
            Box::new(CSRHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/certificates.k8s.io/v1/certificatesigningrequests/{name}".to_string(),
            Box::new(CSRHandler) as Box<dyn ApiHandler>,
        );

        // MutatingWebhookConfiguration routes (admissionregistration.k8s.io/v1)
        routes.insert(
            "GET /apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations".to_string(),
            Box::new(MutatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations".to_string(),
            Box::new(MutatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{name}"
                .to_string(),
            Box::new(MutatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/admissionregistration.k8s.io/v1/mutatingwebhookconfigurations/{name}"
                .to_string(),
            Box::new(MutatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );

        // ValidatingWebhookConfiguration routes (admissionregistration.k8s.io/v1)
        routes.insert(
            "GET /apis/admissionregistration.k8s.io/v1/validatingwebhookconfigurations".to_string(),
            Box::new(ValidatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/admissionregistration.k8s.io/v1/validatingwebhookconfigurations"
                .to_string(),
            Box::new(ValidatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/admissionregistration.k8s.io/v1/validatingwebhookconfigurations/{name}"
                .to_string(),
            Box::new(ValidatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/admissionregistration.k8s.io/v1/validatingwebhookconfigurations/{name}"
                .to_string(),
            Box::new(ValidatingWebhookConfigHandler) as Box<dyn ApiHandler>,
        );

        // HorizontalPodAutoscaler routes (autoscaling/v1)
        routes.insert(
            "GET /apis/autoscaling/v1/horizontalpodautoscalers".to_string(),
            Box::new(HPAHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/autoscaling/v1/namespaces/{namespace}/horizontalpodautoscalers".to_string(),
            Box::new(HPAHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /apis/autoscaling/v1/namespaces/{namespace}/horizontalpodautoscalers/{name}"
                .to_string(),
            Box::new(HPAHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/autoscaling/v1/namespaces/{namespace}/horizontalpodautoscalers".to_string(),
            Box::new(HPAHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /apis/autoscaling/v1/namespaces/{namespace}/horizontalpodautoscalers/{name}"
                .to_string(),
            Box::new(HPAHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /apis/autoscaling/v1/namespaces/{namespace}/horizontalpodautoscalers/{name}"
                .to_string(),
            Box::new(HPAHandler) as Box<dyn ApiHandler>,
        );

        // SelfSubjectAccessReview routes
        routes.insert(
            "POST /api/v1/selfsubjectaccessreviews".to_string(),
            Box::new(SelfSubjectAccessReviewHandler) as Box<dyn ApiHandler>,
        );

        // SubjectAccessReview routes
        routes.insert(
            "POST /api/v1/subjectaccessreviews".to_string(),
            Box::new(SubjectAccessReviewHandler) as Box<dyn ApiHandler>,
        );

        // Authorization APIs
        routes.insert(
            "POST /apis/authorization.a3s.io/v1/subjectaccessreviews".to_string(),
            Box::new(SubjectAccessReviewHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /apis/authorization.a3s.io/v1/selfsubjectaccessreviews".to_string(),
            Box::new(SelfSubjectAccessReviewHandler) as Box<dyn ApiHandler>,
        );

        // Component Status API
        routes.insert(
            "GET /api/v1/componentstatuses".to_string(),
            Box::new(ComponentStatusHandler) as Box<dyn ApiHandler>,
        );

        // Endpoints API (core/v1)
        routes.insert(
            "GET /api/v1/namespaces/{namespace}/endpoints".to_string(),
            Box::new(EndpointsHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1/endpoints".to_string(),
            Box::new(EndpointsHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "POST /api/v1/namespaces/{namespace}/endpoints".to_string(),
            Box::new(EndpointsHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "DELETE /api/v1/namespaces/{namespace}/endpoints/{name}".to_string(),
            Box::new(EndpointsHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "PATCH /api/v1/namespaces/{namespace}/endpoints/{name}".to_string(),
            Box::new(EndpointsHandler) as Box<dyn ApiHandler>,
        );

        // EndpointSlice API
        routes.insert(
            "GET /apis/discovery.a3s.io/v1/namespaces/{namespace}/endpointslices".to_string(),
            Box::new(EndpointSliceHandler) as Box<dyn ApiHandler>,
        );

        // Version API
        routes.insert(
            "GET /version".to_string(),
            Box::new(VersionHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api".to_string(),
            Box::new(VersionHandler) as Box<dyn ApiHandler>,
        );
        routes.insert(
            "GET /api/v1".to_string(),
            Box::new(VersionHandler) as Box<dyn ApiHandler>,
        );

        Self {
            config,
            state,
            routes,
        }
    }

    /// Get listen address.
    pub fn listen_addr(&self) -> SocketAddr {
        format!("{}:{}", self.config.listen_addr, self.config.port)
            .parse()
            .unwrap_or_else(|_| "0.0.0.0:6443".parse().unwrap())
    }

    /// Route a request.
    pub async fn route(
        &self,
        method: HttpMethod,
        path: &str,
        query: HashMap<String, String>,
        body: Option<String>,
    ) -> ApiResponseOrStatus {
        // Find matching route
        let method_str = match method {
            HttpMethod::Get => "GET",
            HttpMethod::Post => "POST",
            HttpMethod::Put => "PUT",
            HttpMethod::Patch => "PATCH",
            HttpMethod::Delete => "DELETE",
        };

        let route_key = format!("{} {}", method_str, path);

        // Try exact match first
        if let Some(handler) = self.routes.get(&route_key) {
            return handler
                .handle(&self.state, method, path, &query, body.as_deref())
                .await
                .unwrap_or_else(|_| ApiResponseOrStatus::NotFound);
        }

        // Try pattern matching for namespaced resources
        // GET /api/v1/namespaces/{name}/pods
        // GET /api/v1/namespaces/{namespace}/services/{name}
        for (pattern, handler) in &self.routes {
            if pattern.starts_with(&format!("{} /", method_str)) {
                if let Ok(params) = Self::match_pattern(pattern, path) {
                    return handler
                        .handle(&self.state, method, path, &params, body.as_deref())
                        .await
                        .unwrap_or_else(|_| ApiResponseOrStatus::NotFound);
                }
            }
        }

        // Check if it's a valid path prefix (for list operations)
        if path.starts_with("/api/v1/namespaces") && method == HttpMethod::Get {
            // Handle namespaced resources
            if let Some(handler) = self.routes.get("GET /api/v1/pods") {
                return handler
                    .handle(&self.state, method, path, &query, body.as_deref())
                    .await
                    .unwrap_or_else(|_| ApiResponseOrStatus::NotFound);
            }
        }

        ApiResponseOrStatus::NotFound
    }

    /// Match a path pattern against an actual path.
    fn match_pattern(pattern: &str, path: &str) -> Result<HashMap<String, String>> {
        let pattern_parts: Vec<&str> = pattern.split('/').collect();
        let path_parts: Vec<&str> = path.split('/').collect();

        if pattern_parts.len() != path_parts.len() {
            return Err(A3sError::Project("pattern mismatch".to_string()));
        }

        let mut params = HashMap::new();
        for (p, actual) in pattern_parts.iter().zip(path_parts.iter()) {
            if p.starts_with('{') && p.ends_with('}') {
                let key = p.trim_start_matches('{').trim_end_matches('}');
                params.insert(key.to_string(), actual.to_string());
            } else if p != actual {
                return Err(A3sError::Project("pattern mismatch".to_string()));
            }
        }

        Ok(params)
    }
}

// Handler implementations

struct NamespaceHandler;

#[async_trait]
impl ApiHandler for NamespaceHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                // Create namespace
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                // Check if already exists
                {
                    let namespaces = state.namespaces.read().await;
                    if namespaces.contains_key(name) {
                        return Ok(ApiResponseOrStatus::Conflict);
                    }
                }

                // Create namespace in SQLite if available
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let namespace_value = serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Namespace",
                        "metadata": {
                            "name": name,
                            "labels": request["metadata"]["labels"].as_object().map(|m| {
                                m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect::<HashMap<String, String>>()
                            }).unwrap_or_default(),
                            "creationTimestamp": chrono::Utc::now().to_rfc3339(),
                        },
                        "spec": {},
                        "status": { "phase": "Active" }
                    });
                    store
                        .upsert("Namespace", "", name, &namespace_value)
                        .await?
                } else {
                    state.next_resource_version().await
                };

                // Create in-memory namespace
                let ns = Namespace::new(name.to_string());
                {
                    let mut namespaces = state.namespaces.write().await;
                    namespaces.insert(name.to_string(), ns);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Namespace",
                        "metadata": {
                            "name": name,
                            "labels": request["metadata"]["labels"].as_object().map(|m| {
                                m.iter().map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string())).collect::<HashMap<String, String>>()
                            }).unwrap_or_default(),
                            "creationTimestamp": chrono::Utc::now().to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                        "spec": {},
                        "status": { "phase": "Active" }
                    }),
                    kind: "Namespace".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = ApiNamespace {
                    api_version: "v1".to_string(),
                    kind: "Namespace".to_string(),
                    metadata: NamespaceMeta {
                        name: name.to_string(),
                        labels: request["metadata"]["labels"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect::<HashMap<String, String>>()
                            })
                            .unwrap_or_default(),
                        resource_version: resource_version.clone(),
                        creation_timestamp: chrono::Utc::now().to_rfc3339(),
                    },
                };

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if this is a request for a specific namespace
                if let Some(name) = query.get("name") {
                    let namespaces = state.namespaces.read().await;
                    if let Some(ns) = namespaces.get(name) {
                        let response = ApiNamespace {
                            api_version: "v1".to_string(),
                            kind: "Namespace".to_string(),
                            metadata: NamespaceMeta {
                                name: ns.meta.name.clone(),
                                labels: ns.meta.labels.clone(),
                                resource_version: resource_version.clone(),
                                creation_timestamp: ns.meta.created_at.to_rfc3339(),
                            },
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Parse pagination params
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();

                // Return list
                let namespaces = state.namespaces.read().await;
                let items: Vec<ApiNamespace> = namespaces
                    .values()
                    .map(|ns| ApiNamespace {
                        api_version: "v1".to_string(),
                        kind: "Namespace".to_string(),
                        metadata: NamespaceMeta {
                            name: ns.meta.name.clone(),
                            labels: ns.meta.labels.clone(),
                            resource_version: resource_version.clone(),
                            creation_timestamp: ns.meta.created_at.to_rfc3339(),
                        },
                    })
                    .collect();

                // Apply pagination
                let total_count = items.len();
                let paginated_items: Vec<ApiNamespace> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "NamespaceList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Delete namespace
                let name = query
                    .get("name")
                    .cloned()
                    .ok_or_else(|| A3sError::Project("namespace name required".into()))?;

                // Remove from in-memory
                let mut namespaces = state.namespaces.write().await;
                let existed = namespaces.remove(&name).is_some();

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Delete from SQLite if available
                if let Some(ref store) = state.sqlite_store {
                    let _ = store.delete("Namespace", "", &name).await;
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Namespace",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "Namespace".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                        "message": "Namespace deleted",
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct PodHandler;

#[async_trait]
impl ApiHandler for PodHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;
                let deployment = request["metadata"]["labels"]
                    .as_object()
                    .and_then(|l| l.get("app"))
                    .and_then(|v| v.as_str())
                    .unwrap_or("")
                    .to_string();

                // Check if already exists
                {
                    let pods = state.pods.read().await;
                    if let Some(ns_pods) = pods.get(namespace) {
                        if ns_pods.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                let now = chrono::Utc::now();
                let pod = crate::state::PodActual {
                    id: name.to_string(),
                    deployment,
                    namespace: namespace.to_string(),
                    status: crate::state::PodStatus::Pending,
                    health: crate::state::HealthStatus::Unknown,
                    ip: None,
                    version: request["spec"]["containers"]
                        .as_array()
                        .and_then(|c| c.first())
                        .and_then(|img| img["image"].as_str())
                        .unwrap_or("")
                        .to_string(),
                    socket_path: None,
                    node_name: request["spec"]["nodeName"].as_str().map(String::from),
                    created_at: now,
                    last_health_check: None,
                    consecutive_failures: 0,
                    ready: false,
                };

                // Store in memory
                {
                    let mut pods = state.pods.write().await;
                    pods.entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    pods.get_mut(namespace)
                        .unwrap()
                        .insert(name.to_string(), pod.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Pod",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "status": {
                            "phase": "Pending",
                        },
                    }),
                    kind: "Pod".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Pod",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "status": {
                        "phase": "Pending",
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut pods = state.pods.write().await;
                let existed = pods
                    .get_mut(namespace)
                    .map(|ns_pods| ns_pods.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Pod",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Pod".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for pod log request
                if _path.ends_with("/log") {
                    let namespace = query
                        .get("namespace")
                        .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                    let name = query
                        .get("name")
                        .ok_or_else(|| A3sError::Project("name required".into()))?;

                    // Parse tailLines parameter (K8s-style)
                    let tail_lines = query.get("tailLines").and_then(|v| v.parse::<usize>().ok());

                    // Get log content from sandbox
                    let log_content = self.get_pod_log(state, namespace, name, tail_lines).await?;

                    return Ok(ApiResponseOrStatus::Response(log_content));
                }

                // Check for specific pod request via path params (namespace and name)
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    let pods = state.pods.read().await;
                    if let Some(ns_pods) = pods.get(namespace) {
                        if let Some(pod) = ns_pods.get(name) {
                            let response = ListResponse {
                                kind: "PodList".to_string(),
                                api_version: "v1".to_string(),
                                metadata: ListMeta {
                                    resource_version,
                                    continue_token: None,
                                },
                                items: vec![pod.clone()],
                            };
                            return Ok(ApiResponseOrStatus::Response(
                                serde_json::to_string(&response).unwrap(),
                            ));
                        }
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Parse field selector (K8s compatibility)
                let field_selector = query
                    .get("fieldSelector")
                    .map(|s| FieldSelector::parse(s))
                    .unwrap_or_default();

                // Parse label selector
                let label_selector = query
                    .get("labelSelector")
                    .map(|s| FieldLabelSelector::parse(s))
                    .unwrap_or_default();

                // Parse pagination params
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();

                let pods = state.pods.read().await;

                // First collect all pods with their namespace info
                let all_pods: Vec<(String, PodActual)> = pods
                    .iter()
                    .flat_map(|(ns, ns_pods)| {
                        ns_pods
                            .values()
                            .map(|pod| (ns.clone(), pod.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();

                // Then filter by selectors
                let items: Vec<PodActual> = all_pods
                    .into_iter()
                    .filter(|(ns, pod)| {
                        let mut fields = HashMap::new();
                        fields.insert("metadata.namespace".to_string(), ns.clone());
                        fields.insert("metadata.name".to_string(), pod.id.clone());
                        fields.insert("status.phase".to_string(), format!("{:?}", pod.status));
                        let pod_labels = HashMap::new();
                        field_selector.matches(&fields) && label_selector.matches(&pod_labels)
                    })
                    .map(|(_, pod)| pod)
                    .collect();

                // Apply pagination
                let total_count = items.len();
                let paginated_items: Vec<PodActual> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "PodList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Patch => {
                // PATCH Pod (JSON Merge Patch)
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing pod
                let pods = state.pods.read().await;
                let existing = pods
                    .get(namespace)
                    .and_then(|ns_pods| ns_pods.get(name))
                    .cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut pod = existing.unwrap();
                drop(pods);

                // Apply patch
                if let Some(status) = patch["status"].as_str() {
                    pod.status = match status {
                        "Running" => crate::state::PodStatus::Running,
                        "Pending" => crate::state::PodStatus::Pending,
                        "Creating" => crate::state::PodStatus::Creating,
                        "Terminating" => crate::state::PodStatus::Terminating,
                        "Terminated" => crate::state::PodStatus::Terminated,
                        "Failed" => crate::state::PodStatus::Failed,
                        _ => pod.status,
                    };
                }
                if let Some(health) = patch["health"].as_str() {
                    pod.health = match health {
                        "Healthy" => crate::state::HealthStatus::Healthy,
                        "Unhealthy" => crate::state::HealthStatus::Unhealthy,
                        _ => pod.health,
                    };
                }
                if let Some(phase) = patch["phase"].as_str() {
                    pod.status = match phase {
                        "Running" => crate::state::PodStatus::Running,
                        "Pending" => crate::state::PodStatus::Pending,
                        "Creating" => crate::state::PodStatus::Creating,
                        "Terminating" => crate::state::PodStatus::Terminating,
                        "Terminated" => crate::state::PodStatus::Terminated,
                        "Failed" => crate::state::PodStatus::Failed,
                        _ => pod.status,
                    };
                }

                // Update in memory
                {
                    let mut pods = state.pods.write().await;
                    if let Some(ns_pods) = pods.get_mut(namespace) {
                        if let Some(existing_pod) = ns_pods.get_mut(name) {
                            *existing_pod = pod.clone();
                        }
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Pod",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                        "status": {
                            "phase": format!("{:?}", pod.status).to_lowercase(),
                        },
                    }),
                    kind: "Pod".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Pod",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                    "status": {
                        "phase": format!("{:?}", pod.status).to_lowercase(),
                    },
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

impl PodHandler {
    /// Get pod log content from sandbox.
    async fn get_pod_log(
        &self,
        state: &ApiServerState,
        namespace: &str,
        name: &str,
        tail_lines: Option<usize>,
    ) -> Result<String> {
        // Find the pod to get its sandbox ID
        let pods = state.pods.read().await;
        let pod = pods
            .get(namespace)
            .and_then(|ns_pods| ns_pods.get(name))
            .ok_or_else(|| {
                A3sError::Project(format!(
                    "pod '{}' not found in namespace '{}'",
                    name, namespace
                ))
            })?;

        let sandbox_id = &pod.id;

        // Construct log path
        let log_path = dirs::home_dir()
            .map(|h| {
                h.join(".a3s")
                    .join("boxes")
                    .join(sandbox_id)
                    .join("logs")
                    .join("console.log")
            })
            .unwrap_or_else(|| {
                std::path::PathBuf::from(format!("~/.a3s/boxes/{}/logs/console.log", sandbox_id))
            });

        // Read log content
        let content = tokio::fs::read_to_string(&log_path).await.map_err(|e| {
            A3sError::Project(format!("failed to read log for pod '{}': {}", name, e))
        })?;

        // Apply tail lines if specified
        let result = if let Some(lines) = tail_lines {
            content
                .lines()
                .rev()
                .take(lines)
                .collect::<Vec<_>>()
                .into_iter()
                .rev()
                .collect::<Vec<_>>()
                .join("\n")
        } else {
            content
        };

        Ok(result)
    }
}

struct ServiceHandler;

#[async_trait]
impl ApiHandler for ServiceHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                // Create Service
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let services = state.services.read().await;
                    if let Some(ns_svcs) = services.get(namespace) {
                        if ns_svcs.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                // Parse service ports
                let ports: Vec<ServicePort> = request["spec"]["ports"]
                    .as_array()
                    .map(|ports| {
                        ports
                            .iter()
                            .map(|p| ServicePort {
                                name: p["name"].as_str().unwrap_or("").to_string(),
                                port: p["port"].as_i64().unwrap_or(80) as u16,
                                target_port: p["targetPort"].as_i64().unwrap_or(80) as u16,
                                protocol: p["protocol"].as_str().unwrap_or("TCP").to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse selector
                let selector: HashMap<String, String> = request["spec"]["selector"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse labels
                let labels: HashMap<String, String> = request["metadata"]["labels"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();

                // Parse service type from spec.type (default to ClusterIP)
                let service_type = request["spec"]["type"]
                    .as_str()
                    .map(|t| match t {
                        "LoadBalancer" => ServiceType::LoadBalancer,
                        "NodePort" => ServiceType::NodePort,
                        _ => ServiceType::ClusterIP,
                    })
                    .unwrap_or(ServiceType::ClusterIP);

                let svc = ServiceDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    service_type,
                    ports,
                    selector,
                    labels,
                    created_at: now,
                };

                // Get resource version from SQLite or memory
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let svc_value = serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Service",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": {
                            "ports": svc.ports,
                            "selector": svc.selector,
                        },
                    });
                    store.upsert("Service", namespace, name, &svc_value).await?
                } else {
                    state.next_resource_version().await
                };

                // Store in memory
                {
                    let mut services = state.services.write().await;
                    services
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_svcs = services.get_mut(namespace).unwrap();
                    ns_svcs.insert(name.to_string(), svc.clone());
                }

                // Create ServiceActual for LoadBalancer services
                if service_type == ServiceType::LoadBalancer {
                    let mut services_actual = state.services_actual.write().await;
                    services_actual
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_actual = services_actual.get_mut(namespace).unwrap();
                    ns_actual.insert(
                        name.to_string(),
                        ServiceActual {
                            name: name.to_string(),
                            namespace: namespace.to_string(),
                            endpoints: vec![],
                            load_balancer_ip: None,
                        },
                    );
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Service",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                        "spec": {
                            "ports": svc.ports,
                            "selector": svc.selector,
                        },
                    }),
                    kind: "Service".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Service",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                        "resourceVersion": resource_version,
                    },
                    "spec": {
                        "type": match service_type {
                            ServiceType::LoadBalancer => "LoadBalancer",
                            ServiceType::NodePort => "NodePort",
                            _ => "ClusterIP",
                        },
                        "ports": svc.ports,
                        "selector": svc.selector,
                    },
                    "status": if service_type == ServiceType::LoadBalancer {
                        serde_json::json!({
                            "loadBalancer": {
                                "ingress": [{
                                    "ip": "10.0.0.1"
                                }]
                            }
                        })
                    } else {
                        serde_json::json!({})
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Parse field selector (K8s compatibility)
                let field_selector = query
                    .get("fieldSelector")
                    .map(|s| FieldSelector::parse(s))
                    .unwrap_or_default();

                // Parse label selector
                let label_selector = query
                    .get("labelSelector")
                    .map(|s| FieldLabelSelector::parse(s))
                    .unwrap_or_default();

                // Parse pagination params
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();

                let services = state.services.read().await;

                // First collect all services with their namespace
                let all_services: Vec<(String, ServiceDesired)> = services
                    .iter()
                    .flat_map(|(ns, ns_services)| {
                        ns_services
                            .values()
                            .map(|svc| (ns.clone(), svc.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();

                // Then filter by selectors
                let items: Vec<ServiceDesired> = all_services
                    .into_iter()
                    .filter(|(ns, svc)| {
                        let mut fields = HashMap::new();
                        fields.insert("metadata.namespace".to_string(), ns.clone());
                        fields.insert("metadata.name".to_string(), svc.name.clone());
                        field_selector.matches(&fields) && label_selector.matches(&svc.labels)
                    })
                    .map(|(_, svc)| svc)
                    .collect();

                // Apply pagination
                let total_count = items.len();
                let paginated_items: Vec<ServiceDesired> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "ServiceList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Delete Service
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut services = state.services.write().await;
                let existed = services
                    .get_mut(namespace)
                    .map(|ns_svcs| ns_svcs.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Also remove from services_actual
                let mut services_actual = state.services_actual.write().await;
                if let Some(ns_actual) = services_actual.get_mut(namespace) {
                    ns_actual.remove(name);
                }

                // Delete from SQLite
                if let Some(ref store) = state.sqlite_store {
                    let _ = store.delete("Service", namespace, name).await;
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Service",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Service".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                        "message": "Service deleted",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                // PATCH Service (JSON Merge Patch)
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing service
                let services = state.services.read().await;
                let existing = services
                    .get(namespace)
                    .and_then(|ns_svcs| ns_svcs.get(name))
                    .cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut svc = existing.unwrap();
                drop(services);

                // Apply patch
                if let Some(ports) = patch["spec"]["ports"].as_array() {
                    svc.ports = ports
                        .iter()
                        .map(|p| ServicePort {
                            name: p["name"].as_str().unwrap_or("").to_string(),
                            port: p["port"].as_i64().unwrap_or(80) as u16,
                            target_port: p["targetPort"].as_i64().unwrap_or(80) as u16,
                            protocol: p["protocol"].as_str().unwrap_or("TCP").to_string(),
                        })
                        .collect();
                }
                if let Some(selector_obj) = patch["spec"]["selector"].as_object() {
                    svc.selector = selector_obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }
                if let Some(labels_obj) = patch["metadata"]["labels"].as_object() {
                    svc.labels = labels_obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }

                // Update in memory
                {
                    let mut services = state.services.write().await;
                    if let Some(ns_svcs) = services.get_mut(namespace) {
                        if let Some(existing_svc) = ns_svcs.get_mut(name) {
                            *existing_svc = svc.clone();
                        }
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Service",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                        "spec": {
                            "ports": svc.ports,
                            "selector": svc.selector,
                        },
                    }),
                    kind: "Service".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Service",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                    "spec": {
                        "ports": svc.ports,
                        "selector": svc.selector,
                    },
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct DeploymentHandler;

#[async_trait]
impl ApiHandler for DeploymentHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                // Create Deployment
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let deployments = state.deployments.read().await;
                    if let Some(ns_deps) = deployments.get(namespace) {
                        if ns_deps.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                // Parse deployment spec
                let image = request["spec"]["image"].as_str().unwrap_or("").to_string();
                let replicas = request["spec"]["replicas"].as_i64().unwrap_or(1) as i32;

                // Parse ports
                let ports: Vec<PortMapping> = request["spec"]["ports"]
                    .as_array()
                    .map(|ports| {
                        ports
                            .iter()
                            .map(|p| PortMapping {
                                name: p["name"].as_str().unwrap_or("").to_string(),
                                container_port: p["containerPort"].as_i64().unwrap_or(80) as u16,
                                host_port: p["hostPort"].as_i64().unwrap_or(0) as u16,
                                protocol: p["protocol"].as_str().unwrap_or("TCP").to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse env vars
                let env: HashMap<String, String> = request["spec"]["env"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse labels
                let labels: HashMap<String, String> = request["metadata"]["labels"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let deploy = DeploymentDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    image,
                    replicas,
                    env,
                    ports,
                    version: "v1".to_string(),
                    strategy: UpdateStrategy::default(),
                    resources: Default::default(),
                    health_check: Default::default(),
                    node_selector: Default::default(),
                    tolerations: Default::default(),
                    labels,
                    created_at: now,
                    updated_at: now,
                };

                // Get resource version from SQLite or memory
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let deploy_value = serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "Deployment",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": {
                            "replicas": deploy.replicas,
                            "image": deploy.image,
                            "ports": deploy.ports,
                        },
                    });
                    store
                        .upsert("Deployment", namespace, name, &deploy_value)
                        .await?
                } else {
                    state.next_resource_version().await
                };

                // Store in memory
                {
                    let mut deployments = state.deployments.write().await;
                    deployments
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_deps = deployments.get_mut(namespace).unwrap();
                    ns_deps.insert(name.to_string(), deploy.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "Deployment",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                        "spec": {
                            "replicas": deploy.replicas,
                            "image": deploy.image,
                        },
                    }),
                    kind: "Deployment".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "apps/v1",
                    "kind": "Deployment",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                        "resourceVersion": resource_version,
                    },
                    "spec": {
                        "replicas": deploy.replicas,
                        "image": deploy.image,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Parse field selector (K8s compatibility)
                let field_selector = query
                    .get("fieldSelector")
                    .map(|s| FieldSelector::parse(s))
                    .unwrap_or_default();

                // Parse label selector
                let label_selector = query
                    .get("labelSelector")
                    .map(|s| FieldLabelSelector::parse(s))
                    .unwrap_or_default();

                // Parse pagination params
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();

                let deployments = state.deployments.read().await;

                // First collect all deployments with their namespace
                let all_deployments: Vec<(String, DeploymentDesired)> = deployments
                    .iter()
                    .flat_map(|(ns, ns_deployments)| {
                        ns_deployments
                            .values()
                            .map(|d| (ns.clone(), d.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();

                // Then filter by selectors
                let items: Vec<DeploymentDesired> = all_deployments
                    .into_iter()
                    .filter(|(ns, deploy)| {
                        let mut fields = HashMap::new();
                        fields.insert("metadata.namespace".to_string(), ns.clone());
                        fields.insert("metadata.name".to_string(), deploy.name.clone());
                        field_selector.matches(&fields) && label_selector.matches(&deploy.labels)
                    })
                    .map(|(_, deploy)| deploy)
                    .collect();

                // Apply pagination
                let total_count = items.len();
                let paginated_items: Vec<DeploymentDesired> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "DeploymentList".to_string(),
                    api_version: "apps/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Delete Deployment
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut deployments = state.deployments.write().await;
                let existed = deployments
                    .get_mut(namespace)
                    .map(|ns_deps| ns_deps.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Delete from SQLite
                if let Some(ref store) = state.sqlite_store {
                    let _ = store.delete("Deployment", namespace, name).await;
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "Deployment",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Deployment".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                        "message": "Deployment deleted",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                // PATCH Deployment (JSON Merge Patch)
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing deployment
                let deployments = state.deployments.read().await;
                let existing = deployments
                    .get(namespace)
                    .and_then(|ns_deps| ns_deps.get(name))
                    .cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut deploy = existing.unwrap();
                drop(deployments);

                // Apply patch (merge patch semantics)
                let now = chrono::Utc::now();

                if let Some(replicas) = patch["spec"]["replicas"].as_i64() {
                    deploy.replicas = replicas as i32;
                }
                if let Some(image) = patch["spec"]["image"].as_str() {
                    deploy.image = image.to_string();
                }
                if let Some(ports) = patch["spec"]["ports"].as_array() {
                    deploy.ports = ports
                        .iter()
                        .map(|p| PortMapping {
                            name: p["name"].as_str().unwrap_or("").to_string(),
                            container_port: p["containerPort"].as_i64().unwrap_or(80) as u16,
                            host_port: p["hostPort"].as_i64().unwrap_or(0) as u16,
                            protocol: p["protocol"].as_str().unwrap_or("TCP").to_string(),
                        })
                        .collect();
                }
                if let Some(env_obj) = patch["spec"]["env"].as_object() {
                    deploy.env = env_obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }
                if let Some(labels_obj) = patch["metadata"]["labels"].as_object() {
                    deploy.labels = labels_obj
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }
                deploy.updated_at = now;

                // Update in memory
                {
                    let mut deployments = state.deployments.write().await;
                    if let Some(ns_deps) = deployments.get_mut(namespace) {
                        if let Some(existing_deploy) = ns_deps.get_mut(name) {
                            *existing_deploy = deploy.clone();
                        }
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "Deployment",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "resourceVersion": state.current_resource_version().await,
                        },
                        "spec": {
                            "replicas": deploy.replicas,
                            "image": deploy.image,
                        },
                    }),
                    kind: "Deployment".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "apps/v1",
                    "kind": "Deployment",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                    "spec": {
                        "replicas": deploy.replicas,
                        "image": deploy.image,
                    },
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct PersistentVolumeHandler;

#[async_trait]
impl ApiHandler for PersistentVolumeHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                // Check if already exists
                {
                    let pvs = state.persistent_volumes.read().await;
                    if pvs.contains_key(name) {
                        return Ok(ApiResponseOrStatus::Conflict);
                    }
                }

                // Parse capacity (in bytes)
                let capacity_str = request["spec"]["capacity"]["storage"]
                    .as_str()
                    .unwrap_or("10Gi");
                let capacity = parse_storage_string(capacity_str);

                // Parse access modes
                let access_modes: Vec<crate::state::volume::AccessMode> = request["spec"]
                    ["accessModes"]
                    .as_array()
                    .map(|modes| {
                        modes
                            .iter()
                            .filter_map(|m| match m.as_str().unwrap_or("") {
                                "ReadWriteOnce" => {
                                    Some(crate::state::volume::AccessMode::ReadWriteOnce)
                                }
                                "ReadOnlyMany" => {
                                    Some(crate::state::volume::AccessMode::ReadOnlyMany)
                                }
                                "ReadWriteMany" => {
                                    Some(crate::state::volume::AccessMode::ReadWriteMany)
                                }
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse volume source
                let source = if let Some(host_path) = request["spec"]["hostPath"].as_object() {
                    crate::state::volume::VolumeSource::HostPath {
                        path: host_path["path"].as_str().unwrap_or("").to_string(),
                        type_: crate::state::volume::HostPathType::DirectoryOrCreate,
                    }
                } else if request["spec"]["nfs"].is_object() {
                    crate::state::volume::VolumeSource::Nfs {
                        server: request["spec"]["nfs"]["server"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        path: request["spec"]["nfs"]["path"]
                            .as_str()
                            .unwrap_or("/")
                            .to_string(),
                        read_only: request["spec"]["nfs"]["readOnly"]
                            .as_bool()
                            .unwrap_or(false),
                    }
                } else if let Some(pvc) = request["spec"]["persistentVolumeClaim"].as_object() {
                    crate::state::volume::VolumeSource::PersistentVolumeClaim {
                        claim_name: pvc["claimName"].as_str().unwrap_or("").to_string(),
                        read_only: pvc["readOnly"].as_bool().unwrap_or(false),
                    }
                } else {
                    crate::state::volume::VolumeSource::EmptyDir {}
                };

                let now = chrono::Utc::now();
                let pv = crate::state::volume::PersistentVolumeDesired {
                    name: name.to_string(),
                    spec: crate::state::volume::PersistentVolumeSpec {
                        capacity,
                        access_modes,
                        reclaim_policy: crate::state::volume::ReclaimPolicy::Delete,
                        storage_class: request["spec"]["storageClassName"]
                            .as_str()
                            .map(String::from),
                        volume_mode: crate::state::volume::VolumeMode::Filesystem,
                        mount_options: request["spec"]["mountOptions"]
                            .as_array()
                            .map(|opts| {
                                opts.iter()
                                    .filter_map(|o| o.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        source,
                    },
                    status: crate::state::volume::VolumeStatus::Available,
                    created_at: now,
                    updated_at: now,
                    claim_name: None,
                };

                // Store in memory
                {
                    let mut pvs = state.persistent_volumes.write().await;
                    pvs.insert(name.to_string(), pv);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolume",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "PersistentVolume".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "PersistentVolume",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut pvs = state.persistent_volumes.write().await;
                let existed = pvs.remove(name).is_some();

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolume",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "PersistentVolume".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing PV
                let existing = {
                    let pvs = state.persistent_volumes.read().await;
                    pvs.get(name).cloned()
                };

                let existing = existing
                    .ok_or_else(|| A3sError::Project("PersistentVolume not found".into()))?;

                // Apply patch - update labels, capacity, etc.
                let mut updated = existing;
                if let Some(labels) = patch
                    .get("metadata")
                    .and_then(|m| m.get("labels"))
                    .and_then(|v| v.as_object())
                {
                    updated.spec.mount_options = labels
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or("")))
                        .collect();
                }
                updated.updated_at = chrono::Utc::now();

                // Update in memory
                {
                    let mut pvs = state.persistent_volumes.write().await;
                    pvs.insert(name.to_string(), updated.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolume",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                    }),
                    kind: "PersistentVolume".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolume",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let pvs = state.persistent_volumes.read().await;
                let items: Vec<PersistentVolumeDesired> = pvs.values().cloned().collect();

                let response = ListResponse {
                    kind: "PersistentVolumeList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version: state.current_resource_version().await,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct PersistentVolumeClaimHandler;

#[async_trait]
impl ApiHandler for PersistentVolumeClaimHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let pvcs = state.persistent_volume_claims.read().await;
                    if let Some(ns_pvcs) = pvcs.get(namespace) {
                        if ns_pvcs.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                // Parse spec
                let access_modes: Vec<AccessMode> = request["spec"]["accessModes"]
                    .as_array()
                    .map(|modes| {
                        modes
                            .iter()
                            .filter_map(|m| match m.as_str().unwrap_or("") {
                                "ReadWriteOnce" => Some(AccessMode::ReadWriteOnce),
                                "ReadOnlyMany" => Some(AccessMode::ReadOnlyMany),
                                "ReadWriteMany" => Some(AccessMode::ReadWriteMany),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let storage_class_name = request["spec"]["storageClassName"]
                    .as_str()
                    .map(String::from);
                let requests: HashMap<String, i64> = request["spec"]["resources"]["requests"]
                    ["storage"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0)))
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let pvc = crate::state::PersistentVolumeClaimDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::PersistentVolumeClaimSpec {
                        access_modes,
                        resources: ClaimResources {
                            requests,
                            limits: Default::default(),
                        },
                        storage_class_name,
                        volume_name: None,
                        volume_mode: VolumeMode::Filesystem,
                        selector: None,
                    },
                    status: VolumeStatus::Pending,
                    volume_name: None,
                    created_at: now,
                    updated_at: now,
                };

                // Store in memory
                {
                    let mut pvcs = state.persistent_volume_claims.write().await;
                    pvcs.entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    pvcs.get_mut(namespace)
                        .unwrap()
                        .insert(name.to_string(), pvc.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolumeClaim",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": {
                            "accessModes": pvc.spec.access_modes.iter().map(|m| format!("{:?}", m)).collect::<Vec<_>>(),
                            "resources": pvc.spec.resources,
                        },
                    }),
                    kind: "PersistentVolumeClaim".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "PersistentVolumeClaim",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let mut pvcs = state.persistent_volume_claims.write().await;
                let existed = pvcs
                    .get_mut(namespace)
                    .map(|ns_pvcs| ns_pvcs.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolumeClaim",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "PersistentVolumeClaim".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing PVC
                let existing = {
                    let pvcs = state.persistent_volume_claims.read().await;
                    pvcs.get(namespace)
                        .and_then(|ns_pvcs| ns_pvcs.get(name).cloned())
                };

                let existing = existing
                    .ok_or_else(|| A3sError::Project("PersistentVolumeClaim not found".into()))?;

                // Apply patch - update labels
                let mut updated = existing;
                if let Some(labels) = patch
                    .get("metadata")
                    .and_then(|m| m.get("labels"))
                    .and_then(|v| v.as_object())
                {
                    // Labels can't be easily patched on PVC spec, but we broadcast the event
                    let _ = labels;
                }
                updated.updated_at = chrono::Utc::now();

                // Update in memory
                {
                    let mut pvcs = state.persistent_volume_claims.write().await;
                    if let Some(ns_pvcs) = pvcs.get_mut(namespace) {
                        ns_pvcs.insert(name.to_string(), updated.clone());
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolumeClaim",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                    }),
                    kind: "PersistentVolumeClaim".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "PersistentVolumeClaim",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let namespace = query.get("namespace");

                let pvcs = state.persistent_volume_claims.read().await;

                let items: Vec<PersistentVolumeClaimDesired> = if let Some(ns) = namespace {
                    pvcs.get(ns)
                        .map(|p| p.values().cloned().collect())
                        .unwrap_or_default()
                } else {
                    pvcs.values().flat_map(|p| p.values().cloned()).collect()
                };

                let response = ListResponse {
                    kind: "PersistentVolumeClaimList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version: state.current_resource_version().await,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ServiceAccountHandler;

#[async_trait]
impl ApiHandler for ServiceAccountHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                if state.service_accounts.get(namespace, name).await.is_some() {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let now = chrono::Utc::now();
                let sa = crate::state::ServiceAccountDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    display_name: request["metadata"]["displayName"]
                        .as_str()
                        .map(String::from),
                    description: request["metadata"]["description"]
                        .as_str()
                        .map(String::from),
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    annotations: Default::default(),
                    automount_service_account_token: request["automountServiceAccountToken"]
                        .as_bool()
                        .unwrap_or(true),
                    created_at: now,
                    updated_at: now,
                };

                state.service_accounts.create(sa.clone()).await;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ServiceAccount",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "ServiceAccount".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ServiceAccount",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let deleted = state.service_accounts.delete(namespace, name).await;
                if !deleted {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ServiceAccount",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "ServiceAccount".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing service account
                let existing = state
                    .service_accounts
                    .get(namespace, name)
                    .await
                    .ok_or_else(|| A3sError::Project("ServiceAccount not found".into()))?;

                // Apply patch
                let mut updated = existing;
                if let Some(labels) = patch.get("labels").and_then(|v| v.as_object()) {
                    updated.labels = labels
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }
                if let Some(automount) = patch
                    .get("automountServiceAccountToken")
                    .and_then(|v| v.as_bool())
                {
                    updated.automount_service_account_token = automount;
                }
                if let Some(spec) = patch.get("spec").as_ref() {
                    if let Some(automount) = spec
                        .get("automountServiceAccountToken")
                        .and_then(|v| v.as_bool())
                    {
                        updated.automount_service_account_token = automount;
                    }
                }
                updated.updated_at = chrono::Utc::now();

                state.service_accounts.update(updated.clone()).await;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ServiceAccount",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                            "labels": updated.labels,
                        },
                    }),
                    kind: "ServiceAccount".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ServiceAccount",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                            "labels": updated.labels,
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let namespace = query.get("namespace");

                let items = if let Some(ns) = namespace {
                    state.service_accounts.list(ns).await
                } else {
                    state.service_accounts.list_all().await
                };

                let response = ListResponse {
                    kind: "ServiceAccountList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version: state.current_resource_version().await,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct StorageClassHandler;

#[async_trait]
impl ApiHandler for StorageClassHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                // Check if already exists
                if state.storage_classes.get(name).await.is_some() {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let provisioner = request["provisioner"]
                    .as_str()
                    .unwrap_or("a3s.io/storage")
                    .to_string();
                let is_default = request["metadata"]["annotations"]
                    .as_object()
                    .and_then(|a| a.get("storageclass.kubernetes.io/is-default-class"))
                    .and_then(|v| v.as_bool())
                    .unwrap_or(false);

                let storage_type = request["parameters"]["type"]
                    .as_str()
                    .unwrap_or("hostpath")
                    .to_string();

                let parameters: crate::state::StorageClassParameters =
                    crate::state::StorageClassParameters {
                        provisioner: provisioner.clone(),
                        storage_type,
                        nfs_server: request["parameters"]["nfsServer"]
                            .as_str()
                            .map(String::from),
                        nfs_path: request["parameters"]["nfsPath"].as_str().map(String::from),
                        host_path_base: request["parameters"]["hostPathBase"]
                            .as_str()
                            .map(String::from),
                        mount_options: request["parameters"]["mountOptions"]
                            .as_array()
                            .map(|opts| {
                                opts.iter()
                                    .filter_map(|o| o.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        volume_binding_mode: crate::state::VolumeBindingMode::Immediate,
                        reclaim_policy: crate::state::ReclaimPolicy::Delete,
                        allow_volume_expansion: request["parameters"]["allowVolumeExpansion"]
                            .as_bool()
                            .unwrap_or(false),
                    };

                let now = chrono::Utc::now();
                let sc = crate::state::StorageClassDesired {
                    name: name.to_string(),
                    provisioner: provisioner.clone(),
                    is_default,
                    parameters,
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    annotations: Default::default(),
                    created_at: now,
                };

                state.storage_classes.create(sc).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "storage.a3s.io/v1",
                        "kind": "StorageClass",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "StorageClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "storage.a3s.io/v1",
                    "kind": "StorageClass",
                    "metadata": {
                        "name": name,
                    },
                    "provisioner": provisioner,
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let deleted = state.storage_classes.delete(name).await;
                if !deleted {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "storage.a3s.io/v1",
                        "kind": "StorageClass",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "StorageClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "storage.a3s.io/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing storage class
                let existing = state
                    .storage_classes
                    .get(name)
                    .await
                    .ok_or_else(|| A3sError::Project("StorageClass not found".into()))?;

                // Apply patch - update annotations
                let mut updated = existing;
                if let Some(annotations) = patch
                    .get("metadata")
                    .and_then(|m| m.get("annotations"))
                    .and_then(|v| v.as_object())
                {
                    updated.annotations = annotations
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }
                updated.labels = patch
                    .get("metadata")
                    .and_then(|m| m.get("labels"))
                    .and_then(|v| v.as_object())
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or(updated.labels);

                state.storage_classes.update(updated.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "storage.a3s.io/v1",
                        "kind": "StorageClass",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "StorageClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "storage.a3s.io/v1",
                        "kind": "StorageClass",
                        "metadata": {
                            "name": name,
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let items = state.storage_classes.list().await;

                let response = ListResponse {
                    kind: "StorageClassList".to_string(),
                    api_version: "storage.a3s.io/v1".to_string(),
                    metadata: ListMeta {
                        resource_version: state.current_resource_version().await,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ConfigMapHandler;

#[async_trait]
impl ApiHandler for ConfigMapHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                // Create ConfigMap
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let configmaps = state.configmaps.read().await;
                    if let Some(ns_cms) = configmaps.get(namespace) {
                        if ns_cms.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                // Create ConfigMap object
                let data: HashMap<String, String> = request["data"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let cm = ConfigMapDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    data,
                    binary_data: HashMap::new(),
                    immutable: false,
                    labels: HashMap::new(),
                    created_at: now,
                    updated_at: now,
                };

                // Get resource version from SQLite or memory
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let cm_value = serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ConfigMap",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "data": cm.data,
                    });
                    store
                        .upsert("ConfigMap", namespace, name, &cm_value)
                        .await?
                } else {
                    state.next_resource_version().await
                };

                // Store in memory
                {
                    let mut configmaps = state.configmaps.write().await;
                    configmaps
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_cms = configmaps.get_mut(namespace).unwrap();
                    ns_cms.insert(name.to_string(), cm.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ConfigMap",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                        "data": cm.data,
                    }),
                    kind: "ConfigMap".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ConfigMap",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                        "resourceVersion": resource_version,
                    },
                    "data": cm.data,
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Parse field selector (K8s compatibility)
                let field_selector = query
                    .get("fieldSelector")
                    .map(|s| FieldSelector::parse(s))
                    .unwrap_or_default();

                // Parse label selector
                let label_selector = query
                    .get("labelSelector")
                    .map(|s| FieldLabelSelector::parse(s))
                    .unwrap_or_default();

                // Parse pagination params
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();

                let configmaps = state.configmaps.read().await;

                // First collect all configmaps with their namespace
                let all_configmaps: Vec<(String, ConfigMapDesired)> = configmaps
                    .iter()
                    .flat_map(|(ns, ns_cms)| {
                        ns_cms
                            .values()
                            .map(|cm| (ns.clone(), cm.clone()))
                            .collect::<Vec<_>>()
                    })
                    .collect();

                // Then filter by selectors
                let items: Vec<ConfigMapDesired> = all_configmaps
                    .into_iter()
                    .filter(|(ns, cm)| {
                        let mut fields = HashMap::new();
                        fields.insert("metadata.namespace".to_string(), ns.clone());
                        fields.insert("metadata.name".to_string(), cm.name.clone());
                        field_selector.matches(&fields) && label_selector.matches(&cm.labels)
                    })
                    .map(|(_, cm)| cm)
                    .collect();

                // Apply pagination
                let total_count = items.len();
                let paginated_items: Vec<ConfigMapDesired> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "ConfigMapList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Delete ConfigMap
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut configmaps = state.configmaps.write().await;
                let existed = configmaps
                    .get_mut(namespace)
                    .map(|ns_cms| ns_cms.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Delete from SQLite
                if let Some(ref store) = state.sqlite_store {
                    let _ = store.delete("ConfigMap", namespace, name).await;
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ConfigMap",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "ConfigMap".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                        "message": "ConfigMap deleted",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing ConfigMap
                let existing = {
                    let configmaps = state.configmaps.read().await;
                    configmaps
                        .get(namespace)
                        .and_then(|ns_cms| ns_cms.get(name).cloned())
                };

                let existing =
                    existing.ok_or_else(|| A3sError::Project("ConfigMap not found".into()))?;

                // Apply patch - update data field
                let mut updated = existing;
                if let Some(data) = patch.get("data").and_then(|v| v.as_object()) {
                    for (k, v) in data {
                        updated
                            .data
                            .insert(k.clone(), v.as_str().unwrap_or("").to_string());
                    }
                }
                updated.updated_at = chrono::Utc::now();

                // Update in memory
                {
                    let mut configmaps = state.configmaps.write().await;
                    if let Some(ns_cms) = configmaps.get_mut(namespace) {
                        ns_cms.insert(name.to_string(), updated.clone());
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ConfigMap",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                        "data": updated.data,
                    }),
                    kind: "ConfigMap".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ConfigMap",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                        "data": updated.data,
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct SecretHandler;

#[async_trait]
impl ApiHandler for SecretHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                // Create Secret
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let secrets = state.secrets.read().await;
                    if let Some(ns_secrets) = secrets.get(namespace) {
                        if ns_secrets.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                // Parse secret data
                let data: HashMap<String, String> = request["data"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let string_data: HashMap<String, String> = request["stringData"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let secret_type = request["type"].as_str().unwrap_or("Opaque").to_string();

                let now = chrono::Utc::now();
                let secret = SecretDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    secret_type,
                    data,
                    string_data,
                    immutable: false,
                    created_at: now,
                    updated_at: now,
                };

                // Get resource version from SQLite or memory
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let secret_value = serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Secret",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "type": secret.secret_type,
                        "data": secret.data,
                    });
                    store
                        .upsert("Secret", namespace, name, &secret_value)
                        .await?
                } else {
                    state.next_resource_version().await
                };

                // Store in memory
                {
                    let mut secrets = state.secrets.write().await;
                    secrets
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_secrets = secrets.get_mut(namespace).unwrap();
                    ns_secrets.insert(name.to_string(), secret.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Secret",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                        "type": secret.secret_type,
                    }),
                    kind: "Secret".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Secret",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                        "resourceVersion": resource_version,
                    },
                    "type": secret.secret_type,
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Delete Secret
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut secrets = state.secrets.write().await;
                let existed = secrets
                    .get_mut(namespace)
                    .map(|ns_secrets| ns_secrets.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Delete from SQLite
                if let Some(ref store) = state.sqlite_store {
                    let _ = store.delete("Secret", namespace, name).await;
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Secret",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Secret".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                        "message": "Secret deleted",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing Secret
                let existing = {
                    let secrets = state.secrets.read().await;
                    secrets
                        .get(namespace)
                        .and_then(|ns_secrets| ns_secrets.get(name).cloned())
                };

                let existing =
                    existing.ok_or_else(|| A3sError::Project("Secret not found".into()))?;

                // Apply patch - update data/stringData fields
                let mut updated = existing;
                if let Some(data) = patch.get("data").and_then(|v| v.as_object()) {
                    for (k, v) in data {
                        updated
                            .data
                            .insert(k.clone(), v.as_str().unwrap_or("").to_string());
                    }
                }
                if let Some(string_data) = patch.get("stringData").and_then(|v| v.as_object()) {
                    for (k, v) in string_data {
                        updated
                            .string_data
                            .insert(k.clone(), v.as_str().unwrap_or("").to_string());
                    }
                }
                updated.updated_at = chrono::Utc::now();

                // Update in memory
                {
                    let mut secrets = state.secrets.write().await;
                    if let Some(ns_secrets) = secrets.get_mut(namespace) {
                        ns_secrets.insert(name.to_string(), updated.clone());
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Secret",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                        "type": updated.secret_type,
                    }),
                    kind: "Secret".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Secret",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                        "type": updated.secret_type,
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let namespace = query.get("namespace");

                // Parse pagination params
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();

                let secrets = state.secrets.read().await;

                let items: Vec<SecretDesired> = if let Some(ns) = namespace {
                    secrets
                        .get(ns)
                        .map(|s| s.values().cloned().collect())
                        .unwrap_or_default()
                } else {
                    secrets.values().flat_map(|s| s.values().cloned()).collect()
                };

                // Apply pagination
                let total_count = items.len();
                let paginated_items: Vec<SecretDesired> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "SecretList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version: state.current_resource_version().await,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct NodeHandler;

#[async_trait]
impl ApiHandler for NodeHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                // Check if already exists
                {
                    let nodes = state.nodes.read().await;
                    if nodes.contains_key(name) {
                        return Ok(ApiResponseOrStatus::Conflict);
                    }
                }

                // Parse node resources
                let cpu_str = request["status"]["capacity"]["cpu"].as_str().unwrap_or("4");
                let cpu_millicores = if cpu_str.parse::<i64>().is_ok() {
                    cpu_str.parse::<i64>().unwrap() * 1000
                } else {
                    4000
                };

                let memory_str = request["status"]["capacity"]["memory"]
                    .as_str()
                    .unwrap_or("8Gi");
                let memory_bytes = parse_storage_string(memory_str);

                let pods = request["status"]["capacity"]["pods"]
                    .as_i64()
                    .unwrap_or(110) as i32;

                let capacity = crate::state::node::NodeResources {
                    cpu_millicores,
                    memory_bytes,
                    pods,
                    storage_bytes: None,
                    ephemeral_storage_bytes: None,
                };

                let allocatable = crate::state::node::NodeAllocatable {
                    cpu_millicores: capacity.cpu_millicores,
                    memory_bytes: capacity.memory_bytes,
                    pods: capacity.pods,
                    storage_bytes: None,
                };

                let now = chrono::Utc::now();
                let node = crate::state::node::NodeDesired {
                    name: name.to_string(),
                    metadata: crate::state::node::NodeMeta {
                        labels: request["metadata"]["labels"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        annotations: request["metadata"]["annotations"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    },
                    system_info: crate::state::node::NodeSystemInfo {
                        operating_system: request["status"]["nodeInfo"]["operatingSystem"]
                            .as_str()
                            .unwrap_or("linux")
                            .to_string(),
                        os_image: request["status"]["nodeInfo"]["osImage"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        kernel_version: request["status"]["nodeInfo"]["kernelVersion"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        container_runtime_version: request["status"]["nodeInfo"]
                            ["containerRuntimeVersion"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        a3slet_version: request["status"]["nodeInfo"]["a3sletVersion"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        machine_id: request["status"]["nodeInfo"]["machineID"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        boot_id: request["status"]["nodeInfo"]["bootID"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        architecture: request["status"]["nodeInfo"]["architecture"]
                            .as_str()
                            .unwrap_or("amd64")
                            .to_string(),
                    },
                    capacity: capacity.clone(),
                    allocatable,
                    taints: vec![],
                    external_id: None,
                    created_at: now,
                    updated_at: now,
                };

                // Store in memory
                {
                    let mut nodes = state.nodes.write().await;
                    nodes.insert(name.to_string(), node);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Node",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "Node".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Node",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut nodes = state.nodes.write().await;
                let existed = nodes.remove(name).is_some();

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Node",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "Node".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing node
                let existing = {
                    let nodes = state.nodes.read().await;
                    nodes.get(name).cloned()
                };

                let existing =
                    existing.ok_or_else(|| A3sError::Project("Node not found".into()))?;

                // Apply patch - update labels
                let mut updated = existing;
                if let Some(labels) = patch
                    .get("metadata")
                    .and_then(|m| m.get("labels"))
                    .and_then(|v| v.as_object())
                {
                    updated.metadata.labels = labels
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }
                if let Some(annotations) = patch
                    .get("metadata")
                    .and_then(|m| m.get("annotations"))
                    .and_then(|v| v.as_object())
                {
                    updated.metadata.annotations = annotations
                        .iter()
                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                        .collect();
                }

                // Update in memory
                {
                    let mut nodes = state.nodes.write().await;
                    nodes.insert(name.to_string(), updated.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Node",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "Node".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Node",
                        "metadata": {
                            "name": name,
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if this is a request for a specific node
                if let Some(name) = query.get("name") {
                    let nodes = state.nodes.read().await;
                    if let Some(node) = nodes.get(name) {
                        let response = ListResponse {
                            kind: "NodeList".to_string(),
                            api_version: "v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![node.clone()],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Return list
                let nodes = state.nodes.read().await;
                let items: Vec<NodeDesired> = nodes.values().cloned().collect();

                // Apply pagination
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();
                let total_count = items.len();
                let paginated_items: Vec<NodeDesired> = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = ListResponse {
                    kind: "NodeList".to_string(),
                    api_version: "v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct EventHandler;

#[async_trait]
impl ApiHandler for EventHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        if method != HttpMethod::Get {
            return Ok(ApiResponseOrStatus::MethodNotAllowed);
        }

        let namespace = query.get("namespace").cloned();
        let filter = crate::state::event::EventFilter {
            namespace,
            ..Default::default()
        };

        let items = state.events.get_events(&filter).await;

        let response = ListResponse {
            kind: "EventList".to_string(),
            api_version: "v1".to_string(),
            metadata: ListMeta {
                resource_version: state.current_resource_version().await,
                continue_token: None,
            },
            items,
        };

        Ok(ApiResponseOrStatus::Response(
            serde_json::to_string(&response).unwrap(),
        ))
    }
}

struct RoleHandler;

#[async_trait]
impl ApiHandler for RoleHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| "default".to_string());

                // Parse rules
                let rules: Vec<crate::state::rbac::PolicyRule> = request["rules"]
                    .as_array()
                    .map(|rules| {
                        rules
                            .iter()
                            .map(|r| crate::state::rbac::PolicyRule {
                                api_groups: r["apiGroups"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resource_names: r["resourceNames"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resources: r["resources"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                non_resource_u_r_l_s: vec![],
                                non_resource_u_r_l_path_suffix: vec![],
                                verbs: r["verbs"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let role = crate::state::rbac::RoleDesired {
                    name: name.to_string(),
                    namespace: Some(namespace.clone()),
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    annotations: Default::default(),
                    rules,
                    created_at: now,
                    updated_at: now,
                };

                state.rbac.create_role(role.clone()).await;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "Role",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Role".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "rbac.authorization.a3s.io/v1",
                    "kind": "Role",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let deleted = state.rbac.delete_role(namespace, name).await;
                if !deleted {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "Role",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Role".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing role
                let existing = state
                    .rbac
                    .get_role(namespace, name)
                    .await
                    .ok_or_else(|| A3sError::Project("Role not found".into()))?;

                // Apply patch - update rules
                let mut updated = existing;
                if let Some(rules) = patch.get("rules").and_then(|v| v.as_array()) {
                    updated.rules = rules
                        .iter()
                        .map(|r| crate::state::rbac::PolicyRule {
                            api_groups: r["apiGroups"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resource_names: r["resourceNames"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resources: r["resources"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            non_resource_u_r_l_s: vec![],
                            non_resource_u_r_l_path_suffix: vec![],
                            verbs: r["verbs"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                        .collect();
                }
                updated.updated_at = chrono::Utc::now();

                state.rbac.update_role(updated.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "Role",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                        "rules": updated.rules,
                    }),
                    kind: "Role".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "Role",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                        "rules": updated.rules,
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let namespace = query
                    .get("namespace")
                    .map(|s| s.as_str())
                    .unwrap_or("default");
                let resource_version = state.current_resource_version().await;

                // Check for specific role request
                if let Some(name) = query.get("name") {
                    if let Some(role) = state.rbac.get_role(namespace, name).await {
                        let response = RoleListResponse {
                            kind: "RoleList".to_string(),
                            api_version: "rbac.authorization.a3s.io/v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![role],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let items = state.rbac.list_roles(namespace).await;
                let response = RoleListResponse {
                    kind: "RoleList".to_string(),
                    api_version: "rbac.authorization.a3s.io/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ClusterRoleHandler;

#[async_trait]
impl ApiHandler for ClusterRoleHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                // Parse rules
                let rules: Vec<crate::state::rbac::PolicyRule> = request["rules"]
                    .as_array()
                    .map(|rules| {
                        rules
                            .iter()
                            .map(|r| crate::state::rbac::PolicyRule {
                                api_groups: r["apiGroups"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resource_names: r["resourceNames"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resources: r["resources"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                non_resource_u_r_l_s: vec![],
                                non_resource_u_r_l_path_suffix: vec![],
                                verbs: r["verbs"]
                                    .as_array()
                                    .map(|g| {
                                        g.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let role = crate::state::rbac::ClusterRoleDesired {
                    name: name.to_string(),
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    annotations: Default::default(),
                    rules,
                    created_at: now,
                    updated_at: now,
                };

                state.rbac.create_cluster_role(role.clone()).await;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRole",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "ClusterRole".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "rbac.authorization.a3s.io/v1",
                    "kind": "ClusterRole",
                    "metadata": {
                        "name": name,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let deleted = state.rbac.delete_cluster_role(name).await;
                if !deleted {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRole",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "ClusterRole".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing cluster role
                let existing = state
                    .rbac
                    .get_cluster_role(name)
                    .await
                    .ok_or_else(|| A3sError::Project("ClusterRole not found".into()))?;

                // Apply patch - update rules
                let mut updated = existing;
                if let Some(rules) = patch.get("rules").and_then(|v| v.as_array()) {
                    updated.rules = rules
                        .iter()
                        .map(|r| crate::state::rbac::PolicyRule {
                            api_groups: r["apiGroups"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resource_names: r["resourceNames"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resources: r["resources"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            non_resource_u_r_l_s: vec![],
                            non_resource_u_r_l_path_suffix: vec![],
                            verbs: r["verbs"]
                                .as_array()
                                .map(|g| {
                                    g.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                        .collect();
                }
                updated.updated_at = chrono::Utc::now();

                state.rbac.update_cluster_role(updated.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRole",
                        "metadata": {
                            "name": name,
                        },
                        "rules": updated.rules,
                    }),
                    kind: "ClusterRole".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRole",
                        "metadata": {
                            "name": name,
                        },
                        "rules": updated.rules,
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific cluster role request
                if let Some(name) = query.get("name") {
                    if let Some(role) = state.rbac.get_cluster_role(name).await {
                        let response = ClusterRoleListResponse {
                            kind: "ClusterRoleList".to_string(),
                            api_version: "rbac.authorization.a3s.io/v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![role],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let items = state.rbac.list_cluster_roles().await;
                let response = ClusterRoleListResponse {
                    kind: "ClusterRoleList".to_string(),
                    api_version: "rbac.authorization.a3s.io/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct RoleBindingHandler;

#[async_trait]
impl ApiHandler for RoleBindingHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .map(String::from)
                    .unwrap_or_else(|| "default".to_string());

                // Parse role ref
                let role_ref = crate::state::rbac::RoleRef {
                    kind: request["roleRef"]["kind"]
                        .as_str()
                        .unwrap_or("Role")
                        .to_string(),
                    name: request["roleRef"]["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    api_group: request["roleRef"]["apiGroup"].as_str().map(String::from),
                };

                // Parse subjects
                let subjects: Vec<crate::state::rbac::Subject> = request["subjects"]
                    .as_array()
                    .map(|s| {
                        s.iter()
                            .map(|subj| crate::state::rbac::Subject {
                                kind: subj["kind"].as_str().unwrap_or("").to_string(),
                                name: subj["name"].as_str().unwrap_or("").to_string(),
                                api_group: subj["apiGroup"].as_str().map(String::from),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let binding = crate::state::rbac::RoleBindingDesired {
                    name: name.to_string(),
                    namespace: Some(namespace.clone()),
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    annotations: Default::default(),
                    role_ref,
                    subjects,
                    created_at: now,
                    updated_at: now,
                };

                state.rbac.create_role_binding(binding.clone()).await;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "RoleBinding",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "RoleBinding".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "rbac.authorization.a3s.io/v1",
                    "kind": "RoleBinding",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let deleted = state.rbac.delete_role_binding(namespace, name).await;
                if !deleted {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "RoleBinding",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "RoleBinding".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing role binding
                let existing = state
                    .rbac
                    .get_role_binding(namespace, name)
                    .await
                    .ok_or_else(|| A3sError::Project("RoleBinding not found".into()))?;

                // Apply patch - update subjects
                let mut updated = existing;
                if let Some(subjects) = patch.get("subjects").and_then(|v| v.as_array()) {
                    updated.subjects = subjects
                        .iter()
                        .map(|subj| crate::state::rbac::Subject {
                            kind: subj["kind"].as_str().unwrap_or("").to_string(),
                            name: subj["name"].as_str().unwrap_or("").to_string(),
                            api_group: subj["apiGroup"].as_str().map(String::from),
                        })
                        .collect();
                }
                updated.updated_at = chrono::Utc::now();

                state.rbac.update_role_binding(updated.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "RoleBinding",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                        "subjects": updated.subjects,
                    }),
                    kind: "RoleBinding".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "RoleBinding",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                        "subjects": updated.subjects,
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let namespace = query
                    .get("namespace")
                    .map(|s| s.as_str())
                    .unwrap_or("default");
                let resource_version = state.current_resource_version().await;

                // Check for specific role binding request
                if let Some(name) = query.get("name") {
                    if let Some(binding) = state.rbac.get_role_binding(namespace, name).await {
                        let response = RoleBindingListResponse {
                            kind: "RoleBindingList".to_string(),
                            api_version: "rbac.authorization.a3s.io/v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![binding],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let items = state.rbac.list_role_bindings(namespace).await;
                let response = RoleBindingListResponse {
                    kind: "RoleBindingList".to_string(),
                    api_version: "rbac.authorization.a3s.io/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ClusterRoleBindingHandler;

#[async_trait]
impl ApiHandler for ClusterRoleBindingHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                // Parse role ref
                let role_ref = crate::state::rbac::RoleRef {
                    kind: request["roleRef"]["kind"]
                        .as_str()
                        .unwrap_or("ClusterRole")
                        .to_string(),
                    name: request["roleRef"]["name"]
                        .as_str()
                        .unwrap_or("")
                        .to_string(),
                    api_group: request["roleRef"]["apiGroup"].as_str().map(String::from),
                };

                // Parse subjects
                let subjects: Vec<crate::state::rbac::Subject> = request["subjects"]
                    .as_array()
                    .map(|s| {
                        s.iter()
                            .map(|subj| crate::state::rbac::Subject {
                                kind: subj["kind"].as_str().unwrap_or("").to_string(),
                                name: subj["name"].as_str().unwrap_or("").to_string(),
                                api_group: subj["apiGroup"].as_str().map(String::from),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let binding = crate::state::rbac::ClusterRoleBindingDesired {
                    name: name.to_string(),
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    annotations: Default::default(),
                    role_ref,
                    subjects,
                    created_at: now,
                    updated_at: now,
                };

                state
                    .rbac
                    .create_cluster_role_binding(binding.clone())
                    .await;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRoleBinding",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "ClusterRoleBinding".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "rbac.authorization.a3s.io/v1",
                    "kind": "ClusterRoleBinding",
                    "metadata": {
                        "name": name,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let deleted = state.rbac.delete_cluster_role_binding(name).await;
                if !deleted {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRoleBinding",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "ClusterRoleBinding".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing cluster role binding
                let existing = state
                    .rbac
                    .get_cluster_role_binding(name)
                    .await
                    .ok_or_else(|| A3sError::Project("ClusterRoleBinding not found".into()))?;

                // Apply patch - update subjects
                let mut updated = existing;
                if let Some(subjects) = patch.get("subjects").and_then(|v| v.as_array()) {
                    updated.subjects = subjects
                        .iter()
                        .map(|subj| crate::state::rbac::Subject {
                            kind: subj["kind"].as_str().unwrap_or("").to_string(),
                            name: subj["name"].as_str().unwrap_or("").to_string(),
                            api_group: subj["apiGroup"].as_str().map(String::from),
                        })
                        .collect();
                }
                updated.updated_at = chrono::Utc::now();

                state
                    .rbac
                    .update_cluster_role_binding(updated.clone())
                    .await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRoleBinding",
                        "metadata": {
                            "name": name,
                        },
                        "subjects": updated.subjects,
                    }),
                    kind: "ClusterRoleBinding".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "rbac.authorization.a3s.io/v1",
                        "kind": "ClusterRoleBinding",
                        "metadata": {
                            "name": name,
                        },
                        "subjects": updated.subjects,
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific cluster role binding request
                if let Some(name) = query.get("name") {
                    if let Some(binding) = state.rbac.get_cluster_role_binding(name).await {
                        let response = ClusterRoleBindingListResponse {
                            kind: "ClusterRoleBindingList".to_string(),
                            api_version: "rbac.authorization.a3s.io/v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![binding],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let items = state.rbac.list_cluster_role_bindings().await;
                let response = ClusterRoleBindingListResponse {
                    kind: "ClusterRoleBindingList".to_string(),
                    api_version: "rbac.authorization.a3s.io/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: None,
                    },
                    items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct DaemonSetHandler;

#[async_trait]
impl ApiHandler for DaemonSetHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                if state
                    .daemonsets
                    .get_daemonset(namespace, name)
                    .await
                    .is_some()
                {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                // Parse DaemonSet spec
                let selector = request["spec"]["selector"]
                    .as_object()
                    .map(|obj| crate::state::daemon::DaemonSetSelector {
                        match_labels: obj["matchLabels"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        match_expressions: vec![],
                    })
                    .unwrap_or_default();

                let template_labels = request["spec"]["template"]["metadata"]["labels"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let template_spec = &request["spec"]["template"]["spec"];
                let image = template_spec["image"].as_str().unwrap_or("").to_string();

                let replicas = request["spec"]["replicas"].as_i64().unwrap_or(1) as i32;

                let now = chrono::Utc::now();
                let ds = crate::state::daemon::DaemonSetDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::daemon::DaemonSetSpec {
                        selector,
                        template: crate::state::daemon::DaemonSetPodTemplate {
                            metadata: crate::state::daemon::DaemonSetPodMetadata {
                                labels: template_labels,
                                annotations: Default::default(),
                            },
                            spec: crate::state::daemon::DaemonSetPodSpec {
                                image,
                                command: template_spec["command"]
                                    .as_array()
                                    .map(|c| {
                                        c.iter()
                                            .filter_map(|s| s.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                args: template_spec["args"]
                                    .as_array()
                                    .map(|c| {
                                        c.iter()
                                            .filter_map(|s| s.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                env: template_spec["env"]
                                    .as_object()
                                    .map(|m| {
                                        m.iter()
                                            .map(|(k, v)| {
                                                (k.clone(), v.as_str().unwrap_or("").to_string())
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                ports: template_spec["ports"]
                                    .as_array()
                                    .map(|ports| {
                                        ports
                                            .iter()
                                            .map(|p| crate::state::daemon::DaemonPortMapping {
                                                name: p["name"].as_str().unwrap_or("").to_string(),
                                                container_port: p["containerPort"]
                                                    .as_i64()
                                                    .unwrap_or(80)
                                                    as u16,
                                                host_port: p["hostPort"].as_i64().unwrap_or(0)
                                                    as u16,
                                                protocol: p["protocol"]
                                                    .as_str()
                                                    .unwrap_or("TCP")
                                                    .to_string(),
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resources: Default::default(),
                                volume_mounts: vec![],
                                health_check: Default::default(),
                            },
                        },
                        strategy: Default::default(),
                        min_ready_secs: 0,
                        replicas,
                    },
                    version: "v1".to_string(),
                    created_at: now,
                    updated_at: now,
                };

                state.daemonsets.create_daemonset(ds.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "DaemonSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": {
                            "replicas": ds.spec.replicas,
                        },
                    }),
                    kind: "DaemonSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "apps/v1",
                    "kind": "DaemonSet",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": {
                        "replicas": ds.spec.replicas,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                state.daemonsets.delete_daemonset(namespace, name).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "DaemonSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "DaemonSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "apps/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing daemonset
                let existing = state
                    .daemonsets
                    .get_daemonset(namespace, name)
                    .await
                    .ok_or_else(|| A3sError::Project("DaemonSet not found".into()))?;

                // Apply patch - support common fields
                let mut updated = existing;
                if let Some(replicas) = patch.get("replicas").and_then(|v| v.as_i64()) {
                    updated.spec.replicas = replicas as i32;
                }
                if let Some(image) = patch.get("image").and_then(|v| v.as_str()) {
                    updated.spec.template.spec.image = image.to_string();
                }
                if let Some(spec) = patch.get("spec").as_ref() {
                    if let Some(replicas) = spec.get("replicas").and_then(|v| v.as_i64()) {
                        updated.spec.replicas = replicas as i32;
                    }
                    if let Some(image) = spec.get("image").and_then(|v| v.as_str()) {
                        updated.spec.template.spec.image = image.to_string();
                    }
                    if let Some(template) = spec.get("template").as_ref() {
                        if let Some(spec) = template.get("spec").as_ref() {
                            if let Some(image) = spec.get("image").and_then(|v| v.as_str()) {
                                updated.spec.template.spec.image = image.to_string();
                            }
                        }
                    }
                }
                updated.spec.replicas = updated.spec.replicas.max(0);
                updated.spec.replicas = updated.spec.replicas.min(i32::MAX as i32) as i32;

                let now = chrono::Utc::now();
                updated.updated_at = now;

                state.daemonsets.update_daemonset(updated.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "DaemonSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                            "labels": updated.spec.template.metadata.labels,
                        },
                        "spec": {
                            "replicas": updated.spec.replicas,
                            "template": {
                                "spec": {
                                    "image": updated.spec.template.spec.image,
                                },
                            },
                        },
                    }),
                    kind: "DaemonSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "DaemonSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                            "labels": updated.spec.template.metadata.labels,
                        },
                        "spec": {
                            "replicas": updated.spec.replicas,
                            "template": {
                                "spec": {
                                    "image": updated.spec.template.spec.image,
                                },
                            },
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific daemonset request
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    if let Some(ds) = state.daemonsets.get_daemonset(namespace, name).await {
                        let response = DaemonSetListResponse {
                            kind: "DaemonSetList".to_string(),
                            api_version: "apps/v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![ds],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List all or by namespace
                let namespace = query.get("namespace").map(|s| s.as_str());
                let items = if let Some(ns) = namespace {
                    state.daemonsets.list_daemonsets(ns).await
                } else {
                    state
                        .daemonsets
                        .list_all_daemonsets()
                        .await
                        .into_iter()
                        .map(|(_, ds)| ds)
                        .collect()
                };

                // Apply pagination
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();
                let total_count = items.len();
                let paginated_items = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = DaemonSetListResponse {
                    kind: "DaemonSetList".to_string(),
                    api_version: "apps/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct StatefulSetHandler;

#[async_trait]
impl ApiHandler for StatefulSetHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                if state
                    .statefulsets
                    .get_statefulset(namespace, name)
                    .await
                    .is_some()
                {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let replicas = request["spec"]["replicas"].as_i64().unwrap_or(1) as i32;
                let service_name = request["spec"]["serviceName"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();

                let now = chrono::Utc::now();
                let ss = crate::state::stateful::StatefulSetDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    version: "v1".to_string(),
                    spec: crate::state::stateful::StatefulSetSpec {
                        replicas,
                        service_name: service_name.clone(),
                        selector: crate::state::stateful::StatefulSetSelector {
                            match_labels: request["spec"]["selector"]["matchLabels"]
                                .as_object()
                                .map(|m| {
                                    m.iter()
                                        .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            match_expressions: vec![],
                        },
                        template: crate::state::stateful::StatefulSetPodTemplate {
                            metadata: crate::state::stateful::StatefulSetPodMetadata {
                                labels: request["spec"]["template"]["metadata"]["labels"]
                                    .as_object()
                                    .map(|m| {
                                        m.iter()
                                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                annotations: Default::default(),
                            },
                            spec: crate::state::stateful::StatefulSetPodSpec {
                                image: request["spec"]["template"]["spec"]["image"]
                                    .as_str()
                                    .unwrap_or("")
                                    .to_string(),
                                command: request["spec"]["template"]["spec"]["command"]
                                    .as_array()
                                    .map(|c| c.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                                    .unwrap_or_default(),
                                args: request["spec"]["template"]["spec"]["args"]
                                    .as_array()
                                    .map(|c| c.iter().filter_map(|s| s.as_str().map(String::from)).collect())
                                    .unwrap_or_default(),
                                env: request["spec"]["template"]["spec"]["env"]
                                    .as_object()
                                    .map(|m| {
                                        m.iter()
                                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                ports: request["spec"]["template"]["spec"]["ports"]
                                    .as_array()
                                    .map(|ports| {
                                        ports
                                            .iter()
                                            .map(|p| crate::state::stateful::StatefulSetPortMapping {
                                                name: p["name"].as_str().unwrap_or("").to_string(),
                                                container_port: p["containerPort"].as_i64().unwrap_or(80) as u16,
                                                host_port: p["hostPort"].as_i64().unwrap_or(0) as u16,
                                                protocol: p["protocol"].as_str().unwrap_or("TCP").to_string(),
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resources: Default::default(),
                                volume_mounts: vec![],
                                health_check: Default::default(),
                            },
                        },
                        volume_claim_templates: request["spec"]["volumeClaimTemplates"]
                            .as_array()
                            .map(|templates| {
                                templates
                                    .iter()
                                    .map(|t| crate::state::stateful::StatefulSetVolumeClaimTemplate {
                                        name: t["metadata"]["name"].as_str().unwrap_or("").to_string(),
                                        storage: t["spec"]["resources"]["requests"]["storage"]
                                            .as_str()
                                            .unwrap_or("1Gi")
                                            .to_string(),
                                        storage_class: t["spec"]["storageClassName"].as_str().map(String::from),
                                        access_mode: t["spec"]["accessModes"]
                                            .as_array()
                                            .and_then(|modes| modes.first())
                                            .and_then(|m| {
                                                match m.as_str().unwrap_or("") {
                                                    "ReadWriteOnce" => Some(crate::state::stateful::StatefulSetAccessMode::ReadWriteOnce),
                                                    "ReadOnlyMany" => Some(crate::state::stateful::StatefulSetAccessMode::ReadOnlyMany),
                                                    "ReadWriteMany" => Some(crate::state::stateful::StatefulSetAccessMode::ReadWriteMany),
                                                    _ => None,
                                                }
                                            })
                                            .unwrap_or_default(),
                                        selector: None,
                                    })
                                    .collect()
                            })
                            .unwrap_or_default(),
                        update_strategy: Default::default(),
                        persistent_volume_claim_retention_policy: None,
                        min_ready_secs: 0,
                        revision_history_limit: 10,
                    },
                    created_at: now,
                    updated_at: now,
                };

                state.statefulsets.create_statefulset(ss.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "StatefulSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": {
                            "replicas": ss.spec.replicas,
                            "serviceName": service_name,
                        },
                    }),
                    kind: "StatefulSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "apps/v1",
                    "kind": "StatefulSet",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": {
                        "replicas": ss.spec.replicas,
                        "serviceName": service_name,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                state
                    .statefulsets
                    .delete_statefulset(namespace, name)
                    .await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "StatefulSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "StatefulSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "apps/v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing statefulset
                let existing = state
                    .statefulsets
                    .get_statefulset(namespace, name)
                    .await
                    .ok_or_else(|| A3sError::Project("StatefulSet not found".into()))?;

                // Apply patch
                let mut updated = existing;
                if let Some(replicas) = patch.get("replicas").and_then(|v| v.as_i64()) {
                    updated.spec.replicas = replicas as i32;
                }
                if let Some(image) = patch.get("image").and_then(|v| v.as_str()) {
                    updated.spec.template.spec.image = image.to_string();
                }
                if let Some(spec) = patch.get("spec").as_ref() {
                    if let Some(replicas) = spec.get("replicas").and_then(|v| v.as_i64()) {
                        updated.spec.replicas = replicas as i32;
                    }
                    if let Some(image) = spec.get("image").and_then(|v| v.as_str()) {
                        updated.spec.template.spec.image = image.to_string();
                    }
                    if let Some(template) = spec.get("template").as_ref() {
                        if let Some(spec) = template.get("spec").as_ref() {
                            if let Some(image) = spec.get("image").and_then(|v| v.as_str()) {
                                updated.spec.template.spec.image = image.to_string();
                            }
                        }
                    }
                }
                updated.spec.replicas = updated.spec.replicas.max(0).min(i32::MAX as i32);
                updated.updated_at = chrono::Utc::now();

                state
                    .statefulsets
                    .update_statefulset(updated.clone())
                    .await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "StatefulSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                        "spec": {
                            "replicas": updated.spec.replicas,
                        },
                    }),
                    kind: "StatefulSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "StatefulSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": updated.created_at.to_rfc3339(),
                        },
                        "spec": {
                            "replicas": updated.spec.replicas,
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific statefulset request
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    if let Some(ss) = state.statefulsets.get_statefulset(namespace, name).await {
                        let response = StatefulSetListResponse {
                            kind: "StatefulSetList".to_string(),
                            api_version: "apps/v1".to_string(),
                            metadata: ListMeta {
                                resource_version,
                                continue_token: None,
                            },
                            items: vec![ss],
                        };
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List all or by namespace
                let namespace = query.get("namespace").map(|s| s.as_str());
                let items = if let Some(ns) = namespace {
                    state.statefulsets.list_statefulsets(ns).await
                } else {
                    state
                        .statefulsets
                        .list_all_statefulsets()
                        .await
                        .into_iter()
                        .map(|(_, ss)| ss)
                        .collect()
                };

                // Apply pagination
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();
                let total_count = items.len();
                let paginated_items = if pagination.limit.is_some() {
                    items
                        .into_iter()
                        .skip(offset)
                        .take(pagination.limit.unwrap_or(usize::MAX))
                        .collect()
                } else {
                    items
                };
                let paginated = PaginatedList::new(paginated_items, total_count, offset);

                let response = StatefulSetListResponse {
                    kind: "StatefulSetList".to_string(),
                    api_version: "apps/v1".to_string(),
                    metadata: ListMeta {
                        resource_version,
                        continue_token: paginated.continue_token,
                    },
                    items: paginated.items,
                };

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct IngressHandler;

#[async_trait]
impl ApiHandler for IngressHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("default");

                // Check if already exists
                {
                    let ingresses = state.ingresses.read().await;
                    if let Some(ns_ing) = ingresses.get(namespace) {
                        if ns_ing.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                // Parse ingress rules
                let rules: Vec<IngressRule> = request["spec"]["rules"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|r| IngressRule {
                                host: r["host"].as_str().unwrap_or("*").to_string(),
                                paths: r["http"]["paths"]
                                    .as_array()
                                    .map(|paths| {
                                        paths
                                            .iter()
                                            .map(|p| IngressPath {
                                                path: p["path"].as_str().unwrap_or("/").to_string(),
                                                backend: p["backend"]["service"]["name"]
                                                    .as_str()
                                                    .unwrap_or("unknown")
                                                    .to_string(),
                                                port: p["backend"]["service"]["port"]
                                                    .as_i64()
                                                    .unwrap_or(80)
                                                    as u16,
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse TLS
                let tls: Vec<IngressTls> = request["spec"]["tls"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|t| IngressTls {
                                hosts: t["hosts"]
                                    .as_array()
                                    .map(|h| {
                                        h.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                secret_name: t["secretName"].as_str().unwrap_or("").to_string(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let now = chrono::Utc::now();
                let ingress = IngressDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: IngressSpec { rules, tls },
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    created_at: now,
                    updated_at: now,
                };

                // Store in memory
                {
                    let mut ingresses = state.ingresses.write().await;
                    ingresses
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    ingresses
                        .get_mut(namespace)
                        .unwrap()
                        .insert(name.to_string(), ingress.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "Ingress",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "Ingress".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "networking.a3s.io/v1",
                    "kind": "Ingress",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut ingresses = state.ingresses.write().await;
                let existed = ingresses
                    .get_mut(namespace)
                    .map(|ns_ing| ns_ing.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "Ingress",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Ingress".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific ingress request
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    let ingresses = state.ingresses.read().await;
                    if let Some(ns_ing) = ingresses.get(namespace) {
                        if let Some(ingress) = ns_ing.get(name) {
                            let response = serde_json::json!({
                                "apiVersion": "networking.a3s.io/v1",
                                "kind": "Ingress",
                                "metadata": {
                                    "name": ingress.name,
                                    "namespace": ingress.namespace,
                                    "creationTimestamp": ingress.created_at.to_rfc3339(),
                                    "labels": ingress.labels,
                                },
                                "spec": {
                                    "rules": ingress.spec.rules.iter().map(|r| {
                                        serde_json::json!({
                                            "host": r.host,
                                            "http": {
                                                "paths": r.paths.iter().map(|p| {
                                                    serde_json::json!({
                                                        "path": p.path,
                                                        "backend": {
                                                            "service": {
                                                                "name": p.backend,
                                                                "port": {
                                                                    "number": p.port
                                                                }
                                                            }
                                                        }
                                                    })
                                                }).collect::<Vec<_>>()
                                            }
                                        })
                                    }).collect::<Vec<_>>(),
                                    "tls": ingress.spec.tls.iter().map(|t| {
                                        serde_json::json!({
                                            "hosts": t.hosts,
                                            "secretName": t.secret_name,
                                        })
                                    }).collect::<Vec<_>>()
                                }
                            });
                            return Ok(ApiResponseOrStatus::Response(
                                serde_json::to_string(&response).unwrap(),
                            ));
                        }
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List ingresses
                let namespace = query.get("namespace").map(|s| s.as_str());
                let all_ingresses: Vec<IngressDesired> = if let Some(ns) = namespace {
                    let ingresses = state.ingresses.read().await;
                    ingresses
                        .get(ns)
                        .map(|ns_ing| ns_ing.values().cloned().collect())
                        .unwrap_or_default()
                } else {
                    let ingresses = state.ingresses.read().await;
                    ingresses
                        .values()
                        .flat_map(|ns_ing| ns_ing.values().cloned())
                        .collect()
                };

                // Apply pagination
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();
                let total_count = all_ingresses.len();
                let paginated_items: Vec<IngressDesired> = all_ingresses
                    .into_iter()
                    .skip(offset)
                    .take(pagination.limit.unwrap_or(usize::MAX))
                    .collect();
                let paginated = PaginatedList::new(paginated_items.clone(), total_count, offset);

                let items: Vec<serde_json::Value> = paginated_items
                    .iter()
                    .map(|ingress| {
                        serde_json::json!({
                            "apiVersion": "networking.a3s.io/v1",
                            "kind": "Ingress",
                            "metadata": {
                                "name": ingress.name,
                                "namespace": ingress.namespace,
                                "creationTimestamp": ingress.created_at.to_rfc3339(),
                                "labels": ingress.labels,
                            },
                            "spec": {
                                "rules": ingress.spec.rules.iter().map(|r| {
                                    serde_json::json!({
                                        "host": r.host,
                                        "http": {
                                            "paths": r.paths.iter().map(|p| {
                                                serde_json::json!({
                                                    "path": p.path,
                                                    "backend": {
                                                        "service": {
                                                            "name": p.backend,
                                                            "port": {
                                                                "number": p.port
                                                            }
                                                        }
                                                    }
                                                })
                                            }).collect::<Vec<_>>()
                                        }
                                    })
                                }).collect::<Vec<_>>(),
                                "tls": ingress.spec.tls.iter().map(|t| {
                                    serde_json::json!({
                                        "hosts": t.hosts,
                                        "secretName": t.secret_name,
                                    })
                                }).collect::<Vec<_>>()
                            }
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "IngressList",
                    "metadata": {
                        "resourceVersion": resource_version,
                        "continue": paginated.continue_token,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing ingress
                let mut ingresses = state.ingresses.write().await;
                let existing = ingresses
                    .get_mut(namespace)
                    .and_then(|ns_ing| ns_ing.get_mut(name))
                    .cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut ingress = existing.unwrap();
                drop(ingresses);

                // Apply patch to labels
                if let Some(labels) = patch["metadata"]["labels"].as_object() {
                    for (k, v) in labels {
                        if v.is_null() {
                            ingress.labels.remove(k);
                        } else {
                            ingress
                                .labels
                                .insert(k.clone(), v.as_str().unwrap_or("").to_string());
                        }
                    }
                }

                // Update in memory
                {
                    let mut ingresses = state.ingresses.write().await;
                    if let Some(ns_ing) = ingresses.get_mut(namespace) {
                        if let Some(existing_ingress) = ns_ing.get_mut(name) {
                            *existing_ingress = ingress.clone();
                        }
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "Ingress",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Ingress".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "networking.a3s.io/v1",
                    "kind": "Ingress",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct JobHandler;

#[async_trait]
impl ApiHandler for JobHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("default");

                // Check if already exists
                if state.jobs.get_job(namespace, name).await.is_some() {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let now = chrono::Utc::now();

                // Parse job spec from request
                let parallelism = request["spec"]["parallelism"].as_i64().map(|v| v as i32);
                let completions = request["spec"]["completions"].as_i64().map(|v| v as i32);
                let active_deadline_seconds = request["spec"]["activeDeadlineSeconds"].as_i64();
                let ttl_seconds_after_finished = request["spec"]["ttlSecondsAfterFinished"]
                    .as_i64()
                    .map(|v| v as i32);

                // Get container info from template
                let container = request["spec"]["template"]["spec"]["containers"]
                    .as_array()
                    .and_then(|c| c.first());

                let image = container
                    .and_then(|c| c["image"].as_str())
                    .unwrap_or("alpine:latest")
                    .to_string();

                let command = container
                    .and_then(|c| c["command"].as_array())
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str().map(String::from))
                            .collect()
                    })
                    .unwrap_or_default();

                let job = JobDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::batch::JobSpec {
                        completions,
                        parallelism,
                        active_deadline_seconds,
                        backoff_limit: Default::default(),
                        retry_policy: Default::default(),
                        template: crate::state::batch::JobPodTemplate {
                            metadata: crate::state::batch::JobPodMetadata {
                                labels: request["spec"]["template"]["metadata"]["labels"]
                                    .as_object()
                                    .map(|m| {
                                        m.iter()
                                            .map(|(k, v)| {
                                                (k.clone(), v.as_str().unwrap_or("").to_string())
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                annotations: Default::default(),
                            },
                            spec: crate::state::batch::JobPodSpec {
                                image,
                                command,
                                args: container
                                    .and_then(|c| c["args"].as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|v| v.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                env: container
                                    .and_then(|c| c["env"].as_array())
                                    .map(|arr| {
                                        arr.iter()
                                            .filter_map(|e| {
                                                Some((
                                                    e["name"].as_str()?.to_string(),
                                                    e["value"].as_str()?.to_string(),
                                                ))
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                resources: Default::default(),
                                restart_policy: Default::default(),
                            },
                        },
                        completion_mode: Default::default(),
                        ttl_seconds_after_finished,
                    },
                    created_at: now,
                    updated_at: now,
                };

                state.jobs.create_job(job.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "Job",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "Job".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "batch/v1",
                    "kind": "Job",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                state.jobs.delete_job(namespace, name).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "Job",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Job".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific job request
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    if let Some(job) = state.jobs.get_job(namespace, name).await {
                        let response = serde_json::json!({
                            "apiVersion": "batch/v1",
                            "kind": "Job",
                            "metadata": {
                                "name": job.name,
                                "namespace": job.namespace,
                                "creationTimestamp": job.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "parallelism": job.spec.parallelism,
                                "completions": job.spec.completions,
                                "activeDeadlineSeconds": job.spec.active_deadline_seconds,
                                "ttlSecondsAfterFinished": job.spec.ttl_seconds_after_finished,
                            },
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List jobs
                let namespace = query.get("namespace").map(|s| s.as_str());
                let items = if let Some(ns) = namespace {
                    state.jobs.list_jobs(ns).await
                } else {
                    state
                        .jobs
                        .list_all_jobs()
                        .await
                        .into_iter()
                        .map(|(_, j)| j)
                        .collect()
                };

                let job_values: Vec<serde_json::Value> = items
                    .iter()
                    .map(|job| {
                        serde_json::json!({
                            "apiVersion": "batch/v1",
                            "kind": "Job",
                            "metadata": {
                                "name": job.name,
                                "namespace": job.namespace,
                                "creationTimestamp": job.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "parallelism": job.spec.parallelism,
                                "completions": job.spec.completions,
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "JobList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": job_values,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                if state.jobs.get_job(namespace, name).await.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "Job",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Job".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "Job",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct CronJobHandler;

#[async_trait]
impl ApiHandler for CronJobHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("default");

                // Check if already exists
                if state.cronjobs.get_cronjob(namespace, name).await.is_some() {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let now = chrono::Utc::now();

                // Parse schedule
                let schedule_str = request["spec"]["schedule"].as_str().unwrap_or("* * * * *");
                let schedule =
                    crate::state::batch::CronJobSchedule::parse(schedule_str).unwrap_or_default();

                // Parse concurrency policy
                let concurrency_policy = match request["spec"]["concurrencyPolicy"].as_str() {
                    Some("Forbid") => crate::state::batch::CronJobConcurrencyPolicy::Forbid,
                    Some("Replace") => crate::state::batch::CronJobConcurrencyPolicy::Replace,
                    _ => crate::state::batch::CronJobConcurrencyPolicy::Allow,
                };

                let cronjob = CronJobDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::batch::CronJobSpec {
                        schedule,
                        time_zone: request["spec"]["timeZone"].as_str().map(String::from),
                        starting_deadline_seconds: request["spec"]["startingDeadlineSeconds"]
                            .as_i64(),
                        job_template: crate::state::batch::JobTemplate {
                            spec: crate::state::batch::JobSpec {
                                completions: request["spec"]["jobTemplate"]["spec"]["completions"]
                                    .as_i64()
                                    .map(|v| v as i32),
                                parallelism: request["spec"]["jobTemplate"]["spec"]["parallelism"]
                                    .as_i64()
                                    .map(|v| v as i32),
                                active_deadline_seconds: request["spec"]["jobTemplate"]["spec"]
                                    ["activeDeadlineSeconds"]
                                    .as_i64(),
                                backoff_limit: Default::default(),
                                retry_policy: Default::default(),
                                template: crate::state::batch::JobPodTemplate {
                                    metadata: crate::state::batch::JobPodMetadata {
                                        labels: request["spec"]["jobTemplate"]["spec"]["template"]
                                            ["metadata"]["labels"]
                                            .as_object()
                                            .map(|m| {
                                                m.iter()
                                                    .map(|(k, v)| {
                                                        (
                                                            k.clone(),
                                                            v.as_str().unwrap_or("").to_string(),
                                                        )
                                                    })
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                        annotations: Default::default(),
                                    },
                                    spec: crate::state::batch::JobPodSpec {
                                        image: request["spec"]["jobTemplate"]["spec"]["template"]
                                            ["spec"]["containers"]
                                            .as_array()
                                            .and_then(|c| c.first())
                                            .and_then(|img| img["image"].as_str())
                                            .unwrap_or("alpine:latest")
                                            .to_string(),
                                        command: request["spec"]["jobTemplate"]["spec"]["template"]
                                            ["spec"]["containers"]
                                            .as_array()
                                            .and_then(|c| c.first())
                                            .and_then(|img| img["command"].as_array())
                                            .map(|arr| {
                                                arr.iter()
                                                    .filter_map(|v| v.as_str().map(String::from))
                                                    .collect()
                                            })
                                            .unwrap_or_default(),
                                        args: Default::default(),
                                        env: Default::default(),
                                        resources: Default::default(),
                                        restart_policy: Default::default(),
                                    },
                                },
                                completion_mode: Default::default(),
                                ttl_seconds_after_finished: request["spec"]["jobTemplate"]["spec"]
                                    ["ttlSecondsAfterFinished"]
                                    .as_i64()
                                    .map(|v| v as i32),
                            },
                        },
                        concurrency_policy,
                        suspend: request["spec"]["suspend"].as_bool().unwrap_or(false),
                        successful_jobs_history_limit: request["spec"]
                            ["successfulJobsHistoryLimit"]
                            .as_i64()
                            .map(|v| v as i32),
                        failed_jobs_history_limit: request["spec"]["failedJobsHistoryLimit"]
                            .as_i64()
                            .map(|v| v as i32),
                    },
                    last_scheduled_time: None,
                    active_jobs: vec![],
                    created_at: now,
                    updated_at: now,
                };

                state.cronjobs.create_cronjob(cronjob.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "CronJob",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "CronJob".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "batch/v1",
                    "kind": "CronJob",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                state.cronjobs.delete_cronjob(namespace, name).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "CronJob",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "CronJob".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific cronjob request
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    if let Some(cronjob) = state.cronjobs.get_cronjob(namespace, name).await {
                        let response = serde_json::json!({
                            "apiVersion": "batch/v1",
                            "kind": "CronJob",
                            "metadata": {
                                "name": cronjob.name,
                                "namespace": cronjob.namespace,
                                "creationTimestamp": cronjob.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "schedule": cronjob.spec.schedule.format(),
                                "concurrencyPolicy": format!("{:?}", cronjob.spec.concurrency_policy),
                                "suspend": cronjob.spec.suspend,
                            },
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List cronjobs
                let namespace = query.get("namespace").map(|s| s.as_str());
                let items = if let Some(ns) = namespace {
                    state.cronjobs.list_cronjobs(ns).await
                } else {
                    state
                        .cronjobs
                        .list_all_cronjobs()
                        .await
                        .into_iter()
                        .map(|(_, cj)| cj)
                        .collect()
                };

                let cronjob_values: Vec<serde_json::Value> = items
                    .iter()
                    .map(|cronjob| {
                        serde_json::json!({
                            "apiVersion": "batch/v1",
                            "kind": "CronJob",
                            "metadata": {
                                "name": cronjob.name,
                                "namespace": cronjob.namespace,
                                "creationTimestamp": cronjob.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "schedule": cronjob.spec.schedule.format(),
                                "concurrencyPolicy": format!("{:?}", cronjob.spec.concurrency_policy),
                                "suspend": cronjob.spec.suspend,
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "CronJobList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": cronjob_values,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                if state.cronjobs.get_cronjob(namespace, name).await.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "CronJob",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "CronJob".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "batch/v1",
                        "kind": "CronJob",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ReplicaSetHandler;

#[async_trait]
impl ApiHandler for ReplicaSetHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .unwrap_or("default");

                // Check if already exists
                {
                    let rs = state.replicasets.read().await;
                    if let Some(ns_rs) = rs.get(namespace) {
                        if ns_rs.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                let now = chrono::Utc::now();

                // Parse replicas
                let replicas = request["spec"]["replicas"].as_i64().map(|v| v as i32);

                // Parse selector
                let match_labels: std::collections::HashMap<String, String> = request["spec"]
                    ["selector"]["matchLabels"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();

                let match_expressions: Vec<LabelSelectorExpression> = request["spec"]["selector"]
                    ["matchExpressions"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|e| LabelSelectorExpression {
                                key: e["key"].as_str().unwrap_or("").to_string(),
                                operator: e["operator"].as_str().unwrap_or("").to_string(),
                                values: e["values"]
                                    .as_array()
                                    .map(|v| {
                                        v.iter()
                                            .filter_map(|x| x.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse template
                let template = request["spec"]["template"].as_object().map(|_| {
                    let containers: Vec<ContainerSpec> = request["spec"]["template"]["spec"]
                        ["containers"]
                        .as_array()
                        .map(|arr| {
                            arr.iter()
                                .map(|c| ContainerSpec {
                                    name: c["name"].as_str().unwrap_or("").to_string(),
                                    image: c["image"].as_str().unwrap_or("").to_string(),
                                    command: c["command"]
                                        .as_array()
                                        .map(|v| {
                                            v.iter()
                                                .filter_map(|x| x.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    args: c["args"]
                                        .as_array()
                                        .map(|v| {
                                            v.iter()
                                                .filter_map(|x| x.as_str().map(String::from))
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    env: c["env"]
                                        .as_array()
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|e| {
                                                    Some(EnvVar {
                                                        name: e["name"].as_str()?.to_string(),
                                                        value: e["value"]
                                                            .as_str()
                                                            .map(String::from),
                                                    })
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    ports: c["ports"]
                                        .as_array()
                                        .map(|arr| {
                                            arr.iter()
                                                .filter_map(|p| {
                                                    Some(ContainerPort {
                                                        container_port: p["containerPort"]
                                                            .as_u64()?
                                                            as u16,
                                                        protocol: p["protocol"]
                                                            .as_str()
                                                            .map(String::from),
                                                    })
                                                })
                                                .collect()
                                        })
                                        .unwrap_or_default(),
                                    resources: Default::default(),
                                })
                                .collect()
                        })
                        .unwrap_or_default();

                    PodTemplateSpec {
                        metadata: PodTemplateMetadata {
                            labels: request["spec"]["template"]["metadata"]["labels"]
                                .as_object()
                                .map(|m| {
                                    m.iter()
                                        .map(|(k, v)| {
                                            (k.clone(), v.as_str().unwrap_or("").to_string())
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                            annotations: Default::default(),
                        },
                        spec: Some(PodTemplateSpecCore {
                            containers,
                            restart_policy: request["spec"]["template"]["spec"]["restartPolicy"]
                                .as_str()
                                .map(String::from),
                        }),
                    }
                });

                let rs = ReplicaSetDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: ReplicaSetSpec {
                        min_ready_seconds: request["spec"]["minReadySeconds"]
                            .as_u64()
                            .map(|v| v as u32),
                        replicas,
                        selector: ReplicaSetSelector {
                            match_labels,
                            match_expressions,
                        },
                        template,
                    },
                    labels: request["metadata"]["labels"]
                        .as_object()
                        .map(|m| {
                            m.iter()
                                .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                .collect()
                        })
                        .unwrap_or_default(),
                    owner_references: Default::default(),
                    created_at: now,
                    updated_at: now,
                };

                // Store in memory
                {
                    let mut replicasets = state.replicasets.write().await;
                    replicasets
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    replicasets
                        .get_mut(namespace)
                        .unwrap()
                        .insert(name.to_string(), rs.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "ReplicaSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "ReplicaSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "apps/v1",
                    "kind": "ReplicaSet",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                // Remove from memory
                let mut replicasets = state.replicasets.write().await;
                let existed = replicasets
                    .get_mut(namespace)
                    .map(|ns_rs| ns_rs.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "ReplicaSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "ReplicaSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check for specific replicaset request
                if let (Some(namespace), Some(name)) = (query.get("namespace"), query.get("name")) {
                    let replicasets = state.replicasets.read().await;
                    if let Some(ns_rs) = replicasets.get(namespace) {
                        if let Some(rs) = ns_rs.get(name) {
                            let response = serde_json::json!({
                                "apiVersion": "apps/v1",
                                "kind": "ReplicaSet",
                                "metadata": {
                                    "name": rs.name,
                                    "namespace": rs.namespace,
                                    "creationTimestamp": rs.created_at.to_rfc3339(),
                                    "labels": rs.labels,
                                },
                                "spec": {
                                    "replicas": rs.spec.replicas,
                                    "selector": {
                                        "matchLabels": rs.spec.selector.match_labels,
                                        "matchExpressions": rs.spec.selector.match_expressions.iter().map(|e| {
                                            serde_json::json!({
                                                "key": e.key,
                                                "operator": e.operator,
                                                "values": e.values,
                                            })
                                        }).collect::<Vec<_>>(),
                                    },
                                },
                                "status": {
                                    "replicas": rs.spec.replicas.unwrap_or(0),
                                    "readyReplicas": 0,
                                    "availableReplicas": 0,
                                },
                            });
                            return Ok(ApiResponseOrStatus::Response(
                                serde_json::to_string(&response).unwrap(),
                            ));
                        }
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List replicasets
                let namespace = query.get("namespace").map(|s| s.as_str());
                let all_rs: Vec<ReplicaSetDesired> = if let Some(ns) = namespace {
                    let replicasets = state.replicasets.read().await;
                    replicasets
                        .get(ns)
                        .map(|ns_rs| ns_rs.values().cloned().collect())
                        .unwrap_or_default()
                } else {
                    let replicasets = state.replicasets.read().await;
                    replicasets
                        .values()
                        .flat_map(|ns_rs| ns_rs.values().cloned())
                        .collect()
                };

                // Apply pagination
                let pagination = PaginationParams::parse(query);
                let offset = pagination.offset();
                let total_count = all_rs.len();
                let paginated_items: Vec<ReplicaSetDesired> = all_rs
                    .into_iter()
                    .skip(offset)
                    .take(pagination.limit.unwrap_or(usize::MAX))
                    .collect();
                let paginated = PaginatedList::new(paginated_items.clone(), total_count, offset);

                let items: Vec<serde_json::Value> = paginated_items
                    .iter()
                    .map(|rs| {
                        serde_json::json!({
                            "apiVersion": "apps/v1",
                            "kind": "ReplicaSet",
                            "metadata": {
                                "name": rs.name,
                                "namespace": rs.namespace,
                                "creationTimestamp": rs.created_at.to_rfc3339(),
                                "labels": rs.labels,
                            },
                            "spec": {
                                "replicas": rs.spec.replicas,
                                "selector": {
                                    "matchLabels": rs.spec.selector.match_labels,
                                },
                            },
                            "status": {
                                "replicas": rs.spec.replicas.unwrap_or(0),
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ReplicaSetList",
                    "metadata": {
                        "resourceVersion": resource_version,
                        "continue": paginated.continue_token,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing replicaset
                let mut replicasets = state.replicasets.write().await;
                let existing = replicasets
                    .get_mut(namespace)
                    .and_then(|ns_rs| ns_rs.get_mut(name))
                    .cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut rs = existing.unwrap();
                drop(replicasets);

                // Apply patch
                if let Some(replicas) = patch["spec"]["replicas"].as_i64() {
                    rs.spec.replicas = Some(replicas as i32);
                }
                if let Some(labels) = patch["metadata"]["labels"].as_object() {
                    for (k, v) in labels {
                        if v.is_null() {
                            rs.labels.remove(k);
                        } else {
                            rs.labels
                                .insert(k.clone(), v.as_str().unwrap_or("").to_string());
                        }
                    }
                }

                // Update in memory
                {
                    let mut replicasets = state.replicasets.write().await;
                    if let Some(ns_rs) = replicasets.get_mut(namespace) {
                        if let Some(existing_rs) = ns_rs.get_mut(name) {
                            *existing_rs = rs.clone();
                        }
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "ReplicaSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "ReplicaSet".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "apps/v1",
                        "kind": "ReplicaSet",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct NetworkPolicyHandler;

#[async_trait]
impl ApiHandler for NetworkPolicyHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Parse label selector
                let _label_selector = query
                    .get("labelSelector")
                    .map(|s| FieldLabelSelector::parse(s))
                    .unwrap_or_default();

                let networkpolicies = state.networkpolicies.read().await;

                // Collect all network policies with their namespace
                let items: Vec<NetworkPolicyDesired> = networkpolicies
                    .iter()
                    .flat_map(|(_ns, ns_policies)| {
                        ns_policies.values().cloned().collect::<Vec<_>>()
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "networking.a3s.io/v1",
                    "kind": "NetworkPolicyList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let networkpolicies = state.networkpolicies.read().await;
                    if let Some(ns_nps) = networkpolicies.get(namespace) {
                        if ns_nps.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                let now = chrono::Utc::now();

                // Parse spec
                let pod_selector = request["spec"]["podSelector"]
                    .as_object()
                    .map(|obj| {
                        let match_labels = obj
                            .get("matchLabels")
                            .and_then(|v| v.as_object())
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let match_expressions = obj
                            .get("matchExpressions")
                            .and_then(|v| v.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|e| {
                                        Some(LabelSelectorExpression {
                                            key: e["key"].as_str()?.to_string(),
                                            operator: e["operator"].as_str()?.to_string(),
                                            values: e["values"]
                                                .as_array()
                                                .map(|v| {
                                                    v.iter()
                                                        .filter_map(|x| {
                                                            x.as_str().map(String::from)
                                                        })
                                                        .collect()
                                                })
                                                .unwrap_or_default(),
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        LabelSelector {
                            match_labels,
                            match_expressions,
                        }
                    })
                    .unwrap_or_default();

                let policy_types = request["spec"]["policyTypes"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|t| match t.as_str()? {
                                "Ingress" => Some(PolicyType::Ingress),
                                "Egress" => Some(PolicyType::Egress),
                                _ => None,
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let np = NetworkPolicyDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: NetworkPolicySpec {
                        pod_selector,
                        policy_types,
                        ingress: Vec::new(),
                        egress: Vec::new(),
                    },
                    created_at: now,
                    is_default_deny: request["spec"]["defaultDeny"].as_bool().unwrap_or(false),
                };

                // Get resource version
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let np_value = serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "NetworkPolicy",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": request["spec"],
                    });
                    store
                        .upsert("NetworkPolicy", namespace, name, &np_value)
                        .await?
                } else {
                    state.next_resource_version().await
                };

                // Store in memory
                {
                    let mut networkpolicies = state.networkpolicies.write().await;
                    networkpolicies
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_nps = networkpolicies.get_mut(namespace).unwrap();
                    ns_nps.insert(name.to_string(), np.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "NetworkPolicy",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                    }),
                    kind: "NetworkPolicy".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "networking.a3s.io/v1",
                    "kind": "NetworkPolicy",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                        "resourceVersion": resource_version,
                    },
                    "spec": request["spec"],
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Parse namespace and name from path
                let parts: Vec<&str> = _path.split('/').collect();
                let namespace = parts.get(3).copied().unwrap_or("default");
                let name = parts.get(5).copied().unwrap_or("");

                if name.is_empty() {
                    return Err(A3sError::Project("name is required".into()));
                }

                // Check if exists
                let exists = {
                    let networkpolicies = state.networkpolicies.read().await;
                    networkpolicies
                        .get(namespace)
                        .map(|ns_nps| ns_nps.contains_key(name))
                        .unwrap_or(false)
                };

                if !exists {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Delete from memory
                {
                    let mut networkpolicies = state.networkpolicies.write().await;
                    if let Some(ns_nps) = networkpolicies.get_mut(namespace) {
                        ns_nps.remove(name);
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "NetworkPolicy",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "NetworkPolicy".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "NetworkPolicy",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let _request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Parse namespace and name from path
                let parts: Vec<&str> = _path.split('/').collect();
                let namespace = parts.get(3).copied().unwrap_or("default");
                let name = parts.get(5).copied().unwrap_or("");

                if name.is_empty() {
                    return Err(A3sError::Project("name is required".into()));
                }

                // Check if exists
                let exists = {
                    let networkpolicies = state.networkpolicies.read().await;
                    networkpolicies
                        .get(namespace)
                        .map(|ns_nps| ns_nps.contains_key(name))
                        .unwrap_or(false)
                };

                if !exists {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // For now, just acknowledge the patch
                // Full JSON patch merging would be more complex
                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "networking.a3s.io/v1",
                        "kind": "NetworkPolicy",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct PodDisruptionBudgetHandler;

#[async_trait]
impl ApiHandler for PodDisruptionBudgetHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        _query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                let pdbs = state.poddisruptionbudgets.read().await;

                // Collect all PDBs with their namespace
                let items: Vec<PodDisruptionBudgetDesired> = pdbs
                    .iter()
                    .flat_map(|(_ns, ns_pdbs)| ns_pdbs.values().cloned().collect::<Vec<_>>())
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "policy/v1beta1",
                    "kind": "PodDisruptionBudgetList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                {
                    let pdbs = state.poddisruptionbudgets.read().await;
                    if let Some(ns_pdbs) = pdbs.get(namespace) {
                        if ns_pdbs.contains_key(name) {
                            return Ok(ApiResponseOrStatus::Conflict);
                        }
                    }
                }

                let now = chrono::Utc::now();

                // Parse spec - fields are Option<String> to support both absolute numbers and percentages
                let min_available = request["spec"]["minAvailable"].as_str().map(String::from);

                let max_disruptions = request["spec"]["maxUnavailable"].as_str().map(String::from);

                let pdb = PodDisruptionBudgetDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: PodDisruptionBudgetSpec {
                        min_available,
                        max_disruptions,
                    },
                    created_at: now,
                };

                // Get resource version
                let resource_version = if let Some(ref store) = state.sqlite_store {
                    let pdb_value = serde_json::json!({
                        "apiVersion": "policy/v1beta1",
                        "kind": "PodDisruptionBudget",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                        "spec": request["spec"],
                    });
                    store
                        .upsert("PodDisruptionBudget", namespace, name, &pdb_value)
                        .await?
                } else {
                    state.next_resource_version().await
                };

                // Store in memory
                {
                    let mut pdbs = state.poddisruptionbudgets.write().await;
                    pdbs.entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_pdbs = pdbs.get_mut(namespace).unwrap();
                    ns_pdbs.insert(name.to_string(), pdb.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "policy/v1beta1",
                        "kind": "PodDisruptionBudget",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                            "resourceVersion": resource_version,
                        },
                    }),
                    kind: "PodDisruptionBudget".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "policy/v1beta1",
                    "kind": "PodDisruptionBudget",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                        "resourceVersion": resource_version,
                    },
                    "spec": request["spec"],
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                // Parse namespace and name from path
                let parts: Vec<&str> = _path.split('/').collect();
                let namespace = parts.get(3).copied().unwrap_or("default");
                let name = parts.get(5).copied().unwrap_or("");

                if name.is_empty() {
                    return Err(A3sError::Project("name is required".into()));
                }

                // Check if exists
                let exists = {
                    let pdbs = state.poddisruptionbudgets.read().await;
                    pdbs.get(namespace)
                        .map(|ns_pdbs| ns_pdbs.contains_key(name))
                        .unwrap_or(false)
                };

                if !exists {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // Delete from memory
                {
                    let mut pdbs = state.poddisruptionbudgets.write().await;
                    if let Some(ns_pdbs) = pdbs.get_mut(namespace) {
                        ns_pdbs.remove(name);
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Deleted,
                    object: serde_json::json!({
                        "apiVersion": "policy/v1beta1",
                        "kind": "PodDisruptionBudget",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "PodDisruptionBudget".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "policy/v1beta1",
                        "kind": "PodDisruptionBudget",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let _request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Parse namespace and name from path
                let parts: Vec<&str> = _path.split('/').collect();
                let namespace = parts.get(3).copied().unwrap_or("default");
                let name = parts.get(5).copied().unwrap_or("");

                if name.is_empty() {
                    return Err(A3sError::Project("name is required".into()));
                }

                // Check if exists
                let exists = {
                    let pdbs = state.poddisruptionbudgets.read().await;
                    pdbs.get(namespace)
                        .map(|ns_pdbs| ns_pdbs.contains_key(name))
                        .unwrap_or(false)
                };

                if !exists {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "apiVersion": "policy/v1beta1",
                        "kind": "PodDisruptionBudget",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ResourceQuotaHandler;

#[async_trait]
impl ApiHandler for ResourceQuotaHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if namespaced request
                if let Some(namespace) = query.get("namespace") {
                    // Check if specific name requested
                    if let Some(name) = query.get("name") {
                        if let Some(quota) = state.resource_quotas.get_quota(namespace, name).await
                        {
                            let response = serde_json::json!({
                                "apiVersion": "v1",
                                "kind": "ResourceQuota",
                                "metadata": {
                                    "name": quota.name,
                                    "namespace": quota.namespace,
                                    "creationTimestamp": quota.created_at.to_rfc3339(),
                                },
                                "spec": quota.spec,
                                "status": quota.status,
                            });
                            return Ok(ApiResponseOrStatus::Response(
                                serde_json::to_string(&response).unwrap(),
                            ));
                        }
                        return Ok(ApiResponseOrStatus::NotFound);
                    }

                    let quotas = state.resource_quotas.list_namespace_quotas(namespace).await;
                    let items: Vec<serde_json::Value> = quotas
                        .iter()
                        .map(|q| {
                            serde_json::json!({
                                "apiVersion": "v1",
                                "kind": "ResourceQuota",
                                "metadata": {
                                    "name": q.name,
                                    "namespace": q.namespace,
                                    "creationTimestamp": q.created_at.to_rfc3339(),
                                },
                                "spec": q.spec,
                            })
                        })
                        .collect();

                    let response = serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ResourceQuotaList",
                        "metadata": {
                            "resourceVersion": resource_version,
                        },
                        "items": items,
                    });

                    return Ok(ApiResponseOrStatus::Response(
                        serde_json::to_string(&response).unwrap(),
                    ));
                }

                // List all quotas across namespaces
                let quotas = state.resource_quotas.list_quotas().await;
                let items: Vec<serde_json::Value> = quotas
                    .iter()
                    .map(|q| {
                        serde_json::json!({
                            "apiVersion": "v1",
                            "kind": "ResourceQuota",
                            "metadata": {
                                "name": q.name,
                                "namespace": q.namespace,
                                "creationTimestamp": q.created_at.to_rfc3339(),
                            },
                            "spec": q.spec,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ResourceQuotaList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                if state
                    .resource_quotas
                    .get_quota(namespace, name)
                    .await
                    .is_some()
                {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let now = chrono::Utc::now();

                // Parse spec.hard
                let hard: std::collections::HashMap<String, i64> = request["spec"]["hard"]
                    .as_object()
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_i64().unwrap_or(0)))
                            .collect()
                    })
                    .unwrap_or_default();

                let quota = crate::state::resourcequota::ResourceQuotaDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::resourcequota::ResourceQuotaSpec {
                        hard,
                        scope_selector: None,
                        scopes: vec![],
                    },
                    status: None,
                    created_at: now,
                };

                // Store via controller
                state.resource_quotas.set_quota(quota.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "ResourceQuota",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "ResourceQuota".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ResourceQuota",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": request["spec"],
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace is required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                state.resource_quotas.delete_quota(namespace, name).await?;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct IngressClassHandler;

#[async_trait]
impl ApiHandler for IngressClassHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if specific name requested
                if let Some(name) = query.get("name") {
                    let ingressclasses = state.ingressclasses.read().await;
                    if let Some(ic) = ingressclasses.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "networking.k8s.io/v1",
                            "kind": "IngressClass",
                            "metadata": {
                                "name": ic.name,
                                "creationTimestamp": ic.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "controller": ic.controller,
                                "parameters": ic.parameters,
                            },
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List all ingressclasses
                let ingressclasses = state.ingressclasses.read().await;
                let items: Vec<serde_json::Value> = ingressclasses
                    .values()
                    .map(|ic| {
                        serde_json::json!({
                            "apiVersion": "networking.k8s.io/v1",
                            "kind": "IngressClass",
                            "metadata": {
                                "name": ic.name,
                                "creationTimestamp": ic.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "controller": ic.controller,
                                "parameters": ic.parameters,
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "networking.k8s.io/v1",
                    "kind": "IngressClassList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let controller = request["spec"]["controller"]
                    .as_str()
                    .unwrap_or("traefik.io/ingress-controller");

                let now = chrono::Utc::now();

                let ic = IngressClassDesired {
                    name: name.to_string(),
                    controller: controller.to_string(),
                    parameters: None,
                    is_default: request["metadata"]["annotations"]
                        .get("ingressclass.kubernetes.io/is-default-class")
                        .and_then(|v| v.as_bool())
                        .unwrap_or(false),
                    created_at: now,
                };

                // Store
                {
                    let mut ingressclasses = state.ingressclasses.write().await;
                    ingressclasses.insert(name.to_string(), ic.clone());
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "networking.k8s.io/v1",
                        "kind": "IngressClass",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "IngressClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "networking.k8s.io/v1",
                    "kind": "IngressClass",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": {
                        "controller": controller,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut ingressclasses = state.ingressclasses.write().await;
                if ingressclasses.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct LeaseHandler;

#[async_trait]
impl ApiHandler for LeaseHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if namespaced request
                if let Some(namespace) = query.get("namespace") {
                    // Check if specific name requested
                    if let Some(name) = query.get("name") {
                        let leases = state.leases.read().await;
                        if let Some(ns_leases) = leases.get(namespace) {
                            if let Some(lease) = ns_leases.get(name) {
                                let response = serde_json::json!({
                                    "apiVersion": "coordination.k8s.io/v1",
                                    "kind": "Lease",
                                    "metadata": {
                                        "name": lease.name,
                                        "namespace": lease.namespace,
                                        "creationTimestamp": lease.created_at.to_rfc3339(),
                                    },
                                    "spec": {
                                        "holderIdentity": lease.spec.holder_identity,
                                        "leaseDurationSeconds": lease.spec.lease_duration_seconds,
                                        "acquireTime": lease.spec.acquire_time.map(|t| t.to_rfc3339()),
                                        "renewTime": lease.spec.renew_time.map(|t| t.to_rfc3339()),
                                        "leaderTransitions": lease.spec.lease_transitions,
                                    },
                                });
                                return Ok(ApiResponseOrStatus::Response(
                                    serde_json::to_string(&response).unwrap(),
                                ));
                            }
                        }
                        return Ok(ApiResponseOrStatus::NotFound);
                    }

                    // List leases in namespace
                    let leases = state.leases.read().await;
                    let items: Vec<serde_json::Value> = if let Some(ns_leases) =
                        leases.get(namespace)
                    {
                        ns_leases
                            .values()
                            .map(|lease| {
                                serde_json::json!({
                                    "apiVersion": "coordination.k8s.io/v1",
                                    "kind": "Lease",
                                    "metadata": {
                                        "name": lease.name,
                                        "namespace": lease.namespace,
                                        "creationTimestamp": lease.created_at.to_rfc3339(),
                                    },
                                    "spec": {
                                        "holderIdentity": lease.spec.holder_identity,
                                        "leaseDurationSeconds": lease.spec.lease_duration_seconds,
                                        "acquireTime": lease.spec.acquire_time.map(|t| t.to_rfc3339()),
                                        "renewTime": lease.spec.renew_time.map(|t| t.to_rfc3339()),
                                        "leaderTransitions": lease.spec.lease_transitions,
                                    },
                                })
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    let response = serde_json::json!({
                        "apiVersion": "coordination.k8s.io/v1",
                        "kind": "LeaseList",
                        "metadata": {
                            "resourceVersion": resource_version,
                        },
                        "items": items,
                    });

                    return Ok(ApiResponseOrStatus::Response(
                        serde_json::to_string(&response).unwrap(),
                    ));
                }

                // List all leases across namespaces
                let leases = state.leases.read().await;
                let items: Vec<serde_json::Value> = leases
                    .values()
                    .flat_map(|ns_leases| {
                        ns_leases.values().map(|lease| {
                            serde_json::json!({
                                "apiVersion": "coordination.k8s.io/v1",
                                "kind": "Lease",
                                "metadata": {
                                    "name": lease.name,
                                    "namespace": lease.namespace,
                                    "creationTimestamp": lease.created_at.to_rfc3339(),
                                },
                                "spec": {
                                    "holderIdentity": lease.spec.holder_identity,
                                    "leaseDurationSeconds": lease.spec.lease_duration_seconds,
                                    "acquireTime": lease.spec.acquire_time.map(|t| t.to_rfc3339()),
                                    "renewTime": lease.spec.renew_time.map(|t| t.to_rfc3339()),
                                    "leaderTransitions": lease.spec.lease_transitions,
                                },
                            })
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "coordination.k8s.io/v1",
                    "kind": "LeaseList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;
                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                // Extract spec values before moving
                let holder_identity = request["spec"]["holderIdentity"].as_str().map(String::from);
                let lease_duration_seconds = request["spec"]["leaseDurationSeconds"]
                    .as_i64()
                    .unwrap_or(15) as i32;

                let lease = LeaseDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::LeaseSpec {
                        holder_identity: holder_identity.clone(),
                        lease_duration_seconds,
                        acquire_time: request["spec"]["acquireTime"]
                            .as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc)),
                        renew_time: request["spec"]["renewTime"]
                            .as_str()
                            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
                            .map(|dt| dt.with_timezone(&chrono::Utc)),
                        last_update_time: Some(now),
                        lease_transitions: request["spec"]["leaderTransitions"]
                            .as_i64()
                            .unwrap_or(0) as i32,
                    },
                    created_at: now,
                };

                // Store
                {
                    let mut leases = state.leases.write().await;
                    leases
                        .entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_leases = leases.get_mut(namespace).unwrap();
                    ns_leases.insert(name.to_string(), lease);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "coordination.k8s.io/v1",
                        "kind": "Lease",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "Lease".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "coordination.k8s.io/v1",
                    "kind": "Lease",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": {
                        "holderIdentity": holder_identity,
                        "leaseDurationSeconds": lease_duration_seconds,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let mut leases = state.leases.write().await;
                let existed = leases
                    .get_mut(namespace)
                    .map(|ns_leases| ns_leases.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing lease
                let leases = state.leases.read().await;
                let existing = leases
                    .get(namespace)
                    .and_then(|ns_leases| ns_leases.get(name))
                    .cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut lease = existing.unwrap();
                drop(leases);

                // Apply patch
                if let Some(holder) = patch["spec"]["holderIdentity"].as_str() {
                    lease.spec.holder_identity = Some(holder.to_string());
                    lease.spec.renew_time = Some(chrono::Utc::now());
                }
                if let Some(duration) = patch["spec"]["leaseDurationSeconds"].as_i64() {
                    lease.spec.lease_duration_seconds = duration as i32;
                }
                if let Some(transitions) = patch["spec"]["leaderTransitions"].as_i64() {
                    lease.spec.lease_transitions = transitions as i32;
                }

                // Update in memory
                {
                    let mut leases = state.leases.write().await;
                    if let Some(ns_leases) = leases.get_mut(namespace) {
                        if let Some(existing_lease) = ns_leases.get_mut(name) {
                            *existing_lease = lease.clone();
                        }
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "coordination.k8s.io/v1",
                        "kind": "Lease",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "Lease".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "coordination.k8s.io/v1",
                    "kind": "Lease",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                    },
                    "spec": {
                        "holderIdentity": lease.spec.holder_identity,
                        "leaseDurationSeconds": lease.spec.lease_duration_seconds,
                        "renewTime": lease.spec.renew_time.map(|t| t.to_rfc3339()),
                        "leaderTransitions": lease.spec.lease_transitions,
                    },
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct CSIDriverHandler;

#[async_trait]
impl ApiHandler for CSIDriverHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if specific name requested
                if let Some(name) = query.get("name") {
                    let csi_drivers = state.csi_drivers.read().await;
                    if let Some(driver) = csi_drivers.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "storage.k8s.io/v1",
                            "kind": "CSIDriver",
                            "metadata": {
                                "name": driver.name,
                                "creationTimestamp": driver.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "driver": driver.spec.driver,
                                "attachRequired": driver.spec.attach_required,
                                "podInfoOnMount": driver.spec.pod_info_on_mount,
                                "mountOptions": driver.spec.mount_options,
                                "volumeLifecycleModes": driver.spec.volume_lifecycle_modes,
                            },
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List all CSIDrivers
                let csi_drivers = state.csi_drivers.read().await;
                let items: Vec<serde_json::Value> = csi_drivers
                    .values()
                    .map(|driver| {
                        serde_json::json!({
                            "apiVersion": "storage.k8s.io/v1",
                            "kind": "CSIDriver",
                            "metadata": {
                                "name": driver.name,
                                "creationTimestamp": driver.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "driver": driver.spec.driver,
                                "attachRequired": driver.spec.attach_required,
                                "podInfoOnMount": driver.spec.pod_info_on_mount,
                                "mountOptions": driver.spec.mount_options,
                                "volumeLifecycleModes": driver.spec.volume_lifecycle_modes,
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "CSIDriverList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let driver = request["spec"]["driver"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("spec.driver is required".into()))?;

                let now = chrono::Utc::now();

                let csi_driver = CSIDriverDesired {
                    name: name.to_string(),
                    spec: crate::state::CSIDriverSpec {
                        driver: driver.to_string(),
                        attach_required: request["spec"]["attachRequired"]
                            .as_bool()
                            .unwrap_or(false),
                        pod_info_on_mount: request["spec"]["podInfoOnMount"]
                            .as_bool()
                            .unwrap_or(false),
                        mount_options: request["spec"]["mountOptions"]
                            .as_array()
                            .map(|opts| {
                                opts.iter()
                                    .filter_map(|o| o.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        volume_lifecycle_modes: request["spec"]["volumeLifecycleModes"]
                            .as_array()
                            .map(|modes| {
                                modes
                                    .iter()
                                    .filter_map(|m| m.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                    },
                    created_at: now,
                };

                // Store
                {
                    let mut csi_drivers = state.csi_drivers.write().await;
                    csi_drivers.insert(name.to_string(), csi_driver);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "storage.k8s.io/v1",
                        "kind": "CSIDriver",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "CSIDriver".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "CSIDriver",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": {
                        "driver": driver,
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut csi_drivers = state.csi_drivers.write().await;
                if csi_drivers.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing driver
                let csi_drivers = state.csi_drivers.read().await;
                let existing = csi_drivers.get(name).cloned();

                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut driver = existing.unwrap();
                drop(csi_drivers);

                // Apply patch to spec
                if let Some(driver_name) = patch["spec"]["driver"].as_str() {
                    driver.spec.driver = driver_name.to_string();
                }
                if let Some(attach) = patch["spec"]["attachRequired"].as_bool() {
                    driver.spec.attach_required = attach;
                }
                if let Some(pod_info) = patch["spec"]["podInfoOnMount"].as_bool() {
                    driver.spec.pod_info_on_mount = pod_info;
                }

                // Update in memory
                {
                    let mut csi_drivers = state.csi_drivers.write().await;
                    if let Some(existing_driver) = csi_drivers.get_mut(name) {
                        *existing_driver = driver.clone();
                    }
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "storage.k8s.io/v1",
                        "kind": "CSIDriver",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "CSIDriver".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "CSIDriver",
                    "metadata": {
                        "name": name,
                    },
                    "spec": {
                        "driver": driver.spec.driver,
                        "attachRequired": driver.spec.attach_required,
                        "podInfoOnMount": driver.spec.pod_info_on_mount,
                    },
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct RuntimeClassHandler;

#[async_trait]
impl ApiHandler for RuntimeClassHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if specific name requested
                if let Some(name) = query.get("name") {
                    if let Some(rc) = state.runtime_classes.get(name).await {
                        let response = serde_json::json!({
                            "apiVersion": "node.k8s.io/v1",
                            "kind": "RuntimeClass",
                            "metadata": {
                                "name": rc.metadata.name,
                                "creationTimestamp": rc.metadata.created_at.to_rfc3339(),
                            },
                            "handler": rc.spec.handler,
                            "overhead": rc.spec.overhead,
                            "scheduling": rc.spec.scheduling,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                // List all RuntimeClasses
                let classes = state.runtime_classes.list().await;
                let items: Vec<serde_json::Value> = classes
                    .iter()
                    .map(|rc| {
                        serde_json::json!({
                            "apiVersion": "node.k8s.io/v1",
                            "kind": "RuntimeClass",
                            "metadata": {
                                "name": rc.metadata.name,
                                "creationTimestamp": rc.metadata.created_at.to_rfc3339(),
                            },
                            "handler": rc.spec.handler,
                            "overhead": rc.spec.overhead,
                            "scheduling": rc.spec.scheduling,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "node.k8s.io/v1",
                    "kind": "RuntimeClassList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let handler = request["handler"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("handler is required".into()))?;

                let now = chrono::Utc::now();

                let rc = RuntimeClass {
                    metadata: crate::state::runtimeclass::RuntimeClassMeta {
                        name: name.to_string(),
                        labels: request["metadata"]["labels"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        annotations: request["metadata"]["annotations"]
                            .as_object()
                            .map(|m| {
                                m.iter()
                                    .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        created_at: now,
                    },
                    spec: crate::state::runtimeclass::RuntimeClassSpec {
                        handler: handler.to_string(),
                        overhead: None,
                        scheduling: None,
                    },
                };

                state.runtime_classes.set(rc.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "node.k8s.io/v1",
                        "kind": "RuntimeClass",
                        "metadata": {
                            "name": name,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "RuntimeClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "node.k8s.io/v1",
                    "kind": "RuntimeClass",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "handler": handler,
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                state.runtime_classes.delete(name).await?;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let patch: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Get existing class
                let existing = state.runtime_classes.get(name).await;
                if existing.is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let mut rc = existing.unwrap();

                // Apply patch
                if let Some(handler) = patch["handler"].as_str() {
                    rc.spec.handler = handler.to_string();
                }

                state.runtime_classes.set(rc.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Modified,
                    object: serde_json::json!({
                        "apiVersion": "node.k8s.io/v1",
                        "kind": "RuntimeClass",
                        "metadata": {
                            "name": name,
                        },
                    }),
                    kind: "RuntimeClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "node.k8s.io/v1",
                    "kind": "RuntimeClass",
                    "metadata": {
                        "name": name,
                    },
                    "handler": rc.spec.handler,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct CSINodeHandler;

#[async_trait]
impl ApiHandler for CSINodeHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let csi_nodes = state.csi_nodes.read().await;
                    if let Some(node) = csi_nodes.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "storage.k8s.io/v1",
                            "kind": "CSINode",
                            "metadata": {
                                "name": node.name,
                                "creationTimestamp": node.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "drivers": node.spec.drivers,
                            },
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let csi_nodes = state.csi_nodes.read().await;
                let items: Vec<serde_json::Value> = csi_nodes
                    .values()
                    .map(|node| {
                        serde_json::json!({
                            "apiVersion": "storage.k8s.io/v1",
                            "kind": "CSINode",
                            "metadata": {
                                "name": node.name,
                                "creationTimestamp": node.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "drivers": node.spec.drivers,
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "CSINodeList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let drivers: Vec<crate::state::CSINodeDriver> = request["spec"]["drivers"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|d| crate::state::CSINodeDriver {
                                name: d["name"].as_str().unwrap_or("").to_string(),
                                address: d["address"].as_str().unwrap_or("").to_string(),
                                topology_keys: d["topologyKeys"]
                                    .as_array()
                                    .map(|keys| {
                                        keys.iter()
                                            .filter_map(|k| k.as_str().map(String::from))
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let csi_node = CSINodeDesired {
                    name: name.to_string(),
                    spec: crate::state::CSINodeSpec { drivers },
                    created_at: now,
                };

                {
                    let mut csi_nodes = state.csi_nodes.write().await;
                    csi_nodes.insert(name.to_string(), csi_node);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "storage.k8s.io/v1",
                        "kind": "CSINode",
                        "metadata": { "name": name },
                    }),
                    kind: "CSINode".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "CSINode",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut csi_nodes = state.csi_nodes.write().await;
                if csi_nodes.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut csi_nodes = state.csi_nodes.write().await;
                if !csi_nodes.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "CSINode",
                        "apiVersion": "storage.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct VolumeAttachmentHandler;

#[async_trait]
impl ApiHandler for VolumeAttachmentHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let attachments = state.volume_attachments.read().await;
                    if let Some(va) = attachments.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "storage.k8s.io/v1",
                            "kind": "VolumeAttachment",
                            "metadata": {
                                "name": va.name,
                                "creationTimestamp": va.created_at.to_rfc3339(),
                            },
                            "spec": va.spec,
                            "status": va.status,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let attachments = state.volume_attachments.read().await;
                let items: Vec<serde_json::Value> = attachments
                    .values()
                    .map(|va| {
                        serde_json::json!({
                            "apiVersion": "storage.k8s.io/v1",
                            "kind": "VolumeAttachment",
                            "metadata": {
                                "name": va.name,
                                "creationTimestamp": va.created_at.to_rfc3339(),
                            },
                            "spec": va.spec,
                            "status": va.status,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "VolumeAttachmentList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let va = VolumeAttachmentDesired {
                    name: name.to_string(),
                    spec: VolumeAttachmentSpec {
                        volume: request["spec"]["volume"].as_str().unwrap_or("").to_string(),
                        node_name: request["spec"]["nodeName"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        attacher: request["spec"]["attacher"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        source: None,
                    },
                    status: Some(VolumeAttachmentStatus {
                        attached: true,
                        attachment_metadata: std::collections::HashMap::new(),
                        error: None,
                    }),
                    created_at: now,
                };

                {
                    let mut attachments = state.volume_attachments.write().await;
                    attachments.insert(name.to_string(), va);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "storage.k8s.io/v1",
                        "kind": "VolumeAttachment",
                        "metadata": { "name": name },
                    }),
                    kind: "VolumeAttachment".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "storage.k8s.io/v1",
                    "kind": "VolumeAttachment",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut attachments = state.volume_attachments.write().await;
                if attachments.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut attachments = state.volume_attachments.write().await;
                if !attachments.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "VolumeAttachment",
                        "apiVersion": "storage.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct FlowSchemaHandler;

#[async_trait]
impl ApiHandler for FlowSchemaHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let schemas = state.flow_schemas.read().await;
                    if let Some(fs) = schemas.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                            "kind": "FlowSchema",
                            "metadata": {
                                "name": fs.name,
                                "creationTimestamp": fs.created_at.to_rfc3339(),
                            },
                            "spec": fs.spec,
                            "status": fs.status,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let schemas = state.flow_schemas.read().await;
                let items: Vec<serde_json::Value> = schemas
                    .values()
                    .map(|fs| {
                        serde_json::json!({
                            "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                            "kind": "FlowSchema",
                            "metadata": {
                                "name": fs.name,
                                "creationTimestamp": fs.created_at.to_rfc3339(),
                            },
                            "spec": fs.spec,
                            "status": fs.status,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                    "kind": "FlowSchemaList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let fs = FlowSchemaDesired {
                    name: name.to_string(),
                    spec: FlowSchemaSpec {
                        priority_level: request["spec"]["priorityLevel"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        rules: vec![],
                    },
                    status: None,
                    created_at: now,
                };

                {
                    let mut schemas = state.flow_schemas.write().await;
                    schemas.insert(name.to_string(), fs);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                        "kind": "FlowSchema",
                        "metadata": { "name": name },
                    }),
                    kind: "FlowSchema".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                    "kind": "FlowSchema",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut schemas = state.flow_schemas.write().await;
                if schemas.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut schemas = state.flow_schemas.write().await;
                if !schemas.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "FlowSchema",
                        "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct PriorityLevelHandler;

#[async_trait]
impl ApiHandler for PriorityLevelHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let pls = state.priority_level_configurations.read().await;
                    if let Some(pl) = pls.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                            "kind": "PriorityLevelConfiguration",
                            "metadata": {
                                "name": pl.name,
                                "creationTimestamp": pl.created_at.to_rfc3339(),
                            },
                            "spec": pl.spec,
                            "status": pl.status,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let pls = state.priority_level_configurations.read().await;
                let items: Vec<serde_json::Value> = pls
                    .values()
                    .map(|pl| {
                        serde_json::json!({
                            "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                            "kind": "PriorityLevelConfiguration",
                            "metadata": {
                                "name": pl.name,
                                "creationTimestamp": pl.created_at.to_rfc3339(),
                            },
                            "spec": pl.spec,
                            "status": pl.status,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                    "kind": "PriorityLevelConfigurationList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let pl = PriorityLevelConfigurationDesired {
                    name: name.to_string(),
                    spec: PriorityLevelConfigurationSpec {
                        r#type: request["spec"]["type"]
                            .as_str()
                            .unwrap_or("Limited")
                            .to_string(),
                        limited: None,
                    },
                    status: None,
                    created_at: now,
                };

                {
                    let mut pls = state.priority_level_configurations.write().await;
                    pls.insert(name.to_string(), pl);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                        "kind": "PriorityLevelConfiguration",
                        "metadata": { "name": name },
                    }),
                    kind: "PriorityLevelConfiguration".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                    "kind": "PriorityLevelConfiguration",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut pls = state.priority_level_configurations.write().await;
                if pls.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut pls = state.priority_level_configurations.write().await;
                if !pls.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "PriorityLevelConfiguration",
                        "apiVersion": "flowcontrol.apiserver.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct CSRHandler;

#[async_trait]
impl ApiHandler for CSRHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let csrs = state.certificate_signing_requests.read().await;
                    if let Some(csr) = csrs.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "certificates.k8s.io/v1",
                            "kind": "CertificateSigningRequest",
                            "metadata": {
                                "name": csr.name,
                                "creationTimestamp": csr.created_at.to_rfc3339(),
                            },
                            "spec": csr.spec,
                            "status": csr.status,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let csrs = state.certificate_signing_requests.read().await;
                let items: Vec<serde_json::Value> = csrs
                    .values()
                    .map(|csr| {
                        serde_json::json!({
                            "apiVersion": "certificates.k8s.io/v1",
                            "kind": "CertificateSigningRequest",
                            "metadata": {
                                "name": csr.name,
                                "creationTimestamp": csr.created_at.to_rfc3339(),
                            },
                            "spec": csr.spec,
                            "status": csr.status,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "certificates.k8s.io/v1",
                    "kind": "CertificateSigningRequestList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let csr = CertificateSigningRequestDesired {
                    name: name.to_string(),
                    spec: CertificateSigningRequestSpec {
                        signer_name: request["spec"]["signerName"]
                            .as_str()
                            .unwrap_or("kubernetes.io/kube-apiserver-client")
                            .to_string(),
                        request: request["spec"]["request"]
                            .as_str()
                            .unwrap_or("")
                            .to_string(),
                        usages: request["spec"]["usages"]
                            .as_array()
                            .map(|u| {
                                u.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default(),
                        credentials: None,
                    },
                    status: None,
                    created_at: now,
                };

                {
                    let mut csrs = state.certificate_signing_requests.write().await;
                    csrs.insert(name.to_string(), csr);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "certificates.k8s.io/v1",
                        "kind": "CertificateSigningRequest",
                        "metadata": { "name": name },
                    }),
                    kind: "CertificateSigningRequest".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "certificates.k8s.io/v1",
                    "kind": "CertificateSigningRequest",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut csrs = state.certificate_signing_requests.write().await;
                if csrs.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut csrs = state.certificate_signing_requests.write().await;
                if !csrs.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "CertificateSigningRequest",
                        "apiVersion": "certificates.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct MutatingWebhookConfigHandler;

#[async_trait]
impl ApiHandler for MutatingWebhookConfigHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let configs = state.mutating_webhook_configurations.read().await;
                    if let Some(config) = configs.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "admissionregistration.k8s.io/v1",
                            "kind": "MutatingWebhookConfiguration",
                            "metadata": {
                                "name": config.name,
                                "creationTimestamp": config.created_at.to_rfc3339(),
                            },
                            "webhooks": config.spec.webhooks,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let configs = state.mutating_webhook_configurations.read().await;
                let items: Vec<serde_json::Value> = configs
                    .values()
                    .map(|config| {
                        serde_json::json!({
                            "apiVersion": "admissionregistration.k8s.io/v1",
                            "kind": "MutatingWebhookConfiguration",
                            "metadata": {
                                "name": config.name,
                                "creationTimestamp": config.created_at.to_rfc3339(),
                            },
                            "webhooks": config.spec.webhooks,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "admissionregistration.k8s.io/v1",
                    "kind": "MutatingWebhookConfigurationList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let config = MutatingWebhookConfigurationDesired {
                    name: name.to_string(),
                    spec: MutatingWebhookConfigurationSpec { webhooks: vec![] },
                    created_at: now,
                };

                {
                    let mut configs = state.mutating_webhook_configurations.write().await;
                    configs.insert(name.to_string(), config);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "admissionregistration.k8s.io/v1",
                        "kind": "MutatingWebhookConfiguration",
                        "metadata": { "name": name },
                    }),
                    kind: "MutatingWebhookConfiguration".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "admissionregistration.k8s.io/v1",
                    "kind": "MutatingWebhookConfiguration",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut configs = state.mutating_webhook_configurations.write().await;
                if configs.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut configs = state.mutating_webhook_configurations.write().await;
                if !configs.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "MutatingWebhookConfiguration",
                        "apiVersion": "admissionregistration.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ValidatingWebhookConfigHandler;

#[async_trait]
impl ApiHandler for ValidatingWebhookConfigHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let configs = state.validating_webhook_configurations.read().await;
                    if let Some(config) = configs.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "admissionregistration.k8s.io/v1",
                            "kind": "ValidatingWebhookConfiguration",
                            "metadata": {
                                "name": config.name,
                                "creationTimestamp": config.created_at.to_rfc3339(),
                            },
                            "webhooks": config.spec.webhooks,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let configs = state.validating_webhook_configurations.read().await;
                let items: Vec<serde_json::Value> = configs
                    .values()
                    .map(|config| {
                        serde_json::json!({
                            "apiVersion": "admissionregistration.k8s.io/v1",
                            "kind": "ValidatingWebhookConfiguration",
                            "metadata": {
                                "name": config.name,
                                "creationTimestamp": config.created_at.to_rfc3339(),
                            },
                            "webhooks": config.spec.webhooks,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "admissionregistration.k8s.io/v1",
                    "kind": "ValidatingWebhookConfigurationList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let config = ValidatingWebhookConfigurationDesired {
                    name: name.to_string(),
                    spec: ValidatingWebhookConfigurationSpec { webhooks: vec![] },
                    created_at: now,
                };

                {
                    let mut configs = state.validating_webhook_configurations.write().await;
                    configs.insert(name.to_string(), config);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "admissionregistration.k8s.io/v1",
                        "kind": "ValidatingWebhookConfiguration",
                        "metadata": { "name": name },
                    }),
                    kind: "ValidatingWebhookConfiguration".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "admissionregistration.k8s.io/v1",
                    "kind": "ValidatingWebhookConfiguration",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut configs = state.validating_webhook_configurations.write().await;
                if configs.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut configs = state.validating_webhook_configurations.write().await;
                if !configs.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "ValidatingWebhookConfiguration",
                        "apiVersion": "admissionregistration.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct HPAHandler;

#[async_trait]
impl ApiHandler for HPAHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if namespaced request
                if let Some(namespace) = query.get("namespace") {
                    // Check if specific name requested
                    if let Some(name) = query.get("name") {
                        let hpas = state.hpas.read().await;
                        if let Some(ns_hpas) = hpas.get(namespace) {
                            if let Some(hpa) = ns_hpas.get(name) {
                                let response = serde_json::json!({
                                    "apiVersion": "autoscaling/v1",
                                    "kind": "HorizontalPodAutoscaler",
                                    "metadata": {
                                        "name": hpa.name,
                                        "namespace": hpa.namespace,
                                        "creationTimestamp": hpa.created_at.to_rfc3339(),
                                    },
                                    "spec": hpa.spec,
                                    "status": hpa.status,
                                });
                                return Ok(ApiResponseOrStatus::Response(
                                    serde_json::to_string(&response).unwrap(),
                                ));
                            }
                        }
                        return Ok(ApiResponseOrStatus::NotFound);
                    }

                    // List HPAs in namespace
                    let hpas = state.hpas.read().await;
                    let items: Vec<serde_json::Value> = if let Some(ns_hpas) = hpas.get(namespace) {
                        ns_hpas
                            .values()
                            .map(|hpa| {
                                serde_json::json!({
                                    "apiVersion": "autoscaling/v1",
                                    "kind": "HorizontalPodAutoscaler",
                                    "metadata": {
                                        "name": hpa.name,
                                        "namespace": hpa.namespace,
                                        "creationTimestamp": hpa.created_at.to_rfc3339(),
                                    },
                                    "spec": hpa.spec,
                                    "status": hpa.status,
                                })
                            })
                            .collect()
                    } else {
                        vec![]
                    };

                    let response = serde_json::json!({
                        "apiVersion": "autoscaling/v1",
                        "kind": "HorizontalPodAutoscalerList",
                        "metadata": {
                            "resourceVersion": resource_version,
                        },
                        "items": items,
                    });

                    return Ok(ApiResponseOrStatus::Response(
                        serde_json::to_string(&response).unwrap(),
                    ));
                }

                // List all HPAs across namespaces
                let hpas = state.hpas.read().await;
                let items: Vec<serde_json::Value> = hpas
                    .values()
                    .flat_map(|ns_hpas| {
                        ns_hpas.values().map(|hpa| {
                            serde_json::json!({
                                "apiVersion": "autoscaling/v1",
                                "kind": "HorizontalPodAutoscaler",
                                "metadata": {
                                    "name": hpa.name,
                                    "namespace": hpa.namespace,
                                    "creationTimestamp": hpa.created_at.to_rfc3339(),
                                },
                                "spec": hpa.spec,
                                "status": hpa.status,
                            })
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "autoscaling/v1",
                    "kind": "HorizontalPodAutoscalerList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;
                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let hpa = HPADesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::hpa::HPASpec {
                        scale_target_ref: crate::state::hpa::ScaleTargetRef {
                            api_version: request["spec"]["scaleTargetRef"]["apiVersion"]
                                .as_str()
                                .map(String::from),
                            kind: request["spec"]["scaleTargetRef"]["kind"]
                                .as_str()
                                .unwrap_or("Deployment")
                                .to_string(),
                            name: request["spec"]["scaleTargetRef"]["name"]
                                .as_str()
                                .unwrap_or("")
                                .to_string(),
                        },
                        min_replicas: request["spec"]["minReplicas"].as_i64().map(|v| v as i32),
                        max_replicas: request["spec"]["maxReplicas"].as_i64().unwrap_or(10) as i32,
                        metrics: vec![],
                        behavior: None,
                    },
                    status: None,
                    created_at: now,
                };

                // Store
                {
                    let mut hpas = state.hpas.write().await;
                    hpas.entry(namespace.to_string())
                        .or_insert_with(HashMap::new);
                    let ns_hpas = hpas.get_mut(namespace).unwrap();
                    ns_hpas.insert(name.to_string(), hpa);
                }

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "autoscaling/v1",
                        "kind": "HorizontalPodAutoscaler",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                        },
                    }),
                    kind: "HorizontalPodAutoscaler".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "autoscaling/v1",
                    "kind": "HorizontalPodAutoscaler",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let mut hpas = state.hpas.write().await;
                let existed = hpas
                    .get_mut(namespace)
                    .map(|ns_hpas| ns_hpas.remove(name).is_some())
                    .unwrap_or(false);

                if !existed {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name required".into()))?;

                let mut hpas = state.hpas.write().await;
                if !hpas
                    .get(namespace)
                    .map(|ns| ns.contains_key(name))
                    .unwrap_or(false)
                {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "HorizontalPodAutoscaler",
                        "apiVersion": "autoscaling/v1",
                        "metadata": { "name": name, "namespace": namespace },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct PriorityClassHandler;

#[async_trait]
impl ApiHandler for PriorityClassHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let pcs = state.priority_classes.read().await;
                    if let Some(pc) = pcs.get(name) {
                        let response = serde_json::json!({
                            "apiVersion": "scheduling.k8s.io/v1",
                            "kind": "PriorityClass",
                            "metadata": {
                                "name": pc.name,
                                "creationTimestamp": pc.created_at.to_rfc3339(),
                            },
                            "value": pc.spec.value,
                            "globalDefault": pc.spec.global_default,
                            "description": pc.spec.description,
                            "preemptionPolicy": pc.spec.preemption_policy,
                        });
                        return Ok(ApiResponseOrStatus::Response(
                            serde_json::to_string(&response).unwrap(),
                        ));
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let pcs = state.priority_classes.read().await;
                let items: Vec<serde_json::Value> = pcs
                    .values()
                    .map(|pc| {
                        serde_json::json!({
                            "apiVersion": "scheduling.k8s.io/v1",
                            "kind": "PriorityClass",
                            "metadata": {
                                "name": pc.name,
                                "creationTimestamp": pc.created_at.to_rfc3339(),
                            },
                            "value": pc.spec.value,
                            "globalDefault": pc.spec.global_default,
                            "description": pc.spec.description,
                            "preemptionPolicy": pc.spec.preemption_policy,
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "scheduling.k8s.io/v1",
                    "kind": "PriorityClassList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;

                let now = chrono::Utc::now();

                let pc = PriorityClassDesired {
                    name: name.to_string(),
                    spec: PriorityClassSpec {
                        value: request["value"].as_i64().unwrap_or(0) as i32,
                        global_default: request["globalDefault"].as_bool().unwrap_or(false),
                        description: request["description"].as_str().unwrap_or("").to_string(),
                        preemption_policy: request["preemptionPolicy"]
                            .as_str()
                            .unwrap_or("PreemptLowerPriority")
                            .to_string(),
                    },
                    created_at: now,
                };

                {
                    let mut pcs = state.priority_classes.write().await;
                    pcs.insert(name.to_string(), pc);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "scheduling.k8s.io/v1",
                        "kind": "PriorityClass",
                        "metadata": { "name": name },
                    }),
                    kind: "PriorityClass".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "scheduling.k8s.io/v1",
                    "kind": "PriorityClass",
                    "metadata": {
                        "name": name,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut pcs = state.priority_classes.write().await;
                if pcs.remove(name).is_none() {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                let mut pcs = state.priority_classes.write().await;
                if !pcs.contains_key(name) {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "PriorityClass",
                        "apiVersion": "scheduling.k8s.io/v1",
                        "metadata": { "name": name },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct EndpointsHandler;

#[async_trait]
impl ApiHandler for EndpointsHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        // Extract namespace from query if present
        let namespace = query.get("namespace").cloned();

        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                if let Some(name) = query.get("name") {
                    let namespace = namespace
                        .as_ref()
                        .ok_or_else(|| A3sError::Project("namespace is required".into()))?;
                    let eps = state.endpoints.read().await;
                    if let Some(ns_eps) = eps.get(namespace) {
                        if let Some(ep) = ns_eps.get(name) {
                            let response = serde_json::json!({
                                "apiVersion": "v1",
                                "kind": "Endpoints",
                                "metadata": {
                                    "name": ep.name,
                                    "namespace": ep.namespace,
                                    "creationTimestamp": ep.created_at.to_rfc3339(),
                                },
                                "subsets": ep.subsets,
                            });
                            return Ok(ApiResponseOrStatus::Response(
                                serde_json::to_string(&response).unwrap(),
                            ));
                        }
                    }
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                let eps = state.endpoints.read().await;
                let items: Vec<serde_json::Value> = if let Some(ref ns) = namespace {
                    eps.get(ns)
                        .map(|ns_eps| {
                            ns_eps
                                .values()
                                .map(|ep| {
                                    serde_json::json!({
                                        "apiVersion": "v1",
                                        "kind": "Endpoints",
                                        "metadata": {
                                            "name": ep.name,
                                            "namespace": ep.namespace,
                                            "creationTimestamp": ep.created_at.to_rfc3339(),
                                        },
                                        "subsets": ep.subsets,
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                } else {
                    eps.values()
                        .flat_map(|ns_eps| {
                            ns_eps
                                .values()
                                .map(|ep| {
                                    serde_json::json!({
                                        "apiVersion": "v1",
                                        "kind": "Endpoints",
                                        "metadata": {
                                            "name": ep.name,
                                            "namespace": ep.namespace,
                                            "creationTimestamp": ep.created_at.to_rfc3339(),
                                        },
                                        "subsets": ep.subsets,
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect()
                };

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "EndpointsList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                let now = chrono::Utc::now();

                let subsets: Vec<EndpointsSubset> = request["subsets"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .map(|s| EndpointsSubset {
                                addresses: s["addresses"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .map(|addr| EndpointAddress {
                                                ip: addr["ip"].as_str().unwrap_or("").to_string(),
                                                hostname: addr["hostname"]
                                                    .as_str()
                                                    .map(String::from),
                                                node_name: addr["nodeName"]
                                                    .as_str()
                                                    .map(String::from),
                                                target_ref: addr["targetRef"].as_object().map(
                                                    |o| EndpointTargetRef {
                                                        kind: o["kind"]
                                                            .as_str()
                                                            .unwrap_or("")
                                                            .to_string(),
                                                        name: o["name"]
                                                            .as_str()
                                                            .unwrap_or("")
                                                            .to_string(),
                                                        namespace: o["namespace"]
                                                            .as_str()
                                                            .map(String::from),
                                                    },
                                                ),
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                notReadyAddresses: s["notReadyAddresses"]
                                    .as_array()
                                    .map(|a| {
                                        a.iter()
                                            .map(|addr| EndpointAddress {
                                                ip: addr["ip"].as_str().unwrap_or("").to_string(),
                                                hostname: addr["hostname"]
                                                    .as_str()
                                                    .map(String::from),
                                                node_name: addr["nodeName"]
                                                    .as_str()
                                                    .map(String::from),
                                                target_ref: addr["targetRef"].as_object().map(
                                                    |o| EndpointTargetRef {
                                                        kind: o["kind"]
                                                            .as_str()
                                                            .unwrap_or("")
                                                            .to_string(),
                                                        name: o["name"]
                                                            .as_str()
                                                            .unwrap_or("")
                                                            .to_string(),
                                                        namespace: o["namespace"]
                                                            .as_str()
                                                            .map(String::from),
                                                    },
                                                ),
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                                ports: s["ports"]
                                    .as_array()
                                    .map(|p| {
                                        p.iter()
                                            .map(|port| EndpointPort {
                                                port: port["port"].as_i64().unwrap_or(0) as i32,
                                                protocol: port["protocol"]
                                                    .as_str()
                                                    .unwrap_or("TCP")
                                                    .to_string(),
                                                app_protocol: port["appProtocol"]
                                                    .as_str()
                                                    .map(String::from),
                                                hostname: port["hostname"]
                                                    .as_str()
                                                    .map(String::from),
                                            })
                                            .collect()
                                    })
                                    .unwrap_or_default(),
                            })
                            .collect()
                    })
                    .unwrap_or_default();

                let ep = EndpointsDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    subsets,
                    created_at: now,
                };

                {
                    let mut eps = state.endpoints.write().await;
                    eps.entry(namespace.to_string())
                        .or_insert_with(HashMap::new)
                        .insert(name.to_string(), ep);
                }

                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "Endpoints",
                        "metadata": { "name": name, "namespace": namespace },
                    }),
                    kind: "Endpoints".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "Endpoints",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;
                let namespace = namespace
                    .as_ref()
                    .ok_or_else(|| A3sError::Project("namespace is required".into()))?;

                let mut eps = state.endpoints.write().await;
                if let Some(ns_eps) = eps.get_mut(namespace) {
                    if ns_eps.remove(name).is_none() {
                        return Ok(ApiResponseOrStatus::NotFound);
                    }
                } else {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            HttpMethod::Patch => {
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;
                let namespace = namespace
                    .as_ref()
                    .ok_or_else(|| A3sError::Project("namespace is required".into()))?;

                let mut eps = state.endpoints.write().await;
                if let Some(ns_eps) = eps.get_mut(namespace) {
                    if !ns_eps.contains_key(name) {
                        return Ok(ApiResponseOrStatus::NotFound);
                    }
                } else {
                    return Ok(ApiResponseOrStatus::NotFound);
                }

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Endpoints",
                        "apiVersion": "v1",
                        "metadata": { "name": name, "namespace": namespace },
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct LimitRangeHandler;

#[async_trait]
impl ApiHandler for LimitRangeHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Check if namespaced request
                if let Some(namespace) = query.get("namespace") {
                    let limitranges = state
                        .limitranges
                        .list_namespace_limitranges(namespace)
                        .await;
                    let items: Vec<serde_json::Value> = limitranges
                        .iter()
                        .map(|lr| {
                            serde_json::json!({
                                "apiVersion": "v1",
                                "kind": "LimitRange",
                                "metadata": {
                                    "name": lr.name,
                                    "namespace": lr.namespace,
                                    "creationTimestamp": lr.created_at.to_rfc3339(),
                                },
                                "spec": {
                                    "limits": lr.spec.limits,
                                },
                            })
                        })
                        .collect();

                    let response = serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "LimitRangeList",
                        "metadata": {
                            "resourceVersion": resource_version,
                        },
                        "items": items,
                    });

                    return Ok(ApiResponseOrStatus::Response(
                        serde_json::to_string(&response).unwrap(),
                    ));
                }

                // List all limitranges across namespaces
                let limitranges = state.limitranges.list_limitranges().await;
                let items: Vec<serde_json::Value> = limitranges
                    .iter()
                    .map(|lr| {
                        serde_json::json!({
                            "apiVersion": "v1",
                            "kind": "LimitRange",
                            "metadata": {
                                "name": lr.name,
                                "namespace": lr.namespace,
                                "creationTimestamp": lr.created_at.to_rfc3339(),
                            },
                            "spec": {
                                "limits": lr.spec.limits,
                            },
                        })
                    })
                    .collect();

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "LimitRangeList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": items,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let name = request["metadata"]["name"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.name is required".into()))?;
                let namespace = request["metadata"]["namespace"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("metadata.namespace is required".into()))?;

                // Check if already exists
                if state
                    .limitranges
                    .get_limitrange(namespace, name)
                    .await
                    .is_some()
                {
                    return Ok(ApiResponseOrStatus::Conflict);
                }

                let now = chrono::Utc::now();

                // Parse spec
                let spec = request["spec"].clone();

                let limitrange = crate::state::limitrange::LimitRangeDesired {
                    name: name.to_string(),
                    namespace: namespace.to_string(),
                    spec: crate::state::limitrange::LimitRangeSpec { limits: vec![] },
                    created_at: now,
                };

                // Store via controller
                state.limitranges.set_limitrange(limitrange.clone()).await?;

                // Broadcast watch event
                let watch_event = ApiWatchEvent {
                    event_type: WatchEventType::Added,
                    object: serde_json::json!({
                        "apiVersion": "v1",
                        "kind": "LimitRange",
                        "metadata": {
                            "name": name,
                            "namespace": namespace,
                            "creationTimestamp": now.to_rfc3339(),
                        },
                    }),
                    kind: "LimitRange".to_string(),
                };
                state.broadcast_watch(watch_event).await;

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "LimitRange",
                    "metadata": {
                        "name": name,
                        "namespace": namespace,
                        "creationTimestamp": now.to_rfc3339(),
                    },
                    "spec": spec,
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            HttpMethod::Delete => {
                let namespace = query
                    .get("namespace")
                    .ok_or_else(|| A3sError::Project("namespace is required".into()))?;
                let name = query
                    .get("name")
                    .ok_or_else(|| A3sError::Project("name is required".into()))?;

                state.limitranges.delete_limitrange(namespace, name).await?;

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&serde_json::json!({
                        "kind": "Status",
                        "apiVersion": "v1",
                        "status": "Success",
                    }))
                    .unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct TokenReviewHandler;

#[async_trait]
impl ApiHandler for TokenReviewHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        _query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                let token = request["spec"]["token"]
                    .as_str()
                    .ok_or_else(|| A3sError::Project("spec.token is required".into()))?;

                // Validate token via TokenController
                let validation = state.token_controller.validate_token(token).await?;

                let error_msg = validation.status.error.clone();
                let response = serde_json::json!({
                    "apiVersion": "authentication.k8s.io/v1",
                    "kind": "TokenReview",
                    "status": {
                        "authenticated": validation.status.authenticated,
                        "user": validation.status.user.map(|u| {
                            serde_json::json!({
                                "username": u.username,
                                "uid": u.uid,
                                "groups": u.groups,
                                "extra": u.extra
                            })
                        }),
                        "tokenExpirationTimestamp": error_msg.clone().or(Some("".to_string())),
                        "error": error_msg
                    }
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct SubjectAccessReviewHandler;

#[async_trait]
impl ApiHandler for SubjectAccessReviewHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        _query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Extract authorization details
                let resource = request["spec"]["resource"].as_str().unwrap_or("");
                let verb_str = request["spec"]["verb"].as_str().unwrap_or("get");
                let namespace = request["spec"]["namespace"].as_str().unwrap_or("");
                let name = request["spec"]["name"].as_str();
                let api_group = request["spec"]["apiGroup"].as_str().unwrap_or("");

                // Get user from spec.user or default to anonymous
                let user_info = request["spec"]["user"].as_str();
                let groups = request["spec"]["groups"]
                    .as_array()
                    .map(|arr| {
                        arr.iter()
                            .filter_map(|v| v.as_str())
                            .map(String::from)
                            .collect()
                    })
                    .unwrap_or_default();

                // Parse verb
                let verb = match verb_str {
                    "get" => crate::state::rbac::Verb::Get,
                    "list" => crate::state::rbac::Verb::List,
                    "watch" => crate::state::rbac::Verb::Watch,
                    "create" => crate::state::rbac::Verb::Create,
                    "update" => crate::state::rbac::Verb::Update,
                    "patch" => crate::state::rbac::Verb::Patch,
                    "delete" => crate::state::rbac::Verb::Delete,
                    "deletecollection" => crate::state::rbac::Verb::Deletecollection,
                    _ => crate::state::rbac::Verb::Get,
                };

                // Build AuthRequest
                let auth_request = crate::state::rbac::AuthRequest {
                    user: user_info.unwrap_or("anonymous").to_string(),
                    groups,
                    namespace: if namespace.is_empty() {
                        None
                    } else {
                        Some(namespace.to_string())
                    },
                    resource: resource.to_string(),
                    api_group: if api_group.is_empty() {
                        None
                    } else {
                        Some(api_group.to_string())
                    },
                    verb,
                    name: name.map(String::from),
                    is_non_resource: false,
                    path: None,
                };

                // Authorize via RBAC
                let result = state.rbac.authorize(&auth_request).await;

                let response = serde_json::json!({
                    "apiVersion": "authorization.k8s.io/v1",
                    "kind": "SubjectAccessReview",
                    "status": {
                        "allowed": result.allowed,
                        "reason": result.reason.unwrap_or(""),
                        "evaluationError": result.error.unwrap_or_default()
                    }
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct SelfSubjectAccessReviewHandler;

#[async_trait]
impl ApiHandler for SelfSubjectAccessReviewHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        _query: &HashMap<String, String>,
        body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Post => {
                let body = body.ok_or_else(|| A3sError::Project("request body required".into()))?;
                let request: serde_json::Value = serde_json::from_str(body)
                    .map_err(|e| A3sError::Project(format!("invalid JSON: {}", e)))?;

                // Extract authorization details
                let resource = request["spec"]["resource"].as_str().unwrap_or("");
                let verb_str = request["spec"]["verb"].as_str().unwrap_or("get");
                let namespace = request["spec"]["namespace"].as_str().unwrap_or("");
                let name = request["spec"]["name"].as_str();
                let api_group = request["spec"]["apiGroup"].as_str().unwrap_or("");

                // SelfSubjectAccessReview uses the user from the auth context
                // For now, default to system:admin (in production, extract from Bearer token)
                let user = "system:admin";
                let groups = vec!["system:masters".to_string()];

                // Parse verb
                let verb = match verb_str {
                    "get" => crate::state::rbac::Verb::Get,
                    "list" => crate::state::rbac::Verb::List,
                    "watch" => crate::state::rbac::Verb::Watch,
                    "create" => crate::state::rbac::Verb::Create,
                    "update" => crate::state::rbac::Verb::Update,
                    "patch" => crate::state::rbac::Verb::Patch,
                    "delete" => crate::state::rbac::Verb::Delete,
                    "deletecollection" => crate::state::rbac::Verb::Deletecollection,
                    _ => crate::state::rbac::Verb::Get,
                };

                // Build AuthRequest
                let auth_request = crate::state::rbac::AuthRequest {
                    user: user.to_string(),
                    groups,
                    namespace: if namespace.is_empty() {
                        None
                    } else {
                        Some(namespace.to_string())
                    },
                    resource: resource.to_string(),
                    api_group: if api_group.is_empty() {
                        None
                    } else {
                        Some(api_group.to_string())
                    },
                    verb,
                    name: name.map(String::from),
                    is_non_resource: false,
                    path: None,
                };

                // Authorize via RBAC
                let result = state.rbac.authorize(&auth_request).await;

                let response = serde_json::json!({
                    "apiVersion": "authorization.k8s.io/v1",
                    "kind": "SelfSubjectAccessReview",
                    "status": {
                        "allowed": result.allowed,
                        "reason": result.reason.unwrap_or(""),
                        "evaluationError": result.error.unwrap_or_default()
                    }
                });

                Ok(ApiResponseOrStatus::Created(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct ComponentStatusHandler;

#[async_trait]
impl ApiHandler for ComponentStatusHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        _query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Return component status (scheduler, controller-manager, etc.)
                // In a real cluster, these would report actual component health
                let components = vec![
                    serde_json::json!({
                        "metadata": {
                            "name": "scheduler",
                            "creationTimestamp": null,
                            "labels": {
                                "k8s.io/component": "scheduler"
                            }
                        },
                        "conditions": [
                            {
                                "type": "Healthy",
                                "status": "True",
                                "message": "scheduler is healthy",
                                "error": ""
                            }
                        ]
                    }),
                    serde_json::json!({
                        "metadata": {
                            "name": "controller-manager",
                            "creationTimestamp": null,
                            "labels": {
                                "k8s.io/component": "controller-manager"
                            }
                        },
                        "conditions": [
                            {
                                "type": "Healthy",
                                "status": "True",
                                "message": "controller-manager is healthy",
                                "error": ""
                            }
                        ]
                    }),
                    serde_json::json!({
                        "metadata": {
                            "name": "etcd-0",
                            "creationTimestamp": null,
                            "labels": {
                                "k8s.io/component": "etcd"
                            }
                        },
                        "conditions": [
                            {
                                "type": "Healthy",
                                "status": "True",
                                "message": "etcd is healthy",
                                "error": ""
                            }
                        ]
                    }),
                ];

                let response = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "ComponentStatusList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": components,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct EndpointSliceHandler;

#[async_trait]
impl ApiHandler for EndpointSliceHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        _path: &str,
        query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        match method {
            HttpMethod::Get => {
                let resource_version = state.current_resource_version().await;

                // Get namespace from path - extract from pattern /namespaces/{namespace}/endpointslices
                let namespace = query
                    .get("namespace")
                    .cloned()
                    .unwrap_or_else(|| "default".to_string());

                // Get services in this namespace to find endpoints
                let services = state.services.read().await;
                let ns_services = services.get(&namespace);

                // Generate endpoint slices based on services
                let mut slices = Vec::new();

                if let Some(ns_svcs) = ns_services {
                    for (svc_name, svc) in ns_svcs {
                        let slice_name = format!("{}-{}", svc_name, "abc123");

                        // Get pods that match the service selector
                        let pods = state.pods.read().await;
                        let matching_pods: Vec<_> = pods
                            .get(&namespace)
                            .map(|ns_pods| {
                                ns_pods
                                    .values()
                                    .filter(|pod| {
                                        // Check if pod labels match service selector
                                        svc.selector.iter().all(|(_k, v)| {
                                            pod.id.contains(v) || pod.deployment.contains(v)
                                        })
                                    })
                                    .cloned()
                                    .collect::<Vec<_>>()
                            })
                            .unwrap_or_default();

                        let endpoints: Vec<serde_json::Value> = matching_pods.iter().map(|pod| {
                            serde_json::json!({
                                "addresses": pod.ip.as_ref().map(|ip| serde_json::json!([ip.clone()])).unwrap_or(serde_json::json!([])),
                                "conditions": {
                                    "ready": pod.status == crate::state::PodStatus::Running,
                                    "serving": pod.status == crate::state::PodStatus::Running,
                                    "terminating": false
                                },
                                "targetRef": {
                                    "kind": "Pod",
                                    "name": pod.id.clone(),
                                    "namespace": namespace.clone(),
                                    "uid": pod.id.clone()
                                },
                                "ports": svc.ports.iter().map(|p| {
                                    serde_json::json!({
                                        "port": p.port,
                                        "protocol": p.protocol,
                                        "name": p.name
                                    })
                                }).collect::<Vec<_>>()
                            })
                        }).collect();

                        slices.push(serde_json::json!({
                            "apiVersion": "discovery.a3s.io/v1",
                            "kind": "EndpointSlice",
                            "metadata": {
                                "name": slice_name,
                                "namespace": namespace.clone(),
                                "labels": {
                                    "kubernetes.io/service-name": svc_name
                                },
                                "resourceVersion": resource_version.clone(),
                            },
                            "addressType": "IPv4",
                            "ports": svc.ports.first().map(|p| {
                                serde_json::json!([{
                                    "port": p.port,
                                    "protocol": p.protocol,
                                    "name": p.name
                                }])
                            }).unwrap_or(serde_json::json!([])),
                            "endpoints": endpoints,
                        }));
                    }
                }

                let response = serde_json::json!({
                    "apiVersion": "discovery.a3s.io/v1",
                    "kind": "EndpointSliceList",
                    "metadata": {
                        "resourceVersion": resource_version,
                    },
                    "items": slices,
                });

                Ok(ApiResponseOrStatus::Response(
                    serde_json::to_string(&response).unwrap(),
                ))
            }
            _ => Ok(ApiResponseOrStatus::MethodNotAllowed),
        }
    }
}

struct VersionHandler;

#[async_trait]
impl ApiHandler for VersionHandler {
    async fn handle(
        &self,
        _state: &ApiServerState,
        method: HttpMethod,
        path: &str,
        _query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        if method != HttpMethod::Get {
            return Ok(ApiResponseOrStatus::MethodNotAllowed);
        }

        if path == "/version" {
            // Kubernetes-style version endpoint
            let response = serde_json::json!({
                "major": "1",
                "minor": "28",
                "gitVersion": "v1.28.0-a3s",
                "gitCommit": "unknown",
                "gitTreeState": "clean",
                "buildDate": chrono::Utc::now().to_rfc3339(),
                "goVersion": "go1.21",
                "compiler": "gc",
                "platform": "darwin/arm64"
            });
            Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ))
        } else if path == "/api" {
            // API discovery endpoint
            let response = serde_json::json!({
                "kind": "APIVersions",
                "versions": ["v1"],
                "serverAddressByClientCIDRs": [
                    {
                        "clientCIDR": "0.0.0.0/0",
                        "serverAddress": "127.0.0.1:6443"
                    }
                ]
            });
            Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ))
        } else if path == "/api/v1" {
            // API v1 discovery
            let response = serde_json::json!({
                "kind": "APIResourceList",
                "groupVersion": "v1",
                "resources": [
                    {"name": "namespaces", "singularName": "", "namespaced": false, "kind": "Namespace", "verbs": ["get", "list", "create", "delete"]},
                    {"name": "pods", "singularName": "", "namespaced": true, "kind": "Pod", "verbs": ["get", "list", "create", "delete", "patch"]},
                    {"name": "services", "singularName": "", "namespaced": true, "kind": "Service", "verbs": ["get", "list", "create", "delete", "patch"]},
                    {"name": "configmaps", "singularName": "", "namespaced": true, "kind": "ConfigMap", "verbs": ["get", "list", "create", "delete", "patch"]},
                    {"name": "secrets", "singularName": "", "namespaced": true, "kind": "Secret", "verbs": ["get", "list", "create", "delete", "patch"]},
                    {"name": "persistentvolumes", "singularName": "", "namespaced": false, "kind": "PersistentVolume", "verbs": ["get", "list"]},
                    {"name": "persistentvolumeclaims", "singularName": "", "namespaced": true, "kind": "PersistentVolumeClaim", "verbs": ["get", "list", "create", "delete", "patch"]},
                    {"name": "serviceaccounts", "singularName": "", "namespaced": true, "kind": "ServiceAccount", "verbs": ["get", "list", "create", "delete", "patch"]},
                    {"name": "events", "singularName": "", "namespaced": true, "kind": "Event", "verbs": ["get", "list"]},
                    {"name": "nodes", "singularName": "", "namespaced": false, "kind": "Node", "verbs": ["get", "list"]},
                    {"name": "componentstatuses", "singularName": "", "namespaced": false, "kind": "ComponentStatus", "verbs": ["get", "list"]}
                ]
            });
            Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ))
        } else {
            Ok(ApiResponseOrStatus::NotFound)
        }
    }
}

// =============================================================================
// Exec, Attach, and PortForward Handlers
// =============================================================================

/// ExecHandler - handles kubectl exec requests.
/// API: POST /api/v1/namespaces/{namespace}/pods/{name}/exec
struct ExecHandler;

#[async_trait]
impl ApiHandler for ExecHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        path: &str,
        query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        if method != HttpMethod::Post {
            return Ok(ApiResponseOrStatus::MethodNotAllowed);
        }

        // Parse path to get namespace, pod name
        let params = Self::extract_params(path)?;
        let namespace = params
            .get("namespace")
            .ok_or_else(|| A3sError::Project("namespace required".into()))?;
        let pod_name = params
            .get("name")
            .ok_or_else(|| A3sError::Project("pod name required".into()))?;

        // Get exec parameters from query string (kubectl style)
        let command = query
            .get("command")
            .cloned()
            .or_else(|| query.get("cmd").cloned())
            .unwrap_or_else(|| "echo OK".to_string());
        let stdin = query.get("stdin").map(|s| s == "true").unwrap_or(false);
        let tty = query.get("tty").map(|s| s == "true").unwrap_or(false);

        // Find the pod in state
        let pods = state.pods.read().await;
        let pod = pods
            .get(namespace)
            .and_then(|pods| pods.get(pod_name))
            .cloned();

        if pod.is_none() {
            return Ok(ApiResponseOrStatus::NotFound);
        }

        let pod = pod.unwrap();

        // Get sandbox ID from pod
        let sandbox_id = pod.id.clone();

        // Check if exec socket exists
        let socket_path = dirs::home_dir()
            .map(|h| {
                h.join(".a3s")
                    .join("boxes")
                    .join(&sandbox_id)
                    .join("sockets")
                    .join("exec.sock")
            })
            .unwrap_or_else(|| {
                std::path::PathBuf::from(format!("~/.a3s/boxes/{}/sockets/exec.sock", sandbox_id))
            });

        if !socket_path.exists() {
            // Return error response for disconnected exec
            let response = serde_json::json!({
                "kind": "Status",
                "apiVersion": "v1",
                "status": "Failure",
                "message": "exec endpoint not available - pod may not be running",
                "reason": "InternalError",
                "details": {
                    "name": pod_name,
                    "kind": "pods/exec"
                },
                "code": 500
            });
            return Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ));
        }

        // For WebSocket-based exec (kubectl --stream), we return upgrade info
        // For non-streaming exec, we execute directly and return output
        if query.get("stream") == Some(&"true".to_string())
            || query.get("stream") == None && !stdin && !tty
        {
            // Streaming exec - return connection upgrade response
            // The actual WebSocket upgrade would happen at the transport layer
            let response = serde_json::json!({
                "kind": "Status",
                "apiVersion": "v1",
                "status": "Success",
                "message": "exec connection ready",
                "details": {
                    "name": pod_name,
                    "kind": "pods/exec"
                }
            });
            return Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ));
        }

        // Non-streaming exec - execute directly via exec socket
        // This is a simplified implementation
        let response = serde_json::json!({
            "kind": "Status",
            "apiVersion": "v1",
            "status": "Success",
            "message": format!("exec would run: {}", command),
            "details": {
                "name": pod_name,
                "kind": "pods/exec"
            }
        });
        Ok(ApiResponseOrStatus::Response(
            serde_json::to_string(&response).unwrap(),
        ))
    }
}

impl ExecHandler {
    fn extract_params(path: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut params = std::collections::HashMap::new();
        // Parse /api/v1/namespaces/{namespace}/pods/{name}/exec
        let parts: Vec<&str> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if *part == "namespaces" && i + 1 < parts.len() {
                params.insert("namespace".to_string(), parts[i + 1].to_string());
            }
            if *part == "pods" && i + 1 < parts.len() {
                params.insert("name".to_string(), parts[i + 1].to_string());
            }
        }
        Ok(params)
    }
}

/// AttachHandler - handles kubectl attach requests.
/// API: POST /api/v1/namespaces/{namespace}/pods/{name}/attach
struct AttachHandler;

#[async_trait]
impl ApiHandler for AttachHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        path: &str,
        query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        if method != HttpMethod::Post {
            return Ok(ApiResponseOrStatus::MethodNotAllowed);
        }

        // Parse path to get namespace, pod name
        let params = Self::extract_params(path)?;
        let namespace = params
            .get("namespace")
            .ok_or_else(|| A3sError::Project("namespace required".into()))?;
        let pod_name = params
            .get("name")
            .ok_or_else(|| A3sError::Project("pod name required".into()))?;

        let stdin = query.get("stdin").map(|s| s == "true").unwrap_or(false);
        let tty = query.get("tty").map(|s| s == "true").unwrap_or(false);

        // Find the pod in state
        let pods = state.pods.read().await;
        let pod = pods
            .get(namespace)
            .and_then(|pods| pods.get(pod_name))
            .cloned();

        if pod.is_none() {
            return Ok(ApiResponseOrStatus::NotFound);
        }

        let pod = pod.unwrap();

        // Check for PTY socket
        let pty_socket = dirs::home_dir()
            .map(|h| {
                h.join(".a3s")
                    .join("boxes")
                    .join(&pod.id)
                    .join("sockets")
                    .join("pty.sock")
            })
            .unwrap_or_else(|| {
                std::path::PathBuf::from(format!("~/.a3s/boxes/{}/sockets/pty.sock", pod.id))
            });

        if !pty_socket.exists() {
            let response = serde_json::json!({
                "kind": "Status",
                "apiVersion": "v1",
                "status": "Failure",
                "message": "attach endpoint not available - pod may not support interactive mode",
                "reason": "InternalError",
                "details": {
                    "name": pod_name,
                    "kind": "pods/attach"
                },
                "code": 500
            });
            return Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ));
        }

        // Return attach ready status
        let response = serde_json::json!({
            "kind": "Status",
            "apiVersion": "v1",
            "status": "Success",
            "message": if tty || stdin { "attach ready with PTY" } else { "attach ready" },
            "details": {
                "name": pod_name,
                "kind": "pods/attach"
            }
        });
        Ok(ApiResponseOrStatus::Response(
            serde_json::to_string(&response).unwrap(),
        ))
    }
}

impl AttachHandler {
    fn extract_params(path: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut params = std::collections::HashMap::new();
        let parts: Vec<&str> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if *part == "namespaces" && i + 1 < parts.len() {
                params.insert("namespace".to_string(), parts[i + 1].to_string());
            }
            if *part == "pods" && i + 1 < parts.len() {
                params.insert("name".to_string(), parts[i + 1].to_string());
            }
        }
        Ok(params)
    }
}

/// PortForwardHandler - handles kubectl port-forward requests.
/// API: POST /api/v1/namespaces/{namespace}/pods/{name}/portforward
struct PortForwardHandler;

#[async_trait]
impl ApiHandler for PortForwardHandler {
    async fn handle(
        &self,
        state: &ApiServerState,
        method: HttpMethod,
        path: &str,
        query: &HashMap<String, String>,
        _body: Option<&str>,
    ) -> Result<ApiResponseOrStatus> {
        if method != HttpMethod::Post {
            return Ok(ApiResponseOrStatus::MethodNotAllowed);
        }

        // Parse path to get namespace, pod name
        let params = Self::extract_params(path)?;
        let namespace = params
            .get("namespace")
            .ok_or_else(|| A3sError::Project("namespace required".into()))?;
        let pod_name = params
            .get("name")
            .ok_or_else(|| A3sError::Project("pod name required".into()))?;

        // Get port from query or body
        let port = query
            .get("port")
            .and_then(|p| p.parse::<u16>().ok())
            .unwrap_or(0);

        // Find the pod in state
        let pods = state.pods.read().await;
        let pod = pods
            .get(namespace)
            .and_then(|pods| pods.get(pod_name))
            .cloned();

        if pod.is_none() {
            return Ok(ApiResponseOrStatus::NotFound);
        }

        let pod = pod.unwrap();

        // Check for port forward socket
        let socket_path = dirs::home_dir()
            .map(|h| {
                h.join(".a3s")
                    .join("boxes")
                    .join(&pod.id)
                    .join("sockets")
                    .join("proxy.sock")
            })
            .unwrap_or_else(|| {
                std::path::PathBuf::from(format!("~/.a3s/boxes/{}/sockets/proxy.sock", pod.id))
            });

        // Get configured ports from pod info
        let ports = pod
            .socket_path
            .as_ref()
            .map(|_p| vec![port])
            .unwrap_or_else(|| vec![8000, 8080, 443]);

        if !socket_path.exists() {
            let response = serde_json::json!({
                "kind": "Status",
                "apiVersion": "v1",
                "status": "Failure",
                "message": "portforward endpoint not available - pod may not be running",
                "reason": "InternalError",
                "details": {
                    "name": pod_name,
                    "kind": "pods/portforward"
                },
                "code": 500
            });
            return Ok(ApiResponseOrStatus::Response(
                serde_json::to_string(&response).unwrap(),
            ));
        }

        // Return port forward ready status
        let response = serde_json::json!({
            "kind": "Status",
            "apiVersion": "v1",
            "status": "Success",
            "message": format!("port forward ready for ports: {:?}", ports),
            "details": {
                "name": pod_name,
                "kind": "pods/portforward",
                "ports": ports
            }
        });
        Ok(ApiResponseOrStatus::Response(
            serde_json::to_string(&response).unwrap(),
        ))
    }
}

impl PortForwardHandler {
    fn extract_params(path: &str) -> Result<std::collections::HashMap<String, String>> {
        let mut params = std::collections::HashMap::new();
        let parts: Vec<&str> = path.split('/').collect();
        for (i, part) in parts.iter().enumerate() {
            if *part == "namespaces" && i + 1 < parts.len() {
                params.insert("namespace".to_string(), parts[i + 1].to_string());
            }
            if *part == "pods" && i + 1 < parts.len() {
                params.insert("name".to_string(), parts[i + 1].to_string());
            }
        }
        Ok(params)
    }
}

/// Start the API server with axum HTTP server.
pub async fn start_server(config: ApiServerConfig, state: Arc<ApiServerState>) -> Result<()> {
    use axum::{
        body::Body,
        extract::{Path, Query},
        http::StatusCode,
        response::Response,
        routing::get,
        Router,
    };
    use tower_http::cors::{Any, CorsLayer};

    let server = ApiServer::new(config.clone(), state.clone());
    let addr = server.listen_addr();

    println!("Starting API server on {}", addr);
    println!("API endpoints available:");
    println!("  GET  /api/v1/namespaces");
    println!("  GET  /api/v1/namespaces/{{name}}");
    println!("  POST /api/v1/namespaces");
    println!("  DELETE /api/v1/namespaces/{{name}}");
    println!("  GET  /api/v1/pods");
    println!("  GET  /api/v1/namespaces/{{namespace}}/pods/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/pods");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/pods/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/pods/{{name}}");
    println!("  GET  /api/v1/services");
    println!("  GET  /api/v1/namespaces/{{namespace}}/services/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/services");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/services/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/services/{{name}}");
    println!("  GET  /apis/apps/v1/deployments");
    println!("  GET  /api/v1/namespaces/{{namespace}}/deployments/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/deployments");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/deployments/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/deployments/{{name}}");
    println!("  GET  /api/v1/persistentvolumes");
    println!("  GET  /api/v1/persistentvolumeclaims");
    println!("  GET  /api/v1/namespaces/{{namespace}}/persistentvolumeclaims/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/persistentvolumeclaims");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/persistentvolumeclaims/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/persistentvolumeclaims/{{name}}");
    println!("  GET  /api/v1/serviceaccounts");
    println!("  GET  /api/v1/namespaces/{{namespace}}/serviceaccounts/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/serviceaccounts");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/serviceaccounts/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/serviceaccounts/{{name}}");
    println!("  GET  /apis/storage.a3s.io/v1/storageclasses");
    println!("  GET  /api/v1/configmaps");
    println!("  GET  /api/v1/namespaces/{{namespace}}/configmaps/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/configmaps");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/configmaps/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/configmaps/{{name}}");
    println!("  GET  /api/v1/secrets");
    println!("  GET  /api/v1/namespaces/{{namespace}}/secrets/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/secrets");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/secrets/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/secrets/{{name}}");
    println!("  GET  /api/v1/nodes");
    println!("  GET  /api/v1/nodes/{{name}}");
    println!("  GET  /api/v1/events");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/roles");
    println!("  POST /apis/rbac.authorization.a3s.io/v1/namespaces/{{namespace}}/roles");
    println!("  DELETE /apis/rbac.authorization.a3s.io/v1/namespaces/{{namespace}}/roles/{{name}}");
    println!("  PATCH /apis/rbac.authorization.a3s.io/v1/namespaces/{{namespace}}/roles/{{name}}");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/clusterroles");
    println!("  POST /apis/rbac.authorization.a3s.io/v1/clusterroles");
    println!("  DELETE /apis/rbac.authorization.a3s.io/v1/clusterroles/{{name}}");
    println!("  PATCH /apis/rbac.authorization.a3s.io/v1/clusterroles/{{name}}");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/rolebindings");
    println!("  POST /apis/rbac.authorization.a3s.io/v1/namespaces/{{namespace}}/rolebindings");
    println!("  DELETE /apis/rbac.authorization.a3s.io/v1/namespaces/{{namespace}}/rolebindings/{{name}}");
    println!(
        "  PATCH /apis/rbac.authorization.a3s.io/v1/namespaces/{{namespace}}/rolebindings/{{name}}"
    );
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/clusterrolebindings");
    println!("  POST /apis/rbac.authorization.a3s.io/v1/clusterrolebindings");
    println!("  DELETE /apis/rbac.authorization.a3s.io/v1/clusterrolebindings/{{name}}");
    println!("  PATCH /apis/rbac.authorization.a3s.io/v1/clusterrolebindings/{{name}}");
    println!("  GET  /apis/apps/v1/daemonsets");
    println!("  GET  /api/v1/namespaces/{{namespace}}/daemonsets/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/daemonsets");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/daemonsets/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/daemonsets/{{name}}");
    println!("  GET  /apis/apps/v1/statefulsets");
    println!("  GET  /api/v1/namespaces/{{namespace}}/statefulsets/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/statefulsets");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/statefulsets/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/statefulsets/{{name}}");
    println!("  GET  /apis/networking.a3s.io/v1/ingresses");
    println!("  GET  /api/v1/namespaces/{{namespace}}/ingresses/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/ingresses");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/ingresses/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/ingresses/{{name}}");
    println!("  GET  /apis/batch/v1/jobs");
    println!("  GET  /apis/batch/v1/namespaces/{{namespace}}/jobs/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/jobs");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/jobs/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/jobs/{{name}}");
    println!("  GET  /apis/batch/v1/cronjobs");
    println!("  GET  /apis/batch/v1/namespaces/{{namespace}}/cronjobs/{{name}}");
    println!("  POST /api/v1/namespaces/{{namespace}}/cronjobs");
    println!("  DELETE /api/v1/namespaces/{{namespace}}/cronjobs/{{name}}");
    println!("  PATCH /api/v1/namespaces/{{namespace}}/cronjobs/{{name}}");
    println!("  GET  /api/v1/watch/pods?watch=true");
    println!("  GET  /api/v1/watch/services?watch=true");
    println!("  GET  /api/v1/watch/configmaps?watch=true");
    println!("  GET  /api/v1/watch/namespaces?watch=true");
    println!("  GET  /api/v1/watch/deployments?watch=true");
    println!("  GET  /api/v1/watch/secrets?watch=true");
    println!("  GET  /api/v1/watch/serviceaccounts?watch=true");
    println!("  GET  /api/v1/watch/persistentvolumeclaims?watch=true");
    println!("  GET  /api/v1/watch/nodes?watch=true");
    println!("  GET  /api/v1/watch/persistentvolumes?watch=true");
    println!("  GET  /apis/apps/v1/watch/daemonsets?watch=true");
    println!("  GET  /apis/apps/v1/watch/statefulsets?watch=true");
    println!("  GET  /apis/apps/v1/watch/replicasets?watch=true");
    println!("  GET  /apis/storage.a3s.io/v1/watch/storageclasses?watch=true");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/watch/roles?watch=true");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/watch/clusterroles?watch=true");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/watch/rolebindings?watch=true");
    println!("  GET  /apis/rbac.authorization.a3s.io/v1/watch/clusterrolebindings?watch=true");
    println!("  GET  /apis/networking.a3s.io/v1/watch/ingresses?watch=true");
    println!("  GET  /apis/batch/v1/watch/jobs?watch=true");
    println!("  GET  /apis/batch/v1/watch/cronjobs?watch=true");
    println!("  GET  /healthz");
    println!("  GET  /readyz");
    println!();
    println!("Query parameters:");
    println!("  watch=true        - Enable watch with initial state (SSE)");
    println!("  limit=N           - Pagination limit");
    println!("  continue=TOKEN    - Pagination continue token");
    println!("  labelSelector=...  - Filter by labels");
    println!("  fieldSelector=... - Filter by fields");

    // Start watch event broadcaster (background task for SSE push)
    if state.sqlite_store.is_some() {
        println!("Starting watch event broadcaster for SSE push");
        state.start_watch_broadcaster();
    }

    // Create shared state for axum
    let api_state = state.clone();

    // Build router with CORS support
    let cors = CorsLayer::new()
        .allow_methods(Any)
        .allow_origin(Any)
        .allow_headers(Any);

    // Helper to create response
    let make_response = |response: ApiResponseOrStatus| -> Response<Body> {
        match response {
            ApiResponseOrStatus::Response(body) => {
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
            ApiResponseOrStatus::Status(status) => {
                let body = serde_json::to_string(&status).unwrap();
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
            ApiResponseOrStatus::SseStream(data) => {
                Response::builder()
                    .status(StatusCode::OK)
                    .header("Content-Type", "text/event-stream")
                    .header("Cache-Control", "no-cache")
                    .header("Connection", "keep-alive")
                    .body(Body::from(data))
                    .unwrap()
            }
            ApiResponseOrStatus::NotFound => {
                Response::builder()
                    .status(StatusCode::NOT_FOUND)
                    .body(Body::from(r#"{"kind":"Status","apiVersion":"v1","status":"Failure","message":"Not Found","reason":"NotFound"}"#))
                    .unwrap()
            }
            ApiResponseOrStatus::MethodNotAllowed => {
                Response::builder()
                    .status(StatusCode::METHOD_NOT_ALLOWED)
                    .body(Body::from(r#"{"kind":"Status","apiVersion":"v1","status":"Failure","message":"Method not allowed","reason":"MethodNotAllowed"}"#))
                    .unwrap()
            }
            ApiResponseOrStatus::Created(body) => {
                Response::builder()
                    .status(StatusCode::CREATED)
                    .header("Content-Type", "application/json")
                    .body(Body::from(body))
                    .unwrap()
            }
            ApiResponseOrStatus::Conflict => {
                Response::builder()
                    .status(StatusCode::CONFLICT)
                    .body(Body::from(r#"{"kind":"Status","apiVersion":"v1","status":"Failure","message":"Namespace already exists","reason":"AlreadyExists"}"#))
                    .unwrap()
            }
        }
    };

    // Build routes
    let api_state_clone = api_state.clone();
    let config_clone = config.clone();

    let app = Router::new()
        .route(
            "/api/v1/namespaces",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/namespaces", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/pods",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/pods", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/services",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/services", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/deployments",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/apis/apps/v1/deployments", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/persistentvolumes",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/persistentvolumes", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/persistentvolumeclaims",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/persistentvolumeclaims",
                                query.0,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/serviceaccounts",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/serviceaccounts", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/storage.a3s.io/v1/storageclasses",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/storage.a3s.io/v1/storageclasses",
                                query.0,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/configmaps",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/configmaps", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/secrets",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/secrets", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/nodes",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/nodes", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/events",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/events", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/roles",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/rbac.authorization.a3s.io/v1/roles",
                                query.0,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/clusterroles",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/rbac.authorization.a3s.io/v1/clusterroles",
                                query.0,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/rolebindings",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/rbac.authorization.a3s.io/v1/rolebindings",
                                query.0,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/clusterrolebindings",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/rbac.authorization.a3s.io/v1/clusterrolebindings",
                                query.0,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/daemonsets",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/apis/apps/v1/daemonsets", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/statefulsets",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/apis/apps/v1/statefulsets", query.0, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        // Individual resource routes with path parameters
        .route(
            "/api/v1/namespaces/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/namespaces/{name}", query, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/nodes/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(HttpMethod::Get, "/api/v1/nodes/{name}", query, None)
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/pods/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/pods/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/services/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/services/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/deployments/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/deployments/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/configmaps/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/configmaps/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/secrets/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/secrets/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/serviceaccounts/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/serviceaccounts/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/api/v1/namespaces/:namespace/persistentvolumeclaims/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/api/v1/namespaces/{namespace}/persistentvolumeclaims/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/namespaces/:namespace/daemonsets/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/apps/v1/namespaces/{namespace}/daemonsets/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/namespaces/:namespace/statefulsets/:name",
            get({
                let api_state = api_state_clone.clone();
                let config = config_clone.clone();
                move |path: Path<std::collections::HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let config = config.clone();
                    async move {
                        let query = path.0;
                        let server = ApiServer::new(config, api_state.clone());
                        let response = server
                            .route(
                                HttpMethod::Get,
                                "/apis/apps/v1/namespaces/{namespace}/statefulsets/{name}",
                                query,
                                None,
                            )
                            .await;
                        make_response(response)
                    }
                }
            }),
        )
        // Watch endpoints (SSE - true push via broadcast channel)
        .route(
            "/api/v1/watch/pods",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        // Create unbounded channel for this stream
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let pods = api_state.pods.read().await;
                            for (_, pod) in pods.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&pod).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        // Spawn task to forward events
                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Pod" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/services",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        // Create unbounded channel for this stream
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let services = api_state.services.read().await;
                            for (_, ns_services) in services.iter() {
                                for (_, service) in ns_services.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&service).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        // Spawn task to forward events
                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Service" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/configmaps",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        // Create unbounded channel for this stream
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let configmaps = api_state.configmaps.read().await;
                            for (_, ns_configmaps) in configmaps.iter() {
                                for (_, cm) in ns_configmaps.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&cm).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        // Spawn task to forward events
                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "ConfigMap" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/namespaces",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let namespaces = api_state.namespaces.read().await;
                            let resource_version = api_state.current_resource_version().await;
                            for (_, ns) in namespaces.iter() {
                                let api_ns = ApiNamespace {
                                    api_version: "v1".to_string(),
                                    kind: "Namespace".to_string(),
                                    metadata: NamespaceMeta {
                                        name: ns.meta.name.clone(),
                                        labels: ns.meta.labels.clone(),
                                        resource_version: resource_version.clone(),
                                        creation_timestamp: ns.meta.created_at.to_rfc3339(),
                                    },
                                };
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&api_ns).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Namespace" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/deployments",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let deployments = api_state.deployments.read().await;
                            for (_, ns_deployments) in deployments.iter() {
                                for (_, deployment) in ns_deployments.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&deployment).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Deployment" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/secrets",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let secrets = api_state.secrets.read().await;
                            for (_, ns_secrets) in secrets.iter() {
                                for (_, secret) in ns_secrets.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&secret).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Secret" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/serviceaccounts",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let accounts = api_state.service_accounts.list_all().await;
                            for account in accounts.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&account).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "ServiceAccount" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/persistentvolumeclaims",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let pvcs = api_state.persistent_volume_claims.read().await;
                            for (_, ns_pvcs) in pvcs.iter() {
                                for (_, pvc) in ns_pvcs.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&pvc).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "PersistentVolumeClaim" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/nodes",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let nodes = api_state.nodes.read().await;
                            for (_, node) in nodes.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&node).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Node" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/api/v1/watch/persistentvolumes",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let pvs = api_state.persistent_volumes.read().await;
                            for (_, pv) in pvs.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&pv).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "PersistentVolume" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/watch/daemonsets",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let daemonsets = api_state.daemonsets.list_all_daemonsets().await;
                            for (_, ds) in daemonsets.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&ds).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "DaemonSet" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/watch/statefulsets",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let statefulsets = api_state.statefulsets.list_all_statefulsets().await;
                            for (_, ss) in statefulsets.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&ss).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "StatefulSet" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/apps/v1/watch/replicasets",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let rss = api_state.replicasets.read().await;
                            for (_, ns_rss) in rss.iter() {
                                for (_, rs) in ns_rss.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&rs).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "ReplicaSet" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/storage.a3s.io/v1/watch/storageclasses",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let scs = api_state.storage_classes.list().await;
                            for sc in scs.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&sc).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "StorageClass" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/watch/roles",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let roles = api_state.rbac.list_all_roles().await;
                            for role in roles.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&role).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Role" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/watch/clusterroles",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let cluster_roles = api_state.rbac.list_cluster_roles().await;
                            for role in cluster_roles.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&role).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "ClusterRole" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/watch/rolebindings",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let bindings = api_state.rbac.list_all_role_bindings().await;
                            for binding in bindings.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&binding).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "RoleBinding" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/rbac.authorization.a3s.io/v1/watch/clusterrolebindings",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let bindings = api_state.rbac.list_cluster_role_bindings().await;
                            for binding in bindings.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&binding).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "ClusterRoleBinding" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/networking.a3s.io/v1/watch/ingresses",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let ingresses = api_state.ingresses.read().await;
                            for (_, ns_ing) in ingresses.iter() {
                                for (_, ingress) in ns_ing.iter() {
                                    let sse = format!(
                                        "event: ADDED\ndata: {}\n\n",
                                        serde_json::to_string(&ingress).unwrap_or_default()
                                    );
                                    let _ = tx.send(Ok(sse));
                                }
                            }
                        }

                        // Spawn task to forward watch events
                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Ingress" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/batch/v1/watch/jobs",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let jobs = api_state.jobs.list_all_jobs().await;
                            for (_, job) in jobs.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&job).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        // Spawn task to forward watch events
                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "Job" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/apis/batch/v1/watch/cronjobs",
            get({
                let api_state = api_state_clone.clone();
                move |query: Query<HashMap<String, String>>| {
                    let api_state = api_state.clone();
                    let watch_with_initial = query.0.get("watch") == Some(&"true".to_string());
                    async move {
                        let (tx, rx): (
                            tokio::sync::mpsc::UnboundedSender<
                                std::result::Result<String, std::convert::Infallible>,
                            >,
                            _,
                        ) = tokio::sync::mpsc::unbounded_channel();

                        // If watch=true, send initial current state as ADDED events
                        if watch_with_initial {
                            let cronjobs = api_state.cronjobs.list_all_cronjobs().await;
                            for (_, cronjob) in cronjobs.iter() {
                                let sse = format!(
                                    "event: ADDED\ndata: {}\n\n",
                                    serde_json::to_string(&cronjob).unwrap_or_default()
                                );
                                let _ = tx.send(Ok(sse));
                            }
                        }

                        // Spawn task to forward watch events
                        let receiver = api_state.watch_subscribe();
                        tokio::spawn(async move {
                            let stream = BroadcastStream::new(receiver);
                            futures_util::pin_mut!(stream);
                            while let Some(result) = stream.next().await {
                                match result {
                                    Ok(event) if event.kind == "CronJob" => {
                                        let event_type = match event.event_type {
                                            WatchEventType::Added => "ADDED",
                                            WatchEventType::Modified => "MODIFIED",
                                            WatchEventType::Deleted => "DELETED",
                                        };
                                        let sse = format!(
                                            "event: {}\ndata: {}\n\n",
                                            event_type,
                                            serde_json::to_string(&event.object)
                                                .unwrap_or_default()
                                        );
                                        let _ = tx.send(Ok(sse));
                                    }
                                    Ok(_) => {}
                                    Err(e) => {
                                        let _ =
                                            tx.send(Ok(format!("event: error\ndata: {}\n\n", e)));
                                    }
                                }
                            }
                        });

                        let stream = UnboundedReceiverStream::new(rx);

                        Response::builder()
                            .status(StatusCode::OK)
                            .header("Content-Type", "text/event-stream")
                            .header("Cache-Control", "no-cache")
                            .header("Connection", "keep-alive")
                            .header("X-Accel-Buffering", "no")
                            .body(Body::from_stream(stream))
                            .unwrap()
                    }
                }
            }),
        )
        .route(
            "/healthz",
            get({
                async fn handler() -> Response<Body> {
                    Response::builder()
                        .status(StatusCode::OK)
                        .body(Body::from(r#"{"status":"ok"}"#))
                        .unwrap()
                }
                handler
            }),
        )
        .route(
            "/readyz",
            get({
                async fn handler() -> Response<Body> {
                    Response::builder()
                        .status(StatusCode::OK)
                        .header("Content-Type", "application/json")
                        .body(Body::from(r#"{"ready":true}"#))
                        .unwrap()
                }
                handler
            }),
        )
        .layer(cors);

    // Start server
    let listener = tokio::net::TcpListener::bind(addr).await?;
    println!("API server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pattern_matching() {
        let pattern = "/api/v1/namespaces/{name}/pods";
        let path = "/api/v1/namespaces/default/pods";

        let params = ApiServer::match_pattern(pattern, path).unwrap();
        assert_eq!(params.get("name"), Some(&"default".to_string()));
    }

    #[test]
    fn test_pattern_matching_no_match() {
        let pattern = "/api/v1/namespaces/{name}/pods";
        let path = "/api/v1/namespaces/default/services";

        assert!(ApiServer::match_pattern(pattern, path).is_err());
    }
}
