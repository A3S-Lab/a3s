use std::collections::{BTreeMap, BTreeSet};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use a3s_code_core::tools::{Tool, ToolContext, ToolErrorKind, ToolExecutor, ToolOutput};
use tokio::sync::Notify;

struct SearchFixture {
    queries: Arc<Mutex<Vec<String>>>,
    results: serde_json::Value,
}

struct QuerySearchFixture {
    queries: Arc<Mutex<Vec<String>>>,
    results_by_query: BTreeMap<String, serde_json::Value>,
}

struct FallbackNoticeSearchFixture {
    queries: Arc<Mutex<Vec<String>>>,
    results: serde_json::Value,
}

struct TextFetchFixture {
    urls: Arc<Mutex<Vec<String>>>,
    bodies: BTreeMap<String, String>,
}

struct SemanticSelectorFixture {
    preferred_fragments: Vec<String>,
    fail: bool,
    invalid_selection: bool,
}

struct RetryOnceSemanticSelectorFixture {
    calls: Arc<AtomicUsize>,
    selector: SemanticSelectorFixture,
    retry_web_source_selection: bool,
}

struct FailMatchingSemanticSelectorFixture {
    schema_name: &'static str,
    fragment: String,
    selector: SemanticSelectorFixture,
}

struct FailWebSourceSelectionFixture {
    selector: SemanticSelectorFixture,
}

struct PaginatedPdfFixture {
    offsets: Arc<Mutex<Vec<u64>>>,
}

struct PaginatedHtmlFixture {
    offsets: Arc<Mutex<Vec<u64>>>,
}

struct TransientFetchFixture {
    calls: Arc<Mutex<Vec<String>>>,
}

struct UntypedFetchFailureFixture {
    calls: Arc<AtomicUsize>,
}

struct InterruptedSiblingFetchFixture {
    calls: Arc<Mutex<Vec<String>>>,
    blocked_url: String,
    blocked_started: Arc<Notify>,
}

struct LocalEvidenceFixture;

struct NoLocalEvidenceFixture;

struct AbsoluteLocalCandidateFixture {
    path: String,
}

struct OversizedLocalRangeFixture;

const PDF_RANGE_ONE: &str =
    "第一段记录了双阶段检索方法以及证据保留边界，内容足够构成一个结构化文本块。";
const PDF_RANGE_TWO: &str = "第二段报告消融实验使引用完整率下降十七个百分点，并给出明确测量条件。";
const PDF_RANGE_THREE: &str =
    "第三段说明评测只覆盖英语技术主题，因此其他语言和领域仍然属于证据缺口。";
const HTML_RANGE_ONE: &str = "项目记录前段说明了阶段一背景和参与方，并提示后续还有完整进展记录。";
const HTML_RANGE_TWO: &str = "项目记录后段说明了阶段二进展，并确认阶段三结论、最终指标和发布日期。";
const HTML_RANGE_THREE: &str = "项目记录末段确认阶段三结论、最终指标和发布日期，构成完整事实记录。";

#[async_trait::async_trait]
impl Tool for SearchFixture {
    fn name(&self) -> &str {
        "fixture_web_search"
    }

    fn description(&self) -> &str {
        "Returns deterministic search candidates."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        assert!(
            args.get("engines").is_none(),
            "DeepResearch must inherit the default search engines from config.acl"
        );
        self.queries.lock().unwrap().push(
            args.get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
        Ok(ToolOutput::success(self.results.to_string()))
    }
}

#[async_trait::async_trait]
impl Tool for QuerySearchFixture {
    fn name(&self) -> &str {
        "fixture_query_web_search"
    }

    fn description(&self) -> &str {
        "Returns deterministic candidates for each exact provider query."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        assert!(
            args.get("engines").is_none(),
            "DeepResearch must inherit the default search engines from config.acl"
        );
        let query = args
            .get("query")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.queries.lock().unwrap().push(query.clone());
        let results = self
            .results_by_query
            .get(&query)
            .ok_or_else(|| anyhow::anyhow!("unexpected fixture query: {query}"))?;
        Ok(ToolOutput::success(results.to_string()))
    }
}

#[async_trait::async_trait]
impl Tool for FallbackNoticeSearchFixture {
    fn name(&self) -> &str {
        "fixture_fallback_web_search"
    }

    fn description(&self) -> &str {
        "Returns deterministic candidates with generic fallback metadata."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        assert!(
            args.get("engines").is_none(),
            "DeepResearch must inherit the default search engines from config.acl"
        );
        self.queries.lock().unwrap().push(
            args.get("query")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
        Ok(ToolOutput::success(self.results.to_string()).with_metadata(
            serde_json::json!({
                "status": "partial",
                "engine_selection_source": "config",
                "selected_engines": ["anysearch"],
                "notices": [
                    "Search degraded because AnySearch quota is exhausted; automatically fell back to Brave and Bing."
                ],
                "search_fallback": {
                    "trigger": "engine_failure",
                    "mode": "additional_engines",
                    "attempted": true,
                    "engines": ["brave", "bing"],
                    "successful": true,
                    "failures": [{
                        "engine": "AnySearch",
                        "provider": "anysearch",
                        "kind": "provider_quota",
                        "transient": false
                    }]
                }
            }),
        ))
    }
}

#[async_trait::async_trait]
impl Tool for TextFetchFixture {
    fn name(&self) -> &str {
        "fixture_web_fetch"
    }

    fn description(&self) -> &str {
        "Returns deterministic fetched source text."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let url = args
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        self.urls.lock().unwrap().push(url.clone());
        let body = self
            .bodies
            .get(&url)
            .ok_or_else(|| anyhow::anyhow!("unexpected fixture URL: {url}"))?;
        let offset = args
            .get("offset")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(0);
        let maximum = args
            .get("max_chars")
            .and_then(serde_json::Value::as_u64)
            .and_then(|value| usize::try_from(value).ok())
            .unwrap_or(50_000);
        let total_chars = body.chars().count();
        if offset > total_chars {
            return Ok(ToolOutput::error("fixture offset exceeds body"));
        }
        let content = body.chars().skip(offset).take(maximum).collect::<String>();
        let returned_chars = content.chars().count();
        let next_offset =
            (offset + returned_chars < total_chars).then_some(offset + returned_chars);
        let mut output = content;
        if let Some(next_offset) = next_offset {
            output.push_str(&format!(
                "\n\n... (fixture continuation; offset={next_offset})\n"
            ));
        }
        Ok(
            ToolOutput::success(output).with_metadata(serde_json::json!({
                "source_anchors": [url],
                "document_kind": "html",
                "content_type": "text/html",
                "range": {
                    "offset": offset,
                    "requested_max_chars": maximum,
                    "applied_max_chars": maximum,
                    "returned_chars": returned_chars,
                    "total_chars": total_chars,
                    "next_offset": next_offset,
                    "eof": next_offset.is_none(),
                    "limit_clamped": false
                }
            })),
        )
    }
}

#[async_trait::async_trait]
impl Tool for InterruptedSiblingFetchFixture {
    fn name(&self) -> &str {
        "fixture_interrupted_sibling_web_fetch"
    }

    fn description(&self) -> &str {
        "Completes one source while holding the first attempt for another source."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let url = args
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let attempt = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(url.clone());
            calls.iter().filter(|observed| *observed == &url).count()
        };
        if url == self.blocked_url && attempt == 1 {
            self.blocked_started.notify_one();
            std::future::pending::<()>().await;
        }
        let output = format!(
            "Durable source material for {url} remains independently recoverable after interruption."
        );
        let returned_chars = output.chars().count();
        Ok(
            ToolOutput::success(output).with_metadata(serde_json::json!({
                "source_anchors": [url],
                "document_kind": "html",
                "content_type": "text/html",
                "range": {
                    "offset": 0,
                    "requested_max_chars": 50_000,
                    "applied_max_chars": 50_000,
                    "returned_chars": returned_chars,
                    "total_chars": returned_chars,
                    "next_offset": null,
                    "eof": true,
                    "limit_clamped": false
                }
            })),
        )
    }
}

#[async_trait::async_trait]
impl Tool for SemanticSelectorFixture {
    fn name(&self) -> &str {
        "generate_object"
    }

