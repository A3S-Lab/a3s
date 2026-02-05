//! Core types for the cron library

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

/// Result type alias for cron operations
pub type Result<T> = std::result::Result<T, CronError>;

/// Cron library errors
#[derive(Debug, Error)]
pub enum CronError {
    /// Invalid cron expression
    #[error("Invalid cron expression: {0}")]
    InvalidExpression(String),

    /// Job not found
    #[error("Job not found: {0}")]
    JobNotFound(String),

    /// Job already exists
    #[error("Job already exists: {0}")]
    JobExists(String),

    /// Storage error
    #[error("Storage error: {0}")]
    Storage(String),

    /// Execution error
    #[error("Execution error: {0}")]
    Execution(String),

    /// Timeout error
    #[error("Job execution timed out after {0}ms")]
    Timeout(u64),

    /// I/O error
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
}

/// Job status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum JobStatus {
    /// Job is active and will run on schedule
    Active,
    /// Job is paused and will not run
    Paused,
    /// Job is currently running
    Running,
}

impl std::fmt::Display for JobStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            JobStatus::Active => write!(f, "active"),
            JobStatus::Paused => write!(f, "paused"),
            JobStatus::Running => write!(f, "running"),
        }
    }
}

/// A cron job definition
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CronJob {
    /// Unique job identifier
    pub id: String,

    /// Human-readable job name
    pub name: String,

    /// Cron schedule expression (5 fields: min hour day month weekday)
    pub schedule: String,

    /// Command to execute
    pub command: String,

    /// Current job status
    pub status: JobStatus,

    /// Execution timeout in milliseconds (default: 60000)
    pub timeout_ms: u64,

    /// Creation timestamp
    pub created_at: DateTime<Utc>,

    /// Last update timestamp
    pub updated_at: DateTime<Utc>,

    /// Last execution timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_run: Option<DateTime<Utc>>,

    /// Next scheduled run timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_run: Option<DateTime<Utc>>,

    /// Total successful run count
    pub run_count: u64,

    /// Total failed run count
    pub fail_count: u64,

    /// Working directory for command execution
    #[serde(skip_serializing_if = "Option::is_none")]
    pub working_dir: Option<String>,

    /// Environment variables for command execution
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub env: Vec<(String, String)>,
}

impl CronJob {
    /// Create a new cron job
    pub fn new(
        name: impl Into<String>,
        schedule: impl Into<String>,
        command: impl Into<String>,
    ) -> Self {
        let now = Utc::now();
        Self {
            id: Uuid::new_v4().to_string(),
            name: name.into(),
            schedule: schedule.into(),
            command: command.into(),
            status: JobStatus::Active,
            timeout_ms: 60_000,
            created_at: now,
            updated_at: now,
            last_run: None,
            next_run: None,
            run_count: 0,
            fail_count: 0,
            working_dir: None,
            env: Vec::new(),
        }
    }

    /// Set the timeout in milliseconds
    pub fn with_timeout(mut self, timeout_ms: u64) -> Self {
        self.timeout_ms = timeout_ms;
        self
    }

    /// Set the working directory
    pub fn with_working_dir(mut self, dir: impl Into<String>) -> Self {
        self.working_dir = Some(dir.into());
        self
    }

    /// Add an environment variable
    pub fn with_env(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.env.push((key.into(), value.into()));
        self
    }

    /// Check if the job is active
    pub fn is_active(&self) -> bool {
        self.status == JobStatus::Active
    }

    /// Check if the job is paused
    pub fn is_paused(&self) -> bool {
        self.status == JobStatus::Paused
    }

    /// Check if the job is running
    pub fn is_running(&self) -> bool {
        self.status == JobStatus::Running
    }
}

/// Execution result status
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExecutionStatus {
    /// Execution succeeded
    Success,
    /// Execution failed
    Failed,
    /// Execution timed out
    Timeout,
    /// Execution was cancelled
    Cancelled,
}

impl std::fmt::Display for ExecutionStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ExecutionStatus::Success => write!(f, "success"),
            ExecutionStatus::Failed => write!(f, "failed"),
            ExecutionStatus::Timeout => write!(f, "timeout"),
            ExecutionStatus::Cancelled => write!(f, "cancelled"),
        }
    }
}

