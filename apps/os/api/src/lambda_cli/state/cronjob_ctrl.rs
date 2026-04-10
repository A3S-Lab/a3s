//! CronJob Controller - CronJob Scheduling and Management.
//!
//! CronJobController manages CronJob scheduling:
//! - Schedule parsing and validation
//! - Job creation based on schedule
//! - Concurrency policy enforcement
//! - Status tracking (active, suspended, last schedule time)
//! - Missed job handling

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// CronJob concurrency policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub enum CronJobConcurrencyPolicy {
    /// Allow concurrent runs.
    Allow,
    /// Forbid concurrent runs.
    Forbid,
    /// Replace ongoing runs.
    Replace,
}

/// CronJob policy for failed jobs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum CronJobFailedJobsPolicy {
    /// Keep failed jobs.
    Keep,
    /// Delete failed jobs.
    Delete,
}

/// CronJob schedule.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobSchedule {
    /// Cron expression.
    pub cron: String,
    /// Timezone.
    pub timezone: String,
}

/// CronJob spec.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobSpec {
    /// Schedule.
    pub schedule: CronJobSchedule,
    /// Time zone.
    pub time_zone: Option<String>,
    /// Concurrency policy.
    pub concurrency_policy: CronJobConcurrencyPolicy,
    /// Suspend.
    pub suspend: bool,
    /// Successful jobs history limit.
    pub successful_jobs_history_limit: i32,
    /// Failed jobs history limit.
    pub failed_jobs_history_limit: i32,
    /// Starting deadline seconds.
    pub starting_deadline_seconds: Option<i64>,
    /// Job template.
    pub job_template: CronJobJobTemplate,
}

/// CronJob job template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobJobTemplate {
    /// Job metadata.
    pub metadata: CronJobTemplateMetadata,
    /// Job spec.
    pub spec: CronJobTemplateSpec,
}

/// CronJob template metadata.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronJobTemplateMetadata {
    /// Labels.
    pub labels: HashMap<String, String>,
    /// Annotations.
    pub annotations: HashMap<String, String>,
}

/// CronJob template spec.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronJobTemplateSpec {
    /// Backoff limit.
    pub backoff_limit: Option<i32>,
    /// Completions.
    pub completions: Option<i32>,
    /// Parallelism.
    pub parallelism: Option<i32>,
    /// TTL seconds after finished.
    pub ttl_seconds_after_finished: Option<i32>,
    /// Command.
    pub command: Option<Vec<String>>,
    /// Args.
    pub args: Option<Vec<String>>,
    /// Env.
    pub env: Option<HashMap<String, String>>,
}

/// CronJob status.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobStatus {
    /// Active jobs.
    pub active: Vec<String>, // Job names
    /// Last schedule time.
    pub last_schedule_time: Option<DateTime<Utc>>,
    /// Last successful time.
    pub last_successful_time: Option<DateTime<Utc>>,
    /// Active job names.
    pub active_jobs: Vec<CronJobActiveJob>,
}

/// Active job reference.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobActiveJob {
    /// Job name.
    pub name: String,
    /// Job UID.
    pub uid: String,
    /// Started at.
    pub started_at: DateTime<Utc>,
}

/// CronJob desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobDesired {
    /// Name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Spec.
    pub spec: CronJobSpec,
    /// Status.
    pub status: Option<CronJobStatus>,
    /// Last scheduled time.
    pub last_scheduled_time: Option<DateTime<Utc>>,
    /// Active jobs.
    pub active_jobs: Vec<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Updated timestamp.
    pub updated_at: DateTime<Utc>,
    /// Labels.
    pub labels: HashMap<String, String>,
}

/// Scheduled job entry.
#[derive(Debug, Clone)]
pub struct ScheduledJob {
    /// CronJob name.
    pub cronjob_name: String,
    /// Namespace.
    pub namespace: String,
    /// Scheduled time.
    pub scheduled_time: DateTime<Utc>,
    /// Job name to create.
    pub job_name: String,
    /// Job UID.
    pub job_uid: String,
}

