pub(crate) mod auto_compact;
pub(crate) mod compactor;
pub(crate) mod model_context;
pub(crate) mod projection;

pub(crate) use compactor::{compact_history, compact_timeline, MANUAL_COMPACT_TIMEOUT};
pub(crate) use model_context::{ContextJsonStore, ModelContextState};
pub(crate) use projection::{
    append_compact_summary, is_compact_message, project_messages_for_llm,
    project_messages_for_llm_with_budget, ProjectionBudget, A3S_COMPACT_ROLE,
};
