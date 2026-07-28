use super::{QueryDiscovery, SelectedCandidate, SelectionEdge, SourceCandidate};
use crate::commands::code::research_runtime::tests::baseline::live::planning::{
    target_index, AcquisitionQuery, EvaluationStrategy, PlanningResult, PreferredSourceKind,
    SourcePreference,
};
use serde_json::Value as JsonValue;
use std::collections::BTreeSet;

/// Select candidates without interpreting query, title, path, or preview prose.
///
/// The evaluator may use only provider scores, closed strategy variants,
/// explicit typed source identities, validated graph edges, and numeric budgets.
/// Semantic relevance is measured downstream; it is never reconstructed here
/// with token overlap or topic-specific rules.
pub(super) fn select_candidates(
    planning: &PlanningResult,
    discoveries: &[QueryDiscovery],
    maximum: usize,
) -> Vec<SelectedCandidate> {
    match planning.strategy {
        EvaluationStrategy::Minimal => select_round_robin(discoveries, maximum, |_, _| 0),
        EvaluationStrategy::Brief => {
            select_round_robin(discoveries, maximum, |candidate, query| {
                query
                    .preferred_sources
                    .iter()
                    .filter_map(|preference| preference_match_score(candidate, preference))
                    .max()
                    .unwrap_or_default()
            })
        }
        EvaluationStrategy::Compiler => select_compiler(planning, discoveries, maximum),
    }
}

fn select_round_robin<F>(
    discoveries: &[QueryDiscovery],
    maximum: usize,
    preference_score: F,
) -> Vec<SelectedCandidate>
where
    F: Fn(&SourceCandidate, &AcquisitionQuery) -> usize,
{
    let ranked = discoveries
        .iter()
        .map(|discovery| {
            let mut candidates = discovery.candidates.clone();
            candidates.sort_by(|left, right| {
                preference_score(right, &discovery.query)
                    .cmp(&preference_score(left, &discovery.query))
                    .then_with(|| {
                        bounded_provider_score(right.provider_score)
                            .cmp(&bounded_provider_score(left.provider_score))
                    })
                    .then_with(|| left.anchor.cmp(&right.anchor))
            });
            candidates
        })
        .collect::<Vec<_>>();
    let mut selected = Vec::new();
    let mut cursors = vec![0usize; ranked.len()];
    let mut query_edges = vec![0usize; ranked.len()];
    let mut made_progress = true;
    while selected.len() < maximum && made_progress {
        made_progress = false;
        for (index, discovery) in discoveries.iter().enumerate() {
            if query_edges[index] >= discovery.query.fetch_slots {
                continue;
            }
            while let Some(candidate) = ranked[index].get(cursors[index]).cloned() {
                cursors[index] += 1;
                let edge = SelectionEdge {
                    query_id: discovery.query.id.clone(),
                    source_target_id: None,
                    match_score: preference_score(&candidate, &discovery.query)
                        .saturating_mul(1_000_000)
                        .saturating_add(bounded_provider_score(candidate.provider_score)),
                };
                match merge_or_push(&mut selected, candidate, edge, maximum) {
                    MergeOutcome::CandidateAdded => {
                        query_edges[index] += 1;
                        made_progress = true;
                        break;
                    }
                    MergeOutcome::EdgeAdded => {
                        made_progress = true;
                    }
                    MergeOutcome::Unchanged => {}
                }
            }
            if selected.len() >= maximum {
                break;
            }
        }
    }
    selected
}

