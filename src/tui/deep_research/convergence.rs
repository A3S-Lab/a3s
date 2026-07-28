//! Terminal assessment for the bounded coverage-driven DeepResearch inquiry.
//!
//! Retrieval runs exactly once. The replayed Inquiry projection is the only
//! authority that can finalize a report; every missing, invalid, exhausted, or
//! otherwise non-publishable projection fails closed.

use serde::{Deserialize, Serialize};

use a3s::research::{
    material_evidence_floor, research_contract_outcome, InquiryEvent, InquiryPhase, InquiryState,
    QuestionStatus, ResearchContractOutcome,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum InquiryTerminalOutcome {
    Completed,
    Qualified,
    Exhausted,
}

/// The validated terminal authority carried by a workflow output.
///
/// Old checked-loop outputs predate the Inquiry projection and retain their
/// historical publication gate. Every current host-managed retrieval output
/// must carry a replayable Inquiry projection; losing it is an error, never a
/// signal to reinterpret retrieval output as a completed run.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ValidatedInquiryProjection {
    LegacyCheckedLoop,
    Inquiry {
        events: Vec<InquiryEvent>,
        state: Box<InquiryState>,
    },
}

fn workflow_uses_host_managed_inquiry(workflow: &serde_json::Value) -> bool {
    workflow
        .pointer("/execution/terminal_authority")
        .and_then(serde_json::Value::as_str)
        == Some("host_inquiry_reducer")
        || workflow
            .pointer("/execution/mode")
            .and_then(serde_json::Value::as_str)
            == Some("collect_only")
        || workflow
            .get("mode")
            .and_then(serde_json::Value::as_str)
            .is_some_and(|mode| {
                // `inquiry_collection_wave` is accepted only to classify
                // historical persisted output. Current workflows emit
                // `inquiry_collection`.
                matches!(mode, "inquiry_collection" | "inquiry_collection_wave")
            })
}

fn validate_host_managed_research_contract(
    events: &[InquiryEvent],
    state: &InquiryState,
) -> Result<(), String> {
    let commitments = events
        .iter()
        .filter_map(|event| match event {
            InquiryEvent::ResearchObligationsCommitted {
                obligations,
                stop_conditions,
            } => Some((obligations, stop_conditions)),
            _ => None,
        })
        .collect::<Vec<_>>();
    if commitments.len() != 1 {
        return Err(format!(
            "host-managed DeepResearch Inquiry must contain exactly one research obligations commitment; found {}",
            commitments.len()
        ));
    }
    let (committed_obligations, committed_stop_conditions) = commitments[0];
    if committed_obligations.is_empty() || committed_stop_conditions.is_empty() {
        return Err(
            "host-managed DeepResearch research obligations and stop conditions cannot be empty"
                .to_string(),
        );
    }
    if state.obligations.is_empty() || state.stop_conditions.is_empty() {
        return Err(
            "host-managed DeepResearch Inquiry state omitted its research obligations or stop conditions"
                .to_string(),
        );
    }
    if state.obligations.as_slice() != committed_obligations.as_slice()
        || state.stop_conditions.as_slice() != committed_stop_conditions.as_slice()
    {
        return Err(
            "host-managed DeepResearch Inquiry state differs from its committed research contract"
                .to_string(),
        );
    }

    if matches!(
        state.phase,
        InquiryPhase::Outlining
            | InquiryPhase::Drafting
            | InquiryPhase::Auditing
            | InquiryPhase::Completed
    ) {
        let assessments = events
            .iter()
            .filter_map(|event| match event {
                InquiryEvent::ResearchContractAssessed { assessment } => Some(assessment),
                _ => None,
            })
            .collect::<Vec<_>>();
        if assessments.len() != 1 {
            return Err(format!(
                "host-managed DeepResearch Inquiry must contain exactly one research contract assessment before reporting; found {}",
                assessments.len()
            ));
        }
        let Some(state_assessment) = state.contract_assessment.as_ref() else {
            return Err(
                "host-managed DeepResearch Inquiry state omitted its research contract assessment"
                    .to_string(),
            );
        };
        if state_assessment.obligations.is_empty() || state_assessment.stop_conditions.is_empty() {
            return Err(
                "host-managed DeepResearch contract assessment cannot be empty".to_string(),
            );
        }
        if state_assessment != assessments[0] {
            return Err(
                "host-managed DeepResearch Inquiry state differs from its contract assessment event"
                    .to_string(),
            );
        }
    }

    Ok(())
}

