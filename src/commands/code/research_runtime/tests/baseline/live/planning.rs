mod brief;

use super::corpus::{AcquisitionTransport, EvidenceScope, LiveBudget, PlannerInput};
use a3s_code_core::llm::structured::{generate_blocking, StructuredMode, StructuredRequest};
use a3s_code_core::llm::LlmClient;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use std::collections::{BTreeMap, BTreeSet};
use std::time::Instant;

pub(super) use brief::{BriefDimension, PreferredSourceKind, ResearchBrief, SourcePreference};

const COMPILER_SPEC_VERSION: u32 = 2;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EvaluationStrategy {
    Minimal,
    Brief,
    Compiler,
}

impl EvaluationStrategy {
    pub(super) fn label(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Brief => "brief",
            Self::Compiler => "compiler",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct AcquisitionQuery {
    pub(super) id: String,
    pub(super) text: String,
    pub(super) transport: AcquisitionTransport,
    pub(super) path: String,
    pub(super) glob: String,
    pub(super) dimension_ids: Vec<String>,
    pub(super) source_target_ids: Vec<String>,
    #[serde(default)]
    pub(super) preferred_sources: Vec<SourcePreference>,
    pub(super) fetch_slots: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct PlanningProposal {
    pub(super) strategy: EvaluationStrategy,
    pub(super) planner_input: PlannerInput,
    pub(super) prompt: String,
    pub(super) proposal: JsonValue,
    pub(super) elapsed_ms: u64,
    pub(super) prompt_tokens: usize,
    pub(super) completion_tokens: usize,
    pub(super) repair_rounds: u8,
    pub(super) mode_used: String,
    pub(super) reserved_query_count: usize,
}

#[derive(Clone, Debug, Serialize)]
pub(super) struct PlanningResult {
    pub(super) strategy: EvaluationStrategy,
    pub(super) planner_input: PlannerInput,
    pub(super) prompt: String,
    pub(super) proposal: JsonValue,
    pub(super) brief: Option<ResearchBrief>,
    pub(super) spec: Option<JsonValue>,
    pub(super) plan: Option<JsonValue>,
    pub(super) queries: Vec<AcquisitionQuery>,
    pub(super) elapsed_ms: u64,
    pub(super) prompt_tokens: usize,
    pub(super) completion_tokens: usize,
    pub(super) repair_rounds: u8,
    pub(super) mode_used: String,
}

pub(super) async fn generate_plan(
    llm: &dyn LlmClient,
    input: PlannerInput,
    budget: &LiveBudget,
    strategy: EvaluationStrategy,
    reserved_query_count: usize,
    bootstrap_observation: Option<&JsonValue>,
) -> Result<PlanningProposal, String> {
    let reserved_query_count = reserved_query_count.min(input.budget.max_queries);
    let prompt = planner_prompt(&input, strategy, bootstrap_observation)?;
    let request = StructuredRequest {
        prompt: prompt.clone(),
        system: Some(
            "You plan bounded evidence acquisition. Infer structure only from the supplied planner input, do not answer the research request, and return only the requested object."
                .to_string(),
        ),
        schema: planner_schema(&input, strategy),
        schema_name: match strategy {
            EvaluationStrategy::Minimal => "deep_research_query_plan".to_string(),
            EvaluationStrategy::Brief => "deep_research_research_brief".to_string(),
            EvaluationStrategy::Compiler => {
                format!("deep_research_{}_plan", strategy.label())
            }
        },
        schema_description: Some(match strategy {
            EvaluationStrategy::Minimal => {
                "A small transport-aware query list without evaluator knowledge".to_string()
            }
            EvaluationStrategy::Brief => {
                "A bounded research brief with non-authoritative source preferences".to_string()
            }
            EvaluationStrategy::Compiler => {
                "A dimension-preserving evidence compiler contract".to_string()
            }
        }),
        mode: StructuredMode::Auto,
        max_repair_attempts: 0,
    };
    let started = Instant::now();
    let generated = tokio::time::timeout(
        std::time::Duration::from_millis(budget.planner_timeout_ms),
        generate_blocking(llm, &request),
    )
    .await;
    let elapsed_ms = started.elapsed().as_millis() as u64;
    let (proposal, prompt_tokens, completion_tokens, repair_rounds, mode_used) = match generated {
        Ok(Ok(generated)) => (
            generated.object,
            generated.usage.prompt_tokens,
            generated.usage.completion_tokens,
            generated.repair_rounds,
            format!("{:?}", generated.mode_used),
        ),
        Ok(Err(error)) if strategy != EvaluationStrategy::Compiler => (
            host_fallback_proposal(
                &input,
                strategy,
                reserved_query_count > 0,
                &format!("planner generation failed: {error:#}"),
            ),
            0,
            0,
            0,
            "HostFallback".to_string(),
        ),
        Err(_) if strategy != EvaluationStrategy::Compiler => (
            host_fallback_proposal(
                &input,
                strategy,
                reserved_query_count > 0,
                "planner host timeout",
            ),
            0,
            0,
            0,
            "HostFallback".to_string(),
        ),
        Ok(Err(error)) => return Err(format!("planner generation failed: {error:#}")),
        Err(_) => return Err("planner host timeout".to_string()),
    };
    Ok(PlanningProposal {
        strategy,
        planner_input: input,
        prompt,
        proposal,
        elapsed_ms,
        prompt_tokens,
        completion_tokens,
        repair_rounds,
        mode_used,
        reserved_query_count,
    })
}

fn host_fallback_proposal(
    input: &PlannerInput,
    strategy: EvaluationStrategy,
    has_bootstrap: bool,
    reason: &str,
) -> JsonValue {
    let transport = match input.evidence_scope {
        EvidenceScope::Workspace => AcquisitionTransport::Workspace,
        EvidenceScope::Web | EvidenceScope::WebAndWorkspace => AcquisitionTransport::Web,
    };
    let query_text = match transport {
        AcquisitionTransport::Web => input.query.clone(),
        AcquisitionTransport::Workspace => r"\S".to_string(),
    };
    let diagnostic = serde_json::json!({
        "reason": reason.chars().take(1_000).collect::<String>()
    });
    match strategy {
        EvaluationStrategy::Minimal => serde_json::json!({
            "queries": [{
                "id": "query.fallback",
                "text": query_text,
                "transport": transport,
                "path": "",
                "glob": ""
            }],
            "host_fallback": diagnostic
        }),
        EvaluationStrategy::Brief => serde_json::json!({
            "dimensions": [{
                "id": "request.primary",
                "question": input.query.chars().take(1_000).collect::<String>(),
                "request_basis": [input.query.chars().take(1_000).collect::<String>()],
                "material": true
            }],
            "queries": if has_bootstrap {
                Vec::<JsonValue>::new()
            } else {
                vec![serde_json::json!({
                    "id": "query.fallback",
                    "text": query_text,
                    "transport": transport,
                    "path": "",
                    "glob": "",
                    "dimension_ids": ["request.primary"],
                    "preferred_sources": []
                })]
            },
            "planning_gaps": [],
            "host_fallback": diagnostic
        }),
        EvaluationStrategy::Compiler => serde_json::json!({
            "host_fallback": diagnostic
        }),
    }
}

pub(super) fn validate_proposal(generated: PlanningProposal) -> Result<PlanningResult, String> {
    let PlanningProposal {
        strategy,
        planner_input: input,
        prompt,
        proposal,
        elapsed_ms,
        prompt_tokens,
        completion_tokens,
        repair_rounds,
        mode_used,
        reserved_query_count,
    } = generated;
    let (brief, spec, plan, queries) = match strategy {
        EvaluationStrategy::Minimal => {
            let brief = validate_root_brief(&proposal, &input, reserved_query_count > 0)?;
            let queries = brief.queries.clone();
            (Some(brief), None, None, queries)
        }
        EvaluationStrategy::Brief => {
            let brief = brief::validate_brief(&proposal, &input, reserved_query_count > 0)?;
            let queries = brief.queries.clone();
            (Some(brief), None, None, queries)
        }
        EvaluationStrategy::Compiler => {
            let (spec, plan, queries) = compiler_contract(&proposal, &input)?;
            (None, Some(spec), Some(plan), queries)
        }
    };
    Ok(PlanningResult {
        strategy,
        planner_input: input,
        prompt,
        proposal,
        brief,
        spec,
        plan,
        queries,
        elapsed_ms,
        prompt_tokens,
        completion_tokens,
        repair_rounds,
        mode_used,
    })
}

pub(super) fn planner_prompt(
    input: &PlannerInput,
    strategy: EvaluationStrategy,
    bootstrap_observation: Option<&JsonValue>,
) -> Result<String, String> {
    let input = serde_json::to_string(input).map_err(|error| error.to_string())?;
    let instructions = match strategy {
        EvaluationStrategy::Minimal => {
            "Return only a small list of acquisition queries. The Host preserves the complete original request as one immutable root obligation; do not create dimensions, source targets, claims, conclusions, expected answers, or fetch allocations. The Host has already attempted the complete request in BOOTSTRAP_OBSERVATION; do not repeat that exact broad query, and use the remaining capacity for canonical or materially different records. A web query must be natural-language, concise, entity-bearing, and source-seeking; include the subject name in every query and prefer wording that can locate first-party records. Never use a regex, bare field name, workspace path, `site:` operator, or Boolean `OR` as a web query. A workspace query must use `text` as a safe Rust-regex pattern with a repository-relative `path` and optional `glob`. The Host owns the root contract, normalization, budgets, selection, evidence admission, and publication."
        }
        EvaluationStrategy::Brief => {
            "Return a small research brief: independently assessable material dimensions, bounded follow-up acquisition queries linked to those dimensions, optional best-effort source preferences, and explicit planning gaps only when a dimension cannot be scheduled. Dimensions and preferences are acquisition proposals only: they do not establish source authority, relevance, support, coverage, or completion. Every dimension must include request_basis containing one or more minimal, exact, contiguous quotations copied from PLANNER_INPUT.query. Preserve the request's actual burden of proof; do not add a required file, source family, corroboration rule, or methodology that the request does not require. The Host has already attempted the complete request in BOOTSTRAP_OBSERVATION; inspect the observed candidates and spend only the remaining query capacity on missing canonical records or materially different evidence. A preference is a soft ranking hint, never a required source target. A web query must be natural-language, concise, entity-bearing, and source-seeking; include the subject name in every query. Never use a regex, bare field name, workspace path, `site:` operator, or Boolean `OR` as a web query. A workspace query must use `text` as a safe Rust-regex pattern with a repository-relative `path` and optional `glob`. Do not create source IDs, claims, conclusions, expected answers, completion criteria, or fetch allocations."
        }
        EvaluationStrategy::Compiler => {
            "Freeze independently assessable material dimensions, claim-appropriate source targets, and exact query-to-dimension-to-target edges. Every material dimension and every declared target must be scheduled or explicitly represented by one planning gap. Use stable ASCII IDs. For a named web target choose repository, domain, or url identity only when that canonical identity is defensible; otherwise use exploratory. A repository match_value is exactly owner/repository, a domain match_value is only a hostname, a url match_value is a complete HTTPS URL, and a workspace_path match_value is repository-relative. For workspace evidence use a known workspace_path identity or exploratory. A workspace query uses `text` as a safe Rust-regex pattern plus a repository-relative `path` and optional `glob`; use empty strings for unused path or glob fields. Named targets require exact mode; exploratory targets require discovery mode. Do not propose per-query fetch allocations: the Host allocates the shared source cap after validating the target edges. Keep scheduled target edges within that cap and put unscheduled targets in planning_gaps. Do not answer the request or state factual conclusions."
        }
    };
    let bootstrap = bootstrap_observation
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| error.to_string())?;
    let bootstrap = bootstrap
        .as_deref()
        .map(|value| format!("\n\nBOOTSTRAP_OBSERVATION={value}"))
        .unwrap_or_default();
    Ok(format!(
        "{instructions}\n\nPLANNER_INPUT={input}{bootstrap}\n\nThe evaluator's expected dimensions, required authorities, guardrails, and answers are intentionally unavailable. Infer only from PLANNER_INPUT and BOOTSTRAP_OBSERVATION."
    ))
}

fn planner_schema(input: &PlannerInput, strategy: EvaluationStrategy) -> JsonValue {
    match strategy {
        EvaluationStrategy::Minimal => return query_list_schema(input, input.budget.max_queries),
        EvaluationStrategy::Brief => return brief::brief_schema(input, input.budget.max_queries),
        EvaluationStrategy::Compiler => {}
    }
    let query = query_schema(input, strategy);
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "dimensions": {
                "type": "array",
                "minItems": 1,
                "maxItems": 20,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "id": stable_id_schema(),
                        "question": { "type": "string", "minLength": 4, "maxLength": 1000 },
                        "material": { "type": "boolean" },
                        "source_target_ids": {
                            "type": "array",
                            "maxItems": 12,
                            "uniqueItems": true,
                            "items": stable_id_schema()
                        }
                    },
                    "required": ["id", "question", "material", "source_target_ids"]
                }
            },
            "source_targets": {
                "type": "array",
                "minItems": 1,
                "maxItems": 24,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "id": stable_id_schema(),
                        "source_family_id": stable_id_schema(),
                        "role": { "type": "string", "enum": ["canonical", "official", "primary", "independent"] },
                        "transport": transport_schema(input.evidence_scope),
                        "match_kind": { "type": "string", "enum": ["repository", "domain", "url", "workspace_path", "exploratory"] },
                        "match_value": { "type": "string", "minLength": 2, "maxLength": 1000 }
                    },
                    "required": ["id", "source_family_id", "role", "transport", "match_kind", "match_value"]
                }
            },
            "queries": {
                "type": "array",
                "minItems": 0,
                "maxItems": input.budget.max_queries,
                "items": query
            },
            "planning_gaps": {
                "type": "array",
                "maxItems": 20,
                "items": {
                    "type": "object",
                    "additionalProperties": false,
                    "properties": {
                        "dimension_id": stable_id_schema(),
                        "missing_source_target_ids": {
                            "type": "array",
                            "uniqueItems": true,
                            "items": stable_id_schema()
                        },
                        "reason": { "type": "string", "minLength": 4, "maxLength": 1000 }
                    },
                    "required": ["dimension_id", "missing_source_target_ids", "reason"]
                }
            }
        },
        "required": ["dimensions", "source_targets", "queries", "planning_gaps"]
    })
}

