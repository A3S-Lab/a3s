//! Phase 1: fast, rule-based security analysis.
//!
//! All checks here must complete in < 5 ms.  No LLM calls, no I/O.
//! The engine is built once at startup from the loaded `Phase1Config`
//! and is then `Send + Sync + 'static`.

use super::config::Phase1Config;
use regex::Regex;
use std::collections::{HashMap, HashSet};
use std::sync::Mutex;
use std::time::{Duration, Instant};

// ── Public result type ───────────────────────────────────────────────────────

/// Outcome of a Phase 1 analysis pass.
#[derive(Debug, Clone)]
pub struct Phase1Result {
    /// Suspicion score 0.0–1.0; drives the Phase 2 trigger decision.
    pub score: f32,

    /// Human-readable descriptions of each rule that matched.
    pub violations: Vec<String>,

    /// `true` when a hard-block rule matched (blocked unconditionally).
    pub should_block: bool,

    /// Reason for the hard block (only meaningful when `should_block = true`).
    pub block_reason: Option<String>,
}

impl Phase1Result {
    fn clean() -> Self {
        Self {
            score: 0.0,
            violations: Vec::new(),
            should_block: false,
            block_reason: None,
        }
    }

    fn block(reason: impl Into<String>) -> Self {
        let reason = reason.into();
        Self {
            score: 1.0,
            violations: vec![reason.clone()],
            should_block: true,
            block_reason: Some(reason),
        }
    }
}

// ── Rate limiter ─────────────────────────────────────────────────────────────

/// Sliding-window rate limiter (per session, per category).
struct SlidingWindow {
    /// Maximum events allowed within the window.
    limit: u32,
    window: Duration,
    timestamps: Vec<Instant>,
}

impl SlidingWindow {
    fn new(limit: u32, window: Duration) -> Self {
        Self { limit, window, timestamps: Vec::new() }
    }

    /// Returns `true` when within limit (records the event); `false` when exceeded.
    fn try_record(&mut self) -> bool {
        if self.limit == 0 {
            return true; // 0 = unlimited
        }
        let now = Instant::now();
        self.timestamps.retain(|t| now.duration_since(*t) < self.window);
        if self.timestamps.len() >= self.limit as usize {
            return false;
        }
        self.timestamps.push(now);
        true
    }
}

/// Rate limiters for one session.
struct SessionLimits {
    tool_calls: SlidingWindow,
    llm_requests: SlidingWindow,
    file_writes: SlidingWindow,
}

// ── Phase 1 engine ───────────────────────────────────────────────────────────

/// Compiled, immutable rule engine built from `Phase1Config`.
pub struct Phase1Engine {
    /// Compiled block-pattern regexes (immediate hard-block on match).
    block_patterns: Vec<(Regex, String)>,

    /// Tools that are always blocked.
    blocked_tools: HashSet<String>,

    /// Tools that bypass Phase 1 scoring and always trigger Phase 2.
    always_analyze_tools: HashSet<String>,

    /// Domain patterns for the network policy.
    block_domains: Vec<glob_match_domain::DomainPattern>,
    allow_domains: Vec<glob_match_domain::DomainPattern>,
    block_on_unknown: bool,

    /// Rate limit parameters (kept to initialise per-session limits).
    rate_cfg: super::config::RateLimitConfig,

    /// Per-session rate limiter state.
    session_limits: Mutex<HashMap<String, SessionLimits>>,

    /// Phase 2 trigger threshold.
    pub phase2_trigger_score: f32,
}

impl std::fmt::Debug for Phase1Engine {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Phase1Engine")
            .field("block_patterns", &self.block_patterns.len())
            .field("blocked_tools", &self.blocked_tools.len())
            .field("always_analyze_tools", &self.always_analyze_tools.len())
            .finish()
    }
}

impl Phase1Engine {
    /// Build the engine from a `Phase1Config`.
    ///
    /// Regex compilation errors are logged and the offending pattern is skipped
    /// rather than panicking — a misconfigured pattern should not break startup.
    pub fn from_config(cfg: &Phase1Config) -> Self {
        let block_patterns = cfg
            .block_patterns
            .iter()
            .filter_map(|pat| {
                match Regex::new(pat) {
                    Ok(re) => Some((re, pat.clone())),
                    Err(e) => {
                        tracing::warn!(pattern = %pat, "Invalid sentinel block_pattern: {e}");
                        None
                    }
                }
            })
            .collect();

        let blocked_tools = cfg.tools.blocked.iter().cloned().collect();
        let always_analyze_tools = cfg.tools.always_analyze.iter().cloned().collect();

        let block_domains = cfg
            .network
            .block_domains
            .iter()
            .map(|p| glob_match_domain::DomainPattern::new(p))
            .collect();
        let allow_domains = cfg
            .network
            .allow_domains
            .iter()
            .map(|p| glob_match_domain::DomainPattern::new(p))
            .collect();

        Self {
            block_patterns,
            blocked_tools,
            always_analyze_tools,
            block_domains,
            allow_domains,
            block_on_unknown: cfg.network.block_on_unknown,
            rate_cfg: cfg.rate_limits.clone(),
            session_limits: Mutex::new(HashMap::new()),
            phase2_trigger_score: cfg.phase2_trigger_score,
        }
    }

