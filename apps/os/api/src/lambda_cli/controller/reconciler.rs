//! Reconciliation controller.
//!
//! The core control loop that ensures actual state matches desired state.
//! Runs periodically and triggers corrections when drift is detected.

use crate::controller::Scheduler;
use crate::deployment::microvm::MicrovmProvider;
use crate::deployment::provider::DeploymentProvider;
use crate::errors::Result;
use crate::state::{
    HealthStatus, PodActual, PodStatus, RollingUpdateConfig, RollingUpdateState, StateStore,
    UpdateStrategy,
};
use chrono::Utc;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::time::{interval, Duration};

/// Reconciliation controller.
///
/// Runs a continuous loop that:
///
/// 1. Loads desired state from StateStore
/// 2. Loads actual state from StateStore
/// 3. Computes diff
/// 4. Triggers corrections via ReplicaController
/// 5. Sleeps and repeats
pub struct Reconciler {
    /// State store.
    store: Arc<StateStore>,
    /// Replica controller.
    replica: Arc<ReplicaController>,
    /// Health monitor.
    health: Arc<HealthMonitor>,
    /// Stop signal.
    stop_tx: tokio::sync::watch::Sender<()>,
}

impl Reconciler {
    /// Create a new reconciler.
    pub fn new(working_dir: &PathBuf) -> Self {
        let store = Arc::new(StateStore::new(working_dir));
        let scheduler = Arc::new(Scheduler::new(store.clone()));
        let replica = Arc::new(ReplicaController::new(store.clone(), scheduler.clone()));
        let health = Arc::new(HealthMonitor::new(store.clone()));
        let (stop_tx, _stop_rx) = tokio::sync::watch::channel(());

        Self {
            store,
            replica,
            health,
            stop_tx,
        }
    }

