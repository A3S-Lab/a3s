//! Bounded structural projection for the durable DeepResearch evidence ledger.

use super::*;

pub(super) fn deep_research_collect_structured_evidence_for_ledger(
    root: &serde_json::Value,
) -> Vec<serde_json::Value> {
    fn walk(
        value: &serde_json::Value,
        round_hint: Option<u64>,
        output: &mut Vec<serde_json::Value>,
        seen: &mut HashSet<String>,
    ) {
        match value {
            serde_json::Value::Object(map) => {
                let round = map
                    .get("round")
                    .and_then(serde_json::Value::as_u64)
                    .or(round_hint);
                let has_structured_container = map.contains_key("structured");
                if let Some(structured) = map.get("structured") {
                    if output.len() < DEEP_RESEARCH_MAX_DIGEST_EVIDENCE {
                        if let Some(evidence) = compact_evidence_object(structured, round, seen) {
                            output.push(evidence);
                        }
                    }
                } else if is_deep_research_evidence_object(value) {
                    if output.len() < DEEP_RESEARCH_MAX_DIGEST_EVIDENCE {
                        if let Some(evidence) = compact_evidence_object(value, round, seen) {
                            output.push(evidence);
                        }
                    }
                    return;
                }
                for (key, child) in map {
                    if has_structured_container && key == "structured" {
                        continue;
                    }
                    if matches!(
                        key.as_str(),
                        "query"
                            | "input"
                            | "history"
                            | "prompt"
                            | "description"
                            | "error"
                            | "output_summary"
                            | "error_summary"
                    ) {
                        continue;
                    }
                    walk(child, round, output, seen);
                }
            }
            serde_json::Value::Array(items) => {
                for item in items {
                    walk(item, round_hint, output, seen);
                }
            }
            _ => {}
        }
    }

    let mut output = Vec::new();
    let mut seen = HashSet::new();
    walk(root, None, &mut output, &mut seen);
    output
}

fn is_deep_research_evidence_object(value: &serde_json::Value) -> bool {
    value
        .get("summary")
        .and_then(serde_json::Value::as_str)
        .is_some()
        && value
            .get("sources")
            .and_then(serde_json::Value::as_array)
            .is_some()
        && value
            .get("confidence")
            .and_then(serde_json::Value::as_str)
            .is_some()
}

fn compact_evidence_object(
    evidence: &serde_json::Value,
    round: Option<u64>,
    seen: &mut HashSet<String>,
) -> Option<serde_json::Value> {
    let summary = evidence.get("summary")?.as_str()?.trim();
    if summary.is_empty() {
        return None;
    }
    let first_source = evidence
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .and_then(|sources| {
            sources
                .iter()
                .find_map(deep_research_traceable_source_anchor)
        })
        .unwrap_or_default();
    let identity = format!("{}|{summary}|{first_source}", round.unwrap_or_default());
    if !seen.insert(identity) {
        return None;
    }

    let mut compact = serde_json::Map::new();
    if let Some(round) = round {
        compact.insert(
            "round".to_string(),
            serde_json::Value::Number(serde_json::Number::from(round)),
        );
    }
    compact.insert(
        "summary".to_string(),
        serde_json::Value::String(deep_research_digest_text(summary, 700)),
    );
    let sources = evidence
        .get("sources")
        .and_then(serde_json::Value::as_array)
        .map(|sources| {
            sources
                .iter()
                .filter_map(compact_source)
                .take(DEEP_RESEARCH_MAX_DIGEST_SOURCES)
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    compact.insert("sources".to_string(), serde_json::Value::Array(sources));
    for key in ["key_evidence", "contradictions", "gaps"] {
        compact.insert(
            key.to_string(),
            serde_json::Value::Array(compact_string_array(
                evidence.get(key),
                DEEP_RESEARCH_MAX_DIGEST_STRINGS,
                if key == "key_evidence" { 2_000 } else { 350 },
            )),
        );
    }
    if let Some(source_coverage) = evidence
        .get("source_coverage")
        .and_then(compact_source_coverage)
    {
        compact.insert("source_coverage".to_string(), source_coverage);
    }
    if evidence.get("relevant_obligation_ids").is_some() {
        compact.insert(
            "relevant_obligation_ids".to_string(),
            serde_json::Value::Array(compact_string_array(
                evidence.get("relevant_obligation_ids"),
                16,
                160,
            )),
        );
    }
    if let Some(confidence) = evidence
        .get("confidence")
        .and_then(serde_json::Value::as_str)
    {
        compact.insert(
            "confidence".to_string(),
            serde_json::Value::String(deep_research_digest_text(confidence, 350)),
        );
    }
    Some(serde_json::Value::Object(compact))
}

fn compact_source(source: &serde_json::Value) -> Option<serde_json::Value> {
    let anchor = deep_research_traceable_source_anchor(source)?;
    let mut compact = serde_json::Map::new();
    if let Some(source_id) = source.get("source_id").and_then(serde_json::Value::as_str) {
        compact.insert(
            "source_id".to_string(),
            serde_json::Value::String(source_id.chars().take(200).collect()),
        );
    }
    for (key, aliases, limit) in [
        ("title", &["title"][..], 220usize),
        (
            "date",
            &["date", "publication_date", "published_at"][..],
            120,
        ),
        (
            "quote_or_fact",
            &["quote_or_fact", "evidence", "quote", "fact"][..],
            2_000,
        ),
        ("reliability", &["reliability"][..], 220),
    ] {
        if let Some(value) = first_string_field(source, aliases) {
            compact.insert(
                key.to_string(),
                serde_json::Value::String(deep_research_digest_text(value, limit)),
            );
        }
    }
    let excerpts = compact_source_evidence_excerpts(source);
    if !excerpts.is_empty() {
        compact.insert(
            "evidence_excerpts".to_string(),
            serde_json::Value::Array(excerpts),
        );
    }
    compact.insert(
        "url_or_path".to_string(),
        serde_json::Value::String(deep_research_digest_text(&anchor, 500)),
    );
    Some(serde_json::Value::Object(compact))
}

fn compact_source_coverage(value: &serde_json::Value) -> Option<serde_json::Value> {
    let bindings = value.as_array()?;
    Some(serde_json::Value::Array(
        bindings
            .iter()
            .take(65)
            .map(|binding| {
                let Some(binding) = binding.as_object() else {
                    return serde_json::Value::Null;
                };
                let mut item = serde_json::Map::new();
                for key in ["source_id", "obligation_id"] {
                    item.insert(
                        key.to_string(),
                        binding
                            .get(key)
                            .and_then(serde_json::Value::as_str)
                            .map(|value| {
                                serde_json::Value::String(value.chars().take(200).collect())
                            })
                            .unwrap_or(serde_json::Value::Null),
                    );
                }
                for key in ["completion_criterion_indexes", "roles"] {
                    item.insert(
                        key.to_string(),
                        binding
                            .get(key)
                            .and_then(serde_json::Value::as_array)
                            .map(|values| {
                                serde_json::Value::Array(values.iter().take(9).cloned().collect())
                            })
                            .unwrap_or(serde_json::Value::Null),
                    );
                }
                serde_json::Value::Object(item)
            })
            .collect(),
    ))
}

fn compact_source_evidence_excerpts(source: &serde_json::Value) -> Vec<serde_json::Value> {
    let mut seen = HashSet::new();
    let candidates = source
        .get("evidence_excerpts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|excerpt| {
            let focus = first_string_field(excerpt, &["focus"])?.trim();
            let quote = first_string_field(
                excerpt,
                &["quote_or_fact", "excerpt", "evidence", "quote", "fact"],
            )?
            .trim();
            if focus.is_empty() || quote.is_empty() {
                return None;
            }
            seen.insert((focus.to_string(), quote.to_string()))
                .then_some((focus, quote))
        })
        .take(4)
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Vec::new();
    }
    let per_item_budget = (2_400 / candidates.len()).min(700);
    candidates
        .into_iter()
        .map(|(focus, quote)| {
            serde_json::json!({
                "focus": deep_research_digest_text(focus, 180),
                "quote_or_fact": deep_research_digest_text(
                    quote,
                    per_item_budget.saturating_sub(1)
                )
            })
        })
        .collect()
}

