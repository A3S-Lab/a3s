//! Per-session TUI settings and paused-goal startup interaction.

use super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine as _};
use serde::{Deserialize, Serialize};
use std::fs::{self, File, OpenOptions};
use std::io::{Error, ErrorKind, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

const TUI_SESSION_STATE_SCHEMA_VERSION: u32 = 1;
const MAX_TUI_SESSION_STATE_BYTES: u64 = 1024 * 1024;
const GOAL_RESUME_PANEL_HEIGHT: usize = 5;
static TUI_SESSION_STATE_TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

/// Minimal host state needed to continue an unfinished durable goal without
/// pretending that the interrupted iteration completed.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub(crate) struct PausedGoalState {
    pub(super) loop_id: String,
    pub(super) goal: String,
    pub(super) iteration: usize,
    pub(super) progress: f32,
    pub(super) failures: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
enum PersistedMode {
    Default,
    Plan,
    Auto,
}

impl From<Mode> for PersistedMode {
    fn from(mode: Mode) -> Self {
        match mode {
            Mode::Default => Self::Default,
            Mode::Plan => Self::Plan,
            Mode::Auto => Self::Auto,
        }
    }
}

impl From<PersistedMode> for Mode {
    fn from(mode: PersistedMode) -> Self {
        match mode {
            PersistedMode::Default => Self::Default,
            PersistedMode::Plan => Self::Plan,
            PersistedMode::Auto => Self::Auto,
        }
    }
}

fn mode_to_persist(current: Mode, autonomy_restore: Option<Mode>) -> Mode {
    autonomy_restore.unwrap_or(current)
}

/// UI-owned settings that Core's generic session snapshot deliberately does
/// not know about. The file is keyed by session id inside the workspace.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub(super) struct TuiSessionState {
    schema_version: u32,
    session_id: String,
    #[serde(default)]
    pub(super) model: Option<ModelSelectionPreference>,
    #[serde(default)]
    effort: Option<String>,
    #[serde(default)]
    mode: Option<PersistedMode>,
    #[serde(default)]
    theme: Option<String>,
    #[serde(default)]
    pub(super) paused_goal: Option<PausedGoalState>,
}

impl TuiSessionState {
    fn capture(app: &App) -> Self {
        let model = app.model.as_ref().map(|model| ModelSelectionPreference {
            source: app.model_source,
            model: model.clone(),
        });
        let effort = EFFORT_LEVELS
            .get(app.effort)
            .map(|profile| profile.id.to_string());
        let theme = THEMES
            .get(
                SYNTAX_THEME
                    .load(std::sync::atomic::Ordering::Relaxed)
                    .min(THEMES.len().saturating_sub(1)),
            )
            .map(|theme| theme.name.to_string());
        let paused_goal = app
            .goal_run
            .as_ref()
            .map(panels::goal_engineering::GoalRunState::paused_state)
            .or_else(|| app.paused_goal.clone());
        // Autonomous one-off commands temporarily switch the runtime to Auto.
        // Persist the user's underlying mode, not that transient override.
        let mode = mode_to_persist(app.mode, app.autonomy_restore);

        Self {
            schema_version: TUI_SESSION_STATE_SCHEMA_VERSION,
            session_id: app.session_id.clone(),
            model,
            effort,
            mode: Some(mode.into()),
            theme,
            paused_goal,
        }
    }

    pub(super) fn capture_for_session(app: &App, session_id: impl Into<String>) -> Self {
        let mut state = Self::capture(app);
        state.session_id = session_id.into();
        state
    }

    pub(super) fn effort_index(&self) -> Option<usize> {
        let effort = self.effort.as_deref()?;
        EFFORT_LEVELS
            .iter()
            .position(|profile| profile.id == effort)
    }

    pub(super) fn mode(&self) -> Mode {
        self.mode.map(Mode::from).unwrap_or(Mode::Default)
    }

    pub(super) fn theme_index(&self) -> Option<usize> {
        let theme = self.theme.as_deref()?;
        THEMES.iter().position(|candidate| candidate.name == theme)
    }

    fn normalize(mut self) -> Self {
        if self.paused_goal.as_ref().is_some_and(|goal| {
            goal.loop_id.trim().is_empty()
                || goal.goal.trim().is_empty()
                || !goal.progress.is_finite()
        }) {
            self.paused_goal = None;
        }
        if let Some(goal) = self.paused_goal.as_mut() {
            goal.iteration = goal.iteration.max(1);
            goal.progress = goal.progress.clamp(0.0, 1.0);
        }
        self
    }
}

