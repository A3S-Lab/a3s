//! Transcript, viewport, remote-view, and approval presentation state.

use super::*;

fn should_capture_bottom_anchor(viewport: &Viewport) -> bool {
    viewport.auto_scroll_enabled()
}

impl App {
    /// One-shot transcript warning as the context fills: a heads-up at 70%
    /// and a red alert at 85% (the auto-compact point). Called wherever
    /// `last_prompt_tokens` updates; the latch re-arms when usage drops.
    pub(super) fn maybe_warn_ctx(&mut self) {
        if self.context_limit == 0 {
            return;
        }
        let pct = (self.last_prompt_tokens * 100 / self.context_limit as usize).min(100);
        let (latch, warn) = ctx_warn_tier(pct, self.ctx_warned_tier);
        self.ctx_warned_tier = latch;
        if warn.is_some() {
            // push_line rebuilds the viewport from `messages` only, which
            // would hide a still-streaming round's text (invisible through a
            // whole approval wait if this round ends in a gated tool call).
            // Finalize it into the transcript first — same as ToolStart does.
            self.finalize_streaming();
        }
        match warn {
            Some(85) => self.push_line(&Style::new().fg(TN_RED).render(&format!(
                "  ✦ context {pct}% full — auto-compacting soon; /compact to summarize now"
            ))),
            Some(_) => self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                "  ✦ context {pct}% full — auto-compacts near 85%; /compact to summarize early"
            ))),
            None => {}
        }
    }

    pub(super) fn mark_agent_activity(&mut self) {
        self.turn_had_agent_activity = true;
        self.turn_text_after_activity = false;
    }

    pub(super) fn mark_assistant_text(&mut self, text: &str) {
        if !text.trim().is_empty() {
            self.turn_text_after_activity = true;
        }
    }

    pub(super) fn prepare_ultracode_synthesis(&self) -> Option<(String, String)> {
        if !needs_synthesis(
            self.ultracode_synthesis_inflight,
            self.ultracode_synthesis_used,
            self.turn_had_agent_activity,
            self.turn_text_after_activity,
        ) {
            return None;
        }

        let user_task = self
            .running_task
            .as_deref()
            .filter(|task| !task.trim().is_empty())
            .unwrap_or("the previous task");
        let mut prompt = format!(
            "[synthesis]\n\
             The previous turn completed planning/tool/subagent work \
             but stopped without a final user-facing answer.\n\n\
             Original user task:\n{user_task}\n\n\
             Write the final answer now in the user's language. Synthesize the \
             completed work into a useful response. Do not call tools or start \
             more subagents unless it is strictly necessary to avoid an incorrect \
             answer. If a child run produced no text output, summarize the \
             available plan/status instead of exposing raw task metadata.\n"
        );

        if !self.plan.is_empty() {
            prompt.push_str("\nPlan/status:\n");
            for task in self.plan.tasks() {
                let status = match task.status {
                    a3s_code_core::planning::TaskStatus::Pending => "pending",
                    a3s_code_core::planning::TaskStatus::InProgress => "in progress",
                    a3s_code_core::planning::TaskStatus::Completed => "done",
                    a3s_code_core::planning::TaskStatus::Failed => "failed",
                    a3s_code_core::planning::TaskStatus::Skipped => "skipped",
                    a3s_code_core::planning::TaskStatus::Cancelled => "cancelled",
                };
                prompt.push_str(&format!("- [{status}] {}\n", task.content));
            }
        }

        let subagents = self.runtime.subagents();
        if !subagents.is_empty() {
            prompt.push_str("\nSubagents:\n");
            for agent in subagents {
                let status = match agent.success {
                    Some(true) => "done",
                    Some(false) => "failed",
                    None if agent.done => "done",
                    None => "unknown",
                };
                prompt.push_str(&format!(
                    "- [{status}] {}: {}\n",
                    agent.display_agent(),
                    agent.description
                ));
            }
        }

        if let Some(workflow) = &self.last_workflow {
            prompt.push_str("\nLatest workflow intent summary:\n");
            prompt.push_str(&truncate(workflow, 4000));
            prompt.push('\n');
        }

        Some((prompt, user_task.to_string()))
    }

    pub(super) fn finalize_streaming(&mut self) {
        let reasoning = std::mem::take(&mut self.thinking);
        if !reasoning.trim().is_empty() {
            self.messages.push(TranscriptEntry::reasoning(reasoning));
        }
        let source = self.streaming.raw_content().to_string();
        if !source.trim().is_empty() {
            self.messages
                .push(TranscriptEntry::assistant_markdown(source));
        }
        self.streaming.clear();
        self.rebuild_viewport();
    }

    pub(super) fn finish(&mut self) {
        self.preserve_interrupted_tools();
        self.llm_turn_checkpoint = None;
        self.active_turn_mode = None;
        self.active_plan_draft = None;
        self.execution_policy.set_mode(self.mode);
        self.state = State::Idle;
        self.running_task = None;
        self.plan.clear();
        self.runtime.finish_turn_entities(Instant::now());
        self.ultracode_synthesis_inflight = false;
        self.relayout();
        self.stream_started = None;
        self.spinner.stop();
        self.rx = None;
        self.stream_join = None;
        self.stream_join_settling = false;
        self.stream_settle_abort = None;
        self.host_tool_abort = None;
        self.host_progress_inflight = false;
        self.host_tool_call_id = None;
        self.deep_research_events = None;
        debug_assert!(self.deep_research_handle.is_none());
        self.restore_current_approval_feedback();
        self.pending_tools.clear();
        self.permission_rule_write_inflight = None;
        self.approval_sel = 0;
        self.interrupting = false;
        self.rebuild_viewport();
    }

    pub(super) fn push_line(&mut self, line: &str) {
        self.messages
            .push(TranscriptEntry::preformatted(line.to_string()));
        self.rebuild_viewport();
    }

    pub(super) fn push_notice(&mut self, kind: NoticeKind, message: impl Into<String>) {
        self.messages.push(TranscriptEntry::notice(kind, message));
        self.rebuild_viewport();
    }

    pub(super) fn push_tracked_line(&mut self, line: &str) -> TranscriptEntryId {
        let entry = self
            .messages
            .push_tracked(TranscriptEntry::preformatted(line.to_string()));
        self.rebuild_viewport();
        entry
    }

    pub(super) fn replace_tracked_line(&mut self, entry: TranscriptEntryId, line: &str) {
        // Capture before replacement clears the old layout so a user reading
        // higher in the transcript keeps the same semantic scroll anchor.
        let anchor = self.capture_viewport_anchor();
        if self.messages.replace_preformatted(entry, line.to_string()) {
            self.rebuild_viewport_from(anchor);
        }
        // A missing ID means the transcript was cleared or rebuilt while the
        // operation was in flight; never resurrect that stale result here.
    }

    pub(super) fn push_terminal_tool(&mut self, completed: CompletedTool) {
        if presentation_policy(&completed.name) == ToolPresentationPolicy::PinnedOnly {
            self.messages.discard_tool(&completed.id);
        } else {
            self.messages.finish_tool_with_state(
                &completed.id,
                completed.name,
                completed.args,
                completed.output,
                completed.exit_code,
                None,
                completed.state,
                true,
            );
        }
        self.rebuild_viewport();
    }

    pub(super) fn push_subagent_completion(&mut self, completed: CompletedSubagent) {
        self.messages.finish_subagent_with_outcome(
            completed.task_id,
            completed.display_agent,
            completed.description,
            completed.outcome,
            completed.output,
            completed.visible_in_transcript,
        );
        self.relayout();
        self.rebuild_viewport();
    }

    pub(super) fn preserve_interrupted_tools(&mut self) {
        for completed in self.runtime.interrupt_unfinished_tools() {
            if presentation_policy(&completed.name) == ToolPresentationPolicy::PinnedOnly {
                self.messages.discard_tool(&completed.id);
            } else {
                self.messages.finish_tool_with_state(
                    &completed.id,
                    completed.name,
                    completed.args,
                    completed.output,
                    completed.exit_code,
                    None,
                    completed.state,
                    true,
                );
            }
        }
        self.messages.interrupt_unfinished_tools();
    }

    pub(super) fn stage_deep_research_report(
        &mut self,
        artifacts: &ResearchReportArtifacts,
        outcome: DeepResearchRunOutcome,
        authority: DeepResearchTerminalArtifactAuthority,
    ) {
        debug_assert!(!matches!(outcome, DeepResearchRunOutcome::Active));
        self.deep_research_outcome = outcome;
        if matches!(outcome, DeepResearchRunOutcome::Degraded) {
            self.loop_remaining = 0;
        }
        self.pending_deep_research_report_view = remote_ui::local_file_view(&artifacts.html).ok();
        self.deep_research_terminal_artifacts = Some((artifacts.clone(), authority));
    }

    pub(super) fn capture_research_report_view(&mut self, output: &str) -> bool {
        // DeepResearch artifacts are staged only by the host-owned structured
        // report pipeline. Text markers remain available for ordinary report
        // workflows, but can never promote a second DeepResearch report path.
        if self.deep_research_loop.is_some() {
            return false;
        }
        let workspace = Path::new(&self.cwd);
        let spec = research_report_view_spec(output, workspace);
        if let Some(spec) = spec {
            let is_new = self.remember_remote_view(spec.clone());
            if is_new {
                self.open_remote_view(&spec);
            }
            return true;
        }
        false
    }

    pub(super) fn open_pending_deep_research_report_view(&mut self) {
        let Some(spec) = self.pending_deep_research_report_view.take() else {
            return;
        };
        let is_new = self.remember_remote_view(spec.clone());
        if is_new {
            self.open_remote_view(&spec);
        }
    }

    /// Open an OS viewUrl in the native `a3s-webview` window. If the helper is
    /// not installed, fall back to the system browser and leave a transcript
    /// hint so the click never feels like a no-op.
    pub(super) fn open_remote_view(&mut self, spec: &remote_ui::ViewSpec) {
        let webview_binary = self.agent_presence.webview_binary().map(Path::to_path_buf);
        match remote_ui::open_window_with(spec, webview_binary.as_deref()) {
            Ok(remote_ui::OpenedWith::Webview) => {}
            Ok(remote_ui::OpenedWith::Browser) => {
                let helper = remote_ui::webview_helper_path_with(webview_binary.as_deref())
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|| {
                        "missing; install a3s-webview or set A3S_WEBVIEW_BIN".to_string()
                    });
                let view_kind = if remote_ui::is_local_image_view(spec) {
                    "no-auth local image preview helper"
                } else if remote_ui::is_local_report_view(spec) {
                    "no-auth local report popup helper"
                } else {
                    "authenticated RemoteUI popup helper"
                };
                self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                    "  ↗ opened URL in browser: {} · {view_kind}: {helper}",
                    spec.url,
                )));
            }
            Err(err) => {
                self.push_line(&Style::new().fg(TN_GRAY).render(&format!(
                    "  ↗\u{200A}open in your browser: {} ({err})",
                    spec.url
                )));
            }
        }
    }

    pub(super) fn find_remote_view_spec(&self, output: &str) -> Option<remote_ui::ViewSpec> {
        // The progressive API returns a RELATIVE view url; complete it against
        // the signed-in OS origin (the TUI is "the edge").
        let os_origin = self
            .os_session
            .as_ref()
            .map(|s| crate::a3s_os::os_origin(&s.address));
        remote_ui::find_view_url(output, os_origin.as_deref())
    }

    pub(super) fn remember_remote_view(&mut self, spec: remote_ui::ViewSpec) -> bool {
        // Remember the view and surface a clickable "Open view" line ourselves
        // (deterministic) rather than trusting the model to print the marker —
        // weaker models often forget it or jq the `.view` object away.
        let is_new = is_new_remote_view(self.last_view.as_ref(), &spec);
        self.last_view = Some(spec.clone());
        self.record_runtime_view_evidence();
        if is_new {
            self.push_line(&gutter(ACCENT, &remote_view_button("click to open")));
        }
        is_new
    }

    /// Skill dirs for the session: the discovered Claude/Codex dirs plus the
    /// login-gated built-in OS `a3s-os-capabilities` skill when signed in.
    pub(crate) fn skill_dirs(&self) -> Vec<std::path::PathBuf> {
        let mut dirs = agent_skill_dirs_with_configured(&self.cwd, &self.asset_directories.skill);
        // Always-available built-in skills (the `okf` LLM-wiki / knowledge compiler).
        if let Some(d) = ensure_builtin_skills_dir() {
            dirs.push(d);
        }
        if self.os_session.is_some() {
            if let Some(cfg) = &self.os_config {
                if let Some(d) = crate::a3s_os::ensure_capability_skill_dir(cfg) {
                    dirs.push(d);
                }
            }
        }
        dirs
    }

    /// After an OS login/logout, rebuild the session so the login-gated
    /// skill loads/unloads immediately, and refresh the start-screen skill list.
    pub(super) fn refresh_after_auth(&mut self) -> Option<Cmd<Msg>> {
        self.os_gateway_models = None;
        self.os_gateway_models_loading = false;
        self.os_gateway_error = None;
        // Login/logout flips whether the A3S Runtime `runtime` tool is available.
        self.sync_runtime_tool();
        let dirs = self.skill_dirs();
        self.skill_count = count_skill_files(&dirs);
        self.skills = load_skills(&dirs);
        if self.state == State::Idle {
            let profile = self.session_rebuild_profile();
            self.start_session_rebuild(
                profile,
                SessionRebuildAction::Refresh {
                    failure_context: Some("refresh the authenticated session"),
                },
            )
        } else {
            None
        }
    }

    /// Register the A3S Runtime `runtime` offload tool while signed in to OS,
    /// unregister it while signed out — so it only appears in the model's toolset
    /// after login. Called after every auth change (login/logout), once the
    /// session has been (re)built.
    pub(super) fn replace_session(&mut self, session: AgentSession) {
        // A task panel is scoped to the exact Core session and its live
        // cancellation handles. Closing it prevents a late refresh or click
        // from targeting a rebuilt session with a coincidentally equal task id.
        self.task_panel = None;
        self.history_panel = None;
        self.permission_panel = None;
        self.session = Arc::new(session);
        let _ = self.session.register_dynamic_workflow_runtime();
        self.sync_runtime_tool();
        if let Ok(mut active) = self.active_session.lock() {
            *active = Arc::clone(&self.session);
            if let Some(use_registry) = self.use_registry.ready_handle() {
                use_registry.replace_session(Arc::clone(&self.session));
            }
        }
    }

    pub(super) fn sync_runtime_tool(&self) {
        let _ = match self.os_session.as_ref() {
            Some(s) => self.session.register_dynamic_tool(std::sync::Arc::new(
                crate::runtime_tool::RuntimeTool::new(s),
            )),
            None => self.session.unregister_dynamic_tool("runtime"),
        };
    }

    /// Open `path` directly in the built-in IDE editor (tree rooted at its
    /// directory, file loaded, editor focused). Used by `/config` + first launch.
    pub(super) fn open_config_in_ide(&mut self, path: &std::path::Path) {
        let dir = path.parent().unwrap_or(std::path::Path::new("."));
        let lines: Vec<String> = std::fs::read_to_string(path)
            .unwrap_or_default()
            .replace('\t', "    ")
            .lines()
            .map(String::from)
            .collect();
        let mut ide = Ide::browse(ide_children(dir, 0), "config");
        ide.file = Some(IdeFile::new(path.to_path_buf(), lines, false, false));
        ide.focus_editor = true;
        self.ide = Some(ide);
    }

    /// Capture a source-free dynamic-workflow intent or a distinct
    /// `parallel_task`/`task` delegation summary for synthesis and a collapsed
    /// transcript marker.
    pub(super) fn capture_workflow(&mut self, name: &str, args: Option<&serde_json::Value>) {
        let Some((doc, label)) = workflow_doc_for_tool(name, args) else {
            return;
        };
        self.last_workflow = Some(doc);
        self.push_line(&Style::new().fg(ACCENT).render(&format!("  ⊞ {label}")));
    }

    /// Open read-only text content in the built-in IDE. Editor-focused for
    /// scroll/nav, but `readonly` blocks edits and Ctrl+S.
    pub(super) fn open_readonly_in_ide(&mut self, title: &str, content: &str) {
        let lines: Vec<String> = content.lines().map(String::from).collect();
        let mut ide = Ide::browse(
            ide_children(std::path::Path::new(&self.cwd), 0),
            "workspace",
        );
        ide.file = Some(IdeFile::new(
            std::path::PathBuf::from(title),
            lines,
            false,
            true,
        ));
        ide.focus_editor = true;
        ide.flash = Some(ide_flash_line(ToastKind::Warning, "read-only"));
        self.ide = Some(ide);
    }

    /// Render the complete semantic conversation plus the current live tail
    /// for Codex-style Ctrl+T, including user and assistant messages, calls in
    /// every lifecycle state, the current plan, subagents, reasoning, and
    /// unterminated streaming Markdown.
    pub(super) fn format_transcript_view(&self) -> Option<String> {
        let content_width = self.width as usize;
        let mut blocks = self.messages.render_transcript_with_activity(
            self.width,
            content_width,
            self.blink_tick % 8 < 4,
        );
        let reasoning = thinking_block(&self.thinking, content_width);
        if !reasoning.is_empty() {
            blocks.push(reasoning);
        }
        if !self.streaming.raw_content().is_empty() {
            let block = assistant_block(&self.streaming.full_view(), content_width);
            if !block.is_empty() {
                blocks.push(block);
            }
        }
        let plan = self.plan_lines();
        if !plan.is_empty() {
            blocks.push(plan.join("\n"));
        }
        let subagents = self.subagent_lines();
        if !subagents.is_empty() {
            blocks.push(subagents.join("\n"));
        }
        (!blocks.is_empty()).then(|| join_transcript_blocks(&blocks))
    }

    pub(super) fn transcript_view_is_open(&self) -> bool {
        self.transcript_view.is_some()
    }

    pub(super) fn open_transcript_view(&mut self) {
        match self.format_transcript_view() {
            Some(content) => {
                self.transcript_view = Some(SemanticTranscriptViewport::new(
                    &content,
                    self.width,
                    self.height,
                ));
            }
            None => self.push_line(
                &Style::new()
                    .fg(TN_GRAY)
                    .render("  no transcript entries yet this session"),
            ),
        }
    }

    /// Refresh the semantic transcript without disturbing its anchored scroll
    /// position or the user's composer draft.
    pub(super) fn refresh_transcript_view(&mut self) {
        if !self.transcript_view_is_open() {
            return;
        }
        let content = self.format_transcript_view().unwrap_or_default();
        if let Some(transcript) = self.transcript_view.as_mut() {
            transcript.set_content(&content);
        }
    }

    /// Move through prompt history and load the entry into the input. Going
    /// forward past the newest entry restores the scratch draft from before
    /// history browsing started.
    pub(super) fn history_recall(&mut self, up: bool) {
        let current = self.textarea.value();
        if let Some(value) = history_recall_value(
            &self.history,
            &mut self.history_pos,
            &mut self.history_draft,
            &current,
            up,
        ) {
            self.textarea.set_value(&value);
        }
    }

    pub(super) fn update_viewport_with_stream(&mut self) {
        // Match Codex's 120fps frame limiter. Transcript entries are cached and
        // the viewport retains the stable prefix, so each frame replaces only
        // the mutable tail instead of rebuilding the full rendered history.
        if let Some(t) = self.last_paint {
            if t.elapsed() < STREAM_COMMIT_TICK_INTERVAL {
                return;
            }
        }
        self.last_paint = Some(Instant::now());
        let anchor = self.capture_viewport_anchor();
        self.update_viewport_with_stream_from(anchor);
    }

    pub(super) fn update_viewport_with_stream_from(&mut self, anchor: ViewportAnchor) {
        let content_width = self.viewport_content_width();
        let mut blocks =
            self.messages
                .render_with_activity(self.width, content_width, self.blink_tick % 8 < 4);
        let body = thinking_block(&self.thinking, self.viewport_content_width());
        if !body.is_empty() {
            blocks.push(body);
        }
        let stable = self.streaming.visible_stable_view();
        let tail = self.streaming.tail_view();
        let mut prefix = String::from("\n");
        if !blocks.is_empty() {
            prefix.push_str(&join_transcript_blocks(&blocks));
        }
        let suffix = if let Some((stream_prefix, stream_suffix)) =
            assistant_stream_block_parts(&stable, &tail, content_width)
        {
            if !blocks.is_empty() {
                prefix.push_str(transcript_block_separator());
            }
            prefix.push_str(&stream_prefix);
            format!("{stream_suffix}\n")
        } else {
            prefix.push('\n');
            String::new()
        };
        // Stable stream rows live in the retained prefix; only the
        // structurally mutable Markdown tail is replaced. Finalization still
        // consolidates the complete raw source into one reflowable transcript
        // entry, matching Codex's committed-history + active-tail model.
        self.viewport.set_content_parts(&prefix, &suffix);
        self.restore_viewport_anchor(anchor);
        self.refresh_transcript_selection_projection();
        self.refresh_transcript_view();
    }

    pub(super) fn rebuild_viewport(&mut self) {
        let anchor = self.capture_viewport_anchor();
        self.rebuild_viewport_from(anchor);
    }

    pub(super) fn rebuild_viewport_from(&mut self, anchor: ViewportAnchor) {
        let content_width = self.viewport_content_width();
        let blocks =
            self.messages
                .render_with_activity(self.width, content_width, self.blink_tick % 8 < 4);
        let full = join_transcript_blocks(&blocks);
        self.viewport.set_content(&format!("\n{full}\n")); // top padding
        self.restore_viewport_anchor(anchor);
        self.refresh_transcript_selection_projection();
        self.refresh_transcript_view();
    }

    pub(super) fn capture_viewport_anchor(&self) -> ViewportAnchor {
        // Auto-follow is explicit user state. During streaming, Markdown
        // reflow can temporarily shrink the rendered tail until a paused
        // viewport happens to be at the current bottom. Treating that geometry
        // as intent would re-enable follow mode and yank the user back down as
        // soon as the next delta grows the tail again.
        if should_capture_bottom_anchor(&self.viewport) {
            return ViewportAnchor::Bottom;
        }
        let offset = self.viewport.scroll_offset();
        self.messages
            .anchor_for_row(offset.saturating_sub(1))
            .map(ViewportAnchor::Transcript)
            .unwrap_or(ViewportAnchor::Absolute(offset))
    }

    pub(super) fn restore_viewport_anchor(&mut self, anchor: ViewportAnchor) {
        match anchor {
            ViewportAnchor::Bottom => {
                self.viewport.set_auto_scroll(true);
                self.viewport.update(ViewportMsg::Bottom);
            }
            ViewportAnchor::Transcript(anchor) => {
                self.viewport.set_auto_scroll(false);
                if let Some(row) = self.messages.row_for_anchor(anchor) {
                    self.viewport.set_scroll_offset(row.saturating_add(1));
                }
            }
            ViewportAnchor::Absolute(offset) => {
                self.viewport.set_auto_scroll(false);
                self.viewport.set_scroll_offset(offset);
            }
        }
    }

    /// Rows the input box needs — the textarea auto-grows its own height with
    /// embedded newlines (Shift+Enter), so the layout just mirrors it.
    pub(crate) fn input_height(&self) -> u16 {
        self.textarea.height()
    }

    /// Scoped approval keys. A grant is derived from the authoritative tool
    /// event, never from the display label.
    pub(super) fn handle_approval_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if self.permission_rule_write_inflight.is_some() {
            return None;
        }
        if self.approval_feedback.is_some() {
            if key.code == KeyCode::Esc {
                self.restore_current_approval_feedback();
                return None;
            }
            match self.textarea.handle_key(key) {
                Some(TextareaMsg::Submit(reason)) if !reason.trim().is_empty() => {
                    let reason = reason.trim().to_string();
                    self.restore_current_approval_feedback();
                    return self.deny_current_approval(&reason).map(cmd::msg);
                }
                Some(TextareaMsg::Submit(_)) => return None,
                Some(TextareaMsg::Changed(_)) => {
                    self.relayout();
                    return None;
                }
                None => return None,
            }
        }

        match key.code {
            KeyCode::Up => {
                self.approval_sel = self.approval_sel.saturating_sub(1);
                None
            }
            KeyCode::Down => {
                self.approval_sel = (self.approval_sel + 1).min(3);
                None
            }
            KeyCode::Enter => self.apply_approval(self.approval_sel).map(cmd::msg),
            KeyCode::Char('y' | 'Y') => self.apply_approval(0).map(cmd::msg),
            KeyCode::Char('s' | 'S') => self.apply_approval(1).map(cmd::msg),
            KeyCode::Char('p' | 'P') => self.apply_approval(2).map(cmd::msg),
            KeyCode::Char('n' | 'N') | KeyCode::Esc => self.apply_approval(3).map(cmd::msg),
            KeyCode::Char(c @ '1'..='4') => {
                self.apply_approval(c as usize - '1' as usize).map(cmd::msg)
            }
            _ => None,
        }
    }

    pub(super) fn handle_approval_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        if self.state != State::Awaiting {
            return None;
        }
        if self.permission_rule_write_inflight.is_some() || self.approval_feedback.is_some() {
            return None;
        }
        let pending = self.pending_tools.front()?;
        let width = (self.width as usize).min(u16::MAX as usize);
        if width == 0 {
            return None;
        }
        let mut prompt = approval_prompt(&pending.label, self.approval_sel);
        let row_count = prompt.lines(width).len();
        if row_count == 0 {
            return None;
        }
        let y_offset =
            approval_overlay_y_offset(self.height as usize, row_count, self.approval_rows_below());
        let row = mouse.row as usize;
        let start = y_offset as usize;
        if row < start || row >= start.saturating_add(row_count) {
            return None;
        }
        prompt.set_y_offset(y_offset);
        let before = prompt.selected_index();

        match prompt.handle_mouse(mouse, width) {
            Some(ApprovalPromptMsg::Selected(index)) => self.apply_approval(index).map(cmd::msg),
            None => {
                let after = prompt.selected_index().min(3);
                if after != before {
                    self.approval_sel = after;
                }
                None
            }
        }
    }

    pub(super) fn apply_approval(&mut self, choice: usize) -> Option<Msg> {
        let pending = self.pending_tools.front()?.clone();
        match choice {
            0 => Some(Msg::ModalConfirm {
                tool_id: pending.tool_id,
                approved: true,
                reason: None,
            }),
            1 => {
                self.permission_grants
                    .allow_for_session(pending.grant.clone());
                self.refresh_permission_panel_grants();
                Some(Msg::ModalConfirm {
                    tool_id: pending.tool_id,
                    approved: true,
                    reason: None,
                })
            }
            2 => {
                if self.project_permission_revoke_inflight.is_some() {
                    self.push_notice(
                        NoticeKind::Warning,
                        "A project permission revocation is running; wait before saving another project rule",
                    );
                    return None;
                }
                self.permission_rule_write_inflight = Some(pending.tool_id.clone());
                Some(Msg::PersistProjectPermission {
                    tool_id: pending.tool_id,
                    grant: pending.grant,
                })
            }
            _ => {
                self.begin_approval_feedback(&pending.tool_id);
                None
            }
        }
    }

    pub(super) fn deny_current_approval(&self, reason: &str) -> Option<Msg> {
        let tool_id = self.pending_tools.front()?.tool_id.clone();
        Some(Msg::ModalConfirm {
            tool_id,
            approved: false,
            reason: Some(reason.trim().to_string()),
        })
    }

    fn begin_approval_feedback(&mut self, tool_id: &str) {
        if self.approval_feedback.is_some() {
            return;
        }
        self.approval_feedback = Some(ApprovalFeedback {
            tool_id: tool_id.to_string(),
            stashed_composer: self.textarea.value(),
        });
        self.textarea.clear();
        self.approval_sel = 3;
        self.relayout();
    }

    pub(super) fn restore_current_approval_feedback(&mut self) {
        let Some(feedback) = self.approval_feedback.take() else {
            return;
        };
        self.textarea.set_value(&feedback.stashed_composer);
        self.relayout();
    }

    pub(super) fn restore_approval_feedback_for(&mut self, tool_id: &str) {
        if self
            .approval_feedback
            .as_ref()
            .is_some_and(|feedback| feedback.tool_id == tool_id)
        {
            self.restore_current_approval_feedback();
        }
    }

    /// Tool-approval options panel (Claude-style numbered choices).
    pub(super) fn overlay_approval(&self, composed: String) -> String {
        if self.state != State::Awaiting {
            return composed;
        }
        let Some(pending) = self.pending_tools.front() else {
            return composed;
        };
        let prompt = approval_prompt(&pending.label, self.approval_sel)
            .with_denial_feedback(self.approval_feedback.is_some())
            .with_project_rule_saving(
                self.permission_rule_write_inflight
                    .as_deref()
                    .is_some_and(|tool_id| tool_id == pending.tool_id.as_str()),
            );
        let menu = prompt.lines(self.width as usize);
        self.overlay_list_with_rows_below(composed, &menu, self.approval_rows_below())
    }

    pub(super) fn approval_rows_below(&self) -> usize {
        approval_rows_below_for(self.transcript_view.is_some(), self.overlay_rows_below())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bottom_anchor_uses_follow_state_not_transient_geometry() {
        let mut viewport = Viewport::new(80, 2).with_auto_scroll(false);
        viewport.set_content("zero\none\ntwo\nthree");
        viewport.set_scroll_offset(2);

        assert!(viewport.at_bottom());
        assert!(!should_capture_bottom_anchor(&viewport));
    }
}
