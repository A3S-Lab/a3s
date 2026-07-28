//! `/evolution` review surface for memory-derived preferences, Skills, and OKF packages.

use super::super::*;
use a3s_tui::components::{divider_line_with, Badge, DetailPanel, DetailRow, Paragraph, Progress};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum EvolutionPendingAction {
    Reject,
    Rollback,
}

#[derive(Debug, Clone)]
pub(crate) struct EvolutionPanel {
    pub(crate) overview: Option<crate::evolution::EvolutionOverview>,
    pub(crate) selected: usize,
    pub(crate) detail_scroll: usize,
    pub(crate) note: String,
    pub(crate) busy: bool,
    pub(crate) confirm: Option<EvolutionPendingAction>,
}

impl EvolutionPanel {
    pub(crate) fn loading() -> Self {
        Self {
            overview: None,
            selected: 0,
            detail_scroll: 0,
            note: "loading memory evidence…".to_string(),
            busy: true,
            confirm: None,
        }
    }

    fn candidates(&self) -> &[crate::evolution::EvolutionCandidate] {
        self.overview
            .as_ref()
            .map(|overview| overview.candidates.as_slice())
            .unwrap_or_default()
    }

    fn selected_candidate(&self) -> Option<&crate::evolution::EvolutionCandidate> {
        self.candidates().get(self.selected)
    }

    pub(crate) fn apply_overview(&mut self, overview: crate::evolution::EvolutionOverview) {
        let selected_id = self
            .selected_candidate()
            .map(|candidate| candidate.id.clone());
        self.overview = Some(overview);
        self.selected = selected_id
            .and_then(|id| {
                self.candidates()
                    .iter()
                    .position(|candidate| candidate.id == id)
            })
            .unwrap_or_else(|| self.selected.min(self.candidates().len().saturating_sub(1)));
        self.detail_scroll = 0;
    }
}

#[derive(Debug, Clone)]
pub(crate) struct EvolutionUiMutation {
    pub(crate) overview: crate::evolution::EvolutionOverview,
    pub(crate) message: String,
    pub(crate) requires_session_reload: bool,
}

#[derive(Debug, Clone, Copy)]
enum EvolutionAction {
    Materialize,
    Reject,
    Reopen,
    Rollback,
}

impl App {
    pub(crate) fn load_evolution_panel(&self) -> Cmd<Msg> {
        evolution_load_cmd(self.cwd.clone(), self.memory_dir.clone())
    }

