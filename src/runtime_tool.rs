//! The `runtime` tool — A3S Runtime offload for the a3s-code TUI.
//!
//! Registered into the session **only after the user logs in to OS (OS)**
//! (see `Tui::sync_runtime_tool`), so the model never sees it while signed out.
//!
//! When the model calls it, it fans the given subtasks out to OS's
//! Function-as-a-Service batch API (`POST /api/v1/functions/<worker>/batch`) for
//! **parallel** execution on the OS pod substrate, polls the batch to completion,
//! collects each invocation's output, and returns the aggregated results — so a
//! decomposed job (e.g. deep-research sub-questions) runs concurrently on remote
//! compute instead of serially in-process. See `docs/function-compute.md` in the
//! OS repo for the wire contract.
//!
//! Mechanics (each live-validated against the deployed OS):
//! - `worker` accepts a tool-kind agent asset **UUID**, or an asset **name**
//!   which is resolved via the assets API (the OS itself only takes UUIDs —
//!   a raw name fails server-side with `RUNNER_ERROR: invalid input syntax
//!   for type uuid`). Unknown names fail fast listing the available workers.
//! - Polling streams live progress into the TUI (`ToolStreamEvent`), backs off
//!   exponentially (1.5s → 6s cap), and tolerates transient network errors.
//! - On completion, results are collected **concurrently**; on budget expiry
//!   the finished subset is still returned (`partial: true` + `batchId`).

use a3s_code_core::tools::{Tool, ToolContext, ToolOutput, ToolStreamEvent};
use a3s_code_core::AgentEvent;
use anyhow::Result;
use async_trait::async_trait;
use serde_json::{json, Value};
use std::time::Duration;

use crate::a3s_os::{os_origin, StoredOsSession};

const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
/// Overall wait cap for a batch to finish (ceiling; callers can lower via `timeout_ms`).
const DEFAULT_BATCH_TIMEOUT_MS: u64 = 600_000;
/// Poll backoff: start fast (short batches return quickly), then ease off so a
/// 10-minute batch costs ~110 polls instead of 400.
const POLL_START: Duration = Duration::from_millis(1500);
const POLL_CAP: Duration = Duration::from_millis(6000);
/// Consecutive poll failures tolerated before giving up — one flaky HTTP tick
/// must not abandon an entire running batch.
const MAX_POLL_FAILURES: u32 = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct BatchProgress {
    done: u64,
    running: u64,
    queued: u64,
    pending: u64,
}

pub(crate) struct RuntimeTool {
    /// OS origin (`scheme://host[:port]`), derived from the login session address.
    origin: String,
    /// OS bearer token (the OAuth access token captured at login/refresh).
    token: String,
    /// Poll pacing — fields (not consts) so tests can run with tiny intervals.
    poll_start: Duration,
    poll_cap: Duration,
}

impl RuntimeTool {
    /// Build from the active OS session. Re-created on every login/refresh so the
    /// captured token stays current (the TUI rebuilds the session on auth change).
    pub(crate) fn new(session: &StoredOsSession) -> Self {
        Self {
            origin: os_origin(&session.address),
            token: session.access_token.clone(),
            poll_start: POLL_START,
            poll_cap: POLL_CAP,
        }
    }

    fn client(&self) -> Result<reqwest::Client> {
        let mut builder = reqwest::Client::builder().timeout(HTTP_TIMEOUT);
        if is_loopback_origin(&self.origin) {
            builder = builder.no_proxy();
        }
        Ok(builder.build()?)
    }

    /// Unwrap the shared OS response envelope `{code,status,message,data,...}` and
    /// return `data` (the real payload). Errors carry the wire `message` so the
    /// model sees why a call failed.
    fn unwrap_envelope(body: &str, status: u16) -> Result<Value> {
        let v: Value = serde_json::from_str(body).map_err(|e| {
            anyhow::anyhow!(
                "Non-JSON response (HTTP {status}): {e}: {}",
                truncate(body, 200)
            )
        })?;
        let code = v
            .get("code")
            .and_then(Value::as_u64)
            .unwrap_or(status as u64);
        if code >= 400 || status >= 400 {
            let msg = v
                .get("message")
                .and_then(Value::as_str)
                .unwrap_or("request failed");
            anyhow::bail!("A3S Runtime returned {code}: {msg}");
        }
        Ok(v.get("data").cloned().unwrap_or(v))
    }
}

fn is_loopback_origin(origin: &str) -> bool {
    origin.starts_with("http://127.")
        || origin.starts_with("https://127.")
        || origin.starts_with("http://localhost")
        || origin.starts_with("https://localhost")
        || origin.starts_with("http://[::1]")
        || origin.starts_with("https://[::1]")
}

#[async_trait]
impl Tool for RuntimeTool {
    fn name(&self) -> &str {
        "runtime"
    }

