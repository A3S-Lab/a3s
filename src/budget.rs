//! Central budget policy for a3s code surfaces.
//!
//! Keep effort-level numbers here instead of scattering one-off limits through
//! the TUI, Code Web, and workflow prompts. Callers derive a concrete
//! [`BudgetPlan`] for the current context window and workload, then apply it to
//! `SessionOptions` or workflow inputs.

use serde_json::{json, Value};

const DEFAULT_CONTEXT_LIMIT: u32 = 200_000;
#[cfg(test)]
pub(crate) const AUTO_COMPACT_THRESHOLD: f64 = crate::config::DEFAULT_AUTO_COMPACT_THRESHOLD;
const DEEP_RESEARCH_MIN_TOOL_ROUNDS: usize = 1_200;
const DEEP_RESEARCH_MIN_CONTINUATION_TURNS: u32 = 12;
const DEEP_RESEARCH_MIN_PARALLEL_TASKS: usize = 4;
const DEEP_RESEARCH_MIN_CHILD_STEPS: usize = 200;
const DEEP_RESEARCH_MIN_WORKFLOW_TOOL_CALLS: usize = 300;
const DEEP_RESEARCH_MIN_WORKFLOW_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
/// Provider-facing child concurrency is an admission window, not a reasoning
/// budget. Keep it at Core's proven default and schedule larger workloads in
/// waves so Ultracode cannot burst 32 requests through one local account.
const MAX_INTERACTIVE_PROVIDER_CONCURRENCY: usize = 8;

pub(crate) const DEFAULT_TUI_EFFORT_INDEX: usize = 2;
pub(crate) const DEFAULT_CODE_WEB_EFFORT_ID: &str = "medium";
pub(crate) const ULTRACODE_INDEX: usize = 5;

const EFFORT_LOW: &str = "\
[effort: low] Favor speed and minimalism. Answer directly, make the smallest \
change that works (reading enough surrounding code to change it safely), and \
keep verification proportionate: still run the narrowest build/test/type-check \
that covers what you touched — just don't add checks or scope the task didn't \
warrant. Don't gold-plate.";
const EFFORT_HIGH: &str = "\
[effort: high] Favor depth. Reason through the approach before acting. After \
changes, verify the narrow path you touched (build / test / type-check) and \
check the obvious edge cases, then re-read your own diff for correctness before \
finishing.";
const EFFORT_XHIGH: &str = "\
[effort: xhigh] Work rigorously. Before choosing an approach, weigh at \
least one alternative. Verify thoroughly — run the relevant tests/build, probe \
edge cases and failure modes, and confirm the change actually does what was \
asked. Do a self-review pass for correctness and simplicity before concluding.";
const EFFORT_MAX: &str = "\
[effort: max] Maximum rigor; prefer correctness and completeness over speed. \
Decompose the problem, compare alternatives, and implement the strongest \
solution. Verify exhaustively: tests, build, edge cases, and boundary / \
adversarial inputs. Finish with a self-critique pass that actively hunts for \
what you may have missed or gotten wrong, and fix it before concluding.";
const ULTRACODE_GUIDELINES: &str = "\
[ultracode] Dynamic-workflow mode is available — you decide whether a turn needs \
it. Match the effort to the task: answer trivial or conversational input (a \
greeting, a single question, a one-step edit) directly, with no plan and no \
fan-out. When a task genuinely needs a dynamic workflow, call the \
`dynamic_workflow` tool with one sandboxed JavaScript PTC workflow script. In \
that script, return A3S Flow commands for workflow replay. Use PTC `ctx` tools \
inside ordinary steps (`ctx.read`, `ctx.grep`, `ctx.tool(\"runtime\", ...)` when \
the login-gated runtime tool exists). For local parallel subagent fan-out, \
schedule a Flow step with `step_name: \"parallel_task\"`; do not call \
`parallel_task` from PTC. Keep child prompts bounded and evidence-oriented, then \
synthesize results before completing the workflow.";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BudgetWorkload {
    Interactive,
    DeepResearch,
}

