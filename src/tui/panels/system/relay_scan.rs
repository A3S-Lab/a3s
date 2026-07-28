//! Session discovery for the `/relay` panel.

use super::super::*;
use crate::account_providers::AccountProvider;
use a3s_code_core::store::{SessionData, SessionStore};
use std::collections::{HashMap, HashSet};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const PER_AGENT: usize = 64;
const TRANSCRIPT_TAIL_BYTES: u64 = 128 * 1024;
const TRANSCRIPT_HEAD_BYTES: usize = 96 * 1024;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RelayAgent {
    A3sCode,
    ClaudeCode,
    Codex,
    WorkBuddy,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RelaySessionStatus {
    Saved,
    Paused,
    Completed,
    Error,
    External,
}

impl RelaySessionStatus {
    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Saved => "saved",
            Self::Paused => "paused",
            Self::Completed => "completed",
            Self::Error => "error",
            Self::External => "transcript",
        }
    }
}

impl RelayAgent {
    pub(crate) const ALL: [Self; 4] = [
        Self::A3sCode,
        Self::ClaudeCode,
        Self::Codex,
        Self::WorkBuddy,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::A3sCode => "A3S Code",
            Self::ClaudeCode => "Claude Code",
            Self::Codex => "Codex",
            Self::WorkBuddy => "WorkBuddy",
        }
    }
}

/// Stable identity used by the relay dashboard across scans and reordering.
///
/// Native sessions already expose a durable session id. External transcripts
/// use their source path so appends can change labels and timestamps without
/// turning the same conversation into a different dashboard row.
#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub(crate) enum RelaySessionIdentity {
    Native(String),
    Transcript { agent: RelayAgent, path: PathBuf },
}

#[derive(Clone, Debug)]
pub(crate) struct RelaySession {
    pub(crate) identity: RelaySessionIdentity,
    pub(crate) agent: RelayAgent,
    pub(crate) native_id: Option<String>,
    pub(crate) seed: Option<String>,
    pub(crate) label: String,
    pub(crate) modified: SystemTime,
    pub(crate) persisted_model: Option<String>,
    pub(crate) status: RelaySessionStatus,
    pub(crate) active_runs: usize,
    pub(crate) active_subagents: usize,
}

#[derive(Clone, Debug, Default)]
struct RelayHistoryRoots {
    claude: Option<PathBuf>,
    codex: Option<PathBuf>,
    workbuddy: Option<PathBuf>,
}

impl RelayHistoryRoots {
    fn discover() -> Self {
        Self {
            claude: AccountProvider::Claude.history_root(),
            codex: AccountProvider::Codex.history_root(),
            workbuddy: AccountProvider::CodeBuddy.history_root(),
        }
    }
}

pub(super) async fn scan_relay_sessions(
    store: Arc<dyn SessionStore>,
    workspace: PathBuf,
    current_session: Arc<AgentSession>,
) -> Result<Vec<RelaySession>, String> {
    let mut sessions = scan_native_sessions(store).await?;
    let current_session_id = current_session.id().to_string();
    let current_history = current_session.history();
    let live_subagents = current_session.pending_subagent_tasks().await.len();
    if let Some(current) = sessions
        .iter_mut()
        .find(|session| session.native_id.as_deref() == Some(current_session_id.as_str()))
    {
        current.active_subagents = live_subagents;
    } else {
        let label = last_user_message_in_history(&current_history)
            .map(|message| truncate(&message, 72))
            .unwrap_or_else(|| format!("session {current_session_id}"));
        sessions.push(RelaySession {
            identity: RelaySessionIdentity::Native(current_session_id.clone()),
            agent: RelayAgent::A3sCode,
            native_id: Some(current_session_id.clone()),
            seed: None,
            label,
            modified: SystemTime::now(),
            persisted_model: None,
            status: RelaySessionStatus::Saved,
            active_runs: 0,
            active_subagents: live_subagents,
        });
    }
    let roots = RelayHistoryRoots::discover();
    let mut foreign =
        tokio::task::spawn_blocking(move || scan_foreign_sessions(&workspace, &roots))
            .await
            .map_err(|error| format!("relay transcript scan failed: {error}"))?;
    sessions.append(&mut foreign);
    Ok(finalize_sessions(sessions, &current_session_id))
}

