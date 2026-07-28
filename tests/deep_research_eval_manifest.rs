use a3s_acl::{Block, Value};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

const EXPECTED_CASES: [&str; 8] = ["F01", "F02", "F03", "F04", "F05", "F06", "F07", "F08"];
const EXPECTED_LIVE_CASES: [&str; 9] = [
    "C01", "C02", "C03", "C04", "C05", "C06", "C07", "C08", "C09",
];

#[test]
fn frozen_deep_research_corpus_is_closed_and_self_consistent() {
    let root = fixture_root();
    let manifest =
        std::fs::read_to_string(root.join("frozen.acl")).expect("read frozen DeepResearch corpus");
    let document = a3s_acl::parse_acl(&manifest).expect("parse frozen DeepResearch corpus");

    assert_eq!(document.blocks.len(), 1, "expected one corpus block");
    let corpus = &document.blocks[0];
    assert_eq!(corpus.name, "corpus");
    assert_eq!(corpus.labels, ["deep-research-frozen-v1"]);
    assert_eq!(string(corpus, "schema"), "a3s/deep-research-eval/v1");
    assert_eq!(string(corpus, "version"), "1.0.0");

    let cases = labeled_blocks(corpus, "case");
    assert_eq!(
        cases.keys().map(String::as_str).collect::<Vec<_>>(),
        EXPECTED_CASES
    );

    let mut referenced_files = BTreeSet::new();
    let mut dimension_count = 0;
    let mut source_count = 0;
    let mut claim_count = 0;
    let mut relation_count = 0;
    for (case_id, case) in cases {
        assert!(
            !string(case, "query").trim().is_empty(),
            "{case_id}: blank query"
        );
        assert!(
            matches!(string(case, "report_language"), "en" | "zh"),
            "{case_id}: unsupported report language"
        );
        assert_eq!(string(case, "expected_terminal"), "report");
        assert!(!string_list(case, "required_behaviors").is_empty());

        let dimensions = labeled_blocks(case, "dimension");
        let sources = labeled_blocks(case, "source");
        let claims = labeled_blocks(case, "claim");
        let relations = labeled_blocks(case, "relation");
        assert!(!dimensions.is_empty(), "{case_id}: no dimensions");
        assert!(!sources.is_empty(), "{case_id}: no sources");
        assert!(!claims.is_empty(), "{case_id}: no claims");

        for (dimension_id, dimension) in &dimensions {
            assert!(
                !string(dimension, "question").trim().is_empty(),
                "{case_id}/{dimension_id}: blank dimension question"
            );
            assert_eq!(
                dimension
                    .attributes
                    .get("material")
                    .and_then(Value::as_bool),
                Some(true),
                "{case_id}/{dimension_id}: frozen dimension must be material"
            );
        }
        for (source_id, source) in &sources {
            validate_source(&root, &case_id, source_id, source, &mut referenced_files);
        }
        for (claim_id, claim) in &claims {
            validate_claim(&case_id, claim_id, claim, &dimensions, &sources, &claims);
        }
        for (relation_id, relation) in &relations {
            validate_relation(&case_id, relation_id, relation, &dimensions, &claims);
        }

        dimension_count += dimensions.len();
        source_count += sources.len();
        claim_count += claims.len();
        relation_count += relations.len();
    }

    assert_eq!(dimension_count, 11);
    assert_eq!(source_count, 11);
    assert_eq!(claim_count, 22);
    assert_eq!(relation_count, 1);
    assert_eq!(referenced_files, source_files(&root));
}

