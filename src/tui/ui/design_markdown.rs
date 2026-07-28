//! DESIGN.md-aligned Markdown rendering for the coding transcript.
//!
//! The upstream `a3s-tui` renderer handles CommonMark and syntax highlighting,
//! but several block shapes can exceed the terminal width. This wrapper keeps
//! that parser/highlighter while enforcing the TUI's fixed-width contract and a
//! quieter Codex-style surface.

use std::{
    borrow::Cow,
    collections::VecDeque,
    ops::Range,
    time::{Duration, Instant},
};

use a3s_tui::{
    markdown::RenderedLineMetadata,
    style::{strip_ansi, visible_len, Color, Style},
};

use super::{ACCENT, SURFACE_SOFT, TN_CYAN, TN_FG, TN_GRAY};

const OSC8_CLOSE: &str = "\x1b]8;;\x1b\\";

fn ansi_escape_sequence_end(value: &str, start: usize) -> Option<usize> {
    let bytes = value.as_bytes();
    if bytes.get(start) != Some(&0x1b) {
        return None;
    }
    match bytes.get(start + 1).copied() {
        Some(b'[') => bytes[start + 2..]
            .iter()
            .position(|byte| (0x40..=0x7e).contains(byte))
            .map(|offset| start + 3 + offset),
        Some(b']') => {
            let mut index = start + 2;
            while index < bytes.len() {
                if bytes[index] == 0x07 {
                    return Some(index + 1);
                }
                if bytes[index] == 0x1b && bytes.get(index + 1) == Some(&b'\\') {
                    return Some(index + 2);
                }
                index += 1;
            }
            None
        }
        Some(_) => Some((start + 2).min(bytes.len())),
        None => None,
    }
}

fn osc8_link_target(sequence: &str) -> Option<&str> {
    let body = sequence.strip_prefix("\x1b]8;")?;
    let body = body
        .strip_suffix("\x1b\\")
        .or_else(|| body.strip_suffix('\x07'))?;
    let (_, target) = body.split_once(';')?;
    Some(target)
}

fn osc8_open(target: &str) -> String {
    format!("\x1b]8;;{target}\x1b\\")
}

#[derive(Clone)]
pub(crate) struct Markdown {
    width: usize,
}

struct RenderedDocument {
    lines: Vec<String>,
    line_metadata: Vec<RenderedLineMetadata>,
}

impl RenderedDocument {
    fn text(&self) -> String {
        self.lines.join("\n")
    }
}

impl Markdown {
    pub(crate) fn new() -> Self {
        Self { width: 80 }
    }

    pub(crate) fn with_width(mut self, width: usize) -> Self {
        self.width = width.max(1);
        self
    }

    pub(crate) fn render(&self, input: &str) -> String {
        self.render_document(input).text()
    }

    fn render_document(&self, input: &str) -> RenderedDocument {
        let json = pretty_print_complete_json(input);
        let normalized = unwrap_markdown_table_fences(json.as_ref());
        render_upstream_markdown(normalized.as_ref(), self.width)
    }
}

/// Pretty-print complete JSON without touching partial streaming fragments.
/// Handles a whole-message JSON value and complete top-level `json` fences;
/// prose, malformed JSON, and still-open fences remain source-identical.
fn pretty_print_complete_json(input: &str) -> Cow<'_, str> {
    let trimmed = input.trim();
    if !trimmed.is_empty() && matches!(trimmed.as_bytes().first(), Some(b'{') | Some(b'[')) {
        if let Ok(value) = serde_json::from_str::<serde_json::Value>(trimmed) {
            if let Ok(pretty) = serde_json::to_string_pretty(&value) {
                return Cow::Owned(format!("```json\n{pretty}\n```"));
            }
        }
    }

    let lines = input.split_inclusive('\n').collect::<Vec<_>>();
    let mut output = String::with_capacity(input.len());
    let mut changed = false;
    let mut index = 0usize;
    while index < lines.len() {
        let opening = lines[index];
        let opening_trimmed = opening.trim();
        let (marker, language) = if let Some(info) = opening_trimmed.strip_prefix("```") {
            ("```", info.trim())
        } else if let Some(info) = opening_trimmed.strip_prefix("~~~") {
            ("~~~", info.trim())
        } else {
            output.push_str(opening);
            index += 1;
            continue;
        };
        if !language.eq_ignore_ascii_case("json") {
            output.push_str(opening);
            index += 1;
            continue;
        }

        let Some(close_offset) = lines[index + 1..]
            .iter()
            .position(|line| line.trim() == marker)
        else {
            output.push_str(opening);
            index += 1;
            continue;
        };
        let close_index = index + 1 + close_offset;
        let body = lines[index + 1..close_index].concat();
        let Ok(value) = serde_json::from_str::<serde_json::Value>(body.trim()) else {
            output.extend(lines[index..=close_index].iter().copied());
            index = close_index + 1;
            continue;
        };
        let Ok(pretty) = serde_json::to_string_pretty(&value) else {
            output.extend(lines[index..=close_index].iter().copied());
            index = close_index + 1;
            continue;
        };
        output.push_str(opening);
        output.push_str(&pretty);
        output.push('\n');
        output.push_str(lines[close_index]);
        changed = true;
        index = close_index + 1;
    }

    if changed {
        Cow::Owned(output)
    } else {
        Cow::Borrowed(input)
    }
}

/// Unwrap complete `md`/`markdown` fences only when their body contains a
/// table. Models commonly fence Markdown examples, but agent transcript
/// tables should retain native table rendering just like Codex CLI. Other
/// fences and incomplete Markdown fences stay byte-for-byte intact as code.
fn unwrap_markdown_table_fences(input: &str) -> Cow<'_, str> {
    if !input.contains("```") && !input.contains("~~~") {
        return Cow::Borrowed(input);
    }

    #[derive(Clone, Copy)]
    struct Fence {
        marker: char,
        len: usize,
        blockquoted: bool,
    }

    struct Candidate {
        fence: Fence,
        opening: Range<usize>,
        body: Vec<Range<usize>>,
    }

    enum ActiveFence {
        Passthrough(Fence),
        Markdown(Box<Candidate>),
    }

    fn strip_fence_indent(line: &str) -> Option<&str> {
        let line = line
            .strip_suffix('\n')
            .unwrap_or(line)
            .strip_suffix('\r')
            .unwrap_or_else(|| line.strip_suffix('\n').unwrap_or(line));
        let mut byte_index = 0usize;
        let mut columns = 0usize;
        for byte in line.bytes() {
            match byte {
                b' ' => {
                    byte_index += 1;
                    columns += 1;
                }
                b'\t' => {
                    byte_index += 1;
                    columns += 4;
                }
                _ => break,
            }
            if columns >= 4 {
                return None;
            }
        }
        Some(&line[byte_index..])
    }

    fn parse_open_fence(line: &str) -> Option<(Fence, bool)> {
        let line = strip_fence_indent(line)?;
        let blockquoted = line.trim_start().starts_with('>');
        let fence_text = strip_blockquote_prefix(line);
        let (marker, len) = parse_fence_marker(fence_text)?;
        let info = fence_text[len..]
            .split_whitespace()
            .next()
            .unwrap_or_default();
        let markdown = info.eq_ignore_ascii_case("md") || info.eq_ignore_ascii_case("markdown");
        Some((
            Fence {
                marker,
                len,
                blockquoted,
            },
            markdown,
        ))
    }

    fn is_close_fence(line: &str, fence: Fence) -> bool {
        let Some(line) = strip_fence_indent(line) else {
            return false;
        };
        let fence_text = if fence.blockquoted {
            if !line.trim_start().starts_with('>') {
                return false;
            }
            strip_blockquote_prefix(line)
        } else {
            line
        };
        parse_fence_marker(fence_text).is_some_and(|(marker, len)| {
            marker == fence.marker && len >= fence.len && fence_text[len..].trim().is_empty()
        })
    }

    fn ranges_content(input: &str, ranges: &[Range<usize>]) -> String {
        let mut content = String::with_capacity(ranges.iter().map(ExactSizeIterator::len).sum());
        for range in ranges {
            content.push_str(&input[range.clone()]);
        }
        content
    }

    fn blank_fence_line(line: &str) -> &str {
        if line.ends_with('\n') {
            "\n"
        } else {
            ""
        }
    }

    fn contains_table(input: &str, blockquoted: bool) -> bool {
        let mut previous = None;
        for line in input.lines() {
            let line = if blockquoted {
                strip_blockquote_prefix(line)
            } else {
                line
            };
            let line = line.trim();
            if line.is_empty() {
                previous = None;
                continue;
            }
            if previous.is_some_and(is_stream_table_header)
                && !previous.is_some_and(is_stream_table_delimiter)
                && is_stream_table_delimiter(line)
            {
                return true;
            }
            previous = Some(line);
        }
        false
    }

    let mut output = String::with_capacity(input.len());
    let mut active = None;
    let mut source_offset = 0usize;

    for line in input.split_inclusive('\n') {
        let start = source_offset;
        source_offset += line.len();
        let range = start..source_offset;

        if let Some(current) = active.take() {
            match current {
                ActiveFence::Passthrough(fence) => {
                    output.push_str(&input[range]);
                    if !is_close_fence(line, fence) {
                        active = Some(ActiveFence::Passthrough(fence));
                    }
                }
                ActiveFence::Markdown(mut candidate) => {
                    if is_close_fence(line, candidate.fence) {
                        let body = ranges_content(input, &candidate.body);
                        if contains_table(&body, candidate.fence.blockquoted) {
                            output.push_str(blank_fence_line(&input[candidate.opening]));
                            output.push_str(&body);
                            output.push_str(blank_fence_line(&input[range]));
                        } else {
                            output.push_str(&input[candidate.opening]);
                            output.push_str(&body);
                            output.push_str(&input[range]);
                        }
                    } else {
                        candidate.body.push(range);
                        active = Some(ActiveFence::Markdown(candidate));
                    }
                }
            }
            continue;
        }

        if let Some((fence, markdown)) = parse_open_fence(line) {
            if markdown {
                active = Some(ActiveFence::Markdown(Box::new(Candidate {
                    fence,
                    opening: range,
                    body: Vec::new(),
                })));
            } else {
                output.push_str(&input[range]);
                active = Some(ActiveFence::Passthrough(fence));
            }
        } else {
            output.push_str(&input[range]);
        }
    }

    if let Some(ActiveFence::Markdown(candidate)) = active {
        output.push_str(&input[candidate.opening]);
        for range in candidate.body {
            output.push_str(&input[range]);
        }
    }

    Cow::Owned(output)
}

fn render_upstream_markdown(input: &str, viewport_width: usize) -> RenderedDocument {
    // The shared renderer owns the one CommonMark AST and performs responsive
    // table layout at the real viewport width. Keeping the complete document
    // in that parse preserves reference definitions and list/quote ancestry
    // across table boundaries.
    let rendered = a3s_tui::markdown::Markdown::new()
        .with_width(viewport_width)
        .render_with_metadata(input);
    let normalized = normalize_markdown_colors(rendered.as_str());
    bound_rendered_markdown(&normalized, viewport_width, rendered.line_metadata())
}

impl Default for Markdown {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Clone)]
pub(crate) struct StreamingMarkdown {
    /// Every delta exactly as received. This is the source of truth for final
    /// rendering and width changes; rendered terminal rows never feed back
    /// into this buffer.
    buffer: String,
    /// Newline-terminated source currently eligible for the live view.
    committed_source_len: usize,
    /// Rendered rows that are structurally safe to commit to scrollback.
    stable_lines: Vec<String>,
    /// Rendered rows that may still change shape, currently an active table or
    /// a speculative table header.
    tail_lines: Vec<String>,
    /// Stable rows that have crossed the animation boundary. Unlike
    /// `stable_lines`, these are the exact snapshots the user has already
    /// seen; later Markdown re-renders must never rewrite them in place.
    emitted_stable_lines: Vec<String>,
    /// Number of stable rows already released by commit ticks. Stable rows
    /// after this boundary remain queued and are deliberately absent from the
    /// live viewport until the pacing policy emits them.
    emitted_stable_len: usize,
    /// Boundary in the latest stable render snapshot through which rows have
    /// either been emitted or enqueued. This mirrors Codex's separate emitted
    /// and enqueued cursors and prevents queued rows from reappearing in tail.
    enqueued_stable_len: usize,
    /// FIFO snapshots of stable rendered rows. Keeping the row with its age is
    /// essential: a later delta may re-render the document, but it must not
    /// mutate content already waiting in the commit animation.
    queued_stable_lines: VecDeque<QueuedStableLine>,
    chunking: AdaptiveStreamChunking,
    md: Markdown,
    #[cfg(test)]
    rerender_count: usize,
}

