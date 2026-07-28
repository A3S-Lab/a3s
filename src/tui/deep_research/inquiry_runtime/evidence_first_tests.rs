use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::Duration;

use a3s_code_core::llm::{
    structured::NativeStructuredSupport, LlmClient, LlmResponse, Message, StreamEvent, TokenUsage,
    ToolDefinition,
};
use a3s_code_core::tools::{Tool, ToolContext, ToolOutput};
use a3s_code_core::{Agent, SessionOptions};
use tokio_util::sync::CancellationToken;

use super::*;

const FETCHED_SENTENCE: &str =
    "The official Nimbus record states that version 2 receives fixes through September 2027.";

fn generated_schema_tool(tools: &[ToolDefinition]) -> anyhow::Result<&ToolDefinition> {
    anyhow::ensure!(
        tools.len() == 1,
        "fixture expected one forced structured-output tool"
    );
    Ok(&tools[0])
}

fn first_schema_enum_string<'a>(schema: &'a Value, pointer: &str) -> anyhow::Result<&'a str> {
    schema
        .pointer(pointer)
        .and_then(Value::as_array)
        .and_then(|values| values.first())
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("fixture schema omitted enum at {pointer}"))
}

fn schema_boolean_choice(schema: &Value, pointer: &str) -> bool {
    schema
        .pointer(pointer)
        .and_then(Value::as_array)
        .is_none_or(|values| values.iter().any(|value| value == &Value::Bool(true)))
}

fn evidence_selection_from_schema(schema: &Value) -> anyhow::Result<Value> {
    let chunk_id = first_schema_enum_string(schema, "/properties/chunk_ids/items/enum")?;
    let coverage = schema
        .pointer("/properties/source_coverage/items/oneOf/0/properties")
        .ok_or_else(|| anyhow::anyhow!("fixture schema omitted source coverage properties"))?;
    let relevance = schema
        .pointer("/properties/source_relevance/items/oneOf/0/properties")
        .ok_or_else(|| anyhow::anyhow!("fixture schema omitted source relevance properties"))?;
    let source_id = first_schema_enum_string(coverage, "/source_id/enum")?;
    let obligation_id = first_schema_enum_string(coverage, "/obligation_id/enum")?;
    let criterion_indexes = coverage
        .pointer("/completion_criterion_indexes/items/enum")
        .and_then(Value::as_array)
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("fixture schema omitted criterion indexes"))?;
    anyhow::ensure!(
        first_schema_enum_string(relevance, "/source_id/enum")? == source_id
            && first_schema_enum_string(relevance, "/obligation_id/enum")? == obligation_id,
        "fixture schema carried inconsistent coverage and relevance identities"
    );

    Ok(serde_json::json!({
        "chunk_ids": [chunk_id],
        "source_coverage": [{
            "source_id": source_id,
            "obligation_id": obligation_id,
            "completion_criterion_indexes": criterion_indexes,
            "roles": {
                "supporting": true,
                "primary": schema_boolean_choice(coverage, "/roles/properties/primary/enum"),
                "independent": schema_boolean_choice(
                    coverage,
                    "/roles/properties/independent/enum"
                )
            }
        }],
        "source_relevance": [{
            "source_id": source_id,
            "obligation_id": obligation_id
        }]
    }))
}