    fn description(&self) -> &str {
        "Offload independent subtasks to OS A3S Runtime for parallel remote \
         execution, stream progress while they run, then return a combined result. \
         Use it for decomposable work such as multiple deep-research subquestions. \
         `worker` is a tool-kind agent asset UUID or name. Names auto-resolve; \
         invalid names list the available workers."
    }

    fn parameters(&self) -> Value {
        json!({
            "type": "object",
            "properties": {
                "tasks": {
                    "type": "array",
                    "description": "Independent subtasks to run in parallel. Each item is passed to the worker as input, usually a subquestion string or an object matching the worker inputSchema.",
                    "items": { "type": ["string", "object"] },
                    "minItems": 1
                },
                "worker": {
                    "type": "string",
                    "description": "Worker that runs each subtask: a tool-kind agent asset UUID or name. Required."
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Maximum wait for the full batch in milliseconds. Defaults to 10 minutes; on timeout, returns completed results."
                }
            },
            "required": ["tasks", "worker"]
        })
    }

    fn requires_confirmation(&self, _args: &Value) -> bool {
        // This tool submits work to a remote runtime and can incur external
        // side effects or cost. Default mode may authorize that exact call;
        // non-interactive Auto mode converts the escalation into a denial.
        true
    }

    async fn execute(&self, args: &Value, ctx: &ToolContext) -> Result<ToolOutput> {
        let tasks: Vec<Value> = match args.get("tasks").and_then(Value::as_array) {
            Some(a) if !a.is_empty() => a.clone(),
            _ => return Ok(ToolOutput::error("`tasks` must be a non-empty array")),
        };
        let worker = match args.get("worker").and_then(Value::as_str) {
            Some(w) if !w.trim().is_empty() => w.trim().to_string(),
            _ => {
                return Ok(ToolOutput::error(
                    "`worker` is required: use a tool-kind agent asset UUID or name",
                ))
            }
        };
        let budget_ms = args
            .get("timeout_ms")
            .and_then(Value::as_u64)
            .unwrap_or(DEFAULT_BATCH_TIMEOUT_MS);

        match self.run_batch(&worker, tasks, budget_ms, ctx).await {
            Ok(out) => Ok(ToolOutput::success(out)),
            // A3S Runtime / network failure is a tool failure, not a crash — surface it.
            Err(e) => Ok(ToolOutput::error(format!(
                "A3S Runtime offload failed: {e}"
            ))),
        }
    }
}

