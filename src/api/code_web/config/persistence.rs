use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::config::{rewrite_acl_sections, ConfigSection};
use a3s_code_core::CodeConfig;

static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

pub(crate) fn persist_config_sections(
    path: &Path,
    config: &CodeConfig,
    sections: &[ConfigSection],
) -> BootResult<CodeConfig> {
    let source = match fs::read_to_string(path) {
        Ok(source) => source,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => String::new(),
        Err(error) => {
            return Err(BootError::Internal(format!(
                "failed to read {}: {error}",
                path.display()
            )))
        }
    };
    let rendered = rewrite_acl_sections(&source, config, sections)
        .map_err(|error| BootError::BadRequest(error.to_string()))?;
    let verified = CodeConfig::from_acl(&rendered).map_err(|error| {
        BootError::BadRequest(format!("generated config.acl is invalid: {error}"))
    })?;
    write_atomic(path, rendered.as_bytes())?;
    Ok(verified)
}

pub(crate) fn write_atomic(path: &Path, contents: &[u8]) -> BootResult<()> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|error| {
        BootError::Internal(format!(
            "failed to create config directory {}: {error}",
            parent.display()
        ))
    })?;
    let temp_path = temporary_path(path);
    let result = write_and_replace(path, &temp_path, contents);
    if result.is_err() {
        let _ = fs::remove_file(&temp_path);
    }
    result
}

fn write_and_replace(path: &Path, temp_path: &Path, contents: &[u8]) -> BootResult<()> {
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(temp_path)
        .map_err(|error| {
            BootError::Internal(format!(
                "failed to create temporary config {}: {error}",
                temp_path.display()
            ))
        })?;
    file.write_all(contents).map_err(|error| {
        BootError::Internal(format!(
            "failed to write temporary config {}: {error}",
            temp_path.display()
        ))
    })?;
    file.sync_all().map_err(|error| {
        BootError::Internal(format!(
            "failed to flush temporary config {}: {error}",
            temp_path.display()
        ))
    })?;

    if let Ok(metadata) = fs::metadata(path) {
        fs::set_permissions(temp_path, metadata.permissions()).map_err(|error| {
            BootError::Internal(format!(
                "failed to preserve permissions for {}: {error}",
                path.display()
            ))
        })?;
    }

    fs::rename(temp_path, path).map_err(|error| {
        BootError::Internal(format!(
            "failed to replace config {}: {error}",
            path.display()
        ))
    })?;
    sync_parent_directory(path);
    Ok(())
}

fn sync_parent_directory(path: &Path) {
    let Some(parent) = path.parent() else {
        return;
    };
    if let Ok(directory) = File::open(parent) {
        let _ = directory.sync_all();
    }
}

fn temporary_path(path: &Path) -> PathBuf {
    let file_name = path
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("config.acl");
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos())
        .unwrap_or_default();
    let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    path.with_file_name(format!(
        ".{file_name}.{}.{}.{}.tmp",
        std::process::id(),
        timestamp,
        sequence
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn atomic_write_replaces_content_and_leaves_no_temp_file() {
        let directory = std::env::temp_dir().join(format!(
            "a3s-code-web-config-persistence-{}-{}",
            std::process::id(),
            TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&directory).expect("temp directory");
        let path = directory.join("config.acl");
        fs::write(&path, "default_model = \"old/model\"\n").expect("initial config");

        write_atomic(&path, b"default_model = \"new/model\"\n").expect("atomic write");

        assert_eq!(
            fs::read_to_string(&path).expect("updated config"),
            "default_model = \"new/model\"\n"
        );
        assert_eq!(
            fs::read_dir(&directory).expect("directory entries").count(),
            1
        );
        fs::remove_dir_all(&directory).expect("cleanup");
    }
}