pub(super) fn deep_research_traceable_source_anchor(source: &serde_json::Value) -> Option<String> {
    let raw_anchor = first_string_field(source, &["url_or_path", "url", "path"])?;
    let safe_anchor = safe_source_anchor(raw_anchor)?;
    let has_traceable_context = ["quote_or_fact", "evidence", "quote", "fact"]
        .iter()
        .any(|key| {
            source
                .get(*key)
                .and_then(serde_json::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
        })
        || source
            .get("evidence_excerpts")
            .and_then(serde_json::Value::as_array)
            .is_some_and(|excerpts| {
                excerpts.iter().any(|excerpt| {
                    first_string_field(
                        excerpt,
                        &["quote_or_fact", "excerpt", "evidence", "quote", "fact"],
                    )
                    .is_some_and(|value| !value.trim().is_empty())
                })
            });
    has_traceable_context.then_some(safe_anchor)
}

fn safe_source_anchor(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.len() > 4_096 || trimmed.chars().any(char::is_control) {
        return None;
    }
    let safe = if let Ok(mut url) = reqwest::Url::parse(trimmed) {
        if !matches!(url.scheme(), "http" | "https") || url.host_str()?.is_empty() {
            return None;
        }
        url.set_username("").ok()?;
        url.set_password(None).ok()?;
        url.set_fragment(None);
        url.to_string()
    } else {
        let normalized = trimmed.replace('\\', "/");
        normalized
            .strip_prefix("./")
            .unwrap_or(&normalized)
            .to_string()
    };
    normalize_research_source_anchor(&safe)
}

#[cfg(test)]
pub(super) fn deep_research_safe_source_anchor(value: &str) -> Option<String> {
    safe_source_anchor(value)
}

fn first_string_field<'a>(value: &'a serde_json::Value, keys: &[&str]) -> Option<&'a str> {
    keys.iter()
        .find_map(|key| value.get(*key).and_then(serde_json::Value::as_str))
}

fn compact_string_array(
    value: Option<&serde_json::Value>,
    max_items: usize,
    max_chars: usize,
) -> Vec<serde_json::Value> {
    let mut seen = HashSet::new();
    value
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(serde_json::Value::as_str)
                .filter_map(|item| {
                    let item = item.trim();
                    (!item.is_empty() && seen.insert(item.to_string())).then(|| {
                        serde_json::Value::String(deep_research_digest_text(item, max_chars))
                    })
                })
                .take(max_items)
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn deep_research_digest_text(text: &str, limit: usize) -> String {
    let compact = text.split_whitespace().collect::<Vec<_>>().join(" ");
    deep_research_truncate_chars(&compact, limit)
}

pub(super) fn deep_research_truncate_chars(text: &str, limit: usize) -> String {
    let mut output = String::new();
    let mut truncated = false;
    for (index, character) in text.chars().enumerate() {
        if index >= limit {
            truncated = true;
            break;
        }
        output.push(character);
    }
    if truncated {
        output.push('…');
    }
    output
}
