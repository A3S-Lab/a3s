//! Sentinel security policy configuration.
//!
//! Loaded from `~/.config/safeclaw/sentinel/policy.hcl` (or the platform
//! config dir equivalent).  When the file is absent, all fields use their
//! documented defaults so the system works out of the box.

use serde::{Deserialize, Serialize};
use std::path::Path;

// ── Top-level policy ────────────────────────────────────────────────────────

/// Complete sentinel policy, parsed from `policy.hcl`.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SentinelPolicy {
    /// Phase 1: fast rule-based checks.
    #[serde(default)]
    pub phase1: Phase1Config,

    /// Phase 2: LLM-based analysis (only triggered when Phase 1 suspects).
    #[serde(default)]
    pub phase2: Phase2Config,

    /// Sub-agent delegation rules.
    #[serde(default)]
    pub delegate: Vec<DelegateRule>,
}

impl SentinelPolicy {
    /// Load from `<sentinel_dir>/policy.hcl`.
    /// Returns `Ok(Default::default())` when the file does not exist.
    pub fn load(sentinel_dir: &Path) -> anyhow::Result<Self> {
        let path = sentinel_dir.join("policy.hcl");
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("Failed to read sentinel policy {}: {e}", path.display()))?;
        let policy: Self = hcl::from_str(&content)
            .map_err(|e| anyhow::anyhow!("Failed to parse sentinel policy {}: {e}", path.display()))?;
        Ok(policy)
    }
}

// ── Phase 1 ─────────────────────────────────────────────────────────────────

/// Rule-based checker configuration — targets < 5 ms per event.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Phase1Config {
    /// Regex patterns whose match in any text field immediately blocks the event.
    pub block_patterns: Vec<String>,

    /// Tool-specific access policy.
    pub tools: ToolPolicy,

    /// Outbound network domain policy (applied to url/domain args).
    pub network: NetworkPolicy,

    /// Per-session rate limits.
    pub rate_limits: RateLimitConfig,

    /// Phase 1 suspicion score threshold above which Phase 2 is triggered.
    /// Range 0.0–1.0; default 0.35.
    pub phase2_trigger_score: f32,
}

impl Default for Phase1Config {
    fn default() -> Self {
        Self {
            block_patterns: Vec::new(),
            tools: ToolPolicy::default(),
            network: NetworkPolicy::default(),
            rate_limits: RateLimitConfig::default(),
            phase2_trigger_score: 0.35,
        }
    }
}

/// Tool access policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct ToolPolicy {
    /// Tools that are completely blocked regardless of arguments.
    pub blocked: Vec<String>,

    /// Tools that always skip Phase 1 scoring and go directly to Phase 2.
    pub always_analyze: Vec<String>,
}

/// Domain-level network policy applied to tool arguments.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct NetworkPolicy {
    /// Domain patterns to block (glob-style, e.g. `*.evil.com`).
    pub block_domains: Vec<String>,

    /// Allowlisted domain patterns; all others are rejected when non-empty.
    pub allow_domains: Vec<String>,

    /// When `true` and `allow_domains` is non-empty, unknown domains are blocked.
    pub block_on_unknown: bool,
}

/// Per-session rate limits for Phase 1.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(default)]
pub struct RateLimitConfig {
    /// Maximum tool calls per minute per session (0 = unlimited).
    pub tool_calls_per_minute: u32,

    /// Maximum LLM requests per minute per session (0 = unlimited).
    pub llm_requests_per_minute: u32,

    /// Maximum file writes per minute per session (0 = unlimited).
    pub file_writes_per_minute: u32,
}

// ── Phase 2 ─────────────────────────────────────────────────────────────────

/// LLM-based analysis configuration.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Phase2Config {
    /// Model to use for sentinel LLM analysis.
    /// `None` means inherit the main agent's model.
    pub model: Option<String>,

    /// Analysis timeout in milliseconds.
    pub timeout_ms: u64,

    /// Optional text appended to the built-in sentinel system prompt.
    /// Use this to add domain-specific guidance (e.g., "This system handles
    /// medical records — treat any patient ID as critical PII").
    pub extra_system_prompt: Option<String>,

    /// How to act on each severity level returned by the LLM.
    pub severity_actions: SeverityActions,
}

impl Default for Phase2Config {
    fn default() -> Self {
        Self {
            model: None,
            timeout_ms: 2000,
            extra_system_prompt: None,
            severity_actions: SeverityActions::default(),
        }
    }
}

/// Action to take per severity level.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct SeverityActions {
    pub low: SeverityAction,
    pub medium: SeverityAction,
    pub high: SeverityAction,
    pub critical: SeverityAction,
}

impl Default for SeverityActions {
    fn default() -> Self {
        Self {
            low: SeverityAction::Log,
            medium: SeverityAction::Warn,
            high: SeverityAction::Block,
            critical: SeverityAction::Block,
        }
    }
}

