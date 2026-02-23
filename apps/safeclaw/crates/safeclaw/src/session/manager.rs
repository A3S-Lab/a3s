//! Unified session management
//!
//! Provides a single `Session` type that optionally supports TEE processing,
//! and a `SessionManager` that handles both regular and TEE session lifecycles.
//!
//! TEE processing is backed by `TeeRuntime` which detects the TEE environment
//! at startup (Phase 11 architecture). SafeClaw runs as a guest inside the TEE,
//! not as a host that boots VMs.

use crate::audit::AuditEventBus;
use crate::config::{SensitivityLevel, TeeConfig};
use crate::error::{Error, Result};
use crate::guard::{
    FirewallResult, InjectionVerdict, InterceptResult, SanitizeResult, SessionIsolation,
};
use crate::privacy::{CumulativeRiskDecision, PrivacyPipeline, SessionPrivacyContext};
use crate::tee::TeeRuntime;
use dashmap::DashMap;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Session state
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionState {
    /// Session is being created
    Creating,
    /// Session is active
    Active,
    /// Session is processing a message (also covers TEE "Busy")
    Processing,
    /// Session is paused
    Paused,
    /// Session is being terminated
    Terminating,
    /// Session has been terminated
    Terminated,
}

/// A user session, optionally backed by a TEE environment.
#[derive(Debug)]
pub struct Session {
    /// Session ID
    pub id: String,
    /// User ID
    pub user_id: String,
    /// Channel ID
    pub channel_id: String,
    /// Chat ID (channel-specific)
    pub chat_id: String,
    /// Current state
    state: Arc<RwLock<SessionState>>,
    /// Highest sensitivity level seen
    sensitivity_level: Arc<RwLock<SensitivityLevel>>,
    /// Creation timestamp
    pub created_at: i64,
    /// Last activity timestamp
    last_activity: Arc<RwLock<i64>>,
    /// Message count
    message_count: Arc<RwLock<u64>>,
    /// Session metadata
    metadata: Arc<RwLock<HashMap<String, serde_json::Value>>>,
    /// Whether this session has been upgraded to TEE
    tee_active: Arc<RwLock<bool>>,
    /// Per-session cumulative privacy context for split-message attack defense
    privacy_context: Arc<RwLock<SessionPrivacyContext>>,
}

impl Session {
    /// Create a new session
    pub fn new(user_id: String, channel_id: String, chat_id: String) -> Self {
        let now = chrono::Utc::now().timestamp_millis();
        Self {
            id: Uuid::new_v4().to_string(),
            user_id,
            channel_id,
            chat_id,
            state: Arc::new(RwLock::new(SessionState::Creating)),
            sensitivity_level: Arc::new(RwLock::new(SensitivityLevel::Normal)),
            created_at: now,
            last_activity: Arc::new(RwLock::new(now)),
            message_count: Arc::new(RwLock::new(0)),
            metadata: Arc::new(RwLock::new(HashMap::new())),
            tee_active: Arc::new(RwLock::new(false)),
            privacy_context: Arc::new(RwLock::new(SessionPrivacyContext::new())),
        }
    }

    /// Get current state
    pub async fn state(&self) -> SessionState {
        *self.state.read().await
    }

    /// Set state
    pub async fn set_state(&self, state: SessionState) {
        *self.state.write().await = state;
    }

    /// Check if session is active
    pub async fn is_active(&self) -> bool {
        matches!(
            self.state().await,
            SessionState::Active | SessionState::Processing
        )
    }

    /// Update last activity
    pub async fn touch(&self) {
        *self.last_activity.write().await = chrono::Utc::now().timestamp_millis();
    }

    /// Get last activity timestamp
    pub async fn last_activity(&self) -> i64 {
        *self.last_activity.read().await
    }

    /// Increment message count
    pub async fn increment_messages(&self) {
        *self.message_count.write().await += 1;
    }

    /// Get message count
    pub async fn message_count(&self) -> u64 {
        *self.message_count.read().await
    }

    /// Update sensitivity level (only increases)
    pub async fn update_sensitivity(&self, level: SensitivityLevel) {
        let mut current = self.sensitivity_level.write().await;
        if level as u8 > *current as u8 {
            *current = level;
        }
    }

    /// Get current sensitivity level
    pub async fn sensitivity_level(&self) -> SensitivityLevel {
        *self.sensitivity_level.read().await
    }

