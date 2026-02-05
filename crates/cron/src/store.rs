//! Cron job persistence layer
//!
//! Provides pluggable storage backends for cron jobs and execution history.

use crate::types::{CronJob, JobExecution, Result};
use async_trait::async_trait;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::sync::RwLock;

/// Cron storage trait
#[async_trait]
pub trait CronStore: Send + Sync {
    /// Save a job
    async fn save_job(&self, job: &CronJob) -> Result<()>;

    /// Load a job by ID
    async fn load_job(&self, id: &str) -> Result<Option<CronJob>>;

    /// Delete a job
    async fn delete_job(&self, id: &str) -> Result<()>;

    /// List all jobs
    async fn list_jobs(&self) -> Result<Vec<CronJob>>;

    /// Check if a job exists
    async fn job_exists(&self, id: &str) -> Result<bool>;

    /// Find a job by name
    async fn find_job_by_name(&self, name: &str) -> Result<Option<CronJob>>;

    /// Save an execution record
    async fn save_execution(&self, execution: &JobExecution) -> Result<()>;

    /// Load execution history for a job
    async fn load_executions(&self, job_id: &str, limit: usize) -> Result<Vec<JobExecution>>;

    /// Delete all executions for a job
    async fn delete_executions(&self, job_id: &str) -> Result<()>;
}

// ============================================================================
// File-based Store
// ============================================================================

/// File-based cron store
///
/// Stores jobs and executions as JSON files:
/// ```text
/// .a3s/cron/
///   jobs.json           # All job definitions
///   history/
///     {job-id}/
///       {timestamp}.json  # Execution records
/// ```
pub struct FileCronStore {
    /// Jobs file path
    jobs_file: PathBuf,
    /// History directory path
    history_dir: PathBuf,
}

impl FileCronStore {
    /// Create a new file-based store
    pub async fn new<P: AsRef<Path>>(workspace: P) -> Result<Self> {
        let base_dir = workspace.as_ref().join(".a3s").join("cron");
        let jobs_file = base_dir.join("jobs.json");
        let history_dir = base_dir.join("history");

        // Create directories
        fs::create_dir_all(&base_dir).await?;
        fs::create_dir_all(&history_dir).await?;

        // Initialize empty jobs file if it doesn't exist
        if !jobs_file.exists() {
            let empty: Vec<CronJob> = Vec::new();
            let json = serde_json::to_string_pretty(&empty)?;
            fs::write(&jobs_file, json).await?;
        }

        Ok(Self {
            jobs_file,
            history_dir,
        })
    }

    /// Load all jobs from file
    async fn load_all_jobs(&self) -> Result<Vec<CronJob>> {
        let content = fs::read_to_string(&self.jobs_file).await?;
        let jobs: Vec<CronJob> = serde_json::from_str(&content)?;
        Ok(jobs)
    }

    /// Save all jobs to file
    async fn save_all_jobs(&self, jobs: &[CronJob]) -> Result<()> {
        let json = serde_json::to_string_pretty(jobs)?;

        // Write atomically
        let temp_path = self.jobs_file.with_extension("json.tmp");
        let mut file = fs::File::create(&temp_path).await?;
        file.write_all(json.as_bytes()).await?;
        file.sync_all().await?;
        fs::rename(&temp_path, &self.jobs_file).await?;

        Ok(())
    }

    /// Get the history directory for a job
    fn job_history_dir(&self, job_id: &str) -> PathBuf {
        // Sanitize job ID
        let safe_id = job_id.replace(['/', '\\'], "_").replace("..", "_");
        self.history_dir.join(safe_id)
    }
}

#[async_trait]
impl CronStore for FileCronStore {
    async fn save_job(&self, job: &CronJob) -> Result<()> {
        let mut jobs = self.load_all_jobs().await?;

        // Update or insert
        if let Some(existing) = jobs.iter_mut().find(|j| j.id == job.id) {
            *existing = job.clone();
        } else {
            jobs.push(job.clone());
        }

        self.save_all_jobs(&jobs).await
    }

    async fn load_job(&self, id: &str) -> Result<Option<CronJob>> {
        let jobs = self.load_all_jobs().await?;
        Ok(jobs.into_iter().find(|j| j.id == id))
    }

    async fn delete_job(&self, id: &str) -> Result<()> {
        let mut jobs = self.load_all_jobs().await?;
        jobs.retain(|j| j.id != id);
        self.save_all_jobs(&jobs).await?;

        // Also delete execution history
        self.delete_executions(id).await?;

        Ok(())
    }

    async fn list_jobs(&self) -> Result<Vec<CronJob>> {
        self.load_all_jobs().await
    }

