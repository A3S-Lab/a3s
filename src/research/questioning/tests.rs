#[cfg(test)]
mod tests {
    use super::*;

    fn set(values: &[&str]) -> BTreeSet<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    fn queued() -> Vec<Question> {
        let mut first = Question::queued("question:first", None, "What is supported?");
        first.obligation_ids = vec!["obligation:primary".to_string()];
        let mut second = Question::queued("question:second", None, "What remains bounded?");
        second.obligation_ids = vec!["obligation:primary".to_string()];
        vec![first, second]
    }

    fn output() -> QuestionResolutionOutput {
        QuestionResolutionOutput {
            resolutions: vec![
                QuestionResolution::Answered {
                    question_id: "question:first".to_string(),
                    answer: "The accepted evidence supports the primary finding.".to_string(),
                    evidence_ids: vec!["evidence:a".to_string()],
                },
                QuestionResolution::Bounded {
                    question_id: "question:second".to_string(),
                    reason: "The closed packet contains no support for this claim.".to_string(),
                },
            ],
        }
    }

    #[test]
    fn schema_is_closed_over_questions_and_contains_no_iteration_surface() {
        let schema =
            question_resolution_json_schema(&queued(), &set(&["evidence:a", "evidence:b"]))
                .expect("schema");

        assert_eq!(schema["additionalProperties"], false);
        assert_eq!(schema["required"], serde_json::json!(["resolutions"]));
        assert!(schema["properties"].get("follow_up_questions").is_none());
        let resolutions = &schema["properties"]["resolutions"];
        assert_eq!(
            resolutions["required"],
            serde_json::json!(["question:first", "question:second"])
        );
        let answered = &resolutions["properties"]["question:first"];
        assert!(answered.get("oneOf").is_none());
        assert!(answered["properties"].get("question_id").is_none());
        assert_eq!(
            answered["properties"]["status"]["enum"],
            serde_json::json!(["answered", "partial", "bounded"])
        );
        assert!(answered["required"]
            .as_array()
            .is_some_and(|required| required.contains(&serde_json::json!("limitation"))));
        assert!(answered["properties"]["evidence_refs"]["items"]
            .get("enum")
            .is_none());
        assert_eq!(
            answered["properties"]["evidence_refs"]["items"]["type"],
            "string"
        );
        assert_eq!(
            answered["properties"]["evidence_refs"]["items"]["pattern"],
            "^E[1-9][0-9]*$"
        );
    }

    #[test]
    fn keyed_wire_decodes_and_unknown_iteration_fields_fail_closed() {
        let value = serde_json::json!({
            "resolutions": {
                "question:first": {
                    "status": "answered",
                    "content": "The accepted evidence supports the finding.",
                    "limitation": "",
                    "evidence_refs": ["E1"]
                },
                "question:second": {
                    "status": "bounded",
                    "content": "The packet has no support.",
                    "limitation": "",
                    "evidence_refs": []
                }
            }
        });
        let decoded = decode_question_resolution(value.clone(), &set(&["evidence:a"]))
            .expect("keyed wire");
        validate_question_resolution(&decoded, &queued(), &set(&["evidence:a"]))
            .expect("validated resolution");

        let mut iterative = value;
        iterative["follow_up_questions"] = serde_json::json!([]);
        assert!(decode_question_resolution(iterative, &set(&["evidence:a"])).is_err());

        let bounded_with_evidence = serde_json::json!({
            "resolutions": {
                "question:first": {
                    "status": "bounded",
                    "content": "The packet has no support.",
                    "limitation": "",
                    "evidence_refs": ["E1"]
                }
            }
        });
        assert!(
            decode_question_resolution(bounded_with_evidence, &set(&["evidence:a"])).is_err()
        );

        let unknown_reference = serde_json::json!({
            "resolutions": {
                "question:first": {
                    "status": "answered",
                    "content": "The accepted evidence supports the finding.",
                    "limitation": "",
                    "evidence_refs": ["E2"]
                }
            }
        });
        assert!(
            decode_question_resolution(unknown_reference, &set(&["evidence:a"])).is_err()
        );
    }

