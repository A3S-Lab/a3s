//! Error types for a3s CLI.

use thiserror::Error;

#[derive(Error, Debug)]
pub enum A3sError {
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("hcl parse error: {0}")]
    HclParse(String),

    #[error("template error: {0}")]
    Template(String),

    #[error("config error: {0}")]
    Config(String),

    #[error("network error: {0}")]
    Network(String),

    #[error("api error: {0}")]
    Api(String),

    #[error("invalid argument: {0}")]
    InvalidArgument(String),

    #[error("project error: {0}")]
    Project(String),

    #[error("other error: {0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, A3sError>;
