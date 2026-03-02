//! Unified session management
//!
//! Provides a single `Session` type that optionally supports TEE processing,
//! and a `SessionManager` that handles both regular and TEE session lifecycles.

use crate::channels::InboundMessage;
use crate::config::{SensitivityLevel, TeeConfig};
use crate::error::{Error, Result};
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

/// Routing decision for a message
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Session to route to
    pub session_id: String,
    /// Whether to process in TEE
    pub use_tee: bool,
    /// Sensitivity level
    pub sensitivity: SensitivityLevel,
}

/// Unified session manager handling both regular and TEE sessions.
pub struct SessionManager {
    /// Active sessions indexed by session ID
    sessions: Arc<DashMap<String, Arc<Session>>>,
    /// Sessions indexed by user_id:channel_id:chat_id
    user_sessions: Arc<DashMap<String, String>>,
    /// TEE configuration
    tee_config: TeeConfig,
    /// TEE runtime for self-detection and sealed storage
    tee_runtime: Arc<TeeRuntime>,
}

impl SessionManager {
    /// Create a new session manager with TEE configuration.
    pub fn new(tee_config: TeeConfig) -> Self {
        let tee_runtime = Arc::new(TeeRuntime::process_only());
        Self {
            sessions: Arc::new(DashMap::new()),
            user_sessions: Arc::new(DashMap::new()),
            tee_config,
            tee_runtime,
        }
    }

    /// Create a new session manager with a pre-detected TEE runtime.
    pub fn with_runtime(tee_config: TeeConfig, tee_runtime: Arc<TeeRuntime>) -> Self {
        Self {
            sessions: Arc::new(DashMap::new()),
            user_sessions: Arc::new(DashMap::new()),
            tee_config,
            tee_runtime,
        }
    }

    /// Initialize the TEE subsystem.
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

        let sessions: Vec<Arc<Session>> =
            { self.sessions.iter().map(|r| r.value().clone()).collect() };

        for session in sessions {
            if session.is_active().await {
                session.set_state(SessionState::Terminating).await;
            }
        }

        self.tee_runtime.shutdown().await?;

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

        let session = Arc::new(Session::new(
            user_id.to_string(),
            channel_id.to_string(),
            chat_id.to_string(),
        ));
        let session_id = session.id.clone();

        session.set_state(SessionState::Active).await;

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

    /// Route an inbound message to a session, creating one if needed.
    pub async fn route_message(&self, message: &InboundMessage) -> Result<RoutingDecision> {
        let session = match self
            .get_user_session(&message.sender_id, &message.channel, &message.chat_id)
            .await
        {
            Some(s) => s,
            None => {
                self.create_session(&message.sender_id, &message.channel, &message.chat_id)
                    .await?
            }
        };

        session.touch().await;
        session.increment_messages().await;

        Ok(RoutingDecision {
            session_id: session.id.clone(),
            use_tee: session.uses_tee().await,
            sensitivity: session.sensitivity_level().await,
        })
    }

    /// Upgrade an existing session to use TEE processing.
    pub async fn upgrade_to_tee(&self, session_id: &str) -> Result<()> {
        if !self.tee_config.enabled {
            return Err(Error::Tee("TEE is not enabled".to_string()));
        }

        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| Error::Tee(format!("Session {} not found", session_id)))?;

        if session.uses_tee().await {
            return Ok(());
        }

        if !self.tee_runtime.is_tee_active() {
            return Err(Error::Tee(format!(
                "Cannot upgrade to TEE: runtime security level is {} (need TeeHardware)",
                self.tee_runtime.security_level()
            )));
        }

        session.mark_tee_active().await;

        tracing::info!(
            session_id = session_id,
            level = %self.tee_runtime.security_level(),
            "Upgraded session to TEE"
        );

        Ok(())
    }

    /// Process a message in TEE for the given session.
    pub async fn process_in_tee(&self, session_id: &str, content: &str) -> Result<String> {
        let session = self
            .get_session(session_id)
            .await
            .ok_or_else(|| Error::Tee(format!("Session {} not found", session_id)))?;

        if !self.tee_runtime.is_tee_active() {
            return Err(Error::Tee(
                "TEE runtime not active — cannot process in TEE".to_string(),
            ));
        }

        session.set_state(SessionState::Processing).await;
        session.touch().await;

        // In Phase 11, processing happens in the current TEE environment.
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

        let user_key = format!(
            "{}:{}:{}",
            session.user_id, session.channel_id, session.chat_id
        );
        self.user_sessions.remove(&user_key);

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
        Self::new(TeeConfig::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let manager = SessionManager::new(config);

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();

        let result = manager.upgrade_to_tee(&session.id).await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_manager_is_tee_enabled() {
        let disabled = SessionManager::new(TeeConfig {
            enabled: false,
            ..Default::default()
        });
        assert!(!disabled.is_tee_enabled());

        let enabled = SessionManager::new(TeeConfig {
            enabled: true,
            ..Default::default()
        });
        assert!(enabled.is_tee_enabled());
    }

    #[tokio::test]
    async fn test_manager_with_runtime() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::ProcessOnly));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime.clone());

        assert!(manager.is_tee_enabled());
        assert_eq!(
            manager.tee_runtime().security_level(),
            SecurityLevel::ProcessOnly
        );
    }

    #[tokio::test]
    async fn test_manager_shutdown_tee() {
        use crate::tee::{SecurityLevel, TeeRuntime};

        let runtime = Arc::new(TeeRuntime::with_level(SecurityLevel::ProcessOnly));
        let config = TeeConfig {
            enabled: true,
            ..Default::default()
        };
        let manager = SessionManager::with_runtime(config, runtime);

        let session = manager
            .create_session("user-123", "telegram", "chat-456")
            .await
            .unwrap();
        assert!(session.is_active().await);

        manager.shutdown_tee().await.unwrap();

        assert_eq!(session.state().await, SessionState::Terminating);
    }

    #[tokio::test]
    async fn test_route_message() {
        let manager = SessionManager::default();
        let message =
            InboundMessage::new("telegram", "user-123", "chat-456", "Hello, how are you?");

        let decision = manager.route_message(&message).await.unwrap();

        assert!(!decision.use_tee);
        assert_eq!(decision.sensitivity, SensitivityLevel::Normal);
    }

    #[tokio::test]
    async fn test_route_message_reuses_session() {
        let manager = SessionManager::default();
        let message = InboundMessage::new("telegram", "user-123", "chat-456", "Hello!");

        let d1 = manager.route_message(&message).await.unwrap();
        let d2 = manager.route_message(&message).await.unwrap();

        assert_eq!(d1.session_id, d2.session_id);
    }
}