async fn scan_native_sessions(store: Arc<dyn SessionStore>) -> Result<Vec<RelaySession>, String> {
    let ids = store
        .list()
        .await
        .map_err(|error| format!("could not list A3S Code sessions: {error}"))?;
    let mut sessions = Vec::new();
    for id in ids {
        let snapshot = match store.load_snapshot(&id).await {
            Ok(Some(snapshot)) => snapshot,
            Ok(None) => continue,
            Err(error) => {
                tracing::warn!(%error, %id, "skipping unreadable relay session");
                continue;
            }
        };
        let active_runs = snapshot
            .run_records
            .iter()
            .filter(|record| {
                matches!(
                    record.snapshot.status,
                    a3s_code_core::RunStatus::Created
                        | a3s_code_core::RunStatus::Planning
                        | a3s_code_core::RunStatus::Executing
                        | a3s_code_core::RunStatus::Verifying
                )
            })
            .count();
        let active_subagents = snapshot
            .subagent_tasks
            .iter()
            .filter(|task| task.status == a3s_code_core::SubagentStatus::Running)
            .count();
        let data = snapshot.session;
        let label = last_user_message(&data)
            .map(|message| truncate(&message, 72))
            .unwrap_or_else(|| format!("session {id}"));
        let modified = u64::try_from(data.updated_at)
            .ok()
            .and_then(|seconds| UNIX_EPOCH.checked_add(Duration::from_secs(seconds)))
            .unwrap_or(UNIX_EPOCH);
        sessions.push(RelaySession {
            identity: RelaySessionIdentity::Native(id.clone()),
            agent: RelayAgent::A3sCode,
            native_id: Some(id),
            seed: None,
            label,
            modified,
            persisted_model: app_launch::persisted_model_from_session(&data),
            status: relay_session_status(data.state),
            active_runs,
            active_subagents,
        });
    }
    Ok(sessions)
}

fn last_user_message_in_history(history: &[Message]) -> Option<String> {
    history.iter().rev().find_map(|message| {
        if message.role != "user" {
            return None;
        }
        let text = message.text();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    })
}

fn relay_session_status(state: a3s_code_core::store::SessionState) -> RelaySessionStatus {
    match state {
        a3s_code_core::store::SessionState::Paused => RelaySessionStatus::Paused,
        a3s_code_core::store::SessionState::Completed => RelaySessionStatus::Completed,
        a3s_code_core::store::SessionState::Error => RelaySessionStatus::Error,
        a3s_code_core::store::SessionState::Unknown
        | a3s_code_core::store::SessionState::Active => RelaySessionStatus::Saved,
    }
}

fn last_user_message(session: &SessionData) -> Option<String> {
    session.messages.iter().rev().find_map(|message| {
        if message.role != "user" {
            return None;
        }
        let text = message.text();
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_string())
    })
}

fn scan_foreign_sessions(workspace: &Path, roots: &RelayHistoryRoots) -> Vec<RelaySession> {
    let directories = workspace_ancestors(workspace);
    let mut sessions = Vec::new();
    let mut seen_files = HashSet::new();

    if let Some(root) = &roots.claude {
        for directory in &directories {
            for key in project_keys(directory, true) {
                collect_jsonl(
                    &root.join("projects").join(key),
                    RelayAgent::ClaudeCode,
                    &mut sessions,
                    &mut seen_files,
                    |_| true,
                );
            }
        }
    }

    if let Some(root) = &roots.workbuddy {
        for directory in &directories {
            for key in project_keys(directory, false) {
                collect_jsonl(
                    &root.join("projects").join(key),
                    RelayAgent::WorkBuddy,
                    &mut sessions,
                    &mut seen_files,
                    |_| true,
                );
            }
        }
    }

    if let Some(root) = &roots.codex {
        collect_jsonl(
            &root.join("sessions"),
            RelayAgent::Codex,
            &mut sessions,
            &mut seen_files,
            |path| transcript_matches_workspace(path, &directories),
        );
    }

    sessions
}

