//! Pre-warmed Phase 2 LLM session pool (P3).
//!
//! Creating a new LLM session for every suspicious event costs ~100-200 ms
//! of setup overhead before the LLM call even starts.  This pool maintains
//! `capacity` sessions ready to accept analysis requests immediately.
//!
//! # Usage
//!
//! ```text
//! let pool = Phase2Pool::new(3, session_manager, system_prompt).await;
//!
//! // In SentinelAgent::phase2_analyze():
//! pool.analyze(context, timeout_ms).await
//! ```
//!
//! # Session hygiene
//!
//! After each analysis the session's conversation history is cleared via
//! `SessionManager::clear()` before the session is returned to the pool.
//! This prevents cross-event contamination while avoiding session teardown
//! and re-creation costs.
//!
//! If the pool is exhausted (all sessions in use concurrently), a new
//! ephemeral session is created and destroyed after use — degraded but correct.

use super::{config::SeverityActions, parse_phase2_verdict, SecurityVerdict};
use a3s_code::session::{SessionConfig, SessionManager};
use std::sync::{Arc, Mutex};
use tokio::time::Duration;
use tracing;

pub struct Phase2Pool {
    available: Mutex<Vec<String>>,
    capacity: usize,
    session_manager: Arc<SessionManager>,
    system_prompt: String,
    severity_actions: SeverityActions,
}

impl Phase2Pool {
    /// Create the pool and pre-warm `capacity` sentinel sessions.
    pub async fn new(
        capacity: usize,
        session_manager: Arc<SessionManager>,
        system_prompt: String,
        severity_actions: SeverityActions,
    ) -> Arc<Self> {
        let pool = Arc::new(Self {
            available: Mutex::new(Vec::with_capacity(capacity)),
            capacity,
            session_manager,
            system_prompt,
            severity_actions,
        });
        pool.prefill().await;
        pool
    }

    async fn prefill(&self) {
        for _ in 0..self.capacity {
            match self.create_session().await {
                Ok(id) => {
                    self.available.lock().expect("phase2 pool lock poisoned").push(id);
                }
                Err(e) => {
                    tracing::warn!("Phase2Pool prefill failed (non-fatal): {e}");
                }
            }
        }
        tracing::debug!(
            warmed = self.available.lock().map(|g| g.len()).unwrap_or(0),
            capacity = self.capacity,
            "Phase2Pool pre-warmed"
        );
    }

    async fn create_session(&self) -> anyhow::Result<String> {
        let id = format!("sentinel-pool-{}", uuid::Uuid::new_v4());
        let cfg = SessionConfig {
            system_prompt: Some(self.system_prompt.clone()),
            ..Default::default()
        };
        self.session_manager.create_session(id.clone(), cfg).await?;
        Ok(id)
    }

    fn acquire(&self) -> Option<String> {
        self.available.lock().expect("phase2 pool lock poisoned").pop()
    }

    async fn release(&self, session_id: String) {
        // Clear history to prevent cross-event contamination.
        if let Err(e) = self.session_manager.clear(&session_id).await {
            tracing::debug!(session = %session_id, "Phase2Pool clear history failed: {e}");
            // Session is dirty — destroy it and don't return to pool.
            let _ = self.session_manager.destroy_session(&session_id).await;
            return;
        }
        let mut slots = self.available.lock().expect("phase2 pool lock poisoned");
        if slots.len() < self.capacity {
            slots.push(session_id);
        } else {
            drop(slots);
            let _ = self.session_manager.destroy_session(&session_id).await;
        }
    }

    /// Run a Phase 2 LLM analysis, respecting `timeout`.
    ///
    /// Returns `SecurityVerdict::allow()` (fail-open) on timeout or error.
    pub async fn analyze(&self, context: &str, timeout_ms: u64) -> SecurityVerdict {
        // Acquire a pre-warmed session or fall back to a fresh one.
        let (session_id, from_pool) = if let Some(id) = self.acquire() {
            (id, true)
        } else {
            tracing::debug!("Phase2Pool exhausted — creating ephemeral session");
            match self.create_session().await {
                Ok(id) => (id, false),
                Err(e) => {
                    tracing::warn!("Phase2Pool: failed to create fallback session: {e}");
                    return SecurityVerdict::allow();
                }
            }
        };

        let timeout = Duration::from_millis(timeout_ms);
        let result = tokio::time::timeout(
            timeout,
            self.session_manager.generate(&session_id, context),
        )
        .await;

        // Return or destroy the session.
        if from_pool {
            self.release(session_id).await;
        } else {
            let _ = self.session_manager.destroy_session(&session_id).await;
        }

        match result {
            Ok(Ok(r)) => parse_phase2_verdict(&r.text, &self.severity_actions),
            Ok(Err(e)) => {
                tracing::debug!("Phase2Pool LLM error: {e}");
                SecurityVerdict::allow()
            }
            Err(_) => {
                tracing::debug!(timeout_ms, "Phase2Pool analysis timed out — failing open");
                SecurityVerdict::allow()
            }
        }
    }
}
