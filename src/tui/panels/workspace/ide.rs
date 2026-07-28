//! `/ide` panel: file-tree navigation + in-place file editing/preview, drawn
//! superfile-style (rounded panels, focus-coloured borders, hover preview,
//! breadcrumb title, metadata footer). Also backs `/config` and the `/kb`
//! knowledge-base browser.

use super::super::*;
use super::code_intelligence::{
    ide_intelligence_command_hint, ide_intelligence_notice, ide_intelligence_panel,
};
use super::spf;
use a3s_tui::components::{Confirm, ConfirmMsg, CursorLine};

impl App {
    /// Handle a key while the `/ide` panel is open. Returns true if consumed.
    pub(crate) fn ide_key(&mut self, key: &KeyEvent) -> bool {
        if self.ide.is_none() {
            return false;
        }
        // Esc: in Insert mode (or with a pending operator) it returns to a clean
        // Normal mode and stays in the editor; in Normal it leaves the editor back
        // to the tree; from the tree it closes the panel.
        if key.code == KeyCode::Esc {
            if let Some(ide) = self.ide.as_mut().filter(|i| i.armed_delete.is_some()) {
                cancel_kb_delete_confirm(ide);
                return true;
            }
            if let Some(ide) = self.ide.as_mut().filter(|i| i.prompt.is_some()) {
                ide.prompt = None;
                ide.flash = None;
                return true;
            }
            if let Some(f) = self
                .ide
                .as_mut()
                .filter(|i| i.focus_editor)
                .and_then(|i| i.file.as_mut())
            {
                if f.mode == EditMode::Insert
                    || f.pending.is_some()
                    || f.visual_line_anchor.is_some()
                {
                    f.mode = EditMode::Normal;
                    f.pending = None;
                    f.count = None;
                    f.visual_line_anchor = None;
                    f.clamp_col();
                    return true;
                }
            }
            let editing = self.ide.as_ref().is_some_and(|i| i.focus_editor);
            if editing {
                if let Some(i) = self.ide.as_mut() {
                    i.focus_editor = false;
                    // Keep the just-left buffer visible (not a stale hover).
                    i.preview = None;
                }
            } else {
                if let Some(ide) = self.ide.take() {
                    ide.intelligence_cancellation.cancel();
                    ide.intelligence_jump_cancellation.cancel();
                }
            }
            return true;
        }
        let h = self.height as usize;
        let w = self.width as usize;
        let workspace_manifest = self.workspace_manifest.clone();
        let workspace = self.cwd.clone();
        let ide = self.ide.as_mut().unwrap();
        if ide.armed_delete.is_some() && ide.kb_root.is_some() {
            return handle_kb_delete_confirm_key(ide, key);
        }
        // Rows inside the main panels: screen minus the metadata/keys footer
        // (3) and the panel borders (2). Must match `render_ide`.
        let body = h.saturating_sub(5);
        if ide.prompt.is_some() {
            return handle_ide_prompt_key(ide, key, &workspace_manifest, &workspace);
        }
        match key.code {
            // Editor focused: full text editing of the open file.
            _ if ide.focus_editor && ide.file.is_some() => {
                let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
                // Ctrl+S saves to disk with explicit footer feedback (success or
                // the OS error) — handled before the long-lived &mut file borrow.
                if ctrl && matches!(key.code, KeyCode::Char('s' | 'S')) {
                    let msg = {
                        let f = ide.file.as_mut().unwrap();
                        save_ide_file(f, &workspace_manifest, &workspace)
                    };
                    ide.flash = Some(msg);
                    return true;
                }
                if ctrl && matches!(key.code, KeyCode::Char('v' | 'V')) {
                    ide.flash = Some(match read_clipboard_text() {
                        Ok(text) => paste_text_into_ide(ide, &text),
                        Err(error) => ide_flash_line(ToastKind::Error, error),
                    });
                    return true;
                }
                if !ctrl
                    && ide
                        .file
                        .as_ref()
                        .is_some_and(|f| f.mode == EditMode::Normal && f.pending.is_none())
                {
                    match key.code {
                        KeyCode::Char('/') => {
                            ide.prompt = Some(IdePrompt::Search {
                                forward: true,
                                text: String::new(),
                            });
                            return true;
                        }
                        KeyCode::Char('?') => {
                            ide.prompt = Some(IdePrompt::Search {
                                forward: false,
                                text: String::new(),
                            });
                            return true;
                        }
                        KeyCode::Char(':') => {
                            ide.prompt = Some(IdePrompt::Command(String::new()));
                            return true;
                        }
                        _ => {}
                    }
                }
                ide.flash = None; // any edit/nav key dismisses the save flash
                                  // Editor content width — single-sourced with `render_ide` via
                                  // spf::ide_content_width so the cursor's horizontal scroll tracks.
                let content_w = spf::ide_content_width(w);
                let f = ide.file.as_mut().unwrap();
                if f.image {
                    return true; // image preview is read-only
                }
                // Vim-aligned editing/navigation lives on the buffer itself.
                f.edit_key(key, body, content_w);
                return true;
            }
            // Tree focused: Tab enters the editor.
            KeyCode::Tab => ide.focus_editor = !ide.focus_editor,
            KeyCode::Up | KeyCode::Char('k') => ide.sel = ide.sel.saturating_sub(1),
            KeyCode::Down | KeyCode::Char('j') => {
                ide.sel = (ide.sel + 1).min(ide.entries.len().saturating_sub(1))
            }
            // ← jumps to the parent directory entry.
            KeyCode::Left => {
                if let Some(d) = ide.entries.get(ide.sel).map(|e| e.depth) {
                    if d > 0 {
                        let mut j = ide.sel;
                        while j > 0 && ide.entries[j].depth >= d {
                            j -= 1;
                        }
                        ide.sel = j;
                    }
                }
            }
            // Enter/→ toggles a directory or opens a file.
            KeyCode::Enter | KeyCode::Right if !ide.entries.is_empty() => {
                let sel = ide.sel.min(ide.entries.len() - 1);
                let (is_dir, expanded, depth, path) = {
                    let e = &ide.entries[sel];
                    (e.is_dir, e.expanded, e.depth, e.path.clone())
                };
                if is_dir && expanded {
                    ide.entries[sel].expanded = false;
                    let mut j = sel + 1;
                    while j < ide.entries.len() && ide.entries[j].depth > depth {
                        j += 1;
                    }
                    ide.entries.drain(sel + 1..j);
                } else if is_dir {
                    ide.entries[sel].expanded = true;
                    let at = sel + 1;
                    for (k, c) in ide_children(&path, depth + 1).into_iter().enumerate() {
                        ide.entries.insert(at + k, c);
                    }
                } else if is_image_path(&path) {
                    let inner = spf::ide_split(w).1.saturating_sub(2);
                    let lines = render_image_file(&path, inner, body)
                        .unwrap_or_else(|| vec!["<cannot decode image>".into()]);
                    touch_workspace_file_path_for_manifest(&workspace_manifest, &workspace, &path);
                    ide.file = Some(IdeFile::new(path, lines, true, true));
                    ide.focus_editor = false; // read-only; keep tree focus
                } else {
                    let lines: Vec<String> = std::fs::read_to_string(&path)
                        .unwrap_or_else(|err| format!("<cannot read: {err}>"))
                        .replace('\t', "    ")
                        .lines()
                        .map(String::from)
                        .collect();
                    touch_workspace_file_path_for_manifest(&workspace_manifest, &workspace, &path);
                    ide.file = Some(IdeFile::new(path, lines, false, false));
                    ide.focus_editor = true;
                }
            }
            // Workspace tree only: open the selected file or static-site
            // directory in the host-owned live-preview surface.
            KeyCode::Char('p')
                if ide.surface == IdeSurface::Workspace && !ide.entries.is_empty() =>
            {
                let sel = ide.sel.min(ide.entries.len() - 1);
                ide.preview_request = Some(ide.entries[sel].path.clone());
                ide.flash = Some(ide_flash_line(ToastKind::Info, "opening live preview…"));
            }
            // `/kb` browser only: `x` opens a shared Confirm row for deletion.
            KeyCode::Char('x') if ide.kb_root.is_some() && !ide.entries.is_empty() => {
                let sel = ide.sel.min(ide.entries.len() - 1);
                let path = ide.entries[sel].path.clone();
                if ide.armed_delete.as_deref() == Some(path.as_path()) {
                    delete_selected_kb_entry(ide);
                } else {
                    ide.armed_delete = Some(path);
                    ide.delete_confirm_yes = true;
                    ide.flash = None;
                }
            }
            _ => {}
        }
        // Any tree key other than `x` disarms a pending delete AND dismisses a
        // lingering flash (mirrors the editor, where any key clears it) — else
        // a "✔ deleted …" result masks the key hints for the whole session.
        if !matches!(key.code, KeyCode::Char('x' | 'p')) {
            ide.armed_delete = None;
            ide.flash = None;
        }
        // Keep the tree selection within the visible window.
        if ide.sel < ide.tree_scroll {
            ide.tree_scroll = ide.sel;
        } else if body > 0 && ide.sel >= ide.tree_scroll + body {
            ide.tree_scroll = ide.sel + 1 - body;
        }
        // Superfile-style hover preview of the tree-selected file.
        if !ide.focus_editor {
            refresh_preview(ide);
        }
        true
    }

