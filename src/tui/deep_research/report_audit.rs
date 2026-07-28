//! Structural source audit for a materialized research report.
//!
//! Natural-language claims are never matched back to evidence text here.
//! Closed claim/source IDs and their bindings are validated before generation;
//! this final gate verifies only exact accepted source anchors in rendered
//! citations.

use comrak::{nodes::NodeValue, parse_document, Arena, Options};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashSet};
use std::sync::OnceLock;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CitationRequirement {
    AtLeastOne,
    #[cfg(test)]
    EveryDeclared,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ReportSourceReference {
    pub(crate) source_id: String,
    pub(crate) anchor: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ReportAudit {
    pub(crate) passed: bool,
    pub(crate) accepted_sources: usize,
    pub(crate) cited_sources: usize,
    #[serde(default)]
    pub(crate) issues: Vec<ReportAuditIssue>,
    pub(crate) reason: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ReportAuditIssue {
    AcceptedSourcesEmpty,
    SourceCatalogInvalid {
        source_id: String,
    },
    SourceNotCited {
        source_id: String,
    },
    SemanticBoundaryViolation {
        target_id: String,
        category: String,
        excerpt: String,
        detail: String,
    },
}

pub(crate) fn audit_report(
    markdown: &str,
    html: &str,
    sources: &[ReportSourceReference],
    requirement: CitationRequirement,
) -> ReportAudit {
    if sources.is_empty() {
        return ReportAudit {
            passed: false,
            accepted_sources: 0,
            cited_sources: 0,
            issues: vec![ReportAuditIssue::AcceptedSourcesEmpty],
            reason: "report audit received no accepted source catalog".to_string(),
        };
    }

    let mut catalog = BTreeMap::new();
    let mut issues = Vec::new();
    for source in sources {
        let source_id = source.source_id.as_str();
        let normalized_anchor = normalize_citation_target(&source.anchor);
        let valid_id = !source_id.is_empty() && source_id.trim() == source_id;
        let duplicate = catalog.contains_key(source_id);
        if !valid_id || normalized_anchor.is_none() || duplicate {
            issues.push(ReportAuditIssue::SourceCatalogInvalid {
                source_id: source.source_id.clone(),
            });
            continue;
        }
        catalog.insert(
            source.source_id.clone(),
            normalized_anchor.expect("checked above"),
        );
    }

    let citation_targets = extract_citation_targets(markdown, html);
    let cited_source_ids = catalog
        .iter()
        .filter_map(|(source_id, anchor)| {
            citation_targets
                .contains(anchor)
                .then_some(source_id.clone())
        })
        .collect::<HashSet<_>>();

    match requirement {
        CitationRequirement::AtLeastOne if cited_source_ids.is_empty() => {
            issues.extend(
                catalog
                    .keys()
                    .cloned()
                    .map(|source_id| ReportAuditIssue::SourceNotCited { source_id }),
            );
        }
        #[cfg(test)]
        CitationRequirement::EveryDeclared => {
            issues.extend(
                catalog
                    .keys()
                    .filter(|source_id| !cited_source_ids.contains(*source_id))
                    .cloned()
                    .map(|source_id| ReportAuditIssue::SourceNotCited { source_id }),
            );
        }
        CitationRequirement::AtLeastOne => {}
    }

    let passed = issues.is_empty();
    let reason = if issues
        .iter()
        .any(|issue| matches!(issue, ReportAuditIssue::SourceCatalogInvalid { .. }))
    {
        "report source catalog is not a closed set of unique IDs and valid anchors"
    } else if issues
        .iter()
        .any(|issue| matches!(issue, ReportAuditIssue::SourceNotCited { .. }))
    {
        match requirement {
            CitationRequirement::AtLeastOne => "report cites none of the accepted evidence sources",
            #[cfg(test)]
            CitationRequirement::EveryDeclared => {
                "report does not cite every source declared by its closed evidence plan"
            }
        }
    } else {
        "report citations resolve to the exact accepted source anchors"
    };

    ReportAudit {
        passed,
        accepted_sources: sources.len(),
        cited_sources: cited_source_ids.len(),
        issues,
        reason: reason.to_string(),
    }
}

#[cfg(test)]
pub(crate) fn report_citation_targets(markdown: &str, html: &str) -> HashSet<String> {
    extract_citation_targets(markdown, html)
}

#[cfg(test)]
pub(crate) fn canonical_citation_target(target: &str) -> Option<String> {
    normalize_citation_target(target)
}

fn extract_citation_targets(markdown: &str, html: &str) -> HashSet<String> {
    let arena = Arena::new();
    let mut options = Options::default();
    options.extension.autolink = true;
    let root = parse_document(&arena, markdown, &options);
    let mut targets = HashSet::new();
    for node in root.descendants() {
        if node.ancestors().skip(1).any(|ancestor| {
            matches!(
                &ancestor.data.borrow().value,
                NodeValue::Heading(heading) if heading.level == 1
            )
        }) {
            continue;
        }
        let data = node.data.borrow();
        match &data.value {
            NodeValue::Link(link) => {
                if let Some(target) = normalize_citation_target(&link.url) {
                    targets.insert(target);
                }
            }
            NodeValue::HtmlInline(fragment) => {
                extract_html_href_targets(fragment, &mut targets);
            }
            NodeValue::HtmlBlock(block) => {
                extract_html_href_targets(&block.literal, &mut targets);
            }
            _ => {}
        }
    }
    extract_html_href_targets(html, &mut targets);
    targets
}

fn extract_html_href_targets(document: &str, targets: &mut HashSet<String>) {
    for captures in html_href_regex().captures_iter(document) {
        let Some(target) = captures
            .name("double")
            .or_else(|| captures.name("single"))
            .or_else(|| captures.name("bare"))
            .map(|capture| decode_basic_html_entities(capture.as_str()))
            .and_then(|target| normalize_citation_target(&target))
        else {
            continue;
        };
        targets.insert(target);
    }
}

fn normalize_citation_target(target: &str) -> Option<String> {
    let target = target.trim();
    let target = target
        .strip_prefix('<')
        .and_then(|target| target.strip_suffix('>'))
        .unwrap_or(target)
        .trim();
    if target.is_empty() {
        return None;
    }
    if target.starts_with('#') {
        return Some(target.to_string());
    }
    if let Some(scheme_end) = target.find("://") {
        let scheme = target[..scheme_end].to_ascii_lowercase();
        let remainder = &target[scheme_end + 3..];
        let authority_end = remainder.find(['/', '?', '#']).unwrap_or(remainder.len());
        let authority = remainder[..authority_end].to_ascii_lowercase();
        if authority.is_empty() {
            return None;
        }
        let suffix = &remainder[authority_end..];
        let suffix = if suffix.is_empty() { "/" } else { suffix };
        return Some(format!("{scheme}://{authority}{suffix}"));
    }
    Some(normalize_local_target(target))
}

fn normalize_local_target(target: &str) -> String {
    let target = target.replace('\\', "/");
    let suffix_start = target.find(['?', '#']).unwrap_or(target.len());
    let (path, suffix) = target.split_at(suffix_start);
    let absolute = path.starts_with('/');
    let mut components = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." if components.last().is_some_and(|last| *last != "..") => {
                components.pop();
            }
            ".." if !absolute => components.push(component),
            ".." => {}
            _ => components.push(component),
        }
    }
    let mut normalized = if absolute {
        format!("/{}", components.join("/"))
    } else {
        components.join("/")
    };
    if normalized.is_empty() && !absolute {
        normalized.push('.');
    }
    normalized.push_str(suffix);
    normalized
}