#[derive(Clone, Debug)]
struct QueuedStableLine {
    line: String,
    enqueued_at: Instant,
}

impl StreamingMarkdown {
    pub(crate) fn new(width: usize) -> Self {
        Self {
            buffer: String::new(),
            committed_source_len: 0,
            stable_lines: Vec::new(),
            tail_lines: Vec::new(),
            emitted_stable_lines: Vec::new(),
            emitted_stable_len: 0,
            enqueued_stable_len: 0,
            queued_stable_lines: VecDeque::new(),
            chunking: AdaptiveStreamChunking::default(),
            md: Markdown::new().with_width(width),
            #[cfg(test)]
            rerender_count: 0,
        }
    }

    /// Append source and expose only complete newline-terminated content.
    ///
    /// Returns `true` when the committed render snapshot changed or new stable
    /// rows entered the commit queue. A complete speculative table header is
    /// immediately available in the mutable tail; ordinary stable rows become
    /// visible through subsequent `commit_tick()` calls.
    pub(crate) fn push(&mut self, token: &str) -> bool {
        let token_start = self.buffer.len();
        self.buffer.push_str(token);
        let Some(last_newline) = token.rfind('\n') else {
            return false;
        };
        let commit_end = token_start + last_newline + 1;
        if commit_end <= self.committed_source_len {
            return false;
        }
        self.committed_source_len = commit_end;
        self.rerender_committed();
        true
    }

    pub(crate) fn clear(&mut self) {
        self.buffer.clear();
        self.committed_source_len = 0;
        self.stable_lines.clear();
        self.tail_lines.clear();
        self.emitted_stable_lines.clear();
        self.emitted_stable_len = 0;
        self.enqueued_stable_len = 0;
        self.queued_stable_lines.clear();
        self.chunking.reset();
        #[cfg(test)]
        {
            self.rerender_count = 0;
        }
    }

    #[cfg(test)]
    pub(crate) fn view(&self) -> String {
        join_render_regions(&self.stable_lines, &self.tail_lines)
    }

    /// The immutable portion of the current newline-gated render.
    #[cfg(test)]
    pub(crate) fn stable_view(&self) -> String {
        self.stable_lines.join("\n")
    }

    /// Stable rows released by commit ticks and therefore visible in the live
    /// transcript. The complete stable region remains available through
    /// `stable_view()` for structural tests and source-backed reflow.
    pub(crate) fn visible_stable_view(&self) -> String {
        self.emitted_stable_lines.join("\n")
    }

    /// The current speculative/table render that may be replaced by the next
    /// complete source line.
    pub(crate) fn tail_view(&self) -> String {
        self.tail_lines.join("\n")
    }

    pub(crate) fn raw_content(&self) -> &str {
        &self.buffer
    }

    /// Canonical render of every source byte received so far, including an
    /// unterminated final line. Used by the live Ctrl+T transcript tail; it is
    /// render-only and does not advance commit state.
    pub(crate) fn full_view(&self) -> String {
        self.md.render(&self.buffer)
    }

    #[cfg(test)]
    pub(crate) fn queued_lines(&self) -> usize {
        self.queued_stable_lines.len()
    }

    /// Release stable rows according to Codex CLI's smooth/catch-up policy.
    /// Smooth mode emits one row per animation tick; a queue depth of eight or
    /// an oldest-row age of 120 ms switches to a full catch-up drain.
    pub(crate) fn commit_tick(&mut self, now: Instant) -> bool {
        self.commit_tick_with_scope(now, false)
    }

    /// Apply policy transitions immediately after enqueueing, but only drain
    /// when those transitions select catch-up mode. Codex performs this extra
    /// tick on newline deltas so a burst never waits for the next animation
    /// frame, while smooth one-row pacing still belongs to the periodic clock.
    pub(crate) fn commit_catch_up_tick(&mut self, now: Instant) -> bool {
        self.commit_tick_with_scope(now, true)
    }

    fn commit_tick_with_scope(&mut self, now: Instant, catch_up_only: bool) -> bool {
        let queued_lines = self.queued_stable_lines.len();
        let oldest_age = self
            .queued_stable_lines
            .front()
            .map(|queued| now.saturating_duration_since(queued.enqueued_at));
        let drain = self.chunking.drain_count(queued_lines, oldest_age, now);
        if drain == 0 || (catch_up_only && self.chunking.mode != StreamChunkingMode::CatchUp) {
            return false;
        }
        let drain = drain.min(self.queued_stable_lines.len());
        self.emitted_stable_lines.extend(
            self.queued_stable_lines
                .drain(..drain)
                .map(|queued| queued.line),
        );
        self.emitted_stable_len = self
            .emitted_stable_len
            .saturating_add(drain)
            .min(self.enqueued_stable_len);
        true
    }

    #[cfg(test)]
    pub(crate) fn final_view(&self) -> String {
        self.full_view()
    }

    /// Re-render both live regions from raw source at a new terminal width.
    ///
    /// Keeping this operation source-backed avoids compounding prior wrapping
    /// or table compaction when the terminal is repeatedly resized.
    pub(crate) fn set_width(&mut self, width: usize) {
        let width = width.max(1);
        if self.md.width == width {
            return;
        }
        self.md = Markdown::new().with_width(width);
        self.rerender_committed();
        // A resize is an explicit global reflow boundary. Publish the complete
        // stable prefix at the new width in one transaction, then leave only
        // the structurally mutable table/header region in tail. Reusing the
        // old rendered-row count at a new width can place the tail before newly
        // wrapped continuation rows.
        self.emitted_stable_lines.clone_from(&self.stable_lines);
        self.emitted_stable_len = self.stable_lines.len();
        self.enqueued_stable_len = self.stable_lines.len();
        self.queued_stable_lines.clear();
        self.chunking.reset();
    }

    fn rerender_committed(&mut self) {
        #[cfg(test)]
        {
            self.rerender_count += 1;
        }
        let source = &self.buffer[..self.committed_source_len];
        let holdback = table_holdback_state(source);
        let provisional_source = provisional_stream_source(source, holdback);
        let render_source = provisional_source.as_ref();
        let rendered = self.md.render_document(render_source);
        let mut stable_len =
            trailing_table_start(&rendered, render_source).unwrap_or(rendered.lines.len());
        let lexical_boundary = match holdback {
            TableHoldbackState::None => rendered.lines.len(),
            TableHoldbackState::PendingHeader { header_start }
            | TableHoldbackState::Confirmed {
                table_start: header_start,
            } => rendered_line_for_source_offset(&rendered, render_source, header_start),
        };
        stable_len = stable_len.min(lexical_boundary);
        let rendered_lines = rendered.lines;
        self.stable_lines = rendered_lines[..stable_len].to_vec();
        self.tail_lines = rendered_lines[stable_len..].to_vec();
        self.sync_stable_queue();
    }

    fn sync_stable_queue(&mut self) {
        let target_stable_len = self.stable_lines.len().max(self.emitted_stable_len);
        if target_stable_len < self.enqueued_stable_len {
            self.queued_stable_lines.clear();
            if self.emitted_stable_len < target_stable_len {
                self.enqueue_stable_range(self.emitted_stable_len, target_stable_len);
            }
            self.enqueued_stable_len = target_stable_len;
            return;
        }
        if target_stable_len == self.enqueued_stable_len {
            return;
        }
        self.enqueue_stable_range(self.enqueued_stable_len, target_stable_len);
        self.enqueued_stable_len = target_stable_len;
    }

    fn enqueue_stable_range(&mut self, start: usize, end: usize) {
        let end = end.min(self.stable_lines.len());
        if start >= end {
            return;
        }
        let now = Instant::now();
        self.queued_stable_lines
            .extend(
                self.stable_lines[start..end]
                    .iter()
                    .cloned()
                    .map(|line| QueuedStableLine {
                        line,
                        enqueued_at: now,
                    }),
            );
    }
}

const ENTER_CATCH_UP_QUEUE_DEPTH: usize = 8;
const ENTER_CATCH_UP_OLDEST_AGE: Duration = Duration::from_millis(120);
const EXIT_CATCH_UP_QUEUE_DEPTH: usize = 2;
const EXIT_CATCH_UP_OLDEST_AGE: Duration = Duration::from_millis(40);
const EXIT_CATCH_UP_HOLD: Duration = Duration::from_millis(250);
const REENTER_CATCH_UP_HOLD: Duration = Duration::from_millis(250);
const SEVERE_QUEUE_DEPTH: usize = 64;
const SEVERE_OLDEST_AGE: Duration = Duration::from_millis(300);

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum StreamChunkingMode {
    #[default]
    Smooth,
    CatchUp,
}

#[derive(Clone, Debug, Default)]
struct AdaptiveStreamChunking {
    mode: StreamChunkingMode,
    below_exit_threshold_since: Option<Instant>,
    last_catch_up_exit_at: Option<Instant>,
}

impl AdaptiveStreamChunking {
    fn reset(&mut self) {
        *self = Self::default();
    }

    fn drain_count(
        &mut self,
        queued_lines: usize,
        oldest_age: Option<Duration>,
        now: Instant,
    ) -> usize {
        if queued_lines == 0 {
            if self.mode == StreamChunkingMode::CatchUp {
                self.last_catch_up_exit_at = Some(now);
            }
            self.mode = StreamChunkingMode::Smooth;
            self.below_exit_threshold_since = None;
            return 0;
        }

        match self.mode {
            StreamChunkingMode::Smooth => {
                let pressure = queued_lines >= ENTER_CATCH_UP_QUEUE_DEPTH
                    || oldest_age.is_some_and(|age| age >= ENTER_CATCH_UP_OLDEST_AGE);
                let severe = queued_lines >= SEVERE_QUEUE_DEPTH
                    || oldest_age.is_some_and(|age| age >= SEVERE_OLDEST_AGE);
                let reentry_hold = self.last_catch_up_exit_at.is_some_and(|exit| {
                    now.saturating_duration_since(exit) < REENTER_CATCH_UP_HOLD
                });
                if pressure && (!reentry_hold || severe) {
                    self.mode = StreamChunkingMode::CatchUp;
                    self.below_exit_threshold_since = None;
                    self.last_catch_up_exit_at = None;
                }
            }
            StreamChunkingMode::CatchUp => {
                let below_exit = queued_lines <= EXIT_CATCH_UP_QUEUE_DEPTH
                    && oldest_age.is_some_and(|age| age <= EXIT_CATCH_UP_OLDEST_AGE);
                if below_exit {
                    match self.below_exit_threshold_since {
                        Some(since)
                            if now.saturating_duration_since(since) >= EXIT_CATCH_UP_HOLD =>
                        {
                            self.mode = StreamChunkingMode::Smooth;
                            self.below_exit_threshold_since = None;
                            self.last_catch_up_exit_at = Some(now);
                        }
                        Some(_) => {}
                        None => self.below_exit_threshold_since = Some(now),
                    }
                } else {
                    self.below_exit_threshold_since = None;
                }
            }
        }

        match self.mode {
            StreamChunkingMode::Smooth => 1,
            StreamChunkingMode::CatchUp => queued_lines,
        }
    }
}

fn trailing_table_start(rendered: &RenderedDocument, source: &str) -> Option<usize> {
    let committed_lines = source.bytes().filter(|byte| *byte == b'\n').count();
    rendered.line_metadata.iter().position(|metadata| {
        metadata.is_table()
            && metadata
                .source()
                .is_some_and(|range| range.end() == committed_lines)
    })
}

fn rendered_line_for_source_offset(
    rendered: &RenderedDocument,
    source: &str,
    source_offset: usize,
) -> usize {
    let source_offset = source_offset.min(source.len());
    let source_line = source[..source_offset]
        .bytes()
        .filter(|byte| *byte == b'\n')
        .count()
        .saturating_add(1);
    rendered
        .line_metadata
        .iter()
        .position(|metadata| {
            metadata
                .source()
                .is_some_and(|range| range.contains(source_line) || range.start() >= source_line)
        })
        .unwrap_or(rendered.lines.len())
}

