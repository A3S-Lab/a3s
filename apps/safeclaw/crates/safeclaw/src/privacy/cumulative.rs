//! Cumulative privacy context for per-session leakage tracking (15.3)
//!
//! Tracks PII disclosures across multiple messages within a session.
//! Prevents split-message attacks where an attacker leaks PII incrementally
//! ("I live in..." + "...Chaoyang District" + "...Wangjing SOHO").
//!
//! **Threat model**: Defends against A1 (malicious user) at AS-2/AS-4.
//! See `docs/threat-model.md` §4 AS-2, §5.

use crate::config::SensitivityLevel;
use std::collections::HashSet;

/// Types of PII that can be disclosed in a session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PiiType {
    Email,
    Phone,
    CreditCard,
    Ssn,
    Address,
    Name,
    DateOfBirth,
    Password,
    ApiKey,
    BankAccount,
    Medical,
    Other,
}

impl PiiType {
    /// Map a rule name (from classifier) to a PII type.
    pub fn from_rule_name(rule: &str) -> Self {
        let lower = rule.to_lowercase();
        if lower.contains("email") {
            Self::Email
        } else if lower.contains("phone") {
            Self::Phone
        } else if lower.contains("credit_card") || lower.contains("creditcard") {
            Self::CreditCard
        } else if lower.contains("ssn") || lower.contains("social_security") {
            Self::Ssn
        } else if lower.contains("address") {
            Self::Address
        } else if lower.contains("name") {
            Self::Name
        } else if lower.contains("dob")
            || lower.contains("date_of_birth")
            || lower.contains("dateofbirth")
        {
            Self::DateOfBirth
        } else if lower.contains("password") {
            Self::Password
        } else if lower.contains("api_key") || lower.contains("apikey") || lower.contains("token") {
            Self::ApiKey
        } else if lower.contains("bank") || lower.contains("routing") {
            Self::BankAccount
        } else if lower.contains("medical") || lower.contains("health") {
            Self::Medical
        } else {
            Self::Other
        }
    }
}

/// Decision from cumulative risk assessment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CumulativeRiskDecision {
    /// Risk is within acceptable limits, proceed normally.
    Allow,
    /// Cumulative risk is elevated, require user confirmation before proceeding.
    RequireConfirmation,
    /// Cumulative risk exceeds threshold, reject the message.
    Reject,
}

/// Per-session cumulative privacy context.
///
/// Tracks which PII types have been disclosed across all messages in a session.
/// When the number of distinct PII types exceeds a configurable threshold,
/// the gate escalates the routing decision.
#[derive(Debug, Clone)]
pub struct SessionPrivacyContext {
    /// Distinct PII types disclosed in this session
    disclosed_types: HashSet<PiiType>,
    /// Highest sensitivity level seen in this session
    max_sensitivity: SensitivityLevel,
    /// Total number of PII matches across all messages
    total_matches: usize,
    /// Number of messages processed
    message_count: usize,
}

impl Default for SessionPrivacyContext {
    fn default() -> Self {
        Self {
            disclosed_types: HashSet::new(),
            max_sensitivity: SensitivityLevel::Normal,
            total_matches: 0,
            message_count: 0,
        }
    }
}

