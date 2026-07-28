//! `/tasks` and Ctrl+B: inspect and control delegated subagent work.

use super::super::*;
use a3s_code_core::{SubagentStatus, SubagentTaskSnapshot};
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::MouseEventKind;

const TASK_PANEL_MAX_VISIBLE_ROWS: usize = 12;
const TASK_PANEL_MAX_RECENT: usize = 64;
const TASK_PANEL_REFRESH_INTERVAL: Duration = Duration::from_secs(1);

pub(crate) struct TaskPanel {
    generation: u64,
    request_id: u64,
    session_id: String,
    tasks: Vec<SubagentTaskSnapshot>,
    selected_task_id: Option<String>,
    query: String,
    searching: bool,
    preview: bool,
    loading: bool,
    error: Option<String>,
    cancel_armed: Option<String>,
    cancel_inflight: Option<String>,
}

impl TaskPanel {
    fn loading(generation: u64, session_id: String) -> Self {
        Self {
            generation,
            request_id: 0,
            session_id,
            tasks: Vec::new(),
            selected_task_id: None,
            query: String::new(),
            searching: false,
            preview: false,
            loading: true,
            error: None,
            cancel_armed: None,
            cancel_inflight: None,
        }
    }

    fn visible_indices(&self) -> Vec<usize> {
        self.tasks
            .iter()
            .enumerate()
            .filter_map(|(index, task)| task_matches_query(task, &self.query).then_some(index))
            .collect()
    }

    fn selected_index(&self) -> usize {
        let indices = self.visible_indices();
        self.selected_task_id
            .as_deref()
            .and_then(|selected| {
                indices.iter().position(|index| {
                    self.tasks
                        .get(*index)
                        .is_some_and(|task| task.task_id == selected)
                })
            })
            .unwrap_or(0)
            .min(indices.len().saturating_sub(1))
    }

    fn selected_task(&self) -> Option<&SubagentTaskSnapshot> {
        let indices = self.visible_indices();
        indices
            .get(self.selected_index())
            .and_then(|index| self.tasks.get(*index))
    }

    fn remember_visible_index(&mut self, visible_index: usize) {
        let task_id = self
            .visible_indices()
            .get(visible_index)
            .and_then(|index| self.tasks.get(*index))
            .map(|task| task.task_id.clone());
        if task_id.is_some() {
            self.selected_task_id = task_id;
        }
    }

    fn move_selection(&mut self, amount: isize) {
        let indices = self.visible_indices();
        if indices.is_empty() {
            return;
        }
        let current = self.selected_index();
        let next = if amount.is_negative() {
            current.saturating_sub(amount.unsigned_abs())
        } else {
            current
                .saturating_add(amount as usize)
                .min(indices.len().saturating_sub(1))
        };
        self.remember_visible_index(next);
        self.cancel_armed = None;
    }

    fn move_selection_to(&mut self, index: usize) {
        let last = self.visible_indices().len().saturating_sub(1);
        self.remember_visible_index(index.min(last));
        self.cancel_armed = None;
    }

    fn select_visible_index(&mut self, index: usize) -> bool {
        let before = self.selected_task_id.clone();
        self.remember_visible_index(index);
        self.cancel_armed = None;
        before != self.selected_task_id
    }

    fn reconcile_selection(&mut self) {
        let selected_still_exists = self
            .selected_task_id
            .as_deref()
            .is_some_and(|selected| self.tasks.iter().any(|task| task.task_id == selected));
        if !selected_still_exists {
            self.selected_task_id = self
                .visible_indices()
                .first()
                .and_then(|index| self.tasks.get(*index))
                .or_else(|| self.tasks.first())
                .map(|task| task.task_id.clone());
        }
        let armed_is_running = self.cancel_armed.as_deref().is_some_and(|armed| {
            self.tasks
                .iter()
                .any(|task| task.task_id == armed && task_is_running(task))
        });
        if !armed_is_running {
            self.cancel_armed = None;
        }
    }

    fn apply_data(
        &mut self,
        session_id: &str,
        generation: u64,
        request_id: u64,
        tasks: Vec<SubagentTaskSnapshot>,
    ) -> bool {
        if self.session_id != session_id
            || self.generation != generation
            || self.request_id != request_id
        {
            return false;
        }
        self.tasks = finalize_task_snapshots(tasks);
        self.loading = false;
        self.reconcile_selection();
        true
    }

