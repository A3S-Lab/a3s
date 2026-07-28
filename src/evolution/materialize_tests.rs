use super::*;
use crate::evolution::model::{EvolutionCatalog, EvolutionEvidence};
use crate::evolution::store::{read_catalog, reject_candidate};

fn ready_candidate(id: &str, kind: EvolutionKind) -> EvolutionCandidate {
    let now = Utc::now();
    EvolutionCandidate {
        id: id.to_string(),
        kind,
        pattern_key: "workflow.rust.memory-tests".to_string(),
        pattern_aliases: Vec::new(),
        title: "Focused memory verification".to_string(),
        summary: "Run focused memory verification after persistence changes.".to_string(),
        instructions: vec![
            "Run the focused memory tests.".to_string(),
            "Inspect file-backed persistence results.".to_string(),
        ],
        state: EvolutionState::Ready,
        evidence: vec![EvolutionEvidence {
            id: "evidence-one".to_string(),
            memory_id: "memory-one".to_string(),
            session_id: Some("session-one".to_string()),
            source: "workflow".to_string(),
            content: "Run focused tests after persistence changes.".to_string(),
            reason: Some("Prevents persistence regressions.".to_string()),
            timestamp: now,
            importance: 0.9,
            confidence: 0.95,
            conflicts_with: Vec::new(),
            explicit_signal: true,
        }],
        occurrences: 1,
        distinct_sessions: 1,
        confidence: 0.95,
        importance: 0.9,
        maturity: 0.8,
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
        audit: Vec::new(),
    }
}

fn seed(paths: &EvolutionPaths, candidate: EvolutionCandidate) {
    mutate_catalog(paths, |catalog| {
        catalog.candidates.push(candidate);
        Ok(())
    })
    .unwrap();
}

#[test]
fn skill_materialization_is_versioned_and_read_only() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    seed(
        &paths,
        ready_candidate("evo-skill-test", EvolutionKind::Skill),
    );
    let result = materialize_candidate(&paths, "evo-skill-test", false, false).unwrap();
    assert!(result.requires_session_reload);
    assert_eq!(result.candidate.current_version, Some(1));
    let asset = temp.path().join(result.candidate.asset_path.unwrap());
    let skill = fs::read_to_string(asset.join("SKILL.md")).unwrap();
    assert!(skill.contains("allowed-tools: \"Read(*), Grep(*), Glob(*), LS(*)\""));
    assert!(asset.join(".a3s/asset.acl").is_file());
    assert!(paths
        .history
        .join("evo-skill-test/v0001/asset/SKILL.md")
        .is_file());
}

#[test]
fn update_refuses_unreviewed_external_edits() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    seed(
        &paths,
        ready_candidate("evo-skill-test", EvolutionKind::Skill),
    );
    let first = materialize_candidate(&paths, "evo-skill-test", false, false).unwrap();
    let asset = temp.path().join(first.candidate.asset_path.unwrap());
    fs::write(asset.join("README.md"), "user edit").unwrap();
    mutate_catalog(&paths, |catalog| {
        let candidate = candidate_mut(catalog, "evo-skill-test")?;
        candidate.update_available = true;
        Ok(())
    })
    .unwrap();
    let error = materialize_candidate(&paths, "evo-skill-test", false, false)
        .unwrap_err()
        .to_string();
    assert!(error.contains("changed outside"), "{error}");
}

#[test]
fn rollback_preserves_pre_rollback_recovery_copy() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    seed(&paths, ready_candidate("evo-okf-test", EvolutionKind::Okf));
    materialize_candidate(&paths, "evo-okf-test", false, false).unwrap();
    mutate_catalog(&paths, |catalog| {
        let candidate = candidate_mut(catalog, "evo-okf-test")?;
        candidate
            .instructions
            .push("Verify the index links.".to_string());
        candidate.update_available = true;
        Ok(())
    })
    .unwrap();
    materialize_candidate(&paths, "evo-okf-test", false, false).unwrap();
    let result = rollback_candidate(&paths, "evo-okf-test", Some(1)).unwrap();
    assert_eq!(result.candidate.state, EvolutionState::RolledBack);
    assert_eq!(result.candidate.current_version, Some(1));
    assert!(result.recovery_path.is_some());
    assert!(temp.path().join(result.recovery_path.unwrap()).exists());
}

