//! Main Code TUI controller actions and agent-event projection.

use super::*;

impl App {
    pub(super) fn session_status_line(&self, width: usize) -> String {
        render_session_status_line(
            &self.cwd,
            self.branch.as_deref(),
            self.model.as_deref(),
            self.context_limit,
            self.last_prompt_tokens,
            self.output_tokens,
            self.session_status_chips(),
            width,
        )
    }

    pub(super) fn session_status_chips(&self) -> Vec<SessionStatusChip> {
        let effective_mode = self.active_turn_mode.unwrap_or(self.mode);
        let mut chips = vec![mode_status_chip(effective_mode)];
        if self
            .active_turn_mode
            .is_some_and(|active| active != self.mode)
        {
            chips.push(
                SessionStatusChip::new("↪", format!("next:{}", self.mode.name()))
                    .color(COMPOSER_CHROME.secondary),
            );
        }
        if self.plan_review.is_some() {
            chips.push(SessionStatusChip::new("◆", "plan review").color(COMPOSER_CHROME.active));
        }

        if self.goal.is_some() {
            chips.push(goal_status_chip(self.goal_since));
        }
        if let Some(dev) = &self.agent_dev {
            chips.push(
                SessionStatusChip::new(
                    "◇",
                    format!("agent:{} · Esc /agent off", truncate(&dev.name, 24)),
                )
                .color(COMPOSER_CHROME.active),
            );
        }
        if let Some(dev) = &self.mcp_dev {
            chips.push(
                SessionStatusChip::new(
                    "◆",
                    format!("mcp:{} · Esc /mcp off", truncate(&dev.name, 24)),
                )
                .color(COMPOSER_CHROME.active),
            );
        }
        if let Some(dev) = &self.skill_dev {
            chips.push(
                SessionStatusChip::new(
                    "✦",
                    format!("skill:{} · Esc /skill off", truncate(&dev.name, 24)),
                )
                .color(COMPOSER_CHROME.active),
            );
        }
        if let Some(dev) = &self.okf_dev {
            chips.push(
                SessionStatusChip::new(
                    "⌁",
                    format!("okf:{} · Esc /okf off", truncate(&dev.name, 24)),
                )
                .color(COMPOSER_CHROME.active),
            );
        }
        if self.loop_remaining > 0 {
            chips.push(
                SessionStatusChip::new("↻", self.loop_remaining.to_string())
                    .color(COMPOSER_CHROME.secondary),
            );
        }
        if let Some(version) = self.update_available.as_deref() {
            chips.push(SessionStatusChip::new("⬆", version).color(COMPOSER_CHROME.warning));
        }

        chips
    }