    // ── Analysis entry points ────────────────────────────────────────────

    /// Analyse a pre-tool-use event.
    pub fn check_tool_use(
        &self,
        session_id: &str,
        tool: &str,
        args: &serde_json::Value,
    ) -> Phase1Result {
        // Hard block: tool is explicitly forbidden
        if self.blocked_tools.contains(tool) {
            return Phase1Result::block(format!("Tool '{tool}' is blocked by sentinel policy"));
        }

        // Rate limit: tool calls per minute
        if !self.record_tool_call(session_id) {
            return Phase1Result::block(format!(
                "Tool call rate limit exceeded for session '{session_id}'"
            ));
        }

        // File-write rate limit for write-adjacent tools
        if is_file_write_tool(tool) && !self.record_file_write(session_id) {
            return Phase1Result::block(format!(
                "File write rate limit exceeded for session '{session_id}'"
            ));
        }

        let mut result = Phase1Result::clean();

        // Scan all string values in args for block patterns
        self.scan_json_strings(args, &mut result);

        // Scan domain/url args against network policy
        if let Some(url) = extract_url_from_args(args) {
            self.check_domain(&url, &mut result);
        }

        // Elevate score for always-analyze tools
        if self.always_analyze_tools.contains(tool) {
            result.score = result.score.max(self.phase2_trigger_score);
        }

        result
    }

    /// Analyse a prompt (pre-prompt event).
    pub fn check_prompt(&self, session_id: &str, prompt: &str) -> Phase1Result {
        // Rate limit: LLM requests per minute
        if !self.record_llm_request(session_id) {
            return Phase1Result::block(format!(
                "LLM request rate limit exceeded for session '{session_id}'"
            ));
        }

        let mut result = Phase1Result::clean();
        self.scan_text(prompt, &mut result);
        result
    }

    /// Analyse an LLM response (post-response event).
    pub fn check_response(&self, response_text: &str) -> Phase1Result {
        let mut result = Phase1Result::clean();
        self.scan_text(response_text, &mut result);
        result
    }

    /// Whether the given tool is in the `always_analyze` set.
    pub fn always_analyze(&self, tool: &str) -> bool {
        self.always_analyze_tools.contains(tool)
    }

    // ── Internal helpers ─────────────────────────────────────────────────

    fn scan_text(&self, text: &str, result: &mut Phase1Result) {
        for (re, pat) in &self.block_patterns {
            if re.is_match(text) {
                result.should_block = true;
                result.score = 1.0;
                let msg = format!("Content matches block pattern: {pat}");
                result.block_reason = Some(msg.clone());
                result.violations.push(msg);
                return; // First match is sufficient to block
            }
        }
    }

    fn scan_json_strings(&self, value: &serde_json::Value, result: &mut Phase1Result) {
        match value {
            serde_json::Value::String(s) => self.scan_text(s, result),
            serde_json::Value::Object(map) => {
                for v in map.values() {
                    self.scan_json_strings(v, result);
                    if result.should_block {
                        return;
                    }
                }
            }
            serde_json::Value::Array(arr) => {
                for v in arr {
                    self.scan_json_strings(v, result);
                    if result.should_block {
                        return;
                    }
                }
            }
            _ => {}
        }
    }

    fn check_domain(&self, url: &str, result: &mut Phase1Result) {
        let domain = extract_domain(url);

        // Hard-block list first
        for pat in &self.block_domains {
            if pat.matches(&domain) {
                result.should_block = true;
                result.score = 1.0;
                let msg = format!("Domain '{domain}' matches block list");
                result.block_reason = Some(msg.clone());
                result.violations.push(msg);
                return;
            }
        }

        // Allowlist enforcement
        if self.block_on_unknown && !self.allow_domains.is_empty() {
            let allowed = self.allow_domains.iter().any(|p| p.matches(&domain));
            if !allowed {
                result.should_block = true;
                result.score = 1.0;
                let msg = format!("Domain '{domain}' is not in the allow list");
                result.block_reason = Some(msg.clone());
                result.violations.push(msg);
            }
        }
    }

