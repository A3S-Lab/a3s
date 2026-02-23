//! Structured message segments for architectural prompt injection defense.
//!
//! Separates user content from system instructions at the type level,
//! preventing injection attacks from crossing segment boundaries.
//!
//! **Threat model**: Defends against A1 (malicious user) at AS-1.
//! See `docs/threat-model.md` §4 AS-1.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// A typed message segment that enforces separation between system
/// instructions, user content, and tool outputs.
///
/// The injection detector only scans `User` segments, preventing
/// false positives on system prompts and tool outputs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "role", rename_all = "snake_case")]
pub enum MessageSegment {
    /// System instructions — immutable, never scanned for injection.
    System {
        content: String,
        /// If true, this segment cannot be overridden by later system segments.
        #[serde(default)]
        immutable: bool,
    },
    /// User-provided content — scanned for injection, carries taint labels.
    User {
        content: String,
        /// Taint labels from privacy classification (e.g. "email", "phone").
        #[serde(default, skip_serializing_if = "HashSet::is_empty")]
        taint: HashSet<String>,
    },
    /// Tool call output — not scanned for injection (trusted source).
    Tool {
        content: String,
        /// Name of the tool that produced this output.
        tool_name: String,
    },
    /// Assistant response — tagged with source segment attribution.
    Assistant {
        content: String,
        /// Which input segment indices influenced this response (best-effort).
        #[serde(default, skip_serializing_if = "Vec::is_empty")]
        source_segments: Vec<usize>,
    },
}

impl MessageSegment {
    /// Create a system segment.
    pub fn system(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
            immutable: false,
        }
    }

    /// Create an immutable system segment.
    pub fn system_immutable(content: impl Into<String>) -> Self {
        Self::System {
            content: content.into(),
            immutable: true,
        }
    }

    /// Create a user segment.
    pub fn user(content: impl Into<String>) -> Self {
        Self::User {
            content: content.into(),
            taint: HashSet::new(),
        }
    }

    /// Create a user segment with taint labels.
    pub fn user_with_taint(content: impl Into<String>, taint: HashSet<String>) -> Self {
        Self::User {
            content: content.into(),
            taint,
        }
    }

    /// Create a tool output segment.
    pub fn tool(tool_name: impl Into<String>, content: impl Into<String>) -> Self {
        Self::Tool {
            content: content.into(),
            tool_name: tool_name.into(),
        }
    }

    /// Create an assistant segment.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self::Assistant {
            content: content.into(),
            source_segments: Vec::new(),
        }
    }

    /// Get the text content regardless of segment type.
    pub fn content(&self) -> &str {
        match self {
            Self::System { content, .. }
            | Self::User { content, .. }
            | Self::Tool { content, .. }
            | Self::Assistant { content, .. } => content,
        }
    }

    /// Returns true if this is a user segment (should be scanned for injection).
    pub fn is_user(&self) -> bool {
        matches!(self, Self::User { .. })
    }

    /// Returns true if this is a system segment.
    pub fn is_system(&self) -> bool {
        matches!(self, Self::System { .. })
    }
}

/// A structured message composed of typed segments.
///
/// Enforces that system instructions and user content are never mixed
/// in the same segment, providing architectural defense against injection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredMessage {
    /// Ordered segments composing this message.
    pub segments: Vec<MessageSegment>,
    /// Optional canary token embedded in system segments for leak detection.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub canary_token: Option<String>,
}

impl StructuredMessage {
    /// Create a new structured message from segments.
    pub fn new(segments: Vec<MessageSegment>) -> Self {
        Self {
            segments,
            canary_token: None,
        }
    }

    /// Create a simple user message (most common case).
    pub fn from_user(content: impl Into<String>) -> Self {
        Self::new(vec![MessageSegment::user(content)])
    }

    /// Attach a canary token for prompt leakage detection.
    pub fn with_canary(mut self, token: String) -> Self {
        self.canary_token = Some(token);
        self
    }

    /// Extract only user segments (for injection scanning).
    pub fn user_segments(&self) -> Vec<(usize, &MessageSegment)> {
        self.segments
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_user())
            .collect()
    }

    /// Extract only system segments.
    pub fn system_segments(&self) -> Vec<(usize, &MessageSegment)> {
        self.segments
            .iter()
            .enumerate()
            .filter(|(_, s)| s.is_system())
            .collect()
    }

    /// Concatenate all user segment content (for legacy APIs that expect a single string).
    pub fn user_content(&self) -> String {
        self.segments
            .iter()
            .filter_map(|s| match s {
                MessageSegment::User { content, .. } => Some(content.as_str()),
                _ => None,
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Collect all taint labels from user segments.
    pub fn taint_labels(&self) -> HashSet<String> {
        let mut labels = HashSet::new();
        for seg in &self.segments {
            if let MessageSegment::User { taint, .. } = seg {
                labels.extend(taint.iter().cloned());
            }
        }
        labels
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_segment_constructors() {
        let sys = MessageSegment::system("You are a helpful assistant");
        assert!(sys.is_system());
        assert!(!sys.is_user());
        assert_eq!(sys.content(), "You are a helpful assistant");

        let user = MessageSegment::user("Hello!");
        assert!(user.is_user());
        assert!(!user.is_system());

        let tool = MessageSegment::tool("bash", "output here");
        assert!(!tool.is_user());
        assert_eq!(tool.content(), "output here");
    }

    #[test]
    fn test_immutable_system() {
        let seg = MessageSegment::system_immutable("core rules");
        if let MessageSegment::System { immutable, .. } = seg {
            assert!(immutable);
        } else {
            panic!("expected System");
        }
    }

    #[test]
    fn test_structured_message_user_segments() {
        let msg = StructuredMessage::new(vec![
            MessageSegment::system("Be helpful"),
            MessageSegment::user("What is 2+2?"),
            MessageSegment::tool("calc", "4"),
            MessageSegment::user("Thanks!"),
        ]);

        let user_segs = msg.user_segments();
        assert_eq!(user_segs.len(), 2);
        assert_eq!(user_segs[0].0, 1); // index 1
        assert_eq!(user_segs[1].0, 3); // index 3
    }

    #[test]
    fn test_user_content_concatenation() {
        let msg = StructuredMessage::new(vec![
            MessageSegment::system("sys"),
            MessageSegment::user("hello"),
            MessageSegment::user("world"),
        ]);
        assert_eq!(msg.user_content(), "hello\nworld");
    }

    #[test]
    fn test_from_user_shorthand() {
        let msg = StructuredMessage::from_user("quick question");
        assert_eq!(msg.segments.len(), 1);
        assert!(msg.segments[0].is_user());
    }

    #[test]
    fn test_canary_token() {
        let msg = StructuredMessage::from_user("hi").with_canary("CANARY-abc123".to_string());
        assert_eq!(msg.canary_token.as_deref(), Some("CANARY-abc123"));
    }

    #[test]
    fn test_taint_labels_collected() {
        let mut taint = HashSet::new();
        taint.insert("email".to_string());
        let msg = StructuredMessage::new(vec![
            MessageSegment::system("sys"),
            MessageSegment::user_with_taint("my email is here", taint.clone()),
            MessageSegment::user("no taint here"),
        ]);
        let labels = msg.taint_labels();
        assert_eq!(labels.len(), 1);
        assert!(labels.contains("email"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let msg = StructuredMessage::new(vec![
            MessageSegment::system_immutable("rules"),
            MessageSegment::user("question"),
            MessageSegment::assistant("answer"),
        ]);
        let json = serde_json::to_string(&msg).unwrap();
        let parsed: StructuredMessage = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed.segments.len(), 3);
    }
}
