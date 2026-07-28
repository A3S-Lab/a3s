use std::collections::VecDeque;
use std::fs::{self, OpenOptions};
use std::io::{BufRead, BufReader, Write};
#[cfg(test)]
use std::io::{Read, Seek, SeekFrom};
use std::path::PathBuf;

use a3s_code_core::llm::{ContentBlock, Message};
use serde::{Deserialize, Serialize};

use crate::compact::{is_compact_message, A3S_COMPACT_ROLE};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct TranscriptEvent {
    pub(crate) event_kind: TranscriptEventKind,
    pub(crate) message: Option<Message>,
    pub(crate) display: DisplayMetadata,
}

#[allow(dead_code)]
#[derive(Debug, Clone)]
pub(crate) struct TimelinePage {
    pub(crate) events: Vec<TranscriptEvent>,
    pub(crate) has_more_before: bool,
    pub(crate) next_before_seq: Option<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct TimelineMetadata {
    pub(crate) source_file_bytes: u64,
    pub(crate) source_event_count: usize,
    pub(crate) source_message_count: usize,
    pub(crate) active_summary_index: Option<usize>,
    pub(crate) compact_generation: u32,
}

#[derive(Debug, Clone)]
pub(crate) struct TimelineCompactionWindow {
    pub(crate) beginning: Vec<Message>,
    pub(crate) latest_summary: Option<Message>,
    pub(crate) recent: Vec<Message>,
    pub(crate) metadata: TimelineMetadata,
}

impl TimelineCompactionWindow {
    #[cfg(test)]
    pub(crate) fn into_messages(self) -> Vec<Message> {
        let mut messages = self.beginning;
        if let Some(summary) = self.latest_summary {
            messages.push(summary);
        }
        messages.extend(self.recent);
        messages
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum TranscriptEventKind {
    UserMessage,
    AssistantMessage,
    ToolResult,
    ContextSummary,
    CompactMarker,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct DisplayMetadata {
    pub(crate) visible: bool,
}

#[derive(Clone)]
pub(crate) struct TimelineJsonlStore {
    path: PathBuf,
}

impl TimelineJsonlStore {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn for_session(store_dir: impl Into<PathBuf>, session_id: &str) -> Self {
        Self::new(
            store_dir
                .into()
                .join("timelines")
                .join(format!("{}.jsonl", safe_session_id(session_id))),
        )
    }

    pub(crate) fn append(&self, event: &TranscriptEvent) -> anyhow::Result<()> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        serde_json::to_writer(&mut file, event)?;
        file.write_all(b"\n")?;
        Ok(())
    }

    #[allow(dead_code)]
    pub(crate) fn load_all(&self) -> anyhow::Result<Vec<TranscriptEvent>> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let file = fs::File::open(&self.path)?;
        let reader = BufReader::new(file);
        let mut events = Vec::new();
        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            events.push(serde_json::from_str(&line)?);
        }
        Ok(events)
    }

    #[cfg(test)]
    pub(crate) fn load_tail_page(&self, limit: usize) -> anyhow::Result<TimelinePage> {
        if limit == 0 || !self.path.exists() {
            return Ok(TimelinePage {
                events: Vec::new(),
                has_more_before: false,
                next_before_seq: None,
            });
        }

        const READ_CHUNK_BYTES: usize = 16 * 1024;
        let mut file = fs::File::open(&self.path)?;
        let mut position = file.metadata()?.len();
        let mut incomplete_line = Vec::new();
        let mut visible_reversed = Vec::with_capacity(limit.saturating_add(1));

        while position > 0 && visible_reversed.len() <= limit {
            let chunk_len = position.min(READ_CHUNK_BYTES as u64) as usize;
            position -= chunk_len as u64;
            file.seek(SeekFrom::Start(position))?;
            let mut chunk = vec![0; chunk_len];
            file.read_exact(&mut chunk)?;
            chunk.extend_from_slice(&incomplete_line);

            let mut line_end = chunk.len();
            while let Some(newline) = chunk[..line_end].iter().rposition(|byte| *byte == b'\n') {
                retain_visible_event(&chunk[newline + 1..line_end], &mut visible_reversed)?;
                line_end = newline;
                if visible_reversed.len() > limit {
                    break;
                }
            }
            incomplete_line = chunk[..line_end].to_vec();
        }
        if position == 0 && visible_reversed.len() <= limit {
            retain_visible_event(&incomplete_line, &mut visible_reversed)?;
        }

        let has_more_before = visible_reversed.len() > limit;
        visible_reversed.truncate(limit);
        visible_reversed.reverse();
        Ok(TimelinePage {
            events: visible_reversed,
            has_more_before,
            next_before_seq: None,
        })
    }