    #[test]
    fn generation_prompt_explicitly_closes_retrieval() {
        let obligations = vec![ResearchObligation::new(
            "obligation:primary",
            "Primary",
            "Resolve the primary obligation",
            true,
            vec!["A traceable answer or bounded gap".to_string()],
        )];
        let params = question_resolution_generation_params(
            "跨语言问题",
            &queued(),
            &obligations,
            &["The material obligation is resolved".to_string()],
            &set(&["evidence:a"]),
            r#"{"evidence_items":[{"evidence_id":"evidence:a","claims":[],"sources":[]}]}"#,
            30_000,
        )
        .expect("generation params");

        assert!(params.prompt.contains("only semantic review pass"));
        assert!(params.prompt.contains("completion_criterion_indexes"));
        assert!(params.prompt.contains("status=partial"));
        assert!(params
            .prompt
            .contains("Never discard supported evidence merely because"));
        assert!(params
            .prompt
            .contains("do not propose additional retrieval or new questions"));
        assert!(params.prompt.contains("evidence_ref"));
        assert!(params.prompt.contains("evidence_refs"));
        assert!(params.prompt.contains("never mention E1, E2"));
        assert!(params.prompt.contains("reader-facing prose"));
        assert!(params
            .prompt
            .contains("Write content and limitation in the query language"));
        assert!(params
            .prompt
            .contains("Do not calculate or estimate intervals"));
        assert!(params
            .prompt
            .contains("same release as its own announcement"));
        assert!(params
            .prompt
            .contains("dependency requirement does not establish incompatibility"));
        assert!(params
            .prompt
            .contains("discontinuation does not establish that no future fixes"));
        assert!(params
            .prompt
            .contains("one or a few named examples"));
        assert!(params
            .prompt
            .contains("supports only that recommendation"));
        assert!(params
            .prompt
            .contains("Source-authored praise such as great or excellent"));
        assert!(params
            .prompt
            .contains("collective all, only, every, or none claim"));
        assert!(params
            .prompt
            .contains("the whole report has no evidence"));
        assert!(params
            .prompt
            .contains("does not document it, not that compatibility is impossible"));
        assert!(params
            .prompt
            .contains("An `updated` timestamp is not a release or publication date"));
        assert!(params
            .prompt
            .contains("A short or incomplete excerpt does not establish that omitted events"));
        assert!(params
            .prompt
            .contains("never turn discontinued into no possible future fix"));
        assert!(params
            .prompt
            .contains("never rewrite that pair as only/sole/incompatible"));
        assert!(params
            .prompt
            .contains("keep each evidence gap scoped to its exact question"));
        assert!(params.prompt.contains("\"evidence_ref\":\"E1\""));
        assert!(!params.prompt.contains("evidence:a"));
        assert!(!params.prompt.contains("follow_up"));
        assert_eq!(params.schema_name, "deep_research_question_resolution");
    }

    #[test]
    fn resolution_events_are_terminal_for_every_question() {
        let events = question_resolution_events(&output(), &queued(), &set(&["evidence:a"]))
            .expect("events");

        assert_eq!(events.len(), 2);
        assert!(matches!(
            &events[0],
            InquiryEvent::QuestionAnswered { question_id, .. }
                if question_id == "question:first"
        ));
        assert!(matches!(
            &events[1],
            InquiryEvent::QuestionBounded { question_id, .. }
                if question_id == "question:second"
        ));
        assert!(!events.iter().any(|event| matches!(
            event,
            InquiryEvent::QuestionDeferred { .. } | InquiryEvent::QuestionsQueued { .. }
        )));
    }

    #[test]
    fn unknown_or_missing_evidence_ids_are_rejected() {
        let mut unknown = output();
        unknown.resolutions[0] = QuestionResolution::Answered {
            question_id: "question:first".to_string(),
            answer: "Unsupported answer".to_string(),
            evidence_ids: vec!["evidence:outside".to_string()],
        };
        assert!(validate_question_resolution(&unknown, &queued(), &set(&["evidence:a"])).is_err());

        let mut missing = output();
        missing.resolutions.pop();
        assert!(validate_question_resolution(&missing, &queued(), &set(&["evidence:a"])).is_err());
    }

    #[test]
    fn partial_resolution_preserves_evidence_and_limitation_in_one_typed_event() {
        let value = serde_json::json!({
            "resolutions": {
                "question:first": {
                    "status": "partial",
                    "content": "Five retained examples support the dominant ecosystem path.",
                    "limitation": "The packet does not establish compatibility for every named crate.",
                    "evidence_refs": ["E1"]
                }
            }
        });
        let queued = queued();
        let questions = &queued[..1];
        let decoded = decode_question_resolution(value, &set(&["evidence:a"]))
            .expect("partial keyed wire");
        let events = question_resolution_events(&decoded, questions, &set(&["evidence:a"]))
            .expect("partial resolution event");

        assert_eq!(
            events,
            vec![InquiryEvent::QuestionPartiallyAnswered {
                question_id: "question:first".to_string(),
                answer: "Five retained examples support the dominant ecosystem path.".to_string(),
                limitation: "The packet does not establish compatibility for every named crate."
                    .to_string(),
                evidence_ids: vec!["evidence:a".to_string()],
            }]
        );

        let mut missing_evidence = decoded.clone();
        let QuestionResolution::Partial { evidence_ids, .. } = &mut missing_evidence.resolutions[0]
        else {
            panic!("expected partial resolution");
        };
        evidence_ids.clear();
        assert!(
            validate_question_resolution(&missing_evidence, questions, &set(&["evidence:a"]))
                .is_err()
        );
    }

    #[test]
    fn answered_wire_with_an_explicit_limitation_is_safely_demoted_to_partial() {
        let value = serde_json::json!({
            "resolutions": {
                "question:first": {
                    "status": "answered",
                    "content": "The official database documentation supports both runtimes.",
                    "limitation": "The closed evidence does not cover every named database library.",
                    "evidence_refs": ["E1"]
                }
            }
        });

        let decoded = decode_question_resolution(value, &set(&["evidence:a"]))
            .expect("safe partial downgrade");

        assert_eq!(
            decoded.resolutions,
            vec![QuestionResolution::Partial {
                question_id: "question:first".to_string(),
                answer: "The official database documentation supports both runtimes."
                    .to_string(),
                limitation:
                    "The closed evidence does not cover every named database library."
                        .to_string(),
                evidence_ids: vec!["evidence:a".to_string()],
            }]
        );
    }
}
