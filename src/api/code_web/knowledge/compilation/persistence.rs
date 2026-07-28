use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

use a3s_acl::{Block, Document, Value as AclValue};

use super::*;

const COMPILATION_STATE_PATH: &str = ".a3s/compilation.acl";
const COMPILATION_STATE_SCHEMA: &str = "a3s.knowledge-compilation.v1";
const COMPILATION_JOBS_PATH: &str = ".a3s/compilations";
static JOB_SEQUENCE: AtomicU64 = AtomicU64::new(1);

impl CompilationState {
    pub(super) fn summary(&self) -> CompilationSummary {
        CompilationSummary {
            policy: self.policy,
            phase: self.phase,
            source_digest: self.source_digest.clone(),
            last_compiled_digest: self.last_compiled_digest.clone(),
            pending_changes: self.pending_source_digest.is_some(),
            active_job_id: self.active_job_id.clone(),
            last_requested_at: self.last_requested_at.clone(),
            last_succeeded_at: self.last_succeeded_at.clone(),
            last_failed_at: self.last_failed_at.clone(),
            last_error: self.last_error.clone(),
            paused_reason: self.paused_reason.clone(),
            compiler_version: self.compiler_version.clone(),
            recompile_recommended: self.last_compiled_digest.is_some()
                && self.compiler_contract_version.as_deref() != Some(COMPILER_CONTRACT_VERSION),
            next_auto_compile_at: next_auto_compile_at(self),
            stable_window_seconds: SOURCE_STABLE_SECONDS,
            quiet_window_seconds: AUTO_QUIET_SECONDS as u64,
            minimum_interval_seconds: AUTO_MIN_INTERVAL_SECONDS as u64,
        }
    }
}

fn next_auto_compile_at(state: &CompilationState) -> Option<String> {
    if state.policy != CompilationPolicy::SmartAuto || state.pending_source_digest.is_none() {
        return None;
    }
    let quiet = state
        .change_detected_at
        .as_deref()
        .and_then(parse_time)
        .map(|time| time + chrono::Duration::seconds(AUTO_QUIET_SECONDS));
    let interval = state
        .last_auto_requested_at
        .as_deref()
        .and_then(parse_time)
        .map(|time| time + chrono::Duration::seconds(AUTO_MIN_INTERVAL_SECONDS));
    match (quiet, interval) {
        (Some(quiet), Some(interval)) => Some(quiet.max(interval).to_rfc3339()),
        (Some(time), None) | (None, Some(time)) => Some(time.to_rfc3339()),
        (None, None) => None,
    }
}

pub(super) fn read_state(base: &Path) -> Result<CompilationState, KnowledgeStoreError> {
    let path = base.join(COMPILATION_STATE_PATH);
    let source = match std::fs::read_to_string(&path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let snapshot = source_packages::load_snapshot(base)?;
            return Ok(CompilationState {
                source_digest: snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.content_digest.clone()),
                ..CompilationState::default()
            });
        }
        Err(error) => return Err(io_error(&path, error)),
    };
    let document = a3s_acl::parse_acl(&source).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("invalid {}: {error}", path.display()))
    })?;
    let block = document
        .blocks
        .iter()
        .find(|block| block.name == "compilation")
        .ok_or_else(|| {
            KnowledgeStoreError::Invalid(format!("{} has no compilation block", path.display()))
        })?;
    if required_string(block, "schema", &path)? != COMPILATION_STATE_SCHEMA {
        return Err(KnowledgeStoreError::Invalid(format!(
            "unsupported knowledge compilation schema in {}",
            path.display()
        )));
    }
    let last_compiled_digest = optional_string(block, "last_compiled_digest");
    // Schema-v1 state written before this field existed always used contract 1.
    let compiler_contract_version = optional_string(block, "compiler_contract_version")
        .or_else(|| last_compiled_digest.as_ref().map(|_| "1".to_string()));
    Ok(CompilationState {
        policy: CompilationPolicy::parse(&required_string(block, "policy", &path)?)?,
        phase: CompilationPhase::parse(&required_string(block, "phase", &path)?)?,
        source_digest: optional_string(block, "source_digest"),
        last_compiled_digest,
        pending_source_digest: optional_string(block, "pending_source_digest"),
        pending_manual: block
            .attributes
            .get("pending_manual")
            .and_then(AclValue::as_bool)
            .unwrap_or(false),
        active_job_id: optional_string(block, "active_job_id"),
        change_detected_at: optional_string(block, "change_detected_at"),
        last_requested_at: optional_string(block, "last_requested_at"),
        last_auto_requested_at: optional_string(block, "last_auto_requested_at"),
        last_succeeded_at: optional_string(block, "last_succeeded_at"),
        last_failed_at: optional_string(block, "last_failed_at"),
        last_error: optional_string(block, "last_error"),
        paused_reason: optional_string(block, "paused_reason"),
        compiler_version: optional_string(block, "compiler_version"),
        compiler_contract_version,
        retry_attempt: optional_string(block, "retry_attempt")
            .and_then(|value| value.parse().ok())
            .unwrap_or_default(),
        retry_at: optional_string(block, "retry_at"),
    })
}

