use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::{LlmClient, Message};

use super::is_compact_message;
use crate::timeline::{TimelineCompactionWindow, TimelineJsonlStore};

const BEGINNING_MESSAGES_FOR_COMPACTION: usize = 20;
const RECENT_MESSAGES_FOR_COMPACTION: usize = 80;
pub(crate) const MANUAL_COMPACT_TIMEOUT: Duration = Duration::from_secs(60);

const COMPACT_PROMPT: &str = r#"Produce exactly one updated compact summary for this A3S terminal session.

Use this markdown structure:

## Context Summary

### Background
Project/workspace context and the user's high-level goal.

### User Requirements
Explicit user requirements, preferences, prohibitions, and acceptance criteria.

### Decisions
Architectural or implementation decisions already made and why.

### Completed Work
Files/modules changed, important behavior changes, and validations already run.

### Current State
Current phase, remaining dirty state, known risks, failures, and blockers.

### Next Steps
The immediate next implementation or verification steps.

Rules:
- Preserve user corrections and review feedback.
- Preserve concrete file paths, commands, and test results when relevant.
- Ignore greetings, filler, and repeated low-value narration.
- Do not include the full old transcript; write a compact continuation summary.
"#;

pub(crate) async fn compact_timeline(
    llm_client: Arc<dyn LlmClient>,
    timeline_store: &TimelineJsonlStore,
) -> Result<Option<String>, String> {
    let window = timeline_store
        .load_compaction_window(
            BEGINNING_MESSAGES_FOR_COMPACTION,
            RECENT_MESSAGES_FOR_COMPACTION,
        )
        .map_err(|error| format!("could not read timeline: {error}"))?;
    compact_history_with_timeout(
        llm_client,
        compaction_history_for_window(window),
        MANUAL_COMPACT_TIMEOUT,
    )
    .await
}

/// Compact an in-memory session history through the same direct, tool-free LLM
/// path used by the persisted Code Web timeline.
///
/// The TUI creates a fresh session after a manual compact, so a previous manual
/// summary lives in its system prompt instead of the new session history. Feed
/// that summary back as the oldest context when compacting again.
pub(crate) async fn compact_history(
    llm_client: Arc<dyn LlmClient>,
    history: &[Message],
    previous_summary: Option<&str>,
) -> Result<Option<String>, String> {
    let mut selected = Vec::new();
    if let Some(summary) = previous_summary.filter(|summary| !summary.trim().is_empty()) {
        let previous_context = format!("Earlier compacted context:\n\n{}", summary.trim());
        selected.push(Message::user(&previous_context));
    }
    selected.extend(compaction_history_for_messages(history));
    compact_history_with_timeout(llm_client, selected, MANUAL_COMPACT_TIMEOUT).await
}

#[cfg(test)]
async fn compact_timeline_with_timeout(
    llm_client: Arc<dyn LlmClient>,
    timeline: Vec<Message>,
    timeout: Duration,
) -> Result<Option<String>, String> {
    compact_history_with_timeout(
        llm_client,
        compaction_history_for_messages(&timeline),
        timeout,
    )
    .await
}

async fn compact_history_with_timeout(
    llm_client: Arc<dyn LlmClient>,
    mut request_messages: Vec<Message>,
    timeout: Duration,
) -> Result<Option<String>, String> {
    if request_messages.is_empty() {
        return Ok(None);
    }
    request_messages.push(Message::user(COMPACT_PROMPT));

    let response = tokio::time::timeout(timeout, llm_client.complete(&request_messages, None, &[]))
        .await
        .map_err(|_| format!("compaction timed out after {} seconds", timeout.as_secs()))?
        .map_err(|error| error.to_string())?;
    let summary = response.text().trim().to_string();
    if summary.is_empty() {
        return Err("compaction failed with an empty summary".to_string());
    }
    Ok(Some(summary))
}

fn compaction_history_for_window(window: TimelineCompactionWindow) -> Vec<Message> {
    let mut selected = window.beginning;
    if let Some(summary) = window.latest_summary {
        selected.push(compact_summary_as_user(&summary));
    }
    selected.extend(window.recent);
    selected
}

fn compaction_history_for_messages(timeline: &[Message]) -> Vec<Message> {
    let latest_summary_index = timeline.iter().rposition(is_compact_message);
    let mut selected = Vec::new();

    for (index, message) in timeline.iter().enumerate() {
        if selected.len() >= BEGINNING_MESSAGES_FOR_COMPACTION {
            break;
        }
        if latest_summary_index.is_some_and(|summary_index| index >= summary_index) {
            break;
        }
        if !is_compact_message(message) {
            selected.push(message.clone());
        }
    }

    if let Some(summary_index) = latest_summary_index {
        selected.push(compact_summary_as_user(&timeline[summary_index]));
        let recent = timeline[summary_index + 1..]
            .iter()
            .filter(|message| !is_compact_message(message))
            .cloned()
            .collect::<Vec<_>>();
        selected.extend(tail_messages(&recent, RECENT_MESSAGES_FOR_COMPACTION));
    } else {
        let recent = timeline
            .iter()
            .skip(selected.len())
            .filter(|message| !is_compact_message(message))
            .cloned()
            .collect::<Vec<_>>();
        selected.extend(tail_messages(&recent, RECENT_MESSAGES_FOR_COMPACTION));
    }

    selected
}

fn compact_summary_as_user(message: &Message) -> Message {
    Message {
        role: "user".to_string(),
        content: message.content.clone(),
        reasoning_content: None,
    }
}

