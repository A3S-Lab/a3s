//! `/effort` overlay: the effort slider (incl. the ultracode flourish).

use super::super::*;
use a3s_tui::components::{LevelSlider, LevelSliderMsg, ShimmerText, SliderLevel};
use a3s_tui::event::MouseEvent;

fn effort_slider(selected: usize) -> LevelSlider {
    let level_colors = [
        TN_GRAY,
        TN_CYAN,
        ACCENT,
        TN_YELLOW,
        BRAND_GRADIENT[4],
        BRAND_GRADIENT[6],
    ];
    let levels = EFFORT_LEVELS
        .iter()
        .enumerate()
        .map(|(index, profile)| {
            let description = if index == ULTRACODE {
                "ultracode: plans, then fans independent work out to parallel subagents."
            } else {
                "higher effort = deeper reasoning + stricter verification + longer tool budget (slower)."
            };
            SliderLevel::new(profile.label)
                .description(description)
                .color(level_colors[index.min(level_colors.len() - 1)])
        })
        .collect::<Vec<_>>();

    LevelSlider::new(levels)
        .title("Effort")
        .range_labels("Faster", "Smarter")
        .selected(selected)
        .separator_after(ULTRACODE.saturating_sub(1))
        .margin(PAD)
        .marker('▲')
        .separator_char('┆')
        .pointer("▸")
        .title_color(ACCENT)
        .selected_color(ACCENT)
        .track_color(TN_FG)
        .muted_color(TN_GRAY)
        .hint("←/→ or wheel/click adjust · Enter confirm · Esc cancel")
}

fn effort_slider_lines(selected: usize, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    effort_slider(selected)
        .view(width.min(u16::MAX as usize) as u16)
        .lines()
        .map(str::to_string)
        .collect()
}

fn effort_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

const ULTRACODE_CARD_ROWS: usize = 7;
const ULTRACODE_INNER_ROWS: usize = ULTRACODE_CARD_ROWS - 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum UltracodeRipplePhase {
    Charge,
    Expand,
    EdgeBloom,
    Settle,
}

fn ultracode_ripple_phase(frame: usize) -> UltracodeRipplePhase {
    match frame {
        0..=2 => UltracodeRipplePhase::Charge,
        3..=14 => UltracodeRipplePhase::Expand,
        15..=16 => UltracodeRipplePhase::EdgeBloom,
        _ => UltracodeRipplePhase::Settle,
    }
}

fn ultracode_ripple_radius(frame: usize) -> f64 {
    match ultracode_ripple_phase(frame) {
        UltracodeRipplePhase::Charge => frame as f64 * 0.035,
        UltracodeRipplePhase::Expand => {
            let progress = frame.saturating_sub(3) as f64 / 11.0;
            let eased = 1.0 - (1.0 - progress).powi(2);
            0.06 + eased * 1.16
        }
        UltracodeRipplePhase::EdgeBloom => 1.24 + frame.saturating_sub(15) as f64 * 0.07,
        UltracodeRipplePhase::Settle => 1.40 + frame.saturating_sub(17).min(2) as f64 * 0.09,
    }
}

fn ultracode_animation_lines(frame: usize, width: usize) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    if width < 16 {
        return compact_ultracode_animation_lines(frame, width);
    }

    // Keep the activation inside the same seven-row footprint as the effort
    // picker while using the entire terminal width.
    let panel_width = width;
    let inner_width = panel_width.saturating_sub(2);
    let title_text = if inner_width >= 28 {
        "✦  U L T R A C O D E  ✦"
    } else if inner_width >= 15 {
        "✦ ULTRACODE ✦"
    } else {
        "ULTRACODE"
    };
    let title = ShimmerText::new(title_text)
        .phase(frame)
        .colors(BRAND_GRADIENT[6], TN_FG)
        .spread(4.0)
        .speed_divisor(1)
        .cycle_gap(7)
        .view();
    let status_text = if inner_width >= 40 {
        "preparing workflow · parallel agents"
    } else if inner_width >= 26 {
        "workflow · parallel agents"
    } else if inner_width >= 15 {
        "parallel agents"
    } else {
        "preparing"
    };
    let status = Style::new()
        .fg(BRAND_GRADIENT[(frame / 2 + 3) % BRAND_GRADIENT.len()])
        .dim()
        .render(status_text);

    let mut lines = Vec::with_capacity(ULTRACODE_CARD_ROWS);
    lines.push(center_panel_line(
        &ultracode_border_line(frame, inner_width, true),
        width,
    ));
    for row in 0..ULTRACODE_INNER_ROWS {
        let canvas = if row == 1 {
            ultracode_ripple_row_with_text(frame, row, inner_width, &title)
        } else if row == 3 {
            ultracode_ripple_row_with_text(frame, row, inner_width, &status)
        } else {
            ultracode_ripple_row(frame, row, inner_width)
        };
        let side = ultracode_side_border_style(frame, row, inner_width);
        let content = format!("{}{}{}", side.clone().render("│"), canvas, side.render("│"));
        lines.push(center_panel_line(&content, width));
    }
    lines.push(center_panel_line(
        &ultracode_border_line(frame, inner_width, false),
        width,
    ));
    lines
}

