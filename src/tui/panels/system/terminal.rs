//! `/terminal` diagnostics for the active Code TUI process.

use a3s_tui::components::{DetailPanel, DetailRow};
use a3s_tui::style::Color;
use a3s_tui::{TerminalProfile, TerminalSupport};

pub(crate) fn current_terminal_diagnostic(width: usize) -> String {
    let profile = TerminalProfile::detect();
    let size = a3s_tui::terminal::Terminal::size().ok();
    render_terminal_diagnostic(&profile, size, width)
}

fn render_terminal_diagnostic(
    profile: &TerminalProfile,
    size: Option<(u16, u16)>,
    width: usize,
) -> String {
    let width = width.min(u16::MAX as usize) as u16;
    if width == 0 {
        return String::new();
    }

    let environment = [
        profile.term().map(|value| format!("TERM={value}")),
        profile
            .term_program()
            .map(|value| format!("TERM_PROGRAM={value}")),
    ]
    .into_iter()
    .flatten()
    .collect::<Vec<_>>();
    let environment = if environment.is_empty() {
        "not reported".to_string()
    } else {
        environment.join(" · ")
    };
    let tty = format!(
        "stdin {} · stdout {} · stderr {}",
        yes_no(profile.stdin_is_terminal()),
        yes_no(profile.stdout_is_terminal()),
        yes_no(profile.stderr_is_terminal())
    );
    let size = size
        .map(|(columns, rows)| format!("{columns} × {rows} cells"))
        .unwrap_or_else(|| "unavailable".to_string());

    let mut panel = DetailPanel::new("Terminal diagnostics")
        .unlimited_rows()
        .label_width(16)
        .title_color(Color::Cyan)
        .pair("emulator", profile.family().to_string())
        .pair("multiplexer", profile.multiplexer().to_string())
        .pair("environment", environment)
        .pair("terminal I/O", tty)
        .pair("canvas", size)
        .pair("render mode", profile.display_mode().to_string())
        .pair("color", profile.color_level().to_string())
        .row(support_row("alternate screen", profile.alternate_screen()))
        .row(support_row("mouse capture", profile.mouse_capture()))
        .row(support_row("bracketed paste", profile.bracketed_paste()))
        .row(support_row("enhanced keys", profile.enhanced_keyboard()))
        .row(support_row("OSC 8 links", profile.hyperlinks()))
        .row(support_row("OSC 52 copy", profile.clipboard()));

    for warning in profile.warnings() {
        panel.add_row(DetailRow::muted(format!("⚠ {warning}")).color(Color::Yellow));
    }

    panel.view(width, usize::MAX)
}

fn support_row(label: &str, support: TerminalSupport) -> DetailRow {
    let (marker, color) = match support {
        TerminalSupport::Supported => ("✓", Color::Green),
        TerminalSupport::Unsupported => ("×", Color::Red),
        TerminalSupport::RequiresPassthrough => ("△", Color::Yellow),
        TerminalSupport::Unknown => ("?", Color::BrightBlack),
    };
    DetailRow::pair(label, format!("{marker} {support}")).color(color)
}

fn yes_no(value: bool) -> &'static str {
    if value {
        "yes"
    } else {
        "no"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::style::{strip_ansi, visible_len};

    #[test]
    fn diagnostic_is_bounded_and_names_every_negotiated_capability() {
        let width = 44;
        let rendered = current_terminal_diagnostic(width);
        let plain = strip_ansi(&rendered);

        assert!(plain.contains("Terminal diagnostics"), "{plain}");
        assert!(plain.contains("alternate screen"), "{plain}");
        assert!(plain.contains("mouse capture"), "{plain}");
        assert!(plain.contains("bracketed paste"), "{plain}");
        assert!(plain.contains("enhanced keys"), "{plain}");
        assert!(plain.contains("OSC 8 links"), "{plain}");
        assert!(plain.contains("OSC 52 copy"), "{plain}");
        assert!(
            rendered.lines().all(|line| visible_len(line) <= width),
            "{rendered:?}"
        );
    }

    #[test]
    fn narrow_diagnostic_stays_inside_the_terminal_canvas() {
        for width in [1_usize, 8, 20] {
            let rendered = current_terminal_diagnostic(width);
            assert!(
                rendered.lines().all(|line| visible_len(line) <= width),
                "width={width}: {rendered:?}"
            );
        }
    }
}