impl RuntimeTool {
    async fn run_batch(
        &self,
        worker: &str,
        tasks: Vec<Value>,
        budget_ms: u64,
        ctx: &ToolContext,
    ) -> Result<String> {
        let client = self.client()?;
        let n = tasks.len();
        let progress = |msg: String| {
            if let Some(tx) = &ctx.event_tx {
                // Best-effort: progress must never block or fail the batch.
                let _ = tx.try_send(ToolStreamEvent::OutputDelta(msg));
            }
        };

        // 0. Resolve a worker NAME to its asset UUID (the OS API only accepts
        //    UUIDs). UUIDs pass straight through.
        let worker_id = if looks_like_uuid(worker) {
            worker.to_string()
        } else {
            let id = self.resolve_worker_name(&client, worker).await?;
            progress(format!("worker {worker} -> {id}\n"));
            id
        };

        // 1. Fan out. idempotencyKey (hash of worker + task set) makes a retry
        //    re-attach to the same batch instead of double-spending.
        let idem = idempotency_key(&worker_id, &tasks);
        let submit_url = format!("{}/api/v1/functions/{}/batch", self.origin, worker_id);
        let resp = client
            .post(&submit_url)
            .bearer_auth(&self.token)
            .json(&json!({ "inputs": tasks, "agentKind": "tool", "idempotencyKey": idem }))
            .send()
            .await?;
        let status = resp.status().as_u16();
        let data = Self::unwrap_envelope(&resp.text().await?, status)?;
        let batch_id = data
            .get("batchId")
            .and_then(Value::as_str)
            .ok_or_else(|| anyhow::anyhow!("batch response did not include batchId"))?
            .to_string();
        let invocation_ids: Vec<String> = data
            .get("invocationIds")
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        progress(format!(
            "{n} parallel subtasks submitted (batch {batch_id})\n"
        ));
        emit_runtime_subagent_starts(ctx, &batch_id, &invocation_ids, &tasks);

        // 2. Poll until every member is terminal or the budget expires — with
        //    exponential backoff, live progress, and transient-failure tolerance.
        let poll_url = format!("{}/api/v1/functions/batches/{}", self.origin, batch_id);
        let mut waited = 0u64;
        let mut interval = self.poll_start;
        let mut consecutive_failures = 0u32;
        let mut last_report = String::new();
        let mut timed_out_pending = 0u64;
        loop {
            let poll = async {
                let resp = client
                    .get(&poll_url)
                    .bearer_auth(&self.token)
                    .send()
                    .await?;
                let status = resp.status().as_u16();
                Self::unwrap_envelope(&resp.text().await?, status)
            }
            .await;
            match poll {
                Ok(bd) => {
                    consecutive_failures = 0;
                    let batch_progress = batch_progress(&bd, n);
                    let (done, running, queued, pending) = (
                        batch_progress.done,
                        batch_progress.running,
                        batch_progress.queued,
                        batch_progress.pending,
                    );
                    // Emit progress only when the picture changes (no spam).
                    let report =
                        format!("⏳ {done}/{n} done · {running} running · {queued} queued\n");
                    if report != last_report {
                        progress(report.clone());
                        last_report = report;
                    }
                    if pending == 0 {
                        break;
                    }
                    if waited >= budget_ms {
                        timed_out_pending = pending;
                        break;
                    }
                }
                Err(e) => {
                    // Tolerate flaky ticks: one failed GET must not abandon a
                    // whole running batch.
                    consecutive_failures += 1;
                    if consecutive_failures > MAX_POLL_FAILURES {
                        return Err(e.context(format!(
                            "polling batch {batch_id} failed {consecutive_failures} consecutive times"
                        )));
                    }
                    progress(format!(
                        "poll failed (attempt {consecutive_failures}; retrying)\n"
                    ));
                }
            }
            tokio::time::sleep(interval).await;
            waited += interval.as_millis() as u64;
            interval = (interval * 3 / 2).min(self.poll_cap);
        }

        // 3. Collect every member's result CONCURRENTLY (one RTT, not N).
        //    On timeout this still runs: finished members' outputs are returned
        //    (partial) instead of being thrown away.
        let fetches = invocation_ids.iter().map(|id| {
            let url = format!("{}/api/v1/functions/invocations/{}", self.origin, id);
            let client = client.clone();
            let token = self.token.clone();
            async move {
                let inv = async {
                    let resp = client.get(&url).bearer_auth(&token).send().await?;
                    let status = resp.status().as_u16();
                    Self::unwrap_envelope(&resp.text().await?, status)
                }
                .await
                .unwrap_or_else(|e| json!({ "error": e.to_string() }));
                let result = inv.get("result").cloned().unwrap_or(inv);
                json!({
                    "invocationId": id,
                    "state": result.get("status").cloned().unwrap_or_else(|| json!("unknown")),
                    "output": result.get("output").cloned().unwrap_or(Value::Null),
                    "error": result.get("error").cloned().unwrap_or(Value::Null),
                })
            }
        });
        let results: Vec<Value> = futures::future::join_all(fetches).await;
        emit_runtime_subagent_ends(ctx, &batch_id, &invocation_ids, &results);

        let mut summary = json!({
            "batchId": batch_id,
            "worker": worker_id,
            "count": n,
            "results": results,
        });
        if timed_out_pending > 0 {
            summary["partial"] = json!(true);
            summary["note"] = json!(format!(
                "Timed out after {waited}ms with {timed_out_pending} subtasks still pending; \
                 completed results were returned. Query batchId={batch_id} later for unfinished items."
            ));
        }
        Ok(serde_json::to_string_pretty(&summary)?)
    }

