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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_request_status_roundtrip() {
        let req = IpcRequest::Status;
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"cmd\":\"status\""));
        let _: IpcRequest = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_request_stop_roundtrip() {
        let req = IpcRequest::Stop { services: vec!["web".into(), "api".into()] };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: IpcRequest = serde_json::from_str(&json).unwrap();
        if let IpcRequest::Stop { services } = decoded {
            assert_eq!(services, vec!["web", "api"]);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_request_logs_roundtrip() {
        let req = IpcRequest::Logs { service: Some("web".into()), follow: true };
        let json = serde_json::to_string(&req).unwrap();
        let decoded: IpcRequest = serde_json::from_str(&json).unwrap();
        if let IpcRequest::Logs { service, follow } = decoded {
            assert_eq!(service, Some("web".into()));
            assert!(follow);
        } else {
            panic!("wrong variant");
        }
    }

    #[test]
    fn test_response_ok_roundtrip() {
        let resp = IpcResponse::Ok;
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"type\":\"ok\""));
        let _: IpcResponse = serde_json::from_str(&json).unwrap();
    }

    #[test]
    fn test_response_log_line_roundtrip() {
        let resp = IpcResponse::LogLine { service: "api".into(), line: "started".into() };
        let json = serde_json::to_string(&resp).unwrap();
        let decoded: IpcResponse = serde_json::from_str(&json).unwrap();
        if let IpcResponse::LogLine { service, line } = decoded {
            assert_eq!(service, "api");
            assert_eq!(line, "started");
        } else {
            panic!("wrong variant");
        }
    }
}

