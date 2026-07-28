//! Typed, read-only context hygiene inspection for `/checkup`.
//!
//! Potentially blocking filesystem probes run outside Tokio's async workers.
//! Only bounded, secret-free summaries cross into the planning prompt; raw
//! Skill contents, MCP errors, and credentials do not.

use super::*;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fs::File;
use std::io::Read;
use std::path::{Component, Path, PathBuf};

use a3s_code_core::mcp::McpServerStatus;

use super::usage::{inspect_skill_history, SkillUsageAudit, SkillUsageSubject};

const LARGE_SKILL_BYTES: u64 = 128 * 1024;
const LARGE_INSTRUCTION_BYTES: u64 = 256 * 1024;
const MCP_STATUS_TIMEOUT: Duration = Duration::from_secs(2);

pub(in crate::tui) struct CheckupPreflight {
    skills: SkillAudit,
    skill_usage: SkillUsageAudit,
    instructions: InstructionAudit,
    mcp: McpAudit,
}

impl CheckupPreflight {
    pub(super) fn render(&self) -> String {
        [
            self.skills.render(),
            self.skill_usage.render(),
            self.instructions.render(),
            self.mcp.render(),
        ]
        .join("\n")
    }
}

struct CheckupPreflightInput {
    workspace: PathBuf,
    configured_skill_dir: PathBuf,
    indexed_instruction_files: Vec<String>,
    disabled_skills: HashSet<String>,
    configured_mcp: usize,
}

impl CheckupPreflightInput {
    fn from_app(app: &App) -> Self {
        Self {
            workspace: PathBuf::from(&app.cwd),
            configured_skill_dir: app.asset_directories.skill.clone(),
            indexed_instruction_files: app
                .files
                .iter()
                .filter(|path| {
                    Path::new(path)
                        .file_name()
                        .is_some_and(|name| name == "AGENTS.md")
                })
                .cloned()
                .collect(),
            disabled_skills: app.disabled_skills.clone(),
            configured_mcp: app.code_config.mcp_servers.len(),
        }
    }

    fn inspect(self) -> HostPreflight {
        let skill_dirs = agent_skill_dirs_with_configured(
            &self.workspace.to_string_lossy(),
            &self.configured_skill_dir,
        );
        let skills = SkillAudit::inspect(&skill_dirs, &self.disabled_skills);
        let instructions =
            InstructionAudit::inspect(&self.workspace, &self.indexed_instruction_files);
        HostPreflight {
            skills,
            instructions,
            configured_mcp: self.configured_mcp,
        }
    }
}

struct HostPreflight {
    skills: SkillAudit,
    instructions: InstructionAudit,
    configured_mcp: usize,
}