fn editorial_plan_from_schema(schema: &Value) -> anyhow::Result<Value> {
    let section_item = schema
        .pointer("/properties/sections/items")
        .ok_or_else(|| anyhow::anyhow!("fixture schema omitted editorial sections"))?;
    let variants = section_item
        .get("oneOf")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_else(|| vec![section_item.clone()]);
    let sections = variants
        .iter()
        .map(|variant| {
            let dimension_id = first_schema_enum_string(variant, "/properties/dimension_id/enum")?;
            let claim_ids = variant
                .pointer("/properties/paragraphs/items/properties/claim_ids/items/enum")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            let paragraphs = if claim_ids.is_empty() {
                Vec::new()
            } else {
                vec![serde_json::json!({
                    "purpose": "evidence",
                    "claim_ids": claim_ids,
                })]
            };
            Ok(serde_json::json!({
                "dimension_id": dimension_id,
                "heading": format!("Evidence for {}", dimension_id.replace(['.', '-'], " ")),
                "paragraphs": paragraphs,
            }))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    Ok(serde_json::json!({ "sections": sections }))
}

struct EvidenceFirstSearch {
    return_source: bool,
}

#[async_trait::async_trait]
impl Tool for EvidenceFirstSearch {
    fn name(&self) -> &str {
        "evidence_first_fixture_search"
    }

    fn description(&self) -> &str {
        "Returns the evidence-first runtime fixture search catalog."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(&self, _args: &Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        let results = if self.return_source {
            serde_json::json!([{
                "title": "Official Nimbus support record",
                "url": "https://docs.rs/nimbus/latest/nimbus/support",
                "engines": ["fixture"]
            }])
        } else {
            serde_json::json!([])
        };
        Ok(ToolOutput::success(results.to_string()))
    }
}

struct EvidenceFirstFetch;

#[async_trait::async_trait]
impl Tool for EvidenceFirstFetch {
    fn name(&self) -> &str {
        "evidence_first_fixture_fetch"
    }

    fn description(&self) -> &str {
        "Returns one deterministic fetched source."
    }

    fn parameters(&self) -> Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(&self, args: &Value, _ctx: &ToolContext) -> anyhow::Result<ToolOutput> {
        anyhow::ensure!(
            args.get("url").and_then(Value::as_str)
                == Some("https://docs.rs/nimbus/latest/nimbus/support"),
            "unexpected evidence-first fixture URL"
        );
        Ok(
            ToolOutput::success(FETCHED_SENTENCE).with_metadata(serde_json::json!({
                "source_anchors": ["https://docs.rs/nimbus/latest/nimbus/support"],
                "document_kind": "html",
                "content_type": "text/html",
                "range": {
                    "offset": 0,
                    "returned_chars": FETCHED_SENTENCE.chars().count(),
                    "next_offset": null,
                    "eof": true
                }
            })),
        )
    }
}

#[derive(Clone, Copy)]
enum ProposalBehavior {
    Slow,
    Invalid,
    FailOnceThenValid,
    Qualified,
}

struct EvidenceFirstProposal {
    behavior: ProposalBehavior,
    report_path: PathBuf,
    calls: Arc<AtomicUsize>,
    saw_staged_report: Arc<AtomicBool>,
}

impl EvidenceFirstProposal {
    async fn proposal(&self, tools: &[ToolDefinition]) -> anyhow::Result<Value> {
        let tool = generated_schema_tool(tools)?;
        match tool.name.as_str() {
            "emit_deep_research_semantic_outline" => {
                let mut tracks = vec![serde_json::json!({
                    "id": "support.boundary",
                    "title": "Support boundary",
                    "focus": "Establish the supported Nimbus release and maintenance boundary.",
                    "material": true,
                    "completion_criteria": [
                        "A traceable source identifies the release and support boundary."
                    ],
                    "questions": [{
                        "question": "Which release and maintenance date does the record establish?",
                        "role": "establish",
                        "completion_criterion_indexes": [0]
                    }],
                    "evidence_requirements": {
                        "primary_source_required": true,
                        "independent_corroboration_required": false
                    }
                })];
                if matches!(self.behavior, ProposalBehavior::Qualified) {
                    tracks.push(serde_json::json!({
                        "id": "support.conditions",
                        "title": "Support conditions",
                        "focus": "Establish any conditions attached to the stated maintenance period.",
                        "material": true,
                        "completion_criteria": [
                            "A traceable source identifies any support conditions."
                        ],
                        "questions": [{
                            "question": "Which conditions limit the stated support period?",
                            "role": "establish",
                            "completion_criterion_indexes": [0]
                        }],
                        "evidence_requirements": {
                            "primary_source_required": true,
                            "independent_corroboration_required": false
                        }
                    }));
                }
                Ok(serde_json::json!({
                    "report_title": "Nimbus support research",
                    "research_scope": "focused",
                    "freshness_required": false,
                    "workspace_evidence_required": false,
                    "tracks": tracks,
                    "supplemental_queries": []
                }))
            }
            "emit_deep_research_web_source_selection" => Ok(serde_json::json!({
                "candidate_ids": [
                    first_schema_enum_string(
                        &tool.parameters,
                        "/properties/candidate_ids/items/enum"
                    )?
                ]
            })),
            "emit_deep_research_evidence_selection"
            | "emit_deep_research_evidence_shard_selection"
            | "emit_deep_research_evidence_source_reduction" => {
                evidence_selection_from_schema(&tool.parameters)
            }
            "emit_deep_research_typed_claim_graph" => {
                self.calls.fetch_add(1, Ordering::SeqCst);
                let staged = std::fs::read_to_string(&self.report_path)?;
                anyhow::ensure!(
                    staged.contains(FETCHED_SENTENCE),
                    "the model proposal started before the source-backed report was staged"
                );
                self.saw_staged_report.store(true, Ordering::SeqCst);

                match self.behavior {
                    ProposalBehavior::Slow => std::future::pending::<anyhow::Result<Value>>().await,
                    ProposalBehavior::Invalid => Ok(serde_json::json!({
                        "report_language": "en",
                        "labels": {
                            "answer": "Direct Answer",
                            "findings": "Findings",
                            "recommendations": "Evidence-Based Recommendations",
                            "limitations": "Limitations",
                            "evidence_boundary": "This report publishes no conclusion beyond the fetched evidence.",
                            "sources": "Sources",
                            "contradiction": "Contradiction",
                            "inference": "Inference",
                            "basis": "Basis",
                            "derivation": "Derivation"
                        },
                        "claims": [{
                            "id": "fabricated-answer",
                            "dimension_id": "support.boundary",
                            "placement": "direct_answer",
                            "kind": "fact",
                            "analysis_role": "conclusion",
                            "text": "A fabricated source claims support through 2099.",
                            "evidence_refs": [{
                                "source_id": "source-99",
                                "chunk_ids": ["source-99:chunk:1"]
                            }],
                            "basis_claim_ids": [],
                            "derivation": null
                        }],
                        "relations": [],
                        "gaps": [],
                        "narrative": {
                            "sections": [{
                                "dimension_id": "support.boundary",
                                "heading": "Support boundary",
                                "paragraphs": []
                            }]
                        }
                    })),
                    ProposalBehavior::FailOnceThenValid
                        if self.calls.load(Ordering::SeqCst) == 1 =>
                    {
                        anyhow::bail!("simulated transient streaming failure")
                    }
                    ProposalBehavior::FailOnceThenValid => Ok(serde_json::json!({
                        "report_language": "en",
                        "labels": {
                            "answer": "Direct Answer",
                            "findings": "Findings",
                            "recommendations": "Evidence-Based Recommendations",
                            "limitations": "Limitations",
                            "evidence_boundary": "This report publishes no conclusion beyond the fetched evidence.",
                            "sources": "Sources",
                            "contradiction": "Contradiction",
                            "inference": "Inference",
                            "basis": "Basis",
                            "derivation": "Derivation"
                        },
                        "claims": [{
                            "id": "nimbus-answer",
                            "dimension_id": "support.boundary",
                            "placement": "direct_answer",
                            "kind": "fact",
                            "analysis_role": "conclusion",
                            "text": "Nimbus version 2 receives fixes through September 2027.",
                            "evidence_refs": [{
                                "source_id": "source-1",
                                "chunk_ids": ["source-1:chunk:1"]
                            }],
                            "basis_claim_ids": [],
                            "derivation": null
                        }, {
                            "id": "nimbus-boundary",
                            "dimension_id": "support.boundary",
                            "placement": "finding",
                            "kind": "fact",
                            "analysis_role": "evidence",
                            "text": "The official Nimbus record identifies version 2 and September 2027 as the support boundary.",
                            "evidence_refs": [{
                                "source_id": "source-1",
                                "chunk_ids": ["source-1:chunk:1"]
                            }],
                            "basis_claim_ids": [],
                            "derivation": null
                        }],
                        "relations": [],
                        "gaps": [],
                        "narrative": {
                            "sections": [{
                                "dimension_id": "support.boundary",
                                "heading": "Support boundary",
                                "paragraphs": [{
                                    "purpose": "evidence",
                                    "claim_ids": ["nimbus-boundary"]
                                }]
                            }]
                        }
                    })),
                    ProposalBehavior::Qualified => Ok(serde_json::json!({
                        "report_language": "en",
                        "labels": {
                            "answer": "Direct Answer",
                            "findings": "Findings",
                            "recommendations": "Evidence-Based Recommendations",
                            "limitations": "Limitations",
                            "evidence_boundary": "This report publishes no conclusion beyond the fetched evidence.",
                            "sources": "Sources",
                            "contradiction": "Contradiction",
                            "inference": "Inference",
                            "basis": "Basis",
                            "derivation": "Derivation"
                        },
                        "claims": [{
                            "id": "nimbus-qualified-answer",
                            "dimension_id": "support.boundary",
                            "placement": "direct_answer",
                            "kind": "fact",
                            "analysis_role": "conclusion",
                            "text": "Nimbus version 2 receives fixes through September 2027.",
                            "evidence_refs": [{
                                "source_id": "source-1",
                                "chunk_ids": ["source-1:chunk:1"]
                            }],
                            "basis_claim_ids": [],
                            "derivation": null
                        }],
                        "relations": [],
                        "gaps": [{
                            "id": "nimbus-unresolved-boundary",
                            "dimension_id": "support.conditions",
                            "text": "The reviewed record does not establish support conditions beyond the stated maintenance date."
                        }],
                        "narrative": {
                            "sections": [{
                                "dimension_id": "support.boundary",
                                "heading": "Support boundary",
                                "paragraphs": []
                            }, {
                                "dimension_id": "support.conditions",
                                "heading": "Support conditions",
                                "paragraphs": []
                            }]
                        }
                    })),
                }
            }
            "emit_deep_research_typed_editorial_plan" => {
                editorial_plan_from_schema(&tool.parameters)
            }
            unexpected => {
                anyhow::bail!("unexpected evidence-first structured schema tool `{unexpected}`")
            }
        }
    }

    fn response(value: Value) -> LlmResponse {
        LlmResponse {
            message: Message::assistant(&value.to_string()),
            usage: TokenUsage::default(),
            stop_reason: Some("stop".to_string()),
            token_logprobs: Vec::new(),
            meta: None,
        }
    }
}

#[async_trait::async_trait]
impl LlmClient for EvidenceFirstProposal {
    fn native_structured_support(&self) -> NativeStructuredSupport {
        NativeStructuredSupport::ForcedTool
    }

    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        self.proposal(tools).await.map(Self::response)
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        tools: &[ToolDefinition],
        _cancel_token: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let response = Self::response(self.proposal(tools).await?);
        let text = response.message.text();
        let (tx, rx) = mpsc::channel(4);
        tokio::spawn(async move {
            tx.send(StreamEvent::TextDelta(text)).await.ok();
            tx.send(StreamEvent::Done(response)).await.ok();
        });
        Ok(rx)
    }
}

struct UnexpectedProposal {
    calls: Arc<AtomicUsize>,
}

#[async_trait::async_trait]
impl LlmClient for UnexpectedProposal {
    fn native_structured_support(&self) -> NativeStructuredSupport {
        NativeStructuredSupport::ForcedTool
    }

    async fn complete(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        tools: &[ToolDefinition],
    ) -> anyhow::Result<LlmResponse> {
        let tool = generated_schema_tool(tools)?;
        if tool.name == "emit_deep_research_semantic_outline" {
            return Ok(EvidenceFirstProposal::response(serde_json::json!({
                "report_title": "Nimbus evidence check",
                "research_scope": "focused",
                "freshness_required": false,
                "workspace_evidence_required": false,
                "tracks": [{
                    "id": "request.primary",
                    "title": "Requested evidence",
                    "focus": "Establish the requested answer.",
                    "material": true,
                    "completion_criteria": ["The answer is supported or explicitly bounded."],
                    "questions": [{
                        "question": "What evidence establishes or bounds the requested answer?",
                        "role": "establish",
                        "completion_criterion_indexes": [0]
                    }],
                    "evidence_requirements": {
                        "primary_source_required": false,
                        "independent_corroboration_required": false
                    }
                }],
                "supplemental_queries": []
            })));
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("no-evidence publication must not invoke report generation")
    }

    async fn complete_streaming(
        &self,
        _messages: &[Message],
        _system: Option<&str>,
        tools: &[ToolDefinition],
        _cancel_token: CancellationToken,
    ) -> anyhow::Result<mpsc::Receiver<StreamEvent>> {
        let tool = generated_schema_tool(tools)?;
        if tool.name == "emit_deep_research_semantic_outline" {
            let response = EvidenceFirstProposal::response(serde_json::json!({
                "report_title": "Nimbus evidence check",
                "research_scope": "focused",
                "freshness_required": false,
                "workspace_evidence_required": false,
                "tracks": [{
                    "id": "request.primary",
                    "title": "Requested evidence",
                    "focus": "Establish the requested answer.",
                    "material": true,
                    "completion_criteria": ["The answer is supported or explicitly bounded."],
                    "questions": [{
                        "question": "What evidence establishes or bounds the requested answer?",
                        "role": "establish",
                        "completion_criterion_indexes": [0]
                    }],
                    "evidence_requirements": {
                        "primary_source_required": false,
                        "independent_corroboration_required": false
                    }
                }],
                "supplemental_queries": []
            }));
            let text = response.message.text();
            let (tx, rx) = mpsc::channel(4);
            tokio::spawn(async move {
                tx.send(StreamEvent::TextDelta(text)).await.ok();
                tx.send(StreamEvent::Done(response)).await.ok();
            });
            return Ok(rx);
        }
        self.calls.fetch_add(1, Ordering::SeqCst);
        anyhow::bail!("no-evidence publication must not invoke report generation")
    }
}

#[tokio::test]
async fn proposal_timeout_preserves_the_already_staged_source_report() {
    let workspace = tempfile::tempdir().expect("create timeout workspace");
    let query = "Which Nimbus release is supported?";
    let calls = Arc::new(AtomicUsize::new(0));
    let saw_staged_report = Arc::new(AtomicBool::new(false));
    let report_path = report_markdown_path(workspace.path(), query);
    let (_agent, session) = fixture_session(
        workspace.path(),
        true,
        Arc::new(EvidenceFirstProposal {
            behavior: ProposalBehavior::Slow,
            report_path,
            calls: Arc::clone(&calls),
            saw_staged_report: Arc::clone(&saw_staged_report),
        }),
    )
    .await;
    let args = evidence_first_args(query, "evidence-first-proposal-timeout");
    record_workflow_started(
        workspace.path(),
        "evidence-first-proposal-timeout",
        deep_research_evidence_first_research_spec(&args),
    )
    .await
    .expect("pre-create the TUI-owned journal with the shared spec");

    let result = tokio::time::timeout(
        Duration::from_secs(6),
        execute_fixture_runtime(session, args, 1_200),
    )
    .await
    .expect("two bounded proposal attempts must cancel an indefinitely pending model call")
    .expect("timeout must fall back instead of failing the run");
    assert!(
        (1..=2).contains(&calls.load(Ordering::SeqCst)),
        "{}",
        result.output
    );
    assert!(saw_staged_report.load(Ordering::SeqCst));
    assert_source_backed_result(workspace.path(), query, &result);
}

#[tokio::test]
async fn transient_report_stream_failure_retries_and_publishes_a_real_report() {
    let workspace = tempfile::tempdir().expect("create retry workspace");
    let query = "Which Nimbus release is supported?";
    let run_id = "evidence-first-proposal-retry";
    let calls = Arc::new(AtomicUsize::new(0));
    let saw_staged_report = Arc::new(AtomicBool::new(false));
    let report_path = report_markdown_path(workspace.path(), query);
    let (_agent, session) = fixture_session(
        workspace.path(),
        true,
        Arc::new(EvidenceFirstProposal {
            behavior: ProposalBehavior::FailOnceThenValid,
            report_path,
            calls: Arc::clone(&calls),
            saw_staged_report: Arc::clone(&saw_staged_report),
        }),
    )
    .await;
    let args = evidence_first_args(query, run_id);

    let result = execute_fixture_runtime(session, args, 5_000)
        .await
        .expect("transient report generation failure must be retried");

    assert_eq!(calls.load(Ordering::SeqCst), 2, "{}", result.output);
    assert!(saw_staged_report.load(Ordering::SeqCst));
    let output: Value = serde_json::from_str(&result.output).expect("decode retry output");
    assert_eq!(output["publication"]["status"], "synthesized");
    assert_eq!(output["research"]["status"], "success");
    assert_eq!(output["publication"]["quality"]["direct_answer_count"], 1);
    assert_eq!(output["publication"]["quality"]["finding_count"], 1);
    assert_eq!(output["publication"]["quality"]["accepted_claim_count"], 2);
    let markdown = std::fs::read_to_string(report_markdown_path(workspace.path(), query))
        .expect("read synthesized retry report");
    assert!(markdown.contains("## Direct Answer"), "{markdown}");
    assert!(markdown.contains("## Findings"), "{markdown}");
    assert!(
        !markdown.contains("Preserved Source Evidence"),
        "{markdown}"
    );
    let recovered =
        super::super::deep_research_artifacts::recover_deep_research_publication_receipt(
            workspace.path(),
            query,
            run_id,
        )
        .expect("read the run-scoped publication receipt")
        .expect("recover the completed publication before terminal settlement");
    assert_eq!(
        recovered.publication,
        super::super::deep_research_artifacts::DeepResearchEvidenceFirstPublication::Synthesized
    );
    assert_eq!(recovered.quality.accepted_claim_count, 2);
    assert_eq!(recovered.quality.cited_source_count, 1);
}

#[tokio::test]
async fn qualified_claim_graph_survives_publication_and_receipt_recovery() {
    let workspace = tempfile::tempdir().expect("create qualified workspace");
    let query = "Which Nimbus release is supported?";
    let run_id = "evidence-first-qualified-report";
    let calls = Arc::new(AtomicUsize::new(0));
    let saw_staged_report = Arc::new(AtomicBool::new(false));
    let report_path = report_markdown_path(workspace.path(), query);
    let (_agent, session) = fixture_session(
        workspace.path(),
        true,
        Arc::new(EvidenceFirstProposal {
            behavior: ProposalBehavior::Qualified,
            report_path,
            calls: Arc::clone(&calls),
            saw_staged_report: Arc::clone(&saw_staged_report),
        }),
    )
    .await;
    let result = execute_fixture_runtime(session, evidence_first_args(query, run_id), 5_000)
        .await
        .expect("qualified report execution");

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(saw_staged_report.load(Ordering::SeqCst));
    let output: Value = serde_json::from_str(&result.output).expect("decode qualified output");
    assert_eq!(output["publication"]["status"], "qualified");
    assert_eq!(output["research"]["status"], "partial_success");
    assert_eq!(output["publication"]["quality"]["accepted_claim_count"], 1);
    assert_eq!(output["publication"]["quality"]["accepted_gap_count"], 1);
    let recovered =
        super::super::deep_research_artifacts::recover_deep_research_publication_receipt(
            workspace.path(),
            query,
            run_id,
        )
        .expect("read qualified publication receipt")
        .expect("recover qualified publication");
    assert_eq!(
        recovered.publication,
        super::super::deep_research_artifacts::DeepResearchEvidenceFirstPublication::Qualified
    );
    assert_eq!(recovered.quality.accepted_gap_count, 1);
}

#[tokio::test]
async fn invalid_proposal_preserves_valid_fetched_evidence() {
    let workspace = tempfile::tempdir().expect("create invalid-proposal workspace");
    let query = "Which Nimbus release is supported?";
    let calls = Arc::new(AtomicUsize::new(0));
    let saw_staged_report = Arc::new(AtomicBool::new(false));
    let report_path = report_markdown_path(workspace.path(), query);
    let (_agent, session) = fixture_session(
        workspace.path(),
        true,
        Arc::new(EvidenceFirstProposal {
            behavior: ProposalBehavior::Invalid,
            report_path,
            calls: Arc::clone(&calls),
            saw_staged_report: Arc::clone(&saw_staged_report),
        }),
    )
    .await;
    let args = evidence_first_args(query, "evidence-first-invalid-proposal");

    let result = execute_fixture_runtime(session, args, 2_000)
        .await
        .expect("invalid proposal must fall back instead of failing the run");
    assert_eq!(
        calls.load(Ordering::SeqCst),
        2,
        "the per-run schema rejects the unknown alias and the durable port uses only its one bounded retry: {}",
        result.output
    );
    assert!(saw_staged_report.load(Ordering::SeqCst));
    assert_source_backed_result(workspace.path(), query, &result);
    let markdown = std::fs::read_to_string(report_markdown_path(workspace.path(), query))
        .expect("read retained source-backed Markdown");
    assert!(!markdown.contains("2099"));
    assert!(!markdown.contains("source-99"));
}

#[tokio::test]
async fn empty_acquisition_publishes_honest_artifacts_without_a_model_call() {
    let workspace = tempfile::tempdir().expect("create no-evidence runtime workspace");
    let query = "核查 Nimbus 当前支持策略";
    let calls = Arc::new(AtomicUsize::new(0));
    let (_agent, session) = fixture_session(
        workspace.path(),
        false,
        Arc::new(UnexpectedProposal {
            calls: Arc::clone(&calls),
        }),
    )
    .await;
    let args = evidence_first_args(query, "evidence-first-no-evidence");

    let result = execute_fixture_runtime(session, args, 1_000)
        .await
        .expect("empty acquisition must publish an honest terminal artifact");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(
        result.metadata.is_none(),
        "the Host result must not expose child workflow metadata"
    );
    let output: Value = serde_json::from_str(&result.output).expect("decode runtime output");
    assert_eq!(output["publication"]["status"], "no_evidence");
    assert_eq!(output["research"]["status"], "failed");
    let published =
        super::super::deep_research_artifacts::deep_research_evidence_first_published_report(
            workspace.path(),
            query,
            &result.output,
        )
        .expect("validate no-evidence publication")
        .expect("rediscover no-evidence artifacts");
    assert_eq!(
        published.publication,
        super::super::deep_research_artifacts::DeepResearchEvidenceFirstPublication::NoEvidence
    );
}

async fn fixture_session(
    workspace: &Path,
    return_source: bool,
    proposal: Arc<dyn LlmClient>,
) -> (Agent, AgentSession) {
    let config = workspace.join("config.acl");
    std::fs::write(
        &config,
        "default_model = \"openai/x\"\n\
         providers \"openai\" {\n  apiKey = \"x\"\n  baseUrl = \"http://127.0.0.1:1\"\n  \
         models \"x\" { name = \"x\" }\n}\n",
    )
    .expect("write evidence-first fixture config");
    let agent = Agent::new(config.to_string_lossy().to_string())
        .await
        .expect("create evidence-first fixture agent");
    let options = SessionOptions::new()
        .with_session_id(format!(
            "evidence-first-fixture-{}-{}",
            std::process::id(),
            rand::random::<u64>()
        ))
        .with_llm_client(proposal)
        .with_auto_save(false)
        .with_tool_timeout(5_000);
    let session = agent
        .session_async(workspace.to_string_lossy().to_string(), Some(options))
        .await
        .expect("create evidence-first fixture session");
    session
        .register_dynamic_workflow_runtime()
        .expect("register dynamic workflow runtime");
    session
        .register_dynamic_tool(Arc::new(EvidenceFirstSearch { return_source }))
        .expect("register fixture search");
    session
        .register_dynamic_tool(Arc::new(EvidenceFirstFetch))
        .expect("register fixture fetch");
    (agent, session)
}

pub(super) async fn product_adapter_fixture_session(workspace: &Path) -> (Agent, AgentSession) {
    fixture_session(
        workspace,
        false,
        Arc::new(EvidenceFirstProposal {
            behavior: ProposalBehavior::Invalid,
            report_path: report_markdown_path(workspace, "unused product adapter fixture"),
            calls: Arc::new(AtomicUsize::new(0)),
            saw_staged_report: Arc::new(AtomicBool::new(false)),
        }),
    )
    .await
}

fn evidence_first_args(query: &str, run_id: &str) -> Value {
    let mut args = super::super::deep_research_workflow_args_with_scope(
        query,
        super::super::DeepResearchEvidenceScope::WebAndWorkspace,
    );
    let source = args["source"]
        .as_str()
        .expect("DeepResearch workflow source")
        .replace(
            "ctx.tool(\"web_search\"",
            "ctx.tool(\"evidence_first_fixture_search\"",
        )
        .replace(
            "tool: \"web_search\"",
            "tool: \"evidence_first_fixture_search\"",
        )
        .replace(
            "ctx.tool(\"web_fetch\"",
            "ctx.tool(\"evidence_first_fixture_fetch\"",
        )
        .replace(
            "tool: \"web_fetch\"",
            "tool: \"evidence_first_fixture_fetch\"",
        );
    args["source"] = Value::String(source);
    args["run_id"] = Value::String(run_id.to_string());
    args
}

async fn execute_fixture_runtime(
    session: AgentSession,
    args: Value,
    proposal_stage_timeout_ms: u64,
) -> Result<ToolCallResult, String> {
    let (progress_tx, mut progress_rx) = mpsc::channel(PROGRESS_CHANNEL_CAPACITY);
    let progress_drain = tokio::spawn(async move { while progress_rx.recv().await.is_some() {} });
    let result = run_evidence_first_research_with_limits(
        Arc::new(session),
        args,
        progress_tx,
        EvidenceFirstRuntimeLimits {
            bootstrap_stage_timeout_ms: 5_000,
            planned_retrieval_stage_timeout_ms: 5_000,
            report_proposal_attempt_timeout_ms: proposal_stage_timeout_ms
                .saturating_sub(200)
                .max(1_000),
            report_proposal_stage_timeout_ms: proposal_stage_timeout_ms,
        },
    )
    .await;
    progress_drain.await.expect("drain progress events");
    result
}

fn assert_source_backed_result(workspace: &Path, query: &str, result: &ToolCallResult) {
    assert!(
        result.metadata.is_none(),
        "the Host result must not expose child workflow metadata"
    );
    let output: Value = serde_json::from_str(&result.output).expect("decode runtime output");
    assert_eq!(output["publication"]["status"], "source_backed");
    assert_eq!(output["research"]["status"], "degraded");
    assert!(output["research"]["warnings"]["report_error"].is_string());
    let published =
        super::super::deep_research_artifacts::deep_research_evidence_first_published_report(
            workspace,
            query,
            &result.output,
        )
        .expect("validate source-backed publication")
        .expect("rediscover source-backed artifacts");
    assert_eq!(
        published.publication,
        super::super::deep_research_artifacts::DeepResearchEvidenceFirstPublication::SourceBacked
    );
    let markdown =
        std::fs::read_to_string(published.artifacts.markdown).expect("read source-backed Markdown");
    assert!(markdown.contains(FETCHED_SENTENCE));
}

fn report_markdown_path(workspace: &Path, query: &str) -> PathBuf {
    workspace
        .join(".a3s/research")
        .join(super::super::deep_research_artifacts::deep_research_report_slug(query))
        .join("report.md")
}
