//! Conflict-safe per-turn conversation and workspace rewind.

use super::*;

const MAX_REWIND_CHECKPOINTS: usize = 3;
const REWIND_GIT_CAPTURE_TIMEOUT: Duration = Duration::from_secs(5);

pub(super) async fn capture_rewind_checkpoint_seed(
    store: Arc<dyn a3s_code_core::store::SessionStore>,
    source_session_id: String,
    workspace: PathBuf,
    task: String,
    history_before: Vec<Message>,
) -> RewindCheckpointSeed {
    let mut warnings = Vec::new();
    let session_before = match store.load_snapshot(&source_session_id).await {
        Ok(Some(snapshot)) if snapshot.session.id == source_session_id => {
            let mut session = snapshot.session;
            // The live session is authoritative if a previous save was delayed.
            session.messages.clone_from(&history_before);
            Some(session)
        }
        Ok(Some(snapshot)) => {
            warnings.push(format!(
                "saved session belongs to {}, not {}",
                snapshot.session.id, source_session_id
            ));
            None
        }
        Ok(None) => None,
        Err(error) => {
            warnings.push(format!("could not read the pre-turn session: {error}"));
            None
        }
    };

    let capture = tokio::task::spawn_blocking(move || GitTreeSnapshot::capture(&workspace));
    let git_before = match tokio::time::timeout(REWIND_GIT_CAPTURE_TIMEOUT, capture).await {
        Ok(Ok(Ok(snapshot))) => Some(snapshot),
        Ok(Ok(Err(error))) => {
            warnings.push(format!("file rewind unavailable: {error}"));
            None
        }
        Ok(Err(error)) => {
            warnings.push(format!("file checkpoint task failed: {error}"));
            None
        }
        Err(_) => {
            warnings.push(format!(
                "file checkpoint exceeded {} seconds",
                REWIND_GIT_CAPTURE_TIMEOUT.as_secs()
            ));
            None
        }
    };

    RewindCheckpointSeed {
        source_session_id,
        task,
        history_before,
        session_before,
        git_before,
        warning: joined_warning(warnings),
    }
}

impl App {
    pub(super) fn start_rewind_checkpoint_finalization(
        &mut self,
        token: u64,
        synthesis: Option<(String, String)>,
    ) -> Option<Cmd<Msg>> {
        let Some(seed) = self.active_rewind_checkpoint.take() else {
            self.state = State::Idle;
            self.relayout();
            return self.continue_after_stream_settled(synthesis);
        };
        self.next_rewind_checkpoint_id = self.next_rewind_checkpoint_id.wrapping_add(1);
        let checkpoint_id = self.next_rewind_checkpoint_id;
        self.rewind_finalization_pending = Some(token);
        let workspace = PathBuf::from(&self.cwd);
        let fallback = seed.clone();

        Some(cmd::cmd(move || async move {
            let capture = tokio::task::spawn_blocking(move || {
                finalize_rewind_checkpoint(checkpoint_id, seed, &workspace)
            });
            let checkpoint = match tokio::time::timeout(REWIND_GIT_CAPTURE_TIMEOUT, capture).await {
                Ok(Ok(checkpoint)) => checkpoint,
                Ok(Err(error)) => conversation_only_checkpoint(
                    checkpoint_id,
                    fallback,
                    format!("file checkpoint task failed: {error}"),
                ),
                Err(_) => conversation_only_checkpoint(
                    checkpoint_id,
                    fallback,
                    format!(
                        "file checkpoint exceeded {} seconds",
                        REWIND_GIT_CAPTURE_TIMEOUT.as_secs()
                    ),
                ),
            };
            Msg::RewindCheckpointFinalized {
                token,
                checkpoint,
                synthesis,
            }
        }))
    }

    pub(super) fn finish_rewind_checkpoint_finalization(
        &mut self,
        token: u64,
        checkpoint: RewindCheckpoint,
        synthesis: Option<(String, String)>,
    ) -> Option<Cmd<Msg>> {
        if self.rewind_finalization_pending != Some(token) || token != self.stream_start_token {
            return None;
        }
        self.rewind_finalization_pending = None;
        self.rewind_checkpoints.push_back(checkpoint);
        while self.rewind_checkpoints.len() > MAX_REWIND_CHECKPOINTS {
            self.rewind_checkpoints.pop_front();
        }
        self.state = State::Idle;
        self.relayout();
        self.continue_after_stream_settled(synthesis)
    }

    pub(super) fn discard_active_rewind_checkpoint(&mut self) {
        self.active_rewind_checkpoint = None;
        self.rewind_finalization_pending = None;
    }