/// One user-facing effort level plus all derived hard limits.
#[derive(Clone, Copy, Debug)]
pub(crate) struct BudgetProfile {
    pub(crate) id: &'static str,
    pub(crate) label: &'static str,
    pub(crate) display_label: &'static str,
    pub(crate) description: &'static str,
    pub(crate) thinking_budget: usize,
    pub(crate) max_tool_rounds: usize,
    pub(crate) max_continuation_turns: u32,
    pub(crate) max_parallel_tasks: usize,
    pub(crate) deep_research_child_steps: usize,
    pub(crate) workflow_max_tool_calls: usize,
    pub(crate) workflow_max_output_bytes: usize,
    pub(crate) guideline: Option<&'static str>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct BudgetPlan {
    pub(crate) effort_id: &'static str,
    pub(crate) thinking_budget: usize,
    pub(crate) max_tool_rounds: usize,
    pub(crate) max_continuation_turns: u32,
    pub(crate) max_parallel_tasks: usize,
    pub(crate) auto_compact_threshold: f64,
    pub(crate) deep_research_child_steps: usize,
    pub(crate) workflow_max_tool_calls: usize,
    pub(crate) workflow_max_output_bytes: usize,
}

pub(crate) const EFFORT_LEVELS: &[BudgetProfile] = &[
    BudgetProfile {
        id: "low",
        label: "low",
        display_label: "Low",
        description: "Fast, focused edits with narrow verification.",
        thinking_budget: 2_048,
        max_tool_rounds: 240,
        max_continuation_turns: 4,
        max_parallel_tasks: 4,
        deep_research_child_steps: 80,
        workflow_max_tool_calls: 120,
        workflow_max_output_bytes: 1024 * 1024,
        guideline: Some(EFFORT_LOW),
    },
    BudgetProfile {
        id: "medium",
        label: "medium",
        display_label: "Medium",
        description: "Balanced default behavior with room for long tasks.",
        thinking_budget: 8_192,
        max_tool_rounds: 800,
        max_continuation_turns: 8,
        max_parallel_tasks: 8,
        deep_research_child_steps: 160,
        workflow_max_tool_calls: 240,
        workflow_max_output_bytes: 2 * 1024 * 1024,
        guideline: None,
    },
    BudgetProfile {
        id: "high",
        label: "high",
        display_label: "High",
        description: "Deeper reasoning with stronger verification.",
        thinking_budget: 16_384,
        max_tool_rounds: 1_200,
        max_continuation_turns: 12,
        max_parallel_tasks: MAX_INTERACTIVE_PROVIDER_CONCURRENCY,
        deep_research_child_steps: 240,
        workflow_max_tool_calls: 360,
        workflow_max_output_bytes: 4 * 1024 * 1024,
        guideline: Some(EFFORT_HIGH),
    },
    BudgetProfile {
        id: "xhigh",
        label: "xhigh",
        display_label: "XHigh",
        description: "Rigorous alternative analysis and edge-case checks.",
        thinking_budget: 32_768,
        max_tool_rounds: 1_800,
        max_continuation_turns: 16,
        max_parallel_tasks: MAX_INTERACTIVE_PROVIDER_CONCURRENCY,
        deep_research_child_steps: 320,
        workflow_max_tool_calls: 480,
        workflow_max_output_bytes: 6 * 1024 * 1024,
        guideline: Some(EFFORT_XHIGH),
    },
    BudgetProfile {
        id: "max",
        label: "max",
        display_label: "Max",
        description: "Maximum completeness and self-review.",
        thinking_budget: 65_536,
        max_tool_rounds: 2_400,
        max_continuation_turns: 24,
        max_parallel_tasks: MAX_INTERACTIVE_PROVIDER_CONCURRENCY,
        deep_research_child_steps: 480,
        workflow_max_tool_calls: 720,
        workflow_max_output_bytes: 8 * 1024 * 1024,
        guideline: Some(EFFORT_MAX),
    },
    BudgetProfile {
        id: "ultracode",
        label: "ultracode",
        display_label: "Ultracode",
        description: "Workflow-grade decomposition and local fan-out when useful.",
        thinking_budget: 65_536,
        max_tool_rounds: 3_200,
        max_continuation_turns: 32,
        max_parallel_tasks: MAX_INTERACTIVE_PROVIDER_CONCURRENCY,
        deep_research_child_steps: 640,
        workflow_max_tool_calls: 960,
        workflow_max_output_bytes: 12 * 1024 * 1024,
        guideline: Some(ULTRACODE_GUIDELINES),
    },
];

