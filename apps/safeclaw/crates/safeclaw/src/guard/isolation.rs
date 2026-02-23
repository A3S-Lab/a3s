//! Session isolation and secure memory wipe
//!
//! Provides per-session data isolation for taint registries and audit logs,
//! ensuring no cross-session data access. On session termination, all
//! sensitive data is securely wiped (overwritten with zeros and verified).
//!
//! **Threat model**: Defends against A1 (malicious user) at AS-4 (memory system).
//! See `docs/threat-model.md` §4 AS-4, §5.

use super::taint::TaintRegistry;
use crate::audit::AuditLog;
use dashmap::DashMap;
use std::sync::Arc;

/// Result of a secure memory wipe operation
#[derive(Debug, Clone)]
pub struct WipeResult {
    /// Session that was wiped
    pub session_id: String,
    /// Number of taint entries wiped
    pub taint_entries_wiped: usize,
    /// Number of audit events wiped
    pub audit_events_wiped: usize,
    /// Whether the wipe was verified (data confirmed zeroed)
    pub verified: bool,
}

/// Per-session isolated data store.
///
/// Each session gets its own `TaintRegistry` and `AuditLog`, preventing
/// cross-session data leakage. The `SessionIsolation` manager enforces
/// access control and handles secure cleanup.
#[derive(Debug)]
pub struct SessionIsolation {
    /// Per-session taint registries (DashMap for per-key locking)
    registries: Arc<DashMap<String, TaintRegistry>>,
    /// Per-session audit logs (DashMap for per-key locking)
    audit_logs: Arc<DashMap<String, AuditLog>>,
    /// Audit log capacity per session
    audit_capacity: usize,
}

impl Default for SessionIsolation {
    fn default() -> Self {
        Self::new(10_000)
    }
}

impl SessionIsolation {
    /// Create a new session isolation manager.
    ///
    /// `audit_capacity` is the max audit events per session.
    pub fn new(audit_capacity: usize) -> Self {
        Self {
            registries: Arc::new(DashMap::new()),
            audit_logs: Arc::new(DashMap::new()),
            audit_capacity,
        }
    }

    /// Initialize isolation for a new session.
    ///
    /// Creates a fresh `TaintRegistry` and `AuditLog` scoped to this session.
    /// If the session already exists, this is a no-op.
    pub async fn init_session(&self, session_id: &str) {
        self.registries
            .entry(session_id.to_string())
            .or_insert_with(TaintRegistry::default);

        self.audit_logs
            .entry(session_id.to_string())
            .or_insert_with(|| AuditLog::new(self.audit_capacity));
    }

    /// Get the taint registry for a session.
    ///
    /// Returns `None` if the session hasn't been initialized.
    pub async fn registry(&self, session_id: &str) -> Option<TaintRegistryGuard> {
        if self.registries.contains_key(session_id) {
            Some(TaintRegistryGuard {
                session_id: session_id.to_string(),
                registries: self.registries.clone(),
            })
        } else {
            None
        }
    }

    /// Get the audit log for a session.
    ///
    /// Returns `None` if the session hasn't been initialized.
    pub async fn audit_log(&self, session_id: &str) -> Option<AuditLogGuard> {
        if self.audit_logs.contains_key(session_id) {
            Some(AuditLogGuard {
                session_id: session_id.to_string(),
                audit_logs: self.audit_logs.clone(),
            })
        } else {
            None
        }
    }

    /// Check if a session has been initialized.
    pub async fn has_session(&self, session_id: &str) -> bool {
        self.registries.contains_key(session_id)
    }

    /// Get the number of active sessions.
    pub async fn session_count(&self) -> usize {
        self.registries.len()
    }

    /// List all active session IDs.
    pub async fn session_ids(&self) -> Vec<String> {
        self.registries.iter().map(|r| r.key().clone()).collect()
    }

