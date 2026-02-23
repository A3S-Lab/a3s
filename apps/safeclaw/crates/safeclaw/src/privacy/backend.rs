//! Pluggable classifier backend architecture
//!
//! Defines the `ClassifierBackend` trait for pluggable PII classification,
//! and `CompositeClassifier` that chains multiple backends together.
//!
//! **Threat model**: Defends against A1 (malicious user) at AS-2 (PII classification).
//! See `docs/threat-model.md` §4 AS-2, §5.
//!
//! ## Architecture
//!
//! ```text
//! Input text → [RegexBackend] → [SemanticBackend] → (optional) [LlmBackend]
//!                   ↓                  ↓                         ↓
//!              merge results → deduplicate by span → ClassificationResult
//! ```
//!
//! ## Accuracy labeling
//!
//! Every `PiiMatch` includes a `backend` field identifying which classifier
//! caught it. This enables audit trails and accuracy analysis.

use crate::config::SensitivityLevel;
use async_trait::async_trait;

/// A single PII match found by a classifier backend.
#[derive(Debug, Clone)]
pub struct PiiMatch {
    /// Rule or pattern name that matched
    pub rule_name: String,
    /// Sensitivity level of the match
    pub level: SensitivityLevel,
    /// Start byte offset in the input text
    pub start: usize,
    /// End byte offset in the input text
    pub end: usize,
    /// Confidence score (0.0 to 1.0)
    pub confidence: f64,
    /// Which backend produced this match
    pub backend: String,
}

/// Pluggable classification backend interface.
///
/// Implementations can use regex, semantic analysis, LLM calls, or any
/// other technique to detect PII in text.
#[async_trait]
pub trait ClassifierBackend: Send + Sync {
    /// Classify text and return all PII matches found.
    async fn classify(&self, text: &str) -> Vec<PiiMatch>;

    /// Minimum confidence this backend can guarantee.
    ///
    /// Used by `CompositeClassifier` to resolve overlapping matches:
    /// when two backends find PII at the same span, the one with
    /// higher confidence floor wins.
    fn confidence_floor(&self) -> f64;

    /// Human-readable name for this backend (used in audit logs).
    fn name(&self) -> &str;
}

/// Regex-based classifier backend.
///
/// Wraps the existing `a3s_common::privacy::RegexClassifier`. Fast, high-precision,
/// but low recall for semantic PII (addresses in prose, passwords in context).
pub struct RegexBackend {
    inner: a3s_common::privacy::RegexClassifier,
}

impl RegexBackend {
    /// Create from existing classification rules
    pub fn new(
        rules: Vec<crate::config::ClassificationRule>,
        default_level: SensitivityLevel,
    ) -> Result<Self, String> {
        let inner = a3s_common::privacy::RegexClassifier::new(&rules, default_level)
            .map_err(|e| format!("Failed to compile classification rules: {}", e))?;
        Ok(Self { inner })
    }
}

#[async_trait]
impl ClassifierBackend for RegexBackend {
    async fn classify(&self, text: &str) -> Vec<PiiMatch> {
        let result = self.inner.classify(text);
        result
            .matches
            .into_iter()
            .map(|m| PiiMatch {
                rule_name: m.rule_name,
                level: m.level,
                start: m.start,
                end: m.end,
                confidence: 0.95, // Regex matches are high-precision
                backend: "regex".to_string(),
            })
            .collect()
    }

    fn confidence_floor(&self) -> f64 {
        0.90
    }

    fn name(&self) -> &str {
        "regex"
    }
}

/// Semantic analysis classifier backend.
///
/// Wraps the existing `SemanticAnalyzer` for context-aware PII detection
/// (e.g., "my password is hunter2", "I live at 123 Main St").
pub struct SemanticBackend {
    inner: crate::privacy::SemanticAnalyzer,
}

impl SemanticBackend {
    /// Create a new semantic backend
    pub fn new(analyzer: crate::privacy::SemanticAnalyzer) -> Self {
        Self { inner: analyzer }
    }
}

