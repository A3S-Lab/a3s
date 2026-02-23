use std::path::PathBuf;

/// Default socket path for IPC between `a3s up` and other commands.
pub fn socket_path() -> PathBuf {
    std::env::temp_dir().join("a3s-dev.sock")
}

/// IPC request from client commands to the running daemon.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "cmd", rename_all = "snake_case")]
pub enum IpcRequest {
    Status,
    Stop { services: Vec<String> },
    Restart { service: String },
    Logs { service: Option<String>, follow: bool },
    History { service: Option<String>, lines: usize },
}

/// IPC response from daemon to client.
#[derive(Debug, serde::Serialize, serde::Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum IpcResponse {
    Status { rows: Vec<StatusRow> },
    Ok,
    Error { msg: String },
    LogLine { service: String, line: String },
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
pub struct StatusRow {
    pub name: String,
    pub state: String,
    pub pid: Option<u32>,
    pub port: u16,
    pub subdomain: Option<String>,
    pub uptime_secs: Option<u64>,
}