pub(crate) fn tui_session_state_path(workspace: &Path, session_id: &str) -> PathBuf {
    let key = URL_SAFE_NO_PAD.encode(session_id.as_bytes());
    workspace
        .join(".a3s")
        .join("tui")
        .join("session-state")
        .join("v1")
        .join(format!("id_{key}.json"))
}

pub(super) fn load_tui_session_state(
    workspace: &Path,
    session_id: &str,
) -> std::io::Result<Option<TuiSessionState>> {
    let path = tui_session_state_path(workspace, session_id);
    if !path.exists() {
        return Ok(None);
    }
    let metadata = std::fs::metadata(&path)?;
    if metadata.len() > MAX_TUI_SESSION_STATE_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!("TUI session state exceeds 1 MiB: {}", path.display()),
        ));
    }
    let bytes = std::fs::read(&path)?;
    let state: TuiSessionState = serde_json::from_slice(&bytes)
        .map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
    if state.schema_version != TUI_SESSION_STATE_SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "unsupported TUI session state schema {}; expected {}",
                state.schema_version, TUI_SESSION_STATE_SCHEMA_VERSION
            ),
        ));
    }
    if state.session_id != session_id {
        return Err(Error::new(
            ErrorKind::InvalidData,
            format!(
                "TUI session state belongs to session {:?}, not {:?}",
                state.session_id, session_id
            ),
        ));
    }
    Ok(Some(state.normalize()))
}

pub(super) fn save_tui_session_state(
    workspace: &Path,
    session_id: &str,
    state: &TuiSessionState,
) -> std::io::Result<()> {
    if state.schema_version != TUI_SESSION_STATE_SCHEMA_VERSION {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "cannot save an unsupported TUI session state schema",
        ));
    }
    if state.session_id != session_id {
        return Err(Error::new(
            ErrorKind::InvalidInput,
            "cannot save TUI state under a different session id",
        ));
    }
    let path = tui_session_state_path(workspace, session_id);
    let parent = path
        .parent()
        .ok_or_else(|| Error::new(ErrorKind::InvalidInput, "session state path has no parent"))?;
    fs::create_dir_all(parent)?;
    let mut body = serde_json::to_vec_pretty(state)
        .map_err(|error| Error::new(ErrorKind::InvalidData, error))?;
    body.push(b'\n');
    if body.len() as u64 > MAX_TUI_SESSION_STATE_BYTES {
        return Err(Error::new(
            ErrorKind::InvalidData,
            "TUI session state exceeds 1 MiB",
        ));
    }

    let temporary = temporary_tui_session_state_path(&path);
    let result = (|| {
        let mut options = OpenOptions::new();
        options.create_new(true).write(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let mut file = options.open(&temporary)?;
        file.write_all(&body)?;
        file.sync_all()?;
        drop(file);
        fs::rename(&temporary, &path)?;
        if let Ok(directory) = File::open(parent) {
            let _ = directory.sync_all();
        }
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn temporary_tui_session_state_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("session-state.json");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = TUI_SESSION_STATE_TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        timestamp,
        sequence
    ))
}

fn goal_resume_panel(goal: &PausedGoalState, selected: usize) -> MenuPanel {
    let summary = goal.goal.split_whitespace().collect::<Vec<_>>().join(" ");
    MenuPanel::new("Resume paused goal?")
        .subtitle(format!("Goal: {summary}"))
        .items(vec![
            MenuItem::new("Resume goal").description("Continue with the next goal iteration"),
            MenuItem::new("Leave paused").description("Keep it paused; use /goal resume later"),
        ])
        .selected(selected.min(1))
        .show_scroll(false)
        .number_shortcuts(true)
        .marker("❯")
        .footer("Enter select · ↑/↓ · 1–2 · Esc exit")
        .indent(2)
        .title_color(TN_FG)
        .subtitle_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_CYAN, SURFACE_SELECTED)
}

impl App {
    pub(super) fn persist_tui_session_state(&self) -> std::io::Result<()> {
        save_tui_session_state(
            Path::new(&self.cwd),
            &self.session_id,
            &TuiSessionState::capture(self),
        )
    }

    pub(super) fn pause_goal_for_exit(&mut self) {
        if let Some(run) = self.goal_run.as_mut() {
            run.pause_for_exit();
        }
    }

    pub(super) fn render_goal_resume_prompt(&self) -> Option<String> {
        let selected = self.goal_resume_prompt?;
        let goal = self.paused_goal.as_ref()?;
        let panel_height = GOAL_RESUME_PANEL_HEIGHT.min(self.height as usize).max(1);
        let panel = goal_resume_panel(goal, selected).view(self.width, panel_height);
        Some(
            Layout::vertical()
                .item(
                    &panel,
                    Constraint::Fixed(panel_height.min(u16::MAX as usize) as u16),
                )
                .item("", Constraint::Fill)
                .render(self.height),
        )
    }

