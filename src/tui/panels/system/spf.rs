//! Superfile-style chrome shared by `/ide`, `/config`, and the `/kb` browser:
//! rounded-corner panels with the title embedded in the top border, focus
//! shown by the title rather than a saturated full-frame border, plus small
//! file-metadata helpers.
//!
//! Contract: content rows passed to [`frame`] must already be fitted to the
//! panel's inner width (plain-truncate BEFORE styling — `truncate` on a styled
//! string would split ANSI codes). `frame` only pads (ANSI-safe) and borders.

use super::super::*;
use a3s_tui::components::{Breadcrumb, PanelFrame};

const IDE_GUTTER_WIDTH: usize = 7;
const FILE_ICON_WIDTH: usize = 2;

/// A terminal-safe file glyph and its semantic accent.
///
/// The glyph set deliberately avoids emoji and private-use Nerd Font codepoints:
/// every icon renders in one or two cells with ordinary terminal fonts, so the
/// tree keeps a stable name column on macOS, Linux, SSH, and narrow panes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FileIcon {
    pub(crate) glyph: &'static str,
    pub(crate) color: Color,
}

/// Split the screen for the tree | editor layout. Returns
/// `(tree_panel_width, right_panel_width)` — both are TOTAL widths including
/// each panel's two border columns.
pub(crate) fn ide_split(w: usize) -> (usize, usize) {
    let tw = (w / 3).clamp(18, 40).min(w.saturating_sub(24).max(14));
    (tw, w.saturating_sub(tw))
}

/// Editor/preview text width inside the right panel: total minus the two
/// border columns and, when there is room, the line-number gutter and rule.
/// The key handler's horizontal-scroll math and the renderer both use this —
/// single source. Honest on tiny terminals: never claims more room than the
/// panel actually has (a flattering floor here overflowed the frame).
pub(crate) fn ide_content_width(w: usize) -> usize {
    let iw = ide_split(w).1.saturating_sub(2);
    if ide_gutter_on(w) {
        iw.saturating_sub(IDE_GUTTER_WIDTH)
    } else {
        iw
    }
}

/// Whether the editor renders the line-number gutter (dropped on tiny panels).
pub(crate) fn ide_gutter_on(w: usize) -> bool {
    ide_split(w).1.saturating_sub(2) >= IDE_GUTTER_WIDTH + 8
}

/// Render the editor's quiet line-number gutter with a stable vertical rule.
pub(crate) fn ide_gutter(line: usize, current: bool) -> String {
    let label = if line > 9_999 {
        format!("{:>3}k", (line / 1_000).min(999))
    } else {
        format!("{line:>4}")
    };
    let number = Style::new()
        .fg(if current { TN_YELLOW } else { TN_SUBTLE })
        .render(&label);
    let rule = Style::new().fg(BORDER_SUBTLE).render(" │ ");
    format!("{number}{rule}")
}

/// Fit PLAIN text to exactly `width` display columns: truncate, pop any
/// residue a width-inconsistent sequence leaves behind (a3s-tui's truncate
/// counts emoji+VS16 as 1 column but visible_len as 2 — e.g. 🖼️ in a file
/// name), then pad. Style AFTER fitting, never before.
pub(crate) fn fit(s: &str, width: usize) -> String {
    let mut t = truncate(s, width);
    while a3s_tui::style::visible_len(&t) > width {
        t.pop();
    }
    pad_to(&t, width)
}

/// One rounded-border panel: `╭─ title ───╮ │…│ ╰───╯`, `w`×`h` cells exactly.
/// Borders stay quiet; focus is carried by the title color.
pub(crate) fn frame(
    title: &str,
    w: usize,
    h: usize,
    focused: bool,
    mut rows: Vec<String>,
) -> Vec<String> {
    rows.truncate(h.saturating_sub(2));
    PanelFrame::new(title)
        .rows(rows)
        .focused(focused)
        .border_color(BORDER_SUBTLE)
        .focused_border_color(BORDER_SUBTLE)
        .title_color(TN_FG)
        .focused_title_color(ACCENT)
        .lines(w.min(u16::MAX as usize) as u16, h)
}

