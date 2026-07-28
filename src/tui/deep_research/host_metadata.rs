//! Durable workflow output selection and bounded TUI presentation.

#[cfg(test)]
use super::*;

/// Return the final workflow projection committed by the event-sourced
/// runtime. The display output is only a transport projection and may be
/// truncated or replaced by diagnostics.
pub(super) fn deep_research_canonical_workflow_output(
    workflow_output: &str,
    workflow_metadata: Option<&serde_json::Value>,
) -> String {
    let Some(dynamic_workflow) = workflow_metadata
        .and_then(|metadata| metadata.get("dynamic_workflow"))
        .and_then(serde_json::Value::as_object)
    else {
        return workflow_output.to_string();
    };
    let completed = dynamic_workflow
        .get("status")
        .and_then(serde_json::Value::as_str)
        .is_some_and(|status| status.eq_ignore_ascii_case("completed"));
    if !completed {
        return workflow_output.to_string();
    }
    let Some(output) = dynamic_workflow
        .get("snapshot")
        .and_then(|snapshot| snapshot.get("output"))
        .filter(|output| !output.is_null())
    else {
        return workflow_output.to_string();
    };

    serde_json::to_string(output).unwrap_or_else(|_| workflow_output.to_string())
}

#[cfg(test)]
pub(super) fn deep_research_tool_card_output(workflow_output: &str) -> String {
    workflow_evidence_summary(workflow_output).unwrap_or_else(|| {
        "Evidence collection did not return a typed summary; raw transport output is available only in terminal diagnostics."
            .to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::deep_research_tool_card_output;

    #[test]
    fn untyped_tool_card_output_is_not_classified_by_its_words() {
        let runtime_words =
            deep_research_tool_card_output("DynamicWorkflowRuntime output: transport details");
        let ordinary_words = deep_research_tool_card_output("ordinary untyped provider response");

        assert_eq!(runtime_words, ordinary_words);
        assert!(runtime_words.contains("did not return a typed summary"));
    }
}
