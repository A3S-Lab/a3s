//! Saved-file Code Intelligence commands and result navigation for `/ide`.

use super::super::*;
use super::spf;
use a3s_code_core::workspace::WorkspacePath;
use tokio_util::sync::CancellationToken;

#[path = "code_intelligence/command.rs"]
mod command;
#[path = "code_intelligence/coordinates.rs"]
mod coordinates;
#[path = "code_intelligence/query.rs"]
mod query;
#[path = "code_intelligence/request.rs"]
mod request;
#[cfg(test)]
#[path = "code_intelligence/tests.rs"]
mod tests;

use command::*;
use coordinates::*;
use query::*;
use request::*;

const WORKSPACE_SYMBOL_LIMIT: usize = 200;
const DIRTY_JUMP_MESSAGE: &str =
    "save or close the current unsaved buffer before opening another result";

enum IdeIntelligenceTask {
    Status,
    DocumentSymbols {
        path: WorkspacePath,
    },
    WorkspaceSymbols {
        query: String,
    },
    Navigate {
        kind: NavigationKind,
        path: WorkspacePath,
        row: usize,
        expanded_col: usize,
    },
    Diagnostics {
        path: Option<WorkspacePath>,
    },
}

struct PreparedIdeIntelligenceQuery {
    title: String,
    task: IdeIntelligenceTask,
    saved_version: bool,
    dirty_buffer: bool,
}

impl App {
    /// Intercept Enter for a Code Intelligence `:` command before the editor's
    /// ordinary `:w`/`:q` command handler. The outer option indicates whether
    /// this was a semantic command; the inner option is its asynchronous work.
    pub(crate) fn try_submit_ide_intelligence_prompt(
        &mut self,
        key: &KeyEvent,
    ) -> Option<Option<Cmd<Msg>>> {
        if key.code != KeyCode::Enter {
            return None;
        }
        let ide = self.ide.as_ref()?;
        let command = ide.prompt.as_ref().and_then(|prompt| match prompt {
            IdePrompt::Command(command) => Some(command.clone()),
            IdePrompt::Search { .. } => None,
        })?;
        let parsed = parse_ide_intelligence_command_for_ide(ide, &command)?;
        if let Some(ide) = self.ide.as_mut() {
            ide.prompt = None;
        }
        Some(match parsed {
            Ok(command) => self.begin_ide_intelligence_query(command),
            Err(error) => {
                if let Some(ide) = self.ide.as_mut() {
                    ide.flash = Some(ide_flash_line(ToastKind::Warning, error));
                }
                None
            }
        })
    }

    fn begin_ide_intelligence_query(
        &mut self,
        command: IdeIntelligenceCommand,
    ) -> Option<Cmd<Msg>> {
        let provider = match self.workspace_services.code_intelligence() {
            Some(provider) => provider,
            None => {
                if let Some(ide) = self.ide.as_mut() {
                    ide.flash = Some(ide_flash_line(
                        ToastKind::Warning,
                        "Code Intelligence is unavailable for this workspace",
                    ));
                }
                return None;
            }
        };
        let prepared = match self.prepare_ide_intelligence_query(command) {
            Ok(prepared) => prepared,
            Err(error) => {
                if let Some(ide) = self.ide.as_mut() {
                    ide.flash = Some(ide_flash_line(ToastKind::Warning, error));
                }
                return None;
            }
        };
        let file_system = self.workspace_services.fs();
        let ide = self.ide.as_mut()?;
        let (request_id, cancellation) = replace_ide_intelligence_request(ide);
        ide.flash = None;
        ide.intelligence = Some(IdeIntelligenceView::loading(
            request_id,
            prepared.title.clone(),
            prepared.saved_version,
            prepared.dirty_buffer,
        ));

        Some(cmd::cmd(move || async move {
            let result =
                execute_ide_intelligence_query(provider, file_system, prepared, cancellation).await;
            Msg::IdeIntelligenceCompleted { request_id, result }
        }))
    }

