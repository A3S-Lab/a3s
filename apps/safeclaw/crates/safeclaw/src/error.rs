//! SafeClaw error types

use thiserror::Error;

/// SafeClaw error type
#[derive(Error, Debug)]
pub enum Error {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(String),

    /// Channel error
    #[error("Channel error: {0}")]
    Channel(String),

    /// Session error
    #[error("Session error: {0}")]
    Session(String),

    /// TEE error
    #[error("TEE error: {0}")]
    Tee(String),

    /// Privacy classification error
    #[error("Privacy error: {0}")]
    Privacy(String),

    /// Runtime error
    #[error("Runtime error: {0}")]
    Runtime(String),

    /// IO error
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization error
    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    /// HTTP error
    #[error("HTTP error: {0}")]
    Http(#[from] reqwest::Error),

    /// Leakage prevention error
    #[error("Leakage error: {0}")]
    Leakage(String),

    /// Internal error
    #[error("Internal error: {0}")]
    Internal(String),
}

/// Result type alias for SafeClaw operations
pub type Result<T> = std::result::Result<T, Error>;

/// Serialize any `Serialize` value to `serde_json::Value` without panicking.
///
/// Falls back to a JSON error object if serialization fails (e.g. non-string
/// map keys or non-finite floats — unlikely for well-typed structs, but
/// eliminates `unwrap()` from production code).
pub fn to_json<T: serde::Serialize>(value: T) -> serde_json::Value {
    serde_json::to_value(value).unwrap_or_else(|e| {
        serde_json::json!({
            "error": {
                "code": "SERIALIZATION_ERROR",
                "message": e.to_string()
            }
        })
    })
}
