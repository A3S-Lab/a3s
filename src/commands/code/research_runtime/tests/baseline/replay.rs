use super::*;

#[test]
fn recorded_f01_is_readmitted_and_rendered_without_a_model() {
    let case = load_frozen_case("F01");
    let raw = recorded_raw_output("F01");

    let (resolved, used, violations) = resolve_source_aliases(&raw, &case);

    assert!(violations.is_empty(), "{violations:#?}");
    assert_eq!(
        used,
        BTreeSet::from(["release-notes".to_string(), "status-archive".to_string()])
    );
    assert!(
        resolved.contains(
            "[1](https://releases.example.test/aurora/2.0) [2](https://status.example.test/aurora/archive)"
        ),
        "adjacent citations must be visibly separated: {resolved}"
    );
    assert!(!resolved.contains("[["), "all aliases must be resolved");
    let ledger = resolved
        .split_once("## Sources")
        .map(|(_, ledger)| ledger)
        .expect("Host-rebuilt source ledger");
    assert_eq!(ledger.matches("Aurora 2.0 Release Notes").count(), 1);
    assert_eq!(
        ledger.matches("Aurora Production Status Archive").count(),
        1
    );

    let html = crate::tui::deep_research_completed_report_html_for_test(&case.query, &resolved);
    assert!(html.contains("<html lang=\"en\">"));
    assert!(html.contains("class=\"report-shell\""));
    assert!(html.contains("class=\"report-nav\""));
    assert!(!html.contains("Evidence profile"));
    assert!(html.contains("https://releases.example.test/aurora/2.0"));
    assert!(html.contains("https://status.example.test/aurora/archive"));
}

#[test]
fn recorded_f04_is_rejected_and_replaced_without_a_model() {
    let case = load_frozen_case("F04");
    let raw = recorded_raw_output("F04");

    let (_, _, violations) = resolve_source_aliases(&raw, &case);

    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("exact H2 Sources protocol")),
        "{violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("raw source alias")),
        "{violations:#?}"
    );
    assert!(
        violations
            .iter()
            .any(|violation| violation.contains("uncited reader prose")),
        "{violations:#?}"
    );

    let directory = tempfile::tempdir().expect("create replay output directory");
    write_deterministic_fallback(directory.path(), &case).expect("write deterministic fallback");
    let markdown = std::fs::read_to_string(directory.path().join("report.md"))
        .expect("read replacement Markdown");
    let html = std::fs::read_to_string(directory.path().join("index.html"))
        .expect("read replacement HTML");

    assert!(markdown.contains("# Verifiable Research Evidence"));
    assert!(markdown.contains("Northwind SDK 3.0 supports Linux and macOS."));
    assert!(markdown.contains("https://docs.example.test/northwind/3/platforms"));
    assert!(!markdown.contains("platform-policy"));
    assert!(html.contains("class=\"report-shell\""));
    assert!(html.contains("class=\"report-nav\""));
    assert!(!html.contains("Evidence profile"));
}

fn recorded_raw_output(case_id: &str) -> String {
    std::fs::read_to_string(
        fixture_root()
            .join("recorded")
            .join(format!("{case_id}.raw.md")),
    )
    .unwrap_or_else(|error| panic!("read recorded {case_id} output: {error}"))
}
