//! One immutable snapshot of the dynamic bottom-pane rows for a render pass.
//!
//! Plan, subagent, and queue rows used to be recomputed independently by the
//! viewport, cursor, and renderer. Keeping them in one projection makes row
//! ownership explicit and prevents a terminal event from leaving a one-frame
//! gap or stale footer row.

use super::super::*;

/// Spacer, transient activity, composer top rule, footer separator, and the
/// single Codex-style footer. The composer itself is accounted separately.
pub(crate) const FIXED_ROWS_EXCLUDING_INPUT: u16 = 5;
/// Footer separator plus the single footer row below the composer.
pub(crate) const FIXED_ROWS_BELOW_INPUT: u16 = 2;

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct BottomPaneProjection {
    pub(crate) plan: Vec<String>,
    pub(crate) subagents: Vec<String>,
    pub(crate) tasks: Vec<String>,
}

impl BottomPaneProjection {
    pub(crate) fn dynamic_rows(&self) -> usize {
        self.plan
            .len()
            .saturating_add(self.subagents.len())
            .saturating_add(self.tasks.len())
    }

    pub(crate) fn rows_below_input(&self) -> usize {
        self.subagents.len().saturating_add(self.tasks.len())
    }

    pub(crate) fn input_cursor_row(
        &self,
        terminal_height: u16,
        input_height: u16,
        input_cursor_row: u16,
    ) -> u16 {
        let dynamic_below = self.rows_below_input().min(u16::MAX as usize) as u16;
        let below = FIXED_ROWS_BELOW_INPUT.saturating_add(dynamic_below);
        terminal_height
            .saturating_sub(below.saturating_add(input_height))
            .saturating_add(input_cursor_row)
    }
}

impl App {
    pub(crate) fn bottom_pane_projection(&self) -> BottomPaneProjection {
        BottomPaneProjection {
            plan: self.plan_lines(),
            subagents: self.subagent_lines(),
            tasks: self.task_lines(),
        }
    }

    /// Rows between an overlay's bottom edge and the terminal bottom.
    ///
    /// The overlay replaces the transcript-to-activity spacer, so the activity
    /// row, other fixed chrome, auto-growing composer, and every dynamic
    /// bottom-pane row remain below it.
    pub(crate) fn overlay_rows_below(&self) -> usize {
        overlay_rows_below_for(
            self.input_height(),
            self.composer_attachment_rows(),
            self.bottom_pane_projection().dynamic_rows(),
        )
    }
}

fn overlay_rows_below_for(input_height: u16, attachment_rows: usize, dynamic_rows: usize) -> usize {
    usize::from(FIXED_ROWS_EXCLUDING_INPUT.saturating_sub(1))
        .saturating_add(usize::from(input_height))
        .saturating_add(attachment_rows)
        .saturating_add(dynamic_rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn projection_counts_each_owned_surface_once() {
        let projection = BottomPaneProjection {
            plan: vec!["plan".into(), "plan 2".into()],
            subagents: vec!["agents".into()],
            tasks: vec!["queue".into(), "queue 2".into()],
        };

        assert_eq!(projection.dynamic_rows(), 5);
        assert_eq!(projection.rows_below_input(), 3);
    }

    #[test]
    fn terminal_projection_has_no_phantom_dynamic_rows() {
        let projection = BottomPaneProjection::default();

        assert_eq!(projection.dynamic_rows(), 0);
        assert_eq!(projection.rows_below_input(), 0);
    }

    #[test]
    fn terminal_transition_restores_the_baseline_cursor_row() {
        let baseline = BottomPaneProjection::default();
        let active = BottomPaneProjection {
            subagents: vec!["agent 1".into(), "agent 2".into()],
            tasks: vec!["queued".into()],
            ..BottomPaneProjection::default()
        };

        assert_eq!(baseline.input_cursor_row(30, 1, 0), 27);
        assert_eq!(active.input_cursor_row(30, 1, 0), 24);
        assert_eq!(
            BottomPaneProjection::default().input_cursor_row(30, 1, 0),
            27
        );
    }

    #[test]
    fn overlay_rows_preserve_the_default_composer_position() {
        assert_eq!(overlay_rows_below_for(1, 0, 0), 5);
    }

    #[test]
    fn overlay_rows_follow_multiline_input_and_dynamic_bottom_surfaces() {
        assert_eq!(overlay_rows_below_for(3, 0, 0), 7);
        assert_eq!(overlay_rows_below_for(3, 0, 4), 11);
        assert_eq!(overlay_rows_below_for(3, 2, 4), 13);
    }
}