fn select_compiler(
    planning: &PlanningResult,
    discoveries: &[QueryDiscovery],
    maximum: usize,
) -> Vec<SelectedCandidate> {
    let Some(spec) = planning.spec.as_ref() else {
        return Vec::new();
    };
    let targets = target_index(spec);
    let mut selected = Vec::new();
    let mut used_edges = BTreeSet::<(String, String, String)>::new();

    for discovery in discoveries {
        let mut allocated = 0usize;
        for target_id in &discovery.query.source_target_ids {
            if allocated >= discovery.query.fetch_slots {
                break;
            }
            let Some(target) = targets.get(target_id) else {
                continue;
            };
            let Some((candidate, score)) = best_candidate(
                &discovery.candidates,
                target,
                &used_edges,
                &discovery.query.id,
                target_id,
            ) else {
                continue;
            };
            used_edges.insert((
                discovery.query.id.clone(),
                target_id.clone(),
                candidate.anchor.clone(),
            ));
            let _ = merge_or_push(
                &mut selected,
                candidate,
                SelectionEdge {
                    query_id: discovery.query.id.clone(),
                    source_target_id: Some(target_id.clone()),
                    match_score: score,
                },
                maximum,
            );
            allocated += 1;
        }

        while allocated < discovery.query.fetch_slots && selected.len() < maximum {
            let best = discovery
                .query
                .source_target_ids
                .iter()
                .filter_map(|target_id| {
                    let target = targets.get(target_id)?;
                    best_candidate(
                        &discovery.candidates,
                        target,
                        &used_edges,
                        &discovery.query.id,
                        target_id,
                    )
                    .map(|(candidate, score)| (candidate, target_id.clone(), score))
                })
                .max_by_key(|(_, _, score)| *score);
            let Some((candidate, target_id, score)) = best else {
                break;
            };
            used_edges.insert((
                discovery.query.id.clone(),
                target_id.clone(),
                candidate.anchor.clone(),
            ));
            let _ = merge_or_push(
                &mut selected,
                candidate,
                SelectionEdge {
                    query_id: discovery.query.id.clone(),
                    source_target_id: Some(target_id),
                    match_score: score,
                },
                maximum,
            );
            allocated += 1;
        }
    }
    selected
}

pub(super) fn rank_discovery_candidates(discovery: &QueryDiscovery) -> Vec<SourceCandidate> {
    let mut candidates = discovery.candidates.clone();
    candidates.sort_by(|left, right| {
        bounded_provider_score(right.provider_score)
            .cmp(&bounded_provider_score(left.provider_score))
            .then_with(|| left.anchor.cmp(&right.anchor))
    });
    candidates
}

fn preference_match_score(
    candidate: &SourceCandidate,
    preference: &SourcePreference,
) -> Option<usize> {
    let kind = match preference.kind {
        PreferredSourceKind::Repository => "repository",
        PreferredSourceKind::Domain => "domain",
        PreferredSourceKind::Url => "url",
        PreferredSourceKind::WorkspacePath => "workspace_path",
    };
    typed_identity_match(&candidate.anchor, kind, &preference.value).map(|()| 1)
}

fn best_candidate(
    candidates: &[SourceCandidate],
    target: &JsonValue,
    used_edges: &BTreeSet<(String, String, String)>,
    query_id: &str,
    target_id: &str,
) -> Option<(SourceCandidate, usize)> {
    candidates
        .iter()
        .filter(|candidate| {
            !used_edges.contains(&(
                query_id.to_string(),
                target_id.to_string(),
                candidate.anchor.clone(),
            ))
        })
        .filter_map(|candidate| {
            candidate_match_score(candidate, target).map(|score| (candidate.clone(), score))
        })
        .max_by(
            |(left_candidate, left_score), (right_candidate, right_score)| {
                left_score
                    .cmp(right_score)
                    .then_with(|| right_candidate.anchor.cmp(&left_candidate.anchor))
            },
        )
}

fn candidate_match_score(candidate: &SourceCandidate, target: &JsonValue) -> Option<usize> {
    let policy = &target["match_policy"];
    let provider_score = bounded_provider_score(candidate.provider_score);
    match policy["kind"].as_str()? {
        "named" => {
            let identity = &policy["identity"];
            typed_identity_match(
                &candidate.anchor,
                identity["kind"].as_str()?,
                identity["value"].as_str()?,
            )?;
            Some(1_000_000usize.saturating_add(provider_score))
        }
        "exploratory" => Some(provider_score),
        _ => None,
    }
}

