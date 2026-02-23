//! Semantic privacy analyzer for context-aware PII detection.
//!
//! Detects sensitive data disclosed in natural language that regex patterns miss:
//! - "my password is hunter2"
//! - "my SSN is 123456789" (no dashes)
//!
//! **Threat model**: Defends against A1 (malicious user) at AS-2 (PII classification).
//! See `docs/threat-model.md` §4 AS-2, §5.
//! - "the API key is sk-abc123..."
//! - "credit card number 4111111111111111" (no separators)
//!
//! Uses a two-phase approach:
//! 1. Trigger phrases identify disclosure context ("my password is", "here's my SSN")
//! 2. Value extraction captures the disclosed data after the trigger

use crate::config::SensitivityLevel;
use serde::{Deserialize, Serialize};

/// Result of semantic analysis
#[derive(Debug, Clone)]
pub struct SemanticResult {
    /// Detected semantic matches
    pub matches: Vec<SemanticMatch>,
    /// Highest sensitivity level found
    pub level: SensitivityLevel,
    /// Whether TEE processing is recommended
    pub requires_tee: bool,
}

/// A single semantic match
#[derive(Debug, Clone)]
pub struct SemanticMatch {
    /// Category of the detected disclosure
    pub category: SemanticCategory,
    /// The trigger phrase that was matched
    pub trigger: String,
    /// The extracted sensitive value (redacted for logging)
    pub redacted_value: String,
    /// Start position of the entire match in the original text
    pub start: usize,
    /// End position of the entire match
    pub end: usize,
    /// Sensitivity level
    pub level: SensitivityLevel,
    /// Confidence score (0.0 - 1.0)
    pub confidence: f64,
}

/// Categories of semantic PII disclosure
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SemanticCategory {
    /// Password or secret disclosure
    Password,
    /// Social Security Number
    Ssn,
    /// Credit card number
    CreditCard,
    /// API key or token
    ApiKey,
    /// Bank account or routing number
    BankAccount,
    /// Date of birth
    DateOfBirth,
    /// Address disclosure
    Address,
    /// Medical information
    Medical,
    /// Generic secret disclosure
    GenericSecret,
}

impl SemanticCategory {
    fn sensitivity(&self) -> SensitivityLevel {
        match self {
            Self::Password | Self::ApiKey => SensitivityLevel::Critical,
            Self::CreditCard | Self::Ssn | Self::BankAccount => SensitivityLevel::HighlySensitive,
            Self::DateOfBirth | Self::Address | Self::Medical => SensitivityLevel::Sensitive,
            Self::GenericSecret => SensitivityLevel::Sensitive,
        }
    }
}

/// Trigger phrase definition
struct TriggerPhrase {
    /// Phrases that indicate disclosure (lowercase)
    phrases: Vec<&'static str>,
    /// Category of the disclosure
    category: SemanticCategory,
    /// Optional value validator (returns true if the extracted value looks valid)
    validator: fn(&str) -> bool,
    /// Confidence boost when validator passes
    confidence: f64,
}

/// Semantic privacy analyzer
pub struct SemanticAnalyzer {
    triggers: Vec<TriggerPhrase>,
}

impl SemanticAnalyzer {
    /// Create a new semantic analyzer with default trigger phrases
    pub fn new() -> Self {
        Self {
            triggers: Self::default_triggers(),
        }
    }

