//! Garbage Collection Controller.
//!
//! GarbageCollectionController cleans up orphaned and dead resources:
//! - Terminated pods without owner references
//! - Completed/failed jobs
//! - Orphaned services without endpoints
//! - Unused PVCs
//! - Stale events
//! - Dead containers and images

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;

/// Garbage collection policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GCPolicy {
    /// Enable pod gc.
    pub enable_pod_gc: bool,
    /// Pod gc threshold (orphaned pods).
    pub pod_gc_threshold: usize,
    /// Job gc policy.
    pub job_gc_policy: JobGCPolicy,
    /// PVC gc policy.
    pub pvc_gc_policy: PvcGCPolicy,
    /// Event gc policy.
    pub event_gc_policy: EventGCPolicy,
    /// Image gc policy.
    pub image_gc_policy: ImageGCPolicy,
    /// Container gc policy.
    pub container_gc_policy: ContainerGCPolicy,
    /// Sync period in seconds.
    pub sync_period_secs: u64,
}

impl Default for GCPolicy {
    fn default() -> Self {
        Self {
            enable_pod_gc: true,
            pod_gc_threshold: 1000,
            job_gc_policy: JobGCPolicy::default(),
            pvc_gc_policy: PvcGCPolicy::default(),
            event_gc_policy: EventGCPolicy::default(),
            image_gc_policy: ImageGCPolicy::default(),
            container_gc_policy: ContainerGCPolicy::default(),
            sync_period_secs: 600, // 10 minutes
        }
    }
}

/// Job garbage collection policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobGCPolicy {
    /// Delete finished jobs after seconds.
    pub delete_after_secs: i64,
    /// Failed job history limit.
    pub failed_history_limit: i32,
    /// Successful job history limit.
    pub successful_history_limit: i32,
}

impl Default for JobGCPolicy {
    fn default() -> Self {
        Self {
            delete_after_secs: 3600, // 1 hour
            failed_history_limit: 3,
            successful_history_limit: 3,
        }
    }
}

/// PVC garbage collection policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PvcGCPolicy {
    /// Delete released PVCs after seconds.
    pub delete_released_after_secs: i64,
    /// Delete failed PVs after seconds.
    pub delete_failed_after_secs: i64,
    /// Protect PVs with reclaim policy retain.
    pub protect_retain_policy: bool,
}

impl Default for PvcGCPolicy {
    fn default() -> Self {
        Self {
            delete_released_after_secs: 3600 * 24, // 24 hours
            delete_failed_after_secs: 3600 * 12,   // 12 hours
            protect_retain_policy: true,
        }
    }
}

/// Event garbage collection policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventGCPolicy {
    /// Delete events older than seconds.
    pub delete_after_secs: i64,
    /// Keep warning events longer.
    pub warning_keep_seconds: i64,
    /// Purge interval in seconds.
    pub purge_interval_secs: u64,
}

impl Default for EventGCPolicy {
    fn default() -> Self {
        Self {
            delete_after_secs: 3600 * 24,        // 24 hours
            warning_keep_seconds: 3600 * 24 * 7, // 7 days
            purge_interval_secs: 3600,           // 1 hour
        }
    }
}

/// Image garbage collection policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ImageGCPolicy {
    /// Enable image gc.
    pub enabled: bool,
    /// Minimum age to delete.
    pub min_age_seconds: i64,
    /// Disk usage threshold percent.
    pub high_disk_usage_threshold: f64,
    /// Low disk usage threshold percent.
    pub low_disk_usage_threshold: f64,
    /// Cleanup period seconds.
    pub cleanup_period_secs: u64,
}

impl Default for ImageGCPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            min_age_seconds: 3600 * 24 * 7, // 7 days
            high_disk_usage_threshold: 85.0,
            low_disk_usage_threshold: 80.0,
            cleanup_period_secs: 3600 * 6, // 6 hours
        }
    }
}

/// Container garbage collection policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContainerGCPolicy {
    /// Delete stopped containers after seconds.
    pub delete_stopped_after_secs: i64,
    /// Keep last N containers.
    pub keep_last_n: i32,
    /// Minimum age to delete.
    pub min_age_seconds: i64,
}

impl Default for ContainerGCPolicy {
    fn default() -> Self {
        Self {
            delete_stopped_after_secs: 3600, // 1 hour
            keep_last_n: 1,
            min_age_seconds: 3600, // 1 hour
        }
    }
}

/// Orphaned pod.
#[derive(Debug, Clone)]
pub struct OrphanedPod {
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
    /// Phase.
    pub phase: String,
    /// Owner reference.
    pub owner_reference: Option<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Reason.
    pub reason: String,
}

