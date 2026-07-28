//! Semantic projection of the built-in batch tool's structured metadata.
//!
//! Keep parsing separate from terminal layout so compact history and the full
//! transcript can share one authoritative outcome instead of inferring batch
//! success from a top-level exit code.

use serde_json::Value;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BatchOutcome {
    Complete,
    Partial,
    Failed,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BatchItem {
    pub(super) index: usize,
    pub(super) tool: String,
    pub(super) args: Option<Value>,
    pub(super) success: bool,
    pub(super) exit_code: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BatchSummary {
    pub(super) outcome: BatchOutcome,
    pub(super) execution_mode: String,
    pub(super) applied_concurrency: usize,
    pub(super) total_count: usize,
    pub(super) success_count: usize,
    pub(super) failure_count: usize,
    pub(super) items: Vec<BatchItem>,
}

impl BatchSummary {
    pub(super) fn from_metadata(
        metadata: Option<&Value>,
        args: Option<&Value>,
        top_level_ok: bool,
    ) -> Option<Self> {
        let metadata = metadata?;
        let metadata = metadata.get("batch").unwrap_or(metadata);
        let results = metadata.get("results")?.as_array()?;
        if results.is_empty() {
            return None;
        }

        let invocations = args
            .and_then(|args| args.get("invocations"))
            .and_then(Value::as_array);
        let mut items = results
            .iter()
            .enumerate()
            .map(|(fallback_index, result)| {
                let index = result
                    .get("index")
                    .and_then(Value::as_u64)
                    .and_then(|index| usize::try_from(index).ok())
                    .unwrap_or(fallback_index);
                let invocation = invocations.and_then(|items| items.get(index));
                let tool = result
                    .get("tool")
                    .and_then(Value::as_str)
                    .or_else(|| invocation?.get("tool")?.as_str())
                    .unwrap_or("tool")
                    .to_string();
                let exit_code = result
                    .get("exit_code")
                    .and_then(Value::as_i64)
                    .and_then(|code| i32::try_from(code).ok())
                    .unwrap_or_else(|| {
                        if result.get("success").and_then(Value::as_bool) == Some(false) {
                            1
                        } else {
                            0
                        }
                    });
                let success = result
                    .get("success")
                    .and_then(Value::as_bool)
                    .unwrap_or(exit_code == 0);
                BatchItem {
                    index,
                    tool,
                    args: invocation
                        .and_then(|invocation| invocation.get("args"))
                        .cloned(),
                    success,
                    exit_code,
                }
            })
            .collect::<Vec<_>>();
        items.sort_by_key(|item| item.index);

        let derived_success = items.iter().filter(|item| item.success).count();
        let derived_failure = items.len().saturating_sub(derived_success);
        let success_count = usize_field(metadata, "success_count").unwrap_or(derived_success);
        let failure_count = usize_field(metadata, "failure_count").unwrap_or(derived_failure);
        let total_count = usize_field(metadata, "total_count")
            .unwrap_or_else(|| success_count.saturating_add(failure_count).max(items.len()));
        let outcome = if failure_count == 0 && top_level_ok {
            BatchOutcome::Complete
        } else if success_count > 0 && failure_count > 0 {
            BatchOutcome::Partial
        } else {
            BatchOutcome::Failed
        };

        Some(Self {
            outcome,
            execution_mode: metadata
                .get("execution_mode")
                .and_then(Value::as_str)
                .unwrap_or("serial")
                .to_string(),
            applied_concurrency: usize_field(metadata, "applied_concurrency").unwrap_or(1),
            total_count,
            success_count,
            failure_count,
            items,
        })
    }
}

fn usize_field(value: &Value, key: &str) -> Option<usize> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| usize::try_from(value).ok())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn partial_batch_uses_item_metadata_and_original_arguments() {
        let args = json!({
            "invocations": [
                {"tool": "read", "args": {"file_path": "README.md"}},
                {"tool": "bash", "args": {"command": "cargo test"}}
            ]
        });
        let metadata = json!({
            "status": "partial_failure",
            "execution_mode": "parallel",
            "applied_concurrency": 2,
            "total_count": 2,
            "success_count": 1,
            "failure_count": 1,
            "results": [
                {"index": 0, "tool": "read", "success": true, "exit_code": 0},
                {"index": 1, "tool": "bash", "success": false, "exit_code": 101}
            ]
        });

        let summary = BatchSummary::from_metadata(Some(&metadata), Some(&args), true).unwrap();
        assert_eq!(summary.outcome, BatchOutcome::Partial);
        assert_eq!(summary.execution_mode, "parallel");
        assert_eq!(summary.applied_concurrency, 2);
        assert_eq!(summary.items[1].exit_code, 101);
        assert_eq!(
            summary.items[1].args.as_ref().unwrap()["command"],
            "cargo test"
        );
    }

    #[test]
    fn missing_counts_are_derived_from_results() {
        let metadata = json!({
            "results": [
                {"tool": "read", "success": true},
                {"tool": "grep", "success": true}
            ]
        });

        let summary = BatchSummary::from_metadata(Some(&metadata), None, true).unwrap();
        assert_eq!(summary.outcome, BatchOutcome::Complete);
        assert_eq!(summary.total_count, 2);
        assert_eq!(summary.success_count, 2);
        assert_eq!(summary.failure_count, 0);
    }
}
