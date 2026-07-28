use super::super::deep_research_artifacts::{
    materialize_deep_research_acquisition_recovery_report,
    recover_deep_research_publication_receipt, DeepResearchEvidenceFirstPublication,
    ResearchReportArtifacts,
};
use super::super::deep_research_workflow_store::recover_deep_research_bootstrap_acquisition_from_store;
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ResearchDiagnosticKind {
    Status,
    Explain,
    Replay,
}

pub(crate) async fn research_diagnostic(
    workspace: &Path,
    run_id: Option<&str>,
    kind: ResearchDiagnosticKind,
) -> Result<String> {
    let (runtime, projection) = if let Some(run_id) = run_id {
        let journal = DeepResearchStateJournal::open(workspace, run_id)
            .await?
            .with_context(|| format!("DeepResearch run `{run_id}` was not found"))?;
        let projection = journal.projection()?;
        (journal.runtime, projection)
    } else {
        load_latest_journal(workspace)
            .await?
            .context("no DeepResearch event journal was found")?
    };
    let head = graph_event_head(runtime.events()).unwrap_or("none");
    let cited_sources = projection
        .report_cited_source_count
        .map(|count| count.to_string())
        .unwrap_or_else(|| "not audited".to_string());
    let common = format!(
        "DeepResearch run {}\noutcome: {}\nevidence: {} accepted · {} sources · {} claims\ntyped graph: {} relations · {} derivations · {} basis edges · {} gaps\nactive: {} steps · {} children\ncited sources: {}",
        projection.run_id,
        outcome_name(projection.outcome),
        projection.accepted_evidence_count,
        projection.source_count,
        projection.claim_count,
        projection.accepted_relation_count,
        projection.accepted_derivation_count,
        projection.accepted_basis_edge_count,
        projection.accepted_gap_count,
        projection.active_steps.len(),
        projection.active_children.len(),
        cited_sources,
    );
    Ok(match kind {
        ResearchDiagnosticKind::Status => common,
        ResearchDiagnosticKind::Explain => format!(
            "{}\nconvergence: {}\nreport audit: {}",
            common,
            projection
                .convergence_reason
                .as_deref()
                .unwrap_or("not evaluated"),
            projection
                .report_audit_reason
                .as_deref()
                .unwrap_or("not audited"),
        ),
        ResearchDiagnosticKind::Replay => format!(
            "{}\nstrict replay: ok\nevents: {}\ngraph: {} objects · {} relations\nhead: {}",
            common,
            runtime.events().len(),
            runtime.graph().objects().count(),
            runtime.graph().relations().count(),
            head,
        ),
    })
}

pub(crate) async fn research_diff(
    workspace: &Path,
    left_run_id: &str,
    right_run_id: &str,
) -> Result<String> {
    let left = DeepResearchStateJournal::open(workspace, left_run_id)
        .await?
        .with_context(|| format!("DeepResearch run `{left_run_id}` was not found"))?;
    let right = DeepResearchStateJournal::open(workspace, right_run_id)
        .await?
        .with_context(|| format!("DeepResearch run `{right_run_id}` was not found"))?;
    let diff = left.runtime.diff(&right.runtime);
    let object_summary = summarize_object_diff(&diff);
    let relation_summary = summarize_relation_diff(&diff);
    Ok(format!(
        "DeepResearch structural diff\nleft: {} · {} events\nright: {} · {} events\nobjects: {}\nrelations: {}\nempty: {}",
        left_run_id,
        left.runtime.events().len(),
        right_run_id,
        right.runtime.events().len(),
        object_summary,
        relation_summary,
        diff.is_empty(),
    ))
}

#[cfg(test)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResearchForkSummary {
    pub(crate) branch_store_id: String,
    pub(crate) fork_sequence: u64,
    pub(crate) objects_added: usize,
    pub(crate) relations_added: usize,
}

