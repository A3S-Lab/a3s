mod materialize;
mod model;
mod observer;
mod runtime;
mod store;

use std::path::{Path, PathBuf};

use a3s_memory::MemoryStore;
use anyhow::Context;

pub(crate) use model::{
    EvolutionCandidate, EvolutionKind, EvolutionMutationResult, EvolutionOverview, EvolutionState,
};
pub(crate) use observer::EvolutionMemoryObserver;

use materialize::{materialize_candidate, rollback_candidate};
use runtime::session_preference_prompt;
use store::{
    auto_materialize_eligible, mark_session_assets_activated, observe_batch, overview,
    pending_session_reload_count, read_catalog, reject_candidate, reopen_candidate, EvolutionPaths,
};

const MAX_MEMORY_SCAN_ITEMS: usize = 10_000;

#[derive(Debug, Clone)]
pub(crate) struct WorkspaceEvolution {
    paths: EvolutionPaths,
}

impl WorkspaceEvolution {
    pub(crate) fn new(workspace: impl AsRef<Path>) -> Self {
        Self {
            paths: EvolutionPaths::new(workspace),
        }
    }

    pub(crate) async fn observe(
        &self,
        observation: a3s_code_core::memory::MemoryObservation,
    ) -> anyhow::Result<()> {
        let evolution = self.clone();
        tokio::task::spawn_blocking(move || evolution.observe_sync(vec![observation]))
            .await
            .context("evolution observer task did not complete")??;
        Ok(())
    }

    pub(crate) async fn synchronize_memory_store(
        &self,
        memory_dir: impl Into<PathBuf>,
    ) -> anyhow::Result<usize> {
        let store = a3s_memory::FileMemoryStore::new(memory_dir.into()).await?;
        let count = store.count().await?.min(MAX_MEMORY_SCAN_ITEMS);
        let items = store.get_recent(count).await?;
        let observations = items
            .into_iter()
            .flat_map(memory_item_observations)
            .collect::<Vec<_>>();
        let observed = observations.len();
        let evolution = self.clone();
        tokio::task::spawn_blocking(move || evolution.observe_sync(observations))
            .await
            .context("evolution memory scan task did not complete")??;
        Ok(observed)
    }

    pub(crate) async fn overview(&self) -> anyhow::Result<EvolutionOverview> {
        let paths = self.paths.clone();
        tokio::task::spawn_blocking(move || overview(&paths))
            .await
            .context("evolution overview task did not complete")?
    }

    pub(crate) async fn materialize(
        &self,
        id: impl Into<String>,
        force: bool,
    ) -> anyhow::Result<EvolutionMutationResult> {
        let paths = self.paths.clone();
        let id = id.into();
        tokio::task::spawn_blocking(move || materialize_candidate(&paths, &id, force, false))
            .await
            .context("evolution materialization task did not complete")?
    }

    pub(crate) async fn reject(
        &self,
        id: impl Into<String>,
        reason: Option<String>,
    ) -> anyhow::Result<EvolutionCandidate> {
        let paths = self.paths.clone();
        let id = id.into();
        tokio::task::spawn_blocking(move || reject_candidate(&paths, &id, reason))
            .await
            .context("evolution rejection task did not complete")?
    }

    pub(crate) async fn reopen(&self, id: impl Into<String>) -> anyhow::Result<EvolutionCandidate> {
        let paths = self.paths.clone();
        let id = id.into();
        tokio::task::spawn_blocking(move || reopen_candidate(&paths, &id))
            .await
            .context("evolution reopen task did not complete")?
    }

    pub(crate) async fn rollback(
        &self,
        id: impl Into<String>,
        target_version: Option<u32>,
    ) -> anyhow::Result<EvolutionMutationResult> {
        let paths = self.paths.clone();
        let id = id.into();
        tokio::task::spawn_blocking(move || rollback_candidate(&paths, &id, target_version))
            .await
            .context("evolution rollback task did not complete")?
    }

    pub(crate) async fn pending_session_reload_count(&self) -> anyhow::Result<usize> {
        let paths = self.paths.clone();
        tokio::task::spawn_blocking(move || pending_session_reload_count(&paths))
            .await
            .context("evolution pending-reload inspection did not complete")?
    }

    pub(crate) async fn mark_session_assets_activated(&self) -> anyhow::Result<usize> {
        let paths = self.paths.clone();
        tokio::task::spawn_blocking(move || mark_session_assets_activated(&paths))
            .await
            .context("evolution activation update did not complete")?
    }

    pub(crate) fn session_preference_prompt(&self) -> anyhow::Result<Option<String>> {
        session_preference_prompt(&self.paths)
    }

    fn observe_sync(
        &self,
        observations: Vec<a3s_code_core::memory::MemoryObservation>,
    ) -> anyhow::Result<()> {
        let changed = observe_batch(&self.paths, observations)?;
        if changed.is_empty() {
            return Ok(());
        }
        let catalog = read_catalog(&self.paths)?;
        for candidate in catalog
            .candidates
            .iter()
            .filter(|candidate| changed.contains(&candidate.id))
            .filter(|candidate| auto_materialize_eligible(candidate))
        {
            if let Err(error) = materialize_candidate(&self.paths, &candidate.id, false, true) {
                tracing::warn!(
                    candidate_id = %candidate.id,
                    %error,
                    "automatic local evolution materialization was deferred"
                );
            }
        }
        Ok(())
    }
}

