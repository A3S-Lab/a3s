//! TTL Controller - Time-To-Live Based Resource Cleanup.
//!
//! TTLController automatically cleans up finished resources after TTL expires:
//! - Completed/failed pods
//! - Finished jobs
//! - Completed workflows
//! - Temporary resources
//!
//! This replaces Kubernetes' TTL Controller.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// TTL configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TtlConfig {
    /// Enable TTL controller.
    pub enabled: bool,
    /// Default TTL for finished pods in seconds.
    pub pod_ttl_secs: i64,
    /// Default TTL for completed jobs in seconds.
    pub job_ttl_secs: i64,
    /// Enable periodic cleanup.
    pub periodic_cleanup_enabled: bool,
    /// Cleanup interval in seconds.
    pub cleanup_interval_secs: u64,
}

impl Default for TtlConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            pod_ttl_secs: 3600,      // 1 hour
            job_ttl_secs: 3600 * 24, // 24 hours
            periodic_cleanup_enabled: true,
            cleanup_interval_secs: 3600, // 1 hour
        }
    }
}

/// Resource with TTL info.
#[derive(Debug, Clone)]
pub struct TtlResource {
    /// Resource kind.
    pub kind: TtlResourceKind,
    /// Namespace.
    pub namespace: String,
    /// Name.
    pub name: String,
    /// UID.
    pub uid: String,
    /// When this resource was finished.
    pub finished_at: DateTime<Utc>,
    /// TTL in seconds from finish.
    pub ttl_secs: i64,
    /// Deadline for deletion.
    pub delete_after: DateTime<Utc>,
}

/// Resource kind for TTL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TtlResourceKind {
    /// Pod.
    Pod,
    /// Job.
    Job,
    /// WorkflowRun,
    WorkflowRun,
    /// Temporary resource.
    Temporary,
}

/// TTL controller state.
#[derive(Debug, Clone)]
pub struct TtlControllerState {
    /// Resource key.
    pub key: String,
    /// TTL.
    pub ttl_secs: i64,
    /// Delete after.
    pub delete_after: DateTime<Utc>,
    /// Last check.
    pub last_check: DateTime<Utc>,
}

/// Cleanup result.
#[derive(Debug, Clone)]
pub struct CleanupResult {
    /// Number of pods cleaned.
    pub pods_cleaned: usize,
    /// Number of jobs cleaned.
    pub jobs_cleaned: usize,
    /// Number of other resources cleaned.
    pub other_cleaned: usize,
    /// Errors.
    pub errors: Vec<String>,
}

/// TtlController cleans up finished resources after TTL.
pub struct TtlController {
    /// Configuration.
    config: TtlConfig,
    /// Resources being tracked.
    tracked: RwLock<HashMap<String, TtlControllerState>>,
    /// Running state.
    running: RwLock<bool>,
}

