//! Output sanitizer for AI agent responses
//!
//! Scans AI-generated output for tainted data (including encoded variants)
//! and auto-redacts before sending to the user. Generates audit events
//! for any blocked leakage attempts.
//!
//! **Threat model**: Defends against A2 (compromised AI model) at AS-3 (model interaction).
//! See `docs/threat-model.md` §4 AS-3, §5.

use super::taint::{TaintMatch, TaintRegistry};
use super::traits::Sanitizer;
use crate::audit::{AuditEvent, AuditSeverity, LeakageVector};

/// Result of sanitizing output
#[derive(Debug, Clone)]
pub struct SanitizeResult {
    /// The sanitized (safe) output text
    pub sanitized_text: String,
    /// Whether any tainted data was found and redacted
    pub was_redacted: bool,
    /// Number of redactions performed
    pub redaction_count: usize,
    /// Audit events generated
    pub audit_events: Vec<AuditEvent>,
    /// Details of what was found
    pub matches: Vec<TaintMatch>,
}

/// Output sanitizer that checks AI responses for data leakage
pub struct OutputSanitizer;

impl OutputSanitizer {
    /// Sanitize AI output text against the taint registry.
    ///
    /// Returns the sanitized text with all tainted data redacted,
    /// plus audit events for logging.
    pub fn sanitize(registry: &TaintRegistry, output: &str, session_id: &str) -> SanitizeResult {
        let matches = registry.detect(output);

        if matches.is_empty() {
            return SanitizeResult {
                sanitized_text: output.to_string(),
                was_redacted: false,
                redaction_count: 0,
                audit_events: vec![],
                matches: vec![],
            };
        }

        let sanitized_text = registry.redact(output);
        let redaction_count = matches.len();

        // Generate audit events for each match, including taint labels
        let audit_events: Vec<AuditEvent> = matches
            .iter()
            .map(|m| {
                AuditEvent::with_taint_labels(
                    session_id.to_string(),
                    AuditSeverity::High,
                    LeakageVector::OutputChannel,
                    format!(
                        "Tainted data detected in output: taint_id={}, type={:?}, variant='{}'",
                        m.taint_id,
                        m.taint_type,
                        if m.matched_variant.len() > 20 {
                            format!("{}...", &m.matched_variant[..20])
                        } else {
                            m.matched_variant.clone()
                        }
                    ),
                    vec![m.taint_id.clone()],
                )
            })
            .collect();

        SanitizeResult {
            sanitized_text,
            was_redacted: true,
            redaction_count,
            audit_events,
            matches,
        }
    }

    /// Quick check: does the output contain any tainted data?
    pub fn contains_leakage(registry: &TaintRegistry, output: &str) -> bool {
        registry.contains_tainted(output)
    }
}

impl Sanitizer for OutputSanitizer {
    fn sanitize(&self, registry: &TaintRegistry, output: &str, session_id: &str) -> SanitizeResult {
        OutputSanitizer::sanitize(registry, output, session_id)
    }

    fn contains_leakage(&self, registry: &TaintRegistry, output: &str) -> bool {
        OutputSanitizer::contains_leakage(registry, output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::guard::taint::TaintType;

    fn setup_registry() -> TaintRegistry {
        let mut registry = TaintRegistry::new();
        registry.register("4111-1111-1111-1111", TaintType::CreditCard);
        registry.register("sk-abc123xyz", TaintType::ApiKey);
        registry
    }

    #[test]
    fn test_sanitize_clean_output() {
        let registry = setup_registry();
        let result =
            OutputSanitizer::sanitize(&registry, "Payment completed successfully.", "sess-1");
        assert!(!result.was_redacted);
        assert_eq!(result.redaction_count, 0);
        assert!(result.audit_events.is_empty());
        assert_eq!(result.sanitized_text, "Payment completed successfully.");
    }

    #[test]
    fn test_sanitize_detects_exact_match() {
        let registry = setup_registry();
        let result = OutputSanitizer::sanitize(
            &registry,
            "Your card 4111-1111-1111-1111 was charged.",
            "sess-1",
        );
        assert!(result.was_redacted);
        assert!(result.redaction_count >= 1);
        assert!(!result.sanitized_text.contains("4111-1111-1111-1111"));
        assert!(!result.audit_events.is_empty());
    }

    #[test]
    fn test_sanitize_detects_base64_variant() {
        let registry = setup_registry();
        let encoded =
            base64::Engine::encode(&base64::engine::general_purpose::STANDARD, "sk-abc123xyz");
        let output = format!("Here's the key: {}", encoded);
        let result = OutputSanitizer::sanitize(&registry, &output, "sess-1");
        assert!(result.was_redacted);
    }

    #[test]
    fn test_sanitize_generates_audit_events() {
        let registry = setup_registry();
        let result = OutputSanitizer::sanitize(&registry, "Card: 4111-1111-1111-1111", "sess-1");
        assert!(!result.audit_events.is_empty());
        assert_eq!(result.audit_events[0].session_id, "sess-1");
        assert_eq!(result.audit_events[0].severity, AuditSeverity::High);
    }

    #[test]
    fn test_contains_leakage() {
        let registry = setup_registry();
        assert!(!OutputSanitizer::contains_leakage(&registry, "Hello world"));
        assert!(OutputSanitizer::contains_leakage(
            &registry,
            "Card: 4111-1111-1111-1111"
        ));
    }

    #[test]
    fn test_sanitize_multiple_leaks() {
        let registry = setup_registry();
        let result = OutputSanitizer::sanitize(
            &registry,
            "Card 4111-1111-1111-1111 and key sk-abc123xyz",
            "sess-1",
        );
        assert!(result.was_redacted);
        assert!(result.redaction_count >= 2);
        assert!(!result.sanitized_text.contains("4111-1111-1111-1111"));
        assert!(!result.sanitized_text.contains("sk-abc123xyz"));
    }
}
