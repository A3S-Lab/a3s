//! A3S Cron - Task scheduling library
//!
//! Provides cron-based task scheduling with:
//! - Standard 5-field cron syntax parsing
//! - Natural language schedule parsing (English & Chinese)
//! - Task persistence and monitoring
//! - CRUD operations for job management
//!
//! ## Quick Start
//!
//! ```ignore
//! use a3s_cron::{CronManager, CronJob, JobStatus, parse_natural};
//!
//! // Create a manager with file-based storage
//! let manager = CronManager::new("/path/to/workspace").await?;
//!
//! // Add a job using natural language
//! let schedule = parse_natural("every day at 2am")?;  // Returns "0 2 * * *"
//! let job = manager.add_job("backup", &schedule, "backup.sh").await?;
//!
//! // Or use Chinese
//! let schedule = parse_natural("每天凌晨2点")?;  // Returns "0 2 * * *"
//!
//! // List all jobs
//! let jobs = manager.list_jobs().await?;
//!
//! // Start the scheduler
//! manager.start().await?;
//! ```

pub mod natural;
mod parser;
mod scheduler;
mod store;
mod types;

pub use natural::parse_natural;
pub use parser::CronExpression;
pub use scheduler::{CronManager, SchedulerEvent};
pub use store::{CronStore, FileCronStore, MemoryCronStore};
pub use types::{CronError, CronJob, JobExecution, JobStatus, Result};
