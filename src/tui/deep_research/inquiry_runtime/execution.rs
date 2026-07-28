//! Bounded tool execution for the standalone DeepResearch engine adapter.

use a3s_code_core::{AgentEvent, AgentSession, ToolCallResult};
use serde::de::DeserializeOwned;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use super::super::{
    deep_research_canonical_workflow_output, recover_deep_research_bootstrap_acquisition_from_store,
};
use super::{EvidenceFirstRunClock, DURABLE_GENERATION_WORKFLOW_SOURCE};

struct AbortInnerToolOnDrop(Option<tokio::task::AbortHandle>);

impl AbortInnerToolOnDrop {
    fn disarm(&mut self) {
        self.0 = None;
    }
}

impl Drop for AbortInnerToolOnDrop {
    fn drop(&mut self) {
        if let Some(abort) = self.0.take() {
            abort.abort();
        }
    }
}

include!("execution/tools.rs");