    /// Breadcrumb for the right panel's title: the path relative to the cwd
    /// (or `~`) with superfile-style ` › ` separators.
    fn ide_crumb(&self, path: &std::path::Path) -> String {
        let rel: std::path::PathBuf = if let Ok(p) = path.strip_prefix(&self.cwd) {
            p.to_path_buf()
        } else if let Some(p) = crate::user_paths::user_home_dir()
            .and_then(|home| path.strip_prefix(home).ok().map(|p| p.to_path_buf()))
        {
            std::path::Path::new("~").join(p)
        } else {
            path.to_path_buf()
        };
        let parts: Vec<String> = rel
            .components()
            .map(|c| c.as_os_str().to_string_lossy().into_owned())
            .collect();
        if parts.is_empty() {
            path.display().to_string()
        } else {
            parts.join(" › ")
        }
    }

    /// Full-screen `/ide`, superfile-style: rounded tree + editor/preview
    /// panels (border colour marks focus), breadcrumb in the right panel's
    /// title, and details + controls footer panels.
    pub(crate) fn render_ide(&self, ide: &Ide) -> String {
        let width = self.width as usize;
        let h = self.height as usize;
        let (tw, rw) = spf::ide_split(width);
        let main_h = h.saturating_sub(3);
        let body = main_h.saturating_sub(2);
        let tree_iw = tw.saturating_sub(2);
        let right_iw = rw.saturating_sub(2);

        // ── left panel: the tree, icons + selection highlight ──
        let mut tree_rows = Vec::with_capacity(body);
        for i in 0..body {
            let row = if let Some(e) = ide.entries.get(ide.tree_scroll + i) {
                spf::file_tree_row(
                    &e.name,
                    e.depth,
                    e.is_dir,
                    e.expanded,
                    ide.tree_scroll + i == ide.sel,
                    !ide.focus_editor,
                    tree_iw,
                )
            } else {
                String::new()
            };
            tree_rows.push(row);
        }

        // ── right panel: hover preview (tree focus) beats the open buffer,
        // except when the hovered file IS the open buffer (unsaved edits win).
        let hover = (!ide.focus_editor)
            .then_some(ide.preview.as_ref())
            .flatten()
            .filter(|(p, _)| ide.file.as_ref().is_none_or(|f| f.path != *p));
        let (right_title, right_rows, meta_path, meta_lines): (
            String,
            Vec<String>,
            Option<std::path::PathBuf>,
            Option<usize>,
        ) = if let Some((title, rows)) = ide_intelligence_panel(ide, body, right_iw) {
            (
                title,
                rows,
                ide.file.as_ref().map(|file| file.path.clone()),
                ide.file.as_ref().map(|file| file.lines.len()),
            )
        } else if let Some((path, plines)) = hover {
            let icon = spf::path_icon(path, false, false);
            let rows = plines
                .iter()
                .take(body)
                .map(|l| highlight_code(&spf::fit(l, right_iw), lang_of(path)))
                .collect();
            (
                format!("{} preview · {}", icon.glyph, self.ide_crumb(path)),
                rows,
                Some(path.clone()),
                Some(plines.len()),
            )
        } else if let Some(f) = &ide.file {
            let icon = spf::path_icon(&f.path, false, false);
            let mut rows = Vec::with_capacity(body);
            for i in 0..body {
                let row = if f.image {
                    // Pre-rendered half-block rows; raw, no line numbers.
                    f.lines.get(f.scroll + i).cloned().unwrap_or_default()
                } else if let Some(line) = f.lines.get(f.scroll + i) {
                    let lineno = f.scroll + i;
                    let cur_row = ide.focus_editor && lineno == f.row;
                    let visual_row = f.visual_line_anchor.is_some_and(|anchor| {
                        let lo = anchor.min(f.row);
                        let hi = anchor.max(f.row);
                        (lo..=hi).contains(&lineno)
                    });
                    let num = if spf::ide_gutter_on(width) {
                        spf::ide_gutter(lineno + 1, cur_row)
                    } else {
                        String::new() // tiny panel: every column goes to text
                    };
                    let cw = spf::ide_content_width(width);
                    // Horizontal scroll: render only [hscroll, hscroll+cw) so
                    // long lines scroll sideways instead of clipping. A wide
                    // (CJK) glyph straddling the window end is kept by
                    // slice_cols — pop it or the row overflows the frame.
                    let mut window = slice_cols(line, f.hscroll, f.hscroll + cw);
                    while a3s_tui::style::visible_len(&window) > cw {
                        window.pop();
                    }
                    let body_str = if cur_row {
                        // Clamp the block cursor into the window (a resize can
                        // leave hscroll stale until the next keypress).
                        let ccol = f
                            .display_col()
                            .saturating_sub(f.hscroll)
                            .min(cw.saturating_sub(1));
                        render_cursor_line(&window, ccol)
                    } else if visual_row {
                        Style::new().fg(TN_FG).bg(SURFACE_SELECTED).render(&window)
                    } else {
                        highlight_code(&window, lang_of(&f.path))
                    };
                    format!("{num}{body_str}")
                } else {
                    String::new()
                };
                rows.push(row);
            }
            let mut title = format!("{} {}", icon.glyph, self.ide_crumb(&f.path));
            if f.dirty {
                title.push_str(" ●");
            }
            if f.readonly {
                title.push_str(" (read-only)");
            }
            (title, rows, Some(f.path.clone()), Some(f.lines.len()))
        } else {
            let sel_dir = ide
                .entries
                .get(ide.sel)
                .filter(|e| e.is_dir)
                .map(|e| e.path.clone());
            (
                "· preview".to_string(),
                vec![Style::new().fg(TN_GRAY).render(&spf::fit(
                    "  Select a file · Enter open · Tab focus",
                    right_iw,
                ))],
                sel_dir,
                None,
            )
        };

        let tree_icon = spf::file_icon(&ide.title, true, true);
        let tree_title = format!("{} {}", tree_icon.glyph, ide.title);
        let left = spf::frame(&tree_title, tw, main_h, !ide.focus_editor, tree_rows);
        let right = spf::frame(&right_title, rw, main_h, ide.focus_editor, right_rows);
        let mut out = spf::hjoin(&left, &right);

        // ── footer: metadata + keys panels ──
        let readonly = ide.file.as_ref().is_some_and(|f| f.readonly);
        let mode = ide.file.as_ref().map(|f| f.mode);
        let hint: String = if let Some(prompt) = ide.prompt.as_ref() {
            match prompt {
                IdePrompt::Search {
                    forward: true,
                    text,
                } => format!("/{text}"),
                IdePrompt::Search {
                    forward: false,
                    text,
                } => format!("?{text}"),
                IdePrompt::Command(text) => format!(":{text}"),
            }
        } else if let Some(flash) = ide.flash.as_deref() {
            flash.to_string()
        } else if let Some(view) = ide.intelligence.as_ref() {
            ide_intelligence_notice(view)
        } else if ide.focus_editor && readonly {
            "read-only · NORMAL · hjkl/↑↓ move · gg/G top/bottom · Esc back".to_string()
        } else if ide.focus_editor {
            match mode {
                Some(EditMode::Insert) => {
                    "-- INSERT -- · paste Cmd/Ctrl+V · Ctrl+Z undo · Ctrl+S save".to_string()
                }
                _ if ide.supports_code_intelligence() => {
                    format!(
                        "{} · :preview · :w/:q · / search",
                        ide_intelligence_command_hint()
                    )
                }
                _ => "-- NORMAL -- · / search · V visual-line · :w/:q/:wq · . repeat".to_string(),
            }
        } else if ide.kb_root.is_some() {
            "Tab edit · ↑↓ nav · Enter open · x delete · Esc close".to_string()
        } else {
            "Tab edit · ↑↓ nav · Enter open · p preview · Esc close".to_string()
        };
        let meta_w = (width * 3) / 5;
        let keys_w = width.saturating_sub(meta_w);
        let meta_inner = meta_w.saturating_sub(2);
        let meta_line = meta_path
            .as_deref()
            .map(|p| spf::file_meta_breadcrumb_line(p, meta_lines, meta_inner))
            .unwrap_or_else(|| Style::new().fg(TN_GRAY).render(&spf::fit(" —", meta_inner)));
        let meta = spf::frame("details", meta_w, 3, false, vec![meta_line]);
        let keys_inner = keys_w.saturating_sub(2);
        let keys_line = if let Some(line) = kb_delete_confirm_line(ide, keys_inner) {
            line
        } else if ide
            .intelligence
            .as_ref()
            .is_some_and(|view| view.dirty_buffer || view.stale)
        {
            Style::new()
                .fg(TN_YELLOW)
                .render(&spf::fit(&format!(" {hint}"), keys_inner))
        } else if ide.flash.is_some() {
            a3s_tui::style::fit_visible(&format!(" {hint}"), keys_inner)
        } else {
            Style::new()
                .fg(TN_GRAY)
                .render(&spf::fit(&format!(" {hint}"), keys_inner))
        };
        let keys = spf::frame("controls", keys_w, 3, false, vec![keys_line]);
        out.extend(spf::hjoin(&meta, &keys));

        out.truncate(h);
        while out.len() < h {
            out.push(String::new());
        }
        out.join("\n")
    }
}

