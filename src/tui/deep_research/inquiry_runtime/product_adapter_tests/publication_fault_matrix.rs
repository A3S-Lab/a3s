use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EvidenceEnvelope {
    Valid,
    Malformed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PublicationStage {
    InitialSource,
    FinalReport,
    RecoverySource,
    NoEvidence,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum FaultBoundary {
    BeforeCommit,
    AfterCommit,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
struct PublicationFault {
    stage: PublicationStage,
    boundary: FaultBoundary,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EngineCompletion {
    Success,
    Failure,
}

struct PublicationScenario {
    id: &'static str,
    evidence: EvidenceEnvelope,
    faults: &'static [PublicationFault],
    completion: EngineCompletion,
    publication: Option<DeepResearchEvidenceFirstPublication>,
    attempts: &'static [PublicationStage],
    commits: &'static [PublicationStage],
}

const PUBLICATION_SCENARIOS: &[PublicationScenario] = &[
    PublicationScenario {
        id: "initial-source-before-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[PublicationFault {
            stage: PublicationStage::InitialSource,
            boundary: FaultBoundary::BeforeCommit,
        }],
        completion: EngineCompletion::Failure,
        publication: None,
        attempts: &[PublicationStage::InitialSource],
        commits: &[],
    },
    PublicationScenario {
        id: "initial-source-after-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[PublicationFault {
            stage: PublicationStage::InitialSource,
            boundary: FaultBoundary::AfterCommit,
        }],
        completion: EngineCompletion::Failure,
        publication: Some(DeepResearchEvidenceFirstPublication::SourceBacked),
        attempts: &[PublicationStage::InitialSource],
        commits: &[PublicationStage::InitialSource],
    },
    PublicationScenario {
        id: "final-before-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[PublicationFault {
            stage: PublicationStage::FinalReport,
            boundary: FaultBoundary::BeforeCommit,
        }],
        completion: EngineCompletion::Success,
        publication: Some(DeepResearchEvidenceFirstPublication::SourceBacked),
        attempts: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
        commits: &[
            PublicationStage::InitialSource,
            PublicationStage::RecoverySource,
        ],
    },
    PublicationScenario {
        id: "final-after-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[PublicationFault {
            stage: PublicationStage::FinalReport,
            boundary: FaultBoundary::AfterCommit,
        }],
        completion: EngineCompletion::Success,
        publication: Some(DeepResearchEvidenceFirstPublication::SourceBacked),
        attempts: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
        commits: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
    },
    PublicationScenario {
        id: "final-before-commit-recovery-before-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[
            PublicationFault {
                stage: PublicationStage::FinalReport,
                boundary: FaultBoundary::BeforeCommit,
            },
            PublicationFault {
                stage: PublicationStage::RecoverySource,
                boundary: FaultBoundary::BeforeCommit,
            },
        ],
        completion: EngineCompletion::Failure,
        publication: Some(DeepResearchEvidenceFirstPublication::SourceBacked),
        attempts: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
        commits: &[PublicationStage::InitialSource],
    },
    PublicationScenario {
        id: "final-after-commit-recovery-before-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[
            PublicationFault {
                stage: PublicationStage::FinalReport,
                boundary: FaultBoundary::AfterCommit,
            },
            PublicationFault {
                stage: PublicationStage::RecoverySource,
                boundary: FaultBoundary::BeforeCommit,
            },
        ],
        completion: EngineCompletion::Failure,
        publication: Some(DeepResearchEvidenceFirstPublication::Synthesized),
        attempts: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
        commits: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
        ],
    },
    PublicationScenario {
        id: "final-after-commit-recovery-after-commit",
        evidence: EvidenceEnvelope::Valid,
        faults: &[
            PublicationFault {
                stage: PublicationStage::FinalReport,
                boundary: FaultBoundary::AfterCommit,
            },
            PublicationFault {
                stage: PublicationStage::RecoverySource,
                boundary: FaultBoundary::AfterCommit,
            },
        ],
        completion: EngineCompletion::Failure,
        publication: Some(DeepResearchEvidenceFirstPublication::SourceBacked),
        attempts: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
        commits: &[
            PublicationStage::InitialSource,
            PublicationStage::FinalReport,
            PublicationStage::RecoverySource,
        ],
    },
    PublicationScenario {
        id: "no-evidence-before-commit",
        evidence: EvidenceEnvelope::Malformed,
        faults: &[PublicationFault {
            stage: PublicationStage::NoEvidence,
            boundary: FaultBoundary::BeforeCommit,
        }],
        completion: EngineCompletion::Failure,
        publication: None,
        attempts: &[PublicationStage::NoEvidence],
        commits: &[],
    },
    PublicationScenario {
        id: "no-evidence-after-commit",
        evidence: EvidenceEnvelope::Malformed,
        faults: &[PublicationFault {
            stage: PublicationStage::NoEvidence,
            boundary: FaultBoundary::AfterCommit,
        }],
        completion: EngineCompletion::Failure,
        publication: Some(DeepResearchEvidenceFirstPublication::NoEvidence),
        attempts: &[PublicationStage::NoEvidence],
        commits: &[PublicationStage::NoEvidence],
    },
];

