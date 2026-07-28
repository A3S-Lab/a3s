//! `/model` picker (with account tabs) + `/effort` rebuild logic + overlays.

use super::super::*;
use crate::account_providers::AccountProvider;
use crate::config::{
    save_model_selection_preference, ModelSelectionPreference, ModelSelectionSource,
};
use a3s_tui::components::{TabbedMenuItem, TabbedMenuPanel, TabbedMenuPanelMsg, TabbedMenuTab};
use a3s_tui::event::MouseEvent;

/// A tab in the `/model` picker: config models, or a signed-in account's models.
struct ModelTab {
    label: &'static str,
    color: Color,
    models: Vec<String>,
    source: ModelSelectionSource,
}

fn selected_model_location(tabs: &[ModelTab], current: Option<&str>) -> (usize, usize) {
    current
        .and_then(|current| {
            tabs.iter().enumerate().find_map(|(tab_idx, tab)| {
                let current = tab
                    .source
                    .account_provider()
                    .map(|provider| provider.canonical_model(current))
                    .unwrap_or_else(|| current.to_string());
                tab.models
                    .iter()
                    .position(|model| {
                        tab.source
                            .account_provider()
                            .map(|provider| provider.canonical_model(model))
                            .unwrap_or_else(|| model.clone())
                            == current
                    })
                    .map(|model_idx| (tab_idx, model_idx))
            })
        })
        .unwrap_or((0, 0))
}

// Verified provider brand colors used by the `/model` source tabs.
const A3S_COLOR: Color = Color::Rgb(108, 163, 255);
const CLAUDE_COLOR: Color = Color::Rgb(217, 119, 87);
const CODEX_COLOR: Color = Color::Rgb(16, 163, 127);
const KIMI_COLOR: Color = Color::Rgb(23, 131, 255);
const WORKBUDDY_COLOR: Color = Color::Rgb(12, 200, 164);
const OS_GATEWAY_COLOR: Color = Color::Rgb(116, 108, 240);
const CODEX_MODEL_REFRESH_TTL: std::time::Duration = std::time::Duration::from_secs(300);

fn account_provider_color(provider: AccountProvider) -> Color {
    match provider {
        AccountProvider::Claude => CLAUDE_COLOR,
        AccountProvider::Codex => CODEX_COLOR,
        AccountProvider::Kimi => KIMI_COLOR,
        AccountProvider::CodeBuddy => WORKBUDDY_COLOR,
    }
}

fn prompt_guideline_for_effort(
    effort: usize,
    has_native_reasoning_effort: bool,
) -> Option<&'static str> {
    let effort = effort.min(EFFORT_LEVELS.len().saturating_sub(1));
    if effort == ULTRACODE || !has_native_reasoning_effort {
        EFFORT_LEVELS[effort].guideline
    } else {
        None
    }
}

fn model_menu_max_rows(height: usize) -> usize {
    height.saturating_sub(8).clamp(3, 12)
}

fn model_menu_height(tabs: &[ModelTab], active_tab: usize, max_items: usize) -> usize {
    if tabs.is_empty() {
        return 0;
    }
    let active_tab = active_tab.min(tabs.len() - 1);
    let active_items = tabs[active_tab].models.len();
    let header_rows = 1 + usize::from(tabs.len() > 1) + 1;
    let item_rows = active_items.max(1).min(max_items);
    header_rows + item_rows + 1
}

fn model_menu_panel(
    tabs: &[ModelTab],
    active_tab: usize,
    selected: usize,
    current_model: Option<&str>,
    max_items: usize,
) -> TabbedMenuPanel {
    let active_tab = active_tab.min(tabs.len().saturating_sub(1));
    if tabs.is_empty() {
        return TabbedMenuPanel::new(Vec::new());
    }
    let panel_tabs = tabs
        .iter()
        .map(|tab| {
            let items = tab
                .models
                .iter()
                .map(|model| {
                    let prefix = if Some(model.as_str()) == current_model {
                        "●"
                    } else {
                        " "
                    };
                    TabbedMenuItem::new(model.clone()).prefix(prefix)
                })
                .collect::<Vec<_>>();
            TabbedMenuTab::new(tab.label, tab.color)
                .items(items)
                .empty_text("(no models)")
        })
        .collect::<Vec<_>>();

    TabbedMenuPanel::new(panel_tabs)
        .title("Select model")
        .hint("↑/↓ model · ←/→ account · Enter · Esc")
        .active_tab(active_tab)
        .selected(selected)
        .max_items(max_items)
        .indent(2)
        .hint_color(TN_GRAY)
        .text_color(TN_GRAY)
        .muted_color(TN_GRAY)
        .inactive_tabs_use_tab_color(true)
        .active_tab_foreground(Color::Black)
        .selected_colors(TN_FG, SURFACE_SELECTED)
}

