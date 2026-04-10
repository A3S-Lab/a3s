//! `a3s delete` command - Delete resources (kubectl delete style).

use crate::commands::Command;
use crate::errors::{A3sError, Result};
use crate::state::StateStore;
use async_trait::async_trait;
use std::path::PathBuf;

/// Delete resources by name.
#[derive(clap::Parser, Debug)]
pub struct DeleteCommand {
    /// Resource type.
    #[arg(value_enum)]
    resource: ResourceType,

    /// Resource name(s) to delete.
    names: Vec<String>,

    /// Namespace (for namespaced resources).
    #[arg(short, long, default_value = "default")]
    namespace: String,

    /// Cascade deletion (delete related resources).
    #[arg(long, default_value = "true")]
    cascade: bool,

    /// Force delete even if resource is not empty.
    #[arg(long)]
    force: bool,
}

#[derive(clap::ValueEnum, Clone, Debug)]
pub enum ResourceType {
    Pod,
    Deployment,
    Service,
    Model,
    ConfigMap,
    Secret,
    Namespace,
    Ingress,
    StatefulSet,
    DaemonSet,
    Job,
    CronJob,
    NetworkPolicy,
    ReplicaSet,
    PVC,
    ServiceAccount,
    PodDisruptionBudget,
    StorageClass,
    Role,
    ClusterRole,
    RoleBinding,
    ClusterRoleBinding,
}

impl DeleteCommand {
    fn boxes_dir() -> PathBuf {
        dirs::home_dir()
            .map(|h| h.join(".a3s").join("boxes"))
            .unwrap_or_else(|| PathBuf::from("~/.a3s/boxes"))
    }

