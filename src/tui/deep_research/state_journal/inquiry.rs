//! Projection of the typed inquiry event stream into the DeepResearch state graph.

use super::{
    checkpoint_path, run_object_id, DeepResearchStateJournal, ResearchCheckpoint,
    CLAIM_OBJECT_TYPE, EVIDENCE_OBJECT_TYPE, SOURCE_OBJECT_TYPE,
};
use a3s_code_core::state_graph::{
    graph_event_head, ExternalEvent, GraphEvent, GraphEventStore, GraphPatch, GraphRuntime,
    GraphSaveOutcome, PatchOperation,
};
use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use std::path::Path;

/// Historical graph object type retained only when replaying an old Inquiry.
pub(super) const PERSPECTIVE_OBJECT_TYPE: &str = "deep_research.perspective";
pub(super) const QUESTION_OBJECT_TYPE: &str = "deep_research.question";
pub(super) const OBLIGATION_OBJECT_TYPE: &str = "deep_research.obligation";
pub(super) const STOP_CONDITION_OBJECT_TYPE: &str = "deep_research.stop_condition";
pub(super) const OUTLINE_SECTION_OBJECT_TYPE: &str = "deep_research.outline_section";
pub(super) const SECTION_DRAFT_OBJECT_TYPE: &str = "deep_research.section_draft";
const INQUIRY_EVENT_SOURCE: &str = "deep_research.inquiry";

impl DeepResearchStateJournal {
    async fn append_inquiry_replay(
        &mut self,
        events: &[a3s::research::InquiryEvent],
        limits: &a3s::research::InquiryLimits,
    ) -> Result<bool> {
        let persisted_sequence = self
            .runtime
            .events()
            .iter()
            .filter_map(|record| match &record.event {
                GraphEvent::ExternalEventObserved {
                    source,
                    stream_id,
                    sequence,
                    ..
                } if source == INQUIRY_EVENT_SOURCE && stream_id == &self.run_id => Some(*sequence),
                _ => None,
            })
            .max()
            .unwrap_or_default();
        let incoming_sequence = u64::try_from(events.len()).context("count inquiry events")?;
        if persisted_sequence > incoming_sequence {
            anyhow::bail!(
                "DeepResearch inquiry replay for `{}` is stale: journal has {persisted_sequence} events, input has {incoming_sequence}",
                self.run_id
            );
        }

        let expected_head = graph_event_head(self.runtime.events()).map(str::to_string);
        let mut projection = a3s::research::InquiryState::default();
        let mut changed = false;
        for (index, event) in events.iter().enumerate() {
            projection
                .apply(event, limits)
                .with_context(|| format!("replay inquiry event {index}"))?;
            let external = inquiry_external_event(&self.run_id, index, event)?;
            if self.runtime.check_external(&external)?.is_some() {
                continue;
            }
            let operations = projection_operations(&self.runtime, &self.run_id, &projection)?;
            self.runtime.project_external(
                external,
                GraphPatch::new(self.runtime.graph().version(), operations),
            )?;
            changed = true;
        }

        if !changed {
            self.persist_checkpoint_required().await?;
            return Ok(false);
        }
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
                self.persist_checkpoint_required().await?;
                Ok(true)
            }
            GraphSaveOutcome::Conflict { actual_head } => anyhow::bail!(
                "DeepResearch run `{}` changed concurrently (actual head: {})",
                self.run_id,
                actual_head.as_deref().unwrap_or("none")
            ),
        }
    }

    async fn persist_checkpoint_required(&self) -> Result<()> {
        self.persist_checkpoint().await;
        let path = checkpoint_path(&self.checkpoint_root, &self.run_id);
        let bytes = tokio::fs::read(&path)
            .await
            .with_context(|| format!("read DeepResearch checkpoint `{}`", path.display()))?;
        let checkpoint: ResearchCheckpoint =
            serde_json::from_slice(&bytes).context("decode DeepResearch checkpoint")?;
        if checkpoint.event_head.as_deref() != graph_event_head(self.runtime.events()) {
            anyhow::bail!("DeepResearch checkpoint does not match the inquiry event head");
        }
        Ok(())
    }
}

#[cfg(test)]
pub(super) async fn load_inquiry_state(
    workspace: &Path,
    run_id: &str,
) -> Result<
    Option<(
        Vec<a3s::research::InquiryEvent>,
        a3s::research::InquiryState,
    )>,
> {
    let Some(journal) = DeepResearchStateJournal::open(workspace, run_id).await? else {
        return Ok(None);
    };
    decode_inquiry_state(run_id, journal.runtime.events())
}

