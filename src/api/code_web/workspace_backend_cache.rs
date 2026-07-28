use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use a3s_code_core::{
    LocalCodeIntelligence, LocalWorkspaceManifest, LocalWorkspaceManifestSnapshot,
    ManifestWorkspaceBackend, WorkspaceCodeIntelligence, WorkspaceFileSystem, WorkspaceServices,
};
use anyhow::{Context, Result};
use tokio::sync::Mutex;

const INITIAL_MANIFEST_WAIT: std::time::Duration = std::time::Duration::from_secs(2);

struct CachedWorkspaceBackend {
    services: Arc<WorkspaceServices>,
    manifest: Arc<LocalWorkspaceManifest>,
    code_intelligence: Arc<LocalCodeIntelligence>,
}

/// Owns the manifest, semantic runtime, and workspace services shared by all
/// Web sessions rooted at the same canonical directory.
#[derive(Default)]
pub(in crate::api::code_web) struct WorkspaceBackendCache {
    state: Mutex<WorkspaceBackendCacheState>,
}

#[derive(Default)]
struct WorkspaceBackendCacheState {
    entries: HashMap<PathBuf, CachedWorkspaceBackend>,
    closed: bool,
}

impl WorkspaceBackendCache {
    pub(in crate::api::code_web) async fn services_for(
        &self,
        workspace: &Path,
    ) -> Result<Arc<WorkspaceServices>> {
        let canonical_root = canonical_workspace_root(workspace).await?;
        let mut state = self.state.lock().await;
        anyhow::ensure!(!state.closed, "workspace backend cache is closed");
        if let Some(entry) = state.entries.get(&canonical_root) {
            return Ok(Arc::clone(&entry.services));
        }

        let backend = ManifestWorkspaceBackend::new(canonical_root.clone());
        let manifest = backend.manifest();
        let file_system: Arc<dyn WorkspaceFileSystem> = backend.clone();
        let code_intelligence =
            LocalCodeIntelligence::start("a3s-code-web", Arc::clone(&manifest), file_system)
                .await
                .with_context(|| {
                    format!(
                        "failed to start Code Intelligence for {}",
                        canonical_root.display()
                    )
                })?;
        let provider: Arc<dyn WorkspaceCodeIntelligence> = code_intelligence.clone();
        let services = WorkspaceServices::local_with_manifest_backend(backend)
            .with_code_intelligence(provider);

        state.entries.insert(
            canonical_root,
            CachedWorkspaceBackend {
                services: Arc::clone(&services),
                manifest,
                code_intelligence,
            },
        );
        Ok(services)
    }

    /// Return the shared file manifest, waiting briefly for its first scan.
    ///
    /// A very large workspace may outlive this bound. In that case callers get
    /// the newest snapshot immediately and can retry without starting a second
    /// traversal or blocking the Web request indefinitely.
    pub(in crate::api::code_web) async fn manifest_snapshot_for(
        &self,
        workspace: &Path,
    ) -> Result<LocalWorkspaceManifestSnapshot> {
        let services = self.services_for(workspace).await?;
        let canonical_root = services
            .local_root()
            .context("local workspace services did not expose a root")?;
        let manifest = {
            let state = self.state.lock().await;
            Arc::clone(
                &state
                    .entries
                    .get(canonical_root)
                    .context("workspace manifest was not cached")?
                    .manifest,
            )
        };
        let mut snapshots = manifest.subscribe();
        let current = manifest.snapshot();
        if current.version > 0 {
            return Ok(current);
        }

        let scanned = tokio::time::timeout(INITIAL_MANIFEST_WAIT, async {
            loop {
                match snapshots.recv().await {
                    Ok(snapshot) if snapshot.version > 0 => return snapshot,
                    Ok(_) | Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => {
                        let current = manifest.snapshot();
                        if current.version > 0 {
                            return current;
                        }
                    }
                    Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                        return manifest.snapshot();
                    }
                }
            }
        })
        .await;
        Ok(scanned.unwrap_or_else(|_| manifest.snapshot()))
    }

    pub(in crate::api::code_web) async fn close(&self) {
        let entries = {
            let mut state = self.state.lock().await;
            state.closed = true;
            std::mem::take(&mut state.entries)
        };
        // Stop synchronous discovery first. Aborting only the async manifest
        // owner does not signal a traversal that has already started.
        for entry in entries.values() {
            entry.manifest.shutdown();
        }
        for entry in entries.into_values() {
            entry.code_intelligence.shutdown().await;
        }
    }

    #[cfg(test)]
    async fn len(&self) -> usize {
        self.state.lock().await.entries.len()
    }
}

