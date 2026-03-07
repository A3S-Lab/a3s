//! Session lineage tracker (P2).
//!
//! Tracks the parent-child relationships between agent sessions so the
//! sentinel can apply stricter scrutiny to deeper sub-agents.
//!
//! # Trust model
//!
//! - Depth 0 (user-initiated): normal Phase 1 scoring.
//! - Depth 1 (main agent): score × 1.0 — no modifier.
//! - Depth 2 (sub-agent): score × 1.3 — elevated scrutiny.
//! - Depth 3+ (grandchild+): score × 1.8 — strong suspicion.
//!
//! The multiplier is applied to the raw Phase 1 score before it is fed
//! into the `SuspicionAccumulator`.  Hard-block rules are unaffected.
//!
//! # Registration
//!
//! Sessions are registered via [`LineageMap::register`].  When the `task`
//! tool spawns a sub-agent the child session ID includes a `subagent-` prefix
//! and carries the parent's `session_id` via `ToolContext`.  The sentinel hook
//! handler registers the lineage on first encounter if it has not been
//! pre-registered.

use std::collections::HashMap;
use std::sync::Mutex;

// ── Types ─────────────────────────────────────────────────────────────────────

/// Trust classification derived from session depth.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrustLevel {
    /// Depth 0 — directly user-initiated.
    User,
    /// Depth 1 — spawned by the main agent.
    Agent,
    /// Depth 2+ — automatically generated sub-agents.
    SubAgent,
}

/// Lineage record for one agent session.
#[derive(Debug, Clone)]
pub struct AgentLineage {
    pub session_id: String,
    pub parent_id: Option<String>,
    /// Number of hops from the user-initiated session (0 = user, 1 = agent, …).
    pub depth: u8,
    pub trust_level: TrustLevel,
}

impl AgentLineage {
    /// Score multiplier based on session depth.
    ///
    /// Applied to the raw Phase 1 score before accumulation.
    pub fn score_multiplier(&self) -> f32 {
        match self.depth {
            0 | 1 => 1.0,
            2 => 1.3,
            _ => 1.8,
        }
    }
}

// ── LineageMap ────────────────────────────────────────────────────────────────

/// Thread-safe registry mapping session IDs to their lineage.
pub struct LineageMap {
    entries: Mutex<HashMap<String, AgentLineage>>,
}

impl LineageMap {
    pub fn new() -> Self {
        Self { entries: Mutex::new(HashMap::new()) }
    }

    /// Register a session with an explicit parent.
    ///
    /// If `parent_id` is `None` the session is treated as user-initiated
    /// (depth 0).  If the parent is not yet registered the depth defaults
    /// to 1 (safe over-approximation).
    pub fn register(&self, session_id: &str, parent_id: Option<&str>) {
        let mut map = self.entries.lock().expect("lineage lock poisoned");
        let depth = parent_id
            .and_then(|pid| map.get(pid))
            .map(|p| p.depth.saturating_add(1))
            .unwrap_or(if parent_id.is_some() { 1 } else { 0 });

        let trust_level = match depth {
            0 => TrustLevel::User,
            1 => TrustLevel::Agent,
            _ => TrustLevel::SubAgent,
        };

        map.insert(
            session_id.to_string(),
            AgentLineage {
                session_id: session_id.to_string(),
                parent_id: parent_id.map(str::to_string),
                depth,
                trust_level,
            },
        );
    }

    /// Register a sub-agent session inferred from its session ID prefix.
    ///
    /// Sub-agent sessions created by the `task` tool use IDs of the form
    /// `subagent-task-<uuid>`.  When no explicit parent is known, this
    /// method records depth 1 as a conservative default.
    pub fn register_inferred(&self, session_id: &str) {
        let is_subagent = session_id.starts_with("subagent-") || session_id.starts_with("sentinel-");
        let parent = None; // parent not known at hook time
        let depth = if is_subagent { 1u8 } else { 0u8 };

        let mut map = self.entries.lock().expect("lineage lock poisoned");
        map.entry(session_id.to_string()).or_insert_with(|| AgentLineage {
            session_id: session_id.to_string(),
            parent_id: parent,
            depth,
            trust_level: if depth == 0 { TrustLevel::User } else { TrustLevel::SubAgent },
        });
    }

    /// Get the lineage for a session, registering it via inference if unknown.
    pub fn get_or_infer(&self, session_id: &str) -> AgentLineage {
        {
            let map = self.entries.lock().expect("lineage lock poisoned");
            if let Some(l) = map.get(session_id) {
                return l.clone();
            }
        }
        self.register_inferred(session_id);
        let map = self.entries.lock().expect("lineage lock poisoned");
        map.get(session_id).cloned().unwrap_or_else(|| AgentLineage {
            session_id: session_id.to_string(),
            parent_id: None,
            depth: 0,
            trust_level: TrustLevel::User,
        })
    }

    /// Remove a session from the registry (call on session destruction).
    pub fn remove(&self, session_id: &str) {
        self.entries.lock().expect("lineage lock poisoned").remove(session_id);
    }

    /// Return the number of registered sessions.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.entries.lock().expect("lineage lock poisoned").len()
    }
}

impl Default for LineageMap {
    fn default() -> Self {
        Self::new()
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_session_depth_zero() {
        let m = LineageMap::new();
        m.register("user-sess", None);
        let l = m.get_or_infer("user-sess");
        assert_eq!(l.depth, 0);
        assert_eq!(l.trust_level, TrustLevel::User);
        assert_eq!(l.score_multiplier(), 1.0);
    }

    #[test]
    fn test_child_inherits_parent_depth() {
        let m = LineageMap::new();
        m.register("parent", None);       // depth 0
        m.register("child", Some("parent")); // depth 1
        m.register("grandchild", Some("child")); // depth 2

        assert_eq!(m.get_or_infer("child").depth, 1);
        assert_eq!(m.get_or_infer("grandchild").depth, 2);
        assert!(m.get_or_infer("grandchild").score_multiplier() > 1.0);
    }

    #[test]
    fn test_unknown_parent_defaults_to_depth_one() {
        let m = LineageMap::new();
        m.register("orphan", Some("unknown-parent"));
        assert_eq!(m.get_or_infer("orphan").depth, 1);
    }

    #[test]
    fn test_subagent_prefix_inferred_as_subagent() {
        let m = LineageMap::new();
        let l = m.get_or_infer("subagent-task-abc");
        assert_eq!(l.trust_level, TrustLevel::SubAgent);
        assert!(l.depth >= 1);
    }

    #[test]
    fn test_score_multiplier_deepens() {
        let m = LineageMap::new();
        m.register("d0", None);
        m.register("d1", Some("d0"));
        m.register("d2", Some("d1"));
        m.register("d3", Some("d2"));

        assert!(m.get_or_infer("d2").score_multiplier() > m.get_or_infer("d1").score_multiplier());
        assert!(m.get_or_infer("d3").score_multiplier() >= m.get_or_infer("d2").score_multiplier());
    }

    #[test]
    fn test_remove() {
        let m = LineageMap::new();
        m.register("s1", None);
        assert_eq!(m.len(), 1);
        m.remove("s1");
        assert_eq!(m.len(), 0);
    }
}
