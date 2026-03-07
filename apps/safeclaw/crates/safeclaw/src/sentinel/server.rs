//! Sentinel daemon server — runs inside the child process.
//!
//! Listens on a Unix Domain Socket, accepts connections from the parent
//! SafeClaw process, and dispatches analysis requests to `SentinelAgent`.
//! Each accepted connection is handled concurrently in its own Tokio task.

use super::ipc::{SentinelRequest, SentinelResponse};
use super::SentinelAgent;
use std::sync::Arc;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixListener;

/// Run the sentinel daemon server loop.
///
/// Removes any stale socket file before binding.  Runs until the listener
/// encounters an unrecoverable error (which terminates the child process).
pub async fn run_server(socket_path: &str, sentinel: Arc<SentinelAgent>) -> anyhow::Result<()> {
    // Remove stale socket left by a previous run.
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    tracing::info!(socket = %socket_path, "Sentinel daemon server listening");

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_connection(stream, sentinel.clone()));
            }
            Err(e) => tracing::warn!("Sentinel daemon accept error: {e}"),
        }
    }
}

// ── Connection handler ────────────────────────────────────────────────────────

async fn handle_connection(stream: tokio::net::UnixStream, sentinel: Arc<SentinelAgent>) {
    let (reader, mut writer) = stream.into_split();
    let mut lines = BufReader::new(reader).lines();

    while let Ok(Some(line)) = lines.next_line().await {
        let request: SentinelRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                tracing::debug!("Sentinel daemon: malformed request: {e}");
                continue;
            }
        };

        let needs_reply = request.expects_response();
        let response = dispatch(&sentinel, request).await;

        if needs_reply {
            let mut out = serde_json::to_string(&response).unwrap_or_default();
            out.push('\n');
            if writer.write_all(out.as_bytes()).await.is_err() {
                break;
            }
        }
    }
}

// ── Dispatcher ────────────────────────────────────────────────────────────────

async fn dispatch(sentinel: &SentinelAgent, request: SentinelRequest) -> SentinelResponse {
    match request {
        SentinelRequest::AnalyzeToolUse { session_id, tool, args } => {
            let v = sentinel.analyze_pre_tool_use(&session_id, &tool, &args).await;
            verdict_to_response(v)
        }
        SentinelRequest::AnalyzePrompt { session_id, prompt } => {
            let v = sentinel.analyze_prompt(&session_id, &prompt).await;
            verdict_to_response(v)
        }
        SentinelRequest::AnalyzeResponse { response } => {
            let v = sentinel.analyze_response(&response).await;
            verdict_to_response(v)
        }
        SentinelRequest::SessionCreated { session_id, parent_id } => {
            sentinel.on_session_created(&session_id, parent_id.as_deref());
            SentinelResponse::allow()
        }
        SentinelRequest::SessionDestroyed { session_id } => {
            sentinel.on_session_destroyed(&session_id);
            SentinelResponse::allow()
        }
        SentinelRequest::Ping => SentinelResponse::allow(),
    }
}

fn verdict_to_response(v: super::SecurityVerdict) -> SentinelResponse {
    if v.should_block {
        SentinelResponse::block(v.reason.unwrap_or_default())
    } else {
        SentinelResponse::allow()
    }
}
