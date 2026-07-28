use super::{
    stable_id_schema, transport_schema, AcquisitionQuery, AcquisitionTransport, EvidenceScope,
    JsonValue, PlannerInput,
};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

const MAX_BRIEF_DIMENSIONS: usize = 16;
const MAX_PREFERENCES_PER_QUERY: usize = 4;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PreferredSourceKind {
    Repository,
    Domain,
    Url,
    WorkspacePath,
}

#[derive(Clone, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
pub(crate) struct SourcePreference {
    pub(crate) kind: PreferredSourceKind,
    pub(crate) value: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct BriefDimension {
    pub(crate) id: String,
    pub(crate) question: String,
    pub(crate) request_basis: Vec<String>,
    pub(crate) material: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct BriefPlanningGap {
    pub(crate) dimension_id: String,
    pub(crate) reason: String,
    pub(crate) host_generated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(crate) struct ResearchBrief {
    pub(crate) dimensions: Vec<BriefDimension>,
    pub(crate) queries: Vec<AcquisitionQuery>,
    pub(crate) planning_gaps: Vec<BriefPlanningGap>,
    pub(crate) normalization_notes: Vec<String>,
}

pub(super) fn brief_schema(input: &PlannerInput, maximum_queries: usize) -> JsonValue {
    let common_query_properties = serde_json::json!({
        "id": stable_id_schema(),
        "text": { "type": "string", "minLength": 2, "maxLength": 4000 },
        "transport": transport_schema(input.evidence_scope),
        "dimension_ids": {
            "type": "array",
            "minItems": 1,
            "maxItems": MAX_BRIEF_DIMENSIONS,
            "uniqueItems": true,
            "items": stable_id_schema()
        },
        "preferred_sources": {
            "type": "array",
            "maxItems": MAX_PREFERENCES_PER_QUERY,
            "uniqueItems": true,
            "items": {
                "type": "object",
                "additionalProperties": false,
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["repository", "domain", "url", "workspace_path"]
                    },
                    "value": { "type": "string", "minLength": 1, "maxLength": 1000 }
                },
                "required": ["kind", "value"]
            }
        }
    });
    let query = brief_query_schema(input, common_query_properties);
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "dimensions": {
                "type": "array",
                "minItems": 1,
                "maxItems": MAX_BRIEF_DIMENSIONS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "id": stable_id_schema(),
                        "question": { "type": "string", "minLength": 4, "maxLength": 1000 },
                        "request_basis": {
                            "type": "array",
                            "minItems": 1,
                            "maxItems": 4,
                            "uniqueItems": true,
                            "items": { "type": "string", "minLength": 2, "maxLength": 1000 }
                        },
                        "material": { "type": "boolean" }
                    },
                    "required": ["id", "question", "request_basis", "material"]
                }
            },
            "queries": {
                "type": "array",
                "minItems": 0,
                "maxItems": maximum_queries,
                "items": query
            },
            "planning_gaps": {
                "type": "array",
                "maxItems": MAX_BRIEF_DIMENSIONS,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "dimension_id": stable_id_schema(),
                        "reason": { "type": "string", "minLength": 4, "maxLength": 1000 }
                    },
                    "required": ["dimension_id", "reason"]
                }
            }
        },
        "required": ["dimensions", "queries", "planning_gaps"]
    })
}

fn brief_query_schema(input: &PlannerInput, common_properties: JsonValue) -> JsonValue {
    let mut properties = common_properties;
    let required = match input.evidence_scope {
        EvidenceScope::Web => {
            vec![
                "id",
                "text",
                "transport",
                "dimension_ids",
                "preferred_sources",
            ]
        }
        EvidenceScope::Workspace | EvidenceScope::WebAndWorkspace => {
            properties["path"] = serde_json::json!({ "type": "string", "maxLength": 500 });
            properties["glob"] = serde_json::json!({ "type": "string", "maxLength": 500 });
            vec![
                "id",
                "text",
                "transport",
                "path",
                "glob",
                "dimension_ids",
                "preferred_sources",
            ]
        }
    };
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": properties,
        "required": required,
    })
}

