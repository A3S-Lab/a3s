//! Sentinel — security observer agent for SafeClaw.
//!
//! The sentinel sits between all agent sessions and the outside world,
//! intercepting hook events to perform intrinsic security analysis.  It is
//! composed of two cooperating analysis phases:
//!
//! - **Phase 1** (< 5 ms): rule-based checks compiled from `policy.hcl`.
//! - **Phase 2** (< 2 s): LLM-based analysis via a dedicated sentinel session;
//!   triggered only when Phase 1 score exceeds the configured threshold.
//!
//! ## User customisation
//!
//! Drop files into `~/.config/safeclaw/sentinel/`:
//!
//! ```text
//! sentinel/
//!   policy.hcl          — security policy (block patterns, tool lists, etc.)
//!   skills/             — custom security analysis skills (.md files)
//!   agents/             — specialist sub-agent definitions (.yaml / .md)
//! ```
//!
//! ## Integration
//!
//! Call [`SentinelAgent::init`] at startup, then [`SentinelAgent::register_hooks`]
//! to wire the blocking hooks into an existing `HookEngine`.

pub mod accumulator;
pub mod config;
pub mod correlator;
pub mod hook;
pub mod lineage;
pub mod phase1;
pub mod phase2_pool;
pub mod sync;

pub use config::{DelegateTrigger, SentinelPolicy};
pub use phase1::{Phase1Engine, Phase1Result};

use accumulator::{AccumulatorDecision, SuspicionAccumulatorMap};
use correlator::{CorrelatedEvent, CorrelatedEventType, CrossSessionCorrelator};
use lineage::LineageMap;
use phase2_pool::Phase2Pool;

use a3s_code::hooks::{Hook, HookConfig, HookEngine, HookEventType};

// ── SentinelObserver trait ────────────────────────────────────────────────────

/// Unified interface for both in-process and out-of-process sentinel observers.
///
/// Implemented by:
/// - [`SentinelAgent`] — in-process (used inside the daemon child process)
/// - [`SentinelDaemon`] — out-of-process (communicates over Unix Domain Socket)
#[async_trait::async_trait]
pub trait SentinelObserver: Send + Sync + std::fmt::Debug {
    async fn analyze_pre_tool_use(
        &self,
        session_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> SecurityVerdict;

    async fn analyze_prompt(&self, session_id: &str, prompt: &str) -> SecurityVerdict;

    async fn analyze_response(&self, response_text: &str) -> SecurityVerdict;

    fn on_session_created(&self, session_id: &str, parent_id: Option<&str>);

    fn on_session_destroyed(&self, session_id: &str);
}

/// Register the three blocking sentinel hooks on `hook_engine`.
///
/// Extracted as a free function so both `SentinelAgent` and `SentinelDaemon`
/// can reuse it without duplication.
pub fn register_sentinel_hooks(
    observer: Arc<dyn SentinelObserver>,
    hook_engine: &HookEngine,
) {
    use hook::SentinelHookHandler;
    let handler = Arc::new(SentinelHookHandler::new(observer));

    let pre_tool = Hook::new(HOOK_ID_PRE_TOOL, HookEventType::PreToolUse)
        .with_config(HookConfig { priority: 1, timeout_ms: 5000, ..Default::default() });
    hook_engine.register(pre_tool);
    hook_engine.register_handler(HOOK_ID_PRE_TOOL, handler.clone());

    let pre_prompt = Hook::new(HOOK_ID_PRE_PROMPT, HookEventType::PrePrompt)
        .with_config(HookConfig { priority: 1, timeout_ms: 5000, ..Default::default() });
    hook_engine.register(pre_prompt);
    hook_engine.register_handler(HOOK_ID_PRE_PROMPT, handler.clone());

    let post_response = Hook::new(HOOK_ID_POST_RESPONSE, HookEventType::PostResponse)
        .with_config(HookConfig { priority: 1, timeout_ms: 5000, ..Default::default() });
    hook_engine.register(post_response);
    hook_engine.register_handler(HOOK_ID_POST_RESPONSE, handler);

    tracing::info!("Sentinel hooks registered (pre_tool_use, pre_prompt, post_response)");
}
use a3s_code::llm::create_client_with_config;
use a3s_code::session::SessionManager;
use a3s_code::skills::SkillRegistry;
use a3s_code::{load_agents_from_dir, AgentDefinition, AgentRegistry};
use a3s_code::tools::ToolExecutor;
use anyhow::{Context, Result};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tracing;

// ── Verdict ──────────────────────────────────────────────────────────────────

/// Outcome of sentinel analysis for a single event.
#[derive(Debug, Clone, Default)]
pub struct SecurityVerdict {
    /// Whether the originating action should be blocked.
    pub should_block: bool,

