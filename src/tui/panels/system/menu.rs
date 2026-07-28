//! `/` slash command menu + the shared overlay-list primitive.

use super::super::*;
use a3s_tui::components::TextOverlay;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEvent;

fn slash_menu_lines(
    candidates: &[(String, String)],
    selected: usize,
    width: usize,
    max_items: usize,
) -> Vec<String> {
    let Some((panel, height)) = slash_menu_panel(candidates, selected, max_items) else {
        return Vec::new();
    };

    panel
        .view(width.min(u16::MAX as usize) as u16, height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn slash_menu_panel(
    candidates: &[(String, String)],
    selected: usize,
    max_items: usize,
) -> Option<(MenuPanel, usize)> {
    if candidates.is_empty() {
        return None;
    }
    let items = candidates
        .iter()
        .map(|(cmd, desc)| {
            MenuItem::new(cmd.clone())
                .description(desc.clone())
                .color(TN_GRAY)
        })
        .collect::<Vec<_>>();
    let panel = MenuPanel::without_title()
        .items(items)
        .selected(selected)
        .max_items(max_items)
        .label_width(11)
        .indent(0)
        .marker(" ")
        .text_color(TN_GRAY)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED);
    let height = candidates.len().min(max_items).saturating_add(1);
    Some((panel, height))
}

fn slash_menu_submit_text(cmd: &str) -> String {
    if SLASH_COMMANDS
        .iter()
        .any(|(registered, _)| *registered == cmd)
    {
        cmd.to_string()
    } else {
        let name = cmd.trim_start_matches('/');
        format!("Use your `{name}` skill.")
    }
}

fn dismiss_slash_menu(input: &str, selected: &mut usize, dismissed_for: &mut Option<String>) {
    *dismissed_for = Some(input.to_string());
    *selected = 0;
}

fn slash_menu_is_dismissed(input: &str, dismissed_for: Option<&str>) -> bool {
    dismissed_for == Some(input)
}

fn overlay_menu_rows(composed: &str, menu: &[String], width: usize, rows_below: usize) -> String {
    if menu.is_empty() {
        return composed.to_string();
    }
    TextOverlay::new(menu.iter().cloned())
        .above_bottom(rows_below)
        .width(width)
        .apply(composed)
}

fn slash_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    /// Built-in commands + loaded skills (as `/<skill>`) matching `input`.
    pub(crate) fn slash_candidates_all(&self, input: &str) -> Vec<(String, String)> {
        // Hide session-mutating commands while a turn is streaming.
        let idle = self.state == State::Idle;
        let mut out: Vec<(String, String)> = slash_candidates(input)
            .into_iter()
            .filter(|(c, _)| idle || !IDLE_ONLY.contains(c))
            .filter(|(c, _)| self.os_config.is_some() || !matches!(*c, "/login" | "/logout"))
            .map(|(c, d)| (c.to_string(), d.to_string()))
            .collect();
        for (name, desc) in &self.skills {
            if self.disabled_skills.contains(name) {
                continue; // hidden via /plugin
            }
            let cmd = format!("/{name}");
            if cmd.starts_with(input) {
                out.push((cmd, format!("skill · {}", truncate(desc, 56))));
            }
        }
        out
    }

    pub(crate) fn slash_menu_open(&self) -> bool {
        let input = self.textarea.value();
        // Available while idle or streaming, but not during an approval prompt.
        self.state != State::Awaiting
            && input.starts_with('/')
            && !input.contains('\n')
            && !slash_menu_is_dismissed(&input, self.slash_menu_dismissed_for.as_deref())
            // Close once args are being typed so Enter submits the whole line
            // instead of just the command.
            && !input.contains(' ')
            && !self.slash_candidates_all(&input).is_empty()
    }

    /// Keys while the slash menu is open: ↑/↓ select, Enter run, Tab complete,
    /// Esc dismiss. Returns `Some(handled)` to consume the key.
    pub(crate) fn handle_slash_key(&mut self, key: &KeyEvent) -> Option<Option<Cmd<Msg>>> {
        let cands = self.slash_candidates_all(&self.textarea.value());
        if cands.is_empty() {
            return None;
        }
        let last = cands.len() - 1;
        self.slash_sel = self.slash_sel.min(last);
        match key.code {
            KeyCode::Up => {
                self.slash_sel = self.slash_sel.saturating_sub(1);
                Some(None)
            }
            KeyCode::Down => {
                self.slash_sel = (self.slash_sel + 1).min(last);
                Some(None)
            }
            KeyCode::Enter => {
                let cmd = cands[self.slash_sel].0.clone();
                self.slash_sel = 0;
                self.slash_menu_dismissed_for = None;
                self.textarea.clear();
                // Run directly on this key event so the redraw is immediate (a
                // Submit message would be frame-throttled and look like a no-op).
                Some(self.on_submit(slash_menu_submit_text(&cmd)))
            }
            KeyCode::Tab => {
                // Fill into the input (trailing space closes the menu) to add args.
                self.textarea
                    .set_value(&format!("{} ", cands[self.slash_sel].0));
                self.slash_sel = 0;
                self.slash_menu_dismissed_for = None;
                Some(None)
            }
            KeyCode::Esc => {
                let input = self.textarea.value();
                dismiss_slash_menu(
                    &input,
                    &mut self.slash_sel,
                    &mut self.slash_menu_dismissed_for,
                );
                Some(None)
            }
            _ => None,
        }
    }

    pub(crate) fn handle_slash_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        if !self.slash_menu_open() {
            return None;
        }
        let cands = self.slash_candidates_all(&self.textarea.value());
        if cands.is_empty() {
            return None;
        }
        let total = cands.len();
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let max_rows = (self.height as usize).saturating_sub(8).clamp(3, 10);
        let selected = self.slash_sel.min(total - 1);
        let (mut panel, panel_height) = slash_menu_panel(&cands, selected, max_rows)?;
        let row_count = panel.view(width as u16, panel_height).lines().count();
        if row_count == 0 {
            return None;
        }
        let y_offset =
            slash_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();

        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                let index = index.min(total - 1);
                let cmd = cands[index].0.clone();
                self.slash_sel = 0;
                self.slash_menu_dismissed_for = None;
                self.textarea.clear();
                self.on_submit(slash_menu_submit_text(&cmd))
            }
            Some(MenuPanelMsg::Cancelled) => {
                let input = self.textarea.value();
                dismiss_slash_menu(
                    &input,
                    &mut self.slash_sel,
                    &mut self.slash_menu_dismissed_for,
                );
                None
            }
            None => {
                let after = panel.selected_index().min(total - 1);
                if after != before {
                    self.slash_sel = after;
                }
                None
            }
        }
    }

    /// Overlay `menu` rows above the activity/composer region, with the final
    /// row replacing the ordinary transcript-to-activity spacer.
    pub(crate) fn overlay_list(&self, composed: String, menu: &[String]) -> String {
        self.overlay_list_with_rows_below(composed, menu, self.overlay_rows_below())
    }

    pub(crate) fn overlay_list_with_rows_below(
        &self,
        composed: String,
        menu: &[String],
        rows_below: usize,
    ) -> String {
        overlay_menu_rows(&composed, menu, self.width as usize, rows_below)
    }

    pub(crate) fn overlay_slash_menu(&self, composed: String) -> String {
        if !self.slash_menu_open() {
            return composed;
        }
        let cands = self.slash_candidates_all(&self.textarea.value());
        let total = cands.len();
        let sel = self.slash_sel.min(total - 1);
        let width = self.width as usize;
        // Cap the menu height (skills make the list long) and scroll a window so
        // the selection stays visible — Claude-Code style.
        let max_rows = (self.height as usize).saturating_sub(8).clamp(3, 10);
        let menu = slash_menu_lines(&cands, sel, width, max_rows);
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slash_menu_dismissal_preserves_the_exact_draft() {
        let input = "/mod".to_string();
        let mut selected = 3;
        let mut dismissed_for = None;

        dismiss_slash_menu(&input, &mut selected, &mut dismissed_for);

        assert_eq!(input, "/mod");
        assert_eq!(selected, 0);
        assert_eq!(dismissed_for.as_deref(), Some("/mod"));
        assert!(slash_menu_is_dismissed(&input, dismissed_for.as_deref()));
    }

    #[test]
    fn slash_menu_changed_input_is_not_dismissed() {
        assert!(slash_menu_is_dismissed("/mod", Some("/mod")));
        assert!(!slash_menu_is_dismissed("/mode", Some("/mod")));
        assert!(!slash_menu_is_dismissed("/mod", None));
    }

    #[test]
    fn slash_menu_lines_truncate_long_descriptions_to_width() {
        let rows = slash_menu_lines(
            &[(
                "/agent".to_string(),
                "pick an agent definition with an intentionally long explanation that must not overflow"
                    .to_string(),
            )],
            0,
            42,
            10,
        );
        let row = &rows[0];

        assert!(
            a3s_tui::style::visible_len(row) <= 42,
            "row should stay bounded: {:?}",
            a3s_tui::style::strip_ansi(row)
        );
        assert!(a3s_tui::style::strip_ansi(row).contains("/agent"));
    }

    #[test]
    fn all_registered_slash_menu_rows_fit_narrow_width() {
        let width = 48;
        let candidates = SLASH_COMMANDS
            .iter()
            .map(|(cmd, desc)| ((*cmd).to_string(), (*desc).to_string()))
            .collect::<Vec<_>>();
        let rows = slash_menu_lines(&candidates, 0, width, SLASH_COMMANDS.len());
        for row in rows {
            assert!(
                a3s_tui::style::visible_len(&row) <= width,
                "slash menu row should stay bounded: {:?}",
                a3s_tui::style::strip_ansi(&row)
            );
        }
    }

    #[test]
    fn overlay_menu_rows_uses_shared_text_overlay_positioning() {
        let frame = (0..10)
            .map(|idx| format!("row {idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        let menu = vec!["menu one".to_string(), "menu two".to_string()];
        let rendered = overlay_menu_rows(&frame, &menu, 20, 5);
        let rows = rendered.lines().collect::<Vec<_>>();

        assert_eq!(rows[2], "row 2");
        assert_eq!(rows[3].trim_end(), "menu one");
        assert_eq!(rows[4].trim_end(), "menu two");
        assert_eq!(a3s_tui::style::visible_len(rows[3]), 20);
        assert_eq!(a3s_tui::style::visible_len(rows[4]), 20);
        assert_eq!(rows[5], "row 5");
        assert_eq!(rows.len(), 10);
    }

    #[test]
    fn overlay_menu_rows_follow_dynamic_composer_chrome() {
        let frame = (0..12)
            .map(|idx| format!("row {idx}"))
            .collect::<Vec<_>>()
            .join("\n");
        let menu = vec!["menu one".to_string(), "menu two".to_string()];

        let baseline = overlay_menu_rows(&frame, &menu, 20, 5);
        let dynamic = overlay_menu_rows(&frame, &menu, 20, 8);
        let baseline_rows = baseline.lines().collect::<Vec<_>>();
        let dynamic_rows = dynamic.lines().collect::<Vec<_>>();

        assert_eq!(baseline_rows[5].trim_end(), "menu one");
        assert_eq!(dynamic_rows[2].trim_end(), "menu one");
        assert_eq!(dynamic_rows[4], "row 4");
    }

    #[test]
    fn slash_mouse_offset_uses_dynamic_rows_below() {
        assert_eq!(slash_overlay_y_offset(18, 4, 5), 9);
        assert_eq!(slash_overlay_y_offset(18, 4, 8), 6);
    }

    #[test]
    fn overlay_menu_rows_bounds_styled_rows_to_frame_width() {
        let frame = "row 0\nrow 1\nrow 2";
        let styled = Style::new().fg(TN_CYAN).render("a very long overlay row");
        let menu = vec![styled];
        let rendered = overlay_menu_rows(frame, &menu, 8, 5);
        let row = rendered.lines().next().unwrap();

        assert_eq!(a3s_tui::style::visible_len(row), 8);
        assert!(row.contains('\u{1b}'));
    }

    #[test]
    fn slash_menu_enter_submits_builtins_to_the_main_handler() {
        assert_eq!(slash_menu_submit_text("/model"), "/model");
        assert_eq!(slash_menu_submit_text("/skill"), "/skill");
        assert_eq!(slash_menu_submit_text("/agent"), "/agent");
    }

    #[test]
    fn slash_menu_enter_turns_skills_into_skill_prompts() {
        assert_eq!(
            slash_menu_submit_text("/inspect-surface"),
            "Use your `inspect-surface` skill."
        );
    }

    #[test]
    fn slash_menu_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let candidates = SLASH_COMMANDS
            .iter()
            .take(4)
            .map(|(cmd, desc)| ((*cmd).to_string(), (*desc).to_string()))
            .collect::<Vec<_>>();
        let width = 48;
        let max_rows = 4;
        let row_count = slash_menu_lines(&candidates, 0, width, max_rows).len();
        let y_offset = slash_overlay_y_offset(18, row_count, 5);
        let (mut panel, _) = slash_menu_panel(&candidates, 0, max_rows).expect("panel");
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::ScrollDown,
            column: 0,
            row: y_offset,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, None);
        assert_eq!(panel.selected_index(), 1);
    }

    #[test]
    fn slash_menu_click_selects_visible_row_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let candidates = SLASH_COMMANDS
            .iter()
            .take(4)
            .map(|(cmd, desc)| ((*cmd).to_string(), (*desc).to_string()))
            .collect::<Vec<_>>();
        let width = 48;
        let max_rows = 4;
        let row_count = slash_menu_lines(&candidates, 0, width, max_rows).len();
        let y_offset = slash_overlay_y_offset(18, row_count, 5);
        let (mut panel, _) = slash_menu_panel(&candidates, 0, max_rows).expect("panel");
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 2,
            row: y_offset + 1,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, Some(MenuPanelMsg::Selected(1)));
    }
}