fn memory_item_observations(
    item: a3s_memory::MemoryItem,
) -> Vec<a3s_code_core::memory::MemoryObservation> {
    let mut ids = vec![item.id.clone()];
    if let Some(duplicates) = item.metadata.get("duplicate_ids") {
        for id in duplicates.split(',').map(str::trim) {
            if !id.is_empty() && !ids.iter().any(|seen| seen == id) {
                ids.push(id.to_string());
            }
        }
    }
    ids.into_iter()
        .map(|id| {
            let mut incoming = item.clone();
            incoming
                .metadata
                .insert("last_observation_id".to_string(), id);
            a3s_code_core::memory::MemoryObservation {
                incoming,
                stored: item.clone(),
                merged: false,
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use a3s_memory::{MemoryItem, MemoryType};

    #[tokio::test]
    async fn file_memory_scan_is_idempotent() {
        let temp = tempfile::tempdir().unwrap();
        let memory_dir = temp.path().join("memory");
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let store = a3s_memory::FileMemoryStore::new(&memory_dir).await.unwrap();
        let item = MemoryItem::new("Prefer concise evidence-backed responses in this workspace.")
            .with_type(MemoryType::Semantic)
            .with_importance(0.92)
            .with_metadata("source", "preference")
            .with_metadata("scope", "user")
            .with_metadata("confidence", "0.95")
            .with_metadata("evolution_schema", "a3s.evolution.signal.v1")
            .with_metadata("evolution_kind", "preference")
            .with_metadata("evolution_pattern", "preference.response.concise")
            .with_metadata("evolution_title", "Concise evidence-backed responses")
            .with_metadata(
                "evolution_summary",
                "Keep responses concise while retaining concrete supporting evidence.",
            )
            .with_metadata(
                "evolution_instructions",
                r#"["Lead with the outcome.","Keep supporting evidence concrete and concise."]"#,
            );
        store.store(item).await.unwrap();
        let evolution = WorkspaceEvolution::new(&workspace);
        evolution
            .synchronize_memory_store(&memory_dir)
            .await
            .unwrap();
        evolution
            .synchronize_memory_store(&memory_dir)
            .await
            .unwrap();
        let overview = evolution.overview().await.unwrap();
        assert_eq!(overview.candidates.len(), 1);
        assert_eq!(overview.candidates[0].occurrences, 1);
    }

    #[tokio::test]
    async fn three_qualifying_observations_materialize_a_skill_and_require_refresh() {
        let temp = tempfile::tempdir().unwrap();
        let workspace = temp.path().join("workspace");
        tokio::fs::create_dir_all(&workspace).await.unwrap();
        let evolution = WorkspaceEvolution::new(&workspace);

        for (id, session) in [
            ("observation-one", "session-one"),
            ("observation-two", "session-two"),
            ("observation-three", "session-two"),
        ] {
            let item = MemoryItem::new(
                "Run the focused persistence checks after changing memory storage.",
            )
            .with_type(MemoryType::Procedural)
            .with_importance(0.91)
            .with_metadata("source", "workflow")
            .with_metadata("scope", "workspace")
            .with_metadata("workspace", workspace.display().to_string())
            .with_metadata("session_id", session)
            .with_metadata("confidence", "0.95")
            .with_metadata("evolution_schema", "a3s.evolution.signal.v1")
            .with_metadata("evolution_kind", "skill")
            .with_metadata("evolution_pattern", "workflow.memory.persistence-checks")
            .with_metadata("evolution_title", "Memory persistence checks")
            .with_metadata(
                "evolution_summary",
                "Run focused persistence verification after memory storage changes.",
            )
            .with_metadata(
                "evolution_instructions",
                r#"["Run the focused memory persistence tests.","Inspect the durable files after the test."]"#,
            );
            let mut incoming = item.clone();
            incoming.id = id.to_string();
            evolution
                .observe(a3s_code_core::memory::MemoryObservation {
                    incoming: incoming.clone(),
                    stored: incoming,
                    merged: false,
                })
                .await
                .unwrap();
        }

        let overview = evolution.overview().await.unwrap();
        assert_eq!(overview.candidates.len(), 1);
        let candidate = &overview.candidates[0];
        assert_eq!(candidate.kind, EvolutionKind::Skill);
        assert_eq!(candidate.state, EvolutionState::Materialized);
        assert_eq!(candidate.current_version, Some(1));
        assert!(candidate.activation_pending);
        assert_eq!(evolution.pending_session_reload_count().await.unwrap(), 1);
        let asset = workspace.join(candidate.asset_path.as_deref().unwrap());
        assert!(asset.join("SKILL.md").is_file());
        assert!(candidate.versions[0].automatic);
    }
}
