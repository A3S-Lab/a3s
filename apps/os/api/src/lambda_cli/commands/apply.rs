//! `a3s apply` command - Apply resources from file (kubectl apply style).

use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::path::PathBuf;

use crate::commands::Command;
use crate::config::manifest::{DeploymentSpec, Manifest, ManifestSpec};
use crate::deployment::microvm::MicrovmProvider;
use crate::errors::{A3sError, Result};
use crate::state::{
    ClusterRoleBindingDesired, ClusterRoleDesired, ConfigMapDesired, CronJobDesired,
    DeploymentDesired, HealthCheckConfig, IngressDesired, IngressPath, IngressRule, IngressSpec,
    IngressTls, JobDesired, LabelSelector, NamespaceMeta, NetworkPolicyDesired, NetworkPolicySpec,
    PodDisruptionBudgetDesired, PodDisruptionBudgetSpec, PolicyRule, PolicyType, PortMapping,
    ResourceRequirements, RoleBindingDesired, RoleDesired, RoleRef, RollingUpdateConfig,
    SecretDesired, ServiceDesired, ServicePort, StateStore, Subject, UpdateStrategy,
};

/// Result of applying a manifest.
enum ApplyResult {
    /// Resource was created with the given sandbox ID.
    Created(String),
    /// Resource was applied (stateless).
    Applied,
    /// Dry run, no changes made.
    DryRun,
}

/// Apply resources from file (kubectl apply style).
#[derive(clap::Parser, Debug)]
pub struct ApplyCommand {
    /// File path containing manifest(s).
    #[arg(short = 'f', long)]
    file: PathBuf,

    /// Dry run without applying.
    #[arg(short, long)]
    dry_run: bool,

    /// Wait for resources to be ready.
    #[arg(short, long)]
    wait: bool,

    /// Timeout in seconds for --wait.
    #[arg(long)]
    timeout: Option<u64>,
}

impl ApplyCommand {
    /// Parse HCL manifest file.
    fn parse_manifest(&self) -> Result<Vec<Manifest>> {
        let content = std::fs::read_to_string(&self.file).map_err(|e| {
            A3sError::Project(format!(
                "failed to read file {}: {}",
                self.file.display(),
                e
            ))
        })?;

        // Try to parse as HCL first
        if let Ok(manifests) = self.parse_hcl_manifest(&content) {
            return Ok(manifests);
        }

        // Fall back to JSON/YAML parsing
        self.parse_json_manifest(&content)
    }

    /// Parse HCL format manifest.
    fn parse_hcl_manifest(&self, content: &str) -> Result<Vec<Manifest>> {
        let body = hcl::parse(content)
            .map_err(|e| A3sError::Project(format!("failed to parse HCL manifest: {}", e)))?;

        let manifest = hcl_body_to_manifest(&body)?;
        Ok(vec![manifest])
    }

    /// Parse JSON/YAML format manifest (supports multi-document YAML with `---` separator).
    fn parse_json_manifest(&self, content: &str) -> Result<Vec<Manifest>> {
        // Try JSON first (single document)
        if let Ok(manifest) = serde_json::from_str::<Manifest>(content) {
            return Ok(vec![manifest]);
        }

        // Try multi-document YAML (documents separated by ---)
        let manifests = self.parse_multidoc_yaml(content)?;
        if !manifests.is_empty() {
            return Ok(manifests);
        }

        // Try single-document YAML as fallback
        serde_yaml::from_str::<Manifest>(content)
            .map(|m| vec![m])
            .map_err(|e| A3sError::Project(format!("failed to parse manifest (JSON/YAML): {}", e)))
    }

    /// Parse multi-document YAML by splitting on `---` separator.
    fn parse_multidoc_yaml(&self, content: &str) -> Result<Vec<Manifest>> {
        let trimmed = content.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }

        // Split by --- separator, handling both Unix and Windows line endings
        let docs: Vec<&str> = trimmed
            .split("\n---\n")
            .filter(|s| !s.trim().is_empty())
            .collect();

        if docs.len() <= 1 {
            return Ok(vec![]);
        }

        let mut manifests = Vec::with_capacity(docs.len());
        for (i, doc_content) in docs.iter().enumerate() {
            // Remove leading --- if present (shouldn't be on first doc, but might be)
            let clean_content = doc_content.trim();
            if clean_content.is_empty() {
                continue;
            }

            match serde_yaml::from_str::<Manifest>(clean_content) {
                Ok(manifest) => manifests.push(manifest),
                Err(e) => {
                    return Err(A3sError::Project(format!(
                        "failed to parse YAML document {}: {}",
                        i + 1,
                        e
                    )));
                }
            }
        }

