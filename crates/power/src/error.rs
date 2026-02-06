#[derive(Debug, thiserror::Error)]
pub enum PowerError {
    #[error("Model not found: {0}")]
    ModelNotFound(String),

    #[error("Backend not available: {0}")]
    BackendNotAvailable(String),

    #[error("Download failed for {model}: {source}")]
    DownloadFailed {
        model: String,
        source: reqwest::Error,
    },

    #[error("Inference failed: {0}")]
    InferenceFailed(String),

    #[error("Invalid model format: {0}")]
    InvalidFormat(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Server error: {0}")]
    Server(String),

    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Serialization error: {0}")]
    Serialization(#[from] serde_json::Error),

    #[error("TOML deserialization error: {0}")]
    TomlDe(#[from] toml::de::Error),

    #[error("TOML serialization error: {0}")]
    TomlSer(#[from] toml::ser::Error),
}

pub type Result<T> = std::result::Result<T, PowerError>;

impl From<PowerError> for axum::response::Response {
    fn from(err: PowerError) -> Self {
        use axum::http::StatusCode;
        use axum::response::IntoResponse;

        let (status, message) = match &err {
            PowerError::ModelNotFound(_) => (StatusCode::NOT_FOUND, err.to_string()),
            PowerError::BackendNotAvailable(_) => {
                (StatusCode::SERVICE_UNAVAILABLE, err.to_string())
            }
            PowerError::InvalidFormat(_) => (StatusCode::BAD_REQUEST, err.to_string()),
            _ => (StatusCode::INTERNAL_SERVER_ERROR, err.to_string()),
        };

        let body = serde_json::json!({ "error": message });
        (status, axum::Json(body)).into_response()
    }
}
