//! Built-in IDE state, selection helpers, viewport actions, and prompt queue types.

use super::*;

/// One visible row of the `/ide` file tree (a flattened, expandable tree).
pub(super) struct IdeEntry {
    pub(super) path: std::path::PathBuf,
    pub(super) name: String,
    pub(super) depth: usize,
    pub(super) is_dir: bool,
    pub(super) expanded: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IdeIntelligenceTarget {
    pub(super) path: String,
    pub(super) position: CodePosition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct IdeIntelligenceRow {
    pub(super) text: String,
    pub(super) target: Option<IdeIntelligenceTarget>,
}

pub(super) struct IdeIntelligenceView {
    pub(super) request_id: u64,
    pub(super) title: String,
    pub(super) rows: Vec<IdeIntelligenceRow>,
    pub(super) selected: usize,
    pub(super) scroll: usize,
    pub(super) truncated: bool,
    pub(super) saved_version: bool,
    pub(super) dirty_buffer: bool,
    pub(super) stale: bool,
    pub(super) workspace_revision: Option<u64>,
}

impl IdeIntelligenceView {
    pub(super) fn loading(
        request_id: u64,
        title: impl Into<String>,
        saved_version: bool,
        dirty_buffer: bool,
    ) -> Self {
        Self {
            request_id,
            title: title.into(),
            rows: vec![IdeIntelligenceRow {
                text: "Loading Code Intelligence…".to_owned(),
                target: None,
            }],
            selected: 0,
            scroll: 0,
            truncated: false,
            saved_version,
            dirty_buffer,
            stale: false,
            workspace_revision: None,
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum IdePrompt {
    Search { forward: bool, text: String },
    Command(String),
}

/// Editor input mode — vim-aligned: Normal navigates/operates, Insert types.
/// Freshly opened buffers start in Normal.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) enum EditMode {
    Normal,
    Insert,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub(super) struct PendingOp {
    pub(super) op: char,
    pub(super) count: usize,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub(super) enum RepeatEdit {
    DeleteChar(usize),
    DeleteLine(usize),
    DeleteWord(usize),
    DeleteToEol,
    ChangeLine(usize),
    Replace(char),
    JoinLine(usize),
    ToggleCase(usize),
}

/// An open, editable file in the `/ide` panel.
pub(super) struct IdeFile {
    pub(super) path: std::path::PathBuf,
    pub(super) lines: Vec<String>, // text rows, or pre-rendered half-block rows if `image`
    pub(super) scroll: usize,      // top visible row (vertical scroll)
    pub(super) hscroll: usize,     // leftmost visible column (horizontal scroll; display columns)
    pub(super) row: usize,         // cursor line
    pub(super) col: usize,         // cursor column (char index)
    pub(super) dirty: bool,
    pub(super) image: bool,    // read-only image preview
    pub(super) readonly: bool, // view-only (e.g. a dynamic-workflow artifact) — edits blocked
    pub(super) mode: EditMode, // vim Normal/Insert (see `ide_key`)
    /// A pending operator/prefix awaiting its second keystroke (`d`, `c`, `g`, `y`).
    pub(super) pending: Option<PendingOp>,
    /// Normal-mode numeric prefix (`3j`, `5dd`, `2w`).
    pub(super) count: Option<usize>,
    /// Undo snapshots (lines + cursor) for `u`; bounded — configs are small.
    pub(super) undo: Vec<(Vec<String>, usize, usize)>,
    /// Redo snapshots for Ctrl+R.
    pub(super) redo: Vec<(Vec<String>, usize, usize)>,
    /// Last repeatable Normal-mode change for `.`.
    pub(super) last_change: Option<RepeatEdit>,
    /// Last search query and direction for `n` / `N`.
    pub(super) search: Option<(String, bool)>,
    /// Visual Line anchor row (`V`).
    pub(super) visual_line_anchor: Option<usize>,
    pub(super) clip: String,        // yank/delete register for p / P
    pub(super) clip_linewise: bool, // register holds whole lines (dd/yy) vs an inline span
}

impl IdeFile {
    /// A freshly opened buffer: cursor at the top, Normal mode, empty undo.
    pub(super) fn new(
        path: std::path::PathBuf,
        lines: Vec<String>,
        image: bool,
        readonly: bool,
    ) -> Self {
        IdeFile {
            path,
            lines: if lines.is_empty() {
                vec![String::new()]
            } else {
                lines
            },
            scroll: 0,
            hscroll: 0,
            row: 0,
            col: 0,
            dirty: false,
            image,
            readonly,
            mode: EditMode::Normal,
            pending: None,
            count: None,
            undo: Vec::new(),
            redo: Vec::new(),
            last_change: None,
            search: None,
            visual_line_anchor: None,
            clip: String::new(),
            clip_linewise: false,
        }
    }
}

/// Overlay the shared one-column vertical scrollbar on the viewport's final
/// column. When content fits, every row keeps the full terminal width instead
/// of reserving an empty right gutter.
pub(super) fn append_scrollbar(
    view: &str,
    canvas_width: usize,
    total: usize,
    scroll_percent: u8,
) -> String {
    if canvas_width == 0 {
        return view
            .split('\n')
            .map(|_| String::new())
            .collect::<Vec<_>>()
            .join("\n");
    }

    let visible = view.split('\n').count();
    let scrollbar = Scrollbar::from_scroll_percent(total, visible, scroll_percent)
        .track_color(TN_GRAY)
        .thumb_color(ACCENT)
        .hide_when_not_overflowing(true);
    if scrollbar.has_overflow() {
        let content_width = canvas_width.saturating_sub(1);
        let bar = scrollbar.styled_view(visible);
        return view
            .split('\n')
            .zip(bar.lines())
            .map(|(row, bar)| format!("{}{bar}", fit_viewport_row(row, content_width)))
            .collect::<Vec<_>>()
            .join("\n");
    }

    view.split('\n')
        .map(|row| fit_viewport_row(row, canvas_width))
        .collect::<Vec<_>>()
        .join("\n")
}

pub(super) fn fit_viewport_row(row: &str, width: usize) -> String {
    let fitted = a3s_tui::style::truncate_visible(row, width);
    let padding = width.saturating_sub(a3s_tui::style::visible_len(&fitted));
    if padding == 0 {
        return fitted;
    }

    let padding = a3s_tui::markdown::trailing_ansi_background(row)
        .map(|color| Style::new().bg(color).render(&" ".repeat(padding)))
        .unwrap_or_else(|| " ".repeat(padding));
    format!("{fitted}{padding}")
}

/// The OSC 52 escape that asks the terminal to set the system clipboard to
/// `text` (base64). Works over SSH on terminals that support OSC 52. Capped so a
/// long reply can't blow past a terminal's OSC 52 size limit.
pub(super) const OSC52_PAYLOAD_BYTE_LIMIT: usize = 64_000;

pub(super) fn osc52_copy(text: &str) -> String {
    use base64::Engine;
    let capped = osc52_payload(text);
    let b64 = base64::engine::general_purpose::STANDARD.encode(capped.as_bytes());
    format!("\x1b]52;c;{b64}\x07")
}

fn osc52_payload(text: &str) -> &str {
    if text.len() <= OSC52_PAYLOAD_BYTE_LIMIT {
        return text;
    }
    let mut end = OSC52_PAYLOAD_BYTE_LIMIT;
    while !text.is_char_boundary(end) {
        end = end.saturating_sub(1);
    }
    &text[..end]
}

/// Marker the agent puts inline in its reply to offer the RemoteUI popup. The
/// host recognises a mouse click on any reply line containing it and opens the
/// remembered view. The styled button is still transcript text, so ANSI stripping
/// keeps this marker clickable.
pub(super) const VIEW_BUTTON_MARKER: &str = "Open view";
pub(super) const VIEW_BUTTON_CLICK_DRIFT_COLS: u16 = 2;
#[cfg(test)]
pub(super) const RESEARCH_VIEW_MARKER: &str = "A3S_RESEARCH_VIEW:";

pub(super) fn remote_view_button(detail: &str) -> String {
    InlineAction::new(VIEW_BUTTON_MARKER)
        .icon("↗")
        .colors(TN_FG, ACCENT)
        .detail_color(TN_GRAY)
        .detail(detail)
        .view()
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ClipboardCopyOutcome {
    /// The terminal accepted the OSC 52 request on stdout. The terminal may
    /// still decline it according to its own clipboard policy.
    pub(super) terminal_requested: bool,
    /// A platform clipboard helper completed successfully.
    pub(super) native_delivered: bool,
    /// OSC 52 carried only the bounded prefix of a larger payload.
    pub(super) terminal_truncated: bool,
    /// Number of complete UTF-8 characters carried by the OSC 52 payload.
    pub(super) terminal_character_count: usize,
}

/// Put `text` on the system clipboard: OSC 52 (portable, survives SSH on
/// supporting terminals) plus the native tool where we have one (macOS pbcopy).
/// The outcome distinguishes a terminal request from verified native delivery.
pub(super) fn copy_to_clipboard(text: &str) -> ClipboardCopyOutcome {
    use std::io::Write;
    let mut out = std::io::stdout();
    let terminal_requested = out
        .write_all(osc52_copy(text).as_bytes())
        .and_then(|()| out.flush())
        .is_ok();
    let terminal_payload = osc52_payload(text);
    let terminal_truncated = terminal_payload.len() < text.len();
    let terminal_character_count = terminal_payload.chars().count();
    #[cfg(target_os = "macos")]
    let native_delivered = {
        let mut delivered = false;
        if let Ok(mut child) = std::process::Command::new("pbcopy")
            .stdin(std::process::Stdio::piped())
            .spawn()
        {
            let wrote = child
                .stdin
                .take()
                .is_some_and(|mut stdin| stdin.write_all(text.as_bytes()).is_ok());
            let succeeded = child.wait().is_ok_and(|status| status.success());
            delivered = wrote && succeeded;
        }
        delivered
    };
    #[cfg(not(target_os = "macos"))]
    let native_delivered = false;
    ClipboardCopyOutcome {
        terminal_requested,
        native_delivered,
        terminal_truncated,
        terminal_character_count,
    }
}

/// Background of an active text selection in the transcript.
pub(super) const SELECTION_BG: Color = SURFACE_SELECTED;

/// Mouse selection projected into the visible transcript viewport.
///
/// Screen cells drive the current highlight. When both endpoints belong to
/// committed transcript entries, `semantic` retains their stable identities so
/// content refresh and resize can project fresh cells without losing the
/// selection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Selection {
    pub(super) anchor: (u16, u16),
    pub(super) head: (u16, u16),
    semantic: Option<TranscriptSelection>,
}

impl Selection {
    pub(super) fn start(cell: (u16, u16), point: Option<TranscriptPoint>) -> Self {
        Self {
            anchor: cell,
            head: cell,
            semantic: point.map(TranscriptSelection::collapsed),
        }
    }

    #[cfg(test)]
    pub(super) fn from_cells(anchor: (u16, u16), head: (u16, u16)) -> Self {
        Self {
            anchor,
            head,
            semantic: None,
        }
    }

    pub(super) fn set_head(&mut self, cell: (u16, u16), point: Option<TranscriptPoint>) {
        self.head = cell;
        match (&mut self.semantic, point) {
            (Some(selection), Some(point)) => selection.set_head(point),
            _ => self.semantic = None,
        }
    }

    pub(super) fn semantic(&self) -> Option<TranscriptSelection> {
        self.semantic
    }

    pub(super) fn project(&mut self, anchor: (u16, u16), head: (u16, u16)) {
        self.anchor = anchor;
        self.head = head;
    }

    pub(super) fn is_empty(&self) -> bool {
        self.anchor == self.head
    }
    /// (top_row, top_col, bottom_row, bottom_col), as usize.
    pub(super) fn ordered(&self) -> (usize, usize, usize, usize) {
        let (a, b) = if self.anchor <= self.head {
            (self.anchor, self.head)
        } else {
            (self.head, self.anchor)
        };
        (a.0 as usize, a.1 as usize, b.0 as usize, b.1 as usize)
    }
}

pub(super) fn viewport_mouse_cell(
    row: u16,
    column: u16,
    viewport_rows: usize,
    max_col: u16,
) -> Option<(u16, u16)> {
    if viewport_rows == 0 || row as usize >= viewport_rows {
        None
    } else {
        Some((row, column.min(max_col)))
    }
}

pub(super) fn viewport_mouse_cell_clamped(
    row: u16,
    column: u16,
    viewport_rows: usize,
    max_col: u16,
) -> Option<(u16, u16)> {
    if viewport_rows == 0 {
        None
    } else {
        Some((
            row.min(viewport_rows.saturating_sub(1) as u16),
            column.min(max_col),
        ))
    }
}

/// Substring of `s` spanning visible columns `[from, to)` (wide chars counted by
/// display width). A char straddling the start is dropped; one straddling the
/// end is kept.
pub(super) fn slice_cols(s: &str, from: usize, to: usize) -> String {
    let mut col = 0usize;
    let mut out = String::new();
    for ch in s.chars() {
        if col >= to {
            break;
        }
        if col >= from {
            out.push(ch);
        }
        col += a3s_tui::style::visible_len(&ch.to_string());
    }
    out
}

/// Plain text of a selection over the rendered viewport `view`: screen rows
/// `r1..=r2`, columns `[c1, c2)` clipped on the first/last rows. Rows are
/// ANSI-stripped and trailing padding trimmed.
pub(super) fn selection_to_text(view: &str, r1: usize, c1: usize, r2: usize, c2: usize) -> String {
    let rows: Vec<&str> = view.split('\n').collect();
    let mut out: Vec<String> = Vec::new();
    for r in r1..=r2 {
        let Some(row) = rows.get(r) else { break };
        let plain = a3s_tui::style::strip_ansi(row);
        let from = if r == r1 { c1 } else { 0 };
        let to = if r == r2 { c2 } else { usize::MAX };
        out.push(slice_cols(&plain, from, to).trim_end().to_string());
    }
    out.join("\n")
}

pub(super) fn viewport_row_contains_view_button(view: &str, row: u16) -> bool {
    view.split('\n')
        .nth(row as usize)
        .map(a3s_tui::style::strip_ansi)
        .is_some_and(|line| line.to_ascii_lowercase().contains("open view"))
}

pub(super) fn is_remote_view_click(view: &str, selection: Selection) -> bool {
    selection.anchor.0 == selection.head.0
        && selection.anchor.1.abs_diff(selection.head.1) <= VIEW_BUTTON_CLICK_DRIFT_COLS
        && viewport_row_contains_view_button(view, selection.anchor.0)
}

pub(super) fn is_quit_key(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'c'))
}

pub(super) fn is_tool_output_key(key: &KeyEvent) -> bool {
    key.modifiers.contains(KeyModifiers::CONTROL)
        && matches!(key.code, KeyCode::Char(c) if c.eq_ignore_ascii_case(&'t'))
}

pub(super) fn quit_is_confirmed(armed: Option<Instant>, now: Instant) -> bool {
    armed.is_some_and(|t| now.saturating_duration_since(t) < Duration::from_secs(2))
}

/// Re-render the viewport `view` with the selected span highlighted: selected
/// rows render in plain text (no syntax colour, transiently) with the selected
/// columns on `SELECTION_BG`; other rows keep their styling.
pub(super) fn highlight_selection(
    view: &str,
    r1: usize,
    c1: usize,
    r2: usize,
    c2: usize,
) -> String {
    let bg = Style::new().bg(SELECTION_BG).fg(TN_FG);
    view.split('\n')
        .enumerate()
        .map(|(i, row)| {
            if i < r1 || i > r2 {
                return row.to_string();
            }
            let plain = a3s_tui::style::strip_ansi(row);
            let from = if i == r1 { c1 } else { 0 };
            let to = if i == r2 { c2 } else { usize::MAX };
            let before = slice_cols(&plain, 0, from);
            let sel = slice_cols(&plain, from, to);
            let after = slice_cols(&plain, to, usize::MAX);
            format!("{before}{}{after}", bg.render(&sel))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// State of the `/ide` panel: the file tree, selection, and the open file.
/// Also backs `/config` (rooted at the config dir) and the `/kb` browser
/// (rooted at the vault, with delete enabled) — all superfile-styled.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IdeSurface {
    Workspace,
    ReusedEditor,
}

pub(super) struct Ide {
    pub(super) entries: Vec<IdeEntry>,
    pub(super) sel: usize,
    pub(super) tree_scroll: usize,
    pub(super) file: Option<IdeFile>,
    pub(super) focus_editor: bool,
    /// Transient save status shown in the footer (set on Ctrl+S).
    pub(super) flash: Option<String>,
    /// Left-panel title ("workspace" / "config" / "knowledge base").
    pub(super) title: String,
    /// Product surface owning this editor. Semantic workspace commands are
    /// intentionally available only in the real `/ide` workspace surface.
    pub(super) surface: IdeSurface,
    /// Superfile-style hover preview of the tree-selected file, keyed by path
    /// so it reloads only when the selection actually moves.
    pub(super) preview: Option<(std::path::PathBuf, Vec<String>)>,
    /// Active command prompt inside the editor footer (`/`, `?`, `:`).
    pub(super) prompt: Option<IdePrompt>,
    /// One-shot artifact path requested by tree `p` or editor `:preview`.
    /// The outer TUI takes it after key handling and owns the async launch.
    pub(super) preview_request: Option<std::path::PathBuf>,
    /// `/kb` browser: the vault root. Enables `x` delete, hard-bounded to
    /// paths inside this root. `None` for /ide and /config.
    pub(super) kb_root: Option<std::path::PathBuf>,
    /// A path armed for deletion — the next `x` on the same selection deletes.
    pub(super) armed_delete: Option<std::path::PathBuf>,
    /// Selected action in the `/kb` delete confirmation row (`true` = delete).
    pub(super) delete_confirm_yes: bool,
    /// Active Code Intelligence result list in the right panel.
    pub(super) intelligence: Option<IdeIntelligenceView>,
    /// Monotonic guard that makes late asynchronous results inert.
    pub(super) intelligence_request_id: u64,
    /// Cancels the active semantic query when it is replaced or the panel closes.
    pub(super) intelligence_cancellation: tokio_util::sync::CancellationToken,
    /// Monotonic guard for asynchronous result jumps within one query view.
    pub(super) intelligence_jump_request_id: u64,
    /// Cancels the previous result jump when a newer selection is opened.
    pub(super) intelligence_jump_cancellation: tokio_util::sync::CancellationToken,
}

impl Ide {
    /// A fresh panel over `entries` (no file open, tree focused).
    pub(super) fn browse(entries: Vec<IdeEntry>, title: &str) -> Self {
        Ide {
            entries,
            sel: 0,
            tree_scroll: 0,
            file: None,
            focus_editor: false,
            flash: None,
            title: title.to_string(),
            surface: IdeSurface::ReusedEditor,
            preview: None,
            prompt: None,
            preview_request: None,
            kb_root: None,
            armed_delete: None,
            delete_confirm_yes: true,
            intelligence: None,
            intelligence_request_id: 0,
            intelligence_cancellation: tokio_util::sync::CancellationToken::new(),
            intelligence_jump_request_id: 0,
            intelligence_jump_cancellation: tokio_util::sync::CancellationToken::new(),
        }
    }

    pub(super) fn workspace(entries: Vec<IdeEntry>) -> Self {
        let mut ide = Self::browse(entries, "workspace");
        ide.surface = IdeSurface::Workspace;
        ide
    }

    pub(super) fn supports_code_intelligence(&self) -> bool {
        self.surface == IdeSurface::Workspace
    }
}

impl Drop for Ide {
    fn drop(&mut self) {
        self.intelligence_cancellation.cancel();
        self.intelligence_jump_cancellation.cancel();
    }
}

/// Directory children for the tree, dirs first then files, noise skipped.
pub(super) fn ide_children(dir: &std::path::Path, depth: usize) -> Vec<IdeEntry> {
    let mut v: Vec<IdeEntry> = std::fs::read_dir(dir)
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            if matches!(
                name.as_str(),
                ".git" | "node_modules" | "target" | ".DS_Store" | ".next" | "dist"
            ) {
                return None;
            }
            let is_dir = e.path().is_dir();
            Some(IdeEntry {
                path: e.path(),
                name,
                depth,
                is_dir,
                expanded: false,
            })
        })
        .collect();
    v.sort_by(|a, b| {
        b.is_dir
            .cmp(&a.is_dir)
            .then(a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    v
}

/// Project instructions for the agent's system prompt. a3s-code already
/// auto-loads `AGENTS.md`; this adds Claude Code's `CLAUDE.md` (preferred), so
/// existing projects work unchanged. Returns the content wrapped with a header.
pub(super) fn project_instructions(workspace: &str) -> Option<String> {
    for name in ["CLAUDE.md", "AGENT.md"] {
        let p = std::path::Path::new(workspace).join(name);
        if let Ok(c) = std::fs::read_to_string(&p) {
            if !c.trim().is_empty() {
                return Some(format!("# Project Instructions ({name})\n\n{c}"));
            }
        }
    }
    None
}

/// Root content is full-bleed. Individual components still own the indentation
/// needed for prompts, markers, trees, and nested content.
pub(super) const PAD: usize = 0;

pub(super) fn viewport_content_width_for(width: u16) -> usize {
    width as usize
}

pub(super) fn transcript_markdown_width_for(width: u16) -> usize {
    viewport_content_width_for(width).saturating_sub(PAD + 2)
}

pub(super) fn textarea_width_for(width: u16) -> u16 {
    transcript_markdown_width_for(width).min(u16::MAX as usize) as u16
}

/// Run mode, cycled with Shift+Tab.
#[derive(Clone, Copy, PartialEq)]
pub(super) enum Mode {
    /// Standard risk-aware mode: safe reads run quietly and side effects prompt.
    Default,
    /// Strict read-only planning mode. Implementation starts only after review.
    Plan,
    /// Non-interactive execution. Hard policy and workspace denials still win.
    Auto,
}

impl Mode {
    pub(super) fn next(self) -> Self {
        match self {
            Mode::Default => Mode::Plan,
            Mode::Plan => Mode::Auto,
            Mode::Auto => Mode::Default,
        }
    }

    pub(super) fn glyph(self) -> &'static str {
        match self {
            Mode::Default => "⏵",
            Mode::Plan => "✎",
            Mode::Auto => "⏵⏵",
        }
    }

    /// Short one-word name for the status line ("auto mode on").
    pub(super) fn name(self) -> &'static str {
        match self {
            Mode::Default => "default",
            Mode::Plan => "plan",
            Mode::Auto => "auto",
        }
    }

    pub(super) fn color(self) -> Color {
        match self {
            Mode::Default => COMPOSER_CHROME.faint,
            Mode::Plan => COMPOSER_CHROME.active,
            Mode::Auto => COMPOSER_CHROME.warning,
        }
    }
}

/// Highest runnable host-turn priority. Safety decisions and stream lifecycle
/// barriers are control-plane operations and therefore remain outside the
/// pending-turn queue.
pub(super) const PLAN_REVIEW_PRIORITY: a3s_lane::Priority = 0;
/// Explicit user submissions run before autonomous continuations.
pub(super) const USER_TURN_PRIORITY: a3s_lane::Priority = 1;
/// Host-generated work that must never overtake an explicit user turn.
pub(super) const SYNTHETIC_TURN_PRIORITY: a3s_lane::Priority = 2;

/// A host-owned turn queued while the agent is busy. Priority and FIFO
/// metadata are owned by `a3s_lane::PriorityQueue`, not duplicated in the UI
/// payload.
#[derive(Clone)]
pub(super) struct Queued {
    pub(super) text: String,
    pub(super) display: String,
    /// Attachments captured for this exact queued turn.
    pub(super) images: Vec<PendingImage>,
    pub(super) runtime_expectation: Option<RuntimeExpectation>,
    pub(super) deep_research: Option<(String, DeepResearchEvidenceScope)>,
}
