//! Cron task scheduler
//!
//! Provides background task scheduling and execution management.

use crate::parser::CronExpression;
use crate::store::{CronStore, FileCronStore};
use crate::types::{CronError, CronJob, JobExecution, JobStatus, Result};
use chrono::Utc;
use std::path::Path;
use std::sync::Arc;
use tokio::process::Command;
use tokio::sync::{broadcast, RwLock};
use tokio::time::{interval, Duration};

/// Scheduler events for monitoring
#[derive(Debug, Clone)]
pub enum SchedulerEvent {
    /// Scheduler started
    Started,
    /// Scheduler stopped
    Stopped,
    /// Job started execution
    JobStarted {
        job_id: String,
        execution_id: String,
    },
    /// Job completed successfully
    JobCompleted {
        job_id: String,
        execution_id: String,
    },
    /// Job failed
    JobFailed {
        job_id: String,
        execution_id: String,
        error: String,
    },
    /// Job timed out
    JobTimeout {
        job_id: String,
        execution_id: String,
    },
}

/// Cron manager for job scheduling and execution
pub struct CronManager {
    /// Storage backend
    store: Arc<dyn CronStore>,
    /// Event broadcaster
    event_tx: broadcast::Sender<SchedulerEvent>,
    /// Scheduler running flag
    running: Arc<RwLock<bool>>,
    /// Workspace directory
    workspace: String,
}

impl CronManager {
    /// Create a new cron manager with file-based storage
    pub async fn new<P: AsRef<Path>>(workspace: P) -> Result<Self> {
        let workspace_str = workspace.as_ref().to_string_lossy().to_string();
        let store = Arc::new(FileCronStore::new(&workspace_str).await?);
        let (event_tx, _) = broadcast::channel(100);

        Ok(Self {
            store,
            event_tx,
            running: Arc::new(RwLock::new(false)),
            workspace: workspace_str,
        })
    }

    /// Create a cron manager with a custom store
    pub fn with_store(store: Arc<dyn CronStore>, workspace: String) -> Self {
        let (event_tx, _) = broadcast::channel(100);
        Self {
            store,
            event_tx,
            running: Arc::new(RwLock::new(false)),
            workspace,
        }
    }

    /// Subscribe to scheduler events
    pub fn subscribe(&self) -> broadcast::Receiver<SchedulerEvent> {
        self.event_tx.subscribe()
    }

    /// Add a new cron job
    pub async fn add_job(&self, name: &str, schedule: &str, command: &str) -> Result<CronJob> {
        // Validate schedule
        let expr = CronExpression::parse(schedule)?;

        // Check for duplicate name
        if self.store.find_job_by_name(name).await?.is_some() {
            return Err(CronError::JobExists(name.to_string()));
        }

        // Create job
        let mut job = CronJob::new(name, schedule, command);
        job.next_run = expr.next_after(Utc::now());
        job.working_dir = Some(self.workspace.clone());

        // Save
        self.store.save_job(&job).await?;

        tracing::info!("Added cron job: {} ({})", job.name, job.id);
        Ok(job)
    }

    /// Get a job by ID
    pub async fn get_job(&self, id: &str) -> Result<Option<CronJob>> {
        self.store.load_job(id).await
    }

    /// Get a job by name
    pub async fn get_job_by_name(&self, name: &str) -> Result<Option<CronJob>> {
        self.store.find_job_by_name(name).await
    }

    /// List all jobs
    pub async fn list_jobs(&self) -> Result<Vec<CronJob>> {
        self.store.list_jobs().await
    }

    /// Update a job
    pub async fn update_job(
        &self,
        id: &str,
        schedule: Option<&str>,
        command: Option<&str>,
        timeout_ms: Option<u64>,
    ) -> Result<CronJob> {
        let mut job = self
            .store
            .load_job(id)
            .await?
            .ok_or_else(|| CronError::JobNotFound(id.to_string()))?;

        if let Some(schedule) = schedule {
            let expr = CronExpression::parse(schedule)?;
            job.schedule = schedule.to_string();
            job.next_run = expr.next_after(Utc::now());
        }

        if let Some(command) = command {
            job.command = command.to_string();
        }

        if let Some(timeout) = timeout_ms {
            job.timeout_ms = timeout;
        }

        job.updated_at = Utc::now();
        self.store.save_job(&job).await?;

        tracing::info!("Updated cron job: {} ({})", job.name, job.id);
        Ok(job)
    }

    /// Pause a job
    pub async fn pause_job(&self, id: &str) -> Result<CronJob> {
        let mut job = self
            .store
            .load_job(id)
            .await?
            .ok_or_else(|| CronError::JobNotFound(id.to_string()))?;

        job.status = JobStatus::Paused;
        job.updated_at = Utc::now();
        self.store.save_job(&job).await?;

        tracing::info!("Paused cron job: {} ({})", job.name, job.id);
        Ok(job)
    }

