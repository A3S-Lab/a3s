//! Process model + collector for `a3s top`. Kept independent from the TUI
//! layer so the same `ProcessRow` snapshot also feeds `--json` and remote
//! consumers.

use std::sync::{Mutex, OnceLock};

use a3s_tui::style::Color;
use sysinfo::{ProcessesToUpdate, System};

use super::{ACCENT, GREEN, ORANGE, RED, YELLOW};

/// A coding agent recognised from a process command line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AgentKind {
    A3sCode,
    ClaudeCode,
    Codex,
    Cursor,
    Gemini,
    WorkBuddy,
}

impl AgentKind {
    pub(crate) const ALL: [AgentKind; 6] = [
        AgentKind::A3sCode,
        AgentKind::ClaudeCode,
        AgentKind::Codex,
        AgentKind::Cursor,
        AgentKind::Gemini,
        AgentKind::WorkBuddy,
    ];

    pub(crate) fn label(self) -> &'static str {
        match self {
            AgentKind::A3sCode => "a3s-code",
            AgentKind::ClaudeCode => "claude",
            AgentKind::Codex => "codex",
            AgentKind::Cursor => "cursor",
            AgentKind::Gemini => "gemini",
            AgentKind::WorkBuddy => "workbuddy",
        }
    }

    pub(crate) fn color(self) -> Color {
        match self {
            AgentKind::A3sCode => ACCENT,
            AgentKind::ClaudeCode => ORANGE,
            AgentKind::Codex => Color::Rgb(16, 163, 127),
            AgentKind::Cursor => Color::Rgb(180, 182, 200),
            AgentKind::Gemini => Color::Rgb(124, 137, 245),
            AgentKind::WorkBuddy => Color::Rgb(182, 155, 241),
        }
    }
}

/// Coarse risk classification for a process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Risk {
    Low,
    Medium,
    High,
}

impl Risk {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Risk::Low => "low",
            Risk::Medium => "med",
            Risk::High => "high",
        }
    }

    pub(crate) fn color(self) -> Color {
        match self {
            Risk::Low => GREEN,
            Risk::Medium => YELLOW,
            Risk::High => RED,
        }
    }
}

/// One host process row.
#[derive(Debug, Clone)]
pub(crate) struct ProcessRow {
    pub(crate) pid: u32,
    pub(crate) ppid: u32,
    pub(crate) cpu_pct: f32,
    pub(crate) mem_pct: f32,
    pub(crate) elapsed: String,
    pub(crate) cwd: Option<String>,
    pub(crate) command: String,
    pub(crate) agent: Option<AgentKind>,
    pub(crate) risk: Risk,
}

/// Snapshot the host process table through sysinfo, sorted agents-first then
/// CPU desc. Keeping one process-global System preserves CPU deltas between
/// refreshes and gives the TUI the same implementation on macOS, Linux, and
/// Windows.
pub(crate) async fn collect_processes() -> anyhow::Result<Vec<ProcessRow>> {
    tokio::task::spawn_blocking(collect_processes_sync)
        .await
        .map_err(|error| anyhow::anyhow!("process collector join failed: {error}"))?
}

static PROCESS_SYSTEM: OnceLock<Mutex<System>> = OnceLock::new();

fn collect_processes_sync() -> anyhow::Result<Vec<ProcessRow>> {
    let system = PROCESS_SYSTEM.get_or_init(|| Mutex::new(System::new_all()));
    let mut system = system
        .lock()
        .map_err(|_| anyhow::anyhow!("process collector state is poisoned"))?;
    system.refresh_processes(ProcessesToUpdate::All, true);
    system.refresh_memory();
    let total_memory = system.total_memory();
    let mut rows = system
        .processes()
        .iter()
        .map(|(pid, process)| {
            let command = if process.cmd().is_empty() {
                process.name().to_string_lossy().into_owned()
            } else {
                process
                    .cmd()
                    .iter()
                    .map(|part| part.to_string_lossy())
                    .collect::<Vec<_>>()
                    .join(" ")
            };
            let agent =
                detect_agent(&process.name().to_string_lossy()).or_else(|| detect_agent(&command));
            ProcessRow {
                pid: pid.as_u32(),
                ppid: process.parent().map(|pid| pid.as_u32()).unwrap_or_default(),
                cpu_pct: process.cpu_usage(),
                mem_pct: if total_memory == 0 {
                    0.0
                } else {
                    process.memory() as f32 * 100.0 / total_memory as f32
                },
                elapsed: format_elapsed_seconds(process.run_time()),
                cwd: process.cwd().map(|path| path.display().to_string()),
                risk: process_risk(&command, agent),
                command,
                agent,
            }
        })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.agent.is_some().cmp(&a.agent.is_some()).then(
            b.cpu_pct
                .partial_cmp(&a.cpu_pct)
                .unwrap_or(std::cmp::Ordering::Equal),
        )
    });
    Ok(rows)
}

