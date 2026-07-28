//! `/kb` local personal knowledge-base panel.
//!
//! `/kb` shows the local vault state, previews imports before copying files,
//! and keeps note/import/search flows explicit so a mistyped path does not turn
//! into a note by accident. Shareable OKF knowledge-package assets live under
//! the workspace-visible `okf/` directory and are managed by `/okf`.

use super::super::*;
use a3s_tui::components::{divider_line_with, DetailPanel, DetailRow};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum KbLocalCommand {
    Home,
    Vault,
    Add(String),
    Import(String),
    Search(String),
    Usage(&'static str),
}

pub(crate) struct KbSearch {
    pub(crate) query: String,
    pub(crate) hits: Vec<kbutil::SearchHit>,
}

pub(crate) struct KbPanel {
    pub(crate) stats: kbutil::KbStats,
    pub(crate) recent: Vec<String>,
    pub(crate) pending_import: Option<kbutil::ImportPreview>,
    pub(crate) search: Option<KbSearch>,
    pub(crate) note: Option<String>,
    pub(crate) scroll: usize,
}

pub(crate) fn parse_kb_command(rest: &str) -> KbLocalCommand {
    let arg = rest.trim();
    if arg.is_empty() {
        return KbLocalCommand::Home;
    }
    let (head, tail) = arg
        .split_once(char::is_whitespace)
        .map(|(h, t)| (h, t.trim()))
        .unwrap_or((arg, ""));
    match head {
        "vault" => {
            if tail.is_empty() {
                KbLocalCommand::Vault
            } else {
                KbLocalCommand::Usage("usage: /kb vault")
            }
        }
        "add" => {
            if tail.is_empty() {
                KbLocalCommand::Usage("usage: /kb add <text>")
            } else {
                KbLocalCommand::Add(tail.to_string())
            }
        }
        "import" => {
            if tail.is_empty() {
                KbLocalCommand::Usage("usage: /kb import <file|folder>")
            } else {
                KbLocalCommand::Import(tail.to_string())
            }
        }
        "search" => {
            if tail.is_empty() {
                KbLocalCommand::Usage("usage: /kb search <query>")
            } else {
                KbLocalCommand::Search(tail.to_string())
            }
        }
        _ => KbLocalCommand::Usage(
            "usage: /kb add <text> · /kb import <path> · /kb search <query> · /kb vault",
        ),
    }
}

fn import_kind_label(kind: kbutil::ImportKind) -> &'static str {
    match kind {
        kbutil::ImportKind::File => "file",
        kbutil::ImportKind::Folder => "folder",
    }
}

fn fmt_bytes(bytes: u64) -> String {
    if bytes >= 1_048_576 {
        format!("{:.1} MiB", bytes as f64 / 1_048_576.0)
    } else if bytes >= 1024 {
        format!("{:.1} KiB", bytes as f64 / 1024.0)
    } else {
        format!("{bytes} B")
    }
}

fn kb_usage_hint() -> &'static str {
    "  /kb personal notes · add/import/search/vault · /okf manages team packages"
}

fn kb_line(rendered: &str, width: usize) -> String {
    pad_to(&truncate(rendered, width), width)
}