    pub(crate) fn evolution_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if key.code == KeyCode::Esc {
            self.evolution = None;
            return None;
        }
        let panel = self.evolution.as_mut()?;
        if panel.busy {
            return None;
        }
        let last = panel.candidates().len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                panel.selected = panel.selected.saturating_sub(1);
                panel.detail_scroll = 0;
                panel.confirm = None;
            }
            KeyCode::Down | KeyCode::Char('j') => {
                panel.selected = (panel.selected + 1).min(last);
                panel.detail_scroll = 0;
                panel.confirm = None;
            }
            KeyCode::Char('g') => {
                panel.selected = 0;
                panel.detail_scroll = 0;
                panel.confirm = None;
            }
            KeyCode::Char('G') => {
                panel.selected = last;
                panel.detail_scroll = 0;
                panel.confirm = None;
            }
            KeyCode::PageUp => {
                panel.detail_scroll = panel
                    .detail_scroll
                    .saturating_sub((self.height as usize / 2).max(1));
            }
            KeyCode::PageDown => {
                panel.detail_scroll += (self.height as usize / 2).max(1);
            }
            KeyCode::Char('r') => {
                panel.busy = true;
                panel.confirm = None;
                panel.note = "rescanning durable memory…".to_string();
                return Some(evolution_load_cmd(
                    self.cwd.clone(),
                    self.memory_dir.clone(),
                ));
            }
            KeyCode::Char('m') | KeyCode::Enter => {
                return self.start_evolution_action(EvolutionAction::Materialize);
            }
            KeyCode::Char('o') => {
                return self.start_evolution_action(EvolutionAction::Reopen);
            }
            KeyCode::Char('x') => {
                if panel.confirm == Some(EvolutionPendingAction::Reject) {
                    return self.start_evolution_action(EvolutionAction::Reject);
                }
                panel.confirm = Some(EvolutionPendingAction::Reject);
                panel.note =
                    "press x again to reject this candidate (o can reopen it later)".to_string();
            }
            KeyCode::Char('b') => {
                if panel.confirm == Some(EvolutionPendingAction::Rollback) {
                    return self.start_evolution_action(EvolutionAction::Rollback);
                }
                panel.confirm = Some(EvolutionPendingAction::Rollback);
                panel.note = match panel.selected_candidate().and_then(|candidate| candidate.current_version) {
                    None => "press b again to restore the latest saved version".to_string(),
                    Some(1) => "press b again to undo materialization; the local asset will be preserved in recovery"
                        .to_string(),
                    Some(_) => "press b again to restore the preceding version; the current asset will be preserved in recovery"
                        .to_string(),
                };
            }
            _ => {
                panel.confirm = None;
            }
        }
        None
    }

    fn start_evolution_action(&mut self, action: EvolutionAction) -> Option<Cmd<Msg>> {
        let panel = self.evolution.as_mut()?;
        let candidate = panel.selected_candidate()?.clone();
        panel.busy = true;
        panel.confirm = None;
        panel.note = match action {
            EvolutionAction::Materialize => "materializing a local version…",
            EvolutionAction::Reject => "rejecting candidate…",
            EvolutionAction::Reopen => "reopening candidate…",
            EvolutionAction::Rollback => "preserving current asset and rolling back…",
        }
        .to_string();
        let workspace = self.cwd.clone();
        let memory_dir = self.memory_dir.clone();
        Some(cmd::cmd(move || async move {
            let result = run_evolution_action(workspace, memory_dir, candidate, action).await;
            Msg::EvolutionMutated(result.map_err(|error| error.to_string()))
        }))
    }

    pub(crate) fn render_evolution(&self, panel: &EvolutionPanel) -> String {
        let width = self.width as usize;
        let height = self.height as usize;
        let mut lines = Vec::new();
        let stats = panel.overview.as_ref().map(|overview| &overview.stats);
        let header = match stats {
            Some(stats) => format!(
                "  evolution · {} candidates · {} ready · {} materialized · {} updates · {} activation pending   {}",
                stats.total,
                stats.ready,
                stats.materialized,
                stats.update_available,
                stats.activation_pending,
                Style::new().fg(TN_GRAY).render(&panel.note),
            ),
            None => format!("  evolution   {}", Style::new().fg(TN_GRAY).render(&panel.note)),
        };
        lines.push(fit_evolution_line(&header, width));
        lines.push(fit_evolution_line(
            &divider_line_with(width.min(u16::MAX as usize) as u16, "─", TN_GRAY),
            width,
        ));
        let body_height = height.saturating_sub(3);
        let candidates = panel.candidates();
        if candidates.is_empty() {
            let empty = if panel.busy {
                "  scanning durable memory for reusable patterns…"
            } else {
                "  no evolution candidates yet — recurring preferences, workflows, and knowledge will appear here"
            };
            lines.push(fit_evolution_line(
                &Style::new().fg(TN_GRAY).render(empty),
                width,
            ));
            while lines.len() + 1 < height {
                lines.push(String::new());
            }
            lines.push(fit_evolution_line("  r rescan · Esc close", width));
            lines.truncate(height);
            return lines.join("\n");
        }

        let left_width = (width / 3).clamp(30, 54);
        let right_width = width.saturating_sub(left_width + 3);
        let left = evolution_candidate_lines(candidates, panel.selected, left_width, body_height);
        let right = evolution_detail_lines(panel.selected_candidate(), right_width);
        let separator = Style::new().fg(TN_GRAY).render(" │ ");
        for row in 0..body_height {
            let left = left
                .get(row)
                .cloned()
                .unwrap_or_else(|| " ".repeat(left_width));
            let right = right
                .get(panel.detail_scroll + row)
                .cloned()
                .unwrap_or_default();
            lines.push(format!("{left}{separator}{right}"));
        }
        lines.push(fit_evolution_line(
            "  ↑↓/jk select · Enter/m materialize/update · x reject · o reopen · b rollback · PgUp/PgDn detail · r rescan · Esc close",
            width,
        ));
        lines.truncate(height);
        lines.join("\n")
    }
}

