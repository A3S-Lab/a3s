//! Privacy classification and data protection

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum PrivacyError {
    #[error("Invalid regex pattern: {0}")]
    InvalidPattern(String),
    #[error("Classification error: {0}")]
    Classification(String),
}

/// Sensitivity level for classified data
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SensitivityLevel {
    Public,
    Normal,
    Sensitive,
    HighlySensitive,
    Critical,
}

impl std::fmt::Display for SensitivityLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Public => write!(f, "Public"),
            Self::Normal => write!(f, "Normal"),
            Self::Sensitive => write!(f, "Sensitive"),
            Self::HighlySensitive => write!(f, "HighlySensitive"),
            Self::Critical => write!(f, "Critical"),
        }
    }
}

impl Default for SensitivityLevel {
    fn default() -> Self {
        Self::Normal
    }
}

impl PartialOrd for SensitivityLevel {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for SensitivityLevel {
    fn cmp(&self, other: &Self) -> Ordering {
        let self_level = match self {
            Self::Public => 0,
            Self::Normal => 1,
            Self::Sensitive => 2,
            Self::HighlySensitive => 3,
            Self::Critical => 4,
        };
        let other_level = match other {
            Self::Public => 0,
            Self::Normal => 1,
            Self::Sensitive => 2,
            Self::HighlySensitive => 3,
            Self::Critical => 4,
        };
        self_level.cmp(&other_level)
    }
}

/// Classification rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClassificationRule {
    pub name: String,
    pub pattern: String,
    pub level: SensitivityLevel,
    pub description: String,
}

/// A single match found during classification
#[derive(Debug, Clone)]
pub struct ClassificationMatch {
    pub rule_name: String,
    pub level: SensitivityLevel,
    pub start: usize,
    pub end: usize,
    pub matched_text: String,
}

/// PII match (alias for ClassificationMatch for compatibility)
pub type PiiMatch = ClassificationMatch;

/// Classification result
#[derive(Debug, Clone)]
pub struct ClassificationResult {
    pub overall_level: SensitivityLevel,
    pub matches: Vec<ClassificationMatch>,
    pub requires_tee: bool,
}

/// Redaction strategy
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RedactionStrategy {
    Mask,
    Remove,
    Hash,
}

/// Regex-based classifier
pub struct RegexClassifier {
    rules: Vec<(String, Regex, SensitivityLevel)>,
    default_level: SensitivityLevel,
}

impl RegexClassifier {
    /// Create a new classifier with the given rules
    pub fn new(
        rules: &[ClassificationRule],
        default_level: SensitivityLevel,
    ) -> Result<Self, PrivacyError> {
        let compiled_rules = rules
            .iter()
            .map(|rule| {
                let regex = Regex::new(&rule.pattern)
                    .map_err(|e| PrivacyError::InvalidPattern(format!("{}: {}", rule.name, e)))?;
                Ok((rule.name.clone(), regex, rule.level))
            })
            .collect::<Result<Vec<_>, PrivacyError>>()?;

        Ok(Self {
            rules: compiled_rules,
            default_level,
        })
    }

    /// Classify text and return matches
    pub fn classify(&self, text: &str) -> ClassificationResult {
        let mut matches = Vec::new();
        let mut overall_level = self.default_level;

        for (rule_name, regex, level) in &self.rules {
            for mat in regex.find_iter(text) {
                matches.push(ClassificationMatch {
                    rule_name: rule_name.clone(),
                    level: *level,
                    start: mat.start(),
                    end: mat.end(),
                    matched_text: mat.as_str().to_string(),
                });
                if *level > overall_level {
                    overall_level = *level;
                }
            }
        }

        let requires_tee = overall_level >= SensitivityLevel::Sensitive;

        ClassificationResult {
            overall_level,
            matches,
            requires_tee,
        }
    }

    /// Redact sensitive data in text
    pub fn redact(&self, text: &str, strategy: RedactionStrategy) -> String {
        let mut result = text.to_string();
        let classification = self.classify(text);

        // Sort matches by start position in reverse order to avoid offset issues
        let mut matches = classification.matches;
        matches.sort_by(|a, b| b.start.cmp(&a.start));

        for mat in matches {
            let redacted = redact_text(&mat.matched_text, &mat.rule_name, strategy);
            result.replace_range(mat.start..mat.end, &redacted);
        }

        result
    }