    /// Resume a paused job
    pub async fn resume_job(&self, id: &str) -> Result<CronJob> {
        let mut job = self
            .store
            .load_job(id)
            .await?
            .ok_or_else(|| CronError::JobNotFound(id.to_string()))?;

        job.status = JobStatus::Active;
        job.updated_at = Utc::now();

        // Recalculate next run
        if let Ok(expr) = CronExpression::parse(&job.schedule) {
            job.next_run = expr.next_after(Utc::now());
        }

        self.store.save_job(&job).await?;

        tracing::info!("Resumed cron job: {} ({})", job.name, job.id);
        Ok(job)
    }

    /// Remove a job
    pub async fn remove_job(&self, id: &str) -> Result<()> {
        let job = self
            .store
            .load_job(id)
            .await?
            .ok_or_else(|| CronError::JobNotFound(id.to_string()))?;

        self.store.delete_job(id).await?;

        tracing::info!("Removed cron job: {} ({})", job.name, job.id);
        Ok(())
    }

    /// Get execution history for a job
    pub async fn get_history(&self, job_id: &str, limit: usize) -> Result<Vec<JobExecution>> {
        self.store.load_executions(job_id, limit).await
    }

    /// Manually run a job
    pub async fn run_job(&self, id: &str) -> Result<JobExecution> {
        let job = self
            .store
            .load_job(id)
            .await?
            .ok_or_else(|| CronError::JobNotFound(id.to_string()))?;

        self.execute_job(&job).await
    }

    /// Execute a job
    async fn execute_job(&self, job: &CronJob) -> Result<JobExecution> {
        let mut execution = JobExecution::new(&job.id);

        // Emit start event
        let _ = self.event_tx.send(SchedulerEvent::JobStarted {
            job_id: job.id.clone(),
            execution_id: execution.id.clone(),
        });

        // Update job status to running
        let mut running_job = job.clone();
        running_job.status = JobStatus::Running;
        self.store.save_job(&running_job).await?;

        // Execute command with timeout
        let timeout = Duration::from_millis(job.timeout_ms);
        let working_dir = job.working_dir.as_deref().unwrap_or(&self.workspace);

        let result = tokio::time::timeout(timeout, async {
            let output = Command::new("sh")
                .arg("-c")
                .arg(&job.command)
                .current_dir(working_dir)
                .envs(job.env.iter().map(|(k, v)| (k.as_str(), v.as_str())))
                .output()
                .await;

            output
        })
        .await;

        // Process result
        execution = match result {
            Ok(Ok(output)) => {
                let stdout = String::from_utf8_lossy(&output.stdout).to_string();
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let exit_code = output.status.code().unwrap_or(-1);

                execution.complete(exit_code, stdout, stderr)
            }
            Ok(Err(e)) => execution.fail(format!("Failed to execute command: {}", e)),
            Err(_) => {
                let _ = self.event_tx.send(SchedulerEvent::JobTimeout {
                    job_id: job.id.clone(),
                    execution_id: execution.id.clone(),
                });
                execution.timeout()
            }
        };

        // Save execution
        self.store.save_execution(&execution).await?;

        // Update job statistics
        let mut updated_job = job.clone();
        updated_job.status = JobStatus::Active;
        updated_job.last_run = Some(execution.started_at);
        updated_job.updated_at = Utc::now();

        if execution.status == crate::types::ExecutionStatus::Success {
            updated_job.run_count += 1;
            let _ = self.event_tx.send(SchedulerEvent::JobCompleted {
                job_id: job.id.clone(),
                execution_id: execution.id.clone(),
            });
        } else {
            updated_job.fail_count += 1;
            let _ = self.event_tx.send(SchedulerEvent::JobFailed {
                job_id: job.id.clone(),
                execution_id: execution.id.clone(),
                error: execution.error.clone().unwrap_or_default(),
            });
        }

        // Calculate next run
        if let Ok(expr) = CronExpression::parse(&updated_job.schedule) {
            updated_job.next_run = expr.next_after(Utc::now());
        }

        self.store.save_job(&updated_job).await?;

        Ok(execution)
    }

    /// Start the scheduler background task
    pub async fn start(&self) -> Result<()> {
        let mut running = self.running.write().await;
        if *running {
            return Ok(());
        }
        *running = true;
        drop(running);

        let _ = self.event_tx.send(SchedulerEvent::Started);
        tracing::info!("Cron scheduler started");

        let store = self.store.clone();
        let event_tx = self.event_tx.clone();
        let running = self.running.clone();
        let workspace = self.workspace.clone();

        tokio::spawn(async move {
            let mut ticker = interval(Duration::from_secs(60));

            loop {
                ticker.tick().await;

                // Check if still running
                if !*running.read().await {
                    break;
                }

                // Get all active jobs
                let jobs = match store.list_jobs().await {
                    Ok(jobs) => jobs,
                    Err(e) => {
                        tracing::error!("Failed to list jobs: {}", e);
                        continue;
                    }
                };

                let now = Utc::now();

                for job in jobs {
                    // Skip non-active jobs
                    if job.status != JobStatus::Active {
                        continue;
                    }

                    // Check if job should run
                    if let Some(next_run) = job.next_run {
                        if next_run <= now {
                            // Create a temporary manager for execution
                            let manager = CronManager {
                                store: store.clone(),
                                event_tx: event_tx.clone(),
                                running: running.clone(),
                                workspace: workspace.clone(),
                            };

                            if let Err(e) = manager.execute_job(&job).await {
                                tracing::error!("Failed to execute job {}: {}", job.id, e);
                            }
                        }
                    }
                }
            }

            let _ = event_tx.send(SchedulerEvent::Stopped);
            tracing::info!("Cron scheduler stopped");
        });

        Ok(())
    }

