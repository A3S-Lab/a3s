use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use crate::domain::LambdaError;
use crate::pool::{VmPoolConfig, VmPoolStats};

/// Agent executor stub - VM execution disabled
pub struct AgentExecutor;

impl AgentExecutor {
    pub async fn new(_registry_url: String, _temp_dir: PathBuf) -> Result<Self, LambdaError> {
        tracing::warn!("AgentExecutor is stubbed - VM execution disabled");
        Ok(Self)
    }

    pub async fn with_pool_config(
        _registry_url: String,
        _temp_dir: PathBuf,
        _pool_config: VmPoolConfig,
    ) -> Result<Self, LambdaError> {
        tracing::warn!("AgentExecutor is stubbed - VM execution disabled");
        Ok(Self)
    }

    pub async fn pool_stats(&self) -> VmPoolStats {
        VmPoolStats {
            idle: 0,
            active: 0,
            max_total: 0,
            available_permits: 0,
        }
    }

    pub async fn shutdown(&self) {}

    pub async fn execute(
        &self,
        _agent: &str,
        _version: &str,
        _input: &Value,
        _timeout: Duration,
    ) -> Result<ExecutionResult, String> {
        Err("Agent execution disabled - SDK not available".to_string())
    }
}

pub struct ExecutionResult {
    pub execution_id: String,
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}