#[cfg(test)]
fn decode_inquiry_state(
    run_id: &str,
    records: &[a3s_code_core::state_graph::GraphEventRecord],
) -> Result<
    Option<(
        Vec<a3s::research::InquiryEvent>,
        a3s::research::InquiryState,
    )>,
> {
    let mut events = Vec::new();
    for record in records {
        let GraphEvent::ExternalEventObserved {
            source,
            stream_id,
            sequence,
            event_id,
            name,
            payload,
        } = &record.event
        else {
            continue;
        };
        if source != INQUIRY_EVENT_SOURCE || stream_id != run_id {
            continue;
        }

        let expected_sequence = u64::try_from(events.len())
            .context("count restored inquiry events")?
            .saturating_add(1);
        if *sequence != expected_sequence {
            anyhow::bail!(
                "DeepResearch inquiry stream `{run_id}` is not contiguous: expected sequence {expected_sequence}, found {sequence}"
            );
        }
        let event = serde_json::from_value::<a3s::research::InquiryEvent>(payload.clone())
            .with_context(|| {
                format!("decode DeepResearch inquiry event `{run_id}` sequence {expected_sequence}")
            })?;
        let expected = inquiry_external_event(run_id, events.len(), &event)?;
        if event_id != &expected.event_id || name != &expected.name {
            anyhow::bail!(
                "DeepResearch inquiry stream `{run_id}` sequence {expected_sequence} has inconsistent event identity"
            );
        }
        events.push(event);
    }
    if events.is_empty() {
        return Ok(None);
    }
    let state = a3s::research::replay(&events, &a3s::research::InquiryLimits::default())
        .context("strictly replay restored DeepResearch inquiry events")?;
    Ok(Some((events, state)))
}

pub(super) async fn record_inquiry_state(
    workspace: &Path,
    run_id: &str,
    events: &[a3s::research::InquiryEvent],
    state: &a3s::research::InquiryState,
) -> Result<()> {
    let limits = a3s::research::InquiryLimits::default();
    let replayed = a3s::research::replay(events, &limits)
        .context("strictly replay DeepResearch inquiry events before journaling")?;
    if replayed != *state {
        anyhow::bail!(
            "DeepResearch inquiry state does not equal the strict replay of its event stream"
        );
    }

    const MAX_ATTEMPTS: usize = 4;
    let mut last_error = None;
    for _ in 0..MAX_ATTEMPTS {
        let Some(mut journal) = DeepResearchStateJournal::open(workspace, run_id).await? else {
            anyhow::bail!("DeepResearch run `{run_id}` has no state journal");
        };
        match journal.append_inquiry_replay(events, &limits).await {
            Ok(_) => return Ok(()),
            Err(error) => {
                last_error = Some(error);
                tokio::task::yield_now().await;
            }
        }
    }
    let Some(last_error) = last_error else {
        anyhow::bail!("record inquiry state exhausted without a result");
    };
    Err(last_error.context("record inquiry state after concurrent-head retries"))
}

fn inquiry_external_event(
    run_id: &str,
    index: usize,
    event: &a3s::research::InquiryEvent,
) -> Result<ExternalEvent> {
    let sequence = u64::try_from(index)
        .context("convert inquiry event index")?
        .saturating_add(1);
    let payload = serde_json::to_value(event)?;
    let mut digest = Sha256::new();
    digest.update(serde_json::to_vec(event)?);
    Ok(ExternalEvent {
        source: INQUIRY_EVENT_SOURCE.to_string(),
        stream_id: run_id.to_string(),
        sequence,
        event_id: format!("{run_id}:inquiry:{sequence}:{:x}", digest.finalize()),
        name: format!("research.inquiry.{}", event.name()),
        payload,
    })
}