    // ── Rate limit helpers ───────────────────────────────────────────────

    fn with_session_limits<F, R>(&self, session_id: &str, f: F) -> R
    where
        F: FnOnce(&mut SessionLimits) -> R,
    {
        let mut map = self.session_limits.lock().expect("sentinel phase1 lock poisoned");
        let limits = map.entry(session_id.to_string()).or_insert_with(|| {
            SessionLimits {
                tool_calls: SlidingWindow::new(
                    self.rate_cfg.tool_calls_per_minute,
                    Duration::from_secs(60),
                ),
                llm_requests: SlidingWindow::new(
                    self.rate_cfg.llm_requests_per_minute,
                    Duration::from_secs(60),
                ),
                file_writes: SlidingWindow::new(
                    self.rate_cfg.file_writes_per_minute,
                    Duration::from_secs(60),
                ),
            }
        });
        f(limits)
    }

    fn record_tool_call(&self, session_id: &str) -> bool {
        self.with_session_limits(session_id, |l| l.tool_calls.try_record())
    }

    fn record_llm_request(&self, session_id: &str) -> bool {
        self.with_session_limits(session_id, |l| l.llm_requests.try_record())
    }

    fn record_file_write(&self, session_id: &str) -> bool {
        self.with_session_limits(session_id, |l| l.file_writes.try_record())
    }
}

// ── Free-standing helpers ────────────────────────────────────────────────────

fn is_file_write_tool(tool: &str) -> bool {
    matches!(
        tool,
        "Write" | "Edit" | "NotebookEdit" | "write_file" | "create_file" | "append_file"
    )
}

/// Extract the first URL-like string value from tool args.
fn extract_url_from_args(args: &serde_json::Value) -> Option<String> {
    let obj = args.as_object()?;
    for key in &["url", "uri", "endpoint", "domain", "host"] {
        if let Some(serde_json::Value::String(v)) = obj.get(*key) {
            return Some(v.clone());
        }
    }
    None
}

/// Extract the hostname from a URL string (best-effort, no external deps).
fn extract_domain(url: &str) -> String {
    // Strip scheme
    let after_scheme = if let Some(pos) = url.find("://") {
        &url[pos + 3..]
    } else {
        url
    };
    // Take up to first '/' or ':'
    after_scheme
        .split(&['/', ':'][..])
        .next()
        .unwrap_or(after_scheme)
        .to_lowercase()
}

// ── Minimal glob-style domain matching ──────────────────────────────────────

mod glob_match_domain {
    /// A compiled domain pattern supporting a single leading `*.` wildcard.
    #[derive(Debug, Clone)]
    pub struct DomainPattern {
        raw: String,
        wildcard_suffix: Option<String>,
    }

    impl DomainPattern {
        pub fn new(pat: &str) -> Self {
            let pat = pat.to_lowercase();
            if pat.starts_with("*.") {
                Self {
                    wildcard_suffix: Some(pat[1..].to_string()), // keep the dot: ".foo.com"
                    raw: pat,
                }
            } else {
                Self { raw: pat, wildcard_suffix: None }
            }
        }

