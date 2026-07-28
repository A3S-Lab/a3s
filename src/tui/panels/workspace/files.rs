//! `@` file picker: an IDE-style collapsible tree (folders expand on demand).

use super::super::*;
use super::spf;
use a3s_tui::components::{TreePicker, TreePickerItem, TreePickerMsg};
use a3s_tui::event::MouseEvent;

fn file_menu_max_items(height: usize) -> usize {
    height.saturating_sub(9).clamp(4, 12)
}

fn file_menu_panel<F>(
    nodes: &[(String, usize, bool)],
    selected: usize,
    height: usize,
    dir_is_open: F,
) -> Option<(TreePicker, usize)>
where
    F: Fn(&str) -> bool,
{
    let total = nodes.len();
    if total == 0 {
        return None;
    }

    let selected = selected.min(total - 1);
    let max_items = file_menu_max_items(height);
    let scroll = selected.saturating_add(1).saturating_sub(max_items);
    let items = nodes
        .iter()
        .map(|(path, depth, is_dir)| {
            let name = path.rsplit('/').next().unwrap_or(path);
            if *is_dir {
                let open = dir_is_open(path);
                let icon = spf::file_icon(name, true, open);
                TreePickerItem::branch(format!("{} {name}/", icon.glyph))
                    .depth(*depth)
                    .open(open)
                    .color(icon.color)
            } else {
                let icon = spf::file_icon(name, false, false);
                TreePickerItem::leaf(format!("{} {name}", icon.glyph))
                    .depth(*depth)
                    .color(icon.color)
            }
        })
        .collect::<Vec<_>>();

    let panel = TreePicker::new("@ file · ↑/↓ · →/← folder · Enter · Esc")
        .items(items)
        .selected(selected)
        .scroll(scroll)
        .max_items(max_items)
        .show_scroll(total > max_items)
        .indent(2)
        .depth_indent(2)
        .markers("▾", "▸", " ")
        .title_color(ACCENT)
        .branch_color(TN_CYAN)
        .leaf_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED);
    Some((panel, max_items + 2))
}