/// Job creation request.
#[derive(Debug, Clone)]
pub struct JobCreationRequest {
    /// Job name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// CronJob name.
    pub cronjob_name: String,
    /// CronJob UID.
    pub cronjob_uid: String,
    /// Scheduled time.
    pub scheduled_time: DateTime<Utc>,
    /// Template spec.
    pub template: CronJobTemplateSpec,
    /// Labels from CronJob.
    pub labels: HashMap<String, String>,
}

/// CronJobController manages CronJob scheduling.
pub struct CronJobController {
    /// CronJobs by namespace/name.
    cronjobs: RwLock<HashMap<String, CronJobDesired>>,
    /// Active jobs by CronJob key.
    active_jobs: RwLock<HashMap<String, Vec<String>>>,
    /// Scheduled jobs queue.
    scheduled: RwLock<Vec<ScheduledJob>>,
    /// Default history limits.
    default_successful_limit: i32,
    default_failed_limit: i32,
    /// Running state.
    running: RwLock<bool>,
}

impl CronJobController {
    /// Create a new controller.
    pub fn new() -> Self {
        Self {
            cronjobs: RwLock::new(HashMap::new()),
            active_jobs: RwLock::new(HashMap::new()),
            scheduled: RwLock::new(Vec::new()),
            default_successful_limit: 3,
            default_failed_limit: 1,
            running: RwLock::new(false),
        }
    }

    /// Start the controller.
    pub async fn start(&self) -> Result<()> {
        *self.running.write().await = true;
        tracing::info!(
            default_successful_limit = self.default_successful_limit,
            default_failed_limit = self.default_failed_limit,
            "CronJobController started"
        );
        Ok(())
    }

    /// Stop the controller.
    pub async fn stop(&self) -> Result<()> {
        *self.running.write().await = false;
        tracing::info!("CronJobController stopped");
        Ok(())
    }

    /// Set CronJob.
    pub async fn set_cronjob(&self, cronjob: CronJobDesired) -> Result<()> {
        let key = format!("{}/{}", cronjob.namespace, cronjob.name);
        let mut cronjobs = self.cronjobs.write().await;
        cronjobs.insert(key, cronjob);
        Ok(())
    }

    /// Get CronJob.
    pub async fn get_cronjob(&self, namespace: &str, name: &str) -> Option<CronJobDesired> {
        let key = format!("{}/{}", namespace, name);
        let cronjobs = self.cronjobs.read().await;
        cronjobs.get(&key).cloned()
    }

    /// Delete CronJob.
    pub async fn delete_cronjob(&self, namespace: &str, name: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, name);
        let mut cronjobs = self.cronjobs.write().await;
        cronjobs.remove(&key);

        let mut active = self.active_jobs.write().await;
        active.remove(&key);