fn query_list_schema(input: &PlannerInput, maximum_queries: usize) -> JsonValue {
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "queries": {
                "type": "array",
                "minItems": 1,
                "maxItems": maximum_queries,
                "items": query_schema(input, EvaluationStrategy::Minimal)
            }
        },
        "required": ["queries"]
    })
}

fn query_schema(input: &PlannerInput, strategy: EvaluationStrategy) -> JsonValue {
    if strategy == EvaluationStrategy::Minimal {
        let mut properties = serde_json::json!({
            "id": stable_id_schema(),
            "text": { "type": "string", "minLength": 2, "maxLength": 4000 },
            "transport": transport_schema(input.evidence_scope),
        });
        let mut required = vec!["id", "text", "transport"];
        if input.evidence_scope != EvidenceScope::Web {
            properties["path"] = serde_json::json!({ "type": "string", "maxLength": 500 });
            properties["glob"] = serde_json::json!({ "type": "string", "maxLength": 500 });
            required.extend(["path", "glob"]);
        }
        return serde_json::json!({
            "type": "object",
            "additionalProperties": false,
            "properties": properties,
            "required": required,
        });
    }
    let (minimum_dimensions, minimum_targets) = if strategy == EvaluationStrategy::Compiler {
        (1, 1)
    } else {
        (0, 0)
    };
    serde_json::json!({
        "type": "object",
        "additionalProperties": false,
        "properties": {
            "id": stable_id_schema(),
            "text": { "type": "string", "minLength": 2, "maxLength": 4000 },
            "transport": transport_schema(input.evidence_scope),
            "path": { "type": "string", "maxLength": 500 },
            "glob": { "type": "string", "maxLength": 500 },
            "mode": { "type": "string", "enum": ["exact", "discovery"] },
            "dimension_ids": {
                "type": "array",
                "minItems": minimum_dimensions,
                "maxItems": 20,
                "uniqueItems": true,
                "items": stable_id_schema()
            },
            "source_target_ids": {
                "type": "array",
                "minItems": minimum_targets,
                "maxItems": 12,
                "uniqueItems": true,
                "items": stable_id_schema()
            },
        },
        "required": ["id", "text", "transport", "path", "glob", "mode", "dimension_ids", "source_target_ids"]
    })
}