    fn prepare_ide_intelligence_query(
        &self,
        command: IdeIntelligenceCommand,
    ) -> Result<PreparedIdeIntelligenceQuery, String> {
        let dirty_buffer = self
            .ide
            .as_ref()
            .and_then(|ide| ide.file.as_ref())
            .is_some_and(|file| file.dirty);
        match command {
            IdeIntelligenceCommand::Status => Ok(PreparedIdeIntelligenceQuery {
                title: "Code Intelligence status".to_owned(),
                task: IdeIntelligenceTask::Status,
                saved_version: false,
                dirty_buffer: false,
            }),
            IdeIntelligenceCommand::Symbols { query: Some(query) } => {
                Ok(PreparedIdeIntelligenceQuery {
                    title: format!("Workspace symbols · {query}"),
                    task: IdeIntelligenceTask::WorkspaceSymbols { query },
                    saved_version: false,
                    dirty_buffer,
                })
            }
            IdeIntelligenceCommand::Symbols { query: None } => {
                let (path, _, _, dirty) = self.open_ide_document()?;
                Ok(PreparedIdeIntelligenceQuery {
                    title: format!("Document symbols · {}", path.as_str()),
                    task: IdeIntelligenceTask::DocumentSymbols { path },
                    saved_version: true,
                    dirty_buffer: dirty,
                })
            }
            IdeIntelligenceCommand::Navigate(kind) => {
                let (path, row, expanded_col, dirty) = self.open_ide_document()?;
                Ok(PreparedIdeIntelligenceQuery {
                    title: format!("{} · {}", navigation_label(kind), path.as_str()),
                    task: IdeIntelligenceTask::Navigate {
                        kind,
                        path,
                        row,
                        expanded_col,
                    },
                    saved_version: true,
                    dirty_buffer: dirty,
                })
            }
            IdeIntelligenceCommand::Diagnostics { workspace: true } => {
                Ok(PreparedIdeIntelligenceQuery {
                    title: "Workspace diagnostics".to_owned(),
                    task: IdeIntelligenceTask::Diagnostics { path: None },
                    saved_version: false,
                    dirty_buffer,
                })
            }
            IdeIntelligenceCommand::Diagnostics { workspace: false } => {
                let (path, _, _, dirty) = self.open_ide_document()?;
                Ok(PreparedIdeIntelligenceQuery {
                    title: format!("Document diagnostics · {}", path.as_str()),
                    task: IdeIntelligenceTask::Diagnostics { path: Some(path) },
                    saved_version: true,
                    dirty_buffer: dirty,
                })
            }
        }
    }

    fn open_ide_document(&self) -> Result<(WorkspacePath, usize, usize, bool), String> {
        let file = self
            .ide
            .as_ref()
            .and_then(|ide| ide.file.as_ref())
            .ok_or_else(|| "open a source file first".to_owned())?;
        if file.image {
            return Err("Code Intelligence is unavailable for image previews".to_owned());
        }
        let relative = file
            .path
            .strip_prefix(Path::new(&self.cwd))
            .map_err(|_| "the open file is outside the workspace".to_owned())?;
        let relative = relative.to_string_lossy().replace('\\', "/");
        let workspace_path = self
            .workspace_services
            .normalize_path(&relative)
            .map_err(|error| format!("invalid workspace path: {error}"))?;
        Ok((workspace_path, file.row, file.col, file.dirty))
    }