impl TtlController {
    /// Create a new controller.
    pub fn new(config: TtlConfig) -> Self {
        Self {
            config,
            tracked: RwLock::new(HashMap::new()),
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        tracing::info!(
            enabled = self.config.enabled,
            pod_ttl_secs = self.config.pod_ttl_secs,
            job_ttl_secs = self.config.job_ttl_secs,
            "TTLController started"
        );
        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("TTLController stopped");
        Ok(())
    }

    /// Track a finished resource.
    pub async fn track_resource(
        &self,
        kind: TtlResourceKind,
        namespace: &str,
        name: &str,
        uid: &str,
        finished_at: DateTime<Utc>,
    ) -> Result<()> {
        let ttl_secs = self.get_ttl_for_kind(kind);
        let delete_after = finished_at + Duration::seconds(ttl_secs);

        let key = format!("{}/{}/{}/{}", kind.as_str(), namespace, name, uid);
        let state = TtlControllerState {
            key: key.clone(),
            ttl_secs,
            delete_after,
            last_check: Utc::now(),
        };

        let mut tracked = self.tracked.write().await;
        tracked.insert(key, state);

        tracing::debug!(
            kind = ?kind,
            namespace = %namespace,
            name = %name,
            ttl_secs = %ttl_secs,
            delete_after = %delete_after,
            "Tracking resource for TTL cleanup"
        );

        Ok(())
    }

    /// Get TTL for resource kind.
    fn get_ttl_for_kind(&self, kind: TtlResourceKind) -> i64 {
        match kind {
            TtlResourceKind::Pod => self.config.pod_ttl_secs,
            TtlResourceKind::Job => self.config.job_ttl_secs,
            TtlResourceKind::WorkflowRun => self.config.job_ttl_secs,
            TtlResourceKind::Temporary => 300, // 5 minutes
        }
    }

    /// Untrack a resource (when it no longer needs TTL).
    pub async fn untrack_resource(
        &self,
        kind: TtlResourceKind,
        namespace: &str,
        name: &str,
        uid: &str,
    ) -> Result<()> {
        let key = format!("{}/{}/{}/{}", kind.as_str(), namespace, name, uid);
        let mut tracked = self.tracked.write().await;
        tracked.remove(&key);

        tracing::debug!(key = %key, "Untracked resource");
        Ok(())
    }

    /// Run cleanup cycle - returns resources that should be deleted.
    pub async fn run_cleanup_cycle(&self) -> Result<Vec<TtlResource>> {
        if !*self.running.read().await {
            return Err(A3sError::Other("TTLController not running".to_string()));
        }

        let now = Utc::now();
        let mut to_delete = Vec::new();
        let mut tracked = self.tracked.write().await;

        let keys_to_remove: Vec<String> = tracked
            .iter()
            .filter(|(_, state)| now >= state.delete_after)
            .map(|(key, state)| {
                let parts: Vec<&str> = key.split('/').collect();
                let kind_str = parts.first().unwrap_or(&"");
                let namespace = parts.get(1).unwrap_or(&"");
                let name = parts.get(2).unwrap_or(&"");
                let uid = parts.get(3).unwrap_or(&"");

                to_delete.push(TtlResource {
                    kind: TtlResourceKind::from_str(kind_str),
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                    uid: uid.to_string(),
                    finished_at: state.delete_after - Duration::seconds(state.ttl_secs),
                    ttl_secs: state.ttl_secs,
                    delete_after: state.delete_after,
                });

                key.clone()
            })
            .collect();

        // Remove deleted resources from tracking
        for key in keys_to_remove {
            tracked.remove(&key);
        }

        if !to_delete.is_empty() {
            tracing::info!(
                resources = to_delete.len(),
                "TTL cleanup found resources to delete"
            );
        }

        Ok(to_delete)
    }

    /// Get resources pending deletion.
    pub async fn get_pending_deletions(&self) -> Vec<TtlResource> {
        let now = Utc::now();
        let tracked = self.tracked.read().await;

        tracked
            .iter()
            .filter(|(_, state)| now >= state.delete_after)
            .map(|(key, _)| {
                let parts: Vec<&str> = key.split('/').collect();
                let kind_str = parts.first().unwrap_or(&"");
                let namespace = parts.get(1).unwrap_or(&"");
                let name = parts.get(2).unwrap_or(&"");
                let uid = parts.get(3).unwrap_or(&"");

                TtlResource {
                    kind: TtlResourceKind::from_str(kind_str),
                    namespace: namespace.to_string(),
                    name: name.to_string(),
                    uid: uid.to_string(),
                    finished_at: Utc::now(),
                    ttl_secs: 0,
                    delete_after: Utc::now(),
                }
            })
            .collect()
    }

    /// Get tracked resource count.
    pub async fn tracked_count(&self) -> usize {
        let tracked = self.tracked.read().await;
        tracked.len()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl TtlResourceKind {
    /// Convert to string.
    pub fn as_str(&self) -> &'static str {
        match self {
            TtlResourceKind::Pod => "Pod",
            TtlResourceKind::Job => "Job",
            TtlResourceKind::WorkflowRun => "WorkflowRun",
            TtlResourceKind::Temporary => "Temporary",
        }
    }

    /// Parse from string.
    pub fn from_str(s: &str) -> Self {
        match s {
            "Pod" => TtlResourceKind::Pod,
            "Job" => TtlResourceKind::Job,
            "WorkflowRun" => TtlResourceKind::WorkflowRun,
            "Temporary" => TtlResourceKind::Temporary,
            _ => TtlResourceKind::Temporary,
        }
    }
}

impl Default for TtlController {
    fn default() -> Self {
        Self::new(TtlConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_track_resource() {
        let controller = TtlController::default();
        controller.start().await.unwrap();

        let finished_at = Utc::now();
        controller
            .track_resource(
                TtlResourceKind::Pod,
                "default",
                "test-pod",
                "uid-123",
                finished_at,
            )
            .await
            .unwrap();

        let count = controller.tracked_count().await;
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_untrack_resource() {
        let controller = TtlController::default();
        controller.start().await.unwrap();

        controller
            .track_resource(
                TtlResourceKind::Pod,
                "default",
                "test-pod",
                "uid-123",
                Utc::now(),
            )
            .await
            .unwrap();

        controller
            .untrack_resource(TtlResourceKind::Pod, "default", "test-pod", "uid-123")
            .await
            .unwrap();

        let count = controller.tracked_count().await;
        assert_eq!(count, 0);
    }

    #[tokio::test]
    async fn test_cleanup_cycle() {
        let controller = TtlController::default();
        controller.start().await.unwrap();

        // Track with old finish time (should be cleaned)
        let old_finish = Utc::now() - Duration::seconds(7200); // 2 hours ago
        controller
            .track_resource(
                TtlResourceKind::Pod,
                "default",
                "old-pod",
                "uid-old",
                old_finish,
            )
            .await
            .unwrap();

        // Track with recent finish time (should not be cleaned)
        let recent_finish = Utc::now() - Duration::seconds(60); // 1 minute ago
        controller
            .track_resource(
                TtlResourceKind::Pod,
                "default",
                "recent-pod",
                "uid-recent",
                recent_finish,
            )
            .await
            .unwrap();

        let to_delete = controller.run_cleanup_cycle().await.unwrap();
        assert_eq!(to_delete.len(), 1);
        assert_eq!(to_delete[0].name, "old-pod");

        let count = controller.tracked_count().await;
        assert_eq!(count, 1); // recent-pod still tracked
    }
}