fn projection_operations(
    runtime: &GraphRuntime,
    run_id: &str,
    state: &a3s::research::InquiryState,
) -> Result<Vec<PatchOperation>> {
    let run = run_object_id(run_id);
    if runtime.graph().object(&run).is_none() {
        anyhow::bail!("DeepResearch graph is missing its run object");
    }
    let mut operations = Vec::new();
    for obligation in &state.obligations {
        let assessment = state.contract_assessment.as_ref().and_then(|assessment| {
            assessment
                .obligations
                .iter()
                .find(|item| item.obligation_id == obligation.id)
        });
        upsert_object(
            runtime,
            &mut operations,
            object_id(run_id, "obligation", &obligation.id),
            OBLIGATION_OBJECT_TYPE,
            serde_json::json!({
                "obligation": obligation,
                "assessment": assessment,
            }),
        )?;
    }
    for (index, condition) in state.stop_conditions.iter().enumerate() {
        let assessment = state.contract_assessment.as_ref().and_then(|assessment| {
            assessment
                .stop_conditions
                .iter()
                .find(|item| item.condition_index == index)
        });
        upsert_object(
            runtime,
            &mut operations,
            object_id(
                run_id,
                "stop-condition",
                &format!("condition:{}", index + 1),
            ),
            STOP_CONDITION_OBJECT_TYPE,
            serde_json::json!({
                "condition_index": index,
                "condition": condition,
                "assessment": assessment,
            }),
        )?;
    }
    // Preserve historical perspective objects during replay. Current Inquiry
    // state always leaves this collection empty.
    for perspective in &state.perspectives {
        upsert_object(
            runtime,
            &mut operations,
            object_id(run_id, "perspective", &perspective.id),
            PERSPECTIVE_OBJECT_TYPE,
            serde_json::to_value(perspective)?,
        )?;
    }
    for obligation in &state.obligations {
        add_relation(
            runtime,
            &mut operations,
            run_id,
            "deep_research.has_obligation",
            &run,
            &object_id(run_id, "obligation", &obligation.id),
        )?;
    }
    for (index, _) in state.stop_conditions.iter().enumerate() {
        add_relation(
            runtime,
            &mut operations,
            run_id,
            "deep_research.has_stop_condition",
            &run,
            &object_id(
                run_id,
                "stop-condition",
                &format!("condition:{}", index + 1),
            ),
        )?;
    }
    for question in &state.questions {
        upsert_object(
            runtime,
            &mut operations,
            object_id(run_id, "question", &question.id),
            QUESTION_OBJECT_TYPE,
            serde_json::to_value(question)?,
        )?;
    }
    if let Some(outline) = &state.outline {
        for section in &outline.sections {
            upsert_object(
                runtime,
                &mut operations,
                object_id(run_id, "outline-section", &section.id),
                OUTLINE_SECTION_OBJECT_TYPE,
                serde_json::to_value(section)?,
            )?;
        }
    }
    for draft in state.drafts.values() {
        upsert_object(
            runtime,
            &mut operations,
            object_id(run_id, "section-draft", &draft.section_id),
            SECTION_DRAFT_OBJECT_TYPE,
            serde_json::to_value(draft)?,
        )?;
    }

    // Historical replay-only relation.
    for perspective in &state.perspectives {
        add_relation(
            runtime,
            &mut operations,
            run_id,
            "deep_research.has_perspective",
            &run,
            &object_id(run_id, "perspective", &perspective.id),
        )?;
    }
    for question in &state.questions {
        let question_id = object_id(run_id, "question", &question.id);
        add_relation(
            runtime,
            &mut operations,
            run_id,
            "deep_research.has_question",
            &run,
            &question_id,
        )?;
        if let Some(perspective_id) = &question.perspective_id {
            // Historical replay-only relation.
            add_relation(
                runtime,
                &mut operations,
                run_id,
                "deep_research.frames_question",
                &object_id(run_id, "perspective", perspective_id),
                &question_id,
            )?;
        }
        if let Some(parent_id) = &question.parent_question_id {
            // Historical replay-only relation.
            add_relation(
                runtime,
                &mut operations,
                run_id,
                "deep_research.has_follow_up",
                &object_id(run_id, "question", parent_id),
                &question_id,
            )?;
        }
        for obligation_id in &question.obligation_ids {
            add_relation(
                runtime,
                &mut operations,
                run_id,
                "deep_research.addresses_obligation",
                &question_id,
                &object_id(run_id, "obligation", obligation_id),
            )?;
        }
        for evidence_id in &question.evidence_ids {
            add_relation_to_existing_typed_object(
                runtime,
                &mut operations,
                run_id,
                "deep_research.answered_by",
                &question_id,
                evidence_id,
                EVIDENCE_OBJECT_TYPE,
            )?;
        }
    }
    if let Some(outline) = &state.outline {
        for section in &outline.sections {
            let section_id = object_id(run_id, "outline-section", &section.id);
            add_relation(
                runtime,
                &mut operations,
                run_id,
                "deep_research.has_outline_section",
                &run,
                &section_id,
            )?;
            for perspective_id in &section.perspective_ids {
                // Historical replay-only relation.
                add_relation(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.covers_perspective",
                    &section_id,
                    &object_id(run_id, "perspective", perspective_id),
                )?;
            }
            for question_id in &section.question_ids {
                add_relation(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.covers_question",
                    &section_id,
                    &object_id(run_id, "question", question_id),
                )?;
            }
            let covered_obligations = section
                .question_ids
                .iter()
                .filter_map(|question_id| state.question(question_id))
                .flat_map(|question| question.obligation_ids.iter())
                .collect::<std::collections::BTreeSet<_>>();
            for obligation_id in covered_obligations {
                add_relation(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.covers_obligation",
                    &section_id,
                    &object_id(run_id, "obligation", obligation_id),
                )?;
            }
            for claim_id in &section.claim_ids {
                add_relation_to_existing_typed_object(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.covers_claim",
                    &section_id,
                    claim_id,
                    CLAIM_OBJECT_TYPE,
                )?;
            }
            for source_id in &section.source_ids {
                add_relation_to_existing_typed_object(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.covers_source",
                    &section_id,
                    source_id,
                    SOURCE_OBJECT_TYPE,
                )?;
            }
        }
    }
    for draft in state.drafts.values() {
        let draft_id = object_id(run_id, "section-draft", &draft.section_id);
        add_relation(
            runtime,
            &mut operations,
            run_id,
            "deep_research.has_section_draft",
            &object_id(run_id, "outline-section", &draft.section_id),
            &draft_id,
        )?;
        replace_draft_citation_relations(runtime, &mut operations, &draft_id, draft, state);
        for citation_id in &draft.citation_ids {
            if state.claim_catalog.contains(citation_id) {
                add_relation_to_existing_typed_object(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.cites_claim",
                    &draft_id,
                    citation_id,
                    CLAIM_OBJECT_TYPE,
                )?;
            }
            if state.source_catalog.contains(citation_id) {
                add_relation_to_existing_typed_object(
                    runtime,
                    &mut operations,
                    run_id,
                    "deep_research.cites_source",
                    &draft_id,
                    citation_id,
                    SOURCE_OBJECT_TYPE,
                )?;
            }
        }
    }
    Ok(operations)
}