fn file_menu_lines<F>(
    nodes: &[(String, usize, bool)],
    selected: usize,
    width: usize,
    height: usize,
    dir_is_open: F,
) -> Vec<String>
where
    F: Fn(&str) -> bool,
{
    if width == 0 {
        return Vec::new();
    }
    let Some((panel, panel_height)) = file_menu_panel(nodes, selected, height, dir_is_open) else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, panel_height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn file_menu_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    /// The `@<query>` after the last `@` in the input (no whitespace), if any.
    pub(crate) fn at_query(&self) -> Option<String> {
        let val = self.textarea.value();
        let at = val.rfind('@')?;
        let after = &val[at + 1..];
        if after.contains(char::is_whitespace) {
            None
        } else {
            Some(after.to_string())
        }
    }

    /// Visible tree nodes `(path, depth, is_dir)`: directories are collapsed
    /// unless in `at_expanded`; a non-empty query auto-opens dirs holding a
    /// match so it behaves like a filter. ponytail: rescans `files` per call —
    /// capped at 2000 matches, switch to a cached tree if large workspaces lag.
    pub(crate) fn at_nodes(&self) -> Vec<(String, usize, bool)> {
        let q = self.at_query().unwrap_or_default().to_lowercase();
        let matches: Vec<&String> = self
            .files
            .iter()
            .filter(|f| q.is_empty() || f.to_lowercase().contains(&q))
            .take(2000)
            .collect();
        let is_open = |dir: &str| -> bool {
            self.at_expanded.contains(dir)
                || (!q.is_empty()
                    && matches
                        .iter()
                        .any(|f| f.starts_with(dir) && f.as_bytes().get(dir.len()) == Some(&b'/')))
        };
        let mut nodes: Vec<(String, usize, bool)> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for f in &matches {
            let parts: Vec<&str> = f.split('/').collect();
            let mut visible = true;
            for d in 0..parts.len() - 1 {
                let dir = parts[..=d].join("/");
                let parent_open = d == 0 || is_open(&parts[..d].join("/"));
                if parent_open && seen.insert(dir.clone()) {
                    nodes.push((dir.clone(), d, true));
                }
                if !is_open(&dir) {
                    visible = false;
                    break;
                }
            }
            if visible {
                nodes.push(((*f).clone(), parts.len() - 1, false));
            }
        }
        nodes.sort_by(|a, b| a.0.cmp(&b.0)); // path order = tree order
        nodes
    }

    fn dir_is_open(&self, dir: &str) -> bool {
        self.at_expanded.contains(dir) || self.at_query().is_some_and(|q| !q.is_empty())
    }

    pub(crate) fn file_menu_open(&self) -> bool {
        self.state != State::Awaiting
            && !self.textarea.value().contains('\n')
            && self.at_query().is_some()
            && !self.files.is_empty()
    }

    /// Keys while the picker is open: ↑/↓ move, →/← expand/collapse a folder,
    /// Enter toggles a folder or inserts a file, Esc dismisses.
    pub(crate) fn handle_file_key(&mut self, key: &KeyEvent) -> Option<Option<Cmd<Msg>>> {
        let nodes = self.at_nodes();
        if nodes.is_empty() {
            return None;
        }
        let last = nodes.len() - 1;
        self.file_sel = self.file_sel.min(last);
        let (path, _, is_dir) = nodes[self.file_sel].clone();
        match key.code {
            KeyCode::Up => {
                self.file_sel = self.file_sel.saturating_sub(1);
                Some(None)
            }
            KeyCode::Down => {
                self.file_sel = (self.file_sel + 1).min(last);
                Some(None)
            }
            KeyCode::Right if is_dir => {
                self.at_expanded.insert(path);
                Some(None)
            }
            KeyCode::Left if is_dir => {
                self.at_expanded.remove(&path);
                Some(None)
            }
            KeyCode::Enter | KeyCode::Tab => {
                if is_dir {
                    self.toggle_file_picker_dir(&path);
                } else {
                    self.insert_file_reference(&path);
                }
                Some(None)
            }
            KeyCode::Esc => {
                let val = self.textarea.value();
                if let Some(at) = val.rfind('@') {
                    self.textarea.set_value(&val[..at]);
                }
                self.file_sel = 0;
                Some(None)
            }
            _ => None,
        }
    }

    pub(crate) fn handle_file_mouse(&mut self, mouse: &MouseEvent) {
        if !self.file_menu_open() {
            return;
        }
        let nodes = self.at_nodes();
        let total = nodes.len();
        if total == 0 {
            return;
        }
        let selected = self.file_sel.min(total - 1);
        let height = self.height as usize;
        let width = (self.width as usize).min(u16::MAX as usize);
        let Some((mut panel, panel_height)) =
            file_menu_panel(&nodes, selected, height, |path| self.dir_is_open(path))
        else {
            return;
        };
        let row_count = panel.view(width as u16, panel_height).lines().count();
        if row_count == 0 {
            return;
        }
        let y_offset = file_menu_overlay_y_offset(height, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return;
        }

        panel.set_y_offset(y_offset);
        let before = panel.selected_index();
        match panel.handle_mouse(mouse) {
            Some(TreePickerMsg::Selected(index)) | Some(TreePickerMsg::Toggled(index)) => {
                self.activate_file_picker_node(&nodes, index);
            }
            Some(TreePickerMsg::Opened(index)) => {
                self.set_file_picker_dir_open(&nodes, index, true);
            }
            Some(TreePickerMsg::Closed(index)) => {
                self.set_file_picker_dir_open(&nodes, index, false);
            }
            Some(TreePickerMsg::Cancelled) => self.clear_file_picker_query(),
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    self.file_sel = after;
                }
            }
        }
    }

    fn activate_file_picker_node(&mut self, nodes: &[(String, usize, bool)], index: usize) {
        let Some((path, _, is_dir)) = nodes.get(index).cloned() else {
            return;
        };
        self.file_sel = index.min(nodes.len().saturating_sub(1));
        if is_dir {
            self.toggle_file_picker_dir(&path);
        } else {
            self.insert_file_reference(&path);
        }
    }

    fn set_file_picker_dir_open(
        &mut self,
        nodes: &[(String, usize, bool)],
        index: usize,
        open: bool,
    ) {
        let Some((path, _, true)) = nodes.get(index) else {
            return;
        };
        self.file_sel = index.min(nodes.len().saturating_sub(1));
        if open {
            self.at_expanded.insert(path.clone());
        } else {
            self.at_expanded.remove(path);
        }
    }

    fn toggle_file_picker_dir(&mut self, path: &str) {
        if !self.at_expanded.remove(path) {
            self.at_expanded.insert(path.to_string());
        }
    }

    fn insert_file_reference(&mut self, path: &str) {
        let val = self.textarea.value().to_string();
        if let Some(at) = val.rfind('@') {
            self.touch_workspace_file(path);
            self.textarea.set_value(&format!("{}@{path} ", &val[..at]));
            self.file_sel = 0;
        }
    }

    fn clear_file_picker_query(&mut self) {
        let val = self.textarea.value().to_string();
        if let Some(at) = val.rfind('@') {
            self.textarea.set_value(&val[..at]);
        }
        self.file_sel = 0;
    }

    /// Overlay the `@` file tree just above the input box.
    pub(crate) fn overlay_file_menu(&self, composed: String) -> String {
        if !self.file_menu_open() {
            return composed;
        }
        let nodes = self.at_nodes();
        let total = nodes.len();
        if total == 0 {
            return composed;
        }
        let sel = self.file_sel.min(total - 1);
        let width = self.width as usize;
        let menu = file_menu_lines(&nodes, sel, width, self.height as usize, |path| {
            self.dir_is_open(path)
        });
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn file_menu_lines_use_bounded_tree_picker_rows() {
        let nodes = vec![
            ("src".to_string(), 0, true),
            ("src/tui".to_string(), 1, true),
            ("src/tui/mod.rs".to_string(), 2, false),
            ("README.md".to_string(), 0, false),
        ];
        let open = HashSet::from(["src".to_string()]);
        let lines = file_menu_lines(&nodes, 2, 32, 20, |path| open.contains(path));
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("@ file"), "{plain}");
        assert!(plain.contains("▾ ◈ src/"), "{plain}");
        assert!(plain.contains("▸ ◆ tui/"), "{plain}");
        assert!(plain.contains("◈ mod.rs"), "{plain}");
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 32),
            "{plain}"
        );
        assert!(
            lines.iter().any(|line| line.contains("\x1b[")),
            "tree picker rows should carry styling"
        );
    }

    #[test]
    fn file_menu_lines_scrolls_selected_item_into_view() {
        let nodes = (0..16)
            .map(|idx| (format!("file-{idx}.rs"), 0, false))
            .collect::<Vec<_>>();
        let lines = file_menu_lines(&nodes, 14, 36, 16, |_| false);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("file-14.rs"), "{plain}");
        assert!(plain.contains("↑↓ 15/16"), "{plain}");
    }

    #[test]
    fn file_menu_panel_handles_mouse_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let nodes = vec![
            ("src".to_string(), 0, true),
            ("src/main.rs".to_string(), 1, false),
        ];
        let row_count = file_menu_lines(&nodes, 0, 40, 20, |_| true).len();
        let y_offset = file_menu_overlay_y_offset(20, row_count, 5);
        let (mut panel, _) = file_menu_panel(&nodes, 0, 20, |_| true).expect("panel");
        panel.set_y_offset(y_offset);

        let branch = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: y_offset + 1,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });
        assert_eq!(branch, Some(TreePickerMsg::Toggled(0)));

        let file = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 4,
            row: y_offset + 2,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });
        assert_eq!(file, Some(TreePickerMsg::Selected(1)));
    }

    #[test]
    fn file_menu_panel_mouse_wheel_moves_selection() {
        use a3s_tui::event::MouseEventKind;

        let nodes = vec![
            ("README.md".to_string(), 0, false),
            ("src/main.rs".to_string(), 0, false),
        ];
        let row_count = file_menu_lines(&nodes, 0, 40, 20, |_| false).len();
        let y_offset = file_menu_overlay_y_offset(20, row_count, 5);
        let (mut panel, _) = file_menu_panel(&nodes, 0, 20, |_| false).expect("panel");
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
    fn file_overlay_moves_above_dynamic_composer_rows() {
        assert_eq!(file_menu_overlay_y_offset(24, 6, 5), 13);
        assert_eq!(file_menu_overlay_y_offset(24, 6, 10), 8);
    }
}
