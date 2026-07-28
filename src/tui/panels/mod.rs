//! Per-panel `impl App` method groups, split out of the god-file.
//!
//! Each submodule adds an additional `impl App { … }` block. A descendant
//! module can see the ancestor `App`'s private fields, so these blocks compile
//! exactly as if they were still inline in `tui/mod.rs`.

// Team digital assets.
#[path = "assets/agent.rs"]
pub(crate) mod agent;
#[path = "assets/asset_resources.rs"]
pub(crate) mod asset_resources;
#[path = "assets/flow.rs"]
pub(crate) mod flow;
#[path = "assets/mcp.rs"]
pub(crate) mod mcp;
#[path = "assets/review.rs"]
pub(crate) mod review;
#[path = "assets/skill.rs"]
pub(crate) mod skill;

// Local personal knowledge.
#[path = "knowledge/kb.rs"]
pub(crate) mod kb;
// Shareable OKF knowledge-package assets.
#[path = "knowledge/okf.rs"]
pub(crate) mod okf;

// Local workspace.
#[path = "workspace/code_intelligence.rs"]
pub(crate) mod code_intelligence;
#[path = "workspace/files.rs"]
mod files;
#[path = "workspace/goal_engineering.rs"]
pub(crate) mod goal_engineering;
#[path = "workspace/ide.rs"]
mod ide;
#[path = "workspace/loop_engineering.rs"]
pub(crate) mod loop_engineering;
#[path = "workspace/transcript.rs"]
pub(crate) mod transcript;

// Context and memory.
#[path = "context/ctx.rs"]
pub(crate) mod ctx;
#[path = "context/evolution.rs"]
pub(crate) mod evolution;
#[path = "context/memory.rs"]
mod memory;
#[path = "context/sleep.rs"]
pub(crate) mod sleep;

// System UI.
#[path = "system/banner.rs"]
mod banner;
#[path = "system/bottom.rs"]
pub(crate) mod bottom;
#[path = "system/checkup.rs"]
pub(crate) mod checkup;
#[path = "system/effort.rs"]
mod effort;
#[path = "system/help.rs"]
mod help;
#[path = "system/history.rs"]
pub(crate) mod history;
#[path = "system/menu.rs"]
mod menu;
#[path = "system/model.rs"]
pub(crate) mod model;
#[path = "system/permissions.rs"]
pub(crate) mod permissions;
#[path = "system/plan.rs"]
mod plan;
#[path = "system/plugins.rs"]
mod plugins;
#[path = "system/queue.rs"]
pub(crate) mod queue;
#[path = "system/relay.rs"]
pub(crate) mod relay;
#[path = "system/relay_scan.rs"]
mod relay_scan;
#[path = "system/spf.rs"]
pub(crate) mod spf;
#[path = "system/tasks.rs"]
pub(crate) mod tasks;
#[path = "system/terminal.rs"]
pub(crate) mod terminal;
#[path = "system/theme.rs"]
mod theme;