fn replace_draft_citation_relations(
    runtime: &GraphRuntime,
    operations: &mut Vec<PatchOperation>,
    draft_id: &str,
    draft: &a3s::research::SectionDraft,
    state: &a3s::research::InquiryState,
) {
    let desired_claims = draft
        .citation_ids
        .iter()
        .filter(|id| state.claim_catalog.contains(*id))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    let desired_sources = draft
        .citation_ids
        .iter()
        .filter(|id| state.source_catalog.contains(*id))
        .map(String::as_str)
        .collect::<std::collections::BTreeSet<_>>();
    for relation in runtime.graph().relations().filter(|relation| {
        relation.source == draft_id
            && match relation.relation_type.as_str() {
                "deep_research.cites_claim" => !desired_claims.contains(relation.target.as_str()),
                "deep_research.cites_source" => !desired_sources.contains(relation.target.as_str()),
                _ => false,
            }
    }) {
        operations.push(PatchOperation::RemoveRelation {
            id: relation.id.clone(),
            expected_version: relation.version,
        });
    }
}

/// Complete inquiry-to-evidence links when the normalized evidence ledger is
/// committed after the inquiry projection. The inverse write order is handled
/// directly by `projection_operations` above.
pub(super) fn append_accepted_evidence_relations(
    runtime: &GraphRuntime,
    run_id: &str,
    evidence: &[super::super::deep_research_evidence_ledger::AcceptedEvidence],
    operations: &mut Vec<PatchOperation>,
) -> Result<()> {
    let evidence_ids = evidence
        .iter()
        .map(|item| item.id.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let claim_ids = evidence
        .iter()
        .flat_map(|item| item.claims.iter().map(|claim| claim.id.as_str()))
        .collect::<std::collections::BTreeSet<_>>();
    let source_ids = evidence
        .iter()
        .flat_map(|item| item.sources.iter().map(|source| source.id.as_str()))
        .collect::<std::collections::BTreeSet<_>>();

    for object in runtime.graph().objects() {
        match object.object_type.as_str() {
            QUESTION_OBJECT_TYPE => {
                let question =
                    serde_json::from_value::<a3s::research::Question>(object.data.clone())
                        .with_context(|| format!("decode inquiry question `{}`", object.id))?;
                for evidence_id in question
                    .evidence_ids
                    .iter()
                    .filter(|id| evidence_ids.contains(id.as_str()))
                {
                    add_relation(
                        runtime,
                        operations,
                        run_id,
                        "deep_research.answered_by",
                        &object.id,
                        evidence_id,
                    )?;
                }
            }
            OUTLINE_SECTION_OBJECT_TYPE => {
                let section =
                    serde_json::from_value::<a3s::research::OutlineSection>(object.data.clone())
                        .with_context(|| {
                            format!("decode inquiry outline section `{}`", object.id)
                        })?;
                for claim_id in section
                    .claim_ids
                    .iter()
                    .filter(|id| claim_ids.contains(id.as_str()))
                {
                    add_relation(
                        runtime,
                        operations,
                        run_id,
                        "deep_research.covers_claim",
                        &object.id,
                        claim_id,
                    )?;
                }
                for source_id in section
                    .source_ids
                    .iter()
                    .filter(|id| source_ids.contains(id.as_str()))
                {
                    add_relation(
                        runtime,
                        operations,
                        run_id,
                        "deep_research.covers_source",
                        &object.id,
                        source_id,
                    )?;
                }
            }
            SECTION_DRAFT_OBJECT_TYPE => {
                let draft =
                    serde_json::from_value::<a3s::research::SectionDraft>(object.data.clone())
                        .with_context(|| format!("decode inquiry section draft `{}`", object.id))?;
                for citation_id in &draft.citation_ids {
                    if claim_ids.contains(citation_id.as_str()) {
                        add_relation(
                            runtime,
                            operations,
                            run_id,
                            "deep_research.cites_claim",
                            &object.id,
                            citation_id,
                        )?;
                    }
                    if source_ids.contains(citation_id.as_str()) {
                        add_relation(
                            runtime,
                            operations,
                            run_id,
                            "deep_research.cites_source",
                            &object.id,
                            citation_id,
                        )?;
                    }
                }
            }
            _ => {}
        }
    }
    Ok(())
}

fn add_relation_to_existing_typed_object(
    runtime: &GraphRuntime,
    operations: &mut Vec<PatchOperation>,
    run_id: &str,
    relation_type: &str,
    source: &str,
    target: &str,
    target_type: &str,
) -> Result<()> {
    let Some(object) = runtime.graph().object(target) else {
        return Ok(());
    };
    if object.object_type != target_type {
        anyhow::bail!(
            "inquiry relation target `{target}` has type `{}` instead of `{target_type}`",
            object.object_type
        );
    }
    add_relation(runtime, operations, run_id, relation_type, source, target)
}

fn upsert_object(
    runtime: &GraphRuntime,
    operations: &mut Vec<PatchOperation>,
    id: String,
    object_type: &str,
    data: serde_json::Value,
) -> Result<()> {
    match runtime.graph().object(&id) {
        Some(object) if object.object_type != object_type => {
            anyhow::bail!("inquiry object `{id}` has an incompatible type")
        }
        Some(object) if object.data != data => operations.push(PatchOperation::UpdateObject {
            id,
            expected_version: object.version,
            data,
        }),
        Some(_) => {}
        None => operations.push(PatchOperation::AddObject {
            id,
            object_type: object_type.to_string(),
            data,
        }),
    }
    Ok(())
}

fn add_relation(
    runtime: &GraphRuntime,
    operations: &mut Vec<PatchOperation>,
    run_id: &str,
    relation_type: &str,
    source: &str,
    target: &str,
) -> Result<()> {
    let id = relation_id(run_id, relation_type, source, target);
    if operations.iter().any(
        |operation| matches!(operation, PatchOperation::AddRelation { id: pending, .. } if pending == &id),
    ) {
        return Ok(());
    }
    match runtime.graph().relation(&id) {
        Some(relation)
            if relation.relation_type == relation_type
                && relation.source == source
                && relation.target == target => {}
        Some(_) => anyhow::bail!("inquiry relation `{id}` has incompatible endpoints"),
        None => operations.push(PatchOperation::AddRelation {
            id,
            relation_type: relation_type.to_string(),
            source: source.to_string(),
            target: target.to_string(),
            data: serde_json::json!({}),
        }),
    }
    Ok(())
}

pub(super) fn object_id(run_id: &str, kind: &str, local_id: &str) -> String {
    let mut digest = Sha256::new();
    digest.update(run_id.as_bytes());
    digest.update([0]);
    digest.update(kind.as_bytes());
    digest.update([0]);
    digest.update(local_id.as_bytes());
    format!("inquiry:{run_id}:{kind}:{:x}", digest.finalize())
}

fn relation_id(run_id: &str, relation_type: &str, source: &str, target: &str) -> String {
    let mut digest = Sha256::new();
    for value in [run_id, relation_type, source, target] {
        digest.update(value.as_bytes());
        digest.update([0]);
    }
    format!("inquiry:{run_id}:relation:{:x}", digest.finalize())
}

#[cfg(test)]
#[path = "inquiry_tests.rs"]
mod tests;
