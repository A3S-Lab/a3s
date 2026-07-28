//! Persistent knowledge-compilation policy, queue, and worker handoff.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use super::personal_bases::{self, KnowledgeStoreError};
use super::source_packages::{self, SourceChanges};
use persistence::{
    empty_claim, list_jobs, new_job_id, read_job, read_state, write_job, write_state,
};

mod persistence;

const COMPILATION_OUTPUT_PATH: &str = ".a3s/compilation-output";
pub(super) const COMPILER_CONTRACT_VERSION: &str = "1";
pub(super) const SOURCE_STABLE_SECONDS: u64 = 5;
pub(super) const AUTO_QUIET_SECONDS: i64 = 30;
pub(super) const AUTO_MIN_INTERVAL_SECONDS: i64 = 10 * 60;
const RETRY_DELAYS_SECONDS: [i64; 3] = [5 * 60, 30 * 60, 2 * 60 * 60];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CompilationPolicy {
    #[default]
    Manual,
    SmartAuto,
}

impl CompilationPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::SmartAuto => "smart_auto",
        }
    }

    fn parse(value: &str) -> Result<Self, KnowledgeStoreError> {
        match value {
            "manual" => Ok(Self::Manual),
            "smart_auto" => Ok(Self::SmartAuto),
            _ => Err(KnowledgeStoreError::Invalid(format!(
                "unsupported knowledge compilation policy `{value}`"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CompilationPhase {
    #[default]
    SourceReady,
    Queued,
    Running,
    Succeeded,
    Failed,
    Paused,
}

impl CompilationPhase {
    fn as_str(self) -> &'static str {
        match self {
            Self::SourceReady => "source_ready",
            Self::Queued => "queued",
            Self::Running => "running",
            Self::Succeeded => "succeeded",
            Self::Failed => "failed",
            Self::Paused => "paused",
        }
    }

    fn parse(value: &str) -> Result<Self, KnowledgeStoreError> {
        match value {
            "source_ready" => Ok(Self::SourceReady),
            "queued" => Ok(Self::Queued),
            "running" => Ok(Self::Running),
            "succeeded" => Ok(Self::Succeeded),
            "failed" => Ok(Self::Failed),
            "paused" => Ok(Self::Paused),
            _ => Err(KnowledgeStoreError::Invalid(format!(
                "unsupported knowledge compilation phase `{value}`"
            ))),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CompilationTrigger {
    Manual,
    SmartAuto,
    Retry,
}

impl CompilationTrigger {
    fn priority(self) -> u8 {
        match self {
            Self::Manual => 0,
            Self::Retry => 1,
            Self::SmartAuto => 2,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CompilationJobPhase {
    Queued,
    Running,
    Succeeded,
    Failed,
    Cancelled,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompilationSummary {
    pub(super) policy: CompilationPolicy,
    pub(super) phase: CompilationPhase,
    pub(super) source_digest: Option<String>,
    pub(super) last_compiled_digest: Option<String>,
    pub(super) pending_changes: bool,
    pub(super) active_job_id: Option<String>,
    pub(super) last_requested_at: Option<String>,
    pub(super) last_succeeded_at: Option<String>,
    pub(super) last_failed_at: Option<String>,
    pub(super) last_error: Option<String>,
    pub(super) paused_reason: Option<String>,
    pub(super) compiler_version: Option<String>,
    pub(super) recompile_recommended: bool,
    pub(super) next_auto_compile_at: Option<String>,
    pub(super) stable_window_seconds: u64,
    pub(super) quiet_window_seconds: u64,
    pub(super) minimum_interval_seconds: u64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompilationJob {
    pub(super) id: String,
    pub(super) knowledge_base_id: String,
    pub(super) trigger: CompilationTrigger,
    pub(super) phase: CompilationJobPhase,
    pub(super) source_digest: String,
    pub(super) created_at: String,
    pub(super) started_at: Option<String>,
    pub(super) completed_at: Option<String>,
    pub(super) output_path: String,
    pub(super) error: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredCompilationJob {
    id: String,
    knowledge_base_id: String,
    trigger: CompilationTrigger,
    phase: CompilationJobPhase,
    source_digest: String,
    created_at: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    output_path: String,
    error: Option<String>,
}

impl From<StoredCompilationJob> for CompilationJob {
    fn from(job: StoredCompilationJob) -> Self {
        Self {
            id: job.id,
            knowledge_base_id: job.knowledge_base_id,
            trigger: job.trigger,
            phase: job.phase,
            source_digest: job.source_digest,
            created_at: job.created_at,
            started_at: job.started_at,
            completed_at: job.completed_at,
            output_path: job.output_path,
            error: job.error,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompilationMutation {
    pub(super) changed: bool,
    pub(super) knowledge_base: super::personal_bases::KnowledgeBase,
    pub(super) job: Option<CompilationJob>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct CompilationClaim {
    pub(super) claimed: bool,
    pub(super) contract_version: String,
    pub(super) job: Option<CompilationJob>,
    pub(super) workspace_root: Option<String>,
    pub(super) knowledge_base_path: Option<String>,
    pub(super) source_path: Option<String>,
    pub(super) previous_wiki_path: Option<String>,
    pub(super) output_path: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum CompilationOutcome {
    Succeeded,
    Failed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct CompilationState {
    policy: CompilationPolicy,
    phase: CompilationPhase,
    source_digest: Option<String>,
    last_compiled_digest: Option<String>,
    pending_source_digest: Option<String>,
    pending_manual: bool,
    active_job_id: Option<String>,
    change_detected_at: Option<String>,
    last_requested_at: Option<String>,
    last_auto_requested_at: Option<String>,
    last_succeeded_at: Option<String>,
    last_failed_at: Option<String>,
    last_error: Option<String>,
    paused_reason: Option<String>,
    compiler_version: Option<String>,
    compiler_contract_version: Option<String>,
    retry_attempt: usize,
    retry_at: Option<String>,
}

pub(super) fn initialize_for_source_package(
    staging: &Path,
    source_digest: &str,
    policy: CompilationPolicy,
    now: DateTime<Utc>,
) -> Result<(), KnowledgeStoreError> {
    let now = now.to_rfc3339();
    let state = CompilationState {
        policy,
        phase: CompilationPhase::SourceReady,
        source_digest: Some(source_digest.to_string()),
        pending_source_digest: Some(source_digest.to_string()),
        change_detected_at: Some(now),
        ..CompilationState::default()
    };
    write_state(staging, &state)
}

pub(super) fn summary_for_base(base: &Path) -> CompilationSummary {
    match read_state(base) {
        Ok(state) => state.summary(),
        Err(error) => CompilationSummary {
            phase: CompilationPhase::Failed,
            last_error: Some(error.to_string()),
            stable_window_seconds: SOURCE_STABLE_SECONDS,
            quiet_window_seconds: AUTO_QUIET_SECONDS as u64,
            minimum_interval_seconds: AUTO_MIN_INTERVAL_SECONDS as u64,
            ..CompilationSummary::default()
        },
    }
}

pub(super) fn status(
    workspace: &Path,
    id: &str,
) -> Result<CompilationSummary, KnowledgeStoreError> {
    let base = personal_bases::resolve_base_path(workspace, id)?;
    Ok(read_state(&base)?.summary())
}

pub(super) fn request_compilation(
    workspace: &Path,
    id: &str,
    now: DateTime<Utc>,
) -> Result<CompilationMutation, KnowledgeStoreError> {
    let base = personal_bases::resolve_base_path(workspace, id)?;
    let mut state = read_state(&base)?;
    let previous = source_packages::load_snapshot(&base)?;
    let plan =
        source_packages::scan_base_sources(&base, previous.as_ref(), Duration::ZERO, now.into())?;
    source_packages::sync_source_package(&base, &plan)?;
    state.source_digest = Some(plan.snapshot.content_digest.clone());
    state.paused_reason = None;
    state.last_error = None;

    if let Some(active_id) = state.active_job_id.clone() {
        let mut active = read_job(&base, &active_id)?;
        match active.phase {
            CompilationJobPhase::Running => {
                state.pending_source_digest = Some(plan.snapshot.content_digest);
                state.pending_manual = true;
                write_state(&base, &state)?;
                return mutation(workspace, id, false, Some(active));
            }
            CompilationJobPhase::Queued => {
                active.trigger = CompilationTrigger::Manual;
                active.source_digest = plan.snapshot.content_digest;
                write_job(&base, &active)?;
                state.phase = CompilationPhase::Queued;
                state.last_requested_at = Some(now.to_rfc3339());
                write_state(&base, &state)?;
                return mutation(workspace, id, true, Some(active));
            }
            _ => state.active_job_id = None,
        }
    }
    let job = enqueue_job(
        &base,
        id,
        CompilationTrigger::Manual,
        &plan.snapshot.content_digest,
        now,
        &mut state,
    )?;
    mutation(workspace, id, true, Some(job))
}

pub(super) fn set_policy(
    workspace: &Path,
    id: &str,
    policy: CompilationPolicy,
    now: DateTime<Utc>,
) -> Result<CompilationMutation, KnowledgeStoreError> {
    let base = personal_bases::resolve_base_path(workspace, id)?;
    let mut state = read_state(&base)?;
    if state.policy == policy {
        return mutation(workspace, id, false, None);
    }
    state.policy = policy;
    state.paused_reason = None;
    if policy == CompilationPolicy::SmartAuto {
        let previous = source_packages::load_snapshot(&base)?;
        let plan = source_packages::scan_base_sources(
            &base,
            previous.as_ref(),
            Duration::ZERO,
            now.into(),
        )?;
        source_packages::sync_source_package(&base, &plan)?;
        let source_digest = plan.snapshot.content_digest;
        let compilation_needed = state.last_compiled_digest.as_deref() != Some(&source_digest);
        state.source_digest = Some(source_digest.clone());
        if compilation_needed {
            state.pending_source_digest = Some(source_digest);
            state.change_detected_at = Some(now.to_rfc3339());
        } else {
            state.pending_source_digest = None;
            state.change_detected_at = None;
        }
        if state.active_job_id.is_none() {
            state.phase = fallback_phase(&state);
        }
    } else if let Some(active_id) = state.active_job_id.clone() {
        let mut job = read_job(&base, &active_id)?;
        if job.phase == CompilationJobPhase::Queued && job.trigger == CompilationTrigger::SmartAuto
        {
            job.phase = CompilationJobPhase::Cancelled;
            job.completed_at = Some(now.to_rfc3339());
            write_job(&base, &job)?;
            state.active_job_id = None;
            state.phase = fallback_phase(&state);
        }
    }
    write_state(&base, &state)?;
    mutation(workspace, id, true, None)
}

pub(super) fn inspect_source_changes(
    workspace: &Path,
    id: &str,
    now: DateTime<Utc>,
) -> Result<SourceChanges, KnowledgeStoreError> {
    let base = personal_bases::resolve_base_path(workspace, id)?;
    let previous = source_packages::load_snapshot(&base)?;
    let plan = source_packages::scan_base_sources(
        &base,
        previous.as_ref(),
        Duration::from_secs(SOURCE_STABLE_SECONDS),
        now.into(),
    )?;
    let changes = source_packages::source_changes(previous.as_ref(), &plan);
    if !changes.changed && changes.unstable_paths.is_empty() {
        source_packages::write_snapshot(&base, &plan.snapshot)?;
    }
    Ok(changes)
}

pub(super) fn poll_compilations(
    workspace: &Path,
    now: DateTime<Utc>,
) -> Result<usize, KnowledgeStoreError> {
    let bases = personal_bases::list_knowledge_bases(workspace);
    let mut changed_bases = 0usize;
    for knowledge_base in bases.items {
        let base = PathBuf::from(&knowledge_base.path);
        let mut state = match read_state(&base) {
            Ok(state) => state,
            Err(error) => {
                tracing::warn!(
                    knowledge_base = %knowledge_base.id,
                    %error,
                    "could not read knowledge compilation state"
                );
                continue;
            }
        };
        if state.phase == CompilationPhase::Failed && retry_due(&state, now) {
            if let Some(digest) = state.source_digest.clone() {
                enqueue_job(
                    &base,
                    &knowledge_base.id,
                    CompilationTrigger::Retry,
                    &digest,
                    now,
                    &mut state,
                )?;
                changed_bases += 1;
            }
            continue;
        }
        if state.policy != CompilationPolicy::SmartAuto {
            continue;
        }
        let previous = match source_packages::load_snapshot(&base) {
            Ok(previous) => previous,
            Err(error) => {
                pause_state(&base, &mut state, error.to_string())?;
                changed_bases += 1;
                continue;
            }
        };
        let plan = match source_packages::scan_base_sources(
            &base,
            previous.as_ref(),
            Duration::from_secs(SOURCE_STABLE_SECONDS),
            now.into(),
        ) {
            Ok(plan) => plan,
            Err(error) => {
                pause_state(&base, &mut state, error.to_string())?;
                changed_bases += 1;
                continue;
            }
        };
        let changes = source_packages::source_changes(previous.as_ref(), &plan);
        if !changes.unstable_paths.is_empty() {
            state.change_detected_at = Some(now.to_rfc3339());
            write_state(&base, &state)?;
            changed_bases += 1;
            continue;
        }
        if let Some(reason) = changes.safety_pause.clone() {
            pause_state(&base, &mut state, reason)?;
            changed_bases += 1;
            continue;
        }
        if changes.changed {
            source_packages::sync_source_package(&base, &plan)?;
            state.source_digest = Some(plan.snapshot.content_digest.clone());
            state.pending_source_digest = Some(plan.snapshot.content_digest);
            state.change_detected_at = Some(now.to_rfc3339());
            state.paused_reason = None;
            state.last_error = None;
            if state.phase != CompilationPhase::Running {
                state.phase = CompilationPhase::SourceReady;
            }
            write_state(&base, &state)?;
            changed_bases += 1;
            continue;
        }
        source_packages::write_snapshot(&base, &plan.snapshot)?;

        if state.active_job_id.is_none() && auto_compile_due(&state, now) {
            if let Some(digest) = state.pending_source_digest.clone() {
                enqueue_job(
                    &base,
                    &knowledge_base.id,
                    CompilationTrigger::SmartAuto,
                    &digest,
                    now,
                    &mut state,
                )?;
                changed_bases += 1;
            }
        }
    }
    Ok(changed_bases)
}

#[cfg(test)]
fn claim_next(
    workspace: &Path,
    now: DateTime<Utc>,
) -> Result<CompilationClaim, KnowledgeStoreError> {
    claim_next_in_workspaces(&[workspace.to_path_buf()], now)
}

pub(super) fn claim_next_in_workspaces(
    workspaces: &[PathBuf],
    now: DateTime<Utc>,
) -> Result<CompilationClaim, KnowledgeStoreError> {
    let mut queued = Vec::<(PathBuf, PathBuf, StoredCompilationJob)>::new();
    for workspace in workspaces {
        let bases = personal_bases::list_knowledge_bases(workspace);
        for knowledge_base in &bases.items {
            let base = PathBuf::from(&knowledge_base.path);
            for job in list_jobs(&base)? {
                if job.phase == CompilationJobPhase::Running {
                    return Ok(empty_claim());
                }
                if job.phase == CompilationJobPhase::Queued {
                    queued.push((workspace.clone(), base.clone(), job));
                }
            }
        }
    }
    queued.sort_by(|left, right| {
        left.2
            .trigger
            .priority()
            .cmp(&right.2.trigger.priority())
            .then_with(|| left.2.created_at.cmp(&right.2.created_at))
            .then_with(|| left.2.id.cmp(&right.2.id))
            .then_with(|| left.0.cmp(&right.0))
    });
    let Some((workspace, base, mut job)) = queued.into_iter().next() else {
        return Ok(empty_claim());
    };
    job.phase = CompilationJobPhase::Running;
    job.started_at = Some(now.to_rfc3339());
    let output = PathBuf::from(&job.output_path);
    if output.exists() {
        std::fs::remove_dir_all(&output).map_err(|error| io_error(&output, error))?;
    }
    std::fs::create_dir_all(output.join("wiki")).map_err(|error| io_error(&output, error))?;
    write_job(&base, &job)?;
    let mut state = read_state(&base)?;
    state.phase = CompilationPhase::Running;
    state.active_job_id = Some(job.id.clone());
    write_state(&base, &state)?;
    Ok(CompilationClaim {
        claimed: true,
        contract_version: COMPILER_CONTRACT_VERSION.to_string(),
        job: Some(job.clone().into()),
        workspace_root: Some(workspace.display().to_string()),
        knowledge_base_path: Some(base.display().to_string()),
        source_path: Some(base.join("sources").display().to_string()),
        previous_wiki_path: Some(base.join("wiki").display().to_string()),
        output_path: Some(output.join("wiki").display().to_string()),
    })
}

#[allow(clippy::too_many_arguments)]
pub(super) fn complete_job(
    workspace: &Path,
    id: &str,
    job_id: &str,
    outcome: CompilationOutcome,
    transient: bool,
    error: Option<&str>,
    compiler_version: Option<&str>,
    now: DateTime<Utc>,
) -> Result<CompilationMutation, KnowledgeStoreError> {
    let base = personal_bases::resolve_base_path(workspace, id)?;
    let mut job = read_job(&base, job_id)?;
    if job.phase != CompilationJobPhase::Running {
        return Err(KnowledgeStoreError::Conflict(format!(
            "knowledge compilation job `{job_id}` is not running"
        )));
    }
    let mut state = read_state(&base)?;
    if state.active_job_id.as_deref() != Some(job_id) {
        return Err(KnowledgeStoreError::Conflict(format!(
            "knowledge compilation job `{job_id}` is no longer active"
        )));
    }
    job.completed_at = Some(now.to_rfc3339());
    state.active_job_id = None;
    match outcome {
        CompilationOutcome::Succeeded => {
            promote_compiled_wiki(&base, Path::new(&job.output_path))?;
            job.phase = CompilationJobPhase::Succeeded;
            state.last_compiled_digest = Some(job.source_digest.clone());
            state.last_succeeded_at = Some(now.to_rfc3339());
            state.last_error = None;
            state.last_failed_at = None;
            state.paused_reason = None;
            state.retry_attempt = 0;
            state.retry_at = None;
            state.compiler_version = compiler_version
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .map(str::to_string)
                .or_else(|| Some(COMPILER_CONTRACT_VERSION.to_string()));
            state.compiler_contract_version = Some(COMPILER_CONTRACT_VERSION.to_string());
            if state.pending_source_digest.as_deref() == Some(job.source_digest.as_str()) {
                state.pending_source_digest = None;
            }
            state.phase = if state.pending_source_digest.is_some() {
                CompilationPhase::SourceReady
            } else {
                CompilationPhase::Succeeded
            };
        }
        CompilationOutcome::Failed => {
            let message = error
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .unwrap_or("knowledge compiler reported a failure")
                .chars()
                .take(1_000)
                .collect::<String>();
            job.phase = CompilationJobPhase::Failed;
            job.error = Some(message.clone());
            state.phase = CompilationPhase::Failed;
            state.last_error = Some(message);
            state.last_failed_at = Some(now.to_rfc3339());
            if transient && state.retry_attempt < RETRY_DELAYS_SECONDS.len() {
                let delay = RETRY_DELAYS_SECONDS[state.retry_attempt];
                state.retry_attempt += 1;
                state.retry_at = Some((now + chrono::Duration::seconds(delay)).to_rfc3339());
            } else {
                state.retry_at = None;
            }
        }
    }
    write_job(&base, &job)?;
    write_state(&base, &state)?;
    if outcome == CompilationOutcome::Succeeded && state.pending_manual {
        state.pending_manual = false;
        if let Some(digest) = state.pending_source_digest.clone() {
            let follow_up = enqueue_job(
                &base,
                id,
                CompilationTrigger::Manual,
                &digest,
                now,
                &mut state,
            )?;
            return mutation(workspace, id, true, Some(follow_up));
        }
    }
    mutation(workspace, id, true, Some(job))
}

pub(super) fn cancel_job(
    workspace: &Path,
    id: &str,
    job_id: &str,
    now: DateTime<Utc>,
) -> Result<CompilationMutation, KnowledgeStoreError> {
    let base = personal_bases::resolve_base_path(workspace, id)?;
    let mut job = read_job(&base, job_id)?;
    if !matches!(
        job.phase,
        CompilationJobPhase::Queued | CompilationJobPhase::Running
    ) {
        return mutation(workspace, id, false, Some(job));
    }
    job.phase = CompilationJobPhase::Cancelled;
    job.completed_at = Some(now.to_rfc3339());
    write_job(&base, &job)?;
    let mut state = read_state(&base)?;
    if state.active_job_id.as_deref() == Some(job_id) {
        state.active_job_id = None;
        state.phase = fallback_phase(&state);
        write_state(&base, &state)?;
    }
    mutation(workspace, id, true, Some(job))
}

fn enqueue_job(
    base: &Path,
    id: &str,
    trigger: CompilationTrigger,
    source_digest: &str,
    now: DateTime<Utc>,
    state: &mut CompilationState,
) -> Result<StoredCompilationJob, KnowledgeStoreError> {
    let job_id = new_job_id(now);
    let output = base.join(COMPILATION_OUTPUT_PATH).join(&job_id);
    let job = StoredCompilationJob {
        id: job_id.clone(),
        knowledge_base_id: id.to_string(),
        trigger,
        phase: CompilationJobPhase::Queued,
        source_digest: source_digest.to_string(),
        created_at: now.to_rfc3339(),
        started_at: None,
        completed_at: None,
        output_path: output.display().to_string(),
        error: None,
    };
    write_job(base, &job)?;
    state.phase = CompilationPhase::Queued;
    state.active_job_id = Some(job_id);
    state.last_requested_at = Some(now.to_rfc3339());
    if trigger == CompilationTrigger::SmartAuto {
        state.last_auto_requested_at = Some(now.to_rfc3339());
    }
    state.last_error = None;
    state.paused_reason = None;
    write_state(base, state)?;
    Ok(job)
}

fn mutation(
    workspace: &Path,
    id: &str,
    changed: bool,
    job: Option<StoredCompilationJob>,
) -> Result<CompilationMutation, KnowledgeStoreError> {
    Ok(CompilationMutation {
        changed,
        knowledge_base: personal_bases::knowledge_base_by_id(workspace, id)?,
        job: job.map(CompilationJob::from),
    })
}

fn auto_compile_due(state: &CompilationState, now: DateTime<Utc>) -> bool {
    if state.policy != CompilationPolicy::SmartAuto || state.pending_source_digest.is_none() {
        return false;
    }
    let quiet = state
        .change_detected_at
        .as_deref()
        .and_then(parse_time)
        .is_some_and(|changed| {
            now.signed_duration_since(changed).num_seconds() >= AUTO_QUIET_SECONDS
        });
    let interval = state
        .last_auto_requested_at
        .as_deref()
        .and_then(parse_time)
        .is_none_or(|last| {
            now.signed_duration_since(last).num_seconds() >= AUTO_MIN_INTERVAL_SECONDS
        });
    quiet && interval
}

fn retry_due(state: &CompilationState, now: DateTime<Utc>) -> bool {
    state
        .retry_at
        .as_deref()
        .and_then(parse_time)
        .is_some_and(|retry_at| now >= retry_at)
        && state.active_job_id.is_none()
}

fn pause_state(
    base: &Path,
    state: &mut CompilationState,
    reason: String,
) -> Result<(), KnowledgeStoreError> {
    state.phase = CompilationPhase::Paused;
    state.paused_reason = Some(reason.chars().take(1_000).collect());
    write_state(base, state)
}

fn fallback_phase(state: &CompilationState) -> CompilationPhase {
    if state.pending_source_digest.is_some() {
        CompilationPhase::SourceReady
    } else if state.last_compiled_digest.is_some() {
        CompilationPhase::Succeeded
    } else {
        CompilationPhase::SourceReady
    }
}

fn promote_compiled_wiki(base: &Path, output: &Path) -> Result<(), KnowledgeStoreError> {
    let wiki = output.join("wiki");
    let metadata = std::fs::symlink_metadata(&wiki).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KnowledgeStoreError::Invalid(format!(
                "compiler output is missing its wiki directory: {}",
                wiki.display()
            ))
        } else {
            io_error(&wiki, error)
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_dir() || !contains_markdown(&wiki) {
        return Err(KnowledgeStoreError::Invalid(format!(
            "compiler output must contain at least one regular Markdown file under {}",
            wiki.display()
        )));
    }
    let target = base.join("wiki");
    let backup = base.join(format!(
        ".wiki.backup-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    if target.exists() {
        std::fs::rename(&target, &backup).map_err(|error| io_error(&target, error))?;
    }
    if let Err(error) = std::fs::rename(&wiki, &target) {
        if backup.exists() {
            let _ = std::fs::rename(&backup, &target);
        }
        return Err(io_error(&target, error));
    }
    if backup.exists() {
        std::fs::remove_dir_all(&backup).map_err(|error| io_error(&backup, error))?;
    }
    let _ = std::fs::remove_dir_all(output);
    Ok(())
}

fn contains_markdown(directory: &Path) -> bool {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return false;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.file_type().is_symlink() {
            continue;
        }
        if metadata.is_dir() && contains_markdown(&path) {
            return true;
        }
        if metadata.is_file() && path.extension().and_then(|value| value.to_str()) == Some("md") {
            return true;
        }
    }
    false
}

fn parse_time(value: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .ok()
        .map(|value| value.with_timezone(&Utc))
}

fn atomic_write(path: &Path, content: &[u8]) -> Result<(), KnowledgeStoreError> {
    let parent = path.parent().ok_or_else(|| {
        KnowledgeStoreError::Invalid(format!("{} has no parent directory", path.display()))
    })?;
    std::fs::create_dir_all(parent).map_err(|error| io_error(parent, error))?;
    let temporary = parent.join(format!(
        ".compilation.tmp-{}-{}",
        std::process::id(),
        timestamp_nanos()
    ));
    std::fs::write(&temporary, content).map_err(|error| io_error(&temporary, error))?;
    if let Err(error) = std::fs::rename(&temporary, path) {
        let _ = std::fs::remove_file(&temporary);
        return Err(io_error(path, error));
    }
    Ok(())
}

fn timestamp_nanos() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default()
}

fn io_error(path: &Path, error: std::io::Error) -> KnowledgeStoreError {
    KnowledgeStoreError::Io(format!("{}: {error}", path.display()))
}

#[cfg(test)]
mod tests;