    fn description(&self) -> &str {
        "Selects semantic chunk IDs from the closed evidence packet."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let schema_name = args.get("schema_name").and_then(serde_json::Value::as_str);
        if matches!(
            schema_name,
            Some(
                "deep_research_web_source_selection"
                    | "deep_research_supplemental_web_source_selection"
            )
        ) {
            let prompt = args
                .get("prompt")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let marker = if schema_name == Some("deep_research_supplemental_web_source_selection") {
                "CLOSED_SUPPLEMENTAL_DISCOVERY_PACKET="
            } else {
                "CLOSED_WEB_DISCOVERY_PACKET="
            };
            let packet = prompt
                .split_once(marker)
                .map(|(_, packet)| packet)
                .ok_or_else(|| {
                    anyhow::anyhow!("web source selector omitted its closed discovery packet")
                })?;
            let packet: serde_json::Value = serde_json::from_str(packet)?;
            let candidates = packet["candidates"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("web source selector packet omitted candidates"))?;
            let maximum = args["schema"]["properties"]["candidate_ids"]["maxItems"]
                .as_u64()
                .unwrap_or(candidates.len() as u64) as usize;
            let mut candidate_ids = Vec::new();
            for preferred in &self.preferred_fragments {
                if let Some(candidate) = candidates.iter().find(|candidate| {
                    ["title", "url", "content"].iter().any(|field| {
                        candidate[*field]
                            .as_str()
                            .is_some_and(|text| text.contains(preferred))
                    })
                }) {
                    let candidate_id = candidate["candidate_id"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("candidate omitted ID"))?;
                    if !candidate_ids.iter().any(|seen| seen == candidate_id) {
                        candidate_ids.push(candidate_id.to_string());
                    }
                }
            }
            if candidate_ids.is_empty() {
                candidate_ids.extend(
                    candidates
                        .iter()
                        .take(maximum)
                        .filter_map(|candidate| candidate["candidate_id"].as_str())
                        .map(str::to_string),
                );
            }
            candidate_ids.truncate(maximum);
            return Ok(ToolOutput::success(
                serde_json::json!({
                    "object": { "candidate_ids": candidate_ids },
                    "repair_rounds": 0,
                    "mode_used": "fixture"
                })
                .to_string(),
            ));
        }
        if self.fail {
            return Ok(ToolOutput::error("simulated semantic selector failure"));
        }
        if self.invalid_selection {
            return Ok(ToolOutput::success(
                serde_json::json!({
                    "object": {
                        "chunk_ids": ["source-1:chunk:not-in-catalog"],
                        "source_coverage": [],
                        "source_relevance": []
                    },
                    "repair_rounds": 0,
                    "mode_used": "fixture"
                })
                .to_string(),
            ));
        }
        let prompt = args
            .get("prompt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let packet = prompt
            .split_once("CLOSED_EVIDENCE_PACKET=")
            .map(|(_, packet)| packet)
            .ok_or_else(|| anyhow::anyhow!("selector omitted its closed evidence packet"))?;
        let packet: serde_json::Value = serde_json::from_str(packet)?;
        let focuses = packet["focuses"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("selector packet omitted focuses"))?;
        for focus in focuses {
            assert!(
                focus["obligation_id"].is_string(),
                "selector focus omitted its stable obligation identity"
            );
            assert!(
                focus["material"].is_boolean(),
                "selector focus omitted its materiality"
            );
            assert!(
                focus["completion_criteria"].is_array(),
                "selector focus omitted its completion criteria"
            );
            assert!(
                focus["evidence_requirements"].is_object(),
                "selector focus omitted its source-quality requirements"
            );
        }
        let sources = packet["sources"]
            .as_array()
            .ok_or_else(|| anyhow::anyhow!("selector packet omitted sources"))?;
        let mut chunk_ids = Vec::new();
        for source in sources {
            let chunks = source["chunks"]
                .as_array()
                .ok_or_else(|| anyhow::anyhow!("selector source omitted chunks"))?;
            let mut retained_for_source = Vec::new();
            for preferred in &self.preferred_fragments {
                if let Some(chunk) = chunks.iter().find(|chunk| {
                    chunk["text"]
                        .as_str()
                        .is_some_and(|text| text.contains(preferred))
                }) {
                    let chunk_id = chunk["chunk_id"]
                        .as_str()
                        .ok_or_else(|| anyhow::anyhow!("selector chunk omitted ID"))?;
                    if !retained_for_source.iter().any(|seen| seen == chunk_id) {
                        retained_for_source.push(chunk_id.to_string());
                    }
                }
            }
            if retained_for_source.is_empty() {
                let chunk_id = chunks
                    .first()
                    .and_then(|chunk| chunk["chunk_id"].as_str())
                    .ok_or_else(|| anyhow::anyhow!("selector source omitted chunks"))?;
                retained_for_source.push(chunk_id.to_string());
            }
            chunk_ids.extend(retained_for_source);
        }
        let maximum = args["schema"]["properties"]["chunk_ids"]["maxItems"]
            .as_u64()
            .unwrap_or(chunk_ids.len() as u64) as usize;
        chunk_ids.truncate(maximum);
        let selected_chunk_ids = chunk_ids
            .iter()
            .map(String::as_str)
            .collect::<std::collections::HashSet<_>>();
        let mut source_coverage = Vec::new();
        let mut source_relevance = Vec::new();
        for source in sources {
            let selected = source["chunks"]
                .as_array()
                .into_iter()
                .flatten()
                .filter_map(|chunk| chunk["chunk_id"].as_str())
                .any(|chunk_id| selected_chunk_ids.contains(chunk_id));
            if !selected {
                continue;
            }
            let source_id = source["source_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("selector source omitted ID"))?;
            for focus in focuses {
                source_relevance.push(serde_json::json!({
                    "source_id": source_id,
                    "obligation_id": focus["obligation_id"],
                }));
                let completion_criterion_count = focus["completion_criteria"]
                    .as_array()
                    .map(Vec::len)
                    .unwrap_or_default();
                let roles = serde_json::json!({
                    "supporting": true,
                    "primary":
                        focus["evidence_requirements"]["primary_source_required"] == true,
                    "independent":
                        focus["evidence_requirements"]["independent_corroboration_required"]
                            == true,
                });
                source_coverage.push(serde_json::json!({
                    "source_id": source_id,
                    "obligation_id": focus["obligation_id"],
                    "completion_criterion_indexes":
                        (0..completion_criterion_count).collect::<Vec<_>>(),
                    "roles": roles,
                }));
            }
        }
        assert!(
            !focuses.is_empty(),
            "selector packet omitted semantic focuses"
        );
        Ok(ToolOutput::success(
            serde_json::json!({
                "object": {
                    "chunk_ids": chunk_ids,
                    "source_coverage": source_coverage,
                    "source_relevance": source_relevance
                },
                "repair_rounds": 0,
                "mode_used": "fixture"
            })
            .to_string(),
        ))
    }
}

#[async_trait::async_trait]
impl Tool for RetryOnceSemanticSelectorFixture {
    fn name(&self) -> &str {
        "generate_object"
    }

    fn description(&self) -> &str {
        "Fails once before selecting semantic chunk IDs from the closed evidence packet."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let web_source_selection = matches!(
            args.get("schema_name").and_then(serde_json::Value::as_str),
            Some(
                "deep_research_web_source_selection"
                    | "deep_research_supplemental_web_source_selection"
            )
        );
        if web_source_selection == self.retry_web_source_selection
            && self.calls.fetch_add(1, Ordering::SeqCst) == 0
        {
            return Ok(ToolOutput::error(
                "simulated transient semantic selector failure",
            ));
        }
        self.selector.execute(args, ctx).await
    }
}

#[async_trait::async_trait]
impl Tool for FailMatchingSemanticSelectorFixture {
    fn name(&self) -> &str {
        "generate_object"
    }

    fn description(&self) -> &str {
        "Fails only semantic selector packets containing one exact fixture fragment."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let prompt = args
            .get("prompt")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        if args["schema_name"] == self.schema_name && prompt.contains(&self.fragment) {
            return Ok(ToolOutput::error("simulated source-local shard timeout"));
        }
        self.selector.execute(args, ctx).await
    }
}

#[async_trait::async_trait]
impl Tool for FailWebSourceSelectionFixture {
    fn name(&self) -> &str {
        "generate_object"
    }

    fn description(&self) -> &str {
        "Fails web source admission while allowing fetched-text selection."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        if matches!(
            args.get("schema_name").and_then(serde_json::Value::as_str),
            Some(
                "deep_research_web_source_selection"
                    | "deep_research_supplemental_web_source_selection"
            )
        ) {
            return Ok(ToolOutput::error(
                "simulated permanent web source admission failure",
            ));
        }
        self.selector.execute(args, ctx).await
    }
}

#[async_trait::async_trait]
impl Tool for PaginatedPdfFixture {
    fn name(&self) -> &str {
        "fixture_pdf_fetch"
    }

    fn description(&self) -> &str {
        "Returns three deterministic extracted PDF ranges from one admitted source."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let offset = args
            .get("offset")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        self.offsets.lock().unwrap().push(offset);
        let second_offset = PDF_RANGE_ONE.chars().count() as u64;
        let third_offset = second_offset + PDF_RANGE_TWO.chars().count() as u64;
        let (body, next_offset) = match offset {
            0 => (PDF_RANGE_ONE, Some(second_offset)),
            value if value == second_offset => (PDF_RANGE_TWO, Some(third_offset)),
            value if value == third_offset => (PDF_RANGE_THREE, None),
            _ => return Ok(ToolOutput::error("unexpected PDF range offset")),
        };
        Ok(ToolOutput::success(body).with_metadata(serde_json::json!({
            "document_kind": "pdf",
            "content_type": "application/pdf",
            "range": {
                "offset": offset,
                "returned_chars": body.chars().count(),
                "next_offset": next_offset,
                "eof": next_offset.is_none()
            }
        })))
    }
}

#[async_trait::async_trait]
impl Tool for PaginatedHtmlFixture {
    fn name(&self) -> &str {
        "fixture_html_fetch"
    }

    fn description(&self) -> &str {
        "Returns three deterministic HTML ranges from one admitted source."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let offset = args
            .get("offset")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_default();
        self.offsets.lock().unwrap().push(offset);
        let second_offset = HTML_RANGE_ONE.chars().count() as u64;
        let third_offset = second_offset + HTML_RANGE_TWO.chars().count() as u64;
        let (body, next_offset) = match offset {
            0 => (HTML_RANGE_ONE, Some(second_offset)),
            value if value == second_offset => (HTML_RANGE_TWO, Some(third_offset)),
            value if value == third_offset => (HTML_RANGE_THREE, None),
            _ => return Ok(ToolOutput::error("unexpected HTML range offset")),
        };
        Ok(ToolOutput::success(body).with_metadata(serde_json::json!({
            "document_kind": "html",
            "content_type": "text/html; charset=utf-8",
            "range": {
                "offset": offset,
                "returned_chars": body.chars().count(),
                "next_offset": next_offset,
                "eof": next_offset.is_none()
            }
        })))
    }
}

#[async_trait::async_trait]
impl Tool for TransientFetchFixture {
    fn name(&self) -> &str {
        "fixture_transient_fetch"
    }

    fn description(&self) -> &str {
        "Fails each initial fetch and succeeds on the bounded retry."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        let url = args
            .get("url")
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default()
            .to_string();
        let attempt = {
            let mut calls = self.calls.lock().unwrap();
            calls.push(url.clone());
            calls.iter().filter(|observed| *observed == &url).count()
        };
        if attempt == 1 {
            return Ok(ToolOutput::error("typed timeout failure").with_error_kind(
                ToolErrorKind::Timeout {
                    op: "fixture fetch".to_string(),
                    duration_ms: 1_000,
                },
            ));
        }
        let output =
            format!("The bounded retry fetched substantive authoritative evidence from {url}.");
        Ok(
            ToolOutput::success(output.clone()).with_metadata(serde_json::json!({
                "source_anchors": [url],
                "document_kind": "html",
                "content_type": "text/html",
                "range": {
                    "offset": 0,
                    "returned_chars": output.chars().count(),
                    "next_offset": null,
                    "eof": true
                }
            })),
        )
    }
}

#[async_trait::async_trait]
impl Tool for UntypedFetchFailureFixture {
    fn name(&self) -> &str {
        "fixture_untyped_fetch_failure"
    }

    fn description(&self) -> &str {
        "Returns transport-like prose without a typed error classification."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        Ok(ToolOutput::error("TLS handshake timed out during lookup"))
    }
}

#[async_trait::async_trait]
impl Tool for LocalEvidenceFixture {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Returns observed and fabricated local source paths."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(
            ToolOutput::success("local evidence").with_metadata(serde_json::json!({
                "success": true,
                "source_anchors": [{
                    "tool": "read",
                    "url_or_path": "src/research.rs"
                }, {
                    "tool": "ls",
                    "url_or_path": "src/listed-only.rs"
                }],
                "structured": {
                    "sources": [{
                        "url_or_path": "src/research.rs",
                        "ranges": [{"offset": 0, "limit": 20}]
                    }, {
                        "url_or_path": "src/fabricated.rs",
                        "ranges": [{"offset": 0, "limit": 20}]
                    }, {
                        "url_or_path": "src/listed-only.rs",
                        "ranges": [{"offset": 0, "limit": 20}]
                    }]
                }
            })),
        )
    }
}

#[async_trait::async_trait]
impl Tool for NoLocalEvidenceFixture {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Returns a typed local task result without an observed source anchor."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(
            ToolOutput::success("no observed local evidence").with_metadata(serde_json::json!({
                "results": [{
                    "success": true,
                    "source_anchors": [],
                    "structured": {
                        "sources": [{
                            "url_or_path": "src/unobserved.rs",
                            "ranges": [{"offset": 0, "limit": 20}]
                        }]
                    }
                }]
            })),
        )
    }
}

#[async_trait::async_trait]
impl Tool for AbsoluteLocalCandidateFixture {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Returns one absolute candidate path inside the workspace."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(
            ToolOutput::success("absolute local candidate").with_metadata(serde_json::json!({
                "results": [{
                    "success": true,
                    "source_anchors": [],
                    "structured": {
                        "sources": [{
                            "url_or_path": self.path,
                            "ranges": [{"offset": 0, "limit": 20}]
                        }]
                    }
                }]
            })),
        )
    }
}

#[async_trait::async_trait]
impl Tool for OversizedLocalRangeFixture {
    fn name(&self) -> &str {
        "task"
    }

    fn description(&self) -> &str {
        "Returns one oversized local source and one valid sibling."
    }

    fn parameters(&self) -> serde_json::Value {
        serde_json::json!({"type": "object"})
    }

    async fn execute(
        &self,
        _args: &serde_json::Value,
        _ctx: &ToolContext,
    ) -> anyhow::Result<ToolOutput> {
        Ok(
            ToolOutput::success("mixed bounded local candidates").with_metadata(
                serde_json::json!({
                    "results": [{
                        "success": true,
                        "structured": {
                            "sources": [{
                                "url_or_path": "src/oversized.rs",
                                "ranges": [
                                    {"offset": 0, "limit": 1},
                                    {"offset": 1, "limit": 1},
                                    {"offset": 2, "limit": 1},
                                    {"offset": 3, "limit": 1}
                                ]
                            }, {
                                "url_or_path": "src/valid.rs",
                                "ranges": [{"offset": 0, "limit": 20}]
                            }]
                        }
                    }]
                }),
            ),
        )
    }
}

fn minimal_plan(
    tracks: serde_json::Value,
    search_queries: serde_json::Value,
    seed_urls: serde_json::Value,
) -> serde_json::Value {
    serde_json::json!({
        "report_title": "Retrieval fixture",
        "research_scope": "focused",
        "freshness_required": false,
        "workspace_evidence_required": false,
        "tracks": tracks,
        "search_queries": search_queries,
        "seed_urls": seed_urls,
        "budget": {
            "retrieval_timeout_ms": 30_000,
            "direct_searches": 4,
            "direct_fetches": 8
        },
        "stop_conditions": ["Retain traceable evidence or a bounded gap."]
    })
}

fn track(id: &str, title: &str, focus: &str) -> serde_json::Value {
    serde_json::json!({
        "id": id,
        "title": title,
        "focus": focus,
        "material": true,
        "questions": [focus],
        "completion_criteria": ["The focus has traceable evidence or a bounded gap."],
        "evidence_requirements": {
            "primary_source_required": false,
            "independent_corroboration_required": false
        }
    })
}

