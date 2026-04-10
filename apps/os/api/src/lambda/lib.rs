//! A3S Lambda - elastic a3s-box runtime platform
//!
//! This library provides the shared runtime for executing all workloads inside
//! isolated a3s-box MicroVMs. Agent invocations are one workload flavor; helper
//! tasks such as fetch, crawl, parse, or user-defined execution adapters are
//! the same runtime class from Lambda's point of view. It is designed to work
//! both as an embeddable Rust library and as the basis of a deployable Agent as
//! a Service surface.
//!
//! # Features
//!
//! - **Universal Box Runtime**: Treat every workload as an a3s-box microVM workload
//! - **Agent Invocation Runtime**: Download and run packaged agents from A3S Registry
//! - **Execution Task Runtime**: Run task adapters behind the same box-runtime contract
//! - **Task Persistence**: Persist invocation state for query and recovery
//! - **MicroVM Isolation**: Run workloads behind explicit runtime boundaries
//! - **Service Surface**: Expose a shared client/server contract for AaaS callers
//! - **PostgreSQL Storage**: Use durable storage with distributed worker coordination
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Language SDKs                            │
//! │         Python · Node.js · Go · Ruby · Java                 │
//! └────────────────────────┬────────────────────────────────────┘
//!                          │ FFI
//! ┌────────────────────────▼────────────────────────────────────┐
//! │                   a3s-lambda (Rust Runtime)                 │
//! │  ┌──────────────────┐  ┌──────────────┐  ┌──────────────┐ │
//! │  │ Box Runtime      │  │   Domain     │  │Infrastructure│ │
//! │  │                  │  │              │  │              │ │
//! │  │ • Agent flavor   │  │ • Task       │  │ • Traits     │ │
//! │  │ • Adapter flavor │  │ • Repository │  │ • Storages   │ │
//! │  └──────────────────┘  └──────────────┘  └──────────────┘ │
//! └────────────────────────┬────────────────────────────────────┘
//!                          │
//! ┌────────────────────────▼────────────────────────────────────┐
//! │                 a3s-lambda-core                             │
//! │       Shared task model, errors, and repository traits      │
//! └────────────────────────┬────────────────────────────────────┘
//!                          │
//! ┌────────────────────────▼────────────────────────────────────┐
//! │                    a3s-box SDK                              │
//! │              MicroVM Runtime (KVM/libkrun)                  │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! Upstream services should link to this crate or call its service surface
//! instead of maintaining separate Lambda runtime trees. Lambda owns runtime
//! isolation and worker observability; higher-level orchestration and scaling
//! policy should remain in A3S OS / Flow. Durable persistence is implemented in
//! `src/storages/pg`.
//!
//! # Example (Rust)
//!
//! ```no_run
//! use a3s_lambda::{LambdaClient, TaskKind, TaskRequest};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Create client
//!     let client = LambdaClient::new("http://localhost:3000").await?;
//!
//!     // Submit agent task
//!     let task = TaskRequest {
//!         kind: TaskKind::AgentRun {
//!             agent: "alice/my-agent".to_string(),
//!             version: "^1.0".to_string(),
//!             input: serde_json::json!({"query": "hello"}),
//!             box_runtime: None,
//!             timeout_secs: 300,
//!         },
//!     };
//!
//!     let result = client.execute_task(task).await?;
//!     println!("Status: {:?}", result.status);
//!     Ok(())
//! }
//! ```
//!
//! # Example (Python via SDK)
//!
//! ```python
//! from a3s_lambda import LambdaClient, AgentTask
//!
//! # Create client
//! client = LambdaClient("http://localhost:3000")
//!
//! # Submit agent task
//! task = AgentTask(
//!     agent="alice/my-agent",
//!     version="^1.0",
//!     input={"query": "hello"},
//!     timeout_secs=300
//! )
//!
//! result = client.execute_task(task)
//! print(f"Success: {result.success}")
//! ```

pub mod config;
pub mod distributed;
pub mod domain;
pub mod executor;
pub mod infrastructure;
pub mod observability;
pub mod pool;
pub mod queue;
pub mod server;
pub mod storages;
pub mod worker;

// Re-exports for convenience
pub use a3s_lambda_core::{
    AgentInvocationRequest, BoxRuntimeSpec, BoxWorkloadEnvelope, CapabilityAccess,
    CapabilityMatchMode, CapabilityRisk, ExecutionCapability, ExecutionCapabilityGrant,
    ExecutionPolicy, ExecutionTaskRequest, ExecutionTaskSubmissionRequest, LambdaError,
    LambdaRepository, LambdaTask, Result, RuntimeClass, TaskKind, TaskRequest, TaskResult,
    TaskStats, TaskStatus, WorkloadKind,
};
pub use config::{ExecutionLaunchMode, LambdaConfig, RunMode, SubmissionMode};
pub use executor::{
    agent::AgentExecutor, http::execute as execute_http, BoxRuntimeExecutor, ExecutionAdapter,
    ExecutionRegistry,
};
pub use observability::{
    WorkerReadiness, WorkerReadinessFailure, WorkerScalingContract, WorkerScalingSignals,
    WorkerScalingSnapshot, WorkerStatsSnapshot, WORKER_SCALING_CONTRACT_VERSION,
};
pub use server::LambdaServer;
pub use storages::{TaskRecord, TaskStore};
pub use worker::PgLeaseWorker;

// High-level client API
mod client;
pub use client::InvocationCancellation;
pub use client::InvocationChildrenSummary;
pub use client::InvocationSummary;
pub use client::LambdaClient;
pub use client::LambdaClientBuilder;
pub use client::TaskCancellation;
pub use server::LambdaServerBuilder;

/// Library version
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