/// What the sentinel does when a given severity is found.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SeverityAction {
    /// Record in audit log only.
    Log,
    /// Log and emit a structured warning (surfaced to the user as a notice).
    Warn,
    /// Block the originating action and return an error.
    Block,
}

// ── Delegation ───────────────────────────────────────────────────────────────

/// A delegation rule that routes events to a specialist sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DelegateRule {
    /// Rule name (used in log messages).
    pub name: String,

    /// Which hook event triggers this rule.
    pub trigger: DelegateTrigger,

    /// Optional list of tool names; when set, only those tools match.
    /// Has no effect for non-tool triggers.
    #[serde(default)]
    pub tool_filter: Vec<String>,

    /// Name of the sub-agent definition file (without `.hcl` extension).
    /// The file must exist at `<sentinel_dir>/agents/<agent>.hcl`.
    pub agent: String,

    /// When `true`, the sub-agent runs fire-and-forget (non-blocking audit).
    /// When `false` (default), execution waits for the sub-agent's verdict.
    #[serde(default)]
    pub async_mode: bool,
}

impl DelegateRule {
    /// Whether this rule matches the given trigger and optional tool name.
    pub fn matches(&self, trigger: DelegateTrigger, tool: Option<&str>) -> bool {
        if self.trigger != trigger {
            return false;
        }
        if !self.tool_filter.is_empty() {
            let Some(t) = tool else { return false };
            return self.tool_filter.iter().any(|f| f == t);
        }
        true
    }
}

/// Event trigger for delegation rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegateTrigger {
    PreToolUse,
    PostToolUse,
    PrePrompt,
    PostResponse,
}

// ── Sub-agent definitions ────────────────────────────────────────────────────
//
// Sub-agent files live in `<sentinel_dir>/agents/` and use the standard
// a3s-code YAML / Markdown-with-YAML-frontmatter format (same as the main
// agents/ directory).  They are loaded via `a3s_code::subagent::load_agents_from_dir`
// into an `AgentRegistry`.  The `agent` field in `DelegateRule` references an
// agent by the `name` field in that definition.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_policy_has_safe_defaults() {
        let policy = SentinelPolicy::default();
        assert!(policy.phase1.block_patterns.is_empty());
        assert_eq!(policy.phase1.phase2_trigger_score, 0.35);
        assert_eq!(policy.phase2.timeout_ms, 2000);
        assert!(policy.delegate.is_empty());
    }

    #[test]
    fn test_severity_actions_defaults() {
        let actions = SeverityActions::default();
        assert_eq!(actions.low, SeverityAction::Log);
        assert_eq!(actions.medium, SeverityAction::Warn);
        assert_eq!(actions.high, SeverityAction::Block);
        assert_eq!(actions.critical, SeverityAction::Block);
    }

    #[test]
    fn test_delegate_rule_matches_trigger() {
        let rule = DelegateRule {
            name: "test".to_string(),
            trigger: DelegateTrigger::PreToolUse,
            tool_filter: vec![],
            agent: "pii_detector".to_string(),
            async_mode: false,
        };
        assert!(rule.matches(DelegateTrigger::PreToolUse, None));
        assert!(!rule.matches(DelegateTrigger::PostResponse, None));
    }

    #[test]
    fn test_delegate_rule_tool_filter() {
        let rule = DelegateRule {
            name: "exfil".to_string(),
            trigger: DelegateTrigger::PreToolUse,
            tool_filter: vec!["write_file".to_string(), "create_file".to_string()],
            agent: "exfil_detector".to_string(),
            async_mode: false,
        };
        assert!(rule.matches(DelegateTrigger::PreToolUse, Some("write_file")));
        assert!(!rule.matches(DelegateTrigger::PreToolUse, Some("read_file")));
        assert!(!rule.matches(DelegateTrigger::PreToolUse, None));
    }

    #[test]
    fn test_policy_load_returns_default_when_missing() {
        let dir = std::path::PathBuf::from("/tmp/nonexistent_sentinel_dir_xyz_123");
        let policy = SentinelPolicy::load(&dir).unwrap();
        // Should return default (not error) when directory/file doesn't exist
        assert!(policy.phase1.block_patterns.is_empty());
    }

    #[test]
    fn test_policy_parses_from_hcl() {
        let hcl = r#"
phase1 {
  block_patterns = ["(?i)rm -rf /", "DROP TABLE"]
  phase2_trigger_score = 0.5
  tools {
    blocked = ["raw_shell"]
    always_analyze = ["bash"]
  }
}
phase2 {
  timeout_ms = 3000
}
"#;
        let policy: SentinelPolicy = hcl::from_str(hcl).unwrap();
        assert_eq!(policy.phase1.block_patterns.len(), 2);
        assert_eq!(policy.phase1.phase2_trigger_score, 0.5);
        assert_eq!(policy.phase1.tools.blocked, vec!["raw_shell"]);
        assert_eq!(policy.phase1.tools.always_analyze, vec!["bash"]);
        assert_eq!(policy.phase2.timeout_ms, 3000);
    }
}