fn replace_web_tools(source: &str, search: &str, fetch: &str) -> String {
    source
        .replace("ctx.tool(\"web_search\"", &format!("ctx.tool(\"{search}\""))
        .replace("ctx.tool(\"web_fetch\"", &format!("ctx.tool(\"{fetch}\""))
        .replace("tool: \"web_search\"", &format!("tool: \"{search}\""))
        .replace("tool: \"web_fetch\"", &format!("tool: \"{fetch}\""))
}

fn workflow_args(
    query: &str,
    scope: super::DeepResearchEvidenceScope,
    plan: serde_json::Value,
    search: &str,
    fetch: &str,
) -> serde_json::Value {
    let mut args = super::deep_research_workflow_args_with_scope(query, scope);
    args["input"]["research_plan"] = plan;
    args["input"]["execution_mode"] = serde_json::json!("collect_only");
    args["limits"]["timeoutMs"] = serde_json::json!(45_000);
    args["limits"]["maxToolCalls"] = serde_json::json!(24);
    if scope == super::DeepResearchEvidenceScope::WebAndWorkspace {
        args["source"] = serde_json::Value::String(replace_web_tools(
            args["source"].as_str().expect("workflow source"),
            search,
            fetch,
        ));
    }
    args
}

async fn execute(executor: &ToolExecutor, args: &serde_json::Value) -> serde_json::Value {
    a3s_code_core::tools::register_dynamic_workflow(executor.registry());
    let result = executor
        .execute("dynamic_workflow", args)
        .await
        .expect("retrieval workflow execution");
    assert_eq!(result.exit_code, 0, "{}", result.output);
    serde_json::from_str(&result.output).expect("retrieval output")
}

fn assert_exact_calls_in_any_order(observed: &[String], expected: &[&str]) {
    let mut observed = observed.to_vec();
    observed.sort();
    let mut expected = expected
        .iter()
        .map(|value| (*value).to_string())
        .collect::<Vec<_>>();
    expected.sort();
    assert_eq!(observed, expected);
}