fn stable_id_schema() -> JsonValue {
    serde_json::json!({
        "type": "string",
        "pattern": "^[A-Za-z0-9][A-Za-z0-9._:-]{0,63}$"
    })
}

fn transport_schema(scope: EvidenceScope) -> JsonValue {
    let values = match scope {
        EvidenceScope::Web => vec!["web"],
        EvidenceScope::Workspace => vec!["workspace"],
        EvidenceScope::WebAndWorkspace => vec!["web", "workspace"],
    };
    serde_json::json!({ "type": "string", "enum": values })
}

fn validate_minimal_plan(
    proposal: &JsonValue,
    input: &PlannerInput,
) -> Result<Vec<AcquisitionQuery>, String> {
    let queries = proposal["queries"]
        .as_array()
        .ok_or_else(|| "minimal plan omitted queries".to_string())?;
    let normalized = queries
        .iter()
        .map(strip_minimal_query_fields)
        .collect::<Vec<_>>();
    let queries = close_query_budget(&normalized, input, false)?;
    let queries = decode_queries(&queries)?;
    validate_queries(&queries, input, false)?;
    Ok(queries)
}

fn validate_root_brief(
    proposal: &JsonValue,
    input: &PlannerInput,
    has_bootstrap: bool,
) -> Result<ResearchBrief, String> {
    let values = proposal["queries"]
        .as_array()
        .ok_or_else(|| "root-contract plan omitted queries".to_string())?;
    let mut queries = if values.is_empty() && has_bootstrap {
        Vec::new()
    } else {
        validate_minimal_plan(proposal, input)?
    };
    for query in &mut queries {
        query.dimension_ids = vec!["request.primary".to_string()];
    }
    let request = input.query.chars().take(1_000).collect::<String>();
    let mut normalization_notes = vec![
        "Host preserved the complete request as the immutable root obligation; semantic coverage is judged only by external corpus evaluation."
            .to_string(),
    ];
    if let Some(reason) = proposal
        .pointer("/host_fallback/reason")
        .and_then(JsonValue::as_str)
    {
        normalization_notes.push(format!("Host used the planner fallback: {reason}"));
    }
    Ok(ResearchBrief {
        dimensions: vec![BriefDimension {
            id: "request.primary".to_string(),
            question: request.clone(),
            request_basis: vec![request],
            material: true,
        }],
        queries,
        planning_gaps: Vec::new(),
        normalization_notes,
    })
}

