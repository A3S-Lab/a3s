use a3s_acl::{Block, Value};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct LiveBudget {
    pub(super) planner_generations: usize,
    pub(super) feedback_generations: usize,
    pub(super) verifier_generations: usize,
    pub(super) report_generations: usize,
    pub(super) max_queries: usize,
    pub(super) max_acquired_sources: usize,
    pub(super) synthesis_packet_chars: usize,
    pub(super) public_excerpt_chars: usize,
    pub(super) wall_clock_ms: u64,
    pub(super) planner_timeout_ms: u64,
    pub(super) verifier_timeout_ms: u64,
    pub(super) search_timeout_ms: u64,
    pub(super) fetch_timeout_ms: u64,
    pub(super) report_timeout_ms: u64,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum EvidenceScope {
    Web,
    Workspace,
    WebAndWorkspace,
}

impl EvidenceScope {
    pub(super) fn permits(self, transport: AcquisitionTransport) -> bool {
        matches!(
            (self, transport),
            (Self::Web, AcquisitionTransport::Web)
                | (Self::Workspace, AcquisitionTransport::Workspace)
                | (Self::WebAndWorkspace, _)
        )
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AcquisitionTransport {
    Web,
    Workspace,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct EvaluationDimension {
    pub(super) id: String,
    pub(super) question: String,
    pub(super) material: bool,
    pub(super) acceptable: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct EvaluationSourceRequirement {
    pub(super) id: String,
    pub(super) description: String,
    pub(super) authority: String,
    pub(super) transport: AcquisitionTransport,
    pub(super) dimension_ids: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct EvaluationExpectations {
    pub(super) dimensions: Vec<EvaluationDimension>,
    pub(super) source_requirements: Vec<EvaluationSourceRequirement>,
    pub(super) guardrails: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct LiveCase {
    pub(super) id: String,
    pub(super) query: String,
    pub(super) report_language: String,
    pub(super) evidence_scope: EvidenceScope,
    pub(super) expected_terminal: String,
    pub(super) expectations: EvaluationExpectations,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct PlannerBudget {
    pub(super) max_queries: usize,
    pub(super) max_acquired_sources: usize,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct PlannerInput {
    pub(super) schema: String,
    pub(super) query: String,
    pub(super) report_language: String,
    pub(super) current_date: String,
    pub(super) display_utc_offset: String,
    pub(super) evidence_scope: EvidenceScope,
    pub(super) budget: PlannerBudget,
}

impl LiveCase {
    pub(super) fn planner_input(
        &self,
        current_date: &str,
        display_utc_offset: &str,
        budget: &LiveBudget,
    ) -> PlannerInput {
        PlannerInput {
            schema: "a3s/deep-research-planner-input/v2".to_string(),
            query: self.query.clone(),
            report_language: self.report_language.clone(),
            current_date: current_date.to_string(),
            display_utc_offset: display_utc_offset.to_string(),
            evidence_scope: self.evidence_scope,
            budget: PlannerBudget {
                max_queries: budget.max_queries,
                max_acquired_sources: budget.max_acquired_sources,
            },
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub(super) struct LiveCorpus {
    pub(super) schema: String,
    pub(super) version: String,
    pub(super) runs_per_case: usize,
    pub(super) artifact_formats: Vec<String>,
    pub(super) budget: LiveBudget,
    pub(super) cases: Vec<LiveCase>,
}

impl LiveCorpus {
    pub(super) fn case(&self, id: &str) -> Option<&LiveCase> {
        self.cases.iter().find(|case| case.id == id)
    }

    pub(super) fn rotated_cases(&self, run_index: usize) -> Vec<&LiveCase> {
        if self.cases.is_empty() {
            return Vec::new();
        }
        let offset = run_index.saturating_sub(1) % self.cases.len();
        self.cases
            .iter()
            .cycle()
            .skip(offset)
            .take(self.cases.len())
            .collect()
    }
}

pub(super) fn load_live_corpus() -> Result<LiveCorpus, String> {
    let source = std::fs::read_to_string(live_corpus_path())
        .map_err(|error| format!("read live corpus: {error}"))?;
    let document =
        a3s_acl::parse_acl(&source).map_err(|error| format!("parse live corpus ACL: {error}"))?;
    if document.blocks.len() != 1 {
        return Err("live corpus must contain exactly one root block".to_string());
    }
    let corpus = &document.blocks[0];
    if corpus.name != "corpus" || corpus.labels != ["deep-research-live-v1"] {
        return Err("live corpus root identity is invalid".to_string());
    }
    let budget = unique_unlabeled_block(corpus, "budget")?;
    let budget = LiveBudget {
        planner_generations: integer(budget, "planner_generations")?,
        feedback_generations: integer(budget, "feedback_generations")?,
        verifier_generations: integer(budget, "verifier_generations")?,
        report_generations: integer(budget, "report_generations")?,
        max_queries: integer(budget, "max_queries")?,
        max_acquired_sources: integer(budget, "max_acquired_sources")?,
        synthesis_packet_chars: integer(budget, "synthesis_packet_chars")?,
        public_excerpt_chars: integer(budget, "public_excerpt_chars")?,
        wall_clock_ms: integer(budget, "wall_clock_ms")? as u64,
        planner_timeout_ms: integer(budget, "planner_timeout_ms")? as u64,
        verifier_timeout_ms: integer(budget, "verifier_timeout_ms")? as u64,
        search_timeout_ms: integer(budget, "search_timeout_ms")? as u64,
        fetch_timeout_ms: integer(budget, "fetch_timeout_ms")? as u64,
        report_timeout_ms: integer(budget, "report_timeout_ms")? as u64,
    };
    if budget.planner_generations != 1
        || budget.feedback_generations != 0
        || budget.verifier_generations != 0
        || budget.report_generations != 1
        || budget.max_queries == 0
        || budget.max_acquired_sources == 0
        || budget.synthesis_packet_chars == 0
        || budget.public_excerpt_chars == 0
    {
        return Err(
            "live corpus requires one query planner, no feedback or runtime reviewer, one report or atomic-synthesis call, and positive caps"
                .to_string(),
        );
    }

    let mut cases = Vec::new();
    let mut case_ids = BTreeSet::new();
    for block in corpus.blocks.iter().filter(|block| block.name == "case") {
        let id = one_label(block)?;
        if !case_ids.insert(id.to_string()) {
            return Err(format!("duplicate live case `{id}`"));
        }
        cases.push(parse_case(id, block)?);
    }
    if cases.is_empty() {
        return Err("live corpus contains no case".to_string());
    }

    Ok(LiveCorpus {
        schema: text(corpus, "schema")?.to_string(),
        version: text(corpus, "version")?.to_string(),
        runs_per_case: integer(corpus, "runs_per_case")?,
        artifact_formats: strings(corpus, "artifact_formats")?,
        budget,
        cases,
    })
}

fn parse_case(id: &str, block: &Block) -> Result<LiveCase, String> {
    if block.blocks.iter().any(|child| child.name == "budget") {
        return Err(format!("{id}: per-case budget is forbidden"));
    }
    let evidence_scope = match text(block, "evidence_scope")? {
        "web" => EvidenceScope::Web,
        "local_only" => EvidenceScope::Workspace,
        "web_and_workspace" => EvidenceScope::WebAndWorkspace,
        value => return Err(format!("{id}: unsupported evidence scope `{value}`")),
    };
    let mut dimensions = Vec::new();
    let mut dimension_ids = BTreeSet::new();
    for dimension in block
        .blocks
        .iter()
        .filter(|child| child.name == "dimension")
    {
        let dimension_id = one_label(dimension)?;
        if !dimension_ids.insert(dimension_id.to_string()) {
            return Err(format!("{id}: duplicate dimension `{dimension_id}`"));
        }
        dimensions.push(EvaluationDimension {
            id: dimension_id.to_string(),
            question: text(dimension, "question")?.to_string(),
            material: boolean(dimension, "material")?,
            acceptable: strings(dimension, "acceptable")?,
        });
    }
    let mut source_requirements = Vec::new();
    let mut requirement_ids = BTreeSet::new();
    for requirement in block
        .blocks
        .iter()
        .filter(|child| child.name == "source_requirement")
    {
        let requirement_id = one_label(requirement)?;
        if !requirement_ids.insert(requirement_id.to_string()) {
            return Err(format!(
                "{id}: duplicate source requirement `{requirement_id}`"
            ));
        }
        let transport = match text(requirement, "transport")? {
            "web" => AcquisitionTransport::Web,
            "workspace" => AcquisitionTransport::Workspace,
            value => return Err(format!("{id}: unsupported transport `{value}`")),
        };
        if !evidence_scope.permits(transport) {
            return Err(format!(
                "{id}: source requirement `{requirement_id}` is outside its evidence scope"
            ));
        }
        let bound_dimensions = strings(requirement, "dimensions")?;
        if bound_dimensions
            .iter()
            .any(|dimension_id| !dimension_ids.contains(dimension_id))
        {
            return Err(format!(
                "{id}: source requirement `{requirement_id}` references an unknown dimension"
            ));
        }
        source_requirements.push(EvaluationSourceRequirement {
            id: requirement_id.to_string(),
            description: text(requirement, "description")?.to_string(),
            authority: text(requirement, "authority")?.to_string(),
            transport,
            dimension_ids: bound_dimensions,
        });
    }
    if dimensions.is_empty() || source_requirements.is_empty() {
        return Err(format!("{id}: evaluator expectations are incomplete"));
    }
    Ok(LiveCase {
        id: id.to_string(),
        query: text(block, "query")?.to_string(),
        report_language: text(block, "report_language")?.to_string(),
        evidence_scope,
        expected_terminal: text(block, "expected_terminal")?.to_string(),
        expectations: EvaluationExpectations {
            dimensions,
            source_requirements,
            guardrails: strings(block, "guardrails")?,
        },
    })
}

fn live_corpus_path() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deep_research_eval/live.acl")
}

fn unique_unlabeled_block<'a>(parent: &'a Block, name: &str) -> Result<&'a Block, String> {
    let blocks = parent
        .blocks
        .iter()
        .filter(|block| block.name == name)
        .collect::<Vec<_>>();
    if blocks.len() != 1 || !blocks[0].labels.is_empty() {
        return Err(format!("expected one unlabeled `{name}` block"));
    }
    Ok(blocks[0])
}

fn one_label(block: &Block) -> Result<&str, String> {
    match block.labels.as_slice() {
        [label] if !label.trim().is_empty() => Ok(label),
        _ => Err(format!("{} requires exactly one label", block.name)),
    }
}

fn text<'a>(block: &'a Block, key: &str) -> Result<&'a str, String> {
    block
        .attributes
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("{} {:?} requires text `{key}`", block.name, block.labels))
}

fn boolean(block: &Block, key: &str) -> Result<bool, String> {
    block
        .attributes
        .get(key)
        .and_then(Value::as_bool)
        .ok_or_else(|| format!("{} {:?} requires bool `{key}`", block.name, block.labels))
}

fn integer(block: &Block, key: &str) -> Result<usize, String> {
    let value = block
        .attributes
        .get(key)
        .and_then(Value::as_number)
        .ok_or_else(|| format!("{} {:?} requires number `{key}`", block.name, block.labels))?;
    if !value.is_finite() || value < 0.0 || value.fract() != 0.0 || value > usize::MAX as f64 {
        return Err(format!(
            "{} {:?} requires non-negative integer `{key}`",
            block.name, block.labels
        ));
    }
    Ok(value as usize)
}

fn strings(block: &Block, key: &str) -> Result<Vec<String>, String> {
    let Some(Value::List(values)) = block.attributes.get(key) else {
        return Err(format!(
            "{} {:?} requires list `{key}`",
            block.name, block.labels
        ));
    };
    values
        .iter()
        .map(|value| {
            value
                .as_str()
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .ok_or_else(|| {
                    format!(
                        "{} {:?} `{key}` requires text values",
                        block.name, block.labels
                    )
                })
        })
        .collect()
}