fn research_source_urls(output: &serde_json::Value) -> Vec<String> {
    output["research"]["results"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|result| {
            result["structured"]["sources"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .filter_map(|source| source["url_or_path"].as_str().map(str::to_string))
        .collect()
}

#[tokio::test]
async fn process_interruption_persists_completed_source_without_replaying_its_fetch() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = Arc::new(ToolExecutor::new(
        workspace.path().to_string_lossy().to_string(),
    ));
    let calls = Arc::new(Mutex::new(Vec::new()));
    let blocked_started = Arc::new(Notify::new());
    let first_url = "https://durable-effects.example/first";
    let second_url = "https://durable-effects.example/second";
    executor.register_dynamic_tool(Arc::new(InterruptedSiblingFetchFixture {
        calls: Arc::clone(&calls),
        blocked_url: second_url.to_string(),
        blocked_started: Arc::clone(&blocked_started),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let query = "Compare two independently acquired records";
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "records.comparison",
            "Independent records",
            "Compare the two acquired records"
        )]),
        serde_json::json!([]),
        serde_json::json!([first_url, second_url]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(0);
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let mut args = workflow_args(
        query,
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "unused_fixture_web_search",
        "fixture_interrupted_sibling_web_fetch",
    );
    args["run_id"] = serde_json::json!("deepresearch-independent-source-effects");
    a3s_code_core::tools::register_dynamic_workflow(executor.registry());

    let first_execution = {
        let executor = Arc::clone(&executor);
        let args = args.clone();
        tokio::spawn(async move { executor.execute("dynamic_workflow", &args).await })
    };
    tokio::time::timeout(Duration::from_secs(5), blocked_started.notified())
        .await
        .expect("the second source fetch should enter its first attempt");

    let workflow_log = workspace
        .path()
        .join(".a3s/workflow/deepresearch-independent-source-effects.jsonl");
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let completed = tokio::fs::read_to_string(&workflow_log)
                .await
                .ok()
                .is_some_and(|history| {
                    history.lines().any(|line| {
                        serde_json::from_str::<serde_json::Value>(line)
                            .ok()
                            .is_some_and(|event| {
                                event["event"]["type"] == "step_completed"
                                    && event["event"]["step_id"] == "retrieve_web_source_1"
                            })
                    })
                });
            if completed {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("the completed sibling source must be durable before the batch finishes");
    first_execution.abort();
    let _ = first_execution.await;

    let resumed = tokio::time::timeout(
        Duration::from_secs(10),
        executor.execute("dynamic_workflow", &args),
    )
    .await
    .expect("the exact interrupted run should resume")
    .expect("resumed retrieval workflow");
    assert_eq!(resumed.exit_code, 0, "{}", resumed.output);
    let output: serde_json::Value =
        serde_json::from_str(&resumed.output).expect("resumed workflow output");
    assert_eq!(
        output["research"]["metadata"]["source_count"],
        2,
        "{}",
        serde_json::to_string_pretty(&output).unwrap()
    );
    let calls = calls.lock().unwrap();
    assert_eq!(
        calls.iter().filter(|url| url.as_str() == first_url).count(),
        1,
        "a durably completed source fetch must not be replayed"
    );
    assert_eq!(
        calls
            .iter()
            .filter(|url| url.as_str() == second_url)
            .count(),
        2,
        "the ambiguous running source attempt must be redelivered"
    );
}

#[tokio::test]
async fn bootstrap_acquisition_preserves_visible_text_and_drops_a_structural_payload_prefix() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "First ranked source",
            "url": "https://bootstrap.example/first",
            "engines": ["fixture"]
        }, {
            "title": "Accountable Reuters alternative",
            "url": "https://www.reuters.com/bootstrap/second",
            "engines": ["fixture"]
        }, {
            "title": "Unspent candidate",
            "url": "https://bootstrap.example/third",
            "engines": ["fixture"]
        }]),
    }));
    let serialized_state = serde_json::json!({
        "state": "HIDDEN_STRUCTURAL_PAYLOAD".repeat(80)
    })
    .to_string();
    let encoded_markup_state = format!(
        r#"{{"transport":"{}","content":"\\u003carticle\\u003e\\u003cp\\u003eThe structurally decoded excerpt remains visible.\\u003c/p\\u003e\\u003c/article\\u003e""#,
        "HIDDEN_TRUNCATED_SERIALIZED_STATE".repeat(40)
    );
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://bootstrap.example/first".to_string(),
                format!(
                    "{serialized_state} The first fetched source contains substantive traceable bootstrap evidence.\n\
                     {encoded_markup_state}\n\
                     Your current User-Agent string appears to be from an automated process.\n\
                     <script>window.__BOOTSTRAP__ = true;</script>\n\
                     Toggle the table of contents 164 languages [Afrikaans](https://example.test/af)"
                ),
            ),
            (
                "https://www.reuters.com/bootstrap/second".to_string(),
                "The second fetched source contains separate substantive bootstrap evidence."
                    .to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "First ranked source".to_string(),
            "HIDDEN_STRUCTURAL_PAYLOAD".to_string(),
            "HIDDEN_TRUNCATED_SERIALIZED_STATE".to_string(),
            "substantive traceable bootstrap evidence".to_string(),
            "structurally decoded excerpt remains visible".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let query = "Acquire evidence before semantic planning";
    let mut plan = minimal_plan(
        serde_json::json!([track("request.primary", "Original request", query)]),
        serde_json::json!([query]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(1);
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let mut args = workflow_args(
        query,
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    args["input"]["execution_mode"] = serde_json::json!("bootstrap_acquisition");
    args["run_id"] = serde_json::json!("deepresearch-bootstrap-acquisition-test");

    let output = execute(&executor, &args).await;

    assert_eq!(*queries.lock().unwrap(), [query]);
    assert_eq!(*urls.lock().unwrap(), ["https://bootstrap.example/first"]);
    assert_eq!(output["mode"], "bootstrap_acquisition");
    assert_eq!(
        output["acquisition"]["packet"]["sources"]
            .as_array()
            .map(Vec::len),
        Some(1)
    );
    let retained_text = output["acquisition"]["packet"]["sources"][0]["chunks"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|chunk| chunk["text"].as_str())
        .collect::<Vec<_>>()
        .join(" ");
    assert!(retained_text.contains("substantive traceable bootstrap evidence"));
    assert!(retained_text.contains("User-Agent"), "{retained_text}");
    assert!(!retained_text.contains("__BOOTSTRAP__"), "{retained_text}");
    assert!(
        !retained_text.contains("HIDDEN_STRUCTURAL_PAYLOAD"),
        "{retained_text}"
    );
    assert!(
        !retained_text.contains("HIDDEN_TRUNCATED_SERIALIZED_STATE"),
        "{retained_text}"
    );
    assert!(
        retained_text.contains("structurally decoded excerpt remains visible"),
        "{retained_text}"
    );
    assert!(retained_text.contains("164 languages"), "{retained_text}");
    assert_eq!(
        output["acquisition"]["metadata"]["source_selection_mode"],
        "semantic_candidate_ids"
    );
    let history = std::fs::read_to_string(
        workspace
            .path()
            .join(".a3s/workflow/deepresearch-bootstrap-acquisition-test.jsonl"),
    )
    .expect("durable bootstrap history");
    assert!(history.lines().any(|line| {
        let event: serde_json::Value = serde_json::from_str(line).unwrap();
        event["event"]["type"] == "step_completed"
            && event["event"]["step_id"] == "checkpoint_bootstrap_acquisition"
    }));
}

#[tokio::test]
async fn bootstrap_source_admission_failure_still_acquires_bounded_candidates() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "First bounded bootstrap fallback",
            "url": "https://bootstrap-fallback-one.example/record",
            "engines": ["fixture"]
        }, {
            "title": "Second bounded bootstrap fallback",
            "url": "https://bootstrap-fallback-two.example/record",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://bootstrap-fallback-one.example/record".to_string(),
                "The first bounded bootstrap source is retained for later semantic review."
                    .to_string(),
            ),
            (
                "https://bootstrap-fallback-two.example/record".to_string(),
                "The second bounded bootstrap source is retained on a distinct host.".to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(FailWebSourceSelectionFixture {
        selector: SemanticSelectorFixture {
            preferred_fragments: Vec::new(),
            fail: false,
            invalid_selection: false,
        },
    }));
    let query = "Acquire evidence when source admission fails";
    let mut plan = minimal_plan(
        serde_json::json!([track("request.primary", "Original request", query)]),
        serde_json::json!([query]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(1);
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let mut args = workflow_args(
        query,
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    args["input"]["execution_mode"] = serde_json::json!("bootstrap_acquisition");
    args["run_id"] = serde_json::json!("deepresearch-bootstrap-fallback-test");

    let output = execute(&executor, &args).await;

    assert_eq!(*queries.lock().unwrap(), [query]);
    assert_exact_calls_in_any_order(
        &urls.lock().unwrap(),
        &[
            "https://bootstrap-fallback-one.example/record",
            "https://bootstrap-fallback-two.example/record",
        ],
    );
    assert_eq!(
        output["acquisition"]["metadata"]["source_selection_mode"],
        "bounded_discovery_fallback"
    );
    assert_eq!(
        output["acquisition"]["packet"]["sources"]
            .as_array()
            .map(Vec::len),
        Some(2)
    );
    assert_eq!(
        output["acquisition"]["packet"]["sources"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|source| source["url_or_path"].as_str())
            .collect::<Vec<_>>(),
        [
            "https://bootstrap-fallback-one.example/record",
            "https://bootstrap-fallback-two.example/record",
        ]
    );
    assert!(output
        .to_string()
        .contains("simulated permanent web source admission failure"));
}

#[tokio::test]
async fn semantic_retrieval_reuses_bootstrap_packet_without_repeating_transport() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["preserved raw evidence".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let query = "Reuse already fetched evidence";
    let plan = minimal_plan(
        serde_json::json!([track("request.primary", "Original request", query)]),
        serde_json::json!([query]),
        serde_json::json!([]),
    );
    let mut args = super::deep_research_workflow_args_with_scope(
        query,
        super::DeepResearchEvidenceScope::WebAndWorkspace,
    );
    args["input"]["research_plan"] = plan;
    args["input"]["execution_mode"] = serde_json::json!("collect_only");
    args["input"]["bootstrap_acquisition"] = serde_json::json!({
        "status": "success",
        "packet": {
            "version": 1,
            "focuses": [],
            "sources": [{
                "source_id": "bootstrap-web-source-1",
                "title": "Preserved source",
                "url_or_path": "https://bootstrap.example/preserved",
                "reliability": "Fetched and durably preserved before planning.",
                "chunks": [{
                    "chunk_id": "bootstrap-web-source-1:chunk:1",
                    "text": "This preserved raw evidence remains available after planning settles."
                }]
            }]
        },
        "errors": [],
        "metadata": {
            "source_selection_mode": "provider_round_robin",
            "fetched_count": 1
        }
    });
    args["run_id"] = serde_json::json!("deepresearch-bootstrap-reuse-test");
    args["limits"]["timeoutMs"] = serde_json::json!(45_000);
    args["limits"]["maxToolCalls"] = serde_json::json!(24);

    // No search or fetch fixture is registered. The final retrieval can only
    // succeed by consuming the immutable bootstrap packet.
    let output = execute(&executor, &args).await;

    assert_eq!(output["mode"], "inquiry_collection");
    assert_eq!(output["research"]["metadata"]["bootstrap_source_count"], 1);
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        "https://bootstrap.example/preserved"
    );
}

#[tokio::test]
async fn semantic_retrieval_reuses_local_bootstrap_packet_when_followup_has_no_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(NoLocalEvidenceFixture));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["preserved workspace evidence".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let query = "Trace the active implementation";
    let mut plan = minimal_plan(
        serde_json::json!([track("request.primary", "Original request", query)]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let mut args = super::deep_research_workflow_args_with_scope(
        query,
        super::DeepResearchEvidenceScope::LocalOnly,
    );
    args["input"]["research_plan"] = plan;
    args["input"]["execution_mode"] = serde_json::json!("collect_only");
    args["input"]["bootstrap_acquisition"] = serde_json::json!({
        "status": "partial",
        "packet": {
            "version": 1,
            "focuses": [],
            "sources": [{
                "source_id": "bootstrap-local-source-1",
                "title": "src/active.rs",
                "url_or_path": "src/active.rs",
                "reliability": "Exact workspace text restored by host read ranges.",
                "chunks": [{
                    "chunk_id": "bootstrap-local-source-1:chunk:1",
                    "text": "This preserved workspace evidence traces the active implementation."
                }]
            }]
        },
        "errors": ["A separate proposed path had no observed read or grep anchor."],
        "metadata": {
            "source_count": 1,
            "chunk_count": 1
        }
    });
    args["run_id"] = serde_json::json!("deepresearch-local-bootstrap-reuse-test");
    args["limits"]["timeoutMs"] = serde_json::json!(45_000);
    args["limits"]["maxToolCalls"] = serde_json::json!(24);

    let output = execute(&executor, &args).await;

    assert_eq!(output["mode"], "inquiry_collection");
    assert_eq!(output["research"]["metadata"]["bootstrap_source_count"], 1);
    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        "src/active.rs"
    );
    assert!(
        output["research"]["warnings"]["collection_errors"]
            .as_array()
            .is_some_and(|errors| !errors.is_empty()),
        "the retained bootstrap evidence must not erase typed collection diagnostics"
    );
}

#[tokio::test]
async fn semantic_retrieval_assigns_unique_ids_after_merging_independent_packets() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/unobserved.rs"),
        "pub const FOLLOWUP: &str = \"The follow-up packet contributes distinct evidence.\";\n",
    )
    .unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(NoLocalEvidenceFixture));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "bootstrap packet contributes".to_string(),
            "follow-up packet contributes".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let query = "Combine independently collected workspace evidence";
    let mut plan = minimal_plan(
        serde_json::json!([track("workspace", "Workspace", query)]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let mut args = super::deep_research_workflow_args_with_scope(
        query,
        super::DeepResearchEvidenceScope::LocalOnly,
    );
    args["input"]["research_plan"] = plan;
    args["input"]["execution_mode"] = serde_json::json!("collect_only");
    args["input"]["bootstrap_acquisition"] = serde_json::json!({
        "status": "success",
        "packet": {
            "version": 1,
            "focuses": [],
            "sources": [{
                "source_id": "local-source-1",
                "title": "src/bootstrap.rs",
                "url_or_path": "src/bootstrap.rs",
                "reliability": "Exact workspace text restored by a prior Host read.",
                "chunks": [{
                    "chunk_id": "local-source-1:chunk:1",
                    "text": "The bootstrap packet contributes preserved evidence."
                }]
            }]
        },
        "errors": [],
        "metadata": {}
    });
    args["run_id"] = serde_json::json!("deepresearch-independent-packet-identity-test");
    args["limits"]["timeoutMs"] = serde_json::json!(45_000);
    args["limits"]["maxToolCalls"] = serde_json::json!(24);

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["metadata"]["source_count"], 2);
    let sources = output["research"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| &result["structured"]["sources"][0])
        .collect::<Vec<_>>();
    assert_eq!(
        sources
            .iter()
            .filter_map(|source| source["url_or_path"].as_str())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from(["src/bootstrap.rs", "src/unobserved.rs"])
    );
    assert_eq!(
        sources
            .iter()
            .filter_map(|source| source["source_id"].as_str())
            .collect::<BTreeSet<_>>()
            .len(),
        2,
        "the aggregate catalog must own unique source identities"
    );
}

#[tokio::test]
async fn semantic_retrieval_searches_only_supplements_and_merges_bootstrap_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "Independent Aurora assessment",
            "url": "https://www.reuters.com/technology/aurora-assessment",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            "https://www.reuters.com/technology/aurora-assessment".to_string(),
            "The independent Aurora assessment documents deployment constraints and operating risks."
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "preserved primary Aurora evidence".to_string(),
            "deployment constraints".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let query = "Aurora";
    let supplemental = "Aurora deployment constraints independent assessment";
    let plan = minimal_plan(
        serde_json::json!([track(
            "aurora.primary",
            "Aurora evidence",
            "Establish Aurora's primary record and independent constraints",
        )]),
        serde_json::json!([query, supplemental]),
        serde_json::json!([]),
    );
    let mut args = workflow_args(
        query,
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    args["input"]["bootstrap_acquisition"] = serde_json::json!({
        "status": "success",
        "packet": {
            "version": 1,
            "focuses": [],
            "sources": [{
                "source_id": "bootstrap-web-source-1",
                "title": "Aurora primary record",
                "url_or_path": "https://docs.rs/aurora/latest/aurora",
                "reliability": "Fetched before semantic planning.",
                "chunks": [{
                    "chunk_id": "bootstrap-web-source-1:chunk:1",
                    "text": "The preserved primary Aurora evidence records the public release."
                }]
            }]
        },
        "errors": [],
        "metadata": {}
    });
    args["run_id"] = serde_json::json!("deepresearch-planned-supplement-merge-test");

    let output = execute(&executor, &args).await;

    assert_eq!(*queries.lock().unwrap(), [supplemental]);
    assert_eq!(
        *urls.lock().unwrap(),
        ["https://www.reuters.com/technology/aurora-assessment"]
    );
    let anchors = output["research"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|result| {
            result["structured"]["sources"][0]["url_or_path"]
                .as_str()
                .map(str::to_string)
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        anchors,
        BTreeSet::from([
            "https://docs.rs/aurora/latest/aurora".to_string(),
            "https://www.reuters.com/technology/aurora-assessment".to_string(),
        ])
    );
}

#[tokio::test]
async fn provider_query_and_cross_language_semantic_selection_are_preserved() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "運用監査記録",
            "url": "https://primary.example/record",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls,
        bodies: BTreeMap::from([(
            "https://primary.example/record".to_string(),
            "監査ログはサービスが正常に稼働していること、観測時刻、監査範囲、証拠境界を記録している。".to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["正常に稼働".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let provider_query = "MiXeD Case?!  日本語／中文 — café №42";
    let plan = minimal_plan(
        serde_json::json!([track("operating.state", "运行状态", "核实服务是否正常运行")]),
        serde_json::json!([provider_query]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Assess the operating condition",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(*queries.lock().unwrap(), [provider_query]);
    assert_eq!(
        output["research"]["metadata"]["evidence_selection_mode"],
        "semantic_chunk_ids_with_typed_coverage"
    );
    assert!(
        output["research"]["results"][0]["structured"]["sources"][0]["quote_or_fact"]
            .as_str()
            .is_some_and(|text| text.contains("正常に稼働"))
    );
}

#[tokio::test]
async fn search_fallback_notice_is_preserved_as_partial_research_metadata() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let url = "https://fallback.example/record";
    executor.register_dynamic_tool(Arc::new(FallbackNoticeSearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "Fallback record",
            "url": url,
            "engines": ["Brave"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: BTreeMap::from([(
            url.to_string(),
            "The fallback engine returned substantive source text that remains traceable after provider quota exhaustion."
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["substantive source text".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let query = "Preserve generic search fallback diagnostics";
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "search.fallback",
            "Search fallback",
            "Retain evidence returned by an automatic fallback engine"
        )]),
        serde_json::json!([query]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(1);
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        query,
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_fallback_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(*queries.lock().unwrap(), [query]);
    assert_eq!(output["research"]["status"], "partial_success");
    assert_eq!(
        output["research"]["metadata"]["web"]["search_fallback_count"],
        1
    );
    assert_eq!(
        output["research"]["metadata"]["web"]["search_fallback_engines"],
        serde_json::json!(["brave", "bing"])
    );
    assert_eq!(
        output["research"]["metadata"]["web"]["search_engine_selection_sources"],
        serde_json::json!(["config"])
    );
    assert!(output["research"]["warnings"]["collection_errors"]
        .as_array()
        .is_some_and(|errors| errors.iter().any(|error| error
            .as_str()
            .is_some_and(|error| error.contains("AnySearch quota is exhausted")))));
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        url
    );
}

#[tokio::test]
async fn provider_publication_date_remains_discovery_metadata() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Provider-dated record",
            "url": "https://dates.example/record",
            "published_date": "2099-12-31",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: BTreeMap::from([(
            "https://dates.example/record".to_string(),
            "The fetched record establishes the requested operational fact without publishing a date."
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["requested operational fact".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track(
            "date.provenance",
            "Date provenance",
            "Retain only dates established by fetched evidence"
        )]),
        serde_json::json!(["provider date provenance"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Do not treat provider metadata as publication evidence",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    let source = &output["research"]["results"][0]["structured"]["sources"][0];
    assert_eq!(source["url_or_path"], "https://dates.example/record");
    assert!(
        source.get("date").is_none() || source["date"].is_null(),
        "provider-supplied dates must not cross the fetched-evidence boundary: {source}"
    );
    assert!(
        !output.to_string().contains("2099-12-31"),
        "unverified discovery dates must not survive evidence materialization"
    );
}

#[tokio::test]
async fn seed_urls_are_fetched_without_publisher_specific_rewrites() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let atom_feed = format!(
        "<?xml version=\"1.0\"?><feed><title>Release notes</title><subtitle>{}</subtitle><entry><id>tag:github.com,2008:Repository/1/v1.13.2</id><updated>2025-08-15T01:43:57Z</updated><link href=\"https://github.com/example/runtime/releases/tag/v1.13.2\"/><title>v1.13.2</title><content>Latest bounded official release notes.</content></entry><entry><id>tag:github.com,2008:Repository/1/v1.13.1</id><updated>2025-03-15T22:05:29Z</updated><title>v1.13.1</title></entry></feed>",
        "feed metadata ".repeat(80)
    );
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            "https://github.com/example/runtime/releases".to_string(),
            atom_feed,
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["v1.13.2".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "release.feed",
            "Release feed",
            "Verify the latest official release record"
        )]),
        serde_json::json!([]),
        serde_json::json!(["https://github.com/example/runtime/releases"]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(0);
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        "Read a bounded official GitHub release catalog",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "unused_fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(
        *urls.lock().unwrap(),
        ["https://github.com/example/runtime/releases"]
    );
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        "https://github.com/example/runtime/releases"
    );
    let fact = output["research"]["results"][0]["structured"]["sources"][0]["quote_or_fact"]
        .as_str()
        .expect("selected release fact");
    assert!(fact.contains("v1.13.2"), "{fact}");
    assert!(fact.contains("2025-08-15T01:43:57Z"), "{fact}");
}

#[tokio::test]
async fn independent_source_effects_avoid_cross_source_batch_truncation() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let source_urls = [
        "https://batch.example/one",
        "https://batch.example/two",
        "https://batch.example/three",
    ];
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: source_urls
            .iter()
            .enumerate()
            .map(|(index, url)| {
                (
                    (*url).to_string(),
                    format!(
                        "Authoritative evidence source {}. {}",
                        index + 1,
                        "bounded evidence text ".repeat(1_550)
                    ),
                )
            })
            .collect(),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "source.effects",
            "Independent source effects",
            "Retain every independently completed source effect"
        )]),
        serde_json::json!([]),
        serde_json::json!(source_urls),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(0);
    plan["budget"]["direct_fetches"] = serde_json::json!(3);
    let args = workflow_args(
        "Retain all independently completed source effects",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "unused_fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(
        output["research"]["metadata"]["source_count"],
        3,
        "{}",
        serde_json::to_string_pretty(&output).unwrap()
    );
    assert_eq!(
        output["research"]["metadata"]["web"]["completed_source_effect_count"],
        3
    );
    assert_eq!(
        output["research"]["metadata"]["web"]["batch_output_recovery_count"],
        0
    );
    assert_exact_calls_in_any_order(&urls.lock().unwrap(), &source_urls);
    assert_eq!(
        research_source_urls(&output),
        source_urls.map(str::to_string)
    );
}

#[tokio::test]
async fn oversubscribed_provider_catalog_is_semantically_admitted_before_fetch() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let search_queries = (1..=4)
        .map(|index| format!("provider query {index}"))
        .collect::<Vec<_>>();
    let results_by_query = search_queries
        .iter()
        .enumerate()
        .map(|(query_index, query)| {
            let results = (1..=16)
                .map(|result_index| {
                    let is_authoritative = query_index == 3 && result_index == 16;
                    serde_json::json!({
                        "title": if is_authoritative {
                            "真正相关的跨语言记录".to_string()
                        } else {
                            format!("Unrelated provider result {query_index}-{result_index}")
                        },
                        "url": if is_authoritative {
                            "https://selection.example/authoritative".to_string()
                        } else {
                            format!(
                                "https://selection.example/unrelated-{query_index}-{result_index}"
                            )
                        },
                        "content": if is_authoritative {
                            "This authoritative record directly addresses the planned focus."
                                .to_string()
                        } else {
                            "A valid page that does not address the planned research focus."
                                .to_string()
                        },
                        "engines": ["fixture"]
                    })
                })
                .collect::<Vec<_>>();
            (query.clone(), serde_json::Value::Array(results))
        })
        .collect::<BTreeMap<_, _>>();
    executor.register_dynamic_tool(Arc::new(QuerySearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results_by_query,
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            "https://selection.example/authoritative".to_string(),
            "真正相关的跨语言记录提供了可追溯的一手证据，并明确回答了计划中的研究问题。"
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["真正相关的跨语言记录".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "semantic.admission",
            "语义来源准入",
            "从完整供应商候选目录中选择真正相关的跨语言记录"
        )]),
        serde_json::json!(search_queries),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        "验证跨语言语义来源准入",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_query_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(
        *urls.lock().unwrap(),
        ["https://selection.example/authoritative"]
    );
    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert!(
        output["research"]["results"][0]["structured"]["sources"][0]["quote_or_fact"]
            .as_str()
            .is_some_and(|text| text.contains("可追溯的一手证据"))
    );
}

#[tokio::test]
async fn plan_seed_does_not_displace_semantically_selected_source_portfolio() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Canonical first-party record",
            "url": "https://official.example/canonical",
            "content": "The canonical record directly establishes the planned focus.",
            "engines": ["fixture"]
        }, {
            "title": "Provider-selected independent record",
            "url": "https://selection.example/independent",
            "content": "The independent record directly corroborates the planned focus.",
            "engines": ["fixture"]
        }, {
            "title": "Unrelated provider record",
            "url": "https://selection.example/unrelated",
            "content": "This page does not address the planned focus.",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://seed.example/generic".to_string(),
                "A generic seed page does not establish the planned fact.".to_string(),
            ),
            (
                "https://official.example/canonical".to_string(),
                "The canonical first-party record establishes the planned fact.".to_string(),
            ),
            (
                "https://selection.example/independent".to_string(),
                "The independent record directly corroborates the planned fact.".to_string(),
            ),
            (
                "https://selection.example/unrelated".to_string(),
                "Unrelated but substantive provider text.".to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "Canonical first-party record".to_string(),
            "Provider-selected independent record".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let mut source_track = track(
        "source.portfolio",
        "Source portfolio",
        "Retain the canonical record and independent corroboration",
    );
    source_track["evidence_requirements"]["primary_source_required"] = serde_json::json!(true);
    source_track["evidence_requirements"]["independent_corroboration_required"] =
        serde_json::json!(true);
    let mut plan = minimal_plan(
        serde_json::json!([source_track]),
        serde_json::json!(["independent corroborating record"]),
        serde_json::json!(["https://seed.example/generic"]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let args = workflow_args(
        "Retain a typed source portfolio",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    let expected_urls = [
        "https://official.example/canonical",
        "https://selection.example/independent",
    ];
    assert_exact_calls_in_any_order(&urls.lock().unwrap(), &expected_urls);
    assert_eq!(output["research"]["metadata"]["source_count"], 2);
    assert_eq!(
        research_source_urls(&output),
        expected_urls.map(str::to_string)
    );
    let structured = output["research"]["results"]
        .as_array()
        .unwrap()
        .iter()
        .map(|result| &result["structured"])
        .collect::<Vec<_>>();
    assert_eq!(structured.len(), 2);
    assert!(structured.iter().all(|item| {
        item["sources"]
            .as_array()
            .is_some_and(|sources| sources.len() == 1)
            && item["source_coverage"]
                .as_array()
                .is_some_and(|bindings| bindings.len() == 1)
            && item["relevant_obligation_ids"]
                .as_array()
                .is_some_and(|obligations| obligations.len() == 1)
    }));
    assert!(structured
        .iter()
        .flat_map(|item| item["source_coverage"].as_array().into_iter().flatten())
        .all(|binding| binding["roles"]
            .as_array()
            .is_some_and(|roles| roles.contains(&serde_json::json!("supporting")))));

    let ledger =
        super::deep_research_evidence_ledger::accepted_evidence_ledger(&output.to_string(), None);
    assert_eq!(ledger.len(), 2);
    assert!(ledger.iter().all(|item| item.sources.len() == 1
        && item.source_coverage.len() == 1
        && item.relevant_obligation_ids == ["source.portfolio"]
        && item.source_coverage[0].source_id == item.sources[0].id));
}

#[tokio::test]
async fn typed_source_gap_drives_one_supplemental_retrieval_pass() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Independent corroborating record",
            "url": "https://loop.example/independent",
            "content": "A separately attributable record corroborates the direct finding.",
            "engines": ["fixture"]
        }, {
            "title": "Unrelated remaining record",
            "url": "https://loop.example/unrelated",
            "content": "This candidate does not address the research obligation.",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://loop.example/primary".to_string(),
                "The direct first-party record establishes the finding.".to_string(),
            ),
            (
                "https://loop.example/independent".to_string(),
                "A separately attributable record corroborates the direct finding.".to_string(),
            ),
            (
                "https://loop.example/unrelated".to_string(),
                "Unrelated but substantive source text.".to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "https://loop.example/primary".to_string(),
            "Independent corroborating record".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let mut source_track = track(
        "source.loop",
        "Source loop",
        "Close direct support and independent corroboration",
    );
    source_track["evidence_requirements"]["primary_source_required"] = serde_json::json!(true);
    source_track["evidence_requirements"]["independent_corroboration_required"] =
        serde_json::json!(true);
    let mut plan = minimal_plan(
        serde_json::json!([source_track]),
        serde_json::json!(["independent corroborating record"]),
        serde_json::json!(["https://loop.example/primary"]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let mut args = workflow_args(
        "Close a typed source portfolio with one supplemental pass",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    args["run_id"] = serde_json::json!("deepresearch-initial-checkpoint-test");

    let output = execute(&executor, &args).await;

    assert_eq!(
        *urls.lock().unwrap(),
        [
            "https://loop.example/primary",
            "https://loop.example/independent"
        ]
    );
    assert_eq!(output["research"]["metadata"]["retrieval_pass_count"], 2);
    assert_eq!(
        output["research"]["metadata"]["supplemental_retrieval_attempted"],
        true
    );
    assert_eq!(output["research"]["metadata"]["source_count"], 2);
    let ledger =
        super::deep_research_evidence_ledger::accepted_evidence_ledger(&output.to_string(), None);
    assert_eq!(ledger.len(), 2);
    assert_eq!(
        ledger
            .iter()
            .flat_map(|item| item.source_coverage.iter())
            .filter(|binding| binding
                .roles
                .contains(&a3s::research::SourceEvidenceRole::Independent))
            .map(|binding| binding.source_id.as_str())
            .collect::<std::collections::HashSet<_>>()
            .len(),
        2
    );
    let history = std::fs::read_to_string(
        workspace
            .path()
            .join(".a3s/workflow/deepresearch-initial-checkpoint-test.jsonl"),
    )
    .expect("durable retrieval history");
    let events = history
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).unwrap())
        .collect::<Vec<_>>();
    let checkpoint_sequence = events
        .iter()
        .find(|event| {
            event["event"]["type"] == "step_completed"
                && event["event"]["step_id"] == "checkpoint_initial_retrieval"
        })
        .and_then(|event| event["sequence"].as_u64())
        .expect("completed initial checkpoint");
    let supplemental_sequence = events
        .iter()
        .find(|event| {
            event["event"]["type"] == "step_created"
                && event["event"]["step_id"] == "select_supplemental_web_sources"
        })
        .and_then(|event| event["sequence"].as_u64())
        .expect("supplemental source selection");
    assert!(checkpoint_sequence < supplemental_sequence);
}

#[tokio::test]
async fn failed_initial_fetch_uses_bounded_supplemental_replacement() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let unavailable = "https://replacement.example/unavailable";
    let replacement = "https://replacement.example/authoritative";
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Initially selected unavailable source",
            "url": unavailable,
            "content": "A promising initial source that fails during fetch.",
            "engines": ["fixture"]
        }, {
            "title": "Authoritative replacement source",
            "url": replacement,
            "content": "A replacement that directly establishes the planned finding.",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            replacement.to_string(),
            "The authoritative replacement directly establishes the planned finding.".to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "Initially selected unavailable source".to_string(),
            "Authoritative replacement source".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "replacement.fetch",
            "Replacement fetch",
            "Retain traceable evidence after the first admitted fetch fails"
        )]),
        serde_json::json!(["replacement evidence"]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        "Recover one failed admitted fetch",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(*urls.lock().unwrap(), [unavailable, replacement]);
    assert_eq!(output["research"]["metadata"]["retrieval_pass_count"], 2);
    assert_eq!(
        output["research"]["metadata"]["supplemental"]["operational_gap_count"],
        1
    );
    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert!(output.to_string().contains("authoritative replacement"));
    assert!(!output.to_string().contains("promising initial source"));
}

#[tokio::test]
async fn supplemental_replacement_keeps_the_full_closed_candidate_catalog() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let unavailable = "https://registry.example/items/unavailable";
    let same_authority = "https://registry.example/items/alternate";
    let different_authority = "https://official.example/releases/current";
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Initially selected unavailable registry page",
            "url": unavailable,
            "content": "A promising registry record that retains no fetched text.",
            "engines": ["fixture"]
        }, {
            "title": "Replacement candidate B",
            "url": same_authority,
            "content": "A replacement candidate with an exact closed identity.",
            "engines": ["fixture"]
        }, {
            "title": "Replacement candidate C",
            "url": different_authority,
            "content": "Another replacement candidate with an exact closed identity.",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                same_authority.to_string(),
                "The selected replacement directly establishes the planned finding.".to_string(),
            ),
            (
                different_authority.to_string(),
                "The second selected replacement independently establishes the planned finding."
                    .to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "Initially selected unavailable registry page".to_string(),
            "Replacement candidate B".to_string(),
            "Replacement candidate C".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "replacement.identity",
            "Replacement identity",
            "Recover traceable evidence through a closed supplemental candidate"
        )]),
        serde_json::json!(["release evidence"]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        "Recover a failed fetch from the closed supplemental catalog",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_exact_calls_in_any_order(
        &urls.lock().unwrap(),
        &[unavailable, same_authority, different_authority],
    );
    assert_eq!(
        output["research"]["metadata"]["supplemental"]["web"]["failed_candidate_count"],
        1
    );
    assert_eq!(output["research"]["metadata"]["source_count"], 2);
    assert_eq!(
        research_source_urls(&output),
        [same_authority.to_string(), different_authority.to_string()]
    );
}

