//! Deployment state management.
//!
//! Tracks the state of deployed models and dependencies.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Overall deployment state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeploymentState {
    pub project_name: String,
    pub provider: String,
    pub models: HashMap<String, ModelDeploymentState>,
    pub dependencies: HashMap<String, DependencyDeploymentState>,
    pub last_updated: DateTime<Utc>,
}

impl DeploymentState {
    pub fn new(project_name: String, provider: String) -> Self {
        Self {
            project_name,
            provider,
            models: HashMap::new(),
            dependencies: HashMap::new(),
            last_updated: Utc::now(),
        }
    }

    pub fn add_model(&mut self, name: String, state: ModelDeploymentState) {
        self.models.insert(name, state);
        self.last_updated = Utc::now();
    }

    pub fn add_dependency(&mut self, name: String, state: DependencyDeploymentState) {
        self.dependencies.insert(name, state);
        self.last_updated = Utc::now();
    }
}

/// Model deployment state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModelDeploymentState {
    pub name: String,
    pub provider: String,
    pub deployment_type: String,
    pub status: DeploymentStatus,
    pub endpoint: Option<String>,
    pub replicas: i32,
    pub gpu: bool,
}

/// Dependency service deployment state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DependencyDeploymentState {
    pub name: String,
    pub image: String,
    pub status: DeploymentStatus,
    pub endpoints: HashMap<String, u16>,
    pub container_id: Option<String>,
}

/// Deployment status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DeploymentStatus {
    Pending,
    Deploying,
    Running,
    Unhealthy,
    Failed,
    Terminating,
    Terminated,
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Deploying => write!(f, "deploying"),
            Self::Running => write!(f, "running"),
            Self::Unhealthy => write!(f, "unhealthy"),
            Self::Failed => write!(f, "failed"),
            Self::Terminating => write!(f, "terminating"),
            Self::Terminated => write!(f, "terminated"),
        }
    }
}
