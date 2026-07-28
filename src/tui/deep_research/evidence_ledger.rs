//! Typed, bounded evidence accepted from a DeepResearch workflow.

use a3s::research::{EvidenceQualityRequirements, SourceCoverageBinding, SourceEvidenceRole};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcceptedSource {
    pub(crate) id: String,
    pub(crate) anchor: String,
    pub(crate) title: Option<String>,
    pub(crate) date: Option<String>,
    pub(crate) reliability: Option<String>,
    #[serde(default)]
    pub(crate) quote_or_fact: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) evidence_excerpts: Vec<AcceptedSourceExcerpt>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcceptedSourceExcerpt {
    pub(crate) id: String,
    pub(crate) focus: String,
    pub(crate) quote_or_fact: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcceptedClaim {
    pub(crate) id: String,
    pub(crate) text: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct AcceptedEvidence {
    pub(crate) id: String,
    pub(crate) summary: String,
    pub(crate) confidence: Option<String>,
    pub(crate) sources: Vec<AcceptedSource>,
    pub(crate) claims: Vec<AcceptedClaim>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) source_coverage: Vec<SourceCoverageBinding>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) relevant_obligation_ids: Vec<String>,
    pub(crate) contradictions: Vec<String>,
    pub(crate) gaps: Vec<String>,
}

#[derive(Clone, Copy, Debug)]
struct ObligationCoverageContract {
    completion_criterion_count: usize,
    evidence_requirements: EvidenceQualityRequirements,
}

pub(crate) fn accepted_evidence_ledger(
    workflow_output: &str,
    workflow_metadata: Option<&serde_json::Value>,
) -> Vec<AcceptedEvidence> {
    let workflow = serde_json::from_str::<serde_json::Value>(workflow_output).ok();
    let coverage_contract = workflow
        .as_ref()
        .and_then(coverage_contract)
        .or_else(|| workflow_metadata.and_then(coverage_contract))
        .unwrap_or_default();
    let mut raw = workflow
        .as_ref()
        .map(super::deep_research_collect_structured_evidence_for_ledger)
        .unwrap_or_default();
    if let Some(metadata) = workflow_metadata {
        raw.extend(super::deep_research_collect_structured_evidence_for_ledger(
            metadata,
        ));
    }
    let mut seen = HashSet::new();
    raw.into_iter()
        .filter_map(|value| normalize_evidence(value, &coverage_contract))
        .filter(|evidence| seen.insert(evidence.id.clone()))
        .take(64)
        .collect()
}

#[cfg(test)]
pub(crate) fn synthesis_payload(evidence: &[AcceptedEvidence]) -> String {
    synthesis_payload_with_context(evidence, "")
}

#[cfg(test)]
pub(crate) fn synthesis_payload_with_context(
    evidence: &[AcceptedEvidence],
    workflow_output: &str,
) -> String {
    let items = evidence
        .iter()
        .map(|item| {
            let source_fact_count = item
                .sources
                .iter()
                .map(|source| {
                    if source.evidence_excerpts.is_empty() {
                        usize::from(source.quote_or_fact.is_some())
                    } else {
                        source.evidence_excerpts.len()
                    }
                })
                .sum::<usize>();
            let summary = format!(
                "Retained {} source-backed fact excerpt(s).",
                source_fact_count
            );
            serde_json::json!({
                "evidence_id": item.id,
                "summary": summary,
                "claims": item.claims,
                "sources": item.sources.iter().map(|source| serde_json::json!({
                    "source_id": source.id,
                    "title": source.title,
                    "url_or_path": source.anchor,
                    "date": source.date,
                    "reliability": source.reliability,
                    "quote_or_fact": source.quote_or_fact,
                    "evidence_excerpts": source.evidence_excerpts,
                })).collect::<Vec<_>>(),
                "confidence": item.confidence,
                "source_coverage": item.source_coverage,
                "relevant_obligation_ids": item.relevant_obligation_ids,
                "contradictions": item.contradictions,
                "gaps": item.gaps,
            })
        })
        .collect::<Vec<_>>();
    let mut payload = serde_json::json!({
        "collection_status": if items.is_empty() { "degraded" } else { "completed" },
        "evidence_items": items,
    });
    if let Some(context) = synthesis_report_context(workflow_output) {
        payload["report_context"] = context;
    }
    serde_json::to_string(&payload).unwrap_or_else(|_| {
        "{\"collection_status\":\"degraded\",\"evidence_items\":[]}".to_string()
    })
}