    /// Analyze text for semantic PII disclosure
    pub fn analyze(&self, text: &str) -> SemanticResult {
        let lower = text.to_lowercase();
        let mut matches = Vec::new();
        let mut max_level = SensitivityLevel::Normal;

        for trigger in &self.triggers {
            for phrase in &trigger.phrases {
                // Find all occurrences of the trigger phrase
                let mut search_from = 0;
                while let Some(pos) = lower[search_from..].find(phrase) {
                    let abs_pos = search_from + pos;
                    let phrase_end = abs_pos + phrase.len();

                    // Extract the value after the trigger phrase
                    if let Some(value) = self.extract_value(text, phrase_end) {
                        let value_trimmed = value.trim();
                        if !value_trimmed.is_empty() {
                            let valid = (trigger.validator)(value_trimmed);
                            let confidence = if valid {
                                trigger.confidence
                            } else {
                                trigger.confidence * 0.5
                            };

                            // Only report if confidence is meaningful
                            if confidence >= 0.3 {
                                let level = trigger.category.sensitivity();
                                if level > max_level {
                                    max_level = level;
                                }

                                let end = phrase_end + value.len();
                                matches.push(SemanticMatch {
                                    category: trigger.category,
                                    trigger: text[abs_pos..phrase_end].to_string(),
                                    redacted_value: redact_value(value_trimmed),
                                    start: abs_pos,
                                    end,
                                    level,
                                    confidence,
                                });
                            }
                        }
                    }

                    search_from = phrase_end;
                }
            }
        }

        // Deduplicate overlapping matches (keep highest confidence)
        matches.sort_by(|a, b| {
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        let mut deduped = Vec::new();
        for m in matches {
            let overlaps = deduped
                .iter()
                .any(|existing: &SemanticMatch| m.start < existing.end && m.end > existing.start);
            if !overlaps {
                deduped.push(m);
            }
        }

        let requires_tee = max_level >= SensitivityLevel::Sensitive;

        SemanticResult {
            matches: deduped,
            level: max_level,
            requires_tee,
        }
    }

    /// Extract the value portion after a trigger phrase.
    /// Captures until end of line, sentence boundary, or max length.
    fn extract_value<'a>(&self, text: &'a str, start: usize) -> Option<&'a str> {
        if start >= text.len() {
            return None;
        }

        let remaining = &text[start..];

        // Skip leading whitespace and common separators
        let value_start = remaining
            .find(|c: char| !c.is_whitespace() && c != ':' && c != '=' && c != '"' && c != '\'')
            .unwrap_or(0);

        let value_text = &remaining[value_start..];
        if value_text.is_empty() {
            return None;
        }

        // Find the end of the value: newline, period+space, comma+space, conjunctions, or max 128 chars
        let max_len = value_text.len().min(128);
        let value_slice = &value_text[..max_len];

        let end = value_slice
            .find('\n')
            .or_else(|| value_slice.find(". "))
            .or_else(|| value_slice.find(", "))
            .or_else(|| value_slice.find(" and "))
            .or_else(|| value_slice.find(" & "))
            .or_else(|| value_slice.find('\t'))
            .unwrap_or(value_slice.len());

        if end == 0 {
            return None;
        }

        Some(&remaining[value_start..value_start + end])
    }

