use std::fs;
use std::path::PathBuf;

use a3s_code_core::llm::Message;
use serde::{Deserialize, Serialize};

use crate::compact::{
    is_compact_message, project_messages_for_llm, project_messages_for_llm_with_budget,
    ProjectionBudget,
};
use crate::timeline::TimelineMetadata;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ModelContextState {
    pub(crate) context_version: u64,
    pub(crate) compact_generation: u32,
    pub(crate) active_summary_index: Option<usize>,
    pub(crate) active_segment_start_index: usize,
    #[serde(default)]
    pub(crate) source_file_bytes: u64,
    #[serde(default)]
    pub(crate) source_event_count: usize,
    pub(crate) source_message_count: usize,
    #[serde(default)]
    pub(crate) last_prompt_tokens: usize,
    #[serde(default)]
    pub(crate) context_limit: u32,
    #[serde(default)]
    pub(crate) auto_compact_threshold: f64,
    pub(crate) messages: Vec<Message>,
    pub(crate) updated_at_ms: i64,
}

impl ModelContextState {
    #[allow(dead_code)]
    pub(crate) fn rebuild_from_timeline(timeline: &[Message]) -> Self {
        Self::rebuild_from_timeline_with_messages(
            timeline,
            project_messages_for_llm(timeline),
            TimelineMetadata {
                source_file_bytes: 0,
                source_event_count: timeline.len(),
                source_message_count: timeline.len(),
                active_summary_index: timeline.iter().rposition(is_compact_message),
                compact_generation: timeline
                    .iter()
                    .filter(|message| is_compact_message(message))
                    .count() as u32,
            },
            0,
            0,
            0.0,
        )
    }

    pub(crate) fn rebuild_from_timeline_with_metadata(
        timeline: &[Message],
        budget: ProjectionBudget,
        metadata: TimelineMetadata,
        last_prompt_tokens: usize,
        context_limit: u32,
        auto_compact_threshold: f64,
    ) -> Self {
        Self::rebuild_from_timeline_with_messages(
            timeline,
            project_messages_for_llm_with_budget(timeline, budget),
            metadata,
            last_prompt_tokens,
            context_limit,
            auto_compact_threshold,
        )
    }

    fn rebuild_from_timeline_with_messages(
        _timeline: &[Message],
        messages: Vec<Message>,
        metadata: TimelineMetadata,
        last_prompt_tokens: usize,
        context_limit: u32,
        auto_compact_threshold: f64,
    ) -> Self {
        let active_segment_start_index = metadata
            .active_summary_index
            .map(|index| index.saturating_add(1))
            .unwrap_or(0);
        Self {
            context_version: 2,
            compact_generation: metadata.compact_generation,
            active_summary_index: metadata.active_summary_index,
            active_segment_start_index,
            source_file_bytes: metadata.source_file_bytes,
            source_event_count: metadata.source_event_count,
            source_message_count: metadata.source_message_count,
            last_prompt_tokens,
            context_limit,
            auto_compact_threshold,
            messages,
            updated_at_ms: chrono::Utc::now().timestamp_millis(),
        }
    }

    pub(crate) fn matches_timeline(&self, metadata: TimelineMetadata) -> bool {
        self.context_version == 2
            && self.source_file_bytes == metadata.source_file_bytes
            && self.source_event_count == metadata.source_event_count
            && self.source_message_count == metadata.source_message_count
            && self.active_summary_index == metadata.active_summary_index
            && self.compact_generation == metadata.compact_generation
    }

    pub(crate) fn update_runtime_metadata(
        &mut self,
        last_prompt_tokens: usize,
        context_limit: u32,
        auto_compact_threshold: f64,
    ) {
        self.last_prompt_tokens = last_prompt_tokens;
        self.context_limit = context_limit;
        self.auto_compact_threshold = auto_compact_threshold;
        self.updated_at_ms = chrono::Utc::now().timestamp_millis();
    }

    pub(crate) fn append_timeline_message(
        &mut self,
        message: &Message,
        appended_event_count: usize,
        source_file_bytes: u64,
        budget: ProjectionBudget,
    ) {
        let message_index = self.source_message_count;
        self.source_file_bytes = source_file_bytes;
        self.source_event_count = self.source_event_count.saturating_add(appended_event_count);
        self.source_message_count = self.source_message_count.saturating_add(1);

        if is_compact_message(message) {
            self.compact_generation = self.compact_generation.saturating_add(1);
            self.active_summary_index = Some(message_index);
            self.active_segment_start_index = message_index.saturating_add(1);
            self.messages = project_messages_for_llm(std::slice::from_ref(message));
        } else {
            let mut timeline = Vec::with_capacity(self.messages.len().saturating_add(1));
            if self.active_summary_index.is_some() && !self.messages.is_empty() {
                let mut summary = self.messages[0].clone();
                summary.role = crate::compact::A3S_COMPACT_ROLE.to_string();
                timeline.push(summary);
                timeline.extend(self.messages[1..].iter().cloned());
            } else {
                timeline.extend(self.messages.iter().cloned());
            }
            timeline.push(message.clone());
            self.messages = project_messages_for_llm_with_budget(&timeline, budget);
        }
        self.updated_at_ms = chrono::Utc::now().timestamp_millis();
    }
}