#[async_trait]
impl ClassifierBackend for SemanticBackend {
    async fn classify(&self, text: &str) -> Vec<PiiMatch> {
        let result = self.inner.analyze(text);
        result
            .matches
            .into_iter()
            .map(|m| PiiMatch {
                rule_name: format!("semantic:{:?}", m.category),
                level: m.level,
                start: m.start,
                end: m.end,
                confidence: m.confidence,
                backend: "semantic".to_string(),
            })
            .collect()
    }

    fn confidence_floor(&self) -> f64 {
        0.60
    }

    fn name(&self) -> &str {
        "semantic"
    }
}

/// Composite classifier that chains multiple backends and merges results.
///
/// Default chain: Regex → Semantic → (optional) LLM.
/// Results are merged with deduplication by span overlap:
/// when two matches overlap, the one with higher confidence wins.
pub struct CompositeClassifier {
    backends: Vec<Box<dyn ClassifierBackend>>,
}

impl CompositeClassifier {
    /// Create a new composite classifier with the given backends.
    ///
    /// Backends are evaluated in order. All results are merged.
    pub fn new(backends: Vec<Box<dyn ClassifierBackend>>) -> Self {
        Self { backends }
    }

    /// Classify text through all backends and merge results.
    pub async fn classify(&self, text: &str) -> CompositeResult {
        let mut all_matches = Vec::new();

        for backend in &self.backends {
            let matches = backend.classify(text).await;
            all_matches.extend(matches);
        }

        // Deduplicate overlapping matches — highest confidence wins
        let deduped = deduplicate_matches(all_matches);

        // Determine overall sensitivity level
        let overall_level = deduped
            .iter()
            .map(|m| m.level)
            .max()
            .unwrap_or(SensitivityLevel::Normal);

        let requires_tee = overall_level >= SensitivityLevel::Sensitive;

        CompositeResult {
            level: overall_level,
            matches: deduped,
            requires_tee,
        }
    }

    /// Check if text contains any sensitive data
    pub async fn contains_sensitive(&self, text: &str) -> bool {
        let result = self.classify(text).await;
        !result.matches.is_empty()
    }
}

/// Result from the composite classifier, including backend attribution.
#[derive(Debug, Clone)]
pub struct CompositeResult {
    /// Overall sensitivity level (max across all matches)
    pub level: SensitivityLevel,
    /// All deduplicated matches with backend attribution
    pub matches: Vec<PiiMatch>,
    /// Whether TEE processing is required
    pub requires_tee: bool,
}

// ---------------------------------------------------------------------------
// LLM-based classifier backend (behind `llm-classifier` feature flag)
// ---------------------------------------------------------------------------

/// Trait for invoking an LLM to classify text.
///
/// Decoupled from `AgentEngine` so the backend is testable with mock responses.
#[async_trait]
pub trait LlmClassifierFn: Send + Sync {
    /// Send a prompt to the LLM and return the text response.
    async fn call(&self, prompt: &str) -> Result<String, String>;
}

/// LLM-based PII classifier backend.
///
/// Sends text to an LLM with a structured classification prompt and parses
/// the JSON response into `PiiMatch` results. Highest recall but slowest
/// and most expensive — intended as the final backend in the chain.
///
/// **Threat model**: Catches semantic PII that regex and trigger-phrase
/// analysis miss (e.g., "I grew up at the corner of Oak and 5th" → address).
pub struct LlmBackend {
    llm: Box<dyn LlmClassifierFn>,
}

impl LlmBackend {
    /// Create a new LLM classifier backend.
    pub fn new(llm: Box<dyn LlmClassifierFn>) -> Self {
        Self { llm }
    }

    /// The system prompt sent to the LLM for PII classification.
    fn classification_prompt(text: &str) -> String {
        format!(
            r#"You are a PII (Personally Identifiable Information) classifier. Analyze the following text and identify ALL instances of PII.

For each PII found, return a JSON array of objects with these fields:
- "rule_name": category of PII (e.g., "email", "phone", "ssn", "credit_card", "address", "name", "password", "api_key", "medical", "financial", "date_of_birth", "national_id")
- "start": start character offset in the original text
- "end": end character offset in the original text
- "confidence": confidence score between 0.0 and 1.0
- "level": sensitivity level — one of "normal", "sensitive", "highly_sensitive", "restricted"

If no PII is found, return an empty array: []

Respond ONLY with the JSON array, no other text.

Text to analyze:
{text}"#
        )
    }
}