#[cfg(test)]
impl ValidatedInquiryProjection {
    pub(crate) fn terminal_outcome(&self) -> Option<InquiryTerminalOutcome> {
        match self {
            Self::LegacyCheckedLoop => None,
            Self::Inquiry { state, .. } => inquiry_terminal_outcome(state),
        }
    }

    pub(crate) fn state(&self) -> Option<&InquiryState> {
        match self {
            Self::LegacyCheckedLoop => None,
            Self::Inquiry { state, .. } => Some(state),
        }
    }
}

/// Derive the only publishable terminal meaning from the replayed Inquiry.
/// The retrieval adapter produces evidence and never owns completion.
pub(crate) fn inquiry_terminal_outcome(state: &InquiryState) -> Option<InquiryTerminalOutcome> {
    if state.phase == InquiryPhase::Exhausted {
        return Some(InquiryTerminalOutcome::Exhausted);
    }
    if !matches!(
        state.phase,
        InquiryPhase::Outlining
            | InquiryPhase::Drafting
            | InquiryPhase::Auditing
            | InquiryPhase::Completed
    ) {
        return None;
    }
    let material = state
        .questions
        .iter()
        .filter(|question| question.material)
        .collect::<Vec<_>>();
    if material.is_empty()
        || !material_evidence_floor(state)
        || state
            .questions
            .iter()
            .any(|question| question.status == QuestionStatus::Queued)
    {
        return None;
    }
    let contract_outcome = if state.obligations.is_empty() {
        None
    } else {
        match research_contract_outcome(state) {
            Some(ResearchContractOutcome::Satisfied) => Some(InquiryTerminalOutcome::Completed),
            Some(ResearchContractOutcome::Qualified) => Some(InquiryTerminalOutcome::Qualified),
            Some(ResearchContractOutcome::Unsatisfied) | None => return None,
        }
    };
    if state.questions.iter().any(|question| {
        question.status == QuestionStatus::Bounded || question.bound_reason.is_some()
    }) {
        Some(InquiryTerminalOutcome::Qualified)
    } else {
        contract_outcome.or(Some(InquiryTerminalOutcome::Completed))
    }
}

/// Validate and classify the workflow's terminal authority.
pub(crate) fn validated_inquiry_projection(
    workflow: &serde_json::Value,
) -> Result<ValidatedInquiryProjection, String> {
    let host_managed = workflow_uses_host_managed_inquiry(workflow);
    let Some(inquiry) = workflow.get("inquiry") else {
        if host_managed {
            return Err(
                "host-managed DeepResearch output omitted its Inquiry projection".to_string(),
            );
        }
        return Ok(ValidatedInquiryProjection::LegacyCheckedLoop);
    };
    let events: Vec<InquiryEvent> = serde_json::from_value(
        inquiry
            .get("events")
            .cloned()
            .ok_or_else(|| "DeepResearch inquiry projection omitted events".to_string())?,
    )
    .map_err(|error| format!("decode DeepResearch inquiry events: {error}"))?;
    let state: InquiryState = serde_json::from_value(
        inquiry
            .get("state")
            .cloned()
            .ok_or_else(|| "DeepResearch inquiry projection omitted state".to_string())?,
    )
    .map_err(|error| format!("decode DeepResearch inquiry state: {error}"))?;
    if host_managed {
        validate_host_managed_research_contract(&events, &state)?;
    }
    let replayed = a3s::research::replay(&events, &a3s::research::InquiryLimits::default())
        .map_err(|error| format!("replay DeepResearch inquiry projection: {error}"))?;
    if replayed != state {
        return Err("DeepResearch inquiry state differs from its event replay".to_string());
    }
    Ok(ValidatedInquiryProjection::Inquiry {
        events,
        state: Box::new(state),
    })
}