impl App {
    pub(crate) fn ide_paste_text(&mut self, text: &str) -> bool {
        let Some(ide) = self.ide.as_mut() else {
            return false;
        };
        ide.flash = Some(paste_text_into_ide(ide, text));
        true
    }
}

fn paste_text_into_ide(ide: &mut Ide, text: &str) -> String {
    let Some(file) = ide.file.as_mut() else {
        return ide_flash_line(ToastKind::Warning, "open a file first");
    };
    if file.image || file.readonly {
        return ide_flash_line(ToastKind::Warning, "read-only");
    }
    ide.focus_editor = true;
    if file.paste_external_text(text) {
        ide_flash_line(ToastKind::Success, "pasted")
    } else {
        ide_flash_line(ToastKind::Warning, "clipboard is empty")
    }
}

fn read_clipboard_text() -> Result<String, String> {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbpaste", &[])]
    } else if cfg!(target_os = "windows") {
        &[(
            "powershell",
            &["-NoProfile", "-Command", "Get-Clipboard -Raw -Format Text"],
        )]
    } else {
        &[
            ("wl-paste", &["--no-newline"]),
            ("xclip", &["-selection", "clipboard", "-out"]),
            ("xsel", &["--clipboard", "--output"]),
        ]
    };
    let mut last_error = None;
    for (program, args) in candidates {
        match std::process::Command::new(program).args(*args).output() {
            Ok(out) if out.status.success() => {
                return Ok(String::from_utf8_lossy(&out.stdout).into_owned());
            }
            Ok(out) => {
                let stderr = String::from_utf8_lossy(&out.stderr).trim().to_string();
                last_error = Some(if stderr.is_empty() {
                    format!("{program} exited with {}", out.status)
                } else {
                    stderr
                });
            }
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
            Err(err) => last_error = Some(format!("{program}: {err}")),
        }
    }
    Err(last_error.unwrap_or_else(|| "clipboard text is unavailable".to_string()))
}

fn normalize_pasted_text(text: &str) -> String {
    text.replace("\r\n", "\n")
        .replace('\r', "\n")
        .replace('\t', "    ")
}

fn save_ide_file(
    file: &mut IdeFile,
    workspace_manifest: &std::sync::Arc<LocalWorkspaceManifest>,
    workspace: &str,
) -> String {
    if file.image || file.readonly {
        return ide_flash_line(ToastKind::Warning, "read-only");
    }
    let content = format!("{}\n", file.lines.join("\n"));
    match std::fs::write(&file.path, content) {
        Ok(()) => {
            file.dirty = false;
            touch_workspace_file_path_for_manifest(workspace_manifest, workspace, &file.path);
            ide_flash_line(ToastKind::Success, "saved")
        }
        Err(e) => ide_flash_line(ToastKind::Error, format!("save failed: {e}")),
    }
}

fn handle_ide_prompt_key(
    ide: &mut Ide,
    key: &KeyEvent,
    workspace_manifest: &std::sync::Arc<LocalWorkspaceManifest>,
    workspace: &str,
) -> bool {
    let Some(mut prompt) = ide.prompt.take() else {
        return false;
    };
    match key.code {
        KeyCode::Enter => match prompt {
            IdePrompt::Search { forward, text } => {
                if let Some(file) = ide.file.as_mut() {
                    if file.apply_search(&text, forward) {
                        ide.flash =
                            Some(ide_flash_line(ToastKind::Success, format!("found {text}")));
                    } else {
                        ide.flash = Some(ide_flash_line(
                            ToastKind::Warning,
                            format!("not found: {text}"),
                        ));
                    }
                }
            }
            IdePrompt::Command(text) => {
                ide.flash = Some(apply_ide_command(
                    ide,
                    text.trim(),
                    workspace_manifest,
                    workspace,
                ));
            }
        },
        KeyCode::Backspace => {
            match &mut prompt {
                IdePrompt::Search { text, .. } | IdePrompt::Command(text) => {
                    text.pop();
                }
            }
            ide.prompt = Some(prompt);
        }
        KeyCode::Char(c) if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT => {
            match &mut prompt {
                IdePrompt::Search { text, .. } | IdePrompt::Command(text) => text.push(c),
            }
            ide.prompt = Some(prompt);
        }
        _ => {
            ide.prompt = Some(prompt);
        }
    }
    true
}

fn apply_ide_command(
    ide: &mut Ide,
    command: &str,
    workspace_manifest: &std::sync::Arc<LocalWorkspaceManifest>,
    workspace: &str,
) -> String {
    match command {
        "w" => ide
            .file
            .as_mut()
            .map(|f| save_ide_file(f, workspace_manifest, workspace))
            .unwrap_or_else(|| ide_flash_line(ToastKind::Warning, "open a file first")),
        "q" => {
            if ide.file.as_ref().is_some_and(|f| f.dirty) {
                ide_flash_line(ToastKind::Warning, "unsaved changes; use :wq or :q!")
            } else {
                ide.focus_editor = false;
                ide_flash_line(ToastKind::Success, "closed editor")
            }
        }
        "q!" => {
            ide.focus_editor = false;
            if let Some(f) = ide.file.as_mut() {
                f.dirty = false;
            }
            ide_flash_line(ToastKind::Success, "closed editor")
        }
        "wq" | "x" => {
            let msg = ide
                .file
                .as_mut()
                .map(|f| save_ide_file(f, workspace_manifest, workspace))
                .unwrap_or_else(|| ide_flash_line(ToastKind::Warning, "open a file first"));
            if ide.file.as_ref().is_some_and(|f| !f.dirty) {
                ide.focus_editor = false;
            }
            msg
        }
        "preview" => {
            if ide.surface != IdeSurface::Workspace {
                return ide_flash_line(
                    ToastKind::Warning,
                    "live preview is available only in the workspace IDE",
                );
            }
            let Some(path) = ide.file.as_ref().map(|file| file.path.clone()) else {
                return ide_flash_line(ToastKind::Warning, "open a file first");
            };
            if ide.file.as_ref().is_some_and(|file| file.dirty) {
                let save = ide
                    .file
                    .as_mut()
                    .map(|file| save_ide_file(file, workspace_manifest, workspace))
                    .unwrap_or_else(|| ide_flash_line(ToastKind::Warning, "open a file first"));
                if ide.file.as_ref().is_some_and(|file| file.dirty) {
                    return save;
                }
            }
            ide.preview_request = Some(path);
            ide_flash_line(ToastKind::Info, "opening live preview…")
        }
        "" => ide_flash_line(ToastKind::Warning, "empty command"),
        other => ide_flash_line(ToastKind::Warning, format!("unknown command: {other}")),
    }
}

fn selected_kb_delete_target(ide: &Ide) -> Option<(usize, std::path::PathBuf, String)> {
    if ide.kb_root.is_none() || ide.entries.is_empty() {
        return None;
    }
    let sel = ide.sel.min(ide.entries.len().saturating_sub(1));
    let entry = ide.entries.get(sel)?;
    Some((sel, entry.path.clone(), entry.name.clone()))
}

fn kb_delete_confirm_line(ide: &Ide, width: usize) -> Option<String> {
    let (_, path, name) = selected_kb_delete_target(ide)?;
    if ide.armed_delete.as_deref() != Some(path.as_path()) {
        return None;
    }

    Some(
        Confirm::new(format!("Delete {name}?"))
            .with_labels("Delete", "Cancel")
            .hint("Enter/y delete | n/Esc cancel")
            .selected(ide.delete_confirm_yes)
            .danger()
            .line(width.min(u16::MAX as usize) as u16),
    )
}

fn handle_kb_delete_confirm_key(ide: &mut Ide, key: &KeyEvent) -> bool {
    let Some((_, path, name)) = selected_kb_delete_target(ide) else {
        cancel_kb_delete_confirm(ide);
        return true;
    };
    if ide.armed_delete.as_deref() != Some(path.as_path()) {
        cancel_kb_delete_confirm(ide);
        return true;
    }

    if matches!(key.code, KeyCode::Char('x')) {
        delete_selected_kb_entry(ide);
        return true;
    }

    let mut confirm = Confirm::new(format!("Delete {name}?"))
        .with_labels("Delete", "Cancel")
        .selected(ide.delete_confirm_yes)
        .danger();
    match confirm.handle_key(key) {
        Some(ConfirmMsg::Confirmed) => delete_selected_kb_entry(ide),
        Some(ConfirmMsg::Cancelled) => cancel_kb_delete_confirm(ide),
        None => ide.delete_confirm_yes = confirm.selected_yes(),
    }
    true
}