#[test]
fn live_deep_research_corpus_is_versioned_hidden_and_budget_equivalent() {
    let root = fixture_root();
    let manifest =
        std::fs::read_to_string(root.join("live.acl")).expect("read live DeepResearch corpus");
    let document = a3s_acl::parse_acl(&manifest).expect("parse live DeepResearch corpus");

    assert_eq!(document.blocks.len(), 1, "expected one live corpus block");
    let corpus = &document.blocks[0];
    assert_eq!(corpus.name, "corpus");
    assert_eq!(corpus.labels, ["deep-research-live-v1"]);
    assert_eq!(string(corpus, "schema"), "a3s/deep-research-live-eval/v1");
    assert_eq!(string(corpus, "version"), "1.0.0");
    assert_eq!(number(corpus, "runs_per_case"), 3);
    assert_eq!(
        string_list(corpus, "artifact_formats"),
        ["markdown", "html"]
    );

    let budget = unique_unlabeled_block(corpus, "budget");
    let expected_budget = [
        ("planner_generations", 1),
        ("feedback_generations", 0),
        ("verifier_generations", 0),
        ("report_generations", 1),
        ("max_queries", 4),
        ("max_acquired_sources", 8),
        ("synthesis_packet_chars", 48_000),
        ("public_excerpt_chars", 12_000),
        ("wall_clock_ms", 900_000),
        ("planner_timeout_ms", 60_000),
        ("verifier_timeout_ms", 60_000),
        ("search_timeout_ms", 12_000),
        ("fetch_timeout_ms", 20_000),
        ("report_timeout_ms", 480_000),
    ];
    assert_eq!(budget.attributes.len(), expected_budget.len());
    for (field, expected) in expected_budget {
        assert_eq!(number(budget, field), expected, "live budget `{field}`");
    }

    let cases = labeled_blocks(corpus, "case");
    assert_eq!(
        cases.keys().map(String::as_str).collect::<Vec<_>>(),
        EXPECTED_LIVE_CASES
    );

    let mut dimension_count = 0;
    let mut requirement_count = 0;
    for (case_id, case) in cases {
        assert!(
            !string(case, "query").trim().is_empty(),
            "{case_id}: blank query"
        );
        assert!(
            matches!(string(case, "report_language"), "en" | "zh"),
            "{case_id}: unsupported report language"
        );
        let evidence_scope = string(case, "evidence_scope");
        assert!(
            matches!(evidence_scope, "web" | "local_only" | "web_and_workspace"),
            "{case_id}: unsupported evidence scope"
        );
        assert_eq!(string(case, "expected_terminal"), "report");
        assert!(
            !string_list(case, "guardrails").is_empty(),
            "{case_id}: no semantic guardrail"
        );
        assert!(
            case.blocks.iter().all(|block| block.name != "budget"),
            "{case_id}: per-case budget would invalidate equal-budget comparison"
        );
        for forbidden in [
            "expected_answer",
            "planner_prompt",
            "planned_queries",
            "source_targets",
        ] {
            assert!(
                !case.attributes.contains_key(forbidden),
                "{case_id}: `{forbidden}` would leak evaluator knowledge into planner input"
            );
        }

        let dimensions = labeled_blocks(case, "dimension");
        let requirements = labeled_blocks(case, "source_requirement");
        assert!(!dimensions.is_empty(), "{case_id}: no expected dimensions");
        assert!(
            !requirements.is_empty(),
            "{case_id}: no source requirements"
        );

        for (dimension_id, dimension) in &dimensions {
            assert!(
                !string(dimension, "question").trim().is_empty(),
                "{case_id}/{dimension_id}: blank dimension question"
            );
            assert_eq!(
                dimension
                    .attributes
                    .get("material")
                    .and_then(Value::as_bool),
                Some(true),
                "{case_id}/{dimension_id}: live dimensions must be material"
            );
            let acceptable = string_list(dimension, "acceptable");
            assert!(
                !acceptable.is_empty()
                    && acceptable
                        .iter()
                        .all(|outcome| matches!(outcome.as_str(), "supported" | "bounded")),
                "{case_id}/{dimension_id}: invalid acceptable outcomes {acceptable:?}"
            );
        }

        let mut referenced_dimensions = BTreeSet::new();
        let mut observed_transports = BTreeSet::new();
        for (requirement_id, requirement) in &requirements {
            assert!(
                !string(requirement, "description").trim().is_empty(),
                "{case_id}/{requirement_id}: blank source requirement"
            );
            assert!(
                matches!(
                    string(requirement, "authority"),
                    "primary" | "official" | "mixed"
                ),
                "{case_id}/{requirement_id}: invalid authority"
            );
            let transport = string(requirement, "transport");
            assert!(
                matches!(transport, "web" | "workspace"),
                "{case_id}/{requirement_id}: invalid transport"
            );
            observed_transports.insert(transport);
            let required_dimensions = string_list(requirement, "dimensions");
            assert!(
                !required_dimensions.is_empty(),
                "{case_id}/{requirement_id}: no bound dimensions"
            );
            for dimension_id in required_dimensions {
                assert!(
                    dimensions.contains_key(&dimension_id),
                    "{case_id}/{requirement_id}: unknown dimension `{dimension_id}`"
                );
                referenced_dimensions.insert(dimension_id);
            }
        }
        assert_eq!(
            referenced_dimensions,
            dimensions.keys().cloned().collect(),
            "{case_id}: every expected dimension needs a source requirement"
        );
        match evidence_scope {
            "web" => assert_eq!(observed_transports, BTreeSet::from(["web"])),
            "local_only" => {
                assert_eq!(observed_transports, BTreeSet::from(["workspace"]))
            }
            "web_and_workspace" => {
                assert_eq!(observed_transports, BTreeSet::from(["web", "workspace"]))
            }
            _ => unreachable!(),
        }

        dimension_count += dimensions.len();
        requirement_count += requirements.len();
    }

    assert_eq!(dimension_count, 53);
    assert_eq!(requirement_count, 17);
}

