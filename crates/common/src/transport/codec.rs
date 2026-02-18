//! Async frame reader/writer for any `AsyncRead`/`AsyncWrite` stream.
//!
//! Wraps the [`Frame`] wire format with buffered async I/O.

use bytes::{Buf, BytesMut};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::frame::Frame;
use super::TransportError;

const INITIAL_BUF_CAPACITY: usize = 8 * 1024;

/// Async frame reader over any `AsyncRead` stream.
///
/// Buffers incoming bytes and yields complete [`Frame`]s.
#[derive(Debug)]
pub struct FrameReader<R> {
    inner: R,
    buf: BytesMut,
}

impl<R: AsyncRead + Unpin> FrameReader<R> {
    /// Wrap a reader.
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            buf: BytesMut::with_capacity(INITIAL_BUF_CAPACITY),
        }
    }

    /// Read the next frame. Returns `None` on clean EOF.
    pub async fn read_frame(&mut self) -> Result<Option<Frame>, TransportError> {
        loop {
            // Try to decode a frame from the buffer
            if let Some((frame, consumed)) = Frame::decode(&self.buf)? {
                self.buf.advance(consumed);
                return Ok(Some(frame));
            }

            // Need more data — read from the stream
            let n = self
                .inner
                .read_buf(&mut self.buf)
                .await
                .map_err(|e| TransportError::RecvFailed(e.to_string()))?;

            if n == 0 {
                // EOF
                if self.buf.is_empty() {
                    return Ok(None);
                }
                return Err(TransportError::RecvFailed(
                    "Connection closed with incomplete frame".to_string(),
                ));
            }
        }
    }

    /// Get a reference to the inner reader.
    pub fn inner(&self) -> &R {
        &self.inner
    }

    /// Consume the reader and return the inner stream.
    pub fn into_inner(self) -> R {
        self.inner
    }
}

/// Async frame writer over any `AsyncWrite` stream.
#[derive(Debug)]
pub struct FrameWriter<W> {
    inner: W,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
    /// Wrap a writer.
    pub fn new(inner: W) -> Self {
        Self { inner }
    }

    /// Write a frame to the stream.
    pub async fn write_frame(&mut self, frame: &Frame) -> Result<(), TransportError> {
        let encoded = frame.encode()?;
        self.inner
            .write_all(&encoded)
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        self.inner
            .flush()
            .await
            .map_err(|e| TransportError::SendFailed(e.to_string()))?;
        Ok(())
    }

    /// Write a data frame with the given payload.
    pub async fn write_data(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        self.write_frame(&Frame::data(payload.to_vec())).await
    }

    /// Write a control frame with the given payload.
    pub async fn write_control(&mut self, payload: &[u8]) -> Result<(), TransportError> {
        self.write_frame(&Frame::control(payload.to_vec())).await
    }

    /// Write a JSON-serializable value as a data frame.
    pub async fn write_json<T: serde::Serialize>(
        &mut self,
        value: &T,
    ) -> Result<(), TransportError> {
        let payload = serde_json::to_vec(value)
            .map_err(|e| TransportError::SendFailed(format!("JSON serialize: {}", e)))?;
        self.write_data(&payload).await
    }

    /// Get a reference to the inner writer.
    pub fn inner(&self) -> &W {
        &self.inner
    }

    /// Consume the writer and return the inner stream.
    pub fn into_inner(self) -> W {
        self.inner
    }
}

/// Combined async frame reader + writer over a split stream.
#[derive(Debug)]
pub struct FrameCodec<R, W> {
    pub reader: FrameReader<R>,
    pub writer: FrameWriter<W>,
}

impl<R: AsyncRead + Unpin, W: AsyncWrite + Unpin> FrameCodec<R, W> {
    /// Create from separate read and write halves.
    pub fn new(reader: R, writer: W) -> Self {
        Self {
            reader: FrameReader::new(reader),
            writer: FrameWriter::new(writer),
        }
    }

    /// Read the next frame.
    pub async fn read_frame(&mut self) -> Result<Option<Frame>, TransportError> {
        self.reader.read_frame().await
    }

