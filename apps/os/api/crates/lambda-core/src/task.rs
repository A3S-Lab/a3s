use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityAccess {
    Read,
    Write,
    Execute,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityRisk {
    Low,
    Medium,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ExecutionCapability {
    Network {
        protocol: String,
        operation: String,
        scope: String,
    },
    Filesystem {
        scope: String,
        access: CapabilityAccess,
    },
    Tool {
        name: String,
        access: CapabilityAccess,
        scope: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionCapabilityGrant {
    pub capability: ExecutionCapability,
    pub risk: CapabilityRisk,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CapabilityMatchMode {
    None,
    AllRequired,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    #[serde(default = "default_capability_match_mode")]
    pub capability_match_mode: CapabilityMatchMode,
    #[serde(default)]
    pub max_risk: Option<CapabilityRisk>,
    #[serde(default)]
    pub allow_risk_escalation: bool,
    #[serde(default)]
    pub allowed_scopes: Vec<String>,
    #[serde(default)]
    pub allow_scope_escalation: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskRequest {
    pub kind: TaskKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeClass {
    A3sBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkloadKind {
    AgentInvocation,
    ExecutionTask,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxRuntimeSpec {
    pub runtime: String,
    pub entrypoint: String,
    #[serde(default)]
    pub args: Vec<String>,
    #[serde(default)]
    pub env: HashMap<String, String>,
}

impl BoxRuntimeSpec {
    pub fn for_agent_invocation(agent: &str, version: &str) -> Self {
        Self {
            runtime: "a3s/agent-runner".into(),
            entrypoint: "a3s-code".into(),
            args: vec![
                "run".into(),
                "--package".into(),
                format!("registry://{agent}@{version}"),
            ],
            env: HashMap::new(),
        }
    }

    pub fn for_execution_adapter(executor: &str, handler: &str) -> Self {
        Self {
            runtime: format!("a3s/executor/{executor}"),
            entrypoint: "a3s-executor".into(),
            args: vec![
                "--executor".into(),
                executor.into(),
                "--handler".into(),
                handler.into(),
            ],
            env: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BoxWorkloadEnvelope {
    pub runtime_class: RuntimeClass,
    pub workload_kind: WorkloadKind,
    pub runtime: BoxRuntimeSpec,
    pub input: serde_json::Value,
    #[serde(default)]
    pub labels: HashMap<String, String>,
}

impl BoxWorkloadEnvelope {
    pub fn validate(&self) -> Result<(), String> {
        if self.runtime.runtime.trim().is_empty() {
            return Err("box workload envelope requires a non-empty runtime".into());
        }

        if self.runtime.entrypoint.trim().is_empty() {
            return Err("box workload envelope requires a non-empty entrypoint".into());
        }

        Ok(())
    }
}

impl TaskRequest {
    pub fn agent_invocation(request: AgentInvocationRequest) -> Self {
        request.into()
    }

    pub fn execution_task(request: ExecutionTaskRequest) -> Self {
        request.into()
    }

    pub fn with_parent_invocation_id(self, parent_invocation_id: Uuid) -> LambdaTask {
        LambdaTask::new(self.kind).with_parent_invocation_id(parent_invocation_id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentInvocationRequest {
    pub agent: String,
    pub version: String,
    pub input: serde_json::Value,
    #[serde(default)]
    pub box_runtime: Option<BoxRuntimeSpec>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
}

impl From<AgentInvocationRequest> for TaskRequest {
    fn from(value: AgentInvocationRequest) -> Self {
        Self {
            kind: TaskKind::AgentRun {
                agent: value.agent,
                version: value.version,
                input: value.input,
                box_runtime: value.box_runtime,
                timeout_secs: value.timeout_secs,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTaskSubmissionRequest {
    pub task: ExecutionTaskRequest,
    #[serde(default)]
    pub parent_invocation_id: Option<Uuid>,
}

impl From<ExecutionTaskSubmissionRequest> for TaskRequest {
    fn from(value: ExecutionTaskSubmissionRequest) -> Self {
        value.task.into()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionTaskRequest {
    pub executor: String,
    pub handler: String,
    #[serde(default)]
    pub input: serde_json::Value,
    #[serde(default)]
    pub required_capabilities: Vec<ExecutionCapability>,
    #[serde(default)]
    pub policy: ExecutionPolicy,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub preferred_lane: Option<String>,
    #[serde(default)]
    pub box_runtime: Option<BoxRuntimeSpec>,
    #[serde(default = "default_timeout")]
    pub timeout_secs: u32,
}

impl From<ExecutionTaskRequest> for TaskRequest {
    fn from(value: ExecutionTaskRequest) -> Self {
        Self {
            kind: TaskKind::Execution {
                executor: value.executor,
                handler: value.handler,
                input: value.input,
                required_capabilities: value.required_capabilities,
                policy: value.policy,
                labels: value.labels,
                preferred_lane: value.preferred_lane,
                box_runtime: value.box_runtime,
                timeout_secs: value.timeout_secs,
            },
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TaskKind {
    AgentRun {
        agent: String,
        version: String,
        input: serde_json::Value,
        #[serde(default)]
        box_runtime: Option<BoxRuntimeSpec>,
        #[serde(default = "default_timeout")]
        timeout_secs: u32,
    },
    Execution {
        executor: String,
        handler: String,
        #[serde(default)]
        input: serde_json::Value,
        #[serde(default)]
        required_capabilities: Vec<ExecutionCapability>,
        #[serde(default)]
        policy: ExecutionPolicy,
        #[serde(default)]
        labels: HashMap<String, String>,
        #[serde(default)]
        preferred_lane: Option<String>,
        #[serde(default)]
        box_runtime: Option<BoxRuntimeSpec>,
        #[serde(default = "default_timeout")]
        timeout_secs: u32,
    },
}

impl TaskKind {
    pub fn workload_kind(&self) -> WorkloadKind {
        match self {
            Self::AgentRun { .. } => WorkloadKind::AgentInvocation,
            Self::Execution { .. } => WorkloadKind::ExecutionTask,
        }
    }

    pub fn runtime_class(&self) -> RuntimeClass {
        RuntimeClass::A3sBox
    }

    pub fn requires_box_runtime(&self) -> bool {
        matches!(self.runtime_class(), RuntimeClass::A3sBox)
    }

    pub fn workload_name(&self) -> &'static str {
        match self.workload_kind() {
            WorkloadKind::AgentInvocation => "agent_invocation",
            WorkloadKind::ExecutionTask => "execution_task",
        }
    }

    pub fn box_runtime(&self) -> BoxRuntimeSpec {
        match self {
            Self::AgentRun {
                agent,
                version,
                box_runtime,
                ..
            } => box_runtime
                .clone()
                .unwrap_or_else(|| BoxRuntimeSpec::for_agent_invocation(agent, version)),
            Self::Execution {
                executor,
                handler,
                box_runtime,
                ..
            } => box_runtime
                .clone()
                .unwrap_or_else(|| BoxRuntimeSpec::for_execution_adapter(executor, handler)),
        }
    }

    pub fn box_workload_envelope(&self) -> BoxWorkloadEnvelope {
        match self {
            Self::AgentRun { input, .. } => BoxWorkloadEnvelope {
                runtime_class: self.runtime_class(),
                workload_kind: self.workload_kind(),
                runtime: self.box_runtime(),
                input: input.clone(),
                labels: HashMap::new(),
            },
            Self::Execution { input, labels, .. } => BoxWorkloadEnvelope {
                runtime_class: self.runtime_class(),
                workload_kind: self.workload_kind(),
                runtime: self.box_runtime(),
                input: input.clone(),
                labels: labels.clone(),
            },
        }
    }
}

fn default_timeout() -> u32 {
    300
}

fn default_capability_match_mode() -> CapabilityMatchMode {
    CapabilityMatchMode::AllRequired
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            capability_match_mode: default_capability_match_mode(),
            max_risk: None,
            allow_risk_escalation: false,
            allowed_scopes: Vec::new(),
            allow_scope_escalation: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LambdaTask {
    pub id: Uuid,
    pub batch_id: Option<Uuid>,
    pub kind: TaskKind,
    pub timeout_secs: u32,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub started_at: Option<chrono::DateTime<chrono::Utc>>,
    pub finished_at: Option<chrono::DateTime<chrono::Utc>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl std::fmt::Display for TaskStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Running => write!(f, "running"),
            Self::Completed => write!(f, "completed"),
            Self::Failed => write!(f, "failed"),
            Self::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl LambdaTask {
    pub fn new(kind: TaskKind) -> Self {
        let timeout_secs = match &kind {
            TaskKind::AgentRun { timeout_secs, .. } => *timeout_secs,
            TaskKind::Execution { timeout_secs, .. } => *timeout_secs,
        };

        Self {
            id: Uuid::new_v4(),
            batch_id: None,
            kind,
            timeout_secs,
            status: TaskStatus::Pending,
            result: None,
            error: None,
            created_at: chrono::Utc::now(),
            started_at: None,
            finished_at: None,
        }
    }

    pub fn with_batch_id(mut self, batch_id: Uuid) -> Self {
        self.batch_id = Some(batch_id);
        self
    }

    pub fn with_parent_invocation_id(self, parent_invocation_id: Uuid) -> Self {
        self.with_batch_id(parent_invocation_id)
    }

    pub fn parent_invocation_id(&self) -> Option<Uuid> {
        self.batch_id
    }

    pub fn is_agent_invocation(&self) -> bool {
        matches!(self.kind, TaskKind::AgentRun { .. })
    }

    pub fn is_execution_task(&self) -> bool {
        matches!(self.kind, TaskKind::Execution { .. })
    }

    pub fn runtime_class(&self) -> RuntimeClass {
        self.kind.runtime_class()
    }

    pub fn requires_box_runtime(&self) -> bool {
        self.kind.requires_box_runtime()
    }

    pub fn box_workload_envelope(&self) -> BoxWorkloadEnvelope {
        self.kind.box_workload_envelope()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskResult {
    pub id: Uuid,
    pub status: TaskStatus,
    pub result: Option<serde_json::Value>,
    pub error: Option<String>,
    pub duration_ms: Option<u64>,
}

impl From<LambdaTask> for TaskResult {
    fn from(task: LambdaTask) -> Self {
        let duration_ms =
            if let (Some(started), Some(finished)) = (task.started_at, task.finished_at) {
                Some((finished - started).num_milliseconds() as u64)
            } else {
                None
            };

        Self {
            id: task.id,
            status: task.status,
            result: task.result,
            error: task.error,
            duration_ms,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_agent_invocation_request() {
        let request = TaskRequest::agent_invocation(AgentInvocationRequest {
            agent: "alice/research-agent".into(),
            version: "1.0.0".into(),
            input: serde_json::json!({"query": "rust async"}),
            box_runtime: None,
            timeout_secs: 120,
        });

        assert!(matches!(request.kind, TaskKind::AgentRun { .. }));
    }

    #[test]
    fn builds_execution_task_request() {
        let request = TaskRequest::execution_task(ExecutionTaskRequest {
            executor: "http".into(),
            handler: "extract".into(),
            input: serde_json::json!({"url": "https://example.com"}),
            required_capabilities: vec![ExecutionCapability::Network {
                protocol: "http".into(),
                operation: "extract".into(),
                scope: "public".into(),
            }],
            policy: ExecutionPolicy {
                capability_match_mode: CapabilityMatchMode::AllRequired,
                allowed_scopes: vec!["public".into()],
                ..Default::default()
            },
            labels: HashMap::new(),
            preferred_lane: Some("batch".into()),
            box_runtime: None,
            timeout_secs: 30,
        });

        assert!(matches!(request.kind, TaskKind::Execution { .. }));
    }

    #[test]
    fn assigns_parent_invocation_to_execution_task() {
        let parent_id = Uuid::new_v4();
        let task = TaskRequest::execution_task(ExecutionTaskRequest {
            executor: "http".into(),
            handler: "get".into(),
            input: serde_json::json!({"url": "https://example.com"}),
            required_capabilities: vec![ExecutionCapability::Network {
                protocol: "http".into(),
                operation: "fetch".into(),
                scope: "public".into(),
            }],
            policy: ExecutionPolicy {
                capability_match_mode: CapabilityMatchMode::AllRequired,
                allowed_scopes: vec!["public".into()],
                ..Default::default()
            },
            labels: HashMap::new(),
            preferred_lane: None,
            box_runtime: None,
            timeout_secs: 30,
        })
        .with_parent_invocation_id(parent_id);

        assert_eq!(task.parent_invocation_id(), Some(parent_id));
        assert!(task.is_execution_task());
    }

    #[test]
    fn all_task_kinds_require_a3s_box_runtime() {
        let agent_task = LambdaTask::new(TaskKind::AgentRun {
            agent: "alice/research-agent".into(),
            version: "1.0.0".into(),
            input: serde_json::json!({}),
            box_runtime: None,
            timeout_secs: 60,
        });
        let execution_task = LambdaTask::new(TaskKind::Execution {
            executor: "http".into(),
            handler: "extract".into(),
            input: serde_json::json!({}),
            required_capabilities: vec![],
            policy: ExecutionPolicy::default(),
            labels: HashMap::new(),
            preferred_lane: None,
            box_runtime: None,
            timeout_secs: 60,
        });

        assert_eq!(agent_task.runtime_class(), RuntimeClass::A3sBox);
        assert_eq!(execution_task.runtime_class(), RuntimeClass::A3sBox);
        assert!(agent_task.requires_box_runtime());
        assert!(execution_task.requires_box_runtime());
        assert_eq!(agent_task.kind.workload_name(), "agent_invocation");
        assert_eq!(execution_task.kind.workload_name(), "execution_task");
    }

    #[test]
    fn derives_default_box_runtime_specs() {
        let agent_kind = TaskKind::AgentRun {
            agent: "alice/research-agent".into(),
            version: "1.0.0".into(),
            input: serde_json::json!({}),
            box_runtime: None,
            timeout_secs: 60,
        };
        let execution_kind = TaskKind::Execution {
            executor: "http".into(),
            handler: "extract".into(),
            input: serde_json::json!({}),
            required_capabilities: vec![],
            policy: ExecutionPolicy::default(),
            labels: HashMap::new(),
            preferred_lane: None,
            box_runtime: None,
            timeout_secs: 60,
        };

        let agent_box = agent_kind.box_runtime();
        let execution_box = execution_kind.box_runtime();

        assert_eq!(agent_box.runtime, "a3s/agent-runner");
        assert_eq!(agent_box.entrypoint, "a3s-code");
        assert_eq!(
            agent_box.args,
            vec![
                "run".to_string(),
                "--package".to_string(),
                "registry://alice/research-agent@1.0.0".to_string(),
            ]
        );

        assert_eq!(execution_box.runtime, "a3s/executor/http");
        assert_eq!(execution_box.entrypoint, "a3s-executor");
        assert_eq!(
            execution_box.args,
            vec![
                "--executor".to_string(),
                "http".to_string(),
                "--handler".to_string(),
                "extract".to_string(),
            ]
        );
    }

    #[test]
    fn builds_box_workload_envelope_for_execution_tasks() {
        let task = LambdaTask::new(TaskKind::Execution {
            executor: "http".into(),
            handler: "extract".into(),
            input: serde_json::json!({"url": "https://example.com"}),
            required_capabilities: vec![],
            policy: ExecutionPolicy::default(),
            labels: HashMap::from([(String::from("source"), String::from("deep-research"))]),
            preferred_lane: None,
            box_runtime: None,
            timeout_secs: 60,
        });

        let envelope = task.box_workload_envelope();

        assert_eq!(envelope.runtime_class, RuntimeClass::A3sBox);
        assert_eq!(envelope.workload_kind, WorkloadKind::ExecutionTask);
        assert_eq!(envelope.runtime.runtime, "a3s/executor/http");
        assert_eq!(envelope.input["url"], "https://example.com");
        assert_eq!(
            envelope.labels.get("source"),
            Some(&"deep-research".to_string())
        );
        assert!(envelope.validate().is_ok());
    }
}