    /// Record PII disclosures from a classification result.
    ///
    /// Called by `SessionRouter::route()` after each message is classified.
    /// Feeds the cumulative privacy context used for split-message attack detection.
    pub async fn record_disclosures(&self, rule_names: &[String], sensitivity: SensitivityLevel) {
        self.privacy_context
            .write()
            .await
            .record_disclosures(rule_names, sensitivity);
    }

    /// Assess cumulative PII risk for this session.
    ///
    /// Returns `CumulativeRiskDecision::Reject` when the session has disclosed
    /// `reject_threshold` or more distinct PII types across all messages.
    pub async fn assess_privacy_risk(
        &self,
        warn_threshold: usize,
        reject_threshold: usize,
    ) -> CumulativeRiskDecision {
        self.privacy_context
            .read()
            .await
            .assess_risk(warn_threshold, reject_threshold)
    }

    /// Set metadata value
    pub async fn set_metadata(&self, key: impl Into<String>, value: serde_json::Value) {
        self.metadata.write().await.insert(key.into(), value);
    }

    /// Get metadata value
    pub async fn get_metadata(&self, key: &str) -> Option<serde_json::Value> {
        self.metadata.read().await.get(key).cloned()
    }

    /// Upgrade this session to use TEE processing.
    pub async fn mark_tee_active(&self) {
        *self.tee_active.write().await = true;
    }

    /// Check if this session uses TEE
    pub async fn uses_tee(&self) -> bool {
        *self.tee_active.read().await
    }
}

/// Unified session manager handling both regular and TEE sessions.
pub struct SessionManager {
    /// Active sessions indexed by session ID (DashMap for per-key locking)
    sessions: Arc<DashMap<String, Arc<Session>>>,
    /// Sessions indexed by user_id:channel_id:chat_id (DashMap for per-key locking)
    user_sessions: Arc<DashMap<String, String>>,
    /// TEE configuration
    tee_config: TeeConfig,
    /// TEE runtime for self-detection and sealed storage (Phase 11)
    tee_runtime: Arc<TeeRuntime>,
    /// Unified privacy + protection pipeline
    pipeline: PrivacyPipeline,
}

impl SessionManager {
    /// Create a new session manager with TEE configuration and audit bus.
    ///
    /// Uses `TeeRuntime` for environment self-detection (Phase 11).
    /// The runtime detects TEE hardware at startup — no VM boot needed.
    pub fn new(tee_config: TeeConfig, audit_bus: Arc<AuditEventBus>) -> Self {
        let tee_runtime = Arc::new(TeeRuntime::process_only());
        let pipeline = PrivacyPipeline::new(tee_config.network_policy.clone(), audit_bus);
        Self {
            sessions: Arc::new(DashMap::new()),
            user_sessions: Arc::new(DashMap::new()),
            tee_config,
            tee_runtime,
            pipeline,
        }
    }

    /// Create a new session manager with a pre-detected TEE runtime.
    pub fn with_runtime(
        tee_config: TeeConfig,
        tee_runtime: Arc<TeeRuntime>,
        audit_bus: Arc<AuditEventBus>,
    ) -> Self {
        let pipeline = PrivacyPipeline::new(tee_config.network_policy.clone(), audit_bus);
        Self {
            sessions: Arc::new(DashMap::new()),
            user_sessions: Arc::new(DashMap::new()),
            tee_config,
            tee_runtime,
            pipeline,
        }
    }

    /// Initialize the TEE subsystem.
    ///
    /// With TeeRuntime, detection happens at startup. This logs the result.
    pub async fn init_tee(&self) -> Result<()> {
        if !self.tee_config.enabled {
            tracing::info!("TEE is disabled, skipping initialization");
            return Ok(());
        }

        tracing::info!(
            level = %self.tee_runtime.security_level(),
            "TEE runtime initialized"
        );
        Ok(())
    }

    /// Shutdown the TEE subsystem
    pub async fn shutdown_tee(&self) -> Result<()> {
        if !self.tee_config.enabled {
            return Ok(());
        }

        tracing::info!("Shutting down TEE subsystem");

        // Terminate all active sessions
        let sessions: Vec<Arc<Session>> =
            { self.sessions.iter().map(|r| r.value().clone()).collect() };

        for session in sessions {
            if session.is_active().await {
                session.set_state(SessionState::Terminating).await;
            }
        }

        // Shutdown the TEE runtime
        self.tee_runtime.shutdown().await?;

        // Wipe all session isolation data
        self.pipeline.isolation().wipe_all().await;

        tracing::info!("TEE subsystem shutdown complete");
        Ok(())
    }