fn workspace_ancestors(workspace: &Path) -> Vec<PathBuf> {
    workspace
        .ancestors()
        .take(6)
        .map(Path::to_path_buf)
        .collect()
}

fn encoded_project_path(path: &Path) -> String {
    path.to_string_lossy()
        .trim_start_matches(['/', '\\'])
        .replace(['/', '\\'], "-")
}

fn project_keys(path: &Path, prefer_leading_dash: bool) -> Vec<String> {
    let encoded = encoded_project_path(path);
    let leading = format!("-{encoded}");
    if prefer_leading_dash {
        vec![leading, encoded]
    } else {
        vec![encoded, leading]
    }
}

fn transcript_matches_workspace(path: &Path, directories: &[PathBuf]) -> bool {
    let Some(cwd) = first_jsonl_cwd(path) else {
        // Older Codex transcripts did not persist cwd. Preserve the original
        // `/relay` behavior for those files instead of making them disappear.
        return true;
    };
    directories.iter().any(|directory| directory == &cwd)
}

fn first_jsonl_cwd(path: &Path) -> Option<PathBuf> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = vec![0; TRANSCRIPT_HEAD_BYTES];
    let read = file.read(&mut bytes).ok()?;
    String::from_utf8_lossy(&bytes[..read])
        .lines()
        .filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        .find_map(|value| value.get("cwd")?.as_str().map(PathBuf::from))
}

fn collect_jsonl<F>(
    directory: &Path,
    agent: RelayAgent,
    sessions: &mut Vec<RelaySession>,
    seen_files: &mut HashSet<PathBuf>,
    include: F,
) where
    F: Fn(&Path) -> bool,
{
    let remaining = PER_AGENT.saturating_sub(
        sessions
            .iter()
            .filter(|session| session.agent == agent)
            .count(),
    );
    if remaining == 0 {
        return;
    }
    let mut paths = Vec::new();
    gather_jsonl(directory, 0, 6, &mut paths);
    paths.sort_by_key(|(_, modified)| std::cmp::Reverse(*modified));

    for (path, modified) in paths
        .into_iter()
        .filter(|(path, _)| include(path))
        .take(remaining)
    {
        if !seen_files.insert(path.clone()) {
            continue;
        }
        let seed = last_user_msg_jsonl(&path).or_else(|| first_user_msg_jsonl(&path));
        let label = seed
            .as_deref()
            .map(|message| truncate(message, 72))
            .unwrap_or_else(|| jsonl_session_name(&path));
        sessions.push(RelaySession {
            identity: RelaySessionIdentity::Transcript {
                agent,
                path: path.clone(),
            },
            agent,
            native_id: None,
            seed,
            label,
            modified,
            persisted_model: None,
            status: RelaySessionStatus::External,
            active_runs: 0,
            active_subagents: 0,
        });
    }
}

fn gather_jsonl(
    directory: &Path,
    depth: usize,
    max_depth: usize,
    paths: &mut Vec<(PathBuf, SystemTime)>,
) {
    if depth > max_depth {
        return;
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            gather_jsonl(&path, depth + 1, max_depth, paths);
        } else if path.extension().and_then(|extension| extension.to_str()) == Some("jsonl") {
            let modified = entry
                .metadata()
                .and_then(|metadata| metadata.modified())
                .unwrap_or(UNIX_EPOCH);
            paths.push((path, modified));
        }
    }
}