fn evolution_load_cmd(workspace: String, memory_dir: std::path::PathBuf) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let evolution = crate::evolution::WorkspaceEvolution::new(workspace);
        let result = async {
            evolution.synchronize_memory_store(memory_dir).await?;
            evolution.overview().await
        }
        .await;
        Msg::EvolutionLoaded(result.map_err(|error| error.to_string()))
    })
}

async fn run_evolution_action(
    workspace: String,
    memory_dir: std::path::PathBuf,
    candidate: crate::evolution::EvolutionCandidate,
    action: EvolutionAction,
) -> anyhow::Result<EvolutionUiMutation> {
    let evolution = crate::evolution::WorkspaceEvolution::new(workspace);
    let (message, requires_session_reload) = match action {
        EvolutionAction::Materialize => {
            let result = evolution.materialize(&candidate.id, false).await?;
            let version = result.candidate.current_version.unwrap_or_default();
            (
                format!(
                    "materialized {} v{} at {}",
                    result.candidate.kind.label(),
                    version,
                    result
                        .candidate
                        .asset_path
                        .as_deref()
                        .unwrap_or("local asset")
                ),
                result.requires_session_reload,
            )
        }
        EvolutionAction::Reject => {
            evolution
                .reject(
                    &candidate.id,
                    Some("Rejected during TUI review".to_string()),
                )
                .await?;
            (
                "candidate rejected; press o to reopen it".to_string(),
                false,
            )
        }
        EvolutionAction::Reopen => {
            evolution.reopen(&candidate.id).await?;
            ("candidate reopened for observation".to_string(), false)
        }
        EvolutionAction::Rollback => {
            let result = evolution.rollback(&candidate.id, None).await?;
            let message = match result.candidate.current_version {
                Some(version) => format!(
                    "restored v{version}; recovery copy: {}",
                    result.recovery_path.as_deref().unwrap_or("not needed")
                ),
                None => format!(
                    "returned to the unmaterialized baseline; recovery copy: {}",
                    result.recovery_path.as_deref().unwrap_or("not needed")
                ),
            };
            (message, result.requires_session_reload)
        }
    };
    evolution.synchronize_memory_store(memory_dir).await?;
    Ok(EvolutionUiMutation {
        overview: evolution.overview().await?,
        message,
        requires_session_reload,
    })
}

fn evolution_candidate_lines(
    candidates: &[crate::evolution::EvolutionCandidate],
    selected: usize,
    width: usize,
    height: usize,
) -> Vec<String> {
    let start = selected.saturating_sub(height.saturating_sub(1) / 2);
    candidates
        .iter()
        .enumerate()
        .skip(start)
        .take(height)
        .map(|(index, candidate)| {
            let state_color = state_color(candidate.state);
            let warning = if candidate.has_conflicts {
                " !"
            } else if candidate.update_available {
                " ↑"
            } else if candidate.activation_pending {
                " ↻"
            } else {
                ""
            };
            let line = format!(
                " {} {} {}{} · {}×/{}s · {:.0}%  {}",
                if index == selected { "›" } else { " " },
                Badge::new(candidate.kind.label())
                    .color(kind_color(candidate.kind))
                    .view(),
                Style::new().fg(state_color).render(candidate.state.label()),
                warning,
                candidate.occurrences,
                candidate.distinct_sessions,
                candidate.maturity * 100.0,
                candidate.title,
            );
            let line = fit_evolution_line(&line, width);
            if index == selected {
                Style::new().bg(SURFACE_SELECTED).render(&line)
            } else {
                line
            }
        })
        .collect()
}