    /// Check if TEE is enabled
    pub fn is_tee_enabled(&self) -> bool {
        self.tee_config.enabled
    }

    /// Get a reference to the TEE runtime
    pub fn tee_runtime(&self) -> &Arc<TeeRuntime> {
        &self.tee_runtime
    }

    /// Get a reference to the session isolation manager
    pub fn isolation(&self) -> &Arc<SessionIsolation> {
        self.pipeline.isolation()
    }

    /// Get a reference to the injection detector
    pub fn injection_detector(&self) -> &Arc<crate::guard::InjectionDetector> {
        self.pipeline.injection_detector()
    }

    /// Get a reference to the network firewall
    pub fn network_firewall(&self) -> &Arc<crate::guard::NetworkFirewall> {
        self.pipeline.network_firewall()
    }

    /// Get a reference to the audit event bus
    pub fn audit_bus(&self) -> &Arc<AuditEventBus> {
        self.pipeline.audit_bus()
    }

    /// Sanitize AI output for a session, publishing any audit events.
    pub async fn sanitize_output(&self, session_id: &str, output: &str) -> SanitizeResult {
        self.pipeline.sanitize_output(session_id, output).await
    }

    /// Intercept a tool call for a session, publishing any audit events.
    pub async fn intercept_tool_call(
        &self,
        session_id: &str,
        tool_name: &str,
        arguments: &str,
    ) -> InterceptResult {
        self.pipeline
            .intercept_tool_call(session_id, tool_name, arguments)
            .await
    }

    /// Check a URL against the network firewall, publishing any audit event.
    pub async fn check_firewall(&self, url: &str, session_id: &str) -> FirewallResult {
        self.pipeline.check_firewall(url, session_id).await
    }

    /// Create a new session
    pub async fn create_session(
        &self,
        user_id: &str,
        channel_id: &str,
        chat_id: &str,
    ) -> Result<Arc<Session>> {
        let user_key = format!("{}:{}:{}", user_id, channel_id, chat_id);

        // Check for existing active session
        if let Some(session_id) = self.user_sessions.get(&user_key) {
            if let Some(session) = self.sessions.get(session_id.value()) {
                if session.is_active().await {
                    return Ok(session.value().clone());
                }
            }
        }

        // Create new session
        let session = Arc::new(Session::new(
            user_id.to_string(),
            channel_id.to_string(),
            chat_id.to_string(),
        ));
        let session_id = session.id.clone();

        session.set_state(SessionState::Active).await;

        // Initialize per-session isolation (taint registry + audit log)
        self.pipeline.isolation().init_session(&session.id).await;

        // Store session
        self.sessions.insert(session_id.clone(), session.clone());
        self.user_sessions.insert(user_key, session_id);

        tracing::info!(
            "Created session {} for user {} on {}:{}",
            session.id,
            user_id,
            channel_id,
            chat_id
        );

        Ok(session)
    }

    /// Get session by ID
    pub async fn get_session(&self, session_id: &str) -> Option<Arc<Session>> {
        self.sessions.get(session_id).map(|r| r.value().clone())
    }

    /// Get session for user
    pub async fn get_user_session(
        &self,
        user_id: &str,
        channel_id: &str,
        chat_id: &str,
    ) -> Option<Arc<Session>> {
        let user_key = format!("{}:{}:{}", user_id, channel_id, chat_id);
        let session_id = self.user_sessions.get(&user_key)?.value().clone();
        self.get_session(&session_id).await
    }

    /// Upgrade an existing session to use TEE processing.
    ///
    /// With TeeRuntime (Phase 11), this checks if the runtime detected TEE
    /// hardware and marks the session accordingly. No VM boot needed.
    pub async fn upgrade_to_tee(&self, session_id: &str) -> Result<()> {
        if !self.tee_config.enabled {
            return Err(Error::Tee("TEE is not enabled".to_string()));
        }

        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| Error::Tee(format!("Session {} not found", session_id)))?;