        Ok(manifests)
    }

    async fn apply_manifest(&self, manifest: &Manifest) -> Result<ApplyResult> {
        match manifest.kind.as_str() {
            "Deployment" => {
                if let ManifestSpec::Deployment(deploy_spec) = &manifest.spec {
                    if self.dry_run {
                        println!(
                            "[dry-run] Would create Deployment '{}'",
                            manifest.metadata.name
                        );
                        println!(
                            "  - Image: {}",
                            deploy_spec
                                .template
                                .spec
                                .containers
                                .first()
                                .map(|c| c.image.as_str())
                                .unwrap_or("unknown")
                        );
                        println!("  - Replicas: {}", deploy_spec.replicas);
                        return Ok(ApplyResult::DryRun);
                    }
                    let id = self
                        .apply_deployment(&manifest.metadata.name, deploy_spec)
                        .await?;
                    Ok(ApplyResult::Created(id))
                } else {
                    return Err(A3sError::Project(
                        "Deployment spec missing or invalid".to_string(),
                    ));
                }
            }
            "Service" => {
                if let ManifestSpec::Service(service_spec) = &manifest.spec {
                    if self.dry_run {
                        println!(
                            "[dry-run] Would create Service '{}'",
                            manifest.metadata.name
                        );
                        return Ok(ApplyResult::DryRun);
                    }
                    self.apply_service(&manifest.metadata.name, service_spec)
                        .await?;
                    Ok(ApplyResult::Applied)
                } else {
                    return Err(A3sError::Project(
                        "Service spec missing or invalid".to_string(),
                    ));
                }
            }
            "Model" => {
                if self.dry_run {
                    println!("[dry-run] Would create Model '{}'", manifest.metadata.name);
                    return Ok(ApplyResult::DryRun);
                }
                println!("Applying Model: {}", manifest.metadata.name);
                Ok(ApplyResult::Applied)
            }
            "ConfigMap" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create ConfigMap '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                self.apply_configmap(&manifest.metadata.name, manifest)
                    .await?;
                Ok(ApplyResult::Applied)
            }
            "Secret" => {
                if self.dry_run {
                    println!("[dry-run] Would create Secret '{}'", manifest.metadata.name);
                    return Ok(ApplyResult::DryRun);
                }
                self.apply_secret(&manifest.metadata.name, manifest).await?;
                Ok(ApplyResult::Applied)
            }
            "Revision" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create Revision '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                println!("Applying Revision: {}", manifest.metadata.name);
                Ok(ApplyResult::Applied)
            }
            "DaemonSet" => {
                if let ManifestSpec::DaemonSet(ds_spec) = &manifest.spec {
                    if self.dry_run {
                        println!(
                            "[dry-run] Would create DaemonSet '{}'",
                            manifest.metadata.name
                        );
                        println!("  - Image: {}", ds_spec.template.spec.image);
                        return Ok(ApplyResult::DryRun);
                    }
                    self.apply_daemonset(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace,
                        ds_spec,
                    )
                    .await?;
                    Ok(ApplyResult::Applied)
                } else {
                    return Err(A3sError::Project(
                        "DaemonSet spec missing or invalid".to_string(),
                    ));
                }
            }
            "Namespace" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create Namespace '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                self.apply_namespace(&manifest.metadata.name).await?;
                Ok(ApplyResult::Applied)
            }
            "Ingress" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create Ingress '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_ingress_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!("[warning] Ingress spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "Job" => {
                if self.dry_run {
                    println!("[dry-run] Would create Job '{}'", manifest.metadata.name);
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_job_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!("[warning] Job spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "CronJob" | "CronJobs" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create CronJob '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_cronjob_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!("[warning] CronJob spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "NetworkPolicy" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create NetworkPolicy '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_networkpolicy_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!("[warning] NetworkPolicy spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "PodDisruptionBudget" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create PodDisruptionBudget '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_poddisruptionbudget_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!(
                        "[warning] PodDisruptionBudget spec parsed as non-raw, using defaults"
                    );
                }
                Ok(ApplyResult::Applied)
            }
            "Role" => {
                if self.dry_run {
                    println!("[dry-run] Would create Role '{}'", manifest.metadata.name);
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_role_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!("[warning] Role spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "ClusterRole" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create ClusterRole '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_cluster_role_raw(&manifest.metadata.name, spec)
                        .await?;
                } else {
                    println!("[warning] ClusterRole spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "RoleBinding" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create RoleBinding '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_role_binding_raw(
                        &manifest.metadata.name,
                        &manifest.metadata.namespace.as_deref().unwrap_or("default"),
                        spec,
                    )
                    .await?;
                } else {
                    println!("[warning] RoleBinding spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            "ClusterRoleBinding" => {
                if self.dry_run {
                    println!(
                        "[dry-run] Would create ClusterRoleBinding '{}'",
                        manifest.metadata.name
                    );
                    return Ok(ApplyResult::DryRun);
                }
                if let ManifestSpec::Raw(spec) = &manifest.spec {
                    self.apply_cluster_role_binding_raw(&manifest.metadata.name, spec)
                        .await?;
                } else {
                    println!("[warning] ClusterRoleBinding spec parsed as non-raw, using defaults");
                }
                Ok(ApplyResult::Applied)
            }
            _ => {
                return Err(A3sError::Project(format!(
                    "unknown kind: {}",
                    manifest.kind
                )));
            }
        }
    }

    /// Apply a Deployment by writing desired state to StateStore.
    /// The reconciliation loop will create actual pods.
    async fn apply_deployment(&self, name: &str, spec: &DeploymentSpec) -> Result<String> {
        let store = StateStore::new(&PathBuf::from("."));

        // Extract container info
        let container = spec
            .template
            .spec
            .containers
            .first()
            .ok_or_else(|| A3sError::Project("no containers in pod template".to_string()))?;

        // Build environment vars
        let mut env = HashMap::new();
        for env_var in &container.env {
            env.insert(
                env_var.name.clone(),
                env_var.value.clone().unwrap_or_default(),
            );
        }

        // Build port mappings
        let ports: Vec<PortMapping> = container
            .ports
            .iter()
            .map(|p| PortMapping {
                name: format!("port-{}", p.container_port),
                container_port: p.container_port,
                host_port: 0,
                protocol: p.protocol.clone().unwrap_or_else(|| "tcp".to_string()),
            })
            .collect();

        // Build resource requirements
        let resources = if let Some(ref reqs) = container.resources {
            ResourceRequirements {
                memory_limit: reqs.limits.get("memory").cloned(),
                cpu_limit: reqs.limits.get("cpu").cloned(),
                memory_request: None,
                cpu_request: None,
            }
        } else {
            ResourceRequirements::default()
        };

        // Create desired state
        let deployment = DeploymentDesired {
            name: name.to_string(),
            namespace: "default".to_string(),
            image: container.image.clone(),
            replicas: spec.replicas,
            env,
            ports,
            version: "v1".to_string(),
            strategy: UpdateStrategy::RollingUpdate(RollingUpdateConfig::default()),
            resources,
            health_check: HealthCheckConfig {
                enabled: true,
                path: Some("/health".to_string()),
                port: container.ports.first().map(|p| p.container_port),
                ..Default::default()
            },
            node_selector: Default::default(),
            tolerations: vec![],
            labels: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        // Save to state store
        store.set_deployment(deployment)?;

        println!("Deployment '{}' desired state saved", name);
        println!(
            "Reconciliation loop will create {} replica(s)",
            spec.replicas
        );

        Ok(name.to_string())
    }

    /// Apply a DaemonSet by writing desired state to StateStore.
    async fn apply_daemonset(
        &self,
        name: &str,
        namespace: &Option<String>,
        spec: &crate::state::DaemonSetSpec,
    ) -> Result<()> {
        use crate::state::DaemonSetDesired;
        use chrono::Utc;

        let store = StateStore::new(&PathBuf::from("."));

        let namespace = namespace.clone().unwrap_or_else(|| "default".to_string());

        let daemonset = DaemonSetDesired {
            name: name.to_string(),
            namespace: namespace.clone(),
            spec: spec.clone(),
            version: "v1".to_string(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_daemonset(daemonset)?;

        println!(
            "DaemonSet '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        println!("DaemonSet reconciliation will ensure pods run on matching nodes");

        Ok(())
    }

    /// Apply a Service by writing desired state to StateStore.
    async fn apply_service(
        &self,
        name: &str,
        spec: &crate::config::manifest::ServiceSpec,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        let service = ServiceDesired {
            name: name.to_string(),
            namespace: "default".to_string(),
            service_type: crate::state::ServiceType::ClusterIP,
            ports: spec
                .ports
                .iter()
                .map(|p| ServicePort {
                    name: p.name.clone(),
                    port: p.port,
                    target_port: p.target_port.unwrap_or(p.port),
                    protocol: p.protocol.clone().unwrap_or_else(|| "tcp".to_string()),
                })
                .collect(),
            selector: spec.selector.clone(),
            labels: std::collections::HashMap::new(),
            created_at: Utc::now(),
        };

        store.set_service(service)?;

        println!("Service '{}' desired state saved", name);
        Ok(())
    }

    /// Apply a ConfigMap by writing desired state to StateStore.
    async fn apply_configmap(&self, name: &str, manifest: &Manifest) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        let spec = match &manifest.spec {
            ManifestSpec::ConfigMap(cs) => cs,
            _ => {
                return Err(A3sError::Project(
                    "ConfigMap spec missing or invalid".to_string(),
                ));
            }
        };

        let configmap = ConfigMapDesired {
            name: name.to_string(),
            namespace: manifest
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            data: spec.data.clone(),
            binary_data: spec.binary_data.clone(),
            immutable: spec.immutable,
            labels: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_configmap(configmap)?;

        println!("ConfigMap '{}' desired state saved", name);
        Ok(())
    }

    /// Apply a Secret by writing desired state to StateStore.
    async fn apply_secret(&self, name: &str, manifest: &Manifest) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        let spec = match &manifest.spec {
            ManifestSpec::Secret(ss) => ss,
            _ => {
                return Err(A3sError::Project(
                    "Secret spec missing or invalid".to_string(),
                ));
            }
        };

        // If stringData is provided, encode it to base64
        let mut data = spec.data.clone();
        for (key, value) in &spec.string_data {
            let encoded = base64_encode(value);
            data.insert(key.clone(), encoded);
        }

        let secret = SecretDesired {
            name: name.to_string(),
            namespace: manifest
                .metadata
                .namespace
                .clone()
                .unwrap_or_else(|| "default".to_string()),
            secret_type: serde_json::to_string(&spec.secret_type)
                .unwrap_or_else(|_| "\"Opaque\"".to_string()),
            data,
            string_data: spec.string_data.clone(),
            immutable: spec.immutable,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_secret(secret)?;

        println!("Secret '{}' desired state saved", name);
        Ok(())
    }

    /// Apply a Namespace by writing desired state to StateStore.
    async fn apply_namespace(&self, name: &str) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        let namespace_meta = NamespaceMeta {
            name: name.to_string(),
            labels: std::collections::HashMap::new(),
            annotations: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_namespace(namespace_meta)?;

        println!("Namespace '{}' created", name);
        Ok(())
    }

    /// Apply an Ingress from raw JSON spec.
    async fn apply_ingress_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse rules from spec
        let rules: Vec<IngressRule> = spec
            .get("rules")
            .and_then(|r| r.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|r| {
                        Some(IngressRule {
                            host: r
                                .get("host")
                                .and_then(|h| h.as_str())
                                .map(String::from)
                                .unwrap_or_default(),
                            paths: r
                                .get("paths")
                                .and_then(|p| p.as_array())
                                .map(|paths| {
                                    paths
                                        .iter()
                                        .filter_map(|p| {
                                            let backend = p
                                                .get("backend")
                                                .and_then(|b| {
                                                    b.get("service")
                                                        .and_then(|s| s.get("name"))
                                                        .and_then(|v| v.as_str())
                                                })
                                                .map(String::from)
                                                .unwrap_or_else(|| "default".to_string());
                                            let port = p
                                                .get("backend")
                                                .and_then(|b| {
                                                    b.get("service").and_then(|s| {
                                                        s.get("port").and_then(|v| v.as_i64())
                                                    })
                                                })
                                                .unwrap_or(80)
                                                as u16;
                                            Some(IngressPath {
                                                path: p
                                                    .get("path")
                                                    .and_then(|v| v.as_str())
                                                    .unwrap_or("/")
                                                    .to_string(),
                                                backend,
                                                port,
                                            })
                                        })
                                        .collect()
                                })
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let tls: Vec<IngressTls> = spec
            .get("tls")
            .and_then(|t| t.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|t| {
                        Some(IngressTls {
                            hosts: t
                                .get("hosts")
                                .and_then(|h| h.as_array())
                                .map(|h| {
                                    h.iter()
                                        .filter_map(|v| v.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            secret_name: t
                                .get("secretName")
                                .and_then(|s| s.as_str())
                                .map(String::from)
                                .unwrap_or_default(),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let ingress = IngressDesired {
            name: name.to_string(),
            namespace: namespace.to_string(),
            spec: IngressSpec { rules, tls },
            labels: std::collections::HashMap::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_ingress(ingress)?;

        println!(
            "Ingress '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a Job from raw JSON spec.
    async fn apply_job_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        use crate::state::batch::{
            BackoffLimitPolicy, CompletionMode, JobPodMetadata, JobPodSpec, JobPodTemplate,
            JobRestartPolicy, JobRetryPolicy,
        };

        let store = StateStore::new(&PathBuf::from("."));

        let parallelism = spec
            .get("parallelism")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;
        let completions = spec
            .get("completions")
            .and_then(|v| v.as_i64())
            .unwrap_or(1) as i32;
        let backoff_limit = spec
            .get("backoffLimit")
            .and_then(|v| v.as_i64())
            .unwrap_or(6) as u32;

        // Parse template from spec
        let template = spec
            .get("template")
            .and_then(|t| t.get("spec"))
            .map(|template_spec| {
                let containers = template_spec
                    .get("containers")
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .map(|container| {
                        let image = container
                            .get("image")
                            .and_then(|v| v.as_str())
                            .unwrap_or("busybox")
                            .to_string();
                        let command = container
                            .get("command")
                            .and_then(|c| c.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let args = container
                            .get("args")
                            .and_then(|a| a.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let env = container
                            .get("env")
                            .and_then(|e| e.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|e| {
                                        Some((
                                            e.get("name")?.as_str()?.to_string(),
                                            e.get("value")?.as_str()?.to_string(),
                                        ))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        JobPodSpec {
                            image,
                            command,
                            args,
                            env,
                            resources: crate::state::batch::JobResourceRequirements::default(),
                            restart_policy: container
                                .get("restartPolicy")
                                .and_then(|r| r.as_str())
                                .map(|r| match r {
                                    "OnFailure" => JobRestartPolicy::OnFailure,
                                    _ => JobRestartPolicy::Never,
                                })
                                .unwrap_or(JobRestartPolicy::Never),
                        }
                    });
                JobPodTemplate {
                    metadata: JobPodMetadata::default(),
                    spec: containers.unwrap_or_else(|| JobPodSpec {
                        image: "busybox".to_string(),
                        command: vec!["echo".to_string(), "hello".to_string()],
                        args: vec![],
                        env: HashMap::new(),
                        resources: crate::state::batch::JobResourceRequirements::default(),
                        restart_policy: JobRestartPolicy::Never,
                    }),
                }
            })
            .unwrap_or_else(|| JobPodTemplate {
                metadata: JobPodMetadata::default(),
                spec: JobPodSpec {
                    image: "busybox".to_string(),
                    command: vec!["echo".to_string(), "hello".to_string()],
                    args: vec![],
                    env: HashMap::new(),
                    resources: crate::state::batch::JobResourceRequirements::default(),
                    restart_policy: JobRestartPolicy::Never,
                },
            });

        let job = JobDesired {
            name: name.to_string(),
            namespace: namespace.to_string(),
            spec: crate::state::JobSpec {
                parallelism: Some(parallelism),
                completions: Some(completions),
                backoff_limit: BackoffLimitPolicy::Limited(backoff_limit),
                retry_policy: JobRetryPolicy::default(),
                template,
                completion_mode: CompletionMode::default(),
                ttl_seconds_after_finished: None,
                active_deadline_seconds: None,
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_job(job)?;

        println!(
            "Job '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a CronJob from raw JSON spec.
    async fn apply_cronjob_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        use crate::state::batch::{
            BackoffLimitPolicy, CompletionMode, CronJobSchedule, JobPodMetadata, JobPodSpec,
            JobPodTemplate, JobRestartPolicy, JobRetryPolicy,
        };

        let store = StateStore::new(&PathBuf::from("."));

        let schedule_str = spec
            .get("schedule")
            .and_then(|v| v.as_str())
            .unwrap_or("*/5 * * * *");
        let schedule = CronJobSchedule::parse(schedule_str).map_err(|e| {
            A3sError::Project(format!("invalid cron schedule '{}': {}", schedule_str, e))
        })?;
        let suspend = spec
            .get("suspend")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        let time_zone = spec
            .get("timeZone")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse job template from CronJob spec
        let job_template_spec = spec
            .get("jobTemplate")
            .and_then(|jt| jt.get("spec"))
            .map(|template_spec| {
                let parallelism = template_spec
                    .get("parallelism")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i32;
                let completions = template_spec
                    .get("completions")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(1) as i32;
                let backoff_limit = template_spec
                    .get("backoffLimit")
                    .and_then(|v| v.as_i64())
                    .unwrap_or(6) as u32;

                // Parse containers
                let containers = template_spec
                    .get("template")
                    .and_then(|t| t.get("spec"))
                    .and_then(|spec| spec.get("containers"))
                    .and_then(|c| c.as_array())
                    .and_then(|arr| arr.first())
                    .map(|container| {
                        let image = container
                            .get("image")
                            .and_then(|v| v.as_str())
                            .unwrap_or("busybox")
                            .to_string();
                        let command = container
                            .get("command")
                            .and_then(|c| c.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let args = container
                            .get("args")
                            .and_then(|a| a.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|v| v.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();
                        let env = container
                            .get("env")
                            .and_then(|e| e.as_array())
                            .map(|arr| {
                                arr.iter()
                                    .filter_map(|e| {
                                        Some((
                                            e.get("name")?.as_str()?.to_string(),
                                            e.get("value")?.as_str()?.to_string(),
                                        ))
                                    })
                                    .collect()
                            })
                            .unwrap_or_default();
                        JobPodSpec {
                            image,
                            command,
                            args,
                            env,
                            resources: crate::state::batch::JobResourceRequirements::default(),
                            restart_policy: container
                                .get("restartPolicy")
                                .and_then(|r| r.as_str())
                                .map(|r| match r {
                                    "OnFailure" => JobRestartPolicy::OnFailure,
                                    _ => JobRestartPolicy::Never,
                                })
                                .unwrap_or(JobRestartPolicy::Never),
                        }
                    });
                crate::state::JobSpec {
                    parallelism: Some(parallelism),
                    completions: Some(completions),
                    backoff_limit: BackoffLimitPolicy::Limited(backoff_limit),
                    retry_policy: JobRetryPolicy::default(),
                    template: JobPodTemplate {
                        metadata: JobPodMetadata::default(),
                        spec: containers.unwrap_or_else(|| JobPodSpec {
                            image: "busybox".to_string(),
                            command: vec!["echo".to_string(), "hello".to_string()],
                            args: vec![],
                            env: HashMap::new(),
                            resources: crate::state::batch::JobResourceRequirements::default(),
                            restart_policy: JobRestartPolicy::Never,
                        }),
                    },
                    completion_mode: CompletionMode::default(),
                    ttl_seconds_after_finished: None,
                    active_deadline_seconds: None,
                }
            })
            .unwrap_or_else(|| crate::state::JobSpec {
                parallelism: Some(1),
                completions: Some(1),
                backoff_limit: BackoffLimitPolicy::Limited(6),
                retry_policy: JobRetryPolicy::default(),
                template: JobPodTemplate {
                    metadata: JobPodMetadata::default(),
                    spec: JobPodSpec {
                        image: "busybox".to_string(),
                        command: vec!["echo".to_string(), "hello".to_string()],
                        args: vec![],
                        env: HashMap::new(),
                        resources: crate::state::batch::JobResourceRequirements::default(),
                        restart_policy: JobRestartPolicy::Never,
                    },
                },
                completion_mode: CompletionMode::default(),
                ttl_seconds_after_finished: None,
                active_deadline_seconds: None,
            });

        let cronjob = CronJobDesired {
            name: name.to_string(),
            namespace: namespace.to_string(),
            spec: crate::state::CronJobSpec {
                schedule,
                time_zone,
                suspend,
                concurrency_policy: crate::state::CronJobConcurrencyPolicy::Allow,
                failed_jobs_history_limit: Some(3),
                successful_jobs_history_limit: Some(3),
                starting_deadline_seconds: None,
                job_template: crate::state::JobTemplate {
                    spec: job_template_spec,
                },
            },
            last_scheduled_time: None,
            active_jobs: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_cronjob(cronjob)?;

        println!(
            "CronJob '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a NetworkPolicy from raw JSON spec.
    async fn apply_networkpolicy_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse pod selector
        let pod_selector = spec
            .get("podSelector")
            .and_then(|ps| ps.as_object())
            .map(|obj| {
                let match_labels = obj
                    .get("matchLabels")
                    .and_then(|ml| ml.as_object())
                    .map(|m| {
                        m.iter()
                            .map(|(k, v)| (k.clone(), v.as_str().unwrap_or("").to_string()))
                            .collect()
                    })
                    .unwrap_or_default();
                LabelSelector {
                    match_labels,
                    match_expressions: Vec::new(),
                }
            })
            .unwrap_or_default();

        // Parse policy types
        let policy_types: Vec<PolicyType> = spec
            .get("policyTypes")
            .and_then(|pt| pt.as_array())
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

        let network_policy = NetworkPolicyDesired {
            name: name.to_string(),
            namespace: namespace.to_string(),
            spec: NetworkPolicySpec {
                pod_selector,
                policy_types,
                ingress: Vec::new(),
                egress: Vec::new(),
            },
            created_at: Utc::now(),
            is_default_deny: spec
                .get("defaultDeny")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        };

        store.add_network_policy(network_policy)?;

        println!(
            "NetworkPolicy '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a PodDisruptionBudget from raw JSON spec.
    async fn apply_poddisruptionbudget_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse minAvailable
        let min_available = spec
            .get("minAvailable")
            .and_then(|v| v.as_str())
            .map(String::from);

        // Parse maxUnavailable (API uses maxUnavailable but we store as maxDisruptions)
        let max_disruptions = spec
            .get("maxUnavailable")
            .and_then(|v| v.as_str())
            .map(String::from);

        let pdb = PodDisruptionBudgetDesired {
            name: name.to_string(),
            namespace: namespace.to_string(),
            spec: PodDisruptionBudgetSpec {
                min_available,
                max_disruptions,
            },
            created_at: Utc::now(),
        };

        store.set_pod_disruption_budget(pdb)?;

        println!(
            "PodDisruptionBudget '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a Role from raw spec.
    async fn apply_role_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse rules
        let rules: Vec<PolicyRule> = spec
            .get("rules")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|rule| {
                        let verbs: Vec<String> = rule
                            .get("verbs")
                            .and_then(|v| v.as_array())
                            .map(|v| {
                                v.iter()
                                    .filter_map(|s| s.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();

                        if verbs.is_empty() {
                            return None;
                        }

                        Some(PolicyRule {
                            api_groups: rule
                                .get("apiGroups")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resource_names: rule
                                .get("resourceNames")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resources: rule
                                .get("resources")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            non_resource_u_r_l_s: rule
                                .get("nonResourceURLs")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            non_resource_u_r_l_path_suffix: rule
                                .get("nonResourceURLPathSuffix")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            verbs,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let role = RoleDesired {
            name: name.to_string(),
            namespace: Some(namespace.to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            rules,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_role(role)?;

        println!(
            "Role '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a ClusterRole from raw spec.
    async fn apply_cluster_role_raw(&self, name: &str, spec: &serde_json::Value) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse rules
        let rules: Vec<PolicyRule> = spec
            .get("rules")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|rule| {
                        let verbs: Vec<String> = rule
                            .get("verbs")
                            .and_then(|v| v.as_array())
                            .map(|v| {
                                v.iter()
                                    .filter_map(|s| s.as_str().map(String::from))
                                    .collect()
                            })
                            .unwrap_or_default();

                        if verbs.is_empty() {
                            return None;
                        }

                        Some(PolicyRule {
                            api_groups: rule
                                .get("apiGroups")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resource_names: rule
                                .get("resourceNames")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            resources: rule
                                .get("resources")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            non_resource_u_r_l_s: rule
                                .get("nonResourceURLs")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            non_resource_u_r_l_path_suffix: rule
                                .get("nonResourceURLPathSuffix")
                                .and_then(|v| v.as_array())
                                .map(|v| {
                                    v.iter()
                                        .filter_map(|s| s.as_str().map(String::from))
                                        .collect()
                                })
                                .unwrap_or_default(),
                            verbs,
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cluster_role = ClusterRoleDesired {
            name: name.to_string(),
            labels: Default::default(),
            annotations: Default::default(),
            rules,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_cluster_role(cluster_role)?;

        println!("ClusterRole '{}' desired state saved", name);
        Ok(())
    }

    /// Apply a RoleBinding from raw spec.
    async fn apply_role_binding_raw(
        &self,
        name: &str,
        namespace: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse roleRef
        let role_ref = spec
            .get("roleRef")
            .and_then(|v| {
                Some(RoleRef {
                    kind: v
                        .get("kind")
                        .and_then(|s| s.as_str())
                        .map(String::from)
                        .unwrap_or_default(),
                    name: v
                        .get("name")
                        .and_then(|s| s.as_str())
                        .map(String::from)
                        .unwrap_or_default(),
                    api_group: v.get("apiGroup").and_then(|s| s.as_str()).map(String::from),
                })
            })
            .unwrap_or_else(|| RoleRef {
                kind: "Role".to_string(),
                name: name.to_string(),
                api_group: None,
            });

        // Parse subjects
        let subjects: Vec<Subject> = spec
            .get("subjects")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|subj| {
                        Some(Subject {
                            kind: subj
                                .get("kind")
                                .and_then(|s| s.as_str())
                                .map(String::from)?,
                            name: subj
                                .get("name")
                                .and_then(|s| s.as_str())
                                .map(String::from)?,
                            api_group: subj
                                .get("apiGroup")
                                .and_then(|s| s.as_str())
                                .map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let role_binding = RoleBindingDesired {
            name: name.to_string(),
            namespace: Some(namespace.to_string()),
            labels: Default::default(),
            annotations: Default::default(),
            role_ref,
            subjects,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_role_binding(role_binding)?;

        println!(
            "RoleBinding '{}' desired state saved in namespace '{}'",
            name, namespace
        );
        Ok(())
    }

    /// Apply a ClusterRoleBinding from raw spec.
    async fn apply_cluster_role_binding_raw(
        &self,
        name: &str,
        spec: &serde_json::Value,
    ) -> Result<()> {
        let store = StateStore::new(&PathBuf::from("."));

        // Parse roleRef
        let role_ref = spec
            .get("roleRef")
            .and_then(|v| {
                Some(RoleRef {
                    kind: v
                        .get("kind")
                        .and_then(|s| s.as_str())
                        .map(String::from)
                        .unwrap_or_default(),
                    name: v
                        .get("name")
                        .and_then(|s| s.as_str())
                        .map(String::from)
                        .unwrap_or_default(),
                    api_group: v.get("apiGroup").and_then(|s| s.as_str()).map(String::from),
                })
            })
            .unwrap_or_else(|| RoleRef {
                kind: "ClusterRole".to_string(),
                name: name.to_string(),
                api_group: None,
            });

        // Parse subjects
        let subjects: Vec<Subject> = spec
            .get("subjects")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|subj| {
                        Some(Subject {
                            kind: subj
                                .get("kind")
                                .and_then(|s| s.as_str())
                                .map(String::from)?,
                            name: subj
                                .get("name")
                                .and_then(|s| s.as_str())
                                .map(String::from)?,
                            api_group: subj
                                .get("apiGroup")
                                .and_then(|s| s.as_str())
                                .map(String::from),
                        })
                    })
                    .collect()
            })
            .unwrap_or_default();

        let cluster_role_binding = ClusterRoleBindingDesired {
            name: name.to_string(),
            labels: Default::default(),
            annotations: Default::default(),
            role_ref,
            subjects,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };

        store.set_cluster_role_binding(cluster_role_binding)?;

        println!("ClusterRoleBinding '{}' desired state saved", name);
        Ok(())
    }

    /// Wait for sandbox to be ready.
    async fn wait_for_ready(&self, sandbox_id: &str, timeout_secs: u64) -> Result<()> {
        let start = std::time::Instant::now();
        let timeout = std::time::Duration::from_secs(timeout_secs);

        let socket_path = dirs::home_dir()
            .map(|h| {
                h.join(".a3s")
                    .join("boxes")
                    .join(sandbox_id)
                    .join("sockets")
                    .join("exec.sock")
            })
            .unwrap_or_else(|| {
                PathBuf::from(format!("~/.a3s/boxes/{}/sockets/exec.sock", sandbox_id))
            });

        while start.elapsed() < timeout {
            if socket_path.exists() {
                // Try to connect and execute a simple command
                let provider = MicrovmProvider::new(&PathBuf::from(".")).await;
                match provider
                    .exec_in_sandbox(sandbox_id, "echo", &["ready"])
                    .await
                {
                    Ok(result) if result.exit_code == 0 => {
                        println!("Deployment ready.");
                        return Ok(());
                    }
                    _ => {}
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        }

        Err(A3sError::Project(format!(
            "timeout waiting for deployment {} to be ready",
            sandbox_id
        )))
    }
}

/// Convert HCL body to Manifest.
fn hcl_body_to_manifest(body: &hcl::Body) -> Result<Manifest> {
    let mut api_version = None;
    let mut kind = None;
    let mut metadata = None;
    let mut spec = None;

    for structure in body.iter() {
        match structure {
            hcl::Structure::Attribute(attr) => {
                let key = attr.key().to_string();
                let value = hcl_expr_to_json_value(attr.expr());
                match key.as_str() {
                    "apiVersion" => {
                        api_version = value.as_str().map(String::from);
                    }
                    "kind" => {
                        kind = value.as_str().map(String::from);
                    }
                    "spec" => {
                        spec = Some(value);
                    }
                    _ => {}
                }
            }
            hcl::Structure::Block(block) => {
                let identifier = block.identifier();
                match identifier {
                    "metadata" => {
                        metadata = Some(hcl_block_to_json_object(block));
                    }
                    "spec" => {
                        spec = Some(hcl_block_to_json_object(block));
                    }
                    _ => {}
                }
            }
        }
    }

    let api_version =
        api_version.ok_or_else(|| A3sError::Project("manifest missing apiVersion".to_string()))?;
    let kind = kind.ok_or_else(|| A3sError::Project("manifest missing kind".to_string()))?;
    let metadata = metadata.unwrap_or(serde_json::Value::Object(serde_json::Map::new()));

    let manifest = Manifest {
        api_version,
        kind,
        metadata: serde_json::from_value(metadata)
            .map_err(|e| A3sError::Project(format!("failed to parse metadata: {}", e)))?,
        spec: spec
            .map(|s| serde_json::from_value(s))
            .transpose()
            .map_err(|e| A3sError::Project(format!("failed to parse spec: {}", e)))?
            .unwrap_or_default(),
    };

    Ok(manifest)
}

/// Convert an HCL Expression to a JSON value.
fn hcl_expr_to_json_value(expr: &hcl::Expression) -> serde_json::Value {
    use hcl::Expression;
    match expr {
        Expression::Bool(b) => serde_json::Value::Bool(*b),
        Expression::Number(n) => serde_json::json!(n.clone()),
        Expression::String(s) => serde_json::Value::String(s.clone()),
        Expression::Array(arr) => {
            let values: Vec<serde_json::Value> = arr.iter().map(hcl_expr_to_json_value).collect();
            serde_json::Value::Array(values)
        }
        Expression::Object(obj) => {
            let map: serde_json::Map<String, serde_json::Value> = obj
                .iter()
                .map(|(k, v)| (k.to_string(), hcl_expr_to_json_value(v)))
                .collect();
            serde_json::Value::Object(map)
        }
        Expression::Null => serde_json::Value::Null,
        other => serde_json::Value::String(other.to_string()),
    }
}

/// Convert an HCL block to a JSON object.
fn hcl_block_to_json_object(block: &hcl::Block) -> serde_json::Value {
    use hcl::Structure;

    let mut obj = serde_json::Map::new();

    for structure in block.body().iter() {
        match structure {
            Structure::Attribute(attr) => {
                let key = attr.key().to_string();
                let value = hcl_expr_to_json_value(attr.expr());
                obj.insert(key, value);
            }
            Structure::Block(nested) => {
                let nested_obj = hcl_block_to_json_object(nested);
                let identifier = nested.identifier().to_string();

                if nested.labels().len() == 1 {
                    let label = match &nested.labels()[0] {
                        hcl::BlockLabel::Identifier(ident) => ident.to_string(),
                        hcl::BlockLabel::String(s) => s.clone(),
                    };
                    let key = format!("{}_{}", identifier, label);
                    obj.insert(key, nested_obj);
                } else {
                    obj.insert(identifier, nested_obj);
                }
            }
        }
    }

    serde_json::Value::Object(obj)
}

/// Encode a string to base64.
fn base64_encode(input: &str) -> String {
    use base64::{engine::general_purpose::STANDARD, Engine};
    STANDARD.encode(input.as_bytes())
}

#[async_trait]
impl Command for ApplyCommand {
    async fn run(&self) -> Result<()> {
        if self.dry_run {
            println!("Dry run mode - no changes will be made");
            println!();
        }

        let manifests = self.parse_manifest()?;
        let mut created_ids = Vec::new();

        for manifest in &manifests {
            match self.apply_manifest(manifest).await? {
                ApplyResult::Created(id) => {
                    created_ids.push(id);
                }
                ApplyResult::Applied => {}
                ApplyResult::DryRun => {}
            }
        }

        if !self.dry_run && self.wait && !created_ids.is_empty() {
            println!("Waiting for deployments to be ready...");
            let timeout_secs = self.timeout.unwrap_or(300);
            for id in &created_ids {
                self.wait_for_ready(id, timeout_secs).await?;
            }
        }

        if self.dry_run {
            println!("Dry run complete - no changes were made.");
        } else {
            println!("Apply complete.");
        }

        Ok(())
    }
}
