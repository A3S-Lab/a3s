//! Configuration management for a3s-lambda
//!
//! Supports loading configuration from:
//! - Environment variables (for K8s/Knative)
//! - YAML files (for local development)

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::domain::LambdaError;

const DEFAULT_RUNTIME_DIR_NAME: &str = "a3s-lambda";
const LEGACY_RUNTIME_DIR_NAME: &str = "a3s-faas";

/// Main configuration for a3s-lambda server
#[derive(Debug, Clone, Deserialize, Default)]
pub struct LambdaConfig {
    /// Runtime behavior configuration
    #[serde(default)]
    pub runtime: RuntimeConfig,

    /// Server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Registry configuration
    #[serde(default)]
    pub registry: RegistryConfig,

    /// VM pool configuration
    #[serde(default)]
    pub vm_pool: VmPoolConfig,

    /// Database configuration
    #[serde(default)]
    pub database: DatabaseConfig,

    /// Queue configuration
    #[serde(default)]
    pub queue: QueueConfig,

    /// Execution adapter configuration
    #[serde(default)]
    pub execution: ExecutionConfig,

    /// Observability configuration
    #[serde(default)]
    pub observability: ObservabilityConfig,

    /// Distributed worker configuration
    #[serde(default)]
    pub worker: WorkerConfig,