fn strip_minimal_query_fields(query: &JsonValue) -> JsonValue {
    let web_query = query["transport"].as_str() == Some("web");
    serde_json::json!({
        "id": query["id"],
        "text": query["text"],
        "transport": query["transport"],
        "path": if web_query { "" } else { query["path"].as_str().unwrap_or_default() },
        "glob": if web_query { "" } else { query["glob"].as_str().unwrap_or_default() },
        "dimension_ids": [],
        "source_target_ids": [],
    })
}

fn compiler_contract(
    proposal: &JsonValue,
    input: &PlannerInput,
) -> Result<(JsonValue, JsonValue, Vec<AcquisitionQuery>), String> {
    let dimensions = proposal["dimensions"]
        .as_array()
        .ok_or_else(|| "compiler plan omitted dimensions".to_string())?;
    let targets = proposal["source_targets"]
        .as_array()
        .ok_or_else(|| "compiler plan omitted source targets".to_string())?;
    let queries = proposal["queries"]
        .as_array()
        .ok_or_else(|| "compiler plan omitted queries".to_string())?;
    let closed_queries = close_query_budget(queries, input, true)?;
    let decoded_queries = decode_queries(&closed_queries)?;
    validate_queries(&decoded_queries, input, true)?;

    let normalized_targets = targets
        .iter()
        .map(normalize_target)
        .collect::<Result<Vec<_>, String>>()?;
    let spec = serde_json::json!({
        "version": COMPILER_SPEC_VERSION,
        "query": input.query,
        "language": input.report_language,
        "current_date": input.current_date,
        "evidence_scope": input.evidence_scope,
        "dimensions": dimensions,
        "source_targets": normalized_targets,
        "budget": {
            "max_queries": input.budget.max_queries,
            "max_fetches": input.budget.max_acquired_sources,
        }
    });
    let digest =
        a3s::research::compiler::evidence_spec_digest(&spec).map_err(|error| error.to_string())?;
    let plan = serde_json::json!({
        "spec_digest": digest,
        "queries": closed_queries.iter().map(strip_evaluator_query_fields).collect::<Vec<_>>(),
        "planning_gaps": proposal["planning_gaps"],
    });
    a3s::research::compiler::validate_evidence_contract(&spec, &plan)
        .map_err(|error| error.to_string())?;
    Ok((spec, plan, decoded_queries))
}