struct PublicationScenarioPorts<'a> {
    replay: &'a ProductReplay,
    evidence: EvidenceEnvelope,
}

#[async_trait::async_trait]
impl StructuredGenerationPort for PublicationScenarioPorts<'_> {
    async fn generate_object(&self, request: GenerationRequest) -> Result<Value, String> {
        Ok(match request.stage {
            GenerationStage::Planning => planner_outline(self.replay),
            GenerationStage::Report => report_proposal(self.replay),
            GenerationStage::Editorial => editorial_plan(self.replay),
        })
    }
}

#[async_trait::async_trait]
impl WorkflowExecutionPort for PublicationScenarioPorts<'_> {
    async fn execute_workflow(&self, request: WorkflowRequest) -> Result<WorkflowOutput, String> {
        match self.evidence {
            EvidenceEnvelope::Valid => Ok(match request.stage {
                WorkflowStage::Bootstrap => bootstrap_output(self.replay),
                WorkflowStage::PlannedRetrieval => planned_output(self.replay),
            }),
            EvidenceEnvelope::Malformed => Ok(super::fault_matrix::malformed_envelope_output(
                self.replay,
                request.stage,
            )),
        }
    }
}

#[derive(Default)]
struct PublicationFaultState {
    source_calls: usize,
    attempts: Vec<PublicationStage>,
    commits: Vec<PublicationStage>,
}

struct PublicationFaultPort<'a> {
    delegate: &'a A3sDeepResearchRuntime<'a>,
    faults: &'static [PublicationFault],
    state: Mutex<PublicationFaultState>,
}

impl PublicationFaultPort<'_> {
    fn stage(&self, request: &PublicationRequest) -> PublicationStage {
        let mut state = self.state.lock().expect("publication fault state lock");
        let stage = match request {
            PublicationRequest::SourceBacked { .. } => {
                let stage = if state.source_calls == 0 {
                    PublicationStage::InitialSource
                } else {
                    PublicationStage::RecoverySource
                };
                state.source_calls += 1;
                stage
            }
            PublicationRequest::Synthesized { .. } => PublicationStage::FinalReport,
            PublicationRequest::NoEvidence { .. } => PublicationStage::NoEvidence,
        };
        state.attempts.push(stage);
        stage
    }

    fn has_fault(&self, stage: PublicationStage, boundary: FaultBoundary) -> bool {
        self.faults.contains(&PublicationFault { stage, boundary })
    }

    fn record_commit(&self, stage: PublicationStage) {
        self.state
            .lock()
            .expect("publication fault state lock")
            .commits
            .push(stage);
    }

    fn records(&self) -> (Vec<PublicationStage>, Vec<PublicationStage>) {
        let state = self.state.lock().expect("publication fault state lock");
        (state.attempts.clone(), state.commits.clone())
    }
}

#[async_trait::async_trait]
impl PublicationPort for PublicationFaultPort<'_> {
    async fn publish(
        &self,
        request: PublicationRequest,
    ) -> Result<ResearchReportArtifacts, String> {
        let stage = self.stage(&request);
        if self.has_fault(stage, FaultBoundary::BeforeCommit) {
            return Err(format!("injected {stage:?} failure before commit"));
        }
        let artifacts = self.delegate.publish(request).await?;
        self.record_commit(stage);
        if self.has_fault(stage, FaultBoundary::AfterCommit) {
            return Err(format!("injected {stage:?} failure after commit"));
        }
        Ok(artifacts)
    }
}