pub(crate) struct ContextJsonStore {
    path: PathBuf,
}

impl ContextJsonStore {
    pub(crate) fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub(crate) fn for_session(store_dir: impl Into<PathBuf>, session_id: &str) -> Self {
        Self::new(
            store_dir
                .into()
                .join("contexts")
                .join(format!("{}.json", safe_session_id(session_id))),
        )
    }

    pub(crate) fn save(&self, state: &ModelContextState) -> anyhow::Result<()> {
        if let Some(parent) = self
            .path
            .parent()
            .filter(|path| !path.as_os_str().is_empty())
        {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(state)?;
        fs::write(&self.path, json)?;
        Ok(())
    }

    pub(crate) fn load(&self) -> anyhow::Result<Option<ModelContextState>> {
        if !self.path.exists() {
            return Ok(None);
        }
        let json = fs::read_to_string(&self.path)?;
        Ok(Some(serde_json::from_str(&json)?))
    }

    pub(crate) fn clear(&self) -> anyhow::Result<()> {
        match fs::remove_file(&self.path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(error) => Err(error.into()),
        }
    }
}

fn safe_session_id(id: &str) -> String {
    id.replace(['/', '\\'], "_").replace("..", "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::llm::{ContentBlock, Message};

    use crate::compact::A3S_COMPACT_ROLE;

    fn msg(role: &str, text: &str) -> Message {
        Message {
            role: role.to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning_content: None,
        }
    }

    fn temp_context_path(name: &str) -> std::path::PathBuf {
        let nanos = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        std::env::temp_dir().join(format!(
            "a3s-context-{name}-{}-{nanos}.json",
            std::process::id()
        ))
    }

    #[test]
    fn rebuild_projects_latest_summary_and_recent_messages() {
        let timeline = vec![
            msg("user", "old user"),
            msg(A3S_COMPACT_ROLE, "summary one"),
            msg("assistant", "old assistant"),
            msg(A3S_COMPACT_ROLE, "summary two"),
            msg("user", "recent user"),
        ];

        let state = ModelContextState::rebuild_from_timeline(&timeline);

        assert_eq!(state.compact_generation, 2);
        assert_eq!(state.active_summary_index, Some(3));
        assert_eq!(state.active_segment_start_index, 4);
        assert_eq!(state.messages.len(), 2);
        assert_eq!(state.messages[0].role, "user");
        assert_eq!(state.messages[0].text(), "summary two");
        assert_eq!(state.messages[1].text(), "recent user");
        assert!(state
            .messages
            .iter()
            .all(|message| message.role != A3S_COMPACT_ROLE));
    }

    #[test]
    fn context_json_store_round_trips_state() {
        let timeline = vec![msg(A3S_COMPACT_ROLE, "summary"), msg("user", "recent")];
        let state = ModelContextState::rebuild_from_timeline(&timeline);
        let store = ContextJsonStore::new(temp_context_path("roundtrip"));

        store.save(&state).expect("save context");
        let loaded = store.load().expect("load context").expect("context");

        assert_eq!(loaded.context_version, state.context_version);
        assert_eq!(loaded.compact_generation, 1);
        assert_eq!(loaded.messages.len(), 2);
        assert_eq!(loaded.messages[0].text(), "summary");
        assert_eq!(loaded.messages[1].text(), "recent");
    }

    #[test]
    fn context_cache_matches_only_the_timeline_it_was_built_from() {
        let timeline = vec![msg(A3S_COMPACT_ROLE, "summary"), msg("user", "recent")];
        let metadata = crate::timeline::TimelineMetadata {
            source_file_bytes: 42,
            source_event_count: 3,
            source_message_count: 2,
            active_summary_index: Some(0),
            compact_generation: 1,
        };
        let state = ModelContextState::rebuild_from_timeline_with_metadata(
            &timeline,
            ProjectionBudget::for_token_limit(200_000),
            metadata,
            123_000,
            200_000,
            0.85,
        );

        assert!(state.matches_timeline(metadata));
        assert!(!state.matches_timeline(crate::timeline::TimelineMetadata {
            source_event_count: 4,
            ..metadata
        }));
        assert!(!state.matches_timeline(crate::timeline::TimelineMetadata {
            active_summary_index: None,
            ..metadata
        }));
        assert_eq!(state.last_prompt_tokens, 123_000);
        assert_eq!(state.context_limit, 200_000);
        assert_eq!(state.auto_compact_threshold, 0.85);
    }
}
