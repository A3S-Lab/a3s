//! `/permissions`: inspect and revoke exact session and project grants.

use super::super::*;
use a3s_tui::components::{MenuItem, MenuPanel, MenuPanelMsg};
use a3s_tui::event::{MouseEvent, MouseEventKind};

const PERMISSION_PANEL_MAX_VISIBLE_ROWS: usize = 12;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PermissionGrantScope {
    Session,
    Project,
}

impl PermissionGrantScope {
    fn label(self) -> &'static str {
        match self {
            Self::Session => "session",
            Self::Project => "project",
        }
    }

    fn prefix(self) -> &'static str {
        match self {
            Self::Session => "S",
            Self::Project => "P",
        }
    }

    fn description(self) -> &'static str {
        match self {
            Self::Session => "session · expires when this TUI exits",
            Self::Project => "project · .a3s/permissions.acl",
        }
    }

    fn color(self) -> Color {
        match self {
            Self::Session => TN_CYAN,
            Self::Project => TN_PURPLE,
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
struct PermissionGrantRow {
    scope: PermissionGrantScope,
    grant: ExactPermissionGrant,
}

impl PermissionGrantRow {
    fn identity(&self) -> String {
        permission_grant_identity(self.scope, &self.grant.stable_key())
    }

    fn matches_query(&self, query: &str) -> bool {
        let query = query.trim();
        if query.is_empty() {
            return true;
        }
        let haystack = format!(
            "{} {} {} {}",
            self.scope.label(),
            self.grant.tool_name(),
            self.grant.scope_label(),
            self.grant.args()
        )
        .to_lowercase();
        query
            .split_whitespace()
            .all(|term| haystack.contains(&term.to_lowercase()))
    }
}

pub(crate) struct PermissionPanel {
    grants: Vec<PermissionGrantRow>,
    selected_identity: Option<String>,
    selected_hint: usize,
    query: String,
    searching: bool,
    revoke_armed: Option<String>,
    revoke_inflight: Option<String>,
    feedback: Option<String>,
    error: Option<String>,
}

impl PermissionPanel {
    fn new(snapshot: PermissionGrantSnapshot) -> Self {
        let grants = permission_grant_rows(snapshot);
        let selected_identity = grants.first().map(PermissionGrantRow::identity);
        Self {
            grants,
            selected_identity,
            selected_hint: 0,
            query: String::new(),
            searching: false,
            revoke_armed: None,
            revoke_inflight: None,
            feedback: None,
            error: None,
        }
    }

    fn visible_indices(&self) -> Vec<usize> {
        self.grants
            .iter()
            .enumerate()
            .filter_map(|(index, grant)| grant.matches_query(&self.query).then_some(index))
            .collect()
    }

    fn selected_index(&self) -> usize {
        let indices = self.visible_indices();
        self.selected_identity
            .as_deref()
            .and_then(|selected| {
                indices.iter().position(|index| {
                    self.grants
                        .get(*index)
                        .is_some_and(|grant| grant.identity() == selected)
                })
            })
            .unwrap_or_else(|| self.selected_hint.min(indices.len().saturating_sub(1)))
    }

    fn selected_grant(&self) -> Option<&PermissionGrantRow> {
        let indices = self.visible_indices();
        indices
            .get(self.selected_index())
            .and_then(|index| self.grants.get(*index))
    }

    fn remember_visible_index(&mut self, visible_index: usize) {
        let identity = self
            .visible_indices()
            .get(visible_index)
            .and_then(|index| self.grants.get(*index))
            .map(PermissionGrantRow::identity);
        if let Some(identity) = identity {
            self.selected_identity = Some(identity);
            self.selected_hint = visible_index;
        }
    }

    fn reconcile_selection(&mut self) {
        let indices = self.visible_indices();
        if indices.is_empty() {
            self.selected_identity = None;
            self.selected_hint = 0;
            self.revoke_armed = None;
            return;
        }
        let selected_is_visible = self.selected_identity.as_deref().is_some_and(|selected| {
            indices.iter().any(|index| {
                self.grants
                    .get(*index)
                    .is_some_and(|grant| grant.identity() == selected)
            })
        });
        if !selected_is_visible {
            self.remember_visible_index(self.selected_hint.min(indices.len() - 1));
        }
        if self
            .revoke_armed
            .as_deref()
            .is_some_and(|armed| !self.grants.iter().any(|grant| grant.identity() == armed))
        {
            self.revoke_armed = None;
        }
    }

    fn sync_snapshot(&mut self, snapshot: PermissionGrantSnapshot) {
        self.grants = permission_grant_rows(snapshot);
        self.reconcile_selection();
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
        self.revoke_armed = None;
    }

    fn move_selection_to(&mut self, index: usize) {
        let last = self.visible_indices().len().saturating_sub(1);
        self.remember_visible_index(index.min(last));
        self.revoke_armed = None;
    }

    fn select_visible_index(&mut self, index: usize) {
        self.remember_visible_index(index);
        self.revoke_armed = None;
    }

    fn reset_filter_selection(&mut self) {
        self.selected_hint = 0;
        self.selected_identity = None;
        self.revoke_armed = None;
        self.reconcile_selection();
    }

    fn arm_or_revoke_selected(&mut self) -> PermissionPanelAction {
        let Some(selected) = self.selected_grant().cloned() else {
            return PermissionPanelAction::None;
        };
        let identity = selected.identity();
        if self.revoke_inflight.as_deref() == Some(identity.as_str()) {
            self.error = Some("This project grant is already being revoked.".to_string());
            return PermissionPanelAction::None;
        }
        self.error = None;
        self.feedback = None;
        if self.revoke_armed.as_deref() == Some(identity.as_str()) {
            self.revoke_armed = None;
            PermissionPanelAction::Revoke(selected)
        } else {
            self.revoke_armed = Some(identity);
            PermissionPanelAction::None
        }
    }

    fn handle_search_key(&mut self, key: &KeyEvent) -> PermissionPanelAction {
        match key.code {
            KeyCode::Esc => self.searching = false,
            KeyCode::Enter => {
                return self
                    .selected_grant()
                    .cloned()
                    .map_or(PermissionPanelAction::None, PermissionPanelAction::Open);
            }
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down => self.move_selection(1),
            KeyCode::PageUp => {
                self.move_selection(-(PERMISSION_PANEL_MAX_VISIBLE_ROWS as isize));
            }
            KeyCode::PageDown => {
                self.move_selection(PERMISSION_PANEL_MAX_VISIBLE_ROWS as isize);
            }
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Backspace => {
                self.query.pop();
                self.reset_filter_selection();
            }
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.query.clear();
                self.reset_filter_selection();
            }
            KeyCode::Char(character)
                if !character.is_control()
                    && !key
                        .modifiers
                        .intersects(KeyModifiers::CONTROL | KeyModifiers::ALT) =>
            {
                self.query.push(character);
                self.reset_filter_selection();
            }
            _ => {}
        }
        PermissionPanelAction::None
    }

    fn handle_key(&mut self, key: &KeyEvent) -> PermissionPanelAction {
        if self.searching {
            return self.handle_search_key(key);
        }
        if key.code == KeyCode::Esc && self.revoke_armed.take().is_some() {
            return PermissionPanelAction::None;
        }
        match key.code {
            KeyCode::Up => self.move_selection(-1),
            KeyCode::Down | KeyCode::Tab => self.move_selection(1),
            KeyCode::BackTab => self.move_selection(-1),
            KeyCode::PageUp => {
                self.move_selection(-(PERMISSION_PANEL_MAX_VISIBLE_ROWS as isize));
            }
            KeyCode::PageDown => {
                self.move_selection(PERMISSION_PANEL_MAX_VISIBLE_ROWS as isize);
            }
            KeyCode::Home => self.move_selection_to(0),
            KeyCode::End => self.move_selection_to(usize::MAX),
            KeyCode::Char('/') => self.searching = true,
            KeyCode::Char('c' | 'C') if !self.query.is_empty() => {
                self.query.clear();
                self.reset_filter_selection();
            }
            KeyCode::Char('x' | 'X') | KeyCode::Delete => {
                return self.arm_or_revoke_selected();
            }
            KeyCode::Enter => {
                self.revoke_armed = None;
                return self
                    .selected_grant()
                    .cloned()
                    .map_or(PermissionPanelAction::None, PermissionPanelAction::Open);
            }
            KeyCode::Esc => return PermissionPanelAction::Close,
            _ => {
                self.revoke_armed = None;
            }
        }
        PermissionPanelAction::None
    }

    fn mark_project_revoke_started(&mut self, stable_key: &str) {
        self.revoke_armed = None;
        self.revoke_inflight = Some(permission_grant_identity(
            PermissionGrantScope::Project,
            stable_key,
        ));
        self.feedback = None;
        self.error = None;
    }

    fn mark_project_revoke_inflight(&mut self, stable_key: &str) {
        self.mark_project_revoke_started(stable_key);
        self.feedback = Some(
            "A project grant revocation is still running; other project rule changes wait."
                .to_string(),
        );
    }

    fn finish_project_revoke(
        &mut self,
        snapshot: PermissionGrantSnapshot,
        stable_key: &str,
        removed: bool,
    ) {
        self.revoke_inflight = None;
        self.error = None;
        self.feedback = Some(if removed {
            "Project grant revoked. Future checks use the updated rules; running tools continue."
                .to_string()
        } else {
            "The project grant was already absent; in-memory grants were synchronized.".to_string()
        });
        let removed_identity = permission_grant_identity(PermissionGrantScope::Project, stable_key);
        if self.selected_identity.as_deref() == Some(removed_identity.as_str()) {
            self.selected_identity = None;
        }
        self.sync_snapshot(snapshot);
    }

    fn fail_project_revoke(&mut self, stable_key: &str, error: &str) {
        let identity = permission_grant_identity(PermissionGrantScope::Project, stable_key);
        if self.revoke_inflight.as_deref() == Some(identity.as_str()) {
            self.revoke_inflight = None;
        }
        self.error = Some(format!("Project grant was not revoked: {error}"));
    }

    fn set_feedback(&mut self, message: impl Into<String>) {
        self.feedback = Some(message.into());
        self.error = None;
    }

    fn set_error(&mut self, message: impl Into<String>) {
        self.error = Some(message.into());
        self.feedback = None;
    }
}

enum PermissionPanelAction {
    None,
    Revoke(PermissionGrantRow),
    Open(PermissionGrantRow),
    Close,
}

fn permission_grant_identity(scope: PermissionGrantScope, stable_key: &str) -> String {
    format!("{}\u{0}{stable_key}", scope.label())
}

fn permission_grant_rows(snapshot: PermissionGrantSnapshot) -> Vec<PermissionGrantRow> {
    snapshot
        .session
        .into_iter()
        .map(|grant| PermissionGrantRow {
            scope: PermissionGrantScope::Session,
            grant,
        })
        .chain(
            snapshot
                .project
                .into_iter()
                .map(|grant| PermissionGrantRow {
                    scope: PermissionGrantScope::Project,
                    grant,
                }),
        )
        .collect()
}

fn permission_panel_footer(panel: &PermissionPanel) -> String {
    if let Some(identity) = panel.revoke_inflight.as_deref() {
        let label = panel
            .grants
            .iter()
            .find(|grant| grant.identity() == identity)
            .map(|grant| grant.grant.scope_label())
            .unwrap_or_else(|| "project grant".to_string());
        return format!(
            "Revoking {}… · running tools are not stopped",
            truncate(&label, 48)
        );
    }
    if let Some(identity) = panel.revoke_armed.as_deref() {
        let label = panel
            .grants
            .iter()
            .find(|grant| grant.identity() == identity)
            .map(|grant| grant.grant.scope_label())
            .unwrap_or_else(|| "selected grant".to_string());
        return format!(
            "Press X again to revoke {} · Esc disarm · future checks only",
            truncate(&label, 40)
        );
    }
    if let Some(error) = panel.error.as_deref() {
        return error.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    if let Some(feedback) = panel.feedback.as_deref() {
        return feedback.split_whitespace().collect::<Vec<_>>().join(" ");
    }
    let session = panel
        .grants
        .iter()
        .filter(|grant| grant.scope == PermissionGrantScope::Session)
        .count();
    let project = panel.grants.len().saturating_sub(session);
    format!("{session} session · {project} project · revocation affects future checks only")
}

fn permission_menu_panel(panel: &PermissionPanel, max_items: usize) -> MenuPanel {
    let indices = panel.visible_indices();
    let mut items = indices
        .iter()
        .filter_map(|index| panel.grants.get(*index))
        .map(|row| {
            MenuItem::new(row.grant.scope_label())
                .prefix(row.scope.prefix())
                .description(row.scope.description())
                .color(row.scope.color())
        })
        .collect::<Vec<_>>();
    if items.is_empty() {
        let label = if panel.query.trim().is_empty() {
            "(no remembered permission grants)"
        } else {
            "(no permission grants match this filter)"
        };
        items.push(MenuItem::new(label).disabled(true));
    }
    let subtitle = if panel.searching {
        format!(
            "Filter: {}▌ · type to refine · ↑/↓ select · Enter details · Esc done · Ctrl+U clear",
            panel.query
        )
    } else if panel.query.is_empty() {
        "S session · P project · / search · Enter details · X twice revoke · Esc close".to_string()
    } else {
        format!(
            "Filter: {} · / edit · C clear · Enter details · X twice revoke",
            panel.query
        )
    };

    MenuPanel::new("Permission grants")
        .subtitle(subtitle)
        .items(items)
        .selected(panel.selected_index())
        .max_items(max_items)
        .footer(permission_panel_footer(panel))
        .indent(2)
        .marker("›")
        .title_color(ACCENT)
        .subtitle_color(TN_GRAY)
        .text_color(TN_FG)
        .muted_color(TN_GRAY)
        .selected_colors(TN_FG, SURFACE_SELECTED)
        .disabled_color(TN_GRAY)
}

fn permission_panel_max_rows(height: usize) -> usize {
    height
        .saturating_sub(9)
        .clamp(3, PERMISSION_PANEL_MAX_VISIBLE_ROWS)
}

fn permission_panel_height(panel: &PermissionPanel, max_items: usize) -> usize {
    let item_count = panel.visible_indices().len().max(1).min(max_items);
    4 + item_count
}

fn permission_menu_lines(panel: &PermissionPanel, width: usize, max_items: usize) -> Vec<String> {
    permission_menu_panel(panel, max_items)
        .view(
            width.min(u16::MAX as usize) as u16,
            permission_panel_height(panel, max_items),
        )
        .lines()
        .map(str::to_string)
        .collect()
}

fn permission_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

fn permission_details_title(row: &PermissionGrantRow) -> String {
    let tool = row
        .grant
        .tool_name()
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .take(40)
        .collect::<String>();
    format!(
        "permission-{}-{}.txt",
        row.scope.label(),
        if tool.is_empty() { "tool" } else { &tool }
    )
}

fn permission_details_document(row: &PermissionGrantRow) -> String {
    let arguments = serde_json::to_string_pretty(row.grant.args())
        .unwrap_or_else(|_| row.grant.args().to_string());
    format!(
        "Permission grant\n\
         \n\
         Scope: {}\n\
         Tool: {}\n\
         Match: exact canonical arguments\n\
         \n\
         Arguments:\n\
         {}\n\
         \n\
         Revocation affects future permission checks only. It does not cancel a tool that is already running.\n",
        row.scope.label(),
        row.grant.tool_name(),
        arguments,
    )
}

impl App {
    pub(crate) fn open_permission_panel(&mut self) {
        let mut panel = PermissionPanel::new(self.permission_grants.snapshot());
        if let Some((_, grant)) = self.project_permission_revoke_inflight.as_ref() {
            panel.mark_project_revoke_inflight(&grant.stable_key());
        }
        self.permission_panel = Some(panel);
    }

    pub(crate) fn refresh_permission_panel_grants(&mut self) {
        let snapshot = self.permission_grants.snapshot();
        if let Some(panel) = self.permission_panel.as_mut() {
            panel.sync_snapshot(snapshot);
        }
    }

    fn revoke_permission_from_panel(&mut self, row: PermissionGrantRow) -> Option<Cmd<Msg>> {
        let stable_key = row.grant.stable_key();
        match row.scope {
            PermissionGrantScope::Session => {
                let removed = self.permission_grants.revoke_session(&stable_key);
                let snapshot = self.permission_grants.snapshot();
                if let Some(panel) = self.permission_panel.as_mut() {
                    panel.sync_snapshot(snapshot);
                    if removed {
                        panel.set_feedback(
                            "Session grant revoked. Future checks will ask again; running tools continue.",
                        );
                    } else {
                        panel.set_error("The selected session grant was already absent.");
                    }
                }
                if removed {
                    self.push_notice(
                        NoticeKind::Info,
                        format!(
                            "Session permission revoked · {} · future checks only",
                            row.grant.scope_label()
                        ),
                    );
                }
                None
            }
            PermissionGrantScope::Project => {
                if self.permission_rule_write_inflight.is_some() {
                    if let Some(panel) = self.permission_panel.as_mut() {
                        panel.set_error(
                            "A project permission is being saved; wait before revoking a project grant.",
                        );
                    }
                    return None;
                }
                if self.project_permission_revoke_inflight.is_some() {
                    if let Some(panel) = self.permission_panel.as_mut() {
                        panel.set_error(
                            "Another project grant revocation is still running; wait for it to finish.",
                        );
                    }
                    return None;
                }

                self.project_permission_revoke_seq =
                    self.project_permission_revoke_seq.wrapping_add(1).max(1);
                let request_id = self.project_permission_revoke_seq;
                self.project_permission_revoke_inflight = Some((request_id, row.grant.clone()));
                if let Some(panel) = self.permission_panel.as_mut() {
                    panel.mark_project_revoke_started(&stable_key);
                }
                self.permission_grants.revoke_project(&stable_key);
                self.refresh_permission_panel_grants();
                let path = self.project_permission_rules_path.clone();
                Some(cmd::cmd(move || async move {
                    let key = stable_key.clone();
                    let result = tokio::task::spawn_blocking(move || {
                        revoke_project_permission_grant(&path, &key)
                    })
                    .await
                    .map_err(|error| format!("permission rule revoker failed: {error}"))
                    .and_then(|result| result);
                    Msg::ProjectPermissionRevoked {
                        request_id,
                        stable_key,
                        result,
                    }
                }))
            }
        }
    }

    pub(crate) fn apply_project_permission_revoke_result(
        &mut self,
        request_id: u64,
        stable_key: String,
        result: Result<ProjectPermissionRevocation, String>,
    ) {
        let Some((active_request_id, active_grant)) =
            self.project_permission_revoke_inflight.as_ref()
        else {
            return;
        };
        if *active_request_id != request_id || active_grant.stable_key() != stable_key {
            return;
        }
        let active_grant = active_grant.clone();
        self.project_permission_revoke_inflight = None;

        match result {
            Ok(revocation) => {
                self.permission_grants
                    .replace_project(revocation.grants.clone());
                let snapshot = self.permission_grants.snapshot();
                if let Some(panel) = self.permission_panel.as_mut() {
                    panel.finish_project_revoke(snapshot, &stable_key, revocation.removed);
                }
                if revocation.removed {
                    self.push_notice(
                        NoticeKind::Info,
                        format!(
                            "Project permission revoked from {} · future checks only",
                            revocation.path.display()
                        ),
                    );
                } else {
                    self.push_notice(
                        NoticeKind::Info,
                        "Project permission was already absent; active grants were synchronized",
                    );
                }
            }
            Err(error) => {
                self.permission_grants.allow_for_project(active_grant);
                self.refresh_permission_panel_grants();
                if let Some(panel) = self.permission_panel.as_mut() {
                    panel.fail_project_revoke(&stable_key, &error);
                }
                self.push_notice(
                    NoticeKind::Warning,
                    format!("Project permission was not revoked: {error}"),
                );
            }
        }
    }

    pub(crate) fn handle_permission_panel_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let action = self.permission_panel.as_mut()?.handle_key(key);
        match action {
            PermissionPanelAction::None => None,
            PermissionPanelAction::Revoke(row) => self.revoke_permission_from_panel(row),
            PermissionPanelAction::Open(row) => {
                self.permission_panel = None;
                self.open_readonly_in_ide(
                    &permission_details_title(&row),
                    &permission_details_document(&row),
                );
                None
            }
            PermissionPanelAction::Close => {
                self.permission_panel = None;
                None
            }
        }
    }

    pub(crate) fn handle_permission_panel_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        match mouse.kind {
            MouseEventKind::ScrollUp => {
                self.permission_panel.as_mut()?.move_selection(-1);
                return None;
            }
            MouseEventKind::ScrollDown => {
                self.permission_panel.as_mut()?.move_selection(1);
                return None;
            }
            _ => {}
        }
        let panel = self.permission_panel.as_ref()?;
        let max_items = permission_panel_max_rows(self.height as usize);
        let height = permission_panel_height(panel, max_items);
        let mut menu = permission_menu_panel(panel, max_items);
        let row_count = menu.view(self.width, height).lines().count();
        if row_count == 0 {
            return None;
        }
        menu.set_y_offset(permission_overlay_y_offset(
            self.height as usize,
            row_count,
            self.overlay_rows_below(),
        ));
        match menu.handle_mouse(mouse) {
            Some(MenuPanelMsg::Selected(index)) | Some(MenuPanelMsg::Toggled(index)) => {
                if let Some(panel) = self.permission_panel.as_mut() {
                    panel.select_visible_index(index);
                }
                None
            }
            Some(MenuPanelMsg::Cancelled) | None => None,
        }
    }

    pub(crate) fn overlay_permission_menu(&self, composed: String) -> String {
        let Some(panel) = self.permission_panel.as_ref() else {
            return composed;
        };
        let menu = permission_menu_lines(
            panel,
            self.width as usize,
            permission_panel_max_rows(self.height as usize),
        );
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
#[path = "permissions_tests.rs"]
mod tests;
