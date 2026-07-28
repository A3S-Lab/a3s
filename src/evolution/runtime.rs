use std::fs::File;
use std::io::Read;
use std::path::Path;

use anyhow::Context;

use super::materialize::{ensure_no_symlinks, marker_owned_by, safe_workspace_join};
use super::model::{EvolutionCandidate, EvolutionKind, EvolutionState};
use super::store::{read_catalog, truncate_chars, EvolutionPaths};

const MAX_ACTIVE_PREFERENCES: usize = 32;
const MAX_PREFERENCE_PREFIX_BYTES: u64 = 32 * 1024;
const MAX_PREFERENCE_SECTION_CHARS: usize = 2_400;
const MAX_PREFERENCE_PROMPT_CHARS: usize = 16_000;

pub(super) fn session_preference_prompt(paths: &EvolutionPaths) -> anyhow::Result<Option<String>> {
    let mut candidates = read_catalog(paths)?
        .candidates
        .into_iter()
        .filter(active_preference)
        .collect::<Vec<_>>();
    candidates.sort_by(|left, right| {
        left.pattern_key
            .cmp(&right.pattern_key)
            .then_with(|| left.title.cmp(&right.title))
    });

    let mut sections = Vec::new();
    let mut used_chars = 0usize;
    for candidate in candidates.into_iter().take(MAX_ACTIVE_PREFERENCES) {
        let Some(section) = load_preference_section(paths, &candidate) else {
            continue;
        };
        let section = format!("### {}\n\n{}", candidate.title.trim(), section.trim());
        let remaining = MAX_PREFERENCE_PROMPT_CHARS.saturating_sub(used_chars);
        if remaining < 32 {
            break;
        }
        let section = truncate_chars(&section, remaining.min(MAX_PREFERENCE_SECTION_CHARS));
        used_chars = used_chars.saturating_add(section.chars().count());
        sections.push(section);
    }
    if sections.is_empty() {
        return Ok(None);
    }

    Ok(Some(format!(
        "# Learned Local Preferences\n\nApply these workspace-local preferences when relevant. A newer explicit user request or higher-priority instruction takes precedence.\n\n{}",
        sections.join("\n\n")
    )))
}

fn active_preference(candidate: &EvolutionCandidate) -> bool {
    candidate.kind == EvolutionKind::Preference
        && candidate.current_version.is_some()
        && matches!(
            candidate.state,
            EvolutionState::Materialized | EvolutionState::RolledBack
        )
}

fn load_preference_section(
    paths: &EvolutionPaths,
    candidate: &EvolutionCandidate,
) -> Option<String> {
    let relative = candidate.asset_path.as_deref()?;
    let asset = safe_workspace_join(paths, relative).ok()?;
    ensure_no_symlinks(paths, &asset).ok()?;
    if !asset.is_file() || !marker_owned_by(&asset, candidate.kind, &candidate.id) {
        return None;
    }
    read_learned_preferences(&asset)
        .map_err(|error| {
            tracing::warn!(
                candidate_id = %candidate.id,
                path = %asset.display(),
                %error,
                "could not load a materialized preference for session context"
            );
            error
        })
        .ok()
}