    async fn job_exists(&self, id: &str) -> Result<bool> {
        let jobs = self.load_all_jobs().await?;
        Ok(jobs.iter().any(|j| j.id == id))
    }

    async fn find_job_by_name(&self, name: &str) -> Result<Option<CronJob>> {
        let jobs = self.load_all_jobs().await?;
        Ok(jobs.into_iter().find(|j| j.name == name))
    }

    async fn save_execution(&self, execution: &JobExecution) -> Result<()> {
        let job_dir = self.job_history_dir(&execution.job_id);
        fs::create_dir_all(&job_dir).await?;

        let filename = format!("{}.json", execution.started_at.timestamp_millis());
        let path = job_dir.join(filename);

        let json = serde_json::to_string_pretty(execution)?;
        fs::write(&path, json).await?;

        Ok(())
    }

    async fn load_executions(&self, job_id: &str, limit: usize) -> Result<Vec<JobExecution>> {
        let job_dir = self.job_history_dir(job_id);

        if !job_dir.exists() {
            return Ok(Vec::new());
        }

        let mut executions = Vec::new();
        let mut entries = fs::read_dir(&job_dir).await?;

        while let Some(entry) = entries.next_entry().await? {
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                let content = fs::read_to_string(&path).await?;
                if let Ok(exec) = serde_json::from_str::<JobExecution>(&content) {
                    executions.push(exec);
                }
            }
        }

        // Sort by start time descending (most recent first)
        executions.sort_by(|a, b| b.started_at.cmp(&a.started_at));

        // Limit results
        executions.truncate(limit);

        Ok(executions)
    }

    async fn delete_executions(&self, job_id: &str) -> Result<()> {
        let job_dir = self.job_history_dir(job_id);

        if job_dir.exists() {
            fs::remove_dir_all(&job_dir).await?;
        }

        Ok(())
    }
}

// ============================================================================
// In-Memory Store (for testing)
// ============================================================================

/// In-memory cron store for testing
pub struct MemoryCronStore {
    jobs: RwLock<HashMap<String, CronJob>>,
    executions: RwLock<HashMap<String, Vec<JobExecution>>>,
}

impl MemoryCronStore {
    /// Create a new in-memory store
    pub fn new() -> Self {
        Self {
            jobs: RwLock::new(HashMap::new()),
            executions: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for MemoryCronStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CronStore for MemoryCronStore {
    async fn save_job(&self, job: &CronJob) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        jobs.insert(job.id.clone(), job.clone());
        Ok(())
    }

    async fn load_job(&self, id: &str) -> Result<Option<CronJob>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.get(id).cloned())
    }

    async fn delete_job(&self, id: &str) -> Result<()> {
        let mut jobs = self.jobs.write().await;
        jobs.remove(id);

        let mut executions = self.executions.write().await;
        executions.remove(id);

        Ok(())
    }