/// Job to delete.
#[derive(Debug, Clone)]
pub struct JobToDelete {
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
    /// Completed at.
    pub completed_at: Option<DateTime<Utc>>,
    /// Success count.
    pub succeeded: i32,
    /// Failed count.
    pub failed: i32,
    /// Reason.
    pub reason: String,
}

/// Resource to collect.
#[derive(Debug, Clone)]
pub struct GCTarget {
    /// Resource type.
    pub resource_type: GCResourceType,
    /// Namespace (if namespaced).
    pub namespace: Option<String>,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
    /// Reason for collection.
    pub reason: String,
    /// Age.
    pub age_seconds: i64,
}

/// GC resource type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GCResourceType {
    /// Pod.
    Pod,
    /// Job.
    Job,
    /// PVC.
    PersistentVolumeClaim,
    /// PV.
    PersistentVolume,
    /// Event.
    Event,
    /// Image.
    Image,
    /// Container.
    Container,
}

/// GC statistics.
#[derive(Debug, Clone)]
pub struct GCStats {
    /// Pods collected.
    pub pods_collected: usize,
    /// Jobs collected.
    pub jobs_collected: usize,
    /// PVCs collected.
    pub pvcs_collected: usize,
    /// Events collected.
    pub events_collected: usize,
    /// Containers collected.
    pub containers_collected: usize,
    /// Images collected.
    pub images_collected: usize,
    /// Last gc time.
    pub last_gc_time: DateTime<Utc>,
}

/// GarbageCollectionController cleans up dead/orphaned resources.
pub struct GarbageCollectionController {
    /// Configuration.
    policy: GCPolicy,
    /// GC history.
    history: RwLock<Vec<GCStats>>,
    /// Running state.
    running: RwLock<bool>,
}