#[cfg(test)]
fn synthesis_report_context(workflow_output: &str) -> Option<serde_json::Value> {
    let workflow = serde_json::from_str::<serde_json::Value>(workflow_output.trim()).ok()?;
    let plan = workflow.get("plan").and_then(serde_json::Value::as_object);
    let inquiry = match super::validated_inquiry_projection(&workflow) {
        Ok(super::ValidatedInquiryProjection::Inquiry { state, .. }) => Some(state),
        Ok(super::ValidatedInquiryProjection::LegacyCheckedLoop) | Err(_) => None,
    };
    // An event-sourced inquiry is the authoritative report contract. Preserve
    // only the bounded verification facts for legacy/degraded workflows so a
    // stale legacy checker packet cannot widen an inquiry projection.
    let verification = if inquiry.is_none() {
        workflow
            .get("verification")
            .and_then(serde_json::Value::as_object)
    } else {
        None
    };
    if plan.is_none() && inquiry.is_none() && verification.is_none() {
        return None;
    }

    let mut context = serde_json::Map::new();
    if let Some(plan) = plan {
        let mut plan_context = serde_json::Map::new();
        if let Some(value) = plan
            .get("report_title")
            .and_then(serde_json::Value::as_str)
            .map(|value| bounded_string(value, 300))
        {
            plan_context.insert("report_title".to_string(), serde_json::Value::String(value));
        }
        let stop_conditions = bounded_string_array(plan.get("stop_conditions"), 6, 500);
        if !stop_conditions.is_empty() {
            plan_context.insert(
                "stop_conditions".to_string(),
                serde_json::json!(stop_conditions),
            );
        }
        let tracks = bounded_plan_tracks(plan.get("tracks"), 6, 500);
        if !tracks.is_empty() {
            plan_context.insert("tracks".to_string(), serde_json::json!(tracks));
        }
        if !plan_context.is_empty() {
            context.insert("plan".to_string(), serde_json::Value::Object(plan_context));
        }
    }
    if let Some(state) = inquiry {
        let questions = state
            .questions
            .iter()
            .take(32)
            .map(|question| {
                serde_json::json!({
                    "id": question.id,
                    "obligation_ids": question.obligation_ids,
                    "completion_criterion_indexes": question.completion_criterion_indexes,
                    "material": question.material,
                    "prompt": bounded_string(&question.prompt, 500),
                    "status": question.status,
                    "answer": question.answer.as_deref().map(|answer| bounded_string(answer, 2_000)),
                    "bound_reason": question.bound_reason.as_deref().map(|reason| bounded_string(reason, 1_000)),
                    "evidence_ids": question.evidence_ids,
                })
            })
            .collect::<Vec<_>>();
        context.insert(
            "inquiry".to_string(),
            serde_json::json!({
                "contract_outcome": a3s::research::research_contract_outcome(&state),
                "obligations": state.obligations,
                "stop_conditions": state.stop_conditions,
                "contract_assessment": state.contract_assessment,
                "questions": questions,
            }),
        );
    }
    if let Some(verification) = verification {
        let mut verification_context = serde_json::Map::new();
        for key in ["status", "checker_completed", "prior_checker_retained"] {
            if let Some(value) = verification.get(key) {
                verification_context.insert(key.to_string(), value.clone());
            }
        }
        if !verification_context.is_empty() {
            context.insert(
                "verification".to_string(),
                serde_json::Value::Object(verification_context),
            );
        }
    }
    (!context.is_empty()).then_some(serde_json::Value::Object(context))
}

#[cfg(test)]
fn bounded_string(value: &str, limit: usize) -> String {
    value.trim().chars().take(limit).collect()
}

#[cfg(test)]
fn bounded_string_array(
    value: Option<&serde_json::Value>,
    item_limit: usize,
    char_limit: usize,
) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .take(item_limit)
        .map(|value| bounded_string(value, char_limit))
        .collect()
}

#[cfg(test)]
fn bounded_plan_tracks(
    value: Option<&serde_json::Value>,
    item_limit: usize,
    char_limit: usize,
) -> Vec<serde_json::Value> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|track| match track {
            serde_json::Value::String(title) if !title.trim().is_empty() => {
                Some(serde_json::json!({"title": bounded_string(title, char_limit)}))
            }
            serde_json::Value::Object(track) => {
                let id = track.get("id").and_then(serde_json::Value::as_str)?;
                let title = track.get("title").and_then(serde_json::Value::as_str)?;
                let focus = track.get("focus").and_then(serde_json::Value::as_str)?;
                Some(serde_json::json!({
                    "id": bounded_string(id, 160),
                    "title": bounded_string(title, char_limit),
                    "focus": bounded_string(focus, 1_200),
                    "material": track
                        .get("material")
                        .and_then(serde_json::Value::as_bool)
                        .unwrap_or(false),
                }))
            }
            _ => None,
        })
        .take(item_limit)
        .collect()
}