    /// Stop the scheduler
    pub async fn stop(&self) {
        let mut running = self.running.write().await;
        *running = false;
    }

    /// Check if scheduler is running
    pub async fn is_running(&self) -> bool {
        *self.running.read().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::store::MemoryCronStore;

    fn create_test_manager() -> CronManager {
        let store = Arc::new(MemoryCronStore::new());
        CronManager::with_store(store, "/tmp".to_string())
    }

    #[tokio::test]
    async fn test_add_job() {
        let manager = create_test_manager();

        let job = manager
            .add_job("test-job", "*/5 * * * *", "echo hello")
            .await
            .unwrap();

        assert_eq!(job.name, "test-job");
        assert_eq!(job.schedule, "*/5 * * * *");
        assert_eq!(job.command, "echo hello");
        assert!(job.next_run.is_some());
    }

    #[tokio::test]
    async fn test_add_duplicate_name() {
        let manager = create_test_manager();

        manager
            .add_job("unique", "* * * * *", "echo 1")
            .await
            .unwrap();

        let result = manager.add_job("unique", "* * * * *", "echo 2").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_add_invalid_schedule() {
        let manager = create_test_manager();

        let result = manager.add_job("bad", "invalid", "echo").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_get_job() {
        let manager = create_test_manager();

        let job = manager
            .add_job("findme", "* * * * *", "echo")
            .await
            .unwrap();

        let found = manager.get_job(&job.id).await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().name, "findme");
    }

    #[tokio::test]
    async fn test_list_jobs() {
        let manager = create_test_manager();

        for i in 1..=3 {
            manager
                .add_job(&format!("job-{}", i), "* * * * *", "echo")
                .await
                .unwrap();
        }

        let jobs = manager.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 3);
    }

    #[tokio::test]
    async fn test_update_job() {
        let manager = create_test_manager();

        let job = manager
            .add_job("updatable", "* * * * *", "echo v1")
            .await
            .unwrap();

        let updated = manager
            .update_job(&job.id, Some("0 * * * *"), Some("echo v2"), Some(30000))
            .await
            .unwrap();

        assert_eq!(updated.schedule, "0 * * * *");
        assert_eq!(updated.command, "echo v2");
        assert_eq!(updated.timeout_ms, 30000);
    }

    #[tokio::test]
    async fn test_pause_resume() {
        let manager = create_test_manager();

        let job = manager
            .add_job("pausable", "* * * * *", "echo")
            .await
            .unwrap();

        // Pause
        let paused = manager.pause_job(&job.id).await.unwrap();
        assert_eq!(paused.status, JobStatus::Paused);

        // Resume
        let resumed = manager.resume_job(&job.id).await.unwrap();
        assert_eq!(resumed.status, JobStatus::Active);
    }

    #[tokio::test]
    async fn test_remove_job() {
        let manager = create_test_manager();

        let job = manager
            .add_job("removable", "* * * * *", "echo")
            .await
            .unwrap();

        manager.remove_job(&job.id).await.unwrap();

        let found = manager.get_job(&job.id).await.unwrap();
        assert!(found.is_none());
    }

    #[tokio::test]
    async fn test_run_job() {
        let manager = create_test_manager();

        let job = manager
            .add_job("runnable", "* * * * *", "echo hello")
            .await
            .unwrap();

        let execution = manager.run_job(&job.id).await.unwrap();
        assert!(execution.stdout.contains("hello"));
    }

    #[tokio::test]
    async fn test_run_job_failure() {
        let manager = create_test_manager();

        let job = manager
            .add_job("failing", "* * * * *", "exit 1")
            .await
            .unwrap();

        let execution = manager.run_job(&job.id).await.unwrap();
        assert_eq!(execution.status, crate::types::ExecutionStatus::Failed);
    }

    #[tokio::test]
    async fn test_get_history() {
        let manager = create_test_manager();

        let job = manager
            .add_job("historical", "* * * * *", "echo test")
            .await
            .unwrap();

        // Run job multiple times
        for _ in 0..3 {
            manager.run_job(&job.id).await.unwrap();
        }

        let history = manager.get_history(&job.id, 10).await.unwrap();
        assert_eq!(history.len(), 3);
    }

    #[tokio::test]
    async fn test_event_subscription() {
        let manager = create_test_manager();
        let mut rx = manager.subscribe();

        let job = manager
            .add_job("evented", "* * * * *", "echo test")
            .await
            .unwrap();

        // Run job
        manager.run_job(&job.id).await.unwrap();

        // Check events
        let event = rx.try_recv().unwrap();
        match event {
            SchedulerEvent::JobStarted { job_id, .. } => {
                assert_eq!(job_id, job.id);
            }
            _ => panic!("Expected JobStarted event"),
        }
    }
}
