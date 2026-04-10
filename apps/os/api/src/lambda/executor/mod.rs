//! Executor module - stub implementation
//!
//! This module is stubbed because the a3s-box-sdk was removed.

use std::time::Duration;

use async_trait::async_trait;

pub mod agent;
pub mod guest;
pub mod http;

pub use agent::{AgentExecutor, ExecutionResult};
pub use http::execute as execute_http;

// Re-exports for convenience
pub use a3s_lambda_core::{
    BoxWorkloadEnvelope, CapabilityRisk, ExecutionCapability, ExecutionCapabilityGrant,
};
pub use crate::observability::BoxRuntimePoolSnapshot;

/// Stub ExecutionAdapter trait - VM execution disabled
#[async_trait]
pub trait ExecutionAdapter: Send + Sync {
    fn capabilities(&self) -> Vec<ExecutionCapabilityGrant>;

    async fn execute(
        &self,
        handler: &str,
        input: &serde_json::Value,
        timeout: Duration,
    ) -> Result<serde_json::Value, String>;
}

/// Stub ExecutionRegistry - VM execution disabled
#[derive(Debug, Clone)]
pub struct ExecutionRegistry;

impl ExecutionRegistry {
    pub fn new() -> Self {
        tracing::warn!("ExecutionRegistry is stubbed - VM execution disabled");
        Self
    }

    pub fn with_defaults() -> Self {
        Self::new()
    }

    #[allow(dead_code)]
    pub fn from_enabled_with_launch_mode(
        _adapters: impl IntoIterator<Item = impl AsRef<str>>,
        _launch_mode: crate::config::ExecutionLaunchMode,
    ) -> Result<Self, String> {
        tracing::warn!("ExecutionRegistry is stubbed - VM execution disabled");
        Ok(Self::new())
    }

    pub async fn execute_box_workload(
        &self,
        _envelope: &BoxWorkloadEnvelope,
        _timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        Err("VM execution disabled - SDK not available".to_string())
    }

    #[allow(dead_code)]
    pub async fn box_runtime_pool_snapshot(&self) -> BoxRuntimePoolSnapshot {
        BoxRuntimePoolSnapshot {
            launch_mode: crate::config::ExecutionLaunchMode::HostAdapterCompat,
            image_pool_count: 0,
            idle_vms: 0,
            active_vms: 0,
            total_vms: 0,
            max_total_vms: 0,
            available_vms: 0,
            occupancy_ratio: 0.0,
            active_ratio: 0.0,
            has_capacity_pressure: false,
        }
    }

    #[allow(dead_code)]
    pub fn validate_capabilities(
        &self,
        _executor: &str,
        _required_capabilities: &[a3s_lambda_core::ExecutionCapability],
        _policy: &a3s_lambda_core::ExecutionPolicy,
    ) -> std::result::Result<(), String> {
        // Stub: always allow since VM execution is disabled
        Ok(())
    }
}

impl Default for ExecutionRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// Stub BoxRuntimeExecutor - VM execution disabled
pub struct BoxRuntimeExecutor;

impl BoxRuntimeExecutor {
    #[allow(dead_code)]
    pub fn new(
        _agent_executor: &AgentExecutor,
        _execution_registry: &ExecutionRegistry,
    ) -> Self {
        tracing::warn!("BoxRuntimeExecutor is stubbed - VM execution disabled");
        Self
    }

    #[allow(dead_code)]
    pub async fn execute(
        &self,
        _task: &a3s_lambda_core::LambdaTask,
        _timeout: Duration,
    ) -> Result<serde_json::Value, String> {
        Err("VM execution disabled - SDK not available".to_string())
    }
}
