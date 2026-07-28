//! `/relay` picker: resume A3S Code sessions or continue external-agent tasks.

use super::super::*;
use a3s_tui::components::{TabbedMenuItem, TabbedMenuPanel, TabbedMenuPanelMsg, TabbedMenuTab};
use a3s_tui::event::MouseEventKind;

pub(crate) use super::relay_scan::RelaySession;
use super::relay_scan::{
    scan_relay_sessions, RelayAgent, RelaySessionIdentity, RelaySessionStatus,
};

const RELAY_MAX_VISIBLE_ROWS: usize = 12;
const RELAY_REFRESH_INTERVAL: Duration = Duration::from_secs(15);

pub(crate) struct RelayPanel {
    generation: u64,
    request_id: u64,
    sessions: Vec<RelaySession>,
    tab: usize,
    selected_by_agent: [Option<RelaySessionIdentity>; RelayAgent::ALL.len()],
    query: String,
    searching: bool,
    preview: bool,
    loading: bool,
    error: Option<String>,
    current_session_id: String,
}

impl RelayPanel {
    fn loading(generation: u64, request_id: u64, current_session_id: String) -> Self {
        Self {
            generation,
            request_id,
            sessions: Vec::new(),
            tab: 0,
            selected_by_agent: std::array::from_fn(|_| None),
            query: String::new(),
            searching: false,
            preview: false,
            loading: true,
            error: None,
            current_session_id,
        }
    }

    fn active_agent(&self) -> RelayAgent {
        RelayAgent::ALL[self.tab.min(RelayAgent::ALL.len() - 1)]
    }

    fn indices_for_tab(&self, tab: usize) -> Vec<usize> {
        let agent = RelayAgent::ALL[tab.min(RelayAgent::ALL.len() - 1)];
        self.sessions
            .iter()
            .enumerate()
            .filter_map(|(index, session)| {
                (session.agent == agent && relay_session_matches_query(session, &self.query))
                    .then_some(index)
            })
            .collect()
    }

    fn active_indices(&self) -> Vec<usize> {
        self.indices_for_tab(self.tab)
    }

    fn selected_index(&self) -> usize {
        let indices = self.active_indices();
        let selected = self
            .selected_by_agent
            .get(self.tab)
            .and_then(Option::as_ref);
        selected
            .and_then(|selected| {
                indices.iter().position(|index| {
                    self.sessions
                        .get(*index)
                        .is_some_and(|session| &session.identity == selected)
                })
            })
            .unwrap_or(0)
            .min(indices.len().saturating_sub(1))
    }

    fn selected_session(&self) -> Option<&RelaySession> {
        let indices = self.active_indices();
        indices
            .get(self.selected_index())
            .and_then(|index| self.sessions.get(*index))
    }

    fn remember_visible_index(&mut self, tab: usize, visible_index: usize) {
        let tab = tab.min(RelayAgent::ALL.len() - 1);
        let identity = self
            .indices_for_tab(tab)
            .get(visible_index)
            .and_then(|index| self.sessions.get(*index))
            .map(|session| session.identity.clone());
        if identity.is_some() {
            self.selected_by_agent[tab] = identity;
        }
    }

    fn reconcile_selections(&mut self) {
        self.tab = self.tab.min(RelayAgent::ALL.len() - 1);
        for tab in 0..RelayAgent::ALL.len() {
            let agent = RelayAgent::ALL[tab];
            let selection_still_exists =
                self.selected_by_agent[tab]
                    .as_ref()
                    .is_some_and(|identity| {
                        self.sessions
                            .iter()
                            .any(|session| session.agent == agent && &session.identity == identity)
                    });
            if selection_still_exists {
                continue;
            }
            self.selected_by_agent[tab] = self
                .indices_for_tab(tab)
                .first()
                .and_then(|index| self.sessions.get(*index))
                .or_else(|| self.sessions.iter().find(|session| session.agent == agent))
                .map(|session| session.identity.clone());
        }
    }

    fn set_tab(&mut self, tab: usize) {
        self.tab = tab.min(RelayAgent::ALL.len() - 1);
    }

    fn move_selection(&mut self, amount: isize) {
        let indices = self.active_indices();
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
        self.remember_visible_index(self.tab, next);
    }

    fn move_selection_to(&mut self, index: usize) {
        let last = self.active_indices().len().saturating_sub(1);
        self.remember_visible_index(self.tab, index.min(last));
    }