    pub(crate) fn load_compaction_window(
        &self,
        beginning_limit: usize,
        recent_limit: usize,
    ) -> anyhow::Result<TimelineCompactionWindow> {
        if !self.path.exists() {
            return Ok(TimelineCompactionWindow {
                beginning: Vec::new(),
                latest_summary: None,
                recent: Vec::new(),
                metadata: TimelineMetadata::default(),
            });
        }

        let file = fs::File::open(&self.path)?;
        let source_file_bytes = file.metadata()?.len();
        let reader = BufReader::new(file);
        let mut beginning = Vec::with_capacity(beginning_limit);
        let mut latest_summary = None;
        let mut recent = VecDeque::with_capacity(recent_limit);
        let mut metadata = TimelineMetadata {
            source_file_bytes,
            ..TimelineMetadata::default()
        };

        for line in reader.lines() {
            let line = line?;
            if line.trim().is_empty() {
                continue;
            }
            let event: TranscriptEvent = serde_json::from_str(&line)?;
            metadata.source_event_count += 1;
            let Some(message) = event.message else {
                continue;
            };
            let message_index = metadata.source_message_count;
            metadata.source_message_count += 1;
            if is_compact_message(&message) {
                metadata.active_summary_index = Some(message_index);
                metadata.compact_generation = metadata.compact_generation.saturating_add(1);
                latest_summary = Some(message);
                recent.clear();
                continue;
            }

            if beginning.len() < beginning_limit {
                beginning.push((message_index, message.clone()));
            }
            if recent_limit > 0 {
                recent.push_back((message_index, message));
                if recent.len() > recent_limit {
                    recent.pop_front();
                }
            }
        }

        let latest_beginning_index = beginning.last().map(|(index, _)| *index);
        let beginning = beginning.into_iter().map(|(_, message)| message).collect();
        let recent = recent
            .into_iter()
            .filter(|(index, _)| latest_beginning_index.is_none_or(|last| *index > last))
            .map(|(_, message)| message)
            .collect();
        Ok(TimelineCompactionWindow {
            beginning,
            latest_summary,
            recent,
            metadata,
        })
    }

    pub(crate) fn metadata(&self) -> anyhow::Result<TimelineMetadata> {
        Ok(self.load_compaction_window(0, 0)?.metadata)
    }

