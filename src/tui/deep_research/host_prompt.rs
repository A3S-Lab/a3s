//! TUI DeepResearch scope parsing and persistent goal projection.

use super::*;

pub(super) fn deep_research_default_evidence_scope() -> DeepResearchEvidenceScope {
    // Scope is a typed caller decision. Free-form query text is never routed
    // through a language-specific phrase or keyword table.
    DeepResearchEvidenceScope::WebAndWorkspace
}

pub(super) fn parse_deep_research_tui_query(
    raw_query: &str,
) -> (String, DeepResearchEvidenceScope) {
    let raw_query = raw_query.trim();
    let mut parts = raw_query.splitn(2, char::is_whitespace);
    let first = parts.next().unwrap_or_default();
    let remainder = parts.next().unwrap_or_default().trim();
    match first {
        "--local-only" | "--offline" => {
            (remainder.to_string(), DeepResearchEvidenceScope::LocalOnly)
        }
        "--web" => (
            remainder.to_string(),
            DeepResearchEvidenceScope::WebAndWorkspace,
        ),
        _ => (
            raw_query.to_string(),
            deep_research_default_evidence_scope(),
        ),
    }
}

pub(super) fn deep_research_input_scope_hint() -> &'static str {
    "◇ deep research · --web | --local-only"
}

/// The persistent `/goal` north-star displayed while one DeepResearch run is
/// active.
pub(super) fn deep_research_goal(query: &str) -> String {
    format!("Deep research — deliver a comprehensive, well-cited report answering: {query}")
}
