//! `a3s get` command - List resources (kubectl get style).

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// Resource types that can be listed.
#[derive(ValueEnum, Clone, Debug)]
pub enum ResourceType {
    /// List all resource types
    All,
    /// List namespaces
    Namespaces,
    /// List nodes
    Nodes,
    /// List pods (MicroVMs)
    Pods,
    /// List services
    Services,
    /// List deployments
    Deployments,
    /// List configmaps
    ConfigMaps,
    /// List secrets
    Secrets,
    /// List events
    Events,
    /// List ingresses
    Ingresses,
    /// List statefulsets
    StatefulSets,
    /// List daemonsets
    DaemonSets,
    /// List jobs
    Jobs,
    /// List cronjobs
    CronJobs,
    /// List networkpolicies
    NetworkPolicies,
    /// List replicasets
    ReplicaSets,
    /// List persistentvolumeclaims
    PersistentVolumeClaims,
    /// List persistentvolumes
    PersistentVolumes,
    /// List serviceaccounts
    ServiceAccounts,
    /// List models
    Models,
    /// List revisions
    Revisions,
    /// List poddisruptionbudgets
    PodDisruptionBudgets,
    /// List storageclasses
    StorageClasses,
    /// List roles
    Roles,
    /// List clusterroles
    ClusterRoles,
    /// List rolebindings
    RoleBindings,
    /// List clusterrolebindings
    ClusterRoleBindings,
    /// List endpointslices
    EndpointSlices,
    /// List componentstatuses
    ComponentStatuses,
}

impl std::fmt::Display for ResourceType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::All => write!(f, "all"),
            Self::Namespaces => write!(f, "namespaces"),
            Self::Nodes => write!(f, "nodes"),
            Self::Pods => write!(f, "pods"),
            Self::Services => write!(f, "services"),
            Self::Deployments => write!(f, "deployments"),
            Self::ConfigMaps => write!(f, "configmaps"),
            Self::Secrets => write!(f, "secrets"),
            Self::Events => write!(f, "events"),
            Self::Ingresses => write!(f, "ingresses"),
            Self::StatefulSets => write!(f, "statefulsets"),
            Self::DaemonSets => write!(f, "daemonsets"),
            Self::Jobs => write!(f, "jobs"),
            Self::CronJobs => write!(f, "cronjobs"),
            Self::NetworkPolicies => write!(f, "networkpolicies"),
            Self::ReplicaSets => write!(f, "replicasets"),
            Self::PersistentVolumeClaims => write!(f, "persistentvolumeclaims"),
            Self::PersistentVolumes => write!(f, "persistentvolumes"),
            Self::ServiceAccounts => write!(f, "serviceaccounts"),
            Self::Models => write!(f, "models"),
            Self::Revisions => write!(f, "revisions"),
            Self::PodDisruptionBudgets => write!(f, "poddisruptionbudgets"),
            Self::StorageClasses => write!(f, "storageclasses"),
            Self::Roles => write!(f, "roles"),
            Self::ClusterRoles => write!(f, "clusterroles"),
            Self::RoleBindings => write!(f, "rolebindings"),
            Self::ClusterRoleBindings => write!(f, "clusterrolebindings"),
            Self::EndpointSlices => write!(f, "endpointslices"),
            Self::ComponentStatuses => write!(f, "componentstatuses"),
        }
    }
}

/// Output format options.
#[derive(ValueEnum, Clone, Debug)]
pub enum OutputFormat {
    /// Default tabular output
    Wide,
    /// JSON output
    Json,
    /// YAML output
    Yaml,
}

impl Default for OutputFormat {
    fn default() -> Self {
        Self::Wide
    }
}

/// List resources (kubectl get style).
#[derive(Parser, Debug)]
pub struct GetCommand {
    /// Resource type to list.
    #[arg(default_value = "all")]
    resource: ResourceType,

    /// Show all resources including stopped/failed.
    #[arg(short, long)]
    all: bool,

    /// Watch for changes.
    #[arg(short, long)]
    watch: bool,

    /// Output format.
    #[arg(short, long, value_parser = clap::value_parser!(OutputFormat))]
    output: Option<OutputFormat>,

    /// Show only resources in this namespace.
    #[arg(short, long)]
    namespace: Option<String>,
}

impl GetCommand {
    /// Get the a3s home directory.
    fn boxes_dir() -> PathBuf {
        std::env::var("A3S_HOME")
            .ok()
            .map(PathBuf::from)
            .or_else(|| dirs::home_dir().map(|h| h.join(".a3s")))
            .unwrap_or_else(|| PathBuf::from("~/.a3s"))
            .join("boxes")
    }