    /// Human-readable reason for the block decision.
    pub reason: Option<String>,

    /// Replacement text when the sentinel redacts sensitive content.
    /// `None` means pass the original text through unchanged.
    pub sanitized_content: Option<String>,

    /// Highest severity level found.
    pub severity: Severity,
}

impl SecurityVerdict {
    fn allow() -> Self {
        Self::default()
    }

    fn block(reason: impl Into<String>) -> Self {
        Self {
            should_block: true,
            reason: Some(reason.into()),
            severity: Severity::High,
            ..Default::default()
        }
    }
}

/// Severity of a security finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Severity {
    #[default]
    None,
    Low,
    Medium,
    High,
    Critical,
}

// ── SentinelAgent ─────────────────────────────────────────────────────────────

/// The sentinel security agent.
///
/// Immutable after construction — clone the `Arc` to share across threads.
pub struct SentinelAgent {
    policy: Arc<SentinelPolicy>,
    phase1: Arc<Phase1Engine>,

    /// Sentinel's own `SessionManager` (isolated from the main agent sessions).
    session_manager: Arc<SessionManager>,

    /// Skills the sentinel agent has access to (kept alive; set on session_manager).
    skill_registry: Arc<SkillRegistry>,

    /// User-defined specialist sub-agents.
    agent_registry: Arc<AgentRegistry>,

    // ── Multi-agent improvements ─────────────────────────────────────────

    /// P1: per-session suspicion accumulator (temporal-evasion defence).
    accumulator: Arc<SuspicionAccumulatorMap>,

    /// P2: session lineage / trust-level tracker.
    lineage: Arc<LineageMap>,

    /// P3: pre-warmed Phase 2 LLM session pool.
    phase2_pool: Arc<Phase2Pool>,

    /// P4: async cross-session pattern correlator.
    correlator: Arc<CrossSessionCorrelator>,
}

impl std::fmt::Debug for SentinelAgent {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SentinelAgent").finish()
    }
}

impl SentinelAgent {
    /// Initialise the sentinel from the given directory.
    ///
    /// If `sentinel_dir` does not exist the sentinel still starts successfully
    /// with default policy — it logs a debug message instead of failing.
    pub async fn init(
        sentinel_dir: &Path,
        main_model_config: Option<&a3s_code::config::CodeConfig>,
    ) -> Result<Arc<Self>> {
        // Load policy (default when file absent)
        let policy = Arc::new(SentinelPolicy::load(sentinel_dir)?);
        tracing::info!(dir = %sentinel_dir.display(), "Sentinel policy loaded");

        // Build Phase 1 engine
        let phase1 = Arc::new(Phase1Engine::from_config(&policy.phase1));

        // Determine the LLM to use for Phase 2
        let llm_client = resolve_llm(&policy, main_model_config);

        // Sentinel uses a read-only-safe tool executor (no file writes, no shell)
        let tool_executor = Arc::new(ToolExecutor::new("/tmp".to_string()));

        let session_manager = Arc::new(
            SessionManager::new(llm_client, tool_executor),
        );

        // Load custom skills
        let skill_registry = Arc::new(SkillRegistry::with_builtins());
        let skills_dir = sentinel_dir.join("skills");
        if skills_dir.exists() {
            match skill_registry.load_from_dir(&skills_dir) {
                Ok(n) if n > 0 => tracing::info!(count = n, "Loaded sentinel skills"),
                Ok(_) => {}
                Err(e) => tracing::debug!("Sentinel skills dir not loaded: {e}"),
            }
        }
        session_manager.set_skill_registry(skill_registry.clone(), skills_dir).await;

        // Load sub-agent definitions
        let agent_registry = Arc::new(AgentRegistry::default());
        let agents_dir = sentinel_dir.join("agents");
        if agents_dir.exists() {
            let defs = load_agents_from_dir(&agents_dir);
            for def in defs {
                agent_registry.register(def);
            }
            tracing::info!(
                count = agent_registry.len(),
                dir = %agents_dir.display(),
                "Loaded sentinel sub-agents"
            );
        }

        // P1: suspicion accumulator (default: 5-minute window, 30s Phase 2 cooldown)
        let accumulator = Arc::new(SuspicionAccumulatorMap::default());

        // P2: session lineage
        let lineage = Arc::new(LineageMap::new());

        // P3: Phase 2 pool (3 pre-warmed sessions)
        let system_prompt = build_sentinel_system_prompt(
            policy.phase2.extra_system_prompt.as_deref().unwrap_or(""),
        );
        let phase2_pool = Phase2Pool::new(
            3,
            session_manager.clone(),
            system_prompt,
            policy.phase2.severity_actions.clone(),
        )
        .await;

        // P4: cross-session correlator (1024-event buffer, 2-minute window)
        let correlator = Arc::new(CrossSessionCorrelator::spawn(
            1024,
            std::time::Duration::from_secs(120),
            None, // alert broadcast channel — callers can add one via their own channel
        ));

        Ok(Arc::new(Self {
            policy,
            phase1,
            session_manager,
            skill_registry,
            agent_registry,
            accumulator,
            lineage,
            phase2_pool,
            correlator,
        }))
    }

