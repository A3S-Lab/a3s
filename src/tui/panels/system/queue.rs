//! Pending-turn queue inspection and control.

use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel};
use a3s_tui::event::MouseEvent;

const QUEUE_MAX_VISIBLE_ROWS: usize = 12;

pub(crate) struct QueuePanel {
    selected_sequence: Option<u64>,
    selected_hint: usize,
    clear_confirmation: bool,
    confirm_selected: usize,
}

impl QueuePanel {
    fn new(sequences: &[u64]) -> Self {
        Self {
            selected_sequence: sequences.first().copied(),
            selected_hint: 0,
            clear_confirmation: false,
            confirm_selected: 0,
        }
    }

    fn selected_index(&self, sequences: &[u64]) -> usize {
        if sequences.is_empty() {
            return 0;
        }
        self.selected_sequence
            .and_then(|sequence| {
                sequences
                    .iter()
                    .position(|candidate| *candidate == sequence)
            })
            .unwrap_or_else(|| self.selected_hint.min(sequences.len() - 1))
    }

    fn select_index(&mut self, sequences: &[u64], index: usize) {
        if sequences.is_empty() {
            self.selected_sequence = None;
            self.selected_hint = 0;
            return;
        }
        let index = index.min(sequences.len() - 1);
        self.selected_sequence = Some(sequences[index]);
        self.selected_hint = index;
    }
}

/// Remove one exact pending item while returning every untouched item with its
/// original priority and FIFO sequence.
pub(crate) fn take_priority_item_by_sequence<T>(
    queue: &mut PriorityQueue<T>,
    sequence: u64,
) -> Option<PriorityItem<T>> {
    let mut untouched = Vec::new();
    let selected = loop {
        match queue.pop() {
            Some(item) if item.sequence() == sequence => break Some(item),
            Some(item) => untouched.push(item),
            None => break None,
        }
    };
    for item in untouched {
        queue.restore(item);
    }
    selected
}

/// Claim an explicit Send-now selection before normal priority/FIFO order.
/// A stale selection is cleared and falls back to the queue head.
pub(crate) fn take_next_priority_item<T>(
    queue: &mut PriorityQueue<T>,
    send_now_sequence: &mut Option<u64>,
) -> Option<PriorityItem<T>> {
    match *send_now_sequence {
        Some(sequence) => match take_priority_item_by_sequence(queue, sequence) {
            Some(item) => Some(item),
            None => {
                *send_now_sequence = None;
                queue.pop()
            }
        },
        None => queue.pop(),
    }
}

fn drain_pending_queue<T>(queue: &mut PriorityQueue<T>) {
    while queue.pop().is_some() {}
}

fn queue_sequences(queue: &PriorityQueue<Queued>) -> Vec<u64> {
    queue
        .ordered()
        .into_iter()
        .map(PriorityItem::sequence)
        .collect()
}

fn queue_max_rows(height: usize) -> usize {
    height.saturating_sub(9).clamp(3, QUEUE_MAX_VISIBLE_ROWS)
}

fn queue_panel_height(item_count: usize, max_items: usize) -> usize {
    let visible = item_count.max(1).min(max_items);
    // Title + subtitle + visible items + optional scroll status + footer.
    3 + visible + usize::from(item_count > visible)
}

fn queue_menu_panel(
    queue: &PriorityQueue<Queued>,
    modes: &HashMap<u64, Mode>,
    send_now_sequence: Option<u64>,
    selected: usize,
    max_items: usize,
) -> (MenuPanel, usize) {
    let ordered = queue.ordered();
    let count = ordered.len();
    let items = if ordered.is_empty() {
        vec![MenuItem::new("(no pending follow-ups)")
            .prefix("·")
            .color(TN_GRAY)
            .disabled(true)]
    } else {
        ordered
            .into_iter()
            .map(|item| {
                let sequence = item.sequence();
                let mode = modes.get(&sequence).copied().unwrap_or(Mode::Default);
                let image_count = item.value().images.len();
                let mut details = vec![mode.name().to_string()];
                if image_count > 0 {
                    details.push(format!(
                        "{image_count} {}",
                        if image_count == 1 { "image" } else { "images" }
                    ));
                }
                if send_now_sequence == Some(sequence) {
                    details.insert(0, "send now".to_string());
                }
                MenuItem::new(item.value().display.trim())
                    .prefix(mode.glyph())
                    .description(details.join(" · "))
                    .color(mode.color())
            })
            .collect()
    };
    let height = queue_panel_height(count, max_items);
    let panel = MenuPanel::new(format!("Queued follow-ups · {count}"))
        .subtitle("Each turn keeps its submission-time mode and attachments.")
        .items(items)
        .selected(selected)
        .max_items(max_items)
        .footer("↑/↓ select · Enter/S send now · D/Delete remove · C clear · Esc close")
        .indent(2)
        .title_color(ACCENT)
        .subtitle_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED)
        .disabled_color(TN_GRAY);
    (panel, height)
}