#[cfg(test)]
pub(crate) fn parse_process_line(line: &str) -> Option<ProcessRow> {
    let mut it = line.split_whitespace();
    let pid = it.next()?.parse().ok()?;
    let ppid = it.next()?.parse().ok()?;
    let cpu_pct = it.next()?.parse().ok()?;
    let mem_pct = it.next()?.parse().ok()?;
    let elapsed = it.next()?.to_string();
    // Command is the verbatim remainder after the 5 fixed columns — slice it
    // from the original line instead of collect()+join() to avoid a Vec alloc
    // per process per tick (and to preserve the command's internal spacing).
    let command = remainder_after_fields(line, 5)?.trim_end();
    if command.is_empty() {
        return None;
    }
    let command = command.to_string();
    let agent = detect_agent(&command);
    Some(ProcessRow {
        pid,
        ppid,
        cpu_pct,
        mem_pct,
        elapsed,
        cwd: None,
        risk: process_risk(&command, agent),
        command,
        agent,
    })
}

#[cfg(test)]
/// Return the slice of `line` after skipping `n` whitespace-separated fields.
fn remainder_after_fields(line: &str, n: usize) -> Option<&str> {
    let mut rest = line.trim_start();
    for _ in 0..n {
        let end = rest.find(char::is_whitespace)?;
        rest = rest[end..].trim_start();
    }
    Some(rest)
}

#[cfg(test)]
pub(crate) fn parse_lsof_cwd(text: &str) -> Option<String> {
    text.lines()
        .find_map(|line| line.strip_prefix('n'))
        .map(str::trim)
        .filter(|path| !path.is_empty())
        .map(ToString::to_string)
}

pub(crate) fn detect_agent(command: &str) -> Option<AgentKind> {
    let words = command.split_whitespace().collect::<Vec<_>>();
    let first = words.first().map(|word| executable_basename(word));
    let wrapper = matches!(
        first.as_deref(),
        Some("node" | "node.exe" | "bun" | "bun.exe" | "npx" | "npx.cmd" | "env")
    );
    let wrapper_target = wrapper.then(|| words.get(1)).flatten().map(|target| {
        target
            .trim_matches(['\'', '"'])
            .replace('\\', "/")
            .to_ascii_lowercase()
    });
    let executable = match first.as_deref() {
        Some("node" | "node.exe" | "bun" | "bun.exe" | "npx" | "npx.cmd" | "env") => {
            words.get(1).map(|word| executable_basename(word))
        }
        _ => first,
    };
    let executable = executable.as_deref().unwrap_or_default();
    let subcommand = words.get(1).map(|word| word.to_ascii_lowercase());
    if matches!(executable, "a3s-code" | "a3s-code.exe")
        || (matches!(executable, "a3s" | "a3s.exe") && subcommand.as_deref() == Some("code"))
    {
        Some(AgentKind::A3sCode)
    } else if matches!(executable, "claude" | "claude.exe" | "claude-code")
        || wrapper_target
            .as_deref()
            .is_some_and(|target| target.contains("/claude-code/") || target.ends_with("/claude"))
    {
        Some(AgentKind::ClaudeCode)
    } else if matches!(executable, "codex" | "codex.exe")
        || wrapper_target.as_deref().is_some_and(|target| {
            target.contains("/@openai/codex/") || target.ends_with("/codex.js")
        })
    {
        Some(AgentKind::Codex)
    } else if matches!(executable, "cursor-agent" | "cursor-agent.exe")
        || wrapper_target
            .as_deref()
            .is_some_and(|target| target.contains("/cursor-agent/"))
    {
        Some(AgentKind::Cursor)
    } else if matches!(executable, "gemini" | "gemini.exe")
        || wrapper_target
            .as_deref()
            .is_some_and(|target| target.contains("/gemini-cli/"))
    {
        Some(AgentKind::Gemini)
    } else if matches!(
        executable,
        "workbuddy" | "workbuddy.exe" | "codebuddy" | "codebuddy.exe" | "cbc" | "cbc.exe"
    ) || wrapper_target
        .as_deref()
        .is_some_and(|target| target.contains("/workbuddy/") || target.contains("/codebuddy/"))
    {
        Some(AgentKind::WorkBuddy)
    } else {
        None
    }
}

fn executable_basename(value: &str) -> String {
    value
        .trim_matches(['\'', '"'])
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(value)
        .to_ascii_lowercase()
}

fn format_elapsed_seconds(seconds: u64) -> String {
    let days = seconds / 86_400;
    let hours = (seconds % 86_400) / 3_600;
    let minutes = (seconds % 3_600) / 60;
    let seconds = seconds % 60;
    if days > 0 {
        format!("{days}-{hours:02}:{minutes:02}:{seconds:02}")
    } else if hours > 0 {
        format!("{hours:02}:{minutes:02}:{seconds:02}")
    } else {
        format!("{minutes:02}:{seconds:02}")
    }
}

pub(crate) fn process_risk(command: &str, agent: Option<AgentKind>) -> Risk {
    let lower = command.to_lowercase();
    if lower.contains("sudo ")
        || lower.contains(" rm -rf ")
        || lower.contains("ptrace")
        || lower.contains("nmap ")
    {
        Risk::High
    } else if agent.is_some()
        || lower.contains("docker ")
        || lower.contains("curl ")
        || lower.contains("bash -c")
    {
        Risk::Medium
    } else {
        Risk::Low
    }
}