    /// Resource limits
    #[serde(default)]
    pub limits: ResourceLimits,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RuntimeConfig {
    #[serde(default)]
    pub run_mode: RunMode,

    #[serde(default)]
    pub submission_mode: SubmissionMode,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum RunMode {
    #[default]
    Server,
    Worker,
    All,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum SubmissionMode {
    #[default]
    Auto,
    AwaitCompletion,
    PersistOnly,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    #[serde(default = "default_host")]
    pub host: String,

    #[serde(default = "default_port")]
    pub port: u16,

    #[serde(default = "default_shutdown_timeout")]
    pub shutdown_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RegistryConfig {
    #[serde(default = "default_registry_url")]
    pub url: String,

    #[serde(default = "default_temp_dir")]
    pub temp_dir: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct VmPoolConfig {
    #[serde(default = "default_pool_size")]
    pub size: usize,

    #[serde(default = "default_min_idle")]
    pub min_idle: usize,

    #[serde(default = "default_max_idle")]
    pub max_idle: usize,

    #[serde(default = "default_warmup_timeout")]
    pub warmup_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DatabaseConfig {
    #[serde(default = "default_database_url")]
    pub url: String,

    #[serde(default = "default_max_connections")]
    pub max_connections: u32,
}

#[derive(Debug, Clone, Deserialize)]
pub struct QueueConfig {
    #[serde(default = "default_realtime_workers")]
    pub realtime_workers: usize,

    #[serde(default = "default_faas_workers")]
    pub faas_workers: usize,

    #[serde(default = "default_batch_workers")]
    pub batch_workers: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ExecutionConfig {
    #[serde(default = "default_enabled_adapters")]
    pub enabled_adapters: Vec<String>,

    #[serde(default)]
    pub launch_mode: ExecutionLaunchMode,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[derive(Default)]
pub enum ExecutionLaunchMode {
    #[default]
    HostAdapterCompat,
    MicroVmPreferred,
    MicroVmRequired,
}

#[derive(Debug, Clone, Deserialize)]
pub struct WorkerConfig {
    #[serde(default)]
    pub enabled: bool,

    #[serde(default)]
    pub worker_id: Option<String>,

    #[serde(default = "default_worker_poll_interval_ms")]
    pub poll_interval_ms: u64,

    #[serde(default = "default_worker_lease_ttl_secs")]
    pub lease_ttl_secs: u64,

    #[serde(default = "default_worker_heartbeat_interval_secs")]
    pub heartbeat_interval_secs: u64,

    #[serde(default = "default_true")]
    pub release_expired_leases: bool,

    #[serde(default = "default_worker_observability_enabled")]
    pub observability_enabled: bool,

    #[serde(default = "default_worker_observability_host")]
    pub observability_host: String,

    #[serde(default = "default_worker_observability_port")]
    pub observability_port: u16,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ObservabilityConfig {
    #[serde(default)]
    pub otel_endpoint: Option<String>,

    #[serde(default = "default_true")]
    pub metrics_enabled: bool,

    #[serde(default = "default_true")]
    pub tracing_enabled: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ResourceLimits {
    #[serde(default = "default_max_concurrent_tasks")]
    pub max_concurrent_tasks: usize,

    #[serde(default = "default_task_timeout")]
    pub task_timeout_secs: u64,
}

// Default values
fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_shutdown_timeout() -> u64 {
    30
}

fn default_registry_url() -> String {
    "http://localhost:3000".to_string()
}

fn default_temp_dir() -> String {
    default_runtime_dir().display().to_string()
}

fn default_pool_size() -> usize {
    10
}

fn default_min_idle() -> usize {
    3
}

fn default_max_idle() -> usize {
    8
}

fn default_warmup_timeout() -> u64 {
    30
}

fn default_database_url() -> String {
    "postgres://127.0.0.1:5432/a3s_lambda".to_string()
}

fn default_max_connections() -> u32 {
    10
}

fn default_realtime_workers() -> usize {
    2
}

fn default_faas_workers() -> usize {
    5
}

fn default_batch_workers() -> usize {
    2
}

fn default_enabled_adapters() -> Vec<String> {
    vec!["http".to_string()]
}

fn default_worker_poll_interval_ms() -> u64 {
    500
}

fn default_worker_lease_ttl_secs() -> u64 {
    30
}

fn default_worker_heartbeat_interval_secs() -> u64 {
    10
}

fn default_worker_observability_enabled() -> bool {
    true
}

fn default_worker_observability_host() -> String {
    "0.0.0.0".to_string()
}

fn default_worker_observability_port() -> u16 {
    9090
}

fn default_max_concurrent_tasks() -> usize {
    100
}

fn default_task_timeout() -> u64 {
    300
}

fn default_true() -> bool {
    true
}

pub fn default_runtime_dir() -> PathBuf {
    let base = std::env::temp_dir();
    let preferred = base.join(DEFAULT_RUNTIME_DIR_NAME);
    let legacy = base.join(LEGACY_RUNTIME_DIR_NAME);

    if legacy.exists() && !preferred.exists() {
        legacy
    } else {
        preferred
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: default_host(),
            port: default_port(),
            shutdown_timeout_secs: default_shutdown_timeout(),
        }
    }
}

impl Default for RuntimeConfig {
    fn default() -> Self {
        Self {
            run_mode: RunMode::Server,
            submission_mode: SubmissionMode::Auto,
        }
    }
}

impl Default for RegistryConfig {
    fn default() -> Self {
        Self {
            url: default_registry_url(),
            temp_dir: default_temp_dir(),
        }
    }
}

impl Default for VmPoolConfig {
    fn default() -> Self {
        Self {
            size: default_pool_size(),
            min_idle: default_min_idle(),
            max_idle: default_max_idle(),
            warmup_timeout_secs: default_warmup_timeout(),
        }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            url: default_database_url(),
            max_connections: default_max_connections(),
        }
    }
}

impl Default for QueueConfig {
    fn default() -> Self {
        Self {
            realtime_workers: default_realtime_workers(),
            faas_workers: default_faas_workers(),
            batch_workers: default_batch_workers(),
        }
    }
}

impl Default for ObservabilityConfig {
    fn default() -> Self {
        Self {
            otel_endpoint: None,
            metrics_enabled: true,
            tracing_enabled: true,
        }
    }
}

impl Default for ExecutionConfig {
    fn default() -> Self {
        Self {
            enabled_adapters: default_enabled_adapters(),
            launch_mode: ExecutionLaunchMode::default(),
        }
    }
}

impl Default for WorkerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            worker_id: None,
            poll_interval_ms: default_worker_poll_interval_ms(),
            lease_ttl_secs: default_worker_lease_ttl_secs(),
            heartbeat_interval_secs: default_worker_heartbeat_interval_secs(),
            release_expired_leases: true,
            observability_enabled: default_worker_observability_enabled(),
            observability_host: default_worker_observability_host(),
            observability_port: default_worker_observability_port(),
        }
    }
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_concurrent_tasks: default_max_concurrent_tasks(),
            task_timeout_secs: default_task_timeout(),
        }
    }
}

impl LambdaConfig {
    /// Load configuration from environment variables
    ///
    /// Environment variables use the pattern: A3S_LAMBDA_<SECTION>_<KEY>
    /// Example: A3S_LAMBDA_SERVER_PORT=8080
    pub fn from_env() -> Result<Self, LambdaError> {
        envy::prefixed("A3S_LAMBDA_")
            .from_env()
            .map_err(|e| LambdaError::Internal(format!("Failed to load config from env: {}", e)))
    }

    /// Load configuration from YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, LambdaError> {
        let content = std::fs::read_to_string(path.as_ref())
            .map_err(|e| LambdaError::Internal(format!("Failed to read config file: {}", e)))?;

        serde_yaml::from_str(&content)
            .map_err(|e| LambdaError::Internal(format!("Failed to parse config file: {}", e)))
    }

    /// Load configuration with fallback
    ///
    /// 1. Try to load from file if path is provided
    /// 2. Override with environment variables
    /// 3. Use defaults for missing values
    pub fn load(config_file: Option<&str>) -> Result<Self, LambdaError> {
        let mut config = if let Some(path) = config_file {
            Self::from_file(path)?
        } else {
            Self::default()
        };

        // Override with environment variables
        if let Ok(env_config) = Self::from_env() {
            config = env_config;
        }

        Ok(config)
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), LambdaError> {
        self.validate_with_default_task_store(true)
    }

    pub fn validate_with_default_task_store(
        &self,
        require_default_task_store: bool,
    ) -> Result<(), LambdaError> {
        if self.vm_pool.size == 0 {
            return Err(LambdaError::Internal(
                "VM pool size must be > 0".to_string(),
            ));
        }

        if self.vm_pool.min_idle > self.vm_pool.size {
            return Err(LambdaError::Internal(
                "VM pool min_idle cannot exceed size".to_string(),
            ));
        }

        if self.vm_pool.max_idle < self.vm_pool.min_idle {
            return Err(LambdaError::Internal(
                "VM pool max_idle cannot be less than min_idle".to_string(),
            ));
        }

        if self.execution.enabled_adapters.is_empty() {
            return Err(LambdaError::Internal(
                "at least one execution adapter must be enabled".to_string(),
            ));
        }

        if self.worker.heartbeat_interval_secs >= self.worker.lease_ttl_secs {
            return Err(LambdaError::Internal(
                "worker heartbeat interval must be less than lease ttl".to_string(),
            ));
        }

        if require_default_task_store && !uses_postgres_backend(&self.database.url) {
            return Err(LambdaError::Internal(
                "the built-in task store requires postgres:// or postgresql://; inject a custom TaskStore to use another backend"
                    .to_string(),
            ));
        }

        Ok(())
    }

    pub fn resolved_run_mode(&self) -> RunMode {
        match (self.runtime.run_mode, self.worker.enabled) {
            // Legacy compatibility: old configs used `worker.enabled: true`
            // without an explicit runtime mode.
            (RunMode::Server, true) => RunMode::All,
            (mode, _) => mode,
        }
    }

    pub fn resolved_submission_mode(&self) -> SubmissionMode {
        match self.runtime.submission_mode {
            SubmissionMode::Auto => SubmissionMode::PersistOnly,
            mode => mode,
        }
    }
}

fn uses_postgres_backend(database_url: &str) -> bool {
    database_url.starts_with("postgres://") || database_url.starts_with("postgresql://")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = LambdaConfig::default();
        assert_eq!(config.runtime.run_mode, RunMode::Server);
        assert_eq!(config.runtime.submission_mode, SubmissionMode::Auto);
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.vm_pool.size, 10);
        assert_eq!(config.execution.enabled_adapters, vec!["http"]);
        assert!(!config.worker.enabled);
        assert_eq!(config.database.url, "postgres://127.0.0.1:5432/a3s_lambda");
        assert!(config.validate().is_ok());
    }