pub(super) fn validate_brief(
    proposal: &JsonValue,
    input: &PlannerInput,
    has_bootstrap: bool,
) -> Result<ResearchBrief, String> {
    let object = proposal
        .as_object()
        .ok_or_else(|| "brief planner returned a non-object proposal".to_string())?;
    let mut notes = Vec::new();
    let mut dimensions = normalize_dimensions(object.get("dimensions"), &input.query, &mut notes);
    if dimensions.is_empty() {
        notes.push("Host created a fallback dimension because no valid dimension survived".into());
        dimensions.push(BriefDimension {
            id: "request.primary".to_string(),
            question: bounded_text(&input.query, 1000),
            request_basis: vec![bounded_text(&input.query, 1000)],
            material: true,
        });
    }
    let dimension_ids = dimensions
        .iter()
        .map(|dimension| dimension.id.as_str())
        .collect::<BTreeSet<_>>();
    let proposed_query_count = object
        .get("queries")
        .and_then(JsonValue::as_array)
        .map(Vec::len)
        .unwrap_or_default();
    let mut queries = normalize_queries(object.get("queries"), input, &dimension_ids, &mut notes);
    let mut planning_gaps = normalize_gaps(object.get("planning_gaps"), &dimension_ids, &mut notes);
    if queries.is_empty() && planning_gaps.is_empty() && proposed_query_count == 0 && !has_bootstrap
    {
        queries.push(fallback_query(input, &dimensions[0].id));
        notes.push("Host created a fallback query because the proposal scheduled no work".into());
    }

    let covered = queries
        .iter()
        .flat_map(|query| query.dimension_ids.iter().map(String::as_str))
        .collect::<BTreeSet<_>>();
    let gapped = planning_gaps
        .iter()
        .map(|gap| gap.dimension_id.clone())
        .collect::<BTreeSet<_>>();
    for dimension in dimensions
        .iter()
        .filter(|dimension| dimension.material && !has_bootstrap)
    {
        if !covered.contains(dimension.id.as_str()) && !gapped.contains(&dimension.id) {
            planning_gaps.push(BriefPlanningGap {
                dimension_id: dimension.id.clone(),
                reason: "No valid acquisition query survived Host normalization for this material dimension."
                    .to_string(),
                host_generated: true,
            });
            notes.push(format!(
                "Host added planning coverage for dimension `{}`",
                dimension.id
            ));
        }
    }

    Ok(ResearchBrief {
        dimensions,
        queries,
        planning_gaps,
        normalization_notes: notes,
    })
}

fn normalize_dimensions(
    value: Option<&JsonValue>,
    request: &str,
    notes: &mut Vec<String>,
) -> Vec<BriefDimension> {
    let mut ids = BTreeSet::new();
    value
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .take(MAX_BRIEF_DIMENSIONS)
        .filter_map(|value| {
            let Some(object) = value.as_object() else {
                notes.push("Host dropped a non-object brief dimension".to_string());
                return None;
            };
            let id = object
                .get("id")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            let question = object
                .get("question")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .trim();
            let material = object
                .get("material")
                .and_then(JsonValue::as_bool)
                .unwrap_or(true);
            let mut request_basis = object
                .get("request_basis")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .filter_map(JsonValue::as_str)
                .map(str::trim)
                .filter(|basis| basis.chars().count() >= 2 && request.contains(*basis))
                .map(|basis| bounded_text(basis, 1000))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .take(4)
                .collect::<Vec<_>>();
            if request_basis.is_empty() {
                request_basis.push(bounded_text(request, 1000));
                notes.push(format!(
                    "Host bounded dimension `{id}` to the complete request because no exact request basis survived"
                ));
            }
            if !stable_id(id) || question.chars().count() < 4 || !ids.insert(id.to_string()) {
                notes.push(format!(
                    "Host dropped invalid or duplicate dimension `{id}`"
                ));
                return None;
            }
            Some(BriefDimension {
                id: id.to_string(),
                question: bounded_text(question, 1000),
                request_basis,
                material,
            })
        })
        .collect()
}