    fn accepts(&self, session_id: &str, generation: u64) -> bool {
        self.session_id == session_id && self.generation == generation
    }

    fn open_selected(&mut self) -> TaskPanelAction {
        self.selected_task()
            .cloned()
            .map(Box::new)
            .map_or(TaskPanelAction::None, TaskPanelAction::Open)
    }

    fn arm_or_cancel_selected(&mut self) -> TaskPanelAction {
        if self.cancel_inflight.is_some() {
            return TaskPanelAction::None;
        }
        let Some(task) = self.selected_task() else {
            return TaskPanelAction::None;
        };
        if !task_is_running(task) {
            self.error = Some("Only a running task can be cancelled.".to_string());
            self.cancel_armed = None;
            return TaskPanelAction::None;
        }
        let task_id = task.task_id.clone();
        self.error = None;
        if self.cancel_armed.as_deref() == Some(task_id.as_str()) {
            self.cancel_armed = None;
            self.cancel_inflight = Some(task_id.clone());
            TaskPanelAction::Cancel(task_id)
        } else {
            self.cancel_armed = Some(task_id);
            TaskPanelAction::None
        }
    }

    fn handle_search_key(&mut self, key: &KeyEvent) -> TaskPanelAction {
        match key.code {
            KeyCode::Esc => self.searching = false,
            KeyCode::Enter => return self.open_selected(),
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-(TASK_PANEL_MAX_VISIBLE_ROWS as isize)),
            KeyCode::PageDown => self.move_selection(TASK_PANEL_MAX_VISIBLE_ROWS as isize),
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Backspace => {
                self.query.pop();
                self.cancel_armed = None;
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.clear();
                self.cancel_armed = None;
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.query.push(character);
                self.cancel_armed = None;
            }
            _ => {}
        }
        TaskPanelAction::None
    }

    fn handle_key(&mut self, key: &KeyEvent) -> TaskPanelAction {
        if is_task_panel_key(key) {
            return TaskPanelAction::Close;
        }
        if self.searching {
            return self.handle_search_key(key);
        }
        if key.code == KeyCode::Esc && self.cancel_armed.take().is_some() {
            return TaskPanelAction::None;
        }
        match key.code {
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down | KeyCode::Tab => self.move_selection(1),
            KeyCode::BackTab => self.move_selection(-1),
            KeyCode::PageUp => self.move_selection(-(TASK_PANEL_MAX_VISIBLE_ROWS as isize)),
            KeyCode::PageDown => self.move_selection(TASK_PANEL_MAX_VISIBLE_ROWS as isize),
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('c' | 'C') if !self.query.is_empty() => {
                self.query.clear();
                self.cancel_armed = None;
            }
            KeyCode::Char(' ') => {
                self.preview = !self.preview;
                self.cancel_armed = None;
            }
            KeyCode::Char('r' | 'R') => {
                self.cancel_armed = None;
                return TaskPanelAction::Refresh;
            }
            KeyCode::Char('x' | 'X') | KeyCode::Delete => {
                return self.arm_or_cancel_selected();
            }
            KeyCode::Enter => {
                self.cancel_armed = None;
                return self.open_selected();
            }
            KeyCode::Esc => return TaskPanelAction::Close,
            _ => {
                self.cancel_armed = None;
            }
        }
        TaskPanelAction::None
    }
}

enum TaskPanelAction {
    None,
    Refresh,
    Cancel(String),
    Open(Box<SubagentTaskSnapshot>),
    Close,
}

pub(crate) fn is_task_panel_key(key: &KeyEvent) -> bool {
    key.code == KeyCode::Char('b') && key.modifiers.contains(KeyModifiers::CONTROL)
}

fn task_is_running(task: &SubagentTaskSnapshot) -> bool {
    task.status == SubagentStatus::Running
}

fn task_status_rank(status: SubagentStatus) -> u8 {
    match status {
        SubagentStatus::Running => 0,
        _ => 1,
    }
}

fn task_status_label(status: SubagentStatus) -> &'static str {
    match status {
        SubagentStatus::Running => "running",
        SubagentStatus::Completed => "completed",
        SubagentStatus::Failed => "failed",
        SubagentStatus::Cancelled => "cancelled",
        _ => "unknown",
    }
}

