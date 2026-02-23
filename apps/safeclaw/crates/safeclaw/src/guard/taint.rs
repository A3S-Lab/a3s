//! Taint tracking system for sensitive data
//!
//! Marks sensitive input data with unique taint IDs and generates
//! known variants (base64, hex, URL-encoded) for detection in outputs.
//!
//! **Threat model**: Defends against A2 (compromised AI model) at AS-3 and AS-4.
//! See `docs/threat-model.md` §4 AS-3/AS-4, §5.

use base64::{engine::general_purpose::STANDARD as BASE64, Engine};
use chrono::Utc;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;

/// Type of sensitive data
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaintType {
    CreditCard,
    Ssn,
    Email,
    Phone,
    ApiKey,
    Password,
    Custom(String),
}

impl std::fmt::Display for TaintType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TaintType::CreditCard => write!(f, "CREDIT_CARD"),
            TaintType::Ssn => write!(f, "SSN"),
            TaintType::Email => write!(f, "EMAIL"),
            TaintType::Phone => write!(f, "PHONE"),
            TaintType::ApiKey => write!(f, "API_KEY"),
            TaintType::Password => write!(f, "PASSWORD"),
            TaintType::Custom(s) => write!(f, "{}", s),
        }
    }
}

/// A single tainted data entry
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaintEntry {
    /// Unique taint identifier
    pub id: String,
    /// Original sensitive value
    pub original: String,
    /// Type of sensitive data
    pub taint_type: TaintType,
    /// Pre-computed variants for detection
    pub variants: Vec<String>,
    /// Fuzzy match similarity threshold (0.0 - 1.0)
    pub similarity_threshold: f64,
    /// Timestamp when tainted
    pub created_at: i64,
}

/// Registry that tracks all tainted data in a session
#[derive(Debug, Default)]
pub struct TaintRegistry {
    /// Entries indexed by taint ID
    entries: HashMap<String, TaintEntry>,
}

/// A match found during taint detection
#[derive(Debug, Clone)]
pub struct TaintMatch {
    /// Taint ID that matched
    pub taint_id: String,
    /// The variant that was found
    pub matched_variant: String,
    /// Type of the tainted data
    pub taint_type: TaintType,
    /// Start position in the scanned text
    pub start: usize,
    /// End position in the scanned text
    pub end: usize,
}

impl TaintRegistry {
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Register a new tainted value. Returns the taint ID.
    pub fn register(&mut self, value: &str, taint_type: TaintType) -> String {
        let taint_id = Uuid::new_v4().to_string();
        let variants = generate_variants(value);
        let entry = TaintEntry {
            id: taint_id.clone(),
            original: value.to_string(),
            taint_type,
            variants,
            similarity_threshold: 0.85,
            created_at: Utc::now().timestamp_millis(),
        };
        self.entries.insert(taint_id.clone(), entry);
        taint_id
    }

    /// Check if text contains any tainted data or its variants.
    /// Returns list of (taint_id, matched_variant) pairs.
    pub fn detect(&self, text: &str) -> Vec<TaintMatch> {
        let mut matches = Vec::new();

        for entry in self.entries.values() {
            for variant in &entry.variants {
                if variant.is_empty() {
                    continue;
                }

                let mut start_pos = 0;
                while let Some(pos) = text[start_pos..].find(variant) {
                    let absolute_pos = start_pos + pos;
                    matches.push(TaintMatch {
                        taint_id: entry.id.clone(),
                        matched_variant: variant.clone(),
                        taint_type: entry.taint_type.clone(),
                        start: absolute_pos,
                        end: absolute_pos + variant.len(),
                    });
                    start_pos = absolute_pos + 1;
                }
            }
        }

        matches.sort_by_key(|m| m.start);
        matches
    }

    /// Check if text contains any tainted data (quick boolean check)
    pub fn contains_tainted(&self, text: &str) -> bool {
        for entry in self.entries.values() {
            for variant in &entry.variants {
                if !variant.is_empty() && text.contains(variant) {
                    return true;
                }
            }
        }
        false
    }