fn normalize_target(target: &JsonValue) -> Result<JsonValue, String> {
    let kind = target["match_kind"]
        .as_str()
        .ok_or_else(|| "source target omitted match_kind".to_string())?;
    let value = target["match_value"]
        .as_str()
        .ok_or_else(|| "source target omitted match_value".to_string())?;
    let match_policy = if kind == "exploratory" {
        serde_json::json!({
            "kind": "exploratory",
            "selection_goal": value,
        })
    } else {
        serde_json::json!({
            "kind": "named",
            "identity": {
                "kind": kind,
                "value": value,
            }
        })
    };
    Ok(serde_json::json!({
        "id": target["id"],
        "source_family_id": target["source_family_id"],
        "role": target["role"],
        "transport": target["transport"],
        "match_policy": match_policy,
    }))
}

fn strip_evaluator_query_fields(query: &JsonValue) -> JsonValue {
    serde_json::json!({
        "id": query["id"],
        "text": query["text"],
        "transport": query["transport"],
        "mode": query["mode"],
        "dimension_ids": query["dimension_ids"],
        "source_target_ids": query["source_target_ids"],
        "fetch_slots": query["fetch_slots"],
    })
}

fn close_query_budget(
    values: &[JsonValue],
    input: &PlannerInput,
    compiler: bool,
) -> Result<Vec<JsonValue>, String> {
    if values.is_empty() || values.len() > input.budget.max_queries {
        return Err("planned query count is outside the shared budget".to_string());
    }

    let mut allocations = values
        .iter()
        .map(|query| {
            if compiler {
                query["source_target_ids"]
                    .as_array()
                    .map(Vec::len)
                    .filter(|count| *count > 0)
                    .ok_or_else(|| {
                        "compiler query omitted source targets before Host allocation".to_string()
                    })
            } else {
                Ok(1)
            }
        })
        .collect::<Result<Vec<_>, String>>()?;
    let required = allocations
        .iter()
        .try_fold(0usize, |total, allocation| total.checked_add(*allocation));
    let required = required.ok_or_else(|| "planned source allocation overflowed".to_string())?;
    if required > input.budget.max_acquired_sources {
        return Err(format!(
            "planned target edges require {required} sources but the shared cap is {}",
            input.budget.max_acquired_sources
        ));
    }

    let mut remaining = input.budget.max_acquired_sources - required;
    let mut index = 0usize;
    let allocation_count = allocations.len();
    while remaining > 0 {
        allocations[index % allocation_count] += 1;
        index += 1;
        remaining -= 1;
    }

    values
        .iter()
        .zip(allocations)
        .map(|(query, fetch_slots)| {
            let mut query = query
                .as_object()
                .cloned()
                .ok_or_else(|| "planner returned a non-object query".to_string())?;
            query.insert("fetch_slots".to_string(), JsonValue::from(fetch_slots));
            Ok(JsonValue::Object(query))
        })
        .collect()
}

