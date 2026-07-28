use std::io::Write;
use std::path::Path;

use a3s_deep_research::engine::{
    DeepResearchEvent, DeepResearchLifecycle, PublicationOutcome, ResearchStage,
};
use a3s_deep_research::report::DeepResearchPublicationQuality;
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::Mutex;

const JOURNAL_SCHEMA_VERSION: u8 = 2;
const JOURNAL_FILE_NAME: &str = "journal-v2.jsonl";
const MAX_JOURNAL_BYTES: u64 = 4 * 1024 * 1024;

pub(super) struct CodeDeepResearchJournal {
    state: Mutex<JournalState>,
}

struct JournalState {
    file: tokio::fs::File,
    sequence: u64,
    terminal: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct JournalRecord {
    schema_version: u8,
    sequence: u64,
    recorded_at: String,
    event: JournalEvent,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CodeDeepResearchJournalSnapshot {
    pub(crate) schema_version: u8,
    pub(crate) run_id: String,
    pub(crate) sequence: u64,
    pub(crate) recorded_at: String,
    pub(crate) lifecycle: DeepResearchLifecycle,
    pub(crate) stage: Option<ResearchStage>,
    pub(crate) publication: Option<PublicationOutcome>,
    pub(crate) quality: Option<DeepResearchPublicationQuality>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct OwnedJournalRecord {
    schema_version: u8,
    sequence: u64,
    recorded_at: String,
    event: JournalEvent,
}

/// Durable projection of an engine event.
///
/// Publication artifacts are intentionally omitted. Engine events contain
/// canonical absolute paths for trusted in-process consumers, but durable Web
/// refresh state needs only the run identity, outcome, and quality.
#[derive(Deserialize, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum JournalEvent {
    RunStarted {
        run_id: String,
        query: String,
    },
    StageStarted {
        run_id: String,
        stage: ResearchStage,
    },
    StageCompleted {
        run_id: String,
        stage: ResearchStage,
    },
    StageDegraded {
        run_id: String,
        stage: ResearchStage,
        reason: String,
    },
    PublicationCompleted {
        run_id: String,
        outcome: PublicationOutcome,
        quality: DeepResearchPublicationQuality,
    },
    RunCompleted {
        run_id: String,
        outcome: PublicationOutcome,
    },
    RunCancelled {
        run_id: String,
    },
    RunFailed {
        run_id: String,
        message: String,
    },
}

impl From<&DeepResearchEvent> for JournalEvent {
    fn from(event: &DeepResearchEvent) -> Self {
        match event {
            DeepResearchEvent::RunStarted { run_id, query } => Self::RunStarted {
                run_id: run_id.clone(),
                query: query.clone(),
            },
            DeepResearchEvent::StageStarted { run_id, stage } => Self::StageStarted {
                run_id: run_id.clone(),
                stage: *stage,
            },
            DeepResearchEvent::StageCompleted { run_id, stage } => Self::StageCompleted {
                run_id: run_id.clone(),
                stage: *stage,
            },
            DeepResearchEvent::StageDegraded {
                run_id,
                stage,
                reason,
            } => Self::StageDegraded {
                run_id: run_id.clone(),
                stage: *stage,
                reason: reason.clone(),
            },
            DeepResearchEvent::PublicationCompleted {
                run_id,
                outcome,
                quality,
                ..
            } => Self::PublicationCompleted {
                run_id: run_id.clone(),
                outcome: *outcome,
                quality: *quality,
            },
            DeepResearchEvent::RunCompleted { run_id, outcome } => Self::RunCompleted {
                run_id: run_id.clone(),
                outcome: *outcome,
            },
            DeepResearchEvent::RunCancelled { run_id } => Self::RunCancelled {
                run_id: run_id.clone(),
            },
            DeepResearchEvent::RunFailed { run_id, message } => Self::RunFailed {
                run_id: run_id.clone(),
                message: message.clone(),
            },
        }
    }
}

impl CodeDeepResearchJournal {
    pub(super) async fn create(workspace: &Path, run_id: &str) -> Result<Self, String> {
        a3s_deep_research::report::validate_deep_research_run_id(run_id)?;
        let workspace = workspace.to_path_buf();
        let run_id = run_id.to_string();
        let file = tokio::task::spawn_blocking(move || prepare_journal_file(&workspace, &run_id))
            .await
            .map_err(|error| format!("prepare DeepResearch journal task failed: {error}"))??;
        Ok(Self {
            state: Mutex::new(JournalState {
                file: tokio::fs::File::from_std(file),
                sequence: 0,
                terminal: false,
            }),
        })
    }

    pub(super) async fn append(&self, event: &DeepResearchEvent) -> Result<(), String> {
        let mut state = self.state.lock().await;
        if state.terminal {
            return if terminal_event(event) {
                Ok(())
            } else {
                Err("DeepResearch journal received an event after terminal settlement".to_string())
            };
        }
        let sequence = state
            .sequence
            .checked_add(1)
            .ok_or_else(|| "DeepResearch journal sequence overflowed".to_string())?;
        let record = JournalRecord {
            schema_version: JOURNAL_SCHEMA_VERSION,
            sequence,
            recorded_at: chrono::Utc::now().to_rfc3339(),
            event: JournalEvent::from(event),
        };
        let mut encoded = serde_json::to_vec(&record)
            .map_err(|error| format!("encode DeepResearch journal event: {error}"))?;
        encoded.push(b'\n');
        state
            .file
            .write_all(&encoded)
            .await
            .map_err(|error| format!("append DeepResearch journal event: {error}"))?;
        state
            .file
            .flush()
            .await
            .map_err(|error| format!("flush DeepResearch journal event: {error}"))?;
        state
            .file
            .sync_data()
            .await
            .map_err(|error| format!("sync DeepResearch journal event: {error}"))?;
        state.sequence = sequence;
        state.terminal = terminal_event(event);
        Ok(())
    }
}

pub(crate) async fn read_code_deep_research_journal(
    workspace: &Path,
    run_id: &str,
) -> Result<Option<CodeDeepResearchJournalSnapshot>, String> {
    a3s_deep_research::report::validate_deep_research_run_id(run_id)?;
    let root = tokio::fs::canonicalize(workspace)
        .await
        .map_err(|error| format!("resolve DeepResearch workspace: {error}"))?;
    let expected_run = root.join(".a3s").join("research").join("runs").join(run_id);
    let run = match tokio::fs::canonicalize(&expected_run).await {
        Ok(run) => run,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("resolve DeepResearch run journal: {error}")),
    };
    let expected_runs = root.join(".a3s").join("research").join("runs");
    let runs = tokio::fs::canonicalize(&expected_runs)
        .await
        .map_err(|error| format!("resolve DeepResearch runs directory: {error}"))?;
    if run.parent() != Some(runs.as_path()) || !run.starts_with(&root) {
        return Err("DeepResearch journal directory escaped the workspace".to_string());
    }
    let path = run.join(JOURNAL_FILE_NAME);
    let metadata = match tokio::fs::symlink_metadata(&path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(format!("inspect DeepResearch journal: {error}")),
    };
    if metadata.file_type().is_symlink()
        || !metadata.is_file()
        || metadata.len() > MAX_JOURNAL_BYTES
    {
        return Err("DeepResearch journal must be a bounded plain file".to_string());
    }
    let text = tokio::fs::read_to_string(&path)
        .await
        .map_err(|error| format!("read DeepResearch journal: {error}"))?;
    let mut snapshot = None;
    let mut expected_sequence = 1_u64;
    for line in text.lines().filter(|line| !line.trim().is_empty()) {
        let record = serde_json::from_str::<OwnedJournalRecord>(line)
            .map_err(|error| format!("decode DeepResearch journal record: {error}"))?;
        if record.schema_version != JOURNAL_SCHEMA_VERSION
            || record.sequence != expected_sequence
            || journal_event_run_id(&record.event) != run_id
        {
            return Err(
                "DeepResearch journal record violated its v2 sequence contract".to_string(),
            );
        }
        let (lifecycle, stage, publication, quality) =
            project_journal_event(snapshot.as_ref(), &record.event);
        snapshot = Some(CodeDeepResearchJournalSnapshot {
            schema_version: record.schema_version,
            run_id: run_id.to_string(),
            sequence: record.sequence,
            recorded_at: record.recorded_at,
            lifecycle,
            stage,
            publication,
            quality,
        });
        expected_sequence = expected_sequence
            .checked_add(1)
            .ok_or_else(|| "DeepResearch journal sequence overflowed".to_string())?;
    }
    Ok(snapshot)
}

fn project_journal_event(
    previous: Option<&CodeDeepResearchJournalSnapshot>,
    event: &JournalEvent,
) -> (
    DeepResearchLifecycle,
    Option<ResearchStage>,
    Option<PublicationOutcome>,
    Option<DeepResearchPublicationQuality>,
) {
    let mut lifecycle = previous
        .map(|snapshot| snapshot.lifecycle)
        .unwrap_or(DeepResearchLifecycle::Running);
    let mut stage = previous.and_then(|snapshot| snapshot.stage);
    let mut publication = previous.and_then(|snapshot| snapshot.publication);
    let mut quality = previous.and_then(|snapshot| snapshot.quality);
    match event {
        JournalEvent::StageStarted { stage: current, .. }
        | JournalEvent::StageCompleted { stage: current, .. }
        | JournalEvent::StageDegraded { stage: current, .. } => stage = Some(*current),
        JournalEvent::PublicationCompleted {
            outcome,
            quality: current_quality,
            ..
        } => {
            publication = Some(*outcome);
            quality = Some(*current_quality);
        }
        JournalEvent::RunCompleted { outcome, .. } => {
            lifecycle = DeepResearchLifecycle::Completed;
            publication = Some(*outcome);
        }
        JournalEvent::RunCancelled { .. } => {
            lifecycle = DeepResearchLifecycle::Cancelled;
        }
        JournalEvent::RunFailed { .. } => {
            lifecycle = DeepResearchLifecycle::Failed;
        }
        JournalEvent::RunStarted { .. } => {
            lifecycle = DeepResearchLifecycle::Running;
        }
    }
    (lifecycle, stage, publication, quality)
}

fn journal_event_run_id(event: &JournalEvent) -> &str {
    match event {
        JournalEvent::RunStarted { run_id, .. }
        | JournalEvent::StageStarted { run_id, .. }
        | JournalEvent::StageCompleted { run_id, .. }
        | JournalEvent::StageDegraded { run_id, .. }
        | JournalEvent::PublicationCompleted { run_id, .. }
        | JournalEvent::RunCompleted { run_id, .. }
        | JournalEvent::RunCancelled { run_id }
        | JournalEvent::RunFailed { run_id, .. } => run_id,
    }
}

fn terminal_event(event: &DeepResearchEvent) -> bool {
    matches!(
        event,
        DeepResearchEvent::RunCompleted { .. }
            | DeepResearchEvent::RunCancelled { .. }
            | DeepResearchEvent::RunFailed { .. }
    )
}

fn prepare_journal_file(workspace: &Path, run_id: &str) -> Result<std::fs::File, String> {
    let root = workspace.canonicalize().map_err(|error| {
        format!(
            "could not resolve DeepResearch workspace {}: {error}",
            workspace.display()
        )
    })?;
    let a3s = root.join(".a3s");
    ensure_plain_directory(&a3s)?;
    let research = a3s.join("research");
    ensure_plain_directory(&research)?;
    let runs = research.join("runs");
    ensure_plain_directory(&runs)?;
    let run = runs.join(run_id);
    ensure_plain_directory(&run)?;

    let canonical_research = research
        .canonicalize()
        .map_err(|error| format!("resolve DeepResearch journal root: {error}"))?;
    let canonical_runs = runs
        .canonicalize()
        .map_err(|error| format!("resolve DeepResearch runs directory: {error}"))?;
    let canonical_run = run
        .canonicalize()
        .map_err(|error| format!("resolve DeepResearch run directory: {error}"))?;
    if canonical_research.parent() != Some(a3s.as_path())
        || canonical_runs.parent() != Some(canonical_research.as_path())
        || canonical_run.parent() != Some(canonical_runs.as_path())
        || !canonical_run.starts_with(&root)
    {
        return Err("DeepResearch journal directory escaped the workspace".to_string());
    }

    let path = canonical_run.join(JOURNAL_FILE_NAME);
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    let mut file = options
        .open(&path)
        .map_err(|error| format!("create DeepResearch journal {}: {error}", path.display()))?;
    file.flush()
        .map_err(|error| format!("initialize DeepResearch journal: {error}"))?;
    file.sync_all()
        .map_err(|error| format!("sync DeepResearch journal: {error}"))?;
    Ok(file)
}

fn ensure_plain_directory(path: &Path) -> Result<(), String> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(format!(
            "refusing symlinked DeepResearch journal directory {}",
            path.display()
        )),
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(format!(
            "DeepResearch journal path is not a directory: {}",
            path.display()
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match std::fs::create_dir(path) {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
                    ensure_plain_directory(path)
                }
                Err(error) => Err(format!("create {}: {error}", path.display())),
            }
        }
        Err(error) => Err(format!("inspect {}: {error}", path.display())),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use serde_json::Value;

