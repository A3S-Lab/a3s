use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use a3s_boot::{BootError, Result as BootResult};
use serde::de::DeserializeOwned;
use serde::Serialize;
use tokio::fs;
use tokio::io::AsyncWriteExt;

static TEMPORARY_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

pub(super) async fn read_json<T>(path: &Path) -> BootResult<T>
where
    T: DeserializeOwned,
{
    let bytes = fs::read(path)
        .await
        .map_err(|error| storage_error(path, error))?;
    serde_json::from_slice(&bytes).map_err(|error| {
        BootError::Internal(format!(
            "failed to decode Work storage file {}: {error}",
            path.display()
        ))
    })
}

pub(super) async fn read_json_optional<T>(path: &Path) -> BootResult<Option<T>>
where
    T: DeserializeOwned,
{
    match fs::read(path).await {
        Ok(bytes) => serde_json::from_slice(&bytes).map(Some).map_err(|error| {
            BootError::Internal(format!(
                "failed to decode Work storage file {}: {error}",
                path.display()
            ))
        }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(storage_error(path, error)),
    }
}

pub(super) async fn list_json<T>(directory: &Path) -> BootResult<Vec<T>>
where
    T: DeserializeOwned,
{
    let mut entries = match fs::read_dir(directory).await {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(storage_error(directory, error)),
    };
    let mut values = Vec::new();
    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|error| storage_error(directory, error))?
    {
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        values.push(read_json(&path).await?);
    }
    Ok(values)
}

pub(super) async fn write_json_atomic<T>(path: &Path, value: &T) -> BootResult<()>
where
    T: Serialize,
{
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| {
        BootError::Internal(format!(
            "failed to encode Work storage file {}: {error}",
            path.display()
        ))
    })?;
    write_bytes_atomic(path, &bytes).await
}

pub(super) async fn write_bytes_atomic(path: &Path, bytes: &[u8]) -> BootResult<()> {
    let parent = path.parent().ok_or_else(|| {
        BootError::Internal(format!(
            "Work storage path has no parent: {}",
            path.display()
        ))
    })?;
    fs::create_dir_all(parent)
        .await
        .map_err(|error| storage_error(parent, error))?;
    let temporary = temporary_path(path);
    let write_result = async {
        let mut file = fs::File::create(&temporary)
            .await
            .map_err(|error| storage_error(&temporary, error))?;
        file.write_all(bytes)
            .await
            .map_err(|error| storage_error(&temporary, error))?;
        file.sync_all()
            .await
            .map_err(|error| storage_error(&temporary, error))?;
        replace_file(&temporary, path).await
    }
    .await;
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary).await;
    }
    write_result
}

pub(super) async fn remove_file_if_exists(path: &Path) -> BootResult<()> {
    match fs::remove_file(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage_error(path, error)),
    }
}

pub(super) async fn remove_dir_if_exists(path: &Path) -> BootResult<()> {
    match fs::remove_dir_all(path).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(storage_error(path, error)),
    }
}

pub(super) async fn copy_file_if_exists(source: &Path, destination: &Path) -> BootResult<bool> {
    let bytes = match fs::read(source).await {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(error) => return Err(storage_error(source, error)),
    };
    write_bytes_atomic(destination, &bytes).await?;
    Ok(true)
}

pub(super) fn storage_error(path: &Path, error: std::io::Error) -> BootError {
    BootError::Internal(format!(
        "Work storage operation failed for {}: {error}",
        path.display()
    ))
}

fn temporary_path(path: &Path) -> PathBuf {
    let sequence = TEMPORARY_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("work");
    path.with_file_name(format!(
        ".{file_name}.tmp-{}-{sequence}",
        std::process::id()
    ))
}

async fn replace_file(source: &Path, destination: &Path) -> BootResult<()> {
    #[cfg(windows)]
    if fs::try_exists(destination)
        .await
        .map_err(|error| storage_error(destination, error))?
    {
        fs::remove_file(destination)
            .await
            .map_err(|error| storage_error(destination, error))?;
    }
    fs::rename(source, destination)
        .await
        .map_err(|error| storage_error(destination, error))
}