    /// Redact all tainted data from text
    pub fn redact(&self, text: &str) -> String {
        let mut matches = self.detect(text);
        if matches.is_empty() {
            return text.to_string();
        }

        // Sort by length descending, then by position, to handle overlaps
        matches.sort_by(|a, b| {
            let len_cmp = (b.end - b.start).cmp(&(a.end - a.start));
            if len_cmp == std::cmp::Ordering::Equal {
                a.start.cmp(&b.start)
            } else {
                len_cmp
            }
        });

        let mut result = text.to_string();
        let mut processed_ranges: Vec<(usize, usize)> = Vec::new();

        for m in matches {
            // Skip if this range overlaps with already processed ranges
            let overlaps = processed_ranges
                .iter()
                .any(|(start, end)| m.start < *end && m.end > *start);

            if !overlaps {
                let replacement = format!("[REDACTED:{}]", m.taint_type);
                let before = &result[..m.start];
                let after = &result[m.end..];
                result = format!("{}{}{}", before, replacement, after);

                // Adjust processed ranges for the length change
                let len_diff = replacement.len() as i64 - (m.end - m.start) as i64;
                for (start, end) in &mut processed_ranges {
                    if *start > m.start {
                        *start = (*start as i64 + len_diff) as usize;
                        *end = (*end as i64 + len_diff) as usize;
                    }
                }

                processed_ranges.push((m.start, m.start + replacement.len()));
            }
        }

        result
    }

    /// Get a taint entry by ID
    pub fn get(&self, taint_id: &str) -> Option<&TaintEntry> {
        self.entries.get(taint_id)
    }

    /// Get all entries
    pub fn entries(&self) -> &HashMap<String, TaintEntry> {
        &self.entries
    }

    /// Remove a taint entry
    pub fn remove(&mut self, taint_id: &str) -> Option<TaintEntry> {
        self.entries.remove(taint_id)
    }

