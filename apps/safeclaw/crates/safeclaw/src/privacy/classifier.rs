//! Privacy classifier for detecting sensitive data
//!
//! Thin wrapper around `a3s_common::privacy::RegexClassifier` that preserves the
//! existing safeclaw API (field names, method signatures) while delegating
//! to the shared implementation.
//!
//! **Threat model**: Defends against A1 (malicious user) at AS-2 (PII classification).
//! See `docs/threat-model.md` §4 AS-2, §5.
//! **Known gap**: Regex-only misses semantic PII — addressed by 15.2 (pluggable classifier).

use crate::config::{ClassificationRule, SensitivityLevel};
use crate::error::{Error, Result};

/// Classification result for a piece of data
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    /// Overall sensitivity level
    pub level: SensitivityLevel,
    /// Individual matches found
    pub matches: Vec<Match>,
    /// Whether TEE processing is required
    pub requires_tee: bool,
}

/// A single match found during classification
#[derive(Debug, Clone)]
pub struct Match {
    /// Rule name that matched
    pub rule_name: String,
    /// Sensitivity level of the match
    pub level: SensitivityLevel,
    /// Start position in the text
    pub start: usize,
    /// End position in the text
    pub end: usize,
    /// The matched text (redacted for display)
    pub redacted: String,
}

/// Privacy classifier for detecting sensitive data.
///
/// Wraps `a3s_common::privacy::RegexClassifier` with the safeclaw-specific API.
pub struct Classifier {
    inner: a3s_common::privacy::RegexClassifier,
}

impl Classifier {
    /// Create a new classifier with the given rules
    pub fn new(rules: Vec<ClassificationRule>, default_level: SensitivityLevel) -> Result<Self> {
        let inner =
            a3s_common::privacy::RegexClassifier::new(&rules, default_level).map_err(|e| {
                Error::Privacy(format!("Failed to compile classification rules: {}", e))
            })?;
        Ok(Self { inner })
    }

    /// Classify a piece of text
    pub fn classify(&self, text: &str) -> ClassificationResult {
        let result = self.inner.classify(text);

        let matches = result
            .matches
            .into_iter()
            .map(|m| {
                let redacted = a3s_common::privacy::redact_text(
                    &m.matched_text,
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
            level: result.overall_level,
            matches,
            requires_tee: result.requires_tee,
        }
    }

    /// Redact sensitive data in text
    pub fn redact(&self, text: &str) -> String {
        self.inner
            .redact(text, a3s_common::privacy::RedactionStrategy::Mask)
    }

    /// Check if text contains any sensitive data
    pub fn contains_sensitive(&self, text: &str) -> bool {
        self.inner.contains_sensitive(text)
    }

    /// Get the highest sensitivity level in text
    pub fn get_sensitivity_level(&self, text: &str) -> SensitivityLevel {
        self.inner.get_sensitivity_level(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_classification_rules;

    fn create_test_classifier() -> Classifier {
        Classifier::new(default_classification_rules(), SensitivityLevel::Normal).unwrap()
    }

    #[test]
    fn test_classify_credit_card() {
        let classifier = create_test_classifier();
        let text = "My card number is 4111-1111-1111-1111";

        let result = classifier.classify(text);
        assert_eq!(result.level, SensitivityLevel::HighlySensitive);
        assert!(result.requires_tee);
        assert!(!result.matches.is_empty());
        assert_eq!(result.matches[0].rule_name, "credit_card");
    }

    #[test]
    fn test_classify_email() {
        let classifier = create_test_classifier();
        let text = "Contact me at test@example.com";

        let result = classifier.classify(text);
        assert_eq!(result.level, SensitivityLevel::Sensitive);
        assert!(result.requires_tee);
    }

    #[test]
    fn test_classify_normal_text() {
        let classifier = create_test_classifier();
        let text = "Hello, how are you today?";

        let result = classifier.classify(text);
        assert_eq!(result.level, SensitivityLevel::Normal);
        assert!(!result.requires_tee);
        assert!(result.matches.is_empty());
    }

    #[test]
    fn test_redact() {
        let classifier = create_test_classifier();
        let text = "My SSN is 123-45-6789 and email is test@example.com";

        let redacted = classifier.redact(text);
        assert!(redacted.contains("***-**-****"));
        assert!(!redacted.contains("123-45-6789"));
    }

    #[test]
    fn test_multiple_matches() {
        let classifier = create_test_classifier();
        let text = "Card: 4111-1111-1111-1111, SSN: 123-45-6789";

        let result = classifier.classify(text);
        assert!(result.matches.len() >= 2);
        assert_eq!(result.level, SensitivityLevel::HighlySensitive);
    }
}
