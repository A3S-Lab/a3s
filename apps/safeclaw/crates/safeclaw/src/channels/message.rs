//! Message types for channel communication

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Inbound message from a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InboundMessage {
    /// Unique message ID
    pub id: String,
    /// Channel identifier (e.g., "telegram", "slack")
    pub channel: String,
    /// Channel-specific message ID
    pub channel_message_id: String,
    /// Sender identifier
    pub sender_id: String,
    /// Sender display name
    pub sender_name: Option<String>,
    /// Chat/conversation ID
    pub chat_id: String,
    /// Message content
    pub content: String,
    /// Attachments
    pub attachments: Vec<MessageAttachment>,
    /// Whether this is a direct message
    pub is_dm: bool,
    /// Whether the bot was mentioned
    pub is_mention: bool,
    /// Reply to message ID (if this is a reply)
    pub reply_to: Option<String>,
    /// Timestamp
    pub timestamp: i64,
    /// Raw channel-specific data
    pub raw: Option<serde_json::Value>,
}

impl InboundMessage {
    /// Create a new inbound message
    pub fn new(channel: &str, sender_id: &str, chat_id: &str, content: &str) -> Self {
        Self {
            id: Uuid::new_v4().to_string(),
            channel: channel.to_string(),
            channel_message_id: String::new(),
            sender_id: sender_id.to_string(),
            sender_name: None,
            chat_id: chat_id.to_string(),
            content: content.to_string(),
            attachments: Vec::new(),
            is_dm: false,
            is_mention: false,
            reply_to: None,
            timestamp: chrono::Utc::now().timestamp_millis(),
            raw: None,
        }
    }

    /// Set sender name
    pub fn with_sender_name(mut self, name: impl Into<String>) -> Self {
        self.sender_name = Some(name.into());
        self
    }

    /// Set as direct message
    pub fn as_dm(mut self) -> Self {
        self.is_dm = true;
        self
    }

    /// Set as mention
    pub fn as_mention(mut self) -> Self {
        self.is_mention = true;
        self
    }

    /// Add attachment
    pub fn with_attachment(mut self, attachment: MessageAttachment) -> Self {
        self.attachments.push(attachment);
        self
    }
}

/// Outbound message to send to a channel
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OutboundMessage {
    /// Target channel
    pub channel: String,
    /// Target chat ID
    pub chat_id: String,
    /// Message content
    pub content: String,
    /// Attachments
    pub attachments: Vec<MessageAttachment>,
    /// Reply to message ID
    pub reply_to: Option<String>,
    /// Message format (text, markdown, html)
    pub format: MessageFormat,
    /// Whether to send typing indicator first
    pub show_typing: bool,
    /// Optional custom card payload (channel-specific JSON).
    /// When set, the adapter sends this card instead of wrapping `content` in a default card.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub card: Option<serde_json::Value>,
}

impl OutboundMessage {
    /// Create a new outbound message
    pub fn new(channel: &str, chat_id: &str, content: &str) -> Self {
        Self {
            channel: channel.to_string(),
            chat_id: chat_id.to_string(),
            content: content.to_string(),
            attachments: Vec::new(),
            reply_to: None,
            format: MessageFormat::Markdown,
            show_typing: true,
            card: None,
        }
    }

    /// Set reply to
    pub fn reply_to(mut self, message_id: impl Into<String>) -> Self {
        self.reply_to = Some(message_id.into());
        self
    }

    /// Set format
    pub fn with_format(mut self, format: MessageFormat) -> Self {
        self.format = format;
        self
    }

    /// Add attachment
    pub fn with_attachment(mut self, attachment: MessageAttachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Disable typing indicator
    pub fn no_typing(mut self) -> Self {
        self.show_typing = false;
        self
    }

    /// Set a custom card payload (channel-specific JSON)
    pub fn with_card(mut self, card: serde_json::Value) -> Self {
        self.card = Some(card);
        self
    }
}

/// Message format
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum MessageFormat {
    /// Plain text
    Text,
    /// Markdown format
    #[default]
    Markdown,
    /// HTML format
    Html,
}

/// Message attachment
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MessageAttachment {
    /// Attachment type
    pub attachment_type: AttachmentType,
    /// File name
    pub filename: Option<String>,
    /// MIME type
    pub mime_type: Option<String>,
    /// File size in bytes
    pub size: Option<u64>,
    /// URL to download the attachment
    pub url: Option<String>,
    /// Base64 encoded data (for small attachments)
    pub data: Option<String>,
    /// Thumbnail URL (for images/videos)
    pub thumbnail_url: Option<String>,
    /// Duration in seconds (for audio/video)
    pub duration: Option<u32>,
    /// Width (for images/videos)
    pub width: Option<u32>,
    /// Height (for images/videos)
    pub height: Option<u32>,
}

impl MessageAttachment {
    /// Create a new attachment
    pub fn new(attachment_type: AttachmentType) -> Self {
        Self {
            attachment_type,
            filename: None,
            mime_type: None,
            size: None,
            url: None,
            data: None,
            thumbnail_url: None,
            duration: None,
            width: None,
            height: None,
        }
    }

    /// Create an image attachment
    pub fn image(url: impl Into<String>) -> Self {
        Self {
            attachment_type: AttachmentType::Image,
            url: Some(url.into()),
            ..Self::new(AttachmentType::Image)
        }
    }

    /// Create a document attachment
    pub fn document(url: impl Into<String>, filename: impl Into<String>) -> Self {
        Self {
            attachment_type: AttachmentType::Document,
            url: Some(url.into()),
            filename: Some(filename.into()),
            ..Self::new(AttachmentType::Document)
        }
    }

    /// Set filename
    pub fn with_filename(mut self, filename: impl Into<String>) -> Self {
        self.filename = Some(filename.into());
        self
    }

    /// Set MIME type
    pub fn with_mime_type(mut self, mime_type: impl Into<String>) -> Self {
        self.mime_type = Some(mime_type.into());
        self
    }

    /// Set size
    pub fn with_size(mut self, size: u64) -> Self {
        self.size = Some(size);
        self
    }
}

/// Attachment type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttachmentType {
    /// Image file
    Image,
    /// Video file
    Video,
    /// Audio file
    Audio,
    /// Voice message
    Voice,
    /// Document/file
    Document,
    /// Sticker
    Sticker,
    /// Location
    Location,
    /// Contact
    Contact,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_inbound_message() {
        let msg = InboundMessage::new("telegram", "user123", "chat456", "Hello!")
            .with_sender_name("John")
            .as_dm();

        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.sender_id, "user123");
        assert!(msg.is_dm);
        assert_eq!(msg.sender_name, Some("John".to_string()));
    }

    #[test]
    fn test_outbound_message() {
        let msg = OutboundMessage::new("telegram", "chat456", "Hello back!")
            .reply_to("msg123")
            .with_format(MessageFormat::Markdown);

        assert_eq!(msg.channel, "telegram");
        assert_eq!(msg.reply_to, Some("msg123".to_string()));
        assert_eq!(msg.format, MessageFormat::Markdown);
    }

    #[test]
    fn test_attachment() {
        let attachment = MessageAttachment::image("https://example.com/image.jpg")
            .with_filename("image.jpg")
            .with_size(1024);

        assert_eq!(attachment.attachment_type, AttachmentType::Image);
        assert_eq!(attachment.filename, Some("image.jpg".to_string()));
        assert_eq!(attachment.size, Some(1024));
    }
}
