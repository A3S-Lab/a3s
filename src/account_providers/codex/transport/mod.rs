//! Codex Responses transport selection and retry state.
//!
//! Transport selection is shared by logical Codex sessions derived from one
//! account client. Once the route proves that WebSocket is unavailable, child
//! sessions inherit the HTTPS fallback instead of repeating the failed probe.

mod network;
mod proxy;

use async_trait::async_trait;
use serde_json::Value;
use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;
use tokio_util::sync::CancellationToken;

const MAX_RETRY_DELAY: Duration = Duration::from_secs(60);

pub(super) use network::NetworkWireClient;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransportKind {
    WebSocket,
    HttpSse,
}

impl TransportKind {
    fn encoded(self) -> u8 {
        match self {
            Self::WebSocket => 0,
            Self::HttpSse => 1,
        }
    }

    fn decoded(value: u8) -> Self {
        if value == Self::HttpSse.encoded() {
            Self::HttpSse
        } else {
            Self::WebSocket
        }
    }
}

#[derive(Clone, Debug)]
pub(super) struct WireRequest {
    pub(super) endpoint: String,
    pub(super) headers: Vec<(String, String)>,
    pub(super) body: Value,
}

#[derive(Debug)]
pub(super) struct WireStream {
    pub(super) kind: TransportKind,
    pub(super) events: mpsc::Receiver<Result<Value, TransportError>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TransportErrorKind {
    Cancelled,
    Http,
    Network,
    Protocol,
    StreamClosed,
}

#[derive(Clone, Debug, thiserror::Error)]
#[error("{message}")]
pub(super) struct TransportError {
    pub(super) kind: TransportErrorKind,
    pub(super) status: Option<u16>,
    pub(super) retry_after: Option<Duration>,
    pub(super) body: Option<String>,
    message: String,
}

impl TransportError {
    pub(super) fn cancelled() -> Self {
        Self::new(TransportErrorKind::Cancelled, "Codex request cancelled")
    }

    pub(super) fn network(message: impl Into<String>) -> Self {
        Self::new(TransportErrorKind::Network, message)
    }

    pub(super) fn protocol(message: impl Into<String>) -> Self {
        Self::new(TransportErrorKind::Protocol, message)
    }

    pub(super) fn stream_closed(message: impl Into<String>) -> Self {
        Self::new(TransportErrorKind::StreamClosed, message)
    }

    pub(super) fn http(status: u16, body: Option<String>, retry_after: Option<Duration>) -> Self {
        let body = body.map(bounded_error_body);
        let excerpt = body
            .as_deref()
            .map(safe_body_excerpt)
            .filter(|value| !value.is_empty());
        let message = excerpt.as_ref().map_or_else(
            || format!("Codex transport returned HTTP {status}"),
            |excerpt| format!("Codex transport returned HTTP {status}: {excerpt}"),
        );
        Self {
            kind: TransportErrorKind::Http,
            status: Some(status),
            retry_after,
            body,
            message,
        }
    }

    fn new(kind: TransportErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            status: None,
            retry_after: None,
            body: None,
            message: message.into(),
        }
    }

    fn is_cancelled(&self) -> bool {
        self.kind == TransportErrorKind::Cancelled
    }

    fn is_unauthorized(&self) -> bool {
        self.status == Some(401)
    }

    fn requires_immediate_http_fallback(&self) -> bool {
        matches!(self.status, Some(403 | 426))
    }

    fn is_terminal_usage_limit(&self) -> bool {
        if self.status != Some(429) {
            return false;
        }
        let Some(error) = self
            .body
            .as_deref()
            .and_then(|body| serde_json::from_str::<Value>(body).ok())
            .and_then(|value| value.get("error").cloned())
        else {
            return false;
        };
        let is_usage_limit = [error.get("type"), error.get("code")]
            .into_iter()
            .flatten()
            .filter_map(Value::as_str)
            .any(|value| value == "usage_limit_reached");
        is_usage_limit
    }

    fn is_retryable(&self) -> bool {
        match self.kind {
            TransportErrorKind::Cancelled | TransportErrorKind::Protocol => false,
            TransportErrorKind::Network | TransportErrorKind::StreamClosed => true,
            TransportErrorKind::Http => {
                !self.is_terminal_usage_limit()
                    && matches!(self.status, Some(408 | 409 | 425 | 429) | Some(500..=599))
            }
        }
    }

    fn permits_websocket_fallback(&self) -> bool {
        !self.is_cancelled() && !self.is_unauthorized()
    }
}

fn safe_body_excerpt(body: &str) -> String {
    const MAX_CHARS: usize = 512;
    let normalized = body.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut excerpt = normalized.chars().take(MAX_CHARS).collect::<String>();
    if normalized.chars().count() > MAX_CHARS {
        excerpt.push('…');
    }
    excerpt
}

fn bounded_error_body(body: String) -> String {
    const MAX_CHARS: usize = 16 * 1024;
    if body.chars().count() <= MAX_CHARS {
        body
    } else {
        body.chars().take(MAX_CHARS).collect()
    }
}