fn evolution_detail_lines(
    candidate: Option<&crate::evolution::EvolutionCandidate>,
    width: usize,
) -> Vec<String> {
    let Some(candidate) = candidate else {
        return Vec::new();
    };
    let mut lines = vec![fit_evolution_line(
        &Style::new().fg(TN_FG).bold().render(&candidate.title),
        width,
    )];
    let progress = Progress::new()
        .value(f64::from(candidate.maturity))
        .width(10)
        .filled_color(state_color(candidate.state))
        .empty_color(TN_GRAY)
        .show_percentage(true)
        .view();
    let panel = DetailPanel::without_title()
        .show_separator(false)
        .indent(0)
        .label_width(13)
        .unlimited_rows()
        .row(DetailRow::pair("state", candidate.state.label()).color(state_color(candidate.state)))
        .pair("pattern", candidate.pattern_key.clone())
        .pair(
            "evidence",
            format!(
                "{} observations · {} sessions",
                candidate.occurrences, candidate.distinct_sessions
            ),
        )
        .pair(
            "quality",
            format!(
                "confidence {:.2} · importance {:.2}",
                candidate.confidence, candidate.importance
            ),
        )
        .pair("maturity", progress)
        .pair(
            "asset",
            candidate
                .asset_path
                .as_deref()
                .unwrap_or("not materialized"),
        )
        .pair(
            "version",
            candidate
                .current_version
                .map(|version| format!("v{version}"))
                .unwrap_or_else(|| "—".to_string()),
        );
    lines.extend(
        panel
            .view(width.min(u16::MAX as usize) as u16, panel.rows().len())
            .lines()
            .map(str::to_string),
    );
    lines.push(divider_line_with(width.min(48) as u16, "─", TN_GRAY));
    lines.extend(wrapped_section("Summary", &candidate.summary, width));
    lines.extend(wrapped_list(
        "Learned instructions",
        &candidate.instructions,
        width,
    ));
    if candidate.has_conflicts {
        lines.extend(wrapped_section(
            "Conflict",
            "This candidate has contradictory memory evidence and will not materialize automatically.",
            width,
        ));
    }
    lines.push(fit_evolution_line("Evidence", width));
    for evidence in candidate.evidence.iter().rev().take(20) {
        let session = evidence.session_id.as_deref().unwrap_or("unknown session");
        let row = format!(
            "• {} · {} · {:.2} · {}",
            evidence.timestamp.format("%Y-%m-%d"),
            session,
            evidence.confidence,
            evidence.content.replace('\n', " ")
        );
        lines.extend(
            Paragraph::new(row)
                .width(width.max(8))
                .color(TN_GRAY)
                .lines(),
        );
    }
    if !candidate.versions.is_empty() {
        lines.push(fit_evolution_line("Versions", width));
        for version in candidate.versions.iter().rev() {
            lines.push(fit_evolution_line(
                &format!(
                    "• v{} · {} · {} evidence{}",
                    version.version,
                    version.created_at.format("%Y-%m-%d %H:%M"),
                    version.evidence_ids.len(),
                    if version.automatic {
                        " · automatic"
                    } else {
                        ""
                    }
                ),
                width,
            ));
        }
    }
    lines
        .into_iter()
        .map(|line| fit_evolution_line(&line, width))
        .collect()
}

fn wrapped_section(title: &str, body: &str, width: usize) -> Vec<String> {
    let mut lines = vec![title.to_string()];
    lines.extend(
        Paragraph::new(body)
            .width(width.max(8))
            .color(TN_FG)
            .lines(),
    );
    lines
}

fn wrapped_list(title: &str, values: &[String], width: usize) -> Vec<String> {
    let mut lines = vec![title.to_string()];
    for value in values {
        lines.extend(
            Paragraph::new(format!("• {value}"))
                .width(width.max(8))
                .color(TN_FG)
                .lines(),
        );
    }
    lines
}

fn kind_color(kind: crate::evolution::EvolutionKind) -> Color {
    match kind {
        crate::evolution::EvolutionKind::Preference => TN_YELLOW,
        crate::evolution::EvolutionKind::Skill => TN_GREEN,
        crate::evolution::EvolutionKind::Okf => TN_CYAN,
    }
}

fn state_color(state: crate::evolution::EvolutionState) -> Color {
    match state {
        crate::evolution::EvolutionState::Observing => TN_GRAY,
        crate::evolution::EvolutionState::Ready => TN_YELLOW,
        crate::evolution::EvolutionState::Materialized => TN_GREEN,
        crate::evolution::EvolutionState::Rejected => TN_RED,
        crate::evolution::EvolutionState::RolledBack => TN_CYAN,
    }
}

