use std::collections::{BTreeSet, HashSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, Context};
use chrono::Utc;
use fs2::FileExt;
use sha2::{Digest, Sha256};

use super::model::{
    EvolutionAuditAction, EvolutionAuditEvent, EvolutionCandidate, EvolutionCatalog,
    EvolutionDescriptor, EvolutionEvidence, EvolutionKind, EvolutionOverview,
    EvolutionPolicySummary, EvolutionState, EvolutionStats, EVOLUTION_SCHEMA,
};

pub(crate) const READY_EVIDENCE: usize = 2;
pub(crate) const AUTO_MATERIALIZE_EVIDENCE: usize = 3;
pub(crate) const AUTO_MATERIALIZE_SESSIONS: usize = 2;
pub(crate) const AUTO_MATERIALIZE_CONFIDENCE: f32 = 0.88;
const MAX_CANDIDATE_EVIDENCE: usize = 200;
const MAX_CANDIDATE_INSTRUCTIONS: usize = 16;

#[derive(Debug, Clone)]
pub(crate) struct EvolutionPaths {
    pub(crate) workspace: PathBuf,
    pub(crate) root: PathBuf,
    pub(crate) state: PathBuf,
    pub(crate) history: PathBuf,
    pub(crate) recovery: PathBuf,
    pub(crate) preferences: PathBuf,
    pub(crate) skill_root: PathBuf,
    pub(crate) okf_root: PathBuf,
    lock: PathBuf,
}

impl EvolutionPaths {
    pub(crate) fn new(workspace: impl AsRef<Path>) -> Self {
        let workspace = workspace.as_ref().to_path_buf();
        let root = workspace.join(".a3s").join("evolution");
        Self {
            state: root.join("state.json"),
            history: root.join("history"),
            recovery: root.join("recovery"),
            preferences: root.join("preferences"),
            lock: root.join("state.lock"),
            skill_root: workspace.join(".a3s").join("skills"),
            okf_root: workspace.join("okf"),
            workspace,
            root,
        }
    }
}

pub(super) fn read_catalog(paths: &EvolutionPaths) -> anyhow::Result<EvolutionCatalog> {
    with_locked_catalog(paths, |catalog| Ok(catalog.clone()))
}

pub(super) fn mutate_catalog<R>(
    paths: &EvolutionPaths,
    mutation: impl FnOnce(&mut EvolutionCatalog) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("could not create {}", paths.root.display()))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&paths.lock)
        .with_context(|| format!("could not open {}", paths.lock.display()))?;
    FileExt::lock_exclusive(&lock)
        .with_context(|| format!("could not lock {}", paths.lock.display()))?;
    let mut catalog = load_catalog_unlocked(paths)?;
    let result = mutation(&mut catalog)?;
    catalog.schema = EVOLUTION_SCHEMA.to_string();
    catalog.workspace_root = paths.workspace.display().to_string();
    catalog.revision = catalog.revision.saturating_add(1);
    catalog.updated_at = Utc::now();
    write_catalog_unlocked(paths, &catalog)?;
    FileExt::unlock(&lock).ok();
    Ok(result)
}

fn with_locked_catalog<R>(
    paths: &EvolutionPaths,
    read: impl FnOnce(&EvolutionCatalog) -> anyhow::Result<R>,
) -> anyhow::Result<R> {
    fs::create_dir_all(&paths.root)
        .with_context(|| format!("could not create {}", paths.root.display()))?;
    let lock = OpenOptions::new()
        .create(true)
        .truncate(false)
        .read(true)
        .write(true)
        .open(&paths.lock)
        .with_context(|| format!("could not open {}", paths.lock.display()))?;
    FileExt::lock_exclusive(&lock)
        .with_context(|| format!("could not lock {}", paths.lock.display()))?;
    let catalog = load_catalog_unlocked(paths)?;
    let result = read(&catalog);
    FileExt::unlock(&lock).ok();
    result
}

