use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

pub(crate) const EVOLUTION_SCHEMA: &str = "a3s.code.evolution.v1";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EvolutionKind {
    Preference,
    Skill,
    Okf,
}

impl EvolutionKind {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Preference => "preference",
            Self::Skill => "skill",
            Self::Okf => "okf",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EvolutionState {
    Observing,
    Ready,
    Materialized,
    Rejected,
    RolledBack,
}

impl EvolutionState {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Observing => "observing",
            Self::Ready => "ready",
            Self::Materialized => "materialized",
            Self::Rejected => "rejected",
            Self::RolledBack => "rolledBack",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionEvidence {
    pub(crate) id: String,
    pub(crate) memory_id: String,
    pub(crate) session_id: Option<String>,
    pub(crate) source: String,
    pub(crate) content: String,
    pub(crate) reason: Option<String>,
    pub(crate) timestamp: DateTime<Utc>,
    pub(crate) importance: f32,
    pub(crate) confidence: f32,
    pub(crate) conflicts_with: Vec<String>,
    pub(crate) explicit_signal: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) enum EvolutionAuditAction {
    Ready,
    Materialized,
    Updated,
    Rejected,
    Reopened,
    RolledBack,
    Activated,
    Deactivated,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionAuditEvent {
    pub(crate) action: EvolutionAuditAction,
    pub(crate) at: DateTime<Utc>,
    pub(crate) version: Option<u32>,
    pub(crate) note: Option<String>,
    pub(crate) recovery_path: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionVersion {
    pub(crate) version: u32,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) asset_path: String,
    pub(crate) snapshot_path: String,
    pub(crate) content_hash: String,
    pub(crate) evidence_ids: Vec<String>,
    pub(crate) automatic: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionCandidate {
    pub(crate) id: String,
    pub(crate) kind: EvolutionKind,
    pub(crate) pattern_key: String,
    #[serde(default)]
    pub(crate) pattern_aliases: Vec<String>,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) instructions: Vec<String>,
    pub(crate) state: EvolutionState,
    pub(crate) evidence: Vec<EvolutionEvidence>,
    pub(crate) occurrences: usize,
    pub(crate) distinct_sessions: usize,
    pub(crate) confidence: f32,
    pub(crate) importance: f32,
    pub(crate) maturity: f32,
    pub(crate) has_conflicts: bool,
    pub(crate) update_available: bool,
    pub(crate) activation_pending: bool,
    pub(crate) created_at: DateTime<Utc>,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) ready_at: Option<DateTime<Utc>>,
    pub(crate) materialized_at: Option<DateTime<Utc>>,
    pub(crate) rejected_at: Option<DateTime<Utc>>,
    pub(crate) rolled_back_at: Option<DateTime<Utc>>,
    pub(crate) rejection_reason: Option<String>,
    pub(crate) asset_path: Option<String>,
    pub(crate) current_version: Option<u32>,
    #[serde(default)]
    pub(crate) versions: Vec<EvolutionVersion>,
    #[serde(default)]
    pub(crate) audit: Vec<EvolutionAuditEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionCatalog {
    pub(crate) schema: String,
    pub(crate) revision: u64,
    pub(crate) workspace_root: String,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) candidates: Vec<EvolutionCandidate>,
}

impl EvolutionCatalog {
    pub(crate) fn empty(workspace_root: String) -> Self {
        Self {
            schema: EVOLUTION_SCHEMA.to_string(),
            revision: 0,
            workspace_root,
            updated_at: Utc::now(),
            candidates: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionStats {
    pub(crate) total: usize,
    pub(crate) observing: usize,
    pub(crate) ready: usize,
    pub(crate) materialized: usize,
    pub(crate) rejected: usize,
    pub(crate) rolled_back: usize,
    pub(crate) update_available: usize,
    pub(crate) activation_pending: usize,
    pub(crate) by_kind: BTreeMap<String, usize>,
}

impl EvolutionStats {
    pub(crate) fn from_candidates(candidates: &[EvolutionCandidate]) -> Self {
        let mut stats = Self {
            total: candidates.len(),
            ..Self::default()
        };
        for candidate in candidates {
            *stats
                .by_kind
                .entry(candidate.kind.label().to_string())
                .or_default() += 1;
            match candidate.state {
                EvolutionState::Observing => stats.observing += 1,
                EvolutionState::Ready => stats.ready += 1,
                EvolutionState::Materialized => stats.materialized += 1,
                EvolutionState::Rejected => stats.rejected += 1,
                EvolutionState::RolledBack => stats.rolled_back += 1,
            }
            stats.update_available += usize::from(candidate.update_available);
            stats.activation_pending += usize::from(candidate.activation_pending);
        }
        stats
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionOverview {
    pub(crate) schema: String,
    pub(crate) revision: u64,
    pub(crate) root: String,
    pub(crate) workspace_root: String,
    pub(crate) skill_root: String,
    pub(crate) okf_root: String,
    pub(crate) updated_at: DateTime<Utc>,
    pub(crate) stats: EvolutionStats,
    pub(crate) candidates: Vec<EvolutionCandidate>,
    pub(crate) policy: EvolutionPolicySummary,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionPolicySummary {
    pub(crate) ready_evidence: usize,
    pub(crate) auto_materialize_evidence: usize,
    pub(crate) auto_materialize_sessions: usize,
    pub(crate) auto_materialize_confidence: f32,
    pub(crate) local_only: bool,
    pub(crate) review_supported: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct EvolutionMutationResult {
    pub(crate) candidate: EvolutionCandidate,
    pub(crate) requires_session_reload: bool,
    pub(crate) recovery_path: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct EvolutionDescriptor {
    pub(crate) kind: EvolutionKind,
    pub(crate) pattern_key: String,
    pub(crate) title: String,
    pub(crate) summary: String,
    pub(crate) instructions: Vec<String>,
    pub(crate) explicit_signal: bool,
}
