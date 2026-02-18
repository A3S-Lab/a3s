//! Shared transport abstraction for the A3S ecosystem.
//!
//! This module provides:
//! - [`Transport`] trait — async send/recv abstraction over any byte stream
//! - [`Frame`] — unified wire format: `[type:u8][length:u32][payload]`
//! - [`FrameReader`] / [`FrameWriter`] — async buffered frame I/O
//! - [`UnixTransport`] — Unix domain socket transport (cross-platform)
//! - [`MockTransport`] — in-memory transport for testing
//! - TEE protocol types for secure communication

use async_trait::async_trait;
use std::collections::VecDeque;
use tokio::sync::Mutex;

pub mod codec;
pub mod frame;
pub mod tee;
pub mod unix;

// Re-exports for convenience
pub use codec::{FrameCodec, FrameReader, FrameWriter};
pub use frame::{Frame, FrameType, MAX_PAYLOAD_SIZE};
pub use tee::{TeeMessage, TeeRequest, TeeRequestType, TeeResponse, TeeResponseStatus};
pub use unix::{UnixListener, UnixTransport};

/// Well-known vsock port assignments
pub mod ports {
    /// gRPC agent control channel
    pub const GRPC_AGENT: u32 = 4088;
    /// Exec server (command execution in guest)
    pub const EXEC_SERVER: u32 = 4089;
    /// PTY server (interactive terminal)
    pub const PTY_SERVER: u32 = 4090;
    /// TEE secure channel (SafeClaw <-> a3s-code)
    pub const TEE_CHANNEL: u32 = 4091;
}

// ---------------------------------------------------------------------------
// Transport trait
// ---------------------------------------------------------------------------

/// Error type for transport operations
#[derive(Debug, thiserror::Error)]
pub enum TransportError {
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),
    #[error("Not connected")]
    NotConnected,
    #[error("Send failed: {0}")]
    SendFailed(String),
    #[error("Receive failed: {0}")]
    RecvFailed(String),
    #[error("Connection closed")]
    Closed,
    #[error("Operation timed out")]
    Timeout,
    #[error("Frame error: {0}")]
    FrameError(String),
    #[error("Protocol error: {0}")]
    Protocol(String),
}

/// Async transport trait for sending and receiving framed messages.
#[async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug {
    /// Establish the connection.
    async fn connect(&mut self) -> Result<(), TransportError>;

    /// Send raw bytes as a data frame.
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;

    /// Send a typed frame.
    async fn send_frame(&mut self, frame: &Frame) -> Result<(), TransportError> {
        // Default: encode frame and send payload only (for backward compat)
        self.send(&frame.encode()?).await
    }

    /// Receive raw bytes (payload of the next data frame).
    async fn recv(&mut self) -> Result<Vec<u8>, TransportError>;

    /// Receive a typed frame. Returns `None` on clean EOF.
    async fn recv_frame(&mut self) -> Result<Option<Frame>, TransportError> {
        // Default: wrap recv() in a data frame
        match self.recv().await {
            Ok(data) => Ok(Some(Frame::data(data))),
            Err(TransportError::Closed) => Ok(None),
            Err(e) => Err(e),
        }
    }

    /// Close the connection.
    async fn close(&mut self) -> Result<(), TransportError>;

    /// Check if connected.
    fn is_connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// MockTransport
// ---------------------------------------------------------------------------

/// Handler function type for mock responses
type ResponseHandler = Box<dyn Fn(&[u8]) -> Vec<u8> + Send + Sync>;

/// In-memory transport for testing.
pub struct MockTransport {
    connected: bool,
    recv_queue: Mutex<VecDeque<Vec<u8>>>,
    sent: Mutex<Vec<Vec<u8>>>,
    handler: Option<ResponseHandler>,
}

impl std::fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MockTransport")
            .field("connected", &self.connected)
            .finish_non_exhaustive()
    }
}

impl MockTransport {
    /// Create a new disconnected mock transport
    pub fn new() -> Self {
        Self {
            connected: false,
            recv_queue: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            handler: None,
        }
    }

    /// Create a mock transport with an auto-response handler.
    pub fn with_handler<F>(handler: F) -> Self
    where
        F: Fn(&[u8]) -> Vec<u8> + Send + Sync + 'static,
    {
        Self {
            connected: false,
            recv_queue: Mutex::new(VecDeque::new()),
            sent: Mutex::new(Vec::new()),
            handler: Some(Box::new(handler)),
        }
    }

    /// Push a message into the recv queue
    pub fn push_recv(&self, data: Vec<u8>) {
        if let Ok(mut queue) = self.recv_queue.try_lock() {
            queue.push_back(data);
        }
    }

    /// Get all sent messages
    pub async fn sent_messages(&self) -> Vec<Vec<u8>> {
        self.sent.lock().await.clone()
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        self.connected = true;
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        self.sent.lock().await.push(data.to_vec());
        if let Some(ref handler) = self.handler {
            let response = handler(data);
            self.recv_queue.lock().await.push_back(response);
        }
        Ok(())
    }

    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        if !self.connected {
            return Err(TransportError::NotConnected);
        }
        match self.recv_queue.lock().await.pop_front() {
            Some(data) => Ok(data),
            None => Err(TransportError::Closed),
        }
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.connected = false;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.connected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_mock_transport_handler() {
        let mut transport = MockTransport::with_handler(|data| {
            let mut resp = b"echo: ".to_vec();
            resp.extend_from_slice(data);
            resp
        });
        transport.connect().await.unwrap();
        transport.send(b"ping").await.unwrap();
        let response = transport.recv().await.unwrap();
        assert_eq!(response, b"echo: ping");
    }

    #[tokio::test]
    async fn test_mock_transport_not_connected() {
        let mut transport = MockTransport::new();
        assert!(!transport.is_connected());
        assert!(transport.send(b"data").await.is_err());
        assert!(transport.recv().await.is_err());
    }

    #[tokio::test]
    async fn test_mock_transport_push_recv() {
        let mut transport = MockTransport::new();
        transport.connect().await.unwrap();
        transport.push_recv(b"queued".to_vec());
        let data = transport.recv().await.unwrap();
        assert_eq!(data, b"queued");
    }

    #[tokio::test]
    async fn test_mock_transport_close() {
        let mut transport = MockTransport::new();
        transport.connect().await.unwrap();
        assert!(transport.is_connected());
        transport.close().await.unwrap();
        assert!(!transport.is_connected());
    }

    #[tokio::test]
    async fn test_mock_sent_messages() {
        let mut transport = MockTransport::new();
        transport.connect().await.unwrap();
        transport.send(b"msg1").await.unwrap();
        transport.send(b"msg2").await.unwrap();
        let sent = transport.sent_messages().await;
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0], b"msg1");
        assert_eq!(sent[1], b"msg2");
    }
}