fn decode_queries(values: &[JsonValue]) -> Result<Vec<AcquisitionQuery>, String> {
    values
        .iter()
        .map(|value| {
            serde_json::from_value::<AcquisitionQuery>(value.clone())
                .map_err(|error| format!("invalid acquisition query: {error}"))
        })
        .collect()
}

fn validate_queries(
    queries: &[AcquisitionQuery],
    input: &PlannerInput,
    compiler: bool,
) -> Result<(), String> {
    if queries.is_empty() || queries.len() > input.budget.max_queries {
        return Err("planned query count is outside the shared budget".to_string());
    }
    let total_slots = queries
        .iter()
        .try_fold(0usize, |total, query| total.checked_add(query.fetch_slots))
        .ok_or_else(|| "planned fetch allocation overflowed".to_string())?;
    if total_slots > input.budget.max_acquired_sources {
        return Err("planned source allocation exceeds the shared budget".to_string());
    }
    let mut ids = BTreeSet::new();
    for query in queries {
        if !ids.insert(query.id.as_str()) {
            return Err(format!("duplicate planned query `{}`", query.id));
        }
        if !input.evidence_scope.permits(query.transport) {
            return Err(format!(
                "query `{}` is outside the evidence scope",
                query.id
            ));
        }
        match query.transport {
            AcquisitionTransport::Web => {
                if !query.path.is_empty() || !query.glob.is_empty() {
                    return Err(format!(
                        "web query `{}` contains workspace-only fields",
                        query.id
                    ));
                }
            }
            AcquisitionTransport::Workspace => {
                if query.path.starts_with('/') || query.path.split('/').any(|part| part == "..") {
                    return Err(format!("workspace query `{}` has an unsafe path", query.id));
                }
                regex::Regex::new(&query.text).map_err(|error| {
                    format!(
                        "workspace query `{}` is not a valid regex: {error}",
                        query.id
                    )
                })?;
            }
        }
        if compiler && (query.dimension_ids.is_empty() || query.source_target_ids.is_empty()) {
            return Err(format!(
                "compiler query `{}` omitted dimension or target edges",
                query.id
            ));
        }
        if !compiler && (!query.dimension_ids.is_empty() || !query.source_target_ids.is_empty()) {
            return Err(format!(
                "minimal query `{}` contains compiler-only identities",
                query.id
            ));
        }
    }
    Ok(())
}

