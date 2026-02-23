use std::sync::Arc;

use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;
use tokio::sync::broadcast;

use crate::ipc::{socket_path, IpcRequest, IpcResponse};
use crate::supervisor::{Supervisor, SupervisorEvent};

/// Start the Unix socket IPC server. Handles status/stop/restart/logs/history requests.
pub async fn serve(sup: Arc<Supervisor>) {
    let path = socket_path();
    let _ = std::fs::remove_file(&path);

    let listener = match UnixListener::bind(&path) {
        Ok(l) => l,
        Err(e) => {
            tracing::error!("IPC socket bind failed: {e}");
            return;
        }
    };

    tracing::debug!("IPC socket at {}", path.display());

    loop {
        let (stream, _) = match listener.accept().await {
            Ok(s) => s,
            Err(e) => {
                tracing::warn!("IPC accept error: {e}");
                continue;
            }
        };

        let sup = sup.clone();
        tokio::spawn(async move {
            let (reader, mut writer) = tokio::io::split(stream);
            let mut lines = BufReader::new(reader).lines();

            while let Ok(Some(line)) = lines.next_line().await {
                let req: IpcRequest = match serde_json::from_str(&line) {
                    Ok(r) => r,
                    Err(e) => {
                        let resp = IpcResponse::Error {
                            msg: format!("bad request: {e}"),
                        };
                        let _ = writer
                            .write_all(
                                format!("{}\n", serde_json::to_string(&resp).unwrap()).as_bytes(),
                            )
                            .await;
                        continue;
                    }
                };

                match req {
                    IpcRequest::Status => {
                        let rows = sup.status_rows().await;
                        let resp = IpcResponse::Status { rows };
                        let _ = writer
                            .write_all(
                                format!("{}\n", serde_json::to_string(&resp).unwrap()).as_bytes(),
                            )
                            .await;
                    }

                    IpcRequest::Stop { services } => {
                        if services.is_empty() {
                            sup.stop_all().await;
                        } else {
                            for name in &services {
                                sup.stop_service(name).await;
                            }
                        }
                        let _ = writer
                            .write_all(
                                format!("{}\n", serde_json::to_string(&IpcResponse::Ok).unwrap())
                                    .as_bytes(),
                            )
                            .await;
                    }

                    IpcRequest::Restart { service } => {
                        let resp = match sup.restart_service(&service).await {
                            Ok(_) => IpcResponse::Ok,
                            Err(e) => IpcResponse::Error { msg: e.to_string() },
                        };
                        let _ = writer
                            .write_all(
                                format!("{}\n", serde_json::to_string(&resp).unwrap()).as_bytes(),
                            )
                            .await;
                    }

                    IpcRequest::Logs { service, follow } => {
                        let mut rx = sup.subscribe_events();
                        loop {
                            match rx.recv().await {
                                Ok(SupervisorEvent::LogLine { service: svc, line }) => {
                                    if service.as_deref().is_none_or(|f| f == svc) {
                                        let resp = IpcResponse::LogLine { service: svc, line };
                                        if writer
                                            .write_all(
                                                format!(
                                                    "{}\n",
                                                    serde_json::to_string(&resp).unwrap()
                                                )
                                                .as_bytes(),
                                            )
                                            .await
                                            .is_err()
                                        {
                                            break;
                                        }
                                    }
                                }
                                Ok(_) => {}
                                Err(broadcast::error::RecvError::Closed) => break,
                                Err(broadcast::error::RecvError::Lagged(_)) => {}
                            }
                            if !follow {
                                break;
                            }
                        }
                    }

                    IpcRequest::History { service, lines } => {
                        let recent = sup.log_history(service.as_deref(), lines);
                        for entry in recent {
                            let resp = IpcResponse::LogLine {
                                service: entry.service,
                                line: entry.line,
                            };
                            if writer
                                .write_all(
                                    format!("{}\n", serde_json::to_string(&resp).unwrap())
                                        .as_bytes(),
                                )
                                .await
                                .is_err()
                            {
                                break;
                            }
                        }
                        // Close connection after sending all history lines
                        break;
                    }
                }
            }
        });
    }
}