fn parse_user_line(line: &str) -> Option<String> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let role = value
        .get("message")
        .and_then(|message| message.get("role"))
        .or_else(|| value.get("payload").and_then(|payload| payload.get("role")))
        .or_else(|| value.get("role"))
        .and_then(serde_json::Value::as_str);
    if role != Some("user") {
        return None;
    }
    let content = value
        .get("message")
        .and_then(|message| message.get("content"))
        .or_else(|| {
            value
                .get("payload")
                .and_then(|payload| payload.get("content"))
        })
        .or_else(|| value.get("content"))?;
    let text = match content {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Array(parts) => parts
            .iter()
            .filter_map(|part| part.get("text").and_then(serde_json::Value::as_str))
            .collect::<Vec<_>>()
            .join(" "),
        _ => return None,
    };
    let text = text.trim();
    if text.is_empty() || text.starts_with('<') {
        return None;
    }
    Some(text.to_string())
}

fn last_user_msg_jsonl(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let length = file.metadata().ok()?.len();
    let start = length.saturating_sub(TRANSCRIPT_TAIL_BYTES);
    file.seek(SeekFrom::Start(start)).ok()?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes).ok()?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines().collect::<Vec<_>>();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    lines.iter().rev().find_map(|line| parse_user_line(line))
}

fn first_user_msg_jsonl(path: &Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut bytes = vec![0; TRANSCRIPT_HEAD_BYTES];
    let read = file.read(&mut bytes).ok()?;
    String::from_utf8_lossy(&bytes[..read])
        .lines()
        .find_map(parse_user_line)
}

fn jsonl_session_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(|stem| {
            stem.strip_prefix("rollout-")
                .unwrap_or(stem)
                .chars()
                .take(19)
                .collect::<String>()
                .replace('T', " ")
        })
        .unwrap_or_else(|| "session".to_string())
}