fn delete_selected_kb_entry(ide: &mut Ide) {
    let Some((sel, path, name)) = selected_kb_delete_target(ide) else {
        cancel_kb_delete_confirm(ide);
        return;
    };
    if ide.armed_delete.as_deref() != Some(path.as_path()) {
        cancel_kb_delete_confirm(ide);
        return;
    }

    ide.armed_delete = None;
    ide.delete_confirm_yes = true;
    let Some(root) = ide.kb_root.clone() else {
        return;
    };
    match spf::delete_within(&root, &path) {
        Ok(()) => {
            // Remove the entry row and visible subtree surgically; a full rescan
            // would collapse the tree and move the selection.
            let depth = ide.entries[sel].depth;
            let mut j = sel + 1;
            while j < ide.entries.len() && ide.entries[j].depth > depth {
                j += 1;
            }
            ide.entries.drain(sel..j);
            ide.sel = sel.min(ide.entries.len().saturating_sub(1));
            ide.preview = None;
            if ide.file.as_ref().is_some_and(|f| f.path.starts_with(&path)) {
                ide.file = None;
                ide.focus_editor = false;
            }
            ide.flash = Some(ide_flash_line(
                ToastKind::Success,
                format!("deleted {name}"),
            ));
        }
        Err(e) => {
            ide.flash = Some(ide_flash_line(
                ToastKind::Error,
                format!("delete failed: {e}"),
            ))
        }
    }
}

fn cancel_kb_delete_confirm(ide: &mut Ide) {
    ide.armed_delete = None;
    ide.delete_confirm_yes = true;
    ide.flash = None;
}

/// (Re)load the superfile-style hover preview for the tree selection. Cached
/// by path so moving within the same file costs nothing. Bounded I/O: at most
/// 64 KiB read (a hover must never stall the UI on a huge file), regular
/// files only (opening a FIFO would block forever), capped at 400 lines.
fn refresh_preview(ide: &mut Ide) {
    let Some(e) = ide.entries.get(ide.sel) else {
        ide.preview = None;
        return;
    };
    if e.is_dir {
        ide.preview = None;
        return;
    }
    if ide.preview.as_ref().is_some_and(|(p, _)| p == &e.path) {
        return;
    }
    let lines = if is_image_path(&e.path) {
        vec!["(image — Enter opens the preview)".into()]
    } else {
        preview_lines(&e.path)
    };
    ide.preview = Some((e.path.clone(), lines));
}

/// Read up to 64 KiB / 400 lines of a regular file for the hover preview.
fn preview_lines(path: &std::path::Path) -> Vec<String> {
    let md = match std::fs::metadata(path) {
        Ok(md) => md,
        Err(err) => return vec![format!("<cannot read: {err}>")],
    };
    if !md.is_file() {
        return vec!["(not a regular file)".into()];
    }
    use std::io::Read;
    let mut buf = vec![0u8; 64 * 1024];
    let n = match std::fs::File::open(path).and_then(|mut f| f.read(&mut buf)) {
        Ok(n) => n,
        Err(err) => return vec![format!("<cannot read: {err}>")],
    };
    let text = String::from_utf8_lossy(&buf[..n]);
    let mut lines: Vec<String> = text
        .replace('\t', "    ")
        .lines()
        .take(400)
        .map(String::from)
        .collect();
    if md.len() > n as u64 || lines.len() == 400 {
        lines.push("… (preview truncated — Enter opens the full file)".into());
    }
    lines
}

/// Vim-aligned editing for an open `IdeFile`. Normal mode navigates and operates;
/// Insert mode types. Arrows / Home / End / PgUp / PgDn work in both modes, and
/// Insert mode also honours the readline shortcuts traditional editors bind
/// (Ctrl+A/E/K/U/W). Read-only buffers allow navigation but block every edit.
impl IdeFile {
    /// Char count of the current line.
    fn cur_len(&self) -> usize {
        self.lines.get(self.row).map_or(0, |l| l.chars().count())
    }

    /// Clamp the cursor column into the current line. Normal mode rests *on* the
    /// last char (col ≤ len-1); Insert may sit *after* it (col ≤ len).
    fn clamp_col(&mut self) {
        let len = self.cur_len();
        let max = if self.mode == EditMode::Insert {
            len
        } else {
            len.saturating_sub(1)
        };
        self.col = self.col.min(max);
    }

    /// Column of the first non-blank char on the current line (vim `^`).
    fn first_nonblank(&self) -> usize {
        self.lines.get(self.row).map_or(0, |l| {
            l.chars().position(|c| !c.is_whitespace()).unwrap_or(0)
        })
    }

    /// vim word classes: 0 = whitespace, 1 = word char, 2 = punctuation.
    fn char_class(c: char) -> u8 {
        if c.is_whitespace() {
            0
        } else if c.is_alphanumeric() || c == '_' {
            1
        } else {
            2
        }
    }

    fn row_chars(&self) -> Vec<char> {
        self.lines
            .get(self.row)
            .map_or(Vec::new(), |l| l.chars().collect())
    }

    /// Start of the next word on this line (vim `w`, within-line).
    fn next_word(&self) -> usize {
        let line = self.row_chars();
        let n = line.len();
        let mut c = self.col;
        if c >= n {
            return n;
        }
        let cls = Self::char_class(line[c]);
        if cls != 0 {
            while c < n && Self::char_class(line[c]) == cls {
                c += 1;
            }
        }
        while c < n && Self::char_class(line[c]) == 0 {
            c += 1;
        }
        c
    }

    /// Start of the previous word on this line (vim `b`, within-line).
    fn prev_word(&self) -> usize {
        let line = self.row_chars();
        let mut c = self.col;
        if c == 0 {
            return 0;
        }
        c -= 1;
        while c > 0 && Self::char_class(line[c]) == 0 {
            c -= 1;
        }
        let cls = Self::char_class(line[c]);
        while c > 0 && Self::char_class(line[c - 1]) == cls {
            c -= 1;
        }
        c
    }

    /// End of the current/next word on this line (vim `e`, within-line).
    fn word_end(&self) -> usize {
        let line = self.row_chars();
        let n = line.len();
        if n == 0 {
            return 0;
        }
        let mut c = self.col;
        if c + 1 >= n {
            return n - 1;
        }
        c += 1;
        while c < n && Self::char_class(line[c]) == 0 {
            c += 1;
        }
        if c >= n {
            return n - 1;
        }
        let cls = Self::char_class(line[c]);
        while c + 1 < n && Self::char_class(line[c + 1]) == cls {
            c += 1;
        }
        c
    }

    /// Leading whitespace of the current line (for `o`/`O` auto-indent).
    fn leading_ws(&self) -> String {
        self.lines.get(self.row).map_or(String::new(), |l| {
            l.chars().take_while(|c| c.is_whitespace()).collect()
        })
    }

    /// Snapshot the buffer + cursor for `u`.
    // ponytail: whole-buffer snapshots, bounded — fine for config-sized files
    fn snapshot(&mut self) {
        if self.undo.len() >= 200 {
            self.undo.remove(0);
        }
        self.undo.push((self.lines.clone(), self.row, self.col));
        self.redo.clear();
    }

    fn undo(&mut self) {
        if let Some((lines, row, col)) = self.undo.pop() {
            if self.redo.len() >= 200 {
                self.redo.remove(0);
            }
            self.redo.push((self.lines.clone(), self.row, self.col));
            self.lines = lines;
            self.row = row.min(self.lines.len().saturating_sub(1));
            self.col = col;
            self.dirty = true;
            self.clamp_col();
        }
    }

    fn redo(&mut self) {
        if let Some((lines, row, col)) = self.redo.pop() {
            if self.undo.len() >= 200 {
                self.undo.remove(0);
            }
            self.undo.push((self.lines.clone(), self.row, self.col));
            self.lines = lines;
            self.row = row.min(self.lines.len().saturating_sub(1));
            self.col = col;
            self.dirty = true;
            self.clamp_col();
        }
    }

    fn take_count_with_flag(&mut self) -> (usize, bool) {
        match self.count.take() {
            Some(count) => (count.max(1), true),
            None => (1, false),
        }
    }

    fn push_count_digit(&mut self, c: char) -> bool {
        let Some(digit) = c.to_digit(10).map(|d| d as usize) else {
            return false;
        };
        if digit == 0 && self.count.is_none() {
            return false;
        }
        let next = self
            .count
            .unwrap_or(0)
            .saturating_mul(10)
            .saturating_add(digit)
            .max(1);
        self.count = Some(next.min(9999));
        true
    }

    fn move_word_forward(&mut self, count: usize) {
        for _ in 0..count {
            self.col = self.next_word();
        }
    }

    fn move_word_backward(&mut self, count: usize) {
        for _ in 0..count {
            self.col = self.prev_word();
        }
    }

    fn move_word_end(&mut self, count: usize) {
        for _ in 0..count {
            self.col = self.word_end();
        }
    }

