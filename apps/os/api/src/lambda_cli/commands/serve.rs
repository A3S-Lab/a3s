//! `a3s serve` command - Start the A3S API server.
//!
//! Starts the REST API server for cluster management.

use crate::api::server::{start_server, ApiServerConfig, ApiServerState};
use crate::commands::Command;
use crate::controller::reconciler::Reconciler;
use crate::errors::Result;
use crate::state::{
    batch::{CronJobController, JobController},
    daemon::DaemonController,
    event::EventController,
    limitrange::LimitRangeController,
    rbac::RbacController,
    resourcequota::ResourceQuotaController,
    runtimeclass::RuntimeClassController,
    service_account::ServiceAccountController,
    sqlite_store::SqliteStateStore,
    stateful::StatefulSetController,
    token::TokenController,
    volume::{StorageClassController, VolumeManager},
};
use async_trait::async_trait;
use clap::Parser;
use std::path::PathBuf;
use std::sync::Arc;

/// Serve command - start the API server.
#[derive(Parser, Debug)]
pub struct ServeCommand {
    /// Listen address.
    #[arg(long, default_value = "0.0.0.0")]
    address: String,

    /// Listen port.
    #[arg(short, long, default_value = "6443")]
    port: u16,

    /// Enable read-only mode.
    #[arg(long)]
    read_only: bool,

    /// Enable debug endpoints.
    #[arg(long)]
    debug: bool,

    /// Working directory for state storage.
    #[arg(long, default_value = ".")]
    working_dir: PathBuf,
}

#[async_trait]
impl Command for ServeCommand {
    async fn run(&self) -> Result<()> {
        println!("Starting A3S API server...");
        println!("Working directory: {}", self.working_dir.display());

        // Create SQLite store (K3s-style embedded database)
        let sqlite_store = match SqliteStateStore::new(&self.working_dir) {
            Ok(store) => {
                println!("SQLite state store initialized");
                Some(Arc::new(store))
            }
            Err(e) => {
                eprintln!("Warning: Failed to initialize SQLite store: {}", e);
                eprintln!("Running without persistent storage");
                None
            }
        };

        // Create controllers
        let volume_manager = Arc::new(VolumeManager::new(&self.working_dir));
        let service_accounts = Arc::new(ServiceAccountController::new());
        let rbac = Arc::new(RbacController::new());
        let storage_classes = Arc::new(StorageClassController::new(volume_manager.clone()));
        let events = Arc::new(EventController::new());
        let daemonsets = Arc::new(DaemonController::new());
        let statefulsets = Arc::new(StatefulSetController::new());
        let jobs = Arc::new(JobController::new());
        let cronjobs = Arc::new(CronJobController::new());
        let limitranges = Arc::new(LimitRangeController::new());
        let resource_quotas = Arc::new(ResourceQuotaController::new());
        let token_controller = Arc::new(TokenController::new(
            "https://kubernetes.default.svc".to_string(),
            "PLACEHOLDER_CA_DATA".to_string(),
        ));
        let runtime_classes = Arc::new(RuntimeClassController::new());

        // Create API server state
        let state = Arc::new(ApiServerState::new(
            service_accounts,
            rbac,
            storage_classes,
            volume_manager,
            events,
            daemonsets,
            statefulsets,
            jobs,
            cronjobs,
            limitranges,
            resource_quotas,
            token_controller,
            runtime_classes,
            sqlite_store,
        ));

        // Create server config
        let config = ApiServerConfig {
            listen_addr: self.address.clone(),
            port: self.port,
            read_only: self.read_only,
            debug: self.debug,
        };

        // Start reconciler in background
        let reconciler = Arc::new(Reconciler::new(&self.working_dir));
        let reconciler_handle = tokio::spawn({
            let reconciler = reconciler.clone();
            async move {
                reconciler.run().await;
            }
        });

        // Handle shutdown gracefully
        tokio::select! {
            result = start_server(config, state) => {
                reconciler.stop();
                if let Err(e) = result {
                    tracing::error!(error = %e, "API server error");
                }
            }
            _ = tokio::signal::ctrl_c() => {
                tracing::info!("Received shutdown signal");
                reconciler.stop();
            }
        }

        // Wait for reconciler to finish
        let _ = reconciler_handle.await;

        Ok(())
    }
}