#[cfg(test)]
pub(crate) async fn fork_with_validated_evidence(
    workspace: &Path,
    run_id: &str,
    fork_sequence: u64,
    branch_name: &str,
    evidence: &[super::super::deep_research_evidence_ledger::AcceptedEvidence],
) -> Result<ResearchForkSummary> {
    validate_run_id(run_id)?;
    validate_run_id(branch_name).context("invalid DeepResearch branch name")?;
    if evidence.is_empty() {
        anyhow::bail!("a research strategy fork requires validated evidence");
    }
    let base = DeepResearchStateJournal::open(workspace, run_id)
        .await?
        .with_context(|| format!("DeepResearch run `{run_id}` was not found"))?;
    let fork = base
        .runtime
        .fork_at(fork_sequence)
        .context("fork DeepResearch graph at a strict event boundary")?;
    let branch_store_id = format!("branch-{run_id}-{branch_name}");
    let store = FileGraphEventStore::new(store_root(workspace));
    if store.load(&branch_store_id).await?.is_some() {
        anyhow::bail!("DeepResearch branch `{branch_name}` already exists");
    }
    match store
        .save_if_head(&branch_store_id, None, fork.events())
        .await?
    {
        GraphSaveOutcome::Saved => {}
        GraphSaveOutcome::Conflict { .. } => {
            anyhow::bail!("DeepResearch branch `{branch_name}` was created concurrently")
        }
    }
    let mut branch = DeepResearchStateJournal {
        store,
        checkpoint_root: checkpoint_root(workspace),
        store_id: branch_store_id.clone(),
        run_id: run_id.to_string(),
        runtime: fork,
    };
    branch
        .append_evidence(
            ResearchDomainEvent {
                source: format!("evidence_branch_{branch_name}"),
                source_sequence: 1,
                source_event_id: format!("{run_id}:branch:{branch_name}:evidence"),
                name: "research.evidence.accepted".to_string(),
                payload: serde_json::json!({
                    "branch": branch_name,
                    "accepted_evidence": evidence.len(),
                    "sources": evidence.iter().map(|item| item.sources.len()).sum::<usize>(),
                    "claims": evidence.iter().map(|item| item.claims.len()).sum::<usize>(),
                }),
            },
            evidence,
        )
        .await?;
    let diff = base.runtime.diff(&branch.runtime);
    Ok(ResearchForkSummary {
        branch_store_id,
        fork_sequence,
        objects_added: diff.objects_added.len(),
        relations_added: diff.relations_added.len(),
    })
}

