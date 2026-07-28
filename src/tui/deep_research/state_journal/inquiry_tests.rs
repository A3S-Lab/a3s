use super::super::super::deep_research_evidence_ledger::{
    AcceptedClaim, AcceptedEvidence, AcceptedSource,
};
use super::super::{DeepResearchStateJournal, ResearchSpec};
use super::*;
use a3s::research::{
    CompletionCriterionAssessment, ContractAssessmentStatus, EvidenceRef, InquiryEvent,
    InquiryLimits, InquiryState, OutlineSection, Perspective, Question, ResearchContractAssessment,
    ResearchMethod, ResearchObligation, ResearchObligationAssessment, ResearchOutline,
    SourceCoverageBinding, SourceEvidenceRole, StopConditionAssessment,
};

fn inquiry_spec() -> ResearchSpec {
    ResearchSpec {
        query: "event-sourced inquiry".to_string(),
        current_date: "2026-07-17".to_string(),
        evidence_scope: "web".to_string(),
        required_claims: vec!["claim:root".to_string()],
        total_budget_ms: 60_000,
        retrieval_stage_budget_ms: 30_000,
        question_review_stage_budget_ms: 15_000,
        finalization_reserve_ms: 9_000,
        host_pid: 0,
    }
}

fn inquiry_events() -> Vec<InquiryEvent> {
    let mut question = Question::queued(
        "question:root",
        Some("perspective:risk".to_string()),
        "What does the evidence establish?",
    );
    question.obligation_ids = vec!["obligation:root".to_string()];
    vec![
        InquiryEvent::StrategySelected {
            method: ResearchMethod::PerspectiveGuided,
        },
        InquiryEvent::ResearchObligationsCommitted {
            obligations: vec![ResearchObligation::new(
                "obligation:root",
                "Material finding",
                "Resolve the material finding",
                true,
                vec!["The finding is traceable to accepted evidence".to_string()],
            )],
            stop_conditions: vec!["The material finding is traceable".to_string()],
        },
        InquiryEvent::ScoutCompleted {
            source_ids: vec!["source:scout".to_string()],
        },
        InquiryEvent::PerspectiveBudgetSelected {
            total_retrieval_waves: 3,
        },
        InquiryEvent::PerspectivesCommitted {
            perspectives: vec![Perspective {
                id: "perspective:risk".to_string(),
                title: "Risk".to_string(),
                focus: "Test material risks".to_string(),
                source_ids: vec!["source:scout".to_string()],
            }],
        },
        InquiryEvent::QuestionsQueued {
            questions: vec![question],
        },
        InquiryEvent::EvidenceAccepted {
            evidence: EvidenceRef::new(
                "evidence:accepted",
                vec!["claim:root".to_string()],
                vec!["source:accepted".to_string()],
            )
            .with_source_coverage(vec![SourceCoverageBinding::new(
                "source:accepted",
                "obligation:root",
                vec![0],
                vec![SourceEvidenceRole::Supporting],
            )]),
        },
        InquiryEvent::QuestionAnswered {
            question_id: "question:root".to_string(),
            answer: "The evidence establishes the material finding.".to_string(),
            evidence_ids: vec!["evidence:accepted".to_string()],
        },
        InquiryEvent::ResearchContractAssessed {
            assessment: ResearchContractAssessment {
                obligations: vec![ResearchObligationAssessment {
                    obligation_id: "obligation:root".to_string(),
                    criteria: vec![CompletionCriterionAssessment {
                        criterion_index: 0,
                        status: ContractAssessmentStatus::Satisfied,
                        rationale: "The accepted evidence supports the material finding."
                            .to_string(),
                        evidence_ids: vec!["evidence:accepted".to_string()],
                    }],
                    primary_source: None,
                    independent_corroboration: None,
                }],
                stop_conditions: vec![StopConditionAssessment {
                    condition_index: 0,
                    status: ContractAssessmentStatus::Satisfied,
                    rationale: "The material finding is traceable.".to_string(),
                    evidence_ids: vec!["evidence:accepted".to_string()],
                }],
                diagnostics: Vec::new(),
            },
        },
        InquiryEvent::OutlineCommitted {
            outline: ResearchOutline {
                sections: vec![OutlineSection {
                    id: "section:findings".to_string(),
                    heading: "Findings".to_string(),
                    purpose: "Explain the accepted finding.".to_string(),
                    perspective_ids: vec!["perspective:risk".to_string()],
                    question_ids: vec!["question:root".to_string()],
                    claim_ids: vec!["claim:root".to_string()],
                    source_ids: vec!["source:accepted".to_string()],
                    composition_hint: "Lead with the finding.".to_string(),
                }],
            },
        },
        InquiryEvent::SectionDrafted {
            section_id: "section:findings".to_string(),
            content: "A source-backed section draft.".to_string(),
            citation_ids: vec!["claim:root".to_string(), "source:accepted".to_string()],
        },
        InquiryEvent::AuditCompleted {
            passed: true,
            issues: Vec::new(),
        },
    ]
}