        if session.uses_tee().await {
            return Ok(()); // Already upgraded
        }

        // Check if TEE hardware is actually available
        if !self.tee_runtime.is_tee_active() {
            return Err(Error::Tee(format!(
                "Cannot upgrade to TEE: runtime security level is {} (need TeeHardware)",
                self.tee_runtime.security_level()
            )));
        }

        // Mark session as TEE-active
        session.mark_tee_active().await;

        tracing::info!(
            session_id = session_id,
            level = %self.tee_runtime.security_level(),
            "Upgraded session to TEE"
        );

        Ok(())
    }

    /// Process a message in TEE for the given session.
    ///
    /// Scans input for prompt injection before processing. With TeeRuntime
    /// (Phase 11), the message is processed in the current TEE environment
    /// rather than being forwarded to a separate VM.
    pub async fn process_in_tee(&self, session_id: &str, content: &str) -> Result<String> {
        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| Error::Tee(format!("Session {} not found", session_id)))?;

        // Prompt injection defense: scan input before processing
        let injection_result = self.pipeline.check_injection(content, session_id).await;
        if injection_result.verdict == InjectionVerdict::Blocked {
            return Err(Error::Tee(format!(
                "Prompt injection blocked: {} pattern(s) detected",
                injection_result.matches.len()
            )));
        }

        if !self.tee_runtime.is_tee_active() {
            return Err(Error::Tee(
                "TEE runtime not active — cannot process in TEE".to_string(),
            ));
        }

        session.set_state(SessionState::Processing).await;
        session.touch().await;

        // In Phase 11, processing happens in the current TEE environment.
        // The actual LLM call is handled by AgentEngine (in-process a3s-code).
        // This method validates TEE state and injection safety.
        session.set_state(SessionState::Active).await;

        Ok(content.to_string())
    }

    /// Terminate a session
    pub async fn terminate_session(&self, session_id: &str) -> Result<()> {
        let session = match self.sessions.remove(session_id) {
            Some((_, s)) => s,
            None => return Ok(()),
        };

        session.set_state(SessionState::Terminating).await;

        // Remove from user sessions
        let user_key = format!(
            "{}:{}:{}",
            session.user_id, session.channel_id, session.chat_id
        );
        self.user_sessions.remove(&user_key);

        // Securely wipe session-scoped taint data and audit log
        let wipe = self.pipeline.isolation().wipe_session(session_id).await;
        if !wipe.verified {
            tracing::error!(session_id = session_id, "Session wipe verification failed");
        }

        session.set_state(SessionState::Terminated).await;

        tracing::info!("Terminated session {}", session_id);

        Ok(())
    }

    /// Get all active sessions
    pub async fn active_sessions(&self) -> Vec<Arc<Session>> {
        let mut active = Vec::new();
        for entry in self.sessions.iter() {
            if entry.value().is_active().await {
                active.push(entry.value().clone());
            }
        }
        active
    }

    /// Get session count
    pub async fn session_count(&self) -> usize {
        self.sessions.len()
    }

    /// Clean up inactive sessions
    pub async fn cleanup_inactive(&self, max_idle_ms: i64) -> Result<usize> {
        let now = chrono::Utc::now().timestamp_millis();
        let sessions: Vec<Arc<Session>> =
            { self.sessions.iter().map(|r| r.value().clone()).collect() };

        let mut cleaned = 0;
        for session in sessions {
            let idle_time = now - session.last_activity().await;
            if idle_time > max_idle_ms {
                if let Err(e) = self.terminate_session(&session.id).await {
                    tracing::warn!("Failed to cleanup session {}: {}", session.id, e);
                } else {
                    cleaned += 1;
                }
            }
        }

        if cleaned > 0 {
            tracing::info!("Cleaned up {} inactive sessions", cleaned);
        }

        Ok(cleaned)
    }
}

