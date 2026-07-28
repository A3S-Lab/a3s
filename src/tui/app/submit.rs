//! Prompt submission, slash-command dispatch, and attachment handling.

use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SubmissionIntent {
    Queue,
    SendNow,
}

pub(super) fn parse_use_status_command(rest: &str) -> Result<bool, &'static str> {
    match rest.trim() {
        "" | "status" => Ok(false),
        "repair" => Ok(true),
        _ => Err("usage: /use [status|repair]"),
    }
}

impl App {
    pub(super) fn on_submit(&mut self, text: String) -> Option<Cmd<Msg>> {
        self.on_submit_with_intent(text, SubmissionIntent::Queue)
    }

    pub(super) fn on_submit_now(&mut self, text: String) -> Option<Cmd<Msg>> {
        let trimmed = text.trim();
        if self.shell_mode
            || self.research_mode
            || matches!(trimmed.chars().next(), Some('/' | '!' | '?'))
        {
            self.textarea.set_value(&text);
            self.push_notice(
                NoticeKind::Warning,
                "Send now accepts an agent prompt, not a shell, research, or slash command",
            );
            return None;
        }
        self.on_submit_with_intent(text, SubmissionIntent::SendNow)
    }

    fn on_submit_with_intent(
        &mut self,
        text: String,
        intent: SubmissionIntent,
    ) -> Option<Cmd<Msg>> {
        let trimmed = text.trim();
        if trimmed.is_empty() && self.pending_images.is_empty() {
            return None;
        }
        // No input while compacting or upgrading.
        if self.compacting.is_some() || self.updating.is_some() {
            self.textarea.clear();
            return None;
        }
        // `/checkup` owns a short host-side inspection before it admits the
        // strict Plan turn. Keep any asynchronously-routed prompt as a draft
        // instead of letting an ordinary turn race that immutable snapshot.
        if self.checkup_inflight {
            self.textarea.set_value(&text);
            return None;
        }
        if self.session_rebuild_pending.is_some() {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  wait for the current session change to finish"),
            );
            return None;
        }
        // Shell mode (`!`) runs a shell command directly (not through the agent).
        if self.shell_mode {
            self.shell_mode = false;
            let cmd = trimmed.trim_start_matches('!').trim().to_string();
            if cmd.is_empty() {
                return None;
            }
            self.messages.push(TranscriptEntry::preformatted(gutter(
                TN_GRAY,
                &Style::new().bold().render(&format!("! {cmd}")),
            )));
            self.textarea.clear();
            self.rebuild_viewport();
            return Some(cmd::cmd(move || async move {
                let out = tokio::process::Command::new("sh")
                    .arg("-c")
                    .arg(&cmd)
                    .output()
                    .await;
                let text = match out {
                    Ok(o) => {
                        let mut s = String::from_utf8_lossy(&o.stdout).into_owned();
                        s.push_str(&String::from_utf8_lossy(&o.stderr));
                        if s.trim().is_empty() {
                            format!("(exit {})", o.status.code().unwrap_or(-1))
                        } else {
                            s
                        }
                    }
                    Err(e) => format!("failed to run: {e}"),
                };
                Msg::ShellOutput(text)
            }));
        }
        // Deep-research mode (`?`) is host-orchestrated for stability. One LLM
        // call defines the semantic research contract and the exact provider
        // queries for one bounded retrieval pass. Rust never routes free-form
        // query text through lexical rules.
        if self.research_mode || trimmed.starts_with('?') {
            self.research_mode = false;
            let raw_query = trimmed.trim_start_matches('?').trim();
            let (query, evidence_scope) = parse_deep_research_tui_query(raw_query);
            if query.is_empty() {
                self.textarea.clear();
                return None;
            }
            self.history.push(format!("? {query}"));
            self.history_pos = None;
            self.history_draft = None;
            self.textarea.clear();
            self.messages.push(TranscriptEntry::preformatted(gutter(
                TN_CYAN,
                &Style::new()
                    .bold()
                    .render(&format!("✦\u{200A}deep research: {query}")),
            )));
            let evidence_scope_label = evidence_scope.label();
            let runtime_hint = if self.os_session.is_some() {
                format!(
                    "  ◎\u{200A}goal set · semantic plan · one evidence pass · {evidence_scope_label} · closed-evidence review · local HTML opens in RemoteUI (Esc stops)"
                )
            } else {
                format!(
                    "  ◎\u{200A}goal set · semantic plan · one evidence pass · {evidence_scope_label} · closed-evidence review · report + HTML opens in RemoteUI (Esc stops)"
                )
            };
            self.push_line(&Style::new().fg(TN_GRAY).render(&runtime_hint));
            let display = format!("✦\u{200A}{query}");
            // The planner defines the semantic contract and provider queries;
            // the host supplies finite caps and one report finalization path.
            let runtime_expectation = Some(RuntimeExpectation::required("deep research"));
            let execution_mode = self.mode;
            self.enqueue_turn(
                USER_TURN_PRIORITY,
                Queued {
                    text: format!("? {query}"),
                    display,
                    images: Vec::new(),
                    runtime_expectation,
                    deep_research: Some((query, evidence_scope)),
                },
                execution_mode,
            );
            if self.state == State::Idle {
                return self.drain_queue();
            }
            // The bottom queue projection is the only owner of pending-turn
            // status. A transcript entry would outlive the queue item after it
            // is claimed and make an already-running turn look pending.
            self.relayout();
            return None;
        }
        // `/goal clear` is intentionally available during a running goal. It
        // invalidates delayed retries immediately, then cancels and joins the
        // active stream before restoring normal Ultracode planning.
        if trimmed == "/goal clear" {
            return self.clear_goal_command();
        }
        // Block session-mutating commands while a turn is streaming.
        if self.state != State::Idle {
            let cmd0 = trimmed.split_whitespace().next().unwrap_or("");
            if IDLE_ONLY.contains(&cmd0) {
                self.textarea.clear();
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                    "  {cmd0} is unavailable while a turn is running — press Esc to stop first"
                )));
                return None;
            }
        }
        if let Some(rest) = slash_tail(trimmed, "/use") {
            self.textarea.clear();
            let include_repair_guidance = match parse_use_status_command(rest) {
                Ok(include_repair_guidance) => include_repair_guidance,
                Err(usage) => {
                    self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {usage}")));
                    return None;
                }
            };
            let status_entry = self.push_tracked_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  inspecting A3S Use capabilities…"),
            );
            let Some(registry) = self.use_registry.clone() else {
                let status = crate::use_registry::unavailable_status_text(include_repair_guidance);
                self.replace_tracked_line(status_entry, &gutter(TN_YELLOW, &status));
                return None;
            };
            let session = Arc::clone(&self.session);
            return Some(cmd::cmd(move || async move {
                let text = registry.status_text(session, include_repair_guidance).await;
                Msg::UseStatus { status_entry, text }
            }));
        }
        if let Some(rest) = slash_tail(trimmed, "/island") {
            return self.submit_agent_island_command(rest);
        }
        if let Some(rest) = slash_tail(trimmed, "/login") {
            self.textarea.clear();
            let Some(os_config) = self.os_config.clone() else {
                self.push_line(&format!(
                    "{}\n{}\n{}\n{}",
                    Style::new()
                        .fg(TN_YELLOW)
                        .render("  /login needs an OS endpoint, but none is configured."),
                    Style::new().fg(TN_GRAY).render(
                        "  Add it to ~/.a3s/config.acl (or your project's .a3s/config.acl):"
                    ),
                    Style::new()
                        .fg(TN_CYAN)
                        .render("      os = \"https://your-os-host.example.com\""),
                    Style::new()
                        .fg(TN_GRAY)
                        .render("  then restart a3s code and run /login again."),
                ));
                return None;
            };
            let token = rest.trim();
            if !token.is_empty() {
                let token = token.to_string();
                let status_entry =
                    self.push_tracked_line(&Style::new().fg(TN_GRAY).render("  signing in to OS…"));
                return Some(cmd::cmd(move || async move {
                    let result = crate::a3s_os::login_with_token(&os_config, &token)
                        .await
                        .map(|session| session.display_label())
                        .map_err(|error| error.to_string());
                    Msg::OsLogin {
                        status_entry,
                        result,
                    }
                }));
            }

            // Already signed in (restored from a previous run) → no need to
            // re-authenticate; tell the user how to switch instead.
            if let Some(s) = &self.os_session {
                self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                    "  already signed in to OS as {} · /logout to switch accounts",
                    s.display_label()
                )));
                return None;
            }

            let status_entry = self.push_tracked_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  opening OS login in your browser…"),
            );
            return Some(cmd::cmd(move || async move {
                let result = crate::a3s_os::login_via_browser(os_config)
                    .await
                    .map(|session| session.display_label())
                    .map_err(|error| error.to_string());
                Msg::OsLogin {
                    status_entry,
                    result,
                }
            }));
        }
        if trimmed == "/logout" {
            self.textarea.clear();
            let Some(os_config) = self.os_config.clone() else {
                self.push_line(&Style::new().fg(TN_YELLOW).render(
                    "  configure `os = \"https://...\"` in .a3s/config.acl to enable /logout",
                ));
                return None;
            };
            match crate::a3s_os::logout(&os_config) {
                Ok(true) => {
                    self.os_session = None;
                    self.asset_list = None;
                    self.runtime_activity = None;
                    crate::a3s_os::remove_capability_skill_dir();
                    crate::a3s_os::clear_os_env();
                    let rebuild = self.refresh_after_auth();
                    self.push_line(
                        &Style::new()
                            .fg(TN_GREEN)
                            .render("  ✓ signed out from OS · capabilities skill removed"),
                    );
                    return rebuild;
                }
                Ok(false) => {
                    self.os_session = None;
                    self.asset_list = None;
                    self.runtime_activity = None;
                    crate::a3s_os::remove_capability_skill_dir();
                    crate::a3s_os::clear_os_env();
                    let rebuild = self.refresh_after_auth();
                    self.push_line(&Style::new().fg(TN_GRAY).render("  no OS login was stored"));
                    return rebuild;
                }
                Err(error) => self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  logout failed: {error}")),
                ),
            }
            return None;
        }
        // `/kb` opens the local personal knowledge-base panel. Notes/imports/search are
        // explicit subcommands so a mistyped path no longer becomes a note.
        // `/ctx <query>` searches past agent sessions; `/ctx <n>` stages hit n
        // as context for the next message (ctx CLI, local SQLite index).
        if let Some(rest) = slash_tail(trimmed, "/research") {
            self.textarea.clear();
            let mut parts = rest.split_whitespace();
            let action = parts.next().unwrap_or("status");
            if action == "diff" {
                let left = parts.next().map(str::to_string);
                let right = parts.next().map(str::to_string);
                if left.is_none() || right.is_none() || parts.next().is_some() {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  usage: /research diff <left-run-id> <right-run-id>"),
                    );
                    return None;
                }
                let (Some(left), Some(right)) = (left, right) else {
                    return None;
                };
                let workspace = PathBuf::from(&self.cwd);
                return Some(cmd::cmd(move || async move {
                    Msg::ResearchDiagnostic(
                        research_diff(&workspace, &left, &right)
                            .await
                            .map_err(|error| error.to_string()),
                    )
                }));
            }
            let explicit_run_id = parts.next().map(str::to_string);
            if parts.next().is_some() {
                self.push_line(&Style::new().fg(TN_GRAY).render(
                    "  usage: /research [status|explain|replay] [run-id] · /research diff <left> <right>",
                ));
                return None;
            }
            let kind = match action {
                "status" => ResearchDiagnosticKind::Status,
                "explain" => ResearchDiagnosticKind::Explain,
                "replay" => ResearchDiagnosticKind::Replay,
                _ => {
                    self.push_line(&Style::new().fg(TN_GRAY).render(
                        "  usage: /research [status|explain|replay] [run-id] · /research diff <left> <right>",
                    ));
                    return None;
                }
            };
            let active_run_id = self
                .deep_research_workflow
                .args
                .as_ref()
                .and_then(|args| args.get("run_id"))
                .and_then(serde_json::Value::as_str)
                .map(str::to_string);
            let run_id = explicit_run_id.or(active_run_id);
            let workspace = PathBuf::from(&self.cwd);
            return Some(cmd::cmd(move || async move {
                Msg::ResearchDiagnostic(
                    research_diagnostic(&workspace, run_id.as_deref(), kind)
                        .await
                        .map_err(|error| error.to_string()),
                )
            }));
        }
        if let Some(rest) = slash_tail(trimmed, "/ctx") {
            return self.handle_ctx_command(rest);
        }
        if let Some(rest) = slash_tail(trimmed, "/okf") {
            return self.handle_okf_command(rest);
        }
        if let Some(rest) = slash_tail(trimmed, "/kb") {
            return self.handle_kb_command(rest);
        }
        // `/goal [text|resume|clear]` — a persistent goal prepended to every prompt.
        if let Some(rest) = slash_tail(trimmed, "/goal") {
            let g = rest.trim();
            self.textarea.clear();
            if g.is_empty() {
                match &self.goal {
                    Some(cur) => self.push_line(&gutter(
                        TN_CYAN,
                        &format!("◎\u{200A}goal: {cur}   (/goal clear to remove)"),
                    )),
                    None => match &self.paused_goal {
                        Some(paused) => self.push_line(&gutter(
                            TN_YELLOW,
                            &format!(
                                "◎\u{200A}goal paused: {}   (/goal resume or /goal clear)",
                                paused.goal
                            ),
                        )),
                        None => self.push_line(
                            &Style::new()
                                .fg(TN_GRAY)
                                .render("  usage: /goal <what you're working toward>"),
                        ),
                    },
                }
            } else if g == "clear" {
                return self.clear_goal_command();
            } else if g == "resume" {
                if self.paused_goal.is_none() {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GRAY)
                            .render("  no paused goal to resume"),
                    );
                    return None;
                }
                return self.resume_paused_goal();
            } else {
                return self.start_goal_run(g);
            }
            return None;
        }
        // `/loop` — engineered loop dashboard + subcommands; unknown tails keep
        // the quick-loop contract (`/loop <task>`).
        if let Some(rest) = slash_tail(trimmed, "/loop") {
            return self.handle_loop_command(rest);
        }
        // `/sleep [focus]` — end-of-day consolidation: the `/loop` mechanism
        // drives the agent through reviewing today's work (cross-session via
        // `ctx` when installed) until a turn ends with the machine-readable
        // ```a3s-sleep report, which capture_sleep persists into long-term
        // memory (experience · preferences · knowledge). Idle-only.
        if let Some(rest) = slash_tail(trimmed, "/sleep") {
            let focus = rest.trim().to_string();
            self.textarea.clear();
            self.sleep_pending = true;
            self.engage_autonomy(8);
            self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  ☾ sleep — consolidating today's work into memory… (Esc stops)"),
            );
            let directive = panels::sleep::sleep_directive(
                &focus,
                self.ctx_ready,
                &panels::sleep::sleep_today(),
            );
            // Like asset reviews: send the directive but show a short display
            // line (echoing the boilerplate as a user message is just noise).
            let display = if focus.is_empty() {
                "☾ sleep".to_string()
            } else {
                format!("☾ sleep · {focus}")
            };
            return self.start_stream_inner(directive, display, true, true, false);
        }
        // `/flow` — select a local DAG JSON and open it in the OS workflow
        // designer (login-gated); `/flow <description>` orchestrates a basic DAG into
        // the flows folder (local, no login needed). Token-boundary filtered
        // so "/flowx" stays a normal message and can't bypass the idle gate.
        if let Some(rest) = slash_tail(trimmed, "/flow") {
            let description = rest.trim().to_string();
            self.textarea.clear();
            if let Some(parsed) = panels::flow::parse_flow_subcommand(&description) {
                match parsed {
                    Ok(panels::flow::FlowSubcommand::Clone(url)) => {
                        return self.clone_asset_command(
                            "workflow",
                            url,
                            self.asset_directories.flow.clone(),
                        );
                    }
                    Ok(panels::flow::FlowSubcommand::List(query)) => {
                        return self
                            .open_asset_list_panel(os_asset_category_query("workflow", &query));
                    }
                    Ok(panels::flow::FlowSubcommand::Activity(query)) => {
                        if self.os_session.is_none() {
                            self.push_line(&os_required_alert(
                                "workflow runtime activity",
                                self.os_config.is_some(),
                            ));
                        } else {
                            self.pending_flow_subcommand =
                                Some(panels::flow::FlowSubcommand::Activity(query));
                            self.open_flow_panel();
                        }
                        return None;
                    }
                    Ok(panels::flow::FlowSubcommand::Review(target)) => {
                        let root = self.asset_directories.flow.clone();
                        let flows = panels::flow::list_flows(&root);
                        let picked = match target {
                            Some(target) => flows
                                .into_iter()
                                .find(|flow| flow == &target || flow.ends_with(&target)),
                            None if flows.len() == 1 => flows.into_iter().next(),
                            None => None,
                        };
                        let Some(file) = picked else {
                            self.pending_flow_subcommand =
                                Some(panels::flow::FlowSubcommand::Review(None));
                            self.open_flow_panel();
                            return None;
                        };
                        let path = root.join(&file);
                        let design = match std::fs::read_to_string(&path) {
                            Ok(value) => value,
                            Err(error) => {
                                self.push_line(&Style::new().fg(TN_RED).render(&format!(
                                    "  could not read {}: {error}",
                                    path.display()
                                )));
                                return None;
                            }
                        };
                        if serde_json::from_str::<serde_json::Value>(&design).is_err() {
                            self.push_line(
                                &Style::new()
                                    .fg(TN_RED)
                                    .render(&format!("  {} is not valid JSON", file)),
                            );
                            return None;
                        }
                        self.messages
                            .push(TranscriptEntry::user(format!("/flow review {file}")));
                        self.engage_autonomy(4);
                        self.review_pending = true;
                        let prompt = panels::flow::flow_review_prompt(&path, &design);
                        let display = format!("⧉ flow review: {}", truncate(&file, 48));
                        return self.start_stream_inner(prompt, display, true, true, false);
                    }
                    Ok(action @ panels::flow::FlowSubcommand::Publish)
                    | Ok(action @ panels::flow::FlowSubcommand::Run)
                    | Ok(action @ panels::flow::FlowSubcommand::Deploy)
                    | Ok(action @ panels::flow::FlowSubcommand::Open)
                    | Ok(action @ panels::flow::FlowSubcommand::Logs)
                    | Ok(action @ panels::flow::FlowSubcommand::Status) => {
                        if self.os_session.is_none() {
                            self.push_line(
                                &Style::new().fg(TN_YELLOW).render(
                                    "  /flow publish/run/deploy/open/logs/status needs OS — sign in with /login first",
                                ),
                            );
                        } else {
                            self.pending_flow_subcommand = Some(action);
                            self.open_flow_panel();
                        }
                        return None;
                    }
                    Err(e) => {
                        self.push_line(&Style::new().fg(TN_RED).render(&format!("  {e}")));
                        return None;
                    }
                }
            }
            if description.is_empty() {
                if self.os_session.is_none() {
                    self.push_line(
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render("  /flow needs OS — sign in with /login first"),
                    );
                } else {
                    self.open_flow_panel();
                }
                return None;
            }
            let dir = self.asset_directories.flow.clone();
            match panels::flow::scaffold_flow_asset(&description, &dir) {
                Ok(path) => {
                    self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ⧉ scaffolded workflow asset → {}",
                        path.parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .display()
                    )));
                    self.open_flow_panel_focused(&path);
                    return None;
                }
                Err(e) => {
                    self.push_line(
                        &Style::new()
                            .fg(TN_RED)
                            .render(&format!("  /flow scaffold failed: {e}")),
                    );
                    return None;
                }
            }
        }
        // `/agent` — select a local a3s-code agent package and enter local
        // multi-turn development mode; `/agent <description>` scaffolds a complete
        // local A3S Code agent package; OS subcommands publish/run/deploy the
        // active local definition through Agent as a Service or Function as a
        // Service according to the kind.
        if let Some(rest) = slash_tail(trimmed, "/agent") {
            let description = rest.trim().to_string();
            self.textarea.clear();
            if let Some(parsed) = panels::agent::parse_agent_subcommand(&description) {
                return match parsed {
                    Ok(subcommand) => self.execute_agent_subcommand(subcommand),
                    Err(e) => {
                        self.push_line(&Style::new().fg(TN_RED).render(&format!("  {e}")));
                        None
                    }
                };
            }
            if description.is_empty() {
                self.open_agent_panel();
                return None;
            }
            let dir = self.asset_directories.agent.clone();
            match panels::agent::scaffold_agent_package(&description, &dir) {
                Ok(dev) => {
                    self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ◇ scaffolded complete agent package → {}",
                        dev.package_path.display()
                    )));
                    return self.activate_agent_package_path(&dev.package_path);
                }
                Err(e) => {
                    self.push_line(
                        &Style::new()
                            .fg(TN_RED)
                            .render(&format!("  /agent scaffold failed: {e}")),
                    );
                    return None;
                }
            }
        }
        // `/mcp` — select a local MCP server asset and enter local multi-turn
        // development mode; `/mcp <description>` drafts a local MCP asset.
        // OS publish/run/test will map MCP tool calls to Function as a Service.
        if let Some(rest) = slash_tail(trimmed, "/mcp") {
            let description = rest.trim().to_string();
            self.textarea.clear();
            if let Some(parsed) = panels::mcp::parse_mcp_subcommand(&description) {
                return match parsed {
                    Ok(subcommand) => self.execute_mcp_subcommand(subcommand),
                    Err(e) => {
                        self.push_line(&Style::new().fg(TN_RED).render(&format!("  {e}")));
                        None
                    }
                };
            }
            if description.is_empty() {
                self.open_mcp_panel();
                return None;
            }
            let dir = self.asset_directories.mcp.clone();
            match panels::mcp::scaffold_mcp_project(&description, &dir) {
                Ok(dev) => {
                    self.agent_dev = None;
                    self.skill_dev = None;
                    self.okf_dev = None;
                    self.mcp_dev = Some(dev.clone());
                    self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ◆ scaffolded MCP asset → {}",
                        dev.path.display()
                    )));
                    self.push_line(&gutter(
                        TN_CYAN,
                        &format!(
                            "◆ mcp dev: {} ({}) · Esc or /mcp off returns to normal mode",
                            dev.name, dev.rel
                        ),
                    ));
                    self.relayout();
                    return None;
                }
                Err(e) => {
                    self.push_line(
                        &Style::new()
                            .fg(TN_RED)
                            .render(&format!("  /mcp scaffold failed: {e}")),
                    );
                    return None;
                }
            }
        }
        // `/skill` — select a local skill asset and enter local multi-turn
        // development mode; `/skill <description>` drafts a local skill asset.
        if let Some(rest) = slash_tail(trimmed, "/skill") {
            let description = rest.trim().to_string();
            self.textarea.clear();
            if let Some(parsed) = panels::skill::parse_skill_subcommand(&description) {
                return match parsed {
                    Ok(subcommand) => self.execute_skill_subcommand(subcommand),
                    Err(e) => {
                        self.push_line(&Style::new().fg(TN_RED).render(&format!("  {e}")));
                        None
                    }
                };
            }
            if description.is_empty() {
                self.open_skill_panel();
                return None;
            }
            let dir = self.asset_directories.skill.clone();
            match panels::skill::scaffold_skill_asset(&description, &dir) {
                Ok(dev) => {
                    self.agent_dev = None;
                    self.mcp_dev = None;
                    self.okf_dev = None;
                    self.skill_dev = Some(dev.clone());
                    self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                        "  ✦ scaffolded skill asset → {}",
                        dev.path
                            .parent()
                            .unwrap_or_else(|| std::path::Path::new("."))
                            .display()
                    )));
                    self.push_line(&gutter(
                        TN_CYAN,
                        &format!(
                            "✦ skill dev: {} ({}) · Esc or /skill off returns to normal mode",
                            dev.name, dev.rel
                        ),
                    ));
                    self.relayout();
                    return None;
                }
                Err(e) => {
                    self.push_line(
                        &Style::new()
                            .fg(TN_RED)
                            .render(&format!("  /skill scaffold failed: {e}")),
                    );
                    return None;
                }
            }
        }
        if let Some(rest) = slash_tail(trimmed, "/preview") {
            return self.submit_live_preview_command(rest);
        }
        if let Some(rest) = slash_tail(trimmed, "/fork") {
            return self.submit_fork_command(rest);
        }
        if let Some(rest) = slash_tail(trimmed, "/copy") {
            return self.submit_copy_command(rest);
        }
        if let Some(rest) = slash_tail(trimmed, "/export") {
            return self.submit_export_command(rest);
        }
        // Slash commands run inline in any state.
        match trimmed {
            "/exit" => return self.begin_graceful_quit(),
            "/rewind" => return self.submit_rewind_command(),
            "/clear" => {
                self.textarea.clear();
                self.cancel_goal_state("cleared by /clear");
                self.clear_paused_goal("cleared by /clear");
                self.goal = None;
                self.goal_since = None;
                // Actually reset the conversation, not just the screen: swap in a
                // fresh session (new id, no history, no carried compact summary)
                // and zero the token/ctx counters. All visible state is committed
                // only by SessionRebuilt after construction succeeds, so a failed
                // clear leaves the current transcript and active modes intact.
                let session_id = new_session_id();
                let mut profile = self.session_rebuild_profile();
                profile.session_id = session_id.clone();
                profile.compact_summary = None;
                return self
                    .start_session_rebuild(profile, SessionRebuildAction::Clear { session_id });
            }
            "/init" => {
                // Agent-driven: analyze the workspace and write AGENTS.md (auto-loaded
                // by the core, like CLAUDE.md). Guarded idle by IDLE_ONLY above.
                self.textarea.clear();
                self.messages
                    .push(TranscriptEntry::user("/init — generate AGENTS.md"));
                self.rebuild_viewport();
                return self.start_stream(
                    "Analyze this codebase and create (or update) an AGENTS.md file at the \
                     project root. Include: a concise project overview, the exact build / test / \
                     lint / run commands, the high-level architecture and key directories, and \
                     the conventions an AI coding agent should follow. Base everything on what's \
                     actually in the workspace, and write the file with your file-writing tool."
                        .to_string(),
                );
            }
            "/compact" => {
                self.textarea.clear();
                if self.state != State::Idle {
                    self.push_line(
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render("  finish the current turn before compacting"),
                    );
                    return None;
                }
                let history = self.session.history();
                if history.is_empty() {
                    self.push_line(&Style::new().fg(TN_GRAY).render("  nothing to compact yet"));
                    return None;
                }
                let llm_client = match crate::session_llm::resolve_session_llm_client(
                    &self.code_config,
                    &self.effort_session_opts(false),
                    &self.session_id,
                ) {
                    Ok(client) => client,
                    Err(error) => {
                        self.push_line(
                            &Style::new()
                                .fg(TN_RED)
                                .render(&format!("  could not prepare compaction: {error}")),
                        );
                        return None;
                    }
                };
                self.compacting = Some(Instant::now()); // progress bar + input lock
                let previous_summary = self.compact_summary.clone();
                return Some(cmd::cmd(move || async move {
                    Msg::Compacted(
                        crate::compact::compact_history(
                            llm_client,
                            &history,
                            previous_summary.as_deref(),
                        )
                        .await,
                    )
                }));
            }
            "/help" => {
                self.textarea.clear();
                self.help_open = true;
                self.help_scroll = 0;
                return None;
            }
            "/terminal" => {
                self.textarea.clear();
                let diagnostic =
                    panels::terminal::current_terminal_diagnostic(self.viewport_content_width());
                self.push_line(&diagnostic);
                return None;
            }
            "/checkup" => return self.submit_checkup_command(),
            "/queue" => {
                self.textarea.clear();
                self.open_queue_panel();
                return None;
            }
            "/history" => {
                self.textarea.clear();
                self.open_history_panel("");
                return None;
            }
            "/permissions" => {
                self.textarea.clear();
                self.open_permission_panel();
                return None;
            }
            "/auto" => {
                self.set_composer_mode(Mode::Auto);
                self.textarea.clear();
                self.rebuild_viewport();
                return None;
            }
            "/config" => {
                self.textarea.clear();
                let path = self.config_path.clone();
                self.open_config_in_ide(&path);
                return None;
            }
            "/model" => {
                self.textarea.clear();
                self.open_model_menu();
                let mut commands = Vec::new();
                if let Some(command) = self.maybe_refresh_codex_models() {
                    commands.push(command);
                }
                if let Some(command) = self.maybe_fetch_active_model_models() {
                    commands.push(command);
                }
                return match commands.len() {
                    0 => None,
                    1 => commands.pop(),
                    _ => Some(cmd::batch(commands)),
                };
            }
            "/effort" => {
                self.textarea.clear();
                self.effort_panel = Some(self.effort);
                return None;
            }
            "/ide" => {
                self.textarea.clear();
                let entries = ide_children(std::path::Path::new(&self.cwd), 0);
                self.ide = Some(Ide::workspace(entries));
                return None;
            }
            "/plugin" => {
                self.textarea.clear();
                if self.skills.is_empty() {
                    self.push_line(&Style::new().fg(TN_GRAY).render(
                        "  no skills/plugins found (~/.claude/skills, ~/.codex/skills, ~/.claude/plugins)",
                    ));
                } else {
                    self.plugins_panel = Some(0);
                }
                return None;
            }
            "/theme" => {
                self.textarea.clear();
                let cur = SYNTAX_THEME.load(std::sync::atomic::Ordering::Relaxed);
                self.theme_panel = Some(cur.min(THEMES.len() - 1));
                return None;
            }
            "/reload" => {
                self.textarea.clear();
                // Hot-reload: re-discover skill dirs, refresh the UI catalog,
                // and rebuild the session so the core skill registry and
                // next Claude/system prompt see the same skills.
                let dirs =
                    agent_skill_dirs_with_configured(&self.cwd, &self.asset_directories.skill);
                self.skills = load_skills(&dirs);
                self.skill_count = count_skill_files(&dirs);
                let profile = self.session_rebuild_profile();
                return self.start_session_rebuild(
                    profile,
                    SessionRebuildAction::Reload {
                        skill_count: self.skills.len(),
                    },
                );
            }
            "/update" => {
                self.textarea.clear();
                self.updating = Some(Instant::now()); // "checking…" + input lock
                self.relayout();
                return Some(cmd::cmd(|| async {
                    // Quick version check only; the actual upgrade runs in the
                    // shell after the TUI exits (run()), so brew's/curl's own
                    // progress shows and the restart picks up the new binary.
                    let latest = crate::update::fetch_latest_async().await;
                    Msg::UpdatePlan(latest)
                }));
            }
            "/tasks" => {
                self.textarea.clear();
                return self.open_task_panel();
            }
            "/relay" => return self.open_relay_panel(),
            "/memory" => {
                self.textarea.clear();
                // Open immediately ("loading…"); load the file snapshot off the
                // UI thread, with live session memory as a fallback.
                let dir = self.memory_dir.clone();
                self.memory = Some(MemPanel {
                    entries: Vec::new(),
                    sel: 0,
                    details: std::collections::BTreeMap::new(),
                    graph: MemoryGraph::default(),
                    loaded_from_session: false,
                    detail: memutil::MemDetail::default(),
                    detail_scroll: 0,
                    dir: dir.clone(),
                    note: "loading…".into(),
                });
                return Some(self.load_memory_panel(dir));
            }
            "/evolution" => {
                self.textarea.clear();
                self.evolution = Some(panels::evolution::EvolutionPanel::loading());
                return Some(self.load_evolution_panel());
            }
            _ => {}
        }

        if !trimmed.is_empty() {
            self.history.push(trimmed.to_string());
        }
        self.history_pos = None;
        self.history_draft = None;
        // Composer chips disappear on submit, while compact textual references
        // remain in the user bubble just like Codex's `[Image #n]` markers.
        let image_references = attachment_reference_line(&self.pending_images);
        let user_display = match (image_references.is_empty(), trimmed.is_empty()) {
            (true, _) => trimmed.to_string(),
            (false, true) => image_references.clone(),
            (false, false) => format!("{image_references}\n{trimmed}"),
        };
        self.messages.push(TranscriptEntry::user(user_display));
        self.textarea.clear();
        // One-shot `/ctx <n>` context: attach the staged transcript to THIS
        // genuine typed message only (never a `/loop` "Continue." re-entry),
        // invisibly — the display bubble above stays clean. Travels with the
        // message whether it runs now or is queued.
        let loop_cont = std::mem::take(&mut self.loop_continuation);
        if !loop_cont {
            if let Some(run) = self.goal_run.as_mut() {
                run.pause_achievement_for_user_turn();
            }
        }
        let typed_prompt = if trimmed.is_empty() {
            "Please inspect the attached image or images.".to_string()
        } else {
            trimmed.to_string()
        };
        let task_label = if trimmed.is_empty() {
            image_references
        } else {
            trimmed.to_string()
        };
        let prompt = match (loop_cont, self.pending_ctx.take()) {
            (false, Some(c)) => format!("{c}\n\n{typed_prompt}"),
            _ => typed_prompt,
        };
        let (prompt, display) = match &self.agent_dev {
            Some(dev) => (
                panels::agent::agent_dev_prompt(dev, &prompt),
                format!("◇ {}: {}", dev.name, truncate(&task_label, 60)),
            ),
            None => match &self.mcp_dev {
                Some(dev) => (
                    panels::mcp::mcp_dev_prompt(dev, &prompt),
                    format!("◆ {}: {}", dev.name, truncate(&task_label, 60)),
                ),
                None => match &self.skill_dev {
                    Some(dev) => (
                        panels::skill::skill_dev_prompt(dev, &prompt),
                        format!("✦ {}: {}", dev.name, truncate(&task_label, 60)),
                    ),
                    None => match &self.okf_dev {
                        Some(dev) => (
                            panels::okf::okf_dev_prompt(dev, &prompt),
                            format!("⌁ {}: {}", dev.name, truncate(&task_label, 60)),
                        ),
                        None => (prompt, task_label),
                    },
                },
            },
        };
        let send_now = intent == SubmissionIntent::SendNow && self.state == State::Streaming;
        let priority = if send_now {
            PLAN_REVIEW_PRIORITY
        } else if loop_cont {
            SYNTHETIC_TURN_PRIORITY
        } else {
            USER_TURN_PRIORITY
        };
        let execution_mode = self.mode;
        let images = std::mem::take(&mut self.pending_images);
        let sequence = if execution_mode == Mode::Plan && !loop_cont {
            let request = PlanDraftRequest::initial(prompt, display.clone());
            self.enqueue_plan_turn(
                priority,
                Queued {
                    text: request.planning_prompt(),
                    display,
                    images,
                    runtime_expectation: None,
                    deep_research: None,
                },
                request,
            )
        } else {
            self.enqueue_turn(
                priority,
                Queued {
                    text: prompt,
                    display,
                    images,
                    runtime_expectation: None,
                    deep_research: None,
                },
                execution_mode,
            )
        };
        if send_now {
            self.send_now_queued_sequence = Some(sequence);
            return self.begin_send_now_interrupt();
        }
        if self.state == State::Idle {
            self.drain_queue()
        } else {
            // Keep this transient state out of the durable transcript. The
            // queue panel disappears as soon as drain_queue claims the turn.
            self.relayout();
            None
        }
    }

    /// Grab a clipboard image and add an interactive chip to the composer.
    /// This method only stages the image; Enter remains the sole send action.
    pub(super) fn paste_clipboard_image(&mut self) {
        match PendingImage::from_clipboard() {
            Ok(image) => {
                self.pending_images.push(image);
                self.relayout();
            }
            Err(error) => self.push_notice(
                NoticeKind::Warning,
                format!("Clipboard image unavailable: {error}"),
            ),
        }
    }

    pub(super) fn start_stream(&mut self, prompt: String) -> Option<Cmd<Msg>> {
        self.start_stream_inner(prompt.clone(), prompt, true, true, false)
    }
}