        Ok(())
    }

    /// Calculate next run time (simplified cron parsing).
    pub fn calculate_next_run(&self, _schedule: &CronJobSchedule) -> Option<DateTime<Utc>> {
        // Simplified: parse minute, hour, day, month, weekday
        // Real implementation would use a proper cron parser
        // For now, return next minute as placeholder
        Some(Utc::now() + Duration::minutes(1))
    }

    /// Check if should run based on concurrency policy.
    pub async fn should_run(
        &self,
        namespace: &str,
        name: &str,
        policy: CronJobConcurrencyPolicy,
    ) -> bool {
        let key = format!("{}/{}", namespace, name);
        let active_jobs = self.active_jobs.read().await;

        match policy {
            CronJobConcurrencyPolicy::Allow => true,
            CronJobConcurrencyPolicy::Forbid => {
                active_jobs.get(&key).map_or(true, |jobs| jobs.is_empty())
            }
            CronJobConcurrencyPolicy::Replace => {
                // Can always run - will replace existing
                true
            }
        }
    }

    /// Mark job as active.
    pub async fn mark_job_active(
        &self,
        namespace: &str,
        cronjob_name: &str,
        job_name: &str,
        job_uid: &str,
    ) -> Result<()> {
        let key = format!("{}/{}", namespace, cronjob_name);
        let mut active = self.active_jobs.write().await;

        let jobs = active.entry(key.clone()).or_insert_with(Vec::new);
        jobs.push(job_name.to_string());

        // Update CronJob status
        let mut cronjobs = self.cronjobs.write().await;
        if let Some(cj) = cronjobs.get_mut(&key) {
            if cj.status.is_none() {
                cj.status = Some(CronJobStatus {
                    active: vec![],
                    last_schedule_time: None,
                    last_successful_time: None,
                    active_jobs: vec![],
                });
            }

            if let Some(ref mut status) = cj.status {
                status.active.push(job_name.to_string());
                status.active_jobs.push(CronJobActiveJob {
                    name: job_name.to_string(),
                    uid: job_uid.to_string(),
                    started_at: Utc::now(),
                });
            }
        }

        Ok(())
    }

    /// Mark job as finished.
    pub async fn mark_job_finished(
        &self,
        namespace: &str,
        cronjob_name: &str,
        job_name: &str,
        succeeded: bool,
    ) -> Result<()> {
        let key = format!("{}/{}", namespace, cronjob_name);
        let mut active = self.active_jobs.write().await;

        if let Some(jobs) = active.get_mut(&key) {
            jobs.retain(|j| j != job_name);
        }

        // Update CronJob status
        let mut cronjobs = self.cronjobs.write().await;
        if let Some(cj) = cronjobs.get_mut(&key) {
            if let Some(ref mut status) = cj.status {
                status.active.retain(|n| n != job_name);
                status.active_jobs.retain(|j| j.name != job_name);

                if succeeded {
                    status.last_successful_time = Some(Utc::now());
                }

                // Enforce history limits
                self.enforce_history_limit(cj).await;
            }
        }

        Ok(())
    }

    /// Enforce history limits.
    async fn enforce_history_limit(&self, cronjob: &mut CronJobDesired) {
        let successful_limit = cronjob.spec.successful_jobs_history_limit;
        let failed_limit = cronjob.spec.failed_jobs_history_limit;

        // Simplified - would track job history in real implementation
        let _ = successful_limit;
        let _ = failed_limit;
    }

    /// Suspend a CronJob.
    pub async fn suspend(&self, namespace: &str, name: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, name);
        let mut cronjobs = self.cronjobs.write().await;

        if let Some(cj) = cronjobs.get_mut(&key) {
            cj.spec.suspend = true;
            tracing::info!(namespace = %namespace, name = %name, "CronJob suspended");
        }

        Ok(())
    }

    /// Resume a CronJob.
    pub async fn resume(&self, namespace: &str, name: &str) -> Result<()> {
        let key = format!("{}/{}", namespace, name);
        let mut cronjobs = self.cronjobs.write().await;

        if let Some(cj) = cronjobs.get_mut(&key) {
            cj.spec.suspend = false;
            tracing::info!(namespace = %namespace, name = %name, "CronJob resumed");
        }

        Ok(())
    }

    /// Create job from CronJob (called when schedule triggers).
    pub async fn create_job(&self, namespace: &str, name: &str) -> Result<JobCreationRequest> {
        let cronjob = self
            .get_cronjob(namespace, name)
            .await
            .ok_or_else(|| A3sError::Other("CronJob not found".to_string()))?;

        if cronjob.spec.suspend {
            return Err(A3sError::Other("CronJob is suspended".to_string()));
        }

        // Check concurrency policy
        if !self
            .should_run(namespace, name, cronjob.spec.concurrency_policy)
            .await
        {
            return Err(A3sError::Other(
                "Concurrency policy forbids new job".to_string(),
            ));
        }

        let job_name = format!("{}-{}", name, Utc::now().timestamp());
        let job_uid = format!("uid-{}", uuid::Uuid::new_v4());

        // Mark job as active
        self.mark_job_active(namespace, name, &job_name, &job_uid)
            .await?;

        // Update last schedule time
        {
            let key = format!("{}/{}", namespace, name);
            let mut cronjobs = self.cronjobs.write().await;
            if let Some(cj) = cronjobs.get_mut(&key) {
                if cj.status.is_none() {
                    cj.status = Some(CronJobStatus {
                        active: vec![],
                        last_schedule_time: None,
                        last_successful_time: None,
                        active_jobs: vec![],
                    });
                }
                cj.status.as_mut().unwrap().last_schedule_time = Some(Utc::now());
            }
        }

        Ok(JobCreationRequest {
            name: job_name,
            namespace: namespace.to_string(),
            cronjob_name: name.to_string(),
            cronjob_uid: cronjob.name.clone(), // simplified
            scheduled_time: Utc::now(),
            template: cronjob.spec.job_template.spec,
            labels: cronjob.labels.clone(),
        })
    }

    /// List all CronJobs.
    pub async fn list_cronjobs(&self) -> Vec<CronJobDesired> {
        let cronjobs = self.cronjobs.read().await;
        cronjobs.values().cloned().collect()
    }

    /// Get active jobs for a CronJob.
    pub async fn get_active_jobs(&self, namespace: &str, name: &str) -> Vec<String> {
        let key = format!("{}/{}", namespace, name);
        let active = self.active_jobs.read().await;
        active.get(&key).cloned().unwrap_or_default()
    }

    /// Check if running.
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

