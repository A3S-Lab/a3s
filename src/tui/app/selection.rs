//! Stable transcript-selection projection between semantic entries and screen cells.

use super::*;

impl App {
    pub(super) fn begin_transcript_selection(&mut self, cell: (u16, u16)) {
        let point = self.transcript_point_for_viewport_cell(cell);
        self.selection = Some(Selection::start(cell, point));
    }

    pub(super) fn update_transcript_selection(&mut self, cell: (u16, u16)) {
        let point = self.transcript_point_for_viewport_cell(cell);
        if let Some(selection) = self.selection.as_mut() {
            selection.set_head(cell, point);
        }
    }

    /// Reproject a committed-history selection after reflow or viewport motion.
    ///
    /// A screen-only selection points at mutable banner/streaming content and
    /// cannot be restored safely, so it is discarded on content refresh.
    pub(super) fn refresh_transcript_selection_projection(&mut self) {
        let Some(semantic) = self.selection.and_then(|selection| selection.semantic()) else {
            self.selection = None;
            return;
        };
        let Some(anchor) = self.viewport_cell_for_transcript_point(semantic.anchor()) else {
            self.selection = None;
            return;
        };
        let Some(head) = self.viewport_cell_for_transcript_point(semantic.head()) else {
            self.selection = None;
            return;
        };
        if let Some(selection) = self.selection.as_mut() {
            selection.project(anchor, head);
        }
    }

    pub(super) fn selected_transcript_text(&self, selection: Selection, view: &str) -> String {
        if let Some(text) = selection
            .semantic()
            .and_then(|semantic| self.messages.selected_text(semantic))
            .filter(|text| !text.is_empty())
        {
            return text;
        }
        let (r1, c1, r2, c2) = selection.ordered();
        selection_to_text(view, r1, c1, r2, c2)
    }

    pub(super) fn transcript_point_for_viewport_cell(
        &self,
        (row, col): (u16, u16),
    ) -> Option<TranscriptPoint> {
        let absolute_row = self.viewport.scroll_offset().saturating_add(row as usize);
        // The main viewport owns one blank row above the transcript.
        let transcript_row = absolute_row.checked_sub(1)?;
        self.messages.point_for_cell(transcript_row, col as usize)
    }

    fn viewport_cell_for_transcript_point(&self, point: TranscriptPoint) -> Option<(u16, u16)> {
        let viewport_rows = self.viewport_rows();
        if viewport_rows == 0 {
            return None;
        }
        let (transcript_row, col) = self.messages.cell_for_point(point)?;
        // Account for the main viewport's leading blank row, then clamp an
        // off-screen endpoint to the nearest edge while retaining its semantic
        // identity for future projections.
        let absolute_row = transcript_row.saturating_add(1);
        let offset = self.viewport.scroll_offset();
        let visible_row = absolute_row
            .saturating_sub(offset)
            .min(viewport_rows.saturating_sub(1));
        let visible_row = if absolute_row < offset {
            0
        } else {
            visible_row
        };
        let max_col = (self.width as usize).saturating_sub(2);
        Some((
            visible_row.min(u16::MAX as usize) as u16,
            col.min(max_col).min(u16::MAX as usize) as u16,
        ))
    }
}