#[tokio::test]
async fn transient_web_source_selector_failure_replays_only_source_admission() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    let selector_calls = Arc::new(AtomicUsize::new(0));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "Unrelated discovery candidate",
            "url": "https://source-retry.example/unrelated",
            "content": "Unrelated discovery metadata.",
            "engines": ["fixture"]
        }, {
            "title": "Authoritative source retry record",
            "url": "https://source-retry.example/authoritative",
            "content": "The authoritative source retry record addresses the focus.",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://source-retry.example/unrelated".to_string(),
                "Unrelated but substantive source text.".to_string(),
            ),
            (
                "https://source-retry.example/authoritative".to_string(),
                "The authoritative source retry record remains traceable after recovery."
                    .to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(RetryOnceSemanticSelectorFixture {
        calls: Arc::clone(&selector_calls),
        selector: SemanticSelectorFixture {
            preferred_fragments: vec!["authoritative source retry record".to_string()],
            fail: false,
            invalid_selection: false,
        },
        retry_web_source_selection: true,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "source.retry",
            "Source retry",
            "Retain the authoritative source retry record"
        )]),
        serde_json::json!(["source selector recovery"]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        "Verify source selector recovery",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    a3s_code_core::tools::register_dynamic_workflow(executor.registry());

    let result = executor
        .execute("dynamic_workflow", &args)
        .await
        .expect("retrieval workflow execution");

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert_eq!(selector_calls.load(Ordering::SeqCst), 2);
    assert_eq!(queries.lock().unwrap().len(), 1);
    assert_eq!(
        *urls.lock().unwrap(),
        ["https://source-retry.example/authoritative"]
    );
    let steps = result
        .metadata
        .as_ref()
        .and_then(|metadata| metadata["dynamic_workflow"]["snapshot"]["steps"].as_object())
        .expect("durable workflow steps");
    assert_eq!(steps.len(), 5);
    assert_eq!(steps["discover_web_sources"]["attempt"], 1);
    assert_eq!(steps["select_web_sources"]["attempt"], 2);
    assert_eq!(steps["retrieve_web_source_1"]["attempt"], 1);
    assert!(!steps.contains_key("retrieve_web"));
    assert_eq!(steps["select_evidence_chunks"]["attempt"], 1);
    assert_eq!(steps["checkpoint_initial_retrieval"]["attempt"], 1);
}

