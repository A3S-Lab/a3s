use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::AgentSession;
use a3s_memory::{FileMemoryStore, MemoryItem, MemoryStore, MemoryType};
use serde::{Deserialize, Serialize};

use crate::config;

const SLEEP_FENCE: &str = "```a3s-sleep";

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct SleepMemory {
    #[serde(default)]
    pub(super) kind: String,
    #[serde(default)]
    pub(super) content: String,
}

#[derive(Deserialize)]
struct SleepReport {
    memories: Vec<SleepMemory>,
}

pub(super) fn sleep_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

pub(super) fn sleep_directive(focus: &str, ctx_ready: bool, today: &str) -> String {
    let ctx_clause = if ctx_ready {
        " Use the `ctx` history CLI (per your context-recall guide) to search today's \
         sessions across ALL projects - query several broad topics from today's work, \
         not just this directory's. If ctx returns nothing, move on -"
    } else {
        " Your only sources are listed next -"
    };
    let focus_clause = if focus.trim().is_empty() {
        String::new()
    } else {
        format!("\n\nFocus especially on: {}", focus.trim())
    };
    format!(
        "You are running SLEEP consolidation for {today} - the end-of-day pass that \
         turns today's work into durable long-term memory.\n\
         1. Reconstruct what was worked on and accomplished today.{ctx_clause} \
         review this session's own conversation. Do NOT crawl the filesystem for \
         other sessions' data - ctx and this conversation are the only sources.\n\
         2. Distill ONLY durable, cross-session takeaways, of three kinds:\n\
         - \"experience\": an approach that WORKED today (or a mistake to avoid) with \
         the why - written so a future session can apply it directly.\n\
         - \"preference\": how the user likes things done - style, tools, language, \
         workflow - observed from their requests and corrections today.\n\
         - \"knowledge\": a stable fact about a project or environment worth keeping \
         (locations, invariants, gotchas).\n\
         Rules: each item self-contained and specific; skip transient state and TODO \
         minutiae; NEVER include secrets, tokens, or credentials; do not repeat what \
         your context shows is already remembered.\n\
         3. End your FINAL message with exactly this fenced block (the host parses it \
         and writes each item into long-term memory):\n\
         {SLEEP_FENCE}\n\
         {{\"memories\": [{{\"kind\": <experience|preference|knowledge>, \"content\": \
         <one self-contained takeaway>}}]}}\n\
         ```\n\
         Valid JSON, at most 20 items, empty array if nothing durable surfaced \
         today.{focus_clause}"
    )
}

pub(super) fn parse_sleep_report(text: &str) -> Option<Vec<SleepMemory>> {
    let mut hay = text;
    while let Some(start) = line_anchored_rfind(hay, SLEEP_FENCE) {
        let body = &hay[start + SLEEP_FENCE.len()..];
        if let Some(end) = body.find("\n```") {
            if let Ok(report) = serde_json::from_str::<SleepReport>(body[..end].trim()) {
                return Some(report.memories);
            }
        }
        hay = &hay[..start];
    }
    None
}

pub(super) async fn store_sleep_memories(
    session: &AgentSession,
    memories: Vec<SleepMemory>,
    today: &str,
) -> BootResult<Vec<SleepMemory>> {
    let selected: Vec<SleepMemory> = memories
        .into_iter()
        .filter(|memory| !memory.content.trim().is_empty())
        .take(20)
        .collect();
    if selected.is_empty() {
        return Ok(Vec::new());
    }

    let items: Vec<MemoryItem> = selected
        .iter()
        .map(|memory| sleep_memory_item(memory, today))
        .collect();
    if let Some(memory) = session.memory().cloned() {
        for item in items {
            memory
                .remember(item)
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?;
        }
    } else {
        let store = FileMemoryStore::new(config::memory_dir())
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        for item in items {
            MemoryStore::store(&store, item)
                .await
                .map_err(|error| BootError::Internal(error.to_string()))?;
        }
    }
    Ok(selected)
}

fn sleep_memory_item(memory: &SleepMemory, today: &str) -> MemoryItem {
    let kind = memory.kind.trim().to_lowercase();
    let memory_type = match kind.as_str() {
        "experience" => MemoryType::Procedural,
        "preference" | "knowledge" => MemoryType::Semantic,
        _ => MemoryType::Episodic,
    };
    let tag = if kind.is_empty() {
        "note".to_string()
    } else {
        kind
    };
    MemoryItem::new(memory.content.trim().to_string())
        .with_type(memory_type)
        .with_importance(0.75)
        .with_tags(vec!["sleep".to_string(), tag])
        .with_metadata("source", "sleep")
        .with_metadata("sleep_date", today)
}

fn line_anchored_rfind(hay: &str, needle: &str) -> Option<usize> {
    let mut upto = hay.len();
    loop {
        let pos = hay[..upto].rfind(needle)?;
        if pos == 0 || hay.as_bytes()[pos - 1] == b'\n' {
            return Some(pos);
        }
        upto = pos;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(json: &str) -> String {
        format!("done\n{SLEEP_FENCE}\n{json}\n```\n")
    }

    #[test]
    fn parses_last_line_anchored_sleep_report() {
        let text = format!(
            "{}\n{}",
            block(r#"{"memories":[{"kind":"experience","content":"old"}]}"#),
            block(r#"{"memories":[{"kind":"preference","content":"Use focused UI controls."}]}"#)
        );
        let memories = parse_sleep_report(&text).expect("sleep report");
        assert_eq!(memories.len(), 1);
        assert_eq!(memories[0].kind, "preference");
        assert_eq!(memories[0].content, "Use focused UI controls.");
    }

    #[test]
    fn ignores_inline_mentions_of_sleep_fence() {
        let text = "memory content mentions ```a3s-sleep but no report";
        assert!(parse_sleep_report(text).is_none());
    }

    #[test]
    fn maps_sleep_memory_item_metadata() {
        let item = sleep_memory_item(
            &SleepMemory {
                kind: "knowledge".to_string(),
                content: "apps/web is a pure React frontend.".to_string(),
            },
            "2026-07-07",
        );
        assert_eq!(item.memory_type, MemoryType::Semantic);
        assert!(item.tags.contains(&"sleep".to_string()));
        assert!(item.tags.contains(&"knowledge".to_string()));
        assert_eq!(item.metadata.get("source"), Some(&"sleep".to_string()));
        assert_eq!(
            item.metadata.get("sleep_date"),
            Some(&"2026-07-07".to_string())
        );
    }
}