/// A job execution record
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JobExecution {
    /// Execution ID
    pub id: String,

    /// Job ID
    pub job_id: String,

    /// Execution status
    pub status: ExecutionStatus,

    /// Start timestamp
    pub started_at: DateTime<Utc>,

    /// End timestamp
    #[serde(skip_serializing_if = "Option::is_none")]
    pub ended_at: Option<DateTime<Utc>>,

    /// Duration in milliseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_ms: Option<u64>,

    /// Exit code (if available)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,

    /// Standard output (truncated if too long)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stdout: String,

    /// Standard error (truncated if too long)
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub stderr: String,

    /// Error message (if failed)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

impl JobExecution {
    /// Create a new execution record
    pub fn new(job_id: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            job_id: job_id.into(),
            status: ExecutionStatus::Success,
            started_at: Utc::now(),
            ended_at: None,
            duration_ms: None,
            exit_code: None,
            stdout: String::new(),
            stderr: String::new(),
            error: None,
        }
    }

    /// Mark execution as completed
    pub fn complete(mut self, exit_code: i32, stdout: String, stderr: String) -> Self {
        let ended_at = Utc::now();
        self.ended_at = Some(ended_at);
        self.duration_ms = Some((ended_at - self.started_at).num_milliseconds() as u64);
        self.exit_code = Some(exit_code);
        self.stdout = truncate_output(stdout, 10_000);
        self.stderr = truncate_output(stderr, 10_000);
        self.status = if exit_code == 0 {
            ExecutionStatus::Success
        } else {
            ExecutionStatus::Failed
        };
        self
    }

    /// Mark execution as failed
    pub fn fail(mut self, error: impl Into<String>) -> Self {
        let ended_at = Utc::now();
        self.ended_at = Some(ended_at);
        self.duration_ms = Some((ended_at - self.started_at).num_milliseconds() as u64);
        self.status = ExecutionStatus::Failed;
        self.error = Some(error.into());
        self
    }

    /// Mark execution as timed out
    pub fn timeout(mut self) -> Self {
        let ended_at = Utc::now();
        self.ended_at = Some(ended_at);
        self.duration_ms = Some((ended_at - self.started_at).num_milliseconds() as u64);
        self.status = ExecutionStatus::Timeout;
        self.error = Some("Execution timed out".to_string());
        self
    }

    /// Mark execution as cancelled
    pub fn cancel(mut self) -> Self {
        let ended_at = Utc::now();
        self.ended_at = Some(ended_at);
        self.duration_ms = Some((ended_at - self.started_at).num_milliseconds() as u64);
        self.status = ExecutionStatus::Cancelled;
        self.error = Some("Execution cancelled".to_string());
        self
    }
}

/// Truncate output to a maximum length
fn truncate_output(s: String, max_len: usize) -> String {
    if s.len() <= max_len {
        s
    } else {
        format!("{}...[truncated]", &s[..max_len])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cron_job_new() {
        let job = CronJob::new("test-job", "*/5 * * * *", "echo hello");
        assert_eq!(job.name, "test-job");
        assert_eq!(job.schedule, "*/5 * * * *");
        assert_eq!(job.command, "echo hello");
        assert_eq!(job.status, JobStatus::Active);
        assert_eq!(job.timeout_ms, 60_000);
        assert!(job.is_active());
    }

    #[test]
    fn test_cron_job_builder() {
        let job = CronJob::new("test", "* * * * *", "cmd")
            .with_timeout(30_000)
            .with_working_dir("/tmp")
            .with_env("KEY", "VALUE");

        assert_eq!(job.timeout_ms, 30_000);
        assert_eq!(job.working_dir, Some("/tmp".to_string()));
        assert_eq!(job.env, vec![("KEY".to_string(), "VALUE".to_string())]);
    }

    #[test]
    fn test_job_status_display() {
        assert_eq!(JobStatus::Active.to_string(), "active");
        assert_eq!(JobStatus::Paused.to_string(), "paused");
        assert_eq!(JobStatus::Running.to_string(), "running");
    }

    #[test]
    fn test_job_execution_complete() {
        let exec = JobExecution::new("job-1").complete(0, "output".to_string(), "".to_string());

        assert_eq!(exec.status, ExecutionStatus::Success);
        assert_eq!(exec.exit_code, Some(0));
        assert!(exec.ended_at.is_some());
    }

    #[test]
    fn test_job_execution_fail() {
        let exec = JobExecution::new("job-1").fail("Something went wrong");

        assert_eq!(exec.status, ExecutionStatus::Failed);
        assert_eq!(exec.error, Some("Something went wrong".to_string()));
    }

    #[test]
    fn test_truncate_output() {
        let short = "hello".to_string();
        assert_eq!(truncate_output(short.clone(), 100), short);

        let long = "a".repeat(200);
        let truncated = truncate_output(long, 100);
        assert!(truncated.ends_with("...[truncated]"));
        assert!(truncated.len() < 200);
    }
}