fn decode_basic_html_entities(target: &str) -> String {
    target
        .replace("&amp;", "&")
        .replace("&#38;", "&")
        .replace("&#x26;", "&")
        .replace("&#X26;", "&")
        .replace("&quot;", "\"")
        .replace("&#34;", "\"")
        .replace("&#x22;", "\"")
        .replace("&#X22;", "\"")
}

fn html_href_regex() -> &'static Regex {
    static REGEX: OnceLock<Regex> = OnceLock::new();
    REGEX.get_or_init(|| {
        Regex::new(
            r#"(?is)(?:^|[\s<])href\s*=\s*(?:\"(?P<double>[^\"]*)\"|'(?P<single>[^']*)'|(?P<bare>[^\s\"'=<>`]+))"#,
        )
        .expect("HTML href regex must compile")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(id: &str, anchor: &str) -> ReportSourceReference {
        ReportSourceReference {
            source_id: id.to_string(),
            anchor: anchor.to_string(),
        }
    }

    #[test]
    fn accepts_exact_declared_source_citations_without_inspecting_prose() {
        let audit = audit_report(
            "任意语言的综合结论。[来源](https://example.gov/release)",
            "",
            &[source("source:release", "https://example.gov/release")],
            CitationRequirement::EveryDeclared,
        );
        assert!(audit.passed);
        assert_eq!(audit.cited_sources, 1);
    }

    #[test]
    fn rejects_source_anchor_that_is_only_a_prefix_of_a_link_target() {
        let audit = audit_report(
            "[Source](https://example.gov/release-notes)",
            "",
            &[source("source:release", "https://example.gov/release")],
            CitationRequirement::EveryDeclared,
        );
        assert!(!audit.passed);
        assert_eq!(
            audit.issues,
            vec![ReportAuditIssue::SourceNotCited {
                source_id: "source:release".to_string(),
            }]
        );
    }

    #[test]
    fn every_declared_requires_each_exact_source_anchor() {
        let audit = audit_report(
            "[One](https://example.gov/one)",
            "",
            &[
                source("source:one", "https://example.gov/one"),
                source("source:two", "https://example.gov/two"),
            ],
            CitationRequirement::EveryDeclared,
        );
        assert!(!audit.passed);
        assert_eq!(audit.cited_sources, 1);
        assert_eq!(
            audit.issues,
            vec![ReportAuditIssue::SourceNotCited {
                source_id: "source:two".to_string(),
            }]
        );
    }

    #[test]
    fn at_least_one_accepts_one_exact_anchor_from_a_larger_catalog() {
        let audit = audit_report(
            "[One](https://example.gov/one)",
            "",
            &[
                source("source:one", "https://example.gov/one"),
                source("source:two", "https://example.gov/two"),
            ],
            CitationRequirement::AtLeastOne,
        );
        assert!(audit.passed);
        assert_eq!(audit.cited_sources, 1);
    }

    #[test]
    fn extracts_inline_autolink_and_reference_citation_targets() {
        for markdown in [
            "[Source](https://example.gov/release)",
            "<https://example.gov/release>",
            "https://example.gov/release",
            "[Source][release]\n\n[release]: https://example.gov/release",
        ] {
            let audit = audit_report(
                markdown,
                "",
                &[source("source:release", "https://example.gov/release")],
                CitationRequirement::EveryDeclared,
            );
            assert!(audit.passed, "{markdown}: {}", audit.reason);
        }
    }

    #[test]
    fn local_citation_targets_match_exact_normalized_paths() {
        let accepted = audit_report(
            "[Workspace source](docs/./research.md)",
            "",
            &[source("source:local", "./docs/research.md")],
            CitationRequirement::EveryDeclared,
        );
        assert!(accepted.passed, "{}", accepted.reason);

        let rejected = audit_report(
            "[Nested readme](docs/README.md)",
            "",
            &[source("source:local", "README.md")],
            CitationRequirement::EveryDeclared,
        );
        assert!(!rejected.passed);
    }

    #[test]
    fn link_like_text_inside_a_code_fence_is_not_a_citation() {
        let audit = audit_report(
            "```html\n<a href=\"https://example.gov/release\">not a citation</a>\n```",
            "",
            &[source("source:release", "https://example.gov/release")],
            CitationRequirement::EveryDeclared,
        );
        assert!(!audit.passed);
    }

    #[test]
    fn html_citations_require_the_exact_href_attribute_name() {
        let targets = report_citation_targets(
            "",
            "<a data-href=\"https://example.gov/metadata\" xhref=\"https://example.gov/lookalike\" href=\"https://example.gov/source\">source</a>",
        );
        assert_eq!(
            targets,
            HashSet::from(["https://example.gov/source".to_string()])
        );
    }

    #[test]
    fn same_document_fragments_remain_non_source_targets() {
        assert_eq!(
            canonical_citation_target("#section-1"),
            Some("#section-1".to_string())
        );
    }

    #[test]
    fn markdown_report_title_links_are_not_evidence_citations() {
        let targets = report_citation_targets("# Analyze https://example.gov/request\n\nBody.", "");
        assert!(!targets.contains("https://example.gov/request"));

        let body_targets = report_citation_targets(
            "# Analyze https://example.gov/request\n\nBody citation: https://example.gov/request",
            "",
        );
        assert!(body_targets.contains("https://example.gov/request"));
    }

    #[test]
    fn rejects_empty_duplicate_or_unaddressable_source_catalogs() {
        let empty = audit_report("", "", &[], CitationRequirement::AtLeastOne);
        assert_eq!(empty.issues, vec![ReportAuditIssue::AcceptedSourcesEmpty]);

        let duplicate = audit_report(
            "[Source](https://example.gov/release)",
            "",
            &[
                source("source:release", "https://example.gov/release"),
                source("source:release", "https://example.gov/other"),
            ],
            CitationRequirement::EveryDeclared,
        );
        assert!(duplicate.issues.iter().any(|issue| matches!(
            issue,
            ReportAuditIssue::SourceCatalogInvalid { source_id }
                if source_id == "source:release"
        )));
    }
}