    /// Handle one key in the focused editor. Ctrl+S (save) is handled by the
    /// caller before this; Esc (Insert→Normal / leave) likewise.
    pub(crate) fn edit_key(&mut self, key: &KeyEvent, body: usize, content_w: usize) {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
        let nlines = self.lines.len();
        let mut handled = false;
        if ctrl {
            handled = true;
            match key.code {
                KeyCode::Char('a' | 'A') => self.col = 0,
                KeyCode::Char('e' | 'E') => self.col = self.cur_len(),
                KeyCode::Char('z' | 'Z') if !self.readonly => self.undo(),
                KeyCode::Char('r' | 'R') if !self.readonly => self.redo(),
                _ => handled = false,
            }
        }
        // A pending Normal-mode operator (d/c/g/y) consumes the next key — even an
        // arrow — instead of the shared navigation below.
        let pending = self.mode == EditMode::Normal && self.pending.is_some();
        if !handled && !pending {
            handled = true;
            match key.code {
                KeyCode::Up => self.row = self.row.saturating_sub(1),
                KeyCode::Down => self.row = (self.row + 1).min(nlines.saturating_sub(1)),
                KeyCode::Left => self.move_left(),
                KeyCode::Right => self.move_right(),
                KeyCode::Home => self.col = 0,
                KeyCode::End => self.col = self.cur_len(),
                KeyCode::PageUp => self.row = self.row.saturating_sub(body),
                KeyCode::PageDown => self.row = (self.row + body).min(nlines.saturating_sub(1)),
                _ => handled = false,
            }
        }
        if !handled {
            match self.mode {
                EditMode::Insert => self.insert_key(key, ctrl),
                EditMode::Normal => self.normal_key(key),
            }
        }
        // Clamp the cursor + scroll it into view, vertically AND horizontally.
        self.clamp_col();
        if self.row < self.scroll {
            self.scroll = self.row;
        } else if body > 0 && self.row >= self.scroll + body {
            self.scroll = self.row + 1 - body;
        }
        // Horizontal: keep the cursor's display column within the content width,
        // so long lines scroll sideways instead of being truncated off-screen.
        let cur_x = self.display_col();
        if cur_x < self.hscroll {
            self.hscroll = cur_x;
        } else if content_w > 0 && cur_x >= self.hscroll + content_w {
            self.hscroll = cur_x + 1 - content_w;
        }
    }

    /// The cursor's display column (sum of glyph widths before `col`), so
    /// horizontal scrolling and the block cursor land correctly with wide (CJK)
    /// chars, not just ASCII.
    pub(crate) fn display_col(&self) -> usize {
        self.lines.get(self.row).map_or(0, |l| {
            l.chars()
                .take(self.col)
                .map(|c| a3s_tui::style::visible_len(&c.to_string()))
                .sum()
        })
    }

    fn move_left(&mut self) {
        if self.col > 0 {
            self.col -= 1;
        } else if self.row > 0 {
            self.row -= 1;
            self.col = self.cur_len();
        }
    }

    fn move_right(&mut self) {
        let len = self.cur_len();
        if self.col < len {
            self.col += 1;
        } else if self.row + 1 < self.lines.len() {
            self.row += 1;
            self.col = 0;
        }
    }

    // ── Insert mode ──────────────────────────────────────────────────────────
    fn insert_key(&mut self, key: &KeyEvent, ctrl: bool) {
        if self.readonly {
            return;
        }
        if ctrl {
            // readline-style shortcuts traditional editors bind in insert mode.
            match key.code {
                KeyCode::Char('a' | 'A') => self.col = 0,
                KeyCode::Char('e' | 'E') => self.col = self.cur_len(),
                KeyCode::Char('k' | 'K') => {
                    self.snapshot();
                    self.kill_to_eol();
                }
                KeyCode::Char('u' | 'U') => {
                    self.snapshot();
                    self.kill_to_bol();
                }
                KeyCode::Char('w' | 'W') => {
                    self.snapshot();
                    self.delete_word_back();
                }
                KeyCode::Char('d' | 'D') => {
                    self.snapshot();
                    self.delete_forward();
                }
                KeyCode::Char('h' | 'H') => {
                    self.snapshot();
                    self.backspace();
                }
                KeyCode::Char('z' | 'Z') => self.undo(),
                KeyCode::Char('r' | 'R') => self.redo(),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Char(c) => self.insert_char(c),
            KeyCode::Tab => self.insert_str("    "),
            KeyCode::Enter => self.split_line(),
            KeyCode::Backspace => self.backspace(),
            KeyCode::Delete => self.delete_forward(),
            _ => {}
        }
    }

    fn insert_char(&mut self, c: char) {
        let b = char_byte(&self.lines[self.row], self.col);
        self.lines[self.row].insert(b, c);
        self.col += 1;
        self.dirty = true;
    }
    fn insert_str(&mut self, s: &str) {
        let b = char_byte(&self.lines[self.row], self.col);
        self.lines[self.row].insert_str(b, s);
        self.col += s.chars().count();
        self.dirty = true;
    }
    fn paste_external_text(&mut self, text: &str) -> bool {
        if self.readonly || self.image {
            return false;
        }
        let normalized = normalize_pasted_text(text);
        if normalized.is_empty() {
            return false;
        }
        self.snapshot();
        self.insert_text(&normalized);
        self.mode = EditMode::Insert;
        true
    }
    fn insert_text(&mut self, text: &str) {
        let parts: Vec<&str> = text.split('\n').collect();
        if parts.len() == 1 {
            self.insert_str(parts[0]);
            return;
        }

        let b = char_byte(&self.lines[self.row], self.col);
        let tail = self.lines[self.row].split_off(b);
        self.lines[self.row].push_str(parts[0]);
        let last_idx = parts.len() - 1;
        for part in &parts[1..last_idx] {
            self.row += 1;
            self.lines.insert(self.row, (*part).to_string());
        }
        self.row += 1;
        self.lines
            .insert(self.row, format!("{}{}", parts[last_idx], tail));
        self.col = parts[last_idx].chars().count();
        self.dirty = true;
    }
    fn split_line(&mut self) {
        let b = char_byte(&self.lines[self.row], self.col);
        let right = self.lines[self.row].split_off(b);
        self.lines.insert(self.row + 1, right);
        self.row += 1;
        self.col = 0;
        self.dirty = true;
    }
    fn backspace(&mut self) {
        if self.col > 0 {
            let b0 = char_byte(&self.lines[self.row], self.col - 1);
            let b1 = char_byte(&self.lines[self.row], self.col);
            self.lines[self.row].replace_range(b0..b1, "");
            self.col -= 1;
            self.dirty = true;
        } else if self.row > 0 {
            let cur = self.lines.remove(self.row);
            self.row -= 1;
            self.col = self.cur_len();
            self.lines[self.row].push_str(&cur);
            self.dirty = true;
        }
    }
    fn delete_forward(&mut self) {
        let len = self.cur_len();
        if self.col < len {
            let b0 = char_byte(&self.lines[self.row], self.col);
            let b1 = char_byte(&self.lines[self.row], self.col + 1);
            self.lines[self.row].replace_range(b0..b1, "");
            self.dirty = true;
        } else if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            self.lines[self.row].push_str(&next);
            self.dirty = true;
        }
    }
    fn kill_to_eol(&mut self) {
        let b = char_byte(&self.lines[self.row], self.col);
        if !self.lines[self.row].split_off(b).is_empty() {
            self.dirty = true;
        }
    }
    fn kill_to_bol(&mut self) {
        let b = char_byte(&self.lines[self.row], self.col);
        self.lines[self.row].replace_range(0..b, "");
        self.col = 0;
        self.dirty = true;
    }
    fn delete_word_back(&mut self) {
        let start = self.prev_word();
        let from = if start < self.col {
            start
        } else {
            self.col.saturating_sub(1)
        };
        if from < self.col {
            let b0 = char_byte(&self.lines[self.row], from);
            let b1 = char_byte(&self.lines[self.row], self.col);
            self.lines[self.row].replace_range(b0..b1, "");
            self.col = from;
            self.dirty = true;
        }
    }

    // ── Normal mode ──────────────────────────────────────────────────────────
    fn normal_key(&mut self, key: &KeyEvent) {
        if let Some(op) = self.pending.take() {
            self.apply_operator(op, key);
            return;
        }
        let ch = match key.code {
            KeyCode::Char(c) => c,
            _ => return,
        };
        if self.push_count_digit(ch) {
            return;
        }
        if self.visual_line_anchor.is_some() {
            self.visual_line_key(ch);
            return;
        }
        let ro = self.readonly;
        let (count, counted) = self.take_count_with_flag();
        match ch {
            // motions
            'h' => self.col = self.col.saturating_sub(count),
            'l' => {
                self.col = (self.col + count).min(self.cur_len().saturating_sub(1));
            }
            'j' => self.row = (self.row + count).min(self.lines.len().saturating_sub(1)),
            'k' => self.row = self.row.saturating_sub(count),
            'w' => self.move_word_forward(count),
            'b' => self.move_word_backward(count),
            'e' => self.move_word_end(count),
            '0' => self.col = 0,
            '^' => self.col = self.first_nonblank(),
            '$' => self.col = self.cur_len().saturating_sub(1),
            'G' => {
                self.row = if counted {
                    count
                        .saturating_sub(1)
                        .min(self.lines.len().saturating_sub(1))
                } else {
                    self.lines.len().saturating_sub(1)
                }
            }
            'n' => self.repeat_search(false),
            'N' => self.repeat_search(true),
            'V' => self.visual_line_anchor = Some(self.row),
            // operator / prefix — wait for the second key (g/d/c/y; r = replace one char)
            'g' | 'd' | 'c' | 'y' => self.pending = Some(PendingOp { op: ch, count }),
            'r' if !ro => self.pending = Some(PendingOp { op: 'r', count }),
            // inline edits
            'x' if !ro => {
                self.snapshot();
                self.delete_chars_under(count);
                self.last_change = Some(RepeatEdit::DeleteChar(count));
            }
            'J' if !ro => {
                self.snapshot();
                for _ in 0..count {
                    self.join_line();
                }
                self.last_change = Some(RepeatEdit::JoinLine(count));
            }
            '~' if !ro => {
                self.snapshot();
                for _ in 0..count {
                    self.toggle_case();
                }
                self.last_change = Some(RepeatEdit::ToggleCase(count));
            }
            'D' if !ro => {
                self.snapshot();
                self.delete_to_eol();
                self.last_change = Some(RepeatEdit::DeleteToEol);
            }
            'C' if !ro => {
                self.snapshot();
                self.delete_to_eol();
                self.mode = EditMode::Insert;
                self.last_change = Some(RepeatEdit::DeleteToEol);
            }
            'p' if !ro => {
                self.snapshot();
                for _ in 0..count {
                    self.paste(true);
                }
            }
            'P' if !ro => {
                self.snapshot();
                for _ in 0..count {
                    self.paste(false);
                }
            }
            'u' if !ro => self.undo(),
            '.' if !ro => self.repeat_last_change(),
            // enter insert
            'i' if !ro => {
                self.snapshot();
                self.mode = EditMode::Insert;
            }
            'a' if !ro => {
                self.snapshot();
                self.col = (self.col + 1).min(self.cur_len());
                self.mode = EditMode::Insert;
            }
            'I' if !ro => {
                self.snapshot();
                self.col = self.first_nonblank();
                self.mode = EditMode::Insert;
            }
            'A' if !ro => {
                self.snapshot();
                self.col = self.cur_len();
                self.mode = EditMode::Insert;
            }
            'o' if !ro => {
                self.snapshot();
                self.open_line(true);
            }
            'O' if !ro => {
                self.snapshot();
                self.open_line(false);
            }
            _ => {}
        }
    }

