//! Batch Workload - Job and CronJob support.
//!
//! Provides Job and CronJob controllers for batch processing workloads.

use crate::errors::{A3sError, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

/// Job completion policy.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum CompletionMode {
    /// Jobs complete when all pods finish successfully.
    #[default]
    NonIndexed,
    /// Each pod has an index and completes individually.
    Indexed,
}

/// Job backoff policy.
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum BackoffLimitPolicy {
    /// No backoff limit.
    Unlimited,
    /// Maximum number of retries.
    Limited(u32),
}

impl Default for BackoffLimitPolicy {
    fn default() -> Self {
        BackoffLimitPolicy::Limited(6)
    }
}

/// Job specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobSpec {
    /// Number of successful completions.
    pub completions: Option<i32>,
    /// Number of parallel pods.
    pub parallelism: Option<i32>,
    /// Active deadline seconds.
    pub active_deadline_seconds: Option<i64>,
    /// Backoff limit.
    #[serde(default)]
    pub backoff_limit: BackoffLimitPolicy,
    /// Retry policy.
    #[serde(default)]
    pub retry_policy: JobRetryPolicy,
    /// Pod template.
    pub template: JobPodTemplate,
    /// Completion mode.
    #[serde(default)]
    pub completion_mode: CompletionMode,
    /// TTL seconds after finished.
    pub ttl_seconds_after_finished: Option<i32>,
}

/// Retry policy for jobs.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum JobRetryPolicy {
    /// Never retry.
    #[default]
    Never,
    /// Retry on failure.
    OnFailure,
}

/// Job desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobDesired {
    /// Job name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Job specification.
    pub spec: JobSpec,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// Job actual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobActual {
    /// Job name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Start time.
    pub start_time: Option<DateTime<Utc>>,
    /// Completion time.
    pub completion_time: Option<DateTime<Utc>>,
    /// Active pods.
    pub active: i32,
    /// Succeeded pods.
    pub succeeded: i32,
    /// Failed pods.
    pub failed: i32,
    /// Ready pods.
    pub ready: i32,
    /// Whether job is suspended.
    pub suspended: bool,
    /// Conditions.
    #[serde(default)]
    pub conditions: Vec<JobCondition>,
}

/// Job condition.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobCondition {
    /// Type.
    pub type_: String,
    /// Status (True/False/Unknown).
    pub status: String,
    /// Reason.
    pub reason: Option<String>,
    /// Message.
    pub message: Option<String>,
    /// Last transition time.
    pub last_transition_time: DateTime<Utc>,
}

/// Pod template for jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPodTemplate {
    /// Metadata.
    pub metadata: JobPodMetadata,
    /// Pod spec.
    pub spec: JobPodSpec,
}

/// Pod metadata for jobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobPodMetadata {
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
}

/// Pod spec for jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobPodSpec {
    /// Container image.
    pub image: String,
    /// Command.
    #[serde(default)]
    pub command: Vec<String>,
    /// Arguments.
    #[serde(default)]
    pub args: Vec<String>,
    /// Environment variables.
    #[serde(default)]
    pub env: HashMap<String, String>,
    /// Resource requirements.
    #[serde(default)]
    pub resources: JobResourceRequirements,
    /// Restart policy.
    #[serde(default)]
    pub restart_policy: JobRestartPolicy,
}

/// Resource requirements for jobs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct JobResourceRequirements {
    pub memory_limit: Option<String>,
    pub cpu_limit: Option<String>,
    pub memory_request: Option<String>,
    pub cpu_request: Option<String>,
}

/// Restart policy for job pods.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum JobRestartPolicy {
    /// Never restart.
    #[default]
    Never,
    /// Restart on failure.
    OnFailure,
    /// Always restart (not typically used in jobs).
    Always,
}

/// CronJob schedule format.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct CronJobSchedule {
    /// Minute (0-59).
    pub minute: String,
    /// Hour (0-23).
    pub hour: String,
    /// Day of month (1-31).
    pub day_of_month: String,
    /// Month (1-12).
    pub month: String,
    /// Day of week (0-6).
    pub day_of_week: String,
}

