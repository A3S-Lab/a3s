use a3s_lambda::RunMode;
use a3s_lambda::{ExecutionRegistry, PgLeaseWorker};
use a3s_lambda::{LambdaConfig, LambdaServer};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "info,a3s_lambda=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    // Load configuration
    let config_file = std::env::var("A3S_LAMBDA_CONFIG_FILE").ok();
    let config = LambdaConfig::load(config_file.as_deref())?;

    tracing::info!("Configuration loaded: {:?}", config);

    let execution_registry = std::sync::Arc::new(
        ExecutionRegistry::from_enabled_with_launch_mode(
            &config.execution.enabled_adapters,
            config.execution.launch_mode,
        )
        .map_err(|e| format!("failed to build execution registry: {e}"))?,
    );

    let run_mode = config.resolved_run_mode();
    let run_server = matches!(run_mode, RunMode::Server | RunMode::All);
    let run_worker = matches!(run_mode, RunMode::Worker | RunMode::All);

    match (run_server, run_worker) {
        (true, true) => {
            let server = LambdaServer::builder(config.clone())
                .execution_registry(execution_registry.clone())
                .build()
                .await?;
            let worker = PgLeaseWorker::from_config(
                &config.worker,
                &config.registry.url,
                std::path::PathBuf::from(&config.registry.temp_dir),
                &config.database,
                execution_registry,
            )
            .await?;

            tokio::try_join!(server.run(), worker.run())?;
        }
        (true, false) => {
            let server = LambdaServer::builder(config)
                .execution_registry(execution_registry)
                .build()
                .await?;
            server.run().await?;
        }
        (false, true) => {
            let worker = PgLeaseWorker::from_config(
                &config.worker,
                &config.registry.url,
                std::path::PathBuf::from(&config.registry.temp_dir),
                &config.database,
                execution_registry,
            )
            .await?;
            worker.run().await?;
        }
        (false, false) => {
            return Err("resolved runtime mode produced no active runtime".into());
        }
    }

    Ok(())
}
