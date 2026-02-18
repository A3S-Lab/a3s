//! Shared types for the A3S ecosystem.
//!
//! This crate provides:
//! - [`privacy`] — PII classification, regex-based redaction, keyword matching
//! - [`tools`] — Core tool definitions and safe path resolution
//! - [`transport`] — Async framed transport abstraction (Unix sockets, TEE protocol)

pub mod privacy;
pub mod tools;
pub mod transport;

// Flat re-exports for convenience (preserves most existing import paths)
pub use privacy::*;
pub use tools::*;
pub use transport::*;