#[async_trait]
pub(super) trait WireClient: Send + Sync {
    async fn open_websocket(
        &self,
        request: &WireRequest,
        cancel: CancellationToken,
    ) -> Result<WireStream, TransportError>;

    async fn open_http_sse(
        &self,
        request: &WireRequest,
        cancel: CancellationToken,
    ) -> Result<WireStream, TransportError>;
}

#[derive(Clone, Debug)]
pub(super) struct TransportConfig {
    pub(super) websocket_retries: u32,
    pub(super) http_retries: u32,
    pub(super) retry_base: Duration,
}

impl Default for TransportConfig {
    fn default() -> Self {
        Self {
            websocket_retries: 2,
            http_retries: 2,
            retry_base: Duration::from_millis(350),
        }
    }
}

struct TransportState {
    active: AtomicU8,
    wire: Arc<dyn WireClient>,
    config: TransportConfig,
}

#[derive(Clone)]
pub(super) struct TransportController {
    state: Arc<TransportState>,
}

impl TransportController {
    pub(super) fn new(wire: Arc<dyn WireClient>) -> Self {
        Self::with_config(wire, TransportConfig::default())
    }

    pub(super) fn with_config(wire: Arc<dyn WireClient>, config: TransportConfig) -> Self {
        Self {
            state: Arc::new(TransportState {
                active: AtomicU8::new(TransportKind::WebSocket.encoded()),
                wire,
                config,
            }),
        }
    }

    pub(super) fn active_kind(&self) -> TransportKind {
        TransportKind::decoded(self.state.active.load(Ordering::Acquire))
    }

    pub(super) async fn open(
        &self,
        request: &WireRequest,
        cancel: CancellationToken,
    ) -> Result<WireStream, TransportError> {
        if cancel.is_cancelled() {
            return Err(TransportError::cancelled());
        }

        if self.active_kind() == TransportKind::HttpSse {
            return self.open_http_with_retry(request, cancel).await;
        }

        let mut attempt = 0;
        loop {
            match self
                .state
                .wire
                .open_websocket(request, cancel.clone())
                .await
            {
                Ok(stream) => return Ok(stream),
                Err(error) if error.is_cancelled() || error.is_unauthorized() => {
                    return Err(error);
                }
                Err(error) if error.is_terminal_usage_limit() => return Err(error),
                Err(error)
                    if error.requires_immediate_http_fallback()
                        || (error.permits_websocket_fallback()
                            && attempt >= self.state.config.websocket_retries) =>
                {
                    self.activate_http(&error);
                    break;
                }
                Err(error) if error.permits_websocket_fallback() => {
                    attempt += 1;
                    tracing::warn!(
                        attempt,
                        max_retries = self.state.config.websocket_retries,
                        error = %error,
                        "Codex WebSocket connection failed; retrying"
                    );
                    self.wait_before_retry(attempt, error.retry_after, &cancel)
                        .await?;
                }
                Err(error) => return Err(error),
            }
        }

        self.open_http_with_retry(request, cancel).await
    }

    /// A WebSocket can fail after setup, when `open()` has already returned a
    /// stream to the LLM adapter. Mark the session so the Core retry replays the
    /// same turn over HTTPS rather than entering an endless WebSocket loop.
    pub(super) fn note_stream_failure(&self, kind: TransportKind, error: &TransportError) {
        if kind == TransportKind::WebSocket && !error.is_cancelled() {
            self.activate_http(error);
        }
    }

    async fn open_http_with_retry(
        &self,
        request: &WireRequest,
        cancel: CancellationToken,
    ) -> Result<WireStream, TransportError> {
        let mut attempt = 0;
        loop {
            match self.state.wire.open_http_sse(request, cancel.clone()).await {
                Ok(stream) => return Ok(stream),
                Err(error) if error.is_retryable() && attempt < self.state.config.http_retries => {
                    attempt += 1;
                    tracing::warn!(
                        attempt,
                        max_retries = self.state.config.http_retries,
                        error = %error,
                        "Codex HTTPS SSE request failed; retrying"
                    );
                    self.wait_before_retry(attempt, error.retry_after, &cancel)
                        .await?;
                }
                Err(error) => return Err(error),
            }
        }
    }

    fn activate_http(&self, error: &TransportError) {
        let previous = self
            .state
            .active
            .swap(TransportKind::HttpSse.encoded(), Ordering::AcqRel);
        if TransportKind::decoded(previous) != TransportKind::HttpSse {
            tracing::warn!(
                error = %error,
                "Falling back from Codex WebSocket to HTTPS SSE for this session"
            );
        }
    }

    async fn wait_before_retry(
        &self,
        attempt: u32,
        retry_after: Option<Duration>,
        cancel: &CancellationToken,
    ) -> Result<(), TransportError> {
        let exponent = attempt.saturating_sub(1).min(6);
        let delay = retry_after
            .unwrap_or_else(|| {
                self.state
                    .config
                    .retry_base
                    .saturating_mul(2_u32.saturating_pow(exponent))
            })
            .min(MAX_RETRY_DELAY);
        tokio::select! {
            biased;
            _ = cancel.cancelled() => Err(TransportError::cancelled()),
            _ = tokio::time::sleep(delay) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests;
