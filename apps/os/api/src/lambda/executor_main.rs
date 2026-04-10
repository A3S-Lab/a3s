//! `a3s-executor` — guest-side binary that runs inside a MicroVM.
//!
//! Receives execution requests from the Lambda host and writes structured
//! output back to a shared filesystem location.

use std::process::ExitCode;

use a3s_lambda::executor::guest::{run_guest_executor, GuestExecutorArgs};

#[tokio::main]
async fn main() -> ExitCode {
    // Initialize tracing with a grounded default if no env var is set.
    let filter = tracing_subscriber::EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| "a3s_lambda::executor::guest=debug".into());

    tracing_subscriber::fmt().with_env_filter(filter).init();

    let argv: Vec<String> = std::env::args().skip(1).collect();
    let args = match GuestExecutorArgs::parse(&argv) {
        Ok(args) => args,
        Err(e) => {
            eprintln!("a3s-executor: invalid arguments: {e}");
            return ExitCode::from(1_u8);
        }
    };

    if let Err(e) = run_guest_executor(args).await {
        tracing::error!(error = %e, "guest executor failed");
        eprintln!("a3s-executor: {e}");
        return ExitCode::from(1_u8);
    }

    ExitCode::SUCCESS
}