fn finalize_sessions(
    mut sessions: Vec<RelaySession>,
    current_session_id: &str,
) -> Vec<RelaySession> {
    sessions.sort_by(|left, right| {
        let left_current = left.native_id.as_deref() == Some(current_session_id);
        let right_current = right.native_id.as_deref() == Some(current_session_id);
        right_current
            .cmp(&left_current)
            .then_with(|| right.modified.cmp(&left.modified))
    });
    let mut kept = HashMap::<RelayAgent, usize>::new();
    sessions.retain(|session| {
        let count = kept.entry(session.agent).or_default();
        *count += 1;
        *count <= PER_AGENT
    });
    sessions
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_workbuddy_input_text_messages() {
        let line = serde_json::json!({
            "type": "message",
            "role": "user",
            "cwd": "/workspace",
            "content": [{"type": "input_text", "text": "continue the WorkBuddy task"}]
        })
        .to_string();

        assert_eq!(
            parse_user_line(&line).as_deref(),
            Some("continue the WorkBuddy task")
        );
    }

    #[test]
    fn workbuddy_project_key_matches_its_local_directory_layout() {
        assert_eq!(
            project_keys(Path::new("/Users/alice/code/a3s"), false),
            ["Users-alice-code-a3s", "-Users-alice-code-a3s"]
        );
    }

    #[test]
    fn workbuddy_project_transcripts_are_discovered_for_the_workspace() {
        let root = tempfile::tempdir().unwrap();
        let workspace = root.path().join("workspace/project");
        std::fs::create_dir_all(&workspace).unwrap();
        let workbuddy = root.path().join("workbuddy");
        let project = workbuddy
            .join("projects")
            .join(encoded_project_path(&workspace));
        std::fs::create_dir_all(&project).unwrap();
        std::fs::write(
            project.join("session.jsonl"),
            format!(
                "{}\n",
                serde_json::json!({
                    "type": "message",
                    "role": "user",
                    "cwd": workspace,
                    "content": [{"type": "input_text", "text": "ship WorkBuddy relay support"}]
                })
            ),
        )
        .unwrap();

        let sessions = scan_foreign_sessions(
            &workspace,
            &RelayHistoryRoots {
                workbuddy: Some(workbuddy),
                ..RelayHistoryRoots::default()
            },
        );

        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].agent, RelayAgent::WorkBuddy);
        assert_eq!(
            sessions[0].seed.as_deref(),
            Some("ship WorkBuddy relay support")
        );
    }

    #[test]
    fn current_native_session_is_pinned_ahead_of_newer_saved_sessions() {
        let sessions = vec![
            RelaySession {
                identity: RelaySessionIdentity::Native("newer".to_string()),
                agent: RelayAgent::A3sCode,
                native_id: Some("newer".to_string()),
                seed: None,
                label: "newer".to_string(),
                modified: UNIX_EPOCH + Duration::from_secs(20),
                persisted_model: None,
                status: RelaySessionStatus::Saved,
                active_runs: 0,
                active_subagents: 0,
            },
            RelaySession {
                identity: RelaySessionIdentity::Native("current".to_string()),
                agent: RelayAgent::A3sCode,
                native_id: Some("current".to_string()),
                seed: None,
                label: "current".to_string(),
                modified: UNIX_EPOCH + Duration::from_secs(10),
                persisted_model: None,
                status: RelaySessionStatus::Saved,
                active_runs: 0,
                active_subagents: 1,
            },
        ];

        let sessions = finalize_sessions(sessions, "current");

        assert_eq!(sessions[0].native_id.as_deref(), Some("current"));
        assert_eq!(sessions[0].active_subagents, 1);
    }

    #[test]
    fn session_catalog_is_larger_but_still_bounded_per_agent() {
        let sessions = (0..80)
            .map(|index| {
                let id = if index == 0 {
                    "current".to_string()
                } else {
                    format!("session-{index}")
                };
                RelaySession {
                    identity: RelaySessionIdentity::Native(id.clone()),
                    agent: RelayAgent::A3sCode,
                    native_id: Some(id),
                    seed: None,
                    label: format!("task {index}"),
                    modified: UNIX_EPOCH + Duration::from_secs(index),
                    persisted_model: None,
                    status: RelaySessionStatus::Saved,
                    active_runs: 0,
                    active_subagents: 0,
                }
            })
            .collect();

        let sessions = finalize_sessions(sessions, "current");

        assert_eq!(sessions.len(), PER_AGENT);
        assert_eq!(sessions[0].native_id.as_deref(), Some("current"));
        assert!(
            sessions.len() > 8,
            "search needs a useful catalog beyond the old eight-row cap"
        );
    }

    #[test]
    fn transcript_collection_respects_the_remaining_agent_budget() {
        let root = tempfile::tempdir().unwrap();
        let line = serde_json::json!({
            "role": "user",
            "content": "continue the bounded transcript"
        })
        .to_string();
        for index in 0..3 {
            std::fs::write(
                root.path().join(format!("{index}.jsonl")),
                format!("{line}\n"),
            )
            .unwrap();
        }
        let mut sessions = (0..(PER_AGENT - 1))
            .map(|index| RelaySession {
                identity: RelaySessionIdentity::Transcript {
                    agent: RelayAgent::Codex,
                    path: PathBuf::from(format!("/existing/{index}.jsonl")),
                },
                agent: RelayAgent::Codex,
                native_id: None,
                seed: Some(format!("existing task {index}")),
                label: format!("existing task {index}"),
                modified: UNIX_EPOCH,
                persisted_model: None,
                status: RelaySessionStatus::External,
                active_runs: 0,
                active_subagents: 0,
            })
            .collect::<Vec<_>>();
        let mut seen_files = HashSet::new();

        collect_jsonl(
            root.path(),
            RelayAgent::Codex,
            &mut sessions,
            &mut seen_files,
            |_| true,
        );

        assert_eq!(sessions.len(), PER_AGENT);
        assert_eq!(seen_files.len(), 1);
    }
}