/// Join two panels side by side (rows are already exact-width).
pub(crate) fn hjoin(left: &[String], right: &[String]) -> Vec<String> {
    (0..left.len().max(right.len()))
        .map(|i| {
            format!(
                "{}{}",
                left.get(i).cloned().unwrap_or_default(),
                right.get(i).cloned().unwrap_or_default()
            )
        })
        .collect()
}

/// File and folder iconography shared by `/ide` and `/config`.
///
/// Directories use a disclosure marker in the tree, so named directory kinds
/// can carry useful identity without losing open/closed state. Files prefer
/// language or role sigils over decorative document emoji.
pub(crate) fn file_icon(name: &str, is_dir: bool, expanded: bool) -> FileIcon {
    let lower = name.trim_end_matches('/').to_ascii_lowercase();
    if is_dir {
        return match lower.as_str() {
            "workspace" => FileIcon {
                glyph: "⌂",
                color: ACCENT,
            },
            "src" | "source" | "lib" => FileIcon {
                glyph: "◈",
                color: TN_CYAN,
            },
            "test" | "tests" | "spec" | "specs" | "__tests__" => FileIcon {
                glyph: "✓",
                color: TN_GREEN,
            },
            "doc" | "docs" | "documentation" | "knowledge base" => FileIcon {
                glyph: "≡",
                color: TN_PURPLE,
            },
            "asset" | "assets" | "image" | "images" | "public" | "static" => FileIcon {
                glyph: "▧",
                color: TN_PURPLE,
            },
            ".a3s" | ".config" | "config" | "configs" | "configuration" => FileIcon {
                glyph: "⌘",
                color: TN_YELLOW,
            },
            ".github" | ".gitlab" => FileIcon {
                glyph: "±",
                color: TN_ORANGE,
            },
            "bin" | "script" | "scripts" => FileIcon {
                glyph: "❯",
                color: TN_GREEN,
            },
            "data" | "database" | "db" | "migration" | "migrations" => FileIcon {
                glyph: "◉",
                color: TN_CYAN,
            },
            "app" | "apps" | "crate" | "crates" | "package" | "packages" | "modules" => FileIcon {
                glyph: "◫",
                color: ACCENT,
            },
            _ => FileIcon {
                glyph: if expanded { "◇" } else { "◆" },
                color: ACCENT,
            },
        };
    }

    if matches!(
        lower.as_str(),
        "cargo.toml" | "cargo.lock" | "rust-toolchain" | "rust-toolchain.toml"
    ) {
        return FileIcon {
            glyph: "◈",
            color: TN_ORANGE,
        };
    }
    if matches!(
        lower.as_str(),
        "package.json"
            | "package-lock.json"
            | "pnpm-lock.yaml"
            | "yarn.lock"
            | "bun.lock"
            | "bun.lockb"
    ) {
        return FileIcon {
            glyph: "J",
            color: TN_YELLOW,
        };
    }
    if matches!(lower.as_str(), "tsconfig.json" | "deno.json" | "deno.jsonc") {
        return FileIcon {
            glyph: "T",
            color: ACCENT,
        };
    }
    if matches!(lower.as_str(), "go.mod" | "go.sum" | "go.work") {
        return FileIcon {
            glyph: "G",
            color: TN_CYAN,
        };
    }
    if matches!(
        lower.as_str(),
        "pyproject.toml" | "requirements.txt" | "poetry.lock" | "pipfile" | "pipfile.lock"
    ) {
        return FileIcon {
            glyph: "P",
            color: TN_YELLOW,
        };
    }
    if matches!(
        lower.as_str(),
        ".editorconfig" | ".prettierrc" | ".eslintrc" | ".stylelintrc" | "biome.json"
    ) {
        return FileIcon {
            glyph: "⌘",
            color: TN_YELLOW,
        };
    }
    if lower == ".env" || lower.starts_with(".env.") {
        return FileIcon {
            glyph: "●",
            color: TN_GREEN,
        };
    }
    if matches!(
        lower.as_str(),
        ".gitignore" | ".gitattributes" | ".gitmodules" | ".gitkeep"
    ) {
        return FileIcon {
            glyph: "±",
            color: TN_ORANGE,
        };
    }
    if matches!(
        lower.as_str(),
        "dockerfile" | "containerfile" | "docker-compose.yml" | "docker-compose.yaml"
    ) {
        return FileIcon {
            glyph: "□",
            color: TN_CYAN,
        };
    }
    if matches!(lower.as_str(), "makefile" | "justfile" | "taskfile") {
        return FileIcon {
            glyph: "⌘",
            color: TN_YELLOW,
        };
    }
    if lower == "agents.md" {
        return FileIcon {
            glyph: "A",
            color: TN_PURPLE,
        };
    }
    if lower == "skill.md" {
        return FileIcon {
            glyph: "✦",
            color: TN_CYAN,
        };
    }
    if lower.starts_with("readme") || lower.starts_with("changelog") {
        return FileIcon {
            glyph: "≡",
            color: TN_PURPLE,
        };
    }
    if lower.starts_with("license") || lower.starts_with("copying") {
        return FileIcon {
            glyph: "¶",
            color: TN_YELLOW,
        };
    }
    if lower.contains(".test.") || lower.contains(".spec.") || lower.ends_with("_test.rs") {
        return FileIcon {
            glyph: "✓",
            color: TN_GREEN,
        };
    }

    let extension = lower.rsplit_once('.').map(|(_, ext)| ext).unwrap_or("");
    match extension {
        "rs" => FileIcon {
            glyph: "◈",
            color: TN_ORANGE,
        },
        "md" | "mdx" | "rst" | "txt" => FileIcon {
            glyph: "≡",
            color: TN_PURPLE,
        },
        "acl" | "toml" | "hcl" | "yaml" | "yml" | "ini" | "conf" | "cfg" => FileIcon {
            glyph: "⌘",
            color: TN_YELLOW,
        },
        "json" | "jsonc" => FileIcon {
            glyph: "{",
            color: TN_YELLOW,
        },
        "lock" => FileIcon {
            glyph: "⌾",
            color: TN_SUBTLE,
        },
        "js" | "jsx" | "mjs" | "cjs" => FileIcon {
            glyph: "J",
            color: TN_YELLOW,
        },
        "ts" | "tsx" => FileIcon {
            glyph: "T",
            color: ACCENT,
        },
        "html" | "htm" | "vue" | "svelte" => FileIcon {
            glyph: "<",
            color: TN_ORANGE,
        },
        "css" | "scss" | "sass" | "less" => FileIcon {
            glyph: "#",
            color: TN_CYAN,
        },
        "py" | "pyi" => FileIcon {
            glyph: "P",
            color: TN_YELLOW,
        },
        "go" => FileIcon {
            glyph: "G",
            color: TN_CYAN,
        },
        "c" | "h" => FileIcon {
            glyph: "C",
            color: ACCENT,
        },
        "cc" | "cpp" | "cxx" | "hpp" | "hxx" => FileIcon {
            glyph: "+",
            color: ACCENT,
        },
        "java" | "kt" | "kts" | "swift" | "rb" | "php" => FileIcon {
            glyph: "◆",
            color: TN_RED,
        },
        "lua" | "dart" | "ex" | "exs" => FileIcon {
            glyph: "◆",
            color: TN_PURPLE,
        },
        "sh" | "bash" | "zsh" | "fish" | "ps1" => FileIcon {
            glyph: "❯",
            color: TN_GREEN,
        },
        "png" | "jpg" | "jpeg" | "gif" | "webp" | "bmp" | "ico" | "svg" => FileIcon {
            glyph: "▧",
            color: TN_PURPLE,
        },
        "sql" | "db" | "sqlite" | "sqlite3" => FileIcon {
            glyph: "◉",
            color: TN_CYAN,
        },
        "csv" | "tsv" => FileIcon {
            glyph: "≣",
            color: TN_GREEN,
        },
        "xml" | "graphql" | "gql" | "proto" => FileIcon {
            glyph: "◇",
            color: TN_PURPLE,
        },
        "log" => FileIcon {
            glyph: "≡",
            color: TN_SUBTLE,
        },
        "zip" | "gz" | "tgz" | "bz2" | "xz" | "7z" | "rar" | "tar" => FileIcon {
            glyph: "▦",
            color: TN_RED,
        },
        "pdf" => FileIcon {
            glyph: "▤",
            color: TN_RED,
        },
        _ => FileIcon {
            glyph: "·",
            color: TN_SUBTLE,
        },
    }
}