fn accepted_evidence() -> Vec<AcceptedEvidence> {
    vec![AcceptedEvidence {
        id: "evidence:accepted".to_string(),
        summary: "The accepted evidence establishes the material finding.".to_string(),
        confidence: Some("high".to_string()),
        sources: vec![AcceptedSource {
            id: "source:accepted".to_string(),
            anchor: "https://example.com/accepted".to_string(),
            title: Some("Accepted source".to_string()),
            date: Some("2026-07-17".to_string()),
            reliability: Some("authoritative".to_string()),
            quote_or_fact: Some("The material finding is established.".to_string()),
            evidence_excerpts: Vec::new(),
        }],
        claims: vec![AcceptedClaim {
            id: "claim:root".to_string(),
            text: "The material finding is established.".to_string(),
        }],
        source_coverage: vec![SourceCoverageBinding::new(
            "source:accepted",
            "obligation:root",
            vec![0],
            vec![SourceEvidenceRole::Supporting],
        )],
        relevant_obligation_ids: vec!["obligation:root".to_string()],
        contradictions: Vec::new(),
        gaps: Vec::new(),
    }]
}

async fn recorded_inquiry(run_id: &str) -> (tempfile::TempDir, Vec<InquiryEvent>, InquiryState) {
    let temp = tempfile::tempdir().unwrap();
    DeepResearchStateJournal::create(temp.path(), run_id, inquiry_spec())
        .await
        .unwrap();
    let events = inquiry_events();
    let state = a3s::research::replay(&events, &InquiryLimits::default()).unwrap();
    let (first, concurrent) = tokio::join!(
        record_inquiry_state(temp.path(), run_id, &events, &state),
        record_inquiry_state(temp.path(), run_id, &events, &state),
    );
    first.unwrap();
    concurrent.unwrap();
    (temp, events, state)
}

fn persisted_inquiry_sequences(journal: &DeepResearchStateJournal) -> Vec<u64> {
    journal
        .runtime
        .events()
        .iter()
        .filter_map(|record| match &record.event {
            GraphEvent::ExternalEventObserved {
                source,
                stream_id,
                sequence,
                ..
            } if source == INQUIRY_EVENT_SOURCE && stream_id == &journal.run_id => Some(*sequence),
            _ => None,
        })
        .collect()
}

