//! First-run welcome banner: animated A3S mascot, wordmark, and session details.

use super::super::*;
use a3s_tui::components::WelcomeBanner;

impl App {
    /// First-run welcome: animated product identity, version, model, and tips.
    pub(crate) fn banner(&self) -> String {
        let model = self.model.as_deref().unwrap_or("no model configured");
        let skills = if self.skill_count > 0 {
            format!("  ·  {} skills", self.skill_count)
        } else {
            String::new()
        };
        let os = match &self.os_session {
            Some(s) => format!("  ·  OS: {}", s.display_label()),
            None => String::new(),
        };
        banner_view(
            self.anim,
            model,
            &skills,
            &os,
            &self.cwd,
            self.update_available.as_deref(),
            self.viewport_content_width().min(u16::MAX as usize) as u16,
        )
    }
}

fn banner_view(
    anim: u8,
    model: &str,
    skills: &str,
    os: &str,
    cwd: &str,
    update_available: Option<&str>,
    width: u16,
) -> String {
    let metadata = format!(
        "a3s-code v{}  ·  {model}{skills}{os}  ·  {cwd}",
        env!("CARGO_PKG_VERSION")
    );
    let mut banner = WelcomeBanner::new()
        .mascot_lines(banner_mascot(anim))
        .art_lines(banner_wordmark())
        .art_offset(1)
        .margin(PAD)
        .gap(2)
        .mascot_color(banner_mascot_color(anim))
        .art_color(banner_wordmark_color(anim))
        .metadata_color(TN_GRAY)
        .tip_color(TN_GRAY)
        .notice_color(ACCENT)
        .metadata(metadata)
        .tip("Type a message · / for commands · Shift+Tab cycles mode · Ctrl+C twice to exit");
    if let Some(v) = update_available {
        banner = banner.notice(format!(
            "⬆ a3s {v} is available (you have {}) — type /update to upgrade",
            env!("CARGO_PKG_VERSION")
        ));
    }

    format!("\n{}\n", banner.view(width, usize::MAX))
}

fn banner_mascot(anim: u8) -> Vec<String> {
    // Preserve the original Song-dynasty guard silhouette. Animation is
    // deliberately sparse: a blink, sword glint, shield glint, and slow weight
    // shift make it feel alive without creating terminal flicker.
    let phase = anim % 24;
    let eyes = if phase == 18 { "- -" } else { "o o" };
    let sword_glint = if phase == 5 { "*" } else { "+" };
    let shield = if phase == 11 { "|✦|" } else { "|#|" };
    let feet = if (phase / 4).is_multiple_of(2) {
        r"/   \"
    } else {
        r"\   /"
    };

    vec![
        r"     .-^-.      ".to_string(),
        r"    /_____\     ".to_string(),
        format!("    ( {eyes} )     "),
        r"  |  /|_|\  _   ".to_string(),
        format!(" -{sword_glint}- |   | {shield}  "),
        r"  |  |___| \#/  ".to_string(),
        format!("     {feet}      "),
    ]
}

fn banner_wordmark() -> Vec<&'static str> {
    vec![
        r" █████╗ ██████╗ ███████╗     ██████╗ ██████╗ ██████╗ ███████╗",
        r"██╔══██╗╚════██╗██╔════╝    ██╔════╝██╔═══██╗██╔══██╗██╔════╝",
        r"███████║ █████╔╝███████╗    ██║     ██║   ██║██║  ██║█████╗",
        r"██╔══██║ ╚═══██╗╚════██║    ██║     ██║   ██║██║  ██║██╔══╝",
        r"██║  ██║██████╔╝███████║    ╚██████╗╚██████╔╝██████╔╝███████╗",
        r"╚═╝  ╚═╝╚═════╝ ╚══════╝     ╚═════╝ ╚═════╝ ╚═════╝ ╚══════╝",
    ]
}

fn banner_mascot_color(anim: u8) -> Color {
    if anim % 24 == 5 || anim % 24 == 11 {
        BRAND_GRADIENT[3]
    } else {
        TN_GRAY
    }
}

fn banner_wordmark_color(anim: u8) -> Color {
    if anim % 24 == 11 {
        BRAND_GRADIENT[1]
    } else {
        ACCENT
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn welcome_banner_uses_shared_component_and_fits_width() {
        let terminal_width = 52;
        let viewport_width = terminal_width;
        let rendered = banner_view(
            3,
            "gpt-5",
            "  ·  4 skills",
            "  ·  OS: dev@example",
            "/Users/roylin/code/a3s",
            Some("0.9.0"),
            viewport_width,
        );
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("a3s-code v"), "{plain}");
        assert!(plain.contains(".-^-.") || plain.contains("████"), "{plain}");
        assert!(plain.contains("Type a message"), "{plain}");
        assert!(plain.contains("0.9.0"), "{plain}");
        assert!(rendered.contains("\x1b["), "banner should carry styling");
        for line in rendered.lines().filter(|line| !line.is_empty()) {
            assert!(
                a3s_tui::style::visible_len(line) <= viewport_width as usize,
                "{:?}",
                a3s_tui::style::strip_ansi(line)
            );
        }
    }

    #[test]
    fn welcome_identity_keeps_mascot_wordmark_and_animation() {
        let first = banner_view(0, "gpt-5", "", "", "/workspace", None, 120);
        let later = banner_view(5, "gpt-5", "", "", "/workspace", None, 120);
        let plain = a3s_tui::style::strip_ansi(&first);

        assert!(plain.contains(".-^-."), "{plain}");
        assert!(plain.contains("████"), "{plain}");
        assert_eq!(first.lines().count(), later.lines().count());
        assert_ne!(first, later);
    }

    #[test]
    fn welcome_banner_does_not_wrap_inside_scrollbar_viewport() {
        let terminal_width = 52;
        let viewport_width = terminal_width;
        let rendered = banner_view(
            3,
            "gpt-5",
            "  ·  4 skills",
            "  ·  OS: dev@example",
            "/Users/roylin/code/a3s",
            Some("0.9.0"),
            viewport_width,
        );
        let expected_rows = rendered.split('\n').count();
        let mut viewport = a3s_tui::components::Viewport::new(viewport_width, 80);

        viewport.set_content(&rendered);

        assert_eq!(viewport.total_lines(), expected_rows);
    }
}
