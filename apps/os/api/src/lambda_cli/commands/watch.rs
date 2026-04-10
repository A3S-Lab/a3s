//! `a3s watch` command - Watch for resource changes in real-time.
//!
//! Provides real-time monitoring of resource changes, similar to `kubectl watch`.

use crate::commands::Command;
use crate::errors::Result;
use async_trait::async_trait;
use clap::Parser;
use std::collections::HashMap;
use std::path::PathBuf;
use tokio::time::{interval, Duration};

/// Watch command - monitor resource changes in real-time.
#[derive(Parser, Debug)]
pub struct WatchCommand {
    /// Resource type to watch.
    #[arg(default_value = "pods")]
    resource: WatchResource,

    /// Polling interval in seconds.
    #[arg(short, long, default_value = "5")]
    interval: u64,

    /// Show only changes (not initial state).
    #[arg(short, long)]
    changes_only: bool,

    /// Namespace to watch (for namespaced resources).
    #[arg(short, long, default_value = "default")]
    namespace: String,
}

/// Resource types that can be watched.
#[derive(clap::ValueEnum, Clone, Debug)]
pub enum WatchResource {
    /// Watch pods.
    Pods,
    /// Watch services.
    Services,
    /// Watch deployments.
    Deployments,
    /// Watch configmaps.
    ConfigMaps,
    /// Watch secrets.
    Secrets,
    /// Watch namespaces.
    Namespaces,
    /// Watch nodes.
    Nodes,
    /// Watch events.
    Events,
    /// Watch ingresses.
    Ingresses,
    /// Watch statefulsets.
    StatefulSets,
    /// Watch daemonsets.
    DaemonSets,
    /// Watch jobs.
    Jobs,
    /// Watch cronjobs.
    CronJobs,
    /// Watch networkpolicies.
    NetworkPolicies,
    /// Watch all resources.
    All,
    /// Watch poddisruptionbudgets.
    PodDisruptionBudgets,
    /// Watch storageclasses.
    StorageClasses,
    /// Watch roles.
    Roles,
    /// Watch clusterroles.
    ClusterRoles,
    /// Watch rolebindings.
    RoleBindings,
    /// Watch clusterrolebindings.
    ClusterRoleBindings,
    /// Watch endpointslices.
    EndpointSlices,
}

impl WatchCommand {
    /// Run the watch loop.
    async fn run_watch(&self) -> Result<()> {
        let poll_interval = Duration::from_secs(self.interval);

        match self.resource {
            WatchResource::Pods => self.watch_pods(poll_interval).await,
            WatchResource::Services => self.watch_services(poll_interval).await,
            WatchResource::Deployments => self.watch_deployments(poll_interval).await,
            WatchResource::ConfigMaps => self.watch_configmaps(poll_interval).await,
            WatchResource::Secrets => self.watch_secrets(poll_interval).await,
            WatchResource::Namespaces => self.watch_namespaces(poll_interval).await,
            WatchResource::Nodes => self.watch_nodes(poll_interval).await,
            WatchResource::Events => self.watch_events(poll_interval).await,
            WatchResource::Ingresses => self.watch_ingresses(poll_interval).await,
            WatchResource::StatefulSets => self.watch_statefulsets(poll_interval).await,
            WatchResource::DaemonSets => self.watch_daemonsets(poll_interval).await,
            WatchResource::Jobs => self.watch_jobs(poll_interval).await,
            WatchResource::CronJobs => self.watch_cronjobs(poll_interval).await,
            WatchResource::NetworkPolicies => self.watch_networkpolicies(poll_interval).await,
            WatchResource::All => self.watch_all(poll_interval).await,
            WatchResource::PodDisruptionBudgets => {
                self.watch_poddisruptionbudgets(poll_interval).await
            }
            WatchResource::StorageClasses => self.watch_storageclasses(poll_interval).await,
            WatchResource::Roles => self.watch_roles(poll_interval).await,
            WatchResource::ClusterRoles => self.watch_cluster_roles(poll_interval).await,
            WatchResource::RoleBindings => self.watch_role_bindings(poll_interval).await,
            WatchResource::ClusterRoleBindings => {
                self.watch_cluster_role_bindings(poll_interval).await
            }
            WatchResource::EndpointSlices => self.watch_endpoints(poll_interval).await,
        }
    }

