//! Agent module — in-process a3s-code agent integration
//!
//! This module integrates a3s-code's `SessionManager` directly into SafeClaw,
//! providing in-process LLM agent execution with tool use, HITL confirmation,
//! and streaming event translation for the browser UI.
//!
//! ## Architecture
//!
//! ```text
//! UI (React) <-WS(JSON)-> handler.rs -> engine.rs -> a3s-code SessionManager (in-process)
//!                          REST API       └── session_store.rs (UI state persistence)
//! ```

pub mod bus;
pub mod engine;
pub mod handler;
pub mod session_store;
pub mod types;

pub use bus::AgentBus;
pub use engine::AgentEngine;
pub use handler::{agent_router, AgentState};
pub use session_store::AgentSessionStore;