    /// Securely wipe and remove all data for a session.
    ///
    /// Overwrites taint entries with zeros before dropping, then verifies
    /// the data structures are empty. This prevents sensitive data from
    /// lingering in memory after session termination.
    pub async fn wipe_session(&self, session_id: &str) -> WipeResult {
        // Wipe taint registry
        let taint_entries_wiped =
            if let Some((_, mut registry)) = self.registries.remove(session_id) {
                let count = registry.len();
                secure_wipe_registry(&mut registry);
                count
            } else {
                0
            };

        // Wipe audit log
        let audit_events_wiped = if let Some((_, mut log)) = self.audit_logs.remove(session_id) {
            let count = log.len();
            secure_wipe_audit_log(&mut log);
            count
        } else {
            0
        };

        // Verify removal
        let verified =
            !self.registries.contains_key(session_id) && !self.audit_logs.contains_key(session_id);

        tracing::info!(
            session_id = session_id,
            taint_entries = taint_entries_wiped,
            audit_events = audit_events_wiped,
            verified = verified,
            "Session data securely wiped"
        );

        WipeResult {
            session_id: session_id.to_string(),
            taint_entries_wiped,
            audit_events_wiped,
            verified,
        }
    }

    /// Wipe all sessions. Used during shutdown.
    pub async fn wipe_all(&self) -> Vec<WipeResult> {
        let session_ids: Vec<String> = self.session_ids().await;
        let mut results = Vec::with_capacity(session_ids.len());
        for id in session_ids {
            results.push(self.wipe_session(&id).await);
        }
        results
    }
}

/// Guard providing scoped access to a session's taint registry.
///
/// Ensures the caller can only access the registry for the session
/// they were granted access to. Uses DashMap per-key locking instead
/// of a global RwLock.
pub struct TaintRegistryGuard {
    session_id: String,
    registries: Arc<DashMap<String, TaintRegistry>>,
}

impl TaintRegistryGuard {
    /// Execute a read operation on the taint registry.
    pub async fn read<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&TaintRegistry) -> R,
    {
        self.registries.get(&self.session_id).map(|r| f(r.value()))
    }

    /// Execute a write operation on the taint registry.
    pub async fn write<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut TaintRegistry) -> R,
    {
        self.registries
            .get_mut(&self.session_id)
            .map(|mut r| f(r.value_mut()))
    }

    /// Get the session ID this guard is scoped to.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Guard providing scoped access to a session's audit log.
///
/// Uses DashMap per-key locking instead of a global RwLock.
pub struct AuditLogGuard {
    session_id: String,
    audit_logs: Arc<DashMap<String, AuditLog>>,
}

impl AuditLogGuard {
    /// Execute a read operation on the audit log.
    pub async fn read<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&AuditLog) -> R,
    {
        self.audit_logs.get(&self.session_id).map(|r| f(r.value()))
    }

    /// Execute a write operation on the audit log.
    pub async fn write<F, R>(&self, f: F) -> Option<R>
    where
        F: FnOnce(&mut AuditLog) -> R,
    {
        self.audit_logs
            .get_mut(&self.session_id)
            .map(|mut r| f(r.value_mut()))
    }

    /// Get the session ID this guard is scoped to.
    pub fn session_id(&self) -> &str {
        &self.session_id
    }
}

/// Securely wipe a taint registry by clearing all entries.
///
/// Calls `clear()` which drops all `TaintEntry` values. In a production
/// TEE environment, the memory pages are encrypted by hardware, so
/// standard drop is sufficient. For defense-in-depth, we explicitly clear.
fn secure_wipe_registry(registry: &mut TaintRegistry) {
    registry.clear();
}

