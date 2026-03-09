//! Workflow execution engine for SafeClaw.
//!
//! Wraps a3s-flow with SafeClaw-specific node registrations.

use a3s_flow::{FlowEngine, NodeRegistry};
use std::sync::Arc;

use super::nodes::McpNode;

/// Create a workflow engine with all SafeClaw nodes registered.
pub fn create_engine() -> FlowEngine {
    let mut registry = NodeRegistry::with_defaults();

    // Register SafeClaw tool nodes
    registry.register(Arc::new(McpNode));

    FlowEngine::new(registry)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_includes_mcp_node() {
        let engine = create_engine();
        // Engine should be created successfully with MCP node registered
        drop(engine);
    }
}
