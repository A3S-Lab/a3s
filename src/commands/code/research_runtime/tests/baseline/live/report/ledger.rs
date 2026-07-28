use super::{
    super::{
        acquisition::{AcquiredSource, AcquisitionResult},
        synthesis::{
            evidence_closure, AdmittedAtomicItem, AtomicItemBody, AtomicItemKind, AtomicLedger,
        },
    },
    EvaluationStrategy, LiveCase, Path, PlanningResult, ReportResult,
};
use serde::Serialize;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::time::Instant;

const KEY_FINDING_LIMIT: usize = 3;

#[derive(Debug, Serialize)]
struct ReportDocument {
    schema: &'static str,
    language: String,
    title: String,
    evaluation_date: String,
    summary: String,
    key_findings: Vec<DocumentItem>,
    additional_findings: Vec<DocumentItem>,
    recommendations: Vec<DocumentItem>,
    boundaries: Vec<String>,
    gaps: Vec<DocumentGap>,
    cited_sources: Vec<DocumentSource>,
    reviewed_sources: Vec<DocumentSource>,
}

#[derive(Debug, Serialize)]
struct DocumentItem {
    id: String,
    kind: AtomicItemKind,
    text: String,
    citation_numbers: Vec<usize>,
    conditions: Vec<String>,
}

#[derive(Debug, Serialize)]
struct DocumentGap {
    id: String,
    text: String,
    related_source_numbers: Vec<usize>,
}

#[derive(Clone, Debug, Serialize)]
struct DocumentSource {
    number: usize,
    title: String,
    anchor: String,
    captured: String,
}

pub(super) fn generate_ledger_report(
    case: &LiveCase,
    planning: &PlanningResult,
    acquisition: &AcquisitionResult,
    ledger: &AtomicLedger,
    output_dir: &Path,
) -> Result<ReportResult, String> {
    let started = Instant::now();
    let markdown_path = output_dir.join("report.md");
    let html_path = output_dir.join("index.html");
    let document_path = output_dir.join("report-document.json");
    let document = build_document(case, planning, acquisition, ledger);
    let markdown = render_markdown(&document);
    let html = render_html(&document);
    crate::tui::deep_research_write_report_pair_for_test(
        &markdown_path,
        markdown,
        &html_path,
        html,
    )
    .map_err(|error| format!("publish atomic-ledger artifacts: {error}"))?;
    std::fs::write(
        &document_path,
        serde_json::to_vec_pretty(&document)
            .map_err(|error| format!("encode atomic report document: {error}"))?,
    )
    .map_err(|error| format!("write atomic report document: {error}"))?;

    Ok(ReportResult {
        strategy: EvaluationStrategy::Brief,
        status: "synthesized".to_string(),
        outcome: "synthesized_items".to_string(),
        markdown_path,
        html_path,
        raw_output_path: Some(document_path),
        elapsed_ms: started.elapsed().as_millis() as u64,
        generation_count: 0,
        prompt_tokens: None,
        completion_tokens: None,
        accepted_claim_count: ledger.items.len(),
        accepted_gap_count: ledger.gaps.len(),
        rejected_item_count: 0,
        source_count: acquisition.sources.len(),
        admitted_ledger: Some(ledger.clone()),
        generation_error: None,
    })
}

pub(super) fn write_no_evidence_report(
    case: &LiveCase,
    planning: &PlanningResult,
    acquisition: &AcquisitionResult,
    output_dir: &Path,
) -> Result<(), String> {
    let document = build_document(case, planning, acquisition, &AtomicLedger::default());
    crate::tui::deep_research_write_report_pair_for_test(
        &output_dir.join("report.md"),
        render_markdown(&document),
        &output_dir.join("index.html"),
        render_html(&document),
    )
    .map_err(|error| format!("publish no-evidence artifacts: {error}"))?;
    std::fs::write(
        output_dir.join("report-document.json"),
        serde_json::to_vec_pretty(&document)
            .map_err(|error| format!("encode no-evidence report document: {error}"))?,
    )
    .map_err(|error| format!("write no-evidence report document: {error}"))
}