    /// Clear all entries (for session cleanup)
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of tracked entries
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Generate all variants of a sensitive value for detection
fn generate_variants(value: &str) -> Vec<String> {
    let mut variants = Vec::new();

    // 1. Original value (exact match)
    variants.push(value.to_string());

    // 2. Base64 encoded
    let base64_encoded = BASE64.encode(value.as_bytes());
    if base64_encoded != value {
        variants.push(base64_encoded);
    }

    // 3. Hex encoded (lowercase)
    let hex_encoded = hex_encode(value);
    if hex_encoded != value {
        variants.push(hex_encoded);
    }

    // 4. URL encoded
    let url_encoded = url_encode(value);
    if url_encoded != value {
        variants.push(url_encoded);
    }

    // 5. Reversed string
    let reversed: String = value.chars().rev().collect();
    if reversed != value {
        variants.push(reversed);
    }

    // 6. Lowercase version (if different from original)
    let lowercase = value.to_lowercase();
    if lowercase != value {
        variants.push(lowercase);
    }

    // 7. No-separator version (remove spaces, dashes, dots)
    let no_separator: String = value
        .chars()
        .filter(|c| !matches!(c, ' ' | '-' | '.' | '_'))
        .collect();
    if no_separator != value && !no_separator.is_empty() {
        variants.push(no_separator);
    }

    // Remove duplicates while preserving order
    let mut seen = std::collections::HashSet::new();
    variants.retain(|v| seen.insert(v.clone()));

    variants
}

/// Encode string as lowercase hex
fn hex_encode(s: &str) -> String {
    s.bytes().map(|b| format!("{:02x}", b)).collect()
}

/// URL encode a string (percent-encoding)
fn url_encode(s: &str) -> String {
    let mut encoded = String::new();
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                encoded.push(byte as char);
            }
            _ => {
                encoded.push_str(&format!("%{:02X}", byte));
            }
        }
    }
    encoded
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_register_and_detect_exact() {
        let mut registry = TaintRegistry::new();
        let taint_id = registry.register("secret123", TaintType::ApiKey);

        let matches = registry.detect("The API key is secret123 here");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].taint_id, taint_id);
        assert_eq!(matches[0].matched_variant, "secret123");
        assert_eq!(matches[0].start, 15);
        assert_eq!(matches[0].end, 24);
    }

    #[test]
    fn test_detect_base64_variant() {
        let mut registry = TaintRegistry::new();
        registry.register("secret123", TaintType::ApiKey);

        // "secret123" in base64 is "c2VjcmV0MTIz"
        let text = "Encoded as c2VjcmV0MTIz in the logs";
        let matches = registry.detect(text);
        assert!(!matches.is_empty(), "Should detect base64 variant");

        let base64_match = matches.iter().find(|m| m.matched_variant == "c2VjcmV0MTIz");
        assert!(base64_match.is_some(), "Should find base64 variant");
    }

    #[test]
    fn test_detect_hex_variant() {
        let mut registry = TaintRegistry::new();
        registry.register("abc", TaintType::Password);

        // "abc" in hex is "616263"
        let text = "Hex encoded: 616263";
        let matches = registry.detect(text);
        assert!(!matches.is_empty(), "Should detect hex variant");

        let hex_match = matches.iter().find(|m| m.matched_variant == "616263");
        assert!(hex_match.is_some(), "Should find hex variant");
    }

    #[test]
    fn test_detect_url_encoded_variant() {
        let mut registry = TaintRegistry::new();
        registry.register("hello world", TaintType::Custom("test".to_string()));

        // "hello world" URL encoded is "hello%20world"
        let text = "URL: hello%20world";
        let matches = registry.detect(text);
        assert!(!matches.is_empty(), "Should detect URL encoded variant");

        let url_match = matches
            .iter()
            .find(|m| m.matched_variant == "hello%20world");
        assert!(url_match.is_some(), "Should find URL encoded variant");
    }

    #[test]
    fn test_detect_no_separator_variant() {
        let mut registry = TaintRegistry::new();
        registry.register("4111-1111-1111-1111", TaintType::CreditCard);

        // Should detect version without dashes
        let text = "Card number: 4111111111111111";
        let matches = registry.detect(text);
        assert!(!matches.is_empty(), "Should detect no-separator variant");

        let no_sep_match = matches
            .iter()
            .find(|m| m.matched_variant == "4111111111111111");
        assert!(no_sep_match.is_some(), "Should find no-separator variant");
    }

    #[test]
    fn test_redact_tainted_data() {
        let mut registry = TaintRegistry::new();
        registry.register("secret123", TaintType::ApiKey);
        registry.register("user@example.com", TaintType::Email);

        let text = "API key: secret123, email: user@example.com";
        let redacted = registry.redact(text);

        assert!(
            redacted.contains("[REDACTED:API_KEY]"),
            "Should redact API key"
        );
        assert!(redacted.contains("[REDACTED:EMAIL]"), "Should redact email");
        assert!(
            !redacted.contains("secret123"),
            "Should not contain original secret"
        );
        assert!(
            !redacted.contains("user@example.com"),
            "Should not contain original email"
        );
    }

    #[test]
    fn test_no_false_positives() {
        let mut registry = TaintRegistry::new();
        registry.register("secret", TaintType::ApiKey);

        let text = "This is a completely different text with no sensitive data";
        let matches = registry.detect(text);
        assert!(
            matches.is_empty(),
            "Should not detect anything in clean text"
        );

        assert!(!registry.contains_tainted(text));
    }

    #[test]
    fn test_multiple_taints() {
        let mut registry = TaintRegistry::new();
        let id1 = registry.register("secret1", TaintType::ApiKey);
        let id2 = registry.register("secret2", TaintType::Password);

        let text = "Keys: secret1 and secret2";
        let matches = registry.detect(text);
        assert_eq!(matches.len(), 2, "Should detect both secrets");

        let ids: Vec<&str> = matches.iter().map(|m| m.taint_id.as_str()).collect();
        assert!(ids.contains(&id1.as_str()));
        assert!(ids.contains(&id2.as_str()));
    }

    #[test]
    fn test_clear_registry() {
        let mut registry = TaintRegistry::new();
        registry.register("secret1", TaintType::ApiKey);
        registry.register("secret2", TaintType::Password);

        assert_eq!(registry.len(), 2);
        assert!(!registry.is_empty());

        registry.clear();

        assert_eq!(registry.len(), 0);
        assert!(registry.is_empty());

        let matches = registry.detect("secret1 secret2");
        assert!(matches.is_empty(), "Should not detect after clear");
    }

    #[test]
    fn test_remove_entry() {
        let mut registry = TaintRegistry::new();
        let id1 = registry.register("secret1", TaintType::ApiKey);
        let id2 = registry.register("secret2", TaintType::Password);

        assert_eq!(registry.len(), 2);

        let removed = registry.remove(&id1);
        assert!(removed.is_some());
        assert_eq!(removed.unwrap().original, "secret1");
        assert_eq!(registry.len(), 1);

        let matches = registry.detect("secret1 secret2");
        assert_eq!(matches.len(), 1, "Should only detect secret2");
        assert_eq!(matches[0].taint_id, id2);
    }

    #[test]
    fn test_get_entry() {
        let mut registry = TaintRegistry::new();
        let id = registry.register("test_secret", TaintType::ApiKey);

        let entry = registry.get(&id);
        assert!(entry.is_some());
        assert_eq!(entry.unwrap().original, "test_secret");
        assert_eq!(entry.unwrap().taint_type, TaintType::ApiKey);

        let missing = registry.get("nonexistent");
        assert!(missing.is_none());
    }

    #[test]
    fn test_contains_tainted_quick_check() {
        let mut registry = TaintRegistry::new();
        registry.register("secret", TaintType::ApiKey);

        assert!(registry.contains_tainted("The secret is here"));
        assert!(!registry.contains_tainted("Nothing sensitive here"));
    }

    #[test]
    fn test_multiple_occurrences() {
        let mut registry = TaintRegistry::new();
        registry.register("secret", TaintType::ApiKey);

        let text = "secret appears twice: secret";
        let matches = registry.detect(text);
        assert_eq!(matches.len(), 2, "Should detect both occurrences");
        assert_eq!(matches[0].start, 0);
        assert_eq!(matches[1].start, 22);
    }

    #[test]
    fn test_redact_overlapping_matches() {
        let mut registry = TaintRegistry::new();
        registry.register("abc", TaintType::Password);
        registry.register("abcdef", TaintType::ApiKey);

        // "abcdef" contains "abc" - should redact the longer match
        let text = "Value: abcdef";
        let redacted = registry.redact(text);

        // Should contain exactly one redaction
        let redaction_count = redacted.matches("[REDACTED:").count();
        assert_eq!(redaction_count, 1, "Should have one redaction");
    }

    #[test]
    fn test_reversed_variant() {
        let mut registry = TaintRegistry::new();
        registry.register("hello", TaintType::Custom("test".to_string()));

        // "hello" reversed is "olleh"
        let text = "Reversed: olleh";
        let matches = registry.detect(text);
        assert!(!matches.is_empty(), "Should detect reversed variant");

        let reversed_match = matches.iter().find(|m| m.matched_variant == "olleh");
        assert!(reversed_match.is_some(), "Should find reversed variant");
    }

    #[test]
    fn test_lowercase_variant() {
        let mut registry = TaintRegistry::new();
        registry.register("SecretKey", TaintType::ApiKey);

        // Should detect lowercase version
        let text = "Key: secretkey";
        let matches = registry.detect(text);
        assert!(!matches.is_empty(), "Should detect lowercase variant");

        let lowercase_match = matches.iter().find(|m| m.matched_variant == "secretkey");
        assert!(lowercase_match.is_some(), "Should find lowercase variant");
    }

    #[test]
    fn test_empty_value_handling() {
        let mut registry = TaintRegistry::new();
        registry.register("", TaintType::Custom("empty".to_string()));

        let text = "Some text";
        let matches = registry.detect(text);
        assert!(matches.is_empty(), "Empty value should not match anything");
    }

    #[test]
    fn test_taint_type_display() {
        assert_eq!(TaintType::CreditCard.to_string(), "CREDIT_CARD");
        assert_eq!(TaintType::Ssn.to_string(), "SSN");
        assert_eq!(TaintType::Email.to_string(), "EMAIL");
        assert_eq!(TaintType::Phone.to_string(), "PHONE");
        assert_eq!(TaintType::ApiKey.to_string(), "API_KEY");
        assert_eq!(TaintType::Password.to_string(), "PASSWORD");
        assert_eq!(
            TaintType::Custom("CUSTOM_TYPE".to_string()).to_string(),
            "CUSTOM_TYPE"
        );
    }

    #[test]
    fn test_variant_deduplication() {
        let mut registry = TaintRegistry::new();
        let id = registry.register("test", TaintType::ApiKey);

        let entry = registry.get(&id).unwrap();
        let variant_set: std::collections::HashSet<_> = entry.variants.iter().collect();

        // All variants should be unique
        assert_eq!(
            variant_set.len(),
            entry.variants.len(),
            "Variants should not contain duplicates"
        );
    }
}
