//! `a3s` — the A3S coding agent CLI.
//!
//! `a3s code` launches the interactive terminal UI (the coding agent); the
//! rest are basic commands.

mod a3s_os;
mod account_providers;
mod api;
mod budget;
mod cli;
mod commands;
mod compact;
mod config;
mod evolution;
mod host_command_guardrail;
mod model;
#[path = "research/code.rs"]
mod research;
mod runtime_tool;
mod session_llm;
mod system_agents;
mod timeline;
mod top;
mod tui;
mod update;
mod use_registry;
mod user_paths;

#[cfg(test)]
static TEST_ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

const RUNTIME_SHUTDOWN_GRACE: std::time::Duration = std::time::Duration::from_secs(2);

fn main() -> std::process::ExitCode {
    let runtime = match tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("failed to start the A3S async runtime: {error}");
            return std::process::ExitCode::FAILURE;
        }
    };
    let exit_code = runtime.block_on(cli::run(std::env::args_os()));
    // Tokio waits indefinitely for blocking-pool work during Runtime::drop.
    // Product hosts perform explicit cleanup; this final bound prevents an
    // unresponsive filesystem or child adapter from keeping a finished CLI
    // process alive forever.
    runtime.shutdown_timeout(RUNTIME_SHUTDOWN_GRACE);
    exit_code
}