fn build_document(
    case: &LiveCase,
    planning: &PlanningResult,
    acquisition: &AcquisitionResult,
    ledger: &AtomicLedger,
) -> ReportDocument {
    let labels = Labels::new();
    let item_index = ledger
        .items
        .iter()
        .map(|item| (item.id.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let item_source_ids = ledger
        .items
        .iter()
        .flat_map(|item| {
            evidence_closure(item, &item_index)
                .into_iter()
                .map(|reference| reference.source_id)
        })
        .collect::<BTreeSet<_>>();
    let gap_source_ids = ledger
        .gaps
        .iter()
        .flat_map(|gap| gap.related_source_ids.iter().cloned())
        .collect::<BTreeSet<_>>();
    let referenced_source_ids = item_source_ids
        .union(&gap_source_ids)
        .cloned()
        .collect::<BTreeSet<_>>();
    let source_numbers = acquisition
        .sources
        .iter()
        .filter(|source| referenced_source_ids.contains(&source.id))
        .enumerate()
        .map(|(index, source)| (source.id.as_str(), index + 1))
        .collect::<BTreeMap<_, _>>();

    let mut findings = ledger
        .items
        .iter()
        .filter(|item| item.kind() != AtomicItemKind::Recommendation)
        .map(|item| document_item(item, &item_index, &source_numbers))
        .collect::<Vec<_>>();
    let additional_findings = if findings.len() > KEY_FINDING_LIMIT {
        findings.split_off(KEY_FINDING_LIMIT)
    } else {
        Vec::new()
    };
    let recommendations = ledger
        .items
        .iter()
        .filter(|item| item.kind() == AtomicItemKind::Recommendation)
        .map(|item| document_item(item, &item_index, &source_numbers))
        .collect::<Vec<_>>();
    let gaps = ledger
        .gaps
        .iter()
        .map(|gap| DocumentGap {
            id: gap.id.clone(),
            text: gap.text.clone(),
            related_source_numbers: gap
                .related_source_ids
                .iter()
                .filter_map(|source_id| source_numbers.get(source_id.as_str()).copied())
                .collect(),
        })
        .collect();
    let all_sources = acquisition
        .sources
        .iter()
        .filter_map(|source| {
            let number = source_numbers.get(source.id.as_str()).copied()?;
            Some(document_source(
                source,
                number,
                &planning.planner_input.display_utc_offset,
            ))
        })
        .collect::<Vec<_>>();
    let cited_sources = all_sources
        .iter()
        .filter(|source| {
            acquisition.sources.iter().any(|acquired| {
                acquired.canonical_anchor == source.anchor && item_source_ids.contains(&acquired.id)
            })
        })
        .cloned()
        .collect();
    let reviewed_sources = all_sources
        .into_iter()
        .filter(|source| {
            acquisition.sources.iter().any(|acquired| {
                acquired.canonical_anchor == source.anchor
                    && gap_source_ids.contains(&acquired.id)
                    && !item_source_ids.contains(&acquired.id)
            })
        })
        .collect();

    let mut boundaries = vec![labels.structural_boundary.to_string()];
    boundaries.push(capture_boundary(
        acquisition,
        labels,
        &planning.planner_input.display_utc_offset,
    ));
    let failed_searches = acquisition
        .discoveries
        .iter()
        .filter(|discovery| discovery.error.is_some())
        .count();
    if failed_searches > 0 {
        boundaries.push(labels.failed_searches(failed_searches));
    }
    if !acquisition.failures.is_empty() {
        boundaries.push(labels.failed_sources(acquisition.failures.len()));
    }
    if acquisition.sources.is_empty() {
        let web_searches = acquisition
            .discoveries
            .iter()
            .filter(|discovery| {
                discovery.query.transport == super::super::corpus::AcquisitionTransport::Web
            })
            .count();
        let workspace_searches = acquisition
            .discoveries
            .iter()
            .filter(|discovery| {
                discovery.query.transport == super::super::corpus::AcquisitionTransport::Workspace
            })
            .count();
        boundaries.push(labels.no_evidence_attempts(
            web_searches,
            workspace_searches,
            acquisition.source_call_count,
        ));
    }

    ReportDocument {
        schema: "a3s/deep-research-report-document/v2",
        language: "und".to_string(),
        title: case.query.clone(),
        evaluation_date: planning.planner_input.current_date.clone(),
        summary: if ledger.items.is_empty() {
            labels.no_evidence_summary.to_string()
        } else {
            labels.summary.to_string()
        },
        key_findings: findings,
        additional_findings,
        recommendations,
        boundaries,
        gaps,
        cited_sources,
        reviewed_sources,
    }
}

fn document_item(
    item: &AdmittedAtomicItem,
    item_index: &BTreeMap<&str, &AdmittedAtomicItem>,
    source_numbers: &BTreeMap<&str, usize>,
) -> DocumentItem {
    let citation_numbers = evidence_closure(item, item_index)
        .iter()
        .filter_map(|reference| source_numbers.get(reference.source_id.as_str()).copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let conditions = match &item.body {
        AtomicItemBody::Recommendation { conditions, .. } => conditions.clone(),
        AtomicItemBody::Fact { .. } | AtomicItemBody::Derivation { .. } => Vec::new(),
    };
    DocumentItem {
        id: item.id.clone(),
        kind: item.kind(),
        text: item.text.clone(),
        citation_numbers,
        conditions,
    }
}

fn document_source(
    source: &AcquiredSource,
    number: usize,
    display_utc_offset: &str,
) -> DocumentSource {
    DocumentSource {
        number,
        title: source.title.clone(),
        anchor: source.canonical_anchor.clone(),
        captured: capture_date(&source.captured_at, display_utc_offset)
            .unwrap_or_else(|| source.captured_at.clone()),
    }
}

fn render_markdown(document: &ReportDocument) -> String {
    let labels = Labels::new();
    let mut markdown = format!("# {}\n", markdown_text(&document.title));
    let _ = write!(
        markdown,
        "\n_{}: {}_\n\n## {}\n\n{}\n",
        labels.observation_date,
        markdown_text(&document.evaluation_date),
        labels.key_findings,
        markdown_text(&document.summary),
    );
    append_markdown_items(&mut markdown, &document.key_findings, labels);
    if document.key_findings.is_empty() {
        let _ = write!(markdown, "\n{}\n", labels.no_findings);
    }
    if !document.additional_findings.is_empty() {
        let _ = write!(markdown, "\n## {}\n", labels.additional_findings);
        append_markdown_items(&mut markdown, &document.additional_findings, labels);
    }
    if !document.recommendations.is_empty() {
        let _ = write!(markdown, "\n## {}\n", labels.recommendations);
        append_markdown_items(&mut markdown, &document.recommendations, labels);
    }
    let _ = write!(markdown, "\n## {}\n", labels.boundaries);
    for boundary in &document.boundaries {
        let _ = write!(markdown, "\n- {}\n", markdown_text(boundary));
    }
    for gap in &document.gaps {
        let citations = markdown_citations(&gap.related_source_numbers);
        let _ = write!(
            markdown,
            "\n- {}{}{}\n",
            labels.gap_prefix,
            markdown_text(&gap.text),
            citations,
        );
    }
    let _ = write!(markdown, "\n## {}\n", labels.sources);
    append_markdown_sources(&mut markdown, &document.cited_sources, labels);
    if !document.reviewed_sources.is_empty() {
        let _ = write!(markdown, "\n## {}\n", labels.reviewed_sources);
        append_markdown_sources(&mut markdown, &document.reviewed_sources, labels);
    }
    markdown
}

fn append_markdown_items(markdown: &mut String, items: &[DocumentItem], labels: Labels) {
    for item in items {
        let kind = labels.item_kind(item.kind);
        let citations = markdown_citations(&item.citation_numbers);
        let _ = write!(
            markdown,
            "\n- **{kind}:** {}{}\n",
            markdown_text(&item.text),
            citations,
        );
        if !item.conditions.is_empty() {
            let _ = write!(markdown, "  - **{}:** ", labels.conditions);
            let conditions = item
                .conditions
                .iter()
                .map(|condition| markdown_text(condition))
                .collect::<Vec<_>>()
                .join("; ");
            let _ = writeln!(markdown, "{conditions}");
        }
    }
}

fn markdown_citations(numbers: &[usize]) -> String {
    if numbers.is_empty() {
        return String::new();
    }
    format!(
        " {}",
        numbers
            .iter()
            .map(|number| format!("[[{number}]](#source-{number})"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn append_markdown_sources(markdown: &mut String, sources: &[DocumentSource], labels: Labels) {
    if sources.is_empty() {
        let _ = write!(markdown, "\n{}\n", labels.no_cited_sources);
        return;
    }
    for source in sources {
        let title = markdown_text(&source.title);
        let anchor = markdown_link_target(&source.anchor);
        if source.anchor.starts_with("https://") {
            let _ = write!(
                markdown,
                "\n<a id=\"source-{}\"></a>{}. [{}]({}) — {} {}\n",
                source.number,
                source.number,
                title,
                anchor,
                labels.captured,
                markdown_text(&source.captured),
            );
        } else {
            let _ = write!(
                markdown,
                "\n<a id=\"source-{}\"></a>{}. {} (`{}`) — {} {}\n",
                source.number,
                source.number,
                title,
                source.anchor.replace('`', "\\`"),
                labels.captured,
                markdown_text(&source.captured),
            );
        }
    }
}

fn render_html(document: &ReportDocument) -> String {
    let labels = Labels::new();
    let mut navigation = vec![("key-findings", labels.key_findings)];
    if !document.additional_findings.is_empty() {
        navigation.push(("additional-findings", labels.additional_findings));
    }
    if !document.recommendations.is_empty() {
        navigation.push(("recommendations", labels.recommendations));
    }
    navigation.push(("boundaries", labels.boundaries));
    navigation.push(("sources", labels.sources));
    let navigation = navigation
        .into_iter()
        .map(|(id, label)| format!("<a href=\"#{id}\">{}</a>", html_text(label)))
        .collect::<Vec<_>>()
        .join("");
    let mut sections = String::new();
    append_html_item_section(
        &mut sections,
        "key-findings",
        labels.key_findings,
        Some(&document.summary),
        &document.key_findings,
        labels,
    );
    if !document.additional_findings.is_empty() {
        append_html_item_section(
            &mut sections,
            "additional-findings",
            labels.additional_findings,
            None,
            &document.additional_findings,
            labels,
        );
    }
    if !document.recommendations.is_empty() {
        append_html_item_section(
            &mut sections,
            "recommendations",
            labels.recommendations,
            None,
            &document.recommendations,
            labels,
        );
    }
    let mut boundaries = String::new();
    for boundary in &document.boundaries {
        let _ = write!(boundaries, "<li>{}</li>", html_text(boundary));
    }
    for gap in &document.gaps {
        let _ = write!(
            boundaries,
            "<li><strong>{}</strong>{}{}</li>",
            html_text(labels.gap_prefix),
            html_text(&gap.text),
            html_citations(&gap.related_source_numbers),
        );
    }
    let _ = write!(
        sections,
        "<section id=\"boundaries\" class=\"panel\"><h2>{}</h2><ul class=\"boundary-list\">{boundaries}</ul></section>",
        html_text(labels.boundaries),
    );
    let cited_sources = html_sources(&document.cited_sources, labels);
    let reviewed_sources = if document.reviewed_sources.is_empty() {
        String::new()
    } else {
        format!(
            "<section id=\"reviewed-sources\" class=\"panel\"><h2>{}</h2>{}</section>",
            html_text(labels.reviewed_sources),
            html_sources(&document.reviewed_sources, labels),
        )
    };
    let _ = write!(
        sections,
        "<section id=\"sources\" class=\"panel\"><h2>{}</h2>{cited_sources}</section>{reviewed_sources}",
        html_text(labels.sources),
    );

    format!(
        "<!doctype html><html lang=\"{}\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width,initial-scale=1\"><meta name=\"color-scheme\" content=\"light dark\"><title>{}</title><style>{}</style></head><body><a class=\"skip-link\" href=\"#content\">{}</a><header class=\"hero\"><div class=\"shell\"><p class=\"eyebrow\">DeepResearch</p><h1>{}</h1><div class=\"meta\"><span>{}: {}</span><span>{}: {}</span><span>{}: {}</span></div></div></header><nav class=\"toc\" aria-label=\"{}\"><div class=\"shell\">{navigation}</div></nav><main id=\"content\" class=\"shell\">{sections}</main><footer class=\"shell\">{}</footer></body></html>",
        html_text(&document.language),
        html_text(&document.title),
        report_css(),
        html_text(labels.skip_to_content),
        html_text(&document.title),
        html_text(labels.observation_date),
        html_text(&document.evaluation_date),
        html_text(labels.finding_count),
        document.key_findings.len() + document.additional_findings.len(),
        html_text(labels.source_count),
        document.cited_sources.len() + document.reviewed_sources.len(),
        html_text(labels.navigation),
        html_text(labels.footer),
    )
}

fn append_html_item_section(
    html: &mut String,
    id: &str,
    heading: &str,
    summary: Option<&str>,
    items: &[DocumentItem],
    labels: Labels,
) {
    let _ = write!(
        html,
        "<section id=\"{}\" class=\"panel\"><h2>{}</h2>",
        html_attr(id),
        html_text(heading),
    );
    if let Some(summary) = summary {
        let _ = write!(html, "<p class=\"summary\">{}</p>", html_text(summary));
    }
    if items.is_empty() {
        let _ = write!(html, "<p>{}</p></section>", html_text(labels.no_findings));
        return;
    }
    let _ = write!(html, "<div class=\"item-grid\">");
    for item in items {
        let kind = labels.item_kind(item.kind);
        let _ = write!(
            html,
            "<article class=\"item\"><p class=\"kind\">{}</p><p>{}{}</p>",
            html_text(kind),
            html_text(&item.text),
            html_citations(&item.citation_numbers),
        );
        if !item.conditions.is_empty() {
            let conditions = item
                .conditions
                .iter()
                .map(|condition| format!("<li>{}</li>", html_text(condition)))
                .collect::<Vec<_>>()
                .join("");
            let _ = write!(
                html,
                "<div class=\"conditions\"><strong>{}</strong><ul>{conditions}</ul></div>",
                html_text(labels.conditions),
            );
        }
        let _ = write!(html, "</article>");
    }
    let _ = write!(html, "</div></section>");
}

fn html_citations(numbers: &[usize]) -> String {
    if numbers.is_empty() {
        return String::new();
    }
    format!(
        " <span class=\"citations\">{}</span>",
        numbers
            .iter()
            .map(|number| format!("<a href=\"#source-{number}\">[{number}]</a>"))
            .collect::<Vec<_>>()
            .join(" ")
    )
}

fn html_sources(sources: &[DocumentSource], labels: Labels) -> String {
    if sources.is_empty() {
        return format!("<p>{}</p>", html_text(labels.no_cited_sources));
    }
    let items = sources
        .iter()
        .map(|source| {
            let title = html_text(&source.title);
            let destination = html_attr(&source.anchor);
            let source_title = if source.anchor.starts_with("https://") {
                format!(
                    "<a href=\"{destination}\" rel=\"noreferrer noopener\">{title}</a>"
                )
            } else {
                format!("{title}<code>{destination}</code>")
            };
            format!(
                "<li id=\"source-{}\"><span class=\"source-number\">{}</span><div>{source_title}<p>{} {}</p></div></li>",
                source.number,
                source.number,
                html_text(labels.captured),
                html_text(&source.captured),
            )
        })
        .collect::<Vec<_>>()
        .join("");
    format!("<ol class=\"source-list\">{items}</ol>")
}

fn report_css() -> &'static str {
    r#"
:root{--bg:#f4f1ea;--surface:#fffdf8;--ink:#18201d;--muted:#5f6964;--line:#d8d3c7;--accent:#176b56;--accent-soft:#dcece6;--shadow:0 16px 40px rgba(24,32,29,.08);font-family:Inter,ui-sans-serif,system-ui,-apple-system,BlinkMacSystemFont,"Segoe UI",sans-serif;color:var(--ink);background:var(--bg)}
*{box-sizing:border-box}html{scroll-behavior:smooth}body{margin:0;line-height:1.65;background:radial-gradient(circle at top right,#dbe9df 0,transparent 34rem),var(--bg);overflow-wrap:anywhere}.shell{width:min(70rem,calc(100% - 2rem));margin-inline:auto}.skip-link{position:absolute;left:-9999px;top:.5rem;background:var(--ink);color:#fff;padding:.6rem .9rem;z-index:10}.skip-link:focus{left:.5rem}.hero{padding:4.5rem 0 2.5rem;border-bottom:1px solid var(--line)}.eyebrow{text-transform:uppercase;letter-spacing:.16em;font-size:.75rem;font-weight:750;color:var(--accent);margin:0 0 .75rem}.hero h1{font-family:Georgia,"Noto Serif SC",serif;font-size:clamp(2rem,6vw,4.4rem);line-height:1.08;max-width:22ch;margin:0}.meta{display:flex;flex-wrap:wrap;gap:.55rem;margin-top:1.5rem}.meta span{border:1px solid var(--line);background:rgba(255,253,248,.72);padding:.35rem .7rem;border-radius:999px;color:var(--muted);font-size:.86rem}.toc{position:sticky;top:0;z-index:5;border-bottom:1px solid var(--line);background:rgba(244,241,234,.92);backdrop-filter:blur(12px)}.toc .shell{display:flex;gap:.35rem;overflow-x:auto;padding-block:.65rem}.toc a{white-space:nowrap;color:var(--ink);text-decoration:none;padding:.35rem .65rem;border-radius:.5rem}.toc a:hover{background:var(--accent-soft)}main{padding-block:2rem 4rem}.panel{scroll-margin-top:4.5rem;background:var(--surface);border:1px solid var(--line);border-radius:1rem;padding:clamp(1.15rem,3vw,2rem);margin-bottom:1rem;box-shadow:var(--shadow)}h2{font-family:Georgia,"Noto Serif SC",serif;font-size:clamp(1.35rem,3vw,2rem);line-height:1.2;margin:0 0 1rem}.summary{font-size:1.08rem;max-width:72ch;color:var(--muted)}.item-grid{display:grid;grid-template-columns:repeat(2,minmax(0,1fr));gap:.75rem}.item{border-left:.23rem solid var(--accent);background:#f8f7f2;padding:1rem;border-radius:.25rem .75rem .75rem .25rem}.item p{margin:.3rem 0}.kind{font-size:.72rem;text-transform:uppercase;letter-spacing:.1em;font-weight:800;color:var(--accent)}.citations a{color:var(--accent);font-weight:750;text-decoration:none}.conditions{font-size:.92rem;color:var(--muted);margin-top:.7rem}.conditions ul,.boundary-list{margin:.4rem 0 0;padding-left:1.25rem}.boundary-list li+li{margin-top:.6rem}.source-list{list-style:none;padding:0;margin:0;display:grid;gap:.65rem}.source-list li{display:grid;grid-template-columns:2rem minmax(0,1fr);gap:.6rem;border-top:1px solid var(--line);padding-top:.75rem}.source-number{display:grid;place-items:center;width:1.75rem;height:1.75rem;border-radius:50%;background:var(--accent-soft);color:var(--accent);font-weight:800}.source-list a{color:var(--accent);font-weight:700}.source-list p{margin:.2rem 0 0;color:var(--muted);font-size:.85rem}code{display:block;margin-top:.25rem;white-space:normal;color:var(--muted)}footer{padding:0 0 3rem;color:var(--muted);font-size:.85rem}a:focus-visible{outline:3px solid #ef9d34;outline-offset:3px}
@media(max-width:44rem){.shell{width:min(100% - 1rem,70rem)}.hero{padding:2.75rem 0 1.75rem}.item-grid{grid-template-columns:1fr}.panel{border-radius:.75rem;padding:1rem}.toc .shell{width:100%;padding-inline:.5rem}}
@media(prefers-color-scheme:dark){:root{--bg:#111614;--surface:#18201d;--ink:#eef4f0;--muted:#abb8b1;--line:#34413b;--accent:#7bd4b4;--accent-soft:#263e35;--shadow:none}.item{background:#202925}.meta span{background:rgba(24,32,29,.78)}.toc{background:rgba(17,22,20,.92)}}
@media print{:root{--bg:#fff;--surface:#fff;--ink:#000;--muted:#333;--line:#bbb;--shadow:none}body{background:#fff;font-size:10.5pt}.hero{padding:0 0 1rem}.toc,.skip-link,footer{display:none}.shell{width:100%}.panel{break-inside:avoid;border:0;border-top:1px solid #bbb;border-radius:0;padding:1rem 0;margin:0}.item-grid{display:block}.item{break-inside:avoid;margin:.5rem 0;background:#fff;border:1px solid #bbb}.source-list a::after{content:" (" attr(href) ")";font-weight:400}}
"#
}

#[derive(Clone, Copy)]
struct Labels {
    observation_date: &'static str,
    key_findings: &'static str,
    additional_findings: &'static str,
    recommendations: &'static str,
    boundaries: &'static str,
    sources: &'static str,
    reviewed_sources: &'static str,
    conditions: &'static str,
    captured: &'static str,
    gap_prefix: &'static str,
    no_cited_sources: &'static str,
    summary: &'static str,
    no_evidence_summary: &'static str,
    no_findings: &'static str,
    structural_boundary: &'static str,
    no_capture_boundary: &'static str,
    captured_on: &'static str,
    captured_between: &'static str,
    finding_count: &'static str,
    source_count: &'static str,
    navigation: &'static str,
    skip_to_content: &'static str,
    footer: &'static str,
}

impl Labels {
    fn new() -> Self {
        Self {
            observation_date: "Observation date",
            key_findings: "Key Findings",
            additional_findings: "Additional Findings",
            recommendations: "Conditional Recommendations",
            boundaries: "Evidence Boundaries",
            sources: "Cited Sources",
            reviewed_sources: "Sources Reviewed for Open Questions",
            conditions: "Conditions",
            captured: "captured",
            gap_prefix: "Evidence gap: ",
            no_cited_sources: "No published finding cites a source.",
            summary: "The findings below use the sources acquired and preserved in this run. Each statement stands independently; aspects not discussed remain unassessed.",
            no_evidence_summary: "This bounded acquisition produced no publishable source evidence, so no research conclusion is presented.",
            no_findings: "No source-backed finding is publishable.",
            structural_boundary: "Every displayed item cites only sources preserved in this run; that does not establish coverage of every part of the request.",
            no_capture_boundary: "No source capture date is available.",
            captured_on: "Sources were captured on",
            captured_between: "Sources were captured between",
            finding_count: "Findings",
            source_count: "Sources",
            navigation: "Report navigation",
            skip_to_content: "Skip to content",
            footer: "A research report backed by preserved sources.",
        }
    }

    fn item_kind(self, kind: AtomicItemKind) -> &'static str {
        match kind {
            AtomicItemKind::Fact => "Fact",
            AtomicItemKind::Derivation => "Inference",
            AtomicItemKind::Recommendation => "Recommendation",
        }
    }

    fn failed_searches(self, count: usize) -> String {
        format!(
            "{count} search attempt(s) did not produce a usable candidate catalog; preserved sources remain available."
        )
    }

    fn failed_sources(self, count: usize) -> String {
        format!("{count} source read(s) did not succeed; other preserved sources remain available.")
    }

    fn no_evidence_attempts(
        self,
        web_searches: usize,
        workspace_searches: usize,
        source_attempts: usize,
    ) -> String {
        format!(
            "This run attempted {web_searches} web search(es), {workspace_searches} workspace search(es), and {source_attempts} source read(s); no source was successfully preserved."
        )
    }
}

fn capture_boundary(
    acquisition: &AcquisitionResult,
    labels: Labels,
    display_utc_offset: &str,
) -> String {
    let mut dates = acquisition
        .sources
        .iter()
        .filter_map(|source| capture_date(&source.captured_at, display_utc_offset))
        .collect::<BTreeSet<_>>();
    let Some(first) = dates.pop_first() else {
        return labels.no_capture_boundary.to_string();
    };
    let last = dates.pop_last().unwrap_or_else(|| first.clone());
    if first == last {
        format!("{} {}.", labels.captured_on, first)
    } else {
        format!("{} {} – {}.", labels.captured_between, first, last)
    }
}

fn capture_date(value: &str, display_utc_offset: &str) -> Option<String> {
    let captured = chrono::DateTime::parse_from_rfc3339(value).ok()?;
    let display =
        chrono::DateTime::parse_from_rfc3339(&format!("2000-01-01T00:00:00{display_utc_offset}"))
            .ok()?;
    Some(
        captured
            .with_timezone(display.offset())
            .date_naive()
            .to_string(),
    )
}

fn markdown_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let mut escaped = String::with_capacity(normalized.len());
    for character in normalized.chars() {
        if matches!(
            character,
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '#' | '|' | '~'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

fn markdown_link_target(value: &str) -> String {
    value.replace('(', "%28").replace(')', "%29")
}

fn html_text(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn html_attr(value: &str) -> String {
    html_text(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::code::research_runtime::tests::baseline::live::{
        acquisition::{AcquisitionFailure, QueryDiscovery, SelectionEdge},
        corpus::{
            AcquisitionTransport, EvaluationExpectations, EvidenceScope, PlannerBudget,
            PlannerInput,
        },
        planning::{AcquisitionQuery, BriefDimension, ResearchBrief},
        synthesis::{AdmittedGap, AtomicItemBody, DerivationMethod, EvidenceRef},
    };

    fn case() -> LiveCase {
        LiveCase {
            id: "test".to_string(),
            query: "What does the Alpha policy establish?".to_string(),
            report_language: "en".to_string(),
            evidence_scope: EvidenceScope::Web,
            expected_terminal: "report".to_string(),
            expectations: EvaluationExpectations {
                dimensions: Vec::new(),
                source_requirements: Vec::new(),
                guardrails: Vec::new(),
            },
        }
    }

    fn planning() -> PlanningResult {
        PlanningResult {
            strategy: EvaluationStrategy::Brief,
            planner_input: PlannerInput {
                schema: "test".to_string(),
                query: case().query,
                report_language: "en".to_string(),
                current_date: "2026-07-22".to_string(),
                display_utc_offset: "+08:00".to_string(),
                evidence_scope: EvidenceScope::Web,
                budget: PlannerBudget {
                    max_queries: 4,
                    max_acquired_sources: 8,
                },
            },
            prompt: String::new(),
            proposal: serde_json::json!({}),
            brief: Some(ResearchBrief {
                dimensions: vec![BriefDimension {
                    id: "request.primary".to_string(),
                    question: "What does the Alpha policy establish?".to_string(),
                    request_basis: vec!["What does the Alpha policy establish?".to_string()],
                    material: true,
                }],
                queries: Vec::new(),
                planning_gaps: Vec::new(),
                normalization_notes: Vec::new(),
            }),
            spec: None,
            plan: None,
            queries: Vec::new(),
            elapsed_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            repair_rounds: 0,
            mode_used: "test".to_string(),
        }
    }

    fn source(id: &str, title: &str, captured_at: &str) -> AcquiredSource {
        AcquiredSource {
            id: id.to_string(),
            title: title.to_string(),
            requested_anchor: format!("https://example.test/{id}"),
            canonical_anchor: format!("https://example.test/{id}"),
            transport: AcquisitionTransport::Web,
            captured_at: captured_at.to_string(),
            provenance: vec![SelectionEdge {
                query_id: "query.bootstrap".to_string(),
                source_target_id: None,
                match_score: 1,
            }],
            chunks: vec![serde_json::json!({
                "id": format!("{id}:chunk-1"),
                "text": "Alpha 2.x receives fixes through 2027."
            })],
            fetch_completed_ms: 1,
            persisted_ms: Some(2),
        }
    }

    fn acquisition() -> AcquisitionResult {
        AcquisitionResult {
            strategy: EvaluationStrategy::Brief,
            discoveries: Vec::new(),
            selected_candidates: Vec::new(),
            sources: vec![
                source("source-1", "Alpha policy", "2026-07-21T16:30:00Z"),
                source("source-2", "Uncited source", "2026-07-22T00:00:00Z"),
                source("source-3", "Gap source", "2026-07-22T00:00:00Z"),
            ],
            failures: vec![AcquisitionFailure {
                anchor: "https://example.test/failed".to_string(),
                edges: Vec::new(),
                reason: "internal transport detail".to_string(),
                failed_ms: 3,
            }],
            compiler_catalog: None,
            query_call_count: 1,
            source_call_count: 4,
            discovery_elapsed_ms: 1,
            source_elapsed_ms: 2,
            phase_elapsed_ms: 3,
            first_source_fetched_ms: Some(1),
            first_source_persisted_ms: Some(2),
        }
    }

    fn ledger() -> AtomicLedger {
        AtomicLedger {
            items: vec![
                AdmittedAtomicItem {
                    id: "item-1".to_string(),
                    text: "Alpha 2.x receives fixes through 2027.".to_string(),
                    body: AtomicItemBody::Fact {
                        direct_evidence: EvidenceRef {
                            source_id: "source-1".to_string(),
                            chunk_ids: vec!["source-1:chunk-1".to_string()],
                        },
                    },
                },
                AdmittedAtomicItem {
                    id: "item-2".to_string(),
                    text: "The policy is time-bounded.".to_string(),
                    body: AtomicItemBody::Derivation {
                        premise_item_ids: vec!["item-1".to_string()],
                        method: DerivationMethod::TemporalQualification,
                    },
                },
            ],
            gaps: vec![AdmittedGap {
                id: "gap-1".to_string(),
                text: "The closed sources do not establish Alpha 3.x support.".to_string(),
                related_source_ids: vec!["source-3".to_string()],
            }],
        }
    }

    #[test]
    fn report_projects_atomic_items_without_semantic_completion_status() {
        let document = build_document(&case(), &planning(), &acquisition(), &ledger());
        let markdown = render_markdown(&document);
        let html = render_html(&document);
        let json = serde_json::to_string(&document).expect("document JSON");

        for content in [&markdown, &html] {
            assert!(content.contains("Alpha 2.x receives fixes through 2027."));
            assert!(content.contains("aspects not discussed remain unassessed"));
            assert!(!content.contains("internal transport detail"));
            assert!(!content.contains("Every research obligation"));
        }
        assert!(!json.contains("supported"));
        assert!(!json.contains("complete"));
        assert!(!json.contains("answered"));
    }

    #[test]
    fn derived_item_uses_premise_citation_and_uncited_sources_stay_out_of_main_ledger() {
        let document = build_document(&case(), &planning(), &acquisition(), &ledger());
        let derivation = document
            .key_findings
            .iter()
            .find(|item| item.kind == AtomicItemKind::Derivation)
            .expect("derived item");

        assert_eq!(derivation.citation_numbers, [1]);
        assert_eq!(document.cited_sources.len(), 1);
        assert_eq!(document.cited_sources[0].title, "Alpha policy");
        assert_eq!(document.reviewed_sources.len(), 1);
        assert_eq!(document.reviewed_sources[0].title, "Gap source");
        assert!(!serde_json::to_string(&document)
            .expect("document JSON")
            .contains("Uncited source"));
    }

    #[test]
    fn markdown_and_html_project_the_same_reader_content() {
        let document = build_document(&case(), &planning(), &acquisition(), &ledger());
        let markdown = render_markdown(&document);
        let html = render_html(&document);

        for text in [
            &document.title,
            &document.summary,
            &document.key_findings[0].text,
            &document.key_findings[1].text,
            &document.gaps[0].text,
            &document.cited_sources[0].title,
            &document.reviewed_sources[0].title,
        ] {
            assert!(markdown.contains(text), "Markdown omitted `{text}`");
            assert!(html.contains(text), "HTML omitted `{text}`");
        }
        assert!(html.starts_with("<!doctype html>"));
        assert!(html.contains("@media(max-width:44rem)"));
        assert!(html.contains("@media print"));
    }

    #[test]
    fn no_evidence_report_names_safe_attempt_boundaries_without_raw_errors() {
        let mut acquisition = acquisition();
        acquisition.sources.clear();
        acquisition.discoveries = vec![QueryDiscovery {
            query: AcquisitionQuery {
                id: "query.bootstrap".to_string(),
                text: "Alpha official policy".to_string(),
                transport: AcquisitionTransport::Web,
                path: String::new(),
                glob: String::new(),
                dimension_ids: vec!["request.primary".to_string()],
                source_target_ids: Vec::new(),
                preferred_sources: Vec::new(),
                fetch_slots: 4,
            },
            candidates: Vec::new(),
            error: Some("private provider failure".to_string()),
            elapsed_ms: 1,
        }];
        let output = tempfile::tempdir().expect("no-evidence output");

        write_no_evidence_report(&case(), &planning(), &acquisition, output.path())
            .expect("no-evidence report");
        let markdown =
            std::fs::read_to_string(output.path().join("report.md")).expect("no-evidence Markdown");
        let html =
            std::fs::read_to_string(output.path().join("index.html")).expect("no-evidence HTML");

        for content in [&markdown, &html] {
            assert!(content.contains("no publishable source evidence"));
            assert!(content.contains("1 web search(es)"));
            assert!(content.contains("4 source read(s)"));
            assert!(!content.contains("private provider failure"));
            assert!(!content.contains("internal transport detail"));
        }
    }

    #[test]
    fn capture_boundary_uses_the_frozen_display_offset() {
        assert_eq!(
            capture_date("2026-07-21T16:30:00Z", "+08:00").as_deref(),
            Some("2026-07-22")
        );
        assert_eq!(
            capture_date("2026-07-21T16:30:00Z", "-07:00").as_deref(),
            Some("2026-07-21")
        );
    }
}