    use super::*;

    #[tokio::test]
    async fn journal_is_run_scoped_and_rejects_post_terminal_events() {
        let workspace = tempfile::tempdir().expect("workspace");
        let journal = CodeDeepResearchJournal::create(workspace.path(), "journal-run")
            .await
            .expect("journal");
        journal
            .append(&DeepResearchEvent::RunStarted {
                run_id: "journal-run".to_string(),
                query: "query".to_string(),
            })
            .await
            .expect("started event");
        journal
            .append(&DeepResearchEvent::RunCancelled {
                run_id: "journal-run".to_string(),
            })
            .await
            .expect("terminal event");
        assert!(journal
            .append(&DeepResearchEvent::RunStarted {
                run_id: "journal-run".to_string(),
                query: "late".to_string(),
            })
            .await
            .is_err());

        let path = workspace
            .path()
            .join(".a3s/research/runs/journal-run/journal-v2.jsonl");
        let text = tokio::fs::read_to_string(path).await.expect("read journal");
        let records = text.lines().collect::<Vec<_>>();
        assert_eq!(records.len(), 2);
        assert!(records[0].contains("\"schemaVersion\":2"));
        assert!(records[1].contains("\"type\":\"run_cancelled\""));
    }

    #[tokio::test]
    async fn journal_reader_restores_terminal_projection_without_artifact_paths() {
        let workspace = tempfile::tempdir().expect("workspace");
        let run_id = "projection-run";
        let journal = CodeDeepResearchJournal::create(workspace.path(), run_id)
            .await
            .expect("journal");
        let quality = DeepResearchPublicationQuality {
            source_count: 4,
            relevant_source_count: 3,
            cited_source_count: 2,
            accepted_claim_count: 6,
            accepted_basis_edge_count: 2,
            analytical_claim_count: 1,
            cross_source_synthesis_count: 1,
            ..DeepResearchPublicationQuality::default()
        };
        let absolute_artifact_dir = workspace
            .path()
            .join(".a3s/research/artifacts/projection-run");
        let events = [
            DeepResearchEvent::RunStarted {
                run_id: run_id.to_string(),
                query: "Assess the migration".to_string(),
            },
            DeepResearchEvent::StageStarted {
                run_id: run_id.to_string(),
                stage: ResearchStage::ReportGeneration,
            },
            DeepResearchEvent::StageCompleted {
                run_id: run_id.to_string(),
                stage: ResearchStage::FinalPublication,
            },
            DeepResearchEvent::PublicationCompleted {
                run_id: run_id.to_string(),
                outcome: PublicationOutcome::Qualified,
                quality,
                artifacts: a3s_deep_research::report::ResearchReportArtifacts {
                    markdown: absolute_artifact_dir.join("report.md"),
                    html: absolute_artifact_dir.join("index.html"),
                },
            },
            DeepResearchEvent::RunCompleted {
                run_id: run_id.to_string(),
                outcome: PublicationOutcome::Qualified,
            },
        ];
        for event in &events {
            journal.append(event).await.expect("append event");
        }
        drop(journal);

        let snapshot = read_code_deep_research_journal(workspace.path(), run_id)
            .await
            .expect("read journal")
            .expect("snapshot");
        assert_eq!(snapshot.schema_version, JOURNAL_SCHEMA_VERSION);
        assert_eq!(snapshot.run_id, run_id);
        assert_eq!(snapshot.sequence, 5);
        assert_eq!(snapshot.lifecycle, DeepResearchLifecycle::Completed);
        assert_eq!(snapshot.stage, Some(ResearchStage::FinalPublication));
        assert_eq!(snapshot.publication, Some(PublicationOutcome::Qualified));
        assert_eq!(snapshot.quality, Some(quality));

        let path = journal_path(workspace.path(), run_id);
        let text = tokio::fs::read_to_string(path).await.expect("read journal");
        let records = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("journal record"))
            .collect::<Vec<_>>();
        assert_eq!(
            records
                .iter()
                .map(|record| record["sequence"].as_u64())
                .collect::<Vec<_>>(),
            vec![Some(1), Some(2), Some(3), Some(4), Some(5)]
        );
        assert!(records[3]["event"].get("artifacts").is_none());
        assert_eq!(
            records[3]["event"]["quality"]["cross_source_synthesis_count"],
            1
        );
        assert!(!text.contains(&workspace.path().display().to_string()));
    }

    #[tokio::test]
    async fn journal_reader_rejects_a_sequence_gap() {
        let workspace = tempfile::tempdir().expect("workspace");
        let run_id = "sequence-run";
        let journal = CodeDeepResearchJournal::create(workspace.path(), run_id)
            .await
            .expect("journal");
        journal
            .append(&DeepResearchEvent::RunStarted {
                run_id: run_id.to_string(),
                query: "query".to_string(),
            })
            .await
            .expect("started event");
        journal
            .append(&DeepResearchEvent::StageStarted {
                run_id: run_id.to_string(),
                stage: ResearchStage::Planning,
            })
            .await
            .expect("stage event");
        drop(journal);

        let path = journal_path(workspace.path(), run_id);
        let text = tokio::fs::read_to_string(&path)
            .await
            .expect("read journal");
        let mut records = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("journal record"))
            .collect::<Vec<_>>();
        records[1]["sequence"] = Value::from(3_u64);
        tokio::fs::write(&path, encode_records(&records))
            .await
            .expect("rewrite journal");

        let error = read_code_deep_research_journal(workspace.path(), run_id)
            .await
            .expect_err("sequence gap must fail closed");
        assert!(error.contains("v2 sequence contract"));
    }

    #[tokio::test]
    async fn journal_reader_rejects_an_event_from_another_run() {
        let workspace = tempfile::tempdir().expect("workspace");
        let run_id = "identity-run";
        let journal = CodeDeepResearchJournal::create(workspace.path(), run_id)
            .await
            .expect("journal");
        journal
            .append(&DeepResearchEvent::RunStarted {
                run_id: run_id.to_string(),
                query: "query".to_string(),
            })
            .await
            .expect("started event");
        journal
            .append(&DeepResearchEvent::StageStarted {
                run_id: run_id.to_string(),
                stage: ResearchStage::Planning,
            })
            .await
            .expect("stage event");
        drop(journal);

        let path = journal_path(workspace.path(), run_id);
        let text = tokio::fs::read_to_string(&path)
            .await
            .expect("read journal");
        let mut records = text
            .lines()
            .map(|line| serde_json::from_str::<Value>(line).expect("journal record"))
            .collect::<Vec<_>>();
        records[1]["event"]["run_id"] = Value::from("different-run");
        tokio::fs::write(&path, encode_records(&records))
            .await
            .expect("rewrite journal");

        let error = read_code_deep_research_journal(workspace.path(), run_id)
            .await
            .expect_err("foreign event must fail closed");
        assert!(error.contains("v2 sequence contract"));
    }

    fn journal_path(workspace: &Path, run_id: &str) -> PathBuf {
        workspace
            .join(".a3s")
            .join("research")
            .join("runs")
            .join(run_id)
            .join(JOURNAL_FILE_NAME)
    }

    fn encode_records(records: &[Value]) -> String {
        let mut encoded = records
            .iter()
            .map(Value::to_string)
            .collect::<Vec<_>>()
            .join("\n");
        encoded.push('\n');
        encoded
    }
}
