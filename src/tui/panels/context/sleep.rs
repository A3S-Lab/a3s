//! `/sleep` — end-of-day consolidation: a `/loop`-driven turn reviews today's
//! work (across sessions via the `ctx` history CLI when installed), distills
//! successful experience, user preferences, and durable knowledge, and ends
//! with a machine-readable ```a3s-sleep report the host persists into the
//! agent's long-term memory (`~/.a3s/memory`, same store `/memory` browses).

use super::super::*;

/// Fence tag of the consolidation report (the TUI parses it from the final
/// message, like the asset-review ```a3s-review block).
pub(crate) const SLEEP_FENCE: &str = "```a3s-sleep";

/// One durable takeaway from a `/sleep` turn. Lenient — one model hiccup in
/// one of 20 items must not discard the whole report.
#[derive(Clone, serde::Deserialize)]
pub(crate) struct SleepMemory {
    #[serde(default)]
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) content: String,
}

/// The machine-readable report a sleep turn must end with.
#[derive(serde::Deserialize)]
struct SleepReport {
    memories: Vec<SleepMemory>,
}

/// Today as `YYYY-MM-DD` (memory provenance + the directive's anchor).
pub(crate) fn sleep_today() -> String {
    chrono::Local::now().format("%Y-%m-%d").to_string()
}