fn typed_identity_match(anchor: &str, kind: &str, value: &str) -> Option<()> {
    match kind {
        "repository" => repository_identity_matches(anchor, value).then_some(()),
        "domain" => domain_identity_matches(anchor, value).then_some(()),
        "url" => url_identity_matches(anchor, value).then_some(()),
        "workspace_path" => workspace_identity_matches(anchor, value).then_some(()),
        _ => None,
    }
}

fn repository_identity_matches(anchor: &str, value: &str) -> bool {
    let mut identity = value.split('/');
    let Some(owner) = identity.next() else {
        return false;
    };
    let Some(repository) = identity.next() else {
        return false;
    };
    if owner.is_empty() || repository.is_empty() || identity.next().is_some() {
        return false;
    }
    let Ok(url) = reqwest::Url::parse(anchor) else {
        return false;
    };
    if !url
        .host_str()
        .is_some_and(|host| host.eq_ignore_ascii_case("github.com"))
    {
        return false;
    }
    let mut segments = url.path_segments().into_iter().flatten();
    segments.next().is_some_and(|segment| {
        segment.eq_ignore_ascii_case(owner)
            && segments
                .next()
                .is_some_and(|segment| segment.eq_ignore_ascii_case(repository))
    })
}

fn domain_identity_matches(anchor: &str, value: &str) -> bool {
    let Ok(url) = reqwest::Url::parse(anchor) else {
        return false;
    };
    let Some(host) = url.host_str() else {
        return false;
    };
    let value = value.trim().trim_end_matches('.');
    !value.is_empty()
        && (host.eq_ignore_ascii_case(value)
            || host
                .strip_suffix(value)
                .is_some_and(|prefix| prefix.ends_with('.')))
}

fn url_identity_matches(anchor: &str, value: &str) -> bool {
    let (Ok(mut anchor), Ok(mut value)) = (reqwest::Url::parse(anchor), reqwest::Url::parse(value))
    else {
        return false;
    };
    anchor.set_fragment(None);
    value.set_fragment(None);
    anchor == value
}

fn workspace_identity_matches(anchor: &str, value: &str) -> bool {
    let anchor = anchor.trim_matches('/');
    let value = value.trim_matches('/');
    !value.is_empty()
        && (anchor == value
            || anchor
                .strip_prefix(value)
                .is_some_and(|suffix| suffix.starts_with('/')))
}