fn normalize_evidence(
    value: serde_json::Value,
    coverage_contract: &BTreeMap<String, ObligationCoverageContract>,
) -> Option<AcceptedEvidence> {
    let summary = value.get("summary")?.as_str()?.trim();
    if summary.is_empty() {
        return None;
    }
    let mut source_ids = HashSet::new();
    let mut source_identity_map = HashMap::new();
    let mut sources = Vec::new();
    for source in value.get("sources")?.as_array()?.iter() {
        let Some(anchor) = super::deep_research_traceable_source_anchor(source) else {
            continue;
        };
        let id = stable_id("source", &anchor);
        if !source_ids.insert(id.clone()) {
            continue;
        }
        if let Some(source_identity) = source.get("source_id") {
            let source_identity = exact_identifier(source_identity)?;
            if source_identity_map
                .insert(source_identity.to_string(), id.clone())
                .is_some()
            {
                return None;
            }
        }
        let reliability = string_field(source, "reliability");
        let evidence_excerpts = accepted_source_excerpts(source, &id);
        sources.push(AcceptedSource {
            id,
            anchor,
            title: string_field(source, "title"),
            date: string_field(source, "date"),
            reliability,
            quote_or_fact: string_field(source, "quote_or_fact")
                .or_else(|| string_field(source, "excerpt"))
                .or_else(|| string_field(source, "fact")),
            evidence_excerpts,
        });
    }
    if sources.is_empty() {
        return None;
    }
    let source_coverage = normalize_source_coverage(
        value.get("source_coverage"),
        &source_identity_map,
        coverage_contract,
    )?;
    let relevant_obligation_ids = normalize_relevant_obligation_ids(
        value.get("relevant_obligation_ids"),
        &source_coverage,
        coverage_contract,
    )?;
    let claims = string_array(value.get("key_evidence"), 32)
        .into_iter()
        .map(|text| AcceptedClaim {
            id: stable_id("claim", &text),
            text,
        })
        .collect::<Vec<_>>();
    let evidence_key = format!(
        "{}|{}",
        summary,
        sources
            .iter()
            .map(|source| source.id.as_str())
            .collect::<Vec<_>>()
            .join("|")
    );
    Some(AcceptedEvidence {
        id: stable_id("evidence", &evidence_key),
        summary: summary.to_string(),
        confidence: string_field(&value, "confidence"),
        sources,
        claims,
        source_coverage,
        relevant_obligation_ids,
        contradictions: string_array(value.get("contradictions"), 16),
        gaps: string_array(value.get("gaps"), 16),
    })
}

fn normalize_relevant_obligation_ids(
    value: Option<&serde_json::Value>,
    source_coverage: &[SourceCoverageBinding],
    coverage_contract: &BTreeMap<String, ObligationCoverageContract>,
) -> Option<Vec<String>> {
    let mut obligation_ids = match value {
        Some(value) => value
            .as_array()?
            .iter()
            .map(|value| exact_identifier(value).map(str::to_string))
            .collect::<Option<Vec<_>>>()?,
        None => source_coverage
            .iter()
            .map(|binding| binding.obligation_id.clone())
            .collect::<Vec<_>>(),
    };
    if obligation_ids.len() > 16 || obligation_ids.len() > coverage_contract.len() {
        return None;
    }
    let original_count = obligation_ids.len();
    obligation_ids.sort();
    obligation_ids.dedup();
    if obligation_ids.len() != original_count
        || obligation_ids
            .iter()
            .any(|obligation_id| !coverage_contract.contains_key(obligation_id))
    {
        return None;
    }
    Some(obligation_ids)
}

fn coverage_contract(
    root: &serde_json::Value,
) -> Option<BTreeMap<String, ObligationCoverageContract>> {
    let plan = root
        .get("plan")
        .or_else(|| root.pointer("/result/plan"))
        .or_else(|| root.pointer("/dynamic_workflow/snapshot/output/plan"))?;
    let tracks = plan.get("tracks")?.as_array()?;
    let mut contract = BTreeMap::new();
    for track in tracks {
        let id = exact_identifier(track.get("id")?)?.to_string();
        let completion_criterion_count = track.get("completion_criteria")?.as_array()?.len();
        if completion_criterion_count == 0 {
            return None;
        }
        let requirements = track.get("evidence_requirements")?;
        let evidence_requirements = EvidenceQualityRequirements {
            primary_source_required: requirements.get("primary_source_required")?.as_bool()?,
            independent_corroboration_required: requirements
                .get("independent_corroboration_required")?
                .as_bool()?,
        };
        if contract
            .insert(
                id,
                ObligationCoverageContract {
                    completion_criterion_count,
                    evidence_requirements,
                },
            )
            .is_some()
        {
            return None;
        }
    }
    (!contract.is_empty()).then_some(contract)
}