fn load_catalog_unlocked(paths: &EvolutionPaths) -> anyhow::Result<EvolutionCatalog> {
    if !paths.state.is_file() {
        return Ok(EvolutionCatalog::empty(
            paths.workspace.display().to_string(),
        ));
    }
    let bytes = fs::read(&paths.state)
        .with_context(|| format!("could not read {}", paths.state.display()))?;
    let catalog: EvolutionCatalog = serde_json::from_slice(&bytes)
        .with_context(|| format!("invalid evolution catalog {}", paths.state.display()))?;
    if catalog.schema != EVOLUTION_SCHEMA {
        return Err(anyhow!(
            "unsupported evolution catalog schema `{}`",
            catalog.schema
        ));
    }
    Ok(catalog)
}

fn write_catalog_unlocked(
    paths: &EvolutionPaths,
    catalog: &EvolutionCatalog,
) -> anyhow::Result<()> {
    let bytes =
        serde_json::to_vec_pretty(catalog).context("could not serialize evolution state")?;
    let tmp = paths.root.join(format!(
        "state.{}.{}.tmp",
        std::process::id(),
        Utc::now().timestamp_nanos_opt().unwrap_or_default()
    ));
    {
        let mut file = OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&tmp)
            .with_context(|| format!("could not create {}", tmp.display()))?;
        file.write_all(&bytes)
            .with_context(|| format!("could not write {}", tmp.display()))?;
        file.sync_all()
            .with_context(|| format!("could not sync {}", tmp.display()))?;
    }
    if let Err(error) = fs::rename(&tmp, &paths.state) {
        let _ = fs::remove_file(&tmp);
        return Err(error).with_context(|| format!("could not replace {}", paths.state.display()));
    }
    Ok(())
}

pub(super) fn observe_batch(
    paths: &EvolutionPaths,
    observations: Vec<a3s_code_core::memory::MemoryObservation>,
) -> anyhow::Result<Vec<String>> {
    mutate_catalog(paths, |catalog| {
        let mut changed = BTreeSet::new();
        for observation in observations {
            let Some((descriptor, evidence)) = descriptor_and_evidence(paths, &observation) else {
                continue;
            };
            let candidate_id = merge_observation(catalog, descriptor, evidence)?;
            changed.insert(candidate_id);
        }
        Ok(changed.into_iter().collect())
    })
}

fn merge_observation(
    catalog: &mut EvolutionCatalog,
    descriptor: EvolutionDescriptor,
    evidence: EvolutionEvidence,
) -> anyhow::Result<String> {
    let position = catalog
        .candidates
        .iter()
        .position(|candidate| candidate_matches(candidate, &descriptor));
    let now = Utc::now();
    let position = match position {
        Some(position) => position,
        None => {
            catalog.candidates.push(EvolutionCandidate {
                id: candidate_id(descriptor.kind, &descriptor.pattern_key),
                kind: descriptor.kind,
                pattern_key: descriptor.pattern_key.clone(),
                pattern_aliases: Vec::new(),
                title: descriptor.title.clone(),
                summary: descriptor.summary.clone(),
                instructions: Vec::new(),
                state: EvolutionState::Observing,
                evidence: Vec::new(),
                occurrences: 0,
                distinct_sessions: 0,
                confidence: 0.0,
                importance: 0.0,
                maturity: 0.0,
                has_conflicts: false,
                update_available: false,
                activation_pending: false,
                created_at: now,
                updated_at: now,
                ready_at: None,
                materialized_at: None,
                rejected_at: None,
                rolled_back_at: None,
                rejection_reason: None,
                asset_path: None,
                current_version: None,
                versions: Vec::new(),
                audit: Vec::new(),
            });
            catalog
                .candidates
                .len()
                .checked_sub(1)
                .context("evolution candidate insertion did not produce a catalog row")?
        }
    };
    let candidate = catalog
        .candidates
        .get_mut(position)
        .context("evolution candidate position is outside the catalog")?;

    if descriptor.pattern_key != candidate.pattern_key
        && !candidate.pattern_aliases.contains(&descriptor.pattern_key)
    {
        candidate.pattern_aliases.push(descriptor.pattern_key);
    }
    let previous_confidence = candidate
        .evidence
        .iter()
        .map(|evidence| evidence.confidence)
        .fold(0.0_f32, f32::max);
    if candidate.evidence.is_empty() || evidence.confidence >= previous_confidence {
        candidate.title = truncate_chars(descriptor.title.trim(), 96);
        candidate.summary = truncate_chars(descriptor.summary.trim(), 360);
    }
    merge_instructions(&mut candidate.instructions, descriptor.instructions);
    if !candidate
        .evidence
        .iter()
        .any(|existing| existing.id == evidence.id)
    {
        candidate.evidence.push(evidence);
        if candidate.evidence.len() > MAX_CANDIDATE_EVIDENCE {
            let overflow = candidate.evidence.len() - MAX_CANDIDATE_EVIDENCE;
            candidate.evidence.drain(..overflow);
        }
    }
    refresh_candidate(candidate, now);
    Ok(candidate.id.clone())
}

