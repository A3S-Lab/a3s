//! Bounded, read-only Skill invocation analysis for `/checkup`.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use a3s_code_core::llm::ContentBlock;
use a3s_code_core::store::SessionStore;

const MAX_USAGE_SESSIONS: usize = 128;
const MAX_REPORTED_CANDIDATES: usize = 24;
const MIN_EVIDENCE_SESSIONS: usize = 3;
const MIN_EVIDENCE_TURNS: usize = 12;
const MIN_SKILL_AGE_DAYS: i64 = 14;
const SECONDS_PER_DAY: i64 = 24 * 60 * 60;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillUsageSubject {
    pub(super) name: String,
    pub(super) bytes: u64,
    pub(super) copies: usize,
    pub(super) oldest_modified_at: Option<i64>,
    pub(super) enabled: bool,
    pub(super) managed: bool,
}

impl SkillUsageSubject {
    pub(super) fn new(name: String, bytes: u64, modified_at: Option<i64>, enabled: bool) -> Self {
        let managed = matches!(name.as_str(), "okf" | "a3s-os-capabilities");
        Self {
            name,
            bytes,
            copies: 1,
            oldest_modified_at: modified_at,
            enabled,
            managed,
        }
    }

    pub(super) fn merge_copy(&mut self, bytes: u64, modified_at: Option<i64>) {
        self.bytes = self.bytes.saturating_add(bytes);
        self.copies = self.copies.saturating_add(1);
        self.oldest_modified_at = match (self.oldest_modified_at, modified_at) {
            (Some(left), Some(right)) => Some(left.min(right)),
            (None, value) | (value, None) => value,
        };
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct InvocationStats {
    invocations: usize,
    sessions: usize,
    last_observed_at: Option<i64>,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(super) struct SkillHistorySample {
    available: bool,
    saved_sessions: usize,
    sampled_sessions: usize,
    sampled_turns: usize,
    load_failures: usize,
    earliest_created_at: Option<i64>,
    latest_updated_at: Option<i64>,
    invocations: BTreeMap<String, InvocationStats>,
}

impl SkillHistorySample {
    fn unavailable() -> Self {
        Self::default()
    }

    fn observe_session(&mut self, session: a3s_code_core::store::SessionData) {
        self.sampled_sessions = self.sampled_sessions.saturating_add(1);
        self.sampled_turns = self
            .sampled_turns
            .saturating_add(session.context_usage.turns);
        self.earliest_created_at = Some(
            self.earliest_created_at
                .map_or(session.created_at, |current| {
                    current.min(session.created_at)
                }),
        );
        self.latest_updated_at = Some(
            self.latest_updated_at
                .map_or(session.updated_at, |current| {
                    current.max(session.updated_at)
                }),
        );

        let mut used_in_session = HashSet::new();
        for message in session.messages {
            for block in message.content {
                let ContentBlock::ToolUse { name, input, .. } = block else {
                    continue;
                };
                if !name.eq_ignore_ascii_case("skill") {
                    continue;
                }
                let Some(skill_name) = skill_invocation_name(&input) else {
                    continue;
                };
                let stats = self.invocations.entry(skill_name.clone()).or_default();
                stats.invocations = stats.invocations.saturating_add(1);
                stats.last_observed_at = Some(
                    stats
                        .last_observed_at
                        .map_or(session.updated_at, |current| {
                            current.max(session.updated_at)
                        }),
                );
                used_in_session.insert(skill_name);
            }
        }
        for name in used_in_session {
            self.invocations.entry(name).or_default().sessions += 1;
        }
    }

    fn evidence_is_sufficient(&self) -> bool {
        self.available
            && self.sampled_sessions >= MIN_EVIDENCE_SESSIONS
            && self.sampled_turns >= MIN_EVIDENCE_TURNS
    }

    fn total_invocations(&self) -> usize {
        self.invocations
            .values()
            .map(|stats| stats.invocations)
            .sum()
    }
}

pub(super) async fn inspect_skill_history(
    store: Arc<dyn SessionStore>,
    current_session_id: String,
) -> SkillHistorySample {
    let Ok(mut ids) = store.list().await else {
        return SkillHistorySample::unavailable();
    };
    ids.sort();
    ids.dedup();
    let saved_sessions = ids.len();

    let mut selected = Vec::with_capacity(saved_sessions.min(MAX_USAGE_SESSIONS));
    if let Some(position) = ids.iter().position(|id| id == &current_session_id) {
        selected.push(ids.remove(position));
    }
    selected.extend(
        ids.into_iter()
            .rev()
            .take(MAX_USAGE_SESSIONS - selected.len()),
    );

    let mut sample = SkillHistorySample {
        available: true,
        saved_sessions,
        ..SkillHistorySample::default()
    };
    for id in selected {
        match store.load(&id).await {
            Ok(Some(session)) => sample.observe_session(session),
            Ok(None) | Err(_) => sample.load_failures = sample.load_failures.saturating_add(1),
        }
    }
    sample
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct LowUsageCandidate {
    name: String,
    bytes: u64,
    invocations: usize,
    sessions: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SkillUsageAudit {
    history: SkillHistorySample,
    never_observed: Vec<LowUsageCandidate>,
    rarely_observed: Vec<LowUsageCandidate>,
    recently_changed: usize,
    already_disabled: usize,
    managed: usize,
    duplicate_names: usize,
    unknown_age: usize,
}

impl SkillUsageAudit {
    pub(super) fn classify(
        subjects: &[SkillUsageSubject],
        history: SkillHistorySample,
        now_epoch_seconds: i64,
    ) -> Self {
        let mut audit = Self {
            history,
            never_observed: Vec::new(),
            rarely_observed: Vec::new(),
            recently_changed: 0,
            already_disabled: 0,
            managed: 0,
            duplicate_names: 0,
            unknown_age: 0,
        };
        if !audit.history.evidence_is_sufficient() {
            return audit;
        }

        for subject in subjects {
            if !subject.enabled {
                audit.already_disabled += 1;
                continue;
            }
            if subject.managed {
                audit.managed += 1;
                continue;
            }
            if subject.copies > 1 {
                audit.duplicate_names += 1;
                continue;
            }
            let Some(modified_at) = subject.oldest_modified_at else {
                audit.unknown_age += 1;
                continue;
            };
            if now_epoch_seconds.saturating_sub(modified_at)
                < MIN_SKILL_AGE_DAYS.saturating_mul(SECONDS_PER_DAY)
            {
                audit.recently_changed += 1;
                continue;
            }

            let stats = audit
                .history
                .invocations
                .get(&subject.name)
                .cloned()
                .unwrap_or_default();
            let candidate = LowUsageCandidate {
                name: subject.name.clone(),
                bytes: subject.bytes,
                invocations: stats.invocations,
                sessions: stats.sessions,
            };
            if stats.invocations == 0 {
                audit.never_observed.push(candidate);
            } else if stats.invocations == 1 && stats.sessions == 1 {
                audit.rarely_observed.push(candidate);
            }
        }
        audit.never_observed.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.name.cmp(&right.name))
        });
        audit.rarely_observed.sort_by(|left, right| {
            right
                .bytes
                .cmp(&left.bytes)
                .then_with(|| left.name.cmp(&right.name))
        });
        audit.never_observed.truncate(MAX_REPORTED_CANDIDATES);
        audit.rarely_observed.truncate(MAX_REPORTED_CANDIDATES);
        audit
    }

    pub(super) fn render(&self) -> String {
        if !self.history.available {
            return "- skill usage history: unavailable; no low-use cleanup candidate was inferred"
                .to_string();
        }
        let window = match (
            self.history.earliest_created_at,
            self.history.latest_updated_at,
        ) {
            (Some(start), Some(end)) => format!("{} to {}", day(start), day(end)),
            _ => "no dated history".to_string(),
        };
        let mut lines = vec![format!(
            "- skill usage history: {} of {} saved session(s) inspected, {} completed turn(s), {} Skill invocation(s), window {window}; {} unreadable session(s)",
            self.history.sampled_sessions,
            self.history.saved_sessions,
            self.history.sampled_turns,
            self.history.total_invocations(),
            self.history.load_failures,
        )];
        if !self.history.evidence_is_sufficient() {
            lines.push(format!(
                "- low-use skill review: insufficient history; at least {MIN_EVIDENCE_SESSIONS} sessions and {MIN_EVIDENCE_TURNS} completed turns are required before suggesting cleanup"
            ));
            return lines.join("\n");
        }

        let never = candidate_list(&self.never_observed);
        let rare = candidate_list(&self.rarely_observed);
        lines.push(format!(
            "- low-use skill review (observed local history only; review before disabling): {} not observed [{}]; {} observed once [{}]",
            self.never_observed.len(), never, self.rarely_observed.len(), rare
        ));
        lines.push(format!(
            "- low-use exclusions: {} changed within {MIN_SKILL_AGE_DAYS} days, {} already disabled, {} managed, {} duplicate-name, {} unknown-age skill(s); no Skill was changed or removed",
            self.recently_changed,
            self.already_disabled,
            self.managed,
            self.duplicate_names,
            self.unknown_age,
        ));
        lines.join("\n")
    }
}

fn skill_invocation_name(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::String(name) => normalize_name(name),
        serde_json::Value::Object(map) => map
            .get("skill_name")
            .or_else(|| map.get("skillName"))
            .or_else(|| map.get("name"))
            .and_then(serde_json::Value::as_str)
            .and_then(normalize_name)
            .or_else(|| {
                map.get("input")
                    .or_else(|| map.get("arguments"))
                    .and_then(skill_invocation_name)
            }),
        _ => None,
    }
}

fn normalize_name(value: &str) -> Option<String> {
    let name = value.trim();
    if name.is_empty() || name.chars().count() > 120 || name.chars().any(char::is_control) {
        return None;
    }
    Some(name.to_string())
}

fn candidate_list(candidates: &[LowUsageCandidate]) -> String {
    if candidates.is_empty() {
        return "none".to_string();
    }
    candidates
        .iter()
        .map(|candidate| {
            format!(
                "{}={} call(s)/{} session(s)/{}",
                safe_name(&candidate.name),
                candidate.invocations,
                candidate.sessions,
                human_bytes(candidate.bytes)
            )
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn safe_name(value: &str) -> String {
    value
        .chars()
        .filter(|character| !character.is_control())
        .take(120)
        .collect::<String>()
        .replace(['[', ']', '\n', '\r'], "_")
}

fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KiB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

fn day(timestamp: i64) -> String {
    chrono::DateTime::from_timestamp(timestamp, 0)
        .map(|value| value.format("%Y-%m-%d").to_string())
        .unwrap_or_else(|| "unknown-date".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_code_core::llm::{Message, TokenUsage};
    use a3s_code_core::store::{
        ContextUsage, MemorySessionStore, SessionConfig, SessionData, SessionState,
    };

    fn sufficient_history() -> SkillHistorySample {
        SkillHistorySample {
            available: true,
            saved_sessions: 4,
            sampled_sessions: 4,
            sampled_turns: 24,
            earliest_created_at: Some(1_700_000_000),
            latest_updated_at: Some(1_710_000_000),
            invocations: BTreeMap::from([
                (
                    "rare".to_string(),
                    InvocationStats {
                        invocations: 1,
                        sessions: 1,
                        last_observed_at: Some(1_710_000_000),
                    },
                ),
                (
                    "frequent".to_string(),
                    InvocationStats {
                        invocations: 8,
                        sessions: 3,
                        last_observed_at: Some(1_710_000_000),
                    },
                ),
            ]),
            ..SkillHistorySample::default()
        }
    }

    fn old_subject(name: &str) -> SkillUsageSubject {
        SkillUsageSubject::new(name.to_string(), 4096, Some(1_600_000_000), true)
    }

    fn stored_session(id: &str, skill_names: &[&str], turns: usize, timestamp: i64) -> SessionData {
        SessionData {
            id: id.to_string(),
            config: SessionConfig::default(),
            state: SessionState::Completed,
            messages: skill_names
                .iter()
                .enumerate()
                .map(|(index, name)| Message {
                    role: "assistant".to_string(),
                    content: vec![ContentBlock::ToolUse {
                        id: format!("call-{index}"),
                        name: "Skill".to_string(),
                        input: serde_json::json!({"skill_name": name}),
                    }],
                    reasoning_content: None,
                })
                .collect(),
            context_usage: ContextUsage {
                turns,
                ..ContextUsage::default()
            },
            total_usage: TokenUsage::default(),
            total_cost: 0.0,
            model_name: None,
            cost_records: Vec::new(),
            tool_names: vec!["Skill".to_string()],
            thinking_enabled: false,
            thinking_budget: None,
            created_at: timestamp,
            updated_at: timestamp + 60,
            llm_config: None,
            tasks: Vec::new(),
            parent_id: None,
            tenant_id: None,
            principal: None,
            agent_template_id: None,
            correlation_id: None,
        }
    }

    #[test]
    fn classifies_only_old_enabled_unambiguous_low_use_skills() {
        let mut duplicate = old_subject("duplicate");
        duplicate.merge_copy(1024, Some(1_600_000_100));
        let mut disabled = old_subject("disabled");
        disabled.enabled = false;
        let recent = SkillUsageSubject::new(
            "recent".to_string(),
            2048,
            Some(1_720_000_000 - SECONDS_PER_DAY),
            true,
        );

        let audit = SkillUsageAudit::classify(
            &[
                old_subject("never"),
                old_subject("rare"),
                old_subject("frequent"),
                old_subject("okf"),
                duplicate,
                disabled,
                recent,
            ],
            sufficient_history(),
            1_720_000_000,
        );

        assert_eq!(audit.never_observed[0].name, "never");
        assert_eq!(audit.rarely_observed[0].name, "rare");
        assert_eq!(audit.managed, 1);
        assert_eq!(audit.duplicate_names, 1);
        assert_eq!(audit.already_disabled, 1);
        assert_eq!(audit.recently_changed, 1);
        assert!(!audit.render().contains("frequent="));
    }

    #[test]
    fn insufficient_history_never_proposes_cleanup() {
        let history = SkillHistorySample {
            available: true,
            saved_sessions: 1,
            sampled_sessions: 1,
            sampled_turns: 3,
            ..SkillHistorySample::default()
        };

        let audit = SkillUsageAudit::classify(&[old_subject("unused")], history, 1_720_000_000);

        assert!(audit.never_observed.is_empty());
        assert!(audit.render().contains("insufficient history"));
    }

    #[test]
    fn parses_the_same_skill_argument_shapes_as_the_runtime_tool() {
        for value in [
            serde_json::json!("review"),
            serde_json::json!({"skill_name": "review"}),
            serde_json::json!({"skillName": "review"}),
            serde_json::json!({"input": {"name": "review"}}),
        ] {
            assert_eq!(skill_invocation_name(&value).as_deref(), Some("review"));
        }
    }

    #[tokio::test]
    async fn aggregates_real_skill_calls_from_persisted_sessions() {
        let store = Arc::new(MemorySessionStore::new());
        store
            .save(&stored_session(
                "session-one",
                &["review", "review"],
                8,
                1_700_000_000,
            ))
            .await
            .unwrap();
        store
            .save(&stored_session(
                "session-two",
                &["review", "release"],
                9,
                1_700_100_000,
            ))
            .await
            .unwrap();

        let history = inspect_skill_history(store, "session-two".to_string()).await;

        assert_eq!(history.saved_sessions, 2);
        assert_eq!(history.sampled_sessions, 2);
        assert_eq!(history.sampled_turns, 17);
        assert_eq!(history.invocations["review"].invocations, 3);
        assert_eq!(history.invocations["review"].sessions, 2);
        assert_eq!(history.invocations["release"].invocations, 1);
        assert_eq!(history.invocations["release"].sessions, 1);
    }
}