    fn default_triggers() -> Vec<TriggerPhrase> {
        vec![
            // Password disclosure
            TriggerPhrase {
                phrases: vec![
                    "my password is",
                    "my password:",
                    "my pass is",
                    "my pwd is",
                    "password is",
                    "password:",
                    "the password is",
                    "here's my password",
                    "here is my password",
                    "login password",
                    "密码是",
                    "密码:",
                    "我的密码",
                ],
                category: SemanticCategory::Password,
                validator: |v| v.len() >= 4 && !v.contains(' ') || v.len() >= 6,
                confidence: 0.95,
            },
            // SSN disclosure
            TriggerPhrase {
                phrases: vec![
                    "my ssn is",
                    "my ssn:",
                    "my social security",
                    "social security number is",
                    "social security:",
                    "ssn is",
                    "ssn:",
                    "社会安全号",
                ],
                category: SemanticCategory::Ssn,
                validator: |v| {
                    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
                    digits.len() == 9
                },
                confidence: 0.95,
            },
            // Credit card disclosure
            TriggerPhrase {
                phrases: vec![
                    "my card number is",
                    "my card number:",
                    "my credit card is",
                    "my credit card:",
                    "card number is",
                    "card number:",
                    "credit card number",
                    "cc number is",
                    "cc number:",
                    "卡号是",
                    "信用卡号",
                ],
                category: SemanticCategory::CreditCard,
                validator: |v| {
                    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
                    (13..=19).contains(&digits.len())
                },
                confidence: 0.95,
            },
            // API key disclosure
            TriggerPhrase {
                phrases: vec![
                    "my api key is",
                    "my api key:",
                    "api key is",
                    "api key:",
                    "api_key is",
                    "api_key:",
                    "api token is",
                    "api token:",
                    "my token is",
                    "my token:",
                    "secret key is",
                    "secret key:",
                    "access key is",
                    "access key:",
                    "bearer token",
                    "authorization:",
                ],
                category: SemanticCategory::ApiKey,
                validator: |v| {
                    v.len() >= 16
                        && v.chars()
                            .all(|c| c.is_ascii_alphanumeric() || "-_./+=".contains(c))
                },
                confidence: 0.90,
            },
            // Bank account disclosure
            TriggerPhrase {
                phrases: vec![
                    "my account number is",
                    "my account number:",
                    "account number is",
                    "account number:",
                    "routing number is",
                    "routing number:",
                    "bank account is",
                    "bank account:",
                    "iban is",
                    "iban:",
                    "银行账号",
                ],
                category: SemanticCategory::BankAccount,
                validator: |v| {
                    let digits: String = v.chars().filter(|c| c.is_ascii_digit()).collect();
                    digits.len() >= 8
                },
                confidence: 0.90,
            },
            // Date of birth
            TriggerPhrase {
                phrases: vec![
                    "my birthday is",
                    "my date of birth is",
                    "my date of birth:",
                    "my dob is",
                    "my dob:",
                    "born on",
                    "date of birth:",
                    "出生日期",
                    "生日是",
                ],
                category: SemanticCategory::DateOfBirth,
                validator: |v| {
                    // Rough check: contains digits and separators
                    let has_digits = v.chars().any(|c| c.is_ascii_digit());
                    let has_sep = v.contains('/') || v.contains('-') || v.contains('.');
                    has_digits && (has_sep || v.len() >= 6)
                },
                confidence: 0.85,
            },
            // Address disclosure
            TriggerPhrase {
                phrases: vec![
                    "my address is",
                    "my address:",
                    "i live at",
                    "my home address",
                    "mailing address is",
                    "mailing address:",
                    "家庭住址",
                    "我住在",
                ],
                category: SemanticCategory::Address,
                validator: |v| v.len() >= 10 && v.chars().any(|c| c.is_ascii_digit()),
                confidence: 0.80,
            },
            // Medical information
            TriggerPhrase {
                phrases: vec![
                    "my diagnosis is",
                    "my diagnosis:",
                    "diagnosed with",
                    "my medical record",
                    "my prescription is",
                    "my prescription:",
                    "my blood type is",
                    "my blood type:",
                    "patient id is",
                    "patient id:",
                    "medical record number",
                ],
                category: SemanticCategory::Medical,
                validator: |v| v.len() >= 3,
                confidence: 0.85,
            },
            // Generic secret disclosure
            TriggerPhrase {
                phrases: vec![
                    "my secret is",
                    "my secret:",
                    "the secret is",
                    "here's a secret",
                    "don't tell anyone",
                    "keep this private",
                    "confidential:",
                    "private key is",
                    "private key:",
                ],
                category: SemanticCategory::GenericSecret,
                validator: |v| v.len() >= 4,
                confidence: 0.70,
            },
        ]
    }
}

impl Default for SemanticAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