    /// Start the reconciliation loop.
    /// This runs forever until stop() is called.
    pub async fn run(&self) {
        tracing::info!("Reconciler starting");

        let mut ticker = interval(Duration::from_secs(10));

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = self.reconcile().await {
                        tracing::error!(error = %e, "Reconciliation failed");
                    }
                }
            }
        }
    }

    /// Run one reconciliation tick.
    async fn reconcile(&self) -> Result<()> {
        tracing::debug!("Running reconciliation tick");

        // Load desired deployments
        let deployments = self.store.load_deployments()?;

        for (name, deployment) in deployments {
            // Get replica diff
            let diff = self.store.get_replica_diff(&name)?;

            tracing::debug!(
                deployment = %name,
                desired = diff.desired_replicas,
                actual = diff.actual_replicas,
                running = diff.running_replicas,
                unhealthy = diff.unhealthy_replicas,
                "replica diff"
            );

            // Check for version change (rolling update)
            if let Some(rolling_state) = self.store.get_rolling_update(&name)? {
                self.rollout_update(&name, &deployment, rolling_state)
                    .await?;
                continue;
            }

            // Check if version mismatch exists (new rolling update needed)
            let pods = self.store.get_pods_by_deployment(&name)?;
            let outdated_pods: Vec<&PodActual> = pods
                .iter()
                .filter(|p| p.version != deployment.version && p.status == PodStatus::Running)
                .collect();

            if !outdated_pods.is_empty() {
                match &deployment.strategy {
                    UpdateStrategy::RollingUpdate(config) => {
                        self.start_rolling_update(&name, &deployment, config, outdated_pods.len())
                            .await?;
                    }
                    UpdateStrategy::Replace => {
                        // Replace all at once
                        tracing::info!(
                            deployment = %name,
                            "Replace strategy: replacing all outdated pods"
                        );
                        for pod in outdated_pods {
                            if let Err(e) =
                                self.replica.replace_pod(&pod.id, &name, &deployment).await
                            {
                                tracing::error!(error = %e, pod_id = %pod.id, "failed to replace pod");
                            }
                        }
                    }
                }
                continue;
            }

            // Create missing replicas
            if diff.need_create > 0 {
                tracing::info!(
                    deployment = %name,
                    count = diff.need_create,
                    "creating missing replicas"
                );
                for i in 0..diff.need_create {
                    if let Err(e) = self.replica.create_pod(&name, &deployment).await {
                        tracing::error!(
                            error = %e,
                            index = i,
                            "failed to create pod"
                        );
                    }
                }
            }

            // Delete extra replicas
            if diff.need_delete > 0 {
                tracing::info!(
                    deployment = %name,
                    count = diff.need_delete,
                    "deleting extra replicas"
                );
                let pods = self.store.get_pods_by_deployment(&name)?;
                for pod in pods.into_iter().take(diff.need_delete as usize) {
                    if let Err(e) = self.replica.delete_pod(&pod.id).await {
                        tracing::error!(
                            error = %e,
                            pod_id = %pod.id,
                            "failed to delete pod"
                        );
                    }
                }
            }

            // Replace unhealthy replicas
            let unhealthy_count = diff.unhealthy_replicas;
            if unhealthy_count > 0 {
                tracing::warn!(
                    deployment = %name,
                    count = unhealthy_count,
                    "replacing unhealthy replicas"
                );
                let pods = self.store.get_pods_by_deployment(&name)?;
                for pod in pods
                    .into_iter()
                    .filter(|p| p.health == HealthStatus::Unhealthy)
                {
                    if let Err(e) = self.replica.replace_pod(&pod.id, &name, &deployment).await {
                        tracing::error!(
                            error = %e,
                            pod_id = %pod.id,
                            "failed to replace unhealthy pod"
                        );
                    }
                }
            }
        }

        // Run health checks
        let pods = self.store.load_pods()?;
        for (_id, pod) in pods.iter().filter(|(_, p)| p.status == PodStatus::Running) {
            if pod.health != HealthStatus::Passthrough {
                let _ = self.health.check_pod(pod).await;
            }
        }

        Ok(())
    }

    /// Start a new rolling update.
    async fn start_rolling_update(
        &self,
        name: &str,
        deployment: &crate::state::DeploymentDesired,
        _config: &RollingUpdateConfig,
        outdated_count: usize,
    ) -> Result<()> {
        let pods = self.store.get_pods_by_deployment(name)?;
        let running_count = pods
            .iter()
            .filter(|p| p.status == PodStatus::Running)
            .count() as i32;

        // Get from_version from the first outdated pod
        let from_version = pods
            .iter()
            .find(|p| p.version != deployment.version)
            .map(|p| p.version.clone())
            .unwrap_or_else(|| deployment.version.clone());

        let rolling_state = RollingUpdateState {
            deployment: name.to_string(),
            target_version: deployment.version.clone(),
            from_version,
            updated_replicas: running_count - outdated_count as i32,
            old_replicas: outdated_count as i32,
            pending_replicas: outdated_count as i32,
            rollback: false,
        };

        tracing::info!(
            deployment = %name,
            from = %rolling_state.from_version,
            to = %rolling_state.target_version,
            outdated = outdated_count,
            "starting rolling update"
        );

        self.store.set_rolling_update(rolling_state.clone())?;
        self.rollout_update(name, deployment, rolling_state).await
    }

    /// Execute one step of rolling update.
    async fn rollout_update(
        &self,
        name: &str,
        deployment: &crate::state::DeploymentDesired,
        mut state: RollingUpdateState,
    ) -> Result<()> {
        let config = match &deployment.strategy {
            UpdateStrategy::RollingUpdate(c) => c,
            UpdateStrategy::Replace => {
                // Clean up rolling state if replace strategy
                self.store.delete_rolling_update(name)?;
                return Ok(());
            }
        };

        // Get current pods
        let pods = self.store.get_pods_by_deployment(name)?;
        let running_count = pods
            .iter()
            .filter(|p| p.status == PodStatus::Running)
            .count() as i32;

        // Calculate max unavailable and max surge
        let _max_unavailable = config.max_unavailable.resolve(deployment.replicas);
        let max_surge = config.max_surge.resolve(deployment.replicas);

        // Calculate how many pods we can update right now
        // Available slots = desired + max_surge - current_running
        let available_slots = (deployment.replicas + max_surge) - running_count;

        // We can update min(pending, available_slots, max_surge) pods
        let can_update = available_slots.min(max_surge).max(0);

        if can_update > 0 && state.pending_replicas > 0 {
            let update_count = can_update.min(state.pending_replicas);

            tracing::info!(
                deployment = %name,
                updating = update_count,
                pending = state.pending_replicas,
                available_slots = available_slots,
                "rolling update: creating new pods"
            );

            // Create new pods with new version
            for _ in 0..update_count {
                let new_pod = self.replica.create_pod(name, deployment).await;
                match new_pod {
                    Ok(_) => {
                        state.updated_replicas += 1;
                        state.pending_replicas -= 1;
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "failed to create pod during rolling update");
                        if config.rollback_on_failure && !state.rollback {
                            state.rollback = true;
                            tracing::warn!(deployment = %name, "rollback initiated due to pod creation failure");
                        }
                    }
                }
            }

            self.store.set_rolling_update(state.clone())?;
        }

        // If rollback is active, delete pods with old version
        if state.rollback {
            let old_pods: Vec<String> = pods
                .iter()
                .filter(|p| p.version == state.from_version && p.status == PodStatus::Running)
                .map(|p| p.id.clone())
                .collect();

            for pod_id in old_pods {
                tracing::info!(deployment = %name, pod_id = %pod_id, "rollback: deleting old pod");
                if let Err(e) = self.replica.delete_pod(&pod_id).await {
                    tracing::error!(error = %e, pod_id = %pod_id, "failed to delete pod during rollback");
                }
                state.old_replicas -= 1;
            }

            self.store.set_rolling_update(state.clone())?;
        }

        // Check if rolling update is complete
        if state.pending_replicas == 0 && state.old_replicas == 0 {
            tracing::info!(
                deployment = %name,
                "rolling update complete"
            );
            self.store.delete_rolling_update(name)?;
        }

        Ok(())
    }

    /// Stop the reconciler.
    pub fn stop(&self) {
        let _ = self.stop_tx.send(());
    }
}