    /// Write a frame.
    pub async fn write_frame(&mut self, frame: &Frame) -> Result<(), TransportError> {
        self.writer.write_frame(frame).await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::super::frame::FrameType;
    use super::*;

    #[tokio::test]
    async fn test_reader_writer_roundtrip() {
        let (client, server) = tokio::io::duplex(1024);
        let (cr, cw) = tokio::io::split(client);
        let (sr, _sw) = tokio::io::split(server);

        let mut writer = FrameWriter::new(cw);
        let mut reader = FrameReader::new(sr);

        writer.write_data(b"hello").await.unwrap();
        writer.write_frame(&Frame::heartbeat()).await.unwrap();
        // Drop both writer and unused read half so the DuplexStream is fully
        // released and the server read half sees EOF.
        drop(writer);
        drop(cr);

        let f1 = reader.read_frame().await.unwrap().unwrap();
        assert_eq!(f1.frame_type, FrameType::Data);
        assert_eq!(f1.payload, b"hello");

        let f2 = reader.read_frame().await.unwrap().unwrap();
        assert_eq!(f2.frame_type, FrameType::Heartbeat);
        assert!(f2.payload.is_empty());

        let f3 = reader.read_frame().await.unwrap();
        assert!(f3.is_none()); // EOF
    }

    #[tokio::test]
    async fn test_codec_bidirectional() {
        let (a, b) = tokio::io::duplex(1024);
        let (ar, aw) = tokio::io::split(a);
        let (br, bw) = tokio::io::split(b);

        let mut codec_a = FrameCodec::new(ar, aw);
        let mut codec_b = FrameCodec::new(br, bw);

        codec_a.writer.write_data(b"ping").await.unwrap();
        let frame = codec_b.reader.read_frame().await.unwrap().unwrap();
        assert_eq!(frame.payload, b"ping");

        codec_b.writer.write_data(b"pong").await.unwrap();
        let frame = codec_a.reader.read_frame().await.unwrap().unwrap();
        assert_eq!(frame.payload, b"pong");
    }

    #[tokio::test]
    async fn test_write_json() {
        let (client, server) = tokio::io::duplex(1024);
        let (_, cw) = tokio::io::split(client);
        let (sr, _) = tokio::io::split(server);

        let mut writer = FrameWriter::new(cw);
        let mut reader = FrameReader::new(sr);

        #[derive(serde::Serialize, serde::Deserialize, Debug, PartialEq)]
        struct Msg {
            text: String,
        }

        writer
            .write_json(&Msg {
                text: "hello".into(),
            })
            .await
            .unwrap();
        drop(writer);

        let frame = reader.read_frame().await.unwrap().unwrap();
        let msg: Msg = serde_json::from_slice(&frame.payload).unwrap();
        assert_eq!(msg.text, "hello");
    }

    #[tokio::test]
    async fn test_reader_incomplete_frame_on_eof() {
        // Write only a partial header
        let (client, server) = tokio::io::duplex(1024);
        let (_, mut cw) = tokio::io::split(client);
        let (sr, _) = tokio::io::split(server);

        let mut reader = FrameReader::new(sr);

        // Write 3 bytes (incomplete header) then close
        cw.write_all(&[0x01, 0x00, 0x00]).await.unwrap();
        drop(cw);

        let result = reader.read_frame().await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_multiple_frames_in_one_read() {
        let (client, server) = tokio::io::duplex(4096);
        let (_, mut cw) = tokio::io::split(client);
        let (sr, _) = tokio::io::split(server);

        // Encode two frames and write them in a single write
        let f1 = Frame::data(b"first".to_vec());
        let f2 = Frame::data(b"second".to_vec());
        let mut buf = f1.encode().unwrap();
        buf.extend_from_slice(&f2.encode().unwrap());
        cw.write_all(&buf).await.unwrap();
        drop(cw);

        let mut reader = FrameReader::new(sr);
        let r1 = reader.read_frame().await.unwrap().unwrap();
        assert_eq!(r1.payload, b"first");
        let r2 = reader.read_frame().await.unwrap().unwrap();
        assert_eq!(r2.payload, b"second");
        assert!(reader.read_frame().await.unwrap().is_none());
    }
}