impl Default for SessionManager {
    fn default() -> Self {
        let global_log = Arc::new(RwLock::new(crate::audit::AuditLog::default()));
        let bus = Arc::new(AuditEventBus::new(256, global_log));
        Self::new(TeeConfig::default(), bus)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::AuditLog;

    /// Create a default bus for tests.
    fn test_bus() -> Arc<AuditEventBus> {
        let log = Arc::new(RwLock::new(AuditLog::default()));
        Arc::new(AuditEventBus::new(256, log))
    }

    /// Create a SessionManager from a TeeConfig with a default test bus.
    fn make_manager(config: TeeConfig) -> SessionManager {
        SessionManager::new(config, test_bus())
    }

    // ---- Session tests ----

    #[tokio::test]
    async fn test_session_creation() {
        let session = Session::new(
            "user-123".to_string(),
            "telegram".to_string(),
            "chat-456".to_string(),
        );

        assert_eq!(session.state().await, SessionState::Creating);
        assert!(!session.uses_tee().await);
    }

    #[tokio::test]
    async fn test_session_state_transitions() {
        let session = Session::new(
            "user-123".to_string(),
            "telegram".to_string(),
            "chat-456".to_string(),
        );

        session.set_state(SessionState::Active).await;
        assert!(session.is_active().await);

        session.set_state(SessionState::Processing).await;
        assert!(session.is_active().await);

        session.set_state(SessionState::Terminated).await;
        assert!(!session.is_active().await);
    }

    #[tokio::test]
    async fn test_session_sensitivity() {
        let session = Session::new(
            "user-123".to_string(),
            "telegram".to_string(),
            "chat-456".to_string(),
        );

        assert_eq!(session.sensitivity_level().await, SensitivityLevel::Normal);

        session
            .update_sensitivity(SensitivityLevel::Sensitive)
            .await;
        assert_eq!(
            session.sensitivity_level().await,
            SensitivityLevel::Sensitive
        );

        // Should not decrease
        session.update_sensitivity(SensitivityLevel::Normal).await;
        assert_eq!(
            session.sensitivity_level().await,
            SensitivityLevel::Sensitive
        );
    }

    #[tokio::test]
    async fn test_session_uses_tee_default_false() {
        let session = Session::new(
            "user-123".to_string(),
            "telegram".to_string(),
            "chat-456".to_string(),
        );
        assert!(!session.uses_tee().await);
    }

    #[tokio::test]
    async fn test_session_mark_tee_active() {
        let session = Session::new(
            "user-123".to_string(),
            "telegram".to_string(),
            "chat-456".to_string(),
        );
        assert!(!session.uses_tee().await);
        session.mark_tee_active().await;
        assert!(session.uses_tee().await);
    }

    // ---- SessionManager tests ----

    #[tokio::test]
    async fn test_manager_create_session() {
        let manager = SessionManager::default();

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();

        assert_eq!(manager.session_count().await, 1);

        // Getting same session should return existing
        let session2 = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();
        assert_eq!(session.id, session2.id);
        assert_eq!(manager.session_count().await, 1);

        // Different chat should create new session
        let session3 = manager
            .create_session("user-123", "telegram", "chat-789")
            .await
            .unwrap();
        assert_ne!(session.id, session3.id);
        assert_eq!(manager.session_count().await, 2);
    }

    #[tokio::test]
    async fn test_manager_terminate_session() {
        let manager = SessionManager::default();

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();
        let session_id = session.id.clone();

        manager.terminate_session(&session_id).await.unwrap();

        assert!(manager.get_session(&session_id).await.is_none());
        assert_eq!(manager.session_count().await, 0);
    }

    #[tokio::test]
    async fn test_manager_tee_disabled_upgrade_fails() {
        let config = TeeConfig {
            enabled: false,
            ..Default::default()
        };
        let manager = make_manager(config);

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();

        let result = manager.upgrade_to_tee(&session.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager_is_tee_enabled() {
        let disabled = make_manager(TeeConfig {
            enabled: false,
            ..Default::default()
        });
        assert!(!disabled.is_tee_enabled());

        let enabled = make_manager(TeeConfig {
            enabled: true,
            ..Default::default()
        });
        assert!(enabled.is_tee_enabled());
    }

    #[tokio::test]
    async fn test_manager_upgrade_nonexistent_session_fails() {
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = make_manager(config);

        let result = manager.upgrade_to_tee("nonexistent").await;
        assert!(result.is_err());
    }

    // ---- Audit bus wiring tests ----

    #[tokio::test]
    async fn test_sanitize_output_publishes_events() {
        let global_log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = Arc::new(AuditEventBus::new(256, global_log.clone()));
        let manager = SessionManager::new(TeeConfig::default(), bus);

        let session = manager
            .create_session("user-1", "test", "chat-1")
            .await
            .unwrap();

        // Register tainted data in the session's registry
        if let Some(guard) = manager.isolation().registry(&session.id).await {
            guard
                .write(|registry| {
                    registry.register("sk-secret-key-123", crate::guard::TaintType::ApiKey);
                })
                .await;
        }

        // Sanitize output containing the tainted data
        let result = manager
            .sanitize_output(&session.id, "The key is sk-secret-key-123")
            .await;
        assert!(result.was_redacted);

        // Events should be in the global log
        let log = global_log.read().await;
        assert!(log.len() > 0);
    }

    #[tokio::test]
    async fn test_intercept_tool_call_publishes_events() {
        let global_log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = Arc::new(AuditEventBus::new(256, global_log.clone()));
        let manager = SessionManager::new(TeeConfig::default(), bus);

        let session = manager
            .create_session("user-1", "test", "chat-1")
            .await
            .unwrap();

        // Register tainted data
        if let Some(guard) = manager.isolation().registry(&session.id).await {
            guard
                .write(|registry| {
                    registry.register("my-secret-password", crate::guard::TaintType::Password);
                })
                .await;
        }

        let result = manager
            .intercept_tool_call(&session.id, "bash", "echo my-secret-password")
            .await;
        // Should detect both tainted data and dangerous command
        assert!(!result.audit_events.is_empty());

        let log = global_log.read().await;
        assert!(log.len() > 0);
    }

    #[tokio::test]
    async fn test_check_firewall_publishes_blocked_event() {
        let global_log = Arc::new(RwLock::new(AuditLog::new(100)));
        let bus = Arc::new(AuditEventBus::new(256, global_log.clone()));
        let mut tee_config = TeeConfig::default();
        // Ensure default-deny firewall is active
        tee_config.network_policy.enabled = true;
        tee_config.network_policy.default_deny = true;
        let manager = SessionManager::new(tee_config, bus);

        let session = manager
            .create_session("user-1", "test", "chat-1")
            .await
            .unwrap();

        // Check a URL that's not in the whitelist
        let result = manager
            .check_firewall("https://evil-exfil.example.com/steal", &session.id)
            .await;

        // Should be blocked
        assert_ne!(result.decision, crate::guard::FirewallDecision::Allow);

        // If blocked, an audit event should have been published
        if result.audit_event.is_some() {
            let log = global_log.read().await;
            assert!(log.len() > 0);
        }
    }

    // ---- TeeRuntime integration tests ----

    #[tokio::test]
    async fn test_manager_with_runtime() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::ProcessOnly));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime.clone(), test_bus());

