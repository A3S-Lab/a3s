//! Shared transport abstraction for the A3S ecosystem.
//!
//! This crate provides:
//! - [`Transport`] trait — async send/recv abstraction over any byte stream
//! - [`Frame`] — unified wire format: `[type:u8][length:u32][payload]`
//! - [`MockTransport`] — in-memory transport for testing
//! - TEE protocol types for TEE communication

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use tokio::sync::Mutex;

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

/// Async transport trait for sending and receiving byte messages.
#[async_trait]
pub trait Transport: Send + Sync + std::fmt::Debug {
    async fn connect(&mut self) -> Result<(), TransportError>;
    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError>;
    async fn recv(&mut self) -> Result<Vec<u8>, TransportError>;
    async fn close(&mut self) -> Result<(), TransportError>;
    fn is_connected(&self) -> bool;
}

// ---------------------------------------------------------------------------
// Frame
// ---------------------------------------------------------------------------

/// Frame types for the wire protocol
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum FrameType {
    Data = 0x01,
    Control = 0x02,
    Heartbeat = 0x03,
    Error = 0x04,
    Close = 0x05,
}

impl TryFrom<u8> for FrameType {
    type Error = TransportError;
    fn try_from(value: u8) -> Result<Self, <Self as TryFrom<u8>>::Error> {
        match value {
            0x01 => Ok(Self::Data),
            0x02 => Ok(Self::Control),
            0x03 => Ok(Self::Heartbeat),
            0x04 => Ok(Self::Error),
            0x05 => Ok(Self::Close),
            _ => Err(TransportError::FrameError(format!(
                "Unknown frame type: 0x{:02x}",
                value
            ))),
        }
    }
}

/// Maximum payload size: 16 MiB
pub const MAX_PAYLOAD_SIZE: u32 = 16 * 1024 * 1024;
const HEADER_SIZE: usize = 5;

/// A framed message on the wire.
/// Wire format: `[type:u8][length:u32 big-endian][payload:length bytes]`
#[derive(Debug, Clone)]
pub struct Frame {
    pub frame_type: FrameType,
    pub payload: Vec<u8>,
}

impl Frame {
    /// Create a new data frame
    pub fn data(payload: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Data,
            payload,
        }
    }

    /// Create a control frame
    pub fn control(payload: Vec<u8>) -> Self {
        Self {
            frame_type: FrameType::Control,
            payload,
        }
    }

    /// Create a heartbeat frame
    pub fn heartbeat() -> Self {
        Self {
            frame_type: FrameType::Heartbeat,
            payload: Vec::new(),
        }
    }

    /// Create an error frame
    pub fn error(message: &str) -> Self {
        Self {
            frame_type: FrameType::Error,
            payload: message.as_bytes().to_vec(),
        }
    }

    /// Create a close frame
    pub fn close() -> Self {
        Self {
            frame_type: FrameType::Close,
            payload: Vec::new(),
        }
    }

    /// Encode this frame into bytes for the wire.
    pub fn encode(&self) -> Result<Vec<u8>, TransportError> {
        let len = self.payload.len() as u32;
        if len > MAX_PAYLOAD_SIZE {
            return Err(TransportError::FrameError(format!(
                "Payload too large: {} bytes (max {})",
                len, MAX_PAYLOAD_SIZE
            )));
        }
        let mut buf = Vec::with_capacity(HEADER_SIZE + self.payload.len());
        buf.push(self.frame_type as u8);
        buf.extend_from_slice(&len.to_be_bytes());
        buf.extend_from_slice(&self.payload);
        Ok(buf)
    }

    /// Decode a frame from bytes.
    /// Returns the frame and the number of bytes consumed, or None if incomplete.
    pub fn decode(buf: &[u8]) -> Result<Option<(Self, usize)>, TransportError> {
        if buf.len() < HEADER_SIZE {
            return Ok(None);
        }
        let frame_type = FrameType::try_from(buf[0])?;
        let len = u32::from_be_bytes([buf[1], buf[2], buf[3], buf[4]]);
        if len > MAX_PAYLOAD_SIZE {
            return Err(TransportError::FrameError(format!(
                "Payload too large: {} bytes (max {})",
                len, MAX_PAYLOAD_SIZE
            )));
        }
        let total = HEADER_SIZE + len as usize;
        if buf.len() < total {
            return Ok(None);
        }
        let payload = buf[HEADER_SIZE..total].to_vec();
        Ok(Some((
            Self {
                frame_type,
                payload,
            },
            total,
        )))
    }
}

// ---------------------------------------------------------------------------
// TEE Protocol types
// ---------------------------------------------------------------------------

/// Message types for TEE communication
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TeeMessage {
    Request(TeeRequest),
    Response(TeeResponse),
    Heartbeat { timestamp: i64 },
    Error { code: i32, message: String },
}

/// Request to TEE environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeRequest {
    pub id: String,
    pub session_id: String,
    pub request_type: TeeRequestType,
    pub payload: Vec<u8>,
    pub timestamp: i64,
}

impl TeeRequest {
    pub fn new(session_id: String, request_type: TeeRequestType, payload: Vec<u8>) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id,
            request_type,
            payload,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Types of TEE requests
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeRequestType {
    InitSession,
    ProcessMessage,
    ExecuteTool,
    StoreSecret,
    RetrieveSecret,
    DeleteSecret,
    GetSessionState,
    TerminateSession,
}

/// Response from TEE environment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeeResponse {
    pub request_id: String,
    pub session_id: String,
    pub status: TeeResponseStatus,
    pub payload: Vec<u8>,
    pub timestamp: i64,
}

impl TeeResponse {
    pub fn success(request_id: String, session_id: String, payload: Vec<u8>) -> Self {
        Self {
            request_id,
            session_id,
            status: TeeResponseStatus::Success,
            payload,
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub fn error(request_id: String, session_id: String, code: i32, message: String) -> Self {
        Self {
            request_id,
            session_id,
            status: TeeResponseStatus::Error { code, message },
            payload: Vec::new(),
            timestamp: chrono::Utc::now().timestamp_millis(),
        }
    }
}

/// Response status
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TeeResponseStatus {
    Success,
    Error { code: i32, message: String },
    Pending,
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
    /// Every `send()` call invokes the handler, and the return value
    /// is queued for the next `recv()`.
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

    #[test]
    fn test_frame_encode_decode_roundtrip() {
        let original = Frame::data(b"hello world".to_vec());
        let encoded = original.encode().unwrap();
        let (decoded, consumed) = Frame::decode(&encoded).unwrap().unwrap();
        assert_eq!(consumed, encoded.len());
        assert_eq!(decoded.frame_type, FrameType::Data);
        assert_eq!(decoded.payload, b"hello world");
    }

    #[test]
    fn test_tee_request_creation() {
        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::ProcessMessage,
            vec![1, 2, 3],
        );
        assert!(!request.id.is_empty());
        assert_eq!(request.session_id, "session-123");
    }

    #[test]
    fn test_tee_message_serialization() {
        let request = TeeRequest::new(
            "session-123".to_string(),
            TeeRequestType::InitSession,
            vec![],
        );
        let message = TeeMessage::Request(request);
        let json = serde_json::to_string(&message).unwrap();
        let parsed: TeeMessage = serde_json::from_str(&json).unwrap();
        assert!(matches!(parsed, TeeMessage::Request(_)));
    }

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
}