fn finalize_task_snapshots(mut tasks: Vec<SubagentTaskSnapshot>) -> Vec<SubagentTaskSnapshot> {
    tasks.sort_by(|left, right| {
        task_status_rank(left.status)
            .cmp(&task_status_rank(right.status))
            .then_with(|| right.updated_ms.cmp(&left.updated_ms))
            .then_with(|| left.task_id.cmp(&right.task_id))
    });
    let mut terminal = 0usize;
    tasks.retain(|task| {
        if task_is_running(task) {
            return true;
        }
        terminal = terminal.saturating_add(1);
        terminal <= TASK_PANEL_MAX_RECENT
    });
    tasks
}

fn task_matches_query(task: &SubagentTaskSnapshot, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let progress = task
        .progress
        .iter()
        .map(|entry| entry.status.as_str())
        .collect::<Vec<_>>()
        .join(" ");
    let haystack = format!(
        "{} {} {} {} {} {} {}",
        task.task_id,
        task.child_session_id,
        task.agent,
        task.description,
        task_status_label(task.status),
        progress,
        task.output.as_deref().unwrap_or_default(),
    )
    .to_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

fn task_status_prefix(status: SubagentStatus) -> &'static str {
    match status {
        SubagentStatus::Running => "◐",
        SubagentStatus::Completed => "✓",
        SubagentStatus::Failed => "!",
        SubagentStatus::Cancelled => "×",
        _ => "?",
    }
}

fn task_status_color(status: SubagentStatus) -> Color {
    match status {
        SubagentStatus::Running => TN_CYAN,
        SubagentStatus::Completed => TN_GREEN,
        SubagentStatus::Failed => TN_RED,
        SubagentStatus::Cancelled => TN_YELLOW,
        _ => TN_GRAY,
    }
}

fn task_panel_now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

fn task_duration_label(milliseconds: u64) -> String {
    let seconds = milliseconds / 1_000;
    match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}

fn task_time_label(task: &SubagentTaskSnapshot, now_ms: u64) -> String {
    if task_is_running(task) {
        return task_duration_label(now_ms.saturating_sub(task.started_ms));
    }
    let terminal_ms = task.finished_ms.unwrap_or(task.updated_ms);
    format!(
        "{} ago",
        task_duration_label(now_ms.saturating_sub(terminal_ms))
    )
}

fn task_compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn task_panel_footer(panel: &TaskPanel) -> String {
    if let Some(task_id) = panel.cancel_inflight.as_deref() {
        return format!("Cancelling {}…", truncate(task_id, 48));
    }
    if let Some(task_id) = panel.cancel_armed.as_deref() {
        return format!(
            "Press X again to cancel {} · Esc disarm",
            truncate(task_id, 48)
        );
    }
    if let Some(error) = panel.error.as_deref() {
        return format!("Cancellation failed · {}", task_compact_text(error));
    }
    if panel.preview {
        return panel.selected_task().map_or_else(
            || "Peek · no matching task".to_string(),
            |task| {
                let detail = task
                    .output
                    .as_deref()
                    .filter(|output| !output.trim().is_empty())
                    .or_else(|| task.progress.last().map(|entry| entry.status.as_str()))
                    .unwrap_or(&task.description);
                format!("Peek · {}", task_compact_text(detail))
            },
        );
    }
    let running = panel
        .tasks
        .iter()
        .filter(|task| task_is_running(task))
        .count();
    let recent = panel.tasks.len().saturating_sub(running);
    format!("{running} running · {recent} recent · auto-refresh 1s · Space peek")
}