        assert!(manager.is_tee_enabled());
        assert_eq!(
            manager.tee_runtime().security_level(),
            SecurityLevel::ProcessOnly
        );
    }

    #[tokio::test]
    async fn test_manager_upgrade_fails_without_tee_hardware() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        // ProcessOnly runtime — upgrade should fail
        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::ProcessOnly));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime, test_bus());

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();

        let result = manager.upgrade_to_tee(&session.id).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("need TeeHardware"));
    }

    #[tokio::test]
    async fn test_manager_process_in_tee_fails_without_hardware() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::ProcessOnly));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime, test_bus());

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();

        let result = manager.process_in_tee(&session.id, "hello from TEE").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("not active"));
    }

    #[tokio::test]
    async fn test_manager_init_tee_logs_level() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::VmIsolation));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime, test_bus());

        // Should succeed without error (just logs)
        manager.init_tee().await.unwrap();
    }

    #[tokio::test]
    async fn test_manager_shutdown_tee() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::ProcessOnly));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime, test_bus());

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();
        assert!(session.is_active().await);

        manager.shutdown_tee().await.unwrap();

        // Session should be marked as terminating
        assert_eq!(session.state().await, SessionState::Terminating);
    }

    #[tokio::test]
    async fn test_manager_terminate_session_wipes_isolation() {
        let manager = SessionManager::default();

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();
        let session_id = session.id.clone();

        // Register some tainted data
        if let Some(guard) = manager.isolation().registry(&session_id).await {
            guard
                .write(|registry| {
                    registry.register("secret-data", crate::guard::TaintType::ApiKey);
                })
                .await;
        }

        manager.terminate_session(&session_id).await.unwrap();
        assert!(manager.get_session(&session_id).await.is_none());
    }
}
