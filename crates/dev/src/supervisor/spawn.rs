use std::sync::Arc;

use tokio::process::{Child, Command};

use crate::config::ServiceDef;
use crate::error::{DevError, Result};
use crate::log::LogAggregator;

/// Everything needed to spawn a service process.
pub struct SpawnSpec<'a> {
    pub name: &'a str,
    pub svc: &'a ServiceDef,
    pub port: u16,
    pub color_idx: usize,
}

pub struct SpawnResult {
    pub child: Child,
    pub pid: u32,
}

/// Spawn a service process, attach stdout to the log aggregator, and return the child.
/// Stderr is forwarded to the log aggregator as well.
pub async fn spawn_process(spec: &SpawnSpec<'_>, log: &Arc<LogAggregator>) -> Result<SpawnResult> {
    let parts = split_cmd(&spec.svc.cmd);
    let program = parts.first().map(|s| s.as_str()).unwrap_or("sh");
    let args = &parts[1..];
    let extra_args = framework_port_args(&parts, spec.port);

    let mut cmd = Command::new(program);
    cmd.args(args)
        .args(&extra_args)
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .envs(&spec.svc.env)
        .env("PORT", spec.port.to_string())
        .env("HOST", "127.0.0.1");

    if let Some(dir) = &spec.svc.dir {
        cmd.current_dir(dir);
    }

    let mut child = cmd.spawn().map_err(|e| DevError::Process {
        service: spec.name.to_string(),
        msg: e.to_string(),
    })?;

    let pid = child.id().unwrap_or(0);

    if let Some(stdout) = child.stdout.take() {
        log.attach(spec.name.to_string(), spec.color_idx, stdout);
    }

    if let Some(stderr) = child.stderr.take() {
        log.attach_stderr(spec.name.to_string(), spec.color_idx, stderr);
    }

    Ok(SpawnResult { child, pid })
}

/// Bind to port 0 and return the OS-assigned free port.
pub fn free_port() -> Option<u16> {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").ok()?;
    Some(listener.local_addr().ok()?.port())
}

/// Shell-style command splitting: handles single/double quotes and backslash escapes.
/// e.g. `node server.js --title 'hello world'` → ["node", "server.js", "--title", "hello world"]
pub fn split_cmd(cmd: &str) -> Vec<String> {
    let mut args = Vec::new();
    let mut current = String::new();
    let mut chars = cmd.chars().peekable();
    let mut in_single = false;
    let mut in_double = false;

    while let Some(ch) = chars.next() {
        match ch {
            '\\' if !in_single => {
                if let Some(next) = chars.next() {
                    current.push(next);
                }
            }
            '\'' if !in_double => in_single = !in_single,
            '"' if !in_single => in_double = !in_double,
            ' ' | '\t' if !in_single && !in_double => {
                if !current.is_empty() {
                    args.push(current.clone());
                    current.clear();
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        args.push(current);
    }
    args
}

/// Detect framework from the command and inject `--port <port>` if needed.
/// `parts` is the full split command (program + args).
pub fn framework_port_args(parts: &[String], port: u16) -> Vec<String> {
    let p = port.to_string();
    let direct = ["vite", "next", "astro", "nuxt", "remix", "svelte-kit", "wrangler"];
    let runners = ["npx", "pnpm", "yarn", "bunx"];

    let program = parts.first().map(|s| s.as_str()).unwrap_or("");
    let second = parts.get(1).map(|s| s.as_str()).unwrap_or("");

    let framework = if direct.contains(&program) {
        program
    } else if runners.contains(&program) {
        if second == "exec" || second == "run" || second == "dlx" {
            parts.get(2).map(|s| s.as_str()).unwrap_or("")
        } else {
            second
        }
    } else {
        ""
    };

    if direct.contains(&framework) {
        vec!["--port".into(), p]
    } else {
        vec![]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_split_cmd_simple() {
        assert_eq!(split_cmd("node server.js"), vec!["node", "server.js"]);
    }

    #[test]
    fn test_split_cmd_single_quotes() {
        assert_eq!(
            split_cmd("node server.js --title 'hello world'"),
            vec!["node", "server.js", "--title", "hello world"]
        );
    }

    #[test]
    fn test_split_cmd_double_quotes() {
        assert_eq!(
            split_cmd(r#"echo "hello world""#),
            vec!["echo", "hello world"]
        );
    }

    #[test]
    fn test_split_cmd_backslash() {
        assert_eq!(split_cmd(r"echo hello\ world"), vec!["echo", "hello world"]);
    }

    #[test]
    fn test_framework_port_args_direct() {
        let parts = vec!["vite".to_string()];
        assert_eq!(framework_port_args(&parts, 3000), vec!["--port", "3000"]);
    }

    #[test]
    fn test_framework_port_args_npx() {
        let parts = vec!["npx".to_string(), "vite".to_string()];
        assert_eq!(framework_port_args(&parts, 3000), vec!["--port", "3000"]);
    }

    #[test]
    fn test_framework_port_args_pnpm_exec() {
        let parts = vec!["pnpm".to_string(), "exec".to_string(), "next".to_string()];
        assert_eq!(framework_port_args(&parts, 3000), vec!["--port", "3000"]);
    }

    #[test]
    fn test_framework_port_args_unknown() {
        let parts = vec!["node".to_string(), "server.js".to_string()];
        assert_eq!(framework_port_args(&parts, 3000), Vec::<String>::new());
    }
}