fn normalize_queries(
    value: Option<&JsonValue>,
    input: &PlannerInput,
    dimension_ids: &BTreeSet<&str>,
    notes: &mut Vec<String>,
) -> Vec<AcquisitionQuery> {
    let mut ids = BTreeSet::new();
    value
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .take(input.budget.max_queries)
        .filter_map(|value| {
            let Some(object) = value.as_object() else {
                notes.push("Host dropped a non-object brief query".to_string());
                return None;
            };
            let id = object
                .get("id")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            let text = object
                .get("text")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .trim();
            let transport = object
                .get("transport")
                .cloned()
                .and_then(|value| serde_json::from_value::<AcquisitionTransport>(value).ok());
            let Some(transport) = transport else {
                notes.push(format!(
                    "Host dropped query `{id}` with an invalid transport"
                ));
                return None;
            };
            if !stable_id(id)
                || !ids.insert(id.to_string())
                || text.chars().count() < 2
                || !input.evidence_scope.permits(transport)
            {
                notes.push(format!("Host dropped invalid or duplicate query `{id}`"));
                return None;
            }
            let mut linked = object
                .get("dimension_ids")
                .and_then(JsonValue::as_array)
                .into_iter()
                .flatten()
                .filter_map(JsonValue::as_str)
                .filter(|id| dimension_ids.contains(*id))
                .map(str::to_string)
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect::<Vec<_>>();
            if linked.is_empty() {
                notes.push(format!(
                    "Host dropped query `{id}` with no known dimension edge"
                ));
                return None;
            }
            linked.sort();
            let mut path = object
                .get("path")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            let mut glob = object
                .get("glob")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .trim()
                .to_string();
            match transport {
                AcquisitionTransport::Web => {
                    if !path.is_empty() || !glob.is_empty() {
                        notes.push(format!(
                            "Host cleared workspace-only fields from web query `{id}`"
                        ));
                    }
                    path.clear();
                    glob.clear();
                }
                AcquisitionTransport::Workspace => {
                    if !safe_workspace_path(&path)
                        || regex::Regex::new(text).is_err()
                        || glob.starts_with('/')
                        || glob.split('/').any(|part| part == "..")
                    {
                        notes.push(format!("Host dropped unsafe workspace query `{id}`"));
                        return None;
                    }
                }
            }
            let preferred_sources =
                normalize_preferences(object.get("preferred_sources"), transport, id, notes);
            let text = match transport {
                AcquisitionTransport::Web => validate_web_query(text, id, notes)?,
                AcquisitionTransport::Workspace => text.to_string(),
            };
            Some(AcquisitionQuery {
                id: id.to_string(),
                text: bounded_text(&text, 4000),
                transport,
                path,
                glob,
                dimension_ids: linked,
                source_target_ids: Vec::new(),
                preferred_sources,
                fetch_slots: 0,
            })
        })
        .collect()
}

fn validate_web_query(text: &str, query_id: &str, notes: &mut Vec<String>) -> Option<String> {
    if text.trim().is_empty() {
        notes.push(format!("Host dropped empty web query `{query_id}`"));
        return None;
    }
    Some(text.to_string())
}