    /// Check if text contains sensitive data
    pub fn contains_sensitive(&self, text: &str) -> bool {
        self.classify(text).overall_level >= SensitivityLevel::Sensitive
    }

    /// Get the highest sensitivity level in text
    pub fn get_sensitivity_level(&self, text: &str) -> SensitivityLevel {
        self.classify(text).overall_level
    }
}

/// Redact text based on rule name and strategy
pub fn redact_text(text: &str, rule_name: &str, strategy: RedactionStrategy) -> String {
    match strategy {
        RedactionStrategy::Mask => match rule_name {
            "ssn" => "***-**-****".to_string(),
            "email" => {
                if let Some(at_pos) = text.find('@') {
                    format!("****{}", &text[at_pos..])
                } else {
                    "[REDACTED]".to_string()
                }
            }
            "credit_card" => {
                let digits: String = text.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 4 {
                    format!("****-****-****-{}", &digits[digits.len() - 4..])
                } else {
                    "****-****-****-****".to_string()
                }
            }
            "phone" => "***-***-****".to_string(),
            _ => "[REDACTED]".to_string(),
        },
        RedactionStrategy::Remove => String::new(),
        RedactionStrategy::Hash => {
            format!("[HASH:{}]", text.len())
        }
    }
}

/// Default classification rules
pub fn default_classification_rules() -> Vec<ClassificationRule> {
    vec![
        ClassificationRule {
            name: "credit_card".to_string(),
            pattern: r"\b\d{4}[-\s]?\d{4}[-\s]?\d{4}[-\s]?\d{4}\b".to_string(),
            level: SensitivityLevel::HighlySensitive,
            description: "Credit card number".to_string(),
        },
        ClassificationRule {
            name: "ssn".to_string(),
            pattern: r"\b\d{3}-\d{2}-\d{4}\b".to_string(),
            level: SensitivityLevel::HighlySensitive,
            description: "Social Security Number".to_string(),
        },
        ClassificationRule {
            name: "email".to_string(),
            pattern: r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Z|a-z]{2,}\b".to_string(),
            level: SensitivityLevel::Sensitive,
            description: "Email address".to_string(),
        },
        ClassificationRule {
            name: "phone".to_string(),
            pattern: r"\b\d{3}[-.\s]?\d{3}[-.\s]?\d{4}\b".to_string(),
            level: SensitivityLevel::Sensitive,
            description: "Phone number".to_string(),
        },
        ClassificationRule {
            name: "api_key".to_string(),
            pattern: r"\b[A-Za-z0-9_-]{32,}\b".to_string(),
            level: SensitivityLevel::Critical,
            description: "API key or token".to_string(),
        },
    ]
}

/// Default dangerous commands (for command filtering)
pub fn default_dangerous_commands() -> Vec<String> {
    vec![
        "rm -rf".to_string(),
        "dd if=".to_string(),
        "mkfs".to_string(),
        ":(){ :|:& };:".to_string(), // fork bomb
    ]
}

/// Keyword matcher configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct KeywordMatcherConfig {
    pub keywords: Vec<String>,
    pub case_sensitive: bool,
    pub sensitive_keywords: Vec<String>,
    pub tee_threshold: SensitivityLevel,
}

impl Default for KeywordMatcherConfig {
    fn default() -> Self {
        Self {
            keywords: Vec::new(),
            case_sensitive: false,
            sensitive_keywords: Vec::new(),
            tee_threshold: SensitivityLevel::Sensitive,
        }
    }
}

/// Keyword matcher
pub struct KeywordMatcher {
    keywords: Vec<String>,
    case_sensitive: bool,
    sensitive_keywords: Vec<String>,
    tee_threshold: SensitivityLevel,
}

impl KeywordMatcher {
    /// Create a new keyword matcher
    pub fn new(config: KeywordMatcherConfig) -> Self {
        Self {
            keywords: config.keywords,
            case_sensitive: config.case_sensitive,
            sensitive_keywords: config.sensitive_keywords,
            tee_threshold: config.tee_threshold,
        }
    }

    /// Create from keyword list (legacy)
    pub fn from_keywords(keywords: Vec<String>) -> Self {
        Self {
            keywords,
            case_sensitive: false,
            sensitive_keywords: Vec::new(),
            tee_threshold: SensitivityLevel::Sensitive,
        }
    }