pub(crate) fn path_icon(path: &std::path::Path, is_dir: bool, expanded: bool) -> FileIcon {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy())
        .unwrap_or_else(|| path.as_os_str().to_string_lossy());
    file_icon(&name, is_dir, expanded)
}

fn styled_cell(text: &str, color: Color, selected: bool, bold: bool) -> String {
    let mut style = Style::new().fg(color);
    if selected {
        style = style.bg(SURFACE_SELECTED);
    }
    if bold {
        style = style.bold();
    }
    style.render(text)
}

/// Render one exact-width file-tree row with independently colored disclosure,
/// icon, and label cells. Selection fills the row without erasing file accents.
pub(crate) fn file_tree_row(
    name: &str,
    depth: usize,
    is_dir: bool,
    expanded: bool,
    selected: bool,
    focused: bool,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }

    let icon = file_icon(name, is_dir, expanded);
    let depth_width = depth.saturating_mul(2).min(width.saturating_sub(1));
    let disclosure_width = 2.min(width.saturating_sub(depth_width));
    let icon_width = FILE_ICON_WIDTH.min(
        width
            .saturating_sub(depth_width)
            .saturating_sub(disclosure_width),
    );
    let name_width = width
        .saturating_sub(depth_width)
        .saturating_sub(disclosure_width)
        .saturating_sub(icon_width);

    let indent = " ".repeat(depth_width);
    let disclosure = fit(
        if is_dir {
            if expanded {
                "▾ "
            } else {
                "▸ "
            }
        } else {
            "  "
        },
        disclosure_width,
    );
    let icon_cell = fit(icon.glyph, icon_width);
    let label = fit(name, name_width);
    let label_color = if name.starts_with('.') && !selected {
        TN_GRAY
    } else {
        TN_FG
    };
    let strong = selected && focused;

    format!(
        "{}{}{}{}",
        styled_cell(&indent, TN_FG, selected, false),
        styled_cell(
            &disclosure,
            if selected && focused {
                ACCENT
            } else {
                TN_SUBTLE
            },
            selected,
            strong,
        ),
        styled_cell(&icon_cell, icon.color, selected, strong),
        styled_cell(&label, label_color, selected, strong),
    )
}

