//! Health monitoring for VM pools and task execution
//!
//! This module provides health check capabilities for monitoring
//! the internal state of a3s-lambda instances (VM pool, queue, database).
//!
//! Note: In K8s + Knative environments, service discovery and node management
//! are handled by the platform. This module focuses on application-level health.

pub mod health;

pub use health::{ClusterHealth, DistributedHealthMonitor, HealthCheck, HealthStatus, NodeHealth};

use std::time::Duration;

/// Configuration for health monitoring
#[derive(Debug, Clone)]
pub struct HealthConfig {
    /// Health check interval
    pub check_interval: Duration,

    /// Timeout for health checks
    pub check_timeout: Duration,
}

impl Default for HealthConfig {
    fn default() -> Self {
        Self {
            check_interval: Duration::from_secs(10),
            check_timeout: Duration::from_secs(5),
        }
    }
}

/// Error types for health operations
#[derive(Debug, thiserror::Error)]
pub enum HealthError {
    #[error("Health check error: {0}")]
    CheckFailed(String),

    #[error("Timeout: {0}")]
    Timeout(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

pub type Result<T> = std::result::Result<T, HealthError>;