pub(crate) fn effort_profile_by_index(index: usize) -> &'static BudgetProfile {
    &EFFORT_LEVELS[index.min(EFFORT_LEVELS.len().saturating_sub(1))]
}

/// Runtime-driven delegation is an orchestration capability, not a reasoning
/// level. Keep Codex's native low/medium/high/xhigh/max effort mapping intact
/// and enable automatic child-agent fan-out only for the explicit ultracode
/// product mode. Manual `task` and `parallel_task` calls remain available.
pub(crate) fn effort_uses_automatic_delegation(index: usize) -> bool {
    index == ULTRACODE_INDEX
}

pub(crate) fn normalize_effort(value: &str) -> Option<&'static BudgetProfile> {
    let value = value.trim().to_ascii_lowercase();
    EFFORT_LEVELS.iter().find(|profile| profile.id == value)
}

pub(crate) fn budget_plan_for_effort_index(
    index: usize,
    context_limit: Option<u32>,
    workload: BudgetWorkload,
) -> BudgetPlan {
    budget_plan_for_profile(effort_profile_by_index(index), context_limit, workload)
}

pub(crate) fn budget_plan_for_effort_id(
    effort: &str,
    context_limit: Option<u32>,
    workload: BudgetWorkload,
) -> BudgetPlan {
    let profile = normalize_effort(effort)
        .or_else(|| normalize_effort(DEFAULT_CODE_WEB_EFFORT_ID))
        .expect("default effort profile must exist");
    budget_plan_for_profile(profile, context_limit, workload)
}

pub(crate) fn budget_plan_for_profile(
    profile: &'static BudgetProfile,
    _context_limit: Option<u32>,
    workload: BudgetWorkload,
) -> BudgetPlan {
    let mut plan = BudgetPlan {
        effort_id: profile.id,
        thinking_budget: profile.thinking_budget,
        max_tool_rounds: profile.max_tool_rounds,
        max_continuation_turns: profile.max_continuation_turns,
        max_parallel_tasks: profile.max_parallel_tasks,
        auto_compact_threshold: crate::config::DEFAULT_AUTO_COMPACT_THRESHOLD,
        deep_research_child_steps: profile.deep_research_child_steps,
        workflow_max_tool_calls: profile.workflow_max_tool_calls,
        workflow_max_output_bytes: profile.workflow_max_output_bytes,
    };
    if workload == BudgetWorkload::DeepResearch {
        plan.max_parallel_tasks = plan
            .max_parallel_tasks
            .max(DEEP_RESEARCH_MIN_PARALLEL_TASKS);
        plan.max_tool_rounds = plan.max_tool_rounds.max(DEEP_RESEARCH_MIN_TOOL_ROUNDS);
        plan.max_continuation_turns = plan
            .max_continuation_turns
            .max(DEEP_RESEARCH_MIN_CONTINUATION_TURNS);
        plan.deep_research_child_steps = plan
            .deep_research_child_steps
            .max(DEEP_RESEARCH_MIN_CHILD_STEPS);
        plan.workflow_max_tool_calls = plan
            .workflow_max_tool_calls
            .max(DEEP_RESEARCH_MIN_WORKFLOW_TOOL_CALLS);
        plan.workflow_max_output_bytes = plan
            .workflow_max_output_bytes
            .max(DEEP_RESEARCH_MIN_WORKFLOW_OUTPUT_BYTES);
    }
    plan
}