fn task_menu_panel(panel: &TaskPanel, max_items: usize) -> MenuPanel {
    let indices = panel.visible_indices();
    let now_ms = task_panel_now_ms();
    let mut items = indices
        .iter()
        .filter_map(|index| panel.tasks.get(*index))
        .map(|task| {
            let label = if task.description.trim().is_empty() {
                task.task_id.clone()
            } else {
                task.description.clone()
            };
            let progress = task
                .progress
                .last()
                .map(|entry| format!(" · {}", task_compact_text(&entry.status)))
                .unwrap_or_default();
            MenuItem::new(label)
                .prefix(task_status_prefix(task.status))
                .description(format!(
                    "{} · {} · {}{}",
                    task.agent,
                    task_status_label(task.status),
                    task_time_label(task, now_ms),
                    progress,
                ))
                .color(task_status_color(task.status))
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        let label = if panel.loading && panel.tasks.is_empty() {
            "(loading delegated tasks…)"
        } else if panel.query.trim().is_empty() {
            "(no delegated tasks in this session)"
        } else {
            "(no tasks match this filter)"
        };
        items.push(MenuItem::new(label).disabled(true));
    }
    let title = if panel.loading && !panel.tasks.is_empty() {
        "Tasks and background work · refreshing…"
    } else {
        "Tasks and background work"
    };
    let subtitle = if panel.searching {
        format!(
            "Filter: {}▌ · type to refine · ↑/↓ select · Enter details · Esc done · Ctrl+U clear",
            panel.query
        )
    } else if panel.query.is_empty() {
        "↑/↓ task · / search · Enter details · X cancel · R refresh · Ctrl+B/Esc close".to_string()
    } else {
        format!(
            "Filter: {} · / edit · C clear · Enter details · X cancel · R refresh",
            panel.query
        )
    };

    MenuPanel::new(title)
        .subtitle(subtitle)
        .items(items)
        .selected(panel.selected_index())
        .max_items(max_items)
        .footer(task_panel_footer(panel))
        .indent(2)
        .marker("›")
        .title_color(ACCENT)
        .subtitle_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED)
        .disabled_color(TN_GRAY)
}

fn task_panel_max_rows(height: usize) -> usize {
    height
        .saturating_sub(9)
        .clamp(3, TASK_PANEL_MAX_VISIBLE_ROWS)
}

fn task_panel_height(panel: &TaskPanel, max_items: usize) -> usize {
    let item_count = panel.visible_indices().len().max(1).min(max_items);
    // Title + subtitle + rows + scroll allowance + footer.
    4 + item_count
}

fn task_menu_lines(panel: &TaskPanel, width: usize, max_items: usize) -> Vec<String> {
    task_menu_panel(panel, max_items)
        .view(
            width.min(u16::MAX as usize) as u16,
            task_panel_height(panel, max_items),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

fn task_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

fn task_panel_tick(generation: u64) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        tokio::time::sleep(TASK_PANEL_REFRESH_INTERVAL).await;
        Msg::TaskPanelTick { generation }
    })
}

fn task_output_title(task_id: &str) -> String {
    let safe = task_id
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(48)
        .collect::<String>();
    format!(
        "task-{}.txt",
        if safe.is_empty() { "details" } else { &safe }
    )
}

fn task_output_document(task: &SubagentTaskSnapshot) -> String {
    let mut output = format!(
        "Task ID: {}\nAgent: {}\nStatus: {}\nChild session: {}\nDescription: {}\n",
        task.task_id,
        task.agent,
        task_status_label(task.status),
        task.child_session_id,
        task.description,
    );
    if !task.progress.is_empty() {
        output.push_str("\nProgress:\n");
        for entry in &task.progress {
            output.push_str(&format!("- {} · {}", entry.timestamp_ms, entry.status));
            if !entry.metadata.is_null()
                && entry
                    .metadata
                    .as_object()
                    .is_none_or(|metadata| !metadata.is_empty())
            {
                output.push_str(" · ");
                output.push_str(&entry.metadata.to_string());
            }
            output.push('\n');
        }
    }
    output.push_str("\nOutput:\n");
    output.push_str(
        task.output
            .as_deref()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or("(no output yet)"),
    );
    output
}

impl App {
    pub(crate) fn open_task_panel(&mut self) -> Option<Cmd<Msg>> {
        self.task_panel_seq = self.task_panel_seq.wrapping_add(1).max(1);
        let generation = self.task_panel_seq;
        self.task_panel = Some(TaskPanel::loading(generation, self.session_id.clone()));
        let refresh = self.refresh_task_panel()?;
        Some(cmd::batch(vec![refresh, task_panel_tick(generation)]))
    }

    pub(crate) fn toggle_task_panel(&mut self) -> Option<Cmd<Msg>> {
        if self.task_panel.is_some() {
            self.task_panel = None;
            None
        } else {
            self.open_task_panel()
        }
    }

