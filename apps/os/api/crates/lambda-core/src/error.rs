use thiserror::Error;

#[derive(Debug, Error)]
pub enum LambdaError {
    #[error("database error: {0}")]
    Database(String),

    #[error("not found: {0}")]
    NotFound(String),

    #[error("bad request: {0}")]
    BadRequest(String),

    #[error("internal error: {0}")]
    Internal(String),

    #[error("execution error: {0}")]
    Execution(String),

    #[error("timeout: {0}")]
    Timeout(String),

    #[error("download error: {0}")]
    Download(String),

    #[error("verification error: {0}")]
    Verification(String),
}

impl From<reqwest::Error> for LambdaError {
    fn from(err: reqwest::Error) -> Self {
        Self::Download(err.to_string())
    }
}

impl From<std::io::Error> for LambdaError {
    fn from(err: std::io::Error) -> Self {
        Self::Internal(err.to_string())
    }
}

pub type Result<T> = std::result::Result<T, LambdaError>;
