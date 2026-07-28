//! Semantic transcript documents for workflows and delegated tasks.

use super::*;

pub(super) fn workflow_doc_for_tool(
    name: &str,
    args: Option<&serde_json::Value>,
) -> Option<(String, String)> {
    match name {
        "dynamic_workflow" => workflow_intent_doc(args, "Dynamic workflow"),
        "program" => workflow_intent_doc(args, "Program"),
        "parallel_task" => {
            let tasks = args
                .and_then(|a| a.get("tasks"))
                .and_then(|t| t.as_array())?
                .iter()
                .collect::<Vec<_>>();
            workflow_doc_for_tasks(&tasks, true)
        }
        "task" => {
            let args = args?;
            if let Some(tasks) = args.get("tasks").and_then(|t| t.as_array()) {
                let tasks = tasks.iter().collect::<Vec<_>>();
                workflow_doc_for_tasks(&tasks, tasks.len() > 1)
            } else {
                workflow_doc_for_tasks(&[args], false)
            }
        }
        _ => None,
    }
}

pub(super) fn workflow_intent_doc(
    args: Option<&serde_json::Value>,
    title: &str,
) -> Option<(String, String)> {
    let preview = program_preview::summarize_program_args(args)?;
    let mut doc = format!("# {title} intent\n\nIntent: {}\n", preview.intent);
    for detail in preview.details {
        doc.push_str(&format!("{}: {}\n", detail.label, detail.value));
    }
    Some((doc, format!("{} intent captured", title.to_lowercase())))
}

pub(super) fn workflow_doc_for_tasks(
    tasks: &[&serde_json::Value],
    parallel: bool,
) -> Option<(String, String)> {
    if tasks.is_empty() {
        return None;
    }

    let mut doc = if parallel {
        format!(
            "# Parallel delegation\n\nFanned out {} parallel subagent task(s):\n\n",
            tasks.len()
        )
    } else {
        "# Delegation\n\nDelegated subagent task(s):\n\n".to_string()
    };

    for (i, task) in tasks.iter().enumerate() {
        let desc = task
            .get("description")
            .or_else(|| task.get("prompt"))
            .or_else(|| task.get("task"))
            .and_then(|v| v.as_str())
            .unwrap_or("(task)");
        let agent = task
            .get("agent")
            .and_then(|v| v.as_str())
            .unwrap_or("agent");
        let prompt = task
            .get("prompt")
            .or_else(|| task.get("task"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        doc.push_str(&format!(
            "## {}. {desc}\n\nAgent: `{agent}`\n\n{prompt}\n\n",
            i + 1
        ));
    }

    let label = if parallel {
        format!("delegation · {} parallel tasks captured", tasks.len())
    } else {
        format!(
            "delegation · {} delegated task{} captured",
            tasks.len(),
            if tasks.len() == 1 { "" } else { "s" }
        )
    };
    Some((doc, label))
}