fn compact_ultracode_animation_lines(frame: usize, width: usize) -> Vec<String> {
    let phase = ultracode_ripple_phase(frame);
    let ripple = match phase {
        UltracodeRipplePhase::Charge => ["·", "✦", "◆"][frame.min(2)],
        UltracodeRipplePhase::Expand => "·─━✦━─·",
        UltracodeRipplePhase::EdgeBloom => "◦━◆✦◆━◦",
        UltracodeRipplePhase::Settle => "· ◦ ✦ ◦ ·",
    };
    let styled_ripple = Style::new()
        .fg(BRAND_GRADIENT[(frame / 2) % BRAND_GRADIENT.len()])
        .bold()
        .render(ripple);
    let title = if width >= 9 { "ULTRACODE" } else { "" };
    vec![
        " ".repeat(width),
        " ".repeat(width),
        center_visible_line(title, width),
        center_visible_line(&styled_ripple, width),
        center_visible_line(if width >= 9 { "preparing" } else { "" }, width),
        " ".repeat(width),
        " ".repeat(width),
    ]
}

fn ultracode_ripple_row(frame: usize, row: usize, width: usize) -> String {
    render_styled_cells(ultracode_ripple_cells(frame, row, width))
}

fn ultracode_ripple_row_with_text(frame: usize, row: usize, width: usize, text: &str) -> String {
    let text = a3s_tui::style::truncate_visible(text, width);
    let text_width = a3s_tui::style::visible_len(&text);
    let start = width.saturating_sub(text_width) / 2;
    let end = start.saturating_add(text_width);
    let mut cells = ultracode_ripple_cells(frame, row, width);
    let split = end.min(cells.len());
    let suffix = cells.split_off(split);
    cells.truncate(start);
    format!(
        "{}{}{}",
        render_styled_cells(cells),
        text,
        render_styled_cells(suffix)
    )
}

fn ultracode_ripple_cells(frame: usize, row: usize, width: usize) -> Vec<(char, Style)> {
    if width == 0 {
        return Vec::new();
    }

    let center_x = (width.saturating_sub(1)) as f64 / 2.0;
    let half_x = center_x.max(1.0);
    let center_y = (ULTRACODE_INNER_ROWS.saturating_sub(1)) as f64 / 2.0;
    let half_y = center_y.max(1.0);
    let radius = ultracode_ripple_radius(frame);
    let x_step = 1.0 / half_x;
    let main_tolerance = (x_step * 0.72).max(0.055);
    let mut cells = Vec::with_capacity(width);

    for column in 0..width {
        let dx = (column as f64 - center_x) / half_x;
        let dy = (row as f64 - center_y) / half_y;
        let distance = (dx * dx + dy * dy).sqrt();
        let color_index = ((column * BRAND_GRADIENT.len() / width.max(1)) + row * 2 + frame / 2)
            % BRAND_GRADIENT.len();
        let main = (distance - radius).abs() <= main_tolerance;
        let echo_radius = radius - 0.20;
        let echo = echo_radius > 0.0 && (distance - echo_radius).abs() <= main_tolerance * 0.82;
        let tail_radius = radius - 0.36;
        let tail = tail_radius > 0.0 && (distance - tail_radius).abs() <= main_tolerance * 0.68;
        let is_center = row == ULTRACODE_INNER_ROWS / 2 && (column as f64 - center_x).abs() <= 0.5;

        let cell = if is_center && frame <= 5 {
            let glyph = match frame {
                0 => '·',
                1 => '✦',
                2 => '◆',
                _ => '✦',
            };
            (glyph, Style::new().fg(TN_FG).bold())
        } else if main {
            let glyph = if row == ULTRACODE_INNER_ROWS / 2 {
                '━'
            } else {
                '•'
            };
            (glyph, Style::new().fg(BRAND_GRADIENT[color_index]).bold())
        } else if echo {
            let glyph = if row == ULTRACODE_INNER_ROWS / 2 {
                '─'
            } else {
                '◦'
            };
            (
                glyph,
                Style::new()
                    .fg(BRAND_GRADIENT
                        [(color_index + BRAND_GRADIENT.len() - 1) % BRAND_GRADIENT.len()]),
            )
        } else if tail {
            let glyph = if row == ULTRACODE_INNER_ROWS / 2 {
                '┈'
            } else {
                '·'
            };
            (
                glyph,
                Style::new()
                    .fg(BRAND_GRADIENT
                        [(color_index + BRAND_GRADIENT.len() - 2) % BRAND_GRADIENT.len()])
                    .dim(),
            )
        } else if frame >= 5
            && distance + 0.10 < radius
            && (column * 17 + row * 31 + frame * 7).is_multiple_of(37)
        {
            (
                '·',
                Style::new()
                    .fg(BRAND_GRADIENT[(color_index + 3) % BRAND_GRADIENT.len()])
                    .dim(),
            )
        } else {
            (' ', Style::new())
        };
        cells.push(cell);
    }

    cells
}

