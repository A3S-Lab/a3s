//! Stable, source-free summaries for sandboxed `program` calls.
//!
//! Large PTC sources are implementation details, not model reasoning. This
//! module derives a bounded user-facing intent from structured invocation
//! inputs and actual completed call metadata without evaluating JavaScript.

use a3s_tui::style::strip_ansi;
use serde_json::Value;

const MAX_INLINE_CHARS: usize = 180;
const MAX_QUERY_CHARS: usize = 96;
const MAX_TOOL_GROUPS: usize = 4;
const MAX_CALL_RECORDS_TO_SCAN: usize = 256;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProgramPreview {
    pub(super) intent: String,
    pub(super) details: Vec<ProgramPreviewDetail>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProgramPreviewDetail {
    pub(super) label: &'static str,
    pub(super) value: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct ProgramCallDigest {
    pub(super) text: String,
    pub(super) has_failure: bool,
}

pub(super) fn summarize_program_args(args: Option<&Value>) -> Option<ProgramPreview> {
    let args = args?;
    let payload = args.get("inputs").or_else(|| args.get("input"));
    let execution_input = payload.and_then(|value| value.get("input")).or(payload);

    let explicit_intent = first_inline_text(
        args,
        &["/intent", "/inputs/intent", "/inputs/input/intent"],
        MAX_INLINE_CHARS,
    )
    .or_else(|| {
        payload
            .and_then(|value| first_object_text(value, &["description", "title"], MAX_INLINE_CHARS))
    })
    .or_else(|| {
        execution_input
            .and_then(|value| first_object_text(value, &["description", "title"], MAX_INLINE_CHARS))
    });
    let query = execution_input
        .and_then(|value| first_object_text(value, &["query", "q"], MAX_QUERY_CHARS))
        .or_else(|| {
            payload.and_then(|value| first_object_text(value, &["query", "q"], MAX_QUERY_CHARS))
        });
    let deep_research = execution_input.is_some_and(is_deep_research_input);

    let intent = explicit_intent.unwrap_or_else(|| {
        if deep_research {
            query
                .as_deref()
                .map(|query| {
                    format!("DeepResearch “{query}”: execute the coverage-driven evidence pipeline")
                })
                .unwrap_or_else(|| {
                    "Execute the coverage-driven DeepResearch evidence pipeline".to_string()
                })
        } else if let Some(query) = query.as_deref() {
            format!("Process “{query}” with a sandboxed script")
        } else if let Some(path) = args.get("path").and_then(Value::as_str) {
            format!(
                "Run workspace script {}",
                clean_inline(path, MAX_INLINE_CHARS)
            )
        } else {
            "Execute sandboxed JavaScript orchestration".to_string()
        }
    });

    let mut details = Vec::with_capacity(2);
    if deep_research {
        if let Some(plan) = execution_input.and_then(deep_research_plan) {
            details.push(ProgramPreviewDetail {
                label: "plan",
                value: plan,
            });
        }
    }
    let phase = payload.and_then(|payload| {
        if deep_research {
            deep_research_program_phase(payload)
        } else {
            program_phase(payload)
        }
    });
    if let Some(phase) = phase {
        details.push(ProgramPreviewDetail {
            label: "phase",
            value: phase,
        });
    } else if deep_research {
        let local_only = execution_input
            .and_then(|value| value.get("evidence_scope"))
            .and_then(Value::as_str)
            == Some("local_only");
        details.push(ProgramPreviewDetail {
            label: "phase",
            value: if local_only {
                "workspace retrieval → typed semantic coverage → closed review".to_string()
            } else {
                "initial retrieval → typed-gap supplement if needed → closed review".to_string()
            },
        });
    } else if let Some(scope) = allowed_tool_scope(args) {
        details.push(ProgramPreviewDetail {
            label: "scope",
            value: scope,
        });
    }
    details.truncate(2);

    Some(ProgramPreview { intent, details })
}

pub(super) fn summarize_program_calls(calls: &[Value]) -> Option<ProgramCallDigest> {
    if calls.is_empty() {
        return None;
    }

    let scanned = calls.len().min(MAX_CALL_RECORDS_TO_SCAN);
    let omitted_records = calls.len().saturating_sub(scanned);
    let mut groups: Vec<(String, usize)> = Vec::with_capacity(MAX_TOOL_GROUPS);
    let mut ungrouped_calls = 0usize;
    let mut succeeded = 0usize;
    for call in calls.iter().take(MAX_CALL_RECORDS_TO_SCAN) {
        let name = call
            .get("tool_name")
            .and_then(Value::as_str)
            .map(|value| clean_inline(value, 40))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| "tool".to_string());
        if call.get("success").and_then(Value::as_bool) == Some(true) {
            succeeded += 1;
        }
        if let Some((_, count)) = groups.iter_mut().find(|(stored, _)| stored == &name) {
            *count += 1;
        } else if groups.len() < MAX_TOOL_GROUPS {
            groups.push((name, 1));
        } else {
            ungrouped_calls += 1;
        }
    }

    let mut route = groups
        .iter()
        .map(|(name, count)| {
            if *count > 1 {
                format!("{name} ×{count}")
            } else {
                name.clone()
            }
        })
        .collect::<Vec<_>>()
        .join(" → ");
    if ungrouped_calls > 0 {
        route.push_str(&format!(" → +{ungrouped_calls} calls"));
    }
    let bounded_result = format!("{succeeded}/{scanned} ok");
    let result = if omitted_records > 0 {
        format!("{bounded_result} · +{omitted_records} records")
    } else {
        bounded_result
    };
    Some(ProgramCallDigest {
        text: format!("called {route} · {result}"),
        has_failure: succeeded < scanned,
    })
}

fn is_deep_research_input(value: &Value) -> bool {
    let Some(object) = value.as_object() else {
        return false;
    };
    object.contains_key("evidence_scope")
        && (object.get("inquiry_host_managed").and_then(Value::as_bool) == Some(true)
            || value
                .pointer("/loop_contract/pattern")
                .and_then(Value::as_str)
                == Some("minimal-deep-research"))
}

fn deep_research_plan(value: &Value) -> Option<String> {
    let scope = value
        .get("evidence_scope")
        .and_then(Value::as_str)
        .map(|scope| match scope {
            "web_and_workspace" => "web + workspace".to_string(),
            "local_only" => "local only".to_string(),
            other => clean_inline(other, 32),
        });
    let searches = value
        .pointer("/loop_contract/hard_caps/max_searches")
        .and_then(Value::as_u64);
    let fetches = value
        .pointer("/loop_contract/hard_caps/max_fetches")
        .and_then(Value::as_u64);

    let mut parts = Vec::new();
    if let Some(scope) = scope.filter(|scope| !scope.is_empty()) {
        parts.push(scope);
    }
    let local_only = value.get("evidence_scope").and_then(Value::as_str) == Some("local_only");
    parts.push(if local_only {
        "one workspace retrieval".to_string()
    } else {
        "≤2 typed-coverage passes".to_string()
    });
    if !local_only {
        if let Some(searches) = searches {
            parts.push(format!("≤{searches} searches"));
        }
        if let Some(fetches) = fetches {
            parts.push(format!("≤{fetches} fetches"));
        }
    }
    Some(parts.join(" · "))
}

fn program_phase(payload: &Value) -> Option<String> {
    match payload.get("kind").and_then(Value::as_str) {
        Some("workflow") => {
            let completed = payload
                .get("step_outputs")
                .and_then(Value::as_object)
                .map_or(0, serde_json::Map::len);
            let failed = payload
                .get("step_failures")
                .and_then(Value::as_object)
                .map_or(0, serde_json::Map::len);
            Some(if completed == 0 && failed == 0 {
                "plan the initial evidence routes".to_string()
            } else if failed > 0 {
                format!("review {completed} completed / {failed} failed steps and choose recovery")
            } else {
                format!("review {completed} completed steps and choose the next route")
            })
        }
        Some("step") => {
            let step = payload
                .get("step_name")
                .and_then(Value::as_str)
                .map(|value| clean_inline(value, 64));
            Some(match step.as_deref() {
                Some("direct_web_research") => {
                    "collect and validate direct web evidence".to_string()
                }
                Some("runtime_preflight") => "verify remote research capability".to_string(),
                Some("runtime_research") => "run remote evidence collection".to_string(),
                Some(step) if step.starts_with("local_research") => {
                    "run a focused local research round".to_string()
                }
                Some(step) if step.starts_with("local_fallback") => {
                    "run the local evidence fallback".to_string()
                }
                Some(step) if !step.is_empty() => format!("execute workflow step {step}"),
                _ => "execute one workflow step".to_string(),
            })
        }
        _ => None,
    }
}

fn deep_research_program_phase(payload: &Value) -> Option<String> {
    match payload.get("kind").and_then(Value::as_str) {
        Some("workflow") => {
            let completed = payload
                .get("step_outputs")
                .and_then(Value::as_object)
                .map_or(0, serde_json::Map::len);
            let failed = payload
                .get("step_failures")
                .and_then(Value::as_object)
                .map_or(0, serde_json::Map::len);
            Some(if completed == 0 && failed == 0 {
                "run initial retrieval and typed semantic coverage".to_string()
            } else if failed > 0 {
                "settle failures without opening an untyped retrieval route".to_string()
            } else {
                "close typed gaps from the existing catalog, then seal evidence".to_string()
            })
        }
        Some("step") => Some(
            match payload.get("step_name").and_then(Value::as_str) {
                Some("retrieve_web") => "run the initial web retrieval pass",
                Some("retrieve_supplemental_web") => {
                    "fetch typed-gap candidates from the existing catalog"
                }
                Some("retrieve_local") => "collect bounded workspace evidence once",
                Some("generate_object") => "select IDs from the closed chunk catalog",
                _ => "execute one bounded stage of the fixed pipeline",
            }
            .to_string(),
        ),
        _ => None,
    }
}

fn allowed_tool_scope(args: &Value) -> Option<String> {
    let tools = args.get("allowed_tools")?.as_array()?;
    let mut names = tools
        .iter()
        .filter_map(Value::as_str)
        .map(|value| clean_inline(value, 40))
        .filter(|value| !value.is_empty())
        .take(MAX_TOOL_GROUPS + 1)
        .collect::<Vec<_>>();
    if names.is_empty() {
        return None;
    }
    let omitted = names.len().saturating_sub(MAX_TOOL_GROUPS);
    names.truncate(MAX_TOOL_GROUPS);
    let mut summary = names.join(" · ");
    if omitted > 0 || tools.len() > MAX_TOOL_GROUPS {
        summary.push_str(&format!(
            " · +{}",
            tools.len().saturating_sub(MAX_TOOL_GROUPS)
        ));
    }
    Some(summary)
}

fn first_inline_text(root: &Value, pointers: &[&str], max_chars: usize) -> Option<String> {
    pointers.iter().find_map(|pointer| {
        root.pointer(pointer)
            .and_then(Value::as_str)
            .map(|value| clean_inline(value, max_chars))
            .filter(|value| !value.is_empty())
    })
}

fn first_object_text(value: &Value, keys: &[&str], max_chars: usize) -> Option<String> {
    keys.iter().find_map(|key| {
        value
            .get(*key)
            .and_then(Value::as_str)
            .map(|value| clean_inline(value, max_chars))
            .filter(|value| !value.is_empty())
    })
}

fn clean_inline(value: &str, max_chars: usize) -> String {
    let without_ansi = strip_ansi(value);
    let collapsed = without_ansi
        .chars()
        .map(|ch| if ch.is_control() { ' ' } else { ch })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let mut chars = collapsed.chars();
    let mut bounded = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        bounded.push('…');
    }
    bounded
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn deep_research_preview_uses_structured_intent_instead_of_source_wrapper() {
        let preview = summarize_program_args(Some(&json!({
            "type": "script",
            "source": "async function run(ctx, inputs) { return {}; }",
            "inputs": {
                "kind": "workflow",
                "step_outputs": {},
                "step_failures": {},
                "input": {
                    "query": "Nimbus\u{1b}[31m 支持状态",
                    "evidence_scope": "web_and_workspace",
                    "inquiry_host_managed": true,
                    "loop_contract": {
                        "pattern": "minimal-deep-research",
                        "hard_caps": {
                            "max_searches": 4,
                            "max_fetches": 8
                        }
                    }
                }
            }
        })))
        .unwrap();

        assert!(preview.intent.contains("DeepResearch “Nimbus 支持状态”"));
        assert!(!preview.intent.contains("async function run"));
        assert_eq!(preview.details[0].label, "plan");
        assert_eq!(
            preview.details[0].value,
            "web + workspace · ≤2 typed-coverage passes · ≤4 searches · ≤8 fetches"
        );
        assert_eq!(
            preview.details[1].value,
            "run initial retrieval and typed semantic coverage"
        );
    }

    #[test]
    fn workflow_phase_changes_after_steps_complete() {
        let preview = summarize_program_args(Some(&json!({
            "type": "script",
            "inputs": {
                "kind": "workflow",
                "step_outputs": {"seed": {}, "verify": {}},
                "step_failures": {},
                "input": {"query": "status"}
            }
        })))
        .unwrap();

        assert_eq!(preview.details[0].label, "phase");
        assert_eq!(
            preview.details[0].value,
            "review 2 completed steps and choose the next route"
        );
    }

    #[test]
    fn workflow_step_preview_names_the_current_operation() {
        let preview = summarize_program_args(Some(&json!({
            "type": "script",
            "inputs": {
                "kind": "step",
                "step_name": "direct_web_research",
                "input": {"query": "status"}
            }
        })))
        .unwrap();

        assert_eq!(
            preview.details[0].value,
            "collect and validate direct web evidence"
        );
    }

    #[test]
    fn completed_calls_are_aggregated_into_one_bounded_digest() {
        let calls = json!([
            {"tool_name": "web_search", "success": true},
            {"tool_name": "web_fetch", "success": true},
            {"tool_name": "web_search", "success": false}
        ]);
        let digest = summarize_program_calls(calls.as_array().unwrap()).unwrap();

        assert_eq!(digest.text, "called web_search ×2 → web_fetch · 2/3 ok");
        assert!(digest.has_failure);
    }

    #[test]
    fn completed_call_digest_has_a_hard_scan_limit() {
        let calls = (0..300)
            .map(|index| {
                json!({
                    "tool_name": format!("tool-{index}"),
                    "success": true
                })
            })
            .collect::<Vec<_>>();
        let digest = summarize_program_calls(&calls).unwrap();

        assert!(digest.text.contains("+252 calls"), "{}", digest.text);
        assert!(
            digest.text.contains("256/256 ok · +44 records"),
            "{}",
            digest.text
        );
        assert!(!digest.has_failure);
    }

    #[test]
    fn path_script_fallback_never_reads_or_echoes_source() {
        let preview = summarize_program_args(Some(&json!({
            "type": "script",
            "path": "scripts/report.mjs",
            "source": "secret implementation"
        })))
        .unwrap();

        assert_eq!(preview.intent, "Run workspace script scripts/report.mjs");
        assert!(!preview.intent.contains("secret"));
    }
}