/// Human-readable byte size ("312 B", "4.2 KB", "1.8 MB").
pub(crate) fn human_size(bytes: u64) -> String {
    const K: f64 = 1024.0;
    let b = bytes as f64;
    if b < K {
        format!("{bytes} B")
    } else if b < K * K {
        format!("{:.1} KB", b / K)
    } else if b < K * K * K {
        format!("{:.1} MB", b / K / K)
    } else {
        format!("{:.1} GB", b / K / K / K)
    }
}

pub(crate) fn file_meta_breadcrumb_line(
    path: &std::path::Path,
    loaded_lines: Option<usize>,
    width: usize,
) -> String {
    if width == 0 {
        return String::new();
    }

    let rendered = Breadcrumb::new(file_meta_parts(path, loaded_lines))
        .separator(" · ")
        .active_color(TN_FG)
        .inactive_color(TN_GRAY)
        .separator_color(TN_GRAY)
        .view(width.min(u16::MAX as usize) as u16);
    let is_dir = std::fs::metadata(path).is_ok_and(|metadata| metadata.is_dir());
    let icon = path_icon(path, is_dir, is_dir);
    let glyph = Style::new().fg(icon.color).render(icon.glyph);
    a3s_tui::style::fit_visible(&format!(" {glyph} {rendered}"), width)
}

fn file_meta_parts(path: &std::path::Path, loaded_lines: Option<usize>) -> Vec<String> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.display().to_string());
    let mut parts = vec![name];
    if let Ok(md) = std::fs::metadata(path) {
        if md.is_dir() {
            let n = std::fs::read_dir(path)
                .map(|rd| rd.take(1000).count())
                .unwrap_or(0);
            parts.push(format!("{n} items"));
        } else {
            parts.push(human_size(md.len()));
        }
        if let Ok(t) = md.modified() {
            let dt: chrono::DateTime<chrono::Local> = t.into();
            parts.push(dt.format("%Y-%m-%d %H:%M").to_string());
        }
    }
    if let Some(n) = loaded_lines {
        parts.push(format!("{n} lines"));
    }
    parts
}