    #[test]
    fn test_validate_pool_size() {
        let mut config = LambdaConfig::default();
        config.vm_pool.size = 0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_min_idle() {
        let mut config = LambdaConfig::default();
        config.vm_pool.min_idle = 20;
        config.vm_pool.size = 10;
        assert!(config.validate().is_err());
    }

    #[test]
    fn default_runtime_dir_uses_supported_name() {
        let dir = default_runtime_dir();
        assert!(dir.ends_with(DEFAULT_RUNTIME_DIR_NAME) || dir.ends_with(LEGACY_RUNTIME_DIR_NAME));
    }

    #[test]
    fn test_validate_enabled_adapters() {
        let mut config = LambdaConfig::default();
        config.execution.enabled_adapters.clear();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_validate_worker_heartbeat_interval() {
        let mut config = LambdaConfig::default();
        config.worker.heartbeat_interval_secs = 30;
        config.worker.lease_ttl_secs = 30;
        assert!(config.validate().is_err());
    }

    #[test]
    fn auto_submission_mode_prefers_persist_only_for_postgres() {
        let mut config = LambdaConfig::default();
        config.database.url = "postgres://localhost/a3s".into();
        assert_eq!(
            config.resolved_submission_mode(),
            SubmissionMode::PersistOnly
        );
    }

    #[test]
    fn auto_submission_mode_defaults_to_persist_only() {
        let config = LambdaConfig::default();
        assert_eq!(
            config.resolved_submission_mode(),
            SubmissionMode::PersistOnly
        );
    }

    #[test]
    fn validate_rejects_non_postgres_database_urls() {
        let mut config = LambdaConfig::default();
        config.database.url = "sqlite:///tmp/a3s-lambda/tasks.db".into();
        assert!(config.validate().is_err());
    }

    #[test]
    fn validate_allows_non_postgres_when_default_store_is_not_required() {
        let mut config = LambdaConfig::default();
        config.database.url = "custom://memory".into();
        assert!(config.validate_with_default_task_store(false).is_ok());
    }

    #[test]
    fn validate_with_default_task_store_requires_postgres_when_default_store_is_required() {
        let mut config = LambdaConfig::default();
        config.database.url = "custom://memory".into();

        let error = config
            .validate_with_default_task_store(true)
            .expect_err("default task store should require postgres");

        assert!(error
            .to_string()
            .contains("the built-in task store requires postgres:// or postgresql://"));
    }

    #[test]
    fn validate_with_default_task_store_still_checks_worker_heartbeat_rules() {
        let mut config = LambdaConfig::default();
        config.database.url = "custom://memory".into();
        config.worker.heartbeat_interval_secs = 30;
        config.worker.lease_ttl_secs = 30;
        assert!(config.validate_with_default_task_store(false).is_err());
    }

    #[test]
    fn validate_with_default_task_store_still_requires_execution_adapters() {
        let mut config = LambdaConfig::default();
        config.database.url = "custom://memory".into();
        config.execution.enabled_adapters.clear();
        assert!(config.validate_with_default_task_store(false).is_err());
    }

    #[test]
    fn legacy_worker_enabled_enables_all_mode() {
        let mut config = LambdaConfig::default();
        config.worker.enabled = true;
        assert_eq!(config.resolved_run_mode(), RunMode::All);
    }

    #[test]
    fn explicit_run_mode_overrides_legacy_worker_enabled_flag() {
        let mut config = LambdaConfig::default();
        config.worker.enabled = true;
        config.runtime.run_mode = RunMode::Worker;
        assert_eq!(config.resolved_run_mode(), RunMode::Worker);
    }
}
