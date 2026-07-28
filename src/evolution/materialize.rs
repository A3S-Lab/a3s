use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};

use anyhow::{anyhow, Context};
use chrono::Utc;
use serde_json::json;
use sha2::{Digest, Sha256};

use super::model::{
    EvolutionAuditAction, EvolutionAuditEvent, EvolutionCandidate, EvolutionKind,
    EvolutionMutationResult, EvolutionState, EvolutionVersion,
};
use super::store::{candidate_mut, mutate_catalog, short_hash, slugify, EvolutionPaths};
use crate::tui::asset_lifecycle::{self, AssetAclDocument, OsService, RuntimeBindingIntent};

pub(super) fn materialize_candidate(
    paths: &EvolutionPaths,
    id: &str,
    force: bool,
    automatic: bool,
) -> anyhow::Result<EvolutionMutationResult> {
    mutate_catalog(paths, |catalog| {
        let candidate = candidate_mut(catalog, id)?;
        if candidate.state == EvolutionState::Rejected {
            return Err(anyhow!(
                "rejected candidates must be reopened before materialization"
            ));
        }
        if automatic && candidate.has_conflicts {
            return Err(anyhow!("conflicting evidence requires review"));
        }
        if candidate.instructions.is_empty() {
            return Err(anyhow!("candidate has no reusable instructions"));
        }
        if candidate.state == EvolutionState::Materialized && !candidate.update_available {
            return Ok(EvolutionMutationResult {
                candidate: candidate.clone(),
                requires_session_reload: false,
                recovery_path: None,
            });
        }

        let asset = resolve_asset_path(paths, candidate)?;
        ensure_path_inside_workspace(paths, &asset)?;
        ensure_no_symlinks(paths, &asset)?;
        if asset.exists() {
            ensure_owned_asset(candidate, &asset, force)?;
            if let Some(current_version) = candidate.current_version {
                if let Some(version) = candidate
                    .versions
                    .iter()
                    .find(|version| version.version == current_version)
                {
                    let actual_hash = hash_path(&asset)?;
                    if actual_hash != version.content_hash && !force {
                        return Err(anyhow!(
                            "{} changed outside the evolution service; retry with force only after reviewing those edits",
                            asset.display()
                        ));
                    }
                }
            }
        }

        let version = candidate
            .versions
            .iter()
            .map(|version| version.version)
            .max()
            .unwrap_or(0)
            .saturating_add(1);
        write_candidate_asset(paths, candidate, &asset, version)?;
        let content_hash = hash_path(&asset)?;
        let snapshot = paths
            .history
            .join(&candidate.id)
            .join(format!("v{version:04}"))
            .join("asset");
        if snapshot.exists() {
            return Err(anyhow!(
                "evolution snapshot already exists: {}",
                snapshot.display()
            ));
        }
        copy_path(&asset, &snapshot)?;

        let relative_asset = relative_workspace_path(paths, &asset)?;
        let relative_snapshot = relative_workspace_path(paths, &snapshot)?;
        let evidence_ids = candidate
            .evidence
            .iter()
            .map(|evidence| evidence.id.clone())
            .collect::<Vec<_>>();
        let was_materialized = candidate.current_version.is_some();
        let now = Utc::now();
        candidate.versions.push(EvolutionVersion {
            version,
            created_at: now,
            asset_path: relative_asset.clone(),
            snapshot_path: relative_snapshot,
            content_hash,
            evidence_ids,
            automatic,
        });
        candidate.asset_path = Some(relative_asset);
        candidate.current_version = Some(version);
        candidate.state = EvolutionState::Materialized;
        candidate.materialized_at = Some(now);
        candidate.rolled_back_at = None;
        candidate.update_available = false;
        candidate.activation_pending = requires_session_reload(candidate.kind);
        candidate.updated_at = now;
        candidate.audit.push(EvolutionAuditEvent {
            action: if was_materialized {
                EvolutionAuditAction::Updated
            } else {
                EvolutionAuditAction::Materialized
            },
            at: now,
            version: Some(version),
            note: Some(if automatic {
                "materialized locally after reaching the automatic maturity threshold".to_string()
            } else {
                "materialized locally after explicit review".to_string()
            }),
            recovery_path: None,
        });

        Ok(EvolutionMutationResult {
            candidate: candidate.clone(),
            requires_session_reload: requires_session_reload(candidate.kind),
            recovery_path: None,
        })
    })
}

