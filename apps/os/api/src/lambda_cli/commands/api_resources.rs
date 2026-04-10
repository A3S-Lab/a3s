//! `a3s api-resources` command - Show supported API resources.

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;

/// API resources command - Show supported API resources.
#[derive(clap::Parser, Debug)]
pub struct ApiResourcesCommand {
    /// Group to filter by (e.g., "apps", "networking.a3s.io").
    #[arg(short, long)]
    api_group: Option<String>,

    /// Show namespaced resources.
    #[arg(long)]
    namespaced: Option<bool>,

    /// Output format.
    #[arg(short = 'o', long, default_value = "text")]
    output: String,

    /// Show verbose output.
    #[arg(short, long)]
    verbose: bool,
}

impl ApiResourcesCommand {
    /// Get API resources grouped by API version.
    fn get_resources() -> Vec<(&'static str, Vec<ResourceInfo>)> {
        vec![
            (
                "v1",
                vec![
                    ResourceInfo {
                        name: "namespaces",
                        singular_name: "Namespace",
                        namespaced: false,
                        kind: "Namespace",
                        short_names: vec!["ns"],
                        verbs: vec!["get", "list", "create", "delete"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "pods",
                        singular_name: "Pod",
                        namespaced: true,
                        kind: "Pod",
                        short_names: vec!["po"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "services",
                        singular_name: "Service",
                        namespaced: true,
                        kind: "Service",
                        short_names: vec!["svc"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "configmaps",
                        singular_name: "ConfigMap",
                        namespaced: true,
                        kind: "ConfigMap",
                        short_names: vec!["cm"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "secrets",
                        singular_name: "Secret",
                        namespaced: true,
                        kind: "Secret",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "persistentvolumes",
                        singular_name: "PersistentVolume",
                        namespaced: false,
                        kind: "PersistentVolume",
                        short_names: vec!["pv"],
                        verbs: vec!["get", "list"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "persistentvolumeclaims",
                        singular_name: "PersistentVolumeClaim",
                        namespaced: true,
                        kind: "PersistentVolumeClaim",
                        short_names: vec!["pvc"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "serviceaccounts",
                        singular_name: "ServiceAccount",
                        namespaced: true,
                        kind: "ServiceAccount",
                        short_names: vec!["sa"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "nodes",
                        singular_name: "Node",
                        namespaced: false,
                        kind: "Node",
                        short_names: vec!["no"],
                        verbs: vec!["get", "list"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "events",
                        singular_name: "Event",
                        namespaced: true,
                        kind: "Event",
                        short_names: vec!["ev"],
                        verbs: vec!["get", "list"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "endpoints",
                        singular_name: "Endpoints",
                        namespaced: true,
                        kind: "Endpoints",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "componentstatuses",
                        singular_name: "ComponentStatus",
                        namespaced: false,
                        kind: "ComponentStatus",
                        short_names: vec!["cs"],
                        verbs: vec!["get", "list"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "limitranges",
                        singular_name: "LimitRange",
                        namespaced: true,
                        kind: "LimitRange",
                        short_names: vec!["limits"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "resourcequotas",
                        singular_name: "ResourceQuota",
                        namespaced: true,
                        kind: "ResourceQuota",
                        short_names: vec!["quota"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                ],
            ),
            (
                "apps/v1",
                vec![
                    ResourceInfo {
                        name: "deployments",
                        singular_name: "Deployment",
                        namespaced: true,
                        kind: "Deployment",
                        short_names: vec!["deploy"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "daemonsets",
                        singular_name: "DaemonSet",
                        namespaced: true,
                        kind: "DaemonSet",
                        short_names: vec!["ds"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "statefulsets",
                        singular_name: "StatefulSet",
                        namespaced: true,
                        kind: "StatefulSet",
                        short_names: vec!["sts"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "replicasets",
                        singular_name: "ReplicaSet",
                        namespaced: true,
                        kind: "ReplicaSet",
                        short_names: vec!["rs"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                ],
            ),
            (
                "networking.a3s.io/v1",
                vec![
                    ResourceInfo {
                        name: "ingresses",
                        singular_name: "Ingress",
                        namespaced: true,
                        kind: "Ingress",
                        short_names: vec!["ing"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "networkpolicies",
                        singular_name: "NetworkPolicy",
                        namespaced: true,
                        kind: "NetworkPolicy",
                        short_names: vec!["netpol"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                ],
            ),
            (
                "networking.k8s.io/v1",
                vec![ResourceInfo {
                    name: "ingressclasses",
                    singular_name: "IngressClass",
                    namespaced: false,
                    kind: "IngressClass",
                    short_names: vec![],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "batch/v1",
                vec![
                    ResourceInfo {
                        name: "jobs",
                        singular_name: "Job",
                        namespaced: true,
                        kind: "Job",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                    ResourceInfo {
                        name: "cronjobs",
                        singular_name: "CronJob",
                        namespaced: true,
                        kind: "CronJob",
                        short_names: vec!["cj"],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec!["all"],
                    },
                ],
            ),
            (
                "autoscaling/v1",
                vec![ResourceInfo {
                    name: "horizontalpodautoscalers",
                    singular_name: "HorizontalPodAutoscaler",
                    namespaced: true,
                    kind: "HorizontalPodAutoscaler",
                    short_names: vec!["hpa"],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec!["all"],
                }],
            ),
            (
                "rbac.authorization.a3s.io/v1",
                vec![
                    ResourceInfo {
                        name: "roles",
                        singular_name: "Role",
                        namespaced: true,
                        kind: "Role",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "clusterroles",
                        singular_name: "ClusterRole",
                        namespaced: false,
                        kind: "ClusterRole",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "rolebindings",
                        singular_name: "RoleBinding",
                        namespaced: true,
                        kind: "RoleBinding",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "clusterrolebindings",
                        singular_name: "ClusterRoleBinding",
                        namespaced: false,
                        kind: "ClusterRoleBinding",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                ],
            ),
            (
                "scheduling.k8s.io/v1",
                vec![ResourceInfo {
                    name: "priorityclasses",
                    singular_name: "PriorityClass",
                    namespaced: false,
                    kind: "PriorityClass",
                    short_names: vec![],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "storage.a3s.io/v1",
                vec![ResourceInfo {
                    name: "storageclasses",
                    singular_name: "StorageClass",
                    namespaced: false,
                    kind: "StorageClass",
                    short_names: vec!["sc"],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "discovery.a3s.io/v1",
                vec![ResourceInfo {
                    name: "endpointslices",
                    singular_name: "EndpointSlice",
                    namespaced: true,
                    kind: "EndpointSlice",
                    short_names: vec![],
                    verbs: vec!["get", "list"],
                    categories: vec![],
                }],
            ),
            (
                "policy/v1beta1",
                vec![ResourceInfo {
                    name: "poddisruptionbudgets",
                    singular_name: "PodDisruptionBudget",
                    namespaced: true,
                    kind: "PodDisruptionBudget",
                    short_names: vec!["pdb"],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "coordination.k8s.io/v1",
                vec![ResourceInfo {
                    name: "leases",
                    singular_name: "Lease",
                    namespaced: true,
                    kind: "Lease",
                    short_names: vec![],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "storage.k8s.io/v1",
                vec![
                    ResourceInfo {
                        name: "csidrivers",
                        singular_name: "CSIDriver",
                        namespaced: false,
                        kind: "CSIDriver",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "csinodes",
                        singular_name: "CSINode",
                        namespaced: false,
                        kind: "CSINode",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "volumeattachments",
                        singular_name: "VolumeAttachment",
                        namespaced: false,
                        kind: "VolumeAttachment",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                ],
            ),
            (
                "node.k8s.io/v1",
                vec![ResourceInfo {
                    name: "runtimeclasses",
                    singular_name: "RuntimeClass",
                    namespaced: false,
                    kind: "RuntimeClass",
                    short_names: vec![],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "flowcontrol.apiserver.k8s.io/v1",
                vec![
                    ResourceInfo {
                        name: "flowschemas",
                        singular_name: "FlowSchema",
                        namespaced: false,
                        kind: "FlowSchema",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "prioritylevelconfigurations",
                        singular_name: "PriorityLevelConfiguration",
                        namespaced: false,
                        kind: "PriorityLevelConfiguration",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                ],
            ),
            (
                "certificates.k8s.io/v1",
                vec![ResourceInfo {
                    name: "certificatesigningrequests",
                    singular_name: "CertificateSigningRequest",
                    namespaced: false,
                    kind: "CertificateSigningRequest",
                    short_names: vec!["csr"],
                    verbs: vec!["get", "list", "create", "delete", "patch"],
                    categories: vec![],
                }],
            ),
            (
                "admissionregistration.k8s.io/v1",
                vec![
                    ResourceInfo {
                        name: "mutatingwebhookconfigurations",
                        singular_name: "MutatingWebhookConfiguration",
                        namespaced: false,
                        kind: "MutatingWebhookConfiguration",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                    ResourceInfo {
                        name: "validatingwebhookconfigurations",
                        singular_name: "ValidatingWebhookConfiguration",
                        namespaced: false,
                        kind: "ValidatingWebhookConfiguration",
                        short_names: vec![],
                        verbs: vec!["get", "list", "create", "delete", "patch"],
                        categories: vec![],
                    },
                ],
            ),
        ]
    }

    /// Print API resources.
    fn print_resources(&self) -> Result<()> {
        let all_resources = Self::get_resources();

        // Filter by API group if specified
        let resources: Vec<_> = if let Some(ref group) = self.api_group {
            all_resources
                .into_iter()
                .filter(|(g, _)| g.contains(group))
                .collect()
        } else {
            all_resources
        };

        // Filter by namespaced status if specified
        let resources: Vec<_> = if let Some(namespaced) = self.namespaced {
            resources
                .into_iter()
                .map(|(g, r)| {
                    (
                        g,
                        r.into_iter()
                            .filter(|r| r.namespaced == namespaced)
                            .collect::<Vec<_>>(),
                    )
                })
                .filter(|(_, r)| !r.is_empty())
                .collect()
        } else {
            resources
        };

        match self.output.as_str() {
            "json" => {
                let json_resources: serde_json::Value = serde_json::json!({
                    "apiVersion": "v1",
                    "kind": "APIResourceList",
                    "groupVersion": "v1",
                    "resources": resources.iter().flat_map(|(_, r)| r).map(|r| {
                        serde_json::json!({
                            "name": r.name,
                            "singularName": r.singular_name,
                            "namespaced": r.namespaced,
                            "kind": r.kind,
                            "shortNames": r.short_names,
                            "verbs": r.verbs
                        })
                    }).collect::<Vec<_>>()
                });
                println!("{}", serde_json::to_string_pretty(&json_resources).unwrap());
            }
            "wide" | "text" => {
                if self.verbose {
                    println!(
                        "{:<40} {:<20} {:<15} {:<15} {:<40}",
                        "NAME", "SHORT", "APIVERSION", "NAMESPACED", "KIND"
                    );
                    println!("{}", "-".repeat(130));
                } else {
                    println!(
                        "{:<40} {:<15} {:<15} {:<10}",
                        "NAME", "SHORT", "APIVERSION", "NAMESPACED"
                    );
                    println!("{}", "-".repeat(80));
                }

                for (api_version, res_list) in &resources {
                    for r in res_list {
                        let short = r.short_names.join(",");
                        if self.verbose {
                            println!(
                                "{:<40} {:<20} {:<15} {:<15} {:<40}",
                                r.name,
                                short,
                                api_version,
                                if r.namespaced { "true" } else { "false" },
                                r.kind
                            );
                        } else {
                            println!(
                                "{:<40} {:<15} {:<15} {:<10}",
                                r.name,
                                short,
                                api_version,
                                if r.namespaced { "true" } else { "false" }
                            );
                        }
                    }
                }
            }
            _ => {
                println!("Unknown output format: {}", self.output);
            }
        }

        Ok(())
    }
}

/// Resource information.
struct ResourceInfo {
    name: &'static str,
    singular_name: &'static str,
    namespaced: bool,
    kind: &'static str,
    short_names: Vec<&'static str>,
    verbs: Vec<&'static str>,
    categories: Vec<&'static str>,
}

#[async_trait]
impl Command for ApiResourcesCommand {
    async fn run(&self) -> Result<()> {
        self.print_resources()
    }
}