    /// Resolve a worker asset NAME to its UUID via the assets API. Fails with
    /// the list of available tool-kind workers so the model can self-correct.
    async fn resolve_worker_name(&self, client: &reqwest::Client, name: &str) -> Result<String> {
        let url = format!("{}/api/v1/assets?category=agent&limit=100", self.origin);
        let resp = client.get(&url).bearer_auth(&self.token).send().await?;
        let status = resp.status().as_u16();
        let data = Self::unwrap_envelope(&resp.text().await?, status)?;
        let items = data
            .get("items")
            .or_else(|| data.get("list"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        let tools: Vec<(&str, &str)> = items
            .iter()
            .filter(|a| a.get("agentKind").and_then(Value::as_str) == Some("tool"))
            .filter_map(|a| {
                Some((
                    a.get("id").and_then(Value::as_str)?,
                    a.get("name").and_then(Value::as_str)?,
                ))
            })
            .collect();
        if let Some((id, _)) = tools
            .iter()
            .find(|(_, n)| n.eq_ignore_ascii_case(name.trim()))
        {
            return Ok(id.to_string());
        }
        let available: Vec<String> = tools
            .iter()
            .take(10)
            .map(|(id, n)| format!("{n} ({id})"))
            .collect();
        anyhow::bail!(
            "No tool-kind worker named \"{name}\". Available workers: {}",
            if available.is_empty() {
                "none; create a tool-kind agent asset in the OS Asset Center first".to_string()
            } else {
                available.join(", ")
            }
        )
    }
}

fn emit_runtime_subagent_starts(
    ctx: &ToolContext,
    batch_id: &str,
    invocation_ids: &[String],
    tasks: &[Value],
) {
    let Some(tx) = &ctx.agent_event_tx else {
        return;
    };
    let started_ms = epoch_ms();
    for (idx, invocation_id) in invocation_ids.iter().enumerate() {
        let _ = tx.send(AgentEvent::SubagentStart {
            task_id: runtime_subagent_task_id(invocation_id),
            session_id: format!("runtime-{batch_id}-{idx}"),
            parent_session_id: ctx.session_id.clone().unwrap_or_default(),
            agent: "runtime".to_string(),
            description: runtime_task_description(idx, tasks.get(idx)),
            started_ms,
        });
    }
}

fn emit_runtime_subagent_ends(
    ctx: &ToolContext,
    batch_id: &str,
    invocation_ids: &[String],
    results: &[Value],
) {
    let Some(tx) = &ctx.agent_event_tx else {
        return;
    };
    let finished_ms = epoch_ms();
    for (idx, invocation_id) in invocation_ids.iter().enumerate() {
        let result = results.get(idx).cloned().unwrap_or_else(|| json!({}));
        let state = result
            .get("state")
            .and_then(Value::as_str)
            .unwrap_or("unknown");
        let success = matches!(state, "succeeded" | "completed" | "success");
        let output = result
            .get("output")
            .filter(|value| !value.is_null())
            .or_else(|| result.get("error").filter(|value| !value.is_null()))
            .cloned()
            .unwrap_or(result);
        let output = value_to_compact_string(&output);
        let _ = tx.send(AgentEvent::SubagentEnd {
            task_id: runtime_subagent_task_id(invocation_id),
            session_id: format!("runtime-{batch_id}-{idx}"),
            agent: "runtime".to_string(),
            output,
            success,
            finished_ms,
        });
    }
}

fn batch_progress(batch: &Value, expected_count: usize) -> BatchProgress {
    let expected = expected_count as u64;
    if let Some(counts) = batch.get("counts").and_then(Value::as_object) {
        let count = |keys: &[&str]| {
            keys.iter()
                .filter_map(|key| counts.get(*key).and_then(Value::as_u64))
                .sum::<u64>()
        };
        let queued = count(&["queued", "pending", "created", "scheduled"]);
        let running = count(&["running", "in_progress", "processing", "active"]);
        let done = count(&[
            "succeeded",
            "success",
            "completed",
            "done",
            "failed",
            "errored",
            "error",
            "canceled",
            "cancelled",
            "unknown",
        ]);
        let observed = done + queued + running;
        let missing = expected.saturating_sub(observed);
        let queued = queued + missing;
        let pending = queued + running;
        return BatchProgress {
            done,
            running,
            queued,
            pending,
        };
    }

    if let Some(items) = batch_member_items(batch) {
        let mut done = 0u64;
        let mut running = 0u64;
        let mut queued = 0u64;
        let mut unknown = 0u64;
        for item in items {
            match batch_item_state(item).as_deref() {
                Some(state) if is_terminal_runtime_state(state) => done += 1,
                Some(state) if is_queued_runtime_state(state) => queued += 1,
                Some(state) if is_running_runtime_state(state) => running += 1,
                _ => unknown += 1,
            }
        }
        let observed = done + running + queued + unknown;
        let missing = expected.saturating_sub(observed);
        queued += unknown + missing;
        return BatchProgress {
            done,
            running,
            queued,
            pending: queued + running,
        };
    }

    if let Some(state) = batch_item_state(batch) {
        if is_terminal_runtime_state(&state) {
            return BatchProgress {
                done: expected,
                running: 0,
                queued: 0,
                pending: 0,
            };
        }
        if is_running_runtime_state(&state) {
            return BatchProgress {
                done: 0,
                running: expected,
                queued: 0,
                pending: expected,
            };
        }
        if is_queued_runtime_state(&state) {
            return BatchProgress {
                done: 0,
                running: 0,
                queued: expected,
                pending: expected,
            };
        }
    }

    BatchProgress {
        done: 0,
        running: 0,
        queued: expected,
        pending: expected,
    }
}

fn batch_member_items(batch: &Value) -> Option<&Vec<Value>> {
    for key in ["invocations", "items", "results", "tasks", "members"] {
        if let Some(items) = batch.get(key).and_then(Value::as_array) {
            return Some(items);
        }
    }
    None
}

fn batch_item_state(value: &Value) -> Option<String> {
    value
        .get("status")
        .or_else(|| value.get("state"))
        .or_else(|| value.pointer("/result/status"))
        .or_else(|| value.pointer("/result/state"))
        .or_else(|| value.pointer("/execution/status"))
        .or_else(|| value.pointer("/execution/state"))
        .and_then(Value::as_str)
        .map(|state| state.trim().to_ascii_lowercase())
}

fn is_terminal_runtime_state(state: &str) -> bool {
    matches!(
        state,
        "succeeded"
            | "success"
            | "completed"
            | "complete"
            | "done"
            | "failed"
            | "failure"
            | "errored"
            | "error"
            | "canceled"
            | "cancelled"
            | "unknown"
    )
}

fn is_running_runtime_state(state: &str) -> bool {
    matches!(
        state,
        "running" | "in_progress" | "processing" | "active" | "started" | "executing"
    )
}

fn is_queued_runtime_state(state: &str) -> bool {
    matches!(
        state,
        "queued" | "pending" | "created" | "scheduled" | "submitted" | "waiting"
    )
}

fn runtime_subagent_task_id(invocation_id: &str) -> String {
    format!("runtime-{invocation_id}")
}

fn runtime_task_description(idx: usize, task: Option<&Value>) -> String {
    let Some(task) = task else {
        return format!("Runtime task {}", idx + 1);
    };
    if let Some(title) = task.get("title").and_then(Value::as_str) {
        return truncate(title, 80);
    }
    if let Some(focus) = task.get("focus").and_then(Value::as_str) {
        return truncate(focus, 80);
    }
    if let Some(query) = task.get("query").and_then(Value::as_str) {
        return truncate(query, 80);
    }
    if let Some(text) = task.as_str() {
        return truncate(text, 80);
    }
    truncate(&value_to_compact_string(task), 80)
}

fn value_to_compact_string(value: &Value) -> String {
    value
        .as_str()
        .map(ToString::to_string)
        .unwrap_or_else(|| serde_json::to_string(value).unwrap_or_else(|_| "null".to_string()))
}

fn epoch_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or_default()
}

/// A canonical hyphenated UUID (the only `ref` form the OS batch API accepts).
fn looks_like_uuid(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 36
        && b.iter().enumerate().all(|(i, c)| match i {
            8 | 13 | 18 | 23 => *c == b'-',
            _ => c.is_ascii_hexdigit(),
        })
}

/// Deterministic idempotency key from the worker + task set (sha256, truncated) —
/// a retry of the same fan-out re-attaches to the existing batch instead of
/// double-spending, while the same tasks on a DIFFERENT worker stay distinct.
fn idempotency_key(worker: &str, tasks: &[Value]) -> String {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(worker.as_bytes());
    h.update([0u8]);
    h.update(serde_json::to_vec(tasks).unwrap_or_default());
    let hex: String = h.finalize().iter().map(|b| format!("{:02x}", b)).collect();
    format!("a3s-code-runtime-{}", &hex[..24])
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        // Char-based to avoid panicking on a multibyte boundary.
        format!("{}…", s.chars().take(max).collect::<String>())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Mutex};
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    // ── pure logic ──────────────────────────────────────────────────────────