    // ── Analysis entry points ────────────────────────────────────────────

    /// Analyse a pre-tool-use event (blocking path).
    pub async fn analyze_pre_tool_use(
        &self,
        session_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> SecurityVerdict {
        // Register session lineage on first encounter (P2).
        let lineage = self.lineage.get_or_infer(session_id);

        let r1 = self.phase1.check_tool_use(session_id, tool, args);

        if r1.should_block {
            // Feed hard-block as high suspicion to correlator (P4).
            self.correlator.submit(CorrelatedEvent {
                session_id: session_id.to_string(),
                event_type: CorrelatedEventType::HighSuspicion,
                timestamp: std::time::Instant::now(),
            });
            return SecurityVerdict::block(
                r1.block_reason.unwrap_or_else(|| "Sentinel Phase 1 block".to_string()),
            );
        }

        // Submit tool-specific events to correlator (P4, non-blocking).
        if let Some(url) = extract_url_from_args(args) {
            let host = extract_domain(&url);
            self.correlator.submit(CorrelatedEvent {
                session_id: session_id.to_string(),
                event_type: CorrelatedEventType::NetworkRequest { host },
                timestamp: std::time::Instant::now(),
            });
        }
        if is_file_write_tool(tool) {
            let path = args.get("file_path")
                .or_else(|| args.get("path"))
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            self.correlator.submit(CorrelatedEvent {
                session_id: session_id.to_string(),
                event_type: CorrelatedEventType::FileWrite { path },
                timestamp: std::time::Instant::now(),
            });
        }

        // Apply lineage depth multiplier then feed into accumulator (P1 + P2).
        let adjusted_score = r1.score * lineage.score_multiplier();
        let acc_decision = self.accumulator.record(session_id, adjusted_score);

        // Accumulator hard block (P1).
        if let AccumulatorDecision::HardBlock { accumulated_score } = acc_decision {
            tracing::warn!(
                session_id,
                accumulated_score,
                "Sentinel: accumulated suspicion hard-block"
            );
            return SecurityVerdict::block(format!(
                "Accumulated suspicion score {accumulated_score:.2} exceeds hard-block threshold"
            ));
        }

        // Delegate: synchronous rules first.
        if let Some(v) = self.try_delegate_blocking(
            DelegateTrigger::PreToolUse,
            Some(tool),
            &serde_json::json!({ "tool": tool, "args": args }),
        ).await {
            return v;
        }

        // Trigger Phase 2 via pool when:
        //   - single-event score crosses threshold, OR
        //   - tool is in always_analyze set, OR
        //   - accumulator says TriggerPhase2  (P1)
        let single_trigger = r1.score >= self.policy.phase1.phase2_trigger_score
            || self.phase1.always_analyze(tool);
        let accum_trigger = matches!(acc_decision, AccumulatorDecision::TriggerPhase2 { .. });

        if single_trigger || accum_trigger {
            let context = format!(
                "Tool '{tool}' called with args: {}",
                serde_json::to_string(args).unwrap_or_default()
            );
            self.phase2_pool.analyze(&context, self.policy.phase2.timeout_ms).await
        } else {
            SecurityVerdict::allow()
        }
    }

    /// Analyse a user prompt (blocking path).
    pub async fn analyze_prompt(&self, session_id: &str, prompt: &str) -> SecurityVerdict {
        let lineage = self.lineage.get_or_infer(session_id);
        let r1 = self.phase1.check_prompt(session_id, prompt);

        if r1.should_block {
            self.correlator.submit(CorrelatedEvent {
                session_id: session_id.to_string(),
                event_type: CorrelatedEventType::HighSuspicion,
                timestamp: std::time::Instant::now(),
            });
            return SecurityVerdict::block(
                r1.block_reason.unwrap_or_else(|| "Sentinel Phase 1 block".to_string()),
            );
        }

        let adjusted_score = r1.score * lineage.score_multiplier();
        let acc_decision = self.accumulator.record(session_id, adjusted_score);

        if let AccumulatorDecision::HardBlock { accumulated_score } = acc_decision {
            return SecurityVerdict::block(format!(
                "Accumulated suspicion {accumulated_score:.2} exceeds hard-block threshold"
            ));
        }

        if let Some(v) = self.try_delegate_blocking(
            DelegateTrigger::PrePrompt,
            None,
            &serde_json::json!({ "prompt": prompt }),
        ).await {
            return v;
        }

        let single_trigger = r1.score >= self.policy.phase1.phase2_trigger_score;
        let accum_trigger = matches!(acc_decision, AccumulatorDecision::TriggerPhase2 { .. });

        if single_trigger || accum_trigger {
            self.phase2_pool
                .analyze(&format!("User prompt: {prompt}"), self.policy.phase2.timeout_ms)
                .await
        } else {
            SecurityVerdict::allow()
        }
    }

    /// Analyse an LLM response (blocking path).
    pub async fn analyze_response(&self, response_text: &str) -> SecurityVerdict {
        let r1 = self.phase1.check_response(response_text);

        if r1.should_block {
            return SecurityVerdict::block(
                r1.block_reason.unwrap_or_else(|| "Sentinel Phase 1 block".to_string()),
            );
        }

        // Fire async (non-blocking) delegates for post-response.
        self.fire_async_delegates(DelegateTrigger::PostResponse, None, response_text);

        if r1.score >= self.policy.phase1.phase2_trigger_score {
            self.phase2_pool
                .analyze(
                    &format!("Agent response: {response_text}"),
                    self.policy.phase2.timeout_ms,
                )
                .await
        } else {
            SecurityVerdict::allow()
        }
    }

    /// Notify the sentinel that a session has been created (registers lineage).
    ///
    /// Call with `parent_id = None` for user-initiated sessions.  For sub-agent
    /// sessions call with the spawning session's ID so the lineage depth is
    /// tracked correctly.
    pub fn on_session_created(&self, session_id: &str, parent_id: Option<&str>) {
        self.lineage.register(session_id, parent_id);
    }

    /// Notify the sentinel that a session has been destroyed (releases state).
    pub fn on_session_destroyed(&self, session_id: &str) {
        self.accumulator.remove(session_id);
        self.lineage.remove(session_id);
    }

    // ── Delegation ────────────────────────────────────────────────────────

    /// Run synchronous (blocking) delegate rules that match the event.
    async fn try_delegate_blocking(
        &self,
        trigger: DelegateTrigger,
        tool: Option<&str>,
        payload: &serde_json::Value,
    ) -> Option<SecurityVerdict> {
        for rule in &self.policy.delegate {
            if rule.async_mode || !rule.matches(trigger, tool) {
                continue;
            }
            let Some(def) = self.agent_registry.get(&rule.agent) else {
                tracing::warn!(agent = %rule.agent, "Sentinel delegate agent not found");
                continue;
            };
            match self.run_sub_agent(&def, payload).await {
                Ok(verdict) => return Some(verdict),
                Err(e) => tracing::warn!(agent = %rule.agent, "Sentinel sub-agent failed: {e}"),
            }
        }
        None
    }

    /// Fire async (non-blocking) delegate rules; errors are logged, not propagated.
    fn fire_async_delegates(&self, trigger: DelegateTrigger, tool: Option<&str>, text: &str) {
        let payload = serde_json::json!({ "text": text });
        for rule in &self.policy.delegate {
            if !rule.async_mode || !rule.matches(trigger, tool) {
                continue;
            }
            let Some(def): Option<AgentDefinition> = self.agent_registry.get(&rule.agent) else {
                continue;
            };
            let sm = self.session_manager.clone();
            let payload = payload.clone();
            tokio::spawn(async move {
                let agent = SentinelSubAgentRunner { session_manager: sm };
                if let Err(e) = agent.run(&def, &payload).await {
                    tracing::debug!(agent = %def.name, "Async sentinel sub-agent failed: {e}");
                }
            });
        }
    }

    async fn run_sub_agent(
        &self,
        def: &AgentDefinition,
        payload: &serde_json::Value,
    ) -> Result<SecurityVerdict> {
        let runner = SentinelSubAgentRunner {
            session_manager: self.session_manager.clone(),
        };
        runner.run(def, payload).await
    }

    // ── Hook registration ─────────────────────────────────────────────────

    /// Unregister all sentinel hooks from `hook_engine`.
    pub fn unregister_hooks(&self, hook_engine: &HookEngine) {
        hook_engine.unregister(HOOK_ID_PRE_TOOL);
        hook_engine.unregister(HOOK_ID_PRE_PROMPT);
        hook_engine.unregister(HOOK_ID_POST_RESPONSE);
        hook_engine.unregister_handler(HOOK_ID_PRE_TOOL);
        hook_engine.unregister_handler(HOOK_ID_PRE_PROMPT);
        hook_engine.unregister_handler(HOOK_ID_POST_RESPONSE);
    }

    /// Return the sentinel's skill registry (used by [`sync::SkillSyncer`]).
    pub fn skill_registry(&self) -> Arc<SkillRegistry> {
        self.skill_registry.clone()
    }

    /// Return the sentinel's agent registry (used by [`sync::SkillSyncer`]).
    pub fn agent_registry(&self) -> Arc<AgentRegistry> {
        self.agent_registry.clone()
    }
}

// ── SentinelObserver impl for SentinelAgent ───────────────────────────────────

#[async_trait::async_trait]
impl SentinelObserver for SentinelAgent {
    async fn analyze_pre_tool_use(
        &self,
        session_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> SecurityVerdict {
        self.analyze_pre_tool_use(session_id, tool, args).await
    }

    async fn analyze_prompt(&self, session_id: &str, prompt: &str) -> SecurityVerdict {
        self.analyze_prompt(session_id, prompt).await
    }

    async fn analyze_response(&self, response_text: &str) -> SecurityVerdict {
        self.analyze_response(response_text).await
    }

    fn on_session_created(&self, session_id: &str, parent_id: Option<&str>) {
        self.on_session_created(session_id, parent_id);
    }

    fn on_session_destroyed(&self, session_id: &str) {
        self.on_session_destroyed(session_id);
    }
}

const HOOK_ID_PRE_TOOL: &str = "sentinel.pre_tool_use";
const HOOK_ID_PRE_PROMPT: &str = "sentinel.pre_prompt";
const HOOK_ID_POST_RESPONSE: &str = "sentinel.post_response";

// ── Default sentinel dir ──────────────────────────────────────────────────────

/// Returns the default sentinel configuration directory.
///
/// `$XDG_CONFIG_HOME/safeclaw/sentinel` on Linux, `~/Library/Application Support/safeclaw/sentinel` on macOS.
pub fn default_sentinel_dir() -> PathBuf {
    dirs_next::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("safeclaw")
        .join("sentinel")
}

// ── Sub-agent runner helper ───────────────────────────────────────────────────

struct SentinelSubAgentRunner {
    session_manager: Arc<SessionManager>,
}

impl SentinelSubAgentRunner {
    async fn run(&self, def: &AgentDefinition, payload: &serde_json::Value) -> Result<SecurityVerdict> {
        let system_prompt = def.prompt.as_deref().unwrap_or(
            "You are a security analysis sub-agent. Analyse the provided content and return JSON \
             with `severity` (none/low/medium/high/critical), `should_block` (bool), `reason` (string).",
        );

        let timeout_ms = 1500u64; // conservative default for sub-agents
        let session_id = format!("sentinel-sub-{}", uuid::Uuid::new_v4());
        let session_cfg = a3s_code::session::SessionConfig {
            system_prompt: Some(system_prompt.to_string()),
            ..Default::default()
        };

        self.session_manager
            .create_session(session_id.clone(), session_cfg)
            .await
            .context("Sentinel sub-agent session creation failed")?;

        let prompt = format!("Analyse:\n{}", serde_json::to_string_pretty(payload).unwrap_or_default());
        let timeout = tokio::time::Duration::from_millis(timeout_ms);

        let result = tokio::time::timeout(
            timeout,
            self.session_manager.generate(&session_id, &prompt),
        )
        .await;

        let _ = self.session_manager.destroy_session(&session_id).await;

        match result {
            Ok(Ok(r)) => Ok(parse_phase2_verdict(&r.text, &Default::default())),
            Ok(Err(e)) => Err(anyhow::anyhow!("Sub-agent LLM error: {e}")),
            Err(_) => Err(anyhow::anyhow!("Sub-agent timed out")),
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Returns `true` for tools that write files to disk.
fn is_file_write_tool(tool: &str) -> bool {
    matches!(tool, "write_file" | "edit_file" | "create_file" | "overwrite_file" | "Write" | "Edit")
}

/// Extract a URL-like string from tool arguments (checks common arg names).
fn extract_url_from_args(args: &serde_json::Value) -> Option<String> {
    for key in &["url", "uri", "endpoint", "host"] {
        if let Some(v) = args.get(key).and_then(|v| v.as_str()) {
            if v.contains("://") || v.contains('.') {
                return Some(v.to_string());
            }
        }
    }
    None
}

/// Extract the domain/host component from a URL-like string.
fn extract_domain(url: &str) -> String {
    // Strip scheme
    let without_scheme = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    // Take only the host part (before first '/' or ':')
    without_scheme
        .split(['/', ':'])
        .next()
        .unwrap_or(without_scheme)
        .to_string()
}

fn resolve_llm(
    policy: &SentinelPolicy,
    main_cfg: Option<&a3s_code::config::CodeConfig>,
) -> Option<Arc<dyn a3s_code::llm::LlmClient>> {
    // If policy specifies a model, try to construct an LLM config for it
    if let Some(ref model_name) = policy.phase2.model {
        if let Some(cfg) = main_cfg {
            // Build a minimal LlmConfig using the main config's provider settings
            // but override the model name
            if let Some(mut llm_cfg) = cfg.default_llm_config() {
                llm_cfg.model = model_name.clone();
                return Some(create_client_with_config(llm_cfg));
            }
        }
    }
    // Fall back to whatever the main config has
    main_cfg
        .and_then(|c| c.default_llm_config())
        .map(create_client_with_config)
}

fn build_sentinel_system_prompt(extra: &str) -> String {
    let base = "You are a security sentinel for an AI agent system. \
        Your job is to analyse agent inputs, outputs, and tool calls for security risks. \
        Always respond with valid JSON containing exactly these fields: \
        `severity` (one of: none, low, medium, high, critical), \
        `should_block` (boolean — true only for high/critical findings that pose immediate risk), \
        `reason` (one-sentence explanation, empty string if severity is none). \
        Be precise and conservative: only block when there is clear evidence of harm.";

    if extra.is_empty() {
        base.to_string()
    } else {
        format!("{base}\n\n{extra}")
    }
}

fn parse_phase2_verdict(
    text: &str,
    severity_actions: &config::SeverityActions,
) -> SecurityVerdict {
    // Extract the first JSON object from the response text
    let json_str = extract_first_json(text);
    let Ok(v) = serde_json::from_str::<serde_json::Value>(&json_str) else {
        tracing::debug!(response = %text, "Sentinel Phase 2 response is not valid JSON");
        return SecurityVerdict::allow();
    };

    let severity_str = v.get("severity").and_then(|s| s.as_str()).unwrap_or("none");
    let should_block_raw = v.get("should_block").and_then(|b| b.as_bool()).unwrap_or(false);
    let reason = v.get("reason").and_then(|r| r.as_str()).unwrap_or("").to_string();

    let severity = match severity_str {
        "low" => Severity::Low,
        "medium" => Severity::Medium,
        "high" => Severity::High,
        "critical" => Severity::Critical,
        _ => Severity::None,
    };

    let action = match severity {
        Severity::None | Severity::Low => severity_actions.low,
        Severity::Medium => severity_actions.medium,
        Severity::High => severity_actions.high,
        Severity::Critical => severity_actions.critical,
    };

    let should_block = should_block_raw && action == config::SeverityAction::Block;

    if should_block {
        SecurityVerdict::block(if reason.is_empty() { format!("Severity: {severity_str}") } else { reason })
    } else {
        if severity != Severity::None {
            tracing::info!(severity = severity_str, %reason, "Sentinel Phase 2 finding (not blocking)");
        }
        SecurityVerdict::allow()
    }
}

/// Extract the first `{...}` JSON object from a string that may have surrounding text.
fn extract_first_json(text: &str) -> String {
    if let Some(start) = text.find('{') {
        let slice = &text[start..];
        let mut depth = 0i32;
        let mut end = None;
        for (i, ch) in slice.char_indices() {
            match ch {
                '{' => depth += 1,
                '}' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(i + 1);
                        break;
                    }
                }
                _ => {}
            }
        }
        if let Some(e) = end {
            return slice[..e].to_string();
        }
    }
    text.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_extract_first_json_clean() {
        let input = r#"{"severity":"low","should_block":false,"reason":""}"#;
        let out = extract_first_json(input);
        assert!(out.starts_with('{'));
        assert!(out.ends_with('}'));
    }

    #[test]
    fn test_extract_first_json_with_prose() {
        let input = r#"Here is my analysis: {"severity":"high","should_block":true,"reason":"dangerous"}. Done."#;
        let out = extract_first_json(input);
        let v: serde_json::Value = serde_json::from_str(&out).unwrap();
        assert_eq!(v["severity"], "high");
    }

    #[test]
    fn test_parse_phase2_verdict_block() {
        let text = r#"{"severity":"high","should_block":true,"reason":"data exfiltration"}"#;
        let verdict = parse_phase2_verdict(text, &Default::default());
        assert!(verdict.should_block);
        assert!(verdict.reason.as_deref().unwrap_or("").contains("exfil"));
    }

    #[test]
    fn test_parse_phase2_verdict_allow_low() {
        let text = r#"{"severity":"low","should_block":false,"reason":"minor"}"#;
        let verdict = parse_phase2_verdict(text, &Default::default());
        assert!(!verdict.should_block);
    }

    #[test]
    fn test_parse_phase2_verdict_invalid_json() {
        let verdict = parse_phase2_verdict("This is not JSON at all", &Default::default());
        assert!(!verdict.should_block); // fail-open
    }

    #[test]
    fn test_security_verdict_block() {
        let v = SecurityVerdict::block("test reason");
        assert!(v.should_block);
        assert_eq!(v.reason.as_deref(), Some("test reason"));
    }

    #[test]
    fn test_build_sentinel_system_prompt_no_extra() {
        let p = build_sentinel_system_prompt("");
        assert!(p.contains("security sentinel"));
        assert!(p.contains("should_block"));
    }

    #[test]
    fn test_build_sentinel_system_prompt_with_extra() {
        let p = build_sentinel_system_prompt("Handle medical records carefully.");
        assert!(p.contains("medical records"));
        assert!(p.contains("security sentinel"));
    }

    #[test]
    fn test_default_sentinel_dir_is_absolute() {
        let dir = default_sentinel_dir();
        assert!(dir.is_absolute());
        assert!(dir.ends_with("sentinel"));
    }
}