fn normalize_preferences(
    value: Option<&JsonValue>,
    transport: AcquisitionTransport,
    query_id: &str,
    notes: &mut Vec<String>,
) -> Vec<SourcePreference> {
    value
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .take(MAX_PREFERENCES_PER_QUERY)
        .filter_map(|value| {
            let mut preference = serde_json::from_value::<SourcePreference>(value.clone()).ok()?;
            let original = preference.value.clone();
            let Some(normalized) = normalized_preference_value(&preference, transport) else {
                notes.push(format!(
                    "Host ignored an invalid source preference on query `{query_id}`"
                ));
                return None;
            };
            preference.value = normalized;
            if preference.value != original {
                notes.push(format!(
                    "Host normalized a source preference on query `{query_id}`"
                ));
            }
            Some(preference)
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect()
}

fn normalize_gaps(
    value: Option<&JsonValue>,
    dimension_ids: &BTreeSet<&str>,
    notes: &mut Vec<String>,
) -> Vec<BriefPlanningGap> {
    let mut seen = BTreeSet::new();
    value
        .and_then(JsonValue::as_array)
        .into_iter()
        .flatten()
        .take(MAX_BRIEF_DIMENSIONS)
        .filter_map(|value| {
            let object = value.as_object()?;
            let id = object
                .get("dimension_id")
                .and_then(JsonValue::as_str)
                .unwrap_or_default();
            let reason = object
                .get("reason")
                .and_then(JsonValue::as_str)
                .unwrap_or_default()
                .trim();
            if !dimension_ids.contains(id)
                || reason.chars().count() < 4
                || !seen.insert(id.to_string())
            {
                notes.push(format!("Host dropped invalid planning gap for `{id}`"));
                return None;
            }
            Some(BriefPlanningGap {
                dimension_id: id.to_string(),
                reason: bounded_text(reason, 1000),
                host_generated: false,
            })
        })
        .collect()
}

fn fallback_query(input: &PlannerInput, dimension_id: &str) -> AcquisitionQuery {
    let transport = match input.evidence_scope {
        EvidenceScope::Workspace => AcquisitionTransport::Workspace,
        EvidenceScope::Web | EvidenceScope::WebAndWorkspace => AcquisitionTransport::Web,
    };
    let text = match transport {
        AcquisitionTransport::Web => input.query.clone(),
        AcquisitionTransport::Workspace => r"\S".to_string(),
    };
    AcquisitionQuery {
        id: "query.fallback".to_string(),
        text,
        transport,
        path: String::new(),
        glob: String::new(),
        dimension_ids: vec![dimension_id.to_string()],
        source_target_ids: Vec::new(),
        preferred_sources: Vec::new(),
        fetch_slots: 0,
    }
}

fn normalized_preference_value(
    preference: &SourcePreference,
    transport: AcquisitionTransport,
) -> Option<String> {
    match (preference.kind, transport) {
        (PreferredSourceKind::Repository, AcquisitionTransport::Web) => {
            normalize_repository_preference(&preference.value)
        }
        (PreferredSourceKind::Domain, AcquisitionTransport::Web) => {
            let value = preference
                .value
                .trim()
                .trim_start_matches("www.")
                .to_ascii_lowercase();
            (value.contains('.')
                && !value.contains('/')
                && !value.contains(char::is_whitespace)
                && value
                    .split('.')
                    .all(|part| !part.is_empty() && safe_identity_part(part)))
            .then_some(value)
        }
        (PreferredSourceKind::Url, AcquisitionTransport::Web) => {
            let value = preference.value.trim();
            reqwest::Url::parse(value)
                .ok()
                .filter(|url| {
                    url.scheme() == "https" && url.host_str().is_some() && url.username().is_empty()
                })
                .map(|_| value.to_string())
        }
        (PreferredSourceKind::WorkspacePath, AcquisitionTransport::Workspace) => {
            let value = preference.value.trim();
            (safe_workspace_path(value) && !value.is_empty()).then(|| value.to_string())
        }
        _ => None,
    }
}

fn normalize_repository_preference(value: &str) -> Option<String> {
    let value = value.trim().trim_end_matches('/');
    let repository = if let Ok(url) = reqwest::Url::parse(value) {
        if url.scheme() != "https"
            || url.host_str()? != "github.com"
            || !url.username().is_empty()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return None;
        }
        url.path().trim_matches('/').to_string()
    } else {
        value
            .strip_prefix("github.com/")
            .unwrap_or(value)
            .to_string()
    };
    let parts = repository.split('/').collect::<Vec<_>>();
    (parts.len() == 2 && parts.iter().all(|part| safe_identity_part(part)))
        .then(|| format!("{}/{}", parts[0], parts[1]))
}

fn stable_id(value: &str) -> bool {
    !value.is_empty()
        && value.chars().count() <= 64
        && value.chars().enumerate().all(|(index, character)| {
            character.is_ascii_alphanumeric()
                || (index > 0 && matches!(character, '.' | '_' | ':' | '-'))
        })
}

fn safe_identity_part(value: &str) -> bool {
    !value.is_empty()
        && value.chars().all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '.' | '_' | '-')
        })
}

fn safe_workspace_path(value: &str) -> bool {
    let path = std::path::Path::new(value.trim());
    !path.is_absolute()
        && path
            .components()
            .all(|component| !matches!(component, std::path::Component::ParentDir))
}

