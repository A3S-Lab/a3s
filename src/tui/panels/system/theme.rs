//! `/theme` overlay: theme list + live syntax-highlight preview.

use super::super::*;
use a3s_tui::components::{PreviewItem, PreviewPanel, PreviewPanelMsg};
use a3s_tui::event::MouseEvent;

fn theme_panel_height() -> usize {
    THEMES.len() + 8
}

fn theme_panel(selected: usize) -> PreviewPanel {
    let selected = selected.min(THEMES.len().saturating_sub(1));
    let items = THEMES
        .iter()
        .map(|theme| PreviewItem::new(theme.name))
        .collect::<Vec<_>>();
    let theme = &THEMES[selected];
    let sample = [
        "// syntax preview",
        "fn compute(n: usize) -> String {",
        "    let total = n * 42;",
        "    format!(\"sum: {}\", total)",
        "}",
    ];
    let preview = sample
        .into_iter()
        .map(|line| highlight_with(line, "rust", theme))
        .collect::<Vec<_>>();

    PreviewPanel::new("Theme")
        .subtitle("↑/↓ preview · Enter/click apply · Esc")
        .items(items)
        .selected(selected)
        .max_items(THEMES.len().max(1))
        .preview_title("preview")
        .preview_lines(preview)
        .indent(2)
        .marker("▸")
        .title_color(ACCENT)
        .subtitle_color(TN_GRAY)
        .text_color(TN_GRAY)
        .muted_color(TN_GRAY)
        .divider_color(TN_GRAY)
        .preview_color(TN_FG)
        .selected_colors(TN_FG, SURFACE_SELECTED)
}

fn theme_panel_lines(selected: usize, width: usize) -> Vec<String> {
    theme_panel(selected)
        .view(width.min(u16::MAX as usize) as u16, theme_panel_height())
        .lines()
        .map(str::to_string)
        .collect()
}

fn theme_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    pub(crate) fn apply_theme_selection(&mut self, selected: usize) {
        let selected = selected.min(THEMES.len().saturating_sub(1));
        SYNTAX_THEME.store(selected, std::sync::atomic::Ordering::Relaxed);
        self.theme_panel = None;
        self.rebuild_viewport();
        self.push_line(
            &Style::new()
                .fg(TN_GREEN)
                .render(&format!("  ◆ code theme: {}", THEMES[selected].name)),
        );
    }

    pub(crate) fn handle_theme_mouse(&mut self, mouse: &MouseEvent) {
        let Some(selected) = self.theme_panel else {
            return;
        };
        let width = (self.width as usize).min(u16::MAX as usize);
        let mut panel = theme_panel(selected);
        let row_count = panel
            .view(width as u16, theme_panel_height())
            .lines()
            .count();
        if row_count == 0 {
            return;
        }
        let y_offset =
            theme_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(PreviewPanelMsg::Selected(index)) => self.apply_theme_selection(index),
            Some(PreviewPanelMsg::Cancelled) => self.theme_panel = None,
            None => {
                let after = panel.selected_index();
                if after != before {
                    self.theme_panel = Some(after);
                }
            }
        }
    }

    /// `/theme` picker: a theme list + a live syntax-highlight preview.
    pub(crate) fn overlay_theme(&self, composed: String) -> String {
        let Some(sel) = self.theme_panel else {
            return composed;
        };
        let width = self.width as usize;
        let menu = theme_panel_lines(sel, width);
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_lines_are_width_bounded_with_styles() {
        let lines = theme_panel_lines(THEMES.len() + 10, 24);

        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 24),
            "{:?}",
            lines
                .iter()
                .map(|line| a3s_tui::style::strip_ansi(line))
                .collect::<Vec<_>>()
        );
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>();
        assert!(plain.iter().any(|line| line.contains("Theme")), "{plain:?}");
        assert!(
            plain.iter().any(|line| line.contains("preview")),
            "{plain:?}"
        );
    }

    #[test]
    fn theme_panel_mouse_wheel_updates_preview_selection() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let row_count = theme_panel_lines(0, 48).len();
        let y_offset = theme_overlay_y_offset(24, row_count, 5);
        let mut panel = theme_panel(0);
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: y_offset + 2,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, None);
        assert_eq!(
            panel.selected_index(),
            1.min(THEMES.len().saturating_sub(1))
        );

        let ignored = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 0,
            row: y_offset.saturating_sub(1),
            modifiers: a3s_tui::KeyModifiers::NONE,
        });
        assert_eq!(ignored, None);
    }

    #[test]
    fn theme_panel_click_selects_item_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        assert!(THEMES.len() > 1);
        let row_count = theme_panel_lines(0, 48).len();
        let y_offset = theme_overlay_y_offset(24, row_count, 5);
        let mut panel = theme_panel(0);
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: y_offset + 3,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, Some(PreviewPanelMsg::Selected(1)));
    }

    #[test]
    fn theme_overlay_moves_above_dynamic_composer_rows() {
        assert_eq!(theme_overlay_y_offset(24, 8, 5), 11);
        assert_eq!(theme_overlay_y_offset(24, 8, 9), 7);
    }
}