fn summarize_object_diff(diff: &a3s_code_core::state_graph::GraphDiff) -> String {
    let mut counts = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
    for object in &diff.objects_added {
        counts.entry(object.object_type.clone()).or_default().0 += 1;
    }
    for object in &diff.objects_removed {
        counts.entry(object.object_type.clone()).or_default().1 += 1;
    }
    for (left, _) in &diff.objects_changed {
        counts.entry(left.object_type.clone()).or_default().2 += 1;
    }
    if counts.is_empty() {
        return "no changes".to_string();
    }
    counts
        .into_iter()
        .map(|(kind, (added, removed, changed))| format!("{kind} +{added}/-{removed}/~{changed}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

fn summarize_relation_diff(diff: &a3s_code_core::state_graph::GraphDiff) -> String {
    let mut counts = std::collections::BTreeMap::<String, (usize, usize, usize)>::new();
    for relation in &diff.relations_added {
        counts.entry(relation.relation_type.clone()).or_default().0 += 1;
    }
    for relation in &diff.relations_removed {
        counts.entry(relation.relation_type.clone()).or_default().1 += 1;
    }
    for (left, _) in &diff.relations_changed {
        counts.entry(left.relation_type.clone()).or_default().2 += 1;
    }
    if counts.is_empty() {
        return "no changes".to_string();
    }
    counts
        .into_iter()
        .map(|(kind, (added, removed, changed))| format!("{kind} +{added}/-{removed}/~{changed}"))
        .collect::<Vec<_>>()
        .join(" · ")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ResearchRecoveryDisposition {
    PublicationPreserved {
        artifacts: ResearchReportArtifacts,
        outcome: ResearchOutcome,
    },
    AcquisitionPreserved {
        artifacts: ResearchReportArtifacts,
    },
    FailedWithoutRecoverableAcquisition,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ResearchRecoverySummary {
    pub(crate) run_id: String,
    pub(crate) cancel_children: Vec<String>,
    pub(crate) orphaned_children: Vec<String>,
    pub(crate) disposition: ResearchRecoveryDisposition,
}

pub(crate) async fn reconcile_interrupted_latest_run(
    workspace: &Path,
    running_tracker_children: &HashSet<String>,
) -> Result<Option<ResearchRecoverySummary>> {
    let Some((runtime, projection)) = load_latest_journal(workspace).await? else {
        return Ok(None);
    };
    if projection.outcome.is_terminal() {
        return Ok(None);
    }
    let persisted_spec = runtime
        .graph()
        .object(&spec_object_id(&projection.run_id))
        .and_then(|object| serde_json::from_value::<ResearchSpec>(object.data.clone()).ok());
    let host_pid = persisted_spec
        .as_ref()
        .map(|spec| spec.host_pid)
        .unwrap_or_default();
    if host_pid > 0 && process_is_alive(host_pid) {
        return Ok(None);
    }
    let mut cancel_children = projection
        .active_children
        .iter()
        .filter(|task_id| running_tracker_children.contains(*task_id))
        .cloned()
        .collect::<Vec<_>>();
    let mut orphaned_children = projection
        .active_children
        .iter()
        .filter(|task_id| !running_tracker_children.contains(*task_id))
        .cloned()
        .collect::<Vec<_>>();
    cancel_children.sort();
    orphaned_children.sort();
    let run_id = projection.run_id;
    let recovered_publication = persisted_spec
        .as_ref()
        .map(|spec| recover_deep_research_publication_receipt(workspace, &spec.query, &run_id))
        .transpose()
        .map_err(anyhow::Error::msg)?
        .flatten();
    let preserved_acquisition = if recovered_publication.is_none() {
        persisted_spec
            .as_ref()
            .map(|spec| preserve_interrupted_bootstrap_acquisition(workspace, &run_id, &spec.query))
            .transpose()?
            .flatten()
    } else {
        None
    };
    let publication_preserved = recovered_publication.is_some();
    let bootstrap_acquisition_preserved = preserved_acquisition.is_some();
    append_event_with_retry(
        workspace,
        &run_id,
        ResearchDomainEvent {
            source: "recovery".to_string(),
            source_sequence: 1,
            source_event_id: format!("{run_id}:recovery:reconciled"),
            name: "research.recovery.reconciled".to_string(),
            payload: serde_json::json!({
                "cancelled_children": cancel_children,
                "orphaned_children": orphaned_children,
                "reason": "host restarted without a valid parent operation lease",
                "publication_preserved": publication_preserved,
                "bootstrap_acquisition_preserved": bootstrap_acquisition_preserved,
            }),
        },
    )
    .await?;
    let disposition = if let Some(publication) = recovered_publication {
        let outcome = match publication.publication {
            DeepResearchEvidenceFirstPublication::Synthesized => ResearchOutcome::Completed,
            DeepResearchEvidenceFirstPublication::Qualified => ResearchOutcome::Qualified,
            DeepResearchEvidenceFirstPublication::SourceBacked
            | DeepResearchEvidenceFirstPublication::NoEvidence => ResearchOutcome::Degraded,
        };
        record_validated_publication_terminal(
            workspace,
            &run_id,
            outcome,
            &publication.artifacts,
            &publication.quality,
        )
        .await?;
        ResearchRecoveryDisposition::PublicationPreserved {
            artifacts: publication.artifacts,
            outcome,
        }
    } else if let Some(artifacts) = preserved_acquisition {
        record_run_terminal(
            workspace,
            &run_id,
            ResearchOutcome::Degraded,
            Some(&artifacts),
        )
        .await?;
        ResearchRecoveryDisposition::AcquisitionPreserved { artifacts }
    } else {
        record_run_terminal(workspace, &run_id, ResearchOutcome::Failed, None).await?;
        ResearchRecoveryDisposition::FailedWithoutRecoverableAcquisition
    };
    Ok(Some(ResearchRecoverySummary {
        run_id,
        cancel_children,
        orphaned_children,
        disposition,
    }))
}

fn preserve_interrupted_bootstrap_acquisition(
    workspace: &Path,
    root_run_id: &str,
    query: &str,
) -> Result<Option<ResearchReportArtifacts>> {
    let bootstrap_run_id = format!("{root_run_id}-bootstrap");
    let args = serde_json::json!({
        "run_id": bootstrap_run_id,
        "input": {
            "query": query,
        },
    });
    let Some(recovered) = recover_deep_research_bootstrap_acquisition_from_store(workspace, &args)
    else {
        return Ok(None);
    };
    let Some(output) = recovered.output.as_deref() else {
        return Ok(None);
    };
    materialize_deep_research_acquisition_recovery_report(
        workspace,
        query,
        root_run_id,
        output,
        Some(&recovered.metadata),
    )
    .map_err(anyhow::Error::msg)
}

fn process_is_alive(pid: u32) -> bool {
    if pid == std::process::id() {
        return true;
    }
    #[cfg(unix)]
    {
        std::process::Command::new("kill")
            .arg("-0")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .is_ok_and(|status| status.success())
    }
    #[cfg(not(unix))]
    {
        false
    }
}

async fn load_latest_journal(
    workspace: &Path,
) -> Result<Option<(GraphRuntime, ResearchRunProjection)>> {
    if let Some(checkpoint) = load_latest_checkpoint(workspace).await {
        if let Some(journal) = DeepResearchStateJournal::open(workspace, &checkpoint.run_id).await?
        {
            let head = graph_event_head(journal.runtime.events()).map(str::to_string);
            if head == checkpoint.event_head
                && journal.runtime.events().len() == checkpoint.event_count
                && journal.projection()? == checkpoint.projection
            {
                return Ok(Some((journal.runtime, checkpoint.projection)));
            }
        }
    }
    let root = store_root(workspace);
    let mut entries = match tokio::fs::read_dir(&root).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("read DeepResearch journal root `{}`", root.display()))
        }
    };
    let mut latest: Option<(std::time::SystemTime, PathBuf)> = None;
    while let Some(entry) = entries.next_entry().await? {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let metadata = entry.metadata().await?;
        if !metadata.is_file() || metadata.len() > 256 * 1024 * 1024 {
            continue;
        }
        let modified = metadata.modified().unwrap_or(std::time::UNIX_EPOCH);
        if latest
            .as_ref()
            .is_none_or(|(current, _)| modified > *current)
        {
            latest = Some((modified, path));
        }
    }
    let Some((_, path)) = latest else {
        return Ok(None);
    };
    let bytes = tokio::fs::read(&path).await?;
    let events: Vec<GraphEventRecord> =
        serde_json::from_slice(&bytes).with_context(|| format!("decode `{}`", path.display()))?;
    let runtime =
        GraphRuntime::restore(events).context("strictly replay latest DeepResearch run")?;
    let object = runtime
        .graph()
        .objects()
        .find(|object| object.object_type == RUN_OBJECT_TYPE)
        .context("latest DeepResearch graph has no run projection")?;
    let projection = serde_json::from_value(object.data.clone())?;
    Ok(Some((runtime, projection)))
}

pub(super) async fn load_latest_checkpoint(workspace: &Path) -> Option<ResearchCheckpoint> {
    let root = checkpoint_root(workspace);
    let mut entries = tokio::fs::read_dir(root).await.ok()?;
    let mut candidates = Vec::new();
    while let Some(entry) = entries.next_entry().await.ok()? {
        let path = entry.path();
        if path.extension().and_then(|extension| extension.to_str()) != Some("json") {
            continue;
        }
        let metadata = entry.metadata().await.ok()?;
        if !metadata.is_file() || metadata.len() > 1024 * 1024 {
            continue;
        }
        candidates.push((metadata.modified().unwrap_or(std::time::UNIX_EPOCH), path));
    }
    candidates.sort_by(|(left, _), (right, _)| right.cmp(left));
    for (_, path) in candidates.into_iter().take(128) {
        let Ok(bytes) = tokio::fs::read(path).await else {
            continue;
        };
        let Ok(checkpoint) = serde_json::from_slice::<ResearchCheckpoint>(&bytes) else {
            continue;
        };
        if checkpoint.schema_version == 1 && validate_run_id(&checkpoint.run_id).is_ok() {
            return Some(checkpoint);
        }
    }
    None
}
