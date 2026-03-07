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

pub mod config;
pub mod hook;
pub mod phase1;

pub use config::{DelegateTrigger, SentinelPolicy};
pub use phase1::{Phase1Engine, Phase1Result};

use a3s_code::hooks::{Hook, HookConfig, HookEngine, HookEventType};
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
    #[allow(dead_code)]
    skill_registry: Arc<SkillRegistry>,

    /// User-defined specialist sub-agents.
    agent_registry: Arc<AgentRegistry>,
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

        Ok(Arc::new(Self { policy, phase1, session_manager, skill_registry, agent_registry }))
    }

    // ── Analysis entry points ────────────────────────────────────────────

    /// Analyse a pre-tool-use event (blocking path).
    pub async fn analyze_pre_tool_use(
        &self,
        session_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> SecurityVerdict {
        let r1 = self.phase1.check_tool_use(session_id, tool, args);

        if r1.should_block {
            return SecurityVerdict::block(
                r1.block_reason.unwrap_or_else(|| "Sentinel Phase 1 block".to_string()),
            );
        }

        // Delegate: synchronous rules first
        if let Some(v) = self.try_delegate_blocking(
            DelegateTrigger::PreToolUse,
            Some(tool),
            &serde_json::json!({ "tool": tool, "args": args }),
        ).await {
            return v;
        }

        // Phase 2 when score exceeds threshold or tool is in always_analyze set
        if r1.score >= self.policy.phase1.phase2_trigger_score || self.phase1.always_analyze(tool) {
            self.phase2_analyze(&format!(
                "Tool '{tool}' called with args: {}",
                serde_json::to_string(args).unwrap_or_default()
            ))
            .await
        } else {
            SecurityVerdict::allow()
        }
    }

    /// Analyse a user prompt (blocking path).
    pub async fn analyze_prompt(&self, session_id: &str, prompt: &str) -> SecurityVerdict {
        let r1 = self.phase1.check_prompt(session_id, prompt);

        if r1.should_block {
            return SecurityVerdict::block(
                r1.block_reason.unwrap_or_else(|| "Sentinel Phase 1 block".to_string()),
            );
        }

        if let Some(v) = self.try_delegate_blocking(
            DelegateTrigger::PrePrompt,
            None,
            &serde_json::json!({ "prompt": prompt }),
        ).await {
            return v;
        }

        if r1.score >= self.policy.phase1.phase2_trigger_score {
            self.phase2_analyze(&format!("User prompt: {prompt}")).await
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

        // Fire async (non-blocking) delegates for post-response
        self.fire_async_delegates(DelegateTrigger::PostResponse, None, response_text);

        if r1.score >= self.policy.phase1.phase2_trigger_score {
            self.phase2_analyze(&format!("Agent response: {response_text}")).await
        } else {
            SecurityVerdict::allow()
        }
    }

    // ── Phase 2 ──────────────────────────────────────────────────────────

    /// Run Phase 2 LLM analysis.  Returns `Allow` on timeout or when no LLM
    /// is configured (fail-open — security must not block normal usage).
    async fn phase2_analyze(&self, context: &str) -> SecurityVerdict {
        let timeout = tokio::time::Duration::from_millis(self.policy.phase2.timeout_ms);

        let extra = self
            .policy
            .phase2
            .extra_system_prompt
            .as_deref()
            .unwrap_or("");

        let system_prompt = build_sentinel_system_prompt(extra);

        let analysis_prompt = format!(
            "Analyse the following agent activity and return JSON with fields \
             `severity` (none/low/medium/high/critical), `should_block` (bool), \
             and `reason` (string).\n\nActivity:\n{context}"
        );

        // Create an ephemeral sentinel session
        let session_id_str = format!("sentinel-{}", uuid::Uuid::new_v4());
        let session_cfg = a3s_code::session::SessionConfig {
            system_prompt: Some(system_prompt.clone()),
            ..Default::default()
        };
        let session_result = self
            .session_manager
            .create_session(session_id_str.clone(), session_cfg)
            .await;

        let session_id = match session_result {
            Ok(id) => id,
            Err(e) => {
                tracing::warn!("Sentinel Phase 2 session creation failed: {e}");
                return SecurityVerdict::allow(); // fail-open
            }
        };

        let generate = tokio::time::timeout(
            timeout,
            self.session_manager.generate(&session_id, &analysis_prompt),
        )
        .await;

        // Best-effort cleanup: ignore errors (session will expire via lifecycle task)
        let _ = self.session_manager.destroy_session(&session_id).await;

        match generate {
            Ok(Ok(result)) => parse_phase2_verdict(&result.text, &self.policy.phase2.severity_actions),
            Ok(Err(e)) => {
                tracing::debug!("Sentinel Phase 2 LLM error: {e}");
                SecurityVerdict::allow()
            }
            Err(_timeout) => {
                tracing::debug!(
                    timeout_ms = self.policy.phase2.timeout_ms,
                    "Sentinel Phase 2 timed out — failing open"
                );
                SecurityVerdict::allow()
            }
        }
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

    /// Register sentinel hooks with `hook_engine`.
    ///
    /// Three blocking hooks are installed (`PreToolUse`, `PrePrompt`,
    /// `PostResponse`).  The hook IDs are stable so they can be unregistered
    /// if needed.
    pub fn register_hooks(self: &Arc<Self>, hook_engine: &HookEngine) {
        use hook::SentinelHookHandler;

        let handler = Arc::new(SentinelHookHandler::new(Arc::clone(self)));

        // PreToolUse — highest priority so sentinel runs before other hooks
        let pre_tool = Hook::new(HOOK_ID_PRE_TOOL, HookEventType::PreToolUse).with_config(
            HookConfig { priority: 1, timeout_ms: 5000, ..Default::default() },
        );
        hook_engine.register(pre_tool);
        hook_engine.register_handler(HOOK_ID_PRE_TOOL, handler.clone());

        // PrePrompt
        let pre_prompt = Hook::new(HOOK_ID_PRE_PROMPT, HookEventType::PrePrompt).with_config(
            HookConfig { priority: 1, timeout_ms: 5000, ..Default::default() },
        );
        hook_engine.register(pre_prompt);
        hook_engine.register_handler(HOOK_ID_PRE_PROMPT, handler.clone());

        // PostResponse
        let post_response =
            Hook::new(HOOK_ID_POST_RESPONSE, HookEventType::PostResponse).with_config(HookConfig {
                priority: 1,
                timeout_ms: 5000,
                ..Default::default()
            });
        hook_engine.register(post_response);
        hook_engine.register_handler(HOOK_ID_POST_RESPONSE, handler);

        tracing::info!("Sentinel hooks registered (pre_tool_use, pre_prompt, post_response)");
    }

    /// Unregister all sentinel hooks from `hook_engine`.
    pub fn unregister_hooks(&self, hook_engine: &HookEngine) {
        hook_engine.unregister(HOOK_ID_PRE_TOOL);
        hook_engine.unregister(HOOK_ID_PRE_PROMPT);
        hook_engine.unregister(HOOK_ID_POST_RESPONSE);
        hook_engine.unregister_handler(HOOK_ID_PRE_TOOL);
        hook_engine.unregister_handler(HOOK_ID_PRE_PROMPT);
        hook_engine.unregister_handler(HOOK_ID_POST_RESPONSE);
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