    fn select_item(&mut self, tab: usize, item: usize) -> Option<RelaySession> {
        self.set_tab(tab);
        let session = self
            .indices_for_tab(self.tab)
            .get(item)
            .and_then(|index| self.sessions.get(*index))
            .cloned()?;
        self.selected_by_agent[self.tab] = Some(session.identity.clone());
        Some(session)
    }

    fn apply_scan(&mut self, request_id: u64, result: Result<Vec<RelaySession>, String>) -> bool {
        if self.request_id != request_id {
            return false;
        }
        self.loading = false;
        match result {
            Ok(sessions) => {
                self.sessions = sessions;
                self.error = None;
            }
            Err(error) => {
                self.error = Some(error);
            }
        }
        self.reconcile_selections();
        true
    }

    fn accepts_refresh_tick(&self, generation: u64) -> bool {
        self.generation == generation
    }

    fn handle_search_key(&mut self, key: &KeyEvent) -> RelayPanelAction {
        match key.code {
            KeyCode::Esc => self.searching = false,
            KeyCode::Enter => {
                return self
                    .selected_session()
                    .cloned()
                    .map_or(RelayPanelAction::None, RelayPanelAction::Activate);
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-(RELAY_MAX_VISIBLE_ROWS as isize)),
            KeyCode::PageDown => self.move_selection(RELAY_MAX_VISIBLE_ROWS as isize),
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Left | KeyCode::BackTab => self.set_tab(self.tab.saturating_sub(1)),
            KeyCode::Right | KeyCode::Tab => {
                self.set_tab((self.tab + 1).min(RelayAgent::ALL.len() - 1));
            }
            KeyCode::Backspace => {
                self.query.pop();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.clear();
            }
            KeyCode::Char(character)
                if !key
                    .modifiers
                    .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.query.push(character);
            }
            _ => {}
        }
        RelayPanelAction::None
    }

    fn handle_key(&mut self, key: &KeyEvent) -> RelayPanelAction {
        if self.searching {
            return self.handle_search_key(key);
        }
        match key.code {
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => self.move_selection(-(RELAY_MAX_VISIBLE_ROWS as isize)),
            KeyCode::PageDown => self.move_selection(RELAY_MAX_VISIBLE_ROWS as isize),
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Left | KeyCode::BackTab => self.set_tab(self.tab.saturating_sub(1)),
            KeyCode::Right | KeyCode::Tab => {
                self.set_tab((self.tab + 1).min(RelayAgent::ALL.len() - 1));
            }
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('c' | 'C') if !self.query.is_empty() => self.query.clear(),
            KeyCode::Char(' ' | 'p' | 'P') => self.preview = !self.preview,
            KeyCode::Char('r' | 'R') => return RelayPanelAction::Refresh,
            KeyCode::Enter => {
                return self
                    .selected_session()
                    .cloned()
                    .map_or(RelayPanelAction::None, RelayPanelAction::Activate);
            }
            KeyCode::Esc => return RelayPanelAction::Close,
            _ => {}
        }
        RelayPanelAction::None
    }
}

enum RelayPanelAction {
    None,
    Refresh,
    Activate(RelaySession),
    Close,
}

fn relay_agent_color(agent: RelayAgent) -> Color {
    match agent {
        RelayAgent::A3sCode => ACCENT,
        RelayAgent::ClaudeCode => TN_ORANGE,
        RelayAgent::Codex => TN_CYAN,
        RelayAgent::WorkBuddy => TN_PURPLE,
    }
}

fn relay_session_matches_query(session: &RelaySession, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    let identity = match &session.identity {
        RelaySessionIdentity::Native(id) => id.clone(),
        RelaySessionIdentity::Transcript { path, .. } => path.to_string_lossy().into_owned(),
    };
    let haystack = format!(
        "{} {} {} {} {} {}",
        session.agent.label(),
        session.status.label(),
        session.label,
        session.seed.as_deref().unwrap_or_default(),
        session.persisted_model.as_deref().unwrap_or_default(),
        identity,
    )
    .to_lowercase();
    query
        .split_whitespace()
        .all(|term| haystack.contains(&term.to_lowercase()))
}

