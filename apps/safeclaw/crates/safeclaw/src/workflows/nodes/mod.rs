//! Custom workflow nodes for SafeClaw.
//!
//! These nodes extend the core a3s-flow engine with SafeClaw-specific
//! integrations and tool nodes.

pub mod mcp;

pub use mcp::McpNode;