pub(super) fn write_state(
    base: &Path,
    state: &CompilationState,
) -> Result<(), KnowledgeStoreError> {
    let path = base.join(COMPILATION_STATE_PATH);
    let mut attributes = HashMap::from([
        (
            "schema".to_string(),
            AclValue::String(COMPILATION_STATE_SCHEMA.to_string()),
        ),
        (
            "policy".to_string(),
            AclValue::String(state.policy.as_str().to_string()),
        ),
        (
            "phase".to_string(),
            AclValue::String(state.phase.as_str().to_string()),
        ),
        (
            "pending_manual".to_string(),
            AclValue::Bool(state.pending_manual),
        ),
        (
            "retry_attempt".to_string(),
            AclValue::String(state.retry_attempt.to_string()),
        ),
    ]);
    for (key, value) in [
        ("source_digest", &state.source_digest),
        ("last_compiled_digest", &state.last_compiled_digest),
        ("pending_source_digest", &state.pending_source_digest),
        ("active_job_id", &state.active_job_id),
        ("change_detected_at", &state.change_detected_at),
        ("last_requested_at", &state.last_requested_at),
        ("last_auto_requested_at", &state.last_auto_requested_at),
        ("last_succeeded_at", &state.last_succeeded_at),
        ("last_failed_at", &state.last_failed_at),
        ("last_error", &state.last_error),
        ("paused_reason", &state.paused_reason),
        ("compiler_version", &state.compiler_version),
        (
            "compiler_contract_version",
            &state.compiler_contract_version,
        ),
        ("retry_at", &state.retry_at),
    ] {
        if let Some(value) = value {
            attributes.insert(key.to_string(), AclValue::String(value.clone()));
        }
    }
    let document = Document {
        blocks: vec![Block {
            name: "compilation".to_string(),
            labels: Vec::new(),
            blocks: Vec::new(),
            attributes,
        }],
    };
    let rendered = a3s_acl::generate_acl(&document);
    a3s_acl::parse_acl(&rendered).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("generated compilation ACL is invalid: {error}"))
    })?;
    atomic_write(&path, rendered.as_bytes())
}

pub(super) fn list_jobs(base: &Path) -> Result<Vec<StoredCompilationJob>, KnowledgeStoreError> {
    let directory = base.join(COMPILATION_JOBS_PATH);
    let entries = match std::fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(&directory, error)),
    };
    let mut jobs = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|error| io_error(&directory, error))?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|error| io_error(&path, error))?;
        let job = serde_json::from_slice(&bytes).map_err(|error| {
            KnowledgeStoreError::Invalid(format!("invalid {}: {error}", path.display()))
        })?;
        jobs.push(job);
    }
    Ok(jobs)
}

pub(super) fn read_job(base: &Path, id: &str) -> Result<StoredCompilationJob, KnowledgeStoreError> {
    validate_job_id(id)?;
    let path = job_path(base, id);
    let bytes = std::fs::read(&path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            KnowledgeStoreError::NotFound(format!("knowledge compilation job `{id}` was not found"))
        } else {
            io_error(&path, error)
        }
    })?;
    serde_json::from_slice(&bytes).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("invalid {}: {error}", path.display()))
    })
}

pub(super) fn write_job(
    base: &Path,
    job: &StoredCompilationJob,
) -> Result<(), KnowledgeStoreError> {
    validate_job_id(&job.id)?;
    let bytes = serde_json::to_vec_pretty(job).map_err(|error| {
        KnowledgeStoreError::Invalid(format!("could not encode compilation job: {error}"))
    })?;
    atomic_write(&job_path(base, &job.id), &bytes)
}

fn job_path(base: &Path, id: &str) -> PathBuf {
    base.join(COMPILATION_JOBS_PATH).join(format!("{id}.json"))
}

fn validate_job_id(id: &str) -> Result<(), KnowledgeStoreError> {
    if id.is_empty()
        || id.len() > 96
        || !id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(KnowledgeStoreError::Invalid(
            "knowledge compilation job ID is invalid".to_string(),
        ));
    }
    Ok(())
}

pub(super) fn new_job_id(now: DateTime<Utc>) -> String {
    let sequence = JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!("job-{}-{sequence}", now.timestamp_micros().unsigned_abs())
}

pub(super) fn empty_claim() -> CompilationClaim {
    CompilationClaim {
        claimed: false,
        contract_version: COMPILER_CONTRACT_VERSION.to_string(),
        job: None,
        workspace_root: None,
        knowledge_base_path: None,
        source_path: None,
        previous_wiki_path: None,
        output_path: None,
    }
}

fn required_string(block: &Block, key: &str, path: &Path) -> Result<String, KnowledgeStoreError> {
    block
        .attributes
        .get(key)
        .and_then(AclValue::as_str)
        .map(str::to_string)
        .ok_or_else(|| {
            KnowledgeStoreError::Invalid(format!(
                "{} is missing string attribute `{key}`",
                path.display()
            ))
        })
}

fn optional_string(block: &Block, key: &str) -> Option<String> {
    block
        .attributes
        .get(key)
        .and_then(AclValue::as_str)
        .map(str::to_string)
}