impl App {
    pub(crate) fn handle_kb_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        match parse_kb_command(rest) {
            KbLocalCommand::Home => {
                self.open_kb_home(None);
                None
            }
            KbLocalCommand::Vault => {
                self.open_kb_browser();
                None
            }
            KbLocalCommand::Add(text) => {
                let cwd = self.cwd.clone();
                let now = chrono::Utc::now().to_rfc3339();
                Some(cmd::cmd(move || async move {
                    let summary = tokio::task::spawn_blocking(move || {
                        kbutil::add_text_to_kb(&cwd, &text, &now)
                    })
                    .await
                    .unwrap_or_else(|e| format!("✗ /kb add failed: {e}"));
                    Msg::KbAdded(summary)
                }))
            }
            KbLocalCommand::Import(arg) => {
                self.prepare_kb_import(arg, None);
                None
            }
            KbLocalCommand::Search(query) => {
                let hits = kbutil::search_kb(&self.cwd, &query);
                let count = hits.len();
                self.open_kb_home(Some(format!(
                    "search `{}` · {count} hit(s)",
                    truncate(&query, 48)
                )));
                if let Some(kb) = self.kb.as_mut() {
                    kb.search = Some(KbSearch { query, hits });
                }
                None
            }
            KbLocalCommand::Usage(usage) => {
                self.open_kb_home(Some(usage.to_string()));
                None
            }
        }
    }

    pub(crate) fn open_kb_home(&mut self, note: Option<String>) {
        self.kb = Some(KbPanel {
            stats: kbutil::kb_stats(&self.cwd),
            recent: kbutil::recent_sources(&self.cwd, 8),
            pending_import: None,
            search: None,
            note,
            scroll: 0,
        });
    }

    pub(crate) fn open_kb_browser(&mut self) {
        let root = kbutil::kb_dir(&self.cwd);
        if !root.is_dir() {
            self.open_kb_home(Some(
                "KB is empty · add sources with `/kb add` or `/kb import`".to_string(),
            ));
            return;
        }
        let mut ide = Ide::browse(ide_children(&root, 0), "knowledge base");
        ide.kb_root = Some(root);
        self.kb = None;
        self.ide = Some(ide);
    }

    fn prepare_kb_import(&mut self, arg: String, note: Option<String>) {
        match kbutil::preview_import(&self.cwd, &arg) {
            Ok(preview) if preview.addable == 0 => {
                self.open_kb_home(Some(format!(
                    "nothing importable in {} · KB stores text files only",
                    truncate(&arg, 48)
                )));
            }
            Ok(preview) => {
                self.open_kb_home(note.or_else(|| {
                    Some("import preview ready · Enter confirms, Esc cancels".to_string())
                }));
                if let Some(kb) = self.kb.as_mut() {
                    kb.pending_import = Some(preview);
                }
            }
            Err(e) => self.open_kb_home(Some(format!("✗ /kb import: {e}"))),
        }
    }

    fn confirm_kb_import(&mut self) -> Option<Cmd<Msg>> {
        let preview = self.kb.as_mut()?.pending_import.take()?;
        let arg = preview.arg;
        if let Some(kb) = self.kb.as_mut() {
            kb.note = Some(format!("importing {}…", truncate(&arg, 48)));
        }
        let cwd = self.cwd.clone();
        let now = chrono::Utc::now().to_rfc3339();
        Some(cmd::cmd(move || async move {
            let summary =
                tokio::task::spawn_blocking(move || kbutil::import_to_kb(&cwd, &arg, &now))
                    .await
                    .unwrap_or_else(|e| format!("✗ /kb import failed: {e}"));
            Msg::KbAdded(summary)
        }))
    }

    pub(crate) fn handle_kb_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        match key.code {
            KeyCode::Esc => {
                if self
                    .kb
                    .as_ref()
                    .and_then(|kb| kb.pending_import.as_ref())
                    .is_some()
                {
                    if let Some(kb) = self.kb.as_mut() {
                        kb.pending_import = None;
                        kb.note = Some("import cancelled".to_string());
                    }
                } else {
                    self.kb = None;
                }
                None
            }
            KeyCode::Enter => self.confirm_kb_import(),
            KeyCode::Char('o') => {
                self.open_kb_browser();
                None
            }
            KeyCode::Char('r') => {
                self.open_kb_home(Some("refreshed".to_string()));
                None
            }
            KeyCode::Char('a') => {
                self.kb = None;
                self.textarea.set_value("/kb add ");
                None
            }
            KeyCode::Char('i') => {
                self.kb = None;
                self.textarea.set_value("/kb import ");
                None
            }
            KeyCode::Char('s') => {
                self.kb = None;
                self.textarea.set_value("/kb search ");
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(kb) = self.kb.as_mut() {
                    kb.scroll = kb.scroll.saturating_sub(1);
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(kb) = self.kb.as_mut() {
                    kb.scroll += 1;
                }
                None
            }
            KeyCode::PageUp => {
                if let Some(kb) = self.kb.as_mut() {
                    kb.scroll = kb.scroll.saturating_sub(10);
                }
                None
            }
            KeyCode::PageDown => {
                if let Some(kb) = self.kb.as_mut() {
                    kb.scroll += 10;
                }
                None
            }
            KeyCode::Char('g') => {
                if let Some(kb) = self.kb.as_mut() {
                    kb.scroll = 0;
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn render_kb(&self, kb: &KbPanel) -> String {
        let width = self.width as usize;
        let h = self.height as usize;
        let mut out = vec![
            kb_line(
                &Style::new().fg(ACCENT).bold().render(&format!(
                    "  /kb — knowledge base · {} source(s) · {} concept(s) · {} import(s) · {}",
                    kb.stats.sources,
                    kb.stats.concepts,
                    kb.stats.imports,
                    fmt_bytes(kb.stats.bytes)
                )),
                width,
            ),
            kb_line(
                &divider_line_with(width.min(u16::MAX as usize) as u16, "─", TN_GRAY),
                width,
            ),
        ];

        if let Some(note) = &kb.note {
            let color = if note.starts_with('✗') {
                TN_RED
            } else if note.contains("cancel") || note.contains("usage") {
                TN_YELLOW
            } else {
                TN_CYAN
            };
            out.push(kb_line(
                &Style::new().fg(color).render(&format!("  {note}")),
                width,
            ));
            out.push(String::new());
        }

        if let Some(preview) = &kb.pending_import {
            self.render_kb_import_preview(preview, &mut out, width);
            out.push(String::new());
        }

        if let Some(search) = &kb.search {
            self.render_kb_search(search, kb.scroll, &mut out, width, h);
        } else {
            self.render_kb_recent(kb, &mut out, width);
        }

        while out.len() + 4 < h {
            out.push(String::new());
        }
        out.push(kb_line(
            &Style::new().fg(TN_GRAY).render(kb_usage_hint()),
            width,
        ));
        out.push(kb_line(
            &Style::new()
                .fg(TN_GRAY)
                .render("  a add · i import · s search · r refresh"),
            width,
        ));
        out.truncate(h);
        while out.len() < h {
            out.push(String::new());
        }
        out.join("\n")
    }

    fn render_kb_import_preview(
        &self,
        preview: &kbutil::ImportPreview,
        out: &mut Vec<String>,
        width: usize,
    ) {
        out.extend(kb_import_preview_lines(preview, width));
    }

    fn render_kb_search(
        &self,
        search: &KbSearch,
        scroll: usize,
        out: &mut Vec<String>,
        width: usize,
        height: usize,
    ) {
        out.push(kb_line(
            &Style::new().fg(TN_CYAN).bold().render(&format!(
                "  Search `{}` · {} hit(s)",
                truncate(&search.query, 40),
                search.hits.len()
            )),
            width,
        ));
        if search.hits.is_empty() {
            out.push(kb_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  no matches yet — try a shorter term or import more sources"),
                width,
            ));
            return;
        }
        let room = height.saturating_sub(out.len() + 4).max(1);
        let start = scroll.min(search.hits.len().saturating_sub(1));
        for (idx, hit) in search.hits.iter().enumerate().skip(start).take(room) {
            let path_budget = (width / 2).clamp(20, 56);
            let path = truncate(&format!("{}:{}", hit.path, hit.line), path_budget);
            let snippet_budget = width.saturating_sub(path_budget + 9);
            let snippet = truncate(&hit.snippet, snippet_budget);
            out.push(kb_line(
                &format!(
                    "  {:>2}. {}  {}",
                    idx + 1,
                    Style::new().fg(TN_FG).render(&path),
                    Style::new().fg(TN_GRAY).render(&snippet)
                ),
                width,
            ));
        }
        if search.hits.len() > room {
            out.push(kb_line(
                &Style::new().fg(TN_GRAY).render(&format!(
                    "  {}/{} · ↑↓/jk/PgUp/PgDn scroll · g top",
                    start + 1,
                    search.hits.len()
                )),
                width,
            ));
        }
    }

    fn render_kb_recent(&self, kb: &KbPanel, out: &mut Vec<String>, width: usize) {
        out.extend(kb_recent_source_lines(&kb.recent, width));

        out.push(String::new());
        out.extend(kb_workflow_lines(width));
    }
}

fn kb_recent_source_lines(recent: &[String], width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let mut panel = DetailPanel::new("Recent sources")
        .show_separator(false)
        .indent(2)
        .title_color(TN_FG)
        .value_color(TN_FG)
        .muted_color(TN_GRAY);
    if recent.is_empty() {
        panel = panel.row(DetailRow::muted(
            "no sources yet — start with `/kb add <text>` or `/kb import <file|folder>`",
        ));
    } else {
        for source in recent {
            panel = panel.text(format!("• {source}"));
        }
    }

    panel
        .view(
            width.min(u16::MAX as usize) as u16,
            recent.len().saturating_add(1).max(2),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

fn kb_workflow_lines(width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    DetailPanel::new("Workflow")
        .show_separator(false)
        .indent(2)
        .title_color(TN_FG)
        .value_color(TN_GRAY)
        .text("add captures a note as Markdown")
        .text("import previews text files before copying")
        .text("search scans local personal knowledge sources")
        .text("vault keeps personal notes separate from shareable OKF packages")
        .view(width.min(u16::MAX as usize) as u16, 5)
        .lines()
        .map(str::to_string)
        .collect()
}

fn kb_import_preview_lines(preview: &kbutil::ImportPreview, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let mut meta = format!(
        "{} text file(s) · {} · {} skipped",
        preview.addable,
        fmt_bytes(preview.bytes),
        preview.skipped
    );
    if preview.capped {
        meta.push_str(" · capped");
    }

    DetailPanel::new("Import preview")
        .show_separator(false)
        .indent(2)
        .title_color(TN_YELLOW)
        .value_color(TN_GRAY)
        .action_color(TN_CYAN)
        .muted_color(TN_GRAY)
        .text(format!(
            "{} · {}",
            import_kind_label(preview.kind),
            preview.path.display()
        ))
        .text(meta)
        .action("Enter confirm import · Esc cancel")
        .view(width.min(u16::MAX as usize) as u16, 4)
        .lines()
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_explicit_kb_subcommands() {
        assert_eq!(parse_kb_command(""), KbLocalCommand::Home);
        assert!(matches!(
            parse_kb_command(" dashboard "),
            KbLocalCommand::Usage(_)
        ));
        for removed in ["open", "list", "os", "logs", "status"] {
            assert!(
                matches!(parse_kb_command(removed), KbLocalCommand::Usage(_)),
                "/kb {removed} should stay out of the local personal KB command surface"
            );
        }
        assert_eq!(parse_kb_command(" vault "), KbLocalCommand::Vault);
        assert_eq!(
            parse_kb_command(" add hello world "),
            KbLocalCommand::Add("hello world".into())
        );
        assert_eq!(
            parse_kb_command(" import docs "),
            KbLocalCommand::Import("docs".into())
        );
        assert_eq!(
            parse_kb_command(" search runtime "),
            KbLocalCommand::Search("runtime".into())
        );
        assert!(matches!(parse_kb_command("add"), KbLocalCommand::Usage(_)));
        assert!(matches!(
            parse_kb_command("unknown"),
            KbLocalCommand::Usage(
                "usage: /kb add <text> · /kb import <path> · /kb search <query> · /kb vault"
            )
        ));
    }

    #[test]
    fn byte_format_is_compact() {
        assert_eq!(fmt_bytes(512), "512 B");
        assert_eq!(fmt_bytes(2048), "2.0 KiB");
        assert_eq!(fmt_bytes(2_097_152), "2.0 MiB");
    }

    #[test]
    fn kb_import_preview_lines_use_shared_detail_panel_and_fit_width() {
        let preview = kbutil::ImportPreview {
            arg: "docs".to_string(),
            path: std::path::PathBuf::from(
                "/Users/roylin/code/a3s/docs/very/long/path/that/must/truncate",
            ),
            kind: kbutil::ImportKind::Folder,
            addable: 12,
            skipped: 3,
            capped: true,
            bytes: 4096,
        };
        let lines = kb_import_preview_lines(&preview, 64);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(lines.len(), 4);
        assert!(plain.contains("Import preview"), "{plain}");
        assert!(plain.contains("folder"), "{plain}");
        assert!(plain.contains("12 text file"), "{plain}");
        assert!(plain.contains("4.0 KiB"), "{plain}");
        assert!(plain.contains("capped"), "{plain}");
        assert!(plain.contains("Enter confirm import"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 64),
            "{plain}"
        );
    }

    #[test]
    fn kb_workflow_lines_use_shared_detail_panel_and_fit_width() {
        let lines = kb_workflow_lines(40);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(lines.len(), 5);
        assert!(plain.contains("Workflow"), "{plain}");
        assert!(plain.contains("add captures"), "{plain}");
        assert!(plain.contains("search scans"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn kb_recent_source_lines_use_shared_detail_panel_and_fit_width() {
        let recent = vec![
            ".a3s/kb/sources/project/very-long-source-name.md".to_string(),
            ".a3s/kb/sources/notes.md".to_string(),
        ];
        let lines = kb_recent_source_lines(&recent, 36);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(lines.len(), 3);
        assert!(plain.contains("Recent sources"), "{plain}");
        assert!(plain.contains("• .a3s/kb"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 36),
            "{plain}"
        );

        let empty = kb_recent_source_lines(&[], 42);
        let empty_plain = empty
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(empty.len(), 2);
        assert!(empty_plain.contains("no sources yet"), "{empty_plain}");
    }

    #[test]
    fn kb_lines_are_width_bounded_with_styles() {
        let line = kb_line(
            &Style::new().fg(TN_GRAY).render(
                "  no sources yet — start with `/kb add <text>` or `/kb import <file|folder>`",
            ),
            38,
        );

        assert!(
            a3s_tui::style::visible_len(&line) <= 38,
            "{}",
            a3s_tui::style::strip_ansi(&line)
        );
    }
}
