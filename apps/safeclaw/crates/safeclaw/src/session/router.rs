//! Session routing based on privacy classification
//!
//! `SessionRouter` routes inbound messages to the appropriate session and
//! decides whether TEE processing is required.
//!
//! **Classification pipeline in `route()`:**
//! ```text
//! message.content
//!   → CompositeClassifier (RegexBackend + SemanticBackend + optional LlmBackend)
//!   → ClassificationResult
//!   → PolicyEngine → RoutingDecision
//! ```
//!
//! The composite classifier is used for routing decisions (security-critical path)
//! because regex alone misses semantic PII ("my password is hunter2", prose addresses).
//! The regex-only `Classifier` is retained for fast, synchronous redaction.

use crate::channels::InboundMessage;
use crate::error::Result;
use crate::privacy::{
    ClassificationResult, Classifier, CompositeClassifier, CumulativeRiskDecision, Match,
    PolicyDecision, PolicyEngine,
};
use crate::session::SessionManager;
use std::sync::Arc;

/// Number of distinct PII types in a session before a confirmation warning is issued.
const CUMULATIVE_WARN_THRESHOLD: usize = 3;
/// Number of distinct PII types in a session before the session is rejected.
const CUMULATIVE_REJECT_THRESHOLD: usize = 5;

/// Routing decision for a message
#[derive(Debug, Clone)]
pub struct RoutingDecision {
    /// Session to route to
    pub session_id: String,
    /// Whether to process in TEE
    pub use_tee: bool,
    /// Classification result (from the full composite pipeline)
    pub classification: ClassificationResult,
    /// Policy decision
    pub policy_decision: PolicyDecision,
    /// Cumulative PII risk for the session (split-message attack defense)
    pub cumulative_decision: CumulativeRiskDecision,
}

/// Session router that handles message routing based on privacy classification.
pub struct SessionRouter {
    session_manager: Arc<SessionManager>,
    /// Regex-only classifier — used for synchronous redaction helpers only.
    classifier: Arc<Classifier>,
    /// Full classifier chain (regex + semantic + optional LLM).
    /// Used in the routing critical path where recall matters most.
    composite: Arc<CompositeClassifier>,
    policy_engine: Arc<PolicyEngine>,
}

impl SessionRouter {
    /// Create a new session router.
    ///
    /// - `classifier`: regex-only, for sync redaction
    /// - `composite`: full chain, for async routing decisions
    pub fn new(
        session_manager: Arc<SessionManager>,
        classifier: Arc<Classifier>,
        composite: Arc<CompositeClassifier>,
        policy_engine: Arc<PolicyEngine>,
    ) -> Self {
        Self {
            session_manager,
            classifier,
            composite,
            policy_engine,
        }
    }

    /// Route an inbound message through the full classification pipeline.
    ///
    /// Uses `CompositeClassifier` (regex + semantic) so semantic PII
    /// ("my password is hunter2", prose addresses) is caught in addition
    /// to structured PII patterns.
    pub async fn route(&self, message: &InboundMessage) -> Result<RoutingDecision> {
        // --- Full composite classification ---
        let composite_result = self.composite.classify(&message.content).await;
        let classification = composite_to_classification(&message.content, composite_result);

        // --- Policy ---
        let policy_decision = self
            .policy_engine
            .evaluate(classification.level, None, None);

        let use_tee = matches!(policy_decision, PolicyDecision::ProcessInTee)
            && self.session_manager.is_tee_enabled();

        // --- Session management ---
        let session = self
            .session_manager
            .get_user_session(&message.sender_id, &message.channel, &message.chat_id)
            .await;

        let session = match session {
            Some(s) => s,
            None => {
                self.session_manager
                    .create_session(&message.sender_id, &message.channel, &message.chat_id)
                    .await?
            }
        };

        session.update_sensitivity(classification.level).await;

        // --- Cumulative privacy gate (split-message attack defense) ---
        let rule_names: Vec<String> = classification
            .matches
            .iter()
            .map(|m| m.rule_name.clone())
            .collect();
        session
            .record_disclosures(&rule_names, classification.level)
            .await;
        let cumulative_decision = session
            .assess_privacy_risk(CUMULATIVE_WARN_THRESHOLD, CUMULATIVE_REJECT_THRESHOLD)
            .await;

        if cumulative_decision == CumulativeRiskDecision::RequireConfirmation {
            tracing::warn!(
                session = %session.id,
                "Cumulative PII disclosure threshold reached: {} distinct PII types detected",
                CUMULATIVE_WARN_THRESHOLD
            );
        }

        if use_tee && !session.uses_tee().await {
            if let Err(e) = self.session_manager.upgrade_to_tee(&session.id).await {
                tracing::warn!(
                    session = %session.id,
                    "TEE upgrade failed (will be handled by processor): {}",
                    e
                );
            }
        }

        session.touch().await;
        session.increment_messages().await;

        Ok(RoutingDecision {
            session_id: session.id.clone(),
            use_tee,
            classification,
            policy_decision,
            cumulative_decision,
        })
    }