fn ultracode_border_line(frame: usize, inner_width: usize, top: bool) -> String {
    let muted = Style::new().fg(BORDER_SUBTLE).dim();
    let (left, right) = if top { ('╭', '╮') } else { ('╰', '╯') };
    let mut cells = Vec::with_capacity(inner_width.saturating_add(2));
    cells.push((left, muted.clone()));

    let center_x = (inner_width.saturating_sub(1)) as f64 / 2.0;
    let half_x = center_x.max(1.0);
    let radius = ultracode_ripple_radius(frame);
    for column in 0..inner_width {
        let dx = (column as f64 - center_x) / half_x;
        let distance = (dx * dx + 1.18_f64.powi(2)).sqrt();
        let contact = (distance - radius).abs() <= 0.09;
        let color = BRAND_GRADIENT[(column * BRAND_GRADIENT.len() / inner_width.max(1)
            + frame / 2)
            % BRAND_GRADIENT.len()];
        let style = if contact {
            Style::new().fg(color).bold()
        } else {
            muted.clone()
        };
        cells.push(('─', style));
    }
    cells.push((right, muted));
    render_styled_cells(cells)
}

fn ultracode_side_border_style(frame: usize, row: usize, inner_width: usize) -> Style {
    let center_y = (ULTRACODE_INNER_ROWS.saturating_sub(1)) as f64 / 2.0;
    let half_y = center_y.max(1.0);
    let dy = (row as f64 - center_y) / half_y;
    let distance = (1.08_f64.powi(2) + dy * dy).sqrt();
    if (distance - ultracode_ripple_radius(frame)).abs() <= 0.10 {
        Style::new()
            .fg(BRAND_GRADIENT[(row * 2 + inner_width + frame / 2) % BRAND_GRADIENT.len()])
            .bold()
    } else {
        Style::new().fg(BORDER_SUBTLE).dim()
    }
}

fn render_styled_cells(cells: Vec<(char, Style)>) -> String {
    let mut output = String::new();
    let mut run = String::new();
    let mut run_style: Option<Style> = None;

    for (glyph, style) in cells {
        if run_style.as_ref().is_some_and(|current| current != &style) {
            if let Some(current) = run_style.take() {
                output.push_str(&current.render(&run));
            }
            run.clear();
        }
        if run_style.is_none() {
            run_style = Some(style);
        }
        run.push(glyph);
    }
    if let Some(style) = run_style {
        output.push_str(&style.render(&run));
    }
    output
}

fn center_panel_line(rendered: &str, width: usize) -> String {
    let visible = a3s_tui::style::visible_len(rendered).min(width);
    let left = width.saturating_sub(visible) / 2;
    let right = width.saturating_sub(visible).saturating_sub(left);
    format!(
        "{}{}{}",
        " ".repeat(left),
        a3s_tui::style::fit_visible(rendered, visible),
        " ".repeat(right)
    )
}