impl CronJobSchedule {
    /// Parse a standard cron expression (minute hour day month dow).
    pub fn parse(cron: &str) -> Result<Self> {
        let parts: Vec<&str> = cron.split_whitespace().collect();
        if parts.len() != 5 {
            return Err(A3sError::Project(format!(
                "invalid cron expression: {}",
                cron
            )));
        }

        Ok(Self {
            minute: parts[0].to_string(),
            hour: parts[1].to_string(),
            day_of_month: parts[2].to_string(),
            month: parts[3].to_string(),
            day_of_week: parts[4].to_string(),
        })
    }

    /// Format as standard cron string.
    pub fn format(&self) -> String {
        format!(
            "{} {} {} {} {}",
            self.minute, self.hour, self.day_of_month, self.month, self.day_of_week
        )
    }
}

/// CronJob update policy.
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub enum CronJobConcurrencyPolicy {
    /// Allow concurrent runs.
    #[default]
    Allow,
    /// Forbid concurrent runs.
    Forbid,
    /// Replace running job.
    Replace,
}

/// CronJob specification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobSpec {
    /// Schedule (cron format).
    pub schedule: CronJobSchedule,
    /// Time zone.
    pub time_zone: Option<String>,
    /// Starting deadline seconds.
    pub starting_deadline_seconds: Option<i64>,
    /// Job template.
    pub job_template: JobTemplate,
    /// Concurrency policy.
    #[serde(default)]
    pub concurrency_policy: CronJobConcurrencyPolicy,
    /// Suspend.
    #[serde(default)]
    pub suspend: bool,
    /// Successful jobs history limit.
    pub successful_jobs_history_limit: Option<i32>,
    /// Failed jobs history limit.
    pub failed_jobs_history_limit: Option<i32>,
}

/// Job template for CronJob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobTemplate {
    /// Job specification.
    pub spec: JobSpec,
}

/// CronJob desired state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobDesired {
    /// CronJob name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// CronJob specification.
    pub spec: CronJobSpec,
    /// Last scheduled time.
    pub last_scheduled_time: Option<DateTime<Utc>>,
    /// Active jobs.
    #[serde(default)]
    pub active_jobs: Vec<String>,
    /// Creation timestamp.
    pub created_at: DateTime<Utc>,
    /// Last update timestamp.
    pub updated_at: DateTime<Utc>,
}

/// CronJob actual state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJobActual {
    /// CronJob name.
    pub name: String,
    /// Namespace.
    pub namespace: String,
    /// Last scheduled time.
    pub last_scheduled_time: Option<DateTime<Utc>>,
    /// Active jobs.
    #[serde(default)]
    pub active_jobs: Vec<String>,
    /// Last successful run time.
    pub last_successful_time: Option<DateTime<Utc>>,
}

/// Job controller.
pub struct JobController {
    /// Jobs.
    jobs: RwLock<HashMap<String, HashMap<String, JobDesired>>>,
    /// Job actual states.
    actuals: RwLock<HashMap<String, HashMap<String, JobActual>>>,
}