fn fit_evolution_line(line: &str, width: usize) -> String {
    pad_to(&truncate(line, width), width)
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_memory::{MemoryItem, MemoryStore, MemoryType};

    #[test]
    fn loading_panel_has_no_candidate() {
        let panel = EvolutionPanel::loading();
        assert!(panel.selected_candidate().is_none());
        assert!(panel.busy);
    }

    #[tokio::test]
    async fn detail_render_includes_evidence_instructions_and_versions() {
        let temp = tempfile::tempdir().unwrap();
        let memory_dir = temp.path().join("memory");
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let (evolution, candidate) = seed_preference(&workspace, &memory_dir).await;
        evolution.materialize(&candidate.id, false).await.unwrap();
        let overview = evolution.overview().await.unwrap();
        let candidate = &overview.candidates[0];

        let rendered = evolution_detail_lines(Some(candidate), 96).join("\n");

        assert!(rendered.contains("Concise evidence-backed responses"));
        assert!(rendered.contains("Lead with the outcome"));
        assert!(rendered.contains("Evidence"));
        assert!(rendered.contains("Keep future responses concise"));
        assert!(rendered.contains("Versions"));
        assert!(rendered.contains("v1"));
    }

    #[tokio::test]
    async fn tui_actions_materialize_reject_and_reopen_shared_candidates() {
        let temp = tempfile::tempdir().unwrap();
        let memory_dir = temp.path().join("memory");
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let (_, candidate) = seed_preference(&workspace, &memory_dir).await;

        let materialized = run_evolution_action(
            workspace.display().to_string(),
            memory_dir.clone(),
            candidate,
            EvolutionAction::Materialize,
        )
        .await
        .unwrap();
        assert!(materialized.message.contains("materialized preference v1"));
        assert!(materialized.requires_session_reload);
        assert!(workspace.join(".a3s/evolution/preferences").is_dir());

        let second_temp = tempfile::tempdir().unwrap();
        let second_memory = second_temp.path().join("memory");
        let second_workspace = second_temp.path().join("workspace");
        tokio::fs::create_dir_all(&second_workspace).await.unwrap();
        let (_, candidate) = seed_preference(&second_workspace, &second_memory).await;
        let rejected = run_evolution_action(
            second_workspace.display().to_string(),
            second_memory.clone(),
            candidate,
            EvolutionAction::Reject,
        )
        .await
        .unwrap();
        assert_eq!(
            rejected.overview.candidates[0].state,
            crate::evolution::EvolutionState::Rejected
        );
        let reopened = run_evolution_action(
            second_workspace.display().to_string(),
            second_memory,
            rejected.overview.candidates[0].clone(),
            EvolutionAction::Reopen,
        )
        .await
        .unwrap();
        assert_eq!(
            reopened.overview.candidates[0].state,
            crate::evolution::EvolutionState::Ready
        );
    }

    #[tokio::test]
    async fn tui_and_web_handles_share_skill_versions_and_session_discovery() {
        let temp = tempfile::tempdir().unwrap();
        let memory_dir = temp.path().join("memory");
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let store = a3s_memory::FileMemoryStore::new(&memory_dir).await.unwrap();
        store
            .store(skill_memory(
                "skill-one",
                "session-one",
                "Run the focused crate tests first.",
            ))
            .await
            .unwrap();
        store
            .store(skill_memory(
                "skill-two",
                "session-two",
                "Start verification with the smallest relevant package target.",
            ))
            .await
            .unwrap();

        let web = crate::evolution::WorkspaceEvolution::new(&workspace);
        web.synchronize_memory_store(&memory_dir).await.unwrap();
        let ready = web.overview().await.unwrap().candidates.remove(0);
        assert_eq!(ready.state, crate::evolution::EvolutionState::Ready);

        let first = run_evolution_action(
            workspace.display().to_string(),
            memory_dir.clone(),
            ready,
            EvolutionAction::Materialize,
        )
        .await
        .unwrap();
        assert!(first.requires_session_reload);
        assert_eq!(first.overview.candidates[0].current_version, Some(1));

        let web_view = web.overview().await.unwrap();
        assert_eq!(web_view.candidates[0].current_version, Some(1));
        assert!(web_view.candidates[0].asset_path.is_some());

        store
            .store(skill_memory(
                "skill-three",
                "session-three",
                "Keep broad workspace checks until focused verification passes.",
            ))
            .await
            .unwrap();
        web.synchronize_memory_store(&memory_dir).await.unwrap();
        let updated = web.overview().await.unwrap().candidates.remove(0);
        assert!(updated.update_available);
        let second = run_evolution_action(
            workspace.display().to_string(),
            memory_dir.clone(),
            updated,
            EvolutionAction::Materialize,
        )
        .await
        .unwrap();
        assert_eq!(second.overview.candidates[0].current_version, Some(2));

        let rolled_back = web
            .rollback(&second.overview.candidates[0].id, Some(1))
            .await
            .unwrap();
        assert_eq!(rolled_back.candidate.current_version, Some(1));
        let tui_reload = crate::evolution::WorkspaceEvolution::new(&workspace)
            .overview()
            .await
            .unwrap();
        assert_eq!(
            tui_reload.candidates[0].state,
            crate::evolution::EvolutionState::RolledBack
        );
        assert_eq!(tui_reload.candidates[0].current_version, Some(1));

        let baseline = run_evolution_action(
            workspace.display().to_string(),
            memory_dir.clone(),
            tui_reload.candidates[0].clone(),
            EvolutionAction::Rollback,
        )
        .await
        .unwrap();
        assert!(baseline.requires_session_reload);
        assert!(baseline.message.contains("unmaterialized baseline"));
        assert_eq!(baseline.overview.candidates[0].current_version, None);
        assert!(!skill_root_for(&workspace, &baseline.overview.candidates[0]).exists());

        let restored = web
            .rollback(&baseline.overview.candidates[0].id, Some(1))
            .await
            .unwrap();
        assert_eq!(restored.candidate.current_version, Some(1));

        let skill_root = workspace.join(".a3s/skills");
        let registry = a3s_code_core::skills::SkillRegistry::new();
        assert_eq!(registry.load_from_dir(&skill_root).unwrap(), 1);
        assert!(registry
            .all()
            .iter()
            .any(|skill| skill.name == "learned-focused-verification"));
        let discovered = crate::tui::skills::agent_skill_dirs_with_configured(
            &workspace.display().to_string(),
            &temp.path().join("unused-configured-skills"),
        );
        assert!(discovered.contains(&skill_root));
    }

    async fn seed_preference(
        workspace: &std::path::Path,
        memory_dir: &std::path::Path,
    ) -> (
        crate::evolution::WorkspaceEvolution,
        crate::evolution::EvolutionCandidate,
    ) {
        let store = a3s_memory::FileMemoryStore::new(memory_dir).await.unwrap();
        let item = MemoryItem::new("Keep future responses concise and evidence-backed.")
            .with_type(MemoryType::Semantic)
            .with_importance(0.94)
            .with_metadata("source", "preference")
            .with_metadata("scope", "user")
            .with_metadata("confidence", "0.96")
            .with_metadata("session_id", "session-one")
            .with_metadata("evolution_schema", "a3s.evolution.signal.v1")
            .with_metadata("evolution_kind", "preference")
            .with_metadata("evolution_pattern", "preference.response.concise-evidence")
            .with_metadata("evolution_title", "Concise evidence-backed responses")
            .with_metadata(
                "evolution_summary",
                "Lead with outcomes while retaining concrete supporting evidence.",
            )
            .with_metadata(
                "evolution_instructions",
                r#"["Lead with the outcome.","Keep supporting evidence concrete and concise."]"#,
            );
        store.store(item).await.unwrap();
        let evolution = crate::evolution::WorkspaceEvolution::new(workspace);
        evolution
            .synchronize_memory_store(memory_dir.to_path_buf())
            .await
            .unwrap();
        let candidate = evolution.overview().await.unwrap().candidates.remove(0);
        (evolution, candidate)
    }

    fn skill_memory(id: &str, session: &str, content: &str) -> MemoryItem {
        let mut item = MemoryItem::new(content)
            .with_type(MemoryType::Procedural)
            .with_importance(0.92)
            .with_metadata("source", "workflow")
            .with_metadata("scope", "workspace")
            .with_metadata("confidence", "0.95")
            .with_metadata("session_id", session)
            .with_metadata("evolution_schema", "a3s.evolution.signal.v1")
            .with_metadata("evolution_kind", "skill")
            .with_metadata("evolution_pattern", "skill.verification.focused")
            .with_metadata("evolution_title", "Focused verification")
            .with_metadata(
                "evolution_summary",
                "Run focused verification before broad workspace checks.",
            )
            .with_metadata(
                "evolution_instructions",
                r#"["Identify the smallest relevant test target.","Run focused checks before broad validation."]"#,
            );
        item.id = id.to_string();
        item
    }

    fn skill_root_for(
        workspace: &std::path::Path,
        candidate: &crate::evolution::EvolutionCandidate,
    ) -> std::path::PathBuf {
        workspace.join(candidate.asset_path.as_deref().unwrap())
    }
}