impl SessionPrivacyContext {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self::default()
    }

    /// Record PII disclosures from a classified message.
    ///
    /// Call this after each message is classified, passing the rule names
    /// and sensitivity level from the classification result.
    pub fn record_disclosures(&mut self, rule_names: &[String], sensitivity: SensitivityLevel) {
        self.message_count += 1;
        self.total_matches += rule_names.len();

        if sensitivity > self.max_sensitivity {
            self.max_sensitivity = sensitivity;
        }

        for rule in rule_names {
            let pii_type = PiiType::from_rule_name(rule);
            self.disclosed_types.insert(pii_type);
        }
    }

    /// Assess cumulative risk given the configured threshold.
    ///
    /// - If distinct PII types exceed `reject_threshold`, return `Reject`.
    /// - If distinct PII types exceed `warn_threshold`, return `RequireConfirmation`.
    /// - Otherwise, return `Allow`.
    pub fn assess_risk(
        &self,
        warn_threshold: usize,
        reject_threshold: usize,
    ) -> CumulativeRiskDecision {
        let count = self.disclosed_types.len();

        if count >= reject_threshold {
            CumulativeRiskDecision::Reject
        } else if count >= warn_threshold {
            CumulativeRiskDecision::RequireConfirmation
        } else {
            CumulativeRiskDecision::Allow
        }
    }

    /// Get the number of distinct PII types disclosed so far.
    pub fn distinct_pii_count(&self) -> usize {
        self.disclosed_types.len()
    }

    /// Get the set of disclosed PII types.
    pub fn disclosed_types(&self) -> &HashSet<PiiType> {
        &self.disclosed_types
    }

    /// Get the highest sensitivity level seen in this session.
    pub fn max_sensitivity(&self) -> SensitivityLevel {
        self.max_sensitivity
    }

    /// Get the total number of PII matches across all messages.
    pub fn total_matches(&self) -> usize {
        self.total_matches
    }

    /// Get the number of messages processed.
    pub fn message_count(&self) -> usize {
        self.message_count
    }

    /// Reset the context (e.g., on explicit user action or session expiry).
    pub fn reset(&mut self) {
        self.disclosed_types.clear();
        self.max_sensitivity = SensitivityLevel::Normal;
        self.total_matches = 0;
        self.message_count = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_context() {
        let ctx = SessionPrivacyContext::new();
        assert_eq!(ctx.distinct_pii_count(), 0);
        assert_eq!(ctx.max_sensitivity(), SensitivityLevel::Normal);
        assert_eq!(ctx.total_matches(), 0);
        assert_eq!(ctx.message_count(), 0);
    }

    #[test]
    fn test_record_single_disclosure() {
        let mut ctx = SessionPrivacyContext::new();
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);

        assert_eq!(ctx.distinct_pii_count(), 1);
        assert!(ctx.disclosed_types().contains(&PiiType::Email));
        assert_eq!(ctx.max_sensitivity(), SensitivityLevel::Sensitive);
        assert_eq!(ctx.total_matches(), 1);
        assert_eq!(ctx.message_count(), 1);
    }

    #[test]
    fn test_cumulative_disclosures() {
        let mut ctx = SessionPrivacyContext::new();

        // Message 1: email
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);

        // Message 2: phone
        ctx.record_disclosures(&["phone".to_string()], SensitivityLevel::Sensitive);

        // Message 3: address
        ctx.record_disclosures(&["address".to_string()], SensitivityLevel::Sensitive);

        assert_eq!(ctx.distinct_pii_count(), 3);
        assert_eq!(ctx.total_matches(), 3);
        assert_eq!(ctx.message_count(), 3);
    }

    #[test]
    fn test_duplicate_type_not_counted_twice() {
        let mut ctx = SessionPrivacyContext::new();

        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);

        // Same PII type, only counted once
        assert_eq!(ctx.distinct_pii_count(), 1);
        // But total matches is 2
        assert_eq!(ctx.total_matches(), 2);
    }

    #[test]
    fn test_max_sensitivity_tracks_highest() {
        let mut ctx = SessionPrivacyContext::new();

        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(
            &["credit_card".to_string()],
            SensitivityLevel::HighlySensitive,
        );
        ctx.record_disclosures(&["phone".to_string()], SensitivityLevel::Sensitive);

        assert_eq!(ctx.max_sensitivity(), SensitivityLevel::HighlySensitive);
    }

    #[test]
    fn test_assess_risk_allow() {
        let mut ctx = SessionPrivacyContext::new();
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);

        assert_eq!(ctx.assess_risk(3, 5), CumulativeRiskDecision::Allow);
    }

    #[test]
    fn test_assess_risk_warn() {
        let mut ctx = SessionPrivacyContext::new();
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(&["phone".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(&["address".to_string()], SensitivityLevel::Sensitive);

        // 3 types >= warn_threshold(3), < reject_threshold(5)
        assert_eq!(
            ctx.assess_risk(3, 5),
            CumulativeRiskDecision::RequireConfirmation
        );
    }

    #[test]
    fn test_assess_risk_reject() {
        let mut ctx = SessionPrivacyContext::new();
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(&["phone".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(&["address".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(
            &["credit_card".to_string()],
            SensitivityLevel::HighlySensitive,
        );
        ctx.record_disclosures(&["ssn".to_string()], SensitivityLevel::HighlySensitive);

        // 5 types >= reject_threshold(5)
        assert_eq!(ctx.assess_risk(3, 5), CumulativeRiskDecision::Reject);
    }

    #[test]
    fn test_reset() {
        let mut ctx = SessionPrivacyContext::new();
        ctx.record_disclosures(&["email".to_string()], SensitivityLevel::Sensitive);
        ctx.record_disclosures(&["phone".to_string()], SensitivityLevel::Sensitive);

        ctx.reset();

        assert_eq!(ctx.distinct_pii_count(), 0);
        assert_eq!(ctx.max_sensitivity(), SensitivityLevel::Normal);
        assert_eq!(ctx.total_matches(), 0);
        assert_eq!(ctx.message_count(), 0);
    }

    #[test]
    fn test_pii_type_from_rule_name() {
        assert_eq!(PiiType::from_rule_name("email"), PiiType::Email);
        assert_eq!(PiiType::from_rule_name("credit_card"), PiiType::CreditCard);
        assert_eq!(PiiType::from_rule_name("ssn"), PiiType::Ssn);
        assert_eq!(PiiType::from_rule_name("phone_number"), PiiType::Phone);
        assert_eq!(PiiType::from_rule_name("password"), PiiType::Password);
        assert_eq!(PiiType::from_rule_name("api_key"), PiiType::ApiKey);
        assert_eq!(PiiType::from_rule_name("unknown_rule"), PiiType::Other);
    }

    #[test]
    fn test_split_message_attack_detection() {
        let mut ctx = SessionPrivacyContext::new();

        // Attacker splits PII across messages
        ctx.record_disclosures(&["name".to_string()], SensitivityLevel::Sensitive);
        assert_eq!(ctx.assess_risk(3, 5), CumulativeRiskDecision::Allow);

        ctx.record_disclosures(&["phone".to_string()], SensitivityLevel::Sensitive);
        assert_eq!(ctx.assess_risk(3, 5), CumulativeRiskDecision::Allow);

        ctx.record_disclosures(&["address".to_string()], SensitivityLevel::Sensitive);
        // Now 3 distinct types — escalate!
        assert_eq!(
            ctx.assess_risk(3, 5),
            CumulativeRiskDecision::RequireConfirmation
        );
    }
}