fn relay_compact_text(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn relay_panel_footer(panel: &RelayPanel) -> String {
    if let Some(error) = panel.error.as_deref() {
        return format!("Refresh failed · {}", relay_compact_text(error));
    }
    if panel.preview {
        return panel.selected_session().map_or_else(
            || "Peek · no matching session".to_string(),
            |session| {
                let task = session.seed.as_deref().unwrap_or(&session.label);
                format!("Peek · {}", relay_compact_text(task))
            },
        );
    }
    let count = panel.active_indices().len();
    format!(
        "{count} {} {} · auto-refresh 15s · Space peek",
        panel.active_agent().label(),
        if count == 1 { "session" } else { "sessions" },
    )
}

fn relay_max_rows(height: usize) -> usize {
    height.saturating_sub(9).clamp(3, RELAY_MAX_VISIBLE_ROWS)
}

fn relay_panel_height(panel: &RelayPanel, max_items: usize) -> usize {
    let item_count = panel.active_indices().len().max(1).min(max_items);
    // Title + tab strip + hint + rows + scroll allowance + status/peek footer.
    5 + item_count
}

fn relay_menu_panel(panel: &RelayPanel, max_items: usize) -> TabbedMenuPanel {
    let empty_text = if panel.loading && panel.sessions.is_empty() {
        "(scanning sessions…)".to_string()
    } else if !panel.query.trim().is_empty() {
        format!("(no matches for \"{}\")", truncate(panel.query.trim(), 48))
    } else {
        "(no sessions for this workspace)".to_string()
    };
    let tabs = RelayAgent::ALL
        .into_iter()
        .map(|agent| {
            let items = panel
                .sessions
                .iter()
                .filter(|session| {
                    session.agent == agent && relay_session_matches_query(session, &panel.query)
                })
                .map(|session| {
                    TabbedMenuItem::new(session.label.clone())
                        .prefix(relay_session_prefix(session, &panel.current_session_id))
                        .description(relay_session_description(
                            session,
                            &panel.current_session_id,
                            SystemTime::now(),
                        ))
                })
                .collect::<Vec<_>>();
            TabbedMenuTab::new(agent.label(), relay_agent_color(agent))
                .items(items)
                .empty_text(empty_text.clone())
        })
        .collect::<Vec<_>>();

    let title = if panel.loading && !panel.sessions.is_empty() {
        "Sessions and background work · refreshing…"
    } else {
        "Sessions and background work"
    };
    let hint = if panel.searching {
        format!(
            "Filter: {}▌ · type to refine · ↑/↓ select · Enter continue · Esc done · Ctrl+U clear",
            panel.query
        )
    } else if panel.query.is_empty() {
        "↑/↓ session · ←/→ agent · / search · Enter continue · R refresh · Esc".to_string()
    } else {
        format!(
            "Filter: {} · / edit · C clear · Enter continue · R refresh · Esc",
            panel.query
        )
    };

    TabbedMenuPanel::new(tabs)
        .title(title)
        .hint(hint)
        .active_tab(panel.tab)
        .selected(panel.selected_index())
        .max_items(max_items)
        .footer(relay_panel_footer(panel))
        .indent(2)
        .hint_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED)
}

fn relay_session_prefix(session: &RelaySession, current_session_id: &str) -> &'static str {
    if session.native_id.as_deref() == Some(current_session_id) {
        return "●";
    }
    if session.active_runs > 0 || session.active_subagents > 0 {
        return "◐";
    }
    match session.status {
        RelaySessionStatus::Saved => "○",
        RelaySessionStatus::Paused => "Ⅱ",
        RelaySessionStatus::Completed => "✓",
        RelaySessionStatus::Error => "!",
        RelaySessionStatus::External => "⮌",
    }
}

fn relay_session_description(
    session: &RelaySession,
    current_session_id: &str,
    now: SystemTime,
) -> String {
    let mut parts = vec![
        if session.native_id.as_deref() == Some(current_session_id) {
            "current".to_string()
        } else {
            session.status.label().to_string()
        },
    ];
    if session.active_runs > 0 {
        parts.push(format!(
            "{} unfinished {}",
            session.active_runs,
            if session.active_runs == 1 {
                "run"
            } else {
                "runs"
            }
        ));
    }
    if session.active_subagents > 0 {
        parts.push(format!(
            "{} background {}",
            session.active_subagents,
            if session.active_subagents == 1 {
                "agent"
            } else {
                "agents"
            }
        ));
    }
    if let Some(model) = session.persisted_model.as_deref() {
        parts.push(truncate(model, 28));
    }
    parts.push(relative_session_age(session.modified, now));
    parts.join(" · ")
}