    /// Result lists are modal within `/ide`: navigation cannot leak into the
    /// underlying editor, and Enter opens the selected saved-file location.
    pub(crate) fn handle_ide_intelligence_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        let ide = self.ide.as_mut()?;
        let view = ide.intelligence.as_mut()?;
        match key.code {
            KeyCode::Esc => {
                ide.intelligence_cancellation.cancel();
                ide.intelligence_jump_cancellation.cancel();
                ide.intelligence = None;
                ide.flash = None;
                return None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                view.selected = view.selected.saturating_sub(1);
            }
            KeyCode::Down | KeyCode::Char('j') => {
                view.selected = (view.selected + 1).min(view.rows.len().saturating_sub(1));
            }
            KeyCode::PageUp => {
                let page = (self.height as usize).saturating_sub(7).max(1);
                view.selected = view.selected.saturating_sub(page);
            }
            KeyCode::PageDown => {
                let page = (self.height as usize).saturating_sub(7).max(1);
                view.selected = (view.selected + page).min(view.rows.len().saturating_sub(1));
            }
            KeyCode::Home | KeyCode::Char('g') => view.selected = 0,
            KeyCode::End | KeyCode::Char('G') => view.selected = view.rows.len().saturating_sub(1),
            KeyCode::Enter => return self.begin_ide_intelligence_jump(),
            _ => {}
        }
        let body = (self.height as usize).saturating_sub(5).max(1);
        if view.selected < view.scroll {
            view.scroll = view.selected;
        } else if view.selected >= view.scroll + body {
            view.scroll = view.selected + 1 - body;
        }
        None
    }

    fn begin_ide_intelligence_jump(&mut self) -> Option<Cmd<Msg>> {
        let (target, request_id) = {
            let view = self.ide.as_ref()?.intelligence.as_ref()?;
            (
                view.rows.get(view.selected)?.target.clone()?,
                view.request_id,
            )
        };
        // Supersede the previous jump as soon as a new target is submitted.
        // Even if validation rejects this target, an older read must not open
        // after the user's newer Enter action.
        let (jump_request_id, cancellation) = {
            let ide = self.ide.as_mut()?;
            replace_ide_intelligence_jump_request(ide)
        };
        let workspace_path = match self.workspace_services.normalize_path(&target.path) {
            Ok(path) => path,
            Err(error) => {
                if let Some(ide) = self.ide.as_mut() {
                    ide.flash = Some(ide_flash_line(
                        ToastKind::Error,
                        format!("Code Intelligence returned an invalid workspace path: {error}"),
                    ));
                }
                return None;
            }
        };
        let display_path = Path::new(&self.cwd).join(workspace_path.as_str());
        let file_system = self.workspace_services.fs();
        let ide = self.ide.as_mut()?;
        if let Err(error) = validate_ide_intelligence_jump_target(ide, &display_path) {
            ide.flash = Some(ide_flash_line(ToastKind::Warning, error));
            return None;
        }
        ide.flash = Some(ide_flash_line(
            ToastKind::Info,
            "opening the saved Code Intelligence result…",
        ));
        Some(cmd::cmd(move || async move {
            let result = read_ide_intelligence_jump(
                file_system,
                workspace_path,
                display_path,
                target.position,
                cancellation,
            )
            .await;
            Msg::IdeIntelligenceJumpCompleted {
                request_id,
                jump_request_id,
                result,
            }
        }))
    }

    pub(crate) fn apply_ide_intelligence_result(
        &mut self,
        request_id: u64,
        result: Result<IdeIntelligenceResult, String>,
    ) {
        let Some(ide) = self.ide.as_mut() else {
            return;
        };
        apply_ide_intelligence_result_to_ide(ide, request_id, result);
    }

    pub(crate) fn apply_ide_intelligence_jump(
        &mut self,
        request_id: u64,
        jump_request_id: u64,
        result: Result<IdeIntelligenceJump, String>,
    ) {
        let body = (self.height as usize).saturating_sub(5);
        let Some(ide) = self.ide.as_mut() else {
            return;
        };
        if !ide_intelligence_jump_request_is_current(ide, request_id, jump_request_id) {
            return;
        }
        let jump = match result {
            Ok(jump) => jump,
            Err(error) => {
                ide.flash = Some(ide_flash_line(ToastKind::Error, error));
                return;
            }
        };
        let jump_path = jump.path.clone();
        let preserve_dirty = install_ide_intelligence_jump(ide, jump, body);
        touch_workspace_file_path_for_manifest(&self.workspace_manifest, &self.cwd, &jump_path);
        ide.focus_editor = true;
        ide.preview = None;
        ide.intelligence = None;
        ide.flash = Some(ide_flash_line(
            if preserve_dirty {
                ToastKind::Warning
            } else {
                ToastKind::Success
            },
            if preserve_dirty {
                "jumped using the saved version; unsaved edits were ignored"
            } else {
                "opened saved Code Intelligence result"
            },
        ));
    }
}

fn parse_ide_intelligence_command_for_ide(
    ide: &Ide,
    command: &str,
) -> Option<Result<IdeIntelligenceCommand, String>> {
    ide.supports_code_intelligence()
        .then(|| parse_ide_intelligence_command(command))
        .flatten()
}