fn merge_instructions(current: &mut Vec<String>, incoming: Vec<String>) {
    for instruction in incoming {
        let instruction = truncate_chars(instruction.trim(), 320);
        if instruction.chars().count() < 8 {
            continue;
        }
        let normalized = normalize_text(&instruction);
        if current
            .iter()
            .any(|existing| normalize_text(existing) == normalized)
        {
            continue;
        }
        current.push(instruction);
        if current.len() >= MAX_CANDIDATE_INSTRUCTIONS {
            break;
        }
    }
}

fn refresh_candidate(candidate: &mut EvolutionCandidate, now: chrono::DateTime<Utc>) {
    candidate.occurrences = candidate.evidence.len();
    candidate.distinct_sessions = candidate
        .evidence
        .iter()
        .filter_map(|evidence| evidence.session_id.as_deref())
        .filter(|session| !session.trim().is_empty())
        .collect::<HashSet<_>>()
        .len();
    let count = candidate.occurrences.max(1) as f32;
    candidate.confidence = candidate
        .evidence
        .iter()
        .map(|evidence| evidence.confidence)
        .sum::<f32>()
        / count;
    candidate.importance = candidate
        .evidence
        .iter()
        .map(|evidence| evidence.importance)
        .sum::<f32>()
        / count;
    candidate.has_conflicts = candidate
        .evidence
        .iter()
        .any(|evidence| !evidence.conflicts_with.is_empty());
    let recurrence = (candidate.occurrences as f32 / 3.0).min(1.0);
    let sessions = (candidate.distinct_sessions as f32 / 2.0).min(1.0);
    candidate.maturity = (candidate.confidence * 0.35
        + candidate.importance * 0.25
        + recurrence * 0.25
        + sessions * 0.15)
        .clamp(0.0, 1.0);
    candidate.updated_at = now;

    let recurrent = candidate.occurrences >= READY_EVIDENCE
        && (candidate.distinct_sessions >= 2 || candidate.occurrences >= 3);
    let explicit_preference = candidate.kind == EvolutionKind::Preference
        && candidate.occurrences >= 1
        && candidate.confidence >= 0.9
        && candidate
            .evidence
            .iter()
            .any(|evidence| evidence.explicit_signal);
    if candidate.state == EvolutionState::Observing
        && !candidate.has_conflicts
        && (recurrent || explicit_preference)
    {
        candidate.state = EvolutionState::Ready;
        candidate.ready_at = Some(now);
        candidate.audit.push(EvolutionAuditEvent {
            action: EvolutionAuditAction::Ready,
            at: now,
            version: None,
            note: Some(format!(
                "matured from {} evidence observations across {} sessions",
                candidate.occurrences, candidate.distinct_sessions
            )),
            recovery_path: None,
        });
    }

    candidate.update_available = match candidate.current_version {
        Some(version) => candidate
            .versions
            .iter()
            .find(|item| item.version == version)
            .map(|item| {
                let snapshot = item.evidence_ids.iter().collect::<HashSet<_>>();
                candidate
                    .evidence
                    .iter()
                    .any(|evidence| !snapshot.contains(&evidence.id))
            })
            .unwrap_or(true),
        None => false,
    };
}