#[test]
fn failed_snapshot_verification_restores_the_pre_rollback_asset() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    seed(
        &paths,
        ready_candidate("evo-skill-corrupt-snapshot", EvolutionKind::Skill),
    );
    materialize_candidate(&paths, "evo-skill-corrupt-snapshot", false, false).unwrap();
    mutate_catalog(&paths, |catalog| {
        let candidate = candidate_mut(catalog, "evo-skill-corrupt-snapshot")?;
        candidate
            .instructions
            .push("Run the broader verification after focused checks.".to_string());
        candidate.update_available = true;
        Ok(())
    })
    .unwrap();
    let current =
        materialize_candidate(&paths, "evo-skill-corrupt-snapshot", false, false).unwrap();
    let asset = temp
        .path()
        .join(current.candidate.asset_path.as_deref().unwrap());
    let current_hash = hash_path(&asset).unwrap();
    let first_snapshot = temp
        .path()
        .join(&current.candidate.versions[0].snapshot_path)
        .join("SKILL.md");
    fs::write(&first_snapshot, "corrupted immutable snapshot").unwrap();

    let error = rollback_candidate(&paths, "evo-skill-corrupt-snapshot", Some(1))
        .unwrap_err()
        .to_string();

    assert!(error.contains("immutable v1 snapshot"), "{error}");
    assert_eq!(hash_path(&asset).unwrap(), current_hash);
    let catalog = read_catalog(&paths).unwrap();
    let candidate = catalog
        .candidates
        .iter()
        .find(|candidate| candidate.id == "evo-skill-corrupt-snapshot")
        .unwrap();
    assert_eq!(candidate.state, EvolutionState::Materialized);
    assert_eq!(candidate.current_version, Some(2));
}

#[test]
fn baseline_rollback_removes_the_first_asset_and_can_restore_its_snapshot() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    seed(
        &paths,
        ready_candidate("evo-skill-baseline", EvolutionKind::Skill),
    );
    let first = materialize_candidate(&paths, "evo-skill-baseline", false, false).unwrap();
    let asset = temp
        .path()
        .join(first.candidate.asset_path.as_deref().unwrap());
    assert!(asset.is_dir());

    let removed = rollback_candidate(&paths, "evo-skill-baseline", Some(0)).unwrap();

    assert_eq!(removed.candidate.state, EvolutionState::RolledBack);
    assert_eq!(removed.candidate.current_version, None);
    assert!(removed.requires_session_reload);
    assert!(!asset.exists());
    assert!(removed
        .recovery_path
        .as_deref()
        .is_some_and(|path| temp.path().join(path).exists()));

    let restored = rollback_candidate(&paths, "evo-skill-baseline", Some(1)).unwrap();

    assert_eq!(restored.candidate.state, EvolutionState::RolledBack);
    assert_eq!(restored.candidate.current_version, Some(1));
    assert!(restored.requires_session_reload);
    assert!(asset.join("SKILL.md").is_file());
    assert_eq!(
        hash_path(&asset).unwrap(),
        first.candidate.versions[0].content_hash
    );
}

#[test]
fn rejection_requires_a_rolled_back_candidate_to_have_no_active_version() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    seed(
        &paths,
        ready_candidate("evo-reject-baseline", EvolutionKind::Okf),
    );
    materialize_candidate(&paths, "evo-reject-baseline", false, false).unwrap();
    mutate_catalog(&paths, |catalog| {
        let candidate = candidate_mut(catalog, "evo-reject-baseline")?;
        candidate
            .instructions
            .push("Verify the restored snapshot before reuse.".to_string());
        candidate.update_available = true;
        Ok(())
    })
    .unwrap();
    materialize_candidate(&paths, "evo-reject-baseline", false, false).unwrap();
    let rolled_back = rollback_candidate(&paths, "evo-reject-baseline", Some(1)).unwrap();
    assert_eq!(rolled_back.candidate.state, EvolutionState::RolledBack);
    assert_eq!(rolled_back.candidate.current_version, Some(1));

    let error = reject_candidate(&paths, "evo-reject-baseline", None)
        .unwrap_err()
        .to_string();
    assert!(error.contains("active materialized version"), "{error}");

    rollback_candidate(&paths, "evo-reject-baseline", Some(0)).unwrap();
    let rejected = reject_candidate(
        &paths,
        "evo-reject-baseline",
        Some("Keep this pattern out of future suggestions.".to_string()),
    )
    .unwrap();
    assert_eq!(rejected.state, EvolutionState::Rejected);
    assert_eq!(rejected.current_version, None);
}

#[test]
fn catalog_seed_uses_current_schema() {
    let temp = tempfile::tempdir().unwrap();
    let paths = EvolutionPaths::new(temp.path());
    let catalog = EvolutionCatalog::empty(temp.path().display().to_string());
    assert_eq!(catalog.schema, super::super::model::EVOLUTION_SCHEMA);
    assert!(!paths.state.exists());
}
