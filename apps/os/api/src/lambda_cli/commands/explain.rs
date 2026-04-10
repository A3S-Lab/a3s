//! `a3s explain` command - Show documentation for resource types (kubectl explain style).

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;

/// Explain command - Show documentation for API resources.
#[derive(clap::Parser, Debug)]
pub struct ExplainCommand {
    /// Resource type to explain.
    #[arg(last = true)]
    resource: Option<String>,

    /// Show recursive field documentation.
    #[arg(short, long)]
    recursive: bool,
}

impl ExplainCommand {
    /// Get documentation for all resources.
    fn explain_all() -> String {
        r#"GROUPS
=======
batch
  cronjob      CronJob represents a time-based job.
  job          Job represents a job configuration.

networking.a3s.io
  ingress     Ingress is a collection of rules that allow inbound connections.
  networkpolicy  NetworkPolicy describes how network pods can communicate.

policy
  poddisruptionbudget  Protection to ensure availability of critical pods.

rbac.authorization.a3s.io
  clusterrole         ClusterRole is a cluster level RBAC authorization.
  clusterrolebinding  ClusterRoleBinding references a ClusterRole.
  role                Role is a namespaced RBAC authorization.
  rolebinding         RoleBinding references a Role.

storage.a3s.io
  storageclass  StorageClass describes the parameters for a class of storage.

apps
  daemonset       DaemonSet represents a set of pods that should run on nodes.
  deployment      Deployment enables declarative updates for pods and replica sets.
  replicaset      ReplicaSet represents a replicaset configuration.
  statefulset     StatefulSet represents a statefulset configuration.

core
  configmap       ConfigMap holds configuration data for pods to consume.
  endpoints       Endpoints is a collection of endpoints that implement a service.
  event           Event is a report of an event.
  namespace       Namespace provides a scope for names.
  node            Node is a worker machine in the cluster.
  persistentvolume  PersistentVolume is a storage resource.
  persistentvolumeclaim  ClaimStatus is a user's request for a persistent volume.
  pod             Pod is a collection of containers.
  secret          Secret holds secret data.
  service         Service is an abstraction for a logical set of pods.
  serviceaccount  ServiceAccount binds a user identity to a role.

discovery.a3s.io
  endpointslices  EndpointSlice represents a subset of the endpoints for a service.

RESOURCES
=========
Use "a3s explain <resource>" for more details on a resource."#
            .to_string()
    }

    /// Get documentation for a specific resource.
    fn explain_resource(resource: &str, recursive: bool) -> String {
        match resource.to_lowercase().as_str() {
            "pod" | "pods" => Self::explain_pod(recursive),
            "deployment" | "deployments" => Self::explain_deployment(recursive),
            "service" | "services" => Self::explain_service(recursive),
            "configmap" | "configmaps" => Self::explain_configmap(recursive),
            "secret" | "secrets" => Self::explain_secret(recursive),
            "namespace" | "namespaces" | "ns" => Self::explain_namespace(recursive),
            "node" | "nodes" => Self::explain_node(recursive),
            "poddisruptionbudget" | "pdb" | "pdbbudgets" => Self::explain_pdb(recursive),
            "ingress" | "ingresses" => Self::explain_ingress(recursive),
            "networkpolicy" | "networkpolicies" => Self::explain_networkpolicy(recursive),
            "cronjob" | "cronjobs" => Self::explain_cronjob(recursive),
            "job" | "jobs" => Self::explain_job(recursive),
            "statefulset" | "statefulsets" => Self::explain_statefulset(recursive),
            "daemonset" | "daemonsets" | "ds" => Self::explain_daemonset(recursive),
            "replicaset" | "replicasets" | "rs" => Self::explain_replicaset(recursive),
            "role" | "roles" => Self::explain_role(recursive),
            "clusterrole" | "clusterroles" => Self::explain_clusterrole(recursive),
            "rolebinding" | "rolebindings" => Self::explain_rolebinding(recursive),
            "clusterrolebinding" | "clusterrolebindings" => {
                Self::explain_clusterrolebinding(recursive)
            }
            "storageclass" | "storageclasses" | "sc" => Self::explain_storageclass(recursive),
            "persistentvolume" | "pv" | "persistentvolumes" => Self::explain_pv(recursive),
            "persistentvolumeclaim" | "pvc" | "persistentvolumeclaims" => {
                Self::explain_pvc(recursive)
            }
            "serviceaccount" | "serviceaccounts" | "sa" => Self::explain_serviceaccount(recursive),
            "endpoints" => Self::explain_endpoints(recursive),
            "endpointslices" | "endpointslice" => Self::explain_endpointslices(recursive),
            "event" | "events" | "ev" => Self::explain_event(recursive),
            _ => format!(
                "Unknown resource type '{}'. Use 'a3s explain' for available resources.",
                resource
            ),
        }
    }