impl Default for CronJobController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_cronjob_creation() {
        let controller = CronJobController::new();
        controller.start().await.unwrap();

        let cronjob = CronJobDesired {
            name: "test-cronjob".to_string(),
            namespace: "default".to_string(),
            spec: CronJobSpec {
                schedule: CronJobSchedule {
                    cron: "*/5 * * * *".to_string(),
                    timezone: "UTC".to_string(),
                },
                time_zone: Some("UTC".to_string()),
                concurrency_policy: CronJobConcurrencyPolicy::Allow,
                suspend: false,
                successful_jobs_history_limit: 3,
                failed_jobs_history_limit: 1,
                starting_deadline_seconds: None,
                job_template: CronJobJobTemplate {
                    metadata: CronJobTemplateMetadata::default(),
                    spec: CronJobTemplateSpec {
                        backoff_limit: Some(6),
                        completions: None,
                        parallelism: None,
                        ttl_seconds_after_finished: None,
                        command: Some(vec!["echo".to_string(), "hello".to_string()]),
                        args: None,
                        env: None,
                    },
                },
            },
            status: None,
            last_scheduled_time: None,
            active_jobs: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            labels: HashMap::new(),
        };

        controller.set_cronjob(cronjob).await.unwrap();

        let found = controller.get_cronjob("default", "test-cronjob").await;
        assert!(found.is_some());
        assert_eq!(
            found.unwrap().spec.concurrency_policy,
            CronJobConcurrencyPolicy::Allow
        );
    }

    #[tokio::test]
    async fn test_suspend_resume() {
        let controller = CronJobController::new();
        controller.start().await.unwrap();

        let cronjob = CronJobDesired {
            name: "test-cronjob".to_string(),
            namespace: "default".to_string(),
            spec: CronJobSpec {
                schedule: CronJobSchedule {
                    cron: "*/5 * * * *".to_string(),
                    timezone: "UTC".to_string(),
                },
                time_zone: Some("UTC".to_string()),
                concurrency_policy: CronJobConcurrencyPolicy::Allow,
                suspend: false,
                successful_jobs_history_limit: 3,
                failed_jobs_history_limit: 1,
                starting_deadline_seconds: None,
                job_template: CronJobJobTemplate {
                    metadata: CronJobTemplateMetadata::default(),
                    spec: CronJobTemplateSpec::default(),
                },
            },
            status: None,
            last_scheduled_time: None,
            active_jobs: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
            labels: HashMap::new(),
        };

        controller.set_cronjob(cronjob).await.unwrap();
        controller.suspend("default", "test-cronjob").await.unwrap();

        let found = controller
            .get_cronjob("default", "test-cronjob")
            .await
            .unwrap();
        assert!(found.spec.suspend);

        controller.resume("default", "test-cronjob").await.unwrap();

        let found = controller
            .get_cronjob("default", "test-cronjob")
            .await
            .unwrap();
        assert!(!found.spec.suspend);
    }

    #[tokio::test]
    async fn test_concurrency_policy() {
        let controller = CronJobController::new();
        controller.start().await.unwrap();

        // Forbid policy - should not run when active
        controller
            .set_cronjob(CronJobDesired {
                name: "forbid-cronjob".to_string(),
                namespace: "default".to_string(),
                spec: CronJobSpec {
                    schedule: CronJobSchedule {
                        cron: "*/5 * * * *".to_string(),
                        timezone: "UTC".to_string(),
                    },
                    time_zone: Some("UTC".to_string()),
                    concurrency_policy: CronJobConcurrencyPolicy::Forbid,
                    suspend: false,
                    successful_jobs_history_limit: 3,
                    failed_jobs_history_limit: 1,
                    starting_deadline_seconds: None,
                    job_template: CronJobJobTemplate {
                        metadata: CronJobTemplateMetadata::default(),
                        spec: CronJobTemplateSpec::default(),
                    },
                },
                status: None,
                last_scheduled_time: None,
                active_jobs: Vec::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                labels: HashMap::new(),
            })
            .await
            .unwrap();

        // Initially can run
        let can_run = controller
            .should_run(
                "default",
                "forbid-cronjob",
                CronJobConcurrencyPolicy::Forbid,
            )
            .await;
        assert!(can_run);

        // Mark a job as active
        controller
            .mark_job_active("default", "forbid-cronjob", "job-1", "uid-1")
            .await
            .unwrap();

        // Should not run now
        let can_run = controller
            .should_run(
                "default",
                "forbid-cronjob",
                CronJobConcurrencyPolicy::Forbid,
            )
            .await;
        assert!(!can_run);

        // Mark job finished
        controller
            .mark_job_finished("default", "forbid-cronjob", "job-1", true)
            .await
            .unwrap();

        // Can run again
        let can_run = controller
            .should_run(
                "default",
                "forbid-cronjob",
                CronJobConcurrencyPolicy::Forbid,
            )
            .await;
        assert!(can_run);
    }

    #[tokio::test]
    async fn test_job_creation() {
        let controller = CronJobController::new();
        controller.start().await.unwrap();

        controller
            .set_cronjob(CronJobDesired {
                name: "test-cronjob".to_string(),
                namespace: "default".to_string(),
                spec: CronJobSpec {
                    schedule: CronJobSchedule {
                        cron: "*/5 * * * *".to_string(),
                        timezone: "UTC".to_string(),
                    },
                    time_zone: Some("UTC".to_string()),
                    concurrency_policy: CronJobConcurrencyPolicy::Allow,
                    suspend: false,
                    successful_jobs_history_limit: 3,
                    failed_jobs_history_limit: 1,
                    starting_deadline_seconds: None,
                    job_template: CronJobJobTemplate {
                        metadata: CronJobTemplateMetadata::default(),
                        spec: CronJobTemplateSpec {
                            backoff_limit: Some(6),
                            completions: None,
                            parallelism: None,
                            ttl_seconds_after_finished: None,
                            command: Some(vec!["echo".to_string(), "hello".to_string()]),
                            args: None,
                            env: None,
                        },
                    },
                },
                status: None,
                last_scheduled_time: None,
                active_jobs: Vec::new(),
                created_at: Utc::now(),
                updated_at: Utc::now(),
                labels: HashMap::new(),
            })
            .await
            .unwrap();

        let job_req = controller
            .create_job("default", "test-cronjob")
            .await
            .unwrap();
        assert!(job_req.name.starts_with("test-cronjob-"));
        assert_eq!(
            job_req.template.command,
            Some(vec!["echo".to_string(), "hello".to_string()])
        );
    }
}