fn model_menu_lines(
    tabs: &[ModelTab],
    active_tab: usize,
    selected: usize,
    current_model: Option<&str>,
    width: usize,
    max_items: usize,
) -> Vec<String> {
    if tabs.is_empty() {
        return Vec::new();
    }
    let height = model_menu_height(tabs, active_tab, max_items);
    model_menu_panel(tabs, active_tab, selected, current_model, max_items)
        .view(width.min(u16::MAX as usize) as u16, height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn model_menu_overlay_y_offset(screen_height: usize, row_count: usize, rows_below: usize) -> u16 {
    screen_height
        .saturating_sub(rows_below)
        .saturating_sub(row_count)
        .min(u16::MAX as usize) as u16
}

fn should_fetch_os_gateway_models(
    active_tab: Option<&ModelTab>,
    gateway_models: Option<&[String]>,
    loading: bool,
    signed_in: bool,
) -> bool {
    active_tab.is_some_and(|tab| tab.source == ModelSelectionSource::OsGateway)
        && signed_in
        && !loading
        && gateway_models.is_none_or(|models| models.is_empty())
}

fn should_fetch_account_models(
    active_tab: Option<&ModelTab>,
    cached: bool,
    loading: bool,
    failed: bool,
) -> bool {
    active_tab
        .and_then(|tab| tab.source.account_provider())
        .is_some_and(|provider| provider != AccountProvider::Codex)
        && !cached
        && !loading
        && !failed
}

pub(in crate::tui) async fn rebuild_agent_session(
    agent: Arc<Agent>,
    workspace: String,
    session_id: String,
    with_thinking: SessionOptions,
    without_thinking: SessionOptions,
    mode: SessionRebuildMode,
) -> Result<(AgentSession, bool), String> {
    let mut failures = Vec::new();
    for (thinking, options) in [(true, with_thinking), (false, without_thinking)] {
        let result = match mode {
            SessionRebuildMode::ResumeExisting => agent
                .resume_session_async(&session_id, options)
                .await
                .map_err(|error| format!("resume failed ({error})")),
            SessionRebuildMode::CreateFresh => agent
                .session_async(workspace.clone(), Some(options))
                .await
                .map_err(|error| format!("fresh session failed ({error})")),
        };
        match result {
            Ok(session) => return Ok((session, !thinking)),
            Err(error) => failures.push(format!(
                "{}: {error}",
                if thinking {
                    "with extended thinking"
                } else {
                    "without extended thinking"
                }
            )),
        }
    }

    Err(format!(
        "could not rebuild the session: {}",
        failures.join("; ")
    ))
}

pub(crate) enum SessionRebuildResult {
    Success(AgentSession, bool),
    Failed {
        error: String,
        recovered: Option<AgentSession>,
    },
}

struct SessionRebuildOptions {
    with_thinking: SessionOptions,
    without_thinking: SessionOptions,
}

struct LiveSessionRebuildRequest {
    agent: Arc<Agent>,
    current_session: Arc<AgentSession>,
    workspace: String,
    session_id: String,
    requested: SessionRebuildOptions,
    recovery: SessionRebuildOptions,
}

async fn rebuild_live_agent_session(request: LiveSessionRebuildRequest) -> SessionRebuildResult {
    let LiveSessionRebuildRequest {
        agent,
        current_session,
        workspace,
        session_id,
        requested,
        recovery,
    } = request;
    if let Err(error) = current_session.save().await {
        return SessionRebuildResult::Failed {
            error: format!("could not save the current session before rebuilding: {error}"),
            recovered: None,
        };
    }
    current_session.close().await;
    match rebuild_agent_session(
        Arc::clone(&agent),
        workspace.clone(),
        session_id.clone(),
        requested.with_thinking,
        requested.without_thinking,
        SessionRebuildMode::ResumeExisting,
    )
    .await
    {
        Ok((session, thinking_dropped)) => SessionRebuildResult::Success(session, thinking_dropped),
        Err(error) => {
            let recovered = rebuild_agent_session(
                agent,
                workspace,
                session_id,
                recovery.with_thinking,
                recovery.without_thinking,
                SessionRebuildMode::ResumeExisting,
            )
            .await
            .ok()
            .map(|(session, _)| session);
            SessionRebuildResult::Failed { error, recovered }
        }
    }
}

impl App {
    /// Tabs: a3s-code always; detected local account providers follow.
    fn model_tabs(&self) -> Vec<ModelTab> {
        let mut tabs = vec![ModelTab {
            label: "a3s-code",
            color: A3S_COLOR,
            models: self.models.clone(),
            source: ModelSelectionSource::Config,
        }];
        for provider in AccountProvider::ALL {
            if !provider.is_available() {
                continue;
            }
            let models = if provider == AccountProvider::Codex {
                self.codex_account_models
                    .iter()
                    .map(|model| model.slug.clone())
                    .collect()
            } else {
                self.account_models
                    .get(&provider)
                    .cloned()
                    .unwrap_or_else(|| provider.local_models())
            };
            tabs.push(ModelTab {
                label: provider.label(),
                color: account_provider_color(provider),
                models,
                source: ModelSelectionSource::from_account_provider(provider),
            });
        }
        // Signed in to OS → offer its unified AI gateway (gateway-managed:
        // we send the OS token + a model id; the gateway holds provider keys).
        if self.os_session.is_some() {
            let models = match &self.os_gateway_models {
                Some(m) if !m.is_empty() => m.clone(),
                // Empty: distinguish a fetch failure from a genuinely empty gateway.
                Some(_) => vec![if self.os_gateway_error.is_some() {
                    "(gateway unreachable)".to_string()
                } else {
                    "(no models configured)".to_string()
                }],
                None => vec!["(loading…)".to_string()],
            };
            tabs.push(ModelTab {
                label: "OS Gateway",
                color: OS_GATEWAY_COLOR,
                models,
                source: ModelSelectionSource::OsGateway,
            });
        }
        tabs
    }

    /// Open the /model picker on the current model + matching tab.
    pub(crate) fn open_model_menu(&mut self) {
        let tabs = self.model_tabs();
        if tabs.iter().all(|t| t.models.is_empty()) {
            self.push_line(
                &Style::new()
                    .fg(TN_RED)
                    .render("  no models configured in config.acl"),
            );
            return;
        }
        let (tab, idx) = selected_model_location(&tabs, self.model.as_deref());
        self.model_tab = tab;
        self.model_menu = Some(idx);
    }

    /// Refresh Codex account models without blocking the terminal event loop.
    /// The installed Codex CLI owns login refresh, entitlement filtering, and
    /// client-version negotiation; this TUI only consumes its picker catalog.
    pub(crate) fn maybe_refresh_codex_models(&mut self) -> Option<Cmd<Msg>> {
        if !AccountProvider::Codex.is_available()
            || self.codex_models_loading
            || self
                .codex_models_refreshed_at
                .is_some_and(|at| at.elapsed() < CODEX_MODEL_REFRESH_TTL)
        {
            return None;
        }

        self.codex_models_loading = true;
        Some(cmd::cmd(|| async {
            Msg::CodexModels(
                crate::account_providers::codex::refresh_codex_models()
                    .await
                    .map_err(|error| error.to_string()),
            )
        }))
    }

    pub(crate) fn maybe_fetch_active_account_models(&mut self) -> Option<Cmd<Msg>> {
        let tabs = self.model_tabs();
        let active_tab = self.model_tab.min(tabs.len().saturating_sub(1));
        let provider = tabs
            .get(active_tab)
            .and_then(|tab| tab.source.account_provider())?;
        if !should_fetch_account_models(
            tabs.get(active_tab),
            self.account_models.contains_key(&provider),
            self.account_models_loading.contains(&provider),
            self.account_model_errors.contains_key(&provider),
        ) {
            return None;
        }

        self.account_models_loading.insert(provider);
        Some(cmd::cmd(move || async move {
            Msg::AccountModels {
                provider,
                result: provider
                    .discover_models()
                    .await
                    .map_err(|error| error.to_string()),
            }
        }))
    }

    pub(crate) fn maybe_fetch_active_os_gateway_models(&mut self) -> Option<Cmd<Msg>> {
        let tabs = self.model_tabs();
        let active_tab = self.model_tab.min(tabs.len().saturating_sub(1));
        if !should_fetch_os_gateway_models(
            tabs.get(active_tab),
            self.os_gateway_models.as_deref(),
            self.os_gateway_models_loading,
            self.os_session.is_some(),
        ) {
            return None;
        }
        let session = self.os_session.clone()?;

        self.os_gateway_models_loading = true;
        self.os_gateway_models = None;
        self.os_gateway_error = None;

        let addr = session.address.clone();
        let token = session.access_token.clone();
        let login_at_ms = session.login_at_ms;
        Some(cmd::cmd(move || async move {
            Msg::OsGatewayModels {
                login_at_ms,
                result: crate::a3s_os::fetch_gateway_models(&addr, &token).await,
            }
        }))
    }

    pub(crate) fn maybe_fetch_active_model_models(&mut self) -> Option<Cmd<Msg>> {
        self.maybe_fetch_active_account_models()
            .or_else(|| self.maybe_fetch_active_os_gateway_models())
    }

    pub(crate) fn clamp_open_model_menu_selection(&mut self) {
        let Some(sel) = self.model_menu else {
            return;
        };
        let tabs = self.model_tabs();
        if tabs.is_empty() {
            return;
        }
        self.model_tab = self.model_tab.min(tabs.len() - 1);
        let last = tabs[self.model_tab].models.len().saturating_sub(1);
        self.model_menu = Some(sel.min(last));
    }

    /// Keys while the /model panel is open: ↑/↓ select, ←/→/Tab switch tab,
    /// Enter activate (config model, or sign in with the tab's account), Esc.
    pub(crate) fn handle_model_key(&mut self, key: &KeyEvent) -> Option<Option<Cmd<Msg>>> {
        let sel = self.model_menu?;
        let tabs = self.model_tabs();
        let tab_count = tabs.len().max(1);
        let t = self.model_tab.min(tab_count - 1);
        let last = tabs[t].models.len().saturating_sub(1);
        match key.code {
            KeyCode::Up => {
                self.model_menu = Some(sel.saturating_sub(1));
                Some(None)
            }
            KeyCode::Down => {
                self.model_menu = Some((sel + 1).min(last));
                Some(None)
            }
            KeyCode::Left => {
                self.model_tab = t.saturating_sub(1);
                self.model_menu = Some(0);
                Some(self.maybe_fetch_active_model_models())
            }
            KeyCode::Right | KeyCode::Tab => {
                self.model_tab = (t + 1).min(tab_count - 1);
                self.model_menu = Some(0);
                Some(self.maybe_fetch_active_model_models())
            }
            KeyCode::Enter => Some(self.activate_model_menu_item(&tabs[t], sel.min(last))),
            KeyCode::Esc => {
                self.model_menu = None;
                Some(None)
            }
            _ => None,
        }
    }

    pub(crate) fn handle_model_mouse(&mut self, mouse: &MouseEvent) -> Option<Cmd<Msg>> {
        let sel = self.model_menu?;
        let tabs = self.model_tabs();
        if tabs.is_empty() {
            return None;
        }
        let active_tab = self.model_tab.min(tabs.len() - 1);
        let max_rows = model_menu_max_rows(self.height as usize);
        let selected = sel.min(tabs[active_tab].models.len().saturating_sub(1));
        let width = (self.width as usize).min(u16::MAX as usize);
        let height = model_menu_height(&tabs, active_tab, max_rows);
        let mut panel =
            model_menu_panel(&tabs, active_tab, selected, self.model.as_deref(), max_rows);
        let row_count = panel.view(width as u16, height).lines().count();
        if row_count == 0 {
            return None;
        }
        panel.set_y_offset(model_menu_overlay_y_offset(
            self.height as usize,
            row_count,
            self.overlay_rows_below(),
        ));

        match panel.handle_mouse(mouse) {
            Some(TabbedMenuPanelMsg::TabChanged(tab)) => {
                self.model_tab = tab.min(tabs.len() - 1);
                self.model_menu = Some(0);
                self.maybe_fetch_active_model_models()
            }
            Some(TabbedMenuPanelMsg::Selected { tab, item }) => tabs
                .get(tab)
                .and_then(|tab| self.activate_model_menu_item(tab, item)),
            Some(TabbedMenuPanelMsg::Cancelled) | None => None,
        }
    }

    fn activate_model_menu_item(&mut self, tab: &ModelTab, item: usize) -> Option<Cmd<Msg>> {
        let model = tab.models.get(item).cloned();
        self.model_menu = None;
        match tab.source {
            ModelSelectionSource::Config => model.and_then(|model| self.switch_model(&model)),
            ModelSelectionSource::Claude
            | ModelSelectionSource::Codex
            | ModelSelectionSource::Kimi
            | ModelSelectionSource::CodeBuddy => model.and_then(|model| {
                let provider = tab.source.account_provider()?;
                self.sign_in_account(provider, &model)
            }),
            ModelSelectionSource::OsGateway => model.and_then(|model| self.use_os_gateway(&model)),
        }
    }

    fn active_context_limit_for(&self, model: &str) -> u32 {
        ctx_limit_for_model(&self.model_ctx, model)
    }

    fn commit_model_switch(
        &mut self,
        session: AgentSession,
        model: String,
        source: ModelSelectionSource,
    ) {
        self.replace_session(session);
        let preference = ModelSelectionPreference {
            source,
            model: model.clone(),
        };
        self.model_source = source;
        self.model = Some(model);
        // The next LLM round will report the new prompt fill for the new model.
        // Until then, do not show the previous model's prompt/token counters as
        // if they belonged to this context window.
        self.last_prompt_tokens = 0;
        self.ctx_warned_tier = 0;
        self.output_tokens = 0;
        if let Err(error) = save_model_selection_preference(&preference) {
            self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                "  model switched, but preference was not saved: {error}"
            )));
        }
    }

    /// Switch to a model backed by a detected local developer-tool account.
    fn sign_in_account(&mut self, provider: AccountProvider, model: &str) -> Option<Cmd<Msg>> {
        if provider == AccountProvider::Codex {
            return self.sign_in_codex(model);
        }
        let model = provider.canonical_model(model);
        if self.state != State::Idle {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  finish the current turn before switching models"),
            );
            return None;
        }
        match provider.client(&model, &self.session_id) {
            Ok(client) => {
                let llm_override = Some(LlmOverride::Static(client));
                let context_limit = self.active_context_limit_for(&model);
                let mut profile = self.session_rebuild_profile();
                profile.model = Some(model.clone());
                profile.context_limit = context_limit;
                profile.llm_override = llm_override.clone();
                self.start_session_rebuild(
                    profile,
                    SessionRebuildAction::Model {
                        model,
                        source: ModelSelectionSource::from_account_provider(provider),
                        llm_override,
                        context_limit,
                    },
                )
            }
            Err(error) => {
                self.push_line(&Style::new().fg(TN_RED).render(&format!(
                    "  {} account unavailable: {error}",
                    provider.label()
                )));
                None
            }
        }
    }

    /// Sign in with the local Codex login and switch to one of its models by
    /// injecting the custom Codex client (talks to the ChatGPT backend).
    fn sign_in_codex(&mut self, model: &str) -> Option<Cmd<Msg>> {
        if self.state != State::Idle {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  finish the current turn before switching models"),
            );
            return None;
        }
        match crate::account_providers::codex::CodexClient::from_codex_login_with_effort(
            model,
            &self.session_id,
            EFFORT_LEVELS[self.effort].id,
        ) {
            Ok(client) => {
                let llm_override = Some(LlmOverride::Codex(client));
                let context_limit = self.active_context_limit_for(model);
                let mut profile = self.session_rebuild_profile();
                profile.model = Some(model.to_string());
                profile.context_limit = context_limit;
                profile.llm_override = llm_override.clone();
                self.start_session_rebuild(
                    profile,
                    SessionRebuildAction::Model {
                        model: model.to_string(),
                        source: ModelSelectionSource::Codex,
                        llm_override,
                        context_limit,
                    },
                )
            }
            Err(error) => {
                self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  Codex sign-in failed: {error}")),
                );
                None
            }
        }
    }

    /// Route the agent's LLM through the OS **unified AI gateway**: an
    /// OpenAI-compatible client at the OS authenticated LLM proxy, authed with
    /// the OS Bearer token (the gateway is "gateway-managed" — it holds the real
    /// provider keys). `model` is a gateway model id.
    fn use_os_gateway(&mut self, model: &str) -> Option<Cmd<Msg>> {
        if model.starts_with('(') {
            // A placeholder row. Surface loading, the precise failure reason, or
            // the genuinely-unconfigured gateway state.
            let reason = if self.os_gateway_models_loading || model.contains("loading") {
                "model list is still loading — try again in a moment".to_string()
            } else {
                self.os_gateway_error.clone().unwrap_or_else(|| {
                    "no models configured — set up the unified AI gateway on OS, then reopen the OS Gateway tab"
                        .to_string()
                })
            };
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render(&format!("  OS gateway unavailable: {reason}")),
            );
            return None;
        }
        if self.state != State::Idle {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  finish the current turn before switching models"),
            );
            return None;
        }
        let session = self.os_session.clone()?;
        let llm_override = Some(os_gateway_llm_override(&session, model));
        let context_limit = self.active_context_limit_for(model);
        let mut profile = self.session_rebuild_profile();
        profile.model = Some(model.to_string());
        profile.context_limit = context_limit;
        profile.llm_override = llm_override.clone();
        self.start_session_rebuild(
            profile,
            SessionRebuildAction::Model {
                model: model.to_string(),
                source: ModelSelectionSource::OsGateway,
                llm_override,
                context_limit,
            },
        )
    }

    /// Switch the active model by resuming the session under it (history kept).
    /// Base session options carrying the current effort. `ultracode` adds a
    /// planning + goal tracking + a wider tool-round budget so a turn plans,
    /// then fans independent work out to visible parallel subagents.
    pub(crate) fn effort_session_opts(&self, thinking: bool) -> SessionOptions {
        let profile = SessionRebuildProfile {
            session_id: self.session_id.clone(),
            model: self.model.clone(),
            effort: self.effort,
            context_limit: self.context_limit,
            llm_override: self.llm_override.clone(),
            compact_summary: self.compact_summary.clone(),
        };
        self.session_options_for_profile(thinking, &profile)
    }

    fn session_options_for_profile(
        &self,
        thinking: bool,
        profile: &SessionRebuildProfile,
    ) -> SessionOptions {
        let budget = budget_plan_for_effort_index(
            profile.effort,
            Some(profile.context_limit),
            BudgetWorkload::Interactive,
        );
        let automatic_delegation = effort_uses_automatic_delegation(profile.effort);
        let mut opts = with_recent_workspace_context(
            tui_session_options_with_gate_grants_and_execution(
                self.confirmation.clone(),
                self.deep_research_report_tool_gate.clone(),
                self.permission_grants.clone(),
                self.execution_policy.clone(),
            )
            .with_session_store(self.store.clone())
            .with_session_id(profile.session_id.as_str())
            .with_workspace_backend(self.workspace_services.clone())
            // Includes the login-gated OS `a3s-os-capabilities` skill.
            .with_skill_dirs(self.skill_dirs())
            .with_auto_save(true)
            // Auto-compact against this model's real context window.
            .with_auto_compact(true)
            .with_max_context_tokens(profile.context_limit as usize)
            .with_auto_compact_threshold(self.auto_compact_threshold as f32)
            .with_file_memory(self.memory_dir.clone())
            .with_memory_observer(crate::evolution::EvolutionMemoryObserver::new(
                crate::evolution::WorkspaceEvolution::new(&self.cwd),
            ))
            // The numeric cap remains available to explicit `parallel_task`
            // calls at every effort. Runtime-driven fan-out is a separate
            // ultracode orchestration capability, not a Codex reasoning level.
            .with_max_parallel_tasks(budget.max_parallel_tasks)
            .with_auto_delegation_enabled(automatic_delegation)
            .with_auto_parallel_delegation(automatic_delegation)
            // Pin manual delegation on so `parallel_task`/`task` stay registered
            // even if config.acl disables them — else ultracode's fan-out calls
            // an unregistered tool ("Unknown tool: parallel_task").
            .with_manual_delegation_enabled(true)
            // Tool-round budget scales with effort (low 240 … max 2,400,
            // ultracode 3,200), so long multi-step work and subagents are
            // not cut off by the core's much smaller default.
            .with_max_tool_rounds(budget.max_tool_rounds)
            // Auto-continuation also scales from 4 through 32 turns.
            .with_max_continuation_turns(budget.max_continuation_turns),
            &self.workspace_manifest,
        );
        // Keep project instructions (CLAUDE.md) + any /compact summary across
        // model/effort/compact rebuilds, injected into the system prompt. When
        // signed in, also steer the model to the progressive-API skill for OS
        // questions (else "OS" reads as the local operating system → `whoami`).
        let mut extra_parts: Vec<String> = Vec::new();
        if let Some(i) = &self.instructions {
            extra_parts.push(i.clone());
        }
        if let Some(s) = &profile.compact_summary {
            extra_parts.push(format!("# Earlier conversation (compacted)\n\n{s}"));
        }
        if let Some(s) = &self.os_session {
            extra_parts.push(os_platform_guide(&s.address));
        }
        match crate::evolution::WorkspaceEvolution::new(&self.cwd).session_preference_prompt() {
            Ok(Some(preferences)) => extra_parts.push(preferences),
            Ok(None) => {}
            Err(error) => {
                tracing::warn!(%error, "could not load learned preferences while rebuilding TUI session")
            }
        }
        let extra = (!extra_parts.is_empty()).then(|| extra_parts.join("\n\n"));
        let ultra = profile.effort == ULTRACODE;
        let effort_profile = &EFFORT_LEVELS[profile.effort];
        let codex_effort = profile
            .llm_override
            .as_ref()
            .and_then(|client| client.codex_effort_status(effort_profile.id));
        // The per-level depth steer (low → max, and ultracode's own) — the lever
        // that scales effort on models with no native thinking control. A Codex
        // model with the exact resolved `reasoning.effort` does not need duplicate
        // depth prompting; capped profiles retain guidance, and ultracode keeps
        // its distinct orchestration instructions.
        let has_exact_codex_effort = codex_effort.as_ref().is_some_and(|status| !status.capped);
        let guideline = prompt_guideline_for_effort(profile.effort, has_exact_codex_effort);
        if extra.is_some() || guideline.is_some() {
            let mut slots = SystemPromptSlots::default();
            if let Some(e) = extra {
                slots = slots.with_extra(e);
            }
            if let Some(g) = guideline {
                slots = slots.with_guidelines(g);
            }
            opts = opts.with_prompt_slots(slots);
        }
        // Extended thinking is Anthropic-only; only request it when asked.
        if thinking {
            opts = opts.with_thinking_budget(budget.thinking_budget);
        }
        if self.goal_run.is_some() {
            // A durable `/goal` is the one mode that forces a plan on every
            // iteration. Ordinary Ultracode remains message-gated so trivial
            // turns never fan out merely because the profile is selected.
            opts = opts
                .with_planning_mode(a3s_code_core::PlanningMode::Enabled)
                .with_goal_tracking(true);
        } else if ultra {
            // Dynamic-workflow mode: planning is message-gated (Auto), so a turn
            // plans + fans out only when the core's pre-analysis judges the task to
            // warrant it — a trivial "hi" stays a direct answer. `Enabled` forced a
            // plan every turn, which is what made ultracode explore on a greeting.
            // A3S Flow is registered below as the durable dynamic-workflow runtime.
            opts = opts
                .with_planning_mode(a3s_code_core::PlanningMode::Auto)
                .with_goal_tracking(true);
        }
        if let Some(model) = &profile.model {
            opts = opts.with_model(model);
        }
        // Signed in via a /model account tab → route through that account
        // client. Config-backed models are also host-created so a3s-code v5.2.2
        // retains the provider's verified structured-output capability during
        // launch, model switches, and effort rebuilds. Unknown custom endpoints
        // deliberately keep the safe prompt fallback.
        if let Some(client) = &profile.llm_override {
            opts = opts.with_llm_client(client.client_for_effort(effort_profile.id));
        } else if let Ok(client) = crate::session_llm::resolve_config_llm_client(
            &self.code_config,
            &opts,
            profile.session_id.as_str(),
        ) {
            opts = opts.with_llm_client(client);
        }
        opts
    }

    pub(crate) fn codex_effort_status_for_index(&self, effort: usize) -> Option<CodexEffortStatus> {
        let profile = &EFFORT_LEVELS[effort.min(EFFORT_LEVELS.len().saturating_sub(1))];
        self.llm_override
            .as_ref()
            .and_then(|client| client.codex_effort_status(profile.id))
    }

    /// Start an async session rebuild. Session stores, file-backed memory, MCP,
    /// and queue resources are async-only; never route a TUI rebuild through the
    /// synchronous compatibility API.
    pub(crate) fn start_session_rebuild(
        &mut self,
        profile: SessionRebuildProfile,
        action: SessionRebuildAction,
    ) -> Option<Cmd<Msg>> {
        if self.session_rebuild_pending.is_some() {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  wait for the current session change to finish"),
            );
            return None;
        }

        let with_thinking = self.session_options_for_profile(true, &profile);
        let without_thinking = self.session_options_for_profile(false, &profile);
        let recovery_profile = self.session_rebuild_profile();
        let recovery_with_thinking = self.session_options_for_profile(true, &recovery_profile);
        let recovery_without_thinking = self.session_options_for_profile(false, &recovery_profile);
        let mode = match &action {
            SessionRebuildAction::Compact { .. } | SessionRebuildAction::Clear { .. } => {
                SessionRebuildMode::CreateFresh
            }
            _ => SessionRebuildMode::ResumeExisting,
        };
        self.session_rebuild_seq = self.session_rebuild_seq.wrapping_add(1);
        let request_id = self.session_rebuild_seq;
        self.session_rebuild_pending = Some(request_id);

        let agent = Arc::clone(&self.agent);
        let current_session = Arc::clone(&self.session);
        let workspace = self.cwd.clone();
        let session_id = profile.session_id;
        Some(cmd::cmd(move || async move {
            let result = if mode == SessionRebuildMode::ResumeExisting {
                rebuild_live_agent_session(LiveSessionRebuildRequest {
                    agent,
                    current_session,
                    workspace,
                    session_id,
                    requested: SessionRebuildOptions {
                        with_thinking,
                        without_thinking,
                    },
                    recovery: SessionRebuildOptions {
                        with_thinking: recovery_with_thinking,
                        without_thinking: recovery_without_thinking,
                    },
                })
                .await
            } else {
                match rebuild_agent_session(
                    agent,
                    workspace,
                    session_id,
                    with_thinking,
                    without_thinking,
                    mode,
                )
                .await
                {
                    Ok((session, thinking_dropped)) => {
                        SessionRebuildResult::Success(session, thinking_dropped)
                    }
                    Err(error) => SessionRebuildResult::Failed {
                        error,
                        recovered: None,
                    },
                }
            };
            Msg::SessionRebuilt {
                request_id,
                action,
                result: Box::new(result),
            }
        }))
    }

    pub(crate) fn session_rebuild_profile(&self) -> SessionRebuildProfile {
        SessionRebuildProfile {
            session_id: self.session_id.clone(),
            model: self.model.clone(),
            effort: self.effort,
            context_limit: self.context_limit,
            llm_override: self.llm_override.clone(),
            compact_summary: self.compact_summary.clone(),
        }
    }

    pub(crate) fn finish_session_rebuild(
        &mut self,
        request_id: u64,
        action: SessionRebuildAction,
        result: SessionRebuildResult,
    ) -> Option<Cmd<Msg>> {
        if self.session_rebuild_pending != Some(request_id) {
            return None;
        }
        self.session_rebuild_pending = None;

        let (session, thinking_dropped) = match result {
            SessionRebuildResult::Success(session, thinking_dropped) => (session, thinking_dropped),
            SessionRebuildResult::Failed { error, recovered } => {
                if let Some(session) = recovered {
                    self.replace_session(session);
                }
                match &action {
                    SessionRebuildAction::GoalStart {
                        generation,
                        previous_effort,
                        previous_goal,
                        previous_goal_since,
                    } => {
                        let still_active = self
                            .goal_run
                            .as_ref()
                            .is_some_and(|run| run.is_generation(*generation));
                        self.goal_run = None;
                        self.effort = *previous_effort;
                        if still_active {
                            self.goal = previous_goal.clone();
                            self.goal_since = *previous_goal_since;
                            self.push_line(
                                &Style::new().fg(TN_RED).render(&format!(
                                    "  /goal could not enable Ultracode: {error}"
                                )),
                            );
                        }
                        return self.drain_queue();
                    }
                    SessionRebuildAction::GoalResume { generation, paused } => {
                        let still_active = self
                            .goal_run
                            .as_ref()
                            .is_some_and(|run| run.is_generation(*generation));
                        if still_active {
                            self.goal_run = None;
                            self.goal = None;
                            self.goal_since = None;
                            self.paused_goal = Some(paused.clone());
                            self.goal_resume_prompt = Some(0);
                            self.push_line(
                                &Style::new().fg(TN_RED).render(&format!(
                                    "  paused goal could not be resumed: {error}"
                                )),
                            );
                        }
                        return self.drain_queue();
                    }
                    SessionRebuildAction::GoalRestore => {
                        self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                            "  goal stopped, but session mode refresh failed: {error}"
                        )));
                        return self.drain_queue();
                    }
                    SessionRebuildAction::Rewind {
                        session_id,
                        files_rewound,
                        ..
                    } => {
                        let file_note = if *files_rewound {
                            " Workspace files were rewound safely."
                        } else {
                            ""
                        };
                        self.push_line(&Style::new().fg(TN_RED).render(&format!(
                            "  rewound session could not be opened: {error}.{file_note} \
                                 Resume the saved conversation with: a3s code resume {session_id}"
                        )));
                        return self.drain_queue();
                    }
                    SessionRebuildAction::EvolutionRuntime {
                        stream_token,
                        synthesis,
                        ..
                    } => {
                        self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                                "  learned assets were saved, but the current session could not refresh: {error}"
                            )));
                        return self.start_rewind_checkpoint_finalization(
                            *stream_token,
                            synthesis.clone(),
                        );
                    }
                    _ => {}
                }
                if matches!(action, SessionRebuildAction::Compact { .. }) {
                    self.compacting = None;
                }
                let context = match action {
                    SessionRebuildAction::Model { .. } => "switch model",
                    SessionRebuildAction::Effort { .. } => "set effort",
                    SessionRebuildAction::GoalStart { .. } => unreachable!("handled above"),
                    SessionRebuildAction::GoalResume { .. } => unreachable!("handled above"),
                    SessionRebuildAction::GoalRestore => unreachable!("handled above"),
                    SessionRebuildAction::Compact { .. } => "compact context",
                    SessionRebuildAction::Fork { .. } => "fork session",
                    SessionRebuildAction::Rewind { .. } => "resume rewound session",
                    SessionRebuildAction::Relay { .. } => "resume relay session",
                    SessionRebuildAction::Clear { .. } => "clear session",
                    SessionRebuildAction::Reload { .. } => "reload session",
                    SessionRebuildAction::EvolutionRuntime { .. } => {
                        "activate learned session assets"
                    }
                    SessionRebuildAction::Refresh { failure_context } => failure_context?,
                };
                self.push_line(
                    &Style::new()
                        .fg(TN_RED)
                        .render(&format!("  failed to {context}: {error}")),
                );
                return None;
            }
        };

        match action {
            SessionRebuildAction::Model {
                model,
                source,
                llm_override,
                context_limit,
            } => {
                self.llm_override = llm_override;
                self.context_limit = context_limit;
                self.commit_model_switch(session, model.clone(), source);
                let label = match source {
                    ModelSelectionSource::Config => format!("switched to {model}"),
                    ModelSelectionSource::Claude => format!("Claude Code · {model}"),
                    ModelSelectionSource::Codex => format!("Codex · {model}"),
                    ModelSelectionSource::Kimi => format!("Kimi · {model}"),
                    ModelSelectionSource::CodeBuddy => format!("WorkBuddy · {model}"),
                    ModelSelectionSource::OsGateway => format!("OS Gateway · {model}"),
                };
                self.push_notice(NoticeKind::Success, label);
            }
            SessionRebuildAction::Effort {
                selected,
                codex_effort,
            } => {
                self.effort = selected;
                self.replace_session(session);
                if let Err(error) = save_tui_effort_preference(selected) {
                    self.push_line(&Style::new().fg(TN_YELLOW).render(&format!(
                        "  effort changed, but preference was not saved: {error}"
                    )));
                }
                if selected == ULTRACODE {
                    self.set_composer_mode(Mode::Auto);
                    self.gradient_until = Some(Instant::now());
                    self.gradient_frame = 0;
                    let native = codex_effort
                        .as_ref()
                        .map(|status| {
                            let cap = if status.capped { " (model limit)" } else { "" };
                            format!(" · Codex reasoning: {}{cap}", status.effective)
                        })
                        .unwrap_or_default();
                    self.push_line(&Style::new().fg(ACCENT).bold().render(&format!(
                        "  ◆ ultracode — planning a dynamic workflow + parallel subagents (non-interactive Auto on){native}",
                    )));
                } else if let Some(status) = codex_effort {
                    let cap = if status.capped { " (model limit)" } else { "" };
                    self.push_line(&Style::new().fg(TN_GREEN).render(&format!(
                        "  ◇ effort: {} · Codex reasoning: {}{cap}",
                        EFFORT_LEVELS[selected].label, status.effective
                    )));
                } else if thinking_dropped {
                    let note = if EFFORT_LEVELS[selected].guideline.is_some() {
                        "depth via reasoning guidance; no extended-thinking on this model"
                    } else {
                        "balanced baseline; no extended-thinking on this model"
                    };
                    self.push_line(&Style::new().fg(TN_GREEN).render(&format!(
                        "  ◇ effort: {} ({note})",
                        EFFORT_LEVELS[selected].label
                    )));
                } else {
                    self.push_line(
                        &Style::new()
                            .fg(TN_GREEN)
                            .render(&format!("  ◇ effort: {}", EFFORT_LEVELS[selected].label)),
                    );
                }
            }
            SessionRebuildAction::GoalStart { generation, .. } => {
                self.replace_session(session);
                self.effort = ULTRACODE;
                if self
                    .goal_run
                    .as_ref()
                    .is_some_and(|run| run.is_generation(generation))
                {
                    return self.finish_goal_start();
                }
                return self.restore_goal_planning_mode();
            }
            SessionRebuildAction::GoalResume { generation, paused } => {
                self.replace_session(session);
                if self
                    .goal_run
                    .as_ref()
                    .is_some_and(|run| run.is_generation(generation))
                {
                    self.effort = ULTRACODE;
                    return self.finish_goal_resume();
                }
                self.paused_goal = Some(paused);
                self.goal_resume_prompt = Some(0);
                return self.restore_goal_planning_mode();
            }
            SessionRebuildAction::GoalRestore => {
                self.replace_session(session);
                return self.drain_queue();
            }
            SessionRebuildAction::Compact {
                summary,
                session_id,
            } => {
                self.compacting = None;
                self.compact_summary = Some(summary);
                self.session_id = session_id;
                self.replace_session(session);
                self.messages.clear();
                self.output_tokens = 0;
                self.last_prompt_tokens = 0;
                self.ctx_warned_tier = 0;
                self.push_line(
                    &Style::new()
                        .fg(TN_GREEN)
                        .bold()
                        .render("  ✦ context compacted — continuing from this summary:"),
                );
                self.push_line(&gutter(
                    TN_CYAN,
                    self.compact_summary.as_deref().unwrap_or(""),
                ));
                self.rebuild_viewport();
            }
            SessionRebuildAction::Fork { session_id } => {
                self.session_id = session_id;
                self.replace_session(session);
                let short: String = self.session_id.chars().take(8).collect();
                self.push_line(&gutter(
                    TN_CYAN,
                    &format!("⑂ forked into a new session ({short}) — the original is kept"),
                ));
            }
            SessionRebuildAction::Rewind {
                session_id,
                files_rewound,
                warning,
            } => {
                let history = session.history();
                let entries = app_launch::resumed_transcript_entries(&history);
                self.restore_autonomy();
                self.session_id = session_id;
                self.replace_session(session);
                self.messages = Transcript::from_entries(entries);
                self.compact_summary = None;
                self.plan.clear();
                self.runtime.clear_turn_entities();
                self.runtime.clear_subagent_entities();
                self.active_rewind_checkpoint = None;
                self.rewind_checkpoints.clear();
                self.rewind_finalization_pending = None;
                self.output_tokens = 0;
                self.last_prompt_tokens = 0;
                self.ctx_warned_tier = 0;
                let scope = if files_rewound {
                    "conversation and workspace"
                } else {
                    "conversation"
                };
                self.push_line(&gutter(
                    TN_CYAN,
                    &format!("↶ rewound the last turn ({scope}) — original session kept"),
                ));
                if let Some(warning) = warning {
                    self.push_line(
                        &Style::new()
                            .fg(TN_YELLOW)
                            .render(&format!("  rewind note: {warning}")),
                    );
                }
                self.rebuild_viewport();
            }
            SessionRebuildAction::Relay { restore } => {
                self.commit_relay_session(session, restore);
            }
            SessionRebuildAction::Clear { session_id } => {
                // Commit the UI reset only after the fresh session exists. A
                // failed `/clear` must leave the old conversation untouched.
                self.restore_autonomy();
                self.session_id = session_id;
                self.replace_session(session);
                self.compact_summary = None;
                self.messages.clear();
                self.plan.clear();
                self.runtime.clear_turn_entities();
                self.runtime.clear_subagent_entities();
                self.queue.clear();
                self.queued_turn_modes.clear();
                self.queued_plan_drafts.clear();
                self.send_now_queued_sequence = None;
                self.queue_panel = None;
                self.active_queued_turn = None;
                self.active_queued_turn_token = None;
                self.active_turn_mode = None;
                self.active_plan_draft = None;
                self.pending_plan_review = None;
                self.plan_review = None;
                self.queue_retry_generation = self.queue_retry_generation.wrapping_add(1);
                self.queue_retry_attempt = 0;
                self.completed = 0;
                self.review_pending = false;
                self.sleep_pending = false;
                self.goal = None;
                self.goal_since = None;
                self.goal_run = None;
                self.pending_goal_failure = None;
                self.pending_ctx = None;
                self.ctx_hits.clear();
                self.agent_dev = None;
                self.pending_flow_subcommand = None;
                self.pending_agent_subcommand = None;
                self.mcp_dev = None;
                self.pending_mcp_subcommand = None;
                self.skill_dev = None;
                self.pending_skill_subcommand = None;
                self.okf_picker = None;
                self.pending_okf_subcommand = None;
                self.okf_dev = None;
                self.asset_list = None;
                self.runtime_activity = None;
                self.kb = None;
                self.output_tokens = 0;
                self.last_prompt_tokens = 0;
                self.ctx_warned_tier = 0;
                self.relayout();
                self.rebuild_viewport();
            }
            SessionRebuildAction::Reload { skill_count } => {
                self.replace_session(session);
                self.push_line(
                    &Style::new()
                        .fg(TN_GREEN)
                        .render(&format!("  ↻ reloaded — {skill_count} skills available")),
                );
            }
            SessionRebuildAction::EvolutionRuntime {
                pending_assets,
                stream_token,
                synthesis,
            } => {
                self.replace_session(session);
                self.push_line(&Style::new().fg(TN_GREEN).render(&format!(
                    "  ↻ activated {pending_assets} learned session asset(s)"
                )));
                return self.start_rewind_checkpoint_finalization(stream_token, synthesis);
            }
            SessionRebuildAction::Refresh { .. } => self.replace_session(session),
        }
        self.drain_queue()
    }

    pub(crate) fn switch_model(&mut self, model: &str) -> Option<Cmd<Msg>> {
        if self.state != State::Idle {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  finish the current turn before switching models"),
            );
            return None;
        }
        let context_limit = self.active_context_limit_for(model);
        let mut profile = self.session_rebuild_profile();
        profile.model = Some(model.to_string());
        profile.context_limit = context_limit;
        profile.llm_override = None;
        self.start_session_rebuild(
            profile,
            SessionRebuildAction::Model {
                model: model.to_string(),
                source: ModelSelectionSource::Config,
                llm_override: None,
                context_limit,
            },
        )
    }

    /// Apply a selected effort by rebuilding the session (keeps model + history).
    /// The old profile remains active if the rebuild fails.
    pub(crate) fn apply_effort(&mut self, selected: usize) -> Option<Cmd<Msg>> {
        if self.state != State::Idle {
            self.push_line(
                &Style::new()
                    .fg(TN_YELLOW)
                    .render("  finish the current turn before changing effort"),
            );
            return None;
        }
        let selected = selected.min(EFFORT_LEVELS.len().saturating_sub(1));
        let codex_effort = self.codex_effort_status_for_index(selected);
        let mut profile = self.session_rebuild_profile();
        profile.effort = selected;
        self.start_session_rebuild(
            profile,
            SessionRebuildAction::Effort {
                selected,
                codex_effort,
            },
        )
    }

    pub(crate) fn overlay_model_menu(&self, composed: String) -> String {
        let Some(sel) = self.model_menu else {
            return composed;
        };
        let tabs = self.model_tabs();
        if tabs.is_empty() {
            return composed;
        }
        let t = self.model_tab.min(tabs.len() - 1);
        let width = self.width as usize;
        // Scroll a window around the selection so a pick past row 12 stays visible
        // and reachable (the list used to render a fixed first-12 only).
        let max_rows = model_menu_max_rows(self.height as usize);
        let sel = sel.min(tabs[t].models.len().saturating_sub(1));
        let menu = model_menu_lines(&tabs, t, sel, self.model.as_deref(), width, max_rows);
        self.overlay_list(composed, &menu)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn provider_brand_tabs() -> Vec<ModelTab> {
        [
            ("a3s-code", A3S_COLOR),
            ("Claude Code", CLAUDE_COLOR),
            ("Codex", CODEX_COLOR),
            ("Kimi", KIMI_COLOR),
            ("WorkBuddy", WORKBUDDY_COLOR),
            ("OS Gateway", OS_GATEWAY_COLOR),
        ]
        .into_iter()
        .map(|(label, color)| ModelTab {
            label,
            color,
            models: vec!["provider-model".into()],
            source: ModelSelectionSource::Config,
        })
        .collect()
    }

    #[test]
    fn model_provider_brand_colors_match_verified_rgb_values() {
        assert_eq!(A3S_COLOR, Color::Rgb(108, 163, 255));
        assert_eq!(CLAUDE_COLOR, Color::Rgb(217, 119, 87));
        assert_eq!(CODEX_COLOR, Color::Rgb(16, 163, 127));
        assert_eq!(KIMI_COLOR, Color::Rgb(23, 131, 255));
        assert_eq!(WORKBUDDY_COLOR, Color::Rgb(12, 200, 164));
        assert_eq!(OS_GATEWAY_COLOR, Color::Rgb(116, 108, 240));
        assert_eq!(
            account_provider_color(AccountProvider::Claude),
            CLAUDE_COLOR
        );
        assert_eq!(account_provider_color(AccountProvider::Codex), CODEX_COLOR);
        assert_eq!(account_provider_color(AccountProvider::Kimi), KIMI_COLOR);
        assert_eq!(
            account_provider_color(AccountProvider::CodeBuddy),
            WORKBUDDY_COLOR
        );
    }

    #[test]
    fn model_menu_inactive_tabs_keep_provider_brand_colors() {
        let tabs = provider_brand_tabs();
        let height = model_menu_height(&tabs, 0, 3);

        for active_tab in 0..tabs.len() {
            let rendered = model_menu_panel(&tabs, active_tab, 0, None, 3).view(160, height);
            for (tab_index, tab) in tabs.iter().enumerate() {
                if tab_index == active_tab {
                    continue;
                }
                let chip = format!(" {} ", tab.label);
                let expected = Style::new().fg(tab.color).render(&chip);
                assert!(
                    rendered.contains(&expected),
                    "inactive {} tab lost its brand color: {:?}",
                    tab.label,
                    a3s_tui::style::strip_ansi(&rendered)
                );
            }
        }
    }

    #[test]
    fn model_menu_active_tabs_use_dark_foreground_on_brand_backgrounds() {
        let tabs = provider_brand_tabs();
        let height = model_menu_height(&tabs, 0, 3);

        for (active_tab, tab) in tabs.iter().enumerate() {
            let rendered = model_menu_panel(&tabs, active_tab, 0, None, 3).view(160, height);
            let chip = format!(" {} ", tab.label);
            let expected = Style::new()
                .fg(Color::Black)
                .bg(tab.color)
                .bold()
                .render(&chip);
            assert!(
                rendered.contains(&expected),
                "active {} tab did not use dark text on its brand background: {:?}",
                tab.label,
                a3s_tui::style::strip_ansi(&rendered)
            );
        }
    }

    #[tokio::test]
    async fn async_rebuild_initializes_default_file_memory() {
        let root = std::env::temp_dir().join(format!(
            "a3s-async-rebuild-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let config = root.join("config.acl");
        std::fs::write(
            &config,
            "default_model = \"openai/x\"\n\
             providers \"openai\" {\n  apiKey = \"x\"\n  baseUrl = \"http://127.0.0.1:1\"\n  \
             models \"x\" { name = \"x\" }\n}\n",
        )
        .unwrap();
        let agent = Arc::new(
            Agent::new(config.to_string_lossy().to_string())
                .await
                .unwrap(),
        );
        let options = SessionOptions::new().with_session_id("async-rebuild-memory");

        let (session, thinking_dropped) = rebuild_agent_session(
            agent,
            workspace.to_string_lossy().to_string(),
            "async-rebuild-memory".to_string(),
            options.clone(),
            options,
            SessionRebuildMode::CreateFresh,
        )
        .await
        .unwrap();

        assert_eq!(session.session_id(), "async-rebuild-memory");
        assert!(!thinking_dropped);
        assert!(workspace.join(".a3s/memory").is_dir());
        let _ = std::fs::remove_dir_all(root);
    }

    #[tokio::test]
    async fn live_session_rebuild_closes_before_resuming_same_id() {
        let root = std::env::temp_dir().join(format!(
            "a3s-live-session-rebuild-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let workspace = root.join("workspace");
        std::fs::create_dir_all(&workspace).unwrap();
        let config = root.join("config.acl");
        std::fs::write(
            &config,
            "default_model = \"openai/x\"\n\
             providers \"openai\" {\n  apiKey = \"x\"\n  baseUrl = \"http://127.0.0.1:1\"\n  \
             models \"x\" { name = \"x\" }\n}\n",
        )
        .unwrap();
        let agent = Arc::new(
            Agent::new(config.to_string_lossy().to_string())
                .await
                .unwrap(),
        );
        let session_id = "live-session-rebuild";
        let store: Arc<dyn a3s_code_core::store::SessionStore> = Arc::new(
            a3s_code_core::store::FileSessionStore::new(root.join("sessions"))
                .await
                .unwrap(),
        );
        let options = SessionOptions::new()
            .with_session_id(session_id)
            .with_session_store(store);
        let current = Arc::new(
            agent
                .session_async(
                    workspace.to_string_lossy().to_string(),
                    Some(options.clone()),
                )
                .await
                .unwrap(),
        );

        let result = rebuild_live_agent_session(LiveSessionRebuildRequest {
            agent: Arc::clone(&agent),
            current_session: current,
            workspace: workspace.to_string_lossy().to_string(),
            session_id: session_id.to_string(),
            requested: SessionRebuildOptions {
                with_thinking: options.clone(),
                without_thinking: options.clone(),
            },
            recovery: SessionRebuildOptions {
                with_thinking: options.clone(),
                without_thinking: options,
            },
        })
        .await;

        let session = match result {
            SessionRebuildResult::Success(session, _) => session,
            SessionRebuildResult::Failed { error, recovered } => panic!(
                "same-id live session rebuild should succeed after closing the old session: {error}; recovered={}",
                recovered.is_some()
            ),
        };
        assert_eq!(session.session_id(), session_id);
        assert_eq!(agent.list_sessions().await, vec![session_id.to_string()]);
        session.close().await;
        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn native_codex_effort_replaces_only_simulated_depth_guidance() {
        assert!(prompt_guideline_for_effort(2, true).is_none());
        assert_eq!(
            prompt_guideline_for_effort(2, false),
            EFFORT_LEVELS[2].guideline
        );
        assert_eq!(
            prompt_guideline_for_effort(ULTRACODE, true),
            EFFORT_LEVELS[ULTRACODE].guideline
        );
        assert_eq!(
            prompt_guideline_for_effort(4, false),
            EFFORT_LEVELS[4].guideline
        );
    }

    #[test]
    fn selected_model_location_finds_account_tab_model() {
        let tabs = vec![
            ModelTab {
                label: "a3s-code",
                color: A3S_COLOR,
                models: vec!["openai/gpt-5".into()],
                source: ModelSelectionSource::Config,
            },
            ModelTab {
                label: "Claude Code",
                color: CLAUDE_COLOR,
                models: vec!["claude-sonnet-4".into()],
                source: ModelSelectionSource::Claude,
            },
        ];

        assert_eq!(
            selected_model_location(&tabs, Some("claude-sonnet-4")),
            (1, 0)
        );
        assert_eq!(
            selected_model_location(&tabs, Some("claude-sonnet-4[1m]")),
            (1, 0)
        );
        assert_eq!(selected_model_location(&tabs, Some("missing")), (0, 0));
    }

    #[test]
    fn os_gateway_models_fetch_only_when_gateway_tab_is_active() {
        let config_tab = ModelTab {
            label: "a3s-code",
            color: A3S_COLOR,
            models: vec!["openai/gpt-5".into()],
            source: ModelSelectionSource::Config,
        };
        let gateway_tab = ModelTab {
            label: "OS Gateway",
            color: OS_GATEWAY_COLOR,
            models: vec!["(loading…)".into()],
            source: ModelSelectionSource::OsGateway,
        };
        let cached = vec!["gpt-5.1".to_string()];

        assert!(!should_fetch_os_gateway_models(
            Some(&config_tab),
            None,
            false,
            true
        ));
        assert!(!should_fetch_os_gateway_models(
            Some(&gateway_tab),
            None,
            false,
            false
        ));
        assert!(!should_fetch_os_gateway_models(
            Some(&gateway_tab),
            None,
            true,
            true
        ));
        assert!(!should_fetch_os_gateway_models(
            Some(&gateway_tab),
            Some(&cached),
            false,
            true
        ));
        assert!(should_fetch_os_gateway_models(
            Some(&gateway_tab),
            None,
            false,
            true
        ));
        assert!(should_fetch_os_gateway_models(
            Some(&gateway_tab),
            Some(&[]),
            false,
            true
        ));
    }

    #[test]
    fn account_models_fetch_once_for_cli_backed_account_tabs() {
        let config_tab = ModelTab {
            label: "a3s-code",
            color: A3S_COLOR,
            models: vec!["openai/gpt-5".into()],
            source: ModelSelectionSource::Config,
        };
        let workbuddy_tab = ModelTab {
            label: "WorkBuddy",
            color: WORKBUDDY_COLOR,
            models: vec!["auto".into()],
            source: ModelSelectionSource::CodeBuddy,
        };
        let kimi_tab = ModelTab {
            label: "Kimi",
            color: KIMI_COLOR,
            models: vec!["k3-agent".into()],
            source: ModelSelectionSource::Kimi,
        };
        let codex_tab = ModelTab {
            label: "Codex",
            color: CODEX_COLOR,
            models: vec!["gpt-5.6-sol".into()],
            source: ModelSelectionSource::Codex,
        };

        assert!(!should_fetch_account_models(
            Some(&config_tab),
            false,
            false,
            false
        ));
        assert!(should_fetch_account_models(
            Some(&workbuddy_tab),
            false,
            false,
            false
        ));
        assert!(should_fetch_account_models(
            Some(&kimi_tab),
            false,
            false,
            false
        ));
        assert!(!should_fetch_account_models(
            Some(&workbuddy_tab),
            true,
            false,
            false
        ));
        assert!(!should_fetch_account_models(
            Some(&workbuddy_tab),
            false,
            true,
            false
        ));
        assert!(!should_fetch_account_models(
            Some(&workbuddy_tab),
            false,
            false,
            true
        ));
        assert!(!should_fetch_account_models(
            Some(&codex_tab),
            false,
            false,
            false
        ));
    }

    #[test]
    fn model_menu_lines_are_width_bounded_with_styles() {
        let lines = model_menu_lines(
            &[ModelTab {
                label: "Codex",
                color: CODEX_COLOR,
                models: vec![
                    "openai-compatible/provider/model-name-with-a-very-long-context-window".into(),
                    "gpt-5-codex".into(),
                ],
                source: ModelSelectionSource::Codex,
            }],
            0,
            0,
            Some("openai-compatible/provider/model-name-with-a-very-long-context-window"),
            36,
            3,
        );

        for line in lines {
            assert!(
                a3s_tui::style::visible_len(&line) <= 36,
                "{}",
                a3s_tui::style::strip_ansi(&line)
            );
        }
    }

    #[test]
    fn model_menu_panel_handles_tab_mouse_with_overlay_offset() {
        use a3s_tui::event::{MouseButton, MouseEventKind};

        let tabs = vec![
            ModelTab {
                label: "a3s-code",
                color: A3S_COLOR,
                models: vec!["openai/gpt-5".into()],
                source: ModelSelectionSource::Config,
            },
            ModelTab {
                label: "Claude Code",
                color: CLAUDE_COLOR,
                models: vec!["claude-sonnet-4".into()],
                source: ModelSelectionSource::Claude,
            },
        ];
        let max_rows = model_menu_max_rows(24);
        let row_count = model_menu_lines(&tabs, 0, 0, None, 48, max_rows).len();
        let y_offset = model_menu_overlay_y_offset(24, row_count, 5);
        let mut panel = model_menu_panel(&tabs, 0, 0, None, max_rows);
        panel.set_y_offset(y_offset);

        let msg = panel.handle_mouse(&MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: y_offset + 1,
            modifiers: a3s_tui::KeyModifiers::NONE,
        });

        assert_eq!(msg, Some(TabbedMenuPanelMsg::TabChanged(1)));
    }

    #[test]
    fn model_menu_mouse_offset_follows_dynamic_rows_below() {
        assert_eq!(model_menu_overlay_y_offset(24, 6, 5), 13);
        assert_eq!(model_menu_overlay_y_offset(24, 6, 9), 9);
    }
}