/// Redact a value for safe logging (show first and last char only)
fn redact_value(value: &str) -> String {
    if value.len() <= 4 {
        return "****".to_string();
    }
    let chars: Vec<char> = value.chars().collect();
    let first = chars[0];
    let last = chars[chars.len() - 1];
    format!(
        "{}{}{}",
        first,
        "*".repeat(chars.len().saturating_sub(2).min(20)),
        last
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn analyzer() -> SemanticAnalyzer {
        SemanticAnalyzer::new()
    }

    // ---- Password detection ----

    #[test]
    fn test_detect_password_disclosure() {
        let a = analyzer();
        let result = a.analyze("my password is hunter2");
        assert_eq!(result.level, SensitivityLevel::Critical);
        assert!(result.requires_tee);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].category, SemanticCategory::Password);
        assert!(result.matches[0].confidence >= 0.9);
    }

    #[test]
    fn test_detect_password_with_colon() {
        let a = analyzer();
        let result = a.analyze("password: MyS3cretP@ss!");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].category, SemanticCategory::Password);
    }

    #[test]
    fn test_detect_chinese_password() {
        let a = analyzer();
        let result = a.analyze("我的密码是abc12345");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].category, SemanticCategory::Password);
    }

    #[test]
    fn test_no_false_positive_password() {
        let a = analyzer();
        let result = a.analyze("Please reset your password in the settings page");
        // "password in" doesn't match any trigger phrase exactly
        assert!(result.matches.is_empty() || result.matches[0].confidence < 0.5);
    }

    // ---- SSN detection ----

    #[test]
    fn test_detect_ssn_no_dashes() {
        let a = analyzer();
        let result = a.analyze("my ssn is 123456789");
        assert_eq!(result.level, SensitivityLevel::HighlySensitive);
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].category, SemanticCategory::Ssn);
    }

    #[test]
    fn test_detect_ssn_with_dashes() {
        let a = analyzer();
        let result = a.analyze("my social security number is 123-45-6789");
        assert_eq!(result.matches.len(), 1);
        assert_eq!(result.matches[0].category, SemanticCategory::Ssn);
    }

    // ---- Credit card detection ----

    #[test]
    fn test_detect_credit_card_no_separators() {
        let a = analyzer();
        let result = a.analyze("my card number is 4111111111111111");
        assert_eq!(result.level, SensitivityLevel::HighlySensitive);
        assert_eq!(result.matches[0].category, SemanticCategory::CreditCard);
    }

    // ---- API key detection ----

    #[test]
    fn test_detect_api_key() {
        let a = analyzer();
        let result = a.analyze("my api key is sk-1234567890abcdef1234567890abcdef");
        assert_eq!(result.level, SensitivityLevel::Critical);
        assert_eq!(result.matches[0].category, SemanticCategory::ApiKey);
    }

    #[test]
    fn test_detect_bearer_token() {
        let a = analyzer();
        let result =
            a.analyze("authorization: Bearer eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjM0NTY3ODkwIn0");
        assert!(!result.matches.is_empty());
        assert_eq!(result.matches[0].category, SemanticCategory::ApiKey);
    }

    // ---- Bank account detection ----

    #[test]
    fn test_detect_bank_account() {
        let a = analyzer();
        let result = a.analyze("my account number is 1234567890");
        assert_eq!(result.matches[0].category, SemanticCategory::BankAccount);
    }

    // ---- Date of birth detection ----

    #[test]
    fn test_detect_dob() {
        let a = analyzer();
        let result = a.analyze("my date of birth is 1990/01/15");
        assert_eq!(result.matches[0].category, SemanticCategory::DateOfBirth);
    }

    // ---- Address detection ----

    #[test]
    fn test_detect_address() {
        let a = analyzer();
        let result = a.analyze("my address is 123 Main Street, Springfield IL 62701");
        assert_eq!(result.matches[0].category, SemanticCategory::Address);
    }

    // ---- Medical detection ----

    #[test]
    fn test_detect_medical() {
        let a = analyzer();
        let result = a.analyze("I was diagnosed with type 2 diabetes");
        assert_eq!(result.matches[0].category, SemanticCategory::Medical);
        assert_eq!(result.level, SensitivityLevel::Sensitive);
    }

    // ---- Generic secret ----

    #[test]
    fn test_detect_generic_secret() {
        let a = analyzer();
        let result = a.analyze("my secret is that I failed the exam");
        assert_eq!(result.matches[0].category, SemanticCategory::GenericSecret);
    }

    // ---- Clean text ----

    #[test]
    fn test_clean_text() {
        let a = analyzer();
        let result = a.analyze("Hello, how are you today? The weather is nice.");
        assert!(result.matches.is_empty());
        assert_eq!(result.level, SensitivityLevel::Normal);
        assert!(!result.requires_tee);
    }

    #[test]
    fn test_clean_technical_text() {
        let a = analyzer();
        let result = a.analyze("Please implement the password reset feature using OAuth2");
        assert!(result.matches.is_empty() || result.matches.iter().all(|m| m.confidence < 0.5));
    }

    // ---- Redaction ----

    #[test]
    fn test_redact_value_short() {
        assert_eq!(redact_value("abc"), "****");
    }

    #[test]
    fn test_redact_value_normal() {
        let redacted = redact_value("hunter2");
        assert!(redacted.starts_with('h'));
        assert!(redacted.ends_with('2'));
        assert!(redacted.contains('*'));
    }

    // ---- Multiple matches ----

    #[test]
    fn test_multiple_disclosures() {
        let a = analyzer();
        let result = a.analyze("my password is hunter2 and my ssn is 123456789");
        assert!(result.matches.len() >= 2);
        assert_eq!(result.level, SensitivityLevel::Critical);
    }
}
