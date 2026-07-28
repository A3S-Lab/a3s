use a3s_code_core::llm::{ContentBlock, Message, ToolResultContent, ToolResultContentField};

pub(crate) const A3S_COMPACT_ROLE: &str = "a3s_compact";

pub(crate) fn is_compact_message(message: &Message) -> bool {
    message.role == A3S_COMPACT_ROLE
}

fn compact_summary_as_user(message: &Message) -> Message {
    Message {
        role: "user".to_string(),
        content: message.content.clone(),
        reasoning_content: None,
    }
}

pub(crate) fn project_messages_for_llm(messages: &[Message]) -> Vec<Message> {
    let Some(summary_index) = messages.iter().rposition(is_compact_message) else {
        return messages.to_vec();
    };

    let mut projected = Vec::with_capacity(messages.len() - summary_index);
    projected.push(compact_summary_as_user(&messages[summary_index]));
    projected.extend(
        messages[summary_index + 1..]
            .iter()
            .filter(|message| !is_compact_message(message))
            .cloned(),
    );
    projected
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProjectionBudget {
    pub(crate) max_messages: usize,
    pub(crate) max_chars: usize,
    pub(crate) max_block_chars: usize,
}

impl ProjectionBudget {
    pub(crate) fn for_token_limit(max_tokens: usize) -> Self {
        Self {
            max_messages: 200,
            max_chars: max_tokens.saturating_mul(4).max(8_000),
            max_block_chars: 20_000,
        }
    }
}

pub(crate) fn project_messages_for_llm_with_budget(
    messages: &[Message],
    budget: ProjectionBudget,
) -> Vec<Message> {
    let has_compact_summary = messages.iter().any(is_compact_message);
    let mut projected: Vec<Message> = project_messages_for_llm(messages)
        .into_iter()
        .map(|message| prune_message_blocks(message, budget.max_block_chars))
        .collect();

    projected = retain_recent_messages(projected, budget.max_messages, has_compact_summary);
    while projected_text_len(&projected) > budget.max_chars && projected.len() > 1 {
        let remove_index = if has_compact_summary { 1 } else { 0 };
        projected.remove(remove_index);
    }

    projected
}

pub(crate) fn append_compact_summary(messages: &mut Vec<Message>, summary: &str) {
    messages.push(Message {
        role: A3S_COMPACT_ROLE.to_string(),
        content: vec![ContentBlock::Text {
            text: summary.to_string(),
        }],
        reasoning_content: None,
    });
}

fn retain_recent_messages(
    messages: Vec<Message>,
    max_messages: usize,
    has_compact_summary: bool,
) -> Vec<Message> {
    if max_messages == 0 || messages.len() <= max_messages {
        return messages;
    }
    if has_compact_summary {
        let recent_count = max_messages.saturating_sub(1);
        let mut retained = Vec::with_capacity(max_messages);
        if let Some(summary) = messages.first() {
            retained.push(summary.clone());
        }
        let start = messages.len().saturating_sub(recent_count);
        retained.extend(messages[start..].iter().cloned());
        retained
    } else {
        messages[messages.len().saturating_sub(max_messages)..].to_vec()
    }
}

fn prune_message_blocks(mut message: Message, max_block_chars: usize) -> Message {
    message.content = message
        .content
        .into_iter()
        .map(|block| prune_content_block(block, max_block_chars))
        .collect();
    message
}

fn prune_content_block(block: ContentBlock, max_block_chars: usize) -> ContentBlock {
    match block {
        ContentBlock::Text { text } => ContentBlock::Text {
            text: truncate_chars(&text, max_block_chars),
        },
        ContentBlock::ToolResult {
            tool_use_id,
            content,
            is_error,
        } => ContentBlock::ToolResult {
            tool_use_id,
            content: prune_tool_result_content(content, max_block_chars),
            is_error,
        },
        other => other,
    }
}

fn prune_tool_result_content(
    content: ToolResultContentField,
    max_block_chars: usize,
) -> ToolResultContentField {
    match content {
        ToolResultContentField::Text(text) => {
            ToolResultContentField::Text(truncate_chars(&text, max_block_chars))
        }
        ToolResultContentField::Blocks(blocks) => ToolResultContentField::Blocks(
            blocks
                .into_iter()
                .map(|block| match block {
                    ToolResultContent::Text { text } => ToolResultContent::Text {
                        text: truncate_chars(&text, max_block_chars),
                    },
                    other => other,
                })
                .collect(),
        ),
    }
}

fn truncate_chars(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let keep = max_chars.saturating_sub(24);
    let mut truncated = text.chars().take(keep).collect::<String>();
    truncated.push_str("\n[truncated for model context]");
    truncated
}

fn projected_text_len(messages: &[Message]) -> usize {
    messages
        .iter()
        .flat_map(|message| message.content.iter())
        .map(content_block_text_len)
        .sum()
}

fn content_block_text_len(block: &ContentBlock) -> usize {
    match block {
        ContentBlock::Text { text } => text.chars().count(),
        ContentBlock::ToolResult { content, .. } => content.as_text().chars().count(),
        ContentBlock::ToolUse { input, .. } => input.to_string().chars().count(),
        ContentBlock::Image { .. } => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn projection_without_compact_keeps_original_messages() {
        let messages = vec![msg("user", "hello"), msg("assistant", "hi")];

        let projected = project_messages_for_llm(&messages);

        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].role, "user");
        assert_eq!(projected[0].text(), "hello");
        assert_eq!(projected[1].role, "assistant");
        assert_eq!(projected[1].text(), "hi");
    }

    #[test]
    fn projection_uses_latest_compact_summary_and_recent_raw() {
        let messages = vec![
            msg("user", "old user"),
            msg("assistant", "old assistant"),
            msg(A3S_COMPACT_ROLE, "summary"),
            msg("user", "new user"),
        ];

        let projected = project_messages_for_llm(&messages);

        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].role, "user");
        assert_eq!(projected[0].text(), "summary");
        assert_eq!(projected[0].reasoning_content, None);
        assert_eq!(projected[1].role, "user");
        assert_eq!(projected[1].text(), "new user");
    }

    #[test]
    fn projection_ignores_older_compact_summaries() {
        let messages = vec![
            msg(A3S_COMPACT_ROLE, "summary one"),
            msg("user", "old raw"),
            msg(A3S_COMPACT_ROLE, "summary two"),
            msg("assistant", "recent raw"),
        ];

        let projected = project_messages_for_llm(&messages);

        assert_eq!(projected.len(), 2);
        assert_eq!(projected[0].text(), "summary two");
        assert_eq!(projected[1].text(), "recent raw");
        assert!(projected.iter().all(|message| !is_compact_message(message)));
    }

    #[test]
    fn append_compact_summary_preserves_existing_messages() {
        let mut messages = vec![msg("user", "old user"), msg("assistant", "old assistant")];

        append_compact_summary(&mut messages, "summary");

        assert_eq!(messages.len(), 3);
        assert_eq!(messages[0].text(), "old user");
        assert_eq!(messages[1].text(), "old assistant");
        assert_eq!(messages[2].role, A3S_COMPACT_ROLE);
        assert_eq!(messages[2].text(), "summary");
        assert_eq!(messages[2].reasoning_content, None);
    }

    #[test]
    fn budget_projection_keeps_latest_summary_and_recent_messages() {
        let messages = vec![
            msg("user", "old user"),
            msg(A3S_COMPACT_ROLE, "summary"),
            msg("user", "recent one"),
            msg("assistant", "recent two"),
            msg("user", "recent three"),
        ];

        let projected = project_messages_for_llm_with_budget(
            &messages,
            ProjectionBudget {
                max_messages: 3,
                max_chars: 1_000,
                max_block_chars: 1_000,
            },
        );

        assert_eq!(projected.len(), 3);
        assert_eq!(projected[0].text(), "summary");
        assert_eq!(projected[1].text(), "recent two");
        assert_eq!(projected[2].text(), "recent three");
        assert!(projected.iter().all(|message| !is_compact_message(message)));
    }

    #[test]
    fn budget_projection_prunes_large_tool_result_blocks() {
        let messages = vec![Message::tool_result("toolu", &"x".repeat(80), false)];

        let projected = project_messages_for_llm_with_budget(
            &messages,
            ProjectionBudget {
                max_messages: 4,
                max_chars: 1_000,
                max_block_chars: 40,
            },
        );

        let ContentBlock::ToolResult { content, .. } = &projected[0].content[0] else {
            panic!("expected tool result");
        };
        let text = content.as_text();
        assert!(text.contains("[truncated for model context]"));
        assert!(text.chars().count() < 80);
    }
}