    /// Watch pods for changes.
    async fn watch_pods(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching pods... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_pods: HashMap<String, PodSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let pods = store.load_pods().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Pods:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (id, pod) in &pods {
                        let snapshot = PodSnapshot {
                            status: format!("{:?}", pod.status),
                            health: format!("{:?}", pod.health),
                            version: pod.version.clone(),
                        };

                        let last = last_pods.get(id);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!(
                                "  {} | {} | {} | {}",
                                id.chars().take(12).collect::<String>(),
                                snapshot.status,
                                snapshot.health,
                                snapshot.version
                            );
                        }
                    }

                    // Check for deletions
                    for id in last_pods.keys() {
                        if !pods.contains_key(id) {
                            _changed = true;
                            println!("  - {} deleted", id.chars().take(12).collect::<String>());
                        }
                    }

                    if _changed && self.changes_only {
                        // Do nothing extra - we already printed changes
                    }

                    // Update last seen
                    last_pods = pods
                        .into_iter()
                        .map(|(id, pod)| {
                            (
                                id,
                                PodSnapshot {
                                    status: format!("{:?}", pod.status),
                                    health: format!("{:?}", pod.health),
                                    version: pod.version,
                                },
                            )
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch services for changes.
    async fn watch_services(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching services... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_services: HashMap<String, ServiceSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let services = store.load_services().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Services:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, svc) in &services {
                        let snapshot = ServiceSnapshot {
                            service_type: format!("{:?}", svc.service_type),
                            port_count: svc.ports.len(),
                        };

                        let last = last_services.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            let ports: Vec<_> = svc.ports.iter().map(|p| format!("{}:{}", p.name, p.port)).collect();
                            println!(
                                "  {} | {:?} | ports: {}",
                                name,
                                svc.service_type,
                                ports.join(", ")
                            );
                        }
                    }

                    for name in last_services.keys() {
                        if !services.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_services = services
                        .into_iter()
                        .map(|(name, svc)| {
                            (
                                name,
                                ServiceSnapshot {
                                    service_type: format!("{:?}", svc.service_type),
                                    port_count: svc.ports.len(),
                                },
                            )
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch deployments for changes.
    async fn watch_deployments(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching deployments... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_deployments: HashMap<String, DeploymentSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let deployments = store.load_deployments().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Deployments:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, dep) in &deployments {
                        let snapshot = DeploymentSnapshot {
                            replicas: dep.replicas,
                            image: dep.image.clone(),
                        };

                        let last = last_deployments.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!(
                                "  {} | {} replicas | {}",
                                name,
                                snapshot.replicas,
                                snapshot.image
                            );
                        }
                    }

                    for name in last_deployments.keys() {
                        if !deployments.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_deployments = deployments
                        .into_iter()
                        .map(|(name, dep)| {
                            (
                                name,
                                DeploymentSnapshot {
                                    replicas: dep.replicas,
                                    image: dep.image,
                                },
                            )
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch configmaps for changes.
    async fn watch_configmaps(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching configmaps... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_configmaps: HashMap<String, ConfigMapSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let configmaps = store.load_configmaps().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] ConfigMaps:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, cm) in &configmaps {
                        let key = format!("{}/{}", cm.namespace, name);
                        let snapshot = ConfigMapSnapshot {
                            data_keys: cm.data.len(),
                        };

                        let last = last_configmaps.get(&key);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {} ({}) | {} keys", name, cm.namespace, snapshot.data_keys);
                        }
                    }

                    for key in last_configmaps.keys() {
                        if !configmaps.iter().any(|(_, cm)| format!("{}/{}", cm.namespace, cm.name) == *key) {
                            _changed = true;
                            println!("  - {} deleted", key);
                        }
                    }

                    last_configmaps = configmaps
                        .into_iter()
                        .map(|(name, cm)| {
                            let key = format!("{}/{}", cm.namespace, name);
                            (key, ConfigMapSnapshot { data_keys: cm.data.len() })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch secrets for changes.
    async fn watch_secrets(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching secrets... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_secrets: HashMap<String, SecretSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let secrets = store.load_secrets().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Secrets:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, secret) in &secrets {
                        let key = format!("{}/{}", secret.namespace, name);
                        let snapshot = SecretSnapshot {
                            data_keys: secret.data.len(),
                        };

                        let last = last_secrets.get(&key);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {} ({}) | {} keys", name, secret.namespace, snapshot.data_keys);
                        }
                    }

                    for key in last_secrets.keys() {
                        if !secrets.iter().any(|(_, s)| format!("{}/{}", s.namespace, s.name) == *key) {
                            _changed = true;
                            println!("  - {} deleted", key);
                        }
                    }

                    last_secrets = secrets
                        .into_iter()
                        .map(|(name, secret)| {
                            let key = format!("{}/{}", secret.namespace, name);
                            (key, SecretSnapshot { data_keys: secret.data.len() })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch namespaces for changes.
    async fn watch_namespaces(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching namespaces... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_namespaces: HashMap<String, NamespaceSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let namespaces = store.load_namespaces().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Namespaces:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, _ns) in &namespaces {
                        let snapshot = NamespaceSnapshot {
                            name: name.clone(),
                        };

                        let last = last_namespaces.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {}", name);
                        }
                    }

                    for name in last_namespaces.keys() {
                        if !namespaces.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_namespaces = namespaces
                        .into_iter()
                        .map(|(name, _ns)| {
                            let ns_name = name.clone();
                            (
                                name,
                                NamespaceSnapshot {
                                    name: ns_name,
                                },
                            )
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch nodes for changes.
    async fn watch_nodes(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching nodes... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_nodes: HashMap<String, NodeSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let nodes = store.load_nodes().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Nodes:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, _node) in &nodes {
                        let snapshot = NodeSnapshot {
                            status: "Ready".to_string(),
                        };

                        let last = last_nodes.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {} | {}", name, snapshot.status);
                        }
                    }

                    for name in last_nodes.keys() {
                        if !nodes.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_nodes = nodes
                        .into_iter()
                        .map(|(name, _node)| {
                            (
                                name,
                                NodeSnapshot {
                                    status: "Ready".to_string(),
                                },
                            )
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch events for changes.
    async fn watch_events(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching events... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_events: Vec<String> = Vec::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let events = store.load_events().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Events:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let event_ids: Vec<String> = events.iter().map(|e| format!("{:?}:{}", e.reason, e.last_timestamp)).collect();

                    // Show new events
                    for event in events.iter().rev().take(10) {
                        println!(
                            "  {:<20} {:<15} {:<30}",
                            format!("{:?}", event.reason),
                            event.involved_object.namespace.as_deref().unwrap_or("<none>"),
                            event.involved_object.name
                        );
                    }

                    last_events = event_ids;
                }
            }
        }
    }

    /// Watch ingresses for changes.
    async fn watch_ingresses(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching ingresses... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_ingresses: HashMap<String, IngressSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let ingresses = store.load_ingresses().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Ingresses:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_ings) in &ingresses {
                        for (name, ing) in ns_ings {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = IngressSnapshot {
                                rules_count: ing.spec.rules.len(),
                            };

                            let last = last_ingresses.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!("  {} ({}) | {} rules", name, ns, snapshot.rules_count);
                            }
                        }
                    }

                    last_ingresses = ingresses
                        .into_iter()
                        .flat_map(|(ns, ns_ings)| {
                            ns_ings.into_iter().map(move |(name, ing)| {
                                let key = format!("{}/{}", ns, name);
                                (key, IngressSnapshot { rules_count: ing.spec.rules.len() })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch statefulsets for changes.
    async fn watch_statefulsets(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching statefulsets... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_statefulsets: HashMap<String, StatefulSetSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let statefulsets = store.load_statefulsets().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] StatefulSets:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_sss) in &statefulsets {
                        for (name, ss) in ns_sss {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = StatefulSetSnapshot {
                                replicas: ss.spec.replicas,
                            };

                            let last = last_statefulsets.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!("  {} ({}) | {} replicas", name, ns, snapshot.replicas);
                            }
                        }
                    }

                    last_statefulsets = statefulsets
                        .into_iter()
                        .flat_map(|(ns, ns_sss)| {
                            ns_sss.into_iter().map(move |(name, ss)| {
                                let key = format!("{}/{}", ns, name);
                                (key, StatefulSetSnapshot { replicas: ss.spec.replicas })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch daemonsets for changes.
    async fn watch_daemonsets(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching daemonsets... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_daemonsets: HashMap<String, DaemonSetSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let daemonsets = store.load_daemonsets().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] DaemonSets:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, ds) in &daemonsets {
                        let key = format!("{}/{}", ds.namespace, name);
                        let snapshot = DaemonSetSnapshot {
                            desired: ds.spec.replicas,
                        };

                        let last = last_daemonsets.get(&key);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {} ({}) | {} desired", name, ds.namespace, snapshot.desired);
                        }
                    }

                    last_daemonsets = daemonsets
                        .into_iter()
                        .map(|(name, ds)| {
                            let key = format!("{}/{}", ds.namespace, name);
                            (key, DaemonSetSnapshot { desired: ds.spec.replicas })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch jobs for changes.
    async fn watch_jobs(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching jobs... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_jobs: HashMap<String, JobSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let jobs = store.load_jobs().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Jobs:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_jobs) in &jobs {
                        for (name, job) in ns_jobs {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = JobSnapshot {
                                parallelism: job.spec.parallelism.unwrap_or(1),
                            };

                            let last = last_jobs.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!("  {} ({}) | parallelism: {}", name, ns, snapshot.parallelism);
                            }
                        }
                    }

                    last_jobs = jobs
                        .into_iter()
                        .flat_map(|(ns, ns_jobs)| {
                            ns_jobs.into_iter().map(move |(name, job)| {
                                let key = format!("{}/{}", ns, name);
                                (key, JobSnapshot { parallelism: job.spec.parallelism.unwrap_or(1) })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch cronjobs for changes.
    async fn watch_cronjobs(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching cronjobs... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_cronjobs: HashMap<String, CronJobSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let cronjobs = store.load_cronjobs().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] CronJobs:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_cjs) in &cronjobs {
                        for (name, cj) in ns_cjs {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = CronJobSnapshot {
                                schedule: cj.spec.schedule.format(),
                                suspend: cj.spec.suspend,
                            };

                            let last = last_cronjobs.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!("  {} ({}) | {} | suspend: {}", name, ns, snapshot.schedule, snapshot.suspend);
                            }
                        }
                    }

                    last_cronjobs = cronjobs
                        .into_iter()
                        .flat_map(|(ns, ns_cjs)| {
                            ns_cjs.into_iter().map(move |(name, cj)| {
                                let key = format!("{}/{}", ns, name);
                                (key, CronJobSnapshot { schedule: cj.spec.schedule.format(), suspend: cj.spec.suspend })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch networkpolicies for changes.
    async fn watch_networkpolicies(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching networkpolicies... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_networkpolicies: HashMap<String, NetworkPolicySnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let networkpolicies = store.load_network_policies().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] NetworkPolicies:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for np in &networkpolicies {
                        let key = format!("{}/{}", np.namespace, np.name);
                        let snapshot = NetworkPolicySnapshot {
                            policy_types: np.spec.policy_types.len(),
                        };

                        let last = last_networkpolicies.get(&key);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {} ({}) | {} policy types", np.name, np.namespace, snapshot.policy_types);
                        }
                    }

                    last_networkpolicies = networkpolicies
                        .into_iter()
                        .map(|np| {
                            let key = format!("{}/{}", np.namespace, np.name);
                            (key, NetworkPolicySnapshot { policy_types: np.spec.policy_types.len() })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch poddisruptionbudgets for changes.
    async fn watch_poddisruptionbudgets(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching poddisruptionbudgets... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_pdbs: HashMap<String, PDBSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let pdbs = store.load_pod_disruption_budgets().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] PodDisruptionBudgets:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_pdbs) in &pdbs {
                        for (name, pdb) in ns_pdbs {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = PDBSnapshot {
                                min_available: pdb.spec.min_available.clone(),
                                max_disruptions: pdb.spec.max_disruptions.clone(),
                            };

                            let last = last_pdbs.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!(
                                    "  {} ({}) | min: {:?} | max: {:?}",
                                    name, ns, snapshot.min_available, snapshot.max_disruptions
                                );
                            }
                        }
                    }

                    for key in last_pdbs.keys() {
                        if !pdbs.iter().any(|(_ns, ns_pdbs)| ns_pdbs.contains_key(key.split('/').nth(1).unwrap_or(""))) {
                            _changed = true;
                            println!("  - {} deleted", key);
                        }
                    }

                    last_pdbs = pdbs
                        .into_iter()
                        .flat_map(|(ns, ns_pdbs)| {
                            ns_pdbs.into_iter().map(move |(name, pdb)| {
                                let key = format!("{}/{}", ns, name);
                                (key, PDBSnapshot {
                                    min_available: pdb.spec.min_available.clone(),
                                    max_disruptions: pdb.spec.max_disruptions.clone(),
                                })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch storageclasses for changes.
    async fn watch_storageclasses(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching storageclasses... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_scs: HashMap<String, StorageClassSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let scs = store.load_storage_classes().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] StorageClasses:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, sc) in &scs {
                        let snapshot = StorageClassSnapshot {
                            provisioner: sc.provisioner.clone(),
                            is_default: sc.is_default,
                        };

                        let last = last_scs.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!(
                                "  {} | provisioner: {} | default: {}",
                                name, snapshot.provisioner, snapshot.is_default
                            );
                        }
                    }

                    for name in last_scs.keys() {
                        if !scs.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_scs = scs
                        .into_iter()
                        .map(|(name, sc)| {
                            (name, StorageClassSnapshot {
                                provisioner: sc.provisioner,
                                is_default: sc.is_default,
                            })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch roles for changes.
    async fn watch_roles(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching roles... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_roles: HashMap<String, RoleSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let roles = store.load_roles().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] Roles:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_roles) in &roles {
                        for (name, role) in ns_roles {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = RoleSnapshot {
                                rules_count: role.rules.len(),
                            };

                            let last = last_roles.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!("  {} ({}) | {} rules", name, ns, snapshot.rules_count);
                            }
                        }
                    }

                    for key in last_roles.keys() {
                        if !roles.iter().any(|(_ns, ns_roles)| ns_roles.contains_key(key.split('/').nth(1).unwrap_or(""))) {
                            _changed = true;
                            println!("  - {} deleted", key);
                        }
                    }

                    last_roles = roles
                        .into_iter()
                        .flat_map(|(ns, ns_roles)| {
                            ns_roles.into_iter().map(move |(name, role)| {
                                let key = format!("{}/{}", ns, name);
                                (key, RoleSnapshot { rules_count: role.rules.len() })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch clusterroles for changes.
    async fn watch_cluster_roles(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching clusterroles... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_cluster_roles: HashMap<String, ClusterRoleSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let cluster_roles = store.load_cluster_roles().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] ClusterRoles:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, cr) in &cluster_roles {
                        let snapshot = ClusterRoleSnapshot {
                            rules_count: cr.rules.len(),
                        };

                        let last = last_cluster_roles.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!("  {} | {} rules", name, snapshot.rules_count);
                        }
                    }

                    for name in last_cluster_roles.keys() {
                        if !cluster_roles.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_cluster_roles = cluster_roles
                        .into_iter()
                        .map(|(name, cr)| {
                            (name, ClusterRoleSnapshot { rules_count: cr.rules.len() })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch rolebindings for changes.
    async fn watch_role_bindings(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching rolebindings... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_role_bindings: HashMap<String, RoleBindingSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let role_bindings = store.load_role_bindings().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] RoleBindings:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (ns, ns_bindings) in &role_bindings {
                        for (name, rb) in ns_bindings {
                            let key = format!("{}/{}", ns, name);
                            let snapshot = RoleBindingSnapshot {
                                role_ref_kind: rb.role_ref.kind.clone(),
                                role_ref_name: rb.role_ref.name.clone(),
                                subjects_count: rb.subjects.len(),
                            };

                            let last = last_role_bindings.get(&key);
                            let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                            if !last_matches {
                                _changed = true;
                                println!(
                                    "  {} ({}) | {} {} | {} subjects",
                                    name, ns, snapshot.role_ref_kind, snapshot.role_ref_name, snapshot.subjects_count
                                );
                            }
                        }
                    }

                    for key in last_role_bindings.keys() {
                        if !role_bindings.iter().any(|(_ns, ns_bindings)| ns_bindings.contains_key(key.split('/').nth(1).unwrap_or(""))) {
                            _changed = true;
                            println!("  - {} deleted", key);
                        }
                    }

                    last_role_bindings = role_bindings
                        .into_iter()
                        .flat_map(|(ns, ns_bindings)| {
                            ns_bindings.into_iter().map(move |(name, rb)| {
                                let key = format!("{}/{}", ns, name);
                                (key, RoleBindingSnapshot {
                                    role_ref_kind: rb.role_ref.kind.clone(),
                                    role_ref_name: rb.role_ref.name.clone(),
                                    subjects_count: rb.subjects.len(),
                                })
                            }).collect::<Vec<_>>()
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch clusterrolebindings for changes.
    async fn watch_cluster_role_bindings(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching clusterrolebindings... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));

        let store = crate::state::StateStore::new(&PathBuf::from("."));
        let mut ticker = interval(poll_interval);
        let mut last_crb: HashMap<String, ClusterRoleBindingSnapshot> = HashMap::new();

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let cluster_role_bindings = store.load_cluster_role_bindings().unwrap_or_default();

                    if !self.changes_only {
                        println!("\n[{:?}] ClusterRoleBindings:", chrono::Utc::now().format("%H:%M:%S"));
                    }

                    let mut _changed = false;
                    for (name, crb) in &cluster_role_bindings {
                        let snapshot = ClusterRoleBindingSnapshot {
                            role_ref_kind: crb.role_ref.kind.clone(),
                            role_ref_name: crb.role_ref.name.clone(),
                            subjects_count: crb.subjects.len(),
                        };

                        let last = last_crb.get(name);
                        let last_matches = last.map(|s| s == &snapshot).unwrap_or(false);
                        if !last_matches {
                            _changed = true;
                            println!(
                                "  {} | {} {} | {} subjects",
                                name, snapshot.role_ref_kind, snapshot.role_ref_name, snapshot.subjects_count
                            );
                        }
                    }

                    for name in last_crb.keys() {
                        if !cluster_role_bindings.contains_key(name) {
                            _changed = true;
                            println!("  - {} deleted", name);
                        }
                    }

                    last_crb = cluster_role_bindings
                        .into_iter()
                        .map(|(name, crb)| {
                            (name, ClusterRoleBindingSnapshot {
                                role_ref_kind: crb.role_ref.kind.clone(),
                                role_ref_name: crb.role_ref.name.clone(),
                                subjects_count: crb.subjects.len(),
                            })
                        })
                        .collect();
                }
            }
        }
    }

    /// Watch endpoint slices for changes.
    async fn watch_endpoints(&self, _poll_interval: Duration) -> Result<()> {
        println!("Watching endpoints... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));
        println!("Note: EndpointSlice data is derived from services");
        println!("Use 'a3s watch services' to monitor service endpoints");
        Ok(())
    }

    /// Watch all resources.
    async fn watch_all(&self, poll_interval: Duration) -> Result<()> {
        println!("Watching all resources... (Ctrl+C to stop)");
        println!("{}", "=".repeat(80));
        println!("Use 'a3s watch pods' or 'a3s watch services' for focused watching");
        println!();

        // Just poll everything periodically
        let mut ticker = interval(poll_interval);
        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    let now = chrono::Utc::now().format("%H:%M:%S");

                    // Pods
                    let store = crate::state::StateStore::new(&PathBuf::from("."));
                    if let Ok(pods) = store.load_pods() {
                        let running = pods.values().filter(|p| p.status == crate::state::PodStatus::Running).count();
                        let unhealthy = pods.values().filter(|p| p.health == crate::state::HealthStatus::Unhealthy).count();
                        println!("[{}] Pods: {} running, {} unhealthy", now, running, unhealthy);
                    }

                    // Services
                    if let Ok(services) = store.load_services() {
                        println!("[{}] Services: {} defined", now, services.len());
                    }

                    // Deployments
                    if let Ok(deployments) = store.load_deployments() {
                        println!("[{}] Deployments: {} defined", now, deployments.len());
                    }

                    // ConfigMaps
                    if let Ok(configmaps) = store.load_configmaps() {
                        println!("[{}] ConfigMaps: {} defined", now, configmaps.len());
                    }

                    // Secrets
                    if let Ok(secrets) = store.load_secrets() {
                        println!("[{}] Secrets: {} defined", now, secrets.len());
                    }

                    // Namespaces
                    if let Ok(namespaces) = store.load_namespaces() {
                        println!("[{}] Namespaces: {} defined", now, namespaces.len());
                    }

                    // Events
                    if let Ok(events) = store.load_events() {
                        println!("[{}] Events: {} recorded", now, events.len());
                    }

                    println!();
                }
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
struct PodSnapshot {
    status: String,
    health: String,
    version: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ServiceSnapshot {
    service_type: String,
    port_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct DeploymentSnapshot {
    replicas: i32,
    image: String,
}

#[derive(Debug, Clone, PartialEq)]
struct ConfigMapSnapshot {
    data_keys: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct SecretSnapshot {
    data_keys: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct NamespaceSnapshot {
    name: String,
}

#[derive(Debug, Clone, PartialEq)]
struct NodeSnapshot {
    status: String,
}

#[derive(Debug, Clone, PartialEq)]
struct IngressSnapshot {
    rules_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct StatefulSetSnapshot {
    replicas: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct DaemonSetSnapshot {
    desired: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct JobSnapshot {
    parallelism: i32,
}

#[derive(Debug, Clone, PartialEq)]
struct CronJobSnapshot {
    schedule: String,
    suspend: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct NetworkPolicySnapshot {
    policy_types: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct PDBSnapshot {
    min_available: Option<String>,
    max_disruptions: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
struct StorageClassSnapshot {
    provisioner: String,
    is_default: bool,
}

#[derive(Debug, Clone, PartialEq)]
struct RoleSnapshot {
    rules_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct ClusterRoleSnapshot {
    rules_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct RoleBindingSnapshot {
    role_ref_kind: String,
    role_ref_name: String,
    subjects_count: usize,
}

#[derive(Debug, Clone, PartialEq)]
struct ClusterRoleBindingSnapshot {
    role_ref_kind: String,
    role_ref_name: String,
    subjects_count: usize,
}

#[async_trait]
impl Command for WatchCommand {
    async fn run(&self) -> Result<()> {
        self.run_watch().await
    }
}
