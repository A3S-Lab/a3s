use super::*;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum EnvelopeFault {
    Bootstrap,
    Planned,
    Both,
}

impl EnvelopeFault {
    const ALL: [Self; 3] = [Self::Bootstrap, Self::Planned, Self::Both];

    fn affects(self, stage: WorkflowStage) -> bool {
        matches!(
            (self, stage),
            (Self::Bootstrap, WorkflowStage::Bootstrap)
                | (Self::Planned, WorkflowStage::PlannedRetrieval)
                | (Self::Both, _)
        )
    }

    fn id(self) -> &'static str {
        match self {
            Self::Bootstrap => "bootstrap",
            Self::Planned => "planned",
            Self::Both => "both",
        }
    }

    fn expected_publication(self) -> DeepResearchEvidenceFirstPublication {
        match self {
            Self::Bootstrap => DeepResearchEvidenceFirstPublication::Synthesized,
            Self::Planned | Self::Both => DeepResearchEvidenceFirstPublication::NoEvidence,
        }
    }
}

struct MalformedEnvelopePorts<'a> {
    replay: &'a ProductReplay,
    fault: EnvelopeFault,
}

#[async_trait::async_trait]
impl StructuredGenerationPort for MalformedEnvelopePorts<'_> {
    async fn generate_object(&self, request: GenerationRequest) -> Result<Value, String> {
        Ok(match request.stage {
            GenerationStage::Planning => planner_outline(self.replay),
            GenerationStage::Report => report_proposal(self.replay),
            GenerationStage::Editorial => editorial_plan(self.replay),
        })
    }
}

#[async_trait::async_trait]
impl WorkflowExecutionPort for MalformedEnvelopePorts<'_> {
    async fn execute_workflow(&self, request: WorkflowRequest) -> Result<WorkflowOutput, String> {
        if self.fault.affects(request.stage) {
            return Ok(malformed_envelope_output(self.replay, request.stage));
        }
        Ok(match request.stage {
            WorkflowStage::Bootstrap => bootstrap_output(self.replay),
            WorkflowStage::PlannedRetrieval => planned_output(self.replay),
        })
    }
}

#[tokio::test]
async fn malformed_envelope_matrix_survives_the_persisted_product_adapter() {
    let replay = load_product_replays()
        .into_iter()
        .find(|replay| replay.id == "F07")
        .expect("frozen product corpus requires F07");

    for fault in EnvelopeFault::ALL {
        let workspace = tempfile::tempdir().expect("malformed-envelope product workspace");
        materialize_workspace_sources(workspace.path(), &replay);
        let (_agent, session) = product_adapter_fixture_session(workspace.path()).await;
        let run_id = format!("persisted-envelope-{}", fault.id());
        let args = workflow_args(&replay, &run_id);
        let run_clock = EvidenceFirstRunClock::initialize(&session, &args)
            .await
            .unwrap_or_else(|error| panic!("{}: initialize product journal: {error}", fault.id()));
        let (progress_tx, _progress_rx) = mpsc::channel(PROGRESS_CHANNEL_CAPACITY);
        let product_runtime = A3sDeepResearchRuntime {
            session: &session,
            progress_tx: &progress_tx,
            run_clock: &run_clock,
        };
        let ports = MalformedEnvelopePorts {
            replay: &replay,
            fault,
        };
        let engine = DeepResearchEngine::new(&ports, &ports, &product_runtime, &product_runtime);

        let run = engine
            .execute(args)
            .await
            .unwrap_or_else(|error| panic!("{}: execute product matrix: {error}", fault.id()));
        let expected_publication = fault.expected_publication();
        assert_eq!(run.publication, expected_publication, "{}", fault.id());

        let requested_outcome = publication_outcome(expected_publication);
        let workflow_output = run.output_json();
        let settled =
            crate::tui::settle_deep_research_cli_run(crate::tui::DeepResearchCliSettlement {
                workspace: workspace.path(),
                run_id: &run_id,
                query: &replay.query,
                workflow_succeeded: true,
                workflow_output: &workflow_output,
                workflow_metadata: None,
                requested_outcome,
                artifacts: &run.artifacts,
                artifact_authority:
                    crate::tui::DeepResearchTerminalArtifactAuthority::ValidatedPublication,
            })
            .await
            .unwrap_or_else(|error| panic!("{}: settle product matrix: {error}", fault.id()));
        assert_eq!(settled, requested_outcome, "{}", fault.id());

        let recovered =
            super::super::super::deep_research_artifacts::recover_deep_research_publication_receipt(
                workspace.path(),
                &replay.query,
                &run_id,
            )
            .unwrap_or_else(|error| panic!("{}: recover product receipt: {error}", fault.id()))
            .unwrap_or_else(|| panic!("{}: missing product receipt", fault.id()));
        assert_eq!(
            recovered.publication,
            expected_publication,
            "{}",
            fault.id()
        );
        let expected_claim_count =
            if expected_publication == DeepResearchEvidenceFirstPublication::Synthesized {
                replay.admitted_claims().count()
            } else {
                0
            };
        let expected_source_count =
            if expected_publication == DeepResearchEvidenceFirstPublication::NoEvidence {
                0
            } else {
                replay.sources.len()
            };
        assert_eq!(
            recovered.quality.accepted_claim_count,
            expected_claim_count,
            "{}",
            fault.id()
        );
        assert_eq!(
            recovered.quality.source_count,
            expected_source_count,
            "{}",
            fault.id()
        );
        assert_eq!(
            (
                recovered.quality.accepted_relation_count,
                recovered.quality.accepted_derivation_count,
                recovered.quality.accepted_basis_edge_count,
                recovered.quality.accepted_gap_count,
            ),
            (0, 0, 0, 0),
            "{}",
            fault.id()
        );

        let journal = crate::tui::deep_research_state_journal::DeepResearchStateJournal::open(
            workspace.path(),
            &run_id,
        )
        .await
        .unwrap_or_else(|error| panic!("{}: reopen product journal: {error}", fault.id()))
        .unwrap_or_else(|| panic!("{}: missing product journal", fault.id()));
        let projection = journal
            .projection()
            .unwrap_or_else(|error| panic!("{}: project product journal: {error}", fault.id()));
        assert_eq!(projection.outcome, requested_outcome, "{}", fault.id());
        assert_eq!(
            projection.claim_count,
            expected_claim_count,
            "{}",
            fault.id()
        );
        assert_eq!(
            projection.source_count,
            expected_source_count,
            "{}",
            fault.id()
        );
    }
}

pub(super) fn malformed_envelope_output(
    replay: &ProductReplay,
    stage: WorkflowStage,
) -> WorkflowOutput {
    let mode = match stage {
        WorkflowStage::Bootstrap => "bootstrap_acquisition",
        WorkflowStage::PlannedRetrieval => "inquiry_collection",
    };
    WorkflowOutput {
        output: serde_json::json!({
            "query": replay.query,
            "mode": mode,
            "acquisition": {
                "packet": {
                    "version": 2,
                    "sources": replay.sources.iter().map(|source| {
                        serde_json::json!({
                            "source_id": source.id,
                            "title": source.title,
                            "url_or_path": source_anchor(source),
                            "chunks": [{
                                "chunk_id": format!("{}:chunk:1", source.id),
                                "text": source.content.trim(),
                            }],
                        })
                    }).collect::<Vec<_>>(),
                },
            },
            "execution": {
                "terminal_authority": "host_inquiry_reducer",
            },
        })
        .to_string(),
        metadata: None,
    }
}
