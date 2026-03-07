//! SafeClaw - Secure Personal AI Assistant with TEE Support
//!
//! SafeClaw is a privacy-focused personal AI assistant that combines the
//! multi-channel capabilities of OpenClaw with the hardware-isolated
//! execution environment provided by A3S Box.
//!
//! ## Architecture
//!
//! SafeClaw runs as a backend service behind **a3s-gateway**, which handles
//! TLS termination, routing, rate limiting, authentication, and multi-channel
//! webhook ingestion.
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────────┐
//! │                        A3S Gateway                                   │
//! │  ┌─────────────────────────────────────────────────────────────┐   │
//! │  │  Reverse Proxy + Routing + Middleware                        │   │
//! │  │  - TLS termination, JWT auth, rate limiting                 │   │
//! │  │  - Channel webhooks (Telegram, Slack, Discord, Feishu, ...) │   │
//! │  │  - Conversation affinity (sticky sessions)                  │   │
//! │  │  - Token metering                                           │   │
//! │  └───────────────────────────┬─────────────────────────────────┘   │
//! └──────────────────────────────┼────────────────────────────────────┘
//!                                │ HTTP / WebSocket
//! ┌──────────────────────────────▼────────────────────────────────────┐
//! │                        SafeClaw Backend                            │
//! │  ┌─────────────────────────────────────────────────────────────┐  │
//! │  │                   Session Router                             │  │
//! │  │  - Route messages to appropriate TEE sessions               │  │
//! │  │  - Handle multi-agent routing                               │  │
//! │  │  - Manage session lifecycle                                 │  │
//! │  └───────────────────────────┬─────────────────────────────────┘  │
//! │                              │                                     │
//! │  ┌───────────────────────────▼───────────────────────────────┐   │
//! │  │                   Privacy Classifier                       │   │
//! │  │  - Classify data sensitivity                               │   │
//! │  │  - Route sensitive data to TEE                             │   │
//! │  │  - Handle encryption/decryption                            │   │
//! │  └───────────────────────────┬───────────────────────────────┘   │
//! └──────────────────────────────┼────────────────────────────────────┘
//!                                │ vsock / encrypted channel
//! ┌──────────────────────────────▼────────────────────────────────────┐
//! │                    TEE Environment (A3S Box)                       │
//! │  ┌─────────────────────────────────────────────────────────────┐  │
//! │  │                    Secure Agent Runtime                      │  │
//! │  │  ┌─────────────────┐  ┌─────────────────────────────────┐   │  │
//! │  │  │  A3S Code Agent │  │     Secure Data Store           │   │  │
//! │  │  │  - LLM Client   │  │  - Encrypted credentials        │   │  │
//! │  │  │  - Tool Exec    │  │  - Private conversation history │   │  │
//! │  │  │  - HITL         │  │  - Sensitive user data          │   │  │
//! │  │  └─────────────────┘  └─────────────────────────────────┘   │  │
//! │  └─────────────────────────────────────────────────────────────┘  │
//! │                         MicroVM (Hardware Isolated)                │
//! └────────────────────────────────────────────────────────────────────┘
//! ```
//!
//! ## Key Features
//!
//! ### A3S Gateway Integration
//! - Runs as backend service behind a3s-gateway
//! - Auto-generates gateway routing configuration
//! - Privacy-aware routing via gateway rules
//! - Conversation affinity via sticky sessions
//! - Token metering per user/agent/session
//!
//! ### Multi-Channel Support
//! - Telegram, Slack, Discord, WebChat, Feishu, DingTalk, WeCom
//! - Extensible channel architecture
//! - Unified message routing
//!
//! ### TEE-Based Privacy Protection
//! - Sensitive data processing in hardware-isolated environment
//! - Encrypted communication between gateway and TEE
//! - Secure credential storage
//! - Private conversation history
//!
//! ### Privacy Classification
//! - Automatic sensitivity detection
//! - Configurable classification rules
//! - Data routing based on sensitivity level
//!
//! ## Modules
//!
//! - [`runtime`]: Runtime orchestrator and HTTP API
//! - [`channels`]: Multi-channel message adapters
//! - [`session`]: Session management and routing
//! - [`tee`]: TEE environment integration with A3S Box
//! - [`config`]: Configuration management

pub mod agent;
pub mod api;
pub mod bootstrap;
pub mod channels;
pub mod config;
pub mod error;
pub mod hardening;
pub mod runtime;
pub mod sentinel;
pub mod session;
pub mod skills;
pub mod tee;

pub use agent::{agent_router, AgentEngine, AgentSessionStore, AgentState};
pub use api::build_app;
pub use config::{A3sGatewayConfig, SafeClawConfig};
pub use error::{Error, Result};
pub use runtime::{ProcessedResponse, Runtime, RuntimeBuilder, RuntimeState};