    pub(super) fn handle_goal_resume_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let selected = self.goal_resume_prompt?;
        let goal = self.paused_goal.as_ref()?;
        let mut panel = goal_resume_panel(goal, selected);
        match panel.handle_key(key) {
            Some(MenuPanelMsg::Selected(0)) | Some(MenuPanelMsg::Toggled(0)) => {
                self.resume_paused_goal()
            }
            Some(MenuPanelMsg::Selected(_)) | Some(MenuPanelMsg::Toggled(_)) => {
                self.leave_goal_paused();
                None
            }
            Some(MenuPanelMsg::Cancelled) => self.begin_graceful_quit(),
            None => {
                self.goal_resume_prompt = Some(panel.selected_index().min(1));
                None
            }
        }
    }

    pub(super) fn handle_goal_resume_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let selected = self.goal_resume_prompt?;
        let goal = self.paused_goal.as_ref()?;
        if mouse.row as usize >= GOAL_RESUME_PANEL_HEIGHT.min(self.height as usize) {
            return None;
        }
        let mut panel = goal_resume_panel(goal, selected);
        panel.set_y_offset(0);
        match panel.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(0)) | Some(MenuPanelMsg::Toggled(0)) => {
                self.resume_paused_goal()
            }
            Some(MenuPanelMsg::Selected(_)) | Some(MenuPanelMsg::Toggled(_)) => {
                self.leave_goal_paused();
                None
            }
            Some(MenuPanelMsg::Cancelled) => None,
            None => {
                self.goal_resume_prompt = Some(panel.selected_index().min(1));
                None
            }
        }
    }

    pub(super) fn resume_paused_goal(&mut self) -> Option<Cmd<Msg>> {
        if self.state != State::Idle || self.session_rebuild_pending.is_some() {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  wait for the current session change before resuming the goal"),
            );
            return None;
        }
        let paused = self.paused_goal.clone()?;
        self.goal_generation = self.goal_generation.wrapping_add(1).max(1);
        let generation = self.goal_generation;
        let run = match panels::goal_engineering::GoalRunState::from_paused(
            &self.cwd, generation, &paused,
        ) {
            Ok(run) => run,
            Err(error) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  paused goal could not be restored: {error}")),
                );
                return None;
            }
        };

        self.goal = Some(run.spec.goal.clone());
        self.goal_since = Some(Instant::now());
        self.goal_run = Some(run);
        self.paused_goal = None;
        self.goal_resume_prompt = None;
        // Durable goals use the Ultracode execution budget. This normally
        // matches the restored sidecar already, but also covers an exit while
        // the original goal-start rebuild was still in flight. Mode is left
        // untouched so Default/Plan/Auto remains exactly as restored.
        let mut profile = self.session_rebuild_profile();
        profile.effort = ULTRACODE;
        let command = self.start_session_rebuild(
            profile,
            SessionRebuildAction::GoalResume {
                generation,
                paused: paused.clone(),
            },
        );
        if command.is_none() {
            self.goal_run = None;
            self.goal = None;
            self.goal_since = None;
            self.paused_goal = Some(paused);
            self.goal_resume_prompt = Some(0);
        }
        command
    }

    pub(super) fn leave_goal_paused(&mut self) {
        self.goal_resume_prompt = None;
        if let Some(goal) = self.paused_goal.as_ref() {
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  goal remains paused · {} · /goal resume to continue",
                truncate(&goal.goal, 72)
            )));
        }
    }

    pub(super) fn clear_paused_goal(&mut self, reason: &str) -> bool {
        let Some(paused) = self.paused_goal.take() else {
            return false;
        };
        self.goal_resume_prompt = None;
        panels::goal_engineering::mark_paused_goal_cancelled(&self.cwd, &paused, reason);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_tui::event::{KeyEvent, MouseEvent, MouseEventKind};
    use a3s_tui::style::{strip_ansi, visible_len};

    fn paused_goal() -> PausedGoalState {
        PausedGoalState {
            loop_id: "goal-ship-resume-12345678".to_string(),
            goal: "尽快完成 E2E 兼容和 Python / TypeScript 测试".to_string(),
            iteration: 7,
            progress: 0.45,
            failures: 2,
        }
    }

    #[test]
    fn session_state_round_trips_mode_effort_model_theme_and_goal() {
        let root = tempfile::tempdir().unwrap();
        let state = TuiSessionState {
            schema_version: TUI_SESSION_STATE_SCHEMA_VERSION,
            session_id: "session/with spaces".to_string(),
            model: Some(ModelSelectionPreference {
                source: ModelSelectionSource::Codex,
                model: "gpt-5.5-codex".to_string(),
            }),
            effort: Some(EFFORT_LEVELS[ULTRACODE].id.to_string()),
            mode: Some(PersistedMode::Plan),
            theme: Some(THEMES[2].name.to_string()),
            paused_goal: Some(paused_goal()),
        };

        save_tui_session_state(root.path(), "session/with spaces", &state).unwrap();
        let restored = load_tui_session_state(root.path(), "session/with spaces")
            .unwrap()
            .unwrap();

        assert_eq!(restored.model, state.model);
        assert_eq!(restored.effort_index(), Some(ULTRACODE));
        assert!(matches!(restored.mode(), Mode::Plan));
        assert_eq!(restored.theme_index(), Some(2));
        assert_eq!(restored.paused_goal, state.paused_goal);
        let path = tui_session_state_path(root.path(), "session/with spaces");
        assert!(path.starts_with(root.path()));
        assert!(!path.to_string_lossy().contains("session/with spaces"));
    }

    #[test]
    fn session_state_rejects_a_payload_for_another_session() {
        let root = tempfile::tempdir().unwrap();
        let state = TuiSessionState {
            schema_version: TUI_SESSION_STATE_SCHEMA_VERSION,
            session_id: "session-a".to_string(),
            model: None,
            effort: None,
            mode: Some(PersistedMode::Auto),
            theme: None,
            paused_goal: None,
        };
        let path = tui_session_state_path(root.path(), "session-b");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, serde_json::to_vec(&state).unwrap()).unwrap();

        let error = load_tui_session_state(root.path(), "session-b").unwrap_err();
        assert_eq!(error.kind(), ErrorKind::InvalidData);
        assert!(error.to_string().contains("session-a"));
    }

    #[test]
    fn transient_auto_mode_persists_the_users_underlying_mode() {
        assert!(matches!(
            mode_to_persist(Mode::Auto, Some(Mode::Plan)),
            Mode::Plan
        ));
        assert!(matches!(mode_to_persist(Mode::Auto, None), Mode::Auto));
    }

    #[test]
    fn paused_goal_panel_matches_numbered_resume_choice_contract() {
        let panel = goal_resume_panel(&paused_goal(), 0);
        let rendered = panel.view(96, GOAL_RESUME_PANEL_HEIGHT);
        let plain = rendered.lines().map(strip_ansi).collect::<Vec<_>>();

        assert_eq!(plain.len(), GOAL_RESUME_PANEL_HEIGHT);
        assert!(plain[0].contains("Resume paused goal?"), "{plain:?}");
        assert!(plain[1].contains("Goal: 尽快完成 E2E"), "{plain:?}");
        assert!(plain[2].contains("1. Resume goal"), "{plain:?}");
        assert!(plain[3].contains("2. Leave paused"), "{plain:?}");
        assert!(plain[4].contains("Enter select"), "{plain:?}");
        assert!(rendered.contains(&SURFACE_SELECTED.bg_ansi()));
        assert!(rendered.lines().all(|line| visible_len(line) == 96));
    }

    #[test]
    fn paused_goal_panel_supports_arrows_numbers_escape_and_mouse() {
        let goal = paused_goal();
        let mut panel = goal_resume_panel(&goal, 0);
        assert_eq!(
            panel.handle_key(&KeyEvent {
                code: KeyCode::Down,
                modifiers: KeyModifiers::NONE,
            }),
            None
        );
        assert_eq!(panel.selected_index(), 1);
        assert_eq!(
            panel.handle_key(&KeyEvent {
                code: KeyCode::Enter,
                modifiers: KeyModifiers::NONE,
            }),
            Some(MenuPanelMsg::Selected(1))
        );

        let mut panel = goal_resume_panel(&goal, 1);
        assert_eq!(
            panel.handle_key(&KeyEvent {
                code: KeyCode::Char('1'),
                modifiers: KeyModifiers::NONE,
            }),
            Some(MenuPanelMsg::Selected(0))
        );
        assert_eq!(
            panel.handle_key(&KeyEvent {
                code: KeyCode::Esc,
                modifiers: KeyModifiers::NONE,
            }),
            Some(MenuPanelMsg::Cancelled)
        );

        let mut panel = goal_resume_panel(&goal, 0);
        panel.set_y_offset(0);
        assert_eq!(
            panel.handle_mouse(&MouseEvent {
                kind: MouseEventKind::Down(a3s_tui::event::MouseButton::Left),
                column: 4,
                row: 3,
                modifiers: KeyModifiers::NONE,
            }),
            Some(MenuPanelMsg::Selected(1))
        );
    }
}