#[tokio::test]
async fn publication_fault_matrix_preserves_exact_commits_through_product_settlement() {
    let replay = load_product_replays()
        .into_iter()
        .find(|replay| replay.id == "F07")
        .expect("frozen product corpus requires F07");

    for scenario in PUBLICATION_SCENARIOS {
        let workspace = tempfile::tempdir().expect("publication-fault product workspace");
        materialize_workspace_sources(workspace.path(), &replay);
        let (_agent, session) = product_adapter_fixture_session(workspace.path()).await;
        let run_id = format!("publication-fault-{}", scenario.id);
        let args = workflow_args(&replay, &run_id);
        let run_clock = EvidenceFirstRunClock::initialize(&session, &args)
            .await
            .unwrap_or_else(|error| panic!("{}: initialize product journal: {error}", scenario.id));
        let (progress_tx, _progress_rx) = mpsc::channel(PROGRESS_CHANNEL_CAPACITY);
        let product_runtime = A3sDeepResearchRuntime {
            session: &session,
            progress_tx: &progress_tx,
            run_clock: &run_clock,
        };
        let ports = PublicationScenarioPorts {
            replay: &replay,
            evidence: scenario.evidence,
        };
        let publication = PublicationFaultPort {
            delegate: &product_runtime,
            faults: scenario.faults,
            state: Mutex::new(PublicationFaultState::default()),
        };
        let engine = DeepResearchEngine::new(&ports, &ports, &publication, &product_runtime);

        let result = engine.execute(args).await;
        assert_eq!(
            result.is_ok(),
            scenario.completion == EngineCompletion::Success,
            "{}",
            scenario.id
        );
        let workflow_output = match &result {
            Ok(run) => run.output_json(),
            Err(error) => error.to_string(),
        };
        let (attempts, commits) = publication.records();
        assert_eq!(attempts, scenario.attempts, "{}", scenario.id);
        assert_eq!(commits, scenario.commits, "{}", scenario.id);

        let receipt =
            super::super::super::deep_research_artifacts::recover_deep_research_publication_receipt(
                workspace.path(),
                &replay.query,
                &run_id,
            )
            .unwrap_or_else(|error| panic!("{}: inspect exact receipt: {error}", scenario.id));
        assert_eq!(
            receipt.as_ref().map(|report| report.publication),
            scenario.publication,
            "{}",
            scenario.id
        );
        let resolved = crate::tui::resolve_deep_research_run_publication(
            workspace.path(),
            &replay.query,
            &run_id,
            &workflow_output,
        )
        .unwrap_or_else(|error| panic!("{}: resolve terminal publication: {error}", scenario.id));
        assert_eq!(resolved, receipt, "{}", scenario.id);

        let (artifacts, authority, requested_outcome, expected_claims, expected_sources) =
            if let Some(report) = resolved {
                (
                    report.artifacts,
                    crate::tui::DeepResearchTerminalArtifactAuthority::ValidatedPublication,
                    publication_outcome(report.publication),
                    report.quality.accepted_claim_count,
                    report.quality.source_count,
                )
            } else {
                let artifacts =
                    super::super::super::deep_research_artifacts::materialize_deep_research_recovery_report(
                        workspace.path(),
                        &replay.query,
                        "the publication transaction did not commit",
                        &workflow_output,
                        None,
                    )
                    .unwrap_or_else(|error| {
                        panic!("{}: materialize verified recovery: {error}", scenario.id)
                    });
                (
                    artifacts,
                    crate::tui::DeepResearchTerminalArtifactAuthority::VerifiedRecovery,
                    ResearchOutcome::Degraded,
                    0,
                    0,
                )
            };
        let accepted_evidence = crate::tui::accepted_evidence_ledger(&workflow_output, None);
        crate::tui::record_deep_research_evidence_ledger(
            workspace.path(),
            &run_id,
            &accepted_evidence,
        )
        .await
        .unwrap_or_else(|error| {
            panic!(
                "{}: record TUI pre-settlement evidence: {error}",
                scenario.id
            )
        });
        let settled =
            crate::tui::settle_deep_research_cli_run(crate::tui::DeepResearchCliSettlement {
                workspace: workspace.path(),
                run_id: &run_id,
                query: &replay.query,
                workflow_succeeded: result.is_ok(),
                workflow_output: &workflow_output,
                workflow_metadata: None,
                requested_outcome,
                artifacts: &artifacts,
                artifact_authority: authority,
            })
            .await
            .unwrap_or_else(|error| panic!("{}: settle product adapter: {error}", scenario.id));
        assert_eq!(settled, requested_outcome, "{}", scenario.id);

        let journal = crate::tui::deep_research_state_journal::DeepResearchStateJournal::open(
            workspace.path(),
            &run_id,
        )
        .await
        .unwrap_or_else(|error| panic!("{}: reopen product journal: {error}", scenario.id))
        .unwrap_or_else(|| panic!("{}: missing product journal", scenario.id));
        let projection = journal
            .projection()
            .unwrap_or_else(|error| panic!("{}: project product journal: {error}", scenario.id));
        assert_eq!(projection.outcome, requested_outcome, "{}", scenario.id);
        assert_eq!(projection.claim_count, expected_claims, "{}", scenario.id);
        assert_eq!(projection.source_count, expected_sources, "{}", scenario.id);
        assert!(projection.active_steps.is_empty(), "{}", scenario.id);
        assert!(projection.active_children.is_empty(), "{}", scenario.id);
    }
}