/// Resolve a model's usable context window: the declared limit, or a sane
/// default when it is missing/zero.
pub(crate) fn resolve_ctx_limit(raw: Option<u32>) -> u32 {
    match raw {
        Some(c) if c > 0 => c,
        _ => DEFAULT_CONTEXT_LIMIT,
    }
}

pub(crate) fn context_limit_for_model(
    model: &str,
    declared_context: Option<u32>,
    account_context: Option<u32>,
) -> u32 {
    resolve_ctx_limit(
        declared_context
            .filter(|context| *context > 0)
            .or_else(|| account_context.filter(|context| *context > 0))
            .or_else(|| inferred_context_limit_for_model(model)),
    )
}

pub(crate) fn inferred_context_limit_for_model(model: &str) -> Option<u32> {
    let model = model.trim().to_ascii_lowercase();
    if let Some(limit) = context_suffix_limit(&model) {
        return Some(limit);
    }

    // Configured/gateway-reported limits win. These are fallbacks for account
    // models, gateway models, or ad-hoc model ids that do not come from config.
    if model.contains("claude") {
        return Some(200_000);
    }
    if model.contains("gpt-5") || model.contains("gpt-4.1") {
        return Some(1_000_000);
    }
    if model.contains("o1") || model.contains("o3") || model.contains("o4") {
        return Some(200_000);
    }
    if model.contains("gpt-4o") || model.contains("gpt-4") || model.contains("glm") {
        return Some(resolve_ctx_limit(None));
    }

    None
}

fn context_suffix_limit(model: &str) -> Option<u32> {
    if !model.ends_with(']') {
        return None;
    }
    let start = model.rfind('[')?;
    let suffix = model.get(start + 1..model.len().checked_sub(1)?)?;
    if suffix.is_empty() {
        return None;
    }
    let (number, scale) = suffix.split_at(suffix.len().saturating_sub(1));
    let base = number.parse::<u32>().ok()?;
    match scale {
        "k" => base.checked_mul(1_000),
        "m" => base.checked_mul(1_000_000),
        _ => suffix.parse::<u32>().ok(),
    }
}

pub(crate) fn effort_levels_json() -> Vec<Value> {
    EFFORT_LEVELS.iter().map(effort_profile_json).collect()
}

pub(crate) fn effort_profile_json(profile: &BudgetProfile) -> Value {
    json!({
        "id": profile.id,
        "label": profile.display_label,
        "description": profile.description,
        "thinkingBudget": profile.thinking_budget,
        "maxToolRounds": profile.max_tool_rounds,
        "maxContinuationTurns": profile.max_continuation_turns,
        "maxParallelTasks": profile.max_parallel_tasks,
        "deepResearchChildSteps": profile.deep_research_child_steps,
        "workflowMaxToolCalls": profile.workflow_max_tool_calls,
        "workflowMaxOutputBytes": profile.workflow_max_output_bytes,
        "ultracode": profile.id == "ultracode",
    })
}