    #[test]
    fn unwrap_envelope_returns_data_and_surfaces_errors() {
        let ok = RuntimeTool::unwrap_envelope(
            r#"{"code":200,"status":"OK","data":{"batchId":"b1"}}"#,
            200,
        )
        .unwrap();
        assert_eq!(ok.get("batchId").unwrap(), "b1");
        let err =
            RuntimeTool::unwrap_envelope(r#"{"code":403,"message":"Forbidden"}"#, 200).unwrap_err();
        assert!(err.to_string().contains("403") && err.to_string().contains("Forbidden"));
        assert!(RuntimeTool::unwrap_envelope(r#"{"message":"x"}"#, 500).is_err());
        assert!(RuntimeTool::unwrap_envelope("<html>502</html>", 502).is_err());
        let bare = RuntimeTool::unwrap_envelope(r#"{"batchId":"b2"}"#, 200).unwrap();
        assert_eq!(bare.get("batchId").unwrap(), "b2");
    }

    #[test]
    fn idempotency_key_covers_worker_and_task_set() {
        let a = idempotency_key("w1", &[json!("q1"), json!("q2")]);
        assert_eq!(a, idempotency_key("w1", &[json!("q1"), json!("q2")]));
        // Same tasks on a DIFFERENT worker must be a different batch.
        assert_ne!(a, idempotency_key("w2", &[json!("q1"), json!("q2")]));
        assert_ne!(a, idempotency_key("w1", &[json!("q2"), json!("q1")]));
        assert!(a.starts_with("a3s-code-runtime-") && a.len() == "a3s-code-runtime-".len() + 24);
    }

    #[test]
    fn uuid_detection_is_strict() {
        assert!(looks_like_uuid("57989959-0b1d-41da-974c-31ad8101df37"));
        assert!(!looks_like_uuid("risk-reporter"));
        assert!(!looks_like_uuid("57989959-0b1d-41da-974c-31ad8101df3")); // 35 chars
        assert!(!looks_like_uuid("g7989959-0b1d-41da-974c-31ad8101df37")); // non-hex
    }

    #[test]
    fn remote_runtime_calls_always_escalate_authorization() {
        let tool = RuntimeTool {
            origin: "https://runtime.example.invalid".to_string(),
            token: "test-token".to_string(),
            poll_start: Duration::from_millis(1),
            poll_cap: Duration::from_millis(1),
        };

        assert!(tool.requires_confirmation(&json!({
            "worker": "worker",
            "tasks": ["task"]
        })));
    }

    // ── mock A3S Runtime speaking the exact OS contract ─────────────────────

    /// Scripted mock state: each poll consumes the next `poll_plan` item and
    /// repeats the final item after the plan is exhausted.
    struct MockState {
        submit_path: Option<String>,
        submit_body: Option<String>,
        /// Each entry: Some(counts json) → 200 with those counts; None → HTTP 500.
        poll_plan: Vec<Option<String>>,
        poll_idx: usize,
    }

    async fn spawn_mock(state: Arc<Mutex<MockState>>) -> String {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        tokio::spawn(async move {
            loop {
                let Ok((mut sock, _)) = listener.accept().await else {
                    return;
                };
                let st = state.clone();
                tokio::spawn(async move {
                    let mut buf = vec![0u8; 16384];
                    let n = sock.read(&mut buf).await.unwrap_or(0);
                    let req = String::from_utf8_lossy(&buf[..n]).into_owned();
                    let line = req.lines().next().unwrap_or("").to_string();
                    let body = req.split("\r\n\r\n").nth(1).unwrap_or("").to_string();
                    let (status, payload) = route(&st, &line, &body);
                    let resp = format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{payload}",
                        payload.len()
                    );
                    let _ = sock.write_all(resp.as_bytes()).await;
                    let _ = sock.flush().await;
                });
            }
        });
        origin
    }

    fn route(st: &Arc<Mutex<MockState>>, line: &str, body: &str) -> (&'static str, String) {
        let env = |data: &str| format!(r#"{{"code":200,"status":"OK","data":{data}}}"#);
        // Specific paths BEFORE the generic "/batch" (substring overlap).
        if line.contains("/api/v1/assets") {
            return (
                "200 OK",
                env(r#"{"items":[
                    {"id":"57989959-0b1d-41da-974c-31ad8101df37","name":"risk-reporter","agentKind":"tool"},
                    {"id":"74af5078-7b53-4857-bf69-fc59c9fdce06","name":"shangfei-poc3","agentKind":"tool"},
                    {"id":"02f3b08e-9358-43aa-97c3-05981b57a1a2","name":"some-app","agentKind":"application"}
                ]}"#),
            );
        }
        if line.contains("/batches/") {
            let mut s = st.lock().unwrap();
            let i = s.poll_idx.min(s.poll_plan.len().saturating_sub(1));
            s.poll_idx += 1;
            return match s.poll_plan.get(i).cloned().flatten() {
                Some(payload) => {
                    let trimmed = payload.trim();
                    let data = if poll_payload_is_counts(trimmed) {
                        format!(r#"{{"batchId":"batch-1","counts":{trimmed}}}"#)
                    } else {
                        let inner = trimmed.trim_start_matches('{').trim_end_matches('}');
                        if inner.contains(r#""batchId""#) {
                            format!("{{{inner}}}")
                        } else {
                            format!(r#"{{"batchId":"batch-1",{inner}}}"#)
                        }
                    };
                    ("200 OK", env(&data))
                }
                None => ("500 Internal Server Error", "boom".to_string()),
            };
        }
        if line.contains("/invocations/inv-1") {
            return (
                "200 OK",
                env(
                    r#"{"status":"succeeded","result":{"status":"succeeded","output":{"answer":"alpha"},"error":null}}"#,
                ),
            );
        }
        if line.contains("/invocations/") {
            return ("200 OK", env(r#"{"status":"running","result":null}"#));
        }
        if line.contains("/batch") {
            let mut s = st.lock().unwrap();
            s.submit_path = Some(line.split_whitespace().nth(1).unwrap_or("").to_string());
            s.submit_body = Some(body.to_string());
            return (
                "200 OK",
                env(r#"{"batchId":"batch-1","invocationIds":["inv-1","inv-2"]}"#),
            );
        }
        ("404 Not Found", "{}".to_string())
    }

    fn poll_payload_is_counts(payload: &str) -> bool {
        payload.contains(r#""queued":"#)
            || payload.contains(r#""running":"#)
            || payload.contains(r#""succeeded":"#)
            || payload.contains(r#""failed":"#)
            || payload.contains(r#""canceled":"#)
            || payload.contains(r#""cancelled":"#)
    }

    fn fast_tool(origin: String) -> RuntimeTool {
        RuntimeTool {
            origin,
            token: "test-token".into(),
            poll_start: Duration::from_millis(10),
            poll_cap: Duration::from_millis(20),
        }
    }

    fn state(poll_plan: Vec<Option<&str>>) -> Arc<Mutex<MockState>> {
        Arc::new(Mutex::new(MockState {
            submit_path: None,
            submit_body: None,
            poll_plan: poll_plan.into_iter().map(|p| p.map(String::from)).collect(),
            poll_idx: 0,
        }))
    }

    /// A ToolContext with a progress channel; returns (ctx, drained-events fn).
    fn ctx_with_progress() -> (ToolContext, tokio::sync::mpsc::Receiver<ToolStreamEvent>) {
        let (tx, rx) = tokio::sync::mpsc::channel(64);
        let ctx = ToolContext::new(std::env::temp_dir()).with_event_tx(tx);
        (ctx, rx)
    }

    fn ctx_with_progress_and_agent_events() -> (
        ToolContext,
        tokio::sync::mpsc::Receiver<ToolStreamEvent>,
        tokio::sync::broadcast::Receiver<AgentEvent>,
    ) {
        let (tool_tx, tool_rx) = tokio::sync::mpsc::channel(64);
        let (agent_tx, agent_rx) = tokio::sync::broadcast::channel(64);
        let ctx = ToolContext::new(std::env::temp_dir())
            .with_event_tx(tool_tx)
            .with_agent_event_tx(agent_tx);
        (ctx, tool_rx, agent_rx)
    }

    #[tokio::test]
    async fn full_flow_streams_progress_and_aggregates() {
        // Two live ticks then terminal — exercises backoff + change-only progress.
        let st = state(vec![
            Some(r#"{"queued":1,"running":1,"succeeded":0,"failed":0}"#),
            Some(r#"{"queued":0,"running":1,"succeeded":1,"failed":0}"#),
            Some(r#"{"queued":0,"running":0,"succeeded":2,"failed":0}"#),
        ]);
        let origin = spawn_mock(st.clone()).await;
        let tool = fast_tool(origin);
        let (ctx, mut rx, mut agent_rx) = ctx_with_progress_and_agent_events();
        let out = tool
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "57989959-0b1d-41da-974c-31ad8101df37" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "{}", out.content);

        // Request contract: inputs + agentKind + worker-scoped idempotency key.
        let sent: Value =
            serde_json::from_str(st.lock().unwrap().submit_body.as_ref().unwrap()).unwrap();
        assert_eq!(sent["inputs"], json!(["a", "b"]));
        assert_eq!(sent["agentKind"], "tool");
        assert_eq!(
            sent["idempotencyKey"].as_str().unwrap(),
            idempotency_key(
                "57989959-0b1d-41da-974c-31ad8101df37",
                &[json!("a"), json!("b")]
            )
        );

        // Progress streamed: submit line + one line per distinct counts picture.
        let mut deltas = Vec::new();
        while let Ok(ev) = rx.try_recv() {
            let ToolStreamEvent::OutputDelta(s) = ev;
            deltas.push(s);
        }
        let all = deltas.join("");
        assert!(all.contains("2 parallel subtasks submitted"), "{all}");
        assert!(
            all.contains("0/2 done") && all.contains("2/2 done"),
            "{all}"
        );

        // Aggregation: both results, in invocation order.
        let agg: Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(agg["count"], 2);
        assert_eq!(agg["results"][0]["output"]["answer"], "alpha");
        assert!(
            agg.get("partial").is_none(),
            "terminal batch is not partial"
        );

        let mut starts = Vec::new();
        let mut ends = Vec::new();
        while let Ok(event) = agent_rx.try_recv() {
            match event {
                AgentEvent::SubagentStart {
                    task_id,
                    session_id,
                    agent,
                    description,
                    ..
                } => starts.push((task_id, session_id, agent, description)),
                AgentEvent::SubagentEnd {
                    task_id,
                    session_id,
                    agent,
                    ..
                } => ends.push((task_id, session_id, agent)),
                _ => {}
            }
        }
        assert_eq!(starts.len(), 2, "{starts:?}");
        assert_eq!(ends.len(), 2, "{ends:?}");
        assert_eq!(starts[0].0, "runtime-inv-1");
        assert_eq!(starts[0].1, "runtime-batch-1-0");
        assert_eq!(starts[0].2, "runtime");
        assert_eq!(starts[0].3, "a");
        assert_eq!(starts[1].0, "runtime-inv-2");
        assert_eq!(starts[1].1, "runtime-batch-1-1");
        assert_eq!(ends[0].0, "runtime-inv-1");
        assert_eq!(ends[0].1, starts[0].1);
        assert_eq!(ends[1].0, "runtime-inv-2");
        assert_eq!(ends[1].1, starts[1].1);
    }

    #[tokio::test]
    async fn transient_poll_failure_is_tolerated_but_persistent_is_not() {
        // One 500 tick between two good ones → still succeeds.
        let st = state(vec![
            Some(r#"{"queued":0,"running":1,"succeeded":1,"failed":0}"#),
            None, // 500
            Some(r#"{"queued":0,"running":0,"succeeded":2,"failed":0}"#),
        ]);
        let origin = spawn_mock(st).await;
        let tool = fast_tool(origin);
        let (ctx, _rx) = ctx_with_progress();
        let out = tool
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "57989959-0b1d-41da-974c-31ad8101df37" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "one flaky tick must not abandon the batch");

        // 4+ consecutive failures → gives up with the poll error surfaced.
        let st2 = state(vec![
            Some(r#"{"queued":0,"running":2,"succeeded":0,"failed":0}"#),
            None,
            None,
            None,
            None,
        ]);
        let origin2 = spawn_mock(st2).await;
        let tool2 = fast_tool(origin2);
        let (ctx2, _rx2) = ctx_with_progress();
        let out2 = tool2
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "57989959-0b1d-41da-974c-31ad8101df37" }),
                &ctx2,
            )
            .await
            .unwrap();
        assert!(!out2.success);
        assert!(
            out2.content.contains("consecutive times"),
            "{}",
            out2.content
        );
    }

    #[tokio::test]
    async fn timeout_returns_partial_results_not_nothing() {
        // Batch never finishes; budget expires after the first tick. The
        // finished member (inv-1) must still come back, flagged partial.
        let st = state(vec![Some(
            r#"{"queued":0,"running":1,"succeeded":1,"failed":0}"#,
        )]);
        let origin = spawn_mock(st).await;
        let tool = fast_tool(origin);
        let (ctx, _rx) = ctx_with_progress();
        let out = tool
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "57989959-0b1d-41da-974c-31ad8101df37", "timeout_ms": 1 }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "{}", out.content);
        let agg: Value = serde_json::from_str(&out.content).unwrap();
        assert_eq!(agg["partial"], true);
        assert!(agg["note"].as_str().unwrap().contains("batch-1"));
        assert_eq!(agg["results"][0]["output"]["answer"], "alpha"); // finished one kept
        assert_eq!(agg["results"][1]["state"], "unknown"); // unfinished: no result yet
    }

    #[tokio::test]
    async fn poll_without_counts_does_not_finish_until_status_is_terminal() {
        let st = state(vec![
            Some(r#"{"status":"running"}"#),
            Some(r#"{"status":"running"}"#),
            Some(r#"{"status":"completed"}"#),
        ]);
        let origin = spawn_mock(st.clone()).await;
        let tool = fast_tool(origin);
        let (ctx, _rx) = ctx_with_progress();
        let out = tool
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "57989959-0b1d-41da-974c-31ad8101df37" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(out.success, "{}", out.content);
        assert_eq!(
            st.lock().unwrap().poll_idx,
            3,
            "missing counts must not make the first poll look terminal"
        );
        let agg: Value = serde_json::from_str(&out.content).unwrap();
        assert!(
            agg.get("partial").is_none(),
            "terminal status should not be marked partial"
        );
    }

    #[tokio::test]
    async fn incomplete_counts_do_not_finish_until_expected_task_count_is_terminal() {
        let st = state(vec![
            Some(r#"{"queued":0,"running":0,"succeeded":1,"failed":0}"#),
            Some(r#"{"queued":0,"running":0,"succeeded":2,"failed":0}"#),
        ]);
        let origin = spawn_mock(st.clone()).await;
        let tool = fast_tool(origin);
        let (ctx, _rx) = ctx_with_progress();
        let out = tool
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "57989959-0b1d-41da-974c-31ad8101df37" }),
                &ctx,
            )
            .await
            .unwrap();

        assert!(out.success, "{}", out.content);
        assert_eq!(
            st.lock().unwrap().poll_idx,
            2,
            "counts with fewer terminal tasks than submitted must keep polling"
        );
        let agg: Value = serde_json::from_str(&out.content).unwrap();
        assert!(
            agg.get("partial").is_none(),
            "eventual full counts should not be marked partial"
        );
    }

    #[tokio::test]
    async fn worker_name_resolves_to_uuid_and_unknown_names_list_options() {
        let st = state(vec![Some(
            r#"{"queued":0,"running":0,"succeeded":2,"failed":0}"#,
        )]);
        let origin = spawn_mock(st.clone()).await;
        let tool = fast_tool(origin.clone());
        let (ctx, _rx) = ctx_with_progress();
        // Name → UUID: the submit URL must target the resolved asset id.
        let out = tool
            .execute(
                &json!({ "tasks": ["a", "b"], "worker": "risk-reporter" }),
                &ctx,
            )
            .await
            .unwrap();
        assert!(out.success, "{}", out.content);
        let path = st.lock().unwrap().submit_path.clone().unwrap();
        assert!(
            path.contains("/functions/57989959-0b1d-41da-974c-31ad8101df37/batch"),
            "{path}"
        );

        // Unknown name → error listing available tool workers (not applications).
        let (ctx2, _rx2) = ctx_with_progress();
        let out2 = tool
            .execute(
                &json!({ "tasks": ["a"], "worker": "no-such-worker" }),
                &ctx2,
            )
            .await
            .unwrap();
        assert!(!out2.success);
        assert!(out2.content.contains("risk-reporter"), "{}", out2.content);
        assert!(
            !out2.content.contains("some-app"),
            "application-kind assets are not workers"
        );
    }

    #[tokio::test]
    async fn bad_args_are_tool_errors_not_requests() {
        let tool = fast_tool("http://127.0.0.1:1".into()); // never contacted
        let ctx = ToolContext::new(std::env::temp_dir());
        let e1 = tool
            .execute(&json!({ "tasks": [], "worker": "w" }), &ctx)
            .await
            .unwrap();
        assert!(!e1.success);
        let e2 = tool
            .execute(&json!({ "tasks": ["x"] }), &ctx)
            .await
            .unwrap();
        assert!(!e2.success && e2.content.contains("worker"));
    }
}