#[cfg(test)]
fn join_render_regions(stable: &[String], tail: &[String]) -> String {
    match (stable.is_empty(), tail.is_empty()) {
        (true, true) => String::new(),
        (false, true) => stable.join("\n"),
        (true, false) => tail.join("\n"),
        (false, false) => format!("{}\n{}", stable.join("\n"), tail.join("\n")),
    }
}

/// A pipe-table can reshape every previous row when a new row arrives, so its
/// source stays in the mutable live tail until finalization. A lone candidate
/// header is held for one complete line because only its successor can prove
/// whether it starts a table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TableHoldbackState {
    None,
    PendingHeader { header_start: usize },
    Confirmed { table_start: usize },
}

/// Complete only the structurally mutable suffix for the live parse. This is
/// the terminal equivalent of Streamdown's remend pass: committed source is
/// never changed, while a candidate table gets enough temporary syntax to
/// render as its final block shape instead of flashing raw pipe text.
fn provisional_stream_source(source: &str, holdback: TableHoldbackState) -> Cow<'_, str> {
    let mut suffix = String::new();
    if matches!(holdback, TableHoldbackState::PendingHeader { .. }) {
        if let Some(delimiter) = provisional_table_delimiter(source) {
            suffix.push_str(&delimiter);
        }
    }

    if !matches!(holdback, TableHoldbackState::None) {
        let mut fence = FenceTracker::default();
        for source_line in source.split_inclusive('\n') {
            let line = source_line.strip_suffix('\n').unwrap_or(source_line);
            let line = line.strip_suffix('\r').unwrap_or(line);
            fence.advance(line);
        }
        if let Some(closing_line) = fence.markdown_closing_line() {
            suffix.push_str(&closing_line);
        }
    }

    if suffix.is_empty() {
        Cow::Borrowed(source)
    } else {
        Cow::Owned(format!("{source}{suffix}"))
    }
}

fn provisional_table_delimiter(source: &str) -> Option<String> {
    let source = source.strip_suffix('\n').unwrap_or(source);
    let raw_line = source.rsplit('\n').next()?;
    let raw_line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
    let header = strip_blockquote_prefix(raw_line);
    let prefix_len = raw_line.len().saturating_sub(header.len());
    let column_count = parse_stream_table_segments(header)?.len();
    if column_count == 0 {
        return None;
    }
    let delimiter = std::iter::repeat_n("---", column_count)
        .collect::<Vec<_>>()
        .join(" | ");
    Some(format!("{}| {delimiter} |\n", &raw_line[..prefix_len]))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FenceKind {
    Outside,
    Markdown,
    Other,
}

#[derive(Clone, Copy)]
struct PreviousTableLine {
    source_start: usize,
    fence_kind: FenceKind,
    is_header: bool,
    is_delimiter: bool,
}

/// Scan newline-committed source only for shapes that do not yet exist as an
/// AST table: a trailing candidate header, or a table inside an unclosed
/// Markdown fence that will be unwrapped when its closing fence arrives.
fn table_holdback_state(source: &str) -> TableHoldbackState {
    let mut source_offset = 0usize;
    let mut fence = FenceTracker::default();
    let mut previous = None;
    let mut pending_header_start = None;
    let mut markdown_fence_start = None;
    let mut markdown_table_start = None;
    let mut native_table_active = false;

    for source_line in source.split_inclusive('\n') {
        let line = source_line.strip_suffix('\n').unwrap_or(source_line);
        let line = line.strip_suffix('\r').unwrap_or(line);
        let fence_kind = fence.kind();
        // A completed Markdown fence containing a table is unwrapped during
        // rendering. Hold the opening marker too, otherwise a code-box row
        // from the incomplete fence could be committed as stable just before
        // the same source reshapes into a native table.
        let mutable_source_start = if fence_kind == FenceKind::Markdown {
            markdown_fence_start.unwrap_or(source_offset)
        } else {
            source_offset
        };
        let candidate = (fence_kind != FenceKind::Other)
            .then(|| strip_blockquote_prefix(line).trim())
            .filter(|line| parse_stream_table_segments(line).is_some());
        let is_header = candidate.is_some_and(is_stream_table_header);
        let is_delimiter = candidate.is_some_and(is_stream_table_delimiter);

        let confirmed_markdown_table = if let Some(PreviousTableLine {
            source_start,
            fence_kind: previous_fence,
            is_header: true,
            is_delimiter: false,
        }) = previous
        {
            (previous_fence == FenceKind::Markdown
                && fence_kind == FenceKind::Markdown
                && is_delimiter)
                .then_some(source_start)
        } else {
            None
        };
        if let Some(table_start) = confirmed_markdown_table {
            markdown_table_start = Some(table_start);
        }

        let confirmed_native_table = matches!(
            previous,
            Some(PreviousTableLine {
                fence_kind: FenceKind::Outside,
                is_header: true,
                is_delimiter: false,
                ..
            })
        ) && fence_kind == FenceKind::Outside
            && is_delimiter;
        if confirmed_native_table {
            native_table_active = true;
        }
        let continues_native_table =
            native_table_active && fence_kind == FenceKind::Outside && candidate.is_some();
        if !continues_native_table {
            native_table_active = false;
        }

        if continues_native_table || line.trim().is_empty() || is_delimiter {
            pending_header_start = None;
        } else {
            pending_header_start = if fence_kind != FenceKind::Other && is_header {
                Some(mutable_source_start)
            } else {
                None
            };
        }

        previous = Some(PreviousTableLine {
            source_start: mutable_source_start,
            fence_kind,
            is_header,
            is_delimiter,
        });
        fence.advance(line);
        match (fence_kind, fence.kind()) {
            (FenceKind::Outside, FenceKind::Markdown) => {
                markdown_fence_start = Some(source_offset);
            }
            (FenceKind::Markdown, FenceKind::Outside) => {
                markdown_fence_start = None;
                markdown_table_start = None;
                pending_header_start = None;
            }
            _ => {}
        }
        source_offset = source_offset.saturating_add(source_line.len());
    }

    if let Some(table_start) = markdown_table_start {
        TableHoldbackState::Confirmed { table_start }
    } else {
        pending_header_start.map_or(TableHoldbackState::None, |header_start| {
            TableHoldbackState::PendingHeader { header_start }
        })
    }
}

fn parse_stream_table_segments(line: &str) -> Option<Vec<&str>> {
    let trimmed = line.trim();
    if trimmed.is_empty() {
        return None;
    }

    let has_outer_pipe = trimmed.starts_with('|') || trimmed.ends_with('|');
    let content = trimmed.strip_prefix('|').unwrap_or(trimmed);
    let content = content.strip_suffix('|').unwrap_or(content);
    let segments = split_unescaped_pipes(content)
        .into_iter()
        .map(str::trim)
        .collect::<Vec<_>>();
    (has_outer_pipe || segments.len() > 1).then_some(segments)
}

fn split_unescaped_pipes(content: &str) -> Vec<&str> {
    let mut segments = Vec::new();
    let mut start = 0usize;
    let bytes = content.as_bytes();
    let mut index = 0usize;
    while index < bytes.len() {
        match bytes[index] {
            b'\\' => index = (index + 2).min(bytes.len()),
            b'|' => {
                segments.push(&content[start..index]);
                start = index + 1;
                index += 1;
            }
            _ => index += 1,
        }
    }
    segments.push(&content[start..]);
    segments
}

fn is_stream_table_header(line: &str) -> bool {
    parse_stream_table_segments(line)
        .is_some_and(|segments| segments.iter().any(|segment| !segment.is_empty()))
}

fn is_stream_table_delimiter(line: &str) -> bool {
    parse_stream_table_segments(line).is_some_and(|segments| {
        segments.into_iter().all(|segment| {
            let segment = segment.trim();
            let segment = segment.strip_prefix(':').unwrap_or(segment);
            let segment = segment.strip_suffix(':').unwrap_or(segment);
            segment.len() >= 3 && segment.chars().all(|ch| ch == '-')
        })
    })
}

fn strip_blockquote_prefix(line: &str) -> &str {
    let mut rest = line.trim_start();
    while let Some(stripped) = rest.strip_prefix('>') {
        rest = stripped.strip_prefix(' ').unwrap_or(stripped).trim_start();
    }
    rest
}

#[derive(Default)]
struct FenceTracker {
    open: Option<(char, usize, FenceKind, bool)>,
}

impl FenceTracker {
    fn kind(&self) -> FenceKind {
        self.open.map_or(FenceKind::Outside, |(_, _, kind, _)| kind)
    }

    fn markdown_closing_line(&self) -> Option<String> {
        let (marker, len, FenceKind::Markdown, blockquoted) = self.open? else {
            return None;
        };
        let quote = if blockquoted { "> " } else { "" };
        Some(format!("{quote}{}\n", marker.to_string().repeat(len)))
    }

    fn advance(&mut self, raw_line: &str) {
        let leading_spaces = raw_line
            .as_bytes()
            .iter()
            .take_while(|byte| **byte == b' ')
            .count();
        if leading_spaces > 3 {
            return;
        }
        let unindented = &raw_line[leading_spaces..];
        let blockquoted = unindented.trim_start().starts_with('>');
        let line = strip_blockquote_prefix(unindented);
        let Some((marker, len)) = parse_fence_marker(line) else {
            return;
        };

        if let Some((open_marker, open_len, _, _)) = self.open {
            if marker == open_marker && len >= open_len && line[len..].trim().is_empty() {
                self.open = None;
            }
        } else {
            let info = line[len..].split_whitespace().next().unwrap_or_default();
            let kind = if info.eq_ignore_ascii_case("md") || info.eq_ignore_ascii_case("markdown") {
                FenceKind::Markdown
            } else {
                FenceKind::Other
            };
            self.open = Some((marker, len, kind, blockquoted));
        }
    }
}

fn parse_fence_marker(line: &str) -> Option<(char, usize)> {
    let first = line.as_bytes().first().copied()?;
    if first != b'`' && first != b'~' {
        return None;
    }
    let len = line.bytes().take_while(|byte| *byte == first).count();
    (len >= 3).then_some((first as char, len))
}

fn bound_rendered_markdown(
    rendered: &str,
    width: usize,
    input_metadata: &[RenderedLineMetadata],
) -> RenderedDocument {
    let mut rows = Vec::new();
    let mut line_metadata = Vec::new();
    for (line_index, line) in rendered.lines().enumerate() {
        let metadata = *input_metadata
            .get(line_index)
            .expect("shared Markdown metadata must align with rendered rows");
        let wrapped = if metadata.is_non_wrapping() || metadata.is_table() {
            vec![line.to_string()]
        } else {
            let plain = strip_ansi(line);
            if is_generated_rule(&plain) {
                vec![render_generated_rule(&plain, width)]
            } else {
                let hanging = markdown_hanging_prefix(&plain);
                if hanging.width > 0 && visible_len(line) > width {
                    let (prefix, body) = split_after_visible_width(line, hanging.width);
                    wrap_ansi_words(body, width, prefix, &hanging.continuation)
                } else {
                    wrap_ansi_words(line, width, "", "")
                }
            }
        };
        line_metadata.extend(std::iter::repeat_n(metadata, wrapped.len()));
        rows.extend(wrapped);
    }
    RenderedDocument {
        lines: rows,
        line_metadata,
    }
}

struct MarkdownHangingPrefix {
    width: usize,
    continuation: String,
}

/// Return the compound structural prefix that continuation rows must retain.
/// Quote and list contexts can be nested in either order (`│ - item` or
/// `  │ quote`), so a width alone is insufficient: the continuation must
/// replay quote gutters while replacing a list marker with spaces.
fn markdown_hanging_prefix(plain: &str) -> MarkdownHangingPrefix {
    let leading_bytes = plain
        .char_indices()
        .take_while(|(_, ch)| ch.is_whitespace())
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    let leading_width = visible_len(&plain[..leading_bytes]);
    let mut rest = &plain[leading_bytes..];
    let mut width = leading_width;
    let mut continuation = plain[..leading_bytes].to_string();

    while let Some(tail) = rest.strip_prefix("│ ") {
        continuation.push_str(&Style::new().fg(TN_GRAY).render("│"));
        continuation.push(' ');
        width = width.saturating_add(2);
        rest = tail;
    }

    for marker in ["-", "✔", "□"] {
        if rest
            .strip_prefix(marker)
            .is_some_and(|tail| tail.starts_with(' '))
        {
            let marker_width = visible_len(marker).saturating_add(1);
            continuation.push_str(&" ".repeat(marker_width));
            return MarkdownHangingPrefix {
                width: width.saturating_add(marker_width),
                continuation,
            };
        }
    }

    let digit_bytes = rest
        .char_indices()
        .take_while(|(_, ch)| ch.is_ascii_digit())
        .map(|(index, ch)| index + ch.len_utf8())
        .last()
        .unwrap_or(0);
    if digit_bytes > 0 {
        let suffix = &rest[digit_bytes..];
        if suffix
            .chars()
            .next()
            .is_some_and(|marker| matches!(marker, '.' | ')'))
            && suffix[1..].starts_with(' ')
        {
            let marker_width = visible_len(&rest[..digit_bytes]).saturating_add(2);
            continuation.push_str(&" ".repeat(marker_width));
            return MarkdownHangingPrefix {
                width: width.saturating_add(marker_width),
                continuation,
            };
        }
    }

    MarkdownHangingPrefix {
        width,
        continuation,
    }
}

/// Generated horizontal rules can be safely reflowed without retaining every
/// repeated dash. Unlike prose, code, URLs, headings, and table cells, these
/// glyphs are terminal decoration rather than source content.
fn is_generated_rule(plain: &str) -> bool {
    !plain.is_empty()
        && plain.chars().all(|ch| {
            matches!(
                ch,
                '─' | '┌' | '┐' | '└' | '┘' | '├' | '┤' | '┬' | '┴' | '┼'
            )
        })
}

fn render_generated_rule(plain: &str, width: usize) -> String {
    if width == 0 || plain.is_empty() {
        return String::new();
    }
    let first = plain.chars().next().unwrap_or('─');
    let last = plain.chars().last().unwrap_or(first);
    let fitted = match width {
        1 => first.to_string(),
        _ if plain.chars().all(|ch| ch == '─') => "─".repeat(width),
        _ => format!("{first}{}{last}", "─".repeat(width.saturating_sub(2))),
    };
    Style::new().fg(TN_GRAY).render(&fitted)
}

/// Split after complete display cells while retaining every ANSI sequence on
/// the side where it originally occurred.
fn split_after_visible_width(line: &str, target_width: usize) -> (&str, &str) {
    let mut index = 0usize;
    let mut used = 0usize;
    while index < line.len() && used < target_width {
        if let Some(end) = ansi_escape_sequence_end(line, index) {
            index = end;
            continue;
        }
        let end = next_grapheme_cluster_end(line, index);
        let cluster_width = visible_len(&line[index..end]);
        if used.saturating_add(cluster_width) > target_width {
            break;
        }
        used = used.saturating_add(cluster_width);
        index = end;
    }
    line.split_at(index)
}

/// Losslessly wrap an ANSI-styled prose row. Continuations replay active SGR
/// and OSC 8 state, emitted rows close both, and wrapping prefers whitespace
/// before falling back to a hard grapheme boundary.
fn wrap_ansi_words(
    payload: &str,
    width: usize,
    first_prefix: &str,
    continuation_prefix: &str,
) -> Vec<String> {
    if width == 0 {
        return vec![String::new()];
    }

    #[derive(Clone)]
    struct BreakPoint {
        resume_index: usize,
        row_len: usize,
        active: ActiveAnsi,
    }

    let mut rows = Vec::new();
    let mut active = ActiveAnsi::default();
    let mut row = first_prefix.to_string();
    let mut row_width = visible_len(first_prefix);
    let mut row_has_payload = false;
    let mut last_non_space_len = row.len();
    let mut break_point: Option<BreakPoint> = None;
    let mut in_whitespace = false;
    let mut index = 0usize;

    while index < payload.len() {
        if let Some(end) = ansi_escape_sequence_end(payload, index) {
            let sequence = &payload[index..end];
            row.push_str(sequence);
            update_active_ansi(sequence, &mut active);
            index = end;
            if in_whitespace {
                if let Some(point) = &mut break_point {
                    point.resume_index = end;
                    point.active.clone_from(&active);
                }
            } else {
                last_non_space_len = row.len();
            }
            continue;
        }

        let end = next_grapheme_cluster_end(payload, index);
        let cluster = &payload[index..end];
        let cluster_width = visible_len(cluster);
        let whitespace = cluster.chars().all(char::is_whitespace);

        if whitespace {
            if cluster_width > 0
                && row_width.saturating_add(cluster_width) > width
                && row_has_payload
            {
                row.truncate(last_non_space_len);
                finish_ansi_row(&mut row, &active);
                rows.push(row);
                row = continuation_prefix.to_string();
                row_width = visible_len(continuation_prefix);
                replay_active_ansi(&mut row, &active);
                row_has_payload = false;
                last_non_space_len = row.len();
                break_point = None;
                in_whitespace = false;
                index = end;
                continue;
            }

            row.push_str(cluster);
            row_width = row_width.saturating_add(cluster_width);
            in_whitespace = true;
            break_point = Some(BreakPoint {
                resume_index: end,
                row_len: last_non_space_len,
                active: active.clone(),
            });
            index = end;
            continue;
        }

        if cluster_width > 0 && row_width.saturating_add(cluster_width) > width {
            if let Some(point) = break_point
                .take()
                .filter(|point| point.row_len > first_prefix.len() || first_prefix.is_empty())
            {
                row.truncate(point.row_len);
                active = point.active;
                finish_ansi_row(&mut row, &active);
                rows.push(row);
                row = continuation_prefix.to_string();
                row_width = visible_len(continuation_prefix);
                replay_active_ansi(&mut row, &active);
                row_has_payload = false;
                last_non_space_len = row.len();
                in_whitespace = false;
                index = point.resume_index;
                continue;
            }

            if row_has_payload {
                finish_ansi_row(&mut row, &active);
                rows.push(row);
                row = continuation_prefix.to_string();
                row_width = visible_len(continuation_prefix);
                replay_active_ansi(&mut row, &active);
            } else if row_width > 0 && cluster_width <= width {
                row.clear();
                row_width = 0;
                replay_active_ansi(&mut row, &active);
            }
        }

        row.push_str(cluster);
        row_width = row_width.saturating_add(cluster_width);
        row_has_payload = true;
        in_whitespace = false;
        last_non_space_len = row.len();
        index = end;
    }

    if row_has_payload || !first_prefix.is_empty() || payload.is_empty() {
        finish_ansi_row(&mut row, &active);
        rows.push(row);
    }
    rows
}

#[derive(Clone, Default)]
struct ActiveAnsi {
    sgr: Vec<String>,
    hyperlink: Option<String>,
}

fn update_active_ansi(sequence: &str, active: &mut ActiveAnsi) {
    if let Some(target) = osc8_link_target(sequence) {
        active.hyperlink = (!target.is_empty()).then(|| target.to_string());
    } else {
        update_active_sgr(sequence, &mut active.sgr);
    }
}

fn update_active_sgr(sequence: &str, active_sgr: &mut Vec<String>) {
    let Some(params) = sequence
        .strip_prefix("\x1b[")
        .and_then(|sequence| sequence.strip_suffix('m'))
    else {
        return;
    };
    let params = if params.is_empty() { "0" } else { params };
    let contains_reset = params
        .split(';')
        .any(|param| param.is_empty() || param == "0");
    if contains_reset {
        active_sgr.clear();
    }
    if params
        .split(';')
        .any(|param| !param.is_empty() && param != "0")
    {
        active_sgr.push(sequence.to_string());
    }
}

fn replay_active_ansi(row: &mut String, active: &ActiveAnsi) {
    if let Some(target) = &active.hyperlink {
        row.push_str(&osc8_open(target));
    }
    for sequence in &active.sgr {
        row.push_str(sequence);
    }
}

fn finish_ansi_row(row: &mut String, active: &ActiveAnsi) {
    if !active.sgr.is_empty() {
        row.push_str("\x1b[0m");
    }
    if active.hyperlink.is_some() {
        row.push_str(OSC8_CLOSE);
    }
}

/// Return the next extended-grapheme-like boundary needed by terminal text.
/// This keeps combining marks, variation selectors, emoji modifiers, flags,
/// keycaps, and zero-width-joiner emoji sequences on the same row.
fn next_grapheme_cluster_end(value: &str, start: usize) -> usize {
    let Some(first) = value[start..].chars().next() else {
        return start;
    };
    let mut end = start + first.len_utf8();
    let mut regional_count = usize::from(is_regional_indicator(first));

    while let Some(next) = value[end..].chars().next() {
        if is_grapheme_extend(next) {
            end += next.len_utf8();
            continue;
        }
        if next == '\u{200d}' {
            end += next.len_utf8();
            if let Some(joined) = value[end..].chars().next() {
                end += joined.len_utf8();
                regional_count = 0;
                continue;
            }
            break;
        }
        if regional_count == 1 && is_regional_indicator(next) {
            end += next.len_utf8();
        }
        break;
    }
    end
}

fn is_regional_indicator(ch: char) -> bool {
    ('\u{1f1e6}'..='\u{1f1ff}').contains(&ch)
}

fn is_grapheme_extend(ch: char) -> bool {
    matches!(
        ch,
        '\u{0300}'..='\u{036f}'
            | '\u{1ab0}'..='\u{1aff}'
            | '\u{1dc0}'..='\u{1dff}'
            | '\u{20d0}'..='\u{20ff}'
            | '\u{fe00}'..='\u{fe0f}'
            | '\u{fe20}'..='\u{fe2f}'
            | '\u{1f3fb}'..='\u{1f3ff}'
            | '\u{e0100}'..='\u{e01ef}'
    ) || ch == '\u{20e3}'
}

fn normalize_markdown_colors(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut chars = input.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '\x1b' && chars.peek() == Some(&'[') {
            chars.next();
            let mut params = String::new();
            let mut final_byte = None;
            for next in chars.by_ref() {
                if next.is_ascii_alphabetic() {
                    final_byte = Some(next);
                    break;
                }
                params.push(next);
            }
            if final_byte == Some('m') {
                out.push_str(&normalize_sgr(&params));
            } else {
                out.push_str("\x1b[");
                out.push_str(&params);
                if let Some(final_byte) = final_byte {
                    out.push(final_byte);
                }
            }
            continue;
        }
        out.push(ch);
    }

    out
}

