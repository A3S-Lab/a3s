mod intent;
mod model;
mod module;
mod render;
mod router;
mod service;

pub(in crate::api::code_web) use intent::{RemoteIntent, RemoteIntentError};
pub(in crate::api::code_web) use model::{
    RemoteReadQuery, RemoteReadResult, RemoteReadScope, RemoteSnapshot, RemoteTarget,
    RemoteTargetId,
};
pub(in crate::api::code_web) use module::RemoteModule;
pub(in crate::api::code_web) use render::{
    render_help, render_latest_reply, render_progress, render_sessions, render_targets,
    REMOTE_LIST_PAGE_SIZE,
};
pub(in crate::api::code_web) use router::RemoteIntentRouter;
pub(in crate::api::code_web) use service::RemoteAgentReadService;