    fn indent(s: &str) -> String {
        s.lines()
            .map(|l| format!("  {}", l))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn explain_pod(recursive: bool) -> String {
        let base = r#"KIND
====
Pod

DESCRIPTION
==========
Pod is a collection of containers that can run on a host.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the desired behavior of the pod.

status       <Object>
  Most recently observed status of the pod."#;

        if recursive {
            format!(
                r#"{}

SPEC (CONTENTS)
{}
"#,
                base,
                Self::indent(
                    r#"containers      <[]Container>
  List of containers belonging to the pod.

dnsPolicy    <string>
  Set DNS policy for the pod.

hostname     <string>
  Hostname for the pod.

nodeSelector <map[string]string>
  Node selector for scheduling.

restartPolicy <string>
  Restart policy for all containers.

terminationGracePeriodSeconds <integer>
  Grace period before forcing kill."#
                )
            )
        } else {
            base.to_string()
        }
    }

    fn explain_deployment(_recursive: bool) -> String {
        r#"KIND
====
Deployment

DESCRIPTION
==========
Deployment enables declarative updates for pods and ReplicaSets.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the desired behavior of the Deployment.

status       <Object>
  Most recently observed status of the Deployment."#
            .to_string()
    }

    fn explain_service(_recursive: bool) -> String {
        r#"KIND
====
Service

DESCRIPTION
==========
Service is an abstraction which provides a network endpoint for a set of pods.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the desired behavior of the Service.

status       <Object>
  Most recently observed status of the Service."#
            .to_string()
    }

    fn explain_configmap(_recursive: bool) -> String {
        r#"KIND
====
ConfigMap

DESCRIPTION
==========
ConfigMap holds configuration data for pods to consume.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

binaryData   <map[string]string>
  BinaryData contains the binary data.

data         <map[string]string>
  Data contains the configuration data.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata."#
            .to_string()
    }

    fn explain_secret(_recursive: bool) -> String {
        r#"KIND
====
Secret

DESCRIPTION
==========
Secret holds secret data.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

data         <map[string]string>
  Data contains the secret data.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

type         <string>
  Type of secret."#
            .to_string()
    }

    fn explain_namespace(_recursive: bool) -> String {
        r#"KIND
====
Namespace

DESCRIPTION
==========
Namespace provides a scope for names.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Spec defines the behavior of the Namespace.

status       <Object>
  Status describes the current state of the Namespace."#
            .to_string()
    }

    fn explain_node(_recursive: bool) -> String {
        r#"KIND
====
Node

DESCRIPTION
==========
Node is a worker machine in the cluster.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Spec defines the behavior of the Node.

status       <Object>
  Most recently observed status of the Node."#
            .to_string()
    }

    fn explain_pdb(_recursive: bool) -> String {
        r#"KIND
====
PodDisruptionBudget

DESCRIPTION
==========
PodDisruptionBudget is a resource that ensures high availability by preventing disruptions.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the PodDisruptionBudget.

status       <Object>
  Most recently observed status of the PodDisruptionBudget."#
            .to_string()
    }

    fn explain_ingress(_recursive: bool) -> String {
        r#"KIND
====
Ingress

DESCRIPTION
==========
Ingress is a collection of rules that allow inbound connections to reach services.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the Ingress.

status       <Object>
  Most recently observed status of the Ingress."#
            .to_string()
    }

    fn explain_networkpolicy(_recursive: bool) -> String {
        r#"KIND
====
NetworkPolicy

DESCRIPTION
==========
NetworkPolicy describes how network pods can communicate.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the NetworkPolicy.

status       <Object>
  Most recently observed status of the NetworkPolicy."#
            .to_string()
    }

    fn explain_cronjob(_recursive: bool) -> String {
        r#"KIND
====
CronJob

DESCRIPTION
==========
CronJob represents a time-based job that runs on a schedule.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the CronJob.

status       <Object>
  Most recently observed status of the CronJob."#
            .to_string()
    }

    fn explain_job(_recursive: bool) -> String {
        r#"KIND
====
Job

DESCRIPTION
==========
Job represents a job configuration.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the Job.

status       <Object>
  Most recently observed status of the Job."#
            .to_string()
    }

    fn explain_statefulset(_recursive: bool) -> String {
        r#"KIND
====
StatefulSet

DESCRIPTION
==========
StatefulSet represents a statefulset configuration.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the StatefulSet.

status       <Object>
  Most recently observed status of the StatefulSet."#
            .to_string()
    }

    fn explain_daemonset(_recursive: bool) -> String {
        r#"KIND
====
DaemonSet

DESCRIPTION
==========
DaemonSet represents a set of pods that should run on nodes.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the DaemonSet.

status       <Object>
  Most recently observed status of the DaemonSet."#
            .to_string()
    }

    fn explain_replicaset(_recursive: bool) -> String {
        r#"KIND
====
ReplicaSet

DESCRIPTION
==========
ReplicaSet represents a replicaset configuration.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the ReplicaSet.

status       <Object>
  Most recently observed status of the ReplicaSet."#
            .to_string()
    }

    fn explain_role(_recursive: bool) -> String {
        r#"KIND
====
Role

DESCRIPTION
==========
Role is a namespaced RBAC authorization.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

rules        <[]PolicyRule>
  Rules holds all the PolicyRules for this Role."#
            .to_string()
    }

    fn explain_clusterrole(_recursive: bool) -> String {
        r#"KIND
====
ClusterRole

DESCRIPTION
==========
ClusterRole is a cluster level RBAC authorization.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

rules        <[]PolicyRule>
  Rules holds all the PolicyRules for this ClusterRole."#
            .to_string()
    }

    fn explain_rolebinding(_recursive: bool) -> String {
        r#"KIND
====
RoleBinding

DESCRIPTION
==========
RoleBinding references a Role or ClusterRole and binds it to a set of subjects.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

subjects     <[]Subject>
  Subjects holds references to the objects the role applies to.

roleRef      <Object>
  RoleRef contains information about the role being used."#
            .to_string()
    }

    fn explain_clusterrolebinding(_recursive: bool) -> String {
        r#"KIND
====
ClusterRoleBinding

DESCRIPTION
==========
ClusterRoleBinding references a ClusterRole and binds it to a set of subjects.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

subjects     <[]Subject>
  Subjects holds references to the objects the role applies to.

roleRef      <Object>
  RoleRef contains information about the role being used."#
            .to_string()
    }

    fn explain_storageclass(_recursive: bool) -> String {
        r#"KIND
====
StorageClass

DESCRIPTION
==========
StorageClass describes the parameters for a class of storage.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

provisioner  <string>
  Provisioner indicates the type of the provisioner.

parameters   <map[string]string>
  Parameters for the provisioner."#
            .to_string()
    }

    fn explain_pv(_recursive: bool) -> String {
        r#"KIND
====
PersistentVolume

DESCRIPTION
==========
PersistentVolume is a storage resource.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the PersistentVolume.

status       <Object>
  Most recently observed status of the PersistentVolume."#
            .to_string()
    }

    fn explain_pvc(_recursive: bool) -> String {
        r#"KIND
====
PersistentVolumeClaim

DESCRIPTION
==========
ClaimStatus is a user's request for a persistent volume.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

spec         <Object>
  Specification of the PersistentVolumeClaim.

status       <Object>
  Most recently observed status of the PersistentVolumeClaim."#
            .to_string()
    }

    fn explain_serviceaccount(_recursive: bool) -> String {
        r#"KIND
====
ServiceAccount

DESCRIPTION
==========
ServiceAccount binds a user identity to a role.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

secrets      <[]ObjectReference>
  Secrets held by the ServiceAccount.

imagePullSecrets <[]LocalObjectReference>
  Image pull secrets for the ServiceAccount."#
            .to_string()
    }

    fn explain_endpoints(_recursive: bool) -> String {
        r#"KIND
====
Endpoints

DESCRIPTION
==========
Endpoints is a collection of endpoints that implement a service.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

subsets      <[]EndpointSubset>
  Set of endpoints for a service."#
            .to_string()
    }

    fn explain_endpointslices(_recursive: bool) -> String {
        r#"KIND
====
EndpointSlice

DESCRIPTION
==========
EndpointSlice represents a subset of the endpoints for a service.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

addressType  <string>
  Type of address (IPv4, IPv6, or FQDN).

endpoints    <[]Endpoint>
  Endpoints in this slice.

ports        <[]EndpointPort>
  Ports for the endpoints."#
            .to_string()
    }

    fn explain_event(_recursive: bool) -> String {
        r#"KIND
====
Event

DESCRIPTION
==========
Event is a report of an event.

FIELDS
======
apiVersion   <string>
  APIVersion defines the versioned schema of this representation.

kind         <string>
  Kind is a string value representing the REST resource.

metadata     <Object>
  Standard object's metadata.

reason       <string>
  Reason for this event.

message      <string>
  Human readable description of this event.

type         <string>
  Type of this event (Normal, Warning)."#
            .to_string()
    }
}

#[async_trait]
impl Command for ExplainCommand {
    async fn run(&self) -> Result<()> {
        if let Some(ref resource) = self.resource {
            println!("{}", Self::explain_resource(resource, self.recursive));
        } else {
            println!("{}", Self::explain_all());
        }
        Ok(())
    }
}
