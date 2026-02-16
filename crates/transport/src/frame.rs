//! Wire frame format: `[type:u8][length:u32 big-endian][payload:length bytes]`

use crate::TransportError;

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
pub(crate) const HEADER_SIZE: usize = 5;

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
    fn test_frame_incomplete() {
        let original = Frame::data(b"hello".to_vec());
        let encoded = original.encode().unwrap();
        // Only pass partial data
        assert!(Frame::decode(&encoded[..3]).unwrap().is_none());
        assert!(Frame::decode(&encoded[..HEADER_SIZE]).unwrap().is_none());
    }

    #[test]
    fn test_frame_payload_too_large() {
        let mut buf = vec![0x01]; // Data type
        buf.extend_from_slice(&(MAX_PAYLOAD_SIZE + 1).to_be_bytes());
        assert!(Frame::decode(&buf).is_err());
    }

    #[test]
    fn test_frame_unknown_type() {
        let buf = [0xFF, 0x00, 0x00, 0x00, 0x00];
        assert!(Frame::decode(&buf).is_err());
    }

    #[test]
    fn test_frame_types() {
        let heartbeat = Frame::heartbeat();
        assert_eq!(heartbeat.frame_type, FrameType::Heartbeat);
        assert!(heartbeat.payload.is_empty());

        let close = Frame::close();
        assert_eq!(close.frame_type, FrameType::Close);

        let err = Frame::error("oops");
        assert_eq!(err.frame_type, FrameType::Error);
        assert_eq!(err.payload, b"oops");

        let ctrl = Frame::control(b"cmd".to_vec());
        assert_eq!(ctrl.frame_type, FrameType::Control);
    }

    #[test]
    fn test_frame_empty_payload() {
        let frame = Frame::data(vec![]);
        let encoded = frame.encode().unwrap();
        assert_eq!(encoded.len(), HEADER_SIZE);
        let (decoded, _) = Frame::decode(&encoded).unwrap().unwrap();
        assert!(decoded.payload.is_empty());
    }
}
