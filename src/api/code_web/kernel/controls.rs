use serde_json::{json, Value};

use crate::api::code_web::state::{CodeWebSessionControls, CodeWebSessionSettings};
use crate::budget::{self, BudgetProfile, BudgetWorkload};

#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct CodeWebContextUsage {
    pub(super) estimated_tokens: usize,
    pub(super) limit_tokens: u32,
    pub(super) history_messages: usize,
    pub(super) compacted: bool,
    pub(super) compact_summary: Option<String>,
}

pub(super) fn effort_levels_json() -> Vec<Value> {
    budget::effort_levels_json()
}

pub(super) fn normalize_effort(value: &str) -> Option<&'static BudgetProfile> {
    budget::normalize_effort(value)
}

pub(super) fn controls_json(
    session_id: &str,
    controls: &CodeWebSessionControls,
    settings: &CodeWebSessionSettings,
    context: &CodeWebContextUsage,
    context_limit: Option<u32>,
    auto_compact_threshold: f64,
) -> Value {
    let profile = normalize_effort(&controls.effort)
        .or_else(|| normalize_effort(budget::DEFAULT_CODE_WEB_EFFORT_ID))
        .expect("medium effort profile must exist");
    let mut plan =
        budget::budget_plan_for_profile(profile, context_limit, BudgetWorkload::Interactive);
    plan.auto_compact_threshold = auto_compact_threshold;
    let goal_active = controls.goal.is_some();
    let planning_forced = goal_active || settings.permission_mode == "plan";
    json!({
        "sessionId": session_id,
        "effort": profile.id,
        "goal": controls.goal.clone(),
        "goalState": controls.goal_run.clone(),
        "planningMode": if planning_forced {
            "enabled"
        } else {
            settings.planning_mode.as_deref().unwrap_or("auto")
        },
        "goalTracking": goal_active || settings.goal_tracking.unwrap_or(false),
        "context": {
            "estimatedTokens": context.estimated_tokens,
            "limitTokens": context.limit_tokens,
            "percent": context_percent(context.estimated_tokens, context.limit_tokens),
            "historyMessages": context.history_messages,
            "compacted": context.compacted,
            "compactSummary": context.compact_summary.clone(),
        },
        "effortLevel": budget::effort_profile_json(profile),
        "effortLevels": effort_levels_json(),
        "budget": budget::budget_plan_json(&plan),
    })
}

fn context_percent(estimated_tokens: usize, limit_tokens: u32) -> f64 {
    if limit_tokens == 0 {
        return 0.0;
    }
    (estimated_tokens as f64 / f64::from(limit_tokens)).clamp(0.0, 1.0)
}

pub(super) fn normalize_goal(value: &str) -> Option<String> {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    if normalized.is_empty() {
        None
    } else {
        Some(normalized)
    }
}

pub(super) fn compose_controlled_prompt(
    controls: &CodeWebSessionControls,
    user_prompt: &str,
) -> String {
    if user_prompt.trim_start().starts_with('/') {
        return user_prompt.to_string();
    }

    let mut directives = Vec::new();
    if let Some(profile) = normalize_effort(&controls.effort) {
        if let Some(guideline) = profile.guideline {
            directives.push(guideline.to_string());
        }
    }
    if let Some(goal) = controls.goal.as_deref().and_then(normalize_goal) {
        directives.push(format!(
            "[session goal] Keep this north-star goal in mind for this turn: {goal}"
        ));
    }

    if directives.is_empty() {
        user_prompt.to_string()
    } else {
        format!(
            "{}\n\nUser request:\n{}",
            directives.join("\n\n"),
            user_prompt
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget::{BudgetWorkload, DEFAULT_CODE_WEB_EFFORT_ID};

    #[test]
    fn controls_json_exposes_budget_for_selected_effort() {
        let controls = CodeWebSessionControls {
            effort: "xhigh".to_string(),
            goal: Some("finish the migration".to_string()),
            goal_run: None,
        };
        let settings = CodeWebSessionSettings {
            planning_mode: Some("disabled".to_string()),
            goal_tracking: Some(false),
            ..CodeWebSessionSettings::default()
        };
        let context = CodeWebContextUsage {
            estimated_tokens: 8_192,
            limit_tokens: 32_768,
            history_messages: 12,
            compacted: true,
            compact_summary: Some("Earlier work was compacted".to_string()),
        };
        let value = controls_json(
            "session-1",
            &controls,
            &settings,
            &context,
            Some(32_768),
            0.9,
        );
        let budget =
            budget::budget_plan_for_effort_id("xhigh", Some(32_768), BudgetWorkload::Interactive);

        assert_eq!(value["sessionId"], "session-1");
        assert_eq!(value["effort"], "xhigh");
        assert_eq!(value["goal"], "finish the migration");
        assert_eq!(value["planningMode"], "enabled");
        assert_eq!(value["goalTracking"], true);
        assert_eq!(value["context"]["estimatedTokens"], 8_192);
        assert_eq!(value["context"]["limitTokens"], 32_768);
        assert_eq!(value["context"]["percent"], 0.25);
        assert_eq!(value["context"]["historyMessages"], 12);
        assert_eq!(value["context"]["compacted"], true);
        assert_eq!(
            value["context"]["compactSummary"],
            "Earlier work was compacted"
        );
        assert_eq!(
            value["budget"]["maxToolRounds"].as_u64(),
            Some(budget.max_tool_rounds as u64)
        );
        assert_eq!(
            value["budget"]["maxParallelTasks"].as_u64(),
            Some(budget.max_parallel_tasks as u64)
        );
        assert_eq!(
            value["budget"]["workflowMaxToolCalls"].as_u64(),
            Some(budget.workflow_max_tool_calls as u64)
        );
        assert_eq!(value["budget"]["autoCompactThreshold"].as_f64(), Some(0.9));
    }

    #[test]
    fn controls_json_falls_back_to_default_budget_for_unknown_effort() {
        let controls = CodeWebSessionControls {
            effort: "warp-drive".to_string(),
            goal: None,
            goal_run: None,
        };
        let value = controls_json(
            "session-2",
            &controls,
            &CodeWebSessionSettings::default(),
            &CodeWebContextUsage::default(),
            None,
            0.85,
        );
        let budget = budget::budget_plan_for_effort_id(
            DEFAULT_CODE_WEB_EFFORT_ID,
            None,
            BudgetWorkload::Interactive,
        );

        assert_eq!(value["effort"], DEFAULT_CODE_WEB_EFFORT_ID);
        assert_eq!(value["planningMode"], "auto");
        assert_eq!(value["goalTracking"], false);
        assert_eq!(value["context"]["percent"], 0.0);
        assert_eq!(
            value["budget"]["maxToolRounds"].as_u64(),
            Some(budget.max_tool_rounds as u64)
        );
        assert_eq!(
            value["budget"]["maxParallelTasks"].as_u64(),
            Some(budget.max_parallel_tasks as u64)
        );
    }

    #[test]
    fn controls_json_reports_planning_enabled_for_plan_execution_mode() {
        let controls = CodeWebSessionControls::default();
        let settings = CodeWebSessionSettings {
            permission_mode: "plan".to_string(),
            planning_mode: Some("disabled".to_string()),
            ..CodeWebSessionSettings::default()
        };
        let value = controls_json(
            "session-plan",
            &controls,
            &settings,
            &CodeWebContextUsage::default(),
            None,
            0.85,
        );

        assert_eq!(value["planningMode"], "enabled");
        assert_eq!(value["goalTracking"], false);
    }
}