pub(super) fn command(app: &App, status_entry: TranscriptEntryId) -> Cmd<Msg> {
    let input = CheckupPreflightInput::from_app(app);
    let session = Arc::clone(&app.session);
    let store = Arc::clone(&app.store);
    let current_session_id = app.session_id.clone();
    cmd::cmd(move || async move {
        let host = tokio::task::spawn_blocking(move || input.inspect());
        let mcp = tokio::time::timeout(MCP_STATUS_TIMEOUT, session.mcp_status());
        let usage = inspect_skill_history(store, current_session_id);
        let (host, mcp, usage) = tokio::join!(host, mcp, usage);
        let result = host
            .map_err(|error| format!("host inspection task failed: {error}"))
            .map(|host| CheckupPreflight {
                skill_usage: SkillUsageAudit::classify(
                    &host.skills.subjects,
                    usage,
                    chrono::Utc::now().timestamp(),
                ),
                skills: host.skills,
                instructions: host.instructions,
                mcp: match mcp {
                    Ok(status) => McpAudit::available(host.configured_mcp, status),
                    Err(_) => McpAudit::timed_out(host.configured_mcp),
                },
            });
        Msg::CheckupPreflightCompleted {
            status_entry,
            result,
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct SkillAudit {
    directories: usize,
    files: usize,
    total_bytes: u64,
    duplicate_names: usize,
    large_files: usize,
    metadata_failures: usize,
    subjects: Vec<SkillUsageSubject>,
}

impl SkillAudit {
    fn inspect(directories: &[PathBuf], disabled_skills: &HashSet<String>) -> Self {
        let mut audit = Self {
            directories: directories.len(),
            files: 0,
            total_bytes: 0,
            duplicate_names: 0,
            large_files: 0,
            metadata_failures: 0,
            subjects: Vec::new(),
        };
        let mut names = BTreeMap::<String, usize>::new();
        let mut subjects = BTreeMap::<String, SkillUsageSubject>::new();
        for directory in directories {
            let Ok(entries) = std::fs::read_dir(directory) else {
                audit.metadata_failures += 1;
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let (name, skill_file) = if path.is_dir() {
                    let skill = path.join("SKILL.md");
                    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    (name.to_string(), skill)
                } else if path.extension().and_then(|value| value.to_str()) == Some("md") {
                    let Some(name) = path.file_stem().and_then(|value| value.to_str()) else {
                        continue;
                    };
                    (name.to_string(), path)
                } else {
                    continue;
                };
                if !skill_file.is_file() {
                    continue;
                }
                let canonical_name = bounded_skill_name(&skill_file).unwrap_or(name);
                audit.files += 1;
                *names.entry(canonical_name.clone()).or_default() += 1;
                match skill_file.metadata() {
                    Ok(metadata) => {
                        audit.total_bytes = audit.total_bytes.saturating_add(metadata.len());
                        if metadata.len() > LARGE_SKILL_BYTES {
                            audit.large_files += 1;
                        }
                        let modified_at = metadata
                            .modified()
                            .ok()
                            .and_then(|value| value.duration_since(std::time::UNIX_EPOCH).ok())
                            .and_then(|value| i64::try_from(value.as_secs()).ok());
                        match subjects.entry(canonical_name.clone()) {
                            std::collections::btree_map::Entry::Vacant(entry) => {
                                entry.insert(SkillUsageSubject::new(
                                    canonical_name.clone(),
                                    metadata.len(),
                                    modified_at,
                                    !disabled_skills.contains(&canonical_name),
                                ));
                            }
                            std::collections::btree_map::Entry::Occupied(mut entry) => {
                                entry.get_mut().merge_copy(metadata.len(), modified_at);
                            }
                        }
                    }
                    Err(_) => audit.metadata_failures += 1,
                }
            }
        }
        audit.duplicate_names = names.values().map(|count| count.saturating_sub(1)).sum();
        audit.subjects = subjects.into_values().collect();
        audit
    }

    fn render(&self) -> String {
        format!(
            "- skill/plugin context: {} file(s) across {} source dir(s), {}; {} duplicate name(s), {} file(s) over 128 KiB, {} metadata failure(s)",
            self.files,
            self.directories,
            human_bytes(self.total_bytes),
            self.duplicate_names,
            self.large_files,
            self.metadata_failures
        )
    }
}

fn bounded_skill_name(path: &Path) -> Option<String> {
    const MAX_FRONTMATTER_BYTES: u64 = 16 * 1024;
    let mut source = String::new();
    File::open(path)
        .ok()?
        .take(MAX_FRONTMATTER_BYTES)
        .read_to_string(&mut source)
        .ok()?;
    let rest = source.trim_start().strip_prefix("---")?;
    let end = rest.find("\n---")?;
    rest[..end].lines().find_map(|line| {
        let value = line.strip_prefix("name:")?.trim().trim_matches(['"', '\'']);
        if value.is_empty() || value.chars().count() > 120 || value.chars().any(char::is_control) {
            None
        } else {
            Some(value.to_string())
        }
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct InstructionAudit {
    files: usize,
    total_bytes: u64,
    large_files: usize,
    metadata_failures: usize,
}

impl InstructionAudit {
    fn inspect(workspace: &Path, indexed_files: &[String]) -> Self {
        let mut audit = Self {
            files: indexed_files.len(),
            total_bytes: 0,
            large_files: 0,
            metadata_failures: 0,
        };
        for indexed in indexed_files {
            let path = Path::new(indexed);
            if path
                .components()
                .any(|component| component == Component::ParentDir)
            {
                audit.metadata_failures += 1;
                continue;
            }
            let path = if path.is_absolute() {
                if !path.starts_with(workspace) {
                    audit.metadata_failures += 1;
                    continue;
                }
                path.to_path_buf()
            } else {
                workspace.join(path)
            };
            match path.metadata() {
                Ok(metadata) => {
                    audit.total_bytes = audit.total_bytes.saturating_add(metadata.len());
                    if metadata.len() > LARGE_INSTRUCTION_BYTES {
                        audit.large_files += 1;
                    }
                }
                Err(_) => audit.metadata_failures += 1,
            }
        }
        audit
    }

    fn render(&self) -> String {
        format!(
            "- workspace instructions: {} indexed AGENTS.md file(s), {}; {} file(s) over 256 KiB, {} metadata failure(s)",
            self.files,
            human_bytes(self.total_bytes),
            self.large_files,
            self.metadata_failures
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct McpAudit {
    available: bool,
    configured: usize,
    registered: usize,
    enabled: usize,
    connected: usize,
    tools: usize,
    errors: usize,
}

impl McpAudit {
    fn available(configured: usize, status: HashMap<String, McpServerStatus>) -> Self {
        Self {
            available: true,
            configured,
            registered: status.len(),
            enabled: status.values().filter(|entry| entry.enabled).count(),
            connected: status.values().filter(|entry| entry.connected).count(),
            tools: status.values().map(|entry| entry.tool_count).sum(),
            errors: status
                .values()
                .filter(|entry| entry.error.is_some())
                .count(),
        }
    }

    fn timed_out(configured: usize) -> Self {
        Self {
            available: false,
            configured,
            registered: 0,
            enabled: 0,
            connected: 0,
            tools: 0,
            errors: 0,
        }
    }

    fn render(&self) -> String {
        if !self.available {
            return format!(
                "- MCP runtime: {} configured; in-memory status timed out (servers were not started)",
                self.configured
            );
        }
        format!(
            "- MCP runtime: {} configured, {} registered, {} enabled, {} connected, {} tool(s), {} error state(s); error text withheld",
            self.configured,
            self.registered,
            self.enabled,
            self.connected,
            self.tools,
            self.errors
        )
    }
}

fn human_bytes(bytes: u64) -> String {
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    if bytes < 1024 * 1024 {
        return format!("{:.1} KiB", bytes as f64 / 1024.0);
    }
    format!("{:.1} MiB", bytes as f64 / (1024.0 * 1024.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn skill_audit_counts_duplicate_names_and_context_bytes() {
        let temp = tempfile::tempdir().unwrap();
        let first = temp.path().join("first");
        let second = temp.path().join("second");
        for root in [&first, &second] {
            let skill = root.join("shared/SKILL.md");
            std::fs::create_dir_all(skill.parent().unwrap()).unwrap();
            std::fs::write(skill, "---\nname: shared\n---\nbody\n").unwrap();
        }

        let audit = SkillAudit::inspect(&[first, second], &HashSet::new());

        assert_eq!(audit.files, 2);
        assert_eq!(audit.duplicate_names, 1);
        assert!(audit.total_bytes > 0);
    }

    #[test]
    fn mcp_audit_exposes_counts_but_not_error_text() {
        let status = HashMap::from([(
            "private".to_string(),
            McpServerStatus {
                name: "private".to_string(),
                connected: false,
                enabled: true,
                tool_count: 0,
                error: Some("Bearer top-secret-token".to_string()),
            },
        )]);

        let rendered = McpAudit::available(1, status).render();

        assert!(rendered.contains("1 error state"));
        assert!(!rendered.contains("top-secret-token"));
    }
}