    pub(crate) fn file_len(&self) -> anyhow::Result<u64> {
        match fs::metadata(&self.path) {
            Ok(metadata) => Ok(metadata.len()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(0),
            Err(error) => Err(error.into()),
        }
    }

    pub(crate) fn clear(&self) -> anyhow::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
fn retain_visible_event(
    line: &[u8],
    visible_reversed: &mut Vec<TranscriptEvent>,
) -> anyhow::Result<()> {
    if line.iter().all(|byte| byte.is_ascii_whitespace()) {
        return Ok(());
    }
    let event: TranscriptEvent = serde_json::from_slice(line)?;
    if event.display.visible {
        visible_reversed.push(event);
    }
    Ok(())
}

fn safe_session_id(id: &str) -> String {
    id.replace(['/', '\\'], "_").replace("..", "_")
}

pub(crate) fn events_for_message(
    _session_id: &str,
    _message_index: usize,
    _first_seq: u64,
    message: &Message,
    _created_at_ms: i64,
) -> Vec<TranscriptEvent> {
    let event_kind = event_kind_for_message(message);
    let mut events = vec![TranscriptEvent {
        event_kind,
        message: Some(message.clone()),
        display: DisplayMetadata {
            visible: event_kind != TranscriptEventKind::ContextSummary,
        },
    }];

    if is_compact_message(message) {
        events.push(TranscriptEvent {
            event_kind: TranscriptEventKind::CompactMarker,
            message: None,
            display: DisplayMetadata { visible: true },
        });
    }

    events
}

pub(crate) fn messages_from_events(events: &[TranscriptEvent]) -> Vec<Message> {
    events
        .iter()
        .filter_map(|event| event.message.clone())
        .collect()
}

fn event_kind_for_message(message: &Message) -> TranscriptEventKind {
    if message.role == A3S_COMPACT_ROLE {
        return TranscriptEventKind::ContextSummary;
    }
    if message
        .content
        .iter()
        .any(|block| matches!(block, ContentBlock::ToolResult { .. }))
    {
        return TranscriptEventKind::ToolResult;
    }
    match message.role.as_str() {
        "assistant" => TranscriptEventKind::AssistantMessage,
        _ => TranscriptEventKind::UserMessage,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn compact_message(text: &str) -> Message {
        Message {
            role: A3S_COMPACT_ROLE.to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning_content: None,
        }
    }

    fn append_message(store: &TimelineJsonlStore, message: &Message) {
        for event in events_for_message("session", 0, 0, message, 0) {
            store.append(&event).expect("append timeline message");
        }
    }

    fn temp_timeline_path(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "a3s-timeline-{name}-{}-{nanos}.jsonl",
            std::process::id()
        ))
    }

    #[test]
    fn jsonl_store_appends_and_reads_events_in_order() {
        let path = temp_timeline_path("append-read");
        let store = TimelineJsonlStore::new(path);
        let user = events_for_message("session", 0, 0, &Message::user("hello"), 0)
            .into_iter()
            .next()
            .expect("user event");
        let compact = Message {
            role: A3S_COMPACT_ROLE.to_string(),
            content: vec![ContentBlock::Text {
                text: "summary".to_string(),
            }],
            reasoning_content: None,
        };
        let compact_events = events_for_message("session", 1, 1, &compact, 0);

        store.append(&user).expect("append user");
        for event in &compact_events {
            store.append(event).expect("append compact event");
        }

        let loaded = store.load_all().expect("load timeline");

        assert_eq!(loaded.len(), 3);
        assert_eq!(loaded[0].event_kind, TranscriptEventKind::UserMessage);
        assert_eq!(loaded[0].message.as_ref().unwrap().text(), "hello");
        assert_eq!(loaded[1].event_kind, TranscriptEventKind::ContextSummary);
        assert!(!loaded[1].display.visible);
        assert_eq!(loaded[2].event_kind, TranscriptEventKind::CompactMarker);
        assert!(loaded[2].display.visible);
    }

    #[test]
    fn jsonl_store_tail_page_skips_hidden_summary_events() {
        let path = temp_timeline_path("tail-page");
        let store = TimelineJsonlStore::new(path);
        for event in events_for_message("session", 0, 0, &Message::user("one"), 0) {
            store.append(&event).expect("append one");
        }
        let compact = Message {
            role: A3S_COMPACT_ROLE.to_string(),
            content: vec![ContentBlock::Text {
                text: "hidden summary".to_string(),
            }],
            reasoning_content: None,
        };
        for event in events_for_message("session", 1, 1, &compact, 0) {
            store.append(&event).expect("append compact");
        }
        for event in events_for_message("session", 2, 2, &Message::assistant("two"), 0) {
            store.append(&event).expect("append two");
        }

        let page = store.load_tail_page(2).expect("tail page");

        assert_eq!(page.events.len(), 2);
        assert_eq!(
            page.events[0].event_kind,
            TranscriptEventKind::CompactMarker
        );
        assert_eq!(page.events[1].message.as_ref().unwrap().text(), "two");
        assert!(page.events.iter().all(|event| event.display.visible));
        assert!(page.has_more_before);
    }

    #[test]
    fn messages_from_events_keeps_hidden_compact_summary_for_model_timeline() {
        let compact = Message {
            role: A3S_COMPACT_ROLE.to_string(),
            content: vec![ContentBlock::Text {
                text: "summary".to_string(),
            }],
            reasoning_content: None,
        };
        let events = events_for_message("session", 0, 0, &compact, 0);

        let messages = messages_from_events(&events);

        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].role, A3S_COMPACT_ROLE);
        assert_eq!(messages[0].text(), "summary");
    }

    #[test]
    fn compaction_window_scans_all_events_but_keeps_bounded_latest_generation() {
        let path = temp_timeline_path("compaction-window");
        let store = TimelineJsonlStore::new(path);
        for index in 0..25 {
            append_message(&store, &Message::user(&format!("beginning-{index}")));
        }
        append_message(&store, &compact_message("old summary"));
        append_message(&store, &Message::assistant("between summaries"));
        append_message(&store, &compact_message("latest summary"));
        for index in 0..85 {
            append_message(&store, &Message::user(&format!("recent-{index}")));
        }

        let window = store
            .load_compaction_window(20, 80)
            .expect("compaction window");

        assert_eq!(window.beginning.len(), 20);
        assert_eq!(window.beginning[0].text(), "beginning-0");
        assert_eq!(window.beginning[19].text(), "beginning-19");
        assert_eq!(
            window.latest_summary.as_ref().map(Message::text).as_deref(),
            Some("latest summary")
        );
        assert_eq!(window.recent.len(), 80);
        assert_eq!(window.recent[0].text(), "recent-5");
        assert_eq!(window.recent[79].text(), "recent-84");
        assert_eq!(window.metadata.source_message_count, 113);
        assert_eq!(window.metadata.source_event_count, 115);
        assert_eq!(window.metadata.active_summary_index, Some(27));
        assert_eq!(window.metadata.compact_generation, 2);
    }

    #[test]
    fn compaction_window_without_summary_does_not_duplicate_beginning_messages() {
        let path = temp_timeline_path("compaction-no-summary");
        let store = TimelineJsonlStore::new(path);
        for index in 0..30 {
            append_message(&store, &Message::user(&format!("message-{index}")));
        }

        let window = store
            .load_compaction_window(20, 80)
            .expect("compaction window");
        let messages = window.into_messages();

        assert_eq!(messages.len(), 30);
        assert_eq!(messages[0].text(), "message-0");
        assert_eq!(messages[29].text(), "message-29");
    }
}