    pub(super) fn clone_asset_command(
        &mut self,
        family: &'static str,
        url: String,
        root: std::path::PathBuf,
    ) -> Option<Cmd<Msg>> {
        let status_entry = self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
            "  cloning {family} asset from {url} → {}",
            root.display()
        )));
        Some(cmd::cmd(move || async move {
            Msg::AssetCloned {
                status_entry,
                result: asset_clone::clone_asset_source(family, url, root).await,
            }
        }))
    }

    pub(super) fn on_asset_cloned(
        &mut self,
        status_entry: TranscriptEntryId,
        result: asset_clone::AssetCloneResult,
    ) {
        self.replace_tracked_line(
            status_entry,
            &Style::new().fg(TN_GREEN).render(&format!(
                "  cloned {} asset → {}",
                result.family,
                result.path.display()
            )),
        );
        match result.family {
            "agent" => self.open_agent_panel_focused(&result.path),
            "mcp" => self.open_mcp_panel_focused(&result.path),
            "skill" => self.open_skill_panel_focused(&result.path),
            "okf" | "knowledge" => self.open_okf_package_panel_focused(&result.path),
            "workflow" => self.open_flow_panel_focused(&result.path),
            _ => self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  run the asset command again to select or operate on it"),
            ),
        }
    }

    pub(super) fn path_is_within(path: &std::path::Path, root: &std::path::Path) -> bool {
        path == root || path.starts_with(root)
    }

    pub(super) fn open_agent_panel_focused(&mut self, cloned_path: &std::path::Path) {
        let root = self.asset_directories.agent.clone();
        let agents = panels::agent::list_agents(&root);
        let Some(sel) = agents
            .iter()
            .position(|agent| Self::path_is_within(&agent.path, cloned_path))
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  cloned source does not contain a recognized agent definition yet"),
            );
            return;
        };
        self.agent_picker = Some(panels::agent::AgentPanel { root, agents, sel });
    }

    pub(super) fn open_mcp_panel_focused(&mut self, cloned_path: &std::path::Path) {
        let root = self.asset_directories.mcp.clone();
        let projects = panels::mcp::list_mcp_projects(&root);
        let Some(sel) = projects
            .iter()
            .position(|project| Self::path_is_within(&project.path, cloned_path))
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  cloned source does not contain a recognized MCP asset yet"),
            );
            return;
        };
        self.mcp_picker = Some(panels::mcp::McpPanel {
            root,
            projects,
            sel,
        });
    }

    pub(super) fn open_skill_panel_focused(&mut self, cloned_path: &std::path::Path) {
        let root = self.asset_directories.skill.clone();
        let skills = panels::skill::list_skill_assets(&root);
        let Some(sel) = skills
            .iter()
            .position(|skill| Self::path_is_within(&skill.path, cloned_path))
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  cloned source does not contain a recognized skill asset yet"),
            );
            return;
        };
        self.skill_picker = Some(panels::skill::SkillPanel { root, skills, sel });
    }

    pub(super) fn open_okf_package_panel_focused(&mut self, cloned_path: &std::path::Path) {
        let root = self.asset_directories.okf.clone();
        let packages = panels::okf::list_okf_packages(&root);
        let Some(sel) = packages
            .iter()
            .position(|package| Self::path_is_within(&package.path, cloned_path))
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  cloned source does not contain a recognized OKF package yet"),
            );
            return;
        };
        self.okf_picker = Some(panels::okf::OkfPackagePanel {
            root,
            packages,
            sel,
        });
    }

    pub(super) fn open_flow_panel_focused(&mut self, cloned_path: &std::path::Path) {
        let root = self.asset_directories.flow.clone();
        let flows = panels::flow::list_flows(&root);
        let Some(sel) = flows
            .iter()
            .position(|flow| Self::path_is_within(&root.join(flow), cloned_path))
        else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  cloned source does not contain a recognized workflow design yet"),
            );
            return;
        };
        self.flow = Some(panels::flow::FlowPanel { root, flows, sel });
    }

    pub(super) fn execute_agent_subcommand(
        &mut self,
        subcommand: panels::agent::AgentSubcommand,
    ) -> Option<Cmd<Msg>> {
        match subcommand {
            panels::agent::AgentSubcommand::Exit => {
                self.exit_agent_dev();
                None
            }
            panels::agent::AgentSubcommand::Clone(url) => {
                self.clone_asset_command("agent", url, self.asset_directories.agent.clone())
            }
            panels::agent::AgentSubcommand::List(query) => {
                self.open_asset_list_panel(os_asset_category_query("agent", &query))
            }
            panels::agent::AgentSubcommand::Activity(query) => {
                let Some(agent_dev) = self.agent_dev.clone() else {
                    self.pending_agent_subcommand =
                        Some(panels::agent::AgentSubcommand::Activity(query));
                    self.open_agent_panel();
                    return None;
                };
                self.open_runtime_activity_panel(runtime_asset_query(
                    "agent",
                    &agent_dev.name,
                    &query,
                ))
            }
            panels::agent::AgentSubcommand::Review => {
                let Some(agent_dev) = self.agent_dev.clone() else {
                    self.pending_agent_subcommand = Some(panels::agent::AgentSubcommand::Review);
                    self.open_agent_panel();
                    return None;
                };
                self.messages.push(TranscriptEntry::user("/agent review"));
                self.engage_autonomy(4);
                self.review_pending = true;
                let prompt = panels::agent::agent_review_prompt(&agent_dev);
                let display = format!("◇ {} review", agent_dev.name);
                self.start_stream_inner(prompt, display, true, true, false)
            }
            other => {
                let Some(agent_dev) = self.agent_dev.clone() else {
                    self.pending_agent_subcommand = Some(other);
                    self.open_agent_panel();
                    return None;
                };
                let Some(session) = self.os_session.clone() else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render("  /agent OS actions need /login first"),
                    );
                    return None;
                };
                let action = match other {
                    panels::agent::AgentSubcommand::Publish(kind) => {
                        panels::agent::AgentOsAction::Publish(kind)
                    }
                    panels::agent::AgentSubcommand::Run => {
                        panels::agent::AgentOsAction::Run(panels::agent::AgentOsKind::Agentic)
                    }
                    panels::agent::AgentSubcommand::Deploy => panels::agent::AgentOsAction::Deploy,
                    panels::agent::AgentSubcommand::Open(kind) => {
                        panels::agent::AgentOsAction::Open(kind)
                    }
                    panels::agent::AgentSubcommand::Logs(kind) => {
                        panels::agent::AgentOsAction::Logs(kind)
                    }
                    panels::agent::AgentSubcommand::Status(kind) => {
                        panels::agent::AgentOsAction::Status(kind)
                    }
                    panels::agent::AgentSubcommand::Exit
                    | panels::agent::AgentSubcommand::Clone(_)
                    | panels::agent::AgentSubcommand::List(_)
                    | panels::agent::AgentSubcommand::Activity(_)
                    | panels::agent::AgentSubcommand::Review => unreachable!(),
                };
                let kind = action.target_kind();
                let status_entry =
                    self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ◇ {} → OS {} {}…",
                        agent_dev.name,
                        kind.service_label(),
                        action.label()
                    )));
                Some(cmd::cmd(move || async move {
                    let result =
                        panels::agent::publish_agent_to_os(session, agent_dev, action).await;
                    Msg::AgentOsCompleted {
                        status_entry,
                        result,
                    }
                }))
            }
        }
    }

    pub(super) fn execute_mcp_subcommand(
        &mut self,
        subcommand: panels::mcp::McpSubcommand,
    ) -> Option<Cmd<Msg>> {
        match subcommand {
            panels::mcp::McpSubcommand::Exit => {
                self.exit_mcp_dev();
                None
            }
            panels::mcp::McpSubcommand::Clone(url) => {
                self.clone_asset_command("mcp", url, self.asset_directories.mcp.clone())
            }
            panels::mcp::McpSubcommand::List(query) => {
                self.open_asset_list_panel(os_asset_category_query("mcp", &query))
            }
            panels::mcp::McpSubcommand::Activity(query) => {
                let Some(mcp_dev) = self.mcp_dev.clone() else {
                    self.pending_mcp_subcommand = Some(panels::mcp::McpSubcommand::Activity(query));
                    self.open_mcp_panel();
                    return None;
                };
                self.open_runtime_activity_panel(runtime_asset_query("mcp", &mcp_dev.name, &query))
            }
            panels::mcp::McpSubcommand::Review => {
                let Some(mcp_dev) = self.mcp_dev.clone() else {
                    self.pending_mcp_subcommand = Some(panels::mcp::McpSubcommand::Review);
                    self.open_mcp_panel();
                    return None;
                };
                self.messages.push(TranscriptEntry::user("/mcp review"));
                self.engage_autonomy(4);
                self.review_pending = true;
                let prompt = panels::mcp::mcp_review_prompt(&mcp_dev);
                let display = format!("◆ {} review", mcp_dev.name);
                self.start_stream_inner(prompt, display, true, true, false)
            }
            other => {
                let Some(action) = other.os_action() else {
                    unreachable!("local MCP actions handled above")
                };
                let Some(mcp_dev) = self.mcp_dev.clone() else {
                    self.pending_mcp_subcommand = Some(other);
                    self.open_mcp_panel();
                    return None;
                };
                let Some(session) = self.os_session.clone() else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render("  /mcp OS actions need /login first"),
                    );
                    return None;
                };
                let status_entry =
                    self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ◆ {} → OS MCP Function as a Service {}…",
                        mcp_dev.name,
                        action.label()
                    )));
                Some(cmd::cmd(move || async move {
                    let result = panels::mcp::publish_mcp_to_os(session, mcp_dev, action).await;
                    Msg::McpOsCompleted {
                        status_entry,
                        result,
                    }
                }))
            }
        }
    }

    pub(super) fn execute_skill_subcommand(
        &mut self,
        subcommand: panels::skill::SkillSubcommand,
    ) -> Option<Cmd<Msg>> {
        match subcommand {
            panels::skill::SkillSubcommand::Exit => {
                self.exit_skill_dev();
                None
            }
            panels::skill::SkillSubcommand::Clone(url) => {
                self.clone_asset_command("skill", url, self.asset_directories.skill.clone())
            }
            panels::skill::SkillSubcommand::List(query) => {
                self.open_asset_list_panel(os_asset_category_query("skill", &query))
            }
            panels::skill::SkillSubcommand::Activity(query) => {
                let Some(skill_dev) = self.skill_dev.clone() else {
                    self.pending_skill_subcommand =
                        Some(panels::skill::SkillSubcommand::Activity(query));
                    self.open_skill_panel();
                    return None;
                };
                self.open_runtime_activity_panel(runtime_asset_query(
                    "skill",
                    &skill_dev.name,
                    &query,
                ))
            }
            panels::skill::SkillSubcommand::Review => {
                if self.skill_dev.is_none() {
                    self.pending_skill_subcommand = Some(panels::skill::SkillSubcommand::Review);
                    self.open_skill_panel();
                    return None;
                }
                let skill = self.skill_dev.clone().expect("checked above");
                let body = match std::fs::read_to_string(&skill.path) {
                    Ok(body) => body,
                    Err(error) => {
                        self.push_line(&Style::new().fg(TN_RED).render(&format!(
                            "  could not read {}: {error}",
                            skill.path.display()
                        )));
                        return None;
                    }
                };
                self.messages.push(TranscriptEntry::user("/skill review"));
                self.engage_autonomy(4);
                self.review_pending = true;
                let prompt = panels::skill::skill_review_prompt(&skill.path, &body);
                let display = format!("✦ {} review", skill.name);
                self.start_stream_inner(prompt, display, true, true, false)
            }
            panels::skill::SkillSubcommand::Publish => {
                self.execute_skill_os_action(panels::skill::SkillOsAction::Publish)
            }
            panels::skill::SkillSubcommand::Deploy => {
                if self.skill_dev.is_none() {
                    self.pending_skill_subcommand = Some(panels::skill::SkillSubcommand::Deploy);
                    self.open_skill_panel();
                    return None;
                }
                self.execute_skill_os_action(panels::skill::SkillOsAction::Deploy)
            }
            panels::skill::SkillSubcommand::Open => {
                self.execute_skill_os_action(panels::skill::SkillOsAction::Open)
            }
            panels::skill::SkillSubcommand::Status => {
                self.execute_skill_os_action(panels::skill::SkillOsAction::Status)
            }
        }
    }

    pub(super) fn execute_skill_os_action(
        &mut self,
        action: panels::skill::SkillOsAction,
    ) -> Option<Cmd<Msg>> {
        let Some(skill_dev) = self.skill_dev.clone() else {
            self.pending_skill_subcommand = Some(match action {
                panels::skill::SkillOsAction::Publish => panels::skill::SkillSubcommand::Publish,
                panels::skill::SkillOsAction::Deploy => panels::skill::SkillSubcommand::Deploy,
                panels::skill::SkillOsAction::Open => panels::skill::SkillSubcommand::Open,
                panels::skill::SkillOsAction::Status => panels::skill::SkillSubcommand::Status,
            });
            self.open_skill_panel();
            return None;
        };
        let Some(session) = self.os_session.clone() else {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  /skill OS actions need /login first"),
            );
            return None;
        };
        let status_entry = self.push_tracked_line(&Style::new().fg(TN_GRAY).render(&format!(
            "  ✦ {} → OS skill Function as a Service {}…",
            skill_dev.name,
            action.label()
        )));
        Some(cmd::cmd(move || async move {
            let result = panels::skill::publish_skill_to_os(session, skill_dev, action).await;
            Msg::SkillOsCompleted {
                status_entry,
                result,
            }
        }))
    }
}

pub(super) fn goal_status_chip(since: Option<Instant>) -> SessionStatusChip {
    let label = since
        .map(|started| format!("goal · {}", fmt_elapsed(started.elapsed())))
        .unwrap_or_else(|| "goal".to_string());
    SessionStatusChip::new("◎", label).color(COMPOSER_CHROME.active)
}
