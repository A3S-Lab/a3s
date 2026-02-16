//! Unix domain socket transport.
//!
//! Cross-platform transport for local IPC. Useful for development on macOS
//! where vsock is not available, and for host-local communication.

use std::path::{Path, PathBuf};

use async_trait::async_trait;
use tokio::net::UnixStream;

use crate::codec::FrameCodec;
use crate::frame::Frame;
use crate::{Transport, TransportError};

/// Transport over a Unix domain socket.
#[derive(Debug)]
pub struct UnixTransport {
    path: PathBuf,
    codec: Option<FrameCodec<tokio::io::ReadHalf<UnixStream>, tokio::io::WriteHalf<UnixStream>>>,
}

impl UnixTransport {
    /// Create a new transport that will connect to the given socket path.
    pub fn new(path: impl AsRef<Path>) -> Self {
        Self {
            path: path.as_ref().to_path_buf(),
            codec: None,
        }
    }

    /// Create a transport from an already-connected `UnixStream`.
    pub fn from_stream(stream: UnixStream) -> Self {
        let (r, w) = tokio::io::split(stream);
        Self {
            path: PathBuf::new(),
            codec: Some(FrameCodec::new(r, w)),
        }
    }
}

#[async_trait]
impl Transport for UnixTransport {
    async fn connect(&mut self) -> Result<(), TransportError> {
        let stream = UnixStream::connect(&self.path)
            .await
            .map_err(|e| TransportError::ConnectionFailed(format!("{}: {}", self.path.display(), e)))?;
        let (r, w) = tokio::io::split(stream);
        self.codec = Some(FrameCodec::new(r, w));
        Ok(())
    }

    async fn send(&mut self, data: &[u8]) -> Result<(), TransportError> {
        let codec = self.codec.as_mut().ok_or(TransportError::NotConnected)?;
        codec.writer.write_data(data).await
    }

    async fn send_frame(&mut self, frame: &Frame) -> Result<(), TransportError> {
        let codec = self.codec.as_mut().ok_or(TransportError::NotConnected)?;
        codec.writer.write_frame(frame).await
    }

    async fn recv(&mut self) -> Result<Vec<u8>, TransportError> {
        let codec = self.codec.as_mut().ok_or(TransportError::NotConnected)?;
        match codec.reader.read_frame().await? {
            Some(frame) => Ok(frame.payload),
            None => Err(TransportError::Closed),
        }
    }

    async fn recv_frame(&mut self) -> Result<Option<Frame>, TransportError> {
        let codec = self.codec.as_mut().ok_or(TransportError::NotConnected)?;
        codec.reader.read_frame().await
    }

    async fn close(&mut self) -> Result<(), TransportError> {
        self.codec = None;
        Ok(())
    }

    fn is_connected(&self) -> bool {
        self.codec.is_some()
    }
}

/// Accept-side helper: listen on a Unix socket and yield transports.
pub struct UnixListener {
    inner: tokio::net::UnixListener,
}

impl UnixListener {
    /// Bind to a path. Removes existing socket file if present.
    pub fn bind(path: impl AsRef<Path>) -> Result<Self, TransportError> {
        let path = path.as_ref();
        // Remove stale socket
        let _ = std::fs::remove_file(path);
        let inner = tokio::net::UnixListener::bind(path)
            .map_err(|e| TransportError::ConnectionFailed(format!("{}: {}", path.display(), e)))?;
        Ok(Self { inner })
    }

    /// Accept the next connection.
    pub async fn accept(&self) -> Result<UnixTransport, TransportError> {
        let (stream, _addr) = self
            .inner
            .accept()
            .await
            .map_err(|e| TransportError::ConnectionFailed(e.to_string()))?;
        Ok(UnixTransport::from_stream(stream))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::FrameType;

    #[tokio::test]
    async fn test_unix_transport_roundtrip() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test.sock");

        let listener = UnixListener::bind(&sock_path).unwrap();

        let client_path = sock_path.clone();
        let client_handle = tokio::spawn(async move {
            let mut client = UnixTransport::new(&client_path);
            client.connect().await.unwrap();
            client.send(b"hello from client").await.unwrap();
            let reply = client.recv().await.unwrap();
            assert_eq!(reply, b"hello from server");
            client.close().await.unwrap();
        });

        let mut server = listener.accept().await.unwrap();
        let data = server.recv().await.unwrap();
        assert_eq!(data, b"hello from client");
        server.send(b"hello from server").await.unwrap();

        client_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_unix_transport_frame_types() {
        let dir = tempfile::tempdir().unwrap();
        let sock_path = dir.path().join("test2.sock");

        let listener = UnixListener::bind(&sock_path).unwrap();

        let client_path = sock_path.clone();
        let client_handle = tokio::spawn(async move {
            let mut client = UnixTransport::new(&client_path);
            client.connect().await.unwrap();
            client.send_frame(&Frame::heartbeat()).await.unwrap();
            client
                .send_frame(&Frame::error("test error"))
                .await
                .unwrap();
            client.send_frame(&Frame::close()).await.unwrap();
        });

        let mut server = listener.accept().await.unwrap();

        let f1 = server.recv_frame().await.unwrap().unwrap();
        assert_eq!(f1.frame_type, FrameType::Heartbeat);

        let f2 = server.recv_frame().await.unwrap().unwrap();
        assert_eq!(f2.frame_type, FrameType::Error);
        assert_eq!(f2.payload, b"test error");

        let f3 = server.recv_frame().await.unwrap().unwrap();
        assert_eq!(f3.frame_type, FrameType::Close);

        client_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_unix_not_connected() {
        let mut transport = UnixTransport::new("/nonexistent.sock");
        assert!(!transport.is_connected());
        assert!(transport.send(b"data").await.is_err());
        assert!(transport.recv().await.is_err());
    }
}