    /// Create from config
    pub fn from_config(config: KeywordMatcherConfig) -> Self {
        Self::new(config)
    }

    /// Check if text matches any keyword
    pub fn matches(&self, text: &str) -> bool {
        let text_to_check = if self.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };

        let mut all_keywords = self.keywords.iter().chain(self.sensitive_keywords.iter());

        all_keywords.any(|keyword| {
            let keyword_to_check = if self.case_sensitive {
                keyword.clone()
            } else {
                keyword.to_lowercase()
            };
            text_to_check.contains(&keyword_to_check)
        })
    }

    /// Classify text based on keyword matches
    pub fn classify(&self, text: &str) -> SensitivityLevel {
        let text_to_check = if self.case_sensitive {
            text.to_string()
        } else {
            text.to_lowercase()
        };

        // Check sensitive keywords first
        for keyword in &self.sensitive_keywords {
            let keyword_to_check = if self.case_sensitive {
                keyword.clone()
            } else {
                keyword.to_lowercase()
            };
            if text_to_check.contains(&keyword_to_check) {
                return self.tee_threshold;
            }
        }

        // Check regular keywords (personal context, not high sensitivity)
        if self.matches(text) {
            SensitivityLevel::Normal
        } else {
            SensitivityLevel::Public
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sensitivity_level_ordering() {
        assert!(SensitivityLevel::Critical > SensitivityLevel::HighlySensitive);
        assert!(SensitivityLevel::HighlySensitive > SensitivityLevel::Sensitive);
        assert!(SensitivityLevel::Sensitive > SensitivityLevel::Normal);
        assert!(SensitivityLevel::Normal > SensitivityLevel::Public);
    }

    #[test]
    fn test_classifier_credit_card() {
        let rules = default_classification_rules();
        let classifier = RegexClassifier::new(&rules, SensitivityLevel::Normal).unwrap();

        let text = "My card is 4111-1111-1111-1111";
        let result = classifier.classify(text);

        assert_eq!(result.overall_level, SensitivityLevel::HighlySensitive);
        assert!(result.requires_tee);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].rule_name, "credit_card");
    }

    #[test]
    fn test_classifier_email() {
        let rules = default_classification_rules();
        let classifier = RegexClassifier::new(&rules, SensitivityLevel::Normal).unwrap();

        let text = "Contact: test@example.com";
        let result = classifier.classify(text);

        assert_eq!(result.overall_level, SensitivityLevel::Sensitive);
        assert!(result.requires_tee);
    }

    #[test]
    fn test_redact_ssn() {
        let text = "123-45-6789";
        let redacted = redact_text(text, "ssn", RedactionStrategy::Mask);
        assert_eq!(redacted, "***-**-****");
    }

    #[test]
    fn test_redact_credit_card() {
        let text = "4111-1111-1111-1111";
        let redacted = redact_text(text, "credit_card", RedactionStrategy::Mask);
        assert_eq!(redacted, "****-****-****-1111");
    }

    #[test]
    fn test_redact_email() {
        let text = "test@example.com";
        let redacted = redact_text(text, "email", RedactionStrategy::Mask);
        assert_eq!(redacted, "****@example.com");
    }

    #[test]
    fn test_keyword_matcher() {
        let config = KeywordMatcherConfig {
            keywords: vec!["secret".to_string()],
            case_sensitive: false,
            sensitive_keywords: vec!["password".to_string()],
            tee_threshold: SensitivityLevel::HighlySensitive,
        };
        let matcher = KeywordMatcher::new(config);

        assert!(matcher.matches("This is a secret message"));
        assert!(matcher.matches("Enter your password"));
        assert!(!matcher.matches("This is a normal message"));
    }

    #[test]
    fn test_keyword_matcher_classify() {
        let config = KeywordMatcherConfig {
            keywords: vec![],
            case_sensitive: false,
            sensitive_keywords: vec!["confidential".to_string()],
            tee_threshold: SensitivityLevel::HighlySensitive,
        };
        let matcher = KeywordMatcher::new(config);

        assert_eq!(
            matcher.classify("This is confidential"),
            SensitivityLevel::HighlySensitive
        );
        assert_eq!(matcher.classify("This is public"), SensitivityLevel::Public);
    }
}
