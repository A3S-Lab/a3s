//! Binary replacement logic.

use std::path::Path;

/// Replace the currently running binary with a new one.
///
/// 1. Determines the current executable path.
/// 2. Renames the current binary to `{name}.bak`.
/// 3. Copies the new binary into place.
/// 4. Sets executable permissions (unix).
pub fn replace_binary(new_binary_path: &Path) -> anyhow::Result<()> {
    let current_exe = std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("Failed to determine current executable path: {}", e))?;

    // Resolve symlinks to get the real path
    let current_exe = current_exe
        .canonicalize()
        .map_err(|e| anyhow::anyhow!("Failed to resolve current executable path: {}", e))?;

    let backup_path = current_exe.with_extension("bak");

    // Rename current binary to .bak
    std::fs::rename(&current_exe, &backup_path).map_err(|e| {
        anyhow::anyhow!(
            "Failed to rename {} to {}: {}",
            current_exe.display(),
            backup_path.display(),
            e
        )
    })?;

    // Copy new binary into place
    if let Err(e) = std::fs::copy(new_binary_path, &current_exe) {
        // Attempt to restore backup on failure
        let _ = std::fs::rename(&backup_path, &current_exe);
        return Err(anyhow::anyhow!(
            "Failed to install new binary to {}: {}",
            current_exe.display(),
            e
        ));
    }

    // Set executable permissions on unix
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(0o755);
        std::fs::set_permissions(&current_exe, perms).map_err(|e| {
            anyhow::anyhow!(
                "Failed to set permissions on {}: {}",
                current_exe.display(),
                e
            )
        })?;
    }

    // Clean up backup
    let _ = std::fs::remove_file(&backup_path);

    // Clean up temp directory
    if let Some(parent) = new_binary_path.parent() {
        let _ = std::fs::remove_dir_all(parent);
    }

    Ok(())
}