#[tokio::test]
async fn load_inquiry_state_restores_only_a_contiguous_strict_prefix() {
    let temp = tempfile::tempdir().unwrap();
    let run_id = "run-inquiry-load-prefix";
    assert!(load_inquiry_state(temp.path(), run_id)
        .await
        .unwrap()
        .is_none());
    DeepResearchStateJournal::create(temp.path(), run_id, inquiry_spec())
        .await
        .unwrap();
    assert!(load_inquiry_state(temp.path(), run_id)
        .await
        .unwrap()
        .is_none());

    let events = inquiry_events();
    let prefix = events[..8].to_vec();
    let state = a3s::research::replay(&prefix, &InquiryLimits::default()).unwrap();
    record_inquiry_state(temp.path(), run_id, &prefix, &state)
        .await
        .unwrap();
    let (restored_events, restored_state) = load_inquiry_state(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(restored_events, prefix);
    assert_eq!(restored_state, state);

    let journal = DeepResearchStateJournal::open(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    let mut gap = journal.runtime.events().to_vec();
    let missing = gap
        .iter()
        .position(|record| {
            matches!(
                &record.event,
                GraphEvent::ExternalEventObserved {
                    source,
                    stream_id,
                    sequence: 3,
                    ..
                } if source == INQUIRY_EVENT_SOURCE && stream_id == run_id
            )
        })
        .unwrap();
    gap.remove(missing);
    let gap_error = decode_inquiry_state(run_id, &gap).unwrap_err();
    assert!(format!("{gap_error:#}").contains("is not contiguous"));

    let mut damaged = journal.runtime.events().to_vec();
    let payload = damaged
        .iter_mut()
        .find_map(|record| match &mut record.event {
            GraphEvent::ExternalEventObserved {
                source,
                stream_id,
                sequence,
                payload,
                ..
            } if source == INQUIRY_EVENT_SOURCE && stream_id == run_id && *sequence == 2 => {
                Some(payload)
            }
            _ => None,
        })
        .unwrap();
    *payload = serde_json::json!({"damaged": true});
    let damaged_error = decode_inquiry_state(run_id, &damaged).unwrap_err();
    assert!(format!("{damaged_error:#}").contains("decode DeepResearch inquiry event"));
}

#[tokio::test]
async fn incremental_inquiry_prefix_survives_reopen_and_extends_without_duplicates() {
    let temp = tempfile::tempdir().unwrap();
    let run_id = "run-inquiry-incremental-prefix";
    DeepResearchStateJournal::create(temp.path(), run_id, inquiry_spec())
        .await
        .unwrap();
    let events = inquiry_events();
    let discovery_prefix = &events[..6];
    let discovery_state =
        a3s::research::replay(discovery_prefix, &InquiryLimits::default()).unwrap();

    record_inquiry_state(temp.path(), run_id, discovery_prefix, &discovery_state)
        .await
        .unwrap();
    let reopened = DeepResearchStateJournal::open(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        persisted_inquiry_sequences(&reopened),
        (1..=discovery_prefix.len() as u64).collect::<Vec<_>>()
    );
    assert!(reopened
        .runtime
        .graph()
        .object(&object_id(run_id, "question", "question:root"))
        .is_some());
    assert!(reopened
        .runtime
        .graph()
        .object(&object_id(run_id, "outline-section", "section:findings"))
        .is_none());
    let checkpoint_path =
        checkpoint_path(&temp.path().join(".a3s/research/runs/checkpoints"), run_id);
    let checkpoint: ResearchCheckpoint =
        serde_json::from_slice(&tokio::fs::read(checkpoint_path).await.unwrap()).unwrap();
    assert_eq!(
        checkpoint.event_head.as_deref(),
        graph_event_head(reopened.runtime.events())
    );
    GraphRuntime::strict_replay(reopened.runtime.events()).unwrap();
    let discovery_event_count = reopened.runtime.events().len();
    drop(reopened);

    record_inquiry_state(temp.path(), run_id, discovery_prefix, &discovery_state)
        .await
        .unwrap();
    let idempotent = DeepResearchStateJournal::open(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(idempotent.runtime.events().len(), discovery_event_count);
    drop(idempotent);

    let completed_state = a3s::research::replay(&events, &InquiryLimits::default()).unwrap();
    record_inquiry_state(temp.path(), run_id, &events, &completed_state)
        .await
        .unwrap();
    let completed = DeepResearchStateJournal::open(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        persisted_inquiry_sequences(&completed),
        (1..=events.len() as u64).collect::<Vec<_>>()
    );
    GraphRuntime::strict_replay(completed.runtime.events()).unwrap();
}

#[tokio::test]
async fn incremental_inquiry_rejects_stale_or_divergent_prefixes() {
    let temp = tempfile::tempdir().unwrap();
    let run_id = "run-inquiry-prefix-conflict";
    DeepResearchStateJournal::create(temp.path(), run_id, inquiry_spec())
        .await
        .unwrap();
    let events = inquiry_events();
    let prefix = &events[..6];
    let prefix_state = a3s::research::replay(prefix, &InquiryLimits::default()).unwrap();
    record_inquiry_state(temp.path(), run_id, prefix, &prefix_state)
        .await
        .unwrap();

    let stale = &events[..2];
    let stale_state = a3s::research::replay(stale, &InquiryLimits::default()).unwrap();
    let stale_error = record_inquiry_state(temp.path(), run_id, stale, &stale_state)
        .await
        .unwrap_err();
    assert!(format!("{stale_error:#}").contains("is stale"));

    let mut divergent = prefix.to_vec();
    let InquiryEvent::ResearchObligationsCommitted {
        stop_conditions, ..
    } = &mut divergent[1]
    else {
        panic!("fixture event 1 must commit the research contract");
    };
    stop_conditions[0] = "A different material stopping condition".to_string();
    let divergent_state = a3s::research::replay(&divergent, &InquiryLimits::default()).unwrap();
    let conflict = record_inquiry_state(temp.path(), run_id, &divergent, &divergent_state)
        .await
        .unwrap_err();
    assert!(format!("{conflict:#}").contains("conflicts with the observed event id"));

    let journal = DeepResearchStateJournal::open(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        persisted_inquiry_sequences(&journal),
        (1..=prefix.len() as u64).collect::<Vec<_>>()
    );
    GraphRuntime::strict_replay(journal.runtime.events()).unwrap();
}

#[tokio::test]
async fn inquiry_objects_replay_idempotently_with_final_state() {
    let (temp, events, state) = recorded_inquiry("run-inquiry-objects").await;
    let journal = DeepResearchStateJournal::open(temp.path(), "run-inquiry-objects")
        .await
        .unwrap()
        .unwrap();
    for (object_type, expected) in [
        (PERSPECTIVE_OBJECT_TYPE, 1),
        (QUESTION_OBJECT_TYPE, 1),
        (OBLIGATION_OBJECT_TYPE, 1),
        (STOP_CONDITION_OBJECT_TYPE, 1),
        (OUTLINE_SECTION_OBJECT_TYPE, 1),
        (SECTION_DRAFT_OBJECT_TYPE, 1),
    ] {
        assert_eq!(
            journal
                .runtime
                .graph()
                .objects()
                .filter(|object| object.object_type == object_type)
                .count(),
            expected
        );
    }
    let question = journal
        .runtime
        .graph()
        .object(&object_id(
            "run-inquiry-objects",
            "question",
            "question:root",
        ))
        .unwrap();
    assert_eq!(
        serde_json::from_value::<Question>(question.data.clone()).unwrap(),
        state.questions[0]
    );
    let event_count = journal.runtime.events().len();
    drop(journal);
    record_inquiry_state(temp.path(), "run-inquiry-objects", &events, &state)
        .await
        .unwrap();
    let reopened = DeepResearchStateJournal::open(temp.path(), "run-inquiry-objects")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(reopened.runtime.events().len(), event_count);
}

#[tokio::test]
async fn inquiry_relations_replay_with_stable_direction_and_run_scope() {
    let (temp, _, _) = recorded_inquiry("run-inquiry-relations").await;
    let journal = DeepResearchStateJournal::open(temp.path(), "run-inquiry-relations")
        .await
        .unwrap()
        .unwrap();
    let run_id = "run-inquiry-relations";
    let perspective = object_id(run_id, "perspective", "perspective:risk");
    let question = object_id(run_id, "question", "question:root");
    let obligation = object_id(run_id, "obligation", "obligation:root");
    let section = object_id(run_id, "outline-section", "section:findings");
    let draft = object_id(run_id, "section-draft", "section:findings");
    let relations = journal.runtime.graph().relations().collect::<Vec<_>>();
    for (relation_type, source, target) in [
        ("deep_research.frames_question", &perspective, &question),
        ("deep_research.addresses_obligation", &question, &obligation),
        ("deep_research.covers_obligation", &section, &obligation),
        ("deep_research.covers_perspective", &section, &perspective),
        ("deep_research.covers_question", &section, &question),
        ("deep_research.has_section_draft", &section, &draft),
    ] {
        assert!(relations.iter().any(|relation| {
            relation.relation_type == relation_type
                && &relation.source == source
                && &relation.target == target
        }));
        assert!(!relations.iter().any(|relation| {
            relation.relation_type == relation_type
                && &relation.source == target
                && &relation.target == source
        }));
    }
    assert_ne!(
        perspective,
        object_id("another-run", "perspective", "perspective:risk")
    );
}

#[tokio::test]
async fn evidence_recorded_after_inquiry_connects_answers_outline_and_draft() {
    let run_id = "run-inquiry-then-evidence";
    let (temp, _, _) = recorded_inquiry(run_id).await;
    super::super::record_evidence_ledger(temp.path(), run_id, &accepted_evidence())
        .await
        .unwrap();

    assert_inquiry_evidence_relations(
        &DeepResearchStateJournal::open(temp.path(), run_id)
            .await
            .unwrap()
            .unwrap(),
        run_id,
    );
}

#[tokio::test]
async fn inquiry_recorded_after_evidence_connects_answers_outline_and_draft() {
    let temp = tempfile::tempdir().unwrap();
    let run_id = "run-evidence-then-inquiry";
    DeepResearchStateJournal::create(temp.path(), run_id, inquiry_spec())
        .await
        .unwrap();
    super::super::record_evidence_ledger(temp.path(), run_id, &accepted_evidence())
        .await
        .unwrap();
    let events = inquiry_events();
    let state = a3s::research::replay(&events, &InquiryLimits::default()).unwrap();
    record_inquiry_state(temp.path(), run_id, &events, &state)
        .await
        .unwrap();

    assert_inquiry_evidence_relations(
        &DeepResearchStateJournal::open(temp.path(), run_id)
            .await
            .unwrap()
            .unwrap(),
        run_id,
    );
}

#[tokio::test]
async fn redrafted_section_replaces_stale_claim_and_source_relations() {
    let temp = tempfile::tempdir().unwrap();
    let run_id = "run-redraft-replaces-citations";
    DeepResearchStateJournal::create(temp.path(), run_id, inquiry_spec())
        .await
        .unwrap();

    let mut evidence = accepted_evidence();
    evidence[0].claims.push(AcceptedClaim {
        id: "claim:replacement".to_string(),
        text: "The replacement finding is established.".to_string(),
    });
    evidence[0].sources.push(AcceptedSource {
        id: "source:replacement".to_string(),
        anchor: "https://example.com/replacement".to_string(),
        title: Some("Replacement source".to_string()),
        date: Some("2026-07-17".to_string()),
        reliability: Some("authoritative".to_string()),
        quote_or_fact: Some("The replacement finding is established.".to_string()),
        evidence_excerpts: Vec::new(),
    });
    super::super::record_evidence_ledger(temp.path(), run_id, &evidence)
        .await
        .unwrap();

    let mut events = inquiry_events();
    let InquiryEvent::EvidenceAccepted {
        evidence: reference,
    } = &mut events[6]
    else {
        panic!("fixture event 6 must accept evidence");
    };
    reference.claim_ids.push("claim:replacement".to_string());
    reference.source_ids.push("source:replacement".to_string());
    let InquiryEvent::OutlineCommitted { outline } = &mut events[9] else {
        panic!("fixture event 9 must commit the outline");
    };
    outline.sections[0]
        .claim_ids
        .push("claim:replacement".to_string());
    outline.sections[0]
        .source_ids
        .push("source:replacement".to_string());
    let InquiryEvent::AuditCompleted { passed, issues } = &mut events[11] else {
        panic!("fixture event 11 must complete the audit");
    };
    *passed = false;
    issues.push("replace the draft evidence".to_string());

    let limits = InquiryLimits::default();
    let initial_state = a3s::research::replay(&events, &limits).unwrap();
    record_inquiry_state(temp.path(), run_id, &events, &initial_state)
        .await
        .unwrap();

    events.push(InquiryEvent::SectionDrafted {
        section_id: "section:findings".to_string(),
        content: "A replacement source-backed section draft.".to_string(),
        citation_ids: vec![
            "claim:replacement".to_string(),
            "source:replacement".to_string(),
        ],
    });
    let redrafted_state = a3s::research::replay(&events, &limits).unwrap();
    record_inquiry_state(temp.path(), run_id, &events, &redrafted_state)
        .await
        .unwrap();

    let journal = DeepResearchStateJournal::open(temp.path(), run_id)
        .await
        .unwrap()
        .unwrap();
    let draft = object_id(run_id, "section-draft", "section:findings");
    let citation_relations = journal
        .runtime
        .graph()
        .relations()
        .filter(|relation| {
            relation.source == draft
                && matches!(
                    relation.relation_type.as_str(),
                    "deep_research.cites_claim" | "deep_research.cites_source"
                )
        })
        .map(|relation| (relation.relation_type.clone(), relation.target.clone()))
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        citation_relations,
        [
            (
                "deep_research.cites_claim".to_string(),
                "claim:replacement".to_string(),
            ),
            (
                "deep_research.cites_source".to_string(),
                "source:replacement".to_string(),
            ),
        ]
        .into_iter()
        .collect()
    );
    GraphRuntime::strict_replay(journal.runtime.events()).unwrap();
}

fn assert_inquiry_evidence_relations(journal: &DeepResearchStateJournal, run_id: &str) {
    let question = object_id(run_id, "question", "question:root");
    let section = object_id(run_id, "outline-section", "section:findings");
    let draft = object_id(run_id, "section-draft", "section:findings");
    let relations = journal.runtime.graph().relations().collect::<Vec<_>>();
    for (relation_type, source, target) in [
        (
            "deep_research.answered_by",
            question.as_str(),
            "evidence:accepted",
        ),
        ("deep_research.covers_claim", section.as_str(), "claim:root"),
        (
            "deep_research.covers_source",
            section.as_str(),
            "source:accepted",
        ),
        ("deep_research.cites_claim", draft.as_str(), "claim:root"),
        (
            "deep_research.cites_source",
            draft.as_str(),
            "source:accepted",
        ),
    ] {
        assert!(
            relations.iter().any(|relation| {
                relation.relation_type == relation_type
                    && relation.source == source
                    && relation.target == target
            }),
            "missing {relation_type}: {source} -> {target}"
        );
    }
    let observed = relations
        .iter()
        .find(|relation| {
            relation.relation_type == "deep_research.observed_in"
                && relation.source == "evidence:accepted"
                && relation.target == "source:accepted"
        })
        .expect("accepted evidence/source relation");
    assert_eq!(
        observed.data["source_coverage"],
        serde_json::json!([{
            "source_id": "source:accepted",
            "obligation_id": "obligation:root",
            "completion_criterion_indexes": [0],
            "roles": ["supporting"]
        }])
    );
    GraphRuntime::strict_replay(journal.runtime.events()).unwrap();
}