fn center_visible_line(rendered: &str, width: usize) -> String {
    center_panel_line(rendered, width)
}

impl App {
    pub(crate) fn confirm_effort_selection(&mut self, selected: usize) -> Option<Cmd<Msg>> {
        let selected = selected.min(EFFORT_LEVELS.len().saturating_sub(1));
        if selected == ULTRACODE {
            // Play the activation flourish at a dedicated frame rate, then
            // close and rebuild the session. The successful rebuild starts the
            // second-stage animated composer ribbon.
            self.effort_panel = Some(selected);
            self.effort_anim = Some(Instant::now());
            self.gradient_until = None;
            self.gradient_frame = 0;
            let epoch = advance_ultracode_animation_epoch(&mut self.ultracode_animation_epoch);
            Some(ultracode_tick(epoch))
        } else {
            self.effort_anim = None;
            self.gradient_until = None;
            self.effort_panel = None;
            advance_ultracode_animation_epoch(&mut self.ultracode_animation_epoch);
            self.apply_effort(selected)
        }
    }

    pub(crate) fn handle_effort_mouse(&mut self, mouse: &MouseEvent) {
        let Some(selected) = self.effort_panel else {
            return;
        };
        if self.effort_anim.is_some() {
            return;
        }
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return;
        }
        let mut slider = effort_slider(selected);
        let row_count = slider.view(width as u16).lines().count();
        if row_count == 0 {
            return;
        }
        let y_offset =
            effort_overlay_y_offset(self.height as usize, row_count, self.overlay_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return;
        }
        slider.set_y_offset(y_offset);
        let before = slider.selected_value();