pub(super) fn auto_materialize_eligible(candidate: &EvolutionCandidate) -> bool {
    candidate.state == EvolutionState::Ready
        && candidate.occurrences >= AUTO_MATERIALIZE_EVIDENCE
        && candidate.distinct_sessions >= AUTO_MATERIALIZE_SESSIONS
        && candidate.confidence >= AUTO_MATERIALIZE_CONFIDENCE
        && candidate.importance >= 0.78
        && !candidate.has_conflicts
        && candidate
            .evidence
            .iter()
            .filter(|evidence| evidence.explicit_signal)
            .count()
            >= 2
}

pub(super) fn reject_candidate(
    paths: &EvolutionPaths,
    id: &str,
    reason: Option<String>,
) -> anyhow::Result<EvolutionCandidate> {
    mutate_catalog(paths, |catalog| {
        let candidate = candidate_mut(catalog, id)?;
        if candidate.current_version.is_some() {
            return Err(anyhow!(
                "candidates with an active materialized version must be rolled back to baseline before rejection"
            ));
        }
        let now = Utc::now();
        let reason = reason
            .as_deref()
            .map(|value| truncate_chars(value.trim(), 240))
            .filter(|value| !value.is_empty());
        candidate.state = EvolutionState::Rejected;
        candidate.rejected_at = Some(now);
        candidate.rejection_reason = reason.clone();
        candidate.activation_pending = false;
        candidate.audit.push(EvolutionAuditEvent {
            action: EvolutionAuditAction::Rejected,
            at: now,
            version: candidate.current_version,
            note: reason,
            recovery_path: None,
        });
        Ok(candidate.clone())
    })
}

pub(super) fn reopen_candidate(
    paths: &EvolutionPaths,
    id: &str,
) -> anyhow::Result<EvolutionCandidate> {
    mutate_catalog(paths, |catalog| {
        let candidate = candidate_mut(catalog, id)?;
        if candidate.state != EvolutionState::Rejected {
            return Err(anyhow!("only rejected candidates can be reopened"));
        }
        let now = Utc::now();
        candidate.state = EvolutionState::Observing;
        candidate.rejected_at = None;
        candidate.rejection_reason = None;
        candidate.audit.push(EvolutionAuditEvent {
            action: EvolutionAuditAction::Reopened,
            at: now,
            version: candidate.current_version,
            note: None,
            recovery_path: None,
        });
        refresh_candidate(candidate, now);
        Ok(candidate.clone())
    })
}

pub(super) fn pending_session_reload_count(paths: &EvolutionPaths) -> anyhow::Result<usize> {
    Ok(read_catalog(paths)?
        .candidates
        .iter()
        .filter(|candidate| candidate.activation_pending)
        .filter(|candidate| {
            matches!(
                candidate.kind,
                EvolutionKind::Preference | EvolutionKind::Skill
            )
        })
        .count())
}

pub(super) fn mark_session_assets_activated(paths: &EvolutionPaths) -> anyhow::Result<usize> {
    mutate_catalog(paths, |catalog| {
        let now = Utc::now();
        let mut count = 0;
        for candidate in &mut catalog.candidates {
            if candidate.activation_pending
                && matches!(
                    candidate.kind,
                    EvolutionKind::Preference | EvolutionKind::Skill
                )
            {
                candidate.activation_pending = false;
                candidate.audit.push(EvolutionAuditEvent {
                    action: if candidate.current_version.is_some() {
                        EvolutionAuditAction::Activated
                    } else {
                        EvolutionAuditAction::Deactivated
                    },
                    at: now,
                    version: candidate.current_version,
                    note: Some(if candidate.current_version.is_some() {
                        "loaded by a Code session".to_string()
                    } else {
                        "removed from a Code session".to_string()
                    }),
                    recovery_path: None,
                });
                count += 1;
            }
        }
        Ok(count)
    })
}

