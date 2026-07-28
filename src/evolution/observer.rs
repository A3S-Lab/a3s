use std::sync::Arc;

use a3s_code_core::memory::{MemoryObservation, MemoryObserver};

use super::WorkspaceEvolution;

#[derive(Clone)]
pub(crate) struct EvolutionMemoryObserver {
    evolution: WorkspaceEvolution,
}

impl EvolutionMemoryObserver {
    pub(crate) fn new(evolution: WorkspaceEvolution) -> Arc<Self> {
        Arc::new(Self { evolution })
    }
}

#[async_trait::async_trait]
impl MemoryObserver for EvolutionMemoryObserver {
    async fn on_memory_stored(&self, observation: MemoryObservation) -> anyhow::Result<()> {
        self.evolution.observe(observation).await
    }
}