        match slider.handle_mouse(mouse, width as u16) {
            Some(LevelSliderMsg::Selected(index)) => {
                self.effort_panel = Some(index.min(EFFORT_LEVELS.len().saturating_sub(1)));
            }
            None => {
                let after = slider
                    .selected_value()
                    .min(EFFORT_LEVELS.len().saturating_sub(1));
                if after != before {
                    self.effort_panel = Some(after);
                }
            }
        }
    }

    pub(crate) fn overlay_effort(&self, composed: String) -> String {
        let Some(sel) = self.effort_panel else {
            return composed;
        };
        let width = self.width as usize;
        // Ultracode confirm flourish: a focused radial ripple inside the
        // picker footprint, followed by the composer ribbon after rebuild.
        if self.effort_anim.is_some() {
            let menu = ultracode_animation_lines(self.gradient_frame, width);
            return self.overlay_list(composed, &menu);
        }
        let menu = effort_slider_lines(sel, width);
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_slider_lines_are_width_bounded() {
        let lines = effort_slider_lines(ULTRACODE, 30);

        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 30),
            "{:?}",
            lines
                .iter()
                .map(|line| a3s_tui::style::strip_ansi(line))
                .collect::<Vec<_>>()
        );
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(plain.starts_with("Effort"), "{plain}");
        assert!(plain.contains("Faster"), "{plain}");
        assert!(plain.contains("Smarter"), "{plain}");
        assert!(plain.contains('▲'), "{plain}");
        assert!(plain.contains('┆'), "{plain}");
        assert!(plain.contains("▸ ultracode"), "{plain}");
    }

    #[test]
    fn effort_slider_clamps_selected_index() {
        let plain = effort_slider_lines(usize::MAX, 48)
            .into_iter()
            .map(|line| a3s_tui::style::strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("▸ ultracode"), "{plain}");
    }

    #[test]
    fn effort_slider_mouse_wheel_moves_selection() {
        use a3s_tui::event::MouseEventKind;

        let width = 48;
        let row_count = effort_slider_lines(0, width).len();
        let y_offset = effort_overlay_y_offset(24, row_count, 5);
        let mut slider = effort_slider(0);
        slider.set_y_offset(y_offset);

        let msg = slider.handle_mouse(
            &MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 0,
                row: y_offset + 2,
                modifiers: a3s_tui::KeyModifiers::NONE,
            },
            width as u16,
        );

        assert_eq!(msg, None);
        assert_eq!(
            slider.selected_value(),
            1.min(EFFORT_LEVELS.len().saturating_sub(1))
        );
    }

    #[test]
    fn effort_slider_click_selects_level_at_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let width = 48;
        let row_count = effort_slider_lines(0, width).len();
        let y_offset = effort_overlay_y_offset(24, row_count, 5);
        let mut slider = effort_slider(0);
        slider.set_y_offset(y_offset);

        let msg = slider.handle_mouse(
            &MouseEvent {
                kind: MouseEventKind::Down(MouseButton::Left),
                column: 21,
                row: y_offset + 2,
                modifiers: a3s_tui::KeyModifiers::NONE,
            },
            width as u16,
        );

        assert_eq!(msg, Some(LevelSliderMsg::Selected(2)));
        assert_eq!(slider.selected_value(), 2);
    }

    #[test]
    fn effort_mouse_offset_follows_dynamic_rows_below() {
        assert_eq!(effort_overlay_y_offset(24, 8, 5), 11);
        assert_eq!(effort_overlay_y_offset(24, 8, 9), 7);
    }

    #[test]
    fn ultracode_animation_is_a_bounded_radial_card() {
        let lines = ultracode_animation_lines(9, 80);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");
        assert_eq!(lines.len(), ULTRACODE_CARD_ROWS);
        let top = a3s_tui::style::strip_ansi(&lines[0]);
        let middle = a3s_tui::style::strip_ansi(&lines[ULTRACODE_CARD_ROWS / 2]);
        assert!(top.starts_with('╭') && top.ends_with('╮'), "{top:?}");
        assert!(
            middle.starts_with('│') && middle.ends_with('│'),
            "{middle:?}"
        );
        assert!(plain.contains("U L T R A C O D E"), "{plain}");
        assert!(plain.contains("workflow"), "{plain}");
        assert!(plain.contains("parallel agents"), "{plain}");
        assert!(plain.contains('╭') && plain.contains('╯'), "{plain}");
        assert!(
            plain
                .lines()
                .filter(|line| line.contains('•') || line.contains('◦'))
                .count()
                >= 2,
            "{plain}"
        );
        assert!(lines.iter().any(|line| line.contains("\x1b[")));
        assert!(
            BRAND_GRADIENT
                .iter()
                .filter(|color| lines.join("\n").contains(&color.fg_ansi()))
                .count()
                >= 4
        );
        assert!(lines
            .iter()
            .all(|line| a3s_tui::style::visible_len(line) == 80));
    }

    #[test]
    fn ultracode_animation_keeps_seven_rows_at_terminal_widths() {
        assert!(ultracode_animation_lines(8, 0).is_empty());

        for width in [1, 8, 15, 16, 27, 32, 48, 80, 120] {
            let lines = ultracode_animation_lines(8, width);
            assert_eq!(lines.len(), ULTRACODE_CARD_ROWS, "width {width}");
            assert!(
                lines
                    .iter()
                    .all(|line| a3s_tui::style::visible_len(line) == width),
                "width {width}: {:?}",
                lines
                    .iter()
                    .map(|line| a3s_tui::style::strip_ansi(line))
                    .collect::<Vec<_>>()
            );
            if width >= 32 {
                assert_eq!(
                    lines.len(),
                    effort_slider_lines(ULTRACODE, width).len(),
                    "picker footprint changed at width {width}"
                );
            }
        }
    }

    #[test]
    fn ultracode_ripple_has_charge_expand_bloom_and_settle_phases() {
        assert_eq!(ultracode_ripple_phase(1), UltracodeRipplePhase::Charge);
        assert_eq!(ultracode_ripple_phase(8), UltracodeRipplePhase::Expand);
        assert_eq!(ultracode_ripple_phase(15), UltracodeRipplePhase::EdgeBloom);
        assert_eq!(ultracode_ripple_phase(18), UltracodeRipplePhase::Settle);
        assert!(ultracode_ripple_radius(3) < ultracode_ripple_radius(8));
        assert!(ultracode_ripple_radius(8) < ultracode_ripple_radius(14));
        assert!(ultracode_ripple_radius(14) < ultracode_ripple_radius(18));

        let charge = ultracode_animation_lines(1, 80);
        let expand = ultracode_animation_lines(8, 80);
        let bloom = ultracode_animation_lines(15, 80);
        let settle = ultracode_animation_lines(18, 80);
        assert_ne!(charge, expand);
        assert_ne!(expand, bloom);
        assert_ne!(bloom, settle);
    }
}