impl JobController {
    /// Create a new Job controller.
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
            actuals: RwLock::new(HashMap::new()),
        }
    }

    // ==================== Job Operations ====================

    /// Create a Job.
    pub async fn create_job(&self, job: JobDesired) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        let ns_jobs = jobs
            .entry(job.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_jobs.insert(job.name.clone(), job);
        Ok(())
    }

    /// Get a Job.
    pub async fn get_job(&self, namespace: &str, name: &str) -> Option<JobDesired> {
        let jobs = self.jobs.read().await;
        jobs.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// List all Jobs in a namespace.
    pub async fn list_jobs(&self, namespace: &str) -> Vec<JobDesired> {
        let jobs = self.jobs.read().await;
        jobs.get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all Jobs.
    pub async fn list_all_jobs(&self) -> Vec<(String, JobDesired)> {
        let jobs = self.jobs.read().await;
        jobs.iter()
            .flat_map(|(ns, ns_jobs)| {
                ns_jobs
                    .values()
                    .map(|j| (ns.clone(), j.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Update a Job.
    pub async fn update_job(&self, job: JobDesired) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(ns_jobs) = jobs.get_mut(&job.namespace) {
            if let Some(existing) = ns_jobs.get_mut(&job.name) {
                *existing = job;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!(
            "Job {} not found in namespace {}",
            job.name, job.namespace
        )))
    }

    /// Delete a Job.
    pub async fn delete_job(&self, namespace: &str, name: &str) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        if let Some(ns_jobs) = jobs.get_mut(namespace) {
            ns_jobs.remove(name);
        }
        Ok(())
    }

    // ==================== Job Actual State ====================

    /// Initialize or update actual state for a Job.
    pub async fn update_actual(&self, actual: JobActual) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        let ns_actuals = actuals
            .entry(actual.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_actuals.insert(actual.name.clone(), actual);
        Ok(())
    }

    /// Get actual state for a Job.
    pub async fn get_actual(&self, namespace: &str, name: &str) -> Option<JobActual> {
        let actuals = self.actuals.read().await;
        actuals.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// Increment active count.
    pub async fn increment_active(&self, namespace: &str, name: &str) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.active += 1;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("Job {} not found", name)))
    }

    /// Decrement active count.
    pub async fn decrement_active(&self, namespace: &str, name: &str) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.active = actual.active.saturating_sub(1);
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("Job {} not found", name)))
    }

    /// Increment succeeded count.
    pub async fn increment_succeeded(&self, namespace: &str, name: &str) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.succeeded += 1;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("Job {} not found", name)))
    }

    /// Increment failed count.
    pub async fn increment_failed(&self, namespace: &str, name: &str) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.failed += 1;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("Job {} not found", name)))
    }

    /// Add condition to job.
    pub async fn add_condition(
        &self,
        namespace: &str,
        name: &str,
        condition: JobCondition,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                // Remove old condition of same type
                actual.conditions.retain(|c| c.type_ != condition.type_);
                actual.conditions.push(condition);
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("Job {} not found", name)))
    }

    /// Check if job is complete.
    pub async fn is_complete(&self, namespace: &str, name: &str) -> bool {
        if let Some(actual) = self.get_actual(namespace, name).await {
            // Check if succeeded equals completions
            if let Some(job) = self.get_job(namespace, name).await {
                if let Some(completions) = job.spec.completions {
                    return actual.succeeded >= completions;
                }
            }
            // If no completions specified, job is complete when active == 0 && succeeded > 0
            return actual.active == 0 && actual.succeeded > 0;
        }
        false
    }

    /// Check if job failed.
    pub async fn is_failed(&self, namespace: &str, name: &str) -> bool {
        if let Some(actual) = self.get_actual(namespace, name).await {
            if let Some(job) = self.get_job(namespace, name).await {
                let backoff_limit = match job.spec.backoff_limit {
                    BackoffLimitPolicy::Limited(n) => n as i32,
                    BackoffLimitPolicy::Unlimited => i32::MAX,
                };
                return actual.failed >= backoff_limit;
            }
        }
        false
    }
}

impl Default for JobController {
    fn default() -> Self {
        Self::new()
    }
}

/// CronJob controller.
pub struct CronJobController {
    /// CronJobs.
    cronjobs: RwLock<HashMap<String, HashMap<String, CronJobDesired>>>,
    /// CronJob actual states.
    actuals: RwLock<HashMap<String, HashMap<String, CronJobActual>>>,
}

impl CronJobController {
    /// Create a new CronJob controller.
    pub fn new() -> Self {
        Self {
            cronjobs: RwLock::new(HashMap::new()),
            actuals: RwLock::new(HashMap::new()),
        }
    }

    // ==================== CronJob Operations ====================

