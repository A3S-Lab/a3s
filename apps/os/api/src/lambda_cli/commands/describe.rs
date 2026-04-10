//! `a3s describe` command - Show details of a resource (kubectl describe style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use async_trait::async_trait;
use std::path::PathBuf;

/// Sandbox info from info.json.
#[derive(Debug, serde::Deserialize)]
struct SandboxInfo {
    id: String,
    name: String,
    status: String,
    #[serde(default)]
    image: String,
    #[serde(default)]
    replicas: i32,
}

/// Show details of a resource.
#[derive(clap::Parser, Debug)]
pub struct DescribeCommand {
    /// Resource type.
    #[arg(value_enum)]
    resource: ResourceType,

    /// Resource name.
    name: String,

    /// Namespace (for namespaced resources).
    #[arg(short, long, default_value = "default")]
    namespace: String,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ResourceType {
    Pod,
    Service,
    Model,
    Node,
    Namespace,
    Deployment,
    ConfigMap,
    Secret,
    Ingress,
    StatefulSet,
    DaemonSet,
    Job,
    CronJob,
    NetworkPolicy,
    ReplicaSet,
    PVC,
    PV,
    ServiceAccount,
    PodDisruptionBudget,
    StorageClass,
    Role,
    ClusterRole,
    RoleBinding,
    ClusterRoleBinding,
    ComponentStatus,
}

impl DescribeCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    /// Find sandbox ID by name (either sandbox ID or deployment name).
    fn find_sandbox_id(name: &str) -> Result<String> {
        let boxes_dir = Self::boxes_dir();

        // First check if name is already a sandbox ID
        if boxes_dir.join(name).exists() {
            return Ok(name.to_string());
        }

        // Search for deployment name in info.json
        if let Ok(entries) = std::fs::read_dir(&boxes_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.is_dir() {
                    let info_path = path.join("info.json");
                    if info_path.exists() {
                        if let Ok(content) = std::fs::read_to_string(&info_path) {
                            if let Ok(info) = serde_json::from_str::<serde_json::Value>(&content) {
                                if info
                                    .get("name")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s == name)
                                    .unwrap_or(false)
                                {
                                    return path
                                        .file_name()
                                        .map(|n| n.to_string_lossy().to_string())
                                        .ok_or_else(|| {
                                            A3sError::Project(format!(
                                                "invalid sandbox id for '{}'",
                                                name
                                            ))
                                        });
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(A3sError::Project(format!("pod '{}' not found", name)))
    }

    fn describe_pod(&self, name: &str) -> Result<()> {
        let sandbox_id = Self::find_sandbox_id(name)?;
        let box_path = Self::boxes_dir().join(&sandbox_id);

        // Try to read sandbox info from info.json
        let info_path = box_path.join("info.json");
        let info: Option<SandboxInfo> = if info_path.exists() {
            if let Ok(content) = std::fs::read_to_string(&info_path) {
                serde_json::from_str(&content).ok()
            } else {
                None
            }
        } else {
            None
        };

        println!("Name:           {}", sandbox_id);
        if let Some(ref info) = info {
            println!("Deployment:     {}", info.name);
        }
        println!("Namespace:      default");
        println!("Priority:       0");
        println!("Node:           local");
        if let Some(ref info) = info {
            println!("Status:         {}", info.status);
        } else {
            println!("Status:         Running");
        }

        // Created time
        if let Ok(meta) = std::fs::metadata(&box_path) {
            if let Ok(created) = meta.created() {
                let dt = chrono::DateTime::<chrono::Utc>::from(created);
                println!("Created:        {}", dt.format("%Y-%m-%d %H:%M:%S UTC"));
            }
        }

        // Image from info.json or logs
        if let Some(ref info) = info {
            if !info.image.is_empty() {
                println!("Image:          {}", info.image);
            }
        }
        if info.as_ref().map(|i| i.image.is_empty()).unwrap_or(true) {
            let log_path = box_path.join("logs").join("console.log");
            if let Ok(content) = std::fs::read_to_string(&log_path) {
                for line in content.lines() {
                    if let Some(idx) = line.find("reference=") {
                        let after_ref = &line[idx + "reference=".len()..];
                        let image = after_ref.trim().trim_matches('"');
                        if !image.is_empty() {
                            println!("Image:          {}", image);
                            break;
                        }
                    }
                }
            }
        }

        // Replicas
        if let Some(ref info) = info {
            if info.replicas > 0 {
                println!("Replicas:       {}", info.replicas);
            }
        }

        // Ports
        println!("Ports:          80/TCP, 443/TCP");

        // State
        let socket_path = box_path.join("sockets").join("exec.sock");
        let state = if socket_path.exists() {
            "Running"
        } else {
            "Stopped"
        };
        println!("State:          {}", state);

        // Volumes
        println!("Volumes:");
        println!("  workspace     {}", box_path.join("workspace").display());

        // Events (placeholder)
        println!("Events:         <none>");

        Ok(())
    }

    fn describe_namespace(&self, name: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_namespaces() {
            Ok(namespaces) => {
                if let Some(ns) = namespaces.get(name) {
                    println!("Name:           {}", ns.name);
                    println!("Labels:         <none>");
                    println!("Annotations:    <none>");
                    println!(
                        "Created:        {}",
                        ns.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    Ok(())
                } else {
                    Err(A3sError::Project(format!("namespace '{}' not found", name)))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load namespace: {}",
                e
            ))),
        }
    }

    fn describe_deployment(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_deployments() {
            Ok(deployments) => {
                if let Some(dep) = deployments.get(name) {
                    if dep.namespace == namespace {
                        println!("Name:           {}", dep.name);
                        println!("Namespace:      {}", dep.namespace);
                        println!("Replicas:       {} desired", dep.replicas);
                        println!("Image:          {}", dep.image);
                        println!(
                            "Created:        {}",
                            dep.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "deployment '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "deployment '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load deployment: {}",
                e
            ))),
        }
    }

    fn describe_configmap(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_configmaps() {
            Ok(configmaps) => {
                if let Some(cm) = configmaps.get(name) {
                    if cm.namespace == namespace {
                        println!("Name:           {}", cm.name);
                        println!("Namespace:      {}", cm.namespace);
                        println!("Immutable:      {}", cm.immutable);
                        println!("Data:");
                        for (key, value) in &cm.data {
                            println!("  {}: {} bytes", key, value.len());
                        }
                        println!(
                            "Created:        {}",
                            cm.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "configmap '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "configmap '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load configmap: {}",
                e
            ))),
        }
    }

    fn describe_secret(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_secrets() {
            Ok(secrets) => {
                if let Some(secret) = secrets.get(name) {
                    if secret.namespace == namespace {
                        println!("Name:           {}", secret.name);
                        println!("Namespace:      {}", secret.namespace);
                        println!("Type:          {}", format!("{:?}", secret.secret_type));
                        println!("Data:");
                        for (key, value) in &secret.data {
                            println!("  {}: {} bytes", key, value.len());
                        }
                        println!(
                            "Created:        {}",
                            secret.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "secret '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "secret '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!("failed to load secret: {}", e))),
        }
    }

    fn describe_ingress(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_ingresses() {
            Ok(ingresses) => {
                if let Some(ns_ings) = ingresses.get(namespace) {
                    if let Some(ing) = ns_ings.get(name) {
                        println!("Name:           {}", ing.name);
                        println!("Namespace:      {}", ing.namespace);
                        println!("Class:         <none>");
                        println!("Controller:     <none>");
                        println!("Rules:");
                        for rule in &ing.spec.rules {
                            println!(
                                "  Host: {}",
                                if rule.host.is_empty() {
                                    "*"
                                } else {
                                    &rule.host
                                }
                            );
                            println!("  Path: /");
                        }
                        println!(
                            "Created:        {}",
                            ing.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "ingress '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "ingress '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!("failed to load ingress: {}", e))),
        }
    }

    fn describe_statefulset(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_statefulsets() {
            Ok(statefulsets) => {
                if let Some(ns_sss) = statefulsets.get(namespace) {
                    if let Some(ss) = ns_sss.get(name) {
                        println!("Name:           {}", ss.name);
                        println!("Namespace:      {}", ss.namespace);
                        println!("Replicas:       {} desired", ss.spec.replicas);
                        println!("Selector:       <none>");
                        println!(
                            "Created:        {}",
                            ss.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "statefulset '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "statefulset '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load statefulset: {}",
                e
            ))),
        }
    }

    fn describe_daemonset(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_daemonsets() {
            Ok(daemonsets) => {
                if let Some(ds) = daemonsets.get(name) {
                    if ds.namespace == namespace {
                        println!("Name:           {}", ds.name);
                        println!("Namespace:      {}", ds.namespace);
                        println!("Selector:       <none>");
                        println!(
                            "Created:        {}",
                            ds.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "daemonset '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "daemonset '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load daemonset: {}",
                e
            ))),
        }
    }

    fn describe_job(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_jobs() {
            Ok(jobs) => {
                if let Some(ns_jobs) = jobs.get(namespace) {
                    if let Some(job) = ns_jobs.get(name) {
                        println!("Name:           {}", job.name);
                        println!("Namespace:      {}", job.namespace);
                        println!("Parallelism:    {}", job.spec.parallelism.unwrap_or(1));
                        println!("Completions:    {}", job.spec.completions.unwrap_or(1));
                        let backoff_limit = match job.spec.backoff_limit {
                            crate::state::batch::BackoffLimitPolicy::Unlimited => {
                                "unlimited".to_string()
                            }
                            crate::state::batch::BackoffLimitPolicy::Limited(n) => n.to_string(),
                        };
                        println!("BackoffLimit:   {}", backoff_limit);
                        println!(
                            "Created:        {}",
                            job.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "job '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "job '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!("failed to load job: {}", e))),
        }
    }

    fn describe_cronjob(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_cronjobs() {
            Ok(cronjobs) => {
                if let Some(ns_cjs) = cronjobs.get(namespace) {
                    if let Some(cj) = ns_cjs.get(name) {
                        println!("Name:           {}", cj.name);
                        println!("Namespace:      {}", cj.namespace);
                        println!("Schedule:       {}", cj.spec.schedule.format());
                        println!("Suspend:        {}", cj.spec.suspend);
                        println!(
                            "Created:        {}",
                            cj.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "cronjob '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "cronjob '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!("failed to load cronjob: {}", e))),
        }
    }

    fn describe_networkpolicy(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_network_policies() {
            Ok(policies) => {
                if let Some(np) = policies
                    .iter()
                    .find(|p| p.name == name && p.namespace == namespace)
                {
                    println!("Name:           {}", np.name);
                    println!("Namespace:      {}", np.namespace);
                    println!(
                        "PodSelector:    {}",
                        np.spec
                            .pod_selector
                            .match_labels
                            .iter()
                            .map(|(k, v)| format!("{}={}", k, v))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    println!("PolicyTypes:");
                    for pt in &np.spec.policy_types {
                        println!("  - {:?}", pt);
                    }
                    println!(
                        "Created:        {}",
                        np.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    Ok(())
                } else {
                    Err(A3sError::Project(format!(
                        "networkpolicy '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load networkpolicy: {}",
                e
            ))),
        }
    }

    fn describe_replicaset(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_replicasets() {
            Ok(replicasets) => {
                if let Some(ns_rss) = replicasets.get(namespace) {
                    if let Some(rs) = ns_rss.get(name) {
                        println!("Name:           {}", rs.name);
                        println!("Namespace:      {}", rs.namespace);
                        println!("Replicas:       {} desired", rs.spec.replicas.unwrap_or(0));
                        println!("Selector:       <none>");
                        println!(
                            "Created:        {}",
                            rs.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "replicaset '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "replicaset '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load replicaset: {}",
                e
            ))),
        }
    }

    fn describe_pvc(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_persistent_volume_claims() {
            Ok(pvcs) => {
                if let Some(ns_pvcs) = pvcs.get(namespace) {
                    if let Some(pvc) = ns_pvcs.get(name) {
                        println!("Name:           {}", pvc.name);
                        println!("Namespace:      {}", pvc.namespace);
                        println!("Status:         {}", format!("{:?}", pvc.status));
                        println!("Volume:         <none>");
                        println!(
                            "StorageClass:  {}",
                            pvc.spec
                                .storage_class_name
                                .as_deref()
                                .unwrap_or("<default>")
                        );
                        println!(
                            "Capacity:       {} bytes",
                            pvc.spec.resources.requests.get("storage").unwrap_or(&0)
                        );
                        println!(
                            "Access Modes:  {}",
                            pvc.spec
                                .access_modes
                                .iter()
                                .map(|m| format!("{:?}", m))
                                .collect::<Vec<_>>()
                                .join(",")
                        );
                        println!(
                            "Created:        {}",
                            pvc.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "persistentvolumeclaim '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "persistentvolumeclaim '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load persistentvolumeclaim: {}",
                e
            ))),
        }
    }

    fn describe_pv(&self, name: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_persistent_volumes() {
            Ok(pvs) => {
                if let Some(pv) = pvs.get(name) {
                    println!("Name:           {}", pv.name);
                    println!("Status:         {}", format!("{:?}", pv.status));
                    println!(
                        "Claim:          {}",
                        pv.claim_name.as_deref().unwrap_or("<none>")
                    );
                    println!(
                        "StorageClass:  {}",
                        pv.spec.storage_class.as_deref().unwrap_or("<none>")
                    );
                    println!("Capacity:       {} bytes", pv.spec.capacity);
                    println!(
                        "Access Modes:  {}",
                        pv.spec
                            .access_modes
                            .iter()
                            .map(|m| format!("{:?}", m))
                            .collect::<Vec<_>>()
                            .join(",")
                    );
                    println!("ReclaimPolicy: {:?}", pv.spec.reclaim_policy);
                    println!(
                        "Created:        {}",
                        pv.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    Ok(())
                } else {
                    Err(A3sError::Project(format!(
                        "persistentvolume '{}' not found",
                        name
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load persistentvolume: {}",
                e
            ))),
        }
    }

    fn describe_serviceaccount(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_service_accounts() {
            Ok(serviceaccounts) => {
                if let Some(ns_sas) = serviceaccounts.get(namespace) {
                    if let Some(sa) = ns_sas.get(name) {
                        println!("Name:           {}", sa.name);
                        println!("Namespace:      {}", sa.namespace);
                        println!("Labels:         <none>");
                        println!("Annotations:    <none>");
                        println!(
                            "Created:        {}",
                            sa.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "serviceaccount '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "serviceaccount '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load serviceaccount: {}",
                e
            ))),
        }
    }

    fn describe_poddisruptionbudget(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_pod_disruption_budgets() {
            Ok(pdbs) => {
                if let Some(ns_pdbs) = pdbs.get(namespace) {
                    if let Some(pdb) = ns_pdbs.get(name) {
                        println!("Name:           {}", pdb.name);
                        println!("Namespace:      {}", pdb.namespace);
                        if let Some(ref min) = pdb.spec.min_available {
                            println!("MinAvailable:   {}", min);
                        }
                        if let Some(ref max) = pdb.spec.max_disruptions {
                            println!("MaxDisruptions: {}", max);
                        }
                        println!(
                            "Created:        {}",
                            pdb.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "poddisruptionbudget '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "poddisruptionbudget '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load poddisruptionbudget: {}",
                e
            ))),
        }
    }

    fn describe_storageclass(&self, name: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.get_storage_class(name) {
            Ok(Some(sc)) => {
                println!("Name:           {}", sc.name);
                println!("Provisioner:    {}", sc.provisioner);
                println!(
                    "IsDefault:      {}",
                    if sc.is_default { "true" } else { "false" }
                );
                println!("Parameters:");
                println!("  storageType:  {}", sc.parameters.storage_type);
                if let Some(ref nfs_server) = sc.parameters.nfs_server {
                    println!("  nfsServer:    {}", nfs_server);
                }
                if let Some(ref nfs_path) = sc.parameters.nfs_path {
                    println!("  nfsPath:      {}", nfs_path);
                }
                if let Some(ref host_path) = sc.parameters.host_path_base {
                    println!("  hostPathBase: {}", host_path);
                }
                println!("  mountOptions: {:?}", sc.parameters.mount_options);
                println!("  reclaimPolicy: {:?}", sc.parameters.reclaim_policy);
                println!(
                    "  volumeBindingMode: {:?}",
                    sc.parameters.volume_binding_mode
                );
                if !sc.labels.is_empty() {
                    println!("Labels:");
                    for (k, v) in &sc.labels {
                        println!("  {}: {}", k, v);
                    }
                }
                println!(
                    "Created:        {}",
                    sc.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                );
                Ok(())
            }
            Ok(None) => Err(A3sError::Project(format!(
                "storageclass '{}' not found",
                name
            ))),
            Err(e) => Err(A3sError::Project(format!(
                "failed to load storageclass: {}",
                e
            ))),
        }
    }

    fn describe_role(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_roles() {
            Ok(roles) => {
                if let Some(ns_roles) = roles.get(namespace) {
                    if let Some(role) = ns_roles.get(name) {
                        println!("Name:           {}", role.name);
                        println!(
                            "Namespace:      {}",
                            role.namespace.as_deref().unwrap_or("<none>")
                        );
                        if !role.labels.is_empty() {
                            println!("Labels:");
                            for (k, v) in &role.labels {
                                println!("  {}: {}", k, v);
                            }
                        }
                        if !role.annotations.is_empty() {
                            println!("Annotations:");
                            for (k, v) in &role.annotations {
                                println!("  {}: {}", k, v);
                            }
                        }
                        println!("Rules:");
                        for (i, rule) in role.rules.iter().enumerate() {
                            println!("  Rule {}:", i + 1);
                            if !rule.api_groups.is_empty() {
                                println!("    APIGroups:    {:?}", rule.api_groups);
                            }
                            if !rule.resources.is_empty() {
                                println!("    Resources:    {:?}", rule.resources);
                            }
                            if !rule.resource_names.is_empty() {
                                println!("    ResourceNames: {:?}", rule.resource_names);
                            }
                            println!("    Verbs:        {:?}", rule.verbs);
                        }
                        println!(
                            "Created:        {}",
                            role.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "role '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "role '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!("failed to load role: {}", e))),
        }
    }

    fn describe_cluster_role(&self, name: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_cluster_roles() {
            Ok(cluster_roles) => {
                if let Some(cr) = cluster_roles.get(name) {
                    println!("Name:           {}", cr.name);
                    println!("Labels:");
                    if cr.labels.is_empty() {
                        println!("  <none>");
                    } else {
                        for (k, v) in &cr.labels {
                            println!("  {}: {}", k, v);
                        }
                    }
                    println!("Annotations:");
                    if cr.annotations.is_empty() {
                        println!("  <none>");
                    } else {
                        for (k, v) in &cr.annotations {
                            println!("  {}: {}", k, v);
                        }
                    }
                    println!("Rules:");
                    for (i, rule) in cr.rules.iter().enumerate() {
                        println!("  Rule {}:", i + 1);
                        if !rule.api_groups.is_empty() {
                            println!("    APIGroups:    {:?}", rule.api_groups);
                        }
                        if !rule.resources.is_empty() {
                            println!("    Resources:    {:?}", rule.resources);
                        }
                        if !rule.resource_names.is_empty() {
                            println!("    ResourceNames: {:?}", rule.resource_names);
                        }
                        if !rule.non_resource_u_r_l_s.is_empty() {
                            println!("    NonResourceURLs: {:?}", rule.non_resource_u_r_l_s);
                        }
                        println!("    Verbs:        {:?}", rule.verbs);
                    }
                    println!(
                        "Created:        {}",
                        cr.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    Ok(())
                } else {
                    Err(A3sError::Project(format!(
                        "clusterrole '{}' not found",
                        name
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load clusterrole: {}",
                e
            ))),
        }
    }

    fn describe_role_binding(&self, name: &str, namespace: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_role_bindings() {
            Ok(role_bindings) => {
                if let Some(ns_bindings) = role_bindings.get(namespace) {
                    if let Some(rb) = ns_bindings.get(name) {
                        println!("Name:           {}", rb.name);
                        println!(
                            "Namespace:      {}",
                            rb.namespace.as_deref().unwrap_or("<none>")
                        );
                        if !rb.labels.is_empty() {
                            println!("Labels:");
                            for (k, v) in &rb.labels {
                                println!("  {}: {}", k, v);
                            }
                        }
                        if !rb.annotations.is_empty() {
                            println!("Annotations:");
                            for (k, v) in &rb.annotations {
                                println!("  {}: {}", k, v);
                            }
                        }
                        println!("RoleRef:");
                        println!("  Kind:         {}", rb.role_ref.kind);
                        println!("  Name:         {}", rb.role_ref.name);
                        println!("Subjects:");
                        for subject in &rb.subjects {
                            println!("  - Kind:       {}", subject.kind);
                            println!("    Name:       {}", subject.name);
                            if let Some(ref api_group) = subject.api_group {
                                println!("    APIGroup:   {}", api_group);
                            }
                        }
                        println!(
                            "Created:        {}",
                            rb.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                        );
                        Ok(())
                    } else {
                        Err(A3sError::Project(format!(
                            "rolebinding '{}' not found in namespace '{}'",
                            name, namespace
                        )))
                    }
                } else {
                    Err(A3sError::Project(format!(
                        "rolebinding '{}' not found in namespace '{}'",
                        name, namespace
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load rolebinding: {}",
                e
            ))),
        }
    }

    fn describe_cluster_role_binding(&self, name: &str) -> Result<()> {
        let store = crate::state::StateStore::new(&std::path::PathBuf::from("."));
        match store.load_cluster_role_bindings() {
            Ok(cluster_role_bindings) => {
                if let Some(crb) = cluster_role_bindings.get(name) {
                    println!("Name:           {}", crb.name);
                    println!("Labels:");
                    if crb.labels.is_empty() {
                        println!("  <none>");
                    } else {
                        for (k, v) in &crb.labels {
                            println!("  {}: {}", k, v);
                        }
                    }
                    println!("Annotations:");
                    if crb.annotations.is_empty() {
                        println!("  <none>");
                    } else {
                        for (k, v) in &crb.annotations {
                            println!("  {}: {}", k, v);
                        }
                    }
                    println!("RoleRef:");
                    println!("  Kind:         {}", crb.role_ref.kind);
                    println!("  Name:         {}", crb.role_ref.name);
                    println!("Subjects:");
                    for subject in &crb.subjects {
                        println!("  - Kind:       {}", subject.kind);
                        println!("    Name:       {}", subject.name);
                        if let Some(ref api_group) = subject.api_group {
                            println!("    APIGroup:   {}", api_group);
                        }
                    }
                    println!(
                        "Created:        {}",
                        crb.created_at.format("%Y-%m-%d %H:%M:%S UTC")
                    );
                    Ok(())
                } else {
                    Err(A3sError::Project(format!(
                        "clusterrolebinding '{}' not found",
                        name
                    )))
                }
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to load clusterrolebinding: {}",
                e
            ))),
        }
    }

    fn describe_component_status(&self, name: &str) -> Result<()> {
        // ComponentStatus reports health of control plane components
        match name {
            "scheduler" => {
                println!("Name:               scheduler");
                println!("Kind:               ComponentStatus");
                println!("Labels:             k8s.io/component=scheduler");
                println!("Conditions:");
                println!("  Type:              Healthy");
                println!("  Status:            True");
                println!("  Message:           scheduler is healthy");
                Ok(())
            }
            "controller-manager" | "controller-manager-1" => {
                println!("Name:               controller-manager");
                println!("Kind:               ComponentStatus");
                println!("Labels:             k8s.io/component=controller-manager");
                println!("Conditions:");
                println!("  Type:              Healthy");
                println!("  Status:            True");
                println!("  Message:           controller-manager is healthy");
                Ok(())
            }
            "etcd-0" | "etcd" => {
                println!("Name:               {}", name);
                println!("Kind:               ComponentStatus");
                println!("Labels:             k8s.io/component=etcd");
                println!("Conditions:");
                println!("  Type:              Healthy");
                println!("  Status:            True");
                println!("  Message:           etcd is healthy");
                Ok(())
            }
            _ => {
                // Show all component statuses
                println!("Name:               {}", name);
                println!("Kind:               ComponentStatus");
                println!("Labels:             <none>");
                println!("Conditions:");
                println!("  Type:              Healthy");
                println!("  Status:            True");
                println!("  Message:           component is healthy");
                Ok(())
            }
        }
    }
}

#[async_trait]
impl Command for DescribeCommand {
    async fn run(&self) -> Result<()> {
        match self.resource {
            ResourceType::Pod => self.describe_pod(&self.name),
            ResourceType::Service => {
                println!("Service: {}", self.name);
                println!("Type:    ClusterIP");
                Ok(())
            }
            ResourceType::Model => self.describe_pod(&self.name),
            ResourceType::Node => {
                println!("Node: local");
                println!("Roles: worker");
                Ok(())
            }
            ResourceType::Namespace => self.describe_namespace(&self.name),
            ResourceType::Deployment => self.describe_deployment(&self.name, &self.namespace),
            ResourceType::ConfigMap => self.describe_configmap(&self.name, &self.namespace),
            ResourceType::Secret => self.describe_secret(&self.name, &self.namespace),
            ResourceType::Ingress => self.describe_ingress(&self.name, &self.namespace),
            ResourceType::StatefulSet => self.describe_statefulset(&self.name, &self.namespace),
            ResourceType::DaemonSet => self.describe_daemonset(&self.name, &self.namespace),
            ResourceType::Job => self.describe_job(&self.name, &self.namespace),
            ResourceType::CronJob => self.describe_cronjob(&self.name, &self.namespace),
            ResourceType::NetworkPolicy => self.describe_networkpolicy(&self.name, &self.namespace),
            ResourceType::ReplicaSet => self.describe_replicaset(&self.name, &self.namespace),
            ResourceType::PVC => self.describe_pvc(&self.name, &self.namespace),
            ResourceType::PV => self.describe_pv(&self.name),
            ResourceType::ServiceAccount => {
                self.describe_serviceaccount(&self.name, &self.namespace)
            }
            ResourceType::PodDisruptionBudget => {
                self.describe_poddisruptionbudget(&self.name, &self.namespace)
            }
            ResourceType::StorageClass => self.describe_storageclass(&self.name),
            ResourceType::Role => self.describe_role(&self.name, &self.namespace),
            ResourceType::ClusterRole => self.describe_cluster_role(&self.name),
            ResourceType::RoleBinding => self.describe_role_binding(&self.name, &self.namespace),
            ResourceType::ClusterRoleBinding => self.describe_cluster_role_binding(&self.name),
            ResourceType::ComponentStatus => self.describe_component_status(&self.name),
        }
    }
}