fn normalize_source_coverage(
    value: Option<&serde_json::Value>,
    source_identity_map: &HashMap<String, String>,
    coverage_contract: &BTreeMap<String, ObligationCoverageContract>,
) -> Option<Vec<SourceCoverageBinding>> {
    let Some(value) = value else {
        return Some(Vec::new());
    };
    let bindings = value.as_array()?;
    if bindings.len() > 64 {
        return None;
    }
    let mut normalized = Vec::with_capacity(bindings.len());
    let mut edges = HashSet::new();
    for binding in bindings {
        let mut binding = serde_json::from_value::<SourceCoverageBinding>(binding.clone()).ok()?;
        let raw_source_id = exact_identifier_str(&binding.source_id)?;
        let stable_source_id = source_identity_map.get(raw_source_id)?.clone();
        let obligation_id = exact_identifier_str(&binding.obligation_id)?;
        let contract = coverage_contract.get(obligation_id)?;
        if binding.completion_criterion_indexes.is_empty()
            || binding.completion_criterion_indexes.len() > contract.completion_criterion_count
        {
            return None;
        }
        let criterion_index_count = binding.completion_criterion_indexes.len();
        binding.completion_criterion_indexes.sort_unstable();
        binding.completion_criterion_indexes.dedup();
        if binding.completion_criterion_indexes.len() != criterion_index_count
            || binding
                .completion_criterion_indexes
                .iter()
                .any(|index| *index >= contract.completion_criterion_count)
        {
            return None;
        }
        if binding.roles.is_empty() || binding.roles.len() > 3 {
            return None;
        }
        let role_count = binding.roles.len();
        binding.roles.sort_unstable();
        binding.roles.dedup();
        if binding.roles.len() != role_count
            || !binding.roles.contains(&SourceEvidenceRole::Supporting)
            || (binding.roles.contains(&SourceEvidenceRole::Primary)
                && !contract.evidence_requirements.primary_source_required)
            || (binding.roles.contains(&SourceEvidenceRole::Independent)
                && !contract
                    .evidence_requirements
                    .independent_corroboration_required)
        {
            return None;
        }
        if !edges.insert((stable_source_id.clone(), obligation_id.to_string())) {
            return None;
        }
        binding.source_id = stable_source_id;
        binding.obligation_id = obligation_id.to_string();
        normalized.push(binding);
    }
    normalized.sort_by(|left, right| {
        (&left.source_id, &left.obligation_id).cmp(&(&right.source_id, &right.obligation_id))
    });
    Some(normalized)
}

fn exact_identifier(value: &serde_json::Value) -> Option<&str> {
    exact_identifier_str(value.as_str()?)
}

fn exact_identifier_str(value: &str) -> Option<&str> {
    if value.is_empty() || value.trim() != value || value.chars().count() > 160 {
        return None;
    }
    Some(value)
}

fn accepted_source_excerpts(
    source: &serde_json::Value,
    source_id: &str,
) -> Vec<AcceptedSourceExcerpt> {
    let mut seen = HashSet::new();
    source
        .get("evidence_excerpts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|excerpt| {
            let focus = string_field(excerpt, "focus")?;
            let quote_or_fact = string_field(excerpt, "quote_or_fact")
                .or_else(|| string_field(excerpt, "excerpt"))
                .or_else(|| string_field(excerpt, "fact"))?;
            let identity = format!("{}|{}|{}", source_id, focus, quote_or_fact);
            let id = stable_id("excerpt", &identity);
            seen.insert(id.clone()).then_some(AcceptedSourceExcerpt {
                id,
                focus,
                quote_or_fact,
            })
        })
        .take(4)
        .collect()
}

fn string_field(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(|text| text.chars().take(2_000).collect())
}

fn string_array(value: Option<&serde_json::Value>, limit: usize) -> Vec<String> {
    value
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|value| value.as_str())
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .take(limit)
        .map(|text| text.chars().take(2_000).collect())
        .collect()
}