    /// Create a CronJob.
    pub async fn create_cronjob(&self, cronjob: CronJobDesired) -> Result<()> {
        let mut cronjobs = self.cronjobs.write().await;
        let ns_cronjobs = cronjobs
            .entry(cronjob.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_cronjobs.insert(cronjob.name.clone(), cronjob);
        Ok(())
    }

    /// Get a CronJob.
    pub async fn get_cronjob(&self, namespace: &str, name: &str) -> Option<CronJobDesired> {
        let cronjobs = self.cronjobs.read().await;
        cronjobs.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// List all CronJobs in a namespace.
    pub async fn list_cronjobs(&self, namespace: &str) -> Vec<CronJobDesired> {
        let cronjobs = self.cronjobs.read().await;
        cronjobs
            .get(namespace)
            .map(|ns| ns.values().cloned().collect())
            .unwrap_or_default()
    }

    /// List all CronJobs.
    pub async fn list_all_cronjobs(&self) -> Vec<(String, CronJobDesired)> {
        let cronjobs = self.cronjobs.read().await;
        cronjobs
            .iter()
            .flat_map(|(ns, ns_cronjobs)| {
                ns_cronjobs
                    .values()
                    .map(|cj| (ns.clone(), cj.clone()))
                    .collect::<Vec<_>>()
            })
            .collect()
    }

    /// Update a CronJob.
    pub async fn update_cronjob(&self, cronjob: CronJobDesired) -> Result<()> {
        let mut cronjobs = self.cronjobs.write().await;
        if let Some(ns_cronjobs) = cronjobs.get_mut(&cronjob.namespace) {
            if let Some(existing) = ns_cronjobs.get_mut(&cronjob.name) {
                *existing = cronjob;
                return Ok(());
            }
        }
        Err(A3sError::Project(format!(
            "CronJob {} not found in namespace {}",
            cronjob.name, cronjob.namespace
        )))
    }

    /// Delete a CronJob.
    pub async fn delete_cronjob(&self, namespace: &str, name: &str) -> Result<()> {
        let mut cronjobs = self.cronjobs.write().await;
        if let Some(ns_cronjobs) = cronjobs.get_mut(namespace) {
            ns_cronjobs.remove(name);
        }
        Ok(())
    }

    // ==================== CronJob Actual State ====================

    /// Update actual state.
    pub async fn update_actual(&self, actual: CronJobActual) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        let ns_actuals = actuals
            .entry(actual.namespace.clone())
            .or_insert_with(HashMap::new);
        ns_actuals.insert(actual.name.clone(), actual);
        Ok(())
    }

    /// Get actual state.
    pub async fn get_actual(&self, namespace: &str, name: &str) -> Option<CronJobActual> {
        let actuals = self.actuals.read().await;
        actuals.get(namespace).and_then(|ns| ns.get(name).cloned())
    }

    /// Add active job.
    pub async fn add_active_job(&self, namespace: &str, name: &str, job_name: &str) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.active_jobs.push(job_name.to_string());
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("CronJob {} not found", name)))
    }

    /// Remove active job.
    pub async fn remove_active_job(
        &self,
        namespace: &str,
        name: &str,
        job_name: &str,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.active_jobs.retain(|j| j != job_name);
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("CronJob {} not found", name)))
    }

    /// Update last scheduled time.
    pub async fn set_last_scheduled(
        &self,
        namespace: &str,
        name: &str,
        time: DateTime<Utc>,
    ) -> Result<()> {
        let mut actuals = self.actuals.write().await;
        if let Some(ns_actuals) = actuals.get_mut(namespace) {
            if let Some(actual) = ns_actuals.get_mut(name) {
                actual.last_scheduled_time = Some(time);
                return Ok(());
            }
        }
        Err(A3sError::Project(format!("CronJob {} not found", name)))
    }

    /// Check if should run based on schedule.
    pub fn should_run_now(&self, schedule: &CronJobSchedule) -> bool {
        use chrono::Datelike;
        use chrono::Timelike;

        let now = Utc::now();
        let naive_date = now.date_naive();

        // Simple schedule matching
        // In a real implementation, this would use a proper cron parser
        let current_minute = now.minute() as i32;
        let current_hour = now.hour() as i32;
        let current_day_of_month = naive_date.day() as i32;
        let current_month = naive_date.month() as i32;
        // Use format to get weekday (0=Sunday, 6=Saturday)
        let dow_str = now.format("%w").to_string();
        let current_day_of_week = dow_str.parse::<i32>().unwrap_or(0);

        self.matches_cron_field(&schedule.minute, current_minute)
            && self.matches_cron_field(&schedule.hour, current_hour)
            && self.matches_cron_field(&schedule.day_of_month, current_day_of_month)
            && self.matches_cron_field(&schedule.month, current_month)
            && self.matches_cron_field(&schedule.day_of_week, current_day_of_week)
    }

    /// Check if a cron field matches the current time.
    fn matches_cron_field(&self, field: &str, current: i32) -> bool {
        if field == "*" {
            return true;
        }

        // Handle step values (*/5)
        if let Some(step_str) = field.strip_prefix("*/") {
            if let Ok(step) = step_str.parse::<i32>() {
                return current % step == 0;
            }
        }

        // Handle lists (1,2,3)
        if field.contains(',') {
            return field
                .split(',')
                .any(|part| self.matches_cron_field(part.trim(), current));
        }

        // Handle ranges (1-5)
        if field.contains('-') {
            let parts: Vec<&str> = field.split('-').collect();
            if parts.len() == 2 {
                if let (Ok(start), Ok(end)) = (parts[0].parse::<i32>(), parts[1].parse::<i32>()) {
                    return current >= start && current <= end;
                }
            }
        }

        // Handle single value
        if let Ok(value) = field.parse::<i32>() {
            return current == value;
        }

        false
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

    fn test_job_controller() -> JobController {
        JobController::new()
    }

    fn test_cronjob_controller() -> CronJobController {
        CronJobController::new()
    }

    fn test_job() -> JobDesired {
        JobDesired {
            name: "pi".to_string(),
            namespace: "default".to_string(),
            spec: JobSpec {
                completions: Some(3),
                parallelism: Some(1),
                active_deadline_seconds: Some(600),
                backoff_limit: BackoffLimitPolicy::Limited(3),
                retry_policy: JobRetryPolicy::OnFailure,
                template: JobPodTemplate {
                    metadata: JobPodMetadata {
                        labels: {
                            let mut m = HashMap::new();
                            m.insert("app".to_string(), "pi".to_string());
                            m
                        },
                        annotations: HashMap::new(),
                    },
                    spec: JobPodSpec {
                        image: "perl:5.34".to_string(),
                        command: vec![
                            "perl".to_string(),
                            "-Mbignum=bpi".to_string(),
                            "-wle".to_string(),
                            "print bpi(2000)".to_string(),
                        ],
                        args: vec![],
                        env: HashMap::new(),
                        resources: Default::default(),
                        restart_policy: JobRestartPolicy::Never,
                    },
                },
                completion_mode: CompletionMode::NonIndexed,
                ttl_seconds_after_finished: Some(3600),
            },
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    fn test_cronjob() -> CronJobDesired {
        CronJobDesired {
            name: "hello".to_string(),
            namespace: "default".to_string(),
            spec: CronJobSpec {
                schedule: CronJobSchedule {
                    minute: "*/1".to_string(),
                    hour: "*".to_string(),
                    day_of_month: "*".to_string(),
                    month: "*".to_string(),
                    day_of_week: "*".to_string(),
                },
                time_zone: Some("UTC".to_string()),
                starting_deadline_seconds: Some(200),
                job_template: JobTemplate {
                    spec: JobSpec {
                        completions: None,
                        parallelism: None,
                        active_deadline_seconds: Some(600),
                        backoff_limit: BackoffLimitPolicy::default(),
                        retry_policy: JobRetryPolicy::default(),
                        template: JobPodTemplate {
                            metadata: JobPodMetadata {
                                labels: HashMap::new(),
                                annotations: HashMap::new(),
                            },
                            spec: JobPodSpec {
                                image: "busybox:1.28".to_string(),
                                command: vec!["/bin/sh".to_string(), "-c".to_string()],
                                args: vec!["echo Hello from the job; sleep 10".to_string()],
                                env: HashMap::new(),
                                resources: Default::default(),
                                restart_policy: JobRestartPolicy::Never,
                            },
                        },
                        completion_mode: CompletionMode::default(),
                        ttl_seconds_after_finished: None,
                    },
                },
                concurrency_policy: CronJobConcurrencyPolicy::Allow,
                suspend: false,
                successful_jobs_history_limit: Some(3),
                failed_jobs_history_limit: Some(1),
            },
            last_scheduled_time: None,
            active_jobs: vec![],
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    #[tokio::test]
    async fn test_create_job() {
        let controller = test_job_controller();
        let job = test_job();

        controller.create_job(job.clone()).await.unwrap();

        let retrieved = controller.get_job("default", "pi").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().name, "pi");
    }

    #[tokio::test]
    async fn test_job_actual_state() {
        let controller = test_job_controller();
        let job = test_job();

        controller.create_job(job).await.unwrap();

        // Create actual state
        let actual = JobActual {
            name: "pi".to_string(),
            namespace: "default".to_string(),
            start_time: Some(Utc::now()),
            completion_time: None,
            active: 1,
            succeeded: 0,
            failed: 0,
            ready: 1,
            suspended: false,
            conditions: vec![],
        };

        controller.update_actual(actual).await.unwrap();

        // Increment succeeded
        controller
            .increment_succeeded("default", "pi")
            .await
            .unwrap();

        let actual = controller.get_actual("default", "pi").await.unwrap();
        assert_eq!(actual.succeeded, 1);
        assert_eq!(actual.active, 1);
    }

    #[tokio::test]
    async fn test_is_complete() {
        let controller = test_job_controller();
        let job = test_job();

        controller.create_job(job).await.unwrap();

        let actual = JobActual {
            name: "pi".to_string(),
            namespace: "default".to_string(),
            start_time: Some(Utc::now()),
            completion_time: None,
            active: 0,
            succeeded: 3,
            failed: 0,
            ready: 0,
            suspended: false,
            conditions: vec![],
        };

        controller.update_actual(actual).await.unwrap();

        let is_complete = controller.is_complete("default", "pi").await;
        assert!(is_complete);
    }

    #[tokio::test]
    async fn test_cronjob_lifecycle() {
        let controller = test_cronjob_controller();
        let cronjob = test_cronjob();

        controller.create_cronjob(cronjob.clone()).await.unwrap();

        let retrieved = controller.get_cronjob("default", "hello").await;
        assert!(retrieved.is_some());

        // Create actual state
        let actual = CronJobActual {
            name: "hello".to_string(),
            namespace: "default".to_string(),
            last_scheduled_time: Some(Utc::now()),
            active_jobs: vec!["hello-12345".to_string()],
            last_successful_time: None,
        };

        controller.update_actual(actual).await.unwrap();

        // Add another active job
        controller
            .add_active_job("default", "hello", "hello-12346")
            .await
            .unwrap();

        let actual = controller.get_actual("default", "hello").await.unwrap();
        assert_eq!(actual.active_jobs.len(), 2);

        // Remove active job
        controller
            .remove_active_job("default", "hello", "hello-12345")
            .await
            .unwrap();

        let actual = controller.get_actual("default", "hello").await.unwrap();
        assert_eq!(actual.active_jobs.len(), 1);
    }

    #[tokio::test]
    async fn test_cron_schedule_parsing() {
        let schedule = CronJobSchedule::parse("*/5 * * * *").unwrap();
        assert_eq!(schedule.minute, "*/5");
        assert_eq!(schedule.hour, "*");
        assert_eq!(schedule.day_of_month, "*");
        assert_eq!(schedule.month, "*");
        assert_eq!(schedule.day_of_week, "*");

        let formatted = schedule.format();
        assert_eq!(formatted, "*/5 * * * *");
    }

    #[tokio::test]
    async fn test_cron_field_matching() {
        let controller = test_cronjob_controller();

        // Test wildcard
        assert!(controller.matches_cron_field("*", 5));

        // Test step
        assert!(controller.matches_cron_field("*/5", 10));
        assert!(controller.matches_cron_field("*/5", 15));
        assert!(!controller.matches_cron_field("*/5", 13));

        // Test list
        assert!(controller.matches_cron_field("1,2,3", 2));
        assert!(!controller.matches_cron_field("1,2,3", 4));

        // Test range
        assert!(controller.matches_cron_field("1-5", 3));
        assert!(!controller.matches_cron_field("1-5", 6));

        // Test single value
        assert!(controller.matches_cron_field("5", 5));
        assert!(!controller.matches_cron_field("5", 6));
    }

    #[tokio::test]
    async fn test_delete_job() {
        let controller = test_job_controller();
        let job = test_job();

        controller.create_job(job).await.unwrap();

        let retrieved = controller.get_job("default", "pi").await;
        assert!(retrieved.is_some());

        controller.delete_job("default", "pi").await.unwrap();

        let retrieved = controller.get_job("default", "pi").await;
        assert!(retrieved.is_none());
    }
}