#[tokio::test]
async fn fetch_candidates_preserve_query_and_provider_result_order() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    let first_query = "First exact provider query";
    let second_query = "第二个精确查询";
    let first_urls = [
        "https://order-a.example/one",
        "https://order-a.example/two",
        "https://order-c.example/three",
    ];
    let second_urls = ["https://order-b.example/one", "https://order-d.example/two"];
    executor.register_dynamic_tool(Arc::new(QuerySearchFixture {
        queries: Arc::clone(&queries),
        results_by_query: BTreeMap::from([
            (
                first_query.to_string(),
                serde_json::json!(first_urls
                    .iter()
                    .map(|url| serde_json::json!({
                        "title": url,
                        "url": url,
                        "engines": ["fixture"]
                    }))
                    .collect::<Vec<_>>()),
            ),
            (
                second_query.to_string(),
                serde_json::json!(second_urls
                    .iter()
                    .map(|url| serde_json::json!({
                        "title": url,
                        "url": url,
                        "engines": ["fixture"]
                    }))
                    .collect::<Vec<_>>()),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: first_urls
            .iter()
            .chain(second_urls.iter())
            .map(|url| {
                (
                    (*url).to_string(),
                    format!("Substantive provider-ordered evidence from {url}."),
                )
            })
            .collect(),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "provider.order",
            "Provider order",
            "Preserve provider discovery order"
        )]),
        serde_json::json!([first_query, second_query]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(4);
    let args = workflow_args(
        "Preserve provider discovery order",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_query_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_exact_calls_in_any_order(&queries.lock().unwrap(), &[first_query, second_query]);
    let expected_urls = [first_urls[0], first_urls[1], first_urls[2], second_urls[0]];
    assert_exact_calls_in_any_order(&urls.lock().unwrap(), &expected_urls);
    assert_eq!(output["research"]["metadata"]["source_count"], 4);
    assert_eq!(
        research_source_urls(&output),
        expected_urls.map(str::to_string)
    );
}

#[tokio::test]
async fn oversized_chunk_catalog_fails_closed_without_positional_sampling() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let source_urls = (1..=8)
        .map(|index| format!("https://overflow.example/evidence-{index}"))
        .collect::<Vec<_>>();
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!(source_urls
            .iter()
            .map(|url| serde_json::json!({
                "title": url,
                "url": url,
                "engines": ["fixture"]
            }))
            .collect::<Vec<_>>()),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: source_urls
            .iter()
            .enumerate()
            .map(|(source_index, url)| {
                let body = (0..170)
                    .map(|chunk_index| {
                        format!(
                            "OVERFLOW_SECRET_EVIDENCE_{}_{chunk_index:03} {}",
                            source_index + 1,
                            "bounded-source-content ".repeat(15)
                        )
                    })
                    .collect::<Vec<_>>()
                    .join("\n");
                (url.clone(), body)
            })
            .collect(),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "closed.catalog",
            "Closed catalog",
            "Retain a complete bounded chunk catalog or fail closed"
        )]),
        serde_json::json!(["oversized evidence catalog"]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(8);
    let args = workflow_args(
        "Retain a complete bounded catalog",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "failed");
    assert_eq!(output["research"]["metadata"]["source_count"], 0);
    let catalog_chunk_count = output["research"]["metadata"]["web"]["catalog_chunk_count"]
        .as_u64()
        .unwrap_or_default();
    assert!(
        catalog_chunk_count > 1_280,
        "fixture produced only {catalog_chunk_count} chunks"
    );
    assert!(output.to_string().contains("closed catalog limit"));
    assert!(!output.to_string().contains("OVERFLOW_SECRET_EVIDENCE"));
}

#[tokio::test]
async fn eight_source_catalog_uses_byte_bounded_multi_source_selectors() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    let source_urls = (1..=8)
        .map(|index| format!("https://source-local-selection.example/source-{index}"))
        .collect::<Vec<_>>();
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!(source_urls
            .iter()
            .map(|url| serde_json::json!({
                "title": url,
                "url": url,
                "engines": ["fixture"]
            }))
            .collect::<Vec<_>>()),
    }));
    let bodies = source_urls
        .iter()
        .enumerate()
        .map(|(source_index, url)| {
            let lines = (1..=30)
                .map(|chunk_index| {
                    format!(
                        "SHARDED_TARGET_{}_{} {}",
                        source_index + 1,
                        chunk_index,
                        "complete semantic shard evidence ".repeat(19)
                    )
                })
                .collect::<Vec<_>>()
                .join("\n");
            (url.clone(), lines)
        })
        .collect::<BTreeMap<_, _>>();
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies,
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: (1..=8)
            .flat_map(|source| {
                [1, 10, 20, 30]
                    .into_iter()
                    .map(move |chunk| format!("SHARDED_TARGET_{source}_{chunk}"))
            })
            .collect(),
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track(
            "selector.sources",
            "Semantic source selectors",
            "Retain the late source-local target after every chunk is semantically considered"
        )]),
        serde_json::json!(["complete source-local semantic catalog"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Verify complete source-local semantic selection",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    a3s_code_core::tools::register_dynamic_workflow(executor.registry());

    let result = executor
        .execute("dynamic_workflow", &args)
        .await
        .expect("source-local retrieval workflow execution");

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert_eq!(queries.lock().unwrap().len(), 1);
    let expected_urls = source_urls.iter().map(String::as_str).collect::<Vec<_>>();
    assert_exact_calls_in_any_order(&urls.lock().unwrap(), &expected_urls);
    let output: serde_json::Value =
        serde_json::from_str(&result.output).expect("sharded retrieval output");
    assert_eq!(
        output["research"]["status"],
        "success",
        "{}",
        serde_json::to_string_pretty(&output).unwrap()
    );
    let selector_shard_count = output["research"]["metadata"]["semantic_selection_shard_count"]
        .as_u64()
        .expect("selector shard count");
    assert!(
        selector_shard_count > 0 && selector_shard_count < 8,
        "small sources should share byte-bounded selector packets"
    );
    assert_eq!(research_source_urls(&output), source_urls);
    assert!(
        output["research"]["metadata"]["catalog_chunk_count"]
            .as_u64()
            .is_some_and(|count| count > 192 && count <= 384),
        "the complete eight-source catalog must cross the retired 192-chunk ceiling"
    );
    assert!(
        output["research"]["metadata"]["semantic_selection_candidate_count"]
            .as_u64()
            .is_some_and(|count| count > 0 && count <= 32)
    );
    assert!(output["research"]["results"]
        .as_array()
        .into_iter()
        .flatten()
        .flat_map(|result| {
            result["structured"]["sources"]
                .as_array()
                .into_iter()
                .flatten()
        })
        .flat_map(|source| source["evidence_excerpts"].as_array().into_iter().flatten())
        .any(|excerpt| excerpt["quote_or_fact"]
            .as_str()
            .is_some_and(|text| text.contains("SHARDED_TARGET_8_30"))));
    assert_eq!(
        output["research"]["results"]
            .as_array()
            .into_iter()
            .flatten()
            .flat_map(|result| {
                result["structured"]["key_evidence"]
                    .as_array()
                    .into_iter()
                    .flatten()
            })
            .count(),
        32,
        "every source keeps its own bounded candidate allowance inside packed selectors"
    );

    let steps = result
        .metadata
        .as_ref()
        .and_then(|metadata| metadata["dynamic_workflow"]["snapshot"]["steps"].as_object())
        .expect("source-local durable workflow steps");
    assert_eq!(steps["discover_web_sources"]["attempt"], 1);
    for index in 1..=8 {
        assert_eq!(steps[&format!("retrieve_web_source_{index}")]["attempt"], 1);
    }
    assert!(!steps.contains_key("retrieve_web"));
    assert!(!steps.contains_key("select_evidence_chunks"));
    let mut shard_attempts = (1..=selector_shard_count)
        .map(|index| {
            steps[&format!("select_evidence_chunks_shard_{index}")]["attempt"]
                .as_u64()
                .expect("shard attempt")
        })
        .collect::<Vec<_>>();
    shard_attempts.sort_unstable();
    assert!(shard_attempts.iter().all(|attempt| *attempt == 1));
}

#[tokio::test]
async fn failed_shards_promote_no_own_text_but_preserve_valid_siblings() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let failed_url = "https://partial-shards.example/failed";
    let retained_url = "https://partial-shards.example/retained";
    let body = |prefix: &str| {
        (1..=12)
            .map(|index| {
                format!(
                    "{prefix}_{index:02} {}",
                    "bounded source-local semantic evidence ".repeat(16)
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    };
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: BTreeMap::from([
            (failed_url.to_string(), body("FAILED_SHARD_TEXT")),
            (retained_url.to_string(), body("RETAINED_SHARD_TEXT")),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(FailMatchingSemanticSelectorFixture {
        schema_name: "deep_research_evidence_shard_selection",
        fragment: "FAILED_SHARD_TEXT".to_string(),
        selector: SemanticSelectorFixture {
            preferred_fragments: vec!["RETAINED_SHARD_TEXT_12".to_string()],
            fail: false,
            invalid_selection: false,
        },
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "partial.shards",
            "Partial shards",
            "Preserve only independently validated source-local shard text"
        )]),
        serde_json::json!([]),
        serde_json::json!([failed_url, retained_url]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(0);
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let args = workflow_args(
        "Retain validated siblings after one source-local selector fails",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "unused_fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "partial_success");
    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_failed_shard_count"],
        1
    );
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_recovery_shard_count"],
        2
    );
    let encoded = output.to_string();
    assert!(encoded.contains("RETAINED_SHARD_TEXT"));
    assert!(!encoded.contains("FAILED_SHARD_TEXT"));
    assert!(encoded.contains("simulated source-local shard timeout"));
}

#[tokio::test]
async fn failed_source_local_selection_drops_only_that_source() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let failed_url = "https://partial-reduction.example/failed";
    let retained_url = "https://partial-reduction.example/retained";
    let failed_body = (1..=40)
        .map(|index| {
            format!(
                "FAILED_SOURCE_REDUCTION_{index:02} {}",
                "bounded source reduction evidence ".repeat(18)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: BTreeMap::from([
            (failed_url.to_string(), failed_body),
            (
                retained_url.to_string(),
                "RETAINED_WITHOUT_REDUCTION direct independently validated evidence.".to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(FailMatchingSemanticSelectorFixture {
        schema_name: "deep_research_evidence_shard_selection",
        fragment: "FAILED_SOURCE_REDUCTION".to_string(),
        selector: SemanticSelectorFixture {
            preferred_fragments: vec![
                "FAILED_SOURCE_REDUCTION_05".to_string(),
                "FAILED_SOURCE_REDUCTION_08".to_string(),
                "FAILED_SOURCE_REDUCTION_15".to_string(),
                "FAILED_SOURCE_REDUCTION_18".to_string(),
                "FAILED_SOURCE_REDUCTION_25".to_string(),
                "FAILED_SOURCE_REDUCTION_28".to_string(),
                "FAILED_SOURCE_REDUCTION_35".to_string(),
                "FAILED_SOURCE_REDUCTION_38".to_string(),
                "RETAINED_WITHOUT_REDUCTION".to_string(),
            ],
            fail: false,
            invalid_selection: false,
        },
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "partial.source-reduction",
            "Partial source reduction",
            "Drop only the source whose final bounded reduction fails"
        )]),
        serde_json::json!([]),
        serde_json::json!([failed_url, retained_url]),
    );
    plan["budget"]["direct_searches"] = serde_json::json!(0);
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let args = workflow_args(
        "Retain independent sources after one source reducer fails",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "unused_fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "partial_success");
    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_failed_shard_count"],
        1
    );
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_recovery_shard_count"],
        2
    );
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_source_reduction_count"],
        0
    );
    let encoded = output.to_string();
    assert!(encoded.contains("RETAINED_WITHOUT_REDUCTION"));
    assert!(!encoded.contains("FAILED_SOURCE_REDUCTION"));
    assert!(encoded.contains("simulated source-local shard timeout"));
}

#[tokio::test]
async fn large_source_fits_one_bounded_selector_without_redundant_reduction() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    let source_url = "https://source-reduction.example/complete";
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "Complete source reduction fixture",
            "url": source_url,
            "engines": ["fixture"]
        }]),
    }));
    let body = (1..=70)
        .map(|chunk_index| {
            format!(
                "SOURCE_REDUCER_TARGET_{chunk_index} {}",
                "closed semantic source evidence ".repeat(20)
            )
        })
        .collect::<Vec<_>>()
        .join("\n");
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(source_url.to_string(), body)]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: [70, 60, 50, 35, 20, 1]
            .into_iter()
            .map(|index| format!("SOURCE_REDUCER_TARGET_{index}"))
            .collect(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "selector.source-reduction",
            "Semantic source reduction",
            "Retain the strongest late candidate while enforcing the per-source evidence limit"
        )]),
        serde_json::json!(["complete source semantic reduction"]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(1);
    let args = workflow_args(
        "Verify semantic per-source reduction",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    a3s_code_core::tools::register_dynamic_workflow(executor.registry());

    let result = executor
        .execute("dynamic_workflow", &args)
        .await
        .expect("semantic source-reduction workflow");

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert_eq!(queries.lock().unwrap().len(), 1);
    let fetched_urls = urls.lock().unwrap();
    assert!(
        !fetched_urls.is_empty() && fetched_urls.iter().all(|url| url == source_url),
        "batch-output recovery may repeat only the exact closed URL: {fetched_urls:?}"
    );
    drop(fetched_urls);
    let output: serde_json::Value =
        serde_json::from_str(&result.output).expect("source-reduction output");
    assert_eq!(
        output["research"]["status"],
        "success",
        "{}",
        serde_json::to_string_pretty(&output).unwrap()
    );
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_shard_count"],
        1
    );
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_source_reduction_count"],
        0
    );
    assert_eq!(
        output["research"]["metadata"]["semantic_selection_materialized_count"],
        4
    );
    let excerpts = output["research"]["results"][0]["structured"]["sources"][0]
        ["evidence_excerpts"]
        .as_array()
        .expect("bounded source excerpts");
    assert_eq!(excerpts.len(), 4);
    assert!(excerpts.iter().any(|excerpt| excerpt["quote_or_fact"]
        .as_str()
        .is_some_and(|text| text.contains("SOURCE_REDUCER_TARGET_70"))));

    let steps = result
        .metadata
        .as_ref()
        .and_then(|metadata| metadata["dynamic_workflow"]["snapshot"]["steps"].as_object())
        .expect("source-reduction durable steps");
    assert_eq!(steps["select_evidence_chunks_shard_1"]["attempt"], 1);
    assert!(!steps.contains_key("select_evidence_chunks_shard_2"));
    assert!(!steps.contains_key("select_evidence_chunks_source_1"));
    assert!(!steps.contains_key("select_evidence_chunks"));
}