fn relative_session_age(modified: SystemTime, now: SystemTime) -> String {
    let seconds = now.duration_since(modified).unwrap_or_default().as_secs();
    match seconds {
        0..=59 => "now".to_string(),
        60..=3_599 => format!("{}m", seconds / 60),
        3_600..=86_399 => format!("{}h", seconds / 3_600),
        _ => format!("{}d", seconds / 86_400),
    }
}

fn relay_menu_lines(panel: &RelayPanel, width: usize, max_items: usize) -> Vec<String> {
    relay_menu_panel(panel, max_items)
        .view(
            width.min(u16::MAX as usize) as u16,
            relay_panel_height(panel, max_items),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

fn relay_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

fn relay_refresh_tick(generation: u64) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        tokio::time::sleep(RELAY_REFRESH_INTERVAL).await;
        Msg::RelayRefreshTick { generation }
    })
}

impl App {
    pub(crate) fn open_relay_panel(&mut self) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        if self.goal_run.is_some() {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  stop the active goal with /goal clear before switching sessions"),
            );
            return None;
        }
        self.relay_scan_seq = self.relay_scan_seq.wrapping_add(1).max(1);
        let generation = self.relay_scan_seq;
        self.relay_panel = Some(RelayPanel::loading(generation, 0, self.session_id.clone()));
        let refresh = self.refresh_relay_panel()?;
        Some(cmd::batch(vec![refresh, relay_refresh_tick(generation)]))
    }

    fn refresh_relay_panel(&mut self) -> Option<Cmd<Msg>> {
        let panel = self.relay_panel.as_mut()?;
        self.relay_scan_seq = self.relay_scan_seq.wrapping_add(1).max(1);
        let request_id = self.relay_scan_seq;
        panel.request_id = request_id;
        panel.loading = true;
        panel.error = None;
        let store = Arc::clone(&self.store);
        let workspace = PathBuf::from(&self.cwd);
        let current_session = Arc::clone(&self.session);
        Some(cmd::cmd(move || async move {
            Msg::RelayData {
                request_id,
                result: scan_relay_sessions(store, workspace, current_session).await,
            }
        }))
    }

    pub(crate) fn apply_relay_scan(
        &mut self,
        request_id: u64,
        result: Result<Vec<RelaySession>, String>,
    ) {
        let Some(panel) = self.relay_panel.as_mut() else {
            return;
        };
        panel.apply_scan(request_id, result);
    }

    pub(crate) fn handle_relay_refresh_tick(&mut self, generation: u64) -> Option<Cmd<Msg>> {
        let panel = self.relay_panel.as_ref()?;
        if !panel.accepts_refresh_tick(generation) {
            return None;
        }
        let tick = relay_refresh_tick(generation);
        if panel.loading {
            return Some(tick);
        }
        match self.refresh_relay_panel() {
            Some(refresh) => Some(cmd::batch(vec![refresh, tick])),
            None => Some(tick),
        }
    }

    pub(crate) fn handle_relay_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let action = self.relay_panel.as_mut()?.handle_key(key);
        match action {
            RelayPanelAction::None => None,
            RelayPanelAction::Refresh => self.refresh_relay_panel(),
            RelayPanelAction::Activate(session) => self.activate_relay_session(session),
            RelayPanelAction::Close => {
                self.relay_panel = None;
                None
            }
        }
    }

    pub(crate) fn handle_relay_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.relay_panel.as_mut()?.move_selection(-1);
                return None;
            }
            MouseEventKind::ScrollDown => {
                self.relay_panel.as_mut()?.move_selection(1);
                return None;
            }
            _ => {}
        }
        let panel = self.relay_panel.as_ref()?;
        let max_items = relay_max_rows(self.height as usize);
        let height = relay_panel_height(panel, max_items);
        let mut menu = relay_menu_panel(panel, max_items);
        let row_count = menu.view(self.width, height).lines().count();
        if row_count == 0 {
            return None;
        }
        menu.set_y_offset(relay_overlay_y_offset(
            self.height as usize,
            row_count,
            self.overlay_rows_below(),
        ));
        match menu.handle_mouse(mouse) {
            Some(TabbedMenuPanelMsg::TabChanged(tab)) => {
                if let Some(panel) = self.relay_panel.as_mut() {
                    panel.set_tab(tab);
                }
                None
            }
            Some(TabbedMenuPanelMsg::Selected { tab, item }) => {
                let session = self
                    .relay_panel
                    .as_mut()
                    .and_then(|panel| panel.select_item(tab, item));
                session.and_then(|session| self.activate_relay_session(session))
            }
            Some(TabbedMenuPanelMsg::Cancelled) | None => None,
        }
    }

    fn activate_relay_session(&mut self, target: RelaySession) -> Option<Cmd<Msg>> {
        self.relay_panel = None;
        if let Some(session_id) = target.native_id.as_deref() {
            if session_id == self.session_id {
                self.push_line(
                    &Style::new()
                        .fg(TN_GRAY)
                        .render("  this A3S Code session is already active"),
                );
                return None;
            }
            if let Err(error) = self.persist_tui_session_state() {
                self.push_line(&Style::new().fg(TN_RED).render(&format!(
                    "  could not save current session settings before relay: {error}"
                )));
                return None;
            }
            let (profile, restore) = match self.relay_restore_profile(&target) {
                Ok(result) => result,
                Err(error) => {
                    self.push_line(
                        &Style::new()
                            .fg(TN_RED)
                            .render(&format!("  could not resume relay session: {error}")),
                    );
                    return None;
                }
            };
            return self.start_session_rebuild(profile, SessionRebuildAction::Relay { restore });
        }

        let Some(seed) = target.seed else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  the selected transcript has no user task to relay"),
            );
            return None;
        };
        let source = target.agent.label();
        self.messages.push(TranscriptEntry::preformatted(gutter(
            relay_agent_color(target.agent),
            &format!("⮌ relaying from {source}: {}", truncate(&seed, 72)),
        )));
        let prompt = format!(
            "The following unfinished task comes from a {source} session in this workspace. \
             Inspect the current workspace state, determine what remains unfinished, then continue \
             and complete the task without assuming that earlier edits were applied:\n\n{seed}"
        );
        self.start_stream_inner(
            prompt,
            format!("⮌ {source}: {}", truncate(&seed, 56)),
            true,
            false,
            false,
        )
    }

    fn relay_restore_profile(
        &self,
        target: &RelaySession,
    ) -> Result<(SessionRebuildProfile, RelayRestoreState), String> {
        let session_id = target
            .native_id
            .clone()
            .ok_or_else(|| "native session id is missing".to_string())?;
        let state = load_tui_session_state(Path::new(&self.cwd), &session_id)
            .map_err(|error| format!("could not load its TUI settings: {error}"))?;
        let effort = state
            .as_ref()
            .and_then(TuiSessionState::effort_index)
            .or_else(load_tui_effort_preference)
            .unwrap_or(DEFAULT_TUI_EFFORT_INDEX)
            .min(EFFORT_LEVELS.len().saturating_sub(1));
        let configured =
            app_launch::configured_model_preference(target.persisted_model.clone(), &self.models);
        let global = load_model_selection_preference().filter(|preference| {
            target.persisted_model.as_deref().is_none_or(|model| {
                app_launch::preference_matches_persisted_model(preference, model)
            })
        });
        let preference = state
            .as_ref()
            .and_then(|state| state.model.clone())
            .or(configured)
            .or(global);
        let restored = preference.as_ref().and_then(|preference| {
            restore_model_selection(
                preference,
                &self.models,
                self.os_session.as_ref(),
                &session_id,
                effort,
            )
        });
        let model_source = restored
            .as_ref()
            .and(preference.as_ref())
            .map(|preference| preference.source)
            .unwrap_or(ModelSelectionSource::Config);
        let model = restored
            .as_ref()
            .map(|(model, _)| model.clone())
            .or_else(|| self.code_config.default_model.clone())
            .or_else(|| self.models.first().cloned());
        let llm_override = restored.and_then(|(_, client)| client);
        let context_limit = model
            .as_deref()
            .map(|model| ctx_limit_for_model(&self.model_ctx, model))
            .unwrap_or_else(|| resolve_ctx_limit(None));
        let restore = RelayRestoreState {
            session_id: session_id.clone(),
            model: model.clone(),
            model_source,
            effort,
            mode: state
                .as_ref()
                .map(TuiSessionState::mode)
                .unwrap_or(Mode::Default),
            context_limit,
            llm_override: llm_override.clone(),
            theme: state.as_ref().and_then(TuiSessionState::theme_index),
            paused_goal: state.and_then(|state| state.paused_goal),
        };
        let profile = SessionRebuildProfile {
            session_id,
            model,
            effort,
            context_limit,
            llm_override,
            compact_summary: None,
        };
        Ok((profile, restore))
    }

    pub(crate) fn commit_relay_session(
        &mut self,
        session: AgentSession,
        restore: RelayRestoreState,
    ) {
        let resumed = session.history();
        let entries = app_launch::resumed_transcript_entries(&resumed);
        let history = resumed
            .iter()
            .filter(|message| message.role == "user")
            .map(|message| message.text().trim().to_string())
            .filter(|text| !text.is_empty())
            .collect::<Vec<_>>();
        let auto_review_revision = u64::try_from(history.len()).unwrap_or(u64::MAX);

        self.restore_autonomy();
        self.session_id = restore.session_id;
        self.model = restore.model;
        self.model_source = restore.model_source;
        self.effort = restore.effort;
        self.set_composer_mode(restore.mode);
        self.context_limit = restore.context_limit;
        self.llm_override = restore.llm_override;
        if let Some(theme) = restore.theme {
            SYNTAX_THEME.store(
                theme.min(THEMES.len().saturating_sub(1)),
                std::sync::atomic::Ordering::Relaxed,
            );
        }
        self.replace_session(session);
        self.messages = Transcript::from_entries(entries);
        self.history = history;
        self.history_pos = None;
        self.history_draft = None;
        self.auto_review = AutoReviewTracker::new(auto_review_revision);
        self.compact_summary = None;
        self.output_tokens = 0;
        self.last_prompt_tokens = 0;
        self.ctx_warned_tier = 0;
        self.completed = 0;
        self.plan.clear();
        self.runtime.clear_turn_entities();
        self.runtime.clear_subagent_entities();
        self.background_subagent_watches.clear();
        self.invalidate_subagent_snapshots();
        self.queue.clear();
        self.queued_turn_modes.clear();
        self.queued_plan_drafts.clear();
        self.send_now_queued_sequence = None;
        self.queue_panel = None;
        self.active_queued_turn = None;
        self.active_queued_turn_token = None;
        self.active_turn_mode = None;
        self.active_plan_draft = None;
        self.active_rewind_checkpoint = None;
        self.rewind_checkpoints.clear();
        self.rewind_finalization_pending = None;
        self.pending_plan_review = None;
        self.plan_review = None;
        self.queue_retry_generation = self.queue_retry_generation.wrapping_add(1);
        self.queue_retry_attempt = 0;
        self.running_task = None;
        self.restore_current_approval_feedback();
        self.pending_tools.clear();
        self.permission_rule_write_inflight = None;
        self.pending_images.clear();
        self.pending_ctx = None;
        self.ctx_hits.clear();
        self.review = None;
        self.review_open = false;
        self.review_pending = false;
        self.sleep_pending = false;
        self.loop_remaining = 0;
        self.loop_panel = None;
        self.goal = None;
        self.goal_since = None;
        self.goal_run = None;
        self.pending_goal_failure = None;
        self.paused_goal = restore.paused_goal;
        self.goal_resume_prompt = self.paused_goal.as_ref().map(|_| 0);
        self.agent_dev = None;
        self.pending_agent_subcommand = None;
        self.mcp_dev = None;
        self.pending_mcp_subcommand = None;
        self.skill_dev = None;
        self.pending_skill_subcommand = None;
        self.okf_dev = None;
        self.pending_okf_subcommand = None;
        self.streaming.clear();
        self.thinking.clear();
        self.turn_text.clear();
        self.transcript_view = None;
        self.push_line(
            &Style::new()
                .fg(TN_GREEN)
                .render(&format!("  ⮌ resumed A3S Code session {}", self.session_id)),
        );
        self.relayout();
        self.rebuild_viewport();
    }

    pub(crate) fn overlay_relay_menu(&self, composed: String) -> String {
        let Some(panel) = self.relay_panel.as_ref() else {
            return composed;
        };
        let menu = relay_menu_lines(
            panel,
            self.width as usize,
            relay_max_rows(self.height as usize),
        );
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
#[path = "relay_tests.rs"]
mod tests;