pub(super) fn rollback_candidate(
    paths: &EvolutionPaths,
    id: &str,
    target_version: Option<u32>,
) -> anyhow::Result<EvolutionMutationResult> {
    mutate_catalog(paths, |catalog| {
        let candidate = candidate_mut(catalog, id)?;
        let current = candidate.current_version;
        let target = match (target_version, current) {
            (Some(target), _) => target,
            (None, Some(current)) => current.saturating_sub(1),
            (None, None) => candidate
                .versions
                .iter()
                .map(|version| version.version)
                .max()
                .ok_or_else(|| anyhow!("candidate has no materialized version"))?,
        };
        if current == Some(target) {
            return Err(anyhow!("v{target} is already the active version"));
        }
        if target == 0 {
            return rollback_to_baseline(paths, candidate, current);
        }
        let version = candidate
            .versions
            .iter()
            .find(|version| version.version == target)
            .cloned()
            .ok_or_else(|| anyhow!("candidate has no version v{target}"))?;
        let asset_rel = candidate
            .asset_path
            .as_deref()
            .unwrap_or(version.asset_path.as_str());
        let asset = safe_workspace_join(paths, asset_rel)?;
        let snapshot = safe_workspace_join(paths, &version.snapshot_path)?;
        if !snapshot.exists() {
            return Err(anyhow!(
                "version snapshot is missing: {}",
                snapshot.display()
            ));
        }
        ensure_no_symlinks(paths, &asset)?;
        ensure_no_symlinks(paths, &snapshot)?;

        let recovery = recovery_asset_path(paths, candidate, current);
        let preserved = preserve_active_asset(candidate, &asset, &recovery)?;
        let restore_result = (|| -> anyhow::Result<()> {
            copy_path(&snapshot, &asset).context("could not restore evolution snapshot")?;
            write_marker(candidate, &asset, target)
                .context("could not mark the restored evolution snapshot")?;
            let restored_hash = hash_path(&asset)?;
            if restored_hash != version.content_hash {
                return Err(anyhow!(
                    "restored asset hash does not match the immutable v{target} snapshot"
                ));
            }
            Ok(())
        })();
        if let Err(error) = restore_result {
            if let Err(recovery_error) =
                recover_failed_rollback(candidate, &asset, &recovery, preserved)
            {
                return Err(anyhow!(
                    "{error:#}; could not restore the pre-rollback asset: {recovery_error:#}"
                ));
            }
            return Err(error);
        }
        let now = Utc::now();
        candidate.current_version = Some(target);
        candidate.state = EvolutionState::RolledBack;
        candidate.rolled_back_at = Some(now);
        candidate.activation_pending = requires_session_reload(candidate.kind);
        candidate.update_available = candidate.evidence.iter().any(|evidence| {
            !version
                .evidence_ids
                .iter()
                .any(|evidence_id| evidence_id == &evidence.id)
        });
        candidate.updated_at = now;
        let recovery_path = preserved
            .then(|| relative_workspace_path(paths, &recovery))
            .transpose()?;
        let from = current
            .map(|version| format!("v{version}"))
            .unwrap_or_else(|| "the unmaterialized baseline".to_string());
        candidate.audit.push(EvolutionAuditEvent {
            action: EvolutionAuditAction::RolledBack,
            at: now,
            version: Some(target),
            note: Some(format!("restored v{target} from {from}")),
            recovery_path: recovery_path.clone(),
        });

        Ok(EvolutionMutationResult {
            candidate: candidate.clone(),
            requires_session_reload: requires_session_reload(candidate.kind),
            recovery_path,
        })
    })
}

