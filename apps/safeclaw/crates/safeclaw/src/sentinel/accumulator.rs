//! Per-session suspicion accumulator (P1).
//!
//! Each session maintains a sliding-window suspicion score that accumulates
//! across individual events.  A single benign-looking event with score 0.3
//! does not trigger Phase 2, but five such events within the window do —
//! defeating temporal-evasion attacks that spread suspicious activity thinly.
//!
//! # Algorithm
//!
//! - Events are recorded with a timestamp and their Phase 1 score.
//! - On each `record()` call, events older than `window` are evicted.
//! - The accumulated score is the **sum** of all scores in the window.
//! - When the accumulated score exceeds `PHASE2_TRIGGER` a Phase 2 analysis
//!   is requested, subject to a per-session cooldown so Phase 2 cannot be
//!   triggered more than once per `phase2_cooldown` interval.
//! - If the accumulated score exceeds `HARD_BLOCK_THRESHOLD` the event is
//!   hard-blocked without waiting for Phase 2.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

/// Accumulated score threshold that triggers Phase 2 analysis.
const PHASE2_TRIGGER: f32 = 1.2;

/// Accumulated score threshold for an immediate hard block.
const HARD_BLOCK_THRESHOLD: f32 = 2.5;

/// Decision returned from [`SuspicionAccumulator::record`].
#[derive(Debug, Clone)]
pub enum AccumulatorDecision {
    /// Nothing remarkable — allow the event to continue normal processing.
    Continue,
    /// Accumulated score crossed the Phase 2 threshold.
    TriggerPhase2 { accumulated_score: f32 },
    /// Accumulated score is so high the event should be hard-blocked immediately.
    HardBlock { accumulated_score: f32 },
}

// ── Per-session state ─────────────────────────────────────────────────────────

struct SessionAccumulator {
    events: std::collections::VecDeque<(Instant, f32)>,
    window: Duration,
    last_phase2_at: Option<Instant>,
    phase2_cooldown: Duration,
}

impl SessionAccumulator {
    fn new(window: Duration, phase2_cooldown: Duration) -> Self {
        Self {
            events: std::collections::VecDeque::new(),
            window,
            last_phase2_at: None,
            phase2_cooldown,
        }
    }

    fn record(&mut self, score: f32) -> AccumulatorDecision {
        if score <= 0.0 {
            return AccumulatorDecision::Continue;
        }
        let now = Instant::now();

        // Evict stale events
        self.events.retain(|(t, _)| now.duration_since(*t) < self.window);
        self.events.push_back((now, score));

        let accumulated: f32 = self.events.iter().map(|(_, s)| s).sum();

        if accumulated >= HARD_BLOCK_THRESHOLD {
            return AccumulatorDecision::HardBlock { accumulated_score: accumulated };
        }

        let in_cooldown = self
            .last_phase2_at
            .map(|t| now.duration_since(t) < self.phase2_cooldown)
            .unwrap_or(false);

        if accumulated >= PHASE2_TRIGGER && !in_cooldown {
            self.last_phase2_at = Some(now);
            AccumulatorDecision::TriggerPhase2 { accumulated_score: accumulated }
        } else {
            AccumulatorDecision::Continue
        }
    }
}

// ── Global accumulator map ────────────────────────────────────────────────────

/// Thread-safe map of per-session suspicion accumulators.
pub struct SuspicionAccumulatorMap {
    sessions: Mutex<HashMap<String, SessionAccumulator>>,
    window: Duration,
    phase2_cooldown: Duration,
}

impl SuspicionAccumulatorMap {
    pub fn new(window: Duration, phase2_cooldown: Duration) -> Self {
        Self {
            sessions: Mutex::new(HashMap::new()),
            window,
            phase2_cooldown,
        }
    }

    /// Record a Phase 1 score for `session_id` and return the escalation decision.
    pub fn record(&self, session_id: &str, score: f32) -> AccumulatorDecision {
        let mut map = self.sessions.lock().expect("accumulator lock poisoned");
        let acc = map.entry(session_id.to_string()).or_insert_with(|| {
            SessionAccumulator::new(self.window, self.phase2_cooldown)
        });
        acc.record(score)
    }

    /// Remove the accumulator for a session (call when session is destroyed).
    pub fn remove(&self, session_id: &str) {
        self.sessions.lock().expect("accumulator lock poisoned").remove(session_id);
    }
}

impl Default for SuspicionAccumulatorMap {
    fn default() -> Self {
        Self::new(
            Duration::from_secs(300), // 5-minute window
            Duration::from_secs(30),  // 30-second Phase 2 cooldown
        )
    }
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn map() -> SuspicionAccumulatorMap {
        // Tight windows for testing
        SuspicionAccumulatorMap::new(Duration::from_secs(60), Duration::from_secs(1))
    }

    #[test]
    fn test_single_low_score_continues() {
        let m = map();
        assert!(matches!(m.record("s1", 0.3), AccumulatorDecision::Continue));
    }

    #[test]
    fn test_accumulation_triggers_phase2() {
        let m = map();
        // Three events of 0.3 each → 0.9 < PHASE2_TRIGGER (1.2) → Continue
        for _ in 0..3 {
            let d = m.record("s1", 0.3);
            assert!(matches!(d, AccumulatorDecision::Continue));
        }
        // Fourth event → 1.2 >= PHASE2_TRIGGER → TriggerPhase2
        let d = m.record("s1", 0.3);
        assert!(matches!(d, AccumulatorDecision::TriggerPhase2 { .. }));
    }

    #[test]
    fn test_hard_block_threshold() {
        let m = map();
        // Push score above HARD_BLOCK_THRESHOLD in one go
        let d = m.record("s1", 3.0);
        assert!(matches!(d, AccumulatorDecision::HardBlock { .. }));
    }

    #[test]
    fn test_phase2_cooldown_prevents_spam() {
        let m = SuspicionAccumulatorMap::new(Duration::from_secs(60), Duration::from_secs(60));
        // First trigger
        m.record("s1", 0.5);
        m.record("s1", 0.5);
        let d1 = m.record("s1", 0.5); // 1.5 > 1.2 → TriggerPhase2
        assert!(matches!(d1, AccumulatorDecision::TriggerPhase2 { .. }));

        // Second trigger within cooldown → Continue
        let d2 = m.record("s1", 0.5);
        assert!(matches!(d2, AccumulatorDecision::Continue));
    }

    #[test]
    fn test_sessions_are_independent() {
        let m = map();
        for _ in 0..5 {
            m.record("s1", 0.3);
        }
        // s2 is a fresh session — should not be affected
        assert!(matches!(m.record("s2", 0.1), AccumulatorDecision::Continue));
    }

    #[test]
    fn test_zero_score_is_ignored() {
        let m = map();
        for _ in 0..100 {
            assert!(matches!(m.record("s1", 0.0), AccumulatorDecision::Continue));
        }
    }

    #[test]
    fn test_remove_clears_state() {
        let m = map();
        for _ in 0..4 {
            m.record("s1", 0.3);
        }
        m.remove("s1");
        // After removal the accumulator resets — single event should continue
        assert!(matches!(m.record("s1", 0.3), AccumulatorDecision::Continue));
    }
}