fn stable_id(kind: &str, value: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(value.as_bytes());
    format!("{kind}:{:x}", digest.finalize())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn evidence_ledger_preserves_fact_beyond_summary_prefix() {
        let prefix = "Background context without the decisive result. ".repeat(18);
        let fact = format!(
            "{prefix}The evaluated system improved the benchmark by 25% and broadened coverage by 10%."
        );
        assert!(fact.chars().count() > 700);
        let output = serde_json::json!({
            "structured": {
                "summary": "The benchmark result is documented.",
                "sources": [{
                    "title": "Primary evaluation",
                    "url_or_path": "https://example.org/evaluation",
                    "quote_or_fact": fact,
                    "reliability": "Primary evaluation record"
                }],
                "key_evidence": [fact],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });

        let ledger = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(ledger.len(), 1);
        assert!(ledger[0].claims[0].text.contains("25%"));
        assert!(ledger[0].sources[0]
            .quote_or_fact
            .as_deref()
            .is_some_and(|quote| quote.contains("10%")));
    }

    #[test]
    fn claim_identity_uses_the_full_bounded_claim_without_prefix_aliasing() {
        let shared_prefix = "x".repeat(400);
        let output = serde_json::json!({
            "structured": {
                "summary": "Two distinct claims share a long prefix.",
                "sources": [{
                    "title": "Primary evaluation",
                    "url_or_path": "https://example.org/evaluation",
                    "quote_or_fact": "Both claims are retained.",
                    "reliability": "Primary evaluation record"
                }],
                "key_evidence": [
                    format!("{shared_prefix} first ending"),
                    format!("{shared_prefix} second ending")
                ],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });

        let ledger = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].claims.len(), 2);
        assert_ne!(ledger[0].claims[0].id, ledger[0].claims[1].id);
    }

    #[test]
    fn structured_evidence_words_are_preserved_without_content_classification() {
        let literal = "The source documents a3s://tool-output as protocol subject matter.";
        let output = serde_json::json!({
            "structured": {
                "summary": "A protocol reference is documented.",
                "sources": [{
                    "title": "Protocol specification",
                    "url_or_path": "https://example.org/protocol",
                    "quote_or_fact": literal,
                    "reliability": "Specification"
                }],
                "key_evidence": [literal],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });

        let ledger = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(ledger[0].claims[0].text, literal);
        assert_eq!(ledger[0].sources[0].quote_or_fact.as_deref(), Some(literal));
    }

    #[test]
    fn evidence_ledger_preserves_bounded_focused_excerpts_under_one_source_identity() {
        let oversized_tail = " supporting detail".repeat(100);
        let output = serde_json::json!({
            "structured": {
                "summary": "One paper supports several distinct research tracks.",
                "sources": [{
                    "title": "Project Quasar paper",
                    "url_or_path": "https://papers.example/archive/1234",
                    "quote_or_fact": "The paper documents the primary mechanism.",
                    "reliability": "Primary paper",
                    "evidence_excerpts": [{
                        "focus": "Method",
                        "quote_or_fact": format!(
                            "The dual-stage outline-and-retrieve mechanism is documented.{oversized_tail}"
                        )
                    }, {
                        "focus": "Evaluation",
                        "quote_or_fact": format!(
                            "The ablation reduced citation completeness by 17 percentage points.{oversized_tail}"
                        )
                    }, {
                        "focus": "Limitations",
                        "quote_or_fact": format!(
                            "The evaluation covered only English-language technology topics.{oversized_tail}"
                        )
                    }, {
                        "focus": "Additional analysis",
                        "quote_or_fact": format!(
                            "The paper reports a bounded additional analysis.{oversized_tail}"
                        )
                    }, {
                        "focus": "Must be omitted",
                        "quote_or_fact": "A fifth excerpt must not cross the host boundary."
                    }]
                }],
                "key_evidence": [
                    "The dual-stage outline-and-retrieve mechanism is documented.",
                    "The ablation reduced citation completeness by 17 percentage points.",
                    "The evaluation covered only English-language technology topics."
                ],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });

        let first = accepted_evidence_ledger(&output.to_string(), None);
        let second = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].sources.len(), 1);

        let source = serde_json::to_value(&first[0].sources[0]).unwrap();
        let excerpts = source["evidence_excerpts"]
            .as_array()
            .expect("focused source excerpts should survive the durable ledger projection");
        assert_eq!(excerpts.len(), 4);
        assert!(excerpts.iter().all(|excerpt| excerpt["id"]
            .as_str()
            .is_some_and(|id| id.starts_with("excerpt:"))));
        assert_eq!(
            excerpts
                .iter()
                .filter_map(|excerpt| excerpt["id"].as_str())
                .collect::<HashSet<_>>()
                .len(),
            excerpts.len(),
            "excerpt identities must be stable and distinct without multiplying source identities"
        );
        let retained_chars = excerpts
            .iter()
            .filter_map(|excerpt| excerpt["quote_or_fact"].as_str())
            .map(str::chars)
            .map(Iterator::count)
            .sum::<usize>();
        assert!(retained_chars <= 2_400, "{source:#}");

        let synthesis = synthesis_payload(&first);
        assert!(synthesis.contains("dual-stage outline-and-retrieve mechanism"));
        assert!(synthesis.contains("17 percentage points"));
        assert!(synthesis.contains("English-language technology topics"));
        assert!(!synthesis.contains("fifth excerpt"));
    }

    #[test]
    fn accepts_only_traceable_evidence_and_assigns_stable_ids() {
        let output = serde_json::json!({
            "structured": {
                "summary": "The release is documented.",
                "sources": [{
                    "title": "Official release",
                    "url_or_path": "https://example.gov/releases/1",
                    "quote_or_fact": "Released on July 12.",
                    "reliability": "Official primary source"
                }],
                "key_evidence": ["The release date is July 12."],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });
        let first = accepted_evidence_ledger(&output.to_string(), None);
        let second = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(first, second);
        assert_eq!(first.len(), 1);
        assert_eq!(first[0].claims.len(), 1);
        let synthesis = synthesis_payload(&first);
        assert!(synthesis.contains("Released on July 12."));
        assert!(synthesis.contains("https://example.gov/releases/1"));
    }

    #[test]
    fn provider_metadata_alone_cannot_promote_a_source_into_the_evidence_ledger() {
        let output = serde_json::json!({
            "structured": {
                "summary": "Discovery returned one metadata record.",
                "sources": [{
                    "title": "Provider result title",
                    "url_or_path": "https://example.gov/releases/1",
                    "publisher": "Provider supplied publisher",
                    "publication_date": "2026-07-24"
                }],
                "key_evidence": ["Metadata must not become a report claim."],
                "contradictions": [],
                "gaps": [],
                "confidence": "unknown"
            }
        });

        assert!(accepted_evidence_ledger(&output.to_string(), None).is_empty());
    }

    #[test]
    fn remaps_closed_source_coverage_to_stable_ledger_identities() {
        let output = serde_json::json!({
            "plan": {
                "tracks": [{
                    "id": "obligation:quality",
                    "completion_criteria": ["The direct finding is supported"],
                    "evidence_requirements": {
                        "primary_source_required": true,
                        "independent_corroboration_required": true
                    }
                }]
            },
            "structured": {
                "summary": "The direct finding is documented.",
                "sources": [{
                    "source_id": "web-source-1",
                    "title": "Direct record",
                    "url_or_path": "https://example.gov/direct",
                    "quote_or_fact": "The direct record establishes the finding.",
                    "reliability": "Fetched source text"
                }],
                "source_coverage": [{
                    "source_id": "web-source-1",
                    "obligation_id": "obligation:quality",
                    "completion_criterion_indexes": [0],
                    "roles": ["supporting", "primary", "independent"]
                }],
                "key_evidence": ["The direct record establishes the finding."],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });

        let ledger = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(ledger.len(), 1);
        assert_eq!(ledger[0].source_coverage.len(), 1);
        assert_eq!(
            ledger[0].source_coverage[0].source_id,
            ledger[0].sources[0].id
        );
        assert_eq!(
            ledger[0].source_coverage[0].roles,
            [
                SourceEvidenceRole::Supporting,
                SourceEvidenceRole::Primary,
                SourceEvidenceRole::Independent
            ]
        );
        assert_eq!(ledger[0].relevant_obligation_ids, ["obligation:quality"]);
    }

    #[test]
    fn retains_partial_obligation_relevance_without_false_criterion_coverage() {
        let output = serde_json::json!({
            "plan": {
                "tracks": [{
                    "id": "obligation:partial",
                    "completion_criteria": ["The complete finding is established"],
                    "evidence_requirements": {
                        "primary_source_required": false,
                        "independent_corroboration_required": false
                    }
                }]
            },
            "structured": {
                "summary": "One useful part of the finding is documented.",
                "sources": [{
                    "source_id": "web-source-1",
                    "url_or_path": "https://example.com/partial",
                    "quote_or_fact": "The source establishes only one bounded component."
                }],
                "source_coverage": [],
                "relevant_obligation_ids": ["obligation:partial"],
                "key_evidence": ["The source establishes only one bounded component."],
                "contradictions": [],
                "gaps": [],
                "confidence": "partial"
            }
        });

        let ledger = accepted_evidence_ledger(&output.to_string(), None);

        assert_eq!(ledger.len(), 1);
        assert!(ledger[0].source_coverage.is_empty());
        assert_eq!(ledger[0].relevant_obligation_ids, ["obligation:partial"]);
    }

    #[test]
    fn rejects_source_coverage_outside_the_host_plan_contract() {
        let output = serde_json::json!({
            "plan": {
                "tracks": [{
                    "id": "obligation:quality",
                    "completion_criteria": ["The finding is supported"],
                    "evidence_requirements": {
                        "primary_source_required": false,
                        "independent_corroboration_required": false
                    }
                }]
            },
            "structured": {
                "summary": "The finding is documented.",
                "sources": [{
                    "source_id": "web-source-1",
                    "title": "Record",
                    "url_or_path": "https://example.org/record",
                    "quote_or_fact": "The record establishes the finding."
                }],
                "source_coverage": [{
                    "source_id": "web-source-1",
                    "obligation_id": "obligation:not-in-plan",
                    "completion_criterion_indexes": [0],
                    "roles": ["supporting"]
                }],
                "key_evidence": ["The record establishes the finding."],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });

        assert!(accepted_evidence_ledger(&output.to_string(), None).is_empty());
    }

    #[test]
    fn rejects_source_free_model_claims() {
        let output = serde_json::json!({
            "structured": {
                "summary": "Unsupported claim",
                "sources": [],
                "key_evidence": ["Invented fact"],
                "contradictions": [],
                "gaps": [],
                "confidence": "high"
            }
        });
        assert!(accepted_evidence_ledger(&output.to_string(), None).is_empty());
        assert!(!synthesis_payload(&[]).contains("Invented fact"));
    }

    #[test]
    fn preserves_the_closed_selector_claim_set_without_host_text_matching() {
        let output = serde_json::json!({
            "structured": {
                "summary": "A collector proposed a decision threshold.",
                "sources": [{
                    "title": "Benchmark",
                    "url_or_path": "https://example.com/benchmark",
                    "quote_or_fact": "The benchmark covers 1M-10M vectors.",
                    "reliability": "Published benchmark"
                }],
                "key_evidence": [
                    "The benchmark covers 1M-10M vectors.",
                    "Use the product below 1M vectors."
                ],
                "contradictions": [],
                "gaps": [],
                "confidence": "medium"
            }
        });
        let evidence = accepted_evidence_ledger(&output.to_string(), None);
        assert_eq!(evidence.len(), 1);
        assert_eq!(evidence[0].claims.len(), 2, "{evidence:#?}");
        assert!(evidence[0].claims[0].text.contains("1M-10M"));
        assert!(evidence[0].claims[1].text.contains("below 1M"));
    }

    fn assessed_inquiry_workflow() -> String {
        use a3s::research::{
            replay, CompletionCriterionAssessment, ContractAssessmentStatus, EvidenceRef,
            InquiryEvent, InquiryLimits, Question, ResearchContractAssessment, ResearchMethod,
            ResearchObligation, ResearchObligationAssessment, StopConditionAssessment,
        };

        let mut core = Question::queued(
            "question:core",
            None,
            "What does the accepted evidence establish?",
        );
        core.obligation_ids = vec!["obligation:core".to_string()];
        let mut context = Question::queued(
            "question:context",
            None,
            "Which supporting limitation can be established?",
        );
        context.obligation_ids = vec!["obligation:core".to_string()];
        context.material = false;
        let events = vec![
            InquiryEvent::StrategySelected {
                method: ResearchMethod::Focused,
            },
            InquiryEvent::ResearchObligationsCommitted {
                obligations: vec![ResearchObligation::new(
                    "obligation:core",
                    "Evidence-backed answer",
                    "Establish the core answer and bound unavailable context",
                    true,
                    vec!["The core answer is traceable to accepted evidence".to_string()],
                )],
                stop_conditions: vec![
                    "Every planned question is answered or explicitly bounded".to_string()
                ],
            },
            InquiryEvent::QuestionsQueued {
                questions: vec![core, context],
            },
            InquiryEvent::EvidenceAccepted {
                evidence: EvidenceRef::new(
                    "evidence:core",
                    vec!["claim:core".to_string()],
                    vec!["source:core".to_string()],
                ),
            },
            InquiryEvent::QuestionAnswered {
                question_id: "question:core".to_string(),
                answer: "The accepted source supports the core answer.".to_string(),
                evidence_ids: vec!["evidence:core".to_string()],
            },
            InquiryEvent::QuestionBounded {
                question_id: "question:context".to_string(),
                reason: "The available evidence does not establish the supporting limitation."
                    .to_string(),
            },
            InquiryEvent::ResearchContractAssessed {
                assessment: ResearchContractAssessment {
                    obligations: vec![ResearchObligationAssessment {
                        obligation_id: "obligation:core".to_string(),
                        criteria: vec![CompletionCriterionAssessment {
                            criterion_index: 0,
                            status: ContractAssessmentStatus::Satisfied,
                            rationale: "The accepted evidence satisfies the core criterion."
                                .to_string(),
                            evidence_ids: vec!["evidence:core".to_string()],
                        }],
                        primary_source: None,
                        independent_corroboration: None,
                    }],
                    stop_conditions: vec![StopConditionAssessment {
                        condition_index: 0,
                        status: ContractAssessmentStatus::Satisfied,
                        rationale: "Both questions reached a closed terminal state.".to_string(),
                        evidence_ids: vec!["evidence:core".to_string()],
                    }],
                    diagnostics: Vec::new(),
                },
            },
        ];
        let state =
            replay(&events, &InquiryLimits::default()).expect("valid assessed inquiry fixture");
        serde_json::json!({
            "mode": "inquiry_collection_wave",
            "execution": {
                "mode": "collect_only",
                "terminal_authority": "host_inquiry_reducer"
            },
            "query": "must not be copied into report_context",
            "plan": {
                "report_title": "Reader-facing title",
                "answer_shape": "must not survive",
                "execution_route": "must not survive",
                "phases": ["must not survive"],
                "tracks": [{
                    "id": "obligation:core",
                    "title": "Evidence-backed answer",
                    "focus": "Establish the core answer and bound unavailable context",
                    "material": true
                }],
                "stop_conditions": [
                    "Every planned question is answered or explicitly bounded"
                ],
                "search_queries": ["internal retrieval instruction"]
            },
            "checker": {
                "decision": "finalize",
                "verified_findings": ["legacy checker finding must not survive"]
            },
            "verification": {
                "status": "completed",
                "checker_completed": true,
                "error": "legacy verification detail must not survive"
            },
            "inquiry": {
                "events": events,
                "state": state
            }
        })
        .to_string()
    }

    #[test]
    fn synthesis_payload_carries_only_plan_and_replayed_inquiry_context() {
        let evidence = AcceptedEvidence {
            id: "evidence:1".to_string(),
            summary: "A source-backed summary.".to_string(),
            confidence: Some("high".to_string()),
            sources: vec![AcceptedSource {
                id: "source:1".to_string(),
                anchor: "https://example.com/source".to_string(),
                title: Some("Source".to_string()),
                date: None,
                reliability: Some("Official".to_string()),
                quote_or_fact: Some("The primary finding is supported.".to_string()),
                evidence_excerpts: Vec::new(),
            }],
            claims: vec![],
            source_coverage: Vec::new(),
            relevant_obligation_ids: Vec::new(),
            contradictions: vec![],
            gaps: vec![],
        };
        let workflow = assessed_inquiry_workflow();
        let payload = synthesis_payload_with_context(&[evidence], &workflow);
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();

        let context = &payload["report_context"];
        assert_eq!(context["plan"]["report_title"], "Reader-facing title");
        assert_eq!(
            context["plan"]["tracks"][0],
            serde_json::json!({
                "id": "obligation:core",
                "title": "Evidence-backed answer",
                "focus": "Establish the core answer and bound unavailable context",
                "material": true
            })
        );
        assert_eq!(
            context["plan"]["stop_conditions"],
            serde_json::json!(["Every planned question is answered or explicitly bounded"])
        );
        assert_eq!(context["inquiry"]["contract_outcome"], "qualified");
        assert_eq!(
            context["inquiry"]["obligations"][0]["id"],
            "obligation:core"
        );
        assert_eq!(
            context["inquiry"]["contract_assessment"]["obligations"][0]["criteria"][0]["status"],
            "satisfied"
        );
        assert_eq!(context["inquiry"]["questions"][0]["status"], "answered");
        assert_eq!(
            context["inquiry"]["questions"][0]["answer"],
            "The accepted source supports the core answer."
        );
        assert_eq!(
            context["inquiry"]["questions"][0]["evidence_ids"],
            serde_json::json!(["evidence:core"])
        );
        assert_eq!(context["inquiry"]["questions"][1]["status"], "bounded");
        assert_eq!(
            context["inquiry"]["questions"][1]["bound_reason"],
            "The available evidence does not establish the supporting limitation."
        );

        for omitted in [
            "query",
            "answer_shape",
            "execution_route",
            "phases",
            "search_queries",
            "checker",
            "verification",
            "verified_findings",
            "unresolved_obligations",
            "publication_status",
        ] {
            assert!(
                !context.to_string().contains(omitted),
                "{omitted} leaked into report context: {context:#}"
            );
        }
        assert!(!context
            .to_string()
            .contains("legacy checker finding must not survive"));
        assert!(!context
            .to_string()
            .contains("legacy verification detail must not survive"));
    }

    #[test]
    fn synthesis_payload_exposes_verification_facts_without_internal_publication_status() {
        let evidence = AcceptedEvidence {
            id: "evidence:1".to_string(),
            summary: "A traceable result survived the checker failure.".to_string(),
            confidence: Some("medium".to_string()),
            sources: vec![AcceptedSource {
                id: "source:1".to_string(),
                anchor: "https://example.com/source".to_string(),
                title: Some("Source".to_string()),
                date: None,
                reliability: Some("Official".to_string()),
                quote_or_fact: Some("A traceable result survived the checker failure.".to_string()),
                evidence_excerpts: Vec::new(),
            }],
            claims: vec![],
            source_coverage: Vec::new(),
            relevant_obligation_ids: Vec::new(),
            contradictions: vec![],
            gaps: vec![],
        };
        let workflow = serde_json::json!({
            "plan": { "report_title": "Qualified result" },
            "verification": {
                "status": "degraded",
                "checker_completed": false,
                "prior_checker_retained": true,
                "error": "internal provider failure must not leak"
            }
        })
        .to_string();

        let payload = synthesis_payload_with_context(&[evidence], &workflow);
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert!(payload["report_context"]
            .get("publication_status")
            .is_none());
        assert_eq!(
            payload["report_context"]["verification"]["checker_completed"],
            false
        );
        assert!(!payload
            .to_string()
            .contains("internal provider failure must not leak"));
    }
}