/// Delete `path` (file or dir), but only inside `root` — the guard that keeps
/// the `/kb` browser's delete from ever touching anything outside the vault.
pub(crate) fn delete_within(root: &std::path::Path, path: &std::path::Path) -> Result<(), String> {
    let canon_root = root.canonicalize().map_err(|e| e.to_string())?;
    let canon = path.canonicalize().map_err(|e| e.to_string())?;
    if !canon.starts_with(&canon_root) || canon == canon_root {
        return Err("outside the KB vault".into());
    }
    if canon.is_dir() {
        std::fs::remove_dir_all(&canon).map_err(|e| e.to_string())
    } else {
        std::fs::remove_file(&canon).map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frame_is_exactly_sized_with_rounded_corners() {
        let out = frame("title", 20, 5, true, vec!["hi".into()]);
        assert_eq!(out.len(), 5);
        for row in &out {
            assert_eq!(a3s_tui::style::visible_len(row), 20, "{row:?}");
        }
        let strip = |s: &str| {
            let mut o = String::new();
            let mut esc = false;
            for c in s.chars() {
                match (esc, c) {
                    (false, '\u{1b}') => esc = true,
                    (false, _) => o.push(c),
                    (true, 'm') => esc = false,
                    (true, _) => {}
                }
            }
            o
        };
        assert!(strip(&out[0]).starts_with("╭─ title"));
        assert!(strip(&out[0]).ends_with('╮'));
        assert!(strip(&out[4]).starts_with('╰') && strip(&out[4]).ends_with('╯'));
        assert!(strip(&out[1]).starts_with('│') && strip(&out[1]).ends_with('│'));
    }

    #[test]
    fn ide_content_width_tracks_split() {
        // Single source of truth: gutter + right borders are accounted for.
        let (tw, right) = ide_split(120);
        assert_eq!(tw + right, 120);
        assert_eq!(ide_content_width(120), right - 2 - IDE_GUTTER_WIDTH);
        // Tiny terminals: honest (≤ panel room), gutter dropped, never a lie
        // that overflows the frame.
        let iw20 = ide_split(20).1.saturating_sub(2);
        assert!(!ide_gutter_on(20));
        assert_eq!(ide_content_width(20), iw20);
    }

    #[test]
    fn icons_are_terminal_safe_and_stay_within_truncate_budgets() {
        for glyph in [
            "⌂", "◈", "✓", "≡", "▧", "⌘", "±", "❯", "◉", "◫", "◇", "◆", "●", "□", "A", "✦", "¶",
            "{", "⌾", "J", "T", "<", "#", "P", "G", "C", "+", "▦", "▤", "≣", "·",
        ] {
            assert!(
                a3s_tui::style::visible_len(glyph) <= FILE_ICON_WIDTH,
                "{glyph} is wider than the icon cell"
            );
        }
        let cases = [
            ("src", true, true),
            ("folder", true, false),
            ("main.rs", false, false),
            ("README.md", false, false),
            ("config.acl", false, false),
            ("image.png", false, false),
            ("script.sh", false, false),
            ("data.json", false, false),
            ("unknown", false, false),
        ];
        for (name, is_dir, expanded) in cases {
            let icon = file_icon(name, is_dir, expanded);
            assert!(
                a3s_tui::style::visible_len(icon.glyph) <= FILE_ICON_WIDTH,
                "{} is wider than the icon cell",
                icon.glyph
            );
            let row = format!("{} very-long-file-name-{}.ext", icon.glyph, "x".repeat(40));
            let cut = truncate(&row, 20);
            assert!(
                a3s_tui::style::visible_len(&cut) <= 20,
                "{} breaks the column budget",
                icon.glyph
            );
        }
    }

    #[test]
    fn icons_cover_workspace_roles_and_common_languages() {
        assert_eq!(file_icon("workspace", true, true).glyph, "⌂");
        assert_eq!(file_icon("src", true, false).glyph, "◈");
        assert_eq!(file_icon("tests", true, false).glyph, "✓");
        assert_eq!(file_icon(".a3s", true, false).glyph, "⌘");
        assert_eq!(file_icon("scripts", true, false).glyph, "❯");
        assert_eq!(file_icon("other", true, false).glyph, "◆");
        assert_eq!(file_icon("other", true, true).glyph, "◇");

        assert_eq!(file_icon("main.rs", false, false).glyph, "◈");
        assert_eq!(file_icon("view.tsx", false, false).glyph, "T");
        assert_eq!(file_icon("data.json", false, false).glyph, "{");
        assert_eq!(file_icon("config.acl", false, false).glyph, "⌘");
        assert_eq!(file_icon("Cargo.toml", false, false).glyph, "◈");
        assert_eq!(file_icon("package.json", false, false).glyph, "J");
        assert_eq!(file_icon("tsconfig.json", false, false).glyph, "T");
        assert_eq!(file_icon("pyproject.toml", false, false).glyph, "P");
        assert_eq!(file_icon("AGENTS.md", false, false).glyph, "A");
        assert_eq!(file_icon("release_test.rs", false, false).glyph, "✓");
        assert_eq!(file_icon("asset.webp", false, false).glyph, "▧");
    }

    #[test]
    fn file_tree_rows_keep_icons_aligned_and_selection_full_width() {
        let folder = file_tree_row("src", 1, true, true, false, true, 24);
        let file = file_tree_row("main.rs", 2, false, false, true, true, 24);
        let folder_plain = a3s_tui::style::strip_ansi(&folder);
        let file_plain = a3s_tui::style::strip_ansi(&file);

        assert_eq!(a3s_tui::style::visible_len(&folder), 24);
        assert_eq!(a3s_tui::style::visible_len(&file), 24);
        assert!(folder_plain.contains("▾ ◈ src"), "{folder_plain:?}");
        assert!(file_plain.contains("◈ main.rs"), "{file_plain:?}");
        assert!(file.contains(&SURFACE_SELECTED.bg_ansi()));
        assert!(file.contains(&TN_ORANGE.fg_ansi()));
    }

    #[test]
    fn ide_gutter_has_a_quiet_rule_and_exact_width() {
        let gutter = ide_gutter(42, true);
        let large = ide_gutter(12_345, false);
        assert_eq!(a3s_tui::style::visible_len(&gutter), IDE_GUTTER_WIDTH);
        assert_eq!(a3s_tui::style::visible_len(&large), IDE_GUTTER_WIDTH);
        assert_eq!(a3s_tui::style::strip_ansi(&gutter), "  42 │ ");
        assert_eq!(a3s_tui::style::strip_ansi(&large), " 12k │ ");
        assert!(gutter.contains(&TN_YELLOW.fg_ansi()));
        assert!(gutter.contains(&BORDER_SUBTLE.fg_ansi()));
    }

    #[test]
    fn human_sizes_read_naturally() {
        assert_eq!(human_size(312), "312 B");
        assert_eq!(human_size(4300), "4.2 KB");
        assert_eq!(human_size(1_900_000), "1.8 MB");
    }

    #[test]
    fn file_meta_breadcrumb_line_uses_shared_breadcrumb_and_fits_width() {
        let root = std::env::temp_dir().join(format!("a3s-spf-meta-{}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let file = root.join("very-long-file-name-for-metadata.md");
        std::fs::write(&file, "one\ntwo\n").unwrap();

        let line = file_meta_breadcrumb_line(&file, Some(2), 96);
        let plain = a3s_tui::style::strip_ansi(&line);

        assert!(plain.contains("very-long"), "{plain}");
        assert!(plain.contains('·'), "{plain}");
        assert!(plain.contains("2 lines"), "{plain}");
        assert!(line.contains("\x1b["));
        assert!(a3s_tui::style::visible_len(&line) <= 96, "{}", plain);
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn delete_within_refuses_outside_and_root_itself() {
        let root = std::env::temp_dir().join(format!("a3s-spf-{}", std::process::id()));
        std::fs::create_dir_all(root.join("sub")).unwrap();
        std::fs::write(root.join("sub/x.md"), "x").unwrap();
        let outside = std::env::temp_dir();
        assert!(delete_within(&root, &outside).is_err());
        assert!(
            delete_within(&root, &root).is_err(),
            "root itself is protected"
        );
        assert!(delete_within(&root, &root.join("sub/x.md")).is_ok());
        assert!(!root.join("sub/x.md").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