impl GarbageCollectionController {
    /// Create a new controller.
    pub fn new(policy: GCPolicy) -> Self {
        Self {
            policy,
            history: RwLock::new(Vec::new()),
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        tracing::info!(
            pod_gc_enabled = self.policy.enable_pod_gc,
            sync_period_secs = self.policy.sync_period_secs,
            "GarbageCollectionController started"
        );
        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("GarbageCollectionController stopped");
        Ok(())
    }

    /// Run garbage collection cycle.
    pub async fn run_gc(&self) -> Result<GCStats> {
        if !*self.running.read().await {
            return Err(A3sError::Other("GC controller not running".to_string()));
        }

        let stats = GCStats {
            pods_collected: 0,
            jobs_collected: 0,
            pvcs_collected: 0,
            events_collected: 0,
            containers_collected: 0,
            images_collected: 0,
            last_gc_time: Utc::now(),
        };

        // Run each gc type
        // Note: In real implementation, would actually delete resources

        tracing::debug!(
            pods = stats.pods_collected,
            jobs = stats.jobs_collected,
            pvcs = stats.pvcs_collected,
            events = stats.events_collected,
            "Garbage collection completed"
        );

        // Record history
        let mut history = self.history.write().await;
        history.push(stats.clone());

        // Keep only last 100 entries
        if history.len() > 100 {
            history.remove(0);
        }

        Ok(stats)
    }

    /// Find orphaned pods (pods without owner references in terminal state).
    pub async fn find_orphaned_pods(
        &self,
        pods: &[(String, String, String, String, Option<String>)], // namespace, name, uid, phase, owner
    ) -> Vec<OrphanedPod> {
        let mut orphaned = Vec::new();

        for (namespace, name, uid, phase, owner) in pods {
            let is_orphan = match phase.as_str() {
                "Succeeded" | "Failed" | "Unknown" => {
                    // Pod is in terminal state
                    owner.is_none() || owner.as_ref().is_some_and(|o| o.is_empty())
                }
                _ => false,
            };

            if is_orphan {
                orphaned.push(OrphanedPod {
                    namespace: namespace.clone(),
                    name: name.clone(),
                    uid: uid.clone(),
                    phase: phase.clone(),
                    owner_reference: owner.clone(),
                    created_at: Utc::now(),
                    reason: format!("Pod {} is {} without owner", name, phase),
                });
            }
        }

        orphaned
    }

    /// Find jobs to delete.
    pub async fn find_jobs_to_delete(
        &self,
        jobs: &[(String, String, String, Option<DateTime<Utc>>, i32, i32)], // ns, name, uid, completed_at, succeeded, failed
    ) -> Vec<JobToDelete> {
        let mut to_delete = Vec::new();
        let now = Utc::now();
        let policy = &self.policy.job_gc_policy;

        for (namespace, name, uid, completed_at, succeeded, failed) in jobs {
            // Check if job is finished
            let is_finished = *succeeded > 0 || *failed > 0;

            if is_finished {
                if let Some(completed) = completed_at {
                    let age = (now - *completed).num_seconds();

                    if age > policy.delete_after_secs {
                        to_delete.push(JobToDelete {
                            namespace: namespace.clone(),
                            name: name.clone(),
                            uid: uid.clone(),
                            completed_at: Some(*completed),
                            succeeded: *succeeded,
                            failed: *failed,
                            reason: format!(
                                "Job completed {} seconds ago (limit: {})",
                                age, policy.delete_after_secs
                            ),
                        });
                    }
                } else if *failed >= policy.failed_history_limit {
                    to_delete.push(JobToDelete {
                        namespace: namespace.clone(),
                        name: name.clone(),
                        uid: uid.clone(),
                        completed_at: *completed_at,
                        succeeded: *succeeded,
                        failed: *failed,
                        reason: format!(
                            "Job has {} failed attempts (limit: {})",
                            failed, policy.failed_history_limit
                        ),
                    });
                }
            }
        }

        to_delete
    }

    /// Find PVCs to delete.
    pub async fn find_pvcs_to_delete(
        &self,
        pvcs: &[(String, String, String, String)], // ns, name, uid, status
    ) -> Vec<GCTarget> {
        let mut to_delete = Vec::new();

        for (namespace, name, uid, status) in pvcs {
            if status == "Released" {
                to_delete.push(GCTarget {
                    resource_type: GCResourceType::PersistentVolumeClaim,
                    namespace: Some(namespace.clone()),
                    name: name.clone(),
                    uid: uid.clone(),
                    reason: "PVC is Released".to_string(),
                    age_seconds: 0,
                });
            }
        }

        to_delete
    }

    /// Get GC policy.
    pub fn policy(&self) -> &GCPolicy {
        &self.policy
    }

    /// Get GC history.
    pub async fn get_history(&self) -> Vec<GCStats> {
        let history = self.history.read().await;
        history.clone()
    }

    /// Get last GC stats.
    pub async fn get_last_stats(&self) -> Option<GCStats> {
        let history = self.history.read().await;
        history.last().cloned()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for GarbageCollectionController {
    fn default() -> Self {
        Self::new(GCPolicy::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[tokio::test]
    async fn test_gc_creation() {
        let gc = GarbageCollectionController::new(GCPolicy::default());
        assert!(!gc.is_running().await);
    }

    #[tokio::test]
    async fn test_find_orphaned_pods() {
        let gc = GarbageCollectionController::new(GCPolicy::default());

        let pods = vec![
            (
                "default".to_string(),
                "pod-1".to_string(),
                "uid-1".to_string(),
                "Succeeded".to_string(),
                None,
            ),
            (
                "default".to_string(),
                "pod-2".to_string(),
                "uid-2".to_string(),
                "Running".to_string(),
                Some("rs-1".to_string()),
            ),
            (
                "default".to_string(),
                "pod-3".to_string(),
                "uid-3".to_string(),
                "Failed".to_string(),
                None,
            ),
        ];

        let orphaned = gc.find_orphaned_pods(&pods).await;
        assert_eq!(orphaned.len(), 2); // pod-1 and pod-3
    }

    #[tokio::test]
    async fn test_find_jobs_to_delete() {
        let gc = GarbageCollectionController::new(GCPolicy::default());

        let jobs = vec![
            (
                "default".to_string(),
                "job-1".to_string(),
                "uid-1".to_string(),
                Some(Utc::now() - Duration::hours(2)),
                1,
                0,
            ),
            (
                "default".to_string(),
                "job-2".to_string(),
                "uid-2".to_string(),
                Some(Utc::now() - Duration::minutes(30)),
                1,
                0,
            ),
        ];

        let to_delete = gc.find_jobs_to_delete(&jobs).await;
        // job-1 completed 2 hours ago, policy is 1 hour - should be deleted
        assert_eq!(to_delete.len(), 1);
        assert_eq!(to_delete[0].name, "job-1");
    }

    #[tokio::test]
    async fn test_gc_run() {
        let gc = GarbageCollectionController::new(GCPolicy::default());
        gc.start().await.unwrap();

        let stats = gc.run_gc().await.unwrap();
        // Check that last_gc_time is recent (within 5 seconds)
        let now = Utc::now();
        let diff = now.signed_duration_since(stats.last_gc_time);
        assert!(
            diff.num_seconds() < 5,
            "last_gc_time should be within 5 seconds of now"
        );
    }
}
