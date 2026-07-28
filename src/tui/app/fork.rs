//! Session and Git-worktree fork orchestration.

use super::*;
use a3s_code_core::store::SessionStore as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum ForkCommand {
    Session,
    Worktree,
}

pub(super) fn parse_fork_command(rest: &str) -> Result<ForkCommand, &'static str> {
    match rest.trim() {
        "" | "session" => Ok(ForkCommand::Session),
        "worktree" => Ok(ForkCommand::Worktree),
        _ => Err("usage: /fork [session|worktree]"),
    }
}

impl App {
    pub(super) fn submit_fork_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        let command = match parse_fork_command(rest) {
            Ok(command) => command,
            Err(usage) => {
                self.textarea.clear();
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {usage}")));
                return None;
            }
        };
        match command {
            ForkCommand::Session => self.submit_session_fork(),
            ForkCommand::Worktree => self.submit_worktree_fork(),
        }
    }

    fn submit_session_fork(&mut self) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let store = self.store.clone();
        let source_id = self.session_id.clone();
        let destination_id = new_session_id();
        let workspace = self.cwd.clone();
        let sidecar = TuiSessionState::capture_for_session(self, destination_id.clone());
        let request_id = self.reserve_fork_request();

        Some(cmd::cmd(move || async move {
            let result = async {
                let snapshot = store
                    .load_snapshot(&source_id)
                    .await
                    .map_err(|error| format!("could not read the session: {error}"))?
                    .ok_or_else(|| {
                        "nothing to fork yet — start a conversation first".to_string()
                    })?;
                let snapshot = snapshot
                    .fork_for_session(destination_id.clone(), workspace.clone())
                    .map_err(|error| format!("could not prepare the fork: {error:#}"))?;
                store
                    .save_snapshot(&snapshot)
                    .await
                    .map_err(|error| format!("could not save the fork: {error}"))?;
                save_tui_session_state(Path::new(&workspace), &destination_id, &sidecar)
                    .map_err(|error| format!("could not save forked TUI state: {error}"))?;
                Ok(destination_id)
            }
            .await;
            Msg::Forked { request_id, result }
        }))
    }

    fn submit_worktree_fork(&mut self) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let store = self.store.clone();
        let source_id = self.session_id.clone();
        let destination_id = new_session_id();
        let identity: String = destination_id.chars().take(12).collect();
        let workspace = PathBuf::from(&self.cwd);
        let sidecar = TuiSessionState::capture_for_session(self, destination_id.clone());
        let request_id = self.reserve_fork_request();
        self.push_line(
            &Style::new()
                .fg(TN_GRAY)
                .render("  creating an isolated Git worktree…"),
        );

        Some(cmd::cmd(move || async move {
            let result = fork_into_worktree(
                store,
                source_id,
                destination_id,
                workspace,
                identity,
                sidecar,
            )
            .await;
            Msg::WorktreeForked { request_id, result }
        }))
    }

    fn reserve_fork_request(&mut self) -> u64 {
        self.session_rebuild_seq = self.session_rebuild_seq.wrapping_add(1);
        let request_id = self.session_rebuild_seq;
        self.session_rebuild_pending = Some(request_id);
        request_id
    }

    pub(super) fn finish_worktree_fork(
        &mut self,
        request_id: u64,
        result: Result<WorktreeForkResult, String>,
    ) -> Option<Cmd<Msg>> {
        if self.session_rebuild_pending != Some(request_id) {
            return None;
        }
        self.session_rebuild_pending = None;

        match result {
            Ok(result) => {
                let command = worktree_resume_command(&result.workspace, &result.session_id);
                self.push_line(&gutter(
                    TN_CYAN,
                    &format!(
                        "⑂ isolated fork ready · {} · {}",
                        result.branch,
                        result.worktree_root.display()
                    ),
                ));
                self.push_line(
                    &Style::new()
                        .fg(TN_FG)
                        .bold()
                        .render(&format!("  {command}")),
                );
            }
            Err(error) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_YELLOW)
                        .render(&format!("  /fork worktree: {error}")),
                );
            }
        }
        self.drain_queue()
    }
}

async fn fork_into_worktree(
    source_store: Arc<dyn a3s_code_core::store::SessionStore>,
    source_id: String,
    destination_id: String,
    workspace: PathBuf,
    identity: String,
    sidecar: TuiSessionState,
) -> Result<WorktreeForkResult, String> {
    let snapshot = source_store
        .load_snapshot(&source_id)
        .await
        .map_err(|error| format!("could not read the session: {error}"))?
        .ok_or_else(|| "nothing to fork yet — start a conversation first".to_string())?;

    let isolated = tokio::task::spawn_blocking(move || {
        GitTreeSnapshot::capture(&workspace)?.fork_worktree(&identity)
    })
    .await
    .map_err(|error| format!("worktree task failed: {error}"))?
    .map_err(|error| error.to_string())?;

    let result = persist_worktree_fork(snapshot, destination_id, isolated, sidecar).await;
    result.map_err(|(error, retained)| {
        format!(
            "{error}; the recoverable worktree was retained at {}",
            retained.display()
        )
    })
}

async fn persist_worktree_fork(
    snapshot: a3s_code_core::store::SessionSnapshotV1,
    destination_id: String,
    isolated: IsolatedWorktree,
    sidecar: TuiSessionState,
) -> Result<WorktreeForkResult, (String, PathBuf)> {
    let retained = isolated.root.clone();
    let workspace = isolated.workspace.to_string_lossy().into_owned();
    let snapshot = snapshot
        .fork_for_session(destination_id.clone(), workspace)
        .map_err(|error| {
            (
                format!("could not prepare the forked session: {error:#}"),
                retained.clone(),
            )
        })?;
    let store_dir = resolve_tui_session_store_dir(&isolated.workspace);
    let destination_store = a3s_code_core::store::FileSessionStore::new(&store_dir)
        .await
        .map_err(|error| {
            (
                format!(
                    "could not open forked session store {}: {error}",
                    store_dir.display()
                ),
                retained.clone(),
            )
        })?;
    destination_store
        .save_snapshot(&snapshot)
        .await
        .map_err(|error| {
            (
                format!("could not save the forked session: {error}"),
                retained.clone(),
            )
        })?;
    save_tui_session_state(&isolated.workspace, &destination_id, &sidecar).map_err(|error| {
        (
            format!("could not save forked TUI state: {error}"),
            retained.clone(),
        )
    })?;

    Ok(WorktreeForkResult {
        session_id: destination_id,
        workspace: isolated.workspace,
        worktree_root: isolated.root,
        branch: isolated.branch,
    })
}

fn worktree_resume_command(workspace: &Path, session_id: &str) -> String {
    format!(
        "cd -- {} && a3s code resume {}",
        shell_single_quote(&workspace.to_string_lossy()),
        shell_single_quote(session_id)
    )
}

fn shell_single_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\"'\"'"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fork_parser_keeps_session_compatibility_and_adds_worktree_isolation() {
        assert_eq!(parse_fork_command(""), Ok(ForkCommand::Session));
        assert_eq!(parse_fork_command(" session"), Ok(ForkCommand::Session));
        assert_eq!(parse_fork_command(" worktree"), Ok(ForkCommand::Worktree));
        assert!(parse_fork_command(" branch").is_err());
    }

    #[test]
    fn worktree_launch_command_quotes_paths_and_session_ids() {
        let command =
            worktree_resume_command(Path::new("/tmp/a user's workspace"), "session'quoted");
        assert_eq!(
            command,
            "cd -- '/tmp/a user'\"'\"'s workspace' && a3s code resume 'session'\"'\"'quoted'"
        );
    }
}
