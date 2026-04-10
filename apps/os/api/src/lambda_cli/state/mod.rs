//! State management for the control plane.
//!
//! Provides persistent storage for desired state (user intent) and
//! actual state (runtime reality). Used by the reconciliation loop
//! to detect and correct drift.

pub mod a3slet;
pub mod admission;
pub mod batch;
pub mod cni;
pub mod cronjob_ctrl;
pub mod csi;
pub mod daemon;
pub mod endpointslice;
pub mod event;
pub mod gc;
pub mod hpa;
pub mod lifecycle;
pub mod limitrange;
pub mod namespace;
pub mod namespace_ctrl;
pub mod network_policy;
pub mod node;
pub mod nodelease;
pub mod pdb;
pub mod priority;
pub mod proxy;
pub mod rbac;
pub mod resourcequota;
pub mod runtimeclass;
pub mod service_account;
pub mod sqlite_store;
pub mod stateful;
pub mod store;
pub mod token;
pub mod ttl;
pub mod types;
pub mod volume;
pub mod watch;

pub use batch::{
    CronJobConcurrencyPolicy, CronJobDesired, CronJobSpec, JobDesired, JobSpec, JobTemplate,
};
pub use daemon::{DaemonController, DaemonSetDesired, DaemonSetSpec};
pub use hpa::HPADesired;
pub use limitrange::{
    LimitRangeController, LimitRangeDesired, LimitRangeItem, LimitRangeSpec, LimitType,
};
pub use namespace::{Namespace, NamespaceMeta};
pub use network_policy::{
    LabelSelector, LabelSelectorExpression, NetworkPolicyDesired, NetworkPolicySpec, PolicyType,
};
pub use node::NodeDesired;
pub use pdb::{PodDisruptionBudgetDesired, PodDisruptionBudgetSpec};
pub use rbac::{
    ClusterRoleBindingDesired, ClusterRoleDesired, PolicyRule, RbacController, RoleBindingDesired,
    RoleDesired, RoleRef, Subject,
};
pub use resourcequota::{ResourceQuotaController, ResourceQuotaDesired};
pub use runtimeclass::{RuntimeClass, RuntimeClassController};
pub use service_account::{ServiceAccountController, ServiceAccountDesired};
pub use sqlite_store::{SqliteStateStore, WatchEventType};
pub use stateful::StatefulSetController;
pub use store::StateStore;
pub use token::TokenController;
pub use types::{
    CSIDriverDesired, CSIDriverSpec, CSINodeDesired, CSINodeDriver, CSINodeSpec,
    CertificateSigningRequestDesired, CertificateSigningRequestSpec, EndpointAddress, EndpointPort,
    EndpointTargetRef, EndpointsDesired, EndpointsSubset, FlowSchemaDesired, FlowSchemaRule,
    FlowSchemaSpec, FlowSchemaStatus, FlowSubject, LeaseDesired, LeaseSpec,
    MutatingWebhookConfigurationDesired, MutatingWebhookConfigurationSpec, NonResourcePolicyRule,
    PriorityClassDesired, PriorityClassSpec, PriorityLevelConfigurationDesired,
    PriorityLevelConfigurationSpec, PriorityLevelConfigurationStatus, ResourcePolicyRule,
    RuleWithOperations, ServiceReference, ValidatingWebhookConfigurationDesired,
    ValidatingWebhookConfigurationSpec, VolumeAttachmentDesired, VolumeAttachmentSource,
    VolumeAttachmentSpec, VolumeAttachmentStatus, Webhook, WebhookClientConfig,
};
pub use types::{
    ConfigMapDesired, ContainerPort, ContainerSpec, DeploymentDesired, EnvVar, HealthCheckConfig,
    HealthStatus, IngressClassDesired, IngressDesired, IngressPath, IngressRule, IngressSpec,
    IngressTls, PodActual, PodStatus, PodTemplateMetadata, PodTemplateSpec, PodTemplateSpecCore,
    PortMapping, ReplicaSetDesired, ReplicaSetSelector, ReplicaSetSpec, ResourceRequirements,
    RollingUpdateConfig, RollingUpdateState, SecretDesired, ServiceActual, ServiceDesired,
    ServicePort, ServiceType, UpdateStrategy,
};
pub use volume::{
    AccessMode, ClaimResources, PersistentVolumeClaimDesired, PersistentVolumeClaimSpec,
    ReclaimPolicy, StorageClassController, StorageClassDesired, StorageClassParameters,
    VolumeBindingMode, VolumeMode, VolumeStatus,
};