    /// Quick sync check: does this message require TEE?
    ///
    /// Uses the regex-only classifier for speed — suitable for pre-checks
    /// where a false-negative is acceptable (full check happens in `route()`).
    pub fn requires_tee(&self, message: &InboundMessage) -> bool {
        let classification = self.classifier.classify(&message.content);
        self.policy_engine.requires_tee(classification.level)
            && self.session_manager.is_tee_enabled()
    }

    /// Redact sensitive data from content using the regex classifier.
    pub fn redact_content(&self, content: &str) -> String {
        self.classifier.redact(content)
    }

    /// Classify content with the regex-only classifier (sync, for display/logging).
    ///
    /// For routing decisions use `route()` which runs the full composite chain.
    pub fn classify(&self, content: &str) -> ClassificationResult {
        self.classifier.classify(content)
    }
}

// ---------------------------------------------------------------------------
// Conversion: CompositeResult → ClassificationResult
// ---------------------------------------------------------------------------

/// Convert a `CompositeResult` into the `ClassificationResult` used by
/// `RoutingDecision`, redacting each match span from the original text.
fn composite_to_classification(
    text: &str,
    result: crate::privacy::CompositeResult,
) -> ClassificationResult {
    let matches = result
        .matches
        .into_iter()
        .map(|m| {
            let slice = text.get(m.start..m.end).unwrap_or("");
            let redacted = a3s_common::privacy::redact_text(
                slice,
                &m.rule_name,
                a3s_common::privacy::RedactionStrategy::Mask,
            );
            Match {
                rule_name: m.rule_name,
                level: m.level,
                start: m.start,
                end: m.end,
                redacted,
            }
        })
        .collect();

    ClassificationResult {
        level: result.level,
        matches,
        requires_tee: result.requires_tee,
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{PrivacyConfig, SensitivityLevel, TeeConfig};
    use crate::privacy::{CompositeClassifier, RegexBackend, SemanticAnalyzer, SemanticBackend};

    fn create_test_router() -> SessionRouter {
        let tee_config = TeeConfig {
            enabled: false,
            ..Default::default()
        };
        let global_log = Arc::new(tokio::sync::RwLock::new(crate::audit::AuditLog::default()));
        let audit_bus = Arc::new(crate::audit::AuditEventBus::new(256, global_log));
        let session_manager = Arc::new(SessionManager::new(tee_config, audit_bus));

        let privacy_config = PrivacyConfig::default();

        let classifier = Arc::new(
            Classifier::new(privacy_config.rules.clone(), privacy_config.default_level).unwrap(),
        );

        let regex_backend =
            RegexBackend::new(privacy_config.rules.clone(), privacy_config.default_level).unwrap();
        let semantic_backend = SemanticBackend::new(SemanticAnalyzer::new());
        let composite = Arc::new(CompositeClassifier::new(vec![
            Box::new(regex_backend),
            Box::new(semantic_backend),
        ]));

        let policy_engine = Arc::new(PolicyEngine::new());

        SessionRouter::new(session_manager, classifier, composite, policy_engine)
    }

    #[tokio::test]
    async fn test_route_normal_message() {
        let router = create_test_router();
        let message =
            InboundMessage::new("telegram", "user-123", "chat-456", "Hello, how are you?");

        let decision = router.route(&message).await.unwrap();

        assert!(!decision.use_tee);
        assert_eq!(decision.classification.level, SensitivityLevel::Normal);
    }

    #[tokio::test]
    async fn test_route_sensitive_message() {
        let router = create_test_router();
        let message = InboundMessage::new(
            "telegram",
            "user-123",
            "chat-456",
            "My credit card is 4111-1111-1111-1111",
        );

        let decision = router.route(&message).await.unwrap();

        assert!(!decision.use_tee); // TEE disabled in tests
        assert_eq!(
            decision.classification.level,
            SensitivityLevel::HighlySensitive
        );
    }

    #[tokio::test]
    async fn test_route_semantic_pii_detected() {
        let router = create_test_router();
        // Semantic PII — regex wouldn't catch this
        let message =
            InboundMessage::new("telegram", "user-123", "chat-456", "my password is hunter2");

        let decision = router.route(&message).await.unwrap();

        // SemanticBackend catches "my password is ..."
        assert!(!decision.classification.matches.is_empty());
        assert!(decision.classification.requires_tee);
    }

    #[test]
    fn test_requires_tee_sync() {
        let router = create_test_router();

        let normal = InboundMessage::new("telegram", "user-123", "chat-456", "Hello!");
        assert!(!router.requires_tee(&normal));

        // TEE disabled in test → always false
        let sensitive =
            InboundMessage::new("telegram", "user-123", "chat-456", "My SSN is 123-45-6789");
        assert!(!router.requires_tee(&sensitive));
    }

    #[test]
    fn test_redact_content() {
        let router = create_test_router();
        let content = "My email is test@example.com and SSN is 123-45-6789";
        let redacted = router.redact_content(content);

        assert!(!redacted.contains("test@example.com"));
        assert!(!redacted.contains("123-45-6789"));
        assert!(redacted.contains("****@example.com"));
        assert!(redacted.contains("***-**-****"));
    }

    #[tokio::test]
    async fn test_cumulative_clean_messages_stay_allow() {
        let router = create_test_router();
        // Clean messages from the same user should never escalate.
        for text in &[
            "Hello!",
            "What is the weather?",
            "Tell me a joke",
            "How are you?",
        ] {
            let msg = InboundMessage::new("telegram", "user-clean", "chat-clean", text);
            let decision = router.route(&msg).await.unwrap();
            assert_eq!(decision.cumulative_decision, CumulativeRiskDecision::Allow);
        }
    }

    #[tokio::test]
    async fn test_cumulative_pii_escalates_to_warn() {
        let router = create_test_router();
        let make_msg = |text: &str| InboundMessage::new("telegram", "u-split", "chat-split", text);

        // Each message contributes a distinct PII type.
        router
            .route(&make_msg("My email is attacker@evil.com"))
            .await
            .unwrap();
        router
            .route(&make_msg("My SSN is 123-45-6789"))
            .await
            .unwrap();
        let d = router
            .route(&make_msg("My credit card is 4111-1111-1111-1111"))
            .await
            .unwrap();

        // 3 distinct PII types hits the warn threshold.
        assert_ne!(d.cumulative_decision, CumulativeRiskDecision::Allow);
    }

    #[tokio::test]
    async fn test_cumulative_reject_via_session() {
        // Drive cumulative context directly through Session to confirm reject path
        // without depending on the live classifier hitting all 5 rule names.
        use crate::audit::AuditEventBus;
        use crate::config::TeeConfig;

        let global_log = Arc::new(tokio::sync::RwLock::new(crate::audit::AuditLog::default()));
        let audit_bus = Arc::new(AuditEventBus::new(256, global_log));
        let manager = Arc::new(SessionManager::new(TeeConfig::default(), audit_bus));

        let session = manager
            .create_session("u-rej", "telegram", "c-rej")
            .await
            .unwrap();

        // Inject 5 distinct PII types directly.
        for rule in &["email", "phone", "ssn", "credit_card", "api_key"] {
            session
                .record_disclosures(&[rule.to_string()], SensitivityLevel::Sensitive)
                .await;
        }

        assert_eq!(
            session.assess_privacy_risk(3, 5).await,
            CumulativeRiskDecision::Reject
        );
    }

    #[test]
    fn test_composite_to_classification_redacts_span() {
        let text = "My SSN is 123-45-6789 and email test@example.com";
        let matches = vec![crate::privacy::PiiMatch {
            rule_name: "ssn".to_string(),
            level: SensitivityLevel::HighlySensitive,
            start: 10,
            end: 21,
            confidence: 0.95,
            backend: "regex".to_string(),
        }];
        let result = crate::privacy::CompositeResult {
            level: SensitivityLevel::HighlySensitive,
            matches,
            requires_tee: true,
        };
        let classification = composite_to_classification(text, result);

        assert_eq!(classification.level, SensitivityLevel::HighlySensitive);
        assert_eq!(classification.matches.len(), 1);
        assert_eq!(classification.matches[0].rule_name, "ssn");
        assert!(!classification.matches[0].redacted.contains("123-45-6789"));
    }
}
