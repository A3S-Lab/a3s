use a3s_code_core::dynamic_workflow_store_path;
use std::io::Read;
use std::path::{Component, Path};
#[cfg(test)]
use std::time::Duration;
use std::time::{SystemTime, UNIX_EPOCH};

const MAX_WORKFLOW_STORE_FILE_BYTES: u64 = 16 * 1024 * 1024;
const BOOTSTRAP_ACQUISITION_CHECKPOINT_STEP_ID: &str = "checkpoint_bootstrap_acquisition";
#[cfg(test)]
const INITIAL_RETRIEVAL_CHECKPOINT_STEP_ID: &str = "checkpoint_initial_retrieval";

fn safe_existing_workflow_store(workspace: &Path, store: &Path) -> bool {
    let Ok(root_metadata) = std::fs::symlink_metadata(workspace) else {
        return false;
    };
    if root_metadata.file_type().is_symlink() || !root_metadata.is_dir() {
        return false;
    }
    let Ok(relative) = store.strip_prefix(workspace) else {
        return false;
    };
    let mut current = workspace.to_path_buf();
    for component in relative.components() {
        let Component::Normal(component) = component else {
            return false;
        };
        current.push(component);
        let Ok(metadata) = std::fs::symlink_metadata(&current) else {
            return false;
        };
        if metadata.file_type().is_symlink() || !metadata.is_dir() {
            return false;
        }
    }
    true
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct DeepResearchWorkflowStoreRun {
    pub(crate) output: Option<String>,
    pub(crate) exit_code: i32,
    pub(crate) metadata: serde_json::Value,
}

pub(crate) fn ensure_deep_research_workflow_run_id(args: &mut serde_json::Value) -> Option<String> {
    let existing = args
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .filter(|run_id| safe_flow_run_id(run_id))
        .map(str::to_string);
    if existing.is_some() {
        return existing;
    }

    let run_id = generated_deep_research_workflow_run_id();
    args.as_object_mut()?.insert(
        "run_id".to_string(),
        serde_json::Value::String(run_id.clone()),
    );
    Some(run_id)
}

pub(crate) fn recover_deep_research_workflow_run_from_store(
    workspace: &Path,
    args: &serde_json::Value,
) -> Option<DeepResearchWorkflowStoreRun> {
    let run_id = args
        .get("run_id")
        .and_then(serde_json::Value::as_str)
        .filter(|run_id| safe_flow_run_id(run_id))?;
    let expected_query = args
        .pointer("/input/query")
        .and_then(serde_json::Value::as_str);
    let store = dynamic_workflow_store_path(workspace);
    if !safe_existing_workflow_store(workspace, &store) {
        return None;
    }
    recover_deep_research_workflow_run_from_path(
        &store.join(format!("{run_id}.jsonl")),
        run_id,
        expected_query,
    )
}

/// Recover the last complete initial evidence portfolio when the optional
/// supplemental pass is still running as the shared retrieval deadline fires.
/// The checkpoint is an ordinary durable Flow step, so recovery remains tied
/// to the exact run ID and query rather than becoming a query-result cache.
#[cfg(test)]
pub(crate) fn recover_deep_research_initial_retrieval_from_store(
    workspace: &Path,
    args: &serde_json::Value,
) -> Option<DeepResearchWorkflowStoreRun> {
    recover_deep_research_checkpoint_from_store(
        workspace,
        args,
        INITIAL_RETRIEVAL_CHECKPOINT_STEP_ID,
        "recovered_initial_retrieval",
    )
}

/// Recover raw search/fetch output independently of planner completion. The
/// exact root run and query still identify the Flow journal, so this is durable
/// process recovery rather than a cross-run query cache.
pub(crate) fn recover_deep_research_bootstrap_acquisition_from_store(
    workspace: &Path,
    args: &serde_json::Value,
) -> Option<DeepResearchWorkflowStoreRun> {
    recover_deep_research_checkpoint_from_store(
        workspace,
        args,
        BOOTSTRAP_ACQUISITION_CHECKPOINT_STEP_ID,
        "recovered_bootstrap_acquisition",
    )
}

fn recover_deep_research_checkpoint_from_store(
    workspace: &Path,
    args: &serde_json::Value,
    step_id: &str,
    metadata_flag: &str,
) -> Option<DeepResearchWorkflowStoreRun> {
    let mut recovered = recover_deep_research_workflow_run_from_store(workspace, args)?;
    let checkpoint = recovered
        .metadata
        .pointer(&format!("/dynamic_workflow/snapshot/steps/{step_id}"))?;
    if checkpoint.get("status").and_then(serde_json::Value::as_str) != Some("completed") {
        return None;
    }
    let output = checkpoint
        .get("output")
        .filter(|output| output.is_object())?
        .clone();
    recovered.output = Some(workflow_output_text(&output));
    recovered.exit_code = 0;
    if let Some(dynamic_workflow) = recovered
        .metadata
        .get_mut("dynamic_workflow")
        .and_then(serde_json::Value::as_object_mut)
    {
        dynamic_workflow.insert(metadata_flag.to_string(), serde_json::Value::Bool(true));
    }
    Some(recovered)
}

fn recover_deep_research_workflow_run_from_path(
    path: &Path,
    run_id: &str,
    expected_query: Option<&str>,
) -> Option<DeepResearchWorkflowStoreRun> {
    let metadata = std::fs::symlink_metadata(path).ok()?;
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_WORKFLOW_STORE_FILE_BYTES
    {
        return None;
    }
    let file = std::fs::File::open(path).ok()?;
    let mut text = String::new();
    file.take(MAX_WORKFLOW_STORE_FILE_BYTES + 1)
        .read_to_string(&mut text)
        .ok()?;
    if text.len() as u64 > MAX_WORKFLOW_STORE_FILE_BYTES {
        return None;
    }
    let mut projection = FlowJsonlProjection::new(run_id);
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        if let Ok(envelope) = serde_json::from_str::<serde_json::Value>(line) {
            projection.apply(&envelope);
        }
    }
    if projection.last_sequence == 0 {
        return None;
    }
    if let Some(expected) = expected_query {
        if projection.query.as_deref() != Some(expected) {
            return None;
        }
    }
    Some(projection.finish())
}