#[tokio::test]
async fn transient_selector_failure_retries_only_the_durable_selection_step() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    let selector_calls = Arc::new(AtomicUsize::new(0));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "Durable selector record",
            "url": "https://selector-retry.example/record",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            "https://selector-retry.example/record".to_string(),
            "The durable semantic selection retry retains this exact authoritative evidence."
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(RetryOnceSemanticSelectorFixture {
        calls: Arc::clone(&selector_calls),
        selector: SemanticSelectorFixture {
            preferred_fragments: vec!["durable semantic selection retry".to_string()],
            fail: false,
            invalid_selection: false,
        },
        retry_web_source_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track(
            "selector.retry",
            "Selector retry",
            "Verify durable semantic selection recovery"
        )]),
        serde_json::json!(["durable selector evidence"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Verify durable semantic selection recovery",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );
    a3s_code_core::tools::register_dynamic_workflow(executor.registry());

    let result = executor
        .execute("dynamic_workflow", &args)
        .await
        .expect("retrieval workflow execution");

    assert_eq!(result.exit_code, 0, "{}", result.output);
    assert_eq!(selector_calls.load(Ordering::SeqCst), 2);
    assert_eq!(queries.lock().unwrap().len(), 1);
    assert_eq!(urls.lock().unwrap().len(), 1);
    let output: serde_json::Value = serde_json::from_str(&result.output).expect("retrieval output");
    assert_eq!(output["research"]["status"], "success");
    assert!(
        output["research"]["results"][0]["structured"]["sources"][0]["quote_or_fact"]
            .as_str()
            .is_some_and(|text| text.contains("exact authoritative evidence"))
    );
    let steps = result
        .metadata
        .as_ref()
        .and_then(|metadata| metadata["dynamic_workflow"]["snapshot"]["steps"].as_object())
        .expect("durable workflow steps");
    assert_eq!(steps.len(), 5);
    assert_eq!(steps["discover_web_sources"]["attempt"], 1);
    assert_eq!(steps["select_web_sources"]["attempt"], 1);
    assert_eq!(steps["retrieve_web_source_1"]["attempt"], 1);
    assert!(!steps.contains_key("retrieve_web"));
    assert_eq!(steps["select_evidence_chunks"]["attempt"], 2);
    assert_eq!(steps["checkpoint_initial_retrieval"]["attempt"], 1);
}

#[tokio::test]
async fn source_admission_failure_uses_bounded_discovery_fallback_before_chunk_review() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let queries = Arc::new(Mutex::new(Vec::new()));
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::clone(&queries),
        results: serde_json::json!([{
            "title": "Fallback candidate one",
            "url": "https://fallback-one.example/record",
            "content": "A candidate that requires fetched-text review.",
            "engines": ["fixture"]
        }, {
            "title": "Fallback candidate two",
            "url": "https://fallback-two.example/record",
            "content": "An independent candidate on another host.",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://fallback-one.example/record".to_string(),
                "The verified fallback evidence directly resolves the requested focus.".to_string(),
            ),
            (
                "https://fallback-two.example/record".to_string(),
                "The second fetched candidate remains available for closed review.".to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(FailWebSourceSelectionFixture {
        selector: SemanticSelectorFixture {
            preferred_fragments: vec!["verified fallback evidence".to_string()],
            fail: false,
            invalid_selection: false,
        },
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "fallback.review",
            "Fallback review",
            "Review verified fallback evidence"
        )]),
        serde_json::json!(["fallback evidence"]),
        serde_json::json!([]),
    );
    plan["budget"]["direct_fetches"] = serde_json::json!(2);
    let args = workflow_args(
        "Verify source-admission failure recovery",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(queries.lock().unwrap().len(), 1);
    let expected_urls = [
        "https://fallback-one.example/record",
        "https://fallback-two.example/record",
    ];
    assert_exact_calls_in_any_order(&urls.lock().unwrap(), &expected_urls);
    assert_eq!(
        output["research"]["metadata"]["web"]["source_selection_mode"],
        "bounded_discovery_fallback"
    );
    assert_eq!(output["research"]["metadata"]["source_count"], 2);
    assert_eq!(
        research_source_urls(&output),
        expected_urls.map(str::to_string)
    );
    assert!(output.to_string().contains("verified fallback evidence"));
    assert!(output
        .to_string()
        .contains("simulated permanent web source admission failure"));
}

#[tokio::test]
async fn selector_failure_promotes_no_fetched_text() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Fetched record",
            "url": "https://failure.example/record",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: BTreeMap::from([(
            "https://failure.example/record".to_string(),
            "RAW_SECRET_EVIDENCE must never be promoted when semantic selection fails.".to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: true,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track("failure.boundary", "Boundary", "Verify the boundary")]),
        serde_json::json!(["boundary record"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Verify the boundary",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "failed");
    assert_eq!(output["research"]["metadata"]["source_count"], 0);
    assert_eq!(
        output["research"]["results"].as_array().map(Vec::len),
        Some(0)
    );
    assert!(!output.to_string().contains("RAW_SECRET_EVIDENCE"));
}

#[tokio::test]
async fn selector_id_outside_closed_catalog_promotes_no_fetched_text() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Fetched record",
            "url": "https://invalid-selection.example/record",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::new(Mutex::new(Vec::new())),
        bodies: BTreeMap::from([(
            "https://invalid-selection.example/record".to_string(),
            "OUT_OF_CATALOG_SECRET must never be promoted from fetched text.".to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: true,
    }));
    let plan = minimal_plan(
        serde_json::json!([track(
            "closed.catalog",
            "Closed catalog",
            "Verify selector IDs against the fetched catalog",
        )]),
        serde_json::json!(["closed catalog record"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Verify the closed evidence catalog",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "failed");
    assert_eq!(output["research"]["metadata"]["source_count"], 0);
    assert_eq!(
        output["research"]["results"].as_array().map(Vec::len),
        Some(0)
    );
    assert!(!output.to_string().contains("OUT_OF_CATALOG_SECRET"));
    assert!(output.to_string().contains("outside the closed catalog"));
}

#[tokio::test]
async fn pdf_additional_ranges_remain_one_source_in_one_retrieval_pass() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let offsets = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(PaginatedPdfFixture {
        offsets: Arc::clone(&offsets),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "双阶段检索方法".to_string(),
            "引用完整率下降".to_string(),
            "只覆盖英语技术主题".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([
            track("method", "方法", "双阶段检索方法"),
            track("evaluation", "评测", "引用完整率下降"),
            track("limitations", "限制", "只覆盖英语技术主题")
        ]),
        serde_json::json!([]),
        serde_json::json!(["https://papers.example/report.pdf"]),
    );
    let args = workflow_args(
        "分析报告的方法、评测与限制",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_pdf_fetch",
    );

    let output = execute(&executor, &args).await;

    let second_offset = PDF_RANGE_ONE.chars().count() as u64;
    let third_offset = second_offset + PDF_RANGE_TWO.chars().count() as u64;
    assert_eq!(*offsets.lock().unwrap(), [0, second_offset, third_offset]);
    assert_eq!(
        output["research"]["metadata"]["web"]["document_range_count"],
        3
    );
    let sources = output["research"]["results"][0]["structured"]["sources"]
        .as_array()
        .expect("PDF sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0]["evidence_excerpts"].as_array().map(Vec::len),
        Some(3)
    );
}

#[tokio::test]
async fn html_additional_ranges_reach_late_article_evidence() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let offsets = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(PaginatedHtmlFixture {
        offsets: Arc::clone(&offsets),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec![
            "阶段一背景".to_string(),
            "阶段二进展".to_string(),
            "阶段三结论".to_string(),
        ],
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([
            track("stage-one", "阶段一", "阶段一背景"),
            track("stage-two", "阶段二", "阶段二进展"),
            track("stage-three", "阶段三", "阶段三结论")
        ]),
        serde_json::json!([]),
        serde_json::json!(["https://records.example/multi-stage.html"]),
    );
    let args = workflow_args(
        "分析项目从阶段一到阶段三的完整记录",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_html_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(
        *offsets.lock().unwrap(),
        [0, HTML_RANGE_ONE.chars().count() as u64]
    );
    assert_eq!(
        output["research"]["metadata"]["web"]["document_range_count"],
        2
    );
    let sources = output["research"]["results"][0]["structured"]["sources"]
        .as_array()
        .expect("HTML sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(
        sources[0]["evidence_excerpts"].as_array().map(Vec::len),
        Some(2)
    );
    assert!(sources[0]["evidence_excerpts"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|excerpt| excerpt["quote_or_fact"]
            .as_str()
            .is_some_and(|text| text.contains("阶段三结论"))));
}

#[tokio::test]
async fn visible_constructor_like_text_is_not_removed_by_vocabulary() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let source_url = "https://records.example.test/generic-project";
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            source_url.to_string(),
            "项目机构公布了第三阶段的最终记录。[完整记录](https://records.example.test/final) var swiper\\_results = new Swiper(\"#results .swiper\", { navigation: { nextEl: \".next\" } });"
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["第三阶段的最终记录".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track("stage-three", "阶段三", "第三阶段的最终记录")]),
        serde_json::json!([]),
        serde_json::json!([source_url]),
    );
    let args = workflow_args(
        "项目状态记录",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(*urls.lock().unwrap(), [source_url]);
    assert!(output.to_string().contains("第三阶段的最终记录"));
    assert!(output.to_string().contains("Swiper"));
    assert!(output.to_string().contains("swiper\\\\_results"));
}

#[tokio::test]
async fn visible_serialized_text_is_not_removed_by_vocabulary() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    let source_url = "https://records.example.test/generic-project";
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([(
            source_url.to_string(),
            r#"项目机构公布了第三阶段的最终记录。 },{\"type\":\"keyValue\",\"key\":\"ddna_timeout\",\"value\":\"5000\"},{\"type\":\"keyValue\",\"key\":\"enabletracking\",\"value\":true}"#
                .to_string(),
        )]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: vec!["第三阶段的最终记录".to_string()],
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track("stage-three", "阶段三", "第三阶段的最终记录")]),
        serde_json::json!([]),
        serde_json::json!([source_url]),
    );
    let args = workflow_args(
        "项目状态记录",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;
    let rendered = output.to_string();

    assert_eq!(*urls.lock().unwrap(), [source_url]);
    assert!(rendered.contains("第三阶段的最终记录"), "{rendered}");
    assert!(rendered.contains("keyValue"), "{rendered}");
    assert!(rendered.contains("ddna_"), "{rendered}");
    assert!(rendered.contains("enabletracking"), "{rendered}");
}

#[tokio::test]
async fn local_only_retrieval_promotes_and_discloses_only_host_verified_paths() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/research.rs"),
        "pub const RESEARCH_BOUNDARY: &str = \"The observed file defines the research boundary from exact workspace text.\";\n",
    )
    .unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(LocalEvidenceFixture));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "workspace",
            "Workspace",
            "Inspect observed workspace evidence"
        )]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let args = workflow_args(
        "Inspect the local workspace",
        super::DeepResearchEvidenceScope::LocalOnly,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "partial_success");
    assert_eq!(
        output["research"]["metadata"]["evidence_selection_mode"],
        "semantic_chunk_ids_with_typed_coverage"
    );
    let sources = output["research"]["results"][0]["structured"]["sources"]
        .as_array()
        .expect("local sources");
    assert_eq!(sources.len(), 1);
    assert_eq!(sources[0]["url_or_path"], "src/research.rs");
    assert!(sources[0]["quote_or_fact"]
        .as_str()
        .is_some_and(|text| text.contains("exact workspace text")));
    assert_eq!(
        output["research"]["results"][0]["structured"]["gaps"],
        serde_json::json!([]),
        "collection diagnostics must not become semantic research gaps"
    );
    assert!(output["research"]["warnings"]["collection_errors"]
        .as_array()
        .is_some_and(|errors| !errors.is_empty()));
    assert!(!output.to_string().contains("fabricated.rs"));
    assert!(!output.to_string().contains("listed-only.rs"));
}

