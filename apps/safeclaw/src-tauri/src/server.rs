/// Embedded SafeClaw gateway — runs the full server in-process.
///
/// Spawned as a background tokio task during Tauri setup so the UI
/// and gateway share the same process. The gateway listens on
/// `127.0.0.1:18790` (same default as the standalone CLI).
use anyhow::Result;

/// Default listen address for the embedded gateway.
const HOST: &str = "127.0.0.1";
const PORT: u16 = 18790;

/// Start the embedded SafeClaw gateway. Blocks until shutdown.
pub async fn start_embedded_gateway() -> Result<()> {
    let (config, config_path) = safeclaw::bootstrap::load_config(None)?;
    safeclaw::bootstrap::run_gateway(config, config_path, HOST, PORT, false).await
}