fn rollback_to_baseline(
    paths: &EvolutionPaths,
    candidate: &mut EvolutionCandidate,
    current: Option<u32>,
) -> anyhow::Result<EvolutionMutationResult> {
    let current =
        current.ok_or_else(|| anyhow!("candidate is already at the unmaterialized baseline"))?;
    let asset_rel = candidate
        .asset_path
        .as_deref()
        .ok_or_else(|| anyhow!("candidate has no asset path"))?;
    let asset = safe_workspace_join(paths, asset_rel)?;
    ensure_no_symlinks(paths, &asset)?;
    let recovery = recovery_asset_path(paths, candidate, Some(current));
    let preserved = preserve_active_asset(candidate, &asset, &recovery)?;
    let now = Utc::now();
    candidate.current_version = None;
    candidate.state = EvolutionState::RolledBack;
    candidate.rolled_back_at = Some(now);
    candidate.activation_pending = requires_session_reload(candidate.kind);
    candidate.update_available = true;
    candidate.updated_at = now;
    let recovery_path = preserved
        .then(|| relative_workspace_path(paths, &recovery))
        .transpose()?;
    candidate.audit.push(EvolutionAuditEvent {
        action: EvolutionAuditAction::RolledBack,
        at: now,
        version: Some(0),
        note: Some(format!(
            "removed the active v{current} asset and returned to the unmaterialized baseline"
        )),
        recovery_path: recovery_path.clone(),
    });

    Ok(EvolutionMutationResult {
        candidate: candidate.clone(),
        requires_session_reload: requires_session_reload(candidate.kind),
        recovery_path,
    })
}

fn requires_session_reload(kind: EvolutionKind) -> bool {
    matches!(kind, EvolutionKind::Preference | EvolutionKind::Skill)
}

fn recovery_asset_path(
    paths: &EvolutionPaths,
    candidate: &EvolutionCandidate,
    current: Option<u32>,
) -> PathBuf {
    let source = current
        .map(|version| format!("v{version:04}"))
        .unwrap_or_else(|| "baseline".to_string());
    paths
        .recovery
        .join(&candidate.id)
        .join(format!(
            "{}-from-{source}",
            Utc::now().format("%Y%m%dT%H%M%S%.3fZ")
        ))
        .join("asset")
}

fn preserve_active_asset(
    candidate: &EvolutionCandidate,
    asset: &Path,
    recovery: &Path,
) -> anyhow::Result<bool> {
    if !asset.exists() {
        return Ok(false);
    }
    if let Some(parent) = recovery.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    fs::rename(asset, recovery).with_context(|| {
        format!(
            "could not preserve {} at {} before rollback",
            asset.display(),
            recovery.display()
        )
    })?;
    if candidate.kind == EvolutionKind::Preference {
        let marker = marker_path(asset, candidate.kind);
        let recovery_marker = marker_path(recovery, candidate.kind);
        if marker.exists() {
            if let Err(error) = fs::rename(&marker, &recovery_marker) {
                let _ = fs::rename(recovery, asset);
                return Err(error).with_context(|| {
                    format!(
                        "could not preserve preference marker {} at {}",
                        marker.display(),
                        recovery_marker.display()
                    )
                });
            }
        }
    }
    Ok(true)
}

fn recover_failed_rollback(
    candidate: &EvolutionCandidate,
    asset: &Path,
    recovery: &Path,
    preserved: bool,
) -> anyhow::Result<()> {
    remove_path_if_exists(asset)?;
    if candidate.kind == EvolutionKind::Preference {
        let marker = marker_path(asset, candidate.kind);
        remove_path_if_exists(&marker)?;
    }
    if preserved {
        restore_recovery_asset(candidate, recovery, asset)?;
    }
    Ok(())
}

fn restore_recovery_asset(
    candidate: &EvolutionCandidate,
    recovery: &Path,
    asset: &Path,
) -> anyhow::Result<()> {
    fs::rename(recovery, asset).with_context(|| {
        format!(
            "could not move recovery asset {} back to {}",
            recovery.display(),
            asset.display()
        )
    })?;
    if candidate.kind == EvolutionKind::Preference {
        let recovery_marker = marker_path(recovery, candidate.kind);
        let marker = marker_path(asset, candidate.kind);
        if recovery_marker.exists() {
            if let Err(error) = fs::rename(&recovery_marker, &marker) {
                let _ = fs::rename(asset, recovery);
                return Err(error).with_context(|| {
                    format!(
                        "could not move recovery marker {} back to {}",
                        recovery_marker.display(),
                        marker.display()
                    )
                });
            }
        }
    }
    Ok(())
}