fn apply_ide_intelligence_result_to_ide(
    ide: &mut Ide,
    request_id: u64,
    result: Result<IdeIntelligenceResult, String>,
) -> bool {
    if !ide_intelligence_request_is_current(ide, request_id) {
        return false;
    }
    let Some(view) = ide.intelligence.as_mut() else {
        return false;
    };
    match result {
        Ok(result) => {
            view.title = result.title;
            view.rows = if result.rows.is_empty() {
                vec![IdeIntelligenceRow {
                    text: "No results from the saved workspace state.".to_owned(),
                    target: None,
                }]
            } else {
                result.rows
            };
            view.selected = 0;
            view.scroll = 0;
            view.truncated = result.truncated;
            view.saved_version = result.saved_version;
            view.dirty_buffer = result.dirty_buffer;
            view.stale = result.stale;
            view.workspace_revision = result.workspace_revision;
            ide.flash = None;
        }
        Err(error) => {
            view.title = "Code Intelligence error".to_owned();
            view.rows = vec![IdeIntelligenceRow {
                text: error,
                target: None,
            }];
            view.selected = 0;
            view.scroll = 0;
            ide.flash = None;
        }
    }
    true
}

fn validate_ide_intelligence_jump_target(ide: &Ide, target: &Path) -> Result<(), &'static str> {
    if ide
        .file
        .as_ref()
        .is_some_and(|file| file.dirty && file.path != target)
    {
        Err(DIRTY_JUMP_MESSAGE)
    } else {
        Ok(())
    }
}

fn install_ide_intelligence_jump(ide: &mut Ide, jump: IdeIntelligenceJump, body: usize) -> bool {
    let preserve_dirty = ide
        .file
        .as_ref()
        .is_some_and(|file| file.dirty && file.path == jump.path);
    if preserve_dirty {
        if let Some(file) = ide.file.as_mut() {
            file.row = jump.row.min(file.lines.len().saturating_sub(1));
            file.col = jump.col.min(
                file.lines
                    .get(file.row)
                    .map_or(0, |line| line.chars().count()),
            );
            file.scroll = file.row.saturating_sub(body / 2);
        }
    } else {
        let mut file = IdeFile::new(jump.path, jump.lines, false, false);
        file.row = jump.row.min(file.lines.len().saturating_sub(1));
        file.col = jump.col.min(
            file.lines
                .get(file.row)
                .map_or(0, |line| line.chars().count()),
        );
        file.scroll = file.row.saturating_sub(body / 2);
        ide.file = Some(file);
    }
    preserve_dirty
}

pub(super) fn ide_intelligence_panel(
    ide: &Ide,
    body: usize,
    width: usize,
) -> Option<(String, Vec<String>)> {
    let view = ide.intelligence.as_ref()?;
    let mut title = format!("⌁ {}", view.title);
    if view.dirty_buffer {
        title.push_str(" · SAVED VERSION");
    }
    if view.truncated {
        title.push_str(" · truncated");
    }
    if view.stale {
        title.push_str(" · stale");
    }
    if let Some(revision) = view.workspace_revision {
        title.push_str(&format!(" · rev {revision}"));
    }
    let mut rows = Vec::with_capacity(body);
    for visible in 0..body {
        let index = view.scroll + visible;
        let Some(row) = view.rows.get(index) else {
            rows.push(String::new());
            continue;
        };
        let marker = if index == view.selected { "› " } else { "  " };
        let raw = spf::fit(&format!("{marker}{}", row.text), width);
        rows.push(if index == view.selected {
            Style::new().fg(TN_FG).bg(SURFACE_SELECTED).render(&raw)
        } else if row.target.is_some() {
            Style::new().fg(TN_FG).render(&raw)
        } else {
            Style::new().fg(TN_GRAY).render(&raw)
        });
    }
    Some((title, rows))
}

pub(super) fn ide_intelligence_notice(view: &IdeIntelligenceView) -> String {
    if view.dirty_buffer {
        "UNSAVED EDITS IGNORED · results and jumps use the saved version".to_owned()
    } else if view.saved_version {
        "Saved version · ↑↓ select · Enter jump · Esc close".to_owned()
    } else {
        "Saved workspace files only · ↑↓ select · Enter jump · Esc close".to_owned()
    }
}

pub(super) const fn ide_intelligence_command_hint() -> &'static str {
    "Code Intelligence :status/:symbols/:definition/…/:diagnostics"
}