    fn visual_line_key(&mut self, ch: char) {
        let ro = self.readonly;
        match ch {
            'j' => self.row = (self.row + 1).min(self.lines.len().saturating_sub(1)),
            'k' => self.row = self.row.saturating_sub(1),
            'G' => self.row = self.lines.len().saturating_sub(1),
            'g' => self.pending = Some(PendingOp { op: 'g', count: 1 }),
            'V' | '\u{1b}' => self.visual_line_anchor = None,
            'y' => {
                self.yank_visual_lines();
                self.visual_line_anchor = None;
            }
            'd' | 'x' if !ro => {
                self.snapshot();
                let count = self.delete_visual_lines();
                self.visual_line_anchor = None;
                self.last_change = Some(RepeatEdit::DeleteLine(count));
            }
            _ => {}
        }
    }

    fn visual_range(&self) -> Option<(usize, usize)> {
        let anchor = self.visual_line_anchor?;
        Some((anchor.min(self.row), anchor.max(self.row)))
    }

    fn yank_visual_lines(&mut self) {
        let Some((lo, hi)) = self.visual_range() else {
            return;
        };
        self.clip = self.lines[lo..=hi].join("\n");
        self.clip_linewise = true;
    }

    fn delete_visual_lines(&mut self) -> usize {
        let Some((lo, hi)) = self.visual_range() else {
            return 0;
        };
        self.clip = self.lines[lo..=hi].join("\n");
        self.clip_linewise = true;
        let count = hi - lo + 1;
        if self.lines.len() == count {
            self.lines.clear();
            self.lines.push(String::new());
            self.row = 0;
        } else {
            self.lines.drain(lo..=hi);
            self.row = lo.min(self.lines.len().saturating_sub(1));
        }
        self.col = 0;
        self.dirty = true;
        count
    }

    fn apply_search(&mut self, query: &str, forward: bool) -> bool {
        if query.is_empty() {
            return false;
        }
        self.search = Some((query.to_string(), forward));
        self.find_query(query, forward)
    }

    fn repeat_search(&mut self, reverse: bool) {
        let Some((query, forward)) = self.search.clone() else {
            return;
        };
        let direction = if reverse { !forward } else { forward };
        let _ = self.find_query(&query, direction);
    }

    fn find_query(&mut self, query: &str, forward: bool) -> bool {
        if self.lines.is_empty() || query.is_empty() {
            return false;
        }
        let n = self.lines.len();
        for step in 1..=n {
            let idx = if forward {
                (self.row + step) % n
            } else {
                (self.row + n - (step % n)) % n
            };
            if let Some(byte) = self.lines[idx].find(query) {
                self.row = idx;
                self.col = self.lines[idx][..byte].chars().count();
                return true;
            }
        }
        false
    }

    fn repeat_last_change(&mut self) {
        let Some(edit) = self.last_change.clone() else {
            return;
        };
        self.snapshot();
        match edit {
            RepeatEdit::DeleteChar(count) => self.delete_chars_under(count),
            RepeatEdit::DeleteLine(count) => self.delete_lines(count, true),
            RepeatEdit::DeleteWord(count) => {
                for _ in 0..count {
                    self.delete_word();
                }
            }
            RepeatEdit::DeleteToEol => self.delete_to_eol(),
            RepeatEdit::ChangeLine(count) => {
                self.change_lines(count);
                self.mode = EditMode::Insert;
            }
            RepeatEdit::Replace(c) => self.replace_char(c),
            RepeatEdit::JoinLine(count) => {
                for _ in 0..count {
                    self.join_line();
                }
            }
            RepeatEdit::ToggleCase(count) => {
                for _ in 0..count {
                    self.toggle_case();
                }
            }
        }
    }

    /// Second key of a two-stroke Normal command (the operator/prefix is `op`).
    fn apply_operator(&mut self, pending: PendingOp, key: &KeyEvent) {
        let ch = match key.code {
            KeyCode::Char(c) => c,
            _ => return, // arrow/other after an operator just cancels it
        };
        let ro = self.readonly;
        match (pending.op, ch) {
            ('g', 'g') => {
                self.row = pending
                    .count
                    .saturating_sub(1)
                    .min(self.lines.len().saturating_sub(1))
            }
            ('d', 'd') if !ro => {
                self.snapshot();
                self.delete_lines(pending.count, true);
                self.last_change = Some(RepeatEdit::DeleteLine(pending.count));
            }
            ('d', 'w') if !ro => {
                self.snapshot();
                for _ in 0..pending.count {
                    self.delete_word();
                }
                self.last_change = Some(RepeatEdit::DeleteWord(pending.count));
            }
            ('d', '$') if !ro => {
                self.snapshot();
                self.delete_to_eol();
                self.last_change = Some(RepeatEdit::DeleteToEol);
            }
            ('c', 'c') if !ro => {
                self.snapshot();
                self.change_lines(pending.count);
                self.mode = EditMode::Insert;
                self.last_change = Some(RepeatEdit::ChangeLine(pending.count));
            }
            ('c', 'w') if !ro => {
                self.snapshot();
                for _ in 0..pending.count {
                    self.delete_word();
                }
                self.mode = EditMode::Insert;
                self.last_change = Some(RepeatEdit::DeleteWord(pending.count));
            }
            ('c', '$') if !ro => {
                self.snapshot();
                self.delete_to_eol();
                self.mode = EditMode::Insert;
                self.last_change = Some(RepeatEdit::DeleteToEol);
            }
            ('y', 'y') => self.yank_lines(pending.count),
            // `r<char>` — replace the char under the cursor in place.
            ('r', c) if !ro => {
                self.snapshot();
                self.replace_char(c);
                self.last_change = Some(RepeatEdit::Replace(c));
            }
            _ => {}
        }
    }

    /// `r<char>` — replace the glyph under the cursor (vim, stays in Normal).
    fn replace_char(&mut self, ch: char) {
        let len = self.cur_len();
        if self.col < len {
            let b0 = char_byte(&self.lines[self.row], self.col);
            let b1 = char_byte(&self.lines[self.row], self.col + 1);
            self.lines[self.row].replace_range(b0..b1, &ch.to_string());
            self.dirty = true;
        }
    }

    /// `J` — join the next line onto this one with a single separating space
    /// (cursor lands at the join, vim-style).
    fn join_line(&mut self) {
        if self.row + 1 < self.lines.len() {
            let next = self.lines.remove(self.row + 1);
            let cur = &mut self.lines[self.row];
            self.col = cur.chars().count();
            if !cur.is_empty() && !next.trim_start().is_empty() {
                cur.push(' ');
            }
            cur.push_str(next.trim_start());
            self.dirty = true;
        }
    }

    /// `~` — toggle the case of the glyph under the cursor and advance.
    fn toggle_case(&mut self) {
        let len = self.cur_len();
        if self.col < len {
            if let Some(ch) = self.lines[self.row].chars().nth(self.col) {
                let flipped: String = if ch.is_uppercase() {
                    ch.to_lowercase().collect()
                } else {
                    ch.to_uppercase().collect()
                };
                let b0 = char_byte(&self.lines[self.row], self.col);
                let b1 = char_byte(&self.lines[self.row], self.col + 1);
                self.lines[self.row].replace_range(b0..b1, &flipped);
                self.col = (self.col + 1).min(self.cur_len().saturating_sub(1));
                self.dirty = true;
            }
        }
    }