        pub fn matches(&self, domain: &str) -> bool {
            let d = domain.to_lowercase();
            if let Some(ref suffix) = self.wildcard_suffix {
                // "*.foo.com" matches "bar.foo.com" and "foo.com"
                d == self.raw[2..] || d.ends_with(suffix.as_str())
            } else {
                d == self.raw
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sentinel::config::{NetworkPolicy, Phase1Config, RateLimitConfig, ToolPolicy};

    fn make_engine(cfg: Phase1Config) -> Phase1Engine {
        Phase1Engine::from_config(&cfg)
    }

    fn default_cfg() -> Phase1Config {
        Phase1Config::default()
    }

    // ── Block patterns ───────────────────────────────────────────────────

    #[test]
    fn test_block_pattern_triggers_block() {
        let cfg = Phase1Config {
            block_patterns: vec!["(?i)rm -rf /".to_string()],
            ..default_cfg()
        };
        let engine = make_engine(cfg);
        let result = engine.check_tool_use(
            "s1",
            "Bash",
            &serde_json::json!({"command": "rm -rf /home/user"}),
        );
        assert!(result.should_block);
        assert_eq!(result.score, 1.0);
    }

    #[test]
    fn test_clean_command_passes() {
        let cfg = Phase1Config {
            block_patterns: vec!["(?i)rm -rf /".to_string()],
            ..default_cfg()
        };
        let engine = make_engine(cfg);
        let result = engine.check_tool_use(
            "s1",
            "Bash",
            &serde_json::json!({"command": "echo hello"}),
        );
        assert!(!result.should_block);
    }

    // ── Tool blocklist ───────────────────────────────────────────────────

    #[test]
    fn test_blocked_tool_is_hard_blocked() {
        let cfg = Phase1Config {
            tools: ToolPolicy {
                blocked: vec!["raw_shell".to_string()],
                always_analyze: vec![],
            },
            ..default_cfg()
        };
        let engine = make_engine(cfg);
        let result = engine.check_tool_use("s1", "raw_shell", &serde_json::json!({}));
        assert!(result.should_block);
    }

    #[test]
    fn test_always_analyze_tool_elevates_score() {
        let cfg = Phase1Config {
            tools: ToolPolicy {
                blocked: vec![],
                always_analyze: vec!["bash".to_string()],
            },
            phase2_trigger_score: 0.35,
            ..default_cfg()
        };
        let engine = make_engine(cfg);
        let result = engine.check_tool_use("s1", "bash", &serde_json::json!({}));
        assert!(!result.should_block);
        assert!(result.score >= 0.35);
    }

    // ── Network policy ───────────────────────────────────────────────────

    #[test]
    fn test_blocked_domain_is_hard_blocked() {
        let cfg = Phase1Config {
            network: NetworkPolicy {
                block_domains: vec!["*.evil.com".to_string()],
                allow_domains: vec![],
                block_on_unknown: false,
            },
            ..default_cfg()
        };
        let engine = make_engine(cfg);
        let result = engine.check_tool_use(
            "s1",
            "WebFetch",
            &serde_json::json!({"url": "https://badstuff.evil.com/payload"}),
        );
        assert!(result.should_block);
    }

    #[test]
    fn test_allowlisted_domain_passes_when_block_on_unknown() {
        let cfg = Phase1Config {
            network: NetworkPolicy {
                block_domains: vec![],
                allow_domains: vec!["api.anthropic.com".to_string()],
                block_on_unknown: true,
            },
            ..default_cfg()
        };
        let engine = make_engine(cfg);
        // Allowed domain should pass
        let ok = engine.check_tool_use(
            "s1",
            "http",
            &serde_json::json!({"url": "https://api.anthropic.com/v1/messages"}),
        );
        assert!(!ok.should_block);

        // Unknown domain should be blocked
        let blocked = engine.check_tool_use(
            "s1",
            "http",
            &serde_json::json!({"url": "https://unknown.example.com/data"}),
        );
        assert!(blocked.should_block);
    }

    // ── Rate limits ──────────────────────────────────────────────────────

    #[test]
    fn test_tool_call_rate_limit() {
        let cfg = Phase1Config {
            rate_limits: RateLimitConfig {
                tool_calls_per_minute: 2,
                llm_requests_per_minute: 0,
                file_writes_per_minute: 0,
            },
            ..default_cfg()
        };
        let engine = make_engine(cfg);

        // First two calls should pass
        assert!(!engine.check_tool_use("s1", "Read", &serde_json::json!({})).should_block);
        assert!(!engine.check_tool_use("s1", "Read", &serde_json::json!({})).should_block);

        // Third call exceeds limit
        let blocked = engine.check_tool_use("s1", "Read", &serde_json::json!({}));
        assert!(blocked.should_block);
    }

    #[test]
    fn test_rate_limits_are_per_session() {
        let cfg = Phase1Config {
            rate_limits: RateLimitConfig {
                tool_calls_per_minute: 1,
                llm_requests_per_minute: 0,
                file_writes_per_minute: 0,
            },
            ..default_cfg()
        };
        let engine = make_engine(cfg);

        // Session 1 uses its limit
        assert!(!engine.check_tool_use("s1", "Read", &serde_json::json!({})).should_block);
        assert!(engine.check_tool_use("s1", "Read", &serde_json::json!({})).should_block);

        // Session 2 has its own independent limit
        assert!(!engine.check_tool_use("s2", "Read", &serde_json::json!({})).should_block);
    }

    // ── Domain helpers ───────────────────────────────────────────────────

    #[test]
    fn test_extract_domain() {
        assert_eq!(extract_domain("https://api.example.com/path"), "api.example.com");
        assert_eq!(extract_domain("http://host:8080/route"), "host");
        assert_eq!(extract_domain("api.example.com"), "api.example.com");
    }

    #[test]
    fn test_glob_domain_wildcard() {
        use super::glob_match_domain::DomainPattern;
        let p = DomainPattern::new("*.evil.com");
        assert!(p.matches("bad.evil.com"));
        assert!(p.matches("very.bad.evil.com"));
        assert!(p.matches("evil.com")); // root itself matches
        assert!(!p.matches("notevil.com"));
    }
}