fn validate_source(
    root: &Path,
    case_id: &str,
    source_id: &str,
    source: &Block,
    referenced_files: &mut BTreeSet<String>,
) {
    let url = string(source, "url");
    assert!(
        url.starts_with("https://") || url.starts_with("local://"),
        "{case_id}/{source_id}: invalid source identity"
    );

    let relative = string(source, "path");
    let relative_path = Path::new(relative);
    assert!(
        !relative_path.is_absolute()
            && relative_path
                .components()
                .all(|component| matches!(component, Component::Normal(_))),
        "{case_id}/{source_id}: unsafe source path"
    );
    assert!(
        referenced_files.insert(relative.to_string()),
        "{case_id}/{source_id}: fixture reused by another source"
    );

    let bytes = std::fs::read(root.join(relative_path))
        .unwrap_or_else(|error| panic!("{case_id}/{source_id}: unreadable source: {error}"));
    assert!(
        bytes.len() >= 100,
        "{case_id}/{source_id}: empty source fixture"
    );
    assert_eq!(
        format!("{:x}", Sha256::digest(bytes)),
        string(source, "sha256"),
        "{case_id}/{source_id}: frozen source changed"
    );
}

fn validate_claim(
    case_id: &str,
    claim_id: &str,
    claim: &Block,
    dimensions: &BTreeMap<String, &Block>,
    sources: &BTreeMap<String, &Block>,
    claims: &BTreeMap<String, &Block>,
) {
    assert!(!string(claim, "statement").trim().is_empty());
    let dimension_id = string(claim, "dimension");
    assert!(
        dimensions.contains_key(dimension_id),
        "{case_id}/{claim_id}: unknown dimension `{dimension_id}`"
    );
    let kind = string(claim, "kind");
    assert!(
        matches!(kind, "fact" | "inference" | "recommendation"),
        "{case_id}/{claim_id}: unsupported claim kind `{kind}`"
    );
    assert!(
        matches!(string(claim, "placement"), "direct_answer" | "finding"),
        "{case_id}/{claim_id}: unsupported placement"
    );
    let basis = string_list(claim, "basis");
    for basis_id in &basis {
        assert!(
            claims.contains_key(basis_id),
            "{case_id}/{claim_id}: unknown basis claim `{basis_id}`"
        );
    }
    if kind == "fact" {
        assert!(basis.is_empty(), "{case_id}/{claim_id}: fact has a basis");
    }
    let disposition = string(claim, "disposition");
    assert!(
        matches!(
            disposition,
            "supported" | "supported_if_recovered" | "derived_allowed" | "forbidden"
        ),
        "{case_id}/{claim_id}: unsupported disposition"
    );
    if disposition == "forbidden" {
        assert!(!string(claim, "reason").trim().is_empty());
        return;
    }

    let evidence = string_list(claim, "evidence");
    assert!(!evidence.is_empty(), "{case_id}/{claim_id}: no evidence");
    for source_id in evidence {
        assert!(
            sources.contains_key(&source_id),
            "{case_id}/{claim_id}: unknown source `{source_id}`"
        );
    }
    if disposition == "derived_allowed" {
        assert!(
            matches!(kind, "inference" | "recommendation") && !basis.is_empty(),
            "{case_id}/{claim_id}: derived claim needs a typed basis"
        );
        assert!(!string(claim, "derivation").trim().is_empty());
    }
}