#[tokio::test]
async fn local_only_retrieval_revalidates_unobserved_candidates_with_host_read() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/unobserved.rs"),
        "pub const HOST_VERIFIED: &str = \"Only the host-verified range becomes evidence.\";\n",
    )
    .unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(NoLocalEvidenceFixture));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "workspace",
            "Workspace",
            "Inspect host-verified workspace evidence"
        )]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let args = workflow_args(
        "Inspect the local workspace",
        super::DeepResearchEvidenceScope::LocalOnly,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        "src/unobserved.rs"
    );
    assert!(
        output["research"]["results"][0]["structured"]["sources"][0]["quote_or_fact"]
            .as_str()
            .is_some_and(|text| text.contains("host-verified range"))
    );
    assert_eq!(
        output["research"]["metadata"]["local"]["observed_source_count"],
        1
    );
}

#[tokio::test]
async fn local_only_retrieval_uses_the_exact_canonical_host_read_anchor() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    let source_path = workspace.path().join("src/canonical.rs");
    std::fs::write(
        &source_path,
        "pub const CANONICAL: &str = \"The canonical host anchor identifies this evidence.\";\n",
    )
    .unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(AbsoluteLocalCandidateFixture {
        path: source_path.to_string_lossy().to_string(),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "workspace",
            "Workspace",
            "Inspect canonically identified workspace evidence"
        )]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let args = workflow_args(
        "Inspect the local workspace",
        super::DeepResearchEvidenceScope::LocalOnly,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        "src/canonical.rs"
    );
    assert!(
        !output
            .to_string()
            .contains(&source_path.to_string_lossy().to_string()),
        "the absolute candidate must be replaced by the canonical host anchor"
    );
}

#[tokio::test]
async fn local_only_retrieval_drops_one_oversized_source_without_losing_valid_siblings() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    std::fs::write(
        workspace.path().join("src/oversized.rs"),
        "first\nsecond\nthird\nfourth\n",
    )
    .unwrap();
    std::fs::write(
        workspace.path().join("src/valid.rs"),
        "pub const VALID: &str = \"A valid sibling remains publishable.\";\n",
    )
    .unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(OversizedLocalRangeFixture));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "workspace",
            "Workspace",
            "Inspect bounded workspace evidence"
        )]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let args = workflow_args(
        "Inspect the local workspace",
        super::DeepResearchEvidenceScope::LocalOnly,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["metadata"]["source_count"], 1);
    assert_eq!(
        output["research"]["results"][0]["structured"]["sources"][0]["url_or_path"],
        "src/valid.rs"
    );
    assert!(
        !output.to_string().contains("src/oversized.rs"),
        "the rejected candidate path must not leak into evidence or diagnostics"
    );
}

#[tokio::test]
async fn local_text_is_not_promoted_when_closed_chunk_selection_fails() {
    let workspace = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(workspace.path().join("src")).unwrap();
    let exact_text =
        "This exact local sentence must remain behind the failed semantic selector boundary.";
    std::fs::write(
        workspace.path().join("src/research.rs"),
        format!("pub const LOCAL_EVIDENCE: &str = \"{exact_text}\";\n"),
    )
    .unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    executor.register_dynamic_tool(Arc::new(LocalEvidenceFixture));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: true,
        invalid_selection: false,
    }));
    let mut plan = minimal_plan(
        serde_json::json!([track(
            "workspace",
            "Workspace",
            "Inspect observed workspace evidence"
        )]),
        serde_json::json!([]),
        serde_json::json!([]),
    );
    plan["workspace_evidence_required"] = serde_json::json!(true);
    let args = workflow_args(
        "Inspect the local workspace",
        super::DeepResearchEvidenceScope::LocalOnly,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(output["research"]["status"], "failed");
    assert_eq!(output["research"]["metadata"]["source_count"], 0);
    assert!(!output.to_string().contains(exact_text));
}

#[tokio::test]
async fn url_path_vocabulary_does_not_preclassify_fetchability() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let urls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Avatar",
            "url": "https://cdn.example/avatar.png",
            "engines": ["fixture"]
        }, {
            "title": "Archive",
            "url": "https://downloads.example/research.zip",
            "engines": ["fixture"]
        }, {
            "title": "Evidence",
            "url": "https://valid.example/evidence",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TextFetchFixture {
        urls: Arc::clone(&urls),
        bodies: BTreeMap::from([
            (
                "https://cdn.example/avatar.png".to_string(),
                "The first endpoint returns substantive traceable evidence for the requested focus."
                    .to_string(),
            ),
            (
                "https://downloads.example/research.zip".to_string(),
                "The second endpoint returns separate traceable evidence for the requested focus."
                    .to_string(),
            ),
            (
                "https://valid.example/evidence".to_string(),
                "The third endpoint returns additional traceable evidence for the requested focus."
                    .to_string(),
            ),
        ]),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track("filter", "Filter", "Retain document evidence")]),
        serde_json::json!(["document evidence"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Retain document evidence",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_web_fetch",
    );

    let output = execute(&executor, &args).await;

    let expected_urls = [
        "https://cdn.example/avatar.png",
        "https://downloads.example/research.zip",
        "https://valid.example/evidence",
    ];
    assert_exact_calls_in_any_order(&urls.lock().unwrap(), &expected_urls);
    assert_eq!(output["research"]["metadata"]["source_count"], 3);
    assert_eq!(
        research_source_urls(&output),
        expected_urls.map(str::to_string)
    );
}

#[tokio::test]
async fn transient_fetches_receive_exactly_one_retry() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let calls = Arc::new(Mutex::new(Vec::new()));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "First",
            "url": "https://first.example/evidence",
            "engines": ["fixture"]
        }, {
            "title": "Second",
            "url": "https://second.example/evidence",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(TransientFetchFixture {
        calls: Arc::clone(&calls),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track("retry", "Retry", "Retain retried evidence")]),
        serde_json::json!(["retried evidence"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Retain retried evidence",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_transient_fetch",
    );

    let output = execute(&executor, &args).await;

    assert_exact_calls_in_any_order(
        &calls.lock().unwrap(),
        &[
            "https://first.example/evidence",
            "https://first.example/evidence",
            "https://second.example/evidence",
            "https://second.example/evidence",
        ],
    );
    assert_eq!(output["research"]["metadata"]["source_count"], 2);
    assert_eq!(
        output["research"]["metadata"]["web"]["transport_retry_count"],
        2
    );
    assert_eq!(
        output["research"]["metadata"]["web"]["transport_retry_success_count"],
        2
    );
}

#[tokio::test]
async fn transport_like_error_text_does_not_trigger_an_untyped_retry() {
    let workspace = tempfile::tempdir().unwrap();
    let executor = ToolExecutor::new(workspace.path().to_string_lossy().to_string());
    let calls = Arc::new(AtomicUsize::new(0));
    executor.register_dynamic_tool(Arc::new(SearchFixture {
        queries: Arc::new(Mutex::new(Vec::new())),
        results: serde_json::json!([{
            "title": "Untyped failure",
            "url": "https://failure.example/evidence",
            "engines": ["fixture"]
        }]),
    }));
    executor.register_dynamic_tool(Arc::new(UntypedFetchFailureFixture {
        calls: Arc::clone(&calls),
    }));
    executor.register_dynamic_tool(Arc::new(SemanticSelectorFixture {
        preferred_fragments: Vec::new(),
        fail: false,
        invalid_selection: false,
    }));
    let plan = minimal_plan(
        serde_json::json!([track(
            "typed-retry",
            "Typed retry",
            "Retry only structured transport failures"
        )]),
        serde_json::json!(["structured retry"]),
        serde_json::json!([]),
    );
    let args = workflow_args(
        "Retry only structured transport failures",
        super::DeepResearchEvidenceScope::WebAndWorkspace,
        plan,
        "fixture_web_search",
        "fixture_untyped_fetch_failure",
    );

    let output = execute(&executor, &args).await;

    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        output["research"]["metadata"]["web"]["transport_retry_count"],
        0
    );
    assert_eq!(output["research"]["status"], "failed");
}
