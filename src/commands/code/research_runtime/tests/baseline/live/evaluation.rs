use super::acquisition::AcquisitionResult;
use super::corpus::{LiveBudget, LiveCase};
use super::planning::PlanningResult;
use super::report::ReportResult;
use serde_json::Value as JsonValue;
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

pub(super) struct EvaluationPacketContext<'a> {
    pub(super) case: &'a LiveCase,
    pub(super) run_index: usize,
    pub(super) planning: &'a PlanningResult,
    pub(super) acquisition: &'a AcquisitionResult,
    pub(super) report: &'a ReportResult,
    pub(super) budget: &'a LiveBudget,
    pub(super) output_dir: &'a Path,
    pub(super) terminal_elapsed_ms: u64,
}

pub(super) fn write_evaluation_packet(
    context: EvaluationPacketContext<'_>,
) -> Result<PathBuf, String> {
    let EvaluationPacketContext {
        case,
        run_index,
        planning,
        acquisition,
        report,
        budget,
        output_dir,
        terminal_elapsed_ms,
    } = context;
    validate_resource_caps(planning, acquisition, report, budget)?;
    let markdown = std::fs::read(&report.markdown_path)
        .map_err(|error| format!("read report Markdown for evaluation: {error}"))?;
    let html = std::fs::read(&report.html_path)
        .map_err(|error| format!("read report HTML for evaluation: {error}"))?;
    let raw_output = report
        .raw_output_path
        .as_ref()
        .map(|path| {
            std::fs::read(path)
                .map_err(|error| format!("read raw candidate output for evaluation: {error}"))
        })
        .transpose()?;
    if markdown.is_empty() || html.is_empty() {
        return Err("terminal report artifact is empty".to_string());
    }
    let annotations = case
        .expectations
        .dimensions
        .iter()
        .map(|dimension| {
            serde_json::json!({
                "dimension_id": dimension.id,
                "question": dimension.question,
                "material": dimension.material,
                "acceptable_outcomes": dimension.acceptable,
                "outcome": JsonValue::Null,
                "rationale": JsonValue::Null,
                "supporting_claims": [],
                "missed_available_sources": [],
            })
        })
        .collect::<Vec<_>>();
    let hard_gates = [
        "critical_factual_safety",
        "citation_integrity",
        "citation_recall",
        "citation_precision",
        "source_authority",
        "no_evidence_loss",
        "partial_salvage",
        "reader_boundary",
        "language",
        "artifact_parity",
        "artifact_availability",
    ]
    .into_iter()
    .map(|gate| (gate.to_string(), JsonValue::Null))
    .collect::<serde_json::Map<_, _>>();
    let quality_scores = [
        "material_coverage",
        "evidence_quality",
        "citation_correctness",
        "synthesis_and_decision_value",
        "directness_and_information_density",
        "calibrated_uncertainty",
        "language_and_readability",
    ]
    .into_iter()
    .map(|dimension| (dimension.to_string(), JsonValue::Null))
    .collect::<serde_json::Map<_, _>>();
    let packet = serde_json::json!({
        "schema": "a3s/deep-research-live-evaluation-packet/v1",
        "evaluator_protocol": super::LIVE_EVALUATOR_PROTOCOL,
        "case_id": case.id,
        "run_index": run_index,
        "strategy": planning.strategy,
        "report_protocol": super::report::ACQUISITION_COMPARISON_REPORT_PROTOCOL,
        "query": case.query,
        "report_language": case.report_language,
        "evidence_scope": case.evidence_scope,
        "evaluation_expectations": case.expectations,
        "observed": {
            "planner_dimensions": planning.brief.as_ref().map(|brief| {
                serde_json::to_value(&brief.dimensions).unwrap_or(JsonValue::Null)
            }).or_else(|| planning.spec.as_ref().map(|spec| spec["dimensions"].clone())),
            "sources": acquisition.sources.iter().map(|source| serde_json::json!({
                "source_id": source.id,
                "title": source.title,
                "requested_anchor": source.requested_anchor,
                "canonical_anchor": source.canonical_anchor,
                "transport": source.transport,
                "provenance": source.provenance,
            })).collect::<Vec<_>>(),
            "atomic_ledger": report.admitted_ledger,
            "report": report,
            "artifact_digests": {
                "markdown_sha256": format!("{:x}", Sha256::digest(&markdown)),
                "html_sha256": format!("{:x}", Sha256::digest(&html)),
                "raw_output_sha256": raw_output.as_ref().map(|bytes| format!("{:x}", Sha256::digest(bytes))),
            },
            "artifact_shape": {
                "markdown_nonempty": !markdown.is_empty(),
                "html_nonempty": !html.is_empty(),
                "raw_output_nonempty": raw_output.as_ref().is_some_and(|bytes| !bytes.is_empty()),
                "html_doctype": String::from_utf8_lossy(&html).trim_start().starts_with("<!doctype html>"),
            }
        },
        "resource_envelope": {
            "caps": budget,
            "observed": {
                "planner_generations": 1,
                "feedback_generations": 0,
                "verifier_generations": 0,
                "report_generations": report.generation_count,
                "queries": acquisition.query_call_count,
                "source_reads": acquisition.source_call_count,
            }
        },
        "timings_ms": {
            "planner": planning.elapsed_ms,
            "discovery": acquisition.discovery_elapsed_ms,
            "source_reads": acquisition.source_elapsed_ms,
            "first_source_fetched": acquisition.first_source_fetched_ms,
            "first_source_persisted": acquisition.first_source_persisted_ms,
            "acquisition_phase": acquisition.phase_elapsed_ms,
            "report": report.elapsed_ms,
            "terminal": terminal_elapsed_ms,
        },
        "annotations": {
            "dimensions": annotations,
            "source_requirements": case.expectations.source_requirements,
            "guardrails": case.expectations.guardrails,
            "hard_gates": hard_gates,
            "quality_scores_0_to_4": quality_scores,
            "weighted_score": JsonValue::Null,
            "evaluator_ids": [],
            "adjudication": JsonValue::Null,
        },
        "generated_at": chrono::Utc::now().to_rfc3339(),
    });
    let path = output_dir.join("evaluation-packet.json");
    std::fs::write(
        &path,
        serde_json::to_vec_pretty(&packet)
            .map_err(|error| format!("encode evaluation packet: {error}"))?,
    )
    .map_err(|error| format!("write evaluation packet: {error}"))?;
    Ok(path)
}

fn validate_resource_caps(
    planning: &PlanningResult,
    acquisition: &AcquisitionResult,
    report: &ReportResult,
    budget: &LiveBudget,
) -> Result<(), String> {
    if planning.queries.len() > budget.max_queries
        || acquisition.query_call_count > budget.max_queries
    {
        return Err("live run exceeded the shared query cap".to_string());
    }
    if acquisition.selected_candidates.len() > budget.max_acquired_sources
        || acquisition.source_call_count > budget.max_acquired_sources
    {
        return Err("live run exceeded the shared source cap".to_string());
    }
    if report.generation_count > budget.report_generations {
        return Err("live run exceeded the shared report-generation cap".to_string());
    }
    if budget.feedback_generations != 0 || budget.verifier_generations != 0 {
        return Err(
            "persisted-evidence evaluation forbids feedback and verifier generations".to_string(),
        );
    }
    Ok(())
}