/// Require report-authored Inquiry runs to have completed their draft audit.
/// `Ok(None)` is reserved for genuinely legacy checked-loop outputs.
pub(crate) fn validated_inquiry_publication_outcome(
    workflow: &serde_json::Value,
) -> Result<Option<InquiryTerminalOutcome>, String> {
    match validated_inquiry_projection(workflow)? {
        ValidatedInquiryProjection::LegacyCheckedLoop => Ok(None),
        ValidatedInquiryProjection::Inquiry { state, .. } => {
            if state.phase != InquiryPhase::Completed {
                return Err(format!(
                    "DeepResearch Inquiry must reach Completed before publication; current phase is {:?}",
                    state.phase
                ));
            }
            match inquiry_terminal_outcome(&state) {
                Some(
                    outcome @ (InquiryTerminalOutcome::Completed
                    | InquiryTerminalOutcome::Qualified),
                ) => Ok(Some(outcome)),
                Some(InquiryTerminalOutcome::Exhausted) | None => Err(
                    "completed DeepResearch Inquiry has no publishable terminal outcome"
                        .to_string(),
                ),
            }
        }
    }
}

#[cfg(test)]
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ConvergenceAction {
    Finalize,
    Degrade,
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ConvergenceDecision {
    pub(crate) action: ConvergenceAction,
    pub(crate) reason: String,
}

/// Convert the replayed inquiry projection into the terminal workflow
/// decision. Retrieval is already closed when this function runs.
#[cfg(test)]
pub(crate) fn evaluate_terminal_inquiry_convergence(state: &InquiryState) -> ConvergenceDecision {
    let (action, reason) = match inquiry_terminal_outcome(state) {
        Some(InquiryTerminalOutcome::Completed) => (
            ConvergenceAction::Finalize,
            "all inquiry questions are evidence-answered and ready for outlining".to_string(),
        ),
        Some(InquiryTerminalOutcome::Qualified) => (
            ConvergenceAction::Finalize,
            "all material inquiry questions are evidence-answered; bounded supporting gaps will qualify the report"
                .to_string(),
        ),
        Some(InquiryTerminalOutcome::Exhausted) => (
            ConvergenceAction::Degrade,
            state
                .budget_exhausted_reason
                .clone()
                .unwrap_or_else(|| "the replayed inquiry exhausted its bounded budget".to_string()),
        ),
        None => (
            ConvergenceAction::Degrade,
            format!(
                "the replayed inquiry returned from non-publishable phase {:?}; no hidden continuation was scheduled",
                state.phase
            ),
        ),
    };
    ConvergenceDecision { action, reason }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn legacy_outlining_events() -> Vec<InquiryEvent> {
        vec![
            InquiryEvent::StrategySelected {
                method: a3s::research::ResearchMethod::Focused,
            },
            InquiryEvent::QuestionsQueued {
                questions: vec![a3s::research::Question::queued(
                    "question:material",
                    None,
                    "What does the evidence establish?",
                )],
            },
            InquiryEvent::EvidenceAccepted {
                evidence: a3s::research::EvidenceRef::new(
                    "evidence:one",
                    vec!["claim:one".to_string()],
                    vec!["source:one".to_string()],
                ),
            },
            InquiryEvent::QuestionAnswered {
                question_id: "question:material".to_string(),
                answer: "The accepted evidence establishes the finding.".to_string(),
                evidence_ids: vec!["evidence:one".to_string()],
            },
        ]
    }

    fn contracted_outlining_events(assessed: bool) -> Vec<InquiryEvent> {
        let mut question = a3s::research::Question::queued(
            "question:material",
            None,
            "What does the evidence establish?",
        );
        question.obligation_ids = vec!["obligation:core".to_string()];
        let mut events = vec![
            InquiryEvent::StrategySelected {
                method: a3s::research::ResearchMethod::Focused,
            },
            InquiryEvent::ResearchObligationsCommitted {
                obligations: vec![a3s::research::ResearchObligation::new(
                    "obligation:core",
                    "Core finding",
                    "Resolve the core finding",
                    true,
                    vec!["The finding is supported by traceable evidence".to_string()],
                )],
                stop_conditions: vec!["The core finding is traceable".to_string()],
            },
            InquiryEvent::QuestionsQueued {
                questions: vec![question],
            },
            InquiryEvent::EvidenceAccepted {
                evidence: a3s::research::EvidenceRef::new(
                    "evidence:one",
                    vec!["claim:one".to_string()],
                    vec!["source:one".to_string()],
                ),
            },
            InquiryEvent::QuestionAnswered {
                question_id: "question:material".to_string(),
                answer: "The accepted evidence establishes the finding.".to_string(),
                evidence_ids: vec!["evidence:one".to_string()],
            },
        ];
        if assessed {
            events.push(InquiryEvent::ResearchContractAssessed {
                assessment: a3s::research::ResearchContractAssessment {
                    obligations: vec![a3s::research::ResearchObligationAssessment {
                        obligation_id: "obligation:core".to_string(),
                        criteria: vec![a3s::research::CompletionCriterionAssessment {
                            criterion_index: 0,
                            status: a3s::research::ContractAssessmentStatus::Satisfied,
                            rationale: "The accepted evidence satisfies the criterion.".to_string(),
                            evidence_ids: vec!["evidence:one".to_string()],
                        }],
                        primary_source: None,
                        independent_corroboration: None,
                    }],
                    stop_conditions: vec![a3s::research::StopConditionAssessment {
                        condition_index: 0,
                        status: a3s::research::ContractAssessmentStatus::Satisfied,
                        rationale: "The finding is traceable.".to_string(),
                        evidence_ids: vec!["evidence:one".to_string()],
                    }],
                    diagnostics: Vec::new(),
                },
            });
        }
        events
    }

    fn workflow_with_inquiry(events: Vec<InquiryEvent>, host_managed: bool) -> serde_json::Value {
        let state = a3s::research::replay(&events, &a3s::research::InquiryLimits::default())
            .expect("replay test inquiry");
        let mut workflow = serde_json::json!({
            "inquiry": {"events": events, "state": state}
        });
        if host_managed {
            workflow["execution"] = serde_json::json!({
                "mode": "collect_only",
                "terminal_authority": "host_inquiry_reducer"
            });
        }
        workflow
    }

    #[test]
    fn terminal_authority_distinguishes_legacy_absence_from_host_projection_loss() {
        let legacy = serde_json::json!({
            "mode": "direct_web",
            "checker": {"decision": "finalize"}
        });
        assert!(matches!(
            validated_inquiry_projection(&legacy),
            Ok(ValidatedInquiryProjection::LegacyCheckedLoop)
        ));

        let host_managed = serde_json::json!({
            "mode": "direct_web",
            "execution": {"terminal_authority": "host_inquiry_reducer"},
            "checker": {"decision": "finalize"}
        });
        let error = validated_inquiry_projection(&host_managed)
            .expect_err("a raw host-managed wave cannot fall back to its inner checker");
        assert!(error.contains("omitted its Inquiry projection"), "{error}");
    }

    #[test]
    fn host_managed_projection_cannot_delete_the_contract_event_and_state_together() {
        let legacy = workflow_with_inquiry(legacy_outlining_events(), false);
        assert!(matches!(
            validated_inquiry_projection(&legacy),
            Ok(ValidatedInquiryProjection::Inquiry { .. })
        ));

        let host_managed = workflow_with_inquiry(legacy_outlining_events(), true);
        let error = validated_inquiry_projection(&host_managed)
            .expect_err("host-managed Inquiry requires its stable research contract");
        assert!(
            error.contains("exactly one research obligations commitment; found 0"),
            "{error}"
        );
    }

    #[test]
    fn host_managed_outlining_projection_requires_one_persisted_contract_assessment() {
        let missing = workflow_with_inquiry(contracted_outlining_events(false), true);
        let error = validated_inquiry_projection(&missing)
            .expect_err("Outlining cannot delete both assessment event and state");
        assert!(
            error.contains("exactly one research contract assessment before reporting; found 0"),
            "{error}"
        );

        let complete = workflow_with_inquiry(contracted_outlining_events(true), true);
        let projection = validated_inquiry_projection(&complete)
            .expect("a uniquely committed and assessed host contract is valid");
        assert_eq!(
            projection.terminal_outcome(),
            Some(InquiryTerminalOutcome::Completed)
        );
    }

    #[test]
    fn host_managed_projection_rejects_duplicate_contract_authority_events() {
        let mut workflow = workflow_with_inquiry(contracted_outlining_events(true), true);
        let events = workflow["inquiry"]["events"].as_array_mut().unwrap();
        let commitment = events
            .iter()
            .find(|event| event["type"] == "research_obligations_committed")
            .cloned()
            .unwrap();
        events.push(commitment);
        let error = validated_inquiry_projection(&workflow)
            .expect_err("duplicate contract commitments cannot share authority");
        assert!(
            error.contains("exactly one research obligations commitment; found 2"),
            "{error}"
        );

        let mut workflow = workflow_with_inquiry(contracted_outlining_events(true), true);
        let events = workflow["inquiry"]["events"].as_array_mut().unwrap();
        let assessment = events
            .iter()
            .find(|event| event["type"] == "research_contract_assessed")
            .cloned()
            .unwrap();
        events.push(assessment);
        let error = validated_inquiry_projection(&workflow)
            .expect_err("duplicate contract assessments cannot share authority");
        assert!(
            error.contains("exactly one research contract assessment before reporting; found 2"),
            "{error}"
        );
    }

    #[test]
    fn inquiry_publication_requires_the_completed_audit_phase() {
        let events = legacy_outlining_events();
        let state =
            a3s::research::replay(&events, &a3s::research::InquiryLimits::default()).unwrap();
        let workflow = serde_json::json!({
            "inquiry": {"events": events, "state": state}
        });

        let projection = validated_inquiry_projection(&workflow).unwrap();
        assert_eq!(
            projection.state().map(|state| state.phase),
            Some(InquiryPhase::Outlining)
        );
        assert_eq!(
            projection.terminal_outcome(),
            Some(InquiryTerminalOutcome::Completed)
        );
        let error = validated_inquiry_publication_outcome(&workflow)
            .expect_err("evidence readiness must not masquerade as report completion");
        assert!(error.contains("current phase is Outlining"), "{error}");
    }

    #[test]
    fn exhausted_terminal_inquiry_degrades_after_coverage_driven_retrieval() {
        let limits = a3s::research::InquiryLimits::default();
        let mut state = InquiryState::default();
        state
            .apply(
                &a3s::research::InquiryEvent::StrategySelected {
                    method: a3s::research::ResearchMethod::Focused,
                },
                &limits,
            )
            .expect("strategy");
        state
            .apply(
                &a3s::research::InquiryEvent::QuestionsQueued {
                    questions: vec![a3s::research::Question::queued(
                        "question:material",
                        None,
                        "What does the evidence establish?",
                    )],
                },
                &limits,
            )
            .expect("question");
        state
            .apply(
                &a3s::research::InquiryEvent::QuestionBounded {
                    question_id: "question:material".to_string(),
                    reason: "No accepted evidence remained.".to_string(),
                },
                &limits,
            )
            .expect("bounded");
        state
            .apply(
                &a3s::research::InquiryEvent::BudgetExhausted {
                    reason: "material question remained bounded".to_string(),
                },
                &limits,
            )
            .expect("exhausted");

        let decision = evaluate_terminal_inquiry_convergence(&state);
        assert_eq!(decision.action, ConvergenceAction::Degrade);
        assert_eq!(decision.reason, "material question remained bounded");
    }
}
