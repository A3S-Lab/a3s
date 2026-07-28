fn last_run_label(dir: &Path) -> String {
    let path = dir.join(RUN_LOG_FILE);
    let Ok(text) = std::fs::read_to_string(path) else {
        return "never".to_string();
    };
    text.lines()
        .rev()
        .find(|l| l.trim_start().starts_with("- "))
        .map(|l| l.trim_start().trim_start_matches("- ").to_string())
        .unwrap_or_else(|| "never".to_string())
}
pub(crate) fn append_run_start(spec: &LoopSpec, os_available: bool) -> Result<(), String> {
    let line = format!(
        "- {} start · level={} · os_runtime={} · status=running\n",
        chrono::Utc::now().to_rfc3339(),
        spec.level,
        os_available && spec.os_runtime
    );
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(spec.dir.join(RUN_LOG_FILE))
        .and_then(|mut f| {
            use std::io::Write;
            f.write_all(line.as_bytes())
        })
        .map_err(|e| e.to_string())
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

pub(crate) fn loop_run_prompt(spec: &LoopSpec, cwd: &str, os_available: bool) -> String {
    let mode = if os_available {
        LoopRuntimeMode::OsAvailable
    } else {
        LoopRuntimeMode::LocalNoOs
    };
    loop_run_prompt_with_runtime(spec, cwd, mode)
}

pub(crate) fn loop_run_prompt_with_runtime(
    spec: &LoopSpec,
    cwd: &str,
    runtime_mode: LoopRuntimeMode,
) -> String {
    let state = path_text(&spec.dir.join(STATE_FILE));
    let log = path_text(&spec.dir.join(RUN_LOG_FILE));
    let reports = path_text(&spec.dir.join("reports"));
    let skills = path_text(&spec.dir.join("skills"));
    let os_directive = match runtime_mode {
        LoopRuntimeMode::OsAvailable if spec.os_runtime => {
            format!(
                "OS IS AVAILABLE AND MUST BE USED. Use the signed-in A3S OS capabilities and A3S Runtime instead of doing the whole loop serially in the local shell. Split independent discovery/checker work into 3-6 parallel `parallel_task` branches or OS Runtime `runtime` tasks. Use shaped progressive API calls (`shaped:true`) when creating/reporting OS views so the TUI can surface RemoteUI. Create a Markdown report and standalone HTML report, then return the OS `.view`/`viewUrl` response; if no view can be created, explain the missing OS capability explicitly. Runtime evidence must include both fan-out (`runtime` or `parallel_task`) and the report view. Runtime work should be visible through the asset-scoped runtime activity panel; do not hide all execution in one long local command. {}",
                RuntimePolicy::Required.directive()
            )
        }
        LoopRuntimeMode::LocalAgentDev => {
            format!(
                "This loop is running inside local /agent development mode. Stay local even if OS is signed in. Do not open OS, WebIDE, RemoteUI, browser pages, or the OS workflow designer. Do not claim an OS RemoteUI view exists. Use local maker/checker passes; update the target agent definition only when the loop goal asks for agent improvements, and always update the loop state/report artifacts. {}",
                RuntimePolicy::LocalOnly.directive()
            )
        }
        LoopRuntimeMode::OsAvailable => {
            "This loop has os_runtime disabled. Run it locally and do not claim an OS RemoteUI view exists unless a later explicit OS tool call returns one.".to_string()
        }
        LoopRuntimeMode::LocalNoOs => {
            "OS is not signed in for this TUI session. Run the loop locally, but keep the same report/state artifacts. Do not claim an OS RemoteUI view exists; tell the user `/login` enables Runtime parallelism and RemoteUI.".to_string()
        }
    };
    let action_policy = match spec.level.as_str() {
        "L1" => {
            "L1 report-only: do not modify project source files. Only update this loop's STATE.md, RUN_LOG.md, and reports/ artifacts unless the user explicitly asks for a fix."
        }
        "A2" => {
            "A2 agent-development loop: the selected agent definition is the target asset. You may edit that agent definition and this loop's STATE.md, RUN_LOG.md, and reports/ artifacts when the goal asks for improvements. Keep changes local, validate the agent definition, and do not deploy."
        }
        "L2" => {
            "L2 assisted: use an isolated git worktree for any code edits, run verifier checks, and stop at a patch/branch plus human handoff. Do not merge or deploy."
        }
        "L3" => {
            "L3 unattended: act only within allowlisted low-risk scope, obey denylist and budget, and escalate to human on ambiguity, repeated failures, secrets, auth, infra, or product decisions."
        }
        _ => "Draft loop: produce a report and improve the loop state before taking action.",
    };
    format!(
        "Run this A3S Code engineered loop.\n\n\
         Loop id: {id}\n\
         Pattern: {pattern}\n\
         Level: {level}\n\
         Cadence target: {cadence}\n\
         Workspace: {cwd}\n\
         Goal: {goal}\n\n\
         Files you must read first:\n\
         - Config: {config}\n\
         - State: {state}\n\
         - Run log: {log}\n\
         - Skills directory: {skills}\n\n\
         Loop contract:\n\
         1. Read the state, run log, and skills before deciding work.\n\
         2. Respect denylist paths: {denylist}.\n\
         3. Respect budget: {budget} tokens/day, max {max_iter} iterations this run.\n\
         4. Use maker/checker split: maker `{maker}`, checker `{checker}`. The maker cannot declare its own work verified.\n\
         5. {action_policy}\n\
         6. {os_directive}\n\
         7. End by updating {state}, appending a finished entry to {log}, and creating both a Markdown and HTML report under {reports}.\n\
         8. Final answer: summarize what changed, list report paths, mention any OS RemoteUI view surfaced by the host, and say what needs human input.\n\n\
         Start now.",
        id = spec.id,
        pattern = spec.pattern,
        level = spec.level,
        cadence = spec.cadence,
        cwd = cwd,
        goal = spec.goal,
        config = path_text(&spec.dir.join(LOOP_CONFIG)),
        state = state,
        log = log,
        skills = skills,
        denylist = spec.denylist.join(", "),
        budget = spec.budget_tokens_per_day,
        max_iter = spec.max_iterations_per_run,
        maker = spec.maker_agent,
        checker = spec.checker_agent,
        reports = reports,
    )
}

fn audit_note(audit: &LoopAudit) -> String {
    let mut note = format!("score {} · {}", audit.score, audit.level);
    if !audit.missing.is_empty() {
        note.push_str(&format!(" · missing {}", audit.missing.len()));
    }
    note
}

impl App {
    pub(crate) fn handle_loop_command(&mut self, rest: &str) -> Option<Cmd<Msg>> {
        match parse_loop_command(rest) {
            LoopCommand::Dashboard => {
                self.textarea.clear();
                self.open_loop_panel(None);
                None
            }
            LoopCommand::Init(arg) => {
                self.textarea.clear();
                let agent = self.agent_dev.clone();
                let result = match agent.as_ref() {
                    Some(dev) => init_agent_loop(&self.cwd, &arg, dev),
                    None => init_loop(&self.cwd, &arg),
                };
                match result {
                    Ok(spec) => {
                        self.push_line(&gutter(
                            TN_GREEN,
                            &format!(
                                "loop `{}` initialized · {} · /loop run {}",
                                spec.id,
                                spec.dir.display(),
                                spec.id
                            ),
                        ));
                        let note = match agent.as_ref() {
                            Some(dev) => {
                                format!("created agent loop `{}` for `{}`", spec.id, dev.name)
                            }
                            None => format!("created `{}`", spec.id),
                        };
                        self.open_loop_panel(Some(note));
                    }
                    Err(e) => self.push_line(
                        &Style::new()
                            .fg(TN_RED)
                            .render(&format!("  /loop init failed: {e}")),
                    ),
                }
                None
            }
            LoopCommand::Run(name) => {
                self.textarea.clear();
                match find_loop(&self.cwd, &name) {
                    Ok(spec) => self.start_engineered_loop(spec),
                    Err(e) => {
                        self.push_line(
                            &Style::new()
                                .fg(TN_YELLOW)
                                .render(&format!("  {e} · create one with /loop init {name}")),
                        );
                        None
                    }
                }
            }
            LoopCommand::Audit(name) => {
                self.textarea.clear();
                match find_loop(&self.cwd, &name) {
                    Ok(spec) => {
                        let audit = audit_loop(&spec);
                        self.push_line(&gutter(
                            TN_CYAN,
                            &format!("loop audit `{}` · {}", spec.id, audit_note(&audit)),
                        ));
                        for item in audit.missing.iter().take(4) {
                            self.push_line(
                                &Style::new()
                                    .fg(TN_YELLOW)
                                    .render(&format!("  missing: {item}")),
                            );
                        }
                        self.open_loop_panel(Some(format!(
                            "audit `{}` · {}",
                            spec.id,
                            audit_note(&audit)
                        )));
                    }
                    Err(e) => self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {e}"))),
                }
                None
            }
            LoopCommand::Logs(name) => {
                self.textarea.clear();
                match find_loop(&self.cwd, &name) {
                    Ok(spec) => {
                        let path = spec.dir.join(RUN_LOG_FILE);
                        match std::fs::read_to_string(&path) {
                            Ok(text) => self.open_readonly_in_ide(
                                &format!("loop-{}-run-log.md", spec.id),
                                &text,
                            ),
                            Err(e) => self.push_line(
                                &Style::new()
                                    .fg(TN_YELLOW)
                                    .render(&format!("  run log unavailable: {e}")),
                            ),
                        }
                    }
                    Err(e) => self.push_line(&Style::new().fg(TN_YELLOW).render(&format!("  {e}"))),
                }
                None
            }
            LoopCommand::Quick(task) => {
                self.textarea.clear();
                if let Some(dev) = &self.agent_dev {
                    self.push_line(&gutter(
                        TN_GREEN,
                        &format!("agent loop `{}` · local auto-continue", dev.name),
                    ));
                }
                self.engage_autonomy(8);
                Some(cmd::msg(Msg::Submit(task)))
            }
            LoopCommand::Usage(usage) => {
                self.textarea.clear();
                self.push_line(&Style::new().fg(TN_GRAY).render(&format!("  {usage}")));
                None
            }
        }
    }

    fn start_engineered_loop(&mut self, spec: LoopSpec) -> Option<Cmd<Msg>> {
        let agent = self.agent_dev.clone();
        let runtime_mode = if agent.is_some() {
            LoopRuntimeMode::LocalAgentDev
        } else if self.os_session.is_some() {
            LoopRuntimeMode::OsAvailable
        } else {
            LoopRuntimeMode::LocalNoOs
        };
        let os_available = matches!(runtime_mode, LoopRuntimeMode::OsAvailable);
        if let Err(e) = append_run_start(&spec, os_available) {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render(&format!("  loop run log could not be updated: {e}")),
            );
        }
        self.goal = Some(match agent.as_ref() {
            Some(dev) => agent::agent_goal_label(dev, &spec.goal),
            None => spec.goal.clone(),
        });
        self.goal_since = Some(Instant::now());
        self.push_line(&gutter(
            TN_CYAN,
            &format!(
                "loop `{}` running · {} · {}",
                spec.id,
                spec.level,
                if matches!(runtime_mode, LoopRuntimeMode::LocalAgentDev) {
                    "local agent engineering"
                } else if os_available && spec.os_runtime {
                    "OS Runtime + RemoteUI required"
                } else {
                    "local fallback"
                }
            ),
        ));
        if os_available && spec.os_runtime {
            self.push_line(&Style::new().fg(TN_GRAY).render(
                "  OS connected: use A3S Runtime parallel workers; inspect them with asset activity",
            ));
        } else if let Some(dev) = &agent {
            self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                "  /agent active: loop stays local and targets {} ({})",
                dev.name, dev.rel
            )));
        }
        let prompt = if matches!(runtime_mode, LoopRuntimeMode::LocalAgentDev) {
            loop_run_prompt_with_runtime(&spec, &self.cwd, runtime_mode)
        } else {
            loop_run_prompt(&spec, &self.cwd, os_available)
        };
        let (prompt, display) = match agent.as_ref() {
            Some(dev) => (
                agent::agent_loop_prompt(dev, &prompt),
                format!("◇ loop {}: {}", dev.name, truncate(&spec.goal, 48)),
            ),
            None => (
                prompt,
                format!("loop {}: {}", spec.id, truncate(&spec.goal, 54)),
            ),
        };
        self.engage_autonomy(8);
        let runtime_expectation = (os_available && spec.os_runtime)
            .then(|| RuntimeExpectation::required_report_view(format!("loop {}", spec.id)));
        self.start_stream_inner_with_runtime(
            prompt,
            display,
            true,
            true,
            false,
            runtime_expectation,
        )
    }

    pub(crate) fn open_loop_panel(&mut self, note: Option<String>) {
        let loops = list_loops(&self.cwd);
        let note = note.unwrap_or_else(|| {
            if let Some(dev) = &self.agent_dev {
                format!(
                    "agent dev `{}` active · /loop init creates an agent-scoped local loop",
                    dev.name
                )
            } else if loops.is_empty() {
                "no loops yet · /loop init daily-triage".to_string()
            } else if self.os_session.is_some() {
                "OS connected · runs use A3S Runtime + RemoteUI when enabled".to_string()
            } else {
                "sign in with /login to use OS Runtime + RemoteUI".to_string()
            }
        });
        self.loop_panel = Some(LoopPanel {
            loops,
            sel: 0,
            note,
        });
    }

    pub(crate) fn handle_loop_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if key.code == KeyCode::Esc {
            self.loop_panel = None;
            return None;
        }
        let last = self
            .loop_panel
            .as_ref()
            .map(|p| p.loops.len().saturating_sub(1))
            .unwrap_or(0);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(p) = self.loop_panel.as_mut() {
                    p.sel = p.sel.saturating_sub(1);
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(p) = self.loop_panel.as_mut() {
                    p.sel = (p.sel + 1).min(last);
                }
                None
            }
            KeyCode::Char('r') | KeyCode::Enter => {
                let spec = self
                    .loop_panel
                    .as_ref()
                    .and_then(|p| p.loops.get(p.sel))
                    .map(|s| s.spec.clone())?;
                self.loop_panel = None;
                self.start_engineered_loop(spec)
            }
            KeyCode::Char('a') => {
                if let Some(p) = self.loop_panel.as_mut() {
                    if let Some(summary) = p.loops.get(p.sel) {
                        p.note = format!(
                            "audit `{}` · {}",
                            summary.spec.id,
                            audit_note(&summary.audit)
                        );
                    }
                }
                None
            }
            KeyCode::Char('l') => {
                let spec = self
                    .loop_panel
                    .as_ref()
                    .and_then(|p| p.loops.get(p.sel))
                    .map(|s| s.spec.clone())?;
                let path = spec.dir.join(RUN_LOG_FILE);
                match std::fs::read_to_string(&path) {
                    Ok(text) => {
                        self.loop_panel = None;
                        self.open_readonly_in_ide(&format!("loop-{}-run-log.md", spec.id), &text);
                    }
                    Err(e) => {
                        if let Some(p) = self.loop_panel.as_mut() {
                            p.note = format!("run log unavailable: {e}");
                        }
                    }
                }
                None
            }
            KeyCode::Char('p') => {
                let query = self
                    .loop_panel
                    .as_ref()
                    .and_then(|p| p.loops.get(p.sel))
                    .map(|s| s.spec.id.clone())
                    .unwrap_or_default();
                self.loop_panel = None;
                self.open_runtime_activity_panel(query)
            }
            KeyCode::Char('i') => {
                let agent = self.agent_dev.clone();
                let result = match agent.as_ref() {
                    Some(dev) => init_agent_loop(&self.cwd, "", dev),
                    None => init_loop(&self.cwd, DEFAULT_PATTERN),
                };
                match result {
                    Ok(spec) => {
                        let note = match agent.as_ref() {
                            Some(dev) => {
                                format!("created agent loop `{}` for `{}`", spec.id, dev.name)
                            }
                            None => format!("created `{}`", spec.id),
                        };
                        self.open_loop_panel(Some(note));
                    }
                    Err(e) => {
                        if let Some(p) = self.loop_panel.as_mut() {
                            p.note = e;
                        }
                    }
                }
                None
            }
            _ => None,
        }
    }

    pub(crate) fn render_loop_panel(&self, panel: &LoopPanel) -> String {
        let width = self.width as usize;
        let h = self.height as usize;
        let (left_w, right_w) = loop_columns(width);
        let mut left = Vec::new();
        left.extend(loop_header_lines(!panel.loops.is_empty(), left_w));
        if !panel.loops.is_empty() {
            for (idx, item) in panel.loops.iter().enumerate() {
                let mark = if idx == panel.sel { ">" } else { " " };
                let row = format!(
                    "{mark} {:<18} {:<3} {:>3}  {}",
                    truncate(&item.spec.id, 18),
                    item.spec.level,
                    item.audit.score,
                    truncate(&item.last_run, left_w.saturating_sub(31))
                );
                let style = if idx == panel.sel {
                    Style::new().fg(TN_CYAN).bold()
                } else {
                    Style::new().fg(TN_FG)
                };
                left.push(loop_line(&style.render(&row), left_w));
            }
        }
        while left.len() < h {
            left.push(" ".repeat(left_w));
        }
        let selected = panel.loops.get(panel.sel);
        let mut right = loop_detail_lines(selected, &panel.note, right_w);
        while right.len() < h {
            right.push(" ".repeat(right_w));
        }
        let mut rows = Vec::new();
        let sep = if width == 0 {
            String::new()
        } else {
            Style::new().fg(TN_GRAY).render("│")
        };
        for i in 0..h {
            rows.push(format!(
                "{}{}{}",
                left.get(i).cloned().unwrap_or_else(|| " ".repeat(left_w)),
                sep,
                right.get(i).cloned().unwrap_or_else(|| " ".repeat(right_w))
            ));
        }
        rows.join("\n")
    }
}