    fn delete_chars_under(&mut self, count: usize) {
        self.clip.clear();
        self.clip_linewise = false;
        for _ in 0..count.max(1) {
            let len = self.cur_len();
            if self.col < len {
                let b0 = char_byte(&self.lines[self.row], self.col);
                let b1 = char_byte(&self.lines[self.row], self.col + 1);
                self.clip.push_str(&self.lines[self.row][b0..b1]);
                self.lines[self.row].replace_range(b0..b1, "");
                self.dirty = true;
            }
        }
    }
    fn delete_to_eol(&mut self) {
        let b = char_byte(&self.lines[self.row], self.col);
        let removed = self.lines[self.row].split_off(b);
        if !removed.is_empty() {
            self.clip = removed;
            self.clip_linewise = false;
            self.dirty = true;
        }
    }
    fn delete_lines(&mut self, count: usize, yank: bool) {
        let count = count.max(1).min(self.lines.len().saturating_sub(self.row));
        if count == 0 {
            return;
        }
        if yank {
            self.clip = self.lines[self.row..self.row + count].join("\n");
            self.clip_linewise = true;
        }
        if self.lines.len() == count {
            self.lines.clear();
            self.lines.push(String::new());
            self.row = 0;
        } else {
            self.lines.drain(self.row..self.row + count);
            if self.row >= self.lines.len() {
                self.row = self.lines.len() - 1;
            }
        }
        self.col = 0;
        self.dirty = true;
    }
    fn delete_word(&mut self) {
        let to = self.next_word().min(self.cur_len());
        if to > self.col {
            let b0 = char_byte(&self.lines[self.row], self.col);
            let b1 = char_byte(&self.lines[self.row], to);
            self.clip = self.lines[self.row][b0..b1].to_string();
            self.clip_linewise = false;
            self.lines[self.row].replace_range(b0..b1, "");
            self.dirty = true;
        }
    }
    fn change_lines(&mut self, count: usize) {
        let count = count.max(1).min(self.lines.len().saturating_sub(self.row));
        if count == 0 {
            return;
        }
        self.clip = self.lines[self.row..self.row + count].join("\n");
        self.clip_linewise = true;
        self.lines
            .splice(self.row..self.row + count, std::iter::once(String::new()));
        self.col = 0;
        self.dirty = true;
    }
    fn yank_lines(&mut self, count: usize) {
        let count = count.max(1).min(self.lines.len().saturating_sub(self.row));
        if count == 0 {
            return;
        }
        self.clip = self.lines[self.row..self.row + count].join("\n");
        self.clip_linewise = true;
    }
    fn paste(&mut self, after: bool) {
        if self.clip.is_empty() && !self.clip_linewise {
            return;
        }
        if self.clip_linewise {
            let at = if after { self.row + 1 } else { self.row };
            self.lines.insert(at, self.clip.clone());
            self.row = at;
            self.col = 0;
        } else {
            let col = if after && self.cur_len() > 0 {
                (self.col + 1).min(self.cur_len())
            } else {
                self.col
            };
            let b = char_byte(&self.lines[self.row], col);
            self.lines[self.row].insert_str(b, &self.clip);
            self.col = col + self.clip.chars().count().saturating_sub(1);
        }
        self.dirty = true;
    }
    fn open_line(&mut self, below: bool) {
        let indent = self.leading_ws();
        let at = if below { self.row + 1 } else { self.row };
        self.lines.insert(at, indent);
        self.row = at;
        self.col = self.cur_len();
        self.mode = EditMode::Insert;
        self.dirty = true;
    }
}

/// Render an editor line's visible window in plain text with a block cursor at
/// `cursor_col` (display columns; a space when the cursor sits past end-of-line).
/// Plain — no syntax colour — so the inverse cursor cell is unambiguous on the
/// active line.
fn render_cursor_line(window: &str, cursor_col: usize) -> String {
    CursorLine::new(window)
        .cursor_col(cursor_col)
        .cursor_style(Style::new().fg(Color::Black).bg(TN_FG))
        .view()
}

#[cfg(test)]
mod vim_tests {
    use super::*;

    fn buf(lines: &[&str]) -> IdeFile {
        IdeFile::new(
            std::path::PathBuf::from("t"),
            lines.iter().map(|s| s.to_string()).collect(),
            false,
            false,
        )
    }
    fn ro(lines: &[&str]) -> IdeFile {
        IdeFile::new(
            std::path::PathBuf::from("t"),
            lines.iter().map(|s| s.to_string()).collect(),
            false,
            true,
        )
    }
    fn k(c: char) -> KeyEvent {
        KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::NONE,
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent {
            code,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn temp_root(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "a3s-ide-{name}-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    fn kb_ide_with_file(root: &std::path::Path, name: &str) -> (Ide, std::path::PathBuf) {
        let path = root.join(name);
        std::fs::write(&path, "note").unwrap();
        let mut ide = Ide::browse(
            vec![IdeEntry {
                path: path.clone(),
                name: name.to_string(),
                depth: 0,
                is_dir: false,
                expanded: false,
            }],
            "knowledge base",
        );
        ide.kb_root = Some(root.to_path_buf());
        ide.armed_delete = Some(path.clone());
        (ide, path)
    }

    /// Feed a sequence of plain Char keys (covers multi-key ops like dd/gg/dw).
    fn feed(f: &mut IdeFile, s: &str) {
        for c in s.chars() {
            f.edit_key(&k(c), 20, 80);
        }
    }

    #[test]
    fn opens_in_normal_and_navigates_without_typing() {
        let mut f = buf(&["foo", "bar"]);
        assert_eq!(f.mode, EditMode::Normal);
        feed(&mut f, "j");
        assert_eq!(f.row, 1);
        feed(&mut f, "$");
        assert_eq!(f.col, 2); // last char of "bar"
        feed(&mut f, "0");
        assert_eq!(f.col, 0);
        assert_eq!(f.lines, vec!["foo", "bar"]); // motions never inserted
    }

    #[test]
    fn word_motions_w_e_b() {
        let mut f = buf(&["foo bar baz"]);
        feed(&mut f, "w");
        assert_eq!(f.col, 4); // start of "bar"
        feed(&mut f, "e");
        assert_eq!(f.col, 6); // end of "bar"
        feed(&mut f, "b");
        assert_eq!(f.col, 4); // back to start of "bar"
    }

    #[test]
    fn gg_and_capital_g_jump() {
        let mut f = buf(&["a", "b", "c"]);
        feed(&mut f, "G");
        assert_eq!(f.row, 2);
        feed(&mut f, "gg");
        assert_eq!(f.row, 0);
        feed(&mut f, "G");
        assert_eq!(f.row, 2);
        feed(&mut f, "1G");
        assert_eq!(f.row, 0);
    }

    #[test]
    fn count_prefix_repeats_motion_and_delete_line() {
        let mut f = buf(&["a", "b", "c", "d"]);

        feed(&mut f, "3j");
        assert_eq!(f.row, 3);
        feed(&mut f, "2k");
        assert_eq!(f.row, 1);

        feed(&mut f, "2dd");
        assert_eq!(f.lines, vec!["a", "d"]);
        assert_eq!(f.row, 1);
        assert_eq!(f.clip, "b\nc");
        assert!(f.clip_linewise);
    }

    #[test]
    fn dot_repeats_last_normal_change() {
        let mut f = buf(&["abc", "def"]);

        feed(&mut f, "x");
        assert_eq!(f.lines[0], "bc");
        feed(&mut f, ".");
        assert_eq!(f.lines[0], "c");

        let mut f = buf(&["one", "two", "three"]);
        feed(&mut f, "dd");
        feed(&mut f, ".");
        assert_eq!(f.lines, vec!["three"]);
    }

    #[test]
    fn ctrl_r_redoes_undo() {
        let mut f = buf(&["abc"]);

        feed(&mut f, "x");
        assert_eq!(f.lines[0], "bc");
        feed(&mut f, "u");
        assert_eq!(f.lines[0], "abc");
        f.edit_key(
            &KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::CONTROL,
            },
            20,
            80,
        );

        assert_eq!(f.lines[0], "bc");
    }

    #[test]
    fn search_and_repeat_search_wrap() {
        let mut f = buf(&["alpha", "beta", "gamma beta"]);

        assert!(f.apply_search("beta", true));
        assert_eq!(f.row, 1);
        feed(&mut f, "n");
        assert_eq!(f.row, 2);
        feed(&mut f, "N");
        assert_eq!(f.row, 1);

        assert!(!f.apply_search("missing", true));
    }

    #[test]
    fn visual_line_yanks_and_deletes_range() {
        let mut f = buf(&["one", "two", "three", "four"]);

        feed(&mut f, "Vj");
        assert_eq!(f.visual_line_anchor, Some(0));
        feed(&mut f, "y");
        assert_eq!(f.clip, "one\ntwo");
        assert!(f.visual_line_anchor.is_none());

        feed(&mut f, "ggVjd");
        assert_eq!(f.lines, vec!["three", "four"]);
        assert_eq!(f.clip, "one\ntwo");
        assert!(f.dirty);
    }

    #[test]
    fn insert_mode_types_literally() {
        let mut f = buf(&["bc"]);
        feed(&mut f, "i");
        assert_eq!(f.mode, EditMode::Insert);
        feed(&mut f, "a"); // literal char in Insert, not "append"
        assert_eq!(f.lines[0], "abc");
        assert_eq!(f.col, 1);
    }

    #[test]
    fn append_at_end_of_line() {
        let mut f = buf(&["ab"]);
        feed(&mut f, "A");
        assert_eq!(f.mode, EditMode::Insert);
        assert_eq!(f.col, 2);
        feed(&mut f, "c");
        assert_eq!(f.lines[0], "abc");
    }

    #[test]
    fn external_paste_inserts_multiline_text_at_cursor() {
        let mut f = buf(&["ab"]);
        f.col = 1;

        assert!(f.paste_external_text("X\nY\tZ\r\n"));

        assert_eq!(f.lines, vec!["aX", "Y    Z", "b"]);
        assert_eq!(f.row, 2);
        assert_eq!(f.col, 0);
        assert_eq!(f.mode, EditMode::Insert);
        assert!(f.dirty);
    }

    #[test]
    fn external_paste_is_one_undo_step() {
        let mut f = buf(&["ab"]);
        f.col = 1;

        assert!(f.paste_external_text("XYZ"));
        assert_eq!(f.lines[0], "aXYZb");
        f.edit_key(
            &KeyEvent {
                code: KeyCode::Char('z'),
                modifiers: KeyModifiers::CONTROL,
            },
            20,
            80,
        );

        assert_eq!(f.lines, vec!["ab"]);
        assert_eq!(f.col, 1);
    }

    #[test]
    fn external_paste_respects_readonly_buffers() {
        let mut f = ro(&["ab"]);

        assert!(!f.paste_external_text("XYZ"));
        assert_eq!(f.lines, vec!["ab"]);
        assert!(!f.dirty);
    }

    #[test]
    fn o_opens_indented_line_below_in_insert() {
        let mut f = buf(&["  foo"]);
        feed(&mut f, "o");
        assert_eq!(f.mode, EditMode::Insert);
        assert_eq!(f.row, 1);
        assert_eq!(f.lines[1], "  "); // indent carried
        feed(&mut f, "x");
        assert_eq!(f.lines[1], "  x");
    }

    #[test]
    fn x_deletes_char_under_cursor() {
        let mut f = buf(&["abc"]);
        feed(&mut f, "x");
        assert_eq!(f.lines[0], "bc");
        assert_eq!(f.col, 0);
    }

    #[test]
    fn dw_deletes_word() {
        let mut f = buf(&["foo bar"]);
        feed(&mut f, "dw");
        assert_eq!(f.lines[0], "bar");
    }

    #[test]
    fn dd_then_p_moves_a_line() {
        let mut f = buf(&["one", "two", "three"]);
        feed(&mut f, "dd");
        assert_eq!(f.lines, vec!["two", "three"]);
        feed(&mut f, "p"); // paste "one" below current line
        assert_eq!(f.lines, vec!["two", "one", "three"]);
    }

    #[test]
    fn u_undoes_a_change() {
        let mut f = buf(&["abc"]);
        feed(&mut f, "x");
        assert_eq!(f.lines[0], "bc");
        feed(&mut f, "u");
        assert_eq!(f.lines[0], "abc");
    }

    #[test]
    fn ctrl_w_kills_word_back_in_insert() {
        let mut f = buf(&["foo bar"]);
        feed(&mut f, "A"); // insert mode at EOL
        f.edit_key(
            &KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::CONTROL,
            },
            20,
            80,
        );
        assert_eq!(f.lines[0], "foo ");
    }

