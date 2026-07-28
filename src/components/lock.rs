use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use anyhow::Context;
use fs2::FileExt;

use super::id::ComponentId;

/// Cross-process guard for one top-level component family.
///
/// Delegated children share their parent's lock (for example every `use/*`
/// operation uses the `use` lock). This keeps parent installation and child
/// lifecycle calls in one deterministic order and avoids parent/child
/// deadlocks during cascade operations.
pub struct ComponentOperationLock {
    file: File,
}

impl ComponentOperationLock {
    pub async fn acquire(path: PathBuf, component: &ComponentId) -> anyhow::Result<Self> {
        let component = component.to_string();
        tokio::task::spawn_blocking(move || Self::acquire_blocking(&path, &component))
            .await
            .context("component lock task failed")?
    }

    pub fn acquire_sync(path: &Path, component: &ComponentId) -> anyhow::Result<Self> {
        Self::acquire_blocking(path, component.as_str())
    }

    fn acquire_blocking(path: &Path, component: &str) -> anyhow::Result<Self> {
        let parent = path
            .parent()
            .context("component operation lock has no parent directory")?;
        std::fs::create_dir_all(parent).with_context(|| {
            format!(
                "failed to create component lock directory {}",
                parent.display()
            )
        })?;
        let mut file = OpenOptions::new()
            .create(true)
            .truncate(false)
            .read(true)
            .write(true)
            .open(path)
            .with_context(|| format!("failed to open component lock {}", path.display()))?;
        file.lock_exclusive()
            .with_context(|| format!("failed to acquire component lock {}", path.display()))?;
        file.set_len(0)
            .with_context(|| format!("failed to truncate component lock {}", path.display()))?;
        writeln!(file, "pid={} component={component}", std::process::id())
            .with_context(|| format!("failed to write component lock {}", path.display()))?;
        Ok(Self { file })
    }
}

impl Drop for ComponentOperationLock {
    fn drop(&mut self) {
        let _ = FileExt::unlock(&self.file);
    }
}

#[cfg(test)]
mod tests {
    use std::sync::mpsc;
    use std::time::Duration;

    use super::*;

    #[test]
    fn concurrent_operations_wait_for_the_existing_lock() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("locks/use.lock");
        let first = ComponentOperationLock::acquire_blocking(&path, "use").unwrap();
        let (started_tx, started_rx) = mpsc::channel();
        let (acquired_tx, acquired_rx) = mpsc::channel();
        let thread_path = path.clone();
        let thread = std::thread::spawn(move || {
            started_tx.send(()).unwrap();
            let _second =
                ComponentOperationLock::acquire_blocking(&thread_path, "use/browser").unwrap();
            acquired_tx.send(()).unwrap();
        });

        started_rx.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(acquired_rx
            .recv_timeout(Duration::from_millis(100))
            .is_err());
        drop(first);
        acquired_rx.recv_timeout(Duration::from_secs(2)).unwrap();
        thread.join().unwrap();
    }
}