fn normalize_sgr(params: &str) -> String {
    let parts = if params.is_empty() {
        vec!["0"]
    } else {
        params.split(';').collect::<Vec<_>>()
    };
    let mut codes = Vec::new();
    let mut i = 0;

    while i < parts.len() {
        let Some(code) = parts[i].parse::<u16>().ok() else {
            i += 1;
            continue;
        };
        match code {
            38 if i + 1 < parts.len() && parts[i + 1] == "2" && i + 4 < parts.len() => {
                let rgb = (
                    parts[i + 2].parse::<u8>().ok(),
                    parts[i + 3].parse::<u8>().ok(),
                    parts[i + 4].parse::<u8>().ok(),
                );
                if let (Some(r), Some(g), Some(b)) = rgb {
                    codes.push(design_fg_for_rgb(r, g, b).fg_ansi());
                }
                i += 5;
            }
            48 if i + 1 < parts.len() && parts[i + 1] == "2" && i + 4 < parts.len() => {
                codes.push(SURFACE_SOFT.bg_ansi());
                i += 5;
            }
            38 if i + 1 < parts.len() && parts[i + 1] == "5" && i + 2 < parts.len() => {
                codes.push(TN_FG.fg_ansi());
                i += 3;
            }
            48 if i + 1 < parts.len() && parts[i + 1] == "5" && i + 2 < parts.len() => {
                codes.push(SURFACE_SOFT.bg_ansi());
                i += 3;
            }
            30..=37 | 90..=97 => {
                codes.push(design_fg_for_ansi(code).fg_ansi());
                i += 1;
            }
            40..=47 | 100..=107 => {
                codes.push(SURFACE_SOFT.bg_ansi());
                i += 1;
            }
            // Preserve both style enables and their selective resets. Dropping
            // 22/23/24/... makes a nested Markdown span leak bold, italic,
            // underline, reverse, strike, or color into the following text.
            0 | 1 | 2 | 3 | 4 | 7 | 9 | 22 | 23 | 24 | 27 | 29 | 39 | 49 => {
                codes.push(code.to_string());
                i += 1;
            }
            _ => i += 1,
        }
    }

    if codes.is_empty() {
        String::new()
    } else {
        format!("\x1b[{}m", codes.join(";"))
    }
}