    /// Find sandbox ID by name (either sandbox ID or deployment name).
    fn find_sandbox_id(name: &str) -> Result<PathBuf> {
        let boxes_dir = Self::boxes_dir();

        // First check if name is already a sandbox ID
        let box_path = boxes_dir.join(name);
        if box_path.exists() {
            return Ok(box_path);
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
                                    return Ok(path);
                                }
                            }
                        }
                    }
                }
            }
        }

        Err(A3sError::Project(format!("pod '{}' not found", name)))
    }

    async fn delete_pod(&self, name: &str) -> Result<()> {
        let box_path = Self::find_sandbox_id(name)?;

        // Check if VM is running and kill it
        let socket_path = box_path.join("sockets").join("exec.sock");
        if socket_path.exists() {
            std::fs::remove_file(&socket_path).ok();
        }

        // Remove the box directory
        if box_path.exists() {
            std::fs::remove_dir_all(&box_path).map_err(|e| {
                A3sError::Project(format!("failed to delete {}: {}", box_path.display(), e))
            })?;
        }

        println!("pod '{}' deleted", name);
        Ok(())
    }

    async fn delete_namespace(&self, name: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_namespace(name) {
            Ok(_) => {
                println!("namespace '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete namespace '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_deployment(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_deployment(name) {
            Ok(_) => {
                println!("deployment '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete deployment '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_configmap(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_configmap(name) {
            Ok(_) => {
                println!("configmap '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete configmap '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_secret(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_secret(name) {
            Ok(_) => {
                println!("secret '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete secret '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_ingress(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_ingress(namespace, name) {
            Ok(_) => {
                println!("ingress '{}' deleted from namespace '{}'", name, namespace);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete ingress '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_statefulset(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_statefulset(namespace, name) {
            Ok(_) => {
                println!(
                    "statefulset '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete statefulset '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_daemonset(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_daemonset(name) {
            Ok(_) => {
                println!("daemonset '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete daemonset '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_job(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_job(namespace, name) {
            Ok(_) => {
                println!("job '{}' deleted from namespace '{}'", name, namespace);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete job '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_cronjob(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_cronjob(namespace, name) {
            Ok(_) => {
                println!("cronjob '{}' deleted from namespace '{}'", name, namespace);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete cronjob '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_networkpolicy(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_network_policy(namespace, name) {
            Ok(_) => {
                println!(
                    "networkpolicy '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete networkpolicy '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_replicaset(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_replicaset(namespace, name) {
            Ok(_) => {
                println!(
                    "replicaset '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete replicaset '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_pvc(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_persistent_volume_claim(namespace, name) {
            Ok(_) => {
                println!(
                    "persistentvolumeclaim '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete persistentvolumeclaim '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_serviceaccount(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_service_account(namespace, name) {
            Ok(_) => {
                println!(
                    "serviceaccount '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete serviceaccount '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_poddisruptionbudget(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_pod_disruption_budget(namespace, name) {
            Ok(_) => {
                println!(
                    "poddisruptionbudget '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete poddisruptionbudget '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_storageclass(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_storage_class(name) {
            Ok(_) => {
                println!("storageclass '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete storageclass '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_role(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_role(namespace, name) {
            Ok(_) => {
                println!("role '{}' deleted from namespace '{}'", name, namespace);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete role '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_cluster_role(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_cluster_role(name) {
            Ok(_) => {
                println!("clusterrole '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete clusterrole '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_role_binding(&self, name: &str, namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_role_binding(namespace, name) {
            Ok(_) => {
                println!(
                    "rolebinding '{}' deleted from namespace '{}'",
                    name, namespace
                );
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete rolebinding '{}': {}",
                name, e
            ))),
        }
    }

    async fn delete_cluster_role_binding(&self, name: &str, _namespace: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));
        match store.delete_cluster_role_binding(name) {
            Ok(_) => {
                println!("clusterrolebinding '{}' deleted", name);
                Ok(())
            }
            Err(e) => Err(A3sError::Project(format!(
                "failed to delete clusterrolebinding '{}': {}",
                name, e
            ))),
        }
    }
}

#[async_trait]
impl Command for DeleteCommand {
    async fn run(&self) -> Result<()> {
        for name in &self.names {
            match self.resource {
                ResourceType::Pod => {
                    self.delete_pod(name).await?;
                }
                ResourceType::Deployment => {
                    self.delete_deployment(name, &self.namespace).await?;
                }
                ResourceType::Service => {
                    // Services don't have separate state - just log
                    println!("service '{}' (stateless)", name);
                }
                ResourceType::Model => {
                    self.delete_pod(name).await?;
                }
                ResourceType::ConfigMap => {
                    self.delete_configmap(name, &self.namespace).await?;
                }
                ResourceType::Secret => {
                    self.delete_secret(name, &self.namespace).await?;
                }
                ResourceType::Namespace => {
                    self.delete_namespace(name).await?;
                }
                ResourceType::Ingress => {
                    self.delete_ingress(name, &self.namespace).await?;
                }
                ResourceType::StatefulSet => {
                    self.delete_statefulset(name, &self.namespace).await?;
                }
                ResourceType::DaemonSet => {
                    self.delete_daemonset(name, &self.namespace).await?;
                }
                ResourceType::Job => {
                    self.delete_job(name, &self.namespace).await?;
                }
                ResourceType::CronJob => {
                    self.delete_cronjob(name, &self.namespace).await?;
                }
                ResourceType::NetworkPolicy => {
                    self.delete_networkpolicy(name, &self.namespace).await?;
                }
                ResourceType::ReplicaSet => {
                    self.delete_replicaset(name, &self.namespace).await?;
                }
                ResourceType::PVC => {
                    self.delete_pvc(name, &self.namespace).await?;
                }
                ResourceType::ServiceAccount => {
                    self.delete_serviceaccount(name, &self.namespace).await?;
                }
                ResourceType::PodDisruptionBudget => {
                    self.delete_poddisruptionbudget(name, &self.namespace)
                        .await?;
                }
                ResourceType::StorageClass => {
                    self.delete_storageclass(name, &self.namespace).await?;
                }
                ResourceType::Role => {
                    self.delete_role(name, &self.namespace).await?;
                }
                ResourceType::ClusterRole => {
                    self.delete_cluster_role(name, &self.namespace).await?;
                }
                ResourceType::RoleBinding => {
                    self.delete_role_binding(name, &self.namespace).await?;
                }
                ResourceType::ClusterRoleBinding => {
                    self.delete_cluster_role_binding(name, &self.namespace)
                        .await?;
                }
            }
        }

        Ok(())
    }
}