fn generated_deep_research_workflow_run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let nonce = rand::random::<u64>();
    format!("deepresearch-{}-{nanos}-{nonce:016x}", std::process::id())
}

fn safe_flow_run_id(run_id: &str) -> bool {
    !run_id.is_empty()
        && run_id
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
}

#[derive(Debug)]
struct FlowJsonlProjection {
    run_id: String,
    query: Option<String>,
    spec: serde_json::Value,
    input: serde_json::Value,
    status: &'static str,
    snapshot_status: &'static str,
    last_sequence: u64,
    source_hash: Option<String>,
    steps: serde_json::Map<String, serde_json::Value>,
    output: Option<serde_json::Value>,
    error: Option<String>,
}

impl FlowJsonlProjection {
    fn new(run_id: &str) -> Self {
        Self {
            run_id: run_id.to_string(),
            query: None,
            spec: serde_json::Value::Null,
            input: serde_json::Value::Null,
            status: "Running",
            snapshot_status: "running",
            last_sequence: 0,
            source_hash: None,
            steps: serde_json::Map::new(),
            output: None,
            error: None,
        }
    }

    fn apply(&mut self, envelope: &serde_json::Value) {
        if envelope.get("run_id").and_then(serde_json::Value::as_str) != Some(self.run_id.as_str())
        {
            return;
        }
        self.last_sequence = envelope
            .get("sequence")
            .and_then(serde_json::Value::as_u64)
            .unwrap_or(self.last_sequence);
        let Some(event) = envelope.get("event").and_then(serde_json::Value::as_object) else {
            return;
        };
        match event.get("type").and_then(serde_json::Value::as_str) {
            Some("run_created") => self.apply_run_created(event),
            Some("run_started") => {
                self.status = "Running";
                self.snapshot_status = "running";
            }
            Some("run_completed") => {
                self.status = "Completed";
                self.snapshot_status = "completed";
                self.output = event.get("output").cloned();
                self.error = None;
            }
            Some("run_failed") => {
                self.status = "Failed";
                self.snapshot_status = "failed";
                self.error = event
                    .get("error")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
            Some("run_cancelled") => {
                self.status = "Cancelled";
                self.snapshot_status = "cancelled";
                self.error = event
                    .get("reason")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
            }
            Some("step_created") => self.apply_step_created(event),
            Some("step_started") => self.update_step(event, "running"),
            Some("step_completed") => self.apply_step_completed(event),
            Some("step_failed") => self.apply_step_failed(event),
            _ => {}
        }
    }

    fn apply_run_created(&mut self, event: &serde_json::Map<String, serde_json::Value>) {
        self.spec = event
            .get("spec")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        self.input = event
            .get("input")
            .cloned()
            .unwrap_or(serde_json::Value::Null);
        self.query = self
            .input
            .get("query")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        self.source_hash = self
            .spec
            .get("version")
            .and_then(serde_json::Value::as_str)
            .map(str::to_string);
        self.status = "Pending";
        self.snapshot_status = "pending";
    }

    fn apply_step_created(&mut self, event: &serde_json::Map<String, serde_json::Value>) {
        let Some(step_id) = event.get("step_id").and_then(serde_json::Value::as_str) else {
            return;
        };
        let step_name = event
            .get("step_name")
            .and_then(serde_json::Value::as_str)
            .unwrap_or(step_id);
        self.steps.insert(
            step_id.to_string(),
            serde_json::json!({
                "step_id": step_id,
                "step_name": step_name,
                "status": "pending",
                "input": event.get("input").cloned().unwrap_or(serde_json::Value::Null),
                "retry": event.get("retry").cloned().unwrap_or(serde_json::Value::Null),
                "output": serde_json::Value::Null,
                "error": serde_json::Value::Null,
                "attempt": 0,
                "retry_after": serde_json::Value::Null,
            }),
        );
    }

    fn update_step(&mut self, event: &serde_json::Map<String, serde_json::Value>, status: &str) {
        let Some(step_id) = event.get("step_id").and_then(serde_json::Value::as_str) else {
            return;
        };
        let Some(step) = self
            .steps
            .get_mut(step_id)
            .and_then(serde_json::Value::as_object_mut)
        else {
            return;
        };
        step.insert(
            "status".to_string(),
            serde_json::Value::String(status.to_string()),
        );
        if let Some(attempt) = event.get("attempt") {
            step.insert("attempt".to_string(), attempt.clone());
        }
    }

    fn apply_step_completed(&mut self, event: &serde_json::Map<String, serde_json::Value>) {
        self.update_step(event, "completed");
        let Some(step_id) = event.get("step_id").and_then(serde_json::Value::as_str) else {
            return;
        };
        let Some(step) = self
            .steps
            .get_mut(step_id)
            .and_then(serde_json::Value::as_object_mut)
        else {
            return;
        };
        step.insert(
            "output".to_string(),
            event
                .get("output")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        step.insert("error".to_string(), serde_json::Value::Null);
    }

    fn apply_step_failed(&mut self, event: &serde_json::Map<String, serde_json::Value>) {
        self.update_step(event, "failed");
        let Some(step_id) = event.get("step_id").and_then(serde_json::Value::as_str) else {
            return;
        };
        let Some(step) = self
            .steps
            .get_mut(step_id)
            .and_then(serde_json::Value::as_object_mut)
        else {
            return;
        };
        step.insert(
            "error".to_string(),
            event
                .get("error")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
    }

    fn finish(self) -> DeepResearchWorkflowStoreRun {
        let output_text = self.output.as_ref().map(workflow_output_text);
        let exit_code = if self.status == "Completed" { 0 } else { 1 };
        let metadata = serde_json::json!({
            "dynamic_workflow": {
                "run_id": self.run_id,
                "status": self.status,
                "last_sequence": self.last_sequence,
                "source_hash": self.source_hash,
                "snapshot": {
                    "run_id": self.run_id,
                    "spec": self.spec,
                    "input": self.input,
                    "status": self.snapshot_status,
                    "steps": self.steps,
                    "waits": {},
                    "hooks": {},
                    "output": self.output,
                    "error": self.error,
                    "last_sequence": self.last_sequence,
                }
            }
        });
        DeepResearchWorkflowStoreRun {
            output: output_text,
            exit_code,
            metadata,
        }
    }
}

fn workflow_output_text(output: &serde_json::Value) -> String {
    serde_json::to_string_pretty(output).unwrap_or_else(|_| output.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tui::accepted_evidence_ledger;

    #[test]
    fn workflow_store_stays_under_project_a3s_root() {
        let workspace = Path::new("/workspace");

        assert_eq!(
            dynamic_workflow_store_path(workspace),
            workspace.join(".a3s/workflow")
        );
    }

    #[test]
    fn generated_workflow_run_ids_are_safe_and_unique() {
        let ids = (0..256)
            .map(|_| generated_deep_research_workflow_run_id())
            .collect::<std::collections::HashSet<_>>();
        assert_eq!(ids.len(), 256);
        assert!(ids
            .iter()
            .all(|run_id| run_id.starts_with("deepresearch-") && safe_flow_run_id(run_id)));
    }

    #[test]
    fn workflow_store_recovers_timeout_metadata_for_completed_step_output_json() {
        let workspace = std::env::temp_dir().join(format!(
            "a3s-deepresearch-workflow-store-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = dynamic_workflow_store_path(&workspace);
        std::fs::create_dir_all(&store).unwrap();
        let run_id = "deepresearch-test-run";
        let evidence = serde_json::json!({
            "summary": "Durable Flow events preserved source-backed evidence after the host timeout fired.",
            "sources": [{
                "title": "Durable Source",
                "url": "https://example.com/durable-flow",
                "publication_date": "2026-07-09",
                "evidence": "A completed parallel_task child result was present in the Flow event log.",
                "publisher": "deterministic test fixture"
            }],
            "key_evidence": ["The completed step output contains valid evidence JSON."],
            "contradictions": [],
            "confidence": "high for deterministic durable event recovery",
            "gaps": []
        });
        let completed_output = serde_json::json!({
            "query": "durable timeout evidence",
            "mode": "local_parallel_task",
            "collection_status": "completed",
            "research": {
                "status": "success",
                "results": [{
                    "success": true,
                    "structured": evidence.clone()
                }]
            }
        });
        let lines = [
            serde_json::json!({
                "run_id": run_id,
                "sequence": 1,
                "event": {
                    "type": "run_created",
                    "spec": { "version": "source-hash" },
                    "input": { "query": "durable timeout evidence" }
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 2,
                "event": { "type": "run_started" }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 3,
                "event": {
                    "type": "step_created",
                    "step_id": "local_research",
                    "step_name": "parallel_task",
                    "input": { "allow_partial_failure": true, "tasks": [] }
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 4,
                "event": {
                    "type": "step_started",
                    "step_id": "local_research",
                    "attempt": 1
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 5,
                "event": {
                    "type": "step_completed",
                    "step_id": "local_research",
                    "output": {
                        "tool": "parallel_task",
                        "exit_code": 0,
                        "metadata": {
                            "success_count": 1,
                            "failed_count": 0,
                            "results": [{
                                "success": true,
                                "source_anchors": [{
                                    "tool": "web_search",
                                    "url_or_path": "https://example.com/durable-flow"
                                }],
                                "output": format!(
                                    "Task completed: task-1\nAgent: deep-research\nOutput:\n{}",
                                    evidence
                                )
                            }]
                        }
                    }
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 6,
                "event": {
                    "type": "run_completed",
                    "output": completed_output
                }
            }),
        ]
        .into_iter()
        .map(|line| serde_json::to_string(&line).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        std::fs::write(store.join(format!("{run_id}.jsonl")), format!("{lines}\n")).unwrap();

        let args = serde_json::json!({
            "run_id": run_id,
            "input": { "query": "durable timeout evidence" }
        });
        let recovered = recover_deep_research_workflow_run_from_store(&workspace, &args)
            .expect("completed durable Flow event log should be recoverable");
        assert_eq!(recovered.exit_code, 0);
        assert_eq!(
            recovered.metadata["dynamic_workflow"]["status"],
            "Completed"
        );
        assert_eq!(
            recovered.metadata["dynamic_workflow"]["snapshot"]["steps"]["local_research"]["status"],
            "completed"
        );
        let workflow_output = recovered
            .output
            .as_deref()
            .expect("run_completed should preserve its structured output");
        let parsed_output = serde_json::from_str::<serde_json::Value>(workflow_output).unwrap();
        assert_eq!(parsed_output["collection_status"], "completed");

        let ledger = accepted_evidence_ledger(workflow_output, Some(&recovered.metadata));
        assert_eq!(ledger.len(), 1);
        assert_eq!(
            ledger[0].sources[0].anchor,
            "https://example.com/durable-flow"
        );
        assert!(
            !workspace
                .join(".a3s/research/durable-timeout-evidence/report.md")
                .exists(),
            "retrieval recovery must not bypass closed-evidence review and report generation"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn running_optional_supplement_recovers_the_completed_initial_portfolio() {
        let workspace = std::env::temp_dir().join(format!(
            "a3s-deepresearch-initial-checkpoint-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = dynamic_workflow_store_path(&workspace);
        std::fs::create_dir_all(&store).unwrap();
        let run_id = "deepresearch-running-supplement";
        let initial_output = serde_json::json!({
            "query": "recover initial evidence",
            "mode": "inquiry_collection",
            "plan": {"tracks": []},
            "research": {
                "status": "success",
                "results": [{
                    "success": true,
                    "structured": {
                        "summary": "The initial pass retained traceable material evidence.",
                        "sources": [{
                            "title": "Initial Source",
                            "url": "https://example.com/initial",
                            "evidence": "The initial source directly supports the finding."
                        }],
                        "key_evidence": ["The initial source directly supports the finding."],
                        "contradictions": [],
                        "gaps": []
                    }
                }]
            },
            "execution": {
                "mode": "collect_only",
                "terminal_authority": "host_inquiry_reducer"
            }
        });
        let lines = [
            serde_json::json!({
                "run_id": run_id,
                "sequence": 1,
                "event": {
                    "type": "run_created",
                    "spec": {"version": "source-hash"},
                    "input": {"query": "recover initial evidence"}
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 2,
                "event": {"type": "run_started"}
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 3,
                "event": {
                    "type": "step_created",
                    "step_id": INITIAL_RETRIEVAL_CHECKPOINT_STEP_ID,
                    "step_name": INITIAL_RETRIEVAL_CHECKPOINT_STEP_ID,
                    "input": initial_output.clone()
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 4,
                "event": {
                    "type": "step_started",
                    "step_id": INITIAL_RETRIEVAL_CHECKPOINT_STEP_ID,
                    "attempt": 1
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 5,
                "event": {
                    "type": "step_completed",
                    "step_id": INITIAL_RETRIEVAL_CHECKPOINT_STEP_ID,
                    "output": initial_output
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 6,
                "event": {
                    "type": "step_created",
                    "step_id": "select_supplemental_evidence_chunks",
                    "step_name": "generate_object",
                    "input": {}
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 7,
                "event": {
                    "type": "step_started",
                    "step_id": "select_supplemental_evidence_chunks",
                    "attempt": 1
                }
            }),
        ]
        .into_iter()
        .map(|line| serde_json::to_string(&line).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        std::fs::write(store.join(format!("{run_id}.jsonl")), format!("{lines}\n")).unwrap();

        let recovered = recover_deep_research_initial_retrieval_from_store(
            &workspace,
            &serde_json::json!({
                "run_id": run_id,
                "input": {"query": "recover initial evidence"}
            }),
        )
        .expect("completed initial checkpoint");

        assert_eq!(recovered.exit_code, 0);
        assert_eq!(recovered.metadata["dynamic_workflow"]["status"], "Running");
        assert_eq!(
            recovered.metadata["dynamic_workflow"]["recovered_initial_retrieval"],
            true
        );
        let output = recovered.output.as_deref().expect("checkpoint output");
        assert_eq!(
            serde_json::from_str::<serde_json::Value>(output).unwrap()["query"],
            "recover initial evidence"
        );
        assert_eq!(accepted_evidence_ledger(output, None).len(), 1);
        assert_eq!(
            accepted_evidence_ledger(output, Some(&recovered.metadata)).len(),
            1
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn running_planner_recovers_the_completed_bootstrap_acquisition() {
        let workspace = std::env::temp_dir().join(format!(
            "a3s-deepresearch-bootstrap-checkpoint-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = dynamic_workflow_store_path(&workspace);
        std::fs::create_dir_all(&store).unwrap();
        let run_id = "deepresearch-running-bootstrap";
        let bootstrap_output = serde_json::json!({
            "query": "recover bootstrap evidence",
            "mode": "bootstrap_acquisition",
            "acquisition": {
                "status": "success",
                "packet": {
                    "version": 1,
                    "focuses": [],
                    "sources": [{
                        "source_id": "bootstrap-web-source-1",
                        "title": "Bootstrap Source",
                        "url_or_path": "https://example.com/bootstrap",
                        "reliability": "Fetched before semantic planning settled.",
                        "chunks": [{
                            "chunk_id": "bootstrap-web-source-1:chunk:1",
                            "text": "The bootstrap source contains durable raw evidence."
                        }]
                    }]
                },
                "errors": [],
                "metadata": {"source_selection_mode": "provider_round_robin"}
            },
            "execution": {
                "mode": "acquire_only",
                "terminal_authority": "host_inquiry_reducer"
            }
        });
        let lines = [
            serde_json::json!({
                "run_id": run_id,
                "sequence": 1,
                "event": {
                    "type": "run_created",
                    "spec": {"version": "source-hash"},
                    "input": {"query": "recover bootstrap evidence"}
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 2,
                "event": {"type": "run_started"}
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 3,
                "event": {
                    "type": "step_created",
                    "step_id": BOOTSTRAP_ACQUISITION_CHECKPOINT_STEP_ID,
                    "step_name": BOOTSTRAP_ACQUISITION_CHECKPOINT_STEP_ID,
                    "input": bootstrap_output.clone()
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 4,
                "event": {
                    "type": "step_started",
                    "step_id": BOOTSTRAP_ACQUISITION_CHECKPOINT_STEP_ID,
                    "attempt": 1
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 5,
                "event": {
                    "type": "step_completed",
                    "step_id": BOOTSTRAP_ACQUISITION_CHECKPOINT_STEP_ID,
                    "output": bootstrap_output
                }
            }),
        ]
        .into_iter()
        .map(|line| serde_json::to_string(&line).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        std::fs::write(store.join(format!("{run_id}.jsonl")), format!("{lines}\n")).unwrap();

        let recovered = recover_deep_research_bootstrap_acquisition_from_store(
            &workspace,
            &serde_json::json!({
                "run_id": run_id,
                "input": {"query": "recover bootstrap evidence"}
            }),
        )
        .expect("completed bootstrap checkpoint");

        assert_eq!(recovered.exit_code, 0);
        assert_eq!(recovered.metadata["dynamic_workflow"]["status"], "Running");
        assert_eq!(
            recovered.metadata["dynamic_workflow"]["recovered_bootstrap_acquisition"],
            true
        );
        let output = recovered.output.as_deref().expect("checkpoint output");
        let output = serde_json::from_str::<serde_json::Value>(output).unwrap();
        assert_eq!(output["query"], "recover bootstrap evidence");
        assert_eq!(output["mode"], "bootstrap_acquisition");
        assert_eq!(
            output["acquisition"]["packet"]["sources"][0]["url_or_path"],
            "https://example.com/bootstrap"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }

    #[test]
    fn workflow_store_exact_recovery_is_not_an_mtime_based_query_cache() {
        let workspace = std::env::temp_dir().join(format!(
            "a3s-deepresearch-workflow-store-stale-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let store = dynamic_workflow_store_path(&workspace);
        std::fs::create_dir_all(&store).unwrap();
        let run_id = "deepresearch-stale-run";
        let lines = [
            serde_json::json!({
                "run_id": run_id,
                "sequence": 1,
                "event": {
                    "type": "run_created",
                    "spec": { "version": "old-source-hash" },
                    "input": { "query": "stale evidence query" }
                }
            }),
            serde_json::json!({
                "run_id": run_id,
                "sequence": 2,
                "event": {
                    "type": "run_completed",
                    "output": { "summary": "old result" }
                }
            }),
        ]
        .into_iter()
        .map(|line| serde_json::to_string(&line).unwrap())
        .collect::<Vec<_>>()
        .join("\n");
        let path = store.join(format!("{run_id}.jsonl"));
        std::fs::write(&path, format!("{lines}\n")).unwrap();
        let stale_time = SystemTime::now()
            .checked_sub(Duration::from_secs(2 * 24 * 60 * 60))
            .unwrap();
        std::fs::File::options()
            .write(true)
            .open(&path)
            .unwrap()
            .set_times(std::fs::FileTimes::new().set_modified(stale_time))
            .unwrap();

        assert!(
            recover_deep_research_workflow_run_from_store(
                &workspace,
                &serde_json::json!({
                    "run_id": run_id,
                    "input": { "query": "stale evidence query" }
                })
            )
            .is_some(),
            "an explicitly identified in-flight run remains recoverable regardless of mtime"
        );

        let _ = std::fs::remove_dir_all(&workspace);
    }
}