/// Replica controller - manages pod lifecycle.
pub struct ReplicaController {
    store: Arc<StateStore>,
    scheduler: Arc<Scheduler>,
}

impl ReplicaController {
    pub fn new(store: Arc<StateStore>, scheduler: Arc<Scheduler>) -> Self {
        Self { store, scheduler }
    }

    /// Create a new pod for a deployment.
    pub async fn create_pod(
        &self,
        deployment_name: &str,
        deployment: &crate::state::DeploymentDesired,
    ) -> Result<PodActual> {
        use crate::deployment::microvm::{MicrovmProvider, PortForward, SandboxOptions};

        // Generate pod ID
        let pod_id = format!(
            "{}-{}-{}",
            deployment_name,
            &deployment.version,
            uuid::Uuid::new_v4().to_string()[..8].to_string()
        );

        tracing::info!(
            deployment = %deployment_name,
            pod_id = %pod_id,
            "creating pod"
        );

        // Schedule pod to a node
        let existing_pods = self.store.get_pods_by_deployment(deployment_name)?;
        let node_name = match self.scheduler.schedule(deployment, &existing_pods).await {
            Ok(Some(name)) => {
                tracing::debug!(node = %name, "scheduled pod to node");
                Some(name)
            }
            Ok(None) => {
                tracing::warn!("no eligible node found for scheduling, pod will be unscheduled");
                None
            }
            Err(e) => {
                tracing::error!(error = %e, "scheduling failed, pod will be unscheduled");
                None
            }
        };

        // Build sandbox options
        let mut opts = SandboxOptions {
            image: deployment.image.clone(),
            cpus: 1,
            memory_mb: parse_memory(&deployment.resources.memory_limit),
            name: Some(format!("a3s-{}-{}", deployment_name, &pod_id[..8])),
            network: true,
            ..Default::default()
        };

        // Add environment variables
        for (k, v) in &deployment.env {
            opts.env.insert(k.clone(), v.clone());
        }

        // Add port forwards
        for port in &deployment.ports {
            opts.port_forwards.push(PortForward {
                guest_port: port.container_port,
                host_port: port.host_port,
                protocol: port.protocol.clone(),
            });
        }

        // Create sandbox via MicrovmProvider
        let provider: MicrovmProvider = MicrovmProvider::new(&PathBuf::from(".")).await;
        let sandbox = provider.create_sandbox(opts).await.map_err(|e| {
            crate::errors::A3sError::Project(format!("failed to create sandbox: {}", e))
        })?;

        // Get sandbox info
        let sandbox_id = sandbox.id().to_string();
        let socket_path = dirs::home_dir()
            .map(|h| {
                h.join(".a3s")
                    .join("boxes")
                    .join(&sandbox_id)
                    .join("sockets")
                    .join("exec.sock")
            })
            .unwrap_or_else(|| {
                PathBuf::from(format!("~/.a3s/boxes/{}/sockets/exec.sock", &sandbox_id))
            });

        // Create pod state
        let pod = PodActual {
            id: sandbox_id.clone(),
            deployment: deployment_name.to_string(),
            namespace: deployment.namespace.clone(),
            status: PodStatus::Creating,
            health: if deployment.health_check.enabled {
                HealthStatus::Unknown
            } else {
                HealthStatus::Passthrough
            },
            ip: None,
            version: deployment.version.clone(),
            socket_path: Some(socket_path.to_string_lossy().to_string()),
            node_name,
            created_at: Utc::now(),
            last_health_check: None,
            consecutive_failures: 0,
            ready: false,
        };

        // Save pod state
        self.store.set_pod(pod.clone())?;

        // Update service endpoints
        let _ = self.store.update_service_endpoints(deployment_name, vec![]);

        tracing::info!(
            deployment = %deployment_name,
            pod_id = %sandbox_id,
            "pod created"
        );

        // Wait for pod to be running, then update status
        tokio::time::sleep(tokio::time::Duration::from_secs(2)).await;

        let mut pod = pod;
        pod.status = PodStatus::Running;
        self.store.set_pod(pod.clone())?;

        Ok(pod)
    }