fn tail_messages(messages: &[Message], limit: usize) -> Vec<Message> {
    let start = messages.len().saturating_sub(limit);
    messages[start..].to_vec()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compact::A3S_COMPACT_ROLE;
    use a3s_code_core::llm::{
        ContentBlock, LlmClient, LlmResponse, StreamEvent, TokenUsage, ToolDefinition,
    };
    use async_trait::async_trait;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Mutex;
    use tokio::sync::mpsc;
    use tokio_util::sync::CancellationToken;

    struct RecordingLlmClient {
        calls: AtomicUsize,
        messages: Mutex<Vec<Message>>,
        tool_counts: Mutex<Vec<usize>>,
    }

    impl RecordingLlmClient {
        fn new() -> Self {
            Self {
                calls: AtomicUsize::new(0),
                messages: Mutex::new(Vec::new()),
                tool_counts: Mutex::new(Vec::new()),
            }
        }
    }

    struct HangingLlmClient;

    #[async_trait]
    impl LlmClient for RecordingLlmClient {
        async fn complete(
            &self,
            messages: &[Message],
            _system: Option<&str>,
            tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            *self.messages.lock().unwrap() = messages.to_vec();
            self.tool_counts.lock().unwrap().push(tools.len());
            Ok(text_response("direct compact summary"))
        }

        async fn complete_streaming(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
            _cancel_token: CancellationToken,
        ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
            unreachable!("manual compact uses one blocking completion")
        }
    }

    #[async_trait]
    impl LlmClient for HangingLlmClient {
        async fn complete(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
        ) -> anyhow::Result<LlmResponse> {
            std::future::pending().await
        }

        async fn complete_streaming(
            &self,
            _messages: &[Message],
            _system: Option<&str>,
            _tools: &[ToolDefinition],
            _cancel_token: CancellationToken,
        ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
            unreachable!("manual compact uses one blocking completion")
        }
    }

    fn text_response(text: &str) -> LlmResponse {
        LlmResponse {
            message: Message::assistant(text),
            usage: TokenUsage {
                prompt_tokens: 1,
                completion_tokens: 1,
                total_tokens: 2,
                cache_read_tokens: None,
                cache_write_tokens: None,
            },
            stop_reason: Some("end_turn".to_string()),
            token_logprobs: Vec::new(),
            meta: None,
        }
    }

    fn msg(role: &str, text: &str) -> Message {
        Message {
            role: role.to_string(),
            content: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
            reasoning_content: None,
        }
    }

    #[test]
    fn compaction_history_uses_beginning_latest_summary_and_recent_raw() {
        let timeline = vec![
            msg("user", "original goal"),
            msg(A3S_COMPACT_ROLE, "old summary"),
            msg("assistant", "middle raw"),
            msg(A3S_COMPACT_ROLE, "latest summary"),
            msg("user", "recent ask"),
            msg("assistant", "recent answer"),
        ];

        let history = compaction_history_for_messages(&timeline);
        let text = history
            .iter()
            .map(Message::text)
            .collect::<Vec<_>>()
            .join("\n");

        assert!(text.contains("original goal"));
        assert!(text.contains("latest summary"));
        assert!(text.contains("recent ask"));
        assert!(text.contains("recent answer"));
        assert!(!text.contains("old summary"));
        assert!(history
            .iter()
            .all(|message| message.role != A3S_COMPACT_ROLE));
    }

    #[test]
    fn compact_prompt_uses_fixed_summary_structure() {
        assert!(COMPACT_PROMPT.contains("## Context Summary"));
        assert!(COMPACT_PROMPT.contains("### User Requirements"));
        assert!(COMPACT_PROMPT.contains("### Current State"));
        assert!(COMPACT_PROMPT.contains("### Next Steps"));
    }

    #[tokio::test]
    async fn compact_uses_one_direct_llm_call_without_tools() {
        let client = Arc::new(RecordingLlmClient::new());

        let summary = compact_timeline_with_timeout(
            client.clone(),
            vec![
                msg("user", "original goal"),
                msg("assistant", "current state"),
            ],
            std::time::Duration::from_secs(1),
        )
        .await
        .expect("compact result")
        .expect("summary");

        assert_eq!(summary, "direct compact summary");
        assert_eq!(client.calls.load(Ordering::SeqCst), 1);
        assert_eq!(client.tool_counts.lock().unwrap().as_slice(), &[0]);
        let messages = client.messages.lock().unwrap();
        assert!(messages.last().unwrap().text().contains("Context Summary"));
        assert!(messages
            .iter()
            .all(|message| message.role != A3S_COMPACT_ROLE));
    }

    #[tokio::test]
    async fn repeated_manual_compact_includes_the_previous_summary() {
        let client = Arc::new(RecordingLlmClient::new());

        let summary = compact_history(
            client.clone(),
            &[msg("user", "new request"), msg("assistant", "new result")],
            Some("earlier goal and completed work"),
        )
        .await
        .expect("compact result")
        .expect("summary");

        assert_eq!(summary, "direct compact summary");
        let text = client
            .messages
            .lock()
            .unwrap()
            .iter()
            .map(Message::text)
            .collect::<Vec<_>>()
            .join("\n");
        assert!(text.contains("earlier goal and completed work"));
        assert!(text.contains("new request"));
        assert!(text.contains("new result"));
    }

    #[tokio::test]
    async fn compact_times_out_when_direct_llm_call_hangs() {
        let error = compact_timeline_with_timeout(
            Arc::new(HangingLlmClient),
            vec![msg("user", "original goal")],
            std::time::Duration::from_millis(10),
        )
        .await
        .expect_err("hanging compact should time out");

        assert!(error.contains("timed out"), "{error}");
    }
}