    fn refresh_task_panel(&mut self) -> Option<Cmd<Msg>> {
        let panel = self.task_panel.as_mut()?;
        panel.request_id = panel.request_id.wrapping_add(1).max(1);
        panel.loading = true;
        let request_id = panel.request_id;
        let generation = panel.generation;
        let session_id = panel.session_id.clone();
        let session = Arc::clone(&self.session);
        Some(cmd::cmd(move || async move {
            Msg::TaskPanelData {
                session_id,
                generation,
                request_id,
                tasks: session.subagent_tasks().await,
            }
        }))
    }

    pub(crate) fn apply_task_panel_data(
        &mut self,
        session_id: String,
        generation: u64,
        request_id: u64,
        tasks: Vec<SubagentTaskSnapshot>,
    ) {
        let Some(panel) = self.task_panel.as_mut() else {
            return;
        };
        panel.apply_data(&session_id, generation, request_id, tasks);
    }

    pub(crate) fn handle_task_panel_tick(&mut self, generation: u64) -> Option<Cmd<Msg>> {
        let panel = self.task_panel.as_ref()?;
        if panel.generation != generation {
            return None;
        }
        let tick = task_panel_tick(generation);
        if panel.loading {
            return Some(tick);
        }
        match self.refresh_task_panel() {
            Some(refresh) => Some(cmd::batch(vec![refresh, tick])),
            None => Some(tick),
        }
    }

    fn cancel_task_from_panel(&mut self, task_id: String) -> Option<Cmd<Msg>> {
        let panel = self.task_panel.as_ref()?;
        let generation = panel.generation;
        let session_id = panel.session_id.clone();
        let session = Arc::clone(&self.session);
        Some(cmd::cmd(move || async move {
            let cancelled = session.cancel_subagent_task(&task_id).await;
            Msg::TaskPanelCancelFinished {
                session_id,
                generation,
                task_id,
                cancelled,
            }
        }))
    }

    pub(crate) fn apply_task_panel_cancel_result(
        &mut self,
        session_id: String,
        generation: u64,
        task_id: String,
        cancelled: bool,
    ) -> Option<Cmd<Msg>> {
        let panel = self.task_panel.as_mut()?;
        if !panel.accepts(&session_id, generation)
            || panel.cancel_inflight.as_deref() != Some(task_id.as_str())
        {
            return None;
        }
        panel.cancel_inflight = None;
        panel.error = (!cancelled).then(|| {
            "The task already finished or no live cancellation handle remains.".to_string()
        });
        self.refresh_task_panel()
    }

    pub(crate) fn handle_task_panel_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let action = self.task_panel.as_mut()?.handle_key(key);
        match action {
            TaskPanelAction::None => None,
            TaskPanelAction::Refresh => self.refresh_task_panel(),
            TaskPanelAction::Cancel(task_id) => self.cancel_task_from_panel(task_id),
            TaskPanelAction::Open(task) => {
                self.task_panel = None;
                self.open_readonly_in_ide(
                    &task_output_title(&task.task_id),
                    &task_output_document(&task),
                );
                None
            }
            TaskPanelAction::Close => {
                self.task_panel = None;
                None
            }
        }
    }

    pub(crate) fn handle_task_panel_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.task_panel.as_mut()?.move_selection(-1);
                return None;
            }
            MouseEventKind::ScrollDown => {
                self.task_panel.as_mut()?.move_selection(1);
                return None;
            }
            _ => {}
        }
        let panel = self.task_panel.as_ref()?;
        let max_items = task_panel_max_rows(self.height as usize);
        let height = task_panel_height(panel, max_items);
        let mut menu = task_menu_panel(panel, max_items);
        let row_count = menu.view(self.width, height).lines().count();
        if row_count == 0 {
            return None;
        }
        menu.set_y_offset(task_overlay_y_offset(
            self.height as usize,
            row_count,
            self.overlay_rows_below(),
        ));
        match menu.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                if let Some(panel) = self.task_panel.as_mut() {
                    panel.select_visible_index(index);
                    panel.preview = true;
                }
                None
            }
            Some(MenuPanelMsg::Cancelled) | None => None,
        }
    }

    pub(crate) fn overlay_task_menu(&self, composed: String) -> String {
        let Some(panel) = self.task_panel.as_ref() else {
            return composed;
        };
        let menu = task_menu_lines(
            panel,
            self.width as usize,
            task_panel_max_rows(self.height as usize),
        );
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
#[path = "tasks_tests.rs"]
mod tests;