/// A single PII match from the LLM response.
#[derive(Debug, serde::Deserialize)]
struct LlmPiiMatch {
    rule_name: String,
    start: usize,
    end: usize,
    #[serde(default = "default_llm_confidence")]
    confidence: f64,
    #[serde(default)]
    level: String,
}

fn default_llm_confidence() -> f64 {
    0.80
}

fn parse_level(s: &str) -> SensitivityLevel {
    match s.to_lowercase().as_str() {
        "critical" | "restricted" => SensitivityLevel::Critical,
        "highly_sensitive" | "highly sensitive" => SensitivityLevel::HighlySensitive,
        "sensitive" => SensitivityLevel::Sensitive,
        _ => SensitivityLevel::Sensitive, // Default to sensitive for any PII
    }
}

#[async_trait]
impl ClassifierBackend for LlmBackend {
    async fn classify(&self, text: &str) -> Vec<PiiMatch> {
        let prompt = Self::classification_prompt(text);

        let response = match self.llm.call(&prompt).await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(error = %e, "LLM classifier call failed, returning empty");
                return Vec::new();
            }
        };

        // Extract JSON array from response (LLM may wrap in markdown code blocks)
        let json_str = response
            .trim()
            .trim_start_matches("```json")
            .trim_start_matches("```")
            .trim_end_matches("```")
            .trim();

        match serde_json::from_str::<Vec<LlmPiiMatch>>(json_str) {
            Ok(matches) => matches
                .into_iter()
                .filter(|m| m.start < text.len() && m.end <= text.len() && m.start < m.end)
                .map(|m| PiiMatch {
                    rule_name: format!("llm:{}", m.rule_name),
                    level: parse_level(&m.level),
                    start: m.start,
                    end: m.end,
                    confidence: m.confidence.clamp(0.0, 1.0),
                    backend: "llm".to_string(),
                })
                .collect(),
            Err(e) => {
                tracing::warn!(
                    error = %e,
                    response = %json_str,
                    "Failed to parse LLM classifier response"
                );
                Vec::new()
            }
        }
    }

    fn confidence_floor(&self) -> f64 {
        0.70
    }

    fn name(&self) -> &str {
        "llm"
    }
}