pub(crate) fn budget_plan_json(plan: &BudgetPlan) -> Value {
    json!({
        "effort": plan.effort_id,
        "thinkingBudget": plan.thinking_budget,
        "maxToolRounds": plan.max_tool_rounds,
        "maxContinuationTurns": plan.max_continuation_turns,
        "maxParallelTasks": plan.max_parallel_tasks,
        "autoCompactThreshold": plan.auto_compact_threshold,
        "deepResearchChildSteps": plan.deep_research_child_steps,
        "workflowMaxToolCalls": plan.workflow_max_tool_calls,
        "workflowMaxOutputBytes": plan.workflow_max_output_bytes,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effort_budgets_scale_monotonically() {
        assert_eq!(ULTRACODE_INDEX, EFFORT_LEVELS.len() - 1);
        assert_eq!(EFFORT_LEVELS[ULTRACODE_INDEX].id, "ultracode");
        for window in EFFORT_LEVELS.windows(2) {
            assert!(window[1].max_tool_rounds >= window[0].max_tool_rounds);
            assert!(window[1].max_continuation_turns >= window[0].max_continuation_turns);
            assert!(window[1].thinking_budget >= window[0].thinking_budget);
            assert!(window[1].deep_research_child_steps >= window[0].deep_research_child_steps);
            assert!(window[1].max_parallel_tasks >= window[0].max_parallel_tasks);
        }
        assert!(EFFORT_LEVELS[1].guideline.is_none());
    }

    #[test]
    fn automatic_delegation_is_reserved_for_ultracode() {
        for (index, profile) in EFFORT_LEVELS.iter().enumerate() {
            assert_eq!(
                effort_uses_automatic_delegation(index),
                profile.id == "ultracode",
                "unexpected automatic delegation policy for {}",
                profile.id
            );
        }
        assert!(!effort_uses_automatic_delegation(EFFORT_LEVELS.len()));
    }

    #[test]
    fn reasoning_depth_never_expands_the_interactive_provider_window_past_core_default() {
        for profile in EFFORT_LEVELS {
            assert!(
                profile.max_parallel_tasks <= MAX_INTERACTIVE_PROVIDER_CONCURRENCY,
                "{} would burst {} concurrent child runs through one provider account",
                profile.id,
                profile.max_parallel_tasks
            );
        }
        assert_eq!(
            EFFORT_LEVELS[ULTRACODE_INDEX].max_parallel_tasks, MAX_INTERACTIVE_PROVIDER_CONCURRENCY,
            "Ultracode should schedule more work in bounded waves, not raise provider concurrency"
        );
    }

    #[test]
    fn deep_research_budget_has_a_safe_child_floor() {
        let low = budget_plan_for_effort_id("low", Some(128_000), BudgetWorkload::DeepResearch);
        assert!(low.deep_research_child_steps >= DEEP_RESEARCH_MIN_CHILD_STEPS);
        assert!(low.max_tool_rounds >= DEEP_RESEARCH_MIN_TOOL_ROUNDS);
        assert!(low.max_parallel_tasks >= DEEP_RESEARCH_MIN_PARALLEL_TASKS);
        assert!(low.workflow_max_tool_calls >= DEEP_RESEARCH_MIN_WORKFLOW_TOOL_CALLS);
        assert!(low.workflow_max_output_bytes >= DEEP_RESEARCH_MIN_WORKFLOW_OUTPUT_BYTES);
        assert!(low.deep_research_child_steps > 30);
        assert!(low.workflow_max_tool_calls > 30);
    }

    #[test]
    fn auto_compact_uses_the_model_specific_core_window() {
        assert!((AUTO_COMPACT_THRESHOLD - 0.85).abs() < f64::EPSILON);
    }

    #[test]
    fn model_context_prefers_declared_then_account_then_inferred() {
        assert_eq!(
            context_limit_for_model("openai/gpt-5", Some(256_000), Some(512_000)),
            256_000
        );
        assert_eq!(
            context_limit_for_model("gpt-5.5", Some(0), Some(512_000)),
            512_000
        );
        assert_eq!(
            context_limit_for_model("claude-sonnet-4", None, None),
            200_000
        );
        assert_eq!(
            context_limit_for_model("unknown[1m]", None, None),
            1_000_000
        );
        assert_eq!(context_limit_for_model("unknown", None, None), 200_000);
        assert_eq!(resolve_ctx_limit(None), 200_000);
    }
}
