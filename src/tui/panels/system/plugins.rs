//! `/plugin` overlay: enable/disable Claude skills.

use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;
use std::collections::HashSet;

fn plugin_panel_max_items(height: usize) -> usize {
    height.saturating_sub(8).clamp(3, 12)
}

fn plugin_panel(
    skills: &[(String, String)],
    disabled_skills: &HashSet<String>,
    selected: usize,
    width: usize,
    height: usize,
) -> Option<(MenuPanel, usize)> {
    let total = skills.len();
    if total == 0 {
        return None;
    }

    let selected = selected.min(total - 1);
    let on_count = total - disabled_skills.len().min(total);
    let max_items = plugin_panel_max_items(height);
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let label_width = width.saturating_sub(14).clamp(8, 18);
    let items = skills
        .iter()
        .map(|(name, desc)| {
            let on = !disabled_skills.contains(name);
            MenuItem::new(format!("/{name}"))
                .description(desc.clone())
                .checked(on)
                .color(if on { TN_CYAN } else { TN_GRAY })
        })
        .collect::<Vec<_>>();

    let panel = MenuPanel::new(format!(
        "Plugins & skills ({on_count}/{total} on) — ↑/↓ · Space toggle · Esc"
    ))
    .items(items)
    .selected(selected)
    .scroll(scroll)
    .max_items(max_items)
    .label_width(label_width)
    .show_scroll(total > max_items)
    .indent(2)
    .marker("▸")
    .title_color(ACCENT)
    .text_color(TN_GRAY)
    .muted_color(TN_GRAY)
    .checked_color(TN_GREEN)
    .selected_colors(TN_FG, SURFACE_SELECTED);
    Some((panel, max_items + 3))
}

fn plugin_panel_lines(
    skills: &[(String, String)],
    disabled_skills: &HashSet<String>,
    selected: usize,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) =
        plugin_panel(skills, disabled_skills, selected, width, height)
    else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn plugin_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    pub(crate) fn toggle_plugin_skill(&mut self, selected: usize) {
        let Some((name, _)) = self
            .skills
            .get(selected.min(self.skills.len().saturating_sub(1)))
        else {
            return;
        };
        let name = name.clone();
        if !self.disabled_skills.remove(&name) {
            self.disabled_skills.insert(name);
        }
        save_disabled_skills(&self.disabled_skills);
    }

    pub(crate) fn handle_plugins_mouse(&mut self, mouse: &MouseEvent) {
        let Some(selected) = self.plugins_panel else {
            return;
        };
        let total = self.skills.len();
        if total == 0 {
            return;
        }
        let selected = selected.min(total - 1);
        let width = (self.width as usize).min(u16::MAX as usize);
        let height = self.height as usize;
        let Some((mut panel, panel_height)) =
            plugin_panel(&self.skills, &self.disabled_skills, selected, width, height)
        else {
            return;
        };
        let row_count = panel.view(width as u16, panel_height).lines().count();
        if row_count == 0 {
            return;
        }
        let y_offset = plugin_overlay_y_offset(height, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                self.plugins_panel = Some(index.min(total - 1));
                self.toggle_plugin_skill(index);
            }
            Some(MenuPanelMsg::Cancelled) => self.plugins_panel = None,
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    self.plugins_panel = Some(after);
                }
            }
        }
    }

    /// `/plugin` panel: enable/disable Claude skills (checkbox list, scrolled).
    pub(crate) fn overlay_plugins(&self, composed: String) -> String {
        let Some(sel) = self.plugins_panel else {
            return composed;
        };
        let total = self.skills.len();
        if total == 0 {
            return composed;
        }
        let sel = sel.min(total - 1);
        let width = self.width as usize;
        let menu = plugin_panel_lines(
            &self.skills,
            &self.disabled_skills,
            sel,
            width,
            self.height as usize,
        );
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plugin_panel_uses_bounded_shared_menu_items() {
        let skills = vec![(
            "very-long-skill-name-that-keeps-going".to_string(),
            "a very long skill description that should never overflow the overlay width"
                .to_string(),
        )];
        let disabled = HashSet::new();
        for width in [28, 44, 80] {
            let lines = plugin_panel_lines(&skills, &disabled, 0, width, 24);
            assert!(
                lines
                    .iter()
                    .all(|line| a3s_tui::style::visible_len(line) <= width),
                "plugin panel should fit width {width}: {:?}",
                lines
                    .iter()
                    .map(|line| a3s_tui::style::strip_ansi(line))
                    .collect::<Vec<_>>()
            );
        }
    }

    #[test]
    fn plugin_panel_scrolls_and_marks_disabled_skills() {
        let skills = (0..16)
            .map(|idx| (format!("skill-{idx}"), format!("description {idx}")))
            .collect::<Vec<_>>();
        let disabled = HashSet::from(["skill-14".to_string()]);
        let lines = plugin_panel_lines(&skills, &disabled, 14, 40, 18);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>();

        assert!(
            plain.iter().any(|line| line.contains("[ ] /skill-14")),
            "{plain:?}"
        );
        assert!(
            plain.iter().any(|line| line.contains("↑↓ 15/16")),
            "{plain:?}"
        );
    }

    #[test]
    fn plugin_panel_click_toggles_checkbox_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let skills = vec![
            ("inspect".to_string(), "inspect surfaces".to_string()),
            ("review".to_string(), "review changes".to_string()),
        ];
        let disabled = HashSet::new();
        let row_count = plugin_panel_lines(&skills, &disabled, 0, 48, 20).len();
        let y_offset = plugin_overlay_y_offset(20, row_count, 5);
        let (mut panel, _) = plugin_panel(&skills, &disabled, 0, 48, 20).expect("panel");
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: y_offset + 1,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, Some(MenuPanelMsg::Toggled(0)));
    }

    #[test]
    fn plugin_panel_mouse_wheel_moves_selection() {
        use a3s_tui::event::MouseEventKind;

        let skills = vec![
            ("inspect".to_string(), "inspect surfaces".to_string()),
            ("review".to_string(), "review changes".to_string()),
        ];
        let disabled = HashSet::new();
        let row_count = plugin_panel_lines(&skills, &disabled, 0, 48, 20).len();
        let y_offset = plugin_overlay_y_offset(20, row_count, 5);
        let (mut panel, _) = plugin_panel(&skills, &disabled, 0, 48, 20).expect("panel");
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: y_offset + 1,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, None);
        assert_eq!(panel.selected_index(), 1);
    }

    #[test]
    fn plugin_overlay_moves_above_dynamic_composer_rows() {
        assert_eq!(plugin_overlay_y_offset(24, 6, 5), 13);
        assert_eq!(plugin_overlay_y_offset(24, 6, 9), 9);
    }
}