/// Deduplicate overlapping matches by keeping the highest-confidence one.
///
/// Two matches overlap if their byte ranges intersect. When they do,
/// the match with higher confidence is kept.
fn deduplicate_matches(mut matches: Vec<PiiMatch>) -> Vec<PiiMatch> {
    if matches.len() <= 1 {
        return matches;
    }

    // Sort by start position, then by confidence descending
    matches.sort_by(|a, b| {
        a.start.cmp(&b.start).then(
            b.confidence
                .partial_cmp(&a.confidence)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });

    let mut result: Vec<PiiMatch> = Vec::new();

    for m in matches {
        // Check if this match overlaps with the last kept match
        if let Some(last) = result.last() {
            if m.start < last.end {
                // Overlapping — keep the one with higher confidence
                if m.confidence > last.confidence {
                    result.pop();
                    result.push(m);
                }
                // Otherwise skip this match (lower confidence)
                continue;
            }
        }
        result.push(m);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::default_classification_rules;

    #[tokio::test]
    async fn test_regex_backend() {
        let backend =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();
        let matches = backend.classify("My card is 4111-1111-1111-1111").await;
        assert!(!matches.is_empty());
        assert_eq!(matches[0].backend, "regex");
        assert_eq!(matches[0].rule_name, "credit_card");
    }

    #[tokio::test]
    async fn test_regex_backend_no_match() {
        let backend =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();
        let matches = backend.classify("Hello, how are you?").await;
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_semantic_backend() {
        let analyzer = crate::privacy::SemanticAnalyzer::new();
        let backend = SemanticBackend::new(analyzer);
        let matches = backend.classify("my password is hunter2").await;
        assert!(!matches.is_empty());
        assert_eq!(matches[0].backend, "semantic");
    }

    #[tokio::test]
    async fn test_composite_classifier_merges() {
        let regex =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();
        let semantic = SemanticBackend::new(crate::privacy::SemanticAnalyzer::new());

        let composite = CompositeClassifier::new(vec![Box::new(regex), Box::new(semantic)]);

        let result = composite.classify("My SSN is 123-45-6789").await;
        assert!(!result.matches.is_empty());
        assert!(result.requires_tee);
    }

    #[tokio::test]
    async fn test_composite_classifier_normal_text() {
        let regex =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();
        let composite = CompositeClassifier::new(vec![Box::new(regex)]);

        let result = composite.classify("Hello world").await;
        assert!(result.matches.is_empty());
        assert_eq!(result.level, SensitivityLevel::Normal);
        assert!(!result.requires_tee);
    }

    #[test]
    fn test_deduplicate_no_overlap() {
        let matches = vec![
            PiiMatch {
                rule_name: "a".into(),
                level: SensitivityLevel::Sensitive,
                start: 0,
                end: 5,
                confidence: 0.9,
                backend: "regex".into(),
            },
            PiiMatch {
                rule_name: "b".into(),
                level: SensitivityLevel::Sensitive,
                start: 10,
                end: 15,
                confidence: 0.8,
                backend: "semantic".into(),
            },
        ];
        let result = deduplicate_matches(matches);
        assert_eq!(result.len(), 2);
    }

    #[test]
    fn test_deduplicate_overlap_keeps_higher_confidence() {
        let matches = vec![
            PiiMatch {
                rule_name: "regex_ssn".into(),
                level: SensitivityLevel::HighlySensitive,
                start: 10,
                end: 21,
                confidence: 0.95,
                backend: "regex".into(),
            },
            PiiMatch {
                rule_name: "semantic_ssn".into(),
                level: SensitivityLevel::Sensitive,
                start: 10,
                end: 21,
                confidence: 0.70,
                backend: "semantic".into(),
            },
        ];
        let result = deduplicate_matches(matches);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].backend, "regex");
    }

    #[test]
    fn test_deduplicate_empty() {
        let result = deduplicate_matches(vec![]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_deduplicate_single() {
        let matches = vec![PiiMatch {
            rule_name: "a".into(),
            level: SensitivityLevel::Sensitive,
            start: 0,
            end: 5,
            confidence: 0.9,
            backend: "regex".into(),
        }];
        let result = deduplicate_matches(matches);
        assert_eq!(result.len(), 1);
    }

    #[test]
    fn test_confidence_floor() {
        let regex =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();
        assert!(regex.confidence_floor() > 0.8);

        let semantic = SemanticBackend::new(crate::privacy::SemanticAnalyzer::new());
        assert!(semantic.confidence_floor() < regex.confidence_floor());
    }

    #[test]
    fn test_backend_names() {
        let regex =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();
        assert_eq!(regex.name(), "regex");

        let semantic = SemanticBackend::new(crate::privacy::SemanticAnalyzer::new());
        assert_eq!(semantic.name(), "semantic");
    }

    // --- LlmBackend tests ---

    /// Mock LLM that returns a canned JSON response
    struct MockLlm {
        response: String,
    }

    #[async_trait]
    impl LlmClassifierFn for MockLlm {
        async fn call(&self, _prompt: &str) -> Result<String, String> {
            Ok(self.response.clone())
        }
    }

    /// Mock LLM that always fails
    struct FailingLlm;

    #[async_trait]
    impl LlmClassifierFn for FailingLlm {
        async fn call(&self, _prompt: &str) -> Result<String, String> {
            Err("LLM unavailable".to_string())
        }
    }

    #[tokio::test]
    async fn test_llm_backend_parses_response() {
        let response = r#"[
            {"rule_name": "email", "start": 14, "end": 30, "confidence": 0.95, "level": "sensitive"},
            {"rule_name": "phone", "start": 42, "end": 54, "confidence": 0.88, "level": "sensitive"}
        ]"#;
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: response.to_string(),
        }));

        let text = "Contact me at user@example.com or call 555-123-4567 please";
        let matches = backend.classify(text).await;
        assert_eq!(matches.len(), 2);
        assert_eq!(matches[0].rule_name, "llm:email");
        assert_eq!(matches[0].backend, "llm");
        assert_eq!(matches[1].rule_name, "llm:phone");
    }

    #[tokio::test]
    async fn test_llm_backend_empty_response() {
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: "[]".to_string(),
        }));
        let matches = backend.classify("Hello world").await;
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_llm_backend_handles_markdown_code_block() {
        let response = "```json\n[{\"rule_name\": \"ssn\", \"start\": 0, \"end\": 11, \"confidence\": 0.9, \"level\": \"highly_sensitive\"}]\n```";
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: response.to_string(),
        }));
        let matches = backend.classify("123-45-6789").await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].level, SensitivityLevel::HighlySensitive);
    }

    #[tokio::test]
    async fn test_llm_backend_handles_failure() {
        let backend = LlmBackend::new(Box::new(FailingLlm));
        let matches = backend.classify("test").await;
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_llm_backend_handles_invalid_json() {
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: "not json at all".to_string(),
        }));
        let matches = backend.classify("test").await;
        assert!(matches.is_empty());
    }

    #[tokio::test]
    async fn test_llm_backend_filters_invalid_offsets() {
        let response = r#"[
            {"rule_name": "email", "start": 0, "end": 5, "confidence": 0.9, "level": "sensitive"},
            {"rule_name": "bad", "start": 100, "end": 200, "confidence": 0.9, "level": "sensitive"}
        ]"#;
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: response.to_string(),
        }));
        let matches = backend.classify("hello").await;
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].rule_name, "llm:email");
    }

    #[tokio::test]
    async fn test_llm_backend_clamps_confidence() {
        let response = r#"[{"rule_name": "x", "start": 0, "end": 3, "confidence": 1.5, "level": "sensitive"}]"#;
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: response.to_string(),
        }));
        let matches = backend.classify("abc").await;
        assert_eq!(matches[0].confidence, 1.0);
    }

    #[test]
    fn test_llm_backend_name_and_confidence() {
        let backend = LlmBackend::new(Box::new(MockLlm {
            response: "[]".to_string(),
        }));
        assert_eq!(backend.name(), "llm");
        assert!(backend.confidence_floor() >= 0.5);
    }

    #[tokio::test]
    async fn test_composite_with_llm_backend() {
        let response = r#"[{"rule_name": "address", "start": 10, "end": 25, "confidence": 0.85, "level": "sensitive"}]"#;
        let llm = LlmBackend::new(Box::new(MockLlm {
            response: response.to_string(),
        }));
        let regex =
            RegexBackend::new(default_classification_rules(), SensitivityLevel::Normal).unwrap();

        let composite = CompositeClassifier::new(vec![Box::new(regex), Box::new(llm)]);
        let result = composite
            .classify("I live at 123 Main Street downtown")
            .await;
        // LLM should find the address that regex missed
        assert!(!result.matches.is_empty());
        assert!(result.matches.iter().any(|m| m.backend == "llm"));
    }

    #[test]
    fn test_parse_level() {
        assert_eq!(parse_level("critical"), SensitivityLevel::Critical);
        assert_eq!(parse_level("restricted"), SensitivityLevel::Critical);
        assert_eq!(
            parse_level("highly_sensitive"),
            SensitivityLevel::HighlySensitive
        );
        assert_eq!(parse_level("sensitive"), SensitivityLevel::Sensitive);
        assert_eq!(parse_level("normal"), SensitivityLevel::Sensitive); // PII defaults to sensitive
        assert_eq!(parse_level("unknown"), SensitivityLevel::Sensitive);
    }
}