pub(super) fn target_index(spec: &JsonValue) -> BTreeMap<String, JsonValue> {
    spec["source_targets"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|target| {
            target["id"]
                .as_str()
                .map(|id| (id.to_string(), target.clone()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::super::corpus::PlannerBudget;
    use super::*;

    fn input() -> PlannerInput {
        PlannerInput {
            schema: "test".to_string(),
            query: "compare alpha and beta".to_string(),
            report_language: "en".to_string(),
            current_date: "2026-07-22".to_string(),
            display_utc_offset: "+08:00".to_string(),
            evidence_scope: EvidenceScope::Web,
            budget: PlannerBudget {
                max_queries: 4,
                max_acquired_sources: 8,
            },
        }
    }

    fn query(id: &str, target_ids: &[&str]) -> JsonValue {
        serde_json::json!({
            "id": id,
            "text": format!("{id} evidence"),
            "transport": "web",
            "path": "",
            "glob": "",
            "mode": "discovery",
            "dimension_ids": if target_ids.is_empty() { Vec::<String>::new() } else { vec!["d1".to_string()] },
            "source_target_ids": target_ids,
        })
    }

    #[test]
    fn host_allocates_the_complete_minimal_source_budget() {
        let queries = vec![query("q1", &[]), query("q2", &[]), query("q3", &[])];
        let closed = close_query_budget(&queries, &input(), false).expect("Host allocation");
        let slots = closed
            .iter()
            .map(|query| query["fetch_slots"].as_u64().expect("fetch slots"))
            .collect::<Vec<_>>();
        assert_eq!(slots, [3, 3, 2]);
    }

    #[test]
    fn minimal_plan_clears_model_authored_compiler_identities_locally() {
        let proposal = serde_json::json!({
            "queries": [{
                "id": "q1",
                "text": "alpha official status",
                "transport": "web",
                "path": "/ignored",
                "glob": "**/*",
                "mode": "exact",
                "dimension_ids": ["invented.dimension"],
                "source_target_ids": ["invented.target"]
            }]
        });

        let queries = validate_minimal_plan(&proposal, &input()).expect("normalized plan");

        assert_eq!(queries.len(), 1);
        assert!(queries[0].dimension_ids.is_empty());
        assert!(queries[0].source_target_ids.is_empty());
        assert!(queries[0].path.is_empty());
        assert!(queries[0].glob.is_empty());
        assert_eq!(queries[0].fetch_slots, 8);
    }

    #[test]
    fn acquisition_candidates_have_distinct_planning_contracts() {
        let input = input();
        let minimal = planner_schema(&input, EvaluationStrategy::Minimal);
        let brief = planner_schema(&input, EvaluationStrategy::Brief);

        assert!(minimal.pointer("/properties/queries").is_some());
        assert!(minimal.pointer("/properties/dimensions").is_none());
        assert!(minimal
            .pointer("/properties/queries/items/properties/path")
            .is_none());
        assert!(brief.pointer("/properties/queries").is_some());
        assert!(brief.pointer("/properties/dimensions").is_some());
        assert!(brief
            .pointer("/properties/queries/items/properties/preferred_sources")
            .is_some());
        let encoded = brief.to_string();
        assert!(!encoded.contains("complete"));
        assert!(!encoded.contains("answered"));
        assert!(!encoded.contains("supported"));
    }

    #[test]
    fn planner_receives_the_real_bootstrap_observation() {
        let input = input();
        let observation = serde_json::json!({
            "attempt": { "status": "completed" },
            "candidates": [{ "title": "Observed primary record" }]
        });

        for strategy in [EvaluationStrategy::Minimal, EvaluationStrategy::Brief] {
            let prompt = planner_prompt(&input, strategy, Some(&observation))
                .expect("planner prompt with bootstrap observation");
            assert!(prompt.contains("Observed primary record"));
            assert!(prompt.contains("BOOTSTRAP_OBSERVATION="));
        }
    }

    #[test]
    fn candidate_schema_preserves_a_bounded_proposal_for_host_budget_closure() {
        let input = input();
        for strategy in [EvaluationStrategy::Minimal, EvaluationStrategy::Brief] {
            let schema = planner_schema(&input, strategy);
            assert_eq!(
                schema["properties"]["queries"]["maxItems"],
                input.budget.max_queries
            );
        }
    }

    #[test]
    fn host_planner_fallback_preserves_progress_for_minimal_and_brief_candidates() {
        let input = input();
        let minimal = PlanningProposal {
            strategy: EvaluationStrategy::Minimal,
            planner_input: input.clone(),
            prompt: "planner prompt".to_string(),
            proposal: host_fallback_proposal(
                &input,
                EvaluationStrategy::Minimal,
                false,
                "planner host timeout",
            ),
            elapsed_ms: 60_000,
            prompt_tokens: 0,
            completion_tokens: 0,
            repair_rounds: 0,
            mode_used: "HostFallback".to_string(),
            reserved_query_count: 0,
        };
        let minimal = validate_proposal(minimal).expect("minimal fallback");
        assert_eq!(minimal.queries.len(), 1);
        assert_eq!(minimal.queries[0].text, input.query);

        let brief = PlanningProposal {
            strategy: EvaluationStrategy::Brief,
            planner_input: input.clone(),
            prompt: "planner prompt".to_string(),
            proposal: host_fallback_proposal(
                &input,
                EvaluationStrategy::Brief,
                true,
                "planner host timeout",
            ),
            elapsed_ms: 60_000,
            prompt_tokens: 0,
            completion_tokens: 0,
            repair_rounds: 0,
            mode_used: "HostFallback".to_string(),
            reserved_query_count: 1,
        };
        let brief = validate_proposal(brief).expect("brief fallback");
        let brief_contract = brief.brief.expect("fallback brief");
        assert_eq!(brief_contract.dimensions.len(), 1);
        assert_eq!(brief_contract.dimensions[0].request_basis, [input.query]);
        assert!(brief.queries.is_empty());
    }

    #[test]
    fn workspace_planner_fallback_is_independent_of_query_text() {
        let mut first = input();
        first.evidence_scope = EvidenceScope::Workspace;
        first.query = "alpha release status".to_string();
        let mut second = first.clone();
        second.query = "完全不同的研究请求".to_string();

        let first = host_fallback_proposal(
            &first,
            EvaluationStrategy::Minimal,
            false,
            "typed planner failure",
        );
        let second = host_fallback_proposal(
            &second,
            EvaluationStrategy::Minimal,
            false,
            "typed planner failure",
        );

        assert_eq!(first["queries"][0]["text"], r"\S");
        assert_eq!(
            first["queries"][0]["text"], second["queries"][0]["text"],
            "fallback search control must not depend on query prose"
        );
    }

    #[test]
    fn compiler_allocation_covers_targets_without_model_authored_slots() {
        let queries = vec![
            query("q1", &["target.alpha", "target.beta"]),
            query("q2", &["target.gamma"]),
        ];
        let closed = close_query_budget(&queries, &input(), true).expect("Host allocation");
        let slots = closed
            .iter()
            .map(|query| query["fetch_slots"].as_u64().expect("fetch slots"))
            .collect::<Vec<_>>();
        assert_eq!(slots, [5, 3]);
    }

    #[test]
    fn compiler_rejects_target_edges_that_cannot_fit_the_shared_cap() {
        let targets = (1..=9)
            .map(|index| format!("target.{index}"))
            .collect::<Vec<_>>();
        let query = serde_json::json!({
            "id": "q1",
            "text": "evidence",
            "transport": "web",
            "path": "",
            "glob": "",
            "mode": "discovery",
            "dimension_ids": ["d1"],
            "source_target_ids": targets,
        });
        let error = close_query_budget(&[query], &input(), true).expect_err("over budget");
        assert!(error.contains("require 9 sources"), "{error}");
    }
}