fn bounded_provider_score(score: f64) -> usize {
    if !score.is_finite() || score <= 0.0 {
        return 0;
    }
    (score.min(1_000.0) * 1_000.0) as usize
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum MergeOutcome {
    CandidateAdded,
    EdgeAdded,
    Unchanged,
}

fn merge_or_push(
    selected: &mut Vec<SelectedCandidate>,
    candidate: SourceCandidate,
    edge: SelectionEdge,
    maximum: usize,
) -> MergeOutcome {
    if let Some(existing) = selected
        .iter_mut()
        .find(|selected| selected.candidate.anchor == candidate.anchor)
    {
        if !existing.edges.contains(&edge) {
            existing.edges.push(edge);
            return MergeOutcome::EdgeAdded;
        }
        return MergeOutcome::Unchanged;
    }
    if selected.len() < maximum {
        selected.push(SelectedCandidate {
            candidate,
            edges: vec![edge],
        });
        return MergeOutcome::CandidateAdded;
    }
    MergeOutcome::Unchanged
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commands::code::research_runtime::tests::baseline::live::corpus::{
        AcquisitionTransport, EvidenceScope, PlannerBudget, PlannerInput,
    };
    use crate::commands::code::research_runtime::tests::baseline::live::planning::{
        AcquisitionQuery, EvaluationStrategy, PlanningResult, PreferredSourceKind, SourcePreference,
    };

    fn planning(
        strategy: EvaluationStrategy,
        transport: AcquisitionTransport,
        preferences: Vec<SourcePreference>,
    ) -> PlanningResult {
        PlanningResult {
            strategy,
            planner_input: PlannerInput {
                schema: "test".to_string(),
                query: "query text must remain inert".to_string(),
                report_language: "en".to_string(),
                current_date: "2026-07-24".to_string(),
                display_utc_offset: "+08:00".to_string(),
                evidence_scope: match transport {
                    AcquisitionTransport::Web => EvidenceScope::Web,
                    AcquisitionTransport::Workspace => EvidenceScope::Workspace,
                },
                budget: PlannerBudget {
                    max_queries: 2,
                    max_acquired_sources: 2,
                },
            },
            prompt: String::new(),
            proposal: JsonValue::Null,
            brief: None,
            spec: None,
            plan: None,
            queries: vec![AcquisitionQuery {
                id: "q1".to_string(),
                text: "query text must remain inert".to_string(),
                transport,
                path: String::new(),
                glob: String::new(),
                dimension_ids: Vec::new(),
                source_target_ids: Vec::new(),
                preferred_sources: preferences,
                fetch_slots: 2,
            }],
            elapsed_ms: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            repair_rounds: 0,
            mode_used: "test".to_string(),
        }
    }

    fn candidate(
        anchor: &str,
        title: &str,
        preview: &str,
        provider_score: f64,
        transport: AcquisitionTransport,
    ) -> SourceCandidate {
        SourceCandidate {
            title: title.to_string(),
            anchor: anchor.to_string(),
            preview: preview.to_string(),
            provider_score,
            transport,
        }
    }

    #[test]
    fn provider_ranking_does_not_interpret_query_title_or_preview() {
        let first = planning(
            EvaluationStrategy::Minimal,
            AcquisitionTransport::Web,
            Vec::new(),
        );
        let mut second = first.clone();
        second.planner_input.query = "完全不同的请求".to_string();
        second.queries[0].text = "unrelated follow-up".to_string();
        let candidates = vec![
            candidate(
                "https://example.test/high",
                "unrelated title",
                "unrelated preview",
                2.0,
                AcquisitionTransport::Web,
            ),
            candidate(
                "https://example.test/low",
                "query text must remain inert",
                "query text must remain inert",
                1.0,
                AcquisitionTransport::Web,
            ),
        ];
        let discoveries = |planning: &PlanningResult| {
            vec![QueryDiscovery {
                query: planning.queries[0].clone(),
                candidates: candidates.clone(),
                error: None,
                elapsed_ms: 0,
            }]
        };

        let first_selected = select_candidates(&first, &discoveries(&first), 2);
        let second_selected = select_candidates(&second, &discoveries(&second), 2);
        assert_eq!(
            first_selected
                .iter()
                .map(|item| item.candidate.anchor.as_str())
                .collect::<Vec<_>>(),
            second_selected
                .iter()
                .map(|item| item.candidate.anchor.as_str())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            first_selected[0].candidate.anchor,
            "https://example.test/high"
        );
    }

    #[test]
    fn typed_preference_is_the_only_brief_ranking_override() {
        let planning = planning(
            EvaluationStrategy::Brief,
            AcquisitionTransport::Web,
            vec![SourcePreference {
                kind: PreferredSourceKind::Url,
                value: "https://example.test/selected".to_string(),
            }],
        );
        let discoveries = vec![QueryDiscovery {
            query: planning.queries[0].clone(),
            candidates: vec![
                candidate(
                    "https://example.test/provider-first",
                    "query text must remain inert",
                    "query text must remain inert",
                    100.0,
                    AcquisitionTransport::Web,
                ),
                candidate(
                    "https://example.test/selected",
                    "unrelated",
                    "unrelated",
                    0.01,
                    AcquisitionTransport::Web,
                ),
            ],
            error: None,
            elapsed_ms: 0,
        }];

        let selected = select_candidates(&planning, &discoveries, 2);
        assert_eq!(
            selected[0].candidate.anchor,
            "https://example.test/selected"
        );
    }

    #[test]
    fn compiler_named_target_uses_exact_typed_identity() {
        let mut planning = planning(
            EvaluationStrategy::Compiler,
            AcquisitionTransport::Web,
            Vec::new(),
        );
        planning.spec = Some(serde_json::json!({
            "source_targets": [{
                "id": "t1",
                "match_policy": {
                    "kind": "named",
                    "identity": { "kind": "repository", "value": "owner/project" }
                }
            }]
        }));
        planning.queries[0].source_target_ids = vec!["t1".to_string()];
        let discoveries = vec![QueryDiscovery {
            query: planning.queries[0].clone(),
            candidates: vec![
                candidate(
                    "https://example.test/owner/project",
                    "perfect matching prose",
                    "owner project",
                    100.0,
                    AcquisitionTransport::Web,
                ),
                candidate(
                    "https://github.com/owner/project",
                    "unrelated",
                    "unrelated",
                    0.01,
                    AcquisitionTransport::Web,
                ),
            ],
            error: None,
            elapsed_ms: 0,
        }];

        let selected = select_candidates(&planning, &discoveries, 2);
        assert_eq!(selected.len(), 1);
        assert_eq!(
            selected[0].candidate.anchor,
            "https://github.com/owner/project"
        );
        assert_eq!(selected[0].edges[0].source_target_id.as_deref(), Some("t1"));
    }

    #[test]
    fn exploratory_target_uses_provider_score_not_selection_goal_prose() {
        let mut planning = planning(
            EvaluationStrategy::Compiler,
            AcquisitionTransport::Web,
            Vec::new(),
        );
        planning.spec = Some(serde_json::json!({
            "source_targets": [{
                "id": "t1",
                "match_policy": {
                    "kind": "exploratory",
                    "selection_goal": "preferred words"
                }
            }]
        }));
        planning.queries[0].source_target_ids = vec!["t1".to_string()];
        let discoveries = vec![QueryDiscovery {
            query: planning.queries[0].clone(),
            candidates: vec![
                candidate(
                    "https://example.test/high",
                    "unrelated",
                    "unrelated",
                    2.0,
                    AcquisitionTransport::Web,
                ),
                candidate(
                    "https://example.test/low",
                    "preferred words",
                    "preferred words",
                    1.0,
                    AcquisitionTransport::Web,
                ),
            ],
            error: None,
            elapsed_ms: 0,
        }];

        let selected = select_candidates(&planning, &discoveries, 1);
        assert_eq!(selected[0].candidate.anchor, "https://example.test/high");
    }

    #[test]
    fn duplicate_candidate_merges_query_edges_and_backfills_budget() {
        let mut planning = planning(
            EvaluationStrategy::Minimal,
            AcquisitionTransport::Web,
            Vec::new(),
        );
        planning.queries[0].fetch_slots = 1;
        let mut second_query = planning.queries[0].clone();
        second_query.id = "q2".to_string();
        planning.queries.push(second_query.clone());
        let shared = candidate(
            "https://example.test/shared",
            "shared",
            "shared",
            2.0,
            AcquisitionTransport::Web,
        );
        let discoveries = vec![
            QueryDiscovery {
                query: planning.queries[0].clone(),
                candidates: vec![shared.clone()],
                error: None,
                elapsed_ms: 0,
            },
            QueryDiscovery {
                query: second_query,
                candidates: vec![
                    shared,
                    candidate(
                        "https://example.test/second",
                        "second",
                        "second",
                        1.0,
                        AcquisitionTransport::Web,
                    ),
                ],
                error: None,
                elapsed_ms: 0,
            },
        ];

        let selected = select_candidates(&planning, &discoveries, 2);
        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].edges.len(), 2);
        assert_eq!(selected[1].candidate.anchor, "https://example.test/second");
    }
}
