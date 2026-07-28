//! Composer image attachments and their Codex-style interactive chips.

use std::io;
use std::ops::Range;

use super::*;

/// One image owned by the composer, a queued turn, or a stream-start command.
///
/// The temporary path is deliberately kept with the attachment so RemoteUI can
/// preview it before submission. It is deleted automatically after successful
/// stream admission, explicit removal, or application shutdown.
#[derive(Clone)]
pub(super) struct PendingImage {
    path: Arc<tempfile::TempPath>,
    width: u32,
    height: u32,
}

impl PendingImage {
    pub(super) fn from_clipboard() -> io::Result<Self> {
        let captured = capture_clipboard_image()?;
        Ok(Self {
            path: Arc::new(captured.path),
            width: captured.width,
            height: captured.height,
        })
    }

    pub(super) fn attachment(&self) -> io::Result<a3s_code_core::llm::Attachment> {
        Ok(a3s_code_core::llm::Attachment::png(std::fs::read(
            self.path.as_ref(),
        )?))
    }

    pub(super) fn preview(&self) -> io::Result<remote_ui::ViewSpec> {
        remote_ui::local_image_view(self.path.as_ref(), self.width, self.height)
    }

    fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AttachmentAction {
    Preview(usize),
    Remove(usize),
}

#[derive(Debug)]
struct AttachmentHitRegion {
    row: usize,
    preview: Range<usize>,
    remove: Range<usize>,
    index: usize,
}

/// Rendered attachment rows plus the exact hit regions used by mouse input.
#[derive(Debug, Default)]
pub(super) struct AttachmentStrip {
    pub(super) rows: Vec<String>,
    hits: Vec<AttachmentHitRegion>,
}

impl AttachmentStrip {
    pub(super) fn hit_test(&self, row: usize, column: usize) -> Option<AttachmentAction> {
        self.hits.iter().find_map(|hit| {
            if hit.row != row {
                return None;
            }
            if hit.remove.contains(&column) {
                Some(AttachmentAction::Remove(hit.index))
            } else if hit.preview.contains(&column) {
                Some(AttachmentAction::Preview(hit.index))
            } else {
                None
            }
        })
    }
}

struct RenderedChip {
    view: String,
    width: usize,
    remove_start: usize,
}

/// Lay out image chips left-to-right and wrap them inside the composer.
pub(super) fn attachment_strip(images: &[PendingImage], width: usize) -> AttachmentStrip {
    if images.is_empty() || width == 0 {
        return AttachmentStrip::default();
    }

    let mut strip = AttachmentStrip::default();
    let mut row = String::new();
    let mut row_width = 0usize;
    let mut row_index = 0usize;

    for (index, image) in images.iter().enumerate() {
        let chip = render_attachment_chip(index, image.dimensions(), width);
        let gap = usize::from(row_width > 0);
        if row_width > 0 && row_width.saturating_add(gap).saturating_add(chip.width) > width {
            strip.rows.push(row);
            row = String::new();
            row_width = 0;
            row_index += 1;
        }

        if row_width > 0 {
            row.push(' ');
            row_width += 1;
        }
        let start = row_width;
        row.push_str(&chip.view);
        row_width = row_width.saturating_add(chip.width);
        strip.hits.push(AttachmentHitRegion {
            row: row_index,
            preview: start..start.saturating_add(chip.remove_start),
            remove: start.saturating_add(chip.remove_start)..start.saturating_add(chip.width),
            index,
        });
    }

    if !row.is_empty() {
        strip.rows.push(row);
    }
    strip
}

impl App {
    pub(crate) fn composer_attachment_rows(&self) -> usize {
        attachment_strip(&self.pending_images, self.viewport_content_width())
            .rows
            .len()
    }