    pub(super) fn submit_rewind_command(&mut self) -> Option<Cmd<Msg>> {
        self.textarea.clear();
        let Some(checkpoint) = self
            .rewind_checkpoints
            .iter()
            .rev()
            .find(|checkpoint| checkpoint.source_session_id == self.session_id)
            .cloned()
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  /rewind: no completed user turn is available"),
            );
            return None;
        };

        let destination_id = new_session_id();
        let sidecar = TuiSessionState::capture_for_session(self, destination_id.clone());
        let store = self.store.clone();
        let workspace = PathBuf::from(&self.cwd);
        let task = truncate(&checkpoint.task, 96);
        self.session_rebuild_seq = self.session_rebuild_seq.wrapping_add(1);
        let request_id = self.session_rebuild_seq;
        self.session_rebuild_pending = Some(request_id);
        self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
            "  checking the last turn for a conflict-safe rewind · {task}"
        )));

        Some(cmd::cmd(move || async move {
            let result =
                execute_rewind(store, workspace, destination_id, checkpoint, sidecar).await;
            Msg::Rewound { request_id, result }
        }))
    }

    pub(super) fn finish_rewind_command(
        &mut self,
        request_id: u64,
        result: Result<RewindResult, String>,
    ) -> Option<Cmd<Msg>> {
        if self.session_rebuild_pending != Some(request_id) {
            return None;
        }
        self.session_rebuild_pending = None;
        match result {
            Ok(result) => {
                self.rewind_checkpoints
                    .retain(|checkpoint| checkpoint.id != result.checkpoint_id);
                let mut profile = self.session_rebuild_profile();
                profile.session_id = result.session_id.clone();
                self.start_session_rebuild(
                    profile,
                    SessionRebuildAction::Rewind {
                        session_id: result.session_id,
                        files_rewound: result.files_rewound,
                        warning: result.warning,
                    },
                )
            }
            Err(error) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_YELLOW)
                        .render(&format!("  /rewind: {error}")),
                );
                self.drain_queue()
            }
        }
    }
}

fn finalize_rewind_checkpoint(
    id: u64,
    seed: RewindCheckpointSeed,
    workspace: &Path,
) -> RewindCheckpoint {
    let mut warning = seed.warning.clone();
    let (file_patch, repository_root) = match seed.git_before.as_ref() {
        Some(before) => {
            match GitTreeSnapshot::capture(workspace).and_then(|after| before.diff_to(&after)) {
                Ok(patch) => (Some(patch), Some(before.repository_root().to_path_buf())),
                Err(error) => {
                    warning = merge_warning(warning, format!("file rewind unavailable: {error}"));
                    (None, None)
                }
            }
        }
        None => (None, None),
    };

    RewindCheckpoint {
        id,
        source_session_id: seed.source_session_id,
        task: seed.task,
        history_before: seed.history_before,
        session_before: seed.session_before,
        file_patch,
        repository_root,
        warning,
    }
}

fn conversation_only_checkpoint(
    id: u64,
    seed: RewindCheckpointSeed,
    warning: String,
) -> RewindCheckpoint {
    RewindCheckpoint {
        id,
        source_session_id: seed.source_session_id,
        task: seed.task,
        history_before: seed.history_before,
        session_before: seed.session_before,
        file_patch: None,
        repository_root: None,
        warning: merge_warning(seed.warning, warning),
    }
}

async fn execute_rewind(
    store: Arc<dyn a3s_code_core::store::SessionStore>,
    workspace: PathBuf,
    destination_id: String,
    checkpoint: RewindCheckpoint,
    sidecar: TuiSessionState,
) -> Result<RewindResult, String> {
    if let (Some(patch), Some(repository_root)) =
        (&checkpoint.file_patch, &checkpoint.repository_root)
    {
        let patch = patch.clone();
        let repository_root = repository_root.clone();
        tokio::task::spawn_blocking(move || {
            patch.check_apply(&repository_root, GitPatchDirection::Reverse)
        })
        .await
        .map_err(|error| format!("rewind conflict check failed: {error}"))?
        .map_err(|error| {
            format!("workspace changed after this turn; refusing to overwrite it: {error}")
        })?;
    }

    let snapshot =
        rewind_session_snapshot(store.as_ref(), &checkpoint, &destination_id, &workspace).await?;
    store
        .save_snapshot(&snapshot)
        .await
        .map_err(|error| format!("could not save the rewound session: {error}"))?;
    save_tui_session_state(&workspace, &destination_id, &sidecar)
        .map_err(|error| format!("could not save rewound TUI state: {error}"))?;

    let files_rewound = checkpoint
        .file_patch
        .as_ref()
        .is_some_and(|patch| !patch.is_empty());
    if let (Some(patch), Some(repository_root)) =
        (checkpoint.file_patch, checkpoint.repository_root)
    {
        if !patch.is_empty() {
            tokio::task::spawn_blocking(move || {
                patch.apply_checked(&repository_root, GitPatchDirection::Reverse)
            })
            .await
            .map_err(|error| {
                format!(
                    "file rewind task failed after saving session {destination_id}: {error}"
                )
            })?
            .map_err(|error| {
                format!(
                    "workspace changed during rewind; session {destination_id} was saved but files were not changed: {error}"
                )
            })?;
        }
    }

    Ok(RewindResult {
        checkpoint_id: checkpoint.id,
        session_id: destination_id,
        files_rewound,
        warning: checkpoint.warning,
    })
}

