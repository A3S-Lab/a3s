use std::collections::HashSet;

use a3s_boot::{BootError, Result as BootResult};
use serde_json::{json, Value};

use crate::top::{collect_processes, ProcessRow};

pub(in crate::api::code_web) struct ProcessesService;

impl ProcessesService {
    pub(in crate::api::code_web) fn new() -> Self {
        Self
    }

    pub(in crate::api::code_web) async fn top(&self) -> BootResult<Value> {
        let rows = collect_processes()
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        Ok(top_snapshot_json(&rows))
    }
}

fn top_snapshot_json(rows: &[ProcessRow]) -> Value {
    let activity_rows = agent_activity_rows(rows);
    json!({
        "generatedAt": chrono::Utc::now().to_rfc3339(),
        "stats": {
            "processes": rows.len(),
            "activityRows": activity_rows.len(),
            "agents": rows.iter().filter(|row| row.agent.is_some()).count(),
            "highRisk": rows.iter().filter(|row| row.risk.label() == "high").count(),
        },
        "rows": rows.iter().map(process_row_json).collect::<Vec<_>>(),
        "activityRows": activity_rows
            .iter()
            .map(|row| process_row_json(row))
            .collect::<Vec<_>>(),
    })
}

fn process_row_json(row: &ProcessRow) -> Value {
    json!({
        "pid": row.pid,
        "ppid": row.ppid,
        "cpuPct": row.cpu_pct,
        "memPct": row.mem_pct,
        "elapsed": row.elapsed.as_str(),
        "cwd": row.cwd.as_deref(),
        "command": row.command.as_str(),
        "agent": row.agent.map(|agent| agent.label()),
        "risk": row.risk.label(),
    })
}

fn agent_activity_rows(rows: &[ProcessRow]) -> Vec<&ProcessRow> {
    let roots = rows
        .iter()
        .filter(|row| row.agent.is_some())
        .map(|row| row.pid)
        .collect::<HashSet<_>>();
    if roots.is_empty() {
        return Vec::new();
    }
    process_forest(rows, roots)
}

fn process_forest(rows: &[ProcessRow], roots: HashSet<u32>) -> Vec<&ProcessRow> {
    let mut included = roots;
    loop {
        let mut added = false;
        for row in rows {
            if !included.contains(&row.pid) && included.contains(&row.ppid) {
                included.insert(row.pid);
                added = true;
            }
        }
        if !added {
            break;
        }
    }
    rows.iter()
        .filter(|row| included.contains(&row.pid))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::top::{AgentKind, Risk};

    #[test]
    fn top_snapshot_includes_agent_activity_forest() {
        let rows = vec![
            ProcessRow {
                pid: 10,
                ppid: 1,
                cpu_pct: 12.5,
                mem_pct: 3.0,
                elapsed: "00:10".to_string(),
                cwd: Some("/repo".to_string()),
                command: "a3s code".to_string(),
                agent: Some(AgentKind::A3sCode),
                risk: Risk::Medium,
            },
            ProcessRow {
                pid: 11,
                ppid: 10,
                cpu_pct: 2.0,
                mem_pct: 1.0,
                elapsed: "00:08".to_string(),
                cwd: None,
                command: "sh -c cargo check".to_string(),
                agent: None,
                risk: Risk::Low,
            },
            ProcessRow {
                pid: 99,
                ppid: 1,
                cpu_pct: 0.1,
                mem_pct: 0.2,
                elapsed: "01:00".to_string(),
                cwd: None,
                command: "launchd".to_string(),
                agent: None,
                risk: Risk::Low,
            },
        ];

        let snapshot = top_snapshot_json(&rows);
        assert_eq!(snapshot["stats"]["processes"], 3);
        assert_eq!(snapshot["stats"]["agents"], 1);
        assert_eq!(snapshot["stats"]["activityRows"], 2);
        assert_eq!(snapshot["activityRows"][0]["agent"], "a3s-code");
        assert_eq!(snapshot["activityRows"][1]["pid"], 11);
        assert_eq!(snapshot["rows"][0]["risk"], "med");
    }
}