fn clear_confirmation_panel(count: usize, selected: usize) -> (MenuPanel, usize) {
    let panel = MenuPanel::new("Clear queued follow-ups?")
        .subtitle(format!(
            "This permanently removes {count} pending {}.",
            if count == 1 { "turn" } else { "turns" }
        ))
        .items(vec![
            MenuItem::new("Keep queued turns").prefix("←").color(TN_FG),
            MenuItem::new(format!("Clear {count} queued turns"))
                .prefix("×")
                .color(TN_RED),
        ])
        .selected(selected.min(1))
        .max_items(2)
        .footer("↑/↓ choose · Enter confirm · Y clear · N cancel · Esc close")
        .indent(2)
        .title_color(TN_RED)
        .subtitle_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED);
    (panel, 5)
}

fn queue_menu_lines(
    queue: &PriorityQueue<Queued>,
    modes: &HashMap<u64, Mode>,
    send_now_sequence: Option<u64>,
    panel: &QueuePanel,
    width: usize,
    max_items: usize,
) -> Vec<String> {
    let sequences = queue_sequences(queue);
    let (menu, height) = if panel.clear_confirmation && !sequences.is_empty() {
        clear_confirmation_panel(sequences.len(), panel.confirm_selected)
    } else {
        queue_menu_panel(
            queue,
            modes,
            send_now_sequence,
            panel.selected_index(&sequences),
            max_items,
        )
    };
    menu.view(width.min(u16::MAX as usize) as u16, height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn queue_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

impl App {
    pub(crate) fn open_queue_panel(&mut self) {
        let sequences = queue_sequences(&self.queue);
        if sequences.is_empty() {
            self.push_notice(NoticeKind::Info, "No follow-ups are queued");
            return;
        }
        self.queue_panel = Some(QueuePanel::new(&sequences));
    }

    pub(crate) fn handle_queue_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let sequences = queue_sequences(&self.queue);
        let panel = self.queue_panel.as_mut()?;

        if panel.clear_confirmation {
            match key.code {
                KeyCode::Up | KeyCode::Left => {
                    panel.confirm_selected = panel.confirm_selected.saturating_sub(1);
                }
                KeyCode::Down | KeyCode::Right => {
                    panel.confirm_selected = (panel.confirm_selected + 1).min(1);
                }
                KeyCode::Char('y' | 'Y') => return self.clear_pending_queue(),
                KeyCode::Char('n' | 'N') => {
                    panel.clear_confirmation = false;
                    panel.confirm_selected = 0;
                }
                KeyCode::Enter if panel.confirm_selected == 1 => {
                    return self.clear_pending_queue();
                }
                KeyCode::Enter => {
                    panel.clear_confirmation = false;
                    panel.confirm_selected = 0;
                }
                KeyCode::Esc => self.queue_panel = None,
                _ => {}
            }
            return None;
        }

        if sequences.is_empty() {
            if key.code == KeyCode::Esc {
                self.queue_panel = None;
            }
            return None;
        }

        let selected = panel.selected_index(&sequences);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                panel.select_index(&sequences, selected.saturating_sub(1));
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                panel.select_index(&sequences, (selected + 1).min(sequences.len() - 1));
                None
            }
            KeyCode::Home => {
                panel.select_index(&sequences, 0);
                None
            }
            KeyCode::End => {
                panel.select_index(&sequences, sequences.len() - 1);
                None
            }
            KeyCode::Enter | KeyCode::Char('s' | 'S') => {
                self.send_queued_turn_now(sequences[selected])
            }
            KeyCode::Delete | KeyCode::Backspace | KeyCode::Char('d' | 'D') => {
                self.remove_queued_turn(sequences[selected]);
                None
            }
            KeyCode::Char('c' | 'C') => {
                panel.clear_confirmation = true;
                panel.confirm_selected = 0;
                None
            }
            KeyCode::Esc => {
                self.queue_panel = None;
                None
            }
            _ => None,
        }
    }

    pub(crate) fn handle_queue_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let sequences = queue_sequences(&self.queue);
        let state = self.queue_panel.as_ref()?;
        let width = self.width as usize;
        if width == 0 {
            return None;
        }
        let max_items = queue_max_rows(self.height as usize);
        let (mut panel, height) = if state.clear_confirmation && !sequences.is_empty() {
            clear_confirmation_panel(sequences.len(), state.confirm_selected)
        } else {
            queue_menu_panel(
                &self.queue,
                &self.queued_turn_modes,
                self.send_now_queued_sequence,
                state.selected_index(&sequences),
                max_items,
            )
        };
        let row_count = panel
            .view(width.min(u16::MAX as usize) as u16, height)
            .lines()
            .count();
        let y_offset =
            queue_overlay_y_offset(self.height as usize, row_count, self.approval_rows_below());
        let row = mouse.row as usize;
        if row < y_offset as usize || row >= (y_offset as usize).saturating_add(row_count) {
            return None;
        }
        panel.set_y_offset(y_offset);
        let before = panel.selected_index();
        let _ = panel.handle_mouse(mouse);
        let after = panel.selected_index();
        if after == before {
            return None;
        }

        let state = self.queue_panel.as_mut()?;
        if state.clear_confirmation {
            state.confirm_selected = after.min(1);
        } else {
            state.select_index(&sequences, after);
        }
        None
    }

    pub(crate) fn overlay_queue_menu(&self, composed: String) -> String {
        let Some(panel) = self.queue_panel.as_ref() else {
            return composed;
        };
        let lines = queue_menu_lines(
            &self.queue,
            &self.queued_turn_modes,
            self.send_now_queued_sequence,
            panel,
            self.width as usize,
            queue_max_rows(self.height as usize),
        );
        self.overlay_list_with_rows_below(composed, &lines, self.approval_rows_below())
    }

    pub(crate) fn send_top_queued_turn_now(&mut self) -> Option<Cmd<Msg>> {
        let sequence = self.queue.ordered().first().map(|item| item.sequence())?;
        self.send_queued_turn_now(sequence)
    }

    fn remove_queued_turn(&mut self, sequence: u64) {
        let selected_index = queue_sequences(&self.queue)
            .iter()
            .position(|candidate| *candidate == sequence)
            .unwrap_or(0);
        let Some(item) = take_priority_item_by_sequence(&mut self.queue, sequence) else {
            return;
        };
        let label = truncate(item.value().display.trim(), 60);
        self.queued_turn_modes.remove(&sequence);
        self.queued_plan_drafts.remove(&sequence);
        if self.send_now_queued_sequence == Some(sequence) {
            self.send_now_queued_sequence = None;
        }

        let sequences = queue_sequences(&self.queue);
        if let Some(panel) = self.queue_panel.as_mut() {
            panel.select_index(
                &sequences,
                selected_index.min(sequences.len().saturating_sub(1)),
            );
        }
        self.push_notice(
            NoticeKind::Info,
            format!("Removed queued follow-up · {label}"),
        );
        self.relayout();
    }

    fn clear_pending_queue(&mut self) -> Option<Cmd<Msg>> {
        let sequences = queue_sequences(&self.queue);
        let count = sequences.len();
        if count == 0 {
            self.queue_panel = None;
            return None;
        }
        // Pop instead of `PriorityQueue::clear`: an admission claim can be
        // temporarily outside the heap, and resetting Lane's sequence counter
        // here could let a new pending turn reuse that live sequence.
        drain_pending_queue(&mut self.queue);
        for sequence in &sequences {
            self.queued_turn_modes.remove(sequence);
            self.queued_plan_drafts.remove(sequence);
        }
        if self
            .send_now_queued_sequence
            .is_some_and(|sequence| sequences.contains(&sequence))
        {
            self.send_now_queued_sequence = None;
        }
        self.queue_panel = None;
        self.push_notice(
            NoticeKind::Info,
            format!(
                "Cleared {count} queued {}",
                if count == 1 {
                    "follow-up"
                } else {
                    "follow-ups"
                }
            ),
        );
        self.relayout();
        None
    }

    fn send_queued_turn_now(&mut self, sequence: u64) -> Option<Cmd<Msg>> {
        if !self
            .queue
            .ordered()
            .into_iter()
            .any(|item| item.sequence() == sequence)
        {
            return None;
        }
        self.send_now_queued_sequence = Some(sequence);
        self.queue_panel = None;
        self.relayout();

        match self.state {
            State::Idle => self.drain_queue(),
            State::Streaming if self.stream_join_settling => {
                if let Some(abort) = self.stream_settle_abort.take() {
                    abort.abort();
                }
                None
            }
            State::Streaming if self.deep_research_subagent_settlement_inflight => {
                self.deep_research_subagent_settlement_inflight = false;
                self.invalidate_subagent_snapshots();
                self.state = State::Idle;
                self.running_task = None;
                self.spinner.stop();
                self.restore_autonomy();
                self.relayout();
                self.rebuild_viewport();
                self.drain_queue()
            }
            State::Streaming => self.begin_send_now_interrupt(),
            State::Awaiting | State::Rebuilding => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::style::strip_ansi;

    fn queued(display: &str) -> Queued {
        Queued {
            text: display.to_string(),
            display: display.to_string(),
            images: Vec::new(),
            runtime_expectation: None,
            deep_research: None,
        }
    }

    #[test]
    fn exact_removal_preserves_untouched_priority_and_fifo_order() {
        let mut queue = PriorityQueue::new();
        let first = queue.push(1, "first");
        let selected = queue.push(1, "selected");
        let third = queue.push(1, "third");
        queue.push(2, "background");

        let removed = take_priority_item_by_sequence(&mut queue, selected).unwrap();

        assert_eq!(removed.sequence(), selected);
        assert_eq!(removed.priority(), 1);
        assert_eq!(removed.into_value(), "selected");
        let remaining = queue
            .ordered()
            .into_iter()
            .map(|item| (item.sequence(), *item.value()))
            .collect::<Vec<_>>();
        assert_eq!(
            remaining,
            [(first, "first"), (third, "third"), (4, "background")]
        );
    }

    #[test]
    fn send_now_claims_the_exact_selected_row_ahead_of_normal_order() {
        let mut queue = PriorityQueue::new();
        queue.push(1, "first");
        let selected = queue.push(2, "selected background");
        queue.push(1, "second");
        let mut send_now = Some(selected);

        let claimed = take_next_priority_item(&mut queue, &mut send_now).unwrap();

        assert_eq!(claimed.sequence(), selected);
        assert_eq!(claimed.into_value(), "selected background");
        assert_eq!(send_now, Some(selected));
        assert_eq!(
            queue
                .ordered()
                .into_iter()
                .map(|item| *item.value())
                .collect::<Vec<_>>(),
            ["first", "second"]
        );
    }

    #[test]
    fn clearing_pending_rows_does_not_reuse_a_live_lane_sequence() {
        let mut queue = PriorityQueue::new();
        assert_eq!(queue.push(1, "claimed elsewhere"), 1);
        let _claimed = queue.pop().unwrap();
        assert_eq!(queue.push(1, "pending"), 2);

        drain_pending_queue(&mut queue);

        assert_eq!(queue.push(1, "new pending"), 3);
    }

    #[test]
    fn queue_panel_shows_submission_mode_and_compact_prompt() {
        let mut queue = PriorityQueue::new();
        let sequence = queue.push(1, queued("run the focused tests"));
        let modes = HashMap::from([(sequence, Mode::Auto)]);
        let state = QueuePanel::new(&[sequence]);

        let plain = queue_menu_lines(&queue, &modes, None, &state, 80, 12)
            .into_iter()
            .map(|line| strip_ansi(&line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("Queued follow-ups · 1"), "{plain}");
        assert!(plain.contains("run the focused tests"), "{plain}");
        assert!(plain.contains("auto"), "{plain}");
        assert!(plain.contains("send now"), "{plain}");
    }

    #[test]
    fn queue_panel_mouse_wheel_moves_selection_at_overlay_offset() {
        use a3s_tui::event::MouseEventKind;

        let mut queue = PriorityQueue::new();
        queue.push(1, queued("first"));
        queue.push(1, queued("second"));
        let (mut panel, _) = queue_menu_panel(&queue, &HashMap::new(), None, 0, 12);
        panel.set_y_offset(7);

        assert!(panel
            .handle_mouse(&MouseEvent {
                kind: MouseEventKind::ScrollDown,
                column: 2,
                row: 9,
                modifiers: KeyModifiers::NONE,
            })
            .is_none());
        assert_eq!(panel.selected_index(), 1);
    }

    #[test]
    fn queue_panel_lines_remain_bounded_at_narrow_width() {
        let mut queue = PriorityQueue::new();
        let sequence = queue.push(
            1,
            queued("an intentionally long follow-up that must never overflow the terminal"),
        );
        let state = QueuePanel::new(&[sequence]);
        for line in queue_menu_lines(&queue, &HashMap::new(), None, &state, 36, 12) {
            assert!(
                a3s_tui::style::visible_len(&line) <= 36,
                "{:?}",
                strip_ansi(&line)
            );
        }
    }

    #[test]
    fn queue_selection_keeps_the_same_sequence_when_rows_change() {
        let sequences = [10, 20, 30];
        let mut panel = QueuePanel::new(&sequences);
        panel.select_index(&sequences, 1);

        assert_eq!(panel.selected_index(&[5, 10, 20, 30]), 2);
        assert_eq!(panel.selected_sequence, Some(20));
    }
}
