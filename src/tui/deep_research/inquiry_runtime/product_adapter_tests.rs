use super::*;
use a3s_code_core::state_graph::{FileGraphEventStore, GraphEventStore};
use a3s_deep_research::engine::{
    DeepResearchEngine, GenerationRequest, GenerationStage, PublicationPort, PublicationRequest,
    StructuredGenerationPort, WorkflowExecutionPort, WorkflowOutput, WorkflowRequest,
    WorkflowStage,
};
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Mutex;
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use super::evidence_first_tests::product_adapter_fixture_session;
use crate::tui::ResearchOutcome;

#[path = "product_adapter_tests/fixture.rs"]
mod fixture;
use fixture::load_product_replays;
#[path = "product_adapter_tests/fault_matrix.rs"]
mod fault_matrix;
#[path = "product_adapter_tests/publication_fault_matrix.rs"]
mod publication_fault_matrix;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureLanguage {
    English,
    Chinese,
}

impl FixtureLanguage {
    fn code(self) -> &'static str {
        match self {
            Self::English => "en",
            Self::Chinese => "zh",
        }
    }

    fn labels(self) -> Value {
        match self {
            Self::English => serde_json::json!({
                "answer": "Direct Answer",
                "findings": "Findings",
                "recommendations": "Recommendation",
                "limitations": "Limitations",
                "evidence_boundary": "No conclusion is published beyond the fetched evidence.",
                "sources": "Sources",
                "contradiction": "Contradiction",
                "inference": "Inference",
                "basis": "Basis",
                "derivation": "Derivation",
            }),
            Self::Chinese => serde_json::json!({
                "answer": "直接回答",
                "findings": "研究发现",
                "recommendations": "建议",
                "limitations": "限制",
                "evidence_boundary": "本报告不发布超出已获取证据的结论。",
                "sources": "来源",
                "contradiction": "证据矛盾",
                "inference": "推论",
                "basis": "依据",
                "derivation": "推导",
            }),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureAuthority {
    Primary,
    LocalPrimary,
}

impl FixtureAuthority {
    fn is_workspace(self) -> bool {
        self == Self::LocalPrimary
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum FixtureFault {
    MalformedEvidenceExtraction { dimension_id: String },
    ReportGenerationTimeout,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureClaimDisposition {
    Supported,
    SupportedIfRecovered,
    DerivedAllowed,
    Forbidden,
}

impl FixtureClaimDisposition {
    fn is_active(self) -> bool {
        self != Self::Forbidden
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureClaimKind {
    Fact,
    Inference,
    Recommendation,
}

impl FixtureClaimKind {
    fn code(self) -> &'static str {
        match self {
            Self::Fact => "fact",
            Self::Inference => "inference",
            Self::Recommendation => "recommendation",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureClaimPlacement {
    DirectAnswer,
    Finding,
}

impl FixtureClaimPlacement {
    fn code(self) -> &'static str {
        match self {
            Self::DirectAnswer => "direct_answer",
            Self::Finding => "finding",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FixtureRelationKind {
    Contradicts,
}

impl FixtureRelationKind {
    fn code(self) -> &'static str {
        match self {
            Self::Contradicts => "contradicts",
        }
    }
}

#[derive(Clone, Debug)]
struct FixtureDimension {
    id: String,
    question: String,
    material: bool,
}

#[derive(Clone, Debug)]
struct FixtureSource {
    id: String,
    title: String,
    url: String,
    path: String,
    authority: FixtureAuthority,
    content: String,
}

#[derive(Clone, Debug)]
struct FixtureClaim {
    id: String,
    disposition: FixtureClaimDisposition,
    dimension_id: String,
    kind: FixtureClaimKind,
    placement: FixtureClaimPlacement,
    text: String,
    source_ids: Vec<String>,
    basis_claim_ids: Vec<String>,
    derivation: Option<String>,
}

impl FixtureClaim {
    fn analysis_role(&self) -> &'static str {
        if self.placement == FixtureClaimPlacement::DirectAnswer {
            return "conclusion";
        }
        match self.kind {
            FixtureClaimKind::Fact => "evidence",
            FixtureClaimKind::Inference => "explanation",
            FixtureClaimKind::Recommendation => "implication",
        }
    }
}

#[derive(Clone, Debug)]
struct FixtureRelation {
    id: String,
    dimension_id: String,
    kind: FixtureRelationKind,
    claim_ids: Vec<String>,
}

#[derive(Clone, Debug)]
struct ProductReplay {
    id: String,
    query: String,
    language: FixtureLanguage,
    dimensions: Vec<FixtureDimension>,
    sources: Vec<FixtureSource>,
    claims: Vec<FixtureClaim>,
    relations: Vec<FixtureRelation>,
    fault: Option<FixtureFault>,
}

impl ProductReplay {
    fn expected_publication(&self) -> DeepResearchEvidenceFirstPublication {
        match self.fault {
            Some(FixtureFault::ReportGenerationTimeout) => {
                DeepResearchEvidenceFirstPublication::SourceBacked
            }
            Some(FixtureFault::MalformedEvidenceExtraction { .. }) => {
                DeepResearchEvidenceFirstPublication::Qualified
            }
            None => DeepResearchEvidenceFirstPublication::Synthesized,
        }
    }

    fn evidence_scope(&self) -> super::super::DeepResearchEvidenceScope {
        // The product adapter exposes a combined web/workspace scope. The
        // frozen port still preserves each source's exact transport identity.
        super::super::DeepResearchEvidenceScope::WebAndWorkspace
    }

    fn malformed_dimension(&self) -> Option<&str> {
        match self.fault.as_ref() {
            Some(FixtureFault::MalformedEvidenceExtraction { dimension_id }) => Some(dimension_id),
            _ => None,
        }
    }

    fn admitted_claims(&self) -> impl Iterator<Item = &FixtureClaim> {
        let malformed_dimension = self.malformed_dimension();
        self.claims.iter().filter(move |claim| {
            claim.disposition.is_active()
                && Some(claim.dimension_id.as_str()) != malformed_dimension
        })
    }
}

struct FrozenPorts<'a> {
    replay: &'a ProductReplay,
}

#[async_trait::async_trait]
impl StructuredGenerationPort for FrozenPorts<'_> {
    async fn generate_object(&self, request: GenerationRequest) -> Result<Value, String> {
        match request.stage {
            GenerationStage::Planning => Ok(planner_outline(self.replay)),
            GenerationStage::Report
                if self.replay.fault == Some(FixtureFault::ReportGenerationTimeout) =>
            {
                Err("frozen typed report timeout".to_string())
            }
            GenerationStage::Report => Ok(report_proposal(self.replay)),
            GenerationStage::Editorial => Ok(editorial_plan(self.replay)),
        }
    }
}

#[async_trait::async_trait]
impl WorkflowExecutionPort for FrozenPorts<'_> {
    async fn execute_workflow(&self, request: WorkflowRequest) -> Result<WorkflowOutput, String> {
        Ok(match request.stage {
            WorkflowStage::Bootstrap => bootstrap_output(self.replay),
            WorkflowStage::PlannedRetrieval => planned_output(self.replay),
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublishedKind {
    Synthesized,
    Qualified,
    SourceBacked,
    NoEvidence,
}

struct TimedProductPublication<'a> {
    delegate: &'a A3sDeepResearchRuntime<'a>,
    started: Instant,
    publications: Mutex<Vec<(PublishedKind, u64)>>,
}

impl TimedProductPublication<'_> {
    fn records(&self) -> Vec<(PublishedKind, u64)> {
        self.publications
            .lock()
            .expect("publication timing lock")
            .clone()
    }
}

#[async_trait::async_trait]
impl PublicationPort for TimedProductPublication<'_> {
    async fn publish(
        &self,
        request: PublicationRequest,
    ) -> Result<ResearchReportArtifacts, String> {
        let kind = match &request {
            PublicationRequest::SourceBacked { .. } => PublishedKind::SourceBacked,
            PublicationRequest::Synthesized { publication, .. } => match publication {
                DeepResearchEvidenceFirstPublication::Synthesized => PublishedKind::Synthesized,
                DeepResearchEvidenceFirstPublication::Qualified => PublishedKind::Qualified,
                DeepResearchEvidenceFirstPublication::SourceBacked => PublishedKind::SourceBacked,
                DeepResearchEvidenceFirstPublication::NoEvidence => PublishedKind::NoEvidence,
            },
            PublicationRequest::NoEvidence { .. } => PublishedKind::NoEvidence,
        };
        let artifacts = self.delegate.publish(request).await?;
        self.publications
            .lock()
            .expect("publication timing lock")
            .push((kind, self.started.elapsed().as_millis() as u64));
        Ok(artifacts)
    }
}

#[tokio::test]
async fn frozen_corpus_survives_the_persisted_product_adapter_with_end_to_end_timing() {
    let mut outcomes = Vec::new();

    for replay in load_product_replays() {
        let workspace = tempfile::tempdir().expect("persisted product replay workspace");
        materialize_workspace_sources(workspace.path(), &replay);
        let (_agent, session) = product_adapter_fixture_session(workspace.path()).await;
        let run_id = format!("persisted-adapter-{}", replay.id.to_ascii_lowercase());
        let args = workflow_args(&replay, &run_id);
        let evaluator_started_at_ms = unix_time_ms();
        let evaluator_started = Instant::now();
        let run_clock = EvidenceFirstRunClock::initialize(&session, &args)
            .await
            .unwrap_or_else(|error| panic!("{}: initialize product journal: {error}", replay.id));
        let (progress_tx, _progress_rx) = mpsc::channel(PROGRESS_CHANNEL_CAPACITY);
        let product_runtime = A3sDeepResearchRuntime {
            session: &session,
            progress_tx: &progress_tx,
            run_clock: &run_clock,
        };
        let publication = TimedProductPublication {
            delegate: &product_runtime,
            started: evaluator_started,
            publications: Mutex::new(Vec::new()),
        };
        let frozen_ports = FrozenPorts { replay: &replay };
        let engine =
            DeepResearchEngine::new(&frozen_ports, &frozen_ports, &publication, &product_runtime);

        let run = engine
            .execute(args)
            .await
            .unwrap_or_else(|error| panic!("{}: product engine replay: {error}", replay.id));
        let expected_publication = replay.expected_publication();
        assert_eq!(run.publication, expected_publication, "{}", replay.id);

        let requested_outcome = publication_outcome(expected_publication);
        let workflow_output = run.output_json();
        let settled =
            crate::tui::settle_deep_research_cli_run(crate::tui::DeepResearchCliSettlement {
                workspace: workspace.path(),
                run_id: &run_id,
                query: &replay.query,
                workflow_succeeded: true,
                workflow_output: &workflow_output,
                workflow_metadata: None,
                requested_outcome,
                artifacts: &run.artifacts,
                artifact_authority:
                    crate::tui::DeepResearchTerminalArtifactAuthority::ValidatedPublication,
            })
            .await
            .unwrap_or_else(|error| panic!("{}: settle product adapter: {error}", replay.id));
        assert_eq!(settled, requested_outcome, "{}", replay.id);

        let recovered =
            super::super::deep_research_artifacts::recover_deep_research_publication_receipt(
                workspace.path(),
                &replay.query,
                &run_id,
            )
            .unwrap_or_else(|error| panic!("{}: recover publication receipt: {error}", replay.id))
            .unwrap_or_else(|| panic!("{}: missing publication receipt", replay.id));
        assert_eq!(recovered.publication, expected_publication, "{}", replay.id);
        let accepted_claim_count =
            usize_metric(&run.output, "/publication/quality/accepted_claim_count");
        let accepted_relation_count =
            usize_metric(&run.output, "/publication/quality/accepted_relation_count");
        let accepted_derivation_count = usize_metric(
            &run.output,
            "/publication/quality/accepted_derivation_count",
        );
        let accepted_basis_edge_count = usize_metric(
            &run.output,
            "/publication/quality/accepted_basis_edge_count",
        );
        let accepted_gap_count =
            usize_metric(&run.output, "/publication/quality/accepted_gap_count");
        assert_eq!(
            recovered.quality.accepted_claim_count, accepted_claim_count,
            "{}",
            replay.id
        );
        assert_eq!(
            recovered.quality.accepted_relation_count, accepted_relation_count,
            "{}",
            replay.id
        );
        assert_eq!(
            recovered.quality.accepted_derivation_count, accepted_derivation_count,
            "{}",
            replay.id
        );
        assert_eq!(
            recovered.quality.accepted_basis_edge_count, accepted_basis_edge_count,
            "{}",
            replay.id
        );
        assert_eq!(
            recovered.quality.accepted_gap_count, accepted_gap_count,
            "{}",
            replay.id
        );

        let markdown = std::fs::read_to_string(&run.artifacts.markdown)
            .unwrap_or_else(|error| panic!("{}: read product Markdown: {error}", replay.id));
        if expected_publication == DeepResearchEvidenceFirstPublication::SourceBacked {
            assert_eq!(
                (
                    accepted_claim_count,
                    accepted_relation_count,
                    accepted_derivation_count,
                    accepted_basis_edge_count,
                    accepted_gap_count,
                ),
                (0, 0, 0, 0, 0),
                "{}: source-backed fallback cannot claim a synthesized graph",
                replay.id
            );
        } else {
            let admitted_claims = replay.admitted_claims().collect::<Vec<_>>();
            assert_eq!(
                accepted_claim_count,
                admitted_claims.len(),
                "{}: admitted claim count diverged from the closed fixture graph",
                replay.id
            );
            assert_eq!(
                accepted_relation_count,
                replay.relations.len(),
                "{}: typed relations were not preserved",
                replay.id
            );
            assert_eq!(
                accepted_derivation_count,
                admitted_claims
                    .iter()
                    .filter(|claim| {
                        claim.kind == FixtureClaimKind::Inference && claim.derivation.is_some()
                    })
                    .count(),
                "{}: reproducible derivations were not preserved",
                replay.id
            );
            assert_eq!(
                accepted_basis_edge_count,
                admitted_claims
                    .iter()
                    .map(|claim| claim.basis_claim_ids.len())
                    .sum::<usize>(),
                "{}: exact basis edges were not preserved",
                replay.id
            );
            assert_eq!(
                accepted_gap_count > 0,
                replay.malformed_dimension().is_some(),
                "{}: typed gap state diverged from the injected fault",
                replay.id
            );
            for claim in admitted_claims {
                assert!(
                    markdown.contains(&claim.text),
                    "{}: admitted claim `{}` disappeared from the product artifact",
                    replay.id,
                    claim.id
                );
            }
            for claim in replay
                .claims
                .iter()
                .filter(|claim| !claim.disposition.is_active())
            {
                assert!(
                    !markdown.contains(&claim.text),
                    "{}: forbidden claim `{}` leaked into the product artifact",
                    replay.id,
                    claim.id
                );
            }
        }

        let journal = crate::tui::deep_research_state_journal::DeepResearchStateJournal::open(
            workspace.path(),
            &run_id,
        )
        .await
        .unwrap_or_else(|error| panic!("{}: reopen product journal: {error}", replay.id))
        .unwrap_or_else(|| panic!("{}: missing product journal", replay.id));
        let projection = journal
            .projection()
            .unwrap_or_else(|error| panic!("{}: product projection: {error}", replay.id));
        assert_eq!(projection.outcome, requested_outcome, "{}", replay.id);
        assert_eq!(
            projection.claim_count, accepted_claim_count,
            "{}",
            replay.id
        );
        assert_eq!(
            projection.accepted_relation_count, accepted_relation_count,
            "{}",
            replay.id
        );
        assert_eq!(
            projection.accepted_derivation_count, accepted_derivation_count,
            "{}",
            replay.id
        );
        assert_eq!(
            projection.accepted_basis_edge_count, accepted_basis_edge_count,
            "{}",
            replay.id
        );
        assert_eq!(
            projection.accepted_gap_count, accepted_gap_count,
            "{}",
            replay.id
        );

        let event_store =
            FileGraphEventStore::new(workspace.path().join(".a3s/research/runs/events"));
        let events = event_store
            .load(&run_id)
            .await
            .unwrap_or_else(|error| panic!("{}: load persisted events: {error}", replay.id))
            .unwrap_or_else(|| panic!("{}: missing persisted events", replay.id));
        let journal_started_at_ms = events.first().expect("created event").timestamp_ms;
        let terminal_at_ms = events.last().expect("terminal event").timestamp_ms;
        assert!(
            journal_started_at_ms >= evaluator_started_at_ms
                && terminal_at_ms >= journal_started_at_ms,
            "{}: evaluator timing does not cover the persisted run",
            replay.id
        );
        let publication_records = publication.records();
        assert_eq!(
            publication_records.first().map(|record| record.0),
            Some(PublishedKind::SourceBacked),
            "{}: source evidence must publish before synthesis",
            replay.id
        );
        assert_eq!(
            publication_records.last().map(|record| record.0),
            Some(published_kind(expected_publication)),
            "{}",
            replay.id
        );
        persist_timing_measurement(
            workspace.path(),
            &replay.id,
            evaluator_started_at_ms,
            journal_started_at_ms,
            terminal_at_ms,
            &publication_records,
        );

        outcomes.push((replay.id, expected_publication));
    }

    assert_eq!(
        outcomes,
        [
            (
                "F01".to_string(),
                DeepResearchEvidenceFirstPublication::Synthesized,
            ),
            (
                "F02".to_string(),
                DeepResearchEvidenceFirstPublication::Synthesized,
            ),
            (
                "F03".to_string(),
                DeepResearchEvidenceFirstPublication::Qualified,
            ),
            (
                "F04".to_string(),
                DeepResearchEvidenceFirstPublication::Synthesized,
            ),
            (
                "F05".to_string(),
                DeepResearchEvidenceFirstPublication::Synthesized,
            ),
            (
                "F06".to_string(),
                DeepResearchEvidenceFirstPublication::SourceBacked,
            ),
            (
                "F07".to_string(),
                DeepResearchEvidenceFirstPublication::Synthesized,
            ),
            (
                "F08".to_string(),
                DeepResearchEvidenceFirstPublication::Synthesized,
            ),
        ]
    );
}

fn workflow_args(replay: &ProductReplay, run_id: &str) -> Value {
    let mut args = super::super::deep_research_workflow_args_with_scope(
        &replay.query,
        replay.evidence_scope(),
    );
    args["run_id"] = Value::String(run_id.to_string());
    args["input"]["current_date"] = Value::String("2026-07-21".to_string());
    args
}

fn planner_outline(replay: &ProductReplay) -> Value {
    serde_json::json!({
        "report_title": replay.query,
        "research_scope": "focused",
        "freshness_required": false,
        "workspace_evidence_required": replay
            .sources
            .iter()
            .any(|source| source.authority.is_workspace()),
        "tracks": replay.dimensions.iter().map(|dimension| {
            serde_json::json!({
                "id": dimension.id,
                "title": bounded_text(&dimension.question, 160),
                "focus": bounded_text(&dimension.question, 500),
                "material": dimension.material,
                "completion_criteria": [bounded_text(&dimension.question, 240)],
                "questions": [{
                    "question": bounded_text(&dimension.question, 500),
                    "role": "establish",
                    "completion_criterion_indexes": [0],
                }],
                "evidence_requirements": {
                    "primary_source_required": true,
                    "independent_corroboration_required": false,
                },
            })
        }).collect::<Vec<_>>(),
        "supplemental_queries": [],
    })
}

fn bootstrap_output(replay: &ProductReplay) -> WorkflowOutput {
    WorkflowOutput {
        output: serde_json::json!({
            "query": replay.query,
            "mode": "bootstrap_acquisition",
            "acquisition": {
                "packet": {
                    "version": 1,
                    "sources": replay.sources.iter().enumerate().map(|(index, source)| {
                        let source_id = format!("bootstrap-web-source-{}", index + 1);
                        serde_json::json!({
                            "source_id": source_id,
                            "title": source.title,
                            "url_or_path": source_anchor(source),
                            "chunks": [{
                                "chunk_id": format!("{source_id}:chunk:1"),
                                "text": source.content.trim(),
                            }],
                        })
                    }).collect::<Vec<_>>(),
                },
            },
            "execution": {
                "terminal_authority": "host_inquiry_reducer",
            },
        })
        .to_string(),
        metadata: None,
    }
}

fn planned_output(replay: &ProductReplay) -> WorkflowOutput {
    let failed_dimension = replay.malformed_dimension();
    let mut relevance = Vec::new();
    let mut coverage = Vec::new();
    let mut relevant_dimensions = BTreeSet::new();
    for (source_index, source) in replay.sources.iter().enumerate() {
        let source_id = format!("catalog-source-{}", source_index + 1);
        for dimension_id in source_dimensions(replay, &source.id) {
            if Some(dimension_id.as_str()) == failed_dimension {
                continue;
            }
            relevance.push(serde_json::json!({
                "source_id": &source_id,
                "obligation_id": dimension_id,
            }));
            coverage.push(serde_json::json!({
                "source_id": &source_id,
                "obligation_id": dimension_id,
                "completion_criterion_indexes": [0],
                "roles": ["supporting", "primary"],
            }));
            relevant_dimensions.insert(dimension_id);
        }
    }
    WorkflowOutput {
        output: serde_json::json!({
            "query": replay.query,
            "mode": "inquiry_collection",
            "research": {
                "status": if failed_dimension.is_some() { "partial" } else { "success" },
                "metadata": {
                    "evidence_selection_mode": "semantic_chunk_ids_with_typed_coverage",
                },
                "results": [{
                        "task_id": "frozen-product-projection",
                    "agent": "workflow",
                    "success": true,
                    "structured": {
                        "summary": "Frozen product projection.",
                        "sources": replay.sources.iter().enumerate().map(|(index, source)| {
                            let source_id = format!("catalog-source-{}", index + 1);
                            serde_json::json!({
                                "source_id": source_id,
                                "title": source.title,
                                "url_or_path": source_anchor(source),
                                "reliability": "fetched",
                                "evidence_excerpts": [{
                                    "focus": "",
                                    "quote_or_fact": source.content.trim(),
                                }],
                            })
                        }).collect::<Vec<_>>(),
                        "source_relevance": relevance,
                        "source_coverage": coverage,
                        "relevant_obligation_ids": relevant_dimensions,
                        "key_evidence": [],
                        "contradictions": [],
                        "confidence": "Closed fixture projection.",
                        "gaps": [],
                    },
                }],
                "warnings": {
                    "collection_errors": [],
                },
            },
        })
        .to_string(),
        metadata: None,
    }
}

fn report_proposal(replay: &ProductReplay) -> Value {
    let source_indexes = replay
        .sources
        .iter()
        .enumerate()
        .map(|(index, source)| (source.id.as_str(), index))
        .collect::<BTreeMap<_, _>>();
    let malformed_dimension = replay.malformed_dimension();
    let claims = replay
        .claims
        .iter()
        .filter(|claim| claim.disposition.is_active())
        .map(|claim| {
            let evidence_refs = claim
                .source_ids
                .iter()
                .map(|source_id| {
                    let index = source_indexes
                        .get(source_id.as_str())
                        .unwrap_or_else(|| panic!("{}: unknown source `{source_id}`", replay.id));
                    let alias = format!("source-{}", index + 1);
                    let chunk_id = if malformed_dimension == Some(claim.dimension_id.as_str()) {
                        format!("{alias}:chunk:missing")
                    } else {
                        format!("{alias}:chunk:1")
                    };
                    serde_json::json!({
                        "source_id": alias,
                        "chunk_ids": [chunk_id],
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "id": claim.id,
                "dimension_id": claim.dimension_id,
                "placement": claim.placement.code(),
                "kind": claim.kind.code(),
                "analysis_role": claim.analysis_role(),
                "text": claim.text,
                "evidence_refs": evidence_refs,
                "basis_claim_ids": claim.basis_claim_ids,
                "derivation": if claim.kind == FixtureClaimKind::Inference {
                    claim.derivation.as_ref().map(|method| serde_json::json!({
                        "method": method,
                        "input_claim_ids": claim.basis_claim_ids,
                    }))
                } else {
                    None
                },
            })
        })
        .collect::<Vec<_>>();
    let narrative_sections = replay
        .dimensions
        .iter()
        .map(|dimension| {
            let paragraphs = replay
                .claims
                .iter()
                .filter(|claim| {
                    claim.disposition.is_active()
                        && claim.dimension_id == dimension.id
                        && claim.placement == FixtureClaimPlacement::Finding
                })
                .map(|claim| {
                    let purpose = match claim.kind {
                        FixtureClaimKind::Fact => "evidence",
                        FixtureClaimKind::Inference => "synthesis",
                        FixtureClaimKind::Recommendation => "implication",
                    };
                    serde_json::json!({
                        "purpose": purpose,
                        "claim_ids": [claim.id],
                    })
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "dimension_id": dimension.id,
                "heading": bounded_text(&dimension.question, 96),
                "paragraphs": paragraphs,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({
        "report_language": replay.language.code(),
        "labels": replay.language.labels(),
        "claims": claims,
        "relations": replay.relations.iter().map(|relation| {
            serde_json::json!({
                "id": relation.id,
                "dimension_id": relation.dimension_id,
                "kind": relation.kind.code(),
                "claim_ids": relation.claim_ids,
            })
        }).collect::<Vec<_>>(),
        "gaps": [],
        "narrative": {
            "sections": narrative_sections,
        },
    })
}

fn editorial_plan(replay: &ProductReplay) -> Value {
    let malformed_dimension = replay.malformed_dimension();
    let mut answered_dimensions = BTreeSet::new();
    let sections = replay
        .dimensions
        .iter()
        .map(|dimension| {
            let paragraphs = replay
                .claims
                .iter()
                .filter(|claim| {
                    claim.disposition.is_active()
                        && claim.dimension_id == dimension.id
                        && Some(claim.dimension_id.as_str()) != malformed_dimension
                })
                .filter_map(|claim| {
                    let is_finding = match claim.placement {
                        FixtureClaimPlacement::Finding => true,
                        FixtureClaimPlacement::DirectAnswer => {
                            !answered_dimensions.insert(claim.dimension_id.clone())
                        }
                    };
                    if !is_finding {
                        return None;
                    }
                    let purpose = if claim.placement == FixtureClaimPlacement::DirectAnswer {
                        match claim.kind {
                            FixtureClaimKind::Fact => "evidence",
                            FixtureClaimKind::Inference => "boundary",
                            FixtureClaimKind::Recommendation => "implication",
                        }
                    } else {
                        match claim.kind {
                            FixtureClaimKind::Fact => "evidence",
                            FixtureClaimKind::Inference => "synthesis",
                            FixtureClaimKind::Recommendation => "implication",
                        }
                    };
                    Some(serde_json::json!({
                        "purpose": purpose,
                        "claim_ids": [claim.id],
                    }))
                })
                .collect::<Vec<_>>();
            serde_json::json!({
                "dimension_id": dimension.id,
                "heading": bounded_text(&dimension.question, 96),
                "paragraphs": paragraphs,
            })
        })
        .collect::<Vec<_>>();
    serde_json::json!({ "sections": sections })
}

fn source_dimensions(replay: &ProductReplay, source_id: &str) -> BTreeSet<String> {
    replay
        .claims
        .iter()
        .filter(|claim| {
            claim.disposition.is_active() && claim.source_ids.iter().any(|id| id == source_id)
        })
        .map(|claim| claim.dimension_id.clone())
        .collect()
}

fn source_anchor(source: &FixtureSource) -> &str {
    if source.authority.is_workspace() {
        &source.path
    } else {
        &source.url
    }
}

fn publication_outcome(publication: DeepResearchEvidenceFirstPublication) -> ResearchOutcome {
    match publication {
        DeepResearchEvidenceFirstPublication::Synthesized => ResearchOutcome::Completed,
        DeepResearchEvidenceFirstPublication::Qualified => ResearchOutcome::Qualified,
        DeepResearchEvidenceFirstPublication::SourceBacked
        | DeepResearchEvidenceFirstPublication::NoEvidence => ResearchOutcome::Degraded,
    }
}

fn published_kind(publication: DeepResearchEvidenceFirstPublication) -> PublishedKind {
    match publication {
        DeepResearchEvidenceFirstPublication::Synthesized => PublishedKind::Synthesized,
        DeepResearchEvidenceFirstPublication::Qualified => PublishedKind::Qualified,
        DeepResearchEvidenceFirstPublication::SourceBacked => PublishedKind::SourceBacked,
        DeepResearchEvidenceFirstPublication::NoEvidence => PublishedKind::NoEvidence,
    }
}

fn usize_metric(value: &Value, pointer: &str) -> usize {
    value
        .pointer(pointer)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
        .unwrap_or_else(|| panic!("publication omitted `{pointer}`"))
}

fn persist_timing_measurement(
    workspace: &Path,
    case_id: &str,
    evaluator_started_at_ms: u64,
    journal_started_at_ms: u64,
    terminal_at_ms: u64,
    publications: &[(PublishedKind, u64)],
) {
    let path = workspace
        .join(".a3s/research/evaluations")
        .join(format!("{case_id}.json"));
    std::fs::create_dir_all(path.parent().expect("evaluation directory"))
        .expect("create evaluation directory");
    let first_source_published_ms = publications.first().expect("source publication timing").1;
    let final_artifact_published_ms = publications.last().expect("final publication timing").1;
    let measurement = serde_json::json!({
        "schema": "a3s/deep-research-persisted-adapter-eval/v1",
        "case_id": case_id,
        "evaluator_started_at_ms": evaluator_started_at_ms,
        "journal_started_at_ms": journal_started_at_ms,
        "first_source_published_elapsed_ms": first_source_published_ms,
        "final_artifact_published_elapsed_ms": final_artifact_published_ms,
        "terminal_journal_at_ms": terminal_at_ms,
        "wall_clock_ms": terminal_at_ms.saturating_sub(evaluator_started_at_ms),
    });
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&measurement).expect("encode evaluation timing"),
    )
    .expect("persist evaluation timing");
    let recovered: Value =
        serde_json::from_slice(&std::fs::read(&path).expect("read persisted evaluation timing"))
            .expect("decode persisted evaluation timing");
    assert_eq!(
        recovered["schema"],
        "a3s/deep-research-persisted-adapter-eval/v1"
    );
    assert!(
        first_source_published_ms <= final_artifact_published_ms,
        "{case_id}: final publication preceded the source snapshot"
    );
}

fn materialize_workspace_sources(workspace: &Path, replay: &ProductReplay) {
    for source in replay
        .sources
        .iter()
        .filter(|source| source.authority.is_workspace())
    {
        let path = workspace.join(&source.path);
        std::fs::create_dir_all(path.parent().expect("workspace fixture directory"))
            .expect("create workspace fixture directory");
        std::fs::write(path, &source.content).expect("write workspace source fixture");
    }
}

fn bounded_text(value: &str, maximum: usize) -> String {
    value.chars().take(maximum).collect()
}

fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or_default()
}