    #[test]
    fn ctrl_d_and_ctrl_h_delete_in_insert() {
        let mut f = buf(&["abc"]);
        f.mode = EditMode::Insert;
        f.col = 1;

        f.edit_key(
            &KeyEvent {
                code: KeyCode::Char('d'),
                modifiers: KeyModifiers::CONTROL,
            },
            20,
            80,
        );
        assert_eq!(f.lines[0], "ac");
        assert_eq!(f.col, 1);

        f.edit_key(
            &KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::CONTROL,
            },
            20,
            80,
        );
        assert_eq!(f.lines[0], "c");
        assert_eq!(f.col, 0);
    }

    #[test]
    fn readonly_navigates_but_blocks_edits() {
        let mut f = ro(&["abc", "def"]);
        feed(&mut f, "j");
        assert_eq!(f.row, 1);
        feed(&mut f, "x"); // edit blocked
        assert_eq!(f.lines, vec!["abc", "def"]);
        feed(&mut f, "i"); // can't enter insert on a read-only buffer
        assert_eq!(f.mode, EditMode::Normal);
    }

    #[tokio::test]
    async fn preview_command_saves_the_open_buffer_and_queues_its_path() {
        let root = temp_root("live-preview-command");
        let path = root.join("index.html");
        std::fs::write(&path, "old").unwrap();
        let manifest = LocalWorkspaceManifest::start(&root);
        let mut ide = Ide::workspace(Vec::new());
        let mut file = IdeFile::new(path.clone(), vec!["new".to_string()], false, false);
        file.dirty = true;
        ide.file = Some(file);

        let result = apply_ide_command(
            &mut ide,
            "preview",
            &manifest,
            root.to_string_lossy().as_ref(),
        );

        assert_eq!(ide.preview_request.as_deref(), Some(path.as_path()));
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "new\n");
        assert!(!ide.file.as_ref().unwrap().dirty);
        assert!(a3s_tui::style::strip_ansi(&result).contains("opening live preview"));
        drop(manifest);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn horizontal_scroll_follows_the_cursor() {
        let mut f = buf(&["abcdefghijklmnopqrst"]); // 20 cols
                                                    // content width 8 → moving to EOL (col 19) scrolls the window right.
        f.edit_key(&k('$'), 20, 8);
        assert_eq!(f.col, 19);
        assert_eq!(f.hscroll, 12); // 19 + 1 - 8
                                   // back to col 0 → scrolled fully left again.
        f.edit_key(&k('0'), 20, 8);
        assert_eq!(f.hscroll, 0);
    }

    #[test]
    fn render_cursor_line_uses_shared_cursor_line() {
        let out = render_cursor_line("hello", 1);
        assert!(out.starts_with('h')); // text before the cursor is plain
        assert!(out.contains('\u{1b}')); // the cursor cell is styled (inverse)
                                         // cursor past end-of-line still renders a block (a styled space).
        assert!(render_cursor_line("hi", 5).contains('\u{1b}'));
        let wide = render_cursor_line("你好", 1);
        assert_eq!(a3s_tui::style::strip_ansi(&wide), "你好");
        assert!(wide.contains("你"));
    }

    #[test]
    fn kb_delete_confirm_line_uses_shared_confirm() {
        let root = temp_root("confirm-line");
        let (ide, _) = kb_ide_with_file(&root, "note.md");
        let line = kb_delete_confirm_line(&ide, 80).expect("confirm line");
        let plain = a3s_tui::style::strip_ansi(&line);

        assert_eq!(a3s_tui::style::visible_len(&line), 80);
        assert!(plain.contains("Delete"), "{plain}");
        assert!(plain.contains("note.md"), "{plain}");
        assert!(plain.contains("Enter/y"), "{plain}");
        assert!(line.contains("\x1b["), "Confirm line should be styled");
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn kb_delete_confirm_can_toggle_to_cancel() {
        let root = temp_root("confirm-cancel");
        let (mut ide, path) = kb_ide_with_file(&root, "note.md");

        assert!(handle_kb_delete_confirm_key(&mut ide, &key(KeyCode::Right)));
        assert!(!ide.delete_confirm_yes);
        assert!(handle_kb_delete_confirm_key(&mut ide, &key(KeyCode::Enter)));

        assert!(path.exists());
        assert!(ide.armed_delete.is_none());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn kb_delete_confirm_y_deletes_selected_entry() {
        let root = temp_root("confirm-delete");
        let (mut ide, path) = kb_ide_with_file(&root, "note.md");

        assert!(handle_kb_delete_confirm_key(&mut ide, &k('y')));

        assert!(!path.exists());
        assert!(ide.entries.is_empty());
        assert!(ide.armed_delete.is_none());
        let flash = ide.flash.as_deref().unwrap_or_default();
        assert!(a3s_tui::style::strip_ansi(flash).contains("deleted note.md"));
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn kb_delete_confirm_preserves_x_again_delete() {
        let root = temp_root("confirm-x");
        let (mut ide, path) = kb_ide_with_file(&root, "note.md");

        assert!(handle_kb_delete_confirm_key(&mut ide, &k('x')));

        assert!(!path.exists());
        assert!(ide.entries.is_empty());
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn vim_join_replace_and_toggle_case() {
        let mut f = buf(&["foo", "bar"]);
        feed(&mut f, "J"); // join the next line with a space
        assert_eq!(f.lines, vec!["foo bar"]);

        let mut f = buf(&["abc"]);
        feed(&mut f, "rx"); // replace the char under the cursor
        assert_eq!(f.lines[0], "xbc");

        let mut f = buf(&["aB"]);
        feed(&mut f, "~"); // toggle case + advance
        assert_eq!(f.lines[0], "AB");
    }
}
