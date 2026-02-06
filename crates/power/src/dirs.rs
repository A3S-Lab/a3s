use std::path::PathBuf;

/// Returns the base directory for Power data.
///
/// Uses `$A3S_POWER_HOME` if set, otherwise defaults to `~/.a3s/power`.
pub fn power_home() -> PathBuf {
    if let Ok(home) = std::env::var("A3S_POWER_HOME") {
        return PathBuf::from(home);
    }

    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".a3s")
        .join("power")
}

/// Returns the directory where model manifest files are stored.
pub fn manifests_dir() -> PathBuf {
    power_home().join("models").join("manifests")
}

/// Returns the directory where model blob files (content-addressed) are stored.
pub fn blobs_dir() -> PathBuf {
    power_home().join("models").join("blobs")
}

/// Returns the path to the user configuration file.
pub fn config_path() -> PathBuf {
    power_home().join("config.toml")
}

/// Ensure all required directories exist.
pub fn ensure_dirs() -> std::io::Result<()> {
    std::fs::create_dir_all(manifests_dir())?;
    std::fs::create_dir_all(blobs_dir())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_power_home_default() {
        // Unset env var to test default path
        std::env::remove_var("A3S_POWER_HOME");
        let home = power_home();
        assert!(home.ends_with(".a3s/power") || home.ends_with(".a3s\\power"));
    }

    #[test]
    fn test_power_home_from_env() {
        std::env::set_var("A3S_POWER_HOME", "/tmp/test-power");
        let home = power_home();
        assert_eq!(home, PathBuf::from("/tmp/test-power"));
        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_manifests_dir() {
        std::env::set_var("A3S_POWER_HOME", "/tmp/test-power");
        assert_eq!(
            manifests_dir(),
            PathBuf::from("/tmp/test-power/models/manifests")
        );
        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_blobs_dir() {
        std::env::set_var("A3S_POWER_HOME", "/tmp/test-power");
        assert_eq!(blobs_dir(), PathBuf::from("/tmp/test-power/models/blobs"));
        std::env::remove_var("A3S_POWER_HOME");
    }

    #[test]
    fn test_config_path() {
        std::env::set_var("A3S_POWER_HOME", "/tmp/test-power");
        assert_eq!(config_path(), PathBuf::from("/tmp/test-power/config.toml"));
        std::env::remove_var("A3S_POWER_HOME");
    }
}