fn design_fg_for_rgb(r: u8, g: u8, b: u8) -> Color {
    match (r, g, b) {
        // Shared Markdown links use the quieter Codex-like cyan role.
        (110, 198, 217) => TN_CYAN,
        // Upstream h1/list blue and h3 cyan become the single active accent.
        (122, 162, 247) | (125, 207, 255) => ACCENT,
        // Upstream table borders / low-emphasis syntax are muted structure.
        (86, 95, 137) | (128, 128, 128) => TN_GRAY,
        // Syntax themes intentionally use a wider palette. Preserve unknown
        // foregrounds instead of flattening every token to the body color.
        _ => Color::Rgb(r, g, b),
    }
}

fn design_fg_for_ansi(code: u16) -> Color {
    match code {
        34 | 36 | 94 | 96 => TN_CYAN,
        32 | 92 => ACCENT,
        30 | 90 => TN_GRAY,
        _ => TN_FG,
    }
}

#[cfg(test)]
#[path = "streaming_markdown_baseline.rs"]
mod streaming_markdown_baseline;

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_bounded(rendered: &str, width: usize) {
        for line in rendered.lines() {
            assert!(
                visible_len(line) <= width,
                "line over width {width}: {:?}",
                strip_ansi(line)
            );
        }
    }

    fn assert_ansi_self_contained(rendered: &str) {
        for line in rendered.lines() {
            let mut sgr = TestSgrState::default();
            let mut active_hyperlink = false;
            let mut index = 0usize;
            while index < line.len() {
                if let Some(end) = ansi_escape_sequence_end(line, index) {
                    let sequence = &line[index..end];
                    if let Some(params) = sequence
                        .strip_prefix("\x1b[")
                        .and_then(|sequence| sequence.strip_suffix('m'))
                    {
                        sgr.apply(params);
                    }
                    if let Some(target) = osc8_link_target(sequence) {
                        active_hyperlink = !target.is_empty();
                    }
                    index = end;
                } else {
                    let ch = line[index..].chars().next().unwrap_or_default();
                    index += ch.len_utf8();
                }
            }
            assert!(sgr.is_default(), "styled line leaks ANSI state: {line:?}");
            assert!(!active_hyperlink, "hyperlink leaks across row: {line:?}");
        }
    }

    #[derive(Default)]
    struct TestSgrState {
        bold_or_dim: bool,
        italic: bool,
        underline: bool,
        reverse: bool,
        strikethrough: bool,
        foreground: bool,
        background: bool,
    }

    impl TestSgrState {
        fn apply(&mut self, params: &str) {
            let codes = if params.is_empty() {
                vec![0]
            } else {
                params
                    .split(';')
                    .filter_map(|part| part.parse::<u16>().ok())
                    .collect::<Vec<_>>()
            };
            let mut index = 0usize;
            while index < codes.len() {
                match codes[index] {
                    0 => *self = Self::default(),
                    1 | 2 => self.bold_or_dim = true,
                    3 => self.italic = true,
                    4 => self.underline = true,
                    7 => self.reverse = true,
                    9 => self.strikethrough = true,
                    22 => self.bold_or_dim = false,
                    23 => self.italic = false,
                    24 => self.underline = false,
                    27 => self.reverse = false,
                    29 => self.strikethrough = false,
                    30..=37 | 90..=97 => self.foreground = true,
                    39 => self.foreground = false,
                    40..=47 | 100..=107 => self.background = true,
                    49 => self.background = false,
                    38 | 48 => {
                        if codes[index] == 38 {
                            self.foreground = true;
                        } else {
                            self.background = true;
                        }
                        index += match codes.get(index + 1) {
                            Some(5) => 2,
                            Some(2) => 4,
                            _ => 0,
                        };
                    }
                    _ => {}
                }
                index += 1;
            }
        }

        fn is_default(&self) -> bool {
            !self.bold_or_dim
                && !self.italic
                && !self.underline
                && !self.reverse
                && !self.strikethrough
                && !self.foreground
                && !self.background
        }
    }

    fn osc8_targets(rendered: &str) -> Vec<String> {
        let mut targets = Vec::new();
        let mut index = 0usize;
        while index < rendered.len() {
            if let Some(end) = ansi_escape_sequence_end(rendered, index) {
                if let Some(target) = osc8_link_target(&rendered[index..end]) {
                    if !target.is_empty() {
                        targets.push(target.to_string());
                    }
                }
                index = end;
            } else {
                let ch = rendered[index..].chars().next().unwrap_or_default();
                index += ch.len_utf8();
            }
        }
        targets
    }

    fn flatten_visible(rendered: &str) -> String {
        strip_ansi(rendered)
            .chars()
            .filter(|ch| {
                !ch.is_whitespace()
                    && !matches!(
                        ch,
                        '─' | '│'
                            | '┌'
                            | '┐'
                            | '└'
                            | '┘'
                            | '├'
                            | '┤'
                            | '┬'
                            | '┴'
                            | '┼'
                            | '╭'
                            | '╮'
                            | '╰'
                            | '╯'
                    )
            })
            .collect()
    }

    #[test]
    fn markdown_color_normalization_preserves_selective_sgr_resets() {
        let normalized =
            normalize_markdown_colors("\x1b[1;3;4;7;9;31;42mstyled\x1b[22;23;24;27;29;39;49mplain");

        assert!(
            normalized.contains("\x1b[22;23;24;27;29;39;49m"),
            "{normalized:?}"
        );
        assert_eq!(strip_ansi(&normalized), "styledplain");
    }

    #[test]
    fn code_blocks_have_no_chrome_and_keep_source_lines_unwrapped() {
        let source = "let value = \"a very very very long code line that should not wrap\";";
        let rendered = Markdown::new()
            .with_width(36)
            .render(&format!("```rust\n{source}\n```"));
        let plain = strip_ansi(&rendered);

        assert_eq!(plain.lines().collect::<Vec<_>>(), vec![source]);
        assert!(!plain.contains(['┌', '│', '└']));
        assert!(visible_len(plain.lines().next().unwrap()) > 36);
    }

    #[test]
    fn complete_json_messages_are_pretty_printed() {
        let rendered = Markdown::new()
            .with_width(48)
            .render(r#"{"name":"a3s","nested":{"enabled":true,"items":[1,2]}}"#);
        let plain = strip_ansi(&rendered);

        assert_eq!(
            plain.lines().collect::<Vec<_>>(),
            vec![
                "{",
                "  \"name\": \"a3s\",",
                "  \"nested\": {",
                "    \"enabled\": true,",
                "    \"items\": [",
                "      1,",
                "      2",
                "    ]",
                "  }",
                "}"
            ]
        );
    }

    #[test]
    fn complete_json_fences_are_pretty_printed_but_partial_streams_are_unchanged() {
        let complete = "Result:\n\n```json\n{\"ok\":true,\"count\":2}\n```\n";
        let rendered = strip_ansi(&Markdown::new().with_width(48).render(complete));
        assert!(rendered.contains("{\n"), "{rendered}");
        assert!(rendered.contains("  \"ok\": true"), "{rendered}");
        assert!(rendered.contains("  \"count\": 2"), "{rendered}");
        assert!(rendered.ends_with('}'), "{rendered}");

        let partial = "```json\n{\"ok\":";
        assert!(matches!(
            pretty_print_complete_json(partial),
            Cow::Borrowed(value) if value == partial
        ));
    }

    #[test]
    fn pipe_tables_wrap_without_compacting_cell_content() {
        let rendered = Markdown::new().with_width(18).render(
            "| Tool | Very long description column |\n\
             | --- | --- |\n\
             | bash | a very long explanation that should be compacted into the viewport |\n",
        );
        let plain = strip_ansi(&rendered);
        let flattened = flatten_visible(&rendered);

        assert!(plain.contains("Tool"));
        assert!(plain.contains("bash"));
        assert!(
            flattened.contains("averylongexplanationthatshouldbecompactedintotheviewport"),
            "{plain}"
        );
        assert_bounded(&rendered, 18);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn table_alignment_is_preserved_by_the_shared_ast_renderer() {
        let rendered = Markdown::new()
            .with_width(80)
            .render("| Left | Mid | Right |\n| :--- | :---: | ---: |\n| x | x | x |");
        let plain = strip_ansi(&rendered);
        let body = plain
            .lines()
            .find(|line| line.matches('x').count() == 3)
            .expect("aligned table body");
        let positions = body
            .match_indices('x')
            .map(|(index, _)| visible_len(&body[..index]))
            .collect::<Vec<_>>();

        assert_eq!(positions, vec![1, 10, 20]);
    }

    #[test]
    fn table_cells_render_rich_nested_inline_content_instead_of_raw_markdown() {
        let rendered = Markdown::new()
            .with_width(120)
            .render("| Rich |\n| --- |\n| **[Docs](https://example.com) and `value`** |");
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("Docs and value"), "{plain}");
        assert_eq!(osc8_targets(&rendered), vec!["https://example.com"]);
        assert!(rendered.contains("\x1b[1m"), "{rendered:?}");
        assert!(rendered.contains(&SURFACE_SOFT.bg_ansi()), "{rendered:?}");
        assert!(!plain.contains("**"), "{plain}");
        assert!(!plain.contains("[Docs]"), "{plain}");
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn reference_links_and_list_context_survive_across_a_table() {
        let rendered = Markdown::new().with_width(80).render(
            "[Before][ref]\n\n\
             | Link |\n\
             | --- |\n\
             | [Inside][ref] |\n\n\
             - [After][ref]\n\n\
             [ref]: https://example.com/reference",
        );
        let plain = strip_ansi(&rendered);

        assert_eq!(
            osc8_targets(&rendered),
            vec![
                "https://example.com/reference",
                "https://example.com/reference",
                "https://example.com/reference"
            ]
        );
        assert!(!plain.contains("[ref]"), "{plain}");
        assert!(
            plain.lines().any(|line| line.starts_with("- After")),
            "{plain}"
        );
    }

    #[test]
    fn table_inside_list_keeps_the_item_ancestry_before_and_after_it() {
        let rendered = Markdown::new().with_width(48).render(
            "- Before table\n\n  | Key | Value |\n  | --- | --- |\n  | one | two |\n\n  After table",
        );
        let plain = strip_ansi(&rendered);
        let before = plain.find("- Before table").expect("list label");
        let cell = plain.find("one").expect("table cell");
        let after = plain.find("  After table").expect("list continuation");

        assert!(before < cell && cell < after, "{plain}");
        assert!(
            plain.lines().any(|line| line.starts_with("   ")
                && line.contains("Key")
                && line.contains("Value")),
            "{plain}"
        );
    }

    #[test]
    fn indented_code_with_pipes_never_enters_table_layout() {
        let rendered = Markdown::new()
            .with_width(40)
            .render("    | A | B |\n    | --- | --- |\n    | one | two |");
        let plain = strip_ansi(&rendered);

        assert!(plain.lines().any(|line| line == "    | A | B |"), "{plain}");
        assert!(!plain.contains(['┌', '┼', '└']), "{plain}");
    }

    #[test]
    fn narrow_header_only_table_keeps_headers_without_source_delimiters() {
        let rendered = Markdown::new()
            .with_width(12)
            .render("| First heading | Second heading |\n| :--- | ---: |");
        let plain = strip_ansi(&rendered);
        let flattened = flatten_visible(&rendered);

        assert!(flattened.contains("Firstheading"), "{plain}");
        assert!(flattened.contains("Secondheading"), "{plain}");
        assert!(!plain.contains('|'), "{plain}");
        assert!(!plain.contains(":---"), "{plain}");
        assert!(!plain.contains("---:"), "{plain}");
        assert_bounded(&rendered, 12);
    }

    #[test]
    fn moderately_narrow_table_stays_columnar_with_wrapped_cells() {
        let rendered = Markdown::new().with_width(28).render(
            "| Tool | Description |\n| --- | --- |\n| bash | a detailed explanation with several words |",
        );
        let plain = strip_ansi(&rendered);
        let flattened = flatten_visible(&rendered);

        assert!(plain.contains('━'), "{plain}");
        assert!(
            !plain.lines().any(|line| line.starts_with("Tool:")),
            "{plain}"
        );
        assert!(
            flattened.contains("adetailedexplanationwithseveralwords"),
            "{plain}"
        );
        assert_bounded(&rendered, 28);
    }

    #[test]
    fn table_card_edges_remain_aligned_after_cli_width_bounding() {
        let rendered = Markdown::new()
            .with_width(80)
            .render("| Key | Value |\n| --- | --- |\n| one | two |");
        let widths = rendered.lines().map(visible_len).collect::<Vec<_>>();

        assert!(!widths.is_empty());
        assert!(widths.iter().all(|width| *width == widths[0]), "{rendered}");
        assert!(widths[0] < 80, "a compact table was stretched: {rendered}");
    }

    #[test]
    fn extremely_narrow_many_column_table_uses_records() {
        let rendered = Markdown::new()
            .with_width(18)
            .render("| A | B | C | D |\n| --- | --- | --- | --- |\n| one | two | three | four |");
        let plain = strip_ansi(&rendered);

        assert!(!plain.contains('━'), "{plain}");
        for expected in ["A: one", "B: two", "C: three", "D: four"] {
            assert!(plain.contains(expected), "missing {expected:?} in {plain}");
        }
        assert_bounded(&rendered, 18);
    }

    #[test]
    fn narrow_code_blocks_preserve_the_complete_code_line() {
        let source = "let extraordinarily_long_identifier = \"完整👩‍💻value\";";
        let rendered = Markdown::new()
            .with_width(12)
            .render(&format!("```rust\n{source}\n```"));
        let plain = strip_ansi(&rendered);

        assert_eq!(plain.lines().collect::<Vec<_>>(), vec![source]);
        assert!(visible_len(plain.lines().next().unwrap()) > 12);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn narrow_headings_preserve_the_complete_title() {
        let title = "完整标题 with a deliberately extraordinarily long suffix";
        let rendered = Markdown::new().with_width(13).render(&format!("# {title}"));
        let flattened = flatten_visible(&rendered);

        assert!(
            flattened.contains(
                &title
                    .chars()
                    .filter(|ch| !ch.is_whitespace())
                    .collect::<String>()
            ),
            "{}",
            strip_ansi(&rendered)
        );
        assert!(strip_ansi(&rendered).starts_with("# "));
        assert_bounded(&rendered, 13);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn section_headings_keep_codex_vertical_spacing_after_cli_bounding() {
        let rendered = Markdown::new()
            .with_width(32)
            .render("Previous section body.\n\n## Next section\n\nNext section body.");
        let plain = strip_ansi(&rendered);

        assert_eq!(
            plain.lines().collect::<Vec<_>>(),
            vec![
                "Previous section body.",
                "",
                "## Next section",
                "",
                "Next section body."
            ]
        );
        assert_bounded(&rendered, 32);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn narrow_bare_links_preserve_the_complete_url() {
        let url = "https://example.com/a/very/long/path?query=完整&mode=codex";
        let rendered = Markdown::new().with_width(14).render(url);
        let flattened = flatten_visible(&rendered);

        assert!(flattened.contains(url), "{}", strip_ansi(&rendered));
        assert!(osc8_targets(&rendered).iter().all(|target| target == url));
        assert_bounded(&rendered, 14);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn codex_style_links_are_cyan_clickable_and_do_not_swallow_parentheses() {
        let url = "https://github.com/A3S-Lab/Code/actions/runs/29246228334";
        let source = format!("v5.2.1 Release run 已启动: {url}\n({url})。我会继续跟踪。");
        let rendered = Markdown::new().with_width(52).render(&source);
        let plain = strip_ansi(&rendered);

        assert_eq!(
            plain
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>(),
            source
                .chars()
                .filter(|ch| !ch.is_whitespace())
                .collect::<String>()
        );
        assert_eq!(
            flatten_visible(&rendered).matches(url).count(),
            2,
            "{plain}"
        );
        assert!(rendered.contains(&TN_CYAN.fg_ansi()), "{rendered:?}");
        assert!(rendered.contains("\x1b[4;"), "{rendered:?}");
        assert!(osc8_targets(&rendered).iter().all(|target| target == url));
        assert!(!osc8_targets(&rendered)
            .iter()
            .any(|target| target.ends_with(')')));
        assert_bounded(&rendered, 52);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn markdown_link_renders_only_its_clickable_label() {
        let url = "https://example.com/releases/latest";
        let rendered = Markdown::new()
            .with_width(48)
            .render(&format!("Read [release notes]({url})."));

        assert_eq!(strip_ansi(&rendered), "Read release notes.");
        assert_eq!(osc8_targets(&rendered), vec![url]);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn narrow_cjk_and_emoji_wrap_at_grapheme_boundaries() {
        let source = "中文内容👩‍💻继续🇨🇳以及e\u{301}组合字符";
        let rendered = Markdown::new().with_width(8).render(source);
        let plain = strip_ansi(&rendered);

        assert_eq!(flatten_visible(&rendered), source);
        assert!(plain.lines().any(|line| line.contains("👩‍💻")), "{plain}");
        assert!(plain.lines().any(|line| line.contains("🇨🇳")), "{plain}");
        assert!(
            plain.lines().any(|line| line.contains("e\u{301}")),
            "{plain}"
        );
        assert_bounded(&rendered, 8);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn narrow_tables_preserve_every_cell_value() {
        let rendered = Markdown::new().with_width(12).render(
            "| key | value |\n\
             | --- | --- |\n\
             | alpha-super-long-token | 支持👩‍💻开发 |\n\
             | second | https://example.com/complete/path |\n",
        );
        let flattened = flatten_visible(&rendered);

        for expected in [
            "alpha-super-long-token",
            "支持👩‍💻开发",
            "https://example.com/complete/path",
        ] {
            assert!(
                flattened.contains(expected),
                "missing {expected:?} in {}",
                strip_ansi(&rendered)
            );
        }
        assert_bounded(&rendered, 12);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn multiple_prose_and_table_blocks_preserve_source_order() {
        let rendered = Markdown::new().with_width(24).render(
            "Before first.\n\n\
             | Name | Value |\n\
             | --- | --- |\n\
             | first | alpha-value |\n\n\
             Between tables.\n\n\
             | Name | Value |\n\
             | --- | --- |\n\
             | second | beta-value |\n\n\
             After second.\n",
        );
        let plain = strip_ansi(&rendered);
        let markers = [
            "Before first.",
            "alpha-value",
            "Between tables.",
            "beta-value",
            "After second.",
        ];
        let positions = markers.map(|marker| {
            plain
                .find(marker)
                .unwrap_or_else(|| panic!("missing {marker:?} in {plain}"))
        });

        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{plain}"
        );
        assert_eq!(
            plain.lines().filter(|line| line.contains('━')).count(),
            2,
            "{plain}"
        );
        assert_bounded(&rendered, 24);
    }

    #[test]
    fn narrow_blockquoted_tables_preserve_quote_gutter_and_cell_values() {
        let rendered = Markdown::new().with_width(14).render(
            "> | Key | Value |\n\
             > | --- | --- |\n\
             > | first | extraordinarily-long-value |\n\
             > | second | 支持👩‍💻开发 |\n",
        );
        let plain = strip_ansi(&rendered);
        let flattened = flatten_visible(&rendered);

        for expected in [
            "first",
            "extraordinarily-long-value",
            "second",
            "支持👩‍💻开发",
        ] {
            assert!(
                flattened.contains(expected),
                "missing {expected:?} in {plain}"
            );
        }
        assert!(
            plain
                .lines()
                .filter(|line| !line.is_empty())
                .all(|line| line == "│" || line.starts_with("│ ")),
            "blockquote gutter disappeared after wrapping:\n{plain}"
        );
        assert_bounded(&rendered, 14);
        assert_ansi_self_contained(&rendered);
    }

    #[test]
    fn markdown_fenced_tables_render_as_tables_but_other_fences_remain_code() {
        let markdown_fenced = Markdown::new()
            .with_width(40)
            .render("```markdown\n| A | B |\n| --- | --- |\n| one | two |\n```\n");
        let markdown_plain = strip_ansi(&markdown_fenced);
        assert!(markdown_plain.contains('━'), "{markdown_plain}");
        assert!(!markdown_plain.contains("```"), "{markdown_plain}");
        assert!(
            !markdown_plain
                .lines()
                .any(|line| line.trim() == "| A | B |"),
            "{markdown_plain}"
        );

        let shell_fenced = Markdown::new()
            .with_width(40)
            .render("```sh\n| A | B |\n| --- | --- |\n| one | two |\n```\n");
        let shell_plain = strip_ansi(&shell_fenced);
        assert!(
            shell_plain.lines().any(|line| line == "| A | B |"),
            "{shell_plain}"
        );
        assert!(!shell_plain.contains('╭'), "{shell_plain}");
        assert_bounded(&markdown_fenced, 40);
        assert_bounded(&shell_fenced, 40);
    }

    #[test]
    fn blockquotes_wrap_instead_of_overflowing() {
        let rendered = Markdown::new().with_width(42).render(
            "> This is a deliberately long quote that needs to wrap inside the terminal transcript.",
        );
        let plain = strip_ansi(&rendered);

        assert!(plain.lines().count() > 1, "{plain}");
        assert!(plain.lines().all(|line| line.starts_with("│ ")));
        assert_bounded(&rendered, 42);
    }

    #[test]
    fn blockquotes_preserve_nested_lists_quotes_and_code() {
        let code = "let extraordinarily_long_identifier = 123456789;";
        let rendered = Markdown::new().with_width(24).render(&format!(
            "> - alpha beta gamma delta epsilon\n>\n> > nested quote\n>\n> ```rust\n> {code}\n> ```"
        ));
        let plain = strip_ansi(&rendered);
        let lines = plain.lines().collect::<Vec<_>>();

        assert!(lines.iter().any(|line| line.starts_with("│ - ")), "{plain}");
        assert!(lines.iter().any(|line| line.starts_with("│   ")), "{plain}");
        assert!(lines.contains(&"│ │ nested quote"), "{plain}");
        assert!(
            lines.iter().any(|line| *line == format!("│ {code}")),
            "{plain}"
        );
        assert!(visible_len(lines.iter().find(|line| line.contains(code)).unwrap()) > 24);
    }

    #[test]
    fn blockquote_inside_list_retains_compound_prefix_after_reflow() {
        let rendered = Markdown::new().with_width(24).render(
            "- list item\n  > block quote inside list with enough words to wrap repeatedly",
        );
        let plain = strip_ansi(&rendered);
        let quoted = plain
            .lines()
            .filter(|line| {
                line.contains("block") || line.contains("quote") || line.contains("wrap")
            })
            .collect::<Vec<_>>();

        assert!(quoted.len() > 1, "{plain}");
        assert!(
            quoted.iter().all(|line| line.starts_with("  │ ")),
            "{plain}"
        );
        assert_bounded(&rendered, 24);
    }

    #[test]
    fn blockquote_inside_ordered_list_retains_marker_continuation_indent() {
        let rendered = Markdown::new()
            .with_width(24)
            .render("1. item with quote\n   > quoted text with enough words to wrap repeatedly");
        let plain = strip_ansi(&rendered);
        let quoted = plain
            .lines()
            .filter(|line| !line.starts_with("1."))
            .filter(|line| !line.is_empty())
            .collect::<Vec<_>>();

        assert!(quoted.len() > 1, "{plain}");
        assert!(
            quoted.iter().all(|line| line.starts_with("   │ ")),
            "{plain}"
        );
        assert_bounded(&rendered, 24);
    }

    #[test]
    fn narrow_prose_wraps_at_word_boundaries() {
        let source = "alpha beta gamma delta epsilon zeta eta theta";
        let rendered = Markdown::new().with_width(14).render(source);
        assert_bounded(&rendered, 14);
        let plain = strip_ansi(&rendered);
        let source_words = source.split_whitespace().collect::<Vec<_>>();
        let rendered_words = plain.split_whitespace().collect::<Vec<_>>();

        assert_eq!(rendered_words, source_words, "{plain}");
        assert!(plain.lines().count() > 1, "{plain}");
    }

    #[test]
    fn narrow_lists_preserve_hanging_indent_after_wrapping() {
        let rendered = Markdown::new()
            .with_width(18)
            .render("- alpha beta gamma delta epsilon zeta\n\n1. one two three four five six\n");
        assert_bounded(&rendered, 18);
        let plain = strip_ansi(&rendered);
        let lines = plain.lines().collect::<Vec<_>>();
        let bullet = lines
            .iter()
            .position(|line| line.starts_with("- "))
            .expect("unordered list bullet");
        let ordered = lines
            .iter()
            .position(|line| line.trim_start().starts_with("1."))
            .expect("ordered list marker");

        assert!(
            lines[bullet + 1].starts_with("  "),
            "unordered continuation lost its hanging indent: {plain}"
        );
        assert!(
            lines[ordered + 1].starts_with("   "),
            "ordered continuation lost its hanging indent: {plain}"
        );
    }

    #[test]
    fn streaming_unterminated_fence_never_commits_temporary_chrome() {
        let source = "let extraordinarily_long_identifier = 123456789;";
        let mut streaming = StreamingMarkdown::new(18);
        streaming.push("```rust\n");
        streaming.push(&format!("{source}\n"));

        let before_close = strip_ansi(&streaming.stable_view());
        assert_eq!(before_close.lines().collect::<Vec<_>>(), vec![source]);
        assert!(!before_close.contains(['┌', '│', '└', '─']));
        assert!(visible_len(before_close.lines().next().unwrap()) > 18);

        streaming.push("```\n");
        assert_eq!(strip_ansi(&streaming.stable_view()), before_close);
        assert!(streaming.tail_view().is_empty());
    }

    #[test]
    fn streaming_blockquote_keeps_nested_structure_and_code_rows_stable() {
        let code = "let extraordinarily_long_identifier = 123456789;";
        let mut streaming = StreamingMarkdown::new(24);
        streaming.push("> - alpha beta\n");
        let first_row = strip_ansi(&streaming.stable_view())
            .lines()
            .next()
            .expect("first quoted list row")
            .to_string();

        streaming.push(">   gamma delta\n");
        assert_eq!(
            strip_ansi(&streaming.stable_view()).lines().next(),
            Some(first_row.as_str())
        );
        streaming.push(">\n> > nested quote\n>\n> ```rust\n");
        streaming.push(&format!("> {code}\n"));

        let before_close = strip_ansi(&streaming.stable_view());
        assert!(before_close.lines().any(|line| line == "│ │ nested quote"));
        assert!(before_close.lines().any(|line| line == format!("│ {code}")));
        assert!(!before_close.contains(['┌', '└']));

        streaming.push("> ```\n");
        assert_eq!(strip_ansi(&streaming.stable_view()), before_close);
        assert!(streaming.tail_view().is_empty());
    }

    #[test]
    fn streaming_markdown_uses_same_bounded_renderer() {
        let mut streaming = StreamingMarkdown::new(34);
        streaming.push("| A | B |\n| --- | --- |\n| alpha | beta gamma delta epsilon zeta |\n");

        assert!(streaming.raw_content().contains("alpha"));
        assert_bounded(&streaming.view(), 34);
    }

    #[test]
    fn streaming_markdown_honors_narrow_width_budget() {
        let mut streaming = StreamingMarkdown::new(11);
        streaming.push("abcdefghijklmnopqrstuvwxyz");

        assert!(streaming.view().is_empty());
        assert_bounded(&streaming.final_view(), 11);
        assert_eq!(
            strip_ansi(&streaming.final_view())
                .lines()
                .collect::<Vec<_>>(),
            vec!["abcdefghijk", "lmnopqrstuv", "wxyz"]
        );
    }

    #[test]
    fn streaming_markdown_commits_only_at_newline_boundaries() {
        let mut streaming = StreamingMarkdown::new(40);

        assert!(!streaming.push("**partial"));
        assert!(streaming.view().is_empty());
        assert_eq!(streaming.raw_content(), "**partial");

        assert!(streaming.push(" heading**\n"));
        let committed = strip_ansi(&streaming.view());
        assert!(committed.contains("partial heading"), "{committed}");
    }

    #[test]
    fn streaming_markdown_releases_stable_rows_one_commit_tick_at_a_time() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("first\n\nsecond\n\nthird\n");
        let queued = streaming.queued_lines();

        assert!(queued >= 2, "expected multiple rendered rows, got {queued}");
        assert!(streaming.visible_stable_view().is_empty());
        assert!(streaming.commit_tick(Instant::now()));
        assert_eq!(streaming.queued_lines(), queued - 1);
        let visible = strip_ansi(&streaming.visible_stable_view());
        assert!(visible.contains("first"), "{visible}");
        assert!(!visible.contains("third"), "{visible}");
    }

    #[test]
    fn streaming_soft_break_queues_new_row_without_mutating_emitted_row() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("alpha\n");
        assert!(streaming.commit_tick(Instant::now()));
        let emitted = streaming.visible_stable_view();
        assert_eq!(strip_ansi(&emitted), "alpha");

        streaming.push("beta\n");

        assert_eq!(
            streaming.visible_stable_view(),
            emitted,
            "a later soft-break row must not rewrite already-emitted content"
        );
        assert_eq!(streaming.queued_lines(), 1);
        assert!(streaming.commit_tick(Instant::now()));
        assert_eq!(strip_ansi(&streaming.visible_stable_view()), "alpha\nbeta");
    }

    #[test]
    fn streaming_hard_break_preserves_visual_line_boundary() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("alpha  \n");
        assert!(streaming.commit_tick(Instant::now()));
        let emitted = streaming.visible_stable_view();

        streaming.push("beta\n");

        assert_eq!(streaming.visible_stable_view(), emitted);
        assert_eq!(streaming.queued_lines(), 1);
        assert!(streaming.commit_tick(Instant::now()));
        assert_eq!(strip_ansi(&streaming.visible_stable_view()), "alpha\nbeta");
    }

    #[test]
    fn streaming_markdown_catches_up_when_the_stable_queue_is_deep() {
        let mut streaming = StreamingMarkdown::new(40);
        let source = (0..12)
            .map(|index| format!("line {index}\n\n"))
            .collect::<String>();
        streaming.push(&source);

        assert!(
            streaming.queued_lines() >= ENTER_CATCH_UP_QUEUE_DEPTH,
            "queue depth was {}",
            streaming.queued_lines()
        );
        assert!(streaming.commit_tick(Instant::now()));
        assert_eq!(streaming.queued_lines(), 0);
        let visible = strip_ansi(&streaming.visible_stable_view());
        assert!(visible.contains("line 0"), "{visible}");
        assert!(visible.contains("line 11"), "{visible}");
    }

    #[test]
    fn enqueue_time_tick_drains_only_when_policy_enters_catch_up() {
        let mut smooth = StreamingMarkdown::new(40);
        smooth.push("first\nsecond\n");
        let smooth_depth = smooth.queued_lines();
        assert!(!smooth.commit_catch_up_tick(Instant::now()));
        assert_eq!(smooth.queued_lines(), smooth_depth);

        let mut burst = StreamingMarkdown::new(40);
        let source = (0..12)
            .map(|index| format!("line {index}\n"))
            .collect::<String>();
        burst.push(&source);
        assert!(burst.queued_lines() >= ENTER_CATCH_UP_QUEUE_DEPTH);
        assert!(burst.commit_catch_up_tick(Instant::now()));
        assert_eq!(burst.queued_lines(), 0);
    }

    #[test]
    fn streaming_markdown_catches_up_when_the_oldest_row_is_stale() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("first\n\nsecond\n\nthird\n");
        assert!(streaming.queued_lines() > 1);

        assert!(streaming
            .commit_tick(Instant::now() + ENTER_CATCH_UP_OLDEST_AGE + Duration::from_millis(1)));
        assert_eq!(streaming.queued_lines(), 0);
    }

    #[test]
    fn streaming_markdown_keeps_table_tail_live_while_prefix_is_queued() {
        let mut streaming = StreamingMarkdown::new(80);
        streaming.push(
            "Intro before the table.\n\n| Name | Role |\n| --- | --- |\n| Ada | Engineer |\n",
        );

        assert!(streaming.visible_stable_view().is_empty());
        assert!(streaming.queued_lines() > 0);
        let tail = strip_ansi(&streaming.tail_view());
        assert!(tail.contains("Name"), "{tail}");
        assert!(tail.contains("Ada"), "{tail}");

        assert!(streaming.commit_tick(Instant::now()));
        assert!(
            strip_ansi(&streaming.visible_stable_view()).contains("Intro before the table."),
            "{}",
            strip_ansi(&streaming.visible_stable_view())
        );
    }

    #[test]
    fn streaming_markdown_resize_does_not_replay_fully_emitted_source() {
        let mut streaming = StreamingMarkdown::new(72);
        streaming.push("A deliberately long paragraph that will wrap after resize.\n");
        while streaming.queued_lines() > 0 {
            assert!(streaming.commit_tick(Instant::now()));
        }
        assert!(!streaming.visible_stable_view().is_empty());

        streaming.set_width(18);

        assert_eq!(streaming.queued_lines(), 0);
        let visible = strip_ansi(&streaming.visible_stable_view());
        assert!(visible.contains("deliberately"), "{visible}");
        assert!(visible.lines().count() > 1, "{visible}");
        assert!(!streaming.commit_tick(Instant::now()));
    }

    #[test]
    fn resize_with_live_table_tail_keeps_reflowed_prefix_before_tail() {
        let mut streaming = StreamingMarkdown::new(72);
        streaming.push(
            "A deliberately long introduction that occupies one wide row before the table.\n\n| Name | Role |\n| --- | --- |\n| Ada | Engineer |\n",
        );
        while streaming.queued_lines() > 0 {
            assert!(streaming.commit_tick(Instant::now()));
        }
        assert!(!streaming.tail_view().is_empty());

        streaming.set_width(18);

        assert_eq!(streaming.queued_lines(), 0);
        let stable = strip_ansi(&streaming.visible_stable_view());
        let tail = strip_ansi(&streaming.tail_view());
        assert!(stable.lines().count() > 1, "{stable}");
        let combined = format!("{stable}\n{tail}");
        let intro_end = combined
            .find("table.")
            .expect("complete reflowed introduction");
        let table_start = combined.find("Name").expect("live table tail");
        assert!(intro_end < table_start, "{combined}");
    }

    #[test]
    fn streaming_markdown_does_not_parse_token_only_partial_lines() {
        let mut streaming = StreamingMarkdown::new(40);
        for _ in 0..10_000 {
            assert!(!streaming.push("x"));
        }
        assert_eq!(streaming.rerender_count, 0);
        assert!(streaming.view().is_empty());

        assert!(streaming.push("\n"));
        assert_eq!(streaming.rerender_count, 1);
    }

    #[test]
    fn streaming_markdown_holds_a_pending_table_header_in_the_mutable_tail() {
        let mut streaming = StreamingMarkdown::new(80);
        assert!(streaming.push("Intro before the table.\n\n"));
        assert!(streaming.push("| Name | Role |\n"));

        let stable = strip_ansi(&streaming.stable_view());
        let tail = strip_ansi(&streaming.tail_view());
        assert!(stable.contains("Intro before the table."), "{stable}");
        assert!(!stable.contains("Name"), "{stable}");
        assert!(tail.contains("Name"), "{tail}");
        assert!(tail.contains("Role"), "{tail}");
        assert!(tail.contains('━'), "{tail}");
        assert!(!tail.contains('|'), "{tail}");
        assert_eq!(
            table_holdback_state(streaming.raw_content()),
            TableHoldbackState::PendingHeader {
                header_start: "Intro before the table.\n\n".len(),
            }
        );
    }

    #[test]
    fn pending_pipe_dense_table_is_provisionally_completed_without_overflow() {
        let source = "| | ✏️修改 | src/compact/compaction.rs | | ✏️修改 | src/compact/mod.rs | | ✏️修改 | src/config.rs |\n";
        let mut streaming = StreamingMarkdown::new(44);

        assert!(streaming.push(source));
        let tail = streaming.tail_view();
        let plain = strip_ansi(&tail);

        assert!(streaming.stable_view().is_empty());
        assert!(!plain.contains('|'), "{plain}");
        assert!(plain.contains("✏️修改"), "{plain}");
        assert!(plain.contains("src/compact/compaction.rs"), "{plain}");
        assert!(tail.lines().all(|line| visible_len(line) <= 44), "{tail:?}");
    }

    #[test]
    fn streaming_markdown_keeps_a_confirmed_table_mutable() {
        let mut streaming = StreamingMarkdown::new(80);
        streaming.push("Intro before the table.\n\n");
        streaming.push("| Name | Role |\n");
        streaming.push("| --- | --- |\n");
        streaming.push("| Ada | Engineer |\n");

        let stable = strip_ansi(&streaming.stable_view());
        let tail = strip_ansi(&streaming.tail_view());
        assert!(stable.contains("Intro before the table."), "{stable}");
        assert!(!stable.contains("Ada"), "{stable}");
        assert!(tail.contains("Name"), "{tail}");
        assert!(tail.contains("Ada"), "{tail}");
        assert!(!matches!(
            table_holdback_state(streaming.raw_content()),
            TableHoldbackState::Confirmed { .. }
        ));
    }

    #[test]
    fn streaming_only_holds_the_trailing_table_at_eof() {
        let source = "| First | Value |\n| --- | --- |\n| one | alpha |\n\nBetween.\n\n| Second | Value |\n| --- | --- |\n| two | beta |\n";
        let mut streaming = StreamingMarkdown::new(48);
        for line in source.split_inclusive('\n') {
            streaming.push(line);
        }

        let stable = strip_ansi(&streaming.stable_view());
        let tail = strip_ansi(&streaming.tail_view());
        assert!(stable.contains("First"), "{stable}");
        assert!(stable.contains("alpha"), "{stable}");
        assert!(stable.contains("Between."), "{stable}");
        assert!(!stable.contains("Second"), "{stable}");
        assert!(tail.contains("Second"), "{tail}");
        assert!(tail.contains("beta"), "{tail}");
    }

    #[test]
    fn pending_header_becomes_stable_after_a_blank_line() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("| candidate | header |\n");
        assert!(streaming.stable_view().is_empty());
        assert!(!streaming.tail_view().is_empty());

        streaming.push("\n");
        assert_eq!(
            table_holdback_state(streaming.raw_content()),
            TableHoldbackState::None
        );
        assert!(strip_ansi(&streaming.stable_view()).contains("candidate | header"));
        assert!(streaming.tail_view().is_empty());
    }

    #[test]
    fn wrapped_rows_clone_source_metadata_and_fenced_tables_keep_line_numbers() {
        let prose = Markdown::new()
            .with_width(10)
            .render_document("alpha beta gamma delta");
        assert!(prose.lines.len() > 1);
        assert_eq!(prose.lines.len(), prose.line_metadata.len());
        assert!(prose.line_metadata.iter().all(|metadata| {
            metadata
                .source()
                .is_some_and(|source| source.start() == 1 && source.end() == 1)
        }));

        let fenced = Markdown::new()
            .with_width(40)
            .render_document("```md\n| A | B |\n| --- | --- |\n| one | two |\n```\n");
        assert_eq!(fenced.lines.len(), fenced.line_metadata.len());
        assert!(fenced
            .line_metadata
            .iter()
            .filter(|meta| meta.is_table())
            .all(|metadata| metadata
                .source()
                .is_some_and(|source| source.start() == 2 && source.end() == 4)));
    }

    #[test]
    fn final_reconciliation_resolves_reference_definitions_after_a_table() {
        let mut streaming = StreamingMarkdown::new(48);
        streaming.push("[Before][ref]\n\n| Link |\n| --- |\n| [Inside][ref] |\n\n");
        streaming.push("[ref]: https://example.com/final\n");

        let final_view = streaming.final_view();
        assert_eq!(
            osc8_targets(&final_view),
            vec!["https://example.com/final", "https://example.com/final"]
        );
        assert!(!strip_ansi(&final_view).contains("[ref]"), "{final_view}");
    }

    #[test]
    fn streaming_markdown_preserves_multiple_prose_and_table_blocks() {
        let source = "Before first.\n\n\
                      | Name | Value |\n\
                      | --- | --- |\n\
                      | first | alpha-value |\n\n\
                      Between tables.\n\n\
                      | Name | Value |\n\
                      | --- | --- |\n\
                      | second | beta-value |\n\n\
                      After second.\n";
        let mut streaming = StreamingMarkdown::new(24);
        for line in source.split_inclusive('\n') {
            streaming.push(line);
        }

        let stable = strip_ansi(&streaming.stable_view());
        let tail = strip_ansi(&streaming.tail_view());
        assert!(tail.is_empty(), "{tail}");
        let markers = [
            "Before first.",
            "alpha-value",
            "Between tables.",
            "beta-value",
            "After second.",
        ];
        let positions = markers.map(|marker| {
            stable
                .find(marker)
                .unwrap_or_else(|| panic!("missing {marker:?} in {stable}"))
        });
        assert!(
            positions.windows(2).all(|pair| pair[0] < pair[1]),
            "{stable}"
        );
        assert_eq!(streaming.view(), streaming.final_view());
        assert_bounded(&streaming.view(), 24);
    }

    #[test]
    fn streaming_markdown_distinguishes_markdown_and_other_fenced_pipes() {
        let mut markdown = StreamingMarkdown::new(40);
        for delta in [
            "```md\n",
            "| A | B |\n",
            "| --- | --- |\n",
            "| one | two |\n",
        ] {
            markdown.push(delta);
        }
        let incomplete_tail = strip_ansi(&markdown.tail_view());
        assert!(markdown.stable_view().is_empty());
        assert!(incomplete_tail.contains("A"), "{incomplete_tail}");
        assert!(incomplete_tail.contains("one"), "{incomplete_tail}");
        assert!(incomplete_tail.contains('━'), "{incomplete_tail}");
        assert!(!incomplete_tail.contains("| A | B |"), "{incomplete_tail}");

        markdown.push("```\n");
        let markdown_stable = strip_ansi(&markdown.stable_view());
        assert!(markdown.tail_view().is_empty());
        assert!(markdown_stable.contains('━'), "{markdown_stable}");
        assert!(!markdown_stable
            .lines()
            .any(|line| line.trim() == "| A | B |"));
        assert_eq!(markdown.view(), markdown.final_view());
        assert_eq!(
            table_holdback_state(markdown.raw_content()),
            TableHoldbackState::None
        );

        let mut shell = StreamingMarkdown::new(40);
        for delta in [
            "```sh\n",
            "| A | B |\n",
            "| --- | --- |\n",
            "| one | two |\n",
            "```\n",
        ] {
            shell.push(delta);
        }
        let shell_stable = strip_ansi(&shell.stable_view());
        assert!(shell.tail_view().is_empty());
        assert!(shell_stable.contains("| A | B |"), "{shell_stable}");
        assert!(!shell_stable.contains('━'), "{shell_stable}");
        assert_eq!(shell.view(), shell.final_view());
        assert_eq!(
            table_holdback_state(shell.raw_content()),
            TableHoldbackState::None
        );
    }

    #[test]
    fn streaming_markdown_releases_non_table_pipe_prose() {
        let mut streaming = StreamingMarkdown::new(80);
        streaming.push("status | owner | note\n");
        assert!(streaming.stable_view().is_empty());
        let provisional = streaming.tail_view();
        assert!(strip_ansi(&provisional).contains('━'), "{provisional}");
        assert!(!strip_ansi(&provisional).contains('|'), "{provisional}");
        assert!(flatten_visible(&provisional).contains("statusownernote"));

        streaming.push("This is ordinary prose, not a table delimiter.\n");
        let stable = strip_ansi(&streaming.stable_view());
        assert!(streaming.tail_view().is_empty());
        assert!(stable.contains("status | owner | note"), "{stable}");
        assert!(stable.contains("ordinary prose"), "{stable}");
    }

    #[test]
    fn streaming_markdown_hides_an_unterminated_table_row() {
        let mut streaming = StreamingMarkdown::new(80);
        streaming.push("| Name | Role |\n| --- | --- |\n| Ada | Engineer |\n");
        let before_partial = streaming.view();

        assert!(!streaming.push("| Grace | Scien"));
        assert_eq!(streaming.view(), before_partial);
        assert!(!strip_ansi(&streaming.view()).contains("Grace"));
        assert!(strip_ansi(&streaming.final_view()).contains("Grace"));
        assert!(streaming.raw_content().ends_with("Scien"));
    }

    #[test]
    fn streaming_markdown_is_chunk_boundary_independent() {
        let source = "Intro.\n\n| Key | Value |\n| --- | --- |\n| alpha | beta gamma |\nunfinished";
        let mut whole = StreamingMarkdown::new(42);
        whole.push(source);

        let mut chunked = StreamingMarkdown::new(42);
        for chunk in [
            "In",
            "tro.\n\n| Ke",
            "y | Value |\n| ---",
            " | --- |\n| alpha | beta",
            " gamma |\nunfin",
            "ished",
        ] {
            chunked.push(chunk);
        }

        assert_eq!(chunked.raw_content(), whole.raw_content());
        assert_eq!(chunked.stable_view(), whole.stable_view());
        assert_eq!(chunked.tail_view(), whole.tail_view());
        assert_eq!(chunked.view(), whole.view());
        assert_eq!(chunked.final_view(), whole.final_view());
    }

    #[test]
    fn streaming_markdown_reflows_from_raw_source_after_resize() {
        let source = "A deliberately long paragraph that wraps differently after a resize.\n\n\
                      | Key | Description |\n\
                      | --- | --- |\n\
                      | one | another deliberately long value |\n";
        let mut streaming = StreamingMarkdown::new(72);
        streaming.push(source);
        let raw = streaming.raw_content().to_string();
        let wide = streaming.view();

        streaming.set_width(24);
        let narrow = streaming.view();
        assert_eq!(streaming.raw_content(), raw);
        assert_eq!(
            narrow,
            Markdown::new()
                .with_width(24)
                .render(streaming.raw_content())
        );
        assert!(narrow.lines().count() > wide.lines().count());

        streaming.set_width(72);
        assert_eq!(streaming.view(), wide);
        assert_eq!(streaming.final_view(), wide);
    }

    #[test]
    fn streaming_markdown_final_view_flushes_unterminated_tail() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("complete line\nanswer with `inline code`");

        let live = strip_ansi(&streaming.view());
        assert!(live.contains("complete line"), "{live}");
        assert!(!live.contains("answer with"), "{live}");
        assert_eq!(
            streaming.final_view(),
            Markdown::new()
                .with_width(40)
                .render(streaming.raw_content())
        );
    }

    #[test]
    fn streaming_markdown_clear_resets_commit_boundary() {
        let mut streaming = StreamingMarkdown::new(40);
        streaming.push("first\n");
        assert!(!streaming.view().is_empty());

        streaming.clear();
        assert!(streaming.view().is_empty());
        assert!(streaming.raw_content().is_empty());
        assert!(!streaming.push("second"));
        assert!(streaming.view().is_empty());
    }

    #[test]
    fn markdown_colors_are_normalized_to_design_palette() {
        let rendered = Markdown::new().with_width(72).render(
            "# Heading\n\
             - item with [link](https://example.com) and `code`\n\
             - [x] done\n\
             ```rust\n\
             let value = 1;\n\
             ```",
        );

        assert!(rendered.contains(&ACCENT.fg_ansi()), "{rendered:?}");
        assert!(rendered.contains(&TN_CYAN.fg_ansi()), "{rendered:?}");
        assert!(rendered.contains(&SURFACE_SOFT.bg_ansi()), "{rendered:?}");
        assert!(!rendered.contains("122;162;247"), "{rendered:?}");
        assert!(!rendered.contains("187;154;247"), "{rendered:?}");
        assert!(!rendered.contains("\x1b[33m"), "{rendered:?}");
        assert!(!rendered.contains("\x1b[32m"), "{rendered:?}");
        assert!(!rendered.contains("\x1b[4;34m"), "{rendered:?}");
        assert_bounded(&rendered, 72);
    }

    #[test]
    fn syntax_tokens_keep_their_distinct_rgb_foregrounds() {
        let rendered = Markdown::new().with_width(96).render(
            "```rust\nfn greet() { let answer = format_value(\"hello\", 42); // note\n}\n```",
        );
        let colors = ["fn", "greet", "hello", "42", "//"].map(|token| {
            foreground_rgb_before(&rendered, token)
                .unwrap_or_else(|| panic!("missing foreground for {token:?} in {rendered:?}"))
        });

        for (index, color) in colors.iter().enumerate() {
            assert!(
                !colors[..index].contains(color),
                "token colors must remain distinct: {colors:?}"
            );
        }
    }

    fn foreground_rgb_before(rendered: &str, token: &str) -> Option<(u8, u8, u8)> {
        let token_start = rendered.find(token)?;
        let prefix = &rendered[..token_start];
        let marker = "\x1b[38;2;";
        let start = prefix.rfind(marker)? + marker.len();
        let end = rendered[start..].find('m')? + start;
        let mut channels = rendered[start..end]
            .split(';')
            .filter_map(|part| part.parse::<u8>().ok());
        Some((channels.next()?, channels.next()?, channels.next()?))
    }
}