async fn rewind_session_snapshot(
    store: &dyn a3s_code_core::store::SessionStore,
    checkpoint: &RewindCheckpoint,
    destination_id: &str,
    workspace: &Path,
) -> Result<a3s_code_core::store::SessionSnapshotV1, String> {
    let mut session = if let Some(session) = checkpoint.session_before.clone() {
        session
    } else {
        let mut session = store
            .load_snapshot(&checkpoint.source_session_id)
            .await
            .map_err(|error| format!("could not read the current session: {error}"))?
            .ok_or_else(|| "the current session snapshot is unavailable".to_string())?
            .session;
        session.context_usage = a3s_code_core::store::ContextUsage::default();
        session.total_usage = a3s_code_core::llm::TokenUsage::default();
        session.total_cost = 0.0;
        session.cost_records.clear();
        session.tasks.clear();
        session
    };
    session.messages.clone_from(&checkpoint.history_before);

    a3s_code_core::store::SessionSnapshotV1::session_only(session)
        .fork_for_session(
            destination_id.to_string(),
            workspace.to_string_lossy().into_owned(),
        )
        .map_err(|error| format!("could not prepare the rewound session: {error:#}"))
}

fn joined_warning(warnings: Vec<String>) -> Option<String> {
    let warnings = warnings
        .into_iter()
        .filter(|warning| !warning.trim().is_empty())
        .collect::<Vec<_>>();
    (!warnings.is_empty()).then(|| warnings.join("; "))
}

fn merge_warning(current: Option<String>, next: String) -> Option<String> {
    match current {
        Some(current) if !current.trim().is_empty() => Some(format!("{current}; {next}")),
        _ => Some(next),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn warnings_remain_bounded_and_readable() {
        assert_eq!(joined_warning(Vec::new()), None);
        assert_eq!(
            merge_warning(Some("first".to_string()), "second".to_string()).as_deref(),
            Some("first; second")
        );
    }

    #[tokio::test]
    async fn rewound_session_uses_the_pre_turn_conversation_under_a_new_id() {
        let workspace = tempfile::tempdir().unwrap();
        let before = test_session(
            "source-session",
            workspace.path(),
            vec![Message::user("keep this turn")],
        );
        let checkpoint = RewindCheckpoint {
            id: 7,
            source_session_id: "source-session".to_string(),
            task: "remove this turn".to_string(),
            history_before: before.messages.clone(),
            session_before: Some(before),
            file_patch: None,
            repository_root: None,
            warning: None,
        };
        let store = a3s_code_core::store::MemorySessionStore::new();

        let snapshot =
            rewind_session_snapshot(&store, &checkpoint, "rewound-session", workspace.path())
                .await
                .unwrap();

        assert_eq!(snapshot.session.id, "rewound-session");
        assert_eq!(snapshot.session.messages.len(), 1);
        assert_eq!(snapshot.session.messages[0].text(), "keep this turn");
        assert_eq!(
            snapshot.session.config.workspace,
            workspace.path().to_string_lossy()
        );
        snapshot.validate_for_session("rewound-session").unwrap();
    }

    fn test_session(
        id: &str,
        workspace: &Path,
        messages: Vec<Message>,
    ) -> a3s_code_core::store::SessionData {
        a3s_code_core::store::SessionData {
            id: id.to_string(),
            config: a3s_code_core::store::SessionConfig {
                workspace: workspace.to_string_lossy().into_owned(),
                ..a3s_code_core::store::SessionConfig::default()
            },
            state: a3s_code_core::store::SessionState::Active,
            messages,
            context_usage: a3s_code_core::store::ContextUsage::default(),
            total_usage: a3s_code_core::llm::TokenUsage::default(),
            total_cost: 0.0,
            model_name: None,
            cost_records: Vec::new(),
            tool_names: Vec::new(),
            thinking_enabled: false,
            thinking_budget: None,
            created_at: 1,
            updated_at: 1,
            llm_config: None,
            tasks: Vec::new(),
            parent_id: None,
            tenant_id: None,
            principal: None,
            agent_template_id: None,
            correlation_id: None,
        }
    }
}