/// Directive for a `/sleep` turn. `focus` narrows the pass (from
/// `/sleep <focus>`); `ctx_ready` widens step 1 to cross-session history.
pub(crate) fn sleep_directive(focus: &str, ctx_ready: bool, today: &str) -> String {
    let ctx_clause = if ctx_ready {
        " Use the `ctx` history CLI (per your context-recall guide) to search today's \
         sessions across ALL projects — query several broad topics from today's work, \
         not just this directory's. If ctx returns nothing, move on —"
    } else {
        " Your only sources are listed next —"
    };
    let focus_clause = if focus.is_empty() {
        String::new()
    } else {
        format!("\n\nFocus especially on: {focus}")
    };
    format!(
        "You are running SLEEP consolidation for {today} — the end-of-day pass that \
         turns today's work into durable long-term memory.\n\
         1. Reconstruct what was worked on and accomplished today.{ctx_clause} \
         review this session's own conversation. Do NOT crawl the filesystem for \
         other sessions' data — ctx and this conversation are the only sources.\n\
         2. Distill ONLY durable, cross-session takeaways, of three kinds:\n\
         - \"experience\": an approach that WORKED today (or a mistake to avoid) with \
         the why — written so a future session can apply it directly.\n\
         - \"preference\": how the user likes things done — style, tools, language, \
         workflow — observed from their requests and corrections today.\n\
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

/// Extract the ```a3s-sleep report from a finished turn's text. Same shape as
/// `parse_review_report`: line-anchored closing fence (valid JSON can't hold a
/// raw newline in a string) and candidates tried back-to-front so prose after
/// the real block can't shadow it. The OPENING fence is line-anchored too —
/// a memory whose content mentions "```a3s-sleep" (likely on a day spent on
/// this very feature) must not make the real report unparseable.
pub(crate) fn parse_sleep_report(text: &str) -> Option<Vec<SleepMemory>> {
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

/// Last occurrence of `needle` that starts a line (position 0 or after `\n`).
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

/// Map one takeaway to a stored `MemoryItem`. Experience is procedural (how to
/// do things); preferences and knowledge are semantic facts; anything else
/// falls back to episodic. Tag + metadata provenance mirror `/ctx save`.
pub(crate) fn sleep_memory_item(m: &SleepMemory, today: &str) -> a3s_memory::MemoryItem {
    let kind = m.kind.trim().to_lowercase();
    let memory_type = match kind.as_str() {
        "experience" => a3s_memory::MemoryType::Procedural,
        "preference" | "knowledge" => a3s_memory::MemoryType::Semantic,
        _ => a3s_memory::MemoryType::Episodic,
    };
    let tag = if kind.is_empty() {
        "note".to_string()
    } else {
        kind
    };
    a3s_memory::MemoryItem::new(m.content.trim().to_string())
        .with_type(memory_type)
        .with_importance(0.75) // a deliberate consolidation pass > auto-recorded baseline
        .with_tags(vec!["sleep".to_string(), tag])
        .with_metadata("source", "sleep")
        .with_metadata("sleep_date", today)
}

impl App {
    /// Scan a finished `/sleep` turn for the report; on a hit, stop the loop
    /// and persist the items. Gated on `sleep_pending` so a turn that merely
    /// QUOTES an a3s-sleep block can't write phantom memories. Returns the
    /// async save command (caller batches it with `complete_turn`).
    pub(crate) fn capture_sleep(&mut self, text: &str) -> Option<Cmd<Msg>> {
        if !self.sleep_pending || !text.contains(SLEEP_FENCE) {
            return None;
        }
        let Some(memories) = parse_sleep_report(text) else {
            // Malformed: say so and stay pending, so a loop continuation or a
            // manual "re-emit the report" can still land it (review-style).
            self.push_line(&Style::new().fg(TN_YELLOW).render(
                "  ⚑ sleep report was malformed — ask the agent to re-emit the a3s-sleep block",
            ));
            return None;
        };
        // The deliverable arrived — stop the loop that was driving it.
        self.sleep_pending = false;
        self.loop_remaining = 0;
        let today = sleep_today();
        let items: Vec<a3s_memory::MemoryItem> = memories
            .iter()
            .filter(|m| !m.content.trim().is_empty())
            .take(20)
            .map(|m| sleep_memory_item(m, &today))
            .collect();
        if items.is_empty() {
            self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  ☾ nothing durable to keep from today"),
            );
            return None;
        }
        // Prefer the session's own memory handle: shares the store instance
        // (and its lock) with in-turn `remember`s, and the running session
        // gets the items in short-term recall immediately (same reasoning as
        // `/ctx save`). Standalone store only when the session has none.
        let mem = self.session.memory().cloned();
        let dir = self.memory_dir.clone();
        Some(cmd::cmd(move || async move {
            let n = items.len();
            let res = async {
                if let Some(mem) = mem {
                    for item in items {
                        mem.remember(item).await.map_err(|e| e.to_string())?;
                    }
                } else {
                    let store = a3s_memory::FileMemoryStore::new(&dir)
                        .await
                        .map_err(|e| e.to_string())?;
                    for item in items {
                        a3s_memory::MemoryStore::store(&store, item)
                            .await
                            .map_err(|e| e.to_string())?;
                    }
                }
                Ok(n)
            }
            .await;
            Msg::SleepSaved(res)
        }))
    }

    /// After a turn's capture attempt: if a `/sleep` run can no longer
    /// continue (loop budget spent or DONE-stopped) and no report was
    /// captured, disarm the gate — a dangling `sleep_pending` would let any
    /// later turn that merely quotes an a3s-sleep block phantom-save.
    pub(crate) fn disarm_sleep_if_over(&mut self, captured: bool) {
        if self.sleep_pending && !captured && self.loop_remaining == 0 {
            self.sleep_pending = false;
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  ☾ sleep ended without a memory report — /sleep to retry"),
            );
        }
    }

    /// A `/sleep` save finished: confirm, and refresh an open `/memory` panel
    /// so the consolidated items show immediately.
    pub(crate) fn on_sleep_saved(&mut self, res: Result<usize, String>) {
        match res {
            Ok(n) => {
                self.push_line(&Style::new().fg(TN_GREEN).render(&format!(
                    "  ☾ sleep consolidation: {n} memor{} saved · /memory to browse",
                    if n == 1 { "y" } else { "ies" }
                )));
                if let Some(m) = self.memory.as_mut() {
                    m.sel = 0;
                    m.apply_data(memutil::load_panel_data(&m.dir));
                }
            }
            Err(e) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  sleep consolidation failed to save: {e}")),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn block(json: &str) -> String {
        format!("prose before\n{SLEEP_FENCE}\n{json}\n```\nDONE")
    }

    #[test]
    fn parse_sleep_report_extracts_items() {
        let text = block(
            r#"{"memories": [{"kind": "experience", "content": "A worked"},
                            {"kind": "preference", "content": "B prefers"}]}"#,
        );
        let items = parse_sleep_report(&text).expect("report parses");
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].kind, "experience");
        assert_eq!(items[1].content, "B prefers");
    }

    #[test]
    fn parse_sleep_report_prefers_last_valid_block() {
        // A quoted (malformed) block earlier must not shadow the real one, and
        // prose after the real block must not break the line-anchored close.
        let text = format!(
            "the {SLEEP_FENCE} tag is documented\n```\nmid\n{}\ntrailing prose",
            block(r#"{"memories": [{"kind": "knowledge", "content": "real"}]}"#)
        );
        let items = parse_sleep_report(&text).expect("real block parses");
        assert_eq!(items.len(), 1);
        assert_eq!(items[0].content, "real");
    }

    #[test]
    fn parse_sleep_report_rejects_malformed() {
        assert!(parse_sleep_report(&block("not json")).is_none());
        assert!(parse_sleep_report("no fence at all").is_none());
    }

    #[test]
    fn echoed_directive_template_never_parses() {
        // A turn that merely quotes the instruction template (the model
        // restating its contract) must not phantom-save: the example block is
        // deliberately NOT valid JSON (angle-bracket placeholders).
        let directive = sleep_directive("", true, "2026-07-02");
        assert!(parse_sleep_report(&directive).is_none());
    }

    #[test]
    fn in_string_fence_mention_cannot_shadow_the_report() {
        // A memory about this very feature mentions the fence inside a JSON
        // string; the line-anchored opening search must skip it and still
        // parse the real report.
        let text = block(
            r#"{"memories": [{"kind": "knowledge", "content": "the TUI parses the ```a3s-sleep block"}]}"#,
        );
        let items = parse_sleep_report(&text).expect("in-string mention must not break parsing");
        assert_eq!(items.len(), 1);
        assert!(items[0].content.contains("a3s-sleep"));
    }

    #[test]
    fn sleep_memory_item_maps_kinds_and_provenance() {
        let today = "2026-07-02";
        let exp = sleep_memory_item(
            &SleepMemory {
                kind: "Experience".into(),
                content: " use flock ".into(),
            },
            today,
        );
        assert_eq!(exp.memory_type, a3s_memory::MemoryType::Procedural);
        assert_eq!(exp.content, "use flock");
        assert!(exp.tags.contains(&"sleep".to_string()));
        assert!(exp.tags.contains(&"experience".to_string()));

        let pref = sleep_memory_item(
            &SleepMemory {
                kind: "preference".into(),
                content: "ACL over TOML".into(),
            },
            today,
        );
        assert_eq!(pref.memory_type, a3s_memory::MemoryType::Semantic);

        let odd = sleep_memory_item(
            &SleepMemory {
                kind: "".into(),
                content: "misc".into(),
            },
            today,
        );
        assert_eq!(odd.memory_type, a3s_memory::MemoryType::Episodic);
        assert!(odd.tags.contains(&"note".to_string()));
    }

    #[test]
    fn sleep_directive_carries_contract_and_focus() {
        let d = sleep_directive("the box crate", true, "2026-07-02");
        assert!(d.contains(SLEEP_FENCE));
        assert!(d.contains("ctx")); // cross-session guidance when ctx is ready
        assert!(d.contains("Focus especially on: the box crate"));
        assert!(d.contains("2026-07-02"));
        let no_ctx = sleep_directive("", false, "2026-07-02");
        assert!(!no_ctx.contains("context-recall guide"));
        assert!(!no_ctx.contains("Focus especially"));
    }
}
