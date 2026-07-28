//! Reusable infrastructure for the umbrella A3S CLI.

pub mod components;
pub mod registry;
pub mod research;

// Compile the process adapter in the library test target as well, so its
// hermetic fake-process contract tests do not depend on the TUI test graph.
#[cfg(test)]
#[path = "use_registry.rs"]
mod use_registry;
