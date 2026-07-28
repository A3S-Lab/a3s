//! Report discovery, terminal authority, and transient state for DeepResearch runs.

use super::*;

pub(super) fn research_report_view_spec(
    output: &str,
    workspace: &Path,
) -> Option<remote_ui::ViewSpec> {
    let artifacts = research_report_artifacts_from_output(output, workspace)?;
    remote_ui::local_file_view(&artifacts.html).ok()
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum DeepResearchRunOutcome {
    #[default]
    Active,
    Completed,
    Qualified,
    SourceBacked,
    NoEvidence,
    Degraded,
}

impl DeepResearchRunOutcome {
    pub(super) fn report_ready(self) -> bool {
        matches!(
            self,
            Self::Completed | Self::Qualified | Self::SourceBacked | Self::NoEvidence
        )
    }

    pub(super) fn ensure_smoke_success(
        self,
        artifacts: &ResearchReportArtifacts,
    ) -> anyhow::Result<()> {
        match self {
            Self::Completed | Self::Qualified | Self::SourceBacked | Self::NoEvidence => Ok(()),
            Self::Degraded => anyhow::bail!(
                "DeepResearch smoke produced only a degraded recovery report at {}",
                artifacts.html.display()
            ),
            Self::Active => anyhow::bail!("DeepResearch smoke ended without a terminal outcome"),
        }
    }
}

/// Ephemeral host data retained between evidence collection and report
/// synthesis. Durable run truth belongs to the event journal; this snapshot
/// only carries values that cannot be reconstructed from the run projection.
#[derive(Debug, Default)]
pub(super) struct DeepResearchWorkflowSnapshot {
    pub(super) output: Option<String>,
    pub(super) metadata: Option<serde_json::Value>,
    pub(super) args: Option<serde_json::Value>,
    pub(super) typed_runner: bool,
}

impl DeepResearchWorkflowSnapshot {
    pub(super) fn reset_for_run(&mut self) {
        *self = Self::default();
    }

    pub(super) fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Validate the successful-report authority after synthesis.
///
/// Inquiry-backed runs must have committed outline, section, and passing audit
/// events. `Ok(None)` means the output is a legacy checked-loop run whose
/// publication remains governed by the legacy report validators.
pub(super) fn deep_research_inquiry_publication_outcome(
    workflow_output: &str,
    workflow_metadata: Option<&serde_json::Value>,
) -> Result<Option<DeepResearchRunOutcome>, String> {
    let canonical = deep_research_canonical_workflow_output(workflow_output, workflow_metadata);
    let value = serde_json::from_str::<serde_json::Value>(&canonical)
        .map_err(|error| format!("decode DeepResearch workflow for publication: {error}"))?;
    validated_inquiry_publication_outcome(&value).map(|outcome| {
        outcome.map(|outcome| match outcome {
            InquiryTerminalOutcome::Completed => DeepResearchRunOutcome::Completed,
            InquiryTerminalOutcome::Qualified => DeepResearchRunOutcome::Qualified,
            InquiryTerminalOutcome::Exhausted => DeepResearchRunOutcome::Degraded,
        })
    })
}

#[cfg(test)]
mod deep_research_workflow_snapshot_tests {
    use super::*;

    #[test]
    pub(super) fn reset_for_run_discards_prior_transient_values() {
        let mut snapshot = DeepResearchWorkflowSnapshot {
            output: Some("stale output".to_string()),
            metadata: Some(serde_json::json!({"stale": true})),
            args: Some(serde_json::json!({"run_id": "old"})),
            typed_runner: true,
        };

        snapshot.reset_for_run();

        assert!(snapshot.output.is_none());
        assert!(snapshot.metadata.is_none());
        assert!(snapshot.args.is_none());
        assert!(!snapshot.typed_runner);
    }

    #[test]
    pub(super) fn clear_removes_all_transient_run_data() {
        let mut snapshot = DeepResearchWorkflowSnapshot {
            output: Some("output".to_string()),
            metadata: Some(serde_json::json!({"source": "workflow"})),
            args: Some(serde_json::json!({"run_id": "run"})),
            typed_runner: true,
        };

        snapshot.clear();

        assert!(snapshot.output.is_none());
        assert!(snapshot.metadata.is_none());
        assert!(snapshot.args.is_none());
        assert!(!snapshot.typed_runner);
    }

    #[test]
    fn every_typed_publication_outcome_is_report_ready() {
        for outcome in [
            DeepResearchRunOutcome::Completed,
            DeepResearchRunOutcome::Qualified,
            DeepResearchRunOutcome::SourceBacked,
            DeepResearchRunOutcome::NoEvidence,
        ] {
            assert!(outcome.report_ready());
        }
        assert!(!DeepResearchRunOutcome::Active.report_ready());
        assert!(!DeepResearchRunOutcome::Degraded.report_ready());
    }
}