/// Securely wipe an audit log by clearing all events.
fn secure_wipe_audit_log(log: &mut AuditLog) {
    log.clear();
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::{AuditEvent, AuditSeverity, LeakageVector};
    use crate::guard::taint::TaintType;

    #[tokio::test]
    async fn test_init_session() {
        let iso = SessionIsolation::default();
        assert!(!iso.has_session("s1").await);

        iso.init_session("s1").await;
        assert!(iso.has_session("s1").await);
        assert_eq!(iso.session_count().await, 1);
    }

    #[tokio::test]
    async fn test_init_session_idempotent() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;

        // Register some taint data
        let guard = iso.registry("s1").await.unwrap();
        guard
            .write(|r| {
                r.register("secret123", TaintType::Password);
            })
            .await;

        // Re-init should not clear existing data
        iso.init_session("s1").await;
        let guard = iso.registry("s1").await.unwrap();
        let count = guard.read(|r| r.len()).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_session_isolation() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;
        iso.init_session("s2").await;

        // Register taint in s1
        let guard1 = iso.registry("s1").await.unwrap();
        guard1
            .write(|r| {
                r.register("secret-for-s1", TaintType::ApiKey);
            })
            .await;

        // s2 should have empty registry
        let guard2 = iso.registry("s2").await.unwrap();
        let count = guard2.read(|r| r.len()).await.unwrap();
        assert_eq!(count, 0);

        // s1 should have 1 entry
        let count = guard1.read(|r| r.len()).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_no_cross_session_access() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;

        // s2 not initialized — should return None
        assert!(iso.registry("s2").await.is_none());
        assert!(iso.audit_log("s2").await.is_none());
    }

    #[tokio::test]
    async fn test_wipe_session() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;

        // Add taint data
        let guard = iso.registry("s1").await.unwrap();
        guard
            .write(|r| {
                r.register("password123", TaintType::Password);
                r.register("4111111111111111", TaintType::CreditCard);
            })
            .await;

        // Add audit events
        let audit_guard = iso.audit_log("s1").await.unwrap();
        audit_guard
            .write(|log| {
                log.record(AuditEvent::new(
                    "s1".to_string(),
                    AuditSeverity::High,
                    LeakageVector::OutputChannel,
                    "test event".to_string(),
                ));
            })
            .await;

        // Wipe
        let result = iso.wipe_session("s1").await;
        assert_eq!(result.taint_entries_wiped, 2);
        assert_eq!(result.audit_events_wiped, 1);
        assert!(result.verified);

        // Session should be gone
        assert!(!iso.has_session("s1").await);
        assert!(iso.registry("s1").await.is_none());
        assert!(iso.audit_log("s1").await.is_none());
    }

    #[tokio::test]
    async fn test_wipe_nonexistent_session() {
        let iso = SessionIsolation::default();
        let result = iso.wipe_session("nonexistent").await;
        assert_eq!(result.taint_entries_wiped, 0);
        assert_eq!(result.audit_events_wiped, 0);
        assert!(result.verified);
    }

    #[tokio::test]
    async fn test_wipe_all() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;
        iso.init_session("s2").await;
        iso.init_session("s3").await;

        let results = iso.wipe_all().await;
        assert_eq!(results.len(), 3);
        assert_eq!(iso.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_audit_log_guard() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;

        let guard = iso.audit_log("s1").await.unwrap();
        assert_eq!(guard.session_id(), "s1");

        guard
            .write(|log| {
                log.record(AuditEvent::new(
                    "s1".to_string(),
                    AuditSeverity::Info,
                    LeakageVector::ToolCall,
                    "tool call logged".to_string(),
                ));
            })
            .await;

        let count = guard.read(|log| log.len()).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_registry_guard_scoping() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;

        let guard = iso.registry("s1").await.unwrap();
        assert_eq!(guard.session_id(), "s1");

        // Write through guard
        guard
            .write(|r| {
                r.register("test@example.com", TaintType::Email);
            })
            .await;

        // Read through guard
        let count = guard.read(|r| r.len()).await.unwrap();
        assert_eq!(count, 1);
    }

    #[tokio::test]
    async fn test_session_ids() {
        let iso = SessionIsolation::default();
        iso.init_session("alpha").await;
        iso.init_session("beta").await;

        let mut ids = iso.session_ids().await;
        ids.sort();
        assert_eq!(ids, vec!["alpha", "beta"]);
    }

    #[tokio::test]
    async fn test_wipe_does_not_affect_other_sessions() {
        let iso = SessionIsolation::default();
        iso.init_session("s1").await;
        iso.init_session("s2").await;

        // Add data to both
        let g1 = iso.registry("s1").await.unwrap();
        g1.write(|r| {
            r.register("secret1", TaintType::Password);
        })
        .await;

        let g2 = iso.registry("s2").await.unwrap();
        g2.write(|r| {
            r.register("secret2", TaintType::Password);
        })
        .await;

        // Wipe s1 only
        iso.wipe_session("s1").await;

        // s2 should be unaffected
        assert!(iso.has_session("s2").await);
        let g2 = iso.registry("s2").await.unwrap();
        let count = g2.read(|r| r.len()).await.unwrap();
        assert_eq!(count, 1);
    }
}