fn remove_path_if_exists(path: &Path) -> anyhow::Result<()> {
    let metadata = match path.symlink_metadata() {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| format!("could not inspect {}", path.display()))
        }
    };
    if metadata.file_type().is_symlink() {
        return Err(anyhow!("refusing to remove symlink {}", path.display()));
    }
    if metadata.is_file() {
        fs::remove_file(path)
            .with_context(|| format!("could not remove partial asset {}", path.display()))?;
    } else if metadata.is_dir() {
        fs::remove_dir_all(path)
            .with_context(|| format!("could not remove partial asset {}", path.display()))?;
    } else {
        return Err(anyhow!(
            "unsupported partial asset entry {}",
            path.display()
        ));
    }
    Ok(())
}

fn resolve_asset_path(
    paths: &EvolutionPaths,
    candidate: &EvolutionCandidate,
) -> anyhow::Result<PathBuf> {
    if let Some(relative) = candidate.asset_path.as_deref() {
        return safe_workspace_join(paths, relative);
    }
    let fallback = format!("learned-{}", &short_hash(&candidate.id)[..10]);
    let slug = slugify(&candidate.title, &fallback);
    let learned_slug = if slug.starts_with("learned-") {
        slug.clone()
    } else {
        format!("learned-{slug}")
    };
    let desired = match candidate.kind {
        EvolutionKind::Preference => paths.preferences.join(format!("{slug}.md")),
        EvolutionKind::Skill => paths.skill_root.join(&learned_slug),
        EvolutionKind::Okf => paths.okf_root.join(&learned_slug),
    };
    if !desired.exists() || marker_owned_by(&desired, candidate.kind, &candidate.id) {
        return Ok(desired);
    }
    let suffix = &short_hash(&candidate.id)[..8];
    Ok(match candidate.kind {
        EvolutionKind::Preference => paths.preferences.join(format!("{slug}-{suffix}.md")),
        EvolutionKind::Skill => paths.skill_root.join(format!("{learned_slug}-{suffix}")),
        EvolutionKind::Okf => paths.okf_root.join(format!("{learned_slug}-{suffix}")),
    })
}

fn write_candidate_asset(
    paths: &EvolutionPaths,
    candidate: &EvolutionCandidate,
    asset: &Path,
    version: u32,
) -> anyhow::Result<()> {
    match candidate.kind {
        EvolutionKind::Preference => write_preference(candidate, asset, version),
        EvolutionKind::Skill => write_skill(paths, candidate, asset, version),
        EvolutionKind::Okf => write_okf(paths, candidate, asset, version),
    }
}

fn write_preference(
    candidate: &EvolutionCandidate,
    asset: &Path,
    version: u32,
) -> anyhow::Result<()> {
    let instructions = markdown_list(&candidate.instructions);
    let evidence = evidence_markdown(candidate);
    let body = format!(
        "---\n\
         schema: a3s.evolution.preference.v1\n\
         candidate: {}\n\
         version: {}\n\
         pattern: {}\n\
         ---\n\n\
         # {}\n\n\
         {}\n\n\
         ## Learned Preferences\n\n\
         {}\n\n\
         ## Evidence\n\n\
         {}\n",
        candidate.id,
        version,
        candidate.pattern_key,
        candidate.title,
        candidate.summary,
        instructions,
        evidence,
    );
    write_file(asset, body.as_bytes())?;
    write_marker(candidate, asset, version)
}

