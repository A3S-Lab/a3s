use super::{
    FixtureAuthority, FixtureClaim, FixtureClaimDisposition, FixtureClaimKind,
    FixtureClaimPlacement, FixtureDimension, FixtureFault, FixtureLanguage, FixtureRelation,
    FixtureRelationKind, FixtureSource, ProductReplay,
};
use a3s_acl::{Block, Value as AclValue};
use std::path::{Path, PathBuf};

pub(super) fn load_product_replays() -> Vec<ProductReplay> {
    let root = fixture_root();
    let manifest =
        std::fs::read_to_string(root.join("frozen.acl")).expect("read frozen product corpus");
    let document = a3s_acl::parse_acl(&manifest).expect("parse frozen product corpus");
    document.blocks[0]
        .blocks
        .iter()
        .filter(|block| block.name == "case")
        .map(|case| parse_replay(&root, case))
        .collect()
}

fn parse_replay(root: &Path, case: &Block) -> ProductReplay {
    let id = label(case);
    let language = match string(case, "report_language") {
        "en" => FixtureLanguage::English,
        "zh" => FixtureLanguage::Chinese,
        value => panic!("{id}: unknown fixture language `{value}`"),
    };
    let dimensions = children(case, "dimension")
        .map(|dimension| FixtureDimension {
            id: label(dimension),
            question: string(dimension, "question").to_string(),
            material: boolean(dimension, "material"),
        })
        .collect();
    let sources = children(case, "source")
        .map(|source| {
            let source_id = label(source);
            let path = string(source, "path").to_string();
            let authority = match string(source, "authority") {
                "primary" => FixtureAuthority::Primary,
                "local_primary" => FixtureAuthority::LocalPrimary,
                value => panic!("{id}/{source_id}: unknown authority `{value}`"),
            };
            FixtureSource {
                id: source_id,
                title: string(source, "title").to_string(),
                url: string(source, "url").to_string(),
                content: std::fs::read_to_string(root.join(&path))
                    .expect("read frozen product source"),
                path,
                authority,
            }
        })
        .collect();
    let claims = children(case, "claim")
        .map(|claim| {
            let claim_id = label(claim);
            let disposition = match string(claim, "disposition") {
                "supported" => FixtureClaimDisposition::Supported,
                "supported_if_recovered" => FixtureClaimDisposition::SupportedIfRecovered,
                "derived_allowed" => FixtureClaimDisposition::DerivedAllowed,
                "forbidden" => FixtureClaimDisposition::Forbidden,
                value => panic!("{id}/{claim_id}: unknown disposition `{value}`"),
            };
            let kind = match string(claim, "kind") {
                "fact" => FixtureClaimKind::Fact,
                "inference" => FixtureClaimKind::Inference,
                "recommendation" => FixtureClaimKind::Recommendation,
                value => panic!("{id}/{claim_id}: unknown claim kind `{value}`"),
            };
            let placement = match string(claim, "placement") {
                "direct_answer" => FixtureClaimPlacement::DirectAnswer,
                "finding" => FixtureClaimPlacement::Finding,
                value => panic!("{id}/{claim_id}: unknown claim placement `{value}`"),
            };
            FixtureClaim {
                id: claim_id,
                disposition,
                dimension_id: string(claim, "dimension").to_string(),
                kind,
                placement,
                text: optional_string(claim, "reader_statement")
                    .unwrap_or_else(|| string(claim, "statement").to_string()),
                source_ids: optional_string_list(claim, "evidence"),
                basis_claim_ids: string_list(claim, "basis"),
                derivation: optional_string(claim, "derivation"),
            }
        })
        .collect();
    let relations = children(case, "relation")
        .map(|relation| FixtureRelation {
            id: label(relation),
            dimension_id: string(relation, "dimension").to_string(),
            kind: match string(relation, "kind") {
                "contradicts" => FixtureRelationKind::Contradicts,
                value => panic!("{id}: unknown relation kind `{value}`"),
            },
            claim_ids: string_list(relation, "claims"),
        })
        .collect();
    let fault = children(case, "fault").next().map(|fault| {
        match (string(fault, "stage"), string(fault, "mode")) {
            ("evidence_extraction", "malformed_target_result") => {
                FixtureFault::MalformedEvidenceExtraction {
                    dimension_id: optional_string(fault, "target")
                        .unwrap_or_else(|| panic!("{id}: malformed fault requires target")),
                }
            }
            ("report_generation", "timeout") => FixtureFault::ReportGenerationTimeout,
            (stage, mode) => panic!("{id}: unknown fixture fault `{stage}` / `{mode}`"),
        }
    });
    ProductReplay {
        id,
        query: string(case, "query").to_string(),
        language,
        dimensions,
        sources,
        claims,
        relations,
        fault,
    }
}

fn children<'a>(block: &'a Block, name: &'a str) -> impl Iterator<Item = &'a Block> {
    block.blocks.iter().filter(move |child| child.name == name)
}

fn label(block: &Block) -> String {
    assert_eq!(block.labels.len(), 1, "{} needs one label", block.name);
    block.labels[0].clone()
}

fn string<'a>(block: &'a Block, key: &str) -> &'a str {
    block
        .attributes
        .get(key)
        .and_then(AclValue::as_str)
        .unwrap_or_else(|| panic!("{} {:?} requires `{key}`", block.name, block.labels))
}

fn optional_string(block: &Block, key: &str) -> Option<String> {
    block
        .attributes
        .get(key)
        .and_then(AclValue::as_str)
        .map(str::to_string)
}

fn boolean(block: &Block, key: &str) -> bool {
    block
        .attributes
        .get(key)
        .and_then(AclValue::as_bool)
        .unwrap_or_else(|| panic!("{} {:?} requires bool `{key}`", block.name, block.labels))
}

fn string_list(block: &Block, key: &str) -> Vec<String> {
    match block.attributes.get(key) {
        Some(AclValue::List(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .unwrap_or_else(|| panic!("{} `{key}` requires strings", block.name))
                    .to_string()
            })
            .collect(),
        _ => panic!("{} {:?} requires list `{key}`", block.name, block.labels),
    }
}

fn optional_string_list(block: &Block, key: &str) -> Vec<String> {
    if block.attributes.contains_key(key) {
        string_list(block, key)
    } else {
        Vec::new()
    }
}

fn fixture_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/deep_research_eval")
}
