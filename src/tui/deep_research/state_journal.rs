//! Event-sourced domain state for DeepResearch runs.
//!
//! Flow remains authoritative for step execution and `AgentEvent` remains
//! authoritative for session/tool lifecycle. This module projects normalized
//! references to those streams into a replayable research-domain graph.

use a3s_code_core::state_graph::{
    graph_event_head, ExternalEvent, FileGraphEventStore, GraphEventRecord, GraphEventStore,
    GraphPatch, GraphRuntime, GraphSaveOutcome, PatchOperation,
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::path::{Path, PathBuf};

const RUN_OBJECT_TYPE: &str = "deep_research.run";
const SPEC_OBJECT_TYPE: &str = "deep_research.spec";
const SOURCE_OBJECT_TYPE: &str = "deep_research.source";
const EVIDENCE_OBJECT_TYPE: &str = "deep_research.evidence";
const CLAIM_OBJECT_TYPE: &str = "deep_research.claim";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ResearchOutcome {
    #[default]
    Active,
    Failed,
    Degraded,
    Qualified,
    Completed,
}

impl ResearchOutcome {
    pub(crate) fn is_terminal(self) -> bool {
        self != Self::Active
    }

    fn quality(self) -> u8 {
        match self {
            Self::Active => 0,
            Self::Failed => 1,
            Self::Degraded => 2,
            Self::Qualified => 3,
            Self::Completed => 4,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ResearchSpec {
    pub(crate) query: String,
    pub(crate) current_date: String,
    pub(crate) evidence_scope: String,
    pub(crate) required_claims: Vec<String>,
    pub(crate) total_budget_ms: u64,
    #[serde(default)]
    pub(crate) retrieval_stage_budget_ms: u64,
    #[serde(default)]
    pub(crate) question_review_stage_budget_ms: u64,
    pub(crate) finalization_reserve_ms: u64,
    #[serde(default)]
    pub(crate) host_pid: u32,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ResearchRunProjection {
    pub(crate) run_id: String,
    pub(crate) outcome: ResearchOutcome,
    pub(crate) last_domain_event: String,
    pub(crate) last_source_sequence: u64,
    pub(crate) active_steps: Vec<String>,
    pub(crate) active_children: Vec<String>,
    pub(crate) convergence_reason: Option<String>,
    pub(crate) artifact_evidence_head: Option<String>,
    pub(crate) report_audit_reason: Option<String>,
    pub(crate) accepted_evidence_count: usize,
    pub(crate) source_count: usize,
    pub(crate) claim_count: usize,
    #[serde(default)]
    pub(crate) accepted_relation_count: usize,
    #[serde(default)]
    pub(crate) accepted_derivation_count: usize,
    #[serde(default)]
    pub(crate) accepted_basis_edge_count: usize,
    #[serde(default)]
    pub(crate) accepted_gap_count: usize,
    #[serde(default)]
    pub(crate) report_cited_source_count: Option<usize>,
}

impl ResearchRunProjection {
    fn new(run_id: String) -> Self {
        Self {
            run_id,
            outcome: ResearchOutcome::Active,
            last_domain_event: "research.run.created".to_string(),
            last_source_sequence: 1,
            active_steps: Vec::new(),
            active_children: Vec::new(),
            convergence_reason: None,
            artifact_evidence_head: None,
            report_audit_reason: None,
            accepted_evidence_count: 0,
            source_count: 0,
            claim_count: 0,
            accepted_relation_count: 0,
            accepted_derivation_count: 0,
            accepted_basis_edge_count: 0,
            accepted_gap_count: 0,
            report_cited_source_count: None,
        }
    }

    pub(crate) fn can_schedule(&self) -> bool {
        !self.outcome.is_terminal()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct ResearchDomainEvent {
    pub(crate) source: String,
    pub(crate) source_sequence: u64,
    pub(crate) source_event_id: String,
    pub(crate) name: String,
    pub(crate) payload: serde_json::Value,
}

pub(crate) struct DeepResearchStateJournal {
    store: FileGraphEventStore,
    checkpoint_root: PathBuf,
    store_id: String,
    run_id: String,
    runtime: GraphRuntime,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
struct ResearchCheckpoint {
    schema_version: u32,
    run_id: String,
    event_head: Option<String>,
    event_count: usize,
    object_count: usize,
    relation_count: usize,
    projection: ResearchRunProjection,
}

impl DeepResearchStateJournal {
    pub(crate) async fn create(workspace: &Path, run_id: &str, spec: ResearchSpec) -> Result<Self> {
        validate_run_id(run_id)?;
        let spec = bounded_spec(spec);
        let store = FileGraphEventStore::new(store_root(workspace));
        let checkpoint_root = checkpoint_root(workspace);
        if store.load(run_id).await?.is_some() {
            anyhow::bail!("DeepResearch run `{run_id}` already exists");
        }

        let mut runtime = GraphRuntime::new().with_correlation_id(run_id);
        let run = ResearchRunProjection::new(run_id.to_string());
        runtime
            .project_external(
                ExternalEvent {
                    source: "deep_research".to_string(),
                    stream_id: run_id.to_string(),
                    sequence: 1,
                    event_id: format!("{run_id}:created"),
                    name: "research.run.created".to_string(),
                    payload: serde_json::to_value(&spec)?,
                },
                GraphPatch::new(
                    0,
                    vec![
                        PatchOperation::AddObject {
                            id: run_object_id(run_id),
                            object_type: RUN_OBJECT_TYPE.to_string(),
                            data: serde_json::to_value(run)?,
                        },
                        PatchOperation::AddObject {
                            id: spec_object_id(run_id),
                            object_type: SPEC_OBJECT_TYPE.to_string(),
                            data: serde_json::to_value(spec)?,
                        },
                    ],
                ),
            )
            .context("project DeepResearch creation event")?;
        match store.save_if_head(run_id, None, runtime.events()).await? {
            GraphSaveOutcome::Saved => {
                let journal = Self {
                    store,
                    checkpoint_root,
                    store_id: run_id.to_string(),
                    run_id: run_id.to_string(),
                    runtime,
                };
                journal.persist_checkpoint().await;
                Ok(journal)
            }
            GraphSaveOutcome::Conflict { .. } => {
                anyhow::bail!("DeepResearch run `{run_id}` was created concurrently")
            }
        }
    }

    pub(crate) async fn open(workspace: &Path, run_id: &str) -> Result<Option<Self>> {
        validate_run_id(run_id)?;
        let store = FileGraphEventStore::new(store_root(workspace));
        let checkpoint_root = checkpoint_root(workspace);
        let Some(events) = store.load(run_id).await? else {
            return Ok(None);
        };
        let runtime = GraphRuntime::restore(events).context("strictly replay DeepResearch run")?;
        Ok(Some(Self {
            store,
            checkpoint_root,
            store_id: run_id.to_string(),
            run_id: run_id.to_string(),
            runtime,
        }))
    }

    pub(crate) fn projection(&self) -> Result<ResearchRunProjection> {
        let object = self
            .runtime
            .graph()
            .object(&run_object_id(&self.run_id))
            .context("DeepResearch graph is missing its run object")?;
        serde_json::from_value(object.data.clone()).context("decode DeepResearch run projection")
    }

    fn spec(&self) -> Result<ResearchSpec> {
        let object = self
            .runtime
            .graph()
            .object(&spec_object_id(&self.run_id))
            .context("DeepResearch graph is missing its research spec object")?;
        serde_json::from_value(object.data.clone()).context("decode DeepResearch research spec")
    }

    pub(crate) async fn append(&mut self, event: ResearchDomainEvent) -> Result<bool> {
        if self
            .runtime
            .check_external(&external_event(&self.run_id, &event))?
            .is_some()
        {
            return Ok(false);
        }
        let mut projection = self.projection()?;
        validate_transition(&projection, &event)?;
        apply_domain_event(&mut projection, &event)?;
        let object = self
            .runtime
            .graph()
            .object(&run_object_id(&self.run_id))
            .context("DeepResearch graph is missing its run object")?;
        let expected_head = graph_event_head(self.runtime.events()).map(str::to_string);
        self.runtime.project_external(
            external_event(&self.run_id, &event),
            GraphPatch::new(
                self.runtime.graph().version(),
                vec![PatchOperation::UpdateObject {
                    id: object.id.clone(),
                    expected_version: object.version,
                    data: serde_json::to_value(projection)?,
                }],
            ),
        )?;
        match self
            .store
            .save_if_head(
                &self.store_id,
                expected_head.as_deref(),
                self.runtime.events(),
            )
            .await?
        {
            GraphSaveOutcome::Saved => {
                self.persist_checkpoint().await;
                Ok(true)
            }
            GraphSaveOutcome::Conflict { actual_head } => anyhow::bail!(
                "DeepResearch run `{}` changed concurrently (actual head: {})",
                self.run_id,
                actual_head.as_deref().unwrap_or("none")
            ),
        }
    }

    async fn append_evidence(
        &mut self,
        event: ResearchDomainEvent,
        evidence: &[super::deep_research_evidence_ledger::AcceptedEvidence],
    ) -> Result<bool> {
        if self
            .runtime
            .check_external(&external_event(&self.run_id, &event))?
            .is_some()
        {
            return Ok(false);
        }
        let mut projection = self.projection()?;
        validate_transition(&projection, &event)?;
        apply_domain_event(&mut projection, &event)?;
        let run_object = self
            .runtime
            .graph()
            .object(&run_object_id(&self.run_id))
            .context("DeepResearch graph is missing its run object")?;
        let mut operations = vec![PatchOperation::UpdateObject {
            id: run_object.id.clone(),
            expected_version: run_object.version,
            data: serde_json::to_value(projection)?,
        }];
        let mut pending_objects = HashSet::new();
        for item in evidence {
            if self.runtime.graph().object(&item.id).is_none()
                && pending_objects.insert(item.id.clone())
            {
                operations.push(PatchOperation::AddObject {
                    id: item.id.clone(),
                    object_type: EVIDENCE_OBJECT_TYPE.to_string(),
                    data: serde_json::to_value(item)?,
                });
            }
            for source in &item.sources {
                if self.runtime.graph().object(&source.id).is_none()
                    && pending_objects.insert(source.id.clone())
                {
                    operations.push(PatchOperation::AddObject {
                        id: source.id.clone(),
                        object_type: SOURCE_OBJECT_TYPE.to_string(),
                        data: serde_json::to_value(source)?,
                    });
                }
                let relation_id = format!("observed-in:{}:{}", item.id, source.id);
                if self.runtime.graph().relation(&relation_id).is_none() {
                    let source_coverage = item
                        .source_coverage
                        .iter()
                        .filter(|binding| binding.source_id == source.id)
                        .collect::<Vec<_>>();
                    operations.push(PatchOperation::AddRelation {
                        id: relation_id,
                        relation_type: "deep_research.observed_in".to_string(),
                        source: item.id.clone(),
                        target: source.id.clone(),
                        data: serde_json::json!({
                            "source_coverage": source_coverage,
                        }),
                    });
                }
            }
            for claim in &item.claims {
                if self.runtime.graph().object(&claim.id).is_none()
                    && pending_objects.insert(claim.id.clone())
                {
                    operations.push(PatchOperation::AddObject {
                        id: claim.id.clone(),
                        object_type: CLAIM_OBJECT_TYPE.to_string(),
                        data: serde_json::to_value(claim)?,
                    });
                }
                let relation_id = format!("supports:{}:{}", item.id, claim.id);
                if self.runtime.graph().relation(&relation_id).is_none() {
                    operations.push(PatchOperation::AddRelation {
                        id: relation_id,
                        relation_type: "deep_research.supports".to_string(),
                        source: item.id.clone(),
                        target: claim.id.clone(),
                        data: serde_json::json!({}),
                    });
                }
            }
        }
        inquiry::append_accepted_evidence_relations(
            &self.runtime,
            &self.run_id,
            evidence,
            &mut operations,
        )?;
        let expected_head = graph_event_head(self.runtime.events()).map(str::to_string);
        self.runtime.project_external(
            external_event(&self.run_id, &event),
            GraphPatch::new(self.runtime.graph().version(), operations),
        )?;
        match self
            .store
            .save_if_head(
                &self.store_id,
                expected_head.as_deref(),
                self.runtime.events(),
            )
            .await?
        {
            GraphSaveOutcome::Saved => {
                self.persist_checkpoint().await;
                Ok(true)
            }
            GraphSaveOutcome::Conflict { actual_head } => anyhow::bail!(
                "DeepResearch run `{}` changed concurrently (actual head: {})",
                self.run_id,
                actual_head.as_deref().unwrap_or("none")
            ),
        }
    }

    async fn persist_checkpoint(&self) {
        if self.store_id != self.run_id {
            return;
        }
        let Ok(projection) = self.projection() else {
            return;
        };
        let checkpoint = ResearchCheckpoint {
            schema_version: 1,
            run_id: self.run_id.clone(),
            event_head: graph_event_head(self.runtime.events()).map(str::to_string),
            event_count: self.runtime.events().len(),
            object_count: self.runtime.graph().objects().count(),
            relation_count: self.runtime.graph().relations().count(),
            projection,
        };
        let Ok(bytes) = serde_json::to_vec(&checkpoint) else {
            return;
        };
        if bytes.len() > 1024 * 1024 {
            return;
        }
        if tokio::fs::create_dir_all(&self.checkpoint_root)
            .await
            .is_err()
        {
            return;
        }
        let path = checkpoint_path(&self.checkpoint_root, &self.run_id);
        let temp = path.with_extension(format!(
            "tmp-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|duration| duration.as_nanos())
                .unwrap_or_default()
        ));
        let result = async {
            use tokio::io::AsyncWriteExt;
            let mut file = tokio::fs::File::create(&temp).await?;
            file.write_all(&bytes).await?;
            file.sync_all().await?;
            drop(file);
            tokio::fs::rename(&temp, &path).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(temp).await;
        }
    }
}

pub(crate) async fn record_inquiry_state(
    workspace: &Path,
    run_id: &str,
    events: &[a3s::research::InquiryEvent],
    state: &a3s::research::InquiryState,
) -> Result<()> {
    inquiry::record_inquiry_state(workspace, run_id, events, state).await
}

/// Return the wall-clock origin of the durable research run.
///
/// A restarted host must continue consuming the original shared deadline
/// instead of granting the same run a fresh budget. The first graph record is
/// the creation event and its timestamp is part of the tamper-evident journal.
pub(crate) async fn load_research_run_started_at_ms(
    workspace: &Path,
    run_id: &str,
) -> Result<Option<u64>> {
    let Some(journal) = DeepResearchStateJournal::open(workspace, run_id).await? else {
        return Ok(None);
    };
    Ok(journal
        .runtime
        .events()
        .first()
        .map(|record| record.timestamp_ms))
}

pub(crate) async fn record_workflow_started(
    workspace: &Path,
    run_id: &str,
    spec: ResearchSpec,
) -> Result<()> {
    let spec = bounded_spec(spec);
    let mut journal = match DeepResearchStateJournal::open(workspace, run_id).await? {
        Some(journal) => {
            validate_reopened_spec(run_id, &journal.spec()?, &spec)?;
            journal
        }
        None => DeepResearchStateJournal::create(workspace, run_id, spec).await?,
    };
    journal
        .append(ResearchDomainEvent {
            source: "flow".to_string(),
            source_sequence: 1,
            source_event_id: format!("{run_id}:workflow-started"),
            name: "research.track.started".to_string(),
            payload: serde_json::json!({"step_id": "evidence-collection"}),
        })
        .await?;
    Ok(())
}

fn validate_reopened_spec(
    run_id: &str,
    persisted: &ResearchSpec,
    requested: &ResearchSpec,
) -> Result<()> {
    let mut changed = Vec::new();
    if persisted.query != requested.query {
        changed.push("query");
    }
    if persisted.current_date != requested.current_date {
        changed.push("current_date");
    }
    if persisted.evidence_scope != requested.evidence_scope {
        changed.push("evidence_scope");
    }
    if persisted.required_claims != requested.required_claims {
        changed.push("required_claims");
    }
    if persisted.total_budget_ms != requested.total_budget_ms {
        changed.push("total_budget_ms");
    }
    if persisted.retrieval_stage_budget_ms != requested.retrieval_stage_budget_ms {
        changed.push("retrieval_stage_budget_ms");
    }
    if persisted.question_review_stage_budget_ms != requested.question_review_stage_budget_ms {
        changed.push("question_review_stage_budget_ms");
    }
    if persisted.finalization_reserve_ms != requested.finalization_reserve_ms {
        changed.push("finalization_reserve_ms");
    }
    if changed.is_empty() {
        return Ok(());
    }
    anyhow::bail!(
        "DeepResearch run `{run_id}` cannot be reopened with a different ResearchSpec identity (changed: {})",
        changed.join(", ")
    )
}

pub(crate) async fn record_workflow_completed(
    workspace: &Path,
    run_id: &str,
    succeeded: bool,
) -> Result<()> {
    append_event_with_retry(
        workspace,
        run_id,
        ResearchDomainEvent {
            source: "flow".to_string(),
            source_sequence: 2,
            source_event_id: format!("{run_id}:workflow-completed"),
            name: "research.track.completed".to_string(),
            payload: serde_json::json!({
                "step_id": "evidence-collection",
                "succeeded": succeeded,
            }),
        },
    )
    .await?;
    Ok(())
}

pub(crate) async fn record_run_terminal(
    workspace: &Path,
    run_id: &str,
    outcome: ResearchOutcome,
    artifacts: Option<&super::ResearchReportArtifacts>,
) -> Result<ResearchRunProjection> {
    record_run_terminal_with_artifact_authority(
        workspace,
        run_id,
        outcome,
        artifacts,
        TerminalArtifactAuthority::JournalEvidence,
        None,
    )
    .await
}

/// Record a terminal artifact pair after the Host publication validator has
/// already established its path, content shape, and outcome. Evidence-first
/// reports do not create the legacy accepted-evidence graph and must not be
/// reclassified by an audit whose inputs belong to that older representation.
pub(crate) async fn record_validated_publication_terminal(
    workspace: &Path,
    run_id: &str,
    outcome: ResearchOutcome,
    artifacts: &super::ResearchReportArtifacts,
    quality: &super::deep_research_artifacts::DeepResearchPublicationQuality,
) -> Result<ResearchRunProjection> {
    if matches!(
        outcome,
        ResearchOutcome::Completed | ResearchOutcome::Qualified
    ) && (quality.accepted_claim_count == 0
        || quality.cited_source_count == 0
        || quality.direct_answer_count == 0)
    {
        anyhow::bail!(
            "a successful validated publication must contain cited claims and a direct answer"
        );
    }
    if quality.source_count > 0 {
        let accepted_evidence_count = if quality.accepted_claim_count > 0 {
            quality.cited_source_count
        } else {
            quality.relevant_source_count
        };
        append_event_with_retry(
            workspace,
            run_id,
            ResearchDomainEvent {
                source: "publication".to_string(),
                source_sequence: 1,
                source_event_id: format!("{run_id}:publication:quality"),
                name: "research.evidence.accepted".to_string(),
                payload: serde_json::json!({
                    "accepted_evidence": accepted_evidence_count,
                    "sources": quality.source_count,
                    "claims": quality.accepted_claim_count,
                    "relations": quality.accepted_relation_count,
                    "derivations": quality.accepted_derivation_count,
                    "basis_edges": quality.accepted_basis_edge_count,
                    "gaps": quality.accepted_gap_count,
                }),
            },
        )
        .await?;
    }
    record_run_terminal_with_artifact_authority(
        workspace,
        run_id,
        outcome,
        Some(artifacts),
        TerminalArtifactAuthority::ValidatedHostPublication,
        Some(quality),
    )
    .await
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TerminalArtifactAuthority {
    JournalEvidence,
    ValidatedHostPublication,
}

async fn record_run_terminal_with_artifact_authority(
    workspace: &Path,
    run_id: &str,
    outcome: ResearchOutcome,
    artifacts: Option<&super::ResearchReportArtifacts>,
    artifact_authority: TerminalArtifactAuthority,
    validated_quality: Option<&super::deep_research_artifacts::DeepResearchPublicationQuality>,
) -> Result<ResearchRunProjection> {
    if outcome == ResearchOutcome::Active {
        anyhow::bail!("cannot record an active DeepResearch run as terminal");
    }
    let mut effective_outcome = outcome;
    let mut tui_sequence = 1;
    if let Some(artifacts) = artifacts {
        let markdown = tokio::fs::read(&artifacts.markdown)
            .await
            .with_context(|| format!("read `{}`", artifacts.markdown.display()))?;
        let html = tokio::fs::read(&artifacts.html)
            .await
            .with_context(|| format!("read `{}`", artifacts.html.display()))?;
        let audit = if artifact_authority == TerminalArtifactAuthority::JournalEvidence
            && matches!(
                outcome,
                ResearchOutcome::Completed | ResearchOutcome::Qualified
            ) {
            let Some(journal) = DeepResearchStateJournal::open(workspace, run_id).await? else {
                anyhow::bail!("DeepResearch run `{run_id}` has no state journal");
            };
            let sources = journal
                .runtime
                .graph()
                .objects()
                .filter(|object| object.object_type == SOURCE_OBJECT_TYPE)
                .filter_map(|object| {
                    serde_json::from_value::<super::deep_research_evidence_ledger::AcceptedSource>(
                        object.data.clone(),
                    )
                    .ok()
                    .map(|source| {
                        super::deep_research_report_audit::ReportSourceReference {
                            source_id: source.id,
                            anchor: source.anchor,
                        }
                    })
                })
                .collect::<Vec<_>>();
            Some(super::deep_research_report_audit::audit_report(
                &String::from_utf8_lossy(&markdown),
                &String::from_utf8_lossy(&html),
                &sources,
                super::deep_research_report_audit::CitationRequirement::AtLeastOne,
            ))
        } else {
            validated_quality
                .filter(|quality| quality.accepted_claim_count > 0)
                .map(|quality| super::deep_research_report_audit::ReportAudit {
                passed: true,
                accepted_sources: quality.source_count,
                cited_sources: quality.cited_source_count,
                issues: Vec::new(),
                reason: "host publication passed the query-scoped answer, depth, citation, and source-relevance quality gate"
                    .to_string(),
                })
        };
        if let Some(audit) = audit.as_ref() {
            append_event_with_retry(
                workspace,
                run_id,
                ResearchDomainEvent {
                    source: "tui".to_string(),
                    source_sequence: tui_sequence,
                    source_event_id: format!("{run_id}:report-audited"),
                    name: "research.report.audited".to_string(),
                    payload: serde_json::to_value(audit)?,
                },
            )
            .await?;
            tui_sequence = tui_sequence.saturating_add(1);
            if !audit.passed {
                effective_outcome = ResearchOutcome::Degraded;
            }
        }
        if audit.as_ref().is_none_or(|audit| audit.passed) {
            let mut digest = Sha256::new();
            digest.update(&markdown);
            digest.update(&html);
            append_event_with_retry(
                workspace,
                run_id,
                ResearchDomainEvent {
                    source: "tui".to_string(),
                    source_sequence: tui_sequence,
                    source_event_id: format!("{run_id}:report-materialized"),
                    name: "research.report.materialized".to_string(),
                    payload: serde_json::json!({
                        "markdown": artifacts.markdown,
                        "html": artifacts.html,
                        "evidence_head": format!("{:x}", digest.finalize()),
                    }),
                },
            )
            .await?;
            tui_sequence = tui_sequence.saturating_add(1);
        }
    }
    let projection = append_event_with_retry(
        workspace,
        run_id,
        ResearchDomainEvent {
            source: "tui".to_string(),
            source_sequence: tui_sequence,
            source_event_id: format!("{run_id}:terminal:{effective_outcome:?}"),
            name: format!("research.run.{}", outcome_name(effective_outcome)),
            payload: serde_json::json!({"outcome": effective_outcome}),
        },
    )
    .await?;
    Ok(projection)
}

fn outcome_name(outcome: ResearchOutcome) -> &'static str {
    match outcome {
        ResearchOutcome::Active => "active",
        ResearchOutcome::Failed => "failed",
        ResearchOutcome::Degraded => "degraded",
        ResearchOutcome::Qualified => "qualified",
        ResearchOutcome::Completed => "completed",
    }
}

pub(crate) async fn record_child_event(
    workspace: &Path,
    run_id: &str,
    source_sequence: u64,
    task_id: &str,
    started: bool,
    payload: serde_json::Value,
) -> Result<ResearchRunProjection> {
    let lifecycle = if started { "started" } else { "completed" };
    append_event_with_retry(
        workspace,
        run_id,
        ResearchDomainEvent {
            source: "agent".to_string(),
            source_sequence,
            source_event_id: format!("{run_id}:child:{task_id}:{lifecycle}"),
            name: format!("research.child.{lifecycle}"),
            payload: bounded_event_payload(payload, 0),
        },
    )
    .await
}

#[cfg(test)]
pub(crate) async fn record_convergence(
    workspace: &Path,
    run_id: &str,
    decision: &super::deep_research_convergence::ConvergenceDecision,
) -> Result<ResearchRunProjection> {
    append_event_with_retry(
        workspace,
        run_id,
        ResearchDomainEvent {
            source: "convergence".to_string(),
            source_sequence: 1,
            source_event_id: format!("{run_id}:convergence:1"),
            name: "research.convergence.evaluated".to_string(),
            payload: serde_json::to_value(decision)?,
        },
    )
    .await
}

pub(crate) async fn record_evidence_ledger(
    workspace: &Path,
    run_id: &str,
    evidence: &[super::deep_research_evidence_ledger::AcceptedEvidence],
) -> Result<ResearchRunProjection> {
    let event = ResearchDomainEvent {
        source: "evidence".to_string(),
        source_sequence: 1,
        source_event_id: format!("{run_id}:evidence:accepted"),
        name: "research.evidence.accepted".to_string(),
        payload: serde_json::json!({
            "accepted_evidence": evidence.len(),
            "sources": evidence.iter().map(|item| item.sources.len()).sum::<usize>(),
            "claims": evidence.iter().map(|item| item.claims.len()).sum::<usize>(),
        }),
    };
    const MAX_ATTEMPTS: usize = 4;
    let mut last_error = None;
    for _ in 0..MAX_ATTEMPTS {
        let Some(mut journal) = DeepResearchStateJournal::open(workspace, run_id).await? else {
            anyhow::bail!("DeepResearch run `{run_id}` has no state journal");
        };
        match journal.append_evidence(event.clone(), evidence).await {
            Ok(_) => return journal.projection(),
            Err(error) => {
                last_error = Some(error);
                tokio::task::yield_now().await;
            }
        }
    }
    let Some(last_error) = last_error else {
        anyhow::bail!("append DeepResearch evidence exhausted without a result");
    };
    Err(last_error.context("append DeepResearch evidence after concurrent-head retries"))
}

async fn append_event_with_retry(
    workspace: &Path,
    run_id: &str,
    event: ResearchDomainEvent,
) -> Result<ResearchRunProjection> {
    const MAX_ATTEMPTS: usize = 4;
    let mut last_error = None;
    for _ in 0..MAX_ATTEMPTS {
        let Some(mut journal) = DeepResearchStateJournal::open(workspace, run_id).await? else {
            anyhow::bail!("DeepResearch run `{run_id}` has no state journal");
        };
        match journal.append(event.clone()).await {
            Ok(_) => return journal.projection(),
            Err(error) => {
                last_error = Some(error);
                tokio::task::yield_now().await;
            }
        }
    }
    let Some(last_error) = last_error else {
        anyhow::bail!("append DeepResearch event exhausted without a result");
    };
    Err(last_error.context("append DeepResearch event after concurrent-head retries"))
}

fn validate_transition(run: &ResearchRunProjection, event: &ResearchDomainEvent) -> Result<()> {
    if run.outcome.is_terminal() && event.name != "research.run.outcome_upgraded" {
        anyhow::bail!(
            "terminal DeepResearch run `{}` cannot accept `{}`",
            run.run_id,
            event.name
        );
    }
    if event.name == "research.track.scheduled" && !run.can_schedule() {
        anyhow::bail!("terminal DeepResearch run cannot schedule work");
    }
    if let Some(outcome) = event_outcome(event)? {
        if outcome == ResearchOutcome::Active {
            anyhow::bail!("a terminal outcome event cannot set active state");
        }
        if run.outcome.is_terminal() && outcome.quality() < run.outcome.quality() {
            anyhow::bail!("DeepResearch outcome quality cannot decrease");
        }
    }
    Ok(())
}

fn apply_domain_event(run: &mut ResearchRunProjection, event: &ResearchDomainEvent) -> Result<()> {
    run.last_domain_event.clone_from(&event.name);
    run.last_source_sequence = event.source_sequence;
    match event.name.as_str() {
        "research.track.scheduled" | "research.track.started" => {
            if let Some(step_id) = event
                .payload
                .get("step_id")
                .and_then(|value| value.as_str())
            {
                if !run.active_steps.iter().any(|current| current == step_id) {
                    run.active_steps.push(step_id.to_string());
                }
            }
        }
        "research.track.completed" => {
            if let Some(step_id) = event
                .payload
                .get("step_id")
                .and_then(|value| value.as_str())
            {
                run.active_steps.retain(|current| current != step_id);
            }
        }
        "research.child.started" => {
            if let Some(task_id) = event
                .payload
                .get("task_id")
                .and_then(|value| value.as_str())
            {
                if !run.active_children.iter().any(|current| current == task_id) {
                    run.active_children.push(task_id.to_string());
                }
            }
        }
        "research.child.completed" => {
            if let Some(task_id) = event
                .payload
                .get("task_id")
                .and_then(|value| value.as_str())
            {
                run.active_children.retain(|current| current != task_id);
            }
        }
        "research.convergence.evaluated" => {
            // Old records may also contain round counters, source tiers, and a
            // `continue` action. They are inert compatibility data: terminal
            // replay has always projected only the stable human-readable
            // reason.
            run.convergence_reason = event
                .payload
                .get("reason")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        "research.report.materialized" => {
            run.artifact_evidence_head = event
                .payload
                .get("evidence_head")
                .and_then(|value| value.as_str())
                .map(str::to_string);
        }
        "research.report.audited" => {
            run.report_audit_reason = event
                .payload
                .get("reason")
                .and_then(|value| value.as_str())
                .map(str::to_string);
            run.report_cited_source_count = event
                .payload
                .get("cited_sources")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok());
        }
        "research.evidence.accepted" => {
            run.accepted_evidence_count = event
                .payload
                .get("accepted_evidence")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
            run.source_count = event
                .payload
                .get("sources")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
            run.claim_count = event
                .payload
                .get("claims")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
            run.accepted_relation_count = event
                .payload
                .get("relations")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
            run.accepted_derivation_count = event
                .payload
                .get("derivations")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
            run.accepted_basis_edge_count = event
                .payload
                .get("basis_edges")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
            run.accepted_gap_count = event
                .payload
                .get("gaps")
                .and_then(serde_json::Value::as_u64)
                .and_then(|value| usize::try_from(value).ok())
                .unwrap_or_default();
        }
        _ => {}
    }
    if let Some(outcome) = event_outcome(event)? {
        run.outcome = outcome;
        run.active_steps.clear();
        run.active_children.clear();
    }
    Ok(())
}

fn event_outcome(event: &ResearchDomainEvent) -> Result<Option<ResearchOutcome>> {
    if !matches!(
        event.name.as_str(),
        "research.run.completed"
            | "research.run.qualified"
            | "research.run.degraded"
            | "research.run.failed"
            | "research.run.outcome_upgraded"
    ) {
        return Ok(None);
    }
    let value = event.payload.get("outcome").cloned().unwrap_or_else(|| {
        serde_json::Value::String(event.name.trim_start_matches("research.run.").to_string())
    });
    serde_json::from_value(value)
        .context("decode DeepResearch terminal outcome")
        .map(Some)
}

fn external_event(run_id: &str, event: &ResearchDomainEvent) -> ExternalEvent {
    ExternalEvent {
        source: event.source.clone(),
        stream_id: run_id.to_string(),
        sequence: event.source_sequence,
        event_id: event.source_event_id.clone(),
        name: event.name.clone(),
        payload: event.payload.clone(),
    }
}

fn store_root(workspace: &Path) -> PathBuf {
    workspace.join(".a3s/research/runs/events")
}

fn checkpoint_root(workspace: &Path) -> PathBuf {
    workspace.join(".a3s/research/runs/checkpoints")
}

fn checkpoint_path(root: &Path, run_id: &str) -> PathBuf {
    let mut digest = Sha256::new();
    digest.update(run_id.as_bytes());
    root.join(format!("{:x}.json", digest.finalize()))
}

fn run_object_id(run_id: &str) -> String {
    format!("research-run:{run_id}")
}

fn spec_object_id(run_id: &str) -> String {
    format!("research-spec:{run_id}")
}

fn validate_run_id(run_id: &str) -> Result<()> {
    if run_id.is_empty()
        || run_id.len() > 128
        || !run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        anyhow::bail!("invalid DeepResearch run id");
    }
    Ok(())
}

fn bounded_spec(mut spec: ResearchSpec) -> ResearchSpec {
    spec.query = bounded_string(&spec.query, 16_000);
    spec.current_date = bounded_string(&spec.current_date, 64);
    spec.evidence_scope = bounded_string(&spec.evidence_scope, 64);
    spec.required_claims = spec
        .required_claims
        .into_iter()
        .take(64)
        .map(|claim| bounded_string(&claim, 2_000))
        .collect();
    spec
}

fn bounded_event_payload(value: serde_json::Value, depth: usize) -> serde_json::Value {
    if depth >= 8 {
        return serde_json::Value::String("[nested payload omitted]".to_string());
    }
    match value {
        serde_json::Value::String(text) => serde_json::Value::String(bounded_string(&text, 4_000)),
        serde_json::Value::Array(items) => serde_json::Value::Array(
            items
                .into_iter()
                .take(64)
                .map(|item| bounded_event_payload(item, depth + 1))
                .collect(),
        ),
        serde_json::Value::Object(map) => serde_json::Value::Object(
            map.into_iter()
                .take(64)
                .map(|(key, value)| {
                    (
                        bounded_string(&key, 128),
                        bounded_event_payload(value, depth + 1),
                    )
                })
                .collect(),
        ),
        scalar => scalar,
    }
}

fn bounded_string(value: &str, max_chars: usize) -> String {
    if value.chars().count() <= max_chars {
        return value.to_string();
    }
    value.chars().take(max_chars).collect::<String>() + "…"
}

#[path = "state_journal_diagnostics.rs"]
mod diagnostics;
#[path = "state_journal/inquiry.rs"]
mod inquiry;
#[cfg(test)]
use diagnostics::{fork_with_validated_evidence, load_latest_checkpoint};
pub(crate) use diagnostics::{
    reconcile_interrupted_latest_run, research_diagnostic, research_diff, ResearchDiagnosticKind,
    ResearchRecoveryDisposition,
};
#[cfg(test)]
#[path = "state_journal_tests.rs"]
mod tests;