    /// List all pods (MicroVMs) from boxes directory.
    fn list_pods(&self) -> Vec<PodInfo> {
        let boxes_dir = Self::boxes_dir();
        if !boxes_dir.exists() {
            return Vec::new();
        }

        let mut pods = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let socket_path = path.join("sockets").join("exec.sock");
                    let running = socket_path.exists();

                    if self.all || running {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Try to read image from info.json first
                        let info_path = path.join("info.json");
                        let image = if info_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&info_path) {
                                if let Ok(info) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    info.get("image").and_then(|v| v.as_str()).map(String::from)
                                } else {
                                    None
                                }
                            } else {
                                None
                            }
                        } else {
                            None
                        }
                        .unwrap_or_else(|| {
                            // Fall back to log extraction
                            let log_path = path.join("logs").join("console.log");
                            Self::extract_image_from_logs(&log_path)
                                .unwrap_or_else(|| "unknown".to_string())
                        });

                        let created = std::fs::metadata(&path)
                            .and_then(|m| m.created())
                            .ok()
                            .map(|t| {
                                chrono::DateTime::<chrono::Utc>::from(t)
                                    .format("%Y-%m-%d %H:%M:%S")
                                    .to_string()
                            })
                            .unwrap_or_else(|| "unknown".to_string());

                        pods.push(PodInfo {
                            name,
                            image,
                            status: if running { "Running" } else { "Stopped" }.to_string(),
                            created,
                            namespace: "default".to_string(),
                        });
                    }
                }
            }
        }
        pods
    }

    fn extract_image_from_logs(log_path: &PathBuf) -> Option<String> {
        if !log_path.exists() {
            return None;
        }
        let content = std::fs::read_to_string(log_path).ok()?;

        // Try to find "reference=image:tag" pattern
        for line in content.lines() {
            if let Some(idx) = line.find("reference=") {
                let after_ref = &line[idx + "reference=".len()..];
                // Handle both "nginx:alpine" and "docker.io/library/nginx:alpine" formats
                let image = after_ref.trim().trim_matches('"');
                if !image.is_empty() && image != "reference" {
                    // Simplify docker.io/library/nginx:alpine -> nginx:alpine
                    if image.starts_with("docker.io/library/") {
                        return Some(image["docker.io/library/".len()..].to_string());
                    } else if image.starts_with("registry-1.docker.io/library/") {
                        return Some(image["registry-1.docker.io/library/".len()..].to_string());
                    }
                    return Some(image.to_string());
                }
            }
        }

        // Fallback: look for any image-like pattern
        for line in content.lines() {
            if line.contains("image") && line.contains(":") {
                if let Some(idx) = line.find("image") {
                    let after = &line[idx + 5..];
                    if after.starts_with('=') || after.starts_with(':') {
                        let image = after.trim_start_matches(&['=', ':', ' ', '"'][..]);
                        let parts: Vec<&str> = image.split_whitespace().collect();
                        if !parts.is_empty() {
                            let img = parts[0].trim_matches('"');
                            if img.contains(":") && !img.contains(" ") {
                                return Some(img.to_string());
                            }
                        }
                    }
                }
            }
        }

        None
    }

    /// Print pods in tabular format.
    fn print_pods(&self, pods: &[PodInfo]) {
        if pods.is_empty() {
            println!("No pods found.");
            return;
        }

        println!(
            "{:<36} {:<25} {:<12} {:<20} {}",
            "NAME", "IMAGE", "STATUS", "CREATED", "NAMESPACE"
        );
        println!("{}", "-".repeat(100));

        for pod in pods {
            println!(
                "{:<36} {:<25} {:<12} {:<20} {}",
                pod.name, pod.image, pod.status, pod.created, pod.namespace
            );
        }
        println!();
        println!("Total: {} pods", pods.len());
    }

    /// Print pods in JSON format.
    fn print_pods_json(&self, pods: &[PodInfo]) {
        let json = serde_json::to_string_pretty(pods).unwrap_or_else(|_| "[]".to_string());
        println!("{}", json);
    }

    /// List services from boxes directory.
    fn list_services(&self) -> Vec<ServiceInfo> {
        let boxes_dir = Self::boxes_dir();
        if !boxes_dir.exists() {
            return Vec::new();
        }

        let mut services = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let socket_path = path.join("sockets").join("exec.sock");
                    let running = socket_path.exists();

                    if self.all || running {
                        // Read info.json to get service details
                        let info_path = path.join("info.json");
                        if info_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&info_path) {
                                if let Ok(info) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    let name = info
                                        .get("name")
                                        .and_then(|v| v.as_str())
                                        .map(String::from)
                                        .unwrap_or_else(|| {
                                            path.file_name()
                                                .map(|n| n.to_string_lossy().to_string())
                                                .unwrap_or_default()
                                        });

                                    let service_type = "ClusterIP".to_string();
                                    let ports = info
                                        .get("ports")
                                        .and_then(|v| v.as_object())
                                        .map(|obj| {
                                            obj.iter()
                                                .map(|(_k, v)| {
                                                    format!("{}/TCP", v.as_u64().unwrap_or(0))
                                                })
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        })
                                        .unwrap_or_else(|| "80/TCP".to_string());

                                    let selector = info
                                        .get("labels")
                                        .and_then(|v| v.as_object())
                                        .map(|obj| {
                                            obj.iter()
                                                .map(|(k, v)| format!("{}={}", k, v))
                                                .collect::<Vec<_>>()
                                                .join(", ")
                                        })
                                        .unwrap_or_default();

                                    services.push(ServiceInfo {
                                        name,
                                        service_type,
                                        ports,
                                        selector,
                                        status: if running { "Running" } else { "Stopped" }
                                            .to_string(),
                                    });
                                }
                            }
                        }
                    }
                }
            }
        }
        services
    }

    /// List models from boxes directory.
    fn list_models(&self) -> Vec<ModelInfo> {
        let boxes_dir = Self::boxes_dir();
        if !boxes_dir.exists() {
            return Vec::new();
        }

        let mut models = Vec::new();
        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let socket_path = path.join("sockets").join("exec.sock");
                    let running = socket_path.exists();

                    // Check if this is a model by looking at info.json or name pattern
                    let info_path = path.join("info.json");
                    if info_path.exists() || self.all || running {
                        let name = path
                            .file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_default();

                        // Skip if name starts with "a3s-dep-" (dependency, not model)
                        if name.starts_with("a3s-dep-") {
                            continue;
                        }

                        // Check image to determine if this is a model
                        let (image, provider) = if info_path.exists() {
                            if let Ok(content) = std::fs::read_to_string(&info_path) {
                                if let Ok(info) =
                                    serde_json::from_str::<serde_json::Value>(&content)
                                {
                                    let img = info
                                        .get("image")
                                        .and_then(|v| v.as_str())
                                        .map(String::from)
                                        .unwrap_or_else(|| "unknown".to_string());

                                    let prov = if img.contains("ollama") {
                                        "ollama"
                                    } else if img.contains("vllm") {
                                        "vllm"
                                    } else {
                                        // Skip non-model images (nginx, alpine, etc.)
                                        continue;
                                    };

                                    (img, prov.to_string())
                                } else {
                                    continue;
                                }
                            } else {
                                continue;
                            }
                        } else {
                            continue;
                        };

                        let replicas = info_path
                            .exists()
                            .then(|| {
                                std::fs::read_to_string(&info_path)
                                    .ok()
                                    .and_then(|c| {
                                        serde_json::from_str::<serde_json::Value>(&c).ok()
                                    })
                                    .and_then(|info| info.get("replicas")?.as_i64())
                            })
                            .flatten()
                            .unwrap_or(1) as i32;

                        models.push(ModelInfo {
                            name,
                            image,
                            provider,
                            status: if running { "Running" } else { "Stopped" }.to_string(),
                            replicas,
                        });
                    }
                }
            }
        }
        models
    }

    /// List configmaps from state store.
    fn list_configmaps(&self) -> Vec<ConfigMapInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_configmaps() {
            Ok(configmaps) => configmaps
                .into_values()
                .map(|cm| {
                    let keys = cm
                        .data
                        .keys()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let age = format_timestamp(&cm.created_at);
                    ConfigMapInfo {
                        name: cm.name,
                        namespace: cm.namespace,
                        data_keys: if keys.len() > 30 {
                            format!("{}...", &keys[..30.min(keys.len())])
                        } else {
                            keys
                        },
                        immutable: cm.immutable,
                        age,
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List secrets from state store.
    fn list_secrets(&self) -> Vec<SecretInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_secrets() {
            Ok(secrets) => secrets
                .into_values()
                .map(|s| {
                    let keys = s
                        .data
                        .keys()
                        .map(|k| k.as_str())
                        .collect::<Vec<_>>()
                        .join(", ");
                    let age = format_timestamp(&s.created_at);
                    SecretInfo {
                        name: s.name,
                        namespace: s.namespace,
                        secret_type: s.secret_type,
                        data_keys: if keys.len() > 30 {
                            format!("{}...", &keys[..30.min(keys.len())])
                        } else {
                            keys
                        },
                        immutable: s.immutable,
                        age,
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List namespaces from state store.
    fn list_namespaces(&self) -> Vec<NamespaceInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_namespaces() {
            Ok(namespaces) => namespaces
                .into_values()
                .map(|ns| NamespaceInfo {
                    name: ns.name,
                    status: "Active".to_string(),
                    age: format_timestamp(&ns.created_at),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List nodes from state store.
    fn list_nodes(&self) -> Vec<NodeInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_nodes() {
            Ok(nodes) => nodes
                .into_values()
                .map(|n| NodeInfo {
                    name: n.name,
                    status: "Ready".to_string(),
                    roles: "worker".to_string(),
                    age: format_timestamp(&n.created_at),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List deployments from state store.
    fn list_deployments(&self) -> Vec<DeploymentInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_deployments() {
            Ok(deployments) => deployments
                .into_values()
                .map(|d| DeploymentInfo {
                    name: d.name,
                    namespace: d.namespace,
                    ready: format!("0/{}", d.replicas),
                    up_to_date: 0,
                    available: 0,
                    age: format_timestamp(&d.created_at),
                    image: d.image,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List events from state store.
    fn list_events(&self) -> Vec<EventInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_events() {
            Ok(events) => events
                .into_iter()
                .map(|e| EventInfo {
                    namespace: e.involved_object.namespace.clone().unwrap_or_default(),
                    last_seen: format_timestamp(&e.last_timestamp),
                    level: format!("{:?}", e.event_type),
                    reason: format!("{:?}", e.reason),
                    object: format!("{}/{}", e.involved_object.kind, e.involved_object.name),
                    message: e.message,
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List ingresses from state store.
    fn list_ingresses(&self) -> Vec<IngressInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_ingresses() {
            Ok(ingresses) => ingresses
                .into_values()
                .flat_map(|ns_ing| {
                    ns_ing
                        .into_values()
                        .map(|i| {
                            let hosts = i
                                .spec
                                .rules
                                .iter()
                                .map(|r| r.host.clone())
                                .collect::<Vec<_>>()
                                .join(",");
                            IngressInfo {
                                name: i.name,
                                namespace: i.namespace,
                                class: "a3s".to_string(),
                                hosts,
                                address: "<none>".to_string(),
                                ports: "80".to_string(),
                                age: format_timestamp(&i.created_at),
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List statefulsets from state store.
    fn list_statefulsets(&self) -> Vec<StatefulSetInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_statefulsets() {
            Ok(statefulsets) => statefulsets
                .into_values()
                .flat_map(|ns_ss| {
                    ns_ss
                        .into_values()
                        .map(|ss| StatefulSetInfo {
                            name: ss.name,
                            namespace: ss.namespace,
                            ready: format!("0/{}", ss.spec.replicas),
                            age: format_timestamp(&ss.created_at),
                            image: ss.spec.template.spec.image.clone(),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List daemonsets from state store.
    fn list_daemonsets(&self) -> Vec<DaemonSetInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_daemonsets() {
            Ok(daemonsets) => daemonsets
                .into_values()
                .map(|ds| DaemonSetInfo {
                    name: ds.name,
                    namespace: ds.namespace.clone(),
                    desired: ds.spec.replicas,
                    current: 0,
                    ready: 0,
                    available: 0,
                    age: format_timestamp(&ds.created_at),
                    image: ds.spec.template.spec.image.clone(),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List jobs from state store.
    fn list_jobs(&self) -> Vec<JobInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_jobs() {
            Ok(jobs) => jobs
                .into_values()
                .flat_map(|ns_jobs| {
                    ns_jobs
                        .into_values()
                        .map(|j| JobInfo {
                            name: j.name,
                            namespace: j.namespace,
                            status: "Active".to_string(),
                            completions: format!("0/{}", j.spec.completions.unwrap_or(1)),
                            duration: "<none>".to_string(),
                            age: format_timestamp(&j.created_at),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List cronjobs from state store.
    fn list_cronjobs(&self) -> Vec<CronJobInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_cronjobs() {
            Ok(cronjobs) => cronjobs
                .into_values()
                .flat_map(|ns_cj| {
                    ns_cj
                        .into_values()
                        .map(|cj| CronJobInfo {
                            name: cj.name,
                            namespace: cj.namespace,
                            schedule: cj.spec.schedule.format(),
                            suspend: cj.spec.suspend,
                            active: 0,
                            last_schedule: "<never>".to_string(),
                            age: format_timestamp(&cj.created_at),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List networkpolicies from state store.
    fn list_networkpolicies(&self) -> Vec<NetworkPolicyInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_network_policies() {
            Ok(policies) => policies
                .into_iter()
                .map(|p| {
                    let pod_selector = p
                        .spec
                        .pod_selector
                        .match_labels
                        .iter()
                        .map(|(k, v)| format!("{}={}", k, v))
                        .collect::<Vec<_>>()
                        .join(",");
                    let policy_types = p
                        .spec
                        .policy_types
                        .iter()
                        .map(|pt| format!("{:?}", pt))
                        .collect::<Vec<_>>()
                        .join(",");
                    NetworkPolicyInfo {
                        name: p.name,
                        namespace: p.namespace,
                        pod_selector,
                        policy_types,
                        age: format_timestamp(&p.created_at),
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List replicasets from state store.
    fn list_replicasets(&self) -> Vec<ReplicaSetInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_replicasets() {
            Ok(replicasets) => replicasets
                .into_values()
                .flat_map(|ns_rs| {
                    ns_rs
                        .into_values()
                        .map(|rs| ReplicaSetInfo {
                            name: rs.name,
                            namespace: rs.namespace,
                            desired: rs.spec.replicas.unwrap_or(0),
                            current: 0,
                            ready: 0,
                            age: format_timestamp(&rs.created_at),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List persistentvolumeclaims from state store.
    fn list_pvcs(&self) -> Vec<PVCInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_persistent_volume_claims() {
            Ok(pvcs) => pvcs
                .into_values()
                .flat_map(|ns_pvc| {
                    ns_pvc
                        .into_values()
                        .map(|pvc| PVCInfo {
                            name: pvc.name,
                            namespace: pvc.namespace,
                            status: format!("{:?}", pvc.status),
                            storage_class: pvc.spec.storage_class_name.unwrap_or_default(),
                            capacity: pvc
                                .spec
                                .resources
                                .requests
                                .get("storage")
                                .map(|s| s.to_string())
                                .unwrap_or_default(),
                            access_modes: pvc
                                .spec
                                .access_modes
                                .iter()
                                .map(|m| format!("{:?}", m))
                                .collect::<Vec<_>>()
                                .join(","),
                            age: format_timestamp(&pvc.created_at),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List persistentvolumes from state store.
    fn list_pvs(&self) -> Vec<PVInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_persistent_volumes() {
            Ok(pvs) => pvs
                .into_values()
                .map(|pv| {
                    let capacity = format_bytes(pv.spec.capacity);
                    let access_modes = pv
                        .spec
                        .access_modes
                        .iter()
                        .map(|m| format!("{:?}", m))
                        .collect::<Vec<_>>()
                        .join(",");
                    PVInfo {
                        name: pv.name,
                        status: format!("{:?}", pv.status),
                        storage_class: pv.spec.storage_class.unwrap_or_default(),
                        capacity,
                        access_modes,
                        claim: pv.claim_name.unwrap_or_default(),
                        age: format_timestamp(&pv.created_at),
                    }
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List serviceaccounts from state store.
    fn list_serviceaccounts(&self) -> Vec<ServiceAccountInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_service_accounts() {
            Ok(sas) => sas
                .into_values()
                .flat_map(|ns_sa| {
                    ns_sa
                        .into_values()
                        .map(|sa| {
                            ServiceAccountInfo {
                                name: sa.name,
                                namespace: sa.namespace,
                                tokens: 0, // Desired state doesn't track tokens
                                age: format_timestamp(&sa.created_at),
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// List poddisruptionbudgets from state store.
    fn list_poddisruptionbudgets(&self) -> Vec<PDBInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_pod_disruption_budgets() {
            Ok(pdbs) => pdbs
                .into_iter()
                .flat_map(|(ns, ns_pdbs)| {
                    let ns_clone = ns.clone();
                    ns_pdbs
                        .into_iter()
                        .map(move |(name, pdb)| {
                            let min_available = pdb.spec.min_available.clone().unwrap_or_default();
                            let max_disruptions =
                                pdb.spec.max_disruptions.clone().unwrap_or_default();
                            PDBInfo {
                                name,
                                namespace: ns_clone.clone(),
                                min_available,
                                max_disruptions,
                                age: format_timestamp(&pdb.created_at),
                            }
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Print services in tabular format.
    fn print_services(&self, services: &[ServiceInfo]) {
        if services.is_empty() {
            println!("No services found.");
            return;
        }

        println!(
            "{:<25} {:<15} {:<30} {:<40} {}",
            "NAME", "TYPE", "PORTS", "SELECTOR", "STATUS"
        );
        println!("{}", "-".repeat(120));

        for svc in services {
            println!(
                "{:<25} {:<15} {:<30} {:<40} {}",
                svc.name, svc.service_type, svc.ports, svc.selector, svc.status
            );
        }
        println!();
        println!("Total: {} services", services.len());
    }

    /// Print models in tabular format.
    fn print_models(&self, models: &[ModelInfo]) {
        if models.is_empty() {
            println!("No models found.");
            return;
        }

        println!(
            "{:<36} {:<30} {:<15} {:<10} {}",
            "NAME", "IMAGE", "PROVIDER", "REPLICAS", "STATUS"
        );
        println!("{}", "-".repeat(100));

        for model in models {
            println!(
                "{:<36} {:<30} {:<15} {:<10} {}",
                model.name, model.image, model.provider, model.replicas, model.status
            );
        }
        println!();
        println!("Total: {} models", models.len());
    }

    /// Print configmaps in tabular format.
    fn print_configmaps(&self, configmaps: &[ConfigMapInfo]) {
        if configmaps.is_empty() {
            println!("No configmaps found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<35} {:<10} {}",
            "NAME", "NAMESPACE", "DATA KEYS", "IMMUTABLE", "AGE"
        );
        println!("{}", "-".repeat(100));

        for cm in configmaps {
            println!(
                "{:<30} {:<15} {:<35} {:<10} {}",
                cm.name, cm.namespace, cm.data_keys, cm.immutable, cm.age
            );
        }
        println!();
        println!("Total: {} configmaps", configmaps.len());
    }

    /// Print secrets in tabular format.
    fn print_secrets(&self, secrets: &[SecretInfo]) {
        if secrets.is_empty() {
            println!("No secrets found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<20} {:<35} {:<10} {}",
            "NAME", "NAMESPACE", "TYPE", "DATA KEYS", "IMMUTABLE", "AGE"
        );
        println!("{}", "-".repeat(110));

        for secret in secrets {
            println!(
                "{:<30} {:<15} {:<20} {:<35} {:<10} {}",
                secret.name,
                secret.namespace,
                secret.secret_type,
                secret.data_keys,
                secret.immutable,
                secret.age
            );
        }
        println!();
        println!("Total: {} secrets", secrets.len());
    }

    /// Print namespaces in tabular format.
    fn print_namespaces(&self, namespaces: &[NamespaceInfo]) {
        if namespaces.is_empty() {
            println!("No namespaces found.");
            return;
        }

        println!("{:<30} {:<15} {}", "NAME", "STATUS", "AGE");
        println!("{}", "-".repeat(55));

        for ns in namespaces {
            println!("{:<30} {:<15} {}", ns.name, ns.status, ns.age);
        }
        println!();
        println!("Total: {} namespaces", namespaces.len());
    }

    /// Print nodes in tabular format.
    fn print_nodes(&self, nodes: &[NodeInfo]) {
        if nodes.is_empty() {
            println!("No nodes found.");
            return;
        }

        println!("{:<30} {:<10} {:<10} {}", "NAME", "STATUS", "ROLES", "AGE");
        println!("{}", "-".repeat(60));

        for node in nodes {
            println!(
                "{:<30} {:<10} {:<10} {}",
                node.name, node.status, node.roles, node.age
            );
        }
        println!();
        println!("Total: {} nodes", nodes.len());
    }

    /// Print deployments in tabular format.
    fn print_deployments(&self, deployments: &[DeploymentInfo]) {
        if deployments.is_empty() {
            println!("No deployments found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {:<10} {:<10} {}",
            "NAME", "NAMESPACE", "READY", "UP-TO-DATE", "AVAILABLE", "AGE"
        );
        println!("{}", "-".repeat(90));

        for d in deployments {
            println!(
                "{:<30} {:<15} {:<10} {:<10} {:<10} {}",
                d.name, d.namespace, d.ready, d.up_to_date, d.available, d.age
            );
        }
        println!();
        println!("Total: {} deployments", deployments.len());
    }

    /// Print events in tabular format.
    fn print_events(&self, events: &[EventInfo]) {
        if events.is_empty() {
            println!("No events found.");
            return;
        }

        println!(
            "{:<15} {:<20} {:<8} {:<15} {:<30} {}",
            "NAMESPACE", "LAST SEEN", "LEVEL", "REASON", "OBJECT", "MESSAGE"
        );
        println!("{}", "-".repeat(100));

        for e in events {
            let msg = if e.message.len() > 50 {
                format!("{}...", &e.message[..50])
            } else {
                e.message.clone()
            };
            println!(
                "{:<15} {:<20} {:<8} {:<15} {:<30} {}",
                e.namespace, e.last_seen, e.level, e.reason, e.object, msg
            );
        }
        println!();
        println!("Total: {} events", events.len());
    }

    /// Print ingresses in tabular format.
    fn print_ingresses(&self, ingresses: &[IngressInfo]) {
        if ingresses.is_empty() {
            println!("No ingresses found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<15} {:<30} {:<15} {:<10} {}",
            "NAME", "NAMESPACE", "CLASS", "HOSTS", "ADDRESS", "PORTS", "AGE"
        );
        println!("{}", "-".repeat(130));

        for ing in ingresses {
            println!(
                "{:<30} {:<15} {:<15} {:<30} {:<15} {:<10} {}",
                ing.name, ing.namespace, ing.class, ing.hosts, ing.address, ing.ports, ing.age
            );
        }
        println!();
        println!("Total: {} ingresses", ingresses.len());
    }

    /// Print statefulsets in tabular format.
    fn print_statefulsets(&self, statefulsets: &[StatefulSetInfo]) {
        if statefulsets.is_empty() {
            println!("No statefulsets found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {}",
            "NAME", "NAMESPACE", "READY", "AGE"
        );
        println!("{}", "-".repeat(70));

        for ss in statefulsets {
            println!(
                "{:<30} {:<15} {:<10} {}",
                ss.name, ss.namespace, ss.ready, ss.age
            );
        }
        println!();
        println!("Total: {} statefulsets", statefulsets.len());
    }

    /// Print daemonsets in tabular format.
    fn print_daemonsets(&self, daemonsets: &[DaemonSetInfo]) {
        if daemonsets.is_empty() {
            println!("No daemonsets found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {:<10} {:<10} {:<10} {}",
            "NAME", "NAMESPACE", "DESIRED", "CURRENT", "READY", "AVAILABLE", "AGE"
        );
        println!("{}", "-".repeat(90));

        for ds in daemonsets {
            println!(
                "{:<30} {:<15} {:<10} {:<10} {:<10} {:<10} {}",
                ds.name, ds.namespace, ds.desired, ds.current, ds.ready, ds.available, ds.age
            );
        }
        println!();
        println!("Total: {} daemonsets", daemonsets.len());
    }

    /// Print jobs in tabular format.
    fn print_jobs(&self, jobs: &[JobInfo]) {
        if jobs.is_empty() {
            println!("No jobs found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {:<15} {:<15} {}",
            "NAME", "NAMESPACE", "STATUS", "COMPLETIONS", "DURATION", "AGE"
        );
        println!("{}", "-".repeat(90));

        for job in jobs {
            println!(
                "{:<30} {:<15} {:<10} {:<15} {:<15} {}",
                job.name, job.namespace, job.status, job.completions, job.duration, job.age
            );
        }
        println!();
        println!("Total: {} jobs", jobs.len());
    }

    /// Print cronjobs in tabular format.
    fn print_cronjobs(&self, cronjobs: &[CronJobInfo]) {
        if cronjobs.is_empty() {
            println!("No cronjobs found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<20} {:<10} {:<10} {:<15} {}",
            "NAME", "NAMESPACE", "SCHEDULE", "SUSPEND", "ACTIVE", "LAST SCHEDULE", "AGE"
        );
        println!("{}", "-".repeat(110));

        for cj in cronjobs {
            println!(
                "{:<30} {:<15} {:<20} {:<10} {:<10} {:<15} {}",
                cj.name, cj.namespace, cj.schedule, cj.suspend, cj.active, cj.last_schedule, cj.age
            );
        }
        println!();
        println!("Total: {} cronjobs", cronjobs.len());
    }

    /// Print networkpolicies in tabular format.
    fn print_networkpolicies(&self, networkpolicies: &[NetworkPolicyInfo]) {
        if networkpolicies.is_empty() {
            println!("No networkpolicies found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<30} {:<20} {}",
            "NAME", "NAMESPACE", "POD-SELECTOR", "POLICY-TYPES", "AGE"
        );
        println!("{}", "-".repeat(100));

        for np in networkpolicies {
            println!(
                "{:<30} {:<15} {:<30} {:<20} {}",
                np.name, np.namespace, np.pod_selector, np.policy_types, np.age
            );
        }
        println!();
        println!("Total: {} networkpolicies", networkpolicies.len());
    }

    /// Print replicasets in tabular format.
    fn print_replicasets(&self, replicasets: &[ReplicaSetInfo]) {
        if replicasets.is_empty() {
            println!("No replicasets found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {:<10} {:<10} {}",
            "NAME", "NAMESPACE", "DESIRED", "CURRENT", "READY", "AGE"
        );
        println!("{}", "-".repeat(80));

        for rs in replicasets {
            println!(
                "{:<30} {:<15} {:<10} {:<10} {:<10} {}",
                rs.name, rs.namespace, rs.desired, rs.current, rs.ready, rs.age
            );
        }
        println!();
        println!("Total: {} replicasets", replicasets.len());
    }

    /// Print persistentvolumeclaims in tabular format.
    fn print_pvcs(&self, pvcs: &[PVCInfo]) {
        if pvcs.is_empty() {
            println!("No persistentvolumeclaims found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<15} {:<20} {:<15} {:<15} {}",
            "NAME", "NAMESPACE", "STATUS", "STORAGECLASS", "CAPACITY", "ACCESS MODES", "AGE"
        );
        println!("{}", "-".repeat(120));

        for pvc in pvcs {
            println!(
                "{:<30} {:<15} {:<15} {:<20} {:<15} {:<15} {}",
                pvc.name,
                pvc.namespace,
                pvc.status,
                pvc.storage_class,
                pvc.capacity,
                pvc.access_modes,
                pvc.age
            );
        }
        println!();
        println!("Total: {} persistentvolumeclaims", pvcs.len());
    }

    /// Print persistentvolumes in tabular format.
    fn print_pvs(&self, pvs: &[PVInfo]) {
        if pvs.is_empty() {
            println!("No persistentvolumes found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<20} {:<15} {:<15} {:<20} {}",
            "NAME", "STATUS", "STORAGECLASS", "CAPACITY", "ACCESS MODES", "CLAIM", "AGE"
        );
        println!("{}", "-".repeat(130));

        for pv in pvs {
            println!(
                "{:<30} {:<15} {:<20} {:<15} {:<15} {:<20} {}",
                pv.name,
                pv.status,
                pv.storage_class,
                pv.capacity,
                pv.access_modes,
                pv.claim,
                pv.age
            );
        }
        println!();
        println!("Total: {} persistentvolumes", pvs.len());
    }

    /// Print serviceaccounts in tabular format.
    fn print_serviceaccounts(&self, serviceaccounts: &[ServiceAccountInfo]) {
        if serviceaccounts.is_empty() {
            println!("No serviceaccounts found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {}",
            "NAME", "NAMESPACE", "TOKENS", "AGE"
        );
        println!("{}", "-".repeat(70));

        for sa in serviceaccounts {
            println!(
                "{:<30} {:<15} {:<10} {}",
                sa.name, sa.namespace, sa.tokens, sa.age
            );
        }
        println!();
        println!("Total: {} serviceaccounts", serviceaccounts.len());
    }

    /// Print poddisruptionbudgets in tabular format.
    fn print_poddisruptionbudgets(&self, pdbs: &[PDBInfo]) {
        if pdbs.is_empty() {
            println!("No poddisruptionbudgets found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<20} {:<20} {}",
            "NAME", "NAMESPACE", "MIN-AVAILABLE", "MAX-DISRUPTIONS", "AGE"
        );
        println!("{}", "-".repeat(90));

        for pdb in pdbs {
            println!(
                "{:<30} {:<15} {:<20} {:<20} {}",
                pdb.name, pdb.namespace, pdb.min_available, pdb.max_disruptions, pdb.age
            );
        }
        println!();
        println!("Total: {} poddisruptionbudgets", pdbs.len());
    }

    /// List storageclasses from state store.
    fn list_storageclasses(&self) -> Vec<StorageClassInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_storage_classes() {
            Ok(scs) => scs
                .into_values()
                .map(|sc| StorageClassInfo {
                    name: sc.name,
                    provisioner: sc.provisioner,
                    is_default: if sc.is_default { "true" } else { "false" }.to_string(),
                    age: format_timestamp(&sc.created_at),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Print storageclasses in tabular format.
    fn print_storageclasses(&self, scs: &[StorageClassInfo]) {
        if scs.is_empty() {
            println!("No storageclasses found.");
            return;
        }

        println!(
            "{:<30} {:<25} {:<10} {}",
            "NAME", "PROVISIONER", "DEFAULT", "AGE"
        );
        println!("{}", "-".repeat(70));

        for sc in scs {
            println!(
                "{:<30} {:<25} {:<10} {}",
                sc.name, sc.provisioner, sc.is_default, sc.age
            );
        }
        println!();
        println!("Total: {} storageclasses", scs.len());
    }

    /// List roles from state store.
    fn list_roles(&self) -> Vec<RoleInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_roles() {
            Ok(roles) => roles
                .into_iter()
                .flat_map(|(ns, ns_roles)| {
                    ns_roles
                        .into_iter()
                        .map(|(name, role)| RoleInfo {
                            name,
                            namespace: ns.clone(),
                            rules_count: role.rules.len(),
                            age: format_timestamp(&role.created_at),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Print roles in tabular format.
    fn print_roles(&self, roles: &[RoleInfo]) {
        if roles.is_empty() {
            println!("No roles found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<10} {}",
            "NAME", "NAMESPACE", "RULES", "AGE"
        );
        println!("{}", "-".repeat(70));

        for role in roles {
            println!(
                "{:<30} {:<15} {:<10} {}",
                role.name, role.namespace, role.rules_count, role.age
            );
        }
        println!();
        println!("Total: {} roles", roles.len());
    }

    /// List clusterroles from state store.
    fn list_cluster_roles(&self) -> Vec<ClusterRoleInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_cluster_roles() {
            Ok(roles) => roles
                .into_values()
                .map(|role| ClusterRoleInfo {
                    name: role.name,
                    rules_count: role.rules.len(),
                    age: format_timestamp(&role.created_at),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Print clusterroles in tabular format.
    fn print_cluster_roles(&self, roles: &[ClusterRoleInfo]) {
        if roles.is_empty() {
            println!("No clusterroles found.");
            return;
        }

        println!("{:<30} {:<10} {}", "NAME", "RULES", "AGE");
        println!("{}", "-".repeat(50));

        for role in roles {
            println!("{:<30} {:<10} {}", role.name, role.rules_count, role.age);
        }
        println!();
        println!("Total: {} clusterroles", roles.len());
    }

    /// List role bindings.
    fn list_role_bindings(&self) -> Vec<RoleBindingInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_role_bindings() {
            Ok(role_bindings) => role_bindings
                .into_iter()
                .flat_map(|(ns, bindings)| {
                    bindings
                        .into_iter()
                        .map(move |(name, rb)| RoleBindingInfo {
                            name,
                            namespace: ns.clone(),
                            role_kind: rb.role_ref.kind.clone(),
                            role_name: rb.role_ref.name.clone(),
                            subjects_count: rb.subjects.len(),
                            age: format_timestamp(&rb.created_at),
                        })
                        .collect::<Vec<_>>()
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Print role bindings in tabular format.
    fn print_role_bindings(&self, role_bindings: &[RoleBindingInfo]) {
        if role_bindings.is_empty() {
            println!("No rolebindings found.");
            return;
        }

        println!(
            "{:<25} {:<20} {:<15} {:<20} {:<10} {}",
            "NAME", "NAMESPACE", "ROLE-KIND", "ROLE-NAME", "SUBJECTS", "AGE"
        );
        println!("{}", "-".repeat(100));

        for rb in role_bindings {
            println!(
                "{:<25} {:<20} {:<15} {:<20} {:<10} {}",
                rb.name, rb.namespace, rb.role_kind, rb.role_name, rb.subjects_count, rb.age
            );
        }
        println!();
        println!("Total: {} rolebindings", role_bindings.len());
    }

    /// List cluster role bindings.
    fn list_cluster_role_bindings(&self) -> Vec<ClusterRoleBindingInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_cluster_role_bindings() {
            Ok(bindings) => bindings
                .into_values()
                .map(|rb| ClusterRoleBindingInfo {
                    name: rb.name.clone(),
                    role_kind: rb.role_ref.kind.clone(),
                    role_name: rb.role_ref.name.clone(),
                    subjects_count: rb.subjects.len(),
                    age: format_timestamp(&rb.created_at),
                })
                .collect(),
            Err(_) => Vec::new(),
        }
    }

    /// Print cluster role bindings in tabular format.
    fn print_cluster_role_bindings(&self, bindings: &[ClusterRoleBindingInfo]) {
        if bindings.is_empty() {
            println!("No clusterrolebindings found.");
            return;
        }

        println!(
            "{:<30} {:<15} {:<20} {:<10} {}",
            "NAME", "ROLE-KIND", "ROLE-NAME", "SUBJECTS", "AGE"
        );
        println!("{}", "-".repeat(80));

        for rb in bindings {
            println!(
                "{:<30} {:<15} {:<20} {:<10} {}",
                rb.name, rb.role_kind, rb.role_name, rb.subjects_count, rb.age
            );
        }
        println!();
        println!("Total: {} clusterrolebindings", bindings.len());
    }

    /// List endpoint slices.
    fn list_endpoint_slices(&self) -> Vec<EndpointSliceInfo> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        // EndpointSlice data is derived from services
        // For now, return empty list as EndpointSlice doesn't have its own store
        let _ = store;
        Vec::new()
    }

    /// Print endpoint slices in tabular format.
    fn print_endpoint_slices(&self, eps: &[EndpointSliceInfo]) {
        if eps.is_empty() {
            println!("No endpointslices found.");
            return;
        }

        println!(
            "{:<30} {:<20} {:<20} {:<10} {}",
            "NAME", "NAMESPACE", "SERVICE", "ENDPOINTS", "AGE"
        );
        println!("{}", "-".repeat(90));

        for ep in eps {
            println!(
                "{:<30} {:<20} {:<20} {:<10} {}",
                ep.name, ep.namespace, ep.service_name, ep.endpoints_count, ep.age
            );
        }
        println!();
        println!("Total: {} endpointslices", eps.len());
    }

    /// Print component statuses.
    fn print_component_statuses(&self) {
        println!("{:<40} {:<15} {:<15}", "NAME", "STATUS", "MESSAGE");
        println!("{}", "-".repeat(70));

        // List known component statuses
        let components = vec![
            ("scheduler", "Healthy", "ok"),
            ("controller-manager", "Healthy", "ok"),
            ("etcd-0", "Healthy", "ok"),
        ];

        for (name, status, message) in &components {
            println!("{:<40} {:<15} {:<15}", name, status, message);
        }

        println!();
        println!("Total: {} componentstatuses", components.len());
    }
}

/// Format a timestamp for display.
fn format_timestamp(dt: &chrono::DateTime<chrono::Utc>) -> String {
    let now = chrono::Utc::now();
    let duration = now.signed_duration_since(*dt);

    if duration.num_seconds() < 60 {
        format!("{}s ago", duration.num_seconds())
    } else if duration.num_minutes() < 60 {
        format!("{}m ago", duration.num_minutes())
    } else if duration.num_hours() < 24 {
        format!("{}h ago", duration.num_hours())
    } else if duration.num_days() < 30 {
        format!("{}d ago", duration.num_days())
    } else {
        dt.format("%Y-%m-%d").to_string()
    }
}

/// Format bytes into human-readable string (Gi, Mi, Ki, B).
fn format_bytes(bytes: i64) -> String {
    const KB: i64 = 1024;
    const MB: i64 = KB * 1024;
    const GB: i64 = MB * 1024;
    const TB: i64 = GB * 1024;

    if bytes >= TB {
        format!("{:.1}T", bytes as f64 / TB as f64)
    } else if bytes >= GB {
        format!("{:.1}Gi", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1}Mi", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1}Ki", bytes as f64 / KB as f64)
    } else {
        format!("{}B", bytes)
    }
}

#[derive(Debug, Clone, serde::Serialize)]
struct PodInfo {
    name: String,
    image: String,
    status: String,
    created: String,
    namespace: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ServiceInfo {
    name: String,
    service_type: String,
    ports: String,
    selector: String,
    status: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ModelInfo {
    name: String,
    image: String,
    provider: String,
    status: String,
    replicas: i32,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ConfigMapInfo {
    name: String,
    namespace: String,
    data_keys: String,
    immutable: bool,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct SecretInfo {
    name: String,
    namespace: String,
    secret_type: String,
    data_keys: String,
    immutable: bool,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NamespaceInfo {
    name: String,
    status: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NodeInfo {
    name: String,
    status: String,
    roles: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DeploymentInfo {
    name: String,
    namespace: String,
    ready: String,
    up_to_date: i32,
    available: i32,
    age: String,
    image: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct EventInfo {
    namespace: String,
    last_seen: String,
    level: String,
    reason: String,
    object: String,
    message: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct IngressInfo {
    name: String,
    namespace: String,
    class: String,
    hosts: String,
    address: String,
    ports: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct StatefulSetInfo {
    name: String,
    namespace: String,
    ready: String,
    age: String,
    image: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct DaemonSetInfo {
    name: String,
    namespace: String,
    desired: i32,
    current: i32,
    ready: i32,
    available: i32,
    age: String,
    image: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct JobInfo {
    name: String,
    namespace: String,
    status: String,
    completions: String,
    duration: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct CronJobInfo {
    name: String,
    namespace: String,
    schedule: String,
    suspend: bool,
    active: i32,
    last_schedule: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ReplicaSetInfo {
    name: String,
    namespace: String,
    desired: i32,
    current: i32,
    ready: i32,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct NetworkPolicyInfo {
    name: String,
    namespace: String,
    pod_selector: String,
    policy_types: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PVCInfo {
    name: String,
    namespace: String,
    status: String,
    storage_class: String,
    capacity: String,
    access_modes: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PVInfo {
    name: String,
    status: String,
    storage_class: String,
    capacity: String,
    access_modes: String,
    claim: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ServiceAccountInfo {
    name: String,
    namespace: String,
    tokens: i32,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct PDBInfo {
    name: String,
    namespace: String,
    min_available: String,
    max_disruptions: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct StorageClassInfo {
    name: String,
    provisioner: String,
    is_default: String,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct RoleInfo {
    name: String,
    namespace: String,
    rules_count: usize,
    age: String,
}

#[derive(Debug, Clone, serde::Serialize)]
struct ClusterRoleInfo {
    name: String,
    rules_count: usize,
    age: String,
}

struct RoleBindingInfo {
    name: String,
    namespace: String,
    role_kind: String,
    role_name: String,
    subjects_count: usize,
    age: String,
}

struct ClusterRoleBindingInfo {
    name: String,
    role_kind: String,
    role_name: String,
    subjects_count: usize,
    age: String,
}

struct EndpointSliceInfo {
    name: String,
    namespace: String,
    service_name: String,
    endpoints_count: usize,
    age: String,
}

#[async_trait]
impl Command for GetCommand {
    async fn run(&self) -> Result<()> {
        match self.resource {
            ResourceType::All | ResourceType::Pods => {
                let pods = self.list_pods();
                match self.output.as_ref() {
                    Some(OutputFormat::Json) => self.print_pods_json(&pods),
                    Some(OutputFormat::Yaml) => {
                        let json = serde_json::to_value(&pods).unwrap_or_default();
                        let yaml = serde_yaml::to_string(&json).unwrap_or_default();
                        println!("{}", yaml);
                    }
                    _ => self.print_pods(&pods),
                }
            }
            ResourceType::Services => {
                let services = self.list_services();
                self.print_services(&services);
            }
            ResourceType::Models => {
                let models = self.list_models();
                self.print_models(&models);
            }
            ResourceType::ConfigMaps => {
                let configmaps = self.list_configmaps();
                self.print_configmaps(&configmaps);
            }
            ResourceType::Secrets => {
                let secrets = self.list_secrets();
                self.print_secrets(&secrets);
            }
            ResourceType::Namespaces => {
                let namespaces = self.list_namespaces();
                self.print_namespaces(&namespaces);
            }
            ResourceType::Nodes => {
                let nodes = self.list_nodes();
                self.print_nodes(&nodes);
            }
            ResourceType::Events => {
                let events = self.list_events();
                self.print_events(&events);
            }
            ResourceType::Ingresses => {
                let ingresses = self.list_ingresses();
                self.print_ingresses(&ingresses);
            }
            ResourceType::Deployments => {
                let deployments = self.list_deployments();
                self.print_deployments(&deployments);
            }
            ResourceType::StatefulSets => {
                let statefulsets = self.list_statefulsets();
                self.print_statefulsets(&statefulsets);
            }
            ResourceType::DaemonSets => {
                let daemonsets = self.list_daemonsets();
                self.print_daemonsets(&daemonsets);
            }
            ResourceType::Jobs => {
                let jobs = self.list_jobs();
                self.print_jobs(&jobs);
            }
            ResourceType::CronJobs => {
                let cronjobs = self.list_cronjobs();
                self.print_cronjobs(&cronjobs);
            }
            ResourceType::NetworkPolicies => {
                let networkpolicies = self.list_networkpolicies();
                self.print_networkpolicies(&networkpolicies);
            }
            ResourceType::ReplicaSets => {
                let replicasets = self.list_replicasets();
                self.print_replicasets(&replicasets);
            }
            ResourceType::PersistentVolumeClaims => {
                let pvcs = self.list_pvcs();
                self.print_pvcs(&pvcs);
            }
            ResourceType::PersistentVolumes => {
                let pvs = self.list_pvs();
                self.print_pvs(&pvs);
            }
            ResourceType::ServiceAccounts => {
                let serviceaccounts = self.list_serviceaccounts();
                self.print_serviceaccounts(&serviceaccounts);
            }
            ResourceType::Revisions => {
                println!("Revisions are not yet implemented.");
            }
            ResourceType::PodDisruptionBudgets => {
                let pdbs = self.list_poddisruptionbudgets();
                self.print_poddisruptionbudgets(&pdbs);
            }
            ResourceType::StorageClasses => {
                let scs = self.list_storageclasses();
                self.print_storageclasses(&scs);
            }
            ResourceType::Roles => {
                let roles = self.list_roles();
                self.print_roles(&roles);
            }
            ResourceType::ClusterRoles => {
                let roles = self.list_cluster_roles();
                self.print_cluster_roles(&roles);
            }
            ResourceType::RoleBindings => {
                let role_bindings = self.list_role_bindings();
                self.print_role_bindings(&role_bindings);
            }
            ResourceType::ClusterRoleBindings => {
                let role_bindings = self.list_cluster_role_bindings();
                self.print_cluster_role_bindings(&role_bindings);
            }
            ResourceType::EndpointSlices => {
                let eps = self.list_endpoint_slices();
                self.print_endpoint_slices(&eps);
            }
            ResourceType::ComponentStatuses => {
                self.print_component_statuses();
            }
        }

        Ok(())
    }
}