pub(super) fn overview(paths: &EvolutionPaths) -> anyhow::Result<EvolutionOverview> {
    let mut catalog = read_catalog(paths)?;
    catalog.candidates.sort_by(|left, right| {
        state_rank(left.state)
            .cmp(&state_rank(right.state))
            .then_with(|| right.updated_at.cmp(&left.updated_at))
            .then_with(|| left.title.cmp(&right.title))
    });
    Ok(EvolutionOverview {
        schema: catalog.schema,
        revision: catalog.revision,
        root: paths.root.display().to_string(),
        workspace_root: paths.workspace.display().to_string(),
        skill_root: paths.skill_root.display().to_string(),
        okf_root: paths.okf_root.display().to_string(),
        updated_at: catalog.updated_at,
        stats: EvolutionStats::from_candidates(&catalog.candidates),
        candidates: catalog.candidates,
        policy: EvolutionPolicySummary {
            ready_evidence: READY_EVIDENCE,
            auto_materialize_evidence: AUTO_MATERIALIZE_EVIDENCE,
            auto_materialize_sessions: AUTO_MATERIALIZE_SESSIONS,
            auto_materialize_confidence: AUTO_MATERIALIZE_CONFIDENCE,
            local_only: true,
            review_supported: true,
        },
    })
}

fn state_rank(state: EvolutionState) -> u8 {
    match state {
        EvolutionState::Ready => 0,
        EvolutionState::Materialized => 1,
        EvolutionState::Observing => 2,
        EvolutionState::RolledBack => 3,
        EvolutionState::Rejected => 4,
    }
}

pub(super) fn candidate_mut<'a>(
    catalog: &'a mut EvolutionCatalog,
    id: &str,
) -> anyhow::Result<&'a mut EvolutionCandidate> {
    catalog
        .candidates
        .iter_mut()
        .find(|candidate| candidate.id == id)
        .ok_or_else(|| anyhow!("evolution candidate `{id}` was not found"))
}