    pub(super) fn attachment_action_at(
        &self,
        terminal_row: u16,
        terminal_column: u16,
    ) -> Option<AttachmentAction> {
        let strip = attachment_strip(&self.pending_images, self.viewport_content_width());
        let row_count = strip.rows.len();
        if row_count == 0 {
            return None;
        }
        let input_start =
            self.bottom_pane_projection()
                .input_cursor_row(self.height, self.input_height(), 0) as usize;
        let strip_start = input_start.saturating_sub(row_count);
        let row = terminal_row as usize;
        if row < strip_start || row >= input_start {
            return None;
        }
        strip.hit_test(row - strip_start, terminal_column as usize)
    }
}

fn render_attachment_chip(index: usize, dimensions: (u32, u32), width: usize) -> RenderedChip {
    let label = format!(" [Image #{}] ", index + 1);
    let dimensions = format!("{}×{} ", dimensions.0, dimensions.1);
    let close = "× ";
    let close_width = a3s_tui::style::visible_len(close).min(width);
    let content_budget = width.saturating_sub(close_width);
    let full_content = format!("{label}{dimensions}");
    let (label, dimensions) = if a3s_tui::style::visible_len(&full_content) <= content_budget {
        (label, dimensions)
    } else {
        (
            a3s_tui::style::fit_visible(&label, content_budget),
            String::new(),
        )
    };
    let content_width = a3s_tui::style::visible_len(&label)
        .saturating_add(a3s_tui::style::visible_len(&dimensions));
    let close = a3s_tui::style::fit_visible(close, close_width);
    let close_width = a3s_tui::style::visible_len(&close);
    let label_view = Style::new()
        .fg(TN_FG)
        .bg(SURFACE_SOFT)
        .bold()
        .render(&label);
    let dimensions_view = Style::new()
        .fg(TN_GRAY)
        .bg(SURFACE_SOFT)
        .render(&dimensions);
    let close_view = Style::new()
        .fg(TN_GRAY)
        .bg(SURFACE_SOFT)
        .bold()
        .render(&close);

    RenderedChip {
        view: format!("{label_view}{dimensions_view}{close_view}"),
        width: content_width.saturating_add(close_width),
        remove_start: content_width,
    }
}

/// Plain references retained in the submitted user bubble after composer chips
/// disappear. The binary image data itself remains out of the transcript.
pub(super) fn attachment_reference_line(images: &[PendingImage]) -> String {
    (0..images.len())
        .map(|index| format!("[Image #{}]", index + 1))
        .collect::<Vec<_>>()
        .join(" ")
}

/// Put a failed stream-admission batch back before images captured while that
/// request was starting. This preserves the user's original attachment order.
pub(super) fn restore_submitted_images(
    pending: &mut Vec<PendingImage>,
    mut submitted: Vec<PendingImage>,
) {
    submitted.append(pending);
    *pending = submitted;
}

#[cfg(test)]
mod tests {
    use super::*;

    fn image(width: u32, height: u32) -> PendingImage {
        let file = tempfile::Builder::new().suffix(".png").tempfile().unwrap();
        PendingImage {
            path: Arc::new(file.into_temp_path()),
            width,
            height,
        }
    }

    #[test]
    fn strip_uses_codex_image_labels_and_wraps_at_terminal_width() {
        let images = [image(1920, 1080), image(800, 600)];
        let wide = attachment_strip(&images, 80);
        let narrow = attachment_strip(&images, 20);

        assert_eq!(wide.rows.len(), 1);
        assert_eq!(narrow.rows.len(), 2);
        assert!(a3s_tui::style::strip_ansi(&wide.rows[0]).contains("[Image #1]"));
        assert!(a3s_tui::style::strip_ansi(&wide.rows[0]).contains("1920×1080"));
        assert!(narrow
            .rows
            .iter()
            .all(|row| a3s_tui::style::visible_len(row) <= 20));
    }

    #[test]
    fn strip_distinguishes_preview_from_remove_hit_targets() {
        let images = [image(100, 100)];
        let strip = attachment_strip(&images, 40);
        let hit = &strip.hits[0];

        assert_eq!(
            strip.hit_test(hit.row, hit.preview.start),
            Some(AttachmentAction::Preview(0))
        );
        assert_eq!(
            strip.hit_test(hit.row, hit.remove.start),
            Some(AttachmentAction::Remove(0))
        );
    }

    #[test]
    fn transcript_reference_line_keeps_image_order() {
        let images = [image(1, 1), image(2, 2)];
        assert_eq!(attachment_reference_line(&images), "[Image #1] [Image #2]");
    }

    #[test]
    fn failed_submission_is_restored_before_new_composer_images() {
        let mut pending = vec![image(3, 3)];
        restore_submitted_images(&mut pending, vec![image(1, 1), image(2, 2)]);

        assert_eq!(
            pending
                .iter()
                .map(PendingImage::dimensions)
                .collect::<Vec<_>>(),
            vec![(1, 1), (2, 2), (3, 3)]
        );
    }

    #[test]
    fn dropping_pending_image_removes_its_preview_file() {
        let image = image(10, 10);
        let path = image.path.to_path_buf();
        assert!(path.exists());
        drop(image);
        assert!(!path.exists());
    }

    #[test]
    fn admission_clone_keeps_temporary_image_alive_for_queue_restore() {
        let image = image(10, 10);
        std::fs::write(image.path.as_ref(), b"queued-image").unwrap();
        let path = image.path.to_path_buf();
        let retained_queue_image = image.clone();

        drop(image);
        assert!(path.exists());
        assert_eq!(
            std::fs::read(retained_queue_image.path.as_ref()).unwrap(),
            b"queued-image"
        );

        drop(retained_queue_image);
        assert!(!path.exists());
    }
}
