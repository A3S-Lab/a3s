use thiserror::Error;

#[derive(Debug, Error)]
pub enum DevError {
    #[error("config error: {0}")]
    Config(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("process error for '{service}': {msg}")]
    Process { service: String, msg: String },

    #[error("dependency cycle detected involving: {0}")]
    Cycle(String),

    #[error("unknown service: '{0}'")]
    UnknownService(String),

    #[error("port conflict: services '{a}' and '{b}' both use port {port}")]
    PortConflict { a: String, b: String, port: u16 },
}

pub type Result<T> = std::result::Result<T, DevError>;