    async fn list_jobs(&self) -> Result<Vec<CronJob>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.values().cloned().collect())
    }

    async fn job_exists(&self, id: &str) -> Result<bool> {
        let jobs = self.jobs.read().await;
        Ok(jobs.contains_key(id))
    }

    async fn find_job_by_name(&self, name: &str) -> Result<Option<CronJob>> {
        let jobs = self.jobs.read().await;
        Ok(jobs.values().find(|j| j.name == name).cloned())
    }

    async fn save_execution(&self, execution: &JobExecution) -> Result<()> {
        let mut executions = self.executions.write().await;
        executions
            .entry(execution.job_id.clone())
            .or_default()
            .push(execution.clone());
        Ok(())
    }

    async fn load_executions(&self, job_id: &str, limit: usize) -> Result<Vec<JobExecution>> {
        let executions = self.executions.read().await;
        let mut result = executions.get(job_id).cloned().unwrap_or_default();
        result.sort_by(|a, b| b.started_at.cmp(&a.started_at));
        result.truncate(limit);
        Ok(result)
    }

    async fn delete_executions(&self, job_id: &str) -> Result<()> {
        let mut executions = self.executions.write().await;
        executions.remove(job_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    // ========================================================================
    // MemoryCronStore Tests
    // ========================================================================

    #[tokio::test]
    async fn test_memory_store_save_and_load() {
        let store = MemoryCronStore::new();
        let job = CronJob::new("test", "* * * * *", "echo hello");

        store.save_job(&job).await.unwrap();

        let loaded = store.load_job(&job.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_memory_store_delete() {
        let store = MemoryCronStore::new();
        let job = CronJob::new("test", "* * * * *", "echo hello");

        store.save_job(&job).await.unwrap();
        assert!(store.job_exists(&job.id).await.unwrap());

        store.delete_job(&job.id).await.unwrap();
        assert!(!store.job_exists(&job.id).await.unwrap());
    }

    #[tokio::test]
    async fn test_memory_store_list() {
        let store = MemoryCronStore::new();

        for i in 1..=3 {
            let job = CronJob::new(format!("job-{}", i), "* * * * *", "echo");
            store.save_job(&job).await.unwrap();
        }

        let jobs = store.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 3);
    }

    #[tokio::test]
    async fn test_memory_store_find_by_name() {
        let store = MemoryCronStore::new();
        let job = CronJob::new("unique-name", "* * * * *", "echo");
        store.save_job(&job).await.unwrap();

        let found = store.find_job_by_name("unique-name").await.unwrap();
        assert!(found.is_some());
        assert_eq!(found.unwrap().id, job.id);

        let not_found = store.find_job_by_name("nonexistent").await.unwrap();
        assert!(not_found.is_none());
    }

    #[tokio::test]
    async fn test_memory_store_executions() {
        let store = MemoryCronStore::new();
        let job = CronJob::new("test", "* * * * *", "echo");
        store.save_job(&job).await.unwrap();

        // Save some executions
        for _ in 0..5 {
            let exec = JobExecution::new(&job.id);
            store.save_execution(&exec).await.unwrap();
        }

        let executions = store.load_executions(&job.id, 10).await.unwrap();
        assert_eq!(executions.len(), 5);

        // Test limit
        let limited = store.load_executions(&job.id, 2).await.unwrap();
        assert_eq!(limited.len(), 2);
    }

    // ========================================================================
    // FileCronStore Tests
    // ========================================================================

    #[tokio::test]
    async fn test_file_store_save_and_load() {
        let dir = tempdir().unwrap();
        let store = FileCronStore::new(dir.path()).await.unwrap();

        let job = CronJob::new("test", "* * * * *", "echo hello");
        store.save_job(&job).await.unwrap();

        let loaded = store.load_job(&job.id).await.unwrap();
        assert!(loaded.is_some());
        assert_eq!(loaded.unwrap().name, "test");
    }

    #[tokio::test]
    async fn test_file_store_persistence() {
        let dir = tempdir().unwrap();

        // Create store and save job
        {
            let store = FileCronStore::new(dir.path()).await.unwrap();
            let job = CronJob::new("persistent", "0 * * * *", "backup.sh");
            store.save_job(&job).await.unwrap();
        }

        // Create new store instance and verify job persists
        {
            let store = FileCronStore::new(dir.path()).await.unwrap();
            let jobs = store.list_jobs().await.unwrap();
            assert_eq!(jobs.len(), 1);
            assert_eq!(jobs[0].name, "persistent");
        }
    }

    #[tokio::test]
    async fn test_file_store_delete() {
        let dir = tempdir().unwrap();
        let store = FileCronStore::new(dir.path()).await.unwrap();

        let job = CronJob::new("to-delete", "* * * * *", "echo");
        store.save_job(&job).await.unwrap();

        store.delete_job(&job.id).await.unwrap();

        let loaded = store.load_job(&job.id).await.unwrap();
        assert!(loaded.is_none());
    }

    #[tokio::test]
    async fn test_file_store_update() {
        let dir = tempdir().unwrap();
        let store = FileCronStore::new(dir.path()).await.unwrap();

        let mut job = CronJob::new("updatable", "* * * * *", "echo v1");
        store.save_job(&job).await.unwrap();

        // Update the job
        job.command = "echo v2".to_string();
        store.save_job(&job).await.unwrap();

        let loaded = store.load_job(&job.id).await.unwrap().unwrap();
        assert_eq!(loaded.command, "echo v2");

        // Should still be only one job
        let jobs = store.list_jobs().await.unwrap();
        assert_eq!(jobs.len(), 1);
    }

    #[tokio::test]
    async fn test_file_store_executions() {
        let dir = tempdir().unwrap();
        let store = FileCronStore::new(dir.path()).await.unwrap();

        let job = CronJob::new("test", "* * * * *", "echo");
        store.save_job(&job).await.unwrap();

        // Save executions
        for _ in 0..3 {
            let exec = JobExecution::new(&job.id);
            store.save_execution(&exec).await.unwrap();
            tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
        }

        let executions = store.load_executions(&job.id, 10).await.unwrap();
        assert_eq!(executions.len(), 3);

        // Delete job should also delete executions
        store.delete_job(&job.id).await.unwrap();
        let executions = store.load_executions(&job.id, 10).await.unwrap();
        assert!(executions.is_empty());
    }
}