fn read_learned_preferences(path: &Path) -> anyhow::Result<String> {
    let mut source = String::new();
    File::open(path)
        .with_context(|| format!("could not open {}", path.display()))?
        .take(MAX_PREFERENCE_PREFIX_BYTES)
        .read_to_string(&mut source)
        .with_context(|| format!("could not read {}", path.display()))?;
    let marker = "## Learned Preferences";
    let start = source
        .find(marker)
        .map(|index| index + marker.len())
        .context("preference asset has no learned-preferences section")?;
    let remainder = source[start..].trim_start_matches(['\r', '\n', ' ']);
    let end = remainder.find("\n## ").unwrap_or(remainder.len());
    let section = remainder[..end].trim();
    if section.is_empty() {
        anyhow::bail!("preference asset has an empty learned-preferences section");
    }
    Ok(truncate_chars(section, MAX_PREFERENCE_SECTION_CHARS))
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    use crate::evolution::materialize::{materialize_candidate, rollback_candidate};
    use crate::evolution::model::{EvolutionAuditEvent, EvolutionCandidate, EvolutionEvidence};
    use crate::evolution::store::{
        mark_session_assets_activated, mutate_catalog, pending_session_reload_count, read_catalog,
    };

    #[test]
    fn only_the_active_preference_instructions_enter_session_context() {
        let temp = tempfile::tempdir().unwrap();
        let paths = EvolutionPaths::new(temp.path());
        mutate_catalog(&paths, |catalog| {
            catalog.candidates.push(preference_candidate());
            Ok(())
        })
        .unwrap();
        let materialized =
            materialize_candidate(&paths, "preference-session-context", false, false).unwrap();
        assert!(materialized.requires_session_reload);
        assert_eq!(pending_session_reload_count(&paths).unwrap(), 1);
        assert_eq!(mark_session_assets_activated(&paths).unwrap(), 1);
        assert_eq!(pending_session_reload_count(&paths).unwrap(), 0);
        assert!(matches!(
            read_catalog(&paths).unwrap().candidates[0]
                .audit
                .last()
                .unwrap()
                .action,
            crate::evolution::model::EvolutionAuditAction::Activated
        ));

        let prompt = session_preference_prompt(&paths).unwrap().unwrap();

        assert!(prompt.contains("Keep completion claims concise."));
        assert!(prompt.contains("A newer explicit user request"));
        assert!(!prompt.contains("The user explicitly requested concise completion claims."));

        rollback_candidate(&paths, "preference-session-context", Some(0)).unwrap();
        assert!(session_preference_prompt(&paths).unwrap().is_none());
        assert_eq!(pending_session_reload_count(&paths).unwrap(), 1);
        assert_eq!(mark_session_assets_activated(&paths).unwrap(), 1);
        assert!(matches!(
            read_catalog(&paths).unwrap().candidates[0]
                .audit
                .last()
                .unwrap()
                .action,
            crate::evolution::model::EvolutionAuditAction::Deactivated
        ));

        rollback_candidate(&paths, "preference-session-context", Some(1)).unwrap();
        assert!(session_preference_prompt(&paths)
            .unwrap()
            .is_some_and(|value| value.contains("Keep completion claims concise.")));
    }

    fn preference_candidate() -> EvolutionCandidate {
        let now = Utc::now();
        EvolutionCandidate {
            id: "preference-session-context".to_string(),
            kind: EvolutionKind::Preference,
            pattern_key: "preference.response.concise".to_string(),
            pattern_aliases: Vec::new(),
            title: "Concise completion claims".to_string(),
            summary: "Keep completion claims concise while retaining evidence.".to_string(),
            instructions: vec!["Keep completion claims concise.".to_string()],
            state: EvolutionState::Ready,
            evidence: vec![EvolutionEvidence {
                id: "preference-evidence".to_string(),
                memory_id: "memory-preference".to_string(),
                session_id: Some("session-one".to_string()),
                source: "preference".to_string(),
                content: "The user explicitly requested concise completion claims.".to_string(),
                reason: Some("This affects future responses.".to_string()),
                timestamp: now,
                importance: 0.94,
                confidence: 0.96,
                conflicts_with: Vec::new(),
                explicit_signal: true,
            }],
            occurrences: 1,
            distinct_sessions: 1,
            confidence: 0.96,
            importance: 0.94,
            maturity: 0.9,
            has_conflicts: false,
            update_available: false,
            activation_pending: false,
            created_at: now,
            updated_at: now,
            ready_at: Some(now),
            materialized_at: None,
            rejected_at: None,
            rolled_back_at: None,
            rejection_reason: None,
            asset_path: None,
            current_version: None,
            versions: Vec::new(),
            audit: Vec::<EvolutionAuditEvent>::new(),
        }
    }
}
