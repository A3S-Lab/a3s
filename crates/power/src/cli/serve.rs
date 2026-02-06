use crate::config::PowerConfig;
use crate::error::Result;
use crate::server;

/// Execute the `serve` command: start the HTTP server.
pub async fn execute(host: &str, port: u16) -> Result<()> {
    let mut config = PowerConfig::load()?;

    // Override config with CLI arguments
    config.host = host.to_string();
    config.port = port;

    println!("A3S Power server starting...");
    println!("Listening on http://{}:{}", config.host, config.port);
    println!("Press Ctrl+C to stop");

    server::start(config).await
}