fn descriptor_and_evidence(
    paths: &EvolutionPaths,
    observation: &a3s_code_core::memory::MemoryObservation,
) -> Option<(EvolutionDescriptor, EvolutionEvidence)> {
    let incoming = &observation.incoming;
    if memory_is_out_of_scope(paths, incoming) || looks_sensitive(&incoming.content) {
        return None;
    }
    let source = incoming.metadata.get("source")?.trim().to_ascii_lowercase();
    let confidence = incoming
        .metadata
        .get("confidence")
        .and_then(|value| value.parse::<f32>().ok())
        .filter(|value| value.is_finite())
        .unwrap_or(incoming.importance)
        .clamp(0.0, 1.0);
    if incoming.importance < 0.7 || confidence < 0.75 {
        return None;
    }
    let descriptor = explicit_descriptor(incoming)?;
    if descriptor
        .instructions
        .iter()
        .any(|value| looks_sensitive(value))
    {
        return None;
    }
    let id = incoming
        .metadata
        .get("last_observation_id")
        .filter(|value| !value.trim().is_empty())
        .cloned()
        .unwrap_or_else(|| {
            if observation.merged {
                format!("{}#{}", observation.stored.id, incoming.id)
            } else {
                incoming.id.clone()
            }
        });
    let conflicts_with = incoming
        .metadata
        .get("conflicts_with")
        .map(|value| {
            value
                .split(',')
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let evidence = EvolutionEvidence {
        id,
        memory_id: observation.stored.id.clone(),
        session_id: incoming
            .metadata
            .get("session_id")
            .cloned()
            .filter(|value| !value.trim().is_empty()),
        source,
        content: truncate_chars(&incoming.content, 1_200),
        reason: incoming
            .metadata
            .get("reason")
            .map(|value| truncate_chars(value, 320))
            .filter(|value| !value.trim().is_empty()),
        timestamp: incoming.timestamp,
        importance: incoming.importance,
        confidence,
        conflicts_with,
        explicit_signal: descriptor.explicit_signal,
    };
    Some((descriptor, evidence))
}

fn explicit_descriptor(item: &a3s_memory::MemoryItem) -> Option<EvolutionDescriptor> {
    if item.metadata.get("evolution_schema").map(String::as_str) != Some("a3s.evolution.signal.v1")
    {
        return None;
    }
    let kind = match item.metadata.get("evolution_kind")?.trim() {
        "preference" => EvolutionKind::Preference,
        "skill" => EvolutionKind::Skill,
        "okf" | "knowledge" => EvolutionKind::Okf,
        _ => return None,
    };
    let pattern_key = normalize_pattern(item.metadata.get("evolution_pattern")?)?;
    let title = truncate_chars(item.metadata.get("evolution_title")?.trim(), 96);
    let summary = truncate_chars(item.metadata.get("evolution_summary")?.trim(), 360);
    let instructions =
        serde_json::from_str::<Vec<String>>(item.metadata.get("evolution_instructions")?)
            .ok()?
            .into_iter()
            .map(|value| truncate_chars(value.trim(), 320))
            .filter(|value| value.chars().count() >= 8)
            .take(MAX_CANDIDATE_INSTRUCTIONS)
            .collect::<Vec<_>>();
    if title.chars().count() < 4 || summary.chars().count() < 12 || instructions.is_empty() {
        return None;
    }
    Some(EvolutionDescriptor {
        kind,
        pattern_key,
        title,
        summary,
        instructions,
        explicit_signal: true,
    })
}

fn memory_is_out_of_scope(paths: &EvolutionPaths, item: &a3s_memory::MemoryItem) -> bool {
    if item
        .metadata
        .get("scope")
        .is_some_and(|scope| scope == "user")
    {
        return false;
    }
    item.metadata
        .get("workspace")
        .filter(|workspace| !workspace.trim().is_empty())
        .is_some_and(|workspace| Path::new(workspace) != paths.workspace)
}

fn candidate_matches(candidate: &EvolutionCandidate, descriptor: &EvolutionDescriptor) -> bool {
    candidate.kind == descriptor.kind
        && (candidate.pattern_key == descriptor.pattern_key
            || candidate.pattern_aliases.contains(&descriptor.pattern_key))
}

fn candidate_id(kind: EvolutionKind, pattern_key: &str) -> String {
    format!(
        "evo-{}-{}",
        kind.label(),
        &short_hash(&format!("{}:{pattern_key}", kind.label()))[..12]
    )
}

pub(crate) fn short_hash(value: &str) -> String {
    let digest = Sha256::digest(value.as_bytes());
    digest.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub(crate) fn slugify(value: &str, fallback: &str) -> String {
    let mut words = Vec::new();
    let mut current = String::new();
    for ch in value.to_ascii_lowercase().chars() {
        if ch.is_ascii_alphanumeric() {
            current.push(ch);
        } else if !current.is_empty() {
            words.push(std::mem::take(&mut current));
        }
    }
    if !current.is_empty() {
        words.push(current);
    }
    let slug = words
        .into_iter()
        .filter(|word| !word.is_empty())
        .take(8)
        .collect::<Vec<_>>()
        .join("-");
    if slug.is_empty() {
        fallback.to_string()
    } else {
        truncate_chars(&slug, 64)
    }
}

fn normalize_pattern(value: &str) -> Option<String> {
    let segments = value
        .split(|ch: char| !ch.is_ascii_alphanumeric())
        .filter_map(normalize_segment)
        .take(12)
        .collect::<Vec<_>>();
    let pattern = segments.join(".");
    if segments.len() < 2 || pattern.len() > 96 {
        None
    } else {
        Some(pattern)
    }
}

fn normalize_segment(value: &str) -> Option<String> {
    let value = value
        .trim()
        .to_ascii_lowercase()
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(40)
        .collect::<String>();
    (!value.is_empty()).then_some(value)
}

fn normalize_text(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .to_lowercase()
}

pub(crate) fn truncate_chars(value: &str, max: usize) -> String {
    if value.chars().count() <= max {
        value.to_string()
    } else {
        value
            .chars()
            .take(max.saturating_sub(1))
            .collect::<String>()
            + "…"
    }
}

fn looks_sensitive(value: &str) -> bool {
    let lower = value.to_ascii_lowercase();
    [
        "-----begin private key-----",
        "authorization: bearer ",
        "api_key=",
        "apikey=",
        "access_token=",
        "refresh_token=",
        "client_secret=",
        "password=",
        "secret_key=",
    ]
    .iter()
    .any(|marker| lower.contains(marker))
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::memory::MemoryObservation;
    use a3s_memory::{MemoryItem, MemoryType};

    fn observation(id: &str, session: &str, pattern: &str) -> MemoryObservation {
        let item = MemoryItem::new("Run focused Rust tests after changing memory persistence.")
            .with_type(MemoryType::Procedural)
            .with_importance(0.9)
            .with_metadata("source", "workflow")
            .with_metadata("scope", "workspace")
            .with_metadata("confidence", "0.93")
            .with_metadata("session_id", session)
            .with_metadata("evolution_schema", "a3s.evolution.signal.v1")
            .with_metadata("evolution_kind", "skill")
            .with_metadata("evolution_pattern", pattern)
            .with_metadata("evolution_title", "Focused memory verification")
            .with_metadata(
                "evolution_summary",
                "Verify memory persistence with focused Rust tests after every change.",
            )
            .with_metadata(
                "evolution_instructions",
                r#"["Run the focused memory test target.","Check file-backed persistence behavior."]"#,
            );
        let mut incoming = item.clone();
        incoming.id = id.to_string();
        MemoryObservation {
            incoming: incoming.clone(),
            stored: incoming,
            merged: false,
        }
    }

    #[test]
    fn llm_pattern_aggregates_repeated_evidence_and_matures() {
        let temp = tempfile::tempdir().unwrap();
        let paths = EvolutionPaths::new(temp.path());
        observe_batch(
            &paths,
            vec![
                observation("one", "session-one", "workflow.rust.memory-tests"),
                observation("two", "session-two", "workflow.rust.memory-tests"),
            ],
        )
        .unwrap();
        let catalog = read_catalog(&paths).unwrap();
        assert_eq!(catalog.candidates.len(), 1);
        let candidate = &catalog.candidates[0];
        assert_eq!(candidate.occurrences, 2);
        assert_eq!(candidate.distinct_sessions, 2);
        assert_eq!(candidate.state, EvolutionState::Ready);
        assert_eq!(candidate.evidence.len(), 2);
    }

    #[test]
    fn similar_words_do_not_merge_distinct_llm_pattern_keys() {
        let temp = tempfile::tempdir().unwrap();
        let paths = EvolutionPaths::new(temp.path());
        observe_batch(
            &paths,
            vec![
                observation("one", "session-one", "workflow.rust.memory-tests"),
                observation("two", "session-two", "workflow.rust.memory-verification"),
            ],
        )
        .unwrap();

        assert_eq!(read_catalog(&paths).unwrap().candidates.len(), 2);
    }

    #[test]
    fn memory_without_llm_evolution_signal_is_not_promoted() {
        let temp = tempfile::tempdir().unwrap();
        let paths = EvolutionPaths::new(temp.path());
        let item = MemoryItem::new("Run focused Rust tests after changing memory persistence.")
            .with_type(MemoryType::Procedural)
            .with_importance(0.95)
            .with_tag("workflow")
            .with_tag("memory-tests")
            .with_metadata("source", "workflow")
            .with_metadata("scope", "workspace")
            .with_metadata("confidence", "0.96");
        observe_batch(
            &paths,
            vec![MemoryObservation {
                incoming: item.clone(),
                stored: item,
                merged: false,
            }],
        )
        .unwrap();

        assert!(read_catalog(&paths).unwrap().candidates.is_empty());
    }

    #[test]
    fn rejected_candidate_stays_rejected_when_new_evidence_arrives() {
        let temp = tempfile::tempdir().unwrap();
        let paths = EvolutionPaths::new(temp.path());
        observe_batch(
            &paths,
            vec![observation(
                "one",
                "session-one",
                "workflow.rust.memory-tests",
            )],
        )
        .unwrap();
        let id = read_catalog(&paths).unwrap().candidates[0].id.clone();
        reject_candidate(&paths, &id, Some("not reusable".to_string())).unwrap();
        observe_batch(
            &paths,
            vec![observation(
                "two",
                "session-two",
                "workflow.rust.memory-tests",
            )],
        )
        .unwrap();
        let candidate = read_catalog(&paths).unwrap().candidates.remove(0);
        assert_eq!(candidate.state, EvolutionState::Rejected);
        assert_eq!(candidate.occurrences, 2);
    }
}