async fn canonical_workspace_root(workspace: &Path) -> Result<PathBuf> {
    let canonical = tokio::fs::canonicalize(workspace).await.with_context(|| {
        format!(
            "failed to resolve workspace directory {}",
            workspace.display()
        )
    })?;
    let metadata = tokio::fs::metadata(&canonical).await.with_context(|| {
        format!(
            "failed to inspect workspace directory {}",
            canonical.display()
        )
    })?;
    anyhow::ensure!(
        metadata.is_dir(),
        "workspace root is not a directory: {}",
        canonical.display()
    );
    Ok(canonical)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn canonical_workspace_reuses_one_services_instance() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let nested = workspace.path().join("nested");
        tokio::fs::create_dir(&nested)
            .await
            .expect("create nested directory");
        let alternate = nested.join("..");
        let cache = WorkspaceBackendCache::default();

        let first = cache
            .services_for(workspace.path())
            .await
            .expect("first services");
        let second = cache
            .services_for(&alternate)
            .await
            .expect("shared services");

        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(cache.len().await, 1);
        cache.close().await;
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn workspace_root_must_be_a_directory() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        let file = workspace.path().join("file.rs");
        tokio::fs::write(&file, "fn main() {}")
            .await
            .expect("write file");
        let cache = WorkspaceBackendCache::default();

        let error = cache
            .services_for(&file)
            .await
            .expect_err("file roots must be rejected");

        assert!(error.to_string().contains("not a directory"));
        assert_eq!(cache.len().await, 0);
    }

    #[tokio::test]
    async fn different_workspaces_have_isolated_services() {
        let first_workspace = tempfile::tempdir().expect("first workspace");
        let second_workspace = tempfile::tempdir().expect("second workspace");
        let cache = WorkspaceBackendCache::default();

        let first = cache
            .services_for(first_workspace.path())
            .await
            .expect("first services");
        let second = cache
            .services_for(second_workspace.path())
            .await
            .expect("second services");

        assert!(!Arc::ptr_eq(&first, &second));
        assert_ne!(first.local_root(), second.local_root());
        assert_eq!(cache.len().await, 2);
        cache.close().await;
    }

    #[tokio::test]
    async fn close_is_idempotent_and_stops_in_flight_workspace_discovery() {
        let workspace = tempfile::tempdir().expect("temporary workspace");
        for directory in 0..64 {
            let directory = workspace.path().join(format!("tree/{directory}"));
            std::fs::create_dir_all(&directory).expect("create scan directory");
            for file in 0..64 {
                std::fs::write(directory.join(format!("file-{file}.ts")), b"export {};")
                    .expect("write scan fixture");
            }
        }
        let cache = WorkspaceBackendCache::default();
        cache
            .services_for(workspace.path())
            .await
            .expect("workspace services");
        let manifest = {
            let state = cache.state.lock().await;
            Arc::clone(
                &state
                    .entries
                    .values()
                    .next()
                    .expect("cached workspace")
                    .manifest,
            )
        };
        let mut snapshots = manifest.subscribe();

        tokio::time::timeout(std::time::Duration::from_secs(2), cache.close())
            .await
            .expect("cache close must not wait for the full scan");
        cache.close().await;
        while snapshots.try_recv().is_ok() {}
        std::fs::write(workspace.path().join("after-close.ts"), b"export {};")
            .expect("write after close");

        assert_eq!(cache.len().await, 0);
        let error = cache
            .services_for(workspace.path())
            .await
            .expect_err("closed cache must reject new workspace services");
        assert!(error.to_string().contains("cache is closed"));
        assert!(
            tokio::time::timeout(std::time::Duration::from_millis(300), snapshots.recv())
                .await
                .is_err()
        );
    }
}