fn validate_relation(
    case_id: &str,
    relation_id: &str,
    relation: &Block,
    dimensions: &BTreeMap<String, &Block>,
    claims: &BTreeMap<String, &Block>,
) {
    assert_eq!(string(relation, "kind"), "contradicts");
    let dimension_id = string(relation, "dimension");
    assert!(
        dimensions.contains_key(dimension_id),
        "{case_id}/{relation_id}: unknown dimension `{dimension_id}`"
    );
    let claim_ids = string_list(relation, "claims");
    assert_eq!(
        claim_ids.len(),
        2,
        "{case_id}/{relation_id}: contradiction needs two claims"
    );
    for claim_id in claim_ids {
        assert!(
            claims.contains_key(&claim_id),
            "{case_id}/{relation_id}: unknown claim `{claim_id}`"
        );
    }
}

fn labeled_blocks<'a>(parent: &'a Block, name: &str) -> BTreeMap<String, &'a Block> {
    parent
        .blocks
        .iter()
        .filter(|block| block.name == name)
        .map(|block| {
            assert_eq!(block.labels.len(), 1, "{name} block needs one label");
            (block.labels[0].clone(), block)
        })
        .collect()
}

fn string<'a>(block: &'a Block, key: &str) -> &'a str {
    block
        .attributes
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_else(|| panic!("{} {:?} requires string `{key}`", block.name, block.labels))
}

fn string_list(block: &Block, key: &str) -> Vec<String> {
    match block.attributes.get(key) {
        Some(Value::List(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("{} `{key}` must contain strings", block.name))
                    .to_string()
            })
            .collect(),
        _ => panic!("{} {:?} requires list `{key}`", block.name, block.labels),
    }
}

fn number(block: &Block, key: &str) -> u64 {
    let value = block
        .attributes
        .get(key)
        .and_then(Value::as_number)
        .unwrap_or_else(|| panic!("{} {:?} requires number `{key}`", block.name, block.labels));
    assert!(
        value.is_finite() && value >= 0.0 && value.fract() == 0.0,
        "{} {:?} requires non-negative integer `{key}`",
        block.name,
        block.labels
    );
    value as u64
}

fn unique_unlabeled_block<'a>(parent: &'a Block, name: &str) -> &'a Block {
    let blocks = parent
        .blocks
        .iter()
        .filter(|block| block.name == name)
        .collect::<Vec<_>>();
    assert_eq!(blocks.len(), 1, "expected one `{name}` block");
    assert!(blocks[0].labels.is_empty(), "`{name}` must be unlabeled");
    blocks[0]
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deep_research_eval")
}

fn source_files(root: &Path) -> BTreeSet<String> {
    std::fs::read_dir(root.join("sources"))
        .expect("read source fixture directory")
        .map(|entry| entry.expect("read source fixture entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "md"))
        .map(|path| {
            path.strip_prefix(root)
                .expect("source fixture below corpus root")
                .to_string_lossy()
                .replace('\\', "/")
        })
        .collect()
}