fn write_skill(
    paths: &EvolutionPaths,
    candidate: &EvolutionCandidate,
    asset: &Path,
    version: u32,
) -> anyhow::Result<()> {
    fs::create_dir_all(asset).with_context(|| format!("could not create {}", asset.display()))?;
    let name = asset
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("invalid skill asset path"))?;
    let description = serde_json::to_string(&candidate.summary)
        .unwrap_or_else(|_| "\"Learned local workflow\"".to_string());
    let workflow = numbered_list(&candidate.instructions);
    let skill = format!(
        "---\n\
         name: {name}\n\
         description: {description}\n\
         kind: instruction\n\
         allowed-tools: \"Read(*), Grep(*), Glob(*), LS(*)\"\n\
         ---\n\n\
         # {title}\n\n\
         {summary}\n\n\
         ## Workflow\n\n\
         {workflow}\n\n\
         ## Safety Boundary\n\n\
         - Treat this learned workflow as guidance, not authority to expand permissions.\n\
         - Re-check current workspace evidence before changing files.\n\
         - Stop and report contradictions instead of guessing.\n\n\
         ## Success Criteria\n\n\
         - The result follows the learned workflow where it applies.\n\
         - Claims and changes are backed by current evidence.\n",
        title = candidate.title,
        summary = candidate.summary,
    );
    write_file(&asset.join("SKILL.md"), skill.as_bytes())?;
    write_file(
        &asset.join("README.md"),
        format!(
            "# {}\n\n{}\n\nGenerated locally from candidate `{}`. Review and manage versions through the Code evolution surface. It is never published automatically.\n\n## Evidence\n\n{}\n",
            candidate.title,
            candidate.summary,
            candidate.id,
            evidence_markdown(candidate)
        )
        .as_bytes(),
    )?;
    write_file(
        &asset.join("tests/smoke.md"),
        b"# Learned Skill Smoke Check\n\n1. Confirm the trigger matches the current task.\n2. Confirm every instruction remains supported by current evidence.\n3. Confirm only read-only tool permissions are declared.\n",
    )?;
    let local_path = relative_workspace_path(paths, asset)?;
    let source = [("definition_path", "SKILL.md")];
    let metadata = [
        ("evolution_candidate", candidate.id.as_str()),
        ("evolution_pattern", candidate.pattern_key.as_str()),
    ];
    let acl = asset_lifecycle::render_asset_acl(AssetAclDocument {
        category: "skill",
        kind: Some("tool"),
        name,
        description: &candidate.summary,
        local_path: Some(&local_path),
        service: OsService::FunctionAsAService,
        runtime: RuntimeBindingIntent {
            kind: "tool",
            isolation: "serving",
            runtime_kind: "a3s-function-service",
            protocol: Some("skill"),
            agent_kind: Some("tool"),
        },
        source: &source,
        metadata: &metadata,
    });
    write_file(&asset.join(asset_lifecycle::ASSET_ACL_PATH), acl.as_bytes())?;
    write_marker(candidate, asset, version)
}

fn write_okf(
    paths: &EvolutionPaths,
    candidate: &EvolutionCandidate,
    asset: &Path,
    version: u32,
) -> anyhow::Result<()> {
    for directory in ["sources", "wiki/concepts", "eval", ".a3s"] {
        fs::create_dir_all(asset.join(directory))
            .with_context(|| format!("could not create {}", asset.join(directory).display()))?;
    }
    let concept_slug = slugify(&candidate.title, "learned-concept");
    let concept_path = format!("concepts/{concept_slug}.md");
    write_file(
        &asset.join("README.md"),
        format!(
            "# {}\n\n{}\n\nThis local OKF package was distilled from audited memory evidence. It is never published automatically.\n\n## Contents\n\n- `sources/evidence.md` records provenance.\n- `wiki/index.md` indexes learned concepts.\n- `eval/smoke.md` defines freshness checks.\n",
            candidate.title, candidate.summary
        )
        .as_bytes(),
    )?;
    write_file(
        &asset.join("sources/evidence.md"),
        format!(
            "# Evidence Ledger\n\nCandidate: `{}`\n\n{}\n",
            candidate.id,
            evidence_markdown(candidate)
        )
        .as_bytes(),
    )?;
    write_file(
        &asset.join("wiki/index.md"),
        format!(
            "# {}\n\n{}\n\n- [{}]({})\n",
            candidate.title, candidate.summary, candidate.title, concept_path
        )
        .as_bytes(),
    )?;
    write_file(
        &asset.join("wiki").join(&concept_path),
        format!(
            "---\ntype: concept\ntitle: {}\n---\n\n# {}\n\n{}\n\n## Learned Knowledge\n\n{}\n\n## Provenance\n\nSee [the evidence ledger](../../sources/evidence.md).\n",
            serde_json::to_string(&candidate.title).unwrap_or_else(|_| "\"Learned concept\"".to_string()),
            candidate.title,
            candidate.summary,
            markdown_list(&candidate.instructions),
        )
        .as_bytes(),
    )?;
    write_file(
        &asset.join("eval/smoke.md"),
        b"# OKF Smoke Evaluation\n\n1. Verify every learned statement is supported by the evidence ledger.\n2. Check for newer or conflicting memory evidence.\n3. Confirm the index links resolve and concept frontmatter declares `type: concept`.\n",
    )?;
    let name = asset
        .file_name()
        .and_then(|value| value.to_str())
        .ok_or_else(|| anyhow!("invalid OKF asset path"))?;
    let local_path = relative_workspace_path(paths, asset)?;
    let source = [
        ("readme_path", "README.md"),
        ("sources_path", "sources"),
        ("wiki_path", "wiki"),
        ("eval_path", "eval"),
    ];
    let metadata = [
        ("evolution_candidate", candidate.id.as_str()),
        ("evolution_pattern", candidate.pattern_key.as_str()),
    ];
    let acl = asset_lifecycle::render_asset_acl(AssetAclDocument {
        category: "knowledge",
        kind: Some("knowledge"),
        name,
        description: &candidate.summary,
        local_path: Some(&local_path),
        service: OsService::KnowledgeService,
        runtime: RuntimeBindingIntent {
            kind: "knowledge",
            isolation: "serving",
            runtime_kind: "a3s-knowledge-service",
            protocol: Some("okf"),
            agent_kind: None,
        },
        source: &source,
        metadata: &metadata,
    });
    write_file(&asset.join(asset_lifecycle::ASSET_ACL_PATH), acl.as_bytes())?;
    write_marker(candidate, asset, version)
}