    /// Delete a pod.
    pub async fn delete_pod(&self, pod_id: &str) -> Result<()> {
        tracing::info!(pod_id = %pod_id, "deleting pod");

        // Get pod info
        let _pod = match self.store.get_pod(pod_id)? {
            Some(p) => p,
            None => {
                tracing::warn!(pod_id = %pod_id, "pod not found, skipping delete");
                return Ok(());
            }
        };

        // Stop sandbox via MicrovmProvider
        let provider: MicrovmProvider = MicrovmProvider::new(&PathBuf::from(".")).await;
        let _ = provider.undeploy_model(pod_id).await;

        // Delete pod state
        self.store.delete_pod(pod_id)?;

        tracing::info!(pod_id = %pod_id, "pod deleted");
        Ok(())
    }

    /// Replace an unhealthy pod with a new one.
    pub async fn replace_pod(
        &self,
        old_pod_id: &str,
        deployment_name: &str,
        deployment: &crate::state::DeploymentDesired,
    ) -> Result<PodActual> {
        tracing::info!(
            old_pod_id = %old_pod_id,
            deployment = %deployment_name,
            "replacing unhealthy pod"
        );

        // Delete old pod
        self.delete_pod(old_pod_id).await?;

        // Create new pod
        self.create_pod(deployment_name, deployment).await
    }
}

/// Health monitor - checks pod health and updates status.
pub struct HealthMonitor {
    store: Arc<StateStore>,
}

impl HealthMonitor {
    pub fn new(store: Arc<StateStore>) -> Self {
        Self { store }
    }

    /// Check health of a single pod.
    pub async fn check_pod(&self, pod: &PodActual) -> Result<()> {
        let socket_path = match &pod.socket_path {
            Some(p) => PathBuf::from(p),
            None => {
                tracing::debug!(pod_id = %pod.id, "no socket path, skipping health check");
                return Ok(());
            }
        };

        if !socket_path.exists() {
            // Socket doesn't exist - pod might be dead
            self.store.update_pod_health(&pod.id, false)?;
            return Ok(());
        }

        // Try to execute a simple health check command
        let provider: MicrovmProvider = MicrovmProvider::new(&PathBuf::from(".")).await;
        let result = provider.exec_in_sandbox(&pod.id, "echo", &["ok"]).await;

        match result {
            Ok(exec_result) if exec_result.exit_code == 0 => {
                self.store.update_pod_health(&pod.id, true)?;
                tracing::debug!(pod_id = %pod.id, "health check passed");
            }
            Ok(exec_result) => {
                tracing::warn!(
                    pod_id = %pod.id,
                    exit_code = exec_result.exit_code,
                    "health check failed with non-zero exit"
                );
                self.store.update_pod_health(&pod.id, false)?;
            }
            Err(e) => {
                tracing::warn!(
                    pod_id = %pod.id,
                    error = %e,
                    "health check failed"
                );
                self.store.update_pod_health(&pod.id, false)?;
            }
        }

        Ok(())
    }
}

/// Parse memory string like "256Mi", "1Gi" to MB.
fn parse_memory(memory: &Option<String>) -> u32 {
    match memory {
        Some(m) => {
            let m = m.to_lowercase();
            if m.ends_with("gi") {
                m[..m.len() - 2].parse::<u32>().unwrap_or(256) * 1024
            } else if m.ends_with("mi") {
                m[..m.len() - 2].parse::<u32>().unwrap_or(256)
            } else if m.ends_with('g') {
                m[..m.len() - 1].parse::<u32>().unwrap_or(256) * 1024
            } else if m.ends_with('m') {
                m[..m.len() - 1].parse::<u32>().unwrap_or(256)
            } else {
                256
            }
        }
        None => 256,
    }
}