fn bounded_text(value: &str, maximum: usize) -> String {
    value.trim().chars().take(maximum).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::code::research_runtime::tests::baseline::live::corpus::PlannerBudget;

    fn input(scope: EvidenceScope) -> PlannerInput {
        PlannerInput {
            schema: "test".to_string(),
            query: "compare alpha and beta".to_string(),
            report_language: "en".to_string(),
            current_date: "2026-07-22".to_string(),
            display_utc_offset: "+08:00".to_string(),
            evidence_scope: scope,
            budget: PlannerBudget {
                max_queries: 4,
                max_acquired_sources: 8,
            },
        }
    }

    #[test]
    fn brief_host_contract_drops_bad_hints_and_closes_dimension_coverage() {
        let proposal = serde_json::json!({
            "dimensions": [
                {"id": "maintenance", "question": "What is maintained?", "material": true},
                {"id": "compatibility", "question": "What is compatible?", "material": true}
            ],
            "queries": [{
                "id": "q1",
                "text": "alpha project official maintenance",
                "transport": "web",
                "path": "src",
                "glob": "*.rs",
                "dimension_ids": ["maintenance", "unknown"],
                "preferred_sources": [
                    {"kind": "repository", "value": "owner/project"},
                    {"kind": "workspace_path", "value": "../../secret"},
                    {"kind": "domain", "value": "not a domain"}
                ]
            }],
            "planning_gaps": []
        });
        let brief = validate_brief(&proposal, &input(EvidenceScope::Web), false).expect("brief");
        assert_eq!(brief.queries.len(), 1);
        assert_eq!(brief.queries[0].path, "");
        assert_eq!(brief.queries[0].preferred_sources.len(), 1);
        assert_eq!(brief.queries[0].fetch_slots, 0);
        assert_eq!(brief.planning_gaps.len(), 1);
        assert_eq!(brief.planning_gaps[0].dimension_id, "compatibility");
        assert!(brief.planning_gaps[0].host_generated);
    }

    #[test]
    fn brief_query_capacity_is_a_cap_not_a_consumption_target() {
        let proposal = serde_json::json!({
            "dimensions": [{"id": "d1", "question": "What is true?", "material": true}],
            "queries": [{
                "id": "q1",
                "text": "alpha canonical policy",
                "transport": "web",
                "path": "",
                "glob": "",
                "dimension_ids": ["d1"],
                "preferred_sources": []
            }],
            "planning_gaps": []
        });
        let brief = validate_brief(&proposal, &input(EvidenceScope::Web), false).expect("brief");
        assert_eq!(brief.queries[0].fetch_slots, 0);
        assert_eq!(
            brief
                .queries
                .iter()
                .map(|query| query.fetch_slots)
                .sum::<usize>(),
            0
        );
    }

    #[test]
    fn invalid_workspace_query_becomes_a_local_planning_gap() {
        let proposal = serde_json::json!({
            "dimensions": [{"id": "path", "question": "Which path is active?", "material": true}],
            "queries": [{
                "id": "q1",
                "text": "(",
                "transport": "workspace",
                "path": "../outside",
                "glob": "*.rs",
                "dimension_ids": ["path"],
                "preferred_sources": []
            }],
            "planning_gaps": []
        });
        let brief =
            validate_brief(&proposal, &input(EvidenceScope::Workspace), false).expect("brief");
        assert!(brief.queries.is_empty());
        assert_eq!(brief.planning_gaps.len(), 1);
        assert_eq!(brief.planning_gaps[0].dimension_id, "path");
    }

    #[test]
    fn web_schema_does_not_invite_workspace_fields() {
        let schema = brief_schema(&input(EvidenceScope::Web), 3);
        let query = &schema["properties"]["queries"]["items"];
        assert!(query["properties"].get("path").is_none());
        assert!(query["properties"].get("glob").is_none());
        assert_eq!(schema["properties"]["queries"]["maxItems"], 3);
    }

    #[test]
    fn web_query_prose_is_not_interpreted_by_the_host() {
        let proposal = serde_json::json!({
            "dimensions": [{"id": "policy", "question": "What is Alpha policy?", "material": true}],
            "queries": [
                {
                    "id": "regex",
                    "text": "(?i)(policy|support).{0,20}",
                    "transport": "web",
                    "dimension_ids": ["policy"],
                    "preferred_sources": [{"kind": "repository", "value": "owner/alpha"}]
                },
                {
                    "id": "bare",
                    "text": "support policy version",
                    "transport": "web",
                    "dimension_ids": ["policy"],
                    "preferred_sources": [{"kind": "repository", "value": "owner/alpha"}]
                }
            ],
            "planning_gaps": []
        });
        let brief = validate_brief(&proposal, &input(EvidenceScope::Web), false).expect("brief");
        assert_eq!(brief.queries.len(), 2);
        assert_eq!(brief.queries[0].text, "(?i)(policy|support).{0,20}");
        assert_eq!(brief.queries[1].text, "support policy version");
        assert!(brief.planning_gaps.is_empty());
    }

    #[test]
    fn bootstrap_covers_material_dimensions_without_forced_followups() {
        let proposal = serde_json::json!({
            "dimensions": [{"id": "policy", "question": "What is Alpha policy?", "material": true}],
            "queries": [],
            "planning_gaps": []
        });
        let brief = validate_brief(&proposal, &input(EvidenceScope::Web), true).expect("brief");
        assert!(brief.queries.is_empty());
        assert!(brief.planning_gaps.is_empty());
    }

    #[test]
    fn github_repository_preference_is_normalized_to_owner_repository() {
        let proposal = serde_json::json!({
            "dimensions": [{"id": "policy", "question": "What is Tokio policy?", "material": true}],
            "queries": [{
                "id": "q1",
                "text": "Tokio canonical policy in tokio-rs repository",
                "transport": "web",
                "dimension_ids": ["policy"],
                "preferred_sources": [{
                    "kind": "repository",
                    "value": "github.com/tokio-rs/tokio"
                }]
            }],
            "planning_gaps": []
        });
        let brief = validate_brief(&proposal, &input(EvidenceScope::Web), false).expect("brief");
        assert_eq!(
            brief.queries[0].preferred_sources[0].value,
            "tokio-rs/tokio"
        );
        assert!(brief
            .normalization_notes
            .iter()
            .any(|note| note.contains("normalized a source preference")));
    }
}