fn evidence_markdown(candidate: &EvolutionCandidate) -> String {
    candidate
        .evidence
        .iter()
        .map(|evidence| {
            let session = evidence.session_id.as_deref().unwrap_or("unknown-session");
            let reason = evidence
                .reason
                .as_deref()
                .unwrap_or("No separate reason recorded.");
            format!(
                "- `{}` · memory `{}` · session `{}` · confidence {:.2} · {}\n  - {}\n  - Why: {}",
                evidence.timestamp.to_rfc3339(),
                evidence.memory_id,
                session,
                evidence.confidence,
                evidence.source,
                evidence.content.replace('\n', " "),
                reason.replace('\n', " ")
            )
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn numbered_list(values: &[String]) -> String {
    values
        .iter()
        .enumerate()
        .map(|(index, value)| format!("{}. {}", index + 1, value))
        .collect::<Vec<_>>()
        .join("\n")
}

fn markdown_list(values: &[String]) -> String {
    values
        .iter()
        .map(|value| format!("- {value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

fn write_marker(candidate: &EvolutionCandidate, asset: &Path, version: u32) -> anyhow::Result<()> {
    let marker = marker_path(asset, candidate.kind);
    let body = serde_json::to_vec_pretty(&json!({
        "schema": "a3s.code.evolution.asset.v1",
        "candidateId": candidate.id,
        "kind": candidate.kind.label(),
        "patternKey": candidate.pattern_key,
        "version": version,
    }))?;
    write_file(&marker, &body)
}

fn marker_path(asset: &Path, kind: EvolutionKind) -> PathBuf {
    match kind {
        EvolutionKind::Preference => asset.with_extension("evolution.json"),
        EvolutionKind::Skill | EvolutionKind::Okf => asset.join(".a3s/evolution.json"),
    }
}

pub(super) fn marker_owned_by(asset: &Path, kind: EvolutionKind, candidate_id: &str) -> bool {
    fs::read(marker_path(asset, kind))
        .ok()
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| {
            value
                .get("candidateId")
                .and_then(|id| id.as_str())
                .map(str::to_string)
        })
        .is_some_and(|id| id == candidate_id)
}

fn ensure_owned_asset(
    candidate: &EvolutionCandidate,
    asset: &Path,
    force: bool,
) -> anyhow::Result<()> {
    if marker_owned_by(asset, candidate.kind, &candidate.id) || force {
        return Ok(());
    }
    Err(anyhow!(
        "refusing to overwrite unowned asset {}",
        asset.display()
    ))
}

fn write_file(path: &Path, bytes: &[u8]) -> anyhow::Result<()> {
    if path
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        return Err(anyhow!(
            "refusing to write through symlink {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .with_context(|| format!("could not create {}", parent.display()))?;
    }
    let mut file = OpenOptions::new()
        .create(true)
        .truncate(true)
        .write(true)
        .open(path)
        .with_context(|| format!("could not write {}", path.display()))?;
    file.write_all(bytes)
        .with_context(|| format!("could not write {}", path.display()))?;
    file.sync_all()
        .with_context(|| format!("could not sync {}", path.display()))
}

fn copy_path(source: &Path, target: &Path) -> anyhow::Result<()> {
    let metadata = source
        .symlink_metadata()
        .with_context(|| format!("could not inspect {}", source.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!("refusing to copy symlink {}", source.display()));
    }
    if metadata.is_file() {
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        fs::copy(source, target).with_context(|| {
            format!(
                "could not copy {} to {}",
                source.display(),
                target.display()
            )
        })?;
        return Ok(());
    }
    if !metadata.is_dir() {
        return Err(anyhow!("unsupported asset entry {}", source.display()));
    }
    fs::create_dir_all(target).with_context(|| format!("could not create {}", target.display()))?;
    let mut entries = fs::read_dir(source)
        .with_context(|| format!("could not read {}", source.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        copy_path(&entry.path(), &target.join(entry.file_name()))?;
    }
    Ok(())
}

fn hash_path(path: &Path) -> anyhow::Result<String> {
    let mut hasher = Sha256::new();
    hash_entry(path, path, &mut hasher)?;
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn hash_entry(root: &Path, path: &Path, hasher: &mut Sha256) -> anyhow::Result<()> {
    let metadata = path
        .symlink_metadata()
        .with_context(|| format!("could not inspect {}", path.display()))?;
    if metadata.file_type().is_symlink() {
        return Err(anyhow!("refusing to hash symlink {}", path.display()));
    }
    let relative = path.strip_prefix(root).unwrap_or(path);
    hasher.update(relative.to_string_lossy().as_bytes());
    if metadata.is_file() {
        hasher.update([0]);
        hasher
            .update(fs::read(path).with_context(|| format!("could not read {}", path.display()))?);
        return Ok(());
    }
    hasher.update([1]);
    let mut entries = fs::read_dir(path)
        .with_context(|| format!("could not read {}", path.display()))?
        .collect::<Result<Vec<_>, _>>()?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        hash_entry(root, &entry.path(), hasher)?;
    }
    Ok(())
}

pub(super) fn safe_workspace_join(
    paths: &EvolutionPaths,
    relative: &str,
) -> anyhow::Result<PathBuf> {
    let relative = Path::new(relative);
    if relative.is_absolute()
        || relative.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(anyhow!(
            "invalid workspace-relative asset path `{relative:?}`"
        ));
    }
    let path = paths.workspace.join(relative);
    ensure_path_inside_workspace(paths, &path)?;
    Ok(path)
}

fn ensure_path_inside_workspace(paths: &EvolutionPaths, path: &Path) -> anyhow::Result<()> {
    if path.starts_with(&paths.workspace) && path != paths.workspace {
        Ok(())
    } else {
        Err(anyhow!(
            "asset path escapes the workspace: {}",
            path.display()
        ))
    }
}

pub(super) fn ensure_no_symlinks(paths: &EvolutionPaths, path: &Path) -> anyhow::Result<()> {
    let relative = path
        .strip_prefix(&paths.workspace)
        .map_err(|_| anyhow!("asset path is outside the workspace"))?;
    let mut current = paths.workspace.clone();
    for component in relative.components() {
        current.push(component.as_os_str());
        match current.symlink_metadata() {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                return Err(anyhow!(
                    "refusing to use symlinked evolution path {}",
                    current.display()
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => break,
            Err(error) => return Err(error).context("could not inspect evolution path"),
        }
    }
    Ok(())
}

fn relative_workspace_path(paths: &EvolutionPaths, path: &Path) -> anyhow::Result<String> {
    path.strip_prefix(&paths.workspace)
        .map(|relative| relative.to_string_lossy().replace('\\', "/"))
        .map_err(|_| anyhow!("path is outside the workspace: {}", path.display()))
}

#[cfg(test)]
#[path = "materialize_tests.rs"]
mod tests;
